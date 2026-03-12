//! LLM client abstraction — re-exports from the shared `ai` crate.
//!
//! All types and functions are now provided by the `ai` crate.
//! This module re-exports them for backward compatibility within
//! the checker pipeline.

// Re-export all public types from the ai crate.
pub use ai::{
    call_llm, call_llm_streaming, call_llm_with_retry, extract_code_block, extract_diff_block,
    extract_json_block, ChatMessage, LlmConfig, LlmResponse, RetryPolicy, TokenUsage,
};

// Re-export the ai error as a convenience alias.
pub use ai::AiError;

/// Role a model is being used for in the repair pipeline.
///
/// This is checker-specific and stays here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelRole {
    Planner,
    Fixer,
    Verifier,
}
