//! Core types for the LLM client abstraction.

use std::time::Duration;

// ---------------------------------------------------------------------------
// LLM Configuration
// ---------------------------------------------------------------------------

/// Configuration for a single LLM call.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// Provider name ("openai", "anthropic", "groq", "gemini", "ollama").
    pub provider: String,
    /// Model name (e.g. "gpt-4o", "gpt-4o-mini", "claude-sonnet-4-20250514", "llama-3.3-70b-versatile", "gemini-2.0-flash").
    pub model: String,
    /// API key (from env or config).
    pub api_key: Option<String>,
    /// Base URL for API (for Ollama or custom endpoints).
    pub base_url: Option<String>,
    /// Temperature for generation (0.0 = deterministic, 1.0 = creative).
    pub temperature: f32,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

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

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

/// Response from an LLM call.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The text content of the response.
    pub content: String,
    /// Token usage information (if available from the API).
    pub usage: Option<TokenUsage>,
}

// ---------------------------------------------------------------------------
// Token usage
// ---------------------------------------------------------------------------

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

    /// Accumulate token usage from another call.
    pub fn accumulate(&mut self, other: Option<&TokenUsage>) {
        if let Some(u) = other {
            self.prompt_tokens += u.prompt_tokens;
            self.completion_tokens += u.completion_tokens;
            self.total_tokens += u.total_tokens;
        }
    }
}

/// Rough per-1K-token pricing (input, output) in USD.
pub fn model_pricing(model: &str) -> (f64, f64) {
    // OpenAI
    if model.contains("gpt-4o-mini") {
        (0.00015, 0.0006)
    } else if model.contains("gpt-4o") {
        (0.0025, 0.01)
    } else if model.contains("gpt-4") {
        (0.03, 0.06)
    // Anthropic
    } else if model.contains("claude-3-5-sonnet") || model.contains("claude-sonnet") {
        (0.003, 0.015)
    } else if model.contains("claude-3-5-haiku") || model.contains("claude-haiku") {
        (0.0008, 0.004)
    // Groq (pricing varies; approximations for hosted models)
    } else if model.contains("llama-3.3-70b") {
        (0.00059, 0.00079)
    } else if model.contains("llama-3.1-8b") || model.contains("llama3-8b") {
        (0.00005, 0.00008)
    } else if model.contains("mixtral") {
        (0.00024, 0.00024)
    } else if model.contains("gemma2-9b") {
        (0.0002, 0.0002)
    // Gemini
    } else if model.contains("gemini-2.0-flash") || model.contains("gemini-1.5-flash") {
        (0.000075, 0.0003)
    } else if model.contains("gemini-2.5-pro") || model.contains("gemini-1.5-pro") {
        (0.00125, 0.005)
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
    /// Base delay before the first retry (for transient/server errors).
    pub base_delay: Duration,
    /// Base delay before retrying after a rate-limit (429) response.
    /// The actual delay is `max(rate_limit_base_delay, parsed_retry_after)`.
    pub rate_limit_base_delay: Duration,
    /// Whether to retry on HTTP 429 (rate limit).
    pub retry_on_rate_limit: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 8,
            base_delay: Duration::from_millis(500),
            rate_limit_base_delay: Duration::from_secs(5),
            retry_on_rate_limit: true,
        }
    }
}
