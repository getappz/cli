use crate::error::ApiError;
use crate::http::error_mapper::map_error;
use reqwest_middleware::reqwest::header::HeaderMap;
use reqwest_middleware::reqwest::{Response, StatusCode};

/// Build ApiError from status, body and headers (for non-JSON response error path).
pub fn error_from_status_body_headers(
    status: StatusCode,
    body: &str,
    headers: &HeaderMap,
) -> ApiError {
    let is_cloudflare = (headers.get("cf-ray").is_some()
        || headers
            .get("server")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_ascii_lowercase().contains("cloudflare"))
            .unwrap_or(false))
        && (body.contains("Attention Required!")
            || body.contains("cf-chl-")
            || body.contains("Just a moment")
            || body.contains("Checking your browser"));

    if is_cloudflare {
        return ApiError::CloudflareBlocked;
    }

    let retry_after_secs = headers
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        let (code_opt, msg_opt) = if let Some(err) = json.get("error") {
            (
                err.get("code").and_then(|v| v.as_str()),
                err.get("message").and_then(|v| v.as_str()),
            )
        } else {
            (
                json.get("code").and_then(|v| v.as_str()),
                json.get("message").and_then(|v| v.as_str()),
            )
        };

        let message = msg_opt.unwrap_or("Bad Request").to_string();
        let detailed_message = if message == "Bad Request" || message.is_empty() {
            if body.len() < 500 {
                format!("{}: {}", message, body)
            } else {
                message
            }
        } else {
            message
        };

        return map_error(status, code_opt, Some(&detailed_message), retry_after_secs);
    }

    ApiError::ApiError {
        code: status.as_u16(),
        message: status
            .canonical_reason()
            .unwrap_or("Unknown")
            .to_string(),
    }
}

pub async fn handle_json<T: serde::de::DeserializeOwned>(
    response: Response,
) -> Result<T, ApiError> {
    let status = response.status();

    if status.is_success() {
        // 204 is not supported for JSON
        if status == StatusCode::NO_CONTENT {
            return Err(ApiError::InvalidResponse(
                "204 No Content - expected JSON body".to_string(),
            ));
        }

        let text = response
            .text()
            .await
            .map_err(|e| ApiError::HttpMiddleware(format!("Failed to read response: {}", e)))?;

        if text.is_empty() {
            return Err(ApiError::InvalidResponse("Empty response body".to_string()));
        }

        tracing::trace!(raw_json = %text, "Raw JSON response received");
        return serde_json::from_str(&text).map_err(ApiError::Json);
    }

    // Error path
    let headers = response.headers().clone();
    let body = response.text().await.unwrap_or_default();

    let body_preview: String = if body.len() > 500 {
        format!("{}... [truncated, {} bytes total]", &body[..500], body.len())
    } else {
        body.clone()
    };
    tracing::debug!(
        status = %status,
        body = %body_preview,
        "API error response"
    );

    // Cloudflare detection (header + HTML markers only)
    let is_cloudflare = (headers.get("cf-ray").is_some()
        || headers
            .get("server")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_ascii_lowercase().contains("cloudflare"))
            .unwrap_or(false))
        && (body.contains("Attention Required!")
            || body.contains("cf-chl-")
            || body.contains("Just a moment")
            || body.contains("Checking your browser"));

    if is_cloudflare {
        return Err(ApiError::CloudflareBlocked);
    }

    let retry_after_secs = headers
        .get("retry-after")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    // Parse JSON error contract and map to ApiError
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
        let (code_opt, msg_opt) = if let Some(err) = json.get("error") {
            (
                err.get("code").and_then(|v| v.as_str()),
                err.get("message").and_then(|v| v.as_str()),
            )
        } else {
            (
                json.get("code").and_then(|v| v.as_str()),
                json.get("message").and_then(|v| v.as_str()),
            )
        };

        // If message is generic like "Bad Request", try to get more details from the response
        let message = msg_opt.unwrap_or("Bad Request");
        let detailed_message = if message == "Bad Request" || message.is_empty() {
            // Try to extract more details from the JSON response
            if let Some(details) = json.get("error").and_then(|e| e.get("details")) {
                format!(
                    "{}: {}",
                    message,
                    serde_json::to_string(details).unwrap_or_default()
                )
            } else if let Some(details) = json.get("details") {
                format!(
                    "{}: {}",
                    message,
                    serde_json::to_string(details).unwrap_or_default()
                )
            } else {
                // Fallback to full body if available and not too long
                if body.len() < 500 {
                    format!("{}: {}", message, body)
                } else {
                    message.to_string()
                }
            }
        } else {
            message.to_string()
        };

        let api_err = map_error(status, code_opt, Some(&detailed_message), retry_after_secs);
        return Err(api_err);
    }

    // Fallback to generic API error
    Err(ApiError::ApiError {
        code: status.as_u16(),
        message: status.canonical_reason().unwrap_or("Unknown").to_string(),
    })
}
