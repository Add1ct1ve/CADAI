use serde::Serialize;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AppError {
    #[error("Python not found on system")]
    PythonNotFound,

    #[error("CadQuery error: {0}")]
    CadQueryError(String),

    #[error("AI provider error: {0}")]
    AiProviderError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

// Implement Serialize manually so AppError can be returned from Tauri commands.
// Tauri requires command return errors to be Serialize. We serialize as the Display string.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
