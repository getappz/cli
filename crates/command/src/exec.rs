use crate::error::CommandError;
use miette::Result;
use std::collections::HashMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::process::{Command as StdCommand, Output, Stdio};

use crate::shell::Shell;

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

    pub fn get_cache_key(&self) -> String {
        let mut hasher = DefaultHasher::new();

        for (key, value) in &self.env {
            if let Some(value) = value {
                key.hash(&mut hasher);
                value.hash(&mut hasher);
            }
        }

        self.bin.hash(&mut hasher);
        for arg in &self.args {
            arg.hash(&mut hasher);
        }

        if let Some(cwd) = &self.cwd {
            cwd.hash(&mut hasher);
        }

        for arg in &self.input {
            arg.hash(&mut hasher);
        }

        format!("{}", hasher.finish())
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

        cmd.envs(std::env::vars());

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
            cmd.stdin(Stdio::piped());
            let mut child = cmd
                .spawn()
                .map_err(|e| CommandError::SpawnFailed(e.to_string()))?;
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin
                    .write_all(input_str.as_bytes())
                    .map_err(|e| CommandError::WriteStdinFailed(e.to_string()))?;
            }
            child
                .wait_with_output()
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()).into())
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

        cmd.status()
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()).into())
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
