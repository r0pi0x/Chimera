# Halo: Combat Evolved: vehicles

Source basis: the game's own map files from the original Xbox disc
(every vehicle tag, physics tag, camera track, vehicle animation block,
scenario vehicle placement and vehicle weapon on all 24 maps; the three
multiplayer vehicles' models and graphs and the Spartan's seat
animations on bloodgulch), read with throwaway programs; and a
decompilation of an Xbox build of the engine (the vehicle update, the
mass-point physics, the unit layer's seats, entry, exit and aiming,
object creation from the scenario, the player's action and look
handling, the following and first-person cameras and the observer),
used for behaviour only. The numbers in the test cases come from a
scratch simulation of the model as read, run on the disc's values.
Status: draft

Sections: Summary; Units and conventions; Constants; The vehicle tag;
The physics tag; Seats, powered seats and weapons; Behaviour (the
control variables, the mass-point step and its order, the four driving
models, other types, sleeping, collisions, pushes, entering and
exiting, input and camera, seated riders, turrets and guns, damage,
multiplayer and placement, exported values and animation); The disc's values;
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

with p the object position, f forward, u up and l = u × f (left). The
model is drawn with its origin at p − R c (plus the object's origin
offset, 0 on the four), and every marker of the model is placed from
that origin. **A scenario placement's position is taken as p, the
centre of mass, not the model origin**: a vehicle placed at z = 0 has
its centre of mass at 0 before the raise below. A vehicle is created
raised by half its bounding radius above its placement (Warthog 0.55
WU, Ghost 0.4, Scorpion 1.1, Banshee 0.7) and falls onto its springs.

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
| 0 | invisible | the rider aims through the vehicle and is hidden once its entry ends (does not choose the view) |
| 1 | locked | not read by the reference |
| 2 | driver | the first occupant becomes the vehicle's driver: its throttle and facing drive the vehicle (does not choose the view; bit 4 does) |
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
2. **Ground query.** The deepest surface within a sphere of radius R_i
   around x_i (the sphere query of `collision.md`) gives a plane with
   normal n, a depth δ_i = R_i − (distance from x_i to the surface),
   and the surface's material type. The surfaces tested are the
   structure BSP and the collision models of **scenery and machines
   only**: other vehicles, bipeds, weapons and items are never ground
   to a vehicle (vehicles meet vehicles only through the pair rule of
   Collisions). A face counts only while x_i is on its front side and
   less than R_i from it: a sphere whose centre has passed through a
   face gets no force from it. No surface: δ_i is measured against a
   horizontal plane at z = −256 (so it is negative). If the surface
   belongs to an object, that object's shield is depleted, and the
   point is "on a volatile surface" when the object is not scenery (a
   machine: lifts, doors) or the surface is breakable.
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
against front-facing faces of the structure BSP, scenery and machines
(the ground query's set). If none hit, accept the pose and stop. Otherwise
take the earliest hit (fraction t_h, normal n_h, sweep vector Δ): ε =
0.0078125/|Δ · n_h| (0.03125 if Δ · n_h = 0), t' = max(t_h − ε, 0);

- if n_h · v' < 0: the velocity's component along n_h is **scaled by
  t'** (v' = v' + n_h (t' − 1)(n_h · v'); the tangential part is kept;
  nothing is reflected or zeroed unless t' = 0), stored as the
  vehicle's velocity, and p' = p + v' (from the start position);
- if n_h · v' >= 0 the linear velocity and p' are left as they are;
- in both cases ω' is scaled by t' (all of it), stored, and the
  orientation is recomputed from the start orientation turned by the
  new ω'.

Then the next pass sweeps again. **If the fourth pass still hits, the
vehicle is not moved at all this tick**: position and orientation stay
those of the tick's start, while the reduced v' and ω' stay stored.
The points that hit in the last pass are remembered ("stuck"). The
sphere radius plays no part here: only the centres are swept; the
springs keep the spheres out of the ground, so on open ground (a fall,
a landing, the 1 WU drop of the Test cases) the freeze never acts.

**Ground and sleep flags.** The vehicle is on the ground if any point
touches, in water if any point is wet. It **falls asleep** ("at rest")
when every point is at rest, at least 3 touch, none is on a volatile
surface, |v'|^2 <= 0.0011111111, |ω'|^2 <= 0.0027415568, |a|^2 <=
3.0864197e-7 and |α|^2 <= 3.0461742e-6.

**Then** the vehicle's contacts with units are processed (Collisions).

**Order of one step** (everything inside one tick's physics, in this
order; nothing is sub-stepped):

1. The type's preparation sets the channels (wheel speed and yaw,
   tread speeds, antigravity fraction) and the magic force and torque.
2. For each mass point in tag order, from the pose and velocities at
   the start of the tick: position, velocity, ground query, normal
   spring and damper, ground friction with drive, water, air, thrust,
   antigravity; its force and torque. Nothing moves yet: every point
   sees the same start state, and the spring and the friction are
   computed together from it.
3. Sum: gravity (−M g_v z) + all point forces + last tick's collision
   accumulators (then cleared) + the magic force; torques likewise.
4. Integrate once: a = F/M, v' = v + a, p' = p + v' (the new velocity
   moves the body: semi-implicit Euler); α = R I⁻¹ Rᵀ T, ω' = ω + α,
   orientation turned by ω'. Store v' and ω'.
5. Penetration freeze (above) on the new pose; it can only shorten the
   move and scale the velocities.
6. Point flags give the vehicle's at-rest, on-ground and in-water
   flags (sleep test with the a and α of step 4 and the v', ω' left by
   step 5).
7. Contacts with bipeds and other vehicles (Collisions): their pushes
   on this vehicle go to the accumulators for the next tick.

### The jeep (Warthog)

Before the step:

- Wheel spin: W = (W + s) mod wheel circumference, kept in [0, C).
- The two channels (0 'front', 1 'back' on the disc, flag ground
  friction) both get ground speed s. Channel 0 is yawed by +τ and
  channel 1 by −τ about the model's z axis: a front wheel's forward
  becomes (cos τ, sin τ, 0) in the model frame and a rear wheel's (cos
  τ, −sin τ, 0). **The front wheels point toward the turn and the rear
  wheels the opposite way** (four-wheel steering, equal angles). With
  τ > 0 the vehicle turns left going forward; reversing, τ follows −σ,
  so the nose still swings toward the camera. (The channel rotation is
  built as a quaternion about z of angle ±τ whose matrix is then
  transposed; with the engine's quaternion convention the two
  transpositions cancel, which settles the sign.)

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
driver's seat power rises over 2 s (power-up time 2 s) and lifts it
with it: centre of mass 0.449 WU up after 1 s, within 0.003 WU of the
hover height (0.665) after 2 s.

