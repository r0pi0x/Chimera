# Team Fortress 2: player movement (T-M1)

Source basis: the dedicated server install (build 11191711, 64-bit
server, run locally on a LAN-only port with no clients): its console
variable defaults read with the console's variable listing and one query
per variable (the listing rounds fractions; the per-variable query does
not), and per-tick measurements of server-side puppet bots on the map
`itemtest`, read through the game's public scripting interface (position,
velocity, flags, eye offset, bounds, max speed, health) while the bots'
buttons were forced (jump, duck, secondary fire) or their velocity or
position was set. Public community documentation of the classes and of
jumping, speed, fall damage and air movement (the community TF2 wiki),
for what could not be measured without a client (anything driven by
move keys). The class scripts are encrypted and were not opened. No
code of the game or engine was read.
Status: draft

## Summary

TF2 runs the shared Source controller of specs/cs_source/movement.md
("movement.md") with gravity 800, accelerate 10, airaccelerate 10,
friction 4, stopspeed 100 and a 66.67 Hz tick, and adds a class layer:
each class has its own top speed (230 to 400), a 0.9 cap when moving
backwards, a single 48 × 48 × 82 box (62 ducked) for everyone, class eye
heights (65, 68 or 75; 45 ducked) and a fixed 289 units/s jump that
peaks at 50 units (70 with an air duck). The Scout jumps once more in
the air: the air jump replaces the vertical speed with 268.33 and the
horizontal velocity with whatever the keys ask for, which is what makes
him hard to track. A jump never leaves the ground faster than 1.2 × the
class's top speed (the anti-bunny-hop cap), so air-strafing still
pays but chained hops cannot snowball. Falls hurt in proportion to max
health above 650 units/s, with ±20 % randomness.

This spec states only what differs from, or adds to, movement.md (the
shared Source controller); everything not named here is as there.

## Units and conventions

- Hammer units (1 unit ≈ 1 inch), Z up, velocities in units/s, angles in
  degrees, pitch positive looking down (as movement.md).
- **Tick dt = 0.015 s (66.67 Hz).** The server prints "setting tickrate
  to 66.7" on map start and the scripting interface's frame time reads
  0.015; TF2 has no tick-rate option in normal play. All numbers below use
  dt 0.015 and gravity 800, so a gravity half-step is 800 × 0.0075 =
  **6 units/s**, a whole step 12.
- Duck timers in milliseconds go down by 15 per tick.
- "Tick 1" is the first tick whose command carries the new input (the
  jump or duck press). "Stored" velocity is the velocity at the end of a
  tick (after the second gravity half-step).
- Team numbers: 2 RED, 3 BLU (see specs/tf2/classes.md).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| gravity | 800 | units/s² | `sv_gravity` |
| accelerate / airaccelerate | 10 / 10 | – | `sv_accelerate`, `sv_airaccelerate` |
| friction / stopspeed | 4 / 100 | – / units/s | `sv_friction`, `sv_stopspeed` |
| step | 18 | units | `sv_stepsize` |
| input_speed | 450 | units/s | `cl_forwardspeed`, `cl_sidespeed`, `cl_backspeed`; `cl_upspeed` 320 |
| class_speed | 230–400 | units/s | per class, table below |
| back_frac | 0.9 | – | top speed while moving backwards |
| duck_frac | 1/3 | – | shared ducked input scale (movement.md) |
| swim_frac | 0.8 | – | shared water wish scale (movement.md) |
| speed_ceiling | 520 | units/s | highest top speed any effect can give (public) |
| hull_stand | (−24,−24,0)–(24,24,82) | units | every class |
| hull_duck | (−24,−24,0)–(24,24,62) | units | every class |
| eye_duck | 45 | units | every class |
| air_duck_lift | 20 | units | 82 − 62 |
| duck_time / unduck_time | 0.2 / 0.2 | s | eye blend length both ways |
| jump_impulse | 289 | units/s | ground jump, every class, added to vz |
| air_jump_vz | 268.3281573 | units/s | Scout's air jump, replaces vz (= √(2·800·45)) |
| bhop_cap_frac | 1.2 | – | ground jump caps speed at 1.2 × top speed |
| fall_safe | 650 | units/s | fall damage above |
| fall_scale | 1/6000 | max health per unit/s | damage = max health × speed / 6000 |
| fall_spread | ±20 % | – | uniform; off with `tf_fall_damage_disablespread` 1 |

