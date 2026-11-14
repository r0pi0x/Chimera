# Plan: the hub (one address for every service)

Status: planned (2026-11-14), nothing built. The user's idea: a master
server that lists servers, hosted on the dev box, reached through **one
address a player sets once** at first run; from it the game learns every
other service (the server list, the asset service, feedback, news), so
nobody types a server's IP again. This plan is main-menu.md's M3 (the
master server, the Internet tab) widened to a discovery document, and it
folds the asset token's entry into the same first-run step.

Placeholders only: `<hub host>`, `<hub port>`, `<fingerprint>`,
`<token>` and `<public address>` are never written into the repository;
the host hands them out privately, as playtest-hosting.md says.

Section 9 lists the decisions the user still has to make; section 10 is
the North Star for accounts (later).

## 1. What exists (build on it)

| Need | Have | Where |
|---|---|---|
| Asking a server for its info and ping | the A2S_INFO-shaped query on the game port (challenge, name, map, folder `chimera`, players, max, bots, password, `NET_VERSION`, instance id, keywords with `minigame=<id>`); `Responder` rate limits | `chimera_net::net::query` |
| A client's queries | `ServerQueries`: one socket, `query(addr)` unicast, `scan_lan`, retries, ping by round trip, one entry per instance (a server reached at two addresses is listed once) | same |
| The browser's model and page | `party_kit::server_list` (`BrowserTab` LAN, Favourites, History; `listed()` rows from addresses, `Status` up/asking/down, sort by ping, scrolling) and `signal_pages` (Join's right column) | `apps/party_kit/src/` |
| An HTTPS service testers already reach | the asset service: tiny_http behind its own TLS front (`tls::Front`, self-signed Ed25519 certificate, clients pin its SHA-256 fingerprint), the client's IP behind the front (`tls::Peers`), access log | `chimera/assets/src/server.rs`, `tls.rs` |
| Accounts and tokens | `accounts.toml` (hashes only), `chimera_pack user add/revoke/list`, `Authenticator` per account kind, per-account `Limits` (rate, burst, bytes a day), bad-token bans, `FrontLimits` (connections per address) | `chimera/assets/src/accounts.rs`, `server.rs`, `tls.rs` |
| A signing key | the server's Ed25519 key that signs manifests (`sign::sign`, `verify`, `GET /key`) | `chimera/assets/src/sign.rs` |
| A player's token, bound to one service | `asset-access.toml` (token, URL, fingerprint; the first offered service binds it; another certificate is refused) | `chimera/assets/src/access.rs` |
| A game server naming its asset service | `chimera:asset_url`, `chimera:asset_fingerprint`; the match manifest's `AssetOffer` | `chimera_net::net::assets` |
| Token entry today | `+chimera:asset_token <token>` on the command line (kept before the app starts), then a join binds it and the game restarts once (`AssetRelaunch`) | `chimera_client::client` (`take_asset_token`, `relaunch.rs`) |
| The startup download | `asset_startup` (the `base` sets behind a progress window, from a bound access) | `chimera_net::net::asset_startup` |
| Router port | `net_upnp`; NAT options ranked | `chimera_net::net::upnp`; [nat-traversal.md](nat-traversal.md) |
| A feedback endpoint | planned: `POST /feedback` on the asset service | [feedback-reports.md](feedback-reports.md) |

Missing: no master list, no way to learn a service except from a game
server at join or from `chimera.local.toml`, no first-run screen in the
party apps, no account right that separates "may download" from "may
list a server".

## 2. The discovery document

### 2.1 What the hub returns

`GET /hub` answers a JSON document (serde_json is in the workspace),
built from the host's `<server store>/server/hub.toml` (host-written, never
in the repo) and from the process itself (its certificate, its key):

