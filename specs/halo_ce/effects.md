# Halo: Combat Evolved: effects, particles, lights, decals

Source basis: the game's own map files from the original Xbox disc
(`bloodgulch.map` measured in full; `hangemhigh.map` and
`damnation.map` compared tag by tag), read with throwaway scripts; and
a decompilation of an Xbox build of the engine (effect creation and
update, particles, point physics, particle systems, contrails, decals,
lights and their rendering, material effects, the player's screen and
camera effects), used for behaviour only. Where the disc and the
decompilation disagree, the disc wins and the spec says so.
Status: draft

Sections: Summary; Units and conventions; Constants; The effect tag;
Creating an effect; Event timing; Parts; Particle entries; Random
draws; The particle tag; Point physics; How a particle lives;
Particle systems; Contrails; Lights; Lens flares and glows; Decals;
Material effects; Damage effect presentation; What is on the disc;
Per-tick order; Limits; Edge cases; Quirks; Test cases; Verification
performed; Confidence per section; Open questions.

---

## Summary

- An **effect** (`effe`) is a short script of **events** played at a
  set of named **locations** (model markers, or the marker list the
  caller hands over: an impact gives "normal", "incident", ...). Each
  event waits a random delay, then makes its **parts** at once (a
  sound, a decal, a light, a damage event, a particle system, an
  object) and spreads its **particles** over a random duration. Parts
  and particles can be limited to air or water, to violent or
  nonviolent use, and particles to the first- or third-person view.
  Two scale inputs A and B, supplied by whoever makes the effect (a
  weapon's rate of fire and heat, an impact's damage and the struck
  object's vitality), multiply chosen values.
- A **particle** (`part`) is a camera-facing or direction-aligned
  sprite with a lifespan, an animated sprite sequence, a radius curve,
  fades, and point physics (`pphy`): gravity weighted by density
  against air or water, drag toward the wind, bounces off the level.
  Muzzle flashes are a handful of additive sprites living two or three
  frames; shell casings are particles (not objects) that tumble, bounce
  and click on the floor through a material effects tag.
- **Particle systems** (`pctl`) drive explosion fireballs and smoke,
  **contrails** (`cont`) are ribbons through emitted points (bullet
  tracers, rocket and needle trails), **lights** (`ligh`) are dynamic
  point lights (the muzzle flash light is a weapon attachment lit by
  the weapon's "illumination" export for three ticks), **decals**
  (`deca`) are textured polygons clipped onto the level (bullet holes,
  scorch marks, blood), and **material effects** (`foot`) map (kind,
  surface material) to an effect and a sound (footsteps, casings,
  dropped weapons, tyres).
- Damage effects (`jpt!`, laid out in `projectiles.md`) carry the
  player-side presentation: screen flash, camera impulse and shake,
  rumble. A weapon's per-shot "kick" is its trigger's firing damage
  effect applied to the shooter.

## Units and conventions

As `cache_file.md`: 1 WU = 3.048 m, +Z up, +X forward, radians, 30
ticks per second, little-endian, tag primitives, blocks, tag references
and data references as defined there. Offsets are from the start of
the struct named. Additions:

- **Time**: event delays and durations, particle lifespans, fades,
  decal lifetimes, contrail and particle-system state times, damage
  effect presentation durations are **seconds**. The one exception
  found: a light's effect **duration** (`ligh` 0xF4) is stored in
  **ticks** (the build tool multiplied by 30): the frag explosion light
  stores 45 (1.5 s) and the engine compares it with elapsed ticks.
- **Velocities**: particle, particle-system and contrail velocities
  are **WU per second** (their physics integrates with dt in seconds).
  Effect part velocities for objects are WU per tick (they become the
  object's velocity), for decals a trace length in WU (below), for
  particle systems WU per second. An effect's own inherited velocity
  is per tick (and multiplied by 30 where it is added to a particle).
- **Angular velocities**: radians per second.
- **Random generator**: the game-state generator is `weapons.md`'s
  (seed s' = 1664525 s + 1013904223 mod 2^32; integer draw s' >> 16;
  real draw r/65535; real range draw lo + (hi - lo) x real; integer
  range draw lo + floor((hi - lo) r / 65536)). Presentation uses a
  second, **local** generator with the same recurrence and its own
  seed; it never affects game state. A **direction draw** is one
  integer range draw into the 1026-entry direction table of
  `weapons.md`.
- **Transition functions** (0 linear, 1 early, 2 very early, 3 late,
  4 very late, 5 cosine) and **periodic functions** (0 one ... 11
  spark) are `objects.md`'s and `shaders.md`'s tables.
- **Framebuffer blend functions** and **fade modes**: `shaders.md`
  (0 alpha blend, 1 multiply, 2 double multiply, 3 add, ...).
- Material types 0 to 32: `collision.md` / `projectiles.md` (0 dirt, 1
  sand, 2 stone, 4 wood, 5-7 metal, 9 glass, 21 cyborg, 22 cyborg
  energy shield, 28 water, 29 leaves, ...).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| MAX_EFFECTS | 256 | | effects alive at once (pool) |
| MAX_EFFECT_LOCATION_INSTANCES | 512 | | effect location instances alive at once |
| MAX_EFFECT_LOCATIONS | 32 | per effect | locations block maximum |
| MAX_EFFECT_EVENTS | 32 | per effect | events block maximum |
| MAX_EFFECT_PARTS | 32 | per event | parts block maximum |
| MAX_EFFECT_PARTICLES | 32 | per event | particles block maximum |
| MAX_INSTANCES_PER_LOCATION | 16 | | markers of one name an effect uses |
| MAX_EVENT_STEPS_PER_UPDATE | 8 | | event phase steps one effect update may take |
| CORPSE_SETTLING | 30 | ticks | a dead biped is a "corpse" this long after death |
| SPLIT_COUNT_KEEP | 6 | particles | per-entry count kept whole in split screen |
| SPLIT_LIFE_KEEP | 0.7 | s | particle lifespan kept whole in split screen |
| MAX_PARTICLES | 1024 | | particles alive at once |
| PARTICLE_UNRENDERED_LIMIT | 15 | frames | a particle not drawn for longer is deleted |
| PARTICLE_REST_SPEED_SQ | 0.0625 | (WU/s)^2 | below this speed squared a particle may come to rest |
| PARTICLE_SETTLE_NORMAL_Z | 0.8 | | a contact normal steeper than this settles |
| PARTICLE_COLLISION_SPEEDS | 0.5, 1.5 | WU/s | collision effect scale 0 and 1 |
| MAX_PARTICLE_SYSTEMS | 64 | | particle systems alive at once |
| MAX_SYSTEM_PARTICLES | 512 | | particle-system particles alive at once |
| MAX_SYSTEM_TYPES | 4 | per system | particle types used |
| MAX_NEW_SYSTEM_PARTICLES | 128 | per type per update | |
| MAX_CONTRAILS | 256 | | contrails alive |
| MAX_CONTRAIL_POINTS | 1024 | | contrail points alive |
| MAX_CONTRAIL_INSTANCES | 4 | per contrail | markers of the attachment's name used |
| MAX_DECALS | 2048 | | decal records |
| DECAL_VERTEX_CACHE | 2560 pages of 64 bytes | | decal geometry memory (one page holds one quad) |
| DECAL_LOCK_CHANCE | 6553.5 / 65536 (about 10 %) | | a new decal is "locked" against recycling |
| DECAL_LOCKED_HIGH, LOW | 512, 256 | decals | above the high mark, locks are released until the low mark |
| MAX_LIGHTS | 896 | | light records per map |
| MAX_RENDERED_LIGHTS | 128 | per frame | |
| MAX_RENDERED_PARTICLE_GROUPS | 512 | per frame | sprite batches (definition, cluster, view) |
| AIR_DENSITY | 0.0011 | water = 1 | engine constant |
| WATER_DENSITY | 1.0 | | engine constant |
| MASS_PER_DENSITY_R3 | 118613.34 | kg per WU^3 | mass of a ball = density x this x r^3 (4/3 pi x 28316.8 kg/WU^3) |
| GRAVITY | 0.0035651792 | WU/tick^2 | `cache_file.md`; times 900 = 3.20866 WU/s^2 |
| POINT_PHYSICS_MAX_BOUNCES | 3 | per update | |
| POINT_PHYSICS_SURFACE_OFFSET | min(radius, 0.005) | WU | a bounced point is put this far off the surface |
| MATERIAL_EFFECT_RANGE_SQ | 100 | WU^2 | material effects made only within 10 WU of a local camera (one or two local players) |
| MATERIAL_EFFECT_LIFT | 0.01 | WU | material effects are placed this far off the surface |
| FOOTSTEP_TRACE | +0.15 then 0.3 down | WU | footstep ground probe from the foot marker |
| IMPACT_EFFECT_MIN_SPEED | 1/120 | WU/tick | `projectiles.md` |
| DECAL_WRAP | 40 deg / 110 deg / 1.5 r | | decal neighbour angles and edge reach (Decals) |

The pool sizes are the original Xbox ones (the reference's own port
enlarges them; implementations may too, the limits below say what
happens when full).

## The effect tag

Class `effe`, main struct 0x40 bytes. Every padding byte listed is
zero on all 318 effect tags of Blood Gulch.

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x00 | 4 | flags32 | flags | | bit 0 deleted when attachment deactivates; bit 1 required for gameplay |
| 0x04 | 2 | index16 | loop start event | | none -1 |
| 0x06 | 2 | index16 | loop stop event | | none -1 |
| 0x08 | 4 | f32 | runtime: danger radius | WU | build-time: the largest cutoff radius of the damage parts (8.0 on the grenade, rocket and tank shell explosions, 4.0 needler, 0.4 flame, else 0); read only to tell whether a dangerous effect is near a player |
| 0x0C | 28 | pad | | | |
| 0x28 | 12 | block | locations | | 0x20-byte elements, at most 32 |
| 0x34 | 12 | block | events | | 0x44-byte elements, at most 32 |

**Flags.** Bit 0, for effects attached to an object through an
attachment: when the attachment's scale function goes inactive, the
effect is deleted (instead of stopped). Bit 1 "required for gameplay"
(8 tags on the disc: the five explosions, the flame detonation, the
Scorpion cannon fire and the co-op teleport): the effect may not be
created from a particle (refused), takes its event timing draws from
the game-state generator, is visible by every player's potential
visibility (not only local players'), and when the effect pool is full
it evicts an effect that lacks this flag. On the disc exactly the
effects with damage parts have it (and the cannon fire).

**Location element (0x20).** A tag string: the marker name. The empty
name means the object's origin (node 0) for effects on objects, and
the first marker of a caller's marker list; a name the caller's list
lacks also falls back to its first marker. A name an object's model
lacks yields no instance at all (parts and particles at that location
are not made).

**Event element (0x44).**

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x00 | 4 | flags32 | flags | | 0 on the disc, not read |
| 0x04 | 4 | fraction | skip fraction | | chance this event is skipped (not tested for event 0) |
| 0x08 | 8 | real_bounds | delay bounds | s | wait before the event starts |
| 0x10 | 8 | real_bounds | duration bounds | s | time the particles are spread over |
| 0x18 | 20 | pad | | | |
| 0x2C | 12 | block | parts | | 0x68-byte elements, at most 32 |
| 0x38 | 12 | block | particles | | 0xE8-byte elements, at most 32 |

**Part element (0x68).**

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x00 | 2 | enum16 | create in (environment) | | 0 any environment, 1 air only, 2 water only, 3 space (never) |
| 0x02 | 2 | enum16 | create in (mode) | | 0 either mode, 1 violent mode only, 2 nonviolent mode only |
| 0x04 | 2 | index16 | location | | element of the locations block; an index outside it skips the part |
| 0x06 | 2 | flags16 | flags | | bit 0 face down regardless of location (decals) |
| 0x08 | 12 | pad | | | |
| 0x14 | 4 | class code | runtime: base class | | build-time copy of the type's base class: `snd!`, `deca`, `ligh`, `jpt!`, `pctl`, or `obje` for any object; the engine dispatches on it |
| 0x18 | 16 | tag ref | type | | `pctl`, `snd!`, any object, `deca`, `jpt!`, `ligh`; null skips the part |
| 0x28 | 24 | pad | | | |
| 0x40 | 8 | real_bounds | velocity bounds | see Parts | |
| 0x48 | 4 | angle | velocity cone angle | rad | |
| 0x4C | 8 | angle_bounds | angular velocity bounds | rad/s | objects only |
| 0x54 | 8 | real_bounds | radius modifier bounds | | decals only |
| 0x5C | 4 | pad | | | |
| 0x60 | 4 | flags32 | A scales values | | see Scale flags |
| 0x64 | 4 | flags32 | B scales values | | |

**Particles element (0xE8).**

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x00 | 2 | enum16 | create in (environment) | | as parts |
| 0x02 | 2 | enum16 | create in (mode) | | as parts |
| 0x04 | 2 | enum16 | camera mode | | 0 independent of camera mode, 1 first person only, 2 third person only, 3 both first and third |
| 0x06 | 2 | pad | | | |
| 0x08 | 2 | index16 | location | | as parts |
| 0x0A | 2 | pad | | | |
| 0x0C | 8 | real_euler_angles2d | relative direction | rad | yaw, pitch in the marker's frame |
| 0x14 | 12 | real_vector3d | relative offset | WU | forward, left, up in the marker's frame |
| 0x20 | 12 | real_vector3d | runtime: relative direction vector | | build-time (cos yaw cos pitch, sin yaw cos pitch, sin pitch); used as stored |
| 0x2C | 40 | pad | | | |
| 0x54 | 16 | tag ref | particle type | | `part` |
| 0x64 | 4 | flags32 | flags | | bit 0 stay attached to marker; bit 1 random initial angle; bit 2 tint from object colour; bit 3 interpolate tint as HSV; bit 4 ... across the long hue path |
| 0x68 | 2 | enum16 | distribution function | | 0 start, 1 end, 2 constant, 3 buildup, 4 falloff, 5 build up and fall off |
| 0x6A | 2 | pad | | | |
| 0x6C | 4 | short_bounds | count | particles | per location instance |
| 0x70 | 8 | real_bounds | distribution radius | WU | random offset sphere |
| 0x78 | 12 | pad | | | |
| 0x84 | 8 | real_bounds | velocity | WU/s | |
| 0x8C | 4 | angle | velocity cone angle | rad | |
| 0x90 | 8 | angle_bounds | angular velocity | rad/s | sprite spin |
| 0x98 | 8 | pad | | | |
| 0xA0 | 8 | real_bounds | radius | WU | |
| 0xA8 | 8 | pad | | | |
| 0xB0 | 16 | real_argb_color | tint lower bound | | |
| 0xC0 | 16 | real_argb_color | tint upper bound | | |
| 0xD0 | 16 | pad | | | |
| 0xE0 | 4 | flags32 | A scales values | | |
| 0xE4 | 4 | flags32 | B scales values | | |

**Scale flags.** One bit per scalable value; a set bit in the A word
multiplies that value by the effect's scale A, in the B word by scale
B (both may apply). A "bounds" value has two bits: one for the lower
bound (the base) and one for the spread (upper - lower).

| Bit | Parts | Particles |
|---|---|---|
| 0 | velocity (lower) | velocity (lower) |
| 1 | velocity delta | velocity delta |
| 2 | velocity cone angle | velocity cone angle |
| 3 | angular velocity | angular velocity |
| 4 | angular velocity delta | angular velocity delta |
| 5 | type-specific scale (the scale handed to the sound, light, damage or particle system) | count |
| 6 | | count delta |
| 7 | | distribution radius |
| 8 | | distribution radius delta |
| 9 | | radius |
| 10 | | radius delta |
| 11 | | tint (see Particle entries) |

On the disc no part sets any scale bit; particle entries use bits 0
to 3, 6, 7 and 11 (the assault rifle's water smoke tints by A, its
smoke spins by A; impact sparks' speed scales with A).

## Creating an effect

### Who creates effects, and with what

| Creator | Placed at | Scale A | Scale B | Owner |
|---|---|---|---|---|
| a weapon's firing, misfire or empty effect (`weapons.md`) | the weapon object (or its holder when the weapon is invisible), its markers; first-person markers too (below) | the trigger's rate of fire (0..1) | the weapon's error (heat / overheated threshold) | the holder |
| a weapon's ready, reload, chamber, overheated, light toggle effects | same | 0 | 0 | |
| a projectile impact (`projectiles.md` behaviour 17) | the hit point, marker list "normal", "incident", "negative incident", "reflection", "gravity" (all at the hit point; forwards: the surface normal n, -v, v, v reflected about n, straight down); attached to the struck object's hit node when an object was hit | the response's scale: damage scale (scale-by 0) or impact angle x 2/pi (scale-by 1), clamped to [0, 1] | the struck object's remaining shield or body vitality, clamped | the shooter |
| a projectile's detonation, super detonation, hit material's detonation effect | the projectile's position, marker list "" (the projectile's forward) and "gravity" (down) | 0 | 0 | the shooter |
| an object attachment of class `effe` (`objects.md`; grenade smoke, rocket exhaust, plasma grenade glow) | the object's markers, as a **looping** effect | the attachment's primary scale function (1 if none) | its secondary scale function | the object |
| damage on an object (`health_shields.md`: area damage effect, shield depleted, body damaged ...) | the object's markers | 0 | 0 | |
| a damage event's localized effect | the hit point with the impact marker list, attached to the node | 1 | 0 | |
| a material effect (footstep, casing, item, tyre) | the surface point + 0.01 x normal, one marker whose forward is the normal (every location uses it) | the material effect's scale | 0 | none |
| a particle's collision or death effect | the particle's position, markers "velocity" (its direction) and "gravity" (down); inherits the particle's velocity / 30 per tick | collision: speed scale; death: 0 | 0 | none |
| equipment (grenade) detonation | `projectiles.md` | | | |

**A weapon's effect slot may name a sound instead of an effect.** Every
effect reference a weapon or trigger carries (firing, misfire, empty,
ready, reload, chamber, overheated, light toggle, charging) may be an
`effe` tag or a `snd!` tag. For a sound the effect machinery is not
used: the sound plays once as a one-shot sound on the holder (the
weapon's parent unit; a weapon lying on the ground has no holder and
plays nothing), at that object's origin facing its forward direction
(no marker, no particles), with the scale the caller would have given
the effect as the sound's scale (1 for an empty click, the trigger's
rate of fire for a firing or misfire reference). On the disc the five
empty references of the shotgun, plasma rifle, sniper rifle, plasma
pistol and rocket launcher are sounds (`sound\sfx\weapons\assault rifle\dryfire` for
the shotgun, sniper rifle and rocket launcher, `sound\sfx\weapons\plasma
rifle\overheat` for the two plasma weapons); no misfire reference is a
sound on the disc, but the format allows it and an implementation
should take the same path. `weapons.md` lists which slot of which gun
names what.

Scales outside [0, 1] are a caller error. The effect also carries a
colour (white unless the caller tints it; attached looping effects
read an object change colour each update) and, for unattached
effects, an inherited velocity.

### Locations and instances

When an effect is created, for each location in order, the markers of
that name are looked up (at most 16) and each found marker becomes an
**instance**: a matrix (position, forward, left, up) relative to the
marker's node, plus the node, plus whether it is on the first-person
weapon model. For an effect on an object the markers are the object's
model markers (`models.md`); when the object is a weapon held by a
local player who is in first person, the first-person weapon model's
markers of the same names are added as further instances flagged
"first person" (they are added again whenever a weapon becomes the
local player's first-person weapon, and removed when it stops). For a
caller's marker list, each named point and forward becomes an
instance whose up is any unit vector perpendicular to the forward (the
reference picks it from the forward's smallest component; the choice
is invisible for symmetric content). For an effect attached to an
object's node through a marker list (impacts on objects), the points
are converted into that node's frame so the instances ride the node.

When a part, a particle or an event needs a location's instances, the
instance's world frame is its stored matrix composed with its node's
current matrix (the first-person weapon node for first-person
instances).

**Which instances a request sees** (by the particle entry's camera
mode; parts always ask as "independent"):

- first person only, or both while the effect belongs to the single
  local player's first-person weapon: only first-person instances;
- otherwise (independent, third person only, both in any other case):
  only third-person instances.

So every part (sounds, decals, lights, damage) is made at the
third-person model's markers, and a "both" particle comes from the
first-person gun for its owner and from the world model for everyone
else.

### Mode (violent / nonviolent)

An effect made on a biped that died more than 30 ticks ago (a
"corpse") is **nonviolent**; every other effect is violent. A violent
effect skips parts and particle entries whose mode is "nonviolent
only"; a nonviolent one skips "violent only" (blood on corpses: 104
entries on the disc use "violent only").

### Environment

"Air only" is made only where the instance's world point is not under
water; "water only" only where it is; "any" always; "space" never.
The test is per instance (per particle for particle entries), against
the effect's location.

### Visibility

Each update (including the one at creation) an effect checks whether
its location's cluster is potentially visible: to the local players
(ordinary effects) or to any player (required-for-gameplay effects).
A location outside the level counts as not visible. A non-looping
effect that is not visible is **deleted** on the spot, without making
anything; a looping one is marked invisible and makes nothing while
invisible but keeps its timing.

## Event timing

State per effect: the current event index, an "in duration" flag, the
event time t and the current phase length T (a delay or a duration),
the last particle fraction, and per particle entry its drawn count.

- **Creation**: event 0 is set: T = a draw from its delay bounds, t =
  0, "in duration" clear. Then the effect is updated once with dt = 0
  (so zero-delay events happen in the creation tick).
- **Update** with dt (seconds; 1/30 per tick; the creation update uses
  0). Repeat while dt >= 0, at most 8 steps:
  1. If the effect is stopped, stop.
  2. If T - t <= dt: the phase completes; dt -= T - t; t = T.
     Otherwise t += dt and dt becomes -1 (this is the last step).
  3. If in duration: make this step's particles (Particle entries).
     If the phase completed, choose the next event: the loop start
     event if the effect is looping and this is the loop stop event and
     a loop start exists, else index + 1; while that event exists and
     a real draw (effect generator) is below its skip fraction, move to
     the next index. If none is left: a looping effect becomes
     **stopped**, others are **deleted**. Else set that event (its
     delay as above).
  4. Else, if the phase (the delay) completed: set "in duration", t =
     0, last fraction = -1 ("none yet"), T = a draw from the duration
     bounds (effect generator); draw each particle entry's count
     (below); make the event's parts (Parts) unless invisible.
- The "effect generator" is the game-state generator for
  required-for-gameplay effects and the local one otherwise.
- Phase completion in float32: a phase of D seconds completes on the
  update n where D - (n - 1) x f32(1/30) <= f32(1/30) first holds
  (accumulating t in float32): 0.03 s and 1/30 s complete on update 1,
  0.05 s on 2, 0.15 s on 5, 0.2 s on 6, 0.25 s on 8, 0.5 s on 15,
  1 s on 30.
- **Looping** (attachment) effects: each update first reads the
  attachment's primary function. When it is inactive: with flag bit 0
  the effect is deleted and the attachment slot cleared; otherwise the
  effect is stopped (below) unless already stopping or stopped. When it
  is active and the effect is stopped: it restarts at event 0. The
  secondary function is read into scale B, the change colour into the
  colour.
- **Stopping** a looping effect: if a loop stop event exists and is not
  the last event, play continues at the event after it ("stopping");
  otherwise the effect is stopped at once. A non-looping effect is
  deleted when stopped.
- Because of the 8-step cap, a looping effect whose loop events are
  all zero-length repeats 4 times per update (two steps per pass):
  the frag grenade's "smoke" effect (one zero-length event, loop 0-0)
  emits 4 x (1 to 2) smoke particles per tick.

## Parts

Made when their event starts, in block order. For each part whose
type is not null, whose location index is valid and whose mode
matches, for each instance of its location (independent camera mode):
compute the instance's world point P, forward F and up U; if the part
has "face down", F = (0, 0, -1) and U = (1, 0, 0); if the environment
allows it at P, make the part with **part scale** S = 1 x (A if A
bit 5) x (B if B bit 5).

**Random velocity** (used below): magnitude m = lo' + U(0, d') where
lo' and d' are the lower bound and the spread scaled by their bits 0
and 1; cone c = cone angle x its bit 2 scaling; angle a = real draw x
c; direction = F, turned by a about a direction-table axis (drawn only
if a != 0) (rotation of F about the axis by angle a); velocity = m x
direction. Draw order: magnitude, angle, axis.

| Base class | What is made |
|---|---|
| `pctl` | a free particle system at P (Particle systems) with velocity = random velocity (local generator) + the effect's inherited velocity, tint = (alpha 1, the effect colour), scale S |
| `snd!` | an impulse sound (`sounds.md`) with scale S: on the effect's object at the instance's node and relative position when the effect is on an object, else at P with forward F |
| object (`obje` family: on the disc never used) | a new object of that tag at P, forward F, up U, owner the effect's owner; translational velocity = random velocity (**game-state** generator) + inherited velocity (WU/tick); angular velocity = a random direction times U(angular bounds, bits 3 and 4) when the magnitude is not 0 (game-state generator) |
| `deca` | a decal (Decals): trace vector = random velocity (local generator; the "velocity" is the trace length in WU), radius modifier = U(radius modifier bounds) (one local draw even when both are 0; 0 means 1) |
| `jpt!` | an area damage event (`projectiles.md` behaviour 10) at P, epicenter P, direction F, scale S, owner the effect's owner (its player and team) |
| `ligh` | an effect light (Lights): on the effect's object at the instance's node and relative position when there is an object, else at P, forward F; intensity scale S |

The shell casings of every multiplayer weapon are particles, not
objects; no effect on the disc has an object part.

## Particle entries

**Counts.** When an event starts, each entry's count is drawn: n =
trunc(lo' + U(0, d')) with lo' = the count lower bound (x A/B if bit 5)
and d' = upper - lower (x A/B if bit 6), stored in a byte. If n > 6
it becomes trunc((n - 6) / local player count + 6) (no change for one
player). A lower bound above the upper (the disc has "1 to 2" written
as lower 2, upper 1) gives n in [upper, lower].

**Distribution.** At each step while the event is in duration, with
fraction f = t / T (1 when T = 0) and the previous fraction f0 (-1 at
the event's first step), each entry makes
k = trunc(G(f) x n) - trunc(G(f0) x n) new particles per instance,
where G(-1) = 0 and

| Function | G(f) |
|---|---|
| 0 start | 1 (all at the first step) |
| 1 end | 0 while f < 1, then 1 |
| 2 constant | f |
| 3 buildup | f^2 |
| 4 falloff | (2 - f) f |
| 5 build up and fall off | (3 - 2f) f^2 |

**One particle.** For each of the k particles of an instance (instance
frame M_i relative to its node, node matrix M_n):

1. e = distribution radius: lo' + U(0, d') (bits 7, 8).
2. o = a direction draw; local point p = M_i.position + e o +
   M_i (relative offset) (the offset turned into the marker's frame).
3. Random velocity as for parts but around the entry's relative
   direction vector (in the marker's frame), magnitude bits 0 and 1,
   cone bit 2; direction and velocity are then turned by M_i.
4. World point, direction and velocity: through M_n (none for
   unattached effects).
5. Environment test at the world point; if refused, this particle is
   skipped (its later draws are not made).
6. If "stay attached to marker": the particle is attached to the
   effect's object at the instance's node (position and direction kept
   in node space), with no inherited velocity. Otherwise it is free:
   world position and direction, velocity = world velocity + 30 x the
   effect's inherited velocity (WU/tick to WU/s), plus an impulse
   field's push if the creator supplied one (none on the disc's
   callers).
7. radius = lo' + U(0, d') (bits 9, 10); angular velocity = lo' +
   U(0, d') (bits 3, 4); rotation = U(0, 2 pi) if "random initial
   angle", else 0.
8. Tint: fraction q = A^(bit 11) x B^(bit 11) when either tint bit is
   set (no draw), else a real draw. RGB = interpolation from the lower
   to the upper bound's RGB by q (in RGB, or in HSV with flag bit 3,
   the long way round the hue circle with bit 4); alpha =
   (1 - q) x lower alpha + q x upper alpha. With "tint from object
   colour" the RGB is multiplied by the effect's colour.
9. The particle remembers its owner local player and view flags:
   camera mode "third person only" sets "not drawn in first person",
   "first person only" sets "not drawn in third person"; an instance
   on the first-person weapon makes it "attached to the local player"
   (it is then drawn with the first-person weapon's projection).
10. The particle is created (How a particle lives).

## Random draws

Order of draws for one effect, so a test can stub them:

- Creation: event 0 delay (effect generator).
- Event start: duration (effect generator); each particle entry's
  count in block order (local); each part in block order and, inside,
  each instance: `pctl` velocity magnitude, cone angle, axis if the
  angle is not 0 (local); object: velocity magnitude, cone angle, axis,
  angular magnitude, angular axis if the magnitude is not 0 (game
  state); decal: velocity magnitude, cone angle, axis, radius modifier
  (local), then the decal's own draws (Decals); sound, light, damage:
  none.
- Event completion: one skip draw per candidate next event that
  exists (effect generator), even when its skip fraction is 0.
- Each new particle: distribution radius, offset direction, velocity
  magnitude, cone angle, cone axis if the angle is not 0, [environment
  refusal ends here], radius, angular velocity, rotation if random,
  tint fraction unless scaled; then the particle's own (How a particle
  lives).

A range draw is always made even when lower = upper (it consumes one
number).

## The particle tag

Class `part`, struct 0x164 bytes.

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 4 | flags32 | flags | | see Particle flags |
| 0x004 | 16 | tag ref | bitmap | | `bitm`, a sprite group |
| 0x014 | 16 | tag ref | physics | | `pphy` |
| 0x024 | 16 | tag ref | material effects | | `foot`; impact kind 8 is played when the particle hits the level |
| 0x034 | 4 | pad | | | |
| 0x038 | 8 | real_bounds | lifespan | s | |
| 0x040 | 4 | f32 | fade in time | s | |
| 0x044 | 4 | f32 | fade out time | s | |
| 0x048 | 16 | tag ref | collision effect | | `effe` or `snd!`, played on hitting the level |
| 0x058 | 16 | tag ref | death effect | | `effe` or `snd!`, played when the particle dies |
| 0x068 | 4 | f32 | minimum size | pixels | a sprite drawn smaller is scaled up to this diameter |
| 0x06C | 8 | (unused) | | | not read; on the disc 49 of 96 tags hold left-over bytes here (some are big-endian copies of 2.0, 1.0, 30.0, 15.0): ignore |
| 0x074 | 8 | real_bounds | radius animation | factor | radius multiplier at age 0 and at the end of the lifespan |
| 0x07C | 4 | pad | | | |
| 0x080 | 8 | real_bounds | animation rate | frames/s | 0 upper bound: never animates |
| 0x088 | 4 | f32 | contact deterioration | s | added to the frame time on every contact |
| 0x08C | 4 | f32 | fade start size | pixels | not read by the engine |
| 0x090 | 4 | f32 | fade end size | pixels | a sprite whose projected diameter is not above this is not drawn |
| 0x094 | 4 | pad | | | |
| 0x098 | 2 | i16 | first sequence index | | |
| 0x09A | 2 | i16 | initial sequence count | | |
| 0x09C | 2 | i16 | looping sequence count | | |
| 0x09E | 2 | i16 | final sequence count | | |
| 0x0A0 | 8 | pad | | | |
| 0x0A8 | 4 | f32 | runtime: 1 / sprite size | 1/pixels | build-time: 1 / S, S = the largest sprite dimension (max of width and height in page pixels) over the sequences the particle uses (first sequence index to first + initial + looping + final - 1), not over the whole bitmap; 94 of 94 sprite-bitmap particles on the disc match within a pixel (the AR flash's is exactly 1/135 = 0.007407407, f32 0x3BF2B9D6, not a rounded 0.0074; `flash c generic med` stores 1/84 although its bitmap's other sequences hold 98-pixel sprites). Exactly 1.0 on the two particles whose bitmap is not a sprite group (see Size, alpha and drawing) |
| 0x0AC | 2 | enum16 | orientation | | 0 screen facing, 1 parallel to direction, 2 perpendicular to direction |
| 0x0AE | 2 | pad | | | |
| 0x0B0 | 0xB4 | struct | shader | | the effect shader part, below |

**Particle flags (0x000).**

| Bit | Name | Behaviour |
|---|---|---|
| 0 | can animate backwards | each particle plays backwards with a 50 % chance |
| 1 | animation stops at rest | no frame advance once at rest |
| 2 | animation starts on random frame | |
| 3 | animate once per frame | one frame per update (per tick), ignoring the rate |
| 4 | dies at rest | |
| 5 | dies on contact with structure | |
| 6 | tint from diffuse texture | colour multiplied by the level's diffuse colour under it at creation |
| 7 | dies on contact with water | dies when the physics says "in water" |
| 8 | dies on contact with air | dies when "in air" |
| 9 | self-illuminated | not darkened by the level's light at creation |
| 10 | random horizontal mirroring | 50 % chance |
| 11 | random vertical mirroring | 50 % chance |

**Effect shader part (0xB4 bytes; also in particle-system states and
contrails).**

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 0x28 | struct | shader header | `shaders.md` common header; all zero in these embedded copies on the disc |
| 0x28 | 2 | flags16 | shader flags | bit 0 sort bias, bit 1 nonlinear tint, bit 2 don't overdraw first-person weapon (names inferred); with bit 1 clear and a blend function other than alpha blend, the sprite's alpha carries only the fade |
| 0x2A | 2 | enum16 | framebuffer blend function | `shaders.md` 0..7; on the disc 0 (alpha blend) and 3 (add) |
| 0x2C | 2 | enum16 | framebuffer fade mode | 0 none, 1 fade when perpendicular, 2 fade when parallel |
| 0x2E | 2 | flags16 | map flags | bit 0 unfiltered (inferred); 0 on the disc |
| 0x30 | 28 | pad | | |
| 0x4C | 16 | tag ref | secondary map | `bitm`; a "z-sprite" depth map on some particles (`effects\zmaps\...`) |
| 0x5C | 2 | enum16 | secondary map anchor | 0 with primary, 1 with screen space, 2 z-sprite |
| 0x5E | 2 | flags16 | secondary map flags | |
| 0x60 | 0x38 | struct | secondary map animation | `shaders.md` texture animation block |
| 0x98 | 4 | f32 | runtime: secondary map radius | set each frame to the batch's mean particle radius |
| 0x9C | 4 | f32 | z-sprite radius scale | |
| 0xA0 | 20 | pad | | |

A faithful first implementation may ignore the secondary map (it only
softens where sprites cut into geometry).

## Point physics

Class `pphy`, struct 0x40 bytes.

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x00 | 4 | flags32 | flags | | bit 0 flamethrower particle collision; bit 1 collides with structures; bit 2 collides with water surface; bit 3 uses simple wind; bit 4 uses damped wind; bit 5 no gravity |
| 0x04 | 4 | f32 | runtime: mass / r^3 | kg/WU^3 | density x 118613.34 |
| 0x08 | 4 | f32 | runtime: water buoyancy scale | | (1 - density) / (1 + density) |
| 0x0C | 4 | f32 | runtime: air buoyancy scale | | (0.0011 - density) / (density + 0.0011) |
| 0x10 | 16 | pad | | | |
| 0x20 | 4 | f32 | density | water = 1 | |
| 0x24 | 4 | f32 | air friction | | drag coefficient in air |
| 0x28 | 4 | f32 | water friction | | drag coefficient in water |
| 0x2C | 4 | f32 | surface friction | fraction | tangential speed lost per bounce |
| 0x30 | 4 | f32 | elasticity | fraction | normal speed kept per bounce |
| 0x34 | 12 | pad | | | |

The three runtime fields are filled on the disc and match the formulas
on all 30 tags (stones: density 2.4, 284672.03, -0.4118, -0.9991; cold
smoke cloud: 0.00115, 136.405, 0.9977, -0.0222; density 0 gives 0, 1,
1). Use them as stored.

**One physics step** for a point of radius r (WU), velocity v (WU/s),
position x, over dt seconds (nothing happens when dt = 0):

1. Medium: in water if the point is under a water surface of the level
   (`structure_bsp.md` water), else air. Wind w: the level's wind or
   water current at the point when bit 3 or 4 is set, else 0.
2. M = (mass/r^3 + 130.47474 (air) or 118613.34 (water)) x r^3, the
   mass including the displaced medium; b = the medium's buoyancy scale
   (0 with "no gravity"); c = (air or water friction) x r^2.
3. Gravity: v.z += 3.2086613 x b x dt.
4. Drag: k = clamp(dt x c / M, 0, 1) (k = 0 if M = 0 and c = 0, 1 if
   M = 0 and c != 0); v += (w - v) k.
5. Motion with up to 3 bounces: trace from x along v dt against
   front-facing level surfaces (bit 1) and water surfaces (bit 2). No
   hit: x = the end point; done. Hit at fraction s with normal n:
   record "collided with structure" or "with water" and the surface's
   material; split v into its normal part v_n and tangential part v_t;
   v = (1 - surface friction) v_t - elasticity x v_n; x = hit point +
   min(r, 0.005) n; dt -= s dt; repeat.

It reports in air / in water / collided with structure / collided with
water, the last contact normal and material. (The level only; objects
are never hit.)

## How a particle lives

### Creation

Given the entry's data (position, direction, velocity, radius,
angular velocity, rotation, tint, owner, view flags):

