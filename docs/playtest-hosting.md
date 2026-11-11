# Hosting a playtest with served game files

How to host Ultimate Gun Game for testers who have no games installed:
the dedicated server tells each joining client where the asset service is
(its URL and its certificate's fingerprint), and the client downloads what
the match needs over HTTPS before it spawns. Testers with the games
installed keep using their installs. Design and limits:
[plans/active/asset-service.md](plans/active/asset-service.md), "Playtest
delivery"; the asset service in general: [assets-for-developers.md](assets-for-developers.md).

Placeholders: `<public-ip>` your internet address (or a DNS name for it),
`<game-port>` the game's UDP port (e.g. 27015), `<asset-port>` the asset
service's TCP port (e.g. 27031). Never commit or post them, nor tokens.

## Once: publish what the match needs

On the machine with the installs (and Combat Arms' keys in its local
config), into the server's store (`~/.local/share/chimera/asset-server`,
Windows `%LOCALAPPDATA%\chimera\asset-server`):

```
cargo run --release --features tools --bin chimera_pack -- publish cs_source
cargo run --release --features tools --bin chimera_pack -- publish combat_arms
cargo run --release --features tools --bin chimera_pack -- publish hl2dm
cargo run --release --features tools --bin chimera_pack -- sets cs_source --sign
cargo run --release --features tools --bin chimera_pack -- sets combat_arms --sign
cargo run --release --features tools --bin chimera_pack -- sets hl2dm --sign
```

A whole publish (`publish <game>` without `--from`) serves every file on
demand; sets make the first match fast (one batch instead of a request
per file). `sets` records `weapon/<id>` and `players`. The `base` and
`map/<name>` sets come from a run of the client with the vfs log on, one
per map the match may play (a client's load, HUD and kit included):

```
CHIMERA_VFS_LOG=/tmp/base.log cargo run --release --bin ultimate_gun_game -- --offscreen 1280x720 --frames 120 --dump /tmp/x
cargo run --release --features tools --bin chimera_pack -- publish cs_source --from /tmp/base.log --set base
CHIMERA_VFS_LOG=/tmp/m.log cargo run --release --bin ultimate_gun_game -- --map cs_source:de_dust2 --offscreen 1280x720 --frames 300 --dump /tmp/x
cargo run --release --features tools --bin chimera_pack -- publish cs_source --from /tmp/m.log --set map/de_dust2
```

(`publish <game> --from <log>` takes only that game's files from the log:
run it once per game the log holds.) A set the service lacks is skipped:
its files still arrive, on demand, while the map loads.

## Each session: start the asset service with HTTPS

```
cargo run --release -p chimera_assets --bin chimera_asset_server -- --tls 0.0.0.0:<asset-port>
```

It prints the store, the signing key, `HTTPS at ...` and the
**certificate fingerprint** (made on first start in
`<server store>/server/tls/`; keep that folder: a new certificate means
every tester's token must be bound again). Limits (defaults): 100
requests a second per account with bursts of 1000, 20 GB a day per
account, 30 bad tokens a minute per address (then 10 minutes refused);
`--rate`, `--burst`, `--daily-gb`, `--bad-tokens` change them. Each request
is a line in `<server store>/server/access.log` (account, path, status,
bytes, `ip=`). The plain listener stays on 127.0.0.1:8731 for this
machine's own tools.

## Each session: start the game server naming the service

```
cargo run --release --bin ultimate_gun_game_server -- -port <game-port> +maxplayers 12 +chimera:asset_url https://<public-ip>:<asset-port>
```

On the same machine as the asset service it reads the fingerprint itself;
on another, add `+chimera:asset_fingerprint <fingerprint>`.

## Open the ports

Forward on the router to this machine, and allow in its firewall:
UDP `<game-port>` (the game) and TCP `<asset-port>` (the asset service).
Nothing else: the plain listener (8731) stays local.

Linux (ufw): `sudo ufw allow <game-port>/udp` and `sudo ufw allow
<asset-port>/tcp`. Windows: `New-NetFirewallRule -DisplayName "Chimera game"
-Direction Inbound -Protocol UDP -LocalPort <game-port> -Action Allow`, and
the same with `-Protocol TCP -LocalPort <asset-port>`.

Check from outside (a phone off Wi-Fi, a friend):
`curl -k https://<public-ip>:<asset-port>/key` prints a hex key.

## Add a tester

```
cargo run --release --features tools --bin chimera_pack -- user add <tester> --note <device>
```

It prints the token once; send it privately. One token per tester (per
device); `user revoke <tester>` takes it back at the service's next
request; `user list` shows the accounts.

## What a tester does (Windows)

1. Unpack the release; no games needed.
2. Once, in PowerShell in that folder:
   ```
   .\ultimate_gun_game.exe +chimera:asset_token <their token> +connect <public-ip>:<game-port>
   ```
   The token is kept in `%LOCALAPPDATA%\chimera\asset-access.toml`. The
   first join binds it to your service (its certificate) and the game
   starts again once by itself, joining the same server; it then downloads
   the match's files ("Downloading game files: X of Y MB" on the loading
   screen) into `%LOCALAPPDATA%\chimera\assets` and spawns.
3. Later: `.\ultimate_gun_game.exe +connect <public-ip>:<game-port>`, or
   Join in the menu. Files already downloaded aren't fetched again; the
   pool's other maps download in the background while playing.

What they may see:

- "the asset service refused your token (revoked or mistyped)": send a
  new token; they enter it again with `+chimera:asset_token`.
- "can't reach the asset service at ...": the service isn't running, or
  `<asset-port>` isn't forwarded.
- "isn't the one your token is for": the service's certificate changed
  (its `tls` folder was lost): they enter the token again.
- "This match needs ... start the game once with +chimera:asset_token":
  they joined without a token.
