use serde::{Deserialize, Serialize};
use tauri::State;

use crate::ai::message::ChatMessage;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Serialize, Deserialize)]
pub struct ProjectFile {
    pub name: String,
    pub code: String,
    pub messages: Vec<ChatMessage>,
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scene: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn save_project(
    name: String,
    code: String,
    messages: Vec<ChatMessage>,
    path: String,
    scene: Option<serde_json::Value>,
) -> Result<(), AppError> {
    let project = ProjectFile {
        name,
        code,
        messages,
        version: 2,
        scene,
    };
    let json = serde_json::to_string_pretty(&project)?;
    std::fs::write(&path, json)?;
    Ok(())
}

#[tauri::command]
pub async fn load_project(path: String) -> Result<ProjectFile, AppError> {
    let contents = std::fs::read_to_string(&path)?;
    let project: ProjectFile = serde_json::from_str(&contents)
        .map_err(|e| AppError::ConfigError(format!("Invalid project file: {}", e)))?;
    Ok(project)
}

#[tauri::command]
pub async fn export_stl(
    code: String,
    output_path: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let venv_path = state.venv_path.lock().unwrap().clone();
    let venv_dir = venv_path
        .ok_or(AppError::CadQueryError("Python environment not set up".into()))?;

    // Find runner script
    let runner_script = super::find_python_script("runner.py")?;

    // Execute CadQuery to generate STL
    let result = crate::python::runner::execute_cadquery(&venv_dir, &runner_script, &code)?;

    // Write STL to the specified path
    std::fs::write(&output_path, &result.stl_data)?;

    Ok(format!("STL exported to {}", output_path))
}

#[tauri::command]
pub async fn export_step(
    code: String,
    output_path: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let venv_path = state.venv_path.lock().unwrap().clone();
    let venv_dir = venv_path
        .ok_or(AppError::CadQueryError("Python environment not set up".into()))?;

    let runner_script = super::find_python_script("runner.py")?;

    // The runner auto-detects .step extension and exports STEP format
    crate::python::runner::execute_cadquery_to_file(
        &venv_dir, &runner_script, &code, &output_path,
    )?;

    Ok(format!("STEP exported to {}", output_path))
}
