# Halo: Combat Evolved: objects, units, bipeds, items and scenery

Source basis: the game's own map files from the original Xbox disc (all
24 `.map` files, the multiplayer ones in depth: every object-derived tag
of Blood Gulch parsed with the layouts below, other maps cross-checked)
and the disc's executable for a few engine constants; and a
decompilation of an Xbox build of the game's engine (the object, unit,
biped, item, equipment and device tag definitions, the per-tick object
update, the function maths, the change colour code), used for meaning
only. The decompilation describes a different Xbox build (PAL) from the
disc's (NTSC); where they differ the disc wins and the difference is
noted. Conventions (tag index, blocks, tag references, units, axes, the
tick) are in [cache_file.md](cache_file.md) and are not repeated here.
Status: draft

Sections: Summary; Units and conventions; Constants; Struct sizes and
where each class's own fields begin; The object base; Object functions;
Change colours; The unit base; The biped; Items, equipment, garbage;
Scenery, sound scenery and devices; The disc's values; Per-tick order;
Edge cases; Quirks; Test cases; Verification performed; Confidence per
section; Open questions.

---

## Summary

- Every object tag (biped, vehicle, weapon, equipment, garbage,
  projectile, scenery, sound scenery, machine, control, light fixture) is
  a struct that begins with the same 0x17C-byte *object base*. A derived
  class's own fields follow directly: the unit base (0x174 bytes) after
  the object base for bipeds and vehicles, the biped's own fields (0x204
  bytes) after that; the item base (0x18C bytes) after the object base
  for weapons, equipment and garbage. The offsets in this spec are all
  absolute from the start of the tag's main struct, so a spec for a
  derived class can use them as they are.
- The object base holds what every object has: type code, flags, the
  bounding sphere, the references to model, animation graph, collision
  model, physics, modifier (shield) shader and creation effect, four
  *function inputs* (A to D, chosen from a list of sources such as shield
  vitality), and five blocks: attachments (lights, sounds, effects,
  contrails, particle systems), widgets, up to four *object functions*
  (small waveform generators that turn inputs and time into a number from
  0 to 1 that drives lights, shaders and animations), up to four *change
  colours*, and predicted resources.
- A multiplayer Spartan is a biped tag, `characters\cyborg_mp\cyborg_mp`,
  whose own fields hold almost nothing about speed: the walking speeds,
  accelerations and the like come from the globals tag (movement spec).
  The biped fields that matter in multiplayer are the jump velocity, the
  heights, radius and camera heights, the slope limits and the landing
  thresholds. Its armour colour is not in the tag: the game creates the
  unit with the player's colour in all four change colour slots, and the
  tag has change colours only for slots A and B, so slots C and D keep
  the player's colour, and the armour's shaders read slot C.
- Scenery has no fields of its own (128 bytes of padding); equipment has
  four (powerup type, grenade type, powerup time, pickup sound).

## Units and conventions

Everything in [cache_file.md](cache_file.md) applies: little-endian, tag
data addresses, 1 world unit (WU) = 3.048 m, +Z up, +X forward at yaw 0,
angles in radians, 30 ticks per second. This spec adds:

- **Offsets** are in bytes from the start of the tag's main struct (the
  address in the tag index entry), not from the start of a layer. Fields
  of a block element use offsets from the start of the element.
- **Unit of each field** is stated in the tables. The rule from the
  conventions spec holds: the stored value is what the engine uses, with
  no conversion on load, and the build tool had already converted some
  fields when it made the map. In this class family, checked against the
  disc and the engine's use:
  - per tick: biped jump velocity (WU/tick);
  - per second: biped landing velocities (WU/s; the engine divides by
    30), unit grenade velocity (WU/s), the moving turning speed (rad/s),
    unit aiming and looking velocity maxima (rad/s) and accelerations
    (rad/s squared), biped flying velocities (unused on the disc);
  - seconds: all times (landing times, ping interrupt times, powerup
    time, detonation delays, the object function period);
  - radians: all angles (camera field of view, slope angles, pitch
    range, turning threshold);
  - WU: heights, radii, offsets, camera heights, autoaim width.
- A **function input index** (used by object functions and change
  colours) is an i16: 0 none; 1 to 4 the object's incoming inputs A to D;
  5 to 8 the object's own outgoing functions A to D. Any other value
  reads as 0. An **attachment function reference** is an i16: 0 none;
  1 to 4 the outgoing function A to D (stored value minus 1 is the
  function's index). A **turn off with** value is an i16: -1 none, else
  the index (0 to 3) of an outgoing function.
- "NONE" for an i16 index is -1.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| OBJECT_BASE_SIZE | 0x17C | bytes | size of the object base |
| UNIT_BASE_SIZE | 0x174 | bytes | the unit layer (bipeds, vehicles) |
| BIPED_OWN_SIZE | 0x204 | bytes | the biped layer |
| ITEM_BASE_SIZE | 0x18C | bytes | the item layer (weapons, equipment, garbage) |
| MAX_ATTACHMENTS | 8 | | per object; more are ignored |
| MAX_FUNCTIONS | 4 | | object functions computed; more are ignored |
| MAX_CHANGE_COLORS | 4 | | A to D |
| FUNCTION_TABLE_SIZE | 1024 | entries | periodic and transition tables |
| FUNCTION_TIME_SCALE | 25.6 | table entries per unit time | table index = time x 25.6 |
| FUNCTION_PHASE_STRIDE | 57 | ticks | per-object phase offset, see Object functions |
| FUNCTION_EPSILON | 0.0001 | | "at the lower bound" tolerance |
| TICK_SECONDS | 1/30 | s | time of one tick (0.033333335 as stored) |

## Struct sizes and where each class's own fields begin

Measured on the disc: the first block a tag points to (its attachments,
or its first non-empty block) starts exactly where its struct ends, for
the classes that have one. This gives the struct sizes below; the
derived classes' own parts are owned by other specs.

| Class | Struct size | Own fields begin at | Owner of own fields |
|---|---|---|---|
| object base (all classes) | 0x17C | 0x000 | this spec |
| `scen`, `ssce` | 0x1FC | 0x17C (all padding, 128 bytes, zero) | this spec: none |
| `proj` | 0x24C | 0x17C | projectiles.md |
| `unit` layer (`bipd`, `vehi`) | | 0x17C (0x174 bytes) | this spec |
| `bipd` | 0x4F4 | 0x2F0 (0x204 bytes) | this spec |
| `vehi` | 0x3F0 | 0x2F0 (0x100 bytes) | vehicles.md |
| item layer (`weap`, `eqip`, `garb`) | | 0x17C (0x18C bytes) | this spec |
| `weap` | 0x508 on 14 of 15 tags (see Open questions) | 0x308 | weapons.md |
| `eqip` | 0x3B0 | 0x308 (0x18 bytes of fields, then 0x90 of zero padding) | this spec |
| `garb` | 0x3B0 | 0x308 (0xA8 bytes of zero padding; no fields) | this spec: none |
| device layer (`mach`, `ctrl`, `lifi`) | | 0x17C (0x114 bytes) | this spec (briefly) |
| `mach` | 0x324 | 0x290 (0x94 bytes) | this spec (briefly) |
| `ctrl` | 0x324 (reference only; no disc tag) | 0x290 (0x94 bytes) | this spec (briefly) |
| `lifi` | 0x2D0 | 0x290 (0x40 bytes, all zero on the disc) | this spec: none known |
| `plac` | not on the disc | | |

Sizes of weapon, projectile and vehicle structs are by this measure; the
specs that own them state their own fields' offsets either absolute (the
numbers above plus their field offset) or relative to the "own fields
begin at" column. **This spec uses absolute offsets everywhere.**

## The object base

All offsets absolute. "pad" ranges are zero on every tag checked.

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 2 | enum16 | object type | | the object type code, see below; equals the tag's class on every tag |
| 0x002 | 2 | flags16 | flags | | see Object flags |
| 0x004 | 4 | f32 | bounding radius | WU | radius of the bounding sphere used for culling, visibility and cheap overlap tests |
| 0x008 | 12 | real_point3d | bounding offset | WU | sphere centre relative to the object's origin (root node), in the object's frame |
| 0x014 | 12 | real_point3d | origin offset | WU | the model is drawn and the object placed with this shift from the object's position |
| 0x020 | 4 | f32 | acceleration scale | | multiplies the push the object receives from a damage effect's instantaneous acceleration (0 = unmoved) |
| 0x024 | 4 | flags32 | runtime flags | | filled by the map build; bit 0: the object has at least one scaled change colour (see Change colours) |
| 0x028 | 16 | tag ref `mode` | model | | render model; null allowed |
| 0x038 | 16 | tag ref `antr` | animation graph | | null allowed |
| 0x048 | 40 | pad | | | |
| 0x070 | 16 | tag ref `coll` | collision model | | null allowed |
| 0x080 | 16 | tag ref `phys` | physics | | null allowed |
| 0x090 | 16 | tag ref (any shader class, `spla` on the disc) | modifier shader | | drawn over the model for shields and similar |
| 0x0A0 | 16 | tag ref `effe` | creation effect | | played when the object is created |
| 0x0B0 | 84 | pad | | | |
| 0x104 | 4 | f32 | render bounding radius | WU | radius used for deciding whether to draw; equals the bounding radius on all 70 object tags of Blood Gulch |
| 0x108 | 2 | enum16 | input A | | "export to functions" source for incoming value A, see Function input sources |
| 0x10A | 2 | enum16 | input B | | same for B |
| 0x10C | 2 | enum16 | input C | | same for C |
| 0x10E | 2 | enum16 | input D | | same for D |
| 0x110 | 44 | pad | | | |
| 0x13C | 2 | i16 | hud text message index | | index into the HUD message text list, used for the prompt shown when the player can interact with the object (vehicle entry prompts on the disc: 16, 23, 21; the flamethrower 12; 0 elsewhere) |
| 0x13E | 2 | i16 | forced shader permutation | | when non-zero, the shader permutation the model is forced to use; 0 on every disc tag |
| 0x140 | 12 | block | attachments | | element 0x48, at most 8 |
| 0x14C | 12 | block | widgets | | element 0x20 |
| 0x158 | 12 | block | functions | | element 0x168, at most 4 are used |
| 0x164 | 12 | block | change colors | | element 0x2C, at most 4 are used |
| 0x170 | 12 | block | predicted resources | | element 8 |

The block data of a tag follows its struct in this order on the disc:
attachments, widgets, functions, change colours (each change colour's
permutations follow the whole change colours block), predicted resources,
then the derived class's blocks. A reader must not rely on it: follow
the block addresses.

### Object type codes

| Code | Class | | Code | Class |
|---|---|---|---|---|
| 0 | `bipd` biped | | 6 | `scen` scenery |
| 1 | `vehi` vehicle | | 7 | `mach` machine |
| 2 | `weap` weapon | | 8 | `ctrl` control |
| 3 | `eqip` equipment | | 9 | `lifi` light fixture |
| 4 | `garb` garbage | | 10 | `plac` placeholder (none on the disc) |
| 5 | `proj` projectile | | 11 | `ssce` sound scenery |

