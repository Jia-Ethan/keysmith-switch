//! Window focus and quit helpers.
//!
//! There is no tray: closing the window is quitting the app. `show_main` stays
//! because a second launch (or a refused unsaved-draft confirm) must be able to
//! bring the existing window forward.

use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::logging;

pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        #[cfg(target_os = "windows")]
        {
            let _ = window.set_skip_taskbar(false);
        }
        #[cfg(target_os = "macos")]
        {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        }
    }
}

pub fn request_quit<R: Runtime>(app: &AppHandle<R>) {
    let _ = logging::write_line("lifecycle", "quit requested");
    let _ = app.emit("app-quit-requested", ());
}

pub fn force_quit<R: Runtime>(app: &AppHandle<R>) {
    let _ = logging::write_line("lifecycle", "quit");
    app.exit(0);
}
