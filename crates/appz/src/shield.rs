use rand::RngCore;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Configuration for the Shield Mode security gateway
#[derive(Debug, Clone)]
pub struct ShieldConfig {
    pub target_port: u16,
    pub pin: Option<String>,
    pub basic_auth: Option<String>,
    pub ttl: Duration,
    /// Receives one entry per request forwarded to the dev server.
    pub events: Option<mpsc::Sender<RequestLog>>,
}

/// One request forwarded to the dev server, for the live inspector.
#[derive(Debug, Clone)]
pub struct RequestLog {
    pub method: String,
    pub path: String,
    /// Reply status; 0 when the dev server sent no status line.
    pub status: u16,
    pub elapsed: Duration,
    /// The request as received (head plus whatever body arrived with it),
    /// kept so it can be replayed.
    pub raw: Vec<u8>,
}

impl ShieldConfig {
    pub fn new(target_port: u16) -> Self {
        Self {
            target_port,
            pin: None,
            basic_auth: None,
            ttl: Duration::from_secs(7200), // 2 hours default
            events: None,
        }
    }
}

/// A running Shield Mode local proxy gateway
pub struct ShieldGateway {
    pub bound_port: u16,
    stop_flag: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ShieldGateway {
    /// Start the Shield Mode HTTP proxy gateway on an ephemeral local port
    pub fn start(config: ShieldConfig) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("failed to bind Shield gateway port: {e}"))?;
        let bound_port = listener
            .local_addr()
            .map_err(|e| format!("cannot read bound port: {e}"))?
            .port();

        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_flag);

        let pin_token = config.pin.as_deref().map(generate_pin_session_token);
        let throttle = Arc::new(UnlockThrottle::default());
        let config = Arc::new(config);
        let handle = thread::spawn(move || {
            let created_at = Instant::now();
            // Blocks in accept (no polling); `stop` wakes it with a connection.
            for stream in listener.incoming() {
                if stop_clone.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(mut client_stream) = stream else {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                };
                let _ = client_stream.set_nodelay(true);
                let _ = client_stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = client_stream.set_write_timeout(Some(Duration::from_secs(5)));
                let config = Arc::clone(&config);
                let pin_token = pin_token.clone();
                let throttle = Arc::clone(&throttle);
                // fire-and-forget: one connection, ends when either side closes.
                // A thread each so a slow or long-lived (websocket) connection
                // does not stall the others.
                thread::spawn(move || {
                    handle_connection(
                        &mut client_stream,
                        &config,
                        created_at,
                        pin_token.as_deref(),
                        &throttle,
                    );
                });
            }
        });

        Ok(Self {
            bound_port,
            stop_flag,
            handle: Some(handle),
        })
    }

    /// Stop the gateway listener and worker thread
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            // Wake the accept loop so it sees the flag.
            let _ = TcpStream::connect(("127.0.0.1", self.bound_port));
            let _ = h.join();
        }
    }
}

impl Drop for ShieldGateway {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Check if the session has expired past TTL
pub fn is_expired(created_at: Instant, ttl: Duration) -> bool {
    created_at.elapsed() >= ttl
}

const MAX_PIN_ATTEMPTS: u32 = 5;
const PIN_LOCKOUT_SECS: u64 = 30;
const MAX_UNLOCK_BODY: usize = 4096;

#[derive(Default)]
struct UnlockThrottle {
    state: Mutex<(u32, Option<Instant>)>,
}

impl UnlockThrottle {
    fn remaining_lockout_secs(&self) -> Option<u64> {
        let mut guard = self.state.lock().unwrap();
        let until = guard.1?;
        let now = Instant::now();
        if until > now {
            Some((until - now).as_secs().max(1))
        } else {
            guard.1 = None;
            None
        }
    }

    fn record_failure(&self) {
        let mut guard = self.state.lock().unwrap();
        guard.0 = guard.0.saturating_add(1);
        if guard.0 >= MAX_PIN_ATTEMPTS {
            guard.0 = 0;
            guard.1 = Some(Instant::now() + Duration::from_secs(PIN_LOCKOUT_SECS));
        }
    }