Counters the scout reads: **on-ground ticks** n_g counts consecutive
ticks in which some mass point's sphere touches the ground (0 while
hovering clear of it, capped at 255); **airborne ticks** count ticks in
which no point touches and no antigravity trace hits (capped at 255).
The antigravity traces, like the ground query, see only the structure,
scenery and machines: a Ghost does not hover over another vehicle.
**The Ghost's turn variable τ is always 0** (its turn limits and turn
rate are 0), so nothing driven by τ moves on the Ghost; its yaw comes
only from the yaw torque above. Water: drive, yaw and the levelling
torques stop while the water at the Ghost's centre is 0.5 WU deep or
more (antigravity stays: its traces do not see water surfaces).

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
  Warthog: the four tyres 11 to 14, E −0.31445, C −0.13452; Scorpion:
  eight tread points, below), a straight trace is cast against the
  structure, scenery and machines. Its start is the point P = p + R
  q_i (the mass point's model position placed **with the centre of
  mass as origin**, a quirk: the trace sits c_x = 0.107 WU ahead of the
  Warthog's tyre) moved along the point's up u_i by (C − c_z) − (E −
  C); its vector is u_i · 2(E − C) (E < C, so it points down, length
  2|E − C| = 0.35986 WU for the Warthog). With the hit fraction t (no
  hit: t = 1) the target compression is k = clamp(2(1 − t), 0, 1). On
  flat ground and a level vehicle this is the same as k = clamp((g −
  (h_i + E))/(C − E), 0, 1), where h_i is the height of the mass
  point's centre and g the ground's: k = 0 with the ground E below the
  centre (0.314 WU for a Warthog tyre), 1 with the ground C below it
  (0.135 WU). The stored value becomes (k + previous)/2, quantised to
  a byte (0..255), and the overlay plays at frame value × (N − 1). If
  the largest k − previous over the elements exceeds 0.3, the
  suspension sound plays at scale clamp((that − 0.3)/0.6, 0, 1). A
  Warthog at rest on flat ground: tyre centres 0.2227 WU above the
  ground, k = 0.51 on every tyre.
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
vehicles inside its bounding sphere (centre the bounding sphere centre,
radius the object's bounding radius: Warthog 1.1, Ghost 0.8, Scorpion
2.2 WU). For another vehicle B (physics
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
marker's world frame composed with the root node's frame at frame 0 of
the player's "enter" animation for that seat (unit seat slot 7 of the
first graph seat with that label: where the walk-in starts): entry =
marker position + marker axes × (root translation at frame 0).
Distance = min(|entry − c_player|, |seat marker − c_player|) with
c_player the player's bounding sphere centre. Candidates: distance < 1
WU; seat flag 9 requires a driver present; the seat must be enterable
(below). Ranking: an available seat beats an occupied one; else the
nearer, where once a driver seat is the best so far a non-driver
seat's distance counts 1.5 times. The "<marker> enter" markers of the
models (`driver enter` ...) are AI hints, not read for players.

Seat markers and entry points on the disc (model frame of the vehicle:
x forward, y left, z up from the **model origin**, which is p − R c;
bind pose; the multiplayer Spartan's graph `characters\cyborg\cyborg`):

| Seat | Seat marker (node) | Root at enter frame 0 (marker frame) | Entry point | Side |
|---|---|---|---|---|
| Warthog W-driver | `driver` (−0.0238, 0.1658, 0.3908), hull | (0.0170, 0.3820, 0.0013) | (−0.0068, 0.5478, 0.3921) | left |
| Warthog W-passenger | `passenger` (−0.1162, −0.1586, 0.5780), hull | (0.0915, −0.4056, −0.1057) | (−0.0247, −0.5642, 0.4723) | right |
| Warthog W-gunner | `gunner` (−0.6228, 0, 0.7556), gun mount (turns with the turret's yaw) | (−0.5561, 0, −0.3898) | (−1.1789, 0, 0.3658) with the turret forward | behind |
| Ghost G-driver | `driver` (−0.0750, 0, 0.1144), seat node | (−0.0080, −0.3165, −0.0034) | (−0.0830, −0.3165, 0.1110) | right |
| Scorpion scorpion-driver | `driver` (−0.0077, 0.1893, 0.5160), hull | (−0.0772, 0.8598, −0.1565) | (−0.0849, 1.0491, 0.3595) | left |
| Scorpion scorpionLF | `passenger lf` (0.3227, 0.8483, 0.5596), facing +y | (0.3906, 0.1465, −0.1638) | (0.1762, 1.2389, 0.3958) | left front |
| Scorpion scorpionRF | `passenger rf` (0.3403, −0.8483, 0.5596), facing −y | the same | (0.4868, −1.2389, 0.3958) | right front |
| Scorpion scorpionLB | `passenger lb` (−0.5307, 0.9483, 0.5596), facing +y | the same | (−0.6772, 1.3389, 0.3958) | left back |
| Scorpion scorpionRB | `passenger rb` (−0.5277, −0.9483, 0.5596), facing −y | the same | (−0.3812, −1.3389, 0.3958) | right back |

All seat markers but the Scorpion riders' face +x. The four Scorpion
rider seats use the `scorpionLF` animations (each seat label has its
own graph seat whose slots point at them). Because the distance is the
smaller of the two, a player within 1 WU of the seat marker itself (a
player standing on the vehicle) is offered the seat as well.

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
  takes both); the unit readies the weapon the seat allows (Weapons on
  entering and leaving, below) and picks the seat label with that
  weapon's class (falling back to the seat's last class); it unzooms;
- the seat's enter animation starts (6-tick blend) from the unit's
  current offset relative to the seat marker, which the animation
  interpolates away: the visible walk-in. While entering, the unit's
  controls are not passed to the vehicle (players' are: the player's
  throttle drives as soon as it is seated; AI's wait).
- **Control starts on the tick of entry**, not at the animation's end:
  a player driver's throttle, look and brake reach the vehicle from the
  next vehicle update, a player gunner's aim and triggers likewise; the
  driver's seat power starts rising at once. What the animation's end
  changes: the unit becomes visible or not by the seat's invisible flag
  (all three multiplayer vehicles: visible), the driver's vehicle plays
  its closing animation (the Scorpion's hatch, 16 ticks; the Warthog
  and Ghost have none), and the camera leaves the entry perspective
  (third-person-on-enter seats).
- the vehicle's driver and gunner are recomputed at once: the first
  unit, in the vehicle's child order, in a driver seat drives (and also
  guns if that seat has the gunner flag and nobody guns yet); a gunner
  seat's occupant takes the gun when nobody guns or the driver does.

**Exiting.** A seated unit with action held and no busy animation
starts its seat's exit animation (slot 8; a seat without one cannot be
left by the button); it is made visible; if it is the driver the
vehicle plays its opening animation (the Scorpion's hatch). The unit
stays seated, and keeps driving or gunning, until the animation's last
frame. When it ends:

1. The exit velocity is read first: the root node's translation at the
   last frame minus that at the frame before, turned from the unit's
   frame (the seat marker's) into the world.
2. The unit is detached: its world position becomes the seat marker's
   plus the root node's last-frame translation (in the marker's axes)
   minus the root node's default z translation (the Spartan's 0.3944
   WU) in world z; its facing is the root node's last-frame direction
   relative to its default orientation, then flattened (forward z set
   to 0 and normalised, up = world z); its velocity becomes the
   vehicle's velocity plus the exit velocity of step 1; it is
   airborne, on the stand seat, and readies its weapon again.
3. The vehicle loses it as driver or gunner (recomputed from the
   remaining riders); its last vehicle and exit time are recorded
   (splatter grace).
4. **Blocked exit spot.** The biped is then fitted: candidates are the
   spot itself and 26 offsets, tried in this order, in the biped's new
   frame (forward, left, up) and scaled by 2 × its pill radius (0.4 WU
   for the Spartan): (0, 0, 0), (1, 0, 0), (0, 0, 1), (h, 0, h), (r, r,
   r), (r, −r, r), (h, h, 0), (h, −h, 0), (0, h, h), (0, −h, h), (−1, 0,
   0), (0, 1, 0), (0, −1, 0), (−h, −h, 0), (−h, h, 0), (−h, 0, h), (−r,
   −r, r), (−r, r, r), then the downward ones (0, 0, −1), (h, 0, −h),
   (−h, 0, −h), (0, h, −h), (0, −h, −h), (r, r, −r), (r, −r, −r), (−r,
   −r, −r), (−r, r, −r), with h = 0.70710677 and r = 0.57735026. The
   first candidate wins that is inside the level, where the pill can
   be fixed out of the level within twice its radius, where the pill
   then touches nothing (structure, scenery, machines, other bipeds
   and vehicles), whose pill does not touch this vehicle's collision
   model, and from which a line to the vehicle's bounding sphere
   centre is clear both ways (or hits only the other one). If none
   fits, a second pass tries the first 18 offsets (no downward ones)
   in world axes around the vehicle's bounding sphere centre, scaled
   by the vehicle's bounding radius (Warthog 1.1, Ghost 0.8, Scorpion
   2.2 WU). If that fails too, the biped stays where step 2 put it.

The exit animations carry no frame info: their whole displacement is
in the root node's track. Timing and the exit spots (marker frame, the
Spartan; "exit velocity" in WU per tick in the marker frame, to which
the vehicle's own velocity is added):

| Seat | Enter (ticks) | Exit (ticks) | Root at the last exit frame | Exit velocity | Faces (vehicle frame) |
|---|---|---|---|---|---|
| Warthog W-driver | 50 | 26 | (0.0566, 0.3287, 0.1820) | (0.0014, 0.0285, −0.0096) | left (0.25, 0.97) |
| Warthog W-passenger | 28 | 30 | (0.0507, −0.4525, −0.2314) | (0.0007, −0.0130, −0.0326) | right (0.16, −0.99) |
| Warthog W-gunner | 46 | 24 | (−0.4961, 0.1248, −0.3041) | (−0.0170, 0, −0.0557) | back (−0.99, 0.11) |
| Ghost G-driver | 35 | 24 | (0.0728, −0.3132, −0.0693) | (0.0013, −0.0273, 0.0132) | right (0.27, −0.96) |
| Scorpion driver | 65 | 55 | (0.0835, 1.1440, −0.0070) | (0, 0.0380, −0.0669) | left (0.14, 0.96) |
| Scorpion riders (LF shown; the others mirror through their markers) | 30 | 22 | (0.4409, 0.0118, −0.1549) | (0.0119, −0.0011, −0.0342) | outward (0.93, 0.35 in the marker frame) |

So a Warthog driver lands at (0.0328, 0.4945, 0.1784) from the model
origin before fitting (0.49 WU left of the centre line). The enter
times matter only for the camera and the rider's pose; the exit times
are how long the button takes to free a player.

**Weapons on entering and leaving.** Entering readies the unit's most
recently used weapon that the seat's animation set can hold (the
unit's seat label with the weapon's label: Seated riders, below); if
none can be held the weapon is put away at once (hidden, kept in the
inventory) and the unit is unarmed. Leaving readies its weapon again.

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

Upside down (up.z = −1) the vehicle starts turning at 0.225 rad/t; the
rate stays there while up.z <= −0.375, falls with −2 up.z, and is held
at the 0.06 rad/t floor once up.z > −0.1. Counting the set rate alone
(gravity and ground contact add to it), a Warthog on its roof reaches
up.z > 0.9 after 28 ticks, one on its side (up.z = 0, 0.06 rad/t
throughout) after 19 ticks, one at 135 degrees after 24. The flip
stops after 30 ticks or once up.z > 0.9, whichever comes first, and the
physics then lands the vehicle on its wheels. The flip needs no rider
and no driver; the player who flips stands outside it (the prompt
appears only with no driver), and riders are thrown out by Ejection
while the vehicle is on its side or roof.

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

**Which view.** A seated player sees third person when the seat has the
third-person-camera flag (bit 4), and also while entering or leaving a
seat with the third-person-on-enter flag (bit 6); otherwise first
person. The driver and invisible flags do not choose the view. On the
disc: Warthog driver and gunner, Ghost driver and Scorpion driver are
third person; the Warthog passenger is first person when seated and
third person during its entry and exit animations; the Scorpion riders
are first person throughout.

**Third-person camera.** Every tick, from the player's look angles
(yaw ψ, pitch θ, world-relative for a third-person seat; θ clamped to
±π/2) and the seat (always the seat's own camera block while seated):

1. **Focus F.** The seat's camera marker on the vehicle, if the seat
   names one; a named marker the model does not have gives the
   vehicle's node 0 (its root node) frame; an empty name gives the
   vehicle's object position, **its centre of mass**. Never the seat
   marker or the rider's head. Disc: Warthog driver: centre of mass
   (model (0.1069, 0, 0.3218)); Warthog gunner: `camera gunner` (model
   (−0.2235, 0, 0.8131) in the bind pose, on the gun mount, so it
   swings with the turret's yaw on a 0.2765 WU radius about the mount
   node); Warthog passenger (entry and exit only): `camera rider`
   (0.0715, −0.1586, 0.8056); Ghost driver: names `driver camera`,
   which `ghost_mp`'s model lacks, so the hull node (0.3928, 0,
   0.1711); Scorpion driver: centre of mass (−0.0338, 0, 0.4236).
