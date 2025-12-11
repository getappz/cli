use extism::{convert::Json, host_fn};

use crate::wasm::plugin::PluginHostData;
use crate::wasm::types::*;
use ui::prompt::{choose, confirm, prompt};

// ============================================================================
// Ask (prompt user input)
// ============================================================================

host_fn!(pub appz_int_ask(
    _user_data: PluginHostData;
    args: Json<AskInput>
) -> Result<String, Error> {
    let input = args.into_inner();

    // Use inquire for prompting
    match prompt(&input.message, input.default.as_deref()) {
        Ok(value) => Ok(value),
        Err(e) => {
            eprintln!("Error prompting user: {}", e);
            Ok(input.default.unwrap_or_default())
        }
    }
});

// ============================================================================
// Ask Choice
// ============================================================================

host_fn!(pub appz_int_ask_choice(
    _user_data: PluginHostData;
    args: Json<ChoiceInput>
) -> Json<ChoiceOutput> {
    let input = args.into_inner();

    // Convert Vec<String> to &[&str] for choose function
    let options: Vec<&str> = input.choices.iter().map(|s| s.as_str()).collect();

    // Find default index if provided
    let default_idx = input.default.as_ref().and_then(|d| {
        input.choices.iter().position(|s| s == d)
    });

    match choose(&input.message, &options, default_idx) {
        Ok(selected) => {
            if input.multiselect.unwrap_or(false) {
                // Multi-select not fully supported with current prompt API
                // Return single selection for now
                Ok(Json(ChoiceOutput {
                    selected: vec![selected.to_string()],
                }))
            } else {
                Ok(Json(ChoiceOutput {
                    selected: vec![selected.to_string()],
                }))
            }
        }
        Err(e) => {
            eprintln!("Error prompting choice: {}", e);
            // Return default if available
            Ok(Json(ChoiceOutput {
                selected: input.default.map(|d| vec![d]).unwrap_or_default(),
            }))
        }
    }
});

// ============================================================================
// Ask Confirmation
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct ConfirmOutput {
    confirmed: u8, // 1 = yes, 0 = no
}

host_fn!(pub appz_int_ask_confirm(
    _user_data: PluginHostData;
    args: Json<ConfirmInput>
) -> Json<ConfirmOutput> {
    let input = args.into_inner();

    let default = input.default.unwrap_or(false);

    match confirm(&input.message, default) {
        Ok(result) => {
            Ok(Json(ConfirmOutput {
                confirmed: if result { 1 } else { 0 },
            }))
        }
        Err(e) => {
            eprintln!("Error prompting confirmation: {}", e);
            // Return default on error
            Ok(Json(ConfirmOutput {
                confirmed: if default { 1 } else { 0 },
            }))
        }
    }
});

// ============================================================================
// Ask Hidden Response (password input)
// ============================================================================

host_fn!(pub appz_int_ask_hidden(
    _user_data: PluginHostData;
    message: String
) -> Result<String, Error> {
    use inquire::Password;

    match Password::new(&message).prompt() {
        Ok(value) => Ok(value),
        Err(e) => {
            eprintln!("Error prompting hidden input: {}", e);
            Ok(String::new())
        }
    }
});

// ============================================================================
// Input Interface (stub)
// ============================================================================

host_fn!(pub appz_int_input(
    _user_data: PluginHostData;
) -> Json<InputHandle> {
    // Input interface is not implemented yet
    Ok(Json(InputHandle {
        handle: 0,
    }))
});

// ============================================================================
// Output Interface (stub)
// ============================================================================

host_fn!(pub appz_int_output(
    _user_data: PluginHostData;
) -> Json<OutputHandle> {
    // Output interface is not implemented yet
    Ok(Json(OutputHandle {
        handle: 0,
    }))
});
