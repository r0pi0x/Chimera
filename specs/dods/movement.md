# Day of Defeat: Source: player movement (deltas from CS:S)

Source basis: measurements on the dedicated server (2026-11-23; build
11003710) with a puppet bot whose per-tick inputs a SourceMod plugin
replaced, logging position, velocity, flags, eye height, hull, stamina
and the prone/sprint state after every tick on flat open ground of
dod_avalanche; the server's cvar list. Where a line says "unmeasured"
it rests on public descriptions of the game only. CS:S's movement spec
(specs/cs_source/movement.md) is the base; this spec gives what differs
or what was checked to be the same.
Status: draft (partly measured 2026-11-23; jump, sprint, stamina and
leaving prone/duck unmeasured, see "Probe limits")

## Summary

DoD:S moves like Source's shared ground/air movement at 220 u/s, with a
stronger ground friction than the cvars suggest, a lower eye (58), a
1/3-speed crouch, and a third stance, prone (eye 10, hull 24 high,
50 u/s crawl), entered over 1.2 s. Its sprint (`+speed`) runs on a
stamina pool that jumping also drains (unmeasured here).

## Units and conventions

Source units, z up, degrees. Tick 0.015 s (the server sets 66.7 Hz).

## Constants

| Name (ours) | Value | Unit | Measured? | Meaning |
|---|---|---|---|---|
| run_speed | 220 | u/s | yes, every class measured, forward/back/side | top ground speed with forward or side input |
| back_speed_factor | 1 | — | yes | no back-pedal penalty (`sv_backspeed` 0) |
| accelerate | 10 | — | yes (cvar, and 33 u/s gained per tick at 220 wish) | `sv_accelerate` |
| air_accelerate | 10 | — | cvar only | `sv_airaccelerate` (CS:S 10) |
| ground_friction_effective | 8 | 1/s | yes | drop per tick = max(speed, 50) × 8 × 0.015 (below) |
| stop_speed_effective | 50 | u/s | yes | the control speed floor in that drop |
| sv_friction / sv_stopspeed (cvars) | 4 / 100 | | cvar | listed values; their product (400) equals the measured 8 × 50 |
| gravity | 800 | u/s² | cvar | `sv_gravity` |
| step_size | 18 | u | cvar | `sv_stepsize` |
| max_velocity | 3500 | u/s | cvar | `sv_maxvelocity` |
| player maxspeed field | 600 | u/s | yes | the player's speed cap field reads 600 in every state; the real caps are the stance speeds |
| hull_stand | (−16,−16,0)–(16,16,72) | u | yes | as CS:S |
| eye_stand | 58 | u | yes | CS:S: 64 |
| hull_duck top | 45 | u | yes | ducked hull (−16,−16,0)–(16,16,45) (width assumed unchanged) |
| eye_duck | 34 | u | yes | |
| duck_speed_factor | 1/3 | — | yes | 73.33 u/s |
| duck_time | ≈0.42 s (28 ticks) | s | yes | eye 58 → 34 eased; the hull and the ducked flag switch on the 28th tick |
| hull_prone top | 24 | u | yes | prone hull (−16,−16,0)–(16,16,24) (width not read) |
| eye_prone | 10 | u | yes | |
| prone_speed | 50 | u/s | yes | crawl; `+speed` while prone does not change it |
| go_prone_time | 1.2 s (80 ticks) | s | yes | eye eased 58 → 10; no movement during it; the hull and prone flag switch at the end |
| unprone_time | unmeasured | s | no | the game's own timer field exists (open question 1) |
| fall damage | `mp_falldamage` 0 | | cvar | the classic Source fall rule (as CS:S) unless measured otherwise |

## Behavior

### Ground speed, acceleration and friction (measured)

Each tick on the ground, with speed v and wish speed w (220 standing,
73.33 ducked, 50 prone):

1. Friction: v ← v − max(v, 50) × 8 × 0.015 (never below 0).
   Equivalently, above 50 u/s the speed is multiplied by 0.88 per tick;
   below it, it drops by 6 u/s per tick.
