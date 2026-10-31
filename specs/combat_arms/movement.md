# Combat Arms: movement

Source basis: the client's local player movement manager and the game's
movement-penalty rules on top of it; the engine's shared motion integrator,
player collision mover and fixed-rate movement clock; the server's player
object (fall damage, crouch-spam configuration as shipped with a dedicated
server); the HUD stamina gauge and the player stats it drives; the game's
character (figure type, vest, head piece) and weapon attribute data.
Status: draft

Sections: Summary; Units and conventions; Constants; Behavior (tick clock,
input to control state, speed limit, acceleration, integration and friction,
stopping, jumping, gravity and air movement, crouching, crouch-spam penalty,
sprint and stamina, slopes and steps, falling and fall damage, volumes
(ladders, liquids, friction, speed factors, gravity volumes, jump pads,
safety nets, damage volumes), other modifiers including pushers and hit
knock-back, eye height); Per-tick order; Edge cases;
Quirks; Corrections (2026-10-19); Test cases; Open questions.

---

## Summary

Combat Arms movement is the LithTech "player physics" model: each fixed
15 ms tick the game clamps horizontal velocity to a stance speed limit,
sets an acceleration from the held keys (ramped up from a small start
value), and hands both to the engine, which integrates them in closed form
with exponential ground friction and adds gravity in the air. Because the
clamp happens *before* integration, the real ground speed settles a little
above the nominal limit (about 296.5 u/s for a 285 u/s run). Speeds are
scaled down by a combined penalty from the held weapon and worn gear.
Sprint multiplies the run speed and drains a stamina gauge; crouching blends
the speed limit and the hull height over about 150 ms and is rate-limited by
a "crouch spam" penalty. Air control is strong (full ground acceleration,
no friction), and a short-lived jump factor weakens back-to-back jumps.

## Units and conventions

- Distance: LithTech world units ("u"). No conversion is applied anywhere in
  this spec. The game's own HUD readouts (scope range, objective and flag
  distances) show world units divided by 40 as "metres", so the game's
  stated scale is 40 u = 1 m (1 u = 2.5 cm). Bodies are not built to that
  scale: the fallback standing hull is 100 u tall (2.5 m) and the eye sits
  83 u above the feet (Open question 3).
- Axes: +Y is up. Horizontal plane is X/Z. Gravity points along -Y.
- Player hull: an axis-aligned box given by its **half**-extents
  (half-width X, half-height Y, half-depth Z) around the object's position,
  which is the hull's centre. The collision mover treats it as an upright
  cylinder of radius `max(halfX, halfZ)` and half-height `halfY`.
- Angles: radians. Movement uses the player's horizontal facing (yaw);
  pitch is ignored for walking.
- Time: the movement tick is 15 ms (`dt = 0.015 s`), about 66.67 Hz.
  Velocities are u/s, accelerations u/s².
- "Tick n" counts from 1 for the first tick a condition holds. Values
  quoted "after tick n" are the object's state at the end of that tick.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| tick | 15 | ms | Fixed movement step (engine default movement rate 1000/15 Hz, step truncated to whole ms) |
| max ticks per frame | 6 | ticks | `floor(100 ms / tick)`; extra due ticks are dropped |
| run speed forward / back / strafe | 285 / 285 / 285 | u/s | Console variables `FRunVel`, `BRunVel`, `SRunVel` |
| walk speed | 70 | u/s | Console variable `WalkVel`; also used when zoomed |
| crouch speed | 50 | u/s | Console variable `DuckVel` |
| jump speed | 330 | u/s | Console variable `JumpVel`; vertical velocity set on jump |
| ladder speed | 200 | u/s | Cap on upward velocity on a ladder; not scaled by penalties or speed factors, and there is no horizontal limit on a ladder (Volumes, "Ladders") |
| player gravity | -1100 | u/s² | Along Y; console variable `PlayerGravity` |
| ground friction coefficient k | 5 | 1/s | `5 × min(1, Friction of the volumes touched)`, normally 5; `Filter` and `SafteyNet` volumes do not count; k = 0 means no friction at all (Volumes) |
| volume viscosity scale | 5000 | u/s² | Speed taken off per unit of a volume's `Viscosity` (same constant as grenades.md) |
| viscosity minimum speed | 1 | u/s | Viscosity acts only when the speed is above this |
| base move accel | 3000 | u/s² | Console variable `BaseMoveAccel` |
| start accel | 500 | u/s² | Console variable `StartAccel` |
| max accel | 3000 | u/s² | Console variable `MaxAccel`; also the cap on combined accel |
| accel ramp rate | 6000 | u/s³ | Console variable `AccelInc` (90 u/s² per tick) |
| crouch accel factor | 0.5 | × | Base accel halved while the crouch flag is set |
| in-air accel factor | 0.1 | × | Console variable `InAirAccelMultiplier`; see Quirks for when it actually applies |
| slide-to-stop time | 0.1 | s | Console variable `SlideToStopTime`; key-release braking |
| stop snap speed | 5 | u/s | Below this horizontal speed, braking zeroes velocity |
| engine rest threshold | 0.1 | u²/s² | `|v|²` and `|a|²` below this count as zero |
| jump factor after a jump | 0.3 | × | Set on every jump |
| jump factor recovery | 1.2 | 1/s | Added per second while not jumping, until ≥ 1 |
| jump stance tolerance | 0.5 | u | Jump needs current half-height ≥ standing half-height - 0.5 |
| walkable normal | 0.7071 | normal.y | Ground steeper than this (normal.y < 0.7071) is not ground for the integrator and the mover; the movement's own "on ground" flag uses 0.707 (Corrections 18) |
| stair step height | 30 (+0.5) | u | Server console variable `StairStepHeight`; mover uses 30.5 |
| standing probe | 0.02 × halfY | u | Distance below the hull that still counts as standing (1 u at halfY 50) |
| fall-probe ray | 150 | u | Downward ray for "falling" status (sound/animation only) |
| stand half-extents (fallback) | 15, 50, 15 | u | Used only if the model's stand animation has no box; normally from the model |
| crouch half-extents (fallback) | 15, 40, 15 | u | Same, from the model's crouch animation |
| crouch duration | 13/75 ≈ 0.17333 | s | Crouch speed `1/duration = 75/13 ≈ 5.76923` per s |
| crouch curve tangents | 3, 1 | | Start and end slope of the crouch rate curve |
| crouch release delay | 100 | ms (wall clock) | Crouch stays on until this long after it was pressed |
| fall damage start height | 300 | u | Drops shorter than this do no damage |
| fall damage full height | 700 | u | Drop that does 200 damage |
| fall damage at full height | 200 | HP | Linear, not clamped above |
| fall minimum | 50 | u | Second, redundant lower bound |
| pusher air factor | 0.1 | × | A pusher's push on a player standing on nothing is a tenth |
| pusher max upward speed | 100 | u/s | Console variable `PusherMaxYVelocity`; after a push, any upward speed above it is cut to it |
| eye offset standing | 0.66 × stand half-height | u | Camera height above the hull centre (33 u with the fallback box; console variable `ChaseCamOffset` holds it) |
| eye crouch offset | -13 | u | Added to the camera height × crouch rate (console variable `DuckDownCamOffSet`) |
| camera rise time | 0.1 | s | Console variable `CameraMoveUpTime`; smoothing of upward camera steps |
| camera rise minimum speed | 140 | u/s | Console variable `CameraMoveUpMinVel` |
| camera rise maximum lag | 30 | u | Console variable `CameraMoveMaxYDiff` |
| mover clearance | 0.1 | u | The collision mover stops 0.1 u short of a surface it hits, measured along that surface's normal |
| step lift / drop | 30.5 | u | Stair-step height + 0.5; the high path lifts and drops exactly this much |
| step fit distance | 0.5 × hull radius | u | Forward probe for stepping onto a steep-topped step (7.5 u with the fallback box) |
| knock-back time scale | 1 + `StiffDuration` | × | Figure-type value (0 in the stock data) that lengthens the hit knock-back |
| HUD distance scale | 40 | u per displayed metre | Every in-game distance readout divides world units by 40 |

Character and weapon data (attribute files; field names as stored):

| Data field | Where | Typical value | Meaning |
|---|---|---|---|
| `DashSpeed` | figure type | 1.0 (1.4 if absent) | Sprint multiplier on run speed |
| `DashSpeed` | vest model, head piece, goggles, backpack, gear mods | vest 0.4, others 0 | Added to `1 +` sum, multiplies sprint speed |
| `DashGauge` | figure type | 30 | Stamina drained per second while sprinting |
| `GaugeRec` | figure type | 25 | Stamina recovered per second |
| `Maxstamina` | figure type | 100 | Stamina capacity before gear bonuses |
| `MaxStamina` | vest, head piece, goggles, backpack, set items, gear mods | 0 (some vests 2.0) | Added to `1 +` sum, multiplies capacity |
| `MovePenalty` | weapon | 0 (knife) to 0.33 (LMG); e.g. pistol 0.03, M4A1 0.10, AK-47 0.13, MK.48 0.30 | Fraction of speed lost while held |
| `MovePenalty` | vest model, head piece, goggles, backpack, gear mods | default vest 0.1 | Added into the same penalty |
| `VariableSpeed`, `VariableWeight`, `VariableJump` | figure type | 0 | Speed bonus, weight penalty, jump bonus |

Crouch-spam penalty parameters (the server sends them to the client on
join; the client's built-in defaults have both penalties switched off). Values
are the dedicated server's shipped configuration; the server's own defaults
before reading it are the client defaults with both penalties switched on.

| Server config key | Shipped value | Client default | Meaning |
|---|---|---|---|
| `CrouchingDuration` | 0.1733333 | 13/75 | Crouch transition time |
| `CrouchingTangent0` / `1` | 3.0 / 1.0 | 3 / 1 | Crouch curve tangents |
| `EnableSlowdownPenalty` | 1 | 0 | Slowdown stamina on/off |
| `SlowdownSP_IncreaseDuration` | 1.7 | 2.0 | s to refill slowdown stamina |
| `SlowdownSP_IncreaseTangent0` / `1` | 0.5 / 3.0 | 0.5 / 3.0 | Refill curve |
| `SlowdownSP_DecreaseAmount` | 0.5 | 0.1225 | Cost per crouch press |
| `MaxSlowdownRate` | 5.0 | 5.0 | Crouch transition at most 5× slower |
| `MinSlowdownSP` | 0.875 | 0.875 | Slowdown starts below this |
| `EnableDisableCrouchingPenalty` | 1 | 0 | Lock-out stamina on/off |
| `DisableSP_IncreaseDuration` | 1.5 | 1.0 | s to refill lock-out stamina |
| `DisableSP_IncreaseTangent0` / `1` | 1.0 / 1.0 | 1 / 1 | Refill curve (constant rate) |
| `DisableSP_DecreaseAmount` | 0.4 | 0.5 | Cost per crouch press |
| `DisableSP_*_InPenalty` | 2 / 1.0 / 1.0 / 0.9 | same as above | Refill time/tangents and cost while locked out (only the cost is used, see Quirks) |
| `*_DecreaseDuringJumpingCrouch` | 1 | 0 | Presses in the air also cost |
| `*_DecreaseByKeyStroke` | 1 | 0 | Cost is charged per key press, not per crouch start |
| `*_DecreaseException_NoMovement` | 1 | 0 | Presses while standing still are free |
| `NoMovementTimeLimit` | 0.2 | 0.2 | s of history for "standing still" |
| `NoMovementDevationLimit` | 0.1 | 0.1 | u moved in that window below which you count as still |

## Behavior

### Tick clock

- Movement runs on a fixed clock. An accumulator adds real elapsed time
  each rendered frame; every whole 15 ms in it is one movement tick. At most
  6 ticks run per rendered frame; ticks beyond that are discarded (time is
  lost, the game slows down rather than catching up).
- Each tick uses `dt = 0.015 s` exactly. The rendered player position is
  interpolated between the last two tick positions by the leftover
  accumulator fraction.
- If the movement rate is set to 1 Hz or less, the clock is off and one tick
  per frame runs with `dt` = frame time clamped to 0–100 ms.
- What runs per tick and what per frame (Open question 18):
  - **Per tick:** input to control flags (including the sprint decision and
    the sprint key state sent to the server), the crouch rate, the
    crouch-spam values, the jump factor, acceleration, braking and the move.
    The ticks of one frame run back to back inside that frame.
  - **Per tick, but against the wall clock:** the 100 ms crouch release
    delay. Each tick compares the OS millisecond clock with the time crouch
    was pressed. All ticks of one frame read (almost) the same clock value,
    so the delay ends on a frame boundary, not a tick boundary.
  - **Per rendered frame:** the stamina gauge (HUD update, after the ticks)
    and the weapon logic (inventory.md, weapon_firing.md), which reads the
    control flags of the frame's last tick.
  - **Hull resize:** the resize for the crouch rate is called twice per tick
    (Per-tick order steps 5 and 10), but the crouch rate does not change in
    between and the first call always leaves the hull at the wanted size
    (forced if needed), so the second call never changes anything. Treat it
    as one resize per tick, before the motion step.
  - **Sprint speed** needs the server's confirmation (Sprint and stamina);
    it does not take effect on the tick the key is pressed.
- **Client and server** (Corrections 20). The movement rules in this spec
  run only on the owning client. Each tick's result is sent to the server
  as a record: the displacement, the control flags, the sprint and
  stamina-increment flags, the hull size, zoom and firing states. The
  server does not re-run the rules: it moves its copy of the player by the
  reported displacement through its own collision (the same mover, with the
  other characters rewound to the time the client saw), then, if the
  result is within its error tolerances of the client's reported position,
  puts the player exactly there. Lost records are filled in from the next
  one. The server keeps its own stamina gauge, advanced per record with the
  record's duration clamped to 100 ms, draining while the record says
  sprint and refilling only when it says "increment"; when its gauge
  reaches 0 it ends sprint and tells the client, which then loses the
  sprint speed (Speed limit). Fall damage, ladder state for fall damage and
  safety nets are judged on the server from the resulting positions, and
  only on records that moved the player. When the server forces a position
  (teleport, respawn) the client takes it with zero velocity and
  acceleration.

### Input to control state

Each tick the held commands become a set of flags. If the game has
no focus for movement (menus, shop open, external camera), all flags clear
except crouch, and sprint ends.

1. **Crouch.** Set while the crouch key is held (the "crouch toggle" binding
   behaves as hold). The press time is noted only on a tick where the
   crouch flag turns on after being off on the previous tick (a re-press
   while the flag is still held on by the delay does not restart it). When
   the key is up and the flag was set last tick, crouch stays set if less
   than 100 ms of wall-clock time has passed since that press time, or if
   the standing hull does not fit. The fit test only runs when the hull is
   at (or below) the crouched half-height; a hull that is partly crouched
   (`r < 1`, hull taller than the crouch box) always counts as fitting
   (Corrections 16). The "crouch key pressed" state for the spam penalty is
   the crouch flag **after** these two holds (not the raw key), and a "key
   press" is that flag turning on (Corrections 8). Then, if the lock-out
   penalty is active and the crouch rate is 0, crouch is cleared.
2. **Run.** Only when crouch is not set: run is set when `walk key held XOR
   always-run`. Always-run is on by default, so run is the default state and
   the walk key turns it off. The run flag does not depend on movement keys:
   it is set while standing still too. The walk key is its own command
   (profile action "Speed", default key C), separate from sprint (action
   "Dash", default left Shift). A "run lock" command flips always-run; it is
   unbound by default.
3. **Jump.** Only when crouch is not set, the player is on ground, not on a
   ladder and not already in a jump: if the jump key is held and no jump has
   been "used" since the key was last up on the ground, the jump is used,
   and the jump flag is set if the hull's current half-height is at least
   standing half-height - 0.5. If the jump key is up on such a tick, the
   "used" mark clears. A held key therefore jumps once; it must be up for at
   least one grounded tick before the next jump.
4. If the crouch rate is not 0 (still crouched or standing up), jump and
   sprint are cleared.
5. **Forward.** If held: forward flag. Then sprint is decided (see Sprint).
   If forward is not held, sprint ends.
6. **Back, left, right** flags from their keys. Holding back also ends
   sprint.
7. **Moving** flag: set if any of forward, back, left, right or jump is set.
   Opposing keys (forward with back, left with right) both set their flags,
   so the moving flag is on even though that axis's acceleration cancels to
   exactly 0 (see Acceleration and Open question 17).
8. **Jump request:** if the jump flag is set now but was not last tick, and
   the player is not in a jump and jumping is allowed, a jump is requested.
   The request is a remembered state, not a one-tick event: it is cleared
   only by a jump that happens, by every tick of free movement (Volumes),
   and on entering a world. A request the motion step cannot honour (not
   on ground any more, see Jumping) stays pending (Corrections 6).
9. While a hit-stagger timer runs (only infected characters in the infection
   mode), run and sprint are cleared.

### Speed limit

The horizontal speed limit `V` for the tick, from the flags after step 9:

- On a ladder: 200 (times the penalty and factors below), but on a ladder
  this limit is never used as a clamp: the horizontal speed is not limited
  at all and only the upward speed is capped, at a plain 200 (Volumes,
  "Ladders").
- Else, with `K` = the last direction speed (forward key held: forward run
  speed; else back held: back run speed; else a strafe key held: strafe run
  speed; else unchanged from before; all 285), and `r` the crouch rate after
  this tick's crouch update:
  - "crouching" means the crouch flag is set or `r = 1`.
  - Sprinting (sprint flag set **and** the server has confirmed sprint),
    not crouching, not zoomed:

```math
V_0 = K \cdot D_{fig} \cdot \left(1 + D_{head} + D_{goggle} + D_{vest} + D_{pack} + D_{gear}\right)
```

  - Else running (run flag), not sprinting, not crouching, not zoomed:
    `V_0 = K`.
  - Else (walking, crouching, zoomed): `V_0 = WalkVel = 70`.
  - Then blend toward crouch speed by the crouch rate:

```math
V_1 = \max(V_0, 50) + (50 - \max(V_0, 50)) \cdot r
```

- Then the penalty:

```math
P = \mathrm{clamp}\left(P_{weapon}(1 + c_{mod}) + P_{vest} + P_{head} + P_{goggle} + P_{pack} + P_{gear} + w_{fig} - s_{fig} + P_{custom} + P_{stun},\ -1,\ 1\right)
```

  where `c_mod` is the weapon modification's own penalty factor (normally 0),
  `w_fig`/`s_fig` the figure's weight and speed values (0), `P_stun` a
  temporary stun penalty from certain ammo. A VIP account uses `P = P_vest`
  only, and its sprint formula drops `D_fig` and every bonus except the vest's.

```math
V = V_1 \cdot (1 - P + S_{set}) \cdot f_{slow} \cdot f_{container}
```

  `S_set` is a set-item speed bonus (0), `f_slow` a temporary slow from
  damage (1 when inactive), `f_container` the product of the
  `PlayerSpeedFactor`s of the volumes the player is in (1; Volumes).
  Between the `(1 - P + S_set)` factor and `f_slow` two mode-only factors
  apply in this order: a mutation-item buff or nerf of the move speed, and,
  for a human (not infected) player in the infection mode while the
  infected side has an infected mercenary, that perk's speed rate. Both
  are 1 outside those modes; their values belong to the game-mode specs
  (Corrections 21).

The reference character (figure DashSpeed 1.0, default vest MovePenalty 0.1
and DashSpeed 0.4, no other gear) holding an M4A1-class rifle (0.1) has
`P = 0.2`: run 228, sprint 319.2, walk 56, crouch 40 u/s.

At the start of the motion step, if the horizontal velocity magnitude
exceeds `V`, it is scaled down to exactly `V` (direction kept, vertical
untouched). On a ladder this clamp is replaced by the upward cap. There is
no other speed cap; see Integration for why the measured speed ends up
higher.

### Acceleration

Basis: forward `F` = the player's facing with its vertical part removed and
renormalized; right `R` = the facing's right vector with vertical part
removed.

- Base magnitude `A = 3000`; halved to 1500 if the crouch flag is set (not
  on ladders); then × 0.1 if in a jump.
- If a jump request is pending (normally only the jump-request tick, but
  see Corrections 6), in free movement the jump flag is set, or the crouch
  flag is set, the directional accelerations are simply `A·F` and `A·R`
  (Open question 12).
  The whole ramp block is skipped on such a tick: there is no start rescale
  to 500, even on the tick the moving flag turns on, and `L_f` and `L_r` are
  neither ramped nor reset; they keep the values they had before. So a
  crouched start from rest accelerates at the full 1500 from its first tick,
  and a player who stands up while still holding forward resumes the ramp
  from the old `L_f` (3000 if they were running at full ramp before
  crouching).
- Otherwise a per-axis ramp replaces their magnitude. Two remembered values,
  `L_f` (forward/back) and `L_r` (strafe), start at 500:
  - On the tick the moving flag turns on, `L_f = L_r = 500` and the whole
    result is later rescaled to 500 (see below). On that tick the forward
    axis is not ramped: its magnitude is the base `A` (3000, or 300 in a
    jump), so the direction of the 500 start follows `(A, a_r)`, not
    `(500, a_r)` (Corrections 15).
  - Forward/back axis: if forward is held now and back was held last tick,
    or back now and forward last tick, the forward accel is 500 and
    `L_f = 500`. Whether the other key is still held does not matter: while
    forward and back are both held this is true on every tick, so `L_f`
    stays at 500 (Corrections 7). Else, if forward or back is held (and the
    moving flag did not just turn on): if `L_f + 6000·dt < 3000`, the
    magnitude is `L_f + 6000·dt` and `L_f` takes that value; otherwise
    3000 and `L_f = 3000`.
  - Strafe axis: same with left/right and `L_r` (this axis ramps even on the
    tick the moving flag turns on, so `L_r` is 590 after that tick).
  - An axis with neither key held resets its `L` to 500.
- Sum: `a = (+F·a_f if forward) (−F·a_f if back)`; then each held strafe
  key adds `±R·a_r` and, after each addition, clamps `|a|` to 3000.
- If the moving flag just turned on and `a ≠ 0`, rescale `a` to magnitude
  500 (only on ticks that went through the ramp, see above).
- If the jump factor `j < 1` (see Jumping), `a ← a·j`. This applies to every
  acceleration, including the bypassed crouched one: crouched in a jump the
  acceleration is `150·j`, not 150 (Open question 11).
- Holding opposing keys gives `a = 0` on that axis but still counts as
  moving. The two keys use the same vector, so the cancellation is exact;
  the axis's `L` is reset to 500 on every such tick (each key was held on
  the tick before, which reads as a reversal), and the start rescale does
  nothing because `a = 0`. Releasing one of the two is a reversal too, so
  the remaining key starts again at 500 (test case 80). Because the moving flag is
  on, braking does not run: on the ground only the engine friction slows the
  player (`v ← v·e^{-k·dt}` per tick), and they never snap to a stop until
  `|v|² < 0.1`.

