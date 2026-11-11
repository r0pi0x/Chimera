//! `chimera_asset_server`: the asset service's dev server
//! (docs/plans/active/asset-service.md, section 5; deployment steps in its
//! M2 results). Serves a store (`--store`, default the machine's server
//! store, `<data_local>/chimera/asset-server`, where `chimera_pack publish`
//! writes) to the accounts `chimera_pack user add` made, on plain HTTP at
//! `--listen` (default `127.0.0.1:8731`: this machine only; `tailscale
//! serve` or a reverse proxy puts HTTPS in front). Prints its public key
//! (what clients pin with `[assets] key`) and logs each request to
//! `<store>/server/access.log`. `--latency-ms` delays every answer (to
//! measure clients as over a slower link).
//!
//! `--tls <ip:port>` also serves HTTPS there (`tls::front`: for testers over
//! the internet), with the server's own self-signed certificate
//! (`<store>/server/tls/`, made on first use); it prints the certificate's
//! fingerprint, which game servers hand out at join
//! (`chimera:asset_fingerprint`) and clients pin. `--rate`, `--burst`,
//! `--daily-gb` and `--bad-tokens` set the limits (`server::Limits`).
//! docs/playtest-hosting.md has the steps. A host tool: not in releases.

use std::process::ExitCode;

use chimera_assets::{
    Store,
    server::{DEFAULT_PORT, Service, default_store, serve},
};

const USAGE: &str = "usage: chimera_asset_server [--store <dir>] [--listen <ip:port>] [--tls <ip:port>] [--threads <n>]
       [--latency-ms <n>] [--rate <requests/s>] [--burst <n>] [--daily-gb <n>] [--bad-tokens <n per minute>]
  --listen     plain HTTP, default 127.0.0.1:8731 (keep it on this machine)
  --tls        HTTPS with the server's own certificate (its fingerprint is printed), e.g. 0.0.0.0:27031";

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut store, mut listen, mut threads) = (None, format!("127.0.0.1:{DEFAULT_PORT}"), 8);
    let mut latency = 0u64;
    let mut tls = None;
    let mut limits = chimera_assets::server::Limits::default();
    let number = |v: String, what: &str| v.parse::<f64>().map_err(|_| format!("{what}: a number"));
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{arg} needs a value\n{USAGE}"));
        match arg.as_str() {
            "--store" => store = Some(Store::new(value()?)),
            "--listen" => listen = value()?,
            "--threads" => threads = value()?.parse().map_err(|_| "--threads: a number")?,
            "--latency-ms" => latency = value()?.parse().map_err(|_| "--latency-ms: a number")?,
            "--tls" => tls = Some(value()?),
            "--rate" => limits.rate = number(value()?, arg)?,
            "--burst" => limits.burst = number(value()?, arg)?,
            "--daily-gb" => limits.daily_bytes = (number(value()?, arg)? * 1e9) as u64,
            "--bad-tokens" => limits.bad_tokens = number(value()?, arg)? as u32,
            _ => return Err(USAGE.into()),
        }
    }
    let store = store.or_else(default_store).ok_or("no store: pass --store")?;
    let peers = chimera_assets::tls::Peers::default();
    let service = Service::new(store.clone())
        .map_err(|e| format!("{}: {e}", store.root.display()))?
        .with_latency(std::time::Duration::from_millis(latency))
        .with_limits(limits)
        .with_peers(peers.clone());
    let key = service.public_key();
    let running = serve(service, &listen, threads).map_err(|e| e.to_string())?;
    println!("serving {} at {}", store.root.display(), running.url());
    println!("public key: {key}");
    let _front = match tls {
        Some(at) => {
            let dir = chimera_assets::server::tls_dir(&store);
            let identity =
                chimera_assets::tls::Identity::load_or_create(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let front = chimera_assets::tls::front(&at, running.addr, &identity, peers, Default::default())
                .map_err(|e| e.to_string())?;
            println!("HTTPS at {} (this machine's port; testers use your public address and port)", front.url());
            println!("certificate fingerprint: {}", identity.fingerprint());
            Some(front)
        }
        None => None,
    };
    println!(
        "limits: {} requests/s (bursts of {}), {} a day per account, {} bad tokens a minute per address",
        limits.rate,
        limits.burst,
        if limits.daily_bytes == 0 { "no cap".to_string() } else { chimera_assets::progress::mb(limits.daily_bytes) },
        limits.bad_tokens
    );
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
