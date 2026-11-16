# Halo: Combat Evolved: player movement

Source basis: the game's biped update (input to throttle, turning,
ground and air movement, the pill mover's use, ground and landing
tests, jumping, crouching, falling damage), the player control and
input layers, the unit's aiming update, the game types' speed
multipliers and the multiplayer teleporter, read in a decompilation of
an Xbox build; every tag value checked against the Xbox disc's own map
files (`bloodgulch`, cross-checked identical on `hangemhigh`,
`prisoner`, `boardingaction` and `damnation`). The test numbers come
from a throwaway simulation of the rules below (not from the game).
Status: draft

## Summary

A Halo player is a vertical pill (radius 0.2 WU, 0.7 WU tall standing)
moved once per 30 Hz tick. On the ground its velocity is pulled toward
the wanted velocity (stick times speed, along the slope) by at most a
fixed step per tick, a linear ramp in 3D: full run speed, 0.075 WU per
tick (6.86 m/s), is reached from rest in 8 ticks and lost in 8. There
is no friction and no gravity while grounded; a small push into the
ground plane each tick keeps the pill in contact. In the air gravity
applies and the stick steers the horizontal velocity toward the wanted
one by only 1/9 of the ground rate, which also brakes a player who lets
go. A jump sets the vertical speed to at least 0.07 WU per tick: a
floaty 0.65 WU (2.0 m) hop lasting 39 ticks (1.3 s). There is no step
logic: the rounded bottom of the pill rides over edges up to about
0.12 WU, often with a small hop. Slopes up to 45 degrees are ground,
with speed scaled 1.25 downhill to 0.65 uphill; steeper ones are air.
Falls are judged only by the vertical speed at impact.

## Units and conventions

Distance in world units (WU, 3.048 m), +Z up, +X forward, +Y left,
angles in radians, the 30 Hz tick: all as `cache_file.md` defines.
Velocities in this spec are per tick (WU/t) unless marked WU/s or m/s;
accelerations are a change of velocity per tick (WU/t per tick, written
WU/t²). Conversions: WU/t × 30 = WU/s; WU/s × 3.048 = m/s.

- The biped's origin is the point between its feet (on the ground when
  standing). Its pill is described in "The pill".
- "Aim yaw" ψ is the yaw of the player's view. f = (cos ψ, sin ψ, 0) is
  the horizontal forward vector, l = (−sin ψ, cos ψ, 0) the left one.
- "Throttle" is (t_x, t_y): t_x forward (+1 full forward, −1 full
  back), t_y left (+1 full left, −1 full right).
- The collision queries (sphere query, feature shapes, the point sweep)
  are those of `collision.md`, "Moving a pill"; this spec says how
  movement uses them and adds what that section leaves out.
- ε = 0.0001 is the engine's "nearly zero" (the same as in
  `collision.md`).
- Tick numbering in the tests: tick 1 is the first tick with the input
  held; "after tick n" means the state stored at the end of tick n.
- The engine computes in 32-bit floats; the expected values below were
  computed in doubles from the 32-bit tag values. Tests should allow
  1e−6 WU on positions and velocities and match tick counts exactly.

## Constants

Values read from the disc (bloodgulch, identical on four other
multiplayer maps). "bipd" means the multiplayer Spartan
`characters\cyborg_mp\cyborg_mp` (the biped the globals' multiplayer
information block names as the multiplayer player); "matg" means
`globals\globals` (its layout is in `globals.md`); field names are the
tags' own.

| Name (ours) | Value | Unit | Meaning, source |
|---|---|---|---|
| GRAVITY | 0.0035651792 | WU/t² | engine constant (3.20866 WU/s², 9.780 m/s²); no tag scales it for bipeds |
| RUN_FORWARD | 2.25 | WU/s | matg player information `run forward speed`: 0.075 WU/t, 6.858 m/s |
| RUN_BACKWARD | 2.0 | WU/s | `run backward speed`: 0.0666667 WU/t, 6.096 m/s |
| RUN_SIDEWAYS | 2.0 | WU/s | `run sideways speed`: 0.0666667 WU/t, 6.096 m/s |
| RUN_ACCEL | 0.32 | WU/s per tick | `run acceleration`: 0.0106667 WU/t² (9.6 WU/s², 29.3 m/s²) |
| SNEAK_FORWARD | 0.9 | WU/s | `sneak forward speed`: 0.03 WU/t, 2.743 m/s |
| SNEAK_BACKWARD | 0.65 | WU/s | `sneak backward speed`: 0.0216667 WU/t, 1.981 m/s |
| SNEAK_SIDEWAYS | 0.6 | WU/s | `sneak sideways speed`: 0.02 WU/t, 1.829 m/s |
| SNEAK_ACCEL | 0.16 | WU/s per tick | `sneak acceleration`: 0.0053333 WU/t² |
| AIR_ACCEL | 0.035 | WU/s per tick | `airborne acceleration`: 0.00116667 WU/t² (1.05 WU/s², 3.2 m/s²) |
| WALKING_SPEED | 0.512 | WU/s | `walking speed`; only in the "alert" stance, never reached by a player in a match |
| DOUBLE_SPEED | 1.25 | factor | `double speed multiplier`; a debug option only |
| MP_SPEED_MULT | 0.0 | factor | `multiplayer only speed multiplier`; unused by the engine |
| STUN_MOVE | 0.8 | factor | `stun movement penalty` |
| STUN_TURN | 0.8 | factor | `stun turning penalty`: scales the jump only (see Damage stun) |
| STUN_JUMP | 1.0 | factor | `stun jumping penalty` (unused, see Quirks) |
| HARD_LAND_DECEL | 0.0053333333 | WU/t² | engine default step used while hard-landing |
| GROUND_PUSH | 1/128 = 0.0078125 | WU/t | push along −ground normal added every grounded tick |
| THROTTLE_DEAD | 0.1 | throttle | throttle magnitude below this is zero |
| CROUCH_STICK | 0.98 | throttle | controller crouch ignored on the ground at or above this |
| PITCH_LIMIT | 85.5 | degrees | view pitch clamp, both ways |
| JUMP_VELOCITY | 0.07 | WU/t | bipd `jump velocity` (stored per tick; 2.1 WU/s, 6.40 m/s) |
| JUMP_RECOVERY | 6 | ticks | grounded ticks after a jump before the next (timer > 5) |
| HEIGHT_STAND | 0.7 | WU | bipd `standing collision height` (2.134 m) |
| HEIGHT_CROUCH | 0.5 | WU | bipd `crouching collision height` (1.524 m) |
| RADIUS | 0.2 | WU | bipd `collision radius` (0.610 m) |
| CAMERA_STAND | 0.62 | WU | bipd `standing camera height` (1.890 m) above the origin |
| CAMERA_CROUCH | 0.35 | WU | bipd `crouching camera height` (1.067 m) |
| CROUCH_TIME | 0.2 | s | bipd `crouch transition time`; per tick 1/(0.2 × 30) = 1/6 (stored precomputed: 0.16666667) |
| MAX_SLOPE | 45° | radians 0.7853982 | bipd `maximum slope angle`; precomputed cosine 0.70710677 |
| UPHILL_FALLOFF | 20° | radians 0.3490658 | bipd `uphill falloff angle`; precomputed sine 0.34202012 |
| UPHILL_CUTOFF | 45° | radians 0.7853982 | bipd `uphill cutoff angle`; precomputed sine 0.70710677 |
| UPHILL_SCALE | 0.65 | factor | bipd `uphill velocity scale` |
| DOWNHILL_FALLOFF | 20° | radians | bipd `downhill falloff angle`; precomputed −0.34202012 |
| DOWNHILL_CUTOFF | 45° | radians | bipd `downhill cutoff angle`; precomputed −0.70710677 |
| DOWNHILL_SCALE | 1.25 | factor | bipd `downhill velocity scale` |
| SOFT_LAND_MIN | 1.5 | WU/s | bipd `minimum soft landing velocity` (0.05 WU/t) |
| HARD_LAND_MIN | 5.0 | WU/s | bipd `minimum hard landing velocity` (0.1666667 WU/t) |
| HARD_LAND_MAX | 8.0 | WU/s | bipd `maximum hard landing velocity` (0.2666667 WU/t) |
| SOFT_LAND_TIME | 0.6 | s | bipd `maximum soft landing time` (18 ticks) |
| HARD_LAND_TIME | 1.0 | s | bipd `maximum hard landing time` (30 ticks) |
| DEATH_LAND | 0.0 | WU/s | bipd `death hard landing velocity`; unused |
| TURN_SPEED | 5.893977 | rad/s | bipd `moving turning speed` (0.1964659 rad/t, 337.7°/s); body only |
| STATIONARY_TURN | 60° | radians 1.0471976 | bipd `stationary turning threshold`; precomputed cosine 0.5 (the campaign cyborg has 80°) |
| AIM_LIMITS | 0, 0 | rad/s, rad/s² | bipd (unit part) `aiming velocity maximum`, `aiming acceleration maximum`: zero, so aim is instant |
| FALL_LOWER | 3.0 | WU | matg falling damage `harmful falling distance` lower bound |
| FALL_UPPER | 6.0 | WU | its upper bound |
| FALL_MAX | 16.5 | WU | matg `maximum falling distance` |
| FALL_V_MIN | 0.14625688 | WU/t | stored precomputed √(2 g × 3.0) |
| FALL_V_MAX | 0.20683846 | WU/t | stored precomputed √(2 g × 6.0) |
| FALL_V_KILL | 0.34300280 | WU/t | stored precomputed √(2 g × 16.5) |
| FALL_DAMAGE | `globals\falling` | jpt! | damage minimum 10, lower and upper bound 155, category falling |
| FALL_KILL_DAMAGE | `globals\distance` | jpt! | 105 (minimum, lower, upper), category falling |
| MAX_CONTACTS | 16 | count | most contacts one mover call records |
| ELEVATOR_MEMORY | 60 | ticks | how long a touched elevator still carries the biped |

