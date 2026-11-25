use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use super::{DEPLOY_USER_AGENT, parse_api_error};

// A CLI invocation has at most one appzdev target, including deploy --all.
static PENDING: OnceLock<Mutex<Option<ureq::Request>>> = OnceLock::new();

pub fn enable_cli_deploy_interrupts() {
    PENDING.get_or_init(Mutex::default);
}

fn cancel_and_exit() -> ! {
    let mut pending = PENDING.get().unwrap().lock().unwrap();
    if let Some(request) = pending.take() {
        match request.call() {
            Ok(_) => eprintln!("Interrupted: hosted deployment canceled."),
            Err(error) => eprintln!("{}", parse_api_error(error, "AppzDev", "Cancel deployment")),
        }
    }
    std::process::exit(130)
}

pub(super) fn check_interrupted() {
    if PENDING.get().is_some() && command::interrupted() {
        cancel_and_exit();
    }
}

// Hold the slot across negotiation so a signal arriving before the response
// can still cancel the ID the server is about to return.
pub(super) fn negotiating() -> Result<Option<MutexGuard<'static, Option<ureq::Request>>>, String> {
    let Some(slot) = PENDING.get() else {
        return Ok(None);
    };
    static HANDLER: OnceLock<Result<(), String>> = OnceLock::new();
    HANDLER
        .get_or_init(|| command::exec::on_interrupt(|| cancel_and_exit()))
        .clone()?;
    check_interrupted();
    Ok(Some(slot.lock().unwrap()))
}

pub(super) struct PendingUpload;

impl PendingUpload {
    pub(super) fn track(
        mut slot: MutexGuard<'static, Option<ureq::Request>>,
        base: &str,
        id: &str,
        token: Option<&str>,
    ) -> Self {
        let mut request = crate::team::post(&format!("{base}/v0/deployments/{id}/cancel"))
            .set("User-Agent", DEPLOY_USER_AGENT)
            .timeout(Duration::from_secs(5));
        if let Some(token) = token {
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        *slot = Some(request);
        Self
    }
}

impl Drop for PendingUpload {
    fn drop(&mut self) {
        let mut slot = PENDING.get().unwrap().lock().unwrap();
        if command::interrupted() {
            drop(slot);
            cancel_and_exit();
        }
        slot.take();
    }
}
