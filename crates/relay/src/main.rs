//! Minimal Saorsa MASQUE relay. No application tunnel or administrative API.
use saorsa_transport::{NatConfig, P2pConfig, P2pEndpoint, upnp::UpnpConfig};
use std::{net::SocketAddr, process::ExitCode};

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().collect();
    let [_, address] = args.as_slice() else {
        eprintln!("usage: thepeer-relay IP:PORT");
        return ExitCode::FAILURE;
    };
    let Ok(bind) = address.parse::<SocketAddr>() else {
        eprintln!("invalid IP:PORT");
        return ExitCode::FAILURE;
    };
    if bind.port() == 0 || bind.ip().is_loopback() {
        eprintln!("relay needs a non-loopback IP and a fixed port");
        return ExitCode::FAILURE;
    }
    let config = match P2pConfig::builder()
        .bind_addr(bind)
        .max_connections(256)
        .nat(NatConfig {
            enable_relay_service: true,
            advertise_external_addresses: false,
            upnp: UpnpConfig::disabled(),
            ..NatConfig::default()
        })
        .build()
    {
        Ok(config) => config,
        Err(error) => {
            eprintln!("configuration: {error}");
            return ExitCode::FAILURE;
        }
    };
    let endpoint = match P2pEndpoint::new(config).await {
        Ok(endpoint) => endpoint,
        Err(error) => {
            eprintln!("bind: {error}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("MASQUE relay listening on {bind}");
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("signal: {error}");
        return ExitCode::FAILURE;
    }
    endpoint.shutdown().await;
    ExitCode::SUCCESS
}
