# Halo: Combat Evolved: vehicles

Source basis: the game's own map files from the original Xbox disc
(every vehicle tag, physics tag, camera track, vehicle animation block,
scenario vehicle placement and vehicle weapon on all 24 maps), read with
throwaway scripts; and a decompilation of an Xbox build of the engine
(the vehicle update, the mass-point physics, the unit layer's seats and
aiming, the player's action and look handling, the following camera),
used for behaviour only. The numbers in the test cases come from a
scratch simulation of the model as read, run on the disc's values.
Status: draft

Sections: Summary; Units and conventions; Constants; The vehicle tag;
The physics tag; Seats, powered seats and weapons; Behaviour (the
control variables, the mass-point step, the four driving models, other
types, sleeping, collisions, pushes, entering and exiting, input and
camera, guns, damage, multiplayer, exported values); The disc's values;
Per-tick order; Edge cases; Quirks; What a rigid-body implementation
must preserve; Test cases; Confidence per section; Open questions.

---

## Summary

- A vehicle is a unit (`objects.md`) with its own 0x100 bytes of tag
  (speeds, turn limits, a type) and a physics tag (`phys`): a rigid body
  made of 5 to 22 **mass points**, each a sphere with a mass, that touch
  the level one by one. Every tick each touching sphere gets a spring
  normal force proportional to how deep it is in the ground and a
  friction force proportional to its velocity along the ground; some
  spheres are **powered**: their friction pulls them toward a commanded
  wheel speed (Warthog wheels, Scorpion treads) or they get a hover
  spring (Ghost, Banshee). The sum is integrated explicitly once per
  tick at 30 Hz. There is no Coulomb friction, no restitution and no
  solver: everything that defines the feel is a linear spring or damper
  in the per-tick units of `cache_file.md`.
- The type picks the driving model: the **Warthog** (jeep) steers all
  four wheels, fronts toward and rears against where the camera looks,
  and its wheels drive toward a speed that ramps at a fixed rate; the
  **Scorpion** (tank) drives its two treads at speed minus and plus a
  turn value; the **Ghost** (scout) is pushed toward the stick's
  velocity (strafing included) and yawed toward the camera by a capped
  torque, floating on three antigravity springs; the **Banshee**
  (fighter) is steered as a whole toward an orientation 15 degrees
  above the camera and thrust along its nose.
- The multiplayer maps hold the Warthog, `ghost_mp` and `scorpion_mp`
  (identical on all 13 maps); the Banshee is campaign only. On the
  disc vehicles have zero vitality and **cannot be destroyed**; their
  riders take 18% of direct hits. Nothing respawns a vehicle.

## Units and conventions

As `cache_file.md`: world units (WU, 1 WU = 3.048 m), +z up, x forward
and y left in a model's own frame, radians unless a field says degrees,
30 ticks per second. Speeds in the vehicle tag are WU per tick (WU/t);
accelerations WU per tick per tick. Several angle fields of the vehicle
and seat blocks are **degrees** on the disc (stated per field); the
engine converts them where it reads them.

g = 0.0035651792 WU/t^2 (`movement.md`). A vehicle's object position
is its **centre of mass**: a point q given in the model frame (mass
point positions, the centre of mass c) is at

  x(q) = p + R (q − c),  R = [f, l, u]

with p the object position, f forward, u up and l = u × f (left). A
vehicle is created raised by half its bounding radius above its
placement (Warthog 0.55 WU, Ghost 0.4, Scorpion 1.1, Banshee 0.7) and
falls onto its springs.

"Steering angle" σ: with d the desired facing vector the driver
supplies, σ = atan2(l·d, f·d) in (−π, π]; positive means the camera
looks to the vehicle's left.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| GRAVITY | 0.0035651792 | WU/t^2 | the engine's gravity, times the physics tag's gravity scale |
| COLLISION_DEPTH | 0.2 | WU | vehicle-vehicle contact spring depth |
| NEW_MODEL_TEST | radius <= 0 | | a physics tag with radius <= 0 uses the model of this spec; > 0 the older one (Edge cases) |
| POINT_REST_SPEED2 | 0.0011111111 | (WU/t)^2 | a mass point is "at rest" below this squared speed (1 WU/s) |
| REST_LINEAR_V2 | 0.0011111111 | (WU/t)^2 | sleep test: linear velocity squared |
| REST_ANGULAR_V2 | 0.0027415568 | (rad/t)^2 | sleep test: angular velocity squared (3 degrees per tick) |
| REST_LINEAR_A2 | 3.0864197e-7 | (WU/t^2)^2 | sleep test: linear acceleration squared |
| REST_ANGULAR_A2 | 3.0461742e-6 | (rad/t^2)^2 | sleep test: angular acceleration squared (0.1 degree per tick^2) |
| REST_MIN_GROUND_POINTS | 3 | count | sleep needs this many touching points |
| STOP_TICKS | 15 | ticks | the slow stop after falling asleep |
| STOP_DAMPING | 0.835 | per tick | velocity factor during the slow stop |
| FREEZE_PASSES | 4 | count | penetration sweeps per tick |
| FREEZE_EPSILON | 0.0078125 | WU | backoff along the hit normal |
| MATERIAL_MASS_LIMIT | 7500 | mass | a material's ground friction scale applies only to physics of at most this mass |
| SCOUT_YAW_ACCEL | 0.0034906587 | rad/t^2 | Ghost yaw rate change limit (0.2 degrees per tick per tick) |
| SCOUT_STEER_SHARP | 0.785398185 | rad | Ghost: steering beyond 45 degrees cuts grounded drive |
| HOVER_RATE | 0.1 | per tick | Ghost hover fraction change limit |
| FLIP_UP_LIMIT | 0.7071 | up.z | below or at this a vehicle shows the flip prompt, above it the enter prompt |
| FLIP_TICKS | 30 | ticks | longest flip |
| FLIP_DONE_UP | 0.9 | up.z | a flip ends when up.z exceeds this |
| FLIP_RATE | 0.3 | rad/t per unit | flip angular speed scale |
| EJECT_UP | 0.2 | up.z | riders are thrown out of a grounded vehicle whose up.z is below this |
| SEAT_REACH | 1.0 | WU | a seat is offered within this of the unit's centre |
| DRIVER_PREFERENCE | 1.5 | factor | non-driver seats count this much farther once a driver seat is a candidate |
| ENTER_MAX_SPEED2 | 0.01 | (WU/t)^2 | the player must move slower than 0.1 WU/t to be offered a seat |
| ENTER_MAX_SPIN2 | 0.01 | (rad/t)^2 | and the vehicle turn slower than 0.1 rad/t |
| SPLAT_MIN_SPEED | 0.06666667 | WU/t | vehicle speed above which touching a biped hurts (2 WU/s) |
| SPLAT_MIN_RELATIVE2 | 0.0011111111 | (WU/t)^2 | or relative velocity squared above this |
| SPLAT_EXIT_GRACE | 90 | ticks | a biped is not hurt by the vehicle it left within this |
| PUSH_LIFT | 0.8 | | upward component added to the push direction on a biped |
| PUSH_MIN_SPEED | 0.1 | WU/t | least push speed |
| PUSH_SPIN | π | | angular velocity added per unit of horizontal push (vehicle pushes) |
| CRASH_THRESHOLD | 0.02 | WU/t | velocity change per tick that plays the crash sound and effect |
| SLIP_THRESHOLD | 0.03 | WU/t | contact sliding speed that makes tyre material effects |
| AIR_FLOOR_GAIN | 0.015625, 0.0625 | per tick | air vehicle floor and ceiling spring and damper |
| CAMERA_PITCH_LIMIT | 1.4922565 | rad | 85.5 degrees, absolute pitch limit |
| CAMERA_LIMIT_SLEW | π/256 | rad/t | pitch limits move toward their targets at this rate |

## The vehicle tag

A `vehi` tag's main struct (0x3F0 bytes) is the object base, the unit
base (`objects.md`) and then the fields below. Offsets are absolute.
Every gap listed as padding is zero on every vehicle tag on the disc.

| Offset | Size | Type | Field | Unit | Meaning |
|---|---|---|---|---|---|
| 0x2F0 | 4 | flags32 | flags | | below |
| 0x2F4 | 2 | enum16 | type | | 0 human tank, 1 human jeep, 2 human boat, 3 human plane, 4 alien scout, 5 alien fighter, 6 turret |
| 0x2F6 | 2 | | padding | | |
| 0x2F8 | 4 | f32 | maximum forward speed | WU/t | the speed variable's positive limit (Speed variables) |
| 0x2FC | 4 | f32 | maximum reverse speed | WU/t | its negative limit, as a positive number |
| 0x300 | 4 | f32 | speed acceleration | WU/t per tick | step of the speed variable away from 0 |
| 0x304 | 4 | f32 | speed deceleration | WU/t per tick | step toward 0 |
| 0x308 | 4 | f32 | maximum left turn | degrees (jeep); see type | the turn variable's upper clamp (converted to radians) |
| 0x30C | 4 | f32 | maximum right turn | degrees, negative | the lower clamp |
| 0x310 | 4 | f32 | wheel circumference | WU | the wheel and tread spin period |
| 0x314 | 4 | f32 | turn rate | degrees/s (jeep); see type | the turn variable's rate |
| 0x318 | 4 | f32 | blur speed | WU/t | at or above this the `~blur` region permutation shows |
| 0x31C | 2 | enum16 | A in | | vehicle function sources, below |
| 0x31E | 2 | enum16 | B in | | |
| 0x320 | 2 | enum16 | C in | | |
| 0x322 | 2 | enum16 | D in | | |
| 0x324 | 12 | | padding | | |
| 0x330 | 4 | f32 | maximum left slide | WU/t | the slide variable's limits and steps (as speed); 0 on every disc tag |
| 0x334 | 4 | f32 | maximum right slide | WU/t | |
| 0x338 | 4 | f32 | slide acceleration | | |
| 0x33C | 4 | f32 | slide deceleration | | |
| 0x340 | 4 | f32 | minimum flipping angular velocity | rad/t per unit | clamp of the flip strength (Flipping) |
| 0x344 | 4 | f32 | maximum flipping angular velocity | | |
| 0x348 | 24 | | padding | | |
| 0x360 | 4 | angle | fixed gun yaw | rad | not read by the player's flight model |
| 0x364 | 4 | angle | fixed gun pitch | rad | fighter: the nose is held this far above the camera |
| 0x368 | 24 | | padding | | |
| 0x380 | 4 | f32 | AI sideslip distance | WU | AI only |
| 0x384 | 4 | f32 | AI destination radius | WU | AI only |
| 0x388 | 4 | f32 | AI avoidance distance | WU | AI only |
| 0x38C | 4 | f32 | AI pathfinding radius | WU | AI only |
| 0x390 | 4 | f32 | AI charge repeat timeout | s | AI only |
| 0x394 | 4 | f32 | AI strafing abort range | WU | AI only |
| 0x398 | 8 | angle bounds | AI oversteering bounds | rad | AI only |
| 0x3A0 | 4 | angle | AI steering maximum | rad | AI only |
| 0x3A4 | 4 | f32 | AI throttle maximum | fraction | AI only |
| 0x3A8 | 4 | f32 | AI move position time | s | AI only |
| 0x3AC | 4 | | padding | | |
| 0x3B0 | 16 | tag ref `snd!` | suspension sound | | played when a wheel's suspension moves fast (Suspension) |
| 0x3C0 | 16 | tag ref `snd!` | crash sound | | played on a hard velocity change (Crashes) |
| 0x3D0 | 16 | tag ref `foot` | material effects | | per material sliding effects (Crashes) |
| 0x3E0 | 16 | tag ref `effe` | effect | | hover thruster ground effect (scout, fighter) |

The four speed fields are one "speed parameters" record (positive
limit, negative limit, acceleration, deceleration), and so are the four
slide fields; the turn fields 0x308 and 0x30C are a (upper, lower)
limit pair. For the fighter type 0x308 and 0x314 mean something else
(The fighter). The tank does not read 0x308, 0x30C or 0x314.

### Vehicle flags (0x2F0)

| Bit | Name | Read by |
|---|---|---|
| 0 | speed wakes physics | a sleeping vehicle wakes while its speed variable is not 0 |
| 1 | turn wakes physics | while its turn variable is not 0 |
| 2 | driver power wakes physics | while driver seat power is not 0 |
| 3 | gunner power wakes physics | while gunner seat power is not 0 |
| 4 | control opposite speed sets brake | throttle against the current speed brakes (Speed variables) |
| 5 | slide wakes physics | while its slide variable is not 0 |
| 6 | kills riders at terminal velocity | Terminal velocity |
| 7 | causes collision damage | read by the damage system for vehicle contacts (`health_shields.md`) |
| 8 | AI weapon cannot rotate | AI only |
| 9 | AI does not require driver | AI only |
| 10 | AI unused | |
| 11 | AI driver enable | AI only |
| 12 | AI driver flying | AI only |
| 13 | AI driver can sidestep | AI only |
| 14 | AI driver hovering | AI only |

### Vehicle function sources (0x31C to 0x322)

Each nonzero source writes the vehicle's incoming function value for
that slot every tick, clamped to 0..1. S_f = |maximum forward speed|,
S_r = |maximum reverse speed|, S = max(S_f, S_r); the same for slide
and turn limits. s, σ_s, τ are the speed, slide and turn variables,
v the linear velocity.

| Value | Name (tag tool) | Value written |
|---|---|---|
| 0 | none | slot untouched |
| 1 | speed absolute | abs(s)/S |
| 2 | speed forward | max(s, 0)/S_f |
| 3 | speed backward | abs(min(s, 0))/S_r |
| 4 | slide absolute | abs(σ_s)/max slide |
| 5 | slide left | abs(σ_s)/left slide |
| 6 | slide right | abs(σ_s)/right slide |
| 7 | speed slide maximum | max(abs(s)/S, abs(σ_s)/max slide) |
| 8 | turn absolute | abs(τ)/max(abs(left), abs(right)) (raw tag values) |
| 9 | turn left | abs(τ)/abs(left) |
| 10 | turn right | abs(τ)/abs(right) |
| 11 | crouch | 1 while the vehicle's crouch flag is set |
| 12 | jump | 1 while the vehicle's brake flag is set (jump or brake) |
| 13 | walk | not written |
| 14 | velocity air | abs(v)/S |
| 15 | velocity water | abs(v)/S while the vehicle touches the ground or water, else 0 (reference) |
| 16 | velocity ground | abs(v)/S while on the ground, else 0 |
| 17 | velocity forward | abs(v·f)/S |
| 18 | velocity left | abs(v·u)/S (the reference reads the up axis; see Open questions) |
| 19 | velocity up | abs(v·u)/S |
| 20 | left tread position | left tread/wheel circumference |
| 21 | right tread position | right tread/wheel circumference |
| 22 | left tread velocity | abs(s − τ)/S |
| 23 | right tread velocity | abs(s + τ)/S |
| 24 to 27 | front left tire position ... | wheel/wheel circumference (all four, reference) |
| 28 to 31 | ... tire velocity | abs(s)/S (reference) |
| 32 | wingtip contrail | (|v − (v·f)f| / 0.3)^2 |
| 33 | hover | the hover fraction |
| 34 | thrust | the thrust value |
| 35 | engine hack | (abs(v·f)/S)(1 − b) + (abs(s)/S_f) b with b = clamp((0.2 a + 1)/2, 0, 1), a = airborne ticks |
| 36 | wingtip contrail new | ((abs(v)/S_f) · thrust − 0.05)/0.85 |

Values 13 to 36 are named from the tag tool's list; the reference's
grouping of 24 to 31 (four positions, then four speeds) is not the
alternating order the names suggest (Open questions). The disc uses 0,
1, 12, 20, 21, 35 and 36.

## The physics tag

`phys`, main struct 0x80 bytes. Offsets from the struct.

| Offset | Size | Type | Field | Unit | Meaning |
|---|---|---|---|---|---|
| 0x00 | 4 | f32 | radius | WU | <= 0: the model of this spec; > 0: the older model (Edge cases); −1 on the four vehicles |
| 0x04 | 4 | f32 | moment scale | | multiplies the inertia tensor (1 everywhere) |
| 0x08 | 4 | f32 | mass | mass units | M; only ratios matter, forces are written as M times an acceleration |
| 0x0C | 12 | point | center of mass | WU, model frame | c; equals the mass-weighted mean of the mass point positions |
| 0x18 | 4 | f32 | density | | input to the mass points' densities |
| 0x1C | 4 | f32 | gravity scale | | g_v = gravity scale × g (1 everywhere) |
| 0x20 | 4 | f32 | ground friction | 1/t | μ_g |
| 0x24 | 4 | f32 | ground depth | WU | D_g: depth at which one contact carries the whole weight |
| 0x28 | 4 | f32 | ground damp fraction | 1/t | κ_g |
| 0x2C | 4 | f32 | ground normal k1 | cos | ground slope at or above which (normal z) drive is full |
| 0x30 | 4 | f32 | ground normal k0 | cos | normal z at or below which drive is 0 |
| 0x34 | 4 | | padding | | |
| 0x38 | 4 | f32 | water friction | 1/t | μ_w |
| 0x3C | 4 | f32 | water depth | WU | D_w: depth of full buoyancy |
| 0x40 | 4 | f32 | water density | | ρ_w |
| 0x44 | 4 | | padding | | |
| 0x48 | 4 | f32 | air friction | 1/t | μ_a |
| 0x4C | 4 | | padding | | |
| 0x50 | 4 | f32 | xx moment | | equal to the tensor's xx (precomputed) |
| 0x54 | 4 | f32 | yy moment | | |
| 0x58 | 4 | f32 | zz moment | | the scout's yaw torque scale |
| 0x5C | 12 | block | inertial matrix and inverse | | 2 elements of 0x24 bytes (3x3 f32, row major): element 0 the tensor I, element 1 its inverse |
| 0x68 | 12 | block | powered mass points | | element 0x80, below |
| 0x74 | 12 | block | mass points | | element 0x80, below |

### Powered mass point (0x80 bytes)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | tag string | name | |
| 0x20 | 4 | flags32 | flags | bit 0 ground friction, 1 water friction, 2 air friction, 3 water lift, 4 air lift, 5 thrust, 6 antigrav |
| 0x24 | 4 | f32 | antigrav strength | A_s, in units of the vehicle's weight per point |
| 0x28 | 4 | f32 | antigrav offset | not read by any vehicle step (0 on the disc) |
| 0x2C | 4 | f32 | antigrav height | H (WU) |
| 0x30 | 4 | f32 | antigrav damp fraction | κ_a (1/t) |
| 0x34 | 4 | f32 | antigrav normal k1 | the point's up.z at or above which antigravity is full |
| 0x38 | 4 | f32 | antigrav normal k0 | at or below which it is 0 |
| 0x3C | 68 | | padding | |

A powered mass point is a channel: the vehicle sets, per tick, its
ground, water and air "friction velocities", lift ratios, thrust
fraction, antigravity fraction and a rotation (a quaternion about the
model frame); every mass point that names it uses them.

### Mass point (0x80 bytes)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | tag string | name | |
| 0x20 | 2 | i16 | powered mass point | index, −1 none |
| 0x22 | 2 | i16 | model node | the node the point belongs to; not read by the vehicle step |
| 0x24 | 4 | flags32 | flags | bit 0 metallic (picks the sliding effect type) |
| 0x28 | 4 | f32 | relative mass | input |
| 0x2C | 4 | f32 | mass | m_i = M · rel_i / Σ rel (precomputed; checked) |
| 0x30 | 4 | f32 | relative density | input |
| 0x34 | 4 | f32 | density | d_i = density · reld_i · Σ(rel/reld) / Σ rel (precomputed; checked: total volume Σ m_i/d_i = M/density) |
| 0x38 | 12 | point | position | q_i, model frame |
| 0x44 | 12 | vector | forward | the point's own forward (wheel rolling direction) |
| 0x50 | 12 | vector | up | the point's own up |
| 0x5C | 2 | enum16 | friction type | 0 point, 1 forward, 2 left, 3 up |
| 0x5E | 2 | | padding | |
| 0x60 | 4 | f32 | friction parallel scale | a_par |
| 0x64 | 4 | f32 | friction perpendicular scale | a_perp |
| 0x68 | 4 | f32 | radius | R_i (WU), the contact sphere |
| 0x6C | 20 | | padding | |

The inertia tensor (element 0, checked to 4 decimals on all four
vehicles) is

  I = moment scale · Σ m_i [ (|r_i|^2 + 0.4 R_i^2) E − r_i r_i^T ],
  r_i = q_i − c

(each sphere counted as a solid ball). The engine only reads the stored
inverse.

## Seats, powered seats and weapons

The unit base's blocks at 0x2CC (powered seats), 0x2D8 (weapons) and
0x2E4 (seats) are owned here; their placement is `objects.md`'s.

### Seat element (0x11C bytes)

| Offset | Size | Type | Field | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 4 | flags32 | flags | | below |
| 0x004 | 32 | tag string | label | | the seat's animation label (`W-driver`); a rider's graph must have it |
| 0x024 | 32 | tag string | marker name | | the vehicle marker the rider is attached to |
| 0x044 | 32 | | padding | | |
| 0x064 | 12 | 3 f32 | acceleration scale | | scales the seat marker's measured acceleration into the rider's lean overlays |
| 0x070 | 12 | | padding | | |
| 0x07C | 4 | f32 | yaw rate | degrees/s | replaces the player's look yaw rate in this seat when > 0 |
| 0x080 | 4 | f32 | pitch rate | degrees/s | the same for pitch |
| 0x084 | 32 | tag string | camera marker name | | the third-person camera's focus marker (empty: the rider's own camera point) |
| 0x0A4 | 32 | tag string | camera submerged marker name | | |
| 0x0C4 | 4 | angle | pitch auto level | rad | the pitch the look drifts to while moving |
| 0x0C8 | 4 | angle | pitch range minimum | rad | relative to the vehicle's tilt; both 0: no seat limit |
| 0x0CC | 4 | angle | pitch range maximum | rad | |
| 0x0D0 | 12 | block | camera tracks | | element 0x1C: tag ref `trak` + 12 bytes padding; only the first is used |
| 0x0DC | 12 | block | unit HUD interfaces | | element 0x30: tag ref `unhi` + 32 bytes padding |
| 0x0E8 | 4 | | padding | | |
| 0x0EC | 2 | i16 | HUD text message index | | the seat's name in the entry prompt |
| 0x0EE | 2 | | padding | | |
| 0x0F0 | 4 | angle | yaw range minimum | rad | relative to the seat marker's yaw; both 0: no limit |
| 0x0F4 | 4 | angle | yaw range maximum | rad | |
| 0x0F8 | 16 | tag ref `actv` | built-in gunner | | AI only; null on the disc |
| 0x108 | 20 | | padding | | |

