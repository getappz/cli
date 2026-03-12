//! Interactive prompts that use TUI when in a TTY and tui feature is enabled,
//! otherwise fall back to inquire.

use miette::Result;

/// Single-select from options. Uses TUI picker when TTY + tui feature, else inquire.
///
/// Returns the selected option string, or None if cancelled.
pub fn select_interactive(message: &str, options: &[String]) -> Result<Option<String>> {
    if options.is_empty() {
        return Ok(None);
    }

    #[cfg(feature = "tui")]
    {
        let use_tui = std::env::var("APPZ_TUI").as_deref() == Ok("1")
            || (atty::is(atty::Stream::Stderr) && atty::is(atty::Stream::Stdin));
        if use_tui {
            return match tui::select(message, options) {
                Ok(Some(idx)) => Ok(options.get(idx).map(String::from)),
                Ok(None) => Ok(None),
                Err(e) => Err(miette::miette!("TUI select failed: {}", e)),
            };
        }
    }

    inquire_select(message, options)
}

fn inquire_select(message: &str, options: &[String]) -> Result<Option<String>> {
    use inquire::{InquireError, Select};
    let options_ref: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
    match Select::new(message, options_ref).prompt() {
        Ok(s) => Ok(Some(s.to_string())),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(miette::miette!("Selection failed: {}", e)),
    }
}

/// Confirm Yes/No. Uses TUI when TTY + tui feature, else inquire.
pub fn confirm_interactive(message: &str, default: bool) -> Result<bool> {
    #[cfg(feature = "tui")]
    {
        let use_tui = std::env::var("APPZ_TUI").as_deref() == Ok("1")
            || (atty::is(atty::Stream::Stderr) && atty::is(atty::Stream::Stdin));
        if use_tui {
            return tui::confirm(message, default)
                .map_err(|e| miette::miette!("TUI confirm failed: {}", e));
        }
    }

    inquire_confirm(message, default)
}

fn inquire_confirm(message: &str, default: bool) -> Result<bool> {
    use inquire::Confirm;
    Confirm::new(message)
        .with_default(default)
        .prompt()
        .map_err(|e| miette::miette!("Confirmation failed: {}", e))
}

/// Template selector with search and custom git/npm/path input.
/// Uses TUI when TTY + tui feature (all-in-one), else inquire with follow-up prompts.
///
/// Options are (display, value) pairs. Returns the selected value or custom-typed string.
pub fn select_template_interactive(
    title: &str,
    options: &[(String, String)],
) -> Result<Option<String>> {
    if options.is_empty() {
        return Ok(None);
    }

    #[cfg(feature = "tui")]
    {
        let use_tui = std::env::var("APPZ_TUI").as_deref() == Ok("1")
            || (atty::is(atty::Stream::Stderr) && atty::is(atty::Stream::Stdin));
        if use_tui {
            return tui::select_template(title, options)
                .map_err(|e| miette::miette!("TUI template select failed: {}", e));
        }
    }

    inquire_template_select(title, options)
}

fn inquire_template_select(
    title: &str,
    options: &[(String, String)],
) -> Result<Option<String>> {
    use inquire::{InquireError, Select, Text};
    let mut choices: Vec<String> = options.iter().map(|(d, _)| d.clone()).collect();
    choices.extend([
        "Custom GitHub URL".to_string(),
        "Custom npm package".to_string(),
        "Local path".to_string(),
    ]);
    let choices_ref: Vec<&str> = choices.iter().map(|s| s.as_str()).collect();
    match Select::new(title, choices_ref).prompt() {
        Ok("Custom GitHub URL") => Text::new("Git repository (user/repo or full URL):")
            .prompt()
            .map(Some)
            .or_else(|e| match e {
                InquireError::OperationCanceled | InquireError::OperationInterrupted => Ok(None),
                _ => Err(miette::miette!("Prompt failed: {}", e)),
            }),
        Ok("Custom npm package") => Text::new("npm package name:")
            .prompt()
            .map(|p| Some(format!("npm:{}", p)))
            .or_else(|e| match e {
                InquireError::OperationCanceled | InquireError::OperationInterrupted => Ok(None),
                _ => Err(miette::miette!("Prompt failed: {}", e)),
            }),
        Ok("Local path") => Text::new("Local template path:")
            .prompt()
            .map(Some)
            .or_else(|e| match e {
                InquireError::OperationCanceled | InquireError::OperationInterrupted => Ok(None),
                _ => Err(miette::miette!("Prompt failed: {}", e)),
            }),
        Ok(selected) => {
            if let Some((_, v)) = options.iter().find(|(d, _)| d == selected) {
                return Ok(Some(v.clone()));
            }
            Ok(None)
        }
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(miette::miette!("Selection failed: {}", e)),
    }
}
