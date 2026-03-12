//! aisdk-backed LLM client for OpenAI, Anthropic, Groq, and Google.
//!
//! Uses the [aisdk](https://crates.io/crates/aisdk) crate for provider-agnostic
//! LLM calls. Ollama is not supported by aisdk and uses the legacy client.
//!
//! Supports [tools](https://aisdk.rs/docs/concepts/tools) and
//! [agents](https://aisdk.rs/docs/concepts/agents) via `call_*_with_tools`.

use aisdk::core::{
    capabilities::DynamicModel,
    language_model::{LanguageModelResponseContentType, LanguageModelStreamChunkType},
    messages::{AssistantMessage, Message, SystemMessage, UserMessage},
    utils::step_count_is,
};
use aisdk::providers::{Anthropic, Google, Groq, OpenAI};
use futures::StreamExt;

use crate::error::{AiError, AiResult};
use crate::types::{ChatMessage, LlmConfig, LlmResponse, TokenUsage};

/// Convert our ChatMessage slice to aisdk Messages.
fn to_aisdk_messages(messages: &[ChatMessage]) -> Vec<Message> {
    messages
        .iter()
        .map(|m| match m.role.as_str() {
            "system" => Message::System(SystemMessage::new(m.content.clone())),
            "assistant" => Message::Assistant(AssistantMessage::new(
                LanguageModelResponseContentType::Text(m.content.clone()),
                None,
            )),
            _ => Message::User(UserMessage::new(m.content.clone())),
        })
        .collect()
}

/// Temperature: we use 0.0–1.0, aisdk uses 0–100.
fn temp_to_aisdk(t: f32) -> u32 {
    (t.clamp(0.0, 1.0) * 100.0) as u32
}

/// Map aisdk Usage to our TokenUsage.
fn aisdk_usage_to_ours(u: &aisdk::core::language_model::Usage) -> Option<TokenUsage> {
    let prompt = u.input_tokens?;
    let completion = u.output_tokens?;
    Some(TokenUsage {
        prompt_tokens: prompt as u32,
        completion_tokens: completion as u32,
        total_tokens: (prompt + completion) as u32,
    })
}

/// Map aisdk error to AiError.
fn map_aisdk_error(e: aisdk::error::Error) -> AiError {
    let s = e.to_string();
    AiError::RequestFailed { reason: s }
}

// ---------------------------------------------------------------------------
// Non-streaming: aisdk-based implementations
// ---------------------------------------------------------------------------

pub async fn call_aisdk_openai(config: &LlmConfig, messages: &[ChatMessage]) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "openai".to_string(),
        env_var: "OPENAI_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com");

    let model = OpenAI::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature))
        .build();

    let result = req.generate_text().await.map_err(map_aisdk_error)?;
    let text = result.text().unwrap_or_default();
    let usage = aisdk_usage_to_ours(&result.usage());

    Ok(LlmResponse {
        content: text,
        usage,
    })
}

/// Call OpenAI with tools (agent loop). Runs until final answer or max_steps.
pub async fn call_aisdk_openai_with_tools(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: impl IntoIterator<Item = aisdk::core::Tool>,
    max_steps: u32,
) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "openai".to_string(),
        env_var: "OPENAI_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com");

    let model = OpenAI::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature));

    for tool in tools {
        req = req.with_tool(tool);
    }

    let mut req = req.stop_when(step_count_is(max_steps as usize)).build();

    let result = req.generate_text().await.map_err(map_aisdk_error)?;
    let text = result.text().unwrap_or_default();
    let usage = aisdk_usage_to_ours(&result.usage());

    Ok(LlmResponse {
        content: text,
        usage,
    })
}

pub async fn call_aisdk_anthropic(
    config: &LlmConfig,
    messages: &[ChatMessage],
) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "anthropic".to_string(),
        env_var: "ANTHROPIC_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.anthropic.com");

    let model = Anthropic::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature))
        .build();

    let result = req.generate_text().await.map_err(map_aisdk_error)?;
    let text = result.text().unwrap_or_default();
    let usage = aisdk_usage_to_ours(&result.usage());

    Ok(LlmResponse {
        content: text,
        usage,
    })
}

