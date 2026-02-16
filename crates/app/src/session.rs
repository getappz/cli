use crate::app::Cli;
use crate::auth;
use crate::context::AppContext;
use crate::importer;
use crate::project::ProjectContext;
use crate::recipe;
use crate::wasm;
use api::{error::ApiError as ApiErrorType, Client};
use async_trait::async_trait;
use starbase::{AppResult, AppSession};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use task::TaskRegistry;
use tracing::debug;

#[derive(Clone)]
pub struct AppzSession {
    pub cli: Cli,
    pub working_dir: PathBuf,

    // Lazy components
    task_registry: OnceLock<Arc<TaskRegistry>>,
    api_client: OnceLock<Arc<Client>>,
    project_context: OnceLock<Option<ProjectContext>>,

    // Performance debugging (APPZ_DEBUG_TIMING=1); Arc for Clone, RwLock for Sync
    timing: Option<std::sync::Arc<std::sync::RwLock<common::timing::TimingDebug>>>,
}

impl AppzSession {
    pub fn new(cli: Cli) -> Self {
        debug!("Creating new application session");

        let timing = if common::timing::TimingDebug::new().enabled() {
            Some(std::sync::Arc::new(std::sync::RwLock::new(
                common::timing::TimingDebug::new(),
            )))
        } else {
            None
        };

        Self {
            working_dir: PathBuf::new(),
            task_registry: OnceLock::new(),
            api_client: OnceLock::new(),
            project_context: OnceLock::new(),
            cli,
            timing,
        }
    }

    pub fn get_task_registry(&self) -> Arc<TaskRegistry> {
        // This should only be called after analyze phase
        self.task_registry.get().unwrap().clone()
    }

    /// Get the application context with all necessary components
    pub fn get_app_context(&self) -> AppContext {
        AppContext::new(
            self.working_dir.clone(),
            self.get_task_registry(),
            self.cli.verbose,
        )
    }

    /// Get the API client (must be called after startup)
    pub fn get_api_client(&self) -> Arc<Client> {
        self.api_client
            .get()
            .expect("API client not initialized - this should be called after startup")
            .clone()
    }

    /// Get project context if available (must be called after analyze)
    pub fn get_project_context(&self) -> Option<&ProjectContext> {
        self.project_context.get().and_then(|ctx| ctx.as_ref())
    }
}

/// Check if a command requires project context (must be linked)
/// Commands that return an error if not linked
/// Note: Build is excluded for now (auth disabled); will be re-enabled later
pub fn requires_project_context(command: &crate::app::Commands) -> bool {
    use crate::app::Commands;
    matches!(command, Commands::Ls)
}