Derived masks the engine uses: unit = 0 or 1; item = 2, 3 or 4; device =
7, 8 or 9. Checked on all object tags of Blood Gulch, Hang 'Em High,
Sidewinder, Chill Out, Carousel and a10 (type code equals the class).

### Object flags (offset 0x002)

| Bit | Name | Meaning |
|---|---|---|
| 0 | does not cast shadow | the object makes no shadow (the engine sets its "shadowless" state at creation) |
| 1 | transparent self occlusion | transparent parts of the model hide each other when drawn |
| 2 | brighter than it should be | the object is lit as if artificially bright (weapons and powerups on the disc: value 4) |
| 3 | not a pathfinding obstacle | AI path planning ignores the object |

Bits 4 to 15 are zero on the disc.

### Function input sources (enumeration for offsets 0x108 to 0x10E)

| Value | Source | Value read each tick |
|---|---|---|
| 0 | none | the input keeps its previous value (0 at creation) |
| 1 | body vitality | the object's body vitality fraction |
| 2 | shield vitality | the shield vitality fraction, capped at 1 (an overshield reads 1) |
| 3 | recent body damage | the decaying "recent body damage" fraction |
| 4 | recent shield damage | the decaying "recent shield damage" fraction |
| 5 | random constant | see Quirks |
| 6 | umbrella shield vitality | vitality of the umbrella shield object |
| 7 | shield stun | |
| 8 | recent umbrella shield vitality | |
| 9 | umbrella shield stun | |
| 10 to 17 | region damage 0 to 7 | damage state of model region n (0 to 255) divided by 255 |
| 18 | alive | 1 while the object is not dead, else 0 |
| 19 | compass | see below |

Values 6 to 9 are as the reference names them; their exact semantics
belong to health_shields.md (none is used by a disc object). On the disc
objects use 0, 2, 4, 19 (and the unit and item layers below).

**Compass**: with the object's forward vector f in the world, if
|f.z| < 0.995 the value is clamp(0.5 + a / (2 pi), 0, 1) where a is the
signed angle from the scenario's local north to the heading
atan2(f.y, f.x), wrapped into (-pi, pi]; otherwise the input keeps its
previous value. (One scenery object on a campaign map uses it; the
scenario spec owns "local north".)

The value of input i is written each tick before the functions are
computed. A class layer's own input list is applied after the object
layer's, and a later layer overwrites an earlier one only for slots where
it names a source. The unit, biped, item and device layers each have
their own A to D fields and enumerations (below); **no disc object
names a source in two layers for one slot** (checked on all 1839
object-derived tags of the 23 non-menu maps; the Spartan uses object
A and B and unit C).

### Attachments (element 0x48)

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 16 | tag ref | type | the attached tag; its class decides the kind (below). May be null |
| 0x10 | 32 | tag string | marker name | the model marker the attachment sits on |
| 0x30 | 2 | i16 | primary scale | attachment function reference (0 none, 1 to 4 outgoing A to D) |
| 0x32 | 2 | i16 | secondary scale | same |
| 0x34 | 2 | i16 | change color | change colour reference: 0 none, 1 to 4 = change colour A to D |
| 0x36 | 2 | pad | | |
| 0x38 | 16 | pad | | |

