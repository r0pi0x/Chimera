# Day of Defeat: Source: player movement (deltas from CS:S)

Source basis: measurements on the dedicated server (2026-11-23; build
11003710) with a puppet bot whose per-tick inputs a SourceMod plugin
replaced, logging position, velocity, flags, eye height, hull, stamina
and the prone/sprint state after every tick on flat open ground of
dod_avalanche; the server's cvar list; sprint, stamina, jumps, getting up and the slow
measured again the same day with `tools/dods_probe` (the same kind of
plugin, kept: tools/dods_probe/README.md; `measure.py` reruns every
case); prone edge cases (in the air, sprinting, ducked, under a low
ceiling), the eye curves and the regeneration's still threshold on
2026-11-24 (`measure.py proneedge ceiling stillregen`, a frozen crate
spawned as the ceiling). Where a line says "unmeasured" it rests on public descriptions of
the game only. CS:S's movement spec
(specs/cs_source/movement.md) is the base; this spec gives what differs
or what was checked to be the same.
Status: draft (measured 2026-11-23 and 2026-11-24; ladders and water
unmeasured)

## Summary

DoD:S moves like Source's shared ground/air movement at 220 u/s, with a
stronger ground friction than the cvars suggest, a lower eye (58), a
1/3-speed crouch, and a third stance, prone (eye 10, hull 24 high,
50 u/s crawl), entered and left over 1.2 s. Its sprint (`+speed`, forward
only) runs on a stamina pool of 100, and **the stamina caps the speed**:
running tops out at 120 + stamina, sprinting at 230 + stamina. Jumps
cost no stamina in this build.

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
| unprone_time | 1.2 s (80 ticks) | s | yes | getting up from prone, to standing or to crouched |
| unduck_time | 0.21 s (14 ticks) | s | yes | eye 34 → 58 over 13 ticks, the standing hull on the 14th |
| jump_speed | 262.33 standing, 268.33 ducked | u/s | yes | apex 43.0 standing, 45.0 ducked (Jumping) |
| stamina_max | 100 | — | yes | |
| sprint_cost | 15 | — | yes | taken on the tick the sprint starts |
| sprint_drain | 0.3 a tick (20/s) | — | yes | while sprinting above 100 u/s |
| regen_still / ducked moving / moving | 0.9 / 0.75 / 0.15 a tick (60 / 50 / 10 per s) | — | yes | below |
| run_cap | 120 + stamina (≤ 220) | u/s | yes | |
| sprint_cap | 230 + stamina; 120 at stamina 0 | u/s | yes | |
| sprint_min_speed | 80 | u/s | yes | the speed the sprint needs to start |
| slowed_cap | 120 | u/s | yes | while hit-slowed (damage.md) |
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
- **The eye curve** (going down and getting up alike, measured tick by
  tick): over the 80 ticks the eye moves by 48 × f(t) with t the
  fraction of the 1.2 s gone and f(t) = 1 − (1 − t)^(7/3) (a fit: at
  t = 1/80, 1/4, 1/2, 3/4 it gives 0.029, 0.488, 0.801, 0.961 against
  the measured 0.029, 0.487, 0.800, 0.960; the getting-up values, eye
  10 → 58: 11.38, 12.75, 14.07 ... 33.38 at tick 20, 48.40 at 40, 55.07
  at 56, 57.70 at 71, 58.00 at 79).
- **Pressed in the air** (5 ticks after a jump): going prone starts at
  once; the eye eases down while the player flies the rest of the jump
  with the standing hull (the jump is not cut) and lands; prone 80
  ticks after the press as on the ground.
- **Pressed while sprinting** (≈305 u/s): the input stops counting but
  the speed is not cut: it decays by ground friction (305 → 199 after
  36 ticks, 80 after 64), the sprint flag stays and drains stamina
  (0.3 a tick) until the speed falls below about 80, then the
  regeneration runs; prone at the end of the 80 ticks, crawling at 50
  if forward is held.
- **Pressed while ducked**: the eye eases 34 → 10 over the 80 ticks
  (the same curve over 24 units), the hull staying the ducked one (45)
  until prone.

### Stamina and sprint (measured)

The player has a stamina pool of 100 (full at spawn). Each tick on the
ground:

1. **Sprinting** needs `+speed` held with forward input (forward alone
   or forward with a side key; back or side alone never sprints, measured
   at 220 with the flag clear), standing (holding duck: no sprint) and a
   speed of at least **80 u/s** on the previous tick (wish 79: never
   sprints; wish 80: sprints once at 80). Stamina is **not** needed: at 0
   the sprint flag still sets.
2. **Starting** a sprint costs **15** stamina at once (never below 0).
3. While sprinting faster than **100 u/s** stamina drains **0.3 a tick**
   (20 per second, from 85: empty after about 4.3 s). Sprinting at 100
   or slower (a short forward input) it regenerates at the moving rate
   instead.
