use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Configuration for the Shield Mode security gateway
#[derive(Debug, Clone)]
pub struct ShieldConfig {
    pub target_port: u16,
    pub pin: Option<String>,
    pub basic_auth: Option<String>,
    pub ttl: Duration,
}

impl ShieldConfig {
    pub fn new(target_port: u16) -> Self {
        Self {
            target_port,
            pin: None,
            basic_auth: None,
            ttl: Duration::from_secs(7200), // 2 hours default
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

        let _ = listener.set_nonblocking(true);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_flag);

        let handle = thread::spawn(move || {
            let created_at = Instant::now();
            while !stop_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut client_stream, _)) => {
                        let _ = client_stream.set_nonblocking(false);
                        let _ = client_stream.set_read_timeout(Some(Duration::from_secs(5)));
                        let _ = client_stream.set_write_timeout(Some(Duration::from_secs(5)));
                        handle_connection(&mut client_stream, &config, created_at);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
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

/// Check if the HTTP Cookie header contains a valid `appz_pin=<pin>` cookie
pub fn check_pin_cookie(headers: &str, expected_pin: &str) -> bool {
    for line in headers.lines() {
        if let Some(cookie_val) = line.strip_prefix("Cookie:") {
            for part in cookie_val.split(';') {
                let trimmed = part.trim();
                if let Some(val) = trimmed.strip_prefix("appz_pin=") {
                    if val.trim() == expected_pin {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check if the HTTP Authorization header matches `Basic <base64(user:pass)>`
pub fn check_basic_auth(headers: &str, expected_auth: &str) -> bool {
    // Look for Authorization: Basic ...
    for line in headers.lines() {
        if let Some(auth_val) = line.strip_prefix("Authorization:") {
            let trimmed = auth_val.trim();
            if let Some(b64) = trimmed.strip_prefix("Basic ") {
                // Decode base64
                let raw_b64 = b64.trim();
                // Simple standard base64 decode
                if let Ok(decoded_bytes) = simple_base64_decode(raw_b64) {
                    if let Ok(decoded_str) = std::str::from_utf8(&decoded_bytes) {
                        return decoded_str == expected_auth;
                    }
                }
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

fn handle_connection(stream: &mut TcpStream, config: &ShieldConfig, created_at: Instant) {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(0) | Err(_) => return,
        Ok(n) => n,
    };

    let req_str = String::from_utf8_lossy(&buf[..n]);

    // 1. Check TTL Expiration
    if is_expired(created_at, config.ttl) {
        let resp = "HTTP/1.1 503 Service Unavailable\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nAppz share session expired past TTL.\r\n";
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    // 2. Check Basic Auth
    if let Some(ref expected_auth) = config.basic_auth {
        if !check_basic_auth(&req_str, expected_auth) {
            let resp = "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"appz shield\"\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nAuthentication required\r\n";
            let _ = stream.write_all(resp.as_bytes());
            return;
        }
    }

    // 3. Check PIN Gatekeeper
    if let Some(ref expected_pin) = config.pin {
        // Check if request is unlock POST
        if req_str.starts_with("POST /__appz_unlock") {
            // Find body
            let pin_param = format!("pin={expected_pin}");
            if req_str.contains(&pin_param) {
                // Success: set cookie and redirect to root
                let resp = format!(
                    "HTTP/1.1 302 Found\r\nSet-Cookie: appz_pin={expected_pin}; Path=/; HttpOnly\r\nLocation: /\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(resp.as_bytes());
                return;
            } else {
                // Incorrect PIN: render page with error
                let html = render_unlock_html(true);
                let resp = format!(
                    "HTTP/1.1 401 Unauthorized\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html.len(),
                    html
                );
                let _ = stream.write_all(resp.as_bytes());
                return;
            }
        }

        // Check for cookie
        if !check_pin_cookie(&req_str, expected_pin) {
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
    if let Ok(mut target_stream) = TcpStream::connect(("127.0.0.1", config.target_port)) {
        let _ = target_stream.write_all(&buf[..n]);
        let mut target_buf = [0u8; 8192];
        while let Ok(tn) = target_stream.read(&mut target_buf) {
            if tn == 0 {
                break;
            }
            if stream.write_all(&target_buf[..tn]).is_err() {
                break;
            }
        }
    } else {
        let resp = "HTTP/1.1 502 Bad Gateway\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nCannot connect to target local dev server.\r\n";
        let _ = stream.write_all(resp.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
