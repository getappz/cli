//! LLM client abstraction with retry, streaming, and token tracking.
//!
//! Wraps OpenAI, Anthropic, and Ollama API calls behind a unified
//! [`call_llm`] function. Supports:
//! - System + user messages and configurable temperature
//! - Exponential backoff with jitter on transient failures
//! - SSE streaming for real-time progress
//! - Token usage tracking from API responses

use std::time::Duration;

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
    /// Token usage information (if available from the API).
    pub usage: Option<TokenUsage>,
}

/// Token usage from an API response.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// Number of tokens in the prompt.
    pub prompt_tokens: u32,
    /// Number of tokens in the completion.
    pub completion_tokens: u32,
    /// Total tokens used.
    pub total_tokens: u32,
}

impl TokenUsage {
    /// Estimate cost in USD based on model name.
    ///
    /// Uses approximate per-1K-token pricing; not guaranteed accurate.
    pub fn estimate_cost_usd(&self, model: &str) -> f64 {
        let (input_per_1k, output_per_1k) = model_pricing(model);
        let input_cost = (self.prompt_tokens as f64 / 1000.0) * input_per_1k;
        let output_cost = (self.completion_tokens as f64 / 1000.0) * output_per_1k;
        input_cost + output_cost
    }
}

/// Rough per-1K-token pricing (input, output) in USD.
fn model_pricing(model: &str) -> (f64, f64) {
    if model.contains("gpt-4o-mini") {
        (0.00015, 0.0006)
    } else if model.contains("gpt-4o") {
        (0.0025, 0.01)
    } else if model.contains("gpt-4") {
        (0.03, 0.06)
    } else if model.contains("claude-3-5-sonnet") || model.contains("claude-sonnet") {
        (0.003, 0.015)
    } else if model.contains("claude-3-5-haiku") || model.contains("claude-haiku") {
        (0.0008, 0.004)
    } else {
        // Unknown model, return zero (free / local).
        (0.0, 0.0)
    }
}

// ---------------------------------------------------------------------------
// Retry policy
// ---------------------------------------------------------------------------

/// Configuration for retry behaviour on transient failures.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including the first).
    pub max_attempts: u32,
    /// Base delay before the first retry.
    pub base_delay: Duration,
    /// Whether to retry on HTTP 429 (rate limit).
    pub retry_on_rate_limit: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 4,
            base_delay: Duration::from_millis(200),
            retry_on_rate_limit: true,
        }
    }
}

/// Compute backoff duration: `base * 2^(attempt-1) * jitter(0.9..1.1)`.
fn backoff(policy: &RetryPolicy, attempt: u32) -> Duration {
    use rand::Rng;
    let exp = 2u64.saturating_pow(attempt.saturating_sub(1));
    let base_ms = policy.base_delay.as_millis() as u64;
    let delay_ms = base_ms.saturating_mul(exp);
    let jitter: f64 = rand::thread_rng().gen_range(0.9..1.1);
    Duration::from_millis((delay_ms as f64 * jitter) as u64)
}

/// Whether an error is retryable.
fn is_retryable(error: &CheckerError) -> bool {
    match error {
        CheckerError::AiFixFailed { reason } => {
            // Retryable: network errors, 5xx, timeouts.
            reason.contains("status: 5")
                || reason.contains("timed out")
                || reason.contains("connection")
                || reason.contains("API request failed")
                || reason.contains("hyper")
                || reason.contains("dns")
        }
        _ => false,
    }
}