```
{
  "schema": 1,
  "name": "<hub name>",                  // shown on the first-run page
  "issued": 1763150000, "expires": 1763754800,   // unix seconds; 7 days
  "servers":  { "url": "/hub/servers" }, // relative: the hub's own origin
  "register": { "url": "/hub/heartbeat" },
  "assets":   { "url": "https://<hub host>:<hub port>",
                "fingerprint": "<fingerprint>", "key": "<manifest key hex>" },
  "feedback": { "url": "/feedback" },    // absent until feedback-reports P1a
  "news": [ { "id": "2026-11-14a", "date": "2026-11-14",
              "title": "...", "text": "..." } ],
  "motd": "Playtest Saturday 13:00 UTC",
  "client": { "min_net": 52, "recommended_build": "0.1.0",
              "download": "<release page>" },
  "next_fingerprint": null               // certificate rotation (2.2)
}
```

- **Endpoints** are relative to the hub when they live in the same
  process (the default) and absolute with their own fingerprint when they
  don't (an asset service on a VPS later), so moving one service is an
  edit to hub.toml, not a client update.
- **`client.min_net`** is the number in `NET_VERSION` (`.../net52`): a
  client below it shows a banner "A newer build is needed for this hub's
  servers" with the `download` link; it still runs (LAN, own install).
  `recommended_build` only gives a softer "an update is out".
- **News and MOTD** are plain text (no HTML, no links followed by the
  game); the main menu shows the MOTD as one line and the newest news item
  until it is dismissed (its id remembered).

### 2.2 Trust: pinned, then signed

- **Pinned TLS**: the address a player sets carries the certificate's
  fingerprint (the hub link, 4.2), so the first contact already trusts
  exactly that certificate (`tls::PinnedTls`), as an asset offer does. No
  CA, no names, no trust on first use.
- **Signed**: the document's bytes are signed with the server's manifest
  key (`X-Chimera-Signature`, as `GET /m/...`), and the key comes in the
  document (`assets.key`). The client pins the key on the first fetch
  (over the pinned connection) and refuses a later document signed by
  another. The signature lets the cached copy be trusted offline and
  lets a game server relay the document later (phase H6) without the
  client trusting that server.
