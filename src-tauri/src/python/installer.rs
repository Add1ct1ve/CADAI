use std::path::Path;
use std::process::Command;
use crate::error::AppError;
use super::venv;

/// Install CadQuery and dependencies into the venv.
pub fn install_cadquery(venv_dir: &Path) -> Result<(), AppError> {
    let pip = venv::get_venv_pip(venv_dir);

    // Upgrade pip first
    let python = venv::get_venv_python(venv_dir);
    let pip_upgrade = Command::new(&python)
        .args(["-m", "pip", "install", "--upgrade", "pip"])
        .output()?;

    if !pip_upgrade.status.success() {
        let stderr = String::from_utf8_lossy(&pip_upgrade.stderr);
        eprintln!("Warning: pip upgrade failed: {}", stderr);
        // Continue anyway, pip should still work
    }

    // Install cadquery
    let output = Command::new(&pip)
        .args(["install", "cadquery>=2.4.0"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::CadQueryError(format!(
            "Failed to install CadQuery: {}",
            stderr
        )));
    }

    Ok(())
}

/// Check if CadQuery is installed in the venv.
pub fn is_cadquery_installed(venv_dir: &Path) -> bool {
    let python = venv::get_venv_python(venv_dir);

    let output = Command::new(python)
        .args(["-c", "import cadquery; print(cadquery.__version__)"])
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}
