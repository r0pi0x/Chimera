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
| S1-S4 carve inside `source_engine` | not started | file split, neutral hooks, CS:S's numbers to `cs_source`, cvars as data |
| S5-S6 the crate | not started | `chimera_movement`, then neutral names with Source aliases |
| S7-S9 GoldSrc seams | not started | clip hulls, command milliseconds, edge friction |
| Hand-off | | goldsrc.md M2: the hull tracer in `goldsrc_engine`, `cs16:movement` |

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
`pub use chimera_movement::*` plus the aliases `SourceMovement =
QuakeMovement`, `SourceMovementConfig = QuakeConfig`,
`SourceMovementConfigs`, `SourceStance = MoveStance`, `SourceSpeed =
MoveSpeed`, `SourceTraits = MoveTraits`, `SourceMovementStep =
MovementStep`, `to_source = to_units`; `CVARS` (Source's extras),
`SourceMovementAppExt`, and the `func_useableladder` reader. Callers keep
compiling unchanged; moving them to the neutral names is optional and
mechanical, one game a commit, later.

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
  go at the end.
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
