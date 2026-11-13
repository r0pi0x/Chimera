# Porting a new game

The checklist for game number N, end to end. The reasons are in
[plans/active/architecture-pass.md](plans/active/architecture-pass.md)
(sections 3 and 4); the rules it leans on are checked by `cargo test -p
repo_checks` (`layers_follow_paths`, `games_share_one_shape`,
`workspace_crates`), whose failures say what to change.

The newest complete example is HL2:DM: `games/hl2dm`, `apps/hl2dm`,
`apps/hl2dm_server`, [plans/active/hl2dm.md](plans/active/hl2dm.md). For
a game of a new engine family, Combat Arms is the example:
`lithtech/formats`, `games/combat_arms`,
[plans/active/combat-arms.md](plans/active/combat-arms.md).

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
| `<id>` | `games/<id>` | the game as a library (below) | `games/hl2dm` |
| `<id>_app` | `apps/<id>` | the game's binary: a `GameApp` static and one call (below) | `apps/hl2dm` |
| `<id>_server` | `apps/<id>_server` | a static and one call (below); Chimera and the game only, never an app (`game_servers_use_no_app`) | `apps/hl2dm_server` |

A new family's folder is a loader layer the day a crate lives in it;
its formats crate may use `chimera_vfs` (archives as mount layers) and
`chimera_formats` (sound files, KeyValues text) and nothing else of the
workspace. Behaviour that every game of the family will share stays in
the first game's crate until a second game needs it; then it moves to
`<family>/engine` (a lift, as `source/engine` came out of `cs_source`).

For each new crate: add it to `members`, `default-members` and
`[workspace.dependencies]` in the root Cargo.toml; give it a `dev`
feature; run `cargo hakari manage-deps -y` and `cargo hakari generate`
(a plain-data crate goes on `.config/hakari.toml`'s `final-excludes`
and the check's `NO_HACK` instead); add its paragraph to
docs/ARCHITECTURE.md.

## The game crate's shape

`games_share_one_shape` checks these names, so every app, tool and
party mode finds the same seams in every game:

```
games/<id>/Cargo.toml        [package.metadata.chimera] loaders = ["<family>"]
                             features: `ui` (the front end's optional
                             dependencies), `dev`
games/<id>/src/lib.rs        pub mod game; pub mod harness; #[cfg(feature = "ui")] pub mod ui;
                             pub struct SimPlugins   (Chimera's base + what its maps need)
games/<id>/src/game/mod.rs   pub const GAME, pub static INFO: GameInfo,
                             pub struct <Game>Plugins (its parts, `RegisterGame(&INFO)` first),
                             pub static BUNDLE: GameBundle
games/<id>/src/harness.rs    pub struct <Game>Sim    (its scenario tests' base)
games/<id>/tests/it/main.rs  one test binary; slow tests in tests/it/heavy/
games/<id>/data/             its numbers as data (weapons, movement), our own files
```

- `loaders` lists the loader layers the game builds on (`[]` for a game
  with none). A game never depends on another game or on an app; what
  two games need moves into Chimera or their family's crates.
- `INFO` (`chimera_core::game_registry::GameInfo`): id, short name,
  content folder, tick, how its install mounts, its map names and
  cycle, its Steam app if it has one, its world unit in metres, its
  console namespaces.
- `BUNDLE` (`chimera_world::game_bundle::GameBundle`): what a party app
  takes of the game: its info, parts, map loader, its players and guns
  on other games' maps, its binds, what it adds to a composed
  simulation.
- With `ui`: `ui::party::CLIENT` (`GameClient`: its HUD and the rest of
  its front end for a party app) and `with_hud` for its own app.

## Order of work

What Combat Arms' skeleton and HL2:DM's milestones taught: get a
character walking on the greybox with placeholder parts first, then
replace one part at a time, each behind its spec and a scenario test.

1. **M0, the skeleton**: `INFO`, the mount (`game::mount::open`; the
   install's archives as layers of a `Mount`, through
   `chimera_core::mount::served::open` so the asset store works), `SimPlugins`,
   the harness, a test that the game registers and (heavy) that the
   install mounts. HL2:DM's M0 was 167 lines of game crate.
2. **M1, the apps**: the server is a `GameServer` static and
   `chimera_net::dedicated::run(&SERVER)` (apps/hl2dm_server: its usage
   text, the static, and a build.rs for the version stamp). What only
   some servers have is an optional hook on the static
   (`ServerHooks { .., ..ServerHooks::NONE }`): the map cycle, the map's
   file for clients without it (`map_files`), the greybox's own tick,
   another player count, several games' maps, a minigame
   (apps/cs_source_server, `party_kit::server`). The app is the same
   shape: a `GameApp` static (`chimera_client::client::app`: the game's
   `INFO` for the usage text, its title, maps, installs, `sim`, `hud`
   and `front_end`) and `chimera_client::client::app::run(&APP)` in
   main.rs (apps/hl2dm); what only some apps have is a
   `GameAppHooks { .., ..GameAppHooks::NONE }` hook: command-line
   defaults, what the machine lacks, a default map, a start of its own
   (a main menu), the start map at the HDR level, the debug views, its
   server's name for `--help`, a last word (apps/cs_source,
   `party_kit::client`). An app's main.rs stays a call
   (`app_mains_are_small`). Placeholder movement
   (`chimera_sim::movement`) and a test weapon as data.
