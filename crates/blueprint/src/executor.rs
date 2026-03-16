use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::error::BlueprintError;
use crate::runtime::WordPressRuntime;
use crate::runtimes::DdevRuntime;
use crate::types::*;

/// Executes a parsed Blueprint against a WordPress project using a runtime backend.
pub struct BlueprintExecutor {
    project_path: PathBuf,
    runtime: Arc<dyn WordPressRuntime>,
}

impl BlueprintExecutor {
    /// Create a new executor with a specific runtime.
    pub fn new(project_path: PathBuf, runtime: Arc<dyn WordPressRuntime>) -> Self {
        Self {
            project_path,
            runtime,
        }
    }

    /// Create an executor using the DDEV runtime (convenience for backward compatibility).
    pub fn with_ddev(project_path: PathBuf) -> Self {
        Self::new(project_path, Arc::new(DdevRuntime::new()))
    }

    /// Execute all steps in the blueprint sequentially.
    pub fn execute(&self, blueprint: &Blueprint) -> Result<(), BlueprintError> {
        // Apply top-level shorthand fields first
        self.apply_preferred_versions(blueprint)?;
        self.apply_top_level_constants(blueprint)?;
        self.apply_top_level_plugins(blueprint)?;
        self.apply_top_level_site_options(blueprint)?;
        self.apply_top_level_login(blueprint)?;

        // Execute steps
        let total = blueprint.steps.len();
        for (i, entry) in blueprint.steps.iter().enumerate() {
            match entry {
                StepEntry::Step(step) => {
                    println!("  [{}/{}] {}...", i + 1, total, step.type_name());
                    self.execute_step(i, step)?;
                }
                StepEntry::String(s) => {
                    // String shorthand — treat as plugin slug install
                    println!("  [{}/{}] installPlugin ({})...", i + 1, total, s);
                    self.runtime.wp_cli(&self.project_path, &["plugin", "install", s, "--force", "--activate"])?;
                }
                StepEntry::Bool(false) | StepEntry::Null => {
                    // Skip disabled/null entries
                }
                StepEntry::Bool(true) => {
                    // `true` is not meaningful, skip
                }
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Top-level shorthand handlers
    // -----------------------------------------------------------------------

    fn apply_preferred_versions(&self, bp: &Blueprint) -> Result<(), BlueprintError> {
        if let Some(ref versions) = bp.preferred_versions {
            if let Some(ref php) = versions.php {
                let php_version = normalize_php_version(php);
                if php_version != "latest" {
                    println!("  Configuring PHP {}...", php_version);
                    self.runtime.set_php_version(&self.project_path, &php_version)?;
                }
            }
            // wp version: handled during init (download specific version).
            if let Some(ref wp) = versions.wp {
                if wp != "latest" && !wp.starts_with("http") {
                    tracing::info!("Blueprint requests WordPress {}, but version is managed by the runtime/init", wp);
                }
            }
        }
        Ok(())
    }

    fn apply_top_level_constants(&self, bp: &Blueprint) -> Result<(), BlueprintError> {
        if let Some(ref consts) = bp.constants {
            for (name, value) in consts {
                let val_str = json_value_to_wp_config_string(value);
                let mut args = vec!["config", "set", name.as_str(), val_str.as_str(), "--type=constant"];
                if value.is_boolean() || value.is_number() {
                    args.push("--raw");
                }
                self.runtime.wp_cli(&self.project_path, &args)?;
            }
        }
        Ok(())
    }

    fn apply_top_level_plugins(&self, bp: &Blueprint) -> Result<(), BlueprintError> {
        for plugin in &bp.plugins {
            match plugin {
                PluginShorthand::Slug(slug) => {
                    println!("  Installing plugin: {}...", slug);
                    self.runtime.wp_cli(&self.project_path, &["plugin", "install", slug, "--force", "--activate"])?;
                }
                PluginShorthand::Resource(resource) => {
                    self.install_plugin_from_resource(resource, true)?;
                }
            }
        }
        Ok(())
    }

    fn apply_top_level_site_options(&self, bp: &Blueprint) -> Result<(), BlueprintError> {
        if let Some(ref opts) = bp.site_options {
            self.set_site_options(opts)?;
        }
        Ok(())
    }

    fn apply_top_level_login(&self, bp: &Blueprint) -> Result<(), BlueprintError> {
        if let Some(ref login) = bp.login {
            match login {
                LoginShorthand::Bool(true) => {
                    tracing::info!("Blueprint requests auto-login (handled by runtime)");
                }
                LoginShorthand::Bool(false) => {}
                LoginShorthand::Credentials { username, password } => {
                    let user = username.as_deref().unwrap_or("admin");
                    if let Some(pass) = password {
                        let pass_flag = format!("--user_pass={}", pass);
                        self.runtime.wp_cli(&self.project_path, &["user", "update", user, &pass_flag])?;
                    }
                }
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Step dispatch
    // -----------------------------------------------------------------------

    fn execute_step(&self, index: usize, step: &Step) -> Result<(), BlueprintError> {
        let result = match step {
            Step::ActivatePlugin(s) => self.step_activate_plugin(s),
            Step::ActivateTheme(s) => self.step_activate_theme(s),
            Step::Cp(s) => self.step_cp(s),
            Step::DefineWpConfigConsts(s) => self.step_define_wp_config_consts(s),
            Step::DefineSiteUrl(s) => self.step_define_site_url(s),
            Step::EnableMultisite(s) => self.step_enable_multisite(s),
            Step::ImportThemeStarterContent(s) => self.step_import_theme_starter_content(s),
            Step::ImportWordPressFiles(s) => self.step_import_wordpress_files(s),
            Step::ImportWxr(s) => self.step_import_wxr(s),
            Step::InstallPlugin(s) => self.step_install_plugin(s),
            Step::InstallTheme(s) => self.step_install_theme(s),
            Step::Login(s) => self.step_login(s),
            Step::Mkdir(s) => self.step_mkdir(s),
            Step::Mv(s) => self.step_mv(s),
            Step::Request(s) => self.step_request(s),
            Step::ResetData(s) => self.step_reset_data(s),
            Step::Rm(s) => self.step_rm(s),
            Step::Rmdir(s) => self.step_rmdir(s),
            Step::RunPHP(s) => self.step_run_php(s),
            Step::RunPHPWithOptions(s) => self.step_run_php_with_options(s),
            Step::RunWpInstallationWizard(s) => self.step_run_wp_installation_wizard(s),
            Step::RunSql(s) => self.step_run_sql(s),
            Step::SetSiteLanguage(s) => self.step_set_site_language(s),
            Step::SetSiteOptions(s) => self.step_set_site_options(s),
            Step::Unzip(s) => self.step_unzip(s),
            Step::UpdateUserMeta(s) => self.step_update_user_meta(s),
            Step::WriteFile(s) => self.step_write_file(s),
            Step::WriteFiles(s) => self.step_write_files(s),
            Step::WpCli(s) => self.step_wp_cli(s),
        };
        result.map_err(|e| BlueprintError::StepFailed {
            step_index: index,
            step_type: step.type_name().to_string(),
            message: e.to_string(),
        })
    }

    // -----------------------------------------------------------------------
    // Step implementations
    // -----------------------------------------------------------------------

    fn step_activate_plugin(&self, s: &ActivatePluginStep) -> Result<(), BlueprintError> {
        let plugin_name = extract_plugin_slug(&s.plugin_path);
        self.runtime.wp_cli(&self.project_path, &["plugin", "activate", &plugin_name])?;
        Ok(())
    }

    fn step_activate_theme(&self, s: &ActivateThemeStep) -> Result<(), BlueprintError> {
        self.runtime.wp_cli(&self.project_path, &["theme", "activate", &s.theme_folder_name])?;
        Ok(())
    }

    fn step_cp(&self, s: &CpStep) -> Result<(), BlueprintError> {
        self.runtime.exec_args(&self.project_path, &["exec", "cp", "-r", "--", &s.from_path, &s.to_path])?;
        Ok(())
    }

    fn step_define_wp_config_consts(&self, s: &DefineWpConfigConstsStep) -> Result<(), BlueprintError> {
        for (name, value) in &s.consts {
            let val_str = json_value_to_wp_config_string(value);
            let mut args = vec!["config", "set", name.as_str(), val_str.as_str(), "--type=constant"];
            if value.is_boolean() || value.is_number() {
                args.push("--raw");
            }
            self.runtime.wp_cli(&self.project_path, &args)?;
        }
        Ok(())
    }

    fn step_define_site_url(&self, s: &DefineSiteUrlStep) -> Result<(), BlueprintError> {
        self.runtime.wp_cli(&self.project_path, &["option", "update", "siteurl", &s.site_url])?;
        self.runtime.wp_cli(&self.project_path, &["option", "update", "home", &s.site_url])?;
        Ok(())
    }

    fn step_enable_multisite(&self, _s: &EnableMultisiteStep) -> Result<(), BlueprintError> {
        self.runtime.wp_cli(&self.project_path, &["core", "multisite-convert"])?;
        Ok(())
    }

    fn step_import_theme_starter_content(&self, s: &ImportThemeStarterContentStep) -> Result<(), BlueprintError> {
        if let Some(ref slug) = s.theme_slug {
            self.runtime.wp_cli(&self.project_path, &["theme", "activate", slug])?;
        }
        self.runtime.wp_eval(
            &self.project_path,
            "do_action(\"after_switch_theme\");",
        )?;
        Ok(())
    }

    fn step_import_wordpress_files(&self, s: &ImportWordPressFilesStep) -> Result<(), BlueprintError> {
        let resource: Result<FileResource, _> = serde_json::from_value(s.wordpress_files_zip.clone());
        match resource {
            Ok(FileResource::Url { url, .. }) => {
                self.runtime.download_and_unzip(
                    &self.project_path,
                    &url,
                    "/var/www/html/",
                )?;
            }
            _ => {
                tracing::warn!("importWordPressFiles: unsupported resource type, skipping");
            }
        }
        Ok(())
    }

    fn step_import_wxr(&self, s: &ImportWxrStep) -> Result<(), BlueprintError> {
        let _ = self.runtime.wp_cli(&self.project_path, &["plugin", "install", "wordpress-importer", "--activate"]);

        let resource: Result<FileResource, _> = serde_json::from_value(s.file.clone());
        match resource {
            Ok(FileResource::Url { url, .. }) => {
                self.runtime.download_url(&self.project_path, &url, "/tmp/import.wxr")?;
                self.runtime.wp_cli(&self.project_path, &["import", "/tmp/import.wxr", "--authors=create"])?;
            }
            Ok(FileResource::Literal { contents, .. }) => {
                if let Some(content_str) = contents.as_str() {
                    let wxr_path = self.project_path.join(".blueprint-import.wxr");
                    std::fs::write(&wxr_path, content_str).map_err(|e| BlueprintError::Io {
                        path: wxr_path.clone(),
                        source: e,
                    })?;
                    let result = self.runtime.wp_cli(
                        &self.project_path,
                        &["import", "/var/www/html/.blueprint-import.wxr", "--authors=create"],
                    );
                    let _ = std::fs::remove_file(&wxr_path);
                    result?;
                } else {
                    tracing::warn!("importWxr: literal contents is not a string, skipping");
                }
            }
            _ => {
                tracing::warn!("importWxr: unsupported resource type, skipping");
            }
        }
        Ok(())
    }

    fn step_install_plugin(&self, s: &InstallPluginStep) -> Result<(), BlueprintError> {
        let activate = s
            .options
            .as_ref()
            .and_then(|o| o.activate)
            .unwrap_or(true);

        if let Some(resource_data) = &s.plugin_data {
            match resource_data {
                ResourceData::File(resource) => {
                    self.install_plugin_from_resource(resource, activate)?;
                }
                ResourceData::Directory(_dir) => {
                    tracing::warn!("installPlugin: directory resources are not yet supported, skipping");
                }
                ResourceData::Raw(v) => {
                    if let Ok(resource) = serde_json::from_value::<FileResource>(v.clone()) {
                        self.install_plugin_from_resource(&resource, activate)?;
                    } else {
                        tracing::warn!("installPlugin: unrecognized pluginData format, skipping");
                    }
                }
            }
        } else if let Some(ref zip_val) = s.plugin_zip_file {
            if let Ok(resource) = serde_json::from_value::<FileResource>(zip_val.clone()) {
                self.install_plugin_from_resource(&resource, activate)?;
            } else {
                tracing::warn!("installPlugin: unrecognized pluginZipFile format, skipping");
            }
        }
        Ok(())
    }

    fn step_install_theme(&self, s: &InstallThemeStep) -> Result<(), BlueprintError> {
        let activate = s
            .options
            .as_ref()
            .and_then(|o| o.activate)
            .unwrap_or(false);
        let import_starter = s
            .options
            .as_ref()
            .and_then(|o| o.import_starter_content)
            .unwrap_or(false);

        if let Some(ref resource_data) = s.theme_data {
            match resource_data {
                ResourceData::File(resource) => {
                    self.install_theme_from_resource(resource, activate)?;
                }
                ResourceData::Directory(_) => {
                    tracing::warn!("installTheme: directory resources are not yet supported, skipping");
                }
                ResourceData::Raw(v) => {
                    if let Ok(resource) = serde_json::from_value::<FileResource>(v.clone()) {
                        self.install_theme_from_resource(&resource, activate)?;
                    } else {
                        tracing::warn!("installTheme: unrecognized themeData format, skipping");
                    }
                }
            }
        } else if let Some(ref zip_val) = s.theme_zip_file {
            if let Ok(resource) = serde_json::from_value::<FileResource>(zip_val.clone()) {
                self.install_theme_from_resource(&resource, activate)?;
            }
        }

        if import_starter {
            self.runtime.wp_eval(&self.project_path, "do_action(\"after_switch_theme\");")?;
        }

        Ok(())
    }

    fn step_login(&self, s: &LoginStep) -> Result<(), BlueprintError> {
        let user = s.username.as_deref().unwrap_or("admin");
        if let Some(ref pass) = s.password {
            let pass_flag = format!("--user_pass={}", pass);
            self.runtime.wp_cli(&self.project_path, &["user", "update", user, &pass_flag])?;
        }
        Ok(())
    }

    fn step_mkdir(&self, s: &MkdirStep) -> Result<(), BlueprintError> {
        self.runtime.exec_args(&self.project_path, &["exec", "mkdir", "-p", &s.path])?;
        Ok(())
    }

    fn step_mv(&self, s: &MvStep) -> Result<(), BlueprintError> {
        self.runtime.exec_args(&self.project_path, &["exec", "mv", "--", &s.from_path, &s.to_path])?;
        Ok(())
    }

    fn step_request(&self, s: &RequestStep) -> Result<(), BlueprintError> {
        if let Some(url) = s.request.get("url").and_then(|v| v.as_str()) {
            let method = s.request.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
            self.runtime.http_request(&self.project_path, method, url)?;
        } else {
            tracing::warn!("request step: no URL found in request object, skipping");
        }
        Ok(())
    }

    fn step_reset_data(&self, _s: &ResetDataStep) -> Result<(), BlueprintError> {
        tracing::warn!("resetData: this will DROP ALL database tables and is destructive");
        self.runtime.wp_cli(&self.project_path, &["db", "reset", "--yes"])?;
        Ok(())
    }

    fn step_rm(&self, s: &RmStep) -> Result<(), BlueprintError> {
        self.runtime.exec_args(&self.project_path, &["exec", "rm", "-f", "--", &s.path])?;
        Ok(())
    }

    fn step_rmdir(&self, s: &RmdirStep) -> Result<(), BlueprintError> {
        self.runtime.exec_args(&self.project_path, &["exec", "rm", "-rf", "--", &s.path])?;
        Ok(())
    }

    fn step_run_php(&self, s: &RunPhpStep) -> Result<(), BlueprintError> {
        let code = match &s.code {
            PhpCode::String(code) => code.clone(),
            PhpCode::File { content, .. } => content.clone(),
        };
        let code = code.strip_prefix("<?php").unwrap_or(&code).trim().to_string();
        self.runtime.wp_cli(&self.project_path, &["eval", &code])?;
        Ok(())
    }

    fn step_run_php_with_options(&self, s: &RunPhpWithOptionsStep) -> Result<(), BlueprintError> {
        if let Some(code) = s.options.get("code").and_then(|v| v.as_str()) {
            let code = code.strip_prefix("<?php").unwrap_or(code).trim().to_string();
            self.runtime.wp_cli(&self.project_path, &["eval", &code])?;
        } else {
            tracing::warn!("runPHPWithOptions: no code field found, skipping");
        }
        Ok(())
    }

    fn step_run_wp_installation_wizard(&self, s: &RunWpInstallationWizardStep) -> Result<(), BlueprintError> {
        let opts = s.options.as_ref();
        let user = opts
            .and_then(|o| o.admin_username.as_deref())
            .unwrap_or("admin");
        let pass = opts
            .and_then(|o| o.admin_password.as_deref())
            .unwrap_or("admin");

        let url = self.runtime.site_url(&self.project_path);
        self.runtime.wp_install(&self.project_path, &url, user, pass)?;
        Ok(())
    }

    fn step_run_sql(&self, s: &RunSqlStep) -> Result<(), BlueprintError> {
        let resource: Result<FileResource, _> = serde_json::from_value(s.sql.clone());
        match resource {
            Ok(FileResource::Url { url, .. }) => {
                // Download SQL file, then execute it
                self.runtime.download_url(&self.project_path, &url, "/tmp/_bp_sql.sql")?;
                self.runtime.wp_cli(&self.project_path, &["db", "import", "/tmp/_bp_sql.sql"])?;
            }
            Ok(FileResource::Literal { contents, .. }) => {
                if let Some(sql) = contents.as_str() {
                    self.runtime.exec_sql(&self.project_path, sql)?;
                } else {
                    tracing::warn!("runSql: literal contents is not a string, skipping");
                }
            }
            _ => {
                if let Some(sql) = s.sql.as_str() {
                    self.runtime.exec_sql(&self.project_path, sql)?;
                } else {
                    tracing::warn!("runSql: unsupported SQL resource type, skipping");
                }
            }
        }
        Ok(())
    }

    fn step_set_site_language(&self, s: &SetSiteLanguageStep) -> Result<(), BlueprintError> {
        self.runtime.wp_cli(&self.project_path, &["language", "core", "install", &s.language])?;
        self.runtime.wp_cli(&self.project_path, &["site", "switch-language", &s.language])?;
        Ok(())
    }

    fn step_set_site_options(&self, s: &SetSiteOptionsStep) -> Result<(), BlueprintError> {
        self.set_site_options(&s.options)
    }

    fn step_unzip(&self, s: &UnzipStep) -> Result<(), BlueprintError> {
        let resource: Result<FileResource, _> = serde_json::from_value(s.zip_file.clone());
        match resource {
            Ok(FileResource::Url { url, .. }) => {
                self.runtime.download_and_unzip(
                    &self.project_path,
                    &url,
                    &s.extract_to_path,
                )?;
            }
            _ => {
                tracing::warn!("unzip: only URL resources are supported currently, skipping");
            }
        }
        Ok(())
    }

    fn step_update_user_meta(&self, s: &UpdateUserMetaStep) -> Result<(), BlueprintError> {
        let user_id_str = s.user_id.to_string();
        for (key, value) in &s.meta {
            let val_str = json_value_to_string(value);
            self.runtime.wp_cli(&self.project_path, &["user", "meta", "update", &user_id_str, key, &val_str])?;
        }
        Ok(())
    }

    fn step_write_file(&self, s: &WriteFileStep) -> Result<(), BlueprintError> {
        let content = match &s.data {
            WriteFileData::String(text) => text.clone(),
            WriteFileData::Resource(FileResource::Literal { contents, .. }) => {
                contents.as_str().unwrap_or("").to_string()
            }
            WriteFileData::Resource(FileResource::Url { url, .. }) => {
                let output = Command::new("curl")
                    .args(["-sL", url])
                    .output()
                    .map_err(|e| BlueprintError::Runtime(crate::runtime::RuntimeError::CommandFailed {
                        command: format!("curl -sL {}", url),
                        message: e.to_string(),
                    }))?;
                String::from_utf8_lossy(&output.stdout).to_string()
            }
            _ => {
                tracing::warn!("writeFile: unsupported data type, skipping");
                return Ok(());
            }
        };

        let relative_path = s.path.strip_prefix("/wordpress/")
            .or_else(|| s.path.strip_prefix("/var/www/html/"))
            .unwrap_or(&s.path);
        let target = self.project_path.join(relative_path);

        ensure_path_within(&self.project_path, &target)?;

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| BlueprintError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        std::fs::write(&target, &content).map_err(|e| BlueprintError::Io {
            path: target,
            source: e,
        })
    }

    fn step_write_files(&self, s: &WriteFilesStep) -> Result<(), BlueprintError> {
        let base_path = s.write_to_path.strip_prefix("/wordpress/")
            .or_else(|| s.write_to_path.strip_prefix("/var/www/html/"))
            .unwrap_or(&s.write_to_path);

        match &s.files_tree {
            FilesTreeData::Directory(DirectoryResource::Literal { files, .. }) => {
                self.write_files_tree(base_path, files)?;
            }
            FilesTreeData::Raw(v) => {
                if let Some(obj) = v.as_object() {
                    let value = serde_json::Value::Object(obj.clone());
                    self.write_files_tree(base_path, &value)?;
                } else {
                    tracing::warn!("writeFiles: unrecognized filesTree format, skipping");
                }
            }
            _ => {
                tracing::warn!("writeFiles: unsupported directory resource type, skipping");
            }
        }
        Ok(())
    }

    fn step_wp_cli(&self, s: &WpCliStep) -> Result<(), BlueprintError> {
        match &s.command {
            WpCliCommand::String(cmd) => {
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.is_empty() {
                    return Ok(());
                }
                self.runtime.wp_cli(&self.project_path, &parts)?;
            }
            WpCliCommand::Args(args) => {
                let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                self.runtime.wp_cli(&self.project_path, &args_refs)?;
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Resource helpers
    // -----------------------------------------------------------------------

    fn install_plugin_from_resource(
        &self,
        resource: &FileResource,
        activate: bool,
    ) -> Result<(), BlueprintError> {
        let mut args = vec!["plugin", "install"];
        let source: String;

        match resource {
            FileResource::WordPressPlugin { slug } => {
                source = slug.clone();
            }
            FileResource::Url { url, .. } => {
                source = url.clone();
            }
            _ => {
                tracing::warn!("installPlugin: unsupported resource type {:?}, skipping", resource);
                return Ok(());
            }
        }

        args.push(&source);
        args.push("--force");
        if activate {
            args.push("--activate");
        }
        self.runtime.wp_cli(&self.project_path, &args)?;
        Ok(())
    }

    fn install_theme_from_resource(
        &self,
        resource: &FileResource,
        activate: bool,
    ) -> Result<(), BlueprintError> {
        let mut args = vec!["theme", "install"];
        let source: String;

        match resource {
            FileResource::WordPressTheme { slug } => {
                source = slug.clone();
            }
            FileResource::Url { url, .. } => {
                source = url.clone();
            }
            _ => {
                tracing::warn!("installTheme: unsupported resource type, skipping");
                return Ok(());
            }
        }

        args.push(&source);
        args.push("--force");
        if activate {
            args.push("--activate");
        }
        self.runtime.wp_cli(&self.project_path, &args)?;
        Ok(())
    }

    fn set_site_options(&self, options: &HashMap<String, serde_json::Value>) -> Result<(), BlueprintError> {
        for (key, value) in options {
            let val_str = json_value_to_string(value);
            self.runtime.wp_cli(&self.project_path, &["option", "update", key, &val_str])?;
        }
        Ok(())
    }

    fn write_files_tree(&self, base: &str, tree: &serde_json::Value) -> Result<(), BlueprintError> {
        if let Some(obj) = tree.as_object() {
            for (name, content) in obj {
                let path = format!("{}/{}", base, name);
                if content.is_object() {
                    self.write_files_tree(&path, content)?;
                } else if let Some(text) = content.as_str() {
                    let target = self.project_path.join(&path);
                    ensure_path_within(&self.project_path, &target)?;
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| BlueprintError::Io {
                            path: parent.to_path_buf(),
                            source: e,
                        })?;
                    }
                    std::fs::write(&target, text).map_err(|e| BlueprintError::Io {
                        path: target,
                        source: e,
                    })?;
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Normalize a PHP version string to major.minor for DDEV.
fn normalize_php_version(version: &str) -> String {
    if version == "latest" {
        return "latest".to_string();
    }
    if version.starts_with("http") {
        return "latest".to_string();
    }
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        version.to_string()
    }
}

/// Extract plugin slug from a plugin path like "wp-content/plugins/gutenberg/gutenberg.php".
fn extract_plugin_slug(path: &str) -> String {
    let path = path
        .strip_prefix("/wordpress/")
        .or_else(|| path.strip_prefix("/var/www/html/"))
        .unwrap_or(path);
    let path = path.strip_prefix("wp-content/plugins/").unwrap_or(path);
    path.split('/').next().unwrap_or(path).to_string()
}

/// Convert a JSON value to a string suitable for WP-CLI option values.
fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Convert a JSON value to wp config constant string.
fn json_value_to_wp_config_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// Escape single quotes for shell strings.
fn shell_escape(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Ensure a target path is contained within the project root (no path traversal).
fn ensure_path_within(base: &Path, target: &Path) -> Result<(), BlueprintError> {
    let normalized = normalize_path(target);
    let base_normalized = normalize_path(base);
    if !normalized.starts_with(&base_normalized) {
        return Err(BlueprintError::UnsupportedResource(format!(
            "path escapes project root: {}",
            target.display()
        )));
    }
    Ok(())
}

/// Normalize a path by resolving `.` and `..` components without filesystem access.
fn normalize_path(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            other => result.push(other),
        }
    }
    result
}