### Seat flags

| Bit | Name | Effect |
|---|---|---|
| 0 | invisible | the rider aims through the vehicle; the third-person camera is used |
| 1 | locked | not read by the reference |
| 2 | driver | the first occupant becomes the vehicle's driver: its throttle and facing drive the vehicle; driver seats use the third-person seat camera |
| 3 | gunner | the occupant becomes the gunner: its aim and triggers drive the vehicle's weapon; a seat that is driver and gunner gives both to one rider |
| 4 | third person camera | the seat's camera is third person |
| 5 | allows weapons | not read by the reference; a rider holds its own weapon when its graph has the seat label for that weapon class |
| 6 | third person on enter | the camera is third person during the entry and exit animations |
| 7 | first person camera slaved to gun | first-person view taken from the vehicle's gun |
| 8 | allow vehicle communication animations | animation only |
| 9 | not valid without driver | the seat is offered only while the vehicle has a driver |
| 10 | allow AI noncombatants | AI only |

### Powered seat element (0x44 bytes)

u32 padding, f32 power-up time (s) at 0x04, f32 power-down time (s) at
0x08, 56 bytes padding. Element 0 is the driver's power, element 1 the
gunner's. Each tick (unit update):

- driver power P0 rises by 1/(30 · power-up time) while the vehicle has
  a driver (or is controlled directly), clamped to 1; otherwise, and
  always once dead, it falls by 1/(30 · power-down time), clamped to 0;
- gunner power P1 does the same while a gunner who is not also the
  driver is seated.

### Weapons block

The unit's weapons block (0x2D8, element 0x24: tag ref `weap` + 20 bytes
padding) gives the vehicle its mounted gun, created at the vehicle's
creation and held by the vehicle, never by a rider (Turrets and guns).

## Behaviour

### What the vehicle reads from its riders

At the start of the vehicle's unit update (before its own physics):

- From the driver (if any, and the vehicle is alive; for an AI driver
  only when not entering or leaving the seat): the driver's control
  flags masked to the driving buttons, its desired facing vector d and
  its throttle (t_i forward, t_j left, each in −1..1, magnitude at most
  1). The vehicle takes the driver's team.
- From the gunner: desired aiming and looking vectors, the gunning
  control flags (triggers, reload), the primary trigger value.
- Nobody driving: throttle 0, d = the vehicle's forward, no flags.

Then: crouch flag F_c = the driver's crouch button; brake flag F_b =
the driver's jump button, or, with vehicle flag 4, (t_i > 0 and s < 0)
or (t_i < 0 and s > 0), where s is the speed variable.

### Speed variables

The speed s, slide σ_s and turn τ are scalars kept per vehicle. The
**variable-speed step** S(x; P, δ) with parameters P = (pos, neg, acc,
dec) and a signed step δ, with a = |δ| acc and b = |δ| dec:

- δ > 0: if x <= −b then x += b; else if x >= 0 then x += a; else
  x = (x/b + 1) · a. Then x = min(x, |δ| pos).
- δ < 0: if x >= b then x −= b; else if x <= 0 then x −= a; else
  x = (x/b − 1) · a. Then x = max(x, −|δ| neg).

