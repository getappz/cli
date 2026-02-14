//! LLM client abstraction.
//!
//! Wraps OpenAI, Anthropic, and Ollama API calls behind a unified
//! [`call_llm`] function. Supports system + user messages and
//! configurable temperature.

use crate::error::{CheckResult, CheckerError};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Role a model is being used for in the repair pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelRole {
    Planner,
    Fixer,
    Verifier,
}

/// Configuration for a single LLM call.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Provider name ("openai", "anthropic", "ollama").
    pub provider: String,
    /// Model name (e.g. "gpt-4o", "gpt-4o-mini", "claude-sonnet-4-20250514").
    pub model: String,
    /// API key (from env or config).
    pub api_key: Option<String>,
    /// Base URL for API (for Ollama or custom endpoints).
    pub base_url: Option<String>,
    /// Temperature for generation (0.0 = deterministic, 1.0 = creative).
    pub temperature: f32,
}

/// A message in a chat completion request.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Role: "system", "user", or "assistant".
    pub role: String,
    /// Message content.
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
}

/// Response from an LLM call.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The text content of the response.
    pub content: String,
}

// ---------------------------------------------------------------------------
// Unified entry point
// ---------------------------------------------------------------------------

/// Call an LLM with the given messages.
pub async fn call_llm(config: &LlmConfig, messages: &[ChatMessage]) -> CheckResult<LlmResponse> {
    match config.provider.as_str() {
        "openai" => call_openai(config, messages).await,
        "anthropic" => call_anthropic(config, messages).await,
        "ollama" => call_ollama(config, messages).await,
        other => Err(CheckerError::AiFixFailed {
            reason: format!(
                "Unknown AI provider: {}. Use openai, anthropic, or ollama.",
                other
            ),
        }),
    }
}

// ---------------------------------------------------------------------------
// Response extraction helpers
// ---------------------------------------------------------------------------

/// Extract a fenced code block from an LLM response.
///
/// Looks for the first ``` ... ``` block and returns its content.
/// If no code fence is found, returns the whole response as-is.
pub fn extract_code_block(response: &str) -> String {
    let lines: Vec<&str> = response.lines().collect();
    let mut in_code_block = false;
    let mut code_lines = Vec::new();

    for line in &lines {
        if line.starts_with("```") {
            if in_code_block {
                break;
            } else {
                in_code_block = true;
                continue;
            }
        }
        if in_code_block {
            code_lines.push(*line);
        }
    }

    if !code_lines.is_empty() {
        return code_lines.join("\n");
    }

    response.to_string()
}

/// Extract a unified diff from an LLM response.
///
/// Looks for content starting with `---` or `diff --git`, or inside a
/// code block tagged as `diff`.
pub fn extract_diff_block(response: &str) -> Option<String> {
    // First try: look for a diff-fenced code block.
    let lines: Vec<&str> = response.lines().collect();
    let mut in_diff_block = false;
    let mut diff_lines = Vec::new();

    for line in &lines {
        if line.starts_with("```diff") || line.starts_with("```patch") {
            in_diff_block = true;
            continue;
        }
        if in_diff_block {
            if line.starts_with("```") {
                break;
            }
            diff_lines.push(*line);
        }
    }

    if !diff_lines.is_empty() {
        return Some(diff_lines.join("\n"));
    }

    // Second try: look for any fenced code block containing diff markers.
    let code = extract_code_block(response);
    if code.contains("---") && code.contains("+++") {
        return Some(code);
    }

    // Third try: look for raw diff content outside of code blocks.
    let mut raw_diff = Vec::new();
    let mut found_header = false;
    for line in &lines {
        if line.starts_with("---") || line.starts_with("diff --git") {
            found_header = true;
        }
        if found_header {
            raw_diff.push(*line);
        }
    }

    if !raw_diff.is_empty() {
        return Some(raw_diff.join("\n"));
    }

    None
}

