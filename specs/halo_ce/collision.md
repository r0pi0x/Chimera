# Halo: Combat Evolved: collision

Source basis: the game's collision geometry code (the collision BSP, its
point, line, sphere and pill queries, the feature lists the biped mover
uses, the collision-model tag) read in a decompilation of an Xbox build,
and every layout and number checked against the Xbox disc's own map
files (`bloodgulch`, `hangemhigh`, `boardingaction`, `prisoner`, and a
flag survey of every campaign and multiplayer map).
Status: draft

## Summary

Every solid thing in Halo is a collision BSP: a 3D BSP tree whose leaves
are open space and whose missing children are solid, plus a winged-edge
mesh of convex polygons ("surfaces") lying on the tree's planes. A level
has one, in its structure BSP tag; an object's collision model has one
per node and per permutation, placed by the object's skeleton. A line
trace walks the tree front to back and reports the first boundary it
crosses between open and solid space; the biped mover asks the tree for
the surfaces, edges and vertices near a sphere and slides a point
through their Minkowski sums with the biped's pill. For a first playable,
the level's surfaces triangulated into a static triangle mesh with one
material per triangle carry everything needed; the few flags
(two-sided, invisible, climbable, breakable) are listed per surface and
are absent from Blood Gulch and Hang 'Em High altogether.

## Units and conventions

Distance unit, axes (right-handed, z up), angles, the 30 Hz tick, and
how tag blocks, tag references and addresses are stored are defined in
`cache_file.md`. Where the level's collision BSP and collision
materials sit inside the structure BSP tag is in `structure_bsp.md`
(checked here: the collision materials block is the field at structure
offset 0xA4, the collision BSP block, always one element, at 0xB0).

Conventions used throughout this spec:

- A plane is four 32-bit floats (i, j, k, d): unit normal n = (i, j, k)
  and distance d. The signed distance of a point p is
  dist(p) = n·p − d. The front of a plane is dist ≥ 0, the back
  dist < 0 (a point exactly on a plane is in front).
- A signed plane index is a 32-bit integer whose low 31 bits index the
  planes block and whose top bit (0x80000000) says the plane is used
  flipped: normal −n and distance −d.
- A child index (3D or 2D tree) is a 32-bit integer. Values 0 to
  0x7FFFFFFF are tree nodes. A value with the top bit set is a
  terminal: −1 (0xFFFFFFFF) is "none", anything else is the terminal's
  index in its low 31 bits (a leaf in the 3D tree, a surface in a 2D
  tree). In the 3D tree "none" is solid space.
- Projection to 2D. For a normal n, the projection axis is z if
  |n_z| ≥ |n_y| and |n_z| ≥ |n_x|; else y if |n_y| ≥ |n_x|; else x. A
  projection sign s (true or false) picks the 2D coordinates (u, v):

| Axis | s false | s true |
|---|---|---|
| x | (z, y) | (y, z) |
| y | (x, z) | (z, x) |
| z | (y, x) | (x, y) |

  With s true the 2D frame is the 3D one seen from the positive end of
  the axis, so a polygon that is counter-clockwise seen from the side
  its normal points to stays counter-clockwise in 2D when s is chosen
  as "the normal's component along the axis is positive".

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| max surface vertices | 8 | count | most edges/vertices a surface ring has (disc: 3 to 8) |
| max 3D tree depth | 128 | nodes | deepest walk the queries allow; disc max is 39 (bloodgulch), 22 (hangemhigh) |
| max 2D tree depth | none | | the game has no limit; disc max is 4 to 5 |
| block maximum: bsp3d nodes | 131072 | count | tag definition limit |
| block maximum: planes | 65536 | count | |
| block maximum: leaves | 65536 | count | |
| block maximum: bsp2d references | 131072 | count | |
| block maximum: bsp2d nodes | 65535 | count | |
| block maximum: surfaces | 131072 | count | |
| block maximum: edges | 262144 | count | |
| block maximum: vertices | 131072 | count | |
| query list size | 256 | count | most leaves, surfaces, edges, vertices a sphere query or a trace records |
| features per query | 256 | count | most spheres, cylinders and prisms (each) in a feature list |
| sphere query padding | 0.0625 | world units | added to the radius when the mover gathers features |
| general epsilon | 0.0001 | | "nearly zero" in the feature tests and edge convexity |
| line-to-leaves split epsilon | 1/4096 | world units | clipping a segment into leaves (not used by the trace) |
| BSP pill test 3D slack | 1/4096 | world units | |
| BSP pill test 2D slack | 1/8192 | world units | |
| keep-location nudge | 1/4096 | world units | trace hit point pushed off the surface |
| keep-location fallback step | 1/32 | parameter | used when the segment is parallel to the hit plane |
| breakable surfaces per map | 256 | count | size of the intact-surface bit vector |
| material types | 33 | count | codes 0 to 32 (table below) |

## Behavior

### The collision BSP block layout

A collision BSP is a 0x60-byte struct of eight tag blocks, in this
order (each block field is 12 bytes, as `cache_file.md` defines):

| Offset | Block (tag field name) | Element size |
|---|---|---|
| 0x00 | bsp3d nodes | 0x0C |
| 0x0C | planes | 0x10 |
| 0x18 | leaves | 0x08 |
| 0x24 | bsp2d references | 0x08 |
| 0x30 | bsp2d nodes | 0x14 |
| 0x3C | surfaces | 0x0C |
| 0x48 | edges | 0x18 |
| 0x54 | vertices | 0x10 |

All values little-endian. Element layouts:

