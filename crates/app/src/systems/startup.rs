use crate::app::Cli;
use crate::app_error::AppError;
use miette::Result;
use std::env;
use std::path::PathBuf;

/// Get the current working directory for the session
///
/// Uses --cwd if provided, otherwise falls back to current directory.
/// The working directory is resolved to an absolute path and canonicalized.
pub fn get_working_dir(cli: &Cli) -> Result<PathBuf> {
    if let Some(ref cwd) = cli.cwd {
        let path = PathBuf::from(cwd);
        // Resolve to absolute path
        let abs_path = if path.is_absolute() {
            path
        } else {
            // Relative path - resolve from current directory
            let current = env::current_dir().map_err(|_| AppError::MissingWorkingDir)?;
            current.join(path)
        };

        // Canonicalize to resolve any . or .. components and symlinks
        abs_path
            .canonicalize()
            .or({
                // If canonicalize fails (e.g., path doesn't exist yet), return the absolute path as-is
                Ok::<PathBuf, std::io::Error>(abs_path)
            })
            .map_err(|_| AppError::MissingWorkingDir.into())
    } else {
        env::current_dir().map_err(|_| AppError::MissingWorkingDir.into())
    }
}
