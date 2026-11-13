use clap::Args;
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use appz_core::{ensure_mise, install_global_tool, mise_bin_paths};
use appz_deploy::{TunnelConfig, TunnelProviderKind, TunnelSession, start_tunnel};

use crate::deploy::{Consent, confirm_mise_install};
use crate::shield::{self, RequestLog, ShieldConfig, ShieldGateway};

/// mise tool spec that provides the `cloudflared` binary.
pub const CLOUDFLARED_MISE_TOOL: &str = "cloudflared";

/// Command-line arguments for `appz share`
#[derive(Args, Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShareArgs {
    /// Port or host:port to share (e.g. 3000, 8080, or 127.0.0.1:3000). Auto-detected if omitted.
    #[arg(value_name = "PORT")]
    pub port: Option<String>,

    /// Project directory (used for auto-detecting port if not specified)
    #[arg(short = 'd', long, default_value = ".")]
    pub dir: PathBuf,

    /// Require a 4-digit PIN to access the shared URL (auto-generated if omitted, or specify custom code)
    #[arg(long, value_name = "CODE", num_args = 0..=1, default_missing_value = "auto")]
    pub pin: Option<String>,

    /// Require HTTP Basic Auth in format username:password
    #[arg(long, value_name = "USER:PASS")]
    pub auth: Option<String>,

    /// Request a specific subdomain on supported tunnel providers
    #[arg(short = 's', long, value_name = "SUBDOMAIN")]
    pub subdomain: Option<String>,

    /// Session TTL (e.g. 30m, 1h, 2h) after which the tunnel automatically closes
    #[arg(long, default_value = "2h", value_name = "DURATION")]
    pub ttl: String,

    /// Enable interactive live request and webhook inspector
    #[arg(long)]
    pub inspect: bool,

    /// Install a missing tunnel backend (cloudflared, with mise) without asking
    #[arg(short = 'y', long)]
    pub yes: bool,
}

/// JSON output representation of a share request/session
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShareConfigJson {
    pub target_port: Option<u16>,
    pub pin_protected: bool,
    pub pin: Option<String>,
    pub auth_required: bool,
    pub subdomain: Option<String>,
    pub ttl: String,
    pub inspect: bool,
}

/// Parse raw port or host:port string into a port number
pub fn parse_target_port(raw: Option<&str>) -> Result<u16, String> {
    match raw {
        Some(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Err("port cannot be empty".to_string());
            }
            let stripped = trimmed
                .trim_start_matches("http://")
                .trim_start_matches("https://");
            let port_str = stripped.split(':').next_back().unwrap_or(stripped);
            port_str
                .parse::<u16>()
                .map_err(|_| format!("invalid port number: `{s}`"))
        }
        None => Err("no port specified".to_string()),
    }
}

/// Validate HTTP Basic Auth format (`username:password`)
pub fn validate_basic_auth(auth: &str) -> Result<(&str, &str), String> {
    let mut parts = auth.splitn(2, ':');
    let user = parts.next().unwrap_or("");
    let pass = parts.next().unwrap_or("");
    if user.is_empty() || pass.is_empty() {
        return Err("auth must be in format username:password".to_string());
    }
    Ok((user, pass))
}

/// Parse human-friendly duration string (e.g. "30m", "1h", "2h", "1d") into seconds
pub fn parse_ttl_duration(ttl: &str) -> Result<u64, String> {
    let ttl = ttl.trim();
    if ttl.is_empty() {
        return Err("TTL cannot be empty".to_string());
    }
    let (num_part, unit) = ttl.split_at(ttl.len().saturating_sub(1));
    let mult = match unit {
        "s" | "S" => 1,
        "m" | "M" => 60,
        "h" | "H" => 3600,
        "d" | "D" => 86400,
        _ => {
            // If whole string is a number, assume minutes
            return ttl
                .parse::<u64>()
                .map(|m| m * 60)
                .map_err(|_| format!("invalid TTL duration: `{ttl}` (expected e.g. 30m, 1h, 2h)"));
        }
    };

    let val = num_part
        .parse::<u64>()
        .map_err(|_| format!("invalid number in TTL: `{ttl}`"))?;
    Ok(val * mult)
}

/// The tunnel backend to run and, for cloudflared, where its binary is.
pub struct Backend {
    provider: TunnelProviderKind,
    cloudflared_bin: Option<PathBuf>,
}

const SSH_BACKEND: Backend = Backend {
    provider: TunnelProviderKind::Ssh,
    cloudflared_bin: None,
};

