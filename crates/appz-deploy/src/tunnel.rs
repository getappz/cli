use crate::error::{DeployError, DeployResult};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Supported tunnel provider kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TunnelProviderKind {
    #[default]
    Auto,
    Cloudflare,
    Ssh,
}

impl std::str::FromStr for TunnelProviderKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "cloudflare" | "cf" => Ok(Self::Cloudflare),
            "ssh" | "localhost.run" => Ok(Self::Ssh),
            _ => Err(format!(
                "unknown tunnel provider: `{s}` (expected auto, cloudflare, or ssh)"
            )),
        }
    }
}

/// Configuration options for creating a tunnel
#[derive(Debug, Clone)]
pub struct TunnelConfig {
    pub local_port: u16,
    pub subdomain: Option<String>,
    pub provider: TunnelProviderKind,
    pub startup_timeout: Duration,
}

impl TunnelConfig {
    pub fn new(local_port: u16) -> Self {
        Self {
            local_port,
            subdomain: None,
            provider: TunnelProviderKind::Auto,
            startup_timeout: Duration::from_secs(15),
        }
    }
}

/// Represents an active public tunnel session
pub struct TunnelSession {
    pub public_url: String,
    pub provider_name: &'static str,
    process: Option<Child>,
}

impl TunnelSession {
    pub fn new(public_url: String, provider_name: &'static str, process: Option<Child>) -> Self {
        Self {
            public_url,
            provider_name,
            process,
        }
    }

    /// Check if the underlying tunnel child process is still running
    pub fn is_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.process {
            match child.try_wait() {
                Ok(None) => true,
                _ => false,
            }
        } else {
            false
        }
    }

    /// Explicitly close and terminate the tunnel process
    pub fn close(&mut self) -> DeployResult<()> {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for TunnelSession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Extract a public HTTPS/HTTP URL ending with `suffix` from an output line
pub fn extract_url_from_line(line: &str, suffix: &str) -> Option<String> {
    if let Some(end_rel) = line.find(suffix) {
        let end_idx = end_rel + suffix.len();
        let candidate_slice = &line[..end_idx];
        if let Some(start_idx) = candidate_slice.rfind("https://") {
            let url = &line[start_idx..end_idx];
            // Ensure no leading or trailing punctuation/space
            let clean = url.trim_matches(|c: char| {
                c.is_whitespace() || c == '|' || c == '<' || c == '>' || c == '(' || c == ')'
            });
            return Some(clean.to_string());
        }
        if let Some(start_idx) = candidate_slice.rfind("http://") {
            let url = &line[start_idx..end_idx];
            let clean = url.trim_matches(|c: char| {
                c.is_whitespace() || c == '|' || c == '<' || c == '>' || c == '(' || c == ')'
            });
            return Some(clean.to_string());
        }
    }
    None
}

/// Abstract interface for a tunnel provider
pub trait TunnelProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn start_tunnel(&self, config: &TunnelConfig) -> Result<TunnelSession, DeployError>;
}

/// Cloudflare Quick Tunnel provider (`cloudflared tunnel --url http://127.0.0.1:<port>`)
pub struct CloudflareTunnelProvider;

impl TunnelProvider for CloudflareTunnelProvider {
    fn name(&self) -> &'static str {
        "Cloudflare Quick Tunnel"
    }

    fn is_available(&self) -> bool {
        command::shell::find_command_on_path("cloudflared").is_some()
    }

    fn start_tunnel(&self, config: &TunnelConfig) -> Result<TunnelSession, DeployError> {
        let mut cmd = Command::new("cloudflared");
        cmd.arg("tunnel")
            .arg("--url")
            .arg(format!("http://127.0.0.1:{}", config.local_port))
            .arg("--no-autoupdate")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            DeployError::Tunnel(format!("failed to start cloudflared process: {e}"))
        })?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| DeployError::Tunnel("cannot capture cloudflared stderr".to_string()))?;

        let reader = BufReader::new(stderr);
        let start = Instant::now();
        let mut public_url = None;

        for line_res in reader.lines() {
            if start.elapsed() > config.startup_timeout {
                break;
            }
            if let Ok(line) = line_res {
                if let Some(url) = extract_url_from_line(&line, ".trycloudflare.com") {
                    public_url = Some(url);
                    break;
                }
            }
        }

        if let Some(url) = public_url {
            Ok(TunnelSession::new(url, self.name(), Some(child)))
        } else {
            let _ = child.kill();
            Err(DeployError::Tunnel(
                "timed out waiting for Cloudflare tunnel URL (trycloudflare.com)".to_string(),
            ))
        }
    }
}

