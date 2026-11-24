# GoldSrc: BSP version 30 (maps)

Source basis: public format documentation and community write-ups of the
Half-Life map format; the installed maps (all 25 in `cstrike/maps/` and
the 125 BSP files in `valve/maps/` of the HLDS install, read with
throwaway scripts outside the repository); Valve's Half-Life SDK as
spec-session reference for the shared player movement's trace results.
No engine source, no decompiled or leaked code.
Status: draft

## Summary

A BSP 30 file is a 124-byte header (version and 15 lump records) and 15
lumps: the entity text, planes, textures (embedded or named for WADs),
vertices, compressed visibility, the render tree's nodes and leaves,
texture projections, faces, RGB lightmaps, the clip node trees, face
lists, edges and the brush models. It holds **no brushes**: the solid
world exists only as four trees per brush model, hull 0 (the render
tree, points) and hulls 1 to 3 (clip trees in which the world is
already grown by a fixed box, so that a box trace becomes a point
trace). Hulls are what the engine collides against; their sizes are
fixed by the engine and baked into the map by the compiler.

## Units and conventions

These hold for every spec in `specs/goldsrc/`.

- Little-endian. Integers are two's complement; "f32" is an IEEE single.
- Distance: the unit (the inch, 0.0254 m). Z is up; X and Y are the
  horizontal axes as in Source (a right-handed frame).
- Angles in entity keys are degrees, given as "pitch yaw roll"; `angle`
  alone is a yaw (with -1 meaning straight up and -2 straight down, as in
  Source).
- Offsets are byte offsets from the start of the file unless a section
  says "relative to" something else.
- Strings are fixed-size, NUL-padded ASCII/Latin-1; anything after the
  first NUL is garbage and ignored.
- Names in these specs (lump and field names) are ours.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| version | 30 | i32 | bytes 0-3; the file has no magic text |
| lump_count | 15 | | lump records after the version |
| header_size | 124 | bytes | 4 + 15 × 8 |
| luxel_size | 16 | units | one lightmap sample per 16 texture units |
| max_light_styles | 4 | | styles per face |
| no_style | 255 | | an unused style slot |
| hull_count | 4 | | hull 0 (point) and clip hulls 1-3 |
| trace_backoff | 1/32 (0.03125) | units | how far a trace's end is kept off the plane it hit |

## Header and lumps

The header is the version (i32, 30) then 15 records of (offset i32,
length i32), in this fixed order. The record size column gives the
record size inside the lump; a lump's length is always a whole number
of records (checked on all 150 maps).

| # | Lump (ours) | Record | Holds |
|---|---|---|---|
| 0 | entities | text | the entity key/values, NUL-terminated |
| 1 | planes | 20 | split and face planes |
| 2 | textures | varies | the texture directory and embedded mip textures |
| 3 | vertices | 12 | face corner points |
| 4 | visibility | bytes | compressed potentially-visible sets |
| 5 | nodes | 24 | the render tree's (hull 0's) inner nodes |
| 6 | texinfo | 40 | texture projections and flags |
| 7 | faces | 20 | polygons |
| 8 | lighting | bytes | RGB lightmaps |
| 9 | clipnodes | 8 | the clip trees of hulls 1-3 |
| 10 | leaves | 28 | the render tree's leaves |
| 11 | marksurfaces | 2 | per-leaf face lists |
| 12 | edges | 4 | vertex pairs |
| 13 | surfedges | 4 | per-face signed edge lists |
| 14 | models | 64 | the world (model 0) and brush models `*1`, `*2`... |

The lumps' data is not stored in directory order. Every stock map has
the same physical order: planes first (at offset 124, right after the
header), then leaves, vertices, nodes, texinfo, faces, clipnodes,
marksurfaces, surfedges, edges, models, lighting, visibility, entities,
textures. A reader must follow the offsets.

Other GoldSrc games (not in this install) are known to ship BSP 30 files
with the entities and planes records swapped (Blue Shift): out of scope
until such a game is ported.

### Worked example: `cstrike/maps/de_dust2.bsp`

2 057 288 bytes, version 30.