Kinds by the referenced tag's class: `ligh` light (primary scale
drives intensity, the change colour tints it), `lsnd` looping sound
(primary scale drives its volume), `effe` effect (primary and secondary
scale drive the effect's scale inputs, the change colour tints), `cont`
contrail, `pctl` particle system (primary scale and change colour). Any
other class, or a null reference, is ignored (the disc has a light
volume `mgs2` attached to one campaign projectile, and two empty
attachments on the multiplayer ghost). At most 8 attachments are created.

### Widgets (element 0x20)

A 16-byte tag reference followed by 16 bytes of padding. Classes on the
disc: `ant!` antenna (warthog, scorpion), `mgs2` light volume (plasma
bolts, rocket, needle, the assault rifle's flashlight), `flag` (the flag
weapon's cloth). Their behaviour belongs to effects.md and vehicles.md.

### Functions (element 0x168)

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x000 | 4 | flags32 | flags | | bit 0 invert, bit 1 additive, bit 2 does not deactivate below lower bound ("always active") |
| 0x004 | 4 | f32 | period | s | see Object functions |
| 0x008 | 2 | i16 | scale period by | | function input index; 0 none |
| 0x00A | 2 | enum16 | function type | | periodic function type, see below |
| 0x00C | 2 | i16 | scale function by | | function input index; 0 none |
| 0x00E | 2 | enum16 | wobble function type | | periodic function type |
| 0x010 | 4 | f32 | wobble period | s | |
| 0x014 | 4 | f32 | wobble magnitude | | 0 disables the wobble |
| 0x018 | 4 | f32 | square wave threshold | | 0 disables |
| 0x01C | 2 | i16 | step count | | at most 1 disables |
| 0x01E | 2 | enum16 | map to | | transition function type: 0 linear, 1 early, 2 very early, 3 late, 4 very late, 5 cosine |
| 0x020 | 2 | i16 | sawtooth count | | at most 1 disables |
| 0x022 | 2 | i16 | add | | function input index; 0 none |
| 0x024 | 2 | i16 | scale result by | | function input index; 0 none |
| 0x026 | 2 | enum16 | bounds mode | | 0 clip to bounds, 1 clip to bounds and normalize, 2 scale to fit bounds |
| 0x028 | 4 | f32 | lower bound | | |
| 0x02C | 4 | f32 | upper bound | | |
| 0x030 | 6 | pad | | | zero on the disc |
| 0x036 | 2 | i16 | turn off with | | -1 none, else outgoing function index 0 to 3 |
| 0x038 | 4 | f32 | scale by | | multiplies the result when greater than 0 |
| 0x03C | 252 | pad | | | zero on the disc |
| 0x138 | 4 | f32 | runtime: 1 / (upper - lower) | | filled by the build; 0 when the bounds are equal |
| 0x13C | 4 | f32 | runtime: sawtooth modulus | | 1 / (sawtooth count - 1) when the count is above 1, else 0 |
| 0x140 | 4 | f32 | runtime: step factor | | 1 / (step count - 1) when the count is above 1, else 0 |
| 0x144 | 4 | f32 | runtime: 1 / period | | 0 when the period is 0 |
| 0x148 | 32 | tag string | usage | | editor label ("shield glow source", "flashlight"); ignored |

The four runtime fields were checked on all 1616 object functions of the
23 non-menu maps (every one equals the stated formula; sawtooth counts
seen: 0, 1, 4; step counts seen: 0, 1, 2, 6, 9, 11). Use them as stored;
a synthetic tag must fill them (a reader may also recompute them).

**Periodic function types** (function type and wobble function type):
0 one, 1 zero, 2 cosine, 3 cosine (variable period), 4 diagonal wave,
5 diagonal wave (variable period), 6 slide, 7 slide (variable period),
8 noise, 9 jitter, 10 wander, 11 spark. Values 12 and above evaluate as
"one". Use counts over all 1616 disc functions: one 1402, zero 15,
cosine 45, cosine var. 8, diagonal 5, diagonal var. 2, slide 18, slide
var. 16, noise 2, jitter 41, wander 23, spark 39. Wobble type: 0 on all
1616; scale period by: used 22 times; add: used 5 (a50 doors); scale
result by: never; wobble magnitude: never; square wave threshold: used 7 times
(a10 shield charge 0.5; a50 holo objects 0.5).

### Change colours (element 0x2C)

| Offset | Size | Type | Name | Meaning |
|---|---|---|---|---|
| 0x00 | 2 | i16 | darken by | function input index; 0 none |
| 0x02 | 2 | i16 | scale by | function input index; 0 none |
| 0x04 | 4 | flags32 | scale flags | bit 0 blend in HSV space, bit 1 take the long way round the hue circle |
| 0x08 | 12 | rgb | color lower bound | 0..1 each |
| 0x14 | 12 | rgb | color upper bound | 0..1 each |
| 0x20 | 12 | block | permutations | element 0x1C |

Permutation element (0x1C): weight f32 (0..1, cumulative: the weights of
one change colour rise to 1.0), color lower bound (rgb, 12), color upper
bound (rgb, 12).

### Predicted resources (element 8)

i16 type (0 bitmap; 1 sound is inferred, no disc tag has one), i16
resource index (0 on the disc), tag id u32 of the bitmap or sound. A hint
for pre-loading; ignore it (all 228 entries of Blood Gulch's object tags
are `bitm` tags with type 0).

## Object functions

An object has up to 4 *outgoing* values (functions A to D), computed
each tick from up to 4 *incoming* values (inputs A to D, from the
sources above) and time. Consumers (attachments, model animation
overlays, shaders, the HUD) read the outgoing values and an *active*
flag per function (inactive = treat as off, for example a light off).

### Time

For function evaluation an object's time is t = (57 x s + k) / 30 s,
where k is the game tick and s is the object's slot number in the
object table (so two objects with the same function are out of phase).
Any stable per-object integer for s is fine in an implementation; 57 is
the constant. Reference only.

### One function, in order

Inputs read below are looked up by function input index (0..8 as defined
in Units and conventions; index 0 never reaches a lookup). Let p0 = the
function's 1 / period (stored runtime field).

1. If scale period by is set and its input value v is above 0, p0 is
   divided by v (the period becomes the period x v). A value of 0 or
   below leaves p0 unchanged.
2. x = periodic(function type, t x p0), the periodic function below.
3. If scale function by is set: x = x times that input.
4. If the invert flag is set: x = 1 - x.
5. If wobble magnitude is not 0: x = x + 2 m (periodic(wobble type, t x
   wobble period) - 0.5), m the wobble magnitude. (The wobble period is
   multiplied, not inverted: see Open questions.)
6. If the square wave threshold is not 0: x = 1 if x is above the
   threshold, else 0.
7. If step count is above 1: x = floor(step count x x) x step factor.
8. If the sawtooth modulus is above 0: x = x modulo that modulus (the
   remainder keeps the sign of x; x is not rescaled).
9. If add is set: x = min(x + that input, 1).
10. If scale result by is set: x = x times that input.
11. y = transition(map to, clamp(x, 0, 1)) (below).
12. If scale by is above 0: y = y x scale by.
13. Bounds, with lo and hi the bounds and e = 0.0001:
    - mode 2 (scale to fit): y = y (hi - lo) + lo; active is false when
      lo + e >= y, unless flag bit 2 is set; else active.
    - modes 0 and 1: if lo + e >= y then y = lo and active is false
      unless flag bit 2 is set (else active is true); then if y > hi,
      y = hi; then in mode 1 only, y = (y - lo) x (1 / (hi - lo)).
14. If turn off with is not -1 and the outgoing function it names is
    inactive (its flag from this tick if that function was already
    computed this tick, else from the last tick), active becomes false.
15. If the additive flag (bit 1) is set: y = (y + the function's previous
    outgoing value) modulo 1, so the output accumulates; the modulo is by
    1.0 and the previous value is last tick's.

The outgoing value of function i is y and its active flag is stored.
Functions are computed in order A, B, C, D, so function j sees this
tick's values of functions with a lower index and the last tick's of the
others. Only the first 4 functions of the block are computed.

A consumer that asks for "function NONE" (-1) gets value 1.0, active.

### Periodic functions

Each type has a table of 1024 bytes. periodic(type, time): type 0 (one)
returns 1. Otherwise let u = time x 25.6, i = floor(u) (rounded toward
minus infinity) and f = u - floor(u) in [0, 1); take i modulo 1024. With
a = table[i] / 255 and b = table[(i + 1) modulo 1024] / 255 the result
is (1 - f) a + f b, clamped to [0, 1]. For slide and slide (variable
period) there is no clamp and the wrap is handled: if a > 0.75 and
b < 0.25 then b = b + 1; r = (1 - f) a + f b; the result is r - 1 when
r is above 1, else r.

Table construction (done once at start): sample s_n for n = 0..1023 with
x = n x 28 / 1024 (the stored constant is 0.027343748), w = 28 times a
monotone random table (see below):

| Type | s_n |
|---|---|
| 1 zero | 0 |
| 2 cosine | cos(2 pi x) |
| 3 cosine (variable period) | cos(2 pi w) |
| 4 diagonal wave | with q = frac(x): 2q when q < 0.5, else 2 - 2q |
| 5 diagonal wave (variable period) | the same with q = frac(w) |
| 6 slide | frac(x) |
| 7 slide (variable period) | frac(w) |
| 8 noise | a uniform random number in [0, 1) |
| 9 jitter, 10 wander | the same table: 0.5 (cos(25.132742 x) cos(0.89759791 x) + cos(43.9823 x) sin(1.5707964 x)) + sin(3.1415927 x) cos(6.2831855 x) |
| 11 spark | frac(w) squared |

For every type except 6 and 7 the samples are then normalised by the
table's own minimum and maximum, v = (s - min) / (max - min) (when the
range is 0 the sample is kept as is), and the byte is
trunc(255 v) clamped to 0..255. For slide types no normalisation is done:
the byte is trunc(255 s). (So the cosine table is the cosine rescaled to
bytes from 0 for the trough to 255 for the crest; a 0 to 1 wave, not
-1 to 1.)

Random monotone table (types 3, 5, 7, 11): an array X with X_0 = 0 and
each next entry larger than the last by 0.25 + 0.25 u1 + (cos(8.2 pi n /
1024) + 1) u2 + (cos(10.2 pi n / 1024) + 1) u3 + (cos(14.6 pi n / 1024) +
1) u4 (the u are fresh uniform random numbers in [0, 1)), then all
entries divided by the grand total so X runs from 0 up to just below 1;
w_n = 28 X_n. The random numbers come from the engine's generator seeded
with 0x20F3F660 at start-up; the exact generator is not specified here
(Open questions). An implementation may use any generator: the random
types are only used for visual flicker (lights, glows).

**Time scale quirk.** Because the tables hold 28 cycles over 1024
entries and are indexed at 25.6 entries per unit time, one cycle of
cosine, diagonal wave and slide lasts 1024 / (28 x 25.6) = 1.428571
units of t x p0, not 1: a function with period 1 s cycles every 1.4286 s.
Keep it (it is what the disc's game does: the constants 25.6, 28 and
0.027343748 are all in the disc's executable). Jitter and wander cover
their table with fractional-frequency terms and do not repeat visibly.

### Transition functions

transition(type, v) with v already clamped to [0, 1]: type 0 (linear)
returns v exactly. The other types use a table of 1024 bytes with
entry k = trunc(255 g(k / 1023)), g(z) = sqrt z (early), z^(1/4) (very
early), z^2 (late), z^4 (very late), 0.5 (sin(pi z - pi/2) + 1) (cosine).
The lookup interpolates linearly between two neighbouring entries around
v x 1023 (the reference offsets the index by half an entry; see Open
questions) and returns the byte divided by 255. Implementations may
compute g directly; the table's result differs from g by at most 2/255 =
0.008 (v = 1 returns 254/255 in the reference, not 1; keep a tolerance).

## Change colours

Each change colour slot i in 0..3 holds an RGB colour (the *base* and
the *outgoing* colour) used to tint model shaders (a shader reads a slot
through its "change colour source": 0 none, 1 A, 2 B, 3 C, 4 D; in Blood
Gulch three model shaders read C, the Spartan's armour, visor and
first-person hands, and two read A, a tree's leaves and the scorpion's
hull) and attachments.

At creation (before the first tick), for slot i in 0..3:

1. base_i = the colour the placement supplied for slot i. A placement
   with no colours supplies white (1, 1, 1) in all four.
2. If i is below the number of change colours in the tag, and the tag's
   change colour i has permutations: compute
   w = frac(|x 315.89313 + y 587.12946 + z 744.12415 + i 431.12894|)
   from the object's position (x, y, z) in WU. Pick the first
   permutation whose weight is at least w (weights are cumulative, the
   last is 1.0, so one is always found). base_i = the colour found by
   blending the permutation's lower and upper bound in HSV space with the
   factor frac(|y| + i x 0.71211) (the object's y, in WU). Permutations
   choose a fixed variation per place.
3. outgoing_i = clamp(base_i, 0, 1).

Then, at creation and on every tick after the function update, if the
tag's runtime flag bit 0 is set (the build sets it when any change colour
has a scale by function): for slot i below the tag's number of change
colours (at most 4):

1. If scale by is set: outgoing_i = blend(lower, upper, v) where v is the
   function input value (index 1..8) of scale by, blended in RGB, or in
   HSV space when scale flag bit 0 is set. The hue path: when bit 1 is
   clear the hue is taken the short way round (if the hues differ by more
   than 0.5, the smaller is raised by 1 before blending); when bit 1 is
   set the long way; a hue above 1 after blending is reduced by 1.
   When scale by is not set, outgoing_i keeps its value (it is not reset
   to base).
2. If darken by is set: outgoing_i is multiplied (each component) by the
   function input value of darken by. Because step 1 does not run for a
   slot with no scale by, a darken-only slot would shrink every tick; no
   disc tag has one.
3. Each component is clamped to 0..1.

Slots at or above the tag's change colour count keep outgoing = base for
the life of the object.

**Spartan multiplayer colour.** At spawn the game creates the unit with
all four placement colours set to one colour: in team games, red
(1, 0, 0) for team 0 and blue (0, 0, 1) for any other team; in free for
all, the player's profile colour from this table of 18 (stored as 0xRRGGBB,
index clamped to 0..17; the table is in the disc's executable, checked):

| Index | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|---|
| RGB | FFFFFF | 000000 | FE0000 | 0201E3 | 707E71 | FFFF01 | 00FF01 | FF56B9 | AB10F4 |

| Index | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 |
|---|---|---|---|---|---|---|---|---|---|
| RGB | 01FFFF | 6493ED | FF7F00 | 1ECC91 | 006401 | 603814 | C69C6C | 9D0B0E | F5999E |

(The colour for each component is the byte / 255. Which colour a player
has chosen is the multiplayer rules' business.) The multiplayer Spartan
tag has only change colours A and B (the shield glow and a white), no
permutations, so slots C and D keep the player's colour, and the armour
shaders (change colour source C) show it. The campaign Spartan
(`characters\cyborg\cyborg`) has a third change colour, slot C, with one
permutation of weight 1 and colour (0.341, 0.404, 0.145), which is why
the campaign armour is always green whatever colour the unit is created
with.

## The unit base

Absolute offsets; the layer starts at 0x17C (layer offset in brackets
where useful). Shared by bipeds and vehicles.

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x17C | 4 | flags32 | flags | | see Unit flags |
| 0x180 | 2 | enum16 | default team | | 0 default, 1 player, 2 human, 3 covenant, 4 flood, 5 sentinel, 6 to 9 unused |
| 0x182 | 2 | enum16 | constant sound volume | | how far the unit's constant noise carries for AI hearing (0 silent, 1 medium, 2 loud, 3 shout, 4 quiet; inferred, see Open questions) |
| 0x184 | 4 | f32 | rider damage fraction | fraction | share of a vehicle's damage passed on to its riders (0.18 on the disc's vehicles) |
| 0x188 | 16 | tag ref `effe` | integrated light toggle | | effect played when the unit's flashlight is toggled |
| 0x198 | 2 | enum16 | input A | | unit input source, see below |
| 0x19A | 2 | enum16 | input B | | |
| 0x19C | 2 | enum16 | input C | | |
| 0x19E | 2 | enum16 | input D | | |
| 0x1A0 | 4 | angle | camera field of view | rad | 1.2217305 (70 degrees) on the disc |
| 0x1A4 | 4 | f32 | camera stiffness | | stiffness of the camera's response to impulses |
| 0x1A8 | 32 | tag string | camera marker name | | the camera's marker; empty on bipeds (the biped's camera height fields are used) |
| 0x1C8 | 32 | tag string | camera submerged marker name | | |
| 0x1E8 | 4 | angle | pitch auto level | rad | |
| 0x1EC | 4 | angle | pitch range minimum | rad | |
| 0x1F0 | 4 | angle | pitch range maximum | rad | |
| 0x1F4 | 12 | block | camera tracks | | element 0x1C: tag ref `trak` (16) then 12 bytes padding |
| 0x200 | 12 | real_vector3d | seat acceleration scale | | scales the unit's measured acceleration into the seat's lean animation; 0 for the Spartan, 200, 200, 150 for the warthog |
| 0x20C | 4 | f32 | integrated light power on time | s | |
| 0x210 | 4 | f32 | integrated light power off time | s | |
| 0x214 | 4 | f32 | integrated light lifespan | s | |
| 0x218 | 4 | f32 | soft ping threshold | fraction | recent damage (body or shield, as a fraction of vitality) above which the unit plays a soft flinch |
| 0x21C | 4 | f32 | soft ping minimum interrupt time | s | the flinch cannot be interrupted for this long (0.3 s = 9 ticks) |
| 0x220 | 4 | f32 | hard ping threshold | fraction | recent body damage above which the unit plays a hard flinch (1.0: never, in effect) |
| 0x224 | 4 | f32 | hard ping minimum interrupt time | s | |
| 0x228 | 4 | f32 | hard death threshold | fraction | when above 0, a killing blow with recent body damage above it plays the hard death animation |
| 0x22C | 4 | f32 | feign death threshold | fraction | with the next two and a positive chance, a non-lethal hit of this recent damage may make the unit feign death |
| 0x230 | 4 | f32 | feign death time | s | base duration; one random second is added |
| 0x234 | 4 | f32 | distance of evade animation | WU | |
| 0x238 | 4 | f32 | distance of dive animation | WU | |
| 0x23C | 4 | f32 | (unused) | | zero |
| 0x240 | 4 | f32 | stunned movement threshold | | movement is damped while the stun exceeds it (0 off) |
| 0x244 | 4 | f32 | feign death chance | probability | |
| 0x248 | 4 | f32 | feign repeat chance | probability | chance of feigning again |
| 0x24C | 16 | tag ref `actv` | spawned actor | | actor variant spawned on death (AI) |
| 0x25C | 2 | i16 | spawned actor count minimum | | |
| 0x25E | 2 | i16 | spawned actor count maximum | | |
| 0x260 | 4 | f32 | spawned velocity | | |
| 0x264 | 4 | f32 | aiming velocity maximum | rad/s | |
| 0x268 | 4 | f32 | aiming acceleration maximum | rad/s squared | |
| 0x26C | 4 | f32 | casual aiming modifier | fraction | scales the two limits above (and the looking ones) when the unit is not in combat |
| 0x270 | 4 | f32 | looking velocity maximum | rad/s | |
| 0x274 | 4 | f32 | looking acceleration maximum | rad/s squared | |
| 0x278 | 8 | pad | | | |
| 0x280 | 4 | f32 | AI vehicle avoidance radius | WU | |
| 0x284 | 4 | f32 | AI danger radius | WU | |
| 0x288 | 16 | tag ref `jpt!` | melee damage | | the damage effect of the unit's melee attack |
| 0x298 | 2 | enum16 | motion sensor blip size | | 0 medium, 1 small, 2 large (disc: warthog 0, scorpion 2, ghost 2) |
| 0x29A | 2 | pad | | | |
| 0x29C | 12 | pad | | | |
| 0x2A8 | 12 | block | new hud interfaces | | element 0x30: tag ref `unhi` (16) then 32 bytes padding; element 0 is used in single player, element 1 (or the last if there is only one) in multiplayer |
| 0x2B4 | 12 | block | dialogue variants | | element 0x18: i16 variant number, 6 bytes padding, tag ref `udlg` (16) at 8 |
| 0x2C0 | 4 | f32 | grenade velocity | WU/s | speed of a thrown grenade (10 WU/s = 30.5 m/s on the disc) |
| 0x2C4 | 2 | enum16 | grenade type | | 0 frag, 1 plasma |
| 0x2C6 | 2 | i16 | grenade count | | grenades the unit starts with (4 on the Spartan); the multiplayer rules may override |
| 0x2C8 | 2 | i16 | soft ping interrupt ticks | ticks | runtime: interrupt time x 30 (9) |
| 0x2CA | 2 | i16 | hard ping interrupt ticks | ticks | runtime |
| 0x2CC | 12 | block | powered seats | | element 0x44: u32 padding, f32 power up time (s), f32 power down time (s), 56 bytes padding (warthog 1.0 s up, 0.1 s down; ghost 2.0, 1.0) |
| 0x2D8 | 12 | block | weapons | | the unit's starting weapons: element 0x24: tag ref `weap` (16), 20 bytes padding. The multiplayer game's starting weapons come from the scenario's equipment, not from here |
| 0x2E4 | 12 | block | seats | | element 0x11C; vehicles.md owns it (below) |

The unit base ends at 0x2F0.

### Unit input sources (offsets 0x198 to 0x19E)

| Value | Source |
|---|---|
| 0 | none |
| 1 | driver seat power |
| 2 | gunner seat power |
| 3 | aiming change (the aiming change byte / 255) |
| 4 | mouth aperture |
| 5 | integrated light power |
| 6 | can blink (1 while alive and allowed to blink, else 0) |
| 7 | shield sapping |

### Unit flags (offset 0x17C)

| Bit | Name | Bit | Name |
|---|---|---|---|
| 0 | circular aiming (circular aiming angles) | 13 | impact melee dies on shield |
| 1 | destroyed after dying | 14 | cannot open doors automatically |
| 2 | half-speed interpolation (slow animation interpolation) | 15 | melee attackers cannot attach |
| 3 | fires from camera | 16 | never unsuspecting (AI) |
| 4 | entrance inside bounding sphere | 17 | shield sapping |
| 5 | (unused) | 18 | runs around flaming |
| 6 | causes passenger dialogue | 19 | inconsequential |
| 7 | resists pings | 20 | special cinematic unit |
| 8 | melee attack is fatal | 21 | ignored by autoaiming |
| 9 | does not reorient during pings | 22 | shields fry infection forms |
| 10 | has no aiming | 23 | integrated light controls weapon directly |
| 11 | simple creature | 24 | integrated light lasts forever |
| 12 | impact melee attaches to target | | |

Names are the reference's; the tag editing tool may word some of them
differently. Disc values: Spartan 0x8008 (fires from camera, melee
attackers cannot attach), warthog 0x41 (circular aiming, causes
passenger dialogue), scorpion 0x1, ghost 0.

### The seats block (summary; vehicles.md owns it)

Element 0x11C. The fields at the start: flags u32 at 0x00 (bit 0 invisible,
bit 2 driver, bit 3 gunner, bit 4 third person camera, bit 6 third person
on enter, bit 7 first person camera, bit 10 allows noncombatants; others
exist), label (tag string) at 0x04, marker name (tag string) at 0x24,
acceleration scale (3 f32) at 0x64, yaw rate (f32) at 0x7C, pitch rate
(f32) at 0x80, a camera sub-struct at 0x84 shaped like the unit's own
(marker, submerged marker, pitch auto level and range, tracks block;
0x58 bytes), a HUD block at 0xDC, yaw minimum and maximum (angles) at
0xF0 and 0xF4, built-in actor `actv` reference at 0xF8. Checked on the
disc: seat counts 3, 5 and 1 (warthog, scorpion, ghost); the seats of
each vehicle follow its weapons block without a gap.

## The biped

Own fields begin at 0x2F0 (absolute offsets). The tag struct is 0x4F4
bytes.

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x2F0 | 4 | f32 | moving turning speed | rad/s | the rate at which a moving biped turns its body toward where it looks (5.894 = 337.7 degrees per second on the Spartan; the engine uses it divided by 30) |
| 0x2F4 | 4 | flags32 | flags | | see Biped flags |
| 0x2F8 | 4 | angle | stationary turning threshold | rad | a standing biped starts to turn its body when the angle between where it faces and where it looks exceeds this (1.0472 = 60 degrees) |
| 0x2FC | 16 | pad | | | |
| 0x30C | 2 | enum16 | input A | | biped input source: 0 none, 1 flying speed |
| 0x30E | 2 | enum16 | input B | | |
| 0x310 | 2 | enum16 | input C | | |
| 0x312 | 2 | enum16 | input D | | |
| 0x314 | 16 | tag ref `jpt!` | (don't use) | | a damage effect reference the engine does not read (the melee damage used is the unit's) |
| 0x324 | 4 | f32 | bank angle | rad | flying only |
| 0x328 | 4 | f32 | bank apply time | s | flying only |
| 0x32C | 4 | f32 | bank decay time | s | flying only |
| 0x330 | 4 | f32 | pitch ratio | | flying only |
| 0x334 | 4 | f32 | max velocity | WU/s | flying only (the walking speeds are in the globals) |
| 0x338 | 4 | f32 | max sidestep velocity | WU/s | flying only |
| 0x33C | 4 | f32 | acceleration | | flying only |
| 0x340 | 4 | f32 | deceleration | | flying only |
| 0x344 | 4 | f32 | angular velocity maximum | rad/s | flying only |
| 0x348 | 4 | f32 | angular acceleration maximum | rad/s squared | flying only |
| 0x34C | 4 | f32 | crouch velocity modifier | fraction | flying only |
| 0x350 | 8 | pad | | | |
| 0x358 | 4 | angle | maximum slope angle | rad | steeper ground is not walkable; 0.7853982 (pi/4). cos of it is stored at 0x4D0 |
| 0x35C | 4 | angle | downhill falloff angle | rad | |
| 0x360 | 4 | angle | downhill cutoff angle | rad | |
| 0x364 | 4 | f32 | downhill velocity scale | factor | walking speed factor at and beyond the cutoff angle when going down (1.25) |
| 0x368 | 4 | angle | uphill falloff angle | rad | |
| 0x36C | 4 | angle | uphill cutoff angle | rad | |
| 0x370 | 4 | f32 | uphill velocity scale | factor | factor at and beyond the cutoff going up (0.65) |
| 0x374 | 24 | pad | | | |
| 0x38C | 16 | tag ref `foot` | footsteps | | the material effects used for footstep sounds and dust (`globals\cyborg`) |
| 0x39C | 24 | pad | | | |
| 0x3B4 | 4 | f32 | jump velocity | WU/tick | upward speed given by a jump (0.07 = 2.1 WU/s), before the stun penalty |
| 0x3B8 | 28 | pad | | | |
| 0x3D4 | 4 | f32 | maximum soft landing time | s | longest landing recovery after a soft landing (0.6) |
| 0x3D8 | 4 | f32 | maximum hard landing time | s | longest after a hard landing (1.0) |
| 0x3DC | 4 | f32 | minimum soft landing velocity | WU/s | falling speed that starts a soft landing (1.5) |
| 0x3E0 | 4 | f32 | minimum hard landing velocity | WU/s | falling speed that starts a hard landing (5.0) |
| 0x3E4 | 4 | f32 | maximum hard landing velocity | WU/s | speed at which the hard landing recovery is at its longest (8.0) |
| 0x3E8 | 4 | f32 | death hard landing velocity | WU/s | fatal falling speed (0 = none) |
| 0x3EC | 20 | pad | | | |
| 0x400 | 4 | f32 | standing camera height | WU | eye height above the biped's origin while standing (0.62) |
| 0x404 | 4 | f32 | crouching camera height | WU | (0.35) |
| 0x408 | 4 | f32 | crouch transition time | s | time to go between the two (0.2) |
| 0x40C | 24 | pad | | | |
| 0x424 | 4 | f32 | standing collision height | WU | height of the collision pill standing (0.7) |
| 0x428 | 4 | f32 | crouching collision height | WU | (0.5) |
| 0x42C | 4 | f32 | collision radius | WU | radius of the pill (0.2) |
| 0x430 | 40 | pad | | | |
| 0x458 | 4 | f32 | autoaim width | WU | width of the autoaim target (0.08) |
| 0x45C | 108 | pad | | | |
| 0x4C8 | 4 | f32 | runtime: cosine of stationary turning threshold | | cos(0x2F8); 0.5 for 60 degrees |
| 0x4CC | 4 | f32 | runtime: crouch transition velocity | per tick | 1 / (crouch transition time x 30) |
| 0x4D0 | 4 | f32 | runtime: minimum normal k | | cos(maximum slope angle) = 0.70710677 |
| 0x4D4 | 4 | f32 | runtime: downhill k0 | | -sin(downhill falloff angle) |
| 0x4D8 | 4 | f32 | runtime: downhill k1 | | -sin(downhill cutoff angle) |
| 0x4DC | 4 | f32 | runtime: uphill k0 | | sin(uphill falloff angle) |
| 0x4E0 | 4 | f32 | runtime: uphill k1 | | sin(uphill cutoff angle) |
| 0x4E4 | 2 | i16 | runtime: pelvis node | | model node index (0) |
| 0x4E6 | 2 | i16 | runtime: head node | | model node index (12) |
| 0x4E8 | 12 | block | contact points | | element 0x40: 32 bytes padding then the marker name (tag string) at 0x20; "left foot" and "right foot" |

The runtime fields were checked against the formulas for both disc
bipeds (all the cosines and sines agree to 7 digits). The walking
speeds are scaled by the slope: the k values are the z component of the
direction of travel (negative going down), and between k0 and k1 the
speed factor moves linearly from 1 to the velocity scale (movement.md).
The area between 0x374 and 0x4C8 that is padding is zero on both disc
bipeds. The **flying** fields (0x324 to 0x34C) are zero on every disc
biped and only matter to flying bipeds, of which the disc has none.

### Biped flags (offset 0x2F4)

| Bit | Name |
|---|---|
| 0 | turns without animating |
| 1 | uses player physics |
| 2 | flying |
| 3 | physics pill centered at origin |
| 4 | spherical |
| 5 | passes through other bipeds |
| 6 | can climb any surface |
| 7 | immune to falling damage |
| 8 | rotate while airborne |
| 9 | uses limp body physics |
| 10 | has no dying airborne |
| 11 | random speed increase |
| 12 | unit uses old player physics |

Disc values: both Spartans 0x202 (uses player physics, uses limp body
physics); campaign bipeds use 0, 0x2, 0x200 and 0x202.

### Biped input sources

0 none; 1 flying speed (the speed over the flying velocity, clamped to
0..1). Both zero on every disc biped.

## Items, equipment, garbage

### The item layer (offsets 0x17C to 0x308)

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x17C | 4 | flags32 | flags | | bit 0 always maintains z up, bit 1 destroyed by explosions, bit 2 unaffected by gravity |
| 0x180 | 2 | i16 | message index | | index into the HUD message text list for the pickup prompt |
| 0x182 | 2 | i16 | sort order | | order in the unit's inventory (0 mostly; plasma pistol 2) |
| 0x184 | 4 | f32 | scale | factor | scale of the item while it lies in the world in a multiplayer game (0 = 1); attached to a unit it is 1. In range 0.5 to 3 |
| 0x188 | 2 | i16 | hud message value scale | | scale of the number shown in the pickup prompt (flamethrower 100) |
| 0x18A | 2 | pad | | | |
| 0x18C | 16 | pad | | | |
| 0x19C | 2 | enum16 | input A | | item input source: none on every disc item (the weapon layer has its own list) |
| 0x19E | 2 | enum16 | input B | | |
| 0x1A0 | 2 | enum16 | input C | | |
| 0x1A2 | 2 | enum16 | input D | | |
| 0x1A4 | 164 | pad | | | |
| 0x248 | 16 | tag ref `foot` | material effects | | effects of the item hitting a surface |
| 0x258 | 16 | tag ref `snd!` | collision sound | | |
| 0x268 | 120 | pad | | | |
| 0x2E0 | 4 | f32 | detonation delay minimum | s | |
| 0x2E4 | 4 | f32 | detonation delay maximum | s | when an explosion destroys the item, it detonates after a random time between the two (0.4 to 0.8 s on grenades and rocket ammo) |
| 0x2E8 | 16 | tag ref `effe` | detonating effect | | played when the countdown starts |
| 0x2F8 | 16 | tag ref `effe` | detonation effect | | played at detonation |

The item layer ends at 0x308. Item flag bit 1 (destroyed by explosions)
is acted on only when no multiplayer game engine is running: a blast
then sets the item off (detonation delay and effects below). Disc
values: the flag weapon 1; frag grenade,
plasma grenade and rocket launcher ammo 2; all others 0.

### Equipment (`eqip`, struct 0x3B0)

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x308 | 2 | enum16 | powerup type | | 0 none, 1 double speed, 2 overshield, 3 active camouflage, 4 full-spectrum vision, 5 health, 6 grenade |
| 0x30A | 2 | enum16 | grenade type | | for powerup type 6: 0 frag, 1 plasma; ignored by the other types |
| 0x30C | 4 | f32 | powerup time | s | how long the powerup lasts (the engine uses it times 30 as ticks) |
| 0x310 | 16 | tag ref `snd!` | pickup sound | | played at the picker's position, unspatialised, volume 1 |
| 0x320 | 144 | pad | | | zero |

Ammo crates are equipment with powerup type 0; picking one up is handled
as ammunition by the weapon the crate names (weapons.md, multiplayer
rules).

### Garbage (`garb`)

The item layer and then 0xA8 bytes of zero padding; it has no fields of
its own. (Debris that comes off a destroyed object: two tags on a10.)

## Scenery, sound scenery and devices

**Scenery** (`scen`) and **sound scenery** (`ssce`): the object base then
128 zero bytes. Scenery's own fields are nothing; all the behaviour is
in the object base (its functions and attachments, the model, the
collision model, change colours with permutations for variety).

**Device base** (`mach`, `ctrl`, `lifi`): starts at 0x17C, ends at 0x290.

| Offset | Size | Type | Name | Unit | Meaning |
|---|---|---|---|---|---|
| 0x17C | 4 | flags32 | flags | | bit 0 position loops, bit 1 position animation not interpolated |
| 0x180 | 4 | f32 | power transition time | s | |
| 0x184 | 4 | f32 | power acceleration time | s | |
| 0x188 | 4 | f32 | powered position transition time | s | |
| 0x18C | 4 | f32 | powered position acceleration time | s | |
| 0x190 | 4 | f32 | depowered position transition time | s | |
| 0x194 | 4 | f32 | depowered position acceleration time | s | |
| 0x198 | 2 x 4 | enum16 | inputs A to D | | device input source: 0 none, 1 power, 2 change in power, 3 position, 4 change in position, 5 locked, 6 delay |
| 0x1A0 | 6 x 16 | tag refs | positive start, negative start, positive stop, negative stop, depowered, repowered effects | | `snd!` or `effe` |
| 0x200 | 4 | f32 | delay time | s | |
| 0x204 | 8 | pad | | | |
| 0x20C | 16 | tag ref | delay effect | | |
| 0x21C | 4 | f32 | automatic activation radius | WU | |
| 0x220 | 84 | pad | | | |
| 0x274 | 28 | 7 x f32 | runtime: maximum power acceleration, power velocity, depowered position acceleration and velocity, powered position acceleration and velocity, delay ticks | per tick | filled by the build |

Machine's own part at 0x290 (0x94 bytes): i16 type (0 door, 1 platform,
2 gear), u16 flags (bit 0 is a pathfinding obstacle, bit 1 is not one
when open, bit 2 is an elevator), f32 door open time (s) at 4, 80
bytes padding, i16 collision response at 0x58, i16 elevator node at 0x5A,
52 bytes padding, i32 runtime door open ticks at 0x90. Control's own part
at 0x290 (reference only, no disc tag): i16 type, i16 triggers when,
f32 call value, 80 bytes padding, three effect references (on, off,
denied) at 0x58, 0x68, 0x78, 12 bytes padding. Their behaviour is
the devices' (a later spec); checked on the disc: 19 machines of a10
parse with plausible types, times and effects; one light fixture on
Carousel.