1. The world position (through the node for attached particles) must
   be inside the level and potentially visible to a local player;
   otherwise nothing is made. No free slot: nothing is made.
2. Flags drawn (local integer draws, in order, only for the flags the
   tag sets): animates backwards, horizontal mirror, vertical mirror.
3. Lifespan L = U(lifespan bounds); if L > 0.7 it becomes
   (L - 0.7) / local player count + 0.7.
4. Frame span = 1 / U(animation rate bounds) if the upper bound is not
   0, else infinite; frame time = "not started".
5. A free particle's velocity gains the creator's impulse x its mass
   (none on the disc).
6. Unless self-illuminated, the tint RGB is multiplied by the level's
   light at the position (lightmap sample; `structure_bsp.md`); with
   "tint from diffuse texture" also by the diffuse texture colour there.
   This is done once, at creation.
7. Sequence: the initial sequence (first + U(0, initial count)) if the
   initial count > 0, else a looping one (first + initial + U(0,
   looping count)) if the lifespan is not over and the looping count >
   0, else a final one; none at all (or a bitmap without sequences)
   kills the particle at once. The index is clamped to the bitmap's
   sequences.
8. Frame: with "random start frame" a random sprite of the sequence
   (minus one, so the first advance lands on it; plus one for backwards
   particles); else backwards particles start past the last sprite and
   others at "before the first" (-1), so the first update shows sprite
   0 (or the last sprite).