/// `cloudflared` on PATH or in mise's bin dirs (shims are not always
/// activated), the lookup `appz deploy` uses for platform CLIs.
fn find_cloudflared(dir: &Path) -> Option<PathBuf> {
    command::shell::find_command_with_paths("cloudflared", None, &mise_bin_paths(dir))
}

/// Pick the tunnel backend. cloudflared is used when present. When it is
/// missing, an interactive run chooses between installing it with mise and
/// ssh (nothing to install); `--yes` installs it; a non-interactive run uses
/// ssh when it can and otherwise fails with the install command.
pub fn resolve_backend(project_dir: &Path, install: Consent) -> Result<Backend, String> {
    let cloudflare = |bin| Backend {
        provider: TunnelProviderKind::Cloudflare,
        cloudflared_bin: Some(bin),
    };
    if let Some(bin) = find_cloudflared(project_dir) {
        return Ok(cloudflare(bin));
    }
    let missing = format!(
        "cloudflared is not installed. Install it with `mise use -g {CLOUDFLARED_MISE_TOOL}`"
    );
    let has_ssh = command::shell::find_command_on_path("ssh").is_some();
    let install = match (install, has_ssh) {
        (Consent::Ask, true) => {
            let pick = cliclack::select("cloudflared is not installed. How should appz share?")
                .item(
                    "install",
                    format!("Install {CLOUDFLARED_MISE_TOOL} with mise"),
                    "recommended",
                )
                .item("ssh", "Use ssh through localhost.run", "nothing to install")
                .interact()
                .map_err(|_| missing.clone())?;
            if pick == "ssh" {
                return Ok(SSH_BACKEND);
            }
            Consent::Yes
        }
        (Consent::Never, true) => return Ok(SSH_BACKEND),
        (policy, _) => policy,
    };
    confirm_mise_install(install, "cloudflared", CLOUDFLARED_MISE_TOOL, &missing)?;
    ensure_mise(project_dir)?;
    install_global_tool(CLOUDFLARED_MISE_TOOL)?;
    find_cloudflared(project_dir)
        .map(cloudflare)
        .ok_or_else(|| {
            "cloudflared was installed but is not on PATH - restart your shell or run `mise reshim`"
                .to_string()
        })
}

/// The PIN to require: the given code, or a random 4-digit one for `auto`
/// (a bare `--pin`).
pub fn resolve_pin(raw: &str) -> String {
    if raw != "auto" {
        return raw.to_string();
    }
    use std::hash::{BuildHasher, Hasher};
    let n = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    format!("{:04}", n % 10_000)
}

/// A public tunnel in front of the Shield gateway in front of a local port.
/// Dropping it closes both.
pub struct Session {
    pub public_url: String,
    pub provider: &'static str,
    tunnel: TunnelSession,
    _gateway: ShieldGateway,
}

impl Session {
    pub fn is_alive(&mut self) -> bool {
        self.tunnel.is_alive()
    }
}

/// Start the gateway for `shield.target_port` and a tunnel to the gateway.
pub fn open(backend: &Backend, shield: ShieldConfig) -> Result<Session, String> {
    let gateway = ShieldGateway::start(shield)?;
    let mut config = TunnelConfig::new(gateway.bound_port);
    config.provider = backend.provider;
    config.cloudflared_bin = backend.cloudflared_bin.clone();
    let tunnel = start_tunnel(&config).map_err(|e| e.to_string())?;
    Ok(Session {
        public_url: tunnel.public_url.clone(),
        provider: tunnel.provider_name,
        tunnel,
        _gateway: gateway,
    })
}

/// One inspector line: `200 GET   /api/hook  42ms`.
pub fn format_request(r: &RequestLog) -> String {
    let status = if r.status == 0 {
        "---".to_string()
    } else {
        r.status.to_string()
    };
    format!(
        "{status} {:<5} {}  {}ms",
        r.method,
        r.path,
        r.elapsed.as_millis()
    )
}

fn print_request(r: &RequestLog) {
    let time = chrono::Local::now().format("%H:%M:%S");
    println!("   {time}  {}", format_request(r));
}

/// Put `text` on the system clipboard with the platform's own tool.
fn copy_to_clipboard(text: &str) -> bool {
    let tools: &[(&str, &[&str])] = if cfg!(windows) {
        &[("clip", &[])]
    } else if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else {
        &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])]
    };
    tools.iter().any(|(bin, args)| {
        let Ok(mut child) = Command::new(bin)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        let wrote = child
            .stdin
            .take()
            .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
        child.wait().is_ok_and(|s| s.success()) && wrote
    })
}