2. **Track offset o(θ).** The seat's first camera track (index 0; the
   second, "tight" track of the Warthog and Scorpion drivers is never
   used; a seat with none uses the globals' default track). With n
   control points (9 on the disc, evenly spaced from θ = −90 to +90
   degrees) and h = 1/(n − 1): t = (θ + π/2)/π; j = floor((n − 1) t);
   k = j − 1, raised to 0 and lowered to n − 4 if needed; then o is
   the **cubic polynomial through the four control-point positions k
   .. k + 3** (placed at t = k h .. (k + 3) h) evaluated at t:
   Lagrange (Newton forward-difference) interpolation, so the curve
   passes through every control point: with s = t/h − k and the
   forward differences Δ, Δ², Δ³ of those four points, o = P_k + s Δ +
   s (s − 1)/2 Δ² + s (s − 1)(s − 2)/6 Δ³. (This is not a B-spline or
   Catmull-Rom curve.) A track with fewer than 4 points is read
   piecewise linearly (none on the disc).
3. **Frame.** o is in the **horizontal heading frame**: x along the
   look's horizontal direction H = (cos ψ, sin ψ, 0), y to its left,
   z = world up. The vehicle's own pitch and roll do not enter.
4. **Command.** depth D = |o|; offset = (D cos θ + o_x, −o_y, D sin θ
   + o_z) in (H, right, up); the camera's forward is the look direction
   L = (cos θ cos ψ, cos θ sin ψ, sin θ). The observer computes the
   probe point Q = F + (D cos θ + o_x) H + o_y Hleft + (D sin θ + o_z) z
   and places the camera at Q − D' L with D' = max(D, 0.6). For D >=
   0.6 (every point of the disc's tracks: |o| >= 1.02) the D terms
   cancel: **the camera sits at C = F + o_x H + o_y Hleft + o_z z**,
   the track point itself, looking along L.
5. **Pull-in.** All rays below start at Q, test front-facing faces of
   the structure BSP, water surfaces (unless Q is under water) and
   scenery (not vehicles, bipeds or machines), and report the fraction
   of their own length at the first hit. E = Q − (D + 0.02) L is the
   desired end; step = 0.174 D; U is the camera's up and W = U × L its
   left.
   1. **Centre ray** Q to E: fraction t1 (t1 = 1 when it misses).
   2. **Four probes**, in this order, to E − step U, E − step W,
      E + step U, E + step W. The probe kept is the one with the
      smallest fraction that is strictly below the best so far (the
      best starts at t1, so a probe must hit nearer than the centre; on
      a tie the earlier probe stays). Its fraction is t2, its direction
      P (one of ±U, ±W, sign included). **A probe can be kept when the
      centre ray missed** (t1 = 1): a wall just beside the line still
      pulls the camera in.
   3. No probe kept: the distance becomes D × t1 (D unchanged when the
      centre ray missed too).
   4. A probe kept: bisect the displacement m (the ray to E + m step P)
      between lo = 0 and hi = 1 for exactly 10 steps, starting with
      last-hit = t2 and last-miss = t1. Each step tests m = (lo + hi)/2:
      if that ray hits **and** its fraction is within 0.1 of last-hit
      (the same near surface), hi = m and last-hit = that fraction;
      otherwise lo = m and last-miss = its fraction if it hit, 1 if it
      missed. After the 10 steps the edge s is **lo when last-miss <
      last-hit, else hi** (not the midpoint), and the distance becomes
      D × ((1 − s) t2 + s t1). (t2 here is the kept probe's fraction,
      not the last bisection hit.)
   The camera is at Q − L × that distance; with no pull-in this is C
   above. Finally a camera within 0.05 WU of a water surface is moved
   to 0.05 WU from it on its own side.
6. **Field of view** 70 degrees, whatever the zoom. **Smoothing:** none
   in steady play. The camera is placed exactly where steps 1 to 5 put
   it, every frame, with no lag, spring or low-pass filter, whether the
   vehicle is still, turning or crashing. Easing happens only during a
   timed transition, below (The observer's transitions).

**The observer's transitions.** The camera is held as five parameters,
each with its own transition timer in seconds: the focus position F
(3 values), the focus offset (the track offset, 3), the distance (1),
the field of view (1) and the orientation (forward and up; its change is
a rotation vector, 3 values). Each frame the camera mode (third-person
seat camera, first-person camera) issues a target for all five and a
timer, and the observer:

1. **Merges the timers.** For each parameter: if the target names its
   own timer for that parameter and it is shorter than the one still
   running, the running one continues (capped at 2 s); if it names
   none, the target's general timer is used, unless that is shorter
   than the running one, in which case the running one continues
   (capped at 2 s). A forced timer is taken as given. So a transition,
   once started, runs to its end even though later frames ask for 0.
2. For each parameter whose remaining time T is greater than the frame
   time dt: with D = target minus current value, it takes the unique
   quintic polynomial in time that starts at the current value with the
   parameter's current velocity and acceleration and reaches the target
   after T with zero acceleration and zero velocity (the focus position:
   see below), and moves the parameter along it by dt (the orientation
   by the resulting rotation vector, about its own axis); the velocity
   and acceleration at that point are kept for the next frame. In
   terms of the time r still to go, the amount covered is
   y(r) = A r^5 + B r^4 + C r^3 + D, with
   A = a / (2 T^3) - 3 w / T^4 - 6 D / T^5,
   B = 7 w / T^3 + 15 D / T^4 - a / T^2,
   C = a / (2 T) - 4 w / T^2 - 10 D / T^3,
   a the current acceleration and w = minus the current velocity (the
   derivative taken with respect to r); y(T) = 0, y(0) = D, and the
   parameter moves by y(T - dt). With w = a = 0 this is the
   "smootherstep" ease: the fraction covered after time s of T is
   10 q^3 - 15 q^4 + 6 q^5, q = s / T. The fit is redone every frame
   against that frame's target, so a moving target is followed smoothly
   and reached exactly when the timer ends. For the focus position the
   curve also carries an end-velocity term built from the command's
   focus velocity: u is the followed unit's velocity (the player's own
   unit, the rider while seated) in WU per tick and U = 30 u per
   second: A -= 3 U / T^4, B += 8 U / T^3, C -= 6 U / T^2 and U r is
   added to y. These terms leave y(T), the start velocity and the start
   acceleration unchanged and make dy/dr = U at r = 0. Since r runs
   backwards in time, the focus's velocity in time at the end of the
   transition is **−U**: the curve is fitted to arrive moving against
   the unit's motion (a quirk of the sign: the term is positive where
   a "match the target's motion" end condition would need it negative).
   The parameter's stored velocity is likewise dy/dr, the negative of
   its velocity in time; when a parameter snaps its stored velocity is
   −(target − old)/dt (0 for a forced timer). Because the fit is redone
   every frame and the parameter snaps to the target when the timer
   runs out, the effect is confined to the 1 s transitions; a port
   that ends the focus at rest differs only during them, and only
   while the unit moves. (Whether a seated rider's own velocity is the
   vehicle's is not traced: Open questions.)
3. **Acceleration cap.** If any value's acceleration at r = T - dt
   exceeds 1500 WU/s^2 (focus position, focus offset) or 100 000 (the
   distance, field of view and orientation), that parameter's timer
   and every other parameter's timer with exactly the same remaining
   time are set to 0: they snap this frame.
4. Every parameter whose remaining time is at most dt **snaps** to its
   target.
5. Every timer drops by dt (not below 0).

**What starts a transition** (in multiplayer):

| Event | Parameters | Length |
|---|---|---|
| the unit or seat the camera follows changes while the third-person camera is in use (entering a third-person seat from another seat, switching seats, leaving into another third-person view) | all five (the general timer) | 1 s |
| the camera mode changes between first person and third person (entering a vehicle with a third-person seat, leaving one, a seat with the third-person-on-enter flag starting or ending its animation) | all five; the general timer is held at least at the remaining "change pause", which starts at 1 s and falls by the frame time | 1 s; when changing **into** first person, once less than 0.2 s remains, the focus position and the distance are forced to snap |
| the rider's crouch-or-jump state changes in a third-person seat (crouch held or jump held: for the Warthog driver, brake or not) | the focus offset only | at least 0.5 s |

Nothing else starts one: driving, turning the look, the turret and the
vehicle's bumps and flips are followed exactly. (The first-person camera
always issues timer 0.)

The common mistakes are taking F at the seat marker or the rider's
eye, turning o by the vehicle's orientation instead of the heading, or
placing the camera D behind F along L in addition to the offset (which
puts it twice as far and too low).

Worked example, Warthog driver, vehicle level, look yaw along the
vehicle's forward (track `warthog_driver_loose`), positions from the
model origin:

| Look pitch | Track points used, s | o (x, z) | D | Camera C (x, y, z) | Above flat ground at rest |
|---|---|---|---|---|---|
| 0 (level) | 3..6, s = 1 (point 4 exactly) | (−2.7524, 0.6827) | 2.8358 | (−2.6455, 0, 1.0045) | 1.037 |
| −30 degrees (down) | 1..4, s = 1.667 | (−2.4078, 1.8922) | 3.0623 | (−2.3009, 0, 2.2140) | 2.247 |
| +30 degrees (up) | 4..7, s = 1.333 | (−2.2864, 0.0048) | 2.2864 | (−2.1795, 0, 0.3266) | 0.359 |
| −15 degrees | 2..5, s = 1.333 | (−2.6878, 1.2900) | 2.9813 | (−2.5809, 0, 1.6118) | 1.645 |

(The model origin of a Warthog at rest is 0.0327 WU above flat ground.)
The camera is 2.6 to 2.8 WU (8 m) behind the centre of mass and about
1 WU (3 m) above the ground when level, higher than the gun's muzzle
(z 0.93), and climbs to 2.2 WU when looking 30 degrees down. Scorpion driver, level: o = (−4.6130, 2.1571), C
= (−4.6468, 0, 2.5807); 30 degrees down: o = (−4.0234, 3.5954). Ghost
driver, level: C = (−2.3596, 0, 0.8538). Warthog gunner, level, turret
forward: C = (−2.9759, 0, 1.4958).