### Integration and ground friction

The engine integrates the object's velocity `v` and acceleration `a` over
`dt`, with gravity `g = (0, -1100, 0)`:

- First, a velocity with `|v|² < 0.1` is zeroed, and the same for `a`.
- **Gravity switched off** (on a ladder, and on the first tick after
  leaving one; Volumes, "Ladders"): none of the rules below apply. If both
  are below threshold the object stops; otherwise `a` is used as it is (no
  gravity, no projection onto the ground) and there is no friction, even
  while standing on walkable ground.
- **Standing on something**, and both are below threshold: the object
  stops (velocity and acceleration zero, no move).
- The surface normal `n` used below is the plane of the world polygon
  stood on (world geometry, including a world model's own polygons). When
  the thing stood on is a model (another character, a prop model), `n` is
  taken as straight up, (0, 1, 0): such a surface is never steep for the
  integrator and gets ground friction (Corrections 19).
- **Standing on ground that is too steep** (`n.y < 0.7071` for the surface
  normal `n`): the acceleration including gravity is projected onto the
  plane, any up-slope part along gravity removed, and rescaled to its
  original magnitude; no friction. The player slides.
- **Standing on walkable ground:** let `v' = v + g·dt`.
  - If `v'·n > 0.01` (moving off the surface, e.g. the jump tick), gravity
    is added to `a` and there is no friction.
  - Else if `a·n > 0.01` (accelerating out of the surface, e.g. walking
    downhill), `a` is pushed into the surface, `a ← a − n(n·a + 1)`, rescaled
    to its original magnitude, with friction.
  - Else (normal walking) `a` is used as is (no gravity), with friction.
- **Not standing:** `a ← a + g`, no friction.

With friction (coefficient `k = 5`, or less in a low-friction volume, but
above 0), exact closed form:

```math
v_{new} = \frac{a}{k} + \left(v - \frac{a}{k}\right) e^{-k\,dt}
```

```math
\Delta x = \frac{a}{k}\,dt + \frac{(k v - a)\left(1 - e^{-k\,dt}\right)}{k^2}
```

With `dt = 0.015`, `e^{-k dt} = 0.9277434863`.

Without friction, and also on the ground when `k = 0` (a volume with
`Friction` 0; the closed form is not used as a limit):

```math
v_{new} = v + a\,dt, \qquad \Delta x = v\,dt + \tfrac{1}{4} a\,dt^2
```

(the displacement uses a quarter, not a half; see Quirks).

The displacement is then moved through the world by the collision mover
(slide along at most two planes, stair-step, see Slopes and steps), only
when its squared length is above 10⁻⁶. After the move:

- If the attempted move had both horizontal components above 0.01, the
  movement's "on ground" flag of this tick (the one set at step 7c, before
  the move: false in a jump, on steep ground and on another character) is
  false, and the position did not change (within 10⁻⁶), the horizontal
  velocity is zeroed (stuck against a wall in the air). What the player
  stands on after the move does not matter (Corrections 17).
- Otherwise, if the attempted vertical move was above 0.01 in size but the
  actual vertical change was below 0.01, the vertical velocity is zeroed
  (landing, hitting a ceiling, or resting on a slope you would slide on).

Because the speed clamp is applied before this integration, a player
holding a direction ends each tick slightly above `V` and is clamped back
next tick. The steady state per tick, with `v = V` and `|a| = 3000` (1500
crouched):

```math
v_{end} = 600 + (V - 600)\,e^{-0.075}, \qquad \Delta x = 9 + \frac{(5V - 3000)(1 - e^{-0.075})}{25}
```

Run 285 → 4.44784 u per tick, 296.52 u/s. Walk 70 → 89.39 u/s. Crouch 50
(accel 1500) → 59.14 u/s.

### Stopping

When the moving flag is off, the player is on ground, not in a jump, the
summed current of the visible volumes the hull touches is zero and their
summed viscosity is zero (a ladder's 0.4 counts, a volume's `Friction` does
not), braking runs after the motion step (before integration) and replaces
the velocity:

- Let `h` = horizontal velocity. If `|h| > 5`: reduce its magnitude by
  `dt · K / 0.1` (42.75 u/s per tick at `K = 285`); if that would reach or
  pass zero, the horizontal velocity becomes zero. In both cases the
  vertical velocity is kept (Corrections 14).
- If `|h| ≤ 5`: the whole velocity, vertical included, becomes zero.

The engine friction then also applies (acceleration is 0, so the velocity
decays by `e^{-k dt}` and moves `v(1 - e^{-k dt})/k`). From a steady run the
player stops in 6 ticks over 9.4034 u (see Test cases).

Braking is skipped while the infection-mode stagger timer runs.

### Jumping

- On the jump-request tick, if the player is not dead, on ground and not on
  a ladder: vertical velocity is set (not added) to

