# Half-Life 2: Deathmatch: player movement (H1)

Source basis: the public Source SDK 2013 multiplayer game code, read in
place in a separate spec session: the shared player movement, the
Half-Life 2 ladder movement and the ladder and dismount entities, the
HL2 and HL2:DM player's speed selection and suit power handling (client
and server copies), the shared step-sound code, the multiplayer rules'
fall damage, the HL2:DM rules' hull table and player limits, the shared
movement console variables. The game's own files on this box (game
build 24600434, dedicated server build 24600426) were read for config
values, cvar names in the shipped server library, sound entry names and
the stock maps' ladder entities and brush contents. No game was launched
and no server was started.
Status: draft

## Summary

HL2:DM uses the shared Source controller of specs/cs_source/movement.md
with HL2 numbers and three HL2 systems on top. Gravity 600 and a fixed
160 units/s jump give a low (20.1-unit), quick hop with no stamina and no
speed cap, so bunny-hopping and air-strafing keep every bit of speed.
Speed comes from the buttons: run 190, `+walk` 150, `+speed` sprint 320
paid for by the suit's aux power. Ladders are `func_useableladder`
entities: the player is pulled onto a line between two points, climbs
along it at a fixed 200 units/s in a direction chosen by where they look,
and is pulled off onto `info_ladder_dismount` points. Falls cost a flat
10 health above 526.5 units/s. Footsteps are the player model's own run
sounds, on the shared timing, and never play while ducked.

This spec states only what differs from, or adds to,
specs/cs_source/movement.md ("movement.md") and
specs/cs_source/fall_damage.md ("fall_damage.md"). It continues
specs/hl2dm/source_deltas.md ("H0"), sections 1.1 and 1.2; where it finds
H0 imprecise it says so in the text and in Open questions.

## Units and conventions

- As movement.md: Source units, Z up, pitch positive looking down;
  forward from angles is (cos p·cos y, cos p·sin y, −sin p).
- Tick dt = 0.015 s (66.67 Hz). All test numbers use dt 0.015 and
  gravity 600, so a gravity half-step is 600 × 0.0075 = **4.5 units/s**.
- Timers in milliseconds go down by 1000·dt = 15 per tick.
- "Tick 1" is the first tick on which the input of a test is held; "tick
  k after X" counts X's tick as 0.
- "Tick-start velocity" is the player's velocity as stored at the end of
  the previous tick (before this tick's movement).
- Where movement.md uses a CS:S measured value (hulls 62/45, eye 47,
  duck lift 8.5, duck scale 0.34, jump 301.993, stamina, the 286 cap,
  ladder dampening, swim lift 260), HL2:DM uses the shared value or the
  value below, never the CS:S one.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| gravity | 600 | units/s² | `sv_gravity` |
| jump_impulse | 160 | units/s | fixed; not recomputed from `sv_gravity` |
| hull_stand | (−16,−16,0)–(16,16,72) | units | eye 64 |
| hull_duck | (−16,−16,0)–(16,16,36) | units | eye 28; dead view height 14 |
| air_duck_lift | 36 | units | feet move up by the whole height difference |
| duck_input_scale | 1/3 | – | 0.33333333 |
| speed_walk / run / sprint | 150 / 190 / 320 | units/s | `hl2_walkspeed`, `hl2_normspeed`, `hl2_sprintspeed` |
| max_speed | 320 | units/s | `sv_maxspeed`; caps every speed above |
| input_speed | 450 | units/s | `cl_forwardspeed`, `cl_sidespeed`, `cl_backspeed` |
| accelerate / airaccelerate | 10 / 10 | – | `sv_accelerate`, `sv_airaccelerate` |
| friction / stopspeed | 4 / 100 | – / units/s | `sv_friction`, `sv_stopspeed` |
| sprint_start_min | 10 | aux | aux needed to start sprinting |
| sprint_stop | 0.01 | aux | sprint ends below this |
| sprint_drain | 25 | aux/s | only while horizontally moving |
| still_epsilon | 0.001 | units/s | \|vx\| and \|vy\| at or below this count as not moving |
| device_off_cost | 2.5 | aux | taken once when sprint switches off |
| recharge / recharge_delay | 12.5 / 0.5 | aux/s / s | (H2) |
| fall_punch | 303 | units/s | landing effects from here |
| fall_safe | 526.5 | units/s | damage above |
| fall_half_safe | 263.25 | units/s | volume 0.85 above |
| fall_fatal | 922.5 | units/s | 100 damage (progressive mode) |
| fall_bounce, floating_bonus | 173, 173 | units/s | |
| fall_fixed | 10 | health | `mp_falldamage` 0 |
| land_roll | 0.013 | ° per unit/s | as movement.md |
| ladder_find | 64 | units | ladder search radius |
| ladder_cone_dist / angle | 32 / 15 | units / ° | auto-mount |
| ladder_really_close | 4 | units | |
| ladder_end_radius | 16 | units | end-point mount; "near an end" |
| ladder_axis_dot | 0.4 | – | `sv_ladderautomountdot` |
| climb_speed | 200 | units/s | along the ladder |
| climb_changeover | 0.3 | – | 0 for ladders under 64 units tall |
| ladder_fall_off | 6 | units | distance from the ladder line |
| ladder_jump | 200 along view, +50 up | units/s | |
| ladder_use_leave | 100 along view, vz = 50 | units/s | |
| dismount_list / sample | 100 / 40 | units | ladder's node list at map start |
| dismount_pick_radius | 100 | units | |
| dismount_dot / strict_dot / strict_dist / alternate_dist | 0.5 / 0.7 / 40 / 60 | – / – / units / units | |
| step_band_walk / run | 90 / 220 | units/s | standing; 60 / 80 ducked or on a ladder |
| step_time | 400 slow, 300 fast; ladder 350; waist water 600; +100 ducked or on ladder | ms | |

## Behavior

### Cvars and configs

The registered defaults are the shipped values: neither install's
`cfg/skill.cfg` (only `sk_*`), `cfg/config_default.cfg` (binds:
SPACE `+jump`, CTRL `+duck`, ALT `+walk`, SHIFT `+speed`), `cfg/valve.rc`
in `hl2mp_pak.vpk` (`hostname`, `mp_forcerespawn 0`) nor Half-Life 2's
`cfg/valve.rc` sets a movement cvar. Movement cvars HL2:DM adds to
movement.md's list: `hl2_walkspeed` 150, `hl2_normspeed` 190,
`hl2_sprintspeed` 320, `sv_ladder_useonly` 0, `sv_autoladderdismount` 1,
`sv_ladderautomountdot` 0.4, `sv_showladders` 0 (debug), `player_limit_jump_speed`
1 (unused, see Jumping), `xc_uncrouch_on_jump` 1, `sv_stickysprint` 0,
`sv_infinite_aux_power` 0 (cheat: aux never drains), `sv_autojump` 0
(an HL2 experiment; left out). Absent: `sv_enablebunnyhopping`,
`sv_ladder_dampen`, `sv_ladder_angle`, `sv_timebetweenducks`.

### Speed selection (walk, run, sprint)

Runs first in every tick, before movement.md's max-speed step, on the
server and in client prediction alike (the max speed is never
networked). Reads: the buttons this tick and last tick, the duck state
and water level left by the previous tick, aux power, whether the suit
is worn (always, in DM), a "sprint allowed" flag. Writes: the sprint
state (kept between ticks), the player's max speed P, two sounds.

1. **Sprint wanted** = `+speed` held, suit worn, and sprint allowed now.
   Sprint is allowed when the player is not fully ducked (ducked and not
   in a transition), the water level is not 3 (eyes), and nothing has
   disabled it (carrying an object with `+use`, H5; off by default).
2. **Sprint changes only on a tick where `+speed` changed** (pressed or
   released) and "wanted" differs from the current state:
   - wanted: if aux < 10, no sprint, and `HL2Player.SprintNoPower` plays
     (that tick only); otherwise sprint starts;
   - not wanted (released): sprint stops.

   On every other tick the state stays as it was, whatever the duck or
   water state does.
3. If aux < 0.01, sprint stops.
4. **Walking** = `+walk` held, not sprinting (after 2–3), and `+duck` not
   held. Walking also forces sprint off (it cannot, given its own
   condition; without a suit walking is forced on). So sprint beats
   walk, and holding duck cancels walk.
5. P = 320 if sprinting (and `HL2Player.SprintStart` plays if `+speed`
   changed this tick), else 150 if walking, else 190. The movement cap is
   M = min(P, `sv_maxspeed`) × the ground material's speed factor, then
   movement.md's input rescale and wish clamp apply unchanged.

Consequences:
- Sprint must be re-pressed after running dry, after pressing `+speed`
  while ducked or under water, and after a "no power" press, even when
  aux later passes 10 while `+speed` is held.
- **A sprint that started before ducking or diving carries on** while
  `+speed` stays held: ducked, the wish speed is 320 × 1/3 = 106.67
  instead of 63.33, and aux keeps draining. Releasing `+speed` ends it.
  (The game's own "stop sprinting while ducked" step is empty in
  HL2:DM.) Probe 9.
- Client prediction differs from the server: the client also refuses
  sprint during a duck or unduck transition, the server does not. The
  server is authoritative; implement the server rule.

### Sprint and aux power, per tick

The aux pool itself (0–100, recharge, other devices, breath) is H2's
(specs/hl2dm/suit.md). What movement does to it, in each tick right
after speed selection and before the shared timers:

1. If P is the sprint speed, the sprint device is switched on (free).
   Otherwise, if it was on, it is switched off: aux loses 2.5 at once
   (not below 0) and the "all devices off" time is set to now when no
   other device is on.
2. Aux update for the tick: if no device is on, aux < 100 and at least
   0.5 s have passed since the "all off" time, aux += 12.5·dt (0.1875).
   Otherwise, while a device is on, aux −= load·dt, where the sprint's
   25/s counts only when the tick-start velocity has |vx| > 0.001 or
   |vy| > 0.001. If aux would fall below 0.01 it becomes 0.

So the drain uses the velocity before this tick's move: the first
sprint tick from rest costs nothing (velocity 0), and every later
moving tick costs 0.375. **H0 test case 3 (99.625 after one tick from
standing) is one tick early** (Open questions 2). Climbing a vertical
ladder (no horizontal velocity) costs nothing. The "0.5 s" test uses
"now < off time + 0.5", so recharge starts on the 34th tick after the
switch-off (0.51 s), not the 33rd (0.495 s).

### Jumping

As movement.md's Jumping, with these differences:

- Impulse 160 × the ground material's jump factor, fixed (not
  √(2·g·h)). Added to vz standing, replaces vz when ducked or in a duck
  transition. Then the extra gravity half-step (4.5).
