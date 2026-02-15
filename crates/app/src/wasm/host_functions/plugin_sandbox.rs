//! Sandbox exec host functions for downloadable plugins.
//!
//! Allows plugins to execute commands through the sandbox provider,
//! with tool management via mise.

use extism::{convert::Json, host_fn};
use sandbox::config::MiseToolSpec;

use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

// ============================================================================
// Sandbox Exec
// ============================================================================

host_fn!(pub appz_psandbox_exec(
    user_data: PluginHostData;
    args: Json<PluginSandboxExecInput>
) -> Json<PluginSandboxExecOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref sandbox) = data.sandbox else {
        return Ok(Json(PluginSandboxExecOutput {
            success: false,
            stdout: None,
            stderr: None,
            exit_code: None,
            error: Some("Sandbox not available for this plugin".to_string()),
        }));
    };

    let sandbox = sandbox.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            sandbox.exec(&input.command).await
        })
    });

    match result {
        Ok(output) => Ok(Json(PluginSandboxExecOutput {
            success: output.success(),
            stdout: Some(output.stdout()),
            stderr: Some(output.stderr()),
            exit_code: output.exit_code(),
            error: None,
        })),
        Err(e) => Ok(Json(PluginSandboxExecOutput {
            success: false,
            stdout: None,
            stderr: None,
            exit_code: None,
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Sandbox Exec With Tool
// ============================================================================

host_fn!(pub appz_psandbox_exec_with_tool(
    user_data: PluginHostData;
    args: Json<PluginSandboxToolInput>
) -> Json<PluginSandboxExecOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref sandbox) = data.sandbox else {
        return Ok(Json(PluginSandboxExecOutput {
            success: false,
            stdout: None,
            stderr: None,
            exit_code: None,
            error: Some("Sandbox not available for this plugin".to_string()),
        }));
    };

    let sandbox = sandbox.clone();
    let tool_spec = {
        let mut spec = MiseToolSpec::new(&input.tool);
        if let Some(ref version) = input.version {
            spec = spec.with_version(version);
        }
        spec
    };
    let command = input.command.clone().unwrap_or_default();

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            sandbox.exec_with_tool(&tool_spec, &command).await
        })
    });

    match result {
        Ok(output) => Ok(Json(PluginSandboxExecOutput {
            success: output.success(),
            stdout: Some(output.stdout()),
            stderr: Some(output.stderr()),
            exit_code: output.exit_code(),
            error: None,
        })),
        Err(e) => Ok(Json(PluginSandboxExecOutput {
            success: false,
            stdout: None,
            stderr: None,
            exit_code: None,
            error: Some(format!("{}", e)),
        })),
    }
});

// ============================================================================
// Sandbox Ensure Tool
// ============================================================================

host_fn!(pub appz_psandbox_ensure_tool(
    user_data: PluginHostData;
    args: Json<PluginSandboxToolInput>
) -> Json<PluginFsWriteOutput> {
    let input = args.into_inner();
    let data = user_data.get()?;
    let data = data.lock().unwrap();

    let Some(ref sandbox) = data.sandbox else {
        return Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some("Sandbox not available for this plugin".to_string()),
        }));
    };

    let sandbox = sandbox.clone();
    let tool_spec = {
        let mut spec = MiseToolSpec::new(&input.tool);
        if let Some(ref version) = input.version {
            spec = spec.with_version(version);
        }
        spec
    };

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            sandbox.ensure_tool(&tool_spec).await
        })
    });

    match result {
        Ok(()) => Ok(Json(PluginFsWriteOutput {
            success: true,
            error: None,
        })),
        Err(e) => Ok(Json(PluginFsWriteOutput {
            success: false,
            error: Some(format!("{}", e)),
        })),
    }
});
