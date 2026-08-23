fn main() {
    println!("cargo:rerun-if-env-changed=KEYSMITH_SWITCH_UPDATER_PUBKEY");
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".into());
    println!("cargo:rustc-env=TARGET_TRIPLE={target}");
    tauri_build::build()
}