**bsp3d node (0x0C)**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | int32 | plane | index into planes; never flipped on the disc |
| 0x04 | 4 | int32 | back child | child index taken when dist < 0 |
| 0x08 | 4 | int32 | front child | child index taken when dist ≥ 0 |

Node 0 is the root. Children: node, leaf (top bit + leaf index) or
none (−1 = solid). On Blood Gulch 12070 children are nodes, 5999 leaves
and 6073 none; every node and leaf is reached exactly once (a tree).

**plane (0x10)**: four floats i, j, k, d as in the conventions. Normals
are unit length to within 5e-8 on the disc.

**leaf (0x08)**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 2 | uint16 | flags | bit 0 "contains double-sided surfaces" |
| 0x02 | 2 | int16 | bsp2d reference count | |
| 0x04 | 4 | int32 | first bsp2d reference | index of the first of the leaf's consecutive references |

Every leaf is open (non-solid) space. Bit 0 is set exactly on the
leaves one of whose references reaches a two-sided surface (checked on
boardingaction: 157 of 157, prisoner: 31 of 31). A leaf may have no
references (142 on Blood Gulch).

**bsp2d reference (0x08)**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | int32 | plane | signed plane index of a plane bounding the leaf |
| 0x04 | 4 | int32 | bsp2d node | root of a 2D tree: a node index, or top bit + surface index directly |

One reference per boundary plane of the leaf. The 2D tree below it
partitions that plane (in its 2D projection) among the surfaces lying
on it that face into the leaf. Every surface reached from a reference
has the same plane index and the same flip bit as the reference
(checked: 13600 of 13600 on Blood Gulch). The flip bit is set when the
leaf lies behind the stored plane. On Blood Gulch 12924 of 13182 roots
are a surface directly.

**bsp2d node (0x14)**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 12 | 3 × float | plane | 2D line (a, b, c): side = a·u + b·v − c |
| 0x0C | 4 | int32 | left child | taken when side < 0 |
| 0x10 | 4 | int32 | right child | taken when side ≥ 0 |

(u, v) are the 2D coordinates of the reference's projection (below).
Children are nodes or surfaces (top bit + surface index); no "none"
child occurs on the disc. (a, b) is unit length.

The projection of a reference: axis from the stored plane's normal n,
sign s = (n's component along the axis > 0) XOR (the reference's flip
bit). Equivalently: the axis and positive-side test of the surface's
own outward normal.

**surface (0x0C)**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | int32 | plane | signed plane index; the flipped plane is the surface's outward (front) plane |
| 0x04 | 4 | int32 | first edge | an edge of the surface's ring |
| 0x08 | 1 | uint8 | flags | bit 0 two sided, bit 1 invisible, bit 2 climbable, bit 3 breakable |
| 0x09 | 1 | int8 | breakable surface | index into the structure BSP's breakable surfaces (meaningful with bit 3) |
| 0x0A | 2 | int16 | material | index into the level's collision materials, or the model's materials; −1 none |

**edge (0x18)**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | int32 | start vertex | |
| 0x04 | 4 | int32 | end vertex | |
| 0x08 | 4 | int32 | forward edge | next edge of the left surface's ring |
| 0x0C | 4 | int32 | reverse edge | next edge of the right surface's ring |
| 0x10 | 4 | int32 | left surface | the surface whose ring runs start → end |
| 0x14 | 4 | int32 | right surface | the surface whose ring runs end → start |

This is a winged-edge mesh: every edge on the disc has both a left and a
right surface (no −1 found on either map), so the level's surface set
is closed.

**vertex (0x10)**: point (3 floats x, y, z) at 0x00, first edge (int32)
at 0x0C, an edge that has the vertex as its start or end (checked: all
vertices of both maps).

### Recovering a surface's polygon

For surface S, start at e = S.first edge and repeat:

1. If e.right surface = S, the surface is on the edge's right: emit
   e.end vertex and continue with e.reverse edge.
2. Otherwise (S is the left surface): emit e.start vertex and continue
   with e.forward edge.
3. Stop when the next edge is the first edge again.

(The test is "right surface equals S", so a degenerate edge with S on
both sides counts as right.) Consecutive edges share the vertex between
them: the end of one side's traversal is the start of the next. Rings
close in 3 to 8 edges; a reader should stop after 8 and treat a longer
ring as malformed.

The vertices come out counter-clockwise when seen from the surface's
front, that is with the outward normal (the plane after applying the
flip bit) pointing at the viewer, in the game's right-handed z-up frame.
Equivalently (v1 − v0) × (v2 − v0) points along the outward normal.
Checked on every surface of both maps: 4916 of 4916 and 1695 of 1695
agree, none opposite.

Every polygon is planar on its plane (largest vertex distance 0.00021
on Blood Gulch, 0.00024 on Hang 'Em High) and convex (no reflex corner
on either map). Triangulate as a fan from the first vertex: triangles
(v0, v_i, v_i+1) for i = 1 … k−2; each keeps the counter-clockwise
winding. If the engine converts axes with a reflection (for example
swapping y and z without negating one), the winding reverses and must
be flipped.

Two-sided surfaces come as twins: two surfaces on the same vertices,
same plane index with opposite flip bits, same flags and material
(checked: all 80 on boardingaction, all 16 on prisoner). A triangle
mesh that keeps both gets a double-sided sheet; one may drop one twin
if the physics engine treats triangles as two-sided anyway.

### Materials

A surface's material index selects an entry of the level's collision
materials block (in the structure BSP tag) or, in a collision model,
of the model's materials block (below). −1 means no material.