So x moves toward 0 by the deceleration and away from 0 by the
acceleration, and a step that crosses 0 is scaled into the new side.
The **seek** T(x → target; P, δ): if x > target, x = S(x; P, −δ) and if
then x <= target, x = target; if x < target, the mirror; equal: nothing.

Each tick, before physics:

1. If F_b: s = T(s → 0; speed, 1). Otherwise s = T(s → t_i; speed, 1)
   and σ_s = T(σ_s → t_j; slide, 1).
   The target is the raw throttle t_i, in WU per tick, clamped only by
   the limits: any stick deflection above the maximum speed asks for
   full speed (Quirks).
2. Turn, every type except the tank: target = σ if s >= 0, −σ if
   s < 0, clamped to [rad(maximum right turn), rad(maximum left turn)];
   τ moves toward the target by rad(turn rate)/30 per tick and stops on
   it (it is also clamped to the raw tag pair (left, right), which for
   the Warthog's ±30 never binds).
3. Turn, tank: if s = 0, τ = T(τ → 0; speed, 1); else τ = T(τ →
   clamp(σ · 2/π, −1, 1) · maximum forward speed; speed, 2) (twice the
   acceleration and deceleration, and limits twice the speeds).

### The mass-point step

Shared by all types (physics radius <= 0). Inputs: the object position
p, orientation (f, u), velocity v, angular velocity ω, the per-channel
values the type set, and two extra vectors the type may add: a "magic"
force F_m and torque T_m. Every force below is per tick: dividing by M
gives a velocity change in WU/t.

**Per mass point i** (in tag order):

1. Position x_i = x(q_i). Forward and up: the tag's vectors turned by
   the channel's rotation (if the point has a channel) and then by R.
   Lever arm r_i = x_i − p. Velocity v_i = v + ω × r_i.
2. **Ground query.** The deepest level (or object) surface within a
   sphere of radius R_i around x_i (the sphere query of
   `collision.md`) gives a plane with normal n, a depth δ_i = R_i −
   (distance from x_i to the surface), and the surface's material
   type. No surface: δ_i is measured against a horizontal plane at
   z = −256 (so it is negative). If the surface belongs to an object,
   that object's shield is depleted, and the point is "on a volatile
   surface" when the object is not scenery or the surface is
   breakable.
3. If δ_i > 0 (**touching**): take the physics tag's μ_g, k1, k0, D_g,
   κ_g and multiply each by the touched material's ground scale from
   `globals.md` when that scale is > 0 (the friction scale only when
   M <= 7500; on the disc only ice, 0.125, is set). Then
   - normal force N_i = M (g · δ_i / D_g − (v_i · n) κ_g) n. Note g
     is the unscaled gravity and M the whole vehicle's mass: each
     touching point alone holds the vehicle up at δ_i = D_g; the
     damping is not clamped, so the force can pull down;
   - ground-relative velocity w_i = v_i − (v_i · n) n;
   - friction F_g = −m_i μ_g w_i;
   - **drive**: if the channel has the ground friction flag and its
     ground speed s_c is not 0: φ = pin(n_z; k0 → k1) (0 at or below
     k0, 1 at or above k1, linear between), α = clamp(u_i · n, 0, 1),
     e = −s_c f_i projected onto the plane (e − (e·n) n), and
     F_g = F_g − m_i μ_g α^2 φ^2 e. On flat ground this is
     −m_i μ_g (w_i − s_c f_i): the contact is pulled toward moving
     at s_c along its own forward;
   - shape the friction by the point's friction type: type 0 leaves
     it; type 1 (forward), 2 (left = u_i × f_i) or 3 (up) split it
     into the part along that axis, scaled by a_par, and the rest,
     scaled by a_perp.
4. If the point is in water (depth h_w > 0 below the water surface):
   depth fraction φ_w = min(h_w / D_w, 1); buoyancy (0, 0, (m_i/d_i)
   ρ_w φ_w g_v) when d_i > 0 and D_w > 0; water friction −m_i μ_w
   (v_i − s_w f_i) (s_w the channel's water speed when it has the water
   friction flag, else 0), shaped by the friction type; water lift
   (water lift flag) |f_i · v_i| · λ_w · M · φ_w along u_i.
5. Air friction −m_i μ_a (v_i − s_a f_i) (s_a the channel's air speed
   with the air friction flag, else 0), shaped by the friction type;
   air lift (air lift flag) |f_i · v_i| · M · λ_a along u_i.
6. Thrust (thrust flag): thrust fraction · M along f_i.
7. **Antigravity** (antigrav flag): a straight trace down from x_i of
   length L = R_i + H against the level and objects. On a hit at
   fraction t with plane normal n_h: h = L t − R_i (height of the
   sphere's bottom), γ = 1 − h/H if h > 0 else 1, β = pin(u_i,z; k0 →
   k1), and the force is
   M A_s β P_c (γ^2 g − (n_h · v_i) κ_a) n_h,
   with P_c the channel's antigravity fraction (seat power). The point
   is then "antigraving".
8. The point's total force F_i is the sum of the above; torque r_i ×
   F_i.

Point flags: at rest when |v_i|^2 < 0.0011111111, on the ground when
δ_i > 0, in water when h_w > 0.

**Sum and integrate.** F = (0, 0, −M g_v) + Σ F_i + (collision force
gathered last tick) + F_m; T = Σ r_i × F_i + (collision torque) + T_m;
the collision accumulators are then cleared. Then, in this order:

  a = F / M,  v' = v + a,  p' = p + v'
  α = R I^-1 R^T T,  ω' = ω + α

and the orientation is turned by the angle |ω'| about ω'/|ω'| (forward
renormalised, up made perpendicular to it and renormalised). Velocity
and angular velocity are stored before the penetration check.

**Penetration freeze.** Up to 4 passes: at the new pose, sweep every
mass point's centre from x_i (this tick's start) to its new position
against front-facing level surfaces and vehicle-blocking objects. If
none hit, accept the pose. Otherwise take the earliest hit (fraction
t_h, normal n_h, sweep vector Δ): ε = 0.0078125/|Δ · n_h| (0.03125 if
Δ · n_h = 0), t' = max(t_h − ε, 0); if n_h · v' < 0, remove the part
(1 − t') of the velocity's motion into the plane: v' = v' + n_h (t' −
1)(n_h · v') and p' = p + v'; scale ω' by t' and recompute the
orientation from the start pose. After 4 passes the last pose is
accepted. The points that hit are remembered ("stuck"). The sphere
radius plays no part here: only the centres are swept; the springs keep
the spheres out of the ground.

**Ground and sleep flags.** The vehicle is on the ground if any point
touches, in water if any point is wet. It **falls asleep** ("at rest")
when every point is at rest, at least 3 touch, none is on a volatile
surface, |v'|^2 <= 0.0011111111, |ω'|^2 <= 0.0027415568, |a|^2 <=
3.0864197e-7 and |α|^2 <= 3.0461742e-6.

**Then** the vehicle's contacts with units are processed (Collisions).

### The jeep (Warthog)

Before the step:

- Wheel spin: W = (W + s) mod wheel circumference, kept in [0, C).
- The two channels (0 'front', 1 'back' on the disc, flag ground
  friction) both get ground speed s. Channel 0 is yawed by +τ and
  channel 1 by −τ about the model's z axis: **the front wheels point
  toward the turn and the rear wheels the opposite way** (four-wheel
  steering, equal angles). With τ > 0 the vehicle turns left going
  forward; reversing, τ follows −σ, so the nose still swings toward
  the camera.

The Warthog's wheels are mass points 11 to 14 (friction type forward:
fronts 0.75 along, 0.45 across; rears 0.45 along, 0.65 across); the ten
hull and bumper points only touch when the hull hits something. What
the rule amounts to: the wheels pull the body toward s along the wheel
direction with rate μ_g Σ m a_par / M ≈ 0.048 per tick, while air
friction takes 0.005 per tick, so the ground speed settles near 0.906 s
(0.2337 WU/t at s = 0.255); and there is no grip limit, so at full lock
the lateral pull cannot hold the turn and the Warthog slides (Test
cases). There is no separate handbrake or power-slide code: jump sets
the brake flag (s → 0 at 0.011 per tick, the wheels then lock at 0
drive and the plain friction stops the vehicle), and the slide is the
linear friction's.

### The tank (Scorpion)

- Treads: left speed s_L = s − τ, right s_R = s + τ; each tread
  position advances by its speed modulo the wheel circumference.
- Channel 0 ('left', the eight tread points at y > 0 on mass points
  0 to 3) gets ground speed s_L, channel 1 ('right') s_R; no rotation.
  The tread points use friction type point (scales 1).
- So the tank turns by tread difference and cannot turn while s = 0:
  τ is forced to 0 when the speed variable is 0 (Quirks).

### The scout (Ghost)

Before the step, with P0 the driver power:

1. Every channel gets antigravity fraction P0 and no rotation.
2. If the water depth at the vehicle is under 0.5 WU and up.z > −0.2:
   - local velocity v_L = (v·f, v·l, v·u);
   - if the hover fraction h_v > 0: maximum speed V = maximum forward
     speed (× 0.8 while F_b), target (V t_i, V t_j, 0) in the vehicle's
     frame, acceleration A = target − v_L with its z set to 0, limited
     to length a_max = speed acceleration (0.005), and a_max is cut to
     a_max (1 − min(0.05 n_g, 0.98)) when the vehicle has been on the
     ground for n_g > 0 ticks and |σ| > 0.785398185; F_m += R A · M h_v.
     Reverse and sideways use the same maximum speed.
   - if h_v > 0: yaw. ω_u = u · ω; desired rate ω_d = sign(σ)
     sqrt(0.0069813174 |σ|), and when |ω_d| > 1e-6 and σ/ω_d < 2 (that
     is |σ| < 0.0279 rad) ω_d = σ/2; error e = clamp(ω_d − ω_u,
     −0.0034906587, +0.0034906587); T_m += u · e · zz moment · h_v.
     This is a bang-bang approach: the yaw rate changes by at most 0.2
     degrees per tick per tick toward the rate that would just stop on
     target.
   - if h_v < 1 (partly grounded): self-righting and stick tilt torques,
     scaled by (1 − h_v). With f2 and l2 the horizontal unit parts of f
     and l: if up.z > 0, alignment (a_f, a_l) = (u·f2, u·l2), rate
     (ω·l2, −ω·f2) (horizontal parts), level torque L = −alignment −
     15 · rate, and per axis the stick torque t_i (or t_j) ·
     clamp(sign(t·L)|L| + 1, 0.3, 2.5) · 0.0015514038 plus (1 − up.z)
     · 0.0038785094 · L; if up.z <= 0 only t_i · 0.0015514038 and
     t_j · 0.0015514038. The first component turns about l (times yy
     moment), the second about f (times −xx moment).
   - while F_b (jump held): with k = clamp((f·v)/maximum speed, 0, 1),
     if k > 0 a nose-up torque yy · k · h_v · (−0.005817764) about l and
     a lift M k h_v 0.004 up; and while airborne a push of (1 − h_v) M
     ρ 0.002 along (l × up, normalised) and (1 − h_v) M ρ 0.001 up,
     with ρ = clamp(1 − airborne ticks/30, 0, 1).
   - F_m and T_m are multiplied by P0.
3. The step runs (the Ghost's antigravity points are 'hull back seat'
   on channel 1 and both wings on channel 0: strength 1.5 each, height
   0.75, damping 0.02, k1 0.5, k0 cos 75 degrees).
4. After the step: the hover target is (antigraving channel points /
   channel points) · max(up.z, 0.4), clamped to 0..1; h_v moves toward
   it by at most 0.1 per tick.

With three points of strength 1.5 the hover balances at γ^2 = 2/9:
each sphere's bottom floats about 0.40 WU (1.2 m) above flat ground
(0.377 to 0.457 WU per point in the simulation). With P0 = 0 (no
driver) the springs are off and the Ghost rests on its spheres; a
driver lifts it over 2 s and it settles in about 2 s more.

### The fighter (Banshee)

Physics radius <= 0, two channels ('body antigrav' height 0.75,
'wing antigrav' height 0.25, strength 1, damping 0.01, k1 0.1, k0 0),
both with antigravity fraction P0 and no rotation. Before the step:

1. **Thrust.** Desired velocity V_d = f · s. Throttle k = s/maximum
   forward speed if s > 0, else −s/maximum reverse speed. A = V_d − v
   + (0, 0, g). Limits: high = k · speed acceleration, low = k · speed
   deceleration; if V_d · A > 1e-6 then high = (high − low) (V_d·A)^2 /
   (|A|^2 |V_d|^2) + low, else high = low; A is limited to length
   high. F_m = A · M · P0. With s = 0 there is **no lift**: the
   Banshee falls (only its antigravity cushions it near the ground).
2. **Attitude.** Desired forward D_f = the driver's facing d; desired
   up D_u = normalise(z − D_f (D_f · z)) (world x if that is 0); for a
   player pilot, the pair is pitched nose-up by the fixed gun pitch
   (D_f' = D_f cos θ + D_u sin θ, D_u' = D_u cos θ − D_f sin θ; θ =
   0.261799, 15 degrees). Bank: β_b = ((D_f,x v_y − D_f,y v_x) /
   maximum forward speed) · (maximum left turn, 1.5708 on the disc);
   D_u is rotated about D_f by β_b (toward the right for positive
   values: the Banshee banks into the side its velocity is lagging).
   The rotation taking the current frame to the desired one has angle
   ψ and axis k; desired angular velocity ω_d = k ψ · (turn rate, 0.2)
   / π (a proportional approach with time constant π/0.2 = 15.7
   ticks). T_m = (ω_d − ω) · (xx + yy + zz)/3 · P0.
3. Thrust value (effects only): spin ratio q = |ω|/0.2; if q > thrust,
   thrust += min(q − thrust, clamp(0.2 (1 − thrust)^2, 0.01, 0.05));
   else thrust += max(q − thrust, −max(0.05 thrust^2, 0.005)).
4. Then the step. Hover and thruster effects come from the vehicle's
   effect at the "hover thrusters" markers.

Air vehicles (plane and fighter) are also held between the level's
vehicle floor and ceiling (`structure_bsp.md` 0x010, 0x014) after the
step, unless the vehicle has no collision: below a nonzero floor z_f,
v_z += ((z_f − p_z) · 0.015625 − v_z · 0.0625) P0; above a nonzero
ceiling z_c, v_z −= ((p_z − z_c) · 0.015625 + v_z · 0.0625) P0. All
multiplayer BSPs have both 0; b40 and c40 have ceilings 175, 190 and
235 WU on three BSPs and b40 a floor of −140 on one.

### Other types (campaign, summary)

- **Turret** (6; the Covenant gun turret, the a10 chairs and cryotube):
  the step with no channels.
- **Human plane** (3; Pelican, dropship, fighter-bomber; physics radius
  4: the older model): while hovering (a script switch) the mass points
  are cleared and no physics runs; otherwise throttle k = (clamp(s, 0,
  S_f)/S_f)^2, hover fraction eases (0.05 per tick) toward (0.25 if
  crouch, 1 if jump, else 0.75) · (1 − k) · P0, thrust = k P0, drive
  (s − f·v) · thrust · M · 0.05 along f, lift (|f·v|/S_f · 1.05 + hover
  · 1.3) g M along u, attitude torque toward the desired frame (with a
  bank of (v_y d_x − v_x d_y) (π/2)/|S_f|) of (angle/30 axis − ω) ·
  radius^2 · M · 0.05; all times P0.
- **Fighter, older model** (lifepod, physics radius 1): like the plane
  without hover: thrust (s − f·v) M 0.05, lift |f·v/S_f| M g 1.05.
- **Human boat** (2): no disc tag uses it (three channels: water
  friction speed s on channel 0 yawed by up to τ/2 at low speed, water
  lift ratios 0.003, 0.003, 0.005, a roll-levelling torque about f at
  most 0.8 degrees per tick per tick).

### Sleeping, waking and the slow stop

- A sleeping vehicle runs no physics. Each tick, before the physics
  decision, it wakes if: flag 0 and s ≠ 0; flag 1 and τ ≠ 0; flag 2
  and P0 ≠ 0; flag 3 and P1 ≠ 0; flag 5 and σ_s ≠ 0. Pushes, flips,
  vehicle contacts and attachment also wake it.
- On the tick it falls asleep a stop counter is set to 15. While asleep
  with the counter > 0: counter −= 1, v and ω are multiplied by 0.835,
  p += v, the orientation turns by ω; when the counter reaches 0, v and
  ω are zeroed. The suspension overlay is still updated.
- A vehicle attached to a parent has its velocities zeroed and is not
  simulated.

Disc flags: the Warthog and Scorpion wake only on speed (a driver who
only steers does not wake a sleeping Warthog, but throttle does); the
Ghost and Banshee also wake on driver power, which is not 0 while a
driver sits in them, so they never sleep while driven.

### Terminal velocity

With vehicle flag 6, after the physics: if v_z < −(the globals'
runtime maximum falling velocity, 0.3430028 WU/t), every child object
(riders included) takes the globals' "maximum distance" damage (the
lethal fall, `globals.md`).

