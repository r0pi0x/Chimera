# Architecture for iteration speed

Goal: decide which changes to the engine's architecture would shorten the
loop from an edit to knowing it works, from measurements, and lay out a
staged path whose first step is small and reversible. The question (user,
2026-11-21): "Is there a way we can change the architecture of the engine
to increase our iteration speed? I'm open to more thorough changes [...]
pluggable systems, like plugins for each game." Done means: each stage
below has a number it must move and is kept or reverted by that number.

This plan is the architecture's side. The workflow's side (hooks,
post-push, waiting) is [iteration-speed.md](iteration-speed.md), the test
cycle's is [test-speed.md](test-speed.md), the cost of game number 30 is
[architecture-pass.md](architecture-pass.md) (its section 7 already
sketches a module API; nothing here contradicts it).

**Short answer.** Yes, but not by loading games as dynamic libraries,
WASM or scripts: those remove none of the costs measured below and would
mean rewriting the games against a host API. What costs time is that
almost every change sits under almost everything: the median Rust commit
can affect 26 of the 44 crates, and a one-constant edit in `chimera_sim`
spends 14 s on itself and about 200 CPU-seconds recompiling and relinking
crates in which nothing changed. The plugin boundary that pays is Cargo's:
a game as two plug-in crates (simulation, front end) that depend on the
engine's contracts, with the hot engine code and the apps above them.

## 1. Measurements (2026-11-21, main at 55277a2d1)

The Linux dev box (12 threads, 31 GB), an agent slot (mold, incremental
builds; sccache was down during the runs), with three other agents and
the post-push tests running: **1-minute load 20-55 throughout, so wall
times are 2-4 times what an idle box gives**; the per-unit seconds
(`cargo --timings`) and the counts are the comparable part. Edits were a
private function appended to a `lib.rs`, then a change of the constant
it returns.

### 1.1 Where agents' time goes