/// Whether a 429 error should be retried.
fn is_rate_limit(error: &CheckerError) -> bool {
    match error {
        CheckerError::AiFixFailed { reason } => {
            reason.contains("status: 429") || reason.contains("rate limit")
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Unified entry point
// ---------------------------------------------------------------------------

/// Call an LLM with the given messages, with automatic retry on transient failures.
pub async fn call_llm(config: &LlmConfig, messages: &[ChatMessage]) -> CheckResult<LlmResponse> {
    call_llm_with_retry(config, messages, &RetryPolicy::default()).await
}

/// Call an LLM with explicit retry policy.
pub async fn call_llm_with_retry(
    config: &LlmConfig,
    messages: &[ChatMessage],
    policy: &RetryPolicy,
) -> CheckResult<LlmResponse> {
    let mut last_error: Option<CheckerError> = None;

    for attempt in 1..=policy.max_attempts {
        match call_llm_once(config, messages).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                let retryable = is_retryable(&e)
                    || (policy.retry_on_rate_limit && is_rate_limit(&e));

                if !retryable || attempt == policy.max_attempts {
                    return Err(e);
                }

                let delay = backoff(policy, attempt);
                let _ = ui::status::warning(&format!(
                    "[LLM] Attempt {}/{} failed, retrying in {}ms: {}",
                    attempt,
                    policy.max_attempts,
                    delay.as_millis(),
                    e
                ));
                tokio::time::sleep(delay).await;
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| CheckerError::AiFixFailed {
        reason: "All retry attempts exhausted".to_string(),
    }))
}

/// Single LLM call without retry.
async fn call_llm_once(config: &LlmConfig, messages: &[ChatMessage]) -> CheckResult<LlmResponse> {
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
// Streaming entry point
// ---------------------------------------------------------------------------

/// Call an LLM with SSE streaming, invoking `on_chunk` for each text delta.
///
/// Returns the fully assembled response. Useful for showing progress
/// during long Fix agent calls.
pub async fn call_llm_streaming(
    config: &LlmConfig,
    messages: &[ChatMessage],
    on_chunk: impl Fn(&str),
) -> CheckResult<LlmResponse> {
    match config.provider.as_str() {
        "openai" => call_openai_streaming(config, messages, on_chunk).await,
        "anthropic" => call_anthropic_streaming(config, messages, on_chunk).await,
        // Ollama doesn't have good SSE support; fall back to non-streaming.
        _ => call_llm(config, messages).await,
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
// Token usage parsing helpers
// ---------------------------------------------------------------------------

/// Parse token usage from an OpenAI-format response.
fn parse_openai_usage(json: &serde_json::Value) -> Option<TokenUsage> {
    let usage = json.get("usage")?;
    Some(TokenUsage {
        prompt_tokens: usage.get("prompt_tokens")?.as_u64()? as u32,
        completion_tokens: usage.get("completion_tokens")?.as_u64()? as u32,
        total_tokens: usage.get("total_tokens")?.as_u64()? as u32,
    })
}

/// Parse token usage from an Anthropic-format response.
fn parse_anthropic_usage(json: &serde_json::Value) -> Option<TokenUsage> {
    let usage = json.get("usage")?;
    let input = usage.get("input_tokens")?.as_u64()? as u32;
    let output = usage.get("output_tokens")?.as_u64()? as u32;
    Some(TokenUsage {
        prompt_tokens: input,
        completion_tokens: output,
        total_tokens: input + output,
    })
}

// ---------------------------------------------------------------------------
// Provider implementations (non-streaming)
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

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com");

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": config.model,
        "messages": msg_values,
        "temperature": config.temperature,
    });

    let response = client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| CheckerError::AiFixFailed {
            reason: format!("API request failed: {}", e),
        })?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        return Err(CheckerError::AiFixFailed {
            reason: format!("OpenAI API status: {} - {}", status, body_text),
        });
    }

    let json: serde_json::Value =
        response
            .json()
            .await
            .map_err(|e| CheckerError::AiFixFailed {
                reason: format!("Failed to parse API response: {}", e),
            })?;

    let usage = parse_openai_usage(&json);

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
        usage,
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

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        return Err(CheckerError::AiFixFailed {
            reason: format!("Anthropic API status: {} - {}", status, body_text),
        });
    }

    let json: serde_json::Value =
        response
            .json()
            .await
            .map_err(|e| CheckerError::AiFixFailed {
                reason: format!("Failed to parse API response: {}", e),
            })?;

    let usage = parse_anthropic_usage(&json);

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
        usage,
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

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        return Err(CheckerError::AiFixFailed {
            reason: format!("Ollama API status: {} - {}", status, body_text),
        });
    }

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

    // Ollama doesn't return token counts in the standard generate endpoint.
    Ok(LlmResponse {
        content: content.to_string(),
        usage: None,
    })
}

