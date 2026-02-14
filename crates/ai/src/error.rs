//! Provider-agnostic AI error types.

use miette::Diagnostic;
use thiserror::Error;

/// Result type alias for AI operations.
pub type AiResult<T> = Result<T, AiError>;

/// Errors that can occur during AI/LLM operations.
#[derive(Error, Debug, Diagnostic)]
pub enum AiError {
    #[diagnostic(
        code(ai::request_failed),
        help("Check your API key and network connection.\nEnsure the AI provider is accessible.")
    )]
    #[error("AI request failed: {reason}")]
    RequestFailed { reason: String },

    #[diagnostic(
        code(ai::provider_unknown),
        help("Supported providers: openai, anthropic, groq, gemini, ollama.")
    )]
    #[error("Unknown AI provider: {provider}")]
    UnknownProvider { provider: String },

    #[diagnostic(
        code(ai::api_key_missing),
        help("Set {env_var} in your environment or configure it in appz.json.")
    )]
    #[error("API key not set for provider {provider}")]
    ApiKeyMissing { provider: String, env_var: String },

    #[diagnostic(
        code(ai::response_parse),
        help("The AI response was in an unexpected format.")
    )]
    #[error("Failed to parse AI response: {reason}")]
    ResponseParse { reason: String },

    #[diagnostic(
        code(ai::stream_error),
        help("A network error occurred during streaming. Try again.")
    )]
    #[error("Stream error: {reason}")]
    StreamError { reason: String },

    #[diagnostic(
        code(ai::tools_not_supported),
        help("Use openai, anthropic, groq, or gemini for tool/agent support.")
    )]
    #[error("Tools and agents are not supported for provider: {provider}")]
    ToolsNotSupported { provider: String },
}