From [iteration-speed.md](iteration-speed.md) (339 agents, 7 days, 374 h
accounted for): thinking and writing 39%; tests, the commit hook and
post-push together 34% (125 h); game runs and captures 11% (40 h);
**builds 5% (19.6 h)**. Compile time as such is the small part; what an
architecture change can move is how much is built, checked and tested
per change (the 34%, and the box's load, which slows everything else).

### 1.2 What changes, and how far it reaches

`tools/arch_survey/survey.py radius 14` (1,016 non-merge commits in 14
days):

| Commits | Count | Share |
|---|---:|---:|
| touching Rust source | 726 | 71% |
| tests only | 67 | 7% |
| docs, specs, tooling only | 219 | 22% |
| data only (`.toml` and the like) | 4 | 0.4% |

File changes: 4,351 Rust source, 2,010 Rust tests, 1,664 docs, 502 data
(TOML and text), 239 specs. Data changes ride along with Rust changes; a data-only
commit is rare.

Of the 726 commits touching Rust source, by the lowest layer they touch:

| Lowest layer touched | Commits | Share | Crates whose tests it can affect |
|---|---:|---:|---|
| Chimera's base (`core`, `world`, `sim`, `assets`, `vfs`, `formats`) | 329 | 45% | 28-40 of 44 |
| `chimera_net` | 35 | 5% | 26 |
| the client side (`client`, `present`, `ui`) | 131 | 18% | 23-24 |
| a loader (`source/`, `lithtech/`, `blam/`) | 35 | 5% | 10-19 |
| games only | 165 | 23% | 7-11 (93 of the commits are Halo's alone) |
| apps only | 31 | 4% | 1-5 |

The median commit can affect **26 of 44** crates (mean 22.4) while
touching 2. 68% of Rust commits edit a Chimera crate; 48% of those also
edit a game in the same commit, and 84% of the commits to the seven hub
crates below add or remove a line declaring a `pub` item or field (a
proxy, from `git log -p`: the engine's interface moves with almost every
engine commit, because features are lifted into it as the second game
needs them).

The hubs (`survey.py closures` counts the other direction):

| Crate | Source lines | Crates above it |
|---|---:|---:|
| `chimera_core` | 15.0k | 32 |
| `chimera_world` | 36.1k | 29 |
| `chimera_sim` | 41.0k | 27 |
| `chimera_net` | 28.1k | 25 |
| `chimera_present` / `chimera_ui` | 35.3k / 11.9k | 23 |
| `chimera_client` | 61.1k | 22 |
| a game (`cs_source`, `combat_arms`, `halo_ce`, `hl2dm`, `tf2`) | 14.9k-110.1k | 6-10 |
| `party_kit` | 29.9k | 4 |

Inside the base, the edits are concentrated: `chimera_world::map` 278
file changes, `chimera_sim::minigame` 169 (18.3k lines of the crate's
41k), `chimera_sim::weapon` 137; all of `chimera_core` 109.

Two edges make the graph taller than the layering needs:

- **Every game's simulation is built with its front end.** A game's
  `ui` is a feature of the game's crate, and its tests turn it on (a
  dev-dependency on itself) so that every selection shares one build
  (docs/performance.md, "Iteration speed"). So `tf2`'s simulation tests
  sit above `chimera_client`, `chimera_present` and `chimera_ui`, and the
  dedicated servers build each game a second time without `ui`. The seam
  is clean in the source: outside `src/ui/`, 0-2 files per game name the
  client or the presentation, and 1-6 of a game's 39-119 `tests/it`
  files do.
- **The party kit names every game.** `party_kit` depends on the six
  game crates (159 references in 8 of its 36 source files; 709 in its
  tests), so any game edit rebuilds the kit and selects its suite and
  Ultimate Gun Game's.

### 1.3 Edit, build, test

| Loop | Wall (load) | Units' seconds | What ran |
|---|---|---|---|
| Nothing changed, `cargo test -p tf2 --no-run` / `-p ultimate_gun_game` | 2.0 s / 3.8 s (43) | - | fingerprints |
| One function in `tf2`, its test binaries | 30.9 s (43-49) | 44 | `tf2` 11.8 s, its two test targets 16 s each |
| The same edit, up to Ultimate Gun Game's tests | +51.6 s (49-54) | 108 | `party_kit` 18.2 s, `ultimate_gun_game` 5.4 s, four link targets 17-28 s each |
| A comment in `tf2`'s `data/blood.toml` (compiled in with `include_str!`) | 17.4 s (20) | 23 | the same three units as a Rust edit |
| `tf2`'s fast tier (289 tests; 2 of `viewmodel_sounds` failed in both runs, edit or no edit) | 48.5 s (50); 10.8 s (20) | - | the tests |
| The commit hook after the `tf2` edit (`.githooks/pre-commit --worktree`) | 544 s (47 falling to 20) | - | type-check of 8 packages' targets (the slot's first check build since a merge), then 2 packages' fast tier |
| One function in `chimera_sim`, up to Ultimate Gun Game's tests | 92.4 s (35-45) | 245 | `chimera_sim` 14.2 s; 13 crates above it 148 s; four link targets 82 s |
| One constant in that function, the same | 87.2 s (35-52) | 219 | `chimera_sim` 14.5 s; 13 crates above it 136 s; four link targets 68 s |
| The commit hook after the `chimera_sim` edit | 704 s (41-59) | - | type-check of 29 packages' targets (warm: the second check build), then `chimera_sim`'s 294 tests (48 s) and `repo_checks`' 46 (83 s) |
| A slot's first build of one game's tests after a one-commit merge | 973 s (43-76) | 2,344 | the chain `assets` to `tf2`, 11 crates |

After the `chimera_sim` constant changed, per crate: `chimera_net` 14.0 s,
`chimera_client` 17.0, `source_engine` 6.5, `source_logic` 4.9,
`source_gameui` 7.0, `tf2` 9.1, `halo_ce` 16.3, `combat_arms` 25.9,
`cs_source` 11.2, `hl2dm` 11.8, `party_kit` 7.8, `ultimate_gun_game` 2.1,
`originals` 2.1. None of their code changed. rustc has no stable
interface between crates: a crate is rebuilt whenever a crate under it
is, whatever changed there, and each rebuild rewrites its library
(`combat_arms`: 310 MB) and every binary above (300-660 MB each, 72 test
targets and 34 binaries in the workspace).

The hook's time is its type-check of everything above the change, not
the changed crate's tests: after the `chimera_sim` edit, 131 s of the
704 s were tests (run times as printed, at that load); the rest was
`cargo check --all-targets` of 29 packages, every game's test crates
among them (55k-86k lines each for `cs_source` and `combat_arms`).

### 1.4 After the push

`.claude/post-push/history.txt`, 352 runs since 2026-11-07 (189 h of
machine time; 102 failed, 29%). Passing runs by what the push selected:

| Selection | Runs | Median |
|---|---:|---:|
| the base (`core`/`world`/`sim`/`assets` in it) | 107 | 24 min |
| the client side without the simulation | 34 | 19 min |
| games and the party kit | 17 | 13 min |
| `repo_checks` only (docs) | 52 | 3 min |

