pub mod adapter;
pub mod announcements;
pub mod cleanup;
pub mod commands;
pub mod data;
pub mod db;
pub mod desktop;
pub mod diff;
pub mod error;
pub mod extensions;
pub mod feedback;
pub mod harness;
pub mod lock;
pub mod logging;
pub mod models;
pub mod netproxy;
pub mod nowindow;
pub mod official;
pub mod ops;
pub mod paths;
pub mod redact;
pub mod rewrite;
pub mod updater;

use tauri::Manager;
use tauri_plugin_window_state::StateFlags;

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION)
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            desktop::show_main(app);
        }))
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Closing the window quits. The frontend gets one chance to veto
                // when an unsaved draft would be lost, so the close is held here
                // until it answers.
                api.prevent_close();
                let _ = window.app_handle().emit_to_frontend_close();
            }
        })
        .setup(|app| {
            let state = commands::AppState::open().map_err(|error| error.to_string())?;
            let _ = logging::init(state.store.paths());
            // The relay may have started before this app ever ran; give it current rules.
            let _ = rewrite::publish(&state.store);
            // Bring an installed relay up to this version, off the startup path.
            let paths = state.store.paths().clone();
            std::thread::spawn(move || {
                if let Err(error) = rewrite::service::refresh(&paths) {
                    let _ = logging::write_line("relay-refresh", &error.to_string());
                }
            });
            app.manage(state);
            desktop::show_main(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_tools,
            commands::list_prompts,
            commands::get_prompt,
            commands::create_prompt,
            commands::create_pasted_prompt,
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
            commands::harness_state,
            commands::plan_cleanup,
            commands::confirm_cleanup,
            commands::list_snapshots,
            commands::plan_rollback,
            commands::confirm_rollback,
            commands::delete_snapshot,
            commands::plan_reconcile,
            commands::confirm_reconcile,
            commands::extensions_state,
            commands::announcements_state,
            commands::announcements_refresh,
            commands::announcements_mark,
            commands::extensions_refresh,
            commands::extension_install,
            commands::extension_uninstall,
            commands::adopt_live_prompt,
            commands::remove_harness,
            commands::recover_tool,
            commands::confirm_recover,
            commands::doctor,
            commands::list_activations,
            commands::list_operations,
            commands::get_settings,
            commands::update_settings,
            commands::get_about,
            feedback::submit_feedback,
            commands::check_app_update,
            commands::install_app_update,
            commands::plan_official_action,
            commands::confirm_official_action,
            commands::cancel_official_action,
            commands::list_advanced_tools,
            commands::run_advanced,
            commands::cancel_advanced,
            commands::get_startup_report,
            commands::import_existing_prompts,
            commands::import_markdown_files,
            commands::inspect_zip_archive,
            commands::import_zip_archive,
            commands::export_zip_archive,
            commands::create_backup,
            commands::list_backups,
            commands::restore_backup,
            commands::plan_clear_all_data,
            commands::clear_all_data,
            commands::get_data_dirs,
            commands::acknowledge_recovery,
            commands::log_frontend_error,
            commands::show_main_window,
            commands::quit_app,
            commands::mark_first_run_done,
            commands::rewrite_state,
            commands::rewrite_set_switches,
            commands::rewrite_save_user_rules,
            commands::rewrite_set_table_enabled,
            commands::rewrite_reorder_tables,
            commands::rewrite_copy_to_user,
            commands::rewrite_accept_update,
            commands::rewrite_connect_codex,
            commands::rewrite_disconnect_codex,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running Keysmith Switch");
}

trait EmitClose {
    fn emit_to_frontend_close(&self) -> Result<(), tauri::Error>;
}

impl EmitClose for tauri::AppHandle {
    fn emit_to_frontend_close(&self) -> Result<(), tauri::Error> {
        use tauri::Emitter;
        self.emit("window-close-requested", ())
    }
}
