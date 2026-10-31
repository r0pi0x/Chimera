# Plan: assets served by the host (ownership-checked)

Status: M1 built 2026-10-30 (section 9, "M1 results"); M2 built with
internal accounts 2026-10-30 (section 9, "M2 results", with the
deployment steps), not yet deployed; made ready for an outside developer
2026-10-31 (section 9, "M2 for developers": whole-game publishes, the
sets, batched on-demand fetches, progress, tests from the service; the
guide is docs/assets-for-developers.md); Steam sign-in waits for the
milestone that opens the server. Decided: the hybrid model
(docs/DESIGN.md, "Assets, code and legal"; backlog roadmap, "Smaller
installs: a hybrid", 2026-10-29). Directions from the user, 2026-10-30:

- Build it soon, first for our own use: a contributor who can't run
  Steam on his dev machine signs in with his Steam account in a browser
  and gets what playtesting needs.
- Long term, **served assets are the main path** for players and
  playtesters: nobody needs a game installed; everyone signs in with
  Steam; the host serves each player the games his account owns.
  Loading from your own install stays as the fallback (offline, no
  service) and is used first when it matches.
- Two delivery modes, like CS:S's map downloads: **from the game server
  itself** or from **a download server the host names**; and before
  both, **the dev server** (section 5), our internal instance that
  contributors develop and test against.
- Stand-ins for games a player doesn't own are the simple ones we have.
- Whether to build our own intermediate format (IR) is weighed in
  section 3, with measurements; the recommendation is native files
  first, IR transforms later only where a measurement asks for them.

## 1. Goals, non-goals, legal model

Goals:

- A player or contributor with no install plays any round whose games
  his Steam account owns; the files arrive before the round (prefetch)
  and stay in a per-user cache.
- Loaders don't change: served files appear through Chimera's mount
  layer (`chimera_vfs`) exactly as an install's do.
- New games, maps and minigames reach players without full game
  downloads: only the files a round reads (about 6% of the CS:S install
  for de_dust2, section 2).
- Works on Linux and Windows; no extra infrastructure for the first
  mode.

Non-goals (for now): our own intermediate format (later, per
measurement, section 10); streaming while playing; real stand-in art;
games not on Steam in served mode (fallback only); public instances;
any GitHub Actions (none without the user's go-ahead).

Legal model, as decided:

- The project never runs a public service and never distributes
  assets: no assets in git, in builds or in releases. A host who owns
  the games serves files derived from his own install to players whose
  own Steam accounts own the same games, checked per game (app id)
  through the Steam Web API.
- **The host chooses the source; never the entitlement.** Ownership is
  always the player's own account. A player who doesn't own a game gets
  stand-ins (4.8), never its files.
- **Keys never leave the host.** The host's packer reads its own
  `chimera.local.toml` (Combat Arms' rez keys) and stores each inner
  file decrypted. No key, no encrypted original, no API or log line
  carries key material; clients never decrypt.
- What a host stores: per game build, the files of its install as the
  game's search path shows them (decrypted, compressed), content
  addressed, plus signed manifests. Nothing else.
- We run one internal instance, the dev server (section 5), for our
  own development and testing, open only to listed contributors who
  own the games.

## 2. What an install provides, measured

Method (2026-10-30, this box, load average about 30 from other agents'
builds, so times are upper bounds): the existing heavy tests run with an
`LD_PRELOAD` shim logging opens, seeks and reads of install files (one
seek per archive entry read); a throughput bench of the real cipher
code (`lithtech_formats::rez::RezCipher`, release, one thread) and zstd
over 200 MB of real archive contents. M1 replaces the shim with a vfs
access log (`CHIMERA_VFS_LOG`, 4.3).

| | CS:S de_dust2 | Combat Arms WAREHOUSE + JUNK_FLEA |
|---|---|---|
| test | `cs_source` `heavy::map_de_dust2::converts_to_a_plausible_map` | `combat_arms` `heavy::map_load::warehouse_loads_within_budget` |
| install | 4.6 GB (`hl2/` 2.7 GB, `cstrike/maps` 226 MB) | 9.5 GB, 108 `.rez` archives |
| files read | ~4,000 VPK entries in 25 archives, 17 loose files, the BSP (16.5 MB, one file) | ~1,550 entries; archive files reopened per read (1,587 opens) |
| bytes read | 266 MB (6% of the install) | 337 MB (3.5%): 231 MB V2 (AES), 16 MB V1 (Twofish), 89 MB unkeyed |
| load time | 3.2 s for the test; 1.7-2.2 s a load (performance.md, 2026-10-21) | 6.2 s first map, 0.61 s second, 0.5 s spawn |

Throughput (MB/s, one thread): Twofish (V1) 160-170; AES-256 CBC (V2)
1,300-2,100; SHA-256 2,100-2,200; zstd level 3: decompress 530-640,
compress 120-220, ratio 0.70-0.73; zstd level 19: decompress ~290,
compress 3, ratio 0.61-0.67.

What this says:

- **Decryption is small.** WAREHOUSE + JUNK_FLEA decrypt 231 MB at
  1.3-2.1 GB/s (0.11-0.18 s) and 16 MB at 0.165 GB/s (0.1 s): 0.2-0.3 s
  of 6.8 s, about 4%. Served files skip it anyway (the host decrypts
  once); a decrypted-file cache for own-install mode would save those
  0.2-0.3 s per session, so it stays optional (section 7).
- **zstd vs decryption:** zstd decodes 3-4x faster than Twofish and ~3x
  slower than AES-NI; both are small next to parsing. zstd saves 27-39%
  of transfer.
- **Parsing and converting dominate, not reading:** Combat Arms' weapon
  data takes 2.0 s once per session, its world convert 0.45 s, bodies
  1.0 s, and the spawn 2.5 s on the main thread, mostly lightmap images
  converted to f16 (performance.md, "Combat Arms map loads");
  surf_sedona's 10 s are LZMA lumps (2.5-3.4 s) and packed files
  (2.9-4.6 s) (performance.md, "Lean map loads").
- **Low detail first is partly free:** VTF stores mip levels smallest
  first (the `vtf` 0.4.1 offset sums the levels after the wanted one), so
  the head of a `.vtf` holds the low mips; DTX stores them largest first
  (`lithtech_formats::dtx`), and LTB keeps LODs in sections, so those
  need a reorder to stream; a BSP is one file.

## 3. Native files, an IR, or both

**A. Native files.** Serve each file the game's search path shows,
individually, content addressed: VPK entries, decrypted `.rez` entries
(decrypted on the host), loose files, whole BSPs. Loaders unchanged;
round sets recorded by logging which files loads open. Exact fidelity,
little new code; works for any game the day its loader works. Limits:
a BSP is monolithic (no partial fetch), per-game parsing at every load,
DTX/LTB can't stream low detail first.

**B. A full IR.** Every asset kind converted once into Chimera's own
format (meshes in GPU layout, textures with mips as pieces, maps split by
region, data documents merged). One loader for all games, faster loads,
streaming of mips, LODs and map regions, dedup of converted data. Costs:
a schema and a converter for every asset kind of every game before that
game can be served; fidelity drift (two code paths to keep equal);
cache invalidation on each converter change; two load paths unless
own-install mode converts too; each new game waits for a full converter.

**C. Hybrid: A first, IR transforms where they pay.** The protocol
(content-addressed files, manifests with dependencies and versions)
carries native files now and can carry transformed files later, each
transform cached per (converter version, source hashes) and justified
by a measurement.

**Recommendation: C**, from the measurements above:

- A round reads 3.5-6% of an install as native files: native per-file
  serving already gives the "smaller packets" without any converter.
- Read and decrypt costs are a few percent of a load; the time is in
  parsing and converting particular things (CA's lightmap images,
  weapon data, Source's LZMA lumps). Those are a handful of targeted
  transforms, not a reason to convert every kind.
- The contributor gets working in days with A; B delays everything by
  converters for textures, models, sounds and maps of two games.
- Candidate transforms are listed in section 10 with the numbers that
  justify each.

## 4. Architecture

### 4.1 Records and manifests (the packer)

- **A record is one file** as the game's search path shows it
  (`Mount::entries`: shadowing already resolved, so a game's search
  order needs no rules of its own): its plain bytes, id = SHA-256 of
  those bytes (`sha2`, already a dependency). Same bytes, same id:
  shared `hl2/` content between CS:S, HL2:DM and TF2 is stored once.
- **On the host** records are stored zstd-compressed (level picked by
  measurement in M1: a high level suits, as packing runs offline and in
  parallel; the transfer sends what is stored).
- **A manifest per game build**: game id, Steam app id, Steam build id
  (the app manifest's, as the mount doctor reads it), packer version,
  entries `(path, id, size, stored size)`, and **sets** (4.3). It is a
  record too (its id is its hash) and signed by the host's Ed25519 key.
  Tens of thousands of entries per game: a few MB, compressed.
- **`chimera_pack`** (a `tools` binary): `chimera_pack <game> [--sets
  ...]` mounts the install (the CA mount with the host's keys), writes
  `<store>/records/ab/<id>.zst` and `<store>/manifests/<game>/<build>`,
  then records the sets. Incremental: an id already stored is skipped,
  so a game update stores only changed files.

### 4.2 The client cache and its mount

- **Where:** `<data_local>/chimera/files/` (`~/.local/share/chimera/
  files/`, `%LOCALAPPDATA%\chimera\files\`), beside `content/` and
  `dump/`, never in the repo; `records/ab/<id>` plain bytes (loaders read
  them directly), `manifests/`, `index.toml` (last use, pins, bytes).
- **Integrity:** every record's hash is checked as it arrives, against
  the manifest the *game server* sent (its id comes over the game
  connection; for the dev server, from the branch's `assets.lock`), so
  a hijacked download server can't inject files.
- **Size cap** (`cl_assetcache_gb`, 5): least recently used first; the
  current and next round's sets pinned; records no manifest references
  go first.
- **The mount:** a new `chimera_vfs::FileSource`, `CachedFiles`, built
  from a manifest: `entries`, `contains` and `files_in` answer from the
  manifest without data; `read` returns the cached record, else asks a
  `Fetcher` (the connection or HTTP), blocking with a timeout, and logs
  `asset miss <path>` so sets can be fixed. A served game's `Mount` is
  `[content dir, CachedFiles]`; `GameInfo` gets a generic
  `mount_served(manifest)` beside its install mount. Map loads, the
  `<game>://` Bevy asset sources (`chimera_present::assets`) and every
  game's own reads go through it unchanged.

### 4.3 What a round needs (sets and prefetch)

- **Recording:** a `Recording` wrapper (`chimera_vfs`) logs every path a
  mount reads; `CHIMERA_VFS_LOG=<file>` turns it on in any run.
- **Sets**, recorded by `chimera_pack` loading headless, client-style
  (textures on: a server's lean load reads less than a client): `base`
  (HUD, menus, fonts, common sounds), `map/<id>` (one map load),
  `weapon/<id>` (a weapon's view and world models, sounds, icons:
  `harness` spawns it, fires, reloads, switches), `players` (character
  models). Each set is a list of ids with its total size, stored in the
  manifest.
- **A round's need** = `base` ∪ `map/<map>` ∪ `weapon/<id>` for every
  weapon the ladder or loadout can hand out ∪ `players`. The match
  manifest already names the map, weapon sets and minigame
  (`chimera_net::net::manifest::MatchManifest`); the client resolves
  them against the game's manifest. For the next round, the server's
  `NextMatch` pick starts the prefetch during the current one.
- Misses in play are fetched on demand and logged; a test keeps them at
  zero for the shipped minigames' maps and ladders.

### 4.4 Sign-in and entitlements

- **Steam OpenID 2.0** on the host: the client opens (or prints) the
  host's `/login?code=<c>` URL; the browser goes to
  `steamcommunity.com/openid/login` and back to `/login/return`; the
  host verifies the assertion with Steam (`check_authentication`) and
  takes the SteamID64 from `claimed_id`. Steam needn't run on the
  player's machine; any browser works (a phone too).
- **Ownership:** `IPlayerService/GetOwnedGames/v1` with the host's own
  Web API key (in its local config, never printed), `steamid`,
  `appids_filter` = the games the host serves, `include_played_free_games=1`
  and `include_free_sub=1` (Combat Arms is free). App ids: CS:S 240,
  Combat Arms: the Classic 1263550, later HL2:DM 320, TF2 440. The API
  answers only when the profile's game details are public (Q4).
- **Tokens:** the host issues a refresh token (30 days, stored hashed
  on the host, revocable) the client keeps per host in
  `<data_local>/chimera/asset-tokens.toml` (mode 0600 on Linux, never in
  `chimera.local.toml`, never in the repo), and short access tokens (1 h)
  Ed25519-signed with: SteamID64, entitled app ids, host origin, expiry.
  Each refresh re-checks ownership at most daily. (The dev server's
  long-lived dev token is the exception, section 5.)
- **Per player on the server:** `PlayerEntitlements` (games owned, from
  the token) next to the mount doctor's report; the server sends each
  client only files of games in its entitlements.

### 4.5 Delivery mode 1: from the game server (first)

The existing map transfer (`chimera_net::net::maps`: `MapRequest`,
`MapChunk`s with a window of acknowledged bytes, checked by hash, written
to the content cache) generalized from one map file to any record:

- `AssetRequest { ids }` and `AssetChunk`s on their own reliable
  channel, below gameplay's priority; the server's upload is capped by
  `sv_asset_maxrate` across all clients and shared fairly; a client's
  `cl_downloadrate` caps its own.
- The server checks the asking client's entitlements before every
  request: a record is sent only if an entitled game's manifest
  references it (record ids are public facts, DESIGN "How code refers to
  assets", so knowing one is never enough).
- Sign-in uses a small HTTP listener on the host (TCP, the game port's
  number) for `/login`; a client sends its access token in a message
  right after `Welcome`.
- A client can also join just to download (`chimera_assets fetch
  <host> [sets]`): connects, signs in, fetches, leaves; no character.

### 4.6 Delivery mode 2: a download server

- The host sets `sv_asseturl`: a static HTTPS file server (nginx, Caddy
  or a CDN) holding the same store layout (`r/<id>.zst`,
  `m/<game>/<build>`); range requests, many parallel clients. The game
  server hands out the manifest id and access tokens only.
- The download server accepts only access tokens signed by the game
  server's (or the host's auth service's) key: a tiny gate (an nginx
  `auth_request` to a small checker, or CDN token auth with signed URLs
  per game prefix), since the files are publisher's assets.
- Records are immutable: `Cache-Control: public, max-age=31536000,
  immutable` behind the gate.
- Clients cache by id either way, so switching modes re-downloads
  nothing.

### 4.7 Match manifest, entitlements, map rotation

- `MatchManifest` gains `assets: Option<AssetOffer>`: per game, its
  build, its manifest id and the round's set names; where to fetch
  (`connection` or the `sv_asseturl` URL); the host's auth URL and public
  key.
- Each client's own view comes with it (in `Welcome`/`ChangeLevel`):
  per game, `install` (its own install matches the server's build:
  nothing downloaded), `served` (owned, fetched) or `stand_in` (not
  owned).
- **`installed_by_all` becomes `available_to_all`** (in
  `chimera_net::minigame_net`): a game counts for a player when his
  install matches *or* he is entitled to it on this host. A map's game
  must be available to all (a map can't be stood in for): the rotation
  and the playable set keep to those maps, as they do with installs
  today.
- The mount doctor reports `served:<build>` for a game read from the
  cache.

### 4.8 Stand-ins (simple, what exists)

For a game a player doesn't own, the client mounts nothing for it and
falls back to what exists already: characters as the shell's capsules
(`client::hud`); a gun as a box (Combat Arms' placeholder looks,
`games/combat_arms/src/ui/visuals.rs`, lifted into Chimera for any
game) or nothing; grenades and smoke as balls; effects as placeholders;
sounds silent; HUD icons as the weapon's name in text. Hits are the
server's, from the game's data, so stand-ins are visual only. The work
is the data path: each loader degrades to its stand-in when its game
isn't mounted, without errors, chosen by the weapon's class from the
data documents the server already sends (`server_docs`). Gameplay values
a client needs for prediction travel as our own data documents (facts),
never as the game's files; M3 checks which values today come from an
install at runtime (Combat Arms' weapon data) and moves them into the
documents the server sends. Better stand-ins (our own generic models
per gun class, generic hands, sounds) are a later, optional item.

## 5. The dev server (the internal instance)

What contributors use to develop and test without installs: the same
store, records, manifests and HTTP protocol as a download server (4.6),
with a dev profile. Normal hosts use a subset of it (per-match
manifests, short tokens, entitlements per player); the dev profile adds
what development needs:

1. **Whole games and named sets**, not just a match's: any record of a
   game the contributor owns, by whole-game manifest or by set
   (`map/de_dust2`, `heavy-tests`: every file the heavy tests read,
   recorded with `CHIMERA_VFS_LOG`, `mapsweep`, `refcmp`).
2. **Access** = an allowlist of contributors' SteamID64s *and* the
   ownership check per game. Steam sign-in once in any browser
   (`tools/assets login <url>`); the dev server then issues a
   long-lived, revocable dev token that goes into the contributor's
   gitignored `chimera.local.toml`:
   ```
   [asset_dev]
   url = "https://<dev server>"
   token = "..."        # revocable; re-checked against ownership daily
   ```
3. **Builds side by side:** several game builds and packer versions
   stored at once (records are shared, so a new build costs only its
   changed files). Each branch pins what its tests expect in a small
   committed lock file, `assets.lock` (per game: Steam build id and
   manifest id; build ids and hashes are facts, DESIGN "How code
   refers to assets"), so tests are reproducible and a game update is
   a reviewable one-line diff. `tools/assets lock` updates it.
4. **Maintainers publish, contributors read.** A maintainer runs
   `chimera_pack --publish` on a machine with the installs (and the CA
   keys: Combat Arms files are served decrypted); it uploads missing
   records and the signed manifest. Contributor tokens are read-only.
5. **No install needed:** with `[asset_dev]` set and no install, the
   vfs (`CachedFiles`) fetches missing files on demand into the cache;
   `tools/assets fetch <game> <set>` prefetches (`heavy-tests` before a
   test run). Tests, tools (`mapsweep`, `refcmp`'s offline parts, `dump
   --list`) and playtests then run without installs or keys. A test
   whose files are neither installed nor fetchable skips, as now.
6. **Access logs and revocation:** each request logged (SteamID, record
   count, bytes); `assets admin revoke <steamid>` drops his tokens; the
   allowlist is a file on the server.
7. **Hosting:** either the dev box behind nginx or Caddy, reached through
   Tailscale or a Cloudflare Tunnel (free; up when the box is), or a
   small VPS (2 vCPU, 4 GB RAM, 40-80 GB disk: a few EUR a month; Q2).
   Storage: CS:S's whole install compressed is ~3 GB, Combat Arms' ~6-7
   GB (zstd ratios of section 2), a later build adds only its changed
   files. Traffic: a contributor's first full fetch is a few GB, then
   only changes. Needed either way: a Steam Web API key (the user's
   account, on the server only), the server's signing key (generated on
   first run), an HTTPS name (`*.ts.net` or a domain: Q3).

A contributor, start to finish (no Steam on his machine):

1. `tools/assets login https://<dev server>`: opens or prints the
   sign-in URL; he signs in with Steam in any browser; the dev token
   lands in his `chimera.local.toml`.
2. `tools/assets fetch cs_source heavy-tests` (or nothing: files arrive
   on demand).
3. `cargo run --features dev`, `cargo test ...`, `mapsweep`: as with an
   install. Playtests on any server work the same (mode 1 or 2; his
   cache already holds most files).

CI: none; no Actions until the user asks. Service tests use our own
fixture files and a fake Steam.

## 6. Configuration

Host (cvars on the game server; secrets in its `chimera.local.toml`):

| Name | Default | Meaning |
|---|---|---|
| `sv_assets` | 1 | Serve files of the games this server has to entitled clients (needs a store or install, and a Steam Web API key) |
| `sv_asseturl` | "" | Empty: the server sends files itself (mode 1). A URL: clients fetch from that download server (mode 2) |
| `sv_asset_maxrate` | 2048 | Upload cap for files over the connection, KB/s, all clients together |
| `sv_assetstore` | "" | The store folder (`chimera_pack`'s output); empty: pack on demand from the server's own install |
| `[asset_host] steam_web_api_key`, `signing_key_file` | none | In the host's `chimera.local.toml`; never echoed |

The existing `sv_allowdownload`, `sv_downloadurl` and `net_maxfilesize`
keep their CS:S meaning (map files) until Q8 is settled.

Client:

| Name | Default | Meaning |
|---|---|---|
| `cl_allowdownload` | 1 | 0: only what you have; joining says exactly what is missing |
| `cl_useinstall` | 1 | Use your own install first when its build matches the server's: nothing downloaded for that game |
| `cl_assetcache_gb` | 5 | Cache cap; least recently used first |
| `cl_downloadrate` | 0 | Download cap, KB/s; 0: none |
| `cl_downloadfilter` | all | Existing; `none` also refuses served files |

Safety either way: every file's hash is checked against the manifest id
the game server sent; ownership is always the player's own account.

## 7. Combat Arms' encryption

- Today `RezArchive::read` decrypts a keyed file on every read (CBC over
  the whole file) and reopens the archive file each time.
- **Served mode:** the host decrypts once when packing; clients get the
  plain inner files (`.ltb`, `.dtx`, `.wav`, `.dat`), never keys, never
  decrypt.
- **Own-install mode:** a decrypted-file cache (per user, outside the
  repo, never in git or builds) would save about 0.2-0.3 s per session
  of a 6-7 s first load (section 2). Optional; not planned until a
  measurement asks for it. Keeping the archive file open between reads
  is a cheaper first step.
- Transfer compression is zstd either way.

## 8. Security

- **Tokens:** signed (Ed25519), short (1 h access, 30 d refresh,
  revocable), bound to the host's origin; a client never sends a token
  to another host, and asks the player before signing in to a host a
  server names.
- **No key material** anywhere outside the host's packer: not in
  records, manifests, tokens, responses, logs or errors (the existing
  `KeyConfig` rule).
- **Hashes:** SHA-256 per record checked on arrival; manifests signed
  and their ids sent over the game connection; sizes checked before
  reading; parsers stay bounds-checked (`Le`).
- **Rate limits:** sign-in per IP; bytes per SteamID per day; one token
  in use from many places flagged and revoked.
- **What a malicious client can do:** copy what it downloaded (as an
  owner can copy his install; the gate keeps non-owners out, not owners);
  share a token (expiry, rate limits, revocation); enumerate ids
  (useless: entitlement per game); flood requests (`sv_asset_maxrate`,
  per-client fairness).
- **What a malicious host can do:** send bad files (hash check against
  its own manifest only protects against third parties: a hostile host
  can serve hostile data, which bounds-checked parsers must survive;
  fuzz the loaders' entry points in M2); make a client sign in (asked
  first; tokens per host).
- **The game connection is not encrypted** (netcode "unsecure" today):
  mode 1 sends publisher files in the clear. Fine on our Tailscale
  network; before public hosts, issue netcode secure connect tokens
  from the sign-in (Q7).

## 9. Milestones (smallest first)

Each lands with tests (docs/OBSERVABILITY.md section 1) and its docs in
the same commit.

- **M1 (done): per-file packer, local cache mount, set recording (no
  network).** `CHIMERA_VFS_LOG` and the `Recording` wrapper; records and
  manifests (`chimera_pack`), the `CachedFiles` source and
  `mount_served`; sets for `base`, maps, weapons, players,
  `heavy-tests`. Tests: a fixture store from our own files round-trips
  (hash checked, corrupt record refused, cap evicts); heavy (skip
  without installs): de_dust2 and WAREHOUSE load from a store holding
  only their sets with no install mounted, zero misses, the same
  `MapData` as from the install; the section 2 numbers re-measured with
  the vfs log. *Contributor:* nothing yet.
- **M1 results (done 2026-10-30).** Built: `chimera_assets` (chimera/assets:
  `Store`, `Manifest`, `CachedFiles`, `Recording`/`Recorder`, `pack`,
  `AssetsConfig`; its CLI `chimera_assets status | verify | gc
  [--dry-run]`), `chimera_core::mount::served` (each game's
  `mount::open` goes through it: the store under the install, or alone
  with `[assets] use_install = false` / `CHIMERA_ASSET_USE_INSTALL=0`, or
  alone when no install is found; `CHIMERA_VFS_LOG` wraps every layer),
  `[assets]` in chimera.local.toml (`store`, `use_install`, `builds`;
  `CHIMERA_ASSET_STORE` over it), the user-wide store
  (`~/.cache/chimera/assets`, `%LOCALAPPDATA%\chimera\assets`), and
  `chimera_pack pack <game> [--set <name> --from <log>]` (the root
  package, `--features tools`). Store layout: `records/<ab>/<id>.zst`
  (zstd level 3), `manifests/<game>/<build>.txt` (a sorted line format,
  sets inside; build from the install's Steam app manifest). Tests:
  `chimera_assets`' own (packing deterministic across thread counts and
  deduplicated, manifest round trip, `CachedFiles` reads and refuses a
  corrupt or wrong record, the recorder's log, settings), and heavy
  `heavy::served` in cs_source and combat_arms (record a load, pack it
  into a scratch store, load again from the store alone: the same
  meshes and texels). Differences from the text above: sets so far are
  per map (`de_dust2`, `warehouse`, recorded from the heavy tests), not
  yet `base`/weapons/players/`heavy-tests`; no size cap (`gc` drops
  unreferenced records); a store layered under an install mounts only
  the install's own build (its Steam app manifest; none when that build
  isn't in the store, isn't known, or `[assets] builds` pins another:
  fixed 2026-10-30 after the merge, `mount::served`'s tests); the store
  standing in for a missing install holds only the sets packed so far,
  so heavy checks of a whole install (`map_load`'s TERRORDOME and view
  model, `pools`) skip what it lacks (`mount::served::alone`,
  `GameInfo::served_alone`); Combat
  Arms' `ATTRIBUTES.rez`, opened as its own archive, is read through the
  game mount when served (same paths) and recorded either way; its
  music (`ui::music`, loose files in `Game/BGM`) isn't served yet.
  Measured 2026-10-30 on this box (load average 15-20 from other
  agents, so times are upper bounds):

  | | CS:S de_dust2 | Combat Arms WAREHOUSE + JUNK_FLEA |
  |---|---|---|
  | set recorded from | `heavy::map_de_dust2` (66 tests) | `heavy::map_load::warehouse_loads_within_budget` |
  | files read (vfs log) | 2,530, 228.1 MB | 753, 298.5 MB (decrypted) |
  | records (dedup) | 2,510 | 746 |
  | stored | 142.7 MB (63%) | 96.4 MB (32%) |
  | `chimera_pack`, 4 threads | 1.24 s; again 0.13 s (nothing new) | 0.25-0.67 s, decryption included |
  | tests with the install hidden | all 66 pass, 2,530 files read, 0 misses | passes: same 281 meshes, 548 textures, 70 held models |
  | map load, install vs store | 1.7-2.3 s vs 1.9-4.1 s (`heavy::served`) | first load 3.6-8.8 s vs 3.6-4.5 s (`map_load`, 3 runs each) |
  | dedicated server `+map de_dust2`, install vs store | 1.72 s vs 2.32 s (lean load, the same 2 warnings, no miss) | |

  Reading from the store costs about what reading the install does (zstd
  instead of VPK seeks or decryption); the spread is the box's load.
  The hidden runs had no install, no Steam folder and an empty local
  config (no keys): `CHIMERA_CONFIG=<empty file> CHIMERA_STEAM_ROOT=<empty
  folder> CHIMERA_ASSET_USE_INSTALL=0`.
- **M2: the dev server (section 5).** The HTTP service (store, signed
  manifests, `/r/<id>`, batch fetch), Steam OpenID sign-in and the
  owned-games check, the allowlist, dev tokens, access log and revoke;
  `chimera_pack --publish`; the HTTPS fetcher behind `CachedFiles`
  (on demand) and `tools/assets login | fetch | lock`; `assets.lock`.
  CS:S first, Combat Arms right after (decrypted on the publishing
  machine). Tests: the service against a fake Steam (OpenID and Web API
  stubs on localhost): an allowlisted owner gets records, a non-owner
  or non-listed account 403, a revoked token 401, another game's record
  403, a tampered record refused by the client; a heavy test runs from
  fetched files with no install. *Contributor: develops, runs tests,
  tools and single-player playtests without installs, keys or Steam on
  his machine.* The user's priority.
- **M2 results (internal accounts, built 2026-10-30; not deployed).** As
  the decisions below say: internal accounts first, no Steam. Built, all
  in `chimera_assets` (no new crate; the server code is unused by the
  games and never in a release's asset paths):
  - **Accounts** (`accounts`): an `Authenticator` per account kind, asked
    in order; `Internal` reads `<server store>/server/accounts.toml`
    (token hashes only, re-read when it changes, so a revoke takes effect
    at the next request). `chimera_pack user add <name> [--games a,b]
    [--note <device>]` prints a new token once (`chimera1_<64 hex>`; one
    per device: run it again), `user revoke <name> [--token <n>]`, `user
    list`. Accounts may be limited to games (`--games`; default all):
    another game's manifest or a record only its manifests refer to is
    403. Steam (OpenID sign-in, the ownership check, the allowlist) is
    another `Authenticator`, to be added before the server is opened up.
  - **The service** (`server`, the `chimera_asset_server` binary): plain
    HTTP/1.1 on `127.0.0.1:8731` (tiny_http, blocking, 8 threads,
    TCP_NODELAY); `GET /key` (the public key, no token), and with
    `Authorization: Bearer <token>`: `/whoami`, `/games`, `/m/<game>/<build>`
    (signed: `X-Chimera-Signature`), `/r/<id>` (as stored, zstd), `POST
    /batch` (ids per line, framed answer). None, unknown or revoked token:
    401. Manifests are re-read when the folder changes (at most once a
    second), so a publish needs no restart; one without a valid `.sig` is
    signed when read. Each request is a line of
    `<server store>/server/access.log` (time, `kind:name#token`, method,
    path, status, records, bytes). The Ed25519 key is
    `<server store>/server/signing.key` (0600, made on first use).
  - **Publishing:** `chimera_pack publish <game> [--from <log> --set
    <name>]` packs the install into the server's store
    (`<data_local>/chimera/asset-server`, `~/.local/share/...`, or
    `--store`), copies the sets the user's store has for that build
    (`--sets-from`; only sets whose files were all published) and signs
    the manifest. Upload over HTTPS (another machine) isn't built.
  - **The client** (`remote`): `[assets] url`, `token`, `key` in
    chimera.local.toml or `CHIMERA_ASSET_URL`, `CHIMERA_ASSET_TOKEN`,
    `CHIMERA_ASSET_KEY` (the console name will be `chimera:asset_url`).
    `https://` only, plain `http://` only to localhost. With a service
    set, `AssetSettings::build` fetches a manifest the store lacks (the
    pinned build: `[assets] builds`, else `assets.lock`; else the
    service's newest), checks its signature (against the pinned key, else
    the key the server names over HTTPS) and the locked id, and keeps it;
    each `CachedFiles` fetches a missing record on demand (`Fetcher`),
    checks it against the manifest's id and size and only then stores it
    in the user-wide store. A service that fails is reported once per
    game and process and the game counts as not served (tests skip).
  - **The CLI:** `chimera_assets login [<url> <token>]` (checks the token,
    prints the `[assets]` lines with the key to pin), `fetch <game>
    [<set>...]` (batches of 256), `lock [<game>...]` (writes `assets.lock`:
    per game `build` and `manifest` id; none is committed yet: there is
    no deployed server to pin).
  - **Tests** (our own fixture bytes, the service on 127.0.0.1, a free
    port): `chimera_assets`' `service` tests: a valid token gets signed
    manifests, records and batches, logged; none, unknown or revoked
    (while running) is 401, a second device's token still works; an
    account limited to one game gets 403 for the other's manifest and
    records (a record both share is allowed); the client refuses a
    tampered record (not stored), a manifest signed by another key, a
    manifest not the locked one, and plain HTTP off this machine; a
    publish is served without a restart; accounts and the lock round
    trip. Heavy (skip without installs):
    `heavy::served::de_dust2_loads_from_the_asset_service_with_no_install`
    (cs_source) and `warehouse_loads_from_the_asset_service_with_no_install`
    (combat_arms): record a load, publish it into a scratch server store,
    serve it, load through `mount::served::open_with` with no install, no
    keys and an empty local store: the same meshes and texels, every
    record fetched on demand. Measured on this box (load average ~15):
    de_dust2 2,469 records on demand, load 2.4 s (61 s before
    TCP_NODELAY: each response's last segment waited for the client's
    delayed ACK); WAREHOUSE 342 records, 12.5 s (the process's first
    load). A CLI run on localhost (publish 2 files, `user add`, the
    server, `login`, `fetch`, `verify`, `lock`, a bad token) worked.
    `chimera_assets` cross-builds for `x86_64-pc-windows-gnu` (ring
    included).
  - **Differs from the text above:** section 5's `[asset_dev]` and
    `tools/assets` are `[assets]` and `chimera_assets`; `chimera_pack
    --publish` is `chimera_pack publish`; no Steam, allowlist or fake
    Steam yet (internal accounts instead); tokens don't expire (revoke).
  - **Deployment (to do with the user; nothing was started):** on the dev
    box, from the checkout:
    ```
    cargo run --release --features tools --bin chimera_pack -- publish cs_source
    cargo run --release --features tools --bin chimera_pack -- publish combat_arms
    cargo run --release --features tools --bin chimera_pack -- sets cs_source --store <server store> --sign
    cargo run --release --features tools --bin chimera_pack -- sets combat_arms --store <server store> --sign
    # base, map/<id>, heavy-tests from CHIMERA_VFS_LOG logs (AGENTS.md, "Game files from the asset store"):
    cargo run --release --features tools --bin chimera_pack -- publish cs_source --from <log> --set base
    cargo run --release --features tools --bin chimera_pack -- user add <tester> --note <device>
    cargo run --release -p chimera_assets --bin chimera_asset_server    # 127.0.0.1:8731; prints the key
    tailscale serve --bg --https=443 http://127.0.0.1:8731
    ```
    `publish` without `--from` packs the whole install (CS:S ~3 GB
    stored); with `--from <vfs log> --set <name>` only a set, e.g. the
    `heavy-tests` log. Keep the server running under a user service
    (systemd `--user` unit with `Restart=on-failure`) or a terminal; it
    binds localhost only, so only `tailscale serve` reaches it, over
    HTTPS on the tailnet name (`https://<box>.<tailnet>.ts.net`). Each
    tester: `[assets] url = "https://<box>.<tailnet>.ts.net"`, `token =
    "<printed>"`, then `chimera_assets login` (prints `key` to pin) and
    `chimera_assets fetch cs_source <set>` or nothing (on demand). Then
    `chimera_assets lock` on a branch and commit `assets.lock`. Back up
    `<server store>/server/` (key and accounts); revoke with `user revoke`.
  - **Left (after "M2 for developers" below):** Steam sign-in, the
    ownership check and the allowlist (an `Authenticator`, before the
    server opens up); publishing over HTTPS from another machine
    (publisher tokens); rate limits and token expiry; the cache cap
    (`cl_assetcache_gb`).
- **M2 for developers (built 2026-10-31; not deployed).** Goal: a
  developer with only `[assets] url` and `token` plays, playtests and runs
  the whole suite, heavy tier included. Guide:
  docs/assets-for-developers.md.
  - **Whole games:** `chimera_pack publish <game>` without `--from` packs
    every file the game mounts and marks the manifest `complete` (a new
    manifest line); served alone, a complete build stands in for the
    install (`mount::served::partial` is false, so whole-install checks
    run, fetching on demand). Combat Arms' loose music (`Game/BGM/`) is
    packed under `loose/bgm/` (`combat_arms::game::mount::loose_music`,
    set `music`); `ui::music` plays it through the mount when there is no
    install folder.
  - **Sets** (published into a scratch server store on this box, measured
    2026-10-31; stored = transferred):

    | | CS:S (build 25338318) | Combat Arms (26236938) |
    |---|---|---|
    | every file (CS:S: with the publisher's content cache, 89 community maps, 2.6 GB plain) | 48,483 files, 47,692 records, 4,394 MB (7,347 plain) | 32,266 files, 29,431 records, 3,806 MB (9,348 plain) |
    | `base` (the app's main menu, `CHIMERA_VFS_LOG`) | 51 files, 1.5 MB | 1,790 files, 9.7 MB |
    | `map/de_dust2`, `map/warehouse` (the app on the map, HUD and kit) | 2,531 files, 142.7 MB | 2,428 files, 92.9 MB |
    | `weapon/<id>` (`chimera_pack sets`) | 29 sets, 0.7-1.9 MB, 21 MB together | 2,363 sets, 0.4-20 MB, 850 MB together |
    | `players` | 68 files, 12.9 MB | none yet |
    | `music` | - | 64 files, 72.3 MB |
    | `heavy-tests` (the whole suite with no install, `CHIMERA_VFS_LOG`) | 15,793 files, 922 MB | 14,866 files, 2,563 MB |

    Publishing took 38 s (CS:S) and 32 s (Combat Arms) for every file, 4
    threads. `chimera_pack sets <game> [--sign]` records `weapon/<id>`
    (CS:S: view and world models with materials, the waves of its sound
    entries; Combat Arms: the view model with hands and parts) and
    `players` (CS:S's eight class models) by loading each piece through
    the game's loaders on a tapped mount. Not yet: Combat Arms' character
    models as `players`, its weapons' world models and sounds.
  - **Batched on demand:** a missing file brings its group in one `POST
    /batch` (`CachedFiles`): the missing records of the smallest set
    holding it, from it on (at most 1,024 records, ~32 MB stored), else
    its companions (same folder and name before the first dot); one group
    at a time per manifest. `CHIMERA_ASSET_BATCH=0` turns it off. First
    loads through the asset service with no install, `heavy::served`
    (`CHIMERA_ASSET_LATENCY_MS` delays each answer, as over a slower link;
    the box's load average 15-30, so CA's times, mostly CPU, are noisy):

    | | de_dust2 | WAREHOUSE |
    |---|---|---|
    | before (main, one request per record) | 3.25 s, 2,469 requests | 11.1 s, 358 requests |
    | localhost, per record / batched | 3.08 s / 2.61 s (2,469 requests / 5 batches) | 26.1 s / 16.8 s (358 / 3) |
    | 20 ms a request, per record / batched | 58.6 s / 4.1 s | 18.3 s / 10.0 s |

  - **Progress:** `chimera_assets fetch` shows a live line on a terminal
    (records and MB done of the total, speed, time left; a plain line
    every 5 s elsewhere) and a summary, fetching batches on 4 connections
    (`progress::Transfer`, `Remote::fill`); a run that fetches on demand
    prints `assets: downloading from ...` every 2 s while it fetches and
    `assets: fetched N files, X MB ... in T s` after (`progress::
    on_demand`), nothing otherwise. `chimera_assets fetch` takes the
    service's newer copy of a build's manifest; a kept manifest that isn't
    the one `assets.lock` pins is fetched again.
  - **Tests from the service:** tests ask
    `chimera_core::mount::served::usable(game, path, marker)` instead of
    `path.join(marker).is_dir()` (77 files), Combat Arms' key checks
    `combat_arms::game::mount::can_read` (served records need no key),
    stock-map checks the mount; checks of the install's own archives
    skip with a note when served. The whole suite (`cargo nextest run
    --workspace --features dev`) with no install, no Steam folder, no
    keys, an empty cache and only `[assets] url/token` against a scratch
    server on 127.0.0.1: see "Heavy tier with no install" below.
  - **Measuring and the server:** `chimera_asset_server --latency-ms`,
    `Service::with_latency`.
  - **Heavy tier with no install (2026-10-31):** `cargo nextest run
    --workspace --features dev` with `CHIMERA_CONFIG` holding only
    `[assets] url/token`, an empty `CHIMERA_STEAM_ROOT`, an empty data
    folder (no content cache), an empty asset cache, against
    `chimera_asset_server` on 127.0.0.1 serving the whole publishes above:
    4,662 tests, all pass, 1,072 s wall (the cache filled to 3.4 GB on
    demand). Of the 584 heavy tests 534 ran from the service (community
    maps included); 50 skip with a note: 36 `lithtech_formats` archive
    tests (they open `.rez` files and keys by file name, below the
    mount), 6 checks of Combat Arms' own archives (`BGM.rez`,
    `ATTRIBUTES.rez`, the XML and language archives' listings, the
    archive layers, keyless archives), 4 `heavy::served` (they pack from
    an install), 4 that want local demo or probe files. Outside the heavy
    tier 6 skip: the CS:S mount's own VPK and layer checks, `dump`, 3
    sample-demo tests. Before this work, the same run had 256 tests
    skipping (the install folder or keys asked for, community maps by
    file) and 9 failing (tests reading the install's files by path).
  - **Windows:** `chimera_assets` (all targets) and `chimera_core`,
    `cs_source`, `combat_arms`, `chimera_pack` check for
    `x86_64-pc-windows-gnu`. MSVC: nothing new; `ring` (rustls) needs the
    MSVC C compiler (the Build Tools' `cl.exe`, as for the rest), the
    TCP_NODELAY call goes through `windows-sys`' WinSock, and the live
    progress line uses `\r` (works in Windows Terminal and conhost).
  - **Left:** Combat Arms' `players` set (character models) and its
    weapons' world models and sounds in `weapon/<id>`;
    `lithtech_formats`' install tests read archives by path (they could
    take the served mount through a test helper above the format crate);
    a size cap for the cache; deployment (section "Deployment" above, plus
    `chimera_pack sets <game> --sign` and the set publishes after
    `publish <game>`); and what "Left" above lists.
- **M3: mode 1, files from the game server.** The game server's
  sign-in listener and short tokens, `PlayerEntitlements`,
  `AssetRequest`/`AssetChunk` with `sv_asset_maxrate`; the client
  fetches a round's sets at join; `cl_allowdownload`, `cl_useinstall`.
  Tests: a networked `harness` scenario with the fake Steam: an owner
  without an install joins and loads, a non-owner gets nothing of that
  game, a tampered chunk fails the hash, snapshots keep flowing while
  files transfer. *Contributor: joins playtests on any of our servers;
  files he has from the dev server aren't fetched again.*
- **M4: per-round prefetch, entitlements in the match, stand-ins.**
  `AssetOffer` per client, `available_to_all`, next-round prefetch from
  `NextMatch`, `served:<build>` in the mount doctor, the stand-in
  fallbacks (4.8) and gameplay values in documents. Tests: a Lucker
  Party gun game where one client owns only CS:S: CA weapons show as
  boxes and text, the rotation keeps to CS:S maps, no panics; a minigame
  change prefetches the next map before `ChangeLevel`.
- **M5: mode 2 for hosts.** `sv_asseturl`: the dev server's code with
  the host profile (per-match manifests, short tokens issued by the
  game server, entitlements per player), or plain static files behind
  a token gate (nginx `auth_request`, CDN token auth); range requests,
  cache headers. Tests: a local static server with the gate; switching
  from mode 1 to 2 downloads nothing again.

## 10. Later, each justified by a measurement

- **Transforms (cached IR pieces), only where the numbers say so:**
  CA lightmaps as GPU-ready images (spawn 2.5 s WAREHOUSE, 4.5 s
  TERRORDOME, mostly f16 conversion); CA weapon data parsed into one
  document (2.0 s per session); Source BSPs with LZMA lumps inflated
  (surf_sedona 2.5-3.4 s) and packed files split out (2.9-4.6 s); DTX
  and LTB reordered low detail first, for streaming; maps split by
  region, for streaming. A transform is a record like any other, keyed
  by (transform version, source ids), so the protocol doesn't change.
- Decrypted-file cache for own-install Combat Arms (section 7).
- Better stand-ins (generic gun per class, hands, sounds).
- Streaming while playing (VTF's low mips are already at the head of
  each file).

### Converted-map cache (design, 2026-10-30; not implemented)

Justified by the heavy tier's numbers (docs/performance.md, "The heavy
tier: a sixth slot, and what conversion costs"): CS:S map conversion is
999 s of the heavy tests' 3,153 s, and 830 s of that converts a map that
an earlier test of the same run already converted (each test is its own
process). Step 2 of P4b (architecture-review.md); task brief
[11](../../tasks/11-heavy-tier-time.md).

- **What is cached:** the `MapData` that
  `cs_source::bsp::load_level_bytes_with` returns, whole (geometry,
  lightmap atlas, collision, brushes and tree, visibility, props, decoded
  textures, the kit as far as `MapLoadOptions::kit` read it), serialized
  with postcard (already a workspace dependency) after serde derives on
  the `map` types. Not cached: what is rebuilt cheaply or can't be
  serialized: `light_field` (a closure; the cache keeps the lighting and
  occluders it closes over and rebuilds it), `load_times` (`&'static`
  names; a hit records one "cache" stage), the progress reporter. Put in
  the world (`map::change_map`) is unchanged: a hit replaces conversion
  only. Combat Arms' loader the same way later, if its numbers say so (its
  sweeps convert each map once, so less to gain).
- **Where:** per user, beside the asset store and never in a checkout:
  `<cache dir>/chimera/maps/` (`~/.cache/chimera/maps`,
  `%LOCALAPPDATA%\chimera\maps`; the asset store's own folder stays
  content records only), one file per key; `CHIMERA_MAP_CACHE` overrides
  the folder, `CHIMERA_MAP_CACHE=0` turns it off. Two checkouts and every
  agent slot share it. Size cap like the content cache (least recently
  used first; a cvar beside `cl_assetcache_gb`), since full loads with
  decoded textures are hundreds of MB each (to be measured).
- **Invalidation key** (a hash, the file's name): the map's name and
  `MapLoadOptions` (`hdr_level`, `textures`, `kit`); the BSP's SHA-256
  (`MapData::file_hash`, already computed); the files the conversion read
  besides it, recorded during the miss with the mount's recording
  (`chimera_assets::recording`): each archive's (VPK, loose file) path,
  size and modification time, or the store record ids when served, kept
  in the entry and checked on a hit (stat only, no hashing of archives);
  the converter's version: a hash `build.rs` takes of the converter's
  sources (`games/cs_source/src/game/`, `chimera/world/src/map/`, the
  format crates under `source/`) and the locked versions of its parsing
  crates (vbsp, vtf), plus a hand-bumped `CONVERTER_VERSION` for changes
  those miss. Any code change in the converter is a new key, so the tests
  still cover conversion after every converter change; changes elsewhere
  keep the cache warm. Unused keys age out by the cap.
- **Writing and cold start:** a miss converts as today, then writes the
  entry to a temporary file and renames it into place (atomic; a reader
  never sees half a file). A lock file per key makes processes that miss
  the same key at once wait for the first one's write instead of all
  converting (dust2's 66 tests start in waves of five). A cold run costs
  what it does now plus one write per distinct map and options (107 maps
  in the heavy tier); a corrupt or unreadable entry is a miss.
- **Who uses it:** every caller of `load_level_bytes_with` (the heavy
  tests of cs_source, party_kit, mashup_demo and the apps, through
  `games::load_map` and `harness::load_level`) without changes to the
  tests; the apps behind the same switch, off by default until measured
  there. One heavy test loads a map both ways and compares the cached
  `MapData` with a fresh conversion; the fast tier checks the key changes
  with each input.
- **Expected gain:** with a hit at a fifth to a quarter of a conversion
  (de_dust2 1.85 s converting; to be measured), ~620-700 s of the 830 s of
  repeated conversion, ~20% of the summed heavy time: heavy wall about
  940 s to 750-800 s at five slots, warm or cold within a run (a cold run
  converts each map once). Measure cold and warm before and after, per
  map load and heavy wall (brief 11's last boxes).

## 11. Open questions for the user

1. **Code in this public repo?** Mode 1 is engine code (`chimera_net`),
   and the packer is beside `dump`; recommended: all of it here, outside
   releases' asset paths, as tools and server features. Or should
   anything (the sign-in, the packer) live in a private repo?
2. **Hosting the dev server:** the dev box behind Caddy/nginx through
   Tailscale or a Cloudflare Tunnel (free, up when the box is), or a
   small VPS (a few EUR a month, always up, ~10 GB per game build pair)?
3. **Domain:** Tailscale's `*.ts.net` names have HTTPS and suffice for
   the dev server; a domain of our own only for a public-facing VPS.
   Steam OpenID's return URL on a tailnet name is to be verified in M2.
4. **Steam profile privacy:** a contributor must make his game details
   public at sign-in (private again after is fine until the daily
   re-check). Acceptable?
5. **First game:** CS:S (owned; what the gun games need), then Combat
   Arms right after in M2?
6. **Budget** for a VPS or CDN later (a contributor's first full fetch
   is a few GB; a player's first round ~250-350 MB, compressed ~70% of
   that).
10. **The allowlist:** who is on it at first (SteamID64s), and who may
   publish (maintainers)?
7. **Encrypting the game connection** (netcode secure tokens from the
   sign-in) before any host outside our tailnet uses mode 1?
8. **`sv_asseturl` or reuse `sv_downloadurl`** (which keeps CS:S's
   plain-HTTP map folder meaning today)?
9. **Combat Arms' app id:** 1263550 (the Classic, installed here): is
   that the one contributors own?

## Decisions (user, 2026-10-30)

- **Steam sign-in waits (user, 2026-10-31).** Don't build Steam sign-in,
  the ownership check or the allowlist until the user says so. First make
  sure developers can work from the service (internal accounts, M2's
  fetching, the sets they need); the server stays private until then.

- **Hosting.** The dev server runs on the dev box first, shared privately
  over Tailscale (no public ports), with Steam sign-in on top. Then it
  goes public behind a Cloudflare Tunnel (still no open ports), and later
  moves to a VPS with better upload.
- **Contributors make their Steam game details public** so ownership can
  be checked. Accepted.
- **Order:** CS:S first, then Combat Arms. This is only test and rollout
  order: the packer is generic, walking the files a game mounts through
  Chimera's vfs, so a new game needs its mount description, its Steam
  app id and a packer run, not a spec of its own.
- **Identity is the Steam ID** from Steam sign-in, never IP, MAC or
  passwords. A server-side allowlist file of Steam IDs (with display
  names) says who may download; ownership is checked per game on top.
  Each device gets its own revocable token.
- **Publishers:** a separate list of Steam IDs; for now only the user.
  Publishing is a command, `chimera_pack publish <game>`. On the dev box
  it writes straight into the server's store; with a VPS it uploads over
  HTTPS with a publisher token, and since files are content-addressed
  only new files upload. A signed manifest per game build ends a
  publish. No web UI or FTP; an admin page (access logs, revocation)
  may come later.
- **Internal phase without Steam (user, 2026-10-30).** Until the server is
  opened up, testers use internal accounts: the server admin runs
  `chimera_pack user add <name>`, which prints a revocable token for the
  tester's `chimera.local.toml` (`user revoke` takes it back). Neither
  Steam sign-in nor the ownership check runs for these accounts; the
  server stays private on Tailscale and ownership rests on the testers'
  word. Steam sign-in and the ownership check plug in at the same seam
  (an `Authenticator` per account kind) and must be on before any public
  exposure. So M2 starts with internal accounts; Steam sign-in moves to
  the milestone that opens the server.
- **Developers outside the game (2026-10-30).** Everything that loads
  assets (the game, `cargo test` and the heavy tests, mapsweep, refcmp,
  dedicated servers, agents' slots) goes through Chimera's vfs, and the
  service is one more mount there. Config: an `[assets]` section in
  `chimera.local.toml` (URL, token), overridable by `CHIMERA_ASSET_URL`
  and `CHIMERA_ASSET_TOKEN`. One user-wide content cache
  (`~/.cache/chimera/assets`, `%LOCALAPPDATA%\chimera\assets`) shared by
  every checkout, worktree and slot. Files are fetched on demand when a
  program opens one it lacks. A CLI, `chimera_assets`, does the rest:
  `login`, `fetch <game> <set>` (prefetch, e.g. `heavy-tests`), `status`,
  `verify`, `gc`, `lock` (the repo's `assets.lock`). Tests that need an
  install accept "install or asset service".
- **More decisions (user, 2026-10-30).** The packer, sign-in and server
  code live in this public repo, outside releases. The asset URL
  setting will be `chimera:asset_url` (see the backlog's console
  namespaces item), not an `sv_` name. Budget: only needed when moving to
  a VPS (open). Encrypted game connections (netcode's secure mode) before
  host-served downloads are used outside Tailscale.
- **Hosting size and encryption (user, 2026-10-30).** No VPS now; the
  design should suit a medium-to-large VPS later: big disk, good network,
  plenty of RAM for caching, modest CPU. The game connection stays
  unencrypted for now. Netcode's secure mode encrypts the whole
  connection or nothing, costs little CPU or bandwidth (ChaCha20-Poly1305,
  ~16-30 bytes a packet), but needs signed connect tokens for every join,
  including LAN and direct IP. Instead, only the sensitive parts are
  encrypted: access tokens and asset files go over HTTPS, to the host's
  small endpoint next to the game server (mode 1) or to the download
  server (mode 2). The match manifest names the endpoint and its
  certificate's fingerprint, so a self-signed certificate can't be
  impersonated. Full secure mode waits until something needs it (anti-
  cheat, public matchmaking).