Nobody waits for these any more, but they are the box's load: the
measurements above ran at load 20-55 on 12 threads.

### 1.5 What the games are made of

Game code is Bevy systems: the smallest game (`tf2`, 14.9k lines) has
120 `Query`, 102 `Res` and 33 `MessageReader` parameters; the five games
are 224k lines of source and 191k of tests, about 4,100 fast and 700
heavy tests, most of them scenario tests on `harness::Sim`. Chimera
names `fn(&mut World, ..)` hooks in 74 places. The data documents (230
TOML files in games and apps) are compiled in: 104
`include_str!`/`include_bytes!` sites in games and apps, none read
from disk at run time in a dev build.

## 2. Options compared

Estimates are per occurrence at the loads measured; "share" is the
share of the 726 Rust commits the option can touch.

| Option | Speeds up | Does not touch | Cost and risk | Verdict |
|---|---|---|---|---|
| A. Games as native dynamic libraries (Rust `dylib`, or `cdylib` behind a C ABI / `abi_stable`) | Static link of our crates into each binary: about 1 s a binary idle (docs/performance.md), the 300-660 MB written per binary | Compiles: a Rust `dylib` is rebuilt with its dependencies exactly like an `rlib`. Tests. Selection | A C ABI means no `Query`/`Res` across it: the games rewritten against a host API (224k lines). Rust `dylib`s must match compiler, Bevy and features exactly; Windows DLLs cap at 65,535 exports (Bevy's own dylib is 313 MB here) | No |
| B. Hot-patching a running game (Bevy 0.20's `hotpatching` feature, `subsecond`, run under `dx serve --hot-patch`) | The capture loop (11% of agent time): a system's body changes without a restart and map load | Tests, hooks, anything headless | Needs the Dioxus CLI on both systems; untried with our dynamic linking, workspace size and Windows; by its design it replaces function bodies, not types or schedules | Time-boxed experiment, later (stage 5) |
| C. Game logic as WASM modules (wasmtime) behind a host API | Engine edits stop rebuilding a module (if the host API holds still) | Tests still run in the host; the API moves with 84% of engine commits today | The same rewrite as A, plus copies in and out per tick, no debugger worth the name, the determinism rules restated at the boundary | Not for iteration. Later, for rule modules from servers (stage 6) |
| D. Scripted behaviour hooks (Lua through `mlua`, or Rhai) | A rule changes with no build at all; hot reload for free | The games' existing systems | Per-call cost (fine for events and round rules, not for per-tick per-character work); a second language to test; determinism of the interpreter's floats and tables | Same as C: with the original minigames, not before |
| E. Data read at run time in dev builds | A data edit: 17-30 s of rebuild (plus everything above) to 0; a running game can reload | Rust edits | Small: one macro, a test that both paths give the same bytes; release builds stay as they are | Yes (stage 2) |
| F1. A game's front end as its own crate | 18% of Rust commits (client side) stop rebuilding, type-checking in the hook and selecting five games' simulation suites; a game's simulation tests stop waiting for `chimera_client` (17 s), `chimera_present`, `chimera_ui`, `source_gameui` after every base edit; servers and clients share one build of each game | Base edits still reach every game | Mechanical moves (1.0k-17.9k lines a game); features, hakari, the architecture test's rules and docs/new-game.md change with it | **Yes, first** (stage 1) |
| F2. The party kit above no game (games handed in as descriptors by a roster) | 23% of Rust commits (games only): `party_kit` (18 s) is not rebuilt, its game-free tests not selected | Composition tests still depend on the games they compose | architecture-pass.md steps 8 and 9, already planned | Yes (stage 3) |
| F3. Contracts apart from implementations in the base (games compile against a thin crate of components, registries and descriptors; systems live in crates only apps and harnesses link) | Base edits that keep the contract: the 136-148 s of unchanged crates per edit, the chain's wall (87-92 s) to about 40 s, and the hook's type-check of 29 packages (most of its 704 s) | Edits that change the contract (most engine commits today); test selection (a behaviour change below can break a game's tests: they must run) | The largest refactor here: 41k-line crates sorted into contract and implementation; orphan rules and Bevy plugin order to respect | Spike on one module first (stage 4), keep only on a measured win |
| G. Test architecture: logic as plain functions over plain data, tested without a `Sim` | CPU per test (a `Sim` test costs 0.06-0.25 s of start alone; a function test microseconds); fewer tests need the engine's implementation at all | Existing 4,400 scenario tests | A rule for new code, not a migration; it is also the shape a module API needs (architecture-pass.md 7.3) | Yes, as a rule (stage 2) |
| H. Modules sent by servers | Nothing in development; players get a minigame without a client update | - | Section 4 | Later (stage 6) |

### Why not dynamic game plugins for iteration

- **A plugin boundary does not stop a rebuild; an unchanged interface
  does.** In Rust the only interface a compiler honours is "this crate's
  dependencies did not change". Loading `tf2` from a `.so` changes how it
  is linked, not when it is compiled.
- **A stable ABI is a second engine API.** Games are ECS systems
  (section 1.5). Behind a C ABI or WASM they cannot hold a `Query`; every
  system becomes a call through tables of columns. That is the module
  API of architecture-pass.md section 7, worth building for small rule
  modules, not as the way the five ports are written.
- **The measured cost is elsewhere.** Builds are 5% of agent time. The
  34% in tests and hooks scales with how many crates a change selects,
  which a loader does not change.
- engine-split.md decided the same in October ("No native dynamic
  plugins": a Bevy dylib must match the compiler and Bevy version
  exactly); the measurements give no reason to reopen it.

## 3. Recommendation

Make games plug-ins **in the crate graph**, and move what changes often
above what depends on it:

1. A game is two crates: `<game>` (simulation: parts, rules, bots, net
   parts, `harness`) on Chimera's simulation crates only, and
   `<game>_ui` (HUD, menus, front-end hooks) on `chimera_client`. Apps
   assemble both by descriptor (`BUNDLE`, `CLIENT`, `SERVER`, as today);
   servers take the first only.
2. Data is read at run time in dev builds and compiled in for releases.
3. The party kit depends on no game; a roster crate above it lists the
   games' descriptors, and the composition tests live with the roster.
4. New engine and game logic is written as plain functions over plain
   data with a thin system around them, tested without a `Sim` where the
   logic allows.
5. Only then, and only if a spike shows the win: contracts apart from
   implementations in `chimera_sim` (the minigame runtime first).
6. Scripted or WASM rule modules arrive with the original minigames and
   server-distributed content, behind the batch-shaped hooks, never as a
   way to write a port.

What this does not fix, said plainly: 45% of Rust commits edit Chimera's
base, and for those every game's tests are rightly selected. Stages 1 to
3 shrink the other half and shorten the base's chain; stage 4 is the only
one aimed at the base, and its win depends on how many base edits keep
the contract, which is not known yet (stage 0 measures it).

## 4. Server-distributed modules and the rules

Independent of iteration speed; recorded here because the same boundary
is asked about.

- **What a module may hold**: our own code and our own data documents
  (definitions in our words and measurements, as in the repository).
  Never another game's assets, keys or third-party code that could not
  be in the repository. A module names assets by id; the assets come
  from the player's install or the asset service under its own access
  rules. Packs stay code-free (ingest.md: no scripts, binaries, Lua or
  WASM in a pack), so modules are a separate channel with their own
  signature and their hash in the match manifest beside the weapon sets'.
- **Combat Arms stays spec-only.** A module is built from the same
  source as the client; nothing about distribution changes how behaviour
  enters the code.
- **Native modules** can only be ours: they must match the client's
  compiler, Bevy and feature set exactly, and they run with the
  player's rights. That is an update channel for the game, not a plugin
  system; the release pipeline already does it.
- **Sandboxed modules** (WASM, or a script) are the only form for code
  from anyone else: a fuel limit, no clock, no filesystem, floats and
  iteration order pinned as the determinism ratchets already demand of
  our own code, state as replicated components the host owns so
  prediction and rollback stay the host's.
- **Order**: data first (minigame definitions, weapon documents and
  `server_docs` already travel), then rule hooks (round flow, scoring:
  per event, cheap), last anything per tick.

## 5. Staged path

Each stage states the number it must move. A stage that misses it is
reverted or stopped; later stages do not depend on it unless said.

### Stage 0: the measurements as tools (done with this plan)

- [x] `tools/arch_survey/survey.py radius [days]`: commits by kind and by
      lowest layer, crates each can affect.
- [ ] The hook's log gets the seconds of its two phases (type-check of
      the dependents, the fast tier): today only the total is recorded.
- [ ] A count of base edits that keep the contract: for the last 14
      days' commits to `chimera_sim`, how many leave `cargo doc`'s public
      item list (or `cargo public-api`, if installed) unchanged. This is
      the upper bound of stage 4's win.

### Stage 1: a game's front end as its own crate (first step: `tf2`)

Small and reversible: `tf2`'s `src/ui/` is 979 lines in 3 files with no
`tests/ui/`; one commit moves it to `games/tf2/ui` (crate `tf2_ui`) and
one revert puts it back.

- [x] `tf2_ui` on `tf2` and `chimera_client`; `tf2` loses its `ui`
      feature, the optional client dependencies and the dev-dependency on
      itself. The 4 `tests/it` files that name the client move to the new
      crate's tests. Apps and the party kit take `tf2_ui::CLIENT`.
- [x] The layering test (`checks/tests/it/architecture.rs`), hakari,
      docs/new-game.md and ARCHITECTURE.md's crate list follow.
- [x] Keep or revert by (results below, "Stage 1 results"): (a) `affected --files chimera/client/src/lib.rs`
      no longer selects `tf2`; (b) after an edit in `chimera_client`,
      `cargo test -p tf2 --features dev --no-run` compiles nothing (now:
      the client, `tf2` and its two test targets); (c) after an edit in
      `chimera_sim`, `tf2`'s test build no longer includes
      `chimera_client` (17 s of 87 at the measured load); (d)
      `selections_share_one_build` passes and the server and the client
      link the same `tf2` unit.
- [x] `halo_ce` (2.9k lines; the most edited game): `halo_ce_ui`
      (games/halo_ce/ui), with its HUD's shader (now
      `embedded://halo_ce_ui/hud/hud.wesl`), the `hud_filters` example and
      the six test files and two tests that named the front end. (a) a
      client edit selects `halo_ce_ui` and no longer `halo_ce`; its test
      build's unit graph: 696 units, 10 workspace packages, no client,
      presentation or UI kit (before: 733, 13 with all three). One test
      (`campaign_units_come_with_the_matchs_ladder_not_the_apps_start`)
      had passed only while the client was linked into the binary: its
      plugin's voices read messages nothing in it registered; it
      registers them now. Recipe step 5 gains: grep the tests for grouped
      imports too (`halo_ce::{ game::.., ui::.. }`), and run the whole
      fast tier, not only the moved tests.
- [x] `hl2dm` (3.8k lines, 54 UI tests): `hl2dm_ui` (games/hl2dm/ui), its
      `tests/ui` (9 files plus the kill-icon heavy one) and the two
      install-look tests of `tests/it/heavy/hud_look.rs` that named it
      (the rest of that file stays). (a) a client edit selects `hl2dm_ui`
      and `hl2dm_app`, not `hl2dm`; (b) after a function appended to
      `chimera_client`, `cargo test -p hl2dm --features dev --no-run`
      compiles nothing (0.4 s); (c) after one appended to `chimera_sim` it
      compiles `chimera_sim`, `chimera_net`, `source_engine`,
      `source_logic`, `hl2dm` (20 s wall, warm): its test build's unit graph
      is 745 units, 12 workspace packages, no client, presentation, UI kit
      or GameUI; (d) `selections_share_one_build` passes. Fast tiers: `hl2dm`
      68 unit + 354 `it`, `hl2dm_ui` 24 unit + 54 `it`, all passing.
      Grouped imports again (`hl2dm::{ game::.., ui::.. }` in three test
      files) were what the first test build tripped on.
- [x] `cs_source` (12.4k lines, 112 UI tests): `cs_source_ui`
      (games/cs_source/ui), its `tests/ui` (19 files, 5 heavy), the radar's
      shader (now `embedded://cs_source_ui/radar.wesl`) and a copy of
      `tests/common` (the helpers, `stack.rs` with `client`). The crate
      root aliases the names the modules reach by `crate::` (`client`,
      `core`, `map`, `game`, `bot`, ...) instead of rewriting them. (a)
      `affected --files chimera/client/src/lib.rs` selects 21 packages,
      `cs_source_ui` and `cs_source_app` among them, not `cs_source`
      (545 fast `it` tests and 77 unit tests leave a client edit's
      selection); (b) after a function appended to `chimera_client`,
      `cargo test -p cs_source --features dev --no-run` compiles nothing
      (0.72 s); (c) after one appended to `chimera_sim` it compiles
      `chimera_sim`, `chimera_net`, `source_logic`, `source_engine`,
      `cs_source` (33 s wall, warm): 746 units, 14 workspace packages, no
      client, UI kit or GameUI. `chimera_present` stays in that graph on
      purpose: the scenario tests' maps run with its `MapPlugin` and
      `render::lod` (a dev-dependency, `TEST_ONLY`), so it is listed in
      `dev`; (d) `selections_share_one_build` passes. Fast tiers:
      `cs_source` 77 unit + 545 `it`, `cs_source_ui` 47 unit + 86 `it`
      (26 heavy), all passing. Grouped imports: `cs_source::ui::` in
      `party_kit`'s tests and sources and the CS:S app's re-export
      (`::cs_source_ui as ui`); `mashup_demo`'s dev-dependency on
      `cs_source` with `ui` is gone (its tests share the one build now).
- [x] `combat_arms` (17.9k lines, 322 UI tests): `combat_arms_ui`
      (games/combat_arms/ui), its `tests/ui` (20 files, 8 heavy), plus a
      copy of `tests/it/mode_kit.rs` and of the HUD rifle's weapon data.
      The crate root aliases the `crate::` names (`client`, `core`,
      `game`, ...); `crate::ui::` became `crate::`. Three install checks
      of the game's `heavy::radar` (art and icons, the large-map and
      layout attribute files) and `heavy::display_names` named the front
      end: they moved to `combat_arms_ui`'s `heavy::radar_install` and
      `heavy::display_names`; `heavy::weapon_skins` lost its optional
      display names (ids only; the front end's string table is above the
      game). `game::world::dtx_bc` is `pub` now (the front end's texture
      loader calls it). The data under `games/combat_arms/data/ui/` stays
      where it is (nine `include_str!` sites in the front end read it
      across the crate boundary). `UI_FEATURES` (the layering test's table
      of games with a `ui` feature) is empty now. (a) `affected --files
      chimera/client/src/lib.rs` selects 20 packages, `combat_arms_ui` and
      `combat_arms_app` among them, no game crate: `combat_arms`'s 1,749
      fast `it` and 101 unit tests leave a client edit's selection; (b)
      after a function appended to `chimera_client`, `cargo test -p
      combat_arms --features dev --no-run` compiles nothing (2.9 s, only
      the link check; `combat_arms_ui`'s own build: `chimera_client`,
      `combat_arms_ui`); (c) after one appended to `chimera_sim` it
      compiles `chimera_sim`, `chimera_net`, `combat_arms` (45 s wall at
      load 16-22): 702 units, 11 workspace packages, no client,
      presentation or UI kit (`combat_arms_ui` after the same edit: 5
      units, 46 s); (d) `selections_share_one_build` passes. Fast tiers:
      `combat_arms` 101 unit + 1,749 `it`, `combat_arms_ui` 10 unit + 303
      `it`, all passing; `cargo check --workspace --features dev
      --all-targets` clean. Grouped imports again: `combat_arms::{ ..,
      ui::.. }` in `party_kit`'s and `ultimate_gun_game`'s tests and
      example (rewritten by script, all multi-line forms); the latter
      gets `combat_arms_ui` as a dev-dependency.