```math
v_y = 330 \cdot (1 + J_{fig} + J_{model}) \cdot j
```

  where `j` is the jump factor at the start of the tick and `J` are the
  figure's and vest's jump bonuses (0). Horizontal velocity is kept. The
  player is now "in a jump"; the request clears. A jump pad volume makes
  the request itself, without the jump key, and its launch velocity is
  added to the whole velocity instead of setting `v_y` (Volumes, "Jump
  pads").
- **Pending request** (Corrections 6). If a request is pending but the
  player is not on ground at the motion step (or is on a ladder, or dead),
  nothing happens and the request stays. While it is pending: the ramp is
  bypassed every tick (directional accel = base `A`, 3000, with no 500
  start; the 0.1 in-air factor does not apply because the player is not in
  a jump), the jump factor does **not** recover, and on the first tick the
  player is on ground again (the landing tick after a fall) the jump
  happens by itself, without the key. The usual way to get one: pressing
  jump on the tick after the move that carried the hull off a ledge (the
  key reads the on-ground state one tick older than the motion step), or
  pressing jump while movement is disallowed (Other modifiers).
- **Jump factor `j`.** Starts at 1. On a jump tick, `j` becomes 0.3 (if it was
  already below 0.3 it decreases by `0.05·dt` instead, which cannot happen in
  practice). On every other tick, if `j < 1` it increases by `1.2·dt`
  (0.018), else it is set to 1. It can overshoot to 1.002 for one tick. It
  scales the horizontal acceleration whenever below 1, and the next jump's
  speed. After a jump it is back to 1 after 40 ticks (0.6 s), just before a
  flat-ground landing.
- **Landing.** At the start of each tick's motion step the game asks the
  engine what the player stands on (from last tick's move). If standing on
  something and the player was in a jump, the jump ends (landing). Ground
  is then denied if the surface normal is steeper than 0.7071 or the thing
  stood on is another character. While in a jump the player is never
  "on ground" for movement purposes.
- **Repeat delay.** There is no timer. A held key jumps once; a new jump
  needs one grounded tick with the key up, then a press. With the landing
  detected one tick after contact, the earliest re-jump is on the third
  tick after the contact tick.
- Jumping needs the standing hull: no jump while the crouch flag is set,
  while the crouch rate is not 0, or while the hull is more than 0.5 u
  below standing half-height.
- Jump stamina: a custom-room option can make jumps cost stamina; in normal
  play it is 0 and jumps are free (see Quirks for the client's handling).

### Gravity and air movement

- Gravity is -1100 u/s² on the player (other objects use -1600). Gravity
  volumes replace it with the sum of their gravities. Liquids do not change
  the player's gravity (their -200 is for other objects only), and on a
  ladder gravity is off (Volumes).
- In the air there is no friction and no braking; horizontal velocity is
  kept when no key is held.
- The speed clamp still applies every tick, using the current stance's
  limit (releasing sprint in the air drops the cap at once).
- Air acceleration is the same ramped value as on the ground (up to 3000),
  times `j`. The 0.1 in-air factor only survives when the ramp is bypassed:
  crouching in the air after a jump (1500 × 0.1 = 150 u/s², then × `j`). A
  player who crouches during a jump has almost no air control: with
  `j = 0.3 + 0.018(n - 1)` on tick `n` after the jump, it is 45 u/s² on
  tick 1, 96.3 on tick 20 (apex) and 147.6 on tick 39; the full 150 only from
  tick 40, which a flat-ground jump barely reaches before landing. (Walking
  off a ledge is not "in a jump": crouched air accel is then 1500, with
  `j = 1`.)
- A flat-ground jump from rest rises to 50.7375 u above the start on
  tick 20 and touches down on tick 41 (see Test cases).

### Crouching

- **Crouch rate** `r ∈ [0, 1]` (0 standing, 1 fully crouched), updated once
  per tick from the crouch flag, before the motion step. Transition speed
  `c = 1/duration` (75/13 per second), possibly slowed by the spam penalty.
  With tangents `T0 = 3`, `T1 = 1`, the shape function is

```math
s(t) = (3t^2 - 4t + 1)\,T_0 + (-6t^2 + 6t) + (3t^2 - 2t)\,T_1 = 6t^2 - 8t + 3
```

  - Crouch flag set and `c > 0`: on the first tick of going down the
    elapsed time `e` is 0, then it grows by 15 ms per tick. `t = e·c` (e in
    seconds), `r ← r + s(t)·dt·c`; if `r ≥ 1 - 10⁻⁶`, `r = 1`.
  - Else if `r > 0`: going up, same with its own elapsed time,
    `r ← r - s(t)·dt·c'` where `c' = c`, or 75/13 if `c` is 0 (the lock-out
    stops crouching down but you still stand up at normal speed); if
    `r ≤ 10⁻⁶`, `r = 0`.
  - Else: idle, both elapsed times reset.
  - **Reversal** (Open question 13): switching direction mid-transition
    (crouch released before `r` reaches 1, or pressed again while standing
    up) restarts the new direction's elapsed time at 0. The rate is not
    mirrored or reset: it continues from its current value, and because
    `s(0) = 3` the first tick of the new direction moves fastest (about
    0.26 of the full range at normal speed). The other direction's elapsed
    time is forgotten and restarts at 0 the next time that direction runs.
  - On a ladder (the hull touching one), while dead, or in free movement
    by the eye test (the eye point in a liquid or ladder volume; Volumes,
    "Membership"), `r` is reset to 0 directly.
  - `t` is not clamped. Full crouch and full stand each take 10 ticks
    (150 ms) at full speed.
- **Hull height.**

```math
h_y = H_{stand} - r\,(H_{stand} - H_{crouch})
```

  on the half-height, other half-extents stay at standing size. Applied once
  per tick before the motion step (the second call before integration is a
  no-op, see Tick clock). If the resize fails (no room), the old size and
  position are restored and then the new size is forced without collision.
  - **Shrinking:** the size is set around the centre. If the player was
    standing on something before the resize, the player is then moved down
    by the shrink amount (no sliding, keeping what it stands on) so the feet
    stay put. When it shrinks in the air, the centre stays and the feet rise.
  - **Growing** (standing up, Open question 14 and Open question 5): the
    half-height grows by `d` in three moves of the old-size hull, each
    stopping at the first obstacle (with the 0.1 u mover clearance): first
    down by up to `d`, then from there up by up to `2d`; the hull is then
    centred in the free span found and given half-height
    `old + (high - low)/2`. Results:
    - on the ground the downward move is blocked at once, so the hull grows
      upward and the **feet stay put** (same as shrinking);
    - in open air the hull grows around its centre: the feet drop by `d`
      and the head rises by `d`;
    - in the air with the floor less than `d` below the feet, the hull is
      pushed up so its bottom ends on the floor (the centre rises by about
      `d` minus the gap);
    - if the free span is shorter than the new height (ceiling), the
      result is smaller than wanted, counts as a failure, and the new size
      is then forced around the old centre without collision, overlapping
      the obstacle for that tick. Releasing crouch does not normally reach
      this case, because the release is refused while the full standing hull
      does not fit (Input to control state, step 1).
- **Crouch-jump:** jump first (standing), then crouch in the air. The feet
  rise by `H_stand - H_crouch` (10 u with the fallback boxes) over the
  10-tick crouch, which lets the player clear higher edges. Standing back up
  in the air grows the hull around the centre again (feet drop back),
  unless the floor is closer than the growth, in which case the hull is
  pushed up off the floor.
- **Speed and accel while crouched:** limit as in Speed limit; base accel
  halved and no ramp. Run and jump flags are not set while crouched; sprint
  ends.

### Crouch-spam penalty

Two "crouch stamina" values in [0, 1], both starting at 1: **slowdown**
(slows the crouch transition) and **lock-out** (forbids crouching). Updated
every tick after the crouch rate, with:

- `Δr` = this tick's change of crouch rate;
- a **crouch start** happens when `|Δr| > 10⁻⁶`, `Δr > 0`, and the last
  non-zero change was not upward; it resets a timer `e_c` to 0, otherwise
  `e_c` grows by 15 ms per tick;
- a **key press** is the raw crouch state turning on this tick;
- **still**: the sum of per-tick horizontal distances moved over the last
  0.2 s (ticks no more than 200 ms older than the newest) is below 0.1 u;
  this uses positions up to the previous tick.

Slowdown (if enabled):
- If no crouch start, no key press and `|Δr| ≤ 10⁻⁶` (fully up or fully down
  and resting): refill by `s_s(e_c/1.7) · dt / 1.7`, with the shape
  `s_s(t) = 4.5t² - 2t + 0.5` (tangents 0.5 and 3); full refill from 0 takes
  1.7 s.
- Else, if (not still, or the still exception is off) and (on ground, or
  air presses count): subtract 0.5 on a key press (or on a crouch start when
  "by keystroke" is off).
- Clamp to [0, 1]. If disabled, it is held at 1.

Lock-out (if enabled), with `L` = whether lock-out was active at the start
of the tick:
- If no crouch start, no key press, and on ground: refill by `dt / 1.5`
  (constant shape 1, tangents 1 and 1; 0.01 per tick). If it reaches 1,
  lock-out ends.
- Else, if (not still, or the *slowdown's* still exception is off) and (on
  ground, or air presses count): subtract 0.4 (0.9 if `L`) on a key press
  (or crouch start, as above). If it reaches 0 or below, lock-out begins.
- Clamp to [0, 1]. If disabled, it is held at 1 and lock-out is off.
- No refill happens in the air.

Effect on crouch speed `c`:
- Lock-out active (and enabled): `c = 0`, so the crouch rate can only
  return to 0, and once it is 0 the crouch flag is cleared each tick. The
  HUD shows a "crouch prohibited" marker.
- Else if slowdown is enabled and `MinSlowdownSP` is not ~0 and the slowdown
  value `S ≤ 0.875`:

```math
c = \frac{c_0}{5 + (1 - 5)\cdot \frac{\mathrm{clamp}(S, 0, 0.875)}{0.875}}
```

  (5× slower at `S = 0`, normal at `S = 0.875`). Else `c = c_0`.

With the shipped values, three crouch presses about 0.2 s apart while
moving lock crouching out for 1.5 s.

### Sprint and stamina

Sprint (the game's "dash") is decided on ticks where forward is held and
movement is allowed:

- A kill-streak perk that grants free sprint instead forces sprint speed
  whenever running, and the sprint flag itself stays off.
- "Can sprint": the run flag is set (walk key not held, not crouched), not
  on a ladder, crouch rate is 0, not carrying a mode flag (always false in
  this build), not snowball-frozen, and (in one build variant) not mid-swing
  with a knife.
- If the sprint key is held, stamina as an integer is at least 1, and can
  sprint: the sprint flag is set, the stamina state is **draining**.
- Else, if the sprint key is held and can sprint (so stamina < 1): the
  sprint flag is off and the stamina state is **stopped**.
- Else: the sprint flag is off and the stamina state is **recovering** (or
  draining at a perk rate if a perk drains stamina passively).
- Not holding forward, or holding back, ends sprint (state recovering).
- **Walk and sprint together:** the walk key clears the run flag, so "can
  sprint" fails; with sprint held the state is then **recovering** (not
  stopped, which needs "can sprint"), the speed limit is walk speed and the
  gauge refills.
- **Forward and back together** (with sprint held and the gauge ≥ 1):
  forward sets the sprint flag and the **draining** state first, then the
  back key clears the sprint flag in the same tick but leaves the gauge
  state alone. The player does not sprint, yet the gauge drains at 30/s
  (see Quirks). Releasing sprint or back ends it.
- Sprint speed takes effect only while the server confirms the local
  player is sprinting; the server ends sprint itself when its own copy of
  the gauge runs out.

Stamina gauge, updated once per rendered frame on the wall clock, with `Δt`
the milliseconds since the previous update:

- Capacity:

```math
S_{max} = \left\lfloor \mathrm{Maxstamina}_{fig} \cdot \left(1 + M_{head} + M_{goggle} + M_{vest} + M_{pack} + M_{set} + M_{gear}\right) \right\rfloor
```

  (stored as an integer; VIP accounts use the figure value alone).
- Draining: `S ← max(0, S - DashGauge·Δt/1000)` (30/s).
- Recovering: `S ← min(S_max, S + GaugeRec·Δt/1000)` (25/s).
- Stopped: `S ← 0` (pinned at zero while the key stays held), except during
  a stamina-kit effect.
- A stamina kit (damage of the stamina type) sets `S = S_max`.
- Spawning and round resets set `S = S_max`.

So a full 100-point gauge gives 3.3 s of sprint (it ends when it drops
below 1) and refills from 0 in 4 s once the sprint key is released.

### Slopes and steps

- Ground is walkable when the surface normal has `n.y ≥ 0.7071` (45°).
  Steeper surfaces are not ground: no jumping, no braking, and the
  integrator slides the player down them.
- The player stands on whatever the hull touches within `0.02·halfY` below
  it (1 u for a 50 u half-height). Surfaces with `n.y < 0.001` are never
  stood on.
- **Collision shape.** The mover sweeps an upright cylinder with flat top
  and bottom caps (radius `max(halfX, halfZ)`, half-height `halfY`). Every
  hit backs the hull off by 0.1 u along the hit surface's normal (mover
  clearance). A flat cap that lands on a horizontal edge reports an upward
  normal (0, 1, 0); the curved side meeting an edge reports a normal
  perpendicular to the edge, pointing at the hull, which is near-horizontal
  for an edge at the side of the hull.
- **Stair stepping** (Open question 15). It is tried on every move with a
  horizontal part (start and target X/Z differ), on the ground or in the
  air:
  1. **Floor plane.** If the player was standing on something, a probe
     straight down from the start (to 0.1 × half-height below the lower of
     start and target) finds the floor. If its normal faces against the
     move (`move·n ≤ 0`), it is the floor plane; if that floor is steep
     (0 < n.y < 0.7071) and the move projected onto that plane would go up
     (`(move - n(move·n)).y > 0`), the **whole** move, vertical part
     included, is replaced by its part along the slope's level direction
     `n × (0, 1, 0)` normalized (sideways only). The floor plane is not
     used for anything else: the low path is not projected onto it
     (Corrections 13).
  2. **Low path.** Slide toward the target at the current height (see
     "Sliding" below). If the low slide touched nothing, keep it. If
     anything it touched during the slide is "not a step", keep it too. Not
     a step: a surface flagged not-a-step in the map, any object that uses
     player collision (moving world objects), and any character. Any other
     hit (a wall, a box, a step's riser, even a slope) is a candidate step;
     there is no height or normal test at this stage.
  3. **Lift.** Move the hull straight up by 30.5 u from the start. If this
     hits anything (low ceiling), keep the low result.
  4. **High path.** Slide toward the target from the lifted start. Then drop
     straight down from the high slide's end by 30.5 u (no further) and stop
     at the first surface. If nothing is found, the high result is simply
     30.5 u lower than the high slide's end, i.e. back at the start height.
  5. **Steep top.** If the surface found on the drop is steep
     (n.y < 0.7071): refuse the step if the high slide hit anything;
     otherwise probe from the lifted start to the high slide's end pushed
     further along the slope's up-hill level direction by
     `max(0.5 × radius, 2·30.5·(1 - n_y²) + 0.1)` u; refuse if that probe
     also ends on a steep surface. (A wall with n.y = 0 has no up-hill
     direction, and the step is refused.)
  6. **Choose.** With `o` the (possibly reduced) move, compare
     `h = (high - start)·o` with `l = max(0, (low - start)·o)`; take the high
     result when `1.01·l < h`. If both results end at the same X and Z, take
     the high one when it is not lower than the low one.
- **Sliding** (Corrections 13). One slide sweeps the hull from its start
  toward its target and stops at the first contact (with the 0.1 u
  clearance). With `o` the remaining move and `n` the contact normal:
  - first contact: if `o·n < 0`, `o ← o + 1.001·|o·n|·n` (the extra
    0.1 % pushes the hull very slightly off the surface), then the rest of
    `o` (scaled by the unused fraction of the sweep) is swept again;
  - second contact with a normal not parallel to the first
    (`|n₂·n₁| < 0.999`): the rest moves only along the crease,
    `o ← c (o·c)` with `c = unit(n₂ × n₁)`;
  - a second contact parallel to the first, or any third contact: stop at
    that contact point;
  - stop as well when the rest is shorter than 0.01 u (squared length
    below 10⁻⁴), when the sweep's target lies behind the start along the
    original move, or after 5 sweeps.
  So at most two surfaces are slid along; a player pushed into a third
  surface in the same tick stops there. A hull that starts a sweep
  already overlapping a surface (contact at time 0 twice on the same
  normal before moving) is moved out along that normal by a quarter of
  the move length, up to four times, before giving up.
- **No ground snap or downhill projection in the mover.** Nothing pulls the
  hull down onto the ground after a move. Walking downhill works through the
  integrator (acceleration pushed into the surface, see Integration) and
  the 1 u standing probe. A drop larger than the probe makes the player
  airborne (no "in a jump", so the full ramped acceleration and `j` stay),
  and gravity takes over.
- **What the player stands on** is found after every move by sweeping the
  hull down by `0.02 × half-height`:
  - nothing hit: not standing;
  - a surface with `n.y < 0.001` (vertical): not standing;
  - a steep surface (n.y < 0.7071) while the player was standing on a
    walkable world surface: the player keeps standing on the old surface if
    a probe from 30.5 u up, moved up-hill by
    `max(0.5 × radius, 2·(30.5 + 0.02·halfY·t)·(1 - n_y²) + 0.11 × radius)`
    (t = hit time of the down sweep, 0..1), meets nothing; otherwise the
    steep surface is stood on (and then counts as "not ground", so the
    player slides);
  - anything else is stood on.
  So a hull whose flat bottom still overlaps a ledge by any amount stands
  on it with normal (0, 1, 0), which is walkable ground, while an edge that
  only touches the curved side does not hold the player up (Open
  question 15).
- Moving up a steep slope while standing on it: the up-slope part of the
  move is removed, leaving only the sideways part.
- A step that is reachable from below (≤ 30.5 u) is climbed without
  jumping; mid-air, the same rule lets the player climb onto ledges up to
  30.5 u above the hull's current base.

### Falling and fall damage

Fall damage is decided on the server from positions:

- While not standing on anything, the highest Y reached is tracked; it
  starts at the Y of the last grounded tick and only increases.
- On each tick the player is standing on something, the drop
  `d = highest Y - current Y` is taken (only if positive), the highest Y is
  set to the current Y, and damage is checked for that tick's drop.
- On a ladder, the highest Y is reset to a very low value (no fall
  accumulates); in spectator mode likewise.
- **Spawn and respawn** (Open question 16): the server first moves the
  player straight down onto the floor below the spawn point, then sets the
  highest Y to the resulting position and clears the accumulated drop. A
  spawn point placed in the air therefore never causes a fall: the player
  starts on the floor. Before the first spawn the highest Y is the very low
  value, so nothing accumulates.
- **Teleports and server position corrections** do not reset the tracking
  (no reset was found outside spawning). The drop is measured from the
  highest Y reached since the last grounded update: a player teleported
  while airborne keeps the old highest Y if the teleport went down, or the
  teleport point if it went up; a player teleported while grounded to a
  lower floor takes the whole height difference as a drop on the next
  grounded update.
- Drops recorded between two damage checks are added together before the
  check (the server checks after each standing-on update, so this only
  matters when a crouch resize records a landing in between; treat each
  landing as one drop).
- If `d > 50` and `d ≥ 300`, the player is not on a ladder, not an infected
  character, and not inside a safety-net volume:

```math
\text{damage} = 200 \cdot \frac{d - 300}{700 - 300} = \frac{d - 300}{2}
```

  as crush damage, upward direction, from the player itself. There is no
  upper clamp (a 1000 u drop does 350).
- A jump's height counts: jumping off a ledge measures from the apex.
- Walking down stairs or slopes never hurts: each grounded tick's small
  drop is checked on its own.
- The client separately marks "falling" (for sounds and animation only)
  when not in a jump and a 150 u ray straight down hits nothing.

### Volumes (containers)

The maps' volume objects (world_dat.md, "Moving world models and volumes")
act on the local player's movement on the client. Property defaults and
clamps are in world_dat.md; the ones that matter here: `Viscosity` (0;
liquids 0.035, `Ladder` 0.4), `Current` (0, 0, 0), `Friction` (1; `Ice`
0), `PlayerSpeedFactor` (1), `PlayerGravity` (`Gravity` only, -2000),
`Hidden` (switched by `ON` / `OFF`). A hidden volume is "not visible".

**Membership** (Open question 21). Three different tests are used:

- **The hull box.** The volumes whose brush the player's hull box overlaps
  (touching counts; the whole box, not its centre) are the ones that act on
  movement: ladder, viscosity, current, speed factor, ground friction,
  gravity volumes and jump pads. The server uses the same box test for
  volume damage.
- **The eye point.** Once per rendered frame the camera's position (the eye,
  Eye height) is tested against the visible volumes; the first one found
  is the player's *eye volume* (a `Filter` found first is replaced by a
  later one; `SafteyNet`, `JumpVolume`, `Weather` and dynamic-occluder
  volumes are never the eye volume). "Free movement" means the eye volume
  is a liquid (`Water`, `CorrosiveFluid`, `FreezingWater`) or a `Ladder`,
  **or** the hull box touches a visible ladder. Free movement drives the
  swimming and ladder controls, the crouch-rate reset and the stop on
  leaving; the camera's upward-smoothing pause (Eye height) uses the eye
  volume alone. Quirk: the eye test also skips `CorrosiveFluid` and
  `Ice` volumes (Quirks), so a corrosive pool never gives free movement.
- **The hull centre.** The server's safety-net test for fall damage looks
  for a visible `SafteyNet` at the hull's centre point.

**Per tick** (Per-tick order step 7b), before the ground check, with
gravity first switched on unless the player was on a ladder last tick:

1. **Ladders first.** If the hull touches any visible `Ladder`, the player
   is *on a ladder* for this tick: gravity is switched off, and the first
   such ladder's viscosity and speed factor are applied (once, however many
   ladders overlap). Every visible ladder's current is added to the
   current total.
