//! MCP (Model Context Protocol) server exposing appz-core to AI agents.
//!
//! `appz mcp` runs a stdio JSON-RPC server so an agent can detect a project's
//! toolchains, diagnose it, and run lifecycle commands over MCP instead of
//! shelling out to the CLI. Tools reuse appz-core detection directly.

use std::path::PathBuf;

use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};

use appz_core::{DetectedToolchain, detect_toolchains, run_doctor};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DirArg {
    /// Project directory to operate on. Defaults to the current directory.
    #[serde(default)]
    pub dir: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Lifecycle {
    Install,
    Build,
    Test,
    Lint,
    Format,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RunArg {
    /// Which lifecycle command to run.
    pub command: Lifecycle,
    /// Project directory to run in. Defaults to the current directory.
    #[serde(default)]
    pub dir: Option<String>,
    /// Return parsed diagnostics instead of raw text. Currently supported
    /// only for a Rust project whose build is exactly `cargo build` (no
    /// other toolchain also contributing a build command) — every other
    /// case silently falls back to the normal raw-text response. Default:
    /// false.
    #[serde(default)]
    pub structured: bool,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeployArg {
    /// Provider slug (e.g. "vercel", "netlify"). Uses appz.jsonc's
    /// `deploy.default` if omitted.
    #[serde(default)]
    pub target: Option<String>,
    /// Project directory. Defaults to the current directory.
    #[serde(default)]
    pub dir: Option<String>,
    /// Production deploy (default: preview).
    #[serde(default)]
    pub prod: bool,
    /// Show what would be deployed without deploying.
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Clone)]
pub struct AppzServer {
    // Read by the `#[tool_handler]`-generated dispatch; dead-code analysis
    // can't see through the macro.
    #[allow(dead_code)]
    tool_router: ToolRouter<AppzServer>,
}

#[tool_router]
impl AppzServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    fn resolve(dir: &Option<String>) -> PathBuf {
        let raw = dir.as_deref().unwrap_or(".");
        appz_core::canonicalize(std::path::Path::new(raw)).unwrap_or_else(|_| PathBuf::from(raw))
    }

    fn err(msg: String) -> CallToolResult {
        CallToolResult::error(vec![ContentBlock::text(msg)])
    }

    #[tool(
        description = "Detect the toolchains (languages, frameworks, versions, lifecycle commands) in a project directory. Returns JSON."
    )]
    fn detect(&self, Parameters(arg): Parameters<DirArg>) -> Result<CallToolResult, McpError> {
        let root = Self::resolve(&arg.dir);
        match detect_toolchains(&root) {
            Ok(tc) => {
                let json = serde_json::to_string_pretty(&tc).unwrap_or_default();
                Ok(CallToolResult::success(vec![ContentBlock::text(json)]))
            }
            Err(e) => Ok(Self::err(format!("detection failed: {e}"))),
        }
    }

    #[tool(
        description = "Diagnose a project: toolchains, package manager, monorepo, config files present, and suggestions. Returns JSON."
    )]
    fn doctor(&self, Parameters(arg): Parameters<DirArg>) -> Result<CallToolResult, McpError> {
        let root = Self::resolve(&arg.dir);
        let report = run_doctor(&root);
        let json = serde_json::to_string_pretty(&report).unwrap_or_default();
        Ok(CallToolResult::success(vec![ContentBlock::text(json)]))
    }

    #[tool(
        description = "Run a lifecycle command (install|build|test|lint|format) in a project directory and return its captured stdout/stderr. install/build are run via `mise run` (mise decides whether cached inputs let it skip the work); test/lint/format run directly. Set `structured: true` to get parsed diagnostics instead of raw text for a Rust `cargo build` — currently the only supported case; every other combination returns the normal raw-text response. Call `detect` first if you want to see the resolved commands."
    )]
    fn run(&self, Parameters(arg): Parameters<RunArg>) -> Result<CallToolResult, McpError> {
        let root = Self::resolve(&arg.dir);
        let toolchains = match detect_toolchains(&root) {
            Ok(t) => t,
            Err(e) => return Ok(Self::err(format!("detection failed: {e}"))),
        };

        if arg.structured && matches!(arg.command, Lifecycle::Build) {
            let build_contributors: Vec<&DetectedToolchain> = toolchains
                .iter()
                .filter(|tc| tc.build_command.is_some())
                .collect();
            if let [tc] = build_contributors[..]
                && tc.slug == "rust"
                && tc.build_command.as_deref() == Some("cargo build")
            {
                return Ok(run_structured_cargo_build(&root));
            }
        }

        if let Some(mise_task) = mise_task_name(arg.command) {
            let has_command = toolchains.iter().any(|tc| match arg.command {
                Lifecycle::Build => tc.build_command.is_some(),
                Lifecycle::Install => tc.install_command.is_some(),
                _ => false,
            });
            if !has_command {
                return Ok(Self::err(format!(
                    "no {:?} command detected for this project",
                    arg.command
                )));
            }
            // Writes mise.toml's [tools]/[tasks] (with sources/outputs) so
            // `mise run` has an up-to-date task to skip or execute.
            appz_core::write_state(&root, &toolchains);
            return Ok(exec_one(&root, "mise", &["run", mise_task]));
        }

        let pick = |tc: &DetectedToolchain| match arg.command {
            Lifecycle::Test => tc.test_command.clone(),
            Lifecycle::Lint => tc.lint_command.clone(),
            Lifecycle::Format => tc.format_command.clone(),
            _ => unreachable!("build/install handled above"),
        };

        let cmds: Vec<String> = toolchains.iter().filter_map(pick).collect();
        if cmds.is_empty() {
            return Ok(Self::err(format!(
                "no {:?} command detected for this project",
                arg.command
            )));
        }

        let mut out = String::new();
        let mut ok = true;
        for cmd in &cmds {
            // Naive whitespace split, same as the CLI's run_cmd — no shell
            // quoting. Upgrade to a shell-words parser if a detected command
            // ever contains quoted args.
            let parts: Vec<&str> = cmd.split(' ').collect();
            let (prog, args) = parts.split_first().unwrap();
            let result = exec_one(&root, prog, args);
            let is_err = result.is_error == Some(true);
            out.push_str(&format!("$ {cmd}\n"));
            if let Some(ContentBlock::Text(t)) = result.content.into_iter().next() {
                out.push_str(&t.text);
            }
            if is_err {
                ok = false;
                break;
            }
        }

        let content = vec![ContentBlock::text(out)];
        Ok(if ok {
            CallToolResult::success(content)
        } else {
            CallToolResult::error(content)
        })
    }

    #[tool(
        description = "Deploy a project to a hosting platform (vercel|netlify) by driving that platform's own CLI. Returns JSON with the deployment URL — `null` if it couldn't be confirmed from the CLI's output, never a guessed URL."
    )]
    fn deploy(&self, Parameters(arg): Parameters<DeployArg>) -> Result<CallToolResult, McpError> {
        let root = Self::resolve(&arg.dir);
        let req = crate::deploy::DeployRequest {
            target: arg.target,
            prod: arg.prod,
            env: vec![],
            build_env: vec![],
            dry_run: arg.dry_run,
            json: true,
        };
        match crate::deploy::run(&root, req) {
            Ok(output) => {
                let json = serde_json::to_string_pretty(&output).unwrap_or_default();
                Ok(CallToolResult::success(vec![ContentBlock::text(json)]))
            }
            Err(e) => Ok(Self::err(e)),
        }
    }
}