### Each update (dt = the frame's time; 1/30 s per tick on the Xbox)

1. If it has not been drawn for more than 15 frames, delete it.
2. age += dt. If age >= L and it is not new (age was not 0 before this
   update) and the tag has no final sequences: it **dies** (death
   effect, then deletion). So a particle is always updated at least
   once.
3. Animation: unless "stops at rest" and at rest: with "animate once
   per frame", advance one frame per update; otherwise advance whole
   frames while the frame time reaches the frame span (a "not started"
   frame time first advances once). Advancing: next sprite of the
   sequence (backwards: previous); past the end, a new sequence by the
   rule of creation step 7 in its current state (initial, then
   looping while age < L, then final, then death).
4. Physics, if not at rest:
   - Free particle: one point-physics step with the radius at the new
     age. On a structure contact: if it has a collision effect or a
     material effects tag, scale = clamp((|v| - 0.5) / 1.0, 0, 1);
     the collision effect is made (effect or sound at the particle,
     scale A = that scale), and the material effects tag's impact kind
     for the hit material (if within 10 WU of a local camera). Then
     with "dies on contact with structure": dies (death effect only if
     there was no collision effect). "In air" with "dies on contact
     with air", or "in water" with "dies on contact with water": dies.
     Any contact with a normal z > 0.8 marks it "settled" for this
     update; every contact adds the contact deterioration to the frame
     span (slowing the animation).
   - Attached particle: deleted if its object is gone; else its
     velocity is multiplied by clamp(1 - c dt / M, 0, 1) with the air
     values (no gravity, no collision) and position += v dt (in node
     space); always "settled".
   - Then: if |v|^2 < 0.0625 and settled: dies with "dies at rest",
     else becomes **at rest** (no more physics). Otherwise its
     direction becomes its velocity. Rotation += angular velocity x dt.

### Size, alpha and drawing

- Radius at age a: R(a) = r x (A0 + (A1 - A0) a / L), (A0, A1) the
  radius animation bounds.
- Sprite: the current sequence's current sprite, centred on its
  registration point (`bitmaps.md`), rotated by the rotation about the
  view axis, mirrored per the mirror flags. If its projected diameter
  is below the minimum size it is scaled up to it; if not above the
  fade end size it is not drawn.