The three falling velocities on the disc equal √(2 × GRAVITY ×
distance) to 7 digits, which also confirms the gravity constant and
that the build tools converted the distances with it.

## The pill

The player collides as a vertical pill: a bottom sphere of radius r =
RADIUS whose centre is r above the origin, a top sphere of the same
radius, and the cylinder between them. With crouch fraction c (0
standing, 1 crouched):

  segment height h = HEIGHT_STAND + (HEIGHT_CROUCH − HEIGHT_STAND)·c
  − 2r

that is 0.3 WU standing and 0.1 WU crouched, so the pill spans z = 0
to 0.7 above the origin standing and 0 to 0.5 crouched. The mover's
point is the bottom sphere's centre, p = origin + (0, 0, r); after the
move the origin is p − (0, 0, r). Only player-controlled bipeds get the
segment; any other biped of this tag moves as a single sphere of
radius r (h = 0).

Other bipeds and objects are not pills to the mover: the mover meets
objects through their collision models (`collision.md`, the `coll`
tag), so a player stands on another player's collision model.

## Behaviour

### 1. Input

**Throttle.** The input layer turns the controller's left stick into
(t_x, t_y) in [−1, 1]² (the stick's dead zone and its diagonal snapping
are the input layer's and are out of scope here; the digital pad gives
±1). Then, in this order:

1. If t_x² + t_y² > 1, both are scaled by 1/√(t_x² + t_y²). So
   diagonals never exceed magnitude 1, and full forward plus full
   left becomes (0.7071068, 0.7071068).
2. In the biped's own update: if t_x² + t_y² < 0.1² = 0.01 the throttle
   becomes (0, 0). This dead zone is part of the simulation.

There is no diagonal speed bonus: a diagonal is slower than straight
forward (the forward and sideways speeds differ, see section 2).

**Keyboard and mouse.** A keyboard gives t_x = (forward key) −
(back key), t_y = (left key) − (right key), each 0 or ±1, then step 1
normalises diagonals. Mouse motion changes the view yaw and pitch
directly; the simulation receives only the resulting view angles, so
there is no turn-rate limit to emulate. The controller's look speed,
acceleration and auto-aim magnetism are input-layer features (out of
scope).

**Buttons.** Jump and crouch are levels, not edges: the biped tests
"jump held" and "crouch held" each tick. Holding jump therefore jumps
again as soon as it is allowed (section 3).

Controller crouch rule: on the ground, crouch held counts only while
the throttle magnitude is below CROUCH_STICK, t_x² + t_y² < 0.98². In
the air it always counts. On the Xbox the crouch button is the left
stick's click, and this rule means pushing the stick fully stands the
player up; to crouch-walk the stick is held just short of full. For a
keyboard the implementation should treat crouch as always counting
(there is no stick to rest at 0.97); this is our choice and gives a
crouch-walk 3% faster than the fastest on a controller.

**Aim.** The view yaw ψ and pitch θ are clamped to |θ| ≤ 85.5°. The
player's aim vector is the unit vector of (ψ, θ). The Spartan's unit
fields `aiming velocity maximum` and `aiming acceleration maximum` are
zero on the disc, so the aim is set to the wanted aim at once each
tick: no aiming lag. The movement direction uses the aim's yaw only
(the aim vector with its z set to 0 and normalised; a straight-up aim
falls back to +X).

**Body facing** (cosmetic: it drives the third-person body and the
animations, not the movement). While the biped is in a moving
animation state the body yaw turns toward ψ by at most TURN_SPEED/30 =
0.1964659 rad per tick and snaps to ψ when the step would overshoot.
While idle the body does not rotate by itself; when the aim is more
than STATIONARY_TURN (60°) from the body, a turn-left or turn-right
animation plays and its root rotation turns the body. When the aim is
more than about 154° behind (cosine below −0.9) the turn keeps its
current side.

### 2. Ground movement

Read each tick: throttle, aim yaw ψ, crouch fraction c, body stun s_b
(0 to 1, from damage: `health_shields.md`), the player's speed
multiplier m_p (1 unless the game type changes it:
`multiplayer_rules.md`), the stored ground plane normal n, velocity v.

**Speeds.** With u = 1 − c:

- if t_x > 0: F = RUN_FORWARD·u + SNEAK_FORWARD·c; else (t_x ≤ 0)
  F = RUN_BACKWARD·u + SNEAK_BACKWARD·c;
- S = RUN_SIDEWAYS·u + SNEAK_SIDEWAYS·c;
- A = RUN_ACCEL·u + SNEAK_ACCEL·c (all in WU/s or WU/s per tick).

Stun scale k_s = (1 − STUN_MOVE·s_b)·m_p. The wanted per-tick motion
in the body's frame is

  a = t_x·k_s·F/30 (forward), b = t_y·k_s·S/30 (left),

and the wanted speed is σ = √(a² + b²). The acceleration limit is
a_max = A/30 per tick. (The tag's speeds win for a player; the biped's
own `flying velocity` fields are for flying bipeds and its animation
root motion is ignored while the player physics flag is set.) A biped
without the player physics flag, which is every campaign character but
the Spartan, moves by its animations' root motion instead, even when a
player controls it: `characters.md`, section 4.

**Direction on the ground plane.** H = a·f + b·l (horizontal). If
n_z > ε the direction is

  d = normalise(H_x, H_y, −(H_x·n_x + H_y·n_y)/n_z),

the horizontal wish lifted onto the ground plane (d = 0 if H = 0).
(Ground planes with n_z ≤ ε only occur on climbable surfaces, section
8.)

**Slope speed scale.** With k = d_z (the sine of the path's climb):

- k ≤ −sin 45°: σ ← σ·DOWNHILL_SCALE (1.25);
- −sin 45° < k < −sin 20°: σ ← σ·(1 + (k + sin 20°)(1.25 − 1)/
  (sin 20° − sin 45°)), that is a linear ramp from 1 at 20° to 1.25 at
  45° downhill;
- k ≥ sin 45°: σ ← σ·UPHILL_SCALE (0.65);
- sin 20° < k < sin 45°: σ ← σ·(1 + (k − sin 20°)(0.65 − 1)/
  (sin 45° − sin 20°));
- otherwise unchanged.

The scale applies to the direction of travel, not the slope: crossing
a 40° slope sideways is level travel and keeps full speed.

**Acceleration.** Wanted velocity w = σ·d (a 3D vector along the
plane). Then

  Δ = w − v; if |Δ| > a_max then Δ ← a_max·Δ/|Δ|;
  Δ ← Δ − n/128;
  v_new = v + Δ.

The clamp is on the 3D vector, not per axis, and it is a constant
step (a linear ramp), not a fraction. There is no separate friction:
releasing the stick makes w = 0 and the same step brakes the player.
Reversing goes through zero at the same rate (from +0.075 to −0.0667
takes 14 ticks). There is no gravity on the ground (except while
slipping, section 8). The −n/128 term pushes the pill into the ground
so that the sweep meets it every tick (that contact is the ground
test, section 5); the sweep then removes it.

Results on flat ground, holding forward from rest: speed 0.0106667 ×
n per tick until it reaches 0.075 on tick 8 (7.03 ticks of ramp);
releasing from 0.075 stops on tick 8 after 0.226333 WU (0.69 m).

### 3. Gravity, jumping, air control

**Gravity.** Only in the air (and while slipping): v_z ← v_z − GRAVITY
each tick, before the move. There is no terminal velocity; very fast
falls kill in the air instead (section 6).

**Air control.** While airborne the wanted horizontal velocity is H
(as on the ground but without slope scale); the horizontal velocity
moves toward it:

  Δ_xy = H − v_xy; if |Δ_xy| > AIR_ACCEL/30 then Δ_xy ←
  (AIR_ACCEL/30)·Δ_xy/|Δ_xy|;
  v_xy ← v_xy + Δ_xy; v_z ← v_z − GRAVITY.

AIR_ACCEL/30 = 0.00116667 WU/t² is 1/9.14 of the ground rate. With no
input H = 0, so letting go of the stick in the air brakes the player
at that rate (a full-speed jump lands at 0.0295 WU/t instead of
0.075). Holding forward from a standing jump gains 0.0455 WU/t by
landing. The vertical velocity is not touched by air control.

**When a jump is allowed.** At the end of the tick's movement (after
the ground test and the landing check: Per-tick order, step 14), if
all of:

- the biped is grounded after this tick's move;
- it is not in a hard landing (section 6);
- the standing-room test did not fail (section 4: a crouched player
  under a low ceiling cannot jump);

then a jump timer counts up by one (saturating at 127; it starts at
127 when the biped is created) and, if jump is held and the timer is
now above 5, the jump happens.

