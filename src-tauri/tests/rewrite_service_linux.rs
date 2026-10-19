//! Installs the relay through the XDG autostart fallback on Linux, checks that it answers,
//! and removes it again. Autostart is forced so the test never touches the real user manager.

#![cfg(target_os = "linux")]

use keysmith_switch_lib::paths::AppPaths;
use keysmith_switch_lib::rewrite::service;

#[test]
fn autostart_install_serves_and_uninstalls() {
    let tmp = tempfile::tempdir().unwrap();
    let autostart = tmp.path().join("autostart");
    let label = format!("keysmith-switch-relay-test-{}", std::process::id());
    std::env::set_var("KEYSMITH_SWITCH_RELAY_SERVICE", "autostart");
    std::env::set_var("KEYSMITH_SWITCH_AUTOSTART_DIR", &autostart);
    std::env::set_var("KEYSMITH_SWITCH_RELAY_LABEL", &label);

    let paths = AppPaths::from_home(tmp.path().join(".keysmith-switch"));
    paths.ensure().unwrap();

    service::install(&paths).unwrap();
    let status = service::status(&paths);
    assert!(status.installed, "autostart entry was not registered");
    assert!(status.running, "relay did not answer after install");

    let entry_path = autostart.join(format!("{label}.desktop"));
    let entry = std::fs::read_to_string(&entry_path).unwrap();
    assert!(entry.contains("[Desktop Entry]"));
    assert!(entry.contains("Name=Keysmith Switch Relay"));
    assert!(entry.contains("X-GNOME-Autostart-enabled=true"));

    service::uninstall(&paths).unwrap();
    assert!(!entry_path.exists(), "autostart entry survived uninstall");
    assert!(!service::status(&paths).installed);
    assert!(!service::status(&paths).running, "relay is still running");
}
