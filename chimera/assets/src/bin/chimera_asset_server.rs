//! `chimera_asset_server`: the asset service's dev server
//! (docs/plans/active/asset-service.md, section 5; deployment steps in its
//! M2 results). Serves a store (`--store`, default the machine's server
//! store, `<data_local>/chimera/asset-server`, where `chimera_pack publish`
//! writes) to the accounts `chimera_pack user add` made, on plain HTTP at
//! `--listen` (default `127.0.0.1:8731`: this machine only; `tailscale
//! serve` or a reverse proxy puts HTTPS in front). Prints its public key
//! (what clients pin with `[assets] key`) and logs each request to
//! `<store>/server/access.log`. `--latency-ms` delays every answer (to
//! measure clients as over a slower link). A host tool: not in releases.

use std::process::ExitCode;

use chimera_assets::{
    Store,
    server::{DEFAULT_PORT, Service, default_store, serve},
};

const USAGE: &str = "usage: chimera_asset_server [--store <dir>] [--listen <ip:port>] [--threads <n>] [--latency-ms <n>]";

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut store, mut listen, mut threads) = (None, format!("127.0.0.1:{DEFAULT_PORT}"), 8);
    let mut latency = 0u64;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{arg} needs a value\n{USAGE}"));
        match arg.as_str() {
            "--store" => store = Some(Store::new(value()?)),
            "--listen" => listen = value()?,
            "--threads" => threads = value()?.parse().map_err(|_| "--threads: a number")?,
            "--latency-ms" => latency = value()?.parse().map_err(|_| "--latency-ms: a number")?,
            _ => return Err(USAGE.into()),
        }
    }
    let store = store.or_else(default_store).ok_or("no store: pass --store")?;
    let service = Service::new(store.clone())
        .map_err(|e| format!("{}: {e}", store.root.display()))?
        .with_latency(std::time::Duration::from_millis(latency));
    let key = service.public_key();
    let running = serve(service, &listen, threads).map_err(|e| e.to_string())?;
    println!("serving {} at {}", store.root.display(), running.url());
    println!("public key: {key}");
    for game in store.games() {
        println!("  {game}: builds {}", store.builds(&game).join(", "));
    }
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