So a jump is allowed on the first grounded tick of a biped that has
not jumped, and after a jump-and-land the next jump comes on the 6th
grounded tick: landing tick L counts 1, the jump fires on tick L + 5.
The timer only resets when jumping (walking off a ledge does not reset
it). Holding jump bunny-hops with a 5-tick ground pause; there is no
speed gain from it.

**The jump.** J = JUMP_VELOCITY·(1 − s_b·STUN_TURN). The velocity's
component along the biped's up vector (always +Z for this biped on the
ground) is raised to J if it is lower:

  v_z ← max(v_z, J);

horizontal velocity unchanged. It replaces, not adds: running up a
slope (v_z up to 0.034) gives no higher jump; a vertical speed already
above J (an elevator, an explosion) is kept. The biped becomes
airborne at once and its support surface is cleared. The jump takes
effect on the next tick's move; the take-off tick itself moved along
the ground as usual.

**The flight.** From flat ground the vertical motion after the jump
tick is v_z(n) = 0.07 − n·g and z(n) = 0.07·n − g·n(n+1)/2 (g =
GRAVITY), n counting the ticks after the jump tick: z(1) = 0.0664348,
apex after tick 19 at z = 0.6526160 WU (1.989 m, 93% of the standing
height), landing on tick 39 (z(38) = 0.0182022 above the floor; tick
39's sweep hits it). Flight 39 ticks = 1.3 s. A run at full speed
covers 2.925 WU (8.92 m) from the end of the take-off tick to the
landing, 3.0 WU counted from the start of the take-off tick.

**Crouch-jumping.** Crouching in the air pulls the feet up: while
airborne and the crouch fraction changes by δ in a tick (|δ| > 0.01),
the move's velocity gets an extra (HEIGHT_STAND − HEIGHT_CROUCH)·δ =
0.2·δ on z for that tick only (removed again from the stored
velocity). A full crouch transition raises the origin by 0.2 WU over 6
ticks while the top of the pill stays put. Jump and crouch on the
same tick: the feet reach 0.8526 WU (2.60 m) instead of 0.6526, and the
landing comes 3 ticks later (tick 42). Standing up in the air lowers
the feet by the same amount. A jump from an already crouched stance has
the normal apex.

### 4. Crouching

The crouch fraction c moves by CROUCH_RATE = 1/6 per tick toward its
target: 1 when the biped's stance is "crouch", 0 otherwise. When the
remaining distance is at most 1/6 it is set exactly to the target. A
full transition takes 6 ticks either way.

The stance is decided at the end of each tick from "crouch held" (as
section 1 counts it) or from a failed standing-room test, so the
fraction starts moving on the tick after the button: press on tick 1,
c = 1/6 after tick 2, c = 1 after tick 7. (The stance change can wait
while an uninterruptible animation plays; open question.)

While c > 0 the speeds blend (section 2): fully crouched, forward 0.03
WU/t, backward 0.0216667, sideways 0.02, acceleration 0.0053333 WU/t².

**Standing room.** When the biped is crouched (c > 0) and crouch is not
held, after the move the engine sweeps a point from the new
bottom-sphere centre straight up by HEIGHT_STAND − 2r = 0.5 WU against
the level and objects (features gathered with a sphere of radius
HEIGHT_STAND/2 = 0.35 centred 0.35 above that point, shapes of radius
r with no segment). Any hit means "cannot stand": the stance stays
crouch for the next tick, and no jump is processed this tick. Only
player bipeds are tested.

**Heights.** The pill uses the crouch fraction from the start of the
tick (Per-tick order, step 6). Camera: section 7.

**In the air** crouching is always allowed and lifts the feet
(section 3).

### 5. Collision response and the ground

**The move.** Each tick the bottom-sphere centre p is swept by the
tick's velocity V (v_new, plus the crouch lift on z in the air) with
the mover of `collision.md` ("Moving a pill"), with these details that
section leaves out:

- The features are gathered with the sphere of centre p + V/2 +
  (0, 0, h/2) and radius |V|/2 + h/2 + r (+0.0625 padding, as
  `collision.md` says).
- A shape counts as hit only if V'·N < −ε, where V' is the motion
  still to do and N the shape's contact normal.
- Each hit: the point moves to the hit point and is then projected
  onto the hit plane (the contact plane pushed out by r), the
  remaining motion is scaled by (1 − t) and its component along N
  removed. With an earlier plane still in the clip set, if the new
  motion still goes into it (V'·N_old < −ε) the motion is restricted
  to the two planes' intersection line (position projected onto the
  line); with a third, the point stops at the three planes' common
  point. The clip set keeps the newest plane plus those used. In
  detail, per contact: the remaining motion V' (the tick's velocity
  scaled by the product of the (1 − t) so far, never the already
  clipped one) is clipped to the new plane to give the candidate
  motion. Then, if the clip set is not empty: if the candidate goes
  into the first stored plane (candidate·N_first < −ε), the pair
  (new, first) gives the line, and V' (not the candidate) is projected
  onto it, the position too; and if a second stored plane exists and
  that line motion goes into it, the point is the common point of the
  three planes and the motion zero. Otherwise, if a second stored
  plane exists and the candidate goes into it, the pair (new, second)
  gives the line the same way. Otherwise the clip set is just the new
  plane. The line's direction is e = N_new × N_other.
- The loop ends when every component of the remaining motion is below
  ε, when the sweep hits nothing (the point moves the rest of the
  way), or after 16 contacts (the rest of the motion is dropped).
- The velocity stored afterwards is the tick's full V clipped by the
  final clip set: V minus its component along the last plane's normal
  (one plane), V projected on the line (two), or zero (three). There is
  no restitution: nothing bounces.
- Extra virtual contact for two or three clip planes (only when fewer
  than 16 contacts were recorded). Let the steepest plane be the one
  of the clip set with the lowest negative n_z (none if no plane
  faces down).
  - Two planes (N_new the newest plane's normal, N_old the other,
    c = N_new·N_old): if the steepest is the newest plane, the
    virtual normal is N_old − c·N_new (the other plane's normal made
    perpendicular to the steepest one's), normalised; if the steepest
    is the older plane, it is N_new − c·N_old, normalised. (This is
    e × N_new, respectively N_old × e, with e = N_new × N_old, so the
    vector lies in the steepest plane and is perpendicular to the
    crease: "keeping the steepest plane's orientation" means exactly
    these two signs, not either sign.) Worked example: N_new =
    (−0.8, 0, 0.6) (a slope facing up), N_old = (0.6, 0, −0.8) (an
    overhang, the steepest, n_z = −0.8): c = −0.96, N_new − c·N_old =
    (−0.224, 0, −0.168), normalised (−0.8, 0, −0.6): the virtual
    contact faces down and sideways. If no plane faces down, the
    virtual normal is +Z made perpendicular to e (+Z − e·e_z/|e|²),
    normalised: for two planes whose crease is horizontal, +Z.
  - Three planes: +Z made perpendicular to the steepest plane
    (+Z − n_z·N_steepest), normalised, or +Z itself if none faces
    down.
  If the normalisation fails the virtual contact is dropped. Its
  point is the last clip plane's contact point. This contact takes
  part in the ground test below.

**The ground test** (every tick, ground and air alike). Among the
tick's contacts (real and virtual), pick the one with the largest
normal z (the first if equal), except that a contact on a climbable
sheet beats the others (section 8, "Climbable surfaces"). The biped is grounded if that z is at
least the precomputed cos(MAX_SLOPE) = 0.70710677; otherwise, or with
no contact at all, it is airborne. When grounded the contact's plane
becomes the stored ground plane (its normal n is used by the next
tick's ground movement) and the contact's surface the support surface;
the landing velocity is

  λ = −N·V

(V being the swept velocity of this tick, crouch lift included).

Consequences:

- The flip between grounded and airborne happens in the same tick as
  the move that found (or lost) the contact. There is no distance
  threshold and no downward probe: the ground push of 1/128 WU per
  tick is the only thing reaching for the ground.
- Walking off a ledge makes the biped airborne on the first tick
  whose sweep meets no floor, and that tick has already moved it down
  by 1/128 WU; the next tick adds gravity to v_z = −0.0078125.
- Running over a crest where the ground falls away faster than 1/128
  per tick of travel launches the player briefly (flat into a 20°
  slope at full speed: 12 airborne ticks).
- A floor steeper than 45° is not ground: the biped is airborne on
  it, gravity pulls it down and the mover slides it along the slope
  (air control still acts). Standing still on ground up to 45° does
  not slide at all (no gravity on the ground).
- At exactly 45° the normal's z equals the threshold to float
  precision: avoid it in tests.
- Two too-steep faces meeting in a V give a virtual +Z contact: the
  player stands in the crease and can walk along it (Quirks).

**Steps.** There is no step-up code. An edge in front of the pill is
met by the edge's rounded shape (a cylinder of radius r around the
edge, for the bottom-sphere centre). Hit below the centre's height, the
contact normal tilts up and the slide lifts the pill; if its z is at
least 0.7071 the contact is ground and the next tick's wish follows it
upward, otherwise the biped is airborne for a few ticks with the
upward velocity the clip gave it. Either way the pill usually leaves
the top of the edge with some upward speed, so stepping up at speed is
a small hop (Quirks). The highest edge climbed depends on the approach
speed; from the simulation of these rules: 0.1231 WU (0.375 m) at full
run, 0.113 from rest touching the edge, 0.087 at half stick, 0.076
crouch-walking. Edges at or above the bottom-sphere centre (0.2 WU)
are walls. Stepping down is falling (no snap down), with the crest
launch above. The camera follows the origin exactly: no smoothing of
steps.

**Ceilings.** A surface facing down meets the pill's top (its slab is
lowered by h, `collision.md`). Jumping into it clips v_z to zero (the
component into the ceiling is removed, no bounce) and gravity takes
over the next tick.

