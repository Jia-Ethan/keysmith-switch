// SPDX-License-Identifier: MIT
// Portions adapted from CC Switch (c) 2025 Jason Young
// https://github.com/farion1231/cc-switch
//
// Keysmith Switch keeps the bundle-path rule (macOS must register the .app,
// not the inner Mach-O) and the enable/disable/is_enabled surface. App name
// and paths are Keysmith Switch.

use crate::error::{Error, Result};
use auto_launch::{AutoLaunch, AutoLaunchBuilder};

const APP_NAME: &str = "Keysmith Switch";

/// Map `/path/Keysmith Switch.app/Contents/MacOS/Keysmith Switch` → `.app`.
#[cfg(target_os = "macos")]
fn macos_app_bundle_path(exe_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let path_str = exe_path.to_string_lossy();
    let marker = ".app/Contents/MacOS/";
    let app_pos = path_str.find(marker)?;
    Some(std::path::PathBuf::from(&path_str[..app_pos + 4]))
}

fn auto_launch() -> Result<AutoLaunch> {
    let exe_path =
        std::env::current_exe().map_err(|error| Error::message(format!("exe path: {error}")))?;

    #[cfg(target_os = "macos")]
    let app_path = macos_app_bundle_path(&exe_path).unwrap_or(exe_path);

    #[cfg(not(target_os = "macos"))]
    let app_path = exe_path;

    AutoLaunchBuilder::new()
        .set_app_name(APP_NAME)
        .set_app_path(&app_path.to_string_lossy())
        .build()
        .map_err(|error| Error::message(format!("auto-launch: {error}")))
}

pub fn enable_auto_launch() -> Result<()> {
    auto_launch()?
        .enable()
        .map_err(|error| Error::message(format!("enable auto-launch: {error}")))
}

pub fn disable_auto_launch() -> Result<()> {
    auto_launch()?
        .disable()
        .map_err(|error| Error::message(format!("disable auto-launch: {error}")))
}

pub fn is_auto_launch_enabled() -> Result<bool> {
    auto_launch()?
        .is_enabled()
        .map_err(|error| Error::message(format!("auto-launch status: {error}")))
}

pub fn apply_auto_launch(enabled: bool) -> Result<()> {
    if enabled {
        enable_auto_launch()
    } else {
        disable_auto_launch()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_bundle_path_strips_inner_binary() {
        let exe = std::path::Path::new(
            "/Applications/Keysmith Switch.app/Contents/MacOS/Keysmith Switch",
        );
        assert_eq!(
            super::macos_app_bundle_path(exe),
            Some(std::path::PathBuf::from(
                "/Applications/Keysmith Switch.app"
            ))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_bundle_path_keeps_spaces() {
        let exe = std::path::Path::new(
            "/Users/test/My Apps/Keysmith Switch.app/Contents/MacOS/Keysmith Switch",
        );
        assert_eq!(
            super::macos_app_bundle_path(exe),
            Some(std::path::PathBuf::from(
                "/Users/test/My Apps/Keysmith Switch.app"
            ))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_dev_binary_is_not_a_bundle() {
        let exe = std::path::Path::new("/Users/dev/project/target/debug/keysmith-switch");
        assert_eq!(super::macos_app_bundle_path(exe), None);
    }
}
