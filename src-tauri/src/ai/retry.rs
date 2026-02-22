use std::time::Duration;

use crate::error::AppError;

const TRANSIENT_STATUSES: [u16; 5] = [429, 500, 502, 503, 529];

fn is_transient(status: reqwest::StatusCode) -> bool {
    TRANSIENT_STATUSES.contains(&status.as_u16())
}

/// Send an HTTP request with automatic retry on transient errors (429, 500, 502, 503, 529).
///
/// `build_request` is called fresh on each attempt because `RequestBuilder` is not cloneable.
/// Retries use exponential backoff: 1s, 2s, 4s, ...
pub async fn send_with_retry(
    build_request: impl Fn() -> reqwest::RequestBuilder,
    provider_name: &str,
    max_retries: u32,
) -> Result<reqwest::Response, AppError> {
    let mut last_error = None;

    for attempt in 0..=max_retries {
        let result = build_request()
            .send()
            .await
            .map_err(|e| AppError::AiProviderError(format!("HTTP request failed: {}", e)));

        let response = match result {
            Ok(resp) => resp,
            Err(e) => {
                // Network-level failure (DNS, connection refused, etc.) — not retryable
                return Err(e);
            }
        };

        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status();
        if is_transient(status) && attempt < max_retries {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "could not read body".into());
            let delay = Duration::from_secs(1 << attempt); // 1s, 2s, 4s
            eprintln!(
                "[{}] Transient error {} (attempt {}/{}), retrying in {:?}: {}",
                provider_name,
                status,
                attempt + 1,
                max_retries + 1,
                delay,
                body.chars().take(200).collect::<String>(),
            );
            last_error = Some(format!("{} API error ({}): {}", provider_name, status, body));
            tokio::time::sleep(delay).await;
            continue;
        }

        // Non-transient error, or final attempt — read body and fail
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "could not read body".into());
        return Err(AppError::AiProviderError(format!(
            "{} API error ({}): {}",
            provider_name, status, body
        )));
    }

    // Should only be reached if all retries exhausted on transient errors
    Err(AppError::AiProviderError(
        last_error.unwrap_or_else(|| format!("{}: all retries exhausted", provider_name)),
    ))
}
