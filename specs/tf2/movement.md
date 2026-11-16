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
move keys). The class scripts are encrypted and were not opened. For
the 2026-11-15 check, Valve's Source SDK 2013 with TF2's client and
server game code (the game's player movement, the shared player code,
the game rules and the weapons) was read in place, on its own machine,
by a spec session; nothing of it was copied, only the behaviour and
numbers are restated here. The class top speeds themselves are not in
that code (they come from the encrypted class scripts), so they stay the
measured values.
Status: checked against the SDK's game code (2026-11-15)

## Summary

TF2 runs the shared Source controller of specs/cs_source/movement.md
("movement.md") with gravity 800, accelerate 10, airaccelerate 10,
friction 4, stopspeed 100 and a 66.67 Hz tick, and adds a class layer:
each class has its own top speed (230 to 400), a hard ground-speed clamp
to that top speed, a 0.9 cap on the backward part of the ground
velocity, a single 48 × 48 × 82 box (62 ducked) for everyone, class eye
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

## Corrections from the SDK (2026-11-15)

Each line: what the spec said before -> what the SDK's game code shows.

- Backwards rule: the forward move negative, 0.9 M as the input rescale target and wish clamp (also in the air) -> on the ground only, after acceleration: when |v| > 100 and v points behind the view, only the component of v along the flat view-forward is capped at 0.9 M; the sideways component is kept; nothing in the air or in water.
- Ground speed: no clamp (shared) -> on every grounded walking tick the horizontal speed is clamped to M after acceleration (a Scout landing at 600 is at 400 after his first ground tick).
- Low wish speeds: shared acceleration -> a wish speed below 40 gets a raised acceleration so it beats friction (a ducked spun Heavy or ducked zoomed Sniper still moves).
- Ground-leave speed: 140 (shared, implied by "as movement.md") -> 250: upward speed above 250 leaves the ground; at or below it the ground probe runs, and its 0.25 surface friction while rising applies for 0 < vz ≤ 250.
- Air strafing: 30 units/s per tick at any class speed -> at most min(30, 0.15 × wish × surface friction): 0.0375 × wish while rising below 250 (Scout 15, Heavy 8.625), and below 30 at any time for wish < 200 (spun Heavy 16.5, zoomed Sniper 12).
- Anti-bunny-hop cap: 1.2 × M -> 1.2 × P, the player's current max speed (not reduced by moving backwards or by the surface; a spun Heavy 132, a zoomed Sniper 96). Open question 4 closed.
- Jump during the duck-down transition: unknown -> allowed, and vz is set to 289 instead of added (apex 52.20, before any duck lift); refused during the unduck transition. Open question 3 closed.
- Jump impulse: 289 -> 289 × the surface's jump factor (1 on ordinary materials).
- Held jump: puppet bots re-jumped -> a real player needs a fresh press (the bots' forced buttons were an artefact). Open question 5 closed.
- Air jump horizontal speed: 360 backwards -> M in every direction (400 for a Scout): the keys after the shared rescale to M, with no backward cap and no duck crop. Open question 6 closed.
- Air duck limit: "twice per air time" (public) -> counted on duck releases in the air; a duck press in the air is refused once two have been counted; the Scout's air jump resets the count to 0. Open question 9 closed.
- Re-ducking: as movement.md -> on the ground a duck is refused for 0.3 s after duck was released, and a duck press during an unduck transition is ignored (the unduck goes on).
- Ducking in water: as movement.md -> no duck while the feet are in water off the ground, or while the eyes are under water.
- Speed ceiling: no effect above 520 -> 520 caps the movement, but the Demoman's shield charge sets 750 past it. Open question 8 closed.
- `sv_maxspeed`: unknown whether it clamps anything -> TF2 replaces it every command; it never affects a player. Open question 1 closed.
- `sv_backspeed`: open -> unused by any movement code. Open question 2 closed.
- `tf_clamp_back_speed`, `tf_scout_air_dash_count`, `tf_parachute_*`: absent -> they exist as development-only variables (0.9 with a 100 units/s minimum, 1, parachute tuning), hidden on release servers.
- Fall damage rounding with the spread: assumed -> confirmed: the spread multiplies first, then the damage is rounded half up; H_max is the current max health; no cap. Open question 7 closed.
- Landing in water: deeper than 1 unit -> any water at the feet (water level at least 1) cancels fall damage.
- Landing sound and view punch: assumed movement.md's -> TF2 uses the multiplayer thresholds 350 / 580 / 290 (Falling and landing); a Scout gets no landing sound or punch below 580. Open question 10 closed.
- Spun Minigun 110: "winding, spun or firing" -> confirmed, and it ends when the wind-down starts (about 0.1 s after release once spun), not when the wind-down ends.
- Spy disguised: the disguise's speed, at most 320 -> the lower of his own and the disguise class's speed, ignored while cloaked.
- Medic with the Medi Gun: the patient's speed -> the higher of his own and the patient's (the patient's computed without a shield charge), while the Medi Gun is the active weapon and has a patient.
- TF2's step-up: shared -> the stepped path is kept whenever it succeeds (Edge cases).

Confirmed unchanged by the SDK: gravity 800, accelerate/airaccelerate 10,
friction 4, stopspeed 100, air speed cap 30, step 18, the boxes (48 × 48
× 82 / 62), duck eye 45, class eye heights 65/68/75, duck and unduck
0.2 s, air lift 20, jump 289 added, no ground jump while fully ducked,
the air jump's 268.3281573 replacing vz, one air jump per air time,
swim 0.8, fall damage H_max·F/6000 above 650 with a uniform ×0.8–1.2
spread and `tf_fall_damage_disablespread`, the spun Minigun's 110, the
zoomed rifle's 80, the Huntsman's 160, the 0.9 / 1.1 humiliation factors
and the Engineer's 0.9 while carrying.

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
| back_frac | 0.9 | – | cap on the backward component of ground velocity, as a fraction of M |
| back_min | 100 | units/s | the backward cap applies only above this ground speed |
| low_wish | 40 | units/s | below this wish speed ground acceleration is raised (100 × friction / accelerate) |
| leave_ground_vz | 250 | units/s | upward speed above which the ground is left (replaces the shared 140) |
| air_speed_cap | 30 | units/s | shared air wish cap, unchanged |
| duck_frac | 1/3 | – | shared ducked input scale (movement.md) |
| swim_frac | 0.8 | – | shared water wish scale (movement.md) |
| speed_ceiling | 520 | units/s | movement cap above every class speed (the shield charge's 750 bypasses it) |
| reduck_delay | 0.3 | s | no new ground duck this long after duck is released |
| hull_stand | (−24,−24,0)–(24,24,82) | units | every class |
| hull_duck | (−24,−24,0)–(24,24,62) | units | every class |
| eye_duck | 45 | units | every class |
| air_duck_lift | 20 | units | 82 − 62 |
| duck_time / unduck_time | 0.2 / 0.2 | s | eye blend length both ways |
| jump_impulse | 289 | units/s | ground jump, every class, added to vz (set instead during a duck-down transition); times the surface's jump factor |
| air_jump_vz | 268.3281573 | units/s | Scout's air jump, replaces vz (= √(2·800·45)) |
| bhop_cap_frac | 1.2 | – | ground jump caps speed at 1.2 × P (current max speed) |
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
| `sv_maxspeed` | 320 | never used for TF2 players: the movement cap is replaced by 520 on every command (SDK) |
| `sv_stepsize` | 18 | |
| `sv_bounce` | 0 | |
| `sv_wateraccelerate` | 10 | |
| `sv_waterfriction` | 1 | |
| `sv_maxvelocity` | 3500 | per axis, as movement.md |
| `sv_backspeed` | 0.6 | archived, replicated; the listing shows "0" (it rounds). Registered but read by no movement code (SDK); TF2's back cap is the development-only `tf_clamp_back_speed` 0.9 |
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
`sv_autobunnyhopping`, `sv_timebetweenducks`. Unknown commands on a
release server but present in the game code as development-only
variables (so their defaults are fixed in practice): `tf_clamp_back_speed`
0.9 and its minimum speed 100, `tf_scout_air_dash_count` 1, the
`tf_parachute_*` tuning, and the overheal `tf_max_health_boost` 1.5 and
`tf_boost_drain_time` 15 (classes.md). The air-current and
lost-footing states belong to airblast and later specs; with neither
active, movement is as below.

### Class top speed

The player's max speed P comes from the class, then from conditions and
weapons. Measured P (the max-speed value of puppet bots of each class,
standing, holding their stock primary):

| Class | P | Straight back on the ground (0.9 P) | Ducked (P/3) | Swimming (0.8 P) |
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

The movement cap is M = min(P, 520) times the ground material's speed
factor (SDK: each command starts from 520 in place of `sv_maxspeed`,
takes the lower of that and P, then the surface factor); then
movement.md's input rescale and wish clamp. With 450 inputs the rescale
always binds, so holding forward asks for exactly M. (If P is above 450,
a key held at full ±450 is first raised to ±P on that axis so the
rescale can reach M; no stock class needs this.)

**How P is computed** (SDK, in this order; each step uses the result
of the one before):

1. Base: the class speed (table above).
2. Disguised Spy, not cloaked: the lower of that and the disguise
   class's base speed.
3. Aiming (a Minigun winding, spun or firing; a Sniper Rifle zoomed; a
   Huntsman drawn; a Cow Mangler charging): the lower of that and the
   aim limit: 110 for a Heavy, 160 with the Huntsman, 80 otherwise.
4. Cloaked: no limit (the cloak cap is 999).
5. Humiliation: winners × 1.1, losers × 0.9.
6. The active weapon's own speed factor (1 for stock weapons).
7. Demoman charging with a shield: P = 750 (replaces everything above).
8. Engineer carrying a building: × 0.9.
9. Item attributes (none on stock items).
10. Medic with the Medi Gun active and a patient: the higher of P and
    the patient's own P (computed the same way but ignoring a shield
    charge).
11. Later item and game-mode effects (out of scope).

P is recomputed when any of these changes (weapon state, condition) and
used from the next movement command. Nothing in it depends on being on
the ground, so a spun Heavy has P = 110 in the air too; but the air
keeps velocity (only the air wish is limited, below), so the speed he
had stays until he lands and the ground clamp takes it.

**Ground speed clamp.** On every grounded walking tick, after friction
and acceleration, a horizontal speed above M is scaled down to M (SDK;
the shared controller has no such clamp). So a Scout landing at 600
walks on at 400 from his first ground tick; a Heavy landing at 300 who
spun up in the air is at 110.

**Low wish speeds.** If 0 < wish < 40 (= 100 × friction × surface
friction / accelerate) on the ground, the acceleration value for that
tick is raised to c·f/wish + 1, with f = friction × surface friction
and c = max(current speed after friction, 100), so one tick adds
(c·f + wish)·dt, a little more than friction removes. Otherwise the
shared acceleration of 10. Only very slow wishes reach this (a ducked
spun Heavy 36.67, a ducked zoomed Sniper 26.67).

**Backwards.** On the ground only, after the ground speed clamp: if the
speed |v| is above 100 and v points behind the view (v · flat forward
< 0), v is split into its part along the flat view-forward (the
backward part) and its part along the flat view-right. If the backward
part is longer than 0.9 M it is scaled to 0.9 M; the sideways part is
kept; v is reassembled, and clamped to M once more. So straight back
gives 0.9 M (the table's column), a 45° back-diagonal gives the full M
(its backward part is only 0.707 M), and nothing changes in the air or
in water (a Scout's air jump backwards is 400; swimming backwards is
0.8 P). The input is not touched. (Development-only variables:
`tf_clamp_back_speed` 0.9, `tf_clamp_back_speed_min` 100.)

**Speed ceiling.** M never exceeds 520 (468 straight back on the
ground, 173.33 ducked, 416 swimming) (public and SDK; the ceiling was
raised from 450 in 2012). P itself may be higher (the bunny-hop cap
uses P). The Demoman's shield charge sets both its P and the movement
cap to 750, bypassing the ceiling.

### Ground and air acceleration, friction

As movement.md with the numbers above: friction removes 6 % a tick above
100 units/s and 6 units/s a tick below; ground acceleration adds up to
10 × wish × 0.015 = 0.15 × wish a tick (60 for a Scout, 34.5 for a
Heavy); air acceleration adds up to 0.15 × wish but only until the
projection reaches 30. Measured: a Scout set sliding at 400 with no
input on flat ground has 376.0, 353.44, 332.234, 312.3, 293.562 … then
from 102.535 below 100 it loses 6 a tick (96.383, 90.383, …, 6.383) and
stops on the 41st tick.

Air acceleration is movement.md's: per tick it adds at most
min(30 − v·wishdir, 10 × wish × 0.015 × surface friction). With surface
friction 1 and wish ≥ 200 (every class's stock top speed) the 30 binds,
so perpendicular strafing adds 30 a tick. Two TF2 differences make it
smaller (SDK):

- **The 0.25 band is 0 < vz ≤ 250.** TF2's ground detection leaves the
  ground without probing only when vz > 250 (the shared code: 140).
  Below that, airborne and rising, the probe finds nothing and sets
  surface friction 0.25 for the next tick's air move. On a standing
  jump the move vz is 277, 265, 253 on ticks 1–3 and 241 on tick 4, so
  air acceleration is quartered on ticks 5–25 (until the move vz is no
  longer positive): 0.0375 × wish a tick, 15 for a Scout, 8.625 for a
  Heavy. After the Scout's air jump (move vz 268.33, 256.33, then
  244.33) it is quartered from the third tick after it.
- **Reduced P**: a spun Heavy's wish is 110 (16.5 a tick, 4.125 in the
  band), a zoomed Sniper's 80 (12, 3).

### Jumping

As movement.md's Jumping, with these differences:

- **Impulse 289** times the surface's jump factor (1 on ordinary
  materials), every class, not recomputed from gravity: added to vz,
  then the extra gravity half-step. Standing on flat ground the jump
  tick's move uses 289 − 12 = 277 and stores 271; tick k's move uses
  277 − 12(k − 1). Apex **50.04** at the end of tick 24 (SDK agrees).
- **No jump while fully ducked on the ground**, and none during an
  unduck transition (the player is still flagged ducked). Measured: a
  bot held fully ducked with jump forced never left the ground; the SDK
  agrees. A "crouch jump" is a jump followed by a duck in the air.
- **Jump during the duck-down transition** (duck pressed, not yet
  complete; SDK): allowed, and vz is **set** to 289 instead of added
  (the shared ducked-jump rule), so the −6 of the first half-step is
  lost: the move uses 283, stores 277, apex **52.20** at the end of
  tick 24. If duck is still held, the next tick (now in the air)
  finishes the duck at once with the 20-unit lift.
- **Not while** dead, taunting, or drawing the Huntsman (a drawn bow
  forbids jumping), and not with water at waist depth or deeper
  (that is swimming: jump sets vz 100 in water, 80 in slime).
- **Anti-bunny-hop cap.** On the jump tick, after the first gravity
  half-step and before the impulse: if the speed |v| (all three
  components) exceeds 1.2 × P (P the player's current max speed, not M:
  no backward, surface or ceiling reduction), v is scaled down to
  length 1.2 × P. Then
  the impulse and the extra half-step apply as usual. Measured with v set
  to (600, 0, 0) the tick before the jump:

  | Class (P) | Cap | Stored after the jump tick |
  |---|---|---|
  | Scout (400) | 480 | vx 479.976, vz 272.2 |
  | Soldier (240) | 288 | vx 287.986, vz 274.12 |

  The vz values show the scaling includes the −6 already in vz:
  (600, 0, −6) has length 600.03; scaled to 480 that is (479.976, 0,
  −4.8), then +289 − 6 − 6 = 272.2. Below the cap nothing changes
  (Scout at 450 keeps 450; Soldier at 280 keeps 280). So a ground jump
  never leaves faster than 1.2 × P horizontally (480 Scout, 288 Soldier,
  276 Heavy, 336 Pyro/Engineer/Sniper, 384 Medic/Spy, 336 Demoman), and
  each hop of a chain starts from at most that, but speed gained in the
  air by strafing is kept until the next jump. While a weapon or
  condition lowers P the cap follows it (SDK): a spun Heavy jumps out
  at most 132, a zoomed Sniper 96. A Scout holding back still gets 480
  (P, not the backward 0.9).
- **Friction** is skipped on the jump tick (shared).
- The jump fires on a fresh press only (SDK: the key must have been up
  on the previous command); holding jump does not re-jump on landing.
  The puppet bots that re-jumped had their buttons forced in a way a
  client cannot.

### The Scout's air jump

Only the Scout (stock loadout; items that add jumps are later specs).

- **When:** jump pressed fresh (not held from the previous tick) while
  not on the ground, not in water at waist depth or deeper, and the air
  jump not yet used since leaving the ground. One per air time; landing
  resets it. A third press does nothing (measured; SDK: one air jump,
  `tf_scout_air_dash_count` 1, development-only). It can be used at
  any point in the air, including after walking off a ledge without a
  ground jump (SDK: only the count matters), and while ducked in the air
  (the fully-ducked jump ban is for the ground). Not while taunting.
- **Vertical:** vz is **replaced** by 268.3281573 (after the tick's first
  gravity half-step), with no extra half-step; the move uses 268.328 and
  the tick stores 262.328. From any fall speed the air jump therefore
  gives the same rise: the moves use 268.328 − 12(k − 1), so the apex is
  **47.03** above the height at the start of the air-jump tick (end of
  its 23rd tick). Measured: air jump on tick 19 of a standing jump
  (stored vz 67 before it): stored 262.328, then 250.328, 238.328, …,
  apex 47.03 higher than the start of that tick (Test case 7).
- **Horizontal:** the horizontal velocity is **replaced** by the wish
  velocity of this tick's keys: the flattened, normalized view forward
  × f plus the flattened, normalized view right × s, with f and s the
  moves after movement.md's rescale to M (SDK). Its length is therefore
  M whenever a key is held at the full 450: 400 for a Scout forwards,
  backwards, sideways or diagonally (no backward cap, which is a ground
  rule, and no duck crop, which needs ground). With no key held it
  becomes 0: measured, a Scout moving at 300 in the air with no keys
  stops dead horizontally on the air jump. This is the redirect players
  use to dodge.
- The anti-bunny-hop cap does not apply (SDK: the air jump returns
  before it).
- The air jump resets the air-duck count to 0 (Ducking).
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
- **Air duck limit** (`tf_clamp_airducks` 1; SDK): each release of duck
  while in the air adds 1 to an air-duck count; while the count is 2 or
  more, the duck key is treated as not held in the air (so the player
  stays standing or starts unducking). So two air ducks per air time;
  the third press does nothing for as long as it is held. The count
  resets to 0 on landing and on the Scout's air jump.
- **Re-duck delay** (`tf_clamp_airducks` 1; SDK): releasing duck starts
  a 0.3 s timer; until it runs out the duck key is treated as not held
  while on the ground. And while the player is in an unduck transition
  (still flagged ducked), the duck key is treated as not held anywhere,
  so pressing duck again during the 0.2 s rise does not freeze or
  restart it (unlike movement.md's re-press rule): the unduck finishes.
- **No duck in water** (SDK): the duck key is treated as not held while
  the feet are in water and the player is off the ground, or while the
  eyes are under water.
- Ducked ground speed is the shared 1/3 input scale, applied only on
  the ground: wish = M/3 (133.33 Scout, 76.67 Heavy). The ground clamp
  stays M, not M/3, so a ducked player sliding faster slows only by
  friction.

### Swimming and water

As movement.md: swimming wish = 0.8 × the (rescaled) input, so the class
swim speeds are 0.8 P (table above); `sv_wateraccelerate` 10,
`sv_waterfriction` 1. Landing with any water at the feet (water level
1 or more) cancels fall damage and the landing effects (SDK; as
movement.md's "in water, nothing"). The backward cap and the ground
clamp do not apply while swimming.

### Falling and landing

- **Fall speed** F = −vz at the start of the landing tick (before its
  first gravity half-step), as movement.md records it.
- **Damage** when F > 650, applied on landing:

  ```math
  D = \operatorname{round}\!\left(H_{max} \cdot \frac{F}{6000} \cdot r\right), \qquad r \sim U[0.8,\ 1.2]
  ```

  with H_max the player's current max health (including anything that
  changes it) and r = 1 when `tf_fall_damage_disablespread` is 1. The
  SDK confirms the order: the base 5 × F/300 (the "old TFC formula")
  times H_max/100, then times r (uniform), and the health loss is the
  result rounded half up (⌊x + 0.5⌋). There is no cap: a long enough
  fall kills. No class difference other than H_max. Only strictly
  above 650. Measured on a Heavy (300) with
  the spread off: F 648 → 0, 652 → 33, 664 → 33, 672 → 34, 684 → 34
  (34.2), 692 → 35 (34.6), 736 → 37 (36.8), 952 → 48 (47.6); on a
  Soldier (200): 796 → 27 (26.53), 1384 → 46 (46.13), 3424 → 114. Every
  sample fits round-to-nearest of H_max·F/6000 and none fits F + 6. With
  the spread on, 24 drops at F 1384 (base 69.2) gave 55–81 (0.795 to
  1.17 of the base). Rounding after the spread: SDK-confirmed.
- So the smallest damaging fall costs about 10.85 % of max health (the
  wiki's table: "5 × F/300 per cent" is the same F/6000 of max health),
  reached after about 264 units of free fall from rest.
- Fall damage ignores the shared 580/1024 damage numbers of movement.md.
- **Landing effects** (SDK; the shared multiplayer thresholds): only
  when F ≥ 350 and not in water: volume 1 if F > 580, else 0.85 (above
  290, always the case here); a landing step sound at that volume and a
  view roll punch of 0.013 × F degrees. A **Scout** gets neither unless
  the volume is 1 (F > 580). (The fall damage check itself runs only
  above 580 and then only damages above 650.)
- Landing ducked "increases" fall damage per the wiki only because an
  air duck lifts the feet 20 units, so the landing comes later and
  faster; no separate rule.
- No landing slowdown, no stamina.

### Speeds while holding weapons or in conditions (reference)

These set P (the steps under "How P is computed"); they belong to the
weapon and condition specs. Measured values marked (m); all are
SDK-confirmed.

| Situation | P (units/s) |
|---|---|
| Heavy, Minigun winding, spun or firing | min(P, 110) (m). From the moment the wind-up starts until the wind-down starts: about 0.1 s after the keys are released once spun, or at the end of the 0.75 s wind-up if released during it. The wind-down itself runs at full speed. In the air too (only the air wish is limited) |
| Sniper, rifle zoomed | min(P, 80) (m), from the zoom-in to the zoom-out; also in the air |
| Sniper, Huntsman drawn | min(P, 160); and no jumping while drawn |
| Soldier, Cow Mangler 5000 charging | min(P, 80) |
| Demoman, shield charge | 750 (the wiki's 250 %), past the 520 ceiling; forward only |
| Engineer, carrying a building | × 0.9 = 270 |
| Medic, Medi Gun active with a patient | max(own P, the patient's P without a shield charge) |
| Spy, disguised, not cloaked | min(own P, the disguise class's base speed): 320 as a Scout, 230 as a Heavy |
| Spy, cloaked | no cap |
| Humiliation (round lost / won) | 0.9 / 1.1 × P |

## Per-tick order

As movement.md, with these changes in the jump check (which still runs
before friction):

0. At the start of the command the movement cap is set to 520, then
   min(P, 520) × surface factor, and the keys are rescaled to it
   (shared rescale).
1. Ducking runs before movement as in movement.md with the 0.2 s times,
   after TF2's duck-key filters (water, re-duck delay, unduck
   transition, air-duck count); the 1/3 crop applies only when ducked
   on the ground.
2. First gravity half-step (shared).
3. Jump check. Grounded, fresh jump press, not flagged ducked on the
   ground (fully ducked or unducking), allowed to jump: apply the
   anti-bunny-hop cap (1.2 × P) to v; add 289 × jump factor to vz (set
   it instead if in a duck-down transition); extra half-step.
   Not grounded, fresh jump press, a Scout with his air jump unused,
   water level below waist: set v_xy to this tick's wish velocity and
   vz to 268.328 (no extra half-step); mark the air jump used and reset
   the air-duck count. The air move of this tick then runs as usual
   (air acceleration toward the same wish adds nothing more, since the
   projection already equals the wish speed).
4. Grounded: friction, then the walk move: acceleration (with the low
   wish boost), the clamp to M, the backward cap, then the position
   move. Not grounded: the air move (wish clamped to M, air speed cap
   30).
5. Ground detection (leave the ground if vz > 250; otherwise probe, and
   surface friction 0.25 if airborne and rising), second gravity
   half-step, landing (fall damage, landing effects).

## Edge cases

- Slopes (normal z ≥ 0.7) and the 18-unit step are shared. Ground
  detection is shared except that only vz > 250 skips the probe (not
  140), and a grounded walker probes 18 units further down to stay on
  stairs and slopes going down.
- **Step-up** (SDK, differs from the shared choice): when the straight
  ground move hits something, TF2 first tries the stepped path (up 18,
  move, down 18). It keeps that path unless it went nowhere or ended on
  ground steeper than 0.7; only then does it redo the move as a plain
  slide. The shared code's comparison of the two paths' distances is
  not made.
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
- **No ground jump while ducked**: players must be fully standing or
  still in the duck-down transition to jump; a jump in the first 0.2 s
  of a duck goes slightly higher (52.20) because it sets vz. Keep.
- **Landing kills speed**: the ground clamp to M on the first ground
  tick removes any speed above M from a rocket jump or air-strafe at
  once (unless the player jumps again on that tick). Keep.
- **Slow upper jump**: air control is quartered while rising below 250
  units/s, most of every jump. Keep.
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
numbers; (s) = derived from the SDK reading (2026-11-15). Every (m) and
(d) case below was rechecked against the SDK's rules and stands.

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
| 23 | Heavy, attack2 held (m) | – | first tick | P 110; released once spun: 230 again when the wind-down starts (about 0.1 s after release, SDK), not after it |
| 24 | Sniper, rifle, attack2 pressed (m) | – | first tick | P 80; pressed again (unzoom): 300 |
| 25 | Pyro at rest on the ground (s) | hold back only | ticks 1–8 | speed 45, 84, 123, 160.62, 195.9828, 229.2238, 260.4704, then 270 from tick 8 on |
| 26 | Pyro at rest (s) | hold back + strafe right | many ticks | 300 (the backward part, 212.13, is under 270) |
| 27 | Scout on the ground, v = (600, 0, 0) (s) | no input, no jump | tick 1 | speed 400 (friction to 564, then the clamp to M) |
| 28 | Scout in the air, air jump unused (s) | press jump holding back only / forward + right | that tick | horizontal speed 400 pointing back / 400 at 45° right of the view |
| 29 | Scout standing, duck pressed on tick 1 (s) | jump pressed on tick 2, duck held | ticks 2 / 3 | tick 2 stored vz 277 (vz set to 289, not added); tick 3 the duck completes in the air (+20 lift); apex 52.20 + 20 = 72.20 above the floor |
| 30 | Scout on the ground, 5 ticks into an unduck (s) | press jump | that tick | no jump |
| 31 | Heavy, Minigun spun (P 110), ground, v = (600, 0, 0) (s) | press jump | jump tick | stored v = (131.9934, 0, 275.68) (cap 1.2 × 110 = 132) |
| 32 | Sniper zoomed (P 80), fully ducked at rest (s) | hold forward | ticks 1–3 | 6.4, 6.8, 7.2 (wish 26.67 < 40: raised acceleration; +0.4 a tick net) |
| 33 | Scout jumps from rest, holds right only on tick 4 / only on tick 5 (s) | – | that tick | horizontal speed 30 / 15 (surface friction 0.25 from the end of tick 4) |
| 34 | Scout in the air (s) | duck, release, duck, release, duck | – | first two ducks lift 20; the third press does nothing |
| 35 | Scout air jumps after two air ducks (s) | duck | – | ducks again (count reset by the air jump) |
| 36 | any class on the ground (s) | release duck, press it again 0.15 s later and hold | – | the duck starts 0.3 s after the release (20 ticks), not before |
| 37 | Soldier lands at F 1384 in water at the feet (s) | – | landing | no damage |
| 38 | Scout lands at F 500 (s) | – | landing | no landing sound, no view punch (a Soldier: sound at 0.85, roll punch 6.5°) |

## Open questions

Closed by the SDK check (2026-11-15), kept by number so references
stay valid:

1. `sv_maxspeed`: never applies to TF2 players (Class top speed).
2. Backward rule: velocity-based, ground only (Class top speed,
   "Backwards"); `sv_backspeed` is unused.
3. Jump during a duck transition: allowed during the duck-down (vz set),
   refused during the unduck (Jumping).
4. Bunny-hop cap with a reduced P: the current P (Jumping).
5. Held jump: a fresh press is required (Jumping).
6. Air jump with keys held: the full rescaled wish, length M (The
   Scout's air jump).
7. Fall damage: spread first, then rounded half up; current max health
   (Falling and landing).
8. Shield charge: 750, past the 520 ceiling (Class top speed).
9. Air duck limit: counted on releases; a refused press is ignored
   while held; reset by the air jump (Ducking).
10. Landing sound and view punch thresholds (Falling and landing).
    Step-sound timing while walking is still assumed movement.md's.

Still open:

11. The class top speeds and max health come from the encrypted class
    scripts, which the SDK does not contain; the measured values stand.
12. Surface jump and speed factors for TF2's materials (which materials
    differ from 1) are data in the game's surface properties, not
    checked.
