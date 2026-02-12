use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use uuid::Uuid;

use super::venv;
use crate::error::AppError;

const DEFAULT_EXECUTION_TIMEOUT_MS: u64 = 30_000;
const POLL_INTERVAL_MS: u64 = 25;

/// Result of executing CadQuery code
pub struct ExecutionResult {
    pub stl_data: Vec<u8>,
    pub stdout: String,
    pub stderr: String,
}

fn create_execution_dir() -> Result<PathBuf, AppError> {
    let dir = std::env::temp_dir()
        .join("cadai-studio")
        .join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn timeout_error(timeout_ms: u64) -> AppError {
    let timeout_s = timeout_ms as f64 / 1000.0;
    AppError::CadQueryError(format!(
        "Execution timed out after {:.1} seconds",
        timeout_s
    ))
}

fn map_runner_error(exit_code: i32, stderr: &str, export_error_label: &str) -> AppError {
    let error_msg = match exit_code {
        2 => format!("CadQuery execution error:\n{}", stderr),
        3 => "Code must assign final geometry to 'result' variable.".to_string(),
        4 => format!("{export_error_label}:\n{}", stderr),
        5 => "Result contains multiple disconnected solids — a cut likely went through a wall and split the body. Reduce cut depth or increase wall thickness.".to_string(),
        _ => format!("Python error (exit code {}):\n{}", exit_code, stderr),
    };
    AppError::CadQueryError(error_msg)
}

fn run_runner_with_timeout(
    python: &Path,
    runner_script: &Path,
    input_file: &Path,
    output_file: &Path,
    timeout_ms: u64,
    execution_dir: &Path,
) -> Result<(std::process::ExitStatus, String, String), AppError> {
    let stdout_path = execution_dir.join("stdout.log");
    let stderr_path = execution_dir.join("stderr.log");
    let stdout_file = std::fs::File::create(&stdout_path)?;
    let stderr_file = std::fs::File::create(&stderr_path)?;

    let mut child = Command::new(python)
        .args([
            runner_script.to_string_lossy().as_ref(),
            input_file.to_string_lossy().as_ref(),
            output_file.to_string_lossy().as_ref(),
        ])
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()?;

    let timeout = Duration::from_millis(timeout_ms.max(1));
    let start = Instant::now();
    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(timeout_error(timeout_ms));
                }
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
        }
    };

    let stdout = std::fs::read_to_string(&stdout_path).unwrap_or_default();
    let stderr = std::fs::read_to_string(&stderr_path).unwrap_or_default();
    Ok((status, stdout, stderr))
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
    execute_cadquery_with_timeout_ms(venv_dir, runner_script, code, DEFAULT_EXECUTION_TIMEOUT_MS)
}

/// Execute CadQuery Python code and return STL data, with a hard timeout.
///
/// Uses a unique per-call temp directory to avoid collisions between concurrent runs.
pub fn execute_cadquery_with_timeout_ms(
    venv_dir: &Path,
    runner_script: &Path,
    code: &str,
    timeout_ms: u64,
) -> Result<ExecutionResult, AppError> {
    let python = venv::get_venv_python(venv_dir);

    if !python.exists() {
        return Err(AppError::PythonNotFound);
    }

    let temp_dir = create_execution_dir()?;
    let input_file = temp_dir.join("input.py");
    let output_file = temp_dir.join("output.stl");

    let result = (|| -> Result<ExecutionResult, AppError> {
        std::fs::write(&input_file, code)?;

        let (status, stdout, stderr) = run_runner_with_timeout(
            &python,
            runner_script,
            &input_file,
            &output_file,
            timeout_ms,
            &temp_dir,
        )?;

        if !status.success() {
            let exit_code = status.code().unwrap_or(-1);
            return Err(map_runner_error(exit_code, &stderr, "STL export error"));
        }

        if !output_file.exists() {
            return Err(AppError::CadQueryError("STL file was not generated".into()));
        }

        let stl_data = std::fs::read(&output_file)?;

        Ok(ExecutionResult {
            stl_data,
            stdout,
            stderr,
        })
    })();

    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}

