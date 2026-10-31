# Game files from the asset service (developers without installs)

You don't need Counter-Strike: Source or Combat Arms installed (or Combat
Arms' keys) to play, playtest or run the tests, heavy tier included. Our
dev server (the asset service, docs/plans/active/asset-service.md) serves
each game's files; Chimera's mount fetches the ones a run reads, checks
each against the signed manifest and keeps them in a per-user cache. Loaders,
tests and tools don't change: they read through the mount as they would
from an install.

## Setup (once)

1. Ask the server's admin for an account. They run `chimera_pack user add
   <you> --note <device>` and send you a URL and a token (one token per
   machine; revocable).
2. Put both into `chimera.local.toml` at the checkout's root (git-ignored,
   never committed; `chimera.local.example.toml` shows the section), or
   point `CHIMERA_CONFIG` at a file elsewhere:
   ```
   [assets]
   url = "https://<the server's name>"
   token = "chimera1_..."
   ```
   Nothing else is needed: no `[games.*]` paths, no keys. The environment
   wins over the file (`CHIMERA_ASSET_URL`, `CHIMERA_ASSET_TOKEN`).
3. Check them and pin the server's key:
   ```
   cargo run -p chimera_assets --bin chimera_assets -- login
   ```
   It prints who you are and a `key = "..."` line; add it to `[assets]`
   (with it, a manifest signed by any other key is refused).
4. Optional: fetch ahead (otherwise files arrive on demand, see below):
   ```
   cargo run -p chimera_assets --bin chimera_assets -- fetch cs_source base map/de_dust2 players
   cargo run -p chimera_assets --bin chimera_assets -- fetch cs_source heavy-tests
   cargo run -p chimera_assets --bin chimera_assets -- fetch combat_arms      # every file
   ```
   On a terminal it shows one live line (records and MB done of the total,
   speed, time left), elsewhere a line every 5 s, then a summary.

What the server has (sizes as stored, i.e. transferred; build 25338318 of
CS:S, 26236938 of Combat Arms, measured 2026-10-31):

| | CS:S | Combat Arms |
|---|---|---|
| every file the game mounts (`fetch <game>`; CS:S: with the publisher's content cache, 89 community maps) | 47,692 records, 4.4 GB (7.5 GB plain) | 29,431 records, 3.8 GB (9.8 GB plain; music included) |
| `base` (menus, fonts, UI sounds) | 1.5 MB | 9.7 MB |
| `map/de_dust2`, `map/warehouse` (a client's load, HUD and kit included) | 143 MB | 93 MB |
| `weapon/<id>` (view and world models, materials, sounds; CA: view models with hands and parts) | 29 sets, 0.7-1.9 MB each, 21 MB together | 2,363 sets, 0.4-20 MB each, 850 MB together |
| `players` (the eight class models) | 12.9 MB | - |
| `music` (`Game/BGM`, loose files) | - | 72 MB |
| `heavy-tests` (what the whole suite reads) | 15,703 records, 922 MB | 13,322 records, 2.6 GB |

`chimera_assets status` lists the sets and sizes of what you have.

## Day to day

Work as with an install: `cargo run --features dev`, `cargo test ...`,
`cargo nextest run --features dev`, `mapsweep`, the dedicated servers. When
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

## The cache

- Linux `~/.cache/chimera/assets`, Windows `%LOCALAPPDATA%\chimera\assets`;
  one per user, shared by every checkout, worktree and agent slot.
  `CHIMERA_ASSET_STORE` or `[assets] store` moves it.
- `records/<ab>/<id>.zst` (each file's bytes, zstd), `manifests/<game>/
  <build>.txt` (+ `.sig`). Nothing in it belongs in the repository.
- `chimera_assets status | verify | gc [--dry-run]`. It has no size cap
  yet: delete the folder (or `records/`) to start over; files come back on
  demand.

## How updates arrive

The repository's `assets.lock` (when there is one) pins each game's build
and manifest id. When a branch changes it, the next run finds its kept
manifest isn't the pinned one and fetches the pinned manifest; the files
it changed arrive on demand. Without a lock, a game is served at the build
first fetched (the newest on the server then); `chimera_assets fetch
<game>` takes the server's newer copy of that build's manifest (new sets),
and `chimera_assets lock` writes the server's newest builds into
`assets.lock` for a commit.

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

## When something goes wrong

- `asset service <url> (<game>): ... HTTP 401`: the token is missing,
  mistyped or revoked; ask the admin for a new one. `403`: your account
  isn't allowed that game. The service is asked once per game and process;
  after a failure the game counts as not installed (tests skip, apps
  report no install).
- Can't connect / timeouts: the server is private (Tailscale): check you
  are on the tailnet and the URL is `https://`. Plain `http://` is
  refused except to this machine.
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
