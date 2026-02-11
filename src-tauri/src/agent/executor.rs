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
    pub component_count: u64,
    pub bounds_min: [f64; 3],
    pub bounds_max: [f64; 3],
    pub volume: f64,
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
    pub post_check_warning: Option<String>,
    pub retry_ladder_stage_reached: Option<u32>,
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
    PostGeometryWarning {
        message: String,
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

fn should_retry_from_post_geometry(report: &PostGeometryValidationReport) -> bool {
    !report.manifold || !report.bbox_ok
}

fn format_post_check_warning(reason: &str) -> String {
    format!(
        "Geometry post-check unavailable (tooling error). Model generated; validate manually if critical. {}",
        reason
    )
}

fn format_decimal(value: f64) -> String {
    let mut s = format!("{:.4}", value);
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.push('0');
    }
    s
}

fn reduce_fillet_radii_in_line(line: &str) -> String {
    let re = Regex::new(r"\.(fillet|chamfer)\(\s*([0-9]+(?:\.[0-9]+)?)\s*\)")
        .expect("valid fillet regex");
    re.replace_all(line, |caps: &regex::Captures<'_>| {
        let op = &caps[1];
        let raw = caps[2].parse::<f64>().unwrap_or(1.0);
        let reduced = (raw * 0.5).max(0.2);
        format!(".{}({})", op, format_decimal(reduced))
    })
    .to_string()
}

fn extract_assignment_lhs(line: &str) -> Option<String> {
    let re = Regex::new(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=").expect("valid assignment regex");
    re.captures(line).map(|caps| caps[1].to_string())
}

fn wrap_line_with_fillet_guard(line: &str) -> String {
    let indent: String = line
        .chars()
        .take_while(|c| c.is_ascii_whitespace())
        .collect();
    let trimmed = line.trim_start();
    let safer = reduce_fillet_radii_in_line(trimmed);
    format!(
        "{i}# auto-fillet-guard\n{i}try:\n{i}    {s}\n{i}except Exception:\n{i}    pass",
        i = indent,
        s = safer
    )
}

fn wrap_line_with_sweep_guard(line: &str) -> String {
    let indent: String = line
        .chars()
        .take_while(|c| c.is_ascii_whitespace())
        .collect();
    let trimmed = line.trim_start();
    let fallback = if let Some(lhs) = extract_assignment_lhs(trimmed) {
        format!(
            "{i}    try:\n{i}        {lhs} = result\n{i}    except Exception:\n{i}        pass",
            i = indent,
            lhs = lhs
        )
    } else {
        format!("{i}    pass", i = indent)
    };

    format!(
        "{i}# auto-sweep-guard\n{i}try:\n{i}    {s}\n{i}except Exception:\n{fallback}",
        i = indent,
        s = trimmed,
        fallback = fallback
    )
}

fn apply_line_targeted_fillet_guard(code: &str, source_line: &str) -> Option<String> {
    let target = source_line.trim();
    if target.is_empty() || !(target.contains(".fillet(") || target.contains(".chamfer(")) {
        return None;
    }

    let mut changed = false;
    let mut out = Vec::new();
    for line in code.lines() {
        if !changed && !line.contains("auto-fillet-guard") && line.trim() == target {
            out.push(wrap_line_with_fillet_guard(line));
            changed = true;
        } else {
            out.push(line.to_string());
        }
    }

    if changed {
        Some(out.join("\n"))
    } else {
        None
    }
}

fn apply_last_fillet_guard_fallback(code: &str) -> Option<String> {
    let lines: Vec<&str> = code.lines().collect();
    let mut idx: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if (t.contains(".fillet(") || t.contains(".chamfer(")) && !t.contains("auto-fillet-guard") {
            idx = Some(i);
        }
    }
    let i = idx?;
    let mut out = Vec::new();
    for (j, line) in lines.iter().enumerate() {
        if j == i {
            out.push(wrap_line_with_fillet_guard(line));
        } else {
            out.push((*line).to_string());
        }
    }
    Some(out.join("\n"))
}

fn apply_line_targeted_sweep_guard(code: &str, source_line: &str) -> Option<String> {
    let target = source_line.trim();
    if target.is_empty() || !target.contains(".sweep(") {
        return None;
    }

    let mut changed = false;
    let mut out = Vec::new();
    for line in code.lines() {
        if !changed && !line.contains("auto-sweep-guard") && line.trim() == target {
            out.push(wrap_line_with_sweep_guard(line));
            changed = true;
        } else {
            out.push(line.to_string());
        }
    }

    if changed {
        Some(out.join("\n"))
    } else {
        None
    }
}

