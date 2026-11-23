# Game files from the asset service (developers without installs)

You don't need Counter-Strike: Source or Combat Arms installed (or Combat
Arms' keys) to play, playtest or run the tests, heavy tier included. Our
dev server (the asset service, docs/plans/active/asset-service.md) serves
each game's files; Chimera's mount fetches the ones a run reads, checks
each against the signed manifest and keeps them in a per-user cache. Loaders,
tests and tools don't change: they read through the mount as they would
from an install.

## Setup (once)

1. Ask the hub's owner for an invite (`chimera_accounts invite`; a link
   `chimera-hub://...`, sent privately, single use).
2. Sign in with it once, as your user on this machine:
   ```
   cargo run -p chimera_assets --bin chimera_assets -- hub "<invite link>" <username>
   ```
   (or paste it on a game app's first-run screen, or start one with
   `+chimera:hub <invite link>`). The account lands in
   `<data_local>/chimera/account.toml`, the recovery key beside it (save
   it elsewhere). Every checkout, slot, test and tool of this user then
   reads from the hub's asset service with the account's tickets: no
   `[games.*]` paths, no keys, nothing in `chimera.local.toml`.
   (`CHIMERA_ACCOUNT` names another account file.)
3. Check it:
   ```
   cargo run -p chimera_assets --bin chimera_assets -- login
   ```
   It prints who you are on which service.
4. Optional: fetch ahead (otherwise files arrive on demand, see below):
   ```
   cargo run -p chimera_assets --bin chimera_assets -- fetch cs_source base map/de_dust2 players
   cargo run -p chimera_assets --bin chimera_assets -- fetch cs_source heavy-tests
   cargo run -p chimera_assets --bin chimera_assets -- fetch combat_arms      # every file
   cargo run -p chimera_assets --bin chimera_assets -- fetch hl2dm heavy-tests
   ```
   On a terminal it shows one live line (records and MB done of the total,
   speed, time left), elsewhere a line every 5 s, then a summary.

What the server has (sizes as stored, i.e. transferred; build 25338318 of
CS:S, 26236938 of Combat Arms, 24600434 of HL2:DM; measured 2026-10-31, HL2:DM
2026-11-03):

