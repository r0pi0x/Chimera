//! `chimera_front`: the hub's one public TLS port (docs/plans/active/hub.md,
//! 2.3; `chimera_services::front`). Serves the certificate in
//! `<store>/server/tls/` (the asset service's: the fingerprint testers
//! pinned; made there on first use) at `--listen` (else `[front] listen` in
//! `<store>/server/services.toml`) and relays each connection to the
//! service on loopback its path names (`services.toml`'s ports: assets
//! 8731, accounts 8741, hub 8742), adding `X-Chimera-Peer`. `/admin`,
//! `/internal` and `/feedback/admin` never pass. `--connections` sets the
//! connections at once per address. A host tool: not in releases.

use std::process::ExitCode;

use chimera_services::{config::ServicesConfig, front};

const USAGE: &str = "usage: chimera_front [--store <dir>] [--listen <ip:port>] [--connections <n per address>]
  --store   the server store (default: the asset service's, <data_local>/chimera/asset-server)
  --listen  the public address, e.g. 0.0.0.0:<hub port> (default: [front] listen in <store>/server/services.toml)";

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut store, mut listen, mut per_address) = (None, None, None);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value\n{USAGE}"));
        match arg.as_str() {
            "--store" => store = Some(std::path::PathBuf::from(value()?)),
            "--listen" => listen = Some(value()?),
            "--connections" => per_address = Some(value()?.parse::<usize>().map_err(|_| "--connections: a number")?.max(1)),
            _ => return Err(USAGE.into()),
        }
    }
    let store = match store {
        Some(s) => s,
        None => chimera_assets::server::default_store().ok_or("no store: pass --store")?.root,
    };
    let config = ServicesConfig::load(&store)?;
    let listen = listen.or(config.front.listen.clone()).ok_or_else(|| format!("no address: pass --listen or set [front] listen\n{USAGE}"))?;
    let mut limits = chimera_assets::tls::FrontLimits::default();
    if let Some(n) = per_address.or(config.front.per_address) {
        limits.per_address = n;
    }
    let dir = store.join("server").join("tls");
    let identity = chimera_assets::tls::Identity::load_or_create(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let routes = config.routes();
    let running = front::serve(&listen, &identity, routes, limits).map_err(|e| e.to_string())?;
    println!("front at {} (certificate fingerprint {})", running.url(), identity.fingerprint());
    println!(
        "routes: /accounts/ -> {}, /hub -> {}, /feedback -> {}, the rest -> {}; /admin, /internal, /feedback/admin refused",
        routes.accounts, routes.hub, routes.feedback, routes.assets
    );
    println!("connections: {} at once per address, {} in all", limits.per_address, limits.total);
    running.wait();
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