Level collision material (0x14 bytes):

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 16 | tag reference | shader | the shader drawn on the surface (class `senv` on both maps) |
| 0x10 | 2 | | padding | 0 |
| 0x12 | 2 | int16 | material type | precomputed on the disc; code below; −1 none |

The type is already filled in on the disc (when the map was built);
the game never recomputes it, so a reader takes it as it is. Material
types, as the tags enumerate them (damage, impact effects and
footsteps depend on them):

| Code | Type | Code | Type |
|---|---|---|---|
| 0 | dirt | 17 | engineer |
| 1 | sand | 18 | engineer force field |
| 2 | stone | 19 | flood combat form |
| 3 | snow | 20 | flood carrier form |
| 4 | wood | 21 | cyborg |
| 5 | metal (hollow) | 22 | cyborg energy shield |
| 6 | metal (thin) | 23 | armored human |
| 7 | metal (thick) | 24 | human |
| 8 | rubber | 25 | sentinel |
| 9 | glass | 26 | monitor |
| 10 | force field | 27 | plastic |
| 11 | grunt | 28 | water |
| 12 | hunter armor | 29 | leaves |
| 13 | hunter skin | 30 | elite energy shield |
| 14 | elite | 31 | ice |
| 15 | jackal | 32 | hunter shield |
| 16 | jackal energy shield | | |

A trace that asks for media and crosses its end cluster's fog plane
(`structure_bsp.md`) before any closer hit reports a media hit with no
surface: type 28 (water) when it starts below the plane, the fog
plane's own material type when it starts above. A type outside 0 to 32
is treated as none.

Surfaces with material −1 are collision that has no render geometry. On
Blood Gulch there are 94 (of 4916 surfaces, all triangles). Every
surface with a material lies inside the render bounding box (they span
it exactly), while the material-less ones extend the collision to the
outer cliffs and a lid at z = 68.037 (the first surface, below): 1 of
them lies wholly outside the box, 81 straddle its faces (one or two of
three vertices inside), and **12 lie wholly inside it**. Those 12 are
small interior terrain triangles (three clusters of 5, 5 and 2 near
(80, −80, 20), (95, −108, 15) and (26, −122, 19), almost all facing
up, one a steep undercut), not boundary walls. Most of the 94 are the
walls that keep players inside the level and must be in the physics
mesh; keep all 94. A hit on one reports material type none (no impact
effect). (Script over bloodgulch; box x 5.907 to 131.902, y −190.219
to −45.057, z −0.349 to 26.233, tolerance 0.01.)

### Surface flags: who collides with what

| Bit | Tag name | Effect |
|---|---|---|
| 0 | two sided | hit from both sides; lies between leaves flagged bit 0; the twin is the other side |
| 1 | invisible | traces can be told to ignore it (projectiles and line of sight do); bipeds and vehicles always collide |
| 2 | climbable | read by the biped mover (ladders: `movement.md`, section 8): every contact the mover records carries its surface's flags, edge and corner contacts included; no effect on collision itself |
| 3 | breakable | ignored once the breakable surface it names is broken |

On the disc every flagged surface is also two-sided. Flag combinations
over all maps: none 1081309 surfaces; two-sided 9750; two-sided +
invisible 4248; two-sided + climbable 168; two-sided + invisible +
climbable 1358; two-sided + breakable 1488. On the multiplayer maps
alone: two-sided 66, two-sided + climbable 120, two-sided + breakable 8
(prisoner); no multiplayer map has an invisible surface (their
boundaries are the material-less surfaces above). The 120 climbable
surfaces are ladders: 60 rectangular sheets of two surfaces each (a
face and its twin), on 8 of the 13 maps (checked by a script over all
13, level collision and every object collision model): boardingaction
80 surfaces (40 sheets), prisoner 8, sidewinder 8, wizard 8,
beavercreek 4, damnation 4, longest 4, putput 4. Blood Gulch, Hang 'Em
High, Carousel, Chill Out and Rat Race have none (the last three
have only two-sided surfaces: 40, 2 and 10; Blood Gulch and Hang 'Em
High have no flagged surface at all).

Breakable state: one bit per breakable surface of the current structure
BSP (256), all set (intact) when the BSP loads, cleared when the
surface breaks (`structure_bsp.md` / a later breakables spec). A surface
with bit 3 whose breakable index is below 256 and whose bit is clear is
skipped by every query; with the bit set it collides normally.

Which flags each kind of query uses (from the reference only; the
caller passes a flag set, the BSP honours five of its bits:
front-facing, back-facing, ignore two-sided, ignore invisible, ignore
breakable):

| Query user | Front | Back | Ignore two-sided | Ignore invisible |
|---|---|---|---|---|
| projectiles | yes | no | no | yes |
| line of sight | yes | no | yes | yes |
| other traces with neither facing bit | yes | yes | no | no |
| biped and vehicle movement (sphere queries) | n/a | n/a | never | never |

If a trace asks for neither front- nor back-facing surfaces, both are
used. No caller sets "ignore breakable" in the reference.

### Model collision (the `coll` tag)

Objects (bipeds, vehicles, weapons, scenery, ...) refer to a collision
model tag, class `coll`, 0x298 bytes. Health, shield and damage fields
belong to `health_shields.md`; their offsets are listed so that spec
can refer to them (names as the tag carries them, checked values from
`characters\cyborg\cyborg` on Blood Gulch in brackets):

