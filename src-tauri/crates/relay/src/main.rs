// No console window when Windows starts the relay at sign-in.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
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
        let loopback = SocketAddr::from((Ipv4Addr::LOCALHOST, config.port));
        let listener = match tokio::net::TcpListener::bind(loopback).await {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("keysmith-relay: cannot listen on {loopback}: {error}");
                return ExitCode::from(4);
            }
        };
        let mut listeners = vec![(loopback, listener)];
        let mut seen = BTreeSet::from([IpAddr::V4(Ipv4Addr::LOCALHOST)]);
        for raw in config.listen_addresses {
            let Ok(ip) = raw.parse::<IpAddr>() else {
                eprintln!("keysmith-relay: ignoring invalid listen address");
                continue;
            };
            if ip.is_unspecified() || ip.is_multicast() || !seen.insert(ip) {
                continue;
            }
            let address = SocketAddr::new(ip, config.port);
            match tokio::net::TcpListener::bind(address).await {
                Ok(listener) => listeners.push((address, listener)),
                Err(error) => eprintln!("keysmith-relay: cannot listen on {address}: {error}"),
            }
        }
        let mut servers = tokio::task::JoinSet::new();
        for (address, listener) in listeners {
            eprintln!("keysmith-relay {VERSION} listening on {address}");
            servers.spawn(serve(relay.clone(), listener));
        }
        tokio::select! {
            result = servers.join_next() => {
                match result {
                    Some(Ok(Err(error))) => eprintln!("keysmith-relay: {error}"),
                    Some(Err(error)) => eprintln!("keysmith-relay: listener task failed: {error}"),
                    _ => {}
                }
                ExitCode::FAILURE
            }
            _ = tokio::signal::ctrl_c() => ExitCode::SUCCESS,
        }
    })
}
