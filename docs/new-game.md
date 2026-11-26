# Porting a new game

The checklist for game number N. Start with
[CONTRIBUTING-GAMES.md](CONTRIBUTING-GAMES.md) (the workflow, the two
kinds of sessions, the session briefs) and
[engine-capabilities.md](engine-capabilities.md) (what Chimera already
does). The reasons are in
[plans/active/architecture-pass.md](plans/active/architecture-pass.md)
(sections 3 and 4); the rules are checked by `cargo test -p repo_checks`
(`layers_follow_paths`, `games_share_one_shape`, `workspace_crates`),
whose failures say what to change.

**`tools/new-game <id> "<Display Name>"`** (`tools\new-game.ps1` on
Windows) does M0's mechanical part: the crates below in this shape, a
stand-in movement and gun as data, the specs' templates, the plan, the
workspace and roster lines, install discovery and a fast scenario test.
The game then plays in the sandbox at once (with
`CHIMERA_<ID>_ROOT` set: a game's guns are offered only when its install
is found): `cargo run --features dev --bin ultimate_gun_game -- +sandbox +rung <id>:weapon_standin`.

Examples: HL2:DM (`games/hl2dm`, `apps/hl2dm`,
[plans/active/hl2dm.md](plans/active/hl2dm.md)); DoD:S's M0 for a Source
game (`games/dods`); Combat Arms for a new engine family
(`lithtech/formats`, `games/combat_arms`).

## Before any code

1. **A plan** in `docs/plans/active/<id>.md` (CLAUDE.md: propose it
   before writing a loader): the install (where it comes from, how it is
   found), the formats, the milestones below, what is out of scope.
2. **Specs** in `specs/<id>/`, written in a separate session in our own
   words ([specs/README.md](../specs/README.md)). Implementation reads
   specs only. No assets, extracted files or keys in the repository,
   ever.
3. **Pick the id**: lower case, one word or snake case (`halo_ce`). It
   names the crate, the folder, the maps (`<id>:<map>`), the console
   namespace, the install's key in `chimera.local.toml` (`[games.<id>]
   path`) and the asset set.

## The crates

| Crate | Folder | Holds | Example |
|---|---|---|---|
| `<family>_formats` | `<family>/formats` | the engine family's file formats as plain data: no Bevy, fixtures written by tests, `heavy::` tests on the install that skip without it. Only for a new family | `lithtech/formats` |
| `<id>` | `games/<id>` | the game's simulation as a library (below) | `games/hl2dm` |
| `<id>_ui` | `games/<id>/ui` | its front end on the client shell (HUD, menus, front-end hooks): on its game and `chimera_client`, never another game or an app | `games/tf2/ui` |
| `<id>_app` | `apps/<id>` | the game's binary: a `GameApp` static and one call (below) | `apps/hl2dm` |
| `<id>_server` | `apps/<id>_server` | a static and one call (below); Chimera and the game only, never an app (`game_servers_use_no_app`) | `apps/hl2dm_server` |

A new family's folder is a loader layer the day a crate lives in it;
its formats crate may use `chimera_vfs` (archives as mount layers) and
`chimera_formats` (sound files, KeyValues text) and nothing else of the
workspace. Behaviour that every game of the family will share stays in
the first game's crate until a second game needs it; then it moves to
`<family>/engine` (a lift, as `source/engine` came out of `cs_source`).

For each new crate (tools/new-game does this for the game's two): add
it to `members`, `default-members` and `[workspace.dependencies]` in the
root Cargo.toml; give it a `dev` feature; run `cargo hakari manage-deps
-y` and `cargo hakari generate` (a plain-data crate goes on
`.config/hakari.toml`'s `final-excludes` and the check's `NO_HACK`
instead); add its paragraph to docs/ARCHITECTURE.md.

## The game crate's shape

`games_share_one_shape` checks these names, so every app, tool and
party mode finds the same seams in every game:

```
games/<id>/Cargo.toml        [package.metadata.chimera] loaders = ["<family>"]
                             features: `dev`; no client, presentation or UI kit
