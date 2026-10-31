# Architecture

A map of the code. Design rationale lives in [DESIGN.md](DESIGN.md); this
file says where things are and which way dependencies may point.

## Crates

A Cargo workspace (docs/plans/active/engine-split.md, "Crates and compile
time"); every member is a default member, so `cargo test` and `cargo
nextest run` at the root test them all (`checks/tests/it/architecture.rs`,
`workspace_crates`).

```
mashup (repository root: src/, tests/it/)   `dump`; `cs_source_app` at the old paths
  |    cs_source_server (apps/cs_source_server/)   the dedicated server: the app without `present`
  |   /
cs_source_app (apps/cs_source/)   the CS:S app: the game binary, SimPlugins (+ its version), games::load_map
  |    combat_arms_app (apps/combat_arms/)   the Combat Arms app: its game binary (beside, not above, CS:S)
  |    mashup_demo (apps/mashup_demo/)   the first mashup app: CS:S's maps played as Combat Arms (both games as libraries)
  |    party_kit (apps/party_kit/)   the party kit: both games composed, minigames, client app, server run loop (`present`: the client)
  |    lucker_party (apps/lucker_party/)   Lucker Party: a thin binary over `party_kit`
  |    lucker_party_server (apps/lucker_party_server/)   its dedicated server: `party_kit` without `present`
  |    ultimate_gun_game (apps/ultimate_gun_game/)   the automatic gun game from every game's parts, on `party_kit`
  |    ultimate_gun_game_server (apps/ultimate_gun_game_server/)   its server: `party_kit`'s run loop with its parts
  |    originals (games/originals/)   the original minigames: definitions (minigames/) and rules hooks, Chimera only
  |    combat_arms_server (apps/combat_arms_server/)   Combat Arms' dedicated server: the game without `ui`
  |    combat_arms (games/combat_arms/)   Combat Arms' skeleton on Chimera, like cs_source; `ui` its HUD
  |
  |    cs_source[ui] (games/cs_source/src/ui/)   CS:S's front end: menus, panels, HUD (feature `ui`)
  |   /         \
  chimera_client  |  (chimera/client/)   client: the shell (input, camera, console UI, HUD base, widgets, debug)
  |    \        |
  |    cs_source (games/cs_source/)   CS:S's simulation: loaders, parts, rules, bots, SimPlugins, harness::CsSim
  |    |    source_logic (source/logic/)   logic: Source entity I/O
  |    |   /
  |    chimera_net (chimera/net/)   net: transport, replication, prediction; harness::NetSim
  |    |
chimera_present (chimera/present/)   render, audio, MapPlugin, assets (installs as asset sources)
  |    chimera_sim (chimera/sim/)   weapon, movement, rule_module, mode, minigame, identity, spawning, announce, server_docs; SimBasePlugins; testkit (feature)
  |    |
chimera_world (chimera/world/)   map: map data, collision, sim-side spawning; music; greybox, harness::Sim
  |                \
chimera_core          source_formats (source/formats/)   formats as plain data: no Bevy
 (chimera/core/)       core, console, slots, character, metrics, mount, game_registry, gameplay, game_parts, announcer, cues, query_cache
                      chimera_vfs (chimera/vfs/)   the neutral VFS (`FileSource`, `Mount`) under both format crates and `mount`
                      chimera_assets (chimera/assets/)   the asset store over the VFS: records, manifests and sets, `CachedFiles`, the vfs log, the asset service (server, HTTPS client); `mount::served` layers it
                      lithtech_formats (lithtech/formats/)   LithTech's formats as plain data (`.rez`, attribute files, worlds, textures, models, animation, effect files)
```

- The loader layers: `source/` (Source's formats and map logic) and
  `lithtech/` (LithTech's, for Combat Arms), each beside the other under
  the games, by the same rules (checked: `workspace_crates`). Their
  format crates never depend on Bevy, avian, Chimera or a game; a
  game converts what they read into Chimera's types. `lithtech_formats`
  uses `chimera_vfs` (`FileSource`, `Mount`, `LooseDir`; no dependencies)
  only for the VFS, as `source_formats`' VPK does, so a `.rez` is a mount
  layer like a VPK; `dump` reads Combat Arms' archives with it. Keys are
  never in the repo (checked: `no_keys_tracked`): they come from
  `chimera.local.toml` at run time.
- `chimera_assets` (plain data over `chimera_vfs`, no Bevy) holds the
  asset store (docs/plans/active/asset-service.md): each game's
  `mount::open` goes through `chimera_core::mount::served`, which puts
  the store's `CachedFiles` under the install or instead of it
  (`[assets]`) and records reads for `CHIMERA_VFS_LOG`; `chimera_pack`
  (the root package, `--features tools`) fills the store from an
  install and publishes into the asset server's. The asset service lives
  here too: `server` (the dev server, `chimera_asset_server`, plain HTTP
  on localhost behind Tailscale's or a proxy's HTTPS), `accounts` (an
  `Authenticator` per account kind), `remote` (the HTTPS client a
  `CachedFiles` fetches missing records with) and `sign` (manifests
  signed with the server's Ed25519 key).

- `source_formats` turns bytes and text into Rust values (glam vectors,
  Source units); it never depends on Bevy, avian or `mashup` (checked).
  Conversion into engine types stays in the game (`cs_source`), which
  re-exports the readers at their old paths.
- An edit in `source_formats` rebuilds every crate (`mount` in
  `chimera_core` uses it); `cargo test -p source_formats` builds and runs
  only its own tests (seconds).
- `chimera_core` is the engine's stable base: the modules every layer
  uses and that rarely change (`core` but the build's `VERSION`,
  `console`, `slots`, `character`, `metrics`, and `mount`, which uses
  `source_formats`' VFS and KeyValues). It never depends on
  `mashup` or a game (checked); `mashup` re-exports its modules at their
  old paths (`mashup::core` adds `VERSION`, stamped in the app's crate
  so a commit doesn't rebuild the base). The layering table covers its
  modules too. `cargo test -p chimera_core --features dev` tests it alone.
- `chimera_world` is the simulation's map (`map`): the neutral map data,
  collision, spawning a map's simulated parts (the drawn ones through
  `map::MapDrawer`), the map's systems. It uses `chimera_core` and
  `source_formats` of ours and never what draws or plays the map, `mashup`
  or a game (checked). It registers the map's systems itself
  (`map::build`): alone as `map::MapSimPlugin` (the dedicated server's,
  nothing drawn or played), or with what draws and plays the map at
  fixed places among them (`map::MapLayers`, filled by `chimera_present`'s
  `MapPlugin`; the client's order). It also holds the greybox (`greybox`:
  the map built in code) and the headless harness (`harness::Sim`, on
  the simulation plugins its caller passes as the base), so every crate
  from here up tests on it at its own layer. `mashup::map` re-exports it, with
  `MapPlugin` in builds with the presentation.
  `cargo test -p chimera_world --features dev` tests it alone.
- `chimera_present` is the presentation beside the simulation: `render`,
  `audio` and `map_plugin` (`MapPlugin`: map's registration with audio's
  and render's systems at the places `map::MapLayers` keeps, in one
  order) and `assets` (each game's install as a Bevy asset source,
  `<game>://<path>` read through its mount; apps add
  `InstallSourcesPlugin` before `DefaultPlugins`). It uses
  `chimera_world` and `chimera_core`, never `mashup` or a game
  (checked); `mashup` re-exports `render` and `audio` at their old
  paths. An edit in it rebuilds it and `mashup`, not the map; an edit in
  `mashup` rebuilds neither.
  `cargo test -p chimera_present --features dev` tests it alone.
