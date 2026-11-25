# Plan: Source's movement lifted into Chimera (shared Quake-lineage movement)

Started 2026-11-24. The short plan [goldsrc.md](goldsrc.md) asks for
before its M2 (§4, §7): `source_engine::source_movement`, the player
movement CS:S, HL2:DM, TF2 and DoD:S run, lives in the `source` loader
layer, which a GoldSrc game (`cs16`) may not use. The movement both
families share moves into Chimera, so `cs16:movement` runs on it with its
own config and the GoldSrc differences plugged in. No code lands with
this plan.

Rules it follows: what the second game needs is generalized into
Chimera, never copied from one game to another; infrastructure goes in
Chimera; no game depends on another (`cs16` never on `source_engine` or
`cs_source`). GoldSrc behaviour reaches the code only through
`specs/goldsrc/` and `specs/cs16/` (CLAUDE.md's hard rules): this plan
names the seams, the specs give the numbers and rules.

## Status

| Step | State | Summary |
|---|---|---|
| Plan | done (2026-11-24) | this file |
| S0 baselines | done (2026-11-24) | golden trajectories and work counts for the four games (the log's table) |
| S1-S4 carve inside `source_engine` | done (2026-11-24) | file split, neutral hooks, CS:S's numbers to `cs_source`, cvars as data: each in the log |
| S5-S6 the crate | done (2026-11-24) | `chimera_movement`, then neutral names with Source aliases (aliases dropped 2026-11-24, last log entry): each in the log |
| S7-S9 GoldSrc seams | done (2026-11-25) | clip hulls (S7), command milliseconds (S8), edge friction (S9): each in the log |
| Hand-off | done (2026-11-25) | goldsrc.md M2: the hull tracer in `goldsrc_engine` (`hulls`), `cs16:movement` on the crate; the GoldSrc rules beyond the seams as named switches (the log's last entry) |

## 1. What is there (origin/main, 2026-11-24)

`source/engine/src/source_movement/`, 5 719 lines:

| File | Lines | Holds |
|---|---|---|
| `mod.rs` | 3 723 | config (about 75 fields), cvars, the component (`SourceMovement`), registration, `Tracer` (box sweeps), `Mover` (the tick: about 70 methods), the `step` system |
| `useable_ladders.rs` | 864 | `func_useableladder` lines: parsing the map's entities, mounting, forced moves, climbing, dismounts; other games' ladder volumes as lines |
| `shadow.rs` | 669 | the player's physics shadow pushing `prop_physics` props (physics_props.md 4.1) |
| `pushaway.rs` | 299 | multiplayer props pushed and pushing back (4.2) |
| `drown.rs` | 164 | CS:S's measured drowning |

It already uses only Chimera's types: `chimera_core` (`core`, `slots`,
`console`, `envelope`), `chimera_world` (`map`: brushes, the BSP tree
over them, terrain, water, props, sounds, surfaces) and avian. Nothing in
it reads a Source file; "Source" is where its spec and numbers came
from. Users: 138 Rust files name it (the games' glue: `cs_source`
58 lines, `hl2dm` 422, `tf2` 438, `dods` 154 plus `stance.rs` 518 and
`stamina.rs` 206; HL2:DM's suit, physics, pickups, sounds and weapons;
TF2's items, blast, flame, recoil; DoD:S's weapon blocks;
`source_engine::player_anim`; `cs_source`'s resim, hit reaction and
movecmp; the party roster's cross-game tests), about 25 docs. Tests on
it: CS:S 66 (`movement`, `source_movement`, `_css`, `_ladder_water`,
`_tricks`), HL2:DM 28, TF2 24, DoD:S 12, plus the heavy map walks
(dust2, nuke, surf, ladders, props, militia) and `party_roster`'s
ladders and water across games.

`cs16:movement` today is the placeholder (`chimera_sim::movement::
placeholder`) with numbers from `games/cs16/data/movement.toml`.

## 2. What is shared, what is Source's

**Engine-neutral (the Quake lineage; moves to Chimera):** the tick's
order; acceleration toward a wish direction, friction with the stop
speed, air acceleration with the 30-unit wish cap; gravity split around
the move; jumping (impulse, held-jump rules, bunny-hop cap as config);
the ground check (2-unit probe, walkable normal 0.7, leaving the ground
by vertical speed); the slide move with clip planes and creases; stair
stepping and staying on the ground; ducking (box, eye, timers, air duck
lift, what blocks standing up); contents ladders; water level, swimming,
water jumps; landings, fall damage and the punch roll; footsteps through
`map::sound::SoundBank` and the surface grid; stuck recovery, the seat
and footing of `slots`; the per-movement configs and the map envelope;
the box sweep against Chimera's map data (`MapBrushes`, terrain, movers,
other characters, props by shape cast). Also the parts that act on
Chimera's own types and are opt-in by config: ladder lines and forced
moves (the climbing half of `useable_ladders`, which "Ladders across
games" in DESIGN.md already uses for every game's ladders), the physics
shadow and push-away (on `map::PhysicsProp`, which every game's maps
spawn) and drowning (`drowning` flag). Leaving them in the Source layer
would make a GoldSrc character pass through props a CS:S one pushes on
the same map: they are infrastructure.

**Source's own (stays in `source_engine::source_movement`):**

- **The registration wrapper**: `SourceMovementAppExt::source_movement(id,
  config, namespace)` keeps its signature, so the four games' plugins
  don't change. It calls Chimera's registration with Source's cvar list
  and Source's key impulse (`core::HalfFirstPress`: measured on CS:S,
  HL2:DM and TF2; GoldSrc's is for its spec to say).
- **Source's cvars**: the names only Source has (`sv_enablebunnyhopping`,
  `sv_autobunnyhopping`, `sv_footsteps`, `cl_forwardspeed`,
  `sv_ladder_dampen`, `sv_ladder_angle`) as a list handed to Chimera's
  cvar binding. The Quake-lineage ones (`sv_accelerate`,
  `sv_airaccelerate`, `sv_friction`, `sv_stopspeed`, `sv_gravity`,
  `sv_maxspeed`, `sv_stepsize`, `sv_maxvelocity`, `sv_bounce`) are
  Chimera's list, which GoldSrc shares by name.
- **The stance hooks' names**: `SourceStance`, `SourceSpeed`,
  `SourceTraits`, `SourceMovementStep` stay as aliases of Chimera's
  neutral types (below). The step must read the hooks, so the types move;
  what fills them stays in the games (DoD:S's prone and stamina, HL2:DM's
  walk/run/sprint, TF2's classes).
- **`func_useableladder` parsing**: `ladders_of` and `build_ladders`
  read Source entities (`func_useableladder`, `info_ladder_dismount`)
  into Chimera's ladder lines.

**Not Source's, though the brief to this plan listed it so: the trace
shapes.** The box sweep reads only Chimera's map data, and goldsrc.md §3
has `cs16`'s player use "Chimera's shapes like any character" on other
games' maps (and every game's characters use them on GoldSrc maps). Its
two Source flavours are already data: the BSP-tree tie-break
(`MapBrushTree`) and displacement terrain (`MapTerrain`) exist only when
a Source map filled them. So the box sweep moves to Chimera as the
default world trace, and GoldSrc's hull trace is a second one beside it
(§3), chosen by map and config.

**CS:S's numbers leave the shared code**: `SourceMovementConfig::default()`
is CS:S's measured config and the bare `SourceMovementConfig` resource is
CS:S's. Chimera's config gets no game's defaults: `reference()` is the
movement spec's shared-code values (today's `shared_code()`, which the
spec's test cases use); CS:S's become `cs_source::movement::config()`
like HL2:DM's, TF2's and DoD:S's, registered by id. 34 call sites.

## 3. How GoldSrc's differences plug in

Each is off by default and costs Source games no work when off (no extra
trace, no extra system). Numbers and exact rules come from
`specs/cs16/movement.md` (a template today) and `specs/goldsrc/bsp.md`
("Hulls and collision"); the plan fixes only the seams.

1. **Hull traces instead of box sweeps** (goldsrc.md §4.1): a world
   trace seam. Chimera defines `ClipHulls`, a trait for a map's
   point-traced hull trees: `trace(hull, start, end) -> HullHit`
   (fraction, end, plane, started/all solid), `contents(hull, point)`,
   with the hull boxes it carries, over the world and the brush models
   (movers by model and transform). A map that has them inserts
   `MapClipHulls(Arc<dyn ClipHulls + Send + Sync>)`. A movement whose
   config sets `clip_hulls` traces the world through it when the map has
   one (standing box: its standing hull, ducked: its ducked hull);
   otherwise, and for every other game's character, the box sweep.
   Other characters, movers without a hull model and props stay box
   sweeps in both cases (a box against a box is what a hull trace of an
   entity box is), merged nearest-first as today. The trait lives in
   Chimera; the implementation over `goldsrc_formats::bsp::Bsp::trace`
   is `goldsrc_engine`'s, so no Chimera crate depends on the loader.
2. **The origin at the box's centre** (§4.2): the mover keeps working in
   feet (the box's minimum corner) and Chimera's `Transform` convention
   stays (the standing box's centre, feet 36 below, for `slots::Footing`
   and seating across movements). The hull seam converts: a hull is
   traced at feet + its box's half height, and its hit's end comes back
   as feet. Ducking (box shrinks by the duck lift on the ground, from
   above in the air) is already in the mover as `duck_height` and
   `air_duck_shift`. Spawns and eye heights are the map loader's and
   the config's.
3. **Milliseconds per command** (§4.3): config `command_msec`. The tick's
   `dt` is cut to whole milliseconds and the remainder carried to the
   next tick in the component (predicted, so a replayed tick sees it).
   At `cs16`'s 0.01 s tick it changes nothing; it matters only when a
   party plays CS 1.6's movement at another tick (0.015 s: 15 ms, also
   exact; 1/66: 15 ms with a carried 0.15 ms).
4. **The 36-unit duck box** (§4.4): config (`duck_height`, eyes,
   `duck_time`, the lift); new behaviour only where the spec finds one
   the existing flags don't cover (each a named flag, off for Source).
5. **Edge friction** (§4.5): config `edge_friction` (1: none, and then no
   probe at all). Above 1, `friction` probes for a drop ahead of the
   player as the spec describes and multiplies the friction.
6. **The rest of §4** (cvar defaults, landing slow-down, speed by
   weapon, ladders, water, fall damage, punch) is config and `cs16`'s
   own hooks on the neutral types, as DoD:S's stance is today.

## 4. Target API and layout

A new crate, **`chimera_movement`** (`chimera/movement/`), on
`chimera_core`, `chimera_world`, avian and Bevy only. Not a module of
`chimera_sim` (goldsrc.md §4 said `chimera_sim::movement`): an edit to
the movement then rebuilds and retests only its users (`source_engine`'s
games, `goldsrc_engine`, `cs16`, the party apps), not every crate above
`chimera_sim`, and the two build in parallel. goldsrc.md is updated to
this name.

```
chimera/movement/src/
  lib.rs          QuakeMovementPlugin, MovementStep (system set),
                  QuakeMovementAppExt::quake_movement(id, config, cvars, namespace)
  config.rs       QuakeConfig (+ reference(), envelope(), fitted(),
                  get/set by cvar name), QuakeConfigs (by id),
                  MovementConfigs (system param), QUAKE_CVARS
  state.rs        QuakeMovement (the predicted component), SettleOnGround
  hooks.rs        MoveSpeed, MoveTraits, MoveStance, StepSounds,
                  PhysicsTouch
  units.rs        to_units / to_engine (inches, Z up <-> metres, Y up)
  trace/mod.rs    Trace, Tracer (world + others + movers + props)
  trace/boxes.rs  the box sweep against MapBrushes, terrain, boxes
  trace/hulls.rs  ClipHulls, MapClipHulls, the feet <-> hull centre seam
  mover/mod.rs    Mover and the tick's order
  mover/ground.rs friction (+ edge friction), accelerate, walk, slide,
                  step, stay on ground, categorize
  mover/air.rs    air move, air jumps
  mover/duck.rs   duck, unduck, stances
  mover/jump.rs   jump, stamina, bunny-hop cap
  mover/water.rs  water level, swim, water jump
  mover/ladders.rs contents ladders
  mover/lines.rs  ladder lines, forced moves, dismounts
  mover/falls.rs  landing, fall damage, punch roll
  mover/steps.rs  footsteps
  mover/stuck.rs  stuck nudges, reseat, seat and footing
  step.rs         the system (Predict::Movement, in MovementStep)
  shadow.rs, pushaway.rs, drown.rs
```

`source/engine/src/source_movement.rs` (one file, about 200 lines):
`pub use chimera_movement::*` plus `to_source = to_units`, `useable_ladders`
and `SourceMovementPlugin`; `CVARS` (Source's extras),
`SourceMovementAppExt`, and the `func_useableladder` reader. The old
`SourceMovement`, `SourceMovementConfig(s)`, `SourceMovementStep`,
`SourceSpeed`, `SourceTraits` and `SourceStance` aliases were dropped
(2026-11-24, after the in-flight branches landed): callers use the neutral
names.

Architecture checks: `chimera_movement` in the workspace, its crate
group and Chimera layer; `ALLOWED` gains the crate's modules (each on
`core`, `map`, `slots`, `console`, `envelope`), `source_movement`'s rule
shrinks, `player_anim`'s points at the crate; the simulation's no-drawn-
parts list includes it. `cargo hakari generate`, ARCHITECTURE.md's layer
diagram and crate list, `docs_index`.

## 5. Steps

Each step is one commit (S1 and S5 are big but mechanical), lands with
every movement test green and the S0 golden runs unchanged, and is
pushed before the next. "No change" means the S0 hashes are equal.

| Step | What | Size | Behaviour |
|---|---|---|---|
| S0 | Baselines: a golden run per game (§7) and its work counts written into this plan's log | ~300 new test lines | none |
| S1 | Split `mod.rs` into the §4 files inside `source_engine` (config, state, hooks, units, trace, mover/*, step), `pub(super)` where fields cross files. Done in one sitting with no other movement branch open (§8) | ~3 700 moved | no change |
| S2 | Neutral hook types inside the module (`MoveStance`, `MoveSpeed`, `MoveTraits`) with the Source names as aliases; hard-coded sound names (`Player.Swim`, `Ladder.Step*`) and Source-only constants into config fields with today's values | ~300 | no change |
| S3 | CS:S's numbers to `cs_source::movement::config()`, registered with its id like the other three; `reference()` replaces `Default` and `shared_code()`; the bare `SourceMovementConfig` resource goes; 34 call sites (tests, movecmp, resim, client) | ~400 across 15 files | no change |
| S4 | Cvars as data: Chimera's binding takes a name list; `QUAKE_CVARS` and Source's extras; `HalfFirstPress` and the `func_useableladder` reader moved into the Source wrapper part; the core part of the module names nothing of `source_engine` (checked by a temporary `ALLOWED` entry) | ~250 | no change |
| S5 | Create `chimera_movement`; `git mv` the core files; `source_movement.rs` becomes the wrapper and re-exports; workspace, hakari, architecture rules, ARCHITECTURE.md, generated docs; bump the net `PROTOCOL_ID` (§6) | ~5 500 moved, ~300 new | no change; system names change |
| S6 | Neutral names inside the crate (`QuakeMovement`, `QuakeConfig`, `MovementStep`, `to_units`); Source aliases in the wrapper | ~600 in the crate | no change |
| S7 | `ClipHulls`, `MapClipHulls`, `clip_hulls` config and the feet/centre seam; Chimera scenario tests on a greybox with a test hull (boxes grown by the hull, a ramp, a step) checking a hull-traced walk, step and landing equal the box sweep's where they must | ~500 | Source off |
| S8 | `command_msec` with the carried remainder; prediction test of a replayed tick | ~120 | Source off |
| S9 | `edge_friction`, from specs/cs16/movement.md once it says how the drop is probed; until then the seam only, with its test | ~100 | Source off |

About 6 000 lines touched in S1-S6, nearly all moved; about 1 000 new
in S0 and S7-S9. Nine commits.

**Hand-off to goldsrc.md M2** (not this plan): `goldsrc_engine`
implements `ClipHulls` over `goldsrc_formats`' hull trace for the world
and brush models, and the loader inserts `MapClipHulls` with the hull
data carried in the converted map (its version bumped, so a client given
only the converted map predicts the same); `cs16:movement` moves from
the placeholder to `quake_movement` with CS 1.6's config from its spec.

## 6. Prediction and replay

- **One step, same place**: the step stays one system in
  `core::Predict::Movement`, in the set the games order against
  (`SourceMovementStep`, an alias of `MovementStep`), so HL2:DM's speed
  choice and DoD:S's stance step still run before it, on the client's
  replays too.
- **The predicted component's layout**: `predicted_net` encodes the
  components in type-name order (`PredictedComponents::encode`), and S5
  and S6 change the type's path and name. Peers of one build agree;
  peers of different builds would misread the blob, so S5 bumps the net
  `PROTOCOL_ID` (and S6 if it lands in another release). Field order
  stays as it is (postcard is positional); new fields (S8's remainder)
  go at the end. S1 already moved the order: DoD:S sends `SourceStance`
  beside `SourceMovement`, and `hooks::SourceStance` sorts before
  `state::SourceMovement` where `SourceMovement` came before
  `SourceStance`, so S1 bumped `NET_VERSION` (net74); S5 bumped again
  (net75: the types are `chimera_movement::`'s). S6's names sort where
  the old ones did (`state::QuakeMovement` after `hooks::MoveStance`, no
  other predicted type between), so it needs none.
- **State lives in the component**: new state (the msec remainder) is in
  `QuakeMovement`, so prediction saves and restores it. No new `Local`s;
  the existing `waist_steps` counter (HL2:DM's shared one-in-four) is a
  known shared state and keeps its behaviour.
- **The hull trace on both sides**: `ClipHulls` is pure over map data
  identical on server and client (same converted map, same version); its
  f64 maths runs the same everywhere; nothing in it caches by frame.
- **Recorded runs**: `cs_source`'s resim (`demosim`) and movecmp replay
  real CS:S input through the movement and compare tick by tick; they
  take CS:S's config from `cs_source::movement::config()` after S3 and
  must report the same as before. The demo tests (`demo_resim`) and
  `net_prediction` / `net_interp` run in every step's affected set.

## 7. Tests

- **S0 golden runs**, one per game, in each game's `tests/it/movement.rs`
  (CS:S, HL2:DM, TF2, DoD:S): a seeded `sim.repeatable()` greybox run of
  scripted input (run, strafe jumps, a duck jump, stairs, a ladder, a
  swim and water jump, a fall with damage, a stance or class where the
  game has one) for about 1 000 ticks; it hashes every tick's feet,
  velocity, flags and eye and asserts the hash taken before S1. A
  legitimate behaviour change later re-takes it in that commit, saying
  why. The same run prints `sim.work(n)`: the step's instructions per
  tick and the tick's total (`harness::own_process`, Linux only; None on
  Windows: skipped with a note).
- **Work counts**: each step compares the step's and the tick's counts
  with S0's numbers in this plan's log. Allowed: under 0.5 % for the
  moves (S1, S5, S6), under 1 % with the GoldSrc flags off (S2-S4,
  S7-S9); more needs a cause found first (a lost inline across the new
  crate boundary is the likely one: `#[inline]` on the small hot
  helpers). The system's name changes in S5: compare by the new name.
- **Every step**: `.githooks/pre-commit --worktree` (affected crates'
  tests), the four games' movement files, `party_roster`'s ladders and
  water across games, `cs_source`'s `net_prediction`, `demo_resim`,
  `prediction`, `hit_reaction`, `footstep_control`. S1, S3, S5 also run
  the heavy map walks (`CHIMERA_PUSH_TESTS=all` on the push: dust2,
  nuke, surf, ladders, props, militia walls; HL2:DM's map ladders).
- **New in Chimera** (S7-S9): `chimera/movement/tests/it/` scenario tests
  on `SimBasePlugins` and the greybox, with `reference()` config: hull
  trace walk, step, slope and landing; a ducked hull; a hull trace
  missing on a map without hulls falls back to boxes; command
  milliseconds with a carried remainder across a replayed tick; edge
  friction off costing no trace.

## 8. Risks

- **Parity drift in a delicate file.** CS:S's movement matches the game
  tick for tick in places (movecmp); a refactor that reorders one float
  operation breaks that silently. The S0 hashes catch it at the step
  that did it; S1, S5 and S6 must be pure moves.
- **Conflicts with work in flight.** DoD:S and TF2 parity agents edit
  `source_movement/mod.rs` often; S1 rewrites its layout. Do S1 when no
  open branch touches it (ask the orchestrator), land S1-S4 back to
  back, and tell agents to rebase; until S5 the file paths are
  `source_engine`'s.
- **Instruction counts.** A new crate boundary can lose inlining; the
  hull seam adds a branch per world trace. Measured per step (§7).
- **Build and test fan-out.** A new crate is one more to compile; its
  dependants are fewer than `chimera_sim`'s, so the affected set for a
  movement edit stays about today's (the Source games and the party
  apps) plus `cs16`.
- **The origin convention.** GoldSrc's origin is the current box's
  centre, Chimera's the standing box's; an 18-unit error at the hull
  seam or in `cs16`'s spawns and eyes would look like sinking or
  floating when ducked. S7's tests trace ducked and standing hulls
  against known floors.
- **Two collisions on one map.** On GoldSrc maps `cs16`'s player traces
  the hulls while other games' characters sweep the reconstructed shapes,
  which stop a few units short of slanted walls (goldsrc.md §3). Known
  and accepted there; players of different games may stand a few units
  apart against the same diagonal wall.
- **Mixed builds at a playtest** misread the predicted state after S5
  unless the protocol id is bumped (§6).
- **The specs lag.** S9 and parts of S7 wait on `specs/cs16/movement.md`
  (not written) and bsp.md's open question on hull choice; the seams can
  land first with Chimera's own tests, the GoldSrc numbers later.
- **Names.** "Source" aliases may linger in callers for a long time;
  harmless, but new code uses the neutral names.

## Log

- 2026-11-24: plan written from origin/main at 2eb65c01a.
- 2026-11-24, S0: the golden runs are in (`golden_run` in cs_source's,
  hl2dm's, tf2's and dods' `tests/it/movement.rs`: one course, the same
  in the four files but for the game's sim, TF2's Scout and DoD:S's prone
  at the end). The greybox's solids are physics colliders, so the course
  adds a brush staircase for the exact sweep. Each tick is run by
  `sim.work(1)` in the test's own process (`own_process`), so the hash
  and the counts come from one run. Hashes are asserted on Linux x86-64
  only (the maths library's last bits elsewhere are not known to match).
  TF2 climbs no ladder on the greybox (no ladders of its own, and the map
  is no other game's): its run expects none. Baselines (instructions a
  tick, dev profile in an agent slot; the step is `source_engine Movement
  step` in `Work::crate_systems`; three runs each: the step repeats to
  within 20 instructions, the tick's total to 0.15 %, so the step's count
  is the one to judge by):

  | Game | Ticks | Hash | Step | Tick |
  |---|---|---|---|---|
  | CS:S | 1 080 | `0x2c28df5ccf0d4a27` | 71 400 | 1 272 000 |
  | HL2:DM | 1 080 | `0x8b3d5be3e6970d7c` | 67 170 | 1 636 000 |
  | TF2 | 1 080 | `0xf46ef5454d21f47c` | 71 705 | 1 648 000 |
  | DoD:S | 1 300 | `0xcc08703f58eba75e` | 71 350 | 1 161 000 |
- 2026-11-24, S1: `mod.rs` split into `config`, `state`, `hooks`,
  `units`, `trace/{mod,boxes}`, `mover/{mod,ground,air,duck,jump,water,
  ladders,falls,steps,stuck}` and `step` by a script that moves whole
  items and methods (the old file's lines and the new files' are the same
  set but for imports, `pub(crate)`, the files' docs and `#[inline]`).
  Ladder lines stay in `useable_ladders.rs` until S4. The tick's
  constants stay together in `mover/mod.rs`. Hashes equal. The first
  build cost 1.35 % more in the step (CS:S 72 364, HL2:DM 68 077): with
  incremental builds and no LTO each module is its own codegen unit, so
  the small helpers were no longer inlined into their callers;
  `#[inline]` on 26 of them (`to_source`, `to_engine`, `Trace::hit`,
  `Tracer::hull`, `sweep`, `sweep_by`, `sweep_box`, `solid`,
  `point_solid`, `clip_velocity`, `hull_height`, the config's
  `air_duck_lift` and `climbs_contents`, and the mover's `clamp_velocity`,
  `half_gravity`, `start_gravity`, `view`, `accelerate`,
  `air_accelerate`, `wish`, `clamp_flat`, `eye_blend`, `smooth`,
  `can_stand`, `duck_timer_start`, `water_sound`) brought it back: CS:S
  71 395 (-0.01 %), HL2:DM 67 192 (+0.03 %), TF2 71 576 (-0.18 %), DoD:S
  71 336 (-0.02 %). The types' paths changed, which reorders the
  predicted blob for DoD:S (section 6): `NET_VERSION` net74.
- 2026-11-24, S2: the hooks are `MoveSpeed`, `MoveTraits` and
  `MoveStance` in `hooks.rs`; `SourceSpeed`, `SourceTraits` and
  `SourceStance` are re-exports of them under the old names (`pub use ..
  as ..`, not `type`: a tuple struct's alias can't construct it), so no
  caller changed. Source's names and numbers the tick held are config
  fields with the values they had: `swim_sound` (`Player.Swim`),
  `ladder_mount_sound` (`Ladder.StepRight`), the step surfaces
  `ladder_surface`, `wade_surface` and `water_surface`, `sound_channel`
  (4), the ladder outputs `ladder_on_output` and `ladder_off_output`
  (`OnPlayerGotOnLadder`, `OnPlayerGotOffLadder`) and
  `velocity_modifier_recovery` (1/3, CS:S's). `&'static str`, not
  `String`: the config is cloned per character each tick. The shared
  code's numbers (ladder and swim speeds, water jump, punch spring) stay
  constants until a GoldSrc spec says one differs. Hashes equal; the step
  CS:S 71 413 (+0.02 %), HL2:DM 67 190 (+0.03 %), TF2 71 644 (-0.09 %),
  DoD:S 71 370 (+0.03 %). `hooks::MoveStance` sorts where
  `hooks::SourceStance` did: no new wire order.
- 2026-11-24, S3: `SourceMovementConfig` has no `Default` and is no
  resource; `reference()` is the shared code's values (`shared_code()`
  renamed) and CS:S's numbers are `cs_source::movement::config()`,
  registered under `cs_source:movement` like the other three's. A
  registration without a config gets the reference values.
  `cs_source::movement::set_config` and `config_mut` replace the
  resource for the app's `--exec`/`--cvar` settings, resim, movecmp and
  the tests (39 call sites); a config set for an id before its movement
  registers stays (`SourceMovementConfigs::set`: the app sets it before
  its plugins build, as it inserted the resource before). A character on
  no registered Source movement (no slot, or another movement's) takes
  the first registered config, where it took the resource (CS:S's
  whatever the app): the same in CS:S's apps and the step's "the app's
  own game's" elsewhere; no test depends on it. The reference values for
  an app with none registered are kept in `SourceMovementConfigs` (a
  `static` is refused by `determinism::simulation_is_deterministic`).
  Hashes equal; the step CS:S 71 245 (-0.22 %), HL2:DM 67 012 (-0.24 %),
  TF2 71 420 (-0.40 %), DoD:S 71 117 (-0.33 %): one resource less to
  fetch.
- 2026-11-24, S4: the module is in two parts. Chimera's (everything but
  `mod.rs` and `source_ladders.rs`): `plugin.rs` is its root (what
  `lib.rs` will hold: `SourceMovementPlugin`, `SourceMovementStep`, and
  `QuakeMovementAppExt::quake_movement(id, config, cvars, namespace)`,
  whose `cvars` is a list of (name, help) lists), `config.rs` has
  `QUAKE_CVARS` (the nine shared names; `set_cvar`/`get_cvar` still know
  every name). Source's: `mod.rs` (`SOURCE_CVARS`, the six extras;
  `CVARS`, both lists in the old order, which the games hand the shell;
  `SourceMovementAppExt::source_movement`, same signature, which adds
  `HalfFirstPress` and calls `quake_movement`; the hooks' Source names)
  and `source_ladders.rs` (`ladders_of` and its test). The reader
  reaches the shared part as data, not a callback: `read_ladders` keeps
  `EntityLadders` the map's entities' (when `MapEntities` changes),
  ordered before `build_ladders`, which takes them on the same change it
  read the entities on before, so the lines change on the same frames.
  `ladders_of` is `source_movement::ladders_of` now (one heavy test's
  import). The check is a test of its own, not an `ALLOWED` entry (that
  table is per top-level module, and the two parts share one until S5):
  `architecture::source_movement_core_names_no_wrapper` fails when a
  shared file names the module by anything but a shared file; S5 removes
  it. Left for S5: `useable_ladders.rs` to `mover/lines.rs`, and the
  `git mv`s. Hashes equal; the step CS:S 71 247 (-0.21 %), HL2:DM
  66 969 (-0.30 %), TF2 71 392 (-0.44 %), DoD:S 71 155 (-0.27 %).
- 2026-11-24, S5: the crate is `chimera/movement` (`chimera_movement`,
  on `chimera_core`, `chimera_world`, Bevy, avian, serde). The shared
  files were `git mv`ed (`plugin.rs` is `lib.rs`, `useable_ladders.rs`
  is `mover/lines.rs`, public as `chimera_movement::lines`), their
  `crate::source_movement::` paths cut to `crate::`; the module's long
  doc and its known shortcuts went to `lib.rs` (tech-debt.md follows).
  `source/engine/src/source_movement.rs` is the wrapper, one file:
  `pub use chimera_movement::*`, the Source names (`SourceSpeed`,
  `SourceTraits`, `SourceStance`, `useable_ladders`), `SOURCE_CVARS` and
  `CVARS`, `SourceMovementAppExt`, and the ladder reader (`ladders_of`,
  `read_ladders`). No caller changed but the golden runs' work-count key
  (`chimera_movement Movement step`). Checks: the crate in the
  workspace, `CRATE_SRC`, the crate table (no loader layer, game, sim,
  net or presentation) and `ALLOWED` (its ten modules; `source_movement`
  down to core and map); determinism's baseline moved the three debug
  statics from `source/engine/src` to `chimera/movement/src`; the
  temporary `source_movement_core_names_no_wrapper` is gone (the crate's
  dependencies say it now). `cargo hakari generate`: no change.
  `NET_VERSION` net75. Hashes equal. The crate boundary cost nothing:
  the step and everything it calls are in the one crate (the games call
  across it only for configs and hooks), so no inlining was lost: CS:S
  71 237 (-0.23 % of S0, -0.01 % of S4), HL2:DM 66 939 (-0.34 %, -0.04
  %), TF2 71 394 (-0.43 %, 0.00 %), DoD:S 71 140 (-0.29 %, -0.02 %).
- 2026-11-24, S6: the crate's names are `QuakeMovement`, `QuakeConfig`,
  `QuakeConfigs`, `QuakeMovementPlugin`, `MovementStep` and `to_units`.
  The callers moved with them in the same commit (74 files: the four
  games, their tests, the party roster, CS:S's tools and app), by whole
  words: `SourceMovement`, `SourceMovementConfig(s)`,
  `SourceMovementStep`, `SourceSpeed`, `SourceTraits` and `SourceStance`
  are no longer written outside the wrapper, which keeps them as
  re-exports for branches in flight. Kept in callers on purpose:
  `to_source` (Source's games converting to Source units is what they
  mean; HL2:DM and the PHY reader have functions of that name of their
  own, so a rename by word would have hit them), `useable_ladders`,
  `SourceMovementAppExt::source_movement` (Source's registration) and
  `cs_source::movement::SourceMovementPlugin` (CS:S's own plugin). No
  wire change: the renamed types sort where they did. Hashes equal; the
  step CS:S 71 229 (-0.24 %), HL2:DM 66 952 (-0.32 %), TF2 71 416
  (-0.40 %), DoD:S 71 158 (-0.27 %). Next: S7-S9 wait
  for specs/cs16/movement.md.
- 2026-11-24, after S6 (DoD:S's two deferred items, dods.md §7): a
  per-game choice `QuakeConfig::turbo_physics` (no shadow, `prop_physics::
  NoShadow`; a `prop_physics` a wall at any mass; a multiplayer prop soft
  only in the push-away group, `PropGroup::live`; the props' filter made
  on the first such player's tick, so a movement without it pays a bool),
  on for DoD:S alone; `MoveStance::no_input` removed (DoD:S's stance
  changes cap the speed instead, its `max_speed`). CS:S, HL2:DM and TF2
  hashes equal; the step CS:S 71 360 (-0.06 % of S0, +0.18 % of S6),
  HL2:DM 67 052 (-0.18 %, +0.15 %), TF2 71 534 (-0.24 %, +0.17 %). DoD:S's
  run re-taken (`0x7396bb2d05258af7`: the course now moves while going
  prone), its step 73 770 (+3.4 % of S0): the moving prone transition's
  ground moves; with `turbo_physics` off the same run counts 73 746, so
  the props' rule costs 24 instructions a tick. `NET_VERSION` net76.
- 2026-11-24, aliases dropped: `SourceMovement`, `SourceMovementConfig`,
  `SourceMovementConfigs`, `SourceMovementStep`, `SourceSpeed`,
  `SourceTraits` and `SourceStance` are gone from `source_movement.rs`
  (no caller was left outside docs: the neutral names are used
  everywhere); kept: `SourceMovementAppExt::source_movement`,
  `cs_source::movement::SourceMovementPlugin`, `useable_ladders`,
  `to_source`. A rename only: golden hashes unchanged.
- 2026-11-25, S7: clip hulls. `trace/hulls.rs`: `ClipHulls` (a map's
  hulls: each hull's box around the traced point, `contents(hull,
  point)`, `trace(hull, start, end) -> HullHit`), `MapClipHulls` (the
  loaded map's, map-scoped) and the seam, `HullTracer`: which hull is a
  character's standing and which its ducked box (by size; no hull for the
  standing box: no hull traces at all) and the way from the feet to each
  hull's point (the box's minimum less the hull's: 36 and 18 units up for
  boxes centred on their point), added to a trace's ends and taken off
  its result. `QuakeConfig::clip_hulls` (off in `reference()` and every
  Source game): with it, on a map with `MapClipHulls`, the world part of
  a sweep or solid test of the standing or ducked box is the hull's
  trace or contents; the map's ladder volumes, the other characters, the
  movers and the props are swept as before and the nearest wins; any
  other box (the ladder floor probe's point), any other movement and any
  map without hulls sweep boxes. The ground test's quarter boxes are left
  out for a hull-traced box (a hull is the whole box's). A start in solid
  is fraction 0, as the spec's mover treats it. The implementation over
  the BSP's clip nodes is the hand-off's (`goldsrc_engine`). Tests, the
  crate's first integration tests (`chimera/movement/tests/it/`, on
  `SimBasePlugins`, which the architecture check allows it for tests
  only): `course` (a floor, a 12-unit step, a 20-degree ramp with a drop,
  a 50-unit ceiling, a wall, 2 000 units over the greybox, as map brushes
  and as a test's `ClipHulls` of the same solids grown by the hull's box)
  and `hulls`: a hull-traced run of the course with no brush of it in the
  map goes where the swept run goes, within 0.05 units every tick, same
  flags (step, ramp, fall, landing, duck under the ceiling, wall); the
  ducked box is the ducked hull (no standing up under the ceiling); a map
  without hulls, a movement not asking and a 48-unit body sweep, bit for
  bit. Hashes equal. The step, against this main without the change
  (CS:S 71 353 measured; the others' last logged): CS:S 71 718 (+0.51 %),
  HL2:DM 67 376 (+0.48 %), TF2 71 821 (+0.40 %), DoD:S 74 123 (+0.48 %):
  about 350 instructions a tick, under the 1 % limit. Looked for and not
  found in the sweeps: a movement tracing hulls takes its own copies of
  the sweep and the solid test (`sweep_box_hulled`, `solid_box_hulled`,
  `sweep_brushes_hulled`), so the swept path is the code it was behind one
  test of `Tracer::hulls`; the count was the same with the hull branch
  inside the sweep (71 772) and with the dispatch inlined (71 718), so it
  is not in the sweeps; where it is was not found (the step itself
  fetches one more resource and builds a wider `Tracer`: not measured
  apart).
- 2026-11-25, S8: commands of whole milliseconds.
  `QuakeConfig::command_msec` (off in `reference()` and every Source
  game): the step's time for that character is `command_msec(dt, carry)`:
  the tick's time plus what the last one left over, cut to whole
  milliseconds (at most 255; a thousandth of a millisecond short of a
  whole one counts as whole, the tick's time being an f32), the rest
  kept in `QuakeMovement::msec_carry`, a new field at the end of the
  predicted state (`NET_VERSION` net78: the blob is positional). A tick
  shorter than a millisecond steps nothing and carries it all. Tests
  (`tests/it/msec.rs`): the cut itself (10 and 15 ms exact; 1/66 s: six
  steps of 15 ms then one of 16, a second of ticks a second of steps); a
  run of the course at 10 ms ticks with it on is the run with it off, bit
  for bit; at 1/66 s a free fall loses 0.8 units/s a millisecond of each
  step and the remainder in the state is the function's; a replay (the
  transform, the velocity and the movement's state put back, the ticks
  run again) reaches the next 16 ms step on the same tick at the same
  bits, and with another remainder put back on another tick. CS:S's
  hash equal, its step 71 720 (+2 of S7; +0.51 % of main before S7); the
  other three were run at S9, with both steps in (equal).
- 2026-11-25, S9: edge friction, from specs/cs16/movement.md ("Ground
  friction"). `QuakeConfig::edge_friction` (1 in `reference()` and every
  Source game: no probe; the console variable `edgefriction` sets it):
  above 1, the ground friction is multiplied by it while `Mover::at_edge`
  holds: the current box, its centre put at the feet's height 16 units
  ahead along the horizontal velocity, traced 34 units straight down,
  meets nothing; a start in solid is a hit, so on a floor (the box starts
  half in it) there is none, and only a column clear from half the box
  over the feet to 34 units and half the box under them is an edge. The
  probe is the movement's own sweep, so it is a hull trace where the
  movement traces hulls. Test (`tests/it/edge.rs`, on the course's
  80-unit block): a slide at 200 units/s loses twice the speed in a tick
  with the body's centre 4 units past the edge, the usual inside the
  block, at the 12-unit step's edge and with the factor at 1. Hashes
  equal, the four games', with S8 in too. The step (of main before S7,
  CS:S's measured, the others' last logged): CS:S 71 724 (+0.52 %),
  HL2:DM 67 373 (+0.48 %), TF2 71 814 (+0.39 %), DoD:S 74 155 (+0.52 %):
  S8 and S9 together cost under 40 instructions a tick. Next: the
  hand-off (goldsrc.md M2: `goldsrc_engine::hulls`, `cs16:movement`).
- 2026-11-24, props for every movement (docs/DESIGN.md "Props across
  games"): meeting physics props is a service of the crate,
  `chimera_movement::props`, which any movement uses. The rule is the
  map's (`PlayerPropRules::for_character`: the map's game's, on a map of
  no game the movement's game's), so `QuakeConfig::turbo_physics` is gone
  (DoD:S's maps register `Turbo`, as they did for the spots' walker). The
  shadow and the push-away read a neutral body (`props::PropBody` in
  `shadow::PhysicsTouch::body`: box height and half width, origin over the
  feet, on the ground) instead of `QuakeMovement` and its config; the
  shared movement writes it in its step with its measured touch;
  `props::observe` (in `Predict::Movement`, set `core::AfterMovements`,
  which every movement's step goes before: the shared one's
  `MovementStep`, Combat Arms', Halo's, the placeholder's, noclip's) writes it for every other movement with a hull
  (`MovementImpl::footing`) from its collider, its ground and its keys,
  with what the box touches and stands on, and gives it the "solid" soft
  props' push-back as Source's accelerate. Combat Arms' and Halo's
  movements leave the passed props out of their casts (`props::PassedProps`,
  over `prop_physics::PropGroups`, which replaced the step's `TurboGroups`),
  run before `core::AfterMovements` (they depend on the crate for
  `PassedProps`; the service's systems, `PropsPlugin`, are the app's, which
  the shared movement's plugin adds: every app with a Source game's maps
  has them, Combat Arms' and Halo's own apps, whose maps have no physics
  props, don't; another body gets a shadow only while the map has a
  physics prop). The shadow's box is the body's (TF2's 24-unit half width, where
  it was 16 for every game). Golden hashes unchanged (CS:S, HL2:DM, TF2,
  DoD:S); the step against S9's: CS:S 71 916 (+0.27 %), HL2:DM 67 579
  (+0.31 %), TF2 72 001 (+0.26 %), DoD:S 74 704 (+0.74 %: the rule looked
  up by the movement's game on the greybox, which has no map game, and
  the body written each tick).
- 2026-11-25, the hand-off (goldsrc.md, M2): `cs16:movement` runs on the
  crate. Beyond the three seams, each rule specs/cs16/movement.md has
  that the existing switches did not cover is a named field of
  `QuakeConfig`, at the shared code's value in `reference()` and so for
  every Source game: `ground_snap` (the ground tested at the start of
  every tick, the body put on its probe's end, no stick-to-ground in the
  walk move, no quarter probes), `unduck_at_once` (no timed unduck; a
  duck let go before it completed stands 18 units up on the ground),
  `duck_ends_on_time` (the duck completes on the tick its timer reaches
  the duck time, not the one after), `jump_sets_speed` (the jump's speed
  set, not added), `bunnyhop_cut` (what a jump over the cap leaves of the
  cap: 0.8), `modifier_per_tick` (a hit's modifier gaining a step each
  ground tick and scaling the velocity after the move, the max speed
  unscaled), `punch_decay` (the roll decaying by constant + share x its
  size), `clip_stop_epsilon` (a slide's small components zeroed),
  `rising_friction` (the 0.25 surface friction while rising: 1 for
  GoldSrc), `water_jump_up` (256; 225) and `walk_scales_keys` (the walk
  key on the keys, not the max speed). The jump penalty is the stamina
  timer CS:S already had. The hull trace is `goldsrc_engine::hulls`
  (`GoldSrcHulls` over `goldsrc_formats::bsp::Bsp::trace`, the trees
  carried in the converted map as `map::clip_trees::MapClipTrees`). With
  all of it in, the four games' hashes are equal. The step, before the
  props service landed under it (of main before S7): CS:S 71 782
  (+0.60 %), HL2:DM 67 427 (+0.56 %), TF2 71 893 (+0.50 %), DoD:S 74 189
  (+0.57 %): about 60 instructions a tick over S9 for the switches'
  tests; rebased over the props service (of the entry above's numbers):
  CS:S 71 958 (+0.06 %), HL2:DM 67 658 (+0.12 %), TF2 72 070 (+0.10 %),
  DoD:S 74 743 (+0.05 %). CS 1.6's own golden run (`0x455eb29c0bc087e4`,
  1 620 ticks of 10 ms on the greybox, box sweeps): the step 71 460 a
  tick.
