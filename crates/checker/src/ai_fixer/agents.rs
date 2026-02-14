//! Three-agent AI repair pipeline.
//!
//! Each agent has a focused role with a structured output contract:
//!
//! - **PlannerAgent**: analyses diagnostics and produces a repair plan (JSON).
//! - **FixAgent**: generates a unified diff patch for the planned changes.
//! - **VerifyAgent**: validates the proposed patch before application.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{CheckResult, CheckerError};
use crate::output::CheckIssue;

use super::context::RepairContext;
use super::llm::{
    call_llm, call_llm_streaming, extract_diff_block, extract_json_block, ChatMessage, LlmConfig,
    TokenUsage,
};

use std::path::Path;

// =========================================================================
// Planner Agent
// =========================================================================

/// Output from the Planner Agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairPlan {
    /// Root cause analysis of the errors.
    pub root_cause: String,
    /// Files that need to be edited.
    pub files_to_edit: Vec<FileEditPlan>,
    /// High-level strategy description.
    pub strategy: String,
}

/// A single file that the planner identified as needing edits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditPlan {
    /// File path.
    pub path: String,
    /// Why this file needs to change.
    pub reason: String,
}

/// Run the Planner Agent.
///
/// Analyses diagnostics and relevant context, returns a structured repair plan
/// and token usage (if available).
pub async fn run_planner(
    config: &LlmConfig,
    issues: &[CheckIssue],
    context: &RepairContext,
    verbose: bool,
    project_dir: Option<&Path>,
) -> CheckResult<(RepairPlan, Option<TokenUsage>)> {
    let diagnostics = format_diagnostics(issues);
    let file_tree = &context.file_tree;

    // Build context snippets for error files.
    let mut file_snippets = String::new();
    for (path, file_ctx) in &context.error_files {
        file_snippets.push_str(&format!("FILE: {}\n", path.display()));
        file_snippets.push_str(&file_ctx.content);
        file_snippets.push_str("\n\n");
    }

    let system_prompt = planner_prompt(project_dir);
    let user_prompt = format!(
        "{FILE_TREE}\n{file_tree}\n\n\
         {ERRORS}\n{diagnostics}\n\n\
         {FILES}\n{file_snippets}\n\n\
         Analyse the errors and produce a repair plan as JSON.",
        FILE_TREE = "## Project File Tree",
        ERRORS = "## Errors",
        FILES = "## Relevant Source Files",
    );

    if verbose {
        let _ = ui::status::info("[Planner] Analysing errors and building repair plan...");
    }

    let messages = vec![
        ChatMessage::system(&system_prompt),
        ChatMessage::user(user_prompt),
    ];

    let response = call_llm(config, &messages).await?;
    let usage = response.usage.clone();

    if verbose {
        let _ = ui::status::info(&format!(
            "[Planner] Response length: {} chars",
            response.content.len()
        ));
    }

    // Parse the JSON output.
    let json_str = extract_json_block(&response.content).ok_or_else(|| CheckerError::AiFixFailed {
        reason: "Planner agent did not return valid JSON".to_string(),
    })?;

    let plan: RepairPlan =
        serde_json::from_str(&json_str).map_err(|e| CheckerError::AiFixFailed {
            reason: format!("Failed to parse planner output: {}", e),
        })?;

    if verbose {
        let _ = ui::status::info(&format!("[Planner] Root cause: {}", plan.root_cause));
        let _ = ui::status::info(&format!("[Planner] Strategy: {}", plan.strategy));
        for f in &plan.files_to_edit {
            let _ = ui::status::info(&format!("[Planner]   Edit: {} — {}", f.path, f.reason));
        }
    }

    Ok((plan, usage))
}

// =========================================================================
// Fix Agent
// =========================================================================