games/<id>/src/lib.rs        pub mod game; pub mod harness;
                             pub struct SimPlugins   (Chimera's base + what its maps need)
games/<id>/src/game/mod.rs   pub const GAME, pub static INFO: GameInfo,
                             pub struct <Game>Plugins (its parts, `RegisterGame(&INFO)` first),
                             pub static BUNDLE: GameBundle
games/<id>/src/harness.rs    pub struct <Game>Sim    (its scenario tests' base)
games/<id>/tests/it/main.rs  one test binary; slow tests in tests/it/heavy/
games/<id>/data/             its numbers as data (weapons, movement), our own files,
                             each named by `chimera_core::game_data!("data/..")`
games/<id>/ui/               crate `<id>_ui`: its front end; src/lib.rs has
                             `with_hud` and `party::CLIENT`; tests/it/ for
                             what only the client shows; ui/data/ its HUD and
                             menu documents
```

Data documents are `const`s made by `game_data!` (a path from the
crate's folder, never outside it), not `include_str!`: a dev build reads
them from the checkout at run time, so editing one rebuilds nothing
(restart, or `data_reload` for readers that ask again), and a release
compiles them in (`chimera_core::game_data`; architecture-for-iteration.md,
stage 2). A front end's documents go in its own crate's `data/`, so their
edits select only the front end's tests and what is above it.

The front end is its own crate so that the simulation and its tests
never build the client shell: a client-side edit then selects the front
ends' and apps' tests, not every game's simulation suite, and a
dedicated server links the same build of the game as the client
(docs/plans/active/architecture-for-iteration.md, stage 1). All
five games have it so.

- `loaders` lists the loader layers the game builds on (`[]` for a game
  with none). A game never depends on another game or on an app; what
  two games need moves into Chimera or their family's crates.
- `INFO` (`chimera_core::game_registry::GameInfo`): id, short name,
  full name (every menu, picker and card names the game by these, through
  `chimera_core::game_registry::known`: no table of names elsewhere), content folder, tick, how its install mounts (and, for a mount that costs much to open, `unopened`: its loose layers and its archives' files named without opening them, so its map list reads back from the converted cache), its map names and
  cycle, its Steam app if it has one (`steam: SteamGame`: app id, the
  files that show a folder is the game, `server_apps` its dedicated
  server apps for SteamCMD's default places, and `folders` the names it
  goes by in a games folder, `~/games/<name>`, `<name>_ds`, `<name>-ds`:
  `["cs16"]`, `["counter-strike-source", "css"]`; case and `_`/`-` don't
  matter; docs/assets-for-developers.md, "Where an install is found"),
  its world unit in metres, its console namespaces.
- `BUNDLE` (`chimera_world::game_bundle::GameBundle`): what a party app
  takes of the game: its info, parts, map loader, its players and guns
  on other games' maps, its binds, its view-model FOV and its cvar,
  what it brings as a base game (`base`), what it adds to a composed
  simulation, and `player_options`: its `data/player_options.toml`, the
  game's own keys and options on the party apps' Options screen (Games
  tab: its actions, its cvars, its own values of shared settings;
  party_kit `game_options`).
- In `<id>_ui`: `party::CLIENT` (`GameClient`: its HUD and the rest of
  its front end for a party app) and `with_hud` for its own app.

**Gated from the first commit.** In a party app the game's `BUNDLE.sim`
runs inside `chimera_core::games_in_play::gate_game`: every per-tick
system whose type path starts in the game's crate runs only while the
game is in play (a character has its `GameIdentity`, a weapon of its
`game:` ids exists, the ladder names its guns, its map spawned, or any
entity holds one of its crate's components). Its front end is gated the
same way on the party's client: every per-frame system of `<id>_ui` (its
`GameClient::ui_crate`) or of the game's crate that its `party::CLIENT`
adds (`party_kit::client::gate_front_ends`); its HUD and view model are
drawn only for a character holding its gun, which puts it in play first.
So: keep the crate's name equal to the game's id; add systems in
plugins' `build` (`finish`'s are gated by crate alone, `cleanup`'s not
at all); a system of the crate that must also work for other games (a
map's explosions of no game, a HUD part drawn under another game's HUD)
is named with `serves_every_game`; a
per-character loop visits the game's own characters
(`games_in_play::GameCharacters::of`), not every character; and leave
data the game needs only once it plays to `GameEntersPlay` (an observer). A
match without the game must cost no instructions of its systems
(`apps/ultimate_gun_game/tests/it/heavy/idle_games.rs`, under 1% for all
absent games together; a client frame with every front end:
`idle_front_ends.rs`).

## Order of work

Get a character walking on the greybox with stand-in parts first (M0:
tools/new-game), then replace one part at a time, each behind its spec
and a scenario test. Each step's detail is in the named module's `//!`
doc and [engine-capabilities.md](engine-capabilities.md).

1. **M0, the skeleton**: `INFO`, the mount (`game::mount::open` through
   `chimera_core::mount::served::open`, so the asset store works),
   `SimPlugins`, the harness, a test that the game registers and (heavy)
   that the install mounts. Scaffolded. Its `PARTS` say `scaffold: true`:
   the party reports the game but deals its stand-in gun into no ladder
   and lists it nowhere in the sandbox, so the cross-game tests (rung
   shares, every movement climbing every ladder) don't hold a stand-in to
   them. Set `scaffold: false` when its first real gun and movement land.
2. **M1, the apps** (optional for a guest game): the server is a
   `GameServer` static and `chimera_net::dedicated::run(&SERVER)`, the
   app a `GameApp` static and `chimera_client::client::app::run(&APP)`
   (apps/hl2dm_server, apps/hl2dm); what only some have is a hook on
   `ServerHooks`/`GameAppHooks` with `..NONE`. A main.rs stays a call
   (`app_mains_are_small`).
3. **Maps**: a `MapLoader` that returns `MapData` in metres
   (engine-specific features in typed `extras`, never a new `MapData`
   field), `MapLoadOptions` honoured (lean: no textures). Then a
   `MapDressing` for its players and guns on other games' maps, kept by
   Chimera: give `chimera_world::map::kept::dress_cached` a `Dressing`
   (CS:S's `bundle::GUEST`; memos through `Dressing::memos`, Combat
   Arms' `world::DRESSING`). Never a cache of your own.
4. **Movement**: a marker component, `register_movement` under
   `<id>:movement`, its state registered with `predicted_net`, its
   numbers as data in the game's own unit, its envelope
   (`add_movement_envelope`).
5. **Weapons**: TOML documents in `data/weapons/`
   ([weapon-data.md](weapon-data.md)), behaviour as registered hooks,
   predicted parts by name, hit zones through `Hitgroup`.
6. **Rules**: rule modules on the mode runtime (`chimera_sim::mode`),
   gated by `gameplay_is(GAME)`, respawns through
   `chimera_sim::spawning`; operator commands `Console::server_command`,
   the caller's `Console::player_command`.
7. **Network**: `net_part("<id>", ...)` for what it replicates of its
   own, `JoinRules` if the default join doesn't fit; one test per
   feature on `harness::NetSim`.
8. **Front end** (`<id>_ui`): its HUD on the shell's plugins
   (`with_hud`), menus through `client::front_end`, layouts on
   `client::canvas` or `client::vgui`. Your HUD scales under its own
   roots (`client::hud::HudRoot`), never Bevy's global `UiScale`, and
   draws nothing of the shell's overlay (the overlay guard,
   `apps/party_roster/tests/it/overlay_guard.rs`;
   `games_leave_the_overlay_alone`; docs/DESIGN.md, "HUD layers").
9. **Party**: `PARTS`, `BUNDLE`, `CLIENT`, and the map loader's `WORDS`
   (`chimera_world::map::MapWords`: spawn teams as `SIDE_FIRST`/
   `SIDE_SECOND`, surfaces as the neutral kinds; a Source game reuses
   `source_engine::maps::SURFACE_WORDS`). The roster lines (`GUESTS`,
   `client::CLIENTS`) are scaffolded; no table names a pair of games,
   or a game: menus, pickers and cards name it by `INFO`'s `name` and
   `short_name` (`game_registry::known`), and the layering rules
   (checks/tests/it/architecture.rs) say `<games>` for every game crate.
   The Controls tab is made of `data/player_options.toml`. A game that
   can be a party's base fills `BUNDLE.base` and `CLIENT.base`.
10. **Tools and docs**: the game in `dump` and `chimera_pack` (its row in
    `games()`; a game without a Steam app names how its build is read,
    `Game::build`; its sets in `record_pieces`); its commands in
    CLAUDE.md; its paragraphs in docs/ARCHITECTURE.md.

New systems go behind the game's run condition once per-game gating
lands ([game-logic-distribution.md](plans/active/game-logic-distribution.md),
section 4): an absent game should cost nothing a tick.

## Don't rely on

- Being the first registered game, or the only one: the gameplay is a
  run-time choice (`chimera_core::gameplay`).
- Your systems running in every match: a party runs them only while the
  game is in play (above, "Gated from the first commit").
- Chimera's defaults being in your unit: several are still Source's
  inch (architecture-pass.md, step 10). Pass your numbers as data.
- One local player forever (architecture-pass.md, step 12).
- A hook that takes `&mut World` being the right shape for per-tick
  work: declare what you read and write (docs/DESIGN.md, "ECS style").
