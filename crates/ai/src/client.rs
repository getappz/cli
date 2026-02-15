//! LLM client with retry, streaming, and token tracking.
//!
//! Wraps OpenAI, Anthropic, Groq, Gemini, and Ollama API calls behind a
//! unified [`call_llm`] function. Supports:
//! - System + user messages and configurable temperature
//! - Exponential backoff with jitter on transient failures
//! - SSE streaming for real-time progress
//! - Token usage tracking from API responses

use std::path::PathBuf;
use std::time::Duration;

use sandbox::SandboxProvider;

use crate::error::{AiError, AiResult};
use crate::skills::{build_skills_prompt, create_load_skill_tool, discover_skills, SkillDir};
use crate::types::{ChatMessage, LlmConfig, LlmResponse, RetryPolicy};

// ---------------------------------------------------------------------------
// Backoff helpers
// ---------------------------------------------------------------------------

/// Compute backoff duration for transient errors: `base * 2^(attempt-1) * jitter`.
fn backoff(policy: &RetryPolicy, attempt: u32) -> Duration {
    use rand::Rng;
    let exp = 2u64.saturating_pow(attempt.saturating_sub(1));
    let base_ms = policy.base_delay.as_millis() as u64;
    let delay_ms = base_ms.saturating_mul(exp);
    let jitter: f64 = rand::thread_rng().gen_range(0.9..1.1);
    Duration::from_millis((delay_ms as f64 * jitter) as u64)
}

/// Compute backoff duration for rate-limit (429) errors.
///
/// Parses "try again in Xs" from the error message when available and
/// uses that as the minimum delay. Otherwise falls back to
/// `rate_limit_base_delay * 2^(attempt-1)`.
fn rate_limit_backoff(policy: &RetryPolicy, attempt: u32, error: &AiError) -> Duration {
    use rand::Rng;

    // Try to extract the suggested wait time from the error body.
    let parsed_wait = match error {
        AiError::RequestFailed { reason } => parse_retry_after_seconds(reason),
        _ => None,
    };

    let delay = if let Some(secs) = parsed_wait {
        // Use the API-suggested wait, plus a small buffer (0.5-1.5s).
        let buffer: f64 = rand::thread_rng().gen_range(0.5..1.5);
        Duration::from_secs_f64(secs + buffer)
    } else {
        // No hint — exponential backoff from the rate-limit base delay.
        let exp = 2u64.saturating_pow(attempt.saturating_sub(1));
        let base_ms = policy.rate_limit_base_delay.as_millis() as u64;
        let delay_ms = base_ms.saturating_mul(exp);
        let jitter: f64 = rand::thread_rng().gen_range(0.9..1.1);
        Duration::from_millis((delay_ms as f64 * jitter) as u64)
    };

    // Clamp: at least rate_limit_base_delay, at most 120s.
    let min = policy.rate_limit_base_delay;
    let max = Duration::from_secs(120);
    delay.max(min).min(max)
}

/// Parse a retry delay hint from an error body.
///
/// Looks for multiple patterns used by different providers:
///   - Groq:   "try again in 22.215s", "try again in 500ms"
///   - Gemini: "Please retry in 43.172084359s."
///   - Gemini JSON: `"retryDelay": "43s"`
fn parse_retry_after_seconds(text: &str) -> Option<f64> {
    let lower = text.to_lowercase();

    // Pattern 1: "try again in Xs" / "retry in Xs" (covers both Groq and Gemini)
    for marker in &["retry in ", "try again in "] {
        if let Some(pos) = lower.find(marker) {
            let start = pos + marker.len();
            let rest = &lower[start..];
            if let Some(result) = parse_duration_str(rest) {
                return Some(result);
            }
        }
    }

    // Pattern 2: "retryDelay": "43s" (Gemini JSON response)
    if let Some(pos) = lower.find("\"retrydelay\"") {
        let rest = &lower[pos..];
        // Find the value after the colon: "retryDelay": "43s"
        if let Some(quote_start) = rest.find(": \"") {
            let val_start = quote_start + 3;
            let val_rest = &rest[val_start..];
            if let Some(quote_end) = val_rest.find('"') {
                let val = &val_rest[..quote_end];
                return parse_duration_str(val);
            }
        }
    }

    None
}

/// Parse a duration string like "43s", "43.172s", "500ms" into seconds.
fn parse_duration_str(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some(ms_end) = s.find("ms") {
        let num_str = s[..ms_end].trim();
        let ms: f64 = num_str.parse().ok()?;
        Some(ms / 1000.0)
    } else if let Some(s_end) = s.find('s') {
        let num_str = s[..s_end].trim();
        num_str.parse().ok()
    } else {
        None
    }
}