| Lump | Offset | Length | Records |
|---|---|---|---|
| entities | 1 315 884 | 9 802 | text |
| planes | 124 | 191 640 | 9 582 |
| textures | 1 325 688 | 731 600 | 44 textures |
| vertices | 232 504 | 78 660 | 6 555 |
| visibility | 1 245 288 | 70 594 | bytes |
| nodes | 311 164 | 66 384 | 2 766 |
| texinfo | 377 548 | 25 520 | 638 |
| faces | 403 068 | 107 660 | 5 383 |
| lighting | 742 324 | 502 962 | bytes (a multiple of 3) |
| clipnodes | 510 728 | 66 568 | 8 321 |
| leaves | 191 764 | 40 740 | 1 455 |
| marksurfaces | 577 296 | 13 040 | 6 520 |
| edges | 688 224 | 51 348 | 12 837 |
| surfedges | 590 336 | 97 888 | 24 472 |
| models | 739 572 | 2 752 | 43 |

Largest counts over the 150 maps: 32 612 planes, 25 837 nodes, 32 551
clip nodes, 17 676 leaves, 41 123 faces, 299 models. Clip node counts
approach 32 767 because their child links are signed 16-bit.

## Records

### Planes (20 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 3 × f32 | normal (unit length) |
| 12 | f32 | distance: a point p is on the plane when normal · p = distance |
| 16 | i32 | kind: 0, 1, 2 = normal is the X, Y or Z axis; 3, 4, 5 = not axial, and X, Y or Z is its largest component |

A point is on the plane's **front** when normal · p − distance ≥ 0.
Planes are stored in pairs: every even-indexed plane is followed by its
opposite (normal and distance negated) on all checked maps (4 791 pairs
in de_dust2), so axial planes with negative normals exist. Nodes and
clip nodes may use either of a pair.

### Vertices (12 bytes)

A point: 3 × f32.

### Edges (4 bytes) and surfedges (4 bytes)

An edge is two vertex indices (u16, u16). Edge 0 is a placeholder and
never used, so that its index can be negated. A surfedge is an i32: a
value e ≥ 0 means edge e walked from its first vertex to its second; e <
0 means edge −e walked backwards (its second vertex first). A face's
corners are the first vertex of each of its surfedges, in order.

### Faces (20 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | u16 | plane index |
| 2 | u16 | side: nonzero when the face looks along the plane's negative normal |
| 4 | i32 | first surfedge |
| 8 | u16 | surfedge count (corners) |
| 10 | u16 | texinfo index |
| 12 | 4 × u8 | light styles (255 = unused slot) |
| 16 | i32 | lightmap byte offset into the lighting lump, −1 = no lightmap |

Faces are convex polygons. Winding: as listed, the corners run clockwise
when seen from the face's front (the side it is drawn on), the usual
Quake-lineage convention; renderers that cull back faces must check this
against a screenshot (open question 3).

### Texinfo (40 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 4 × f32 | s axis (3) and s offset |
| 16 | 4 × f32 | t axis (3) and t offset |
| 32 | i32 | texture index (into the textures lump) |
| 36 | i32 | flags: 1 = special (no lightmap, not lit: sky and tool faces) |

A point p gets texel coordinates s = s_axis · p + s_offset and t =
t_axis · p + t_offset; the texture coordinate is (s / width, t / height)
of the texture. The axes are not normalized: their length is 1 / scale.
Only flag values 0 and 1 occur. Flag 1 is on `sky` and on the tool
textures (`aaatrigger`, `clip`, `null`, `origin`); unlit faces in the
stock maps are exactly the `sky` faces and the `aaatrigger` faces.

### Lightmaps

Each lit face has, for each used style, a block of w × h RGB triples
(3 bytes each, 0-255 a channel, rows along t, samples along s), the
blocks of its styles one after another from the face's lightmap offset.
The block size comes from the face's texel extents:

1. For each corner, compute s and t as above **in double precision,
   then round the result to single precision**.
2. s_min, s_max, t_min, t_max over the corners.
3. w = ceil(s_max / 16) − floor(s_min / 16) + 1, likewise h from t.