    fn reset(&self) {
        let mut guard = self.state.lock().unwrap();
        guard.0 = 0;
        guard.1 = None;
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn generate_pin_session_token(pin: &str) -> String {
    let mut secret = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret);
    let mut hasher = Sha256::new();
    hasher.update(secret);
    hasher.update(b":");
    hasher.update(pin.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn header_lines(headers: &str) -> impl Iterator<Item = (&str, &str)> {
    headers
        .lines()
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let (name, val) = line.split_once(':')?;
            Some((name.trim(), val.trim()))
        })
}

/// Check if the HTTP Cookie header contains a valid `appz_pin=<token>` cookie
pub fn check_pin_cookie(headers: &str, expected_token: &str) -> bool {
    for (name, cookie_val) in header_lines(headers) {
        if name.eq_ignore_ascii_case("cookie") {
            for part in cookie_val.split(';') {
                if let Some(val) = part.trim().strip_prefix("appz_pin=")
                    && constant_time_eq(val.trim().as_bytes(), expected_token.as_bytes())
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if the HTTP Authorization header matches `Basic <base64(user:pass)>`
pub fn check_basic_auth(headers: &str, expected_auth: &str) -> bool {
    for (name, auth_val) in header_lines(headers) {
        if name.eq_ignore_ascii_case("authorization")
            && auth_val.len() >= 6
            && auth_val[..6].eq_ignore_ascii_case("basic ")
        {
            let raw_b64 = auth_val[6..].trim();
            if let Ok(decoded_bytes) = simple_base64_decode(raw_b64)
                && let Ok(decoded_str) = std::str::from_utf8(&decoded_bytes)
            {
                return constant_time_eq(decoded_str.as_bytes(), expected_auth.as_bytes());
            }
        }
    }
    false
}

fn simple_base64_decode(input: &str) -> Result<Vec<u8>, ()> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for &b in input.as_bytes() {
        if b == b'=' {
            break;
        }
        let val = TABLE.iter().position(|&c| c == b).ok_or(())? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

/// Generate modern dark-themed HTML unlock page for PIN authentication
pub fn render_unlock_html(pin_failed: bool) -> String {
    let error_banner = if pin_failed {
        r#"<div style="color: #ef4444; margin-bottom: 1rem; font-size: 0.875rem;">Incorrect PIN. Please try again.</div>"#
    } else {
        ""
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Appz Shield • Authentication Required</title>
    <style>
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            background: #090a0f;
            color: #f3f4f6;
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            display: flex;
            align-items: center;
            justify-content: center;
            min-height: 100vh;
        }}
        .card {{
            background: #12141c;
            border: 1px solid #232738;
            border-radius: 12px;
            padding: 2.5rem;
            width: 100%;
            max-width: 400px;
            box-shadow: 0 8px 32px rgba(0,0,0,0.4);
            text-align: center;
        }}
        .logo {{ font-weight: 700; font-size: 1.25rem; letter-spacing: -0.025em; margin-bottom: 0.5rem; color: #60a5fa; }}
        .title {{ font-size: 1.125rem; font-weight: 600; margin-bottom: 0.5rem; }}
        .desc {{ font-size: 0.875rem; color: #9ca3af; margin-bottom: 1.5rem; }}
        input[type="password"], input[type="text"] {{
            width: 100%;
            padding: 0.75rem 1rem;
            background: #1a1d29;
            border: 1px solid #32374d;
            border-radius: 8px;
            color: #fff;
            font-size: 1.25rem;
            letter-spacing: 0.25em;
            text-align: center;
            margin-bottom: 1rem;
            outline: none;
        }}
        input:focus {{ border-color: #3b82f6; }}
        button {{
            width: 100%;
            padding: 0.75rem 1rem;
            background: #2563eb;
            color: #fff;
            font-weight: 600;
            border: none;
            border-radius: 8px;
            cursor: pointer;
            font-size: 0.875rem;
        }}
        button:hover {{ background: #1d4ed8; }}
        .footer {{ margin-top: 1.5rem; font-size: 0.75rem; color: #6b7280; }}
    </style>
</head>
<body>
    <div class="card">
        <div class="logo">appz • shield</div>
        <div class="title">Protected Development Server</div>
        <div class="desc">Enter the 4-digit PIN displayed in the host terminal to unlock access.</div>
        {error_banner}
        <form method="POST" action="/__appz_unlock">
            <input type="password" name="pin" maxlength="12" placeholder="••••" autofocus required>
            <button type="submit">Unlock Session</button>
        </form>
        <div class="footer">Secured by Appz Guardrails</div>
    </div>
</body>
</html>"#
    )
}

fn parse_content_length(headers: &str) -> usize {
    for (name, val) in header_lines(headers) {
        if name.eq_ignore_ascii_case("content-length")
            && let Ok(len) = val.parse::<usize>()
        {
            return len;
        }
    }
    0
}

fn decode_form_component(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                let h1 = *bytes.get(i + 1)? as char;
                let h2 = *bytes.get(i + 2)? as char;
                let hi = h1.to_digit(16)? as u8;
                let lo = h2.to_digit(16)? as u8;
                out.push((hi << 4) | lo);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn parse_form_pin(body: &str) -> Option<String> {
    let mut found = None;
    for pair in body.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=')?;
        if decode_form_component(k)?.as_str() == "pin" {
            if found.is_some() {
                return None;
            }
            found = Some(decode_form_component(v)?);
        }
    }
    found
}

fn read_unlock_body(stream: &mut TcpStream, initial: &[u8]) -> Option<String> {
    let end = head_end(initial)?;
    let headers = String::from_utf8_lossy(&initial[..end]);
    let content_len = parse_content_length(&headers);
    if content_len == 0 || content_len > MAX_UNLOCK_BODY {
        return None;
    }
    let body_start = end + HEAD_END.len();
    let mut body = initial[body_start..].to_vec();
    while body.len() < content_len {
        let mut buf = [0u8; 1024];
        let want = (content_len - body.len()).min(buf.len());
        match stream.read(&mut buf[..want]) {
            Ok(0) | Err(_) => break,
            Ok(n) => body.extend_from_slice(&buf[..n]),
        }
    }
    if body.len() < content_len {
        return None;
    }
    body.truncate(content_len);
    String::from_utf8(body).ok()
}

fn is_unlock_request(req_head: &str) -> bool {
    let (method, path) = request_line(req_head);
    let clean_path = path.split('?').next().unwrap_or(&path);
    method == "POST" && clean_path == "/__appz_unlock"
}

fn handle_connection(
    stream: &mut TcpStream,
    config: &ShieldConfig,
    created_at: Instant,
    pin_token: Option<&str>,
    throttle: &UnlockThrottle,
) {
    let Some(request) = read_head(stream) else {
        return;
    };
    let head_bytes = head_end(&request).map_or(request.len(), |e| e + HEAD_END.len());
    let req_head = String::from_utf8_lossy(&request[..head_bytes]);

    // 1. Check TTL Expiration
    if is_expired(created_at, config.ttl) {
        let resp = "HTTP/1.1 503 Service Unavailable\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nAppz share session expired past TTL.\r\n";
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    // 2. Check Basic Auth
    if let Some(ref expected_auth) = config.basic_auth
        && !check_basic_auth(&req_head, expected_auth)
    {
        let resp = "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"appz shield\"\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nAuthentication required\r\n";
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    // 3. Check PIN Gatekeeper
    if let Some(ref expected_pin) = config.pin {
        let expected_cookie = pin_token.unwrap_or(expected_pin.as_str());
        if is_unlock_request(&req_head) {
            if let Some(wait_secs) = throttle.remaining_lockout_secs() {
                let resp = format!(
                    "HTTP/1.1 429 Too Many Requests\r\nRetry-After: {wait_secs}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nToo many incorrect PIN attempts. Try again in {wait_secs}s.\r\n"
                );
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
            let submitted = read_unlock_body(stream, &request).and_then(|b| parse_form_pin(&b));
            if let Some(pin) = submitted
                && constant_time_eq(pin.as_bytes(), expected_pin.as_bytes())
            {
                throttle.reset();
                let resp = format!(
                    "HTTP/1.1 302 Found\r\nSet-Cookie: appz_pin={expected_cookie}; Path=/; HttpOnly; SameSite=Lax\r\nLocation: /\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
            throttle.record_failure();
            let html = render_unlock_html(true);
            let resp = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = stream.write_all(resp.as_bytes());
            return;
        }

        // Check for cookie
        if !check_pin_cookie(&req_head, expected_cookie) {
            let html = render_unlock_html(false);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = stream.write_all(resp.as_bytes());
            return;
        }
    }

    // 4. Forward authorized traffic to target dev server
    let started = Instant::now();
    let status = forward(stream, config.target_port, &request);
    if let Some(tx) = &config.events {
        let (method, path) = request_line(&req_head);
        let _ = tx.send(RequestLog {
            method,
            path,
            status,
            elapsed: started.elapsed(),
            raw: request.clone(),
        });
    }
}

const HEAD_END: &[u8] = b"\r\n\r\n";
/// How long to wait for the dev server to accept a connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// Largest request head read before forwarding regardless.
const MAX_HEAD: usize = 64 * 1024;

fn head_end(data: &[u8]) -> Option<usize> {
    data.windows(HEAD_END.len()).position(|w| w == HEAD_END)
}

/// Read until the end of the request head (the body may follow later).
fn read_head(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    while head_end(&data).is_none() && data.len() < MAX_HEAD {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    }
    (!data.is_empty()).then_some(data)
}

/// `(method, path)` from the request line.
fn request_line(request: &str) -> (String, String) {
    let mut parts = request.lines().next().unwrap_or("").split_whitespace();
    (
        parts.next().unwrap_or("?").to_string(),
        parts.next().unwrap_or("/").to_string(),
    )
}

/// Status code from the start of a reply (`HTTP/1.1 200 OK`), 0 if absent.
fn parse_status(reply: &[u8]) -> u16 {
    String::from_utf8_lossy(&reply[..reply.len().min(32)])
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn is_upgrade(request: &[u8]) -> bool {
    String::from_utf8_lossy(request)
        .to_ascii_lowercase()
        .contains("\r\nupgrade:")
}

/// Rewrite the request head for the dev server:
/// - `Host` becomes `localhost:<port>`. Dev servers (Vite, Astro) refuse a
///   host they do not know; the visitor's host moves to `X-Forwarded-Host`.
/// - Unless `keep_alive` (a websocket upgrade), `Connection` becomes `close`,
///   so the dev server ends the reply by closing and every request arrives on
///   its own connection. The tunnel client pools connections across visitors;
///   without this, a visitor could ride a connection another one authenticated.
fn rewrite_head(request: &[u8], target_port: u16, keep_alive: bool) -> Vec<u8> {
    let Some(end) = head_end(request) else {
        return request.to_vec();
    };
    let head = String::from_utf8_lossy(&request[..end]);
    let mut out = String::with_capacity(request.len() + 96);
    let mut public_host = None;
    for line in head.split("\r\n") {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("host:") {
            public_host = Some(line[5..].trim().to_string());
        } else if keep_alive || !lower.starts_with("connection:") {
            out.push_str(line);
            out.push_str("\r\n");
        }
    }
    out.push_str(&format!("Host: localhost:{target_port}\r\n"));
    if let Some(host) = public_host {
        out.push_str(&format!(
            "X-Forwarded-Host: {host}\r\nX-Forwarded-Proto: https\r\n"
        ));
    }
    if !keep_alive {
        out.push_str("Connection: close\r\n");
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(&request[end + HEAD_END.len()..]);
    bytes
}

/// Send `request` (already read) plus whatever the client sends next to the
/// dev server and stream the reply back. Returns the reply status: 502 when
/// the dev server is unreachable, 0 when it sent no status line.
fn forward(client: &mut TcpStream, target_port: u16, request: &[u8]) -> u16 {
    let Ok(mut target) = appz_core::connect_local(target_port, CONNECT_TIMEOUT) else {
        let resp = "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nCannot connect to target local dev server.\r\n";
        let _ = client.write_all(resp.as_bytes());
        return 502;
    };
    // A websocket (HMR) stays open both ways, so it keeps `Connection`.
    let head = rewrite_head(request, target_port, is_upgrade(request));
    if target.write_all(&head).is_err() {
        return 0;
    }
    // The head is in: the rest (upload body, websocket frames) has no deadline.
    let _ = client.set_read_timeout(None);
    let _ = target.set_nodelay(true);
    if let (Ok(mut from), Ok(mut to)) = (client.try_clone(), target.try_clone()) {
        // fire-and-forget: ends when the client closes or the reply loop below
        // shuts both sockets down.
        thread::spawn(move || {
            let _ = std::io::copy(&mut from, &mut to);
            let _ = to.shutdown(Shutdown::Write);
        });
    }
    let mut status = None;
    let mut buf = vec![0u8; 64 * 1024];
    while let Ok(n) = target.read(&mut buf) {
        if n == 0 {
            break;
        }
        status.get_or_insert_with(|| parse_status(&buf[..n]));
        if client.write_all(&buf[..n]).is_err() {
            break;
        }
    }
    let _ = client.shutdown(Shutdown::Both);
    let _ = target.shutdown(Shutdown::Both);
    status.unwrap_or(0)
}

/// Re-send a captured request to the dev server and return the reply status.
pub fn replay(target_port: u16, request: &[u8]) -> Result<u16, String> {
    let mut target = appz_core::connect_local(target_port, CONNECT_TIMEOUT)
        .map_err(|e| format!("cannot connect to port {target_port}: {e}"))?;
    let _ = target.set_read_timeout(Some(Duration::from_secs(30)));
    target
        .write_all(&rewrite_head(request, target_port, false))
        .map_err(|e| e.to_string())?;
    let mut buf = [0u8; 64];
    let n = target.read(&mut buf).map_err(|e| e.to_string())?;
    Ok(parse_status(&buf[..n]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_head_sets_local_host_and_closes_the_connection() {
        let req = b"POST /hook HTTP/1.1\r\nHost: abc.trycloudflare.com\r\nconnection: keep-alive\r\n\r\nbody";
        assert_eq!(
            String::from_utf8(rewrite_head(req, 4321, false)).unwrap(),
            "POST /hook HTTP/1.1\r\nHost: localhost:4321\r\nX-Forwarded-Host: abc.trycloudflare.com\r\nX-Forwarded-Proto: https\r\nConnection: close\r\n\r\nbody"
        );
    }

    #[test]
    fn rewrite_head_keeps_connection_for_a_websocket_upgrade() {
        let req = b"GET /ws HTTP/1.1\r\nHost: abc.example\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n";
        let out = String::from_utf8(rewrite_head(req, 5173, true)).unwrap();
        assert!(out.contains("Connection: Upgrade\r\n"), "{out}");
        assert!(out.contains("Host: localhost:5173\r\n"), "{out}");
        assert!(!out.contains("Connection: close"), "{out}");
    }

    #[test]
    fn request_line_and_status_are_parsed() {
        assert_eq!(
            request_line("GET /a?b=1 HTTP/1.1\r\nHost: x\r\n"),
            ("GET".to_string(), "/a?b=1".to_string())
        );
        assert_eq!(parse_status(b"HTTP/1.1 404 Not Found\r\n"), 404);
        assert_eq!(parse_status(b"garbage"), 0);
    }

    #[test]
    fn upgrade_requests_are_detected() {
        assert!(is_upgrade(
            b"GET /ws HTTP/1.1\r\nUpgrade: websocket\r\n\r\n"
        ));
        assert!(!is_upgrade(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n"));
    }

    /// End to end through the gateway: the request reaches the dev server
    /// with `Connection: close`, the reply comes back, and it is logged.
    #[test]
    fn gateway_forwards_and_logs_a_request() {
        let upstream = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = upstream.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut s, _) = upstream.accept().unwrap();
            let head = read_head(&mut s).unwrap();
            s.write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nok")
                .unwrap();
            String::from_utf8(head).unwrap()
        });

        let (tx, rx) = mpsc::channel();
        let mut config = ShieldConfig::new(port);
        config.events = Some(tx);
        let gateway = ShieldGateway::start(config).unwrap();

        let mut client = TcpStream::connect(("127.0.0.1", gateway.bound_port)).unwrap();
        client
            .write_all(b"GET /hello HTTP/1.1\r\nHost: x\r\nConnection: keep-alive\r\n\r\n")
            .unwrap();
        let mut reply = String::new();
        client.read_to_string(&mut reply).unwrap();

        assert!(reply.starts_with("HTTP/1.1 201 Created"), "{reply}");
        assert!(reply.ends_with("ok"));
        assert!(server.join().unwrap().contains("Connection: close"));
        let log = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!((log.method.as_str(), log.path.as_str()), ("GET", "/hello"));
        assert_eq!(log.status, 201);
        assert_eq!(replay(port, &log.raw).ok(), None, "upstream is gone");
    }

    #[test]
    fn check_pin_cookie_detects_matching_cookie() {
        let headers =
            "GET / HTTP/1.1\r\nHost: localhost\r\nCookie: session=xyz; appz_pin=7391; dark=1\r\n";
        assert!(check_pin_cookie(headers, "7391"));
        assert!(!check_pin_cookie(headers, "0000"));
        assert!(!check_pin_cookie(
            "GET / HTTP/1.1\r\nHost: localhost\r\n",
            "7391"
        ));
    }

    #[test]
    fn check_basic_auth_validates_base64() {
        // "admin:secret" base64 is "YWRtaW46c2VjcmV0"
        let headers = "GET /api HTTP/1.1\r\nAuthorization: Basic YWRtaW46c2VjcmV0\r\n";
        assert!(check_basic_auth(headers, "admin:secret"));
        assert!(!check_basic_auth(headers, "admin:wrong"));
        assert!(!check_basic_auth("GET / HTTP/1.1\r\n", "admin:secret"));
    }

    #[test]
    fn is_expired_detects_past_duration() {
        let created_at = Instant::now() - Duration::from_secs(10);
        assert!(is_expired(created_at, Duration::from_secs(5)));
        assert!(!is_expired(created_at, Duration::from_secs(60)));
    }

    #[test]
    fn render_unlock_html_contains_required_elements() {
        let normal = render_unlock_html(false);
        assert!(normal.contains("Appz Shield"));
        assert!(normal.contains("/__appz_unlock"));
        assert!(!normal.contains("Incorrect PIN"));

        let failed = render_unlock_html(true);
        assert!(failed.contains("Incorrect PIN"));
    }

    #[test]
    fn header_checks_are_case_insensitive_and_ignore_body() {
        let headers = "GET / HTTP/1.1\r\nhost: localhost\r\ncookie: appz_pin=tok123\r\nauthorization: basic YWRtaW46c2VjcmV0\r\n\r\n";
        assert!(check_pin_cookie(headers, "tok123"));
        assert!(check_basic_auth(headers, "admin:secret"));

        let spoofed_in_body = "POST / HTTP/1.1\r\nHost: localhost\r\n\r\nCookie: appz_pin=tok123\r\nAuthorization: Basic YWRtaW46c2VjcmV0\r\n";
        assert!(!check_pin_cookie(spoofed_in_body, "tok123"));
        assert!(!check_basic_auth(spoofed_in_body, "admin:secret"));
    }

    #[test]
    fn form_pin_parser_is_exact() {
        assert_eq!(parse_form_pin("pin=1234"), Some("1234".to_string()));
        assert_eq!(parse_form_pin("foo=1&pin=12%334"), Some("1234".to_string()));
        assert_eq!(parse_form_pin("pin=12340"), Some("12340".to_string()));
        assert_eq!(parse_form_pin("pin=1234&pin=9999"), None);
        assert_eq!(parse_form_pin("other=1234"), None);
    }

    #[test]
    fn pin_unlock_rejects_prefix_sets_opaque_cookie_and_throttles() {
        let upstream = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = upstream.local_addr().unwrap().port();
        thread::spawn(move || {
            while let Ok((mut s, _)) = upstream.accept() {
                let _ = read_head(&mut s);
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
            }
        });

        let mut config = ShieldConfig::new(port);
        config.pin = Some("1234".to_string());
        let gateway = ShieldGateway::start(config).unwrap();

        let send = |raw: &[u8]| -> String {
            let mut c = TcpStream::connect(("127.0.0.1", gateway.bound_port)).unwrap();
            c.write_all(raw).unwrap();
            let mut out = String::new();
            c.read_to_string(&mut out).unwrap();
            out
        };

        // Prefix match (`pin=12340`) must be rejected.
        let bad =
            send(b"POST /__appz_unlock HTTP/1.1\r\nHost: x\r\nContent-Length: 9\r\n\r\npin=12340");
        assert!(bad.starts_with("HTTP/1.1 401"), "{bad}");

        // Exact PIN succeeds, sets HttpOnly SameSite=Lax token cookie that does not leak `1234`.
        let ok =
            send(b"POST /__appz_unlock HTTP/1.1\r\nHost: x\r\nContent-Length: 8\r\n\r\npin=1234");
        assert!(ok.starts_with("HTTP/1.1 302"), "{ok}");
        let set_cookie = ok
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("set-cookie:"))
            .unwrap();
        assert!(set_cookie.contains("HttpOnly"), "{set_cookie}");
        assert!(set_cookie.contains("SameSite=Lax"), "{set_cookie}");
        let cookie_kv = set_cookie
            .split(':')
            .nth(1)
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .trim();
        assert_ne!(cookie_kv, "appz_pin=1234");

        // Using the issued session cookie reaches upstream.
        let authed =
            send(format!("GET / HTTP/1.1\r\nHost: x\r\nCookie: {cookie_kv}\r\n\r\n").as_bytes());
        assert!(authed.starts_with("HTTP/1.1 200 OK"), "{authed}");
        assert!(authed.ends_with("ok"), "{authed}");

        // 5 failed attempts trigger 429 lockout.
        for _ in 0..5 {
            let r = send(
                b"POST /__appz_unlock HTTP/1.1\r\nHost: x\r\nContent-Length: 8\r\n\r\npin=0000",
            );
            assert!(r.starts_with("HTTP/1.1 401"), "{r}");
        }
        let locked =
            send(b"POST /__appz_unlock HTTP/1.1\r\nHost: x\r\nContent-Length: 8\r\n\r\npin=1234");
        assert!(locked.starts_with("HTTP/1.1 429"), "{locked}");
    }
}
