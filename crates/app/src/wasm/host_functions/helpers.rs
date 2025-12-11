use extism::{convert::Json, Error, UserData};
use std::sync::{Arc, Mutex};

use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::HookResponse;

/// Helper to get and lock PluginHostData with consistent error handling
pub fn get_host_data(
    user_data: UserData<PluginHostData>,
) -> Result<Arc<Mutex<PluginHostData>>, Error> {
    user_data
        .get()
        .map_err(|e| Error::msg(format!("Failed to get user data: {}", e)))
}

/// Helper to qualify a task name with plugin namespace
pub fn qualify_task_name(name: &str, plugin_id: &str) -> String {
    if name.contains(':') {
        format!("{}:{}", plugin_id, name)
    } else {
        format!("{}:{}", plugin_id, name)
    }
}

/// Helper to create success response
pub fn success_response(message: impl Into<String>) -> Json<HookResponse> {
    Json(HookResponse {
        success: true,
        message: Some(message.into()),
    })
}

/// Helper to create failure response
pub fn failure_response(message: impl Into<String>) -> Json<HookResponse> {
    Json(HookResponse {
        success: false,
        message: Some(message.into()),
    })
}

/// Helper to create error response from Error type
pub fn error_response(error: impl std::fmt::Display) -> Json<HookResponse> {
    failure_response(format!("{}", error))
}