- **Rotation**: a hub that must change its certificate (a lost `tls`
  folder is the only case today) publishes `next_fingerprint` signed by
  the current key for a while; clients that saw it accept the new
  certificate. A hub that lost both key and certificate is a new hub:
  players paste its link again (the asset token's rule today).

### 2.3 How the client caches it

- `<data_local>/chimera/hub.toml` (beside `asset-access.toml`; mode 0600
  on Unix; `CHIMERA_HUB` names another file): the hub's URL, its
  fingerprint, its pinned key, the skip mark (5.3) and the dismissed news
  id. `hub-discovery.json` with `.sig` beside it: the last document as
  received.
- At start the cached document is used at once (the menu has endpoints
  before any network answer); a thread fetches it again (at most
  `REFRESH_WAIT`, as `asset_startup::refresh`) and when Join opens if it
  is older than 5 minutes. A failure keeps the cache and says "hub
  offline" on the Internet tab; past `expires` the endpoints are still
  used and the page says the list may be old.
- A release ships no hub.toml; tests and tool runs use none (as
  `BrowserFiles`).

## 3. Server registration

### 3.1 The heartbeat

A game server that is listed sends `POST /hub/heartbeat` over the pinned
HTTPS every 30 s, and at once when its info changes (map, players, a
password set), from a thread (never the main thread), with its host
token (`Authorization: Bearer`). The body is what its A2S reply says
plus what the hub can't learn by querying:

```
{ "instance": "<instance id hex>", "port": 27015, "lan": ["192.168.x.y:27015"],
  "name": "...", "app": "ultimate_gun_game", "minigame": "gun_game",
  "map": "de_dust2", "players": 5, "max": 12, "bots": 3,
  "password": false, "dedicated": true, "net": "0.1.0/net52" }
```

- **The address is the one the hub sees** (`tls::Peers`: the client
  address behind the front) with the body's `port`; a body never names
  another machine's address, so nobody can list a stranger. Exception:
  a heartbeat from the hub's own machine (loopback or its LAN address)
  is listed at the hub's `public_address` from hub.toml, so a dedicated
  server on this box is listed at the address outsiders reach.
- `lan` (the server's own interface addresses, private ranges only) is
  given to clients beside the public address: a player behind the same
  router as the server (no hairpin NAT) still reaches it, and
  `ServerQueries`' instance id merges the two into one row.
- **Expiry**: a listing missing 3 heartbeats (90 s) is dropped;
  `POST /hub/leave` (on `quit`, a map-less shutdown, `sv_hub 0`) drops it
  at once. One listing per (account, instance); a restarted server is a
  new instance and the old one expires.
- Text is cleaned: names to 64 characters, control characters out,
  map and minigame ids to `[a-z0-9_:-]`; anything else is 400.
- The hub never trusts the heartbeat's numbers for the list's order:
  clients query each server themselves (3.4).

### 3.2 Auth: who may list

- Listing needs a token whose account has the **`list`** right, a new
  field in accounts.toml (`list = 4`: how many servers at once; 0 or
  absent: none). `chimera_pack user add <name> --list <n>` sets it; a
  host token is an account with `list` and no games (`--games none`).
- Testers' tokens get `list = 1` or none (decision 3): with it a tester's
  listen server (Play) can be listed; without it only the host's servers
  are.
- Past `list`, or past a rate of one heartbeat per 10 s per listing:
  429 with the reason. Unknown tokens count toward the bad-token ban as
  on the asset routes. `chimera_assets hub delist <instance>` and
  `user revoke` drop a listing at once; a revoked account's listings go
  at its next heartbeat.

### 3.3 Reachability and NAT

- **The probe**: after a listing's first heartbeat (and every 5 minutes)
  the hub sends one A2S_INFO request from its own UDP socket to the seen
  address and port. It answers the heartbeat with `reachable: true |
  false | unknown`; the server prints it once ("hub: listed, reachable
  from the internet" or "hub: listed, but the hub can't reach UDP
  <port>: forward it or try net_upnp 1") and `status` shows it.
- The probe goes only to the address the heartbeat came from, at most
  once a minute per listing, so the hub can't be aimed at anyone else.
- **Unreachable listings** are hidden from the list by default and
  shown to their own account (the host sees "not reachable" on its own
  Internet tab); decision 4.
- NAT as today ([nat-traversal.md](nat-traversal.md)): a forwarded port
  or `net_upnp` works; carrier-grade NAT can't be listed reachably (the
  probe says so); a relay or hole punching is out of scope here and is
  that plan's (c)/(d). A tailnet hub sees tailnet addresses and lists
  them: one population per hub (6.3).
- IPv4 only at first (`ServerQueries` binds IPv4); an IPv6 heartbeat is
  listed when a later `ServerQueries` asks over IPv6.

### 3.4 The list and the Internet tab

- `GET /hub/servers` (token required, decision 5): the live listings,
  each `{ addr, lan[], name, app, minigame, map, players, max, bots,
  password, net, reachable, since }`, at most 500, as JSON. Cheap: from
  memory.
- **Internet** becomes the browser's first tab (`BrowserTab::Internet`,
  before LAN): its rows are `server_list::listed()` over the list's
  addresses, as Favourites are, so each server is **queried directly**
  for its info and ping with the existing unicast `ServerQueries::query`
  (the hub's numbers fill a row until the server answers). Sorted by
  ping, then asking, then silent (the Favourites order).
- Up to 64 queries at once (the rest queued), so a long list doesn't
  flood the player's link or trip `Responder`'s limits.
- A server of another `NET_VERSION` stays listed, greyed: "needs build
  <its version>"; another app's servers (the `app` field) are left out
  unless the page's filter asks for them.
- F5 refreshes (the list again, then the queries); the page opens with
  the cached list and asks for a new one.

## 4. First-run setup

### 4.1 The screen

A Signal page (`party_kit::signal_pages`, the look of Join and Settings)
shown before the main page on the first start, when there is no
hub.toml, no bound asset access and no skip mark, and the command line
doesn't play (`starts_at_menu`):

- **Hub link** (text field, paste): the link or code from the host.
- **Token** (text field, hidden as a password): empty when the link
  carried one (an invite, 4.2).
- **Connect** (primary): fetch the document over the pinned link, show
  the hub's name, MOTD and what it serves ("servers, game files,
  reports"), keep hub.toml, bind the token to the document's asset
  service (`access::Access::bind`, the same rule as an offer: the token
  only ever goes to that certificate). Errors in words: wrong link, hub
  unreachable, refused token, another certificate.
- **Skip: play from my installs / LAN** (secondary): writes the skip
  mark. Everything works as today: installs, LAN, Favourites, History,
  Direct; the Internet tab says "no hub: Settings > Network".
- Settings > Network: the hub (name, status), **Change hub**, **Forget
  hub** (removes hub.toml; the asset binding stays unless asked).

### 4.2 The link

`chimera-hub://<hub host>:<hub port>/<fingerprint>` (the fingerprint as
the 64 hex digits, or base32 to shorten it), with an optional
`?token=<token>` making it an **invite**. The hub prints its link at
start; `chimera_pack user add <name> [--list n]` prints that account's
invite, the one line to send privately. The same text works as
`+chimera:hub <link>` on the command line (kept before the app starts,
as `+chimera:asset_token` is), so `Play-UGG.bat +chimera:hub <invite>`
replaces today's token line. Registering `chimera-hub://` as an OS link
handler (a click in a chat opens the game) is decision 6.

### 4.3 Folding the asset token in

- With a hub, the token is entered once, at first run (or in the
  invite), and bound to the hub's asset service at once, so the startup
  download (`asset_startup`) can run before the first join and no join
  restarts the game for a token.
- The mounts are opened at startup, so a first run that binds a token
  for a game that isn't installed restarts once (the existing
  `AssetRelaunch` path, then the startup download's window). A later
  step can ask the hub link in the startup window instead and save that
  restart.
- `+chimera:asset_token` and a game server's `AssetOffer` keep working
  unchanged (no hub, a server with its own service); a server that
  offers another certificate than the hub-bound one is refused as now.
- Players with the games installed may skip, or connect for the list
  alone (an account with no games; their reports then upload too).

## 5. Hosting on this box

### 5.1 One process: routes on the asset service (recommended)

| Option | For | Against |
|---|---|---|
| **A. `/hub/*` routes on the asset service** (recommended) | One port, one certificate, one link for everything (the user's "one route"); the TLS front, accounts, limits, bans and access log already exist; feedback-reports.md puts `/feedback` there too; nothing new to open on the router | The asset crate grows a third job (split later, 5.4); a hub restart restarts downloads |
| B. Its own binary and port | Separate restarts, could run on another machine | A second port, certificate and token story; the link would still have to point at a document that names the asset service: back to A's shape |
| C. A Valve-style UDP master protocol | Familiar to Source tools | A second protocol and port, no TLS, no tokens; our clients query servers by A2S already and need only the list |

A, with the code in its own module so it can move: `chimera_asset_server
--hub` turns the routes on (off by default; the asset service alone
stays as it is).

### 5.2 Ports, TLS, limits

- **TCP `<hub port>`**: the asset service's `--tls` port, unchanged; the
  hub link points at it. Its plain listener stays on 127.0.0.1:8731 for
  this machine's tools and for a dedicated server on this box (plain
  HTTP to localhost is already allowed by the client).
- **UDP**: none inbound for the hub; the probe goes out from an
  ephemeral port. Game servers keep their own UDP ports (27015...).
- **TLS**: the service's own certificate, pinned by the link; no domain,
  no CA. Behind a Cloudflare Tunnel or `tailscale serve` later the
  front's certificate changes, so the link changes too (6.3).
- **Limits** (new `Limits` fields, CLI flags): `/hub` and
  `/hub/servers` 2 a second per account (bursts of 20); heartbeats as
  3.2; at most 4 listings per address whatever the accounts; 500
  listings in all; the existing 48 connections per address and the
  bad-token ban. Past a limit: 429 and an access-log line.

### 5.3 Tailnet or public

- Today the asset service's TLS port faces the internet during
  playtests (playtest-hosting.md), with internal accounts, and the asset
  plan's decision says Steam sign-in must be on before it is opened up
  for good. The hub follows the same line: **token-only** (the document,
  the list and heartbeats all want a token), public only while a
  playtest runs, otherwise reached over the tailnet (decision 7).
- A hub over the tailnet lists tailnet addresses (players must be on
  it); a public hub lists public addresses. Mixing needs two links.
- The VPS later (the asset plan's hosting decision): the same binary
  there; hub.toml's `assets.url` may point back at this box with its
  own fingerprint until the store moves.

### 5.4 Code layout (layering)

Each crate's folder is its layer (checks/tests/it/architecture.rs):

- **`chimera_formats::a2s`** (pure data): the A2S packets' codec
  (`ServerInfo`, `encode`, `parse_reply`, `info_request`, challenges)
  lifted from `chimera_net::net::query`, which re-exports it; the
  Bevy-side `Responder`, `ServerQueries` and cvars stay in
  `chimera_net`. Needed so the hub (pure data) can probe without
  depending on the network crate. `chimera_assets` may use
  `chimera_formats` (the neutral layer).
- **`chimera_assets::hub`** (pure data, no Bevy): `discovery` (the
  document, hub.toml's host side, signing and checking, the link and
  invite format), `registry` (listings, heartbeat validation, expiry,
  per-account and per-address caps, the probe's schedule), `client`
  (blocking fetches of the document and list and the heartbeat over
  `remote`/`tls`), the client cache (`HubAccess`, like `access`); the
  routes in `server.rs`; accounts' `list` right; `chimera_assets hub
  link | servers | delist` and `chimera_pack user add --list`. If it
  outgrows the crate it moves, with feedback, to an engine crate
  `chimera_services` (a `workspace_crates` row, `cargo hakari
  generate`); not before.
- **`chimera_net::net::hub`** (engine): the server's heartbeat thread
  from `LocalServerInfo` (cvars `sv_hub` 0/1, `chimera:hub`,
  `chimera:hub_token`; `[hub]` in the dedicated server's local config),
  the reachability line in `status`; the client's `Hub` resource (the
  cached document, refresh thread, the list fetch) feeding
  `ServerQueries`.
- **`chimera_client::client`**: `+chimera:hub` kept before the app
  starts, the relaunch after a first-run bind (`relaunch.rs`).
- **`party_kit`** (app): `server_list::BrowserTab::Internet`, the
  first-run page and Settings > Network in `signal_pages`/`settings`,
  the MOTD and news line on the main page, the Play row "List on the
  hub" (a `[[play]]` row in data/main_menu.toml on `sv_hub`).

No change to the game protocol: the heartbeat is HTTPS, the A2S reply
is unchanged, so no `NET_VERSION` bump.

## 6. Phases

| Phase | What | Size | Tests |
|---|---|---|---|
| **H0** (now, no code) | The host sends address and token privately; Direct, Favourites, History | none | none |
| **H1** the hub's server side | `chimera_assets::hub` discovery (hub.toml, signing, the link), registry with expiry, `/hub`, `/hub/servers`, `/hub/heartbeat`, `/hub/leave`, the `list` right, limits, `--hub`, the CLI | 3-4 h | `chimera_assets` it tests on port 0 over the TLS front: the document's signature checks with `GET /key`'s key; a pinned client refuses another certificate; heartbeat without a token 401, without `list` 403, past `list` 429; the address listed is the peer's, never the body's; loopback heartbeats listed at `public_address`; expiry after 3 missed beats (a test clock), leave drops at once; delist and revoke; bad names 400 |
| **H2** servers heartbeat | `chimera_net::net::hub` server side; dedicated servers from `[hub]` or `+chimera:hub`/`+chimera:hub_token`; listen servers by the Play row | 2-3 h | a dedicated server (headless `dedicated`) against a hub on port 0: listed with the right fields, updated when a bot joins and a password is set, left on quit, expired when killed; nothing sent with `sv_hub 0` or no token |
| **H3** the probe | `chimera_formats::a2s` lifted (`chimera_net` re-exports; its query tests kept); the hub's probe; `reachable` in the answer and `status`; hidden-unless-own rule | 2 h | the probe against a real `Responder` on loopback (reachable) and a closed port (not); the probe never goes to an address other than the heartbeat's; the codec's round-trip tests moved |
| **H4** client and Internet tab | `Hub` resource, cache and refresh; `BrowserTab::Internet` first; direct queries, 64 at a time, ping sort, version greying, app filter; MOTD/news line; the min-client banner | 3-4 h | `party_kit` `signal_pages.rs`: rows from a hub list on a NetSim pair, sorted by ping, a silent one last, another `NET_VERSION` greyed, another app left out; offline uses the cache with its note; a document signed by another key refused; `ui_scene join internet` screenshots |
| **H5** first-run setup | The first-run page, invite and `+chimera:hub`, binding the token, the one restart, Skip, Settings > Network | 3 h | `main_menu.rs`: first start shows the page, a second start doesn't; Connect with an invite binds the token to the document's service and keeps hub.toml; Skip leaves installs, LAN and Direct as today; a wrong link, an unreachable hub and a refused token each say so; with an install nothing is fetched; screenshots |
| **H6** folding the rest | A server without `chimera:asset_url` offers the hub's asset service; feedback-reports.md's endpoint order gains "the hub's `feedback`" first; a joined server relays the hub's document (signed) to clients without one; docs | 2 h + docs | offers from the hub's document; a relayed document with a bad signature ignored |
| Release | A playtest build with H1-H5 (all players and the server update; `NET_VERSION` unchanged by this plan) | user's go | `tools/post-push-tests.sh`; a fresh Windows PC: paste an invite, see the list, join |

Docs to change with the phases: docs/playtest-hosting.md (starting the
hub, its link and invites, hub.toml, ports, delisting), docs/playtest.md
(first run: paste the invite), assets-for-developers.md (the hub link as
an alternative to `[assets]`), main-menu.md (M3 as built), DESIGN "The
party apps' main menu", each new file's `//!` doc (`docs_index`).

## 7. Risks

- **One process, three jobs**: a hub bug can take the downloads down
  mid-playtest. The routes are behind `--hub`, and H1's tests run the
  asset routes beside them.
- **Junk listings**: tokens, the `list` right, per-address caps, cleaned
  text, delist and revoke; the probe hides what nobody can reach.
- **The hub's address changes** (a home connection): the link names the
  certificate, not only the address, and the asset binding already
  survives an address change; the link with the new address is sent
  again, the token stays.
- **A lost `tls` folder or key**: every player pastes a new link (as
  tokens today); keep `<server store>/server/` backed up.
- **Exposure**: a public hub with internal accounts is the same risk the
  asset service already takes during playtests; Steam sign-in before
  it stays open (the asset plan's rule).

## 8. Alternatives considered (short)

Valve's master server protocol (UDP, no auth, a second port); a static
JSON list on a web host (no heartbeats, edited by hand); Steam's server
browser (needs an app id: nat-traversal.md section 1); listing through
each game server (no first contact without an address); a list without
a discovery document (the user wants one address for every service).

## 9. Decisions for the user

1. **Where the hub runs:** routes on the asset service process on this
   box, behind `--hub` (recommended), or a separate binary.
2. **The link:** a link with the fingerprint, and invites that carry the
   token too (one paste; recommended), or the link and the token as two
   things to enter.
3. **Who may list a server:** host tokens only, or every tester's token
   with one listing (`list = 1`) so their Play games show up
   (recommended: testers get 1).
4. **Unreachable servers:** hidden from others, shown to their own host
   with the reason (recommended), or listed greyed for everyone.
5. **Who may read the list and the document:** token holders only
   (recommended while accounts are internal), or public.
6. **`chimera-hub://` as an OS link handler** (a click in a chat opens
   the game with the link) in a later phase, or paste only.
7. **Reach:** public only during playtests, the tailnet otherwise
   (recommended, as the asset service now), or always public.
8. **First run:** the hub page on every fresh start until connected or
   skipped (recommended), or only from Join's Internet tab.
9. **News and MOTD:** in the hub's hub.toml edited by hand (recommended),
   or a `chimera_assets hub news add` command; and whether Discord update
   posts should also land there.
10. **Min client:** a banner only (recommended), or refuse to list
    servers to an old client.

## 10. North Star: accounts by redeem code

Not for now (user, 2026-11-14): none of H1-H6 depends on it, and the
tokens work until it is built. The idea: a login that admits only the
people allowed to test, as simple as possible for the host and for the
player, with nothing to type but the hub link and a code. A usernameless
account (display names change at will) is also the base for a longer-term
identity and, later, meta progression.

**Host side.** One command on the hub box mints codes, for example
`chimera_pack code mint <n> [--label <text>] [--role tester|host]
[--expires <days>] [--games a,b]`; each code is single-use and printed
once (only its hash is stored, like tokens today). `code list` shows
label, role, state (unused, redeemed by which account, expired) and
`code revoke <code or label>` kills an unused code. The accounts file
grows from "name and token hashes" to account id, role, rights (`list`,
games), limits, a note, and the device credentials' hashes.

**Player side.** First-run setup (4.1) asks for the hub link and the
code, nothing else; an invite link can carry the code, so one paste does
both. Redeeming (`POST /hub/redeem`, over the pinned link, rate limited
per address) creates an **account**: an opaque random id plus a **device
credential** the client keeps in its local access file. That one
credential replaces the per-tester asset token: asset access, feedback
uploads (feedback-reports.md) and server listing (3.2) all derive from
the account's rights, so there is one thing to store, bind and revoke.
Display names stay free to change in Settings and are never an account
key; the hub may record the last name seen, for the host's eyes only.

**Recovery.** Losing a device or reinstalling is the host's job to fix
in the simplest case: re-issue a code **bound to the same account**
(`code mint --for <account>`), which redeems into a fresh device
credential and revokes the old one. Optionally Settings shows a backup
**account key** (a long phrase of words) that the player can store and
paste at first run to recover alone; whether to offer it is a decision
when this is built, since it makes the key a bearer secret.

**Fit with the hub's phases.** Section 4.2's invite already carries a
token; the redeem code is that token's replacement: the same invite
shape (`?token=` becomes `?code=`), the same first-run field, the same
`+chimera:hub` line. Migration: the hub keeps accepting today's tokens
as legacy device credentials of an account made for them, so existing
testers change nothing; `chimera_pack user add` turns into a shortcut
for mint-and-redeem for the host's own devices; a legacy token can be
swapped for a code-made credential whenever the host re-invites. Build
it after H5, as a new phase, once the token flow in H1-H6 has shown
what testers trip over.

**Later uses.** Session stats and meta progression stored on the hub by
account id (levels, unlocks, match history; the hub then also needs
backups and a say in what clients may claim, so servers report results
to it, never the client); report attribution (a report carries the
account id, so repeated reporters and repeat crashes are visible);
bans by account (revoke the credential and the account's id stays
barred, so a new code can't be minted for it by mistake).

**Privacy.** No email, password, real name or phone number: a code is
the only thing handed out, the id is random, and the hub stores the id,
role, rights, credential hashes, last-seen time and (for the host) the
last display name. A player can ask the host to delete the account;
that removes the id and its stats. Nothing here is written into the
repository (placeholders only, as above).