fn mise_task_name(command: Lifecycle) -> Option<&'static str> {
    match command {
        Lifecycle::Build => Some("build"),
        Lifecycle::Install => Some("install"),
        Lifecycle::Test | Lifecycle::Lint | Lifecycle::Format => None,
    }
}

/// Run one command, capturing output into a `CallToolResult`. Shell-wrapped
/// (cmd /C on Windows) so npm's `.cmd`/`.ps1` shims resolve — see the
/// matching comment on `run_cmd` in main.rs.
fn exec_one(root: &std::path::Path, prog: &str, args: &[&str]) -> CallToolResult {
    let mut cmd = command::Command::new(prog);
    cmd.args(args)
        .cwd(root)
        .prepend_paths(appz_core::find_node_modules_bin_paths(root));
    match cmd.exec() {
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&o.stderr));
            if !o.status.success() {
                text.push_str(&format!("[exit: {}]\n", o.status.code().unwrap_or(-1)));
                CallToolResult::error(vec![ContentBlock::text(text)])
            } else {
                CallToolResult::success(vec![ContentBlock::text(text)])
            }
        }
        Err(e) => CallToolResult::error(vec![ContentBlock::text(format!(
            "failed to run '{prog}': {e}"
        ))]),
    }
}

#[derive(serde::Serialize)]
struct StructuredBuildResult {
    success: bool,
    diagnostics: Vec<appz_core::Diagnostic>,
    raw: String,
}

