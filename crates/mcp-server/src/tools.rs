//! MCP tool definitions and handlers for appz commands.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::RoleServer;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars;
use rmcp::tool;
use rmcp::tool_handler;
use rmcp::tool_router;
use rmcp::transport::stdio;
use rmcp::ErrorData as McpError;
use sandbox::{create_sandbox, SandboxConfig, SandboxSettings};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

use crate::auth;

const DEFAULT_SHELL_TIMEOUT_MS: u64 = 60_000;

/// Tool names that require authentication (from app::auth::requires_auth).
const AUTH_REQUIRED_TOOLS: &[&str] = &[
    "ls", "run", "plan", "switch", "teams", "projects", "aliases", "domains",
    "promote", "rollback", "remove", "gen",
];

fn resolve_workdir(workdir: Option<&str>) -> Result<PathBuf, McpError> {
    let path = match workdir {
        Some(d) => PathBuf::from(d),
        None => std::env::current_dir()
            .map_err(|e| McpError::invalid_params(format!("No workdir and current_dir failed: {}", e), None))?,
    };
    path.canonicalize().map_err(|e| {
        McpError::invalid_params(format!("Invalid workdir '{}': {}", path.display(), e), None)
    })
}

fn requires_auth(tool_name: &str) -> bool {
    AUTH_REQUIRED_TOOLS.contains(&tool_name)
}

fn ensure_auth(tool_name: &str) -> Result<(), McpError> {
    if !requires_auth(tool_name) {
        return Ok(());
    }
    if auth::has_auth() {
        return Ok(());
    }
    Err(McpError::invalid_params(
        "Authentication required. Run 'appz login' or set APPZ_API_TOKEN environment variable."
            .to_string(),
        None,
    ))
}

/// Resolve the appz binary path. When running as `appz mcp` subcommand, current_exe is appz.
fn appz_binary() -> Result<PathBuf, McpError> {
    let exe = std::env::current_exe().map_err(|e| {
        McpError::internal_error(
            format!("Failed to get current executable: {}", e),
            None,
        )
    })?;
    let name = exe.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    // When run as "appz mcp", current_exe is the appz binary
    if name == "appz" {
        return Ok(exe);
    }
    // Standalone appz-mcp-server: find appz on PATH
    which::which("appz").map_err(|e| {
        McpError::internal_error(
            format!(
                "Could not find appz binary: {}. Install appz or run via 'appz mcp'",
                e
            ),
            None,
        )
    })
}

async fn run_appz(args: &[String], workdir: Option<&str>) -> Result<CommandOutput, McpError> {
    let bin = appz_binary()?;
    let mut cmd = Command::new(&bin);
    cmd.args(args.iter().map(|s| s.as_str()));
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    if let Some(cwd) = workdir {
        cmd.current_dir(cwd);
    }
    let output = cmd
        .output()
        .await
        .map_err(|e| McpError::internal_error(format!("Failed to run appz: {}", e), None))?;
    Ok(CommandOutput {
        exit_code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CommandOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

// --- Init ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InitParams {
    pub template_or_name: Option<String>,
    pub name: Option<String>,
    pub template: Option<String>,
    #[serde(default)]
    pub skip_install: bool,
    #[serde(default)]
    pub force: bool,
    /// Output directory for the new project
    pub output: Option<String>,
    /// Working directory to run the command from (default: current)
    #[serde(default)]
    pub workdir: Option<String>,
}

// --- Build ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BuildParams {
    #[serde(default)]
    pub workdir: Option<String>,
}

// --- Dev ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DevParams {
    #[serde(default)]
    pub workdir: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub share: bool,
}

// --- Deploy ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeployParams {
    #[serde(default)]
    pub workdir: Option<String>,
    pub provider: Option<String>,
    #[serde(default)]
    pub preview: bool,
    #[serde(default)]
    pub no_build: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub json: bool,
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub yes: bool,
}

// --- Run ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunParams {
    pub task: String,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub changed: bool,
}

// --- Plan ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PlanParams {
    pub task: String,
    #[serde(default)]
    pub workdir: Option<String>,
}

// --- Ls (list deployments) ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LsParams {
    #[serde(default)]
    pub workdir: Option<String>,
}