#[async_trait]
impl AppSession for AppzSession {
    /// Setup initial state for the session
    async fn startup(&mut self) -> AppResult {
        if let Some(ref t) = self.timing {
            t.write().unwrap().checkpoint("startup: get_working_dir (pre)");
        }

        // Determine working directory (respects --cwd if provided)
        self.working_dir = crate::systems::startup::get_working_dir(&self.cli)?;

        if let Some(ref t) = self.timing {
            t.write().unwrap().checkpoint("startup: get_working_dir");
        }

        // Resolve token from available sources (CLI > env > auth.json)
        // Do NOT prompt for login here - that happens in analyze() if needed
        let token = auth::resolve_token(&self.cli);

        // Create API client
        let client =
            Client::new().map_err(|e| miette::miette!("Failed to create API client: {}", e))?;

        // Set token on client if we found one
        if let Some(ref token) = token {
            client.set_token(token.clone()).await;
        }

        // Resolve team_id from available sources (--scope > env > auth.json)
        let team_identifier = auth::resolve_team_id(&self.cli);

        // If we have a team identifier, resolve it to team ID and set on client
        if let Some(ref identifier) = team_identifier {
            // If --scope was provided, it may be a team ID or slug, so resolve it
            if self.cli.scope.is_some() {
                // Resolve team identifier (ID or slug) to team ID
                match crate::commands::teams::resolve_team_id(&client, identifier).await {
                    Ok(resolved_team_id) => {
                        client.set_team_id(Some(resolved_team_id)).await;
                    }
                    Err(e) => {
                        // Log warning but don't fail startup - the command will handle the error
                        tracing::warn!(
                            "Failed to resolve scope '{}' to team ID: {}",
                            identifier,
                            e
                        );
                    }
                }
            } else {
                // For env var or auth.json, assume it's already a team ID
                client.set_team_id(Some(identifier.clone())).await;
            }
        }

        // Wrap client in Arc for sharing
        let client_arc = Arc::new(client);

        // Set up unauthorized callback for automatic login
        // This will be called when API requests return Unauthorized errors
        let client_for_callback = Arc::clone(&client_arc);
        client_arc
            .set_unauthorized_handler(Arc::new(move || {
                let client_for_login = Arc::clone(&client_for_callback);
                Box::pin(async move {
                    use ui::status;

                    // Show user-friendly message
                    let _ = status::info("Session expired, logging in...");

                    // Create a temporary unauthenticated client for login
                    let temp_client = api::Client::new().map_err(|e| {
                        ApiErrorType::Middleware(format!("Failed to create API client: {}", e))
                    })?;

                    // Run device flow login
                    let token = auth::device_flow_login(&temp_client)
                        .await
                        .map_err(|e| ApiErrorType::Unauthorized(format!("Login failed: {}", e)))?;

                    // Save token to auth.json
                    let auth_config = auth::AuthConfig::with_token(token.clone());
                    auth::save_auth(&auth_config).map_err(|e| {
                        ApiErrorType::Middleware(format!("Failed to save authentication: {}", e))
                    })?;

                    // Update the client with the new token
                    client_for_login.set_token(token.clone()).await;

                    Ok(token)
                })
            }))
            .await;

        // Store client in session (may be authenticated or unauthenticated)
        let _ = self.api_client.set(client_arc);

        if let Some(ref t) = self.timing {
            t.write().unwrap().checkpoint("startup: full");
        }

        Ok(None)
    }

