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
    let mut config = {
        let guard = state
            .config
            .lock()
            .map_err(|e| format!("Failed to lock config: {}", e))?;
        guard.clone()
    };

    if registry::normalize_config_model(&mut config) {
        config.save().map_err(|e| format!("{}", e))?;
        let mut guard = state
            .config
            .lock()
            .map_err(|e| format!("Failed to lock config: {}", e))?;
        *guard = config.clone();
    }

    Ok(config)
}

#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, mut config: AppConfig) -> Result<(), String> {
    registry::normalize_config_model(&mut config);
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