- **No stamina, no landing slowdown, no speed cap.** HL2:DM has none of
  movement.md's CS:S jump stamina or 286 cap; the horizontal velocity is
  untouched by the jump.
- **`player_limit_jump_speed`** (1, replicated) is registered but never
  read: setting it changes nothing (Probe 7).
- **The forward boost exists only when the server has exactly one
  player slot,** and HL2:DM's game code sets the minimum player count to
  2, so a running HL2:DM server never applies it. For the record, what
  it does on a one-slot server, after the impulse:

  ```math
  p = \begin{cases} 0.5 & \text{not sprinting and not fully ducked} \\ 0.1 & \text{otherwise} \end{cases}
  \qquad a = |f|\,p, \qquad C = M(1+p)
  ```

  where f is the forward move after the rescale (190 running). If
  a + |v_xy| > C, a is reduced by the excess (it can go negative). If
  f < 0, a is negated. Then v_xy += a·ĝ, with ĝ the flattened view
  forward. Moving along the view this caps speed at C on each jump;
  moving **backwards relative to the view** faster than C while not
  holding back, the negative a points along the motion and the speed
  becomes 2|v| − C each hop: the "accelerated back hop" of single-player
  HL2. (HL2:DM never sets the "sprinting" flag this reads, so p would be
  0.5 when sprinting too.) None of this applies to HL2:DM; back-hopping
  there is ordinary bunny-hopping (Probe 7, Open questions 3).
- The single-player duck-jump eye path is likewise off (one-slot only).
- No jump while standing in a `trigger_vphysics_motion` (it sets the
  shared "slow movement" state). No stock map has one.
- `xc_uncrouch_on_jump` 1: a jump un-toggles the toggle-duck state (the
  `toggle_duck` style of ducking, server side only); `+duck` is
  unaffected.
- On the jump tick a footstep plays at volume 1 (silent if fully
  ducked); see Step sounds.

Standing jump on flat ground (vz 0, grounded): first half −4.5, +160,
extra half −4.5: the move uses 151.0, the tick ends at 146.5. From then
on the move of tick k uses 151 − 9(k − 1).

```math
z_n = 0.015\,\big(151\,n - 4.5\,n(n-1)\big)
```

Apex 20.145 at the end of tick 17 (move vz 7.0; tick 18 uses −2.0).
Ground comes back at the end of **tick 34**: the move leaves the feet
1.275 above the floor, inside the 2-unit ground probe (the move vz −146
is not "rising"), and the fall speed recorded at the start of tick 34 is
141.5 (no landing effect). H0 says 35 (Open questions 1). Ducked jump:
the move uses 155.5 − 9(k − 1), apex 21.33 at the end of tick 18,
ground at the end of tick 35 (1.3125 above the floor).

### Bunny-hopping and air movement

- Friction is skipped on the jump tick (shared), and nothing else slows
  a hop, so chained hops keep their speed exactly.
