use extism::{convert::Json, host_fn, Error};
use std::collections::HashMap;
use tokio::task as tokio_task;

use crate::shell::{command_exists, timestamp_utc_iso8601, which};
use crate::wasm::host_functions::helpers::*;
use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

// ============================================================================
// Log Info
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct VoidResponse {}

host_fn!(pub appz_util_info(
    _user_data: PluginHostData;
    message: String
) -> Json<VoidResponse> {
    println!("{}", message);
    Ok(Json(VoidResponse {}))
});

// ============================================================================
// Log Warning
// ============================================================================

host_fn!(pub appz_util_warning(
    _user_data: PluginHostData;
    message: String
) -> Json<VoidResponse> {
    eprintln!("WARNING: {}", message);
    Ok(Json(VoidResponse {}))
});

// ============================================================================
// Write Line
// ============================================================================

host_fn!(pub appz_util_writeln(
    _user_data: PluginHostData;
    message: String
) -> Json<VoidResponse> {
    println!("{}", message);
    Ok(Json(VoidResponse {}))
});

// ============================================================================
// Command Exists
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct CmdExistsOutput {
    exists: u8, // 1 = true, 0 = false
}

host_fn!(pub appz_util_cmd_exists(
    _user_data: PluginHostData;
    command: String
) -> Json<CmdExistsOutput> {
    let exists = command_exists(&command);
    Ok(Json(CmdExistsOutput {
        exists: if exists { 1 } else { 0 },
    }))
});

// ============================================================================
// Command Supports Option
// ============================================================================

host_fn!(pub appz_util_cmd_supports(
    _user_data: PluginHostData;
    args: Json<CommandSupportsInput>
) -> Json<CmdExistsOutput> {
    let input = args.into_inner();

    // Simple check: try running command with --help and see if option appears
    // This is a simplified implementation
    use std::process::Command;

    let output = Command::new(&input.command)
        .arg("--help")
        .output();

    let supports = if let Ok(output) = output {
        let help_text = String::from_utf8_lossy(&output.stdout);
        help_text.contains(&input.option) || String::from_utf8_lossy(&output.stderr).contains(&input.option)
    } else {
        false
    };

    Ok(Json(CmdExistsOutput {
        exists: if supports { 1 } else { 0 },
    }))
});

// ============================================================================
// Which (find command path)
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct WhichOutput {
    path: Option<String>,
}

host_fn!(pub appz_util_which(
    _user_data: PluginHostData;
    name: String
) -> Json<WhichOutput> {
    match which(&name) {
        Ok(path) => Ok(Json(WhichOutput {
            path: Some(path),
        })),
        Err(_) => Ok(Json(WhichOutput {
            path: None,
        })),
    }
});

// ============================================================================
// Remote Environment (returns local env for now)
// ============================================================================

host_fn!(pub appz_util_remote_env(
    user_data: PluginHostData;
) -> Json<HashMap<String, String>> {
    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    // Get environment variables from context
    // Use block_in_place to allow blocking in async context
    let env = {
        let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_read());
        ctx_guard.env().clone()
    };

    // Merge with system environment
    let mut full_env: HashMap<String, String> = std::env::vars().collect();
    full_env.extend(env);

    Ok(Json(full_env))
});

// ============================================================================
// Error (create error object)
// ============================================================================

host_fn!(pub appz_util_error(
    _user_data: PluginHostData;
    message: String
) -> Json<ErrorInfo> {
    Ok(Json(ErrorInfo {
        message,
        code: Some(HostError::InternalError as i32),
    }))
});

// ============================================================================
// Timestamp
// ============================================================================

host_fn!(pub appz_util_timestamp(
    _user_data: PluginHostData;
) -> Result<String, Error> {
    Ok(timestamp_utc_iso8601())
});

// ============================================================================
// Fetch (HTTP request)
// ============================================================================

// Helper for fetch results
fn fetch_error(msg: impl Into<String>) -> Json<FetchOutput> {
    Json(FetchOutput {
        success: false,
        status_code: None,
        body: None,
        headers: None,
        error: Some(msg.into()),
    })
}

host_fn!(pub appz_util_fetch(
    _user_data: PluginHostData;
    args: Json<FetchInput>
) -> Json<FetchOutput> {
    let input = args.into_inner();
    let method = input.method.as_deref().unwrap_or("GET");

    // Use reqwest for HTTP requests
    // Note: Client::new() doesn't return Result, but builder pattern can fail
    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|e| Error::msg(format!("Failed to create HTTP client: {}", e)))?;

    let mut request = match method {
        "GET" => client.get(&input.url),
        "POST" => client.post(&input.url),
        "PUT" => client.put(&input.url),
        "DELETE" => client.delete(&input.url),
        "PATCH" => client.patch(&input.url),
        _ => return Ok(fetch_error(format!("Unsupported HTTP method: {}", method))),
    };

    // Add headers
    if let Some(headers_map) = &input.headers {
        for (key, value) in headers_map {
            request = request.header(key, value);
        }
    }

    // Add body
    if let Some(body) = &input.body {
        request = request.body(body.clone());
    }

    match request.send() {
        Ok(response) => {
            let status = response.status();
            let headers_map: HashMap<String, String> = response
                .headers()
                .iter()
                .map(|(k, v)| {
                    (k.to_string(), v.to_str().unwrap_or("").to_string())
                })
                .collect();

            match response.text() {
                Ok(body_text) => {
                    Ok(Json(FetchOutput {
                        success: status.is_success(),
                        status_code: Some(status.as_u16()),
                        body: Some(body_text),
                        headers: Some(headers_map),
                        error: None,
                    }))
                }
                Err(e) => {
                    Ok(Json(FetchOutput {
                        success: false,
                        status_code: Some(status.as_u16()),
                        body: None,
                        headers: Some(headers_map),
                        error: Some(format!("Failed to read response body: {}", e)),
                    }))
                }
            }
        }
        Err(e) => Ok(fetch_error(format!("HTTP request failed: {}", e)))
    }
});