2. **Every other visible volume**, in the order found: its viscosity, its
   gravity (if a `Gravity` volume) and its speed factor are applied, and
   its current is added to the current total.
3. **Jump pads** (also when switched off; the client does not check), see
   "Jump pads" below.

- **Viscosity** (Open questions 22 and 23), per volume, applied to the
  velocity at once: if the volume's viscosity `c > 0` and `|v| > 1`, the
  speed along the direction of motion drops by `5000·c·dt` (the velocity
  becomes 0 if that is not less than the speed); vertical included. It is
  skipped while the player is in a jump (from the tick after the jump tick
  through the tick on which the landing is detected, Jumping), except on a
  ladder. The jump tick itself and the tick after the landing tick are
  slowed. Viscosity values are also summed for the braking test.
- **Current**: `Current × dt` of every visible volume touched is added to
  the velocity at step 7g (after the jump), whatever the volume's viscosity
  and whatever the speed, even at rest; also while in a jump. (Non-player
  objects differ: grenades.md.)
- **Speed factor**: `f_container` in Speed limit is the product of the
  visible volumes' `PlayerSpeedFactor`s (the first ladder's only).
- **Ground friction**: `k = 5 × min(1, F)` with `F` the `Friction`s of all
  volumes the hull touches, **hidden ones included**, except `Filter` and
  `SafteyNet` volumes (and jump pads). Values are clamped to [0, 1] on load,
  so a volume can only lower friction. `k = 0` uses the frictionless
  formulas (Integration): a player holding a direction then ends each tick
  at `V + 45` and moves `V·dt + 3000·dt²/4`; braking still stops a player
  who lets go (test case 72).
- **Gravity volumes**: if any visible `Gravity` volume is touched, the
  player's gravity for the tick is the sum of their `PlayerGravity`s instead
  of -1100 (two overlapping -300 volumes give -600). It replaces gravity in
  the integrator and in the jump arc; the jump speed itself is unchanged.

**Ladders** (Open questions 20 and 25).

- Gravity is off on every tick the hull touches a visible ladder, and on
  the first tick after it stops touching one (the switch-on at the start
  of a tick looks at last tick's ladder state). With gravity off the
  integrator ignores the ground (Integration): no friction, and an upward
  acceleration is not pushed back into the floor. So a player standing on
  the floor at a ladder's foot, looking up and holding forward, simply
  rises; no jump or lift is needed (test case 56).
- Movement is free movement: forward is the camera's full direction
  (looking up climbs, looking down descends, looking level pushes along the
  floor or into the wall); right stays horizontal. The ramp, the 500 start
  and the jump factor work as on the ground. The crouch flag does not
  halve the acceleration on a ladder; instead it sets the acceleration's
  vertical part to -3000 (climb down), replacing whatever vertical part the
  forward/back keys gave, and as usual skips the ramp. The crouch rate is
  held at 0, sprint is impossible.
- Speed: there is **no horizontal speed limit** on a ladder (the stance
  clamp is skipped) and the upward speed is capped at a plain 200 at step
  7d, after the viscosity; penalties and speed factors do not change the
  cap, and downward speed is not capped. The stock ladder viscosity 0.4
  takes 30 u/s off per tick. Climbing straight up at full ramp settles at
  an end speed of 245, capped to 200 after viscosity at the start of each
  tick, and 3.16875 u per tick (211.25 u/s). The climb starts slowly: while
  the acceleration adds less than 30 u/s per tick (below 2000 u/s², the
  first 17 ticks) the viscosity wipes the previous tick's speed each tick.
- Letting go: no braking (the viscosity is not 0), no gravity; the
  viscosity alone stops the player within a few ticks, and they hang.
- **Jumping**: there is no jump on a ladder. The jump flag is never set
  while on a ladder, so the key does nothing (it does not even count as
  moving). A player leaves by moving out of the volume (back, strafe,
  looking away, climbing past its top or bottom) or climbing down.
- **Leaving**: the stance clamp applies again at once; gravity returns one
  tick later. The velocity is **kept**, unless the eye volume was a ladder
  or liquid on the previous tick and this tick is not free movement any
  more (the eye and the hull left together); then the whole velocity is
  zeroed at step 7d. Climbing out over a ladder's top therefore carries the
  climbing speed (up to 245 u/s) into a short hop (test case 59).
- Fall damage tracking is reset while on a ladder (Falling).

**Liquids and swimming** (Open question 24). `Water`, `CorrosiveFluid` and
`FreezingWater` are the liquid classes, and "free movement" is exactly the
liquid classes plus `Ladder` (by the tests in "Membership").

- Viscosity, current and speed factor act through the hull box like any
  volume. Player gravity is unchanged (-1100). With the stock water
  (viscosity 0.1, current (0, 1500, 0)) the current's +22.5 u/s per tick
  beats gravity's -16.5: a player at rest rises at 0.275625 u per tick
  (end speed 6.0 each tick) until the hull leaves the water, then drops
  back: they float at the surface. In still water (current 0) the player
  sinks, viscosity only slowing the fall a little.
- The speed limit is the normal stance limit, horizontal only; vertical
  speed is not limited.
- **Swim controls** (only in free movement by the eye test): forward is
  the camera direction with any upward part removed (then renormalized):
  looking down and holding forward swims down, looking up moves level at
  the full acceleration. The back key reverses it, so looking down and
  holding back rises. Right stays horizontal. Crouch sets the vertical
  acceleration to -1500 (the crouched base, halved as usual), replacing the
  forward part's vertical, and since crouch is set the speed limit is walk
  speed (70) and the run and jump flags are off. The 0.1 in-air factor is
  never applied in free movement.
- **Jump in a liquid**: the jump flag still needs ground (standing on the
  bottom, not in a jump) and a fresh press, as on land. Then it is a
  normal jump (`v_y = 330·j`, in a jump, `j` = 0.3) and, on that one tick,
  the vertical acceleration gets +3000 (scaled by `j` like any
  acceleration, with `j` as it was at the start of the tick).
  Holding the key does nothing more; there is no swim-up key. Jump-pad
  requests are dropped in free movement.
- The crouch rate is held at 0; the camera's upward smoothing is paused.
- **Getting out** over an edge has no special rule: the stair step (Slopes
  and steps) climbs a ledge up to 30.5 u above the hull's bottom, in the
  air as on land. When the eye leaves the liquid (and the hull touches no
  ladder) the velocity is zeroed once, at step 7d of that tick.
- There is no drowning and no breath.

**Jump pads** (`JumpVolume`, Open question 10). The pad's launch velocity
is `Velocity × forward`, with `Velocity` the speed (u/s, default 1000) and
`forward` the forward vector of the pad object's `Rotation` (world_dat.md).
It fires on its own, without the jump key: when the hull touches a pad and
the player was on ground on the previous tick (walkable, not a character,
not in a jump), the pad requests a jump. The jump then happens on this
tick if the player is on ground now and not on a ladder (Jumping): the pad
velocity is **added** to the velocity (horizontal and vertical; not
scaled by the jump factor), instead of setting `v_y` = 330. Otherwise it is
a normal jump: in a jump, `j` = 0.3, the ramp bypassed on that tick, and
the stance clamp applies to the horizontal part from the next tick on.
Overlapping pads: the last one found wins. A player standing on a pad is
relaunched on the tick after each landing is detected. A player who falls
or jumps into a pad is not launched until they land in it. Pads do not
work in free movement.

**Safety nets** (`SafteyNet`, Open question 26) take no part in the
physics: they are left out of the friction minimum and of the eye volume,
and their viscosity, current and speed factor are fixed at 0, (0, 0, 0)
and 1 in every observed map. Their only effect is the fall-damage test at
the hull's centre (Falling and fall damage). So a net's `Friction` 0 in a
map changes nothing. A `Gravity` volume's `Friction` 0 (in one gravity map
every gravity volume has it) does make the ground frictionless inside it.

**Damage volumes** (Open question 28). `Damage` is per second, dealt by the
server to a player whose hull touches the volume, of type `DamageType`
(world_dat.md). The damage type has no movement effect of its own: freezing
damage (`FREEZE`) only tints the screen blue, shows a frozen icon on the
HUD and sways the camera slightly; movement and input stay allowed. The
slowdown players feel in the observed freezing `DamageVolume`s comes from
those volumes' own properties (`PlayerSpeedFactor` 0.7 and `Friction` 0:
test case 74).

### Other modifiers (brief)

- **Pushers** (Open question 27). A pusher has a position, radius, start
  delay, duration and strength; the client keeps a list of them. Each tick
  (Per-tick order step 9) every pusher's delay counts down by `dt`; once
  it is used up, its time left counts down by `dt` too, and it is removed
  when that reaches 0. While active, if the hull centre is less than
  `radius` from the pusher and the straight segment between them hits
  nothing, the velocity gets
  `strength·(1 - dist/radius)` along the unit vector from the pusher to the
  hull centre (three-dimensional), a tenth of that if the player stands on
  nothing. The push repeats on every active tick. Then, if the vertical
  velocity is above 100, it is set to 100, also when it was already above
  100 before the push (a jumping player's 330 is cut to 100). Downward
  speed is not capped. Which pushers exist: the impact effect of
  full-metal-jacket bullets names a pusher record (`BulletExpPusher`:
  radius 0, delay 0.05 s, duration 0.5 s, strength 400), and a second
  record (`RocketPusher`: radius 0, 0.05 s, 0.5 s, 800) is not used by
  anything; with radius 0 neither ever pushes. Explosions do not push
  players. The only working pusher is sent by the server when an AI kicks
  a door open in front of its player target: at the door's position,
  radius 96, no delay, 1 s, strength 1000 (66 active ticks).
- **Movement disallowed** (Corrections 9): while the client locks
  movement (planting the bomb or installing a MOAB, game_modes.md; also
  some developer camera tools), the movement keys set no flags (sprint
  ends), but crouch and the jump key are still read, the crouch rate still
  updates and the hull still resizes. The motion step is replaced by:
  the stored acceleration (last tick's, gravity included) keeps only its
  vertical part, an upward part is removed and a downward part is limited
  to -1100; the velocity's horizontal part is halved and an upward part
  removed (no speed clamp, no volumes, no ground check, no jump). Braking
  (Stopping) then still runs on the halved velocity, using the on-ground
  state and volume totals left from the last normal tick, and the
  integrator adds gravity on top of the kept acceleration, so a player
  locked in the air falls at 2200 u/s². A jump pressed during the lock
  becomes a pending request (Jumping) and fires on the first unlocked tick
  on ground. While dying, or when the game disallows player movement
  altogether, velocity and acceleration are zeroed instead (step 7g).
- **Hit knock-back (infection mode only)** (Open question 19). When the
  local player is an infected character and takes damage from someone not
  on their team, with ammo whose `DamageMovePenalty` `K > 0` and
  `DamageMovePenaltyDuration` `T > 0` (attribute_data.md), a knock-back
  timer of `T·(1 + StiffDuration)` seconds starts (only if longer than the
  time left on the current one; the latest `K` is kept with it). While it
  runs:
  - the run and sprint flags are cleared each tick (speed limit = walk
    speed × penalty factors), and braking does not run;
  - after the normal acceleration is computed (and scaled by `j`), it is
    replaced by a push along the horizontal part of the last damage
    direction (the direction the damage travelled, away from the attacker):

```math
a = \hat d_{xz}\cdot K^2\,(1 - t)
```

    where `t` is the game time in seconds since the timer started (the same
    for every tick of a frame). With the crouch flag set (the flag, so the
    100 ms release hold and the no-headroom hold count, and a lock-out
    clears it) the push is × 0.25 when the moving flag is set (a movement
    key or the jump flag); with the crouch flag set and the moving flag off
    the velocity is zeroed and there is no push (Corrections 22);
  - a hit with no horizontal direction gives `a = 0`.
  Other characters are not affected: for a normal (non-infected) player the
  value does nothing.

### Eye (camera) height

(Open question 2.) The first-person camera, which is also the start of the
shot ray (weapon_firing.md) and of the pickup ray (inventory.md), is placed
each rendered frame at:

```math
y_{eye} = y_{centre} + 0.66\,H_{stand} - 13\,r
```

with `y_centre` the hull centre interpolated for the frame, `H_stand` the
standing half-height (from the model's stand animation; 50 with the
fallback box) and `r` the crouch rate. The eye offset is fixed when the
character is set up; it does not follow the current hull size. On the
ground the centre is `feet + h_y` with `h_y = H_stand - r(H_stand - H_crouch)`,
so the eye above the feet is

```math
e(r) = H_{stand} + 0.66\,H_{stand} - r\,(H_{stand} - H_{crouch}) - 13\,r
```

which is 83 u standing and 60 u fully crouched with the fallback boxes
(`83 - 23r`, linear in the crouch rate, so it follows the crouch curve).
That is 0.83 of the standing hull height (100) and 0.75 of the crouched
one (80), not 0.9.

- **Upward smoothing:** when the camera's wanted height is more than 1 u
  above its last height and the player is not in a jump (and not in a
  free-movement volume), the camera rises at
  `max((wanted - current)/0.1, 140)` u/s, but never lags more than 30 u
  (a larger step jumps to 30 u below first) and snaps once within 1 u.
  Downward moves are immediate. Standing up on the ground (centre rising
  about 2.6 u in the first tick) is therefore smoothed too.
- Head bob, camera shake from damage, and the recoil back-push
  (weapon_firing.md) are added on top; they are not part of this spec.

## Per-tick order

1. (Networked play) other characters and moving world models are rewound
   to the time the player sees, for collision only; restored at the end.
   (The world models' rewind runs only under the server's newer player-state
   protocol; the dedicated server's default is the legacy one:
   `world_objects.md` Follow-up (2026-10-31b) 2.2.)
2. Note the eye volume (from the camera point of the last rendered frame;
   Volumes, "Membership") and last tick's.
3. Update control flags from input (Input to control state, steps 1–9).
4. Update the crouch rate, then the crouch-spam values (using movement
   history up to last tick).
5. Resize the hull for the crouch rate.
6. Update the last direction speed `K`.
7. Motion step:
   a. if movement is disallowed, damp and skip to step 8;
   b. zero acceleration; process the volumes the hull touches (gravity on
      unless on a ladder last tick, ladders, viscosity, gravity override,
      speed factor, current totals, jump pads; Volumes);
   c. ground check from last tick's standing state; handle landing;
   d. compute the speed limit and clamp horizontal velocity (on a ladder:
      cap the upward speed at 200 instead); if last tick's eye volume was
      a liquid or ladder and this tick is not free movement, zero the
      velocity;
   e. compute the acceleration (base, crouch, air factor, ramp, sum, clamp,
      start rescale), then scale by the jump factor if below 1; while the
      infection-mode knock-back timer runs, replace it by the push;
   f. if a jump is requested (or pending) and allowed, set vertical
      velocity, start the jump, set the jump factor to 0.3; if one is
      requested but not allowed, keep it pending and leave the jump factor
      alone; otherwise recover the jump factor;
   g. add volume currents to velocity; if dying or movement is blocked,
      zero velocity and acceleration.
8. Braking when no movement keys are held (Stopping).
9. Pushers (Other modifiers).
10. Resize the hull for the crouch rate again.
11. Integrate with gravity and friction, move through the world with
    sliding and stair-stepping, update what the player stands on, apply the
    stuck-at-wall and blocked-vertical fixes.
12. Determine the "falling" status for sounds.
13. Record the tick (position, velocity, hull, flags) for the server and
    for the "still" history.

Stamina and the HUD update once per rendered frame after the ticks.

## Edge cases

- **Rising edge of moving while in the air** also uses the 500 start accel.
- **Reversal:** pressing forward on the tick after back (or the reverse)
  resets that axis to 500 even if both keys overlap; holding both keeps
  resetting it, and releasing one resets it once more (test case 80).
- **Late jump at a ledge:** a jump pressed one tick too late at a ledge
  does not jump in the air; it stays pending and fires by itself on the
  landing (Jumping, "Pending request"; test case 82).
- **Diagonal movement** is not faster: accel is capped at 3000 total and
  the speed limit applies to the horizontal magnitude.
- **Backpedal and strafe** use their own run speeds (all 285 by default);
  sprint needs forward, so strafing or backpedalling never sprints.
- **Zoomed** (scoped): speed limit is walk speed (70 × penalty factor) even
  while running or sprinting.
- **Crouch release with no headroom:** crouch stays forced on; the player
  stays crouched until there is room.
- **Pressing jump while standing up:** the press is consumed and no jump
  happens; it must be released and pressed again once fully up.
- **Landing:** no landing speed loss; horizontal velocity carries into the
  ground tick. The vertical velocity is zeroed on the first tick whose
  downward move is fully blocked (usually the tick after contact).
- **Standing on another player** is not ground (cannot jump off heads,
  braking does not apply), though the hull does rest there.
- **Speed limit change mid-air** applies immediately (no air speed is kept
  above the current limit).
- **Penalty clamp:** `P` is clamped to [-1, 1], so a speed can at most
  double from penalties (negative penalties exist on some vests).
- **Frame hitch:** a frame longer than 100 ms runs only 6 ticks; the rest
  of the time is dropped.
- **Spawn:** the client's local hull is placed at the server's last
  acknowledged position before the first tick.

## Quirks

- **Speed above the limit.** The clamp comes before integration, so the
  real speed is about 4 % above the nominal run speed (296.5 vs 285) and
  about 28 % above the nominal walk speed (89.4 vs 70). Keep: this is the
  game's feel.
- **Quarter-step displacement in the air.** Without friction the engine
  moves `v·dt + a·dt²/4` instead of `/2`. Jumps go slightly higher
  (50.74 u instead of the 49.5 u the continuous formula gives) and fall
  slightly slower. Keep.
- **Strong air control.** The in-air factor is overwritten by the ramp, so
  air acceleration is the full 3000 (times the jump factor). Only a player
  crouching during a jump gets the reduced 150, times the jump factor. Keep.
- **Forward + back + sprint drains stamina.** Holding all three drains the
  gauge at the sprint rate without sprinting (Sprint and stamina). Keep; it
  is harmless and matches the original.
- **Ramp memory across crouch.** Crouching freezes `L_f`/`L_r`, so standing
  up while holding forward resumes at the old ramp value instead of 500.
  Keep.
- **Jump factor recovery while airborne.** The 0.3 jump factor recovers
  during the jump itself, so on flat ground the next jump is full height;
  it only bites on short hops (landing on something higher). Keep.
- **Lock-out refill ignores its "in penalty" timing.** The configuration has
  separate refill values for while locked out, but only the separate cost
  (0.9) is ever used; refill always uses the normal 1.5 s. Keep (it is what
  players experienced).
- **Lock-out uses the slowdown's still exception flag.** Both flags are 1 in
  the shipped config, so there is no visible difference.
- **Jump stamina on the client.** With the custom-room jump-stamina option,
  the client never subtracts the cost and never blocks the jump; only the
  server subtracts it. In normal play the cost is 0. Implement the server
  behaviour (subtract on jump) and no client block.
- **Stamina at zero while holding sprint is pinned to 0** until the key is
  released; players learn to let go of sprint to recover.
- **Sprint speed waits for the server.** Locally the sprint flag is set at
  once, but the higher speed limit applies only after the server echoes
  sprint on. In a local or listen game this is immediate; keep a one
  round-trip delay only if we simulate latency.
- **Crouch release delay** is measured on the wall clock (100 ms), not in
  ticks; a tap holds crouch for at least 7 ticks.
- **No horizontal speed limit on a ladder.** Only the upward speed is
  capped (at 200); sideways and downward the acceleration (45 u/s per tick
  at full ramp) beats the ladder's viscosity (30 u/s per tick), so speed
  keeps growing while the hull stays in the volume. Keep; ladder volumes
  are narrow, so it rarely shows.
- **Ladder top hop.** Leaving a ladder keeps the velocity and gravity
  returns one tick late, so climbing out over the top carries up to
  245 u/s upward into a short hop. Keep.
- **The eye test skips corrosive fluid and ice.** The set of volume
  classes the eye test ignores was meant to leave out generic volumes, but
  is built from a class number instead of its flag, which also leaves out
  `CorrosiveFluid` and `Ice` (and lets generic `VolumeBrush`es be the eye
  volume). So a corrosive pool never gives swimming or the crouch reset;
  its viscosity, current and damage still act through the hull. Keep.
- **Switched-off volumes still lower friction.** The ground-friction
  minimum does not check whether a volume is hidden. Keep.
- **The pusher's upward cap also cuts speeds already above it**: any push,
  however small, sets a vertical speed above 100 to 100. Keep (with no
  working explosion pushers it only matters for the AI door kick).
- **Bullet impacts carry a pusher of radius 0**, which can never reach
  anyone, so bullets and explosions do not push players. Keep (do not
  invent a radius).
- **Pending jump.** A jump that could not happen stays requested and fires
  on the next landing, with full unramped air acceleration meanwhile
  (Corrections 6). Keep.
- **Partly crouched players always stand up.** The no-headroom hold only
  tests a fully crouched hull, so releasing crouch halfway under a low
  ceiling grows the hull into the ceiling (forced overlap) (Corrections
  16). Keep.
- **Bomb-plant lock stops a runner in about three ticks** (velocity halved
  and braked each tick) and makes an airborne player fall at double
  gravity (Corrections 9). Keep.
- **Jump pads keep their horizontal part for one tick only.** The stance
  clamp applies from the next tick, so a pad's horizontal launch (the dam
  map's pads, whose direction points 45° down toward -Z by the rotation
  reading in world_dat.md, Open question 29) moves the player about 20 u
  on the launch tick and is then cut to the run speed. Keep.

## Corrections (2026-10-19)

A behaviour-by-behaviour comparison of the reference's movement (client
movement manager, engine integrator and player mover, server player
object) with this spec and with our implementation
(`games/combat_arms/src/game/movement.rs`, `volumes.rs`,
`data/movement.toml`). The reference is the truth. "Spec" means this file
before these corrections; the body above is already fixed. Ordered by how
much they change play. Each names the test cases that check it (new ones
from 80 on).

