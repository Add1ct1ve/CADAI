use crate::ai::registry::{self, ProviderInfo};
use crate::config::AppConfig;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub fn get_provider_registry() -> Vec<ProviderInfo> {
    registry::get_provider_registry()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state
        .config
        .lock()
        .map_err(|e| format!("Failed to lock config: {}", e))?;
    Ok(config.clone())
}

#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    // Save to disk
    config.save().map_err(|e| format!("{}", e))?;
    // Update in memory
    let mut current = state
        .config
        .lock()
        .map_err(|e| format!("Failed to lock config: {}", e))?;
    *current = config;
    Ok(())
}
