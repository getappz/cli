//! Stub host functions for features that are not compiled in.
//!
//! When a plugin-only feature (check, site) is disabled, the heavy
//! host function implementation is replaced by a lightweight stub that returns
//! a clear error message. This lets plugins still load and call the function
//! without crashing — they just get an actionable error instead.
//!
//! Note: The `migrate` feature and its host functions (`appz_pmigrate_run`,
//! `appz_pconvert_run`) have been removed entirely. The ssg-migrator plugin
//! is now self-contained and calls ssg-migrator directly via the Vfs trait.

use extism::{convert::Json, host_fn};

use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

const NOT_AVAILABLE: &str = "This feature is not included in the current CLI build. \
    Rebuild with the appropriate feature flag or install the full CLI.";

#[cfg(not(feature = "check"))]
host_fn!(pub appz_pcheck_run_stub(
    _user_data: PluginHostData;
    _args: Json<PluginCheckRunInput>
) -> Json<PluginCheckRunOutput> {
    Ok(Json(PluginCheckRunOutput {
        exit_code: 1,
        message: Some(NOT_AVAILABLE.to_string()),
    }))
});

#[cfg(not(feature = "site"))]
host_fn!(pub appz_psite_run_stub(
    _user_data: PluginHostData;
    _args: Json<PluginSiteRunInput>
) -> Json<PluginSiteRunOutput> {
    Ok(Json(PluginSiteRunOutput {
        exit_code: 1,
        message: Some(NOT_AVAILABLE.to_string()),
    }))
});
