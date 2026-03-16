use serde::Deserialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Top-level Blueprint
// ---------------------------------------------------------------------------

/// WordPress Playground Blueprint — full spec parity.
/// See: https://wordpress.github.io/wordpress-playground/blueprints/data-format
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blueprint {
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,

    #[serde(default)]
    pub meta: Option<Meta>,

    #[serde(default)]
    pub landing_page: Option<String>,

    /// Deprecated top-level description — use meta.description instead.
    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub preferred_versions: Option<PreferredVersions>,

    #[serde(default)]
    pub features: Option<Features>,

    #[serde(default)]
    pub extra_libraries: Vec<String>,

    /// Top-level shorthand: constants to define in wp-config.php.
    #[serde(default)]
    pub constants: Option<HashMap<String, serde_json::Value>>,

    /// Top-level shorthand: plugin slugs to install.
    #[serde(default)]
    pub plugins: Vec<PluginShorthand>,

    /// Top-level shorthand: site options.
    #[serde(default)]
    pub site_options: Option<HashMap<String, serde_json::Value>>,

    /// Top-level shorthand: `true` or `{ username, password }`.
    #[serde(default)]
    pub login: Option<LoginShorthand>,

    #[serde(default)]
    pub steps: Vec<StepEntry>,
}

// ---------------------------------------------------------------------------
// Meta
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
}

// ---------------------------------------------------------------------------
// Preferred versions
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PreferredVersions {
    pub php: Option<String>,
    pub wp: Option<String>,
}

// ---------------------------------------------------------------------------
// Features
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct Features {
    pub networking: Option<bool>,
    pub intl: Option<bool>,
}

// ---------------------------------------------------------------------------
// Login shorthand (top-level)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum LoginShorthand {
    Bool(bool),
    Credentials {
        username: Option<String>,
        password: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Plugin shorthand (top-level `plugins` array)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum PluginShorthand {
    Slug(String),
    Resource(FileResource),
}

// ---------------------------------------------------------------------------
// Steps — the `steps` array may contain step objects, strings, false, or null
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StepEntry {
    Step(Step),
    /// String shorthand (e.g. plugin slug).
    String(String),
    /// `false` or `null` entries are skipped.
    Bool(bool),
    Null,
}

// ---------------------------------------------------------------------------
// Step enum — all 29 step types via internally-tagged enum
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(tag = "step")]
pub enum Step {
    #[serde(rename = "activatePlugin")]
    ActivatePlugin(ActivatePluginStep),

    #[serde(rename = "activateTheme")]
    ActivateTheme(ActivateThemeStep),

    #[serde(rename = "cp")]
    Cp(CpStep),

    #[serde(rename = "defineWpConfigConsts")]
    DefineWpConfigConsts(DefineWpConfigConstsStep),

    #[serde(rename = "defineSiteUrl")]
    DefineSiteUrl(DefineSiteUrlStep),

    #[serde(rename = "enableMultisite")]
    EnableMultisite(EnableMultisiteStep),

    #[serde(rename = "importThemeStarterContent")]
    ImportThemeStarterContent(ImportThemeStarterContentStep),

    #[serde(rename = "importWordPressFiles")]
    ImportWordPressFiles(ImportWordPressFilesStep),

    #[serde(rename = "importWxr")]
    ImportWxr(ImportWxrStep),

    #[serde(rename = "installPlugin")]
    InstallPlugin(InstallPluginStep),

    #[serde(rename = "installTheme")]
    InstallTheme(InstallThemeStep),

    #[serde(rename = "login")]
    Login(LoginStep),

    #[serde(rename = "mkdir")]
    Mkdir(MkdirStep),

    #[serde(rename = "mv")]
    Mv(MvStep),

    #[serde(rename = "request")]
    Request(RequestStep),

    #[serde(rename = "resetData")]
    ResetData(ResetDataStep),

    #[serde(rename = "rm")]
    Rm(RmStep),

    #[serde(rename = "rmdir")]
    Rmdir(RmdirStep),

    #[serde(rename = "runPHP")]
    RunPHP(RunPhpStep),

    #[serde(rename = "runPHPWithOptions")]
    RunPHPWithOptions(RunPhpWithOptionsStep),

