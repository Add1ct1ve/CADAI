use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use regex::Regex;
use serde::Serialize;
use tokio::time::timeout;
use uuid::Uuid;

use crate::agent::rules::AgentRules;
use crate::agent::static_validate;
use crate::agent::validate;
use crate::ai::message::ChatMessage;
use crate::ai::provider::TokenUsage;
use crate::commands::chat::{build_retry_prompt, create_provider};
use crate::config::AppConfig;
use crate::error::AppError;
use crate::python::runner;

const EXECUTION_TIMEOUT_SECS: u64 = 30;

/// Everything the executor needs to run and validate code.
pub struct ExecutionContext {
    pub venv_dir: PathBuf,
    pub runner_script: PathBuf,
    pub config: AppConfig,
}

/// Geometry quality report emitted after successful execution.
#[derive(Debug, Clone, Serialize)]
pub struct PostGeometryValidationReport {
    pub watertight: bool,
    pub manifold: bool,
    pub degenerate_faces: u64,
    pub euler_number: i64,
    pub triangle_count: u64,
    pub bbox_ok: bool,
    pub warnings: Vec<String>,
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
    pub static_findings: Vec<String>,
    pub post_geometry_report: Option<PostGeometryValidationReport>,
}

/// Progress events emitted during the validation loop.
#[derive(Debug, Clone)]
pub enum ValidationEvent {
    Attempt {
        attempt: u32,
        max_attempts: u32,
        message: String,
    },
    StaticValidation {
        passed: bool,
        findings: Vec<String>,
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
    PostGeometryValidation {
        report: PostGeometryValidationReport,
    },
}

fn configured_max_attempts(config: &AppConfig) -> u32 {
    config.max_validation_attempts.clamp(1, 8)
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

fn parse_dimensions_from_text(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let re = Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(?:mm|millimeter|millimeters)\b").unwrap();
    for cap in re.captures_iter(text) {
        if let Ok(v) = cap[1].parse::<f64>() {
            out.push(v);
        }
    }

    let compact =
        Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*[x×]\s*(\d+(?:\.\d+)?)\s*[x×]\s*(\d+(?:\.\d+)?)")
            .unwrap();
    for cap in compact.captures_iter(text) {
        for i in 1..=3 {
            if let Ok(v) = cap[i].parse::<f64>() {
                out.push(v);
            }
        }
    }

    out.retain(|v| *v > 0.0 && *v <= 10000.0);
    out
}

fn build_retry_prompt_with_findings(
    code: &str,
    runtime_error: &str,
    static_findings: &[String],
    post_warnings: &[String],
    structured_error: &validate::StructuredError,
    strategy: &validate::RetryStrategy,
    anti_pattern: Option<&crate::agent::rules::AntiPatternEntry>,
) -> String {
    let mut prompt = build_retry_prompt(
        code,
        runtime_error,
        structured_error,
        strategy,
        anti_pattern,
    );

    if !static_findings.is_empty() {
        prompt.push_str("\n\nStatic validator findings to address:\n");
        for finding in static_findings {
            prompt.push_str(&format!("- {}\n", finding));
        }
    }

    if !post_warnings.is_empty() {
        prompt.push_str("\n\nPost-geometry validation findings to address:\n");
        for warning in post_warnings {
            prompt.push_str(&format!("- {}\n", warning));
        }
    }

    prompt
}

fn run_post_geometry_checks(
    code: &str,
    ctx: &ExecutionContext,
    user_request: Option<&str>,
) -> Result<PostGeometryValidationReport, String> {
    let script = crate::commands::find_python_script("manufacturing.py")
        .map_err(|e| format!("cannot find manufacturing.py: {}", e))?;

    let temp_dir = std::env::temp_dir()
        .join("cadai-studio")
        .join(format!("post-check-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("failed to create post-check temp dir: {}", e))?;

    let code_file = temp_dir.join("post_check_code.py");
    std::fs::write(&code_file, code)
        .map_err(|e| format!("failed to write post-check code file: {}", e))?;

    let code_file_s = code_file.to_string_lossy().to_string();
    let args: Vec<&str> = vec!["mesh_check", &code_file_s];

    let script_result = runner::execute_python_script(&ctx.venv_dir, &script, &args)
        .map_err(|e| format!("post-check execution failed: {}", e));

    let _ = std::fs::remove_file(&code_file);
    let _ = std::fs::remove_dir_all(&temp_dir);

    let script_result = script_result?;
    if script_result.exit_code != 0 {
        return Err(format!(
            "post-check returned exit code {}: {}",
            script_result.exit_code, script_result.stderr
        ));
    }

    let parsed: serde_json::Value = serde_json::from_str(script_result.stdout.trim())
        .map_err(|e| format!("failed to parse post-check result: {}", e))?;

    let watertight = parsed["watertight"].as_bool().unwrap_or(false);
    let winding_consistent = parsed["winding_consistent"].as_bool().unwrap_or(false);
    let degenerate_faces = parsed["degenerate_faces"].as_u64().unwrap_or(0);
    let euler_number = parsed["euler_number"].as_i64().unwrap_or(0);
    let triangle_count = parsed["triangle_count"].as_u64().unwrap_or(0);

    let mut warnings: Vec<String> = parsed["issues"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut bbox_ok = true;

    if let (Some(req), Some(bounds_arr)) = (user_request, parsed["bounds"].as_array()) {
        if bounds_arr.len() == 2 {
            let min = bounds_arr[0].as_array();
            let max = bounds_arr[1].as_array();
            if let (Some(min), Some(max)) = (min, max) {
                if min.len() >= 3 && max.len() >= 3 {
                    let dx = max[0].as_f64().unwrap_or(0.0) - min[0].as_f64().unwrap_or(0.0);
                    let dy = max[1].as_f64().unwrap_or(0.0) - min[1].as_f64().unwrap_or(0.0);
                    let dz = max[2].as_f64().unwrap_or(0.0) - min[2].as_f64().unwrap_or(0.0);
                    let bbox_max = dx.max(dy).max(dz).abs();

                    let expected = parse_dimensions_from_text(req);
                    if let Some(expected_max) = expected
                        .iter()
                        .copied()
                        .filter(|v| *v > 0.0)
                        .reduce(f64::max)
                    {
                        if bbox_max > expected_max * 8.0 || bbox_max < expected_max * 0.05 {
                            bbox_ok = false;
                            warnings.push(format!(
                                "Bounding box sanity check failed: max extent {:.2}mm is inconsistent with requested size {:.2}mm",
                                bbox_max, expected_max
                            ));
                        }
                    }
                }
            }
        }
    }

    let manifold = watertight && winding_consistent && degenerate_faces == 0 && euler_number == 2;

    Ok(PostGeometryValidationReport {
        watertight,
        manifold,
        degenerate_faces,
        euler_number,
        triangle_count,
        bbox_ok,
        warnings,
    })
}

/// Execute code and retry with AI fixes if execution fails.
///
/// The loop runs up to `config.max_validation_attempts` times:
/// 1. Static-validate generated code.
/// 2. Execute via `runner.py`.
/// 3. Post-validate geometry using mesh checks.
/// 4. If failure, classify, build retry prompt, call AI for fix.
pub async fn validate_and_retry(
    code: String,
    ctx: &ExecutionContext,
    system_prompt: &str,
    user_request: Option<&str>,
    on_event: &(dyn Fn(ValidationEvent) + Send + Sync),
) -> Result<ValidationResult, AppError> {
    let mut current_code = code;
    let mut retry_usage = TokenUsage::default();
    let max_attempts = configured_max_attempts(&ctx.config);
    let mut static_findings_accum: Vec<String> = Vec::new();

    for attempt in 1..=max_attempts {
        let message = if attempt == 1 {
            "Validating generated code...".to_string()
        } else {
            format!("Retrying... (attempt {}/{})", attempt, max_attempts)
        };
        on_event(ValidationEvent::Attempt {
            attempt,
            max_attempts,
            message,
        });

        let static_result = static_validate::validate_code(&current_code);
        let static_findings: Vec<String> = static_result
            .findings
            .iter()
            .map(|f| format!("{:?}: {}", f.level, f.message))
            .collect();

        for finding in &static_findings {
            if !static_findings_accum.contains(finding) {
                static_findings_accum.push(finding.clone());
            }
        }

        on_event(ValidationEvent::StaticValidation {
            passed: static_result.passed,
            findings: static_findings.clone(),
        });

        let execution_result = if static_result.passed {
            execute_with_timeout(&current_code, &ctx.venv_dir, &ctx.runner_script).await
        } else {
            Err(format!(
                "Static validation failed:\n{}",
                static_findings.join("\n")
            ))
        };

        match execution_result {
            Ok(exec_result) => {
                let post_report = run_post_geometry_checks(&current_code, ctx, user_request)
                    .map_err(AppError::CadQueryError)?;

                on_event(ValidationEvent::PostGeometryValidation {
                    report: post_report.clone(),
                });

                if !post_report.manifold || !post_report.bbox_ok {
                    let err = if post_report.warnings.is_empty() {
                        "Post-geometry validation failed".to_string()
                    } else {
                        format!(
                            "Post-geometry validation failed:\n{}",
                            post_report.warnings.join("\n")
                        )
                    };

                    let will_retry = attempt < max_attempts;
                    on_event(ValidationEvent::Failed {
                        attempt,
                        error_category: "PostGeometry".to_string(),
                        error_message: err.clone(),
                        will_retry,
                    });

                    if !will_retry {
                        return Ok(ValidationResult {
                            code: current_code,
                            stl_base64: None,
                            success: false,
                            attempts: attempt,
                            error: Some(err),
                            retry_usage,
                            static_findings: static_findings_accum,
                            post_geometry_report: Some(post_report),
                        });
                    }
                } else {
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
                        static_findings: static_findings_accum,
                        post_geometry_report: Some(post_report),
                    });
                }
            }
            Err(error_msg) => {
                let structured_error = validate::parse_traceback(&error_msg);
                let strategy =
                    validate::get_retry_strategy(&structured_error, attempt, Some(&current_code));

                let category_str = format!("{:?}", structured_error.category);
                let will_retry = attempt < max_attempts;

                on_event(ValidationEvent::Failed {
                    attempt,
                    error_category: category_str,
                    error_message: error_msg.clone(),
                    will_retry,
                });

                if !will_retry {
                    return Ok(ValidationResult {
                        code: current_code,
                        stl_base64: None,
                        success: false,
                        attempts: attempt,
                        error: Some(error_msg),
                        retry_usage,
                        static_findings: static_findings_accum,
                        post_geometry_report: None,
                    });
                }

                let rules = AgentRules::from_preset(ctx.config.agent_rules_preset.as_deref()).ok();
                let anti_pattern = rules.as_ref().and_then(|r| {
                    r.anti_patterns.as_ref().and_then(|patterns| {
                        strategy
                            .matching_anti_pattern
                            .as_ref()
                            .and_then(|title| patterns.iter().find(|p| p.title == *title))
                    })
                });

                let retry_prompt = build_retry_prompt_with_findings(
                    &current_code,
                    &error_msg,
                    &static_findings,
                    &[],
                    &structured_error,
                    &strategy,
                    anti_pattern,
                );

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

                match crate::agent::extract::extract_code(&ai_response) {
                    Some(new_code) => {
                        current_code = new_code;
                    }
                    None => {
                        return Ok(ValidationResult {
                            code: current_code,
                            stl_base64: None,
                            success: false,
                            attempts: attempt,
                            error: Some("AI retry did not produce extractable code".to_string()),
                            retry_usage,
                            static_findings: static_findings_accum,
                            post_geometry_report: None,
                        });
                    }
                }
            }
        }
    }

    Ok(ValidationResult {
        code: current_code,
        stl_base64: None,
        success: false,
        attempts: configured_max_attempts(&ctx.config),
        error: Some("Validation loop exhausted".to_string()),
        retry_usage,
        static_findings: static_findings_accum,
        post_geometry_report: None,
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
            static_findings: vec![],
            post_geometry_report: Some(PostGeometryValidationReport {
                watertight: true,
                manifold: true,
                degenerate_faces: 0,
                euler_number: 2,
                triangle_count: 128,
                bbox_ok: true,
                warnings: vec![],
            }),
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
            static_findings: vec!["Error: missing result".to_string()],
            post_geometry_report: None,
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

    #[test]
    fn test_parse_dimensions_from_text() {
        let dims = parse_dimensions_from_text("make a box 42x28x7.5mm with 1.8mm walls");
        assert!(dims.contains(&42.0));
        assert!(dims.contains(&28.0));
        assert!(dims.contains(&7.5));
        assert!(dims.contains(&1.8));
    }
}
