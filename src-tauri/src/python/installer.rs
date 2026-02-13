use super::venv;
use crate::error::AppError;
use std::path::Path;
use std::process::Command;

/// Install Build123d and dependencies into the venv.
pub fn install_build123d(venv_dir: &Path) -> Result<(), AppError> {
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

    // Install build123d
    let output = Command::new(&pip)
        .args(["install", "build123d"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::CadError(format!(
            "Failed to install build123d: {}",
            stderr
        )));
    }

    Ok(())
}

/// Check if Build123d is installed in the venv.
pub fn is_build123d_installed(venv_dir: &Path) -> bool {
    let python = venv::get_venv_python(venv_dir);

    let output = Command::new(python)
        .args(["-c", "import build123d; print(build123d.__version__)"])
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Detect the installed Build123d version string.
pub fn detect_build123d_version(venv_dir: &Path) -> Option<String> {
    let python = venv::get_venv_python(venv_dir);
    let output = Command::new(python)
        .args(["-c", "import build123d; print(build123d.__version__)"])
        .output()
        .ok()?;
    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !version.is_empty() {
            Some(version)
        } else {
            None
        }
    } else {
        None
    }
}

/// Parse "2.4.0" → (2, 4, 0). Returns None if unparseable.
pub fn parse_version(version: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts.get(2).and_then(|p| p.parse().ok()).unwrap_or(0);
        Some((major, minor, patch))
    } else {
        None
    }
}

/// Check if `installed` >= `required`.
pub fn version_gte(installed: &str, required: &str) -> bool {
    match (parse_version(installed), parse_version(required)) {
        (Some(i), Some(r)) => i >= r,
        _ => true, // if can't parse, assume compatible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_full() {
        assert_eq!(parse_version("2.4.0"), Some((2, 4, 0)));
    }

    #[test]
    fn test_parse_version_partial() {
        assert_eq!(parse_version("2.4"), Some((2, 4, 0)));
    }

    #[test]
    fn test_parse_version_invalid() {
        assert_eq!(parse_version("abc"), None);
    }

    #[test]
    fn test_parse_version_single() {
        assert_eq!(parse_version("2"), None);
    }

    #[test]
    fn test_version_gte_equal() {
        assert!(version_gte("2.4.0", "2.4.0"));
    }

    #[test]
    fn test_version_gte_greater() {
        assert!(version_gte("2.5.0", "2.4.0"));
    }

    #[test]
    fn test_version_gte_less() {
        assert!(!version_gte("2.3.0", "2.4.0"));
    }

    #[test]
    fn test_version_gte_unparseable_returns_true() {
        assert!(version_gte("unknown", "2.4.0"));
    }

    #[test]
    fn test_version_gte_minor_difference() {
        assert!(version_gte("2.4.1", "2.4.0"));
        assert!(!version_gte("2.3.9", "2.4.0"));
    }
}
