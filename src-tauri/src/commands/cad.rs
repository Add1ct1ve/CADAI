use std::time::Instant;

use base64::Engine;
use serde::Serialize;
use tauri::State;

use crate::error::AppError;
use crate::python::{detector, installer, runner, venv};
use crate::state::AppState;

const DEFAULT_EXECUTION_TIMEOUT_MS: u64 = 30_000;
const MIN_EXECUTION_TIMEOUT_MS: u64 = 1_000;
const MAX_EXECUTION_TIMEOUT_MS: u64 = 120_000;

#[derive(Serialize)]
pub struct ExecutionArtifacts {
    pub stl_base64: Option<String>,
}

#[derive(Serialize)]
pub struct ExecutionTiming {
    pub duration_ms: u64,
    pub timeout_ms: u64,
}

#[derive(Serialize)]
pub struct ExecuteResult {
    pub success: bool,
    pub artifacts: ExecutionArtifacts,
    pub stdout: String,
    pub stderr: String,
    pub logs: Vec<String>,
    pub timing: ExecutionTiming,
    pub error: Option<String>,
    // Backward compatibility for existing frontend callers.
    pub stl_base64: Option<String>,
}

#[derive(Serialize)]
pub struct PythonStatus {
    pub python_found: bool,
    pub python_version: Option<String>,
    pub python_path: Option<String>,
    pub venv_ready: bool,
    pub cadquery_installed: bool,
    pub cadquery_version: Option<String>,
}

fn clamp_timeout(timeout_ms: Option<u64>) -> u64 {
    timeout_ms
        .unwrap_or(DEFAULT_EXECUTION_TIMEOUT_MS)
        .clamp(MIN_EXECUTION_TIMEOUT_MS, MAX_EXECUTION_TIMEOUT_MS)
}

fn collect_logs(stdout: &str, stderr: &str, error: Option<&str>) -> Vec<String> {
    let mut logs = Vec::new();
    if !stdout.trim().is_empty() {
        logs.push(stdout.trim().to_string());
    }
    if !stderr.trim().is_empty() {
        logs.push(stderr.trim().to_string());
    }
    if let Some(err) = error {
        if !err.trim().is_empty() {
            logs.push(err.trim().to_string());
        }
    }
    logs
}

#[tauri::command]
pub async fn execute_code(
    code: String,
    timeout_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<ExecuteResult, AppError> {
    let start = Instant::now();
    let timeout_ms = clamp_timeout(timeout_ms);
    let venv_path = state
        .venv_path
        .lock()
        .map_err(|_| AppError::ConfigError("Failed to access Python environment state".into()))?
        .clone();

    let venv_dir = match venv_path {
        Some(p) => p,
        None => {
            let stderr =
                "Python environment not set up. Click 'Setup Python' in settings.".to_string();
            let duration_ms = start.elapsed().as_millis() as u64;
            return Ok(ExecuteResult {
                success: false,
                artifacts: ExecutionArtifacts { stl_base64: None },
                stdout: String::new(),
                stderr: stderr.clone(),
                logs: collect_logs("", &stderr, Some(&stderr)),
                timing: ExecutionTiming {
                    duration_ms,
                    timeout_ms,
                },
                error: Some(stderr),
                stl_base64: None,
            });
        }
    };

    // Find the runner.py script
    let runner_script = super::find_python_script("runner.py")?;
    let venv_owned = venv_dir.clone();
    let runner_owned = runner_script.clone();
    let code_owned = code.clone();

    let result = tokio::task::spawn_blocking(move || {
        runner::execute_cadquery_with_timeout_ms(
            &venv_owned,
            &runner_owned,
            &code_owned,
            timeout_ms,
        )
    })
    .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(exec_result)) => {
            let stl_base64 =
                base64::engine::general_purpose::STANDARD.encode(&exec_result.stl_data);
            Ok(ExecuteResult {
                success: true,
                artifacts: ExecutionArtifacts {
                    stl_base64: Some(stl_base64.clone()),
                },
                stdout: exec_result.stdout.clone(),
                stderr: exec_result.stderr.clone(),
                logs: collect_logs(&exec_result.stdout, &exec_result.stderr, None),
                timing: ExecutionTiming {
                    duration_ms,
                    timeout_ms,
                },
                error: None,
                stl_base64: Some(stl_base64),
            })
        }
        Ok(Err(e)) => {
            let message = e.to_string();
            Ok(ExecuteResult {
                success: false,
                artifacts: ExecutionArtifacts { stl_base64: None },
                stdout: String::new(),
                stderr: message.clone(),
                logs: collect_logs("", &message, Some(&message)),
                timing: ExecutionTiming {
                    duration_ms,
                    timeout_ms,
                },
                error: Some(message),
                stl_base64: None,
            })
        }
        Err(join_err) => {
            let message = format!("Execution task panicked: {}", join_err);
            Ok(ExecuteResult {
                success: false,
                artifacts: ExecutionArtifacts { stl_base64: None },
                stdout: String::new(),
                stderr: message.clone(),
                logs: collect_logs("", &message, Some(&message)),
                timing: ExecutionTiming {
                    duration_ms,
                    timeout_ms,
                },
                error: Some(message),
                stl_base64: None,
            })
        }
    }
}

