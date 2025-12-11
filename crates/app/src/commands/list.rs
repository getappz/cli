use crate::session::AppzSession;
use starbase::AppResult;
use task::Task;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn list(session: AppzSession) -> AppResult {
    use std::collections::BTreeMap;

    let registry = session.get_task_registry();
    let mut groups: BTreeMap<String, Vec<(&String, &Task)>> = BTreeMap::new();

    for (name, t) in registry.all() {
        if t.hidden {
            continue;
        }
        let ns = name.split(':').next().unwrap_or("global");
        groups.entry(ns.to_string()).or_default().push((name, t));
    }

    for (ns, items) in groups {
        println!("[{ns}]");
        for (name, t) in items {
            let desc = t.description.as_deref().unwrap_or("");
            println!("  {name}\t{desc}");
        }
        println!();
    }

    Ok(None)
}