Stage 1 is complete for all five games. Counted for the four whose
numbers were taken here (TF2 279, HL2:DM 422, CS:S 622, Combat Arms
1,850 fast tests) 3,173 simulation tests leave a client edit's
selection, plus Halo's.

What Stage 2 (data at run time in dev) needs next: the macro must read
a crate's data from that crate's folder, and the front ends now keep
data in a sibling folder: `combat_arms_ui` reads `games/combat_arms/
data/ui/` (nine sites: fonts, strings, menu and options layouts, mission
HUD), and the first Stage 2 decision is whether UI data moves into
`games/<g>/ui/data` (then a data edit selects only the front end's
tests) or stays with the game's (then it selects the game and everything
above it, as now). `affected` maps data files by path, so a moved file
needs no change there; its `feature-paths` metadata, used only by the
old `ui` features, can be deleted with the `UI_FEATURES` table.

Expected once all five are split: the 131 client-side commits per 14
days (18%) select the front ends' suites and the apps instead of every
game's simulation suite (about 3,700 fast tests, and their heavy ones
after the push: the 19-minute class of post-push runs moves toward the
13-minute one or below); every game-simulation loop after a base edit
drops the client chain; the hook after a client-side edit type-checks
the front ends and apps, not the games' simulation test crates (191k
lines); a dedicated-server build reuses the client build's game crates.

