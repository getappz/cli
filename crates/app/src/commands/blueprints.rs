//! `appz blueprints` subcommands.

use init::registry::RegistryClient;
use miette::miette;
use starbase::AppResult;

/// List available blueprints from the registry.
pub async fn list(framework_filter: Option<String>, no_cache: bool) -> AppResult {
    let client = RegistryClient::new();
    let index = client
        .fetch_index(no_cache)
        .await
        .map_err(|e| miette!("Failed to fetch blueprint registry: {}", e))?;

    if let Some(fw) = &framework_filter {
        let entry = index
            .frameworks
            .get(fw.as_str())
            .ok_or_else(|| miette!("Framework '{}' not found in registry", fw))?;

        println!(
            "\n{}",
            ui::theme::style_accent_bold(&format!("{} blueprints", entry.name))
        );
        println!();

        let mut blueprints: Vec<_> = entry.blueprints.iter().collect();
        blueprints.sort_by_key(|(name, _)| if *name == "default" { "" } else { name.as_str() });

        for (name, bp) in &blueprints {
            let cmd = format!("appz init {}/{}", fw, name);
            println!(
                "  {}  {}",
                ui::theme::style_accent_bold(&format!("{:<12}", name)),
                bp.description,
            );
            println!(
                "  {}  {}",
                " ".repeat(12),
                ui::theme::style_muted_italic(&cmd),
            );
        }
        println!();
    } else {
        println!(
            "\n{}",
            ui::theme::style_accent_bold("Available blueprints")
        );
        println!(
            "{}",
            ui::theme::style_muted_italic("Use `appz init <framework>/<blueprint>` to create a project")
        );
        println!();

        let mut frameworks: Vec<_> = index.frameworks.iter().collect();
        frameworks.sort_by_key(|(slug, _)| slug.as_str());

        for (slug, entry) in &frameworks {
            let count = entry.blueprints.len();
            let count_label = if count == 1 {
                "1 blueprint".to_string()
            } else {
                format!("{} blueprints", count)
            };
            println!(
                "  {}  {}",
                ui::theme::style_accent_bold(&format!("{:<14}", entry.name)),
                ui::theme::style_muted_italic(&count_label),
            );

            let mut blueprints: Vec<_> = entry.blueprints.iter().collect();
            blueprints.sort_by_key(|(name, _)| if *name == "default" { "" } else { name.as_str() });

            for (name, bp) in &blueprints {
                println!(
                    "    {:<24} {}",
                    format!("{}/{}", slug, name),
                    ui::theme::style_muted_italic(&bp.description),
                );
            }
            println!();
        }
    }

    Ok(None)
}