| Offset | Type | Field |
|---|---|---|
| 0x000 | uint32 | flags [7] |
| 0x004 | int16 | indirect damage material (index into materials) [1] |
| 0x008 | float | maximum body vitality [75] |
| 0x00C | float | body system shock |
| 0x044 | float | friendly damage resistance |
| 0x070 | tag ref | localized damage effect |
| 0x080 | float | area damage effect threshold |
| 0x084 | tag ref | area damage effect |
| 0x094 | float | body damaged threshold |
| 0x098 | tag ref | body damaged effect |
| 0x0A8 | tag ref | body depleted effect |
| 0x0B8 | float | body destroyed threshold |
| 0x0BC | tag ref | body destroyed effect |
| 0x0CC | float | maximum shield vitality [75] |
| 0x0D2 | int16 | shield material type [22, cyborg energy shield] |
| 0x0EC | int16 | shield failure function |
| 0x0F0 | float | shield failure threshold |
| 0x0F4 | float | failing shield leak fraction |
| 0x108 | float | minimum stun damage |
| 0x10C | float | stun time |
| 0x110 | float | recharge time [4.0 s] |
| 0x184 | float | shield damaged threshold |
| 0x188 | tag ref | shield damaged effect |
| 0x198 | tag ref | shield depleted effect |
| 0x1A8 | tag ref | shield recharging effect |
| 0x1C0 | float | runtime shield recharge velocity [0.0083333 = 1 / (4.0 s × 30 ticks/s)] |
| 0x234 | block | materials (0x48 each) |
| 0x240 | block | regions (0x54 each) |
| 0x24C | block | modifiers (empty on every Blood Gulch model) |
| 0x268 | 6 floats | pathfinding box (x0, x1, y0, y1, z0, z1; zero on the models checked) |
| 0x280 | block | pathfinding spheres (0x20 each) |
| 0x28C | block | nodes (0x40 each) |

Gaps between the listed fields are unused or padding.

The vehicles' collision models (warthog, scorpion_mp, ghost_mp on every
multiplayer map checked) have flags 0x28, maximum body vitality 0 and
maximum shield vitality 0: see `health_shields.md`. (Of the Blood Gulch
collision models 31 have flags 0 and no vitality, the three vehicles
0x28, the Spartan 0x7 with 75 and 75, and one has flags 0x20 with body
vitality 50.)

Material (0x48): name (32-byte string) at 0x00; flags at 0x20 (bit 0
"head": set on the cyborg's head material); material type (int16, the
table above) at 0x24; shield leak percentage 0x28, shield damage
multiplier 0x2C and body damage multiplier 0x3C (floats, for
`health_shields.md`). A model surface's material index selects one of
these; the hit's material type is that entry's type.

Region (0x54): name at 0x00; flags 0x20; damage threshold 0x28 (float);
destroyed effect (tag ref) 0x38; permutations block 0x48 whose elements
are a 32-byte name each. Warthog: regions "barrels" (permutations
`__base`, `~primary-blur`), "hull", "tire" (`__base`, `~blur`).

