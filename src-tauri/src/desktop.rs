//! Tray, close-to-tray, and window focus helpers.
//!
//! Menu is intentionally small: show main window and quit.
//! Provider/MCP/usage tray items from CC Switch are not carried over.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::logging;

pub const TRAY_ID: &str = "keysmith-switch-tray";

#[cfg(target_os = "macos")]
fn macos_tray_icon() -> Option<tauri::image::Image<'static>> {
    const ICON_BYTES: &[u8] = include_bytes!("../icons/tray/macos/statusbar_template_3x.png");
    match tauri::image::Image::from_bytes(ICON_BYTES) {
        Ok(icon) => Some(icon),
        Err(err) => {
            let _ = logging::write_line(
                "tray",
                &format!("failed to load macOS tray template icon: {err}"),
            );
            None
        }
    }
}

pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let menu = tray_menu(app)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Keysmith Switch")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show_main" => show_main(app),
            "quit" => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    #[cfg(target_os = "macos")]
    {
        if let Some(icon) = macos_tray_icon() {
            builder = builder.icon(icon).icon_as_template(true);
        } else if let Some(icon) = app.default_window_icon().cloned() {
            let _ = logging::write_line(
                "tray",
                "macOS tray template icon failed to load; falling back to default window icon",
            );
            builder = builder.icon(icon);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(icon) = app.default_window_icon().cloned() {
            builder = builder.icon(icon);
        }
    }
    builder.build(app)?;
    Ok(())
}

fn tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let show = MenuItem::with_id(app, "show_main", "显示主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Keysmith Switch", true, None::<&str>)?;
    Menu::with_items(app, &[&show, &PredefinedMenuItem::separator(app)?, &quit])
}

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

pub fn hide_to_tray<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        #[cfg(target_os = "windows")]
        {
            let _ = window.set_skip_taskbar(true);
        }
        #[cfg(target_os = "macos")]
        {
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
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

#[cfg(test)]
mod tests {
    #[test]
    fn macos_tray_template_png_decodes() {
        const ICON_BYTES: &[u8] = include_bytes!("../icons/tray/macos/statusbar_template_3x.png");
        assert_eq!(
            &ICON_BYTES[..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
        let image = tauri::image::Image::from_bytes(ICON_BYTES).expect("decode tray template png");
        assert_eq!(image.width(), 72);
        assert_eq!(image.height(), 72);
        let mut transparent = false;
        let mut visible = false;
        for pixel in image.rgba().chunks_exact(4) {
            let [red, green, blue, alpha] = pixel else {
                unreachable!("RGBA pixels are four bytes");
            };
            transparent |= *alpha == 0;
            visible |= *alpha > 0;
            if *alpha > 0 {
                assert_eq!((*red, *green, *blue), (0, 0, 0));
            }
        }
        assert!(
            transparent,
            "tray template needs transparent background pixels"
        );
        assert!(visible, "tray template needs visible monochrome pixels");
    }
}