/// Call Anthropic with tools (agent loop). Runs until final answer or max_steps.
pub async fn call_aisdk_anthropic_with_tools(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: impl IntoIterator<Item = aisdk::core::Tool>,
    max_steps: u32,
) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "anthropic".to_string(),
        env_var: "ANTHROPIC_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.anthropic.com");

    let model = Anthropic::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature));

    for tool in tools {
        req = req.with_tool(tool);
    }

    let mut req = req.stop_when(step_count_is(max_steps as usize)).build();

    let result = req.generate_text().await.map_err(map_aisdk_error)?;
    let text = result.text().unwrap_or_default();
    let usage = aisdk_usage_to_ours(&result.usage());

    Ok(LlmResponse {
        content: text,
        usage,
    })
}

pub async fn call_aisdk_groq(config: &LlmConfig, messages: &[ChatMessage]) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "groq".to_string(),
        env_var: "GROQ_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.groq.com/openai/");

    let model = Groq::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature))
        .build();

    let result = req.generate_text().await.map_err(map_aisdk_error)?;
    let text = result.text().unwrap_or_default();
    let usage = aisdk_usage_to_ours(&result.usage());

    Ok(LlmResponse {
        content: text,
        usage,
    })
}

/// Call Groq with tools (agent loop). Runs until final answer or max_steps.
pub async fn call_aisdk_groq_with_tools(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: impl IntoIterator<Item = aisdk::core::Tool>,
    max_steps: u32,
) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "groq".to_string(),
        env_var: "GROQ_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.groq.com/openai/");

    let model = Groq::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature));

    for tool in tools {
        req = req.with_tool(tool);
    }

    let mut req = req.stop_when(step_count_is(max_steps as usize)).build();

    let result = req.generate_text().await.map_err(map_aisdk_error)?;
    let text = result.text().unwrap_or_default();
    let usage = aisdk_usage_to_ours(&result.usage());

    Ok(LlmResponse {
        content: text,
        usage,
    })
}

pub async fn call_aisdk_google(
    config: &LlmConfig,
    messages: &[ChatMessage],
) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "gemini".to_string(),
        env_var: "GEMINI_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://generativelanguage.googleapis.com");

    let model = Google::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature))
        .build();

    let result = req.generate_text().await.map_err(map_aisdk_error)?;
    let text = result.text().unwrap_or_default();
    let usage = aisdk_usage_to_ours(&result.usage());

    Ok(LlmResponse {
        content: text,
        usage,
    })
}

/// Call Google (Gemini) with tools (agent loop). Runs until final answer or max_steps.
pub async fn call_aisdk_google_with_tools(
    config: &LlmConfig,
    messages: &[ChatMessage],
    tools: impl IntoIterator<Item = aisdk::core::Tool>,
    max_steps: u32,
) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "gemini".to_string(),
        env_var: "GEMINI_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://generativelanguage.googleapis.com");

    let model = Google::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature));

    for tool in tools {
        req = req.with_tool(tool);
    }

    let mut req = req.stop_when(step_count_is(max_steps as usize)).build();

    let result = req.generate_text().await.map_err(map_aisdk_error)?;
    let text = result.text().unwrap_or_default();
    let usage = aisdk_usage_to_ours(&result.usage());

    Ok(LlmResponse {
        content: text,
        usage,
    })
}

// ---------------------------------------------------------------------------
// Streaming: aisdk-based implementations
// ---------------------------------------------------------------------------

