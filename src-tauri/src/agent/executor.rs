use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use serde::Serialize;
use tokio::time::timeout;

use crate::agent::rules::AgentRules;
use crate::agent::validate;
use crate::ai::message::ChatMessage;
use crate::ai::provider::TokenUsage;
use crate::commands::chat::{build_retry_prompt, create_provider};
use crate::config::AppConfig;
use crate::error::AppError;
use crate::python::runner;

const MAX_VALIDATION_ATTEMPTS: u32 = 3;
const EXECUTION_TIMEOUT_SECS: u64 = 30;

/// Everything the executor needs to run and validate code.
pub struct ExecutionContext {
    pub venv_dir: PathBuf,
    pub runner_script: PathBuf,
    pub config: AppConfig,
}

/// Outcome of the full validation loop.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub code: String,
    pub stl_base64: Option<String>,
    pub success: bool,
    pub attempts: u32,
    pub error: Option<String>,
    pub retry_usage: TokenUsage,
}

/// Progress events emitted during the validation loop.
#[derive(Debug, Clone)]
pub enum ValidationEvent {
    Attempt {
        attempt: u32,
        max_attempts: u32,
        message: String,
    },
    Success {
        attempt: u32,
        message: String,
    },
    Failed {
        attempt: u32,
        error_category: String,
        error_message: String,
        will_retry: bool,
    },
}

/// Run CadQuery code through `runner.py` with a timeout, using an isolated temp directory.
///
/// Safe for concurrent execution — each call gets its own temp subdirectory.
pub async fn execute_with_timeout_isolated(
    code: &str,
    venv_dir: &Path,
    runner_script: &Path,
) -> Result<runner::ExecutionResult, String> {
    let code_owned = code.to_string();
    let venv_owned = venv_dir.to_path_buf();
    let runner_owned = runner_script.to_path_buf();

    let result = timeout(
        Duration::from_secs(EXECUTION_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            runner::execute_cadquery_isolated(&venv_owned, &runner_owned, &code_owned)
        }),
    )
    .await;

    match result {
        Err(_) => Err(format!(
            "Execution timed out after {} seconds",
            EXECUTION_TIMEOUT_SECS
        )),
        Ok(Err(join_err)) => Err(format!("Execution task panicked: {}", join_err)),
        Ok(Ok(Err(AppError::CadQueryError(msg)))) => Err(msg),
        Ok(Ok(Err(e))) => Err(e.to_string()),
        Ok(Ok(Ok(exec_result))) => Ok(exec_result),
    }
}

/// Run CadQuery code through `runner.py` with a timeout.
///
/// Returns `Ok(ExecutionResult)` on success, `Err(error_message)` on failure or timeout.
pub async fn execute_with_timeout(
    code: &str,
    venv_dir: &Path,
    runner_script: &Path,
) -> Result<runner::ExecutionResult, String> {
    let code_owned = code.to_string();
    let venv_owned = venv_dir.to_path_buf();
    let runner_owned = runner_script.to_path_buf();

    let result = timeout(
        Duration::from_secs(EXECUTION_TIMEOUT_SECS),
        tokio::task::spawn_blocking(move || {
            runner::execute_cadquery(&venv_owned, &runner_owned, &code_owned)
        }),
    )
    .await;

    match result {
        Err(_) => Err(format!(
            "Execution timed out after {} seconds",
            EXECUTION_TIMEOUT_SECS
        )),
        Ok(Err(join_err)) => Err(format!("Execution task panicked: {}", join_err)),
        Ok(Ok(Err(AppError::CadQueryError(msg)))) => Err(msg),
        Ok(Ok(Err(e))) => Err(e.to_string()),
        Ok(Ok(Ok(exec_result))) => Ok(exec_result),
    }
}