2. Acceleration (Source's): add min(10 × w × 0.015, w − v·wishdir).

Measured: standing start 0, 33.00, 60.00, 85.80, 108.50, 128.48,
146.06, 161.54, 175.15, 187.13, ... 220; stop from 220: 193.60,
170.37, 149.92, 131.93, 116.10, 102.17, 89.91, 79.12, 69.62, 61.27,
53.92, 47.45, 41.45 (then −6 per tick).

CS:S's spec uses friction 4 and stop speed 100; the two laws agree
below 50 u/s and differ above it. Whether DoD:S doubles the friction
(and halves the stop speed) or something else scales it, the measured
law above is what to implement. Open question 2.

### Ducking (measured)

- Holding duck: the eye moves from 58 to 34 over 28 ticks with an
  ease (58.00, 57.90, 57.61, 57.15, 56.54, 55.78, 54.90, 53.90, 52.81,
  51.64, 50.40, 49.11, 47.79, 46.44, 45.10, 43.76, 42.44, 41.17, 39.95,
  38.81, 37.75, 36.79, 35.94, 35.23, 34.67, 34.26, 34.04, 34.00); the
  hull drops to 45 and the ducked flag sets on tick 28.
- The 1/3 speed applies from the first tick of the duck key, not when
  fully ducked (first tick +11 u/s from rest).

### Prone (measured: entering, crawling)

- `+alt1` (one press) starts going prone: the eye eases from 58 to 10
  over 1.2 s (80 ticks: 56.61, 55.25 ... 46.38 at tick 9, 35.57 at 19,
  26.87 at 29, 20.16 at 39, 15.31 at 49, 12.15 at 59, 10.47 at 69,
  10.00 at 79); the player cannot move meanwhile; at the end the hull
  becomes 24 high and the prone flag sets.
- Crawling: wish speed 50 (start 7.5, then +1.5 per tick to 50; stop
  −6 per tick). Sprint does not speed it up.
- Prone is the stance the MGs deploy in anywhere (weapons.md).

### Sprint and stamina (unmeasured)

From public descriptions: holding `+speed` while moving forward sprints
faster than the run speed, draining a stamina pool of 100 (the player's
stamina field reads 100 at spawn, measured); stamina regenerates when
not sprinting; jumping costs stamina and low stamina weakens jumps.
The probe could not make the bot sprint (its sprint flag never set);
every number here is open (question 3).

### Jumping (unmeasured)

The probe's bot never left the ground on the jump key (see "Probe
limits"). Public descriptions: a jump is lower than CS:S's and costs
stamina; no jumping while prone. Open question 4.

### View

- Eye heights as above. The view-model offsets change with stance:
  each weapon script has a normal, a prone and an iron-sight offset
  (weapons.md).
- Blasts stun the view: `dod_stun_min/max_pitch` 30/50,
  `_yaw` 120/150, `_roll` 15/30 (degrees, cvars; how they are applied
  is unmeasured).
- `sv_rollangle` 0: no strafe roll.

### Being slowed when hit

A hit sets the victim's "slowed until" time 0.5 s ahead (2.0 s for a
leg hit; damage.md). The slow's effect on speed is unmeasured
(question 5).

## Per-tick order

As CS:S's spec, with the stance transitions (duck, prone) advanced in
the duck step. Unmeasured beyond that.

## Probe limits (2026-11-23)

In three runs the puppet bot (the game's own `bot` command, inputs
replaced each tick) moved, ducked and went prone as commanded, but:

- never jumped (one-tick and held jump, standing and running, on flat
  ground, ducked flag clear, stamina 100),
- never sprinted (the sprint flag stayed 0 with `+speed` held while
  moving forward),
- never came back up from a full duck or from prone (releasing duck;
  pressing `+alt1` again, with jump, with duck).

The buttons reached the player (its button field showed them), so the
game itself refuses these actions for this bot. A likely cause is a
game state the bot is in (the round or warm-up state, or something the
real client sends that a puppet bot doesn't); finding it is the first
task of a proper DoD:S probe (as `tools/hl2dm_probe`).

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| standing still | hold forward | 1 tick | 33.00 u/s |
| standing still | hold forward | 3 ticks | 85.80 u/s |
| running 220 | release | 1 tick | 193.60 u/s |
| moving 47.45 | release | 1 tick | 41.45 u/s |
| standing still | hold duck + forward | 1 tick | 11.00 u/s |
| standing | hold duck | 28 ticks | eye 34, hull top 45 |
| standing | press `+alt1` | 79 ticks | eye 10.00, still standing hull |
| standing | press `+alt1` | 80 ticks | prone, hull top 24 |
| prone, still | hold forward | 1 tick / 2 ticks | 7.5 / 9.0 u/s |
| prone, still | hold forward | long | 50 u/s |
| backward input | hold back | long | 220 u/s |

## Open questions

1. Leaving prone: its time, whether movement is blocked, what blocks it
   (low ceiling), and what `+alt1` / jump / duck do while prone.
2. The friction law: does it hold on every surface; is it a doubled
   friction or a scaled stop speed (they differ only in naming)?
3. Sprint: speed, stamina drain per second, regeneration rate and
   delay, the minimum stamina to start, whether sideways or backward
   input sprints.
4. Jump: impulse, stamina cost, the stamina-scaled jump, crouch-jump.
5. The slow when hit: speed factor or cap, and its interaction with
   sprint.
6. Unducking time (CS:S: 0.2 s).
7. Ladders and water: assumed as CS:S's (unmeasured).