1. **Ladders are climbed from the ground** (implementation). Reference: on
   every tick the hull touches a visible ladder, and on the first tick
   after it stops touching one, the player's gravity is switched off, and
   with gravity off the integrator skips all ground handling: no
   push-into-surface of an upward acceleration, no ground friction, no
   slope projection; `v ← v + a·dt`, `Δx = v·dt + a·dt²/4`, and it stops
   only when `|v|²` and `|a|²` are both below 0.1. So a player standing at a
   ladder's foot, looking up and holding forward, rises at once (500 u/s²
   on tick 1). Ours: gravity is set to 0 on a ladder
   (movement.rs:1135-1136) but the ground branches still run whenever the
   player stands on something (movement.rs:1370-1398): the upward
   acceleration is pushed back into the floor and friction applies, so a
   ladder can only be entered from the air; and gravity returns on the
   very tick the ladder is left (no one-tick delay). Matters: every ladder
   in every map. Numbers: test cases 56 (climb from the floor: 7.5, 8.85,
   10.2 u/s after ticks 1-3), 57, 59 (top hop: 3.675 u then 3.613125 u).

2. **A volume acts when the hull box touches it** (implementation).
   Reference: the volumes acting on movement are found by overlapping the
   player's whole axis-aligned box (half-extents 15 × h_y × 15 around the
   centre; touching counts) with each volume's brush. Ours: a volume acts
   only when the hull's centre is inside (movement.rs:962-963,
   volumes.rs:164-170). Matters: ladders (a player walking up to a ladder
   touches it 15 u before the centre gets there; a ladder volume thinner
   than the hull may never contain the centre), shallow water (a 20 u deep
   pool slows the legs: case 62), jump pads, friction and gravity volumes
   all start and stop up to 15 u (sideways) or `h_y` (vertically) early
   or late. Also from the reference, for the same lists: the speed factor
   and viscosity of only the **first** touched ladder count (ours
   multiplies every ladder's factor, movement.rs:1142); braking is blocked
   by the **sums** of current (as `current × dt`) and viscosity of the
   visible volumes, so two opposite currents that cancel do not block it
   (ours blocks on any non-zero one, movement.rs:1309). Test cases 62, 69,
   92.

3. **Ladder speed and controls** (implementation). Reference: on a ladder
   there is no horizontal speed clamp at all; only the upward speed is
   capped, at a plain 200 u/s (no penalty, no speed factor, no downward
   cap); the crouch flag sets the vertical acceleration to -3000 (climb
   down) and does not halve the acceleration. Ours: the horizontal speed is
   clamped to `200 × (1 - P) × f_container` and the upward cap is applied
   as well (movement.rs:1164, 1173-1183), and crouching on a ladder only
   skips the halving, with no downward acceleration (movement.rs:1195,
   1204). Matters: with the reference loadout ours caps ladder sideways
   speed at 160 instead of none; crouch does nothing on our ladders
   instead of sliding down at -45 u/s per tick. Test cases 56, 58, 61.

4. **Free movement (swimming) follows the eye** (implementation).
   Reference: "free movement" is the camera point being in a liquid
   (`Water`, `FreezingWater`; `CorrosiveFluid` never, Quirks) or a ladder,
   or the hull touching a visible ladder. In free movement: forward is the
   camera direction with any upward part removed and renormalized (looking
   down swims down), crouch sets the vertical acceleration to -1500 (base
   halved), a jump from the bottom also adds +3000 upward acceleration on
   its tick, the 0.1 in-air factor never applies, jump-pad requests are
   dropped, and the crouch rate is held at 0. When last tick's eye volume
   was free movement and this tick is not, the whole velocity is zeroed
   once (after the speed clamp). Ours: none of the swim controls exist;
   the crouch rate is held at 0 whenever the hull's **centre** is in any
   liquid (movement.rs:965, 1063), so a player in waist-deep water cannot
   crouch although the reference lets them; the velocity is zeroed when the
   centre leaves a liquid (movement.rs:1125-1127), not when the eye leaves
   a liquid or ladder. Matters: water maps (can't dive, can't sink, can't
   crouch in shallow water, wrong stop when climbing out). Test cases 60,
   62, 65-69.

5. **Jump pads fire by themselves, along the pad's rotation**
   (implementation). Reference: a pad whose box the hull touches requests
   a jump whenever the player was on ground last tick and not in a jump
   (no key needed; switched-off pads too), and the launch is
   `Velocity × forward(Rotation)` added to the whole velocity, not scaled
   by the jump factor. Ours: only a jump-key jump inside a pad launches,
   and the launch is straight up (movement.rs:1279-1281,
   volumes.rs:111-114, 198). Matters: on the jump-pad maps, walking onto a
   pad must throw the player; ours needs a key press and ignores the
   pad's direction. Test cases 70, 71.

6. **A jump request stays pending** (spec gap and implementation).
   Reference: the jump request set on the jump flag's rising edge is a
   remembered state. It is cleared only by a jump that happens, by any tick
   of free movement and on entering a world. If at the motion step the
   player is not on ground (or is on a ladder, or dead), nothing happens
   and the request stays. While pending: the acceleration ramp is skipped
   every tick (each axis gets the base 3000, no 500 start, no ramp, `L_f`
   and `L_r` untouched; not in a jump, so no 0.1 factor), the jump factor
   does not recover, and the jump fires by itself on the first tick the
   player is on ground again. Because the jump flag reads the on-ground
   state one tick older than the motion step, pressing jump on the tick
   after the move that carried the hull off a ledge creates exactly this.
   Ours: the request is a one-tick event (movement.rs:1052, 1277); a late
   press is lost. Matters: edge jumps a tick late turn into an automatic
   hop on landing in the reference, with strong air control during the
   fall; a jump pressed while planting fires when the plant is cancelled
   (Correction 9). Test case 82.

7. **Opposing keys and reversals reset the ramp every tick** (spec and
   implementation). Reference: the forward axis is reset to 500 whenever
   forward is held now and back was held last tick, or back now and
   forward last tick, regardless of the other key; the strafe axis the
   same with left and right. Holding both keys therefore resets the axis
   on every tick, and releasing one of them resets it once more. Spec: said
   `L_f` keeps ramping to 3000 while both are held (Acceleration; case 51).
   Ours: a reversal needs the newly held key to have been up last tick
   (movement.rs:1220, 1229), so holding both keeps ramping and releasing
   back while holding forward goes straight on at 3000. Matters: after a
   forward+back hold (a common stop-and-go habit) the reference restarts
   slowly (500, 590, 680 u/s²), ours at 3000: 36 u/s more speed on the
   first tick. Test cases 51 (corrected), 80.

8. **The crouch "press" is the crouch flag, holds included** (spec
   ambiguous, implementation). Reference: the press time for the 100 ms
   release hold is set only when the crouch flag turns on after being off
   last tick; the crouch-spam "key press" is the crouch flag (after the
   100 ms hold and the no-headroom hold, before the lock-out clears it)
   turning on. So a release and re-press inside the 100 ms hold is one
   press, and it does not extend the hold. Ours: the raw key is used for
   both (movement.rs:973-988, 1096), so fast taps are charged twice and the
   hold restarts. Matters: crouch spam: ours locks out sooner (two quick
   taps cost 0.8 of the lock-out gauge instead of 0.4, and 1.0 of the
   slowdown gauge instead of 0.5). Test case 85.

9. **Movement disallowed (bomb plant)** (spec incomplete, implementation).
   Reference: while the client locks movement (planting the bomb,
   installing a MOAB): no movement-key flags; crouch, crouch rate, hull
   resize and the jump key still run; the motion step is replaced by
   "horizontal velocity halved, upward velocity removed, stored
   acceleration reduced to its vertical part with any upward part removed
   and a downward part limited to -1100"; braking still runs afterwards
   (using the last normal tick's on-ground state and volume totals); the
   integrator then adds gravity on top, so airborne the fall is 2200 u/s².
   A jump pressed during the lock stays pending (Correction 6). Spec: only
   the halving, and named it for freezes and round start. Ours: not
   modelled (movement.rs:88); the round-setup freeze masks the movement
   keys (rules `frozen_intent`), which leaves normal braking. Matters: a
   runner who starts a plant is at rest from the third tick after
   1.73 u in the reference, after 6 ticks and 9.4 u with braking alone
   (case 9). Test case 88.

10. **Viscosity is skipped for the whole jump** (implementation).
    Reference: a volume's viscosity is neither applied nor added to the
    braking total on any tick the player is in a jump (from the tick after
    the jump tick through the landing tick), except on a ladder; it acts
    only when the speed is above 1 u/s. Ours: skipped only on the tick
    after the jump tick (movement.rs:1128-1134), and applied at any speed
    (a speed below 1 u/s is zeroed). Matters: jumping out of water or
    viscous volumes: in ours the jump is slowed from its second tick (a
    330 jump in stock water loses 7.5 u/s per tick) and is much lower.
    Test case 64.

11. **Ground friction from volumes** (implementation). Reference: `k = 5 ×
    min(1, Friction)` over every touched volume except `Filter`,
    `SafteyNet` and jump pads, **hidden ones included**; `k = 0` uses the
    frictionless formulas, with the quarter-step displacement
    `v·dt + a·dt²/4`. Ours: hidden volumes are left out (they are not
    "contained", volumes.rs:166), `Filter` counts (movement.rs:1403), and
    `k = 0` uses `a·dt²/2` (movement.rs:1410-1412). Matters: ice and
    freezing-water floors (switched-off ice still slippery in the
    reference); per-tick distance on ice 4.44375 u vs ours 4.6125 u at a
    steady run (`P` = 0). Test cases 72, 73.

12. **Pushers last and cap hard** (implementation). Reference: a pusher
    has a start delay and a duration; it pushes on every tick while active
    (`strength·(1 - d/radius)` from the pusher to the hull centre, a tenth
    when standing on nothing), and after each push an upward speed above
    100 is set to 100 even if it was already above. Ours: a pusher pushes
    once at its time and the cap only limits the part the push added
    (movement.rs:1328-1356). Matters only for the AI door kick (radius 96,
    1 s, 1000), the only working pusher. Test case 77.

13. **Sliding: two planes at most, no floor projection** (spec and
    implementation). Reference: the mover slides along the first surface
    with a 1.001 deflection, along the crease of the first two
    non-parallel surfaces, and stops at a third contact (or a second one
    parallel to the first); the floor plane is not used to steer the low
    path; and on a steep floor a move whose on-plane projection goes up
    is replaced, vertical part included, by its part along the slope's
    level direction. Spec: said "up to 5 planes" and left the floor plane's
    use open. Ours: avian projection over up to 5 planes
    (movement.rs:689-715), the low path projected onto the floor plane
    (movement.rs:800-803), and the steep-slope cut keeps the vertical part
    and uses the horizontal normal (movement.rs:787-796). Matters: corners
    and wedges (the reference stops dead where ours keeps sliding),
    behaviour on ramps and steep slopes. Test cases 87, 94.

14. **Braking keeps the vertical speed** (spec and implementation).
    Reference: when braking takes the horizontal speed to zero or past it,
    only the horizontal part is zeroed; the vertical speed is kept. The
    whole velocity is zeroed only when the horizontal speed is already 5
    or less. Spec and ours (movement.rs:1314-1316): the whole velocity.
    Matters little: on flat ground the vertical speed is zeroed by the
    blocked-move rule anyway; on a downhill slope the player keeps sinking
    onto it. Test case 81.

15. **The 500 start keeps the unramped forward direction** (spec gap,
    implementation). Reference: on the tick the moving flag turns on, the
    forward axis is not ramped and keeps the base magnitude `A` (3000; 300
    in a jump; 1500 crouched never reaches here), the strafe axis is
    ramped (590), the sum is clamped to 3000 and then rescaled to 500. So
    a diagonal start is mostly forward on the ground and mostly sideways in
    a jump. Ours: the forward axis uses 500 (movement.rs:1224), giving a
    different direction. Matters: one tick of direction (a few u/s);
    visible in air strafing starts. Test case 83.

16. **The no-headroom hold only tests a fully crouched hull** (spec and
    implementation). Reference: when crouch is released, the "can stand
    up" test returns yes at once if the hull is taller than the crouch box
    (`r < 1`); only a fully crouched hull is test-resized to standing
    size. Ours: tests the standing hull at any crouch rate
    (movement.rs:986), and also while the flag was already off. Matters:
    releasing crouch halfway under a low ceiling stands up into the
    ceiling (forced overlap) in the reference. Test case 86.

17. **Stuck-at-wall uses the movement's on-ground flag** (spec ambiguous,
    implementation). Reference: the horizontal velocity is zeroed after a
    fully blocked move with both horizontal components above 0.01 when the
    movement's "on ground" flag of this tick (set before the move) is
    false. Ours: tests that nothing is stood on after the move
    (movement.rs:1425). Matters: a player in a jump wedged on a ledge, on
    a steep slope or on another player's head keeps its horizontal speed
    in ours. Test case 89.

18. **"On ground" uses 0.707** (spec and implementation). Reference: the
    movement's on-ground flag (jump allowed, braking, landing) needs a
    normal with `n.y ≥ 0.707`; the integrator and the mover use 0.7071.
    Ours: 0.7071 everywhere (data/movement.toml:68, movement.rs:1149).
    Matters only for surfaces between 45.0° and 45.004°. Test case 90.

19. **Standing on a model is flat for the integrator** (spec gap,
    implementation). Reference: the integrator uses the stood-on world
    polygon's plane; standing on a model (another character, a prop
    model) it uses (0, 1, 0): never steep, ground friction applies (the
    movement still does not count it as ground: no jump, no braking).
    Ours: the normal of the probe's contact (movement.rs:1372, 1384), so a
    rounded or tilted model can make the player slide. Test case 93.

