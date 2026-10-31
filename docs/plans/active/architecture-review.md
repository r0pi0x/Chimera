# Plan: architecture review and refactoring

Status: second pass 2026-10-20, on main at 1cdaf890 (the first pass,
2026-10-13, approved then, is summarised in section 1 and its full text
is in this file's git history). The second pass reviews the workspace
against the long-term goals: Chimera as an engine for faithful
first-person recreations (CS:S and Combat Arms first), each game
playable alone and usable as a library; Lucker Party as the end goal
(minigames and mashups mixing parts of several games, then original
minigames); data and registered hooks over hard-coded Rust; multiplayer
with client prediction; Linux and Windows.

Method: read-only. Line counts, function lengths, ECS pattern counts,
`cargo metadata`, `git log`; no builds (the dev box was busy), so every
time below is either quoted from docs/performance.md with its date or
marked as an estimate with what to measure instead. Counts exclude
tests unless they say so. The survey scripts are throwaway (section 2,
end); keeping them under `tools/` is item N6's.

## 0. Summary

The crate split worked: 23 crates, the layering checked by the compiler
and `repo_checks`, leaf edits test-built in 14-30 s, and Combat Arms
went from nothing to 40k lines plus 25k of tests since 2026-10-18
without a layering violation. The first pass's structural proposals are done.
What the code does not have yet is the thing Lucker Party needs most:
**two games' gameplay in one running app, chosen at run time**. Today
an app is a compile-time composition in which the first registered game
is "the" game (its tick, binds, HUD), each game's simulation plugin set
brings its own copy of the engine's base, and most of a game's systems
run whenever its plugins are added. The mashup demo works around this
by not adding CS:S's simulation at all, which is why a CS:S map's
doors, buttons and breakables stand as placed in it and CS:S's guns
can't be offered.

The second theme is state with no owner: `MapData` (73 fields) stays
resident for a map's whole life, map unloading is a hand-kept list of 25
resources, a mashup strips another game's parts with another hand-kept
list, and about 40 process-wide statics (weapon sets leaked by design,
Combat Arms' data, effects and model caches) sit outside any `World`.
Each is a small bug source today and a blocker for a server hosting
several matches, for hot-reloading data and for memory once two games'
content share a session.

The third theme is determinism as composition grows: tests run Bevy's
multi-threaded executor while the game and servers run single-threaded
ones, no simulation schedule is checked for ambiguous order, the client
and the server build different plugin sets, and Windows-to-Linux
prediction has never been measured.

Recommendations, in priority order (sections 3-8 have the evidence and
migration of each; section 11 orders them for parallel agents):

| # | Recommendation | Section | Size | Phase |
|---|---|---|---|---|
| 1 | Games as parts without the engine base; the gameplay a run-time slot gating each game's match-wide systems | A1 | M | now, next |
| 2 | Determinism guards: tests on the production executor, an ambiguity ratchet on the simulation schedules, server and client order compared | E1 | S-M | now |
| 3 | Map lifetime: map-scoped resources registered once, `MapData`'s bulk dropped after spawning, then the world split from the gameplay's kit | A2, F2 | M | now, next |
| 4 | Lean map loads: load options (no textures for servers and tests), CS:S texture decode on threads | F1 | S-M | now |
| 5 | Process-wide statics into resources or assets, and a check against new ones | E2 | M | next |
| 6 | Source-shaped vocabulary out of Chimera: `OwnState` parts, named `Intent` actions, Steam table and map files in `GameInfo`, units | A3 | M (slices) | next |
| 7 | A mode lifecycle lifted from Combat Arms' `CaMode`, then minigame definitions as data on it | D2 | L | later |
| 8 | Party networking: a match manifest in the handshake, the mount doctor, one app per match | M1-M4 | L | later |
| 9 | Engine contract tests on a Chimera test kit; CI on Linux and Windows without installs | T2, T3 | M | now, next |
| 10 | Docs that stop colliding: debt and module indexes generated from `//!` docs, finished briefs archived | T5 | S | now |

## 1. The first pass (2026-10-13): what became of it

| P | Proposal | Status (2026-10-20) | Evidence |
|---|---|---|---|
| P1 | Split `map/mod.rs`, `spawn_map` in phases | **done** | `chimera/world/src/map/mod.rs` 368 lines, `map/spawn.rs` 872, drawn phases in `chimera/present/src/render/spawn.rs` (1,901); the spawn dump tool kept (`heavy/map_spawn_dump.rs`) |
| P2 | Docs diet: one line per module and debt | **done, regressing** | ARCHITECTURE.md 133 KB to 22 KB on 10-13, 62 KB now; tech-debt.md 15 KB then, 34 KB now; both still top conflict files (2.6); T5 |
| P3 | Sort `map` and `client` into modules | **done** (became crates) | `chimera_present` (render, audio), `chimera_client`, `chimera_world`; `simulation_builds_no_drawn_parts` checks it |
| P4 | Test support module | **partial** (X8: `sim()` and readers lifted where identical) | `games/cs_source/tests/common/` exists; still 45 `fn sim(`, 24 `fn load(`, 12 `fn feet(`, 10 `fn active_id(` across test files; T4 |
| P4b | Heavy tier: measure, split, more slots, cache | **steps 1, 3, 4 done; 2 open** | full suite 1,812 s to 1,009 s (10-14), then 1,757 s with Combat Arms (10-19); the lean load path is in (N3, 10-21: servers' and tests' memory halved, time saved per load 0.4-1.3 s); no cache yet; 2026-10-30: 940 s heavy wall with the installs, a sixth slot -15% but out of memory (kept at five), the cache designed (asset-service.md section 10), not built |
| P5 | Game composition as plugin groups | **partial** | `CsSourcePlugins`, `CombatArmsPlugins`, `CsSim`, `CaSim` exist; the materials (`chimera_present::PresentationPlugins`) and the physics (`chimera_world::physics_plugins`) are one group each since N7; the tick and log layer still per app; A4 |
| P6 | One VGUI dialog layer | **partial** (model 10-24, L6) | `chimera_client::client::dialog` (message, confirm, input/password) is shared by Combat Arms' lobby and CS:S' server browser; the 7 UI scale clamps and a shared `Hit<T>` stay open, and each game still draws its own popups |
| P7 | One KeyValues tokenizer, one binary reader | **done** (reader 10-24, L6) | `chimera_vfs::bytes::Le`; `nav`, `pcf`, `vpk`, `ltb` and `world` use it; `net::query`'s 3-method `Option` reader stays (chimera_net does not depend on the VFS) |
| P8 | Split the giant systems | **open, grown** | `bot::think` 696 lines (was 667), Combat Arms' `movement::step` 634 (new), `apply_effects` 439; E3 |
| P9 | The crate split along fine lines; cheap wins first | **done**, beyond the plan | 23 crates; tools behind `tools`; mold and `line-tables-only` still unmeasured on a full cycle |

The first pass's "not worth doing" list still holds (section 10).

## 2. Facts (main at 1cdaf890)

### 2.1 Size

Rust lines per crate (non-test code; tests in `tests/` and inline
`#[cfg(test)]` modules separately; `#[test]` attributes):

| Crate | Code | Inline tests | Test files | `#[test]` |
|---|---|---|---|---|
| `cs_source` (+ `ui`) | 61,366 | 6,077 | 52,537 | 1,138 |
| `combat_arms` (+ `ui`) | 40,115 | 430 | 25,123 | 841 |
| `chimera_client` | 20,451 | 2,713 | 132 | 115 |
| `chimera_present` | 18,842 | 3,207 | 1,265 | 130 |
| `source_logic` | 18,828 | 183 | 7,200 | 226 |
| `chimera_world` | 18,107 | 1,602 | 0 | 56 |
| `chimera_net` | 13,918 | 813 | 0 | 37 |
| `chimera_sim` | 10,507 | 888 | 147 | 39 |
| `lithtech_formats` | 5,850 | 213 | 3,888 | 110 |
| `chimera_core` | 5,636 | 536 | 0 | 22 |
| `source_formats` | 5,554 | 1,295 | 0 | 42 |
| tools, apps, checks, shims (10 crates) | 8,736 | 538 | 1,646 | 44 |
| **total** | **227,919** | **18,495** | **91,938** | **2,800** |

Plus 1,976 lines of WGSL, 39k lines of specs, 1.17 MB of docs.
Growth since the first pass: its 165k lines of `src/` (inline tests
included) are 246k now (228k of code, 18.5k of inline tests) a week
later; Combat Arms is 40k of the code.

Functions: 6,862; 3 over 400 lines, 39 of 200-400, 181 of 100-200.
185 take 8 or more parameters (35 take 12-16, Bevy's limit is 16);
170 `too_many_arguments` allows, 17 `#[derive(SystemParam)]`. Largest:
`cs_source::bot::think` (696 lines, 16 params,
games/cs_source/src/bot/mod.rs:1122), `combat_arms::movement::step`
(634, games/combat_arms/src/game/movement.rs:863),
`logic::bridge::apply_effects` (439), `logic::classes::class_input`
(394), `cs_source::bsp::load_level_bytes` (372),
`client::console::client_commands` (335), `weapon::weapon_frame` (324).

### 2.2 Crate graph

```
apps:     cs_source_app <- cs_source_server; combat_arms_app; combat_arms_server; mashup_demo
games:    cs_source (+ui)              combat_arms (+ui)
loaders:  source_logic   source_formats   lithtech_formats (uses source_formats' VFS)
chimera:  chimera_client -> chimera_net, chimera_present, chimera_sim, chimera_world, chimera_core
          chimera_net -> chimera_sim -> chimera_world -> chimera_core -> source_formats
```

From `cargo metadata` (23 members, 790 packages in Cargo.lock):

- Every Chimera crate's closure contains `bevy_render` and `wgpu`
  (631 crates for `chimera_core` alone): the workspace's `bevy` has its
  default features, so the dedicated servers still compile Bevy's
  render, window and audio crates (engine-split.md lists the blockers).
  (Done 10-22, L6: the servers' trees have none of them; 689 to 495
  packages for `cs_source_server`.)
- `chimera_core` depends on `source_formats` (the VFS and KeyValues
  for `mount`), and `lithtech_formats` on `source_formats` for the VFS:
  the engine's base and the LithTech loader both sit on a Source crate.
- `source_logic` depends on `chimera_net` (its replicated movers and
  HUD messages), so Source's map logic can't be used without the
  network crate.
- `cs_source_tools` depends on `cs_source_app` (tech-debt.md).
- Both games' `ui` features pull `chimera_client` and `chimera_present`
  as optional dependencies; their simulations don't (checked).

### 2.3 Where edits land (since 2026-10-15)

261 non-merge commits on main, 186 touching Rust. The lowest crate a
commit touches decides what rebuilds and which tests a push runs:

| Lowest crate touched | Commits | Share |
|---|---|---|
| `chimera_world` | 36 | 19% |
| `chimera_sim` | 20 | 11% |
| `chimera_core` | 15 | 8% |
| `lithtech_formats`, `source_formats` | 15 | 8% |
| `source_logic`, `chimera_net`, `chimera_client`, `chimera_present` | 34 | 18% |
| a game (`cs_source`, `combat_arms`, their `ui`) | 40 | 22% |
| the rest (tests, tools, apps, checks, the root shim) | 26 | 14% |

38% of Rust commits start at Chimera's bottom three crates: the
"lift into Chimera at the second use" workflow (combat-arms.md) puts
the busiest work where every edit rebuilds and retests everything.
Most-edited files: docs (2.6), then `games/combat_arms/src/game/mod.rs`
and `tests/it/main.rs` (22 each: registration lists every module adds a
line to), `cs_source` `bsp.rs` (21), `Cargo.lock` (18), `core.rs`,
`weapon/mod.rs`, `weapon/grenade.rs`, `logic/bridge.rs` (16 each),
`map/data.rs` (15: `MapData`'s fields).

### 2.4 ECS by the numbers

| What | Count | Note |
|---|---|---|
| `add_systems` calls | 396 | 83 `in_set`, 121 `before`, 197 `after`, 86 `chain`, 169 `run_if` |
| system sets | 23 | core: `SimSet`, `Predict` (schedules), `InterpSystems`, `WeaponFrame`; `LogicSet` lives in `source_logic` |
| resource types | ~470 | largest: `MovementData` 45 fields (CA), `NetGraph` 38, `SourceMovementConfig` 36, `GameMenu` 35 |
| component types | ~460 | largest: CS:S `Bot` 62 fields, `CaBot` 46, `CaMovement` 39, `SourceMovement` 30, `Intent` 15 |
| message types / `add_message` | 111 / 172 | observers: 21 (`On<>` 15) |
| `Query` params | 1,068 | 21 with 8+ components (movement steps 12 and 11) |
| `Option<Res<_>>` params | 627 | most: `ActiveHud` 41, `SoundBank` 15, `NetRole` 14 |
| change detection | 33 `Changed<>`, 15 `Added<>`, 52 `is_changed()`, 83 `set_if_neq`, 18 `bypass_change_detection` | the bypasses are avian wake-up workarounds (performance.md, "Props that never sleep") |
| parallel iteration | 0 `par_iter` | every per-frame schedule on a single-threaded executor (`client::app::run`) |
| `HashMap` in simulation crates | std's in 65 files | std's hasher is seeded per process |
| process-wide mutable statics | ~40 | weapon sets, Combat Arms' data, effects, models, view-model caches, binds (E2) |
| replicated components / server / client messages | 32 / 33 / 20 | union of both games; one replicon protocol hash |
| predicted components | 17 + 12 weapon parts | union of both games, save/restore per replayed command |

### 2.5 Tests and build

- Integration tests by crate: `cs_source` 857 (283 heavy),
  `combat_arms` 813 (66 heavy), `lithtech_formats` 101 (35 heavy),
  `chimera_present` 21, `chimera_sim` 9, `chimera_client` 3,
  `chimera_world` and `chimera_net` none (56 and 37 unit tests). The
  engine's contracts (the weapon frame, prediction, `NetSim`, map
  spawning) are tested almost only through the two games.
- Full suite (performance.md, "The push hook", 2026-10-19): 1,757 s
  wall at load 15-25, 2,833 tests; heavy tests ~5,500 s of ~7,200 s
  summed; a third of the heavy time is converting maps (10-14
  measurement). A push touching `chimera_world` selects nearly all of
  it.
- Leaf edits (performance.md, 10-15): `cargo test --features dev
  --no-run` 14-23 s in a game or the shell; `cargo test -p <crate>`
  2-15 s. A cold build 41-85 minutes by load.
- The harness (`chimera_world::harness::Sim`) runs Bevy's
  multi-threaded executor; the game, both dedicated servers and
  `perfgate` run single-threaded ones (`client::app::run`,
  `harness::single_threaded_schedules`). nextest runs 8 test processes,
  each with Bevy's default task pools.
- CI: only `release.yml` (builds on tags). No test runs off the dev box;
  Windows runs only as playtests (tech-debt.md: "Windows build not yet
  verified"). Found while committing this review on the Windows PC: in a
  clone with `core.autocrlf=true` (Git for Windows' default), docs are
  checked out with CRLF and `architecture::docs_stay_one_line_per_entry`
  panics (it splits ARCHITECTURE.md on `"\n## Modules\n"`,
  checks/tests/it/architecture.rs:677), so the pre-commit hook refuses
  every commit there, docs-only ones included. The fix is a line-ending
  normalisation in the check (or a `.gitattributes` with `eol=lf` for
  Markdown); T3's Windows job would have caught it.

### 2.6 Docs

docs/ is 1.17 MB. Sizes: combat-arms.md 180 KB, OBSERVABILITY.md 113
KB, multiplayer.md 111 KB, performance.md 92 KB, community-maps.md 90
KB, parity-gaps.md 76 KB, ARCHITECTURE.md 62 KB. Commits touching them
since 2026-10-15: tech-debt.md **99**, parity-gaps.md 73,
ARCHITECTURE.md 56, OBSERVABILITY.md 55, backlog.md 47, combat-arms.md
44. The one-line-per-entry rule keeps rows short but every feature
still appends to the same two lists, so they remain the files parallel
branches conflict on most.

Survey scripts (not kept, as before): line and test counts per crate
with inline test modules subtracted; function lengths and parameter
counts by brace matching on comment- and string-stripped source;
pattern counts (the table above); widest `Query` tuples; field counts
of `Resource`/`Component`/`Message` structs; `cargo metadata` closures;
`git log --no-merges --name-only` by lowest crate. Each is under 100
lines of Python; a third pass should keep them (T5).

## 3. Modularity

### A1. Games as parts; the gameplay a run-time choice (priority 1)

- **Evidence**: `cs_source::SimPlugins` adds `chimera_sim::
  SimBasePlugins` (games/cs_source/src/lib.rs:74), and so does
  `combat_arms::SimPlugins` (games/combat_arms/src/lib.rs:60): an app
  adding both panics on the duplicate plugin. `game_registry`: "the
  first registered is the default game" (chimera/core/src/
  game_registry.rs:8-12), which decides the tick, binds, the map list
  and the shell's map-change tick (chimera/client/src/client/
  console.rs:1948); `MashupPlugins` registers Combat Arms first and CS:S
  second only for its install (apps/mashup_demo/src/lib.rs). Each app
  sets its tick by hand (`Time::<Fixed>::from_seconds(<game>::
  TICK_INTERVAL)`, six places in apps/). A HUD is chosen when the app
  is built (`with_hud`, games/*/src/ui/mod.rs). A game's systems are
  gated by role only (`authoritative`, `server`, `client`: 31 of the
  games' 56 `run_if`s), never by "this game is the one playing"; CS:S's
  `DeathmatchPlugin`, `ObjectivesPlugin` and `BotPlugin` run on the
  server whenever added. The mashup therefore can't run Source's map logic (it comes
  with CS:S's `SimPlugins`) or offer CS:S's guns (their hooks are
  registered by CS:S's simulation) (docs/plans/active/mashup-demo.md,
  "Left").
- **Problem**: Lucker Party rotates minigames from several games in one
  session. As built, switching from a CS:S minigame to a Combat Arms one
  needs a different binary. The likely workaround under deadline, a
  Lucker Party app that copies or forks game code, is exactly what the
  layering exists to prevent.
- **Proposal**:
  1. Split each game's simulation into its parts and the base:
     `cs_source::CsSourceParts` (rules, objectives, bots, net part,
     Source's logic), `combat_arms::CombatArmsParts`; `SimPlugins` stays
     as base + parts for the faithful apps and harnesses. Separately, a
     weapons-only plugin per game (data set and hooks), so an app can
     offer a game's guns without its rules.
  2. A `chimera_core::gameplay` module: a `Gameplay` resource naming the
     active implementation of each match-wide slot (rules module id,
     HUD id, character kit, weapon sets, tick), set from the match (a
     minigame definition later, the app's default now). `Games` stays
     the registry of installs and map loaders; "default game" keeps only
     its meaning for bare map names.
  3. Gating by slot, not by plugin presence: each game puts its
     match-wide systems (rules, bots, its HUD, its net part's
     match-state systems) in its own set per schedule, configured once
     with `run_if(gameplay_is(GAME))`. Per-entity behaviour is already
     gated by components (the movement marker, a weapon's game
     components from its hooks, `CaBot`), so that sweep is limited to
     plugins that act on match state. The tick comes from the gameplay
     (`core::set_tick_length` on a gameplay change).
  4. HUDs registered as front-end parts with their own gating instead
     of `with_hud` at build time.
- **Migration** (each step lands alone):
  1. Parts without the base (N2): a test in `apps/mashup_demo` that
     builds an app with both games' parts and the base once; the five
     apps' `CHIMERA_SYSTEM_DUMP` identical before and after.
  2. `Gameplay` resource and `gameplay_is` (X1, chimera_core only),
     with the apps setting it; nothing gated yet. Unit tests.
  3. One game at a time: its match-wide plugins into its set; the
     faithful app's system dump identical modulo set names; its full
     suite; then a scenario test with both games' parts where only the
     active gameplay's rules run and a switch changes the tick.
  4. Mashup: Source's logic and CS:S's weapons added to the mashup app;
     a heavy test on cs_office opens a door, breaks a window and fires a
     CS:S gun as Combat Arms' soldier.
- **Risk**: medium. Adding sets doesn't reorder systems unless the sets
  get constraints; ambiguous pairs can still sort differently (E1 first
  makes that visible). Guard: system dumps, the full suite, the network
  tests of both games.
- **Payoff**: the precondition for Lucker Party; also lets the mashup
  use Source's map logic and CS:S's guns now.

### A2. Map lifetime and the world/kit split (priority 3)

- **Evidence**: `MapData` has 73 fields (chimera/world/src/map/
  data.rs:814): geometry and collision, entities, lighting, sky, vis,
  water, sounds, but also what a game brings to every map (character
  bodies, held and view models, the HUD look, the radio, bot profiles,
  the navigation mesh, round sounds, muzzle flash, shells, decals) and
  Source-flavoured effect lists (`steam`, `teslas`, `strikes`, `embers`,
  `smokestacks`, `dust`, `detail_props`). A mashup strips the map's game
  parts with a hand-kept list of 12 fields (`MapData::take_game_parts`,
  chimera/world/src/map/sources.rs:126); `unload_map` removes 25
  resources and several components by hand (chimera/world/src/map/
  mod.rs:232-292) and then runs `MapHooks::unload`. `PendingMap(Arc<
  MapData>)` is removed only at unload (mod.rs:137, 246), so every
  decoded texture (RGBA8 plus mips) stays in RAM for the map's life
  while `render::images::to_image` copies the bytes into the GPU image
  (`t.rgba8.clone()`, chimera/present/src/render/images.rs:99;
  tech-debt.md: ~1 GB of textures on surf_sedona).
- **Problem**: a new game part added to `MapData` without a matching
  line in `take_game_parts` leaks into mashups silently; a resource a
  new feature inserts at spawn and forgets in `unload_map` leaks into
  the next map (and, in Lucker Party's rotation, into the next game).
  Every new game feature edits `map/data.rs` at the bottom of the graph
  (15 commits since 10-15), which rebuilds everything above.
- **Proposal**:
  1. Map-scoped resources registered once (`app.map_scoped::<T>()` in
     `chimera_world`): `unload_map` removes every registered one; a
     test spawns each kind of map (greybox, a code-built CS:S map, a
     code-built Combat Arms world), unloads, and fails on any resource
     present after unload that wasn't there before the load and isn't
     registered as kept.
  2. Drop the bulk after spawning: `spawn_map` takes the textures (and
     the meshes' vertex data where the drawer copied them) out of the
     pending data, or `PendingMap` is replaced by the few resources
     later systems need. Measure the RSS after load before and after.
  3. Split `MapData` into the map's world (`MapWorld`: what the map's
     file defines) and a game's kit (`GameKit`: bodies, held and view
     models, HUD look, radio, bot names, round sounds, muzzle flash,
     shells, decals), loaded by the gameplay's game, not the map's. A
     faithful app loads both from one game; a mashup loads the world
     from one and the kit from the other, and `take_game_parts` goes.
     Source-only effect lists move behind one typed extension (`MapData
     ::extras`, keyed by type, filled and read by the Source crates),
     so a third game's own map features don't widen Chimera's struct.
- **Migration**: (1) and (2) are one session (N4), guarded by the spawn
  dump on the five maps, `drawn_parts.rs` and the new unload test. (3)
  is two to three sessions (X7) after A1 step 1: the loaders first
  return both halves from one call, then the mashup loads them apart,
  then `take_game_parts` is deleted; the mashup's heavy test and both
  games' heavy map tests guard it.
- **Risk**: low for (1)-(2); medium for (3) (every loader and the
  spawn phases change their inputs).
- **Payoff**: memory (an estimated few hundred MB on texture-heavy
  maps, to be measured), no leaks between maps or games, fewer
  bottom-crate edits.

### A3. Source-shaped vocabulary in Chimera (priority 6)

What the second game and the mashup had to work around, and what a
third game or an original minigame would hit next:

| Where | What | Proposal |
|---|---|---|
| `net::OwnState` (chimera/net/src/net/mod.rs:612), filled in server.rs:919-955 | CS:S's `money`, `armor` (amount, helmet), `kit` (defuse kit), `nightvision` in every game's own-state message; `movement` as a `String` per player per tick | own-state parts registered like `predicted_codec` (name, encode, decode); CS:S registers money, kit and night vision, Combat Arms its armour; movement as a registry index |
| `weapon::economy` (chimera/sim/src/weapon/economy.rs:75, 353, 360) | `Money`, `DefuseKit`, `NightVision` as engine types | the shop stays Chimera's; the kit and goggles become CS:S's items (`ItemParts` already exists for that) |
| `core::Intent` (chimera/core/src/core.rs:87) | a fixed set of buttons (Source's: walk, use, secondary, lastinv) | named actions (`ActionRegistry`: a game or minigame registers `throw`, `ability`, `emote`; binds name actions; `Intent` carries a bit set beside today's fields, which become aliases) |
| `core::Hitgroup` (core.rs:266) | Source's seven hit groups | keep as the normalised damage vocabulary, add `Other(u8)` for a game's own zones |
| `mount::install::STEAM_GAMES` (chimera/core/src/mount/install.rs:43) | CS:S's Steam app and files in Chimera's base | a `steam: Option<SteamGame>` field on `GameInfo` (**done**, X6) |
| `net::maps` (chimera/net/src/net/maps.rs:227) | the map offer is a BSP file (`maps/<name>.bsp`, `.bsp.bz2`, `sv_downloadurl`) | a `MapLoader` names its map's file (if one exists) and layout; maps without a downloadable file (Combat Arms') are by id and hash only | (**done**, X6)
| `map::MapLoader::load(name, hdr_level)` (chimera/world/src/map/sources.rs:33) | Source's `mat_hdr_level` in the engine's loader interface | a `MapLoadOptions` struct (done in N3 with `hdr_level` and `textures`; `models` and `sounds` when something turns them off) |
| 80 `0.0254` literals in 30 Chimera files (most: `render/water.rs` 8, `weapon/drop.rs` 7, `render/emitters.rs` 7, `map/ragdoll.rs` 6); `net::quant`'s 1/32-inch grid | Source's numbers as engine defaults (drop distances, ragdoll pushes, effect sizes, third-person camera, shadows, audio distances) | each becomes a named constant of the game that measured it, passed as data; the net grid stays (it is a precision, 0.79 mm) but named in metres |
| `logic::LogicSet` (source/logic/src/logic/bridge.rs:103) in the tick's order | the map-logic stages are Source's crate's | `SimSet::{LogicPre, LogicPost, LogicDamage}` in `chimera_core`; Source's logic and Combat Arms' world objects (Wave 5, item 23) both use them |
| `client::debug_ui` (chimera/client/src/client/debug_ui.rs:177-183), `binds::POLLED`, the shell's `--help` text | `mp_*` cvars, CS:S's polled commands, `cs_source` in usage | from `GameDebug`, `GameInfo` |
| surfaces | Source's surface properties vs Combat Arms' surface types: the mashup's effects and footsteps fall back to "unknown" | a normalised surface kind in Chimera (concrete, metal, wood, dirt, grass, water, glass, flesh, ...) with each game's table to it as data; effects keyed by the normalised kind plus the game's own |

Slices are independent (X4, X5, X6 in section 11); each bumps
`NET_VERSION` where the protocol changes and keeps CS:S's behaviour
(its network and weapon tests unchanged in their expectations).

### A4. One app builder (priority 11)

- **Evidence**: the 12 material plugins (`WorldMaterialPlugin` ...
  `DetailMaterialPlugin`) are listed in apps/cs_source/src/main.rs:
  105-118, apps/combat_arms/src/main.rs:96-107 and apps/mashup_demo/
  src/main.rs:78-89; `PhysicsPlugins` with Chimera's collision hooks and
  without interpolation in four places (three apps and the harness);
  the log layer, window settings and tick in each app.
- **Proposal**: `chimera_present::PresentationPlugins` (the materials)
  and `chimera_world::PhysicsSetup` (avian as Chimera runs it, used by
  the harness too), plus the tick taken from the gameplay (A1). An app
  `main.rs` then lists only its games and its front end.
- **Migration**: one session (N7); the system dump of each app
  identical, screenshots of de_dust2 and WAREHOUSE unchanged.
- **Payoff**: an app can't forget a material (a missing one draws
  nothing and fails no test today); Lucker Party's app starts small.

### A5. Is the crate layering right?

Mostly yes: the lowest crates rarely depend on what they shouldn't,
the presentation is off the simulation and the dedicated servers build
without it, and the game crates never meet. Three corrections, none
urgent:

- A neutral VFS crate (`chimera/vfs` or similar: `FileSource`, `Mount`,
  `LooseDir`, `normalize`) under `source_formats` and
  `lithtech_formats`, so Chimera's base and the LithTech loader stop
  depending on a Source crate; Steam's `libraryfolders.vdf` parsing can
  stay on `source_formats`' KeyValues or take a small reader with it
  (X10, one session; `workspace_crates` rules updated).
- Source behaviour that isn't CS:S's (BSP, MDL and VMT conversion,
  Source movement, sound scripts) stays in `cs_source` until a second
  Source game (HL2DM, CS:GO maps) needs it, then becomes a Bevy-level
  crate in `source/` (combat-arms.md's lift survey lists the files).
  Not before: one user doesn't prove the interface.
- `bevy` without default features in the workspace, each crate asking
  for what it uses (engine-split.md has the blockers): the dedicated
  servers would stop compiling `bevy_render`/`wgpu`. Worth it when a
  server build's time or size matters (Lucker Party's hosted servers).
  **Done 10-22** (L6, note below).

The layering is missing two planned layers: `games/originals` (Lucker
Party's own parts and minigames' rule code) and `minigames/` (data).
Create them with the first original minigame (L2), with
`workspace_crates` rules: originals may use any game crate's public
parts only through Chimera's slots, never a game's internals.

### A6. Checklist for adding game #3

What a third game (MW2, Minecraft, Half-Life 2) does today, in order,
with the traps:

1. Formats in a plain-data crate in its loader layer (`source/`,
   `lithtech/`, or a new one): no Bevy, fixtures written by tests,
   `heavy::` tests on the install that skip without it; add it to
   `workspace_crates` and the hakari excludes if it has no Bevy.
2. A game crate `games/<id>` with `GameInfo` (id, short name, content
   folder, tick, `open`, map cycle, import, default binds, movement
   cvars) registered first in its plugin group; after A3, its Steam
   app and map file layout there too.
3. A `MapLoader` producing `MapData` in metres with `entity_scale` for
   its own units, and a `MapDressing` for playing its gameplay on other
   games' maps. Never add game-specific fields to `MapData` (A2's
   extension instead).
4. Movement: a marker component and `register_movement::<M>("<id>:
   movement")`; its state registered with `predicted_net`; its numbers
   as data (`data/movement.toml`), in its own unit.
5. Weapons as TOML in `data/weapons/` (docs/weapon-data.md), behaviour
   as hooks (`register_weapon_hook`), predicted parts with
   `predicted_part`; hit zones through `Hitgroup` (or `Other` after
   A3).
6. Rules as rule modules (`RuleModules::register`), in its gameplay set
   (A1); its respawn through `chimera_sim::spawning`.
7. Its net part (`net_part("<id>", ...)`): replicated components and
   messages, `JoinRules` if the default join doesn't fit, own-state
   parts (A3).
8. Its front end behind a `ui` feature: HUD (registered, A1), menus via
   `client::front_end`, its layouts on `client::canvas` or
   `client::vgui`.
9. `harness::<Game>Sim` as its tests' base; scenario tests per spec case;
   one network test per feature on `NetSim`.
10. Apps: its faithful app and dedicated server (without `ui`) on the
    app builder (A4); `combat_arms_server` is the smaller example.
11. Architecture test entries, ARCHITECTURE.md's crate paragraph, a plan
    in `docs/plans/active/`.
12. Don't rely on: the first-registered game being yours, `OwnState`'s
    CS:S fields, Chimera's Source-inch defaults (A3's list), BSP map
    downloads, `LogicSet` being generic, or the 12 material plugins
    being added for you.

## 4. ECS design

### E1. Determinism and ordering guards (priority 2)

- **Evidence**: `Sim::start` (chimera/world/src/harness.rs:85-100)
  never calls `single_threaded_schedules`; the game, both dedicated
  servers and `perfgate` do (chimera/client/src/client/app.rs:26,
  apps/*_server/src/main.rs). Only two test files switch it on
  (`net_bots.rs`, `heavy/net_capacity.rs`). Ambiguity detection is
  opt-in and only for the render world's `Render` schedule
  (`CHIMERA_AMBIGUITY=1`, app.rs:62). The client adds presentation
  systems into the simulation's schedules at fixed places
  (`map::MapLayers`, `client::input`'s `FixedPreUpdate`,
  `client::interp`'s `FixedLast`), the server doesn't. 65 simulation
  files use std's `HashMap`/`HashSet` (seeded per process); the ones
  sampled iterate order-independently (Combat Arms' assist sums), but
  nothing keeps it so. Windows-to-Linux prediction is unmeasured
  (multiplayer.md: "if they show, use the `libm` crate").
- **Problem**: with a single-threaded executor, two systems with
  conflicting access and no order run in whatever order the graph sorts
  to, and adding any system can change it (performance.md saw this in
  the render world). Tests run another executor than the shipped game,
  so an order a test passes with may not be the one players get; and a
  client and its server, built from different plugin sets, can order
  the same simulation systems differently, which is a misprediction no
  single-process test sees. The more games and minigames share an app,
  the more systems there are to collide.
- **Proposal**:
  1. `Sim` on the production executors by default (opt out with an
     argument), and its task pools capped (`TaskPoolPlugin` with 2
     compute threads), which also stops 8 nextest processes from each
     starting a full set of threads.
  2. An ambiguity ratchet per game: build the app as each faithful
     binary and dedicated server composes it, count the ambiguous pairs
     in `FixedUpdate`, `FixedPostUpdate`, `FixedLast` and the three
     `Predict` schedules (after a schedule is initialised its graph
     lists them: `conflicting_systems`, bevy_ecs 0.19.1
     src/schedule/schedule.rs:786), and fail when the count rises;
     lower the stored number as pairs get ordered.
  3. A test per game that the simulation systems of `FixedUpdate` and
     `Predict::*` run in the same order in the server's composition and
     the client's (filtering out what only the client registers).
  4. A `repo_checks` scan (or `clippy.toml` `disallowed-types` in the
     simulation crates) against std's `HashMap`/`HashSet` where new code
     iterates them, pointing to `BTreeMap` or Bevy's fixed-hash maps.
  5. Measure Windows against Linux once: a Windows client on a Linux
     dedicated server, a scripted run (strafe jumps, ladders, a magazine
     of recoil) with `cl_showerror 1`, errors per minute (the Windows
     PC can do it with `scripts\playtest.ps1` and the box's server).
- **Migration**: (1) first, alone, with the full suite run twice: any
  test that now fails had an order dependence, fixed by an explicit
  edge, never by reverting (N1). (2) and (3) next, in new test files
  only. (4) after (1). (5) any time.
- **Risk**: low for the code, medium for the schedule: (1) may surface
  several order bugs at once; budget a session for them.
- **Payoff**: prediction stays exact as games are combined; tests
  check what ships; likely faster tests (the single-threaded executor
  cut the main and render worlds' time by a quarter and a sixth on
  de_dust2: `client::app`'s doc, performance.md "Frame time pass").

### E2. Process-wide state into the world (priority 5)

- **Status (X3, 10-21)**: done: Combat Arms' `BUILTIN_ONLY` switch and the
  install-derived tables (weapon set, attachments, movement, gear,
  character records, missions, attribute data) are one `CaData` resource
  per app (`games/combat_arms/src/game/data/store.rs`; `CaSim` inserts
  `CaData::builtin()`, `CaSimWith` any other); `weapon::data::LOADED` /
  `publish` / `lookup` are gone (`WeaponCatalog::get`, `get_name`; the view
  animation driver reads the data from `ViewState::data`);
  `tests/it/data_isolation.rs` runs two `Sim`s with different weapon data.
  Left (Combat Arms 22 statics, CS:S 6): the memos of install content
  that the map loaders fill, because `MapLoader`/`MapDressing` are plain
  function pointers with no app to ask (they use `CaData::install()`):
  the effect library and its parse (`effects::library`), the install
  mount (`world::mount`), rigs, body groups and shared character models
  (`characters::models`), on-demand view models (`view_model::on_demand`);
  plus read-only tables (movement, render styles, fx tables, music,
  lobby, surfaces, anim lists, the two `CaData` singletons). Moving the
  loader-filled ones into resources needs the loaders to receive the app's
  context (F-series map-loading work); they then free with the app.

- **Evidence** (mutable statics outside tests): `weapon::data::LOADED`
  (chimera/sim/src/weapon/data.rs:663; tech-debt.md: "Weapon data sets
  are leaked and process-wide"); Combat Arms' `data::BUILTIN_ONLY`
  (games/combat_arms/src/game/data.rs:150, an `AtomicBool` switch) and
  its `OnceLock` caches of movement data, attributes, mod tables,
  character records, rigs, body groups, shared character models, the
  effects library and its mount, render styles, view models on demand
  ("never freed", tech-debt.md) and the world's mount; the shell's
  `binds::GAME_DEFAULTS` and `OLD_DEFAULTS`, `console::MAP_NAMES`;
  `map::loading::PROGRESS`.
- **Problem**: they outlive any `World`. `NetSim` runs a server and
  clients in one process, so they share these (a test that loads
  another weapon set or flips `BUILTIN_ONLY` changes every other app in
  the process); a server can't host two matches with different data in
  one process; data can't reload without a restart (minigame tuning,
  modding); caches hold memory after a game's minigame ends.
- **Proposal**: resources (or Bevy assets, with F6) owned by the app:
  weapon sets in `WeaponCatalog` with ids interned per app, Combat Arms'
  tables as resources its data plugin fills, caches keyed by path with
  handles so unused entries free. A `repo_checks` test listing the
  allowed statics (read-only tables, the log layer, metrics) and failing
  on new mutable ones in simulation crates.
- **Migration**: one module per commit, Combat Arms first (most
  statics, newest code, best spec tests), then `weapon::data`; guarded
  by the game's suite and a new test that two `Sim`s in one process with
  different weapon data don't see each other's (X3).
- **Risk**: low-medium (weapon ids are `&'static str` throughout the
  weapon code; interning keeps that).
- **Payoff**: multi-match servers, hot-reloadable data, honest tests.

### E3. Giant systems and fat components (P8, grown)

`bot::think` (696 lines, 16 parameters), Combat Arms' `movement::step`
(634, 11) and `bots::brain` (1,711-line file), `apply_effects` (439),
`class_input` (394), `client_commands` (335), `weapon_frame` (324),
`radar::update`, `overview::draw`, `impact_effects`, `shot_effects`
(15-16 parameters each). Components: CS:S's `Bot` 62 fields, `CaBot` 46,
`CaMovement` 39, `SourceMovement` 30.

- **Problem**: hard to review, and every write to a fat component marks
  all of it changed; predicted fat components (`CaMovement`,
  `SourceMovement`) are cloned whole per entity per replayed command.
- **Proposal** (opportunistic, when a feature touches one): systems split
  by phase into chained functions with `SystemParam` bundles (the
  characters, the local player, fonts and scale); bots' components split
  by how often they change (configuration, perception, plan, steering);
  movement state split into the predicted part and the derived part.
- **Risk**: low-medium; existing tests cover each. Not a project on its
  own: bots are parked for CS:S, and Combat Arms' movement follows its
  spec's order on purpose.

### E4. Plugins that half-exist

627 `Option<Res<_>>` parameters (most for `ActiveHud`, `SoundBank`,
`NetRole`, materials) let systems run with parts of an app missing.
That is how the server and headless tests work without presentation,
and it hides missing plugins: a system silently does nothing. Prefer
run conditions (`resource_exists::<T>`) at registration for whole-
system absence, and `Option` only where a system has real work either
way; a `debug_assert` in each plugin's `finish` that its required
plugins are present (`App::is_plugin_added`). Low priority; do it in
files being edited anyway.

### E5. Messages, observers, change detection, hierarchies

- Messages (172 registered) for per-tick flows and observers (21) for
  network events and one-off reactions is the right split for Bevy
  0.19; nothing to change. Messages written in `Update` and read in
  `FixedUpdate` survive frames without a tick (Bevy updates message
  buffers only after a fixed tick ran); keep it so when adding readers
  in the fixed schedules.
- The 18 `bypass_change_detection` calls are avian wake-up workarounds
  (a velocity write wakes a body); document each at its site (most
  are) and list them for the avian upgrade (L5).
- Hierarchies: map parts under the map's root, static parts under their
  own; avian walks every collider under a root each tick (0.3 ms on
  dust2, performance.md); fine at stock maps' counts.

### E6. Does the prediction model fit the ECS?

Yes, well. Prediction runs the `Predict::{Select, Movement, Weapons}`
schedules once per command (core.rs:1130-1154), predicted components are
registered by type (`PredictedAppExt`) and saved, restored and compared
generically, side effects are gated by `FirstTimePredicted`, and map
movers replay through `NetHooks::replay`. Two games plug into it without
touching it. Small things:

- `PredictedComponents::net_order` builds and sorts a `Vec` on every
  `encode`, `decode`, `split` and `differing` (core.rs:1257): once per
  player per tick on the server, more on a client's replays. Sort once
  at `finish`.
- Save and restore box every registered component per entity per
  snapshot (`Box<dyn Any>`); with the union of every game's predicted
  types in a Lucker Party app, entities pay for types they don't have
  (an `Option` per type per command). Cheap now; measure with N1's
  executor and `perfgate` after A1 combines games.
- Nothing checks that a system added to a `Predict` schedule is a pure
  function of predicted state, `Intent` and `SimClock`: a game that
  reads `Time` or an unpredicted resource there mispredicts only under
  latency. Each game's network suite should keep a scripted
  "everything predicted" scenario run with zero errors (CS:S's
  `net_prediction.rs` and Combat Arms' `net.rs` do; make it a template
  in the test kit, T2).

## 5. Performance

### Where the time goes

| Area | Number | Source |
|---|---|---|
| Frame (client, dust2, playtest build) | p50 3.9-5.4 ms, p99 8.6-17.4 ms, render world 13.7M instructions, main world 6.7-7.3M, GPU 0.22-0.25 ms; the 240 Hz goal is 4.2 ms | performance.md, "Baseline" |
| Systems per frame | ~1,500 (main and render), most 0.01-0.04 ms; the executor's hand-off cost more than them | performance.md, "Frame time pass" |
| Tick (server, dust2, 10 bots) | 17.9M instructions per tick summed over threads (p99 26-27M); estimate 3-6 ms on the dev box, against a 15 ms tick | perfgate baseline |
| Server capacity | bots' sight is the limit (64 bots on mg_jacks don't fit); ~17 KB/s per client at 32 players, no relevance culling (37-45% of pairs in each other's PVS on dust2) | tech-debt.md, server.rs's `//!` |
| Map conversion | dust2 under 1 s; Combat Arms 0.2-1.3 s per map plus 2 s of weapon data per session; the dedicated servers and tests decode every texture they never draw | performance.md, "Combat Arms map loads"; bsp.rs has no options |
| Memory | Combat Arms' app peaks 2.1-3.4 GB; heavy tests median 1.1 GB, ten over 3.3 GB; surf_sedona's textures ~1 GB as RGBA8; `MapData` resident while mapped (A2) | performance.md, tech-debt.md |
| GPU memory | every texture RGBA8 (no BC formats anywhere): 4 bytes a texel against DXT1's 0.5 and DXT5's 1, both games' native formats | `render::images`, no `Bc*` format in the code |

The game is CPU-bound by per-entity and per-system work, not by the GPU
(a 3080 draws dust2 in a quarter of a millisecond).

### F1. Lean map loads (priority 4)

- **Problem**: the dedicated servers and most heavy tests decode,
  mip and keep every texture; a third of the heavy tier's time is map
  conversion (10-14), and CS:S decodes its textures on one thread while
  Combat Arms' loader uses eight (`decode_all`).
- **Proposal**: `MapLoadOptions { textures, models, sounds, hdr_level }`
  passed to every `MapLoader`; the servers and tests that check
  collision, logic or movement load without textures (placeholders of
  the right size, so materials and decals still find them); CS:S's
  decode on threads through a shared helper (Combat Arms' `decode_all`
  moved down into `chimera_world`).
- **Migration** (N3): the options first with everything on (the spawn
  dump identical), then the servers, then the heavy files that never
  look at textures (a list in the PR; screenshot tests keep them on);
  record the heavy tier's sum and the server's RSS before and after
  (`MapData::load_times` already times the phases).
- **Done** as N3 (10-21): on a quiet comparison the texture decode was
  0.4-0.7 s of a 1.8-3 s dust2/de_nuke load (on one thread, before), not a
  third; the memory win is the one that held (servers 2.3x smaller).
- **Estimate**: a third to a half off map loads where textures dominate,
  so roughly 10-15% off the heavy tier, and the servers' memory by the
  texture set. Measure first on dust2, de_nuke and surf_sedona.

### F2. Drop `MapData`'s bulk after spawning

A2 step 2. Cheap, and the largest memory win per line changed.

### F3. Compressed textures

- **Proposal**: upload VTF's and DTX's DXT1/3/5 blocks as BC1/2/3 when
  the adapter has `TEXTURE_COMPRESSION_BC` (every desktop GPU we
  target), decoding to RGBA8 only where a CPU reads texels (lightmap
  helpers, surface colours, the occasional effect). Shaders sample the
  same.
- **Payoff**: VRAM and upload 4-8 times smaller, texture decode gone
  from loads. **Risk**: medium (sRGB variants, mip chains shorter than
  full, alpha-tested pieces, the merged world's atlases); guard with
  refcmp on de_dust2 and de_nuke and the Combat Arms screenshots.
  After F1 (both touch the loaders' texture output) (X9).

### F4. Parallelism where it is real

The single-threaded executors stay (measured right for ~1,500 tiny
systems). Parallel work belongs inside the few systems that iterate a
lot of independent read-only work:

- bots' sight and path search (read-only `SpatialQuery`, one result per
  bot): `par_iter_mut` over bots, the server's capacity limit;
- the server's per-client encoding (`OwnState`, replicon serialises
  itself) once there are many clients;
- map conversion (F1).

Measure each with `perfgate` (instructions summed over threads and wall
time) before and after; keep a change only if wall time drops at load.

### F5. What to measure, and how

- A per-tick allocation count: a counting global allocator in
  `perfgate` (behind its `tools` feature), so allocation regressions show
  like instruction ones. The code has 1,519 `collect`s, 1,676 `format!`s
  and 1,146 `Vec::new`/`vec!` outside tests; which run per tick is not
  known. Then remove the ones in the top systems.
- `perfgate` scenarios for Combat Arms (WAREHOUSE with bots) and for the
  mashup, so Lucker Party's composition has a baseline before A1 lands.
- RSS after load and after unload for every map in the heavy sweeps
  (sampled from `/proc` on Linux, `GetProcessMemoryInfo` on Windows):
  catches A2's leaks and F2's win.
- Frames and ticks on the Windows PC (the `GetThreadTimes` 15.6 ms
  granularity makes per-frame CPU time useless there: compare wall times
  over many frames, and GPU timestamps).
- Bandwidth per client at 8, 16 and 32 players for each game's modes
  (`heavy::net_capacity` for CS:S; none for Combat Arms yet).

### F6. The asset pipeline

- **Evidence**: no `AssetLoader` or `AssetSource` anywhere (4
  `AssetServer` uses in all); a map converts synchronously into
  `MapData` and its meshes and images go into `Assets<_>` by hand
  (`render::spawn`); "Maps load synchronously before the app starts"
  (tech-debt.md; apps/mashup_demo/src/main.rs loads before `App::new`);
  CS:S decodes shared textures again on every load (no cache,
  tech-debt.md); Combat Arms built session caches of its own (view
  models on demand, character models once per session, both never
  freed).
- **Problem**: content shared between maps (hl2's textures, a game's
  character models) loads once per map or is kept forever; nothing
  loads in the background, which Lucker Party's rotation needs (the
  next minigame loading while the current one plays); every game
  reinvents caching and freeing.
- **Proposal**: an `install://<game>/<path>` asset source reading through
  the game's `Mount`; asset loaders per converted kind (textures,
  models, sounds) in each game crate; a map as an asset whose
  dependencies are those handles; the loading screen waits on load
  states. Assets free when the last handle drops.
- **Migration**: textures first, behind a cvar (after X9 settles their
  format), spawn dumps compared; then models (Combat Arms' caches
  replaced); maps last, with `change_map` waiting on the load. One
  game at a time.
- **Risk**: high (every loader and spawn phase; Bevy's asset API moves
  between releases). **Payoff**: background loads, shared content
  loaded once, memory that follows what is in use (L4).

### Quick wins and deep work

| Quick (a session or less) | Deep (several) |
|---|---|
| F2: drop the texture bytes after spawning | F3: BC textures |
| F1: no textures for servers and collision tests; CS:S decode on threads | F6: the asset pipeline (L4) |
| `net_order` sorted once (E6) | relevance culling (M5) |
| E1's executor in tests (also faster tests) | bots' sight in parallel (F4) (done 10-22) |
| a per-tick allocation counter in `perfgate` | `bevy` without default features for servers (A5) (done 10-22) |

## 6. Data-driven design and a minigame API

### D1. What is still code that should be data or a hook

Already data: weapons, the shop, penetration, CS:S's round numbers,
Combat Arms' movement, modes' settings, bots, characters, effects,
surfaces, render styles, UI strings and layouts (docs/weapon-data.md,
docs/economy-data.md, games/combat_arms/data/). Still code:

- **Which parts play**: the app's plugin list and the first registered
  game (A1). Should be a match's choice of slot implementations by id.
- **Mode flow**: CS:S's round loop is code (`rules/rounds.rs`), its
  rule modules are descriptions whose `applies` picks nothing that runs
  (chimera/sim/src/rule_module.rs:60: "the first registered module that
  applies" is reported, not enforced). Combat Arms' modes are a table
  of data and hooks (`CaMode`: teams, spawn family, round time, warm-up,
  respawn, scoring, setup, per-tick, round over, death, respawn and
  joiner hooks; games/combat_arms/src/game/rules.rs) driven by one
  mission update: the best shape in the repository for a minigame
  runtime, but private to Combat Arms.
- **HUD composition**: chosen at build time (`with_hud`); Combat Arms
  binds the install's layouts' controls to values by name
  (`ui::hud::bound`), which is the data-driven HUD pattern to lift.
- **Input actions**: `Intent`'s fixed buttons (A3).
- **Surfaces**: per game, no shared kind (A3).
- **Bots' brains**: code per game, correctly (they are each game's
  behaviour); a minigame needs only "fight", "go to", "carry" goals on
  them.

### D2. A minigame API (sketch)

Goal: an original Lucker Party minigame is a data file, plus at most one
small rules hook in `games/originals` when the built-in pieces don't
cover it. The pieces exist in the two games; the work is lifting them
into one Chimera module (rule of three: CS:S's rounds, Combat Arms'
modes, the first original minigame).

The definition (data, loaded at run time, sent to clients by hash: it is
ours, unlike game assets):

```toml
# minigames/gun_game_dust2.toml
id = "originals:gun_game_dust2"
name = "Gun Game: Dust II"
players = { min = 2, max = 16 }
maps = ["cs_source:de_dust2", "combat_arms:warehouse"]   # a pool
teams = "free_for_all"
time_limit = "10m"

[slots]                              # implementations by id (A1's Gameplay)
movement = "cs_source:movement"
character = "combat_arms:soldier"    # the kit (A2) of Combat Arms
hud = "chimera:minimal"
weapons = ["cs_source", "combat_arms"]   # the weapon sets available

[spawn]
respawn = { delay = "2s", protection = "1.5s" }   # chimera_sim::spawning
points = "map"                       # or "team", "far_from_enemies"

[rules]
module = "chimera:ladder"            # built from primitives below
ladder = ["cs_source:weapon_glock", "combat_arms:m4a1",
          "cs_source:weapon_awp", "combat_arms:m9"]
advance_on = "kill"
demote_on = "melee_death"
win = { kind = "ladder_end" }

[modifiers]                          # Roll the Dice: named, registered hooks
gravity = 0.6
dice = ["big_heads"]
```

The runtime (Chimera, `chimera_sim::mode`, lifted from `CaMode`):

- **Phases** with timers: warm-up, play, round over, intermission, end;
  a definition picks which exist and how long they last.
- **Teams**: two or more named teams, free-for-all, one team (co-op),
  assignment policies (balanced, as joined, by rule).
- **Spawning and loadouts**: `chimera_sim::spawning` (already lifted),
  kits per phase, rung or team from the weapon catalogue.
- **Scoring**: kills, deaths, assists and objective points as a table
  (`CaStats` and CS:S's `Score` generalised), the scoreboard's columns
  from the definition.
- **Objective primitives**: zones (enter, leave, hold for a time;
  Combat Arms' Seize and Secure, CS:S's bomb sites and rescue zones),
  carriables (an item that follows its carrier, drops on death, returns
  after a time: flags, the bomb, intel, a hot potato), counters and
  timers.
- **Win conditions**: first to N, most at the time limit, last alive
  (player or team), objective done, ladder end; ties by the
  definition's order.
- **Events for the HUD and the announcer**, named (`round_won`,
  `lead_taken`, `rung_up`), bound by the HUD's data as Combat Arms binds
  its panels.
- **Modifiers** registered by name (gravity, speed, damage scale, head
  size), each a small hook a definition turns on.

A minigame the primitives can't express adds one rules hook (Rust,
`games/originals`), registered by name and named by the definition:

```rust
pub static HOT_POTATO: ModeRules = ModeRules {
    id: "originals:hot_potato",
    setup: |world, def| { /* hand the potato (a carriable) to a random player */ },
    tick: |world, clock| { /* the fuse; the holder blows up at 0 */ },
    event: |world, event| { /* ModeEvent::Hit { by, to }: the potato passes */ },
    ended: |world| { /* one player left */ None },
};
```

Later, if rule code must ship without a rebuild, the same hooks behind
WASM or Lua (engine-split.md, "Minigames"). CS:S's modes and Combat
Arms' modes become definitions on the same runtime, each faithful app
playing them as before (their spec tests guard it). Arms Race (Combat
Arms' VWRM, built) is the weapon ladder's first user and Gun Game its
second: the point to lift it as `chimera:ladder`.

## 7. Multiplayer for Lucker Party

The network core is in good shape for it: server authority, prediction
generic over games, game-registered parts (`net_part`, in name order),
join, bot and hold hooks, an in-memory `NetSim`, quantised bodies,
paced updates. What Lucker Party adds:

### M1. One app per match, a party per process

A party (lobby plus its matches) is one server app with every game's
parts registered (A1), so the replicon protocol hash is the same for
every minigame and no reconnect is needed between them. A minigame
change is a `ChangeMatch` (today's `ChangeLevel` plus the definition's
id and hash and the gameplay slots), loaded behind the loading screen
the shell already has. A host serving several parties runs one process
each until E2 removes the process-wide state; then one thread per app.
The dedicated party server is an app without presentation, like the
existing two.

### M2. A match manifest in the handshake

`Welcome` and `ChangeLevel` (chimera/net/src/net/mod.rs:383-420) carry
a map id, its file's hash and size, a download URL and the tick. Add:
the minigame definition (its id, hash and, for ours, its bytes), the
gameplay's slot ids, each map half's source (A2: the world's game and
file hash, the kit's game and its version), and the games the match
needs mounted. The client refuses with a clear reason before loading
anything it can't load.

### M3. The mount doctor and the lobby

DESIGN.md's mount doctor (an install's games, build ids and checked
files) isn't built. Clients report it at `Join` (installed games, Steam
build ids, a hash of the files the loaders read); the lobby offers only
minigames every player can play, or lets the party pick and makes the
players without a game spectators. Map downloads stay per loader (A3:
BSP files can be sent, Combat Arms' maps can't: they are in the
install's archives).

### M4. Protocol and versions

One protocol per Lucker Party build: its hash covers every game's
parts. A faithful CS:S client and a Lucker Party server differ in
protocol; refuse with "this server plays Lucker Party" rather than a
hash mismatch. Keep `NET_VERSION` per app. Predicted and own-state parts
registered by name (E6, A3) keep the encoding stable when a game adds a
type.

### M5. Bandwidth and relevance

At party sizes (8-16) bandwidth is not the limit (~17 KB/s per client
at 32 players today). The rule that matters for mixed content: what
every client must see (events, scores) stays small, and bodies are
culled by relevance when a mode or map makes it cheap (Source's PVS on
BSP maps, which `VisClusters` already holds; distance for others).
Replicon's per-client visibility does the culling; the rules for what
a client still gets (footsteps, radar) need a spec per game. Update rate
per mode (a slow party game needs less than CS:S's 66 ticks).

### M6. Authority and hosting

Already: the server validates commands, looks (Combat Arms), votes,
cvars. Add with M2: loadouts and modifiers only from the definition.
Hosting behind NAT: nat-traversal.md's plan (UPnP done; a relay or a
hosted server next) is the open decision for the user, not an
architecture change.

## 8. Tests and build

### T1. Tests on the production executor

E1 step 1. Also expected to shorten the suite: 8 nextest processes no
longer each spin a full task pool on a 16-thread box. Measure the full
suite before and after at similar load.

### T2. A Chimera test kit and engine contract tests (priority 9)

- **Problem**: Chimera's generic parts (weapon frame, grenades,
  prediction, `NetSim`, map spawning) are tested almost only through
  `cs_source` and `combat_arms` (2.5). A lift into Chimera can only be
  checked by running both games' suites, which is what makes 38% of
  commits run nearly everything; and a third game would find CS:S-
  shaped assumptions by surprise.
- **Proposal**: a small test kit in Chimera (a module of `chimera_sim`
  behind a `testkit` feature, or a `chimera/testkit` crate used as a
  dev-dependency): a test weapon set as TOML, a minimal rules module, a
  code-built map with spawn points, a movement; `NetSim` helpers. Then
  contract tests per Chimera crate: the weapon frame's timing and
  magazines on test data, prediction with zero errors for a scripted
  run, map spawn and unload (A2), own-state parts. When a lift moves
  code, the generic part of its tests moves down onto the kit.
- **Payoff**: a lift checked by `cargo test -p chimera_sim` in seconds;
  the affected selection can then skip a game's heavy tier for changes
  that only touch what the contract tests cover (a later decision; the
  user ruled out coverage-based selection, not crate-based).

### T3. CI without installs (priority 9)

A workflow on Linux and Windows runners: `cargo check --workspace
--lib --bins --features tools`, `cargo check -p cs_source_server -p
combat_arms_server`, and the fast tier of the crates that need no
install (`repo_checks`, both format crates, the Chimera crates' tests,
the games' tests that build their maps in code). It catches Windows
breakage (such as the CRLF failure in 2.5) and anything pushed with
`--no-verify`; it costs runner time only (rust-cache keeps it under ~15
minutes after the first run, estimate).

### T4. The test support module (P4, finish)

Per game, one `support` module: the install skip rule (one macro), map
loading by id, `sim()` builders for the common bases, `feet`, the
active weapon id. Then one sweep per game moving the 45 `fn sim(`, 24
`fn load(` and their kin into it (guard: the list of passing and skipped
tests identical before and after).

### T5. Docs that stop colliding (priority 10)

- `docs/tech-debt.md` becomes generated: `repo_checks --bin debt` lists
  every `//!` "Known shortcuts" paragraph's first sentence with its file
  (the detail is already there); the committed file keeps its header
  and the entries without a file. A feature then edits only its own
  file's `//!` doc. Same for ARCHITECTURE.md's module list from each
  file's first `//!` line (the test that checks paths and lengths moves
  to the generator).
- combat-arms.md: the spec briefs of finished waves (lines 441-2195,
  ~150 KB) move to `docs/plans/completed/combat-arms-briefs.md`; the
  board and decisions stay.
- The survey scripts of this review kept under `tools/arch_survey/`
  (2.6) so the next pass compares like with like.

### T6. Compile time

Leaf edits are fine (14-30 s). What costs is bottom edits: 38% of Rust
commits rebuild from `chimera_core`, `chimera_world` or `chimera_sim`.
A2 (game data off `MapData`) and moving non-vocabulary out of
`chimera_core::core` (2,377 lines: damage application, the predicted
component registry, `BrushGrid`, which is Source movement's broad phase)
into the crates that use them cut how often that happens. mold and
`debug = "line-tables-only"` for dependencies are still unmeasured on a
full cycle (performance.md, "Test cycle"); measure on an idle box before
adopting. Windows builds use `rust-lld` (.cargo/config.toml); measure a
leaf edit there once (no number exists).

## 9. Top 10 risks over the next 3-6 months

1. **Lucker Party can't combine games at run time** (A1): the first
   party build gets written as copies or a fork of game code, or
   waits on a large late refactor.
2. **Prediction drift as games combine** (E1): ambiguous order, test vs
   production executors, client vs server plugin sets, Windows vs Linux
   floats; mispredictions that only show under latency with friends.
3. **Bottom-crate churn**: lifts land in `chimera_world`/`sim`/`core`
   (38% of Rust commits), each rebuilding and retesting everything
   (the push suite 1,757 s and growing: Combat Arms brought 841 tests
   in two days, and the suite went from 1,644 tests on 10-15 to 2,833
   on 10-19); agents queue behind pushes and conflict in shared files.
4. **Memory with two games' content in a session**: RGBA8 textures,
   `MapData` resident, Combat Arms' caches never freed (its app peaks
   at 3.4 GB already); 8-16 GB machines run out on a minigame rotation.
5. **Process-wide state** (E2): blocks several matches per server and
   data reloads; makes in-process network tests of mixed apps share
   state silently.
6. **State leaking between maps and games** through `MapData`'s
   hand-kept strip list and `unload_map`'s hand-kept removals (A2).
7. **Source-shaped core vocabulary hardening** (A3): every week more
   code depends on `OwnState`'s CS:S fields, `Intent`'s fixed buttons,
   inch constants and BSP-only map offers; each fix later costs more
   and bumps the protocol.
8. **Windows regressions found late**: no automated build or test on
   Windows (T3); playtesters are the first to see breakage; the perf
   tools are weaker there (no hardware counters, coarse thread times).
9. **Docs as a tax**: 1.17 MB that sessions are sent to, and the top
   conflict files (tech-debt.md 99 commits in five days); a slower and
   noisier merge cycle for ~60 agents.
10. **A Bevy or avian upgrade under parallel work**: Bevy is pinned at
    0.19.1 and avian at 0.7 with workarounds in our code (contact flags,
    a stale "body disabled" flag, wake-ups by change detection);
    228k lines and both games move at once, so an upgrade needs a
    planned freeze and its own lane (L5).

## 10. Not worth doing (updated)

- Everything on the first pass's list still: rewriting net replication
  or the logic world, a typed error overhaul, dead-code hunting,
  another UI toolkit, macros for cvars or replication, splitting files
  only for size.
- Bevy's multi-threaded executor back on: measured slower for this
  workload; parallelism goes inside the few heavy systems (F4).
- A disk cache of converted maps before F1 and F3: lean loads and
  native compressed textures remove most of what a cache would save,
  without its invalidation and per-user storage.
- Native dynamic plugins for minigames (no stable ABI): data plus
  registered hooks now, scripting behind the same hooks later.
- A general "everything is a slot" registry before A1's few match-wide
  slots prove the shape: lift at the second use, as the Combat Arms work
  does.
- Splitting `cs_source` into Source and CS:S crates before a second
  Source game exists.

## 11. Roadmap

Sizes: S one session or less, M two or three, L four or more. Items in a
phase touch disjoint files so agents can run them in parallel; each
lands alone, quickly, with the tests named. Every item that changes a
fact in ARCHITECTURE.md edits only its own lines there; N6 lands first
or rebases last.

### Now

| # | Item | Files (lane) | Size | Guard |
|---|---|---|---|---|
| N1 | **Done** (10-21): `Sim` on the production executors with capped task pools; ambiguity ratchet and server/client order tests per game (E1 1-3). No test depended on the old executor (affected crates' full suites twice, 3616 tests each). Step 3 landed as a ratchet: prediction's schedules run every conflicting pair in the same order on server and client, the fixed tick's don't yet (CS:S 56/30/17, Combat Arms 253/5/32 pairs in `FixedUpdate`/`FixedPostUpdate`/`FixedLast`; mostly from the game adding the map plugin first and `NetPlugin` after the parts, and from the presentation's `FixedPostUpdate` systems); ordering them is follow-up work | `chimera/world/src/harness.rs`; new `games/*/tests/it/schedules.rs` | S-M | full suite twice; failures fixed with explicit edges |
| N2 | **Done 2026-10-21.** Game parts without the base: `CsSourceParts`, `CombatArmsParts`, a weapons-only plugin each; both games' parts in one app builds (A1 step 1) | `games/*/src/lib.rs`, `games/*/src/harness.rs`, `apps/mashup_demo/src/lib.rs` | S | the five apps' system dumps identical; new two-games test |
| N3 | **Done 10-21** (note below). Lean map loads: `MapLoadOptions`; servers and collision-only heavy tests without textures; CS:S texture decode on threads (F1) | `chimera/world/src/map/sources.rs`, `games/cs_source/src/game/{bsp,material}.rs`, `games/combat_arms/src/game/world.rs`, test support | S-M | spawn dump identical with textures on; heavy tier sum and server RSS before and after |
| N4 | **Done 2026-10-21.** Map lifetime: map-scoped resource registry, unload test; textures dropped after spawning (A2 1-2, F2) | `chimera/world/src/map/{mod,registration,spawn}.rs`, `chimera/present/src/render/{spawn,images}.rs` | S-M | spawn dump on five maps; `drawn_parts.rs`; new unload test; RSS after load |
| N5 (deferred) | CI on Linux and Windows without installs (T3) (2026-10-24: the user chose to keep relying on the local hooks for now; revisit on request) | `.github/workflows/` | S | the workflow itself |
| N6 **done** | Generated debt and module indexes; Combat Arms' finished briefs archived; survey scripts kept (T5) | `checks/src/bin/`, `docs/tech-debt.md`, `docs/ARCHITECTURE.md`, `docs/plans/`, `tools/arch_survey/` | S | `repo_checks` |
| N7 **done** | App builder: `PresentationPlugins`, the physics setup shared with the harness (A4); `physics_plugins()` is a function returning the group (smallest change), the tick waits for A1; the five apps' system dumps identical | `chimera/present/src/lib.rs`, `chimera/world/src/lib.rs`, `apps/*/src/main.rs` | S | system dumps; dust2 and WAREHOUSE screenshots |

N2 landed as `cs_source::CsSourceParts` (rules, objectives, bots, net part,
Source's logic; `SimPlugins` = base + parts + tick metrics), `combat_arms::
CombatArmsParts` (= `game::CombatArmsPlugins`), and a weapons-only plugin
each (`cs_source::CsSourceGuns` = `CsGunsPlugin`; `combat_arms::CombatArmsGuns`,
a group: data and the hook-registering plugins, with the radar's scanner hook
alone). Test: `apps/mashup_demo/tests/it/two_games.rs`. The five apps' system
dumps are identical before and after. Not done: `harness.rs` files (the
bases are unchanged), the mashup app keeps its own plugin list.

N4 landed as `MapScopedApp::map_scoped::<T>()` (`MapHooks::scoped`;
`unload_map` removes every registered resource; the spawn phases' are listed
in `registration::scoped_resources`, render's in `render::scoped_resources`)
and `MapDrawer::retire`: `spawn_map` takes `PendingMap` out once spawned and
the drawer drops what it needn't keep (render's shadows keep the data for the
map's life and shed the texels of every texture they don't sample,
`shadows::shed_unread_textures`). `MapPlugin::data` is now a
`Mutex<Option<Arc<MapData>>>` the plugin empties in `build` (a plugin keeps
its fields for the app's life, so its `Arc` would hold the textures). The
unload test (`chimera/present/tests/it/map_unload.rs`) found four resources
nothing removed (`TextureAnims`, `ProbeLightScale`, `MapTraceSkip`,
`MapPropBrushStart`). Spawn dumps on the five maps differ only by the absent
`PendingMap`; system dumps identical. RSS after load (headless, one map per
process, whose `Assets<Image>` keep their own copy; a drawn app drops that
after upload): dust2 1887 to 1817 MB, nuke 3040 to 3027, surf_demise 5821 to
5644. Not done: dropping the meshes' vertex data (the shadows' `ShadowSource`
still holds `MapData` whole; X7 splits it), and the code-built Combat Arms
and greybox unload cases.

X7, done (2026-10-21), the smallest sound split. `MapData::kit: GameKit`
holds what a map's game brings to every map (round sounds, radio, bot
profiles, navigation mesh, character bodies, held and view models, decals,
HUD look, muzzle flash, shells and their physics); `take_game_parts` and its
hand-kept list are gone: `MapSources::dress` replaces the kit
(`GameKit::stay(kept)`, which rebuilds it from `Default` keeping only the
kept held and view models), so a field added to `GameKit` is stripped from a
mashup's map with no line to add. Source's effect lists (`steam`, `teslas`,
`strikes`, `embers`, `smokestacks`, `dust`, `detail_props`) moved to
`SourceEffects` behind `MapData::extras` (`MapExtras`, one value per type in
a `BTreeMap` keyed by `TypeId`; `source_fx()`/`source_fx_mut()`), filled by
the CS:S loader and read by the renderer; `trails` and `muzzle_flashes` stay
on `MapData`. `render::shadows::shed_unread` (was `shed_unread_textures`
alone) now also drops the kit, the extensions, the world's meshes, lightmap,
cubemaps, collision and entities from what `ShadowSource` keeps for the
map's life. Spawn dumps on the five maps (and their system dumps) are
byte-identical.

X7, rest done (2026-10-22): a map of another game than the gameplay's no
longer pays for its own game's kit. The kit's indices still point into
`MapData::textures` (one list, appended in load order), so the kit was not
split off the texture list; instead the loaders skip loading it.
`MapLoadOptions::kit: KitLoad` (`All`, the default, or `Only(prefixes)`:
just the held and view models whose keys start with a prefix);
`MapSources::load_with` sets `Only(kept)` whenever the app's gameplay is not
the loader's game, so a mashup's map is its world plus the kept guns, and
`dress` still replaces the rest (`GameKit::stay`, kept as the net for a
loader that ignores the option). CS:S's `bsp.rs` skips character bodies,
hostages, decals, HUD, round sounds, radio, bot names, muzzle flash, shells
and the nav mesh, and loads a kept gun's held and view model only when its
key is wanted (the held ones need the player skeleton: one body is loaded to
take its bones, and not kept); Combat Arms' `load_from_with` skips the
shared character models (`append_shared` takes the `KitLoad`: bodies only
with `parts()`, held models by `wants`). Normal loads (`All`) run the same
code in the same order: spawn dumps on the five maps and their system dumps
are byte-identical to before. The mashup's `de_dust2` (CS:S's map, Combat
Arms' gameplay; one load in its own process, a shared and busy machine, so
the time is rough): 918 to 725 textures, resident 2325 to 2200 MB, 9 to 11 s
to about 6 s (`apps/mashup_demo` heavy `a_mashup_map_loads_only_its_world`
also asserts the kit is empty but the kept guns, which equal a whole load's).
Not done: `AppHooks::load_map` still takes an HDR level only. The mashup's
hook is `MapSources::load`, which sets the kit scope itself; a client always
draws and always wants its own game's whole kit, so the options would add
nothing there. A third game's loader reads the option when it has a kit.

N3 and N4 both reach `MapData`'s consumers but not the same files; if
N3 changes the `MapLoader` signature, N4 rebases on it (one line).

N3, done (10-21; measurements in docs/performance.md, "Lean map loads"):
`MapLoader::load` is `fn(&str, &MapLoadOptions)`
(`chimera/world/src/map/sources.rs`; `MapSources::load_with`); the
options hold `hdr_level` and `textures` only, since `models` and `sounds`
would have nothing to turn off yet (view models already load on demand,
character models once per session, sounds are data). Off, a texture is
a placeholder of the right size (no texels, no mips); the CS:S
loader's `MaterialLoader::lean` and Combat Arms' `world::load_with`
honour it (CS:S's cubemaps and HDR sky, Combat Arms' props, view models
and character models still decode: small). Both dedicated servers load lean;
27 CS:S heavy files (never screenshot, drawn-part or texel tests) use
`games::load_map_lean`. CS:S textures now decode on threads
(`MaterialLoader::finish_textures` on `map::parallel::par_map`, which is
Combat Arms' `decode_all` moved down). Not done: `AppHooks::load_map`
(the client's) still takes an HDR level only, since a client always
draws (X7 left it as is: see its note).

N6, done (10-21), with these choices. `repo_checks --bin docs_index`
(`checks/src/docs_index.rs`; `--check` only reports) generates two things
from every Rust file's `//!` doc: **`docs/modules.md`**, one line per file
(first sentence of its first paragraph, at most 150 characters, files under
`tests/` left out), and the generated block of `docs/tech-debt.md`, one line
per "Known shortcuts" paragraph (its first sentence, 150 characters, file
path first). The module list is a new file rather than ARCHITECTURE.md's
"Modules" section because one line per file is 666 lines and 105 KB, which
would undo P2; that section is now a pointer and ARCHITECTURE.md is 28 KB.
The hand-curated layer tags and the folder-level lines are gone with it
(folders are the headings). tech-debt.md keeps its header and the 17
entries no file's doc carries. The test `generated_docs_are_fresh`
(architecture.rs) replaces `docs_stay_one_line_per_entry`: it fails when
either file is stale and names the command; it compares with CRLF
normalised, so a Windows clone passes (2.5's finding). Combat Arms'
briefs for Waves 1-4 (items 1-22, with their follow-up briefs; 1,765 lines)
moved to `docs/plans/completed/combat-arms-briefs.md`; the board, decisions
and Wave 5's briefs (items 23-29 are still in flight) stay. The survey
scripts of 2.6 were not kept (they were throwaway), so
`tools/arch_survey/survey.py` re-creates them: lines and tests per crate,
function lengths and parameter counts, pattern counts and widest `Query`
tuples, widest `Resource`/`Component`/`Message` structs, workspace
dependency closures, non-merge commits by lowest crate. Its numbers are
by the same method but not those of the first two passes.

### Next

| # | Item | Depends on | Files (lane) | Size | Guard |
|---|---|---|---|---|---|
| X1 | **Done 2026-10-21** (note below). `Gameplay` slots and `gameplay_is`; each game's match-wide plugins gated; tick, binds and HUD from the gameplay; the mashup runs Source's logic and offers CS:S's guns (A1 2-4) | N2, N7 | `chimera/core` (new module), each game's rules, bots, HUD and net-part registration | M | system dumps modulo set names; both suites; the mashup's heavy test on cs_office (doors, a CS:S gun) |
| X2 **done** | Determinism checks: std hash maps and new mutable statics refused in simulation crates (E1 4, E2's check) (done 10-21: `checks/tests/it/determinism.rs`, per-crate ratchets; counts in its `//!`) | N1 | `checks/` | S | `repo_checks` |
| X3 **done** | Process-wide state into resources: Combat Arms' data, effects and model caches, then weapon sets (E2) (done 10-21, partly: `CaData` resource for the install-derived data, `WeaponCatalog` for weapon lookups, the global `weapon::data::lookup`/`publish` and `BUILTIN_ONLY` gone; the loader-filled caches below remain) | X2 | `games/combat_arms/src/game/{data,effects,characters,view_model}`, `chimera/sim/src/weapon/data.rs` | M | Combat Arms' suite; a two-`Sim`s isolation test |
| X4 **done** | (done 10-21, note below) `OwnState` parts; CS:S's money, kit and night vision and Combat Arms' armour as parts; `net_order` sorted once (A3, E6) | none | `chimera/net/src/net/{mod,server,client,predict}.rs`, `games/*/src/game/net.rs`, `chimera/core/src/core.rs` (predicted registry only) | S-M | both games' network suites; `NET_VERSION` bump |
| X5 **done** | (done 10-21, note below) Named `Intent` actions; binds by action; `POLLED` from the game (A3) | none | `chimera/core/src/core.rs` (`Intent`), `chimera/client/src/client/{binds,input}.rs`, `chimera/net` command encoding | M | both suites; bind migration tests; `NET_VERSION` bump |
| X6 **done** | `GameInfo` carries its Steam app and map files; the network map offer per loader (A3) (done 10-21: `GameInfo::steam`, `map_file -> Option`, `net::maps::offer_file`) | N3 | `chimera/core/src/{game_registry,mount/install}.rs`, `chimera/net/src/net/maps.rs`, `games/*/src/game/mod.rs` | S-M | install tests; map download tests |
| X7 **done** | (done 2026-10-21, note below) `MapData` split into the world and the gameplay's kit; `take_game_parts` deleted; Source-only effect lists behind an extension (A2 3) | N3, N4, N2 | `chimera/world/src/map/{data,sources,spawn}.rs`, both loaders, `chimera/present/src/render/spawn.rs` | M | spawn dumps; both heavy map suites; the mashup's heavy test |
| X8 **done** | (done 10-21, note below) Chimera test kit and contract tests; generic tests moved down; per-game test support (T2, T4) | N1 | `chimera/sim` (feature) or `chimera/testkit`, `chimera/*/tests/`, `games/*/tests/*/support` | M | test counts before and after |
| X9 **done** | (done 10-21, note below) BC textures (F3) | N3 | `chimera/present/src/render/images.rs`, the loaders' texture output | M | refcmp on dust2 and de_nuke; Combat Arms screenshots |
| X10 **done** | (done 10-21, note below) A neutral VFS crate under both format crates (A5) | none | `source/formats/src/fs.rs` to a new crate, `chimera/core/src/mount/` | S | `workspace_crates`; format crates' tests |

X9, done (10-21): `MapTexture` gained `bc: Option<MapBc>` (the file's
DXT1/3/5 blocks, every stored level; `MapBcFormat` Bc1/2/3). The loaders
fill it beside the decoded copy: CS:S's `bc_levels` (still textures with
a full mip chain, a size multiple of 4; `level_spans` shared with the mip
decode), Combat Arms' `dtx_bc` (the file's own levels from the loaded
one down, which stop at 4 x 4: the box-filtered chain is not used for
BC). `render::to_image(t, look, bc)` uploads the blocks as
`Bc1/2/3Rgba(Unorm|UnormSrgb)` when `bc_wanted` (the main world's
`CompressedImageFormatSupport` has BC; `CHIMERA_BC=0` forces decoded) and
the blocks fit (`bc_fits`), else the decoded path as before. Kept decoded:
animated frames, water's normal arrays (layers of one array image),
second layers, anything not DXT. The CPU copy is
still decoded at load (surface colours, shadows' alpha, sky, HUD and the
tests read it; `shed_unread_textures` drops it and `bc` after the spawn),
so "texture decode gone from loads" is not done: it needs the loader to
know which textures the CPU reads (follow-up; the GPU side of F3 is). Image
data of the maps' textures, from the new `textures:` log line: dust2 635
to 165 MiB, de_nuke 1039 to 183, Combat Arms warehouse 451 to 56,
junk_flea2 563 to 72; peak RSS dust2 3.0 to 2.1 GiB, nuke 5.1 to 3.4,
warehouse 3.2 to 2.4. Guard (no reference captures on this box: A/B of
`CHIMERA_BC=0` against on): 32 dust2 and 27 de_nuke views identical to
under 0.15% of pixels differing (0.0002-0.001 of pixel-channel error;
at most 300 pixels off by more than 4%, all edges and one-pixel
highlights), the GPUs' DXT decoding against ours; Combat Arms
warehouse and junk_flea2 at four looks: as close as two decoded runs are
to each other (view model sway, effects).

X8, done (10-21): the kit is `chimera_sim::testkit` (feature `testkit`,
on through dev-dependencies only; smaller than a new crate, so no layering
or hakari change): a TOML weapon set in round numbers (`kit:pistol`,
`kit:rifle`, `kit:shotgun`), `kit_sim()`, `with_player`, `active`,
`active_id`, `feet`, `aim_at`. Contract tests: `chimera/sim/tests/it/
{weapon_frame,map_lifetime}.rs` (trigger timing, magazines, reloads,
pellets, damage, draw; greybox spawn points and unload) and
`chimera/net/tests/it/prediction.rs` (zero prediction errors and
bit-for-bit agreement at 50/100/150 ms, loss and jitter converge, own-state
parts round-trip). Moved down: three of `cs_source`'s `noclip_damage`
tests (bullet through noclip, suicide, kill height) to
`chimera/sim/tests/it/noclip_damage.rs` on the kit's rifle; the HE blast
stays. Test support (T4, the smallest sweep): `cs_source`'s
`tests/common::sim()` replaces six identical `fn sim()`; Combat Arms got
`tests/common/` (`sim`, `active`, `active_id`) replacing one `fn sim()` and
two pairs of readers. The other `sim`/`load`/`feet` variants differ in
setup (maps, resources, signatures) and stay; a later sweep lifts them as
tests are touched. `SimBasePlugins` is `Clone` for `NetSim`. Counts
(`cargo nextest list --features dev`): 3,837 before, 3,852 after; per
crate `chimera_sim` 39 to 54 (the contract tests plus the three moved),
`chimera_net` 38 to 41, `cs_source::it` 852 to 849, the rest unchanged; the
only tests missing by name are the three moved. The affected selection
does not yet skip a game's heavy tier for changes the contract tests
cover (a later decision, T2).

X10, done (10-21): the crate is `chimera_vfs` (`chimera/vfs`), no
dependencies: `source/formats/src/fs.rs` moved there whole (`Entry`,
`FileSource`, `Mount`, `LooseDir`, `normalize`, with its tests).
`source_formats` (VPK) and `lithtech_formats` (`.rez`, and its tests) use
it; `lithtech_formats` no longer depends on `source_formats`; `chimera_core`
re-exports it from `mount` as before (and still takes KeyValues from
`source_formats`; Steam's `libraryfolders.vdf` stays on them). Checks:
`chimera/vfs` is in `final-excludes` (plain, like the format crates) and
`NO_HACK`; its `FORBIDDEN` rule bars every other workspace crate;
`source/formats` now bars `chimera_*` except `chimera_vfs`, and the same
exemption lifts the `chimera_` prefix rule for it in `lithtech/formats`.
No `source_formats::fs` alias was kept.

X4, done (10-21): `net::parts::OwnParts` (`NetAppExt::own_part(name,
encode, decode)`, sorted by name as registered); `OwnState::parts` is one
blob (u16 length per part, `u16::MAX` absent) replacing `money`, `armor`,
`kit` and `nightvision`; the client applies only parts that differ from
its player's own encoding. CS:S registers `cs_source.{money,armor,kit,
nightvision}` in its net part (the types stay in `weapon::economy`: moving
them is the shop item in A3). Combat Arms needed none: its armour
(`CaArmor`) already replicates through replicon, not `OwnState`.
`PredictedComponents` keeps its networked list sorted at registration
(`net_order` gone). `movement` stays a `String` (smallest change).
`NET_VERSION` is net28.

X5, done (10-21): `core::ActionRegistry` (`ActionAppExt::intent_action`;
sorted names, at most 32) and `Intent::actions: u32` beside the fixed
fields, which stay as Chimera's aliases; `NetCmd::actions` carries them
(`NET_VERSION` net29). Binds by action: a bind line is unchanged
(`bind mouse4 +throw`, so saved configs and Combat Arms' profile
migrate by doing nothing); `chimera_client::binds::BindsAppExt::
bound_action("throw")` registers the action and polls `+throw`, and
`write_local_intent` sets the bit while a key bound to it is held.
`POLLED` is Chimera's own list plus the `PolledBinds` resource
(`polled_bind`): CS:S's `buymenu`, `chooseteam` and `radio1..3` register
from its UI plugins (Combat Arms' stock binds use none of them; tests pin
both). `console::run_binds` and the options page take the set from the
resource. Smallest-change choices: console-typed holds (`HeldActions`)
and the wheel stay Chimera's own only; the bots and game rules read
`Intent::actions` by name through the registry when a game needs it (no
game registers an action yet).

Lanes in Next: X4 and X5 both touch `chimera_net`'s protocol; run them
one after the other or agree on the file split (X4 owns own-state
encoding, X5 the command encoding). X7 waits for N3 and N4.

X1, done (10-21): `chimera_core::gameplay`. `Gameplay` names the rules,
HUD and binds by game id and the tick (`Gameplay::of(&INFO)`: all one
game's); the apps and `CaSim` set it (`set_gameplay`, which also sets
`Time<Fixed>`, replacing the hand-written tick in the apps; the CS:S app
and server keep the engine's 64 Hz for the greybox), and each game's gated
parts make themselves the gameplay when none is set (`default_gameplay`:
the first game's parts added play). `switch_gameplay` changes it at run
time with the tick. Gating is by run conditions, `gameplay_is(id)` and
`hud_is(id)`, on each match-wide system: CS:S's rules, objectives (not the
C4's predicted arming), bots and the match state of its net part (through
`objectives::plays`, since `rules` and `bot` can't reach `game::GAME`;
the id now lives there), Combat Arms' rules, bots and net part, and each
game's HUD (CS:S's `with_hud` plugins, Combat Arms' `CaHudPlugin` with its
`HudUpdate` and `RadarUpdate` sets). Not a set per game, as A1 proposed:
Bevy removes every system set from the dependency graph with a
swap-remove when it flattens it, so a new set node reordered hundreds of
unordered systems in all five apps; a condition adds no node (a tuple
with a collective `run_if` takes a second one on its existing anonymous
set, one without takes `distributive_run_if`, and the HUD sets'
conditions are configured after the sets exist). Without a `Gameplay`
the conditions are true, so a plugin added alone in a test runs as
before. The shell takes the binds and movement cvars from the gameplay's
`binds` game, the camera's unit from its `rules` game and a loaded map's
tick from the gameplay (each falling back to the default game). The
mashup already ran Source's logic and offered CS:S's guns
(`MashupPartsPlugin`); it now sets Combat Arms as the gameplay. Tests:
`gameplay`'s unit tests; `two_games.rs`: with both games' parts only the
gameplay's rules run and a switch moves them and the tick; the mashup's
heavy `cs_maps.rs` on cs_office (its func_door opened by +use, a
func_breakable_surf window broken by CS:S's AK-47). The five apps' system
dumps are identical (both servers through a temporary dump patch,
reverted). Not done: a gameplay switch doesn't re-read the binds or swap
single-slot hooks (`core::NewGame`, the network's join, bot and hold
hooks) or console commands; a HUD switched off keeps the nodes it spawned
at start; the character kit and weapon sets aren't slots yet (L1, L2).

### Later

| # | Item | Depends on | Size |
|---|---|---|---|
| L1 | The mode runtime (`chimera_sim::mode`) lifted from `CaMode`; Combat Arms' modes, then CS:S's rounds, on it, their spec tests unchanged (D2) | X1 | L (**done 2026-10-22**) |
| L2 | Minigame definitions and primitives; `games/originals` and `minigames/`; `chimera:ladder`; the first original minigames; `apps/lucker_party` and its server (D2, A5) | L1 | L (**done 2026-10-22**; note below) |
| L3 | Party networking: `ChangeMatch`, the match manifest, the mount doctor at join, the lobby's playable set, relevance culling where cheap (M1-M5) | X6, L2 | L  (**stages 1-3 done 2026-10-23**: manifest, doctor, `ChangeMatch`, playable set; relevance culling left; note below) |
| L4 | Asset pipeline: the installs as an `AssetSource` over the VFS, loaders for textures, models and sounds, maps converted in the background behind the loading screen (F6) | X7, X9 | L (**stages 1-3 done 2026-10-23**: sources, texture and sound loaders with first consumers, spawn work prepared on the loading task; models, the remaining consumers and maps as assets left; note below) |
| L5 | Bevy and avian upgrades as their own lane, each release: the workarounds listed (contact flags, the disabled-body flag, the 18 change-detection bypasses), spawn and system dumps compared, full suite | any time, alone | M (**done 2026-10-23** for Bevy 0.20 and avian 0.8; note below) |
| L6 | Opportunistic: giant systems split when touched (E3), the dialog layer (P6), one binary reader (P7), bots' sight in parallel (F4) **done**, `bevy` without default features for servers (A5) **done** | none | S each (two done 10-22, note below) |

L4, stages 1-3 done (10-23):

- Sources (`chimera_present::assets`): each game's install is a Bevy
  asset source named by its id (`cs_source://materials/...`,
  `combat_arms://...`), read through its `chimera_vfs::Mount`
  (case-insensitive, read-only, no `.meta` files), the mount opened on
  the first read (`InstallSource::of(&INFO)`, retried until an install
  exists) or shared with the game (`InstallSource::shared`: Combat Arms'
  `world::mount`). Bevy takes sources only before `AssetPlugin`, so the
  four client apps add `InstallSourcesPlugin` ahead of `DefaultPlugins`;
  the dedicated servers never link it. `Mount::contains` /
  `FileSource::contains` (VPK, rez and loose folders answer without
  reading).
- Loaders: Combat Arms `.dtx` (`ui::textures::DtxLoader`; default as the
  maps upload them, X9's blocks or RGBA8 with mips; `plain` the decoded
  top level), CS:S `.vtf` (`ui::textures::VtfLoader`, the map loader's
  decode and `render::to_image`), `.wav`/`.mp3` as
  `audio::clip_asset::SoundAsset`; `assets::texture_bc` is read in each
  loader plugin's `finish`. Consumers moved: Combat Arms' effect textures
  (`effects::FxImages`: decals, tracers, casing skins, muzzle flashes,
  plain look, so unchanged) and CS:S's interface sounds (menu and freeze
  cam; `GameUi::sound_files` replaces the decoded `sounds`). Heavy
  `install_assets` (cs_source ui tests): a VTF and a WAV through
  `cs_source://` equal the by-hand decode.
- Maps behind the loading screen: the parse and convert already ran on
  the async pool; the spawn's data-only work now does too (`map::prepare`:
  `MapHooks::prepare` jobs made from the world as the load starts, run
  after it, taken by the spawn phases, computed in place without them).
  Prepared: the world's colliders and terrain, the models' colliders and
  hulls, the props' placed brushes (on threads: de_nuke's bevels were
  10.3 s of one frame), the textures', cubemaps' and lightmap's images.
  Swap frames: de_dust2 495 to 166 ms, de_nuke 10.7 s to 510 ms,
  warehouse 244 to 118 ms; the loading screen stays at 60 fps
  (performance.md, "Map changes behind the loading screen"). A client
  joining a listen server loads and joins.
- Guards: spawn dumps of the five maps identical (before/after, and
  after with `CHIMERA_SPAWN_DUMP_PREPARED=1`); system dumps of the four
  clients: stage 1 and 3 identical, stage 2 adds `SoundAsset`'s two asset
  systems in `Last` and Bevy reorders unordered systems of `PreUpdate`
  and `PostUpdate` (fixed, `Predict` and `Update` unchanged); X2 and the
  schedule ratchets unchanged.
- Left: model loaders (Combat Arms' view and character model caches,
  CS:S `.mdl`) and the other by-hand consumers (CS:S's menu backgrounds
  and option pictures, HUD textures, Combat Arms' UI art and music); a
  map as an asset with its textures as dependencies, shared between maps
  (hl2's textures loaded once); the servers' level change and startup
  `--map` still spawn in place; `MapBrush::from_planes`' cost.

L5, done for Bevy 0.20.0 and avian3d 0.8.0 (10-23):

- Dependencies: bevy_replicon 0.45.2, bevy_egui 0.43.1,
  bevy-inspector-egui 0.38, wgpu-types 30, glam 0.33.
  `bevy_replicon_renet` has no Bevy 0.20 release yet: it, `bevy_renet` and
  our `renetcode` come from the authors' upstream commits (git, pinned by
  Cargo.lock; tech-debt.md, backlog.md).
- Shaders: naga_oil is gone, so the 25 shaders with imports or defs are
  WESL (`.wesl`): `import ...;`, `@if(DEF) { }`, `constants::NAME`, Bevy's
  modules at their new paths (`bevy_pbr::render::forward_io`,
  `bevy_pbr::prepass::io`, `bevy_ui_render::ui_vertex_output`), ours as
  `chimera_present::render::{fog, dither}`. Every `Camera3d` keeps a
  `ScreenSpaceTransmission` (0.20 dropped it as a required component; the
  water and sprite shaders read the scene behind), and our
  `Tonemapping::None` cameras are `Linear` (0.20's `None` skips the
  deband dither too).
- Avian 0.8's new defaults reverted to 0.7's (`chimera_world`'s
  `ContactDefaults`, in `physics_plugins`): speculative contacts on every
  body, no automatic swept CCD, `contact_tolerance` 5 mm, no contact
  recycling. With 0.8's, crates rebounded low, ragdoll joints pulled
  apart, a cupboard on a player hurt it twice as much and ragdolls on
  steps never slept. What's left of 0.8's solver (friction solved only in
  its relax pass): a crate dropped flat lands a few units aside, so the
  breakable-on-a-prop test compares with the same prop alone, and an arm
  on a de_inferno step slides off 3.4-3.8 s after the fall, so the ragdoll
  settle test's quiet time is 4 s. Characters stand on their 0.1 u skin
  now (0.7's shape cast let them touch the floor); a Combat Arms bot case
  ran its chase a tick past the moment the target came nearest and was
  shortened.
- Workarounds: `map::contact_flags` removed (0.8 clears the started flag
  itself, avian #1068, `avian_still_needs_the_guard` failed; ported, its
  handle matching broke 0.8's deferred constraint bookkeeping). The
  disabled-body flag is still left on proxies by 0.8 (same `Discard`
  observer): `set_prop_shown` keeps its workaround, now guarded by
  `logic::bridge::tests::avian_still_leaves_body_disabled_on_the_proxy`.
  The change-detection bypasses: 0.8 still wakes a body on a velocity
  write (`wake_on_changed`), so the six in prop_physics and ragdoll (and
  the test's) stay; contact_flags' two went with it; the rest were never
  avian's (UI and net state). Ragdolls' gyroscopic torque is still cleared
  by hand (0.8 has no switch). Bevy UI's `Interaction`/`Button` are
  deprecated in 0.20; the menus keep the state under its undeprecated name
  (`client::widgets::Interaction`, backlog.md).
- Dumps: system dumps of the four clients and three servers (patched
  servers) keep every fixed-tick and `Predict` schedule's order of our
  systems exactly (contact_flags' two systems gone); the per-frame
  schedules (`Update`, `PostUpdate`, `PreUpdate`, `First`, `Last`) order
  our unordered systems differently because Bevy's own systems changed;
  the schedule ratchets and X2's counts are unchanged. Spawn dumps of
  de_dust2, de_nuke, mg_lego_multigames_v2, gg_tbr_water_basin and
  surf_demise: the same entities with the same components and values of
  ours; avian's own components changed (`SolverBodyIndex` for
  `SolverBody`, `BodySizeMetrics`, `ColliderAabbMargin`, the CCD
  components), and a falling prop's first step differs by about 2 mm.
  Screenshots of de_dust2 and WAREHOUSE match (differences at the
  run-to-run level).

L6, P7 done (10-24): `chimera_vfs::bytes::Le` is the one bounds-checked
little-endian reader (`take`, `array`, `u8`..`f32`, `f32s`, `cstr`,
`left`) with a `Truncated { at, wanted, left }` error that converts into
`String` and `io::Error`; `source_formats`' `nav`, `pcf` and `vpk` and
`lithtech_formats`' `ltb` and `world` lost their private cursors and keep
only their format's helpers (`vec3`, `lstring`, `count`, ...) as a small
trait on `Le`, their own errors made `From<Truncated>`. Format tests and
the heavy install tests are unchanged and pass. Truncation messages of
`.nav` and `.pcf` now read "N bytes wanted, M left (at byte K)".

L6, P6 partly done (10-24): `chimera_client::client::dialog::Dialog` is
the shared model of a modal popup (`Kind::Message | Confirm | Input`,
`Dialog::password` masks; `handle(Press) -> Option<Outcome>`; the entry
is `widgets`' `Caret`/`Edit`, with a byte limit). Combat Arms' lobby uses
it for the password box (`LobbyScreen::password_box`, 8 characters,
replacing `join_password`) and the message box's OK; CS:S' server
browser for its password dialog (`Dialog::Password { entry }`). Unit
tests in `dialog.rs`, a lobby UI test for the limit and Cancel. Drawing
stays per game (XML layouts; GameUI widgets), as do the scale clamps.
E3: nothing was touched, so no system was split; the candidates are
listed with their sizes in E3 (`bot::think` 696 lines, Combat Arms'
`movement::step` 634, `apply_effects` 439, `class_input` 394,
`client_commands` 335, `weapon_frame` 324).

L6, two items done (10-22): `bevy` without default features for the
servers (A5) and bots' sight in parallel (F4).

- A5: the workspace's `bevy` and `avian3d` have no default features
  (Cargo.toml lists the simulation's); `chimera_present` and
  `chimera_client` ask for `bevy/default`, `wav` and avian's `debug-plugin`,
  `bevy_picking` and `bevy_scene`, so the clients' features are what they
  were; every simulation crate takes the same as dev-dependencies, so
  `cargo test -p <crate>` still shares the clients' builds;
  `chimera_world` got a `render` feature (the greybox's materials,
  `MapAlpha::shader_alpha_mode`, `MonitorFog::distance_fog`) and a direct
  `wgpu-types` dependency (the texture types the loaders' images use);
  hakari leaves `bevy`, `bevy_internal`, `bevy_render`, `bevy_winit`,
  `bevy_window`, `bevy_input`, `bevy_light`, `bevy_picking`, `wgpu`,
  `wgpu-core` and `avian3d` out of the hack (a hack entry is a normal
  dependency, so the servers compiled them through it).
  `server_builds_without_presentation` now also refuses Bevy's render,
  window and audio crates in a server's tree. Measured (release, `-j 4`):
  `cs_source_server` 689 to 495 packages, 115.3 to 86.9 MB (76.3 to 58.9
  stripped), RSS on de_dust2 after the load 441 to 427 MB;
  `combat_arms_server` 647 to 447 packages, 117.4 to 89.0 MB; a release
  build of both from an empty target 38.5 min (502 crates compiled) to
  32.3 min (185 compiled, 294 from sccache: the times are not like for
  like). The clients' `CHIMERA_SYSTEM_DUMP`s and the two servers' dumps
  (a temporary patch) are identical to before. docs/performance.md,
  "Dedicated servers without Bevy's default features".
- F4: `think` starts with a pass that finds each bot's nearest enemy in
  sight with `par_iter_mut` on the compute task pool, into
  `Bot::sighted` (the loop after it takes it); each bot's result is its own, so the pick is the serial
  loop's in any thread order (`bot_sight::a_crowd_sees_the_same_on_any_executor`
  checks it bit for bit against the single- and multi-threaded executors).
  No system added (a first version as a system of its own reordered the
  unordered ones around it in the CS:S dumps), so the five apps' dumps are
  identical to before. Combat Arms' bots are one
  exclusive system with shared state and stay serial. Measured with
  `heavy::net_capacity` (a timer around the sight code, de_dust2, 4
  clients): 64 characters 186 to 121 us a tick, 256 characters 1,842 to
  1,019 us (-45%); at 32 characters the sight is 3-5% of the tick and the
  run to run noise at load is bigger than the change; tick instructions
  (all threads) equal to within 1-3%. The minigame lobby case (the 25 ms
  tick) is not measurable here (its map is not on this box).
  docs/performance.md, "Bots' sight in parallel".

L1, done: stages 1 and 2 (10-21), stage 3 (10-22, below).
`chimera_sim::mode` is the runtime: `Mode` (id, key, name, respawns,
warm-up and its revives, a forced setup, `Teams`, `on_death`,
`time_out`, `match_end`, `Hooks`: setup, update, touches, round over,
round end, died, may respawn, holds joiner, before spawns, after round),
registered by id under its game (`Modes`, `ModeAppExt::add_modes`; the
playing game is the gameplay's rules game, else the first registered;
its mode the one `ModeGame::chosen` names; `chimera_modes` lists them);
the match (`Match`, `Phase`, `RoundResult`, the two sides' ids 1 and 2,
free-for-all ids, `Death`, `classify`) and the phase machine
(`mode::flow`: `mission_update`, `update`, `touches`) calling the game's
side (`Flow`: frame, players in join order, respawn, `Span` lengths,
`FlowEvent` steps) and the mode's hooks. Contract tests on the test kit
(chimera/sim/tests/it/mode.rs). Combat Arms runs on it: `CaMode` is a
`Mode` (`CaMode::rules`, read through `Deref`, so `mode.key` and the
spec tests read as before) plus its own fields (spawns, panel, order,
friendly fire, assists, timer, death drops, `leaving` and `left`, which
take its `Leaver`/`LeftPlayer`); `CaMatch` is the runtime's `Match`
(a type alias, the tests' `CaMatch { .. }` unchanged); `flow::FLOW` is
its side (20 Hz frame, `missions`' lengths, map messages, clean-up,
statistics). Every Combat Arms test passes unchanged; the five apps'
system dumps (servers through a temporary dump patch, reverted) keep
their order, three systems renamed (`rules::mode_update`,
`flow::mission_update`, `rules::mode_touches` to
`chimera_sim::mode::flow::{update, mission_update, touches}`).
Smallest-change choices: the match keeps Combat Arms' counters as the
generic ones (round kills and deaths, returns, hold, immunity: a
scoring table is L2's); the runtime's systems are chained by the game
into its rules' set (a runtime plugin adding them would reorder
unordered systems, X1's lesson); joining, leaving, the freeze's intent
and the network view stay Combat Arms'; `CaStats` isn't generalised;
rule modules (`rule_module`) stay beside the registry.

Stage 3, CS:S's rounds on the runtime (10-22). The runtime's new
per-game options are `Flow::options` (`FlowOptions`; Combat Arms'
`FlowOptions::MISSIONS` keeps its behaviour): `slack` (the phases' end
times compared as `now + slack >= until`: CA 1e-9, CS:S 0),
`wait_for_players` (false: the first step starts the match, no
`Starting`), `timer_ends_round` (false: the timer only tells the new
`Hooks::decide`, called every live step before the end test, so a round
it decides is over in the same step), `direct` (round over's end goes
straight into the next mission start in the same step: the round end's
steps, `match_end`, `after_round`, no intermission; a result there is
`MatchOver`) and `count_at_start` (`Match::round` counts rounds
started); also `Teams::AsJoined` (the runtime moves nobody),
`Match::live_since`, `Span` lengths in f64 (so CS:S's `round_time as f64
* 60.0` stays bit-identical; CA converts its f32s), and
`flow::step_game` with `mode::current_of` (a game steps its own chosen
mode from its rules' system, whichever game plays). Contract tests on
the kit for each (chimera/sim/tests/it/mode.rs, two new). CS:S
(`rules/rounds.rs`): three modes by the map's objectives,
`cs_source:{bomb,hostage,elimination}` (the rule modules' names;
deathmatch is no mode, the runtime idles), `FLOW` (every tick, exact,
started at once, decided each tick, direct, counted at start; players
as the old round start's query, respawn `put_at_spawn(fresh or dead)`,
lengths from `RoundSettings`), hooks for the start money and bonuses
(`event`: `Prepare`, `MissionStart`), the round's setup (`mp_tkpunish`,
bots' shopping, loose weapons, `objectives::round_start`), `decide`
(`live_outcome`), `round_over` (money, `RoundEnded` with its summary, a
limit reached: the game end at once), `match_end` and `after_round`
(the game end as the pause runs out, else team balance). `run_rounds`
stays the rules' system (so no system moves): `mp_restartgame`, the
map's clock, the game end's intermission, deathmatch and the broke's
money around `step_game`, then `RoundState` as a view of `Match` (phase,
number, wins, written only on change; kept while `MatchOver`; round wins
written into it outside the rules, by `game_score` or a test, are
taken back at the next step; a view at `Off` with the match elsewhere,
another game's after a gameplay switch, starts the match over), then
the buy window, freeze and `RoundOpen` from it. `RoundState`'s own
fields (losses, draws, payouts, restart, map start, game over) stay
CS:S's. Every CS:S round, objective and economy test passes unchanged
(cs_source's whole suite, heavy included); Combat Arms' suite passes;
the five apps' system dumps are identical (servers through a temporary
dump patch, reverted). Left (L2): the game end held at round over (a
limit reached as a round ends) leaves the match in `RoundOver` while
the runtime's `MatchOver` comes only from the pause's end; both games
share one `Match` (a gameplay switch doesn't save the other's);
`RoundState` stays a separate view rather than the HUD and `NetRound`
reading `Match`.

L2, stage 1 done (10-22): `chimera_sim::minigame`. A definition is TOML
(`def::MinigameDef`, D2's shape: id, name, players, map pool, teams, time
limit, `[slots]` hud/binds/tick game ids plus movement, character and
the weapon sets allowed, `[spawn]` respawn delay and protection, points
(`map`, `far_from_enemies`, `team`) and a kit, `[rules]` module plus its
own keys, `[modifiers]`), parsed at run time (`add_minigames`,
`minigame_load <file>`) and checked against the app (`check`: the module,
its parameters, every weapon in the catalogue and an allowed set, the
slots' games and movement, the modifiers). Small choices: a minigame is
the gameplay's rules slot `MINIGAME` ("chimera"), played on the mode
runtime by Chimera's own `Flow` (`runtime::FLOW`), so neither game's
rules run; a rules module is a `Mode` plus `check`/`start`/`loadout`
(`RulesModule`, registered by name); one round a match, live at once (no
setup), its winner the match's, then round over (5 s), intermission (3 s)
and match over (`minigame_restart` again); the match waits for
`players.min`, joiners play at once meanwhile; protection is `core::God`
under a `Protected` marker, ended by time or the first shot; deaths count
in `core::Score` (the scoring table stays kills and deaths); definitions
are leaked to `&'static` (few, loaded once). `chimera:ladder`
(`minigame::ladder`): free-for-all, an enemy kill a rung up (weapon
swapped at once), `demote_on = "melee_death"` a rung down, the last
rung's kill wins, the time limit's winner the highest rung then kills (a
tie a draw); events `rung_up`, `rung_down`, `match_won`
(`MinigameEvent`, bound to no HUD yet). Contract tests on the test kit
(chimera/sim/tests/it/minigame.rs). Not done: objective primitives,
built-in modifiers, a scoring table, events on a HUD.

L2, stage 2 done (10-22): `games/originals` (crate `originals`, Chimera
only: checked, A5) holds the original minigames' definitions, compiled
in from the repository's `minigames/` (a test keeps the list and the
directory equal; the hooks' test selection maps `minigames/` to it), and
will hold rules hooks no built-in module covers (none: Gun Game is
`chimera:ladder`'s). The first original, `originals:gun_game_dust2`
(minigames/gun_game_dust2.toml): de_dust2 then Warehouse, free for all,
10 minutes, Combat Arms' HUD, binds, tick and movement, both games'
weapon sets, a ten-rung ladder alternating CS:S's and Combat Arms' guns
(Glock to AWP), the Combat Arms knife as the kit, a knife death a rung
down. `apps/lucker_party` (crate `lucker_party`) composes it: Combat
Arms' simulation, CS:S registered with its maps, Source's map logic and
CS:S's guns, `MinigamePlugin` and `OriginalsPlugin` (`PartyPartsPlugin`),
the mashup's `MapSources` (its surface table read from
apps/mashup_demo/data: one table for both apps); the presentation behind
its default feature `present`. Scenario tests on `CaSim` with
`PartyPartsPlugin` and scripted soldiers (apps/lucker_party/tests/it/
gun_game.rs): the definition checks against both games' parts and takes
the rules slot; a full gun game shot through the weapon frame with every
rung's gun runs to the winner and the match's end (kills with CS:S's and
Combat Arms' guns); a death to Combat Arms' knife costs a rung.

L2, stage 3 done (10-22): `apps/lucker_party` is the client (binary
`lucker_party`: N7's `PresentationPlugins`, `physics_plugins()`, Combat
Arms' gameplay until the command line's `+minigame` takes the rules slot;
without one the gun game on its pool's first map, three bots) and
`apps/lucker_party_server` its dedicated server (`lucker_party` without
`present`, `+minigame <id>`, `+map`, `changelevel`; checked by
`server_builds_without_presentation`). Small choices: a `bots` slot on
`Gameplay` (`gameplay::bots_are`; `Gameplay::of` sets it to the game,
so the other apps are unchanged), which Combat Arms' bots now gate on, so
a minigame naming `bots = "combat_arms"` has them play under Chimera's
rules; the party turns Combat Arms' modes off (`ca_mode none`: a joiner
comes in at a free point and the minigame takes it); Combat Arms' bots
read CS:S's guns' AI rows from the app's data
(apps/lucker_party/data/bot_gun_types.toml); `far_from_enemies` prefers a
free point. Tests: the server on the greybox (fast) and, heavy, on
de_dust2 from the installs (soldiers at dust2's points, a Glock kill and
the Beretta next); Combat Arms' bots play the gun game on the greybox and
kill each other. Polish (10-22): the HUD shows the minigame
(`minigame::hud::MinigameHud`: the rules module's status lines, time,
notices from `MinigameEvent`s, `{player}`/`{weapon}` placeholders the HUD
fills; Combat Arms' `draw_minigame`), CS:S's guns show CS:S's own names
(`weapons::display_names` into `ItemNames`), have view models (Combat Arms'
driver lets go of a character holding another game's gun, Chimera's
generic driver plays it with CS:S's `ViewAnimRules`) and the kill feed
names the local player by the `name` setting (never "Character"). Left
for later: Combat Arms' guns show their ids as names, the bots keep their first
life's reach, the map pool doesn't rotate, a client joins a party server
by id only (L3's match manifest), objective primitives and modifiers
(D2) and more original minigames.

L2, identity follows the weapon (10-22): a minigame option, `[slots]
identity = "weapon"` (on in the gun game): each player is the game of the
weapon it holds (its id's `game:` prefix), switching whenever that changes
(a rung's swap, the knife, a pickup, `give`). Generic in Chimera
(`chimera_sim::identity`, `slots::FollowsWeapon`, `slots::GameIdentity`);
no game names the other; the party composes both.
Movement: switched in the predicted tick right after the weapon selection
(`follow_weapon`, `MovementRegistry::of_game`), from the predicted
inventory only, so server and client switch on the same tick; the client
now takes a server's movement change with the state it came in
(`predict::reconcile`, before the decode), not on receipt; a server-made
swap counts in `core::Rebased` (`OwnState::rebased`), a restart for the
client, not an error. Body: `GameIdentity` (server's, replicated) picks
the map's model among its game's (`MapCharacterModel::game`,
`CharacterModels::for_identity`; none: the shell's capsule); Combat Arms
dresses, animates and leaves corpses only for its own identity, CS:S's
animation drives only CS:S's bodies (`CharacterModels::is_of`);
`KitLoad::Players` / `MapSources::players` keep the map game's player
bodies (after the gameplay's) and HUD look on the party's CS:S maps. HUD:
`hud_follows_identity` makes the local player's identity the `hud` slot;
the shell's `client::hud::HudSwitchPlugin` keeps only the active HUD up
(`HudRoot`s hidden, `GameDraws` from `HudDraws`, the map's HUD look put
away while another game's HUD draws, `HudLookOwner`), CS:S's HUD parts
tear down on the look's removal whatever the HUD (X1's "a HUD switched
off keeps its nodes" fixed for both HUDs); the minigame's panel is on
both (CS:S's `ui::minigame_hud`). Tests: lucker_party's `identity`
(rung-ups and the knife: movement and body; NetSim with two clients:
zero prediction errors, every compared tick bit for bit, the other client
sees each new body) and `hud` (the HUD swap with the panel on both).
Left: on Combat Arms' maps (Warehouse) CS:S's bodies, HUD look and gun
models aren't there (a CS:S player is a capsule with the shell's plain
HUD); CS:S's radar, spectator panel and target ids aren't in the party;
(a network client's minigame panel was empty here until L3: it now
plays the server's minigame, `minigame_net`, and its panel follows the
server's state, `net_gun_game`); CS:S's team models are by team (free for all: everyone but team 1
wears the CT model); the shell's binds and movement cvars stay the
gameplay's `binds` game's.

L3, stages 1-2 (10-23): the match manifest, the mount doctor at join and
`ChangeMatch`. `net::manifest::MatchManifest` (the map; the games the
match reads, the map's game required, the others the minigame names
wanted; the gameplay's slots; the weapon sets; the minigame's id, the
SHA-256 of its definition's text and the text) goes in `Welcome` and
`ChangeLevel`, and alone in the new `ChangeMatch` event when the match
changes on the same map (`manifest::announce`, called by `minigame_net`
when the minigame changes). Parts add to it and check it through two new
`NetHooks` lists, `manifest` (the server's) and `accept` (a client's).
The mount doctor's report (`MountReport`: per game installed, its Steam
build, notes such as a missing key; `manifest::examine` for the registered
games, `lucker_party::mount_report` adds Combat Arms' keys) is the app's
(`LocalMounts`; Lucker Party's client makes it at start). Choices: the
report travels in the `Join`'s userinfo (`_mounts`, the installed games
and builds) rather than a new `Join` field, so a build without it still
joins and `Join` stays readable by other builds; only the map's game is
required (without the other games the match plays, looking wrong, as
`lucker_party::missing` already said); a client without a report is
taken as having everything (the faithful apps); the server refuses at
join with the exact games missing ("This match needs CS:S (cs_source)
(its map ...), which isn't installed on your machine: ..."), a client
checks every manifest before loading and leaves with
`JoinFailure::Mounts`; a client plays the server's definition when its
own has another hash or is missing (`minigame::add`, which checks it
against the build: a module or modifier the build lacks is a refusal,
"The server's minigame <id> can't be played by this build: ..."), and
starts a replicated minigame only once a manifest names it.
`minigame_net::change_match` (console `change_match <id> [map]`, the
servers' and a listen host's) starts the minigame (players kept and
connected, scores anew: `minigame::start`), on `map`, else this map if in
the pool, else the pool's first (through the app's `changelevel`).
`NET_VERSION` net31. Tests: `apps/lucker_party/tests/it/net_party.rs` (the
manifest at join, the server's definition played by clients without it, a
refused client lacking a required game, a client that can't play the
minigame leaving before loading, `change_match` with two clients kept
connected, scores reset, there and back) and `manifest`'s unit tests.
Not done: the hash of the files the loaders read (M3: installs and builds
only); the report is checked at join and by clients on each manifest, the
server never re-checks a joined player.

L3, stage 3 (10-23): the playable set. `minigame_net::playable` checks each
registered minigame, on its pool's first map, against every connected
player's report (from its userinfo; a player without one and bots don't
count against it); the minigames every player can play go to the clients
as `PartyOffer` (a server message, sent when the set or the players
change; a client keeps the newest as a resource, the lobby's list) and
console `party_offer` lists them (on the server, the others with who lacks
what: "(originals:gun_game_dust2 on cs_source:de_dust2: <player> lacks CS:S
(cs_source))"). `change_match` refuses a minigame not every player can
play on its target map ("not every player can play <id> on <map>: ...").
Choices: "can play" is the required games only (the map's), so the same
minigame on the greybox is playable by everyone; M3's other option (let
the party pick and make the players without the game spectators) isn't
built, nor a lobby UI (the offer is data and a console list). Test:
`net_party.rs`'s `the_party_offers_only_what_every_player_can_play` (one
client lacking CS:S: the gun game not offered and refused, playable on the
greybox, offered once that client leaves). Live (dev box, port 28961):
`lucker_party_server +map greybox` and one `lucker_party +connect`; the
client's mount doctor logged both installs (CS:S with its Steam build),
`party_offer` listed the gun game on de_dust2, `minigame_load` of a short
ladder on the server only, then `change_match test:short_ladder`: the
client logged "the server changed match" and "playing the server's
definition", stayed connected, and the server's `minigame_status` showed
it on the new ladder's first rung.

L2/L3 polish (10-24): the gun game's rotation, results and CS:S bodies on
Warehouse.
- Rotation: `MinigameDef::rotate` (default true; set in the gun game's
  TOML). `minigame_net::serve_minigame` (a server or a standalone game,
  `FixedLast`) waits `runtime::RESULTS_HOLD` (8 s) of `MatchOver`, then
  picks the pool's next map in turn that every connected player can play
  (`playable_on`; this map last) and calls `change_match`, so clients
  follow on the same connection (a new manifest, or `ChangeLevel` to the
  next map), the scores anew; `rotate = false` restarts on this map. A
  standalone game (no server) loads the next map with `map`, not
  `changelevel`, and the minigame goes on (live: the gun game's short
  match on Warehouse, results, then de_dust2 and a new match).
  (10-28: the next map is picked as the hold begins, `minigame::
  NextMatch`, replicated in `NetMinigame::next`, for the shell's
  match-end screen `client::match_end`; the change still comes after
  the 8 s.)
- Results: `minigame::results::MatchResults` (title, ladder length, a row a
  player: name, kills, deaths, final rung, winner first; sorted winner,
  rung, kills, fewest deaths), collected by `results::record` as soon as
  the match has its result (the intermission), gone at the next
  `restart`; it rides `NetMinigame::results` to clients (names from
  `NetCharacter`) and into `MinigameHud::results`, so both HUDs show it:
  CS:S's `minigame_hud` (`PanelLine::Result`) and Combat Arms'
  `MinigameView::results`, in the place of the notices.
- CS:S bodies on Combat Arms' maps: `MapSources::guests` (a `MapDressing`
  per guest game, called after the gameplay's dressing on maps of other
  games): `lucker_party::CS_SOURCE_PARTS` -> `cs_source::game::
  dress_guest` -> `bsp::dress_guest` reads CS:S's player bodies, the kept
  guns' held and view models and (on a map without one) the HUD look from
  the install with a `MaterialLoader::guest` whose texture and cubemap
  lists begin past the map's own, so the models' indices are the map's
  once the new textures are appended (no remapping). The kit loading
  was split (`add_bodies`, `add_guns`) with CS:S's own maps unchanged. A
  free-for-all player of an identity's game with interchangeable skins
  wears one by its team id (`CharacterModels::varied`), not the one
  counter-terrorist model. Tests: lucker_party's `rotation` (NetSim: the
  results on both clients, the next map, scores anew, rotate off) and
  `results_hud` (both HUDs), the sim's `minigame` results rows,
  `sources`' guest unit test and, heavy, `lucker_party_server`'s
  `warehouse` (a Glock holder on Warehouse wears a CS:S body, a varied one
  per player).
- Left: the guest kit is read from the install at every Combat Arms map
  load (a cache would share its textures between loads); CS:S's radar,
  spectator panel and target ids; a vote or lobby for the next map.

L2 playtest fixes (10-24): a switch of game seats the new movement on
the old one's feet (`slots::Footing`, `MovementImpl::footing`/`seat`,
called by `set_movement`; each movement frees and grounds its hull in its
next predicted step: CS:S `Mover::reseat`, Combat Arms `reseat`; a
crouched Combat Arms player switching to CS:S stood 7.6 units in the
floor); the kit by game (`SpawnDef::kit` a list or a table by game,
`runtime::refit_kit` on a rung's swap: the gun game's knife is the gun's
game's); Combat Arms' camera rise only on its own movement (it smoothed
every CS:S jump and step: "floaty"); CS:S's sounds in the guest kit on
Combat Arms' maps (`bsp::add_guest_sounds`: CS:S guns were silent on
Warehouse); Combat Arms' bodies under a minigame go as in its like mode
(free for all: One Man Army's 5 s after the respawn). Tests:
lucker_party's `switches` (feet, walls, air, kit, knife demotion with
both knives, CS:S movement parity tick for tick, bodies, slots, fire
sounds), `identity`'s NetSim with client-predicted switches standing and
crouched, `lucker_party_server`'s heavy `cs_source_guns_sound_on_both_maps`.

L2 identity fixes (10-24): a bot that died as a Combat Arms soldier
(`characters::drive` hides the player object for its death body) and
respawned holding a CS:S gun stayed `Visibility::Hidden` for good, its CS:S
body with it ("bots go invisible"): showing again no longer asks whose
identity it is (`object_visibility`). CS:S's view-model bob and sway
(`view_motion`, `cl_bobcycle`, `cl_wpn_sway_*`) run in the party for the
local player while Combat Arms' driver has let go (`view_anim::add_motion`).
Layered binds (`Console::layers`, `bindgame`, `unbindgame`, `bindgamedefaults`,
saved in config.cfg): the shared binds under the identity's game's own, read
by the local input before the intent is sent (`Console::effective_binds`);
`lucker_party::GAME_BINDS` ships Combat Arms' (Shift sprint, C walk) and
CS:S's (Shift `+speed` walk, Ctrl duck); a later game adds a row list in
`controls::GAMES`. The Controls page (`F6`, `controls [tab]`;
`lucker_party::controls`): Shared first, a tab a game, the wheel as a bind.
Tests: lucker_party's `controls`, chimera_client's `a_games_own_bind_beats_
the_shared_one`, combat_arms' `visibility_tests`.

Left of L3 (stage 4, M5): relevance culling. Not cheap here: replicon's
per-client visibility removes a hidden entity on the client (its body
despawns and comes back), so culling needs the client's handling of a
body that leaves and returns (held, drawn at once, the soak's
bodies-heard check) and per-game rules for what a client still gets of
players it can't see (footsteps and gunfire through walls, radar: a spec
per game), as `server.rs`'s known shortcuts say; a lower update rate for
distant bodies needs per-client, per-entity mutation filtering replicon
doesn't offer (`pace_messages` drops a client's mutations per tick, not
per entity). At party sizes (8-16) bandwidth isn't the limit (~17 KB/s a
client at 32 players). Next step when it matters: measure per-client bytes
on de_dust2 with 16 players (`heavy::net_capacity`), then PVS culling of
`NetBody` with the client-side hold, per the spec.

X6, done (10-21), with these choices. `GameInfo::steam` is an
`Option<SteamGame>`; `install::STEAM_GAMES` and `steam_game` are gone, and
`resolve`/`resolve_with` take the `SteamGame` as an argument, so the mount
layer stays below the games. `LocalConfig::game_path(id)` now searches only
the config and the saved settings (no Steam); `game_path_in(id, steam)` and
`GameInfo::install()` add Steam, and CS:S's own code uses the latter (the
CS:S's tests and tools call `game_path_in(GAME, INFO.steam.as_ref())`, so
they still find a Steam-only install). `GameInfo::map_file` returns `Option<String>`: the map's downloadable
file under a content folder, None for a map offered by id and hash only.
Combat Arms returns None, which replaces its `worlds/<name>.dat`
approximation (its worlds live in several `worlds*` archives and are read
through the mount: `world::find`). `net::maps` finds the file with
`offer_file` (the id's game prefix, else the default game, from `Games`);
cache path, `.part`, HTTP URLs (`<url>/<file>.bz2`) and messages derive from
it. A map with no file is served without bytes, never cached or
downloaded, and a client missing it fails as missing. A net-only app with no
`Games` registered cannot download maps.
