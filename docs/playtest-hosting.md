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
   The network version is `net43` since a broken thing's kill-feed line
   (`net::ObjectDestroyed`: Combat Arms' auto turret) and the turret's
   shots replicated (`CaTurretShots`); `net42` builds (playtest6's) and
   earlier are refused.
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
`%LOCALAPPDATA%\chimera\asset-server`; `<server store>` below; `sets`
without `--store` writes the user's cache instead):

```
cargo run --release --features tools --bin chimera_pack -- publish cs_source
cargo run --release --features tools --bin chimera_pack -- publish combat_arms
cargo run --release --features tools --bin chimera_pack -- publish hl2dm
cargo run --release --features tools --bin chimera_pack -- sets cs_source --store <server store> --sign
cargo run --release --features tools --bin chimera_pack -- sets combat_arms --store <server store> --sign
cargo run --release --features tools --bin chimera_pack -- sets hl2dm --store <server store> --sign
```

`sets <game> --only <prefix>` records only the sets whose names start
with it and leaves the others as they are (`sets combat_arms --only
character/ --sign`: the bodies' sets alone, in under a minute; `--only
parts/`: the guns' fitted parts alone, about a minute; `--only base`:
Combat Arms' effect and debris files merged into its `base`, seconds).

A whole publish serves every file, on demand. Sets make a match's first
download one batch instead of a request per file: `sets` records each gun's
`weapon/<id>` (Combat Arms': with the gun's own sounds), `players`, and
`map/<name>` for **every map of the install**, in the pool or not (the
files loading the map reads as another game's gameplay loads it, never
from the converted-map cache, less `base` and `players`; Combat Arms'
also the sounds its objects name, the ambient loops): the offer names the
loaded map's set, so a `changelevel` to any installed map arrives as one
download too. CS:S's 109 maps take about 4 minutes, Combat Arms' 105
about a minute, HL2:DM's 8 seconds (`--only map/` for the maps alone).
`map/<name>` and Combat Arms' `base` are merged into what the manifest
has; the other sets are replaced. Before (rehearsal, 2026-11-11) 16 of
the pool's 24 maps had no set; a match on one started 20 s after the
vote without the players still loading it, and a map without a set took
2 min 25 s to load group by group with 150 ms per answer.

The maps' sets made from a client's run with the vfs log on (the pool's,
before `sets` recorded maps) stay as they are, with what `sets` adds
merged in; it read everything those runs read beyond `base` and
`players` (0 files missed on CS:S's and HL2:DM's, only the run's own
guns and bodies on Combat Arms'). A set from a run, for one map:

```
CHIMERA_VFS_LOG=/tmp/m.log cargo run --release --bin ultimate_gun_game -- --map cs_source:de_dust2 --offscreen 1280x720 --frames 400 --dump /tmp/x
cargo run --release --features tools --bin chimera_pack -- publish cs_source --from /tmp/m.log --set map/de_dust2 --sets-from /nonexistent
```

`base` for a game: the same from a run on the other games' maps (what a
match reads of a game whose map isn't played). `--sets-from /nonexistent`
keeps `publish` from copying the sets of your own user cache over the
server's. With `sets` run as above (2026-11-12) the server has, per game:

| Set | CS:S | Combat Arms | HL2:DM |
|---|---|---|---|
| `base` (fetched at the first start) | 62.6 MB | 268 MB: also the music (`Game/BGM`), the G23's sounds, the pool maps' loading images, the guns' impact and shell sounds and models and every sound the effect tables name (`sets`: surfaces, event sounds, the heartbeat) | 62 MB: also all 12 guns, the 19 player models, the weapon and headcrab-canister sounds every match plays |
| `players` (each join) | 12.9 MB | 66 MB, nearly all in `base`: the stock characters and footsteps | 18 MB |
| `map/<name>` | all 109 maps (the 18 stock ones and the publisher's community maps), 0.3-196 MB each, median 18 MB, 4.7 GB together | all 105 maps, 0.9-174 MB each, median 25 MB, 3.2 GB together | its 8 maps, 91-154 MB each |
| `weapon/<id>` | every gun | every gun: view model with hands and parts, the held model (what other players hold), its own sounds (fire, silenced, dry, reload, select, misc) and what it throws or fires (sounds, model, skins, flare) | every gun |
| `character/<record>` | - | each of the 691 body records with a file (a rung's body: its model, faces, head pieces and textures; beyond `base` and `players` mostly its vest and legs textures, 0.3 MB at the median), with the first-person hand model and sleeves its rung look wears when they aren't the default character's (`rungs::body_hands`; 669 sets, 0.3 MB more at the median; the default hands are in `base`) | - |
| `parts/<id>` | - | each of the 1,265 guns with parts: the parts a rung may draw for it and its default ones (their view and held models and textures, beyond its `weapon/<id>`; 2 MB at the median, mostly shared: 326 files, 7.5 MB in all) | - |

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

Combat Arms' `base` also wants its guns' impact sounds and decals, shell
sounds and shell models (`guns_snd_impacts/`, `fx/i_mark/`,
`guns_snd_shells/`, `guns_sc/`: 328 files, +13 MB to the startup
download), which any map with Combat Arms guns reads and which were
otherwise in no set but `map/darkhills` (a miss there took up to 13.6 MB
of that map's set a session). From the server store's manifest (Linux;
the build as listed in `manifests/combat_arms/`), the current `base`
plus those folders:

```
M=~/.local/share/chimera/asset-server/manifests/combat_arms/26236938.txt
awk '$1=="file"{p=$0; sub(/^([^ ]+ ){4}/,"",p); id[p]=$2; sz[p]=$3; o[++n]=p}
  $1=="set" && $2=="base"{p=$0; sub(/^set base /,"",p); b[p]=1}
  END{for(i=1;i<=n;i++){p=o[i]; if(b[p] || p ~ /^(guns_snd_impacts|guns_snd_shells|guns_sc|fx\/i_mark)\//)
  printf "combat_arms\t%s\t%s\t%s\n", p, sz[p], id[p]}}' "$M" > /tmp/ca-base.txt
cargo run --release --features tools --bin chimera_pack -- publish combat_arms --from /tmp/ca-base.txt --set base --sets-from /nonexistent
```

The rung bodies are named in the join's offer
(`character/<record>`), and so are the rung guns' fitted parts
(`parts/<id>`, for each Combat Arms rung gun showing parts; before them 0-9
requests a session, a part each, up to 2.8 MB: a seeded session over
warehouse, de_aztec and de_dust2, ladder seeds 217377-217379, prefetch
off, went from 7 such requests, 2.8 MB, to none (two runs), the joins' downloads
growing by 3.3 MB, from 165 to 168.6 MB; the parts sets add 3.1 MB to
Combat Arms' manifest, 7.4 to 10.5 MB, fetched once per tester and
publish); at a rotation the level change's offer
names the coming ladder's drawn melee weapons, bodies and parts (its match
deals them only once the map is in: `ladder::draw` makes the same draw
ahead). After a rotation (`ugg_new 217377 13 cs_source:de_aztec` from
warehouse, prefetch off, two runs each) a client fetched 19-23 requests,
5.9-7.1 MB on demand, 14-16 of them a melee weapon's or body's set, and
since then 1-3 requests, 0.01-0.5 MB (fitted parts only), the level
change's download growing from 39.4 to 45.6-47.6 MB. A seeded session on
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

A join or level change that fetches holds every on-demand miss in its store until its
download has claimed what it fetches (`chimera_assets::pending`,
`starting`): the new ladder's held models and bodies, which a client
starts loading as the match arrives with the level change, came a group
each before the download had asked the service for its list. A seeded
session (warehouse, de_aztec, de_dust2 by `ugg_new` with ladder seeds
217377-217379, rung seed 4242, one no-install client, prefetch off) went
from 62 on-demand requests and 32.1 MB (39 of them, 17.4 MB, files of
the offered `weapon/`, `character/` and `players` sets) to 19 and 14.8
MB, the joins' downloads from 162.6 to 176.6 MB; with Combat Arms'
`base` widened as above, 50 requests and 17.6 MB before the hold and
9-13 and 0.7-1.1 MB with it (two runs; each a Combat Arms gun's own
sound, `guns_snd_*`, then in no set), the startup download from 355.5 to
368.7 MB. Since then the guns' sounds are in their `weapon/<id>` sets,
the effect tables' sounds and `ui_snd/heartbeat.wav` in Combat Arms'
`base` (61 files, 1.6 MB more), and every installed map has a
`map/<name>` set: the last measured session's 6 on-demand requests
(0.45 MB) are all in sets now. The manifests grew
by 3.24 MB in all: Combat Arms 10.55 to 12.76 MB, CS:S 9.41 to 10.41,
HL2:DM 5.545 to 5.573.

Measured again with those sets (2026-11-12; the same seeded session, about
3 minutes a match, then a `changelevel` to `cs_source:gg_simpsons_d`,
outside the pool): one on-demand request, during warehouse's match,
Combat Arms' `attributes/debris.txt` (911 bytes, in no set: read when a
prop first breaks); nothing else on demand, nothing while joining or
loading. After the first match started, the only other requests were
the level changes' downloads (41.5, 87.0 and 0.2 MB). Combat Arms' `base`
now has the debris table and its pieces' models, skins and sounds (30
files, 1 MB; `sets combat_arms --only base`); a run with that `base`:
nothing on demand, the startup download 371.3 MB instead of 370.3. The
guard below, which climbs every rung, then found three more, all sounds
any match may play: Combat Arms' infection call-out
(`chars_snd/q_male/infected_male.wav`) and CS:S's lead cues
(`sound/buttons/blip1.wav`, `button8.wav`). `sets` now also merges into
`base` Combat Arms' infection and grenade call-outs and both games' minigame
cue packs (data/cues.toml: a rung up or down, the lead, the last rung,
the match's end; 16 files). A third guard run found a thrown grenade's
sound in flight (`guns_pt/snd/grenade.wav`, a Combat Arms rung's
grenade on de_aztec): each Combat Arms `weapon/<id>` set now also holds
what the gun throws or fires (its projectile's, grenade's, device's or
bolt's sounds in flight and at rest, create sound, flare sprite, model
and skins: `special::object_files`; 80 sets grew, by 171 files;
`sets combat_arms --only weapon/`, about 10 minutes).

Since the second rehearsal (2026-11-13), `sets` also merges into Combat
Arms' `base` the attribute files its install data reads at the first
start (`attributes/clientbutes.txt` came on demand at each first start)
and the effect textures every map takes (`effects::library::add_looks`:
the install guns' own FX.TXT groups, 7 textures on demand at each first
join): 8 files, 0.06 MB (`sets combat_arms --store <server store> --sign
--only base`).

**The guard**: `cargo test -p ultimate_gun_game --features dev --test it
heavy::no_fetch_in_match -- --nocapture` plays such a session as a
client without installs against a service reading the published store
(this machine's server store, or `CHIMERA_GUARD_STORE=<store>`), every
character climbing every rung, and fails naming each file fetched on
demand during a match and how to cover it (about 3.5 minutes; it runs
with the heavy tests wherever the server store and the installs are, so
it fails there until the sets above are published). Run it after
publishing, and after a change to what a match loads.

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
- **The map pool goes by the player count.** At each map change (the
  rotation, the end screen's vote, `changelevel` with no map) the server
  counts the characters present, players and bots, and leaves out the
  maps too big for that many (over 10 s on average to meet someone); the
  rest are drawn by weight, best around 4-8 s to meet, the tiny arenas
  now and then. `map_pool [players]` on the console lists the pool at a
  count (each map's chance, or why it is out). To keep a map out, add it
  to its game's `games/<game>/data/map_pool.toml` with a reason; the
  numbers (target 10 s, the weights) are `chimera/sim/data/map_fit.toml`
  (both built in: a rebuild). Recently played maps are held back
  (`mp_avoid_recent`). Every map that can come up needs its `map/<name>`
  set published (below, "Publishing the game files"; `sets` records every
  installed map).
- **The pre-match lobby.** Each new match (the first after the server
  starts, each rotation and map change: a host's `changelevel <map>` is the
  running minigame's next match there, a new ladder as a rotation's)
  waits on its loaded map until every
  connected player has it, showing everyone each player's status
  (connecting, downloading with MB done of the total, loading the map,
  ready; bots are ready at once) and ping; then a countdown and everyone
  spawns together. Players who join while the match runs join late, as
  before. The server's log says when it waits, counts down and starts.
  Cvars (on the command line, `+mp_lobby_max_wait 120`, or the console):

  | Cvar | Default | Meaning |
  |---|---|---|
  | `mp_lobby` | 1 (the party servers; 0 elsewhere) | 0 starts each match at once, as before |
  | `mp_lobby_countdown` | 5 | the countdown once everyone is ready, s |
  | `mp_lobby_max_wait` | 90 | the longest wait, s, from the lobby's start or the first player's arrival (whichever is later), before the countdown starts anyway; the stragglers join late |
  | `mp_lobby_min_players` | 1 | players (not bots) connected before being ready counts; set it to the testers you expect for the first match, so it doesn't start for the first one in (the max wait still applies) |

  `lobby` on the console lists the players and what it waits for;
  `lobby_start` starts the countdown now (`lobby_start 0`: the match at
  once). With no player connected it waits without a limit. A tester still
  in the first start's download (the small window before the game opens)
  isn't connected yet and isn't listed.

Console commands on the server's stdin (type in its window; run it inside
`tmux` or `screen`, or a PowerShell window you keep open):

`status`, `lobby`, `lobby_start [s]` (the pre-match lobby, above),
`tick_stats` (the last ticks' cost and the server's memory),
`minigames`, `minigame_status`, `minigame_restart`,
`change_match <minigame> [map]`, `ugg_new [seed] [rungs]` (a new ladder now;
a seed replays one), `parts` (what each game lends), `changelevel <map>`
(the next match there, with its lobby; no map: the next match on a map
drawn for the players present), `map_pool [players]` (the pool at that
count), `quit` (Ctrl+C is the same).

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
- **In the background, while playing**: the pool's other maps, the
  likeliest for the players present first, at a capped rate, held while
  the tester joins or changes map. With the pool by player count that is
  about 100 maps (several GB over a long session); `+chimera:asset_prefetch_max 0`
  turns it off, and each map then arrives as one download at its change.

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
- **Files fetched mid-match.** A match's files should all come with the
  join's or level change's download. A line ending in ` on-demand` in the
  access log is a file a game read that the download didn't bring (sent
  by builds after v0.1.0-playtest5; older ones send the same requests
  without the mark): `grep on-demand <server store>/server/access.log`.
  The tester's game says which file and when, in its console and
  `ugg-last.log`: `assets: fetched on demand during the match on <map>:
  <game> <path> (set <set>, <n> files, <KB>); this match <n> fetches,
  <MB>`, each match's start (`assets: the match on <map> started (<n>
  fetches on demand, <MB>, while joining or loading it)`) and its end
  (`assets: the match on <map> ended: <n> fetches on demand during it`).
  A file named there belongs in a set: "Publishing the game files".
- Half-Life style logs: `log on` on the console writes `logs/L<MMDD><NNN>.log`
  in the data folder.
- Testers' bug reports arrive as zipped `bugreports` folders and
  `ugg-last.log` files; note the time and map of each so you can find the
  server's log lines.
- If the server wedges: `quit`, restart with the same command on the same
  port; players' games show "Connection lost - reconnecting... (attempt N)"
  and join it again on their own once it answers (asked every 3 s for 2
  minutes; a server killed rather than quit is noticed after the 30 s
  timeout). After that, or if they pressed Cancel, they rejoin from the
  menu (their files are saved). `cl_autoreconnect 0` turns it off.
- To change the ladder mid-session: `ugg_new`. To restart the match:
  `minigame_restart`.

### Surprises (host only; don't tell the testers)

Two console commands on the server shake a match up. They need the
client build of 2026-11-13 or later (older clients show the shells and
explosions wrong and don't hear the siren). Code:
`apps/party_kit/src/surprise.rs`.

- `surprise airstrike [count] [seconds]` (default 30 shells over 8 s): a
  3 s warning ("AIR STRIKE INCOMING! TAKE COVER!" in red, a red flash and
  an air-raid siren from Combat Arms' install), then Combat Arms' air
  strike shells (LGB and napalm, 2:1) fall from under the sky onto open
  ground: half near living players, half anywhere walkable. They look
  and sound like the real thing: the designators' missile with its trail
  and incoming whistle, the LGB's explosion (`Dirt_Burster`) and the
  napalm's fire (`Exp_Napalm_Shell`, burning for the sphere's 5 s). The
  LGB's blast is 110 in 1200 u (lethal within about 5 m), the napalm's
  burning sphere 80 HP/s for 5 s in 350 u. Deaths are the world's: no
  rung up or down for anyone (a death on the scoreboard).
- `surprise gravity [seconds] [scale]` (default 60 s at 0.2): moon gravity
  for every player's movement and the physics world (props, ragdolls),
  with a "Moon gravity!" banner, a countdown in the last 5 s and "Gravity
  is back!". Every client predicts with it (CS:S's and HL2:DM's
  `sv_gravity`, Combat Arms' `combat_arms:sv_gravity`, all replicated: no
  corrections) and its own ragdolls and props fall with it
  (`chimera:phys_gravity_scale`, replicated). A map change ends it.
- `surprise stop` ends both at once.

Both run on the server's console only (a client's console refuses them):
a dedicated server's, or the game's own console when you host from the
game (a listen server), whose player sees and hears everything a client
does: the banners and flash, the siren, the shells and their explosions
(builds before 2026-11-14 showed a listen host none of the warnings and
almost none of the explosions; a dedicated server's clients were fine).
The shells fall nose first, on every client: the server turns them.
A tester without the installs has everything they show before they fire
once Combat Arms' `base` holds the siren and the shells' model, sounds
and effect files (`special::devices::strike_files`):
`chimera_pack sets combat_arms --store <server store> --sign --only base`
(the guard `ultimate_gun_game heavy::no_fetch_in_match` fires both on
its last map and fails on any fetch during the match).

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
| Server memory (RSS + swap) | 2.4 GB at start; 5.2 GB after 3 maps, 5.5 GB after 6 (about +100 MB a map; since fixed: about 2.3 GB after 28 maps headless, 1.8 GB after 12 once its Combat Arms view models went, performance.md, "The dedicated server's memory") |
| Client memory (offscreen 640x360) | 3.5-5.5 GB resident each (since: 3.2-3.3 GB after each of three maps with the installs, performance.md, "A client's memory") |
| First start to spawn, de_dust2 (sets published), 150 ms per answer | 36 s, 581 MB in 157 requests |
| First start to spawn, de_cbble, 150 ms per answer, before and after the sets above (later that day) | 26.6 s (108 on-demand requests, 181 MB) -> 12.6 s (28, 92 MB) |
| On demand over three maps (de_cbble, darkhills, de_aztec), before and after the sets | 144 requests, 252 MB -> 31, 39 MB |
| A map without a set, 150 ms per answer | 2 min 25 s to 2 min 41 s from loading to joined |
| A tester over a session (4-9 maps, prefetch on) | 820-990 MB |
| 429s | none; 44 connections refused per address (with the old limit of 16) |

Found and fixed: a token entered again on a later start left the game
with no service (nothing fetched); the TLS front's 16 connections per
address; startup and join downloads that lost files when the service
restarted; the waiting launcher processes' memory; the command clock's
bursts after each map change (the client predicted while the new map
settled behind the loading screen, a frame running at most a tick, so its
clock fell behind and jumped every few frames: 16-26 jumps and 53-94
"nothing predicted" restarts per client per map change, on a quiet box
too; now it starts predicting once in the game: 0 jumps and 1-2 restarts,
also with 80 ms lag and 20 ms jitter). Open items:
[backlog.md](backlog.md), "UGG playtest rehearsal".

**Rehearsal 2026-11-12/13.** The released clients (v0.1.0-playtest5,
net41, built `--release` from the tag) against an
`ultimate_gun_game_server` built `--release` from main's last net41
commit, on one loaded Linux machine (load 20-50 from other builds, swap
in use): two clients without installs (one fresh, one that had fetched
before) and one with the installs, offscreen, a scratch asset service
(150 ms per answer for the second half), `+maxplayers 12 +MaxPlayer 3
+mp_lobby_min_players 3`; nine matches over six maps of the three games in
about 30 minutes, with a server `quit` and restart. Found:

- **A newer server's game data refuses older clients of the same
  network version.** Every playtest5 client was disconnected at the join
  with "The server's game data (combat_arms:weapons) doesn't load here:
  ... unknown field `hidden_pieces`": main had added `projectile_piece`
  and `hidden_pieces` to Combat Arms' `view_model` hook data, which the
  server sends and an older client parses strictly. Run testers and the
  server from the same commit (as "The day before" says). The rest of
  the rehearsal ran a server with those two keys left out. Since, a
  client says the format of each data it reads (`net::formats`, generated
  from the types), and a server of another build refuses it at connect,
  before any download, naming the data ("This game reads the server's
  game data in another format (combat_arms:view_model) ...").
- **No client can join an HL2:DM map in Ultimate Gun Game.** The server
  offers its maps by name and hash and has no file to send; the party
  client read only CS:S maps for the check, found none and asked the
  server, which refused: all three clients (the installed one too) were
  disconnected on `dm_lockdown` ("Could not download
  maps/dm_lockdown.bsp: The server has no file to send"), and the lobby
  waited its 90 s for nobody. The fix is the client's
  (`party_kit::client::map_file_bytes` reading HL2:DM's maps too, branch
  `rehearsal-fixes`); a playtest5 client can't play them, so with those
  clients keep HL2:DM's maps out of the rotation.
- **`changelevel` kept the running match**: the map changed, the ladder
  and scores went on, and there was no lobby (players spawned as their
  loads ended); the vote's rotation and `change_match
  ultimate_gun_game:auto <map>` both held the lobby. Since, a host's
  `changelevel <map>` in a party app starts the minigame's next match
  there as a rotation does (a new ladder, scores anew, the lobby;
  `minigame_net::host_changelevel`).
- **Lobby** (from its start to the match): 27-74 s at each map change
  with the three clients, everyone ready in all of them (22-69 s, the
  slow ones de_inferno's and de_aztec's loads on the loaded machine, 13-17
  s to read the map and up to 45 s more to spawn it at a few frames a
  second), 50 s for the first match with a fresh client (its startup
  download 373 MB in 3.7 s, its join 98 MB, locally). The countdown box
  draws over the loading panel when that is still up.
- **Restart**: `quit` took 16 s to exit, the new server 21 s to listen;
  every client showed "Connection lost - reconnecting... (attempt N)",
  rejoined at attempt 15, 2 s after the server listened, through the
  lobby (everyone ready 4.6 s after it began, the match 9.5 s).
- **Surprises** on playtest5 clients: the air strike's warning, shells and
  world kills (de_dust2, de_inferno) and moon gravity's banner
  (warehouse) showed on every client.
- **On demand** (the clients' `CHIMERA_ASSET_FETCH_LOG`; playtest5 sends
  no `on-demand` mark, so the access log has none): one request at a
  join (Combat Arms' `guns_snd_ar/m4a1/reload3.wav` with the rest of its
  group, 0.46 MB) and one at a match's end (CS:S's
  `sound/bot/yea_baby.wav`, 16 KB, in no set); the installed client made
  no request at all. The first was the base game's starting M4A1, which
  a joining player holds from its spawn until the minigame arms it, and
  which no offer named: the offer now names the app's starting weapons
  too (`minigame_net::match_weapons`; ultimate_gun_game
  `offered_weapons`: every weapon any character holds, bots' too, is
  offered). The second is CS:S's party match-end winner sound (a bot's
  cheer, `parts::MATCH_WON_SOUND`), now recorded into CS:S's `base` by
  `sets` (publish: `chimera_pack -- sets cs_source --store <server
  store> --sign --only base`). Requests: 1,258, all 200;
  a tester without installs 1.15-1.62 GB over the session (prefetch on).
- **Memory** (resident plus swap): the server 1.2-1.3 GB at start,
  1.8-2.4 GB during play over 5 maps, back to 1.8 GB between them
  (bounded; `tick_stats`' line shows the resident part only, 0.08-1.9 GB
  here as the machine swapped it out); clients 2.9-3.8 GB in a match,
  peaks of 5.7-7.1 GB while a map loaded, and a waiting launcher process
  of 0.1-0.6 GB each. Server ticks: CPU p50 1.8-3.7 ms, p99 2.6-8.6 ms.
- **Errors**: no panics; the server logs `renetcode` "no private key was
  found for this address" (10) when a client restarts for its token or
  disconnects; clients `sequence already received` (2 each).

**Rehearsal 2026-11-13 (the playtest6 build).** Main at 600f5e10 (net42),
built `--profile playtest`: the `ultimate_gun_game_server` on the
installs, `+maxplayers 12 +MaxPlayer 3 +mp_lobby_min_players 3`, the
asset service on a scratch store reading the server store's records and
manifests (the guard's way: the real store isn't written), three
offscreen clients without installs started like testers (fresh homes,
`+chimera:asset_token ... +connect`), one at 1280x720 dumping a frame
every 3 s; on one loaded Linux machine that swapped (1-2 GB of each
process in swap). Seven matches over six maps of the three games in 19
minutes: de_dust2 to its time limit and vote, the vote's de_inferno, a
host's `changelevel` to warehouse, dm_lockdown, cs_office and junk_flea,
the surprises (air strike on de_dust2, dm_lockdown and junk_flea,
gravity and `surprise stop`), a client stopped 40 s (dropped, then
rejoined by itself), and a server `quit` and restart.

| Measured | |
|---|---|
| First start to spawn (fresh client, token entered) | ready in the lobby 23 s after start (startup download 375.6 MB in 7.4 s, join 94 MB in 0.5 s, locally), the match 29 s |
| Lobby, from its start to the match | 10-25 s at each map change (everyone ready in 7-20 s), 40 s for the first |
| Client dropped (stopped 40 s) | the server timed it out after 30 s; resumed, it reconnected at attempt 1 and was in the game 7 s after the loss |
| Server `quit` and restart | quit in 8 s; the new server listened 11 s after start; every client rejoined (attempt 14) 2 s later, the match 8 s after that |
| On demand | none during any match; 8 Combat Arms files at each tester's first start and join, outside every set (see "Publishing the game files": `--only base` covers them now) |
| Server tick (3 players, 3 bots) | CPU p50 1.7-3.5 ms, p99 3.2-11 ms; a map change one tick of 2.7-3.8G instructions (0.3-0.6 s CPU) |
| Server memory (resident + swap) | 1.3 GB at start, 1.9-2.5 GB during play over six maps, 2.0 GB at the end |
| Client memory (resident + swap) | 2.7-3.1 GB in the first match, 3.4-3.5 GB after six maps, peaks of 5.5-5.7 GB while cs_office loaded; two waiting launcher processes (token restart, startup download) 0.2-0.35 GB together |
| A swap's worst frame (instructions, main world) | 220-380M, of which 60-240M catch-up ticks (15-18 a frame while the swapping box ran 3 frames a second); render world under 125M |
| A tester's download over the session (prefetch on) | 1.6 GB (all three games' `base`, six maps' joins, the pool's other maps in the background); 3.1 GB on disk (1.6 GB of it converted maps) |
| Errors | no panics; the server's `renetcode` "no private key was found" (9, the token restarts); clients only `symphonia` mp3 warnings |

Found: the 8 files above (fixed in `sets`; publish `base` again), and a
joined player's loading panel ("Retrieving game data..." at 5%) under the
lobby with its countdown box over it at every match start (fixed: the
panel gives way to the lobby once joined). Open items: backlog.md,
"UGG playtest rehearsal, 2026-11-13".
