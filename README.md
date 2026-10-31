# Chimera

> [!WARNING]
> ## 🤖⚠️ This entire repository is AI-generated ⚠️🤖
>
> **Every file here (code, shaders, tests, specs and docs) was written by AI
> coding agents (Claude). No code has been reviewed by a human.** Expect the
> mistakes and quirks that come with that, and review anything before
> relying on it.

Chimera is a game engine (Rust, [Bevy](https://bevyengine.org)) for
rebuilding existing first-person games from their own files, and then
mixing their parts: a Combat Arms character on a Counter-Strike: Source
map, one game's guns with another's movement. **Lucker Party**, a party
game of minigames built from those parts, is what it is all heading
towards.

No game files are in this repository or in any build. The maps, models,
textures and sounds load at run time from **your own installs** of the
games, the way Garry's Mod uses them.

## Counter-Strike: Source, the first game

The main app today is a remake of **Counter-Strike: Source** that aims to
play exactly like it. "Exactly" is measured, not guessed: values come from
a real CS:S dedicated server driven by a test plugin, from SourceTV demos,
and from written behaviour specs. Movement is compared with CS:S tick by
tick, and differences we know of are tracked in
[docs/plans/active/parity-gaps.md](docs/plans/active/parity-gaps.md).

What works:

- **Every stock map** and most community maps (surf, bhop, kz, minigame
  maps), with their lighting, water, 3D skyboxes, props, doors, ladders,
  breakables and map logic.
- **Movement** as CS:S's: walking, crouching, jumping, bunny-hop limits,
  ladders, swimming, fall damage, surf ramps.
- **All weapons and grenades**: damage, falloff, armour, penetration,
  recoil, spread, scopes, burst and silencer modes, the buy menu, the
  economy, dropping and picking up.
- **Rounds and objectives**: bomb defusal and hostage rescue, freeze time,
  round money, team balance.
- **Bots** that buy, play the objectives and talk on the radio.
- **The CS:S interface** drawn from the game's own files: main menu, HUD,
  scoreboard, buy and team menus, radar, spectating and the freeze cam,
  options.
- **Multiplayer**: listen servers, a dedicated server, LAN and internet
  play with prediction and lag compensation, voice chat, text chat, votes.
- **Playing CS:S demos** (`playdemo`) recorded by the real game.

What is still different is listed by area, with how much it matters, in
the parity-gaps page above. The largest open areas are sound distance
falloff, some rendering details (HDR, shadows) and things only the real
game's client can show, which need captures on Windows.

Next: Combat Arms as the second game, then Lucker Party itself.

## Installing

You need:

- **Counter-Strike: Source** from Steam (app 240), installed. The game
  finds it through Steam's library folders; if it can't, it asks for the
  folder on first start.
- **Rust** 1.95 or newer ([rustup.rs](https://rustup.rs)).
- **Linux or Windows** and a GPU with Vulkan, DirectX 12 or Metal. On
  Linux, Bevy needs a few system libraries (ALSA and udev development
  files; see Bevy's
  [Linux dependencies](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md)).
- Git.

Then:

```bash
git clone https://github.com/r0pi0x/Chimera
cd Chimera
cargo run --profile playtest
```

The first build takes a while (several minutes); later ones are quick.
`--profile playtest` is an optimized build that rebuilds fast; plain
`cargo run --release` works too. On Windows, `scripts\playtest.ps1` pulls
the latest version and runs it.

If the game picks the wrong install, open the console (`~`) and use
`chimera_install <folder>` (the folder containing `cstrike/` and `hl2/`).

## Playing

It starts at CS:S's main menu. **Create Server** starts a game on any map
with bots, as in CS:S; the controls, binds and console commands are CS:S's
defaults (WASD, `B` to buy, `M` for teams, `~` for the console, `Esc` for
the menu).

Some console commands:

| Command | Does |
|---|---|
| `chimera_quickstart` | de_dust2 with nine bots, straight away |
| `map <name>` | load a map (`map de_nuke`) |
| `bot_add`, `bot_kick`, `bot_quota <n>` | add, remove or set the number of bots |
| `connect <ip[:port]>` | join a server |
| `callvote` | start a vote (kick, change map, restart) |
| `playdemo <file>` | play a CS:S demo |
| `menu extras` | Chimera's own options (things CS:S doesn't have) |

Settings and binds are saved as CS:S's `config.cfg` in your user folder
(`~/.local/share/chimera/cfg/` on Linux, `%APPDATA%\chimera\cfg\` on
Windows; a `mashup` folder there from before the rename is used until a
`chimera` one exists), never in the game install.

### Hosting a server

A listen server is Create Server with more than one player allowed. For a
dedicated server, without graphics or sound:

```bash
cargo run --release -p cs_source_server -- +map de_dust2 +maxplayers 10
```

Players join with `connect <your ip>` (UDP port 27015, change it with
`+hostport`), or find it in Find Servers on the same network.

Combat Arms has one too: `cargo run --release -p combat_arms_server --
+map combat_arms:warehouse +ca_mode TDM +MaxPlayer 6`; players join with
`combat_arms +connect <your ip>`.

## The mashup demo

The first mashup: Counter-Strike: Source's de_dust2 played as Combat Arms.
The map (geometry, lighting, collision, spawn points, ambience) is CS:S's;
everything played on it is Combat Arms': its soldier and movement, the
M4A1, the knife and grenades with their own first-person models, its HUD,
its bots and Team Death Match (terrorist spawns are Alpha's,
counter-terrorist spawns Bravo's). It needs both games installed (Combat
Arms' keys in `chimera.local.toml`, see `chimera.local.example.toml`):

```bash
cargo run --release -p mashup_demo                     # de_dust2, three bots
cargo run --release -p mashup_demo -- --map cs_source:de_inferno +MaxPlayer 7
```

Combat Arms' own maps work too (`--map combat_arms:warehouse`).

## Lucker Party

Minigames made of both games' parts, each described by a data file in
`minigames/`. The first is Gun Game on de_dust2: every player for
themselves up a ladder of CS:S's and Combat Arms' guns, a rung a kill, a
knife death costing a rung, the last gun's kill winning. It needs both
games installed, like the mashup demo:

```bash
cargo run --release -p lucker_party                    # the gun game on de_dust2
cargo run --release -p lucker_party -- --map combat_arms:warehouse
```

Ultimate Gun Game: the ladder is built from every game's guns (seeded,
spread across the games), the map pool from every game's maps; each gun
makes you its game. `cargo run --release -p ultimate_gun_game` starts at
its main menu: Play hosts a game others can join (bots fill it), Join
connects to an address (its server: `-p ultimate_gun_game_server`;
`--map`, `+minigame` or `+connect` on the command line skip the menu;
`ugg_new <seed>` on the console deals a new ladder).

### Play the gun game with friends

Everyone who plays needs their **own** copy of what the game loads at run
time, because each player's game draws the maps, soldiers, guns and sounds
from their own install (nothing is sent over the network but the game's
state):

- **Rust** 1.95 or newer and Git, and a checkout of this repository (see
  Installing above).
- **Counter-Strike: Source** from Steam (app 240), installed.
- **Combat Arms** installed, its folder under `[games.combat_arms]` `path`
  and its two archive keys (`V1` and `V2`) under `[games.combat_arms.keys.V1]`
  and `.V2` in your `chimera.local.toml` (copy
  `chimera.local.example.toml`; the file is never committed). Without the
  keys some soldiers, guns and sounds don't load.

When something is missing the game says exactly what and how to get it
before it starts (and refuses to start without Counter-Strike: Source).
Host and joiners should run the same commit: `git pull` before playing (a
different network version is refused with a message saying which side is
older; the scripts pull first).

On Windows (PowerShell, no admin rights needed; each script pulls, builds and
runs an optimized build):

```powershell
.\scripts\lucker_party_host.ps1                  # you host and play; prints the join command
.\scripts\lucker_party_host.ps1 -Dedicated       # only serve (a PC that stays on)
.\scripts\lucker_party_host.ps1 -Players 6 -Bots 2 -Port 28930
.\scripts\lucker_party_join.ps1 192.168.1.20     # a friend joins (:28930 unless it says another port)
```

On Linux or macOS the same by hand: `cargo run --profile playtest --bin
lucker_party -- -port 28930 +maxplayers 4 +MaxPlayer 0 +net_upnp 1` hosts
(or `--bin lucker_party_server` for no window) and `cargo run --profile
playtest --bin lucker_party -- +connect <ip>:28930` joins.

The host script prints your LAN address and the exact command to give your
friends; the game port is **UDP 28930** (`-Port` changes it).

- **Same network (LAN, same Wi-Fi):** friends join with your LAN address.
  The first time, Windows asks whether the game may use the network: allow
  it for private networks (allow public ones too if you host from a cafe or
  similar). If friends can't connect, that prompt, or Windows Firewall
  blocking `lucker_party.exe`, is the usual reason.
- **Over the internet:** friends need your public address and the host's
  router to let UDP 28930 through. The host script asks the router to
  forward it by itself (UPnP, `net_upnp 1`; the game prints a `UPnP:` line
  saying whether it worked and the router's outside address, and `status`
  shows it again). If the router has UPnP off, or sits behind another NAT
  (an ISP box), forward UDP 28930 to the host's PC by hand in the router's
  settings, or use a VPN everyone joins (Tailscale, ZeroTier) and use the VPN
  addresses.
- **Bots:** `+MaxPlayer <n>` keeps `n` Combat Arms bots in the match (the
  host script's `-Bots`; 0 for friends only; a solo run keeps 3 without it,
  and a joining game never starts bots of its own).
- `+name <name>` is your name in the game (`-Name` in the scripts).
- The match: first to climb the whole ladder (a rung a kill, CS:S's guns and
  Combat Arms' in turn; a knife death, a suicide or a fall costs a rung)
  wins; a late joiner starts on the lowest rung of the others; it waits
  for at least two players.

## How it is built

```
apps/       the programs: apps/cs_source (the game), apps/cs_source_server,
            apps/combat_arms, apps/combat_arms_server, apps/mashup_demo
            and apps/lucker_party
games/      each game as a library of parts: games/cs_source,
            games/combat_arms; games/originals, Lucker Party's own
minigames/  Lucker Party's minigames as data
source/     Source engine file formats and map logic, shared by Source games
chimera/    the engine: simulation, physics, rendering, sound, network, UI shell
specs/      behaviour specs: how the original games work, in plain words
docs/       architecture, design, plans, how to test
```

Each layer uses only the ones below it. A game is a library of parts
(movement, weapons, rules, HUD), and an app picks parts: the CS:S app uses
all of CS:S's, and a mashup app can take parts from several games. Game
values such as weapons, prices and rules are data files, so most tuning
needs no code. Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the
code map and [docs/DESIGN.md](docs/DESIGN.md) for the design and its
reasons.

## Contributing and checking work

[AGENTS.md](AGENTS.md) is the contract for contributors and their coding
agents (rules, setup with `tools/doctor`, definition of done; task briefs
in [docs/tasks/](docs/tasks/README.md)); [CLAUDE.md](CLAUDE.md) has the
full command list;
[docs/OBSERVABILITY.md](docs/OBSERVABILITY.md) explains how to test,
take screenshots and compare against the real game. Tests:

```bash
cargo test --features dev -- --skip heavy::   # the fast tests
cargo nextest run --features dev              # everything
```

Tests that need a game install skip when it is missing.

## Assets and legal

- No game's files are committed or shipped; they load from your install.
- Behaviour from the original games reaches this repository only as
  written specs (values, formulas, rules) in [specs/](specs/README.md).
  No third-party code is committed or translated.
- Game names are trademarks of their owners. This project is not
  affiliated with or endorsed by Valve, Nexon, Valofe or any other
  publisher.

## License

The code in this repository is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution
you submit for inclusion is dual licensed as above, without any additional
terms or conditions.

This license covers this repository's own code and documents only. No game
files are included: maps, models, textures and sounds are loaded at run time
from your own installs of the games.
