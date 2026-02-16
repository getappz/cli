//! Shared types for WASM plugins
//!
//! This module provides all the types needed to interact with saasctl host functions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Error codes for host functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum HostError {
    Success = 0,
    InvalidInput = 1,
    NotFound = 2,
    PermissionDenied = 3,
    ExecutionFailed = 4,
    Timeout = 5,
    InternalError = 99,
}

impl From<HostError> for i32 {
    fn from(code: HostError) -> Self {
        code as i32
    }
}

// ============================================================================
// Registry Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInput {
    pub name: String,
    pub desc: Option<String>,
    pub body: Option<String>, // WASM callback ID or null for array deps
    pub deps: Option<Vec<String>>,
    pub only_if: Option<Vec<String>>, // Condition expressions as strings
    pub unless: Option<Vec<String>>,
    pub once: Option<bool>,
    pub hidden: Option<bool>,
    pub timeout: Option<u64>, // Timeout in seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
    pub success: bool,
    pub task_name: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescInput {
    pub task: String,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookInput {
    pub target: String,
    pub hook: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResponse {
    pub success: bool,
    pub message: Option<String>,
}

// ============================================================================
// Context Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSetInput {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextGetInput {
    pub key: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextGetOutput {
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAddInput {
    pub key: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextParseInput {
    pub template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextParseOutput {
    pub result: String,
}

// ============================================================================
// Host Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInput {
    pub hostnames: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub alias: String,
    pub hostname: String,
    pub local: bool,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostResponse {
    pub success: bool,
    pub hosts: Vec<HostInfo>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSelectInput {
    pub selector: String,
}

// ============================================================================
// Execution Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInput {
    pub command: String,
    pub cwd: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub secret: Option<String>, // For %secret% placeholder
    pub nothrow: Option<bool>,
    pub force_output: Option<bool>,
    pub timeout: Option<u64>,      // Timeout in seconds
    pub idle_timeout: Option<u64>, // Idle timeout in seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOutput {
    pub success: bool,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeInput {
    pub task: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnInput {
    pub hosts: Vec<String>, // Host aliases or selector string
    pub callback: String,   // WASM callback ID
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithinInput {
    pub path: String,
    pub callback: String, // WASM callback ID
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BecomeInput {
    pub user: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreHandle {
    pub handle: u64, // Opaque handle to restore
}

// ============================================================================
// Filesystem Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadInput {
    pub source: String,
    pub destination: String,
    pub config: Option<UploadConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadConfig {
    pub flags: Option<String>,        // rsync flags override
    pub options: Option<Vec<String>>, // Additional rsync options
    pub timeout: Option<u64>,
    pub progress_bar: Option<bool>,
    pub display_stats: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadInput {
    pub source: String,
    pub destination: String,
    pub config: Option<UploadConfig>, // Reuse same config struct
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferResult {
    pub success: bool,
    pub message: Option<String>,
    pub error: Option<String>,
}

// ============================================================================
// Interaction Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskInput {
    pub message: String,
    pub default: Option<String>,
    pub autocomplete: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceInput {
    pub message: String,
    pub choices: Vec<String>,
    pub default: Option<String>,
    pub multiselect: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceOutput {
    pub selected: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmInput {
    pub message: String,
    pub default: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputHandle {
    pub handle: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputHandle {
    pub handle: u64,
}

// ============================================================================
// Utility Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmdExistsOutput {
    pub exists: u8, // 1 = true, 0 = false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSupportsInput {
    pub command: String,
    pub option: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhichOutput {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub message: String,
    pub code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchInput {
    pub url: String,
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchOutput {
    pub success: bool,
    pub status_code: Option<u16>,
    pub body: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoidResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResponse {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionInput {
    pub name: String,
    pub shortcut: Option<String>,
    pub mode: Option<String>,
    pub desc: Option<String>,
    pub default: Option<String>,
}

// ============================================================================
// Test Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestOutput {
    pub result: u8, // 1 = true, 0 = false
}

// ============================================================================
// Plugin Handshake Types
// ============================================================================

/// Handshake challenge sent from host to plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHandshakeChallenge {
    pub nonce: String,
    pub cli_version: String,
}

/// Handshake response from plugin to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHandshakeResponse {
    pub hmac: String,
}

// ============================================================================
// Plugin Info / Execute Types
// ============================================================================

/// Plugin metadata returned by `appz_plugin_info()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub commands: Vec<PluginCommandDef>,
}

/// Describes a CLI command provided by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommandDef {
    pub name: String,
    pub about: String,
    #[serde(default)]
    pub args: Vec<PluginArgDef>,
    #[serde(default)]
    pub subcommands: Vec<PluginCommandDef>,
}

/// Describes a CLI argument for a plugin command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginArgDef {
    pub name: String,
    #[serde(default)]
    pub short: Option<char>,
    #[serde(default)]
    pub long: Option<String>,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<String>,
}

/// Input to `appz_plugin_execute()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginExecuteInput {
    pub command: String,
    pub args: HashMap<String, serde_json::Value>,
    pub working_dir: String,
}

/// Output from `appz_plugin_execute()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginExecuteOutput {
    pub exit_code: i32,
    pub message: Option<String>,
}

// ============================================================================
// Plugin Filesystem Types (for host function calls)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFsReadInput {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFsReadOutput {
    pub success: bool,
    pub content: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFsWriteInput {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFsWriteOutput {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFsWalkInput {
    pub path: String,
    pub glob: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFsWalkOutput {
    pub success: bool,
    pub paths: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFsExistsOutput {
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFsCopyInput {
    pub source: String,
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFsJsonInput {
    pub path: String,
    pub content: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginFsJsonOutput {
    pub success: bool,
    pub content: Option<serde_json::Value>,
    pub error: Option<String>,
}

// ============================================================================
// Plugin Git Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginGitFilesOutput {
    pub success: bool,
    pub files: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginGitIsRepoOutput {
    pub is_repo: bool,
}

// ============================================================================
// Plugin Sandbox Exec Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSandboxExecInput {
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSandboxExecOutput {
    pub success: bool,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSandboxToolInput {
    pub tool: String,
    pub version: Option<String>,
    pub command: Option<String>,
}

// ============================================================================
// Plugin AST Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAstTransformInput {
    pub code: String,
    pub rules: Vec<PluginAstRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAstRule {
    pub rule_type: String,
    pub params: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAstTransformOutput {
    pub success: bool,
    pub code: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAstParseInput {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAstParseOutput {
    pub success: bool,
    pub ast: Option<serde_json::Value>,
    pub error: Option<String>,
}

// ============================================================================
// Plugin Check Run Types (for appz_pcheck_run host function)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCheckRunInput {
    pub working_dir: String,
    pub fix: bool,
    pub ai_fix: bool,
    pub strict: bool,
    pub changed: bool,
    pub staged: bool,
    pub format: bool,
    pub json: bool,
    pub checker: Option<String>,
    pub jobs: Option<usize>,
    pub init: bool,
    pub max_attempts: u32,
    pub ai_verify: Option<bool>,
    pub verbose_ai: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCheckRunOutput {
    pub exit_code: i32,
    pub message: Option<String>,
}

// ============================================================================
// Plugin Site Run Types (for appz_psite_run host function)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSiteRunInput {
    pub working_dir: String,
    /// Subcommand: "redesign" | "create" | "clone" | "generate-page"
    pub subcommand: String,
    /// For redesign/clone: source URL
    pub url: Option<String>,
    /// For create: natural-language prompt
    pub prompt: Option<String>,
    /// Output directory
    pub output: Option<String>,
    pub theme: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub transform_content: bool,
    pub no_build: bool,
    pub resume: bool,
    pub dry_run: bool,
    /// For generate-page: page paths or ["*"] for all
    pub pages: Option<Vec<String>>,
    pub all: bool,
    /// For generate-page: create-mode project (no URL)
    pub create: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSiteRunOutput {
    pub exit_code: i32,
    pub message: Option<String>,
}

// ============================================================================
// Plugin Migrate Run Types (for appz_pmigrate_run host function)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrateRunInput {
    pub working_dir: String,
    pub source_dir: String,
    pub output_dir: String,
    pub target: String,
    pub force: bool,
    pub dry_run: bool,
    pub install: bool,
    pub yes: bool,
    #[serde(default)]
    pub transforms: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrateRunOutput {
    pub exit_code: i32,
    pub message: Option<String>,
}

// ============================================================================
// Plugin Convert Run Types (for appz_pconvert_run host function)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertRunInput {
    pub working_dir: String,
    pub files: Vec<String>,
    pub dry_run: bool,
    pub force: bool,
    pub output: Option<String>,
    pub target: String,
    pub transform: Option<String>,
    pub list: bool,
    pub client_directive: Option<String>,
    pub slot_style: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertRunOutput {
    pub exit_code: i32,
    pub message: Option<String>,
}

// ============================================================================
// Plugin HTTP Download Types (for appz_phttp_download host function)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHttpDownloadInput {
    pub url: String,
    pub dest_path: String,
    pub strict_ssl: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHttpDownloadOutput {
    pub success: bool,
    pub bytes_written: Option<u64>,
    pub error: Option<String>,
}
