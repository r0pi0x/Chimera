# Halo: Combat Evolved: animation graphs and node animations

Source basis: the game's own map files from the original Xbox disc, read
with throwaway scripts (every `antr` tag of all 23 playable maps, with
`bloodgulch.map` measured in detail and `a10.map` for the compressed
form); and a decompilation of an Xbox build of the engine (the animation
graph definitions and sampler, the unit and biped animation logic, the
first-person weapon view, the weapon timers, the object pose builder),
used for behaviour. The decompilation describes a PAL build; the disc is
NTSC. Where they differ the disc wins and the difference is noted.
Status: draft

Sections: Summary; Units and conventions; Constants; The animation
graph tag; Index tables; One animation; Uncompressed frame data;
Compressed animation data; Sampling a frame; Matching animations to a
model; Building the pose; Playback; Blending; Aiming and looking
screens; Units; Bipeds; First person; The weapon's own animations;
Gameplay events driven by animations; Per-tick order; Measured facts;
Verification; Edge cases; Quirks; Test cases; Confidence per section;
Open questions.

---

## Summary

- An animation graph (`antr`, "model_animations") holds a flat list of
  animations plus index tables that name which animation plays in which
  situation: per unit *seat* (stand, crouch, vehicle seats), per
  *weapon class* inside the seat (rifle, pistol, ...), per *weapon type*
  inside the class (`ar`, `hp`, ...), and separate tables for weapons,
  vehicles, devices, damage reactions and first-person views.
- An animation is a sequence of frames, one per 30 Hz tick. Each frame
  gives every node a local rotation (quaternion stored as four 16-bit
  integers), translation (three reals, world units) and scale (one
  real). Nodes that do not move store their value once ("default
  data"). Rotations follow the same convention as the model's node
  rotations: a stored (i, j, k, w) is the usual rotation of its
  conjugate.
- Base animations set the pose; overlays add a rotation and a
  translation on top; replacement animations overwrite only the nodes
  they animate. Aiming and looking are 9-frame (or larger) overlay grids
  sampled bilinearly by yaw and pitch. Changing state blends from the
  last pose over a few ticks.
- There is no sub-frame interpolation in steady playback: the game
  samples integer frames, advances one frame per tick, and draws at the
  tick rate.
- The first-person arms and weapon are two models posed by one graph
  (the weapon's first-person graph) whose node list is the union of
  both models' nodes, matched by name; the graph's root sits at the
  camera.
- Several gameplay timers are animation lengths: a weapon's ready time
  is its first-person `ready` frame count, a reload is its
  `reload-empty` slot's frame count, a player's melee lasts its
  first-person `melee` frame count less a quarter, with the hit at the
  key frame; a unit's grenade leaves the hand at the key frame of its
  third-person `throw-grenade` animation.
- On the disc 186 animations (171 distinct) use the Xbox compressed
  form, all of them campaign cinematics; no multiplayer map holds any.

## Units and conventions

Bytes, tag blocks, tag references, data references, world units (WU),
axes (+X forward, +Y left, +Z up), angles (radians) and the 30 Hz tick
are as in [cache_file.md](cache_file.md). Node trees, the matrix4x3
type and the stored quaternion convention are as in
[models.md](models.md), "Nodes and the skeleton".

- **Quaternions** are written (i, j, k, w). The rotation matrix of a
  stored quaternion q is the one in models.md (columns forward, left,
  up, with s = 2 / |q|^2): the usual matrix of the conjugate
  (-i, -j, -k, w). This holds for animation data too (checked, see
  Verification). An implementation using the usual convention negates
  i, j and k of every quaternion it reads from an animation, exactly as
  for model nodes, and then must also reverse the order of quaternion
  products (see Blending).
- **Hamilton product** a * b of (i, j, k, w) quaternions:
  i = aw bi + ai bw + aj bk - ak bj;
  j = aw bj + aj bw + ak bi - ai bk;
  k = aw bk + ak bw + ai bj - aj bi;
  w = aw bw - ai bi - aj bj - ak bk.
- **nlerp(a, b, t)**: if a . b < 0 use -t in place of t for the b term
  (shortest path): r = (1 - t) a + t' b, then normalise r (if |r|^2 is 0
  the result is (0, 0, 0, 1)). The engine never uses spherical
  interpolation for animation.
- **lerp(a, b, t)** = (1 - t) a + t b for points and scalars.
- **A node's orientation** is the triple (rotation q, translation t,
  scale s). Its local matrix maps a point p of the node's frame to
  s R(q) p + t in the parent's frame.
- **Frame index** f counts from 0. An animation of N frames lasts N
  ticks.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| GRAPH_SIZE | 0x80 | bytes | main struct of an `antr` tag |
| ANIMATION_SIZE | 0xB4 | bytes | one animation element |
| GRAPH_NODE_SIZE | 0x40 | bytes | one graph node |
| QUAT16_SCALE | 1/32767 | | stored 16-bit component c decodes to c / 32767 |
| COMPRESSED_KEY_COUNT_BITS | 12 | bits | low bits of a compressed node header |
| MAX_FRAMES | 2048 | frames | per animation (editor limit) |
| MAX_NODES | 64 | | per animation; flag words hold 64 bits |
| MAX_ANIMATIONS | 256 | | per graph |
| MAX_SOUND_REFERENCES | 257 | | per graph |
| FRAMES_PER_TICK | 1 | | every animation advances one frame per tick |
| UNIT_TRANSITION_TICKS | 6 | ticks | default blend when a unit changes state |
| UNIT_TURN_TRANSITION_TICKS | 1 | ticks | between idle and turn states |
| UNIT_LAND_TRANSITION_TICKS | 2 | ticks | into a landing |
| UNIT_DAMAGE_TRANSITION_TICKS | 3 | ticks | into a hard ping or death |
| FP_TRANSITION_TICKS | 6 | ticks | default first-person blend |
| FP_FIRE_TRANSITION_TICKS | 3 | ticks | into fire and misfire |
| FP_OVERLAY_FRAMES | 9 | frames | minimum length of the first-person `overlays` animation |
| FP_MOVING_THROTTLE | 0.1 | | throttle magnitude above which the `moving` overlay runs |
| FP_PUSH_BACK_KICK | 0.05 | per tick | added to the push-back velocity per primary shot |
| FP_RELOAD_SETTLE_TICKS | 44 | ticks | needler ammunition display delay during a reload |
| IK_REACH | 0.98 | | fraction of arm length an IK target may reach |

## The animation graph tag

### Main struct (`antr`, 0x80 bytes)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 12 | block | objects | object overlays, 0x14-byte elements |
| 0x0C | 12 | block | units | seats, 0x64-byte elements |
| 0x18 | 12 | block | weapons | at most 1 element of 0x1C bytes |
| 0x24 | 12 | block | vehicles | at most 1 element of 0x74 bytes |
| 0x30 | 12 | block | devices | at most 1 element of 0x60 bytes |
| 0x3C | 12 | block | unit damage | index table, 2-byte elements |
| 0x48 | 12 | block | first person weapons | at most 1 element of 0x1C bytes |
| 0x54 | 12 | block | sound references | 0x14-byte elements |
| 0x60 | 4 | f32 | limp body node radius | WU; 0 on every multiplayer graph (grunt 0.02, hunter 0.05) |
| 0x64 | 2 | flags16 | flags | build-time options; 0, 1 or 3 on the disc; not read at run time |
| 0x66 | 2 | | padding | |
| 0x68 | 12 | block | nodes | GRAPH_NODE_SIZE elements |
| 0x74 | 12 | block | animations | ANIMATION_SIZE elements |

An *index table* is a block of 2-byte elements, each an index16 into
the animations block (NONE = no animation). Its count may be shorter
than the full enumeration of its slots: a slot at or beyond the count
means "none". The meaning of each slot is in Index tables.

### Object overlay (0x14 bytes)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 2 | index16 | animation | an overlay animation |
| 0x02 | 2 | enum16 | function | which of the object's four exported functions (A..D = 0..3) drives it |
| 0x04 | 2 | enum16 | mode | 0 frame, 1 scale |
| 0x06 | 14 | | unused | |

### Unit seat (0x64 bytes)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | tag string | label | seat label: `stand`, `crouch`, `alert`, a vehicle seat (`W-driver`) |
| 0x20 | 24 | screen bounds | looking bounds | for the `look` overlay (below) |
| 0x38 | 8 | | unused | |
| 0x40 | 12 | block | animations | index table, unit seat slots |
| 0x4C | 12 | block | IK points | 0x40-byte elements (vehicle seats: hands to wheel grips) |
| 0x58 | 12 | block | weapons | weapon classes, 0xBC-byte elements |

**Screen bounds** (24 bytes), used for aiming and looking:

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | angle | right yaw per frame | radians of yaw (toward negative yaw) between columns |
| 0x04 | 4 | angle | left yaw per frame | radians of yaw (toward positive yaw) between columns |
| 0x08 | 2 | i16 | right frame count | columns to the right of centre |
| 0x0A | 2 | i16 | left frame count | columns to the left of centre |
| 0x0C | 4 | angle | down pitch per frame | radians of pitch (toward negative pitch) between rows |
| 0x10 | 4 | angle | up pitch per frame | radians of pitch (toward positive pitch) between rows |
| 0x14 | 2 | i16 | down frame count | rows below centre |
| 0x16 | 2 | i16 | up frame count | rows above centre |

**IK point** (0x40 bytes): marker name on the unit (tag string, 0x00),
marker name on the other object (tag string, 0x20).