/// Execute code and retry with AI fixes if execution fails.
///
/// The loop runs up to `MAX_VALIDATION_ATTEMPTS` times:
/// 1. Execute the code via `runner.py`
/// 2. If success → return immediately with STL data
/// 3. If failure → classify error, build retry prompt, call AI for a fix, loop
///
/// The `on_event` callback is invoked for each progress event so the caller
/// can forward them to the frontend.
pub async fn validate_and_retry(
    code: String,
    ctx: &ExecutionContext,
    system_prompt: &str,
    on_event: &(dyn Fn(ValidationEvent) + Send + Sync),
) -> Result<ValidationResult, AppError> {
    let mut current_code = code;
    let mut retry_usage = TokenUsage::default();

    for attempt in 1..=MAX_VALIDATION_ATTEMPTS {
        // Emit attempt event
        let message = if attempt == 1 {
            "Validating generated code...".to_string()
        } else {
            format!("Retrying... (attempt {}/{})", attempt, MAX_VALIDATION_ATTEMPTS)
        };
        on_event(ValidationEvent::Attempt {
            attempt,
            max_attempts: MAX_VALIDATION_ATTEMPTS,
            message,
        });

        // Execute
        match execute_with_timeout(&current_code, &ctx.venv_dir, &ctx.runner_script).await {
            Ok(exec_result) => {
                // Success — encode STL as base64
                let stl_base64 =
                    base64::engine::general_purpose::STANDARD.encode(&exec_result.stl_data);
                on_event(ValidationEvent::Success {
                    attempt,
                    message: format!("Code validated successfully on attempt {}.", attempt),
                });
                return Ok(ValidationResult {
                    code: current_code,
                    stl_base64: Some(stl_base64),
                    success: true,
                    attempts: attempt,
                    error: None,
                    retry_usage,
                });
            }
            Err(error_msg) => {
                // Parse and classify the error
                let structured_error = validate::parse_traceback(&error_msg);
                let strategy = validate::get_retry_strategy(&structured_error, attempt);

                let category_str = format!("{:?}", structured_error.category);
                let will_retry = attempt < MAX_VALIDATION_ATTEMPTS;

                on_event(ValidationEvent::Failed {
                    attempt,
                    error_category: category_str,
                    error_message: error_msg.clone(),
                    will_retry,
                });

                // If this was the last attempt, return failure
                if !will_retry {
                    return Ok(ValidationResult {
                        code: current_code,
                        stl_base64: None,
                        success: false,
                        attempts: attempt,
                        error: Some(error_msg),
                        retry_usage,
                    });
                }

                // Look up anti-pattern from agent rules
                let rules = AgentRules::from_preset(
                    ctx.config.agent_rules_preset.as_deref(),
                )
                .ok();
                let anti_pattern = rules.as_ref().and_then(|r| {
                    r.anti_patterns.as_ref().and_then(|patterns| {
                        strategy.matching_anti_pattern.as_ref().and_then(|title| {
                            patterns.iter().find(|p| p.title == *title)
                        })
                    })
                });

                // Build the retry prompt
                let retry_prompt = build_retry_prompt(
                    &current_code,
                    &error_msg,
                    &structured_error,
                    &strategy,
                    anti_pattern,
                );

                // Call AI for a fix (non-streaming)
                let provider = create_provider(&ctx.config)?;
                let messages = vec![
                    ChatMessage {
                        role: "system".to_string(),
                        content: system_prompt.to_string(),
                    },
                    ChatMessage {
                        role: "user".to_string(),
                        content: retry_prompt,
                    },
                ];

                let (ai_response, usage) = provider.complete(&messages, None).await?;
                if let Some(ref u) = usage {
                    retry_usage.add(u);
                }

                // Extract code from the AI response
                match crate::agent::extract::extract_code(&ai_response) {
                    Some(new_code) => {
                        current_code = new_code;
                    }
                    None => {
                        // AI didn't produce extractable code — give up
                        return Ok(ValidationResult {
                            code: current_code,
                            stl_base64: None,
                            success: false,
                            attempts: attempt,
                            error: Some(
                                "AI retry did not produce extractable code".to_string(),
                            ),
                            retry_usage,
                        });
                    }
                }
            }
        }
    }

    // Should not reach here, but just in case
    Ok(ValidationResult {
        code: current_code,
        stl_base64: None,
        success: false,
        attempts: MAX_VALIDATION_ATTEMPTS,
        error: Some("Validation loop exhausted".to_string()),
        retry_usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result_serialization() {
        let result = ValidationResult {
            code: "import cadquery as cq\nresult = cq.Workplane('XY').box(10,10,10)".to_string(),
            stl_base64: Some("c3RsZGF0YQ==".to_string()),
            success: true,
            attempts: 1,
            error: None,
            retry_usage: TokenUsage::default(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"attempts\":1"));
        assert!(json.contains("stl_base64"));
    }

    #[test]
    fn test_validation_result_failure_serialization() {
        let result = ValidationResult {
            code: "bad code".to_string(),
            stl_base64: None,
            success: false,
            attempts: 3,
            error: Some("execution failed".to_string()),
            retry_usage: TokenUsage {
                input_tokens: 500,
                output_tokens: 200,
            },
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"attempts\":3"));
        assert!(json.contains("execution failed"));
    }

    #[test]
    fn test_execution_context_creation() {
        let ctx = ExecutionContext {
            venv_dir: PathBuf::from("/tmp/venv"),
            runner_script: PathBuf::from("/tmp/runner.py"),
            config: AppConfig::default(),
        };
        assert_eq!(ctx.venv_dir, PathBuf::from("/tmp/venv"));
        assert_eq!(ctx.runner_script, PathBuf::from("/tmp/runner.py"));
    }
}