/// Whether an error is retryable (transient / server-side).
fn is_retryable(error: &AiError) -> bool {
    match error {
        AiError::RequestFailed { reason } => {
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

/// Whether a rate-limit / quota error should be retried.
fn is_rate_limit(error: &AiError) -> bool {
    match error {
        AiError::RequestFailed { reason } => {
            reason.contains("status: 429")
                || reason.contains("429 Too Many Requests")
                || reason.contains("rate limit")
                || reason.contains("rate_limit")
                || reason.contains("RESOURCE_EXHAUSTED")
                || reason.contains("quota")
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Unified entry points
// ---------------------------------------------------------------------------

/// Call an LLM with the given messages, with automatic retry on transient failures.
pub async fn call_llm(config: &LlmConfig, messages: &[ChatMessage]) -> AiResult<LlmResponse> {
    call_llm_with_retry(config, messages, &RetryPolicy::default()).await
}

/// Call an LLM with tools (agent loop). Runs until the model produces a final answer or `max_steps` is reached.
///
/// Uses the [aisdk](https://aisdk.rs/docs/concepts/agents) agent loop: the model can call tools,
/// observe results, and decide on the next step. Only supported for openai, anthropic, groq, gemini.
/// Ollama does not support tools.
///
/// # Example
/// ```ignore
/// use ai::{call_llm_with_tools, tool, ChatMessage, LlmConfig, Tool};
///
/// #[tool]
/// fn get_weather(location: String) -> Tool {
///     Ok(format!("Sunny in {}", location))
/// }
///
/// let config = LlmConfig { ... };
/// let messages = [ChatMessage::user("What's the weather in Tokyo?")];
/// let response = call_llm_with_tools(&config, &messages, [get_weather()], 10).await?;
/// ```
pub async fn call_llm_with_tools(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: impl IntoIterator<Item = aisdk::core::Tool>,
    max_steps: u32,
) -> AiResult<LlmResponse> {
    if config.provider == "ollama" {
        return Err(AiError::ToolsNotSupported {
            provider: "ollama".to_string(),
        });
    }
    call_llm_with_tools_once(config, messages, tools, max_steps).await
}

/// Default skill directories for a sandbox: project `.agents/skills` and user `~/.appz/skills`.
pub fn skill_directories_for_sandbox(_sandbox: &dyn SandboxProvider) -> Vec<SkillDir> {
    let mut dirs = vec![SkillDir::Relative(PathBuf::from(".agents/skills"))];
    if let Some(appz_dir) = common::user_config::user_appz_dir() {
        let skills_path = appz_dir.join("skills");
        if skills_path.exists() {
            if let Ok(canonical) = skills_path.canonicalize() {
                dirs.push(SkillDir::Allowed(canonical));
            }
        }
    }
    dirs
}

/// Call an LLM with skills support: discovers skills from the sandbox, injects a skills prompt,
/// adds a loadSkill tool, and runs the agent loop.
pub async fn call_llm_with_skills(
    config: &LlmConfig,
    messages: &[ChatMessage],
    sandbox: &dyn SandboxProvider,
    base_tools: impl IntoIterator<Item = aisdk::core::Tool>,
    max_steps: u32,
) -> AiResult<LlmResponse> {
    let skill_dirs = skill_directories_for_sandbox(sandbox);
    let skills = discover_skills(sandbox.fs(), &skill_dirs)?;
    let skills_prompt = build_skills_prompt(&skills);
    let load_skill_tool = create_load_skill_tool(skills, sandbox.fs());

    let mut msgs = messages.to_vec();
    if !skills_prompt.is_empty() {
        if let Some(first) = msgs.iter_mut().find(|m| m.role == "system") {
            first.content = format!("{}{}", skills_prompt, first.content);
        } else {
            msgs.insert(0, ChatMessage::system(skills_prompt));
        }
    }

    let tools: Vec<aisdk::core::Tool> = std::iter::once(load_skill_tool)
        .chain(base_tools)
        .collect();
    call_llm_with_tools(config, &msgs, tools, max_steps).await
}

async fn call_llm_with_tools_once(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: impl IntoIterator<Item = aisdk::core::Tool>,
    max_steps: u32,
) -> AiResult<LlmResponse> {
    match config.provider.as_str() {
        "openai" => crate::aisdk_client::call_aisdk_openai_with_tools(config, messages, tools, max_steps).await,
        "anthropic" => crate::aisdk_client::call_aisdk_anthropic_with_tools(config, messages, tools, max_steps).await,
        "groq" => crate::aisdk_client::call_aisdk_groq_with_tools(config, messages, tools, max_steps).await,
        "gemini" => crate::aisdk_client::call_aisdk_google_with_tools(config, messages, tools, max_steps).await,
        other => Err(AiError::ToolsNotSupported {
            provider: other.to_string(),
        }),
    }
}

/// Call an LLM with explicit retry policy.
pub async fn call_llm_with_retry(
    config: &LlmConfig,
    messages: &[ChatMessage],
    policy: &RetryPolicy,
) -> AiResult<LlmResponse> {
    let mut last_error: Option<AiError> = None;

    for attempt in 1..=policy.max_attempts {
        match call_llm_once(config, messages).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                let rate_limited = is_rate_limit(&e);
                let retryable =
                    is_retryable(&e) || (policy.retry_on_rate_limit && rate_limited);

                if !retryable || attempt == policy.max_attempts {
                    return Err(e);
                }

                // Use rate-limit-aware backoff when we get a 429;
                // otherwise use standard exponential backoff.
                let delay = if rate_limited {
                    rate_limit_backoff(policy, attempt, &e)
                } else {
                    backoff(policy, attempt)
                };

                let _ = ui::status::warning(&format!(
                    "[LLM] Attempt {}/{} failed, retrying in {:.0}s: {}",
                    attempt,
                    policy.max_attempts,
                    delay.as_secs_f64(),
                    e
                ));

                // Show a countdown so the user knows progress is happening.
                {
                    use std::io::Write;
                    let total_secs = delay.as_secs();
                    if total_secs > 3 {
                        for remaining in (1..=total_secs).rev() {
                            eprint!("\r\x1b[K  ⏳ Retrying in {}s...", remaining);
                            let _ = std::io::stderr().flush();
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                        eprint!("\r\x1b[K");
                        let _ = std::io::stderr().flush();
                    } else {
                        tokio::time::sleep(delay).await;
                    }
                }
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| AiError::RequestFailed {
        reason: "All retry attempts exhausted".to_string(),
    }))
}

/// Single LLM call without retry.
async fn call_llm_once(config: &LlmConfig, messages: &[ChatMessage]) -> AiResult<LlmResponse> {
    match config.provider.as_str() {
        "openai" => crate::aisdk_client::call_aisdk_openai(config, messages).await,
        "anthropic" => crate::aisdk_client::call_aisdk_anthropic(config, messages).await,
        "groq" => crate::aisdk_client::call_aisdk_groq(config, messages).await,
        "gemini" => crate::aisdk_client::call_aisdk_google(config, messages).await,
        "ollama" => call_ollama(config, messages).await,
        other => Err(AiError::UnknownProvider {
            provider: other.to_string(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Streaming entry point
// ---------------------------------------------------------------------------

/// Call an LLM with SSE streaming, invoking `on_chunk` for each text delta.
///
/// Returns the fully assembled response. Useful for showing progress
/// during long generation calls.
pub async fn call_llm_streaming(
    config: &LlmConfig,
    messages: &[ChatMessage],
    on_chunk: impl Fn(&str),
) -> AiResult<LlmResponse> {
    match config.provider.as_str() {
        "openai" => crate::aisdk_client::call_aisdk_openai_streaming(config, messages, on_chunk).await,
        "anthropic" => {
            crate::aisdk_client::call_aisdk_anthropic_streaming(config, messages, on_chunk).await
        }
        "groq" => crate::aisdk_client::call_aisdk_groq_streaming(config, messages, on_chunk).await,
        "gemini" => crate::aisdk_client::call_aisdk_google_streaming(config, messages, on_chunk)
            .await,
        // Ollama doesn't have good SSE support; fall back to non-streaming.
        _ => call_llm(config, messages).await,
    }
}

// ---------------------------------------------------------------------------
// Ollama provider (aisdk does not support Ollama; keep manual implementation)
// ---------------------------------------------------------------------------

async fn call_ollama(config: &LlmConfig, messages: &[ChatMessage]) -> AiResult<LlmResponse> {
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
        .map_err(|e| AiError::RequestFailed {
            reason: format!("Ollama API request failed: {}. Is Ollama running?", e),
        })?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        return Err(AiError::RequestFailed {
            reason: format!("Ollama API status: {} - {}", status, body_text),
        });
    }

    let json: serde_json::Value = response.json().await.map_err(|e| AiError::ResponseParse {
        reason: format!("Failed to parse Ollama response: {}", e),
    })?;

    let content = json
        .get("response")
        .and_then(|r| r.as_str())
        .ok_or_else(|| AiError::ResponseParse {
            reason: "Unexpected Ollama response format".to_string(),
        })?;

    // Ollama doesn't return token counts in the standard generate endpoint.
    Ok(LlmResponse {
        content: content.to_string(),
        usage: None,
    })
}

// (OpenAI, Anthropic, Groq, Gemini use aisdk_client - see aisdk_client.rs)