20. **Client authority with server collision** (spec gap; ours differs by
    design). Reference: only the client runs these rules; the server moves
    its copy by the client's per-tick displacement through its own
    collision and snaps to the client's position within tolerance; the
    server's own stamina gauge (record durations clamped to 100 ms) decides
    when sprint ends; fall damage is judged on the server from positions.
    Ours: the server re-runs the full movement from inputs and the client
    predicts (net.rs:16). Keep ours, but the sprint end must still come
    from a gauge advanced per tick with at most 100 ms per step, and a
    client's sprint speed must drop when the authority ends sprint. Test
    case 91.

21. **Mode speed factors** (spec gap). Reference: after the penalty factor
    the limit is scaled by a mutation-item buff or nerf and, for humans in
    the infection mode when the infected side has an infected mercenary,
    that perk's speed rate (stamina capacity likewise). 1 outside those
    modes. Ours: none. Values belong to the game-mode specs.

22. **Knock-back reads the crouch and moving flags** (spec and
    implementation). Reference: the crouched knock-back variants use the
    crouch **flag** and the **moving** flag (movement keys or the jump
    flag). Spec said "crouch held" and "a movement key"; ours uses the raw
    crouch key and the four movement keys (movement.rs:1265-1266). Matters
    in the infection mode only: a crouch tap held on by the 100 ms hold
    still quarters the push in the reference.

Follow-up brief (movement in volumes), questions 1-10: re-checked against
the reference; the answers in Open questions 10 and 20-28 stand. What our
implementation still lacks for them is Corrections 1-5 and 10-12.

## Test cases

All on flat ground (normal (0, 1, 0)), `k = 5`, `dt = 0.015`, standing hull,
`P = 0` and no gear bonuses unless stated, always-run on, facing +Z.
Numbers rounded to 4 decimals where not exact.

1. **Run start, tick 1.** At rest; hold forward from tick 1. After tick 1:
   accel 500, speed 7.2257 u/s, distance 0.0549 u.
2. **Run ramp.** Same, after tick 2: accel 590, speed 15.2298, total
   distance 0.2240. After tick 3: accel 680, speed 23.9563, distance 0.5187.
   After tick 10: accel 1310, speed 101.3233, distance 6.8853.
3. **Ramp reaches max.** Same: accel is 2930 on tick 28 and 3000 from
   tick 29 on.
4. **Steady run.** Running at 285 u/s with accel 3000. Per tick: end speed
   307.7608 (clamped back to 285 at the next tick's start), distance
   4.44784 u (296.523 u/s). From rest, total distance after 60 ticks:
   208.9518 u.
5. **Steady walk.** Hold the walk key and forward, steady state: end speed
   108.2960, distance 1.34081 u per tick (89.387 u/s).
6. **Steady crouch walk.** Hold crouch and forward, fully crouched, steady:
   accel 1500, end speed 68.0641, distance 0.88717 u per tick
   (59.145 u/s).
7. **Reference loadout caps.** `P = 0.2` (vest 0.1, rifle 0.1), vest sprint
   bonus 0.4, figure sprint 1.0: limits run 228, sprint 319.2, walk 56,
   crouch 40. Steady distances per tick: run 3.62412, sprint 4.94207, walk
   1.13849, crouch 0.74266.
8. **Weapon caps.** Run limit with only a weapon penalty: knife (0) 285;
   pistol (0.03) 276.45; M4A1 (0.10) 256.5; AK-47 (0.13) 247.95; MK.48 (0.30)
   199.5. With `P = 1.2` the clamp gives `P = 1`: limit 0.
9. **Stop.** Steady run (speed 307.7608 at the end of the last held tick),
   release all keys. End speeds per tick: 224.7459, 168.8455, 116.9843,
   68.8703, 24.2330, then 0 on tick 6. Distance covered over ticks 1–5:
   9.4034 u; none on tick 6.
10. **Opposing keys.** Hold forward and back from rest: accel 0, moving flag
    on, no braking, speed stays 0, no start accel rescale.
11. **Diagonal.** Hold forward + right at steady state: total accel 3000
    (clamped), horizontal speed limit 285; per-tick distance 4.44784 u, the
    same as straight.
12. **Reversal.** Steady forward run, then switch to back only. Tick 1:
    back accel 500 (axis reset); tick 2: 590.
13. **Jump arc.** At rest, press jump on tick 1. After tick 1: height
    4.888125 u, vertical speed 313.5. After tick 2: 9.52875, 297.0. After
    tick 20: apex 50.7375 u, vertical speed 0. After tick 21: 50.675625,
    -16.5. Tick 41 is the first tick that reaches the floor (it starts at
    2.475 u and would move -5.011875). Landing (jump ends) is detected on
    tick 42.
14. **Jump factor.** Values at the start of each tick after a jump on tick
    0: tick 1: 0.3, tick 2: 0.318, tick n: `0.3 + 0.018(n - 1)`; tick 39:
    0.984; tick 40: 1.002; tick 41 on: 1.0.
15. **Air accel after a jump.** Steady run, jump on tick 0: the jump tick's
    horizontal end speed is 330 (accel 3000, no friction), distance 4.44375 u.
    Tick 1: clamped to 285, accel `3000 × 0.3 = 900`, end speed 298.5,
    horizontal distance 4.325625 u.
16. **Hold jump.** Hold jump continuously: exactly one jump. After landing,
    release on one grounded tick and press on the next: the second jump
    happens on that press tick (earliest: the third tick after the contact
    tick, tick 44 in case 13).
17. **Jump while standing up.** Fully crouched; release crouch and hold jump
    from the same tick: no jump during the 10 stand-up ticks nor after; a new
    press after full standing jumps.
18. **Air fall.** At rest in the air, no keys: after 20 ticks fallen
    48.2625 u, vertical speed -330; after 49 ticks 294.0919 u; after 50
    ticks 306.2813 u, -825 u/s.
19. **Air control from rest.** In the air (walked off, not a jump), hold
    forward: tick 1 accel 500, horizontal speed 7.5, distance 0.028125; tick
    2 accel 590, speed 16.35, distance 0.173813 total; tick 3 accel 680,
    speed 26.55.
20. **Crouched air control.** In the air, holding crouch and forward
    (crouch flag set): accel `3000 × 0.5 × 0.1` = 150 only if in a jump
    (speed +2.25 u/s per tick); if walked off (not in a jump), 1500
    (+22.5 u/s per tick). *Correction:* the in-jump figure assumes a jump
    factor of 1, which after a jump only holds from tick 40 on; before that
    the 150 is scaled by `j` (case 37). The walked-off figure is right.
21. **Crouch rate.** Hold crouch from standing, full speed: rate after
    ticks 1–10: 0.259615, 0.463208, 0.618555, 0.733433, 0.815619, 0.872890,
    0.913022, 0.943794, 0.972982, 1.0. Release when full: rate after ticks
    1–10: 0.740385, 0.536792, 0.381445, 0.266567, 0.184381, 0.127110,
    0.086978, 0.056206, 0.027018, 0.0.
22. **Crouch hull and speed.** Fallback boxes (half-heights 50 and 40). Tick
    1 of crouching: half-height 47.40385; on ground the centre moves down
    2.59615 so the feet stay. Speed limit tick 1 (P = 0): 64.8077; when full,
    50.
23. **Crouch-jump.** Jump from rest; start holding crouch on tick 20 (apex).
    After 10 crouch ticks the half-height is 40 and the centre is on the same
    path as without crouching, so the feet are 10 u higher than an uncrouched
    jump at the same tick.
24. **Crouch tap delay.** Press crouch for one tick at wall time 15 ms,
    release at 30 ms: crouch flag stays set until wall time reaches 115 ms
    (ticks at 15…105 ms keep it), cleared at the tick at 120 ms.
25. **Crouch spam (shipped config).** Running on flat ground (not still).
    Crouch key held for ticks 1–7, up 8–14, held 15–21, up 22–28, held 29–35,
    up 36–42, held 43 on. Lock-out value after tick 1: 0.6; after tick 14:
    0.73; after 15: 0.33; after 28: 0.46; after 29: 0.06; after 42: 0.19;
    after 43: 0 and lock-out begins. Slowdown value: 0.5 after tick 1, 0 after
    tick 15. Crouch rate (slowed from tick 2 on, since the slowdown value is
    read before it is updated): 0.259615 after tick 1, 0.680425 after tick 7,
    0.163968 after tick 14, 0.259615 after tick 15, 0.207692 after tick 28,
    0.259615 after tick 43, 0 after tick 44 (locked out: stands up at full
    speed); crouch flag cleared from tick 45 while held. Lock-out ends after about 1.5 s on the ground (100
    ticks at +0.01, ±1 for float rounding).
26. **Crouch spam while still.** Same key pattern, standing still: both
    values stay 1, no lock-out.
27. **Sprint drain.** Reference character (capacity 100), full gauge, hold
    forward + sprint for 1.000 s: gauge 70. Sprint flag drops when the gauge
    falls below 1, after 3.3 s; while the key stays held the gauge reads 0.
28. **Sprint recover.** Gauge 0, sprint released, 2.000 s: gauge 50; full
    (100) after 4.000 s, then stays at 100.
29. **Sprint blocked.** With a full gauge and sprint held: walk key held →
    no sprint; crouch held or crouch rate > 0 → no sprint; back held → no
    sprint; strafe only (no forward) → no sprint; on a ladder → no sprint;
    zoomed → sprint flag may be on but the limit is walk speed.
30. **Sprint speed.** Reference character with `P = 0` instead (no weapon
    penalty, vest penalty removed): sprint limit `285 × 1.0 × 1.4 = 399`;
    steady end speed 413.5236, distance 6.09529 u per tick (406.353 u/s).
31. **Fall damage.** Server, landing after a drop of: 299 u → none; 300 u →
    0 (no damage); 400 u → 50; 500 u → 100; 700 u → 200; 1000 u → 350. On a
    ladder or in a safety-net volume: none.
32. **Jump-off fall.** Jump from rest at the edge of a 300 u ledge, reach
    the apex (50.7375 u up) before falling: drop 350.7375 u, damage 25.36875.
33. **Stairs down.** Walk down a staircase of 20 steps of 16 u: no damage
    (each grounded tick's drop < 300).
34. **Step up.** Run into a 30 u high box: the player ends on top without
    jumping. A 31 u box blocks (no step, since 31 > 30.5).
35. **Slope.** Ground with normal.y 0.70: not ground (jump ignored, braking
    off, slides down). Normal.y 0.71: ground.
36. **Tick clock.** A 100 ms frame runs 6 ticks; a 200 ms frame also runs 6
    (the other 100 ms is dropped); a 20 ms frame after a 10 ms remainder
    runs 2 ticks and leaves 0 ms (interpolation factor 0).
37. **Crouched air control after a jump (corrects 20).** At rest
    horizontally, jump on tick 0, crouch flag set from tick 1, forward held.
    Horizontal acceleration `150·j` with `j` from case 14: tick 1: 45 u/s²
    (+0.675 u/s); tick 20: 96.3 (+1.4445 u/s); tick 39: 147.6 (+2.214 u/s);
    tick 40 on (`j = 1.002`, not scaled): 150 (+2.25 u/s). No start rescale
    and no ramp on any of these ticks.
38. **Crouched start from rest.** Fully crouched (`r = 1`), at rest on flat
    ground, hold forward from tick 1. Acceleration 1500 on every tick (no
    500 start, no ramp). End speeds: tick 1: 21.6770, tick 2: 41.7876,
    tick 3: 60.4451 u/s; total distance after tick 3: 1.41097 u. Tick 4
    starts above the 50 u/s limit, is clamped to 50 and ends at 68.0641
    (the steady state of case 6).
39. **Jump-request tick bypasses the ramp.** At rest, no keys on tick 0;
    press forward and jump on tick 1. Tick 1 (jump tick): horizontal
    acceleration 3000 (`j = 1` at the start of the tick, no 500 rescale),
    no friction, horizontal end speed 45 u/s. Tick 2: the moving flag was
    already on, so no rescale; `L_f` was left at 500 by tick 0, so the ramp
    gives 590, times `j = 0.3`: 177 u/s², +2.655 u/s.
40. **Crouch reversal halfway.** Standing, room to stand. Crouch flag set
    on ticks 1–5, cleared from tick 6 (after the release delay). Rate after
    ticks 1–5: 0.259615, 0.463208, 0.618555, 0.733433, 0.815619; after ticks
    6–10: 0.556003, 0.352411, 0.197064, 0.082186, 0.0. Half-heights
    (fallback boxes): 47.40385, 45.36792, 43.81445, 42.66567, 41.84381, then
    44.43997, 46.47589, 48.02936, 49.17814, 50.0. On the ground the feet stay
    at the same height on every tick (centre = feet + half-height).
41. **Crouch re-pressed while standing up.** As case 40, but the flag is
    set again from tick 8. Rate after ticks 6–13: 0.556003, 0.352411,
    0.612026, 0.815619, 0.970965, 1.0, 1.0, 1.0 (the down time restarts at
    0 on tick 8).
42. **Standing up on the ground keeps the feet.** Fully crouched on a floor
    at y = 0 (centre 40, half-height 40), release with room. After stand
    tick 1 (`r = 0.740385`): half-height 42.59615, centre 42.59615, feet at
    0. After tick 10: half-height 50, centre 50, feet at 0.
43. **Standing up in open air.** Crouched (half-height 40), centre at
    y = 200, far from any surface, `r` going 1 → 0 over ticks 1–10. The
    centre stays at 200 (gravity aside); the feet go from 160 down to 150
    and the head from 240 up to 250.
44. **Standing up just above the floor.** Hull in the air with its feet
    1.0 u above the floor; a tick grows the half-height by 2.0. The bottom
    ends on the floor (within the 0.1 u clearance) and the centre rises by
    about 1.0 instead of staying put.
45. **Not a step.** Walk into a 20 u high box that is another player
    (character) or a moving object: no step-up, the player is blocked.
    The same 20 u box as plain world geometry: stepped onto (case 34).
46. **Ledge on the flat bottom.** Hull radius 15 resting with its centre
    14 u beyond a ledge's edge (the bottom overlaps the ledge by 1 u): the
    player stands (normal (0, 1, 0)), is on ground and can jump. With the
    centre 15.2 u beyond the edge (no overlap), the player falls.
47. **Air spawn.** Spawn point 200 u above the floor below it: the player
    starts on the floor, highest Y = that floor's Y; no fall damage.
48. **Teleport down (no reset).** Grounded at y = 0, then placed on a floor
    at y = -400 by a teleport: on the next grounded update the drop is 400,
    damage 50.
49. **Walk and sprint together.** Full gauge, hold forward + walk + sprint:
    no sprint flag, gauge state recovering (stays at 100), speed limit 70,
    steady 89.387 u/s (case 5).
50. **Forward + back + sprint.** Gauge 100, hold all three for 1.000 s: no
    sprint (limit = run speed), acceleration 0 on the forward axis, gauge
    70 (drains at 30/s).
51. **Opposing keys from a run.** Steady run (end speed 307.7608), then hold
    forward + back. Tick 1: clamped to 285, end speed 264.4069, distance
    4.11862 u; tick 2: 245.3018; tick 3: 227.5771 (friction only, no
    braking). *Corrected (Corrections 7):* `L_f` is reset to 500 on each of
    these ticks, not kept ramping at 3000 (test case 80).
52. **Eye height.** Fallback boxes, on the ground, feet at y = 0: standing
    eye at 83.0; fully crouched 60.0; at `r = 0.5`, 71.5. Case 40's ticks
    1–5 give eye heights 77.0288, 72.3462, 68.7732, 66.1310, 64.2408.
53. **Camera smoothing up a step.** Standing, 60 fps, the hull steps up
    30 u in one tick (not in a jump). Camera height above its start after
    frames 1–10: 5.0, 9.1667, 12.6389, 15.5324, 17.9437, 20.2770, 22.6103,
    24.9437, 27.2770, 30.0 (snapped).
54. **Unit scale.** A HUD distance readout for a target 400 u away shows
    10 m (whole metres: 419 u still shows 10).
55. **Knock-back (infection mode).** Infected local player standing still,
    hit by an enemy's M4A1 round (`DamageMovePenalty` 40, duration 0.3 s,
    `StiffDuration` 0) travelling along +Z. Timer 0.3 s. First tick (t = 0):
    acceleration 1600 u/s² along +Z, run flag off, end speed 23.1221 u/s.
    With crouch held and no movement key: velocity 0. A non-infected player
    hit by the same round: no change. The M9 knife (`DamageMovePenalty` 20
    but duration 0): no knock-back.

Volume cases (56–79): a stock ladder has viscosity 0.4 and current 0; the
hull is the fallback box; "at full ramp" means `L_f` = 3000.

56. **Ladder from the floor.** Standing at rest on flat ground, the hull
    touching a ladder, looking straight up (camera forward (0, 1, 0)),
    forward held from tick 1. Gravity off, no ground friction or push-in.
    Upward acceleration and end speed / height above the start: tick 1:
    500, 7.5, 0.028125; tick 2: 590, 8.85, 0.0613125; tick 3: 680, 10.2,
    0.0995625; tick 17: 1940, 29.1, 1.166625 (each tick's start speed is
    below 30 and wiped by the viscosity); tick 18: 2030, 30.45, 1.2808125;
    tick 19: 2120, 32.25, 1.4068125; tick 29: 3000, 124.2, 7.770375. From
    tick 38 on: speed 245 at each tick's end, 200 after viscosity and cap,
    3.16875 u per tick (211.25 u/s); height 36.230625 after tick 40.
57. **Letting go on a ladder.** Climbing as case 56 at steady state (end
    speed 245), release all keys. Speed after viscosity and cap on ticks
    1–7: 200, 170, 140, 110, 80, 50, 20; 0 from tick 8 (no braking, no
    gravity). Height gained over ticks 1–7: 11.55 u; the player then hangs.
58. **Jump and crouch on a ladder.** Hanging at rest in a ladder: holding
    jump does nothing (no jump, moving flag off, speed stays 0). Holding
    crouch: acceleration (0, -3000, 0) from tick 1 (not halved, no ramp, no
    500 start); end speed -45 and height -0.16875 after tick 1; -60 and
    -0.5625 after tick 2; crouch rate stays 0.
59. **Ladder top hop.** Climbing straight up at steady state (end speed
    245); on the next tick the hull no longer touches the ladder and the eye
    was not in it last tick. Tick 1: velocity kept, no viscosity, no cap, no
    gravity: rises 3.675 u, vertical speed stays 245. Tick 2: gravity back:
    rises 3.613125 u, vertical speed 228.5.
60. **Leaving with the eye.** Last tick's eye volume was a ladder (or
    water); this tick neither the eye nor the hull is in a free-movement
    volume; velocity (100, 50, 0) at the start of the tick: it is zeroed
    at step 7d.