The rounding in step 1 matters: computed entirely in double, 303 of
de_dust2's 4 002 lit faces get a size one row or column too large;
entirely in single precision with a left-to-right sum, 21 still
disagree; with step 1 as written, every lit face in all 150 maps sizes
its block so that the blocks tile the lighting lump exactly. The lumps
are packed in face order with no gaps.

A lightmap sample (i, j) sits at texel (16 × (floor(s_min / 16) + i),
16 × (floor(t_min / 16) + j)); a texel's light is the bilinear blend of
the four nearest samples.

Styles: 0 is the always-on light; 1-63 are switchable or animated light
styles (lights with a name or a pattern); 255 ends the list. In the stock
maps only (0, 255, 255, 255) and (255, 255, 255, 255) occur (de_dust2:
4 002 and 1 381 faces). Stored values are the compiler's linear result;
the original renderer brightens them with gamma settings before
display. The brightening is a presentation matter, measured against
captures (open question 1).

Worked example: de_dust2 face 0: plane 1776, side 0, surfedges 0-3,
texinfo 8, styles (0, 255, 255, 255), lightmap offset 0. Its s extents
are −1952 to −1856 and t −64 to −32, so w = −116 − (−122) + 1 = 7 and h =
−2 − (−4) + 1 = 3: 21 samples, 63 bytes.

### Textures lump

| Part | Layout |
|---|---|
| count | i32 |
| directory | count × i32: offset of each texture relative to the lump start; −1 = missing (none in the stock maps) |
| each texture | a mip texture record (below) |

