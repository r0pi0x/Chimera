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
//! `--daily-gb` and `--bad-tokens` set the limits (`server::Limits`),
//! `--connections` the HTTPS connections at once per address
//! (`tls::FrontLimits`).
//!
//! Feedback reports (`feedback::inbox`): `POST /feedback` takes them with
//! the testers' tokens; `--reports-per-hour`, `--reports-per-day` and
//! `--reports-per-address` set its limits, `--reports-keep-days` (60) and
//! `--reports-keep-mb` (2048) its retention (0: none); `--on-report <command>` (else
//! `[feedback] on_report` in the local config) runs a command for each new
//! one (a Discord ping). The triage page is at
//! `http://127.0.0.1:<port>/feedback/admin/` on this machine only (a proxy on
//! this machine may use this machine's own name or a Tailscale `*.ts.net`
//! name as `Host`; `--admin-host <name>`, or `admin_hosts = [...]` under
//! `[feedback]`, adds others).
//! docs/playtest-hosting.md has the steps. A host tool: not in releases.

use std::process::ExitCode;

use chimera_assets::{
    Store,
    server::{DEFAULT_PORT, Service, default_store, serve},
};

const USAGE: &str = "usage: chimera_asset_server [--store <dir>] [--listen <ip:port>] [--tls <ip:port>] [--threads <n>]
       [--latency-ms <n>] [--rate <requests/s>] [--burst <n>] [--daily-gb <n>] [--bad-tokens <n per minute>]
       [--connections <n per address>] [--on-report <command>] [--admin-host <name>]... [--reports-per-hour <n>]
       [--reports-per-day <n>] [--reports-per-address <n per hour>] [--reports-keep-days <n>]
       [--reports-keep-mb <n>] [--accounts <url> | --no-tickets]
  --listen     plain HTTP, default 127.0.0.1:8731 (keep it on this machine)
  --tls        HTTPS with the server's own certificate (its fingerprint is printed), e.g. 0.0.0.0:27031
  --on-report  a command run for each new feedback report (default: [feedback] on_report in the local config)
  --admin-host another Host name the triage page answers to, from this machine only (repeatable; besides\n               localhost, this machine's name and *.ts.net; default: admin_hosts under [feedback] in the local config)
  --reports-keep-days, --reports-keep-mb  delete reports older than that (default 60 days), then the oldest
               past that total (default 2048 MB); 0: keep
  --accounts   the accounts service on this machine, for the revocation feed (default http://127.0.0.1:8741)
  --no-tickets legacy tokens only (no account tickets)";

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut store, mut listen, mut threads) = (None, format!("127.0.0.1:{DEFAULT_PORT}"), 8);
    let mut latency = 0u64;
    let mut tls = None;
    let mut limits = chimera_assets::server::Limits::default();
    let mut front_limits = chimera_assets::tls::FrontLimits::default();
    let mut feedback_limits = chimera_assets::feedback::inbox::FeedbackLimits::default();
    let mut on_report = None;
    let mut admin_hosts: Vec<String> = Vec::new();
    let mut accounts = Some("http://127.0.0.1:8741".to_string());
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
            "--connections" => front_limits.per_address = number(value()?, arg)?.max(1.0) as usize,
            "--on-report" => on_report = Some(value()?),
            "--admin-host" => admin_hosts.push(value()?),
            "--accounts" => accounts = Some(value()?),
            "--no-tickets" => accounts = None,
            "--reports-per-hour" => feedback_limits.per_account_hour = number(value()?, arg)? as usize,
            "--reports-per-day" => feedback_limits.per_account_day = number(value()?, arg)? as usize,
            "--reports-per-address" => feedback_limits.per_address_hour = number(value()?, arg)? as usize,
            "--reports-keep-days" => feedback_limits.keep_days = number(value()?, arg)? as u64,
            "--reports-keep-mb" => feedback_limits.keep_bytes = (number(value()?, arg)? * 1048576.0) as u64,
            _ => return Err(USAGE.into()),
        }
    }
    let store = store.or_else(default_store).ok_or("no store: pass --store")?;
    let local = chimera_assets::feedback::LocalFeedback::load()?;
    let on_report = on_report.or(local.on_report);
    if admin_hosts.is_empty() {
        admin_hosts = local.admin_hosts;
    }
    let peers = chimera_assets::tls::Peers::default();
    let service = match &accounts {
        Some(url) => Service::with_tickets(store.clone(), url, chimera_assets::accounts::FEED_EVERY),
        None => Service::new(store.clone()),
    };
    let service = service
        .map_err(|e| format!("{}: {e}", store.root.display()))?
        .with_latency(std::time::Duration::from_millis(latency))
        .with_limits(limits)
        .with_peers(peers.clone())
        .with_feedback(feedback_limits, on_report.clone())
        .with_admin_hosts(admin_hosts);
    let key = service.public_key();
    let running = serve(service, &listen, threads).map_err(|e| e.to_string())?;
    println!("serving {} at {}", store.root.display(), running.url());
    println!("public key: {key}");
    match &accounts {
        Some(url) => println!("account tickets: checked with {}, revocations from {url}", chimera_assets::accounts::ticket_keys_path(&store.root).display()),
        None => println!("account tickets: off (legacy tokens only)"),
    }
    let _front = match tls {
        Some(at) => {
            let dir = chimera_assets::server::tls_dir(&store);
            let identity =
                chimera_assets::tls::Identity::load_or_create(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let front = chimera_assets::tls::front(&at, running.addr, &identity, peers, front_limits)
                .map_err(|e| e.to_string())?;
            println!("HTTPS at {} (this machine's port; testers use your public address and port)", front.url());
            println!("certificate fingerprint: {}", identity.fingerprint());
            println!("connections: {} at once per address", front_limits.per_address);
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
    println!(
        "feedback: POST /feedback, {} reports an hour and {} a day per account, {} an hour per address{}; \
         the page at {}/feedback/admin/ (this machine only)",
        feedback_limits.per_account_hour,
        feedback_limits.per_account_day,
        feedback_limits.per_address_hour,
        if on_report.is_some() { ", a command per report" } else { "" },
        running.url()
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
