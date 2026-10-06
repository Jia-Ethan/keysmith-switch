// No console window when Windows starts the relay at sign-in.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;

use keysmith_relay::{default_home, serve, Relay, VERSION};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut home: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => {
                println!("keysmith-relay {VERSION}");
                return ExitCode::SUCCESS;
            }
            "--home" => home = args.next().map(PathBuf::from),
            other => {
                eprintln!("keysmith-relay: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(home) = home.or_else(default_home) else {
        eprintln!("keysmith-relay: cannot find the Keysmith Switch data folder");
        return ExitCode::from(2);
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("keysmith-relay: {error}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(async move {
        let relay = Relay::new(&home);
        let Some(config) = relay.config() else {
            eprintln!("keysmith-relay: relay.json is missing or invalid");
            return ExitCode::from(3);
        };
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, config.port));
        let listener = match tokio::net::TcpListener::bind(address).await {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("keysmith-relay: cannot listen on {address}: {error}");
                return ExitCode::from(4);
            }
        };
        eprintln!("keysmith-relay {VERSION} listening on {address}");
        tokio::select! {
            result = serve(relay, listener) => {
                if let Err(error) = result {
                    eprintln!("keysmith-relay: {error}");
                    return ExitCode::FAILURE;
                }
                ExitCode::SUCCESS
            }
            _ = tokio::signal::ctrl_c() => ExitCode::SUCCESS,
        }
    })
}
