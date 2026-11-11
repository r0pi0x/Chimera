# Ultimate Gun Game playtest: the host's checklist

For the host of the internal playtest (the tester guide is
[playtest.md](playtest.md)). Placeholders only: `<host address>`, `<port>`,
`<asset url>`, `<token>` and `<tester>` are never written into the repository,
the release notes or chat channels the testers do not share. Give each
tester their address and token privately.

Sections marked `TODO(asset flow)` belong to the join-and-download work
(joining tells the client where the asset service is, HTTPS with a pinned
certificate; the client downloads what the match needs). Fill them in when
that lands; the rest stands on its own. The asset service itself is
described in [assets-for-developers.md](assets-for-developers.md) and
[plans/active/asset-service.md](plans/active/asset-service.md).

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
3. **Check the asset service has everything the match needs** (below) and
   that it answers over HTTPS from outside your network.

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
ultimate_gun_game_server -port 27015 +maxplayers 12 +MaxPlayer 3
```

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

`status`, `minigames`, `minigame_status`, `minigame_restart`,
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
- **TCP `<asset port>`** for the asset service: TODO(asset flow) (the port
  the HTTPS front serves; the service itself listens on localhost only and a
  reverse proxy or tunnel puts HTTPS in front, see asset-service.md "Deployment").
  Testers need to reach it from outside your network.
- Windows: allow `ultimate_gun_game_server.exe` through Defender Firewall
  (the first-run prompt) for the networks that matter.

Bandwidth: roughly 0.15 Mbit/s up per connected player.

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

TODO(asset flow): where tokens are checked on join, anything beyond
`user add` the new flow needs (a join token, a certificate to pin), how to
rotate the pinned certificate, and the exact `<asset url>` testers use.

## Pre-flight smoke test (the evening before, and an hour before)

1. Server up, log shows the banner, the build version, a printed seed and a
   map loaded without `error:`. `status` shows 0 players and the bots.
2. From **another machine outside your network** (a phone hotspot is
   enough), on a fresh Windows user profile or a PC with none of the games:
   download the release zip, unzip, `Play-UGG.bat`, enter a test token,
   join `<host address>:<port>`, watch the first download finish, spawn,
   shoot, climb a rung, open Esc > Options, press F9 and find the report
   folder, quit. TODO(asset flow): the exact steps for token entry and the
   download.
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
  shows the match and rungs.
- The asset service logs each request to `<server store>/server/access.log`:
  watch for 401 (bad or revoked token), 403 and hash-refusal lines.
  TODO(asset flow): the join-time lines.
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
