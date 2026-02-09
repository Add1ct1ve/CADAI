pub mod cad;
pub mod chat;
pub mod drawing;
pub mod manufacturing;
pub mod parallel;
pub mod project;
pub mod settings;

use crate::error::AppError;

/// Find a Python script in the python/ directory by name.
/// Shared by cad, drawing, and manufacturing commands.
pub(crate) fn find_python_script(name: &str) -> Result<std::path::PathBuf, AppError> {
    let candidates = vec![
        std::env::current_dir()
            .unwrap_or_default()
            .join("python")
            .join(name),
        std::env::current_exe()
            .unwrap_or_default()
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("python")
            .join(name),
        std::env::current_dir()
            .unwrap_or_default()
            .join("..")
            .join("python")
            .join(name),
    ];

    for candidate in candidates {
        if let Ok(path) = candidate.canonicalize() {
            if path.exists() {
                return Ok(path);
            }
        }
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(AppError::ConfigError(format!("{} not found", name)))
}