## Behavior

### Console variables

Defaults on the dedicated server, build 11191711, no config files
present (`server.cfg` and the map config were absent), so these are the
registered values:

| Variable | Default | Note |
|---|---|---|
| `sv_gravity` | 800 | |
| `sv_accelerate` | 10 | |
| `sv_airaccelerate` | 10 | |
| `sv_friction` | 4 | |
| `sv_stopspeed` | 100 | |
| `sv_maxspeed` | 320 | **not** a cap on class speed: the Scout's 400 and the 520 ceiling exceed it (Open questions 1) |
| `sv_stepsize` | 18 | |
| `sv_bounce` | 0 | |
| `sv_wateraccelerate` | 10 | |
| `sv_waterfriction` | 1 | |
| `sv_maxvelocity` | 3500 | per axis, as movement.md |
| `sv_backspeed` | 0.6 | archived, replicated; the listing shows "0" (it rounds). TF2's measured back speed is 0.9, so this one is not TF2's back cap (Open questions 2) |
| `sv_rollangle` / `sv_rollspeed` | 0 / 200 | no view roll from strafing |
| `sv_noclipspeed` / `sv_noclipaccelerate` | 5 / 5 | |
| `sv_specspeed` / `sv_specaccelerate` | 3 / 5 | |
| `sv_footsteps` | 1 | |
| `sv_waterdist` | 12 | view fix-up near the water plane |
| `cl_forwardspeed`, `cl_sidespeed`, `cl_backspeed` | 450 | cheat-protected |
| `cl_upspeed` | 320 | |
| `tf_clamp_airducks` | 1 | limits ducks per air time (Ducking) |
| `option_duck_method` | 1 | client option (hold vs toggle; not movement physics) |
| `xc_crouch_debounce` | 0 | controller crouch debounce |
| `mp_falldamage` | 0 | the shared rule's switch; TF2 uses its own formula (Falling) |
| `tf_fall_damage_disablespread` | 0 | 1 removes the ±20 % |
| `tf_avoidteammates_pushaway` | 1 | teammates push each other apart when overlapping (they never collide) |
| `tf_resolve_stuck_players` | 1 | |
| `tf_player_movement_restart_freeze` | 1 | no movement during the round-restart freeze |
| `tf_preround_push_from_damage_enable` | 0 | |
| `tf_movement_aircurrent_aircontrol_mult` | 0.25 | cheat; air control multiplier while in an "air current" (after an airblast) |
| `tf_movement_aircurrent_friction_mult` | 0.75 | cheat; wall-slide friction in an air current |
| `tf_movement_lost_footing_friction` | 0.1 | cheat; ground friction while "lost footing" |
| `tf_movement_lost_footing_restick` | 50 | cheat; the lost-footing state ends early below this ground speed |
| `tf_grapplinghook_enable` | 0 | grappling hook off (its `tf_grapplinghook_*` tuning is out of scope) |
| `smoothstairs` | 1 | smooths the eye height over stairs (view only) |

Absent in TF2 (unknown commands): `sv_enablebunnyhopping`,
`sv_autobunnyhopping`, `sv_timebetweenducks`, any `tf_parachute_*`,
`tf_scout_air_dash_count`, `tf_clamp_back_speed`. The air-current and
lost-footing states belong to airblast and later specs; with neither
active, movement is as below.

### Class top speed