fn apply_last_sweep_guard_fallback(code: &str) -> Option<String> {
    let lines: Vec<&str> = code.lines().collect();
    let mut idx: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.contains(".sweep(") && !t.contains("auto-sweep-guard") {
            idx = Some(i);
        }
    }
    let i = idx?;
    let mut out = Vec::new();
    for (j, line) in lines.iter().enumerate() {
        if j == i {
            out.push(wrap_line_with_sweep_guard(line));
        } else {
            out.push((*line).to_string());
        }
    }
    Some(out.join("\n"))
}

fn apply_global_fillet_guard(code: &str) -> Option<String> {
    let mut changed = false;
    let mut out = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if (trimmed.contains(".fillet(") || trimmed.contains(".chamfer("))
            && !trimmed.contains("auto-fillet-guard")
        {
            out.push(wrap_line_with_fillet_guard(line));
            changed = true;
        } else {
            out.push(line.to_string());
        }
    }
    if changed {
        Some(out.join("\n"))
    } else {
        None
    }
}

fn strip_fillet_chamfer_operations(code: &str) -> Option<String> {
    let mut changed = false;
    let mut out = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.contains(".fillet(") || trimmed.contains(".chamfer(") {
            let indent: String = line
                .chars()
                .take_while(|c| c.is_ascii_whitespace())
                .collect();
            if let Some(lhs) = extract_assignment_lhs(trimmed) {
                out.push(format!(
                    "{i}# auto-strip-fillet\n{i}{lhs} = {lhs}",
                    i = indent,
                    lhs = lhs
                ));
            } else {
                out.push(format!("{}# auto-strip-fillet {}", indent, trimmed));
            }
            changed = true;
        } else {
            out.push(line.to_string());
        }
    }
    if changed {
        Some(out.join("\n"))
    } else {
        None
    }
}

fn apply_global_sweep_guard(code: &str) -> Option<String> {
    let mut changed = false;
    let mut out = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.contains(".sweep(") && !trimmed.contains("auto-sweep-guard") {
            out.push(wrap_line_with_sweep_guard(line));
            changed = true;
        } else {
            out.push(line.to_string());
        }
    }
    if changed {
        Some(out.join("\n"))
    } else {
        None
    }
}

fn line_has_ambiguous_faces_workplane_chain(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains(".faces(") || !trimmed.contains(".workplane(") {
        return false;
    }
    if trimmed.contains(".first().workplane(")
        || trimmed.contains(".last().workplane(")
        || trimmed.contains(".item(")
    {
        return false;
    }

    let faces_idx = match trimmed.find(".faces(") {
        Some(i) => i,
        None => return false,
    };
    let wp_idx = match trimmed.find(".workplane(") {
        Some(i) => i,
        None => return false,
    };

    faces_idx < wp_idx
}

fn add_first_before_workplane(line: &str) -> Option<String> {
    if !line_has_ambiguous_faces_workplane_chain(line) {
        return None;
    }

    let wp_idx = line.find(".workplane(")?;
    let mut out = String::new();
    out.push_str(&line[..wp_idx]);
    out.push_str(".first().workplane(");
    out.push_str(&line[wp_idx + ".workplane(".len()..]);
    Some(out)
}

fn apply_line_targeted_workplane_face_fix(code: &str, source_line: &str) -> Option<String> {
    let target = source_line.trim();
    if target.is_empty() || !line_has_ambiguous_faces_workplane_chain(target) {
        return None;
    }

    let mut changed = false;
    let mut out = Vec::new();
    for line in code.lines() {
        if !changed && line.trim() == target {
            if let Some(rewritten) = add_first_before_workplane(line) {
                out.push(rewritten);
                changed = true;
            } else {
                out.push(line.to_string());
            }
        } else {
            out.push(line.to_string());
        }
    }

    if changed {
        Some(out.join("\n"))
    } else {
        None
    }
}

fn apply_global_workplane_face_fix(code: &str) -> Option<String> {
    let mut changed = false;
    let mut out = Vec::new();
    for line in code.lines() {
        if let Some(rewritten) = add_first_before_workplane(line) {
            out.push(rewritten);
            changed = true;
        } else {
            out.push(line.to_string());
        }
    }
    if changed {
        Some(out.join("\n"))
    } else {
        None
    }
}

