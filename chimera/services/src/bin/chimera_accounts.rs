//! `chimera_accounts`: the hub's accounts service and the owner's command
//! line for it (docs/plans/active/hub.md, 2.4, 3, 5, 7.1).
//!
//! `chimera_accounts serve` runs the service on loopback (`--listen`,
//! default 127.0.0.1:8741) over `<store>/server/accounts/accounts.db`
//! (`--store`, default the machine's server store, the asset service's),
//! writes the ticket public key for the services on this box
//! (`accounts/ticket.pub`), makes the services' shared secret if missing
//! (`services/secret`), and backs the database up nightly
//! (`<store>/server/backups/`). `--tls <ip:port>` also serves HTTPS there
//! with the asset service's certificate (until the front, S1b);
//! `--public <host:port>` is the hub's public address, for the invite
//! links it prints.
//!
//! Every other command asks the running service (its admin API, with the
//! secret from the store): `invite <name>` (prints the link or code once),
//! `invite revoke <name>`, `invites`, `account list`, `account show
//! <username>`, `account set <username> ...`, `account refresh
//! <username>` (its tickets asked again, so new rights apply at once),
//! `account revoke <username>`, `account delete <username>`, `device
//! revoke <id>`, `ban <username|address>`, `import-legacy
//! [<accounts.toml>]`, `backup`; `page` prints the admin page's address
//! with its key (`accounts::page`). `restore <backup>` puts a backup in
//! place while the service is stopped. `status` asks every service on
//! this machine (`services.toml`'s ports: accounts, assets, the hub, the
//! front over TLS) for its `/health` line; `install-units` writes the
//! systemd user units (`units`) and prints the steps after. A host tool:
//! not in releases.

