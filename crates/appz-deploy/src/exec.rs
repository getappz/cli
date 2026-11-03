use std::io::Read;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use command::Command;

#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Run `bin args...` with NO shell involved — the arg vector goes straight to
/// the OS, so there's no `sh -c "..."`/`cmd /C "..."` string to escape and no
/// injection surface from interpolated values (the upstream deployer this was
/// ported from built shell command strings with `format!`).
///
/// Captures stdout/stderr, draining both concurrently on background threads
/// so a chatty child can't deadlock against a full OS pipe buffer while we're
/// only polling for exit. Enforces a hard timeout, killing the child if
/// exceeded — the upstream version had no timeout at all.
pub fn exec_captured(
    bin: &str,
    args: &[&str],
    cwd: &Path,
    env: &[(&str, &str)],
    timeout: Duration,
) -> Result<ExecOutput, String> {
    exec_inner(bin, args, cwd, env, "", timeout, false)
}

/// Like [`exec_captured`], but the child does not inherit any parent
/// environment variable whose name starts with `scrub_prefix` (e.g. ambient
/// `RUSTIC_*` settings); only `env` supplies those.
pub fn exec_captured_scrubbed(
    bin: &str,
    args: &[&str],
    cwd: &Path,
    env: &[(&str, &str)],
    scrub_prefix: &str,
    timeout: Duration,
) -> Result<ExecOutput, String> {
    exec_inner(bin, args, cwd, env, scrub_prefix, timeout, false)
}

/// Like [`exec_captured`], but also copies the child's output to our stderr
/// as it arrives, so a long build or upload shows progress instead of silence.
/// The full output is still captured (URL parsing, error reports).
pub fn exec_live(
    bin: &str,
    args: &[&str],
    cwd: &Path,
    env: &[(&str, &str)],
    timeout: Duration,
) -> Result<ExecOutput, String> {
    exec_inner(bin, args, cwd, env, "", timeout, true)
}

/// Drain `pipe` to the end, optionally echoing each chunk to stderr.
fn drain(pipe: Option<impl Read>, echo: bool) -> Vec<u8> {
    use std::io::Write;
    let mut buf = Vec::new();
    if let Some(mut pipe) = pipe {
        let mut chunk = [0u8; 4096];
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            if echo {
                let mut err = std::io::stderr().lock();
                let _ = err.write_all(&chunk[..n]);
                let _ = err.flush();
            }
        }
    }
    buf
}

fn exec_inner(
    bin: &str,
    args: &[&str],
    cwd: &Path,
    env: &[(&str, &str)],
    scrub_prefix: &str,
    timeout: Duration,
    echo: bool,
) -> Result<ExecOutput, String> {
    let mut cmd = Command::new(bin);
    cmd.without_shell();
    cmd.args(args.iter().copied());
    cmd.cwd(cwd);
    if !scrub_prefix.is_empty() {
        for (k, _) in std::env::vars_os() {
            if k.to_string_lossy().starts_with(scrub_prefix) {
                cmd.env_remove(k);
            }
        }
    }
    for (k, v) in env.iter() {
        cmd.env(*k, *v);
    }
    let _ = cmd.inherit_path();

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_handle = thread::spawn(move || drain(stdout_pipe, echo));
    let stderr_handle = thread::spawn(move || drain(stderr_pipe, echo));

    let start = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("'{bin}' timed out after {}s", timeout.as_secs()));
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();

    Ok(ExecOutput {
        success: status.success(),
        code: status.code(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    fn echo(msg: &'static str) -> (&'static str, Vec<&'static str>) {
        ("cmd", vec!["/C", "echo", msg])
    }
    #[cfg(not(windows))]
    fn echo(msg: &'static str) -> (&'static str, Vec<&'static str>) {
        ("echo", vec![msg])
    }

    #[cfg(windows)]
    fn sleepy(secs: &'static str) -> (&'static str, Vec<&'static str>) {
        ("cmd", vec!["/C", "ping", "-n", secs, "127.0.0.1"])
    }
    #[cfg(not(windows))]
    fn sleepy(secs: &'static str) -> (&'static str, Vec<&'static str>) {
        ("sleep", vec![secs])
    }

    #[test]
    fn captures_stdout_of_a_successful_command_with_no_shell() {
        let dir = std::env::temp_dir();
        let (bin, args) = echo("hello-appz");
        let out = exec_captured(bin, &args, &dir, &[], Duration::from_secs(10)).unwrap();
        assert!(
            out.success,
            "exit code: {:?}, stderr: {}",
            out.code, out.stderr
        );
        assert!(
            out.stdout.contains("hello-appz"),
            "stdout: {:?}",
            out.stdout
        );
    }

    #[test]
    fn kills_and_errors_on_timeout_instead_of_hanging_forever() {
        let dir = std::env::temp_dir();
        // "ping -n 4" / "sleep 4" run for several seconds; a 200ms timeout
        // must fire well before that — this is the fix for "no timeouts
        // anywhere" (the child would otherwise block us indefinitely).
        let (bin, args) = sleepy("4");
        let result = exec_captured(bin, &args, &dir, &[], Duration::from_millis(200));
        let err = result.expect_err("expected a timeout error");
        assert!(err.contains("timed out"), "error: {err}");
    }
}