// ---------------------------------------------------------------------------
// Provider implementations (SSE streaming)
// ---------------------------------------------------------------------------

async fn call_openai_streaming(
    config: &LlmConfig,
    messages: &[ChatMessage],
    on_chunk: impl Fn(&str),
) -> CheckResult<LlmResponse> {
    use futures::StreamExt;

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

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com");

    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": config.model,
        "messages": msg_values,
        "temperature": config.temperature,
        "stream": true,
        "stream_options": { "include_usage": true },
    });

    let response = client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| CheckerError::AiFixFailed {
            reason: format!("API request failed: {}", e),
        })?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        return Err(CheckerError::AiFixFailed {
            reason: format!("OpenAI streaming API status: {} - {}", status, body_text),
        });
    }

    let mut stream = response.bytes_stream();
    let mut full_content = String::new();
    let mut usage: Option<TokenUsage> = None;
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| CheckerError::AiFixFailed {
            reason: format!("Stream read error: {}", e),
        })?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        // Process complete SSE lines.
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    continue;
                }

                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    // Extract delta content.
                    if let Some(delta) = json
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        full_content.push_str(delta);
                        on_chunk(delta);
                    }

                    // Extract usage from the final chunk.
                    if let Some(u) = parse_openai_usage(&json) {
                        usage = Some(u);
                    }
                }
            }
        }
    }

    Ok(LlmResponse {
        content: full_content,
        usage,
    })
}

async fn call_anthropic_streaming(
    config: &LlmConfig,
    messages: &[ChatMessage],
    on_chunk: impl Fn(&str),
) -> CheckResult<LlmResponse> {
    use futures::StreamExt;

    let api_key = config
        .api_key
        .as_deref()
        .ok_or_else(|| CheckerError::AiFixFailed {
            reason: "ANTHROPIC_API_KEY environment variable not set".to_string(),
        })?;

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
        "stream": true,
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

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        return Err(CheckerError::AiFixFailed {
            reason: format!("Anthropic streaming API status: {} - {}", status, body_text),
        });
    }

    let mut stream = response.bytes_stream();
    let mut full_content = String::new();
    let mut usage = TokenUsage::default();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| CheckerError::AiFixFailed {
            reason: format!("Stream read error: {}", e),
        })?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    let event_type = json
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("");

                    match event_type {
                        "content_block_delta" => {
                            if let Some(text) = json
                                .get("delta")
                                .and_then(|d| d.get("text"))
                                .and_then(|t| t.as_str())
                            {
                                full_content.push_str(text);
                                on_chunk(text);
                            }
                        }
                        "message_start" => {
                            // Extract input token count.
                            if let Some(u) = json.get("message").and_then(|m| m.get("usage")) {
                                if let Some(input) = u.get("input_tokens").and_then(|v| v.as_u64())
                                {
                                    usage.prompt_tokens = input as u32;
                                }
                            }
                        }
                        "message_delta" => {
                            // Extract output token count.
                            if let Some(u) = json.get("usage") {
                                if let Some(output) =
                                    u.get("output_tokens").and_then(|v| v.as_u64())
                                {
                                    usage.completion_tokens = output as u32;
                                    usage.total_tokens =
                                        usage.prompt_tokens + usage.completion_tokens;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let final_usage = if usage.total_tokens > 0 {
        Some(usage)
    } else {
        None
    };

    Ok(LlmResponse {
        content: full_content,
        usage: final_usage,
    })
}