### Collisions

**Level.** The springs and the penetration freeze are the level
response. Suspension and crash effects follow the step:

- *Suspension (visual and sound).* For each suspension element of the
  vehicle's animation graph (`animations.md`: mass point index, overlay
  animation, full extension depth E, full compression depth C;
  Warthog: the four tyres, E −0.31445, C −0.13452; Scorpion: eight
  tread points), from the mass point's position moved along its up by
  (C − c_z) − (E − C) a trace of length 2(E − C) along the up is cast;
  the hit fraction t gives the target compression k = clamp(2(1 − t),
  0, 1) (no hit: t = 1, k = 0); the stored value becomes the average of
  k and the previous value, quantised to a byte, and the overlay plays
  at that fraction. If some wheel's k − previous exceeds 0.3, the
  suspension sound plays at scale clamp((jump − 0.3)/0.6, 0, 1).
- *Crashes.* Only when no suspension sound played this tick: Δv = v'
  − v_before; if |Δv| > 0.02 and some point touches, the globals'
  vehicle-hit-environment damage (all zero on the disc) is applied to
  the vehicle at scale clamp((|Δv| − 0.02) · 45.454544, 0, 1) and the
  crash sound plays at that scale.
- *Sliding effects.* Per touching point with ground-relative speed
  above 0.03, the material effects tag's effect for the ground material
  (type 10 for metallic points, 9 otherwise) at scale clamp((speed −
  0.03) · 4.5454545, 0, 1), at the contact point.

**Other vehicles.** After its own step, each vehicle finds bipeds and
vehicles inside its bounding sphere. For another vehicle B (physics
radius <= 0 on the disc's four) the pair is tested when B's index is
lower, or B is asleep: for every pair of mass points with centre
distance d < R_a + R_b and d > 0, penetration π_p = (R_a + R_b − d)/2,
force magnitude 2 sqrt(M_a M_b) g / 0.2 · π_p, pushing the points apart
along the line, applied at the point π_p short of A's sphere surface;
forces and torques (about each vehicle's centre) are added to both
vehicles' collision accumulators and both are woken. The accumulators
enter each vehicle's next step.

**Bipeds.** For each living biped in the sphere whose pill base is
inside the vehicle's collision model (or within its features):

- push Δv: n = normalise(normalise(c_b − c_v) + (0, 0, 0.8)), Δv = (n
  · max(|v_vehicle|, 0.1) + v_vehicle) / 2, given to the biped as a
  push (`movement.md`: a biped takes half a push);
- the biped's pill is moved by 2 Δv and fixed out of the vehicle;
- **splatter**: unless the fix succeeded and (the biped left this
  vehicle less than 90 ticks ago, or the vehicle's speed is at most
  0.06666667 WU/t (2 WU/s) and |v_vehicle − v_biped|^2 <= 0.0011111111),
  the biped takes the globals' vehicle collision damage (1000 points,
  `projectiles.md`) at scale 1, flagged as area damage, owned by the
  driver (or the vehicle when empty), from the vehicle's centre. 1000
  kills any multiplayer biped. The vehicle takes the globals' "vehicle
  killed unit" effect (all zero) scaled 0.5, 0.25 or 1 by the biped's
  blip size.

### Pushes on the vehicle

A damage push (`projectiles.md`, `health_shields.md`: Δv = instantaneous
acceleration × the vehicle's object acceleration scale / 30 along the
lifted direction, doubled for effects that detonate explosives) and a
melee push (`weapons.md`, 0.035 per unit of acceleration scale) are
applied instantly when the vehicle has physics:

  v += Δv,  ω += π (z × Δv)

and the vehicle wakes. The spin term is large: a rocket on a Warthog
(acceleration scale 0.3, Δv 0.12 WU/t mostly horizontal) adds about
0.36 rad/t (21 degrees per tick) of roll or pitch, which is why
explosions tumble vehicles. Object acceleration scales: Warthog 0.3,
Ghost 0.3, Banshee 0.3, Scorpion 0.07.

### Entering, exiting and flipping

**Examination.** Each tick a player on foot looks at objects inside its
own bounding sphere (`multiplayer_rules.md` covers items). For a living
vehicle:

- If the vehicle's up.z > cos(90° − minimum vehicle flipping angle) =
  cos 45° = 0.7071 (`globals.md` player control): if the player is not
  overcharged, |v_player|^2 < 0.01 and |ω_vehicle|^2 < 0.01, the best
  seat is searched (below). An available seat makes the **enter**
  prompt (with the seat's HUD text message index), an AI-occupied seat
  that the AI agrees to give up makes the **evict** prompt.
- Otherwise (tilted 45 degrees or more), if the vehicle is not already
  flipping and has no driver: the **flip** prompt. A flip prompt
  overrides other prompts; between equal prompts the nearer object
  wins.

**Seat search.** For each seat with a non-empty label that the
player's graph has an animation set for: the entry point is the seat
marker composed with the root displacement of frame 0 of the player's
"enter" animation for that seat (where the animation starts); distance
= min(|entry − c_player|, |seat marker − c_player|) with c_player the
bounding sphere centre. Candidates: distance < 1 WU; seat flag 9
requires a driver present; the seat must be enterable (below). Ranking:
an available seat beats an occupied one; else the nearer, where once a
driver seat is the best so far a non-driver seat's distance counts
1.5 times.

**Who may enter.** A seat is enterable when no unit sits in that seat
and (for a player) no unit seated anywhere in the vehicle is an enemy
of the player's team: a player cannot board an enemy-occupied vehicle
at all (no hijacking on the Xbox).

**Pressing action.** The action button is level-triggered: on any tick
the player is on foot with action held and a prompt set, the action
happens (if none applies, it becomes a reload). Entering:

- the unit is attached to the seat marker (parent = vehicle), its seat
  set, the vehicle's driver and gunner recomputed (the first occupant
  of a driver seat drives; of a gunner seat guns; a driver-gunner seat
  takes both); the unit readies its next weapon and picks the seat
  label with that weapon class (falling back to the seat's unarmed
  set); it unzooms;
- the seat's enter animation starts (6-tick blend) from the unit's
  current offset relative to the seat marker, which the animation
  interpolates away: the visible walk-in. While entering, the unit's
  controls are not passed to the vehicle (players' are: the player's
  throttle drives as soon as it is seated; AI's wait).

**Exiting.** A seated unit with action held and no busy animation
starts its seat's exit animation (slot 8); if it is the driver the
vehicle plays its opening animation. When the exit animation ends: the
unit is detached at the position of its root node (minus the root
node's default z translation), facing the root node's direction, made
visible, with the exit animation's last-frame root displacement as its
velocity (`animations.md`), in the airborne state; the vehicle loses it
as driver or gunner; the unit's last vehicle and exit time are
recorded (splatter grace).

**Ejection.** Each tick a seated unit whose vehicle has up.z < 0.2 and
is on the ground leaves at once (no animation): flipped vehicles throw
their riders.

**Flipping.** Pressing action on the flip prompt records the player's
last vehicle (no splatter for 90 ticks) and starts the flip with an
approach type: if |f_z| > 0.7071, type 3 (nose up) or 4 (nose down);
else Δ = vehicle − player position, type 2 if (z × Δ) · f > 0 else 1.
Each tick while flipping, the type is set, fewer than 30 ticks have
passed and up.z <= 0.9:

- strength k = clamp(−2 up.z, minimum, maximum flipping angular
  velocity) (0.2 to 0.75 on the disc);
- axis: f × u for types 3 and 4, f for types 1 and 2, with types 1
  and 2 then using f + (f × u)(−f_z);
- ω = axis · (±0.3 k) (+ for types 2 and 4, − for 1 and 3): the angular
  velocity is **set**, then the vehicle wakes; a fighter's v_z = min(v_z,
  −0.01); a tank's velocity is cut to its forward part;
- the physics then runs normally.

Upside down (up.z = −1) the vehicle turns at 0.225 rad/t and rights
itself in about 14 ticks; on its side (up.z = 0) at 0.06 rad/t. The
flip stops after 30 ticks or once up.z > 0.9.

**Death.** Vehicles do not die on the disc (Damage). A rider killed in
its seat stays attached as a body (`health_shields.md`).

### Driving input and the camera

**Look.** A seated player's look works as on foot (`globals.md` look
function, acceleration, magnetism) except:

- yaw and pitch rates: the seat's yaw rate and pitch rate (degrees per
  second) replace the player's look rates when > 0: Warthog driver 60
  and 30, gunner 60 (pitch: the player's), Ghost and Banshee drivers 80,
  Scorpion driver 50. These are the vehicles' turning feel, since the
  camera is the steering input;
- yaw limits: if the seat's yaw range is not (0, 0), the desired yaw is
  kept inside [marker yaw + minimum, marker yaw + maximum] (marker yaw
  = the seat marker's forward); a yaw outside the arc snaps to the
  nearer end (by signed angular difference). Only the Warthog
  passenger has one: ±90 degrees;
- pitch limits: the seat camera's (minimum, maximum) when not both 0,
  shifted by the vehicle's tilt along the look direction (when up.z >
  0.2: tilt = 90° − angle(look yaw direction, vehicle up)), clamped to
  ±85.5 degrees; the active limits move toward these by π/256 rad per
  tick, and pitch is clamped to the active limits;
- auto-level: if the seat camera's pitch auto level ψ_a ≠ 0, pitch
  moves toward ψ_a − tilt by at most |pitch − ψ_a| (2/π) 0.08 |v_vehicle|
  per tick (only while moving).

**Steering.** The driver's desired facing d (from the look yaw and
pitch) is the steering input of every type (σ above): the Warthog
steers its wheels toward it, the Scorpion turns its treads toward it,
the Ghost yaws toward it, the Banshee points its nose 15 degrees above
it. The stick's forward axis is t_i, its strafe axis t_j (only the
Ghost uses t_j). Aircraft pilots may have their own look inversion
preference.