Pathfinding sphere (0x20): node index (int16, −1 for the object's
origin) at 0x00; centre (3 floats, in that node's space) at 0x10;
radius at 0x1C. Used by AI path planning only (a later spec); the
cyborg has one sphere (node −1, centre (0, 0, 0.2), radius 0.2), the
warthog fifteen.

Node (0x40): name at 0x00 (32 bytes; the same names as the object's
model nodes, e.g. "bip01 pelvis"); region index (int16, −1 none) 0x20;
parent node 0x22; next sibling node 0x24; first child node 0x26 (int16,
−1 none); a runtime damage part number (int16, precomputed) at 0x32;
bsps block at 0x34, elements are 0x60-byte collision BSPs exactly as
above, one per permutation of the node's region.

Placement. Collision node i is the object's node i: the object's
current node matrices (from its animation, `animations.md` and
`objects.md`) carry node-local coordinates to world, including scale. A
node takes part only if its region is not none, the object's current
permutation index for that region is not none, and it has at least one
BSP and that BSP has at least one 3D node. The BSP used is the one at
the permutation index, clamped to 0 … count − 1. To trace a model,
transform the segment's start and vector by the inverse of the node
matrix, run the BSP trace in node space, and keep the closest hit over
all nodes (the parameter t is unchanged by an affine map). The hit
normal goes back to world space through the node matrix (as a plane:
transform, renormalise) and is flipped if the surface's plane is.
Model BSPs ignore breakable state. On Blood Gulch every surface of
every collision model has flags 0.

### Point test

Starting at node 0, take the front child when dist(p) ≥ 0 and the back
child otherwise, until the child is not a node. A leaf means the point
is in open space (and the leaf's index is also the structure BSP's leaf
index, whose cluster `structure_bsp.md` gives: the two BSPs have the
same node and leaf counts); none means the point is in solid space,
which includes everything outside the level.

### Line trace

Input: start P, vector V (the segment is P + tV, t in [0, 1]), a flag
set, and a maximum t (the level passes "infinite"; a model trace passes
the best t found so far). Output: hit or not, and for a hit: t, the
surface index, its outward plane (the plane flipped as the surface
says), flags, breakable index, material index, and the list of leaves
visited in order (at most 256; further leaves overwrite the last
entry). The first leaf visited and the last give the start and end
locations.

State carried along the walk: the previous leaf and its contents
(unknown at the start), the plane last crossed, and the best t (starting
at the maximum t, clamped to ≥ 0). The walk covers t from 0 to
min(maximum t, 1).

Visiting a node over the interval [t0, t1]: with d = dist(P) and
k = n·V, let d0 = d + k·t0, d1 = d + k·t1. The interval reaches the
back if d0 < 0 or d1 < 0 and reaches the front if d0 ≥ 0 or d1 ≥ 0.

- Only one side reached: visit that child over [t0, t1].
- Both reached: the crossing is t = −d / k. The near side is the back
  child if k > 0, else the front child. Visit the near child over
  [t0, t]; if it reports a hit, stop. If the best t is ≤ t, stop
  without a hit (pruning against an earlier, closer hit). Otherwise
  record this node's plane as the plane last crossed and visit the far
  child over [t, t1].

Reaching a terminal at interval start t0: its contents are solid for
none, "open with two-sided surfaces" for a leaf with bit 0, else open.
Then, with the previous contents:

1. Front-facing enabled, previous open (either kind), current solid:
   test the previous leaf.
2. Else back-facing enabled, previous solid, current open (either
   kind): test the current leaf.
3. Else two-sided not ignored, previous and current both "open with
   two-sided surfaces": test the previous leaf if front-facing is
   enabled, else the current one, and require the point to be inside
   the polygon (step c below).
4. Otherwise no test.

Testing leaf L at t0 against the plane last crossed (index q):
(a) find L's first reference whose plane index (top bit ignored) is q;
(b) project the point P + t0·V with that reference's axis and sign and
walk its 2D tree to a surface (none: no surface, try the next matching
reference); (c) only in case 3, check the point against the polygon:
walk the ring, project each edge a → b, and the point is outside if
(p − a) × (b − a) > 0 (2D cross u1·v2 − v1·u2) for any edge; a broken
breakable surface counts as outside (breakable surfaces are two-sided
on the disc, so they are only met in case 3); outside means try the
next matching reference. A surface found is a hit unless it
is invisible and invisible surfaces are ignored, or breakable and
breakable surfaces are ignored. Then t = t0, and the walk ends.

If no hit, the leaf (if any) is appended to the visited list and becomes
the previous leaf with its contents. A trace that ends with no hit
reports t = 1.

Front-facing means "from open space into solid", which is a crossing of
a surface from its front; back-facing the reverse. Cases 1 and 2 do not
check the polygon: the 2D tree of a closed BSP already says which
surface covers the point. Only two-sided sheets, which do not separate
open from solid, need the polygon check.

Keeping the location valid (projectiles ask for it): after a hit, if
the hit point's leaf (point test) differs from the last leaf visited,
move the point 1/4096 along the hit normal and look it up again; if it
is now in no leaf, step t back by 1/4096 / |V·n| (1/32 if V·n = 0),
recomputing the point, until it is in a leaf or t reaches 0.

### Sphere query (what the level answers for movement)

Input: centre c, radius r, the breakable bits. Output: up to 256 each
of leaves, surfaces, edges and vertices, without duplicates (beyond 256
further ones are dropped); "touches" if any surface or edge was found.

1. Walk the 3D tree from node 0 with δ = dist(c), keeping a stack of
   signed plane indices. If −r < δ < r, visit the back child with the
   node's plane pushed flipped, then the front child with it pushed
   unflipped, popping after each. Otherwise go to the front child if
   δ ≥ r, else the back child, pushing nothing.
2. At a leaf: record it. For each of its references whose signed plane
   index equals one on the stack (top bit included): project c onto the
   reference's plane (c − dist(c)·n), take 2D coordinates with the
   reference's axis and sign, and walk its 2D tree visiting the left
   child when side ≤ r and the right child when side ≥ −r (both if
   both). At each surface reached: skip it if breakable and broken;
   record each ring vertex within r of c (|v − c|² ≤ r²); record each
   ring edge whose segment comes within r of c; if no vertex or edge was
   recorded, record the surface only if the projected centre is inside
   the polygon (for every ring edge a → b, (a − c2) × (b − c2) ≥ 0);
   otherwise record the surface.
3. None children are not visited (solid has no surfaces).

The sphere query ignores the invisible and two-sided flags: everything
blocks.

### Moving a pill (bipeds, `movement.md` owns the numbers)

The biped is a vertical pill: a segment from the point p (the centre of
its bottom sphere) up to p + (0, 0, h), radius w. The pill dimensions
come from the biped tag (`movement.md`). To move it by a displacement
D:

1. Query spheres (level, and objects in the clusters of the leaves
   found) with centre p + D/2 + (0, 0, h/2) and radius
   |D|/2 + h/2 + w + 0.0625.
2. Turn every recorded feature into Minkowski-sum shapes for a point
   standing for the bottom-sphere centre:
   - vertex v: a sphere of radius w at v; if h > 0 also a sphere at
     v − (0, 0, h) and a cylinder of radius w from v − (0, 0, h) up to v;
   - edge a → b, only if its two surfaces have different signed plane
     indices and the edge is convex: with n0, n1 the stored (unflipped)
     plane normals of the left and right surfaces and e = b − a, the
     edge is used if both indices name the same plane, or the flips
     agree and (n0 × n1)·e > −0.0001, or the flips differ and
     (n0 × n1)·e < 0.0001. It becomes a cylinder of radius w along
     a → b; if h > 0 also the same cylinder moved down by h, and two
     vertical slabs (the quad a, b, b − h, a − h, thickness w, outward
     normals ± the horizontal perpendicular of e);
   - surface: its polygon extruded along its outward normal into a slab
     of thickness w; if h > 0 and the normal's z < 0 (a ceiling), the
     slab moves down by h (plane distance d − h·n_z and the polygon's z
     coordinates lowered by h).
3. Sweep the point from p along D against all shapes. A hit counts only
   if D·(hit normal) < −0.0001. On a hit, move to the hit point, scale
   the remaining motion by (1 − t), remove its component along the hit
   plane; with two planes slide along their intersection line; with
   three stop. Repeat until the remaining motion is below 0.0001 in
   every component or the caller's collision budget is used.

The slab test: a point at distance δ from the slab plane is inside when
0 ≤ δ < w and its projection lies inside the polygon. All shapes report
their plane as the normal at the contact pushed out by w.

For the first playable, a capsule cast against the triangle mesh in the
physics engine is an acceptable stand-in: the sum above is a rounded
pill against convex polygons, except that concave edges get no cylinder
and the pill's top hits ceilings through the lowered slab.

### Pill sweep against the BSP (reference only)

A second, older query sweeps a sphere of radius r along a segment
through the BSP: the 3D walk reaches both sides of a plane when either
end is within r + 1/4096 of it; at a leaf, for each reference matching
the side stack, it takes the parameter where the sphere first touches
the plane, t = −d/k − r/|k| clamped to [0, 1], tests the touch point
against the surface's polygon, and also sweeps the sphere against every
ring edge as a capsule (and the edge's end vertices as spheres) with a
2D walk slack of r + 1/8192. Bipeds use it to check a standing pill
fits; the mover above does the sliding. Its exact use belongs to
`movement.md`.

## Per-tick order

Collision queries do not run on their own; they are called inside the
object and biped updates (`movement.md`, `projectiles.md`). Within one
line trace against the environment the order is: level BSP trace;
water (fog plane) check in the cluster of the last leaf; object traces
in the clusters of every visited leaf (each object once), each
replacing the result when closer; t = 1 if nothing hit; then the
keep-location step if asked.

## Edge cases

- A segment that starts in solid space (outside the level) and stays
  there hits nothing, whatever the flags.
- A segment starting in solid and entering open space hits only if
  back-facing surfaces are enabled; the hit is at the boundary, with
  the surface's outward normal (pointing along the segment).
- A point exactly on a plane is on its front.
- Two-sided twins: from either side the trace hits the twin facing it
  (front-facing case 3 uses the previous leaf's references, which lead
  to the twin facing that leaf).
- Coplanar surfaces on one plane are told apart only by the 2D tree.
- The trace's leaf test uses the plane last crossed, not the surface
  planes of the leaf in general; a leaf reached without crossing a
  plane (the very first leaf) never yields a hit.
- Malformed data (indices out of range, rings longer than 8, trees with
  cycles) does not occur on the disc; a reader should reject or clamp,
  treating a bad 3D child as solid and a bad leaf as solid.

## Quirks

- Collision geometry extends beyond the render geometry (Blood Gulch:
  up to x 174.0, y −220.4, z 68.0 against render bounds x 131.9,
  y −190.2, z 26.2). The material-less walls and lid are invisible
  limits; keep them.
- Player limits in multiplayer are not "invisible"-flagged surfaces,
  so projectiles stop at them too (their impact has no material).
- Every flagged surface on the disc is two-sided; ladders are
  two-sided + climbable thin sheets.

## Measured facts (disc)

Counts (level collision BSP, structure BSP 0):

| Map | 3D nodes | planes | leaves | 2D refs | 2D nodes | surfaces | edges | vertices |
|---|---|---|---|---|---|---|---|---|
| bloodgulch | 12071 | 4616 | 5999 | 13182 | 418 | 4916 | 7650 | 2704 |
| hangemhigh | 1946 | 964 | 1116 | 2659 | 1175 | 1695 | 3132 | 1381 |

| Map | triangles (fan) | surfaces with 3/4/5/6/7/8 vertices | surfaces with a flipped plane | leaves with bit 0 | max 3D / 2D depth |
|---|---|---|---|---|---|
| bloodgulch | 5468 | 4447 / 420 / 19 / 28 / 0 / 2 | 16 | 0 | 39 / 4 |
| hangemhigh | 2874 | 739 / 829 / 66 / 38 / 11 / 12 | 21 | 0 | 22 / 5 |

"Surfaces with a flipped plane" counts surfaces (of 4916 and 1695) whose
signed plane index has the flip bit set (16 distinct planes on Blood
Gulch, 15 on Hang 'Em High); no 3D node's plane is flipped, and 33 of
Blood Gulch's 13182 and 31 of Hang 'Em High's 2659 leaf references are
(script over the level collision BSPs).

Surface flags: none set on either map (4916 and 1695 surfaces with
flags 0; breakable index 0 everywhere).

Collision materials and surfaces per material:

| Map | Index | Shader | Type | Surfaces |
|---|---|---|---|---|
| bloodgulch | −1 | none | none | 94 |
| bloodgulch | 0 | `levels\test\bloodgulch\shaders\cap_cliff01b` | 2 stone | 1339 |
| bloodgulch | 1 | `levels\b30\shaders\metal flat generic` | 7 metal (thick) | 563 |
| bloodgulch | 2 | `levels\b30\shaders\metal panels generic ext clean` | 7 | 62 |
| bloodgulch | 3 | `levels\b30\shaders\metal panels generic ext` | 7 | 206 |
| bloodgulch | 4 | `levels\test\bloodgulch\shaders\cap_metal` | 7 | 87 |
| bloodgulch | 5 | `levels\test\bloodgulch\shaders\cap_ramp` | 7 | 32 |
| bloodgulch | 6 | `levels\test\bloodgulch\shaders\blood ground` | 1 sand | 2415 |
| bloodgulch | 7 | `levels\test\beavercreek\shaders\beavercreek boulder` | 2 stone | 48 |
| bloodgulch | 8 | `levels\test\bloodgulch\shaders\cap_ramp_blue` | 7 | 24 |
| bloodgulch | 9 | `levels\b30\shaders\metal panels ext unearthed` | 7 | 46 |
| hangemhigh | −1 | none | none | 9 |
| hangemhigh | 0 to 8 | nine `levels\b30\shaders\...` metal shaders | 7 metal (thick), all | 327, 857, 74, 66, 197, 27, 123, 2, 13 |

Bounds (x, y, z):

| Map | Collision vertices | Render geometry (= structure world bounds) |
|---|---|---|
| bloodgulch | −7.977 … 174.009, −220.411 … −30.122, −0.349 … 68.037 | 5.907 … 131.902, −190.219 … −45.057, −0.349 … 26.233 |
| hangemhigh | 5.900 … 35.878, −22.823 … 15.823, −9.253 … 8.183 | identical |

On Blood Gulch the surfaces that have a material span exactly the render
bounds; the excess is made of material-less surfaces (82 of the 94
reach outside the box; 12 sit wholly inside it).

Verification done with throwaway scripts on the disc's files:

- Every surface polygon of both maps rebuilt with the ring walk: every
  ring closes, consecutive edges share their vertex, every polygon is
  planar (≤ 0.00024), convex, and counter-clockwise about its outward
  normal. Exported as OBJ meshes (2704 vertices, 5468 triangles;
  1381, 2874), bounds as in the table.
- The line trace as described, against brute-force segment/triangle
  intersection over every triangle: 300 random segments (random start
  in the collision bounds, random direction, length 1 to 60) per map
  and per flag set (front + back; front only, brute force counting
  triangles facing the segment and two-sided ones) on bloodgulch,
  hangemhigh, prisoner and boardingaction: all 2400 agree on hit or
  miss, and hits agree on the surface and on t (largest difference
  0.00014 world units along the segment).
- Segments through the centre of every two-sided surface, from both
  sides (boardingaction 160, prisoner 32): agree with brute force with
  front only and with front + back; with "ignore two-sided" none of
  them hits the two-sided surface.
- Collision model layout on all 36 `coll` tags of Blood Gulch: every
  node's BSPs parse with the same layout; values as quoted.

## Test cases

### (a) Synthetic: a closed box room

Interior x ∈ [0, 4], y ∈ [0, 4], z ∈ [0, 3]; open inside, solid
outside; one collision material (index 0, type 2 stone).

Vertices (point; first edge):

| Index | Point | First edge |
|---|---|---|
| 0 | (0, 0, 0) | 0 |
| 1 | (4, 0, 0) | 0 |
| 2 | (4, 4, 0) | 1 |
| 3 | (0, 4, 0) | 2 |
| 4 | (0, 0, 3) | 4 |
| 5 | (4, 0, 3) | 6 |
| 6 | (4, 4, 3) | 5 |
| 7 | (0, 4, 3) | 4 |

Planes (i, j, k, d), all facing into the room:

| Index | Plane | Meaning |
|---|---|---|
| 0 | (0, 0, 1, 0) | floor z = 0 |
| 1 | (0, 0, −1, −3) | ceiling z = 3 |
| 2 | (1, 0, 0, 0) | wall x = 0 |
| 3 | (−1, 0, 0, −4) | wall x = 4 |
| 4 | (0, 1, 0, 0) | wall y = 0 |
| 5 | (0, −1, 0, −4) | wall y = 4 |

3D nodes (plane, back child, front child): node i for i = 0 … 4 is
(i, −1, i + 1); node 5 is (5, −1, 0x80000000), the last front child
being leaf 0.

Leaf 0: flags 0, reference count 6, first reference 0.

bsp2d references: reference i (i = 0 … 5) is (plane i, 0x80000000 | i):
plane i unflipped, root directly surface i. No bsp2d nodes.

Surfaces (plane, first edge, flags, breakable, material): surface i is
(i, f_i, 0, 0, 0) with f = 0, 4, 3, 10, 9, 2 for i = 0 … 5.

Edges (start, end, forward, reverse, left, right):

| Index | Edge |
|---|---|
| 0 | (0, 1, 1, 9, 0, 4) |
| 1 | (1, 2, 2, 10, 0, 3) |
| 2 | (2, 3, 3, 11, 0, 5) |
| 3 | (3, 0, 0, 8, 0, 2) |
| 4 | (4, 7, 5, 9, 1, 2) |
| 5 | (7, 6, 6, 8, 1, 5) |
| 6 | (6, 5, 7, 11, 1, 3) |
| 7 | (5, 4, 4, 10, 1, 4) |
| 8 | (3, 7, 4, 2, 2, 5) |
| 9 | (4, 0, 3, 7, 2, 4) |
| 10 | (1, 5, 6, 0, 3, 4) |
| 11 | (6, 2, 1, 5, 3, 5) |

| Setup | Input | After | Expected |
|---|---|---|---|
| box room | polygon of surface 0 | ring walk | (0,0,0), (4,0,0), (4,4,0), (0,4,0) |
| box room | polygon of surface 1 | ring walk | (0,0,3), (0,4,3), (4,4,3), (4,0,3) |
| box room | polygon of surface 2 | ring walk (first edge has surface 2 on its right) | (0,0,0), (0,4,0), (0,4,3), (0,0,3) |
| box room | polygon of surface 3 | ring walk | (4,0,0), (4,0,3), (4,4,3), (4,4,0) |
| box room | polygon of surface 4 | ring walk | (0,0,0), (0,0,3), (4,0,3), (4,0,0) |
| box room | polygon of surface 5 | ring walk | (0,4,0), (4,4,0), (4,4,3), (0,4,3) |
| box room | every polygon | (v1 − v0) × (v2 − v0) | parallel to the plane's normal (counter-clockwise from inside) |
| box room | triangulate | fan | 12 triangles |
| box room | point (1, 1, 1) | point test | leaf 0 |
| box room | point (5, 1, 1) | point test | solid |
| box room | P (1, 1, 2), V (0, 0, −3), front + back | trace | hit, t = 2/3, surface 0, normal (0, 0, 1), point (1, 1, 0), material 0 |
| box room | P (1, 1, 1), V (2, 2, 1), front + back | trace | no hit, t = 1 |
| box room | P (1, 1, −1), V (0, 0, 2), front only | trace | no hit |
| box room | P (1, 1, −1), V (0, 0, 2), front + back | trace | hit, t = 0.5, surface 0, normal (0, 0, 1) |
| box room | P (2, 2, 1), V (4, 0, 0), front + back | trace | hit, t = 0.5, surface 3, normal (−1, 0, 0) |
| box room | P (5, 5, 5), V (1, 0, 0), front + back | trace | no hit (all solid) |

### (b) Disc-backed (slow tests)

| Setup | Input | After | Expected |
|---|---|---|---|
| bloodgulch level collision BSP | read | counts | as the counts table; 5468 fan triangles; no surface flags |
| hangemhigh level collision BSP | read | counts | as the counts table; 2874 fan triangles |
| bloodgulch | every surface | ring walk | closes, ≤ 8 vertices, counter-clockwise about the outward normal, planar within 0.0003 |
| bloodgulch | surface 0 | fields | plane 0 (0, 0, −1, −68.03709) unflipped, first edge 0, flags 0, material −1 |
| bloodgulch | surface 0 polygon | ring walk | (169.49675, −39.11990, 68.03709), (174.00922, −220.41060, 68.03709), (41.40953, −183.45016, 68.03709), (−5.94453, −145.65318, 68.03709), (−7.97682, −40.75824, 68.03709), (89.12006, −30.12179, 68.03709) |
| bloodgulch, player starting location 0 at (98.49340, −157.63901, 1.70473) | P = location + (0, 0, 1), V (0, 0, −20), front + back | trace | hit, t = 0.0500622, height z = 1.703486, surface 1300, normal (0, 0, 1), material 2 (metal (thick)) |
| bloodgulch, player starting location 6 at (43.91446, −80.67870, 1.70433) | same | trace | hit, z = 1.703486, surface 2306, material 2 |
| bloodgulch | P (65, −120, 30), V (0, 0, −40), front only | trace | hit, t = 0.7465926, z = 0.136297, surface 1495, normal (−0.009558, 0.045286, 0.998928), material 6 (sand) |
| bloodgulch | point (65, −120, 30) | point test | leaf 5960 |
| bloodgulch | material-less surfaces | count | 94: 1 wholly outside the render bounds, 81 straddling them, 12 wholly inside |
| boardingaction | two-sided surfaces | count, twins | 80, each with a twin of opposite flip |
| `characters\cyborg\cyborg` (coll) | nodes | read | 19 nodes; 3 with region −1 and no BSP; materials head, body, legs, shield generator, all type 21 |

Player starting locations are the scenario's block at offset 0x354
(`scenario.md`), 52 bytes each, position first.

## Confidence

| Section | Confidence |
|---|---|
| Block and element layouts of the collision BSP | checked against the disc (both maps; field meanings from the reference, confirmed by the ring, tree and trace checks) |
| Child and signed plane index encodings, projection table | reference; confirmed on the disc by the trace agreeing with brute force |
| Polygon recovery, winding, convexity | checked against the disc |
| Two-sided twins, leaf bit 0 | checked against the disc (boardingaction, prisoner) |
| Level collision materials, precomputed type | checked against the disc |
| Material type names and codes | reference (tag enumeration); codes seen on the disc (1, 2, 7, 8, 9, 21, 22) fit |
| Material-less surfaces are mostly the level boundary (12 of 94 on Blood Gulch are interior triangles) | inferred from the disc (bounds; script count) |
| Flag statistics | checked against the disc (all maps) |
| Which query ignores which flag | reference only |
| `coll` layout | offsets of materials, regions, pathfinding, nodes and the values quoted checked on Blood Gulch; other health field offsets from the reference |
| Node placement by the skeleton | reference only |
| Line trace | reference; reimplemented and checked against brute force on four maps |
| Sphere query, feature shapes, pill mover, BSP pill sweep | reference only |
| Keep-location step | reference only |

## Open questions

- Material-less surfaces: confirm in the game that they are the sky or
  boundary faces (and what their render shader was in the source
  level), and what the 12 interior ones are (invisible patches in the
  terrain, or collision-only helpers); a hit on one should show no
  impact effect.
- The flag sets per query (projectiles ignore invisible but not
  two-sided, line of sight ignores both) come from one build of the
  reference; they cannot be read from the disc. Check by shooting
  through a ladder on boardingaction: if bullets stop, projectiles do
  not ignore two-sided surfaces.
- How the biped's position relates to the pill's bottom sphere, and
  the pill's height and radius, belong to `movement.md`; this spec only
  fixes that the mover's point is the bottom sphere's centre.
- The modifiers block of `coll` is empty on every Blood Gulch model;
  its element layout is not known.
- Whether a node matrix with non-uniform scale occurs (the hit normal
  would then need the inverse transpose); none was looked for.
- The region "damage threshold" offset (0x28) and the material
  multipliers were read as zero or one on the models checked, so their
  offsets rest on the reference.