The player's max speed P comes from the class, then from conditions and
weapons. Measured P (the max-speed value of puppet bots of each class,
standing, holding their stock primary):

| Class | P | Backwards (0.9 P) | Ducked (P/3) | Swimming (0.8 P) |
|---|---|---|---|---|
| Scout | 400 | 360 | 133.33 | 320 |
| Soldier | 240 | 216 | 80 | 192 |
| Pyro | 300 | 270 | 100 | 240 |
| Demoman | 280 | 252 | 93.33 | 224 |
| Heavy | 230 | 207 | 76.67 | 184 |
| Engineer | 300 | 270 | 100 | 240 |
| Medic | 320 | 288 | 106.67 | 256 |
| Sniper | 300 | 270 | 100 | 240 |
| Spy | 320 | 288 | 106.67 | 256 |

The community wiki gives the same speeds as percentages of 300 (Scout
133 %, Soldier 80 %, …) and its backward, crouched and swimming columns
are these fractions. Ducking and swimming are the shared input scale
(1/3) and water wish scale (0.8) of movement.md applied to P; nothing
TF2-specific is needed for them. P does **not** change while ducked (the
max-speed value stays 400 on a ducked Scout).

The movement cap is M = P (the shared min with `sv_maxspeed` is not
applied: Open questions 1), times the ground material's speed factor;
then movement.md's input rescale and wish clamp. With 450 inputs the
rescale always binds, so holding forward asks for exactly M.

**Backwards.** While the move is backwards, the cap is 0.9 M. The
public sources state the fraction (and a 468 = 0.9 × 520 backward
ceiling) but not what counts as backwards for a diagonal: take "the
forward move is negative" as the rule, applying 0.9 M as both the
rescale target and the wish clamp (Open questions 2).

**Speed ceiling.** No effect raises the cap above 520 forwards (468
backwards, 173.33 ducked, 416 swimming) (public; the ceiling was raised
from 450 in 2012).

### Ground and air acceleration, friction