use std::{path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use chimera_services::{
    accounts::{
        AccountChange, AccountDetail, AccountView, Accounts, Clock, DEFAULT_PORT, Imported, InviteSpec, InviteView,
        Invited, Options, Paths,
        http::{Http, serve},
    },
    admin::Admin,
    config::ServicesConfig,
    db, units,
};

const USAGE: &str = "usage: chimera_accounts <command> [--store <dir>] [--accounts <url>]
  serve [--listen <ip:port>] [--tls <ip:port>] [--public <host:port>] [--threads <n>]
  invite <name> [--role tester|host|owner] [--content a,b|*] [--games a,b|*] [--list <n>]
                [--expires <days>] [--for <username> [--keep]] [--hub <host:port>]
  invite revoke <name>        invites
  account list [--json]       account show <username> [--json]
  account set <username> [--username <new>] [--display-name <name>] [--role tester|host|owner]
              [--content a,b|*] [--games a,b|*] [--list <n>] [--note <text>] [--mute | --unmute]
  account refresh <username>  (its tickets asked again: rights at once)
  account revoke <username>   account delete <username>
  device revoke <device id>
  ban <username|address> [--for <days>] --reason <text>
  import-legacy [<accounts.toml>]   (default: the store's server/accounts.toml)
  backup                      restore <backup file>   (the service stopped)
  page                        (the admin page's address, with its key: open it on this machine)
  status                      (every service on this machine: front, accounts, assets, hub)
  install-units [--bin <dir>] [--front <ip:port>] [--to <dir>] [--assets-args \"<args>\"]
                [--accounts-args \"<args>\"] [--rw <dir>]   (systemd user units)
  --store     the server store (default: the asset service's, <data_local>/chimera/asset-server)
  --accounts  the running service (default http://127.0.0.1:8741, or services.toml's port)";

struct Args {
    words: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse() -> Result<Self, String> {
        const SWITCHES: &[&str] = &["--keep", "--json", "--mute", "--unmute"];
        let mut words = Vec::new();
        let mut flags = Vec::new();
        let mut it = std::env::args().skip(1);
        while let Some(a) = it.next() {
            if a == "--help" || a == "-h" {
                return Err(USAGE.into());
            }
            if a.starts_with("--") {
                let value = if SWITCHES.contains(&a.as_str()) {
                    None
                } else {
                    Some(it.next().ok_or_else(|| format!("{a} needs a value\n{USAGE}"))?)
                };
                flags.push((a, value));
            } else {
                words.push(a);
            }
        }
        Ok(Self { words, flags })
    }

    fn flag(&self, name: &str) -> Option<String> {
        self.flags.iter().rev().find(|(n, _)| n == name).and_then(|(_, v)| v.clone())
    }

    fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(n, _)| n == name)
    }

    fn paths(&self) -> Result<Paths, String> {
        match self.flag("--store") {
            Some(s) => Ok(Paths::new(PathBuf::from(s))),
            None => chimera_assets::server::default_store().map(|s| Paths::new(s.root)).ok_or("no store: pass --store".into()),
        }
    }

    fn url(&self) -> String {
        self.flag("--accounts").unwrap_or_else(|| {
            let port = self.services().map_or(DEFAULT_PORT, |c| c.ports.accounts);
            format!("http://127.0.0.1:{port}")
        })
    }

    /// `services.toml` in the store (the defaults without one).
    fn services(&self) -> Result<ServicesConfig, String> {
        ServicesConfig::load(&self.paths()?.store)
    }

    fn admin(&self) -> Result<Admin, String> {
        let path = self.paths()?.secret();
        let secret = std::fs::read_to_string(&path)
            .map_err(|e| format!("{}: {e} (start the service once: chimera_accounts serve)", path.display()))?;
        Ok(Admin::new(&self.url(), secret.trim()))
    }

    fn number(&self, name: &str) -> Result<Option<u64>, String> {
        self.flag(name).map(|v| v.parse().map_err(|_| format!("{name}: a number"))).transpose()
    }
}

fn when(t: u64) -> String {
    chimera_assets::accounts::utc_text(t)
}

fn run() -> Result<(), String> {
    let args = Args::parse()?;
    let words: Vec<&str> = args.words.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["serve"] => {
            let paths = args.paths()?;
            let listen = args.flag("--listen").unwrap_or_else(|| format!("127.0.0.1:{DEFAULT_PORT}"));
            let threads = args.number("--threads")?.unwrap_or(4) as usize;
            let options = Options { public: args.flag("--public"), ..Options::default() };
            let accounts = Arc::new(Accounts::open(paths.clone(), options, Clock::default())?);
            let peers = chimera_assets::tls::Peers::default();
            let running = serve(Http::new(accounts.clone(), Some(peers.clone())), &listen, threads, Some(Duration::from_secs(600)))
                .map_err(|e| e.to_string())?;
            println!("accounts service: {} at {} (database version {})", paths.db().display(), running.url(), accounts.db_version());
            println!("ticket public key: {}", paths.ticket_pub().display());
            let _front = match args.flag("--tls") {
                Some(at) => {
                    let identity = chimera_assets::tls::Identity::load_or_create(&paths.tls())
                        .map_err(|e| format!("{}: {e}", paths.tls().display()))?;
                    let front = chimera_assets::tls::front(&at, running.addr, &identity, peers, Default::default())
                        .map_err(|e| e.to_string())?;
                    println!("HTTPS at {} (certificate fingerprint {})", front.url(), identity.fingerprint());
                    Some(front)
                }
                None => None,
            };
            running.wait();
            Ok(())
        }
        ["invite", "revoke", name] => {
            let n: usize = args.admin()?.post("/admin/invite/revoke", &serde_json::json!({ "name": name }))?;
            println!("{n} unused invite(s) named {name:?} revoked");
            Ok(())
        }
        ["invite", name] => {
            let spec = InviteSpec {
                name: name.to_string(),
                role: args.flag("--role"),
                content: args.flag("--content"),
                games: args.flag("--games"),
                list: args.number("--list")?.map(|n| n as u32),
                expires_days: args.number("--expires")?,
                for_username: args.flag("--for"),
                keep: args.has("--keep"),
            };
            let invited: Invited = args.admin()?.post("/admin/invite", &spec)?;
            let link = invited.link.clone().or_else(|| {
                let host = args.flag("--hub")?;
                let fingerprint = chimera_assets::tls::fingerprint_in(&args.paths().ok()?.tls());
                Some(chimera_assets::account::HubLink { host, fingerprint, code: Some(invited.code.clone()) }.to_text(true))
            });
            match link {
                Some(link) => println!("{link}"),
                None => println!(
                    "code {} (the link: chimera-hub://<hub host>:<hub port>/<fingerprint>?code={}; --hub names the host)",
                    invited.code, invited.code
                ),
            }
            eprintln!("for {:?}, valid until {}; send it privately, it is shown only now", invited.name, when(invited.expires));
            Ok(())
        }
        ["invites"] => {
            let list: Vec<InviteView> = args.admin()?.get("/admin/invites")?;
            for i in list {
                let target = i.for_username.map(|u| format!(" (re-invite into {u})")).unwrap_or_default();
                println!("{:<9} {:<24} {:<7} until {}{target}", i.state, i.name, i.role, when(i.expires));
            }
            Ok(())
        }
        ["account", "list"] | ["list"] => {
            let list: Vec<AccountView> = args.admin()?.get("/admin/accounts")?;
            if args.has("--json") {
                println!("{}", serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?);
                return Ok(());
            }
            for a in list {
                let live = a.devices.iter().filter(|d| !d.revoked).count();
                let seen = a.last_seen.map_or("never".into(), when);
                println!("{:<20} {:<20} {:<6} {:<7} devices {live}/{} seen {seen}", a.username, a.display_name, a.role, a.state, a.devices.len());
                for d in &a.devices {
                    println!("    {} {:<6} {}{}", d.id, d.kind, d.label, if d.revoked { " (revoked)" } else { "" });
                }
            }
            Ok(())
        }
        ["account", "show", username] => {
            let d: AccountDetail = args.admin()?.get(&format!("/admin/account?username={username}"))?;
            if args.has("--json") {
                println!("{}", serde_json::to_string_pretty(&d).map_err(|e| e.to_string())?);
                return Ok(());
            }
            let a = &d.account;
            println!("{} ({}), id {}, {}", a.username, a.display_name, a.id, a.state);
            println!("  role {}, content {}, games {}, list {}{}{}", a.role, if a.content.is_empty() { "none" } else { &a.content }, a.games, a.list_slots, if a.report { ", reports results" } else { "" }, if a.muted { ", muted" } else { "" });
            if !a.note.is_empty() {
                println!("  note: {}", a.note);
            }
            println!("  made {}, last seen {}", when(a.created), a.last_seen.map_or("never".into(), when));
            println!("  devices:");
            for dev in &a.devices {
                let seen = dev.last_seen.map_or("never".into(), when);
                println!("    {} {:<6} {} (seen {seen}){}", dev.id, dev.kind, dev.label, if dev.revoked { " revoked" } else { "" });
            }
            println!("  profiles and stats: none yet (S4)");
            println!("  latest changes:");
            for e in &d.audit {
                println!("    {} {:<16} by {} {}", when(e.at), e.action, e.actor, e.detail);
            }
            Ok(())
        }
        ["account", "set", username] => {
            let change = AccountChange {
                username: username.to_string(),
                new_username: args.flag("--username"),
                display_name: args.flag("--display-name"),
                role: args.flag("--role"),
                content: args.flag("--content"),
                games: args.flag("--games"),
                list: args.number("--list")?.map(|n| n as u32),
                note: args.flag("--note"),
                muted: if args.has("--mute") { Some(true) } else if args.has("--unmute") { Some(false) } else { None },
            };
            let a: AccountView = args.admin()?.post("/admin/account/set", &change)?;
            println!("{}: role {}, content {}, games {}, list {}{}", a.username, a.role, a.content, a.games, a.list_slots, if a.muted { ", muted" } else { "" });
            eprintln!("its devices get the new rights with their next ticket; `chimera_accounts account refresh {}` makes it now", a.username);
            Ok(())
        }
        ["account", "refresh", username] => {
            args.admin()?.post::<()>("/admin/account/refresh", &serde_json::json!({ "username": username }))?;
            println!("{username}: its tickets are asked again (the services drop the old ones within 30 s)");
            Ok(())
        }
        ["account", "delete", username] => {
            args.admin()?.post::<()>("/admin/account/delete", &serde_json::json!({ "username": username }))?;
            println!("{username}: deleted with its devices; the username is free");
            Ok(())
        }
        ["page"] => {
            let admin = args.admin()?;
            if admin.health().is_none() {
                return Err(format!("the accounts service at {} isn't answering: start it first", args.url()));
            }
            let secret = std::fs::read_to_string(args.paths()?.secret()).map_err(|e| e.to_string())?;
            println!("{}/admin/#key={}", args.url().replace("://127.0.0.1", "://localhost"), secret.trim());
            eprintln!("open it in a browser on this machine (from another: ssh -L <port>:127.0.0.1:<port> to it); the key is the services' secret: don't share the address");
            Ok(())
        }
        ["install-units"] => {
            let paths = args.paths()?;
            let config = args.services()?;
            let front_listen = args
                .flag("--front")
                .or(config.front.listen.clone())
                .ok_or("the front's public address: --front <ip:port> (or [front] listen in services.toml)")?;
            let split = |v: Option<String>| v.map(|v| v.split_whitespace().map(String::from).collect()).unwrap_or_default();
            let bin = match args.flag("--bin") {
                Some(b) => PathBuf::from(b),
                None => dirs::home_dir().ok_or("no home folder: pass --bin")?.join(".local").join("lib").join("chimera"),
            };
            let settings = units::UnitSettings {
                bin: bin.clone(),
                store: std::path::absolute(&paths.store).map_err(|e| e.to_string())?,
                config,
                front_listen,
                assets_args: split(args.flag("--assets-args")),
                accounts_args: split(args.flag("--accounts-args")),
                writable: args.flags.iter().filter(|(n, _)| n == "--rw").filter_map(|(_, v)| v.clone().map(PathBuf::from)).collect(),
            };
            let to = match args.flag("--to") {
                Some(t) => PathBuf::from(t),
                None => units::user_unit_dir().ok_or("no config folder: pass --to")?,
            };
            for path in units::install(&settings, &to)? {
                println!("wrote {}", path.display());
            }
            println!("next:");
            println!("  install -D -m 0755 -t {} target/release/chimera_front target/release/chimera_accounts target/release/chimera_hub target/release/chimera_asset_server", bin.display());
            println!("  systemctl --user daemon-reload");
            println!("  systemctl --user enable chimera-front chimera-accounts chimera-assets chimera-hub chimera-services.target");
            println!("  systemctl --user start chimera-services.target");
            println!("  loginctl enable-linger \"$USER\"   # once: they run without a login session");
            println!("  chimera_accounts status          # one line per service");
            Ok(())
        }
        ["account", "revoke", username] => {
            args.admin()?.post::<()>("/admin/account/revoke", &serde_json::json!({ "username": username }))?;
            println!("{username}: revoked with all its devices");
            Ok(())
        }
        ["device", "revoke", device] => {
            args.admin()?.post::<()>("/admin/device/revoke", &serde_json::json!({ "device": device }))?;
            println!("device {device} revoked");
            Ok(())
        }
        ["ban", target] => {
            let reason = args.flag("--reason").ok_or("--reason <text> is needed")?;
            let days = args.number("--for")?;
            args.admin()?.post::<()>("/admin/ban", &serde_json::json!({ "target": target, "days": days, "reason": reason }))?;
            println!("{target}: banned {}", days.map_or("for good".into(), |d| format!("for {d} days")));
            Ok(())
        }
        ["import-legacy", rest @ ..] => {
            let path = match rest {
                [p] => PathBuf::from(p),
                [] => chimera_assets::accounts::Accounts::path(&args.paths()?.store),
                _ => return Err(USAGE.into()),
            };
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let done: Imported = args.admin()?.post("/admin/import-legacy", &serde_json::json!({ "toml": text }))?;
            println!(
                "imported {} accounts ({} legacy devices); {} skipped (imported before); {} devices and {} accounts revoked from the file",
                done.accounts, done.devices, done.skipped, done.revoked, done.revoked_accounts
            );
            Ok(())
        }
        ["backup"] => {
            let out: serde_json::Value = args.admin()?.post("/admin/backup", &serde_json::json!({}))?;
            println!("backup {} (integrity ok)", out["path"].as_str().unwrap_or("?"));
            Ok(())
        }
        ["restore", backup] => {
            if Admin::new(&args.url(), "").health().is_some() {
                return Err(format!("the accounts service at {} is running: stop it first", args.url()));
            }
            let paths = args.paths()?;
            db::restore(std::path::Path::new(backup), &paths.db())?;
            println!("{} restored from {backup}; start the service", paths.db().display());
            Ok(())
        }
        ["status"] => {
            let config = args.services().unwrap_or_default();
            let line = |name: &str, url: String| match Admin::new(&url, "").health() {
                Some(line) => println!("{name:<9} {url}: {}", line.trim()),
                None => println!("{name:<9} {url}: not answering"),
            };
            let routes = config.routes();
            line("accounts", args.url());
            line("assets", format!("http://{}", routes.assets));
            line("hub", format!("http://{}", routes.hub));
            if routes.feedback != routes.assets {
                line("feedback", format!("http://{}", routes.feedback));
            }
            match config.front.listen.as_deref().and_then(|l| l.rsplit_once(':')).map(|(_, port)| port.to_string()) {
                Some(port) => {
                    let url = format!("https://127.0.0.1:{port}");
                    let fingerprint = chimera_assets::tls::fingerprint_in(&args.paths()?.tls());
                    let answer = fingerprint
                        .ok_or_else(|| "no certificate in the store".to_string())
                        .and_then(|f| chimera_services::front::health(&url, &f));
                    match answer {
                        Ok(text) => println!("{:<9} {url}: {}", "front", text.trim()),
                        Err(e) => println!("{:<9} {url}: not answering ({e})", "front"),
                    }
                }
                None => println!("{:<9} (no [front] listen in services.toml)", "front"),
            }
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
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
