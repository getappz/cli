//! Edit project settings interactively
//!
//! Matches Vercel's edit-project-settings.ts functionality

use detectors::{detect_framework_record, DetectFrameworkRecordOptions, StdFilesystem};
use crate::project::ProjectSettings;
use frameworks::frameworks;
use miette::{miette, Result};
use std::path::Path;
use std::sync::Arc;
use tracing::instrument;
use ui::prompt::{checkbox, confirm, prompt};

/// Setting name mapping
const SETTING_NAMES: &[(&str, &str)] = &[
    ("build_command", "Build Command"),
    ("dev_command", "Development Command"),
    ("install_command", "Install Command"),
    ("output_directory", "Output Directory"),
];

/// Edit project settings interactively
///
/// Detects framework, displays settings, and allows user to modify them.
#[instrument(skip(project_path, local_config_overrides))]
pub async fn edit_project_settings(
    project_path: &Path,
    local_config_overrides: Option<&ProjectSettings>,
    auto_confirm: bool,
) -> Result<ProjectSettings> {
    // Create filesystem detector
    let fs = Arc::new(StdFilesystem::new(Some(project_path.to_path_buf())));

    // Get all available frameworks
    let framework_list: Vec<_> = frameworks().to_vec();

    // Detect framework
    let options = DetectFrameworkRecordOptions { fs, framework_list };

    let detected = detect_framework_record(options)
        .await
        .map_err(|e| miette!("Failed to detect framework: {}", e))?;

    let mut settings = ProjectSettings::default();

    // Apply local config overrides first
    if let Some(overrides) = local_config_overrides {
        if let Some(ref cmd) = overrides.build_command {
            settings.build_command = Some(cmd.clone());
        }
        if let Some(ref cmd) = overrides.dev_command {
            settings.dev_command = Some(cmd.clone());
        }
        if let Some(ref cmd) = overrides.install_command {
            settings.install_command = Some(cmd.clone());
        }
        if let Some(ref dir) = overrides.output_directory {
            settings.output_directory = Some(dir.clone());
        }
        if let Some(ref fw) = overrides.framework {
            settings.framework = Some(fw.clone());
        }
    }

    if let Some((framework, _version, _package_manager)) = detected {
        // Framework detected
        let framework_name = framework.name;
        settings.framework = framework.slug.map(|s| s.to_string());

        // Display detected framework (user-facing output)
        println!("\nAuto-detected Project Settings ({}):", framework_name);

        // Display default settings from framework
        if let Some(fw_settings) = &framework.settings {
            // Build Command
            if settings.build_command.is_none() {
                if let Some(ref build_cmd) = fw_settings.build_command {
                    if let Some(ref value) = build_cmd.value {
                        println!("  - Build Command: {}", value);
                        settings.build_command = Some(value.to_string());
                    } else if let Some(ref placeholder) = build_cmd.placeholder {
                        println!("  - Build Command: {}", placeholder);
                    }
                }
            }

            // Development Command
            if settings.dev_command.is_none() {
                if let Some(ref dev_cmd) = fw_settings.dev_command {
                    if let Some(ref value) = dev_cmd.value {
                        println!("  - Development Command: {}", value);
                        settings.dev_command = Some(value.to_string());
                    } else if let Some(ref placeholder) = dev_cmd.placeholder {
                        println!("  - Development Command: {}", placeholder);
                    }
                }
            }

            // Install Command
            if settings.install_command.is_none() {
                if let Some(ref install_cmd) = fw_settings.install_command {
                    if let Some(ref value) = install_cmd.value {
                        println!("  - Install Command: {}", value);
                        settings.install_command = Some(value.to_string());
                    } else if let Some(ref placeholder) = install_cmd.placeholder {
                        println!("  - Install Command: {}", placeholder);
                    }
                }
            }

            // Output Directory
            if settings.output_directory.is_none() {
                if let Some(ref output_dir) = fw_settings.output_directory {
                    if let Some(ref value) = output_dir.value {
                        println!("  - Output Directory: {}", value);
                        settings.output_directory = Some(value.to_string());
                    } else if let Some(ref placeholder) = output_dir.placeholder {
                        println!("  - Output Directory: {}", placeholder);
                    }
                }
            }
        }
    } else {
        // No framework detected
        println!("\nNo framework detected. Default Project Settings:");
    }

    // Show local config overrides if any
    if let Some(overrides) = local_config_overrides {
        let has_overrides = overrides.build_command.is_some()
            || overrides.dev_command.is_some()
            || overrides.install_command.is_some()
            || overrides.output_directory.is_some()
            || overrides.framework.is_some();

        if has_overrides {
            println!("\nLocal settings detected in appz.json:");
            if let Some(ref cmd) = overrides.build_command {
                println!("  - Build Command: {}", cmd);
            }
            if let Some(ref cmd) = overrides.dev_command {
                println!("  - Development Command: {}", cmd);
            }
            if let Some(ref cmd) = overrides.install_command {
                println!("  - Install Command: {}", cmd);
            }
            if let Some(ref dir) = overrides.output_directory {
                println!("  - Output Directory: {}", dir);
            }
            if let Some(ref fw) = overrides.framework {
                println!("  - Framework: {}", fw);
            }
        }
    }

    // Prompt to modify settings
    if !auto_confirm && confirm("Want to modify these settings?", false)? {
        // Build choices for settings that can be modified
        let mut choices: Vec<(String, String)> = Vec::new();

        // Skip framework, command_for_ignoring_build_step, and install_command
        // Also skip if overridden by local config
        if local_config_overrides.is_none_or(|o| o.build_command.is_none()) {
            choices.push(("Build Command".to_string(), "build_command".to_string()));
        }
        if local_config_overrides.is_none_or(|o| o.dev_command.is_none()) {
            choices.push(("Development Command".to_string(), "dev_command".to_string()));
        }
        if local_config_overrides.is_none_or(|o| o.output_directory.is_none()) {
            choices.push((
                "Output Directory".to_string(),
                "output_directory".to_string(),
            ));
        }

        if !choices.is_empty() {
            let selected = checkbox(
                "Which settings would you like to overwrite (select multiple)?",
                choices.clone(),
            )?;

            for setting_key in selected {
                let setting_name = SETTING_NAMES
                    .iter()
                    .find(|(key, _)| *key == setting_key.as_str())
                    .map(|(_, name)| *name)
                    .unwrap_or_else(|| setting_key.as_str());

                let value = prompt(&format!("What's your {}?", setting_name), None)?;

                match setting_key.as_str() {
                    "build_command" => settings.build_command = Some(value),
                    "dev_command" => settings.dev_command = Some(value),
                    "output_directory" => settings.output_directory = Some(value),
                    _ => {}
                }
            }
        }
    }

    Ok(settings)
}