**First-person seats.** The eye is the seat's camera marker on the
vehicle (an empty name: the vehicle's centre of mass): Warthog
passenger `camera rider` (0.0715, −0.1586, 0.8056); Scorpion riders
`passenger lf camera` (0.3227, 0.8483, 0.7841), `rf` (0.3403, −0.8483,
0.7841), `lb` (−0.5307, 0.9483, 0.7841), `rb` (−0.5277, −0.9483,
0.7841) (on the tread nodes, so they bob with the tread suspension).
In a seat without the third-person flag the look angles are not
world-relative: they are taken in a frame whose up is the vehicle's
up and whose forward is normalise(u_vehicle × (0, −1, 0)) (world x
tilted with the vehicle), so the view pitches and rolls with the
vehicle while yaw stays world-referenced; the camera's up is built in
the vehicle's frame. A seat with the first-person-slaved-to-gun flag
(bit 7) would look from the vehicle's `primary trigger` marker along
its axes; the only such seat on the disc (Warthog gunner) is third
person, so this never shows. The field of view is the player's own
(70 degrees unzoomed); zoom is cancelled on entering.

The track points at pitch 0 (control point 4, x, y, z in WU):

| Track | Used by | Pitch 0 point | Pitch −90 | Pitch +90 |
|---|---|---|---|---|
| `camera\warthog_driver_loose` | Warthog all seats, Ghost driver | (−2.7524, 0, 0.6827) | (0.5974, 0, 3.5431) | (−1.0209, 0, −0.1809) |
| `camera\scorpion_driver_loose` | Scorpion driver | (−4.6130, 0, 2.1571) | (0.3328, 0, 4.6906) | (−1.8471, 0, −0.1524) |
| `camera\banshee_driver_loose` | Banshee driver | (−2.9329, 0, 1.1784) | (1.6825, 0, 4.2036) | (−0.7176, 0, −2.4833) |
| `globals\default unit camera track` | seats without a track (Scorpion riders; never seen, those seats are first person) | (−3.6826, 0, 0.5752) | (0.2387, −0.0058, 3.8862) | (−0.0381, −0.0038, −1.9795) |

Full lists: Warthog loose x, z per point (pitch −90, −67.5, ..., +90):
(0.5974, 3.5431), (−0.8369, 3.2299), (−1.9179, 2.4325), (−2.5767,
1.6104), (−2.7524, 0.6827), (−2.4676, 0.1141), (−1.8786, −0.1244),
(−1.3638, −0.1717), (−1.0209, −0.1809). Scorpion: (0.3328, 4.6906),
(−1.9374, 4.5644), (−3.3776, 4.1085), (−4.2563, 3.2836), (−4.6130,
2.1571), (−4.3739, 1.0201), (−3.7841, 0.2852), (−2.7714, −0.0658),
(−1.8471, −0.1524). Banshee: (1.6825, 4.2036), (−0.3336, 4.0599),
(−1.9989, 3.2481), (−2.6678, 2.3735), (−2.9329, 1.1784), (−2.7761,
0.0856), (−2.3899, −0.8667), (−1.7120, −1.7484), (−0.7176, −2.4833).
y is 0 on all three. The globals' default unit camera track (x, y, z,
the same nine pitches): (0.2387, −0.0058, 3.8862), (−1.4858, −0.0019,
3.6764), (−2.8604, 0.0004, 2.9561), (−3.5224, 0.0009, 1.8384),
(−3.6826, −0.0001, 0.5752), (−3.1813, −0.0038, −0.7709), (−2.0725,
−0.0070, −1.5752), (−0.9813, −0.0070, −1.8648), (−0.0381, −0.0038,
−1.9795). Every track on the disc has 9 points (also the unused
`camera\warthog_driver_tight`, x, z: (−0.1282, 1.0641), (−0.2322,
1.0129), (−0.3042, 0.9482), (−0.3831, 0.8272), (−0.3947, 0.7195),
(−0.3698, 0.6719), (−0.3550, 0.6537), (−0.3322, 0.6454), (−0.2999,
0.6385)); each point also stores an orientation (a pitch quaternion
matching its angle), which the seat camera does not read. The camera's
distance is also multiplied by a per-camera distance scale, 1 in play:
offset (D cos θ + o_x, −o_y, D sin θ + o_z) × scale and D' = max((D −
0.6) × scale + 0.6, 0.6).

**Look limits and rates per seat** (the rules above; angles in
degrees; "player" = the player's own look rate; yaw arcs relative to
the seat marker's forward):

| Seat | View | Yaw rate, pitch rate (deg/s) | Pitch range (relative to tilt) | Auto level | Yaw arc |
|---|---|---|---|---|---|
| Warthog driver | third | 60, 30 | −30 to +15 | −18 | free |
| Warthog passenger | first (third while entering or leaving) | player, player | −50 to +90 | none | −90 to +90 about +x |
| Warthog gunner | third | 60, player | −20 to +35 | none | free |
| Ghost driver | third | 80, player | −30 to +30 | −5 | free |
| Scorpion driver | third | 50, player | −30 to +35 | −18 | free |
| Scorpion riders | first | player, player | −30 to +35 | −18 | −90 to +90 about the marker, which faces outward (+y left riders, −y right riders) |

### Seated riders

**The animation set.** A unit in a seat uses the graph seat whose label
equals the vehicle seat's label (`animations.md`, Seat, weapon class
and weapon type: the label search, last match wins) and, inside it,
the weapon class holding its weapon's label, falling back to the
seat's last class when it holds no usable weapon. Its base state is
idle, so the pose is that class's idle animation (looping), with these
layers (`animations.md`, Per-tick order): the acceleration overlays of
the seat (slots 2 to 4), the class's aim-still overlay as an aiming
screen, and the seat's IK points. The Spartan's sets
(`characters\cyborg\cyborg`):

| Vehicle seat label | Classes (weapon labels) | Idle (frames) | Aim-still screen | IK |
|---|---|---|---|---|
| W-driver | unarmed only | `W-driver unarmed idle` (50) | 3 × 3, ±90 degrees | hands to `left/right hand wheel grip` (steering wheel node) |
| W-gunner | fixed (only the unarmed type) | `W-gunner fixed idle` (50) | 5 × 3: yaw 90 per column (±180), pitch −20 to +35 | feet to `gunner left/right foot` (gun mount), hands to `gunner left/right hand` (gun); the class adds `left/right hand gun grip`, which the Warthog model lacks |
| W-passenger | rifle (ar, sg, sr, pr, f), pistol (hp, pp, ne, b), missile (rl), support (ft) | rifle and pistol 40, missile 30, support none | 3 × 3: yaw ±90, pitch −22 to +30 (missile +45, support −30 to +60) | left hand to the weapon (`left hand cyborg`) |
| G-driver | unarmed only | `G-driver unarmed idle` (50) | none (zero bounds) | hands to wheel grips (the Ghost model has none of that name) |
| scorpion-driver | unarmed only | `scorpion-driver unarmed idle` (5 frames, no node moves: the bind pose) | 3 × 3, ±90 | none |
| scorpionLF, RF, LB, RB | rifle ("", ar, pr, ne, sg, sr, f), pistol ("", hp, pp, b), missile ("", rl), support ("") | `scorpionLF rifle idle` (30) for every class | 3 × 3: yaw ±90, pitch −30 to +60 | left hand to `left hand lf` (`lb`, `rf`, `rb`) on the tread node |

**IK to a missing marker.** Each IK point names a marker on the rider
(the hand) and a marker on the vehicle (seat IK) or on the rider's
current weapon (class IK). The hand is pulled onto the grip only when
**both** markers exist by those names; if either is missing the point
is skipped entirely and that hand keeps the pose the animation gives
it (no pull toward the vehicle's origin or root node: the root-node
fallback of a failed marker lookup is not used here). So:

| Seat | IK point | Result |
|---|---|---|
| Warthog driver | hands to `left/right hand wheel grip` | the Warthog has them: hands on the wheel, moving with the steering |
| Warthog gunner | feet and hands to the gun mount and gun | applied |
| Warthog gunner, class IK | hands to `left/right hand gun grip` on the current weapon | missing: skipped (hands stay where the seat IK put them) |
| Ghost driver | hands to `left/right hand wheel grip` | the Ghost has no such markers: skipped; the hands stay in `G-driver unarmed idle`'s pose |
| Warthog passenger, Scorpion riders | left hand to the weapon's or the tread's marker | applied when the marker exists |

The seat IK applies only while the rider is attached to the vehicle,
the seat's IK is enabled in the animation state and no grenade throw
releases it; the class IK only while the rider holds a weapon and
weapon IK is enabled.

**Acceleration lean.** Each tick a seated unit takes the change of the
vehicle's per-tick displacement (its acceleration, WU/t^2) in the seat
marker's axes, multiplies each component by the seat's acceleration
scale (Warthog and Scorpion 100, 100, 50; Ghost 25, 100, 100), adds
0.5 and clamps to 0..1; the three values play the seat's acc-front-back,
acc-left-right and acc-up-down overlays at frame value × (N − 1) (N = 3
on the disc). At constant velocity all three sit at the middle frame;
a Warthog accelerating at 0.003 WU/t^2 puts the driver's front-back
overlay at 0.8 (frame 1.6).

**Who may shoot.** Only a unit whose current weapon fits its seat's
animation set holds a weapon (Weapons on entering and leaving):

- Warthog driver, Ghost driver, Scorpion driver: unarmed; their own
  weapons are stowed. The Ghost's and Scorpion's drivers fire the
  vehicle's gun through the gunner flag; the Warthog's driver cannot
  shoot.
- Warthog gunner: unarmed in the hands; fires the chaingun through
  the vehicle (Turrets and guns).
- **Warthog passenger: holds and fires its own weapon** with every
  multiplayer weapon (rifle, pistol, missile and support classes), in
  first person, aiming with its own look limited to the seat's ±90
  degree arc; reloading works as on foot (the reload's length comes
  from the weapon's first-person animation, not the rider's), but
  **no seated unit can throw a grenade or melee** (below).
