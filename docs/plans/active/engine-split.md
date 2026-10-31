# Plan: engine, Source and games as crates in one monorepo

Status: decided 2026-10-07: one monorepo (`r0pi0x/LuckerParty`, renamed `r0pi0x/Chimera` on 2026-10-16, now public;
history rewritten to the new identity), restructured into the crates
below. Separate repos only if a good reason comes up later; crates keep
that a folder move.

Naming and layering (user, 2026-10-15): the engine is **Chimera**
(folder `chimera/`, crates `chimera_core`, `chimera_world`, `chimera_sim`,
`chimera_net`, `chimera_present`, `chimera_client`; were `engine/` and
`engine_*`, the names this plan's history below still uses). Layers:
Rust, Bevy, Chimera, the loaders (`source_formats`, `source_logic`), games
as libraries (`cs_source`, later `combat_arms`, ...), apps on top (one
faithful app per game, mashup apps using several, Lucker Party the rich
one). A tool lives at the lowest layer whose knowledge it needs (Chimera's,
Source's, a game's; an app's only for app concerns).

## Why

- Other people (a Discord server sharing game mashups) want to build on the
  engine: a Half-Life 2 group now, more games later. CS:S should be
  playable and buildable by others too.
- Lucker Party will move onto this engine, with friends writing original
  minigames.
- Before anything is shared, the work moves to an anonymous account
  (rewritten history; see "Order").

## Target layout

```
chimera/             neutral: app shell, console, slots/mounts, physics,
                     movement framework, weapons/rounds/bots, HUD, net
source/              Source engine formats and shared Source behaviour:
                     BSP/VPK/VTF/VMT/MDL/PHY, entity I/O and logic classes,
                     soundscapes, Source materials and shaders
games/cs_source/     CS:S: library (plugins); its tools in games/cs_source/tools
games/combat_arms/   Combat Arms: library (local, git-ignored for now)
games/originals/     Lucker Party's own games and assets
minigames/           minigame definitions + rule modules that mix games
apps/cs_source/      the faithful CS:S app (done: `cs_source_app`, binary `cs_source`)
apps/cs_source_server/  its dedicated server (done: binary `cs_source_server`)
apps/mashup/         dev sandbox: any game, any map, all debug tools
apps/lucker_party/   the party product
```

Each game's faithful app is its own crate under `apps/` (decided
2026-10-15, superseding "library + standalone app" in one crate); the
game crate stays a library.

Rules:
- `engine` knows no game. `source` depends on `engine` only. Games depend
  on `engine` (and `source` if they're Source games), never on each other.
- Apps pick games with cargo features. A game that needs something the
  engine can't express gets a new engine abstraction, not a special case.
- Each game crate is both a library and a program: `cargo run -p
  cs_source` is a faithful CS:S (its menus, HUD, modes, loaded from the
  player's install); the same library supplies parts to minigames. A
  game's own modes are minigame definitions, so both uses run the same
  code.
- `checks/tests/it/architecture.rs` already enforces most of this as layers;
  crates make the compiler enforce it.

## Crates and compile time (agreed 2026-10-13)

The layout above stays the outer structure (ownership and reuse); inside
it, crates are cut so that hot code sits high in the dependency graph
(architecture-review.md measured why: one crate means every edit
recompiles all 165k lines, and one `engine` crate at the bottom wouldn't
help).

```
chimera/  game-agnostic, several crates with stable interfaces
  core      core, console, slots/registries, version, metrics
  world     map data, collision, sim-side spawning
  sim       frameworks: movement slot, weapon system, rule-module interface, bots framework
  present   render, audio, animation (a dedicated server can leave it out)
  net       replication and protocol; games register what replicates
  client    app shell, input, console UI, a VGUI-like widget layer, debug tools
source/
  formats   VPK/BSP/MDL/PHY/PCF/KeyValues/.res: pure data, no Bevy
  source    entity I/O and logic classes, Source materials/shaders, soundscapes, Source movement
games/cs_source/   parts (weapons, rule modules, HUD, GameUI menus, bot behaviour)
                   + CsSourcePlugins (the faithful game) + its app
minigames/, games/originals/, apps/*
chimera/tools, source/tools, games/<game>/tools   each layer's tools (behind `tools`)
```

- Parts depend on engine (and source) interfaces only, never on each
  other: CS:S weapons don't assume the CS:S HUD or rounds. Each part
  plugs into a slot (movement, weapons, character, HUD, rules, menus).
  The architecture test enforces it.
- Today's engine-level modules that are really CS:S parts move to
  `games/cs_source` first: the GameUI menus, team/buy menus and HUD
  pieces in `client` (8 files carry `#Cstrike_*`/`cstrike/`), the round,
  bomb and hostage rules in `rules`/`objectives` (behind a rule-module
  interface), and CS:S round state in `net` (game-registered
  replication). This is also the compile win: the hot code moves up.
- **Data-driven first** (user, 2026-10-13): where a thing can be data,
  make it data loaded at run time, with Rust only for behaviour data
  can't express (then as a hook the data names). First target: weapons
  (stats, model, sounds, animations, fire modes as data; special
  behaviour as registered hooks), then rule modules' settings, HUD
  layouts, minigame definitions (below), bots' buy tables.
- Example of what this must support later (not planned now): a game
  crate built on top of `cs_source` that adds original guns and modes
  ("CS:S, our way") as its own ids and data, without touching
  `cs_source`; the faithful game stays the parity baseline and its tests
  run against `cs_source` alone.

Progress (2026-10-13, one session; architecture-review.md has the
measurements):

- [x] (1) Tools out of the test build: no `tools/` package yet; the
  binaries no test runs are behind `required-features` (`tools`: refcmp,
  movecmp, perfgate, tracesum, mapsweep; `server`: mashup_server). A
  leaf edit's test build went from ~228 s to ~127 s (medians, load
  16-36). A `tools/` package comes with the crate split.
- [x] (2) `map/mod.rs` split (P1): `data` (MapData, plain data), `state`
  (what the logic writes), `spawn` (`spawn_map` as ~25 phase functions),
  `bodies`, `props`, `materials`, `images`, `collision`, `sky`; paths
  outside `map` unchanged. `games/cs_source/tests/it/heavy/map_spawn_dump.rs` dumps
  spawned maps for before/after comparisons.
- [x] (2) `audio` out of `map` (P3, first slice): playback, live sounds,
  room DSP, soundscapes, hearing; `map::sound` keeps the simulation's
  side. `map_plugin.rs` builds `map::MapPlugin` (map, then audio, in the
  old order); the architecture test keeps map, logic and net off audio.
- [x] (2) `anim`: decided (coordinator, 2026-10-13): animation
  evaluation is simulation (the server poses hitboxes, as Source's
  server-side bones), so `map::anim`, `bodies::SimAnimator` and the
  ragdolls stay on the sim side; only drawing and skinning is
  presentation and moves with `render`.
- [x] (2) `render` out of `map` (2026-10-13 and 14, three sessions,
  each commit with the spawn dump identical on the five maps modulo
  moved paths; then `render` behind a feature for the server and the
  crate cut, both below):
  - [x] the state `logic`/`net`/`weapon` write moved to `map::state`
    (`VisClusters`, `LogicHidden`, areaportal/occluder states,
    `RenderLook`/`BrushTint`, `MonitorCamera`/`MonitorFog`, the
    effects' messages, `PlaceDecal`, `ParticleRng`): those layers use no
    drawing module of map's.
  - [x] `render/` with the drawn phases of spawning: `map::spawn_map`
    is generic over `map::MapDrawer` (a system parameter called at the
    old places: drawn parts, each prop's drawn parts, textures for the
    resources), `render::MapAssets` implements it; `map_plugin` adds
    `spawn_map::<render::MapAssets>` and `MapHooks::spawn` (for
    `change_map`; was `MapUnloadHooks`).
  - [x] map's system registration (`build_rest`) lives in `map_plugin`
    (order kept); `tint`, `detail`, `sky_occluder` moved whole; `beams`,
    `color_correction`, `lod` split (data stays in map, drawing in
    render).
  - [~] The rest, roughly in this order (each needs its data types left
    in map and its users in map's sim side cut first): glow sprites
    (`GlowSprite`, `glow_visibility`: `vis` and `beams` use them),
    `psys`, `shells` and `rope_sway` (data split), `fish`, `dust`,
    `steam`, `tracer`, `emitters`, `particles` drawing, `decal`
    drawing, `monitor`, `water` drawing, `shadows` (`props.rs`'s
    `update_prop_shadows` with it), `vis` culling (`cull`,
    `fade_windows`, chunking; `copies`, `merge`, `breakables`,
    `prop_physics` name its tags), `sky` drawing, `light_styles`,
    `view_model` drawing, then the materials and WGSL last
    (`world_material`, `prop_material`, `sprite_material`,
    `rope_material`, `fog`, `probe_lit`, `materials.rs`/`images.rs`
    helpers: `breakables`, `loose`, `bodies`, `ragdoll`, `view_model`
    use them, so their drawn parts have to move first, through more
    `MapDrawer` hooks where they spawn). Render's resources and entities
    go at unload through `render::unload_map` (a `MapHooks::unload`
    hook, so they now go after map's own). Done (2026-10-14, one commit
    per group, the spawn dump identical modulo moved paths):
    - [x] `glow_visibility` (`render::glow`; `GlowSprite` stays in map
      for `vis`), `psys` (definitions stay; `Instance`, `SystemEmitter`,
      `update_systems` moved), `shells` (types and `shell_step` stay;
      assets, `eject`, `fly` moved), `rope_sway` (`RopeSway`, `spline`
      stay; `Gusts`, `RopeMotion`, `RopeWind`, `sway_ropes` moved),
      `fish` (`Fish` stays; `swim` moved).
    - [x] `dust` (whole), `steam` (`MapSteam` stays; the headless
      stand-ins through `MapDrawer::spawn_undrawn`), `switch_parts`
      (`render::parts`), `tracer` (`Tracer`, `MuzzleCache` stay),
      `emitters` (the `Map*` data and beam noise stay), `particles`
      drawing (the pool, its stepping and `ParticleMaterials` stay;
      `ParticleAssets` inserted at its old place through
      `MapDrawer::insert_drawn`), `fog::apply_scene_fog` (names the
      particle material; never scheduled).
    - [x] `decal` drawing (`MapDecals`, `LIFT`, `DEPTH_DROP` stay;
      `DecalSurfaces`, `DecalAssets` and `ImpactDecals` inserted at their
      places through `MapDrawer::insert_drawn`), `monitor` (the screen
      cameras' markers and `MonitorFog`'s view fog stay; `MonitorCamera`
      is initialized by map where `MonitorPlugin` was added, the plugin's
      system by render right after).
    - [x] `water` drawing (`MapWaterMaterial`, `WaterSurface`,
      `move_water` and `WaterMovers`, `WaterView` (hearing reads it),
      the reflection camera's markers, the layers and `height_fog` stay;
      the material, cameras, fog, overlays and their systems moved).
    - [x] `shadows` whole, with `props.rs`'s `ShadowState`,
      `update_prop_shadows` and `PropShadow`.
    - [x] `vis` culling: `cull`, `fade_windows` and the culling cvars
      and stats (`NoVis`, `PortalsOpenAll`, `Occlusion`, `VisStats`);
      the visibility data, chunking and the tags (`VisClusters`,
      `FadeDistance`, `Occludee`, `WindowBrush`, `ActiveVisibility`:
      bots and hearing read it) stay.
    - [x] `sky` drawing: `attach_sky`, `follow_sky_camera`,
      `show_skybox_in_place` and the sky resources render inserts; the
      images, `SkyboxCamera` and the layers stay.
    - [x] `light_styles` whole (its texel encoders to `map::images`);
      `MapPlugin::build_first` split at the place its plugin was added
      (`build_after_light_styles`), so registration keeps its order.
    - [x] `view_model` drawing: assets, `draw_view_models`, screens,
      muzzle flashes and their lights, `unload`; the games' types, the
      animator, events, markers (`HeldMuzzle`, which `bodies` spawns) and
      the placement maths (`shells` uses it) stay.
    - [x] `architecture::simulation_uses_no_drawing_module_of_map`: the
      simulation's layers name none of the materials left in map
      (`MAP_DRAWING`; one exception, `SpriteBlend`).
    - [x] Drawer hooks that run after spawning (2026-10-14, one commit
      per group, the spawn dump identical modulo moved paths; component
      ids and the change tick masked where a system or component was
      added): drawn-part requests (docs/ARCHITECTURE.md): what the
      simulation makes in play says in plain data what it looks like
      and render's systems fulfil it; headless nothing is drawn
      (`games/cs_source/tests/it/drawn_parts.rs`). `render::bodies` (bodies, held
      models, the ragdoll's adoption of the dead's body), `render::loose`
      (`LooseItem`, `ShownItem`), `render::breakables` (a gib's
      `GibLook`, windows' `BrushPanes` clipped and made whole),
      `render::props` (`PropLook` on skins and body groups). Resource
      order kept: the gibs' meshes live in `render::bodies::PlayModels`
      (was `CharacterBodies`), built in the old order and inserted at
      the bodies' place; `MapPlugin::build_first` split where the
      ragdoll's drawn systems were added.
    - [x] The materials and WGSL: `world_material`, `prop_material`,
      `sprite_material`, `rope_material`, `probe_lit`, `fog` (merged
      with `render::fog`), `materials.rs`/`images.rs` and their shaders
      are render's; `SpriteBlend` went to `map::data`,
      `parameter_gamma_to_linear` to `map::material_fx`, the water fog
      modes to `map::water`; `SceneFog` is inserted through
      `MapDrawer::insert_drawn` (`DrawnResource::Fog`) at its old place.
      `architecture::simulation_builds_no_drawn_parts` replaces the
      `MAP_DRAWING` check: the simulation's layers name no drawing type
      (`Mesh3d`, `Assets<Mesh>`, materials) and `src/map` holds no
      shader.
    - [x] `merge` whole (a node copied at run time is finished by a
      `MapHooks::copied` hook: render shows its pieces, never merged)
      and the placed ragdoll's never-culled meshes
      (`render::bodies::uncull_placed_ragdolls`): the simulation's
      layers name no drawing type, with no exceptions.
    Client post passes (`client::hdr`, `gamma`,
    `color_correction`) stay in client: they hang off
    `FirstPersonCamera`/`UiCamera`/`ClientArgs`.
  - [x] A server without rendering (2026-10-14): `chimera_world`
    registers all of map's systems itself (`map::build`, in
    `map/registration.rs`), with what draws and plays the map at fixed
    places among them (`map::MapLayers`: hooks that register at a place,
    and the drawing systems of chains that mix map's and render's, e.g.
    the gibs' and the culling's, handed over as `ScheduleConfigs` so each
    stays one chain). `map::MapSimPlugin` is map's alone (the `()`
    drawer: nothing drawn, no headless stand-ins); `chimera_present`'s
    `MapPlugin` fills the places (`Present`). The client's order is
    unchanged: the spawn dump (five maps) and each schedule's system
    order (now in the dump too, `<map>.systems.txt`) are identical
    before and after. `mashup`'s `chimera_present` dependency and the
    client shell (`client`, `games::*::ui`, egui, glyph, clipboard,
    winit crates) are behind the default feature `present`;
    `mashup_server` adds `MapSimPlugin` and builds with
    `--no-default-features --features server` (pre-push hook,
    `architecture::server_builds_without_presentation`, release.yml).
    The server no longer runs render's and audio's systems headless,
    nor spawns the steam jets' undrawn stand-ins or render's
    per-prop components (`LodGroup`, `PropVariants`): its entity ids
    differ from before (it never matched a client's). Measured
    (`cargo build --release`, this box, 4 jobs, sccache): the server
    binary 117.3 MB (77.6 stripped) before, 106.3 (69.4) after; no
    `chimera_present`, egui, client or cpal/rodio symbols left, wgpu's
    halved (Bevy's own); a rebuild after a game file's edit 255 s
    before, 204 s after; 548 crates compiled before, 518 after. Done after
    (2026-10-22, architecture-review.md A5/L6): the workspace's `bevy` and
    `avian3d` have no default features, so the servers build none of Bevy's
    render, window, audio and gamepad crates (689 to 495 packages for
    `cs_source_server`, 115.3 to 86.9 MB); `present` and `client` ask for
    `bevy/default` and avian's `debug-plugin`/`bevy_picking`/`bevy_scene`,
    the simulation crates take them as dev-dependencies, and
    `chimera_world`'s `render` feature holds the greybox's materials and
    the two `bevy_pbr` helpers (docs/performance.md).
  - [x] Cutting the crates (`map` to `chimera/world`, `render` to
    `chimera/present`). Done (2026-10-14): `chimera/world` (package
    `chimera_world`, module `map`): `MapPlugin`'s type moved up into
    `map_plugin` beside its `impl Plugin` (re-exported as
    `map::MapPlugin`; map's part is `map::build_first`,
    `build_after_ragdolls`, `build_after_light_styles`), map's
    `pub(crate)` items became `pub`, `impl MapBeam`'s drawing maths split
    (the data's in map, `render::beams::beam_params` in render), a
    `#[cfg(test)]` stand-in inlined into render's test; spawn dump
    identical on the five maps modulo moved paths (no masking needed).
    Then `chimera/present` (package `chimera_present`): `render`, `audio`
    and `map_plugin` (`MapPlugin`, `pub use`d by `mashup::map`) moved
    whole; their `pub(crate)` items became `pub`, the embedded shaders'
    paths are `embedded://chimera_present/render/...`; `mashup`
    re-exports `render` and `audio`. Spawn dump identical again (five
    maps, modulo paths), `games/cs_source/tests/it/drawn_parts.rs` passes, de_dust2 and
    gg_tbr_water_basin screenshots draw as before. Measured
    (docs/performance.md, "`chimera_world` and `chimera_present`"): a
    leaf edit in render or map, `cargo test --features dev --no-run`,
    30-72 s (was 210 s); in a game file still 127-176 s (mashup's 134k
    lines); `cargo test -p chimera_world` 9 s, `-p chimera_present` 12 s.
    Left: the simulation's other layers (`chimera/sim`), `mashup` itself
    (`render` behind a feature for the server: done, above).
    What it needed (2026-10-14): (a)
    `impl Plugin for MapPlugin` lives in `map_plugin` (root crate) for a
    type in `map`: the orphan rule forbids that across crates, so the
    type moves up beside its registration (e.g. `present::MapPlugin`,
    re-exported as `map::MapPlugin`), or `world` gets its own plugin
    with registration hooks that `present` fills in the same order;
    (b) about 50 `pub(crate)` items of `map` (and more `pub(crate)`
    fields: `LooseAsset`, `GibAssets`, `FlyingGib`, `Ragdoll`,
    `PendingMap`, `PlayableArea`, the props' and bodies' types) that
    `render`/`map_plugin`/`audio` use become `pub`; (c) `map` uses only
    `core` (`chimera_core`) of ours, plus avian, replicon and Bevy's
    image and camera types (`map::sky`'s images, the camera markers):
    no blocker; (d) the root crate re-exports `world::map` and
    `present::render` so `crate::map`/`mashup::map` paths keep working;
    (e) `audio` (above `map`, beside `render`) goes with `present`.
  - Measured: a leaf edit's test build (`client/window_icon.rs`, a new
    `pub fn`, `cargo test --features dev --no-run`) 239 s before
    (load 14-26), 114 s after (load 17-18): one crate either way, so
    the difference is load and cache, not the move; the win comes with
    the crate cut.
- [x] Docs diet (P2): docs/ARCHITECTURE.md's module table became one
  line per module (133 KB to 22 KB) and docs/tech-debt.md one line per
  entry (109 KB to 15 KB); the detail moved into the files' `//!` docs
  (debt under "Known shortcuts"). `architecture::docs_stay_one_line_per_entry`
  caps the line lengths and checks the paths.
- [x] Heavy tier (P4b): map-looping heavy tests split per map, nextest
  tuned to 5 heavy slots weighted by memory; full suite 1,812 s to
  1,009 s (docs/performance.md, "The heavy tier").
- [~] (2) CS:S UI out of `client`, rules behind a rule-module interface,
  game-registered replication (2026-10-13, one session):
  - [x] `games/cs_source/ui/` (`CsSourceUiPlugin`, added by main.rs
    after `ClientPlugin`): the GameUI menus and dialogs (`game_menu/`,
    `options` (its rows; the video cvars stay in `client::video`),
    `first_run`, `server_browser`), the join flow (`join_menus`), the
    buy, team, radio and spectator menus, the freeze cam panel
    (`freeze_cam`), `objectives_hud`, `bomb_fx`,
    `target_id`, `scoreboard`, `window_icon`. The shell reaches them
    through `client::front_end`: triggered events each front end observes
    (`EnteredGame`, `LeftGame`, `Joining`, `JoinFailed`, `MapLoadFailed`,
    `StartGame`) and the state
    it reads, `MenuStatus` and `Panels`, which the menus copy in
    `FrontEndSync` (First, Update after their systems, PostUpdate; the
    shell's readers saw the menus' resources directly before, so a
    reader in `Update` may see a change a frame later than it did).
    `bot::DIFFICULTIES` moved out of Create Server (the debug UI uses
    it). Architecture test: `games_ui` may use `client`; the rest of a
    game may not use its `ui`; `client` names a game only in
    `CLIENT_GAME_EXCEPTIONS` (console, binds, net glue: the install,
    maps, map cycle and tick of CS:S, waiting for a game registry; the
    debug UI's Source movement cvars).
  - Left in `client`, CS:S-flavoured but on generic data (`map::hud`'s
    layouts): `game_hud`, `hud_sprites`, `win_panel` (CS:S's
    `win_round.res` and strings; `game_hud` spawns it), `weapon_select`
    (its `#Cstrike_WPNHUD_*` table), `spectate` (draws CS:S's
    `spectator.res` bars), `radar`, `overview`, `vgui`
    (its `VguiOpen`/`VguiMenu` name CS:S's menus), `hud_text`, `fonts`.
    Next: the HUD as a game part (a HUD slot), then these.
  - [~] Rule modules: `rules::module` (`RuleModule`: name, description,
    its cvars as data, when it applies; `RuleModules` registry; `pick`;
    `chimera_rules`), CS:S's registered by `games/cs_source/rules.rs`
    (`cs_source:deathmatch`, `:bomb`, `:hostage`, `:elimination`,
    picked by `chimera_rounds` and the map's objectives as the code
    already does). The code did not move: `rules::rounds` and
    `objectives` are named by `net` (`RoundState`, `RoundEnded`,
    `frozen_intent`, `Hostage`, `BombHold`), `bot` (bomb and hostage
    goals, `RoundOpen`), `cs_source::movement`/`player_anim`/`pain`
    (`Hostage`), `rules.rs` itself (deathmatch respawns read the round
    phase) and the client's HUD; each needs an interface first (round
    state as replicated data the module registers; objectives behind a
    bot-goal and use-target interface). Not started: game-registered
    replication (`net` still names the round and objective types).
  - [x] (3) `cs_source::CsSourcePlugins` (movement, weapons with the
    bomb and hostages, rule modules, player and view model animation):
    main.rs and `mashup_server` (without the animation, as before) use
    it; tests still pick plugins one by one.
- [x] (3) First crate (2026-10-14): the repository is a Cargo workspace
  (`mashup` at the root, every member a default member) and
  `source/formats` (package `source_formats`, pure data, no Bevy:
  checked by `architecture::workspace_crates`) holds the VFS core, VPK,
  pakfile, LZMA lumps, `.vhv`, `.phy`, WAV/MP3, the detail lump, DMX/PCF,
  `.nav` records, KeyValues (one tokenizer, explicit escapes: P7) and
  localization files; `mashup` re-exports them at their old paths. A
  format edit is checked with `cargo test -p source_formats` in about a
  second (docs/performance.md, "The first crate"). Not moved yet: BSP
  and MDL conversion (they build engine types; vbsp/vmdl do the
  parsing), surface properties, sound scripts and soundscapes (their
  parsing is mixed with the loading), `.res` layout conversion.
- [x] (3) `chimera/core` (package `chimera_core`, 2026-10-14): `core`,
  `console`, `slots`, `character`, `metrics` and `core::version`
  (`format_version`) moved whole; they used nothing above each other,
  so no interface was needed. `mashup` re-exports them at their old
  paths; `mashup::core` (src/core.rs) adds `VERSION`, stamped by
  build.rs in the app's crate so a commit doesn't rebuild the base.
  `movement` (placeholder, noclip) stays: it is a slot implementation,
  `chimera/sim` material. The layering table covers chimera/core/src too;
  `workspace_crates` keeps it off `mashup` and the games. `dev` forwards
  to `chimera_core/dev` (same Bevy build for `cargo test -p chimera_core
  --features dev`). Measured (a new `pub fn`, load 10-40): an edit in
  `chimera_core` checked with `cargo test -p chimera_core --features dev`
  (build and run) in 4-7 s; a leaf edit in `mashup`, `cargo test
  --features dev --no-run`, 127-160 s as before (one crate below it
  less, but `mashup` is still ~160k lines); after an `chimera_core` edit
  the same command took 28-81 s (`mashup` recompiles incrementally).
- [x] (3) `mashup`'s module graph measured (2026-10-14, after the
  engine crates): every `crate::`/`mashup::` path (grouped imports
  split) from each top-level module to another, comments stripped.
  Lines are the module's own; `cs_ui` is `games/cs_source/ui/`, `engine`
  the modules re-exported from the engine crates (core, console, slots,
  character, metrics, map, render, audio).

  | Module (lines) | Uses (references) | Used by |
  |---|---|---|
  | `logic` (24.6k) | engine only (map 141, core 28, console 5) | net 8, client 8, cs_ui 2, harness 1 |
  | `movement` (0.2k) | engine only | net 6, client 1 |
  | `mount` (0.8k) | nothing of ours (source_formats) | cs_source 10, client 6, cs_ui 6, games 1, bins |
  | `greybox` (0.3k) | engine only | client 3, cs_ui 1, bins |
  | `weapon` (7.8k) | engine only (core 64, map 48, console 13) | cs_source 28, client 32, cs_ui 17, bot 15, net 12, rules 11, objectives 8 |
  | `objectives` (2.2k) | weapon 8, engine | cs_source 11, rules 8, bot 7, cs_ui 7, client 5, net 4 |
  | `rules` (2.0k) | objectives 8, weapon 11, engine | client 33, net 28, cs_ui 22, cs_source 2 |
  | `cs_source` (28.3k) | weapon 28, objectives 11, mount 10, rules 2, engine (map 244) | client 13 (4 files, `CLIENT_GAME_EXCEPTIONS`), cs_ui 9, main, bins |
  | `bot` (7.0k) | weapon 15, objectives 7, engine | net 6, cs_ui 5, client 6 |
  | `net` (13.8k) | rules 28, weapon 12, logic 8, bot 6, movement 6, objectives 4, engine | client 41, cs_ui 35, harness 21 |
  | `client` (23.3k) | everything (map 101, console 92, core 77, net 41, rules 33, weapon 32, cs_source 13) | cs_ui 138, main |
  | `cs_ui` (17.9k) | client 138, map 71, console 41, net 35, rules 22, weapon 17, cs_source 9 | main |
  | `harness` (0.5k) | net 21, lib, engine | tests |

  Self-contained layers on the engine crates (no back-reference to
  cut): `logic`, `movement`, `weapon`, `mount`, `greybox`; then
  `objectives` (on weapon) and `rules` (on objectives, weapon). The
  simulation of CS:S (`cs_source` without `ui/`) uses only those and
  the engine: nothing of `net`, `bot`, `client` or `logic`. So, bottom
  first: `logic` to `source/logic` (needs nothing); `weapon` and
  `movement` to `chimera/sim` (nothing); `mount` down to the engine
  (its config's repository path); then `cs_source`'s simulation with
  the CS:S rules (`objectives`, `rules`) to `games/cs_source` (its
  `ui/` stays in `mashup` with the client, re-exported beside it).
  `net` (to `chimera/net`) is blocked by naming CS:S's rules (round
  state, bomb, hostages: game-registered replication) and `logic`
  (Source); `bot` by its bomb and hostage goals; `client` by
  `CLIENT_GAME_EXCEPTIONS` (a game registry).
- [x] (3) `source/logic` (package `source_logic`, 2026-10-14): `logic`
  moved whole (it uses only `core`, `console` and `map`); one
  `pub(crate)` item became `pub`; `mashup` re-exports it as
  `mashup::logic`. The layering table covers source/logic/src;
  `workspace_crates` keeps it off `chimera_present`, `mashup` and the
  games. Spawn dump identical on the five maps modulo moved paths.
- [x] (3) `chimera/sim` (package `chimera_sim`, 2026-10-14): `weapon`,
  `movement` (placeholder, noclip) and `rules::module` (as
  `rule_module`: the `RuleModule` registry, `pick`, `chimera_rules`)
  moved whole; they use only the engine crates. `mashup` re-exports
  `weapon` and `movement` at their old paths and `rules::module` as
  before; `rule_module::plugin` became `pub` (rules adds it). Spawn dump
  identical on the five maps modulo moved paths.
- [x] (3) `mount` down into `chimera_core` (2026-10-14): the VFS and
  KeyValues re-exports, `chimera.local.toml` and finding installs, so a
  game crate can use it; `mashup::mount` re-exports it. `chimera_core`
  now depends on `source_formats` (the VFS, KeyValues) and on `dirs`,
  `toml` and `windows-sys` (moved from `mashup`); the config's
  checkout fallback is the repository root (`CARGO_MANIFEST_DIR/../..`).
  No ECS types, so no spawn dump.
- [x] (3) `games/cs_source` (package `cs_source`, 2026-10-14): the
  game crate. `src/games/cs_source/*.rs` (without `ui/`) moved whole as
  its module `game`, CS:S's rules with it: `rules` (deathmatch, team
  joining, rounds, the end of a game) and `objectives` (bomb, hostages,
  zones) at its root, so their paths and the game's (`super::`,
  `crate::rules`, `crate::objectives`) didn't change; one path
  (`crate::games::cs_source::movement`) became `crate::game::movement`
  and 18 `pub(crate)` items of `vgui`, `gameui`, `hud`, `propdata`,
  `material` (the front end uses them) became `pub`. `mashup` keeps
  `games::cs_source` as `pub use ::cs_source::game::*` with `pub mod ui`
  (the front end stays beside the client shell), and re-exports `rules`
  and `objectives` at their old paths. The format crates the loaders use
  (vbsp, vmdl, vtf, vmt-parser, bzip2, sha2) became workspace
  dependencies. Architecture test: the crate's `game` module is `games`
  in the layering table (`crate::game::` counts as its own game),
  `workspace_crates` keeps it off `chimera_present`, `mashup` and Combat
  Arms. Spawn dump identical on the five maps modulo moved paths; the
  heavy network tests and `cargo test -p cs_source --features dev` (106
  tests) pass.
- Remaining couplings (2026-10-14, what keeps the rest in `mashup`;
  references counted as in the graph above):
  - [x] `net` (done 2026-10-14: `chimera/net`, next entries but one) named CS:S's rules: `rules` 28 (the join
    flow: `JoinChoice`, `join_team`/`join_class`/`join_spectators`,
    `make_unassigned`, `hold_out`, `enter_game`; `Dead`, `Score`,
    `MapLoads`; `rounds::{RoundState, Phase, FunFact, Mvp, RoundEnded,
    ...}` and `frozen_intent`), `objectives` 4 (`Hostage`, `BombHold`,
    `ObjectiveEvent`, `RoundOpen`), `bot` 6 (`Bot`, `BotQuota`,
    `kick_bot`, `sync_quota`) and `logic` 8 (Source: `MapSay`, `HudShow`,
    `LogicOneShot`, carried movers, `set_node_shown`). Proposed
    interfaces: (a) game-registered replication: `net/game.rs` (the round,
    scores, bomb, hostages over the network) moves to the game, which
    registers its replicated components, events and client-side writers
    through an `chimera_net` API (`NetRound` and its kin become the
    game's); (b) a join-flow hook (`JoinRules`: a resource of fn pointers
    or a trait object the game inserts: spawn as the rules allow, join a
    team/class/spectators, hold out while loading) in place of naming
    `rules::*`; `Dead`, `Score` and `MapLoads` move down to `core` (shared
    vocabulary); (c) a prediction hook for the game's holds
    (`frozen_intent`, `BombHold`): a system set the game fills; (d) bots
    as `core::Bot` plus a quota hook; (e) the Source pieces (`MapSay`,
    `HudShow`, carried movers, `set_node_shown`) either move down into
    `map::state` (as `VisClusters` and its kin did) or their replication
    is registered by `source_logic`'s plugin. Each is its own commit
    with the network tests; (a) is the big one.
  - `bot` joined `cs_source` (next entry); an engine bots framework
    (`chimera_sim`) needs the bomb and hostage goals, buying and the
    radio behind a bot-goal interface first.
  - [x] `client` named `cs_source` in 4 files (`CLIENT_GAME_EXCEPTIONS`:
    the default game's install, map cycle, tick, binds, the movement
    cvars). Done (2026-10-14): `chimera_core::game_registry`: a game fills
    a `GameInfo` (`static`: id, short name, its folder in the install,
    tick, `open`/`map_cycle`/`import_map` as `fn`s, its default binds'
    file and built-in table, movement cvars as data) and registers it
    (`RegisterGame`, first in `CsSourcePlugins`); `Games` (a resource)
    keeps them in order, the first is the default game. The console's map
    commands, the map list and cycle, the downloads' folder, a loaded
    map's tick (its game's), the bind defaults (`binds::GAME_DEFAULTS`,
    read at start from `GameInfo`) and the debug UI's movement tab read
    it; CS:S's bind table moved to `cs_source::game::DEFAULT_BINDS`. The
    exceptions are gone; the architecture test forbids the shell any path
    naming a game (`cs_source::` too, not only `crate::games::`).
  - [x] The front end (`src/games/cs_source/ui`, 17.9k) into
    `games/cs_source` behind a feature (done 2026-10-15: (b)-(d) below;
    it was blocked: It needs, besides the
    engine crates and CS:S's own: `net` (35 references: `net::client`,
    `NetSettings`, `NetCharacter`, `score_flags`, `query` (the server
    browser), `motd`, `maps`, `RadioRequest`, `NetScore`, `NetEvent`),
    which named CS:S's rules and sat in `mashup` until `chimera_net`
    (game-registered replication; now done: `cs_source` may use `net`,
    with CS:S's replicated types in `cs_source::game::net`); and nearly the whole shell
    (138 references: `client::console` 29 (`ConsoleUi`, `map_names`),
    `vgui` 26, `fonts` 16, `input` 9, `chat` 9, `spectate` 8, `binds` 8,
    `hud` 5, `front_end` 5, `widgets`, `video`, `radar`, `perf`,
    `audio`, `ClientArgs`, `FirstPersonCamera`, `shown_name`,
    `zoom_camera`), whose files reach each other and `net` (`console`
    uses nearly every other; `input` uses `chat`, `debug_ui`,
    `spectate`, `vgui`, `weapon_select`). Every file of `ui/` uses
    `game_menu`, and `game_menu` uses `net` and `client::console`, so no
    part of it could move first).
    - [x] (2026-10-14) The shell's base as a crate: `chimera/client`
      (package `chimera_client`, module `client`): `front_end`, `fonts`,
      `hud_text` (its shader at `embedded://chimera_client/client/`),
      `widgets`, `binds` and `vgui`'s drawing moved whole; they use only
      `chimera_core` and `chimera_world` (no `chimera_present`, no `net`).
      `mashup::client` re-exports them at their old paths; its own
      `vgui` re-exports the base's and adds what needs the console and
      chat (the mouse grab while a game-look menu is open, `keys_live`),
      its `VguiPlugin` adding the base's first (system order kept).
      `binds::test_binds` and `SAMPLE_CONFIG_DEFAULT` are no longer
      `cfg(test)` (the front end's tests use them across the crate
      boundary). `workspace_crates` keeps the crate off
      `chimera_present`, `chimera_sim`, `source_logic`, replicon,
      `mashup` and the games; the server build check lists it as
      client-only. Main menu and buy menu screenshots on de_dust2 are
      pixel-identical before and after, the scoreboard differs in 4
      pixels (bots). Measured (docs/performance.md, "`chimera_client`"): a
      leaf edit's `cargo test --features dev --no-run` 18-20 s in the
      front end, 36-64 s in the shell (was 77-78 s), 15-37 s in
      `chimera_client`; `cargo test -p chimera_client` 2 s.
    - Left, in order: (a) `net` to `chimera_net` (another session); (b)
      the shell's references to CS:S's rules and bots (`rules` 33:
      `JoinChoice`, `Dead`, `RoundState`/`Phase`, `MapCycle`,
      `new_game`, `game_end::clock`; `objectives::bomb` in `radar` and
      `game_hud`; `bot` in the debug UI and console) through the game
      registry or `core` (`Dead`, the round phase as shared vocabulary);
      (c) the CS:S-flavoured HUD still in the shell (`game_hud`,
      `win_panel`, `weapon_select`, `spectate`, `radar`, `overview`,
      `hud_sprites`) to the front end, or a HUD slot; (d) then the rest
      of the shell to `chimera_client`, and `ui/` to `cs_source` behind a
      `ui` feature depending on `chimera_client` (`workspace_crates`
      keeps `chimera_present` off `cs_source` today: the feature would
      need that rule relaxed for the feature's dependencies only).
    - [x] (b) (2026-10-15) The shell names none of CS:S's rules, bots or
      objectives outside the HUD files of (c), nor the app or Source's
      logic: `Dead`/`Score` from `core`; `JoinChoice`, `MapCycle`,
      `FadeToBlack` moved down to `core` (re-exported by `rules`); the
      rules' `new_game` behind `core::NewGame` (the rules insert it,
      `core::new_game` calls it); bots by `core::Bot`, hostages by
      `core::Npc`; the debug UI's Bots and Rounds tabs through
      `debug_ui::GameDebug` and the bots' overlays (`chimera_drawbots`,
      `bot_debug`) in the front end (`ui::debug`); the app's map loading,
      greybox and commit through `client::AppHooks` (mashup's
      `client_hooks`), its version through `core::BuildVersion`;
      `DEFAULT_TICK_HZ` in `core`; the logic's HUD messages, fades and
      shakes as data in `map::logic_view` (re-exported by `logic::hud`)
      and its entities for tools through `map::logic_view::LogicInspect`.
      Architecture test: `client` lost `lib`, `greybox`, `games`, `bot`
      and `logic`.
    - [x] (c) (2026-10-15) CS:S's HUD to its front end: `game_hud`,
      `win_panel`, `weapon_select`, `radar` (with its shader), `overview`,
      `hud_sprites` moved whole to `src/games/cs_source/ui/`; spectating
      split: the state and camera stay the shell's (`client::spectate`),
      the bars and plain panel are `ui::spectator_hud`; the selection's
      state the shell's input reads is `client::weapon_menu::WeaponMenu`
      and `additive_as_alpha` is `client::hud`'s. The shell is a plugin
      group (`client::ClientPlugins`: `ShellSetupPlugin` first,
      `ShellCameraPlugin` last); the front end puts its HUD plugins where
      they were (`ui::with_hud`, `PluginGroupBuilder::add_after`), so the
      game's system order (`CHIMERA_SYSTEM_DUMP`) is the same modulo paths
      but for the bots' overlays, which now run after the ragdolls'
      (gizmos only). Screenshots (main menu, options, buy, team menu,
      scoreboard, HUD with weapon selection, spectator bars and overview,
      debug UI tabs, de_dust2, 1280x720) match before and after within
      the run-to-run noise (the version label aside). Architecture test:
      `client` lost `rules` and `objectives`.
    - [x] (d) (2026-10-15) The rest of the shell into `chimera_client`:
      every file of `src/client/` moved whole (shaders at
      `embedded://chimera_client/client/`); the shell's `vgui` (the mouse
      grab, `keys_live`) joined the base's (one `VguiPlugin`, its systems
      in the same order). Decided: one crate, base and shell together,
      on the presentation and the network (its picture settings are
      render passes, its sound settings the mixer's, its glue the
      network's): nothing uses the base without the shell (a game's front
      end needs both), so a split bought no compile time; it still names
      no Source logic, app or game (`workspace_crates`: no
      `source_logic`, replicon, `mashup` or game). `mashup::client`
      (src/client.rs, app glue: counted as the crate root's) re-exports
      it, with `ClientPlugin { args }` (the shell's group with
      `client_hooks` and CS:S's HUD), CS:S's HUD modules where the shell
      had them, and `spectate` with the spectator's HUD. Three
      `pub(crate)` cvar registrations became `pub`, `crc32fast` a
      dev-dependency (only a test uses it), `profile` forwards to the
      crate.
    - [x] (d) (2026-10-15) CS:S's front end into `cs_source` behind its
      feature `ui` (`games/cs_source/src/ui/`, moved whole; paths
      `crate::games::cs_source::` became `crate::game::`, the radar's
      shader `embedded://cs_source/ui/`): the feature turns on the shell
      (`chimera_client`), the presentation, Source's logic (the
      objectives' HUD reads the map logic), replicon's netcode client
      (the loading dialog), the clipboard and winit, all optional;
      `mashup`'s `present` turns it on, so the dedicated server
      (`--no-default-features --features server`) builds without it.
      `mashup::games::cs_source::ui` re-exports it. Architecture test:
      a game crate's `ui` is `games_ui`, its parts may not use
      `crate::ui::`; `workspace_crates` keeps `chimera_present`,
      `chimera_client` and `source_logic` off `cs_source` but as optional
      dependencies only `ui` turns on (`UI_FEATURES`), and
      `game_ui_only_with_its_feature` checks `cargo tree -p cs_source`
      without and with the feature. `mashup` is now the app's glue: the
      binaries, the harness, `SimPlugins`/`swap_map`, `games::load_map`,
      the greybox, `client_hooks`, `mashup::client` (re-exports and
      `ClientPlugin`) and re-exports at the old paths.
- [x] (3) `bot` into `cs_source` (2026-10-14): CS:S's bots (their
  buying, radio, bomb and hostage goals) moved whole to
  `games/cs_source/src/bot` (it uses only `weapon`, `objectives`, `map`
  and the base); `mashup::bot` re-exports it. `mashup` keeps the client,
  its front end (`games/cs_source/ui`), `net`, `harness`, `greybox`,
  `games::load_map` and the binaries.
  Measured (docs/performance.md, "`chimera_sim`, `source_logic` and
  `cs_source`"; a new `pub fn`, load 9-32): `cargo test --features dev
  --no-run` after a leaf edit in `cs_source` 22-27 s (was 127-176 s),
  in `chimera_sim` or `source_logic` about 20-28 s, in CS:S's front end
  (still `mashup`) 18-41 s, in the client 77 s; `cargo test -p` builds
  and runs `chimera_sim` in 2 s, `source_logic` in 2-3 s, `cs_source` in
  3-4 s.
- [x] (3) `chimera/net` (package `chimera_net`, 2026-10-14): `net` moved
  whole and names no game or Source logic; (a)-(e) above as proposed:
  - (a) Parts: a game or Source's logic registers a function by name
    (`net::NetAppExt::net_part`; `net::NetParts`), which
    `NetPlugin::finish` calls in name order after the engine's protocol
    (after every plugin's `build`, so the order an app adds its plugins
    in doesn't matter; before replicon's `finish`, which seals the
    protocol). A part registers its replicated components and messages
    (with `ordered`, `interp::write_snapshot`, `interp::hold_heard`),
    its systems on both sides and its hooks. CS:S's
    (`cs_source::game::net`, `CsSourceNetPlugin`, added by `SimPlugins`):
    `NetRound`, `NetBomb`, `NetHostage`, `TeamRequest`, `ClassRequest`,
    `ObjectiveNews`, `RoundOver` and their systems (was most of
    `net/game.rs`; scores, buying, the rules' sounds and the client's own
    money stay the engine's). Source's logic's (`logic::net`, added by
    `LogicPlugin`): `NetMover` (was `net/movers.rs`) and `MapHud`.
  - (b) `net::JoinRules` (fn pointers: a joined player held out and put
    where it asked, entering when its client has the map, a client's
    choice as userinfo), filled with `rules::JoinChoice`, `hold_out`,
    `make_unassigned`, `join_spectators`, `enter_game`; `Dead`, `Score`
    and `MapLoads` moved down to `core` (re-exported by `rules`).
  - (c) `net::Holds` (the freeze's end for `OwnState::frozen_until`,
    `frozen_intent`) and `net::predict::RecordCommand` (a set: CS:S
    orders `BombHold` before it).
  - (d) `core::Bot` (CS:S's `bot::Bot` requires it) and `net::BotHooks`
    (kick with the quota one lower, `sync_quota`); hostages require
    `core::Npc` (not players: not counted, replicated by their game);
    the bomb carrier's scoreboard flag is the weapon's `core::ScoreMark`
    (C4 requires it), so `NetScore` stays the engine's.
  - (e) `LogicOneShot` and `MapSay` moved down to `map::state` (the
    network forwards them without the logic); movers and HUD messages
    are the logic's part (`logic::net`; a client's replayed ticks place
    the movers through `net::NetHooks::replay`, since 2026-10-31 the
    `core::Predict::PlaceMovers` stage; stopping serving takes
    the parts' components off through `NetHooks::stopped_serving`).
  The registration order changed (the parts last): `NET_VERSION` net24
  (multiplayer.md). `source_logic` now depends on `chimera_net` (and so
  on `chimera_sim`). `mashup::net` re-exports `chimera_net::net` with the
  parts' types at their old paths; `version_text` takes the build
  (`core::BuildVersion`, which `SimPlugins` inserts). Architecture test:
  `net`'s row lost `objectives`, `rules`, `bot`, `logic`; `logic` and
  `games` may use `net` (games also `bot`); `workspace_crates` keeps
  `chimera_net` off `chimera_present`, `source_logic`, `mashup` and the
  games. Spawn dump identical on the five maps modulo entity ids, moved
  paths and two new resources (`NetParts`, `BuildVersion`); the network
  tests and the heavy `net_*`/`map_net_*` pass. Measured
  (docs/performance.md, "`chimera_net`"; a new `pub fn`, load 17-19):
  `cargo test --features dev --no-run` after a leaf edit in `chimera_net`
  27-32 s, in the client 19-23 s; `cargo test -p chimera_net --features
  dev` builds and runs its 29 tests in 6 s.
- [x] (3) Chimera, apps, layer tools, workspace-hack (2026-10-15, one
  session; user decisions of the day): `engine/` became `chimera/`
  (crates `chimera_*`); the root package's app glue moved to
  `apps/cs_source` (`cs_source_app`: the game binary `mashup`, the
  workspace's `default-run`; `SimPlugins`, the harness, the greybox,
  `games::load_map`, `client_hooks`, build.rs), the dedicated server to
  `apps/cs_source_server` (`mashup_server`, the app without `present`:
  `cargo build --release -p cs_source_server`); `mashup` is a thin shim
  (`cs_source_app` at the old paths, for tests/it and `dump`). Tools by layer: `chimera_tools` (the perf gate as
  a library, `imgcmp`, `tracesum`), `source_tools` (RCON, `demodump`,
  `testmap`), `cs_source_tools` (refcmp's CS:S driver, movecmp,
  mapsweep, perfgate's scenarios; on the app's harness, tech-debt.md).
  `chimera/workspace-hack` (`cargo hakari`) with Bevy's development
  features in `chimera_dev` and `[profile.dev.build-override] debug =
  false`: the first `cargo test -p chimera_world --features dev` after a
  warm workspace build went from 560 s to 3 s (performance.md,
  "Workspace-hack"). Binary names users run are unchanged.
- [x] Tests in their crates, affected-test selection (2026-10-15, one
  session): the root's one test crate split by the lowest crate whose
  API each file needs, one test binary per crate (`<crate>/tests/it/`,
  `heavy::` kept): `cs_source_app` the scenario tests on its harness and
  greybox and all but five heavy files (78 files, 706 tests);
  `cs_source` its loaders' and spec tests (tests/it/, 80 tests: three
  heavy map files) and its front end's (tests/ui/ with `ui`, 8 tests);
  `chimera_present` its spec cases (10); the root `dump`'s test; the
  architecture tests in a new plain crate, `repo_checks` (checks/), with
  `affected`: changed files to packages, then every package whose tests
  depend on them (dev-dependencies too) through `cargo metadata
  --all-features`; a test file's change selects its module; the
  lockfile, workspace manifest, .config/, hooks or a build.rs select
  everything, docs only `repo_checks`. The hooks run that selection
  (pre-commit: staged files, fast tier through `cargo test -p`;
  pre-push: each pushed ref against the remote's, nextest with heavy
  tests; `CHIMERA_PUSH_TESTS=all`/`0`). 1616 tests before, 1624 after (8
  unit tests of the selection; three renamed). Measured (warm, load from
  other work): full suite 846 s; docs-only push 8 s, a CS:S tool's edit
  8 s, one heavy test file's edit 101 s (41 tests), a `cs_source` front
  end edit 847 s (1064 tests: the app's), a `chimera_core` edit 865 s (1588 tests); the
  pre-commit fast tier 4 s for a test file, 82-91 s for a crate under the
  app.
  What it shows: the heavy tests are the app's (its harness runs the
  app's `SimPlugins`), so any change under the app still runs nearly
  all of them; next for shorter pushes: the greybox and a `Sim` taking
  its base plugins below the app, so tests (heavy ones above all) can
  sit on the layer they test (tech-debt.md).
- [x] Harness below the app (2026-10-15, one session): `Sim` takes its
  simulation plugins as a base and lives in `chimera_world` (`harness`,
  with the greybox), `NetSim` in `chimera_net`; `chimera_sim::
  SimBasePlugins` is the engine's base, `cs_source::SimPlugins` CS:S's
  simulation (the old app set: the base, its rules, objectives, bots,
  network part and Source's logic, so `cs_source` depends on
  `source_logic`), `cs_source::harness::CsSim` that as the tests' and
  tools' base, with its test maps; `swap_map` is the game's. The app's
  `SimPlugins` adds its build's version and keeps the old paths. The
  app's 711 tests moved, names unchanged: 623 to `cs_source`'s `it`
  (the heavy map and network files; tests/common/stack.rs re-exports
  what they use at the app's paths), 73 to its `ui` (state of the
  shell or front end), 9 to `chimera_sim`, 3 to `chimera_present`, 2 to
  `chimera_client`; the app keeps one (its version). `cs_source`'s tests
  may use the presentation (`MapPlugin`; architecture.rs `TEST_ONLY`).
  `affected` learned feature folders (cs_source's `src/ui/` runs its
  `ui` tests and the crates above). 1643 tests before, 1644 after (one
  selection test). Measured (pre-push selection, warm, binaries built):
  full suite 826 s; a `cs_source` front end edit 48 s (217 tests; was
  847 s), an app edit 2 s (23; was most of the suite), a `cs_source`
  simulation edit 823 s (1051), a `chimera_world` edit 818 s (1568), a
  `source_logic` edit 820 s (1268).
  What it shows: edits above the game are cheap now; an edit to the game
  or anything under it still runs its heavy map tests, which are
  `cs_source`'s because its loader makes the maps. Next for shorter
  pushes there: a Source map loader below the game (`source/`), so map
  tests that need Source's world but no CS:S rule can sit in
  `source_logic` or a Source crate.
- [x] Data-driven weapons (2026-10-15, one session; docs/weapon-data.md):
  a weapon is a TOML document (`chimera_sim`'s `weapon::data`: stats,
  trigger, magazine and ammo types, hitscan, damage, penetration, melee,
  scope/silencer/burst, sounds, models, HUD names, idle timing, prices
  and bots' weights; `base` templates; units per document) built into
  the parts at run time; behaviour data can't express is a hook a game
  registers by name (`register_weapon_hook`; unknown names and bad
  parameters are load errors). CS:S's 29 weapons (24 guns, knife, three
  grenades, C4) are `games/cs_source/data/weapons/*.toml` with six hooks
  (accuracy, recoil, hit slowdown, the Elites' hands, grenades, the C4);
  the `GUNS`/`Gun`/ammo/model/weight/price tables are gone. Each weapon's
  parts were compared with the old tables' component by component before
  they went; the weapon tests pass unchanged in their expectations; a gun
  defined only in a test's data file is bought, fired and played over the
  network (`weapon_data.rs`). Next: the buy menu layout and the spawn kit
  as data, then rule modules' settings, HUD layouts, minigames.
- [x] Data-driven shop and penetration (2026-10-15, one session;
  docs/economy-data.md): an economy document (`chimera_sim`'s
  `weapon::economy_data`) gives the items that aren't weapons
  (`ShopItem`: armour, or a part registered by name in `ItemParts`; the
  buying rules find them by what they give, not by CS:S's names), the
  buy menu's categories and order, the spawn kit per team, the shop's
  sounds and buy aliases; penetration by surface material is a document
  too (`PassMaterials::parse`), and CS:S's round numbers
  (`RoundSettings`) are read with `rule_module::parse_settings`. CS:S's
  are `games/cs_source/data/{economy,penetration,rounds}.toml`; its
  `prices`/`menu`/`pass_materials` tables and spawn-kit code are gone
  (compared with the loaded data before they went). The economy, buy
  menu and penetration tests pass unchanged in their expectations; an
  item and a menu category defined only in a test's data file are
  bought (`economy_data.rs`). Next: bots' buying thresholds and the
  default autobuy/rebuy lists, rule modules' other settings, HUD
  layouts, minigames.
- Order: (1) the `tools` package; (2) inside today's single crate, move
  code to its future home (the `map` sort, CS:S UI out of `client`,
  rules behind a rule-module interface, game-registered replication),
  each move-only with the architecture test updated; (3) cut crates
  bottom-up, one per commit with the full suite; (4) minigame
  definitions, the existing modes as the first rule modules.

Test selection (user, 2026-10-14): no coverage-based test selection;
the crate split (with each crate's own tests and cargo's crate graph to
pick affected tests) is the way to shorter pushes. Since 2026-10-15
pushes run the affected crates' tests (`repo_checks`' `affected`).

## Minigames (the middle layer)

- A minigame is a data file picking pieces by id from any game (map,
  character, movement, weapons) plus a rule module by name:

  ```toml
  name = "Gun Game: Dust II"
  map = "cs_source:de_dust2"
  movement = "cs_source:movement"
  weapons = ["cs_source:weapon_*"]
  rules = "originals:gungame"
  players = "2-16"
  ```

- Rule modules (round flow, scoring, win conditions) are Rust code that
  games register by name; definitions load at run time, so new minigames
  need no rebuild. The existing modes (deathmatch, bomb and hostage
  rounds) become the first rule modules.
- No native dynamic plugins (Rust has no stable ABI; a Bevy dylib must
  match the compiler and Bevy version exactly). If rule code must ship
  without a rebuild later, scripted rule modules (WASM or Lua) behind the
  same interface.

## Repo

- One monorepo holding everything, developed in the open: engine,
  Source, games, minigames, both apps, specs and docs.
- Combat Arms (loader and specs) is developed locally and git-ignored for
  now, and was removed from the history (2026-10-07). Since 2026-10-18 its
  specs and crate are public again, clean room ([combat-arms.md](combat-arms.md));
  its `.rez` reader is `lithtech_formats` (lithtech/formats).

## Developing in the open

- Never commit game assets, extracted files, keys, or personal traces
  (names, personal emails, machine paths, Discord ids); commit as
  `r0pi0x` in UTC (CLAUDE.md, "Commit identity").
- Licence: MIT OR Apache-2.0 for the code (LICENSE-MIT, LICENSE-APACHE);
  original game assets may get their own licence per folder later.

## Order

1. [x] New account and identity: history rewritten to `r0pi0x` (noreply
   email, UTC timestamps), `r0pi0x/LuckerParty` created (private then, public now),
   `origin` re-pointed (2026-10-07). Left: Windows re-clones; the old
   repo is deleted by the owner when satisfied.
2. [ ] Cargo workspace, one crate at a time, tests passing at each step:
   `engine` out of `src/` first, then `source` (the Source formats and
   logic out of `games/cs_source`, `logic`, parts of `map`), then
   `games/*` and `apps/*`. Sort every module first (table below), then
   move. [architecture-review.md](architecture-review.md) (P9) measured
   it: one `engine` crate at the bottom gives no iteration gain (most
   commits touch `core` or `map`); a finer engine split after sorting
   `map` does.
3. [ ] Minigame definitions and rule modules; the existing modes as the
   first definitions.
4. [ ] Networking ([multiplayer.md](multiplayer.md)); the Lucker Party app (lobby, party flow, rotation,
   launcher).

## Module sorting (to fill in at step 3)

| Module | Goes to | Notes |
|---|---|---|
| `src/core.rs`, `console.rs`, `slots.rs`, `harness.rs` | engine | done: `chimera/core`; `harness.rs` split by layer: `Sim` in `chimera/world`, `NetSim` in `chimera/net`, CS:S's base in `games/cs_source` |
| `src/logic` | source | done: `source/logic` (`source_logic`) |
| `src/map` | split | neutral runtime (MapData, culling, particles) vs Source materials/shaders (world.wesl, water); audio already out (`src/audio`) |
| `src/games/cs_source` | split | simulation with `rules`/`objectives` done: `games/cs_source` (crate `cs_source`); `ui/` still in `mashup`; formats (bsp, material, mdl, sound) to source later |
| `src/games/combat_arms` | games/combat_arms, lithtech/ | the `.rez` reader done: `lithtech/formats` (`lithtech_formats`); the game crate from specs |
| `specs/` | split | `source/` to source (after review), `cs_source/` to cs_source, `combat_arms/` in the repo since 2026-10-18 ([combat-arms.md](combat-arms.md)) |
