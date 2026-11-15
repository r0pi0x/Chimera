# Halo: Combat Evolved: projectiles, damage effects, grenades

Source basis: the game's own map files from the original Xbox disc (all
16 projectile tags and all 66 damage effect tags of `bloodgulch.map`,
compared with `hangemhigh.map`, `sidewinder.map`, `chillout.map` and the
two grenade projectiles of `ui.map`), two checks of the disc's own
executable, and a decompilation of an Xbox build of the engine (the
projectile update, the damage system, the effect system's damage parts,
the unit's grenade throw, the biped's push) used for behaviour. The
decompilation describes another Xbox build; where the disc says
otherwise the disc wins and the spec says so. The numbers in the test
cases come from a throwaway re-statement of these rules, not from the
real game.
Status: draft

Sections: Summary; Units and conventions; Constants; The projectile
tag; The damage effect tag; Behaviour (1 creation, 2 timers, 3 flight,
4 hitscan versus slow projectiles, 5 the per-tick trace, 6 choosing a
material response, 7 the responses, 8 the damage of a direct hit, 9
detonation, 10 area damage, 11 the push, 12 guidance, 13 attaching and
the needler's supercombine, 14 the plasma pistol's charged bolt, 15
grenades, 16 the other projectiles, 17 presentation hooks, 18 the damage
event); What is on the disc; Per-tick order; Edge cases; Quirks; Test
cases; Verification performed; Confidence per section; Open questions.

---

## Summary

- There is no hitscan. Every shot is a projectile object that moves
  once per tick along a straight segment from where it is to where its
  velocity takes it, tracing the level and every object's collision
  model along that segment. A pistol bullet covers 10 WU (30.5 m) per
  tick and a sniper bullet 33.3 WU (101.6 m), so most hits land on the
  tick of the shot; plasma bolts (1.67 WU/tick), needles (0.133), rockets
  (0.4) and grenades are visibly slow. A projectile gets its first
  update on the tick it is fired.
- Speed falls from an initial to a final speed over a distance window
  ("air damage range") at a constant deceleration, and **the damage of a
  direct hit is interpolated between the damage effect's minimum and
  its random full amount by how much of that speed is left**. Pistol,
  assault rifle and sniper bullets never slow, so their damage never
  falls off; they simply vanish at their maximum range (40, 40 and 1000
  WU).
- What happens at a surface is a per-material table on the projectile:
  disappear, detonate, bounce (two frictions), pass through, or stick.
  Stuck needles count toward a supercombine; grenades bounce (frag) or
  stick to units and vehicles (plasma).
- Explosions are damage effects inside the detonation effect: every
  object whose bounding sphere reaches the cutoff radius and that is
  not occluded takes the full random amount inside the falloff radius,
  falling linearly toward the minimum at the cutoff radius and nothing
  beyond. The push is a fixed velocity change (not scaled by distance)
  along the blast direction tilted upward: this is the grenade jump and
  the Warthog flip.

## Units and conventions

Everything in [cache_file.md](cache_file.md) applies: little-endian, tag
data addresses, 1 world unit (WU) = 3.048 m, +Z up, radians, 30 ticks
per second, blocks, tag references and data references as defined
there. Offsets in the projectile tables are absolute from the start of
the tag's main struct, as in [objects.md](objects.md) (which owns the
object base, 0x000 to 0x17B). Units of stored values, checked against
the disc and the engine's use:

- projectile initial and final velocity, minimum velocity: **WU per
  tick** (multiply by 30 for WU/s, by 91.44 for m/s);
- air and water damage range bounds, maximum range, collision radius,
  danger and perception radii: WU;
- arming time, timer bounds: seconds (the engine multiplies by 30);
- guided angular velocity: **radians per second** (the engine divides
  by 30);
- material response angle bounds and angular noise: radians; velocity
  bounds and velocity noise: WU per tick;
- damage effect radii and core radius: WU; damage amounts: points of
  vitality (a multiplayer Spartan has 75 shield and 75 body points:
  `health_shields.md`); stun time: seconds; instantaneous acceleration:
  WU per second of velocity change (the engine divides by 30).

"Update" below means one call of the projectile's per-tick update;
"update k" is the k-th since the projectile was created, update 1
happening on the tick it was created (behaviour 4). `U(a, b)` is a
uniform random real in [a, b]; clamp(x, a, b) limits x to [a, b].
Material type codes are those of [collision.md](collision.md) (0 dirt
... 21 cyborg, 22 cyborg energy shield ... 32 hunter shield).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| PROJECTILE_STRUCT_SIZE | 0x24C | bytes | `proj` main struct (objects.md); own fields from 0x17C |
| RESPONSE_SIZE | 0xA0 | bytes | one material response element |
| DAMAGE_EFFECT_SIZE | 0x2A0 | bytes | `jpt!` main struct |
| MATERIAL_SLOTS | 40 | reals | material modifier slots in a damage effect; 33 used (types 0 to 32), slots 33 to 39 are zero on the disc |
| GRAVITY | 0.0035651792 | WU/tick^2 | engine gravity (cache_file.md); a projectile uses GRAVITY x its gravity scale |
| TICKS | 30 | ticks/s | seconds to ticks |
| MAX_COLLISIONS_PER_UPDATE | 10 | | surfaces a projectile can meet in one update; reaching it requests detonation |
| REST_SPEED_SQUARED | 0.0001 | (WU/tick)^2 | after a response, a speed squared below this means "stopped" (speed below 0.01 WU/tick) |
| FLOOR_NORMAL_Z | 0.3 | | a hit surface whose normal has z above this counts as ground (rest, "bounced") |
| IMPACT_EFFECT_SPEED | 1/120 | WU/tick | the impact effect plays only if the normal speed into the surface exceeds this |
| MEDIA_STEP | 0.001 | WU | how far a projectile is pushed through a water surface it passes |
| COMBINE_COUNT | 6 | | more than this many unconsumed needles in one biped make a supercombine |
| PUSH_UPWARD_BIAS | 0.45 | | added to the z of the blast direction before normalising |
| BIPED_PUSH_SCALE | 0.5 | | a living biped takes half of the push |
| AREA_OBJECTS_MAX | 64 | objects | objects one area damage event can consider (reference) |
| WANDER_NEAR, WANDER_FAR | 2, 10 | WU | guidance wander ramps from 0 to 1 WU between these target distances |
| GUIDANCE_VS_PLAYER | 0.5, 0.5, 0.75, 1.0 | factor | easy, normal, heroic, legendary: guidance rate multiplier when the target is a player's unit (globals difficulty table, value 19, read from the disc) |

## The projectile tag

### Projectile part (absolute offsets)

| Offset | Size | Type | Name (as the tag has it) | Unit | Meaning |
|---|---|---|---|---|---|
| 0x17C | 4 | flags32 | flags | | see Projectile flags |
| 0x180 | 2 | enum16 | detonation timer starts | | 0 immediately, 1 after first bounce, 2 when at rest (but see behaviour 2: 1 and 2 act the same) |
| 0x182 | 2 | enum16 | impact noise | | how loud an impact is to AI hearing: 0 silent, 1 medium, 2 loud, 3 shout, 4 quiet (names inferred, as objects.md) |
| 0x184 | 2 | enum16 | A in | | source of the projectile's object input A: 0 none, 1 range remaining, 2 time remaining, 3 tracer |
| 0x186 | 2 | enum16 | B in | | same for B |
| 0x188 | 2 | enum16 | C in | | same for C |
| 0x18A | 2 | enum16 | D in | | same for D |
| 0x18C | 16 | tag ref `effe` | super detonation | | effect of a supercombine (behaviour 13) |
| 0x19C | 4 | f32 | AI perception radius | WU | how close AI notices it (AI only) |
| 0x1A0 | 4 | f32 | collision radius | WU | extra width against objects (behaviour 5); 0 = a line |
| 0x1A4 | 4 | f32 | arming time | s | a requested detonation waits until armed (behaviour 2) |
| 0x1A8 | 4 | f32 | danger radius | WU | AI flee radius (AI only) |
| 0x1AC | 16 | tag ref `effe` | effect | | the detonation effect (carries the area damage) |
| 0x1BC | 4 | f32 | timer lower bound | s | detonation timer |
| 0x1C0 | 4 | f32 | timer upper bound | s | |
| 0x1C4 | 4 | f32 | minimum velocity | WU/tick | after a collision response, slower than this detonates |
| 0x1C8 | 4 | f32 | maximum range | WU | travelled distance at which it detonates; 0 = none |
| 0x1CC | 4 | f32 | air gravity scale | | x GRAVITY |
| 0x1D0 | 4 | f32 | air damage range lower bound | WU | deceleration window, see behaviour 3 |
| 0x1D4 | 4 | f32 | air damage range upper bound | WU | |
| 0x1D8 | 4 | f32 | water gravity scale | | used while wholly under water |
| 0x1DC | 4 | f32 | water damage range lower bound | WU | |
| 0x1E0 | 4 | f32 | water damage range upper bound | WU | also used, in the air, by one disappearance test (Quirks) |
| 0x1E4 | 4 | f32 | initial velocity | WU/tick | speed added along the aim at creation |
| 0x1E8 | 4 | f32 | final velocity | WU/tick | speed after the deceleration window |
| 0x1EC | 4 | f32 | guided angular velocity | rad/s | turning rate toward the target |
| 0x1F0 | 2 | enum16 | detonation noise | | AI hearing, as impact noise |
| 0x1F2 | 2 | pad | | | |
| 0x1F4 | 16 | tag ref `effe` | detonation started | | played once when the timer first starts after a stop or a stick |
| 0x204 | 16 | tag ref `snd!` | flyby sound | | behaviour 17 |
| 0x214 | 16 | tag ref `jpt!` | attached detonation damage | | applied to the object it is stuck in when it detonates |
| 0x224 | 16 | tag ref `jpt!` | impact damage | | applied to an object it hits |
| 0x234 | 12 | pad | | | zero on the disc |
| 0x240 | 12 | block | material responses | | element 0xA0, one per material type in code order (0 dirt first) |

### Projectile flags (offset 0x17C)

| Bit | Name | Effect in the engine |
|---|---|---|
| 0 | oriented along velocity | the object's forward follows its velocity (presentation) |
| 1 | AI must use ballistic aiming | AI aims it with a gravity solution (behaviour 17) |
| 2 | detonation max time if attached | the timer starts from the lower bound; on sticking it is re-armed to the upper bound |
| 3 | has super combining explosion | needle supercombine (behaviour 13) |
| 4 | combine initial velocity with parent velocity | not read by the engine; the weapon always adds the shooter's along-aim speed (weapons.md) |
| 5 | random attached detonation time | not read by the engine |
| 6 | minimum unattached detonation time | not read by the engine |

Disc values: 0 on all but the frag grenade (2), the plasma grenade (6),
the needle (11 = bits 0, 1, 3) and the charged bolt (1).

### Material response element (0xA0)

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x00 | 2 | flags16 | flags | | bit 0 "cannot be overpenetrated"; not read by the engine; 0 on the disc |
| 0x02 | 2 | enum16 | default response | | response enumeration below |
| 0x04 | 16 | tag ref `effe` | default effect | | impact effect with the default response |
| 0x14 | 16 | pad | | | |
| 0x24 | 2 | enum16 | potential response | | 0 (disappear) means "no potential response" |
| 0x26 | 2 | flags16 | potential flags | | bit 0 only against units |
| 0x28 | 4 | fraction | skip fraction | | chance the potential response is skipped |
| 0x2C | 4 | angle | between angle lower bound | rad | impact angle window (0 grazing to pi/2 head-on) |
| 0x30 | 4 | angle | between angle upper bound | rad | 0 = no angle condition |
| 0x34 | 4 | f32 | and velocity lower bound | WU/tick | impact speed window (normal component) |
| 0x38 | 4 | f32 | and velocity upper bound | WU/tick | 0 = no velocity condition |
| 0x3C | 16 | tag ref `effe` | potential effect | | impact effect with the potential response |
| 0x4C | 16 | pad | | | |
| 0x5C | 2 | enum16 | scale effects by | | 0 damage, 1 angle (behaviour 17) |
| 0x5E | 2 | pad | | | |
| 0x60 | 4 | angle | angular noise | rad | added to the measured impact angle and turns the outgoing velocity |
| 0x64 | 4 | f32 | velocity noise | WU/tick | lowers the measured impact speed and changes the outgoing speed |
| 0x68 | 16 | tag ref `effe` | detonation effect | | extra effect when the projectile later detonates having hit this material |
| 0x78 | 24 | pad | | | |
| 0x90 | 4 | fraction | initial friction | | speed lost passing through an object |
| 0x94 | 4 | f32 | maximum distance | | not read by the engine (100 on some needle entries) |
| 0x98 | 4 | fraction | parallel friction | | fraction of the velocity component **along the surface normal** lost in a bounce |
| 0x9C | 4 | fraction | perpendicular friction | | fraction of the component **along the surface** lost in a bounce |

