use crate::error::CommandError;
use miette::Result;
use std::collections::HashMap;
use std::env;
use std::ffi::{OsStr, OsString};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::{Command as StdCommand, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::sync::{Mutex, Once};

use crate::shell::Shell;

/// Set by the Windows Ctrl+C handler when it fires. Callers that cascade
/// through multiple fallback commands on a non-success exit (e.g. mise's
/// winget -> PowerShell installer) should check this so a cancelled step
/// doesn't get silently retried via a different method.
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

/// Ensures the process-wide Ctrl+C handler is installed only once, even
/// though `exec_interactive` may be called many times in a single run.
///
/// Windows-only: on Unix, the default "Ctrl+C terminates the process"
/// disposition already works correctly (the child shares our process
/// group and gets SIGINT directly). Installing a handler there — even one
/// whose body does nothing — disables that default termination for the
/// rest of the run, since the OS no longer kills a process that has a
/// registered signal handler.
#[cfg(windows)]
static CTRLC_HANDLER_INIT: Once = Once::new();

/// PID of the currently running interactive child, if any. Read by the
/// Ctrl+C handler so it knows what to kill.
#[cfg(windows)]
static CURRENT_CHILD_PID: Mutex<Option<u32>> = Mutex::new(None);

#[cfg(windows)]
fn install_ctrlc_handler() {
    CTRLC_HANDLER_INIT.call_once(|| {
        let _ = ctrlc::set_handler(|| {
            INTERRUPTED.store(true, Ordering::SeqCst);
            if let Some(pid) = *CURRENT_CHILD_PID.lock().unwrap() {
                let _ = StdCommand::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        });
    });
}

#[derive(Debug, Clone)]
pub struct Command {
    pub bin: OsString,
    pub args: Vec<OsString>,
    pub cwd: Option<OsString>,
    pub env: HashMap<OsString, Option<OsString>>,
    pub paths_before: Vec<OsString>,
    pub paths_after: Vec<OsString>,
    pub input: Vec<OsString>,
    pub shell: Option<Shell>,
    pub error_on_nonzero: bool,
    pub print_command: bool,
    pub continuous_pipe: bool,
    pub escape_args: bool,
    pub prefix: Option<String>,
}

impl Command {
    pub fn new<S: AsRef<OsStr>>(bin: S) -> Self {
        Command {
            bin: bin.as_ref().to_os_string(),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            paths_before: vec![],
            paths_after: vec![],
            input: vec![],
            shell: Some(Shell::default()),
            error_on_nonzero: true,
            print_command: false,
            continuous_pipe: false,
            escape_args: true,
            prefix: None,
        }
    }

    pub fn arg<A: AsRef<OsStr>>(&mut self, arg: A) -> &mut Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn arg_if_missing<A: AsRef<OsStr>>(&mut self, arg: A) -> &mut Self {
        let arg = arg.as_ref();
        let present = self.args.iter().any(|a| a == arg);
        if !present {
            self.arg(arg);
        }
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
        self
    }

    pub fn cwd<P: AsRef<OsStr>>(&mut self, dir: P) -> &mut Self {
        let dir = dir.as_ref().to_os_string();
        self.env("PWD", &dir);
        self.cwd = Some(dir);
        self
    }

    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.env.insert(
            key.as_ref().to_os_string(),
            Some(val.as_ref().to_os_string()),
        );
        self
    }

    pub fn env_if_missing<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let key = key.as_ref();
        if !self.env.contains_key(key) {
            self.env(key, val);
        }
        self
    }

    pub fn env_remove<K>(&mut self, key: K) -> &mut Self
    where
        K: AsRef<OsStr>,
    {
        self.env.insert(key.as_ref().to_os_string(), None);
        self
    }

    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (k, v) in vars {
            self.env(k, v);
        }
        self
    }

    pub fn input<I, V>(&mut self, input: I) -> &mut Self
    where
        I: IntoIterator<Item = V>,
        V: AsRef<OsStr>,
    {
        for i in input {
            self.input.push(i.as_ref().to_os_string());
        }
        self
    }

    pub fn prepend_paths<I, V>(&mut self, list: I) -> &mut Self
    where
        I: IntoIterator<Item = V>,
        V: AsRef<OsStr>,
    {
        let mut new_paths: Vec<OsString> = list
            .into_iter()
            .map(|path| path.as_ref().to_os_string())
            .collect();
        new_paths.append(&mut self.paths_before);
        self.paths_before = new_paths;
        self
    }

    pub fn append_paths<I, V>(&mut self, list: I) -> &mut Self
    where
        I: IntoIterator<Item = V>,
        V: AsRef<OsStr>,
    {
        self.paths_after
            .extend(list.into_iter().map(|path| path.as_ref().to_os_string()));
        self
    }

    pub fn inherit_path(&mut self) -> Result<&mut Self> {
        let key = OsString::from("PATH");
        if self.env.contains_key(&key)
            || (self.paths_before.is_empty() && self.paths_after.is_empty())
        {
            return Ok(self);
        }

        let mut paths = vec![];
        paths.extend(self.paths_before.clone());

        if let Some(path_env) = env::var_os(&key) {
            for path in env::split_paths(&path_env) {
                paths.push(path.into_os_string());
            }
        }

        paths.extend(self.paths_after.clone());

        let new_path =
            env::join_paths(paths).map_err(|e| CommandError::JoinPathFailed(e.to_string()))?;
        self.env(&key, new_path);
        Ok(self)
    }

    pub fn inherit_colors(&mut self) -> &mut Self {
        if !is_test_env() {
            let no_color = OsString::from("NO_COLOR");
            let force_color = OsString::from("FORCE_COLOR");

            if !self.env.contains_key(&no_color) && !self.env.contains_key(&force_color) {
                let level = detect_color_support().to_string();
                self.env_remove(no_color);
                self.env(force_color, &level);
                self.env("CLICOLOR_FORCE", &level);
            }
        }

        self.env("COLUMNS", "80");
        self.env("LINES", "24");
        self
    }

    pub fn get_bin_name(&self) -> String {
        self.bin.to_string_lossy().to_string()
    }

    fn print_command_line(&self) {
        if let Some(ref prefix) = self.prefix {
            eprint!("[{}] ", prefix);
        }
        eprint!("> {}", self.bin.to_string_lossy());
        for arg in &self.args {
            eprint!(" {}", arg.to_string_lossy());
        }
        eprintln!();
    }

    pub fn get_prefix(&self) -> Option<&str> {
        self.prefix.as_deref()
    }

    pub fn set_continuous_pipe(&mut self, state: bool) -> &mut Self {
        self.continuous_pipe = state;
        self
    }

    pub fn set_print_command(&mut self, state: bool) -> &mut Self {
        self.print_command = state;
        self
    }

    pub fn set_error_on_nonzero(&mut self, state: bool) -> &mut Self {
        self.error_on_nonzero = state;
        self
    }

    pub fn set_prefix(&mut self, prefix: &str) -> &mut Self {
        self.prefix = Some(prefix.to_owned());
        self
    }

    pub fn should_error_nonzero(&self) -> bool {
        self.error_on_nonzero
    }

    pub fn with_shell(&mut self, shell: Shell) -> &mut Self {
        self.shell = Some(shell);
        self
    }

    pub fn without_shell(&mut self) -> &mut Self {
        self.shell = None;
        self
    }

    fn build_std_command(&mut self) -> Result<StdCommand> {
        self.inherit_path()?;

        let mut cmd = if let Some(shell) = &self.shell {
            let cmd_str = if self.args.is_empty() {
                self.bin.to_string_lossy().to_string()
            } else {
                let mut parts = vec![self.bin.clone()];
                parts.extend(self.args.clone());
                shell.join_args(parts).to_string_lossy().to_string()
            };

            let (bin, args) = shell.shell_type.wrap_command(&cmd_str);
            let mut std_cmd = StdCommand::new(bin);
            std_cmd.args(args);
            std_cmd
        } else {
            let mut std_cmd = StdCommand::new(&self.bin);
            std_cmd.args(&self.args);
            std_cmd
        };

        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }

        cmd.envs(forwarded_env(std::env::vars()));

        for (key, value) in &self.env {
            match value {
                Some(val) => {
                    cmd.env(key, val);
                }
                None => {
                    cmd.env_remove(key);
                }
            }
        }

        Ok(cmd)
    }

    pub fn spawn(&mut self) -> Result<std::process::Child> {
        if self.print_command {
            self.print_command_line();
        }

        let mut cmd = self.build_std_command()?;
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| CommandError::SpawnFailed(e.to_string()))?;

        if !self.input.is_empty() {
            let input_str = self
                .input
                .iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n");
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(input_str.as_bytes());
            }
        }

        Ok(child)
    }

    /// Spawn the command in its own process group (Unix) or Job Object
    /// (Windows) and return the wrapped child. `kill()` on the result kills the
    /// entire tree — the direct child plus any grandchildren it spawned (a
    /// `.cmd` shim -> node -> npm -> astro chain) — where a bare
    /// `std::process::Child::kill` stops at the top-level process and leaves
    /// the rest orphaned. Used by the deploy executor's hard timeout.
    pub fn spawn_group(&mut self) -> Result<Box<dyn process_wrap::std::ChildWrapper>> {
        if self.print_command {
            self.print_command_line();
        }

        let mut cmd = self.build_std_command()?;
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut command = process_wrap::std::CommandWrap::from(cmd);
        #[cfg(unix)]
        command.wrap(process_wrap::std::ProcessGroup::leader());
        #[cfg(windows)]
        command.wrap(process_wrap::std::JobObject);

        command
            .spawn()
            .map_err(|e| CommandError::SpawnFailed(e.to_string()))
            .map_err(Into::into)
    }

    pub fn exec(&mut self) -> Result<Output> {
        if self.print_command {
            self.print_command_line();
        }

        let mut cmd = self.build_std_command()?;

        if !self.input.is_empty() {
            let input_str = self
                .input
                .iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n");
            // Capture output like `cmd.output()` does on the no-input path;
            // otherwise callers get an empty stdout/stderr (e.g. the reason
            // `gh secret set` failed) and the child writes to our terminal.
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = cmd
                .spawn()
                .map_err(|e| CommandError::SpawnFailed(e.to_string()))?;
            // Write on another thread while `wait_with_output` drains the
            // output: writing all input first deadlocks once the child fills
            // its output pipe before it has read everything we send.
            let writer = child.stdin.take().map(|mut stdin| {
                std::thread::spawn(move || {
                    use std::io::Write;
                    stdin.write_all(input_str.as_bytes())
                })
            });
            let output = child
                .wait_with_output()
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            if let Some(writer) = writer {
                match writer.join() {
                    // A child that exits without reading all its input
                    // closes the pipe; its exit status already says how it went.
                    Ok(Err(e)) if e.kind() != std::io::ErrorKind::BrokenPipe => {
                        return Err(CommandError::WriteStdinFailed(e.to_string()).into());
                    }
                    _ => {}
                }
            }
            Ok(output)
        } else {
            cmd.output()
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()).into())
        }
    }

    pub fn exec_interactive(&mut self) -> Result<std::process::ExitStatus> {
        if self.print_command {
            self.print_command_line();
        }

        let mut cmd = self.build_std_command()?;

        // Inherit stdin/stdout/stderr for interactivity
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // On Windows, detach the child from the console's automatic Ctrl+C
        // delivery so cmd.exe/npm.cmd never sees CTRL_C_EVENT directly (which
        // is what causes the "Terminate batch job (Y/N)?" prompt). We handle
        // Ctrl+C ourselves below and forward it to the whole process tree.
        #[cfg(windows)]
        {
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
            install_ctrlc_handler();
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| CommandError::SpawnFailed(e.to_string()))?;

        #[cfg(windows)]
        {
            let mut current = CURRENT_CHILD_PID.lock().unwrap();
            *current = Some(child.id());
        }

        let result = child
            .wait()
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()).into());

        #[cfg(windows)]
        {
            let mut current = CURRENT_CHILD_PID.lock().unwrap();
            *current = None;
        }

        result
    }

    pub fn run(&mut self) -> Result<()> {
        let output = self.exec()?;
        if output.status.success() || !self.error_on_nonzero {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let cmd_str = if self.args.is_empty() {
                self.bin.to_string_lossy().to_string()
            } else {
                format!(
                    "{} {}",
                    self.bin.to_string_lossy(),
                    self.args
                        .iter()
                        .map(|a| a.to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };
            Err(CommandError::CommandFailed {
                command: cmd_str,
                exit_code: output.status.code(),
                stdout: if stdout.trim().is_empty() {
                    "(empty)".to_string()
                } else {
                    stdout.to_string()
                },
                stderr: if stderr.trim().is_empty() {
                    "(empty)".to_string()
                } else {
                    stderr.to_string()
                },
            }
            .into())
        }
    }
}

fn is_test_env() -> bool {
    env::var("TEST").is_ok() || env::var("CI").is_ok()
}

fn detect_color_support() -> u8 {
    if env::var("NO_COLOR").is_ok() || env::var("CI").is_ok() {
        return 0;
    }

    if let Ok(force) = env::var("FORCE_COLOR") {
        if let Ok(level) = force.parse::<u8>() {
            return level;
        }
    }

    #[cfg(windows)]
    {
        use std::io::IsTerminal;
        if std::io::stdout().is_terminal() {
            return 1;
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(term) = env::var("TERM") {
            if term != "dumb" && term.contains("color") {
                return 1;
            }
        }
    }

    0
}

/// Env vars never forwarded to a child process. The vault passphrase must stay
/// inside the appz process: inheriting it would hand it to every build tool,
/// postinstall script and provider CLI appz runs. A caller that really wants a
/// child to see one sets it explicitly with [`Command::env`].
///
/// Keep in step with `appz_vault::PASSPHRASE_ENV` (a test in the `appz` crate
/// checks it).
pub const DENIED_CHILD_ENV: &[&str] = &["APPZ_VAULT_PASSPHRASE"];

/// The parent's environment minus [`DENIED_CHILD_ENV`].
fn forwarded_env(
    vars: impl Iterator<Item = (String, String)>,
) -> impl Iterator<Item = (String, String)> {
    // Case-insensitive: Windows env names are, so `appz_vault_passphrase` is
    // read by appz yet would slip past an exact match.
    vars.filter(|(k, _)| !DENIED_CHILD_ENV.iter().any(|d| d.eq_ignore_ascii_case(k)))
}

#[cfg(test)]
mod denied_env_tests {
    use super::*;

    #[test]
    fn the_vault_passphrase_is_not_inherited_by_children() {
        let vars = vec![
            ("PATH".to_string(), "/bin".to_string()),
            ("APPZ_VAULT_PASSPHRASE".to_string(), "s3cret".to_string()),
            ("Appz_Vault_Passphrase".to_string(), "s3cret".to_string()),
            ("HOME".to_string(), "/h".to_string()),
        ];
        let kept: Vec<String> = forwarded_env(vars.into_iter()).map(|(k, _)| k).collect();
        assert_eq!(kept, ["PATH", "HOME"]);
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;

    /// A child that echoes its input back: its output must be captured, and
    /// input larger than a pipe buffer must not deadlock against it.
    #[test]
    fn input_is_fed_while_the_output_is_captured() {
        let line = "x".repeat(63);
        let lines: Vec<&str> = std::iter::repeat_n(line.as_str(), 8000).collect();
        #[cfg(windows)]
        let mut cmd = Command::new("findstr");
        #[cfg(windows)]
        cmd.args(["x"]);
        #[cfg(not(windows))]
        let mut cmd = Command::new("cat");
        let out = cmd.without_shell().input(lines).exec().unwrap();
        assert!(out.status.success());
        let echoed = String::from_utf8_lossy(&out.stdout);
        assert_eq!(echoed.lines().filter(|l| *l == line).count(), 8000);
    }
}