**Keyboard and mouse** (consistent with the rule; the engine's Intent
supplies throttle and look): W and S give t_i = ±1 (full speed), A and
D give t_j = ±1 (A left, Ghost strafe), the mouse moves the look (yaw
and pitch through the same seat limits; the seat yaw rate is a
stick-only rate), Space is jump (brake for the Warthog and Scorpion,
the Ghost's jump flag, the Banshee's nothing), Ctrl crouch, the action
key enters, exits and flips, the primary fire button fires the
gunner's or driver-gunner's weapon trigger 0, the secondary button
(grenade) fires trigger 1 where there is one (Scorpion machine gun).

**Third-person camera.** For a seat with the invisible, driver or third
person camera flag, the seat's camera is used, else the unit's. Focus:
the rider's camera point (the seat's camera marker if named). The first
of the seat's camera tracks (only index 0; the "tight" second track of
the Warthog is never used) gives an offset as a function of pitch: t =
(pitch + π/2)/π, a uniform cubic spline through the track's control
point positions (at least 4; the disc's have 9 evenly spaced from
pitch −90 to +90 degrees). depth = |offset|; camera offset (along the
horizontal look, sideways, up) = (depth cos pitch + o_x, −o_y, depth sin
pitch + o_z); the observer places the camera depth behind the focus
along the look plus that offset (`hud.md` and the observer own the
smoothing; a seat change blends over 1 s). Field of view 70 degrees.
The track points at pitch 0 (control point 4, x, y, z in WU):

| Track | Used by | Pitch 0 point | Pitch −90 | Pitch +90 |
|---|---|---|---|---|
| `camera\warthog_driver_loose` | Warthog all seats, Ghost driver | (−2.7524, 0, 0.6827) | (0.5974, 0, 3.5431) | (−1.0209, 0, −0.1809) |
| `camera\scorpion_driver_loose` | Scorpion driver | (−4.6130, 0, 2.1571) | (0.3328, 0, 4.6906) | (−1.8471, 0, −0.1524) |
| `camera\banshee_driver_loose` | Banshee driver | (−2.9329, 0, 1.1784) | (1.6825, 0, 4.2036) | (−0.7176, 0, −2.4833) |
| `globals\default unit camera track` | seats without a track (Scorpion passengers) | (−3.6826, 0, 0.5752) | (0.2387, −0.0058, 3.8862) | (−0.0381, −0.0038, −1.9795) |

Full lists: Warthog loose x, z per point (pitch −90, −67.5, ..., +90):
(0.5974, 3.5431), (−0.8369, 3.2299), (−1.9179, 2.4325), (−2.5767,
1.6104), (−2.7524, 0.6827), (−2.4676, 0.1141), (−1.8786, −0.1244),
(−1.3638, −0.1717), (−1.0209, −0.1809). Scorpion: (0.3328, 4.6906),
(−1.9374, 4.5644), (−3.3776, 4.1085), (−4.2563, 3.2836), (−4.6130,
2.1571), (−4.3739, 1.0201), (−3.7841, 0.2852), (−2.7714, −0.0658),
(−1.8471, −0.1524). Banshee: (1.6825, 4.2036), (−0.3336, 4.0599),
(−1.9989, 3.2481), (−2.6678, 2.3735), (−2.9329, 1.1784), (−2.7761,
0.0856), (−2.3899, −0.8667), (−1.7120, −1.7484), (−0.7176, −2.4833).
y is 0 on all three.

### Turrets and guns

- The vehicle holds its weapon (unit weapons block); it is readied at
  creation. A seat with the gunner flag (or invisible) makes its
  occupant aim *through* the vehicle: the vehicle's desired aiming
  vector is the gunner's, and the vehicle's triggers are the gunner's
  primary (fire) and secondary (grenade button) triggers. The weapon
  fires from the vehicle exactly as `weapons.md` describes (its own
  triggers, rates, heat and ammunition), with the vehicle as the
  shooter (the vehicle is ignored by its own shots, `projectiles.md`).
- The vehicle's **aiming vector** follows the desired one with the
  unit's aiming velocity maximum / 30 per tick and acceleration maximum
  / 900 per tick^2 (Warthog and Banshee π rad/s and 10.472 rad/s^2,
  Scorpion π/2 and 13.963), inside the vehicle graph's aiming screen
  (`animations.md`); a unit with both 0 (Ghost) aims instantly, clipped
  to its aiming bounds.
- Warthog: the gunner seat (W-gunner) is the gunner; the chaingun is
  `vehicles\warthog\warthog gun` (one trigger, 8 to 15 rounds per
  second as it spins up, `weapons.md`). The driver does not fire; the
  passenger fires its own weapon if its graph has W-passenger for that
  weapon class.
- Scorpion: the driver seat is driver and gunner: the turret aims where
  the driver looks; trigger 0 (fire) is the tank shell, chamber-limited
  (`weapons.md`), trigger 1 (grenade button) the machine gun at 15 per
  second. The four passenger seats need a driver (flag 9) and give no
  weapon on the multiplayer tag.
- Ghost: driver-gunner, `vehicles\ghost\ghost gun` (5 to 6 bolts per
  second, heat as `weapons.md`).
- Banshee (campaign only): driver-gunner, `vehicles\banshee\banshee
  gun`: trigger 0 the bolts (5 to 6 per second, `vehicles\banshee\
  banshee bolt`), trigger 1 the fuel rod (`vehicles\banshee\banshee
  fuel rod`). There is no Banshee, and so no fuel rod, in Xbox
  multiplayer.

### Damage

- On the disc the three multiplayer vehicles' collision models have 0
  body and 0 shield vitality (`health_shields.md`), so hits scale to 0
  and the vehicles **cannot be destroyed**; the Banshee, Ghost and
  Scorpion campaign tags use their own collision models (not read
  here; see Open questions).
- Riders: a direct hit on the vehicle passes to each seated player and
  the driver with multiplier (1 − the effect's vehicle passthrough
  penalty) × rider damage fraction (0.18 on the three multiplayer tags
  and the campaign Warthog and Scorpion; **0 on the campaign Ghost and
  the Banshee**). Rule and penalties are `health_shields.md` and
  `projectiles.md`.
- Pushes from damage: Pushes on the vehicle. The globals' vehicle hit
  environment and vehicle killed unit effects do nothing on the disc.

### Multiplayer

| Map | Placements (scenario) |
|---|---|
| Blood Gulch | 2 Warthogs, 4 `ghost_mp`, 2 `scorpion_mp` |
| Sidewinder | 2 Warthogs, 4 `ghost_mp`, 2 `scorpion_mp` |
| the other 11 multiplayer maps | none (the tags are present but unplaced) |

Blood Gulch, position (WU) and yaw (degrees): Warthog (29.14, −90.83,
0.29) −48.6 and (102.31, −144.63, 0.58) 108.6; Ghost (26.99, −86.83,
0.43) −44.6, (51.11, −89.98, 0.20) −120.7, (104.69, −149.32, 0.20)
106.9, (82.22, −153.99, −0.09) 114.9; Scorpion (95.46, −174.39, 0.75)
92.3 and (40.38, −63.96, 0.91) −92.3. Sidewinder: Warthog (30.10,
−30.77, −3.69) 87.6 and (−31.89, −26.67, −3.84) 90.4; Ghost (23.35,
−30.70, −3.72) 82.5, (36.82, −30.71, −3.80) 87.1, (−38.32, −25.89,
−3.58) 93.6, (−25.50, −26.16, −3.84) 77.8; Scorpion (−31.96, −20.58,
−3.83) 93.3 and (30.46, −24.41, −3.44) 90.6.

- The game type's vehicle set (`multiplayer_rules.md`: 0 all, 1 none,
  2 Warthog, 3 Ghost, 4 Scorpion) keeps only that kind; the placements
  carry no team or game type data on the disc.
- **No respawn** (confirming `multiplayer_rules.md`): the reference
  has no vehicle respawn; an abandoned vehicle stays where it is for
  the whole game, and there is no abandoned-vehicle timer.
- Race creates its own vehicles at the race vehicle flags
  (`multiplayer_rules.md`).

### Exported values and animation

- The vehicle's animation (`animations.md` vehicle slots): steering
  overlay at τ (Warthog bounds 35 degrees), roll overlay at frame
  ((u, f, v) triple product / S_f + 1)/2, throttle overlay at s
  (negative half for reverse), velocity overlay at clamp(v·f, 0,
  1)/S_f, ground-speed overlay at W / wheel circumference (the wheel
  spin follows the commanded speed s, not the ground speed: spinning
  wheels while stuck), suspension overlays as above.
- The `~blur` region permutation shows while |s| >= blur speed.

## The disc's values

Every vehicle tag was read on every map that holds it; each is
byte-identical (as read field by field) across its maps.

### Which maps hold which vehicles

| Tag | Type | Maps |
|---|---|---|
| `vehicles\warthog\warthog` | jeep | all 13 multiplayer maps; a10, a30, b30, b40, c40, d40 |
| `vehicles\ghost\ghost_mp` | scout | all 13 multiplayer maps |
| `vehicles\scorpion\scorpion_mp` | tank | all 13 multiplayer maps |
| `vehicles\ghost\ghost` | scout | b30, b40, c40, d20 |
| `vehicles\scorpion\scorpion` | tank | a10, b40 |
| `vehicles\scorpion\scorpion_cinematic` | tank | a10 |
| `vehicles\banshee\banshee` | fighter | a30, b30, b40, c40, d20, d40 |
| `vehicles\banshee\banshee_cinematic` | fighter | d40 |
| `vehicles\banshee\banshee_oldplayback` | fighter (older model) | d20 |
| `vehicles\wraith\wraith` | scout | a50, b40, c40 |
| `vehicles\c gun turret\c gun turret` | turret | a50, b40, c10, c40, d20, d40 |
| `vehicles\pelican\pelican` | plane | a10, a30, a50, b30, b40, c10 |
| `vehicles\pelican\pelican_astral`, `vehicles\fighterbomber\fighterbomber`, `vehicles\lifepod_docked\lifepod_docked` | plane, plane, turret | a10 |
| `vehicles\pelican\pelican_crashing` | plane | d40 |
| `vehicles\c_dropship\c_dropship` | plane | a30, a50, b30, b40, c40, d20, d40 |
| `vehicles\lifepod\lifepod` | fighter (older model) | a10, a30, b30 |
| `vehicles\lifepod_entry\lifepod_entry`, `scenery\vehicles\lifepod_atmosphere_entry\...` | fighter | a30; a10 |
| a10 chairs, cryotube, `scenery\halo\halo`; `cinematics\props\b30_falling_box` | turret; jeep | a10; b30 |

c20 and `ui` hold no vehicle.

### Vehicle fields of the four