**Weapon class** (0xBC bytes):

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | tag string | label | `rifle`, `pistol`, `missile`, `support`, `unarmed`, `fixed` |
| 0x20 | 32 | tag string | grip marker | marker on the weapon (empty for hand-held classes on the disc) |
| 0x40 | 32 | tag string | hand marker | marker on the unit the weapon attaches to (`right hand`) |
| 0x60 | 24 | screen bounds | aiming bounds | for `aim-still` and `aim-move` |
| 0x78 | 32 | | unused | |
| 0x98 | 12 | block | animations | index table, weapon class slots |
| 0xA4 | 12 | block | IK points | 0x40 bytes each (left hand to the weapon's `left hand cyborg`) |
| 0xB0 | 12 | block | weapon types | 0x3C-byte elements |

**Weapon type** (0x3C bytes): label (tag string, 0x00; matched against
the weapon tag's label, e.g. `ar`, `hp`; empty for unarmed), 16 unused
bytes (0x20), animations index table (block at 0x30, weapon type
slots).

### Weapons (0x1C bytes)

16 unused bytes, then the animations index table (block at 0x10,
weapon slots). Used by a weapon's own third-person graph.

### Vehicles (0x74 bytes)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 24 | screen bounds | steering bounds | for the vehicle's aiming screen |
| 0x18 | 68 | | unused | |
| 0x5C | 12 | block | animations | index table, vehicle slots |
| 0x68 | 12 | block | suspension | 0x14-byte elements |

Suspension element (0x14 bytes): mass point index (i16, 0x00),
animation (index16, 0x02), full extension ground depth (f32, 0x04),
full compression ground depth (f32, 0x08), 8 unused bytes. Owned by
`vehicles.md`.

### Devices (0x60 bytes)

84 unused bytes, then the animations index table (block at 0x54,
device slots).

### First person weapons (0x1C bytes)

16 unused bytes, then the animations index table (block at 0x10,
first-person slots).

### Sound reference (0x14 bytes)

A tag reference to a `snd!` (0x00) and 4 unused bytes. An animation's
sound index selects one of these (see One animation).

### Node (0x40 bytes)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | tag string | name | `bip01 pelvis`, `frame gun` |
| 0x20 | 2 | index16 | next sibling | NONE if last |
| 0x22 | 2 | index16 | first child | NONE if a leaf |
| 0x24 | 2 | index16 | parent | **0** for node 0 on every disc graph (models store NONE) |
| 0x26 | 2 | | padding | |
| 0x28 | 4 | flags32 | flags | bit 0 ball-socket, bit 1 hinge, bit 2 no movement |
| 0x2C | 12 | real_vector3d | base vector | a unit vector (joint axis); 0 when no movement |
| 0x38 | 4 | f32 | vector range | radians |
| 0x3C | 4 | | unused | |

The joint fields are exporter data; no run-time code reads them. On
the disc every node's parent index is below its own (node 0 apart) and
the sibling and child links agree with the parents. A graph's node
block may be empty (the Warthog, Ghost and plant graphs): then only a
model supplies the node tree.

## Index tables

The slot names are data the tags expose; the type in brackets is the
type the editing tools expect in that slot (b base, o overlay, r
replacement, - any).

**Unit seat** (30 slots): 0 airborne-dead (b), 1 landing-dead (b),
2 acc-front-back (o), 3 acc-left-right (o), 4 acc-up-down (o), 5 push
(o), 6 twist (o), 7 enter (b), 8 exit (b), 9 look (o), 10 talk (o),
11 emotions (-), 12 unused, 13..22 user0..user9 (-), 23 flying-front
(b), 24 flying-back (b), 25 flying-left (b), 26 flying-right (b),
27 opening (b), 28 closing (b), 29 hovering (b).

**Weapon class** (55 slots): 0 idle, 1 gesture, 2 turn-left,
3 turn-right, 4 dive-front, 5 dive-back, 6 dive-left, 7 dive-right,
8 move-front, 9 move-back, 10 move-left, 11 move-right, 12 slide-front,
13 slide-back, 14 slide-left, 15 slide-right, 16 airborne,
17 land-soft, 18 land-hard, 19 unused (o), 20 throw-grenade,
21 disarm (r), 22 drop (r), 23 ready (r), 24 put-away (r),
25 aim-still (o), 26 aim-move (o), 27 surprise-front,
28 surprise-back, 29 berserk, 30 evade-left, 31 evade-right,
32 signal-move, 33 signal-attack, 34 warn, 35 stunned-front,
36 stunned-back, 37 stunned-left, 38 stunned-right, 39 melee,
40 celebrate, 41 panic, 42 melee-airborne, 43 flaming,
44 resurrect-front, 45 resurrect-back, 46 melee-continuous,
47 feeding, 48 leap-start, 49 leap-airborne, 50 leap-melee,
51 zapping, 52..54 unused. All base unless marked.

**Weapon type** (10 slots): 0 reload-1 (r), 1 reload-2 (r),
2 chamber-1 (o), 3 chamber-2 (o), 4 fire-1 (o), 5 fire-2 (o),
6 charged-1 (o), 7 charged-2 (o), 8 melee (o in the tools; the
Spartan's are replacement animations), 9 overheat (o).

**Weapon** (11 slots, a weapon's own graph): 0 idle, 1 ready,
2 put-away, 3 reload-1, 4 reload-2, 5 chamber-1, 6 chamber-2,
7 charged-1, 8 charged-2, 9 fire-1, 10 fire-2.

**Vehicle** (8 slots, all overlays): 0 steering, 1 roll, 2 throttle,
3 velocity, 4 braking, 5 ground-speed, 6 occupied, 7 unoccupied.

**Device** (2 slots, overlays): 0 position, 1 power.

**First person** (28 slots): 0 idle (b), 1 posing (b), 2 fire-1 (b),
3 moving (o), 4 overlays (o), 5 light-off (b), 6 light-on (b),
7 reload-empty (b), 8 reload-full (b), 9 overheated (b), 10 ready (b),
11 put-away (b), 12 overcharged (b), 13 melee (b), 14 fire-2 (b),
15 overcharged-jitter (o), 16 throw-grenade (b), 17 ammunition (o),
18 misfire-1 (b), 19 misfire-2 (b), 20 throw-overheated (b),
21 overheating (b), 22 overheating-again (b), 23 enter (b),
24 exit-empty (b), 25 exit-full (b), 26 o-h-exit (b),
27 o-h-s-enter (b). Slots 23..25 are the shotgun's one-shell reload
(enter, exit when the tube was empty, exit when it was not); 26 and 27
are the overheat exit and the overheat with supercharge.

**Unit damage** (176 slots): slot = (type x 4 + direction) x 11 +
part, with type 0 s-ping, 1 h-ping, 2 s-kill, 3 h-kill; direction
0 front, 1 left, 2 right, 3 back; part 0 gut, 1 chest, 2 head,
3 l-arm, 4 l-hand, 5 l-leg, 6 l-foot, 7 r-arm, 8 r-hand, 9 r-leg,
10 r-foot. "s" is soft and "h" hard; "ping" is a flinch, "kill" a
death.

**Missing slots are pre-filled.** The build tool fills a slot the
artist left empty with a related animation (the Spartan's crouching
pistol `airborne` slot holds `stand unarmed airborne`; the assault
rifle's first-person `light-on` holds `first-person light-off`, its
`reload-empty` holds `first-person reload-full`). A slot that is still
NONE on the disc really has no animation; the run time does no
name-based fallback of its own.

## One animation

Animation element, 0xB4 bytes:

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | tag string | name | e.g. `stand rifle move-front`; permutations end in `%1`, `%2` |
| 0x20 | 2 | enum16 | type | 0 base, 1 overlay, 2 replacement |
| 0x22 | 2 | i16 | frame count | N, at least 1 on the disc (27 animations have 1) |
| 0x24 | 2 | i16 | frame size | bytes per uncompressed frame (below) |
| 0x26 | 2 | enum16 | frame info type | 0 none, 1 dx dy, 2 dx dy dyaw, 3 dx dy dz dyaw |
| 0x28 | 4 | u32 | node list checksum | equals the model's node list checksum (models.md); 0 = do not check |
| 0x2C | 2 | i16 | node count | nodes this animation poses |
| 0x2E | 2 | i16 | loop frame | 0 = play once; > 0 = loop back to this frame |
| 0x30 | 4 | f32 | weight | permutation weight (editor value; 0 on the disc mostly) |
| 0x34 | 2 | i16 | key frame | frame of the animation's event (0 = none) |
| 0x36 | 2 | i16 | second key frame | a second event frame (0 = none) |
| 0x38 | 2 | index16 | next animation | next permutation of the same slot, or NONE |
| 0x3A | 2 | flags16 | flags | bit 0 compressed data, bit 1 world relative, bit 2 25 Hz (PAL) |
| 0x3C | 2 | index16 | sound | into sound references, or NONE |
| 0x3E | 2 | i16 | sound frame | frame at which the sound starts |
| 0x40 | 1 | u8 | left foot frame | footstep frame of the left foot (0 = none) |
| 0x41 | 1 | u8 | right foot frame | footstep frame of the right foot (0 = none) |
| 0x42 | 2 | index16 | first permutation | build-time: index of the first animation of this permutation chain (the animation's own index when it has none) |
| 0x44 | 4 | f32 | cumulative weight | build-time: this permutation's share of the chain, cumulative, 1.0 for the last (and for a lone animation) |
| 0x48 | 20 | data reference | frame info | N records of 8, 12 or 16 bytes (below), in tag data |
| 0x5C | 8 | 2 x u32 | translation flags | bit n set: node n has per-frame translation |
| 0x64 | 8 | | unused | zero |
| 0x6C | 8 | 2 x u32 | rotation flags | bit n set: node n has per-frame rotation |
| 0x74 | 8 | | unused | zero |
| 0x7C | 8 | 2 x u32 | scale flags | bit n set: node n has per-frame scale |
| 0x84 | 4 | | unused | zero |
| 0x88 | 4 | i32 | compressed data offset | offset of the compressed header inside the frame data; 0 on the disc |
| 0x8C | 20 | data reference | default data | values of the nodes without per-frame data (uncompressed only) |
| 0xA0 | 20 | data reference | frame data | the frames, or the compressed stream |

- **Flag words.** Node n's bit is bit (n mod 32) of word n div 32; the
  first u32 covers nodes 0..31. Only bits below the node count mean
  anything: three Spartan and Warthog animations on bloodgulch have
  stray bits set above it, and a reader masks them off.
- **Frame size** = 8 R + 12 T + 4 S, where R, T, S are the numbers of
  nodes (below the node count) with the rotation, translation and scale
  bit. The stored value equals this on every disc animation, compressed
  ones included.
- **Default data size** = 8 (n - R) + 12 (n - T) + 4 (n - S) for an
  uncompressed animation of n nodes (checked on every disc animation);
  0 for a compressed one.
- **Frame data size** = N x frame size for an uncompressed animation
  (checked on every one); the compressed stream's own size otherwise.
- **Frame info size** = N x (0, 8, 12, 16) by type (checked on every
  one).
- **Frame info records** (all f32, per frame, in the unit's own frame:
  x forward, y left): type 1 (dx, dy) in WU per tick; type 2 (dx, dy,
  dyaw); type 3 (dx, dy, dz, dyaw); dyaw in radians per tick, positive
  turning left (counter-clockwise from above). Record f is the motion
  for the tick spent on frame f. See Bipeds for who uses it.
- **Key frames.** A key frame of 0 never fires (see Playback), so 0
  means none. On the disc only 11 animations use the second key frame.
- **Sound.** When the animation advances off its sound frame the
  referenced sound is started on the object (see Playback).
- **Foot frames** are bytes and may exceed the frame count (the
  Spartan's crouching turns have 14 on 7-frame animations); such a
  value never matches and makes no sound.
- **Permutations.** A slot names the first animation of a chain linked
  by next animation. When the slot is played the engine draws one
  uniform random number r in [0, 1) and walks the chain from its first
  animation, taking the first whose cumulative weight >= r. Only the
  stored cumulative weight is used; the editor weight is not. (Spartan:
  `alert unarmed idle` with `%1` and `%2` at 1/3, 2/3, 1.) Game-state
  animations draw from the game's synchronised random sequence; the
  first-person view draws from the local one.
- **World relative** (23 cinematic animations): the root node's
  transform is used as a world transform, not composed with the
  object's position and facing.
- **25 Hz**: never set on the disc (the NTSC build). A PAL build's
  maps would need its frames stretched; out of scope.
- **Compressed**: set on 186 animations of the disc (a10 66, a30 10,
  a50 18, b40 22, c40 28, d20 5, d40 37; 171 distinct graph and name
  pairs), all in `cinematics\animations\...` graphs. None in any
  multiplayer map, `ui.map` has no graphs. When set, the frame data is
  the compressed stream, the compressed data offset is 0 and the
  default data is empty.

## Uncompressed frame data

Frame f starts at byte f x frame size of the frame data. Within a
frame, and within the default data, nodes come in node order 0, 1, ...,
node count - 1, and for each node the parts it has, in this order:

1. rotation, 8 bytes: four i16 (i, j, k, w); each component c becomes
   c / 32767. The result is close to unit length (the encoder truncated
   toward zero) and is used as it is: the rotation matrix divides by
   |q|^2, and blends normalise.
2. translation, 12 bytes: three f32 (x, y, z), WU, in the parent
   node's frame (for node 0, in the object's frame).
3. scale, 4 bytes: f32 (1.0 on the disc wherever stored by default).

A node whose rotation bit is set has its rotation in the frame;
otherwise its rotation is the next rotation in the default data. Same
for translation and scale, each part independently. The default data
is therefore the same walk over the nodes, taking only the parts whose
bit is clear. Example, a node with only the rotation bit set: 8 bytes
in each frame; 12 + 4 bytes (translation, scale) in the default data.

The default data is the value for the whole animation, **not** the
model's bind pose: the Spartan's animations store default rotations up
to 1.32 apart (in component terms) from the model's node rotations.
Translation lengths do match the model's distance from parent (within
5e-5 WU on every frame of every Spartan base animation).

## Compressed animation data

The stream starts at the compressed data offset (0) of the frame data.
All offsets below are bytes from the stream's start; all values are
little-endian.

**Header** (0x2C bytes, then the rotation node headers):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 4 | i32 | rotation key frame times offset |
| 0x04 | 4 | i32 | default rotations offset |
| 0x08 | 4 | i32 | rotation keys offset |
| 0x0C | 4 | i32 | translation node headers offset |
| 0x10 | 4 | i32 | translation key frame times offset |
| 0x14 | 4 | i32 | default translations offset |
| 0x18 | 4 | i32 | translation keys offset |
| 0x1C | 4 | i32 | scale node headers offset |
| 0x20 | 4 | i32 | scale key frame times offset |
| 0x24 | 4 | i32 | default scales offset |
| 0x28 | 4 | i32 | scale keys offset |
| 0x2C | 4 x R | u32 | rotation node headers |

On the disc the sections follow each other in exactly this order with
no gaps: rotation node headers (R x 4), rotation key times (K_r x 2),
default rotations (node count x 6), rotation keys (K_r x 6),
translation node headers (T x 4), translation key times (K_t x 2),
default translations (node count x 12), translation keys (K_t x 12),
then the scale sections (empty on every disc animation: no compressed
animation has a scale bit).

- **Node header** (u32): first key = header >> 12, key count = header
  and 0xFFF. Headers are in order of the animated nodes: the m-th
  header of a kind belongs to the m-th node (in node order) whose bit of
  that kind is set.
- **Key frame times**: u16 frame numbers, one per key; a node's times
  are entries first .. first + count - 1. On the disc each node's times
  increase strictly, the first is at least 1 and the last is N - 1
  (checked on all keys of the decoded animations).
- **Defaults**: rotations and translations have one entry per node of
  the animation (indexed by node number, animated or not); scales are
  indexed by the scale node's rank (the m-th scaled node), not by node
  number (reference; no disc data). For an animated node the default is
  its value at frame 0, which is never a key.
- **Rotation, 6 bytes, 12 bits per component.** Read three u16 words
  w0, w1, w2 (little-endian) and form the 48-bit number
  X = w0 x 2^32 + w1 x 2^16 + w2. The components are the 12-bit fields
  i = bits 47..36, j = bits 35..24, k = bits 23..12, w = bits 11..0.
  Each 12-bit field c widens to 16 bits by repeating its top four bits
  below it: c16 = (c << 4) or (c >> 8), read as a signed i16; the
  component is c16 / 32767. No component is dropped and there is no
  separate sign: all four are stored. Every use of a compressed
  rotation normalises it afterwards.
- **Translation**: three f32 per key and per default. **Scale**: one
  f32.

Unpacking example (disc): the first default rotation of `crew_panic1`
(a10, `cinematics\animations\crewman\x10\x10`) is the bytes
D3 44 3F 38 64 A4: words 0x44D3, 0x383F, 0xA464; fields 0x44D, 0x338,
0x3FA, 0x464; widened 0x44D4, 0x3383, 0x3FA3, 0x4644 = 17620, 13187,
16291, 17988; components 0.537736, 0.402448, 0.497177, 0.548967
(|q| = 0.99984); normalised (0.537824, 0.402513, 0.497258, 0.549056).

## Sampling a frame

Sampling animation A at integer frame f (0 <= f < N) gives each node
n < node count a (q, t, s):

**Uncompressed:** as in Uncompressed frame data.

**Compressed:** for each kind separately:

- node without the kind's bit: rotation = normalise(default rotation
  of n); translation = default translation of n; scale = 1.0.
- node with the bit and key count 0: the default (normalised for
  rotations).
- otherwise, with key times k_0 < k_1 < ... < k_last:
  - f > k_last (cannot happen on the disc): the last key.
  - f < k_0: interpolate from (frame 0, default) to (k_0, key 0).
  - f = k_last: interpolate from (k_last, last key) to (k_last + 1,
    default); since f equals the start, this is the last key.
  - otherwise find the key m with k_m <= f < k_(m+1) (binary search)
    and interpolate from (k_m, key m) to (k_(m+1), key m+1).
  - "Interpolate from (a, A) to (b, B)": if f = a the value is A
    (normalised for rotations); else u = (f - a) / (b - a) and the
    value is nlerp(A, B, u) for rotations, lerp for translations and
    scales.

The same rule serves a fractional frame f in the one place the engine
asks for one (the continuous overlay, Blending); the comparisons then
use floor(f) to find the keys and f itself for u.

**Between frames during playback the engine does not interpolate.**
Base, replacement and plain overlay animations are always sampled at
the integer current frame. Fractional sampling exists only for the
continuous overlays below.

## Matching animations to a model

- A unit's, vehicle's or weapon's graph poses the object's model by
  node index: animation node n is model node n. The Spartan's graph
  node names equal its model's in order (checked).
- A base animation poses the object only if it is a base animation,
  its node count equals the model's node count, and its checksum is 0,
  or the model's checksum is 0, or the two are equal. Otherwise the
  object takes its model's bind pose for that tick (each node's default
  rotation and translation, scale 1). Every Spartan animation carries
  0x6CFA6498, the model's value (checked).
- Overlays and replacements are applied whenever their type matches;
  the engine does not check their checksum. An animation with more
  nodes than the model is skipped (none on the disc).
- First-person graphs carry a checksum matching neither the arms nor
  the weapon model (assault rifle graph 0x9EEC079E, arms 0x03350AD2,
  weapon 0x41F353DE); the first-person view never checks it (no model
  is passed). See First person for the name matching it uses instead.

## Building the pose

From node orientations to node matrices:

- Local matrix L(n) of (q, t, s): rotation columns from q as in
  models.md, translation t, scale factor s.
- Root: W(0) = O x L(0), where O is the object's frame (position;
  forward; left = up x forward; up; mirrored objects negate left) and
  the product composes rigid transforms. A world-relative animation
  uses W(0) = L(0).
- Children: W(n) = W(parent) x L(n), where the product of matrices
  with scales (a, sa) and (b, sb) has rotation Ra Rb, position
  pa + sa Ra pb and scale sa sb.
- The engine walks the node tree breadth-first through the child and
  sibling links; because every parent index is below its child's on the
  disc, composing in index order gives the same result.
- Units use the model's node tree. The first-person view uses the
  graph's node tree (its parent indices; node 0's stored parent 0 is
  ignored).
