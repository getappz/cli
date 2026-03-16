use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::BlueprintError;
use crate::types::*;

/// Executes a parsed Blueprint against a DDEV WordPress project.
pub struct BlueprintExecutor {
    project_path: PathBuf,
}

impl BlueprintExecutor {
    pub fn new(project_path: PathBuf) -> Self {
        Self { project_path }
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
                    self.run_wp_cli(&["plugin", "install", s, "--activate"])?;
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
                // Normalize to major.minor (DDEV expects "8.2" not "8.2.1")
                let php_version = normalize_php_version(php);
                if php_version != "latest" {
                    println!("  Configuring PHP {}...", php_version);
                    self.run_ddev(&[
                        "config",
                        &format!("--php-version={}", php_version),
                    ])?;
                }
            }
            // wp version: handled during init (download specific version).
            // For `blueprint apply`, the WP version is already installed.
            if let Some(ref wp) = versions.wp {
                if wp != "latest" && !wp.starts_with("http") {
                    tracing::info!("Blueprint requests WordPress {}, but version is managed by DDEV/init", wp);
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
                self.run_wp_cli(&args)?;
            }
        }
        Ok(())
    }

    fn apply_top_level_plugins(&self, bp: &Blueprint) -> Result<(), BlueprintError> {
        for plugin in &bp.plugins {
            match plugin {
                PluginShorthand::Slug(slug) => {
                    println!("  Installing plugin: {}...", slug);
                    self.run_wp_cli(&["plugin", "install", slug, "--activate"])?;
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
                    // Auto-login as admin — ensure admin user exists (already done by wp core install)
                    tracing::info!("Blueprint requests auto-login (handled by DDEV)");
                }
                LoginShorthand::Bool(false) => {}
                LoginShorthand::Credentials { username, password } => {
                    let user = username.as_deref().unwrap_or("admin");
                    if let Some(pass) = password {
                        let pass_flag = format!("--user_pass={}", pass);
                        self.run_wp_cli(&["user", "update", user, &pass_flag])?;
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
        // pluginPath is e.g. "wp-content/plugins/gutenberg/gutenberg.php" or just "gutenberg"
        // Extract plugin directory name for wp plugin activate
        let plugin_name = extract_plugin_slug(&s.plugin_path);
        self.run_wp_cli(&["plugin", "activate", &plugin_name])
    }

    fn step_activate_theme(&self, s: &ActivateThemeStep) -> Result<(), BlueprintError> {
        self.run_wp_cli(&["theme", "activate", &s.theme_folder_name])
    }

    fn step_cp(&self, s: &CpStep) -> Result<(), BlueprintError> {
        self.run_ddev(&["exec", "cp", "-r", "--", &s.from_path, &s.to_path])
    }

    fn step_define_wp_config_consts(&self, s: &DefineWpConfigConstsStep) -> Result<(), BlueprintError> {
        for (name, value) in &s.consts {
            let val_str = json_value_to_wp_config_string(value);
            let mut args = vec!["config", "set", name.as_str(), val_str.as_str(), "--type=constant"];
            if value.is_boolean() || value.is_number() {
                args.push("--raw");
            }
            self.run_wp_cli(&args)?;
        }
        Ok(())
    }

    fn step_define_site_url(&self, s: &DefineSiteUrlStep) -> Result<(), BlueprintError> {
        self.run_wp_cli(&["option", "update", "siteurl", &s.site_url])?;
        self.run_wp_cli(&["option", "update", "home", &s.site_url])
    }

    fn step_enable_multisite(&self, _s: &EnableMultisiteStep) -> Result<(), BlueprintError> {
        self.run_wp_cli(&["core", "multisite-convert"])
    }

    fn step_import_theme_starter_content(&self, s: &ImportThemeStarterContentStep) -> Result<(), BlueprintError> {
        if let Some(ref slug) = s.theme_slug {
            // Activate the theme — WordPress auto-imports starter content on activation
            self.run_wp_cli(&["theme", "activate", slug])?;
        }
        // Trigger starter content import via PHP
        self.run_ddev_shell(
            "wp eval 'do_action(\"after_switch_theme\");'"
        )
    }

    fn step_import_wordpress_files(&self, s: &ImportWordPressFilesStep) -> Result<(), BlueprintError> {
        // This step imports WordPress core files from a zip.
        // In DDEV context, WordPress is already installed, so this is mainly for overwriting.
        let resource: Result<FileResource, _> = serde_json::from_value(s.wordpress_files_zip.clone());
        match resource {
            Ok(FileResource::Url { url, .. }) => {
                // Download and extract
                self.run_ddev_shell(&format!(
                    "curl -sL '{}' -o /tmp/wp-files.zip && unzip -o /tmp/wp-files.zip -d /var/www/html/ && rm /tmp/wp-files.zip",
                    shell_escape(&url),
                ))?;
            }
            _ => {
                tracing::warn!("importWordPressFiles: unsupported resource type, skipping");
            }
        }
        Ok(())
    }

    fn step_import_wxr(&self, s: &ImportWxrStep) -> Result<(), BlueprintError> {
        // Ensure wordpress-importer plugin is available
        let _ = self.run_wp_cli(&["plugin", "install", "wordpress-importer", "--activate"]);

        let resource: Result<FileResource, _> = serde_json::from_value(s.file.clone());
        match resource {
            Ok(FileResource::Url { url, .. }) => {
                self.run_ddev_shell(&format!(
                    "curl -sL '{}' -o /tmp/import.wxr && wp import /tmp/import.wxr --authors=create && rm /tmp/import.wxr",
                    shell_escape(&url),
                ))
            }
            Ok(FileResource::Literal { contents, .. }) => {
                if let Some(content_str) = contents.as_str() {
                    // Write WXR to a temp file on host, DDEV will see it
                    let wxr_path = self.project_path.join(".blueprint-import.wxr");
                    std::fs::write(&wxr_path, content_str).map_err(|e| BlueprintError::Io {
                        path: wxr_path.clone(),
                        source: e,
                    })?;
                    let result = self.run_ddev_shell(
                        "wp import /var/www/html/.blueprint-import.wxr --authors=create",
                    );
                    let _ = std::fs::remove_file(&wxr_path);
                    result
                } else {
                    tracing::warn!("importWxr: literal contents is not a string, skipping");
                    Ok(())
                }
            }
            _ => {
                tracing::warn!("importWxr: unsupported resource type, skipping");
                Ok(())
            }
        }
    }

    fn step_install_plugin(&self, s: &InstallPluginStep) -> Result<(), BlueprintError> {
        let activate = s
            .options
            .as_ref()
            .and_then(|o| o.activate)
            .unwrap_or(true);
        let overwrite = s.if_already_installed.as_deref() == Some("overwrite");

        // Try pluginData first, fall back to deprecated pluginZipFile
        if let Some(resource_data) = &s.plugin_data {
            match resource_data {
                ResourceData::File(resource) => {
                    self.install_plugin_from_resource(resource, activate)?;
                    if overwrite {
                        // --force flag would have been added
                    }
                }
                ResourceData::Directory(_dir) => {
                    tracing::warn!("installPlugin: directory resources are not yet supported in DDEV mode, skipping");
                }
                ResourceData::Raw(v) => {
                    // Try parsing as FileResource
                    if let Ok(resource) = serde_json::from_value::<FileResource>(v.clone()) {
                        self.install_plugin_from_resource(&resource, activate)?;
                    } else {
                        tracing::warn!("installPlugin: unrecognized pluginData format, skipping");
                    }
                }
            }
        } else if let Some(ref zip_val) = s.plugin_zip_file {
            // Deprecated pluginZipFile
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
                    tracing::warn!("installTheme: directory resources are not yet supported in DDEV mode, skipping");
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
            self.run_ddev_shell("wp eval 'do_action(\"after_switch_theme\");'")?;
        }

        Ok(())
    }

    fn step_login(&self, s: &LoginStep) -> Result<(), BlueprintError> {
        let user = s.username.as_deref().unwrap_or("admin");
        if let Some(ref pass) = s.password {
            let pass_flag = format!("--user_pass={}", pass);
            self.run_wp_cli(&["user", "update", user, &pass_flag])?;
        }
        // In DDEV context, login is handled by the browser session, not the CLI.
        // Setting the password is the meaningful action.
        Ok(())
    }

    fn step_mkdir(&self, s: &MkdirStep) -> Result<(), BlueprintError> {
        self.run_ddev(&["exec", "mkdir", "-p", &s.path])
    }

    fn step_mv(&self, s: &MvStep) -> Result<(), BlueprintError> {
        self.run_ddev(&["exec", "mv", "--", &s.from_path, &s.to_path])
    }

    fn step_request(&self, s: &RequestStep) -> Result<(), BlueprintError> {
        // Best-effort: extract URL from the request object and curl it
        if let Some(url) = s.request.get("url").and_then(|v| v.as_str()) {
            let method = s.request.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
            self.run_ddev_shell(&format!(
                "curl -sS -X {} '{}'",
                shell_escape(method),
                shell_escape(url),
            ))?;
        } else {
            tracing::warn!("request step: no URL found in request object, skipping");
        }
        Ok(())
    }

    fn step_reset_data(&self, _s: &ResetDataStep) -> Result<(), BlueprintError> {
        tracing::warn!("resetData: this will DROP ALL database tables and is destructive");
        self.run_wp_cli(&["db", "reset", "--yes"])
    }

    fn step_rm(&self, s: &RmStep) -> Result<(), BlueprintError> {
        self.run_ddev(&["exec", "rm", "-f", "--", &s.path])
    }

    fn step_rmdir(&self, s: &RmdirStep) -> Result<(), BlueprintError> {
        self.run_ddev(&["exec", "rm", "-rf", "--", &s.path])
    }

    fn step_run_php(&self, s: &RunPhpStep) -> Result<(), BlueprintError> {
        let code = match &s.code {
            PhpCode::String(code) => code.clone(),
            PhpCode::File { content, .. } => content.clone(),
        };
        // Strip leading <?php if present — wp eval doesn't need it
        let code = code.strip_prefix("<?php").unwrap_or(&code).trim().to_string();
        // Use arg list to avoid shell injection — wp eval receives code as a direct argument
        self.run_wp_cli(&["eval", &code])
    }

    fn step_run_php_with_options(&self, s: &RunPhpWithOptionsStep) -> Result<(), BlueprintError> {
        // Extract code from options object
        if let Some(code) = s.options.get("code").and_then(|v| v.as_str()) {
            let code = code.strip_prefix("<?php").unwrap_or(code).trim().to_string();
            self.run_wp_cli(&["eval", &code])
        } else {
            tracing::warn!("runPHPWithOptions: no code field found, skipping");
            Ok(())
        }
    }

    fn step_run_wp_installation_wizard(&self, s: &RunWpInstallationWizardStep) -> Result<(), BlueprintError> {
        let opts = s.options.as_ref();
        let user = opts
            .and_then(|o| o.admin_username.as_deref())
            .unwrap_or("admin");
        let pass = opts
            .and_then(|o| o.admin_password.as_deref())
            .unwrap_or("admin");

        // Get DDEV project URL
        let project_name = self
            .project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("wordpress");
        let url_flag = format!("--url=https://{}.ddev.site", project_name);
        let user_flag = format!("--admin_user={}", user);
        let pass_flag = format!("--admin_password={}", pass);

        self.run_wp_cli(&[
            "core", "install",
            &url_flag, "--title=WordPress",
            &user_flag, &pass_flag,
            "--admin_email=admin@example.com", "--skip-email",
        ])
    }

    fn step_run_sql(&self, s: &RunSqlStep) -> Result<(), BlueprintError> {
        // sql can be a FileResource (URL) or inline literal
        let resource: Result<FileResource, _> = serde_json::from_value(s.sql.clone());
        match resource {
            Ok(FileResource::Url { url, .. }) => {
                self.run_ddev_shell(&format!(
                    "curl -sL '{}' | wp db query",
                    shell_escape(&url),
                ))
            }
            Ok(FileResource::Literal { contents, .. }) => {
                if let Some(sql) = contents.as_str() {
                    self.run_ddev_sql(sql)
                } else {
                    tracing::warn!("runSql: literal contents is not a string, skipping");
                    Ok(())
                }
            }
            _ => {
                // Try as raw string
                if let Some(sql) = s.sql.as_str() {
                    self.run_ddev_sql(sql)
                } else {
                    tracing::warn!("runSql: unsupported SQL resource type, skipping");
                    Ok(())
                }
            }
        }
    }

    fn step_set_site_language(&self, s: &SetSiteLanguageStep) -> Result<(), BlueprintError> {
        self.run_wp_cli(&["language", "core", "install", &s.language])?;
        self.run_wp_cli(&["site", "switch-language", &s.language])
    }

    fn step_set_site_options(&self, s: &SetSiteOptionsStep) -> Result<(), BlueprintError> {
        self.set_site_options(&s.options)
    }

    fn step_unzip(&self, s: &UnzipStep) -> Result<(), BlueprintError> {
        let resource: Result<FileResource, _> = serde_json::from_value(s.zip_file.clone());
        match resource {
            Ok(FileResource::Url { url, .. }) => {
                self.run_ddev_shell(&format!(
                    "curl -sL '{}' -o /tmp/blueprint-unzip.zip && unzip -o /tmp/blueprint-unzip.zip -d '{}' && rm /tmp/blueprint-unzip.zip",
                    shell_escape(&url),
                    shell_escape(&s.extract_to_path),
                ))
            }
            _ => {
                tracing::warn!("unzip: only URL resources are supported in DDEV mode, skipping");
                Ok(())
            }
        }
    }

    fn step_update_user_meta(&self, s: &UpdateUserMetaStep) -> Result<(), BlueprintError> {
        let user_id_str = s.user_id.to_string();
        for (key, value) in &s.meta {
            let val_str = json_value_to_string(value);
            self.run_wp_cli(&["user", "meta", "update", &user_id_str, key, &val_str])?;
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
                // Download content
                let output = Command::new("curl")
                    .args(["-sL", url])
                    .output()
                    .map_err(|e| BlueprintError::DdevFailed {
                        command: format!("curl -sL {}", url),
                        message: e.to_string(),
                    })?;
                String::from_utf8_lossy(&output.stdout).to_string()
            }
            _ => {
                tracing::warn!("writeFile: unsupported data type, skipping");
                return Ok(());
            }
        };

        // Paths in blueprints are relative to /wordpress/ — map to project root
        let relative_path = s.path.strip_prefix("/wordpress/")
            .or_else(|| s.path.strip_prefix("/var/www/html/"))
            .unwrap_or(&s.path);
        let target = self.project_path.join(relative_path);

        // Path traversal check: ensure target is within project root
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
                // Try to interpret as a file tree object { "path": "content", ... }
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
                // Split command string into args to avoid shell injection
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.is_empty() {
                    return Ok(());
                }
                self.run_wp_cli(&parts)
            }
            WpCliCommand::Args(args) => {
                let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                self.run_wp_cli(&args_refs)
            }
        }
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
        if activate {
            args.push("--activate");
        }
        self.run_wp_cli(&args)
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
        if activate {
            args.push("--activate");
        }
        self.run_wp_cli(&args)
    }

    fn set_site_options(&self, options: &HashMap<String, serde_json::Value>) -> Result<(), BlueprintError> {
        for (key, value) in options {
            let val_str = json_value_to_string(value);
            self.run_wp_cli(&["option", "update", key, &val_str])?;
        }
        Ok(())
    }

    fn write_files_tree(&self, base: &str, tree: &serde_json::Value) -> Result<(), BlueprintError> {
        if let Some(obj) = tree.as_object() {
            for (name, content) in obj {
                let path = format!("{}/{}", base, name);
                if content.is_object() {
                    // Nested directory
                    self.write_files_tree(&path, content)?;
                } else if let Some(text) = content.as_str() {
                    // File content
                    let target = self.project_path.join(&path);
                    // Path traversal check
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

    // -----------------------------------------------------------------------
    // DDEV command execution
    // -----------------------------------------------------------------------

    /// Run `ddev <args>` with arguments passed safely (no shell interpolation).
    fn run_ddev(&self, args: &[&str]) -> Result<(), BlueprintError> {
        let status = Command::new("ddev")
            .args(args)
            .current_dir(&self.project_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| BlueprintError::DdevFailed {
                command: format!("ddev {}", args.join(" ")),
                message: e.to_string(),
            })?;

        if !status.success() {
            return Err(BlueprintError::DdevFailed {
                command: format!("ddev {}", args.join(" ")),
                message: format!("exit code: {}", status.code().unwrap_or(-1)),
            });
        }
        Ok(())
    }

    /// Run `ddev exec <shell_command>` — for commands that need shell features (pipes, etc).
    fn run_ddev_shell(&self, cmd: &str) -> Result<(), BlueprintError> {
        self.run_ddev(&["exec", "bash", "-c", cmd])
    }

    /// Run `ddev exec wp <args>` with safe argument passing.
    fn run_wp_cli(&self, args: &[&str]) -> Result<(), BlueprintError> {
        let mut ddev_args = vec!["exec", "wp"];
        ddev_args.extend_from_slice(args);
        self.run_ddev(&ddev_args)
    }

    /// Pipe SQL to `ddev mysql` via stdin.
    fn run_ddev_sql(&self, sql: &str) -> Result<(), BlueprintError> {
        let mut child = Command::new("ddev")
            .args(["mysql"])
            .current_dir(&self.project_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| BlueprintError::DdevFailed {
                command: "ddev mysql".to_string(),
                message: e.to_string(),
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(sql.as_bytes()).map_err(|e| BlueprintError::DdevFailed {
                command: "ddev mysql (stdin write)".to_string(),
                message: e.to_string(),
            })?;
        }

        let status = child.wait().map_err(|e| BlueprintError::DdevFailed {
            command: "ddev mysql".to_string(),
            message: e.to_string(),
        })?;

        if !status.success() {
            return Err(BlueprintError::DdevFailed {
                command: "ddev mysql".to_string(),
                message: format!("exit code: {}", status.code().unwrap_or(-1)),
            });
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
    // Handle URL-style versions (Playground allows URLs for wp versions)
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
    // Get the first path component (the plugin directory name)
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

/// Escape single quotes for shell strings (replace ' with '\'' ).
fn shell_escape(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Ensure a target path is contained within the project root (no path traversal).
fn ensure_path_within(base: &Path, target: &Path) -> Result<(), BlueprintError> {
    // Normalize by resolving .. components without requiring the path to exist
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
