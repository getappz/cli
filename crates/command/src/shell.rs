use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ShellType {
    Sh,
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
}

impl ShellType {
    pub fn detect() -> Self {
        #[cfg(target_os = "windows")]
        {
            if let Ok(shell) = env::var("SHELL") {
                if shell.contains("powershell") || shell.contains("pwsh") {
                    return ShellType::PowerShell;
                }
            }
            ShellType::Cmd
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(shell) = env::var("SHELL") {
                if shell.contains("zsh") {
                    return ShellType::Zsh;
                } else if shell.contains("fish") {
                    return ShellType::Fish;
                } else if shell.contains("bash") {
                    return ShellType::Bash;
                }
            }
            ShellType::Sh
        }
    }

    pub fn bin_name(&self) -> &'static str {
        match self {
            ShellType::Sh => "sh",
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::PowerShell => "pwsh",
            ShellType::Cmd => "cmd",
        }
    }

    pub fn wrap_command(&self, cmd: &str) -> (String, Vec<String>) {
        match self {
            ShellType::Cmd => ("cmd".to_string(), vec!["/C".to_string(), cmd.to_string()]),
            ShellType::PowerShell => (
                "pwsh".to_string(),
                vec!["-Command".to_string(), cmd.to_string()],
            ),
            _ => ("sh".to_string(), vec!["-c".to_string(), cmd.to_string()]),
        }
    }
}

pub fn find_command_on_path(name: &str) -> Option<PathBuf> {
    which::which(name).ok()
}

pub fn get_default_shell() -> ShellType {
    ShellType::detect()
}

#[inline]
pub fn is_windows_script<T: AsRef<OsStr>>(bin: T) -> bool {
    bin.as_ref()
        .to_str()
        .map(|bin| bin.to_lowercase())
        .is_some_and(|bin| bin.ends_with(".cmd") || bin.ends_with(".bat") || bin.ends_with(".ps1"))
}

#[derive(Debug, Clone)]
pub struct Shell {
    pub bin: PathBuf,
    pub bin_name: String,
    pub shell_type: ShellType,
}

impl Shell {
    pub fn new(shell_type: ShellType) -> Self {
        let bin_name = shell_type.bin_name().to_string();
        let bin =
            find_command_on_path(&bin_name).unwrap_or_else(|| PathBuf::from(bin_name.clone()));

        Self {
            bin,
            bin_name,
            shell_type,
        }
    }

    pub fn join_args(&self, args: Vec<OsString>) -> OsString {
        // Simple joining with spaces, escaping if needed
        let mut result = String::new();
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                result.push(' ');
            }
            if let Some(s) = arg.to_str() {
                if s.contains(' ') || s.contains('\'') || s.contains('"') {
                    result.push_str(&shell_words::quote(s));
                } else {
                    result.push_str(s);
                }
            } else {
                // Fallback for non-UTF8 strings
                result.push_str(&arg.to_string_lossy());
            }
        }
        OsString::from(result)
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new(get_default_shell())
    }
}