#### Stage 1 results (`tf2`, 2026-11-21)

Kept. Measured in an agent slot at load 34-47 on 12 threads:

- (a) `affected --files chimera/client/src/lib.rs` selects 24 packages
  before and after, `tf2_ui` where `tf2` was: TF2's 279 fast simulation
  tests (221 in `it`, 58 unit) leave a client edit's selection; the 10
  that need the client (`tf2_ui`) stay in it.
- (b) After a function appended to `chimera_client`, `cargo test -p tf2
  --features dev --no-run` compiles nothing (0.55 s; before: the client,
  `tf2` and its two test targets). The front end's own test build after
  the same edit: `chimera_client` and `tf2_ui`, 17.7 s.
- (c) After a function appended to `chimera_sim`, `tf2`'s test build
  compiles `chimera_sim`, `chimera_net`, `source_logic`,
  `source_engine`, `tf2` and its two test targets: 80 unit-seconds, 62 s
  wall; no `chimera_client`. Its unit graph (`--unit-graph`): 745 units,
  12 workspace packages, no client, presentation or UI kit (before: 781
  units, 15 packages with `chimera_client`, `chimera_present`,
  `chimera_ui`).
- (d) `selections_share_one_build` passes. `tf2`'s own unit has the same
  features (`dev`) in the client (`ultimate_gun_game`), the servers
  (`ultimate_gun_game_server`, `lucker_party_server`) and `--workspace`;
  before, the client and `--workspace` built `[dev, ui]` and a server
  `[dev]`. **But a server built alone still does not reuse the client's
  `tf2` artifact**: the servers' Bevy and Avian are built without render
  features on purpose, so the unit's dependency closure differs (668
  units under `tf2` for a server, 743 for the client) and so does its
  hash. The expectation above that "a dedicated-server build reuses the
  client build's game crates" holds only within one cargo invocation
  (`--workspace`, the hooks), where it held before too. Worth knowing
  before splitting more games for that reason alone; the other three
  numbers are the reason to go on.