61. **No horizontal limit on a ladder.** On a ladder, horizontal speed 300
    at the start of a tick (above the 285 run limit, `P` = 0): not clamped;
    after viscosity 270.
62. **Box versus eye.** A stock-water volume (viscosity 0.1, current
    (0, 1500, 0)) whose top is 20 u above the floor; the player stands on
    the floor (feet in it, eye 83 u up): the hull touches it, so viscosity
    and current act and braking is off; the eye is not in it, so crouching
    works normally (case 21 rates) and there are no swim controls.
63. **Current without viscosity; slow speeds.** A `VolumeBrush` with
    viscosity 0 and current (0, 0, 100): the velocity gains (0, 0, 1.5) at
    step 7g every tick, also from rest. A volume with viscosity 0.1 and
    speed 0.9: unchanged by viscosity; speed 5: becomes 0 (7.5 ≥ 5); speed
    20: 12.5.
64. **Viscosity during a jump.** Running inside a viscosity-0.1 volume
    (no current, reaching above the apex), jump on tick 1 (landing
    detected on tick 42, as case 13): viscosity acts on tick 1 (before the
    jump) and again from tick 43; it is skipped on ticks 2–42. On a ladder
    it is never skipped.
65. **Floating in stock water.** Hull and eye inside stock water, at rest,
    no keys, not standing: every tick ends at vertical speed +6.0 and rises
    0.275625 u (18.375 u/s): the 6.0 left from the previous tick is wiped
    by the viscosity, the current adds 22.5 and gravity takes 16.5.
66. **Sinking in still water.** Water with viscosity 0.03 and no current,
    at rest, no keys: vertical speed after ticks 1, 2, 3: -16.5, -30.75,
    -45.0; after tick 10: -144.75, fallen 10.2375 u; after tick 20:
    -287.25, fallen 41.85 u.
67. **Swim controls.** Eye in water, hanging at rest, tick 1 (500 start):
    looking 30° down holding forward: acceleration 433.0127 horizontal,
    -250 vertical; looking 30° up holding forward: 500 horizontal, 0
    vertical; looking 30° down holding back: +250 vertical. Holding crouch
    only: (0, -1500, 0), speed limit 70, crouch rate 0.
68. **Jump in water.** Standing on the bottom of a water volume (eye in
    it), press jump: a normal jump (vertical speed set to 330, `j` = 0.3)
    with +3000 upward acceleration on that tick. Pressing jump while not
    standing: nothing.
69. **Corrosive fluid is not free movement.** Eye and hull in a
    `CorrosiveFluid` volume: no swim controls (forward stays horizontal,
    crouch halves the acceleration as on land), crouch rate follows case
    21; its viscosity, current and burn damage still act.
70. **Jump pad.** Standing at rest on flat ground (on ground last tick),
    the hull touching a pad with `Velocity` 800 and forward (0, 1, 0), no
    keys. Tick 1: vertical speed 800 (not 330, not scaled by `j`), rises
    11.938125 u, ends at 783.5; in a jump, `j` = 0.3 after the tick. With a
    jump factor of 0.3 already, the launch is still 800. A player standing
    on the pad after a landing is launched again on the tick after the
    landing is detected.
71. **Falling onto a pad.** Falling through a pad volume from above: no
    launch while in the air; launched on the tick after the landing tick.
72. **Frictionless ground.** Inside a volume with `Friction` 0 (`k` = 0),
    on flat ground, steady run (`P` = 0): end speed 330, 4.44375 u per
    tick (296.25 u/s). Release all keys: speeds after braking 287.25,
    244.5, 201.75, 159, 116.25, 73.5, 30.75, then 0 on tick 8; distance
    16.695 u over ticks 1–7.
73. **Friction combinations.** Stock water with `Friction` 0.1 (one
    observed map): `k` = 0.5. A `SafteyNet` or `Filter` with `Friction` 0:
    `k` = 5. A hidden `Ice` volume: `k` = 0. `Ice` overlapping water
    (`Friction` 1): 0. A stored `Friction` of 2: clamped to 1, `k` = 5.
74. **Freezing damage volume.** A `DamageVolume` with `PlayerSpeedFactor`
    0.7, `Friction` 0 and `DamageType` `FREEZE`, `P` = 0: run limit 199.5;
    steady end speed 244.5, 3.16125 u per tick (210.75 u/s). Two such
    volumes overlapping: factor 0.49, limit 139.65. The freezing damage
    itself changes no movement value.
75. **Gravity volume.** Inside a `Gravity` volume with `PlayerGravity`
    -300, jump from rest: after tick 1 height 4.933125, vertical speed
    325.5; after tick 2: 9.79875, 321.0. Two overlapping -300 volumes:
    gravity -600. A liquid's -200 never applies to the player.
76. **Bullet pusher.** A full-metal-jacket bullet hits the floor 10 u from
    the player: its pusher (radius 0) never pushes; velocity unchanged.
77. **Door-kick pusher.** Radius 96, strength 1000, no delay, 1 s. Player
    on the ground, hull centre 48 u from it, clear line, at rest: +500 u/s
    along the line on each active tick. Standing on nothing: +50. In the
    air with vertical speed 330, pusher 48 u straight below: vertical
    330 + 50, then cut to 100. Active on ticks 1–66 after it is added,
    removed on tick 67.
78. **Safety net at the centre.** A drop of 500 u onto a floor with a
    `SafteyNet` volume 30 u deep on it: the hull touches the net but its
    centre (50 u up) is outside, so the landing does 100 damage. With the
    net 60 u deep: no damage.
79. **Safety net physics.** Walking into a `SafteyNet` volume (stored
    `Friction` 0): speed limit, friction (`k` = 5), braking and steady
    distances exactly as without it (case 4).

Correction cases (80-94), same conventions; "Corr. n" is the entry in
Corrections (2026-10-19) each one checks.

80. **Opposing keys, then release back** (Corr. 7). Steady forward run
    (end speed 307.7608, `L_f` 3000). Ticks 1-3: forward and back held:
    end speeds 264.4069, 245.3018, 227.5771 (distance 4.11862, 3.82102,
    3.54493 u), forward-axis acceleration 0, `L_f` = 500 after each tick.
    Tick 4: forward only: acceleration 500 (back was held last tick), end
    speed 218.3588, distance 3.34366 u; tick 5: 590, 211.1073 (3.22032 u);
    tick 6: 680, 205.6803 (3.12540 u). (Ramping on through the hold would
    give 3000 and 254.4871 on tick 4: wrong.) The same with left and right
    on the strafe axis.
81. **Braking keeps the vertical speed** (Corr. 14). On walkable ground,
    not in a jump, no keys, no volumes, `K` = 285. Velocity entering
    braking (20, -11.547, 0): after braking (0, -11.547, 0). (60, -11.547,
    0): (17.25, -11.547, 0). (4, -11.547, 0): (0, 0, 0). On flat ground the
    end-of-tick result is the same as zeroing everything (the blocked
    downward move zeroes `v_y`), so check the braking step itself.
82. **Pending jump** (Corr. 6). In the air after walking off a 100 u
    ledge (not in a jump, `j` = 1, horizontal speed 0, forward held), with a
    jump request pending (the key was pressed on the tick after the move
    that left the ledge). No jump in the air. Horizontal acceleration 3000
    on every pending tick (no 500 start, no ramp, no 0.1 factor): speed 45,
    90, 135 u/s and total horizontal distance 0.16875, 1.0125, 2.53125 u
    after pending ticks 1-3 (the 285 clamp applies as usual later).
    Vertical motion as case 18. On the tick the landing is detected the
    jump fires without the key: `v_y` = 330, in a jump, `j` → 0.3. With
    `j` = 0.6 when the request became pending: the acceleration is
    1800 u/s², `j` stays 0.6 on every pending tick (no recovery), and the
    landing jump is 198 u/s. A request made while in free movement is
    dropped on that tick.
83. **Start direction** (Corr. 15). At rest on the ground, forward and
    right pressed on tick 1: acceleration 490.6023 forward and 96.4851
    right (|a| = 500), end speeds 7.0897 forward, 1.3943 right. Tick 2:
    forward 590 (not ramped on tick 1, `L_f` = 500), right 680. In a jump,
    keys up since the jump tick, forward and right pressed on tick 5 after
    the jump (`j` = 0.372): before `j`, 226.6233 forward and 445.6926 right
    (`500 × (300, 590)/|(300, 590)|`); after `j`, 84.3039 and 165.7976.
84. **Jump pressed during a plant** (Corr. 6, 9). On flat ground at rest,
    movement locked from tick 1; jump pressed on tick 3 and released on
    tick 4; lock ends at tick 10. No jump on ticks 3-9; on tick 10 the
    jump fires (`v_y` = 330, in a jump) without the key held.
85. **Crouch taps inside the release hold** (Corr. 8). Shipped crouch-spam
    config, running (not still), on ground; tick `n` at wall time `15n`
    ms. Crouch key down on tick 1, up on tick 2, down on tick 3, up from
    tick 4. Crouch flag: on for ticks 1-7 (one press, at 15 ms), off from
    tick 8 (120 ms). Lock-out value after ticks 1, 2, 3: 0.6, 0.61, 0.62;
    slowdown value 0.5 after each of them (no refill while the rate
    moves). Wrong (raw key): a second charge on tick 3 (lock-out 0.21,
    slowdown 0) and the flag held until tick 10.
86. **Standing up halfway under a low ceiling** (Corr. 16). Fallback boxes,
    standing still on a floor with a ceiling 95 u above it (no room for the
    100 u standing hull, room for the 80 u crouched one). Crouch key down
    on tick 1 only (wall 15 ms): the flag is held by the 100 ms hold
    through tick 7 (rate 0.913022, half-height 40.87698). Tick 8: the flag
    clears, because the hull is taller than the crouch box (the standing
    fit is not tested). Rates after ticks 8-11: 0.653407, 0.449814,
    0.294467, 0.179589; half-heights 43.46593, 45.50186, 47.05533,
    48.20411; on tick 11 the wanted 96.41 u hull does not fit and is forced,
    overlapping the ceiling. Holding crouch until `r` = 1 and then
    releasing: the flag stays on (no room), as before.
87. **Slide deflection and the third contact** (Corr. 13). Hull resting
    0.1 u from a flat wall with normal (0, 0, -1); displacement
    (3.14510, 0, 3.14510) into it: end displacement (3.14510, 0, -0.00315)
    (along the wall, 0.1 % pushed off it). Into a 90° corner of two walls:
    the second contact leaves only the crease (vertical), so a horizontal
    move stops at the corner. A move whose remainder meets a third surface
    in the same tick stops at that contact point (no further sliding).
88. **Movement locked while running** (Corr. 9). Steady run (end speed
    307.7608), the lock starts on tick 1, no keys, no volumes, on flat
    ground: speed after halving and braking, then friction: tick 1: 153.88
    halved, 111.13 braked, end 103.1005, distance 1.60598 u; tick 2: end
    8.1644, distance 0.12718 u; tick 3: 4.08 halved, 5 or less, all zero.
    Locked in the air at rest: vertical speed -33, -66, -99 and fallen
    0.12375, 0.7425, 1.85625 u after ticks 1-3 (2200 u/s²). The crouch
    rate still follows the crouch key while locked.
89. **Stuck at a wall in a jump** (Corr. 17). In a jump, the attempted
    move (2, -1, 2) is completely blocked (position unchanged) while the
    hull stands on a ledge after it: horizontal velocity set to 0. The same
    blocked move on walkable ground, not in a jump: horizontal velocity
    kept (the vertical rule applies instead).