    #[serde(rename = "runWpInstallationWizard")]
    RunWpInstallationWizard(RunWpInstallationWizardStep),

    #[serde(rename = "runSql")]
    RunSql(RunSqlStep),

    #[serde(rename = "setSiteLanguage")]
    SetSiteLanguage(SetSiteLanguageStep),

    #[serde(rename = "setSiteOptions")]
    SetSiteOptions(SetSiteOptionsStep),

    #[serde(rename = "unzip")]
    Unzip(UnzipStep),

    #[serde(rename = "updateUserMeta")]
    UpdateUserMeta(UpdateUserMetaStep),

    #[serde(rename = "writeFile")]
    WriteFile(WriteFileStep),

    #[serde(rename = "writeFiles")]
    WriteFiles(WriteFilesStep),

    #[serde(rename = "wp-cli")]
    WpCli(WpCliStep),
}

impl Step {
    /// Return the step type string for logging.
    pub fn type_name(&self) -> &'static str {
        match self {
            Step::ActivatePlugin(_) => "activatePlugin",
            Step::ActivateTheme(_) => "activateTheme",
            Step::Cp(_) => "cp",
            Step::DefineWpConfigConsts(_) => "defineWpConfigConsts",
            Step::DefineSiteUrl(_) => "defineSiteUrl",
            Step::EnableMultisite(_) => "enableMultisite",
            Step::ImportThemeStarterContent(_) => "importThemeStarterContent",
            Step::ImportWordPressFiles(_) => "importWordPressFiles",
            Step::ImportWxr(_) => "importWxr",
            Step::InstallPlugin(_) => "installPlugin",
            Step::InstallTheme(_) => "installTheme",
            Step::Login(_) => "login",
            Step::Mkdir(_) => "mkdir",
            Step::Mv(_) => "mv",
            Step::Request(_) => "request",
            Step::ResetData(_) => "resetData",
            Step::Rm(_) => "rm",
            Step::Rmdir(_) => "rmdir",
            Step::RunPHP(_) => "runPHP",
            Step::RunPHPWithOptions(_) => "runPHPWithOptions",
            Step::RunWpInstallationWizard(_) => "runWpInstallationWizard",
            Step::RunSql(_) => "runSql",
            Step::SetSiteLanguage(_) => "setSiteLanguage",
            Step::SetSiteOptions(_) => "setSiteOptions",
            Step::Unzip(_) => "unzip",
            Step::UpdateUserMeta(_) => "updateUserMeta",
            Step::WriteFile(_) => "writeFile",
            Step::WriteFiles(_) => "writeFiles",
            Step::WpCli(_) => "wp-cli",
        }
    }
}

// ---------------------------------------------------------------------------
// Resource types
// ---------------------------------------------------------------------------

/// A file resource reference — covers all spec resource types.
#[derive(Debug, Deserialize)]
#[serde(tag = "resource")]
pub enum FileResource {
    #[serde(rename = "wordpress.org/plugins")]
    WordPressPlugin { slug: String },

    #[serde(rename = "wordpress.org/themes")]
    WordPressTheme { slug: String },

    #[serde(rename = "url")]
    Url {
        url: String,
        #[serde(default)]
        caption: Option<String>,
    },

    #[serde(rename = "vfs")]
    Vfs { path: String },

    #[serde(rename = "literal")]
    Literal {
        #[serde(default)]
        name: Option<String>,
        contents: serde_json::Value,
    },

    #[serde(rename = "bundled")]
    Bundled { path: String },

    #[serde(rename = "zip")]
    Zip {
        inner: Box<serde_json::Value>,
        #[serde(default)]
        name: Option<String>,
    },
}

/// A directory resource reference.
#[derive(Debug, Deserialize)]
#[serde(tag = "resource")]
pub enum DirectoryResource {
    #[serde(rename = "git:directory")]
    Git {
        url: String,
        #[serde(default, rename = "ref")]
        git_ref: Option<String>,
        #[serde(default, rename = "refType")]
        ref_type: Option<String>,
        #[serde(default)]
        path: Option<String>,
    },

    #[serde(rename = "literal:directory")]
    Literal {
        #[serde(default)]
        name: Option<String>,
        files: serde_json::Value,
    },
}