#### The recipe, per game

One commit per game; `<g>` is the game's id.

1. `git mv games/<g>/src/ui/mod.rs games/<g>/ui/src/lib.rs` and the
   rest of `src/ui/` to `games/<g>/ui/src/`; in them `crate::game::`
   and `super::super::game::` become `<g>::game::`, `super::<m>` (a
   sibling) `crate::<m>`. Remove `pub mod ui;` (and its `cfg`) from
   `src/lib.rs` and its doc's mention.
2. `games/<g>/ui/Cargo.toml` (copy games/tf2/ui's): package `<g>_ui`;
   dependencies `<g>`, the Chimera crates its code names,
   `chimera_client` (and `chimera_present`/`chimera_ui`/`source_gameui`
   if named), `bevy`, `workspace-hack`; the dev-dependencies the
   presentation's builds have (`chimera_world` with `render`, Bevy
   `default`+`wav`, Avian's three features, `bevy_winit`), so
   `selections_share_one_build` holds; a `dev` feature forwarding to
   every workspace dependency's `dev`; one test binary `tests/it/main.rs`.
3. `games/<g>/Cargo.toml`: drop the `ui` feature, the optional client
   dependencies, the dev-dependency on itself with `ui`,
   `[package.metadata.affected] feature-paths`, and the `?/dev` entries
   of the client crates in `dev`.
4. The root Cargo.toml: `games/<g>/ui` in `members` and
   `default-members`, `<g>_ui = { path = "games/<g>/ui" }`; drop
   `<g>/ui` from the root's `present`. `apps/party_kit`: `<g>_ui`
   optional, `dep:<g>_ui` in `present` for `<g>/ui`, `<g>_ui?/dev` in
   `dev`; the game's own app the same (non-optional). `<g>::ui::` becomes
   `<g>_ui::` everywhere (`grep -rn "<g>::ui"`).
5. Tests: whatever in `games/<g>/tests/it` names `chimera_client`,
   `chimera_present` or `<g>::ui` moves to `games/<g>/ui/tests/it/`
   with the helpers it needs (copied: test helpers are not shared across
   crates).
6. checks/tests/it/architecture.rs: `games/<g>/ui/src` in `CRATE_SRC`;
   the game's row out of `UI_FEATURES`; a `FORBIDDEN` row for
   `games/<g>` naming `chimera_present`, `chimera_client`, `chimera_ui`
   (`layers_follow_paths` and `games_share_one_shape` already know
   `games/<id>/ui`). `cargo hakari manage-deps -y && cargo hakari
   generate`; ARCHITECTURE.md's crate tree; `docs_index`.
7. Measure (a)-(c) as above (`affected --files chimera/client/src/lib.rs`;
   an appended function in `chimera_client`, then `chimera_sim`, and
   `cargo test -p <g> --features dev --no-run`), then the game's fast
   tier, `<g>_ui`'s tests, `repo_checks`.

### Stage 2: data at run time in dev, logic outside the `Sim`

- [ ] One macro in `chimera_core` for a crate's data documents: with
      `dev`, the file is read from the crate's folder at run time (and
      can be read again: a console command); without, `include_str!` as
      now. `tf2` first (9 sites), then the other games and the apps
      (104 sites in all). A test asserts the two paths agree.