A mip texture record is the same as a WAD3 mip texture
([wad3.md](wad3.md) "Mip textures"): 16-byte name, width and height
(u32), four mip offsets (u32, relative to the record's start), then, when
embedded, the four mip levels' pixels, a u16 colour count (256), 256 RGB
colours and padding to a multiple of 4. When all four offsets are 0 the
texture is **not embedded**: the record is just its 40 bytes and the
pixels are looked up by name in the WADs (mount.md).

Worked example (de_dust2): 44 textures; the directory's first offset is
180 (= 4 + 4 × 44). Texture 0 `+0~fifties_lgt2` (16 × 64) is not
embedded (offsets all 0, record 40 bytes); texture 8 `-0SandRock2`
(128 × 256) is embedded with mip offsets 40, 32 808, 41 000, 43 048,
colour count 256 at byte 43 560 of the record, and the record spans 44
332 bytes (44 330 rounded up to a multiple of 4). 15 of the 44 are
embedded. Over the stock maps, some embed nothing (as_oilrig), some
everything (de_inferno, de_aztec, cs_siege: every texture).

### Nodes (24 bytes): hull 0's inner nodes

| Offset | Type | Field |
|---|---|---|
| 0 | i32 | plane index |
| 4 | 2 × i16 | children: front, back. A value ≥ 0 is a node index; a value < 0 is the leaf −(value + 1) |
| 8 | 3 × i16 | bounding box minimum |
| 14 | 3 × i16 | bounding box maximum |
| 20 | u16 | first face |
| 22 | u16 | face count (the faces lying on this node's plane) |

### Leaves (28 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | i32 | contents (below) |
| 4 | i32 | visibility offset into the visibility lump, −1 = none |
| 8 | 3 × i16 | bounding box minimum |
| 14 | 3 × i16 | bounding box maximum |
| 20 | u16 | first marksurface |
| 22 | u16 | marksurface count |
| 24 | 4 × u8 | ambient sound levels (water, sky, slime, lava) |

Leaf 0 is the single shared solid leaf (contents −2); every solid branch
of hull 0 ends in it. Leaves 1 to visleafs (model 0's count) belong to
the world; the brush models' leaves follow (de_dust2: 1 + 1 201 + 253 =
1 455; the 253 brush-model leaves have visibility offset −1).

### Marksurfaces (2 bytes)

A u16 face index; a leaf's faces are the marksurface range it names.

### Models (64 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 3 × f32 | bounding box minimum |
| 12 | 3 × f32 | bounding box maximum |
| 24 | 3 × f32 | origin (0 for the stock maps' models; a brush entity's origin comes from its `origin` key) |
| 36 | 4 × i32 | head of each hull: hull 0's root node, hulls 1-3's root clip nodes |
| 52 | i32 | visleafs (meaningful for model 0) |
| 56 | i32 | first face |
| 60 | i32 | face count |

Model 0 is the world; model n is the brush entity whose `model` key is
`*n`. Every brush model has its own four trees. de_dust2 model 0: box
(−2816, −1664, −320) to (2048, 3936, 640), heads (0, 0, 2350, 4521),
1 201 visleafs, faces 0-5129. Model 1 (a breakable crate): box (−1472,
2592, 0) to (−1344, 2656, 64), heads (2513, 7358, 7364, 7370), faces
5130-5135.

### Visibility

For each world leaf (1 to visleafs) with an offset ≥ 0, a run-length
encoded bit row of ceil(visleafs / 8) bytes: bit k (least significant
bit first within a byte) set means world leaf k + 1 may be visible. In
the encoding, a nonzero byte is literal; a zero byte is followed by a
count byte and stands for that many zero bytes. The last run may
overshoot the row; ignore the excess. de_dust2: rows of 151 bytes; leaf
1's row starts at offset 0, takes 131 encoded bytes and marks 196 leaves
visible; the world's leaves see 133 on average. Leaf 0's offset is 0
and meaningless.

### Entities

ASCII/Latin-1 text ending in a NUL: a sequence of blocks `{ ... }`, each
holding lines of `"key" "value"`. The first block is `worldspawn`.
Values may contain backslashes (Windows paths); there are no escapes.
Keys can repeat in a block (de_dust2's worldspawn has `classname`
twice); the last occurrence wins (both are the same in the stock maps).
Key case varies (`MaxRange`); compare keys case-insensitively.

`worldspawn` keys seen in the stock maps: `wad` (25 of 25), `skyname`
(25), `mapversion` (24, value 220), `MaxRange` (24), `sounds`,
`message` (the map's title), `light`, `WaveHeight`, `csg_options`,
`startdark`, `newunit`, `gametitle`, `worldtype`.

- `wad`: a `;`-separated list of WAD paths from the mapper's machine
  (de_dust2: `\sierra\half-life\valve\halflife.wad;...\decals.wad;...\cs_dust.wad;`).
  Only each entry's file name counts (mount.md). A listed WAD may be
  absent from the install when the map embeds the textures it used
  (de_inferno lists 22 WADs, 10 of them absent, and embeds all 97 of its
  textures). Every non-embedded texture of all 25 stock maps is found in
  a WAD that is both listed and installed.
- `skyname`: the sky box's base name (mount.md "Sky"); `desert` when
  absent.
- `MaxRange`: the far clip distance in units (8192 in de_dust2).

Entity classes that matter to a map loader: `info_player_start`
(Counter-Terrorist spawns in CS), `info_player_deathmatch` (Terrorist
spawns), `light_environment` (sun colour and angle), the brush entities
below, `env_sprite`/`env_glow`/`cycler_sprite` (sprites), `infodecal`
(decals by texture name), `ambient_generic` (sounds), `func_ladder`.

## Contents

A leaf's (and clip tree end's) contents value:

| Value | Meaning | In the stock maps |
|---|---|---|
| −1 | empty | yes |
| −2 | solid | yes |
| −3 | water | yes (48 leaves in cstrike's maps, 3 595 in valve's) |
| −4 | slime | not in this install |
| −5 | lava | not in this install |
| −6 | sky (solid for movement; the sky box is drawn through faces facing it) | yes |
| −7 to −15 | compiler-only (origin, clip, currents, translucent); never in a compiled tree | no |

Clip trees use −1, −2 and −6, rarely −3 (127 ends in valve's maps, none
in cstrike's). Water volumes in CS maps are mostly brush entities
(`func_water`, whose `skin` key holds its contents value: −3 on all 50
in the stock maps), so a point's full contents are the world's plus any
brush entity containing it.

## Hulls and collision

### Hull sizes

The engine knows four hulls; the compiler builds hulls 1-3 by growing
every solid brush by the hull's box before building the tree, so that a
box moving in the original world is a point moving in the hull.

| Hull | Box minimum | Box maximum | Size | Used for |
|---|---|---|---|---|
| 0 | (0, 0, 0) | (0, 0, 0) | a point | bullets, line traces, rendering, point contents |
| 1 | (−16, −16, −36) | (16, 16, 36) | 32 × 32 × 72 | a standing player |
| 2 | (−32, −32, −32) | (32, 32, 32) | 64 × 64 × 64 | large boxes (big monsters, some pushables) |
| 3 | (−16, −16, −18) | (16, 16, 18) | 32 × 32 × 36 | a ducking player |

The boxes are centred on the traced point: a standing player's origin is
the centre of its box, 36 units above its feet.

Checked on de_dust2's model 1, a box from (−1472, 2592, 0) to (−1344,
2656, 64). Its hull 0 tree splits on exactly those six planes. Its hull
1 tree's planes are x = −1488 and −1328, y = 2576 and 2672, z = −36 and
100 (each face moved out by 16, 16 and 36). Hull 2's: x = −1504 and
−1312, y = 2560 and 2688, z = −32 and 96. Hull 3's: x = −1488 and −1328,
y = 2576 and 2672, z = −18 and 82. And on the spawns: the 18 usual
`info_player_start` points of de_dust2 are 40 units above the floor
in hull 0 and 4 units above solid in hull 1 (the 36-unit half-height,
plus 4).

### How a clip tree describes solid space

A clip node (8 bytes): plane index (i32), then two children (2 × i16),
front then back. A child ≥ 0 is another clip node; a child < 0 is **a
contents value itself** (−1 empty, −2 solid, −6 sky...), not a leaf index
as in hull 0. The hull's root is the model's head for that hull.

To find the contents of point p in hull h of a model: start at the
model's head for h; at each node take the front child when plane normal
· p − distance ≥ 0, else the back child; stop at a negative value, which
is the answer. Hull 0 does the same over nodes and returns the reached
leaf's contents. The space a hull calls solid (or sky) is where the
corresponding box may not have its centre.

De_dust2 model 1's hull 1 tree, as an example of the shape: root plane
x = −1328 (front: empty); then x = −1488 (back: empty); then z = −36
(back: empty); then z = 100 (front: empty); then y = 2576 (back: empty);
then y = 2672: front empty, back solid. Six nodes, one solid end.

### What the clip hulls add and how faces are grown

- **Clip brushes** (`clip` texture) are solid in hulls 1-3 only; hull 0
  has no trace of them. Player clip in CS maps lives only in the clip
  hulls.
- Axial faces are moved out by exactly the box's half size on that axis
  (above).
- **Non-axial faces are not moved by the box's full support distance.**
  Measured over the small brush models of the 25 CS maps (9 625 pairs of
  a hull 0 plane and the same-normal plane in hulls 1-3): vertical
  non-axial faces (walls at an angle) are moved out by exactly the
  horizontal half size (16 in hulls 1 and 3, 32 in hull 2) in most cases,
  within about ±1.5 units in the rest, where the full support distance
  would be up to 22.6 (hulls 1, 3) or 45.3 (hull 2). Sloped faces are
  moved by roughly the larger of (horizontal half size × the normal's
  horizontal length) and (vertical half size × |normal z|), up to a few
  units more, and never more than the support distance. Example
  (as_oilrig model 14, a tilted plank): the face with normal (0, 0.972,
  0.236) is at 83.87 in hull 0, 100.34 in hull 1 (+16.46), 115.65 in hull
  2 (+31.78) and 99.39 in hull 3 (+15.52); the face with normal (0,
  −0.236, 0.972) at 40.38, 74.36 (+33.99), 72.39 (+32.01) and 57.84
  (+17.46).
- The compiler also adds bevel planes (axial planes at the grown box's
  extremes, and planes along grown edges), so a clip tree has more
  planes than its brushes had faces.

Consequence (a quirk players see): a box can stand a few units closer
to a diagonal wall than its corners allow; its corners pass into the
wall by up to (√2 − 1) × 16 ≈ 6.6 units for a 45° wall in hull 1. A
reimplementation that traces the clip hulls reproduces it; one that
sweeps a true box against reconstructed brushes does not.

### Reconstructing shapes (for engines that want solids)

The plan's collision for other games' characters (goldsrc.md §3) builds
convex shapes from hull 0's solid leaves, plus the volumes solid in hull
1 but not explained by hull 0. Facts this needs:

- Hull 0's solid leaves are convex: intersect the half-spaces of the
  planes on the path from the root (the side taken at each node). All of
  them are the shared leaf 0, so reconstruct per path, not per leaf.
- A clip hull's solid regions are likewise convex per path.
- Shrinking a hull 1 region back to the original solid: subtract 16, 16
  or 36 from axial planes' distances exactly; for non-axial planes use
  the measured offsets above, or better, the hull 0 plane with the same
  normal when one exists. The bevel planes must be dropped, not shrunk.

### Hull traces

A hull trace moves a point from start to end through one hull of one
model and reports the first contact. Results:

| Result (ours) | Meaning |
|---|---|
| fraction | 0 to 1: how far along start→end the point got (1 = no contact) |
| end | start + fraction × (end − start) |
| plane | the plane (normal, distance) that was hit, facing the open side |
| started_solid | the start point is in a solid or sky region |
| all_solid | the whole path is in solid or sky |
| in_open, in_water | whether the path passed through empty or water regions |

Behaviour:

1. Walk the tree from the root with the segment. When both ends are on
   the same side of a node's plane, go down that side only. When they
   straddle it, split the segment at the plane, go down the near side
   first with the near part, then the far side with the far part.
2. At a contents end: an empty or water part clears all_solid and sets
   in_open or in_water; a solid or sky part at the segment's start sets
   started_solid.
3. The first crossing from a non-solid side into a solid side is the
   hit. The fraction is put so that the end is **1/32 unit short of the
   plane**, measured along its normal (the fraction is computed from the
   two ends' plane distances with 1/32 subtracted on the near side, then
   clamped to 0-1). If that end point is still solid (possible near
   other planes), the fraction is pulled back in steps of 0.1 of the
   segment part until it is not, or reaches the part's start.
4. When the trace is all_solid, started_solid is set too; when
   started_solid, the movement code treats the fraction as 0 (the
   player does not move).

Against several models, the world and each solid brush entity's model
are traced (the entity's origin subtracted from start and end, and for
a rotated entity both rotated into its frame and the plane normal
rotated back) and the nearest hit wins.

Which hull a box uses: the player's movement chooses hull 1 standing and
hull 3 ducked. For other boxes the engine picks by size: a box at most
8 units wide uses hull 0; up to 36 wide, hull 3 if at most 36 tall, else
hull 1; wider, hull 2 (thresholds to confirm, open question 2). A box
whose minimum differs from the chosen hull's is traced with the
difference added to start and end, so the hull's box shares its minimum
corner.

## Brush entities and render modes

A brush entity draws its model's faces with its render keys:

| `rendermode` | Name (ours) | Drawing |
|---|---|---|
| 0 | normal | opaque, lightmapped |
| 1 | colour | flat `rendercolor`, `renderamt` as alpha |
| 2 | texture | texture blended by `renderamt` / 255, lightmapped |
| 3 | glow | sprites only; not used on brushes |
| 4 | solid | alpha-tested: texel index 255 of a `{` texture is a hole; `renderamt` 255 |
| 5 | additive | texture added, scaled by `renderamt` / 255 |

`renderamt` is 0-255, `rendercolor` "r g b", `renderfx` a small effect
number (pulse, fade, hologram...). Counted in the stock maps: `func_wall`
mode 0 (1 110), mode 4 (477), mode 2 (65); `func_illusionary` modes 4
(473), 0 (418), 2 (84), 5 (27); `func_breakable` 0 (221), 2 (100), 4
(30); `func_water` mode 2 (46). `func_illusionary` is never solid;
trigger brush entities (`trigger_*`, `func_buyzone`, `func_bomb_target`,
`func_hostage_rescue`) are never drawn; `func_ladder` is drawn only
when its render mode says so (invisible in practice).

## Texture name prefixes

| Name | Meaning |
|---|---|
| `{name` | alpha-tested: index 255 is transparent (drawn as a hole with rendermode 4) |
| `!name` | water/liquid: drawn with a sine warp, turbulent; the face is lit in the file (open question 4) |
| `+0name`..`+9name` | animation frames 0-9, played in order at 10 frames a second |
| `+aname`..`+jname` | the alternate animation, shown while a brush entity is toggled on (buttons) |
| `-0name`..`-9name` | random tiling: each face (or tile) shows one of the variants, chosen at random |
| `sky` | not drawn; the sky box is seen through it; unlit (texinfo flag 1) |
| `aaatrigger` | trigger volumes; not drawn (their entities are invisible) |
| `clip` | removed from hull 0, solid in hulls 1-3 |
| `origin` | marks a brush entity's rotation origin; removed |
| `null` | removed face, solid |
| `scroll...` | scrolled by `func_conveyor` |

Names are case-insensitive and at most 15 characters. Counted over the
stock maps' 2 893 textures: 200 with `-`, 190 with `+`, 180 with `{`, 36
with `~` (a naming habit for light-emitting textures; no meaning to the
engine), 25 `sky`, 9 with `!`.

## Edge cases

- A texture directory entry of −1 (not in this install) is a missing
  texture; use a placeholder.
- A non-embedded texture found in no WAD: placeholder (none in the stock
  maps).
- Faces with lightmap offset −1 are drawn fullbright (sky never drawn).
- The visibility lump may be empty (unvised maps): everything visible.
- Plane normals of axial planes may be negative (the pairs); never assume
  kind 0-2 means a positive normal.

## Test cases

| Setup | Input | Expected |
|---|---|---|
| de_dust2.bsp | read the header | version 30; planes at 124, 191 640 bytes; models 43 |
| de_dust2.bsp | count records | 9 582 planes, 2 766 nodes, 8 321 clip nodes, 1 455 leaves, 5 383 faces, 638 texinfo |
| de_dust2.bsp | textures | 44, 15 embedded; texture 8 `-0SandRock2` 128 × 256, mip offsets 40, 32 808, 41 000, 43 048 |
| de_dust2.bsp | face 0 lightmap | 7 × 3 samples, style 0 only, offset 0 |
| every stock map | size every lit face's lightmap | blocks tile the lighting lump exactly |
| de_dust2.bsp model 1 | hull heads | 2513, 7358, 7364, 7370 |
| de_dust2.bsp model 1 | point contents hull 1 at (−1400, 2620, 99) / (−1400, 2620, 101) | solid / empty |
| de_dust2.bsp model 1 | point contents hull 3 at (−1400, 2620, 81) / (−1400, 2620, 83) | solid / empty |
| de_dust2.bsp | hull 1 contents 4.5 units below a usual `info_player_start` | solid; 4 units below: empty |
| de_dust2.bsp | leaf 0 | contents −2 |
| de_dust2.bsp | leaf 1 visibility | 196 leaves visible |
| de_dust2.bsp worldspawn | `wad`, `skyname`, `MaxRange` | three WAD names (`halflife.wad`, `decals.wad`, `cs_dust.wad`), `des`, 8192 |
| a hull 1 tree for a flat floor at z = 0 (its hull 1 plane at z = 36) | trace from z = 100 to z = 0 | fraction = (64 − 1/32) / 100 = 0.6396875, end z = 36.03125, plane normal (0, 0, 1), started_solid false |
| same | trace from z = 20 to z = 0 | started_solid and all_solid true, fraction treated as 0 |

## Open questions

1. The lightmap brightening (the original's gamma and brightness
   settings and their defaults) for refcmp: measure from captures of the
   client (not on this box).
2. The engine's hull choice thresholds for boxes other than the player's
   (8 and 36 units above) come from public engine write-ups; confirm by
   measurement before relying on them (CS 1.6's own player uses hulls 1
   and 3 directly).
3. Face winding direction for back-face culling: confirm on one face
   against a screenshot.
4. `!` water faces carry lightmaps in the file; whether the original
   draws water with them or fullbright needs a capture.
5. The compiler's exact rule for growing non-axial faces is not needed
   (the hulls are data), but a reconstruction that shrinks hull 1 back
   should be checked against hull 0 on the 25 maps (the M0 spike).
