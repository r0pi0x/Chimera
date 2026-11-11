# Ultimate Gun Game playtest: the host's checklist

For the host of the internal playtest (the tester guide is
[playtest.md](playtest.md)). Placeholders only: `<host address>`, `<port>`,
`<asset url>`, `<token>` and `<tester>` are never written into the repository,
the release notes or chat channels the testers do not share. Give each
tester their address and token privately.

How testers without the games get them: the game server names the asset
service when a player joins (its URL and its certificate's fingerprint);
the client downloads what the match needs over HTTPS, pinning that
certificate, before it spawns. A tester with the games installed uses the
installs. Design and limits: [plans/active/asset-service.md](plans/active/asset-service.md),
"Playtest delivery"; the service in general:
[assets-for-developers.md](assets-for-developers.md).

## The day before

1. **The release.** Tag and release only on your say-so (the workflow
   builds on a `v*` tag or a manual run). It makes, per platform,
   `UltimateGunGame-<tag>-windows.zip` (the game, the server, `Play-UGG.bat`,
   `PLAYTEST.md`) and `LuckerParty-<tag>-windows.zip` (CS:S, which testers do
   not need). Testers and the server must run the same build: the server
   refuses another network version with a message saying which side is older.
   Run the server from the same release's zip, or from the same commit.
2. **Send each tester privately:** the release link, `<host address>:<port>`,
   their token, and `docs/playtest.md` if they are not unzipping it.
3. **Check the asset service has everything the match needs** ("Publishing
   the game files", below) and that it answers over HTTPS from outside your
   network (`curl -k https://<host address>:<asset port>/key` prints a hex
   key).
4. **Let testers fetch ahead on Friday.** A tester's first start downloads
   about 0.5 GB from your asset service (what the games read at startup,
   then the first match's files); ten testers doing that at 1pm on Saturday
   share your upload. With the asset service and the game server up the
   day before, ask each tester to run their first start then
   (`Play-UGG.bat +chimera:asset_token <token> +connect <host address>:<port>`),
   wait for the progress window and the first match to load, and quit.
   On Saturday they start from what they already have; only new maps and
   guns come. Tell them to start on Saturday **without** the token
   (`Play-UGG.bat`, then Join, or `Play-UGG.bat +connect <host address>:<port>`):
   entering it again forgets which service it is for, and the game then
   starts again once at the join to bind it anew.

## Publishing the game files (once, and after a game update)

On the machine with the installs and Combat Arms' keys, from a checkout
(`chimera_pack` is a host tool, not in releases), into the server's store
(Linux `~/.local/share/chimera/asset-server`, Windows
`%LOCALAPPDATA%\chimera\asset-server`):

```
cargo run --release --features tools --bin chimera_pack -- publish cs_source
cargo run --release --features tools --bin chimera_pack -- publish combat_arms
cargo run --release --features tools --bin chimera_pack -- publish hl2dm
cargo run --release --features tools --bin chimera_pack -- sets cs_source --sign
cargo run --release --features tools --bin chimera_pack -- sets combat_arms --sign
cargo run --release --features tools --bin chimera_pack -- sets hl2dm --sign
```

`sets <game> --only <prefix>` records only the sets whose names start
with it and leaves the others as they are (`sets combat_arms --only
character/ --sign`: the bodies' sets alone, in under a minute).

A whole publish serves every file, on demand. Sets make a match's first
download one batch instead of a request per file: `sets` records each gun's
`weapon/<id>` and `players`; `base` and each map's `map/<name>` come from a
run of the client with the vfs log on (with the installs; per pool map).
**Do this for every map of the pool**, not only the ones listed below: the
automatic gun game draws at random from all of them, and a map without its
set loads file group by file group on demand. In the rehearsal
(2026-11-11) 16 of the pool's 24 maps had no set (every CS:S map but
de_dust2 and cs_office); a match on one started 20 s after the vote
without the players still loading it, who joined it late (about 50 s on
a loaded machine). The pool is CS:S's 18 stock maps (cs_assault,
cs_compound, cs_havana, cs_italy, cs_militia, cs_office, de_aztec,
de_cbble, de_chateau, de_dust, de_dust2, de_inferno, de_nuke, de_piranesi,
de_port, de_prodigy, de_tides, de_train), Combat Arms' WAREHOUSE,
JUNK_FLEA, DARKHILLS, BLOODMONEY and HL2:DM's dm_lockdown, dm_overwatch;
a client's log names a map without a set when it prefetches
(`prefetched cs_source:map/<name>: ... not on the service`). For one map:

```
CHIMERA_VFS_LOG=/tmp/m.log cargo run --release --bin ultimate_gun_game -- --map cs_source:de_dust2 --offscreen 1280x720 --frames 400 --dump /tmp/x
cargo run --release --features tools --bin chimera_pack -- publish cs_source --from /tmp/m.log --set map/de_dust2 --sets-from /nonexistent
```

Every CS:S map of the pool (Linux; build both binaries once first):

```
for m in cs_assault cs_compound cs_havana cs_italy cs_militia de_aztec de_cbble de_chateau de_dust de_inferno de_nuke de_piranesi de_port de_prodigy de_tides de_train; do
  rm -f /tmp/m.log
  CHIMERA_VFS_LOG=/tmp/m.log target/release/ultimate_gun_game --map cs_source:$m --offscreen 1280x720 --frames 400 --dump /tmp/x
  target/release/chimera_pack publish cs_source --from /tmp/m.log --set map/$m --sets-from /nonexistent
done
```

`base` for a game: the same from a run on the other games' maps (what a
match reads of a game whose map isn't played). `--sets-from /nonexistent`
keeps `publish` from copying the sets of your own user cache over the
server's. On this box (2026-11-11) the server has, per game:

| Set | CS:S | Combat Arms | HL2:DM |
|---|---|---|---|
| `base` (fetched at the first start) | 62.6 MB | 250 MB: also the music (`Game/BGM`), the G23's sounds, the pool maps' loading images | 62 MB: also all 12 guns, the 19 player models, the weapon and headcrab-canister sounds every match plays |
| `players` (each join) | 12.9 MB | 66 MB, nearly all in `base`: the stock characters and footsteps | 18 MB |
| `map/<name>` | the 18 pool maps, 90-190 MB each | the 4 pool maps | dm_lockdown, dm_overwatch and six more |
| `weapon/<id>` | every gun | every gun: view model with hands and parts, and the held model (what other players hold) | every gun |
| `character/<record>` | - | each of the 689 body records with a file (a rung's body: its model, faces, head pieces and textures; beyond `base` and `players` mostly its vest and legs textures, 0.3 MB at the median) | - |

The weapon sets are named as the offer asks (`weapon/ak47`, not
`weapon/weapon_ak47`), and the offer names the ladder's drawn melee
weapons too. A set the service lacks is skipped: its files arrive on
demand while the map loads, slower (a request per group of files).

`base` and `players` beyond what `sets` records were made from what
no-install clients fetched outside their sets: run a client like a
tester (see "Rehearsal") with `CHIMERA_ASSET_FETCH_LOG=<file>` (a line
per on-demand fetch: game, the set its group came from or `-`, records,
bytes, path) and `CHIMERA_VFS_LOG`, list the files every session read,
write them with the set's current files as a `CHIMERA_VFS_LOG`-style list
(`<game>\t<path>\t<size>\t<id>`), and publish it as the whole set:
`chimera_pack publish <game> --from <list> --set base --sets-from
/nonexistent` (a set is replaced, not merged). `chimera_pack sets
combat_arms` records all of Combat Arms' 2,363 weapon sets again (each
a superset of the last); it leaves the other sets alone.

What still comes on demand in a match (2026-11-11): Combat Arms' rung
guns' fitted parts (drawn per rung: up to 9 files, a request each, up
to 1.1 MB). The rung bodies are named in the join's offer
(`character/<record>`; a ladder dealt at a later rotation isn't: each
of its bodies then comes as its set, one request): a seeded session on
warehouse (ladder seed 217377, no prefetch, three runs each) went from
32-39 on-demand requests and 9-19 MB (17-20 of them body textures) to
10-19 requests and 2.5-4.9 MB, and from 78-85 requests to the service
in all to 56-65, the bytes the same (about 410 MB). A body or part whose only set is another pool map's now takes
half a megabyte of that map's set, not 25-32 MB (a set's on-demand
batches start small and grow with each miss from it), and a file the
join's download is already fetching is waited for, not fetched again.
A seeded session (Combat Arms' warehouse, ladder seed 217377, then de_aztec;
prefetch off), before and after: 17 and 34 on-demand requests, 34.5 and
67.8 MB (one or two 30 MB groups of `map/darkhills`) -> 23-33 requests,
4.4-10.2 MB.

