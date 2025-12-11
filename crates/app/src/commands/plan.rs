use crate::session::AppzSession;
use starbase::AppResult;
use task::Runner;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn plan(session: AppzSession, task: String) -> AppResult {
    let registry = session.get_task_registry();
    let r = Runner::new(&registry);
    let plan = r.plan(&task).map_err(|e| miette::miette!("{}", e))?;

    for n in plan {
        println!("{n}");
    }

    Ok(None)
}