| Field | Warthog | Ghost (`ghost_mp`) | Scorpion (`scorpion_mp`) | Banshee |
|---|---|---|---|---|
| flags | 0xCD1 (0, 4, 6, 7, 10, 11) | 0x29C5 (0, 2, 6, 7, 8, 11, 13) | 0x8D1 (0, 4, 6, 7, 11) | 0x1984 (2, 7, 8, 11, 12) |
| type | 1 jeep | 4 scout | 0 tank | 5 fighter |
| max forward speed (WU/t) | 0.255 (23.32 m/s) | 0.225 (20.57 m/s) | 0.14 (12.80 m/s) | 0.22 (20.12 m/s) |
| max reverse speed | 0.1 | 0 | 0.12 | 0.03 |
| speed acceleration | 0.00275 | 0.005 | 0.002 | 0.008 |
| speed deceleration | 0.011 | 0 | 0.006 | 0.016 |
| max left / right turn | 30 / −30 (deg) | 0 / 0 | 0 / 0 | 1.5708 / 0 (bank) |
| wheel circumference | 1 | 0 | 2 | 0 |
| turn rate | 180 (deg/s) | 0 | 0 | 0.2 (attitude) |
| blur speed | 0.08 | 0 | 0.05 | 0 |
| A, B, C, D in | 0, 12, 35, 0 | 0, 35, 0, 0 | 20, 21, 0, 35 | 0, 1, 36, 35 |
| slide fields | 0 | 0 | 0 | 0 |
| flipping angular velocity min, max | 0.2, 0.75 | 0.2, 0.75 | 0.2, 0.75 | 0.2, 0.75 |
| fixed gun yaw, pitch | 0, 0 | 0, 0 | 0, 0 | 0, 0.261799 |
| AI: sideslip, destination, avoidance, pathfinding | 0, 1.75, 0, 1 | 4, 1.5, 0, 0 | all 0 | 0, 5, 8, 1.8 |
| AI: charge timeout, strafe abort, oversteer, steer max, throttle max, move time | 0 | 0 | 0 | 9, 5.5, 0.0349 to 0.5236, 0.6981, 0, 0 |
| suspension sound | `sound\sfx\impulse\impacts\warthog_suspension` | the same | `...\scorpion_suspension` | warthog_suspension |
| crash sound | `...\jeep_hit_solid` | `...\jeep_hit_solid` | none | `...\jeep_hit_solid` |
| material effects | `vehicles\vehicle` (`foot`) | the same | the same | the same |
| effect | none | `vehicles\banshee\effects\antigravity particles` | none | the same as the Ghost |
| object bounding radius | 1.1 | 0.8 | 2.2 | 1.4 |
| object acceleration scale | 0.3 | 0.3 | 0.07 | 0.3 |
| unit flags | 0x41 | 0 | 0x1 | 0 |
| rider damage fraction | 0.18 | 0.18 | 0.18 | 0 |
| unit inputs A..D | 1, 0, 0, 0 | 1, 0, 0, 0 | 0, 0, 1, 0 | 1, 0, 0, 0 |
| aiming velocity max, accel max | 3.14159, 10.472 | 0, 0 | 1.5708, 13.9626 | 3.14159, 10.472 |
| powered seat (up, down s) | 1.0, 0.1 | 2.0, 1.0 | 1.0, 0.1 | 2.0, 1.0 |
| weapon | `vehicles\warthog\warthog gun` | `vehicles\ghost\ghost gun` | `vehicles\scorpion\scorpion cannon` | `vehicles\banshee\banshee gun` |
| unit HUD | none (per seat) | none | none | `ui\hud\banshee` |

Campaign variants: `vehicles\ghost\ghost` differs from `ghost_mp` only
in AI sideslip 6, pathfinding radius 0.8, rider damage fraction 0, a
unit HUD `ui\hud\ghost` and a second seat 'G-driver elite' (marker
`driver_elite`, otherwise as G-driver); `vehicles\scorpion\scorpion`
differs from `scorpion_mp` in AI destination radius 2.5, pathfinding
radius 2, AI throttle maximum 0.5, and its passenger seats have flags
0x240 and HUDs `ui\hud\passenger_4`, `passenger`, `passenger_2`,
`passenger_3`. The Warthog is one tag everywhere.

### Seats

Seat flags in hex (bits), acceleration scale, yaw and pitch rates
(deg/s), camera marker, pitch auto level, pitch range (rad), tracks,
HUD, text index, yaw range (rad). Built-in gunner null everywhere.

| Vehicle: seat | Flags | Marker | Accel scale | Yaw, pitch rate | Camera marker | Auto level | Pitch range | Tracks | HUD | Text | Yaw range |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Warthog: W-driver | 0x14 (2, 4) | driver | 100, 100, 50 | 60, 30 | (none) | −0.3141593 | −0.5235988 to 0.2617994 | warthog_driver_loose, warthog_driver_tight | `ui\hud\warthog driver` | 13 | none |
| Warthog: W-passenger | 0x60 (5, 6) | passenger | 100, 100, 50 | 0, 0 | camera rider | 0 | −0.8726646 to 1.570796 | warthog_driver_loose | `ui\hud\passenger_2` | 15 | −1.570796 to 1.570796 |
| Warthog: W-gunner | 0x198 (3, 4, 7, 8) | gunner | 100, 100, 50 | 60, 0 | camera gunner | 0 | −0.3490658 to 0.6108652 | warthog_driver_loose | `ui\hud\gunner` | 14 | none |
| Ghost: G-driver | 0x1C (2, 3, 4) | driver | 25, 100, 100 | 80, 0 | driver camera | −0.08726646 | −0.5235988 to 0.5235988 | warthog_driver_loose | none | 13 | none |
| Scorpion: scorpion-driver | 0x1C (2, 3, 4) | driver | 100, 100, 50 | 50, 0 | (none) | −0.3141593 | −0.5235988 to 0.6108652 | scorpion_driver_loose, warthog_driver_tight | none | 13 | none |
| Scorpion: scorpionLF, RF, LB, RB | 0x200 (9) | passenger lf, rf, lb, rb | 100, 100, 50 | 0, 0 | passenger lf camera (etc.) | −0.3141593 | −0.5235988 to 0.6108652 | none (default track) | none | 15 | −1.570796 to 1.570796 |
| Banshee: B-driver | 0x1C (2, 3, 4) | driver | 100, 100, 50 | 80, 0 | driver camera | 0 | none (0, 0) | banshee_driver_loose | none | 13 | none |

### Physics: the four

| Field | Warthog | Ghost | Scorpion | Banshee |
|---|---|---|---|---|
| tag | `vehicles\warthog\warthog` | `vehicles\ghost\ghost` | `vehicles\scorpion\scorpion` | `vehicles\banshee\banshee` |
| radius | −1 | −1 | −1 | −1 |
| moment scale | 1 | 1 | 1 | 1 |
| mass | 5000 | 2000 | 20000 | 4000 |
| center of mass | (0.106921, 0, 0.321783) | (0.362, 0, 0.108) | (−0.033818, 0, 0.423552) | (−0.116623, 0, 0.597857) |
| density | 5 | 3 | 8 | 4 |
| gravity scale | 1 | 1 | 1 | 1 |
| ground friction | 0.2 | 0.2 | 0.2 | 0.2 |
| ground depth | 0.15 | 0.15 | 0.25 | 0.15 |
| ground damp fraction | 0.05 | 0.05 | 0.05 | 0.05 |
| ground normal k1, k0 | 0.707107, 0.5 | the same | the same | the same |
| water friction, depth, density | 0.05, 0.25, 1 | the same | the same | the same |
| air friction | 0.005 | 0.0025 | 0.001 | 0.005 |
| xx, yy, zz moment | 595.045, 1689.021, 1990.222 | 189.408, 301.6, 423.648 | 8785.955, 16731.90, 21959.68 | 1497.566, 992.686, 1470.974 |
| tensor xz (= zx) | 85.45504 | 5.712 | 1015.991 | 147.3873 |
| inverse (xx, yy, zz, xz) | 0.001691, 0.0005921, 0.0005056, −7.26e-5 | 0.0052818, 0.0033156, 0.0023614, −7.12e-5 | 1.144e-4, 5.98e-5, 4.58e-5, −5.3e-6 | 6.744e-4, 1.0074e-3, 6.866e-4, −6.76e-5 |
| channels | 0 front, 1 back: flags 0x1, all else 0 | 0 front antigrav, 1 back antigrav: 0x40, strength 1.5, height 0.75, damp 0.02, k1 0.5, k0 0.258819 | 0 left, 1 right: 0x1 | 0 body antigrav (height 0.75), 1 wing antigrav (0.25): 0x40, strength 1, damp 0.01, k1 0.1, k0 0 |
| mass points | 15, relative masses sum 24.5 | 5, sum 5 | 18, sum 18 | 22, sum 22 |

(The other off-diagonal terms are 0 to 1e-5: every vehicle is
symmetric in y.)

**Warthog mass points** (all forward (1, 0, 0), up (0, 0, 1); m from
relative mass × 5000/24.5; density 6.836735, tyres 3.418367):

| # | Name | Channel | Node | Metal | Rel. mass | Mass | Rel. density | Position | Friction (type: along, across) | Radius |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | bumper back left | − | 0 | no | 1 | 204.0816 | 1 | (−0.77711, 0.18628, 0.40146) | point 1, 1 | 0.3 |
| 1 | bumper back right | − | 0 | no | 1 | 204.0816 | 1 | (−0.77711, −0.18628, 0.40146) | point | 0.3 |
| 2 | hull bottom middle right | − | 0 | no | 3 | 612.2449 | 1 | (0.00113, −0.18131, 0.39201) | point | 0.3 |
| 3 | hull bottom middle left | − | 0 | no | 3 | 612.2449 | 1 | (0.00113, 0.18131, 0.39201) | point | 0.3 |
| 4 | hull top back left | − | 0 | yes | 1 | 204.0816 | 1 | (−0.41087, 0.21523, 0.40143) | point | 0.3 |
| 5 | hull top back right | − | 0 | yes | 1 | 204.0816 | 1 | (−0.41087, −0.21523, 0.40143) | point | 0.3 |
| 6 | hull top front right | − | 0 | yes | 1.5 | 306.1224 | 1 | (0.42573, −0.16515, 0.40126) | point | 0.3 |
| 7 | hull top front left | − | 0 | yes | 1.5 | 306.1224 | 1 | (0.42573, 0.16515, 0.40126) | point | 0.3 |
| 8 | bumper front left | − | 0 | no | 1 | 204.0816 | 1 | (0.84754, 0.16494, 0.32011) | point | 0.3 |
| 9 | bumper front right | − | 0 | no | 1 | 204.0816 | 1 | (0.84754, −0.16494, 0.32011) | point | 0.3 |
| 10 | top of gun | − | 0 | yes | 0.5 | 102.0408 | 1 | (−0.22703, 0, 0.7437) | point | 0.3 |
| 11 | tire back left | 1 back | 14 | no | 1.5 | 306.1224 | 0.5 | (−0.63, 0.38, 0.19) | forward 0.45, 0.65 | 0.26 |
| 12 | tire back right | 1 back | 16 | no | 1.5 | 306.1224 | 0.5 | (−0.63, −0.38, 0.19) | forward 0.45, 0.65 | 0.26 |
| 13 | tire front left | 0 front | 15 | no | 3 | 612.2449 | 0.5 | (0.67, 0.38, 0.19) | forward 0.75, 0.45 | 0.26 |
| 14 | tire front right | 0 front | 17 | no | 3 | 612.2449 | 0.5 | (0.67, −0.38, 0.19) | forward 0.75, 0.45 | 0.26 |

**Scorpion mass points** (all relative mass 1, mass 1111.111, relative
density 1, density 8, friction point 1, 1, forward (1, 0, 0), up
(0, 0, 1)):

| # | Name | Channel | Node | Metal | Position | Radius |
|---|---|---|---|---|---|---|
| 0 | tread front left front | 0 left | 4 | yes | (1.2, 0.78, 0.218) | 0.32 |
| 1 | tread front left back | 0 left | 4 | yes | (0.32, 0.78, 0.256) | 0.36 |
| 2 | tread back left front | 0 left | 3 | yes | (−0.53, 0.87, 0.256) | 0.36 |
| 3 | tread back left back | 0 left | 3 | yes | (−1.15927, 0.87, 0.218) | 0.32 |
| 4 | tread back right back | 1 right | 5 | yes | (−1.15927, −0.87, 0.218) | 0.32 |
| 5 | tread back right front | 1 right | 5 | yes | (−0.53, −0.87, 0.256) | 0.36 |
| 6 | tread front right back | 1 right | 6 | yes | (0.32, −0.78, 0.256) | 0.36 |
| 7 | tread front right front | 1 right | 6 | yes | (1.2, −0.78, 0.218) | 0.32 |
| 8 | turret front | − | 8 | yes | (−0.285636, 0, 1.11114) | 0.35 |
| 9 | turret back | − | 7 | yes | (−0.944265, 0, 1.00178) | 0.48 |
| 10 | hull back left | − | 0 | yes | (−1.05126, 0.2, 0.48964) | 0.4 |
| 11 | hull mid right | − | 0 | yes | (0.543795, −0.2, 0.413791) | 0.38 |
| 12 | hull front right | − | 0 | yes | (1.18333, −0.22, 0.40408) | 0.3 |
| 13 | hull front left | − | 0 | yes | (1.18333, 0.22, 0.40408) | 0.3 |
| 14 | hull mid left | − | 0 | yes | (0.543795, 0.2, 0.413791) | 0.38 |
| 15 | hull hatch left | − | 0 | yes | (−0.196007, 0.22, 0.5) | 0.44 |
| 16 | hull hatch right | − | 0 | yes | (−0.196007, −0.22, 0.5) | 0.44 |
| 17 | hull back right | − | 0 | yes | (−1.05126, −0.2, 0.48964) | 0.4 |

**Ghost mass points** (relative mass 1, mass 400, density 3, metallic,
friction point, forward (1, 0, 0), up (0, 0, 1)):