Publishing again at the same build reaches testers who fetched before:
each start asks the service whether it has a newer manifest than the
one kept (waiting at most 5 s) and fetches the startup sets' new files
behind the progress window; each join fetches the offered sets' new
files before the tester spawns. Nothing more for the tester to do.

## The asset service (each session)

```
cargo run --release -p chimera_assets --bin chimera_asset_server -- --tls 0.0.0.0:<asset port>
```

It serves this machine's server store (`--store <dir>` serves another;
then see "Starting it" for the fingerprint). Its plain listener takes
127.0.0.1:8731; if that port is taken (another asset server on the same
machine, e.g. a developer's), add `--listen 127.0.0.1:<other port>`.
It prints `HTTPS at ...` and the **certificate fingerprint**. The
certificate is its own (self-signed, made on first start in
`<server store>/server/tls/`; keep that folder: a new certificate means each
tester's token has to be entered again). Clients trust exactly that
certificate, by the fingerprint the game server hands out at join; the
address may change. Limits (defaults): 100 requests a second per account
(bursts of 1000), 20 GB a day per account, 30 bad tokens a minute per
address (then 10 minutes refused), 48 connections at once per address
(testers behind one home address share it; one game holds up to about 8
while it joins); `--rate`, `--burst`, `--daily-gb`, `--bad-tokens`,
`--connections` change them. A connection past the limit is closed
(`tls: refused a connection from <ip>: too many at once` on its console);
the game asks again for a file that failed that way. Each request
is a line of `<server store>/server/access.log` (account, path, status,
bytes, `ip=`). The plain listener stays on 127.0.0.1:8731 for this
machine's own tools; only the `--tls` port faces the internet.

## Building or updating the server

Easiest: the server from the release zip (`ultimate_gun_game_server` or
`.exe`). To build it yourself from a checkout at the release commit:

```
git pull
cargo build --release -p ultimate_gun_game_server
# Windows: target\release\ultimate_gun_game_server.exe
```

It has no window, render or audio. It needs the maps and guns of the games
to serve a match, from this machine's installs or its asset store
(`[assets]` in `chimera.local.toml` or `CHIMERA_ASSET_URL`/`CHIMERA_ASSET_TOKEN`;
see assets-for-developers.md). Missing pieces are told at start (`error:` stops
the server, `warning:` only spoils something). Do not paste the config file
anywhere; it holds keys.

## Starting it

```
ultimate_gun_game_server -port 27015 +maxplayers 12 +MaxPlayer 3 +chimera:asset_url https://<host address>:<asset port>
```

`+chimera:asset_url` is what joining clients are told (the address testers
reach, not localhost). On the same machine as the asset service the server
reads the certificate's fingerprint itself, from the default server store
(`<server store>/server/tls/`); on another machine, or with the asset
service on another store (`--store`), add
`+chimera:asset_fingerprint <fingerprint>` (the one it printed). The
server listens on every interface (`+ip <address>` for one). In a `.cfg` file or the
console, quote the URL (`chimera:asset_url "https://..."`: `//` starts a
comment there); on the command line it needs no quotes.

Options (`ultimate_gun_game_server --help`):

| Option | Meaning |
|---|---|
| `-port <n>` | UDP port (default 27015) |
| `+minigame <id>` | the minigame (default `ultimate_gun_game:auto`) |
| `+map <name>` | start map: `greybox`, `cs_source:<name>`, `combat_arms:<name>` (bare is CS:S). Without it, the first map of the automatic pool |
| `+maxplayers <n>` | players the server takes (the party server's default is 16) |
| `+MaxPlayer <n>` | bots kept in the match (Combat Arms' bots); 0 for humans only |
| `+hostname "<name>"` | server name |
| `+sv_password <pw>` | password (testers type it on the Join screen) |
| `+chimera:asset_url <url>` | the asset service joining clients fetch from |
| `+chimera:asset_fingerprint <hex>` | its certificate's fingerprint (default: the one on this machine) |
| `+chimera:asset_prefetch_max <KB/s>` | the most each client downloads later maps at in the background (default -1: the clients' own 2 MB/s; 0: none). See "Download speed and your upload" |
| `+<command> [args]` | any other console command or cvar |

Maps and matches:

- The automatic gun game rotates at random through the maps of every game
  the players have, never repeating the last 10 played; players vote on the
  end screen. A fresh ladder (seed printed in the log) is dealt at each
  rotation.
- Rotation and rules can be set at start with the host cvars `mp_map_rotation`,
  `mp_avoid_recent`, `mp_knife_finale`, `mp_melee_rungs`,
  `mp_spawn_protection`, `mp_demote_on_suicide` (the list is
  `apps/party_kit/data/host_settings.toml`).
- A match needs 2 players; bots count (`+MaxPlayer`). Late joiners are fine.

Console commands on the server's stdin (type in its window; run it inside
`tmux` or `screen`, or a PowerShell window you keep open):

`status`, `tick_stats` (the last ticks' cost and the server's memory),
`minigames`, `minigame_status`, `minigame_restart`,
`change_match <minigame> [map]`, `ugg_new [seed] [rungs]` (a new ladder now;
a seed replays one), `parts` (what each game lends), `changelevel <map>`,
`quit` (Ctrl+C is the same).

Keep it simple on the day: `+maxplayers 12 +MaxPlayer 3` and no `+map`.

## Ports to open

On the router and the host's firewall:

- **UDP `<port>`** (27015 unless `-port`), forwarded to the server's machine.
  The server can ask the router itself (UPnP): `+net_upnp 1`; the log says
  whether the router agreed, and `status` shows the outside address. If the
  ISP gives no public address (carrier-grade NAT), forwarding cannot work:
  use a VPN everyone joins, or a host with a public address.
- **TCP `<asset port>`** for the asset service: the port given to
  `chimera_asset_server --tls` (its own HTTPS; nothing in front). Testers
  need to reach it from outside your network. Linux: `sudo ufw allow
  <asset port>/tcp`; Windows: `New-NetFirewallRule -DisplayName "Chimera
  assets" -Direction Inbound -Protocol TCP -LocalPort <asset port> -Action
  Allow`. The plain port 8731 stays closed.
- Windows: allow `ultimate_gun_game_server.exe` through Defender Firewall
  (the first-run prompt) for the networks that matter.

Bandwidth: roughly 0.15 Mbit/s up per connected player for the game
itself; the asset service's downloads are what fill an upload ("Download
speed and your upload", below).

## Download speed and your upload

What a tester without the games downloads from the asset service:

- **First start**, before the game's window: what the games read at startup
  (about 360 MB: each game's `base` set), behind a small window that says
  "Downloading game files: X of Y MB" with the speed and time left. Errors
  (a refused token, the service unreachable, another certificate) show
  there with what to do; the game then starts by itself. Testers with the
  games installed download nothing of those games.
- **Each join**: what the match needs that isn't there yet (the first one
  about 50-100 MB), at full speed, on the loading panel.
- **In the background, while playing**: the pool's other maps (about
  170 MB), at a capped rate, held while the tester joins or changes map.

The background rate is each tester's `chimera:asset_prefetch_rate` (KB/s,
default 2000: 2 MB/s; 0 turns it off; saved in their config). You set the
most any tester may use with the game server's
`+chimera:asset_prefetch_max <KB/s>` (default -1: the testers' own rate;
0: no background download for anyone, maps then come at the join).
It reaches clients in the match manifest at each join and map change.

Your upload is shared by everyone downloading at once. Rough first-download
times if all testers start together (0.5 GB each), and a suggested cap
(about half your upload, split between the testers):

| Your upload | Testers | First downloads, all at once | `chimera:asset_prefetch_max` |
|---|---|---|---|
| 10 Mbit/s | 5 | ~35 min | 100 |
| 10 Mbit/s | 10+ | ~70 min: fetch on Friday | 0 (off) |
| 20 Mbit/s | 10 | ~35 min | 100 |
| 50 Mbit/s | 10 | ~15 min | 300 |
| 50 Mbit/s | 15 | ~20 min | 200 |
| 100 Mbit/s | 10 | ~7 min | 600 |
| 100 Mbit/s | 15 | ~10 min | 400 |
| 500 Mbit/s or more | 15 | ~2 min | -1 (the testers' 2 MB/s) |

In general: cap ≈ (your upload in KB/s ÷ 2) ÷ testers (1 Mbit/s is about
125 KB/s). Measure your upload (any speed test) the day before. Testers
who fetched on Friday ("The day before", 4) take nearly nothing from it on
the day.

## Accounts and tokens

On the asset server's machine, from a checkout built with `--features tools`
(`chimera_pack` is a host tool and is not in releases):

```
cargo run --release --features tools --bin chimera_pack -- user add <tester> --note <device>
```

It prints the token **once**: copy it into a private message to that tester.
One token per machine (run it again for a second device); account names are
letters, digits, `_`, `-`, `.`. `--games a,b` limits what a token may fetch;
the default is every game. `user list` shows the accounts. The accounts
file keeps only hashes, in `<server store>/server/accounts.toml`.

Revoke (takes effect on the next request, no restart):

```
chimera_pack user revoke <tester>               # all of a tester's tokens
chimera_pack user revoke <tester> --token <n>   # one
```

Revoking stops downloads; a player already in a match keeps playing it. To
remove one from the server use its console, or end the match.

Nothing else is needed per tester: the token is all they enter, once
(`chimera_pack user add` prints the line for them). The game
server checks nothing of it; the asset service checks it on every request.
The first server that offers its files binds the token to that service's
certificate on the tester's machine (`%LOCALAPPDATA%\chimera\asset-access.toml`);
from then on the token is only ever sent to that certificate, whatever
address the server names. To change the certificate (a lost `tls` folder),
every tester enters their token again (`+chimera:asset_token <token>`),
which clears the binding.

## Pre-flight smoke test (the evening before, and an hour before)

1. Server up, log shows the banner, the build version, a printed seed and a
   map loaded without `error:`. `status` shows 0 players and the bots.
2. From **another machine outside your network** (a phone hotspot is
   enough), on a fresh Windows user profile or a PC with none of the games:
   download the release zip, unzip, `Play-UGG.bat`, enter a test token,
   join `<host address>:<port>`, watch the first download finish, spawn,
   shoot, climb a rung, open Esc > Options, press F9 and find the report
   folder, quit. Token entry and the download: run
   `Play-UGG.bat +chimera:asset_token <token> +connect <host address>:<port>`
   once; the game binds the token, starts again by itself (one restart, the
   first time only), shows a small window "Downloading game files: X of Y
   MB" while it fetches what the games read at startup, opens the game,
   shows the same line on the loading panel for the match's files, then
   spawns. Later joins download only what is new.
3. `chimera_pack user revoke` that test token, join again, confirm the
   download is refused with a clear message.
4. Join with two clients and check a match starts with bots off
   (`+MaxPlayer 0`), then with bots on.
5. Check the release zips for the traces of a developer's machine (the
   workflow's scan step does it; if you built by hand, build from a neutral
   path such as `/var/tmp/<name>`, and `grep -a` the binaries).

## During the session

- Watch the server's console output; it is the log. Keep it:
  Linux `./ultimate_gun_game_server ... 2>&1 | tee -a ugg-server.log` (stdin
  stays on the terminal); Windows PowerShell
  `.\ultimate_gun_game_server.exe ... 2>&1 | Tee-Object -Append ugg-server.log`.
- Lines worth watching: joins and refusals (build or mount mismatch), the
  seed and map at each rotation, `error:`/`warning:`, panics.
- `status` lists players, ping and the outside address; `minigame_status`
  shows the match and rungs; `tick_stats` the tick's cost (CPU ms p50 and
  p99: under 15 ms, a tick's length, is fine; the rehearsal measured
  2.9 ms p50 and 4-7 ms p99 with 3 players and 7 bots) and the memory.
- The asset service logs each request to `<server store>/server/access.log`:
  watch for 401 (bad or revoked token), 403 and hash-refusal lines.
  A tester's join is a burst of `POST /batch` lines (the match's sets),
  then `/batch` lines for the pool's other maps in the background.
