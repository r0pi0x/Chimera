# Counter-Strike 1.6: movement

Source basis: the community reimplementation of the CS 1.6 game library
(ReGameDLL_CS, open source; read in a separate spec session as reference
only, its original-game behaviour, never its optional fixes or
additions), the HL SDK's shared movement for Half-Life's base, then
checked against the game: the console variable defaults read from a
local HLDS (build 10211, `mod cstrike`) started without a server config,
the client's shipped `config.cfg`, and the player models' hull data in
the install. Numbers marked "(ref.)" come from the reference and were not
yet measured in the running game (see Open questions). No code is
reproduced here.
Status: draft

## Summary

- Quake-lineage kinematic movement, one step per client command whose
  duration is the command's own whole number of milliseconds. Half of the
  gravity is applied before the move and half after; ground friction
  runs before acceleration; acceleration only adds speed along the wish
  direction until the velocity's projection on it reaches the wish speed.
- Collision is a trace of the player's box through the map's
  precompiled clip hulls (specs/goldsrc/bsp.md, "Hulls and collision"),
  with the player's origin at the **centre** of the box.
- What makes CS 1.6 feel like itself: a low ground acceleration (5), a
  harsh speed loss after every jump (a timer that scales the horizontal
  speed on each ground command, so it depends on the frame rate), a
  bunny-hop clamp at 1.2 × the maximum speed, weapon-dependent maximum
  speeds, edge friction near drops, and an 0.4 s duck whose speed
  penalty starts the moment the key goes down.

## Units and conventions