| # | Name | Channel | Node | Position | Radius |
|---|---|---|---|---|---|
| 0 | hull back seat | 1 back antigrav | 4 | (−0.25, 0, 0.08) | 0.22 |
| 1 | hull mid | − | 0 | (0.2, 0, 0.2) | 0.3 |
| 2 | wing right | 0 front antigrav | 0 | (0.62, −0.4, 0.06) | 0.23 |
| 3 | hull front | − | 0 | (0.62, 0, 0.14) | 0.32 |
| 4 | wing left | 0 front antigrav | 0 | (0.62, 0.4, 0.06) | 0.23 |

**Banshee mass points** (relative mass 1, mass 181.8182, density 4,
forward (1, 0, 0), up (0, 0, 1)); points 0 to 10 are metallic, 11 to
21 are not:

| # | Name | Channel | Node | Position | Friction | Radius |
|---|---|---|---|---|---|---|
| 0 | canopy back | − | 1 | (−0.480394, 0, 1.308344) | forward 0.25, 1 | 0.3 |
| 1 | canopy middle | − | 1 | (−0.050241, 0, 1.08) | forward 0.25, 1 | 0.35 |
| 2 | hull back | 0 body | 0 | (−0.788579, 0, 0.7) | forward 0.25, 1 | 0.4 |
| 3 | hull middle | − | 0 | (−0.182189, 0, 0.618048) | forward 0.25, 1 | 0.34 |
| 4 | wing right back | − | 0 | (−0.27, −0.52, 0.44) | up 2, 0.25 | 0.35 |
| 5 | wing right front | − | 0 | (0.18, −0.44, 0.44) | up 2, 0.25 | 0.35 |
| 6 | wing right tip | 1 wing | 0 | (0.025, −0.85, 0.27) | forward 0.25, 1 | 0.35 |
| 7 | wing left back | − | 0 | (−0.27, 0.52, 0.44) | up 2, 0.25 | 0.35 |
| 8 | wing left front | − | 0 | (0.18, 0.44, 0.44) | up 2, 0.25 | 0.35 |
| 9 | wing left tip | 1 wing | 0 | (0.025, 0.85, 0.27) | forward 0.25, 1 | 0.35 |
| 10 | front | − | 0 | (0.348551, 0, 0.662314) | forward 0.25, 1 | 0.44 |
| 11 | canopy back | − | 1 | (−0.480394, 0, 1.308344) | point | 0.25 |
| 12 | canopy middle | − | 1 | (−0.050241, 0, 1.108206) | point | 0.28 |
| 13 | hull back | − | 0 | (−0.788579, 0, 0.667235) | point | 0.4 |
| 14 | hull middle | − | 0 | (−0.182189, 0, 0.618048) | point | 0.34 |
| 15 | wing right back | − | 0 | (−0.27, −0.52, 0.44) | point | 0.34 |
| 16 | wing right front | − | 0 | (0.18, −0.44, 0.44) | point | 0.3 |
| 17 | wing right tip | − | 0 | (0.025, −0.9, 0.18) | point | 0.25 |
| 18 | wing left back | − | 0 | (−0.27, 0.52, 0.44) | point | 0.34 |
| 19 | wing left front | − | 0 | (0.18, 0.44, 0.44) | point | 0.3 |
| 20 | wing left tip | − | 0 | (0.025, 0.9, 0.18) | point | 0.25 |
| 21 | front | − | 0 | (0.348551, 0, 0.662314) | point | 0.44 |

The Banshee's eleven points appear twice: once with directional air
drag (forward points resist mostly sideways motion, the four inner
wing points resist mostly vertical motion, twice the plain drag) and
once plain.

### Other vehicle tags (summary)

| Tag | Type | Flags | Fwd, rev, acc, dec | Physics (radius, mass, points, channels) | Seats | Weapon |
|---|---|---|---|---|---|---|
| `wraith` | scout | 0x28C5 | 0.125, 0, 0.002, 0 | −1, 10000, 15, 2 antigrav | 1 | `wraith mortar` (0.5 per second) |
| `c gun turret` | turret | 0xC0 | 0 | −1, 2000, 4, 2 ground friction | 1 GT-gunner | `c gun turret gun` |
| `pelican` (and astral, crashing) | plane | 0x4 | 0.5, 0.1, 0.03, 0.01 | 4, 10000, 19, 0 | 12 | none |
| `c_dropship` | plane | 0x4 | 0.5, 0.1, 0.03, 0.01 | 4, 10000, 11, 0 | 14 | `cd gun turret gun` |
| `fighterbomber` | plane | 0x4 | 1, 0.1, 0.03, 0.01 | 4, 10000, 18, 0 | 1 | none |
| `lifepod` | fighter | 0x1804 | 0.5, 0.1, 0.05, 0.03 | 1, 4000, 4, 0 | 1 | none |
| `banshee_oldplayback` | fighter | 0x1984 | 0.5, 0.1, 0.05, 0.03 | 1, 4000, 11, 2 | 1 | `banshee gun` |
| `banshee_cinematic` | fighter | 0x1984 | 0.22, 0.03, 0.01, 0.02 | the Banshee's | 1 | none |
| `scorpion_cinematic` | tank | 0x8D1 | 0.18, 0.14, 0.002, 0.006 | the Scorpion's | 5 | `scorpion cannon` |

### Materials

From `globals.md`: of the five vehicle ground scales of the 32
materials only ice's friction scale (0.125) is non-zero; it applies to
the Warthog, Ghost and Banshee (mass <= 7500), not the Scorpion. Every
other material leaves the physics tag's values. Terrain therefore does
not change grip on any multiplayer map; slopes do (through n_z in the
drive factor φ and the gravity component).

## Per-tick order

Objects update parents before children (`objects.md`), so the vehicle
runs before its riders, with the riders' controls written at the start
of the tick by player control.

**A vehicle's tick:**

1. Unit layer: copy driver throttle, facing and driving flags, gunner
   aim, look, triggers; team from the driver (or gunner).
2. Unit layer: seat lean (rider acceleration overlays), powered seat
   power P0 and P1.
3. Unit layer: aiming vector toward the desired one; the weapon is
   updated with the triggers (fires).
4. If attached to a parent: zero velocities, wake, skip to 12.
5. Crouch and brake flags; steering angle σ.
6. Flip step (if flipping).
7. Speed, slide and turn variables.
8. Wake checks.
9. If the vehicle has physics and is awake: the type's preparation
   (wheel spin, channels, magic force and torque), the mass-point step
   (forces, collision accumulators, integration, penetration freeze,
   sleep and ground flags, then contacts with bipeds and other
   vehicles), the Ghost's hover update; sliding effects; suspension,
   and crash effects if no suspension sound played; airborne and
   on-ground tick counters; stop counter 15 if now asleep; air floor
   and ceiling. Else if the stop counter is > 0: slow stop and
   suspension.
10. Terminal velocity kill of riders (flag 6).
11. (Damage and pushes from other objects arrive during their updates:
    pushes act immediately on v and ω.)
12. Animation state update; `~blur` permutation.
13. Export function values (unit inputs, then vehicle inputs).

**A seated unit's tick** (after its vehicle):

1. Player control: look with the seat's rates and limits, throttle,
   buttons (written before objects update).
2. Its position and orientation follow the seat marker (attached).
3. If action is held and no animation is busy: start the exit.
4. If the vehicle's up.z < 0.2 and it is on the ground: leave now.
5. Entry or exit animation progress; at the exit animation's end, the
   detach of Exiting.

## Edge cases

- **Speed variable versus ground speed.** All driving effects read s
  (and the wheel spin uses it), not the measured speed. A Warthog
  against a wall at full throttle shows full-speed wheels.
- **Steering in reverse.** The jeep's target turn is −σ while s < 0,
  so the nose turns toward the camera in both directions; while s = 0
  exactly, the target is σ.
- **Scorpion standstill.** τ = 0 while s = 0, so a tank with no throttle
  never turns, whatever the camera does. Any throttle makes s non-zero
  and lets τ reach its full value within 35 ticks: turning in place
  needs a touch of throttle.
- **Ghost without driver.** Seat power 0 turns off antigravity, drive
  and yaw: the Ghost lies on its five spheres (sphere bottoms about
  0.045 WU into flat ground in the simulation, as the ground springs
  dictate) and slides like an object.
- **Ghost over steep ground or upside down.** Antigravity fades with
  the point's up.z between cos 75 degrees and 0.5 (β); drive and yaw
  need up.z > −0.2 and less than 0.5 WU of water.
- **Banshee at zero speed** has no lift (thrust step 1). It sinks or
  falls; within 0.75 WU of the ground the body antigravity holds it.
- **Ice.** Only the new model's material scaling applies (the older
  model instead divides the friction-type scales by 8 on ice).
- **Mass point order** matters only for flags and the earliest-hit
  choice in the freeze; forces are summed.
- **Older model** (physics radius > 0): no material scaling (ice ×
  0.125 on the friction shape instead), no collision accumulators, the
  moment of inertia about the torque axis computed each tick as Σ
  (|r_i⊥|^2 + 0.4 R_i^2) m_i · moment scale, no penetration freeze, and
  the position moved with the location updated along the line. Only
  campaign aircraft, the lifepod and `banshee_oldplayback` use it.
- **Vehicles on vehicles.** A mass point resting on another vehicle
  makes the surface volatile (no sleep) and depletes that vehicle's
  shield (none on the disc).
- **Water.** No multiplayer vehicle has a water channel; water acts only
  through buoyancy (density) and friction. Multiplayer water (Damnation,
  Blood Gulch's stream) is shallow.

## Quirks

- **Throttle is in WU per tick.** The jeep, tank and fighter seek the
  raw stick value as a speed: a stick at 30% asks the Warthog for 0.3
  WU/t, more than its 0.255 maximum, so anything above about a quarter
  stick is full speed; only very small deflections give slow driving.
  Keep (it is the feel of the controller); the keyboard gives full
  speed anyway. The Ghost scales by its maximum, so its stick is truly
  analogue.
- **Ghost moves equally fast in every direction**: reverse and strafe
  use the forward maximum (the tag's reverse limit 0 is not read).
- **Four-wheel counter-steering** on the Warthog with a 180 degrees per
  second steering rate makes it turn very tightly and slide at full
  lock (the linear friction has no grip limit to break, but its
  strength is small compared with the needed centripetal acceleration).
  Keep.
- **Sleep mid-bounce.** The sleep test can pass as a bouncing vehicle
  crosses its spring equilibrium moving slowly (acceleration near 0,
  |v| < 1 WU/s); it then drifts 15 ticks on the decaying velocity and
  freezes, slightly above or below equilibrium, not touching with all
  wheels. A Warthog spawned with its origin on flat ground falls asleep
  on tick 25 and freezes 0.062 WU above equilibrium, front-down by 2.4
  degrees. Keep the rule if spawn poses must match.
- **Pushes spin hard** (π times the horizontal push as angular
  velocity).
- **Banshee climbs when the camera is level**: the nose is held 15
  degrees above the camera and thrust is along the nose (Open
  questions).
- **Wheel spin from the commanded speed** (Exported values).
- **Seat yaw rates in degrees** while every other angle is radians.

## What a rigid-body implementation must preserve

The feel is set by these and can be measured in a rigid-body engine
(avian3d) and tuned to match:

- **Speed curves**: the speed variable's ramp (Warthog 0.00275 per
  tick up, 0.011 down; Scorpion 0.002 and 0.006; Banshee 0.008 and
  0.016), the lag of the body behind it (the Warthog reaches 0.039 WU/t
  at 1 s, 0.107 at 2 s, 0.232 at 5 s, 0.2337 top), the Ghost's direct
  0.005 per tick to 0.2244 in any direction, the stopping behaviour
  (Warthog from top speed stops in 86 ticks over 6.7 WU on release).
- **Turn response**: the Warthog's steering rate (6 degrees per tick),
  its ±30 degree four-wheel lock and the resulting slide; the
  Scorpion's tread-difference turning (0.077 rad/t at full lock while
  moving, none at rest); the Ghost's 0.2 degrees per tick^2 bang-bang
  yaw (90 degrees in about 40 ticks); the Banshee's 15.7-tick attitude
  time constant and its bank rule.
- **Suspension**: equilibrium compression (each contact's depths sum
  to the ground depth, 0.15 WU for the Warthog: 0.032 and 0.043 WU per
  tyre), stiffness M g / D_g per contact, damping M κ_g per contact
  (Warthog vertical: ω_n ≈ 0.31 rad/t, damping ratio ≈ 0.32; a 1 WU
  drop settles within 0.01 WU in 57 ticks), and the Ghost's hover
  height (sphere bottoms about 0.4 WU up) and its softer, less damped
  spring.
- **Linear friction**: grip proportional to slip velocity (no
  Coulomb limit), per-axis scales of each wheel, air drag rates 0.005,
  0.0025, 0.001 and 0.005 per tick: these fix top speeds and slides.
- **Interactions**: the splatter rule and its thresholds, the push
  formula with its spin, the flip's set angular velocity, the
  ejection angle, the entry reach (1 WU) and conditions.

