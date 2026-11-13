use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

/// Returns the standard default development port for a recognized framework.
pub fn framework_default_port(framework_name: &str) -> Option<u16> {
    let lower = framework_name.to_lowercase();
    if lower.contains("next")
        || lower.contains("react")
        || lower.contains("remix")
        || lower.contains("nuxt")
        || lower.contains("rails")
    {
        Some(3000)
    } else if lower.contains("vite") || lower.contains("svelte") {
        Some(5173)
    } else if lower.contains("astro") {
        Some(4321)
    } else if lower.contains("angular") {
        Some(4200)
    } else if lower.contains("gatsby")
        || lower.contains("django")
        || lower.contains("fastapi")
        || lower.contains("uvicorn")
    {
        Some(8000)
    } else if lower.contains("vue")
        || lower.contains("spring")
        || lower.contains("actix")
        || lower.contains("rocket")
        || lower.contains("axum")
    {
        Some(8080)
    } else if lower.contains("flask") {
        Some(5000)
    } else if lower.contains("phoenix") || lower.contains("elixir") {
        Some(4000)
    } else {
        None
    }
}

/// Attempts to parse a port argument or environment variable from a dev command string.
///
/// Handles forms like:
/// - `-p 4000` or `-p=4000`
/// - `--port 8080` or `--port=8080`
/// - `PORT=5000 npm run dev`
pub fn parse_port_from_dev_command(cmd: &str) -> Option<u16> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        // Handle PORT=3000 prefix
        if let Some(rest) = part.strip_prefix("PORT=")
            && let Ok(p) = rest.parse::<u16>()
        {
            return Some(p);
        }
        // Handle --port=3000 or -p=3000
        if let Some(rest) = part.strip_prefix("--port=")
            && let Ok(p) = rest.parse::<u16>()
        {
            return Some(p);
        }
        if let Some(rest) = part.strip_prefix("-p=")
            && let Ok(p) = rest.parse::<u16>()
        {
            return Some(p);
        }
        // Handle --port 3000 or -p 3000
        if (*part == "--port" || *part == "-p")
            && i + 1 < parts.len()
            && let Ok(p) = parts[i + 1].parse::<u16>()
        {
            return Some(p);
        }
    }
    None
}

/// Probes whether a TCP socket is listening at `127.0.0.1:<port>` within a short timeout.
pub fn is_port_listening(port: u16, timeout_ms: u64) -> bool {
    connect_local(port, Duration::from_millis(timeout_ms)).is_ok()
}

/// Connect to a dev server on this machine. Tries IPv4 then IPv6 loopback:
/// a server bound to `localhost` (Vite, Astro) may listen on `::1` only.
pub fn connect_local(port: u16, timeout: Duration) -> std::io::Result<TcpStream> {
    let v4 = SocketAddr::from(([127, 0, 0, 1], port));
    let v6 = SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&v4, timeout).or_else(|_| TcpStream::connect_timeout(&v6, timeout))
}

/// Probes a slice of candidate ports, returning the first one with an active TCP listener.
pub fn find_active_listener(candidates: &[u16], timeout_ms: u64) -> Option<u16> {
    candidates
        .iter()
        .find(|&&port| is_port_listening(port, timeout_ms))
        .copied()
}

/// Detects the target dev server port for a project.
///
/// Order of evaluation:
/// 1. `PORT` environment variable (if set and valid).
/// 2. Explicit port flags in `dev_command` (e.g. `-p 4000`, `--port 8080`).
/// 3. Active listener on any known framework default port.
/// 4. Known framework default port from detected frameworks.
/// 5. First active listener among standard common dev ports.
/// 6. Fallback default (3000).
pub fn detect_dev_port(_root: &Path, framework_names: &[&str], dev_command: Option<&str>) -> u16 {
    // 1. PORT env var
    if let Ok(val) = std::env::var("PORT")
        && let Ok(p) = val.trim().parse::<u16>()
    {
        return p;
    }

    // 2. Command flag
    if let Some(cmd) = dev_command
        && let Some(p) = parse_port_from_dev_command(cmd)
    {
        return p;
    }

    // 3. Framework defaults
    let mut candidate_ports = Vec::new();
    for fw in framework_names {
        if let Some(p) = framework_default_port(fw)
            && !candidate_ports.contains(&p)
        {
            candidate_ports.push(p);
        }
    }

    // Check if any framework candidate is actively listening (fast 50ms probe)
    if let Some(active) = find_active_listener(&candidate_ports, 50) {
        return active;
    }

    // If candidate found from framework, use it
    if let Some(&first) = candidate_ports.first() {
        return first;
    }

    // Common fallback ports to probe
    const COMMON_PORTS: &[u16] = &[3000, 5173, 8080, 8000, 4321, 4200, 5000, 4000];
    if let Some(active) = find_active_listener(COMMON_PORTS, 50) {
        return active;
    }

    // Default universal fallback
    3000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_local_reaches_an_ipv6_only_listener() {
        // Not every CI host has IPv6 loopback; nothing to check without it.
        let Ok(listener) = std::net::TcpListener::bind("[::1]:0") else {
            return;
        };
        let port = listener.local_addr().unwrap().port();
        assert!(is_port_listening(port, 500));
    }

    #[test]
    fn framework_default_port_maps_correctly() {
        assert_eq!(framework_default_port("Next.js"), Some(3000));
        assert_eq!(framework_default_port("React"), Some(3000));
        assert_eq!(framework_default_port("Remix"), Some(3000));
        assert_eq!(framework_default_port("Vite"), Some(5173));
        assert_eq!(framework_default_port("Svelte"), Some(5173));
        assert_eq!(framework_default_port("Astro"), Some(4321));
        assert_eq!(framework_default_port("Angular"), Some(4200));
        assert_eq!(framework_default_port("Gatsby"), Some(8000));
        assert_eq!(framework_default_port("Django"), Some(8000));
        assert_eq!(framework_default_port("FastAPI"), Some(8000));
        assert_eq!(framework_default_port("Vue"), Some(8080));
        assert_eq!(framework_default_port("Flask"), Some(5000));
        assert_eq!(framework_default_port("Phoenix"), Some(4000));
        assert_eq!(framework_default_port("UnknownFramework"), None);
    }

    #[test]
    fn parse_port_from_dev_command_extracts_flags() {
        assert_eq!(parse_port_from_dev_command("next dev -p 4000"), Some(4000));
        assert_eq!(parse_port_from_dev_command("vite --port 5174"), Some(5174));
        assert_eq!(parse_port_from_dev_command("vite --port=5175"), Some(5175));
        assert_eq!(
            parse_port_from_dev_command("PORT=8001 npm run dev"),
            Some(8001)
        );
        assert_eq!(parse_port_from_dev_command("npm run dev"), None);
        assert_eq!(parse_port_from_dev_command("cargo run"), None);
    }

    #[test]
    fn detect_dev_port_prioritizes_framework() {
        let temp = std::env::temp_dir();
        let port = detect_dev_port(&temp, &["Vite"], None);
        assert_eq!(port, 5173);

        let port_next = detect_dev_port(&temp, &["Next.js"], None);
        assert_eq!(port_next, 3000);

        let port_astro = detect_dev_port(&temp, &["Astro"], None);
        assert_eq!(port_astro, 4321);
    }

    #[test]
    fn detect_dev_port_respects_explicit_command_flag() {
        let temp = std::env::temp_dir();
        let port = detect_dev_port(&temp, &["Next.js"], Some("next dev -p 3005"));
        assert_eq!(port, 3005);
    }
}