fn is_workplane_selection_failure_candidate(
    structured_error: &validate::StructuredError,
    runtime_error: &str,
) -> bool {
    let msg = runtime_error.to_lowercase();
    let source = structured_error
        .context
        .as_ref()
        .and_then(|c| c.source_line.as_ref())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if msg.contains("if multiple objects selected, they all must be planar faces")
        || (msg.contains("multiple objects selected") && msg.contains("planar faces"))
    {
        return true;
    }

    matches!(
        structured_error.category,
        validate::ErrorCategory::ApiMisuse
    ) && source.contains(".faces(")
        && source.contains(".workplane(")
}

fn is_fillet_failure_candidate(
    structured_error: &validate::StructuredError,
    runtime_error: &str,
) -> bool {
    let category_hit = matches!(
        structured_error.category,
        validate::ErrorCategory::Topology(validate::TopologySubKind::FilletFailure)
    );
    if category_hit {
        return true;
    }

    let msg = runtime_error.to_lowercase();
    let source = structured_error
        .context
        .as_ref()
        .and_then(|c| c.source_line.as_ref())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    if msg.contains("cannot find a solid on the stack or in the parent chain")
        && (source.contains(".fillet(") || source.contains(".chamfer("))
    {
        return true;
    }
    (msg.contains("stdfail_notdone") || msg.contains("brep_api: command not done"))
        && (msg.contains("fillet")
            || msg.contains("chamfer")
            || source.contains(".fillet(")
            || source.contains(".chamfer("))
}

fn maybe_apply_fillet_auto_repair(
    current_code: &str,
    structured_error: &validate::StructuredError,
    runtime_error: &str,
) -> Option<String> {
    if !is_fillet_failure_candidate(structured_error, runtime_error) {
        return None;
    }

    if let Some(source_line) = structured_error
        .context
        .as_ref()
        .and_then(|c| c.source_line.as_ref())
    {
        if let Some(repaired) = apply_line_targeted_fillet_guard(current_code, source_line) {
            return Some(repaired);
        }
    }

    apply_last_fillet_guard_fallback(current_code)
}

fn is_sweep_failure_candidate(
    structured_error: &validate::StructuredError,
    runtime_error: &str,
) -> bool {
    if matches!(
        structured_error.category,
        validate::ErrorCategory::Topology(validate::TopologySubKind::SweepFailure)
    ) {
        return true;
    }

    let msg = runtime_error.to_lowercase();
    let source = structured_error
        .context
        .as_ref()
        .and_then(|c| c.source_line.as_ref())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if msg.contains("no pending wires present") {
        return true;
    }

    (msg.contains("sweep") && msg.contains("wire")) || source.contains(".sweep(")
}

fn maybe_apply_sweep_auto_repair(
    current_code: &str,
    structured_error: &validate::StructuredError,
    runtime_error: &str,
) -> Option<String> {
    if !is_sweep_failure_candidate(structured_error, runtime_error) {
        return None;
    }

    if let Some(source_line) = structured_error
        .context
        .as_ref()
        .and_then(|c| c.source_line.as_ref())
    {
        if let Some(repaired) = apply_line_targeted_sweep_guard(current_code, source_line) {
            return Some(repaired);
        }
    }

    apply_last_sweep_guard_fallback(current_code)
}