May be replaced: the per-sphere contact query (any shape set that
gives the same contact heights and footprint), the penetration freeze
(any continuous collision that keeps the body out), the explicit
integrator (a smaller sub-step is acceptable if the per-tick response
is kept), the sleep machinery (but not its visible results where they
matter), the inertia tensor's construction (use the stored one), the
older model and the campaign aircraft types.

## Test cases

Simulation setup unless stated: flat infinite ground plane z = 0, one
material with no physics scaling (stone), gravity g, the disc's tags,
driver power 1 (driver seated long enough), sleep disabled for the
driving cases, vehicle level and at rest at its spring equilibrium,
desired facing = the vehicle's forward unless stated. "v" is the
ground speed v·f in WU/t; "tick n" is after n ticks of input.

### Formula cases (exact)

| Setup | Input | After | Expected |
|---|---|---|---|
| Warthog, s = 0 | t_i = 1 | 1 tick | s = 0.00275 |
| Warthog, s = 0 | t_i = 1 | 93 ticks | s = 0.255 (clamped on tick 93) |
| Warthog, s = 0.255 | t_i = 0 | 23 ticks | s = 0.002; tick 24: 0 |
| Warthog, s = 0.005 | t_i = −1 (brake flag) | 1 tick | s = 0 (seek to 0, stops on target) |
| Warthog, s = 0 | t_i = −1 | 1 tick | s = −0.00275 |
| speed step, s = −0.005, P = (0.255, 0.1, 0.00275, 0.011) | δ = +1 | once | s = (−0.005/0.011 + 1) · 0.00275 = 0.00150 |
| Warthog, τ = 0, moving forward | camera 90 degrees left | 1 tick, 5 ticks | τ = 0.10472, 0.5236 (30 degrees, held) |
| Warthog, τ = 0, s = −0.05 | camera 20 degrees left | 4 ticks | τ = −0.3491 (−20 degrees) |
| Scorpion, s = 0.05 | camera 90 degrees left | 1 tick, 35 ticks | τ = 0.004, 0.14; treads s ∓ τ |
| Scorpion, s = 0 | camera 90 degrees left | any | τ = 0 |
| one Warthog contact, depth 0.075 WU, v = 0 | | | normal force = M g/2 (half the weight) |
| Ghost antigrav point, sphere bottom 0.2 WU above ground, still, P0 = 1, up = z | | | force = M · 1.5 · (1 − 0.2/0.75)^2 g = 0.80667 M g |
| Ghost, σ = 1.0 rad, ω = 0, h_v = 1 | | 1 tick | yaw rate change 0.0034907 rad/t (capped); desired rate sqrt(0.0069813) = 0.08356 |
| Ghost, σ = 0.01 rad | | | desired rate 0.005 (σ/2) |
| Flip, upside down (up.z = −1), type 2 | | first tick | |ω| = 0.3 × 0.75 = 0.225 rad/t |
| Flip, on its side (up.z = 0) | | first tick | |ω| = 0.3 × 0.2 = 0.06 rad/t |
| Warthog at rest | rocket push Δv = (0.12, 0, 0) | instant | v = (0.12, 0, 0), ω = π (0, 0.12, 0) = (0, 0.37699, 0) |
| driver power, Ghost | driver enters | 30 ticks, 60 ticks | P0 = 0.5, 1.0 |
| driver power, Warthog | driver leaves from 1 | 3 ticks | P0 = 0 (0.1 s) |
| passenger seat, marker yaw 0 | desired yaw 100 degrees | | 90 degrees |
| passenger seat, marker yaw 0 | desired yaw 200 degrees | | −90 degrees (stored 270) |
| passenger seat, marker yaw 0 | desired yaw 45 degrees | | 45 degrees |
| splatter: Warthog speed 0.05, biped still, pill fix succeeds | | | damage: 0.05 <= 0.0667 but relative 0.0025 > 0.0011111 |
| splatter: Warthog speed 0.03, biped still, pill fix succeeds | | | no damage: 0.03 <= 0.0667 and relative 0.0009 <= 0.0011111 |
| splatter: biped left this Warthog 60 ticks ago, speed 0.2, fix succeeds | | | no damage (grace 90 ticks) |
| splatter: Warthog speed 0.01, pill fix fails | | | damage (a failed fix always hurts) |

(So a still biped is hurt by any vehicle faster than 1/30 WU/t that
touches it; the 2 WU/s figure matters only when the biped moves with
the vehicle.)

### Simulation cases (the model as specified, scratch simulation)

| Setup | Input | After | Expected |
|---|---|---|---|
| Warthog, no driver input, sleep off | none | settled | centre of mass z = 0.35360 (model origin 0.03182 above ground), forward.z = −0.0078 (nose down 0.45 degrees); tyre depths 0.03243 (rear), 0.04257 (front), sum over four 0.15 |
| Warthog dropped from 1 WU above equilibrium, sleep off | none | tick 23, 27, 57 | 0.045 above; lowest −0.131 (tick 27); within 0.01 WU and |v_z| < 0.001 from tick 57 on |
| the same, sleep on | none | | falls asleep on tick 42 (0.009 above, moving down 0.008), freezes 0.029 below equilibrium |
| Warthog spawned with origin 0.55 above the plane (origin on the ground) | none | | asleep on tick 25 moving up 0.018; freezes 0.062 above equilibrium, forward.z −0.043 |
| Warthog at equilibrium | t_i = 1 | ticks 1, 2, 30, 60, 93, 150, 300 | v = 0.00013, 0.00039, 0.03895, 0.10720, 0.18880, 0.23156, 0.23367 (top: 7.01 WU/s, 21.37 m/s); distance 0.463 WU at 1 s, 2.661 at 2 s, 20.12 at 5 s, 55.13 at 10 s |
| Warthog at top speed | t_i = 0 (sleep on) | | stops 86 ticks later, 6.71 WU on |
| Warthog at top speed | t_i = −1 held | | ground speed below 0.001 after 50 ticks, 5.96 WU on |
| Warthog | t_i = −1 | 600 ticks | v = −0.0916 |
| Warthog at top speed | camera fixed 10 degrees left of the heading (world) | ticks 10, 30, 120 | heading 5.9, 14.0 (overshoot), 10.08 degrees |
| Warthog at top speed | camera kept 90 degrees left of the vehicle (full lock), t_i = 1 | after 300 ticks, steady | |v| 0.0683, sideways 0.0620, yaw rate 0.1568 rad/t, path radius 0.44 WU: a spin |
| the same at t_i = 0.08 | | | |v| 0.051, yaw rate 0.049 rad/t, radius 1.03 WU |
| Ghost, driver power 1, hovering | none | settled | centre of mass z = 0.66474 (origin 0.55674 above ground); sphere bottoms: hull front 0.377, wings 0.387, back seat 0.417, hull mid 0.457 WU |
| Ghost, empty | none | settled | rests on its spheres, lowest 0.045 WU into the ground |
| Ghost hovering | t_i = 1 | ticks 1, 10, 30, 45, 60 | v = 0.00500, 0.04944, 0.14469, 0.21306, 0.22444 (top: 20.52 m/s) |
| Ghost hovering | t_j = 1 (strafe) | steady | sideways 0.22444, no yaw |
| Ghost hovering | t_i = −1 | steady | −0.22444 |
| Ghost hovering | (0.7071, 0.7071) | steady | 0.22444 |
| Ghost at top speed | release | | below 0.001 WU/t after 43 ticks |
| Ghost hovering still | camera 90 degrees left | ticks 5, 10, 20, 30, 40 | yaw 3.0, 10.9, 41.4, 76.0, 90.5 degrees; peak rate 3.9 degrees per tick; settled at 90 by tick 60 |
| Scorpion | none | settled | centre of mass z = 0.49530 (origin 0.07175 above ground) |
| Scorpion | t_i = 1 | ticks 30, 60, 120, 300 | v = 0.04048, 0.09871, 0.13827, 0.13844 (top: 12.66 m/s) |
| Scorpion | t_i = −1 | steady | −0.11866 |
| Scorpion at top speed | camera 90 degrees left, t_i = 1 | ticks 10, 30, 60, 300 | τ = 0.04, 0.12, 0.14; yaw rate 0.0105, 0.0506, 0.0767, 0.0774 rad/t (133 degrees per second); speed falls to 0.107; path radius 1.39 WU |
| Banshee, driver power 1, s = 0, near the ground | none | settled | hovers on its antigravity, lowest sphere 0.111 WU up, nose 13.8 degrees up |
| Banshee | t_i = 1, camera level | ticks 30, 60 | |v| 0.1237, 0.2193; steady 0.21931 (20.05 m/s) along the nose, which is 15 degrees up: climbing at 0.0567 WU/t |
| Banshee at speed | camera 45 degrees up | steady | nose 60 degrees, v_z 0.1899, horizontal 0.1097 |
| Banshee at speed | camera straight up | steady | v_z 0.2119 |
| Banshee at speed, high | t_i = 0 | ticks 30, 120, 300 | s = 0 after 14 ticks; v_z −0.051, −0.278, −0.498 (falling) |

## Confidence per section

| Section | Confidence |
|---|---|
| Vehicle tag layout and values | checked against the disc (every vehicle tag on every map; padding zero, references of the expected classes, types 0, 1, 3, 4, 5, 6 only); field names from the tag tool; meanings of 0x308, 0x314 and 0x364 for the fighter from the reference |
| Vehicle flags | bits 0 to 7 from the reference; 8 to 14 names only (AI) |
| Function sources | reference only; 24 to 31 uncertain |
| Physics tag layout and values | checked against the disc (all phys tags; masses, densities, centre of mass and inertia recomputed from the points and equal); meaning reference only |
| Seats, powered seats, weapons | layout and values checked against the disc; use of the flags reference only (bits 1, 5, 8 unread there) |
| Speed variables | reference only; arithmetic checked in simulation |
| Mass-point step | reference only; the sphere query's depth is the reading of the collision features (collision.md) |
| Jeep, tank, scout, fighter models | reference only; the jeep's wheel yaw sign inferred from what makes the vehicle drivable |
| Sleep and slow stop | reference only |
| Collisions, pushes | reference only; damage effect values from `projectiles.md` (disc) |
| Entering, exiting, flipping, ejection | reference only; the ejection threshold (0.2) is what the reference states the retail game does; its own older build used 0 |
| Input and camera | reference only; camera tracks checked against the disc |
| Guns | weapons checked against the disc (triggers, projectiles); routing reference only |
| Damage | vitality from `health_shields.md` (disc); rider fractions checked on the disc |
| Multiplayer placements | checked against the disc (all 13 maps) |
| Test cases | formula cases computed from the rules; simulation cases from a scratch simulation of the model as written here, on flat ground only |

## Open questions

1. **Simulation not validated against the game.** Every number of the
   simulation cases comes from the model as read. Measure in the game:
   time a Warthog from rest over 10 s on Blood Gulch's flat with full
   stick (expected 55 WU), its top speed (0.234 WU/t, 7.0 WU/s), the
   Ghost's (0.224), the Scorpion's (0.138).
2. **Throttle as a speed.** Does a half-pressed stick really give a
   full-speed Warthog? Drive with the stick at 10% and 50% and compare
   speeds (expected about 0.09 and 0.234 WU/t).
3. **Front wheels toward the turn.** The reference applies +τ to
   channel 0 and −τ to channel 1 through a rotation and a transpose
   whose convention was not traced; the spec takes the drivable
   reading (front toward the camera, rears opposite). Watch the rear
   wheels' collision behaviour (the animation is only visual).
4. **Banshee pitch.** Nose up 15 degrees with the camera level, so a
   level camera climbs. Measure: hold full throttle with the camera on
   the horizon and watch altitude. Also whether the Banshee falls when
   the throttle is released (the reading: free fall above 0.75 WU).
5. **Sleep mid-bounce** and the spawn pose. Compare the height of a
   freshly spawned Blood Gulch Warthog above the ground with the
   equilibrium (0.032 WU) or the frozen (0.094 WU origin height).
6. **Scorpion turning at rest.** Expected: no turning without throttle.
7. **Action held.** The action is level-triggered: holding it through
   the end of the entry animation should start the exit. Check whether
   the Xbox game does that (or suppresses it in the input layer).
8. **Ejection angle.** 0.2 (on its side) per the reference's note on
   the retail behaviour; an older build used 0 (upside down). Tip a
   Warthog on its side with riders and see whether they are thrown.
9. **Campaign vehicle vitality.** The campaign Ghost, Scorpion and
   Banshee collision models were not read; they are not needed for
   multiplayer.
10. **Function sources 15, 18 and 24 to 31.** The reference's reading
    disagrees with the source names in places; only 1, 12, 20, 21,
    35, 36 are on the disc, so only those matter.
11. **The observer.** How the camera command's depth and offset become
    a camera position and how it is smoothed is the observer's (not
    this spec's); the track offsets are given.
12. **Splatter and friendly fire.** The damage goes through the normal
    rules with the driver as owner; whether a teammate is splattered
    depends on the game type's friendly fire (`health_shields.md`).