- Scorpion riders: hold and fire their own weapon, except the
  flamethrower (no `ft` type in their sets: it is stowed).
- A rider's shots and damage are its own (`weapons.md`); it is not
  the vehicle's.

**The seat animation sets on the disc are complete as listed here**
(the Spartan's graph, every animation whose name starts with a seat
label): for each of W-driver, W-gunner, W-passenger, G-driver and
scorpionLF only `enter`, `exit`, the three acceleration overlays
(`acc-front-back`, `acc-left-right`, `acc-up-down`, 3 frames each;
none for scorpion-driver), and per class `idle` and `aim-still`
(W-driver unarmed aim-still 9 frames, W-gunner fixed aim-still 16,
W-passenger rifle, pistol and missile aim-still 9 each, scorpionLF
rifle aim-still 9; G-driver and scorpion-driver have none). The
scorpion-driver set has only enter (65), exit (55) and its 5-frame
bind-pose idle. Consequences, by the general rules
(`animations.md`):

1. **No turning, firing, reload, melee or throw animations in any
   seat.** A seated rider's body never plays a recoil or reload: an
   overlay or action whose animation is missing is simply not started
   (the shot, the reload and its timing still happen, from the weapon).
2. **No grenades and no melee from any seat.** The throw and the three
   melee states cannot be entered without an animation (the only
   states with that rule, with the two leap states), so pressing the
   grenade button or melee in a seat does nothing for a rider; for a
   driver-gunner the grenade button is the vehicle's trigger 1
   instead. The grenade count is kept.
3. **Turning** shows only through the yaw of the rider's aim-still
   screen (its aiming vector relative to the seat marker) and, for the
   Warthog driver, the hands following the steering wheel by IK. The
   rider's body otherwise stays in the idle pose of its seat, which is
   attached to the seat marker and so follows the vehicle (and the
   turret, for the gunner).
4. **Acceleration** is the acceleration lean above; **entering and
   exiting** are the enter and exit base animations (Entering, exiting
   and flipping), each started with a 6-tick blend.
5. **Aim** is the class's aim-still overlay played as an aiming screen
   on the rider's aiming vector (relative to the seat marker's frame),
   clipped to the class's bounds (table above). A W-gunner's aim-still
   turns the upper body with the turret's yaw through ±180 degrees in
   90-degree columns; a passenger's turns ±90 degrees.
6. A first-person rider (Warthog passenger, Scorpion riders) holding a
   weapon sees that weapon's first-person model and animations exactly
   as on foot (fire, reload, idle), placed at the seat's eye; seats
   whose rider is unarmed draw no first-person weapon.

**Visibility.** Riders are visible in all seats of the three
multiplayer vehicles (no seat has the invisible flag); a seated unit's
flashlight is switched off and cannot be switched on.

**The rider's HUD** (`hud.md` sections 2 and 7 for the pieces). Each
unit HUD in the list below is chosen as element 0 of its block, or
element 1 when more than one local player shares the screen (clamped
to the block's last element):

1. **Unit HUDs.** Always the rider's own (the Spartan's: shield,
   health, motion tracker, as on foot). A rider in a seat with the
   **driver** flag also gets the vehicle's own unit HUD (none on the
   three multiplayer vehicles) and then, for **every seated unit** of
   the vehicle in its child order (the driver itself included), that
   unit's seat HUD, drawn with that unit's state; a rider in any other
   seat gets no seat HUD at all. They are drawn last in the list first,
   so the rider's own HUD is on top. On the disc: the Warthog driver
   sees `ui\hud\warthog driver` (a reticle picture, `hud_reticles`
   sequence 14, at (0, −40) from the bottom centre, anchor 4, colour
   (40, 150, 255) opaque) plus, when the seats are taken,
   `ui\hud\passenger_2` (the passenger's health: background sequence 10
   of the unit-meter page at (12, 31), meter sequence 7 at (12, 41),
   top-left anchor) and `ui\hud\gunner` (the gunner's health:
   background sequence 9 at (24, 2), meter at (24, 12)); both meters
   run from (255, 0, 0) empty to (40, 150, 255) full with the medium
   colour (255, 223, 0) at cutoffs 0.33 and 0.67, flash (255, 85, 43);
   these show the riders' health to the driver only. The Ghost and Scorpion seats have
   no seat HUD, so their riders see only their own.