**Moving and other objects.** The mover sees objects' collision models
as obstacles at their current pose; it does not move the biped with
them. Standing on a vehicle carries nothing: a moving vehicle slides
out from under the player. Pushes by vehicles are the vehicle's
business (`vehicles.md`). The one exception is elevators: a machine
flagged as an elevator, touched by the biped's sweep in the last
ELEVATOR_MEMORY = 60 ticks, moves the biped's origin by its elevator
node's displacement each tick it moves. The biped's velocity is not
changed by that carry.

**Volatile contacts.** Any contact with a breakable surface or a
non-scenery object marks the tick "volatile" (the biped is then not
"at rest"); this only matters for object sleeping.

### 6. Landing and falling

**Landing.** Each grounded tick with λ > 0 runs the landing check
(λ is about 0.0078 on every ordinary grounded tick, from the ground
push; that is far below the thresholds). With v_s = SOFT_LAND_MIN/30 =
0.05, v_h = HARD_LAND_MIN/30 = 0.1666667, v_m = HARD_LAND_MAX/30 =
0.2666667:

- λ < v_s: nothing.
- v_s ≤ λ < v_h: soft landing. q = clamp((λ − v_s)/(v_h − v_s), 0,
  1); recovery ticks R = trunc(SOFT_LAND_TIME·30·q) = trunc(18q).
- λ ≥ v_h: hard landing. q = clamp(λ/(v_m − v_h), 0, 1); since λ/0.1
  is always above 1.67 here, q = 1 and R = 30 (Quirks).

The landing state, its counter set to 0 and R are stored (a new
landing replaces an old one). Then each tick the biped is grounded and
in a landing state, after the move, the counter goes up by one; when
it reaches R the state ends. The landing tick itself counts.

Effects: a soft landing only selects the soft landing animation (and
the landing sound on its 2nd tick): movement and jumping are normal. A
hard landing selects the hard landing animation and, while it lasts:
the wanted motion is zero, the acceleration limit is HARD_LAND_DECEL =
0.0053333 WU/t² on the ground and 0 in the air, jumping is blocked
and the jump timer does not count. A hard landing from full run
therefore slides 15 ticks to a stop, and the player can move again on
the 30th tick after the landing tick. No camera dip is part of the
movement state (first-person landing motions are animation:
out of scope).

**Falling damage.** Each tick after the move, unless the biped is
immune (a unit flag scripts can set, or the biped tag's immune flag;
neither is set for players in a match):

1. If λ > FALL_V_MIN (0.14625688): damage with the effect
   `globals\falling` at scale q_f = clamp((λ − FALL_V_MIN)/(FALL_V_MAX
   − FALL_V_MIN), 0, 1). The damage system (`health_shields.md`)
   turns it into (1 − q_f)·10 + q_f·155 damage points (the effect's
   damage minimum, and its lower and upper bounds, both 155).
2. Otherwise, if v_z < −FALL_V_KILL (−0.3430028, after this tick's
   move) and the biped is not dead: damage with `globals\distance`
   (105 points) every such tick. This kills a player falling too fast
   before it lands.

