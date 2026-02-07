use std::path::PathBuf;
use std::sync::Mutex;

use crate::config::AppConfig;

#[allow(dead_code)]
pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub python_path: Mutex<Option<PathBuf>>,
    pub venv_path: Mutex<Option<PathBuf>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: Mutex::new(AppConfig::default()),
            python_path: Mutex::new(None),
            venv_path: Mutex::new(None),
        }
    }
}