90. **The 0.707 on-ground threshold** (Corr. 18). Ground with normal.y
    0.70705: the movement counts the player on ground (a jump starts,
    braking runs, a landing ends a jump), while the integrator treats it as
    steep (acceleration projected along the slope, no friction). With
    normal.y 0.70695 it is steep for both (no jump, no braking).
91. **Server stamina steps** (Corr. 20). Server gauge 100, records of
    250 ms each: sprinting drains 3.0 per record (100 ms × 30/s), not 7.5;
    a refill record adds 2.5. When the server's gauge reaches 0 it ends
    sprint, and from the client's next tick its limit is the run speed even
    if the client's own gauge still reads 1 or more.
92. **Ladder by the box** (Corr. 2). A ladder volume spanning x from 0 to
    10 (and the player's height range); hull centre at x = -10 (box edge
    at x = 5): on a ladder (gravity off, case 56's climb applies). Centre at
    x = -15.5 (box edge at -0.5): not on a ladder.
93. **Standing on another player** (Corr. 19). Resting on another
    character's head, horizontal velocity (100, 0, 0), no keys: not on
    ground (no braking, the jump key does nothing), but ground friction
    applies with a flat normal: end speed 92.7743 u/s, distance 1.44513 u.
94. **Up a steep slope** (Corr. 13). Standing on a steep slope with normal
    (0.8, 0.6, 0) (up-hill is -X), displacement (-1, -0.5, 0.3): replaced
    by (0, 0, 0.3) before sliding (vertical part dropped). Displacement
    (1, -0.5, 0.3) (down-hill, `move·n` > 0): not changed.

## Open questions

1. **Hull sizes.** The stand and crouch boxes come from each character
   model's stand and crouch animations; only the fallback (half-extents
   15×50×15 and 15×40×15) is known here. Read the real values from the
   default character model once the model reader exists (models spec), or
   measure in game with a wall-height test.
2. **Eye height** and the camera's crouch offset are not part of this
   movement code; they belong with the camera spec.
   **Answered:** the camera sits `0.66 × H_stand` above the hull centre,
   plus `-13 × r` for the crouch rate (console variables `ChaseCamOffset`
   and `DuckDownCamOffSet`): 83 u above the feet standing, 60 u fully
   crouched with the fallback boxes, `83 - 23r` in between, so it follows
   the crouch curve. Upward camera steps are smoothed (0.1 s, at least
   140 u/s, at most 30 u behind). It applies in the new "Eye (camera)
   height" section; inventory's pickup ray and weapon_firing's shot origin
   start at this camera point, not at 0.9 of the hull height. Test cases
   52 and 53. The real stand half-height still comes from the model (Q1).
3. **Unit scale.** The spec stays in world units; whether 1 u is meant as
   1 cm (as in later LithTech games) is not stated anywhere here. Only
   matters when mixing with other games' maps.
   **Answered (as far as the reference goes):** the only stated mapping is
   the HUD's: every in-game distance readout (scope range, objective and
   flag markers) prints world units divided by 40 as metres, so the game
   calls 40 u one metre (1 u = 2.5 cm). Nothing in the movement or physics
   code uses metres. The bodies are not to that scale (a 100 u standing
   hull would be 2.5 m), so for mixing with other games' maps scale by body
   size, not by the HUD factor. See Units and conventions; test case 54.
   To settle a physical scale, measure in game: stand next to a standard
   door or crate of known real size and compare its height in world units
   with the player's eye height (83 u).
4. **Movement basis yaw.** The movement facing is the body rotation plus the
   upper-body twist, which should equal the view yaw; confirm there is no
   case where they differ (e.g. while the twist is clamped).
5. **Standing up in the air** grows the hull around its centre; whether the
   collision resize pushes the player up when the feet would enter the
   floor, or fails and is then forced (leaving the hull in the floor for a
   tick), needs a live check: jump, crouch, stand up just before landing.
   **Answered:** the growth tries down first, then up, and centres the hull
   in the free span. In open air it grows around the centre; with the floor
   closer than the growth it is pushed up so it rests on the floor; it only
   fails (and is forced, overlapping) when the span between floor and
   ceiling is shorter than the new height. See Crouching, "Growing"; test
   cases 43 and 44.
6. **Live movement rate.** The engine default is 15 ms; the server sends the
   value to the client at connect. No shipped server configuration
   overriding it was found, but the live servers may have used another
   value. Check by measuring jump airtime (41 ticks ≈ 0.615 s).
7. **Live crouch-spam values.** Taken from one shipped dedicated-server
   configuration; the official servers may have used other numbers.
8. **Weapon modification penalty factor** (`c_mod`) defaults to 0; check
   attribute data for weapons whose modifications change it.
9. **Stamina timing granularity.** The gauge uses the OS millisecond tick
   count, which on Windows advances in 10–16 ms steps; we use exact frame
   time. Effects should be invisible.
10. **Jump pads** add their launch velocity; the pad's data and activation
    rules belong to a world-objects spec. (Also asked: `JumpVolume`'s
    `Velocity` is a number in the maps, 800 to 3000: its direction, and
    when it fires: entering, standing in it, or only a jump inside it.)
    **Answered:** `Velocity` is a speed; the direction is the forward
    vector of the pad object's `Rotation` (the gravity map's pads point
    straight up, 800 and 1400). It fires on its own, without the jump key,
    whenever the hull touches the pad and the player was on ground on the
    previous tick and is on ground now (not in a jump, not on a ladder, not
    in free movement): the pad velocity is added to the velocity, not
    scaled by the jump factor, and the rest is a normal jump. Standing on a
    pad relaunches on the tick after each landing is detected. Applies in
    Volumes, "Jump pads", and Jumping; test cases 70 and 71. The dam map's
    pads: Open question 29.
11. **Jump factor and crouched air acceleration.** The Acceleration section
    scales by `j` when `j < 1`, but test case 20's +2.25 u/s per tick fits
    `j = 1`. Does `j` scale the crouched in-jump 150 u/s²?
    **Answered:** yes. The jump-factor scaling comes after the whole
    acceleration is built, bypassed or not, so crouched in a jump the
    acceleration is `150·j`. Case 20 assumed `j = 1`, which after a jump
    holds only from tick 40 on (`j` is 1.002 at the start of tick 40); on
    tick `n < 40` it is `150·(0.3 + 0.018(n - 1))`. Applies in Acceleration
    and Gravity and air movement; case 20 corrected by case 37.
12. **Moving-start rescale with crouch or a bypassed ramp.** Does the
    rescale to 500 apply when crouch is held (base 1500) or when the start
    ramp is bypassed?
    **Answered:** no. The rescale belongs to the ramp, and the ramp is
    skipped entirely while the crouch flag is set and on the jump-request
    tick: the acceleration is the plain 1500 (or 3000) from the first tick,
    and `L_f`/`L_r` are left untouched. From rest, crouched, holding
    forward: 21.6770, 41.7876, 60.4451 u/s after ticks 1–3. Applies in
    Acceleration; test cases 38 and 39.
13. **Crouch direction reversal.** On a reversal mid-transition, does the
    other direction's elapsed time restart at zero, continue, or start from
    the mirrored remainder?
    **Answered:** it restarts at zero; the rate itself continues from where
    it is. Released after 5 crouching ticks: rates 0.815619 → 0.556003,
    0.352411, 0.197064, 0.082186, 0; half-heights 41.84381 → 44.43997,
    46.47589, 48.02936, 49.17814, 50. Applies in Crouching ("Reversal");
    test cases 40 and 41.
14. **Standing up on the ground.** Does the growing hull keep its feet, as
    the spec says for shrinking?
    **Answered:** yes. The growth first tries to extend downward, the floor
    stops it at once, so all of the growth goes upward and the feet stay
    put. Applies in Crouching ("Growing"); test case 42. (Standing up in the
    air: Q5.)
15. **Steps and slopes.** What counts as a step, and how is the downhill
    projection or ground snap done?
    **Answered:** a step-up is tried after any low slide that touched
    something other than a "not a step" surface, a player-collision object
    or a character; there is no height or normal test before trying. The
    hull lifts 30.5 u (refused if that hits anything), slides, drops at most
    30.5 u, refuses a steep landing unless a short forward probe clears it,
    and is taken only if it got more than 1.01× as far along the move.
    There is no ground snap: the mover never pulls the hull down; the
    standing probe is 0.02 × half-height (1 u) and anything further is a
    fall. A flat-bottomed hull overlapping a ledge by any amount stands on
    it; an edge only touching the curved side does not hold it. Applies in
    Slopes and steps; test cases 45 and 46.
16. **Falling after an air spawn or teleport.** Where is the fall height
    measured from?
    **Answered for spawns:** the server drops the player onto the floor
    below the spawn point before resetting the tracking, so an air spawn
    never falls. **Teleports:** no reset was found; the drop is measured
    from the highest Y since the last grounded update (case 48). Whether
    Combat Arms maps use teleporters at all is not known; check live by
    using one with a large height difference and watching for damage.
    Applies in Falling and fall damage; test cases 47 and 48.
17. **Walk key versus sprint, both held, opposing keys.**
    **Answered:** the walk key ("Speed", default C) is separate from sprint
    ("Dash", left Shift). Walk + sprint: no sprint, gauge recovering, walk
    speed. Forward + back + sprint: no sprint but the gauge drains (quirk).
    Opposing keys: moving flag on, that axis's acceleration exactly 0, no
    braking, the player coasts down by friction alone while the axis ramp
    keeps climbing. Applies in Input to control state, Acceleration and
    Sprint and stamina; test cases 49, 50 and 51.
18. **What runs per tick and what per frame.** Do stamina and the crouch
    release delay run per tick or per frame; is the hull resized once or
    twice; does sprint take effect at once?
    **Answered:** stamina per rendered frame; the crouch release delay is
    tested every tick against the OS millisecond clock, so in effect per
    frame; the hull resize runs twice per tick but the second call never
    changes anything; sprint speed waits for the server's confirmation
    (one round trip; immediate on a listen server). Applies in Tick clock.
19. **Hit knock-back (from attribute_data.md Q3).** What does an ammo's
    `DamageMovePenalty` do to movement?
    **Answered:** it is a knock-back strength `K`, only for infected
    characters in the infection mode: for `DamageMovePenaltyDuration` ×
    (1 + `StiffDuration`) seconds the acceleration is replaced by
    `K²·(1 - t)` along the hit direction, and run and sprint are off. See
    Other modifiers; test case 55. The figure types of the infected
    characters (their `StiffDuration`) belong to a game-modes spec.
20. **Ladder from the ground.** Standing on walkable ground inside a
    `Ladder` volume, looking up and holding forward, the Integration rules
    push the upward acceleration back into the ground, so the ladder could
    only be climbed by entering it in the air. What lifts a player at a
    ladder's foot? Jumping off a ladder, and what happens on leaving one?
    **Answered:** on a ladder the player's gravity is switched off, and
    with gravity off the integrator skips all of its ground rules (no
    push-in, no friction), so the upward acceleration lifts the player
    straight off the floor (case 56). There is no jump on a ladder (the
    key does nothing). On leaving, the velocity is kept and gravity
    returns one tick late (a hop over the top, case 59); it is zeroed only
    when the eye was in the ladder (or a liquid) on the previous tick and
    nothing gives free movement now (case 60). Applies in Volumes,
    "Ladders", and Integration; test cases 56–61.
    Re-checked against the reference (2026-10-19): confirmed. The
    implementation still runs the ground rules with gravity 0 on a ladder
    (Corrections 1, 3).
21. **Container membership.** Is a player "in" a volume when the hull's
    centre is inside, or when the hull touches it?
    **Answered:** for movement (ladder, viscosity, current, speed factor,
    friction, gravity volumes, jump pads) and for volume damage, when the
    hull **box** touches the volume. Free movement (swimming, crouch reset,
    stopping on leaving) also uses the eye point, once per frame. The
    fall-damage safety-net test uses the hull's centre point. Applies in
    Volumes, "Membership"; test cases 62, 69 and 78.
22. **`viscosity_max`.** Its value for players; whether viscosity also
    acts only above 1 u/s; whether the current applies when the volume's
    viscosity is 0.
    **Answered:** 5000 (the same constant as for grenades), only when the
    speed is above 1 u/s and the volume's viscosity is above 0. Unlike
    grenades, a player gets every visible volume's current whatever its
    viscosity and whatever the speed. Applies in Volumes and Constants;
    test case 63.
23. **"Skipped right after a jump".** On which ticks exactly?
    **Answered:** on every tick the player is in a jump: from the tick
    after the jump tick through the tick on which the landing is detected
    (ticks 2–42 for a jump on tick 1 on flat ground). The jump tick itself
    and the tick after the landing tick are slowed. Never skipped on a
    ladder; currents are never skipped. Applies in Volumes; test case 64.
24. **Swimming.** Speed limit, acceleration and gravity in the liquids;
    whether view, jump or crouch move a swimmer up or down; getting out;
    drowning; is "free movement" exactly the liquid classes?
    **Answered:** the normal stance limit (horizontal only) and
    acceleration; player gravity unchanged (-1100); the stock water's
    current floats a player at the surface, still water sinks them. Looking
    down and holding forward swims down (an upward view part is removed),
    back reverses it, crouch accelerates down at 1500 (walk-speed limit);
    the jump only works from the bottom, as a normal jump. No special
    edge climb (the stair step does it), the velocity is zeroed once when
    the eye leaves, no drowning or breath. Free movement is the liquid
    classes plus `Ladder`, by the eye point (or the hull touching a
    ladder); `CorrosiveFluid` never counts (quirk). Applies in Volumes,
    "Liquids and swimming"; test cases 62 and 65–69.
    Re-checked against the reference (2026-10-19): confirmed, including
    no drowning or breath (no air timer or drowning damage exists in the
    movement, the player object or the HUD). The implementation has none of
    the swim controls yet and tests free movement by the hull centre
    (Corrections 4).
25. **Ladder speed.** Is the 200 limit scaled by the penalty and the speed
    factors like the other limits?
    **Answered:** no. On a ladder the horizontal speed is not limited at
    all, and only the upward speed is capped, at a plain 200 (no penalty,
    no speed factor, no downward cap). Applies in Speed limit and Volumes,
    "Ladders"; test cases 56 and 61.
26. **Volume property defaults; friction 0 in safety nets and gravity
    volumes; do safety nets take part in the physics?**
    **Answered:** `Friction` defaults to 1 for every class except `Ice`
    (0), `PlayerSpeedFactor` to 1 for every class (world_dat.md). Ground
    friction is `5 × min(1, Friction)` over the volumes touched, without
    `Filter` and `SafteyNet`: a safety net's 0 changes nothing, a gravity
    volume's 0 makes the ground frictionless inside it. Safety nets have no
    physics at all; they only stop fall damage. Applies in Volumes; test
    cases 72, 73 and 79.
27. **Pushers.** The pusher records (radius, strength, duration), the push
    direction, and how the 100 u/s upward cap works when the speed is
    already above it.
    **Answered:** the records are `BulletExpPusher` (radius 0, delay
    0.05 s, duration 0.5 s, strength 400; named by the full-metal-jacket
    bullet impact) and `RocketPusher` (radius 0, 0.05 s, 0.5 s, 800;
    unused). With radius 0 they never push: explosions do not push players.
    The only working pusher is an AI's door kick (radius 96, no delay, 1 s,
    1000). The push points from the pusher to the hull centre in 3D,
    repeats every active tick, and afterwards any upward speed above 100
    is set to 100, also when it was already above it. Applies in Other
    modifiers, "Pushers"; test cases 76 and 77.
28. **Damage volumes.** The `DamageType` values and what freezing damage
    does.
    **Answered:** `DamageType` names an entry of the game's damage-type
    table (world_dat.md lists the class defaults and the names seen in
    maps). Freezing damage is ordinary damage plus a blue screen tint, a
    frozen HUD icon and a slight camera sway; it does not slow the player.
    The observed freezing `DamageVolume`s slow players through their own
    `PlayerSpeedFactor` 0.7 and `Friction` 0. Applies in Volumes, "Damage
    volumes"; test case 74.
29. **The dam map's jump pads.** Read with world_dat.md's rotation formula,
    its three pads (2000, 2000 and 3000 u/s) point 45° downward toward -Z,
    which would only shove a player about 20 u along the floor. Check live:
    walk onto one and watch whether the player is thrown up and forward
    (the pitch sign would then be the other way round for these objects) or
    only shoved.