- An object's scale (when positive) multiplies node 0's scale and
  translation before the matrices are built.

## Playback

Each playing animation has a state (animation index, current frame f).
Starting it sets f = 0 (the first permutation chosen as above). Once a
tick the owner advances it, in this order:

1. **Sound**: if the animation has a sound and f equals its sound
   frame, the sound starts now (before advancing). So a sound at frame
   0 starts on the first advance, one tick after the animation began.
2. f = f + 1.
3. If f >= N:
   - loop frame L > 0: f = min(L, N - 1); result *looped*. A loop frame
     at or past the end therefore holds the last frame for ever (113
     disc animations have L >= N, e.g. the plasma rifle's first-person
     `overheated`, 50 frames, loop 58).
   - L = 0: a new permutation of the same slot is chosen (from the
     chain's first animation), f = 0; result *restarted*.
4. Else if f + 1 = N and L = 0: result *ends next tick*.
5. Else if f equals the key frame or the second key frame: result
   *key frame*.
6. Else *running*.

Consequences: the results are exclusive and checked in that order, so
a key frame on the last frame of a play-once animation is reported as
"ends next tick" instead and never fires; a key frame of 0 never fires.
An owner that wants a play-once animation to stop reacts to "ends next
tick" (the frame then shown is N - 1, the last), so a play-once
animation occupies exactly N ticks: frames 0 .. N - 1.

The current frame is what the pose uses until the next advance. A unit
switching to a new state in the same tick shows frame 0 of the new
animation that tick.

## Blending

All blends work on node orientations (q, t, s) before matrices are
built. "Stored" means the quaternions as stored (the game's
convention).

- **Base**: sets every node (q, t, s) from the sample.
- **Replacement**: for each node, sets only the parts whose bit is set
  (rotation, translation, scale independently); everything else keeps
  the value beneath. Uncompressed parts come from the frame; nodes
  without a bit are left alone (the default data is not read).
- **Overlay**: for each node and each part whose bit is set, with the
  sampled (q_o, t_o, s_o): q = q_o * q (stored Hamilton product, overlay
  on the left), t = t + t_o, s = s x s_o. Parts without a bit are
  untouched. In the usual (conjugated) convention this is
  q_usual = q_usual * q_o_usual, the overlay on the right: the overlay's
  rotation acts in the node's own frame after the base. (Checked: with
  this order the Spartan's aim grid turns the torso and both hands
  together by +-90 degrees of yaw; with the other order the hands
  diverge.)
