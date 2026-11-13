use clap::Args;
use serde::Serialize;
use std::path::PathBuf;

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
            let port_str = stripped.split(':').last().unwrap_or(stripped);
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

#[cfg(test)]
mod tests {
    use super::*;

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