4. Not sprinting, it **regenerates**: **0.9 a tick** (60/s) standing
   still (standing, ducked, prone or in the air; "still" is a
   horizontal speed of exactly 0: a player sliding to a stop with no
   input, or creeping at 4.5 u/s, regenerates at the moving rate until
   the speed is 0), **0.75 a tick** (50/s)
   moving ducked, **0.15 a tick** (10/s) moving standing or crawling
   prone. No delay: the first tick after a sprint ends already gains.
   Capped at 100.
5. **The speed cap follows the stamina** (the speed is clamped to it the
   same tick, not slowed by friction):
   - running (not sprinting): 120 + stamina, so 220 only at full
     stamina (measured: stamina 90 → 210, 50 → 170, 10 → 130);
   - sprinting: 230 + stamina while stamina is above 0 (90 → 320,
     50 → 280, 10 → 240; from a full pool the cost leaves 85, so the top
     sprint speed is 315 and falls 0.3 a tick with the drain:
     312.3 → 230 over about 4.3 s); at stamina 0 the cap is 120;
   - ducked (73.33) and prone (50) speeds don't depend on it;
   - hit-slowed: 120 (damage.md) whatever the stamina.

Measured from rest with forward and `+speed` held, stamina 100: speeds
0, 48.00, 90.00, 127.20 (sprint starts: stamina 85.0), 159.19, 187.29,
211.97, 233.65, 252.68, 269.39, 284.04, 296.89, 308.15, 312.30 (cap
82.3 + 230), then −0.3 a tick with the stamina; stamina 0 at about 4.3 s,
then 120. Releasing `+speed` at stamina 67.9 while running: speed 187.90
the next tick (120 + stamina), then +0.15 a tick with the regeneration.

Firing an undeployed MG costs stamina too (weapons.md): that fits these
rates (15 on the first shot, then the shots' cost against 0.9 a tick
standing).

### Jumping (measured)

- Jumping costs **no stamina** and its height does not depend on it
  (stamina 0 and 100 give the same jump; sprinting, the drain simply
  continues in the air at 0.3 a tick).
- Standing: the vertical speed after the jump tick reads 250.33 and the
  first tick rises 3.845 units, as Source's jump of √(2 × 800 × 43.01)
  = 262.33 u/s with gravity 800 (apex 43.01 units, 43 ticks in the air
  on flat ground).
- Ducked (fully crouched, duck held): 256.33 after the tick and 3.935
  units the first tick: √(2 × 800 × 45) = 268.33 u/s, apex 44.99, 44
  ticks in the air.
- Duck pressed on the jump tick from standing: the same 256.33, and the
  feet rise 13.6 units extra on the next tick as the box shrinks in the
  air (as Source's air duck); apex 58.0 measured to the feet.
- Running at 220 or sprinting: the same vertical numbers; the
  horizontal speed is kept.
- Prone: the jump key does nothing (no jump, no getting up).
- Something above the head stops the rise (one spot on dod_avalanche
  with a hidden obstacle about 30 units above the head cut a jump at
  30.35 and refused jumps after a placement: that was the old probe's
  "can't jump").

### Getting up (measured)

- **From a crouch**: releasing duck eases the eye 34 → 58 over 13 ticks
  (34.000, 34.385, 35.458, 37.098, 39.184, 41.594, 44.206, 46.899,
  49.552, 52.043, 54.250, 56.052, 57.328, 57.956), then on the 14th tick
  (0.21 s) the hull becomes 72 and the ducked flag clears (eye 58).
- **From prone, `+alt1` again**: on the press the prone flag clears and
  the hull is the **standing** one at once; the eye eases 10 → 58 over
  1.2 s (80 ticks; in two of the first runs it eased to 34 and jumped to
  58 at the end; not reproduced since). The player **can move** while
  getting up: with forward held the speed climbs from 7.5 on the first
  tick to 220 at the end (7.51, 9.06, 10.66, 12.35 ... 46.04 at tick 16,
  95.09 at 28, 135.01 at 40, 172.11 at 52, 202.32 at 64, 218.77 at 76,
  220.00 at 80): the cap rises with the getting up.
- **From prone, duck pressed**: the player goes to a crouch: on the
  press the prone flag clears and the hull is the ducked one (45); the
  eye eases 10 → 34 over 1.2 s (80 ticks); movement is allowed, the speed
  creeping from 7.5 towards the ducked 73.3 (57.95 at tick 32, 69.07 at
  60, 72.60 at 80). With duck released the player then stands up as from
  a crouch (0.21 s more).
- **A ceiling too low** (measured: prone under a frozen crate): the
  key is refused and nothing changes (no partial rise, the prone flag
  and hull stay; pressing again later is refused again):
  - ceiling 50 above the ground: `+alt1` (stand, needs 72) refused;
    duck (needs 45) works, eye 10 → 34;
  - ceiling 40: duck refused; ceiling 30: both refused.

### View

- Eye heights as above. The view-model offsets change with stance:
  each weapon script has a normal, a prone and an iron-sight offset
  (weapons.md).
- Blasts stun the view: `dod_stun_min/max_pitch` 30/50,
  `_yaw` 120/150, `_roll` 15/30 (degrees, cvars; how they are applied
  is unmeasured).
- `sv_rollangle` 0: no strafe roll.
- **Sighted** (the Garand's and K98's iron sights, measured): the speed
  is capped at **42 u/s** while sighted (running at 220 into the sights:
  168 on the press, about 190 during the 0.3 s raise, then 42.0 from
  the moment the FOV reaches 55; back to 220 by normal acceleration after
  leaving them). From rest a sighted player accelerates very slowly
  (the cap barely exceeds what friction takes: 2.4 u/s a tick in one
  run, creeping at 4.5 in another; open question 8). A sighted player
  never passes the spread's 45 u/s move threshold (weapons.md).

### Being slowed when hit

A hit sets the victim's "slowed until" time 0.5 s ahead (2.0 s for a
leg hit; damage.md). While it runs the horizontal speed is capped at
**120 u/s** at once (measured; damage.md, "Being slowed when hit").