- **Scaled overlay** with weight w in [0, 1]: q_o' = (1 - w) e + w' q_o
  with e = (0, 0, 0, 1) and the same sign rule as nlerp, **not**
  normalised; then q = q_o' * q; t = t + w t_o; s = s x (w s_o + 1 - w).
- **Continuous overlay** at fractional frame x (0 <= x <= N): frame
  a = floor(x), fraction u = x - a; if a >= N then a = N - 1 and u = 1.
  The next frame is a + 1, or 0 when a = N - 1 (it wraps). Uncompressed:
  q_o = nlerp(q_a, q_next, u), t_o = lerp, s_o = lerp, then applied as
  an overlay. Compressed: the rotation is sampled at the integer frame
  a (the fraction is ignored for rotations, a quirk), translation and
  scale at x.
- **Continuous scaled overlay**: the continuous sample, then the
  scaled rule, except that here the weighted rotation is normalised.
- **Transitions** (units and the first-person view): when a state
  changes, the engine copies the current final pose as the "from" pose
  and starts a blend of n ticks with index i = 0 (unless a blend
  already running has more ticks left than n, which it then keeps).
  While a blend runs, after all other layers, each node becomes
  q = nlerp(q_from, q, u), t = lerp(t_from, t, u), s = lerp(s_from, s,
  u), with u = (i + 1) / n. The index rises by 1 each tick; at i = n
  the blend ends. So the first tick of an n-tick blend shows 1/n of
  the new pose and the n-th tick shows all of it. The blend is
  computed after aiming, overlays and replacements, so it smooths those
  too.

## Aiming and looking screens

An *aiming screen* is an overlay animation whose frames form a grid:
W = right count + left count + 1 columns, H = down count + up count +
1 rows, frame = column + row x W. Column 0 is the furthest right
(most negative yaw), row 0 the furthest down (most negative pitch);
the centre frame (right count, down count) is the neutral pose. The
animation must be an overlay with at least W x H frames, else it is
skipped.

Given yaw and pitch (radians, relative, below) and screen bounds:

1. For yaw: d = left yaw per frame if yaw >= 0, else right yaw per
   frame. v = yaw / d (0 if d = 0). c = v truncated toward zero; u = v
   - c (fractional part with the sign of v); if u < 0 then u = u + 1,
   c = c - 1. (So c = floor(v), u = v - floor(v).)
2. Clamp: if c >= left count then c = left count - 1, u = 1. If
   c < -right count then c = -right count, u = 0.
3. Column = c + right count. The same for pitch with down/up values
   gives row and fraction p.
4. If row or column falls outside the grid (possible only with zero
   counts), nothing is applied.
5. Next column c1 = column + 1, or column if that is the last; next
   row r1 likewise. Frames f00 = (column, row), f10 = (c1, row),
   f01 = (column, r1), f11 = (c1, r1).
6. For each node with the rotation bit: y0 = nlerp(q00, q10, u),
   y1 = nlerp(q01, q11, u), q_o = nlerp(y0, y1, p); then q = q_o * q.
   For each node with the translation bit: t = t + (1 - u)(1 - p) t00
   + u (1 - p) t10 + (1 - u) p t01 + u p t11. Scale is ignored.

**Which angles.** Aiming: the unit's aiming vector expressed in the
unit's own frame (forward, left, up); yaw = atan2(y, x), pitch =
atan2(z, sqrt(x^2 + y^2)). Looking: the unit's looking vector in the
same frame, minus the aiming angles (yaw_look - yaw_aim, pitch_look -
pitch_aim). The aiming screen uses the current weapon class's aiming
bounds and its `aim-still` or `aim-move` animation; the looking screen
uses the seat's looking bounds and its `look` animation. The aiming
screen is applied first, then the looking screen. The looking screen
needs a current weapon or a player.

**Which aim animation.** `aim-still` in idle, turn, flying, airborne,
land-soft, land-hard, opening and closing states; `aim-move` in the
move, stunned-move and slide states; none in other states. Neither
screen is applied while a unit plays an action (reload, ready,
put-away, melee, throw, disarm, drop), nor in the hard ping, dying,
seat entry and exit, user animation, AI impulse, melee, throw grenade,
resurrect and leap-melee states, nor for units flagged as having no
aiming or as simple creatures.

The resulting bounds (right x right per frame etc.) are also what the
unit's aim is clipped to (the unit spec owns that).

## Units

### Seat, weapon class and weapon type

A unit's current animation set is a (seat, weapon class, weapon type)
triple of element indices in its graph. It is chosen by a seat label
and a weapon label:

- seat label: for an unseated unit `stand`, or `crouch` when crouching
  (players: crouch control held or unable to stand); `alert`,
  `asleep`, `flee`, `flaming` for AI moods; a vehicle seat's own label
  when seated; `stand` again at death.
- weapon label: the held weapon's label (a 32-byte string in the
  weapon tag, at offset 0x30C of the `weap` main struct on the disc:
  `ar` assault rifle, `hp` pistol, `sg` shotgun, `pr` plasma rifle,
  `rl` rocket launcher, `sr` sniper rifle, `pp` plasma pistol, `ft`
  flamethrower, `ne` needler, `b` ball, `f` flag; vehicle guns `cg`,
  `cannon`, `gun`), or `unarmed` with no weapon.

The engine scans every seat whose label equals the seat label
(case-insensitive), every weapon class of it, and every weapon type of
the class; a type matches if its label equals the weapon label
(case-insensitive), or the weapon label is `unarmed` and the type's
label is empty. **Every match overwrites the previous one, so the last
match in block order wins.** (The Spartan's `stand` seat has an `ne`
type under both `rifle` and `pistol`; the pistol class, later, wins.)
No match leaves the triple unchanged. The weapon class is thus found
through its types; class labels are never compared. Which labels the
campaign characters' graphs carry, and so which weapons each can
hold, is in `characters.md`, section 2.

On a change the unit's animation state is reset so its current state
re-resolves, and a flag records whether the seat has any of the three
acceleration overlays (slots 2..4).

### State to animation

Each unit state reads one slot:

| States | Table and slot |
|---|---|
| idle, gesture, turn-left, turn-right | class 0, 1, 2, 3 |
| move front/back/left/right | class 8..11 |
| stunned move front/back/left/right | class 35..38 (stunned-*) |
| slide front/back/left/right | class 12..15 |
| airborne, land-soft, land-hard | class 16, 17, 18 |
| throw grenade | class 20 |
| melee, melee-airborne, melee-continuous | class 39, 42, 46 |
| resurrect front/back, feeding | class 44, 45, 47 |
| leap start/airborne/melee | class 48, 49, 50 |
| flying front/back/left/right | seat 23..26 |
| dying airborne | seat 0 (airborne-dead) |
| opening, closing, hovering | seat 27, 28, 29 |

The dying (on ground) and hard ping states take their animation from
the unit damage table instead (Units, Damage and death).

If the slot is empty, the state is still entered except for melee,
melee-airborne, melee-continuous, throw grenade, leap-start and
leap-melee, which are refused. Entering it with an empty slot clears
the unit's base animation (the old one does not keep playing): the
base pose becomes the model's default node pose, reached through the
usual transition blend, there is no root motion and nothing advances
until the next state change; the overlays (aiming screen and the rest)
still apply on top. What this means for a moving biped is in
`characters.md`, section 4 ("A missing animation").

**Interruptions.** A new desired state replaces the current one when
it differs, except: AI impulse, melee, melee-airborne, throw grenade,
resurrect and leap states yield only to a hard ping; dying states yield
only to the other dying state; turn-left, turn-right, opening and
closing do not yield to idle; hard ping, seat entry and exit and user
animations yield to nothing. Gesture, turns, hard ping, seat entry and
exit, user animations, AI impulse, melee, melee-airborne, throw
grenade, resurrect and leap-start/leap-melee are play-once: when their
animation reports "ends next tick" the unit re-evaluates its desired
state even if it is unchanged. All other states loop through their
animation (restarting at its end).

**Transitions.** Entering a state starts a 6-tick blend, except
between idle and turn states (1 tick) and into land-soft or land-hard
(2 ticks). A change of aiming screen (still versus move) also starts a
6-tick blend.

### Actions and overlay actions

Weapon events start extra animations on the unit, layered on the base
state:

| Event (from the weapon or the player) | Layer | Animation |
|---|---|---|
| primary / secondary reload | action | weapon type 0 reload-1 / 1 reload-2 |
| player melee | action | weapon type 8 melee, no blend |
| readying, putting away | action | class 23 ready / 24 put-away |
| disarm, drop | action | class 21 / 22 |
| overheat | action | weapon type 9 |
| primary / secondary shot | overlay action | weapon type 4 fire-1 / 5 fire-2 |
| primary / secondary chamber | overlay action | weapon type 2 / 3 |
| primary / secondary charged | overlay action | weapon type 6 / 7 |

- An *action* is applied as a replacement after the base, starting
  with a 6-tick blend (0 for melee). It plays once; when it reports
  "ends next tick" it is removed and a 6-tick blend back starts. An
  empty slot starts nothing.
- An *overlay action* is applied as an overlay after the action. A new
  one replaces the current one only if its rank is at least the current
  rank (rank: primary shot 1, secondary shot 2, primary charged 3,
  secondary charged 4, primary chamber 5, secondary chamber 6). None is
  started in the busy states (hard ping, dying, seat entry/exit, AI
  impulse, melee, throw, resurrect, leap). It plays once and is removed
  at its end.

### Damage and death