## The disc's values

All from `bloodgulch.map` (tag index and tag ids as in that map), the
other multiplayer maps give identical values (see Verification).

### The multiplayer Spartan, `characters\cyborg_mp\cyborg_mp`

Tag 150 of Blood Gulch (class `bipd`, parents `unit`, `obje`), tag id
0xE20A0096, struct address 0x8072917C.

Object base:

| Offset | Field | Value |
|---|---|---|
| 0x000 | object type | 0 (biped) |
| 0x002 | flags | 0 |
| 0x004 | bounding radius | 0.42 WU (0x3ED70A3D) |
| 0x008 | bounding offset | (0, 0, 0) |
| 0x014 | origin offset | (0, 0, 0) |
| 0x020 | acceleration scale | 0.7 (0x3F333333) |
| 0x024 | runtime flags | 1 |
| 0x028 | model | `mode` `characters\cyborg\cyborg` |
| 0x038 | animation graph | `antr` `characters\cyborg\cyborg` |
| 0x070 | collision model | `coll` `characters\cyborg\cyborg` |
| 0x080 | physics | null (`phys`) |
| 0x090 | modifier shader | `spla` `characters\cyborg\shaders\shield hit` |
| 0x0A0 | creation effect | null (`effe`) |
| 0x104 | render bounding radius | 0.42 WU |
| 0x108 | inputs A, B, C, D | 4 (recent shield damage), 2 (shield vitality), 0, 0 |
| 0x13C | hud text message index | 0 |
| 0x13E | forced shader permutation | 0 |
| 0x140 | attachments | 1 (below) |
| 0x14C | widgets | 0 |
| 0x158 | functions | 2 (below) |
| 0x164 | change colors | 2 (below) |
| 0x170 | predicted resources | 5, all bitmaps with resource index 0 (`characters\cyborg\bitmaps\cyborg`, `cyborg multipurpose`, `detail corroded metal light`, `cyborg reflection armor`, `cubemap mirror surface`) |