| | CS:S | Combat Arms | HL2:DM | Halo (on the server since 2026-11-15) |
|---|---|---|---|---|
| every file the game mounts (`fetch <game>`; CS:S: with the publisher's content cache, 89 community maps) | 47,692 records, 4.4 GB (7.5 GB plain) | 29,431 records, 3.8 GB (9.8 GB plain; music included) | 30,792 files, 30,390 records, 1.9 GB (3.2 GB plain; its `hl2/` records are CS:S's, shared) | 30 files, 30 records, 2.50 GB (2.51 GB plain: the maps are zlib inside and the videos Bink, so zstd gains 0.5%; `xdemos/` left out) |
| `base` (what every start and match reads: menus, fonts, data, what every match loads; lean since 2026-11-15, `chimera_pack lean`) | 1,753 files, 57 MB | 2,224 files, 63 MB | 1,475 files, 55 MB (scripts, decals, impact and footstep sounds, every gun and player model) | the donor map `maps/bloodgulch.map`, 1 record, 22.5 MB |
| `common` (what a match may play beyond `base`: event sounds, debris, loading images; each join) | 13 files, 0.6 MB | 447 files, 19 MB | 85 files, 1.5 MB | - |
| `map/<name>` for every installed map (2026-11-12, `chimera_pack sets`; the pool's also from a client's load) | 109 maps, 0.3-196 MB each (`map/de_dust2` 143 MB) | 105 maps, 0.9-174 MB each (`map/warehouse` 93 MB) | `map/<dm_lockdown, dm_overwatch, dm_powerhouse, dm_resistance, dm_runoff, dm_steamlab, dm_underpass, halls3>`: 2,014-3,318 records, 90-153 MB each (the app on the map) | 13 maps, one record each, 19.6-24.8 MB (`map/hangemhigh` 21.3 MB; `map/bloodgulch` is `base`'s record) |
| `weapon/<id>` (view and world models, materials, sounds; CA: view models with hands and parts, the held model and the gun's own sounds) | 29 sets, 0.7-1.9 MB each, 21 MB together | 2,363 sets, 0.4-20 MB each, 1.0 GB together (2026-11-11, with the held models) | 12 sets, 0.7-1.8 MB each, 13 MB together (view and world models, projectile models, materials, sounds) | - (every Halo weapon is in the donor map, `base`, which a match offering a Halo gun names: a `weapon/<id>` the offer names is absent and skipped) |
| `character/<record>` (a rung's body: model, faces, head pieces, textures, and its rung look's first-person hand model and sleeves when they aren't the default character's; 2026-11-12) | - | 691 sets, 1,729 records, 378 MB together (the hands 322 records, 73 MB); beyond `base` and `players` 0.3 MB at the median, the hands 0.3 MB more | - | - |
| `players` (the eight class models) | 12.9 MB | 257 files, 66 MB (the stock characters, footsteps) | 166 records, 18 MB (the 19 player models) | the donor map, `base`'s record |
| `music` (`Game/BGM`, loose files), and `music/<track>` per file | - | 71 MB; 64 track sets, 0.9 MB at the median | - | - |
| `heavy-tests` (what the whole suite reads) | 15,703 records, 922 MB | 13,322 records, 2.6 GB | 6,598 records, 366 MB | - |

`chimera_assets status` lists the sets and sizes of what you have.

**Halo** (`halo_ce`): its install is the Xbox disc image
(`CHIMERA_HALO_CE_ROOT`), so its build is no Steam build but the build
string its maps' headers carry (`01.10.12.2276` for the NTSC disc,
`halo_ce::game::mount::build`), and the store serves it instead of the
disc, never under it (a player has the whole disc or none of it).
`chimera_pack pack halo_ce` packs the game's own files: the 24 map files
(the 13 multiplayer maps, the menu's and the ten campaign maps, up to
187 MB each, one record apiece), `default.xbe` and the five `bink/`
videos; it leaves out `xdemos/` (1,136 files, 1.2 GB: other games'
demos the disc carries, which nothing in Chimera reads; `--all-files`
packs them). Measured on the Windows PC: 125 s on 4 threads, a peak of
1.6 GB of memory (whole records of up to 200 MB in memory at once).
`chimera_pack sets halo_ce` makes `base` the donor map
(`maps/bloodgulch.map`: every multiplayer map holds the same weapons and
bipeds, so Halo's guns and the Spartan will be read from it on any map),
`players` the same record, and `map/<name>` each map's one file;
`weapon/<id>` sets would all be the donor map and aren't recorded: a
match offering a Halo gun names its `base` too. A client with the store and no disc plays Halo's maps
from it (`CHIMERA_ASSET_USE_INSTALL=0`, or no disc configured). Halo's
`base` is fetched with a match that uses Halo (its offer names it with the
map, gun or body), not at a client's start-up like the other games'
(`sets::startup_sets`). A manifest packed elsewhere is published by
copying it and its records into the server's store and running
`chimera_assets --store <server store> sign halo_ce` there (the light
binary, no Bevy: `cargo build --release -p chimera_assets --bin
chimera_assets`); `chimera_pack sign halo_ce --store <server store>` is the
alternative.

Players rather than developers (a playtest over the internet, no
checkout): they sign in with their invite on the first-run screen, the
game server names the service at join and the client downloads what the
match needs ([playtest-hosting.md](playtest-hosting.md)). The service is
reached over HTTPS through the hub's front, its self-signed certificate
pinned by the fingerprint in the invite link.

**An asset service with no hub** (`chimera_asset_server --no-tickets`,
e.g. a game server's own) has its own tokens: its admin runs
`chimera_pack token add <you> --note <device>` and sends a URL and a
token, which go into `chimera.local.toml` (git-ignored;
`chimera.local.example.toml` shows the section; the environment wins:
`CHIMERA_ASSET_URL`, `CHIMERA_ASSET_TOKEN`):
```
[assets]
url = "https://<the server's name>"
token = "<the token>"
```
`chimera_assets login` then prints a `key = "..."` line to add (a
manifest signed by any other key is refused). A player of such a server
enters the token once with `+chimera:asset_token <token>`; the first
join binds it to that service's certificate
(`%LOCALAPPDATA%\chimera\asset-access.toml`, used when `[assets]` names
no service).

## Installs and the service

Players play the published build's files and data, installed games or
not: the same files for everyone (TF2's decrypted scripts and the
declared sets included), and no keys needed. So the game apps and their
servers read a game the asset service serves from the store alone
whenever a service is set (a hub account, `[assets] url`/`token` or their
variables, a token bound by a join), the install unused
(`chimera_assets::config::InstallDefault`, set at an app's start).
Tests and tools keep the install first, so the suite reads the installs
as before and stays offline-safe.

| | no install | install |
|---|---|---|
| **no service**, any process | nothing (a test skips) | the install (as before) |
| **service**, a test or tool | the store, filled from the service | the install first, the store under it (adds only what the install lacks) |
| **service**, a game app or server (the default) | the store, filled from the service | **the store**, filled from the service; a game the service doesn't serve (TF2 today) reads its install |
| **service**, the install chosen (Settings ▸ Network ▸ "Use my installed games", `[assets] use_install = true`, `CHIMERA_ASSET_USE_INSTALL=1`, a server's `+chimera:asset_use_install 1`) | the store, filled from the service | the install first, the store under it |
| a client whose hub account, or token for a service with no hub (entered with `+chimera:asset_token`, not bound yet), waits for its first join | nothing until the join binds it | hidden until the join binds the service, then as with a service (a game the service doesn't serve: its install) |
| `use_install = false` / `CHIMERA_ASSET_USE_INSTALL=0`, anywhere | the store alone | the store alone (a game it doesn't serve: none) |

The setting is `chimera:asset_use_install` (archived in config.cfg, read
at the start: a change applies at the next start). A developer who runs
the apps with the installs and a service configured reads the store
like a player; to work from the installs set `use_install = true` in
`[assets]` (or the variable), or turn the setting on. Offline with a
service configured: a game whose manifest the store lacks falls back to
its install (the service can't serve it); one whose manifest it has
reads what the store holds and misses the rest, so set
`CHIMERA_ASSET_USE_INSTALL=1` to work from the installs offline.

## Day to day

Work as with an install: `cargo run --features dev`, `cargo test ...`,
`cargo nextest run --features dev`, `mapsweep`, the dedicated servers. A dedicated
server fetches its games' `base` and start map before it starts and each
match's sets before the match (playtest-hosting.md, "The server's own
game files"). When
a run opens a file the cache lacks, the mount fetches it together with the
rest of its group in one request: the missing files of the smallest set
holding it (a map's set, a weapon's), else the files beside it with the
same name (a model's `.mdl`, `.vvd`, `.vtx`, `.phy`). While that happens
the run prints, every 2 s,

```
assets: downloading from https://<server>: 1203 files, 61.0 MB so far, 9.8 MB/s
assets: fetched 2511 files, 142.7 MB from https://<server> in 14.2 s (10.0 MB/s)
```

and nothing when nothing is fetched. A first load is as slow as the link;
later ones read the cache.
`CHIMERA_ASSET_FETCH_LOG=<file>` writes a line per on-demand fetch (the
set its group came from, or `-` for none): what a run reads outside
the sets it fetched. A game client also prints each on-demand fetch made
while a match runs (`assets: fetched on demand during the match on
...`, with the match's running total) and each match's total when it
ends; the service marks those requests ` on-demand` in its access log.

### The sandbox's guns: units on demand

In the sandbox (Ultimate Gun Game's `sandbox`), a gun's files come when
it's picked, not with the join: the Change gun view shows each gun's
size before anything downloads ("not here: 4.8 MB"), and a pick fetches
its unit (`weapon/<id>`; a Halo gun's is Halo's `base`) with what it needs
(its game's `base`, `players` and `common`, TF2's class body) in the
background at `chimera:unit_rate` KB/s (4000; 0: no cap), while you keep
the gun in hand. `unit_fetch <game> <set>` on the console asks for one,
`unit_fetch` alone lists them. Only the sandbox fetches this way (the
no-fetch-in-match rule: not in a match, not in the menu; voices,
`voice/<id>`, are the exception), and leaving the sandbox unqueues what
was waiting (the fetch already running ends by itself); the match's files come with the join as before. A server
fetches a picked gun into its own store the same way
(`chimera_net::net::units`).

## The cache

- Linux `~/.cache/chimera/assets`, Windows `%LOCALAPPDATA%\chimera\assets`;
  one per user, shared by every checkout, worktree and agent slot.
  `CHIMERA_ASSET_STORE` or `[assets] store` moves it.
- `records/<ab>/<id>.zst` (each file's bytes, zstd level 19; level 3 before 2026-11-15), `manifests/<game>/
  <build>.txt` (+ `.sig`). Nothing in it belongs in the repository.
- `chimera_assets status | verify | gc [--dry-run]`. It has no size cap
  yet: delete the folder (or `records/`) to start over; files come back on
  demand.

## Converted maps

Beside it, `~/.cache/chimera/maps` (`%LOCALAPPDATA%\chimera\maps`) keeps
maps as converted (CS:S maps whole; Combat Arms' worlds only with
`CHIMERA_MAP_CACHE_CA=1`, they convert quickly), one file per
map, options and converter version, in our own format
(chimera_world's `map::cache`): the next load of the same map, by any
test, tool or app in any checkout, reads it back instead of converting.
Install files the conversion read are checked on each hit (archive
checksums, store record ids, else a hash), so a patched install converts
again; so does a change to what a converter's version covers (its
conversion's sources and data, its number: docs/formats.md), not a
gameplay edit. The guests' dressings (each game's players and guns on
other games' maps) are kept there too, and when the store alone serves a
game (no install) the asset service's converted copy is downloaded
instead of converting, if one was made for this build's converter
(`chimera_pack convert`; `CHIMERA_CONVERTED_SERVED=0` never takes one).

- `CHIMERA_MAP_CACHE=0` turns it off; `CHIMERA_MAP_CACHE=<folder>` moves
  it; `CHIMERA_MAP_CACHE_GB` caps it (40 by default; least recently used
  entries go first; a whole CS:S map is about 400 MB, every crate's heavy
  tests' maps 24-28 GB). `mapsweep --no-map-cache` converts every map;
  in a game or server, `chimera:map_cache 0` (console, or
  `+chimera:map_cache 0` on the command line) does the same from the
  next map load.
- Deleting the folder is always safe. Nothing in it belongs in the
  repository.

## How updates arrive

The repository's `assets.lock` (when there is one) pins each game's build
and manifest id. When a branch changes it, the next run finds its kept
manifest isn't the pinned one and fetches the pinned manifest; the files
it changed arrive on demand. Without a lock, a game is served at the build
first fetched (the newest on the server then); when the server publishes
that build again (new or bigger sets), a game client's next start and
its next join's download take the newer manifest (`sets::refresh`), then
fetch the sets' new files with the rest, and `chimera_assets fetch
<game>` takes it too. `chimera_assets lock` writes the server's newest
builds into `assets.lock` for a commit.

## What still needs an install

Everything that reads the game through its mount runs from the service,
the community maps the heavy tests load included (they are in the CS:S
mount on the publishing machine, from its content cache, so a whole
publish carries them). A few checks examine the install itself and skip
with a note: `lithtech_formats`' archive tests (`.rez` files and their
keys: the format crate reads archives, not the mount), the CS:S mount's
own VPK and layer checks, Combat Arms' `BGM.rez`, `ATTRIBUTES.rez` and
archive listings, the `heavy::served` tests (they pack from an install),
`dump`. Local demo and probe files (`target/demos`, probe logs) aren't
served either: those tests skip as they do for us without them.

## Day of Defeat: Source: an install of your own

DoD:S (docs/plans/active/dods.md) is served from 2026-11-23 (the
dedicated server's build 25338204, published with `--build`: the
SteamCMD layout has no app 300 manifest): every weapon's
`weapon/<id>` (the guns, knife, spade and explosives), `players` (the
twelve class bodies with the shared animation model) and `map/<name>`
for the nine stock maps, its guest dressing converted. Its tests and the
party's `dods:` maps read an install, found as the other games' are:
`$CHIMERA_DODS_ROOT` for one run, else `[games.dods] path` in
`chimera.local.toml`, else Steam's libraries (app 300, "Day of Defeat
Source"). The dedicated server (app 232290, anonymous) has everything
the party reads, about 1.8 GB:

```
steamcmd +force_install_dir <folder> +login anonymous +app_update 232290 validate +quit
```

A SteamCMD install outside Steam's libraries needs the config line or
the variable (a folder holding `dod/`, `hl2/` and `platform/`). The
scripts are plain text: no key.

## Team Fortress 2: an install of your own, or the service

TF2 (docs/plans/active/tf2.md) is served from 2026-11-15 (its guns'
`weapon/<id>`, since 2026-11-23 each unlock's too, `weapon/<name>` (its
own model and sounds: docs/plans/active/tf2-weapons.md), `map/ctf_2fort`, `base`, and since 2026-11-16 its bodies per
class: `players/<class>`, its voice's waves too since 2026-11-18,
offered only for the classes a match's TF2 guns make, with `players`
holding what every class shares, the HUD's look and the kill icons). Its tests and the party's `tf2:` maps read an install,
found as the other games' are: `$CHIMERA_TF2_ROOT` for one run, else
`[games.tf2] path` in `chimera.local.toml`, else Steam's libraries
(app 440). Two layouts work:

- **The client** (Steam app 440, "Team Fortress 2"; free to play, but
  even through SteamCMD it needs a Steam login: `steamcmd +force_install_dir
  <folder> +login <your account> +app_update 440 validate +quit`):
  everything, maps, models, textures, sounds. The heavy tests' "no
  missing material" checks need it.
- **The dedicated server** (app 232250, anonymous):
  ```
  steamcmd +force_install_dir <folder> +login anonymous +app_update 232250 validate +quit
  ```
  About 15 GB: maps, models, material text, scripts, but no textures
  and no sounds (their VPKs' `_dir` files are there, their archives
  aren't; the mount leaves those files out). Maps convert and play with
  stand-in textures and no sound; geometry, collision and spawns are the
  same. `CHIMERA_TF2_SERVER_ROOT=<folder>` lets `cargo test -p tf2
  --features dev` check this layout too.

Both are folders holding `tf/`, `hl2/` and `platform/`; a SteamCMD
install outside Steam's libraries needs the config line or the variable.

**The weapon scripts' key.** TF2's weapon and class scripts
(`scripts/*.ctx`) are encrypted (specs/tf2/ice.md). Reading them from an
install needs an 8-character key that **you supply locally**: it is not
in the repository, the specs, the docs or any log, and never should be.
Put it in `CHIMERA_TF2_CTX_KEY` for one run, or in `chimera.local.toml`
(the commented `[games.tf2.keys.ctx]` section of
`chimera.local.example.toml`). Without it, a test that needs the scripts
passes with a note. Once TF2's sets are on the asset service, their
records hold the scripts already decrypted (the host packs them with its
key), so a developer or player served from it needs no key. `dump tf2
--script <name>` prints one script to stdout; `dump tf2 --ice-probe` says
which ICE level decrypts the files with your key and how many of the
install's `.ctx` scripts decrypt (Thin-ICE, all of them, on 2026-11-15).

## When something goes wrong

- `asset service <url> (<game>): ... HTTP 401`: your device or account
  was revoked (or, with a service of no hub, the token is mistyped or
  revoked); ask the owner for a new invite. `403`: your account isn't
  allowed that game (its `content`). The service is asked once per game and process;
  after a failure the game counts as not installed (tests skip, apps
  report no install).
- Can't connect / timeouts: the hub's front (or a service with no hub)
  isn't answering: check with its owner that it is up and its port open,
  and that the URL is `https://`. Plain `http://` is refused except to
  this machine.
- `the server's key isn't the one pinned`: the server was re-keyed or the
  URL points elsewhere; check with the admin before changing `key`.
- `asset refused <game>: <path>: record ...: hash mismatch`: a record
  arrived or sits damaged; it was not used. Run `chimera_assets verify`
  and delete the files it names (they are fetched again).
- `asset miss <game>: <path>`: the file is in the manifest but couldn't be
  fetched (offline, or the server lacks it): the message says why.
- `manifest ..., but assets.lock pins ...`: the server doesn't have the
  pinned build any more, or the lock is newer than the server: update the
  branch or ask the admin to publish.

Windows: the same commands in PowerShell; the cache is under
`%LOCALAPPDATA%`; the client uses rustls (no system TLS library needed).
