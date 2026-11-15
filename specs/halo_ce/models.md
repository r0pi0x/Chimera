# Halo: Combat Evolved: models

Source basis: the Xbox disc's map files, decoded field by field with
throwaway scripts (every model tag of the 13 multiplayer maps, plus
`a10` for comparison); and the game's model, object, renderer, sky and
first-person weapon code from the behaviour reference, used for meaning
only. Where the reference and the disc differ, the disc wins and the
difference is noted.
Status: draft

Sections: Summary; Units and conventions; Constants; The model tag;
Markers; Nodes and the skeleton; Regions and permutations; Geometries
and parts; Vertex and index data; Texture coordinates; Skinning;
Shaders; Choosing a level of detail; Choosing permutations; Which
models the game draws where; Measured facts; Verification done; Edge
cases; Quirks; Test cases; Confidence per section; Open questions.

---

## Summary

- Every drawn object (bipeds, weapons in third and first person, the
  first-person arms, vehicles, scenery, powerups, projectiles, the sky)
  is a `mode` (model) tag. The Xbox disc has no `mod2` tags.
- A model has a skeleton of **nodes** (bind pose as translation plus
  quaternion per node, relative to the parent), named **markers**
  attached to nodes, **regions** each with one or more
  **permutations**, and **geometries**. Each permutation names one
  geometry per level of detail (five levels); a geometry is a list of
  **parts**, one shader each.
- A part's vertices and its triangle strip are not in the part: the
  part holds pointers to buffer records in the tag header's model
  buffer arrays, whose middle word is the address of the data in the
  tag data. Vertices are 32-byte compressed vertices in model space
  (the bind pose) with two node indices stored times 3 and one 16-bit
  weight. Indices are one 16-bit triangle strip per part.
- The stored quaternions rotate the **opposite way** from the usual
  right-handed convention: the rotation a stored (i, j, k, w) means is
  the usual rotation of its conjugate (-i, -j, -k, w). Getting this
  wrong mirrors the skeleton (the left thigh lands on the right).
- Strip triangles taken in strip order are **clockwise** seen from
  their front (the side the vertex normals point to), in the right-
  handed world frame.

## Units and conventions

- Distances, axes (+X forward, +Y left, +Z up, right-handed), the tag
  block, tag reference and data reference forms, address translation,
  byte order and NONE are as `cache_file.md` defines them. Positions
  here are in world units (WU).
- A model's own frame: node positions and vertex positions are in the
  frame of the object that draws the model, before the object's own
  placement. Objects face +X; a biped stands on z = 0 with its up
  along +Z.
- Offsets in tables are hexadecimal byte offsets from the start of the
  structure they describe. "Block" is the 12-byte tag block; "index16"
  is a signed 16-bit element index, NONE = -1.
- Levels of detail are numbered as the engine uses them: **level 0 is
  the lowest detail, level 4 the highest**. The five-element arrays
  below (cutoffs, node counts, geometry indices) are indexed by level.
- Quaternions in this spec are written (i, j, k, w), the order they are
  stored in (vector part first). "Usual convention" means: column
  vectors, right-handed frame, a unit quaternion q rotates a vector v
  to q v q*, and its matrix has first column
  (1 - 2(j^2 + k^2), 2(ij + wk), 2(ik - wj)).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| MODEL_CLASS | `mode` (0x6D6F6465) | class code | the model tag class ("model") |
| MODEL_SIZE | 0xE8 | bytes | the model's main struct |
| MARKER_SIZE | 0x40 | bytes | one marker |
| MARKER_INSTANCE_SIZE | 0x20 | bytes | one marker instance |
| NODE_SIZE | 0x9C | bytes | one node |
| REGION_SIZE | 0x4C | bytes | one region |
| PERMUTATION_SIZE | 0x58 | bytes | one permutation |
| PERMUTATION_MARKER_SIZE | 0x50 | bytes | one permutation marker (never present on the disc) |
| GEOMETRY_SIZE | 0x30 | bytes | one geometry |
| PART_SIZE | 0x68 | bytes | one part (the Xbox part; there is no local-node table) |
| SHADER_REF_SIZE | 0x20 | bytes | one shader reference |
| VERTEX_SIZE | 0x20 (32) | bytes | one compressed model vertex |
| UNCOMPRESSED_VERTEX_SIZE | 0x44 (68) | bytes | one uncompressed model vertex (never on the disc) |
| BUFFER_RECORD_SIZE | 12 | bytes | one record of the tag header's buffer arrays |
| DETAIL_LEVELS | 5 | | levels of detail |
| VERTEX_TYPE_MODEL_COMPRESSED | 5 | | the vertex buffer type of every part on the disc |
| VERTEX_TYPE_MODEL_UNCOMPRESSED | 4 | | the other model vertex type (never on the disc) |
| INDEX_TYPE_TRIANGLES | 0 | | index buffer type: a list, 3 indices per triangle (never on the disc) |
| INDEX_TYPE_STRIP | 1 | | index buffer type: a triangle strip (every part on the disc) |
| NODE_INDEX_SCALE | 3 | | vertex node indices are stored multiplied by this |
| NODE_NONE_BYTE | 0xFD (-3 as i8) | | "no second node" in a vertex |
| WEIGHT_SCALE | 32767 | | the vertex weight is i16 / 32767 |
| MAX_MARKERS | 256 | | per model |
| MAX_MARKER_INSTANCES | 32 | | per marker |
| MAX_NODES | 64 | | per model |
| MAX_REGIONS | 32 | | per model |
| MAX_PERMUTATIONS | 32 | | per region |
| MAX_GEOMETRIES | 256 | | per model |
| MAX_PARTS | 32 | | per geometry |
| MAX_SHADERS | 32 | | per model |
| MAX_VERTICES_PER_PART | 65535 | | |
| MAX_REGIONS_PER_OBJECT | 8 | | regions an object keeps a permutation for |
| VARIANT_RANDOM_LIMIT | 100 | | permutations with a variant number of 100 or more are never chosen when no variant is asked for |
| SKY_SCALE | 1/1024 | | the scale the sky model is drawn at |
| SKY_CAMERA_FACTOR | 0.9990234375 (1023/1024) | | the sky model's origin is the camera position times this |

## The model tag

One `mode` tag per model. Every object tag (biped, vehicle, weapon,
equipment, scenery, projectile) references its model with a tag
reference at object offset 0x28 (checked on every `bipd`, `vehi`,
`weap` and `scen` tag of bloodgulch; the vehicle gun weapons have a
null one); the objects spec owns the rest of the object.