As movement.md with the numbers above: friction removes 6 % a tick above
100 units/s and 6 units/s a tick below; ground acceleration adds up to
10 × wish × 0.015 = 0.15 × wish a tick (60 for a Scout, 34.5 for a
Heavy); air acceleration adds up to 0.15 × wish but only until the
projection reaches 30. Measured: a Scout set sliding at 400 with no
input on flat ground has 376.0, 353.44, 332.234, 312.3, 293.562 … then
from 102.535 below 100 it loses 6 a tick (96.383, 90.383, …, 6.383) and
stops on the 41st tick. Air-strafing works exactly as movement.md's
(30 units/s per tick perpendicular at any class speed, since 0.15 × wish
≥ 30 for every class: the Heavy's 34.5 is the smallest).

### Jumping

As movement.md's Jumping, with these differences:

- **Impulse 289**, every class, not recomputed from gravity: added to vz,
  then the extra gravity half-step. Standing on flat ground the jump
  tick's move uses 289 − 12 = 277 and stores 271; tick k's move uses
  277 − 12(k − 1). Apex **50.04** at the end of tick 24.
- **No jump while fully ducked on the ground.** A grounded player whose
  duck is complete cannot jump (measured: a bot held fully ducked with
  jump forced never left the ground). A "crouch jump" is a jump followed
  by a duck in the air. Whether a jump is allowed during the 0.2 s duck
  or unduck transition is not measured (Open questions 3).
- **Anti-bunny-hop cap.** On the jump tick, after the first gravity
  half-step and before the impulse: if the speed |v| (all three
  components) exceeds 1.2 × M, v is scaled down to length 1.2 × M. Then
  the impulse and the extra half-step apply as usual. Measured with v set
  to (600, 0, 0) the tick before the jump:

  | Class (M) | Cap | Stored after the jump tick |
  |---|---|---|
  | Scout (400) | 480 | vx 479.976, vz 272.2 |
  | Soldier (240) | 288 | vx 287.986, vz 274.12 |

  The vz values show the scaling includes the −6 already in vz:
  (600, 0, −6) has length 600.03; scaled to 480 that is (479.976, 0,
  −4.8), then +289 − 6 − 6 = 272.2. Below the cap nothing changes
  (Scout at 450 keeps 450; Soldier at 280 keeps 280). So a ground jump
  never leaves faster than 1.2 × M horizontally (480 Scout, 288 Soldier,
  276 Heavy, 336 Pyro/Engineer/Sniper, 384 Medic/Spy, 336 Demoman), and
  each hop of a chain starts from at most that, but speed gained in the
  air by strafing is kept until the next jump. Which M the cap uses
  while a weapon or condition changes P (spun minigun, zoom) is taken to
  be the current one (Open questions 4).
- **Friction** is skipped on the jump tick (shared).
- The jump fires on a fresh press only (shared); a forced held jump
  re-fires on the first grounded tick (puppet-bot measurement; Open
  questions 5).

### The Scout's air jump

Only the Scout (stock loadout; items that add jumps are later specs).

- **When:** jump pressed fresh (not held from the previous tick) while
  not on the ground, not in water at waist depth or deeper, and the air
  jump not yet used since leaving the ground. One per air time; landing
  resets it. A third press does nothing (measured). The wiki says it can
  be used at any point in the air, which includes walking off a ledge
  without a ground jump (not measured).
- **Vertical:** vz is **replaced** by 268.3281573 (after the tick's first
  gravity half-step), with no extra half-step; the move uses 268.328 and
  the tick stores 262.328. From any fall speed the air jump therefore
  gives the same rise: the moves use 268.328 − 12(k − 1), so the apex is
  **47.03** above the height at the start of the air-jump tick (end of
  its 23rd tick). Measured: air jump on tick 19 of a standing jump
  (stored vz 67 before it): stored 262.328, then 250.328, 238.328, …,
  apex 47.03 higher than the start of that tick (Test case 7).
- **Horizontal:** the horizontal velocity is **replaced** by the wish
  velocity of this tick's keys: the flattened view forward × f plus right
  × s after movement.md's rescale to M (so its length is M when any move
  key is held, e.g. 400 for a Scout, 360 backwards). With no key held
  it becomes 0: measured, a Scout moving at 300 in the air with no keys
  stops dead horizontally on the air jump. This is the redirect players
  use to dodge. With keys held the result is not measured (no client
  input); that it equals the full wish velocity is the public
  description ("in any direction") plus the no-key measurement (Open
  questions 6).
- The anti-bunny-hop cap does not apply (it has nothing to cap: the new
  horizontal is at most M).
- The air jump plays the double-jump animation; it is not a fresh ground
  jump for footsteps.

### Ducking

The shared duck logic with TF2's box and timing:

- Box 82 → 62 tall (48 wide), eye from the class eye height (65, 68 or
  75) to **45 for every class**.
