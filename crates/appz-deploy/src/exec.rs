use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
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

/// Drain `pipe` to the end into `buf`, optionally echoing each chunk to
/// stderr. The buffer is shared so the caller can take what arrived without
/// waiting for the pipe to close.
fn drain(pipe: Option<impl Read>, echo: bool, buf: Arc<Mutex<Vec<u8>>>) {
    use std::io::Write;
    if let Some(mut pipe) = pipe {
        let mut chunk = [0u8; 4096];
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 {
                break;
            }
            buf.lock().unwrap().extend_from_slice(&chunk[..n]);
            if echo {
                let mut err = std::io::stderr().lock();
                let _ = err.write_all(&chunk[..n]);
                let _ = err.flush();
            }
        }
    }
}

/// How long to keep reading after the CLI itself has exited. Its output pipes
/// close then, unless something it started in the background (a telemetry or
/// dev daemon) inherited them; without a limit we'd wait for that forever.
const OUTPUT_GRACE: Duration = Duration::from_secs(5);

/// Wait until both readers are done or `deadline` passes.
fn readers_done_by(readers: &[&thread::JoinHandle<()>], deadline: Instant) -> bool {
    loop {
        if readers.iter().all(|h| h.is_finished()) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
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
            // Windows env names are case-insensitive: `rustic_repo_hot` must go too.
            let name = k.to_string_lossy();
            let hit = if cfg!(windows) {
                name.to_ascii_uppercase()
                    .starts_with(&scrub_prefix.to_ascii_uppercase())
            } else {
                name.starts_with(scrub_prefix)
            };
            // Never queue a removal for a name we set explicitly: the command
            // env is a HashMap, and on Windows a differently-cased removal
            // could be applied after the set and delete our value.
            let explicit = env.iter().any(|(ek, _)| {
                if cfg!(windows) {
                    ek.eq_ignore_ascii_case(&name)
                } else {
                    *ek == name
                }
            });
            if hit && !explicit {
                cmd.env_remove(k);
            }
        }
    }
    for (k, v) in env.iter() {
        cmd.env(*k, *v);
    }
    let _ = cmd.inherit_path();

    let mut child = cmd.spawn_group().map_err(|e| e.to_string())?;

    let stdout_pipe = child.stdout().take();
    let stderr_pipe = child.stderr().take();

    let stdout_buf = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf = Arc::new(Mutex::new(Vec::new()));
    // fire-and-forget: each reader ends when its pipe closes; we join it when
    // it has, and leave it behind if a lingering background process keeps
    // the pipe open past OUTPUT_GRACE.
    let stdout_handle = {
        let buf = Arc::clone(&stdout_buf);
        thread::spawn(move || drain(stdout_pipe, echo, buf))
    };
    let stderr_handle = {
        let buf = Arc::clone(&stderr_buf);
        thread::spawn(move || drain(stderr_pipe, echo, buf))
    };

    let start = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None => {
                if start.elapsed() >= timeout {
                    // Kills the whole tree (process group / job object), not
                    // just the top-level shim, so grandchildren die too.
                    let _ = child.kill();
                    return Err(format!("'{bin}' timed out after {}s", timeout.as_secs()));
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let readers = [&stdout_handle, &stderr_handle];
    if !readers_done_by(&readers, Instant::now() + OUTPUT_GRACE) {
        // The CLI is done; end what it left running in its process group /
        // job so the pipes close, then take whatever output arrived.
        #[cfg(windows)]
        let _ = child.start_kill();
        #[cfg(not(windows))]
        let _ = child.kill();
        readers_done_by(&readers, Instant::now() + Duration::from_secs(1));
    }
    let stdout = std::mem::take(&mut *stdout_buf.lock().unwrap());
    let stderr = std::mem::take(&mut *stderr_buf.lock().unwrap());

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

    /// The test binary doubles as the tree-kill spawner/grandchild: re-running
    /// it with the `tree_kill_helper` filter and this env var makes it play one
    /// of those roles instead of running this test normally.
    const TREE_KILL_ROLE: &str = "APPZ_TEST_TREE_KILL_ROLE";
    const TREE_KILL_PIDFILE: &str = "APPZ_TEST_TREE_KILL_PIDFILE";

    #[test]
    fn tree_kill_helper() {
        match std::env::var(TREE_KILL_ROLE).as_deref() {
            Ok("spawner") => tree_kill_spawner(),
            Ok("grandchild") => tree_kill_grandchild(),
            Ok("leaver") => leave_a_process_holding_stdout(),
            _ => {}
        }
    }

    /// Parent of the tree: spawn a grandchild (ourselves) that sleeps, record
    /// its PID for the test to inspect, then wait on it so both stay alive
    /// until the timeout fires.
    fn tree_kill_spawner() {
        let pidfile = std::env::var(TREE_KILL_PIDFILE).unwrap();
        let exe = std::env::current_exe().unwrap();
        let mut grandchild = std::process::Command::new(&exe)
            .arg("tree_kill_helper")
            .env(TREE_KILL_ROLE, "grandchild")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::fs::write(&pidfile, grandchild.id().to_string()).unwrap();
        let _ = grandchild.wait();
    }

    /// Print, start a grandchild that inherits our stdout/stderr, and exit
    /// right away, like a CLI that leaves a background daemon behind.
    #[allow(clippy::zombie_processes)]
    fn leave_a_process_holding_stdout() {
        let exe = std::env::current_exe().unwrap();
        std::process::Command::new(&exe)
            .arg("tree_kill_helper")
            .env(TREE_KILL_ROLE, "grandchild")
            .spawn()
            .unwrap();
        println!("deployed-ok");
    }

    fn tree_kill_grandchild() {
        // Sleep long enough to outlive the test's timeout; the timeout's tree
        // kill is what must end us, and a cap bounds the damage of a regression.
        for _ in 0..120 {
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    fn process_alive(pid: u32) -> bool {
        #[cfg(windows)]
        {
            let out = std::process::Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH"])
                .output();
            out.map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
                .unwrap_or(true)
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .status()
                .map(|s| s.success())
                .unwrap_or(true)
        }
    }

    #[test]
    fn timeout_kills_the_whole_process_tree() {
        let dir = std::env::temp_dir().join(format!("appz-tree-kill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pidfile = dir.join("grandchild.pid");

        let exe = std::env::current_exe().unwrap();
        let bin = exe.to_str().unwrap();
        let result = exec_captured(
            bin,
            &["tree_kill_helper"],
            &dir,
            &[
                (TREE_KILL_ROLE, "spawner"),
                (TREE_KILL_PIDFILE, pidfile.to_str().unwrap()),
            ],
            Duration::from_secs(3),
        );
        assert!(
            result
                .expect_err("expected a timeout")
                .contains("timed out")
        );

        // The spawner writes the grandchild's PID before it starts waiting, so
        // it is on disk well before the 3s timeout; give a slow CI a little
        // slack, then the grandchild must be gone — killing only the top-level
        // spawner would leave it alive.
        let mut grandchild_pid = None;
        for _ in 0..100 {
            if let Ok(s) = std::fs::read_to_string(&pidfile) {
                grandchild_pid = s.trim().parse::<u32>().ok();
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let grandchild_pid = grandchild_pid.expect("spawner did not record the grandchild PID");

        let mut alive = true;
        for _ in 0..100 {
            alive = process_alive(grandchild_pid);
            if !alive {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            !alive,
            "grandchild {grandchild_pid} survived the timeout kill"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn returns_soon_after_the_cli_exits_even_if_a_leftover_process_holds_its_output() {
        let exe = std::env::current_exe().unwrap();
        let start = Instant::now();
        let out = exec_captured(
            exe.to_str().unwrap(),
            &["tree_kill_helper", "--nocapture"],
            &std::env::temp_dir(),
            &[(TREE_KILL_ROLE, "leaver")],
            Duration::from_secs(60),
        )
        .expect("the CLI itself exited, so this is not a timeout");
        assert!(out.success, "{out:?}");
        assert!(out.stdout.contains("deployed-ok"), "{out:?}");
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "waited {:?} for a leftover process",
            start.elapsed()
        );
    }
}