- [ ] Keep by: a data edit rebuilds nothing (now 17.4 s for `tf2` alone
      at load 20, plus everything above it), and the changed crate's
      tests still run in the hook (`affected` maps the file by its path).
- [ ] The rule for new code, in AGENTS.md and the scenario-test skill:
      decide in a plain function, apply in a system; test the function
      directly and keep one scenario test for the wiring.

### Stage 3: the party kit above no game

architecture-pass.md steps 8 and 9 (roster of descriptors; the base game
by descriptor), with the iteration measure added: after a one-line edit
in a game, `party_kit`'s library is not rebuilt (now 18 s) and its
game-free tests are not selected; the composition tests move to the
roster's crate, split by game where they compose one game.

### Stage 4: contracts apart from implementations (spike, then decide)

- [ ] Spike on `chimera_sim::minigame` (18.3k lines, 169 of the crate's
      323 file changes in 14 days; games name about 25 of its paths):
      the types games register against stay; the runtime moves to a
      crate that apps and harnesses add. `chimera_net` and
      `chimera_client` use the runtime today (9 and 24 files), so the
      spike first answers whether their uses can move with it.
- [ ] Decide by stage 0's count and the spike: if under a third of base
      edits keep the contract, stop here.
- [ ] If kept: `chimera_world::map`'s spawning and `chimera_sim::weapon`
      next, by the same measure.