## Per-tick order

As CS:S's spec, with the stance transitions (duck, prone) advanced in
the duck step. The stamina caps a tick's speed with the stamina as it
was before that tick's drain or regeneration (sprinting: speed = 230 +
the previous tick's stamina), and the sprint's start looks at the
previous tick's speed. Unmeasured beyond that.

## Probe notes (2026-11-23)

The first probe's bot "could not" jump, sprint or get up. The cause was
ours: it was placed 2 units above the ground (z 130 on ground at
128.03), and at a spot with something low overhead. Placed on the
ground at an open spot, the bot sprints, jumps and gets up as a player
does (`tools/dods_probe/README.md`).

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
| standing, stamina 100 | hold forward + `+speed` from rest | 3 ticks | 127.20 u/s, sprinting, stamina 85.0 |
| sprinting, stamina 82.3 | hold | 1 tick | speed 312.30, stamina 82.0 |
| sprinting, stamina 0.1 | hold | 2 ticks | stamina 0, speed 120 |
| running 298 (sprint), stamina 67.9 | release `+speed`, keep forward | 1 tick | 187.90 u/s, stamina 68.05 |
| stamina 50, running | hold forward | long | 170 u/s (stamina rising 0.15 a tick) |
| stamina 20, still | nothing | 1 tick | 20.9 |
| stamina 20, ducked and moving | | 1 tick | 20.75 |
| wish 79, `+speed` + forward | | long | 79 u/s, no sprint |
| `+speed` + back | | long | 220 u/s, no sprint, stamina unchanged |
| standing | jump | 1 tick | vz 250.33, z +3.845 |
| standing | jump | | apex 43.01, lands after 43 ticks, stamina unchanged |
| ducked | jump | 1 tick | vz 256.33; apex 44.99 |
| stamina 0 | jump | | apex 43.01 |
| prone | jump | | nothing |
| ducked, eye 34 | release duck | 13 ticks / 14 ticks | eye 57.956, hull 45 / eye 58, hull 72 |
| prone | press `+alt1`, hold forward | 1 / 80 ticks | standing hull, 7.5 u/s / 220 u/s, eye 58 |
| prone | press duck | 80 ticks | ducked, eye 34 |
| running 220 | slowed (hit) | 1 tick | 120 u/s |
| prone | press `+alt1` | 1 / 20 / 40 / 79 ticks | eye 11.38 / 33.38 / 48.40 / 58.00 |
| standing | jump, press `+alt1` 5 ticks later | | lands normally; prone 80 ticks after the press |
| sprinting ≈305 | press `+alt1` | 36 ticks | ≈199 u/s, still sprinting, no input effect |
| ducked | press `+alt1` | 80 ticks | prone; hull 45 until then |
| prone under a ceiling 50 up | press `+alt1` | | nothing |
| prone under a ceiling 50 up | press duck | 80 ticks | ducked, eye 34 |
| prone under a ceiling 40 up | press duck | | nothing |
| sliding from 150 with no input, stamina 20 | | each tick until stopped / after | +0.15 / +0.9 |
| running 220, Garand | alt fire (sights) | after the raise | 42 u/s |

## Open questions

1. Leaving prone: answered (time, movement, keys, low ceilings, the eye
   curve) except the exact law of the speed cap while getting up (the
   measured speeds above are the reference).
2. The friction law: does it hold on every surface; is it a doubled
   friction or a scaled stop speed (they differ only in naming)?
3. Sprint: answered, and the regeneration's still threshold: speed 0
   exactly (every tick with any speed regenerates at the moving rate).
4. Jump: answered (no stamina cost or scaling in this build; public
   descriptions of a stamina cost are of older versions or wrong).
5. The slow: answered (120 cap; the sprint keeps draining).
6. Unducking: answered (0.21 s).
7. Ladders and water: assumed as CS:S's (unmeasured).
8. The sighted speed from rest: accelerating from rest while sighted
   gave different curves in two runs (2.4 u/s a tick to 36.6 then a drop
   to 4.45; creeping at 4.5 throughout). The 42 cap is steady.
