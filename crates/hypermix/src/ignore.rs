//! Update .gitignore, .cursorignore, .cursorignoreindex for pack output.

use std::path::Path;

use miette::IntoDiagnostic;

/// Append pattern to file if not already present. Create file if missing.
fn modify_ignore_file(
    file_path: &Path,
    pattern: &str,
    should_exist: bool,
    content_to_add: &str,
) -> miette::Result<()> {
    use std::io::Write;

    if !file_path.exists() {
        if should_exist {
            std::fs::write(file_path, content_to_add.trim()).into_diagnostic()?;
        }
        return Ok(());
    }

    let content = std::fs::read_to_string(file_path).into_diagnostic()?;
    let pattern_exists = content.contains(pattern);

    if (should_exist && !pattern_exists) || (!should_exist && pattern_exists) {
        let mut f = std::fs::OpenOptions::new().append(true).open(file_path).into_diagnostic()?;
        f.write_all(content_to_add.as_bytes()).into_diagnostic()?;
    }

    Ok(())
}

/// Update .gitignore, .cursorignore, .cursorignoreindex for output path.
pub fn update_ignore_files(cwd: &Path, output_path: &Path) -> miette::Result<()> {
    let relative =
        pathdiff::diff_paths(output_path, cwd).unwrap_or_else(|| output_path.to_path_buf());
    let normalized = relative.to_string_lossy().replace('\\', "/");
    let context_pattern = format!("{}/**/*.xml", normalized);

    modify_ignore_file(
        &cwd.join(".gitignore"),
        &context_pattern,
        true,
        &format!("\n# AI context files (appz pack)\n{}\n", context_pattern),
    )?;

    modify_ignore_file(
        &cwd.join(".cursorignoreindex"),
        &context_pattern,
        true,
        &format!("\n# AI context files (appz pack)\n{}\n", context_pattern),
    )?;

    let negated = format!("!{}", context_pattern);
    modify_ignore_file(
        &cwd.join(".cursorignore"),
        &negated,
        true,
        &format!("\n# Include AI context files (appz pack)\n!{}\n", context_pattern),
    )?;

    Ok(())
}
