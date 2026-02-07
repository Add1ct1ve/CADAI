use base64::Engine;
use serde::Serialize;
use tauri::State;

use crate::error::AppError;
use crate::python::{detector, installer, runner, venv};
use crate::state::AppState;

#[derive(Serialize)]
pub struct ExecuteResult {
    pub stl_base64: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

#[derive(Serialize)]
pub struct PythonStatus {
    pub python_found: bool,
    pub python_version: Option<String>,
    pub python_path: Option<String>,
    pub venv_ready: bool,
    pub cadquery_installed: bool,
}

#[tauri::command]
pub async fn execute_code(
    code: String,
    state: State<'_, AppState>,
) -> Result<ExecuteResult, AppError> {
    let venv_path = state.venv_path.lock().unwrap().clone();

    let venv_dir = match venv_path {
        Some(p) => p,
        None => {
            return Ok(ExecuteResult {
                stl_base64: None,
                stdout: String::new(),
                stderr: "Python environment not set up. Click 'Setup Python' in settings."
                    .into(),
                success: false,
            });
        }
    };

    // Find the runner.py script
    let runner_script = find_runner_script()?;

    match runner::execute_cadquery(&venv_dir, &runner_script, &code) {
        Ok(result) => {
            let stl_base64 =
                base64::engine::general_purpose::STANDARD.encode(&result.stl_data);
            Ok(ExecuteResult {
                stl_base64: Some(stl_base64),
                stdout: result.stdout,
                stderr: result.stderr,
                success: true,
            })
        }
        Err(e) => Ok(ExecuteResult {
            stl_base64: None,
            stdout: String::new(),
            stderr: e.to_string(),
            success: false,
        }),
    }
}

#[tauri::command]
pub async fn check_python(
    state: State<'_, AppState>,
) -> Result<PythonStatus, AppError> {
    // Check if Python is detected
    let python_info = detector::detect_python().ok();

    let python_found = python_info.is_some();
    let python_version = python_info.as_ref().map(|i| i.version.clone());
    let python_path = python_info
        .as_ref()
        .map(|i| i.path.to_string_lossy().to_string());

    // Update state
    if let Some(ref info) = python_info {
        *state.python_path.lock().unwrap() = Some(info.path.clone());
    }

    // Check venv
    let venv_dir = venv::get_venv_dir()?;
    let venv_ready = venv::venv_exists(&venv_dir);
    let cadquery_installed = if venv_ready {
        installer::is_cadquery_installed(&venv_dir)
    } else {
        false
    };

    if venv_ready {
        *state.venv_path.lock().unwrap() = Some(venv_dir);
    }

    Ok(PythonStatus {
        python_found,
        python_version,
        python_path,
        venv_ready,
        cadquery_installed,
    })
}

#[tauri::command]
pub async fn setup_python(
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    // Detect Python
    let info = detector::detect_python()?;
    *state.python_path.lock().unwrap() = Some(info.path.clone());

    // Create venv
    let venv_dir = venv::get_venv_dir()?;
    if !venv::venv_exists(&venv_dir) {
        venv::create_venv(&info.path, &venv_dir)?;
    }

    // Install CadQuery
    if !installer::is_cadquery_installed(&venv_dir) {
        installer::install_cadquery(&venv_dir)?;
    }

    *state.venv_path.lock().unwrap() = Some(venv_dir);

    Ok(format!(
        "Python {} environment ready with CadQuery",
        info.version
    ))
}

pub(crate) fn find_runner_script() -> Result<std::path::PathBuf, AppError> {
    // In development, look for it relative to the current working directory
    let candidates = vec![
        std::env::current_dir()
            .unwrap_or_default()
            .join("python")
            .join("runner.py"),
        // Also try relative to the executable
        std::env::current_exe()
            .unwrap_or_default()
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("python")
            .join("runner.py"),
        // Try up from src-tauri
        std::env::current_dir()
            .unwrap_or_default()
            .join("..")
            .join("python")
            .join("runner.py"),
    ];

    for candidate in candidates {
        let canonical = candidate.canonicalize().ok();
        if let Some(path) = canonical {
            if path.exists() {
                return Ok(path);
            }
        }
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(AppError::ConfigError("runner.py not found".into()))
}
