use crate::error::ApiError;
use reqwest_middleware::reqwest::StatusCode;

pub fn map_error(
    status: StatusCode,
    code_opt: Option<&str>,
    message_opt: Option<&str>,
    retry_after: Option<u64>,
) -> ApiError {
    // 429 first
    if status.as_u16() == 429 {
        return ApiError::RateLimited(retry_after);
    }

    let code_lc = code_opt.map(|c| c.to_ascii_lowercase());
    let message = message_opt
        .unwrap_or_else(|| status.canonical_reason().unwrap_or("Unknown"))
        .to_string();

    if let Some(code) = code_lc.as_deref() {
        return match code {
            "invalid_security_code" => ApiError::InvalidSecurityCode(message),
            "unauthorized" | "invalid_token" => ApiError::Unauthorized(message),
            "forbidden" => ApiError::Forbidden(message),
            "not_found" => ApiError::NotFound(message),
            "validation_error" | "bad_request" => ApiError::Validation(message),
            _ => ApiError::ApiError {
                code: status.as_u16(),
                message,
            },
        };
    }

    match status {
        StatusCode::UNAUTHORIZED => ApiError::Unauthorized(message),
        StatusCode::FORBIDDEN => ApiError::Forbidden(message),
        StatusCode::NOT_FOUND => ApiError::NotFound(message),
        StatusCode::UNPROCESSABLE_ENTITY | StatusCode::BAD_REQUEST => ApiError::Validation(message),
        _ => ApiError::ApiError {
            code: status.as_u16(),
            message,
        },
    }
}
