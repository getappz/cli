//! Safe ripgrep execution — no shell, explicit args, timeout.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::parse::parse_rg_json_line;
use crate::schema::{RawMatch, SearchRequest};
use crate::validate::validate_request;

const TIMEOUT_SECS: u64 = 5;

/// Execute ripgrep on a file path. Returns raw matches (line-in-file, column, snippet).
/// Caller is responsible for mapping line numbers to source files when searching packed content.
pub fn execute(req: &SearchRequest, path: &Path) -> anyhow::Result<Vec<RawMatch>> {
    validate_request(req)?;

    let mut cmd = Command::new("rg");
    cmd.arg("--json");
    cmd.arg("--no-config");
    cmd.arg("--max-filesize").arg("2M");
    cmd.arg("--max-columns").arg("500");
    cmd.arg("--no-follow");

    if req.is_regex.unwrap_or(false) {
        cmd.arg("-e").arg(&req.query);
    } else {
        cmd.arg("--fixed-strings").arg(&req.query);
    }

    cmd.arg(path);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to run ripgrep: {}", e))?;

    let timeout = Duration::from_secs(TIMEOUT_SECS);
    let status = child.wait_timeout(timeout)?;

    if status.is_none() {
        let _ = child.kill();
        anyhow::bail!("ripgrep timed out after {}s", TIMEOUT_SECS);
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("No stdout from ripgrep"))?;

    let reader = BufReader::new(stdout);
    let mut results = Vec::new();
    let max_results = req.max_results.unwrap_or(20);

    for line in reader.lines() {
        let line = line?;
        if let Some(m) = parse_rg_json_line(&line)? {
            results.push(m);
            if results.len() >= max_results {
                break;
            }
        }
    }

    Ok(results)
}
