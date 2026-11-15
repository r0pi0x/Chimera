# Plan: the hub and its services (accounts, server list, assets, feedback)

Status: S1a in progress (2026-11-14): the user's decisions are in
section 11; the plan below follows them.
The user's direction: a master server players connect to up front, which
serves the game files by default, lists every lobby (hosting one lists it
for everyone on the same master) and holds everyone's **accounts**, which
the owner alone admits; their stats are kept in a **persistent database**
beside the assets; the parts are **separate services** that restart
without breaking each other. The player's whole setup: **the hub link and
an invite code**, nothing else. Anyone with an account uses the asset
service with no further step.

This plan is main-menu.md's M3 (the master server, the Internet tab)
widened to a discovery document, plus the accounts that the first version
of this plan kept as a North Star; both are now one design.

**A Chimera platform, not one game's** (user, 2026-11-14): the account
is the hub's identity (invite, username, devices, recovery, asset
access), owned by Chimera crates. Under it each account has a **profile
per game** (`ultimate_gun_game`, later `lucker_party`), made the first
time it plays that game, holding that game's stats, progression and
settings in a schema **the game declares as data** and registers, as
minigames and hooks are. Game servers report results per game. By
default an account plays every game on the hub; access per game is an
option.

Words: a **game** here is a title played on the hub (an app's id:
`ultimate_gun_game`); the asset service's games (`cs_source`, `hl2dm`,
`combat_arms`: whose files may be fetched) are called **content**.

Placeholders only: `<hub host>`, `<hub port>`, `<fingerprint>`,
`<server store>`, `<code>` and `<public address>` are never written into
the repository; the host hands the link and codes out privately, as
playtest-hosting.md says.

Section 11 lists the user's decisions (2026-11-14) and what is still open.

## 1. What exists (build on it)

| Need | Have | Where |
|---|---|---|
| Asking a server for its info and ping | the A2S_INFO-shaped query on the game port (challenge, name, map, folder `chimera`, players, max, bots, password, `NET_VERSION`, instance id, keywords with `minigame=<id>`); `Responder` rate limits | `chimera_net::net::query` |
| A client's queries | `ServerQueries`: one socket, `query(addr)` unicast, `scan_lan`, retries, ping by round trip, one entry per instance | same |
| The browser's model and page | `party_kit::server_list` (`BrowserTab` LAN, Favourites, History; `listed()` rows, `Status`, sort by ping) and `signal_pages` (Join's right column) | `apps/party_kit/src/` |
| An HTTPS service testers already reach | the asset service: tiny_http on 127.0.0.1 behind its own TLS front (`tls::Front`, self-signed Ed25519 certificate, clients pin its SHA-256 fingerprint), the client's address behind the front (`tls::Peers`), access log | `chimera/assets/src/server.rs`, `tls.rs` |
| Accounts today | `accounts.toml` (token hashes only, `chimera1_` tokens), `chimera_pack user add/revoke/list`, the `Authenticator` trait (one per account kind, `Verdict`, `Principal` with its games), per-account `Limits` (rate, burst, bytes a day), bad-token bans, `FrontLimits` | `chimera/assets/src/accounts.rs`, `server.rs`, `tls.rs` |
| A signing key | the server's Ed25519 key that signs manifests (`sign`, `GET /key`) | `chimera/assets/src/sign.rs` |
| A player's token, bound to one service | `asset-access.toml` (token, URL, fingerprint; the first offered service binds it) | `chimera/assets/src/access.rs` |
| A game server naming its asset service | `chimera:asset_url`, `chimera:asset_fingerprint`; the match manifest's `AssetOffer` | `chimera_net::net::assets` |
| Token entry today | `+chimera:asset_token <token>` on the command line, bound at the next join | `chimera_client::client`, `chimera_net::net::assets` |
| Feedback reports | built (P1): `POST /feedback` on the asset service with the tester's token, `feedback::inbox` (files under `<server store>/server/feedback/`), the triage page on the plain listener only, the client's outbox with retries | `chimera/assets/src/feedback/`, [feedback-reports.md](feedback-reports.md) |
| A match's results | `minigame::MatchResults` (name, kills, deaths, rung, wins per player; the winner first), recorded on the authoritative side at `MatchEnd`, replicated by `NetMinigame` | `chimera/sim/src/minigame/results.rs`, `chimera/net/src/minigame_net.rs` |

Missing: no master list; no way to learn a service except from a game
server at join or the local config; no first-run screen; no account that
outlives a token; no database; one process does every server-side job.

## 2. Architecture: separate services

### 2.1 The services

Each is its own process, started and restarted on its own:

