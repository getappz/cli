use extism::{convert::Json, host_fn};
use std::collections::HashMap;

use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

// ============================================================================
// Create Remote Host (stub for future)
// ============================================================================

host_fn!(pub appz_host_create(
    _user_data: PluginHostData;
    _args: Json<HostInput>
) -> Json<HostResponse> {
    // Remote host support is not implemented yet (local-only for now)
    Ok(Json(HostResponse {
        success: false,
        hosts: vec![],
        message: Some("Remote host support is not yet implemented. Use localhost() for local execution.".to_string()),
    }))
});

// ============================================================================
// Create Localhost
// ============================================================================

host_fn!(pub appz_host_localhost(
    _user_data: PluginHostData;
    args: Json<HostInput>
) -> Json<HostResponse> {
    let input = args.into_inner();

    // For local execution, we create localhost host info
    let hosts: Vec<HostInfo> = if input.hostnames.is_empty() {
        // Default localhost
        vec![HostInfo {
            alias: "localhost".to_string(),
            hostname: "localhost".to_string(),
            local: true,
            config: HashMap::new(),
        }]
    } else {
        input.hostnames
            .iter()
            .map(|hostname| HostInfo {
                alias: hostname.clone(),
                hostname: hostname.clone(),
                local: true,
                config: HashMap::new(),
            })
            .collect()
    };

    Ok(Json(HostResponse {
        success: true,
        hosts: hosts.clone(),
        message: Some(format!("Created {} localhost(s)", hosts.len())),
    }))
});

// ============================================================================
// Get Current Host
// ============================================================================

host_fn!(pub appz_host_current(
    _user_data: PluginHostData;
) -> Json<HostInfo> {
    // For local execution, return localhost
    // In future, this could return the actual current host from context

    Ok(Json(HostInfo {
        alias: "localhost".to_string(),
        hostname: "localhost".to_string(),
        local: true,
        config: HashMap::new(),
    }))
});

// ============================================================================
// Select Hosts by Criteria
// ============================================================================

host_fn!(pub appz_host_select(
    _user_data: PluginHostData;
    args: Json<HostSelectInput>
) -> Json<Vec<HostInfo>> {
    let input = args.into_inner();

    // For local execution, selector parsing is simplified
    // In future, this could parse "stage=prod, role=db" style selectors
    // For now, return localhost if selector matches "localhost" or is empty
    if input.selector.is_empty() || input.selector.contains("local") {
        Ok(Json(vec![HostInfo {
            alias: "localhost".to_string(),
            hostname: "localhost".to_string(),
            local: true,
            config: HashMap::new(),
        }]))
    } else {
        // No hosts match the selector
        Ok(Json(vec![]))
    }
});

// ============================================================================
// Get Selected Hosts
// ============================================================================

host_fn!(pub appz_host_selected(
    _user_data: PluginHostData;
) -> Json<Vec<HostInfo>> {
    // For local execution, return localhost
    // In future, this would return hosts selected via CLI
    Ok(Json(vec![HostInfo {
        alias: "localhost".to_string(),
        hostname: "localhost".to_string(),
        local: true,
        config: HashMap::new(),
    }]))
});