- Distance: units (1 unit = 1 inch, Chimera's 0.0254 m). Z is up.
  Velocities in units/s.
- Angles in degrees: pitch (positive looks down), yaw, roll. Forward
  from angles is (cos p·cos y, cos p·sin y, −sin p); right with zero roll
  is (sin y, −cos y, 0).
- **Origin at the box centre.** Standing box: (−16, −16, −36) to
  (16, 16, 36) around the origin; ducked: (−16, −16, −18) to (16, 16, 18).
  Feet are origin − 36 standing, origin − 18 ducked.
- Time: one movement step per user command. The step length is
  `msec / 1000` s, where `msec` is a whole number of milliseconds carried
  in the command (1..255). The game's timers that count down in
  milliseconds (duck, jump penalty, step sounds, swim sounds, water
  jump) subtract `msec` each command.
- Inputs: forward, side and up amounts in units/s, ±400 per key by
  default (`cl_forwardspeed`, `cl_sidespeed`, `cl_backspeed` are 400 in
  the shipped client config; `cl_upspeed` 320 (ref.)). The walk key
  scales them by `cl_movespeedkey` (0.52 (ref.)).
- Entity friction and entity gravity are per-player multipliers, both 1
  for a normal player.

## Constants

### Console variables (server defaults, read from HLDS build 10211)

| Name (ours) | Cvar | Value | Unit | Meaning |
|---|---|---|---|---|
| accelerate | `sv_accelerate` | 5 | 1/s | ground (and water) acceleration factor |
| air_accelerate | `sv_airaccelerate` | 10 | 1/s | air acceleration factor |
| friction | `sv_friction` | 4 | 1/s | ground friction factor |
| stop_speed | `sv_stopspeed` | 75 | units/s | below this, friction acts as if the speed were this |
| edge_friction | `edgefriction` | 2 | × | friction multiplier near a drop |
| max_speed_server | `sv_maxspeed` | 900 engine default; 320 in the shipped `server.cfg` | units/s | cap on the weapon's speed (never reached by a weapon) |
| step_size | `sv_stepsize` | 18 | units | highest step walked up |
| gravity | `sv_gravity` | 800 | units/s² | |
| bounce | `sv_bounce` | 1 | | wall overbounce term (no effect while entity friction is 1) |
| max_velocity | `sv_maxvelocity` | 2000 | units/s | each velocity component is clamped to ± this |
| water_accelerate | `sv_wateraccelerate` | 10 | | exists but CS's water movement uses `sv_accelerate` (ref.) |
| water_friction | `sv_waterfriction` | 1 | | exists, unused by the movement (ref.) |
| roll_angle, roll_speed | `sv_rollangle`, `sv_rollspeed` | 0, 0 | | no view roll from strafing |
| server frame rate | `sys_ticrate` | 100 | Hz | the dedicated server's frame rate |
| client frame cap | `fps_max` | 100 | Hz | shipped client config: 10 ms commands |

### Fixed numbers (ref.)

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| hull_stand | ±16, ±16, ±36 | units | standing box about the origin |
| hull_duck | ±16, ±16, ±18 | units | ducked box about the origin |
| eye_stand | +17 | units | eye above the origin standing (feet + 53) |
| eye_duck | +12 | units | eye above the ducked origin (feet + 30) |
| eye_dead | −8 | units | eye when dead |
| duck_shift | 18 | units | origin moves down this much when ducking on the ground |
| duck_time | 0.4 | s | duck transition on the ground |
| duck_timer_start | 1000 | ms | duck timer set when the duck key is pressed |
| duck_speed_factor | 0.333 | × | move inputs while the duck key is held, ducking or ducked |
| jump_speed | √(2 · 800 · 45) = 268.3281573 | units/s | jump take-off speed (fixed, not from `sv_gravity`) |
| jump_penalty_start | 1315.789429 | ms | jump penalty timer set on each jump |
| jump_penalty_slope | 0.019 | % per ms | speed scale = (100 − 0.019 × timer) / 100 |
| bhop_cap_factor | 1.2 | × maxspeed | take-off speed above this is cut |
| bhop_cut | 0.8 | × | the cut leaves 0.8 × the cap |
| air_wish_cap | 30 | units/s | air acceleration's target-speed cap |
| walkable_normal_z | 0.7 | | a plane is ground when its normal's z ≥ 0.7 |
| ground_probe | 2 | units | the ground test traces this far down |
| leave_ground_vz | 180 | units/s | rising faster than this is never on ground |
| edge_probe_ahead | 16 | units | edge friction probe, ahead along the velocity |
| edge_probe_down | 34 | units | edge friction probe depth |
| stop_epsilon | 0.1 | units/s | slide components smaller than this become 0 |
| bump_count | 4 | | sweeps per slide move |
| clip_planes_max | 5 | | planes per slide move |
| speed_floor | 1 | units/s | ground speed below this becomes 0 |
| ladder_speed | 200 | units/s | climb speed (capped by maxspeed; × 0.333 ducked) |
| ladder_jump_off | 270 | units/s | speed along the ladder's normal when jumping off |
| water_sink | 60 | units/s | downward wish speed in water with no input |
| water_wish_factor | 0.8 | × | wish speed in water |
| swim_up | 100 / 80 / 50 | units/s | upward speed holding jump in water / slime / lava |
| water_jump_up | 225 | units/s | water jump's upward speed |
| water_jump_push | 50 | units/s | toward the ledge during a water jump |
| water_jump_time | 2000 | ms | water jump duration |
| water_jump_low | 8 | units | first water-jump probe height above the origin |
| water_jump_reach | 24 | units | both water-jump probes' forward reach |
| water_current | 50 | units/s per water level | push of a current volume |
| fall_punch_min | 250 | units/s | minimum landing speed for any landing effect |
| fall_safe | 500 | units/s | fall damage starts above this |
| fall_fatal | 1100 | units/s | 100 / (1100 − 500) of damage per unit/s, before the × 1.25 |
| fall_damage_factor | 1.25 | × | |
| fall_sound_full | 580 | units/s | landing sound at full volume above this |
| fall_roll_punch | 0.013 | ° per unit/s | landing roll kick |
| punch_decay_const | 10 | °/s | punch angle decay |
| punch_decay_prop | 0.5 | 1/s | punch angle decay, proportional part |
| hit_slow_small | 0.5 | × | speed modifier after most hits |
| hit_slow_large | 0.65 | × | speed modifier after a "large flinch" hit |
| hit_knockback | 170 | units/s | push away from the shooter on a large flinch |
| hit_slow_recover | 0.01 | per command | modifier regained per ground command |

### Maximum speed by what is held (ref.)

The player's maximum speed is the held weapon's, recomputed whenever
the weapon, its zoom or its shield changes. The movement uses the
smaller of it and `sv_maxspeed`.

| Held | Speed | Zoomed |
|---|---|---|
| nothing | 240 | |
| knife, all pistols, grenades, C4, MAC-10, TMP, MP5, UMP45 | 250 | |
| P90 | 245 | |
| AUG, XM1014, FAMAS, Galil | 240 | AUG unchanged when zoomed |
| SG552 | 235 | 200 |
| M3, M4A1 | 230 | |
| AK-47 | 221 | |
| M249 | 220 | |
| AWP, G3SG1, SG550 | 210 | 150 |
| Scout | 260 | 220 |
| shield, raised (pistol, knife or grenade) | 180 | |
| shield, lowered | 250 | |
| VIP (any weapon) | 227 | |
| freeze time at round start | 1 | |
| observer | 900 | |

## Behavior

### Wish direction and the speed cap

At the start of each command the input amounts (forward, side, up) are
scaled down together, if their combined length exceeds the maximum
speed, to have length equal to the maximum speed. Frozen or dead
players have all inputs zeroed. Then, while the duck key is held or a
duck is in progress or the player is ducked, all three amounts are
multiplied by 0.333.

For walking and air movement the forward and right vectors are flattened
(z = 0) and renormalised; the wish velocity is forward · fmove +
right · smove (z = 0); the wish speed is its length, capped again at the
maximum speed.

### Ground acceleration

With wish direction **d** and wish speed W: current = **v**·**d**;
add = W − current; if add ≤ 0 nothing happens. Otherwise
a = accelerate · dt · W · (entity friction), capped at add, and **v**
gains a·**d**. The vertical component is zeroed before and after.

### Ground friction

Runs only on ground, before acceleration, after the vertical velocity is
set to 0. With speed s (≥ 0.1, else nothing):

- Edge test: a box trace of the current hull from the point 16 units
  ahead along the horizontal velocity direction, at the height of the
  box's bottom (origin + the hull's lowest z), straight down 34 units.
  If that trace hits nothing over its whole length, friction ×
  `edgefriction` (2). Because the box is centred on that point, it spans
  36 (standing) or 18 (ducked) units above and below it: the test only
  fires when there is no solid anywhere in a box-sized column reaching
  from the box's half height above the feet down to 34 + 36 (or 34 + 18)
  units below them, shifted 16 units ahead. Starting in solid counts as
  a hit (normal friction).