pub async fn call_aisdk_openai_streaming(
    config: &LlmConfig,
    messages: &[ChatMessage],
    on_chunk: impl Fn(&str),
) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "openai".to_string(),
        env_var: "OPENAI_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com");

    let model = OpenAI::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature))
        .build();

    let mut stream_resp = req.stream_text().await.map_err(map_aisdk_error)?;
    let mut full_content = String::new();
    let mut usage = TokenUsage::default();

    while let Some(chunk) = stream_resp.stream.next().await {
        match chunk {
            LanguageModelStreamChunkType::Text(text) => {
                full_content.push_str(&text);
                on_chunk(&text);
            }
            LanguageModelStreamChunkType::End(msg) => {
                if let Some(ref u) = msg.usage {
                    if let Some(our_u) = aisdk_usage_to_ours(u) {
                        usage = our_u;
                    }
                }
            }
            _ => {}
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

pub async fn call_aisdk_anthropic_streaming(
    config: &LlmConfig,
    messages: &[ChatMessage],
    on_chunk: impl Fn(&str),
) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "anthropic".to_string(),
        env_var: "ANTHROPIC_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.anthropic.com");

    let model = Anthropic::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature))
        .build();

    let mut stream_resp = req.stream_text().await.map_err(map_aisdk_error)?;
    let mut full_content = String::new();
    let mut usage = TokenUsage::default();

    while let Some(chunk) = stream_resp.stream.next().await {
        match chunk {
            LanguageModelStreamChunkType::Text(text) => {
                full_content.push_str(&text);
                on_chunk(&text);
            }
            LanguageModelStreamChunkType::End(msg) => {
                if let Some(ref u) = msg.usage {
                    if let Some(our_u) = aisdk_usage_to_ours(u) {
                        usage = our_u;
                    }
                }
            }
            _ => {}
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

pub async fn call_aisdk_groq_streaming(
    config: &LlmConfig,
    messages: &[ChatMessage],
    on_chunk: impl Fn(&str),
) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "groq".to_string(),
        env_var: "GROQ_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.groq.com/openai/");

    let model = Groq::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature))
        .build();

    let mut stream_resp = req.stream_text().await.map_err(map_aisdk_error)?;
    let mut full_content = String::new();
    let mut usage = TokenUsage::default();

    while let Some(chunk) = stream_resp.stream.next().await {
        match chunk {
            LanguageModelStreamChunkType::Text(text) => {
                full_content.push_str(&text);
                on_chunk(&text);
            }
            LanguageModelStreamChunkType::End(msg) => {
                if let Some(ref u) = msg.usage {
                    if let Some(our_u) = aisdk_usage_to_ours(u) {
                        usage = our_u;
                    }
                }
            }
            _ => {}
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

pub async fn call_aisdk_google_streaming(
    config: &LlmConfig,
    messages: &[ChatMessage],
    on_chunk: impl Fn(&str),
) -> AiResult<LlmResponse> {
    let api_key = config.api_key.as_deref().ok_or_else(|| AiError::ApiKeyMissing {
        provider: "gemini".to_string(),
        env_var: "GEMINI_API_KEY".to_string(),
    })?;

    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://generativelanguage.googleapis.com");

    let model = Google::<DynamicModel>::builder()
        .model_name(&config.model)
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(map_aisdk_error)?;

    let msgs = to_aisdk_messages(messages);

    let mut req = aisdk::core::LanguageModelRequest::builder()
        .model(model)
        .messages(msgs)
        .temperature(temp_to_aisdk(config.temperature))
        .build();

    let mut stream_resp = req.stream_text().await.map_err(map_aisdk_error)?;
    let mut full_content = String::new();
    let mut usage = TokenUsage::default();

    while let Some(chunk) = stream_resp.stream.next().await {
        match chunk {
            LanguageModelStreamChunkType::Text(text) => {
                full_content.push_str(&text);
                on_chunk(&text);
            }
            LanguageModelStreamChunkType::End(msg) => {
                if let Some(ref u) = msg.usage {
                    if let Some(our_u) = aisdk_usage_to_ours(u) {
                        usage = our_u;
                    }
                }
            }
            _ => {}
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
