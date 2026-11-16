//! `chimera_hub`: the hub service (docs/plans/active/hub.md, section 4;
//! `chimera_services::hub`): the signed discovery document, the server
//! registry with its heartbeats and reachability probe, and the list, on
//! loopback (`--listen`, default 127.0.0.1:8742 or `services.toml`'s
//! port) behind the front at `/hub`. Its listings are kept in
//! `<store>/server/hub/hub.db`, its settings read from
//! `<store>/server/hub/hub.toml` (name, MOTD, news, `public_address`,
//! `min_net`, ...; read again when edited). Tickets are checked with the
//! accounts service's public key in the store and its revocation feed
//! (`--accounts`, default http://127.0.0.1:8741 or `services.toml`'s
//! port). A host tool: not in releases.

use std::{path::PathBuf, process::ExitCode};

use chimera_services::{
    accounts::Clock,
    config::ServicesConfig,
    hub::{Hub, Options, Paths, describe_config, http},
};

const USAGE: &str = "usage: chimera_hub [serve] [--store <dir>] [--listen <ip:port>] [--accounts <url>] [--threads <n>]
  --store     the server store (default: the asset service's, <data_local>/chimera/asset-server)
  --listen    the loopback address (default 127.0.0.1:8742, or services.toml's [ports] hub)
  --accounts  the accounts service, for its revocation feed (default http://127.0.0.1:8741)";

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut store, mut listen, mut accounts, mut threads) = (None, None, None, 4usize);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value\n{USAGE}"));
        match arg.as_str() {
            "serve" => {}
            "--store" => store = Some(PathBuf::from(value()?)),
            "--listen" => listen = Some(value()?),
            "--accounts" => accounts = Some(value()?),
            "--threads" => threads = value()?.parse::<usize>().map_err(|_| "--threads: a number")?.max(1),
            _ => return Err(USAGE.into()),
        }
    }
    let store = match store {
        Some(s) => s,
        None => chimera_assets::server::default_store().ok_or("no store: pass --store")?.root,
    };
    let config = ServicesConfig::load(&store)?;
    let listen = listen.unwrap_or_else(|| format!("127.0.0.1:{}", config.ports.hub));
    let accounts = accounts.unwrap_or_else(|| format!("http://127.0.0.1:{}", config.ports.accounts));
    let paths = Paths::new(store);
    let hub = Hub::open(paths.clone(), Options::default(), Clock::default(), Some(accounts.clone()))?;
    let running = http::serve(http::Http::new(hub.clone()), &listen, threads).map_err(|e| e.to_string())?;
    println!("hub service: {} at {} (database version {})", paths.db().display(), running.url(), hub.db_version());
    println!("settings: {}", describe_config(&paths.config()));
    println!("tickets: {} with revocations from {accounts}", paths.ticket_pub().display());
    println!("document key: {} (the asset service's manifest key)", hub.public_key());
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