- f = friction (× 2 at an edge) × entity friction;
  drop = f · max(s, stop_speed) · dt; new speed = max(s − drop, 0); the
  velocity is scaled by new/s.

### Jump penalty (the landing slowdown)

A jump sets a penalty timer to 1315.789429 ms; it counts down by `msec`
each command. On **every ground walking command** while the timer is
above 0, before acceleration, the horizontal velocity is multiplied by

  r = 1 − 0.00019 × timer   (0.75 right after the jump, 1 when it ends).

The timer runs 1.316 s; a normal jump lasts about 0.68 s, so the first
ground command after landing still scales by about 0.879, and the next
~0.64 s of ground commands each scale again. Because the scale is applied
once per command, the loss depends on the command rate: at 10 ms commands
a landing with no input keeps only about 31 % of its speed after 10
commands (see Test cases); at higher rates more is lost. A new jump while
the timer runs also scales the jump's vertical speed by the same r.

### Air movement

Wish velocity as on the ground. Acceleration as on the ground except:
the target speed used for "how much to add" is min(W, 30), while the
amount added per command is air_accelerate · W · dt (W uncapped by the
30). Then the slide move (no stepping).

### Moving: walk move, slide move, stairs

After acceleration (and adding any base velocity, e.g. a conveyor), if
the horizontal speed is under 1 it is zeroed and the command moves
nothing. Otherwise:

1. Try the straight move to origin + **v**·dt (z unchanged). If the
   trace completes, take it and stop.
2. If the player was not on ground (and not in water), stop: no
   stepping in the air. During a water jump, stop.
3. "Down" try: a slide move from the original position.
4. "Up" try: from the original position trace up `sv_stepsize` (18);
   if that trace did not start or stay in solid, move there; slide move;
   then trace down 18. If that down trace's plane normal z < 0.7 (a
   steep plane or no hit), use the down try. Otherwise, if the trace
   started clear, take its end.
5. Compare the horizontal distance travelled (squared) of both tries;
   the down try wins only if strictly farther. When the up try wins, the
   vertical velocity is taken from the down try.

Slide move: up to 4 sweeps over the remaining time. A sweep that
starts and stays in solid zeroes the velocity and ends. A sweep that
moves any distance takes its end and clears the plane list. A full
sweep ends the slide. Otherwise the hit plane is added (at most 5; more
zeroes the velocity). Velocity clipping: when there is exactly one plane
and the player walks and either is in the air or has entity friction
≠ 1, the velocity is clipped against it: floors (normal z > 0.7) with
overbounce 1, others with 1 + bounce · (1 − entity friction). Otherwise
the velocity is clipped against each plane in turn (overbounce 1) until
one result points away from all the other planes; if none does: with
two planes, slide along their crease (the cross product direction,
keeping the velocity's component along it), with more, stop. If the new
velocity points against the velocity at the slide's start (dot ≤ 0),
stop. Clipping removes the component into the plane (times overbounce)
and zeroes components under 0.1 in size. If no sweep moved at all, the
velocity is zeroed.

### Ground detection

Done at the start of each command and after the move (and after a duck
changes the origin). Water level is updated first. If the vertical
velocity is above 180 the player is in the air. Otherwise a hull trace
from the origin 2 units straight down: if its plane normal z ≥ 0.7 the
player is on that entity (else in the air). On ground, the water jump
ends, and if the water level is below 2 and the trace did not start in
solid, the origin snaps to the trace end (so the player is kept on
slopes and down small drops of up to 2 units per command).

### Gravity

Walking players not in water (water level ≤ 1): before the move,
vz −= gravity · entity gravity · 0.5 · dt (plus any upward base
velocity · dt); after the move, the same half again. On ground the
vertical velocity ends at 0. Each component is then clamped to
± `sv_maxvelocity`.

### Jumping

On a command with the jump key down:

- Not allowed when: dead, in the air (the key is then latched as "used"
  until released), the key was already down on the previous command
  (no auto-jump: it must be released and pressed again).
- In water at level ≥ 2: no jump; the player swims up (vz = 100 in
  water, 80 slime, 50 lava).
- Otherwise: leave the ground; if the total speed (3D) exceeds
  1.2 × maxspeed, scale the velocity so that its length becomes
  0.96 × maxspeed (the cap times 0.8); set vz = 268.3281573 (scaled by
  the jump penalty's r if its timer is running); set the penalty timer to
  1315.789429 ms; apply the second gravity half immediately (and again at
  the end of the command, as every air command does).
- Ducked or ducking players jump the same way (no long jump in CS).

### Ducking

The duck key's press is detected against the previous command's key
state.

- Holding duck while standing on the ground: on the press the duck timer
  is set to 1000 ms and a duck is "in progress". The inputs are × 0.333
  from that very command. While in progress, the eye height eases from
  +17 to the ducked height with an ease-in-out curve
  f = 3u² − 2u³, u = (1 − timer/1000) / 0.4: eye = (12 − 18) · f +
  17 · (1 − f) relative to the (not yet moved) origin. When
  timer/1000 ≤ 0.6 (0.4 s after the press), the duck completes: ducked
  hull, eye +12, origin moved down 18 (the box's bottom stays on the
  floor), then if the box is now stuck, the origin is nudged up one unit
  at a time, up to 36 times, until free (else restored); ground is
  re-detected.
- Pressing duck in the air: the duck completes at once, without moving
  the origin: the box shrinks 18 from the top **and lifts its bottom 18
  units** (the duck-jump: feet clear 18 more units).
- Releasing duck (or a duck in progress): try to stand. On ground the
  test position is origin + 18, in the air the origin itself. If the
  standing box fits there, stand: standing hull, eye +17, timers
  cleared, origin moved to the test position, ground re-detected.
  Otherwise stay ducked and keep trying every command.
- A player who presses duck again before a duck completes restarts
  nothing: the timer is only set on a press while not ducked.

### Steps and slopes

Steps up to 18 units are climbed by the walk move's up try. Slopes with
normal z ≥ 0.7 (up to about 45.6°) are ground; steeper ones are slid on.

### Ladders

A ladder is a `func_ladder` brush entity. The player is on it when the
player's origin, tested against the ladder model's hull for the player's
current box size, is not empty. Not on a train and alive. On a ladder:

- Movement type becomes "fly": no gravity, no friction.
- A hull trace from the origin toward the ladder model's centre gives
  the ladder's facing plane. If it hits:
  - speed = min(200, maxspeed), × 0.333 when ducked;
  - forward/back/left/right are taken from the **keys** (not the input
    amounts): forward = ± speed, right = ± speed;
  - jump: leave the ladder with velocity 270 along the plane normal;
  - no keys: velocity 0;
  - otherwise the intended velocity is view forward × forward + view
    right × right; its part into the ladder's face is turned into
    vertical motion along the ladder (into the face moves up the ladder
    when looking up, down when looking down; the sideways part is kept);
    when standing on solid (the point 1 unit below the box) and pushing
    away from the ladder, 200 is added along the normal. Precisely: the
    part of the intended velocity along the ladder's normal is removed
    and the same amount (sign kept, into the face = up) is added along
    the ladder's surface, perpendicular to its horizontal edge; the rest
    is kept. So looking up while climbing is faster than looking level.
- The move itself is a slide move with any base velocity added and
  removed around it.

### Water

Water level, from point contents: 1 when the point 1 unit above the
box's bottom is water (or slime or lava), 2 when the origin (the box's
middle) is too, 3 when the eye is too. A current volume adds
50 × level along its direction to the base velocity. In water at level
≥ 2 there is no gravity.

Swimming (level ≥ 2): at level exactly 2, a water jump is tried (see
below). The wish velocity uses the full 3D view vectors plus the up
input (or −60 when there is no input at all), capped at maxspeed, then
× 0.8. Water friction: speed −= friction · entity friction · dt · speed.
Acceleration toward the wish speed with `sv_accelerate` (×
entity friction × dt × wish speed, capped at what is missing). Then the
move: from the destination raised by step size + 1, trace down to the
destination; if clear, take it, else slide move.

Water jump: only when not already water-jumping, vz ≥ −180, and not
moving backwards relative to the flat view direction. A point-hull trace
24 units forward from the origin + 8; if it hits a near-vertical wall
(|normal z| < 0.1), a second trace 24 units forward from the box's top
height; if that one is clear: vz = 225, a horizontal push of 50 toward
the wall for 2000 ms (the water-jump timer counts down by `msec`; it
ends early when leaving water or falling while swimming). During it,
acceleration and friction are off.

### Fall damage and landing

While in the air, the falling speed (−vz) is recorded at the start of
each command. On the command that lands:

- Landing effects (movement): if the falling speed ≥ 250: volume 1 above
  580, 0.85 above 290, 0 below 350 (so 250–290 is silent), always 0.5
  in water. When the volume is above 0: a landing footstep, the step
  sound timer set to 300 ms, and a roll punch of falling speed × 0.013
  degrees.
- Fall damage (game rules): on landing with falling speed above 500 and
  not in water: damage = (speed − 500) × (100 / 600) × 1.25 =
  (speed − 500) / 4.8, applied as whole points (truncated). Armour does
  not absorb it. The damage zeroes the pitch punch.

### Being hit

Every bullet or knife hit from a player sets a speed modifier (ref.,
details in damage.md): 0.5 for most hits; 0.65 plus a 170 units/s push
away from the shooter (only when the victim is slower than 300) for a
"large flinch": a hit not in the legs, victim not ducked, from a Scout,
AUG, SG550, Galil, FAMAS, AWP, M3, M4A1, G3SG1, Deagle, SG552 or AK-47.
On each command the player is on ground with the modifier below 1, the
modifier gains 0.01 and then the whole velocity is multiplied by it.
This is per command (frame-rate dependent): at 10 ms commands, from 0.5
it recovers in 50 commands (0.5 s) and the velocity is scaled by
0.51 × 0.52 × … on the way.

### View punch decay

Each command, before the angles are used: the punch vector's length L
becomes max(L − (10 + 0.5 · L) · dt, 0), direction kept. The view and
the shots use view angles + punch.

### Commands, milliseconds and frame rate

- The movement and the weapon run once per user command; the step is
  the command's `msec` × 0.001 s. The shipped client caps at 100 frames
  a second and sends one command per frame: 10 ms steps. Chimera's
  0.01 s tick is that case.
- Frame-rate dependent parts (all per command, not per second):
  the jump penalty's per-command speed scale, the hit speed modifier's
  recovery, the Glock's burst (weapons.md), weapon cycle times rounded up
  to whole commands (weapons.md). Time-based parts (correct at any rate):
  friction, acceleration, gravity, the timers themselves, punch decay.
- Whole milliseconds: a command's time is never fractional. At a
  1/66 s tick a party would step 15 ms and carry the remainder.

## Per-tick order

One command, a living walking player:

1. Clamp the inputs to the maximum speed; zero them if frozen; decay
   the punch angle; build the view angles (command angles + punch).
2. Step length = msec / 1000. Count the millisecond timers down (step
   sound, duck, swim sound, jump penalty).
3. If stuck in solid, try nudges (client) or wait (see Edge cases).
4. Detect water level and ground.
5. Record the falling speed if in the air.
6. Find a ladder.
7. Duck handling (input scale × 0.333, duck/unduck).
8. Footstep sounds.
9. Ladder movement if on one (fly type), else walking type.
10. Walking, not in water deeper than 1: first gravity half.
11. Water jump in progress: keep its push, slide move, update water,
    end the command.
12. Swimming (level ≥ 2): water jump check, jump key (swim up), water
    move, ground and water detection. Skip to 20.
13. Jump key: jump (not on a ladder), else clear the key latch.
14. On ground: vz = 0, friction (with the edge test).
15. Clamp velocity components.
16. On ground: walk move (jump penalty scale, accelerate, move with
    steps); in the air: air move.
17. Ground detection.
18. Clamp; second gravity half (not in water).
19. On ground: vz = 0. Landing effects.
20. Water enter/leave sounds.

The game rules then (after the movement, same command) handle the
weapon (weapons.md), fall damage and the hit speed modifier.

## Edge cases

- **Stuck**: at the start of a command, if the box is in solid: the
  client tries a table of small offsets; the server only retries one
  offset every 50 ms and otherwise does not move the player that
  command. A player stuck inside another player while pressing jump,
  duck or attack is pushed out to the first free spot on a grid
  (x, y in −8..8 step 8, z in 0..72 step 18).
- Ducking on ground under a low ceiling: the duck always completes
  (the box shrinks), but standing up waits until the standing box fits.
- A ducked player standing up in the air keeps the origin, so the box
  grows 18 down: under a ledge this can put the feet into the floor and
  fails; the player stays ducked.
- The edge test traces with the box, so an edge 16 units away from a
  box-wide floor rarely triggers; it acts mostly at the lips of real
  drops and when the box overhangs.
- Players are other boxes in the traces; a ducked player's box as seen
  by others reaches from −18 to +32 around its origin (ref., see Open
  questions).
- Bunny-hop clamp uses the full 3D speed at take-off (vz is −4 from the
  first gravity half at that point).

## Quirks

- The jump penalty is applied per command: higher frame rates lose more
  speed after landing. Keep it, at 10 ms commands, for parity.
- The duck's 0.333 input scale starts at the press, 0.4 s before the
  box shrinks: a player who taps duck slows immediately.
- Fall-damage threshold (500) and the landing sound thresholds (580,
  290, 350) differ; a fall at 290-500 units/s is heard but harmless.
- The jump speed does not follow `sv_gravity`.

## Test cases

At 10 ms commands, flat ground, maxspeed 250 (knife), default cvars.

| Setup | Input | After | Expected |
|---|---|---|---|
| standing still | hold forward | 1 command | speed 12.5 |
| standing still | hold forward | 2 commands | speed 22.0 (friction 3, +12.5) |
| standing still | hold forward | 10 commands | speed 97.31 |
| standing still | hold forward | 40 / 41 commands | ≥ 249 at 40; exactly 250 at 41, then steady |
| running at 250 | release | 1 command | 240.0 |
| running at 250 | release | 10 commands | 166.21 |
| running at 250 | release | 30 commands | 73.46 |
| running at 250 | release | 55 commands | 0 |
| standing still | jump once | apex | 44.99 units above the start; 68 commands in the air; lands at vz −275.67 |
| landing from a standing jump at 250 horizontal, no input | | first ground command | horizontal speed × 0.8792 (timer 635.79 ms) before friction |
| jump penalty timer 1315.789429 | | r | 0.75 |
| on ground, ducking pressed | | 39 commands | still standing hull; inputs × 0.333 |
| on ground, ducking pressed | | 40 commands | ducked hull, origin 18 lower, eye at feet + 30 |
| in the air, press duck | | 1 command | ducked hull, origin unchanged, feet 18 higher |
| ducked under a 60-unit ceiling | release duck | | stays ducked |
| landing at 600 units/s | | | 20 damage |
| landing at 1100 units/s | | | 125 damage |
| landing at 500 units/s | | | 0 damage, sound volume 0.85 |
| landing at 280 units/s | | | no sound, no punch |
| moving at 400 (3D) with maxspeed 250 | jump | take-off | speed scaled to 240 before vz is set |
| on a vertical ladder, facing it level, maxspeed 250 | hold forward | | velocity 200 straight up |
| same, looking 45° up | hold forward | | 282.8 up (141.4 kept from the view's own vertical part + 141.4 converted) |
| hit by an AK-47 in the chest, standing on ground at 250 | | next command | modifier 0.66, velocity × 0.66 |

## Open questions

1. Everything marked (ref.) is from the reference and has not been
   measured in the running game: a scripted client or a server plugin is
   needed to log positions per command. The client (app 10) is installed
   but needs a running Steam client and 32-bit libraries this box lacks.
2. How the client rounds a frame's time to `msec` (truncation and any
   carried remainder) is the engine's; at `fps_max` 100 it is taken to
   give a steady 10 ms.
3. Whether other players see a ducked player's collision box as
   (−16, −16, −18)-(16, 16, 32) (the game sets that size) or the ducked
   hull's ±18.
4. `cl_movespeedkey` and `cl_upspeed` client defaults (0.52 and 320 from
   the reference's era) to read from the client's console.
5. Whether a bullet trace counts a ducked player's box or only the
   hitboxes (damage.md).