fn run_structured_cargo_build(root: &std::path::Path) -> CallToolResult {
    let mut cmd = command::Command::new("cargo");
    cmd.args(["build", "--message-format=json"]).cwd(root);
    match cmd.exec() {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            let result = StructuredBuildResult {
                success: o.status.success(),
                diagnostics: appz_core::parse_cargo_diagnostics(&stdout),
                raw: stderr.into_owned(),
            };
            let json = serde_json::to_string_pretty(&result).unwrap_or_default();
            if result.success {
                CallToolResult::success(vec![ContentBlock::text(json)])
            } else {
                CallToolResult::error(vec![ContentBlock::text(json)])
            }
        }
        Err(e) => CallToolResult::error(vec![ContentBlock::text(format!(
            "failed to run 'cargo build': {e}"
        ))]),
    }
}

#[tool_handler]
impl ServerHandler for AppzServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("appz", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "appz drives project toolchains. Tools: `detect` (JSON toolchain \
             report), `doctor` (JSON diagnosis + suggestions), `run` (execute \
             install/build/test/lint/format and return output). Pass `dir` to \
             target a project; it defaults to the current directory."
                    .to_string(),
            )
    }
}

/// Run the stdio MCP server. Blocks until the client disconnects.
pub fn serve() -> Result<(), String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("async runtime: {e}"))?;
    rt.block_on(async {
        let service = AppzServer::new()
            .serve(stdio())
            .await
            .map_err(|e| format!("serve: {e}"))?;
        service
            .waiting()
            .await
            .map_err(|e| format!("waiting: {e}"))?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_broken_rust_project(dir: &std::path::Path) {
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/main.rs"),
            "fn main() {\n    let x: i32 = \"not a number\";\n    println!(\"{}\", x);\n}\n",
        )
        .unwrap();
    }

    fn write_clean_rust_project(dir: &std::path::Path) {
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {\n    println!(\"hi\");\n}\n")
            .unwrap();
    }

    #[test]
    fn test_structured_build_reports_diagnostics_on_broken_project() {
        let tmp = tempfile::tempdir().unwrap();
        write_broken_rust_project(tmp.path());

        let server = AppzServer::new();
        let arg = RunArg {
            command: Lifecycle::Build,
            dir: Some(tmp.path().to_string_lossy().to_string()),
            structured: true,
        };
        let result = server.run(Parameters(arg)).unwrap();

        assert_eq!(result.is_error, Some(true), "broken build must report an error result");
        let ContentBlock::Text(text) = result.content.into_iter().next().unwrap() else {
            panic!("expected text content");
        };
        let parsed: serde_json::Value = serde_json::from_str(&text.text).unwrap();
        assert_eq!(parsed["success"], false);
        let diagnostics = parsed["diagnostics"].as_array().unwrap();
        assert!(!diagnostics.is_empty(), "must report at least one diagnostic");
        assert_eq!(diagnostics[0]["file"], "src/main.rs");
        assert_eq!(diagnostics[0]["severity"], "error");
    }

    #[test]
    fn test_structured_build_reports_no_diagnostics_on_clean_project() {
        let tmp = tempfile::tempdir().unwrap();
        write_clean_rust_project(tmp.path());

        let server = AppzServer::new();
        let arg = RunArg {
            command: Lifecycle::Build,
            dir: Some(tmp.path().to_string_lossy().to_string()),
            structured: true,
        };
        let result = server.run(Parameters(arg)).unwrap();

        assert_ne!(
            result.is_error,
            Some(true),
            "clean build must not be an error result"
        );
        let ContentBlock::Text(text) = result.content.into_iter().next().unwrap() else {
            panic!("expected text content");
        };
        let parsed: serde_json::Value = serde_json::from_str(&text.text).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 0);
    }
}