- A soft ping (a flinch that is not a kill) plays the s-ping slot for
  the hit's direction and body part (a random permutation) as an extra
  overlay layer at full weight (after the overlay action), unless one
  is playing and its frame is at most the unit's soft ping interrupt
  ticks. It starts at frame 0, advances one frame per unit update and
  is removed by the update in which it reaches its last frame; it is
  not a state, so it plays over any state (moving, airborne, seated,
  reloading) and starts no transition blend. Hand IK of the weapon is
  off while it plays. The Spartan's 44 s-ping slots all hold `s-ping
  front gut` (9 frames, three permutations), so direction and part do
  not show (`health_shields.md`, Pings).
- A hard ping (not a kill) sets the hard ping state with the h-ping
  slot's animation (3-tick blend), only when not seated and the slot
  holds one; the Spartan has none, so it never happens to players.
- A kill: if the unit is a biped whose airborne flag is set (and its
  biped tag allows a dying-airborne state) the state becomes dying
  airborne with the seat's airborne-dead animation (the usual 6-tick
  state blend); otherwise state dying with the h-kill (hard) or s-kill
  slot for the direction and part, blended in over 3 ticks. The seat is
  first re-resolved as `stand` with the current weapon. A grenade
  being thrown is released early.
- Each tick a dead biped wants dying airborne after 3 or more airborne
  updates and dying otherwise; the change dying airborne to dying plays
  the seat's landing-dead animation (6-tick blend), the change dying to
  dying airborne the airborne-dead animation (6-tick blend). Dying
  states yield only to each other. `health_shields.md`, The body, has
  the timing (landing-dead starts the tick after touchdown).
- A dying (ground) animation reaching its end holds its last frame (the
  frame steps back by one each tick), its root translation is ignored
  from then on, and a biped with the limp body flag then starts its
  limp body on the first such tick on which it is at rest; from then
  the node matrices are no longer rebuilt from the animation (the limp
  body moves them for 20 updates, then the pose stays frozen). The
  airborne-dead animation has loop frame 0 and simply repeats.
- The weapon is dropped after a random delay drawn from the integers
  floor(N/4) .. floor(N/4) + floor(N/2) − 1 (N the death animation's
  frame count), at least 1; at once for an airborne death or a killing
  push of 2 WU/s or more (`health_shields.md` owns death).

### Other layers

Seat emotions (frame = emotion index), mouth (seat slot 10, scaled
overlay of frame 0 by the mouth aperture), acceleration (seat slots
2..4 as continuous overlays at frame (N - 1) x the seat acceleration
component) and object overlays (the graph's objects block: frame mode
plays the overlay continuously at frame N x value for an additive
function, (N - 1) x value otherwise; scale mode plays frame
(object tick count mod N) scaled by the value) belong to cinematics and
vehicles; their order is in Per-tick order.

### Inverse kinematics

After the node matrices are built, a unit pins hands to markers:

- seated in a vehicle (not in hard ping, dying, seat entry or exit, AI
  impulse or resurrect states): each IK point of the seat;
- holding a weapon (no soft ping playing, no action, and not in
  flying, hard ping, dying, seat entry/exit, user animation, AI
  impulse, any melee, throw grenade, resurrect or leap states): each IK
  point of the weapon class (the Spartan: its `left hand` marker to the
  weapon's `left hand cyborg` marker).

For one IK point: the unit's marker sits on a hand node H with parent
E (elbow) and grandparent S (shoulder). The target hand matrix is the
other object's marker matrix times the inverse of the unit marker's
offset from H. With L1 = |E - S|, L2 = |H - E| (current positions),
D = |target - S|, direction a = (target - S) / D, bend normal
n = normalise(a x (E - S)), bend direction b = n x a: if D > 0.98
(L1 + L2) the target is pulled to S + 0.98 (L1 + L2) a. Then
x = (D^2 + L1^2 - L2^2) / (2 D), h = sqrt(L1^2 - x^2). The shoulder's
forward axis becomes normalise(x a + h b), its up normalise(forward x
old left), its left up x forward; the elbow moves to S + L1 forward;
the elbow's forward becomes normalise((D - x) a - h b), up and left
rebuilt the same way; the hand takes the target matrix. Only these
three matrices change. (Reference only.)

## Bipeds

**Movement state** (each tick, for a living, grounded, unseated biped
with nonzero throttle): base = stunned-move when body stun > 0.2 or
recent damage exceeds the unit's stun threshold, else move. If
|throttle forward| >= |throttle left|: front when throttle forward >= 0
else back; otherwise left when throttle left >= 0 else right. Zero
throttle gives idle. A throttle below 0.1 in magnitude counts as zero.

**Playback rate.** Movement animations play at one frame per tick
whatever the speed: there is no rate scaling. The Spartan's run cycle
is 20 frames (0.67 s) at any throttle.

**Turning in place.** A standing, non-moving biped whose facing
differs from its desired facing by more than its stationary turning
threshold enters turn-left or turn-right; the turn animation's frame
info rotates the body (the Spartan's are 7 frames of 0.2244 rad, 90
degrees in all). A biped that is moving turns at its moving turning
speed instead, without animation.

**Airborne and landing.** After more than 3 ticks in the air the
desired state is airborne (the Spartan's loops from frame 40). On
landing, a landing type (soft or hard) and a recovery time are set by
the biped tag from the impact speed; until the recovery count reaches
it the desired state is land-soft or land-hard (`movement.md` owns the
numbers). The landing animation's length does not decide anything; it
loops or holds while the recovery lasts.

**Root motion (frame info).** For bipeds that do not use player
physics, and for any biped in a user (scripted) animation, the frame's
dx and dy become its desired velocity in its own frame (WU per tick)
and dz is used by type 3. For player bipeds (the Spartan uses player
physics) dx, dy and dz are ignored: movement comes from the throttle.
**dyaw is applied for every biped**: the body's forward vector is
rotated about its up by dyaw each tick; this is what turns a player
standing still. Translation is ignored after a death animation's end.
A unit exiting a vehicle seat leaves with the root node's displacement
between the exit animation's last two frames added to its velocity.

**Footsteps.** In a move state with throttle magnitude above 0.5, a
footstep is made on the tick whose current frame equals the left or
right foot frame (when either is nonzero); in turn states, both feet
shuffle on frame 0.

## First person

### The graph and its models

A weapon tag names a first-person model and a first-person graph; the
globals tag names the arms model (`characters\cyborg\fp\fp`, 37 nodes).
The graph's node list holds every node of both models (assault rifle
graph: 42 = 37 arm nodes + 5 weapon nodes); its animations pose all of
them. To draw, the engine builds one matrix per graph node and gives
each model node the matrix of the graph node with the **same name**
(exact, case-sensitive comparison). If any node of the arms model or of
the weapon model has no namesake in the graph, the weapon is not shown
in first person. (Every multiplayer weapon's graph contains all its
nodes; checked.) Markers of the first-person weapon (muzzle effects)
use the same remapped matrices.

**Root at the camera.** The graph's node 0 (`frame bone24`) is composed
with the camera's frame: position = camera position, forward = camera
forward, up = camera up. No node drives the camera. The first-person
view is hidden while zoomed and outside first-person perspective.

Sanity check from the disc: in the assault rifle's idle, `frame gun` is
0.067 WU ahead of the camera, 0.040 right and 0.089 below, the right
wrist 0.039 from it and the left wrist 0.10 further forward.

### The pose each drawn frame

1. Base: the state animation at its current frame (if its node count
   fits the graph). First-person animations are sampled without a
   model, so the checksum is ignored and a mismatch never falls back.
2. Ammunition (slot 17, overlay): frame = rounds loaded in magazine 0,
   applied if that frame exists (the needler shows its needles). While
   reloading, once 44 ticks of the reload have passed, the frame moves
   toward the predicted count: elapsed e = reload length - remaining;
   if e >= 44, g = min(1, (e - 44) x 0.2), frame = loaded +
   trunc((min(rounds total, magazine maximum) - loaded) x g).
3. Moving (slot 3, overlay) at its own frame, if running.
4. Overcharged jitter (slot 15), continuous scaled overlay at its own
   fractional frame with weight overcharge + 0.5.
5. Overlays (slot 4, a 9-frame overlay, used only if it has at least 9
   frames), scaled overlays by weight:

| Frame | Applied when | Weight |
|---|---|---|
| 0 | position x > 0 | position x |
| 1 | position x < 0 | -position x |
| 3 | position y > 0 | position y |
| 2 | position y < 0 | -position y |
| 4 | turning x > 0 | turning x |
| 5 | turning x < 0 | -turning x |
| 7 | turning y > 0 | turning y |
| 6 | turning y < 0 | -turning y |
| 8 | push back > 0 | push back |

6. The transition blend, if running.

### Smoothed inputs

All three use one smoothing step toward a target x with maximum change
a per tick, maximum speed m and bounds [lo, hi], on position p and
velocity v: let d = x - p. If |d - v| <= min(a, m): v = 0, p = x
clamped to [lo, hi]. Otherwise u = m if 2 a |d| >= m^2, else
sqrt(2 a |d|), with the sign of d; e = u - v clamped to [-a, a];
v = v + e; p = p + v + e / 2, clamped to [lo, hi] (v is not clamped).

- position x, y: target = the unit's throttle forward and left;
  a = 0.08, m = 0.5, bounds [-1, 1]. Updated only once the view has
  been drawn.
- turning x, y: target x = (signed yaw change of the camera since the
  last tick) x 30, target y = (signed pitch change) x -30, each clamped
  to [-1, 1]; a = 0.03, m = 0.2, bounds [-1, 1]. So turning x > 0 is
  turning left and turning y > 0 is pitching down.
- push back: each primary shot adds 0.05 to its velocity; target 0,
  a = 0.01, m = 0.2, bounds [0, 1]; when it reaches exactly 1 its
  velocity is zeroed.

### States

The view runs a small state machine; each state plays one slot:

| State | Slot | Entered by |
|---|---|---|
| idle | 0 idle | weapon switch; end of most states |
| posing | 1 posing | random idle timeout |
| primary fire / secondary fire | 2 fire-1 / 14 fire-2 | a shot |
| primary / secondary misfire | 18 / 19 | a misfire |
| melee | 13 melee | player melee |
| light on / light off | **5** / **6** | flashlight toggled (only from idle or posing) |
| reload empty / reload full | 7 / 8 | reload start with 0 / some rounds loaded |
| shotgun enter / exit empty / exit full | 23 / 24 / 25 | shotgun reload (below) |
| put away | 11 put-away | weapon put away |
| ready | 10 ready | weapon readied |
| throw grenade | 16 | grenade throw |
| charged | 12 overcharged | trigger charged |
| overheating, overheating again | 21, 22 | overheat; ready while overheated |
| overheated | 9 | end of overheating |
| throw grenade overheated | 20 | throw while overheated |
| overheated exit | 26 o-h-exit | the weapon's overheat ends with an exit |
| overheating with supercharge | 27 o-h-s-enter | supercharged overheat |

- Entering a state whose slot is empty does nothing: the view stays in
  its current state and animation. (The assault rifle and flamethrower
  have no fire-1, so firing them changes no state; only the push-back
  overlay moves the gun.)
- Fire and misfire are entered only from idle, posing, primary fire,
  charged, the shotgun reload states, overheated exit and the reload
  states; a shot during primary fire restarts the fire animation at
  frame 0, so a weapon firing faster than its fire animation shows only
  its first frames. Ready is ignored when already in ready. While the
  weapon is overheated, ready becomes overheating again and throw
  grenade becomes throw grenade overheated. A plasma pistol entering
  overheated while not overheated goes to idle instead.
- Blend into a state: 0 ticks for overheated, melee and ready (and for
  anything after a shotgun exit-empty), 3 for fire and misfire, 6
  otherwise. A ready message also cancels any running blend. Moving
  stopping in idle starts a 6-tick blend.
- When the state animation reports "ends next tick": idle, posing,
  fire, misfire, melee, light, shotgun exits, ready, throw grenade and
  overheated exit go to idle; the overheating states and throw
  overheated go to overheated; put away holds its last frame; shotgun
  enter goes to idle, or to an exit when it loaded the only shell
  needed; a reload goes to idle (shotgun: to exit empty or full when
  the last shell is in). Key frames do nothing in the view.
- The state animation's sound starts at its sound frame (only in
  first-person perspective).
- Moving starts (frame 0) when the throttle magnitude exceeds 0.1 and
  the unit is not flying through the air; it advances each tick and
  loops; it stops when that ends.
- Overcharged jitter starts at frame 0 on entering charged; each tick
  its frame becomes (frame + 2 (overcharge + 1)) mod N; it stops on
  leaving charged.
- Posing: while idle and no input moves (zero autoaim, no zoom, all
  smoothed inputs exactly 0), a counter runs; the wait is drawn
  uniformly between the globals' first-person idle time bounds (seconds
  x 30); when the counter exceeds it a new wait is drawn and, unless a
  uniform draw falls below the globals' idle skip fraction, the view
  enters posing. Any input during posing returns to idle.

### Shotgun reloading

The shotgun loads one shell per reload step. When a reload starts from
outside the reload states, shells needed k = min(magazine maximum -
loaded, rounds total); the view plays enter (slot 23) and remembers
whether the tube was empty. If k = 1 the enter is followed by an exit;
otherwise each further shell plays slot 7 (reload-empty, 12 frames on
the disc) and the last one is followed by exit-empty or exit-full.

## The weapon's own animations

A weapon's third-person graph (pistol, shotgun, sniper rifle on the
disc) has a weapons table. When the weapon changes state it plays, as
its base animation: idle slot 0, primary shot 9, secondary shot 10,
primary chamber 5, secondary chamber 6, either reload 3, ready 1,
put-away 2, and for either charged state slot 8 (reference reading;
charged-1 is never used, see Open questions). An empty slot keeps the
current animation, except that idle always starts. The pistol's and
the sniper rifle's only one is `fire-1` (5 frames: the slide, the
bolt). The shotgun's graph fills slot 4 (`reload-2`, 27 frames) and
slot 5 (`chamber-1`, 22 frames); by the rule above its reload (slot 3)
plays nothing and only the pump (chamber) animates.

## Gameplay events driven by animations

`weapons.md` and `movement.md` own the surrounding rules; these are the
links to animation data:

| Event | Animation | Rule |
|---|---|---|
| weapon ready | first-person slot 10 ready | the weapon's ready timer is set to its frame count; triggers are ignored until the timer, decremented once per tick before the triggers are read, reaches 0 |
| magazine reload | first-person slot 7 reload-empty, **whether or not the magazine is empty** | the magazine's reload timer is set to its frame count; rounds are added on the tick its decremented value is at most 1 |
| reload interrupted | same | rounds are still added if more than half the time had run (2 x remaining < frame count) |
| shotgun first shell | first-person slot 23 enter | timer = enter's frame count when the reload starts fresh |
| shotgun further shells | slot 7 | timer = slot 7's frame count |
| player melee | first-person slot 13 melee: frame count M, key frame K | lockout T = M - floor(M / 4) ticks; the hit (melee damage) happens K + 1 ticks after the melee starts; the next melee can start T + 1 ticks after the start |
| unit melee (AI, airborne melee, leap melee) | third-person melee state animation | damage on its key frame |
| grenade release | third-person class slot 20 throw-grenade | the grenade is created in the hand once the throw animation's frame is at least 2, and released on the tick the animation reports its key frame; leaving the state earlier releases it at once |
| jump | (none) | the unit's animation update reporting a request triggers the jump |
| footsteps | move animations' foot frames | see Bipeds |

The first-person fire animation's length does not limit the rate of
fire (the trigger's timing does); the first-person `throw-grenade`
animation is display only. The player's third-person melee animation
(weapon type melee, 36 frames for the assault rifle) is display only:
the hit timing is the first-person one.

## Per-tick order

For one biped unit (player or AI), one tick:

1. Movement and turning set the desired state (Bipeds); frame info of
   the current base animation is read at its current frame (dyaw
   applied; dx, dy for non-player physics).
2. Airborne, landing or dying logic may override the desired state.
3. Player melee: start or count down; the hit on its tick.
4. Animation update: advance the current base animation (sound;
   key-frame and end events: grenade release, melee damage, death
   hold, seat exit, ...), then the action (removed at its end, blend
   back), then the overlay action; then apply the desired state if it
   differs and may interrupt, or if a play-once state ended.
5. Weapon update (elsewhere in the tick): timers, which start actions
   and overlay actions through weapon state changes.
6. Pose (when node matrices are needed): base sample (or bind pose),
   object overlays, object scale, action (replacement), overlay action,
   soft ping, emotion, mouth, acceleration overlays, aiming screen,
   looking screen, transition blend, then matrices, then IK.
7. The transition index advances by one at the start of the object's
   next update.

For the first-person view, each tick: overheat checks, advance the
state animation (sound; end leads to the next state), moving overlay,
overcharged jitter, smoothed inputs (position, turning only after a
draw), push back, blend index, posing timer. Each drawn frame builds
the pose as in First person. On the Xbox the game draws one frame per
tick and nothing is interpolated between ticks; a port that draws
faster must hold the tick's pose (or interpolate final matrices as its
own choice, not the original's).

## Measured facts

All on bloodgulch unless stated; the multiplayer maps share these tags.

### Inventory

| Graph | Nodes | Animations | Blocks |
|---|---|---|---|
| `characters\cyborg\cyborg` | 19 | 195 | 24 seats, weapons 1, unit damage 176, 23 sounds |
| `weapons\assault rifle\fp\fp` | 42 | 13 | first person 17 slots, 3 sounds |
| `weapons\pistol\fp\fp` | 44 | 10 | 17 slots, 4 sounds |
| `weapons\shotgun\fp\fp` | 41 | 12 | 26 slots, 8 sounds |
| `weapons\plasma rifle\fp\fp` | 42 | 14 | 27 slots, 5 sounds |
| `weapons\rocket launcher\fp\fp` | 42 | 10 | 17 slots, 6 sounds |
| `weapons\sniper rifle\fp\fp` | 41 | 10 | 17 slots, 5 sounds |
| `weapons\plasma pistol\fp\fp` | 41 | 18 | 28 slots, 5 sounds |
| `weapons\flamethrower\fp\fp` | 40 | 8 | 17 slots, 4 sounds (and a stray seat block) |
| `weapons\needler\fp\fp` | 54 | 10 | 18 slots, 4 sounds |
| `weapons\ball\fp\fp`, `weapons\flag\fp\fp` | 38 | 6 | 14 slots, 3 sounds |
| `weapons\pistol\pistol` | 7 | 1 | weapons table |
| `weapons\shotgun\shotgun` | 2 | 2 | weapons table |
| `weapons\sniper rifle\sniper rifle` | 4 | 1 | weapons table |
| Warthog, Scorpion, Ghost, a plant | 0, 11, 0, 0 | 11, 14, 4, 1 | vehicle, overlays |

Disc totals over the 23 playable maps: 628 graphs (with repeats),
12 711 animations: 9173 base, 3097 overlay, 441 replacement; frame info
none 6364, dx dy 5525, dx dy dyaw 812, dx dy dz dyaw 10.

### The Spartan's labels

Seats (in block order): `W-driver`, `W-gunner`, `W-passenger`,
`crouch`, `stand`, `G-driver`, `B-driver`, `S-driver`, `S-gunner`,
`S-rider`, `R-driver`, `P-driver`, `alert`, `scorpion-driver`,
`driver lifepod`, `W-rider`, `P-riderLF`, `GT-gunner`, `CT-driver`,
`P-riderRF`, `scorpionLF`, `scorpionLB`, `scorpionRB`, `scorpionRF`.

`stand` weapon classes and types: `rifle` (`ar`, `sg`, `sr`, `pr`,
`pb`, `f`, `ne`), `pistol` (`hp`, `pp`, `ne`, `b`), `missile` (`rl`),
`support` (`ft`), `unarmed` (empty). `crouch`: the same without `pb`
and `rifle ne`. Vehicle seats have `unarmed`, `fixed` or `rifle`
classes.

Bounds: every seat's looking bounds are right/left 1.2217 rad (70
degrees) per frame, 1 frame each way, down/up 0.7854 (45 degrees),
1 frame each (3 x 3 `stand look`). `stand rifle` and `stand pistol`
aiming bounds are 1.5708 per frame and 1 frame in all four directions
(3 x 3 grids, 9 frames).

### The Spartan's animations, standing

Frame count / key frame / loop frame; frame info type; foot frames
left/right; sums over the animation of dx, dyaw.

| Slot | Rifle | Pistol |
|---|---|---|
| idle | `stand rifle idle` 59/0/0, fi 1 | `stand pistol idle` 49/0/0, fi 1 |
| turn-left | 7/0/0, fi 2, dyaw +1.5708, feet 7/14 | 7/0/0, fi 2, dyaw +1.5708, feet 4/8 |
| turn-right | 7/0/0, fi 2, dyaw -1.5708, feet 14/7 | 7/0/0, dyaw -1.5708, feet 8/4 |
| move-front | 20/0/0, fi 1, dx 1.50 (0.075 WU/tick), feet 0/10 | 20/0/0, dx 0, feet 0/10 |
| move-back | 20/0/0, dx -1.34, feet 9/19 | 20/0/0, feet 9/19 |
| move-left | 20/0/0, dy +1.34, feet 17/8 | 20/0/0, feet 17/8 |
| move-right | 20/0/0, dy -1.34, feet 8/17 | 20/0/0, feet 8/17 |
| airborne | 79/0/40 | 80/0/40 |
| land-soft | 29/0/0 | 29/0/30 (holds the last frame) |
| land-hard | 39/0/30 | 40/0/30 |
| throw-grenade | `stand rifle throw-grenade` 38/**8**/0 | the same animation |
| stunned-front/back/left/right | 19, 26, 19, 19 frames | the rifle's |
| aim-still, aim-move | overlays, 9 frames | overlays, 9 frames |
| ready (class 23) | `stand rifle ready` replacement, 15 | none |
| reload-1 (type) | `ar` 87, `sg` 11, `sr` 64 (replacement) | `hp` 64, `ne` 64 |
| fire-1 (type, overlay) | `ar` 4, `sg` 13, `pr` uses `ar` 4 | `hp` 9, `pp` uses `hp` 9 |
| fire-2 (type) | `ar` 15 | none |
| chamber-1 (type) | `sg` 12 | none |
| melee (type, replacement) | `ar` 36, `sg` 38, `sr` 35, `pr` 40, `f` 32 | `hp` 46, `pp` 40, `ne` 46, `b` 28 |

Crouching (rifle / pistol): idle 60 / 31; turns 7 (dyaw +-1.5708);
moves 30 frames each (dx 0.90 front, -0.65 back, dy +-0.60 for the
rifle and pistol), feet front 29/15; airborne `crouch rifle airborne`
40/0/20 / `stand unarmed airborne` 79/0/39; land-soft 29 / 30;
land-hard 59 (no loop) / 40 (loop 30); throw-grenade and type
animations as standing (the crouching `sg` and `pr` melee slots use
`stand rifle sr melee`, 35).

Deaths (every seat): airborne-dead `stand airborne-dead` 37 frames,
loop frame 0, no sound; landing-dead `stand landing-dead` 42 (sound at
frame 2). Damage table: every one of the 44 s-ping slots (all
directions and parts) holds the same `s-ping front gut`, a 9-frame
overlay with three permutations (`%0`, `%1`, `%2`); s-kill front gut
69, front chest 44, left 30, right 30, back 43; h-kill front gut 46,
front head 60, left gut 41, left head 44, right gut 41, right head 44,
back 40; no h-ping. Which part resolves to which kill animation is
`health_shields.md`'s table (arms and hands share chest's, legs and
feet gut's). All kill animations have loop frame 0 and one
permutation. Their sounds (graph sounds: 0 `...\bodyfalls\marines\
skillfrontgut`, 1 `...\hkillfronthead`, 2 `...\general`): s-kill front
gut sound 2 at frame 15, front chest 2 at 23, left none, right 2 at 15,
back 2 at 16; h-kill front gut 2 at 16, front head 1 at 0, left gut 2
at 17, left head 2 at 17, right gut 2 at 17, right head 2 at 29, back 2
at 17; landing-dead 2 at 2. None has a key frame. No Spartan animation
is compressed. Graph nodes and their limp body joints: spine, spine1
and neck fixed (flag 0x4); both calves and both forearms hinges (0x2);
the others ball and socket (0x1); limp body node radius 0 (0.03 WU is
used).

### First-person graphs

Frame count, key frame, loop frame of each filled slot. Every
first-person animation on the disc is uncompressed; every slotted one
has key frame 0 except melee; the second key frame is 0 everywhere.
"(=x)" means the slot holds the same animation as slot x.

| Weapon | ready | put-away | fire-1 | reload-empty | reload-full | melee | throw-grenade | others |
|---|---|---|---|---|---|---|---|---|
| assault rifle | 29/0/0 | 11/0/0 | none | 87/0/0 (`reload-full`) | 87 (=7) | 37/**3**/0 | 37/0/0 | idle 80, posing 118, moving 26, overlays 9, light-off 27 (also in light-on) |
| pistol | 35/0/0 | none | 15/0/0 | 67/0/0 | 65/0/0 | 45/**4**/0 | 38/0/0 | idle 60, posing 133, moving 25, overlays 9 |
| plasma rifle | 29/0/0 | none | 22/0/0 | none | none | 36/**6**/0 | 37/0/0 | overheated 50/0/58, overheating 58, overheating-again 50, throw-overheated 35, misfire-1 18, o-h-exit 35, idle 60, posing 105 |
| shotgun | 23/0/0 | none | 26/0/0 | 12/0/0 | 12 (=7) | 36/**4**/0 | 37/0/0 | enter 15, exit-empty 42, exit-full 24, idle 62, posing 66 |
| sniper rifle | 29/0/0 | none | 19/0/0 | 94/0/0 | 83/0/0 | 36/**6**/0 | 37/0/0 | idle 80, posing 166 |
| rocket launcher | 22/0/0 | none | 28/0/0 | 125/0/0 | 111/0/0 | 52/**4**/0 | 43/0/0 | idle 60, posing 118 |
| needler | 23/0/0 | none | 8/0/0 | 70/0/0 (`reload-full`) | 70 (=7) | 48/**6**/0 | 37/0/0 | ammunition overlay 21 frames, idle 60, posing 88 |
| plasma pistol | 14/0/0 | none | 13/0/0 | 9/0/34 (`overheated`) | (=7) | 33/**4**/0 | 37/0/0 | fire-2 24, overcharged 20/0/20, overcharged-jitter 16 (overlay), overheated 9/0/34, overheating 34, overheating-again 25, misfire-1 19, throw-overheated 33, o-h-exit 24, o-h-s-enter 24 |
| flamethrower | 30/0/0 | none | none | 104/0/0 | 104 (=7) | 51/**8**/0 | 39/0/0 | idle 60, posing 113 |

Every graph's moving overlay is 25 or 26 frames and its overlays
animation 9. The assault rifle graph also holds three animations no
slot uses: `first-person firing` (21 frames), `first-person
reload-full2` (87) and `first-person stealth-melee` (61, key 24).

Derived gameplay times (ticks; see Gameplay events):

| Weapon | ready | reload | melee lockout T | melee hit after | next melee after |
|---|---|---|---|---|---|
| assault rifle | 29 | 87 | 28 | 4 | 29 |
| pistol | 35 | 67 (even when not empty) | 34 | 5 | 35 |
| plasma rifle | 29 | (no magazine) | 27 | 7 | 28 |
| shotgun | 23 | 15 first shell, 12 each further | 27 | 5 | 28 |
| sniper rifle | 29 | 94 | 27 | 7 | 28 |
| rocket launcher | 22 | 125 | 39 | 5 | 40 |
| needler | 23 | 70 | 36 | 7 | 37 |
| plasma pistol | 14 | (no magazine) | 25 | 5 | 26 |
| flamethrower | 30 | 104 | 39 | 9 | 40 |

## Verification

Done on the disc with throwaway scripts:

- Every layout above: element sizes from the block contents, every
  animation's frame size, default size, frame data size and frame info
  size against the formulas, on all 23 maps (no exception); unused
  fields zero; graph node parent order; slot tables against animation
  names (the names say what the slots mean).
- Quaternion convention and component order: the Spartan's `stand
  rifle idle` and `stand rifle move-front` posed with the model's node
  tree. Game convention: pelvis at height 0.360, head 0.56, both feet
  at z 0.068, left foot at +Y (0.072) and right foot at -Y (-0.068),
  hands forward of the body (x 0.11 to 0.24). With the conjugated
  (usual) convention the same data puts both feet at z 0.39 to 0.44,
  above the pelvis, and both on the right side (y -0.29 and -0.30):
  wrong.
- Segment lengths: |translation| of every node on every frame of every
  Spartan base animation equals the model's distance from parent within
  5e-5 WU (this checks the frame and default-data walk; lengths do not
  depend on rotation, so the convention is checked by the pose test
  above).
- Run cycle: over frames 0, 4, 8, 12, 16 of `stand rifle move-front`
  the left foot's x goes 0.067, -0.217, -0.194, -0.093, 0.069 and the
  right foot's -0.161, -0.011, 0.139, -0.081, -0.208: they alternate,
  and each lifts (z up to 0.19) while moving forward. The pelvis stays
  at x -0.007: the cycle is in place; travel is in the frame info
  (0.0763 WU per tick at frame 0, 1.50 WU over the 20 frames).
- Aim grid orientation and overlay order: applying each frame of
  `stand rifle aim-still` to idle frame 0, frame 4 leaves the pelvis,
  spine, clavicles, head, arms and hands unchanged (but not the legs:
  the left thigh and foot turn 2.9 degrees and the right ones 4.1),
  frames 3 and 5 swing both hands about 90 degrees right and left,
  frames 1 and 7 lower and raise them.
- First person: in every multiplayer weapon's graph all 37 arm nodes
  and every weapon model node are present by name; in idle and ready,
  `frame gun` keeps a constant distance from `frame r wriste` (it is
  its child) and both wrists stay within 0.3 WU of the camera and below
  it.
- Compressed: six a10 crewman cinematics (`crew_panic1`, `crew_panic2`,
  `cryo_assistant`, `cryo_technician`, `sitting_turn00`,
  `sitting_turn01`) decoded with the layout above. Raw 12-bit
  rotations have |q| between 0.9994 and 1.0002 for all keys and
  defaults (a wrong bit layout gives arbitrary lengths); key counts sum
  to the key table sizes; key times increase and end at N - 1; segment
  lengths match `characters\crewman\crewman` within 1.1e-4 WU; standing
  poses have feet at z 0.052 and head at 0.50 to 0.55, sitting poses
  have the feet ahead and below the pelvis; the median per-frame
  rotation change of a node is 0.08 to 1 degree (the largest, 48 to 92
  degrees, are fast limb swings with a key every frame).

## Edge cases

- Index table shorter than the enumeration: missing slots are empty.
- Animation index out of the animations block, a permutation chain
  that loops or runs past the block: treat as none (a chain that loops
  stops at its first animation).
- Frame index outside 0 .. N - 1 for a replacement or overlay: not
  applied. A base animation that cannot be sampled (bad node count or
  data outside the data reference) gives the model's bind pose.
- Frame count 0: never on the disc; overlays with 0 frames are skipped.
- Flag bits at or above the node count: ignored.
- Compressed offsets or key ranges outside the stream: that node gets
  identity rotation, zero translation, scale 1 (the engine reports and
  skips).
- An aiming screen with fewer frames than W x H: skipped.
- A first-person animation with more nodes than the graph: not
  applied (none on the disc).
- A seat whose index table lacks slot 9 (look): no looking screen.
- Graph with an empty node block: unit and vehicle poses use the
  model's tree only; nothing reads graph nodes then.

## Quirks

- **Last match wins** in the seat/weapon label search (keep: it decides
  how the needler is held).
- **Flashlight slots are crossed**: turning the light on plays slot 5,
  labelled light-off; turning it off plays slot 6. Only the assault
  rifle fills them, with one animation in both. Keep.
- **Reload length ignores the magazine state**: the timer always uses
  slot 7 (reload-empty); the pistol reload takes 67 ticks even when its
  animation is the 65-frame reload-full. Keep.
- **Melee is a quarter faster than its animation**: the lockout drops
  floor(M / 4) ticks while the animation still plays all M frames.
  Keep.
- **Needler ammunition display uses local player 0's state** while
  reloading (the engine reads the first view's state for every view).
  Matters only in split screen; keep or fix deliberately.
- **Scaled overlays are not renormalised** (the continuous scaled one
  is): observable only through a following transition blend. Keep.
- **Compressed continuous overlays ignore the fraction for rotation.**
  Affects only cinematics; keep.
- **A key frame on the last frame of a play-once animation, or at 0,
  never fires.** Keep.
- **Loop frames past the end hold the last frame** (plasma rifle and
  plasma pistol `overheated`, the pistol's `land-soft`). Keep.
- **The graph's root node stores parent 0** rather than NONE. Readers
  must not treat node 0 as its own child.
- **Foot frames past the end** never sound. Keep.
- **Charged states play the weapon's slot 8** for both triggers
  (reference reading). Keep as read until checked in game.

## Test cases

### Synthetic graph (unit tests)

A graph with two nodes and one animation, everything else empty.

Nodes block (2 elements of 0x40 bytes): node 0 `root`, sibling NONE,
child 1, parent 0, flags 0; node 1 `arm`, sibling NONE, child NONE,
parent 0.

Animation element: name `test`, type 0, frame count 3, frame size 20,
frame info type 0, checksum 0, node count 2, loop frame 0, weight 0,
key frame 1, second key frame 0, next NONE, flags 0, sound NONE, sound
frame 0, feet 0, 0, first permutation 0, cumulative weight 1.0, frame
info empty, translation flags (0x00000002, 0), rotation flags
(0x00000002, 0), scale flags (0, 0), compressed offset 0.

Default data (28 bytes): node 0 rotation i16 (0, 0, 0, 32767); node 0
translation f32 (0, 0, 0.5); node 0 scale f32 1.0; node 1 scale f32
1.0.

Frame data (60 bytes = 3 x 20), node 1 rotation i16 then translation
f32 per frame:

| Frame | Node 1 rotation (raw) | Node 1 translation |
|---|---|---|
| 0 | (0, 0, 0, 32767) | (0.1, 0, 0) |
| 1 | (0, 0, 23170, 23170) | (0.2, 0, 0) |
| 2 | (0, 0, 32767, 0) | (0.3, 0, 0) |

Expected samples (rotation as decoded, before any normalisation):

| Frame | Node 0 (q, t, s) | Node 1 (q, t, s) | Node 1 forward axis in node 0's frame | Node 1 origin, object frame |
|---|---|---|---|---|
| 0 | (0, 0, 0, 1), (0, 0, 0.5), 1 | (0, 0, 0, 1), (0.1, 0, 0), 1 | (1, 0, 0) | (0.1, 0, 0.5) |
| 1 | same | (0, 0, 0.707114, 0.707114), (0.2, 0, 0), 1 | (0, -1, 0) within 1e-5 | (0.2, 0, 0.5) |
| 2 | same | (0, 0, 1, 0), (0.3, 0, 0), 1 | (-1, 0, 0) | (0.3, 0, 0.5) |

Playback of `test` from frame 0: advance 1 gives frame 1, result key
frame; advance 2 gives frame 2, result ends next tick; advance 3 gives
frame 0, result restarted. With loop frame 1 instead: advance 3 gives
frame 1, result looped.

Overlay of frame 1 (type 1) onto a base with node 1 at (0, 0, 0, 1),
(1, 2, 3): node 1 becomes (0, 0, 0.707114, 0.707114), (1.2, 2, 3); node
0 unchanged. Overlay onto a base rotation q_b = (0, 0, 0.707107,
0.707107) gives q_o * q_b = (0, 0, 1.0000103, 0) (two quarter turns: the
stored 23170 / 32767 = 0.7071139 is 7e-6 above the exact value, so the
product is 1.03e-5 above 1; within 2e-5 of (0, 0, 1, 0), exact once
normalised).

Transition: from pose A (node 1 rotation identity, translation (0, 0,
0)) to pose B (rotation (0, 0, 0.707107, 0.707107), translation (0.3,
0, 0)) over n = 3 ticks: tick i = 0 gives u = 1/3: rotation
normalise(0, 0, 0.235702, 0.902369) = (0, 0, 0.252725, 0.967538),
translation (0.1, 0, 0); i = 2 gives u = 1: B exactly.

Interpolated sample (compressed rule or continuous overlay) half way
between frames 0 and 1: rotation (0, 0, 0.382685, 0.923879),
translation (0.15, 0, 0).

### Compressed rotation (unit test)

Bytes 80 5A 00 00 A8 05: words 0x5A80, 0x0000, 0x05A8; fields i =
0x5A8, j = 0, k = 0, w = 0x5A8; widened 0x5A85 = 23173 each; components
(0.707205, 0, 0, 0.707205); normalised (0.707107, 0, 0, 0.707107).
A negative field: 0xC00 widens to 0xC00C = -16372, component
-0.499649. The disc example in Compressed animation data is a third
case.

A compressed node with key times (3, 9) for N = 10, default D and keys
K0, K1: frame 0 gives normalise(D); frame 2 gives nlerp(D, K0, 2/3);
frame 3 gives normalise(K0); frame 6 gives nlerp(K0, K1, 0.5); frame 9
gives normalise(K1). Node header 0x0000A002 means first key 10, count
2.

### Aiming screen (unit test)

Bounds 1.5708 rad per frame and 1 frame in all four directions (3 x 3,
the Spartan's rifle). Yaw 0.5, pitch -0.3: column 1, u = 0.318310; row
0, p = 0.809014; frames f00 = 1, f10 = 2, f01 = 4, f11 = 5;
translation weights 0.130193, 0.060793, 0.551497, 0.257517. Yaw 2.0:
column 1, u = 1 (all of column 2). Yaw -2.0: column 0, u = 0. Pitch
1.0: row 1, p = 0.636620.

### First-person smoothing (unit test)

Push back after one shot (velocity 0.05, position 0), one step per
tick: positions 0.035, 0.06, 0.075, 0.08, 0.075, 0.06, 0.035, 0.005314,
0, 0, then velocity 0 from tick 11. Position x with throttle 1 from
rest (a = 0.08, m = 0.5): 0.12, 0.32, 0.6, 0.859473, 0.992456, 1.0,
0.972982, 1.0 (velocity 0 at tick 8).

### Selection (unit test)

A graph with seat `stand` holding class `rifle` (types `ar`, `ne`) and
class `pistol` (types `hp`, `ne`): label `ne` selects (stand, pistol,
ne); label `ar` selects (stand, rifle, ar); label `xx` leaves the
previous selection. A seat with class `unarmed` whose single type has
an empty label is selected by `unarmed`.

### Disc-backed (slow tests, bloodgulch)

| Setup | Expected |
|---|---|
| `weapons\assault rifle\fp\fp` first-person table | 17 slots: 0 idle 80, 1 posing 118, 2 none, 3 moving 26 (overlay), 4 overlays 9 (overlay), 5 and 6 `first-person light-off` 27, 7 and 8 `first-person reload-full` 87, 9 none, 10 ready 29, 11 put-away 11, 12 none, 13 melee 37 key 3, 14 none, 15 none, 16 throw-grenade 37; no loop frames; sound indices: reload 0, melee 1, ready 2 (sound frame 0) |
| pistol, plasma rifle, shotgun first-person tables | as in Measured facts |
| `characters\cyborg\cyborg`, `stand rifle idle` frame 0 | 19 nodes, rotation flags 0x0002FD7E, no translation or scale flags, frame size 112, default size 344; node 0 `bip01 pelvis` rotation raw (-20504, -10788, -20504, -10788) from the default data, translation (0, 0, 0.36); node 3 `bip01 spine` rotation raw (2810, -484, -5546, 32167) from frame 0; node 12 `bip01 head` raw (-10323, 1667, 1366, 31023) |
| same, posed with the model's tree | pelvis (0, 0, 0.360), head (0.049, -0.051, 0.560), left foot (0.121, 0.072, 0.068), right foot (-0.096, -0.068, 0.068), all within 0.001 |
| `stand rifle turn-left` frame info | 7 records of (0, 0, 0.2244) |
| unit selection with seat `stand`, labels `ar`, `hp`, `ne`, `unarmed` | classes rifle, pistol, pistol, unarmed |
| a10 `crew_panic1` | compressed; 17 rotation and 2 translation nodes; 1421 rotation keys; node 0 header 0x00000033 (first 0, count 51); default rotation of node 0 as in the worked example |
| `characters\cyborg\cyborg` damage table, slots 0 to 43 | each resolves to `s-ping front gut%0` with permutations `%1`, `%2` (overlay, 9 frames) |
| same, s-kill front: left hand, left arm, chest, head, left leg, left foot | `s-kill front chest` ×4, `s-kill front gut` ×2; h-kill front head `h-kill front head`, h-kill front hand `h-kill front gut` |
| same, h-kill back head; s-kill left foot | `h-kill back gut` (40); `s-kill left gut` (30) |
| `stand airborne-dead`, `stand landing-dead` | loop frame 0 on both; landing-dead sound index 2 (`sound\sfx\impulse\bodyfalls\marines\general`) at frame 2 |
| graph nodes | flags: spine, spine1, neck 0x4; calves, forearms 0x2; others 0x1; limp body node radius 0 |

## Confidence per section

| Section | Confidence |
|---|---|
| Graph and element layouts, sizes, offsets | checked against the disc (all maps) |
| Slot names and order | data names; the order from the reference, confirmed by the animation names in the Spartan's and every first-person graph |
| One animation, uncompressed frame data | checked against the disc |
| Compressed layout and quaternion packing | checked against the disc (six decoded animations, a10) |
| Compressed sampling between keys | reference; consistent with the disc (smooth results) |
| Quaternion convention, overlay product order, aim grid layout | checked against the disc (poses) |
| Playback results, loops, key frames, sound timing | reference only |
| Blends, transitions, tick counts | reference only |
| Unit selection, states, actions, interruptions, death | reference only (labels and slot contents checked on the disc; the whole Spartan damage table, loop frames and death sounds read from the disc) |
| IK | reference only |
| Bipeds: movement choice, root motion use, footsteps | reference only (frame info values checked) |
| First person: remapping, root at camera | reference; node presence and pose checked on the disc |
| First person: state machine, smoothing, posing | reference only |
| Gameplay timers from animations | reference only for the rules; the frame counts and key frames checked on the disc |
| Measured facts | checked against the disc |

## Open questions

- The reference is a PAL build; the disc NTSC. No disc animation has
  the 25 Hz flag, so frame counts are taken as 30 Hz ticks. Whether
  the NTSC build's timers (ready, reload, melee) use the frame counts
  exactly as the PAL reference does should be confirmed by timing a
  reload and a melee in the real game (pistol reload 67 ticks = 2.23 s;
  assault rifle melee hit 4 ticks after the press).
- Weapon state "charged" playing the weapon graph's slot 8 for both
  triggers may be a decompilation slip for slots 7 and 8; no
  multiplayer weapon graph fills either, so it does not matter for
  multiplayer.
- The unit overlay action's looping test in the reference compares the
  unit's state rather than the overlay action (it loops only while the
  unit is in turn-right or move-front); taken as written, its effect on
  the third-person fire overlays is that they restart while running
  forward. Check in game whether a running Spartan's fire recoil
  repeats.
- The exact moment the grenade is created in hand ("frame at least 2"
  is checked against the unit's base animation in the unit update,
  whose order relative to the animation advance was not pinned down to
  the tick); `projectiles.md` should confirm with a frame capture.
- Scale keys in compressed data are indexed by rank for defaults in the
  reference; no disc data exercises this.
- Graph flags 1 and 3 on some campaign graphs are assumed to be
  build-time compression options with no run-time effect.
- The first-person idle time bounds and skip fraction live in the
  globals tag (`player information`, offsets 0x9C, 0xA0 and 0xA4: 10 s,
  15 s and 0.25 on the disc); `globals.md` owns the layout.
