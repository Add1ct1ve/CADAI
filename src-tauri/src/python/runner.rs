use std::path::Path;
use std::process::Command;
use crate::error::AppError;
use super::venv;

/// Result of executing CadQuery code
pub struct ExecutionResult {
    pub stl_data: Vec<u8>,
    pub stdout: String,
    pub stderr: String,
}

/// Execute CadQuery Python code and return the resulting STL data.
///
/// This writes the code to a temp file, runs the Python runner script,
/// and reads back the generated STL file.
pub fn execute_cadquery(
    venv_dir: &Path,
    runner_script: &Path,
    code: &str,
) -> Result<ExecutionResult, AppError> {
    let python = venv::get_venv_python(venv_dir);

    if !python.exists() {
        return Err(AppError::PythonNotFound);
    }

    // Create temp directory for this execution
    let temp_dir = std::env::temp_dir().join("cadai-studio");
    std::fs::create_dir_all(&temp_dir)?;

    let input_file = temp_dir.join("input.py");
    let output_file = temp_dir.join("output.stl");

    // Write the code to the temp input file
    std::fs::write(&input_file, code)?;

    // Remove any existing output file
    let _ = std::fs::remove_file(&output_file);

    // Run the Python script
    let output = Command::new(&python)
        .args([
            runner_script.to_string_lossy().as_ref(),
            input_file.to_string_lossy().as_ref(),
            output_file.to_string_lossy().as_ref(),
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let exit_code = output.status.code().unwrap_or(-1);
        let error_msg = match exit_code {
            2 => format!("CadQuery execution error:\n{}", stderr),
            3 => "Code must assign final geometry to 'result' variable.".to_string(),
            4 => format!("STL export error:\n{}", stderr),
            _ => format!("Python error (exit code {}):\n{}", exit_code, stderr),
        };
        return Err(AppError::CadQueryError(error_msg));
    }

    // Read the generated STL file
    if !output_file.exists() {
        return Err(AppError::CadQueryError(
            "STL file was not generated".into(),
        ));
    }

    let stl_data = std::fs::read(&output_file)?;

    // Cleanup temp files
    let _ = std::fs::remove_file(&input_file);
    let _ = std::fs::remove_file(&output_file);

    Ok(ExecutionResult {
        stl_data,
        stdout,
        stderr,
    })
}