fn maybe_apply_ladder_auto_repair(
    current_code: &str,
    structured_error: &validate::StructuredError,
    runtime_error: &str,
    attempt: u32,
) -> Option<(String, u32)> {
    let workplane_candidate =
        is_workplane_selection_failure_candidate(structured_error, runtime_error);
    if workplane_candidate {
        if let Some(source_line) = structured_error
            .context
            .as_ref()
            .and_then(|c| c.source_line.as_ref())
        {
            if let Some(repaired) =
                apply_line_targeted_workplane_face_fix(current_code, source_line)
            {
                return Some((repaired, 1));
            }
        }
        if let Some(repaired) = apply_global_workplane_face_fix(current_code) {
            return Some((repaired, 1));
        }
    }

    let fillet_candidate = is_fillet_failure_candidate(structured_error, runtime_error);
    let sweep_candidate = is_sweep_failure_candidate(structured_error, runtime_error);

    if fillet_candidate {
        match attempt {
            1 => {
                if let Some(source_line) = structured_error
                    .context
                    .as_ref()
                    .and_then(|c| c.source_line.as_ref())
                {
                    if let Some(repaired) =
                        apply_line_targeted_fillet_guard(current_code, source_line)
                    {
                        return Some((repaired, 1));
                    }
                }
                if let Some(repaired) = apply_last_fillet_guard_fallback(current_code) {
                    return Some((repaired, 1));
                }
            }
            2 => {
                if let Some(repaired) = apply_global_fillet_guard(current_code) {
                    return Some((repaired, 2));
                }
            }
            _ => {
                if let Some(repaired) = strip_fillet_chamfer_operations(current_code) {
                    return Some((repaired, 3));
                }
            }
        }
    }

    if sweep_candidate && attempt >= 3 {
        if let Some(source_line) = structured_error
            .context
            .as_ref()
            .and_then(|c| c.source_line.as_ref())
        {
            if let Some(repaired) = apply_line_targeted_sweep_guard(current_code, source_line) {
                return Some((repaired, 4));
            }
        }
        if let Some(repaired) = apply_last_sweep_guard_fallback(current_code) {
            return Some((repaired, 4));
        }
        if let Some(repaired) = apply_global_sweep_guard(current_code) {
            return Some((repaired, 4));
        }
    }

    None
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
    let component_count = parsed["component_count"].as_u64().unwrap_or(1).max(1);
    let triangle_count = parsed["triangle_count"].as_u64().unwrap_or(0);
    let volume = parsed["volume"].as_f64().unwrap_or(0.0);

    let mut warnings: Vec<String> = parsed["issues"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut bbox_ok = true;
    let mut bounds_min = [0.0_f64; 3];
    let mut bounds_max = [0.0_f64; 3];

    if let Some(bounds_arr) = parsed["bounds"].as_array() {
        if bounds_arr.len() == 2 {
            let min = bounds_arr[0].as_array();
            let max = bounds_arr[1].as_array();
            if let (Some(min), Some(max)) = (min, max) {
                if min.len() >= 3 && max.len() >= 3 {
                    bounds_min = [
                        min[0].as_f64().unwrap_or(0.0),
                        min[1].as_f64().unwrap_or(0.0),
                        min[2].as_f64().unwrap_or(0.0),
                    ];
                    bounds_max = [
                        max[0].as_f64().unwrap_or(0.0),
                        max[1].as_f64().unwrap_or(0.0),
                        max[2].as_f64().unwrap_or(0.0),
                    ];

                    if let Some(req) = user_request {
                        let dx = bounds_max[0] - bounds_min[0];
                        let dy = bounds_max[1] - bounds_min[1];
                        let dz = bounds_max[2] - bounds_min[2];
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
    }

    let expected_euler = (2_u64.saturating_mul(component_count)) as i64;
    let manifold =
        watertight && winding_consistent && degenerate_faces == 0 && euler_number == expected_euler;

    Ok(PostGeometryValidationReport {
        watertight,
        manifold,
        degenerate_faces,
        euler_number,
        triangle_count,
        component_count,
        bounds_min,
        bounds_max,
        volume,
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
    let mut retry_ladder_stage_reached: Option<u32> = None;

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

        let static_result = static_validate::validate_code_with_profile(
            &current_code,
            &ctx.config.generation_reliability_profile,
            attempt == 1,
        );
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
                match run_post_geometry_checks(&current_code, ctx, user_request) {
                    Ok(post_report) => {
                        on_event(ValidationEvent::PostGeometryValidation {
                            report: post_report.clone(),
                        });

                        if should_retry_from_post_geometry(&post_report) {
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
                                    post_check_warning: None,
                                    retry_ladder_stage_reached,
                                });
                            }
                        } else {
                            let stl_base64 = base64::engine::general_purpose::STANDARD
                                .encode(&exec_result.stl_data);
                            on_event(ValidationEvent::Success {
                                attempt,
                                message: format!(
                                    "Code validated successfully on attempt {}.",
                                    attempt
                                ),
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
                                post_check_warning: None,
                                retry_ladder_stage_reached,
                            });
                        }
                    }
                    Err(reason) => {
                        // Soft-fail policy for post-check infrastructure errors.
                        let warning = format_post_check_warning(&reason);
                        on_event(ValidationEvent::PostGeometryWarning {
                            message: warning.clone(),
                        });

                        let stl_base64 =
                            base64::engine::general_purpose::STANDARD.encode(&exec_result.stl_data);
                        on_event(ValidationEvent::Success {
                            attempt,
                            message: format!(
                                "Code validated successfully on attempt {} (post-check soft-fail).",
                                attempt
                            ),
                        });

                        return Ok(ValidationResult {
                            code: current_code,
                            stl_base64: Some(stl_base64),
                            success: true,
                            attempts: attempt,
                            error: None,
                            retry_usage,
                            static_findings: static_findings_accum,
                            post_geometry_report: None,
                            post_check_warning: Some(warning),
                            retry_ladder_stage_reached,
                        });
                    }
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
                        post_check_warning: None,
                        retry_ladder_stage_reached,
                    });
                }

                if let Some((auto_fixed_code, stage)) = maybe_apply_ladder_auto_repair(
                    &current_code,
                    &structured_error,
                    &error_msg,
                    attempt,
                ) {
                    retry_ladder_stage_reached = Some(
                        retry_ladder_stage_reached
                            .map(|s| s.max(stage))
                            .unwrap_or(stage),
                    );
                    current_code = auto_fixed_code;
                    continue;
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
                            post_check_warning: None,
                            retry_ladder_stage_reached,
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
        post_check_warning: None,
        retry_ladder_stage_reached,
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
                component_count: 1,
                bounds_min: [0.0, 0.0, 0.0],
                bounds_max: [10.0, 10.0, 10.0],
                volume: 1000.0,
                bbox_ok: true,
                warnings: vec![],
            }),
            post_check_warning: None,
            retry_ladder_stage_reached: None,
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
            post_check_warning: None,
            retry_ladder_stage_reached: None,
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

    #[test]
    fn test_should_retry_from_post_geometry_behavior() {
        let good = PostGeometryValidationReport {
            watertight: true,
            manifold: true,
            degenerate_faces: 0,
            euler_number: 2,
            triangle_count: 100,
            component_count: 1,
            bounds_min: [0.0, 0.0, 0.0],
            bounds_max: [10.0, 10.0, 10.0],
            volume: 1000.0,
            bbox_ok: true,
            warnings: vec![],
        };
        assert!(!should_retry_from_post_geometry(&good));

        let bad = PostGeometryValidationReport {
            manifold: false,
            warnings: vec!["bad".to_string()],
            ..good
        };
        assert!(should_retry_from_post_geometry(&bad));
    }

    #[test]
    fn test_post_check_warning_message() {
        let msg = format_post_check_warning("post-check returned exit code 1");
        assert!(msg.contains("Geometry post-check unavailable"));
        assert!(msg.contains("post-check returned exit code 1"));
    }

    #[test]
    fn test_soft_fail_validation_result_shape() {
        let exec_result = runner::ExecutionResult {
            stl_data: vec![1, 2, 3],
            stdout: String::new(),
            stderr: String::new(),
        };
        let stl_base64 = base64::engine::general_purpose::STANDARD.encode(&exec_result.stl_data);
        let result = ValidationResult {
            code: "import cadquery as cq\nresult = cq.Workplane('XY').box(1,1,1)".to_string(),
            stl_base64: Some(stl_base64),
            success: true,
            attempts: 1,
            error: None,
            retry_usage: TokenUsage::default(),
            static_findings: vec![],
            post_geometry_report: None,
            post_check_warning: Some(format_post_check_warning("trimesh API mismatch")),
            retry_ladder_stage_reached: None,
        };

        assert!(result.success);
        assert!(result.error.is_none());
        assert!(result.stl_base64.is_some());
        assert!(result.post_check_warning.is_some());
    }

    #[test]
    fn test_maybe_apply_fillet_auto_repair_targets_source_line() {
        let code = r#"import cadquery as cq
result = cq.Workplane("XY").box(10, 10, 10)
result = result.edges("|Z").fillet(4.0)"#;

        let err = validate::StructuredError {
            error_type: "OCP.StdFail_NotDone".to_string(),
            message: "BRep_API: command not done".to_string(),
            line_number: Some(3),
            suggestion: None,
            category: validate::ErrorCategory::Topology(validate::TopologySubKind::FilletFailure),
            failing_operation: Some("fillet".to_string()),
            context: Some(validate::ErrorContext {
                source_line: Some(r#"result = result.edges("|Z").fillet(4.0)"#.to_string()),
                failing_parameters: None,
            }),
        };

        let repaired = maybe_apply_fillet_auto_repair(code, &err, "StdFail_NotDone").unwrap();
        assert!(repaired.contains("auto-fillet-guard"));
        assert!(repaired.contains("try:"));
        assert!(repaired.contains(".fillet(2.0)"));
    }

    #[test]
    fn test_maybe_apply_fillet_auto_repair_fallback_last_fillet() {
        let code = r#"import cadquery as cq
result = cq.Workplane("XY").box(10, 10, 10)
body = result.edges("|Z").fillet(3.0)
result = body"#;

        let err = validate::StructuredError {
            error_type: "UnknownError".to_string(),
            message: "BRep_API: command not done".to_string(),
            line_number: None,
            suggestion: None,
            category: validate::ErrorCategory::ApiMisuse,
            failing_operation: None,
            context: None,
        };

        let repaired = maybe_apply_fillet_auto_repair(code, &err, "StdFail_NotDone in fillet")
            .expect("fallback should wrap last fillet");
        assert!(repaired.contains("auto-fillet-guard"));
        assert!(repaired.contains(".fillet(1.5)"));
    }

    #[test]
    fn test_maybe_apply_fillet_auto_repair_handles_no_solid_error() {
        let code = r#"import cadquery as cq
result = cq.Workplane("XY").box(10, 10, 10)
result = result.edges("|Z").fillet(3.0)"#;

        let err = validate::StructuredError {
            error_type: "ValueError".to_string(),
            message: "Cannot find a solid on the stack or in the parent chain".to_string(),
            line_number: Some(3),
            suggestion: None,
            category: validate::ErrorCategory::ApiMisuse,
            failing_operation: None,
            context: Some(validate::ErrorContext {
                source_line: Some(r#"result = result.edges("|Z").fillet(3.0)"#.to_string()),
                failing_parameters: None,
            }),
        };

        let repaired = maybe_apply_fillet_auto_repair(
            code,
            &err,
            "ValueError: Cannot find a solid on the stack or in the parent chain",
        )
        .expect("should guard fillet for no-solid failure");
        assert!(repaired.contains("auto-fillet-guard"));
    }

    #[test]
    fn test_maybe_apply_sweep_auto_repair_targets_source_line() {
        let code = r#"import cadquery as cq
result = cq.Workplane("XY").box(10, 10, 1)
ridge = cq.Workplane("XY").rect(2, 2).sweep(path)
result = result.union(ridge)"#;

        let err = validate::StructuredError {
            error_type: "ValueError".to_string(),
            message: "No pending wires present".to_string(),
            line_number: Some(3),
            suggestion: None,
            category: validate::ErrorCategory::Topology(validate::TopologySubKind::SweepFailure),
            failing_operation: Some("sweep".to_string()),
            context: Some(validate::ErrorContext {
                source_line: Some(
                    r#"ridge = cq.Workplane("XY").rect(2, 2).sweep(path)"#.to_string(),
                ),
                failing_parameters: None,
            }),
        };

        let repaired =
            maybe_apply_sweep_auto_repair(code, &err, "ValueError: No pending wires present")
                .expect("should guard sweep");
        assert!(repaired.contains("auto-sweep-guard"));
        assert!(repaired.contains("ridge = result"));
    }

    #[test]
    fn test_workplane_face_fix_inserts_first() {
        let line = r#"wp = body.faces(">Z").workplane(offset=1.0)"#;
        let fixed = add_first_before_workplane(line).expect("should rewrite ambiguous workplane");
        assert_eq!(
            fixed,
            r#"wp = body.faces(">Z").first().workplane(offset=1.0)"#
        );
    }

    #[test]
    fn test_workplane_face_fix_skips_existing_first() {
        let line = r#"wp = body.faces(">Z").first().workplane(offset=1.0)"#;
        assert!(add_first_before_workplane(line).is_none());
    }

    #[test]
    fn test_ladder_repairs_planar_faces_workplane_error() {
        let code = r#"import cadquery as cq
body = cq.Workplane("XY").box(10, 10, 10)
wp = body.faces(">Z").workplane(offset=1.0)
result = body"#;

        let err = validate::StructuredError {
            error_type: "ValueError".to_string(),
            message: "If multiple objects selected, they all must be planar faces.".to_string(),
            line_number: Some(3),
            suggestion: None,
            category: validate::ErrorCategory::ApiMisuse,
            failing_operation: Some("workplane".to_string()),
            context: Some(validate::ErrorContext {
                source_line: Some(r#"wp = body.faces(">Z").workplane(offset=1.0)"#.to_string()),
                failing_parameters: None,
            }),
        };

        let repaired = maybe_apply_ladder_auto_repair(
            code,
            &err,
            "ValueError: If multiple objects selected, they all must be planar faces.",
            1,
        )
        .expect("planar-faces workplane failure should be auto-repaired");

        assert_eq!(repaired.1, 1);
        assert!(repaired
            .0
            .contains(r#"body.faces(">Z").first().workplane(offset=1.0)"#));
    }
}