/// Single key presses, lowercased. The reader stops after `q` or `p` so it
/// does not take input from whatever runs next.
fn spawn_key_reader() -> mpsc::Receiver<char> {
    let (tx, rx) = mpsc::channel();
    // fire-and-forget: blocked on the terminal; ends on q/p, a read error, or exit.
    std::thread::spawn(move || {
        let term = console::Term::stdout();
        loop {
            match term.read_key() {
                Ok(console::Key::Char(c)) => {
                    let c = c.to_ascii_lowercase();
                    if tx.send(c).is_err() || c == 'q' || c == 'p' {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });
    rx
}

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// How a share session ended.
enum End {
    Quit,
    Promote,
    Expired,
    TunnelClosed,
}

/// `appz share`: open the tunnel to `port`, show the URLs, then stream
/// requests and serve the shortcuts until the session ends.
pub fn run_session(args: &ShareArgs, dir: &Path, port: u16, ttl: Duration) {
    let install = if args.yes {
        Consent::Yes
    } else if crate::is_tty() {
        Consent::Ask
    } else {
        Consent::Never
    };
    let backend = resolve_backend(dir, install).unwrap_or_else(|e| crate::error(&e));
    if args.subdomain.is_some() {
        crate::warning("--subdomain is not available on quick tunnels; using a random URL");
    }

    let (tx, requests) = mpsc::channel();
    let mut shield = ShieldConfig::new(port);
    shield.pin = args.pin.as_deref().map(resolve_pin);
    shield.basic_auth = args.auth.clone();
    shield.ttl = ttl;
    shield.events = args.inspect.then_some(tx);
    let security = match (&shield.pin, &shield.basic_auth) {
        (Some(pin), Some(_)) => format!("PIN {pin} + basic auth"),
        (Some(pin), None) => format!("PIN {pin}"),
        (None, Some(_)) => "basic auth".to_string(),
        (None, None) => "none (anyone with the URL can open it)".to_string(),
    };

    let mut session = crate::with_spinner("opening tunnel...", "tunnel started", || {
        open(&backend, shield)
    })
    .unwrap_or_else(|e| crate::error(&e));
    let url = session.public_url.clone();
    crate::info(&format!(
        "local:    http://localhost:{port}\npublic:   {url}\nsecurity: {security}\nttl:      {}\nvia:      {}",
        args.ttl, session.provider
    ));

    // Ctrl+C has to end the loop, not the process, so the tunnel is closed.
    let _ = ctrlc::set_handler(|| INTERRUPTED.store(true, Ordering::SeqCst));
    let keys = crate::is_tty().then(spawn_key_reader);
    crate::info(match (&keys, args.inspect) {
        (Some(_), true) => "[r] replay last request  [c] copy url  [p] promote to deploy  [q] stop",
        (Some(_), false) => "[c] copy url  [p] promote to deploy  [q] stop",
        (None, _) => "press Ctrl+C to stop",
    });

    let started = Instant::now();
    let tick = Duration::from_millis(200);
    let mut last: Option<RequestLog> = None;
    let end = loop {
        if INTERRUPTED.load(Ordering::SeqCst) {
            break End::Quit;
        }
        if started.elapsed() >= ttl {
            break End::Expired;
        }
        if !session.is_alive() {
            break End::TunnelClosed;
        }
        match requests.recv_timeout(tick) {
            Ok(r) => {
                print_request(&r);
                last = Some(r);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            // Not inspecting: nothing ever arrives, so just wait out the tick.
            Err(mpsc::RecvTimeoutError::Disconnected) => std::thread::sleep(tick),
        }
        match keys.as_ref().and_then(|k| k.try_recv().ok()) {
            Some('q') => break End::Quit,
            Some('p') => break End::Promote,
            Some('c') => {
                if copy_to_clipboard(&url) {
                    crate::success("URL copied");
                } else {
                    crate::warning(&format!("could not reach the clipboard; the URL is {url}"));
                }
            }
            Some('r') => match &last {
                Some(r) => match shield::replay(port, &r.raw) {
                    Ok(status) => {
                        crate::success(&format!("replayed {} {}: {status}", r.method, r.path))
                    }
                    Err(e) => crate::warning(&format!("replay failed: {e}")),
                },
                None if args.inspect => crate::warning("no request to replay yet"),
                None => {}
            },
            _ => {}
        }
    };

    drop(session);
    match end {
        End::Quit => crate::outro("share session closed"),
        End::Expired => crate::outro(&format!("share session expired after {}", args.ttl)),
        End::TunnelClosed => crate::error("the tunnel process exited; run `appz share` again"),
        End::Promote => {
            crate::outro("share closed, deploying");
            let status = std::env::current_exe()
                .and_then(|exe| Command::new(exe).arg("deploy").arg(dir).status());
            match status {
                Ok(s) if s.success() => {}
                Ok(s) => std::process::exit(s.code().unwrap_or(1)),
                Err(e) => crate::error(&format!("could not run `appz deploy`: {e}")),
            }
        }
    }
}

/// The share half of `appz dev --share`. The dev server owns the terminal,
/// so the tunnel opens in the background once the server's port answers.
pub struct DevShare {
    session: Arc<Mutex<Option<Session>>>,
    stopped: Arc<AtomicBool>,
}

impl DevShare {
    /// With `inspect`, each forwarded request is printed between the dev
    /// server's own output.
    pub fn start(backend: Backend, mut shield: ShieldConfig, inspect: bool) -> Self {
        let port = shield.target_port;
        let requests = inspect.then(|| {
            let (tx, rx) = mpsc::channel();
            shield.events = Some(tx);
            rx
        });
        let session = Arc::new(Mutex::new(None));
        let stopped = Arc::new(AtomicBool::new(false));
        let (slot, stop) = (Arc::clone(&session), Arc::clone(&stopped));
        // fire-and-forget: polls `stopped`; the session it opens is handed to
        // `slot`, which `stop` empties.
        std::thread::spawn(move || {
            while !appz_core::is_port_listening(port, 200) {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
            match open(&backend, shield) {
                Ok(s) => {
                    let mut slot = slot.lock().unwrap();
                    // Checked under the lock: after `stop` nobody would close it.
                    if !stop.load(Ordering::SeqCst) {
                        crate::success(&format!(
                            "sharing http://localhost:{port} at {} (via {})",
                            s.public_url, s.provider
                        ));
                        *slot = Some(s);
                    }
                }
                Err(e) => crate::warning(&format!("share: {e}")),
            }
            // Ends when the gateway (which holds the sender) is closed.
            for r in requests.into_iter().flatten() {
                print_request(&r);
            }
        });
        Self { session, stopped }
    }

    /// Close the tunnel (or keep it from opening).
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.session.lock().unwrap().take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_pin_keeps_custom_and_generates_four_digits() {
        assert_eq!(resolve_pin("8492"), "8492");
        let auto = resolve_pin("auto");
        assert_eq!(auto.len(), 4);
        assert!(auto.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn format_request_shows_status_method_path_and_time() {
        let mut r = RequestLog {
            method: "GET".to_string(),
            path: "/api".to_string(),
            status: 200,
            elapsed: Duration::from_millis(42),
            raw: Vec::new(),
        };
        assert_eq!(format_request(&r), "200 GET   /api  42ms");
        r.status = 0;
        assert_eq!(format_request(&r), "--- GET   /api  42ms");
    }

    #[test]
    fn parse_target_port_handles_plain_number() {
        assert_eq!(parse_target_port(Some("3000")), Ok(3000));
        assert_eq!(parse_target_port(Some("8080")), Ok(8080));
    }

    #[test]
    fn parse_target_port_handles_host_prefix() {
        assert_eq!(parse_target_port(Some("localhost:3000")), Ok(3000));
        assert_eq!(parse_target_port(Some("127.0.0.1:5173")), Ok(5173));
        assert_eq!(parse_target_port(Some("http://localhost:8000")), Ok(8000));
        assert_eq!(parse_target_port(Some("https://0.0.0.0:4000")), Ok(4000));
    }

    #[test]
    fn parse_target_port_rejects_invalid() {
        assert!(parse_target_port(Some("not-a-port")).is_err());
        assert!(parse_target_port(Some("999999")).is_err());
        assert!(parse_target_port(Some("")).is_err());
        assert!(parse_target_port(None).is_err());
    }

    #[test]
    fn validate_basic_auth_checks_format() {
        assert_eq!(validate_basic_auth("user:pass"), Ok(("user", "pass")));
        assert_eq!(
            validate_basic_auth("admin:secret:with:colons"),
            Ok(("admin", "secret:with:colons"))
        );
        assert!(validate_basic_auth("nopass").is_err());
        assert!(validate_basic_auth(":pass").is_err());
        assert!(validate_basic_auth("user:").is_err());
    }

    #[test]
    fn parse_ttl_duration_parses_units() {
        assert_eq!(parse_ttl_duration("30s"), Ok(30));
        assert_eq!(parse_ttl_duration("15m"), Ok(900));
        assert_eq!(parse_ttl_duration("2h"), Ok(7200));
        assert_eq!(parse_ttl_duration("1d"), Ok(86400));
        assert_eq!(parse_ttl_duration("60"), Ok(3600)); // default minutes
        assert!(parse_ttl_duration("bad").is_err());
    }
}