    /// Analyze the current state and build necessary components
    async fn analyze(&mut self) -> AppResult {
        if let Some(ref t) = self.timing {
            t.write().unwrap().checkpoint("analyze: (pre)");
        }
        // Check if current command requires authentication
        if auth::requires_auth(&self.cli.command) {
            let client = self.get_api_client();

            // Check if token is set on the client
            let has_token = client.get_token().await.is_some();

            if !has_token {
                // Token is missing, prompt for login
                let temp_client = Client::new()
                    .map_err(|e| miette::miette!("Failed to create API client: {}", e))?;

                // Run device flow login
                let token = auth::device_flow_login(&temp_client)
                    .await
                    .map_err(|e| miette::miette!("Login flow failed: {}", e))?;

                // Save token to auth.json
                let auth_config = auth::AuthConfig::with_token(token.clone());
                auth::save_auth(&auth_config)
                    .map_err(|e| miette::miette!("Failed to save authentication token: {}", e))?;

                // Update the existing client with the new token
                client.set_token(token).await;
            }
        }

        // Build task registry
        let mut reg = TaskRegistry::new();

        // Register common recipes
        recipe::register_common(&mut reg);

        // Register tools
        recipe::tools::mise::register_mise_tools(&mut reg);
        recipe::tools::ddev::register_ddev_tools(&mut reg);
        recipe::tools::docker::register_docker_tools(&mut reg);

        // Register recipes
        recipe::laravel::register_laravel(&mut reg);
        recipe::vercel::register_vercel(&mut reg);

        // Import recipe file: prefer APPZ_IMPORT, otherwise auto-detect ./recipe.yaml or ./recipe.json
        if let Ok(path) = std::env::var("APPZ_IMPORT") {
            if let Err(e) = importer::import_file(path, &mut reg) {
                eprintln!("Warning: Failed to import recipe: {}", e);
            }
        } else {
            let yml = self.working_dir.join("recipe.yaml");
            let json = self.working_dir.join("recipe.json");
            if yml.exists() {
                if let Err(e) = importer::import_file(yml, &mut reg) {
                    eprintln!("Warning: Failed to import recipe.yaml: {}", e);
                }
            } else if json.exists() {
                if let Err(e) = importer::import_file(json, &mut reg) {
                    eprintln!("Warning: Failed to import recipe.json: {}", e);
                }
            }
        }

        // Load plugins if specified
        if let Some(plugin_path) = &self.cli.plugin {
            let mut plugin_manager = wasm::PluginManager::new();

            // Auto-generate plugin_id from filename, avoiding conflicts with existing namespaces
            let plugin_id = generate_plugin_id(plugin_path, &reg);

            if let Err(e) = plugin_manager.load_plugin(&mut reg, plugin_id.clone(), plugin_path) {
                eprintln!("Warning: Failed to load plugin: {}", e);
            } else {
                eprintln!(
                    "Loaded plugin '{}' from {}",
                    plugin_id,
                    common::user_config::path_for_display(std::path::Path::new(plugin_path))
                );
            }
        }

        // Store registry in session
        let _ = self.task_registry.set(Arc::new(reg));

        if let Some(ref t) = self.timing {
            t.write().unwrap().checkpoint("analyze: task_registry + recipes");
        }

        // Load project context if command requires it
        // This will automatically link the project if not already linked
        if requires_project_context(&self.cli.command) {
            let client = self.get_api_client();
            match crate::project::ensure_project_link(
                "command",
                &client,
                &self.working_dir,
                false, // Don't auto-confirm, but will link interactively
            )
            .await
            {
                Ok(ctx) => {
                    let _ = self.project_context.set(Some(ctx));
                }
                Err(e) => {
                    // If linking fails, return error (don't proceed without project context)
                    return Err(miette::miette!("Failed to ensure project link: {}", e));
                }
            }
        }

        if let Some(ref t) = self.timing {
            t.write().unwrap().checkpoint("analyze: full");
        }

        Ok(None)
    }

    async fn execute(&mut self) -> AppResult {
        if let Some(ref t) = self.timing {
            t.write().unwrap().checkpoint("execute: (pre)");
        }

        // Check for new version (non-blocking, won't fail command execution)
        if let Err(e) = crate::systems::version_check::check_for_new_version().await {
            debug!("Failed to check for new version: {}", e);
        }

        if let Some(ref t) = self.timing {
            t.write().unwrap().checkpoint("execute: version_check");
        }

        Ok(None)
    }

    async fn shutdown(&mut self) -> AppResult {
        if let Some(ref t) = self.timing {
            t.write().unwrap().checkpoint("shutdown");
            t.read().unwrap().print();
        }
        // Cleanup resources if needed
        Ok(None)
    }
}

/// Generate a unique plugin ID from the file path, avoiding conflicts with existing namespaces
fn generate_plugin_id<P: AsRef<std::path::Path>>(
    plugin_path: P,
    registry: &TaskRegistry,
) -> String {
    // Start with filename stem (e.g., "hello_plugin.wasm" -> "hello_plugin")
    let base_id = plugin_path
        .as_ref()
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("plugin")
        .to_string();

    // Sanitize: replace invalid chars, make lowercase, limit length
    let sanitized: String = base_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_lowercase();

    let sanitized = if sanitized.len() > 50 {
        sanitized[..50].to_string()
    } else {
        sanitized
    };

    // Extract existing namespaces from registry
    let existing_namespaces: std::collections::HashSet<String> = registry
        .all()
        .filter_map(|(task_name, _)| {
            // Extract namespace from task name (part before first ':')
            task_name
                .find(':')
                .map(|colon_pos| task_name[..colon_pos].to_string())
        })
        .collect();

    // Check for conflicts and append suffix if needed
    let mut candidate_id = sanitized.clone();
    let mut counter = 0;

    while existing_namespaces.contains(&candidate_id) {
        counter += 1;
        candidate_id = format!("{}_{}", sanitized, counter);
    }

    candidate_id
}