3. **Maps**: a `MapLoader` that returns `MapData` in metres (the world
   and the game's kit apart; engine-specific features in typed
   `extras`, never a new `MapData` field), `MapLoadOptions` honoured
   (lean: no textures, for servers and tests). Then a `MapDressing` so
   the game's players and guns can stand on other games' maps.
4. **Movement**: a marker component, `register_movement` under
   `<id>:movement`, its state registered with `predicted_net`, its
   numbers as data in the game's own unit.
5. **Weapons**: TOML documents in `data/weapons/`
   ([weapon-data.md](weapon-data.md)), behaviour as registered hooks,
   predicted parts by name. Hit zones through Chimera's `Hitgroup`.
6. **Rules**: rule modules on the mode runtime (`chimera_sim::mode`),
   gated by the gameplay (`gameplay_is(GAME)`), respawns through
   `chimera_sim::spawning`. Commands only the server's operator may run:
   `Console::server_command`; commands that act on the caller's player:
   `Console::player_command`.
7. **Network**: `net_part("<id>", ...)` for what the game replicates of
   its own, own-state parts, `JoinRules` if the default join doesn't
   fit. One network test per feature on `harness::NetSim`.
8. **Front end** (`ui`): its HUD on the client shell's plugins, menus
   through `client::front_end`, layouts on `client::canvas` or
   `client::vgui`. Nothing outside `ui` uses `ui`.
   Your HUD scales under its own roots (`client::hud::HudRoot`) by its
   own rule (node sizes from its canvas), never Bevy's global `UiScale`,
   and draws nothing of the shell's overlay (a minigame's panel and
   callouts, the kill feed): the overlay guard
   (`apps/party_kit/tests/it/overlay_guard.rs`) covers you once your
   `CLIENT` is in the party's list, and `repo_checks`
   (`games_leave_the_overlay_alone`) fails a game crate that names the
   overlay or `UiScale` (docs/DESIGN.md, "HUD layers").
9. **Party**: `PARTS` (`GameParts`: what it lends), `BUNDLE`, `CLIENT`;
   add the bundle to the party kit's list. Its gun rungs appear in the
   Ultimate Gun Game from the parts registry.
10. **Tools and docs**: the game in `dump` and `chimera_pack`; its
    commands in CLAUDE.md; its paragraphs in docs/ARCHITECTURE.md.

## Don't rely on

- Being the first registered game, or the only one: the gameplay is a
  run-time choice (`chimera_core::gameplay`).
- Chimera's defaults being in your unit: several are still Source's
  inch (architecture-pass.md, step 10). Pass your numbers as data.
- One local player forever (architecture-pass.md, step 12).
- A hook that takes `&mut World` being the right shape for per-tick
  work: declare what you read and write (docs/DESIGN.md, "ECS style").