| Service | Job | Its state |
|---|---|---|
| **front** | The one public TLS port: the TLS handshake with the certificate players pin, then each connection relayed to the service its path names (2.3); per-address connection limits (`FrontLimits`) | none (the certificate's folder, read at start) |
| **accounts** | The platform account: invites the owner mints, sign-up with a username, device keys, device link codes, recovery, tickets (2.4), revocation, bans, rights; per-game profiles, match results and stats (section 9); the admin API and page (localhost) | `accounts.db` (section 3): the one database that must never be lost |
| **hub** | The discovery document (`GET /hub`), the server registry and its heartbeats, the reachability probe, the list | `hub.db`: listings only (ephemeral, kept so a restart doesn't empty the list) |
| **assets** | Today's asset service, unchanged in its routes; accepts account tickets beside the legacy tokens | the store (records, manifests), access log |
| **feedback** | Today's `/feedback` route and triage page, moved to their own process in S5 (until then they stay in assets) | report folders on disk, as built |

What depends on what: nothing calls another service to answer a
player's request, except a player asking accounts for a ticket.
Assets, hub and feedback check tickets locally with the accounts
service's public key; they ask accounts (on loopback) only for the
revocation list, every 30 s, and keep the last one when it doesn't
answer.

### 2.2 A binary per service (decided)

The user's decision (2026-11-14): **one binary per service**, so each is
updated and deployed on its own: `chimera_front`, `chimera_accounts`,
`chimera_hub`, the asset service (`chimera_asset_server`, kept under its
name so developers' steps and assets-for-developers.md keep working) and
later `chimera_feedback`. They share code through library crates: the
`chimera_services` library (`chimera/services/`: the database helpers,
the accounts service, later the front and the hub; a `[[bin]]` per
service) and `chimera_assets` (tickets, TLS, the asset service). The
owner's command line is the accounts binary's own (`chimera_accounts
invite <name>`, 7.1). A quick host on Windows starts each binary in its
own console.

| Option | For | Against |
|---|---|---|
| **One binary per service** (decided) | Each binary is rebuilt, copied and restarted alone; a service's update never ships the others | A build per service; shared code linked into each (small: no Bevy) |
| One binary, a role per process (the earlier recommendation) | One file to install | Every update ships every role |
| One process, many routes (the first version of this plan) | Simplest | A crash or restart takes everything down; the user asked for the opposite |

### 2.3 Ports and the TLS front

One public TCP port, `<hub port>`: **today's asset `--tls` port and its
certificate** (`<server store>/server/tls/`), so every tester's existing
binding (`asset-access.toml` pins that fingerprint) keeps working and the
hub link names the same fingerprint. One router forward, one firewall
rule.

Internal listeners, plain HTTP on 127.0.0.1 only (`services.toml` in
`<server store>/server/`, host-written; these defaults; 8732 is not
used because today's host already runs a second asset listener there):

| Service | Loopback port | Public path prefixes (through the front) |
|---|---|---|
| assets | 8731 (today's) | everything not below: `/key`, `/whoami`, `/games`, `/m/`, `/r/`, `/batch` (old clients use root paths) |
| accounts | 8741 | `/accounts/` |
| hub | 8742 | `/hub` and `/hub/` |
| feedback | 8743 (S5; before it, assets) | `/feedback` |
| front | none (its public port only) | |

**Routing**: the front reads the first request's line on a connection,
picks the service by its path's prefix and relays that connection to it
for its whole life. Each request head on the connection gets
`X-Chimera-Peer: <client ip:port>` added (an incoming one is stripped),
which replaces the in-process `tls::Peers` for the access logs and
per-address limits; to add it the front parses request heads and
`Content-Length` bodies (our clients never send chunked request bodies;
the front answers 411 to one). Our clients keep one connection pool per
service (one ureq `Agent` each), and a service answers **421** to a path
that isn't its own, so a stray request is loud, not misrouted.

The front never relays `/admin`, `/internal` or `/feedback/admin`
(404): those exist on the loopback listeners only, for this machine.
Services trust `X-Chimera-Peer` only from a loopback connection.

Alternative (not taken, decision 2): no front; each service runs its own in-process
TLS listener on its own public port with the same certificate, and the
discovery document names each port. No single point of failure, but a
router forward per service and old clients keep needing the asset port.
The front is small, holds no state and changes rarely, so restarts of the
others (the frequent case) never touch it.

### 2.4 How services trust a player: tickets

- **Device key.** Redeeming a code (5.1) makes an Ed25519 key pair **on
  the player's machine**; the accounts service stores only the public
  key. No secret of a player's ever sits on the server: a copied
  `accounts.db` grants nothing.
- **Ticket.** The client asks `POST /accounts/ticket` with a request
  signed by its device key (device id, time, a random nonce; refused
  outside +-5 minutes or with a nonce seen in that window) and receives a
  **ticket**: `chimera2t.<payload>.<signature>` (base64url), the payload
  (postcard) carrying the version, the key id, the account id, the device
  id, the username, role, rights (`content`: what the asset service may
  serve it; `games`: `*` or the games it may play; `list` slots,
  `report`, `muted`), the
  audience, issued and expiry times. Signed with the accounts service's
  ticket key (`<server store>/server/accounts/ticket.key`, mode 0600, not
  the manifest key).
- **Checked locally.** Every other service (and a game server, S4)
  checks a ticket with the ticket public key alone: signature, expiry
  (60 s of skew), audience, the revocation list. The public key is a file
  beside the private one for services on this box and is in the
  discovery document (`accounts.ticket_keys`) for game servers and
  clients. Tickets carry a key id, so a new key can be added and the old
  one retired once its tickets expire.
- **Lifetime: 6 hours**, refreshed by the client at half-life and kept on
  disk, so a game started while accounts restarts still has a valid one
  (decision 4). **Revocation** is quick anyway: accounts serves
  `GET /internal/revoked?since=<n>` (account and device ids, loopback,
  with the services' shared secret) and assets, hub and feedback poll it
  every 30 s; a revoked device stops within 30 s while accounts is up and
  at its ticket's expiry at the latest while it is down.
- **Audiences**: `services` (assets, hub list and heartbeats, feedback)
  and `join:<instance id>` (S4: a short ticket, 5 minutes, that a player
  shows one game server; that server can't reuse it elsewhere).
- **Services among themselves** talk only on loopback, with a shared
  secret (`<server store>/server/services/secret`, 0600, made at first
  start) in `X-Chimera-Service`. Game servers are players of the hub:
  they hold an account (role `host`) and tickets like anyone.
- **Legacy tokens** (`chimera1_...`) keep working as bearer tokens
  (section 6).

The asset service gains a `TicketAuth` (an `Authenticator` of kind
`account`, recognising the `chimera2t.` prefix) before `Internal`; its
`Principal` name is the account id and its `Games` are the ticket's
`content`.
Today's per-account limits, bans and access log stay as they are.

### 2.5 What keeps working when one is down

| Down | Still works | Stops |
|---|---|---|
| front | Joined matches; LAN, Favourites, History, Direct joins; installs; everything on this box over loopback | Every public route; clients use the cached document and list and say "hub offline"; listings expire after 90 s without heartbeats and come back with the next ones |
| accounts | Downloads, the list, heartbeats and reports for anyone with a valid ticket (up to 6 h); the revocation list as last fetched | Redeeming, new tickets, the admin page; stats at match end wait in the game server's outbox and are sent when it is back |
| hub | Downloads, accounts, joins by address; game servers retry heartbeats quietly | The Internet tab's fresh list (the cached one shows, marked old) |
| assets | Players with installs; accounts, the list, joins | Downloads (clients retry as today) |
| feedback | Everything else | Uploads wait in the client's outbox (as built) |
| `accounts.db` corrupt | The others, as "accounts down" | accounts refuses to start and logs why; restore the last backup (3.3) |

### 2.6 Supervision on this box

- **systemd user units**, one per role: `chimera-front.service`,
  `chimera-accounts.service`, `chimera-hub.service`,
  `chimera-assets.service`, `chimera-feedback.service`, all
  `PartOf=chimera-services.target`; `loginctl enable-linger` once so they
  run without a login session. Templates in `tools/services/systemd/`
  using `%h` and `<server store>` placeholders; an install script
  (S1b) writes them to `~/.config/systemd/user/`.
- **Restart policy**: `Restart=on-failure`, `RestartSec=2`,
  `StartLimitIntervalSec=120`, `StartLimitBurst=5` (a crash loop stops
  and shows in `systemctl --user status`). Hardening that costs nothing:
  `NoNewPrivileges=yes`, `PrivateTmp=yes`, `ProtectSystem=strict`,
  `ReadWritePaths=<server store>`.
- **The binaries** are installed to `~/.local/lib/chimera/` (copies of
  release builds), so a `cargo build` never swaps one under a running
  service; an update is a copy of that service's binary and `systemctl
  --user restart chimera-<service>`.
- **Logs**: stderr to journald (`journalctl --user -u chimera-accounts
  -f`); the access logs stay files under `<server store>/server/`.
  `chimera_accounts status` (S1b) asks each loopback `GET /health`
  (version, uptime, DB state, revocation list age) and prints one line
  each.
- **Graceful stop**: SIGTERM stops accepting, finishes requests in flight
  (at most 10 s), checkpoints the WAL (accounts, hub) and exits 0.
- **Windows** (a host without systemd): each binary in its own
  console, as `chimera_asset_server` runs today.

### 2.7 Code layout (layering)

Each crate's folder is its layer (checks/tests/it/architecture.rs):

- **`chimera_assets`** (engine, plain data, no Bevy) gains what clients,
  game servers and services all need, small and SQLite-free:
  `ticket` (the format, signing, checking, the revocation set),
  `account` (the client side: device key, `account.toml`, redeem and
  ticket requests over `tls::PinnedTls`, the ticket cache; replaces
  `access`'s token for a hub-bound player), `hub_doc` (the discovery
  document and the link format, client and server side) and `TicketAuth`
  in `accounts`.
- **`chimera_services`** (new, `chimera/services/`, engine layer, no Bevy;
  a `workspace_crates` row: `default-members`, an empty `dev` feature
  turning on its dependencies' `dev`, listed in `NO_HACK` like
  `chimera/assets`; `cargo hakari generate`): a library with the
  services (`accounts`; later `front`, `hub`, `feedback` wrapping
  `chimera_assets::feedback`), `db` (rusqlite, the migrations, backup),
  the admin API, page and CLI, and a `[[bin]]` per service
  (`chimera_accounts`; later `chimera_front`, `chimera_hub`,
  `chimera_feedback`). Builds on
  `chimera_assets` and `chimera_formats` (the A2S codec for the probe).
  **rusqlite lives only here**, so no game or client build compiles
  SQLite. A host tool: not in player releases.
- **`chimera_assets::profile`** (plain data): the profile schema format
  (9.1): parsing, validation, applying a match's results to a profile,
  deriving stats. Shared by the accounts service (which stores and
  checks) and game servers (which declare and report), so both read one
  definition.
- **The game's schema** stays in its own crate as data: e.g.
  `apps/ultimate_gun_game/data/profile.toml`, registered by the app with
  `app.register_profile_schema(...)` (`chimera_net::net::hub`'s
  `ProfileSchemas` resource, beside minigame registration). Chimera never
  names a game; a game crate never touches the database.
- **`chimera_formats::a2s`**: the A2S packets' codec lifted from
  `chimera_net::net::query` (which re-exports it), so the hub can probe
  without the network crate.
- **`chimera_net::net::hub`** (engine): the game server's heartbeat
  thread, its match-results outbox (S4), the join-ticket check (S4); the
  client's `Hub` resource (cached document, refresh, the list).
- **`chimera_client::client`**: `+chimera:hub <link>` kept before the
  app starts; the redeem at first run.
- **`party_kit`** (app): `BrowserTab::Internet`, the first-run page,
  Settings > Network and Account (devices, link a device, recovery key),
  the account chip, the board's column; each party app passes its game
  id and schema.

## 3. The persistent database

### 3.1 SQLite (recommended) or Postgres

| | SQLite (rusqlite, `bundled`) | Postgres |
|---|---|---|
| Running it | A file; nothing to install, start or patch | A daemon to install, run, secure and upgrade |
| Builds | rusqlite's `bundled` compiles SQLite itself on Linux and Windows (MSVC) | sqlx or tokio-postgres pull an async runtime into a blocking codebase |
| Backup | One online backup call to a file | `pg_dump` and its own schedule |
| Scale | Tens of thousands of accounts and a write a second are nothing; one writer at a time (WAL lets readers run beside it) | Many writers on many machines |
| Moving later | Plain SQL, no SQLite-only tricks, so the schema ports | |

**SQLite now**, with rusqlite (blocking, like the rest of the server
code; sqlx's async and compile-time checks buy nothing here). Postgres
only if services ever run on several machines writing the same data.

### 3.2 Setup

- Files: `<server store>/server/accounts/accounts.db` (owned by the
  accounts process alone; the CLI goes through its admin API, 7.1) and
  `<server store>/server/hub/hub.db`. One database per service, never
  shared between processes.
- Pragmas at open: `journal_mode=WAL`, `synchronous=NORMAL`,
  `foreign_keys=ON`, `busy_timeout=5000`.
- **Migrations**: numbered SQL files compiled in
  (`chimera/services/migrations/0001_init.sql`, ...), applied at start in
  one transaction each, `PRAGMA user_version` as the counter (a few lines
  of our own; no migration crate needed). A database newer than the binary
  is refused with a message, never touched. A copy is taken (3.3) before
  any migration runs.

### 3.3 Backups

- **Nightly**, by the accounts service itself (a thread: once a UTC day,
  after 04:00 UTC, and at start when the day's copy is missing; no
  timer needed), and on demand with `chimera_accounts backup`: an online
  backup (SQLite's backup API) to
  `<server store>/server/backups/accounts-<YYYYMMDD>.db`, then `PRAGMA
  integrity_check` on the copy, logged.
- **Retention**: 14 daily copies, then one a week for 8 weeks; older ones
  deleted by the same command. Also one before every migration.
- **Off this box** (decision 7, undecided): local backups only for now.
  Until the backups folder is copied elsewhere (another machine on the
  tailnet, an encrypted cloud folder), a disk failure loses every
  account.
- **Restore**: stop accounts, `chimera_accounts restore <backup>` (checks
  the copy and refuses while the service answers), start; a test does
  exactly this.
- hub.db has no backup: its listings come back with the next heartbeats.

### 3.4 Schema (S1, plus S4's tables)

```
-- The platform account (Chimera's)
accounts   id BLOB(16) PK (random), username TEXT UNIQUE (case-folded; permanent),
           display_name (at most 20 characters), created, role ('tester'|'host'|'owner'), note (owner's),
           content ('*' or a list: asset service games), games ('*' or a list: hub games),
           list_slots INT, report INT (may send match results),
           muted INT (feedback), limits (JSON: rate, burst, daily bytes; null = role's),
           recovery_hash TEXT NULL (the recovery key's hash, 5.3),
           password_hash TEXT NULL (later, optional; argon2),
           state ('active'|'revoked'), revoked_at, last_seen
devices    id BLOB(16) PK, account_id FK, kind ('key'|'legacy'),
           public_key BLOB(32) UNIQUE NULL, token_hash TEXT UNIQUE NULL (legacy),
           label, created, last_seen, last_address (owner's eyes, trimmed after 30 days),
           revoked_at
invites    code_hash TEXT PK (SHA-256), name (the owner's label and the suggested
           username), role, content, games, list_slots,
           for_account FK NULL (a re-invite into that account), created, expires,
           redeemed_at, redeemed_device FK NULL, revoked_at
link_codes code_hash TEXT PK, account_id FK, made_by_device FK, created,
           expires (10 minutes), used_at, used_device FK NULL
bans       id PK, account_id FK NULL, address TEXT NULL, reason, created, until NULL
revocations seq INTEGER PK, kind ('account'|'device'), target BLOB(16), at
           (the feed services poll)
audit      seq PK, at, actor ('cli'|'page'|'system'|'<device>'), action, target, detail

-- Games on the platform (each game's data shaped by its own schema)
game_schemas game TEXT, version INT, schema TEXT (the game's profile.toml),
           hash, registered_by FK, at; PK (game, version)
game_profiles account_id FK, game TEXT, schema_version INT,
           data TEXT (JSON: progression and settings the schema declares),
           created (first play), updated; PK (account_id, game)
matches    id BLOB(16) PK (the server's), game, server_account FK, server_device FK,
           instance, minigame, map, started, ended, length_ms, received, ranked INT
match_players  match_id FK, slot, account_id FK NULL (a guest), name,
           fields TEXT (JSON: the schema's per-player result fields:
           kills, deaths, rung, place, won, ...)
stats      account_id FK, game, scope ('*' or '<minigame>'), key, value INT,
           match_id FK NULL (the match that last changed it), updated;
           PK (account_id, game, scope, key)

-- hub.db
servers    instance PK, account_id, address, port, lan (JSON), name, game,
           minigame, map, players, max, bots, password, net, reachable,
           first_seen, last_heartbeat
```

`stats` and each profile's derived parts are computed from
`match_players` by the game's schema in the same transaction that stores
a match, so they can always be rebuilt (`admin stats rebuild [--game]`).
The platform tables never name a game's fields; a new game adds rows,
not columns.

## 4. The discovery document and the server list

### 4.1 What the hub returns

`GET /hub` (a `services` ticket or a legacy token; decision 6) answers
JSON, built from the host's `<server store>/server/hub/hub.toml` and the
process itself:

```
{
  "schema": 1,
  "name": "<hub name>",
  "issued": 1763150000, "expires": 1763754800,
  "accounts": { "url": "/accounts", "ticket_keys": { "<key id>": "<public key hex>" } },
  "servers":  { "url": "/hub/servers" },
  "register": { "url": "/hub/heartbeat" },
  "assets":   { "url": "/", "key": "<manifest key hex>" },
  "feedback": { "url": "/feedback" },
  "news": [ { "id": "2026-11-14a", "date": "2026-11-14", "title": "...", "text": "..." } ],
  "motd": "Playtest Saturday 13:00 UTC",
  "client": { "min_net": 53, "recommended_build": "0.1.0", "download": "<release page>" },
  "next_fingerprint": null
}
```

- **Endpoints** are relative to the hub's origin (the front) by default
  and absolute with their own fingerprint when a service moves elsewhere
  (a VPS for the assets later): an edit to hub.toml, not a client update.
- **`client.min_net`**: below it the menu shows "A newer build is needed
  for this hub's servers" with the link; LAN and installs still work.
- **News and MOTD**: plain text; the menu shows the MOTD as one line and
  the newest item until dismissed.
- **Trust**: pinned TLS (the link carries the fingerprint) and the
  document signed with the manifest key (`X-Chimera-Signature`), pinned on
  the first fetch, so a cached copy is trusted offline and a game server
  may relay it later. **Rotation**: `next_fingerprint` signed by the
  current key; a hub that lost both key and certificate is a new hub.
- **Client cache**: `<data_local>/chimera/hub.toml` (URL, fingerprint,
  pinned key, skip mark, dismissed news; 0600 on Unix; `CHIMERA_HUB`
  names another file) and `hub-discovery.json` with its `.sig`. Used at
  once at start, fetched again on a thread (at most `REFRESH_WAIT`) and
  when Join opens if older than 5 minutes.

### 4.2 Heartbeats

A listed game server sends `POST /hub/heartbeat` every 30 s, and at once
when its info changes, from a thread, with its `services` ticket:

```
{ "instance": "<instance id hex>", "port": 27015, "lan": ["192.168.x.y:27015"],
  "name": "...", "game": "ultimate_gun_game", "minigame": "gun_game",
  "map": "de_dust2", "players": 5, "max": 12, "bots": 3,
  "password": false, "dedicated": true, "net": "0.1.0/net53" }
```

- **Who may list**: a ticket whose `list` right is above 0 (how many
  servers at once; role `host` gets 4, testers 1 or 0: decision 5). Past
  it, or past one heartbeat per 10 s per listing: 429 with the reason.
- **The address is the one the hub sees** (`X-Chimera-Peer`) with the
  body's port; a body never names another machine, so nobody can list a
  stranger. A heartbeat from this box itself is listed at hub.toml's
  `public_address`. `lan` (private ranges only) is given beside it, for
  players behind the same router; `ServerQueries`' instance id merges the
  two into one row.
- **Expiry**: 3 missed heartbeats (90 s) drops a listing; `POST
  /hub/leave` (quit, `sv_hub 0`) at once. One listing per (account,
  instance). A revoked account's listings go with the revocation feed.
- Text cleaned: names to 64 characters, no control characters, ids to
  `[a-z0-9_:-]`; anything else 400.
- **The probe**: after the first heartbeat and every 5 minutes, one
  A2S_INFO from the hub's UDP socket to the seen address and port;
  `reachable: true | false | unknown` in the answer, printed once by the
  server ("hub: listed, but the hub can't reach UDP <port>: forward it or
  try net_upnp 1") and in `status`. Only ever to the heartbeat's own
  address, at most once a minute per listing. Unreachable listings are
  hidden from others and shown to their own account with the reason.
- NAT as in [nat-traversal.md](nat-traversal.md): a forwarded port or
  `net_upnp`; no relay here. IPv4 first.

### 4.3 The list and the Internet tab

- `GET /hub/servers` (a ticket): the live listings as JSON (`addr`,
  `lan[]`, name, game, minigame, map, players, max, bots, password, net,
  reachable, since), at most 500, from memory.
- **Internet** becomes the browser's first tab (`BrowserTab::Internet`,
  before LAN): rows are `server_list::listed()` over the list's
  addresses, each **queried directly** with `ServerQueries::query` (the
  hub's numbers fill a row until the server answers), at most 64 queries
  at once, sorted by ping, then asking, then silent. Another
  `NET_VERSION` is greyed ("needs build ..."); another game's servers are
  left out unless the filter asks, and a game the account may not play
  is greyed (5.5). F5 refreshes. **Join** as from any
  other tab.

## 5. Accounts

### 5.1 Invite and sign-up

- **The owner** runs `chimera_accounts invite <name>` (or mints on
  the admin page), which prints one line to send privately:
  `chimera-hub://<hub host>:<hub port>/<fingerprint>?code=XXX-XXX-XXX`.
  The code is 9 characters of Crockford base32 (45 bits; no 0/O or 1/I
  confusion, case and dashes ignored), **single use**, valid 14 days
  (`--expires`); only its SHA-256 is stored and it is printed once.
  Redeeming is rate limited (10 tries an hour per address; wrong codes
  count toward the bad-token ban), which makes 45 bits plenty. `<name>`
  is the owner's label and the suggested username. The link without
  `?code=` is the plain hub link. The same text works as `+chimera:hub
  <link>` on the command line.
- **The player** pastes the link in the first-run page (8), picks a
  **username** (3-20 characters, `[a-z0-9_]` after case folding, unique
  on the hub, a short list of reserved words refused; the name from the
  invite filled in) and presses Connect. The client makes an Ed25519 key
  pair; `POST /accounts/redeem {code, username, device_public_key,
  device_label}` over the pinned link creates the account and answers
  `{account, device, ticket, recovery_key}` (5.3). A taken username is
  answered without using up the code.
- **After that, silent login**: the client asks for tickets with its
  device key; no password, nothing to type.
- **Username and display name**: the username is the permanent handle
  (the owner's lists, the board's tooltip, `admin` commands, re-invites);
  the **display name** is what games show (HUD, board, chat): at most
  **20 characters**, enforced by the accounts service whenever it is set
  or changed; it starts as the username and changes freely in
  Settings. The username is shown only in account and admin places. (A
  separate UI task truncates per widget.) A username changes only by the owner (`admin
  account set <id> --username`).
- **Passwords**: none for now; an optional password (argon2, a column
  already reserved) may come later as one more way to add a device.
- **The client's file**: `<data_local>/chimera/account.toml` (hub URL
  and fingerprint, account and device ids, username, the device's private
  key, the last ticket), mode 0600 on Unix; on Windows the user's
  `%LOCALAPPDATA%` as `asset-access.toml` is today (DPAPI is a known
  shortcut to take later).

### 5.2 Devices

- **Link a device**: Settings > Account > **Link a device** asks
  `POST /accounts/link` (with a ticket) and shows a short code
  (`XXX-XXX`, 10 minutes, single use, hashed). On the new PC the player
  enters the hub link and that code on the first-run page (the code field
  takes either kind); `POST /accounts/redeem` with a link code adds a key
  device to the same account. The first device's page shows "linked:
  <device label>" when it is used.
- **Listed and revocable**: Settings > Account lists the devices (label,
  last seen, "this device") with **Revoke** on the others; revoking goes
  into the revocation feed (2.4). **Sign out of this device** revokes
  its own and deletes `account.toml`. The owner sees and revokes the same
  on the admin page.

### 5.3 Recovery

- **Recovery key**, shown once at sign-up: 16 Crockford base32
  characters in four groups (80 bits), with **Copy** and **Save to a
  file** buttons and a check box "I saved it" before continuing (it can
  be shown again from Settings while signed in only by making a new one,
  which replaces the old). The hub stores its hash only. On a new PC
  without another device, the first-run page's "Recover with a key"
  takes the hub link, the username and the key; it adds a device and
  revokes the others (the old PC is presumed lost).
- **Or the owner re-invites into the same account**: `admin invite
  <name> --for <username>` (a button on the admin page) prints an invite
  that, redeemed, adds a device to that account (keeping the username,
  profiles and stats) and revokes its other devices unless `--keep`.
- A recovery key is a bearer secret: rate limited per username and per
  address as codes are, and a successful recovery is an `audit` row the
  owner sees.

### 5.4 Rights and roles

`tester` (the default: the content the owner chose, every game, `list`
1 or 0, may send feedback), `host` (`list` 4, `report` 1: its match
results count; no content unless given) and `owner` (everything, and the
admin API from another machine later). Rights are on the account and
travel in its tickets; a change takes effect at the next ticket (within
3 h) or at once with `admin account refresh <id>` (its devices' current
tickets go into the revocation feed, so the clients ask again).

### 5.5 Per-game access (optional)

By default `games = '*'`: an account plays every game on the hub, and a
game server admits any account. The owner may narrow it (`admin account
set <id> --games ultimate_gun_game`, or per invite); a game server then
refuses a join ticket for a game the account lacks ("this account isn't
invited to <game>"), and the Internet tab greys those servers. Accounts
mints a join ticket only for a game the account may play.

### 5.6 Privacy

No email, real name or phone number. Stored: the random ids, username,
display name, role, rights, device public keys and labels, last-seen
times, the recovery key's hash, the per-game profiles and stats, and for
30 days the last address a device used (abuse checks; the owner's eyes
only). A player can ask the owner to delete the account: `admin account
delete <id>` removes it, its devices, profiles and stats (match rows
keep the name as played, without the id) and frees the username after
30 days.

## 6. Migration from today's tokens

- **Import once**: `chimera_accounts import-legacy
  <server store>/server/accounts.toml` makes one account per internal
  account (role `tester`, its games as `content`, every game to play,
  limits and note; its old name, case-folded, as the username) and one
  `legacy` device per token hash. Nothing changes for testers.
- **Legacy tokens keep working everywhere**: the asset service keeps
  `Internal` (reading accounts.toml) until S3 has shipped one playtest
  build; accounts also accepts `Authorization: Bearer chimera1_...` at
  `POST /accounts/ticket` (answering a ticket for that legacy device), so
  a new client with only a token still gets tickets.
- **Automatic upgrade** (recommended, decision 8): a hub-aware client
  with a legacy token makes a device key and calls `POST
  /accounts/upgrade` with the token; the legacy device is replaced by a
  key device of the same account and the token stops working. The token
  line in `Play-UGG.bat` can then go.
- **Retiring accounts.toml**: after a playtest with S3, `Internal` is
  switched off (`--legacy-tokens off`); `chimera_pack user add` becomes a
  shortcut for `admin invite <name>` and prints the invite link instead
  of a token.
- `+chimera:asset_token` and a game server's own `AssetOffer` keep
  working for a server with its own asset service and no hub.

## 7. Owner-only account creation

### 7.1 The command line

Talks to the accounts service's admin API on loopback (with the services
secret), so the database has one writer:

```
chimera_accounts invite <name> [--role tester|host] [--content a,b|*] [--games a,b|*]
                                     [--list <n>] [--expires <days>]
                                     [--for <username> [--keep]]   # prints the invite link
chimera_accounts invites [--state unused|redeemed|expired|revoked]
chimera_accounts invite revoke <code or name>
chimera_accounts account list [--json]
chimera_accounts account show <username>    # devices, rights, last seen, profiles, stats
chimera_accounts account set <username> [--username ..] [--role ..] [--content ..]
                                   [--games ..] [--list n] [--note ..] [--mute|--unmute]
chimera_accounts account revoke <username> | device revoke <device>
chimera_accounts account delete <username>
chimera_accounts ban <username|address> [--for <days>] --reason <text>
chimera_accounts import-legacy <accounts.toml>
chimera_accounts stats rebuild [--game <game>]
chimera_accounts backup | status | install-units --store <dir>
```

Every change is an `audit` row.

### 7.2 The admin page

`http://127.0.0.1:8741/admin/` (loopback only, never through the front;
the same rule as the feedback triage page; from another machine over an
SSH tunnel): **Invites** (invite with a name, role, content, games and
expiry, the link shown once with a copy button; the list with state;
revoke), **Accounts** (username, display name, role, devices, last seen,
each game's profile summary; set rights, revoke a device or the account,
ban, **Re-invite** into the account), **Servers** (the hub's listings,
delist), **Matches** (the latest results per game) and **Games** (the
registered schemas and their versions). Plain HTML and a little
JavaScript served by the service, as the triage page is.

## 8. Client UX

- **First run** (a Signal page before the main page, when there is no
  hub.toml, no account, no bound asset token and no skip mark, and the
  command line doesn't play): **Hub link** (paste; an invite link fills
  the code too), **Code** (an invite code or a device link code),
  **Connect**; then, for an invite, **Pick a username** and the
  **recovery key** screen (5.3); a link code needs neither. Secondary:
  **Recover with a key** and **Skip: play from my installs / LAN**.
  Connect fetches the document, shows the hub's name and MOTD, redeems,
  keeps hub.toml and account.toml, and starts the startup download for
  the content the account may fetch. Errors in words: wrong link, hub
  unreachable, code unknown, used or expired, username taken, another
  certificate. A first run that adds content not installed restarts once
  (the existing `AssetRelaunch` path).
- **Main menu**: an **account chip** (display name, and this game's
  matches and wins or level once S4 is in; "Guest" without an account; a
  click opens Settings > Account), the MOTD line, the newest news.
- **Join**: Internet first (4.3), then LAN, Favourites, History; Join.
- **Settings > Network**: the hub (name, status), Change hub, Forget hub.
  **Settings > Account**: username, display name (editable), Link a
  device, the device list with Revoke, New recovery key, Sign out of this
  device.
- Players with every game installed still connect (for the list, reports
  and stats); Skip keeps everything working as today.

## 9. Per-game profiles and stats (S4)

### 9.1 A game declares its profile

Each game ships a schema as data in its own crate
(`apps/ultimate_gun_game/data/profile.toml`; later Lucker Party's) and
registers it (`register_profile_schema`, 2.7); Chimera's code never
names a game's fields. A sketch:

```
game = "ultimate_gun_game"
version = 1

[result]              # one row per player per match, from MatchResults
kills = "int"
deaths = "int"
rung = "int?"
place = "int"
won = "bool"

[stats]               # how results add up (scope "*" unless said)
matches = { count = true }
wins = { sum = "won" }
kills = { sum = "kills" }
deaths = { sum = "deaths" }
best_place = { min = "place" }
best_rung = { max = "rung", scope = "minigame" }

[progression]         # derived on the server, never sent by a client
xp = { per_match = 50, per_win = 100, per_kill = 2 }
level = { from = "xp", steps = [0, 500, 1200, 2100] }

[settings]            # the player's own, synced across devices
crosshair = { type = "string", max = 64 }
```

- **Result fields** come from `minigame::MatchResults`' rows by name
  (kills, deaths, rung, place, won); a game whose results carry more
  registers a hook that adds fields, as rules hooks are registered.
- **Stats and progression** are computed by `chimera_assets::profile`
  from stored results only: written by servers, never by clients.
- **Settings** are the only part a client writes (`PUT
  /accounts/profiles/<game>/settings` with a ticket), validated against
  the schema, at most 8 KB.
- **Versions**: a schema may add fields in a higher version; renaming or
  removing one needs a `[migrate]` entry (later); `admin stats rebuild`
  recomputes stats and progression from `match_players` after a change.
- **Registration with the hub**: a game server's report names its
  schema's game, version and hash; accounts answers 409 for one it
  doesn't know, and the server (a `host` account) uploads it to `POST
  /accounts/schemas`. A different schema under a known (game, version) is
  refused.

### 9.2 The profile

The first ticket for a game (a join ticket, or `GET
/accounts/profiles/<game>` from that game's menu) makes the account's
profile for it (`game_profiles`: empty settings, zero stats). An account
has as many profiles as games it has played; the platform account stays
the same across them.

### 9.3 Results at match end

- **Who is who**: at connect the client asks accounts for a `join`
  ticket for the server's game and instance id (from its A2S reply) and
  sends it in the connect message; the game server checks it locally
  with the document's ticket keys (and refuses a game the account may not
  play, 5.5). No ticket (no hub, accounts down, LAN): the player is a
  guest and nothing of theirs is recorded. This changes the connect
  message: a `NET_VERSION` bump in S4.
- **Report**: at `MatchEnd` the server takes `MatchResults`, maps them to
  the schema's result fields and posts `POST /accounts/matches` with its
  own `services` ticket and the body signed by its device key (match id,
  game and schema version, instance, minigame, map, times, one row per
  player: account id or none, name, the fields). It goes through an
  outbox on disk (`<data_local>/chimera/match-outbox/`) with retries, so
  an accounts restart loses nothing; the match id makes a resend
  harmless.
- **What counts**: results from an account with `report` (hosts) are
  ranked and update stats and progression; others are stored unranked
  or refused (decision 9). A row's account must have been issued a join
  ticket for that instance in the last hour, so a host can't credit
  absent players.
- **Shown**: the account chip shows this game's numbers (the schema
  names which: `[display] chip = ["level", "wins"]`); the board (Tab)
  gets each account's chosen stat beside the name (`[display] board =
  "wins"`): the server fetches `GET /accounts/public/<id>/<game>` for each
  joined account once and replicates it; guests show none. `GET
  /accounts/me/<game>` gives the client its own profile.

## 10. Phases

Each phase leaves everything working; the layering rules hold at each
step (2.7).

| Phase | What | Size | Tests |
|---|---|---|---|
| **S0** (now, no code) | Tokens and the asset `--tls` port as today; the host sends address and token privately | none | none |
| **S1a** accounts and the database | `chimera/services` crate (workspace row, `dev`, `NO_HACK`, hakari); `db` (rusqlite bundled, WAL, migrations, backup); the `accounts` role: invites (`admin invite <name>`), sign-up with a username, device keys, device link codes, the recovery key and owner re-invites, tickets, revocation feed, legacy import and legacy tickets, rights and per-game access; the admin API, CLI and page; `chimera_assets::ticket` and `TicketAuth` in the asset service | 7-9 h | `chimera_services` it tests on port 0: an invite redeems once with a username (a second redeem 4xx, an expired one 4xx, a revoked one 4xx; a taken or malformed username refused without using up the code); a link code adds a device to the same account once and expires after 10 minutes; a recovery key adds a device and revokes the others, a wrong one counts toward the ban; a re-invite `--for` keeps the username; a ticket checks with the public key alone, a changed byte fails, an expired one fails, a wrong audience fails; a ticket request with an old time or a seen nonce is refused; revoke a device: its tickets fail at the asset service within one revocation poll (a test clock); import-legacy: an old token still downloads, and gets a ticket; the asset service serves a ticket's content only; migrations from 0 on an empty DB and from a fixture DB of the previous version; backup then restore gives the same rows; a DB newer than the binary refused. `chimera_assets` tests for the ticket format |
| **S1b** front, units, the client's credential | The `front` role (connection routing, `X-Chimera-Peer`, 421, admin paths refused) on today's certificate; `all`; systemd templates and `install-units`; `admin status`; the client's `account` (redeem by `+chimera:hub <invite>` with no UI yet, the ticket cache and refresh, the asset service used with tickets); docs | 5-6 h | through the front: an old client's root paths reach assets with a legacy token and the same fingerprint; `/accounts/` reaches accounts; `/admin` 404 from outside; a forged `X-Chimera-Peer` stripped; per-address limits see the real peer; accounts stopped mid-test: downloads with a ticket go on, a new redeem fails cleanly; assets restarted: accounts unaffected. Client: `+chimera:hub <invite>` redeems once and downloads; a second start reuses the ticket with accounts down |
| **S2** the hub and the Internet tab | `chimera_formats::a2s` lifted; the `hub` role (document, signing, registry, heartbeats, expiry, the probe, `/hub/servers`, delist); `chimera_net::net::hub` heartbeats from dedicated and listen servers (`sv_hub`, `[hub]`); the client's `Hub` resource and `BrowserTab::Internet`; MOTD and news line; the min-client banner | 10-12 h (three agents' worth: server, game server, client) | the document's signature checks with `GET /key`; heartbeat without a ticket 401, without `list` 403, past it 429; the listed address is the peer's; this box's heartbeats listed at `public_address`; expiry after 3 missed beats, leave at once; revoke drops listings; the probe reaches a real `Responder` and not a closed port, and never another address; a hub restart keeps the list (hub.db); a headless dedicated server listed, updated when a bot joins, gone on quit; `party_kit` rows from a hub list on a NetSim pair sorted by ping, another `NET_VERSION` greyed, another app left out, offline shows the cache; `ui_scene join internet` screenshots |
| **S3** first-run setup | The first-run page (link and code, username, recovery key, Recover, Skip), Settings > Network and Account (display name, Link a device, devices and Revoke, new recovery key, sign out), the account chip (name only), legacy auto-upgrade, `chimera_pack user add` printing invites; docs (playtest.md: paste the invite) | 4-5 h | `main_menu.rs`: a first start shows the page, a second doesn't; Connect with an invite asks a username, shows the recovery key, keeps both files and starts the download; a link code from a second NetSim client's Settings signs a fresh client into the same account; revoking it from the first stops it; Skip leaves installs, LAN and Direct as today; a wrong link, an unreachable hub and a used code each say so; a legacy-token client upgrades once and the token stops working; screenshots at 720p and 1440p |
| **S4** per-game profiles and stats | `chimera_assets::profile` (schema format, results to stats and progression); `register_profile_schema` and UGG's `data/profile.toml`; schema upload; profiles made on first play; settings sync; join tickets in the connect message (`NET_VERSION` bump); the server's results outbox; `/accounts/matches`, `/accounts/me/<game>`, `/accounts/public/<id>/<game>`; `admin stats rebuild`; the chip's numbers; the board's column | 8-10 h | `chimera_assets`: a schema parses, a bad one is refused with the field named; results fold into stats and progression as declared; a rebuild equals the incremental sums. NetSim: a client with a join ticket is that account on the server, one without is a guest, a ticket for another instance or a game the account lacks refused; the first match makes the profile; an unknown schema is uploaded once (409 then 201), a changed one under the same version refused; settings over 8 KB or off-schema refused; two games' profiles on one account stay apart; a match end posts one report, resent after accounts comes back, stored once; ranked only from a `report` account; an absent account's row refused; the chip and board show the schema's numbers (screenshots) |
| **S5** feedback under accounts | The `feedback` role (its own process and port, routed at `/feedback`); reports carry the account id from the ticket; mute on the account (`muted`); the triage page on its loopback port; the `/feedback` route leaves assets | 2-3 h | feedback's existing it tests run against the role; a ticket's account named in `meta.toml`; a muted account gets 202 and nothing stored; assets restarted while a report uploads: the report lands |
| Release | A playtest build with S1-S3 (all players update) and the host's units installed; S4 in the next (its protocol bump) | user's go | `tools/post-push-tests.sh`; a fresh Windows PC: paste an invite, see the list, join; an old build's token still downloads |

Docs to change with the phases: playtest-hosting.md (the services, units,
invites, backups, ports), playtest.md (first run), assets-for-developers.md
(the hub link and invites beside `[assets]`), ARCHITECTURE.md (the new
crate), DESIGN.md "The party apps' main menu" and the asset service's
account kinds, main-menu.md (M3 as built), each new file's `//!` doc
(`docs_index`).

## 11. Decisions

Taken (user, 2026-11-14), first: a Chimera platform account with
per-game profiles declared by each game as data; every game open to
every account by default; the account UX of section 5 (`invite <name>`,
username, device key, link codes, recovery key or re-invite, no
password for now); legacy `chimera1_` tokens keep working.

Taken (user, 2026-11-14), second round:

1. **A binary per service**, not one binary with roles (each updated and
   deployed on its own): `chimera_front`, `chimera_accounts`,
   `chimera_hub`, the asset service (`chimera_asset_server`, kept),
   `chimera_feedback` later; shared code in library crates (2.2).
2. **One public port behind the front**: yes (2.3).
3. **SQLite**: yes (3.1).
4. (Ticket lifetime: 6 hours refreshed at half-life with a 30 s
   revocation feed, as planned.)
5. (Who may list: open, S2.)
6. **Who may read the document and the list**: account holders only
   (follows from 10).
7. **Backups off this box**: undecided; local backups only for now
   (3.3).
8. (Legacy auto-upgrade: open, S3.)
9. (Testers' own servers' results: open, S4.)
10. **Reach: always public, but locked down.** Nothing works without an
    account the owner explicitly created: no anonymous access to
    anything, the discovery document and the server list included. The
    only routes without a ticket or token are redeeming an invite or
    link code, recovering with a key and asking a ticket with a device's
    signed request, each rate limited per address; `/key` stays public (a
    public key).
11. **Usernames**: 3-20 characters of `[a-z0-9_]`, case-insensitive
    (stored case-folded); permanent handles shown only in account and
    admin places. **Display names** (what the HUD shows) are at most 20
    characters, enforced by the accounts service when set or changed;
    the default is the username (5.1).
12. **The recovery key** is saved at sign-up (the check box). A lost key
    is handled case by case: the owner re-invites into the same account
    (`chimera_accounts invite <name> --for <username>`, 5.3).

Still open: 5, 8, 9 and 13 (`chimera-hub://` as an OS link handler
later, or paste only).