### Stage 5: hot-patching for the capture loop (experiment)

One day, one app: `dx serve --hot-patch` on `ultimate_gun_game` with
Bevy's `hotpatching` feature beside `dev`, on Linux and Windows. Keep
only if a system's body changes in a running game without the app
restarting, on both systems, with dynamic linking on.

### Stage 6: rule modules (with the original minigames)

When original minigames are asked for: the rule-module interface in the
batch shape (architecture-pass.md 7.3), one module run both natively and
sandboxed, the choice between WASM and Lua made on that module's
measured cost per event and per tick.

## Not doing, and why

- **Games as native dynamic libraries, a C ABI, `abi_stable`**: section
  2; no compile saved, the games rewritten.
- **Rewriting a port as WASM or scripts**: the same, plus slower tests.
- **A cargo feature per game for subset builds**: every selection shares
  one build today (`selections_share_one_build`); features per game
  would bring back a build per selection.
- **Splitting `combat_arms` (110k lines) into crates for its own sake**:
  its front end leaves in stage 1 (17.9k); the rest waits for a measure
  showing which of its modules change together.

## Progress

2026-11-21: measured, options compared, plan written; `survey.py radius`
added. Nothing in the product changed.
2026-11-21: stage 1 for `tf2`: its front end is `tf2_ui` (games/tf2/ui);
criteria (a)-(c) met, (d) as far as features go (results above); the
layering check knows a front-end crate by its path (`Layer::FrontEnd`).
2026-11-21: `halo_ce` the same: `halo_ce_ui` (games/halo_ce/ui).

## Decision log

2026-11-21: plugins as crates, not as loaded libraries, for iteration
speed: the measured costs are the number of crates a change reaches and
the tests it selects, which a loader does not change, and a stable ABI
would cost a rewrite of the games.
2026-11-21: first step is `tf2`'s front end as its own crate: the
smallest game, no UI tests, one commit to move and one to revert.
2026-11-21: `tf2_ui` kept: a client edit no longer builds or selects
`tf2`'s simulation tests, and a base edit no longer builds the client
for them. A server built on its own still builds its own `tf2` (its
Bevy has no render features), so "servers share the client's game
build" is not a reason to split; iteration on the client side is.
2026-11-21: `hl2dm` the same: `hl2dm_ui` (games/hl2dm/ui).
2026-11-22: `cs_source` the same: `cs_source_ui` (games/cs_source/ui).
2026-11-22: `combat_arms` the same: `combat_arms_ui` (games/combat_arms/ui); stage 1 complete.