#[tauri::command]
pub async fn check_python(state: State<'_, AppState>) -> Result<PythonStatus, AppError> {
    // Check if Python is detected
    let python_info = detector::detect_python().ok();

    let python_found = python_info.is_some();
    let python_version = python_info.as_ref().map(|i| i.version.clone());
    let python_path = python_info
        .as_ref()
        .map(|i| i.path.to_string_lossy().to_string());

    // Update state
    if let Some(ref info) = python_info {
        *state
            .python_path
            .lock()
            .map_err(|_| AppError::ConfigError("Failed to update Python path state".into()))? =
            Some(info.path.clone());
    }

    // Check venv
    let venv_dir = venv::get_venv_dir()?;
    let venv_ready = venv::venv_exists(&venv_dir);
    let cadquery_installed = if venv_ready {
        installer::is_cadquery_installed(&venv_dir)
    } else {
        false
    };

    // Detect and cache CadQuery version
    let cadquery_version = if cadquery_installed {
        let ver = installer::detect_cadquery_version(&venv_dir);
        *state.cadquery_version.lock().map_err(|_| {
            AppError::ConfigError("Failed to update CadQuery version state".into())
        })? = ver.clone();
        ver
    } else {
        None
    };

    if venv_ready {
        *state
            .venv_path
            .lock()
            .map_err(|_| AppError::ConfigError("Failed to update venv state".into()))? =
            Some(venv_dir);
    }

    Ok(PythonStatus {
        python_found,
        python_version,
        python_path,
        venv_ready,
        cadquery_installed,
        cadquery_version,
    })
}

#[tauri::command]
pub async fn setup_python(state: State<'_, AppState>) -> Result<String, AppError> {
    // Detect Python
    let info = detector::detect_python()?;
    *state
        .python_path
        .lock()
        .map_err(|_| AppError::ConfigError("Failed to update Python path state".into()))? =
        Some(info.path.clone());

    // Create venv
    let venv_dir = venv::get_venv_dir()?;
    if !venv::venv_exists(&venv_dir) {
        venv::create_venv(&info.path, &venv_dir)?;
    }

    // Install CadQuery
    if !installer::is_cadquery_installed(&venv_dir) {
        installer::install_cadquery(&venv_dir)?;
    }

    // Detect and cache CadQuery version
    let cq_version = installer::detect_cadquery_version(&venv_dir);
    *state
        .cadquery_version
        .lock()
        .map_err(|_| AppError::ConfigError("Failed to update CadQuery version state".into()))? =
        cq_version.clone();

    *state
        .venv_path
        .lock()
        .map_err(|_| AppError::ConfigError("Failed to update venv state".into()))? = Some(venv_dir);

    let cq_ver_str = cq_version.unwrap_or_else(|| "unknown".to_string());
    Ok(format!(
        "Python {} environment ready with CadQuery {}",
        info.version, cq_ver_str
    ))
}