2. **Weapon HUD.** The rider's own current weapon's HUD when it holds
   one (Warthog passenger, Scorpion riders: the weapon's full HUD,
   ammunition and crosshair, as on foot). A rider holding none in a
   **gunner** seat gets the vehicle's current weapon's HUD, with the
   vehicle weapon's state: Warthog gunner `vehicles\warthog\warthog
   gun` (one crosshair, `hud_reticles` sequence 2, flashing, colour
   (40, 150, 255), no ammunition counter), Ghost driver
   `vehicles\ghost\ghost gun` (sequence 13, the same), Scorpion driver
   `vehicles\scorpion\scorpion cannon` (sequence 11 as the aim
   crosshair, plus crosshair type 7, "reload / overheat", drawn while
   magazine 0 reloads or chambers: `hud_reticle_warnings` sequence 9,
   the five progress frames, at (0, 30) from the centre, so a progress
   bar shows under the reticle for the cannon's 4 s chamber). None of
   the three has meters, numbers, overlays, sounds or a screen effect.
   A rider holding none in a seat without the gunner flag (the Warthog
   driver) gets **no** weapon HUD, not the empty default.
3. The weapon-swap prompt (a depleted weapon) is not shown in a driver
   or gunner seat.

### Turrets and guns

- The vehicle holds its weapon (unit weapons block); it is readied at
  creation and is invisible: its markers and effects are the vehicle's
  (the weapon's functions are written into the vehicle's incoming
  function slots, Exported values). A seat with the gunner flag (or
  invisible) makes its occupant aim *through* the vehicle: each tick,
  while the gunner is a player (or an AI not entering or leaving), the
  vehicle's desired aiming and looking vectors become the gunner's
  desired aiming vector, and the vehicle's triggers are the gunner's
  primary (fire) and secondary (grenade button) triggers and reload.
  The weapon fires from the vehicle exactly as `weapons.md` describes
  (its own triggers, rates, heat and ammunition), with the vehicle as
  the shooter (the vehicle is ignored by its own shots,
  `projectiles.md`).
- The vehicle's **aiming vector** follows the desired one with the
  unit's aiming velocity maximum / 30 per tick and acceleration maximum
  / 900 per tick^2 (Warthog and Banshee π rad/s and 10.472 rad/s^2,
  Scorpion π/2 and 13.963), and is clipped to the vehicle's own aiming
  bounds, which are the aiming screen of its graph's `stand` seat,
  `fixed` class (expressed relative to the vehicle's forward and up);
  a unit with both 0 (Ghost) aims instantly, clipped to its bounds.
  The aiming screen then poses the turret nodes (`animations.md`,
  Aiming and looking screens):

| Vehicle | Aim-still overlay (frames) | Screen | Aim limits (relative to the hull) | Nodes it turns |
|---|---|---|---|---|
| Warthog | `stand fixed aim-still` (15) | 5 × 3: yaw 90 per column, 2 each side; pitch −15 (1 row), +35 (1 row) | yaw ±180 (all round), pitch −15 to +35 | gun mount base (yaw), gun (pitch), chain |
| Scorpion | `stand fixed aim-still` (15) | the same | yaw ±180, pitch −15 to +35 | turret rotator (yaw), turret (pitch, with the cannon and the machine gun) |
| Ghost | `stand fixed aim-still` (3) | 1 × 3: no yaw; pitch −45, +20 | yaw 0 (the guns always point along the hull's yaw), pitch −45 to +20 | guns |

  So the Warthog's turret takes 1 s to swing 180 degrees at full rate
  (π rad/s), the Scorpion's 2 s; the Ghost's guns follow the camera's
  pitch at once and its yaw only by turning the whole Ghost.
- **Where shots start.** Each shot's origin is the vehicle model's
  `primary trigger` marker (trigger 1: `secondary trigger`) as posed
  by the turret: Warthog on the gun node, bind pose (0.2552, 0,
  0.9301) from the model origin; Ghost two markers on the guns node,
  (0.9420, ±0.1867, 0.0717); Scorpion cannon (1.5219, 0, 1.1998) and
  machine gun (0.6519, −0.1785, 1.2058) on the turret node. The
  direction is that marker's forward (the turret's actual pointing,
  which lags the camera while the turret swings), then the player's
  aim correction toward the camera's target (`weapons.md` section 6
  step 4, section 15), then the trigger's error. A trigger with
  "projectiles use weapon origin" fires from every marker of the name:
  the Ghost's gun fires one bolt from each of its two markers per shot
  (with identical error); the Scorpion's two triggers use their single
  markers. Inherited speed = direction · the vehicle's velocity.
- Warthog: the gunner seat (W-gunner) is the gunner; the chaingun is
  `vehicles\warthog\warthog gun` (one trigger, 8 to 15 rounds per
  second as it spins up over 1 s and down over 4 s, error 1 to 2
  degrees, no heat, no ammunition limit, projectile
  `vehicles\warthog\bullet`: `weapons.md` tables). The driver does not
  fire; the passenger fires its own weapon (Seated riders).
- Scorpion: the driver seat is driver and gunner: the turret aims where
  the driver looks; trigger 0 (fire) is the tank shell
  (`vehicles\scorpion\tank shell`), chamber-limited (one round loaded,
  chamber time 4 s: `weapons.md`), trigger 1 (grenade button)
  the machine gun at 15 per second (`vehicles\scorpion\bullet`). The
  four rider seats need a driver (flag 9), give no vehicle weapon and
  let the riders use their own (Seated riders).
- Ghost: driver-gunner, `vehicles\ghost\ghost gun` (5 to 6 shots per
  second, two bolts per shot, error 0.5 to 3 degrees, no heat: heat
  per round 0). **Its ammunition never runs out but it pauses:** the
  magazine holds 50 loaded rounds of a 300-round reserve and each shot
  (both bolts) costs 1 round; the reserve is only drawn down for a
  weapon that belongs to a player (a weapon picked up), which a
  vehicle's gun never is, so it stays at 300. When the loaded count
  reaches 0 the gun reloads: the reload's length is the weapon's
  first-person reload animation, which a vehicle gun lacks, so it ends
  on the next tick; the magazine is then chambered for its chamber
  time, 0.3 s (9 ticks). So after every 50 shots (about 9 s of held
  fire at 5.5 per second) the Ghost stops for about 10 ticks. The
  gunner's reload button starts the same reload early (when fewer than
  50 are loaded). The Scorpion's cannon follows the same path every
  shot (1 loaded round, 9999 reserve never drawn down): fire, reload
  on the next tick, chamber 4 s; its machine gun and the Warthog's gun
  use no rounds at all. Its **strafing** is the
  driver's left stick sideways (t_j): the drive targets (V t_i, V t_j)
  in the hull frame, with V the maximum forward speed in every
  direction (The scout).
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
- Riders: a hit on the vehicle that is **not area damage** (a bullet,
  a bolt, a projectile's direct impact; not an explosion's area, which
  reaches riders, if at all, through their own bodies) is passed to
  every biped child of the vehicle that is a player, and to the driver
  whatever it is, with the multiplier (1 − the damage effect's vehicle
  passthrough penalty) × the vehicle's rider damage fraction. The same
  multiplier applies in **every seat** (driver, passenger, gunner,
  Scorpion riders): 0.18 on the three multiplayer tags and the campaign
  Warthog and Scorpion, **0 on the campaign Ghost and the Banshee**.
  The passed damage is dealt to the rider as an ordinary hit with no
  body region (shields first for players; a non-player driver's
  shields are bypassed), then the multiplier returns to 1 for the
  vehicle's own damage. Rule and penalties are `health_shields.md` and
  `projectiles.md`. So stage 1's 18% is right for every seat, provided
  it is applied only to non-area damage and further scaled by the
  effect's passthrough penalty.
- A seated rider is also hit directly by shots that reach its own
  collision model (a Warthog passenger or gunner is exposed); that hit
  is the rider's, at full value.
- Pushes from damage: Pushes on the vehicle. The globals' vehicle hit
  environment and vehicle killed unit effects do nothing on the disc.

### Multiplayer

**Which placements spawn.** At map load every vehicle placement of the
scenario (`scenario.md`, block 0x240) is created when all of these
hold, and no other test applies:

1. its palette index names a palette entry (the disc's always do);
2. its placement flag bit 0 ("not automatic") is clear (0 on every
   disc placement); the difficulty bits 1 to 3 are ignored in
   multiplayer;
3. its name is not already in use by a living object (the disc's
   placements have no names);
4. the game type keeps it: in a running multiplayer game the vehicle
   tag is first substituted (`multiplayer_rules.md` 5.3 and 8): a tag
   other than the globals' three multiplayer vehicles (`globals.md`
   multiplayer information: `warthog`, `ghost_mp`, `scorpion_mp`)
   creates nothing; vehicle set 0 (default) keeps all three, set 1
   keeps none, set 2 only the Warthog, 3 only `ghost_mp`, 4 only
   `scorpion_mp`. The placement has no team, so a team game's two
   sides get the same set.

**The on-BSP flags (placement offset 0x20) are not read for vehicles.**
Only scenery and light fixtures are filtered by them (`scenario.md`);
on the disc the field is 0 on every vehicle placement, so applying the
scenery rule to vehicles wrongly removes all of them. The created
vehicle's centre of mass is at the placement position raised by half
its bounding radius (Units and conventions); the placement's yaw,
pitch and roll give its forward and up (`scenario.md`, Rotation and
facing). Race deletes these and creates its own (`multiplayer_rules.md`
6.7). Blood Gulch under the stock slayer variants (set 2) therefore
shows its 2 Warthogs; with set 0 all 8 vehicles.

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

The vehicle's pose each tick (when node matrices are built) is its
base animation (the `stand` seat's idle, or the bind pose: none of the
three moves a node in its base), then these overlays from the graph's
vehicle block (`animations.md` vehicle slots), then the turret's
aiming screen (Turrets and guns) and the object overlays:

| Slot | Played at | Warthog | Ghost | Scorpion |
|---|---|---|---|---|
| 0 steering | an aiming screen with yaw = τ, pitch 0, over the block's steering bounds | `vehicle steering`, 3 frames, 35 degrees per column: τ = 30 degrees shows 6/7 of the full-left frame; moves the steering wheel and the four wheel hubs (so the visible front wheels turn with τ and the rears against it) | `vehicle steering` on the two flaps, 30 degrees per column, but τ is always 0 for the Ghost (its turn limits and rate are 0), so **the flaps never move** | none |
| 1 roll | frame ((u × f) · v / S_f + 1)/2 × (N − 1), clamped | none | none | none |
| 2 throttle | frame (s/S_f + 1)/2 × (N − 1); reversing 0.5 − s/S_r × 0.5 | none | none | none |
| 3 velocity | frame clamp(v·f, 0, 1)/S_f clamped to 1, × (N − 1) | none | none | none |
| 5 ground speed | frame (W/C) × N, continuous and wrapping, W the wheel position (W += s per tick, mod C) | `vehicle ground-speed`, 4 frames, the four tyre nodes: **one turn per 1 WU of commanded travel** (C = 1), driven by s, not by the ground speed (wheels spin while stuck and stop when s = 0 even while sliding) | none | none |
| suspension | frame (stored value) × (N − 1), one overlay per element (Collisions) | four, 2 frames each, the tyres' suspension nodes | none | eight: `tread suspension` at points 1, 2, 5, 6 (E −0.3836, C −0.1671) and `tread pitch` at points 0, 3, 4, 7 (front E −0.4104, C −0.1113; back E −0.3711, C −0.1489) |

- **Object overlays** (`animations.md` Other layers, `objects.md`
  object functions): the Warthog's `firing` overlay (4 frames: the
  chain, the gun and the barrels) plays in frame mode on the vehicle's
  function D, `chaingun`: input D is the gun's primary rate of fire
  (the invisible weapon writes its function sources into the vehicle:
  `weapons.md` warthog gun D = 4, primary rate of fire, the trigger's
  0..1 spin-up fraction), passed through the function's bounds (scale
  to fit, 0 to 0.18) and its **additive** flag (`objects.md` Object
  functions, step 15): the output accumulates each tick modulo 1 and
  the overlay plays at frame 4 × output, so the barrels spin while the
  gun spins up, fires and spins down, and stop at rate 0. The
  Scorpion's single object overlay names no animation.
- **Recoil overlays.** The vehicle is the unit holding its gun, so a
  shot starts the vehicle's own weapon-type recoil overlay
  (`animations.md`: primary shot fire-1, secondary shot fire-2, in the
  vehicle graph's `stand` seat, `fixed` class), as for a biped: the
  Scorpion's `stand fixed cannon fire-1` (22 frames: the cannon's
  recoil) on each shell and `stand fixed cg fire-2` (5 frames) on each
  machine-gun round; the Ghost's `stand fixed c ghost gun fire-1` (10
  frames) on each shot. The Warthog has no recoil overlay (its barrels
  spin by the `firing` object overlay above). The Scorpion's base is
  `stand fixed idle` (5 frames, no movement).
- **Functions for shaders and effects** (Vehicle function sources):
  Warthog B jump/brake, C engine hack; Ghost B engine hack; Scorpion A
  and B the tread positions (scrolling tread textures), D engine hack.
  Unit input A is the driver's seat power on the Warthog and Ghost (C
  on the Scorpion).
- **Hatch.** The Scorpion's `stand` seat has opening and closing (16
  frames each): the driver's exit starts opening, the end of the
  driver's entry starts closing; both hold their last frame.
- The `~blur` region permutation shows while |s| >= blur speed
  (Warthog 0.08, Scorpion 0.05 WU/t).

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
   in the order of "Order of one step" (forces from the start state,
   collision accumulators, one integration, penetration freeze, sleep
   and ground flags, then contacts with bipeds and other vehicles),
   the Ghost's hover update; sliding effects; suspension,
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
4. If the vehicle's up.z < 0.2 and it is on the ground: leave now
   (Exiting steps 2 to 4, with no exit velocity: the rider leaves from
   its seated pose with the vehicle's velocity).
5. Its own weapon is updated with its own triggers (riders that hold
   one: Seated riders).
6. Entry or exit animation progress; at the exit animation's end, the
   detach of Exiting; at the entry animation's end, visibility and the
   vehicle's closing.
7. Pose: the seat's idle, acceleration lean, aim-still screen, IK.

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
- **Vehicles on vehicles.** A vehicle's mass points never stand on
  another vehicle (the ground query and the freeze ignore vehicles):
  a Warthog dropped on a Scorpion is held off only by the sphere-pair
  contact force (Collisions), which is a spring of stiffness 2 √(M_a
  M_b) g / 0.2 per penetration of the summed radii, with no damping
  and no friction, so it slides off. A mass point resting on a
  machine (a lift) is on a volatile surface: the vehicle cannot sleep
  there.
- **Vehicle against vehicle** has no damage, no friction and no
  restitution: only that pair spring, applied in the next tick's step.
  Splatter applies to bipeds only.
- **Water.** No multiplayer vehicle has a water channel; water acts only
  through buoyancy (density), water friction (μ_w = 0.05 per tick on
  each submerged point, shaped by its friction type) and, for the
  Ghost, the 0.5 WU cut-off of its drive. Full buoyancy is Σ m_i/d_i ·
  ρ_w · g = (M/density) g: 20% of the weight for the Warthog, 33% for
  the Ghost, 12.5% for the Scorpion, so **every vehicle sinks**; a
  wholly submerged vehicle falls at the speed where water and air
  drag balance the rest. The depth fraction ramps over the first 0.25
  WU (water depth). Multiplayer water (Damnation, Blood Gulch's
  stream) is shallow: wading vehicles only slow down. Nothing in the
  vehicle rules ejects riders in water.

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
  wheels. A Warthog placed with its placement point (its centre of
  mass) on flat ground is created 0.55 WU up, falls asleep on tick 19
  and freezes 0.033 WU above equilibrium; one whose model origin starts
  0.55 WU above the ground (centre of mass at 0.8718) falls asleep on
  tick 25 and freezes 0.062 WU above equilibrium, front-down by 2.4
  degrees. Keep the rule if spawn poses must match.
- **The Ghost's flaps never move** (its τ is always 0) although its
  graph has a steering overlay for them.
- **Suspension traces start c_x ahead of the wheel** (the mass point is
  placed from the centre of mass, not the model origin): on a bump the
  Warthog's wheels animate 0.107 WU early. Keep (visual only).
- **The Warthog gunner's entry point turns with the turret**: its seat
  marker is on the gun mount, so a turret left facing sideways moves
  the place to enter it.
- **The third-person camera ignores the vehicle's roll and pitch**: its
  offset is in the look's horizontal heading frame, so it stays level
  when the vehicle tilts.
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
- **Presentation**: the camera position rule (focus at the centre of
  mass or the named marker, the track point in the heading frame, the
  pull-in), the enter and exit lengths and exit spots, who holds a
  weapon in which seat, the turret's screen limits and rates, and the
  overlays driven by τ, the wheel position and the suspension values.

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
| Warthog, no driver input, sleep off | none | settled | centre of mass z = 0.35360 (model origin 0.0327 above ground; 0.0318 if the tilt is ignored), forward.z = −0.0078 (nose down 0.45 degrees); tyre depths 0.03243 (rear), 0.04257 (front), sum over four 0.15 |
| Warthog dropped from 1 WU above equilibrium (same orientation, v = ω = 0), sleep off | none | ticks 20 to 30, 57 | centre of mass relative to equilibrium: tick 20 +0.2705, 21 +0.1986, 22 +0.1234, 23 +0.0451, 24 −0.0287, 25 −0.0844, 26 −0.1139, **27 −0.1215 (lowest)**, 28 −0.1087, 29 −0.0871, 30 −0.0622; v_z at tick 23 −0.0784, at 27 −0.0077; within 0.01 WU and |v_z| < 0.001 from tick 57 on (tick 57: +0.0031). The freeze never acts (no sphere centre reaches the ground) |
| the same, sleep on | none | | falls asleep on tick 42 (0.010 above, moving down 0.0077), freezes 0.026 below equilibrium |
| Warthog created from a placement on flat ground (centre of mass at the placement, raised 0.55: z = 0.55), sleep on | none | | asleep on tick 19 moving up 0.0093; freezes 0.033 above equilibrium |
| Warthog with its model origin 0.55 above the plane (centre of mass z = 0.8718), sleep on | none | | asleep on tick 25 moving up 0.018; freezes 0.062 above equilibrium, forward.z −0.043 |
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
| Ghost at top speed | release | | below 0.001 WU/t after 43 ticks, 4.57 WU on |
| Ghost hovering | t_i = 1 | 1 s, 2 s, 5 s, 10 s | distance 2.27, 8.35, 28.55, 62.21 WU |
| Ghost empty, at rest on its spheres | driver enters (power rises 1/60 per tick) | ticks 30, 60, 90 | centre of mass z 0.449, 0.662, 0.668 (hover 0.665) |
| Ghost hovering still | camera 90 degrees left | ticks 5, 10, 20, 30, 40 | yaw 3.0, 10.9, 41.4, 76.0, 90.5 degrees; peak rate 4.2 degrees per tick (the ideal bang-bang with the 0.2 degree per tick squared cap peaks at exactly 4.2 at tick 21 and gives 3.0, 11.0, 42.0, 76.7, 91.5; the simulation's small losses give the angles stated; the earlier "3.9" was wrong); settled at 90 by tick 60 |
| Scorpion | none | settled | centre of mass z = 0.49530 (origin 0.07175 above ground) |
| Scorpion | t_i = 1 | ticks 30, 60, 120, 300 | v = 0.04048, 0.09871, 0.13827, 0.13844 (top: 12.66 m/s) |
| Scorpion | t_i = 1 | 1 s, 2 s, 4 s, 5 s, 10 s | distance 0.509, 2.619, 10.44, 14.59, 35.36 WU |
| Scorpion at top speed | release (t_i = 0) | | s = 0 after 24 ticks; ground speed below 0.001 after 66 ticks, 2.94 WU on |
| Scorpion | t_i = −1 | steady | −0.11866 |
| Scorpion at rest | camera 90 degrees left, t_i = 0 | any | yaw rate 0 |
| Scorpion at top speed | camera 90 degrees left, t_i = 1 | ticks 10, 30, 60, 300 | τ = 0.04, 0.12, 0.14; yaw rate 0.0105, 0.0506, 0.0767, 0.0774 rad/t (133 degrees per second); speed falls to 0.107; path radius 1.39 WU |
| Banshee, driver power 1, s = 0, near the ground | none | settled | hovers on its antigravity, lowest sphere 0.111 WU up, nose 13.8 degrees up |
| Banshee | t_i = 1, camera level | ticks 30, 60 | |v| 0.1237, 0.2193; steady 0.21931 (20.05 m/s) along the nose, which is 15 degrees up: climbing at 0.0567 WU/t |
| Banshee at speed | camera 45 degrees up | steady | nose 60 degrees, v_z 0.1899, horizontal 0.1097 |
| Banshee at speed | camera straight up | steady | v_z 0.2119 |
| Banshee at speed, high | t_i = 0 | ticks 30, 120, 300 | s = 0 after 14 ticks; v_z −0.051, −0.278, −0.498 (falling) |

### Freeze, placement, camera, seats and animation cases

(a) Synthetic:

| Setup | Input | After | Expected |
|---|---|---|---|
| freeze: earliest hit t_h = 0.5, n_h = (0, 0, 1), sweep Δ = (0, 0, −0.1), v' = (0.2, 0, −0.1), ω' = (0, 0.01, 0), next pass clear | | one tick | ε = 0.078125, t' = 0.421875; v' = (0.2, 0, −0.0421875) (only the normal part scaled), ω' = (0, 0.00421875, 0); the vehicle moves by the new v' |
| freeze: all four passes hit | | one tick | position and orientation unchanged from the tick's start; the scaled v' and ω' kept |
| freeze: hit with n_h · v' > 0 (moving away) | | | v' unchanged, ω' scaled by t' |
| placement with on-BSP flags 0, flags 0, vehicle set 0 | load | | created |
| placement of `vehicles\ghost\ghost` (campaign tag) in a multiplayer game, set 0 | load | | nothing (not one of the globals' three) |
| placement of the Warthog, vehicle set 3 (Ghost) | load | | nothing |
| placement at (x, y, 0.29), Warthog | load | | centre of mass created at (x, y, 0.84); model origin at 0.84 − 0.3218 (level) |
| camera track evaluation, 9 points, θ = 0 | | | t = 0.5, j = 4, points 3..6, s = 1: o = control point 4 exactly |
| camera track evaluation, θ = −90 degrees, θ = +90 degrees | | | points 0..3 at s = 0 (point 0), points 5..8 at s = 3 (point 8) |
| third-person command, o = (−2, 0, 1), θ = 0, F = (0, 0, 0), heading +x, no obstruction | | | D = 2.2361; offset (0.2361, 0, 1); Q = (0.2361, 0, 1); camera (−2, 0, 1), looking +x |
| the same, heading +y (ψ = 90 degrees) | | | camera (0, −2, 1), looking +y |
| the same, a wall plane facing the camera at x = −1 | | | ray from Q back 2.2561 WU hits at fraction (1.2361/2.2561) = 0.5479: camera at x = 0.2361 − 2.2361 × 0.5479 = −0.9891 (the four probes hit the same plane at fractions ≥ 0.5479, none strictly nearer, so no bisection) |
| pull-in probes, D = 2.2361 | | | step = 0.174 × 2.2361 = 0.3891 WU; probes in the order end − step up, end − step left, end + step up, end + step left |
| pull-in, D = 2.2361, centre ray misses (t1 = 1), only the "+ step left" probe hits (t2 = 0.4); synthetic edge: rays with m >= 0.3 hit at 0.4, rays with m < 0.3 miss | 10 bisection steps: m = 0.5 hit, 0.25 miss, 0.375 hit, 0.3125 hit, 0.28125 miss, 0.296875 miss, 0.3046875 hit, 0.30078125 hit, 0.298828125 miss, 0.2998046875 miss | | last-miss 1 is not below last-hit 0.4, so s = hi = 0.30078125; distance = D × (0.69921875 × 0.4 + 0.30078125) = D × 0.58046875 = 1.2980 WU (the camera is pulled in although the centre line is clear) |
| pull-in, bisection where the last non-matching ray hit something nearer (last-miss 0.2 < last-hit 0.4) | | | s = lo (the side toward the centre) |
| default unit camera track, θ = 0 | | | o = (−3.6826, −0.0001, 0.5752) (point 4) |
| suspension, Warthog tyre centre 0.2227 WU above flat ground, previous value 0.51 | | | k = (−0.2227 + 0.31445)/0.17993 = 0.510; stored value unchanged |
| suspension, ground 0.40 below the tyre centre (wheel hanging) | | | k = 0 |
| suspension, ground 0.10 below the tyre centre | | | k = 1 |
| flip strength, up.z = −1, −0.5, −0.2, 0, 0.5 | | | k = 0.75, 0.75, 0.4, 0.2, 0.2; rate 0.3 k = 0.225, 0.225, 0.12, 0.06, 0.06 rad/t |
| rider damage, bullet on a Warthog (passthrough penalty p), passenger and gunner players | 10 damage points | | each rider takes 10 × (1 − p) × 0.18; same in every seat; a grenade's area damage passes nothing this way |
| acceleration lean, Warthog driver, vehicle accelerating 0.003 WU/t^2 forward | | | front-back value 0.8, frame 1.6 of 3 |

(b) Against the disc (bloodgulch tags; vehicle level at rest):

| Setup | Expected |
|---|---|
| Warthog driver camera, look level along the vehicle | focus = centre of mass (model (0.1069, 0, 0.3218)); camera at model (−2.6455, 0, 1.0045), 1.037 WU above the ground |
| Warthog driver camera, look 30 degrees down | track points 1..4, s = 1.667: o = (−2.4078, 0, 1.8922); camera at model (−2.3009, 0, 2.2140) |
| Warthog driver camera, look 30 degrees up | o = (−2.2864, 0, 0.0048); camera at model (−2.1795, 0, 0.3266) |
| Scorpion driver camera, level | o = (−4.6130, 0, 2.1571); camera at model (−4.6468, 0, 2.5807) |
| Ghost driver camera, level | focus = hull node (0.3928, 0, 0.1711) (the named marker is missing); camera at model (−2.3596, 0, 0.8538) |
| Warthog gunner camera, level, turret forward | focus `camera gunner` (−0.2235, 0, 0.8131); camera at model (−2.9759, 0, 1.4958) |
| Warthog passenger eye (first person) | `camera rider` at model (0.0715, −0.1586, 0.8056) |
| third-person camera, no transition running, Warthog driving at 0.1 WU/tick | camera each frame exactly at the computed C (no lag: the offset between computed and shown is 0) |
| a parameter at 0 with target 1 (fixed), w = a = 0, a 1 s transition, frames of 1/30 s | value after 15 frames 0.5, after 6 frames 0.0579 (10 q^3 − 15 q^4 + 6 q^5 at q = 0.2), after 30 frames exactly 1 |
| Ghost driver seated | hands: the IK points are skipped (no `hand wheel grip` markers); hand nodes equal the idle animation's |
| entry points (model frame) | W-driver (−0.0068, 0.5478, 0.3921); W-passenger (−0.0247, −0.5642, 0.4723); W-gunner (−1.1789, 0, 0.3658); G-driver (−0.0830, −0.3165, 0.1110); Scorpion driver (−0.0849, 1.0491, 0.3595); scorpionLF (0.1762, 1.2389, 0.3958) |
| player whose bounding sphere centre is 0.9 WU from the W-driver entry point and 1.3 WU from the driver marker, still, Warthog upright | offered W-driver |
| enter and exit lengths (ticks) | W-driver 50 / 26, W-passenger 28 / 30, W-gunner 46 / 24, G-driver 35 / 24, Scorpion driver 65 / 55, Scorpion riders 30 / 22 |
| player enters the Warthog driver seat and holds full throttle from that tick | the speed variable is 0.00275 on the next vehicle tick (no wait for the 50-tick animation) |
| Warthog driver exits, vehicle at rest, nothing blocking | after 26 ticks: origin at model (0.0328, 0.4945, 0.1784) before fitting, velocity (0.0014, 0.0285, −0.0096) in the vehicle frame, facing (0.25, 0.97) flattened |
| Warthog driver exits while the vehicle moves at (0.2, 0, 0) | exit velocity (0.2014, 0.0285, −0.0096) (level vehicle, heading +x) |
| a player with an assault rifle enters W-passenger | keeps the rifle, animation set W-passenger rifle (idle 40 frames), can fire |
| a player with an assault rifle enters W-driver or W-gunner | rifle stowed, unarmed; on exit the rifle is readied again |
| a player with a flamethrower enters a Scorpion rider seat | flamethrower stowed (no `ft` type) |
| Warthog passenger with an assault rifle and 2 frag grenades presses the grenade button, then melee | nothing happens (no throw or melee animation in the seat); still 2 grenades |
| Warthog passenger reloads the assault rifle | the magazine refills after the weapon's own reload time; the rider's third-person body plays no reload |
| Ghost driver holds fire from a full gun | 50 shots (100 bolts), then about 10 ticks without a shot (a 1-tick reload and the 9-tick chamber), then fire resumes; the reserve stays 300 |
| Scorpion driver fires the cannon | the cannon fires, the vehicle plays `stand fixed cannon fire-1` (22 frames), the "reload / overheat" crosshair shows the progress frames for the 4 s chamber, the next shell 4 s later |
| HUD, Warthog driver alone | own Spartan HUD, the `warthog driver` reticle picture at (0, −40) from the bottom centre; no weapon HUD |
| HUD, Warthog driver with a passenger and a gunner | adds the passenger's and the gunner's health meters (top left, (12, 41) and (24, 12)) |
| HUD, Warthog gunner | own Spartan HUD and the warthog gun's crosshair (sequence 2); no seat HUDs |
| HUD, Warthog passenger with a pistol | own Spartan HUD and the pistol's HUD, as on foot |
| HUD, Ghost driver | own Spartan HUD and the ghost gun crosshair (sequence 13) |
| Warthog ground-speed overlay, s = 0.255 for 1 tick from W = 0.9 | W = 0.155 (mod 1), frame 0.62 of 4 |
| Warthog steering overlay, τ = 0.5236 (30 degrees) | 6/7 of the way from the centre frame to the full-left frame |
| Ghost with camera 90 degrees left | its steering overlay stays at the centre frame (τ = 0) |
| Warthog turret, gunner looks 180 degrees behind from rest (turret forward) | yaw rate rises by 10.472/900 rad/t per tick to π/30 rad/t; the turret's aim is clipped to pitch −15..+35 degrees |
| Ghost guns, camera 30 degrees right of the hull, 60 degrees down | gun aim yaw 0 (clipped), pitch −45 degrees (clipped) |

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
| Jeep, tank, scout, fighter models | reference only; the jeep's wheel yaw sign traced through the engine's quaternion and matrix conventions |
| Placement | reference (creation path and game-type substitution) and the disc (flags) |
| Camera | reference (track evaluation, command, observer placement and pull-in, the observer's transitions and timers); tracks and markers from the disc; worked examples computed from them; the pull-in's probe order, tie rule and bisection end traced; the focus end-velocity sign traced (the end velocity in time is minus the unit's); all tracks re-read from the disc (9 points each) |
| Rider IK with missing markers | reference (the point is skipped when either marker is missing) |
| Entering, exiting, seated riders | reference for the rules; marker positions, animation lengths, root tracks, classes and bounds read from the disc (the Spartan's graph and the three models, bind pose) |
| Turrets, vehicle animation | reference for the rules; screens, nodes, overlays and suspension elements from the disc |
| Sleep and slow stop | reference only |
| Collisions, pushes | reference only; damage effect values from `projectiles.md` (disc) |
| Entering, exiting, flipping, ejection | reference only; the ejection threshold (0.2) is what the reference states the retail game does; its own older build used 0 |
| Input | reference only |
| Guns | weapons checked against the disc (triggers, projectiles); routing and shot origin reference only; markers from the disc; the Ghost's reload and chamber pause from the reference's magazine rules (the trigger for the automatic reload at 0 loaded not traced line by line; "about 10 ticks" may be 9 to 11) |
| Seat animation sets, grenades and melee in seats | animation lists read from the disc (every seat animation of the Spartan's graph and the vehicles' graphs); the no-animation rules from the reference |
| The rider's HUD per seat | selection rules from the reference; HUD tags read from the disc |
| Damage | vitality from `health_shields.md` (disc); rider fractions checked on the disc; the passthrough rule (non-area only, every seat) reference |
| Multiplayer placements | checked against the disc (all 13 maps) |
| Test cases | formula cases computed from the rules; simulation cases from a scratch simulation of the model as written here, on flat ground only; the drop and spawn cases re-run in a second, independent simulation written from this spec, which reproduces every other Warthog, Ghost and Scorpion number of the tables to the digits given |

## Open questions

1. **Simulation not validated against the game.** Every number of the
   simulation cases comes from the model as read. Measure in the game:
   time a Warthog from rest over 10 s on Blood Gulch's flat with full
   stick (expected 55 WU), its top speed (0.234 WU/t, 7.0 WU/s), the
   Ghost's (0.224), the Scorpion's (0.138).
2. **Throttle as a speed.** Does a half-pressed stick really give a
   full-speed Warthog? Drive with the stick at 10% and 50% and compare
   speeds (expected about 0.09 and 0.234 WU/t).
3. **Front wheels toward the turn.** Resolved: traced through the
   engine's conventions (The jeep). Kept as a check in the game: the
   rear wheels counter-steer.
4. **Banshee pitch.** Nose up 15 degrees with the camera level, so a
   level camera climbs. Measure: hold full throttle with the camera on
   the horizon and watch altitude. Also whether the Banshee falls when
   the throttle is released (the reading: free fall above 0.75 WU).
5. **Sleep mid-bounce** and the spawn pose. Compare the height of a
   freshly spawned Blood Gulch Warthog's model origin above the ground
   with the equilibrium (0.033 WU) and the frozen pose of the matching
   simulation case.
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
11. **The observer's smoothing.** Settled from the reference (Third-
    person camera, The observer's transitions): no lag in steady play;
    quintic eases only over the 1 s and 0.5 s transitions. The focus
    end-velocity sign is now traced (Third-person camera, step 2 of the
    transitions). Open: whether a seated rider's own object velocity
    (which the term reads) is the vehicle's or 0, and the exact frame on which a
    seat change is seen (it compares the followed unit and seat with
    the previous frame's). Check on video that entering a Warthog eases
    the camera out over 1 s and that driving shows no trailing.
12. **Splatter and friendly fire.** The damage goes through the normal
    rules with the driver as owner; whether a teammate is splattered
    depends on the game type's friendly fire (`health_shields.md`).
13. **IK to missing markers.** Settled from the reference (Seated
    riders, "IK to a missing marker"): an IK point whose grip marker is
    missing from the vehicle (or weapon), or whose hand marker is
    missing from the rider, is skipped; the hand keeps the animation's
    pose. Not checked on hardware: whether the Ghost driver's idle
    animation alone puts the Spartan's hands near the Ghost's handles.
14. **Marker positions are bind pose.** The seat, camera and trigger
    markers above are given in the bind pose; the turret, steering and
    suspension overlays move the ones on animated nodes (gunner, gunner
    camera, triggers, Scorpion riders' markers on the tread nodes).