/// Run the Fix Agent.
///
/// Given the repair plan and full file context, generates a unified diff.
/// Returns the raw diff string and token usage. Uses streaming for real-time
/// progress display.
pub async fn run_fixer(
    config: &LlmConfig,
    plan: &RepairPlan,
    issues: &[CheckIssue],
    context: &RepairContext,
    reflection: Option<&str>,
    verbose: bool,
    project_dir: Option<&Path>,
) -> CheckResult<(String, Option<TokenUsage>)> {
    let diagnostics = format_diagnostics(issues);

    // Build the plan section.
    let plan_json = serde_json::to_string_pretty(plan).unwrap_or_default();

    // Build file contents for files the planner said to edit.
    let mut file_contents = String::new();
    let files_to_include: Vec<PathBuf> = plan
        .files_to_edit
        .iter()
        .map(|f| PathBuf::from(&f.path))
        .collect();

    for path in &files_to_include {
        if let Some(file_ctx) = context.error_files.get(path) {
            file_contents.push_str(&format!("FILE: {}\n", path.display()));
            file_contents.push_str(&file_ctx.content);
            file_contents.push_str("\n\n");
        } else if let Some(content) = context.related_files.get(path) {
            file_contents.push_str(&format!("FILE: {}\n", path.display()));
            file_contents.push_str(content);
            file_contents.push_str("\n\n");
        }
    }

    // Include related files for type context.
    for (path, content) in &context.related_files {
        if !files_to_include.contains(path) {
            file_contents.push_str(&format!("RELATED FILE (read-only): {}\n", path.display()));
            file_contents.push_str(content);
            file_contents.push_str("\n\n");
        }
    }

    let system_prompt = fixer_prompt(project_dir);

    let mut user_prompt = format!(
        "{ERRORS}\n{diagnostics}\n\n\
         {PLAN}\n{plan_json}\n\n\
         {FILES}\n{file_contents}\n\n\
         Generate a unified diff that fixes the reported errors.\n\
         Output ONLY the unified diff. No explanations.",
        ERRORS = "## Errors to Fix",
        PLAN = "## Repair Plan",
        FILES = "## Source Files",
    );

    // Add reflection context if this is a retry.
    if let Some(reflection_ctx) = reflection {
        user_prompt = format!(
            "{REFLECTION}\n{reflection_ctx}\n\n{user_prompt}",
            REFLECTION = "## Previous Attempt Feedback",
        );
    }

    if verbose {
        let _ = ui::status::info("[Fixer] Generating unified diff patch...");
    }

    let messages = vec![
        ChatMessage::system(&system_prompt),
        ChatMessage::user(user_prompt),
    ];

    // Use streaming for the fix agent (longest response) to show progress.
    let char_count = std::sync::atomic::AtomicUsize::new(0);
    let response = call_llm_streaming(config, &messages, |delta| {
        let count = char_count.fetch_add(delta.len(), std::sync::atomic::Ordering::Relaxed)
            + delta.len();
        if count % 200 < 20 {
            eprint!("\r[Fixer] Generating... {} chars", count);
        }
    })
    .await?;
    let final_count = char_count.load(std::sync::atomic::Ordering::Relaxed);
    if final_count > 0 {
        eprintln!("\r[Fixer] Generation complete: {} chars", final_count);
    }

    let usage = response.usage.clone();

    if verbose {
        let _ = ui::status::info(&format!(
            "[Fixer] Response length: {} chars",
            response.content.len()
        ));
    }

    // Extract the diff from the response.
    let diff = extract_diff_block(&response.content).ok_or_else(|| CheckerError::AiFixFailed {
        reason: "Fix agent did not return a valid unified diff".to_string(),
    })?;

    Ok((diff, usage))
}

// =========================================================================
// Verify Agent
// =========================================================================

/// Output from the Verify Agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    /// Whether the patch is valid.
    pub valid: bool,
    /// Explanation of why the patch is valid or invalid.
    pub reason: String,
    /// Risk score (0.0 = no risk, 1.0 = very risky).
    pub risk_score: f32,
}

/// Run the Verify Agent.
///
/// Checks the proposed diff against the original errors and file content.
/// Returns the verification result and token usage.
pub async fn run_verifier(
    config: &LlmConfig,
    diff: &str,
    issues: &[CheckIssue],
    context: &RepairContext,
    verbose: bool,
    project_dir: Option<&Path>,
) -> CheckResult<(VerifyResult, Option<TokenUsage>)> {
    let diagnostics = format_diagnostics(issues);

    // Include original file contents for the files being patched.
    let mut file_contents = String::new();
    for (path, file_ctx) in &context.error_files {
        file_contents.push_str(&format!("ORIGINAL FILE: {}\n", path.display()));
        file_contents.push_str(&file_ctx.content);
        file_contents.push_str("\n\n");
    }

    let system_prompt = verifier_prompt(project_dir);
    let user_prompt = format!(
        "{ERRORS}\n{diagnostics}\n\n\
         {FILES}\n{file_contents}\n\n\
         {PATCH}\n```diff\n{diff}\n```\n\n\
         Verify whether this patch correctly fixes the reported errors.\n\
         Return your assessment as JSON.",
        ERRORS = "## Original Errors",
        FILES = "## Original Files",
        PATCH = "## Proposed Patch",
    );

    if verbose {
        let _ = ui::status::info("[Verifier] Validating proposed patch...");
    }

    let messages = vec![
        ChatMessage::system(&system_prompt),
        ChatMessage::user(user_prompt),
    ];

    let response = call_llm(config, &messages).await?;
    let usage = response.usage.clone();

    if verbose {
        let _ = ui::status::info(&format!(
            "[Verifier] Response length: {} chars",
            response.content.len()
        ));
    }

    // Parse the JSON output.
    let json_str =
        extract_json_block(&response.content).ok_or_else(|| CheckerError::AiFixFailed {
            reason: "Verify agent did not return valid JSON".to_string(),
        })?;

    let result: VerifyResult =
        serde_json::from_str(&json_str).map_err(|e| CheckerError::AiFixFailed {
            reason: format!("Failed to parse verifier output: {}", e),
        })?;

    if verbose {
        let _ = ui::status::info(&format!(
            "[Verifier] Valid: {}, Risk: {:.2}, Reason: {}",
            result.valid, result.risk_score, result.reason
        ));
    }

    Ok((result, usage))
}