/// Extract JSON from an LLM response.
///
/// Looks for a JSON code block, or falls back to finding the first
/// `{` ... `}` pair in the response.
pub fn extract_json_block(response: &str) -> Option<String> {
    // Try a json-fenced code block first.
    let lines: Vec<&str> = response.lines().collect();
    let mut in_json_block = false;
    let mut json_lines = Vec::new();

    for line in &lines {
        if line.starts_with("```json") {
            in_json_block = true;
            continue;
        }
        if in_json_block {
            if line.starts_with("```") {
                break;
            }
            json_lines.push(*line);
        }
    }

    if !json_lines.is_empty() {
        return Some(json_lines.join("\n"));
    }

    // Try any code block that looks like JSON.
    let code = extract_code_block(response);
    if code.trim_start().starts_with('{') && code.trim_end().ends_with('}') {
        return Some(code);
    }

    // Try raw JSON in the response.
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            if end > start {
                return Some(response[start..=end].to_string());
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Provider implementations
// ---------------------------------------------------------------------------

async fn call_openai(config: &LlmConfig, messages: &[ChatMessage]) -> CheckResult<LlmResponse> {
    let api_key = config
        .api_key
        .as_deref()
        .ok_or_else(|| CheckerError::AiFixFailed {
            reason: "OPENAI_API_KEY environment variable not set".to_string(),
        })?;

    let msg_values: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content,
            })
        })
        .collect();

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": config.model,
        "messages": msg_values,
        "temperature": config.temperature,
    });

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| CheckerError::AiFixFailed {
            reason: format!("API request failed: {}", e),
        })?;

    let json: serde_json::Value =
        response
            .json()
            .await
            .map_err(|e| CheckerError::AiFixFailed {
                reason: format!("Failed to parse API response: {}", e),
            })?;

    let content = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| CheckerError::AiFixFailed {
            reason: "Unexpected API response format".to_string(),
        })?;

    Ok(LlmResponse {
        content: content.to_string(),
    })
}

async fn call_anthropic(
    config: &LlmConfig,
    messages: &[ChatMessage],
) -> CheckResult<LlmResponse> {
    let api_key = config
        .api_key
        .as_deref()
        .ok_or_else(|| CheckerError::AiFixFailed {
            reason: "ANTHROPIC_API_KEY environment variable not set".to_string(),
        })?;

    // Anthropic uses a separate `system` parameter.
    let system_msg = messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone());

    let user_msgs: Vec<serde_json::Value> = messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| {
            serde_json::json!({
                "role": m.role,
                "content": m.content,
            })
        })
        .collect();

    let mut body = serde_json::json!({
        "model": config.model,
        "max_tokens": 8192,
        "messages": user_msgs,
        "temperature": config.temperature,
    });

    if let Some(sys) = system_msg {
        body["system"] = serde_json::Value::String(sys);
    }

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| CheckerError::AiFixFailed {
            reason: format!("API request failed: {}", e),
        })?;

    let json: serde_json::Value =
        response
            .json()
            .await
            .map_err(|e| CheckerError::AiFixFailed {
                reason: format!("Failed to parse API response: {}", e),
            })?;

    let content = json
        .get("content")
        .and_then(|c| c.get(0))
        .and_then(|b| b.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| CheckerError::AiFixFailed {
            reason: "Unexpected API response format".to_string(),
        })?;

    Ok(LlmResponse {
        content: content.to_string(),
    })
}

async fn call_ollama(config: &LlmConfig, messages: &[ChatMessage]) -> CheckResult<LlmResponse> {
    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("http://localhost:11434");

    // Combine system + user messages into a single prompt for Ollama's
    // /api/generate endpoint.
    let prompt = messages
        .iter()
        .map(|m| {
            if m.role == "system" {
                format!("SYSTEM: {}", m.content)
            } else {
                m.content.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": config.model,
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": config.temperature,
        },
    });

    let response = client
        .post(format!("{}/api/generate", base_url))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| CheckerError::AiFixFailed {
            reason: format!("Ollama API request failed: {}. Is Ollama running?", e),
        })?;

    let json: serde_json::Value =
        response
            .json()
            .await
            .map_err(|e| CheckerError::AiFixFailed {
                reason: format!("Failed to parse Ollama response: {}", e),
            })?;

    let content = json
        .get("response")
        .and_then(|r| r.as_str())
        .ok_or_else(|| CheckerError::AiFixFailed {
            reason: "Unexpected Ollama response format".to_string(),
        })?;

    Ok(LlmResponse {
        content: content.to_string(),
    })
}
