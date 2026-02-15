//! Check host function for downloadable plugins.
//!
//! Allows the check plugin to run the full checker (Biome, tsc, Ruff, etc.)
//! with optional AI-assisted fixes via the host's sandbox.

use extism::{convert::Json, host_fn};
use std::path::Path;

use crate::commands::check::run_check_with_sandbox;
use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

host_fn!(pub appz_pcheck_run(
    user_data: PluginHostData;
    args: Json<PluginCheckRunInput>
) -> Json<PluginCheckRunOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref sandbox) = data.sandbox else {
        return Ok(Json(PluginCheckRunOutput {
            exit_code: 1,
            message: Some("Sandbox not available for check plugin".to_string()),
        }));
    };

    // Handle --init separately.
    if input.init {
        let project_dir = Path::new(&input.working_dir);
        let _ = ui::status::info("Initializing checker configuration...");
        match checker::init::run_init(project_dir) {
            Ok(created) => {
                let msg = if created.is_empty() {
                    "All config files already exist. No changes made.\nDelete existing config files if you want to regenerate them.".to_string()
                } else {
                    for file in &created {
                        let _ = ui::status::success(&format!("Created {}", file));
                    }
                    format!(
                        "Initialized {} config file(s). Run 'appz check' to start checking.",
                        created.len()
                    )
                };
                return Ok(Json(PluginCheckRunOutput {
                    exit_code: 0,
                    message: Some(msg),
                }));
            }
            Err(e) => {
                return Ok(Json(PluginCheckRunOutput {
                    exit_code: 1,
                    message: Some(format!("{}", e)),
                }));
            }
        }
    }

    let sandbox = sandbox.clone();
    let working_dir = input.working_dir.clone();
    let project_dir = Path::new(&working_dir).to_path_buf();

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            run_check_with_sandbox(
                &project_dir,
                sandbox,
                input.fix,
                input.ai_fix,
                input.strict,
                input.changed,
                input.staged,
                input.format,
                input.json,
                input.checker,
                input.jobs,
                input.max_attempts,
                input.ai_verify,
                input.verbose_ai,
            )
            .await
        })
    });

    match result {
        Ok(exit_code) => Ok(Json(PluginCheckRunOutput {
            exit_code,
            message: None,
        })),
        Err(e) => Ok(Json(PluginCheckRunOutput {
            exit_code: 1,
            message: Some(format!("{}", e)),
        })),
    }
});
