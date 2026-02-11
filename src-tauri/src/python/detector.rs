use crate::error::AppError;
use std::path::PathBuf;
use std::process::Command;

/// Represents a detected Python installation
pub struct PythonInfo {
    pub path: PathBuf,
    pub version: String,
}

/// Try to find a Python 3.x installation on the system.
/// Returns the path to the Python executable and its version.
pub fn detect_python() -> Result<PythonInfo, AppError> {
    // Try candidates in order of preference
    let candidates = get_candidates();

    for candidate in &candidates {
        if let Ok(info) = try_python(candidate) {
            // Only accept Python 3.8+
            if is_version_compatible(&info.version) {
                return Ok(info);
            }
        }
    }

    Err(AppError::PythonNotFound)
}

fn get_candidates() -> Vec<String> {
    if cfg!(target_os = "windows") {
        vec![
            "py".to_string(),
            "python".to_string(),
            "python3".to_string(),
        ]
    } else {
        vec!["python3".to_string(), "python".to_string()]
    }
}

fn try_python(candidate: &str) -> Result<PythonInfo, AppError> {
    let mut args = vec![];
    let cmd;

    if cfg!(target_os = "windows") && candidate == "py" {
        cmd = "py";
        args.push("-3");
        args.push("--version");
    } else {
        cmd = candidate;
        args.push("--version");
    }

    let output = Command::new(cmd)
        .args(&args)
        .output()
        .map_err(|_| AppError::PythonNotFound)?;

    if !output.status.success() {
        return Err(AppError::PythonNotFound);
    }

    let version_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // "Python 3.11.5" -> "3.11.5"
    let version = version_str
        .strip_prefix("Python ")
        .unwrap_or(&version_str)
        .to_string();

    // Get the actual path to the python executable
    let path_args = if cfg!(target_os = "windows") && candidate == "py" {
        vec!["-3", "-c", "import sys; print(sys.executable)"]
    } else {
        vec!["-c", "import sys; print(sys.executable)"]
    };

    let path_output = Command::new(cmd)
        .args(&path_args)
        .output()
        .map_err(|_| AppError::PythonNotFound)?;

    let path = PathBuf::from(String::from_utf8_lossy(&path_output.stdout).trim());

    Ok(PythonInfo { path, version })
}

fn is_version_compatible(version: &str) -> bool {
    // Parse "3.11.5" -> (3, 11)
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 2 {
        return false;
    }

    let major: u32 = parts[0].parse().unwrap_or(0);
    let minor: u32 = parts[1].parse().unwrap_or(0);

    major == 3 && minor >= 8
}
