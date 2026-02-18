//! .gitignore management for skills installation.

use std::path::{Path, PathBuf};

/// Find .gitignore by traversing up from cwd.
fn find_gitignore(cwd: &Path) -> Option<PathBuf> {
    let mut dir = cwd.canonicalize().ok()?;

    loop {
        let candidate = dir.join(".gitignore");
        if candidate.exists() {
            return Some(candidate);
        }
        let parent = dir.parent()?;
        if parent == dir {
            break;
        }
        dir = parent.to_path_buf();
    }

    let root = dir;
    let candidate = root.join(".gitignore");
    if candidate.exists() {
        return Some(candidate);
    }
    None
}

/// Add entries to .gitignore if not present. Creates .gitignore if missing.
pub fn add_gitignore_entries<P: AsRef<Path>>(
    cwd: P,
    entries: &[&str],
    create_if_not_exists: bool,
) -> Result<Option<PathBuf>, miette::Report> {
    let cwd = cwd.as_ref();
    let mut unique: Vec<String> = entries
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    unique.sort();

    if unique.is_empty() {
        return Ok(None);
    }

    let gitignore_path = find_gitignore(cwd);

    let path = match gitignore_path {
        Some(p) => p,
        None => {
            if !create_if_not_exists {
                return Ok(None);
            }
            let path = cwd.join(".gitignore");
            let content = unique.join("\n") + "\n";
            starbase_utils::fs::write_file(&path, content)
                .map_err(|e| miette::miette!("Failed to write .gitignore: {}", e))?;
            return Ok(Some(path));
        }
    };

    let content = starbase_utils::fs::read_file(&path)
        .map_err(|e| miette::miette!("Failed to read .gitignore: {}", e))?;
    let existing: std::collections::HashSet<String> = content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let new_entries: Vec<&String> = unique.iter().filter(|e| !existing.contains(*e)).collect();
    if new_entries.is_empty() {
        return Ok(Some(path));
    }

    let prefix = if content.ends_with('\n') { "" } else { "\n" };
    let to_append = format!(
        "{}{}\n",
        prefix,
        new_entries.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n")
    );
    starbase_utils::fs::append_file(&path, &to_append)
        .map_err(|e| miette::miette!("Failed to append to .gitignore: {}", e))?;

    Ok(Some(path))
}