/// SSH remote reverse-tunnel provider (`ssh -R 80:127.0.0.1:<port> ...`)
pub struct SshTunnelProvider;

impl TunnelProvider for SshTunnelProvider {
    fn name(&self) -> &'static str {
        "SSH Reverse Proxy"
    }

    fn is_available(&self) -> bool {
        command::shell::find_command_on_path("ssh").is_some()
    }

    fn start_tunnel(&self, config: &TunnelConfig) -> Result<TunnelSession, DeployError> {
        let mut cmd = Command::new("ssh");
        cmd.arg("-T")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("ServerAliveInterval=30")
            .arg("-R")
            .arg(format!("80:127.0.0.1:{}", config.local_port))
            .arg("nokey@localhost.run")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| DeployError::Tunnel(format!("failed to start ssh tunnel: {e}")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DeployError::Tunnel("cannot capture ssh stdout".to_string()))?;

        let reader = BufReader::new(stdout);
        let start = Instant::now();
        let mut public_url = None;

        for line_res in reader.lines() {
            if start.elapsed() > config.startup_timeout {
                break;
            }
            if let Ok(line) = line_res {
                if let Some(url) = extract_url_from_line(&line, ".lhr.life")
                    .or_else(|| extract_url_from_line(&line, "localhost.run"))
                {
                    public_url = Some(url);
                    break;
                }
            }
        }

        if let Some(url) = public_url {
            Ok(TunnelSession::new(url, self.name(), Some(child)))
        } else {
            let _ = child.kill();
            Err(DeployError::Tunnel(
                "timed out waiting for SSH tunnel endpoint".to_string(),
            ))
        }
    }
}

/// Resolves and launches an appropriate tunnel for the provided configuration
pub fn start_tunnel(config: &TunnelConfig) -> Result<TunnelSession, DeployError> {
    match config.provider {
        TunnelProviderKind::Cloudflare => {
            let cf = CloudflareTunnelProvider;
            if !cf.is_available() {
                return Err(DeployError::Tunnel(
                    "`cloudflared` is not installed or not on PATH. Install it via `mise use -g cloudflared` or download from https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/".to_string(),
                ));
            }
            cf.start_tunnel(config)
        }
        TunnelProviderKind::Ssh => {
            let ssh = SshTunnelProvider;
            if !ssh.is_available() {
                return Err(DeployError::Tunnel(
                    "`ssh` executable not found on system PATH".to_string(),
                ));
            }
            ssh.start_tunnel(config)
        }
        TunnelProviderKind::Auto => {
            let cf = CloudflareTunnelProvider;
            if cf.is_available() {
                return cf.start_tunnel(config);
            }
            let ssh = SshTunnelProvider;
            if ssh.is_available() {
                return ssh.start_tunnel(config);
            }
            Err(DeployError::Tunnel(
                "no tunnel backend available on system: install `cloudflared` (recommended) or ensure `ssh` is in PATH".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_url_from_line_parses_trycloudflare() {
        let line = "2026-11-13T04:20:00Z INF | https://fancy-apple-123.trycloudflare.com |";
        let url = extract_url_from_line(line, ".trycloudflare.com");
        assert_eq!(
            url,
            Some("https://fancy-apple-123.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn extract_url_from_line_parses_localhost_run() {
        let line = "Connect to your app at https://my-subdomain.lhr.life or http://...";
        let url = extract_url_from_line(line, ".lhr.life");
        assert_eq!(url, Some("https://my-subdomain.lhr.life".to_string()));
    }

    #[test]
    fn extract_url_from_line_returns_none_when_absent() {
        let line = "Some unrelated log output without any tunnel url";
        assert_eq!(extract_url_from_line(line, ".trycloudflare.com"), None);
    }

    #[test]
    fn provider_kind_parses_from_string() {
        use std::str::FromStr;
        assert_eq!(
            TunnelProviderKind::from_str("auto"),
            Ok(TunnelProviderKind::Auto)
        );
        assert_eq!(
            TunnelProviderKind::from_str("cloudflare"),
            Ok(TunnelProviderKind::Cloudflare)
        );
        assert_eq!(
            TunnelProviderKind::from_str("cf"),
            Ok(TunnelProviderKind::Cloudflare)
        );
        assert_eq!(
            TunnelProviderKind::from_str("ssh"),
            Ok(TunnelProviderKind::Ssh)
        );
        assert!(TunnelProviderKind::from_str("invalid").is_err());
    }

    #[test]
    fn tunnel_session_reports_dead_when_no_process() {
        let mut session = TunnelSession::new("https://mock.example.com".to_string(), "Mock", None);
        assert!(!session.is_alive());
        assert!(session.close().is_ok());
    }
}
