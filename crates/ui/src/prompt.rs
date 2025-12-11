//! User input prompts and interactions.

use miette::Result;

/// Prompt for text input.
///
/// # Arguments
/// * `message` - Prompt message
/// * `default` - Optional default value
///
/// # Returns
/// User input string or error
pub fn prompt(message: &str, default: Option<&str>) -> Result<String> {
    use inquire::Text;
    let mut text_prompt = Text::new(message);

    if let Some(default) = default {
        text_prompt = text_prompt.with_default(default);
    }

    text_prompt
        .prompt()
        .map_err(|e| miette::miette!("Prompt failed: {}", e))
}

/// Prompt for confirmation (yes/no).
///
/// # Arguments
/// * `message` - Prompt message
/// * `default` - Default value (true/false)
///
/// # Returns
/// User's boolean choice or error
pub fn confirm(message: &str, default: bool) -> Result<bool> {
    use inquire::Confirm;
    Confirm::new(message)
        .with_default(default)
        .prompt()
        .map_err(|e| miette::miette!("Confirmation failed: {}", e))
}

/// Prompt for selection from a list of options.
///
/// # Arguments
/// * `message` - Prompt message
/// * `options` - Slice of option strings
/// * `default_index` - Optional default selection index
///
/// # Returns
/// Selected option string or error
pub fn choose<'a>(
    message: &str,
    options: &'a [&'a str],
    default_index: Option<usize>,
) -> Result<&'a str> {
    use inquire::Select;
    let mut select = Select::new(message, options.to_vec());

    if let Some(idx) = default_index {
        select = select.with_starting_cursor(idx);
    }

    select
        .prompt()
        .map_err(|e| miette::miette!("Selection failed: {}", e))
}

/// Prompt for multi-select from a list of options.
///
/// # Arguments
/// * `message` - Prompt message
/// * `options` - Slice of option strings
/// * `default_selections` - Optional default selected indices
///
/// # Returns
/// Vector of selected option strings or error
pub fn choose_multiple<'a>(
    message: &str,
    options: &'a [&'a str],
    default_selections: Option<&[usize]>,
) -> Result<Vec<&'a str>> {
    use inquire::MultiSelect;
    let mut multi_select = MultiSelect::new(message, options.to_vec());

    if let Some(selections) = default_selections {
        multi_select = multi_select.with_default(selections);
    }

    multi_select
        .prompt()
        .map_err(|e| miette::miette!("Multi-selection failed: {}", e))
}

/// Prompt for password input (hidden).
///
/// # Arguments
/// * `message` - Prompt message
///
/// # Returns
/// User input string or error
pub fn password(message: &str) -> Result<String> {
    use inquire::Password;
    Password::new(message)
        .prompt()
        .map_err(|e| miette::miette!("Password prompt failed: {}", e))
}

/// Select from typed options, returning the selected value.
///
/// # Arguments
/// * `message` - Prompt message
/// * `options` - Vector of (display_name, value) pairs
/// * `default` - Optional default value to select
///
/// # Returns
/// Selected value of type T or error
pub fn select_with_value<T: Clone + std::fmt::Display>(
    message: &str,
    options: Vec<(String, T)>,
    default: Option<&T>,
) -> Result<T> {
    use inquire::Select;

    let choices: Vec<String> = options.iter().map(|(name, _)| name.clone()).collect();
    let default_index = default.and_then(|d| {
        options.iter().position(|(_, val)| {
            // Compare by string representation for simplicity
            format!("{}", val) == format!("{}", d)
        })
    });

    let mut select = Select::new(message, choices);
    if let Some(idx) = default_index {
        select = select.with_starting_cursor(idx);
    }

    let selected_name = select
        .prompt()
        .map_err(|e| miette::miette!("Selection failed: {}", e))?;

    options
        .into_iter()
        .find(|(name, _)| name == &selected_name)
        .map(|(_, value)| value)
        .ok_or_else(|| miette::miette!("Selected option not found"))
}

/// Multi-select checkbox prompt with typed values.
///
/// # Arguments
/// * `message` - Prompt message
/// * `options` - Vector of (display_name, value) pairs
///
/// # Returns
/// Vector of selected values of type T or error
pub fn checkbox<T: Clone + std::fmt::Display>(
    message: &str,
    options: Vec<(String, T)>,
) -> Result<Vec<T>> {
    use inquire::MultiSelect;

    let choices: Vec<String> = options.iter().map(|(name, _)| name.clone()).collect();

    let selected = MultiSelect::new(message, choices)
        .prompt()
        .map_err(|e| miette::miette!("Multi-selection failed: {}", e))?;

    let mut results = Vec::new();
    for selected_name in selected {
        if let Some((_, value)) = options.iter().find(|(name, _)| name == &selected_name) {
            results.push(value.clone());
        }
    }

    Ok(results)
}

/// Text input with validation function.
///
/// # Arguments
/// * `message` - Prompt message
/// * `default` - Optional default value
/// * `validator` - Function that returns None if valid, Some(error_msg) if invalid
///
/// # Returns
/// Validated user input string or error
pub fn text_with_validation<F>(message: &str, default: Option<&str>, validator: F) -> Result<String>
where
    F: Fn(&str) -> Result<Option<String>> + Clone + 'static, // Returns None if valid, Some(error_msg) if invalid
{
    use inquire::Text;

    loop {
        let mut prompt = Text::new(message);
        if let Some(default) = default {
            prompt = prompt.with_default(default);
        }

        let validator_clone = validator.clone();
        prompt = prompt.with_validator(move |input: &str| match validator_clone(input) {
            Ok(None) => Ok(inquire::validator::Validation::Valid),
            Ok(Some(err)) => Ok(inquire::validator::Validation::Invalid(
                inquire::validator::ErrorMessage::Custom(err),
            )),
            Err(e) => Ok(inquire::validator::Validation::Invalid(
                inquire::validator::ErrorMessage::Custom(format!("{}", e)),
            )),
        });

        match prompt.prompt() {
            Ok(value) => return Ok(value),
            Err(e) => return Err(miette::miette!("Prompt failed: {}", e)),
        }
    }
}

/// Text input with transformer function for display.
///
/// Note: The transformer parameter is kept for API compatibility but currently not used
/// as inquire's formatter has limitations. The function simply prompts for text input.
///
/// # Arguments
/// * `message` - Prompt message
/// * `default` - Optional default value
/// * `_transformer` - Unused transformer function (kept for API compatibility)
///
/// # Returns
/// User input string or error
pub fn text_with_transformer<F>(
    message: &str,
    default: Option<&str>,
    _transformer: F,
) -> Result<String>
where
    F: Fn(&str) -> String + 'static,
{
    use inquire::Text;

    let mut prompt = Text::new(message);
    if let Some(default) = default {
        prompt = prompt.with_default(default);
    }

    // Note: inquire's formatter is for display only, it doesn't affect the returned value
    // We'll apply the transformer manually after getting the input
    let input = prompt
        .prompt()
        .map_err(|e| miette::miette!("Prompt failed: {}", e))?;

    // The transformer is just for display, so we return the original input
    // (The formatter in inquire is read-only for display purposes)
    Ok(input)
}