Attachment 0: type `ligh` `characters\cyborg\flashlight_cyborg`, marker
`flashlight`, primary scale 1 (outgoing A), secondary 0, change colour 0.

Functions: both have flags 0, period 1.0, function type 0 (one), step
count 0, sawtooth 0, map to linear, bounds mode 0, bounds 0 to 1, turn
off with -1, scale by 0. Function A: scale function by 3 (input C),
usage `integrated light power`. Function B: scale function by 1 (input
A), usage `shield glow source`. So function A is the flashlight's
power, function B the recent shield damage.

Change colours: slot A: darken by 6 (outgoing B), scale by 2 (input B,
shield vitality), flags 1 (HSV), lower (0.74, 0.98, 1.0), upper (0.33,
0.51, 1.0), no permutations; slot B: darken by 0, scale by 7 (outgoing
C, which does not exist: reads 0), flags 0, lower (1, 1, 1), upper
(0, 0, 0), no permutations. So slot A is the colour the shield flashes,
from pale blue (full shield) toward a deeper blue (low shield), darkened
to black when no shield damage was taken recently; slot B is white.

Unit layer:

| Offset | Field | Value |
|---|---|---|
| 0x17C | flags | 0x8008 |
| 0x180 | default team | 1 (player) |
| 0x182 | constant sound volume | 0 |
| 0x184 | rider damage fraction | 0 |
| 0x188 | integrated light toggle | null (`effe`) |
| 0x198 | inputs A, B, C, D | 0, 0, 5 (integrated light power), 0 |
| 0x1A0 | camera field of view | 1.2217305 rad (0x3F9C61AA; 70 degrees) |
| 0x1A4 | camera stiffness | 0 |
| 0x1A8, 0x1C8 | camera marker names | empty, empty |
| 0x1E8, 0x1EC, 0x1F0 | pitch auto level, pitch range | 0, 0, 0 |
| 0x1F4 | camera tracks | 1: `trak` `camera\biped_small_tight` |
| 0x200 | seat acceleration scale | (0, 0, 0) |
| 0x20C to 0x214 | integrated light times | 0, 0, 0 |
| 0x218 | soft ping threshold | 0 |
| 0x21C | soft ping interrupt time | 0.3 s (0x3E99999A) |
| 0x220 | hard ping threshold | 1.0 |
| 0x224 | hard ping interrupt time | 0 |
| 0x228 to 0x248 | hard death, feign death threshold, time, evade, dive distances, stunned movement, feign chances | all 0 |
| 0x24C | spawned actor | null (`actv`) |
| 0x264 to 0x274 | aiming and looking limits, casual modifier | all 0 |
| 0x280, 0x284 | AI radii | 0, 0 |
| 0x288 | melee damage | `jpt!` `characters\cyborg\melee` |
| 0x298 | motion sensor blip size | 0 |
| 0x2A8 | new hud interfaces | 2: `unhi` `ui\hud\cyborg` (single player), `unhi` `ui\hud\cyborg_mp` (multiplayer) |
| 0x2B4 | dialogue variants | 1: variant 0, `udlg` `sound\dialog\chief\chief` |
| 0x2C0 | grenade velocity | 10.0 WU/s |
| 0x2C4 | grenade type | 0 (frag) |
| 0x2C6 | grenade count | 4 |
| 0x2C8 | soft ping interrupt ticks | 9 |
| 0x2CA | hard ping interrupt ticks | 0 |
| 0x2CC | powered seats | 0 |
| 0x2D8 | weapons | 2: `weap` `weapons\assault rifle\assault rifle`, `weap` `weapons\pistol\pistol` |
| 0x2E4 | seats | 0 |

