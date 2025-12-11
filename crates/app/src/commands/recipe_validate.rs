use crate::importer;
use crate::session::AppzSession;
use starbase::AppResult;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn recipe_validate(session: AppzSession, path: Option<String>) -> AppResult {
    if let Some(p) = path {
        importer::validate_file(p)?;
    } else if let Ok(env_path) = std::env::var("APPZ_IMPORT") {
        importer::validate_file(env_path)?;
    } else {
        let yml = session.working_dir.join("recipe.yaml");
        let json = session.working_dir.join("recipe.json");
        if yml.exists() {
            importer::validate_file(yml)?;
        } else if json.exists() {
            importer::validate_file(json)?;
        } else {
            return Err(miette::miette!(
                "No recipe.yaml or recipe.json found; use --path or APPZ_IMPORT"
            ));
        }
    }

    Ok(None)
}
