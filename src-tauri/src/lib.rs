pub mod adapter;
pub mod commands;
pub mod db;
pub mod diff;
pub mod error;
pub mod lock;
pub mod models;
pub mod official;
pub mod ops;
pub mod paths;
pub mod redact;
pub mod updater;

use tauri::Manager;

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let state = commands::AppState::open().map_err(|error| error.to_string())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_tools,
            commands::list_prompts,
            commands::get_prompt,
            commands::create_prompt,
            commands::update_prompt,
            commands::delete_prompt,
            commands::copy_prompt,
            commands::prompt_history,
            commands::prompt_diff,
            commands::restore_prompt_version,
            commands::tool_status,
            commands::plan_activate,
            commands::activate,
            commands::plan_deactivate,
            commands::deactivate,
            commands::recover_tool,
            commands::doctor,
            commands::list_activations,
            commands::list_operations,
            commands::get_settings,
            commands::update_settings,
            commands::get_about,
            commands::check_app_update,
            commands::install_app_update,
            commands::plan_official_action,
            commands::confirm_official_action,
            commands::list_advanced_tools,
            commands::run_advanced,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running Keysmith Switch");
}
