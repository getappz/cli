use extism::{convert::Json, host_fn};
use tokio::task as tokio_task;

use crate::wasm::host_functions::helpers::*;
use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;

// ============================================================================
// Context Set
// ============================================================================

host_fn!(pub appz_ctx_set(
    user_data: PluginHostData;
    args: Json<ContextSetInput>
) -> Json<HookResponse> {
    let input = args.into_inner();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    {
        let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_write());
        ctx_guard.set(&input.key, &input.value);
    }

    Ok(success_response(format!("Set '{}' = '{}'", input.key, input.value)))
});

// ============================================================================
// Context Get
// ============================================================================

// Two versions: one that takes key string directly, one that takes ContextGetInput
host_fn!(pub appz_ctx_get(
    user_data: PluginHostData;
    key: String
) -> Json<ContextGetOutput> {
    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    let value = {
        let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_read());
        ctx_guard.get(&key)
    };

    Ok(Json(ContextGetOutput { value }))
});

// ============================================================================
// Context Has
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct HasOutput {
    has: u8, // 1 = true, 0 = false
}

host_fn!(pub appz_ctx_has(
    user_data: PluginHostData;
    key: String
) -> Json<HasOutput> {
    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    let has = {
        let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_read());
        if ctx_guard.contains(&key) { 1u8 } else { 0u8 }
    };

    Ok(Json(HasOutput { has }))
});

// ============================================================================
// Context Add (merge array values)
// ============================================================================

#[derive(Debug, serde::Deserialize)]
struct ContextAddInput {
    key: String,
    values: Vec<String>,
}

host_fn!(pub appz_ctx_add(
    user_data: PluginHostData;
    args: Json<ContextAddInput>
) -> Json<HookResponse> {
    let input = args.into_inner();
    let values_count = input.values.len();
    let values_clone = input.values.clone();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    {
        let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_write());
        // Get existing value if any
        let existing = ctx_guard.get(&input.key);

        // For add(), we merge arrays. If existing value is not an array, we create one.
        // Since context stores strings, we'll join values with commas or store as JSON
        // For simplicity, we'll just append values to the existing string with a separator
        if let Some(existing_val) = existing {
            // If existing value looks like JSON array, parse and merge
            if existing_val.starts_with('[') {
                match serde_json::from_str::<Vec<String>>(&existing_val) {
                    Ok(mut existing_vec) => {
                        existing_vec.extend(values_clone.iter().cloned());
                        let merged = serde_json::to_string(&existing_vec)
                            .unwrap_or_else(|_| format!("{:?}", existing_vec));
                        ctx_guard.set(&input.key, &merged);
                    }
                    Err(_) => {
                        // Not valid JSON, append with separator
                        let merged = format!("{},{}", existing_val, values_clone.join(","));
                        ctx_guard.set(&input.key, &merged);
                    }
                }
            } else {
                // Existing value is not an array, create new array with both
                let new_array = vec![existing_val];
                let merged: Vec<String> = new_array.into_iter().chain(values_clone).collect();
                let merged_str = serde_json::to_string(&merged)
                    .unwrap_or_else(|_| format!("{:?}", merged));
                ctx_guard.set(&input.key, &merged_str);
            }
        } else {
            // No existing value, create new array
            let merged_str = serde_json::to_string(&values_clone)
                .unwrap_or_else(|_| format!("{:?}", values_clone));
            ctx_guard.set(&input.key, &merged_str);
        }
    }

    Ok(success_response(format!("Added {} values to '{}'", values_count, input.key)))
});

// ============================================================================
// Context Parse (template string with {{var}} substitution)
// ============================================================================

host_fn!(pub appz_ctx_parse(
    user_data: PluginHostData;
    args: Json<ContextParseInput>
) -> Json<ContextParseOutput> {
    let input = args.into_inner();

    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    let result = {
        let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_read());
        ctx_guard.parse(&input.template)
    };

    Ok(Json(ContextParseOutput { result }))
});

// ============================================================================
// Context Remove
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct RemoveOutput {
    value: Option<String>,
}

host_fn!(pub appz_ctx_remove(
    user_data: PluginHostData;
    key: String
) -> Json<RemoveOutput> {
    let data = get_host_data(user_data)?;
    let data_guard = data.lock().unwrap();
    let ctx = data_guard.context.clone();

    let value = {
        let ctx_guard = tokio_task::block_in_place(|| ctx.blocking_write());
        ctx_guard.remove(&key)
    };

    Ok(Json(RemoveOutput { value }))
});