Biped layer:

| Offset | Field | Value |
|---|---|---|
| 0x2F0 | moving turning speed | 5.893977 rad/s (0x40BC9B76) |
| 0x2F4 | flags | 0x202 |
| 0x2F8 | stationary turning threshold | 1.0471976 rad (60 degrees) |
| 0x30C | inputs A, B, C, D | 0, 0, 0, 0 |
| 0x314 | (don't use) | null (`jpt!`) |
| 0x324 to 0x34C | flying fields | all 0 |
| 0x358 | maximum slope angle | 0.7853982 rad (45 degrees) |
| 0x35C, 0x360 | downhill falloff, cutoff angle | 0.3490658 rad (20 degrees), 0.7853982 (45 degrees) |
| 0x364 | downhill velocity scale | 1.25 |
| 0x368, 0x36C | uphill falloff, cutoff angle | 0.3490658 (20 degrees), 0.7853982 (45 degrees) |
| 0x370 | uphill velocity scale | 0.65 |
| 0x38C | footsteps | `foot` `globals\cyborg` |
| 0x3B4 | jump velocity | 0.07 WU/tick (0x3D8F5C29) = 2.1 WU/s |
| 0x3D4, 0x3D8 | max soft, hard landing time | 0.6 s, 1.0 s |
| 0x3DC, 0x3E0, 0x3E4 | min soft, min hard, max hard landing velocity | 1.5, 5.0, 8.0 WU/s |
| 0x3E8 | death hard landing velocity | 0 |
| 0x400, 0x404, 0x408 | standing, crouching camera height, crouch transition time | 0.62 WU, 0.35 WU, 0.2 s |
| 0x424, 0x428, 0x42C | standing, crouching collision height, collision radius | 0.7 WU, 0.5 WU, 0.2 WU |
| 0x458 | autoaim width | 0.08 WU |
| 0x4C8 | cos stationary turning threshold | 0.4999999 (0x3EFFFFFF) |
| 0x4CC | crouch transition velocity | 0.16666667 per tick |
| 0x4D0 | minimum normal k | 0.70710677 |
| 0x4D4, 0x4D8 | downhill k0, k1 | -0.34202012, -0.70710677 |
| 0x4DC, 0x4E0 | uphill k0, k1 | 0.34202012, 0.70710677 |
| 0x4E4, 0x4E6 | pelvis, head node | 0, 12 |
| 0x4E8 | contact points | 2: "left foot", "right foot" |

At 3.048 m per WU: 0.7 WU tall is 2.13 m, 0.5 crouched is 1.52 m, the
radius is 0.61 m wide, 0.62 WU is 1.89 m for the standing eye height and
0.35 WU is 1.07 m crouched; the jump of 2.1 WU/s is 6.4 m/s; landing
thresholds 1.5, 5.0 and 8.0 WU/s are 4.6, 15.2 and 24.4 m/s.

The campaign Spartan `characters\cyborg\cyborg` (tag 1675 of Blood
Gulch) has the same biped and unit layers except:
bounding radius 0.45 and bounding offset (-0.02, 0, 0), render
bounding radius 0.45, three change colours (slot C has one permutation
of weight 1.0 with colour (0.341, 0.404, 0.145) at both bounds), unit
flags 0x447008, integrated light toggle `effe`
`weapons\assault rifle\effects\flashlight`, melee damage null,
stationary turning threshold 1.3962634 rad (80 degrees; cosine
0.17364822), hard ping threshold 0.

### Equipment (all 14, tag order of Blood Gulch)

| Path | Powerup type | Powerup time (s) | Grenade type | Pickup sound |
|---|---|---|---|---|
| `powerups\assault rifle ammo\assault rifle ammo` | 0 none | 0 | 0 | `sound\sfx\weapons\weapon_pickup_ammo\ar_ammo` |
| `powerups\pistol ammo\pistol ammo` | 0 none | 0 | 0 | `sound\sfx\weapons\weapon_pickup_ammo\pistol_ammo` |
| `powerups\shotgun ammo\shotgun ammo` | 0 none | 0 | 0 | `sound\sfx\weapons\weapon_pickup_ammo\shotgun_ammo` |
| `powerups\active camouflage` | 3 active camouflage | 45 | 0 | `sound\sfx\ui\pickup_invis` |
| `powerups\over shield` | 2 overshield | 60 | 0 | `sound\sfx\ui\pickup_dbl_shield` |
| `powerups\health pack` | 5 health | 1 | 0 | `sound\sfx\ui\pickup_health` |
| `weapons\frag grenade\frag grenade` | 6 grenade | 0 | 0 (frag) | `sound\sfx\weapons\weapon_pickup_ammo\grenade_ammo` |
| `powerups\rocket launcher ammo\rocket launcher ammo` | 0 none | 0 | 0 | `sound\sfx\weapons\weapon_pickup_ammo\rocket_ammo` |
| `powerups\sniper rifle ammo\sniper rifle ammo` | 0 none | 0 | 0 | `sound\sfx\weapons\weapon_pickup_ammo\sniper_ammo` |
| `weapons\plasma grenade\plasma grenade` | 6 grenade | 0 | 1 (plasma) | `sound\sfx\weapons\weapon_pickup_ammo\grenade_plas_ammo` |
| `powerups\flamethrower ammo\flamethrower ammo` | 0 none | 0 | 0 | none (null) |
| `powerups\needler ammo\needler ammo` | 0 none | 0 | 0 | `sound\sfx\weapons\weapon_pickup_ammo\needle_ammo` |
| `powerups\double speed` | 1 double speed | 60 | 0 | `sound\sfx\ui\pickup_dbl_time` |
| `powerups\full-spectrum vision` | 4 full-spectrum vision | 60 | 0 | none (null) |

Other item and object values of the equipment: object flags 4 (brighter
than it should be) on the camouflage, overshield, health pack and frag
and plasma grenades, 0 on the rest; bounding radius 0.2 (ammo crates;
the rocket ammo 0.2875, the sniper ammo 0.1), 0.1 (powerups and
grenades), 0.17 (health pack); acceleration scale 0.5 camouflage,
overshield and frag grenade, 0.75 health pack, 0.15 plasma grenade, 1.0
on the assault rifle, pistol, shotgun and sniper ammo, 0.9 rocket ammo,
1.5 needler ammo, 0.5 flamethrower ammo, 0 on double speed and
full-spectrum vision;
item message index 30 (camouflage), 38 (overshield), 36 (health), 2
(frag grenade), 40 (plasma grenade), else 0; item scale 1.75 (frag
grenade), 1.25 (plasma grenade), else 0; item flags 2 on the frag and
plasma grenades and the rocket launcher ammo (detonation delay 0.4 to
0.8 s; detonating effect only on the plasma grenade,
`weapons\plasma grenade\effects\detonation start`; detonation effect
`weapons\frag grenade\effects\explosion`,
`weapons\plasma grenade\effects\explosion`,
`weapons\rocket launcher\effects\rocket explosion`). The camouflage tag
uses the overshield's model (`powerups\over shield\over shield`) and
the overshield tag uses the camouflage's (`powerups\active
camoflage\active camoflage`, spelt so on the disc): the two pickups'
models are swapped on the disc. The double speed and full-spectrum
vision pickups use the model `item collections\powerups\powerups`
with no collision model. Other equipment: model and collision model of
the same path as the tag (`powerups\<name>\<name>`), none for the grenades'
collision model.

### Weapons and vehicles: object base essentials

Model class `mode`, collision `coll`, animation graph `antr`, physics
`phys`; "same" is the tag's own path; "-" is null. Bounding radius in
WU (the render bounding radius is equal in every case). Weapons: the
object function inputs are all 0 (assault rifle: A, B, C 0 and D 19,
compass); the weapon's own fields are weapons.md's.

| Tag | Class | Bounding radius | Model | Collision model | Animation graph | Physics |
|---|---|---|---|---|---|---|
| `weapons\assault rifle\assault rifle` | weap | 0.6 | same | same | - | - |
| `weapons\pistol\pistol` | weap | 0.1 | same | same | same | - |
| `weapons\shotgun\shotgun` | weap | 0.2 | same | same | same | - |
| `weapons\sniper rifle\sniper rifle` | weap | 0.4125 | same | same | same | - |
| `weapons\rocket launcher\rocket launcher` | weap | 0.2875 | same | same | - | - |
| `weapons\plasma pistol\plasma pistol` | weap | 0.2 | same | same | - | - |
| `weapons\plasma rifle\plasma rifle` | weap | 0.2 | same | same | - | - |
| `weapons\needler\needler` | weap | 0.2 | same | same | - | - |
| `weapons\flamethrower\flamethrower` | weap | 0.2 | same | same | - | - |
| `weapons\gravity rifle\gravity rifle` | weap | 0.2 | same | - | - | - |
| `weapons\ball\ball` | weap | 0.3 | same | same | - | - |
| `weapons\flag\flag` | weap | 0.45 | same | - | - | - |
| `vehicles\warthog\warthog gun` | weap | 0.2 | - | - | - | - |
| `vehicles\scorpion\scorpion cannon` | weap | 0.2 | - | - | - | - |
| `vehicles\ghost\ghost gun` | weap | 0 | - | - | - | - |
| `vehicles\warthog\warthog` | vehi | 1.1 | same | same | same | same |
| `vehicles\scorpion\scorpion_mp` | vehi | 2.2 | `vehicles\scorpion\scorpion_mp\scorpion_mp` | `vehicles\scorpion\scorpion_mp\scorpion_mp` | `vehicles\scorpion\scorpion` | `vehicles\scorpion\scorpion` |
| `vehicles\ghost\ghost_mp` | vehi | 0.8 | `vehicles\ghost\ghost` | same | `vehicles\ghost\ghost` | `vehicles\ghost\ghost` |

(The weapons' own item layer values, for weapons.md: message index and
item scale are 4/1.8 assault rifle, 20/2.25 pistol, 26/1.7 shotgun,
28/1.8 sniper rifle, 24/1.7 rocket launcher, 6/2.0 plasma pistol (sort
order 2), 22/1.8 plasma rifle, 18/1.3 needler, 12/2.0 flamethrower (value
scale 100), 16/0 gravity rifle, 1/2.2 ball, 10/0 flag (item flag 1);
vehicle guns 0/0. Item inputs all 0. Object flags 4 on assault rifle,
pistol, plasma rifle, rocket launcher, sniper rifle, plasma pistol,
needler and ball.)

Unit layer of the vehicles, for vehicles.md: flags 0x41 warthog, 0x1
scorpion, 0 ghost; default team 0; constant sound volume 1; rider damage
fraction 0.18; field of view 1.2217305; seat acceleration scale (200,
200, 150) warthog and scorpion, (100, 100, 100) ghost; aiming velocity
maximum 3.1415927 rad/s and acceleration maximum 10.471975 rad/s squared
(warthog), 1.5707964 and 13.962634 (scorpion); motion sensor blip size 0
warthog, 2 scorpion and ghost; unit inputs: warthog A 1 (driver seat
power), scorpion C 1, ghost A 1; one powered seat and one weapon each;
3, 5 and 1 seats. Object-layer inputs 0; HUD text message index 16
(warthog), 23 (scorpion), 21 (ghost).

## Per-tick order

For each object (the engine updates parents before their children):

1. The class's own update (physics, movement, animation; other specs).
2. The damage update, for objects that have a collision model.
3. Export the incoming values: the object layer's inputs A to D, then
   the unit layer's, then the biped's (or item, weapon, vehicle, device
   layer's), each writing only the slots it names a source for.
4. Compute node matrices (animation overlays read the outgoing values of
   the previous tick).
5. Compute the outgoing functions, A to D (Object functions).
6. Compute the change colours (Change colours).
7. Reconnect lights if the object moved.

At creation the same steps 3, 5 and 6 run once (with the random change
colour choice first) so that the first drawn frame has values.

## Edge cases

- A function block with more than 4 elements, an attachment block with
  more than 8, a change colour block with more than 4: only the first
  4, 8, 4 are used.
- A function input index outside 0..8 reads 0. A turn-off index outside
  0..3 and not -1: treat as none.
- A periodic function type of 12 or more evaluates as "one" (1.0); a map
  transition type of 6 or more is linear.
- Divide guards: the 1 / period runtime field is 0 for a period of 0,
  which holds the periodic argument at 0. A bounds range of 0 gives a
  normalise factor of 0.
- Attachments of an unsupported class or with a null type are skipped;
  later attachments still get their slot index.
- The object's region damage input uses regions 0 to 7 only; a larger
  index reads 0.
- A change colour slot whose scale by or darken by names an outgoing
  function that does not exist reads that function as 0.
- A tag with no model and no collision model has a bounding sphere but no
  nodes; the object is positioned as a point.
- Bounding sphere: its centre is the root node's frame applied to the
  bounding offset, its radius is the bounding radius times the object's
  scale when the object's scale is above 0.

## Quirks

- **Function period is not the true period** (see Object functions:
  cycles last 1.4286 x the period for cosine, diagonal wave, slide).
  Keep it.
- **An additive function accumulates**: each tick its output is
  (previous output + new output) modulo 1, so a constant input makes a
  sawtooth that climbs by that input every tick.
- **Random constant input**: the slot is rewritten with a new random
  number only while its stored value is exactly 1.0; otherwise it keeps
  its value. So the value is chosen once (at creation the slot's value is
  expected to be 1.0) and then constant for the object's life. No disc
  object uses it.
- **Darken without scale** accumulates (Change colours); unused.
- **Scale period by** lengthens the period when the input is below 1 and
  does nothing when the input is 0.
- **Spartan armour colour** comes from change colour slots the tag does
  not mention (above).
- **The two powerup models are swapped on the disc** (camouflage and
  overshield). Keep; players see the swapped pickups.
- **Movement fields in the biped are mostly zero**: do not look for the
  Spartan's walking speed here.
- **Transition function at 1.0** returns 254/255 in the reference.

## Test cases

### (a) Synthetic tags (unit tests)

Build a biped tag in memory: a struct of 0x4F4 bytes at an address A
(4-aligned, any), zero except the values below; block elements follow
the struct. Every offset is from A. Give the tag index a `mode` tag (id
0xE1750001, path `characters\test\test`) and a `weap` tag (id
0xE1760002, path `weapons\test\gun`), a `trak` tag (id 0xE1770003).

| Offset | Type | Value | Meaning |
|---|---|---|---|
| 0x000 | i16 | 0 | object type biped |
| 0x002 | u16 | 0x0005 | does not cast shadow, brighter than it should be |
| 0x004 | f32 | 0.5 | bounding radius |
| 0x010 | f32 | 0.25 | bounding offset z |
| 0x020 | f32 | 0.8 | acceleration scale |
| 0x024 | u32 | 1 | runtime flags |
| 0x028 | tag ref | `mode`, path address from the index, id 0xE1750001 | model |
| 0x038 | tag ref | `antr`, null (id 0xFFFFFFFF) | animation graph |
| 0x104 | f32 | 0.5 | render bounding radius |
| 0x108 | i16 x 4 | 4, 2, 0, 0 | input A to D |
| 0x13C | i16 | 3 | hud text message index |
| 0x158 | block | count 1, address A + 0x5BC | functions |
| 0x17C | u32 | 0x8008 | unit flags |
| 0x180 | i16 | 1 | default team player |
| 0x198 | i16 x 4 | 0, 0, 5, 0 | unit input C integrated light power |
| 0x1A0 | f32 | 1.2217305 | camera field of view |
| 0x21C | f32 | 0.3 | soft ping interrupt time |
| 0x220 | f32 | 1.0 | hard ping threshold |
| 0x2C0 | f32 | 10.0 | grenade velocity |
| 0x2C6 | i16 | 4 | grenade count |
| 0x2D8 | block | count 2, address A + 0x4F4 | weapons |
| 0x2F0 | f32 | 5.893977 | moving turning speed |
| 0x2F4 | u32 | 0x0202 | biped flags |
| 0x2F8 | f32 | 1.0471976 | stationary turning threshold |
| 0x358 | f32 | 0.7853982 | maximum slope angle |
| 0x364 | f32 | 1.25 | downhill velocity scale |
| 0x370 | f32 | 0.65 | uphill velocity scale |
| 0x3B4 | f32 | 0.07 | jump velocity |
| 0x400 | f32 | 0.62 | standing camera height |
| 0x404 | f32 | 0.35 | crouching camera height |
| 0x424 | f32 | 0.7 | standing collision height |
| 0x42C | f32 | 0.2 | collision radius |
| 0x4C8 | f32 | 0.5 | cosine of stationary turning threshold |
| 0x4E8 | block | count 2, address A + 0x53C | contact points |

Block data: weapons at A + 0x4F4, two 0x24-byte elements, tag
references to `weapons\test\gun` (element 0) and null `weap` (element
1); contact points at A + 0x53C, two 0x40-byte elements with the tag
strings "left foot" and "right foot" at element offset 0x20; one
function at A + 0x5BC, 0x168 bytes: period 2.0 at +0x04, function type 2
(cosine) at +0x0A, bounds 0 and 1 at +0x28 and +0x2C, turn off with -1
at +0x36, runtime fields (1.0, 0, 0, 0.5) at +0x138 to +0x144. The tag
data ends at A + 0x724.

| Setup | Input | Expected |
|---|---|---|
| the tag above | read the object layer | type 0, flags {does not cast shadow, brighter than it should be}, bounding radius 0.5, bounding offset (0, 0, 0.25), acceleration scale 0.8, model resolves to the `mode` tag, animation graph null, inputs (4, 2, 0, 0) = (recent shield damage, shield vitality, none, none), hud text index 3 |
| same | read the unit layer | flags {fires from camera, melee attackers cannot attach}, team player, input C = integrated light power, field of view 1.2217305 rad (70 degrees, to 1e-6), soft ping interrupt 0.3 s, hard ping threshold 1.0, grenade velocity 10 WU/s, grenade count 4, weapons block 2 elements (the first resolving to the gun tag, the second null) |
| same | read the biped layer | turning speed 5.893977 rad/s, flags {uses player physics, uses limp body physics}, jump velocity 0.07 WU/tick (2.1 WU/s), camera heights 0.62 and 0.35, collision 0.7 tall, radius 0.2, slope 0.7853982, downhill scale 1.25, uphill 0.65, contact points "left foot", "right foot" |
| same | struct size and layer bases | unit fields begin at 0x17C, biped fields at 0x2F0, struct 0x4F4 |
| function block count 5 | read functions | the first four only |
| weapons block count 3 with the third element past the tag data | read | rejected (outside tag data) |
| object type 12 | read | invalid (out of range) |
| input A = 40 | read | invalid (out of range) |

Object functions (period = 2.0 s so the stored 1 / period = 0.5, bounds
0 to 1, mode 0, map linear, no inputs unless stated; the object's time
t is set directly):

| Function | Time t (s) | Expected value | Active |
|---|---|---|---|
| type 0 (one) | any | 1.0 | yes |
| cosine (2) | 0 | 1.0 | yes |
| cosine | 2.5 | 217 / 255 = 0.850980 | yes |
| cosine | 5.0 | 127 / 255 = 0.498039 | yes |
| cosine | 7.5 | 37 / 255 = 0.145098 | yes |
| cosine | 10.0 | 0.0 (lower bound reached) | no |
| diagonal wave (4) | 0 | 0.0 | no |
| diagonal wave | 2.5 | 63 / 255 = 0.247059 | yes |
| diagonal wave | 5.0 | 127 / 255 = 0.498039 | yes |
| diagonal wave | 10.0 | 1.0 | yes |
| slide (6) | 2.5 | 223 / 255 = 0.874510 | yes |
| slide | 5.0 | 191 / 255 = 0.749020 | yes |
| slide | 7.5 | 159 / 255 = 0.623529 | yes |
| cosine, invert flag | 2.5 | 1 - 0.850980 = 0.149020 | yes |
| cosine, square wave threshold 0.5 | 2.5 | 1.0 | yes |
| cosine, square wave threshold 0.5 | 5.0 | 0.0 (0.498 is not above 0.5) | no |

(The table indices are t x 0.5 x 25.6 = 12.8 t: 32, 64, 96, 128 for the
times 2.5, 5, 7.5, 10; compare with 1e-6, the values are byte / 255.)

Further chains (type one, period 1.0, stored 1 / period 1.0), incoming
input A as given:

| Setup | Input A | Expected value | Active |
|---|---|---|---|
| scale function by 1 | 0.4 | 0.4 | yes |
| same | 0.0 | 0.0 | no |
| same plus flag bit 2 | 0.0 | 0.0 | yes |
| scale function by 1, bounds mode 2, bounds 0.2 and 0.8 | 0.5 | 0.5 (0.5 x 0.6 + 0.2) | yes |
| scale function by 1, bounds mode 2, bounds 0.2 and 0.8 | 0.0 | 0.2 | no |
| scale function by 1, bounds mode 1, bounds 0.2 and 0.8, stored 1 / (hi - lo) = 1.6667 | 0.5 | 0.5 ((0.5 - 0.2) x 1.6667) | yes |
| scale function by 1, scale by 0.5 | 0.4 | 0.2 | yes |
| scale function by 1, step count 9 (step factor 0.125) | 0.3 | 0.25 (floor(2.7) = 2) | yes |
| scale function by 1, add 5 (outgoing A) where outgoing A is 0.5 | 0.4 | 0.9 (0.4 + 0.5), computed as function B | yes |
| scale function by 1, additive flag (bit 1), over 4 ticks | 0.3 | 0.3, 0.6, 0.9, 0.2 (1.2 mod 1), to 1e-6 | yes |
| scale function by 1, map to late (3) | 0.5 | 0.25 within 0.01 | yes |
| function A scale function by 1, function B scale function by 1 and turn off with 0 | 0.0 | A 0.0 inactive; B 0.0, inactive | no, no |
| function A as above, function B turn off with 0 and input A 0.7 | 0.7 | A 0.7 active; B 0.7 active | yes, yes |

Change colours (scale by input A, darken by outgoing A, flags 0, lower
(0, 0, 0), upper (1, 0.5, 0.25), no permutations; the object created
with placement colour white; runtime flag set):

| Setup | Expected outgoing colour of slot 0 |
|---|---|
| input A = 0.5, outgoing A = 1.0 | (0.5, 0.25, 0.125) |
| input A = 0.5, outgoing A = 0.5 | (0.25, 0.125, 0.0625) |
| input A = 0, outgoing A = 1 | (0, 0, 0) |
| the tag has 1 change colour, placement colour (1, 0, 0) | slots 1, 2, 3 are (1, 0, 0) and stay so every tick |
| the tag has 2 change colours with no permutations and no function (a multiplayer Spartan without its glow) , placement colour (0, 0, 1) | all four slots (0, 0, 1) |
| permutations weights 0.333, 0.667, 1.0, object at position (0, 0, 0), slot 0 | w = 0, picks permutation 0; the colour is that permutation's lower bound (blend factor 0), within 1e-4 after the HSV round trip |
| permutations as above, position (0.001, 0.0, 0.0) | w = frac(0.31589) = 0.3159, permutation 0 (0.3159 is at most 0.333) |
| permutations as above, position (0.002, 0.0, 0.0) | w = frac(0.63179) = 0.6318, permutation 1 |

Team colours: a team 0 player spawns with all four slots (1, 0, 0), team
1 with (0, 0, 1); a free-for-all player with profile colour index 3
(0x0201E3) gets (2/255, 1/255, 227/255) = (0.00784, 0.00392, 0.89020).

### (b) Disc-backed (slow tests; need the user's own disc files)

`bloodgulch.map`:

| Check | Expected |
|---|---|
| tag 150 | `bipd` `characters\cyborg_mp\cyborg_mp`, id 0xE20A0096, struct 0x8072917C |
| object type, flags, bounding radius, accel scale | 0, 0, 0.42, 0.7 |
| model, collision model, animation graph | `characters\cyborg\cyborg` (classes `mode`, `coll`, `antr`) |
| modifier shader | `spla` `characters\cyborg\shaders\shield hit` |
| input A to D (object) | 4, 2, 0, 0; unit input C 5 |
| block counts: attachments, widgets, functions, change colours, predicted resources | 1, 0, 2, 2, 5 |
| unit flags, team, field of view | 0x8008, 1, 1.2217305 |
| soft ping interrupt time, hard ping threshold | 0.3, 1.0 |
| grenade velocity, type, count | 10.0, 0, 4 |
| unit blocks: camera tracks, huds, dialogue, powered seats, weapons, seats | 1, 2, 1, 0, 2, 0 |
| biped flags, moving turning speed | 0x202, 5.893977 |
| stationary turning threshold | 1.0471976 (60 degrees) |
| maximum slope, downhill falloff and cutoff | 0.7853982, 0.3490658, 0.7853982 |
| downhill, uphill velocity scale | 1.25, 0.65 |
| jump velocity | 0.07 WU/tick |
| landing times, landing velocities | 0.6 and 1.0 s; 1.5, 5.0, 8.0 WU/s |
| camera heights, crouch time | 0.62, 0.35 WU; 0.2 s |
| collision heights, radius | 0.7, 0.5, 0.2 WU |
| autoaim width | 0.08 |
| contact points | 2 ("left foot", "right foot") |
| runtime fields | cos threshold 0.5 (0.4999999 stored), min normal k 0.70710677, crouch transition velocity 0.16666667 |
| tag struct end | the attachments block address minus the tag address is 0x4F4 |
| the 14 equipment tags | the table above (path, powerup type, time, grenade type, pickup sound) |
| weapons and vehicles | bounding radius, model, collision model, animation graph, physics as in the table above |
| every object-derived tag of the map (70) | type code equals class; every model, collision, animation, physics, modifier and creation effect reference is null or of the right class; flags only in bits 0 to 3; function types 0 to 11, map to 0 to 5, bounds mode 0 to 2, function inputs 0 to 8, turn off -1 to 3; runtime function fields consistent; no padding byte non-zero |
| the same checks on `hangemhigh`, `sidewinder`, `chillout` | same values for all 34 weapon, equipment, biped, vehicle tags |

## Verification performed

A scratch reader with the layouts above parsed every object-derived tag
of `bloodgulch.map` (70: 19 scenery, 2 biped, 15 weapon, 14 equipment, 16
projectile, 3 vehicle, 1 sound scenery) and found no problem: object type
equals class for all; all references of the object base resolve to
tags of the expected class (model `mode`, collision `coll`, animation
`antr`, physics `phys`, modifier a shader class, creation effect `effe`);
object flags use bits 0 to 3 only; the object, unit, biped and item
layers' padding is zero on every tag; floats are plausible (no
denormals or huge values in radii, heights, speeds, angles); enumerations
are in range (function types 0 to 11, map to 0 to 5, bounds modes 0 to
2, function inputs 0 to 8, turn off -1 to 3, change colour indices 0 to
8, flags within their bits, powerup types 0 to 6); the block data follows
the struct in the stated order and sizes (attachments 0x48, widgets
0x20, functions 0x168, change colours 0x2C plus 0x1C per permutation,
predicted resources 8, camera tracks 0x1C, huds 0x30, dialogue 0x18,
powered seats 0x44, weapons 0x24, seats 0x11C, contact points 0x40
back to back with no gap); the unit camera tracks, huds and
weapons resolve to `trak`, `unhi`, `weap` tags; the biped runtime
fields equal the formulas; each of the 1616 functions of the 23
non-menu maps has its four runtime fields equal to the stated formulas.
Cross-map: the 34 tags (2 bipeds, 3 vehicles, 15 weapons, 14
equipment) of `bloodgulch`, `hangemhigh`, `sidewinder` and `chillout`
agree in every parsed object, unit, biped, item and equipment field,
every block element's bytes of functions and change colours, and every
reference's path. The same parser, with the object base and these
device layouts, passed on a10 (campaign: 14 bipeds, 13 vehicles, 19
machines, 2 garbage, 33 scenery; no problem) and Carousel (light
fixture). The disc's executable contains the constants 25.6, 28.0,
0.027343748 and the 18-entry player colour table (identical).

Not checked against the disc: every behaviour in "Object functions" and
"Change colours" (function evaluation, colour blending, the order of
operations) comes from the reference and was checked only for
consistency with the stored runtime fields and the plausible values of
disc functions; the numbers of the test cases for periodic functions are
computed from the table construction described above (the cosine,
diagonal wave and slide tables are deterministic).

## Confidence per section

| Section | Confidence |
|---|---|
| Struct sizes, layer bases, the object base layout and sizes | checked against the disc (70 tags on four multiplayer maps, a10) |
| Object type codes, flags | checked (codes); bit names reference only |
| Function input sources | checked (values 0, 2, 4, 19 occur on the disc; names and 1, 3, 5 to 18 reference only) |
| Attachment, widget, function, change colour, permutation, predicted resource layouts | checked against the disc |
| Function runtime fields | checked against the disc (1616 functions) |
| Object function evaluation | reference only (consistent with the runtime fields); constants 25.6 and 28 and the table slope checked in the disc's executable |
| Periodic function table shapes: cosine, diagonal, slide, jitter and wander | reference only |
| Random tables (noise, variable period, spark) | reference only, not reproducible exactly |
| Change colour creation and update; permutation choice | reference only; the Spartan's slot usage checked against the disc (change colour count, source slots C of the armour shaders) |
| Player colour table and team colours | table checked in the disc's executable; team colours and the use at spawn reference only |
| Unit base layout | checked against the disc (all fields, the three vehicles and bipeds, a10) |
| Unit flags and enumerations | values in range checked; names reference only |
| Unit field meanings and units | reference only except: field of view radians, ping times, grenade velocity per second (checked values) |
| Biped layout and the Spartan's values | checked against the disc |
| Biped units (per tick or second) | jump per tick checked against the conventions spec's disc check and the engine's use; landing per second reference only (plausible values) |
| Item layer, equipment, garbage, scenery sizes and values | checked against the disc |
| Item flags meaning | reference only |
| Device base and machine part | checked on a10's 19 machines (plausible values); control part reference only |
| Disc value tables | checked against the disc |

## Open questions

1. **The reference's weapon struct size.** One weapon tag (the sole
   `weap` whose first block follows 0x528 bytes, not 0x508) shows a
   different distance to its first block; it may have an empty first
   block. weapons.md owns the weapon size; check it there.
2. **The random periodic tables.** Noise, the variable-period types and
   spark are built from the engine's generator seeded with 0x20F3F660.
   The generator is not specified; a lookalike is fine for flicker, but a
   deterministic replay or screenshot match would need it (find it in the
   disc's executable).
3. **Transition function rounding.** The reference computes the table
   index from the value times 1023 minus 0.5 with a float-to-int
   conversion whose rounding (truncate or nearest) is not specified;
   results differ by less than 1/255. Check with a probe on the live game
   if exactness is ever needed.
4. **Wobble period.** The wobble multiplies the time by the stored wobble
   period (not by its reciprocal as the main period does). No disc
   function uses a wobble (0 of 1616), so this is unconfirmed.
5. **Random constant input.** The initial value of an incoming slot is
   not known (the rule needs it to be exactly 1.0 to draw once). No disc
   object uses the source.
6. **Constant sound volume enumeration** (silent, medium, loud, shout,
   quiet) is inferred; the disc only has 0 and 1.
7. **Predicted resource type 1** (sound) is inferred; the disc only has
   bitmaps.
8. **Unit flag bit 5 and bit 16** are named "unused" and "never
   unsuspecting" by the reference; the tag tool's names are unknown.
   Disc: bit 16 is not set on the Spartans.
9. **Function input sources 6 to 9** (umbrella shield) are not used by
   any disc object; their exact values are for health_shields.md.
10. **Light fixture own fields.** 0x40 bytes, all zero on the one tag
    checked per map; whether they hold anything is unknown. Placeholder
    tags do not occur on the disc.
11. **Control own fields** are from the reference only (no disc tag).
12. **Seat yaw and pitch rates** (warthog 60 and 30, ghost 80, scorpion
    50) look like degrees per second while other angles are radians;
    vehicles.md should check how they are used.
13. **Cross-region builds.** The disc is the NTSC build. The reference's
    PAL build agrees on every layout checked; a PAL disc was not
    available.