- Air acceleration per tick = 10 × wish × 0.015 × surface friction:
  28.5 running (below the 30 cap: from rest the air gives 28.5, then
  30), 22.5 walking, 48 sprinting (cut to the 30 cap by the add test).
  Perpendicular strafing therefore adds 28.5² = 812.25 to |v|² a tick
  when running and 900 when sprinting (movement.md's formula).
- Surface friction 0.25 (movement.md's "rising slowly" rule) applies to
  ticks 4–18 of a standing jump (detections at the end of ticks 3–17 see
  0 < vz ≤ 140); ticks 1–3 and 19–34 use 1. One ideally strafed running
  hop: |v|² gains 19 × 812.25 + 15 × 7.125² = 16 194.23.

### Ducking

The shared duck logic of movement.md with HL2:DM's hulls (the shared
values, not CS:S's measured ones): box top 72 → 36, eye 64 → 28,
0.4 s down, 0.2 s up, eye curve 64 − 36·S(t), input scale exactly 1/3
on the ground (decided from the tick-start state, as movement.md), none
in the air, none on ladders. In the air a duck is instant and lifts the
feet 36 (head stays); an in-air unduck needs 36 units free below the
feet. At dt 0.015 the ducked box is in use 27 ticks after the press
(0.405 s > 0.4) and the standing box 14 ticks after the release
(0.21 s > 0.2). H0 marks the lift and the scale "to be measured" (H0
Open questions 4): these are the SDK values; Probe 4 and 5 confirm.

Consequences of the full 36 lift with a 20-unit hop:
- Standing jump, duck any time before the apex: feet peak at
  20.145 + 36 = 56.145.
- Duck and jump pressed on the same grounded tick: the duck starts a
  transition (not finished on the ground), so the jump replaces vz
  (ducked jump, 21.33); the next tick, in the air, the duck finishes and
  lifts the feet 36: feet peak 57.33.

### Ladders (`func_useableladder`)

Contents ladders are gone: HL2:DM replaces movement.md's whole Ladders
section. A brush with ladder contents or a climbable material is never
climbed; it collides as its other contents say. dm_lockdown's 10 such
brushes carry ladder + grate + translucent contents (two also detail),
and grate is in the player-solid mask, so **they block players like a
grate** (bullets, whose mask has no grate, pass) and are not climbable
(H0 Open questions 5; Probe 11).

**The ladder entity (map data).** A point entity, never solid, never
drawn, networked to clients in its PVS. Keys:

| Key | Meaning |
|---|---|
| `point0`, `point1` | world positions of the player's origin (feet) at the two ends; whichever has the higher z is the top |
| `StartDisabled` | 1: starts disabled (inputs `Enable`, `Disable`) |
| `spawnflags` | 1: "don't get on": the mount checks run, fire the output and the sound, then refuse |
| `ladderSurfaceProperties` | surface for ladder step sounds; unused by HL2:DM's model footsteps |
| `targetname`, `parentname` | the points move with a parent |
| outputs | `OnPlayerGotOnLadder` (activator: the player), `OnPlayerGotOffLadder` |

At map start each end point is fixed up: a standing hull is swept from
4 units above the point down to it (brushes only); if that sweep does
not start in solid the point becomes its end, so a point at or slightly
inside a floor is lifted out (to 1/32 above a floor it touches). Then the
ladder collects its dismount list: every `info_ladder_dismount` within
100 units of the top, of the bottom, or of the points 40, 80, …
units up from the bottom (while below the top), skipping a node whose
`target` names an entity other than this ladder (the first entity of
that name; the client skips none). The stock maps' 26 ladders and 131
nodes are all plain (`spawnflags` 0); dm_underpass has three ladders
named `ladder_03` (none of its nodes has a target).

The ladder line is the infinite line through the two points; "the
segment" is the part between them; L is its length; up is the unit
vector bottom → top.

**Finding a ladder** (every tick while not on one, not dead, not on a
train, not in noclip): among enabled ladders, take the closest point C
on each segment to the player's origin; keep those with |C − origin| ≤
64 and a clear line (player-solid mask) from the origin to C, or, when
that line hits something other than the ladder, from the origin raised
by half the current box height (36 standing, 18 ducked) to C. The
nearest kept ladder is the candidate. With `sv_ladder_useonly` 1 a
candidate shows the `#Valve_Hint_Ladder` hint, at most once a minute.

**Mounting.** Only when not on a ladder:
- **`+use` pressed this tick** (a fresh press): mount the candidate if
  the player faces it: with g the view forward with z zeroed (not
  renormalized), the flat direction from origin − 8·g to C has a dot ≥ 0
  with g normalized (≥ −0.99 with `sv_ladder_useonly` 1). A fresh `+use`
  press disables the automatic mounts below for that tick.
- **Automatic** (only with `sv_ladder_useonly` 0, no fresh `+use`, and
  **forward held**: forward move > 0; H0's "from the air or pressing
  forward" is imprecise, forward is needed in the air too, Open
  questions 4). With d = |C − origin| (3D, from the feet), φ the angle
  between the flat view forward and the flat direction to C, strafing =
  |side move| ≥ 1, along = |up · F| > 0.4 (F the 3D view forward), the
  **cone** mounts when 0 < d < 32 and (φ < 15° or d < 4) and ((cos φ > 0
  and not strafing) or along). Failing that, the **end point** mounts
  when the origin is within 16 of the top or bottom point and F points
  along the ladder toward the other end by more than 0.4 (more than
  23.58° up from the bottom, or down from the top).
- **The mount itself**: the ladder fires `OnPlayerGotOnLadder` and plays
  `Ladder.StepRight` from the player; a "don't get on" ladder stops
  here. The server then tests the player's current box at C (and at C +
  1 up); if both are blocked (another player, a prop) the mount is
  refused, after the sound and output. Otherwise a **forced move**
  starts toward C at speed M (the tick's movement cap: 190 running, 150
  walking, 320 sprinting), and `+use` is consumed for the tick.

**Forced moves** (mounting and dismounting share them). While one runs
the player is not solid and does not move by physics (no gravity, no
stuck test, no collision, no input; velocity is kept but unused), and a
player-sized invisible solid box owned by the player sits at the
destination so nobody else can step into it. With T = |goal − start| / M
(at least 0.001 s), on the k-th tick after the start the origin is
start + min(k·dt / T, 1)·(goal − start). On the first tick with
k·dt > T the move ends: solid again, velocity zero, move type walking,
or on the ladder for a mount. A mount's arrival tick then runs the
ladder tick below (climbing at once if a climb key is held). A dismount's
arrival tick runs the walking move with the input zeroed.

**Climbing** (every tick on a ladder, before the move):
- the ladder is dropped (walking, velocity kept) if it was disabled or
  the player's base velocity (conveyor, push trigger, current) exceeds 1
  unit/s; this drop fires no `OnPlayerGotOffLadder`;
- the gravity scale is reset to default; ladder movement has no gravity;
- **jump held** (no fresh press needed): leave the ladder; d̂ = F, or −F
  when the forward move is negative (back held without forward);
  v = 200·d̂, then vz += 50 unless F points down (F_z < 0). The walking
  move runs this same tick (movement.md's order), with its normal jump
  check: a player whose feet are within the ground probe of a floor and
  who pressed jump fresh also gets the 160 impulse;
- **climb keys**: fs = ±1 for forward/back, rs = ±1 for right/left (each
  pair cancels). If fs = rs = 0, v = 0 (the player hangs). Otherwise:
  1. if the origin is more than 6 units from the ladder line, leave the
     ladder (walking, velocity kept) and run the walking move this tick;
  2. u = normalize(fs·F + rs·R), R the view right;
  3. c = 0.3, or 0 when the ends' heights differ by less than 64;
  4. the climb goes **down** when u_z ≥ 0 and up·u < −c, or when u_z < 0
     and −up·u > c; otherwise **up**;
  5. v = ±200·up. The speed is always exactly 200, whatever the pitch,
     the walk, sprint or duck state.

  On a vertical ladder this means: forward climbs up unless looking down
  more than asin 0.3 = 17.46°; back climbs **up** too unless looking
  **up** more than 17.46°; a strafe key alone climbs up; forward plus a
  strafe at 20° down climbs up (u_z = −0.342/√2 = −0.242). Probe 10.

**The ladder move** (ladder move type): water check; the jump-held flag
is cleared; the ground is cleared; the parametric position along the
line is noted (t = 0 at the top, 1 at the bottom); the slide move runs
with base velocity (no gravity, friction, stepping or landing). Then,
with t' the new position, "pressing" = forward or side move ≠ 0,
"along" = pressing and |F·up| > 0.9 (within 25.84° of straight up or
down), "near" = origin within 16 of either end point, "at end" = within
200·dt (3 units at dt 0.015) of either end point:

1. **Would leave** = t' < −1/L and t' < t (more than 1 unit past the top
   and moving up), or t' > 1 + 1/L and t' > t (past the bottom, moving
   down).
2. **Use-dismount** = `+use` pressed this tick, or (at end and pressing
   and not along and `sv_autoladderdismount` 1).
3. **Under water** (level 3) without use-dismount: try a strict
   dismount (below); if one starts, done.
4. Without use-dismount: if it would leave, put origin and velocity back
   as before the move (the player stops at the end). Done.
5. If it would not leave and the player is not near an end: on a fresh
   `+use`, try a dismount node; if none, **leave the ladder** with
   v = 100·F and then vz = 50 (overwritten). Done.
6. A fresh `+use` is consumed. Try a dismount node (with the
   "alternate" fallback only on a fresh `+use`); if one starts, done.
7. If it would leave: a fresh `+use` leaves the ladder with 100·F, vz 50;
   otherwise origin and velocity are put back.

A ladder left in step 5 or 7 runs no walking move that tick (the move
type was already dispatched); the new velocity is used from the next
tick.

**Choosing a dismount node.** From the ladder's list, nodes within 100
units of the origin, nearest first. Each candidate spot is the node's
origin + 1 up; skip it if the player's current box there is blocked.
With s the flat direction from the eye to the spot and D its flat
distance, score = s · F (the 3D view forward, so looking up or down
lowers it: no node qualifies beyond 60° of pitch). A score < 0.5 only
records the spot as the "alternate" when D is the smallest so far. The
best score ≥ 0.5 wins; a strict choice also needs score ≥ 0.7 and
D ≤ 40. With no winner, the alternate is used when allowed and D ≤ 60.
The winner starts a forced dismount to the spot at speed M (refused
silently on the server if the reserved box is blocked; the player stays
on the ladder and may have moved past the end by one tick). Leaving the
ladder by any way except the drop above fires `OnPlayerGotOffLadder`.

**Ducking on a ladder** works as anywhere in the air (instant, feet +36).
On a slanted ladder the lift moves the origin off the line by
36·sin(slant), so beyond about 9.6° of slant a duck while holding a
climb key drops the player (fall-off rule).

### Water

Movement.md's Water section holds, including the water jump, with
these values:
- Swim wish cap 0.8 × M: 152 running, 120 walking, 256 sprinting.
- Holding jump at waist depth or deeper adds P (the speed selection's
  190, 150 or 320) upward, not CS:S's measured 260; the look-up lift's
  clamp is 0 to P.
- Sprint cannot start at water level 3 but continues if already on (see
  Speed selection); it can start at level 2.
- During a water jump at level ≤ 1 vz falls by 4.5 a tick; the 2000 ms
  timer ends it on the 134th tick after the trigger. On the trigger tick
  the swim friction takes vz from 256 to 240.64 (×0.94); no acceleration
  is added (240.64 > 152).
- Drowning and the breath/aux interaction are H2's (suit.md).

### Always-allowed ground acceleration

HL2:DM removes movement.md's "no acceleration while dead or water-jumping"
gate from the ground (and spectator and noclip) acceleration; the air
acceleration keeps its own gate. For a living walking player this
changes nothing visible: a water jump replaces the whole walking move,
and a dead player's move inputs are already zeroed. Its only effect is
that dead, roaming spectators accelerate (H6). H0's row is right but has
no movement consequence (Open questions 6).

### Ground

A ground probe hit on a combine ball fired by a hostile NPC (or by a
launcher flagged to hit players) is ignored and the previous ground
stays. HL2:DM's player-fired AR2 balls use a different collision group
and are **not** excluded, so H0's "a combine ball can never become the
player's ground" is too broad (Open questions 5; H4).

### Falling and landing

Movement.md's landing check with the HL2 thresholds, and fall_damage.md's
rule replaced:

- Landing effects need fall speed ≥ 303 and the player alive. Out of
  water: subtract 173 on a floating object; add a descending ground's
  vz (floor 0.1); then fall > 526.5: damage, volume 1; > 263.25: volume
  0.85; < 173: volume 0 (no effects); otherwise 0.5. In water (level ≥ 1
  on a walking tick): no damage, volume 0.5. On plain ground every
  landing from 303 to 526.5 is 0.85; the 0.5 tier needs water or a
  floating object.
- **Damage** (`mp_falldamage`, server): 1 means progressive, any other
  value fixed:
  - fixed: 10;
  - progressive: the fall speed itself is lowered by 526.5 and the
    damage is the remainder × 100/396.

  ```math
  D_\text{prog} = (v - 526.5)\cdot\frac{100}{922.5-526.5} = (v-526.5)\times 0.252525\ldots
  ```

  Health takes the whole part; the fraction is carried in a per-player
  accumulator and adds 1 when it reaches 1. Armour takes nothing (fall
  damage bypasses it); no attacker (the world). `Player.FallDamage`
  plays when health was taken.
- If volume > 0: the step timer is set to 400 ms, a footstep plays at
  that volume (silent if fully ducked), and the view punch roll = the
  fall speed × 0.013°. **In progressive mode that fall speed has already
  been lowered by 526.5**, so a damaging landing rolls the view only by
  (v − 526.5) × 0.013. Keep.
- Map inputs on the player, `IgnoreFallDamage` and
  `IgnoreFallDamageWithoutReset` (seconds; ≤ 0 means 10): fall damage
  in that window does nothing (no health, no `Player.FallDamage`); the
  first one also ends the window at the first ignored landing. Unused in
  the stock maps.

Fall from rest off a ledge h above the floor: the move of tick k uses
vz = 4.5 − 9k, the feet drop 0.0675·n² by the end of tick n, the landing
tick is m = ⌈√((h − 2)/0.0675)⌉ (the first end within the 2-unit probe)
and the fall speed is 9(m − 1). Damage starts at m = 60 (fall 531, h >
236.97); progressive death from 100 health at m = 104 (fall 927, h >
718.11).

### Step sounds

- The shared step logic (movement.md left it to game code): runs once
  per tick before ducking, with the tick-start velocity, origin, ground
  flag and move type. The step timer counts down 15 ms a tick; a step
  can play once it is 0. No steps when frozen, in noclip or observer, or
  with `sv_footsteps` 0 (`mp_footsteps` is registered but read by
  nothing that plays steps; H0's "gate" is only `sv_footsteps`, Open
  questions 7).
- A step needs speed (3D) ≥ the walk band and either the ladder move
  type or (the ground flag and horizontal speed > 0.0001). Bands: 90/220
  standing, 60/80 fully ducked or on a ladder; "slow" = speed < the run
  band.
- Interval and volume, by where the player is: ladder: 350 ms, 0.5;
  water level 2: 600 ms, 0.65; water level 1: 400 ms slow / 300 fast,
  0.2 / 0.5; otherwise (level 0 or 3) by the ground material: 400 / 300
  ms and volume 0.2 / 0.5, except dirt 0.25 / 0.55 and vent 0.4 / 0.7;
  +100 ms when fully ducked or on a ladder; ×0.65 volume when ducked
  (moot, see below). With no ground material, nothing plays and the
  timer is not set (it retries next tick).
- In ticks: the step after the one that set T ms comes on tick ⌈T/15⌉:
  400 → 27, 300 → 20, 450 → 30, 600 → 40.
- **HL2:DM's speeds sit in the slow band**: running (190) and walking
  (150) both step every 27 ticks at 0.2; only sprinting (320, ≥ 220)
  steps every 20 ticks at 0.5. Ladder climbing (200) steps every 30
  ticks at 0.5.
- Water level 2 uses a counter shared by every player in the server
  process: of every four step attempts the first returns without playing
  and without setting the timer, so that step comes one tick later. H0's
  "every-fourth-step skipping" is in effect a one-tick delay (Open
  questions 7).
- **What plays**: the model type's `NPC_Citizen.RunFootstepLeft` /
  `Right`, `NPC_CombineS.RunFootstep…` or `NPC_MetroPolice.RunFootstep…`,
  alternating sides (the side flips before each step; a fresh player's
  first step is Left), on the body channel at the computed volume, with
  the entry's own level and pitch. The surface never changes the sound,
  on ladders and in water too.
- **Nothing plays while fully ducked** (the ducked flag): no steps, no
  jump step, no landing step. The timer is still set.
- The jump step (volume 1) does not set the timer; a landing step sets
  it to 400.

## Per-tick order

One user command, replacing movement.md's steps 2–4 and 9–11 as noted;
everything else as movement.md:

1. Speed selection (sprint, walk, run; sounds) from the buttons, the
   previous tick's duck and water state and aux power. Sets P.
2. Movement.md step 2: M, input rescale, zeroing; punch decay; view
   angles.
3. Sprint device on/off and the aux update (tick-start velocity), then
   the shared timers (duck, jump, swim sound).
4. View vectors; stuck check (skipped in forced moves: move type none);
   ground and water categorization (full when not walking); fall speed if
   airborne.
5. Step-sound update (tick-start velocity, before ducking).
6. Ducking (with the 1/3 crop).
7. Ladder step (not dead, not on a train; noclip drops the ladder):
   a. a running forced move continues; if it has not arrived, the tick
      ends (move type none);
   b. drop the ladder if disabled or pushed by base velocity;
   c. not on a ladder: find a candidate; `+use` mount or automatic mount
      (starts a forced move, the tick ends) or nothing (go to 8 as
      walking);
   d. on a ladder: jump-off (leave, set v, continue to 8 as walking),
      fall-off (leave, continue to 8 as walking) or set the climb
      velocity.
8. By move type: none: nothing; ladder: the ladder move (water check,
   slide, would-leave, dismount); walking: movement.md step 11 with the
   HL2 jump.
9. Store this tick's buttons.

## Edge cases

- **Mount refused by a blocked spot**: the sound and output still fire,
  every tick the automatic conditions hold (a player standing on the
  bottom point makes the next one's approach repeat `Ladder.StepRight`).
- **Two ladders in reach**: the nearest closest point wins, even if the
  player faces the other.
- **Mounting from below the bottom or above the top**: C is the end
  point; d includes the vertical gap, so a player standing 8 below a
  bottom point is already 8 away.
- **Directly under or over C** (flat distance 0): φ is taken as 90°; the
  cone mounts only within 4 units.
- **At the top with no node**: the player stops within 1 unit past the
  top point and stays (holding forward), until jump or `+use` near the
  end (which, with no node and no "would leave", does nothing; jump
  works).
- **Looking steeply along the ladder** (within 25.84°): no automatic
  dismount at the ends.
- **Mid-span `+use` with a node within 100 and score ≥ 0.5**: the
  player is moved to that node even from the middle of the ladder.
- **A player still holding a climb key** but moved more than 6 units off
  the line (by a push or a duck on a slanted ladder) falls off; one
  holding no key stays attached at any distance.
- **During a forced move** the player is not solid: other players and
  traces pass through (whether bullets can hit them: Open questions 9).
  Gravity and fall speed don't accumulate; the kept velocity is zeroed
  on arrival, so a fall caught by a ladder mount does no fall damage.
- **Jump-off near the floor**: the ladder tick clears the ground, but
  the next tick's start categorizes it again; jump-off with fresh jump
  and feet within 2 of a floor also does a normal jump (vz 50 − 4.5 +
  160).
- **Death or noclip on a ladder**: noclip drops the ladder; a dead
  player skips the ladder step entirely.
- **Sprint state across ducks and dives** (Speed selection).
- **`sv_maxspeed` below 320** caps sprint (and lower values cap all).
- **Client prediction**: the client's copy of the fall thresholds is the
  non-HL2 set (350 punch, 580 safe, 200 bounce), so the local player's
  predicted landing sound and roll differ from the server's for falls
  between 303 and 350 and around the damage threshold (Open questions
  8). The client builds its ladders' dismount lists without the
  `target` filter. Implement the server's values.

## Quirks

- **No bunny-hop cap, no stamina** (keep): chained hops lose nothing.
- **Back climbs up** on a ladder unless looking up more than 17.46°
  (keep; Probe 10).
- **Strafing on a ladder climbs up** at 200 (keep).
- **Sprint survives ducking and diving** while `+speed` stays held
  (keep; Probe 9).
- **Sprint costs nothing on its first tick from rest and while standing
  still**; each release costs 2.5 and restarts the 0.5 s recharge delay
  (keep).
- **Running steps on the slow timing** (190 < 220) (keep).
- **Progressive fall damage shrinks the landing roll** (keep).
- **Ladder mount sound spam** on a blocked mount (keep; rare).
- **The waist-water step counter is shared by all players** (keep only
  if cheap; it only delays a step by a tick).

## Test cases

Implementer checks, headless. Unless stated: dt 0.015, gravity 600,
`sv_accelerate` 10, `sv_airaccelerate` 10, `sv_friction` 4,
`sv_stopspeed` 100, surface friction 1, flat floor at z = 0, suit worn,
aux 100, keys give ±450 rescaled to M, view yaw 0, pitch 0. "Speed" is
horizontal speed after the tick. Tolerance ±0.01.

**The test ladder** (rows 30–45): `point0` (0, 0, 264), `point1` (0, 0,
8) (top 264, bottom 8, nothing within 4 below either, so no fix-up),
L = 256, a solid block x 17…400, z 0…264 behind it (its top is a ledge at
264), node D `info_ladder_dismount` at (48, 0, 264) unless stated.

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | at rest on ground | hold forward | ticks 1–9 | speed 28.5, 51.0, 73.5, 96.0, 118.5, 139.89, 160.00, 178.90, then exactly 190 (stays) |
| 2 | at rest | hold forward + `+walk` | ticks 1–9 | 22.5, 39.0, 55.5, 72.0, 88.5, 105.0, 121.2, 136.43, then exactly 150 |
| 3 | at rest | hold forward + `+speed` | ticks 1–9 | 48, 90, 132, 172.08, 209.76, 245.17, 278.46, 309.75, then exactly 320 |
| 4 | moving 190, no input | – | ticks 1 / 10 / 11 / 12 / 26 / 27 | 178.6 / 102.34 / 96.20 (×0.94 still) / 90.20 (−6 a tick) / 6.20 / 0 (under 1, zeroed) |
| 5 | fully ducked at rest | hold forward | ticks 1–3, 16, 17 | 9.5, 13.0, 16.5 (+3.5 a tick), 62.0, then exactly 63.333 |
| 6 | fully ducked, sprint kept from before the duck (`+speed` held throughout) | hold forward | ticks 1 / 10 / 11 | 16.0 / 106.0 / exactly 106.667 |
| 7 | holding `+walk` and `+duck`, fully ducked | forward | – | wish 190/3 = 63.33 (walk cancelled), same as 5 |
| 8 | airborne, horizontal 0 | hold forward | ticks 1 / 2 | 28.5 / 30 |
| 9 | airborne, v = (190, 0), surface friction 1 | right only (⟂) | 1 tick | √(190² + 28.5²) = 192.126 |
| 10 | as 9, sprinting (P 320) | right only | 1 tick | √(190² + 30²) = 192.354 |
| 11 | standing on ground, jump not held last tick | press jump | jump tick | move vz 151.0; z after 2.265; vz at end 146.5 |
| 12 | same | – | apex / landing | z 20.145 at the end of tick 17 (move vz 7.0); ground at the end of tick 34 with feet 1.275 up; fall speed 141.5, no landing effect |
| 13 | ducked flag set, on ground | press jump | – | move vz 155.5, end of tick 151.0; apex 21.33 (end of tick 18); ground at the end of tick 35 |
| 14 | standing jump | press duck on tick 10 | – | tick 10: feet +36 (z 15.525 → 51.525 before the move), box top 36, eye 28; feet peak 56.145 at the end of tick 17 |
| 15 | on ground | press duck and jump on the same tick | – | jump replaces vz (as 13); next tick feet +36; feet peak 57.33 |
| 16 | running 190, airborne after a running jump | hold forward | landing tick, next tick | 190 on landing; 190 the tick after (friction 178.6, +11.4): no slowdown |
| 17 | moving 600 horizontally on ground, jump not held | press jump | jump tick | horizontal still 600 (no cap, no boost) |
| 18 | moving backwards relative to the view at 400, on ground, nothing held | press jump | jump tick | horizontal 400 (no back-hop gain; the boost needs a one-slot server) |
| 19 | `player_limit_jump_speed` 0 vs 1 | rows 17 and 18 | – | identical |
| 20 | one ideally strafed running hop from 190 | perpendicular wish every tick | ticks 1–34 | landing speed √(190² + 16 194.23) = 228.68 |
| 21 | standing, ground | press and hold duck | tick 13 / 26 / 27 after the press | eye 46.675 / standing box, eye 28.066 / ducked box, eye 28 |
| 22 | ducked, ground, room | release duck | tick 7 / 13 / 14 after | eye 47.349 / 63.934 (still ducked box) / standing box, eye 64 |
| 23 | aux 100, at rest | hold forward + `+speed` | ticks 1 / 2 / n | max 320; aux 100 / 99.625 / 100 − 0.375(n − 1); `HL2Player.SprintStart` on tick 1 only |
| 24 | aux 100, standing still | hold `+speed` 67 ticks (1 s) | – | aux 100, sprint on, no recharge |
| 25 | sprinting, aux 50, moving | release `+speed` | that tick; 33 / 34 ticks later | aux 47.5 (speed selection already set P 190, so the device goes off with its 2.5 cost and that tick has no drain); 47.5 still / first +0.1875 |
| 26 | sprinting from 100 at rest, holding forward + `+speed` | – | tick 267 / 268 / 269 | aux 0.25 / 0 / sprint off, M 190 |
| 27 | aux 9.9 | press `+speed` | – | no sprint, `HL2Player.SprintNoPower` once, M 190; holding on while aux recharges past 10: still no sprint |
| 28 | fully ducked | press `+speed` | – | no sprint, no sound; standing up while holding: no sprint |
| 29 | `+walk` held, then `+speed` pressed (aux ≥ 10) | – | – | M 320 (sprint beats walk); `+speed` held, then `+walk` pressed: still 320 |
| 30 | test ladder; on the floor at (−20, 0, 0), not on a ladder | hold forward | tick 1 | cone mount (d 21.54, φ 0): `Ladder.StepRight`, `OnPlayerGotOnLadder`; forced move to (0, 0, 8), T = 0.11337 s |
| 31 | same | – | ticks 2–8 | origin at fractions 0.1323, 0.2646 … 0.9262 of the way; tick 9: arrives, on the ladder, climbs: z 11 |
| 32 | same, `+speed` held (M 320) | – | – | arrives 5 ticks after the mount tick (T 0.06731) |
| 33 | on the floor at (−20, 0, 0) | hold forward + right | tick 1 | no mount (strafing, not looking along) |
| 34 | same, pitch −30 | hold forward + right | tick 1 | mounts (along: 0.5 > 0.4, φ 0) |
| 35 | on the floor at (−10, 0, 0), yaw 90, pitch −30 | hold forward | tick 1 | cone fails (φ 90°, d 12.81); end-point mount (within 16 of the bottom, F·up 0.5) |
| 36 | on the floor at (−50, 0, 0) | press `+use` | – | mounts (d 50.64 ≤ 64); arrives 18 ticks later (T 0.26651); at yaw 180: no mount; at yaw 170 no mount, but with `sv_ladder_useonly` 1 it mounts (dot −0.985 ≥ −0.99) |
| 37 | `sv_ladder_useonly` 1, as 30 | hold forward | – | no mount; the hint shows once |
| 38 | on the ladder at (0, 0, 128) | forward at pitch 0 / 17 / 18 / −89 | 1 tick | z 131 / 131 / 125 / 131 |
| 39 | same | back at pitch 0 / −17 / −18 | 1 tick | z 131 / 131 / 125 |
| 40 | same | right only; forward + right at pitch 20; forward only at pitch 20; forward + back; none | 1 tick | 131; 131; 125; 128 (v 0); 128 (v 0) |
| 41 | same, yaw 180 | press jump, no keys | that tick | v = (−200, 0, 50) → walking air move: x −3.0, z 128.6825, end v (−200, 0, 41) |
| 42 | same, yaw 0 | back + jump | that tick | as 41 (−F, +50); at pitch 30 down: v before the move (−173.205, 0, 100), end vz 91 |
| 43 | same, yaw 180, no node within 100 (D is 144 away) | press `+use`, no keys | that tick / next | leaves the ladder, v = (−100, 0, 50), no move this tick; next tick: x −1.5, z +0.6825, end vz 41 |
| 44 | climbing from z 11 (row 31), forward held, pitch 0 | – | 84 ticks after arrival | z 263 (within 3 of the top): automatic dismount to D's spot (48, 0, 265): score 1, T = 48.0416/190 = 0.25285 s, arrives 17 ticks later standing on the ledge, velocity 0 |
| 45 | as 44 without D | – | – | z 263, next tick 266 would leave (> 265): put back to 263, stays; at pitch −70 (along) with D present: no automatic dismount, stays at 263 |
| 46 | on the ladder at (0, 7, 128) (moved off the line) | no keys / forward | 1 tick | stays attached / leaves the ladder (7 > 6) and falls this tick |
| 47 | on the ladder, base velocity (0, 5, 0) | – | 1 tick | leaves the ladder |
| 48 | dm_lockdown's ladder-contents brushes (contents ladder + grate + translucent) | walk into one, press forward and `+use` | – | blocked like a wall; never climbs |
| 49 | step off a ledge 128 high | – | landing | tick 44, fall 387: no damage, footstep 0.85, roll 5.031° |
| 50 | ledge 64 | – | landing | tick 31, fall 270: no effect at all |
| 51 | ledge 236.9 / 245 | – | landing | tick 59, fall 522: no damage, 0.85 / tick 60, fall 531: 10 damage (fixed), volume 1, roll 6.903° |
| 52 | ledge 512, `mp_falldamage` 0 / 1, armour 50 | – | landing | tick 87, fall 774: 10 / 62 health (62.5, 0.5 carried), armour 50 both; roll 10.062° / 3.2175° |
| 53 | ledge 719, `mp_falldamage` 1, health 100 | – | landing | tick 104, fall 927: 101.14: dead |
| 54 | landing at 600 at water level 1 / on a floating object | – | – | no damage, volume 0.5, roll 7.8° / 427: no damage, 0.85, roll 5.551° |
| 55 | citizen model running 190 on concrete from rest | hold forward | – | first step on tick 5 (tick-start speed 96 ≥ 90), `NPC_Citizen.RunFootstepLeft` at 0.2; then every 27 ticks, alternating Right, Left |
| 56 | sprinting 320 on concrete / dirt / vent | – | – | every 20 ticks at 0.5 / 0.55 / 0.7 |
| 57 | climbing a ladder at 200, standing box | – | – | a step every 30 ticks at 0.5, model entry (never a ladder surface sound) |
| 58 | fully ducked, moving at 63.33 on ground; jumping ducked; landing ducked at 400 | – | – | no step sound ever |
| 59 | water level 3, at rest, no move keys | hold jump | tick 1 / 2+ | vz 116.8 / 104.2 (lift +190, cap 152, surface friction 0.25 from tick 2) |
| 60 | water level 3, at rest | hold forward | ticks 1–3 / 9 | 22.8, 44.232, 64.378 / exactly 152 |
| 61 | water level 3, no input | – | ticks 1, 2 / 9 | vz −7.2, −13.968 / −48 |
| 62 | level 2, facing a ledge (movement.md's water-jump setup) | hold forward | trigger tick / at level 1 / end | vz 256 → 240.64; −4.5 a tick at level ≤ 1; ends at level 0 or on the 134th tick |

Probe cases (real game; Steam builds game 24600434, dedicated server
24600426; probe server as H0 Open questions 2, a SourceMod fake client):

| # | How | Expected |
|---|---|---|
| P1 | RCON at start: the cvars in "Cvars and configs", `sv_maxspeed`, `sv_stepsize`, `mp_falldamage`; on a client `cl_forwardspeed` | as listed; nothing changed by a config |
| P2 | standing jump on flat ground, z and vz every tick | move vz 151 − 9(k − 1); apex 20.145; grounded at the end of tick 34, feet 1.275 up |
| P3 | ducked jump | 155.5 first move; apex 21.33 |
| P4 | duck in the air; read the box and the view offset | feet up 36, box top 36, eye 28; standing top 72, eye 64 (H0 Open questions 4) |
| P5 | ducked ground acceleration from rest | 9.5, 13.0, 16.5 … (scale 1/3) |
| P6 | rows 1–4 | as listed |
| P7 | bunny-hop chains from 400 and 600, holding forward, holding back, holding nothing with the view reversed (back-hop setup), with `player_limit_jump_speed` 0 and 1; also try a listen server started with `maxplayers 1` (does `status` show 2?) | no cap, no gain, no loss; cvar without effect; no one-slot server possible (H0 Open questions 3) |
| P8 | rows 23–29 (aux via the HUD or a plugin reading the aux value) | as listed, incl. no drain on the first tick and while still |
| P9 | `+speed` held, then duck fully; then dive to level 3 | still sprinting: ducked top speed 106.67, aux draining; start sprint mid-duck-transition: allowed on the server |
| P10 | dm_resistance's ladder `ladder` (bottom (−450, 299, −1215), top (−450, 299, −947), nodes (−408, 299, −952), (−406, 299, −1215), (−494, 299, −1215), (−450, 343, −1215)): mount from 20 units at forward, climb, the pitch thresholds of rows 38–40, back at level view, strafe, jump-off velocities, automatic dismount at the top onto (−408, 299, −951) | as rows 30–45 |
| P11 | dm_lockdown's 10 ladder-contents brushes: walk into, shoot through | block players, bullets pass, never climbable |
| P12 | drops from measured heights (rows 49–54), both `mp_falldamage` values, armour 50 | as listed; health only whole points |
| P13 | footsteps (sound hook): rows 55–58; `mp_footsteps` 0 vs `sv_footsteps` 0 | as listed; `mp_footsteps` 0 changes nothing, `sv_footsteps` 0 silences |
| P14 | rows 59–62 | as listed |
| P15 | shoot a player during a ladder mount transit | Open questions 9 |
| P16 | a client (not the dedicated server): landing at 320 | does the local player hear a landing step and see a roll (client thresholds 350)? Open questions 8 |

## Open questions

1. **H0 test case 1, landing tick.** H0 says the standing jump is back
   on the ground 35 ticks after the jump; with movement.md's 2-unit ground
   probe it is the end of tick 34 (feet 1.275 above the floor). H0's peak
   (20.145) and end-of-tick vz (146.5) agree. P2 settles it.
2. **H0 test case 3, first sprint tick.** The drain reads the tick-start
   velocity, so from standing the first tick costs nothing (aux 100) and
   99.625 comes after the second. P8.
3. **Jump boost and back-hop in multiplayer** (H0 Open questions 3).
   The SDK applies the boost only on one-slot servers and HL2:DM's minimum
   is 2 slots; `player_limit_jump_speed` is never read. Whether the
   engine can still be made to run one slot, and whether the shipped
   server reads the cvar (it is in its library's strings), only P7 can
   tell.
4. **H0's "auto-mount … from the air or pressing forward"**: forward is
   needed in both cases (the cone and end-point tests require a positive
   forward move); in the air without forward, only `+use` mounts.
5. **H0's combine-ball ground rule** covers only balls in the
   hostile-NPC group; player-fired AR2 balls can be stood on as far as the
   movement code goes (H4 to confirm whether they collide with players at
   all).
6. **H0's "acceleration not blocked while water-jumping or dead"** has no
   effect on a living player's movement (see Always-allowed ground
   acceleration); it matters only for dead spectators (H6).
7. **H0's footstep notes**: `mp_footsteps` gates nothing (only
   `sv_footsteps`), and the waist-depth "skip" is a one-tick delay with a
   counter shared by all players. P13.
8. **Client fall thresholds.** The client is built without the HL2
   switch for these constants, so its prediction uses 350/580/200 while
   the server uses 303/526.5/173: the local player may hear no landing
   step and see the roll only after correction for falls of 303–350.
   Needs a client capture (P16); Chimera uses the server's numbers.
9. **Shootability during a forced move.** The player is not solid while
   being pulled on or off a ladder; whether bullets (hitbox traces)
   still hit them is not clear from the movement code. P15.
10. **Ladder "back climbs up".** The SDK's direction rule makes back at a
    level view climb up; this is unusual enough to confirm in P10 before
    implementing as a quirk.
11. **Spawn and the sprint state.** Spawning refills aux to 100 but the
    sprint state is not visibly reset; whether a player respawning with
    `+speed` held sprints at once is H2/H6's to check.
12. **Probe results (M3, 2026-11-03, dedicated server build 24600426,
    tools/hl2dm_probe, an `hl2mp_bot_add` bot whose commands the probe
    replaces).** Answered: rows 1-4 and P5 exactly (28.5 ... 190; walk
    and sprint likewise; friction 178.6/102.34/96.2/90.2/6.2/0); P2 the
    standing jump's first vz 146.5, apex 20.145 at the end of tick 17,
    grounded at the end of **tick 34** with the feet 1.275 up (Open
    question 1: 34, not H0's 35); P3 the ducked jump 151.0, apex 21.33 at
    tick 18, grounded at tick 35; P4 the air duck lifts the feet the whole
    36 (37.05 with the tick's move), eye 28 ducked and 64 standing (the
    probe's `m_vecMaxs` reads 60 ducked, the collision property's box:
    the movement hull's 36 is unconfirmed); row 21's eye 46.675 / 28.066 /
    28 at ticks 13 / 26 / 27; P7 a jump at 600 keeps 600 with
    `player_limit_jump_speed` 0 and 1 (no cap, the cvar does nothing;
    the back-hop at 400 wasn't measured: the run's bot didn't jump);
    P8 aux 100, 99.625, 99.25 ... from rest (Open question 2: the first
    tick drains nothing), a standing sprint drains nothing, the tap's
    97.125 and recharge 34 ticks after, running dry at tick 268; P9 a
    sprint kept ducked runs 16.0 / 106.0 / 106.667 and keeps draining;
    P10 on dm_resistance: the forced move at 190 (2.85 a tick), the
    mount 8 ticks later, the climb at exactly 3 units a tick, forward
    climbs up to 17° down and down from 18°, **back climbs up** at a level
    view and down from 18° up (Open question 10: confirmed), a strafe key
    climbs up, forward + right at 20° down up, forward at 20° down down,
    no keys hangs, jump-off at yaw 180 gives (-200, 0, 41) after the tick,
    the automatic dismount at the top onto (-408, 299, -951) when facing
    it and none when facing away; `Ladder.StepRight` plays on the mount
    tick at volume 1; P12 a fall of 526.5 lands without damage at 0.85, a
    fall of 544.5 costs 10 with a step at 1.0 and `Player.FallDamage`, 778.5
    costs 10, with `mp_falldamage` 1 health 38 (62 of 62.5), armour 50
    untouched; P13 running steps on ticks 5, 32, 59 at 0.2 (the bot's
    Combine model: `hardboot_generic*`), none ducked; P1 `sv_gravity`
    600, `sv_accelerate` 10, `sv_stopspeed` 100, `sv_friction` 4,
    `sv_airaccelerate` 10, `sv_maxspeed` 320, `sv_stepsize` 18, the
    `hl2_*` speeds 150/190/320, `mp_falldamage` 0, `sv_footsteps` and
    `mp_footsteps` 1, `sv_ladder_useonly` 0, `sv_autoladderdismount` 1,
    `xc_uncrouch_on_jump` 1. Not measured yet: P11 (dm_lockdown's ladder
    brushes), P14 (water), P15, P16.
13. **Probe results, round 2 (2026-11-03, same build).** The back-hop
    (test 18, H1): moving backwards at 376 relative to the view, nothing
    or back held, a jump keeps 376 (no gain); forward the same. P11:
    dm_lockdown's ladder-contents brushes (contents ladder + grate +
    translucent, 1 unit thick, 64 wide) block a player walking into them
    with forward and `+use` (stopped at the face), never climbed, jumping
    against them included; shooting through them wasn't settled (each one
    tried stands against other geometry). P15 needs two driven players
    (the probe drives one); P16 needs a game client.