// =========================================================================
// Helpers
// =========================================================================

/// Format check issues into a structured diagnostics string.
fn format_diagnostics(issues: &[CheckIssue]) -> String {
    let mut output = String::new();
    for issue in issues {
        let location = match (issue.line, issue.column) {
            (Some(l), Some(c)) => format!("{}:{}:{}", issue.file.display(), l, c),
            (Some(l), None) => format!("{}:{}", issue.file.display(), l),
            _ => issue.file.display().to_string(),
        };
        let code = issue
            .code
            .as_deref()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();
        output.push_str(&format!(
            "{} {}: {}{}\n",
            issue.severity, location, issue.message, code
        ));
    }
    output
}

// =========================================================================
// Custom prompt loading
// =========================================================================

/// Load a custom prompt from the project or user prompts directory.
///
/// Lookup order:
/// 1. `{project_dir}/.appz/prompts/{name}.md`
/// 2. `~/.appz/prompts/{name}.md`
/// 3. Falls back to the hardcoded default.
///
/// If a custom prompt is found, its content replaces the system prompt.
/// The file should contain plain text (markdown is fine).
fn load_custom_prompt(project_dir: Option<&Path>, name: &str, default: &str) -> String {
    let filename = format!("prompts/{}.md", name);

    // Use the layered file reader from common::user_config.
    if let Some(dir) = project_dir {
        if let Some(content) = common::user_config::read_layered_file(dir, &filename) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    } else {
        // No project dir: try user-level only.
        if let Some(user_file) = common::user_config::user_appz_file(&filename) {
            if user_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&user_file) {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return trimmed.to_string();
                    }
                }
            }
        }
    }

    default.to_string()
}

/// Resolve the planner system prompt (custom or default).
pub fn planner_prompt(project_dir: Option<&Path>) -> String {
    load_custom_prompt(project_dir, "planner", DEFAULT_PLANNER_SYSTEM_PROMPT)
}

/// Resolve the fixer system prompt (custom or default).
pub fn fixer_prompt(project_dir: Option<&Path>) -> String {
    load_custom_prompt(project_dir, "fixer", DEFAULT_FIXER_SYSTEM_PROMPT)
}

/// Resolve the verifier system prompt (custom or default).
pub fn verifier_prompt(project_dir: Option<&Path>) -> String {
    load_custom_prompt(project_dir, "verifier", DEFAULT_VERIFIER_SYSTEM_PROMPT)
}

// =========================================================================
// Prompt Templates (defaults)
// =========================================================================

const DEFAULT_PLANNER_SYSTEM_PROMPT: &str = "\
You are a senior software engineer analysing compiler, linter, and test errors.

You are given:
1. A project file tree
2. Relevant source files (or regions of large files)
3. Structured diagnostics (compiler/linter errors with file, line, message)

Your job:
- Identify the root cause of the errors
- Determine which files must be edited to fix them
- Describe a minimal repair strategy
- Do NOT suggest unnecessary refactors
- Do NOT suggest changes to files not related to the errors
- Preserve existing behaviour

Output ONLY valid JSON in this exact schema:
{
  \"root_cause\": \"<concise root cause analysis>\",
  \"files_to_edit\": [
    {
      \"path\": \"<relative file path>\",
      \"reason\": \"<why this file needs to change>\"
    }
  ],
  \"strategy\": \"<high-level strategy description>\"
}

Do not wrap the JSON in a code block. Output raw JSON only.";

const DEFAULT_FIXER_SYSTEM_PROMPT: &str = "\
You are an automated code repair system. You generate unified diff patches.

Rules:
- ONLY modify the files specified in the repair plan
- Produce a standard unified diff (with --- and +++ headers)
- Make MINIMAL necessary changes to fix the reported errors
- Do NOT rewrite entire files
- Do NOT refactor code beyond what is needed for the fix
- Keep formatting consistent with the original code
- Ensure the patched code will compile / pass the linter
- Preserve all existing behaviour

Output format:
- Standard unified diff with proper file headers
- Use a/ and b/ prefixes for file paths
- Include sufficient context lines (3 lines before and after changes)

Output ONLY the unified diff. No explanations, no markdown, no commentary.";

const DEFAULT_VERIFIER_SYSTEM_PROMPT: &str = "\
You are a code review system verifying a proposed patch.

You are given:
1. The original errors that the patch is supposed to fix
2. The original source files
3. The proposed unified diff patch

Your job:
- Determine if the patch actually fixes the reported errors
- Check for new type errors, logic bugs, or broken behaviour
- Check for unnecessary changes or refactors
- Assess the risk level

Output ONLY valid JSON in this exact schema:
{
  \"valid\": true or false,
  \"reason\": \"<explanation of assessment>\",
  \"risk_score\": 0.0 to 1.0
}

Risk score guide:
- 0.0-0.2: Trivial fix, very safe
- 0.2-0.4: Simple fix, low risk
- 0.4-0.6: Moderate complexity
- 0.6-0.8: Complex change, needs review
- 0.8-1.0: Risky, likely introduces issues

Do not wrap the JSON in a code block. Output raw JSON only.";
