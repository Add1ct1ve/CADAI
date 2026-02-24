mod agent;
mod ai;
mod commands;
mod config;
mod error;
mod mechanisms;
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
        session_memory: std::sync::Mutex::new(agent::memory::SessionMemory::new()),
        build123d_version: std::sync::Mutex::new(None),
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
            commands::chat::clear_session_memory,
            commands::cad::execute_code,
            commands::cad::check_python,
            commands::cad::setup_python,
            commands::cad::import_cad_file,
            commands::settings::get_provider_registry,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::project::save_project,
            commands::project::load_project,
            commands::project::export_stl,
            commands::project::export_step,
            commands::parallel::generate_parallel,
            commands::parallel::generate_design_plan,
            commands::parallel::generate_from_plan,
            commands::parallel::retry_skipped_steps,
            commands::parallel::retry_part,
            commands::drawing::generate_drawing_view,
            commands::drawing::export_drawing_pdf,
            commands::drawing::export_drawing_dxf,
            commands::manufacturing::export_3mf,
            commands::manufacturing::mesh_check,
            commands::manufacturing::orient_for_print,
            commands::manufacturing::sheet_metal_unfold,
            commands::mechanisms::list_mechanisms,
            commands::mechanisms::get_mechanism,
            commands::mechanisms::search_mechanisms,
            commands::mechanisms::install_mechanism_pack,
            commands::mechanisms::remove_mechanism_pack,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