// --- Skills ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SkillsAddParams {
    pub source: String,
    #[serde(default)]
    pub workdir: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SkillsListParams {
    #[serde(default)]
    pub workdir: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SkillsRemoveParams {
    pub name: String,
    #[serde(default)]
    pub workdir: Option<String>,
}

// --- Code search ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CodeIndexParams {
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CodeSearchParams {
    pub query: String,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

// --- Grep search (packed code) ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GrepSearchParams {
    pub query: String,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub is_regex: Option<bool>,
    #[serde(default)]
    pub file_glob: Option<String>,
    #[serde(default)]
    pub max_results: Option<usize>,
    #[serde(default)]
    pub pack_hash: Option<String>,
}

// --- Shell (sandboxed) ---
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ShellParams {
    /// The shell command to execute (e.g. "npm run build", "ls -la")
    pub command: String,
    /// Working directory (project root). Must be an absolute path.
    pub workdir: String,
    /// Timeout in milliseconds (default: 60000)
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShellResult {
    pub exit_code: i32,
    /// Combined stdout and stderr
    pub output: String,
    pub timed_out: bool,
}

#[derive(Clone)]
pub struct AppzTool {
    tool_router: ToolRouter<AppzTool>,
}

#[tool_router]
impl AppzTool {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool]
    async fn init(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<InitParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut args = vec!["init".to_string()];
        if let Some(ref t) = params.template_or_name {
            args.push(t.clone());
        }
        if let Some(ref n) = params.name {
            args.extend(["--name".to_string(), n.clone()]);
        }
        if let Some(ref t) = params.template {
            args.extend(["--template".to_string(), t.clone()]);
        }
        if params.skip_install {
            args.push("--skip-install".to_string());
        }
        if params.force {
            args.push("--force".to_string());
        }
        if let Some(ref o) = params.output {
            args.extend(["--output".to_string(), o.clone()]);
        }
        let workdir = params.workdir.as_deref().or(params.output.as_deref());
        let out = run_appz(&args, workdir).await?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[tool]
    async fn build(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<BuildParams>,
    ) -> Result<CallToolResult, McpError> {
        let args = vec!["build".to_string()];
        let out = run_appz(&args, params.workdir.as_deref()).await?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[tool]
    async fn dev(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<DevParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut args = vec!["dev".to_string()];
        if let Some(p) = params.port {
            args.extend(["--port".to_string(), p.to_string()]);
        }
        if params.share {
            args.push("--share".to_string());
        }
        let out = run_appz(&args, params.workdir.as_deref()).await?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[tool]
    async fn deploy(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<DeployParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut args = vec!["deploy".to_string()];
        if let Some(ref p) = params.provider {
            args.push(p.clone());
        }
        if params.preview {
            args.push("--preview".to_string());
        }
        if params.no_build {
            args.push("--no-build".to_string());
        }
        if params.dry_run {
            args.push("--dry-run".to_string());
        }
        if params.json {
            args.push("--json".to_string());
        }
        if params.all {
            args.push("--all".to_string());
        }
        if params.yes {
            args.push("--yes".to_string());
        }
        let out = run_appz(&args, params.workdir.as_deref()).await?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[tool]
    async fn run(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<RunParams>,
    ) -> Result<CallToolResult, McpError> {
        ensure_auth("run")?;
        let mut args = vec!["run".to_string(), params.task.clone()];
        if params.force {
            args.push("--force".to_string());
        }
        if params.changed {
            args.push("--changed".to_string());
        }
        let out = run_appz(&args, params.workdir.as_deref()).await?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[tool]
    async fn plan(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<PlanParams>,
    ) -> Result<CallToolResult, McpError> {
        ensure_auth("plan")?;
        let args = vec!["plan".to_string(), params.task.clone()];
        let out = run_appz(&args, params.workdir.as_deref()).await?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[tool]
    async fn ls(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<LsParams>,
    ) -> Result<CallToolResult, McpError> {
        ensure_auth("ls")?;
        let args = vec!["ls".to_string()];
        let out = run_appz(&args, params.workdir.as_deref()).await?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[tool]
    async fn skills_add(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<SkillsAddParams>,
    ) -> Result<CallToolResult, McpError> {
        let args = vec!["skills".to_string(), "add".to_string(), params.source.clone()];
        let out = run_appz(&args, params.workdir.as_deref()).await?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[tool]
    async fn skills_list(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<SkillsListParams>,
    ) -> Result<CallToolResult, McpError> {
        let args = vec!["skills".to_string(), "list".to_string()];
        let out = run_appz(&args, params.workdir.as_deref()).await?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[tool]
    async fn skills_remove(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<SkillsRemoveParams>,
    ) -> Result<CallToolResult, McpError> {
        let args = vec!["skills".to_string(), "remove".to_string(), params.name.clone()];
        let out = run_appz(&args, params.workdir.as_deref()).await?;
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[cfg(feature = "code-search")]
    #[tool]
    async fn code_index(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<CodeIndexParams>,
    ) -> Result<CallToolResult, McpError> {
        let workdir = resolve_workdir(params.workdir.as_deref())?;
        let result = code_search::index(&workdir, params.force, None)
            .await
            .map_err(|e| McpError::internal_error(e.0, None))?;
        let out = serde_json::json!({
            "indexed_files": result.indexed_files,
            "chunks": result.chunks,
            "collection": result.collection,
        });
        Ok(CallToolResult::success(vec![Content::json(out)?]))
    }

    #[cfg(not(feature = "code-search"))]
    #[tool]
    async fn code_index(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(_params): Parameters<CodeIndexParams>,
    ) -> Result<CallToolResult, McpError> {
        Err(McpError::invalid_params(
            "code_index requires the code-search feature. Build appz with --features code-search.".to_string(),
            None,
        ))
    }

    #[cfg(feature = "code-search")]
    #[tool]
    async fn code_search(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<CodeSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let workdir = resolve_workdir(params.workdir.as_deref())?;
        let results = code_search::search(&workdir, &params.query, params.limit)
            .await
            .map_err(|e| McpError::internal_error(e.0, None))?;
        let out: Vec<serde_json::Value> = results
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "path": r.path,
                    "content": r.content,
                    "score": r.score,
                })
            })
            .collect();
        Ok(CallToolResult::success(vec![Content::json(serde_json::json!({
            "results": out
        }))?]))
    }

    #[cfg(not(feature = "code-search"))]
    #[tool]
    async fn code_search(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(_params): Parameters<CodeSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        Err(McpError::invalid_params(
            "code_search requires the code-search feature. Build appz with --features code-search.".to_string(),
            None,
        ))
    }

    /// Search packed code (grep over cached pack from appz code pack). No shell, no flag injection.
    #[tool]
    async fn grep_search(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<GrepSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let workdir = resolve_workdir(params.workdir.as_deref())?;
        let packs = code_mix::get_packs_for_workdir(&workdir)
            .map_err(|e| McpError::internal_error(e.0, None))?;

        if packs.is_empty() {
            return Err(McpError::invalid_params(
                "No packed code found for this project. Run 'appz code pack' first.".to_string(),
                None,
            ));
        }

        let (_, pack_path) = if let Some(ref hash) = params.pack_hash {
            packs
                .into_iter()
                .find(|(e, _)| e.content_hash == *hash)
                .ok_or_else(|| {
                    McpError::invalid_params(
                        format!("Pack with hash '{}' not found for this project", hash),
                        None,
                    )
                })?
        } else {
            packs
                .into_iter()
                .next()
                .expect("packs not empty")
        };

        let req = code_grep::SearchRequest {
            query: params.query,
            is_regex: params.is_regex,
            file_glob: params.file_glob,
            max_results: params.max_results,
        };

        let results = code_mix::search_packed(&req, &pack_path)
            .map_err(|e| McpError::internal_error(e.0, None))?;

        let out: Vec<serde_json::Value> = results
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "file": r.file,
                    "line": r.line,
                    "column": r.column,
                    "snippet": r.snippet,
                })
            })
            .collect();
        Ok(CallToolResult::success(vec![Content::json(serde_json::json!({
            "results": out
        }))?]))
    }

    /// Run a shell command inside the appz sandbox (project-root scoped, mise environment).
    #[tool]
    async fn shell(
        &self,
        _context: RequestContext<RoleServer>,
        Parameters(params): Parameters<ShellParams>,
    ) -> Result<CallToolResult, McpError> {
        let workdir = PathBuf::from(&params.workdir);
        let workdir = workdir
            .canonicalize()
            .map_err(|e| {
                McpError::invalid_params(
                    format!("Invalid workdir '{}': {}", params.workdir, e),
                    None,
                )
            })?;

        let config = SandboxConfig::new(&workdir).with_settings(
            SandboxSettings::default().quiet(),
        );
        let sandbox = create_sandbox(config)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let timeout_duration = Duration::from_millis(
            params.timeout_ms.unwrap_or(DEFAULT_SHELL_TIMEOUT_MS),
        );
        let exec_future = sandbox.exec(&params.command);

        let result = match timeout(timeout_duration, exec_future).await {
            Ok(Ok(out)) => ShellResult {
                exit_code: out.exit_code().unwrap_or(1),
                output: format!("{}\n{}", out.stdout(), out.stderr()).trim().to_string(),
                timed_out: false,
            },
            Ok(Err(e)) => {
                return Err(McpError::internal_error(e.to_string(), None));
            }
            Err(_) => ShellResult {
                exit_code: 124, // conventional timeout exit code
                output: format!(
                    "Command timed out after {} ms",
                    params.timeout_ms.unwrap_or(DEFAULT_SHELL_TIMEOUT_MS)
                )
                .to_string(),
                timed_out: true,
            },
        };

        Ok(CallToolResult::success(vec![Content::json(result)?]))
    }
}

#[tool_handler]
impl ServerHandler for AppzTool {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This server provides tools to run appz CLI commands: init, build, dev, deploy, run, \
                 plan, ls, skills, code_index, code_search, grep_search, and a sandboxed shell. Auth-required tools \
                 (run, plan, ls, etc.) need 'appz login' or APPZ_API_TOKEN.                  code_index indexes the \
                 codebase with Repomix+Qdrant; code_search runs semantic search over it. grep_search \
                 searches packed code (from appz code pack) with ripgrep."
                    .to_string(),
            ),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}

/// Run the MCP server over stdio. Blocks until the client disconnects.
pub async fn run_server() -> Result<(), rmcp::service::ServerInitializeError> {
    let tool = AppzTool::new();
    let service = tool.serve(stdio()).await?;
    // Wait until client disconnects. JoinError from task panic is logged but not fatal.
    if let Err(e) = service.waiting().await {
        eprintln!("MCP server task ended unexpectedly: {}", e);
    }
    Ok(())
}