/// `pluginData` / `themeData` can be a FileResource, DirectoryResource, or raw JSON.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ResourceData {
    File(FileResource),
    Directory(DirectoryResource),
    /// Fallback for any shape we don't explicitly handle.
    Raw(serde_json::Value),
}

/// `writeFile.data` can be a string, FileResource, or raw bytes (TypedArray in spec).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum WriteFileData {
    String(String),
    Resource(FileResource),
    Raw(serde_json::Value),
}

/// `writeFiles.filesTree` can be a DirectoryResource or raw JSON.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum FilesTreeData {
    Directory(DirectoryResource),
    Raw(serde_json::Value),
}

/// `runPHP.code` can be a string or `{ filename, content }`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum PhpCode {
    String(String),
    File {
        #[serde(default)]
        filename: Option<String>,
        content: String,
    },
}

/// `wp-cli.command` can be a string or array of strings.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum WpCliCommand {
    String(String),
    Args(Vec<String>),
}

// ---------------------------------------------------------------------------
// Step structs — one per step type
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivatePluginStep {
    pub plugin_path: String,
    #[serde(default)]
    pub plugin_name: Option<String>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateThemeStep {
    pub theme_folder_name: String,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpStep {
    pub from_path: String,
    pub to_path: String,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefineWpConfigConstsStep {
    pub consts: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub virtualize: Option<bool>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefineSiteUrlStep {
    pub site_url: String,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnableMultisiteStep {
    #[serde(default)]
    pub wp_cli_path: Option<String>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportThemeStarterContentStep {
    #[serde(default)]
    pub theme_slug: Option<String>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportWordPressFilesStep {
    #[serde(rename = "wordPressFilesZip")]
    pub wordpress_files_zip: serde_json::Value,
    #[serde(default)]
    pub path_in_zip: Option<String>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportWxrStep {
    pub file: serde_json::Value,
    #[serde(default)]
    pub importer: Option<String>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPluginStep {
    #[serde(default)]
    pub plugin_data: Option<ResourceData>,
    /// Deprecated — use pluginData.
    #[serde(default)]
    pub plugin_zip_file: Option<serde_json::Value>,
    #[serde(default)]
    pub options: Option<InstallPluginOptions>,
    #[serde(default)]
    pub if_already_installed: Option<String>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPluginOptions {
    #[serde(default)]
    pub activate: Option<bool>,
    #[serde(default)]
    pub target_folder_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallThemeStep {
    #[serde(default)]
    pub theme_data: Option<ResourceData>,
    /// Deprecated — use themeData.
    #[serde(default)]
    pub theme_zip_file: Option<serde_json::Value>,
    #[serde(default)]
    pub options: Option<InstallThemeOptions>,
    #[serde(default)]
    pub if_already_installed: Option<String>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallThemeOptions {
    #[serde(default)]
    pub activate: Option<bool>,
    #[serde(default)]
    pub import_starter_content: Option<bool>,
    #[serde(default)]
    pub target_folder_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginStep {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MkdirStep {
    pub path: String,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MvStep {
    pub from_path: String,
    pub to_path: String,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestStep {
    pub request: serde_json::Value,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetDataStep {
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RmStep {
    pub path: String,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RmdirStep {
    pub path: String,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPhpStep {
    pub code: PhpCode,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPhpWithOptionsStep {
    pub options: serde_json::Value,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunWpInstallationWizardStep {
    #[serde(default)]
    pub options: Option<WpInstallationOptions>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WpInstallationOptions {
    #[serde(default)]
    pub admin_username: Option<String>,
    #[serde(default)]
    pub admin_password: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSqlStep {
    pub sql: serde_json::Value,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSiteLanguageStep {
    pub language: String,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSiteOptionsStep {
    pub options: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnzipStep {
    pub zip_file: serde_json::Value,
    pub extract_to_path: String,
    /// Deprecated.
    #[serde(default)]
    pub zip_path: Option<String>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserMetaStep {
    pub user_id: u64,
    pub meta: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteFileStep {
    pub path: String,
    pub data: WriteFileData,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteFilesStep {
    pub write_to_path: String,
    pub files_tree: FilesTreeData,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WpCliStep {
    pub command: WpCliCommand,
    #[serde(default)]
    pub wp_cli_path: Option<String>,
    #[serde(default)]
    pub progress: Option<serde_json::Value>,
}