Response enumeration (default and potential): 0 disappear, 1 detonate,
2 reflect (bounce), 3 overpenetrate (pass through), 4 attach (stick).

A material type with no element (index at or beyond the block's count,
or the "none" type -1) uses the engine's built-in response: default
disappear, no potential response, no effects, all zeros. On the disc
every block has 33 elements except the flame's, which has 31 (ice and
hunter shield fall back to the built-in one).

## The damage effect tag

Class `jpt!`, struct 0x2A0 bytes. The first part is presentation and
radius, the damage part starts at 0x1C4. Every pad range listed is zero
on all 66 tags of Blood Gulch (checked).

### Radius, flags and presentation

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 4 | f32 | radius lower bound | WU | falloff radius: full scale inside it |
| 0x004 | 4 | f32 | radius upper bound | WU | cutoff radius: nothing at or beyond it |
| 0x008 | 4 | f32 | cutoff scale | | not read by the engine for damage (0 on every disc tag) |
| 0x00C | 4 | flags32 | flags | | bit 0 do not scale damage by distance (set on the 11 weapon melee effects only) |
| 0x010 | 20 | pad | | | |
| 0x024 | 2 | enum16 | screen flash type | | 0 none, 1 lighten, 2 darken, 3 max, 4 min, 5 invert, 6 tint (names from general knowledge of the tag) |
| 0x026 | 2 | enum16 | screen flash priority | | 0 low, 1 medium, 2 high |
| 0x028 | 12 | pad | | | |
| 0x034 | 4 | f32 | screen flash duration | s | |
| 0x038 | 2 | enum16 | screen flash fade function | | transition function (objects.md: 0 linear ... 5 cosine) |
| 0x03A | 2 | pad | | | |
| 0x03C | 8 | pad | | | |
| 0x044 | 4 | fraction | screen flash maximum intensity | | |
| 0x048 | 4 | fraction | screen flash zero scale factor | | intensity at scale 0 (behaviour 17) |
| 0x04C | 16 | real_argb_color | screen flash color | | |
| 0x05C | 4 | fraction | low frequency vibrate frequency | | controller rumble, left motor |
| 0x060 | 4 | f32 | low frequency vibrate duration | s | |
| 0x064 | 2 | enum16 | low frequency vibrate fade function | | transition function |
| 0x066 | 10 | pad | | | |
| 0x070 | 4 | fraction | high frequency vibrate frequency | | right motor |
| 0x074 | 4 | f32 | high frequency vibrate duration | s | |
| 0x078 | 2 | enum16 | high frequency vibrate fade function | | |
| 0x07A | 10 | pad | | | |
| 0x084 | 4 | fraction | vibrate zero scale factor | | |
| 0x088 | 16 | pad | | | |
| 0x098 | 4 | f32 | temporary camera impulse duration | s | |
| 0x09C | 2 | enum16 | temporary camera impulse fade function | | |
| 0x09E | 2 | pad | | | |
| 0x0A0 | 4 | angle | temporary camera impulse rotation | rad | |
| 0x0A4 | 4 | f32 | temporary camera impulse pushback | WU | |
| 0x0A8 | 8 | real_bounds | temporary camera impulse jitter | WU | |
| 0x0B0 | 4 | fraction | temporary camera impulse zero scale factor | | |
| 0x0B4 | 4 | pad | | | |
| 0x0B8 | 4 | angle | permanent camera impulse angle | rad | |
| 0x0BC | 4 | fraction | permanent camera impulse zero scale factor | | |
| 0x0C0 | 12 | pad | | | |
| 0x0CC | 4 | f32 | camera shaking duration | s | |
| 0x0D0 | 2 | enum16 | camera shaking falloff function | | transition function |
| 0x0D2 | 2 | pad | | | |
| 0x0D4 | 4 | f32 | camera shaking random translation | WU | |
| 0x0D8 | 4 | angle | camera shaking random rotation | rad | |
| 0x0DC | 12 | pad | | | |
| 0x0E8 | 2 | enum16 | camera shaking wobble function | | periodic function (objects.md) |
| 0x0EA | 2 | pad | | | |
| 0x0EC | 4 | f32 | camera shaking wobble period | s | 30 on most disc tags |
| 0x0F0 | 4 | fraction | camera shaking wobble weight | | |
| 0x0F4 | 4 | fraction | camera shaking zero scale factor | | |
| 0x0F8 | 28 | pad | | | |
| 0x114 | 16 | tag ref `snd!` | sound | | played unspatialised for a damaged local player |
| 0x124 | 112 | pad | | | |
| 0x194 | 4 | f32 | breaking effect forward velocity | WU/s | shards of a broken surface (breakable surfaces' spec) |
| 0x198 | 4 | f32 | breaking effect forward radius | WU | |
| 0x19C | 4 | f32 | breaking effect forward exponent | | |
| 0x1A0 | 12 | pad | | | |
| 0x1AC | 4 | f32 | breaking effect outward velocity | WU/s | |
| 0x1B0 | 4 | f32 | breaking effect outward radius | WU | |
| 0x1B4 | 4 | f32 | breaking effect outward exponent | | |
| 0x1B8 | 12 | pad | | | |

### Damage part

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x1C4 | 2 | enum16 | side effect | | 0 none, 1 harmless, 2 lethal to the unsuspecting, 3 emp |
| 0x1C6 | 2 | enum16 | category | | 0 none, 1 falling, 2 bullet, 3 grenade, 4 high explosive, 5 sniper, 6 melee, 7 flame, 8 mounted weapon, 9 vehicle, 10 plasma, 11 needle, 12 shotgun |
| 0x1C8 | 4 | flags32 | flags | | see Damage flags |
| 0x1CC | 4 | f32 | area of effect core radius | WU | width of the occlusion probes for units (behaviour 10) |
| 0x1D0 | 4 | f32 | damage lower bound | points | the amount at scale 0 ("minimum") |
| 0x1D4 | 8 | real_bounds | damage upper bound | points | the random amount at scale 1: U(lower, upper) |
| 0x1DC | 4 | fraction | vehicle passthrough penalty | | share withheld from a vehicle's riders on a direct hit on the vehicle (health_shields.md) |
| 0x1E0 | 4 | fraction | active camouflage damage | | taken off a camouflaged unit's camouflage per hit |
| 0x1E4 | 4 | fraction | stun | | added to a player's stun per hit, times the event scale |
| 0x1E8 | 4 | fraction | maximum stun | | the stun is not raised above this times the scale |
| 0x1EC | 4 | f32 | stun time | s | added to the player's stun ticks |
| 0x1F0 | 4 | pad | | | |
| 0x1F4 | 4 | f32 | instantaneous acceleration | WU/s | the push (behaviour 11) |
| 0x1F8 | 8 | pad | | | zero on the disc |
| 0x200 | 160 | f32 x 40 | material modifiers | factor | slot t multiplies damage to material type t (types 0 to 32; 33 to 39 zero and unused) |

**Damage flags (0x1C8).**

| Bit | Name | Effect |
|---|---|---|
| 0 | does not hurt owner | area damage skips the owner object |
| 1 | can cause headshots | a hit with body damage on a material flagged "head" kills outright when a multiplayer game is running or the victim is not a player's biped (health_shields.md) |
| 2 | pings resistant units | animation only (units.md / animations) |
| 3 | does not hurt friends | area damage skips objects of a team that is not the owner's enemy |
| 4 | does not ping units | animation only |
| 5 | detonates explosives | doubles the push on vehicles; lets items set off (single player only); hurts objects that only explosives hurt |
| 6 | only hurts shields | no body damage |
| 7 | causes flaming death | the victim burns when killed |
| 8 | damage indicators always point down | HUD |
| 9 | skips shields | the shield is ignored |
| 10 | does not hurt infection forms | campaign only |
| 11 | can cause multiplayer headshots | in a multiplayer game, body damage to a head material is doubled |
| 12 | infection form pop | campaign only |

Disc values: melee effects 0x5; pistol and sniper bullets 0x2; plasma
rifle bolt 0x804; plasma pistol bolt, charged bolt and ghost bolt 0x4;
frag, plasma grenade, rocket, tank shell and needler explosions 0x20;
flame 0x80; all others 0.

## Behaviour

### 1. Creation

Whoever creates the projectile (a weapon: weapons.md; a grenade throw:
behaviour 15) supplies its position, its forward direction d (unit),
an up vector, an owner object (the shooter) and a starting velocity
v_start (the weapon passes d x (d . shooter's velocity), the inherited
along-aim speed). Then:

1. velocity = v_start + d x initial velocity;
2. the shooter is remembered as the object to ignore: the ultimate
   parent of the owner (the vehicle if the shooter sits in one, else the
   shooter itself);
3. detonation timer step = 1 / (T x 30) if T x 30 >= 1, else 0 (no
   timer), where T = timer lower bound when flag 2 is set, else
   U(timer lower bound, timer upper bound); the timer itself starts at 0;
4. arming step = 1 / (arming time x 30) if that is >= 1, else 0 (no
   arming); arming starts at 0;
5. "under water" = whether the position is wholly inside water (the
   scenario's media);
6. the deceleration state of behaviour 3 is set for air or water;
7. the tracer flag is set (the weapon removes it from non-tracer
   rounds; behaviour 17);
8. the target is none (the weapon then sets its aim assist target,
   which guided projectiles follow);
9. no hit material yet, no action requested, not stopped, not stuck.

A player's projectile is also pushed back inside the level from the
camera position (weapons.md) and marked never to be garbage-collected.
Every projectile is deleted, not deactivated, when its cluster stops
being active (no player's potentially visible set reaches it:
objects.md, structure_bsp.md).

### 2. Timers

Three accumulators advance at the **start** of every update, before any
movement, each by its step, stored as 32-bit floats (the counts below
are exact for that precision):

- **arming**: += arming step.
- **deceleration timer**: += deceleration step (behaviour 3).
- **detonation timer**: += detonation step, only while it is running.
  It runs if the timer-starts value is 0; or, for values 1 and 2, once
  the projectile has stopped after a collision (behaviour 7); or once
  it has run before; or while the projectile is stuck. **Values 1 and
  2 behave the same**: the "after first bounce" case also waits for a
  stop (checked in the disc's executable: both values test the same
  "stopped" state). When the timer reaches 1, a detonation is
  requested.

Requests are an ordered priority: none < detonate < disappear; a later
request only raises it. A **detonation request waits for arming**: if
the projectile has an arming step and arming is below 1, it keeps
flying and bouncing (the movement loop of behaviour 3 still runs) and
detonates at the end of the first update in which arming has reached
1. A "disappear" request removes it at the end of the update with no
detonation.

Counts at 32-bit precision: arming 1.5 s, step 1/45, reaches 1 on the
45th update; frag timer 0.5 s, step 1/15, on the 15th running update;
needle timer 0.75 s, step 1/22.5 = 0.044444446, on the 23rd; plasma
grenade 2 s, step 1/60 = 0.016666668, **on the 61st** (sixty additions
leave 0.99999994).

### 3. Flight

Per update the projectile has one tick of time, `r = 1`, to spend. It
moves in up to 10 segments: each segment either reaches the end of the
tick or stops at a surface, which spends a fraction of r and leaves the
rest for the next segment. A segment is computed only while no request
is pending (or a detonation request is waiting for arming), the
projectile is not stuck, not at rest and has no parent object.

**Deceleration state** (set at creation and whenever it enters or
leaves water; "lower" and "upper" are the air or water damage range
bounds, v0 the initial and vf the final velocity):

- a = (v0^2 - vf^2) / (2 (upper - lower)) WU/tick^2 if v0 != vf and
  upper != lower, else a = 0;
- if lower > 0: deceleration timer = 0 and its step = **lower / v0**;
  otherwise the timer = 1 (deceleration active at once) and step 0.

The step is a distance divided by a speed (checked in the disc's
executable: the engine divides the lower bound by the initial
velocity), not the speed over the distance it plainly meant: on the
disc every projectile with lower > 0 except the shotgun pellet has
lower >= v0 and so decelerates from its first update; the pellet
(lower 1.5, v0 4.667) starts decelerating on update 4 (0.3214 x 4 >= 1)
instead of after 1.5 WU. Keep this.

**One segment**, with v the velocity at its start and s = |v|:

1. Guidance (behaviour 12) may turn the new velocity v' (v' = v
   otherwise).
2. If the deceleration timer is >= 1:
   - if s > vf and a != 0: s' = s - a r. If s' > vf: v' is scaled to
     s' and the segment's average velocity is (v' + v) / 2, its average
     speed s - a r / 2. Otherwise (it would pass vf this segment): with
     f = (s - vf) / (a r), the new speed is s' = 0.99 vf, v' is scaled
     to it, the average speed is (s' + s) f / 2 + (1 - f) vf and the
     average velocity (v' + v) f / 2 + (1 - f) v';
   - else, if maximum range = 0 and timer upper bound = 0 and minimum
     velocity <= vf and (a != 0 or travelled distance >= the water
     damage range upper bound): **request disappear** (this is how a
     plasma rifle bolt or a Warthog bullet ends once slowed);
   - else, if 0 < s < vf: v' is scaled to 0.99 vf.
   With no deceleration the average velocity is v and the average speed
   s.
3. Gravity: g = GRAVITY x (water gravity scale if under water, else air
   gravity scale); v'.z -= g r; average velocity z -= g r / 2.
4. Range: if maximum range != 0 and average speed x r + travelled >
   maximum range, the segment is cut to the fraction q = (maximum range
   - travelled) / average speed x r (q = 0 if the average speed is 0)
   and a **detonation** is requested. Otherwise q = 1. (Range is
   measured with the average speed, before gravity's share; the
   odometer adds the real length moved.)
5. The end point is P_end = P + average velocity x q r.
6. Unless 10 surfaces were already met this update or a disappearance
   is pending, the segment P to P_end is traced (behaviour 5). With no
   hit: r = 0 (the tick is used up); if the 10-surface limit stopped the
   trace, a detonation is requested.
7. With a hit at fraction t of the segment: r = 1 - t (what remains);
   v'.z += g r (gravity is taken back for the unused part); if the
   end-of-segment speed s' != 0, v' is scaled by min(r a + s', s) / s'
   (the deceleration is taken back for the unused part, never above the
   starting speed); if the surface's normal z > 0.3 the "bounced" state
   is set; the ignored object is cleared; the response of behaviours
   6 and 7 runs with the hit point as the new position and v' as the
   velocity; the AI is told of an impact; a stick ends the movement.
8. If the projectile moved: travelled += |P_new - P|; the flyby sound
   may play (behaviour 17); the orientation is updated; the position
   becomes P_new and the velocity v'.

The ordering means a projectile's **damage scale (behaviour 8) uses
the speed at the moment of impact**, `s - a t` on a decelerating
segment, clamped to the starting speed.

**Lifetime.** Nothing else limits a projectile's life: it ends by
detonating (timer, range, response, minimum velocity, the collision
limit), by disappearing (response, the slow-down rule above), or by
being deleted with its parent or when its part of the level is
deactivated.

**Water.** Passing through a water surface (an overpenetrate response
to a media hit) toggles "under water", recomputes the deceleration
state with the water bounds (the step is then lower / v0 of the water
window; the timer is not reset), and moves the point 0.001 WU through
the surface. In the air, the disappearance test of step 2 compares the
travelled distance with the **water** upper bound (kept from the
original; on the disc it never matters: every projectile it applies to
has a != 0).

### 4. Hitscan versus slow projectiles

There is no separate hitscan path. The object update of a tick first
updates every existing object; an object created during that pass (a
projectile fired by a weapon held by a unit being updated) is skipped
by it and **receives its first update at the end of the same tick's
object update**. So a shot fired on tick N has moved one full tick of
velocity by the end of tick N and can hit anything within |v| WU of the
muzzle on tick N itself; its k-th update happens on tick N + k - 1.

A bullet's trace in one update covers its whole per-tick speed:

| Projectile | WU per update | Hit at 10 m (3.28 WU) | 50 m (16.4 WU) | 100 m (32.8 WU) |
|---|---|---|---|---|
| sniper bullet | 33.33 | update 1 | update 1 | update 1 |
| pistol bullet | 10.0 | 1 | 2 | 4 |
| assault rifle bullet | 10.8 | 1 | 2 | 4 |
| Warthog bullet | 9.77, slowing | 1 | 2 | 4 |
| shotgun pellet | 4.67 then 3.3 | 1 | 4 | 9 |
| tank shell | 3.33 | 1 | 5 | 10 |
| plasma rifle bolt | 1.67, slowing | 3 | 12 | never (gone at 30.8 WU) |
| ghost bolt | 1.67, slowing | 3 | 12 | 28 |
| plasma pistol bolt | 0.83, slowing | 5 | 22 | 51 |
| charged bolt | 0.5, slowing | 7 | 43 | 98 |
| rocket | 0.4, slowing | 9 | 42 | 85 |
| needle | 0.133 | 25 | 124 | never (range 20 WU) |

(Fired level along +X with nothing below it; the delay in ticks is
the update number minus 1.) An implementation
may treat a bullet as an instant trace only if it reproduces this
exactly: on the firing tick, trace min(|v|, remaining range) WU; then
continue the next tick from the end point. The difference matters for
targets beyond 10 WU (pistol, rifle) and for anything that moves or
drops in the meantime: gravity bends even the pistol bullet by 0.0036
WU per tick squared.

### 5. The per-tick trace

The segment P to P_end is traced (collision.md) with: front-facing
surfaces only, invisible surfaces ignored, the level, water surfaces
(media) and **every object type** (bipeds, vehicles, weapons, equipment,
garbage, projectiles, scenery, devices) through their collision models,
keeping the hit location valid. Objects without a collision model (all
projectiles on the disc) cannot be hit. Excluded: the ignored object
(the shooter's ultimate parent until the first collision of any kind;
after an overpenetration, the object just passed) and invisible
objects. Dead bipeds are hit (their model is still there). The nearest
hit wins.

If nothing is hit and the collision radius is at least 0.0001, two
more traces are made against **objects only** (not the level), from
P +- R L to P_end +- R L, where R is the radius and L = normalise(Z x
(P_end - P)), or +Y when the segment is vertical: a horizontal widening
only. The first of the two that hits (left first) is used.

What a hit gives: the kind (level surface, water, object), the
fraction t, the point, the surface plane (normal n, flipped as the
surface says; for objects transformed to world space), the material
type (level: the collision material's type, -1 for material-less
surfaces; water: 28 or the fog plane's type; object: the type of the
model material the surface uses), and for objects the object, its
collision node, its region and the model material index; for level
surfaces the surface and breakable flags.

### 6. Choosing a material response

At a hit, with v' the velocity at impact (behaviour 3 step 7):

1. If the hit is an object and the projectile has an impact damage,
   the direct-hit damage event is applied first (behaviour 8). The
   hit material then becomes the material the damage reports: the
   **shield's material type** when the shield took more of this hit than
   the body (22 for a Spartan with shields up), else the model
   material's type; unchanged if the event did no damage.
2. The hit material is remembered for detonation (behaviour 9) and
   selects the response element (or the built-in one, see the tag).
3. Impact speed u = -(v' . n) + U(-velocity noise, 0); impact angle
   alpha = angle(v', n) - pi/2 + U(-angular noise, +angular noise), so
   alpha is 0 for a grazing hit and pi/2 for a head-on one.
4. The **potential response** is used when all hold: it is not 0;
   the angle upper bound is 0 or lower <= alpha <= upper; the velocity
   upper bound is 0 or lower <= u <= upper; "only against units" is
   clear or the hit object is a unit (biped or vehicle); and a fresh
   U(0, 1) >= skip fraction. Otherwise the **default response**. The
   matching effect goes with it.
5. A hit on a breakable level surface also sends the impact damage to
   that surface (structure_bsp.md / breakable surfaces).

### 7. The responses

The new position is the hit point. Then, by response:

- **Overpenetrate** first resolves to something else: a water hit
  toggles "under water" (behaviour 3) and continues; an object hit
  multiplies the velocity by (1 - initial friction) and makes that
  object the ignored one, then continues; a level hit becomes an
  **attach** (with "bounced" and "stopped" set) if the projectile has a
  timer (upper bound != 0), else a **detonate**.
- **Reflect**: with the velocity split into N = (v' . n) n (along the
  normal) and T = v' - N (along the surface):
  v'' = (1 - perpendicular friction) T - (1 - parallel friction) N.
  Stone under a frag (0.7, 0.8): the normal speed is reversed and kept
  at 30 %, the sliding speed kept at 20 %.
- Every response other than reflect and overpenetrate sets the velocity
  to zero.
- Angular noise != 0: the velocity is turned about a random axis
  perpendicular to it by U(0, angular noise) (its length unchanged).
- Velocity noise != 0 and the velocity is not zero: its length becomes
  |v''| + U(-velocity noise, +velocity noise).
- Not attach, and |v''|^2 < minimum velocity^2: **detonate** requested.
- |v''|^2 < 0.0001: the projectile is **stopped** (which starts timers
  of kinds 1 and 2); if also n.z > 0.3 it is **at rest** (it stops
  moving for good unless pushed) and, if it has no timer (upper bound
  0), a detonation is requested.
- The impact effect (behaviour 17) plays if u > 1/120; the
  detonation-started effect plays if the timer has never run and the
  projectile is now stopped or sticking.
- **Disappear** requests disappearance; **detonate** requests
  detonation; reflect and overpenetrate continue the movement with the
  remaining r.
- **Attach**: (supercombine bookkeeping, behaviour 13); velocity and
  spin zero; stuck and at rest; placed at the hit point; if the hit is
  an object, attached to it at the hit collision node (it then moves
  with that node); if flag 2 is set the timer step is re-armed to 1 /
  (upper bound x 30). A level hit just leaves it fixed in the world.
  The detonation timer runs while stuck.

A projectile stuck in an object:

- moves with the node it is attached to, every tick, and is updated
  right after its parent (its timer keeps running, it does not move by
  itself);
- when the parent object is **destroyed** (blown apart, not merely
  killed: a vehicle exploding, a body past its destroyed threshold),
  it is detached with arming and timer set to 1, and so detonates on
  its next update where it is;
- when the parent merely dies (a killed Spartan) it stays stuck in the
  body and detonates on its own timer.

### 8. The damage of a direct hit

When a projectile hits an object and has an impact damage effect, it
applies one damage event (behaviour 18) to that object with:

- scale s = clamp((speed at impact - vf) / (v0 - vf), 0, 1) when v0 !=
  vf, else s = 1; the speed at impact is |v'| of behaviour 3 step 7;
- direction = v' normalised; epicenter and origin = the hit point;
- the hit node, region, model material index and surface normal;
- owner object, player and team = the projectile's owner;
- flag "from a weapon".

The event's amount (behaviour 18): **amount = (1 - s) x damage lower
bound + s x U(upper bound pair)**. So a projectile's damage falls off
linearly with its lost speed: full random damage while at initial
speed, the damage effect's "lower bound" once at final speed. Distance
enters only through speed: pistol, rifle and sniper bullets (v0 = vf)
do the same damage at any range until they vanish.

Headshots are decided on the target's side (health_shields.md); the
projectile side supplies the damage effect's flags 1 and 11, its
category, and the hit's model material (whose "head" flag the Spartan's
head material has).

### 9. Detonation

At the end of an update with a detonation request (and armed), in
order:

1. Supercombine check for stuck needles (behaviour 13), which may swap
   the effect and move the projectile.
2. The tracer contrail is told the projectile ended (presentation).
3. The detonation effect (or the super detonation) is created where
   the projectile is, facing its forward, with a "gravity" marker
   pointing down, owned by the projectile's **owner** (the shooter).
   Its damage parts are area damage events (behaviour 10) at the
   effect's location; on the disc every such part has zero delay.
4. If still attached to an object and it has an attached detonation
   damage: one damage event on that object (not area: no falloff, the
   collision model's indirect damage material), direction = the
   projectile's forward, epicenter = its position, scale 1, flag "from a
   weapon", owner = the projectile's owner.
5. If a hit material was recorded, that material response's detonation
   effect is created at the same place.
6. The AI is told of a detonation (detonation noise).
7. The projectile is deleted.

A projectile that detonates at maximum range in the air has no hit
material: bullets have no detonation effect, so they just vanish;
rockets, the flame and the tank shell explode there.

### 10. Area damage

An area damage event (from an effect's damage part) has an epicenter
E, the owner (taken from the effect's owner object: its owner player
and team) and a scale (the effect's; replaced below unless the damage
effect has "do not scale damage by distance"). The candidates are the
objects whose bounding sphere intersects the sphere of the **cutoff
radius** (radius upper bound) around E: at most 64, of any type. For
each candidate, with C its bounding-sphere centre (objects.md: the
root node plus the bounding offset; a biped's pelvis):

1. Visibility (traces against front-facing level surfaces and vehicles,
   scenery and machines, ignoring the candidate's ultimate parent):
   - a **unit** with core radius > 0.0001: let w = C - E,
     p = normalise(perp(w)), c = normalise(w x p), where perp(a) is
     (0, a.z, -a.y) if |a.x| is the smallest component, else
     (-a.z, 0, a.x) if |a.y| <= |a.z|, else (a.y, -a.x, 0). For each of
     the four offsets +core p, -core p, +core c, -core c: trace from E
     along the offset; from the hit point (or the offset's end) trace to
     C. The candidate is visible if **any** of the four second traces is
     clear;
   - anything else: visible if the single trace E to C is clear;
   - an invisible (hidden) object is never damaged.
2. Excluded: the owner object when the damage effect does not hurt
   its owner; objects whose team is not the owner's enemy when it does
   not hurt friends.
3. Scale: with d = |C - E|, r0 = radius lower bound, r1 = radius upper
   bound: **s = clamp(1 - (d - r0) / (r1 - r0), 0, 1)** if r1 > r0,
   else 1. Direction = (C - E) / d (zero when d = 0).
4. If s > 0, a damage event with that scale and direction is applied
   to the candidate (flag "area of effect"; no node, region or
   material: the target uses its indirect material).
5. If the candidate's collision model passes area damage to its
   children (health_shields.md), the same is done for each of its
   children.

In one line: **amount(d) = 0 if d >= r1, else ((1 - s) x lower + s x
U(upper)) with s = clamp(1 - (d - r0)/(r1 - r0), 0, 1)**: full
random damage inside the falloff radius, falling linearly to the
"lower bound" at the cutoff radius, then none. Distance is measured to
the target's bounding-sphere centre, not its surface. The cutoff scale
field plays no part.

Breakable level surfaces near E are also damaged (structure_bsp.md).

**Owner and chain reactions.** None of the disc's explosions has "does
not hurt owner", so a player's own grenades and rockets hurt him. A
grenade in flight has no collision model: an explosion does no damage
to it, only pushes it (acceleration scale 2.0 on both grenades, behaviour
11). Items lying on the ground with "destroyed by explosions" are set
off only when no multiplayer game is running (objects.md). So in
multiplayer there are **no chain reactions**: one grenade never sets
off another.

### 11. The push (instantaneous acceleration)

Every damage event that does any damage (amount > 0 before material
modifiers) to an object whose acceleration scale (object base 0x020)
exceeds 0.0001 pushes it, after the shield and body damage:

- direction D = normalise(event direction + (0, 0, 0.45));
- magnitude m = instantaneous acceleration x acceleration scale / 30
  WU per tick;
- Delta v = m D, applied by object type:
  - biped (not dead, not a "special" unit): **half** of Delta v is
    added to its velocity; it becomes airborne (movement.md) and
    "slipping"; a dead biped takes the whole Delta v and also gets a
    random spin (ragdoll: physics);
  - vehicle: Delta v, **doubled** when the damage effect detonates
    explosives, handed to the vehicle's physics (vehicles.md);
  - weapon, equipment, garbage: handed to the item's physics (objects.md);
  - projectile (not stuck): Delta v is added to its velocity, plus a
    random spin of |Delta v| x U(0, 1) x pi/2 about a random axis; it
    is no longer at rest.

The push does **not** fall off with distance: anything inside the
cutoff radius that takes damage gets the same push. It does depend on
the direction, which is from the epicenter to the target's centre with
an upward bias: a blast at a Spartan's feet throws him straight up.

Multiplayer Spartan (acceleration scale 0.7, alive): frag and plasma
grenade (4.0): 0.046667 WU/tick (1.40 WU/s); rocket and tank shell
(6.0): 0.07 WU/tick (2.10 WU/s); needler explosion (2.0): 0.023333
WU/tick. For scale, a jump is 0.07 WU/tick upward (movement.md).
Warthog (0.3) from a rocket: 6 x 0.3 / 30 x 2 = 0.12 WU/tick; Ghost
(0.3) the same; Scorpion (0.07): 0.028.

### 12. Guidance

A projectile with a target object and a guided angular velocity > 0
turns once per segment (so normally once per update) before moving:

1. omega = guided angular velocity / 30 rad; if the target is a unit
   controlled by a player, omega x= the globals' "guidance vs player"
   value at the current difficulty (0.5 at easy and normal, 0.75
   heroic, 1.0 legendary on the disc).
2. Target point T = the target's "body" marker (for any object: its
   marker, or its origin if it has none), moved by a **wander** offset:
   with e = distance between the two bounding-sphere centres, the
   offset length is 1 WU if e > 10, (e - 2) / 8 if 2 < e <= 10, and 0
   if e <= 2; its direction has yaw pi - W1 x pi/2 and pitch W2 x 2 pi
   (direction from angles as cache_file.md), where W1 and W2 are the
   "wander" periodic function (objects.md) at times
   ((game tick + 3 k) mod 65536) / 90 and ((game tick + 7 k) mod 65536)
   / 90, k a stable per-projectile integer (any will do).
3. With w = T - P (from the projectile's position) and axis = v x w: if
   w . v > 0 (target ahead) and |axis| > 0, the velocity is rotated
   about normalise(axis) by **exactly omega** (not the smaller of omega
   and the remaining angle, so it overshoots and wobbles once aligned).
   A target behind it is not chased.

Guidance continues for the projectile's whole flight; it stops only
when the target object is deleted (the target becomes none). A dead
target is still followed.

### 13. Attaching and the needler's supercombine

When a projectile with flag 3 (the needle) sticks into an object, every
needle (same projectile tag) already stuck in that object and not yet
"consumed" has its timer and arming set back to 0. So all the needles
in a target detonate together, 0.75 s (23 updates) after the **last**
one stuck. (A "will super explode" mark is also set on the new needle
when six or more were counted; nothing reads it.)

When a stuck needle detonates and its parent is a biped that is AI or
in a running multiplayer game, count n = the needles of that tag stuck
in the parent and not consumed (including itself). If **n > 6** (seven
or more):

1. Walk the parent's attached objects in order; for each such needle,
   with the running count n: if n <= 6 it is marked consumed, its
   timer and arming multiplied by U(0, 1) each (so it detonates later,
   at a random time, normally); otherwise its timer and arming are set
   to 0; then n -= 1.
2. The detonating needle's effect becomes the super detonation; it is
   detached, moved to the parent's **origin** (a biped's feet), pushed
   back inside the level if that point is outside, and explodes there.
   Having been detached, it does not deal its own attached detonation
   damage.

Each needle that detonates normally while stuck deals its attached
detonation damage (10 points, bullet) to the parent, and plays the
small "needle detonate" effect, which has no damage part. The needle's
impact damage effect does nothing (all zero), so needles hurt only when
they detonate. Needles that stick into the level just detonate harmlessly
after 0.75 s. In single player against the player's biped there is no
supercombine.

Worked example against a full multiplayer Spartan (75 + 75), seven
needles: the super explosion (60, emp) strips the shield with no
excess, the six consumed needles then deal 60 to the body at random
later times and the reset one 10 more: 70 of 75 body points. A
supercombine of exactly seven does not kill by itself; it does with
any earlier damage.

### 14. The plasma pistol's charged bolt

The overcharged shot (`weapons\plasma rifle\charged bolt`, fired by the
plasma pistol's second trigger) is an ordinary projectile with:
guidance at 0.506145 rad/s (29 degrees per second; x 0.5 against
players at normal difficulty: 0.00844 rad per tick), toward the aim
assist target the weapon gives it; a 0.1 WU collision radius against
objects; speed 0.5 falling to 0.3 WU/tick over its first 15 WU (the
deceleration starts at once: 5 / 0.5 >= 1); range 40 WU; flag 0
(oriented along velocity). Its responses detonate on every material
(on all 33 elements the default response is disappear and the
potential response is an unconditional detonate, so the potential one
always wins; leaves: potential pass through), and its
detonation effect has no damage. Its impact damage
`weapons\plasma rifle\charged bolt` is 70 points (no falloff: lower =
upper = 70) with side effect **emp**: on a shielded target the shield
is emptied completely whatever the amount, any excess over what the
shield had going to the body (health_shields.md). No damage effect on
the disc has "only hurts shields". Modifiers: 1.0 on a Spartan's
shield, 0.6 on the cyborg body, 1.5 on an Elite's shield.

### 15. Grenades

| | Frag | Plasma |
|---|---|---|
| projectile | `weapons\frag grenade\frag grenade` | `weapons\plasma grenade\plasma grenade` |
| pickup item (eqip) | `weapons\frag grenade\frag grenade` | `weapons\plasma grenade\plasma grenade` |
| globals grenade entry | 0: maximum count 4 | 1: maximum count 4 |
| throwing effect | `weapons\frag grenade\effects\throw` | `weapons\plasma grenade\effects\throw` |
| speed at release | unit's grenade velocity: 10 WU/s = 0.3333 WU/tick (Spartan) | same |
| gravity | 1.0 | 1.0 |
| arming | 1.5 s | none |
| timer | 0.5 s, starts when at rest | 2.0 s (flag 2: re-armed to the upper bound, 2.0 s, on sticking), starts when at rest or stuck |
| on most surfaces | bounces (behaviour 7) | bounces, losing 95 % of its sliding and 90 % of its normal speed |
| on a unit (biped or vehicle) | bounces (no impact damage) | **sticks** (potential attach, "only against units") on every material but glass, force field, jackal shield, engineer force field, monitor, water and hunter shield |
| explosion | `weapons\frag grenade\explosion` + `weapons\frag grenade\shock wave` | `weapons\plasma grenade\explosion` + `weapons\plasma grenade\shock wave` |
| stuck damage | none | `weapons\plasma grenade\attached` (120, emp) to the object it is in |

**The throw.** The unit's throw animation and its timing belong to
weapons.md and animations.md; the grenade's part:

1. When the throw animation reaches frame 2, one grenade of the
   current type is taken from the unit's count (not for infinite
   grenades) and the grenade projectile is created at the unit's "left
   hand" marker, forward = the unit's aim, attached to the hand's node.
   From this moment its arming and its timer (if it runs) count; it
   does not move by itself.
2. At the animation's key frame it is released: detached; for a
   player's unit the position becomes the camera position + i F + j L
   + k U with (i, j, k) the globals' player grenade origin (0, 0.05, 0
   on the disc), F the aim, L = normalise(Z x F) (or Z when F is
   vertical) and U = normalise(F x L): **0.05 WU to the left of the
   camera**; the velocity becomes exactly aim x grenade velocity / 30
   (**the thrower's own velocity is not added**), with a random spin of
   up to |change| x pi/2; it is pushed back inside the level from the
   camera position, and deleted if it cannot be.
3. A throw cut short (the throw animation interrupted, e.g. by a
   damage animation or death) releases with power p = ticks held /
   (key frame - starting frame + 1); if p < 1 the velocity is p x full
   + (1 - p) x aim x U(0.02, 0.046667) WU/tick.
4. An AI's throw uses its own ballistic aim (behaviour 17).

There is no cooking: holding the button changes nothing, the release
is at the key frame, and the frag's fuse starts only once it lies still.
A frag that comes to rest within 1.5 s of appearing in the hand waits
until then; one still rolling keeps rolling. On a flat stone floor a
frag stops within two or three bounces (test cases).

**Counts.** Each type up to the globals' maximum count (4 each on the
disc). Picking up a grenade item adds one if the unit has fewer than
the maximum, else the item is left. Spawn counts and their reduction
with player count belong to multiplayer_rules.md and the scenario's
starting equipment (scenario.md): in the reference the spawn gives the
maximum of frags (2 with 5 or more players, 1 with 9 or more) and no
plasma grenades unless the game type or the starting equipment says
otherwise (Open questions).

A thrown grenade is owned by the thrower: kills are his, his own
grenade hurts him.

### 16. The other projectiles

| Projectile | Fired by | Flight | Ends | Explosion |
|---|---|---|---|---|
| `weapons\rocket launcher\rocket` | rocket launcher | 0.4 slowing to 0.3333 WU/tick over 100 WU (36.6 to 30.5 m/s), no gravity, 0.1 WU wide against objects | detonates on any surface (leaves: passes through, 10 % detonates) and at 128 WU (update 358) | `weapons\rocket launcher\explosion` + frag shock wave |
| `weapons\flamethrower\flame` | flamethrower and gravity rifle (neither is placed on a stock map) | 0.3333 WU/tick, no gravity, 0.1 WU wide | disappears on any surface (no damage on contact); detonates at 2.5 WU (update 8) | `weapons\flamethrower\explosion`: 10 points within 0.4 WU, flame, causes flaming death |
| `vehicles\scorpion\tank shell` | Scorpion cannon, trigger 0 | 3.3333 WU/tick (304.8 m/s), no gravity | detonates on any surface and at 1000 WU | `vehicles\scorpion\shell explosion` + frag shock wave |
| `vehicles\scorpion\bullet` | Scorpion cannon, trigger 1 | as the assault rifle bullet | | |
| `vehicles\warthog\bullet` | Warthog gun | 9.7667 slowing to 0.3333 WU/tick over its first 100 WU (decelerates from update 1), gravity 1 | disappears once at final speed (about 100 WU, update 21) | |
| `vehicles\ghost\ghost bolt` | Ghost gun | as the plasma rifle bolt, but range 50 WU: after slowing (30.8 WU) it continues at 0.825 WU/tick to 50 WU | | |

The flame's projectile is a short-lived blob that explodes at the end
of its 2.5 WU, damaging within 0.4 WU of that point; a flamethrower's
stream is many of them (weapons.md for the rate).

### 17. Presentation hooks

- **Tracers.** If the projectile has a contrail attachment, the first
  one is its tracer. A projectile that is not a tracer deletes it at
  its first update (weapons.md decides which rounds are tracers).
- **Impact effect.** At each hit with impact speed above 1/120 WU/tick,
  the chosen response's effect plays at the hit point (attached to the
  object at the hit node for object hits) with markers "normal" (n),
  "incident" (-v'), "negative incident" (v'), "reflection" (v'
  reflected about n), "gravity" (down); its scale is the damage scale
  of behaviour 8 (scale-by 0) or alpha x 2 / pi (scale-by 1), clamped to
  [0, 1]; a second scale is the struck object's remaining shield (if the
  shield took the hit) or body vitality. effects.md owns effects.
- **Flyby.** Once per update at most: for each local player's unit
  other than the ignored object, if the listener's centre projects onto
  this segment's movement within the segment and lies closer to it than
  the flyby sound's maximum distance, the flyby sound plays at the
  listener's position mirrored across the path, once.
- **Orientation.** With flag 0 the forward is set to the velocity at
  the start of the movement (so it lags one segment) and the up vector
  re-orthogonalised and rolled by the spin; otherwise a spinning
  projectile (a pushed grenade) tumbles about its spin axis by |spin|
  rad per segment.
- **Function inputs** A to D: 1 "range remaining" = travelled /
  maximum range (it grows; 0 without a range), 2 "time remaining" = the
  detonation timer (0 to 1), 3 tracer = 1 or 0. None is used on the
  disc.
- **AI.** Impact and detonation noises (heard by AI); perception and
  danger radii; ballistic aiming (flag 1): with gravity a = GRAVITY x
  air gravity scale, the aim solves a^2 t^4 / 4 - (v^2 - a h) t^2 + |D|^2
  = 0 for the flight time t (D the offset to the target, h its height
  difference, v the speed; the shorter or "lob" root) and aims at
  D / t + (0, 0, a t / 2). Not needed for players.
- **Screen and controller.** A damage event on a player's unit starts
  the damage effect's screen flash, controller vibration, camera
  impulse and camera shake for that player with intensity z + (1 - z)
  s (z the zero scale factor, s the event's scale), plays the damage
  effect's sound, and (if any damage was done and there is an owner)
  lights the HUD's damage indicator toward the attacker or down (flag
  8). The "shock wave" damage effects of explosions do no damage (all
  zero) and exist for this: they shake the camera of players within
  their larger radius (3.5 to 8 WU for grenades). effects.md and
  hud.md own the rest.

### 18. The damage event (the seam with health_shields.md)

A damage event is what this spec hands to the damage system. It
carries:

| Field | Meaning |
|---|---|
| damage effect | the `jpt!` tag |
| scale s | 0 to 1: speed-based for direct hits, distance-based for area damage, 1 for attached damage, the falling fraction for falling |
| multiplier m | 1, except inside the vehicle pass-through to riders (health_shields.md) |
| flags | area of effect; from a weapon; bypasses shields; kill instantly; no statistics; ... |
| owner | object, player and team that get the credit and decide "owner" and "friend" |
| epicenter, origin | where it happened |
| direction | unit vector, for the push and the damage indicator |
| location | the cluster, for effects |
| target detail | for a direct hit: the object, its collision node, region, model material index and the surface normal; none for area and attached damage |

**The amount** before the target's side:
amount = ((1 - s) x L + s x U(H1, H2)) x m x G, with L the damage lower
bound, (H1, H2) the upper bound pair, and G the multiplayer game's
multiplier (the game variant's health setting and player traits:
multiplayer_rules.md; 1 by default). In single player an AI attacker's
damage is adjusted by its actor and an enemy's by difficulty.

**The material modifier** for a damaged material type t is the damage
effect's slot t. health_shields.md decides which t: the shield's
material type for damage the shield absorbs, the hit model material's
(or the indirect material's) type for the body. Also on the target's
side: shield leak, shield and body multipliers of the model material,
headshots, emp, stun, active camouflage loss, death. This spec owns
the push (behaviour 11), which follows.

## What is on the disc

All values below were read from `bloodgulch.map` and are identical in
`hangemhigh.map`, `sidewinder.map` and `chillout.map` (all 16
projectiles and 66 damage effects compared field by field).

### Which weapon fires each projectile

| Projectile | Fired by (weapons.md) |
|---|---|
| `weapons\assault rifle\bullet` | assault rifle |
| `weapons\pistol\bullet` | pistol |
| `weapons\shotgun\pellet` | shotgun (15 per shot) |
| `weapons\sniper rifle\sniper bullet` | sniper rifle |
| `weapons\rocket launcher\rocket` | rocket launcher |
| `weapons\plasma pistol\bolt` | plasma pistol, trigger 0 |
| `weapons\plasma rifle\charged bolt` | plasma pistol, trigger 1 (overcharge) |
| `weapons\plasma rifle\bolt` | plasma rifle |
| `weapons\needler\needle` | needler |
| `weapons\flamethrower\flame` | flamethrower, gravity rifle |
| `weapons\frag grenade\frag grenade` | grenade throw, type 0 (globals) |
| `weapons\plasma grenade\plasma grenade` | grenade throw, type 1 (globals) |
| `vehicles\warthog\bullet` | Warthog gun |
| `vehicles\scorpion\tank shell` | Scorpion cannon, trigger 0 |
| `vehicles\scorpion\bullet` | Scorpion cannon, trigger 1 |
| `vehicles\ghost\ghost bolt` | Ghost gun |

### Flight numbers

Speeds in WU/tick (m/s in brackets); a = deceleration (WU/tick^2);
"decel. from" = first decelerating update. All have 0 for the AI
radii except the grenades (perception 8, danger 2.25), and 0 arming
except the frag.

| Projectile | Initial (m/s) | Final (m/s) | Air range | a | Decel. from | Max range | Gravity air / water | Coll. radius | Timer (s), starts | Guided (rad/s) | Min. vel. |
|---|---|---|---|---|---|---|---|---|---|---|---|
| AR bullet | 10.8 (987.6) | 10.8 | 5 - 15 | 0 | | 40 | 1 / 0.4 | 0 | none | 0 | 0 |
| pistol bullet | 10.000001 (914.4) | 10.000001 | 0 - 100 | 0 | | 40 | 1 / 0.4 | 0 | none | 0 | 0 |
| shotgun pellet | 4.666667 (426.7) | 3.3333335 (304.8) | 1.5 - 3 | 3.555556 | 4 | 40 | 1 / 0.4 | 0 | none | 0 | 0 |
| sniper bullet | 33.333336 (3048) | 33.333336 | 1000 - 1000 | 0 | | 1000 | 0 / 0 | 0 | none | 0 | 0 |
| rocket | 0.40000004 (36.6) | 0.33333334 (30.5) | 0 - 100 | 0.0002444446 | 1 | 128 | 0 / 0.4 | 0.1 | none | 0 | 0 |
| plasma pistol bolt | 0.8333334 (76.2) | 0.5 (45.7) | 20 - 50 | 0.007407409 | 1 | 50 | 0.1 / 0 | 0 | none | 0 | 0 |
| charged bolt | 0.5 (45.7) | 0.3 (27.4) | 5 - 20 | 0.005333333 | 1 | 40 | 0 / 0 | 0.1 | none | 0.5061455 | 0 |
| plasma rifle bolt | 1.6666667 (152.4) | 0.8333334 (76.2) | 20 - 50 | 0.03472222 | 1 | 0 | 0 / 0 | 0 | none | 0 | 0 |
| needle | 0.13333334 (12.2) | 0.13333334 | 0 - 100 | 0 | | 20 | 0 / 0 | 0 | 0.75, at rest/stuck | 1.5707964 | 0.06666667 |
| flame | 0.33333334 (30.5) | 0.33333334 | 0 - 0 | 0 | | 2.5 | 0 / 0 | 0.1 | none | 0 | 0 |
| frag grenade | 0 | 0 | 0 - 10 | 0 | | 0 | 1 / 1 | 0 | 0.5, at rest | 0 | 0 |
| plasma grenade | 0 | 0 | 0 - 10 | 0 | | 0 | 1 / 1 | 0 | 2.0, at rest/stuck | 0 | 0 |
| Warthog bullet | 9.766667 (893.1) | 0.33333334 (30.5) | 0 - 100 | 0.4763834 | 1 | 0 | 1 / 0.4 | 0 | none | 0 | 0 |
| tank shell | 3.3333335 (304.8) | 3.3333335 | 1000 - 1000 | 0 | | 1000 | 0 / 0 | 0 | none | 0 | 0 |
| Scorpion bullet | 10.8 (987.6) | 10.8 | 5 - 15 | 0 | | 40 | 1 / 0.4 | 0 | none | 0 | 0 |
| ghost bolt | 1.6666667 (152.4) | 0.8333334 (76.2) | 20 - 50 | 0.03472222 | 1 | 50 | 0 / 0 | 0 | none | 0 | 0 |

The speeds, ranges and the other columns are the stored single-precision
values, written as the shortest decimal that reads back as the same
value (so the sniper bullet's 33.333336 is the word 0x42055556, one
step above 1000/30 in single precision, and the needle's 0.13333334 is
0x3E088889); brackets give metres per second at 3.048 m per WU and 30
ticks per second. The deceleration a is not stored: it is the formula
of behaviour 3 evaluated on those values, shown to 7 digits (the last
digit depends on the order of the single-precision operations).

Water ranges: AR and Scorpion bullet 0 - 5; pistol, rocket, needle,
Warthog bullet, sniper, tank shell 0 - 10; pellet 0 - 1; plasma rifle and
ghost bolts 20 - 50; plasma pistol, charged bolt, flame 0 - 0; grenades
0 - 10. Timer-starts values: 1 for the AR, pistol, pellet, rocket,
sniper and Scorpion bullets and the frag; 2 for the needle and plasma
grenade; 0 for the rest (all equivalent where there is no timer).
Noises (impact, detonation): pistol 1, 0; pellet 1, 0; sniper 1, 0;
rocket 0, 2; tank shell 0, 2; frag 4, 2; plasma grenade 1, 2; needle
4, 1; others 0, 0.

References: impact damage = the damage effect of the same path
(`weapons\assault rifle\bullet`, `weapons\pistol\bullet`,
`weapons\shotgun\pellet`, `weapons\sniper rifle\sniper bullet`,
`weapons\plasma pistol\bolt`, `weapons\plasma rifle\charged bolt`,
`weapons\plasma rifle\bolt`, `vehicles\warthog\bullet`,
`vehicles\scorpion\bullet`, `vehicles\ghost\ghost bolt`) and
`weapons\needler\impact damage`; the rocket, flame, tank shell and
grenades have none. Attached detonation damage: needle
`weapons\needler\detonation damage`, plasma grenade
`weapons\plasma grenade\attached`. Detonation effect: rocket
`weapons\rocket launcher\effects\rocket explosion`, flame
`weapons\flamethrower\effects\flame detonation`, tank shell
`vehicles\scorpion\shell explosion`, frag
`weapons\frag grenade\effects\explosion`, plasma grenade
`weapons\plasma grenade\effects\explosion`, needle
`weapons\needler\effects\needle detonate` (no damage), charged bolt
`weapons\plasma rifle\effects\detonate` (no damage). Super detonation:
needle `weapons\needler\effects\explosion`. Detonation started: plasma
grenade `weapons\plasma grenade\effects\detonation start`. Flyby
sounds: bullets `sound\sfx\impulse\impacts\bullets_projectile` (AR,
pistol, sniper, Scorpion bullet), plasma `...\plasrif_projectile`
(plasma rifle, plasma pistol, charged bolt), needle
`...\needler_projectile`, tank shell `...\scorpion_projectile`.

Damage parts inside the detonation effects (event 0, zero delay):

| Effect | Damage effects |
|---|---|
| `weapons\frag grenade\effects\explosion` | `weapons\frag grenade\explosion`, `weapons\frag grenade\shock wave` |
| `weapons\plasma grenade\effects\explosion` | `weapons\plasma grenade\shock wave`, `weapons\plasma grenade\explosion` |
| `weapons\rocket launcher\effects\rocket explosion` | `weapons\rocket launcher\explosion`, `weapons\frag grenade\shock wave` |
| `vehicles\scorpion\shell explosion` | `vehicles\scorpion\shell explosion`, `weapons\frag grenade\shock wave` |
| `weapons\needler\effects\explosion` | `weapons\needler\shock wave`, `weapons\needler\explosion` |
| `weapons\flamethrower\effects\flame detonation` | `weapons\flamethrower\explosion` |

### Material responses

Summary by material for the surfaces that matter (default response;
"/ pot." gives the potential response and its condition; frictions as
parallel/perpendicular).

| Projectile | Dirt (0), stone (2) | Cyborg body (21) | Cyborg shield (22) | Metal hollow, thin, thick (5, 6, 7) | Exceptions |
|---|---|---|---|---|---|
| AR bullet, Scorpion bullet | disappear | pass through, keeps 30 % (initial friction 0.7) | disappear | disappear | every organic body (grunt, hunter skin, elite, jackal, engineer, human, armored human) passes at 30 %; flood, water, leaves pass at full speed, and glass for the AR bullet only; jackal and hunter shields reflect |
| pistol bullet | disappear | disappear | disappear | disappear | glass, leaves pass; hunter shield reflects |
| shotgun pellet | disappear | disappear | disappear | disappear | glass, water, leaves pass; jackal and hunter shields reflect |
| sniper bullet | disappear | pass through, full speed | disappear | disappear | every organic body (hunter armor excepted), the elite shield, glass, water and leaves pass at full speed; jackal and hunter shields reflect |
| rocket | detonate | detonate | detonate | detonate | leaves: pass, 10 % detonate (skip 0.9) |
| plasma pistol bolt, plasma rifle bolt, ghost bolt | disappear | disappear | disappear | disappear | hunter shield reflects; leaves pass (rifle and ghost bolt) |
| charged bolt | disappear / pot. detonate (no condition, skip 0) | same | same | same | every element (all 33) is default disappear with an unconditional potential detonate, so it detonates on any material; leaves: potential overpenetrate (pass) |
| needle | attach; stone: reflect if impact angle <= 0.4363 (25 deg, angle noise 0.2618) with 0.3/0.3 | attach | attach | attach / pot. reflect if angle <= 0.6109 (35 deg, noise 0.0873), 0.3/0.3 | glass reflect <= 15 deg; force field, jackal shield, engineer force field, hunter shield reflect; water, leaves pass |
| frag grenade | reflect: dirt 0.8/0.9, stone 0.7/0.8 | reflect 0.8/0.9 | reflect 0.8/0.9 | reflect 0.7/0.8 | snow 0.9/0.95; force field and carrier 0.6/0.7; jackal shield 0.8/0.8; engineer 0.5/0.7; ice 0.55/0.65; water: pass, pot. reflect 0.7/0.7 (skimming) if impact angle <= 0.5236 (30 deg) and 0.0333 <= u <= 33.3; leaves: reflect, pot. pass (skip 0.2) |
| plasma grenade | reflect 0.9/0.95 (pot. attach to units only) | reflect / **attach** (units) | reflect / **attach** | reflect / **attach** (vehicles) | glass, force field 0.7/0.8 no stick; jackal shield 0.8/0.8; engineer FF, monitor, hunter shield no stick; ice 0.65/0.75 sticks to units; water pass 0.5/0.5 |
| flame | disappear | disappear | disappear | disappear | ice and hunter shield: built-in (disappear) |
| Warthog bullet | disappear | pass through, keeps 30 % | disappear | hollow and thin pass, keeps 30 %; thick disappear | wood passes, keeps 30 %; jackal shield reflects |
| tank shell | detonate | detonate | detonate | detonate | |

Which column applies to a Spartan: a projectile with impact damage
reports the shield's type (22) while the shield takes the larger share
of the hit, the cyborg type (21) once it does not; grenades and needles
(no damage on impact) always report 21. So an assault rifle bullet that
strikes an unshielded Spartan passes through him at 30 % speed and can
hit someone behind at full damage (v0 = vf, so no loss); a sniper
bullet passes at full speed.

### Damage effects

L = damage lower bound (the minimum), H = upper bound pair. "Mods"
gives the material modifiers for cyborg (21; the Spartan's body and
head materials) and cyborg energy shield (22). Stun = (stun, maximum,
time s).

| Damage effect | Used by | Category | Side effect | Flags | L | H | Radius r0 - r1 | Core | Camo | Stun | Accel (WU/s) | Mods 21, 22 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `weapons\assault rifle\bullet` | AR impact | bullet | none | 0 | 10 | 10 - 10 | 0 | 0 | 0.25 | 0 | 0 | 1, 1 |
| `weapons\pistol\bullet` | pistol impact | bullet | none | headshots | 25 | 25 - 25 | 0 | 0 | 0.45 | 0 | 0 | 1.5, 1 |
| `weapons\shotgun\pellet` | pellet impact | shotgun | none | 0 | 8 | 18 - 25 | 0 | 0 | 0.18 | 0 | 0 | 1, 0.5 |
| `weapons\sniper rifle\sniper bullet` | sniper impact | sniper | none | headshots | 101 | 101 - 101 | 0 | 0 | 1 | 0 | 0 | 1, 1 (passthrough 0.5) |
| `weapons\plasma rifle\bolt` | plasma rifle impact | plasma | none | pings, MP headshots | 10 | 12 - 14 | 0 | 0 | 0.12 | 1, 1, 0.15 | 0 | 0.5, 2 |
| `weapons\plasma pistol\bolt` | plasma pistol impact | plasma | none | pings | 10 | 16 - 20 | 0 | 0 | 0.12 | 1, 1, 0.15 | 0 | 0.6, 0.6 |
| `weapons\plasma rifle\charged bolt` | charged bolt impact | plasma | emp | pings | 70 | 70 - 70 | 0 | 0 | 1 | 0 | 0 | 0.6, 1 |
| `weapons\needler\impact damage` | needle impact | bullet | none | 0 | 0 | 0 - 0 | 0 | 0 | 0.15 | 0 | 0 | 0, 0 |
| `weapons\needler\detonation damage` | stuck needle | bullet | none | 0 | 10 | 10 - 10 | 0 | 0 | 0.1 | 0 | 0 | 1, 1 |
| `weapons\needler\explosion` | supercombine | grenade | emp | det. explosives | 40 | 60 - 60 | 0.5 - 1 | 0 | 1 | 0 | 2 | 1, 1 |
| `weapons\needler\shock wave` | supercombine | none | none | 0 | 0 | 0 | 2 - 4 | 0 | 0 | 0 | 0 | 0, 0 |
| `weapons\frag grenade\explosion` | frag | grenade | none | det. explosives | 80 | 120 - 120 | 1.5 - 2.5 | 0.4 | 0.6 | 0 | 4 | 1, 1 |
| `weapons\frag grenade\shock wave` | frag, rocket, tank shell | none | none | 0 | 0 | 0 | 3.5 - 8 | 0 | 0 | 0 | 0 | 0, 0 |
| `weapons\plasma grenade\explosion` | plasma grenade | grenade | emp | det. explosives | 80 | 120 - 120 | 0.5 - 2 | 0.4 | 1 | 0 | 4 | 1, 1 |
| `weapons\plasma grenade\attached` | stuck plasma grenade | grenade | emp | 0 | 80 | 120 - 120 | 0 | 0 | 0 | 0 | 4 | 1, 1 |
| `weapons\plasma grenade\shock wave` | plasma grenade | none | none | 0 | 0 | 0 | 3.5 - 8 | 0 | 0 | 0 | 0 | 0, 0 |
| `weapons\rocket launcher\explosion` | rocket | high explosive | none | det. explosives | 80 | 300 - 330 | 0.5 - 2 | 0.6 | 1 | 0 | 6 | 1, 1 |
| `vehicles\scorpion\shell explosion` | tank shell | mounted weapon | none | det. explosives | 80 | 351 - 351 | 1.5 - 3.25 | 0 | 0 | 0 | 6 | 1, 1 |
| `weapons\flamethrower\explosion` | flame | flame | none | flaming death | 10 | 10 - 10 | 0.4 - 0.4 | 0 | 1 | 0 | 0 | 1, 1 |
| `vehicles\warthog\bullet` | Warthog gun | mounted weapon | none | 0 | 16 | 16 - 24 | 0 | 0 | 0.25 | 0 | 0 | 1, 1 |
| `vehicles\scorpion\bullet` | Scorpion trigger 1 | mounted weapon | none | 0 | 10 | 10 - 10 | 0 | 0 | 1 | 0 | 0 | 1, 1 |
| `vehicles\ghost\ghost bolt` | Ghost gun | mounted weapon | none | pings | 10 | 10 - 14 | 0 | 0 | 0.25 | 1, 1, 0.15 | 0 | 0.5, 2 |
| `weapons\<weapon>\melee` (assault rifle, pistol, shotgun, plasma rifle, sniper rifle, plasma pistol, needler, rocket launcher) | weapon melee | melee | lethal to the unsuspecting | no hurt owner, pings; tag flag: no distance scaling | 40 | 50 - 60 | 0.5 - 0.5 | 0 | 0 | 0 | 0 | 1.4, 1.4 |
| `weapons\flamethrower\melee` | melee | melee | lethal to the unsuspecting | as above | 60 | 70 - 80 | 0.5 | 0 | 0 | 0 | 0 | 1, 1 |
| `weapons\ball\melee`, `weapons\flag\melee` | melee | melee | lethal to the unsuspecting | as above | 80 | 80 - 80 | 0.5 | 0 | 0 | 0 | 0 | 1, 1 |
| `characters\cyborg\melee` | the Spartan unit's own melee (no weapon) | melee | none | no hurt owner | 1000 | 1000 - 1000 | 0.2 - 0.2 | 0 | 0 | 0 | 1.5 | 1, 1 |
| `globals\falling` | falling (movement.md) | falling | none | 0 | 10 | 155 - 155 | 0 | 0 | 0 | 0 | 0 | 1, 1 |
| `globals\distance` | lethal fall | falling | none | 0 | 105 | 105 - 105 | 0 | 0 | 0 | 0 | 0 | 1, 1 |
| `globals\vehicle_collision` | vehicle hits a unit (vehicles.md) | vehicle | none | 0 | 1000 | 1000 - 1000 | 0 | 0 | 0 | 0 | 0 | 1, 1 |
| `globals\flaming_death` | burning | flame | none | 0 | 5000 | 5000 - 5000 | 0 | 0 | 0 | 0 | 0 | 1, 1 |
| `globals\vehicle_hit_environment`, `globals\vehicle_killed_unit` | vehicles | none | none | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1, 1 |

Firing ("trigger") damage effects, applied to the shooter for its
screen kick (weapons.md), do no damage (all modifiers 0); their only
gameplay field is active camouflage damage: assault rifle 0.01, plasma
pistol and its overcharge 1.0, all others 0. Melee "response" effects
(applied to the attacker when the melee connects) and the misfire
effects are all zero. `vehicles\scorpion\shell shock wave` (the cannon's
firing damage) has radii 10 - 15 and no damage.

**Material modifiers**, all 33 types. Given as "value: types"; types
not listed have 0. (Types: 0 dirt, 1 sand, 2 stone, 3 snow, 4 wood, 5
metal hollow, 6 metal thin, 7 metal thick, 8 rubber, 9 glass, 10 force
field, 11 grunt, 12 hunter armor, 13 hunter skin, 14 elite, 15 jackal,
16 jackal energy shield, 17 engineer, 18 engineer force field, 19 flood
combat form, 20 flood carrier form, 21 cyborg, 22 cyborg energy shield,
23 armored human, 24 human, 25 sentinel, 26 monitor, 27 plastic, 28
water, 29 leaves, 30 elite energy shield, 31 ice, 32 hunter shield.)

| Damage effect | Modifiers |
|---|---|
| AR bullet | 1: 9, 11, 14, 15, 21 to 25; 0.25: 5 to 8; 0.5: 12, 13; 0.6: 17; 0.65: 19, 20; 0.7: 30 |
| Scorpion bullet | as AR bullet, and 0.7: 16 |
| pistol bullet | 1: 11, 13, 14, 15, 22, 23, 24, 30; 1.5: 19, 20, 21; 4: 9; 2: 17; 0.25: 5 to 8; 0.2: 12, 25 |
| shotgun pellet | 1: 9, 11, 13, 14, 15, 17, 21, 23, 24, 30; 0.5: 22, 25; 0.2: 12; 1.5: 19; 1.75: 20; 0.25: 5 to 8 |
| sniper bullet | 1: 9, 11, 14, 15, 21 to 24; 2: 30; 0.5: 12, 13; 0.2: 17, 25; 0.05: 19, 20 |
| plasma rifle bolt | 1: 9, 11, 14 to 17, 19, 20, 23, 24; 2: 10, 22, 25, 30; 0.5: 12, 13, 21; 0.25: 5 to 8 |
| ghost bolt | 1: 11, 14, 15, 17, 19, 20, 23, 24; 2: 10, 16, 22, 25, 30; 0.5: 12, 13, 21; 0.25: 5 to 8 |
| plasma pistol bolt | 1: 9, 11, 14 to 17, 19, 20, 23, 24; 2: 10, 25, 30; 0.6: 21, 22; 0.5: 12, 13; 0.25: 5 to 8 |
| charged bolt | 1: 9, 11, 14 to 17, 19, 20, 22; 4: 10, 25; 1.5: 30; 0.6: 21; 0.5: 12, 13; 0.2: 23, 24; 0.25: 5 to 8 |
| needle detonation damage | 1: 9, 11, 14, 15, 16, 21 to 24, 30; 1.2: 19, 20; 0.6: 17; 0.5: 12, 13; 0.25: 5 to 8 |
| needle impact damage | 0 for all |
| Warthog bullet | 1: 9, 11, 14, 15, 17, 19 to 25, 30; 0.5: 12, 13; 0.25: 5 to 8 |
| frag explosion | 1: 0 to 8, 11, 14 to 17, 21 to 24, 27 to 31; 4: 9, 20; 2: 19; 0.5: 13; 0.4: 25; 0.25: 12; 0.2: 10 |
| rocket explosion, tank shell explosion | 1: 0 to 8, 11 to 17, 21 to 24, 27 to 31; 4: 9, 20; 2: 19; 0.4: 25; 0.2: 10 |
| plasma grenade explosion | 1: 0 to 11, 14, 15, 17, 21 to 24, 27, 28, 29, 31; 4: 20, 25, 30; 2: 19; 1.5: 16; 0.5: 13; 0.25: 12 |
| needler explosion | as the plasma grenade explosion but 0.2: 12 |
| plasma grenade attached | 1: 9, 10, 11, 13 to 17, 19 to 24; 4: 25, 30; 0.2: 12 |
| flame explosion | 1: 11, 13, 15, 16, 17, 21 to 24, 30; 4: 9, 20; 2: 19; 0.4: 14, 25; 0.2: 10, 12 |
| weapon melees (8 weapons) | 1: 9, 11 to 18, 23, 24, 30; 1.4: 21, 22; 0.2: 19, 20, 25, 26 |
| flamethrower, ball, flag melee | 1: 9, 11 to 18, 21 to 24, 30; 0.2: 19, 20, 25, 26 |
| Spartan unit melee | 1: 11, 14 to 25, 30; 0.2: 12, 13 |
| falling, distance, vehicle effects | 1: 0 to 30 (0 for ice and hunter shield) |
| flaming death | 1: 0 to 31 (0 for hunter shield) |
| shock waves | 0 (frag shock wave: 1 for glass, 9) |

The explosions (frag, rocket, tank shell, plasma grenade, needler) have
0 for 18, 26 and 32 (engineer force field, monitor, hunter shield).

### Shots to kill (sanity check)

Against a full multiplayer Spartan, read only to check this spec
(health_shields.md owns the rule): collision model
`characters\cyborg\cyborg`, 75 shield (material type 22, no leak), 75
body; model materials "head" (type 21, head flag, body x 1), "body"
(21, x 1), "legs" (21, x 0.8), "shield generator" (21, x 1); indirect
material "body". Point-blank hits (scale 1), 32-bit arithmetic,
shield emptied when the hit's share exceeds what is left and the excess
passing to the body, death when body vitality goes below 0:

| Weapon | Per hit | Shots to kill |
|---|---|---|
| pistol | 25 on the shield, 37.5 on the body | 5 body shots (the third leaves a tiny excess, so two more pass 0); **3 if the third is to the head** (headshot flag; the excess is tiny but positive) |
| assault rifle | 10 | 16 bullets |
| plasma rifle | 24 - 28 on the shield, 6 - 7 on the body | 14 - 15 bolts; 8 - 9 to the head (doubled in multiplayer) |
| shotgun | pellet 9 - 12.5 on the shield, 18 - 25 on the body | one shot if 9 to 13 of the 15 pellets hit within about 18 WU; beyond (8 per pellet) 29 pellets |
| sniper rifle | 101 | **1 to the head** (101 exceeds the 75 shield; the 26 that reach the body make it a headshot); 2 to the body |
| frag | 120 within 1.5 WU | 2 (one leaves 30 body points) |
| plasma grenade stuck | 120 attached (emp) + 120 area | 1 |
| rocket | 300 - 330 within 0.5 WU, 153 - 163 at 1.5 WU | 1 anywhere inside 2 WU |
| charged bolt then pistol | shield emptied (no excess), then 37.5 per body shot | bolt + 1 headshot, or bolt + 3 body |

These agree with what is widely known of the game (three-shot pistol,
sticky plasma kill, two frags, one rocket, the "noob combo").

## Per-tick order

**Within the object update of a tick** (objects.md): objects existing
at the start are updated in object order; a projectile stuck in an
object is updated right after it; objects created during that pass,
including every projectile fired this tick, get their first update
after it, in the same tick. (While the double speed powerup is active,
objects that are not players' units, projectiles included, skip their
update on odd ticks; reference only.)

**One projectile update:**

1. Remove the tracer contrail if not a tracer.
2. Arming += step; deceleration timer += step.
3. Decide whether the detonation timer runs (behaviour 2); if it does,
   timer += step, and request detonation at 1.
4. Write the object function inputs.
5. Movement loop, up to 10 surfaces, while r > 0 and no request is
   pending (or a detonation waits for arming) and not stuck, not at
   rest, no parent:
   1. guidance turn;
   2. deceleration, or the slow-down disappearance, or the final-speed
      floor;
   3. gravity;
   4. range cut (may request detonation);
   5. trace the segment (unless disappearing or at the surface limit);
   6. at a hit: restore gravity and deceleration for the remainder,
      direct-hit damage and push (behaviours 8 and 11), response
      (behaviours 6 and 7: may stick, stop, request), impact effect,
      AI impact noise;
   7. odometer, flyby, orientation, position, velocity.
6. A detonation request with arming complete (or no arming):
   detonation (behaviour 9), then deletion. A disappearance request:
   deletion.

**One detonation:** supercombine check; contrail end; detonation
effect (its damage parts: area damage events, each in candidate order:
visibility, exclusions, scale, damage event, push, children); attached
detonation damage on the parent; hit material's detonation effect; AI
noise; delete.

**One damage event on one object** (the parts this spec owns in
brackets): [amount], damage to shield and body and death
(health_shields.md), [push], statistics, unit reactions.

## Edge cases

- **Target exactly at a segment's end.** A trace ending exactly on a
  surface does not hit it (the end is not past the surface); the next
  update starts on it and hits at t = 0.
- **Starting on a floor.** A grenade lying on a surface whose segment
  heads down hits at t = 0; gravity and deceleration are restored for
  the full remainder and it bounces again in the same update; ten such
  contacts in one update request a detonation that then waits for
  arming (Quirks).
- **Material-less level surfaces** (Blood Gulch's outer walls,
  collision.md) report no material: the built-in response applies, so
  **anything, grenades and rockets included, disappears** there without
  exploding or making an effect.
- **Hit material index beyond the block** (the flame on ice or a
  hunter shield): built-in response.
- **Zero velocity at a hit** (a resting grenade pushed into a wall): the
  direction used for effects is +Z.
- **Coincident epicenter and centre** (d = 0): scale 1, direction zero
  for damage, the push points straight up.
- **The owner is gone** when the explosion happens (left the game,
  deleted): the effect has no owner object, so the damage has no owner:
  no credit, and "does not hurt owner" protects nobody.
- **Shooter in a vehicle**: the vehicle is the ignored object until
  the first collision, so a Warthog gunner's bullets do not hit the
  Warthog; after a bounce they can.
- **Overpenetration** ignores only the one object passed, so a bullet
  cannot hit the same object twice in a row, but can hit it again after
  touching anything else.
- **Grenade in hand when the thrower dies**: the death animation cuts
  the throw, the grenade is released with reduced power (behaviour 15)
  and goes off on its own rules.
- **Stuck in a vehicle that explodes**: detached and detonated on its
  next update (behaviour 7).
- **Under water at creation**: the water gravity scale and water
  window apply from the start.
- **An object without the "body" marker** guided toward: its origin.

## Quirks

1. **The deceleration step is lower bound / initial velocity** (checked
   in the disc's executable). Every disc projectile except the pellet
   decelerates from its first update, so the plasma rifle bolt spends
   its speed over 0 to 30 WU instead of 20 to 50 and vanishes at 30.8
   WU; the pellet keeps full speed and full damage for three ticks (14
   WU) and loses it all in the fourth. Keep.
2. **"After first bounce" waits for a stop** (checked in the disc's
   executable): the frag's 0.5 s fuse starts when it lies still. Keep.
3. A detonation request waits for arming but is never cancelled: a frag
   that hit the 10-surface limit while unarmed explodes the moment it
   is armed, whatever its fuse. Keep.
4. The air case of the disappearance test reads the water window's
   upper bound. Harmless on the disc. Keep.
5. Guidance turns by the full rate every segment and so oscillates
   around the target line. Keep.
6. The push ignores distance. Keep.
7. Explosions at maximum range: rockets explode in mid-air at 128 WU;
   bullets end at 40 WU (pistol, rifle, pellet) with nothing to show.
   Keep.
8. Supercombine needles moved to the victim's feet; with exactly seven
   needles a full Spartan survives (behaviour 13). Keep.
9. Range uses the average speed without gravity's share while the
   odometer counts the real path; the difference is below 0.0001 WU on
   the disc. Keep.
10. The pistol's three-shot kill rests on 25 / 75 rounding up in 32-bit
    floats (the third shot leaves a positive excess for the headshot).
    Use 32-bit floats for shield and body arithmetic (health_shields.md).

## Test cases

All on a flat stone floor z = 0 (material type 2, normal +Z) unless
said otherwise, no other geometry, gravity 0.0035651792, 32-bit timer
arithmetic. Position after the update, speed = |velocity| in WU/tick.
Values to 1e-4 unless noted.

### (a) Level shots from (0, 0, 0.62), direction +X, shooter at rest

| Projectile | Update 1 | Update 2 | Update 5 | Update 10 | End |
|---|---|---|---|---|---|
| pistol bullet | (10.0, 0, 0.61822), 10.000001 | (20.0, 0, 0.61287) | gone | gone | update 4: cut at x = 40.0 (z 0.59148), range detonation (no effect) |
| assault rifle bullet | (10.8, 0, 0.61822) | (21.6, 0, 0.61287) | gone | gone | update 4 at x = 40.0 (z 0.59518) |
| plasma rifle bolt | (1.64931, 0, 0.62), 1.631944 | (3.26389, 0, 0.62), 1.597222 | (7.89931, 0, 0.62), 1.493056 | (14.93056, 0, 0.62), 1.319444 | reaches the final speed at x = 30.0 after update 24; at most one more move (to 30.825 at 0.825) and deleted on the next update (disappear; exact update depends on rounding at the final-speed boundary: accept 25 or 26) |
| needle | (0.13333, 0, 0.62) | (0.26667, 0, 0.62) | (0.66667, 0, 0.62) | (1.33333, 0, 0.62) | update 150: x = 20.0, range detonation (needle detonate effect, no damage) |
| rocket | (0.39988, 0, 0.62), 0.399756 | (0.79951, 0, 0.62), 0.399511 | (1.99694, 0, 0.62), 0.398778 | (3.98778, 0, 0.62), 0.397556 | final speed after update 273 (x 100.089, speed 0.33); update 358: x = 128.0, explodes |
| plasma pistol bolt | (0.82963, 0, 0.61982), 0.825926 | (1.65185, 0, 0.61929) | (4.07407, 0, 0.61559) | (7.96296, 0, 0.60264), 0.759267 | hits the floor on update 65 at (39.7756, 0, 0): disappear |
| shotgun pellet | (4.66667, 0, 0.61822) | (9.33333, 0, 0.61287) | (20.85624, 0, 0.58111), 3.300014 | (37.35596, 0, 0.46308) | update 4 ends at x 17.55624 (speed 3.30001); update 11: x 39.99938, range |
| sniper bullet | (33.33333, 0, 0.62) | (66.66666, 0, 0.62) | (166.66666, 0, 0.62) | (333.33332, 0, 0.62) | update 31: x = 1000 |
| Warthog bullet | (9.52848, 0, 0.61822), 9.290285 | (18.58057, 0, 0.61296), 8.813903 | (42.87855, 0, 0.57828), 7.384769 | (73.84754, 0, 0.47095), 5.002915 | update 21: disappears at x 100.065 |
| flame | (0.33333, 0, 0.62) | (0.66667, 0, 0.62) | (1.66667, 0, 0.62) | gone | update 8: x = 2.5, explodes |

### (b) Direct-hit damage against a wall at x = D (shot as above)

| Projectile | D | Hit on update, t | Speed at impact | Scale | Amount |
|---|---|---|---|---|---|
| pistol | 5 | 1, 0.5 | 10.0 | 1 | 25 |
| pistol | 30 | 4, 0.0 (update 3 ends exactly on the wall) | 10.0 | 1 | 25 |
| pistol | 60 | never (gone at 40 WU) | | | |
| Warthog bullet | 5 | 1, 0.52474 | 9.51669 | 0.9735 | 16 + 0.9735 (U(16, 24) - 16): 16.0 to 23.788 |
| Warthog bullet | 30 | 4, 0.35111 | 8.17026 | 0.8308 | 16.0 to 22.646 |
| Warthog bullet | 60 | 8, 0.53355 | 6.17785 | 0.6196 | 16.0 to 20.956 |
| plasma rifle bolt | 5 | 4, 0.10112 | 1.55899 | 0.8708 | 11.742 to 13.483 |
| plasma rifle bolt | 20 | 15, 0.0597 | 1.17848 | 0.4142 | 10.828 to 11.657 |
| shotgun pellet | 15 | 4, 0.2812 | 4.66668 (restored, clamped to the start speed) | 1 | 18 to 25 |
| shotgun pellet | 20 | 5, 0.74053 | 3.30001 | 0 | 8 |

### (c) Grenades on a flat stone floor

Thrown from (0, 0, 0.62) with the grenade's velocity set to 0.33333 x
(cos p, 0, sin p) on update 1 (time in the hand not counted: arming
starts with update 1).

| Case | Bounce 1 (update, t, point, velocity after) | Bounce 2 | Bounce 3 | Comes to rest | Detonates |
|---|---|---|---|---|---|
| frag, p = 0.2 rad | 45, 0.89501, (14.6667, 0, 0), (0.065338, 0, 0.028151) (impact normal speed 0.093836) | 61, 0.67309, (15.6976, 0, 0), (0.013068, 0, 0.00843) | 66, 0.3535, (15.75877, 0, 0), (0.002614, 0, 0.002477) | update 66 (speed^2 1.3e-5) | update 81 (timer 15 updates from 67; armed since 45) at (15.75877, 0, 0) |
| frag, p = 0 | 19, 0.64348, (6.21449, 0, 0), (0.066667, 0, 0.01994) | 30, 0.81613, (6.95934, 0, 0), (0.013333, 0, 0.005968) | 34, 0.12981, (7.00352, 0, 0), (0.002667, 0, 0.001754) | update 34 | update 49 |
| frag dropped (speed 0) | 19, 0.64348, (0, 0, 0), (0, 0, 0.01994) | 30, 0.81613, (0, 0, 0), (0, 0, 0.005968): stopped | | update 30 | update 45 (timer and arming both reach 1) |
| plasma grenade, p = 0.2 | 45, 0.89501, (14.6667, 0, 0), (0.016334, 0, 0.009384) | 51, 0.13653, (14.75232, 0, 0), (0.000817, 0, 0.00093): stopped | | update 51 | update 112 (61 timer updates from 52) |

Positions before the first bounce, frag p = 0.2: update 1 (0.32669, 0,
0.68444) velocity (0.326689, 0, 0.062658); update 10 (3.26689, 0,
1.10397); update 20 (6.53378, 0, 1.23143); update 30 (9.80067, 0,
1.00236).

| Setup | Expected |
|---|---|
| plasma grenade hits a standing Spartan (any of materials 21, 22) | potential attach: stuck at the hit point on the hit node, velocity 0, detonation started effect plays, timer step 1/60; detonates on the 61st update after sticking |
| plasma grenade hits a Warthog (metal 5 to 7) | sticks (a vehicle is a unit) |
| frag hits a Spartan | bounces, 0.8 / 0.9 |
| frag hits a material-less wall | disappears, no explosion |
| player throw, camera at (0, 0, 0.62), aim +X | grenade starts at (0, 0.05, 0.62) with velocity (0.33333, 0, 0) even if the thrower runs at 0.075 WU/tick |
| throw cut at p = 0.5 with U = 0.03 | velocity = 0.5 x 0.33333 + 0.5 x 0.03 = 0.18167 WU/tick along the aim |

### (d) Area damage and push

d = distance from the epicenter to the target's bounding-sphere centre.
Amounts before the target's material modifiers; the push for a living
multiplayer Spartan (acceleration scale 0.7) with the centre level with
the epicenter (direction (1, 0, 0) before the bias).

| Explosion | d = 0 | 0.5 | 1.0 | 1.5 | 2.0 |
|---|---|---|---|---|---|
| frag (1.5 - 2.5) | s 1: 120 | 120 | 120 | 120 | s 0.5: 100 |
| plasma grenade (0.5 - 2) | 120 | 120 | s 0.66667: 106.667 | s 0.33333: 93.333 | s 0: none |
| rocket (0.5 - 2) | 300 to 330 | 300 to 330 | 226.667 to 246.667 | 153.333 to 163.333 | none |
| tank shell (1.5 - 3.25) | 351 | 351 | 351 | 351 | s 0.71429: 273.571 |
| needler super (0.5 - 1) | 60 | 60 | none | none | none |

| Push | Delta v on the biped (WU/tick) |
|---|---|
| frag or plasma, any d inside the cutoff, horizontal | (0.042556, 0, 0.019150), magnitude 0.046667 |
| frag, d = 0 | (0, 0, 0.046667) |
| rocket or tank shell, horizontal | (0.063835, 0, 0.028726), magnitude 0.07 |
| rocket, dead biped | magnitude 0.14 |
| frag at d = 2.5 or beyond | none |
| rocket on a Warthog (acceleration scale 0.3, detonates explosives) | magnitude 0.12, handed to the vehicle |
| frag explosion on a resting frag grenade (scale 2.0) | 0.26667 added to its velocity, at rest cleared |

Full Spartan (75 / 75): one frag at d <= 1.5 leaves shield 0, body 0.4
(30 points); a second kills. A stuck plasma grenade: attached 120 emp
leaves body 0.4, the area 120 then kills. A rocket at d = 1.5 (153.3
minimum) kills.

### (e) Guidance

Needle fired from (0, 0, 0.62) along +X (0.13333 WU/tick); the target's
body point during update k is (8, 0.075 (k - 1), 0.62) (crossing at
2.25 WU/s); wander forced to 0 for the test (it is a separate unit
test: the length is 1 WU at e = 12, 0.5 at e = 6, 0 at e = 1.5).
No floor.

| Rate | Update 1 | Update 2 | Update 10 | Update 20 | Update 40 | Update 60 |
|---|---|---|---|---|---|---|
| player target, normal (0.0261799 rad = 1.5 deg per update) | pos (0.1333, 0, 0.62), heading 0 deg | heading 1.5 deg | pos (1.3277, 0.0485), heading 4.5 deg | pos (2.633, 0.2378), 13.5 deg | pos (5.0923, 1.167), 31.5 deg | pos (7.0202, 2.9398), 55.5 deg |
| AI target (0.0523599 rad = 3 deg) | heading 0 | 3.0 deg | (1.3311, 0.0558), 3.0 deg | (2.6365, 0.2416), 15.0 deg | (5.0943, 1.1703), 33.0 deg | (7.0215, 2.9424), 57.0 deg |

(Heading = atan2(vy, vx) after the update; on update 1 the target is
dead ahead so the rotation axis is zero and nothing turns. At the AI
rate the heading after updates 2, 5 and 10 is 3, 0 and 3 degrees: it
overshoots and swings back around the line of sight.) Charged bolt, same target, player rate
0.0084357 rad: heading 0.483 deg after update 2, 4.35 deg after 10,
8.7 deg after 20, passing the target at 0.755 WU on update 18.

### (f) Timers and supercombine

| Setup | Expected |
|---|---|
| needle sticks into a biped at update N | detonates at the end of update N + 23 (timer 23 x 0.044444446 >= 1) |
| six needles stuck, a seventh sticks | all seven timers 0; seven detonate 23 updates later; the first to update in a multiplayer game triggers the supercombine (7 > 6) |
| six needles stuck only | no supercombine; each deals 10 |
| seven needles in a player's biped in single player | no supercombine |
| needle grazes stone at 20 deg, random draws stubbed to 0 (no angle or turn noise) | potential reflect with 0.3/0.3: velocity along the normal x -0.7, along the surface x 0.7; speed 0.09333 > minimum 0.06667, keeps flying |
| needle hits stone at 60 deg, draws stubbed to 0 | attach to the level; detonates harmlessly 23 updates later |
| stuck projectile's parent destroyed | detonates on its next update |

## Verification performed

- All 16 projectile tags and 66 damage effects of Blood Gulch parsed
  with the layouts above: every tag reference has the expected class
  (`effe`, `snd!`, `jpt!`, or null); every enumeration in range
  (timer starts 0 to 2, responses 0 to 4, scale-by 0 or 1, side effect
  0 to 3, category 0 to 12); every pad range of the response element
  (0x14, 0x4C, 0x78) and of the damage effect (14 ranges) is zero;
  material modifier slots 33 to 39 are zero on every damage effect;
  response blocks have 33 elements (31 for the flame); reals plausible.
- Hang 'Em High, Sidewinder and Chill Out: all 82 tags identical field
  by field (addresses aside); `ui.map`'s two grenade projectiles
  identical too.
- The struct sizes agree with objects.md (0x24C) and with the zero
  pads (0x2A0: the last damage effect field ends at 0x2A0 with 28
  zero bytes before it).
- Disc executable: the instruction pair "load the air (water) damage
  range lower bound, divide by the initial velocity, store" exists for
  both windows; the timer-starts test sends values 1 and 2 to the same
  "stopped" bit check.
- Shots to kill computed from the disc's collision model and damage
  effects (table above) match common knowledge.

## Confidence per section

| Section | Confidence |
|---|---|
| Projectile tag layout and values | checked against the disc (4 maps) |
| Material response layout and values | checked against the disc; meaning of fields from the reference; "cannot be overpenetrated" and "maximum distance" unused per the reference |
| Damage effect layout and values | checked against the disc (pads zero, values plausible, 4 maps); presentation field names partly from general knowledge of the tag |
| Damage flags, categories, side effects | names from the reference and the tag; values from the disc |
| 1 Creation | reference only (inherited velocity: weapons.md) |
| 2 Timers | reference; the fall-through of timer-starts checked in the disc's executable; 32-bit counts computed |
| 3 Flight | reference; the deceleration step checked in the disc's executable |
| 4 Hitscan | reference (object update order) and computed |
| 5 Trace | reference; collision.md for the trace itself |
| 6, 7 Responses | reference; disc values |
| 8 Direct-hit damage | reference |
| 9 Detonation | reference; effect contents from the disc |
| 10 Area damage | reference |
| 11 Push | reference; acceleration scales from the disc |
| 12 Guidance | reference; difficulty values from the disc's globals |
| 13 Supercombine | reference |
| 14 Charged bolt | disc values, reference behaviour |
| 15 Grenades | reference (throw, release, counts); disc values (globals, unit) |
| 16, 17 | disc values; presentation from the reference |
| Shots to kill | computed from disc values with health_shields' rules as the reference has them (to be confirmed by that spec) |
| Test cases | computed by a scratch re-statement of these rules |

## Open questions

1. **Frictions' axes.** The reference applies "parallel friction" to
   the component along the surface normal and "perpendicular friction"
   to the sliding component; the tag names suggest the opposite reading
   is possible. Not checked in the executable. Measure: drop a frag
   straight onto stone from a known height in the real game and time
   the second bounce (normal restitution 0.3 versus 0.2).
2. **Multiplayer difficulty.** Guidance against players uses the
   session's difficulty (0.5 at easy and normal, 0.75 heroic, 1.0
   legendary). Which difficulty a multiplayer game on the Xbox runs at
   is not settled here. Measure: needle a strafing player at 8 WU and
   compare the curve with test (e).
3. **Spawn grenade counts.** The reference gives the maximum (4) frags
   at spawn, 2 with 5 or more players, 1 with 9 or more, and plasma
   grenades only through the game type or starting equipment. Common
   memory of the game is "two frags". multiplayer_rules.md should
   settle it; measure by spawning in a 2-player game.
4. **The 32-bit boundary at the final speed** (plasma rifle bolt ends on
   update 25 or 26) depends on x87 precision in the real engine. A test
   should accept either.
5. **Frags rolling.** On flat stone a frag stops in two or three
   bounces; players remember grenades rolling down Blood Gulch's
   slopes. Slopes keep giving them gravity along the surface; check on
   a hill that a thrown frag keeps sliding until the slope flattens.
6. **Effect timing.** That an explosion's damage happens on the
   detonation tick assumes zero-delay effect events are processed at
   creation (effects.md). Measure with a grenade at a known fuse.
7. **Double speed powerup** halves the update rate of projectiles and
   every non-player object (reference). The powerup is not placed on
   stock maps; confirm if it ever matters.
8. **Parent deletion.** What happens to a projectile stuck in an object
   that is deleted (not destroyed), such as a corpse removed by garbage
   collection, is not traced here (probably deleted with it).