Nothing in multiplayer turns this off. Only the vertical speed at
impact matters (onto a slope, the speed along the slope's normal),
never the height fallen. Falling from rest, the impact speed after k
ticks of fall is k·g, so:

| Impact on tick k (from rest) | λ (WU/t) | drop needed (WU) | result |
|---|---|---|---|
| ≤ 14 | ≤ 0.0499125 | ≤ 0.3743 | below soft: no landing state |
| 15 to 46 | 0.0534777 to 0.1639982 | 0.3743 to 3.8540 | soft landing, 0 to 17 ticks |
| 41 | 0.1461723 | 2.9234 to 3.0696 | soft (14 ticks), no damage (just under 0.14626) |
| 42 | 0.1497375 | 3.0696 to 3.2194 | soft (15 ticks), damage 18.33 (q_f 0.05745) |
| 47 | 0.1675634 | 3.8540 to 4.0215 | hard landing, damage 61.0 |
| 59 | 0.2103456 | 6.1000 to 6.3104 | hard, full 155 damage (from tick 59 on) |
| 97 and later, still falling | v_z ≤ −0.3458224 | more than 16.5995 | 105 damage in the air each such tick |

(The drop ranges are g·(k−1)k/2 < drop ≤ g·k(k+1)/2.)

### 7. Camera

The first-person camera is at the origin plus (0, 0, e) with

  e = (1 − c)·CAMERA_STAND + c·CAMERA_CROUCH,

0.62 WU standing, 0.35 crouched, linear in the crouch fraction (0.045
WU per tick of transition). It follows the origin exactly: no
smoothing of steps, no bob, no landing dip in the movement state.
The crouch lift in the air moves the origin and therefore the camera
too.

### 8. Other states in brief

**Explosions and other impulses.** A damage effect's instantaneous
acceleration reaches a living biped halved: v ← v + a/2. The biped
becomes airborne and "slipping", and its body turns to face the push.
Slipping persists through the air. On landing, while slipping, each
ground tick also subtracts GRAVITY from v_z (so it slides downhill)
and slipping continues as long as the ground acceleration is still
being clamped (the velocity is more than one step from the wanted
one); the first tick that is within one step clears it. Slipping never
starts any other way.

**Damage stun.** The body stun s_b (0 to 1; set and decayed by
`health_shields.md`: the globals' stun times, the damage effect's stun
values) has exactly these effects, and this paragraph is the one place
that states them (other specs refer here):

- the wanted ground speed is multiplied by (1 − STUN_MOVE·s_b), the
  `stun movement penalty` (0.8): the factor k_s of section 2, and
  nothing else of the movement (acceleration limit, air control, turn
  rate) changes;
- the jump velocity of a player-controlled biped is multiplied by
  (1 − STUN_TURN·s_b), the `stun turning penalty` (0.8; the name is
  the tag's, the jump is all it scales: it does not affect the turn
  rate). A biped without a player is not scaled. The tag's `stun
  jumping penalty` (1.0) is never read. Both penalties happen to be
  0.8 on the disc, so on the disc a stun of 1 leaves 0.2 of both;
- above s_b = 0.2 the stunned moving animations play. The unit tag's
  `stunned movement threshold` is 0 on the Spartan (off).

**Game-type speed.** The player speed multiplier m_p is set by the game
type (Oddball ball carrier 1.25 or 0.75, Race catch-up 1.1 or 1.2,
Slayer kill and death adjustments): `multiplayer_rules.md`. It scales
the wanted speed only, not acceleration or jump.

**Vehicles.** A biped in a seat does not run this movement at all: its
position is the seat's, its throttle drives the vehicle, and leaving
is the seat-exit sequence (`vehicles.md`). Out of scope here.

**Climbable surfaces (ladders).** Which maps have them is checked
against the disc (the level collision BSP of every multiplayer map, a
script over all 13): the climbable flag is set on 120 surfaces, which
are 60 rectangular two-sided sheets (each sheet is two surfaces, one
per face), on **8 maps**: boardingaction 80 surfaces (40 sheets, in
vertical stacks of 2.5 WU sheets), prisoner 8 (4 sheets), sidewinder 8
(4), wizard 8 (4), beavercreek 4 (2), damnation 4 (2), longest 4 (2)
and putput 4 (2). Blood Gulch, Hang 'Em High, Carousel, Chill Out and
Rat Race have none, and no object collision model on any multiplayer
map carries the flag. (A count of 9 maps is one too many; the totals
of 60 sheets and 40 on Boarding Action are right.) All are thin
vertical sheets 0.44 to 0.8 WU wide and 1.75 to 5 WU tall (a stack is
several coplanar sheets end to end). Their normals are horizontal to
within 0.0001, except on two Boarding Action ladders (at x = -2.47 and
x = 22.48) whose faces tilt by 0.0128 in z (one face up, its twin
down). The behaviour below is from the reference only; it was
simulated, not compared with the game.

How a biped uses them, all of it in the ground test and the ground
wish (sections 2 and 5):

1. **A ladder contact beats a floor contact.** A contact whose surface
   carries the climbable flag (its face, or an edge or corner of the
   sheet) is "walkable" while the biped is alive. The ground test of
   section 5 is then not simply "largest n_z". Go through the
   contacts in order, keeping one best. A walkable contact is skipped
   if its normal's horizontal part points along the wish direction by
   more than 0.5: n_x w_x + n_y w_y > 0.5, with w the unit horizontal
   direction of the throttle in the world (the (a, b) of section 2
   turned by the facing); w is the zero vector with no throttle, and
   also in the air, so in the air nothing is skipped. Otherwise a
   walkable contact becomes the best if there is no walkable best yet,
   or if the push into it, −N·(the tick's velocity before the sweep),
   is larger than the best's. A non-walkable contact becomes the best
   only while no walkable best exists and its n_z is larger than the
   best's (the first of equals). A walkable best makes the biped
   grounded whatever its n_z (the 0.7071 test applies only to a
   non-walkable best), and its plane becomes the ground plane. The
   landing velocity is −N·V as always.
2. **The foot.** Walking at a ladder from the floor, the sweep reports
   the floor (the ground push keeps it in the set), the sheet and the
   virtual crease contact on the same tick; the sheet wins, so at the
   end of the tick that first touches it the ground plane becomes the
   sheet's plane, a vertical wall. Nothing else happens on that tick:
   the velocity into the sheet is clipped away, and the landing
   velocity of a full run (0.075) is above the soft limit of 0.05, so
   a 3-tick soft landing plays, which does not affect movement. From
   the next tick the wall wish of item 3 applies. With the aim level
   that wish is zero, so the biped just stands against the ladder. No
   press "mounts" a ladder: touching it while the horizontal wish does
   not point away from it is enough. Leaving at the foot: a throttle
   pointing away from the sheet (within 60 degrees of its normal)
   makes the sheet skipped; the biped then has no contact on that
   tick, so it is airborne for one tick, lands on the floor on the
   next and walks off normally.
3. **The wall wish** (the stored ground plane has n_z ≤ ε = 0.0001).
   forward = the aim vector minus its component along n; left =
   (up × aim, normalised) minus its component along n; neither is
   renormalised. The wish is a·forward + b·left (a and b of section 2:
   at full forward throttle a = 0.075 WU/t, so the throttle scales
   the length); its z component is multiplied by 5; it is normalised
   only if its length is at least ε (a shorter one is used as it is,
   which gives almost no motion). The slope scale of section 2 then
   applies to the normalised z: at least 0.7071 gives 0.65, at most
   −0.7071 gives 1.25, with the ramps between. The velocity moves
   toward the wanted one by at most A/30 per tick in 3D, minus n/128,
   as on the ground. There is no gravity while a ladder holds the
   biped, so releasing the throttle brings it to rest in place: it
   does not slide down. A face whose n_z is above ε (the tilted sheets
   above, and the slanted contact normal of a sheet's top edge, item
   7) takes the ordinary ground wish of section 2 instead: the
   horizontal wish lifted onto the plane, which for so steep a plane
   is nearly vertical (pressing into it climbs at 0.65 speed whatever
   the pitch).
4. **The aim.** The wish uses the aim vector, not the body. Head-on at
   a ladder, any pitch up whose forward part survives the factors
   (sin(pitch) × a × 5 ≥ 0.0001: 0.015 degrees at full throttle) gives
   straight up, any pitch down gives straight down, because the factor
   5 dominates: looking 0.01 radian up and 0.5 radian up climb at the
   same speed; at level aim nothing moves. Turning the aim sideways
   adds a component along the sheet (cos(pitch) sin(yaw offset)
   against the vertical sin(pitch) × 5), and the climber drifts along
   the sheet while climbing (at 45 degrees of yaw offset and 0.5 radian
   of pitch the lateral speed settles at 0.0122 WU/t with 0.0472 up).
   The throttle's sideways part strafes along the sheet at the
   sideways speed (0.0667 WU/t at level aim).
5. **Speeds.** Straight up at full forward throttle: 0.04875 WU/t
   (0.075 × 0.65: 1.4625 WU/s, 4.46 m/s), reached in 5 ticks from
   rest (0.0106667, 0.0213333, 0.032, 0.0426667, then 0.04875).
   Straight down: up to 0.09375 WU/t (0.075 × 1.25), reached in 9
   ticks from rest. Crouching does not detach: the speeds blend toward
   the sneak ones as in section 2 (crouched at 0.9 of the stick: 0.0176
   WU/t up). At the bottom the floor stops a descent and the ladder
   still holds the biped at floor level; the top is item 7.
6. **What detaches.** (a) A wish direction within 60 degrees of the
   sheet's outward normal (n_h·w > 0.5), that is the throttle pushed
   back with the aim toward the sheet, or forward with the aim turned
   more than 120 degrees away from it: the sheet is skipped, there is
   no contact, and the biped is airborne from that tick and falls (also
   in mid-ladder). Strafing (n_h·w = 0) and releasing the throttle do
   not. (b) A jump: the jump rule of section 3 runs as on the floor
   (grounded, timer above 5), so v_z becomes 0.07 and the biped is
   airborne on the next tick. With no throttle it leaves: the
   horizontal velocity is 0, the sweep never touches the sheet again
   and gravity takes over. With the throttle into the sheet, air
   control presses it back against the sheet within a tick and the
   sheet takes it again (nothing is skipped in the air), so a jump
   with forward held only adds two quick ticks of 0.0664 and 0.0558
   before the climb at 0.04875 resumes. (c) Running out of sheet: item
   7, or off its side. A biped that is airborne and moves into a sheet
   (velocity into it above ε after one tick of air control) catches it
   from any direction; the wall wish then brakes its fall only at the
   acceleration limit (a catch at 0.268 WU/t down takes about 25
   ticks to stop, the aim level). Crouching never detaches.
7. **The top.** The sheet's top edge is a rounded convex edge like any
   (`collision.md`: a cylinder of radius r for the bottom-sphere
   centre, that is the origin plus 0.2), so contact lasts until the
   centre is above the top by about 0.04 to 0.09 WU (the sweep's 1/128
   push reaches an edge that is up to √(2·r/128) = 0.056 away; which
   tick is the last depends on the climb's phase, see the table in the
   tests). The last contact tick is often the edge's, with a slanted
   normal (n_z from 0.002 up to about 0.3), so the ground plane tilts
   and the next tick uses the ordinary ground wish; then a tick with no
   contact at all: the biped is airborne with its climbing speed
   (0.04875 up) and only air control (0.00117 per tick) on the
   horizontal velocity, and follows the free-fall arc of section 3
   (it rises about 0.31 WU more, over about 14 ticks) to land
   wherever the geometry puts it. A climber who keeps pushing into the
   wall therefore goes over the top into whatever is beyond: with a
   platform at the sheet's top height behind it, the simulation lands
   on the platform on the 25th tick after the last contact tick. Nothing holds it
   at the top (item 8).
8. **The edge between two sheets.** Two stacked coplanar sheets (the
   disc's stacks have 38 shared same-plane edges, and 8 more between
   sheets whose planes differ by 0.013 in n_z) need nothing special:
   an edge between two surfaces of one plane is not a feature
   (`collision.md`), and the sheets' contact regions overlap, so the
   contact is continuous across the seam. The reference also has a rule
   for a tick on which the sweep found **no contact at all** while the
   biped had a support surface (the surface of last tick's best
   contact): for each edge of that surface's polygon whose other
   surface is also climbable, take that neighbour's plane (normal N,
   offset d) if N·V > 0 (V the swept velocity, the push into the old
   plane included) and N·p − d > −r/2 = −0.1 (p the end point of the
   sweep); among those pick the one whose shared edge is nearest (the
   squared distance from p projected on the old surface's plane to the
   closest point of the edge, ends included). If that squared distance
   is at most (2r)² = 0.16 and N·V ≤ 1.6/30, and the point's distance
   from the plane pushed out by r, N·p − (d + r), is at most 0.1 in
   magnitude, then the point is moved along N to exactly r in front of
   the neighbour's plane, the velocity's component along N is set to
   −1/30 if it was above that, and that one contact (flags cleared,
   surface the neighbour) is the tick's only contact. Because its
   flags are cleared it is not "walkable": the biped counts as grounded
   on it only if its n_z is at least 0.7071; the point and velocity
   changes happen either way. On the disc this never matters: a
   climber's swept velocity includes the push into its own plane, so a
   coplanar neighbour has N·V < 0, and the sheet's back face is 0.2
   (more than 0.1) behind the point. The rule exists for a ladder that
   bends round a corner or a lip, which no multiplayer map has; an
   implementation may leave it out.

**Water.** None: bipeds have no swimming or wading. Water in a level is
a fog plane that affects rendering and projectiles; a biped walks on
the collision surface under it as anywhere else.

**Teleporters.** A multiplayer teleporter moves the player's origin to
the destination flag and turns the facing by (destination facing −
source facing). The velocity is left as it was, in world axes: it is
neither zeroed nor rotated with the facing (reference only; open
question); the ground plane, support surface and airborne flag are not
touched either, so the first tick after arriving still uses the source's
ground plane for the ground wish. The teleporter's own rules (radius
0.5 WU, re-arm after moving 1 WU from the arrival flag, the exit's
blocking test, the telefrag counter) are `multiplayer_rules.md`, 6.8.
Spawns and script teleports set the velocity to zero.

**Not in a match.** Flying bipeds, "climbs anything" bipeds, AI leaps,
cinematic and scripted animation movement, the debug double speed and
the debug cheats use other branches and are left out.

### 9. Animation state, in brief

Each tick, before the move, the wanted animation state is: idle when
the throttle is zero; otherwise one of move front, back, left, right by
the dominant throttle axis (|t_x| ≥ |t_y|: front if t_x ≥ 0, else back;
otherwise left if t_y ≥ 0, else right), using the stunned versions
when s_b > 0.2. After the move: airborne once it has been airborne for
more than 3 ticks (or if already in the airborne state); else land soft
or land hard while a landing state lasts; idle stances turn left or
right as section 1 says. The animations do not move a player (root
motion is ignored); footstep sounds follow their foot frames. The
stance (stand or crouch) is set at the end of the tick (section 4).

## Per-tick order

One tick of a player's biped on foot (not in a seat, alive), after the
player's input has been turned into the unit's controls:

1. Controls: throttle (normalised to magnitude ≤ 1), aim from the view
   angles (instantly, section 1), jump and crouch levels, the
   controller crouch rule applied with the airborne flag from the end
   of the previous tick.
2. The aim's yaw becomes the movement facing (z dropped, normalised).
3. Throttle dead zone: magnitude below 0.1 becomes zero.
4. Airborne tick counter: +1 if the stored state is airborne (capped
   at 127), else 0; same for a slipping counter.
5. Body turning (cosmetic, section 1).
6. Movement setup: the pill from the start-of-tick crouch fraction;
   speeds, wanted (a, b), acceleration limits (ground a_max, air
   AIR_ACCEL/30; or the hard-landing values). Animation move state.
7. Crouch fraction steps toward the stance (from the end of the last
   tick) by 1/6; if airborne and it changed by more than 0.01, the
   crouch lift 0.2·δ for this move.
8. Velocity: airborne → air control and gravity; else ground wish on
   the stored ground plane, slope scale, clamped step, ground push
   (and gravity when slipping).
9. Sweep the pill by the velocity (plus the crouch lift) through the
   level and objects; contacts, the virtual crease contact.
10. Ground test on the contacts: grounded or airborne, the new ground
    plane, support surface, landing velocity λ (a climbable sheet's
    contact wins over a floor's: section 8).
11. Standing-room test if crouched and crouch not held.
12. Store: origin = new point − (0, 0, r); velocity = the clipped
    velocity minus the crouch lift; airborne and slipping flags;
    ground plane. If the standing room failed, the stance stays
    crouch.
13. Landing check with λ (section 6).
14. Jump timer and jump, unless the standing room failed (section 3).
15. Falling damage (section 6).
16. Elevator memory update; "on ground" and "at rest" flags.
17. After the move: airborne animation, or the landing counter step,
    or slipping effects.
18. Melee and footsteps (out of scope).
19. Animation update: the stance becomes crouch if crouch was held (or
    standing room failed), else stand; this is the stance step 7 of
    the next tick uses.
20. Elsewhere in the same tick, elevators that moved carry the bipeds
    that touched them (section 5).

## Edge cases

- Releasing the stick and pressing the opposite way are the same
  linear ramp; the switch to the backward speed happens as soon as
  t_x ≤ 0, so pure strafing uses the backward table for its (zero)
  forward part: harmless.
- A diagonal back input (−0.7071, 0.7071) has speed 0.0666667 exactly
  (backward and sideways speeds are equal).
- Pressing jump in the air does nothing and is not buffered; holding
  it through the landing jumps on the 6th grounded tick.
- A jump on the first grounded tick after walking off a ledge and
  landing is allowed at once (the timer was not reset).
- The jump and the landing check run in the same tick: landing at
  λ ≥ 0.1667 with jump held never jumps (hard landing blocks), landing
  softer with jump held jumps on that tick only if the timer allows.
- Crouching while airborne near the ground: the crouch lift moves the
  feet up; standing up near the ground moves them down into the
  ground, where the sweep stops them (and may count as a landing).
- A ceiling lower than the standing pill above a crouched player keeps
  the stance crouched and blocks jumping.
- 16 contacts in one tick drop the rest of the motion (a "splatter"
  flag is raised for vehicles; unused for a walking player).
- A biped's z below −2000 WU outside the map is deleted outside a game
  type; in a match the player dies to the fall speed first.

## Quirks

Keep all of these; they are part of how the game plays.

- **Hard landings always last the full 30 ticks.** The soft branch
  measures the velocity from its lower bound, the hard branch does not
  (λ/(v_m − v_h) with λ ≥ v_h is always above 1), so q is clamped
  to 1.
- **Jump stun uses the turning penalty.** The jump is scaled by the
  `stun turning penalty` STUN_TURN (0.8), not by the tag's `stun
  jumping penalty` (1.0, which would make a fully stunned player unable
  to jump). Keep 0.8: a fully stunned player still jumps at 0.2 × 0.07
  = 0.014 WU/t.
- **Letting go in the air brakes.** Air control steers toward the
  stick's wish, which is zero without input.
- **Hops over edges.** Small edges and crests launch the player a
  little (a 0.03 WU lip at full run: 12 airborne ticks rising to
  0.099 WU). This is the feel of running over Halo's uneven terrain.
- **Crouch is cancelled by a fully pushed stick on the ground**
  (controller). Crouch-walking needs a slightly relaxed stick.
- **Crease standing.** Two slopes too steep to stand on, meeting in a
  V, hold the player up through the virtual contact (simulated with
  two 60° faces: grounded, strafes along the crease at full speed).
- **Crouch-jump** adds 0.2 WU to the height the feet reach.
- **No terminal velocity, but a death speed.** Faster than 0.343 WU/t
  down (about 16.6 WU of free fall) kills in the air.
- **Teleporters keep world velocity** (reference only).
- **First-tick fall.** The tick that walks off a ledge already moves
  the biped 1/128 WU down.
- **A ladder is climbed by looking up.** The wish on a vertical sheet
  comes from the aim, with its z multiplied by 5: any look up at all
  climbs at full speed, level aim stands still, any look down descends
  at 1.25 times run speed, and walking backward off a ladder (or
  forward looking away) lets go.

## Test cases

Common setup unless stated: the flat floor is the plane z = 0 (normal
+Z, infinite); the biped stands with its origin at (0, 0, 0), at rest,
grounded on the floor, crouch 0, stance stand, jump timer 127, no
stun, m_p = 1, aim yaw 0 (facing +X). Inputs are held from tick 1. A
"settle" of 3 idle ticks before tick 1 changes nothing on flat ground.
Speeds are horizontal |v_xy| in WU/t unless stated.

### Ground speed

| Setup | Input | After | Expected |
|---|---|---|---|
| flat | throttle (1, 0) | tick 1 | speed 0.0106667; x = 0.0106667 |
| flat | (1, 0) | tick 2 | speed 0.0213333; x = 0.032 |
| flat | (1, 0) | tick 5 | speed 0.0533333; x = 0.16 |
| flat | (1, 0) | tick 7 | speed 0.0746667; x = 0.2986667 |
| flat | (1, 0) | tick 8 | speed 0.075 (top; 2.25 WU/s, 6.858 m/s); x = 0.3736667 |
| flat | (1, 0) | tick 10 | 0.075; x = 0.5236667 |
| flat | (1, 0) | tick 30 | 0.075; x = 2.0236667; z = 0, grounded every tick |
| flat, at 0.075 forward | throttle (0, 0) | ticks 1, 2, 6, 7, 8 | 0.0643333, 0.0536667, 0.011, 0.0003333, 0 |
| same | (0, 0) | until stopped | distance 0.2263333 WU (0.690 m), stopped on tick 8 |
| flat, at 0.075 forward | (−1, 0) | ticks 7, 8, 13, 14 | v_x 0.0003333, −0.0103333, −0.0636667, −0.0666667 (top) |
| flat | (−1, 0) | tick 30 | v = (−0.0666667, 0, 0) (6.096 m/s) |
| flat | (0, 1) | tick 30 | v = (0, 0.0666667, 0) |
| flat | (1, 1) (normalised to 0.7071068 each) | tick 30 | v = (0.0530330, 0.0471405, 0), speed 0.0709558 (6.488 m/s) |
| flat | (−1, 1) | tick 30 | speed 0.0666667 |
| flat | (0.5, 0) | tick 30 | 0.0375 |
| flat | (0.09, 0) | tick 30 | 0 (dead zone) |
| flat, aim yaw 90° | (1, 0) | tick 30 | v = (0, 0.075, 0) |

### Crouch

| Setup | Input | After | Expected |
|---|---|---|---|
| flat | crouch held, no throttle | ticks 1, 2, 4, 7 | c = 0, 1/6, 1/2, 1; eye height 0.62, 0.575, 0.485, 0.35; pill segment h 0.3, 0.2667, 0.2, 0.1 (at the start of the next tick) |
| flat, crouched | crouch released | ticks 1, 2, 7 | c = 1, 5/6, 0 |
| flat | controller (0.97, 0) + crouch | tick 40 | crouched, speed 0.0291 (2.661 m/s) |
| flat | controller (1, 0) + crouch | tick 40 | not crouched (c = 0), speed 0.075 |
| flat | keyboard (1, 0) + crouch | tick 40 | c = 1, speed 0.03 (2.743 m/s) |
| flat | keyboard (−1, 0) + crouch | tick 40 | 0.0216667 |
| flat | keyboard (0, 1) + crouch | tick 40 | 0.02 |
| flat with a ceiling plane at z = 0.6 (facing down), crouched for 8 ticks | release crouch, hold jump | 10 ticks | standing room fails every tick: c stays 1, stance crouch, never jumps, grounded |

### Jumping and air control

| Setup | Input | After | Expected |
|---|---|---|---|
| flat | jump on tick 0 only | tick 0 | still grounded at z = 0 during the move; stored v_z = 0.07, airborne |
| same | nothing | ticks 1, 2, 3, 5 | z = 0.0664348, 0.1293045, 0.1886089, 0.2965223 |
| same | nothing | ticks 18, 19, 20 | z = 0.6503544, 0.6526160 (apex, 1.989 m), 0.6513124 |
| same | nothing | tick 38 | z = 0.0182022, v_z = −0.0654768 |
| same | nothing | tick 39 | landed: z = 0, grounded, λ = 0.0690420, soft landing of 2 ticks |
| flat | jump held every tick | ticks 0 to 100 | jumps on 0, 44, 88; lands on 39, 83 |
| flat, at 0.075 forward | jump + (1, 0) on the take-off tick, then (1, 0) | landing | on tick 39 after take-off; x gained from the take-off tick's start 3.0, from its end 2.925 |
| flat, at 0.075 forward, jumped | (−1, 0) in the air | ticks 1, 10, 20, 30 | v_x 0.0738333, 0.0633333, 0.0516667, 0.04; lands tick 39 at 0.0295 |
| same | (0, 0) in the air | tick 39 | v_x at landing 0.0295 (same as reversing: both clamp) |
| flat, standing jump | (1, 0) in the air | ticks 1, 10, 30, 39 | v_x 0.0011667, 0.0116667, 0.035, 0.0455; x = 0.91 at landing |
| flat | jump + crouch on tick 0, crouch held | ticks 1, 2, 6 | z = 0.0997682, 0.1959711, 0.5451312; apex z = 0.8526159; lands tick 42 |
| flat, crouched 8 ticks | jump, crouch held | flight | apex z = 0.6526160, lands tick 39 |
| ceiling plane z = 1.0 facing down | jump | ticks 5, 6, 7 | z = 0.2965223, 0.3 (head at the ceiling, v_z = 0), 0.2964348 |
| flat | jump with body stun 0.5 | tick 0 | v_z = 0.042 |

### Slopes

An infinite plane through the origin rising toward +X at angle α; the
biped starts airborne at the origin and settles onto it.

| Setup | Input | After | Expected (speed along the slope, |v|) |
|---|---|---|---|
| α = 10°, facing up | (1, 0) | tick 40 | 0.075; v_z = 0.0130236 |
| α = 10°, facing down (yaw 180°) | (1, 0) | tick 40 | 0.075; v_z = −0.0130236 |
| α = 20°, up / down | (1, 0) | tick 40 | 0.075 both (edge of the falloff) |
| α = 30°, up | (1, 0) | tick 40 | 0.0636411 (5.819 m/s), horizontal 0.0551148 |
| α = 30°, down | (1, 0) | tick 40 | 0.0831135 (7.600 m/s) |
| α = 40°, up | (1, 0) | tick 40 | 0.0533746 (4.881 m/s) |
| α = 40°, down | (1, 0) | tick 40 | 0.0904467 (8.270 m/s) |
| α = 30°, any | (1, 0) | ticks 1, 2 | 0.0106667, 0.0213333 (same ramp) |
| α = 40° | no input | 5 ticks | no movement at all, grounded |
| α = 46° | any | any | airborne every tick, sliding down |

### Steps and edges

Floor z = 0 for x < 0.6; a block for x ≥ 0.6 with a vertical wall at
x = 0.6 and its top at z = s; all infinite in y; start at the origin
at rest and hold (1, 0).

| Setup | Input | After | Expected |
|---|---|---|---|
| s = 0.10 | (1, 0) | tick 8 | x = 0.3736667, speed 0.075, not yet touching |
| s = 0.10 | (1, 0) | tick 9 | first touch of the edge (contact normal (−0.866, 0, 0.5)); still grounded through the floor contact; x = 0.4312763, z = 0.0077620, v = (0.0153671, 0, 0.0266166) |
| s = 0.10 | (1, 0) | ticks 10 to 14 | airborne (the edge contact alone is too steep); tick 14 z = 0.0802655 |
| s = 0.10 | (1, 0) | ticks 15, 16 | grounded on the edge's rounded contact (normal z 0.903, 0.947) |
| s = 0.10 | (1, 0) | ticks 17 to 19 | airborne, top z 0.1058883 on tick 18 |
| s = 0.10 | (1, 0) | tick 20 | grounded on the top: x = 0.7338624, z = 0.1, speed 0.0452953; 0.075 again on tick 23 |
| s = 0.12 | (1, 0) | tick 40 | on the top, z = 0.12, grounded, speed 0.075 |
| s = 0.13 | (1, 0) | tick 9 | first touch: x = 0.415832, z = 0.0085163, v = (0.0066261, 0, 0.0177342), grounded (floor and the edge's rounded contact, normal (−0.937, 0, 0.35)) |
| s = 0.13 | (1, 0) | ticks 10 to 21 | airborne on the edge contact alone: apex z = 0.0455154 and x = 0.4367284 on tick 15, back at x = 0.4161609, z = 0.0096103 on tick 21 (the hop is the clip's upward slide, then gravity brings it back) |
| s = 0.13 | (1, 0) | tick 22 | the floor is hit again: x = 0.4108436, z = 0, v = (−0.0047308, 0, 0), grounded |
| s = 0.13 | (1, 0) | ticks 23 to 40 (and on) | blocked: origin x = 0.6 − √(0.2² − 0.07²) = 0.4126501, z = 0, v = 0 exactly, grounded; each tick's contacts are the floor, the edge and the virtual +Z contact |
| s = 0.15 | (1, 0) | tick 9 | first touch: x = 0.4079286, z = 0.0061106 |
| s = 0.15 | (1, 0) | ticks 10 to 14 | airborne (apex z = 0.0129625 on tick 11) |
| s = 0.15 | (1, 0) | ticks 15, 16 on | tick 15: x = 0.405607, z = 0, v = (−0.0008787, 0, 0), grounded; from tick 16 blocked at x = 0.4063508 (= 0.6 − √(0.2² − 0.05²)), v = 0 |
| s = 0.03 | (1, 0) | ticks 10, 11 | grounded on the edge contact; v = (0.0506893, 0, 0.0314144), then (0.0540089, 0, 0.0227750) |
| s = 0.03 | (1, 0) | ticks 12 to 23 | airborne hop, apex z = 0.0987121 on tick 17 |
| s = 0.03 | (1, 0) | tick 24 | grounded on the top: x = 1.5113295, z = 0.03, speed 0.075 |
| ledge: floor z = 0 for x < 1.0, ground z = −5 beyond | (1, 0) | tick 18 | airborne; z = −0.0078125, v_z = −0.0078125 |
| two faces at 60° forming a V along y (crease at z = 0) | released from z = 1 | 80 ticks | grounded (virtual contact, normal +Z), origin z = 0.2 |

Why the blocked rows end at speed exactly 0: once the sphere is down
on the floor again and pressed into the edge's rounded contact (a
cylinder around the edge), both planes are in the clip set; the stored
velocity is the tick's velocity projected on their common line, which
runs along y, so its x and z are 0, and the position is projected onto
the same line, which holds it at the stated x. The ground push
(1/128 along the ground normal) is what keeps the floor plane in the
set each tick; the edge's slide upward (the hop of ticks 10 to 21) only
happens while the floor plane is not in the set, as on the first touch.
The rows were produced by a simulation of the clip rules above
(its last digits may differ in single precision); the same simulation climbs 0.12 and blocks 0.125
and every higher step (full-run limit 0.1231).

The step rows depend on the full mover; an implementation using a
physics engine's capsule cast instead (`collision.md` allows it for a
first playable) should still pass the 0.10 / 0.13 rows and may differ
in the hop details.

### Ladders

A free-standing two-sided vertical sheet in the plane x = 0, its
outward normal +X (the biped's side), 1 WU wide (y from −0.5 to 0.5),
from z = 0 to its height H, over the flat floor z = 0 (the sheet's
twin face, normal −X, exists but is never touched). The biped starts
at origin (0.5, 0, 0), settled on the floor, aim yaw 180 degrees
(looking along −X, into the sheet), pitch +0.5 radian (up), throttle
(1, 0) held from tick 1. H = 2.5 unless stated. Edge cylinders on the
sheet's four edges, as `collision.md` builds them (side edges only
matter off the centre line).

| Tick (k = ticks after the first sheet contact) | Expected |
|---|---|
| 1 to 7 | on the floor, speed 0.0106667 n; x falls to 0.2013333 on tick 7 |
| 8 (k = 0) | sheet contact at x = 0.2 (the contacts are the floor, the sheet and the virtual contact; the sheet is chosen), velocity 0, ground plane (1, 0, 0), grounded, landing velocity 0.075 |
| 9 to 13 (k = 1 to 5) | v_z = 0.0106667, 0.0213333, 0.032, 0.0426667, 0.04875; z = 0.0106667, 0.032, 0.064, 0.1066667, 0.1554167 |
| k > 5 | v_z = 0.04875; z = 0.1554167 + 0.04875 (k − 5) (k = 10: 0.3991667; 20: 0.8866667; 40: 1.8616667); x stays 0.2 |
| k = 50 (tick 58) | last supported tick: contact is the top edge, normal (1, 0, 0.0021), z = 2.349183 |
| k = 51 (tick 59) | airborne, z = 2.397916, v = (−0.007915, 0, 0.048734); no contact |
| k = 52 onward | free arc: z peaks at 2.7070 on tick 72 (x = −0.017: it has passed over the sheet) and falls |

The tick at which the sheet is lost depends on H and the climb's
phase; the first airborne tick (and k) in the same setup, to within one
tick for an implementation whose edge shapes differ in the last 1/128:

| H (WU) | 0.4 | 0.6 | 1.0 | 1.75 | 2.0 | 2.5 | 3.0 | 5.0 |
|---|---|---|---|---|---|---|---|---|
| first airborne tick (k) | 16 (8) | 20 (12) | 28 (20) | 43 (35) | 49 (41) | 59 (51) | 69 (61) | 110 (102) |

Other rows, same sheet but H = 6 unless a row says otherwise (the
climbing rows start from the climb above, at the first tick whose z is
above 1; throttle (1, 0), aim as above unless stated):

| Setup | Expected |
|---|---|
| level aim (pitch 0) from the start | stands at x = 0.2, v = 0, grounded on the sheet, indefinitely |
| pitch 0.01 radian from the start | climbs exactly as at pitch 0.5; pitch 0.0003 climbs; pitch 0.0001 stands |
| pitch +0.5 then −0.5 after 22 ticks of climbing | v_z falls 0.0106667 per tick from 0.04875 (0.03808, 0.02742, ...), is negative on the 5th tick, reaches −0.09375 on the 14th tick and stays; stops at the floor (z = 0) and stays grounded on the sheet |
| throttle (−1, 0), aim toward the sheet | airborne on the first tick (no contact), then free fall with horizontal speed 0.00117 per tick away |
| throttle released | v_z falls by 0.0106667 per tick to 0 in 5 ticks and holds: the biped stays on the ladder |
| jump pressed on the first tick, throttle (1, 0) held | airborne at the end of that tick with v_z = 0.07; grounded on the sheet again on the next (moves 0.0664, then 0.0558, then 0.04875 per tick) |
| jump pressed on the first tick, no throttle | airborne and leaves: v_z 0.07, 0.0664, ... a plain ballistic arc, x unchanged |
| throttle (0.9, 0) with crouch held | speed falls to 0.01755 WU/t by the 8th tick (the crouch transition and the acceleration limit); still on the sheet |
| standing at the foot (level aim, grounded on the sheet), throttle (−1, 0) | tick 1: no contact, airborne; tick 2: back on the floor, grounded, then walks away |
| from the foot, throttle (0, 1) (left) at level aim | slides along the sheet toward −y (the biped's left at this yaw), 0.0106667 per tick up to 0.0666667 WU/t |
| sheet of H = 20 and no floor; the biped airborne at x = 0.4, z = 10, v_z = −0.2, throttle (1, 0), level aim | the sheet catches it on tick 19 (x = 0.2) with v_z −0.268; then v_z rises 0.0106667 per tick toward 0 |

The same sheet in front of a wall (a solid wall in the plane x = 0 up
to the platform at z = 2.5 and a platform behind it, normal +Z), same
inputs: the sheet is lost on tick 59 as above, the lip of the wall does
not touch the biped, and it lands on the platform on tick 83 at
x = −0.348, z = 2.5, v_z = 0, and walks away over it (supported by the
platform from then on).

### Landing and falling

The origin is dropped from rest at height H above flat ground,
airborne.

| Setup | Input | After | Expected |
|---|---|---|---|
| H = 1.0 | none | landing | tick 24, λ = 0.0855643, soft landing 5 ticks, no damage |
| H = 2.0 | none | landing | tick 33, λ = 0.1176509, soft 10 ticks, no damage |
| H = 3.0 | none | landing | tick 41, λ = 0.1461723, soft 14 ticks, no damage |
| H = 3.1 | none | landing | tick 42, λ = 0.1497375, soft 15 ticks, damage scale 0.0574539, 18.33 points |
| H = 4.0 | none | landing | tick 47, λ = 0.1675634, hard 30 ticks, scale 0.3517, 61.00 points |
| H = 5.0 | none | landing | tick 53, λ = 0.1889545, hard, scale 0.7048, 112.20 points |
| H = 6.5 | none | landing | tick 60, λ = 0.2139108, hard, 155 points |
| H = 16.5 | none | landing | tick 96, λ = 0.3422572: 155 points, no in-air kill |
| H = 17.0 | none | tick 97 | still airborne, v_z = −0.3458224: 105 points in the air; lands tick 98 (155 more) |
| H = 5.0, (1, 0) and jump held throughout | | landing tick L = 53 | hard landing; speed falls 0.0053333 per tick; no movement wish through L + 29; on L + 30 moves (speed 0.0106667) and jumps |
| flat, at 0.075, dropped from H = 5 holding (1, 0) | | after landing | speed 0.0696667, 0.0643333, ... 0.0003333 on L+14, 0 on L+15 |

## Confidence per section

| Section | Confidence |
|---|---|
| Constants: every tag value | checked against the disc (bloodgulch; identical on hangemhigh, prisoner, boardingaction, damnation); the multiplayer biped named by the globals' multiplayer information block checked |
| Precomputed tag values (sines, cosine, crouch rate, fall velocities) | checked against the disc, and consistent with the stated formulas and gravity |
| Gravity value | reference; confirmed by the disc's precomputed fall velocities |
| Per-tick vs per-second units of the fields | jump velocity (per tick) checked by value; that speeds and accelerations are per second and divided by 30 is from the reference, and gives the expected ~6.9 m/s run |
| Input: normalisation, dead zone, crouch rule, levels | reference only |
| Aim without lag | disc (zero limits) + reference |
| Ground movement, slope scale, acceleration | reference only; numbers from the simulation |
| Gravity, jump, air control, jump timer | reference only; simulated |
| Crouching, standing room | reference only |
| Collision response, ground test, virtual contact | reference only; the mover's shapes are `collision.md`'s (reference only there too); steps, crests and creases are emergent results of the simulation |
| Landing and falling damage | thresholds and effects from the disc; logic reference only |
| Camera | reference only (fields from the disc) |
| Climbable surfaces: which maps and how many | checked against the disc (all 13 multiplayer maps, level and object collision) |
| Climbable surfaces: how a biped mounts, climbs, tops out and drops | reference only; the numbers from a simulation of the rules here, which is not the full mover on the map's real geometry (sheet width, the wall behind it) |
| Elevators, teleporters, impulses | reference only, not simulated |
| Damage-system formula for the falling damage points | reference only; belongs to `health_shields.md` |

The reference build differs from the disc's in places, and its port
added features that are not the Xbox game's (riding moving vehicles,
players passing through each other in co-op, the body turning in the
air); this spec leaves those out. Where the biped tag offsets were
needed to read the values: the biped part of a `bipd` begins 0x2F0
into the tag's main struct (after the object and unit parts); the
globals' player information block is the field at 0x170 of `matg`,
falling damage 0x188, multiplayer information 0x164. `objects.md` owns
the biped layout and `globals.md` owns the globals' (every field of
player information, falling damage and the rest). The globals hold no
gravity field: all 61 spare words at the start of `matg` are zero, and
the gravity constant is the engine's (checked in `globals.md`).

## Open questions

- **Feel check.** Measure in the game: time from rest to full run
  (expected 8 ticks), a running jump's length (expected 2.925 WU =
  8.9 m) and the apex (0.65 WU: a standing jump onto a ledge 0.65 WU
  high, not 0.66, from flat ground). Capture: a level ledge test on
  Blood Gulch's bases (known heights from the collision BSP) and a
  frame-by-frame video of a standing jump next to a known wall.
- **Hard landing length.** The 30-tick hard landing for any hard
  landing comes from the clamp described in Quirks. Check: drop from
  about 4 WU and from 10 WU and time how long the player cannot move
  or jump (expected 1 s in both).
- **Air braking.** Confirm that releasing the stick mid-jump slows the
  player (expected landing speed 0.0295 WU/t from 0.075).
- **Step heights.** The 0.12 WU limit and the hops are emergent from
  the mover; measure which curbs and lips on the multiplayer maps a
  player can walk onto, and whether walking over a small lip gives a
  visible hop.
- **Crouch latency.** The stance follows the button one tick late
  (the animation update ends the tick); it can be delayed further if
  the current animation cannot be interrupted. Measure crouch-press to
  camera-move latency.
- **Controller crouch rule.** Confirm that a fully pushed stick
  uncrouches a grounded player on the Xbox.
- **Teleporter velocity.** Confirm that a player running into a
  teleporter keeps world-space velocity (exits moving in the same
  world direction, not the new facing). Run through the Hang 'Em High
  or Chill Out teleporters at full speed and watch the first ticks.
- **Falling damage in multiplayer.** Nothing exempts players in a
  match; confirm a 6 WU drop (Blood Gulch's cliffs) kills a player at
  full health and shields as `health_shields.md` computes it.
- **Standing on players.** The mover meets other players through
  their collision models; confirm in the game that one player can
  stand on another's head and how high that is.
- **Ladders on the real geometry.** The ladder rules are simulated on
  an ideal sheet, not on the maps' collision. Check on Boarding Action
  (its stacked ladders, a tilted pair at x = -2.47 and x = 22.48) and a
  one-sheet ladder (Wizard, Putput): where the top of the climb throws
  the player, and whether a level look at the ladder really stands
  still.
- **Elevator carry.** Which multiplayer maps have elevator machines
  (none expected); not checked on the disc.