- **World size of a sprite.** Let R = R(a) in WU, k = the particle's
  "1 / sprite size" field (1/pixels), and (w, h) the current sprite's
  width and height in pixels, (x_r, y_r) its registration point in
  pixels from the sprite's top-left corner (rectangle fractions times
  the page's pixel size, `bitmaps.md`). Then
  - one sprite pixel is s = 2 R k WU;
  - the quad is w s wide and h s tall (WU), its corners at
    -x_r s .. (w - x_r) s along the sprite's x axis and -y_r s .. (h -
    y_r) s along its y axis (y down the picture), relative to the
    particle's position.
  Because k = 1 / S with S the largest sprite dimension among the
  sequences the particle uses, the largest of those sprites spans
  exactly 2R along its longer side and every other sprite is drawn at
  the same pixel scale (smaller sprites look smaller, never stretched).
  **Units:** r (the entry's radius draw) and R are in WU and are a
  radius: the sprite is about 2R across, not R and not R in some other
  unit; the radius animation bounds multiply r (they are factors, not
  sizes); k is 1 / pixels. (A particle system's size has a different
  rule: its scale is WU per sprite pixel, Particle systems, Drawing
  size.)
- **A bitmap that is not a sprite group** (tag type 0, 2D textures:
  the disc's only cases are the two "lens flare" particles below) has
  no sprite rectangles: a sequence is one or more whole bitmaps
  (`bitmaps.md`, Sprite sequences), each drawn whole (texture
  coordinates 0 to 1) as one frame. For such a particle the build tool
  stored k = 1.0 exactly, and the size rule is: **the whole bitmap is
  one unit square, so the quad is 2R by 2R** (centred on the bitmap's
  registration point, which is its centre, (32, 32) of 64 x 64, on both
  disc bitmaps). It is NOT 2R x k x the bitmap's pixel size: that
  would make a 64-pixel bitmap 128 R across (the assault rifle's
  common bullet impact flare, R 0.25 - 0.7, would be a 32 to 90 WU
  square on every hit, and the charged plasma bolt's impact flare, R
  from 2 to 3, 256 to 384 WU). Inferred (see Confidence): k = 1.0 is what the
  sprite rule yields when the whole picture counts as one unit-sized
  sprite, and it is the only reading that keeps "the particle's
  largest frame spans 2R" true for every particle on the disc; the
  sequence index is reduced into the bitmap's sequences as
  `bitmaps.md` says (`flare h stealth cannon` asks for sequence 3 of a
  one-sequence bitmap and gets sequence 0).
- **Worked sizes (from the disc, 1 WU = 3.048 m).**

  | Particle (bitmap, type) | k | Sprite (px) | Entry radius r (WU) | Quad (WU) |
  |---|---|---|---|---|
  | `flare h stealth cannon` (`effects\particles\lens flare\bitmaps\flares weapon lens`, 2D, one 64 x 64 y8 bitmap) | 1.0 | whole bitmap | charged plasma bolt impacts 2.0 - 3.0 | 4.0 - 6.0 square (12.2 - 18.3 m) |
  | the same | 1.0 | whole bitmap | bullet impacts (AR, pistol, shotgun, sniper) mostly 0.25 - 0.7; pistol stone 0.15 - 0.175 | 0.5 - 1.4; 0.30 - 0.35 |
  | `grenade explosion flare` (`...\flares_generic`, 2D, sequence 0 = a 64 x 64 x8r8g8b8 bitmap) | 1.0 | whole bitmap | rocket and tank shell explosions 2.0 | 4.0 square |
  | `flash c generic` (`effects\particles\energy\bitmaps\flash c generic muzzle`, sprites) | 1/98 | 98 x 84, registration (49, 38) | plasma pistol first person 0.04 - 0.05; plasma rifle first person 0.06 - 0.075, third person 0.12 - 0.15 | 0.080 - 0.100 x 0.069 - 0.086; 0.120 - 0.150 x 0.103 - 0.129; 0.24 - 0.30 x 0.21 - 0.26 |
  | `flash c generic med` (same bitmap, sequences 3 - 5) | 1/84 | 84 x 82 | plasma pistol first person 0.065 - 0.08; plasma rifle first person 0.05 - 0.06 and 0.075 - 0.125 | 0.130 - 0.160 x 0.127 - 0.156; 0.10 - 0.12 and 0.15 - 0.25 |
  | `flash c generic small` (sequences 6 - 8) | 1/84 | 84 x 82 | plasma pistol 0.15 - 0.25 (both persons); plasma rifle first person 0.075 - 0.125, third person 0.15 - 0.25 | 0.30 - 0.50; 0.15 - 0.25; 0.30 - 0.50 |
  | `smoke white c generic` / `smoke c generic` | 1/68 | 68 x 54 | plasma pistol 0.075 - 0.15 and 0.015 - 0.025; plasma rifle first person 0.045 - 0.09 and 0.0225 - 0.045 | 2r x (1 by 54/68) |
  | `tracer c generic` (orientation 1) | 1/55 | 32, 55, 54 x 17 (three frames) | plasma rifle 0.08 - 0.16 | longest frame 0.16 - 0.32 long, 0.05 - 0.10 wide |

  The visible picture inside these quads is smaller than the quad:
  on the energy flash sprites the pixels at half the peak brightness
  or more span about 55 of 98 (flash) and 50 to 60 of 84 (med, small)
  pixels; the `flares weapon lens` picture is a bright core about 8
  pixels across (peak 219 of 255), a faint ring and a halo falling
  from about 25 % at 20 pixels from the centre to 0 at the edge. Both
  flare particles draw additively (blend 3); `flare h stealth cannon`
  lives 0.20 - 0.21 s with a 0.27 s fade out longer than its life (so
  its fade starts at about 0.74 and falls to 0), and its charged-bolt
  tints are light green, alpha 1 (RGB from about (0.6, 1.0, 0.58) to
  (0.45, 1.0, 0.44)).
- **View-filling sprites are capped.** For every sprite built (plain
  particles, particle systems, weather, glows), the fraction of the view
  its world-space bounding box covers is estimated (1 when the box
  reaches the camera plane); counting from the start of each view's
  frame, once more than 11 sprites with a fraction above 0.5 have been
  built (the 12th onward), each further such sprite is dropped. Smaller
  sprites are never dropped by this rule.
- Orientation: screen facing (0); parallel to direction (1: the
  sprite's long axis along the particle's direction, turned to face the
  viewer as far as it can); perpendicular to direction (2: the sprite
  lies in the plane normal to the direction). As vectors (inferred, see
  open question 5): p the particle's position, d its unit direction, e
  the unit vector from p to the eye, X and Y the world vectors of the
  sprite's x axis (rightward in the picture) and y axis (downward in
  the picture), each multiplied by the pixel scale s of World size:
  - 0: X = the camera's right vector, Y = the camera's down vector,
    both turned by the particle's rotation about the view axis.
  - 1: X = d (the picture's x axis is its long one on every streak
    sprite of the disc: `tracer c generic` 55 x 17, `sparks energy
    trail` 62 x 25, `shield jackal spark` 72 x 40), Y = normalize(d x
    e). The quad contains the direction and turns about it toward the
    eye; looked at along d it shrinks to a line. The rotation draw does
    not apply.
  - 2: X and Y any orthonormal pair perpendicular to d, turned by the
    rotation about d.
  The direction d is the entry's direction (its relative yaw and pitch
  in the marker's frame, turned into the world) at creation and the
  velocity's direction after each physics step. **The plasma muzzle
  flashes:** in both plasma weapons' fire effects two `flash c generic
  med` entries have pitch +2.094 and -2.094 rad (+-120 degrees) in the
  muzzle marker's frame, d = (-0.5, 0, +-0.866) in (forward, left, up):
  pointing back and up, back and down, moving that way at 0.7 and 1.0
  WU/s; the third `med` and the `small` point along the barrel (d =
  (1, 0, 0)) and so are seen nearly end-on from the first-person
  camera. All of them are "stay attached to marker". The two +-120
  degree streaks and the end-on barrel ones are the plasma muzzle's
  spiky shape; a streak running diagonally across the view from the
  muzzle is expected when the gun is held canted, not an orientation
  bug. The plasma rifle's `tracer c generic` (event 1) starts 0.2 WU
  ahead of the muzzle along the barrel and flies along it at 4 - 6 WU/s
  for its 0.067 s: from the first-person eye it is a short streak from
  the muzzle toward the aim point.
- Fade: fade = 1; if fade in > 0 and a < fade in, fade = a / fade in;
  if fade out > 0 and L - a < fade out, fade x= (L - a) / fade out.
  With a framebuffer fade mode other than none and a non-screen-facing
  orientation, fade x= |n . view| (mode 1) or 1 - that (mode 2).
- Colour: the tint (RGB already lit), alpha = tint alpha x fade; for a
  non-alpha-blend shader without the nonlinear-tint flag the vertex
  alpha is fade alone. Blend per the shader part.
- Batching: particles are drawn grouped by definition, cluster and
  view; a particle first counts as drawn on the frame after its first
  update.
- Views: a particle "not drawn in first person" is hidden from its own
  local player while that player is in first person (or in a gunner
  seat); "not drawn in third person" is shown only to its local player
  in first person.
- **First-person particles.** The size rule (World size of a sprite)
  is the same in both persons, in WU. The effect tags give first- and
  third-person entries their own radii: the plasma rifle's
  third-person flashes are twice its first-person ones (radii and
  offsets; one smoke bound 0.1825 against 0.09), the
  plasma pistol's are the same in both. Entries on the first-person
  weapon's markers are placed in the first-person model's frame,
  which is rooted at the camera (`animations.md`, First person), and
  drawn with the projection the first-person weapon model is drawn
  with (step 9 of One particle). So what fixes their look is their
  size against the first-person gun, and that is in the data: the
  plasma pistol's first-person model is 0.112 WU long (x -0.0215 to
  0.0907), its muzzle markers `primary trigger` and `primary
  trigger01` lie 0.056 WU ahead of `frame gun` (0.0005 and 0.029 WU
  below it) with the gun's own axes; the plasma rifle's is 0.217 WU
  long, `primary trigger` and `primary trigger1` 0.106 WU ahead of
  `frame gun` and 0.02 WU above and below it. The plasma pistol's core
  flash (0.08 - 0.10 WU across) is as wide as the whole first-person
  pistol, and its `small` flash (0.30 - 0.50 WU) about three to four
  and a half times its length; those are the data's sizes, additive
  and lasting one or two frames (each has "animate once per frame"
  and a one-sprite sequence, so it dies on the update after the one
  that shows it, whatever its lifespan). Whether the first-person projection has
  its own field of view or near plane is not settled here (open
  question 11); a port draws these particles with exactly the
  projection it uses for the first-person weapon model, so a
  different first-person field of view scales gun and flash alike.

## Particle systems

Class `pctl`, struct 0x68 bytes. All 7 on the disc are used by the
multiplayer weapons and grenades (frag and plasma grenade, rocket,
charged bolt, needler supercombine and tank shell explosions; rocket
exhaust; flamethrower).

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 0x38 | pad | | |
| 0x38 | 16 | tag ref | point physics | `pphy`: moves a free system's own position each update (radius 1 WU) |
| 0x48 | 2 | enum16 | system update physics | 0 default, 1 explosion (same as default) |
| 0x4A | 2 | pad | | |
| 0x4C | 4 | flags32 | physics flags | 0 on the disc |
| 0x50 | 12 | block | physics constants | 4-byte elements: one f32 each; not read |
| 0x5C | 12 | block | particle types | 0x80-byte elements, 4 used |

**Particle type (0x80).**

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | tag string | name | |
| 0x20 | 4 | flags32 | flags | bit 0 type states loop; 1 forward/backward; 2 particle states loop; 3 forward/backward; 4 particles die in water; 5 in air; 6 on ground; 7 rotational sprites animate sideways; 8 disabled; 9 tint by effect colour; 10 initial count scales with A; 11 minimum count scales; 12 creation rate scales; 13 scale scales; 14 animation rate scales; 15 rotation rate scales; 16 not drawn in first person; 17 not drawn in third person |
| 0x24 | 2 | i16 | initial particle count | made when the system is created |
| 0x26 | 2 | pad | | |
| 0x28 | 2 | enum16 | complex sprite render mode | 0 simple, 1 rotational (all on the disc) |
| 0x2A | 2 | enum16 | sprite orientation | as particles (simple mode) |
| 0x2C | 4 | f32 | radius | WU, collision radius multiplier base |
| 0x30 | 0x24 | pad | | |
| 0x54 | 2 | enum16 | initial particle creation physics | 0 default, 1 explosion, 2 jet |
| 0x56 | 2 | pad | | |
| 0x58 | 4 | flags32 | physics flags | 0 |
| 0x5C | 12 | block | physics constants | f32 elements: explosion (xy spread WU, z spread WU, intensity 1/s); jet (velocity WU/s, spread fraction, rotates up) |
| 0x68 | 12 | block | type states | 0xC0-byte elements, at most 8 |
| 0x74 | 12 | block | particle states | 0x178-byte elements, at most 8 |

**Type state (0xC0).** 0x00 name (tag string); 0x20 duration bounds
(s); 0x28 transition time bounds (s); 0x30 flags32 (0); 0x34 the state
variables: scale multiplier, animation rate multiplier, rotation rate
multiplier (f32 each), colour multiplier (real_argb), radius
multiplier, minimum particle count, particle creation rate (per
second) (f32 each); 0x5C 84 bytes pad; 0xB0 enum16 particle creation
physics; 0xB2 enum16 particle update physics (0 default); 0xB4 block
physics constants (empty on the disc).

**Particle state (0x178).** 0x00 name; 0x20 duration bounds (s); 0x28
transition time bounds (s); 0x30 tag ref bitmap; 0x40 i16 sequence
index; 0x42 6 bytes pad; 0x48 scale bounds (WU per bitmap pixel);
0x50 animation rate bounds (frames/s); 0x58 rotation rate bounds
(rad/s); 0x60 colour lower bound and 0x70 upper (real_argb); 0x80 f32
radius multiplier; 0x84 tag ref point physics; 0x94 0x24 pad; 0xB8 the
effect shader part (0xB4); 0x16C 12 pad.

**Behaviour.**

- Creation (from an effect part): position P, velocity, tint, scale
  (the part scale); lighting sampled at P once. Each type with at
  least one type state starts in state 0 with time left = U(its
  duration). One update of 0.001 s runs at once with "initializing"
  set (a type with no states makes the creation fail).
- Update (dt per frame): a free system moves by its point physics.
  For each type not disabled: time left -= dt; while its state has
  ended: if not transitioning, start a transition to the next state
  (index +1; past the end: loop to 0, or bounce with forward/backward,
  only for attached systems with the loop flag, else the type ends)
  lasting U(transition time); else enter the target state for U(its
  duration). The type's live variables are the state's, or during a
  transition w x current + (1 - w) x next with w = time left /
  length; then multiplied per the scale flags by the system scale
  and the colour by the system tint (flag 9).
- New particles: initializing: the initial count (x scale + 0.5 with
  flag 10); otherwise rate x dt, the fraction carried. At most 128 per
  update, each at a random marker of the attachment (the system's
  position for free systems), created by the creation physics:
  default: at the marker with the system's velocity; **explosion**: a
  direction draw d, scaled by (xy, xy, z spread); position = marker +
  that; velocity = that x intensity + system velocity; spin axis =
  its horizontal part; **jet**: velocity = (1 - spread) V F + spread V
  x a random direction (V = constant 0 x seconds per tick) + system
  velocity. Each new particle draws a random rotation 0..2 pi. While
  the type has fewer particles than its minimum count, its time left
  is multiplied by 0.3 per tick (states hurry on).
- Particles: each enters particle state 0 on its first update (time
  left = U(duration); random variables: animation rate, rotation rate,
  scale, alpha each U(bounds); RGB = lower + q (upper - lower) with
  one shared draw q); states advance like type states (loop with flag
  2/3, else the particle dies at the end); during a transition the
  next state's random variables are drawn at its start. Rotation and
  sprite position advance by the (interpolated) rates x the type
  multipliers x dt.
- **Physics, per particle, per update: the point physics of the
  particle state that particle is in now**, not the first state's: each
  particle state names its own `pphy` (the frag explosion's "fire" and
  "smoke plume" states use `fire explosion`, its last state `smoke
  explosion`), and a particle switches the moment it enters the next
  state. Outside a transition: that state's tag as stored. During a
  transition from state s to state n, with w = time left / transition
  length (1 at the start of the transition, 0 at its end), every
  numeric field (density, the three runtime values, air and water
  friction, surface friction, elasticity) is blended as (1 - w) x s's +
  w x n's, and the flags are s's. **Quirk, keep it:** these weights
  are the reverse of every other blended variable (scale, colour,
  radius use w x s's + (1 - w) x n's): at the start of a transition the
  particle already moves with the next state's physics, drifts back to
  the current state's by the end, then jumps to the next state's when
  it enters it. Radius for the physics = type radius x the type's live
  radius multiplier x the particle state's radius multiplier (blended w
  x s's + (1 - w) x n's in a transition). It dies on the flags 4 to 6
  conditions.
- **Drawing size.** scale k = the particle's random scale (U(the
  state's scale bounds), drawn when it enters the state; in a
  transition w x the current state's draw + (1 - w) x the next state's)
  x the type's live scale multiplier (the type state's, blended between
  type states, x the system's scale only with type flag 13). k is in
  **WU per pixel of the sprite**: the quad spans (sprite width in
  pixels) x k by (sprite height in pixels) x k WU (the engine multiplies
  both the horizontal and the vertical texture-fraction extents by the
  page's **width** in pixels; the disc's pages are square, so this
  matters only for odd pages). Nothing else multiplies it: not the type
  radius, not the radius multipliers (those are physics only), not the
  effect part's scale (unless flag 13), not a 2. Example: a 36 by 36
  pixel `fire cloud` sprite at k = 0.02 is 0.72 by 0.72 WU.
- Colour: the particle's random colour (drawn on entering the state) x the type's
  live colour multiplier, blended likewise; alpha-blended states (blend
  0) are multiplied by the system's lighting; additive ones are not.
  The current state's sprite is drawn with weight w and the next
  state's with 1 - w (each only above 0.01) when they differ in blend
  function, map flags or sequence, else only the current one at weight
  1; the weight multiplies the vertex alpha (or is the vertex alpha for
  a non-alpha-blend shader without the nonlinear-tint flag).
- **Rotational render mode** (complex sprite render mode 1, every
  type on the disc): with theta the angle between the view direction to
  the particle and the particle's spin axis (in view space) minus 90
  degrees, f = clamp(theta^2 / (90 degrees)^2, 0, 1). If f > 0.05 an
  "edge-on" sprite from the next sequence (state's sequence + 1) is
  drawn at weight f (mirrored when theta < 0); if 1 - f > 0.05 a
  "face-on" sprite from the state's sequence is drawn at weight 1 - f,
  its sprite index = (count x rotation / (2 pi) + 0.5) mod count and
  its in-plane rotation the angle of the spin axis in the view plane.
  Both at the same scale k. A bitmap with no next sequence draws only
  the face-on sprite. (The frag explosion's `fire cloud` and `smoke
  cloud` have 60 face-on and 4 edge-on sprites, all about 36 by 36
  pixels.)
- A free system with no live types is deleted.

Disc values (explosions): the frag "explosion med" has one type "fire
to smoke" (flags 0x10: dies in water; rotational render mode; radius
1.5 WU): 35 initial particles, explosion creation with spread 0.012
(xy), 0.08 (z), intensity 200; type state "birth" 1 - 1.5 s then a 4 -
5 s transition to "death" (colour multiplier 0: whatever is left fades
out over those seconds); its particle states:

| State | Duration; transition (s) | Bitmap (sprite size) | Scale k (WU per pixel) | Drawn size (WU) | Colour lower .. upper; alpha | Blend | Physics |
|---|---|---|---|---|---|---|---|
| fire | 0.1 - 0.2; 0.1 - 0.8 | `effects\particles\flash\bitmaps\fire cloud` (36 x 36) | 0.01 - 0.03 | 0.36 - 1.08 | (1, 0.92, 0.64) .. (1, 0.50, 0.28); 1 | add | `fire explosion` |
| smoke plume | 0.3 - 0.6; 0.3 - 0.8 | `effects\particles\air\bitmaps\smoke cloud` (36 x 36) | 0.01 - 0.04 | 0.36 - 1.44 | (0.64, 0.61, 0.57) .. (0.49, 0.46, 0.42); 0.2 - 0.4 | alpha | `fire explosion` |
| death | 0; 0 | smoke cloud | 0.01 - 0.04 | ends at once | grey; 0 | alpha | `smoke explosion` |

A particle lives at most 0.2 + 0.8 + 0.6 + 0.8 = 2.4 s. `fire
explosion`: density 0.0005, air friction 2000, flags 0 (no collisions:
the particles pass through floors), air buoyancy +0.375; at radius 1.5
WU: M = 640.5, drag 0.2342 of the velocity per tick, rise acceleration
1.20 WU/s^2, terminal rise speed 0.17 WU/s. A particle starts at the
explosion point + (0.012, 0.012, 0.08) x d (d a unit direction draw)
with velocity 200 x that offset (up to 2.4 WU/s sideways and 16 WU/s up
or down) and, the drag acting before each move, coasts v0 x (1 - 0.2342)
/ (0.2342 x 30) = v0 x 0.109 s before drag stops it: at most 0.26 WU
sideways and 1.74 WU up or down. So the 35 fire sprites, each 0.36 -
1.08 WU across, form a column about 0.5 WU wide reaching up to 1.8 WU
above (and below) the explosion point: **the fireball is a bright
additive cluster about 1.5 WU (4.5 m) wide and up to about 2.3 WU (7 m)
tall above the ground at its largest (sprite size included), turning to
grey smoke puffs of up to 1.44 WU within a second and gone within about
2.4 s.**

**Drifting smoke** (the explosion effect's only particle entry,
`weapons\frag grenade\effects\drifting smoke`, a plain particle, not a
system): count 10 - 15, radius R = U(1.2, 2.0) WU (radius animation 1
to 1: constant), velocity 1 - 2 WU/s, tint grey (0.39 .. 0.31) with
alpha 0.4 .. 0.6, lit by the level (not self-illuminated), alpha blend;
lifespan 2 - 4 s, fade in 0.3 s, fade out 3.0 s; bitmap
`effects\particles\air\bitmaps\smoke tank cannon` (18 soft puff sprites
on two pages, about 23 to 33 pixels, sprite size field 1/33); physics `smoke explosion`
(density 0.0008, air friction 200, simple and damped wind, buoyancy
+0.158: at R = 1.6 a drag of 0.0185 per tick and a terminal rise of
0.91 WU/s). Drawn size = 2 R x (sprite pixels / 33): **1.7 to 3.4 WU
(5 to 10 m) across each**, the largest sprite exactly 2R. Opacity at
its peak (age 0.3 s): tint alpha x min(1, (L - 0.3) / 3) x the sprite's
own alpha (soft, at most about 0.85 in a puff's middle and 0 at its
edge): 0.23 to 0.6 times the texture's. On screen: from 10 m (3.3 WU)
the 70 degree view is 4.6 WU wide, so each puff spans 37 to 74 percent
of it; from 2 m (0.66 WU) the view is 0.92 WU wide and every puff covers
all of it, so a close explosion **does** fill the view with layered
translucent grey (no puff is opaque and each has soft edges and a
random sprite; a flat, edge-to-edge uniform sheet means the sprite's
alpha or its sub-rectangle was not used), and at most 11 such
view-filling sprites are drawn per frame (How a particle lives, Size,
alpha and drawing).

The
plasma grenade's has 10 initial, 15 per second while in "birth" (0.25
s), cyan colours, all additive. The tank shell's: 30 initial, spread
0.03 / 0.08 / 380. Full values come from the tags.

## Contrails

Class `cont`, struct 0x144 bytes. 8 on the disc, all on projectiles:
`weapons\assault rifle\bullet` (assault rifle and Warthog bullets),
`vehicles\scorpion\bullet`, `weapons\sniper rifle\sniper`,
`vehicles\scorpion\shell`, `weapons\rocket launcher\exhaust`,
`weapons\plasma rifle\plasma` (plasma rifle and Ghost bolts),
`weapons\plasma rifle\plasma charged bolt`, `weapons\needler\needler`.

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 2 | flags16 | flags | | bit 0 first point unfaded; bit 1 last point unfaded; bits 2 to 5 pinning to media/ground (inferred names, 0 on the disc); bit 6 fades slowly (the view-angle fade goes through transition 2) |
| 0x002 | 2 | flags16 | scale flags | | multiply by the attachment's density: 0 point generation rate, 1 point velocity, 2 velocity delta, 3 cone angle, 4 inherited velocity fraction, 5 sequence animation rate, 6 texture scale u, 7 texture scale v, 8 texture animation u, 9 texture animation v |
| 0x004 | 4 | f32 | point generation rate | points/s | |
| 0x008 | 8 | real_bounds | point velocity | WU/s | |
| 0x010 | 4 | angle | point velocity cone angle | rad | around the marker's forward |
| 0x014 | 4 | fraction | inherited velocity fraction | | of the object's velocity (per tick, as stored) |
| 0x018 | 2 | enum16 | render type | | 0 vertical orientation, 1 horizontal orientation, 2 media mapped, 3 ground mapped (not drawn), 4 viewer facing (all on the disc) |
| 0x01A | 2 | pad | | | |
| 0x01C | 4 | f32 | texture repeats u | per point | |
| 0x020 | 4 | f32 | texture repeats v | across | |
| 0x024 | 4 | f32 | texture animation u | repeats/s | |
| 0x028 | 4 | f32 | texture animation v | repeats/s | |
| 0x02C | 4 | f32 | animation rate | frames/s | |
| 0x030 | 16 | tag ref | bitmap | | |
| 0x040 | 2 | i16 | first sequence index | | |
| 0x042 | 2 | i16 | sequence count | | |
| 0x044 | 64 | pad | | | |
| 0x084 | 0xB4 | struct | shader | | the effect shader part |
| 0x138 | 12 | block | point states | | 0x68-byte elements, at most 16 |

**Point state (0x68).** 0x00 duration bounds (s); 0x08 transition
duration bounds (s); 0x10 tag ref `pphy` physics (null: points do not
move); 0x20 32 pad; 0x40 f32 width (WU, full width); 0x44 colour lower
and 0x54 upper (real_argb); 0x64 flags32 scale flags: bits 0 to 3
duration, duration delta, transition duration, delta; bit 4 width;
bit 5 colour (multiplied by the point's density).

**Behaviour.**

- A contrail is an object attachment. Its density is the attachment's
  primary scale function (1 when none; projectiles have none). It
  emits while that function is active; a change of activity forces
  one point.
- Points: the time until the next point counts down by the tick's
  time; each time it runs out one more point is due and the interval
  1 / rate restarts. Points are emitted once per tick (after the
  object moved), at each marker of the attachment's name (up to 4
  ribbons). k due points are spread evenly from the previous newest
  point to the marker: point i of k at fraction i/k (the last at the
  marker), with density and velocity interpolated likewise. A marker
  that has not moved emits nothing unless forced. Velocity = inherited
  fraction x object velocity + a random direction in the cone x
  U(velocity bounds). A projectile that hits something emits its last
  points to the hit point first.
- Point life: a new point enters state 0 on its first update. **Each
  point draws its own lengths, one draw per state and per transition,
  at the moment it enters that state or transition** (not the mean of
  the bounds, not one draw per contrail): length = lower x f0 + U(0,
  (upper - lower) x f1) seconds from the local (presentation) random
  generator, f0 = the point's density if the state's scale-flag bit for
  that bound is set (bit 0 duration, bit 2 transition duration), else
  1, and f1 likewise with bit 1 (duration delta) or bit 3 (transition
  delta); the draw is made even when lower = upper. Each update the
  point's progress grows by dt / length; when it passes 1 (strictly),
  or at once when the length is 0, the point moves on and its progress
  restarts at 0 (the overshoot is dropped, not carried): from a state
  to the transition toward the next state, from a transition into that
  next state; several zero lengths are passed in the same update. When
  the point's **last** state ends (its own duration, after the
  transition into it) it **expires**; a last state of duration 0
  therefore expires the update the transition into it ends. During a
  transition, the progress is the interpolation fraction for width and
  colour. Each tick
  every live point except those just made runs its state's point
  physics (radius = width / 2). Expired points at the tail are freed
  while the point before them is expired too (one expired point stays
  as the ribbon's end).
- Width and colour of a point: the state's width and lower + (upper -
  lower) x (density if colour bit, else 1); during a transition
  interpolated toward the next state by the transition's elapsed
  fraction.
- Ribbon: consecutive points joined; each point gives two vertices
  half the width either side, perpendicular to the segment and (render
  type 4) to the eye direction. u advances by -repeats u per point from
  the texture offset (which moves by -animation u x dt); v spans the
  offset to offset + repeats v. Vertex alpha x the view-angle fade when
  the shader has a fade mode; the newest point's vertices and the
  oldest's are alpha 0 unless flagged unfaded. Blend per the shader.
- Disc example, the assault rifle tracer: 60 points per second (2 per
  tick), state 0 (duration 0, width 0.05 WU, colour (1, 0.89, 0.28)
  alpha 1) transitions in 0.01 - 0.0125 s to state 1 (black): each
  point is drawn one tick at full colour and expires at the next
  update, so the visible tracer is a 0.05 WU additive streak covering
  the bullet's path over its latest tick, its tail fading to black.
- **Geometry, summarised** (what a renderer needs; the ribbon rule
  above is from the reference, the attachments and numbers from the
  disc):
  - **Where points start.** On the disc every contrail hangs on a
    projectile, at the empty marker name (the projectile's origin)
    except the rocket's, at its "exhaust" marker; none is on a weapon,
    so no contrail is part of any first-person or muzzle effect. A new
    point starts at the marker position after the projectile moved
    this tick (k due points spread back along the segment from the
    previous newest point).
  - **Which way the strip runs.** Along the chain of points, newest
    to oldest: the path the marker has travelled (for a projectile,
    its trajectory), not the marker's forward axis and not any
    velocity. Each point drifts on its own afterwards: initial speed
    U(point velocity bounds) along a direction in the cone around the
    marker's forward (the projectile's direction of flight), plus
    the inherited fraction of the object's velocity, then its state's
    point physics; on the disc the bounds are 0 - 0.05 WU/s (0 - 5 for
    the rocket exhaust), the cone 0 and the inherited fraction 0, so
    the strip stays on the trajectory except for the rocket smoke.
  - **How the width faces the camera.** Render type 4 (all 8 on the
    disc): at point i with neighbour segment direction t_i (from the
    point toward the next newer one; the newest point uses the segment
    to its predecessor) and e_i the unit vector from the point to the
    eye, the two vertices are p_i +- (w_i / 2) normalize(t_i x e_i): the
    ribbon is flat, contains the path, and turns about it toward the
    eye. Seen end-on (eye on the path) it collapses to a line.
    (Neighbour averaging of t at interior points, and the treatment
    of t parallel to e, are inferred, not read.)
  - **Point size.** A point has no size of its own beyond its full
    width w (WU) from its state (and transition): plasma rifle bolt
    0.1 at birth, 0.035 after 0.125 s, then over 1.0 - 1.2 s to 0.0125
    and black; charged plasma bolt 0.5, then 0.2, then 0.05 (10 points
    per second, texture moving at -3 repeats/s); needler 0.04 to
    0.008; assault rifle and Warthog 0.05; sniper 0.025, 0.044, 0.088;
    tank shell 0.05, 0.1, 0.15; rocket 0.15, 0.2, 0.3125, 0.5. All
    contrail bitmaps are 2D textures (one 32 x 32 or 64 x 64 bitmap),
    stretched as u (repeats u per point) along the strip and v across
    it.

## Lights

Class `ligh`, struct 0x160 bytes. 32 on Blood Gulch.

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 4 | flags32 | flags | | bit 0 dynamic; bit 1 no specular; bit 2 don't light own object; bit 3 supersize in first person; bit 4 first-person flashlight; bit 5 don't fade with active camouflage |
| 0x004 | 4 | f32 | radius | WU | |
| 0x008 | 8 | real_bounds | radius modifier | factor | at intensity 0 and 1 |
| 0x010 | 4 | angle | falloff angle | rad | spot cone (inner) |
| 0x014 | 4 | angle | cutoff angle | rad | spot cone (outer) |
| 0x018 | 4 | f32 | lens flare only radius | WU | |
| 0x01C | 4 | f32 | runtime: cos falloff | | |
| 0x020 | 4 | f32 | runtime: cos cutoff | | |
| 0x024 | 4 | f32 | specular radius multiplier | | (0 on the disc) |
| 0x028 | 4 | f32 | runtime: sin cutoff | | |
| 0x02C | 8 | pad | | | |
| 0x034 | 4 | flags32 | colour interpolation flags | | bit 0 blend in HSV; bit 1 more colours (long hue path) |
| 0x038 | 16 | real_argb_color | colour lower bound | | colour at intensity 0 |
| 0x048 | 16 | real_argb_color | colour upper bound | | colour at intensity 1 |
| 0x058 | 12 | pad | | | |
| 0x064 | 16 | tag ref | primary cube map (gel) | | `bitm` |
| 0x074 | 2 | pad | | | |
| 0x076 | 2 | enum16 | texture animation function | | periodic |
| 0x078 | 4 | f32 | texture animation period | s | |
| 0x07C | 16 | tag ref | secondary cube map | | |
| 0x08C | 2 | pad | | | |
| 0x08E | 2 | enum16 | yaw function | | periodic |
| 0x090 | 4 | f32 | yaw period | s | |
| 0x094 | 2 | pad | | | |
| 0x096 | 2 | enum16 | roll function | | |
| 0x098 | 4 | f32 | roll period | s | |
| 0x09C | 2 | pad | | | |
| 0x09E | 2 | enum16 | pitch function | | |
| 0x0A0 | 4 | f32 | pitch period | s | |
| 0x0A4 | 8 | pad | | | |
| 0x0AC | 16 | tag ref | lens flare | | `lens` |
| 0x0BC | 24 | pad | | | not read (two tags hold leftovers at 0xBC/0xC0) |
| 0x0D4 | 4 | f32 | intensity | | effect parameters: not read by the engine (0 or 0.01 to 0.02 on 5 tags) |
| 0x0D8 | 12 | real_rgb_color | colour | | not read |
| 0x0E4 | 16 | pad | | | |
| 0x0F4 | 4 | f32 | duration | **ticks** | effect lights: lifetime |
| 0x0F8 | 2 | pad | | | |
| 0x0FA | 2 | enum16 | falloff function | | transition function over the lifetime |
| 0x0FC | 100 | pad | | | |

**Attached lights** (object attachments of class `ligh`; the muzzle
flash light, flashlights, the frag and plasma grenade glow, plasma
bolt lights). Made only for "dynamic" lights or ones with a lens
flare. Each frame, intensity i = the attachment's primary function
(1 if none); colour = interpolate(lower, upper, i) (RGB, by the
interpolation flags) x the change colour (white if none); radius =
(modifier lower x (1 - i) + modifier upper x i) x radius. A light
whose colour is black, or whose radius is 0, is not drawn. On a unit
with active camouflage c > 0 (unless flag bit 5) the colour is
multiplied by 1 - c. A first-person flashlight is placed at the
first-person weapon's flashlight, and a light on a held weapon at the
first-person weapon's marker, for its own player.

**Effect lights** (effect parts of class `ligh`). Remember the tick
they were made (T0) and the part scale S. Each frame, with elapsed e =
current tick - T0: if e > duration the light is deleted; else
i = (1 - transition(falloff function, e / duration)) x S, colour =
interpolate(lower, upper, i), radius as above. On an object they ride
its node.

**The muzzle flash light**, in numbers (`weapons\assault rifle\muzzle
flash`, used by the assault rifle, pistol, shotgun, rocket launcher,
Warthog and Scorpion guns): flags dynamic + don't fade with camouflage,
radius 2.0 WU, modifiers 1 and 1 (so always 2 WU), colour from black
to (0.961, 0.910, 0.686), attached at "primary trigger" with the
weapon's outgoing function B (assault rifle, shotgun) or A (pistol),
which is the weapon's "illumination" export (`weapons.md`): 1 on the
shot's tick, then falling by 1/(30 x 0.1) = 1/3 per tick: intensity
1, 2/3, 1/3, 0: three ticks of light, linearly dimming. The plasma
rifle's (`weapons\plasma rifle\muzzle flash`, at "secondary trigger",
function C, recovery 0.15 s: 1/4.5 per tick) is (0.5, 1, 1) blue,
radius 2; plasma pistol (0.082, 1, 0.059) green; needler (1, 0.216,
1); sniper (0.961, 0.909, 0.686) radius 3.

Lights per frame: at most 128 are drawn (the ones the visibility pass
finds first); how a light lights surfaces belongs to the renderer
(`shaders.md`, `structure_bsp.md`).

## Lens flares and glows

**Lens flare** (`lens`, 0xF0 bytes; 22 on Blood Gulch), drawn where a
light (or the sun) shines toward the camera:

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | angle | falloff angle | full brightness inside |
| 0x04 | 4 | angle | cutoff angle | none outside (0 and 0: omnidirectional) |
| 0x08 | 8 | 2 f32 | runtime cosines | |
| 0x10 | 4 | f32 | occlusion radius | WU, visibility probe size |
| 0x14 | 2 | enum16 | occlusion offset direction | 0 toward viewer, 1 marker forward, 2 none |
| 0x18 | 8 | 2 f32 | near, far fade distance | WU (0 0: no distance fade) |
| 0x20 | 16 | tag ref | bitmap | |
| 0x30 | 2 | flags16 | flags | |
| 0x80 | 2 | enum16 | corona rotation function | 0 none, 1 rotation, 2 rotation reflected, 3 rotation translated, 4 (5 values) |
| 0x84 | 4 | f32 | corona rotation function scale | 360 (degrees) on most tags |
| 0xA0 | 8 | real_vector2d | corona radius scale | |
| 0xC4 | 12 | block | reflections | 0x80-byte elements |

Reflection (0x80): 0x00 flags16, 0x02 type, 0x04 bitmap index, 0x1C
offset (along the line from the flare through the screen centre),
0x20 rotation offset, 0x28 radius bounds and 0x30 radius scale
function (0 none, 1 rotation, 2 rotation reflected, 3 translation),
0x34 brightness bounds and 0x3C brightness scale function, 0x40 tint
(argb), 0x50 animation colour bounds (argb x 2), 0x70 animation flags,
0x72 animation function (periodic), 0x74 period, 0x78 phase. Bytes
between are padding (zero on the disc). Behaviour beyond this layout
is left to the renderer spec (open question 8); the grenade
quickflash's flare (12 WU and 3 WU reflections, brightness 0 to 1) and
the plasma bolts' flares are the multiplayer ones.

**"Lens flare" particles are not lens flares.** The particles under
`effects\particles\lens flare\` (`flare h stealth cannon`, `grenade
explosion flare`) are ordinary `part` sprites: they share bitmaps with
the `lens` tags but none of a lens flare's fields (corona radius
scale, reflection radius bounds, occlusion, fade distances) applies to
them; their size is the particle rule (How a particle lives, World
size). Every `lens` bitmap on Blood Gulch is a 2D-texture tag (type
0), never a sprite group; a reflection picks one bitmap by its bitmap
index. Disc values for comparison (each with corona radius scale (1,
1), except the needler explosion's (0.75, 2.5)): the plasma rifle
bolt's and Ghost bolt's `lens` use `flares weapon lens` bitmap 0 with
one reflection of radius 0.125 (occlusion radius 0.03); the charged
bolt's, the same bitmap at radius 0.5 (occlusion 0.125); the `lens`
tags `weapons\plasma pistol\plasma pistol lens flare` and `weapons\plasma
rifle\plasma rifle lens flare` (`soft flares`) 0.019 and 0.05; the needler's 0.003; the AR flashlight's 0.7 and 0.2. So the
charged bolt's flare in flight has a reflection radius of 0.5 and its
impact particle a radius of 2 - 3 WU. What unit a reflection radius is
in (WU at the flare, or a fraction of the screen) belongs to open
question 8.

**Glow** (`glw!`, 0x154 bytes; none on Blood Gulch, so layout from the
reference only): 0x00 attachment marker (tag string); 0x20 i16 number
of particles; 0x22 boundary effect; 0x24 particle distribution; 0x26
trailing particle distribution; 0x28 flags32; 0x48 render mode; 0x4A
render orientation; 0x4C render flags; then five "attachment-scaled"
groups, each an i16 attachment index (+2 pad) and f32 values: 0x50
particle rotational velocity (value, scale bounds), 0x60 effect
rotational velocity, 0x70 effect translational velocity, 0x80 distance
to object (minimum, maximum, scale bounds); 0x9C particle size (size
bounds, scale bounds); 0xB0 colour (lower, upper, scale lower, scale
upper argb at 0xB4..0xF3); 0xF4 colour rate of change; 0xF8 percentage
of edge fade; 0xFC trailing particle generation frequency, lifetime,
velocity, minimum t, maximum t; 0x144 texture (`bitm`).

## Decals

Class `deca`, struct 0x10C bytes. 39 on Blood Gulch.

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 2 | flags16 | flags | | bit 0 geometry inherited by next decal in chain; 1 interpolate colour in HSV; 2 ... more colours; 3 no random rotation; 4 water effect; 5 snap to axis (editor); 6 incremental counter (editor); 7 animation loop; 8 preserve aspect |
| 0x002 | 2 | enum16 | type | | 0 scratch, 1 splatter, 2 burn, 3 painted sign |
| 0x004 | 2 | enum16 | layer | | 0 primary, 1 secondary, 2 light, 3 alpha-tested, 4 water (names inferred); drawn in layer order |
| 0x006 | 2 | pad | | | |
| 0x008 | 16 | tag ref | next decal in chain | | `deca`, made right after this one |
| 0x018 | 8 | real_bounds | radius | WU | half size |
| 0x020 | 12 | pad | | | |
| 0x02C | 8 | fraction_bounds | intensity | | the decal colour's alpha |
| 0x034 | 12 | real_rgb_color | colour lower bound | | |
| 0x040 | 12 | real_rgb_color | colour upper bound | | |
| 0x04C | 12 | pad | | | |
| 0x058 | 2 | i16 | animation loop frame | | not read by the engine |
| 0x05A | 2 | i16 | animation speed | ticks/frame | not read (1 on the disc) |
| 0x05C | 28 | pad | | | |
| 0x078 | 8 | real_bounds | lifetime | s | 0 = permanent |
| 0x080 | 8 | real_bounds | decay time | s | fade-out length |
| 0x088 | 12 | pad | | | |
| 0x094 | 0x28 | struct | shader header | | `shaders.md` header; zero |
| 0x0BC | 2 | flags16 | shader flags | | 0 |
| 0x0BE | 2 | enum16 | shader type | | 0 |
| 0x0C0 | 2 | enum16 | framebuffer blend function | | `shaders.md`: on the disc 1 multiply (blood), 2 double multiply (bullet holes, burns), 3 add (plasma glows), 6 component max (alien blood) |
| 0x0C2 | 22 | pad | | | |
| 0x0D8 | 16 | tag ref | map | | `bitm` (sprites: one sequence per variant) |
| 0x0E8 | 20 | pad | | | |
| 0x0FC | 4 | f32 | runtime: maximum sprite extent | | 16.0 on all tags |
| 0x100 | 2 | u16 | runtime: incremental counter | | editor |
| 0x102 | 10 | pad | | | |

**Placement** (a decal part: origin P, trace vector V, radius modifier
k):

1. Trace from P along V against front-facing level surfaces and water
   surfaces only (objects are never hit; a decal on a body lands on
   whatever level surface is behind or below it). Nothing hit: no
   decal. A water hit makes a water decal only for "water effect"
   decals (and a level hit is ignored by them).
2. Frame on the hit surface (normal n): unless "no random rotation"
   (and V points into the surface), the decal is rotated by a random
   angle U(0, 2 pi) about n; with the flag, its forward follows the
   projection of V. Sequence: a random sequence of the map (integer
   draw over the sequence count); radius = U(radius bounds) x k (k = 1
   if 0).
3. The decal is the square (or the sprite's rectangle, scaled so the
   sprite's registration point sits at the hit point; with "preserve
   aspect" on a non-sprite map the height is radius x bitmap height /
   width) in the plane through the hit point with normal n.
4. Clipping: the square is projected onto the hit surface's polygon
   and clipped to it. For every edge of a surface the square crosses
   whose segment comes within 1.5 x radius of the centre, the
   neighbouring surface across that edge is queued (each once, at most
   1024). A queued surface whose normal is within 40 degrees of the
   decal's is clipped with the same projection; one between 40 and 110
   degrees is a "deviant": for scratches, splatters and burns, deviant
   surfaces are grouped (within 40 degrees of each other) and the
   decal is folded over the shared edge (rotated about the edge nearest
   the decal plane by the angle between the planes) and clipped onto
   them; painted signs use 10 degrees for both limits and never fold.
   If nothing is left, no decal.
5. The clipped polygons become quads (a fan of each polygon, two
   triangles per quad) in the decal vertex cache, pushed off the
   surfaces by a small depth offset along the mean normal when all
   normals agree within 0.5 per axis. Texture coordinates come from
   the projection, inside the chosen sprite's rectangle.
6. Record: lifetime = U(lifetime bounds), decay = U(decay bounds),
   colour = interpolate(lower, upper, a draw) with alpha = U(intensity
   bounds), creation tick. With a chain, the next decal is made the
   same way (reusing this frame and radius if flag bit 0).

Draw order in step 2 to 6: rotation angle (if random), sequence,
radius, the lock draw (Limits), lifetime, decay, intensity, colour
fraction.

**Lifetime.** Each frame: elapsed = ticks since creation / 30. While
elapsed < lifetime (or lifetime is 0): fully opaque except in the last
"decay" seconds, where the decal's intensity byte is trunc((lifetime
- elapsed) / decay x 255) (rounded as the hardware rounds; the
renderer multiplies it in). Once elapsed >= lifetime its geometry is
freed and it is gone. Bullet holes: lifetime 180 s, decay 20 s (stone
300 s and 1 s); plasma glows 1 - 2.75 s.

**Limits.** Decal records 2048; decal geometry in a 2560-page cache of
64-byte pages (one quad per page): a new decal takes pages from the
least recently used unlocked decals (which disappear). About 10 % of
new decals are "locked" (an integer draw below 6553.5) and are not
evicted; when more than 512 are locked, locks are released at random
(an integer draw below 26869.35, about 41 %, or any decal outside the
level) until 256 remain.

## Material effects

Class `foot`, struct 0x0C bytes: one block "effects" of 0x1C-byte
elements (at most 13), one per **kind**; each kind element is a block
"materials" (12 bytes) then 16 bytes pad; each material element (0x30
bytes, one per material type in code order, at most 33) is an `effe`
reference at 0x00, a `snd!` reference at 0x10, and 16 bytes pad.

| Kind | Name (names inferred where marked) | Who plays it |
|---|---|---|
| 0 | walk | biped footstep while crouching |
| 1 | run | biped footstep while standing |
| 2 | sliding | biped slipping (more than 3 ticks, sliding speed above 1/30 WU/tick), both feet |
| 3 | shuffle | biped turning in place (first frame of the turn animation) and stopping (4 ticks after the move ends), both feet |
| 4 | jump | biped jump, both feet |
| 5 | jump land | biped landing (2 ticks into the landing recovery, or at once for short recoveries), both feet |
| 6 | biped unharmed land (inferred) | not played by the engine |
| 7 | biped injured land (inferred) | not played by the engine |
| 8 | impact | an item (dropped weapon, grenade item) hitting the level; a particle with this tag hitting the level |
| 9 | vehicle tire slip | vehicles (`vehicles.md`) |
| 10 | vehicle chassis slip (inferred) | vehicles, mass points flagged so |
| 11, 12 | (unknown) | not played |

**Playing one** (tag, kind, material m, point p, normal n, scale s):
if the kind exists and m is below its materials count, the material
element's effect is made at p + 0.01 n facing n (scale A = s, B = 0,
not allowed to carry gameplay parts) and its sound played there with
scale s. Callers first require the point to be within 10 WU of a local
player's camera (always true with more than two local players).

- **Footsteps**: on the animation's left- and right-foot frames while
  moving (throttle squared above 0.25), for the foot's contact point
  (the biped's contact point markers "left foot", "right foot",
  `objects.md`): probe from the marker + 0.15 WU up, 0.3 WU straight
  down against the level and objects; on a hit, material = the hit
  surface's, or water (28) when the hit point is under water; scale 0.
  The multiplayer Spartan references `globals\cyborg` (6 kinds, 32
  materials: footstep sounds per surface, and `effects\footsteps water`
  splashes on water).
- **Items**: a dropped weapon whose move hits the level plays kind 8
  of its own material effects (`sound\sfx\impulse\material_effects\
  weapon` for the hand weapons) with scale clamp(speed / 0.1 WU per
  tick, 0, 1).
- **Particles**: kind 8 (casings: `...\shellcasings\ar_casings` for
  the assault rifle and pistol casings, `...\shotgun_casings`) with the
  collision speed scale.

## Damage effect presentation

The fields are `projectiles.md`'s damage effect layout (0x024 to
0x123). When a damage event reaches a local player's unit (and when a
weapon applies its firing damage effect to its holder), with event
scale s and per-part zero scale factor z, intensity = z + (1 - z) s.
All three parts below are per local player; durations are converted
to ticks as (short)(30 x seconds) (truncation) for the countdown.

**Screen flash.** Types 0 none (no flash), 1 lighten, 2 darken, 3 max,
4 min, 5 invert, 6 tint; priority 0 low, 1 medium, 2 high. A new flash
replaces the current one unless the current has a higher priority and
more ticks left than the new one's duration. Stored intensity I =
clamp(z + (1 - z) s, 0, maximum intensity). Each frame while ticks
remain (and on the frame it starts): shown intensity =
transition(fade function, I x ticks left / duration ticks) (I when the
duration is 0), with the flash colour (argb), then the ticks count
down by the frame's elapsed ticks. The renderer blends the screen with
the colour by type and intensity (`hud.md`).

**Camera impulse (temporary).** Replaces the current one when the
current has already started counting down, or the new intensity is
higher, or equal with more time. The impulse direction is the event's
direction flattened to the horizontal plane and expressed relative to
the player's facing (angle phi from the facing); jitter = a vector
perpendicular to that direction of length U(jitter bounds) turned by a
random angle about it. Each frame while ticks remain: k = 1 on the
first frame, else transition(fade function, 1 - elapsed / duration) x
intensity; the view is rotated by k x rotation about the axis up x
direction and translated by k x (pushback x direction + jitter).

**Camera impulse (permanent).** At once: the player's desired aim
changes by yaw += (left . d) x angle x i_p and pitch += (forward . d)
x angle x i_p, with d the event direction, left and forward the
player's, i_p = permanent zero scale factor + (1 - it) x s. (The frag
grenade and tank shell explosions: 0.0698 rad.)

**Camera shake.** Replaces the current one when the new duration is
longer than the time left, or the new intensity is higher. Each frame
while ticks remain: k = 1 on the first frame, else transition(falloff
function, 1 - elapsed / duration) x intensity; magnitude = k x
(periodic(wobble function, elapsed / (30 x wobble period)) x wobble
weight + 1 - wobble weight); the view gets a rotation by magnitude x
random rotation about a random direction and a translation of
magnitude x random translation in a random direction (two direction
draws from the local generator, once per tick).

**Rumble.** The low- and high-frequency motor parts (frequency 0..1,
duration s, fade transition function) set the controller motors,
scaled by the event's intensity; the "vibrate zero scale factor" is
their z. (Presentation only.)

**Firing kick.** A weapon applies its trigger's firing damage effect
(`weapons\<weapon>\trigger`) to its holder at every shot (direction =
minus the aim, `weapons.md`); these effects do no damage and carry
only presentation:

| Firing damage effect | Shake (duration s, falloff, translation WU, rotation rad, wobble) | Rumble low (freq, s, fn) | Rumble high (freq, s, fn) | Flash / impulse |
|---|---|---|---|---|
| `weapons\assault rifle\trigger` | 0.05 (1 tick), linear, 0, 0.00873 (0.5 deg), none | none | 0.75, 0.1, linear | none |
| `weapons\pistol\trigger` | 0.2 (6 ticks), very late, 0, 0.00349, none | 0.4, 0.4, linear | 1.0, 0.3, linear | none |
| `weapons\shotgun\trigger` | 0.2 (6 ticks), late, 0.0325, 0, cosine variable period 6 s weight 0.5 | 1.0, 0.1, linear | 1.0, 0.3, linear | none |
| `weapons\plasma rifle\trigger` | none | 0.4, 0.1, early | 1.0, 0.08, early | none |
| `weapons\sniper rifle\trigger` | 0.2, early, 0.025, 0, cosine variable 6 s weight 0.2 | 0.1, 0.1, linear | 0.4, 0.2, very late | none |
| `weapons\rocket launcher\trigger` | 1.0, linear, 0.03, 0.0349 | 0.3, 1.0, cosine | 0.3, 1.0, very early | none |
| `weapons\plasma pistol\trigger` | none | 0.5, 0.2, early | 0.8, 0.1, early | none |
| `weapons\plasma pistol\trigger overcharge` | none | 0.5, 0.3, early | 0.8, 0.125, early | flash lighten 1.5 s late, max 0.5, colour (0.008, 0.384, 0) green; impulse 0.2 s early, rotation 0.0262, pushback 0.1, jitter 0.01 |
| `weapons\needler\trigger` | none | 0.4, 0.1, linear | 0.8, 0.1, early | none |
| `vehicles\warthog\trigger` | 0.05, linear, 0, 0.00436 | 0.5, 0.1, early | 0.25, 0.1, very early | none |
| `vehicles\scorpion\shell shock wave` | 0.5, late, 0.05, 0.01745, cosine variable 6 s weight 0.6 | 1.0, 1.2, late | 1.0, 1.0, late | radii 10 - 15 WU (area event) |

All zero scale factors are 0 and "maximum intensity" 0.5 where no
flash is set (type none, so no flash). Wobble period 30 s on the
others is the tags' default and has no effect with weight 0.

Victim-side values (bullet hits, explosions) are in the tags; the
explosions' shock waves (`weapons\frag grenade\shock wave`: shake 1.5
s late, translation 0.1 WU, wobble cosine variable 6 s weight 0.4,
radii 3.5 - 8 WU) are what shakes nearby players.

## What is on the disc

All values below are from `bloodgulch.map`; the 550 effect-family tags
it shares with `hangemhigh.map` (547 with `damnation.map`) parse to
identical values, so any multiplayer map gives the same effects.

Counts on Blood Gulch: `effe` 318, `part` 96, `pctl` 7, `cont` 8,
`ligh` 32, `lens` 22, `glw!` 0, `deca` 39, `pphy` 30, `foot` 5, `jpt!`
66 (`mgs2` light volumes 7: not covered here). The 15 weapon and 16
projectile tags reach 289 effects, 89 particles, all 7 particle
systems, all 8 contrails, 22 lights, 14 lens flares, 33 decals, 28
point physics and 2 material effects tags (plus 48 sounds, 24 damage
effects, 90 bitmaps).

Notation: "x n" is a particle count (lo - hi when they differ);
"cam 1/2/3" first person only / third person only / both (absent:
independent); "add" and "alpha" the blend function; r the particle
radius in WU; L the lifespan in s; locations by marker name. Particle
velocities WU/s.

### Assault rifle: `weapons\assault rifle\effects\fire bullet`

Locations: 0 "primary trigger", 1 "primary ejection", 2 "secondary
ejection". Four events, all delay 0 and duration 0 (so everything is
made on the shot's tick).

| Event | What | Where | Numbers |
|---|---|---|---|
| 0 | 6 x `flash h ar`, cam 2 (third-person star) | primary trigger, offsets (-0.005, 0, +-0.025) and (-0.005, +-0.025, +-0.02), directions yaw +-60 deg, pitch +-30/90 deg | r 0.07 - 0.1, velocity 0.01 (first: 0), attached |
| 1 | sound `sound\sfx\weapons\assault rifle\fire` | primary trigger (third-person) | |
| 1 | sound `sound\sfx\weapons\pistol\eject` | secondary ejection | |
| 1 | `flash h ar center` x 1, cam 2 | primary trigger, offset 0.005 forward | r 0.05 - 0.08, spin 0 - 2 pi rad/s, random angle, attached |
| 1 | `smoke rifle water` x 5 - 7, cam 2, water only | primary trigger | distribution radius 0.05 - 0.1, cone 10 deg, r 0.0615 - 0.082, tint grey 0.51 - 0.56 by scale A |
| 1 | `bubbles` x 6 - 8 and x 3 - 7, cam 2, water only | trigger, ejection | r 0.0048 - 0.0228 |
| 1 | `casing h ar bullet` x 1, cam 3 | primary ejection | velocity 2.0 - 2.5 along the marker forward, cone 15 deg (0.2618), spin 0 - 2 pi, r 0.015 |
| 1 | 2 x `flash h ar`, cam 2 | offsets 0.05 and 0.15 forward | velocities 0.25 - 0.325 and 0.6 - 0.625, r 0.16 - 0.185 and 0.1 - 0.125, attached |
| 1 | `smoke h fp` x 4 - 6, cam 3 | primary trigger | velocity 0.1 - 1.5, spin 0.26 - 0.79 (by A), r 0.08 - 0.135, tint alpha 0.75 - 0.875 grey 0.32 |
| 2 | 6 x `flash h ar`, cam 1 (first-person star) | primary trigger, offsets about 0.0025 forward, 0.02 - 0.03 out; directions pitched +-35 deg, yawed +-45 deg | r 0.1 - 0.125, velocity 0.01, attached |
| 3 | `flash h ar center` x 1, cam 1, offset (0, 0, -0.01); water smoke and bubbles, cam 1 | | r 0.0375 - 0.0425 |

Particles used (all "air only" except where said):

| Particle | Bitmap | L (s) | Fades in/out | Animation | Orientation, blend | Physics | Flags |
|---|---|---|---|---|---|---|---|
| `effects\particles\flash\flash h ar` | `effects\particles\flash\bitmaps\flash h ar fp` | 0.075 | 0 / 0 | initial sequences 0 - 5 (one at random), once per frame | parallel to direction, add | cold smoke cloud | once per frame, dies in water, self-illuminated |
| `...\flash\flash h ar center` | same bitmap | 0.3 | 0 / 0 | sequences 6 - 8, once per frame | screen facing, add | cold smoke cloud | once per frame, self-illuminated |
| `...\air\smoke h fp` | `effects\particles\air\bitmaps\smoke tank cannon` | 0.8 - 1.2 | 0 / 2.0 | one sequence, random start frame, not animated | screen facing, alpha | warm smoke cloud (density 0.0009, air friction 75, wind) | minimum size 2 px |
| `...\solid\casing h ar bullet` | `effects\particles\solid\bitmaps\casings h ar` | 30 | 0 / 1.0 | 3 looping sequences, 6 - 30 frames/s, may run backwards, random start, stops at rest; contact deterioration 5 s | screen facing, alpha | stones (density 2.4, air friction 900, surface friction 0.1, elasticity 0.35, collides with structure) | material effects `sound\sfx\impulse\shellcasings\ar_casings` |
| `...\air\smoke rifle water` | `...\air\bitmaps\smoke warthog gun` | 2.1 | 0 / 0.9 | 4.29 frames/s | alpha | cold smoke puff | dies in air |
| `...\air\bubbles` | `...\air\bitmaps\bubbles` | 4.0 | 0 / 0.9 | looping, 7.5 - 10 frames/s | alpha | hot smoke puff | dies in air |

So, on one shot of the assault rifle, the shooter (in first person)
sees a six-ray additive star and a centre flash at the first-person
muzzle for two frames (0.075 s: dies on the third update), a 2 WU
yellow-white light at the muzzle fading over three ticks, smoke puffs
drifting up for about a second, and a casing thrown from the
first-person ejection port at 2 - 2.5 WU/s; everyone else sees the
third-person star at the world model's muzzle. The firing kick is a
0.5 degree random camera twitch for one tick.

### Pistol: `weapons\pistol\effects\fire bullet`

Locations "primary trigger", "primary ejection". Event 0 (delay 0):
sound `sound\sfx\weapons\pistol\fire` at the trigger, sound
`sound\sfx\weapons\pistol\eject` at the ejection port; cam 2: `flash h
pistol` x 1 (r 0.075 - 0.094), 2 x `flash h pistol side` (yaw +-65
deg, r 0.035 - 0.05), `smoke h fp` x 6 - 10, `flash h pistol muzzle`
(independent of camera, offset -0.004, r 0.045 - 0.056), bubbles under
water. Event 1 (delay 0): cam 1: 2 x `flash h pistol side` (r 0.0375 -
0.05), `flash h pistol center` (offset 0.025, r 0.045 - 0.056), `smoke
h fp` x 6 - 10 (and x 20 - 25 buildup under water). Event 2 (delay
0.03 s, so on the next tick): `casing h pistol` x 1, cam 3, at the
ejection port, velocity 1.75 - 2.75, cone 10 deg, r 0.01. Flash
particles: `effects\particles\flash\bitmaps\flash h pistol`, L 0.075,
add, once per frame, self-illuminated (the "side" ones parallel to
direction, sequences 11 - 13; "muzzle" perpendicular to direction,
sequences 7 - 10). Casing `weapons\pistol\bitmaps\casings`, L 10 s,
stones, `ar_casings`. Light: the assault rifle muzzle flash light
(function A).

### Shotgun: `weapons\shotgun\effects\shotgun firing`

Locations "primary trigger", "primary ejection". Event 0 (cam 1) and
event 1 (cam 2), each: sound `sound\sfx\weapons\shotgun\fire`, `sparks
trail` x 6 - 9 (velocity 15 - 20 first person, 3 - 6 third person, r
0.0084 - 0.0159), `flash h generic` x 1 (offset 0.025, r 0.1 - 0.125),
`smoke h fp` x 12 - 18 (velocity 0.5 - 5, r 0.125 - 0.375); two
entries have count 0 - 0 (never made). Event 2: delay 0.5 s (made on
the 15th tick after the shot): `casing h shotgun` x 1, cam 3, at the
ejection port, velocity 1.75 - 2.75, cone 10 deg, r 0.0225 (`weapons\
shotgun\bitmaps\casings`, L 30 s, `shotgun_casings`). Note the shotgun
plays its firing sound twice (once per event). Light: the assault
rifle muzzle flash light (function B).

### Plasma rifle: `...\plasma rifle upper fire`, `...\lower fire`

Effects `weapons\plasma rifle\effects\plasma rifle upper fire` and
`weapons\plasma rifle\effects\plasma rifle lower fire`. Upper at "primary trigger", lower at "primary trigger1" (the two
barrels, alternating per shot, `weapons.md`). Each: event 0 (cam 1):
sound `sound\sfx\weapons\plasma rifle\fire`, `flash c generic` x 1 (r
0.06 - 0.075, spin -2.09 - 0), 4 x `flash c generic med`/`small`
(velocities 0.7 - 1.0, r 0.05 - 0.125), 2 x `smoke c generic` x 6 - 10
(velocity 1 and 6, r 0.045 - 0.09 and 0.0225 - 0.045); event 1:
`tracer c generic` x 1 (independent camera, offset 0.2, velocity 4 -
6, r 0.08 - 0.16, L 0.067); event 2 (cam 2): the same as event 0 with
doubled sizes and the fire sound again. Energy particles: `effects\
particles\energy\bitmaps\flash c generic muzzle` and `smoke c generic`,
add. Light: `weapons\plasma rifle\muzzle flash` (blue, radius 2) at
"secondary trigger", function C; two `plasma rifle flare` lights at
"heat flare", "heat flare1" (function B, lens flares). Misfire (when
overheated): `weapons\plasma rifle\effects\misfire` at "vent": overheat
sound, smoke.

### First four weapons' impacts

Impacts are made at the hit point with the impact markers (Creating
an effect). Decal parts trace along their location's forward by the
given length.

| Projectile | Material | Effect | Parts | Particles |
|---|---|---|---|---|
| AR bullet | dirt (0) | `weapons\assault rifle\effects\impact dirt` | sound `sound\sfx\impulse\impacts\dirthits`; decal `effects\decals\bullet holes\dirt pistol` trace 1.0 WU along "negative incident", radius x 0.25 | `dirt animated kick` x 1 (r 0.15 - 0.175), `dirt animated dust` x 3; event 1 (8 s): bubbles x 5 - 7 water only |
| AR bullet | sand (1) | `...\impact sand` | dirthits; decal dirt pistol x 0.25 | `sand impact burst`, dust x 5 - 9 |
| AR bullet | stone (2) | `...\impact stone` | decal `bullet holes\stone` trace 0.1; sound `granhit` at "reflection" | event 0 (0.15 s): `sparks impact` x 1, `sparks trail` x 3 - 4 (velocity 2 - 4.5 x A, cone 65 deg), `gravel small` x 1; event 1: `smoke impact` x 8 - 12 (velocity 1 - 2) |
| AR bullet | wood (4) | `...\impact wood` | sound `woodhit` | `flash h shield impact`, `wood impact` x 10 - 12, `flare h stealth cannon`; smoke x 2 |
| AR bullet | metal hollow, thin (5, 6) | `...\impact metal hollow reflect`, `...\metal thin reflect` | decal `bullet holes\metal` trace 0.1; sound `metalhit` | sparks impact, `sparks trail` x 20 - 40, smoke x 2 - 3, `flash h shield impact` |
| AR bullet | metal thick (7) | `...\metal thick reflect` | as thin, plus decal `blood splat small` along "incident" (traces back out of the wall; normally finds nothing) | as thin |
| AR bullet | glass (9) | `...\impact glass` | sound `glass_hits`; decal `unbreakable glass` trace 1.0 | flash, flare; smoke x 2 |
| AR bullet | cyborg (21; Spartan, shield down) | `...\impact cyborg armor` | sound `metalhit`; decal `blood splat small` trace 2.0 along "negative incident" (on the wall behind) and `blood splat` trace 1.0 down (on the floor) | sparks impact, sparks trail x 20 - 40, smoke x 2 - 3, flash |
| AR bullet | cyborg energy shield (22; the shield flare) | `...\impact cyborg shield` | none | at "normal": 13 entries of `shield jackal sparks` x 1 - 2 fanned in a half-circle around the normal (velocity 0.1 - 0.75, r 0.1 - 0.16, tint (1, 0.82, 0.39)), `flash c generic` x 1 (r 0.1 - 0.12); event 1: 11 more spark entries |
| AR bullet | water (28) | `...\impact water pen` | sound `ar_water` | `water ripple` x 1 - 2, `water burst small` x 8 - 10, `water burst` x 8 - 10 |
| AR bullet | leaves (29) | `...\impact grass` | | `grass impact` x 4 - 5 |
| pistol bullet | dirt, sand | `weapons\pistol\effects\impact dirt` | dirthits; event 1 (delay 0.2 - 0.25 s): decal dirt pistol trace 0.1 | kick, dust x 3, `flash h ar` x 2 - 3 at "normal"; later `flash h pistol detonate`, flare |
| pistol bullet | stone | `weapons\pistol\effects\impact stone` | decal `stone` trace 0.1 and in event 1 `stone med` trace 0.1; granhit | event 0 (0.25 s): sparks trail x 3 - 4, gravel x 1, `flash h ar` x 2 - 3 at "normal" (r 0.3 - 0.5, alpha 0); event 1: smoke x 2, `flash h pistol detonate` (r 0.155 - 0.2, L 0.5, grows to 1.25x), gravel x 8 - 10, flare |
| pistol bullet | wood, metal, cyborg, shield | `weapons\pistol\effects\impact ...` | as the AR's, plus `flash h ar` x 2 - 3 at "normal" | |
| pistol bullet | glass, water, leaves | the AR's effects | | |
| shotgun pellet | each | `weapons\shotgun\effects\impact ...` | as the AR's (same decals and sounds) | about half the AR's counts (sparks trail x 1 - 2 on stone, x 4 - 8 on metal) |
| plasma rifle bolt | dirt | `weapons\plasma rifle\effects\impact dirt` | sound `plasmahit`; decal `bullet holes\plasma burn` trace 1.0 (r 0.085, chained to `plasma burn inner glow` r 0.08 add 1 s and `outer glow` r 0.15 add 2 s) | event 0 (0.05 s): `smoke c generic` x 8 - 12, `flash c generic med` x 5 - 8, flash, dust x 5 - 10; event 1 (0.45 s): smoke x 2 x 10 - 13 |
| plasma rifle bolt | stone | `...\impact stone` | decal plasma burn trace 0.1; plasmahit | as dirt with `gravel small` x 12 - 15 |
| plasma rifle bolt | metal (5 - 7) | `...\impact metal reflect` | plasmahit; plasma burn | flashes, `metal chips` x 3 - 10, `sparks energy trail` x 10 - 25; smoke |
| plasma rifle bolt | cyborg | `...\impact cyborg armor` | plasma burn trace 0.1, blood splats trace 2 | as metal |
| plasma rifle bolt | cyborg shield | `...\impact cyborg shield` | plasmahit at "normal" | the same shield flare as the AR's |
| plasma rifle bolt | water, leaves, sand, wood, glass | `...\impact water pen`, `grass`, `sand`, `wood`, `glass` | | |

The plasma rifle bolt has **no** detonation effect (it vanishes on
impact); the plasma pistol's charged bolt detonates with `weapons\
plasma rifle\effects\detonate` (one part: the particle system
`weapons\plasma rifle\effects\charged bolt explosion`), and its impact
uses its "potential" responses (`charged bolt impact ...`).

Decals used:

| Decal | Type, layer, blend | Radius (WU) | Lifetime / decay (s) | Map |
|---|---|---|---|---|
| `effects\decals\bullet holes\stone` | scratch, 0, double multiply | 0.035 - 0.065 | 300 / 1 | `...\bitmaps\stone` |
| `...\stone med` | scratch, 0, double multiply | 0.085 - 0.1 | 180 / 20 | stone |
| `...\dirt pistol` | scratch, 0, double multiply | 0.085 - 0.1 (x 0.25 for the AR: 0.021 - 0.025) | 180 / 20 | dirt pistol |
| `...\metal` | scratch, 0, double multiply | 0.025 - 0.035 | 180 / 20 | metal |
| `...\unbreakable glass` | scratch, 0, double multiply | 0.09 | 180 / 20 | |
| `...\plasma burn` -> `inner glow` -> `outer glow` | scratch 0 double multiply; then layer 1 add; layer 2 add | 0.085; 0.08; 0.15 | 180/20; 1/1; 2/2 | |
| `...\grenade char` | burn, 0, double multiply | 1.0 - 1.25 | 180 / 20 | dirt pistol |
| `...\plasma burn large` -> `inner glow large` -> `outer glow large` | as plasma burn | 0.55 (x 1.0 - 1.5); 0.75; 0.25 | 180/20; 1/1; 2.75/2 | |
| `effects\decals\blood splats\blood splat`, `blood splat small` | scratch, 0, multiply | 0.15 - 0.225; 0.05 (intensity 0.5 - 1) | 180 / 20 | blood splat |

### The rest of the arsenal (compact)

| Effect | Events: parts; particles |
|---|---|
| `weapons\sniper rifle\effects\fire bullet` | e0: sound fire at "primary trigger"; e1: casing `vehicles\warthog\warthog gun casing par` cam 3 at "primary ejection", 3 x `flash h sniper muzzle break` x 15 - 20 cam 3, 3 x `smoke h fp` x 6 - 9 cam 3. Light `weapons\sniper rifle\muzzle flash` radius 3; tracer contrail `weapons\sniper rifle\sniper` (250 points/s, width 0.025 -> 0.044 -> 0.088, smoke-coloured, lasting 0.1 - 0.35 s + 0.875 s) |
| `weapons\rocket launcher\effects\fire rocket` | e0 (duration 0.2 - 0.3 s): sound fire; `smoke h fp` x 9 - 12 cam 2 and x 24 - 28 cam 3 spread over the duration; 2 x `flash h ar` |
| rocket in flight | attachments: effect `weapons\rocket launcher\effects\exhaust` at "exhaust" (looping: `flash h pistol side`), contrail `weapons\rocket launcher\exhaust` (15 points/s, 4 states, width 0.15 -> 0.5), particle system `weapons\rocket launcher\effects\rocket exhaust` at "exhaust" (one type "fire to smoke": 30 particles per second with a minimum of 10, default creation at the marker, fire then smoke plume states) |
| `weapons\rocket launcher\effects\rocket explosion` (also the rocket launcher ammo's detonation) | e0: damage `weapons\rocket launcher\explosion` and `weapons\frag grenade\shock wave`; sound expl; particle system `weapons\frag grenade\effects\explosion med`; decal `grenade char` (trace 1.0 along the projectile's forward); light `weapons\frag grenade\explosion`; `grenade explosion flare` x 1; `drifting smoke` x 40 - 60 |
| `weapons\plasma pistol\effects\fire bolt` | e0 cam 1 / e1 cam 2: sound fire; `flash c generic`, 3 x `flash c generic med`, `smoke white c generic` x 4 - 6 and x 8 - 12, `flash c generic small` |
| `weapons\plasma rifle\effects\charged bolt fire` | sound `chargefire` only |
| `weapons\needler\effects\fire needle` | e0 (0.05 s): sound fire; 9 flashes each cam 1 and cam 2 (incl. `tracer c generic`, `flash h shield impact`); e1 (0.3 s): `smoke c generic` x 4 at "smoke" cam 1 / cam 2 |
| `weapons\needler\effects\needle detonate` | e0 (0.085 - 0.1 s): decal dirt pistol x 2 trace 1; sound expl; `needler spike debris` x 5 - 8; `flash h pistol detonate`; flare; e1: smoke x 8 - 12 |
| `weapons\needler\effects\explosion` (supercombine) | particle system `weapons\needler\effects\explosion`, damage shock wave and explosion, light `weapons\needler\explosion` (r 1, 30 ticks), sound `needler_super_expl`, `plasma electric bolts` x 4 - 8 |
| `weapons\flamethrower\effects\flame thrower jet` | e0 (0.2 s): `burning flame` x 5 - 8 cam 2, 4 x 1 cam 1, `flash h pistol side`; attachment particle system `weapons\flamethrower\effects\defoliant` |
| `weapons\flamethrower\effects\flame detonation` | damage `weapons\flamethrower\explosion` only |
| `vehicles\warthog\effects\warthog gun fire` | sound fire; 6 x `flash h generic`, 3 x `smoke h fp`, casing `warthog gun casing par` at "primary ejection", all cam 2 |
| `vehicles\scorpion\scorpion cannon fire` (required for gameplay; loops 0-0) | e0: sound expl; light at "cannon flash"; `cannon flash small` x 40, `med` x 4, `large` x 8; e1 (delay 0.13 s, 0.1 - 0.12 s): `cannon smoke` x 5; e2 (1.5 - 1.8 s): `cannon smoke trail` x 20 |
| `vehicles\scorpion\secondary fire bullet` | identical to the assault rifle's fire bullet at "secondary trigger" |
| `vehicles\scorpion\shell explosion` | damage shell explosion and frag shock wave; sound; particle system `vehicles\scorpion\shell explosion`; decal grenade char down 1 WU; light `vehicles\scorpion\shell explosion` (r 15, 30 ticks, early fall-off, lens flare); flare; `drifting smoke` x 40 - 60 |
| `vehicles\ghost\ghost gun fire beam` | sound fire; flashes and smoke (independent camera); e1: tracer |
| empty effects (AR, pistol, needler: `effe` tags `weapons\<name>\effects\empty`; shotgun, plasma rifle, sniper rifle, plasma pistol, rocket launcher: a `snd!` tag named directly, see Who creates effects) | the dry-fire sound at the object's origin ("" location) |
| `weapons\plasma rifle\effects\overheated`, `...\plasma pistol\effects\overheated` | looping (loop 0-0) at "vent" / "vent_rear", "vent_mid", "vent_front": (null sound), `plasma overheat` x 2 - 4 per vent cam 1 and cam 2, smoke |

### Grenades

| Effect | Content |
|---|---|
| `weapons\frag grenade\effects\explosion` (required for gameplay, danger radius 8) | locations "" (the grenade's forward), "normal" (absent: falls back to the first marker), "gravity". Event 0, all at once: damage `weapons\frag grenade\explosion` and `...\shock wave`; sound `sound\sfx\weapons\frag grenade\expl`; particle system `weapons\frag grenade\effects\explosion med`; decal `grenade char` traced 1.0 WU straight down (radius 1.0 - 1.25, burn, 180 s); light `weapons\frag grenade\explosion` (dynamic, no specular, radius 10, black to (1, 0.808, 0.357), 45 ticks = 1.5 s, linear fall-off); light `weapons\frag grenade\effects\grenade quickflash` (radius 1, colour (0.996, 0.671, 0.18), 4.5 ticks, with its lens flare); `weapons\frag grenade\effects\drifting smoke` x 10 - 15 (velocity 1 - 2 WU/s, r 1.2 - 2.0, spin 0.87 - 1.40, alpha 0.4 - 0.6 grey, L 2 - 4 s, fade in 0.3 s, out 3 s, physics `smoke explosion` density 0.0008) |
| frag grenade in flight (the grenade trail) | attachments: looping effect `weapons\frag grenade\effects\smoke` at "smoke" (one zero-length event, loop 0-0: 4 passes per tick of `smoke impact` x 1 - 2, L 1 - 2 s, fade out 3.5 s, rising cold smoke) and light `weapons\frag grenade\frag grenade` (radius 1, (1, 0.98, 0.54)) |
| frag grenade bounces | material responses: `weapons\frag grenade\effects\dirt reflection` (sound `bouncedirt`, smoke x 1 - 2) for dirt, sand, stone, wood, glass, organics; `...\metal reflection` (sound `bouncemetal`, sparks) for metal, cyborg, ice; shield (22): the AR's shield flare |
| `weapons\plasma grenade\effects\explosion` (required for gameplay, danger 8) | locations "", "gravity": particle system `weapons\plasma grenade\effects\explosion`; damage `...\shock wave` and `...\explosion`; sound `plasmagrenexpl`; light `weapons\plasma grenade\explosion` (dynamic, no specular, radius 10, black to (0.525, 0.953, 0.976), 30 ticks, late fall-off); decal `plasma burn large` traced 1.0 down with radius x 1.0 - 1.5 |
| `weapons\plasma grenade\effects\detonation start` (the stuck or landed grenade's countdown) | event 0 lasting 2 s: sound `dirthits` at "normal"; spread over the 2 s: `smoke c generic` x 80, `flash h shield impact` x 60, `plasma electric bolts` x 60, `flare h stealth cannon` x 60 and x 30 - 35, bubbles x 60 (water) |
| plasma grenade in flight | attachments: looping `weapons\plasma grenade\effects\plasma grenade armed` at "light" (particle `weapons\plasma grenade\plasma grenade armed` x 5 - 12 per pass, add, L 0.5 - 1), light `weapons\plasma grenade\plasma grenade` (radius 1, modifiers 1 - 2, (0.25, 0.5, 0.5) to (0.5, 1, 1), lens flare), a looping sound |
| plasma grenade bounces | `weapons\plasma grenade\effects\bounce` (sound, smoke, flash, dust) and `bounce water` |
| throws | `weapons\frag grenade\effects\throw`, `weapons\plasma grenade\effects\throw`: the throw sound |

### The Spartan

- Shield hit ("shield flare"): there is no shield-hit effect on the
  collision model; the flare is each projectile's material response
  for cyborg energy shield (22): `weapons\<weapon>\effects\impact
  cyborg shield` (table above). The glowing shield overlay itself is
  the model's shield shader (`health_shields.md`, `shaders.md`), not an
  effect.
- Shield down: the collision model `characters\cyborg\cyborg` names
  `characters\cyborg\cyborg shield depletion` as its shield depleted
  effect (made on the biped with no markers: its "" location is the
  biped's origin): `shield jackal depletion` x 6 - 8 (L 0.25 - 0.3,
  radius growing 1 to 2.5x, add) and `shield jackal sparks` x 10 - 20
  (L 2 - 2.25, parallel to direction, add, vacuum physics: no gravity,
  no drag). Shield damaged and recharging effects: none.
- Body: area damage effect `effects\blood aoe human` (blood splat
  decal traced 5 WU from the origin, radius x 2; `blood engineer
  trails` x 15 - 20; `blood h decal causing` x 5 - 6); localized
  damage effect `characters\cyborg\localized damage` (never played in
  multiplayer, `health_shields.md`).
- Overshield and active camouflage: no effect tags on the disc; both
  looks are the biped's shader and render modes (`health_shields.md`,
  `multiplayer_rules.md`).
- Footsteps: `globals\cyborg` (from the biped's footsteps reference):

| Kind | Sounds by material (all under `sound\sfx\impulse\footsteps\cyborg\`) | Effect |
|---|---|---|
| 0 walk | dirt `dirt\walk`, sand `gravel\walk`, stone `grtcement\walk`, snow `snow\walk`, wood `wood\walk`, metal hollow/thin `grating\walk`, metal thick `cleancement\walk`, glass/jackal shield/ice `marble\walk`, water `water\walk`, leaves `grass\walk`; others none | water: `effects\footsteps water` |
| 1 run | the same with `run` | water splash |
| 2 sliding | none | water splash |
| 3 shuffle | the same with `shuffle` | water splash |
| 4 jump | the same with `jump_up`; the 20 creature materials `jump\genericjump` | water splash |
| 5 jump land | the same with `jump` | water splash |

`effects\footsteps water`: `water burst small` x 10 - 15 and `water
burst` x 8 - 10.

## Per-tick order

Within a game tick (`weapons.md`, `projectiles.md` own the rest):

1. Units and AI, players' input.
2. **Effects update** with dt = 1/30 (each effect in pool order: its
   attachment checks, visibility, event steps; parts made now).
3. Antennas, first-person weapons, game engine, scripts.
4. Objects update: weapons fire (firing effects created, each updated
   once with dt 0 at creation), projectiles move and hit (impact and
   detonation effects created and updated with dt 0), attachments
   follow; damage events start player screen effects.
5. Players, HUD, player effects bookkeeping.

Then, per rendered frame (once per tick on the Xbox, dt = 1/30):

6. Particles update (each in pool order: unrendered check, age,
   animation, physics, death).
7. Contrails update (point emission once per tick, then point states
   and physics).
8. Particle systems update.
9. Decals update (fade and expiry), lights (effect light expiry and
   intensities), then drawing.

So a particle made by an effect in tick N gets its first update in the
frame after tick N and is first drawn then (age 1/30); a shot's flash
of lifespan 0.075 s is drawn in two frames.

## Limits

| Pool | Size | When full |
|---|---|---|
| effects | 256 | creation fails; a required-for-gameplay effect frees the first effect without that flag and takes its slot |
| effect location instances | 512 | further instances of that effect are dropped |
| particles | 1024 | creation fails silently; particles not drawn for 15 frames are deleted |
| particle systems | 64 | creation fails |
| system particles | 512 | creation stops |
| contrails / contrail points | 256 / 1024 | creation / emission fails |
| decals | 2048 records, 2560 cache pages | least recently used unlocked decals are evicted |
| lights | 896 records, 128 drawn per frame | creation fails / extra lights not drawn |
| rendered particle groups | 512 per frame | remaining groups not drawn |

Split screen (several local players): per-entry counts above 6 and
lifespans above 0.7 s are divided by the local player count above
those thresholds.

## Edge cases

- **Missing marker**: a location whose name the object's model lacks
  yields no instances; the parts and particles there are silently not
  made (sounds included). The empty name always yields the origin.
- **Location index out of range** (5 entries on the disc): skipped.
- **Count lower above upper** ("x 1(2-)" in the disc tables): the
  draw lands between them; never negative.
- **Zero-duration events**: their particles are all made in one step
  (fraction 1) whatever the distribution function, except "end",
  which also gives 1 at fraction 1.
- **Effects on deleted objects**: an attached effect whose object is
  gone is deleted at its next update. Particles attached to a deleted
  object are deleted.
- **Not visible**: an impact behind a wall from every local player is
  deleted at creation, so it makes no sound and no decal either.
  Required-for-gameplay effects use every player's visibility; an
  explosion no player could potentially see makes nothing at all
  (its damage included).
- **Under water**: environment filters per instance/particle; the
  footstep probe hitting under water uses the water material; point
  physics switches mass, buoyancy and drag.
- **Light of duration 0**: lives only while its creation tick is
  current (deleted once one tick has passed); its fall-off input is
  0/0, which the implementation must treat as 0 (full intensity).
- **Particles on a bitmap that is not a sprite group** (`flare h
  stealth cannon`, `grenade explosion flare`; 2D-texture bitmaps,
  "1 / sprite size" 1.0): the whole bitmap is drawn as a 2R by 2R
  square, never 2R x the bitmap's pixel size (How a particle lives,
  World size). A requested sequence beyond the bitmap's sequences is
  reduced into them (`flare h stealth cannon` asks for 3 of 1).
- **Particle physics with radius 0** works (offset min(r, 0.005) = 0).
- **Effect light on an object** follows the node; a free one stays.

## Quirks

- The 8-step cap makes zero-length looping effects run four passes per
  tick (frag grenade smoke, plasma grenade armed glow). Keep it: it
  sets the trail density.
- Event 0's skip fraction is never tested.
- A required-for-gameplay effect far from every player's view is
  deleted without its damage. Keep (it is what the game does); it only
  matters for explosions nobody can see.
- Particle parts are made at the third-person markers even for the
  first-person view; only particle entries use first-person markers.
- The shotgun plays its firing sound twice, the plasma rifle too
  (third- and first-person events both carry it).
- The AR's metal-thick impact traces a blood decal back out of the
  wall ("incident"), which normally finds nothing.
- The camera impulse and shake show at full scale (k = 1) on their
  first frame regardless of intensity.
- `ligh` duration is stored in ticks while every other effect time is
  in seconds.
- An effect part's object velocity is per tick but a particle
  system's is per second, and the inherited effect velocity (per tick)
  is added unscaled to the latter.

## Test cases

### (a) Synthetic

**Synthetic effect.** Build an `effe` tag (struct 0x40) with flags 0,
loop -1/-1, locations ["primary trigger", ""], one event: flags 0, skip
0, delay 0.1/0.1, duration 0.2/0.2; parts:

| Field | Part 0 | Part 1 |
|---|---|---|
| environment, mode | 1 (air only), 0 | 0, 1 (violent only) |
| location, flags | 0, 0 | 1, 1 (face down) |
| runtime class, type | `snd!`, a sound tag | `deca`, a decal tag |
| velocity, cone | 0/0, 0 | 0.5/0.5, 0 |
| radius modifier | 0/0 | 1.0/2.0 |
| A, B | 0, 0 | 0, 0 |

and one particles entry: environment 0, mode 0, camera 0, location 0,
direction (0, 0), offset (0, 0, 0), runtime direction (1, 0, 0), a
`part` tag, flags 0, distribution 2 (constant), count 3/3,
distribution radius 0/0, velocity 1/1, cone 0, angular 0/0, radius
0.1/0.2, tint lower (1, 1, 1, 1), upper (1, 0, 0, 0) (argb), A 0, B 0.

| Setup | Input | Expected |
|---|---|---|
| the bytes above | parse | 2 locations; 1 event (skip 0, delay 0.1-0.1, duration 0.2-0.2); part 0 env 1, mode 0, location 0, class `snd!`; part 1 env 0, mode 1, location 1, flags 1, velocity 0.5, radius modifier 1-2; particle entry count 3-3, distribution 2, radius 0.1-0.2 |
| effect on an object with marker "primary trigger", in air, alive | create; update each tick | updates 1, 2: nothing; update 3: delay completes (float32): sound at the marker; decal traced 0.5 WU straight down from the object's origin with radius modifier 1 + r (one local draw r); particles: 0 at update 3 and 4, 1 at 5, 1 at 7, 1 at 9; effect deleted at update 9 |
| same | random draws at update 3 | duration (effect generator), count (local), then for part 1: velocity magnitude, cone angle (no axis: angle 0), radius modifier, then the decal's |
| same, the object under water | update 3 | no sound (air only); decal made |
| effect on a biped dead for 31 ticks (corpse) | update 3 | sound made; no decal (violent only) |
| object lacking "primary trigger" | all updates | no sound and no particles; decal made (the "" location is the origin) |
| one particle with draws: tint fraction 0.25 | colour | RGB (0.75, 0.75, 0.75) (lerp from white to black), alpha 1 |
| same with radius draw 0.5 | radius | 0.15 |
| a duplicate effect with duration 0, distribution 0, count 3 | update with dt 0 at creation | event start and all 3 particles in the creation update; effect deleted in it |
| loop start 0, loop stop 0, one event delay 0 duration 0, made as an attachment | one update | 4 passes: 4 x the particle count made |
| skip fraction 0.5 on event 1 of a 2-event effect, skip draw 0.4 | event 0 ends | event 1 skipped, effect deleted |

**Particle motion and fade.** A free particle, radius r = 0.02,
radius animation 1.0 to 2.0, lifespan 0.5, fade in 0.1, fade out 0.2,
physics density 2.4 (mass/r^3 284672.03, air buoyancy -0.99908), air
friction 900, flags 0 (no collisions, no wind); start (0, 0, 1),
velocity (2, 0, 1) WU/s; updates of dt = 1/30; per update: radius at
the new age, gravity, drag, move.

| After updates | Position | Velocity | Radius | Fade |
|---|---|---|---|---|
| 0 | (0, 0, 1) | (2, 0, 1) | 0.02 | 0 (fade in) |
| 1 | (0.06634, 0, 1.02962) | (1.99012, 0, 0.88873) | 0.02133 | 0.3333 |
| 5 | (0.32878, 0, 1.11151) | (1.95622, 0, 0.45060) | 0.02667 | 1.0 |
| 10 | (0.65136, 0, 1.13281) | (1.92273, 0, -0.08606) | 0.03333 | 0.8333 |

(tolerance 1e-4; it dies at update 15, age 0.5.) A vacuum particle
(density 0, no gravity flag, friction 0), velocity (3, 0, 0), radius
0.05: positions (0.1, 0, 0), (0.5, 0, 0), (1.0, 0, 0) after 1, 5, 10
updates, velocity unchanged.

| Setup | Input | Expected |
|---|---|---|
| a particle of lifespan 0.075 with "animate once per frame", 6 sprites | updates | sprite 0 after update 1, sprite 1 after update 2, dies at update 3 (age 0.1 >= 0.075) |
| a particle that hits a floor (normal (0, 0, 1)) with v = (1, 0, -2), surface friction 0.1, elasticity 0.35 | bounce | v = (0.9, 0, 0.7) |
| collision speed 1.0 WU/s | collision effect scale | 0.5 |
| particle with "dies on contact with structure" and a collision effect | hits | collision effect made, no death effect, particle gone |

**Decal draws.** Decal radius bounds 0.035 / 0.065, no random
rotation flag clear, lifetime 180 / 180, decay 20 / 20, part radius
modifier 0. Draws stubbed: angle 0.25, sequence 0, radius 0.5, lock
draw 30000 (not locked), lifetime 0.3, decay 0.7, intensity 0.5,
colour 0.5.

| Input | Expected |
|---|---|
| radius | 0.035 + 0.5 x 0.03 = 0.05 WU (modifier 0 counts as 1) |
| radius modifier 1.5 | 0.075 |
| lifetime, decay | 180, 20 |
| intensity byte at elapsed 0, 150, 165 s | 255, 255, 191 (15/20 x 255 = 191.25) |
| elapsed 180 s | geometry freed (gone) |
| trace vector (0, 0, -0.5) from 0.3 above a floor | decal on the floor at the point below |
| trace vector that hits only an object (a biped) | no decal |

**Effect light.** Light radius 10, modifiers 1 and 1, colours black to
(1, 0.808, 0.357), duration 45 (ticks), linear fall-off, part scale 1:

| Ticks after creation | Intensity | Colour | Exists |
|---|---|---|---|
| 0 | 1.0 | (1, 0.808, 0.357) | yes |
| 15 | 0.6667 | (0.6667, 0.5387, 0.2380) | yes |
| 45 | 0 | black (not drawn) | yes |
| 46 | | | deleted |

**Screen flash.** Lighten, priority 1, duration 0.4 s, fade function
late, maximum 0.8, zero scale factor 0, event scale 0.5: stored
intensity 0.5, 12 ticks; shown intensity on the first frame =
transition(late, 0.5 x 12/12); after 6 ticks transition(late, 0.25).

**Muzzle light.** Assault rifle shot on tick N: light intensity 1,
2/3, 1/3, 0 on ticks N, N+1, N+2, N+3; radius 2.0 WU throughout;
colour 0.961, 0.910, 0.686 times the intensity.

**Particle systems and contrails (fourth errata batch).**

| Setup | Input | Expected |
|---|---|---|
| particle system type, state 0 physics P0 (air friction 2000), state 1 physics P1 (air friction 200), particle in state 0 | one update | P0 used as stored |
| same, particle transitioning 0 to 1, time left / length w = 0.75 | one update | air friction = 0.25 x 2000 + 0.75 x 200 = 650 (the next state's weight is w: the reversed blend); flags P0's |
| same, w = 0.75, state radius multipliers 1 and 2, type radius 1.5, type multiplier 1 | radius for the physics | 1.5 x (0.75 x 1 + 0.25 x 2) = 1.875 |
| same, particle in state 1 (transition over) | one update | P1 as stored |
| particle system particle, random scale 0.02, type scale multiplier 1, sprite 36 by 36 pixels on a 256 page | drawn quad | 0.72 by 0.72 WU |
| same, type state scale multiplier blended 0.5, system scale 3 without type flag 13 | drawn quad | 0.36 by 0.36 WU (the system scale is not applied) |
| plain particle, radius draw 1.5, radius animation (1, 1), sprite size field 1/33, sprite 33 pixels wide | drawn width | 3.0 WU (2R) |
| 12 sprites each covering the whole view, built in one view's frame | draw | the first 11 drawn, the 12th dropped |
| contrail point state 0 duration bounds 0.1 - 0.25, no scale flags, local draw stubbed to 0.5 | point enters state 0 | its length 0.175 s; at 30 updates/s its progress is 0.952 after 5 updates and 1.143 after 6: it leaves state 0 on the 6th, its transition progress starting at 0 (the 0.143 is dropped) |
| two points of the same contrail entering state 0 with draws 0 and 1 | lengths | 0.1 s and 0.3 s (each point its own draw; never the mean 0.2 for both) |
| last state with duration 0 | transition into it ends | the point expires in that same update |
| last state with duration 0.5 | transition into it ends | the point lives 0.5 s more in it, then expires |

**Particle size and orientation (2026-11-18).**

| Setup | Input | Expected |
|---|---|---|
| sprite-group bitmap, one sequence of sprites 98 x 84 and 60 x 40 pixels, registration (49, 38) on the first; particle k = 1/98, radius draw 0.05, radius animation (1, 1) | draw the first sprite | quad 0.100 x 0.0857 WU, corners x from -0.05 to +0.05, y (down the picture) from -0.0388 to +0.0469 WU about the particle |
| same | draw the second sprite | quad 0.0612 x 0.0408 WU (same pixel scale 0.00102 WU per pixel; never stretched to 2R) |
| particle using sequences 3 to 5 of a bitmap whose sequence 0 holds a 98-pixel sprite and 3 to 5 hold 84 x 82 | build-time k | 1/84 (only the used sequences count) |
| 2D-texture bitmap (type 0), one sequence of one 64 x 64 bitmap, registration (32, 32); particle k = 1.0, radius draw 2.5 | drawn quad | 5.0 x 5.0 WU centred on the particle (not 320 WU) |
| same bitmap, particle asks for first sequence index 3 | creation | sequence 0 drawn (the index is reduced into the one sequence); the particle lives |
| orientation 1, d = (1, 0, 0), eye at (0, 0, 10) from the particle, sprite 55 x 17, k = 1/55, R = 0.1 | quad | 0.2 WU along world x, 0.0618 WU along world y (normalize(d x e) = (0, -1, 0) up to sign), lying in the ground plane |
| same, eye at (10, 0, 0) (looking along d) | quad | zero area (not drawn) |
| first-person entry radius 0.04, third-person entry radius 0.08 of the same sprite particle | both drawn | the third-person quad is twice the first-person one; neither is rescaled for its view |

### (b) Disc-backed (slow tests, the user's own disc files)

| Check | Expected |
|---|---|
| `weapons\frag grenade\effects\explosion med` (`pctl`) | one type: flags 0x10, initial 35, render mode 1, radius 1.5, explosion creation (0.012, 0.08, 200); type states (1 - 1.5 s, transition 4 - 5 s, multipliers 1) and (colour multiplier 0); particle states fire (0.1 - 0.2 / 0.1 - 0.8, `fire cloud` seq 0, scale 0.01 - 0.03, rotation 3.49 - 6.98 rad/s, blend 3, `fire explosion`), smoke plume (0.3 - 0.6 / 0.3 - 0.8, `smoke cloud`, 0.01 - 0.04, alpha 0.2 - 0.4, blend 0, `fire explosion`), death (0 / 0, alpha 0, `smoke explosion`) |
| `weapons\frag grenade\effects\drifting smoke` | flags 4, lifespan 2 - 4, fade in 0.3, fade out 3.0, radius animation 1 - 1, fade end size 4, sprite size field 0.030303031, bitmap `smoke tank cannon`, physics `smoke explosion`; the effect entry: count 10 - 15, radius 1.2 - 2.0, velocity 1 - 2, tint alpha 0.4 - 0.6 |
| `fire explosion` / `smoke explosion` (`pphy`) | density 0.0005 / 0.0008, air friction 2000 / 200, flags 0 / 0x18 |
| `weapons\assault rifle\effects\fire bullet` | flags 0, loop -1/-1, 3 locations "primary trigger", "primary ejection", "secondary ejection"; 4 events, all delay 0 duration 0, skip 0; event 0: 0 parts, 6 particles (all `effects\particles\flash\flash h ar`, camera 2); event 1: 2 parts (`snd!` `sound\sfx\weapons\assault rifle\fire` at location 0, `snd!` `sound\sfx\weapons\pistol\eject` at location 2) and 8 particles (`flash h ar center`, `smoke rifle water`, `bubbles`, `casing h ar bullet` at location 1 camera 3 velocity 2.0-2.5 cone 0.2618 radius 0.015, `bubbles`, 2 x `flash h ar`, `smoke h fp`); event 2: 6 x `flash h ar` camera 1; event 3: 4 particles |
| the AR weapon's attachments | light `weapons\assault rifle\muzzle flash` at "primary trigger", primary scale 2 (function B); light `characters\marine_armored\flashlight_marine` at "flashlight" |
| `weapons\assault rifle\muzzle flash` | flags 0x21, radius 2.0, modifiers 1/1, upper colour (0.961, 0.910, 0.686), duration 0 |
| `effects\particles\flash\flash h ar` | flags 0x288, bitmap `effects\particles\flash\bitmaps\flash h ar fp`, physics `effects\point physics\cold smoke cloud`, lifespan 0.075/0.075, sequences first 0 initial 6, orientation 1, blend 3, 1/sprite size 1/135 (0.007407407) |
| `effects\particles\solid\casing h ar bullet` | flags 7, lifespan 30, fade out 1.0, physics `effects\point physics\stones`, material effects `sound\sfx\impulse\shellcasings\ar_casings`, looping sequences 3, animation 6-30, contact deterioration 5 |
| `effects\point physics\stones` | flags 3, density 2.4, air friction 900, water friction 14000, surface friction 0.1, elasticity 0.35, runtime 284672.03 / -0.4118 / -0.9991 |
| pistol bullet's material response 2 (stone) default effect | `weapons\pistol\effects\impact stone`: locations "reflection", "negative incident", "normal"; 2 events; event 0 duration 0.25: parts decal `effects\decals\bullet holes\stone` (location 1, velocity 0.1) and sound `sound\sfx\impulse\impacts\granhit` (location 0); particles `sparks trail` 3-4, `gravel small` 1, `flash h ar` 2-3 at location 2; event 1: decal `stone med` (location 1, 0.1), particles `smoke impact` 2, `flash h pistol detonate` 1, `gravel small` 8-10, `flare h stealth cannon` 1 |
| `effects\decals\bullet holes\stone` | type 0, layer 0, radius 0.035-0.065, lifetime 300, decay 1, blend 2, map `effects\decals\bullet holes\bitmaps\stone` |
| `weapons\frag grenade\effects\explosion` | flags 2, danger radius 8.0, 1 event, 7 parts: `jpt!` explosion, `snd!` expl, `jpt!` shock wave, `pctl` explosion med, `deca` grenade char (location 2 "gravity", velocity 1), `ligh` explosion, `ligh` grenade quickflash; 1 particle entry `drifting smoke` 10-15 |
| `globals\cyborg` | 6 kinds of 32 materials; kind 1 material 2 sound `sound\sfx\impulse\footsteps\cyborg\grtcement\run`; kind 0 material 28 effect `effects\footsteps water` |
| parse every `effe`, `part`, `pctl`, `cont`, `ligh`, `lens`, `deca`, `pphy`, `foot` of the map with these layouts | every reference resolves to its class, every enumeration in range, every listed padding byte zero except `part` 0x6C..0x73 |
| the same tags on `hangemhigh.map` | identical parsed values (550 tags) |
| `effects\particles\lens flare\flare h stealth cannon` | flags 0x208, lifespan 0.2 - 0.21, fade out 0.27, sequences first 3 initial 1, orientation 0, blend 3, "1 / sprite size" exactly 1.0; bitmap `effects\particles\lens flare\bitmaps\flares weapon lens`: tag type 0 (2D textures), one sequence (bitmap 0, count 1, no sprites), one 64 x 64 bitmap, format 1 (y8), registration (32, 32) |
| `effects\particles\lens flare\grenade explosion flare` | "1 / sprite size" 1.0; bitmap `...\flares_generic`: type 0, 7 one-bitmap sequences (64, 32, 16, 32, 32, 16, 64 pixels square, format 10) |
| every other `part` of the map (94) | bitmap type 3 (sprites); 1 / "1 / sprite size" equals, within one pixel, the largest of width and height in pixels over the sprites of sequences first .. first + initial + looping + final - 1 |
| `weapons\plasma rifle\effects\charged bolt impact dirt` | `flare h stealth cannon` x 1, radius 2.0 - 3.0, tint (a, r, g, b) (1, 0.6, 1, 0.58) to (1, 0.45, 1, 0.44); drawn 4.0 - 6.0 WU square |
| `weapons\plasma pistol\effects\fire bolt` | locations "primary trigger", "primary trigger01"; event 0 camera 1 and event 1 camera 2 with the same entries: `flash c generic` r 0.04 - 0.05 offset 0.015 forward; three `flash c generic med` r 0.065 - 0.08 (pitch +2.094 at offset up 0.04 velocity 0.7, pitch -2.094 at offset down 0.04 velocity 1.0, pitch 0 velocity 0.7); `smoke white c generic` x 4 - 6 r 0.075 - 0.15; `flash c generic small` r 0.15 - 0.25; `smoke white c generic` x 8 - 12 r 0.015 - 0.025 velocity 1 - 6 |
| `effects\particles\energy\flash c generic` / `med` / `small` | k 1/98, 1/84, 1/84; sprites 98 x 84 (registration 49, 38), 84 x 82, 84 x 82; orientation 0, 1, 1; flags 0x208 |
| the 8 contrails' attachments | on projectiles only, marker "" except the rocket's "exhaust"; render type 4; plasma rifle widths 0.1, 0.035, 0.0125 (30 points/s); charged bolt 0.5, 0.2, 0.05 (10 points/s) |
| the 22 `lens` tags | every bitmap of type 0; plasma rifle bolt reflection radius 0.125, charged bolt 0.5, both on `flares weapon lens` |

## Verification performed

- All 318 `effe`, 96 `part`, 7 `pctl`, 8 `cont`, 32 `ligh`, 22
  `lens`, 39 `deca`, 30 `pphy`, 5 `foot` tags of `bloodgulch.map`
  parsed with the layouts above by a coverage check: every byte not
  covered by a listed field had to be zero. Only the `part` bytes
  0x6C..0x73 failed (49 tags; documented as unused leftovers). No
  real was NaN or above 1e6 in magnitude; every tag reference resolved
  to an indexed tag whose class equals the stored class; every
  enumeration fell inside the ranges given; block counts were within
  the maxima.
- From the 15 weapon and 16 projectile tags, every reference in the
  fields listed (weapon attachments, ready, overheated, detonation,
  light toggle, reload, chamber, charging, firing/misfire/empty
  effects and damages; projectile attachments, detonation, super
  detonation, detonation started, all material responses' effects) was
  followed through effects (parts, particles), particles (bitmap,
  physics, material effects, collision and death effects), decals
  (chain, map), lights (lens flare, maps), contrails and particle
  systems (physics, bitmaps), material effects: 289 effects and the
  other tags counted above, all parsed.
- All effect-family tags common to `bloodgulch.map` and
  `hangemhigh.map` (550) and `damnation.map` (547) compared field by
  field (references by path): identical.
- `pphy` runtime fields checked against the formulas on all 30 tags;
  the particle "1 / sprite size" against the bitmaps' sprite rectangles
  on all 96 tags (within one pixel; 1.0 for two lens-flare particles).
- Effect event timing simulated in float32 for the delays and
  durations quoted.
- 2026-11-18 (particle size, orientation, first person; data only:
  the behaviour reference was not available to this session, so
  nothing in this batch was read from it): on `bloodgulch.map`, the
  bitmap tag type, sequences, sprites and pixel sizes of every
  particle's bitmap; the "1 / sprite size" rule over used sequences
  checked on all 94 sprite-group particles; the two non-sprite
  particles' bitmaps (type, sizes, formats, registration) and the
  `flares weapon lens` and `flares_generic` pixel profiles decoded
  (y8 and x8r8g8b8, to stdout); the energy flash and smoke sprites
  decoded (DXT1) to measure the visible extent inside the quad; every
  effect entry using the two flare particles listed with its radius;
  the plasma pistol and plasma rifle fire effects' entries; the
  plasma pistol and plasma rifle first-person models' nodes and muzzle
  markers; all 8 contrails with their projectiles' attachments; all 22
  `lens` tags' bitmaps and reflection radii.
- Statistics: 576 events (23 with a skip fraction), 461 parts
  (214 decal, 221 sound, 11 damage, 9 light, 6 particle system, 0
  object), particle entries' camera modes 0/1/2/3 = 1737/80/93/19.

## Confidence per section

| Section | Confidence |
|---|---|
| Effect tag layout, enumerations, flags | checked against the disc; names of flags and enumeration values from general knowledge of the tag set, matching the reference's meaning |
| Scale flags bit assignment | reference; disc uses consistent bits |
| Creating an effect, locations, instances, camera modes, mode, environment, visibility | reference only |
| Event timing, distribution functions, loop and 8-step cap | reference; timings computed |
| Parts and particle entries behaviour, random draw order | reference only |
| Particle tag layout | checked against the disc (0x6C..0x73 unused per the reference; leftovers on the disc) |
| Particle behaviour, size, fade, drawing | reference only; "1 / sprite size" relation (largest sprite over the used sequences spans 2R) checked on all 94 sprite particles of the disc; the non-sprite rule (whole bitmap = 2R square) inferred from the disc's stored 1.0 and the impact sizes it would otherwise give, not read from the reference; orientation 1 and 2 vector construction inferred |
| First-person particles | the radii, markers and model sizes checked against the disc; "drawn with the first-person weapon's projection" from the reference (earlier batch); that projection's field of view not determined |
| Point physics layout and runtime formulas | checked against the disc; integration from the reference |
| Particle systems layout | checked against the disc; behaviour reference only (per-state physics and its reversed blend, the drawing scale in WU per sprite pixel and the rotational sprites read from the reference's update and draw code); the frag explosion's values and sizes from the disc |
| View-filling sprite cap (11 per view frame) | reference only |
| Contrails layout | checked against the disc (attachments, render types, widths re-checked 2026-11-18); behaviour reference only (per-point duration draws, the dropped overshoot and the expiry after the last state's own duration read from the reference); flag names bits 2 to 5 inferred |
| Lights layout | checked against the disc (0xD4..0xE3 values present but not read per the reference); behaviour reference only; duration in ticks inferred from the reference's use and the disc's values |
| Lens flares | layout checked against the disc; behaviour not specified |
| Glows | layout from the reference only (no tag on the disc's multiplayer maps) |
| Decals layout | checked against the disc; layer names inferred |
| Decal placement and lifetime | reference only |
| Material effects layout and disc contents | checked against the disc; kind names 6, 7, 10 inferred; callers from the reference |
| Damage effect presentation | layout `projectiles.md` (disc); behaviour reference only; disc values checked |
| What is on the disc | checked against the disc (three maps) |
| Per-tick order, limits | reference only (pool sizes are the original's as the reference records them) |

## Open questions

1. **Particle update rate.** The Xbox updated particles, contrails and
   particle systems once per rendered frame. The game ran at 30 frames
   per second in multiplayer, so this spec treats it as once per tick;
   if the Xbox dropped frames, effects ran with larger dt. A test on
   hardware (frame-step the AR flash) would confirm "two frames".
2. **Light duration 0.** The muzzle-flash-style effect lights with
   duration 0 divide 0 by 0 in the fall-off; the spec takes full
   intensity. None of the multiplayer weapons uses such an effect
   light (their muzzle lights are attachments), so it is moot for the
   first port.
3. **Lens-flare particles' size.** Resolved from the data (2026-11-18):
   both bitmaps are 2D-texture tags, not sprite groups, and the whole
   bitmap is drawn as a 2R square (How a particle lives, World size).
   What stays inferred is that the engine treats a non-sprite frame as
   one unit square rather than as its pixel size; the reference was
   not available to confirm it. Check on hardware or a capture: a
   charged plasma pistol shot into a wall shows a light-green additive
   glow about 4 to 6 WU (12 to 18 m) across at most, its bright core
   about an eighth of that, gone within a quarter second; an assault
   rifle bullet on wood or metal shows one well under 1.5 WU.
4. **Wind and water current** values for point physics with the wind
   flags: owned by the level spec (`scenario.md` / `structure_bsp.md`
   wind); this spec assumes zero without the flags.
5. **Sprite orientation 1 and 2** basis construction and the
   "rotational" complex sprites of particle systems are described in
   outline. Size and Orientation now give a vector construction
   (orientation 1: x along the direction, y = normalize(d x e)) that
   matches the disc's streak sprites (all long along the picture's x)
   but is inferred, not read; which end of the picture leads, whether
   the rotation draw applies to orientations 1 and 2, and the basis
   of orientation 2 remain to pin from the reference.
6. **Effect shader flags** names (bit 0 to 2) are inferred; only "bit 1
   nonlinear tint" changes behaviour here.
7. **Decal depth offset** magnitude is a renderer setting not in the
   tags; any small polygon offset will do.
8. **Lens flare drawing** (occlusion test, fades, reflections along
   the screen line, corona rotation): not specified; the multiplayer
   uses are the plasma bolts, the grenade quickflash, the needler and
   flashlights. Also open: the unit of a reflection's radius (the
   disc's values, 0.003 to 12, are listed under Lens flares and
   glows). Settled: particles named "lens flare" are plain particles
   and use none of a `lens` tag's fields.
9. **Material effects kinds 6, 7, 11, 12**: never played by the
   engine per the reference; their names are guesses.
10. **The 0x6C..0x73 leftovers** in `part` tags look like unconverted
    big-endian copies of fields from an older layout; harmless, but a
    hint that the field set changed late.
11. **First-person projection.** First-person particles are drawn
    with the first-person weapon's projection (reference, step 9 of
    One particle); whether that projection's field of view or near
    plane differs from the world camera's (`weapons.md` section 12) is
    not determined, and no tag carries one. It does not change their
    size relative to the first-person gun. Check from the reference's
    first-person weapon drawing, or on a capture: the plasma pistol's
    core muzzle flash appears about as wide as the pistol.
12. **Contrail segment direction at interior points.** Whether a
    ribbon vertex pair uses the segment to the newer point, to the
    older one, or their mean, and what happens when that segment
    points at the eye; only visible on curved or end-on ribbons.
