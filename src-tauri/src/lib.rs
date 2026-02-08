mod agent;
mod ai;
mod commands;
mod config;
mod error;
mod python;
mod state;

use state::AppState;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load persisted config (or use defaults)
    let loaded_config = config::AppConfig::load().unwrap_or_default();
    let app_state = AppState {
        config: std::sync::Mutex::new(loaded_config),
        python_path: std::sync::Mutex::new(None),
        venv_path: std::sync::Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::chat::send_message,
            commands::chat::auto_retry,
            commands::cad::execute_code,
            commands::cad::check_python,
            commands::cad::setup_python,
            commands::settings::get_provider_registry,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::project::save_project,
            commands::project::load_project,
            commands::project::export_stl,
            commands::project::export_step,
            commands::parallel::generate_parallel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