- Half-Life style logs: `log on` on the console writes `logs/L<MMDD><NNN>.log`
  in the data folder.
- Testers' bug reports arrive as zipped `bugreports` folders and
  `ugg-last.log` files; note the time and map of each so you can find the
  server's log lines.
- If the server wedges: `quit`, restart with the same command; players
  rejoin from the menu (their files are saved).
- To change the ladder mid-session: `ugg_new`. To restart the match:
  `minigame_restart`.

## After

Revoke all tokens (`user revoke <tester>` each, or stop the asset service),
stop the server, close the ports (UPnP mappings expire, a manual forward does
not), and collect the reports.

## Rehearsal

A dress rehearsal on one Linux machine (2026-11-11), everything on
127.0.0.1 at test ports: `chimera_asset_server --tls` on a scratch server
store (the real store's manifests and records, its own key, certificate
and accounts; `--latency-ms 150` for the later half), the
`ultimate_gun_game_server` with `+maxplayers 12 +MaxPlayer 7` and the
installs, and `ultimate_gun_game` clients started like testers: each with
its own empty home (`HOME`, `XDG_*`), `CHIMERA_CONFIG` an empty file,
`CHIMERA_STEAM_ROOT=` (no Steam), `CHIMERA_CFG_DIR`, `--offscreen 640x360
+fps_max 30 +cl_showerror 1`, `+chimera:asset_token <token> +connect
127.0.0.1:<port>`; the server's stdin a fifo (`echo tick_stats > fifo`
each minute). Eleven matches over nine maps in about 50 minutes, with a
server restart.

