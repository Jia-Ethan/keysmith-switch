use std::fs;
use std::path::Path;

use keysmith_switch_lib::updater::verify_minisign;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!("usage: verify_updater PUBLIC_KEY_FILE ARTIFACT SIGNATURE_FILE");
        std::process::exit(2);
    }
    let pubkey = read_text(&args[0]);
    let artifact = fs::read(&args[1]).unwrap_or_else(|error| fail(&args[1], error));
    let signature = read_text(&args[2]);
    if let Err(error) = verify_minisign(&pubkey, &artifact, &signature) {
        eprintln!("updater signature verification failed: {error}");
        std::process::exit(1);
    }
    println!("updater signature verified");
}

fn read_text(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| fail(path, error))
}

fn fail(path: &str, error: std::io::Error) -> ! {
    eprintln!("failed to read {}: {error}", Path::new(path).display());
    std::process::exit(1);
}