- **0.2 s both ways** (movement.md's 0.4 s duck becomes 0.2 s): the eye
  follows E − (E − 45)·S(x), S(x) = 3x² − 2x³, x = elapsed/0.2, on the
  ground; the box switches on the first tick with elapsed > 0.2, i.e.
  **14 ticks after the press** (0.21 s), same for the unduck.
  Measured on a Scout (E = 65), ticks 1–13 after the duck press:
  64.6794, 63.785, 62.4181, 60.68, 58.6719, 56.495, 54.2506, 52.04,
  49.9644, 48.125, 46.6231, 45.56, 45.0369; tick 14: 45 and the 62 box.
  The unduck mirrors it (45.3206, 46.215, …, 64.9631; tick 14: 65 and
  the 82 box). During both transitions the small box stays in use.
- **In the air** a duck is instant: the box switches and the feet rise
  20 (the head stays). Measured: a duck pressed on tick 9 of a jump
  moves the origin up 20 that tick (−163.769 → −141.054 with the 2.715
  of the move), eye 45; the apex becomes **70.04** above the floor.
  Unducking in the air needs 20 units free below the feet (shared rule
  with TF2's hull difference). A ducked player that lands stays ducked
  while duck is held.
- **Air duck limit** (`tf_clamp_airducks` 1): a player may duck at most
  twice in one air time (public; not measured). The count resets on
  landing.
- Ducked ground speed is the shared 1/3 input scale: wish = M/3
  (133.33 Scout, 76.67 Heavy).

### Swimming and water

As movement.md: swimming wish = 0.8 × the (rescaled) input, so the class
swim speeds are 0.8 P (table above); `sv_wateraccelerate` 10,
`sv_waterfriction` 1. Landing in water deeper than 1 unit cancels fall
damage (public; as movement.md's "in water, nothing").

### Falling and landing

- **Fall speed** F = −vz at the start of the landing tick (before its
  first gravity half-step), as movement.md records it.
- **Damage** when F > 650, applied on landing:

  ```math
  D = \operatorname{round}\!\left(H_{max} \cdot \frac{F}{6000} \cdot r\right), \qquad r \sim U[0.8,\ 1.2]
  ```

  with H_max the player's max health and r = 1 when
  `tf_fall_damage_disablespread` is 1. Measured on a Heavy (300) with
  the spread off: F 648 → 0, 652 → 33, 664 → 33, 672 → 34, 684 → 34
  (34.2), 692 → 35 (34.6), 736 → 37 (36.8), 952 → 48 (47.6); on a
  Soldier (200): 796 → 27 (26.53), 1384 → 46 (46.13), 3424 → 114. Every
  sample fits round-to-nearest of H_max·F/6000 and none fits F + 6. With
  the spread on, 24 drops at F 1384 (base 69.2) gave 55–81 (0.795 to
  1.17 of the base). Rounding after the spread is assumed (Open
  questions 7).
- So the smallest damaging fall costs about 10.85 % of max health (the
  wiki's table: "5 × F/300 per cent" is the same F/6000 of max health),
  reached after about 264 units of free fall from rest.
- Fall damage ignores the shared 580/1024 numbers of movement.md;
  landing sound and view punch thresholds are not measured (assume
  movement.md's).
- Landing ducked "increases" fall damage per the wiki only because an
  air duck lifts the feet 20 units, so the landing comes later and
  faster; no separate rule.
- No landing slowdown, no stamina.

### Speeds while holding weapons or in conditions (reference)

These set P; they belong to the weapon and condition specs. Measured
values marked (m); the rest from the community wiki as a percentage of
300.

| Situation | P (units/s) |
|---|---|
| Heavy, Minigun winding, spun or firing | 110 (m) |
| Sniper, rifle zoomed | 80 (m) |
| Sniper, Huntsman drawn | 160 |
| Soldier, Cow Mangler 5000 charging | 80 |
| Demoman, shield charge | 750 (the wiki's 250 %; how it relates to the 520 ceiling: Open questions 8) |
| Engineer, carrying a building | 0.9 × 300 = 270 (wiki "90 %") |
| Medic, healing a faster teammate (Medi Gun) | the patient's speed |
| Spy, disguised | the disguise class's speed, at most 320 (a Spy disguised as a Scout moves at 320) |
| Humiliation (round lost / won) | 0.9 / 1.1 × P |

## Per-tick order

As movement.md, with these changes in the jump check (which still runs
before friction):

1. Grounded, fresh jump press, not fully ducked on the ground (and the
   shared conditions): first gravity half-step already applied; apply
   the anti-bunny-hop cap to v; add 289 to vz; extra half-step.
2. Not grounded, fresh jump press, class allows an air jump and it is
   unused, water level below waist: build the wish velocity from this
   tick's keys (the same rescale as the walking move), set v_xy to it
   and vz to 268.328; mark the air jump used. The air move of this tick
   then runs as usual (air acceleration toward the same wish adds
   nothing more, since the projection already equals the wish speed).
3. Ducking runs before movement as in movement.md with the 0.2 s times.

## Edge cases

- Ground detection, step-up 18 and slopes (normal z ≥ 0.7) are shared.
- After a standing jump the feet come within the 2-unit ground probe at
  the end of tick 47, 0.705 above the floor (move vz −275), and the
  player is grounded there without snapping (shared behaviour); the
  next jump starts from that height.
- Teammates do not block each other (`tf_avoidteammates_pushaway`
  pushes them apart); enemies collide with the full box.
- The air jump while ducked keeps the duck (box and lift).

## Quirks

- **Crouch jump**: jump, then duck in the air: +20 units of clearance
  (50 → 70), the standard way onto a 64-72 unit ledge. Keep.
- **No ground jump while ducked**: players must release duck (or be in
  transition, Open questions 3) to jump. Keep.
- **Scout air-jump redirect**: the air jump discards all horizontal
  momentum and points the Scout where the keys say; with no key held it
  stops him in mid-air horizontally. Keep.
- **Speed above the bunny-hop cap is clipped only on the jump tick**;
  air-strafing between jumps still gains speed (the wiki's "a small
  speed boost").
- **Rocket and sticky jumps** are explosion knockback (weapon specs);
  crouching in the air makes them stronger (knockback scales with the
  ducked box; weapon spec). The "ctap" (tapping duck just before the
  rocket hits) belongs there too.

## Test cases

dt 0.015, gravity 800; "stored" = end-of-tick velocity. (m) = measured
on the dedicated server; (d) = derived from movement.md with TF2's
numbers.

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | Scout at rest, flat ground (d) | hold forward | ticks 1–8 | speed 60, 114, 167.16, 217.1304, 264.1026, 308.2564, 349.761, 388.7754; tick 9: 400, and stays 400 |
| 2 | Heavy at rest (d) | hold forward | ticks 1–8 | 34.5, 63, 91.5, 120, 147.3, 172.962, 197.0843, 219.7592; tick 9: 230 |
| 3 | Scout fully ducked at rest (d) | hold forward | ticks 1–3 | 20, 34, 48 (wish 133.33); reaches 133.33 on tick 10 |
| 4 | Scout sliding at 400, ground, no input (m) | – | ticks 1–5 | 376, 353.44, 332.234, 312.3, 293.562; below 100 it loses 6 a tick; stops after 40 ticks |
| 5 | any class standing (m) | press jump | ticks 1–3 | stored vz 271, 259, 247; z +4.155, +8.13; apex 50.04 at the end of tick 24 |
| 6 | Scout, jump, duck pressed while rising (tick-start vz 187) (m) | – | that tick | z rises 20 + 2.715 (move vz 181), box 62, eye 45; apex 70.04 above the floor |
| 7 | Scout, jump, air jump pressed on tick 19 (stored vz 67) (m) | – | tick 19 / 20 / 21 | stored vz 262.328 / 250.328 / 238.328; the move of tick 19 rises 4.025 |
| 8 | Scout in the air moving (300, 0), air jump with no move key (m) | press jump | that tick | v_xy = (0, 0), vz stored 262.328 |
| 9 | Scout after case 7 | press jump again in the air | – | nothing (one air jump) |
| 10 | Heavy (m) | press jump in the air after a ground jump | – | nothing |
| 11 | Scout fully ducked on the ground (m) | hold jump | 60 ticks | stays on the ground |
| 12 | Scout, ground, v = (600, 0, 0) (m) | press jump | jump tick | stored v = (479.976, 0, 272.2) |
| 13 | Soldier, same (m) | press jump | jump tick | stored v = (287.986, 0, 274.12) |
| 14 | Scout at 450 on the ground (m) | press jump | jump tick | horizontal 450 kept (below 480); vz stored 271 |
| 15 | Scout standing, duck pressed (m) | hold duck | ticks 1–14 | eye 64.6794, 63.785, 62.4181, 60.68, 58.6719, 56.495, 54.2506, 52.04, 49.9644, 48.125, 46.6231, 45.56, 45.0369, then 45 with the 62 box on tick 14 |
| 16 | Scout fully ducked, duck released, room above (m) | – | ticks 1–14 | eye 45.3206, 46.215, 47.5819, …, 64.9631, then 65 with the 82 box on tick 14 |
| 17 | Medic standing, duck pressed (d) | hold duck | ticks 1 / 14 | eye 75 − 30·S(0.075) = 74.519; 45 |
| 18 | Heavy (300) lands at F 648, spread off (m) | – | landing | no damage |
| 19 | Heavy lands at F 652 / 692 / 952, spread off (m) | – | landing | 33 / 35 / 48 damage |
| 20 | Soldier (200) lands at F 1384 / 3424, spread off (m) | – | landing | 46 / 114 |
| 21 | Heavy lands at F 1384, spread on (m) | – | many landings | damage always within 55–83, mean near 69 |
| 22 | any class | every speed query | – | P per the class table; Scout 400 not limited by `sv_maxspeed` 320 |
| 23 | Heavy, attack2 held (m) | – | first tick | P 110; released and wound down: 230 |
| 24 | Sniper, rifle, attack2 pressed (m) | – | first tick | P 80; pressed again (unzoom): 300 |

## Open questions

1. **`sv_maxspeed` 320 vs class speeds.** Scout 400, Medic and Spy 320
   and the 520 ceiling show the shared min(`sv_maxspeed`, P) is not
   applied to the class speed. Whether `sv_maxspeed` still clamps
   anything (e.g. if set below P) is unmeasured. Check: set
   `sv_maxspeed` 200 on a listen server and run as a Scout.
2. **Backward rule.** What counts as moving backwards (forward move < 0;
   or the wish direction more than 90° from the view; how diagonals
   blend), and whether `sv_backspeed` 0.6 plays any part (it is
   replicated and archived but TF2's documented back speed is 0.9).
   Check with `cl_showpos` on a listen server: hold back, then
   back+strafe, as a Pyro (expect 270 back).
3. **Jump during a duck transition.** Allowed during the 0.2 s duck-down
   and during the unduck? (The ctap technique suggests a jump during the
   unduck works.) Check: tap duck then jump within 0.2 s and watch vz.
4. **Bunny-hop cap with a reduced P.** Which M (base class speed or
   current) a spun Heavy or zoomed Sniper is capped to.
5. **Held jump.** Puppet bots with jump forced every tick jumped again on
   their first grounded tick; for real players the shared "fresh press"
   rule is assumed (TF2 has no auto-hop). Check on a listen server
   holding the key.
6. **Air jump with keys held.** Horizontal = full wish velocity of
   length M (400 for a Scout)? Or a fraction? Check: air jump holding
   strafe, read `cl_showpos`.
7. **Fall damage rounding and spread order.** Without the spread,
   round-to-nearest is measured. With it, rounding after multiplying by
   r is assumed; truncation after the spread would also fit the 24
   samples. Also unmeasured: whether H_max is the current max health
   (items that change it) or the class base, and whether the spread is
   uniform (the samples are spread over the whole range, consistent with
   uniform).
8. **Shield charge speed and the 520 ceiling.** The wiki lists the charge
   at 250 % (750); whether the ceiling applies to it is for the Demoman
   items spec.
9. **Air duck limit** (`tf_clamp_airducks`): "twice per air time" is the
   wiki's; what happens to a third press (ignored, or the duck held but
   no lift) is unmeasured.
10. **Landing sound and view punch thresholds** and step-sound timing:
    not measured; assumed movement.md's.