| Measured | |
|---|---|
| Server tick (3 players, 7 bots) | CPU p50 2.0-3.3 ms, p99 3.7-7.4 ms; 4.8-8.4M instructions p50; a map change one tick of 0.3-1.4 s |
| Server memory (RSS + swap) | 2.4 GB at start; 5.2 GB after 3 maps, 5.5 GB after 6 (about +100 MB a map; since fixed: about 2.3 GB after 28 maps headless, performance.md, "The dedicated server's memory") |
| Client memory (offscreen 640x360) | 3.5-5.5 GB resident each |
| First start to spawn, de_dust2 (sets published), 150 ms per answer | 36 s, 581 MB in 157 requests |
| First start to spawn, de_cbble, 150 ms per answer, before and after the sets above (later that day) | 26.6 s (108 on-demand requests, 181 MB) -> 12.6 s (28, 92 MB) |
| On demand over three maps (de_cbble, darkhills, de_aztec), before and after the sets | 144 requests, 252 MB -> 31, 39 MB |
| A map without a set, 150 ms per answer | 2 min 25 s to 2 min 41 s from loading to joined |
| A tester over a session (4-9 maps, prefetch on) | 820-990 MB |
| 429s | none; 44 connections refused per address (with the old limit of 16) |

Found and fixed: a token entered again on a later start left the game
with no service (nothing fetched); the TLS front's 16 connections per
address; startup and join downloads that lost files when the service
restarted; the waiting launcher processes' memory. Open items:
[backlog.md](backlog.md), "UGG playtest rehearsal".