/// Result of running a generic Python script
pub struct ScriptResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Execute an arbitrary Python script from the venv with given arguments.
/// Returns stdout, stderr, and exit code.
pub fn execute_python_script(
    venv_dir: &Path,
    script: &Path,
    args: &[&str],
) -> Result<ScriptResult, AppError> {
    let python = venv::get_venv_python(venv_dir);

    if !python.exists() {
        return Err(AppError::PythonNotFound);
    }

    let mut cmd_args: Vec<String> = vec![script.to_string_lossy().to_string()];
    for arg in args {
        cmd_args.push(arg.to_string());
    }

    let output = Command::new(&python).args(&cmd_args).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(ScriptResult {
        stdout,
        stderr,
        exit_code,
    })
}

/// Execute an arbitrary Python script with a timeout (in milliseconds).
/// Kills the subprocess if it exceeds the timeout.
/// Uses the same poll-based approach as `run_runner_with_timeout`.
pub fn execute_python_script_with_timeout(
    venv_dir: &Path,
    script: &Path,
    args: &[&str],
    timeout_ms: u64,
) -> Result<ScriptResult, AppError> {
    let python = venv::get_venv_python(venv_dir);
    if !python.exists() {
        return Err(AppError::PythonNotFound);
    }

    let mut cmd_args: Vec<String> = vec![script.to_string_lossy().to_string()];
    for arg in args {
        cmd_args.push(arg.to_string());
    }

    let mut child = Command::new(&python)
        .args(&cmd_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms.max(1));

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut r| {
                        let mut s = String::new();
                        std::io::Read::read_to_string(&mut r, &mut s).ok();
                        s
                    })
                    .unwrap_or_default();
                let stderr = child
                    .stderr
                    .take()
                    .map(|mut r| {
                        let mut s = String::new();
                        std::io::Read::read_to_string(&mut r, &mut s).ok();
                        s
                    })
                    .unwrap_or_default();
                let exit_code = status.code().unwrap_or(-1);
                return Ok(ScriptResult {
                    stdout,
                    stderr,
                    exit_code,
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AppError::CadQueryError(format!(
                        "Script timed out after {:.1}s",
                        timeout_ms as f64 / 1000.0
                    )));
                }
                std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(e) => return Err(AppError::from(e)),
        }
    }
}

/// Execute CadQuery Python code in an isolated temp subdirectory.
///
/// Kept for API compatibility with call sites that explicitly request isolation.
pub fn execute_cadquery_isolated(
    venv_dir: &Path,
    runner_script: &Path,
    code: &str,
) -> Result<ExecutionResult, AppError> {
    execute_cadquery_with_timeout_ms(venv_dir, runner_script, code, DEFAULT_EXECUTION_TIMEOUT_MS)
}

/// Execute CadQuery Python code and export directly to a specific file path.
///
/// The runner script auto-detects the export format based on the output file extension
/// (.step/.stp → STEP, otherwise STL).
pub fn execute_cadquery_to_file(
    venv_dir: &Path,
    runner_script: &Path,
    code: &str,
    output_path: &str,
) -> Result<(), AppError> {
    let python = venv::get_venv_python(venv_dir);

    if !python.exists() {
        return Err(AppError::PythonNotFound);
    }

    let temp_dir = create_execution_dir()?;
    let input_file = temp_dir.join("input.py");
    let output_file = Path::new(output_path);

    let result = (|| -> Result<(), AppError> {
        std::fs::write(&input_file, code)?;

        let (status, _stdout, stderr) = run_runner_with_timeout(
            &python,
            runner_script,
            &input_file,
            output_file,
            DEFAULT_EXECUTION_TIMEOUT_MS,
            &temp_dir,
        )?;

        if !status.success() {
            let exit_code = status.code().unwrap_or(-1);
            return Err(map_runner_error(exit_code, &stderr, "Export error"));
        }

        if !output_file.exists() {
            return Err(AppError::CadQueryError(
                "Export file was not generated".into(),
            ));
        }

        Ok(())
    })();

    let _ = std::fs::remove_dir_all(&temp_dir);
    result
}