Main struct, 0xE8 bytes:

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | flags32 | flags | Bit 0: blend shared normals (a build-time option of the tools); bit 1: parts have local nodes (the PC format only). 0 on every multiplayer model; `characters\elite\elite` in a10 has 1. The reader ignores it. |
| 0x04 | 4 | u32 | node list checksum | Opaque value computed from the node list by the tools; animation graphs carry the same kind of value to tell whether they match. Store, never recompute. (Spartan 0x6CFA6498; the plasma rifle's third- and first-person models share 0x409EF03C.) |
| 0x08 | 20 | 5 x f32 | detail cutoff pixels | One per level, level 0 first: the projected size in pixels at or above which the level may be used (see Choosing a level of detail). Non-decreasing on the disc. All zero on 49 of bloodgulch's 63 models (scenery, powerups, first-person models, the sky). |
| 0x1C | 10 | 5 x i16 | node counts | One per level. On the disc each equals the **highest node index** referenced by any vertex of that level's geometries (not a count: the Spartan has 19 nodes and stores 18 at every level; the assault rifle stores 0, 0, 0, 0, 2). Checked for every model of the 13 multiplayer maps. Informational; the reader does not need it. |
| 0x26 | 2 | | padding | |
| 0x28 | 8 | | unused | zero |
| 0x30 | 4 | f32 | base map u scale | multiplies decoded u (see Texture coordinates); 1.0 on most models, e.g. 1.999474 on the Warthog |
| 0x34 | 4 | f32 | base map v scale | multiplies decoded v; e.g. 3.000816 on the Warthog |
| 0x38 | 116 | | unused | zero |
| 0xAC | 12 | block | markers | MARKER_SIZE elements |
| 0xB8 | 12 | block | nodes | NODE_SIZE elements |
| 0xC4 | 12 | block | regions | REGION_SIZE elements |
| 0xD0 | 12 | block | geometries | GEOMETRY_SIZE elements |
| 0xDC | 12 | block | shaders | SHADER_REF_SIZE elements |

## Markers

A marker is a named frame attached to a node: where a weapon's
projectiles leave, where a hand grips, where a seat or camera is, where
effects and lights attach. On the disc the markers are stored on the
model (the map form); the editing tools' per-permutation form is never
present (every permutation's marker block is empty on every map
checked).

Marker, 0x40 bytes:

| Offset | Size | Type | Field |
|---|---|---|---|
| 0x00 | 32 | tag string | name |
| 0x20 | 20 | | unused (zero) |
| 0x34 | 12 | block | instances (MARKER_INSTANCE_SIZE elements) |

Marker instance, 0x20 bytes:

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 1 | u8 | region index | the region this instance belongs to |
| 0x01 | 1 | u8 | permutation index | the permutation of that region |
| 0x02 | 1 | u8 | node index | the node it is attached to |
| 0x03 | 1 | | padding | zero |
| 0x04 | 12 | real_point3d | translation | in the node's frame, WU |
| 0x10 | 16 | real_quaternion | rotation | (i, j, k, w), same convention as node rotations (below) |

- **Order.** Markers are sorted by name, case-insensitively, on every
  disc model; the engine looks a name up by binary search with a
  case-insensitive compare. A reader may build a map from lower-cased
  name to marker instead.
- **Which instances count.** For an object, an instance counts only if
  the object's current permutation of the instance's region equals the
  instance's permutation index. A marker can therefore have several
  instances (the plasma rifle's `vent` has 8, the Warthog's
  `headlights` 4), each a separate frame; a lookup returns all
  instances that count, in instance order (up to the caller's maximum).
  Without an object (no permutations given), every instance counts.
- **Marker frame.** marker world = world(node) x M(translation,
  rotation), with M built as for nodes. If the object is mirrored, the
  marker frame's left axis is negated afterwards.
- Instances whose region is 8 or more, or whose node is not below the
  node count, are skipped as damage.

Marker names on the disc (bloodgulch):

| Model | Markers (instance count 1 unless given; node in brackets) |
|---|---|
| `characters\cyborg\cyborg` (the multiplayer Spartan's) | antenna [6 spine1], body [6], flashlight [12 head], head [12], left foot [8], left hand [17], right foot [11], right hand [18] |
| `weapons\assault rifle\assault rifle` | flashlight, ground point, left hand cyborg, left hand marine, primary ejection, primary trigger, secondary ejection, secondary trigger, smoke (all on node 0) |
| `weapons\assault rifle\fp\fp` | flashlight, left hand cyborg, left hand marine, primary ejection, primary trigger, smoke |
| `weapons\pistol\pistol` | ground point, laser, left hand cyborg, left hand marine, primary ejection, primary trigger |
| `weapons\pistol\fp\fp` | flashlight, laser, left hand cyborg, left hand marine, primary ejection, primary trigger |
| `weapons\plasma rifle\plasma rifle` | ground point, heat flare, heat flare1, left hand cyborg, left hand marine, primary trigger, primary trigger1, secondary trigger, vent (8) |
| `weapons\plasma rifle\fp\fp` | flashlight, ground point, heat flare, heat flare1, left hand, primary trigger, primary trigger1, secondary trigger, vent (8) |
| `vehicles\warthog\warthog` (all on region 1 `hull`) | antenna, camera driver, camera gunner [1], camera rider, driver, driver enter, engine, gunner [1], gunner enter, gunner left foot [1], gunner left hand [8 gun], gunner right foot [1], gunner right hand [8], headlights (4), headlights_light, left hand wheel grip [6 steering wheel], passenger, passenger enter, primary ejection [8], primary trigger [8], right hand wheel grip [6], taillights (2) |
| `characters\cyborg\fp\fp`, the sky | none |

What the names are for (the specs of the systems that look them up own
the details): `primary trigger` / `secondary trigger` are the muzzles
where a weapon trigger's projectiles and firing effects start (there
is no marker called "muzzle"); `primary ejection` is where casings
leave; `left hand cyborg` / `left hand marine` are where the off hand
grips the weapon for each kind of holder; `ground point` is where a
dropped weapon rests; the Spartan's `right hand` and `left hand` are
where held weapons attach; `head` and `body` are aiming and effect
points; vehicle `driver`, `gunner`, `passenger` are seats, `... enter`
their entry points, `camera ...` the seat cameras; `headlights` and
`taillights` carry lights.

Measured frames (game convention, bind pose, model frame): the assault
rifle's `primary trigger` is at (0.2197, 0, 0.0525) with identity
rotation (facing +X, down the barrel); the Spartan's `head` marker is
at (-0.0031, 0, 0.6486) facing +X, its `left foot` at (0.0145, 0.063,
0.0037) and `right foot` at (0.0145, -0.0626, 0.0037), both facing +X;
the Warthog's `driver` is at (-0.0238, 0.1658, 0.3908) facing +X and
its `left hand wheel grip` and `right hand wheel grip` both face
roughly +X.

## Nodes and the skeleton

Node, 0x9C bytes:

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | tag string | name | e.g. `bip01 pelvis`, `frame gun` |
| 0x20 | 2 | index16 | next sibling node | NONE if last |
| 0x22 | 2 | index16 | first child node | NONE if a leaf |
| 0x24 | 2 | index16 | parent node | NONE for node 0 only |
| 0x26 | 2 | | padding | zero |
| 0x28 | 12 | real_point3d | default translation | the node's origin in its parent's frame, WU |
| 0x34 | 16 | real_quaternion | default rotation | (i, j, k, w), unit length (within 0.001 on every disc node) |
| 0x44 | 4 | f32 | distance from parent | = length of the default translation (0 for the root) |
| 0x48 | 32 | | unused | zero |
| 0x68 | 52 | matrix4x3 | default inverse matrix | build-tool field: the inverse of the node's bind-pose world transform (below) |

The matrix4x3 (52 bytes) is 13 f32: scale, forward (3), left (3),
up (3), position (3). Forward, left and up are the images of the x, y
and z axes (the matrix's columns). It maps a point p to
scale x (p.x forward + p.y left + p.z up) + position, and a vector the
same without the position. Every matrix stored in a model has scale
1.

**Node order.** On the disc node 0 is the root, every other node's
parent index is lower than its own, and the sibling/child links agree
with the parent indices (checked on every model of bloodgulch and
a10). A reader may compose transforms in index order using the parent
index. (The engine walks the tree breadth-first through the child and
sibling links from node 0 instead; on the disc both give the same
result.) A reader rejects a model whose parent indices break this
order, or treats such a parent as the root.

**Rotation from a stored quaternion (the game's convention).** With
s = 2 / (i^2 + j^2 + k^2 + w^2) (s = 0 if that sum is 0):

- forward = (1 - s(j^2 + k^2), s(ij - wk), s(ik + wj))
- left = (s(ij + wk), 1 - s(i^2 + k^2), s(jk - wi))
- up = (s(ik - wj), s(jk + wi), 1 - s(i^2 + j^2))

This is the **transpose** of the usual matrix of (i, j, k, w), that is
the usual matrix of the conjugate (-i, -j, -k, w). In usual terms the
stored quaternion rotates parent-to-child by its inverse: a stored
(0, 0, sin 45 deg, cos 45 deg) turns +X to **-Y** (90 degrees
clockwise seen from above), not to +Y. An implementation that keeps
the usual convention (for example a game engine's quaternion type)
must negate i, j and k when reading every node, marker and animation
quaternion of this game. (The animation spec states whether animation
quaternions follow the same rule; they are interpolated against node
defaults, so they must be read the same way.)

**Local transform.** L(n) = the rigid transform with rotation R(q_n)
(columns forward, left, up above) and translation t_n: it maps a point
in node n's frame into its parent's frame.

**World (bind) transform.**

- W(0) = O x L(0), where O is the object's frame (identity for a model
  viewed on its own; for a placed object, its position and its forward
  and up vectors, left = up x forward).
- W(n) = W(parent(n)) x L(n) for n > 0.

So a node's world position is W(parent).position + R_parent t_n, and
the distance from a node's world position to its parent's equals the
stored distance from parent (checked: within 3e-7 WU on every node of
the Spartan).

**Default inverse matrix.** The stored inverse equals the inverse of
the bind-pose world transform with O = identity: rotation part the
transpose of W(n)'s rotation, position -R^T p (checked on every Spartan
node: largest difference 2.8e-6 with the game convention; with the
usual convention the difference is 2.03, i.e. wrong). A reader may use
the stored matrices or recompute them; the tests compare both.

**The skin transform** of node n in a pose is S(n) = W_pose(n) x
Inverse(n). In the bind pose every S(n) is the identity, because
vertices are stored in model space already posed (see Skinning).

**Runtime state.** Nodes have no runtime fields in the tag beyond the
build-time inverse matrix. The object's current node matrices are
engine state.

Measured skeleton (Spartan, bind pose, model frame, WU): pelvis
(-0.0082, 0, 0.3944); left thigh (-0.0082, +0.0422, 0.3944) and right
thigh (-0.0082, -0.0422, 0.3944); spine (-0.0083, 0, 0.4318); head
(-0.007, 0, 0.609); left upper arm (-0.008, +0.090, 0.569); left hand
(0.009, +0.133, 0.352), right hand (0.009, -0.133, 0.352); left foot
(-0.039, +0.063, 0.069). Left nodes are at +Y, as the axes require;
the usual quaternion convention would put them at -Y.

## Regions and permutations

Region, 0x4C bytes:

| Offset | Size | Type | Field |
|---|---|---|---|
| 0x00 | 32 | tag string | name (e.g. `cyborg`, `hull`, `__unnamed`) |
| 0x20 | 32 | | unused (zero) |
| 0x40 | 12 | block | permutations (PERMUTATION_SIZE elements) |

Permutation, 0x58 bytes:

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | tag string | name | e.g. `__base`, `base00`, `~blur`, `~damaged`, `head_chris-11` |
| 0x20 | 4 | flags32 | flags | bit 0: cannot be chosen randomly |
| 0x24 | 2 | i16 | variant number | 0 normally; campaign characters use numbers such as 10..14 and 100..102, matching a "-NN" suffix of the permutation name |
| 0x26 | 2 | | padding | |
| 0x28 | 24 | | unused | zero |
| 0x40 | 10 | 5 x index16 | geometry indices | one per level, **level 0 (lowest detail) first, level 4 (highest) last**; index into the model's geometries |
| 0x4A | 2 | | padding | |
| 0x4C | 12 | block | markers | PERMUTATION_MARKER_SIZE elements; empty on the disc |

Permutation marker (for completeness; never present on the disc), 0x50
bytes: name (32 bytes), node index16 at 0x20, 2 bytes padding,
rotation quaternion at 0x24, translation at 0x34, then 16 unused bytes
from 0x40 to 0x50.

Notes:

- The Spartan's single permutation lists geometries (4, 3, 2, 1, 0):
  geometry 0 is its highest detail. The assault rifle lists (2, 2, 2,
  1, 0): levels 0 to 2 share one geometry. Models with all cutoffs zero
  usually list the same geometry five times.
- On the disc no geometry index is NONE. A reader treats NONE as "draw
  nothing at this level" and any other out-of-range index as damage.
- Permutations whose names start with `~` are states the game switches
  to by name, flagged cannot-be-chosen-randomly: `~blur` (Warthog and
  Scorpion tires/treads, chosen when the vehicle moves fast),
  `~primary-blur` (the Warthog's gun barrels while firing fast; a
  `~secondary-blur` name exists for second triggers), `~damaged` (the
  Ghost; chosen when the matching damage region is destroyed),
  `~shield_off` (ratrace, a10).

## Geometries and parts

Geometry, 0x30 bytes:

| Offset | Size | Type | Field |
|---|---|---|---|
| 0x00 | 4 | flags32 | flags (zero on the disc) |
| 0x04 | 32 | | unused (zero) |
| 0x24 | 12 | block | parts (PART_SIZE elements) |

Part, 0x68 bytes:

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | flags32 | flags | bit 0: stripped. The engine draws a part only if this bit is clear. 0 on every disc part. |
| 0x04 | 2 | index16 | shader index | into the model's shader block |
| 0x06 | 1 | i8 | previous filthy part index | NONE (-1) normally |
| 0x07 | 1 | i8 | next filthy part index | NONE (-1) normally |
| 0x08 | 2 | index16 | centroid primary node | the node the part's centroid moves with |
| 0x0A | 2 | index16 | centroid secondary node | the second most weighted node (0 when there is none) |
| 0x0C | 4 | f32 | centroid primary node weight | |
| 0x10 | 4 | f32 | centroid secondary node weight | primary + secondary = 1 |
| 0x14 | 12 | real_point3d | centroid | model space, WU |
| 0x20 | 12 | block | uncompressed vertices | empty on the disc |
| 0x2C | 12 | block | compressed vertices | empty on the disc |
| 0x38 | 12 | block | triangles | empty on the disc |
| 0x44 | 2 | i16 | index buffer type | 1 (strip) on the disc |
| 0x46 | 2 | | padding | |
| 0x48 | 4 | i32 | index buffer count | strip: number of indices minus 2 |
| 0x4C | 4 | address | index data address | the index data (equals the record's middle word below) |
| 0x50 | 4 | address | index buffer record | address of a 12-byte record in the tag header's index buffer array |
| 0x54 | 2 | i16 | vertex buffer type | 5 (compressed model vertices) on the disc |
| 0x56 | 2 | | padding | |
| 0x58 | 4 | i32 | vertex count | |
| 0x5C | 4 | i32 | vertex offset | 0 on the disc |
| 0x60 | 4 | u32 | (junk) | a console-side value left by the build; not an address in the tag data; ignore |
| 0x64 | 4 | address | vertex buffer record | address of a 12-byte record in the tag header's vertex buffer array |

- The centroid weights are the two largest per-node sums of the
  part's vertex weights, normalised to sum to 1 (checked: the
  Spartan's first part gives node 6 at 0.8182 and node 1 at 0.1818 both
  from its vertices and as stored). The engine uses the centroid,
  transformed by the primary node's matrix, as the sort point of
  transparent parts.
- **Filthy part links** chain transparent parts that must be drawn in a
  fixed order next to each other instead of being sorted separately:
  part a with next = b is drawn just before b. Only
  `weapons\needler\fp\fp` uses them on the multiplayer maps (its part 2,
  glass, has next 3; part 3, the core, has previous 2). The engine
  ignores a link to part 0.
- There is no local-node table on the Xbox: vertex node indices name
  model nodes directly. (The PC format's 0x84-byte part has one; it
  does not occur here.)

## Vertex and index data

### The path from a part to its bytes

1. Read the tag header (`cache_file.md`): model vertex buffer count
   and array address (header offsets 0x10, 0x14), model index buffer
   count and array address (0x18, 0x1C).
2. From the part, take the vertex buffer record address (part +0x64).
   Check `(record - vertex array address)` is a multiple of 12 and the
   quotient is below the vertex buffer count. The record's **middle
   word** (record + 4) is the address of the vertex data in the tag
   data. Read `vertex count (part +0x58) x 32` bytes there.
3. From the part, take the index buffer record address (part +0x50);
   check it the same way against the index buffer array. Its middle
   word is the address of the index data; on the disc it always equals
   the part's own index data address (part +0x4C), which a reader may
   use as a cross-check. Read `(index buffer count + 2) x 2` bytes
   there.
4. The first and third words of each record are console junk
   (0xCACACACA in index records, varying in vertex records); ignore
   them.

On every multiplayer map each part owns exactly one vertex record and
one index record, at the same position in the two arrays, and the
number of records equals the number of parts of all model tags
(bloodgulch: 359 parts, 359 records each). Records are not shared.

### Compressed vertex (32 bytes; every vertex on the disc)

| Offset | Size | Type | Field | Decoding |
|---|---|---|---|---|
| 0x00 | 12 | 3 x f32 | position | model space, WU, not packed |
| 0x0C | 4 | u32 | normal | packed vector, as `structure_bsp.md` "Vertex storage on the Xbox" (11/11/10 bits) |
| 0x10 | 4 | u32 | binormal | same packing |
| 0x14 | 4 | u32 | tangent | same packing |
| 0x18 | 2 | i16 | u | (2u + 1) / 65535 |
| 0x1A | 2 | i16 | v | (2v + 1) / 65535 |
| 0x1C | 1 | i8 | node 0 index x 3 | node 0 = value / 3 |
| 0x1D | 1 | i8 | node 1 index x 3 | node 1 = value / 3; -3 (0xFD) means none |
| 0x1E | 2 | i16 | node 0 weight | w0 = value / 32767; w1 = 1 - w0 |

- The normal, binormal and tangent use exactly the BSP's packing
  (x = bits 0..10, y = bits 11..21, z = bits 22..31, each two's
  complement; component = (2x + 1)/2047, (2y + 1)/2047, (2z + 1)/1023).
  The 16-bit texture coordinates use the BSP's 16-bit coordinate rule.
  Model and BSP vertices differ only in the last 8 bytes: the BSP has
  two f32 texture coordinates there, the model has two i16 texture
  coordinates, two node bytes and a weight.
- Node bytes are signed and always multiples of 3 on the disc (the
  Xbox vertex shader indexes its constant registers, three per node
  matrix). The largest node index on the disc is 40 (a10, a 41-node
  model; the multiplayer maximum is 36, the first-person arms); signed
  bytes allow at most 42.
- When node 1 is none the weight is exactly 32767 on the disc (w0 = 1).
- The weight is a signed 16-bit value over 32767, in 0..32767 on the
  disc. (The reference has two decoders: one reads only the low byte
  of the weight over 255, which gives wrong weights for values such as
  16383; the disc's values only make sense over 32767: 32767 for every
  rigid vertex, 16383 for half and half, and the per-part sums match
  the stored centroid weights.)

### Uncompressed vertex (68 bytes; for completeness, never on the disc)

Position (3 x f32), normal, binormal, tangent (3 x f32 each), u and v
(2 x f32), node 0 and node 1 indices (2 x i16, plain indices, NONE for
none), node 0 and node 1 weights (2 x f32).

### Indices and the strip

- Index data is a list of u16 indices into the part's own vertices,
  `index buffer count + 2` of them. It is one triangle strip. A count
  of 0 or less means no triangles. Every index on the disc is below
  the part's vertex count; no 0xFFFF restart index occurs.
- Strip triangle t (t = 0 .. count - 1) uses indices k[t], k[t+1],
  k[t+2]. A triangle with two equal indices is degenerate (used to join
  strips) and is skipped, but it still counts for the parity below.
  About 48% of strip triangles on the disc are degenerate.
- **Winding.** Taken as (k[t], k[t+1], k[t+2]) for even t and
  (k[t], k[t+2], k[t+1]) for odd t, triangles are **clockwise** seen
  from their front, the side their vertex normals point to, in the
  model's right-handed frame. Checked on every model of bloodgulch:
  70 437 non-degenerate triangles agree with the average of their
  three vertex normals, 91 disagree (0.13%, small folded spots), 68
  are flat to rounding.
- **To a triangle list with counter-clockwise front faces** (the
  common convention for right-handed renderers): for each
  non-degenerate t emit (k[t], k[t+2], k[t+1]) when t is even and
  (k[t], k[t+1], k[t+2]) when t is odd. Back faces are culled unless
  the shader says two-sided (`shaders.md`).

## Texture coordinates

- Decoded u and v (the 16-bit rule above, range -1..1) are multiplied
  by the model's base map u and v scales: u' = u x base u scale,
  v' = v x base v scale. The shader's own map scales then apply on top
  (`shaders.md`). Most multiplayer models have scales 1; 7 to 14 per
  map do not (the Warthog 1.999474 and 3.000816, the Scorpion 4.148820
  and 0.999792, the desert dead tree 96.43 and 54.0). On the disc
  decoded u and v lie in 0..1 apart from a handful of vertices just
  below 0 (bloodgulch: 26 vertices in all).
- The coordinates are in Direct3D's texture convention: (0, 0) is the
  first texel of the bitmap's first row as stored, v grows down the
  rows. An engine that also puts v = 0 at the first stored row (wgpu,
  Vulkan, Direct3D) uses v' unchanged; a tool with v = 0 at the bottom
  (OBJ, OpenGL-style) writes 1 - v'. This is inferred (see Open
  questions).

## Skinning

For a vertex with position p, normal n, nodes a and b and weight w0
(w1 = 1 - w0):

- p_pose = w0 S(a) p + w1 S(b) p, where S(x) = W_pose(x) x Inverse(x)
  (Nodes and the skeleton);
- n_pose = normalise(w0 R_S(a) n + w1 R_S(b) n), R_S the rotation part;
  binormal and tangent likewise;
- when b is none the vertex is rigid: p_pose = S(a) p.

It is a two-bone linear blend. The vertex positions are in model space
in the bind pose, so with every node at its default the drawn shape is
the stored positions (checked: vertices rigidly bound to a node lie on
average 0.05 to 0.12 WU from that node's bind-pose world position on
the Spartan, and 0.005 WU for the rifle's small display nodes; they
would be far off if positions were node-local). For drawing, the
engine multiplies each node's current matrix by its default inverse
matrix once per model and hands those to the vertex shader.

## Shaders

Shader reference, 0x20 bytes:

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 16 | tag reference | shader | any shader class (`soso`, `schi`, `sotr`, `sgla`, `smet`, `spla`, `swat`, `senv`); the stored class is the target's own |
| 0x10 | 2 | i16 | permutation | the shader permutation (which bitmap of a sequence, for example) the part is drawn with; 0 on most, 1 on the assault rifle's second `numbers` reference, 2 on a door blinker |
| 0x12 | 14 | | padding / unused | zero |

A part's shader index selects one of these. The same shader can appear
twice with different permutations (the assault rifle's `numbers`).
`shaders.md` owns the shader classes and what each draws, and which
pass a part is drawn in (opaque first, then alpha-blended decal model
shaders, then transparent shaders sorted by centroid).

## Choosing a level of detail

- The size used is the diameter in pixels of the object's bounding
  sphere on screen: d = (P / max(|z_view|, 0.1)) x 2 r, where r is the
  object's bounding radius, z_view the depth of its centre in view
  space, and P the projection's vertical scale in pixels
  (= the vertical projection scale x viewport height / 2, i.e. half the
  viewport height divided by the tangent of half the vertical field of
  view). The game measured this on its 640 x 480 Xbox screen.
- If d < cutoff[0] the model is not drawn at all (shadows excepted).
- Otherwise the level starts at 4 and steps down while level > 0 and
  d < cutoff[level]: the chosen level is the highest one whose cutoff
  is at most d. The geometry drawn for each region is the region's
  current permutation's geometry index at that level.
- Models with all cutoffs zero are always drawn at level 4. The
  first-person weapon and arms models and the sky are drawn with d = 0
  and have all cutoffs zero, so they always use level 4. Objects in a
  cinematic flagged as stars use an infinite d.
- For our engine at another resolution, scale d by 480 / (our viewport
  height in pixels) before comparing, to keep the Xbox's switching
  distances (inferred).

## Choosing permutations

An object keeps one permutation index per region, for its first
MAX_REGIONS_PER_OBJECT (8) regions; regions beyond that always draw
permutation 0.

**On creation** (each object has a variant number, 0 by default; the
scenario can set it):

1. If the variant is above 0, try "select by variant" with it. If that
   fails, or the variant is 0 or less, go on:
2. Select by variant with "none"; then set the object's variant to the
   variant number of the chosen permutation of the first region whose
   chosen permutation has a non-zero one (0 if none); if that is above
   0, select by variant again with it.

**Select by variant V**, per region:

- The candidates are the permutations not flagged cannot-be-chosen-
  randomly whose variant number equals V, or, when V is none, whose
  variant number is below 100.
- If there are none and V is not none, the candidates are those with
  variant number 0 (same flag rule).
- No candidates: the region gets permutation 0 and the selection is
  reported as failed. One candidate: it is taken without drawing a
  random number. Several: one is picked uniformly with the game's
  random number generator (range [0, count)), drawing one number per
  such region in region order.

On the multiplayer maps every region of the multiplayer objects has
exactly one candidate (`__base` or another single name), so no random
numbers are drawn for them.

**By name.** A permutation can be set by name for one region or for
every region: the first permutation whose name matches
(case-insensitive) becomes the region's current one when switching on,
and permutation 0 when switching off. The game uses this for:

- `~damaged`, when a damage region is destroyed (the collision model's
  region of the same index; `health_shields.md` owns when);
- `~blur` on every region of a vehicle when its speed reaches the
  vehicle tag's blur speed, and back to 0 below it;
- `~primary-blur` / `~secondary-blur` on a weapon when a trigger's rate
  of fire exceeds its blurred rate of fire;
- regions of the collision model flagged "missing when shield is zero"
  with more than one permutation are switched between permutation 0
  (shield up) and 1 (shield down) directly, by index.

## Which models the game draws where

| Use | Model (bloodgulch tag paths) | How it is drawn |
|---|---|---|
| Multiplayer Spartan, third person | `characters\cyborg\cyborg` (referenced by the `bipd` `characters\cyborg_mp\cyborg_mp`, and by the campaign cyborg) | skinned by its animation graph; LOD by size |
| The player's arms in first person | `characters\cyborg\fp\fp` (37 nodes: `frame bone24`, arms, wrists, five fingers in three joints each side) | posed by the held weapon's first-person animations through a node-name remapping; level 4 |
| A weapon in first person | `weapons\<name>\fp\fp` | same animations; level 4 |
| A weapon in third person and on the ground | `weapons\<name>\<name>` | attached to the holder's hand marker; LOD by size |
| Vehicles, scenery, powerups | the object's model | LOD by size |
| The sky | the `sky ` tag's model (bloodgulch: `sky\mp clear afternoon\mp clear afternoon`) | one node; every region drawn; node matrices scaled by 1/1024 and placed at camera position x 1023/1024, so the sky stays around the camera; level 4 |

The first-person weapon and arms are drawn as separate models sharing
one set of animated node matrices; `animations.md` owns the remapping.
The sky model is authored about 1024 times larger than the world
(its bounding box spans about 160 000 units).

## Measured facts

All from bloodgulch (identical on every multiplayer map: the shared
tags are byte-identical apart from addresses). "Highest detail" is
level 4 of each region's first permutation. Triangle counts exclude
degenerate strip triangles. Bounding boxes are of those vertices'
stored positions (the bind pose), in WU.

### Multiplayer Spartan: `characters\cyborg\cyborg`

- 19 nodes: `bip01 pelvis`, `bip01 l thigh`, `bip01 r thigh`,
  `bip01 spine`, `bip01 l calf`, `bip01 r calf`, `bip01 spine1`,
  `bip01 l clavicle`, `bip01 l foot`, `bip01 neck`, `bip01 r clavicle`,
  `bip01 r foot`, `bip01 head`, `bip01 l upperarm`, `bip01 r upperarm`,
  `bip01 l forearm`, `bip01 r forearm`, `bip01 l hand`, `bip01 r hand`.
- Cutoffs 5, 50, 120, 250, 400; node counts 18 x 5; uv scales 1, 1;
  node list checksum 0x6CFA6498.
- One region `cyborg`, one permutation `__base` (flags 0, geometries
  4, 3, 2, 1, 0). Five geometries; vertex totals 2256, 1686, 940, 636,
  507 for geometries 0..4 (levels 4..0).
- Shaders: 0 `characters\cyborg\shaders\light shield` (`schi`),
  1 `characters\cyborg\shaders\light shield generator` (`schi`),
  2 `characters\cyborg\shaders\armor` (`soso`),
  3 `characters\cyborg\shaders\visor` (`soso`).
- Highest detail (geometry 0), 4 parts:

| Part | Shader | Vertices | Strip indices (count field) | Triangles | Centroid nodes, weight |
|---|---|---|---|---|---|
| 0 | 0 | 60 | 88 (86) | 30 | 6, 1, 0.8182 |
| 1 | 1 | 4 | 4 (2) | 2 | 3, 0, 1.0 |
| 2 | 2 | 2123 | 3653 (3651) | 1880 | 6, 12, 0.6385 |
| 3 | 3 | 69 | 122 (120) | 104 | 12, 0, 1.0 |
| total | | 2256 | 3867 | 2016 | |

- Bounding box: x -0.1046..0.0796, y -0.1491..0.1491, z 0.0..0.7036
  (0.70 WU tall = 2.14 m, matching the biped's 0.7 WU collision
  height).
- 8 markers (see Markers).

### Assault rifle

`weapons\assault rifle\assault rifle` (third person): 3 nodes
(`frame gun`, `frame display`, `frame ophandle`); cutoffs 5, 10, 20, 40,
80; node counts 0, 0, 0, 0, 2; one region `__unnamed`, permutation
`__base`, geometries 2, 2, 2, 1, 0; geometry vertex totals 819, 198, 58.
9 shaders: `weapons\assault rifle\shaders\ar lod` (`soso`), and from
`weapons\assault rifle\fp\shaders\`: `grip`, `gun`, `display` (`soso`),
`compass`, `numbers`, `ar data`, `numbers` with permutation 1 (`schi`),
then `weapons\assault rifle\shaders\bolt` (`sotr`). Highest detail:
8 parts (shaders 1..8; vertices 376, 408, 9, 6, 4, 6, 4, 6); **819
vertices, 1371 strip indices, 764 triangles**. Bounding box x
-0.0895..0.1991, y -0.0147..0.0147, z -0.0252..0.0821.

`weapons\assault rifle\fp\fp` (first person): 5 nodes (`frame gun`,
`frame display`, `frame magazine`, `frame ophandle`, `frame safety
button`); cutoffs all 0; one region, permutation `assault rifle fp`,
one geometry of 8 parts (vertices 666, 989, 6, 6, 4, 4, 21, 17);
1713 vertices, 3116 strip indices, 1964 triangles; 8 shaders (`grip`,
`gun`, `display` `soso`; `compass`, `ar data`, `numbers` (perm 1),
`numbers`, `icons` `schi`). Bounding box x -0.0895..0.1991, y
-0.0148..0.016, z -0.0252..0.0821.

### Pistol

Third person `weapons\pistol\pistol`: 7 nodes (`frame gun`, `frame
magazine`, `frame slide`, `frame slide_release`, `frame trigger`,
`frame bullet`, `frame chamber_button`); cutoffs 0, 9, 18, 35, 70;
permutation `__base` geometries 2, 2, 1, 1, 0; highest detail 2 parts
(831 + 202 vertices), 1033 vertices, 787 triangles; shaders `p lod`,
`fp\shaders\silver`, `fp\shaders\black` (all `soso`). Bounding box x
-0.0372..0.0507, y -0.0092..0.0092, z -0.04..0.0512.

First person `weapons\pistol\fp\fp`: the same 7 nodes; permutation
`pistol fp`; 3 parts (1283, 317, 128 vertices), 1728 vertices, 1558
triangles; shaders `silver`, `black`, `bullet` (`soso`). Bounding box x
-0.0372..0.0507, y -0.0092..0.0092, z -0.0532..0.0379.

### Plasma rifle

Third person `weapons\plasma rifle\plasma rifle`: 5 nodes (`frame gun`,
`frame rod_left`, `frame rod_right`, `frame wing_left`, `frame
wing_right`); cutoffs 0, 20, 40, 80, 300; geometries 3, 3, 2, 1, 0;
highest detail 5 parts (19, 510, 493, 40, 8 vertices), 1070 vertices,
1029 triangles; shaders `pr lod` (`soso`), `gauge` (`smet`), `dull`,
`gun`, `luminous`, `amber` (`soso`). Bounding box x -0.1182..0.0969,
y -0.0297..0.0297, z -0.069..0.0557.

First person `weapons\plasma rifle\fp\fp`: the same 5 nodes and node
list checksum; permutation `plasma rifle fp`; 5 parts (618, 18, 1208,
94, 19 vertices), 1957 vertices, 2259 triangles. Bounding box x
-0.1199..0.0969, y -0.0297..0.0297, z -0.069..0.0559.

### Shotgun, first person

`weapons\shotgun\fp\fp`: 4 nodes (`frame gun`, `frame bullet`, `frame
magazine`, `frame pump`; node list checksum 0x4011D166); cutoffs all
0; one region (unnamed), permutation `shotgun fp`; one geometry of 3
parts (1276, 4, 112 vertices); **1392 vertices, 1516 triangles**;
shaders `weapons\shotgun\shaders\shotgun gun` (`soso`), `shotgun
primary` (`sotr`), `shotgun shell` (`soso`). Bounding box x
-0.2374..0.2008, y -0.0167..0.0167, z -0.0641..0.0284. Markers (one
instance each): `flashlight`, `left hand cyborg`, `left hand marine`,
`primary ejection`, `primary trigger`. Identical on all 13 multiplayer
maps.

### The other first-person weapon models

Every held weapon of the multiplayer maps (the eleven: assault rifle,
pistol, shotgun, plasma rifle, sniper rifle, plasma pistol, needler,
rocket launcher, flamethrower, ball, flag) has a `weapons\<name>\fp\fp`
model, each identical on all 13 multiplayer maps, each with all cutoffs
0, one region and one permutation, one geometry. The assault rifle,
pistol, shotgun and plasma rifle are above; the others:

| Model | Nodes | Parts (vertices) | Vertices | Triangles | Shaders (class) | Bounding box x, y, z | Markers |
|---|---|---|---|---|---|---|---|
| sniper rifle | 4: `frame gun`, `frame magazine`, `frame ophandle`, `frame bullet` | 6 (1952, 6, 292, 14, 4, 6) | 2274 | 2236 | metal `soso`, primary `sotr`, grip `soso`, decal `soso`, subscreen `schi`, screen `schi` | -0.1736..0.4455, -0.0389..0.0389, -0.026..0.0812 | `flashlight`, `left hand cyborg`, `left hand marine`, `primary ejection`, `primary trigger` |
| plasma pistol | 4: `frame gun`, `frame cover`, `frame dial`, `frame face` | 4 (485, 389, 104, 4) | 982 | 1030 | shiney, dull, luminous `soso`; gauge `smet` | -0.0215..0.0907, -0.0105..0.0105, -0.0429..0.0477 | `flashlight`, `left hand cyborg`, `primary trigger`, `primary trigger01`, `secondary trigger`, `secondary trigger flare1`, `secondary trigger flare2`, `vent_front`, `vent_mid`, `vent_rear` |
| needler | 17: `frame body`, `frame eneedle13`, `frame needle01` to `needle12`, `needle14` to `needle16` | 4 (1273, 357, 672, 64) | 2366 | 1774 | gun, dull `soso`; luminous `sgla`; core `sotr` | -0.1227..0.0978, -0.0351..0.0368, -0.0954..0.1044 | `flashlight`, `primary trigger`, `smoke` |
| rocket launcher | 5: `frame gun`, `frame body`, `frame foregrip`, `frame tubes`, `frame push_bar` | 4 (372, 1192, 1482, 34) | 3080 | 2176 | grip, body, tubes, decal (all `soso`) | -0.2821..0.164, -0.0408..0.0295, -0.033..0.1028 | `flashlight`, `left hand cyborg`, `left hand marine`, `primary ejection`, `primary trigger` |
| flamethrower | 3: `frame gun`, `frame ammo`, `frame cover` | 6 (2070, 98, 4, 4, 4, 4) | 2184 | 2174 | `ft gun`, `ft ammo`, `ft decals` `soso`; `ft hundreds`, `ft ones`, `ft tens` `smet` | -0.0283..0.449, -0.0782..0.0538, -0.1131..0.0765 | `cyborg left hand`, `flashlight`, `marine left hand`, `pilot`, `primary trigger` |
| ball | 1: `frame skull` | 1 (1239) | 1239 | 1792 | `weapons\ball\shaders\bone` `soso` | -0.0428..0.0391, -0.03..0.0292, -0.0079..0.0568 | none |
| flag | 1: `frame pole` | 1 (143) | 143 | 240 | `weapons\flag\fp\shaders\flag fp` `soso` | -0.0075..0.0075, -0.0075..0.0075, 0..0.5 | `flag bottom`, `flag top`, `left hand` |

The plasma rifle's marker list: `flashlight`, `ground point`, `heat
flare`, `heat flare1`, `left hand`, `primary trigger`, `primary
trigger1`, `secondary trigger`, `vent` (8 instances); the assault
rifle's: `flashlight`, `left hand cyborg`, `left hand marine`, `primary
ejection`, `primary trigger`, `smoke`; the pistol's: `flashlight`,
`laser`, `left hand cyborg`, `left hand marine`, `primary ejection`,
`primary trigger`. The permutation names are `<weapon> fp` for the
assault rifle, pistol, shotgun, plasma rifle, plasma pistol and needler,
`rocketlauncher` for the rocket launcher, `gameball` for the ball,
`__base` for the sniper rifle, flamethrower and flag. The node list
checksums (first-person model): needler 0xDD4D9E68, plasma pistol
0x000AED50, rocket launcher 0xC0467E26, sniper rifle 0x602DB6D5,
flamethrower 0x400542E7, ball 0x00033E60, flag 0x00019ED9.

### First-person arms: `characters\cyborg\fp\fp`

37 nodes; one region `cyborg`, permutation `__base`; one geometry of 2
parts (`rubber hands` 866 vertices, `armor hands` 574, both `soso`);
1440 vertices, 1612 triangles. Bounding box x -0.002..0.2907, y
-0.1275..0.1275, z -0.0336..0.043.

### Warthog: `vehicles\warthog\warthog`

18 nodes (`frame hull`, `frame gun mount base`, four suspensions,
`frame steering wheel`, `frame chain`, `frame gun`, four engines,
`frame barrels`, four tires); cutoffs 0, 50, 80, 120, 250; uv scales
1.999474, 3.000816; 20 geometries. Regions: `barrels` (`__base` 9, 9,
6, 3, 0; `~primary-blur` flags 1, 9, 19, 18, 17, 16), `hull`
(`__base` 10, 10, 7, 4, 1), `tire` (`__base` 11, 11, 8, 5, 2; `~blur`
flags 1, 11, 15, 14, 13, 12). Highest detail: barrels 1 part (205
vertices), hull 12 parts (2853), tire 1 part (632, all four tires in
one part); 3690 vertices, 2798 triangles. 14 shaders: `soso` tires,
hull, chain gun, decal buttons, decal sensor; `schi` lights, decal
speedometer, decal velocity, meter sensor, chain gun belt ammo,
chain gun_blur; `sgla` windshield; `smet` meter engine, meter
speedometer. Bounding box x -0.9477..0.9677, y -0.4875..0.4875, z
-0.01..1.0253.

### Sky: `sky\mp clear afternoon\mp clear afternoon` (bloodgulch)

1 node `frame sky` (identity); five regions `00.sky`, `14.threshold`,
`15.basis`, `20.ring`, `31.clouds`, each one `__base` permutation
naming geometry 0..4 at every level; one part each (37, 4, 4, 222, 29
vertices; 60, 2, 2, 86, 33 triangles); shaders `sky clear blue` and
`clouds puffy light` (`schi`), `threshold clear afternoon`, `basis clear
afternoon`, `ring clear afternoon` (`sotr`). Bounding box x
-71793.6..55507.6, y -53461.3..86000.4, z -0.001..98293.1 (x -70..54,
y -52..84, z 0..96 WU after the 1/1024 sky scale).

### Model tags per multiplayer map

| Map | `mode` tags | Parts (= buffer records) | Largest model (highest detail, all regions) | Largest part |
|---|---|---|---|---|
| beavercreek | 53 | 341 | Warthog, 3690 vertices | Spartan part 2, 2123 |
| bloodgulch | 63 | 359 | Warthog, 3690 | Spartan part 2, 2123 |
| boardingaction | 51 | 336 | Warthog, 3690 | 2123 |
| carousel | 50 | 328 | Warthog, 3690 | 2123 |
| chillout | 47 | 324 | Warthog, 3690 | 2123 |
| damnation | 52 | 335 | Warthog, 3690 | 2123 |
| hangemhigh | 46 | 323 | Warthog, 3690 | 2123 |
| longest | 46 | 324 | Warthog, 3690 | 2123 |
| prisoner | 45 | 322 | Warthog, 3690 | 2123 |
| putput | 45 | 318 | Warthog, 3690 | 2123 |
| ratrace | 47 | 326 | Warthog, 3690 | 2123 |
| sidewinder | 56 | 351 | Warthog, 3690 | 2123 |
| wizard | 48 | 329 | Warthog, 3690 | 2123 |

The largest node count on any multiplayer map is 37 (the first-person
arms). For comparison a10 has 76 model tags, 623 parts, the largest
model `vehicles\lifepod\lifepod` (10 613 vertices, a 4118-vertex part)
and a 41-node model.

## Verification done

Throwaway scripts decoded the models and checked, on the disc:

- every model of the 13 multiplayer maps: layouts above, every block
  in range, every part's buffer records inside the arrays, index data
  address equal to the record's, every strip index below the vertex
  count, every node byte a multiple of 3 naming an existing node (or
  -3 for none), weights in 0..32767 and exactly 32767 when rigid,
  marker names sorted, node counts equal to the highest node index
  used per level, all unused words zero;
- the Spartan, the assault rifle (third and first person) written as
  OBJ files: positions form a 0.70 WU tall figure and a 0.29 WU long
  rifle; normals have length 0.9989..1.0011 (every model of
  bloodgulch);
- the quaternion convention three ways: the stored inverse matrices
  (match to 3e-6 only with the game convention), the skeleton (left
  limbs at +Y, head above the pelvis at 0.609 vs 0.394, hands 0.27
  apart, every child at its stored distance from its parent), and the
  markers (feet, head and seat markers face +X only with the game
  convention);
- the winding against vertex normals on every model of bloodgulch
  (99.87% of non-degenerate triangles);
- the weight decoding against the stored centroid weights.

Not checked against the game running: how the textures look (the v
direction), and the level-of-detail switching distances.

## Edge cases

- **Empty blocks**: a model with no geometries, regions or nodes draws
  nothing; with no nodes, skinning uses the object frame alone.
- **Indices out of range** (shader, geometry, node, permutation, marker
  region or node): the engine skips the part, region or marker as
  damage. A reader reports and skips, never reads wild.
- **Bad buffer record** (not inside the arrays, not a multiple of 12
  from the start, or its data outside the tag data): the part is not
  drawn.
- **Strip count 0 or less**: no triangles.
- **Node byte not a multiple of 3, or above 3 x (node count - 1)**:
  damage; treat the vertex as rigid to node 0.
- **Weight above 32767 or below 0**: clamp w0 to 0..1 (none on the
  disc).
- **Cutoff[0] above zero and a tiny object**: not drawn at all.
- **A permutation's geometry index NONE**: nothing drawn for that
  region at that level.
- **More than 8 regions**: only the first 8 get chosen permutations;
  the rest draw permutation 0.
- **Base map scale 0**: never on the disc; treat as given (u' = 0).

## Quirks

- The quaternion convention is the transpose of the usual one. Keep the
  data as stored and convert once on load (negate i, j, k) if the
  engine's quaternion type uses the usual convention; then animation,
  node and marker data must all be converted the same way.
- Node "counts" store the highest node index used, not a count.
- The part's centroid secondary node is 0, not NONE, when a part uses
  one node.
- The first-person and third-person weapon models are separate tags
  with separate geometry; the third-person rifle even references
  first-person shaders.
- Every multiplayer object's model has a single permutation per region
  except the vehicles' `~` states, so object variety in multiplayer
  comes from change colours (objects spec), not permutations.

## Test cases

### (a) Synthetic model (unit tests)

Start from the synthetic cache file of `cache_file.md` test (a) (base
B = 0x803A6000), with the tag data grown to 0x700 bytes and tag index
entry 1 changed to class `mode` (0x6D6F6465, both parents NONE) with
its struct at B+0x200 (keep its id and path). Then write these bytes
(little-endian; zero where not given). Float bit patterns: 0.70710677
= 0x3F3504F3, 2.0 = 0x40000000, 1.0 = 0x3F800000, 0.5 = 0x3F000000,
0.75 = 0x3F400000, 0.25 = 0x3E800000.

Tag header and buffers:

| Address | Contents |
|---|---|
| B+0x10 | 1 (vertex buffer count) |
| B+0x14 | B+0x100 (vertex buffer array) |
| B+0x18 | 1 (index buffer count) |
| B+0x1C | B+0x10C (index buffer array) |
| B+0x100 | vertex record: 0xCACACACA, B+0x120, 0xCACACACA |
| B+0x10C | index record: 0xCACACACA, B+0x1A0, 0xCACACACA |
| B+0x120 | 4 vertices of 32 bytes (below) |
| B+0x1A0 | indices u16 0, 1, 2, 3 |

Model struct at B+0x200: flags 0; checksum 0x12345678; cutoffs 1, 10,
20, 40, 80; node counts 1 x 5; u scale 2.0, v scale 1.0; markers (1,
B+0x300); nodes (2, B+0x360); regions (1, B+0x4A0); geometries (1,
B+0x550); shaders (1, B+0x5F0).

| Address | Element | Fields |
|---|---|---|
| B+0x300 | marker 0 | name `primary trigger`; instances (1, B+0x340) |
| B+0x340 | instance 0 | region 0, permutation 0, node 1, pad 0; translation (0.5, 0, 0); rotation (0, 0, 0.70710677, 0.70710677) |
| B+0x360 | node 0 | name `root`; sibling -1, child 1, parent -1; translation (0, 0, 1); rotation (0, 0, 0.70710677, 0.70710677); distance 0; inverse: scale 1, forward (0, 1, 0), left (-1, 0, 0), up (0, 0, 1), position (0, 0, -1) |
| B+0x3FC | node 1 | name `tip`; sibling -1, child -1, parent 0; translation (2, 0, 0); rotation (0, 0, 0, 1); distance 2.0; inverse: scale 1, forward (0, 1, 0), left (-1, 0, 0), up (0, 0, 1), position (-2, 0, -1) |
| B+0x4A0 | region 0 | name `body`; permutations (1, B+0x4F0) |
| B+0x4F0 | permutation 0 | name `__base`; flags 0; variant 0; geometry indices 0, 0, 0, 0, 0 |
| B+0x550 | geometry 0 | parts (1, B+0x580) |
| B+0x580 | part 0 | flags 0; shader 0; previous -1, next -1; centroid nodes 0, 1; weights 0.75, 0.25; centroid (0.5, 0.5, 1); index type 1, count 2, index address B+0x1A0, index record B+0x10C; vertex type 5, count 4, offset 0, junk 0, vertex record B+0x100 |
| B+0x5F0 | shader 0 | null tag reference of class `soso` (path 0, id NONE); permutation 0 |

Vertices (position; then normal 0x7FC00000, binormal 0x001FF800,
tangent 0x000003FF for all four; u, v; node bytes; weight):

| # | Position | u, v (i16) | Nodes | Weight (i16) |
|---|---|---|---|---|
| 0 | (0, 0, 1) | 0, 0 | 0x00, 0xFD | 32767 |
| 1 | (0, 1, 1) | 0, 32767 | 0x00, 0xFD | 32767 |
| 2 | (1, 0, 1) | 32767, 0 | 0x03, 0xFD | 32767 |
| 3 | (1, 1, 1) | 32767, 32767 | 0x00, 0x03 | 24575 |

Expected:

| Check | Expected |
|---|---|
| model | 2 nodes, 1 marker, 1 region with 1 permutation, 1 geometry with 1 part, 1 shader (null, class `soso`) |
| decoded normal (all) | (1/2047, 1/2047, 1) = (0.00048852, 0.00048852, 1.0) |
| decoded binormal | (0.00048852, 1.0, 0.00097752) |
| decoded tangent | (1.0, 0.00048852, 0.00097752) |
| vertex 0 texcoord after base scale | (2 x 1/65535, 1/65535) = (0.0000305, 0.0000153) |
| vertex 3 texcoord after base scale | (2.0, 1.0) |
| vertex 0 skin | rigid, node 0, w0 = 1 |
| vertex 2 skin | rigid, node 1, w0 = 1 |
| vertex 3 skin | nodes 0 and 1, w0 = 24575/32767 = 0.7499924 (0.74999237), w1 = 1 - w0 = 0.2500076 |
| strip | 4 indices (count 2 + 2); triangles in strip order (0, 1, 2), (1, 3, 2), both clockwise seen from +Z (the normals' side) |
| triangle list, counter-clockwise front | (0, 2, 1), (1, 2, 3) |
| node 0 rotation matrix (forward, left, up columns) | forward (0, -1, 0), left (1, 0, 0), up (0, 0, 1) |
| node 0 world | rotation as above, position (0, 0, 1) |
| node 1 world | same rotation, position (0, -2, 1); distance to node 0 = 2.0 |
| inverse matrices | equal to the stored ones (within 1e-6) |
| bind-pose skin matrices | identity for both nodes; skinned positions equal stored positions |
| pose: node 1 translation (3, 0, 0), node 0 unchanged | node 1 world position (0, -3, 1); skin(1) = translation by (0, -1, 0); skinned v2 = (1, -1, 1); v3 = (1, 0.7499924, 1); v0, v1 unchanged |
| marker `primary trigger` | one instance; world position (0, -2.5, 1); forward (-1, 0, 0), left (0, -1, 0), up (0, 0, 1) |
| marker lookup `PRIMARY TRIGGER` | found (case-insensitive) |
| LOD at d = 1000 | level 4 |
| LOD at d = 15 | level 1 |
| LOD at d = 20 | level 2 |
| LOD at d = 0.5 | not drawn |
| corrupt: vertex record B+0x104 | rejected (not a multiple of 12 from the array) |
| corrupt: vertex 2 node byte 0x04 | rejected or rigid to node 0, reported |
| corrupt: index 3 replaced by 9 | rejected (index beyond vertex count) |
| corrupt: node 1 parent 1 | rejected (parent not below its own index) |

**Quaternion example.** Stored (0.5, 0.5, 0.5, 0.5) must give forward
(0, 0, 1), left (1, 0, 0), up (0, 1, 0), i.e. the matrix with rows
(0, 1, 0), (0, 0, 1), (1, 0, 0) acting on column vectors: (1, 0, 0)
goes to (0, 0, 1). The usual convention would send (1, 0, 0) to
(0, 1, 0); a reader producing that is wrong. This is the Spartan
pelvis's rotation: its x axis points up the spine.

Stored (0, 0, 0.70710677, 0.70710677) must give forward (0, -1, 0),
left (1, 0, 0), up (0, 0, 1).

### (b) Disc-backed (slow tests; need the user's own disc files)

bloodgulch.map:

| Check | Expected |
|---|---|
| `mode` tags | 63; parts 359; vertex and index buffer records 359 each |
| `characters\cyborg\cyborg` | tag 151, struct 0x80729B64; flags 0; checksum 0x6CFA6498; cutoffs 5, 50, 120, 250, 400; node counts 18 x 5; uv scales 1, 1; blocks: 8 markers, 19 nodes, 1 region, 5 geometries, 4 shaders |
| Spartan region / permutation | `cyborg` / `__base`, flags 0, geometries 4, 3, 2, 1, 0 |
| Spartan geometry 0 | 4 parts; vertices 60, 4, 2123, 69 (2256); strip indices 88, 4, 3653, 122 (3867); non-degenerate triangles 30, 2, 1880, 104 (2016) |
| Spartan node 0 | `bip01 pelvis`, links -1/2/-1, translation (-0.0082, 0, 0.3944), rotation (0.5, 0.5, 0.5, 0.5) to 4 decimals |
| Spartan world positions (game convention) | `bip01 l thigh` y = +0.0422; `bip01 head` (-0.007, 0, 0.609); `bip01 l hand` (0.009, 0.133, 0.352) |
| Spartan bind bounding box | x -0.1046..0.0796, y -0.1491..0.1491, z 0.0..0.7036 |
| Spartan geometry 0 part 0 | shader 0, links -1/-1, centroid nodes 6 and 1, weights 0.8182 and 0.1818; index count field 86, index data 0x80688FF8, index record 0x8064952C (array index 313); vertex count 60, vertex record 0x803C7220 (array index 313), vertex data 0x8062B260 |
| its first vertex (raw) | 50 FA C2 BB C0 0F 04 3E 64 5B 08 3F CF CF DE 21 7F 98 C8 85 09 9C FF EE 64 70 30 04 27 FD FF 7F |
| its first vertex (decoded) | position (-0.0059502, 0.1289663, 0.5326445); normal (-0.047386, 0.962872, 0.264907); u, v raw 28772, 1072 = (0.878080, 0.032731); node 13 (`bip01 l upperarm`, byte 39), no second node, w0 = 1 |
| its first strip indices | 0, 1, 2 (then 3, 3, 4) |
| `weapons\assault rifle\assault rifle` | 3 nodes, 9 markers, 1 region (`__unnamed` / `__base`, geometries 2, 2, 2, 1, 0), 3 geometries, 9 shaders; cutoffs 5, 10, 20, 40, 80; node counts 0, 0, 0, 0, 2 |
| assault rifle geometry 0 | 8 parts; 819 vertices; 1371 strip indices; 764 triangles; bounding box x -0.0895..0.1991, y -0.0147..0.0147, z -0.0252..0.0821 |
| assault rifle `primary trigger` | node 0, translation (0.2197, 0, 0.0525), rotation identity |
| `weapons\assault rifle\fp\fp` | 5 nodes, 1 geometry of 8 parts, 1713 vertices, 1964 triangles |
| winding over all models | at least 99.5% of non-degenerate triangles clockwise against their vertex normals' side |
| every multiplayer map | the per-map table above |

## Confidence per section

| Section | Confidence |
|---|---|
| Model struct layout, block element sizes | checked against the disc (all 13 multiplayer maps and a10); field names and flag meanings from the reference |
| Markers: layout, sorted order, names | checked against the disc; lookup rules (permutation match, mirroring) from the reference; marker uses from the reference and the names |
| Nodes: layout, order, links, distance, inverse matrix | checked against the disc |
| Quaternion convention | checked against the disc three ways (inverse matrices, skeleton, markers), and matches the reference's matrix formula |
| Regions, permutations, geometry indices per level | checked against the disc; level order (index 4 highest) from the reference's selection loop and confirmed by vertex counts on the disc |
| Permutation markers form | reference only (never on the disc) |
| Parts: layout, buffer path, records | checked against the disc; meaning of the junk word inferred |
| Compressed vertex layout and decoding | checked against the disc (node bytes, weights, packing, unit normals) |
| Uncompressed vertex, triangle-list index type | reference only |
| Strip rules and winding | checked against the disc (vertex normals) |
| Texture coordinate scale | checked (values) and reference (use); v direction inferred |
| Skinning formula | reference; model-space vertices checked on the disc |
| LOD selection | reference only; cutoffs and geometry order checked on the disc |
| Permutation choice, `~` names | reference only; flags, names and variant numbers checked on the disc |
| Sky scale and placement | reference only; the sky's size checked on the disc |
| Measured facts, per-map table | checked against the disc |

## Open questions

1. **v direction.** The texture coordinates are taken to follow
   Direct3D (v = 0 at the bitmap's first stored row). Check by drawing
   the assault rifle's ammo counter quad (`numbers` part, 4 vertices)
   or the Warthog's speedometer decal with the bitmaps spec's decoder:
   the digits must read the right way up.
2. **Node counts field.** The disc stores the highest node index used
   per level; whether the engine reads it (for example to size the
   vertex shader's matrix upload) was not found in the reference.
   Nothing here depends on it.
3. **Vertex buffer word at part +0x60.** A console-side value (it looks
   like a physical memory address); ignored. Confirm nothing reads it.
4. **Model flags.** Bit 0 on `characters\elite\elite` (a10) only; it is
   a tool-time normal-blending option and changes nothing at run time
   as far as the reference shows.
5. **Level-of-detail distances at other resolutions.** The game
   measured pixels on a 480-line screen; scaling by 480 / our height is
   our choice to keep the Xbox's switching distances. Compare against
   captures of the real game if LOD popping looks wrong.
6. **Weight decoding in the reference.** One of its decoders reads the
   weight's low byte over 255; the disc's data rules it out (see
   Compressed vertex). If a capture of the game ever disagrees, revisit.
7. **The editing tools' names for the five levels.** This spec numbers
   levels by the engine's use (0 lowest). The tools may list the five
   cutoff and geometry fields in the opposite order with names such as
   "super high" first; only the engine's use matters for us.
8. **Blur and damage switching** details (thresholds, timing, what
   happens to a destroyed region's permutation on respawn) belong to
   the vehicle, weapon and damage specs; only the names and the
   by-name rule are fixed here.
