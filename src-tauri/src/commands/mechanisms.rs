use serde::Serialize;
use tauri::State;

use crate::error::AppError;
use crate::mechanisms::catalog;
use crate::mechanisms::importer;
use crate::mechanisms::schema::{CatalogMechanism, CatalogPackage, MechanismImportReport};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct MechanismListResponse {
    pub packages: Vec<CatalogPackage>,
    pub mechanisms: Vec<CatalogMechanism>,
}

#[tauri::command]
pub fn list_mechanisms(state: State<'_, AppState>) -> Result<MechanismListResponse, AppError> {
    let config = state
        .config
        .lock()
        .map_err(|e| AppError::ConfigError(format!("Failed to lock config: {}", e)))?
        .clone();

    let catalog = catalog::get_catalog(&config)?;
    Ok(MechanismListResponse {
        packages: catalog.packages,
        mechanisms: catalog.mechanisms,
    })
}

#[tauri::command]
pub fn get_mechanism(
    state: State<'_, AppState>,
    mechanism_id: String,
) -> Result<Option<CatalogMechanism>, AppError> {
    let config = state
        .config
        .lock()
        .map_err(|e| AppError::ConfigError(format!("Failed to lock config: {}", e)))?
        .clone();

    catalog::get_mechanism_by_id(&config, &mechanism_id)
}

#[tauri::command]
pub fn search_mechanisms(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<CatalogMechanism>, AppError> {
    let config = state
        .config
        .lock()
        .map_err(|e| AppError::ConfigError(format!("Failed to lock config: {}", e)))?
        .clone();

    let cap = limit.unwrap_or(30).clamp(1, 200) as usize;
    catalog::search_mechanisms(&config, &query, cap)
}

#[tauri::command]
pub async fn install_mechanism_pack(
    state: State<'_, AppState>,
    manifest_url: String,
) -> Result<MechanismImportReport, AppError> {
    let config = state
        .config
        .lock()
        .map_err(|e| AppError::ConfigError(format!("Failed to lock config: {}", e)))?
        .clone();

    importer::install_pack_from_url(&config, &manifest_url).await
}

#[tauri::command]
pub fn remove_mechanism_pack(
    _state: State<'_, AppState>,
    package_id: String,
) -> Result<bool, AppError> {
    importer::remove_imported_pack(&package_id)
}
