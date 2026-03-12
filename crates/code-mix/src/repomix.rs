//! Repomix invocation via sandbox.
//!
//! Builds CLI args from PackOptions and runs `npx repomix@latest`.

use std::path::Path;

use sandbox::SandboxProvider;

use crate::types::PackOptions;

/// Error from Repomix execution.
#[derive(Debug)]
pub struct RepomixError(pub String);

/// Run Repomix with the given options. Optionally pass file paths via stdin
/// (one per line) when `stdin_paths` is `Some`. Otherwise Repomix discovers
/// files from include/ignore patterns. Uses the provided `sandbox` for exec.
///
/// When `output_override` is `Some`, repomix writes to that path and
/// `--stdout`/`--copy` are not passed (caller handles redirect).
pub async fn run_repomix(
    sandbox: &dyn SandboxProvider,
    options: &PackOptions,
    stdin_paths: Option<&[String]>,
    output_override: Option<&Path>,
) -> Result<(), RepomixError> {
    let _workdir = sandbox.project_path();
    let mut args: Vec<String> = vec!["npx repomix@latest".to_string()];

    // Output: use override when caching, else options
    if let Some(out) = output_override {
        args.push(format!("-o {}", out.display()));
    } else if let Some(ref out) = options.output {
        args.push(format!("-o {}", out.display()));
    } else if options.stdout {
        args.push("--stdout".to_string());
    }
    if let Some(ref s) = options.style {
        args.push(format!("--style {}", s));
    }
    if output_override.is_none() && options.copy {
        args.push("--copy".to_string());
    }

    // Processing
    if options.compress {
        args.push("--compress".to_string());
    }
    if options.remove_comments {
        args.push("--remove-comments".to_string());
    }
    if options.remove_empty_lines {
        args.push("--remove-empty-lines".to_string());
    }
    if let Some(ref split) = options.split_output {
        args.push(format!("--split-output {}", split));
    }
    if let Some(ref inst) = options.instruction {
        args.push(format!("--instruction-file-path {}", inst.display()));
    }
    if let Some(ref h) = options.header {
        let escaped = h.replace('"', "\\\"");
        args.push(format!("--header-text \"{}\"", escaped));
    }

    // Include/ignore
    if !options.include.is_empty() {
        let joined = options.include.join(",");
        args.push(format!("--include \"{}\"", joined));
    }
    if !options.ignore.is_empty() {
        let joined = options.ignore.join(",");
        args.push(format!("-i \"{}\"", joined));
    }

    // Stdin: when we have a custom file list, write to temp file and pipe
    let cmd = if let Some(paths) = stdin_paths {
        if paths.is_empty() {
            return Err(RepomixError("No files to pack".into()));
        }
        let content = paths.join("\n");
        sandbox
            .fs()
            .write_string(".appz-pack-stdin.txt", content.as_str())
            .map_err(|e| RepomixError(format!("Failed to write stdin file: {}", e)))?;
        args.push("--stdin".to_string());
        let full_cmd = args.join(" ");
        format!("cat .appz-pack-stdin.txt | {}", full_cmd)
    } else {
        args.push(".".to_string());
        args.join(" ")
    };

    let status = sandbox
        .exec_interactive(&cmd)
        .await
        .map_err(|e| RepomixError(e.to_string()))?;

    if !status.success() {
        return Err(RepomixError(format!(
            "Repomix exited with code {}",
            status.code().unwrap_or(-1)
        )));
    }

    Ok(())
}
