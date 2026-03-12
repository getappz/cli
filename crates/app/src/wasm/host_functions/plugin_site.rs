//! Site host function for downloadable plugins.
//!
//! Allows the site plugin to run the full site-builder pipeline
//! (AI-powered create, redesign, clone, generate-page) on the host.

use extism::{convert::Json, host_fn};

use crate::commands::site::run_site_with_config;
use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

host_fn!(pub appz_psite_run(
    _user_data: PluginHostData;
    args: Json<PluginSiteRunInput>
) -> Json<PluginSiteRunOutput> {
    let input = args.into_inner();

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            run_site_with_config(&input).await
        })
    });

    match result {
        Ok(()) => Ok(Json(PluginSiteRunOutput {
            exit_code: 0,
            message: None,
        })),
        Err(e) => Ok(Json(PluginSiteRunOutput {
            exit_code: 1,
            message: Some(format!("{}", e)),
        })),
    }
});
