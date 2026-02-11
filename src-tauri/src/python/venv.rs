use crate::error::AppError;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Get the default venv directory path (in app data).
pub fn get_venv_dir() -> Result<PathBuf, AppError> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| AppError::ConfigError("Cannot find app data directory".into()))?;
    Ok(data_dir.join("cadai-studio").join("venv"))
}

/// Check if a venv exists and is valid.
pub fn venv_exists(venv_dir: &Path) -> bool {
    let python = get_venv_python(venv_dir);
    python.exists()
}

/// Get the path to the Python executable inside a venv.
pub fn get_venv_python(venv_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python")
    }
}

/// Get the path to pip inside a venv.
pub fn get_venv_pip(venv_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        venv_dir.join("Scripts").join("pip.exe")
    } else {
        venv_dir.join("bin").join("pip")
    }
}

/// Create a new venv using the given Python executable.
pub fn create_venv(python_path: &Path, venv_dir: &Path) -> Result<(), AppError> {
    // Ensure parent directory exists
    if let Some(parent) = venv_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let output = Command::new(python_path)
        .args(["-m", "venv", &venv_dir.to_string_lossy()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::CadQueryError(format!(
            "Failed to create venv: {}",
            stderr
        )));
    }

    Ok(())
}
