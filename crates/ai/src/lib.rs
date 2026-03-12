//! Shared LLM client abstraction for OpenAI, Anthropic, Groq, Gemini, and Ollama.
//!
//! Provides a unified interface for calling large language models with:
//! - Multi-provider support (OpenAI, Anthropic, Groq, Gemini, Ollama)
//! - Automatic retry with exponential backoff
//! - SSE streaming for real-time progress
//! - Token usage tracking and cost estimation
//! - Response extraction helpers (JSON, code blocks, diffs)

pub mod aisdk_client;
pub mod client;
pub mod error;
pub mod parse;
pub mod skills;
pub mod types;

// Re-export commonly used items at the crate root.
pub use client::{
    call_llm, call_llm_streaming, call_llm_with_retry, call_llm_with_tools, call_llm_with_skills,
};
pub use skills::{build_skills_prompt, create_load_skill_tool, discover_skills, SkillDir, SkillMetadata};
pub use error::{AiError, AiResult};
pub use parse::{extract_and_parse_json, extract_code_block, extract_diff_block, extract_json_block};
pub use types::{ChatMessage, LlmConfig, LlmResponse, RetryPolicy, TokenUsage};

// Re-export aisdk types for defining tools and agents.
pub use aisdk::core::{utils::step_count_is, Tool};
pub use aisdk::macros::tool;
