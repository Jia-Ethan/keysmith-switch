fn main() {
    println!("cargo:rerun-if-env-changed=KEYSMITH_SWITCH_UPDATER_PUBKEY");
    if let Ok(pubkey) = std::env::var("KEYSMITH_SWITCH_UPDATER_PUBKEY") {
        let pubkey = pubkey.trim();
        assert!(!pubkey.is_empty(), "updater public key must not be empty");
        println!("cargo:rustc-env=KEYSMITH_SWITCH_UPDATER_PUBKEY={pubkey}");
    }
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".into());
    println!("cargo:rustc-env=TARGET_TRIPLE={target}");
    tauri_build::build()
}