- `chimera_client` is the client shell (`client`, re-exported by
  `mashup::client` at its old paths): local input and binds, the local
  player's camera, the console UI, chat, the fallback HUD and HUD text,
  fonts, the VGUI-like widgets and the drawing of a game's VGUI layouts,
  spectating's state and camera, picture and sound settings, captures,
  the debug UI and overlays, the network glue and the front end's
  interface (`front_end`). Its plugins are a group (`ClientPlugins`) a
  game's front end puts its HUD into (`add_after`). It uses the engine's
  crates, the presentation and the network included, and never Source's
  logic, `mashup` or a game (checked): a game says what the shell needs as
  data (`game_registry`, `core`, `debug_ui::GameDebug`), the map logic as
  `map::logic_view`, the app as `client::AppHooks`.
  `cargo test -p chimera_client --features dev` tests it alone.
- `chimera_sim` is the simulation's frameworks: `weapon` (the weapon
  system), `movement` (the movement slot's placeholder and noclip) and
  `rule_module` (the rule-module interface, `mashup::rules::module`),
  `mode` (the mode runtime: modes as data and hooks on one match whose
  phases it steps, architecture-review.md D2; Combat Arms' modes and
  CS:S's rounds play on it), `minigame` (minigames as data files played
  on the mode runtime by Chimera's own flow, with the `chimera:ladder`
  rules; D2, L2), `identity` (a character's game identity following the
  weapon it holds: its movement in the predicted tick, its body and its
  player's HUD; a minigame's `identity = "weapon"`),
  `server_docs` (a game's derived data as named documents a network
  client whose own differ takes from the server: weapon sets rebuilt in
  place, multiplayer.md "Server data documents")
  and `spawning` (what every game's respawn does), and, behind the `testkit` feature, `testkit` (the test kit: a weapon set as data and the base on the greybox, docs/OBSERVABILITY.md). They name no game; games fill them. It uses `chimera_world` and
  `chimera_core`, never the presentation, `mashup` or a game (checked);
  `mashup` re-exports them at their old paths. `cargo test -p chimera_sim
  --features dev` tests it alone.
- `cs_source` is Counter-Strike: Source: `game` (its loaders, Source
  movement, weapons, effects, animation, `CsSourcePlugins`;
  `mashup::games::cs_source`), CS:S's rules, `rules` and `objectives`,
  its bots, `bot`, and its part of the network (`game::net`: its round,
  bomb and hostages replicated, its join flow and bots behind the
  network's hooks); with its feature `ui`, its front end (`ui`:
  `mashup::games::cs_source::ui`; the GameUI menus and dialogs, the join
  flow, the buy, team, radio and spectator menus, the scoreboard and its
  HUD, on the shell, `with_hud` putting the HUD among the shell's
  plugins). The simulation uses the engine crates (not `chimera_present`),
  Source's logic and `source_formats`, never `mashup` or another game
  (checked); only `ui` uses the presentation and the shell, as optional
  dependencies of the feature (checked: `workspace_crates`,
  `game_ui_only_with_its_feature`), and nothing else of the crate uses
  `ui`; its tests may use the presentation too (a dev-dependency:
  `TEST_ONLY`). `SimPlugins` is CS:S's simulation (Chimera's base,
  `chimera_sim::SimBasePlugins`, with its rules, objectives, bots,
  network part and Source's logic), which the app runs; `harness::CsSim`
  is that as the tests' and tools' base, with its test maps for network
  tests. `cargo test -p cs_source --features dev` tests the simulation
  (its scenario tests, heavy map tests among them), `--features dev,ui`
  the front end too.
- `source_logic` is Source's map logic (`logic`: entity I/O, logic
  classes, triggers, movers, breakables, props, fire, the bridge to the
  map's entities) with its part of the network (`logic::net`: movers,
  HUD messages). It uses `chimera_net`, `chimera_world` and `chimera_core`,
  never the presentation, `mashup` or a game (checked); `mashup::logic`
  re-exports it. `cargo test -p source_logic --features dev` tests it
  alone.
- `chimera_net` is network play (`net`): the transport, the join
  handshake, replication of what every game has, prediction and
  interpolation. A game and Source's logic register what they replicate
  of their own as parts (`net::NetAppExt::net_part`, applied in name
  order in `NetPlugin::finish`, after the engine's protocol) and fill
  its hooks (`net::parts`: `JoinRules`, `BotHooks`, `Holds`,
  `NetHooks`); `harness::NetSim` runs a server and clients over its
  in-memory link. It uses `chimera_sim`, `chimera_world` and `chimera_core`,
  never the presentation, Source's logic, `mashup` or a game (checked);
  `mashup::net` re-exports it with the parts' replicated types.
  `minigame_net` (its own module, above `net`) is the minigame runtime's
  part of the network (`MinigameNetPlugin`: the match, the ladder rung and
  the match's events replicated, a client starting the server's minigame);
  an app that plays minigames over the network adds it.
  `cargo test -p chimera_net --features dev` tests it alone.
- `cs_source_app` (apps/cs_source/) is the faithful CS:S app: the game
  binary (`cs_source`, the workspace's `default-run`) and the app's glue:
  `SimPlugins` (`cs_source::SimPlugins` with the build's version),
  `games::load_map`, `client_hooks` and `client` (the shell with this app's hooks
  and CS:S's HUD), re-exports at the old paths, and build.rs's stamp. It
  uses `chimera_present` only with its default feature `present` (with
  the client shell and CS:S's front end, `cs_source/ui`).
- `combat_arms` (games/combat_arms/) is Combat Arms, wave 0: a skeleton
  of placeholder parts on Chimera, each replaced by Combat Arms'
  behaviour as its spec lands (specs/combat_arms/; firing, inventory,
  grenades, the weapon classes, attachments, the game modes (TDM,
  Elimination Pro, S&D, CTF, One Man Army and the rest; Hired Guns with
  its AI mercenaries, VIP Escort, the clan modes), bots and the
  player characters, the death camera and radio and chat so far;
  docs/plans/active/combat-arms.md), one
  module per subsystem in `game` (`movement`, `weapons`, `inventory`,
  `grenades`, `attachments`, `data`, `rules`: its game modes as rule
  modules, `bots`: Bot Deathmatch's computer soldiers writing `Intent`,
  `characters`: model choice, third-person animation, the posed
  skeleton's hit spheres and sockets, death and bodies, `gear`: what the
  vest, helmet, goggles, backpack, gear mods and set items change and the
  loadout that brings them, `perks`: the perks and what each changes,
  `lobby`: the lobby service (rooms, teams, ready, start and countdown,
  join in progress), the client's checks and room list, the loadout's
  rules and item lifetimes, and its host on the engine (`ca_lobby`;
  `ui::lobby`: the room list, creation popup, waiting room and inventory
  screens), `net`:
  its part of the
  network on `chimera_net`, `world`: the install's maps as `MapData`),
  with its feature `ui` its
  front end (`ui::layout`: its layouts on Chimera's `client::canvas`,
  `ui::hud`, `ui::scoreboard`, `ui::menu`: the Esc menu, `ui::visuals`), and `harness::CaSim` as its
  scenario tests' base (with chimera_net's `NetSim` for its network
  tests). Layered as `cs_source`: the engine crates only,
  never `cs_source`, Source's logic, `mashup` or an app (checked); only
  `ui` uses the presentation and the shell. `apps/combat_arms`
  (`combat_arms_app`) is its game binary (`combat_arms`): the greybox
  or `--map combat_arms:<name>`, the shell with its HUD, the network (a
  listen server or a client, the shell's options), its build's stamp.
  What Combat Arms needs that lives
  in `cs_source` today moves into Chimera first (a lift), never across.
- `mashup_demo` (apps/mashup_demo/) is the first mashup app (binary
  `mashup_demo`): CS:S's maps (`cs_source::game::MAP_LOADER`) played as
  Combat Arms (its simulation, front end and bots). It composes the two
  games as libraries through Chimera's `map::MapSources` (each game's
  `MapLoader`, the gameplay's `MapDressing`, spawn teams mapped, the
  app's surface tables, the map game's kept models), never another app
  (checked: `workspace_crates`); of CS:S's simulation it adds Source's
  map logic (`source_logic`) and its guns alone (`CsGunsPlugin`), never
  its rules or front end. docs/plans/active/mashup-demo.md has what was
  generalised and what's left.
- `party_kit` (apps/party_kit/) is what the four party apps share, so none
  depends on another app (checked): the two-game composition
  (`PartyPlugins`, `PartyPartsPlugin`, `GAME_PARTS`, `MAP_SOURCES`), the
  requirements and mount report, the minigame a command line names, and
  with `present` the client app (`client_app`, `with_defaults_for`,
  controls); its `server` module is the dedicated server's run loop
  (`parse_for`, `run`, `ServerPlugin`). Its tests are the party's and the
  server's (tests/it/, tests/server/).
- `lucker_party` (apps/lucker_party/) is Lucker Party (binary
  `lucker_party`), a thin binary over `party_kit`: minigames (`chimera_sim::minigame`, definitions as data
  on the mode runtime) played with both games' parts, composed as the
  mashup demo composes them (Combat Arms' simulation, CS:S's maps, logic
  and guns) plus the minigame runtime and `originals`; it starts the
  minigame `+minigame <id>` names (the gun game without one) on its
  pool's first map. Its presentation, shell and Combat Arms' front end are
  behind `party_kit`'s default feature `present`. Never another app (checked).
- `lucker_party_server` (apps/lucker_party_server/) is Lucker Party's
  dedicated server (a thin binary): `party_kit` without
  `present`, the network and the minigame the command line names
  (`+minigame <id>`, its first map unless `+map`), so it links no render
  or audio code (checked the same way as the other servers).
- `ultimate_gun_game` (apps/ultimate_gun_game/) is the Ultimate Gun Game
  (binary `ultimate_gun_game`; docs/plans/active/ultimate-gun-game.md):
  `party_kit`'s client app and composition plus a gun game made
  at run time from each game's parts (`chimera_core::game_parts`,
  `chimera_sim::minigame::auto`). It names no game crate.
  `ultimate_gun_game_server` (apps/ultimate_gun_game_server/) is its
  dedicated server: `party_kit::server::run` with those parts,
  no presentation (checked).
- `originals` (games/originals/) is Lucker Party's original minigames:
  their definitions, our own TOML in the repository's `minigames/`
  (compiled in, parsed at run time), and the rules hooks Chimera's
  built-in modules don't cover. It uses Chimera only: a game's parts
  reach it by id through Chimera's registries (checked; review A5).
- `cs_source_server` (apps/cs_source_server/) is the dedicated server
  (`cs_source_server`): `cs_source_app` without `present`, so built alone it
  links no render or audio code (checked:
  `server_builds_without_presentation`, `workspace_crates`); in a
  workspace build features unify, so releases build it with `-p`.
- `combat_arms_server` (apps/combat_arms_server/) is Combat Arms'
  dedicated server (`combat_arms_server`): Chimera's simulation crates,
  the network and `combat_arms` without `ui` (its `ServerPlugin`, which
  its tests build too), so it links no render or audio code and never
  the app (`combat_arms_app`), CS:S or Source's logic (checked the same
  way); built alone with `-p` for release.
- `mashup` (the repository root; the old name) is a thin shim: `cs_source_app`
  re-exported at the old `mashup::` paths (src/lib.rs) for `dump` (an
  install inspector for several games; its test in tests/it/). Nothing
  new goes in it.
- Each crate keeps its tests: unit tests beside the code, integration
  tests in one binary per crate (`<crate>/tests/it/`, `heavy::` for the
  slow ones; `cs_source`'s front end's in tests/ui/ with `ui`), in the
  lowest crate whose API they need (docs/OBSERVABILITY.md, section 1;
  checked: `tests_are_one_binary_per_crate`). Scenario tests run on the
  engine's harness (`chimera_world::harness::Sim`) with the base of
  their layer: the engine's (`chimera_sim::SimBasePlugins`) in Chimera's
  crates, CS:S's (`cs_source::harness::CsSim`) in `cs_source`'s, so
  the app keeps only its own.
- `repo_checks` (checks/) is the repository's own checks: the
  architecture tests (checks/tests/it/architecture.rs) and `affected`,
  the hooks' test selection (changed files to packages, then every
  package whose tests depend on them through cargo's crate graph). Plain
  crates only, no workspace crate or Bevy (checked), so it builds in
  seconds; its tests run on every selection.
- Tools live at the lowest layer whose knowledge they need, each layer's
  in its own crate, binaries behind `tools` (checked: `workspace_crates`):
  `chimera_tools` (chimera/tools/: the perf gate, `imgcmp`, `tracesum`;
  no Source, game or app), `source_tools` (source/tools/: RCON,
  `demodump`, `testmap`; no Bevy, Chimera or game) and `cs_source_tools`
  (games/cs_source/tools/: `refcmp`'s CS:S driver, `movecmp`, `demosim`, `mapsweep`,
  `perfgate`'s scenarios). CS:S's tools still depend on `cs_source_app`
  (its build stamp, map loading by id: tech-debt.md).
  An app's own tools only for app concerns (launch, packaging, budgets).
- `workspace-hack` (chimera/workspace-hack/, generated by `cargo hakari`,
  .config/hakari.toml) unifies third-party features: every member but
  `source_formats`, `lithtech_formats`, `source_tools`, `chimera_dev` and
  `repo_checks` depends on it (checked), so `cargo test -p <crate>` reuses the workspace's Bevy build.
  `chimera_dev` (chimera/dev/) carries Bevy's development features
  (`dev`: dynamic linking, the remote protocol; `profile`: Chrome traces),
  which every crate's `dev`/`profile` reaches through `chimera_core`'s;
  hakari leaves it out, so the hack never turns them on for release.
  The workspace's `bevy` and `avian3d` have no default features: the
  dedicated servers build without Bevy's render, window and audio crates,
  which `chimera_present` and `chimera_client` ask for (`default`, `wav`;
  avian's `debug-plugin`) and every simulation crate takes as
  dev-dependencies for its own tests; hakari leaves those crates out of the
  hack (`.config/hakari.toml`; docs/performance.md, "Dedicated servers
  without Bevy's default features").

## Layers

```
            client            (window, input, camera, HUD, debug, agent tools)
              |
   harness    |    games/<name>   (one module per game)
      \       |       /    \
       net ---'      /      \       (net: transport, replication, join handshake)
        |           /        \
       bot   rules   logic  movement   greybox   mount   (mount: VFS, local config)
        |      |       |      |          |
        |  objectives  |      |          |      (objectives: bomb, hostages, zones)
        |      |       |      |          |
        |    weapon    |      |          |      (weapon: parts, inventory, weapon frame)
        |      |      /       |          |      (logic: entity I/O, triggers, movers, +use)
        |     map ---'        |          |      (map: neutral MapData + spawner, sound data)
         \     |       /          /
           character             (components every character has)
               |
             slots               (registries, loadout, swapping)
               |
            console              (cvars, commands, queue; games and client register)
               |                 (game_log beside it: game events, the server log)
             core                (shared vocabulary)
                                 (env under console and mount: variables, user folders;
                                  profile beside them: a player's settings file;
                                  query_cache under console: kept query states)
```

Exact edges: game_log uses core, console and gameplay (the gameplay's
game names players in the log); net uses the engine's
simulation (core, console, game_log, slots, character, movement, map,
weapon) and names no game or Source logic (`minigame_net` uses net,
minigame, mode and core):
logic and games/<name> use it to register what they replicate (their
parts, `net::parts`), and harness and client use it; bot uses core, console,
slots, character, map, weapon (grenades
it carries and throws), objectives (planting, defusing); rules uses core,
console, game_log, gameplay (it registers its hooks by game), weapon,
objectives; objectives uses core, console, slots,
character, map, weapon (games use it too, to fill in their bomb and
hostages); weapon uses core, console, map; logic uses core, console,
map (games feed it through `MapData::entities`; harness and client may use
it); map uses only core. games/<name> uses core, console, slots,
character, movement, mount, map, weapon, objectives and rules (to register
its rule modules), bot and net (its part of the network); its `ui/` uses
those, client and the rest of the simulation.

Presentation sits beside the simulation, above `map`: `audio` (sound
playback, live sounds, room DSP, soundscapes, hearing) uses core and map,
and only `client` and `map_plugin` use it; `render` (drawing what the map
spawns and simulates: the drawn phases of spawning a map, what is made
in play, effects, culling, water, shadows, the sky, view models, the
materials and their shaders) uses core and map, and only `client`,
`map_plugin` and a game's `ui/` use it. `map_plugin` builds
`map::MapPlugin` (its type, re-exported into `map`): map's registration
(`map::build`) with audio's and render's systems at the places map keeps
for them (`map::MapLayers`), in one fixed order; the dedicated server
adds `map::MapSimPlugin` (map's alone). `assets` (the games' installs as
asset sources, read through `mount`; the loaders' device check from
`render`) uses core, mount, game_registry and render; apps and a game's
`ui/` use it. Animation evaluation stays on the
simulation's side (the server poses hitboxes); see
docs/plans/active/engine-split.md, "Crates and compile time".

Drawn-part requests: what the simulation makes during play says in plain
data what it looks like, and `render` fulfils it; the simulation never
builds a mesh or material. A character's team, `BodyName` and class pick
its body, `Held` names what it holds, a `Ragdoll` takes the dead's body
over, `LooseItem`/`ShownItem` name a held model to lie or move as;
a gib carries its `GibLook` (which model of which gib list, skin, glass,
fade distances) and a window its `BrushPanes`; render's systems react to
them (added, changed, removed, without their drawn marker). Without `render` (a dedicated server) the requests stay
unfulfilled and the simulation's parts (bodies, colliders) work alone
(`games/cs_source/tests/it/drawn_parts.rs`).

`metrics` (per-thread CPU time and hardware counters, distributions, the
tick's measurements) uses no other module of ours; any layer may use it
(so far harness and client; `SimPlugins` adds its tick plugin).

A module may use only the modules below it. Enforced by
`checks/tests/it/architecture.rs` (`ALLOWED`); update both together.

- `core` never depends on anything in this crate.
- Simulation code (everything except `client`) never depends on `client`, so
  it runs headless in tests and, later, on a dedicated server.
- `games/<a>` never uses `games/<b>`. Shared needs go into `core` once two or
  three games need them (README, "rule of three"), or through a bridge.
- A game's front end (its crate's `ui`, behind its `ui` feature: menus, panels, HUD) sits on
  `client` (the shell: input, camera, console UI, VGUI-like widgets, debug
  tools, net glue); the rest of the game never uses it. The shell reaches
  it only through `client::front_end` (hooks the game registers), and the
  app adds it (main.rs). The shell names no game (checked): what it needs
  of one (install, map cycle, tick, default binds, movement cvars) is data
  the game registers (`game_registry::GameInfo`, the first registered
  being the default game, for bare map names); which game's rules, HUD,
  binds and tick play is the `gameplay::Gameplay` the app sets, and each
  game's match-wide systems and HUD run only while it is the gameplay's
  (`gameplay_is`, `hud_is`).
- Rule modules (`rules::module`): a game registers its modes by name
  (`cs_source:bomb`, ...), picked by the map and cvars; their code is
  still in `rules` and `objectives` (engine-split.md).

## Modules

The module list is `docs/modules.md`: one line per source file, grouped by
crate folder, generated from the first sentence of each file's `//!` doc
(which is the source of truth). When code changes what a file does, change
its `//!` doc in the same commit and regenerate with
`cargo run -q -p repo_checks --bin docs_index`; the test
`generated_docs_are_fresh` (`checks/tests/it/architecture.rs`) fails when the
list is stale. Never edit `docs/modules.md` by hand. Known shortcuts get the
same treatment (`docs/tech-debt.md`).

## Data flow per tick

```
PreUpdate:    (server) UserCmds from clients ──> CommandBuffer (by tick), DropRequest ──> drop;
              (client) replication ──> NetBody, NetMover, NetProp, NetItem and their Snapshots (by server
              tick); ticks heard ──> InterpClock, held snapshots; our NetBody ──> Transform (until
              predicted); OwnState ──> pending; NetHeld ──> others' held weapon; FireBullets ──>
              RemoteShot; WeaponFx, Killed, HitConfirm, Detonation, Senses ──> weapon events, Died,
              Detonated, Blinded, Deafened; NetSmoke ──> SmokeCloud
Update:       keyboard/mouse ──> Intent (local player)
FixedFirst:   (client) drawn transforms ──> the simulation's (map::interp::restore); command tick + SimClock;
              server state vs. predicted: on a mismatch restore it, re-run later commands (core::predict)
FixedUpdate:  bot brains, (server) each player's command for this tick ──> Intent, its view tick ──>
              lagcomp::ViewTick (before the rules, which hold it); (client) local Intent normalized
              (+ the render tick) ──> this tick's NetCmd
              SimSet::Rules     rounds (each step ──> mode::ModeStep observers, at once;
                                new round ──> RoundRestarts + RoundRestart: gibs, panes reset at
                                once (map::breakables' observer), objectives::round_start;
                                bomb/hostage state ──> round end; FreezeTime, RoundOpen),
                                respawn; the dead's and (freeze time) everyone's Intent held
              objectives        arming/defusing hold still; hostages follow ──> Intent
              MapControls ──> Intent (taken buttons, at controls, frozen; the buttons kept for game_ui)
              weapon selection/deploy ──> MaxSpeed           (core::Predict::Select)
              RoundRestarts changed ──> logic world re-created, nodes back
              LogicSet::Pre     thinks, movers push/carry characters, +use (Intent.use_key);
                                (client) movers stepped to the command tick, our player carried
                                (logic::net::place, also in each replayed command)
              SimSet::Commands  Intent.command stamped (tick + Seed)
              SimSet::Movement  Intent (+ MaxSpeed) ──> Transform, Velocity, MovementState  (Predict::Movement)
              AnchorSet         placed weapons (loose or carried) ──> EntityAnchors, anchor nodes
              LogicSet::Post    trigger touches (push ──> BaseVelocity, teleport), untouch,
                                event queue (outputs ──> inputs), removals; effects ──> Damage, HUD text
              SimSet::Weapons   Intent + MovementState ──> traces, Damage, WeaponEvent, PlaySound
                                (inaccuracy, frame, recoil, timed sounds, zoom, grenade throws:
                                Predict::Weapons; (server) a remote player's traces against others
                                rewound to its view tick from HitHistory: weapon::lagcomp)
                                then the bomb: arming, plant, defuse (use search), beeps,
                                Explosion, FireEntityOutput; +use on hostages ──> ObjectiveEvent
              LogicSet::Damage  Damage on breakable and prop nodes ──> break, panes, gibs, sounds,
                                Explosion (next tick, in SimSet::Weapons)
              then              Damage ──> Health, Died ──> Score, Dead
                                WeaponEvent, Damage ──> impact sounds, decals, Particles (game effects)
FixedPostUpdate: physics step; new contacts of props ──> impact Damage (crush, next tick)
FixedLast:    (client) Transform, eye, look, punch ──> interpolation samples (last two ticks); predicted
              state ──> PredictionHistory; (server) characters ──> NetBody, NetHeld, OwnState (with
              what they carry); bodies animated from the tick's values (SimAnimator) ──> Hitboxes
              (map::SimPose) ──> HitHistory; movers ──> NetMover; physics props ──> NetProp; loose
              weapons, grenades ──> NetItem
after fixed loop: (client) samples + overstep fraction ──> drawn Transform, RenderedView; others,
              physics props and items (NetDrawn) from their snapshots at the render tick (net::interp,
              net::props, net::weapons)
Update:       (client) RemoteShot ──> others' rounds traced again (no damage) ──> WeaponEvent, PlaySound
PostUpdate:   (server) changes replicated (a mutate message with each client's updates: UpdatePacing),
              OwnStateDelta to each player due one;
              WeaponEvent ──> FireBullets, WeaponFx (to the others), HitConfirm (to the shooter);
              Detonated, Died, Blinded/Deafened ──> Detonation, Killed, Senses;
              (client) new NetCmds + backups ──> UserCmds; drop ──> DropRequest
              PlaySound ──> one-shots; SoundControl ──> LiveSounds (ambient loops) ──> audio
Update:       camera <── Intent (local look) + drawn Transform + RenderedView (eye offset, roll, punch)
              Particles stepped (frame time, at most 0.1 s) and drawn
              DriveAnimation  WeaponEvent ──> Animator (bodies), ViewAnimator (view models) ──> joints
                              view-model animation events ──> ViewModelEvent ──> flash, light, shells
                              Intent + Velocity ──> ViewModelOffset (bob, sway)
```

Intent buttons are levels, not edges. Movement runs at a fixed tick
(`DEFAULT_TICK_HZ` until a spec sets one); rendering draws between the last
two ticks (`map::interp`): gameplay, traces and shots use the simulation's
values, cameras, bodies and their animation the drawn ones.

## Slots

An implementation registers under a namespaced ID (`chimera:noclip`,
`combat_arms:movement`) with a marker component. Its systems act only on
entities with that marker, so swapping is removing one marker and inserting
another (`slots::set_movement`). Only Movement exists so far; the other slots
follow the same pattern when they arrive.
