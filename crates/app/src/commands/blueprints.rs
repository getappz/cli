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

        println!("{} blueprints:", entry.name);
        for (name, bp) in &entry.blueprints {
            println!("  {} - {}", name, bp.description);
        }
    } else {
        println!("Available blueprints:\n");
        let mut frameworks: Vec<_> = index.frameworks.iter().collect();
        frameworks.sort_by_key(|(slug, _)| slug.as_str());

        for (slug, entry) in frameworks {
            println!("{}:", entry.name);
            for (name, bp) in &entry.blueprints {
                println!("  {}/{} - {}", slug, name, bp.description);
            }
            println!();
        }
    }

    Ok(None)
}
