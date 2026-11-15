# Halo: Combat Evolved: structure BSP

Source basis: the Xbox disc's map files, decoded field by field with
throwaway scripts (all 13 multiplayer maps, plus the campaign maps a10
and b40 for the multi-BSP layout), and the game's level loading, level
rendering, visibility, fog and lighting code from the behaviour
reference.
Status: draft

Sections: Summary; Units and conventions; Locating a BSP in the map
file; The BSP region header; Top-level layout; Render geometry;
Lightmaps; Drawing order and transparency; Visibility and partition;
Other blocks; Collision BSP and collision materials; World bounds,
lighting, vehicles and sky; Measured facts; Quirks; Test cases;
Confidence; Open questions.

---

## Summary

- A level's geometry is one or more structure BSP tags (class `sbsp`).
  Every multiplayer map has exactly one. Unlike every other tag, a BSP's
  data is not in the map's tag data: it is a separate region of the map
  file with its own address base, listed by the scenario, and loaded so
  that it ends at the top of the tag cache.
- Render geometry is grouped by **lightmap group** (one lightmap page
  each), and inside each group by **material** (one shader each). A
  material owns its own vertex array (32-byte compressed vertices on the
  Xbox) and a parallel array of 8-byte lightmap vertices, and a
  contiguous range of the BSP's triangle list, whose indices are local to
  the material's vertices.
- Visibility is by **clusters** joined by **portals**, with a
  precomputed potentially visible set; each cluster lists its
  triangles through **subclusters** (bounding box plus triangle indices).
  The collision BSP, which also locates the camera's cluster, sits in the
  same tag.

## Units and conventions

- Distances, axes, the tag block / data reference / tag reference forms,
  address translation and byte order are as `cache_file.md` defines them.
  Positions here are in world units, right-handed, +Z up. Convert to
  metres with the factor `cache_file.md` gives.
- "Block" below means a tag block field (12 bytes: count, address, and a
  word that is zero on the disc). "Data reference" means the 20-byte data
  field; only its size and its address are used here. "Tag reference"
  means the 16-byte field (class, path pointer, unused, tag index).
- Offsets in tables are byte offsets from the start of the structure
  they describe, in hexadecimal.
- **Every address inside a BSP's data uses the BSP's own address base**
  (see below), not the tag data's. Tag references inside the BSP still
  name ordinary tags (shaders, the lightmap bitmap) by tag index, and the
  path pointer of such a reference points into the ordinary tag data.

## Locating a BSP in the map file

The scenario tag holds a block of structure BSP references at offset
0x5A4 of its main structure (the scenario's other fields are
`scenario.md`'s). Each element is 32 bytes:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 4 | u32 | File offset of the BSP's region, in the decompressed map |
| 0x04 | 4 | u32 | Size of the region in bytes |
| 0x08 | 4 | u32 | Address base: the address the region's first byte has |
| 0x0C | 4 | u32 | Unused, 0 |
| 0x10 | 16 | tag ref | The `sbsp` tag |

- Address translation for a BSP: an address a with base ≤ a < base +
  size is at file offset (a − base) + file offset. Any other address is
  translated as ordinary tag data (`cache_file.md`).
- The `sbsp` tag's entry in the tag index has address 0: the tag index
  does not say where the BSP is. Use the scenario's reference.
- Observed on the disc (all 24 maps that have BSPs, every BSP): base +
  size = 0x819A6000, the end of the tag cache, so each BSP is placed at
  the top of the cache. Sizes and file offsets are multiples of 0x800.
  BSP regions are the first thing after the 0x800-byte file header: the
  first BSP is at file offset 0x800 and further BSPs (campaign maps)
  follow one after another. A reader must still use the stored values.
- A reader should check: size ≥ 24, offset + size within the file, the
  header's signature, and that every pointer it follows lies inside the
  region (or inside the tag data, for tag paths).

## The BSP region header

The region starts with a 24-byte header:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 4 | u32 | Address of the `sbsp` tag's main structure (inside the region) |
| 0x04 | 4 | u32 | R, count of rendered-vertex buffer descriptors |
| 0x08 | 4 | u32 | Address of R descriptors, 12 bytes each |
| 0x0C | 4 | u32 | L, count of lightmap-vertex buffer descriptors |
| 0x10 | 4 | u32 | Address of L descriptors, 12 bytes each |
| 0x14 | 4 | u32 | Signature 0x73627370 (`sbsp` read as a big-endian four-character code) |

- A buffer descriptor is 12 bytes: a word the console fills in at load
  time (0xCACACACA on the disc), the **address of the vertex data**, and
  another load-time word (0xCACACACA). Only the middle word is data.
- R = L = the total number of materials in every observed BSP. The
  descriptor arrays are in **reverse** material order: material k
  (counting across all lightmap groups in order, from 0) uses descriptor
  R − 1 − k. A reader does not need this: each material points at its
  descriptors directly (below) and also holds a data reference to the
  same bytes.
- Observed region order: header, rendered descriptors, lightmap
  descriptors, all materials' vertex data (starting at the next 32-byte
  boundary, in material order), then the tag's main structure, then its
  blocks. Nothing depends on this order.

## Top-level layout

The main structure is 0x288 bytes (648). Element sizes are in brackets.

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x000 | 16 | tag ref | Lightmap bitmap (class `bitm`) |
| 0x010 | 4 | f32 | Vehicle floor (world units) |
| 0x014 | 4 | f32 | Vehicle ceiling (world units) |
| 0x018 | 20 | | Unused, zero |
| 0x02C | 116 | lighting | Default lighting (layout under Render geometry) |
| 0x0A0 | 4 | | Unused, zero |
| 0x0A4 | 12 | block | Collision materials [20] |
| 0x0B0 | 12 | block | Collision BSP [96], one element |
| 0x0BC | 12 | block | Nodes [6] |
| 0x0C8 | 24 | 6 × f32 | World bounds: x min, x max, y min, y max, z min, z max |
| 0x0E0 | 12 | block | Leaves [16] |
| 0x0EC | 12 | block | Leaf surface references [8] |
| 0x0F8 | 12 | block | Surfaces (triangles) [6] |
| 0x104 | 12 | block | Lightmap groups [32] |
| 0x110 | 12 | | Unused, zero |
| 0x11C | 12 | block | Lens flares [16] |
| 0x128 | 12 | block | Lens flare markers [16] |
| 0x134 | 12 | block | Clusters [104] |
| 0x140 | 20 | data ref | Cluster data: the potentially visible set |
| 0x154 | 12 | block | Cluster portals [64] |
| 0x160 | 12 | | Unused, zero |
| 0x16C | 12 | block | Breakable surfaces [48] |
| 0x178 | 12 | block | Fog planes [32] |
| 0x184 | 12 | block | Fog regions [40] |
| 0x190 | 12 | block | Fog palette [136] |
| 0x19C | 24 | | Unused, zero |
| 0x1B4 | 12 | block | Weather palette [240] |
| 0x1C0 | 12 | block | Weather polyhedra [32] |
| 0x1CC | 24 | | Unused, zero |
| 0x1E4 | 12 | block | Pathfinding surfaces [1] |
| 0x1F0 | 12 | block | Pathfinding edges [1] |
| 0x1FC | 12 | block | Background sound palette [116] |
| 0x208 | 12 | block | Sound environment palette [80] |
| 0x214 | 20 | data ref | Sound cluster data |
| 0x228 | 24 | | Unused, zero |
| 0x240 | 12 | block | Markers [60] |
| 0x24C | 12 | block | Detail objects [64] |
| 0x258 | 12 | block | Runtime decals [16] |
| 0x264 | 8 | | Unused, zero |
| 0x26C | 4 | u32 | Load-time pointer; holds junk on the disc, ignore |
| 0x270 | 12 | block | Leaf map leaves [24] |
| 0x27C | 12 | block | Leaf map portals [24] |

- Blocks in the region are 4-byte aligned; an element array of 6-byte
  elements is followed by 2 bytes of padding when needed.
- The leaf map blocks are empty (count 0) in every observed BSP.

## Render geometry

### Lightmap group (32 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 2 | i16 | Bitmap index into the lightmap bitmap's images; −1 = none |
| 0x02 | 2 | | Padding |
| 0x04 | 16 | | Unused, zero |
| 0x14 | 12 | block | Materials [256] |

### Material (256 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 16 | tag ref | Shader (any shader class; see Measured facts) |
| 0x10 | 2 | i16 | Shader permutation index |
| 0x12 | 2 | u16 | Flags: bit 0 coplanar, bit 1 fog plane |
| 0x14 | 4 | i32 | First surface: index into the BSP's surface list |
| 0x18 | 4 | i32 | Surface count |
| 0x1C | 12 | 3 × f32 | Centroid (filled for transparent materials, zero otherwise) |
| 0x28 | 116 | lighting | Lighting (layout below) |
| 0x9C | 16 | plane | Plane (normal, d); filled for transparent materials |
| 0xAC | 2 | i16 | Breakable surface index; −1 = none |
| 0xAE | 2 | | Padding |
| 0xB0 | 20 | vertex buffer | Rendered vertices |
| 0xC4 | 20 | vertex buffer | Lightmap vertices |
| 0xD8 | 20 | data ref | Uncompressed vertex data: size 0 on the disc |
| 0xEC | 20 | data ref | Compressed vertex data |

Vertex buffer field (20 bytes):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 2 | i16 | Vertex form: 0 rendered uncompressed, 1 rendered compressed, 2 lightmap uncompressed, 3 lightmap compressed |
| 0x02 | 2 | | Padding |
| 0x04 | 4 | i32 | Vertex count |
| 0x08 | 4 | i32 | Offset; 0 in every observed material |
| 0x0C | 4 | u32 | Stale build-time address; ignore |
| 0x10 | 4 | u32 | Address of this material's buffer descriptor in the region header's array |

Lighting (116 bytes; also the BSP's default lighting):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 12 | rgb | Ambient colour |
| 0x0C | 2 | i16 | Distant light count (0..2) |
| 0x0E | 2 | | Padding |
| 0x10 | 24 | | Distant light 0: colour rgb, direction vector |
| 0x28 | 24 | | Distant light 1: colour rgb, direction vector |
| 0x40 | 2 | i16 | Point light count |
| 0x42 | 2 | | Padding |
| 0x44 | 8 | 2 × i32 | Point light indices |
| 0x4C | 16 | argb | Reflection tint |
| 0x5C | 12 | vector | Shadow vector |
| 0x68 | 12 | rgb | Shadow colour |

Colours are f32 components in 0..1. The materials of each observed map
carry only two distinct lighting blocks; the renderer passes a
material's lighting to transparent shaders. A first renderer can ignore
lighting.

### Vertex storage on the Xbox

- **Every material on the disc is compressed**: rendered form 1 and
  lightmap form 3, in all 13 multiplayer maps and the campaign maps
  checked. The uncompressed data reference has size 0.
- The compressed data reference's address points at the rendered
  vertices (count × 32 bytes) immediately followed by the lightmap
  vertices (lightmap count × 8 bytes). Its size is exactly 32 × count +
  8 × lightmap count. The rendered descriptor's data address equals the
  data reference's address; the lightmap descriptor's equals that plus
  32 × count. A reader should read from the data reference.
- The lightmap vertex count is either equal to the rendered count (the
  material is lightmapped) or 0 (it is not). Never anything else.

Compressed rendered vertex (32 bytes):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 12 | 3 × f32 | Position (world units) |
| 0x0C | 4 | u32 | Normal, packed |
| 0x10 | 4 | u32 | Binormal, packed |
| 0x14 | 4 | u32 | Tangent, packed |
| 0x18 | 8 | 2 × f32 | Base texture coordinates u, v (not packed) |

Compressed lightmap vertex (8 bytes):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 4 | u32 | Incident radiosity direction, packed like a normal |
| 0x04 | 2 | i16 | Lightmap u, packed |
| 0x06 | 2 | i16 | Lightmap v, packed |

Uncompressed forms, for completeness (none on the disc): rendered is 56
bytes (position, normal, binormal, tangent as 3 × f32 each, then u, v as
f32); lightmap is 20 bytes (incident radiosity 3 × f32, then u, v f32).

**Unpacking a vector** from a u32 w. Split it into three signed fields:
x = bits 0–10 (11 bits), y = bits 11–21 (11 bits), z = bits 22–31 (10
bits), each read as two's complement in its width (x, y in −1024..1023,
z in −512..511). Then

  component x = (2x + 1) / 2047, component y = (2y + 1) / 2047,
  component z = (2z + 1) / 1023.

So raw 0 decodes to a small positive value (1/2047 or 1/1023), not 0,
and the extremes decode to exactly −1 and +1. The game packs with
x = floor(c · 1023.5), y likewise, z = floor(c · 511.5), each masked
to its width, after clamping c to −1..1.

**Unpacking a 16-bit coordinate** from a signed i16 s:
  value = (2s + 1) / 65535,
so s = 32767 gives 1.0 and s = −32768 gives −1.0. The game packs with
s = floor(c · 32767.5) after clamping c to −1..1.

Measured on Blood Gulch: every decoded normal has length within 0.0011
of 1; incident radiosity directions have lengths 0.58..1.0 (not unit:
their length encodes how directional the incoming light is); tangents
and binormals are not exactly orthogonal to the normal (some vertices
are far off), but the tangent follows increasing u and the binormal
increasing v in 99% of triangles, and binormal × tangent points along
the normal in about 92% of them.

### Surfaces and triangle lists

- A surface is one triangle: three u16 vertex indices (6 bytes). The
  indices are **local to the material** that owns the surface: they
  index that material's rendered vertices (and, equally, its lightmap
  vertices).
- Material m owns surfaces first(m) .. first(m) + count(m) − 1. Across
  lightmap groups in order and materials in order, the ranges are
  contiguous, start at 0 and end at the surface count, in every observed
  BSP. The sum of material surface counts equals the surface block's
  count.
- To draw a material: for each owned surface s, the triangle is
  (v[i0], v[i1], v[i2]) with i0, i1, i2 its three indices. It is a plain
  triangle list; there are no strips and no restart values.
- **Winding and facing.** The front of a triangle is the side its vertex
  normals point to. Seen from the front, the three vertices run
  **clockwise**; equivalently, (p1 − p0) × (p2 − p0) points away from
  the normals, to the back. On the disc this holds for every triangle of
  the 13 multiplayer maps except one sliver on Blood Gulch. The game
  culls the faces that appear counter-clockwise on screen, so
  back-face culling with "clockwise is front" draws the level correctly.
  A converter to a counter-clockwise-front convention swaps i1 and i2.

## Lightmaps

- The lightmap bitmap (top-level tag reference) is a `bitm` tag whose
  image list holds one image ("page") per lit lightmap group. A group's
  bitmap index is the index of its page in that list. On the disc,
  group i uses page i for every lit group, and the page count is the
  number of lit groups.
- All pages on the disc are 2D, one mip level, 16-bit colour (bitmap
  format code 6; image byte size = width × height × 2). Decoding them,
  including the Xbox's swizzled storage, is `bitmaps.md`'s.
- Lightmap coordinates come from the lightmap vertices, one per rendered
  vertex. Observed range 0.0019..0.998 on every map: inside 0..1 with
  about half a texel of margin. The game samples the page with clamping
  and bilinear filtering. Orientation follows the console's texture
  convention: (0, 0) is the first texel of the first stored row; u runs
  along a row, v down the rows (inferred, see Open questions).
- Base texture coordinates (in the rendered vertex) are unbounded and
  tile (Blood Gulch: u −6.3..15.6, v −5.7..6.7); the shader's map
  scales apply on top.
- **How the light combines.** The game draws opaque level geometry in
  passes: first a lightmap pass writes the light (lightmap colour,
  modulated by the bump map against the incident radiosity direction,
  times the shader's material colour) into the frame, then the diffuse
  texture pass multiplies the frame by the shader's texture output.
  For a first renderer: colour = lightmap texel × base texture. Whether
  the texture pass adds a ×2 scale and how detail maps blend is
  `shaders.md`'s (Open questions).
- **No lightmap.** A group whose bitmap index is −1 has no page. In all
  13 multiplayer maps there is exactly one such group and it is the
  **last** group; every material in it has lightmap vertex count 0, and
  every one uses a transparent shader class (`sotr`, `swat`, `schi`).
  Some campaign BSPs have no such group (their last group has a page).
  Every material in a lit group has as many lightmap vertices as
  rendered vertices, and is `senv` except for some glass (`sgla`)
  materials, which are lit and transparent. If the lightmap bitmap
  reference itself is none, the game draws everything in a debug
  "no lightmap" mode; no disc map does this.

## Drawing order and transparency

- The game builds a set of visible surfaces each frame (below), then
  walks lightmap groups in order, and in each group its materials in
  order, drawing the visible surfaces of each material as one batch.
  Each group binds its page before its materials are drawn.
- A material whose shader class is transparent is not drawn in the
  opaque passes; it is handed to the transparent-geometry pass with its
  centroid (for sorting back to front), its plane when the coplanar flag
  is set, a fog offset when the fog-plane flag is set, and its lighting.
- A material with a breakable surface index draws only while that
  breakable surface is intact (two materials on Prisoner).
- The shader permutation index (0 or 1 observed) selects among a
  shader's permutations; `shaders.md` defines that.

## Visibility and partition

### Clusters (104 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 2 | i16 | Sky index into the scenario's sky list; −1 = none |
| 0x02 | 2 | i16 | Fog: −1 none; with bit 15 set, bits 0–14 are a fog plane index; else a fog region index |
| 0x04 | 2 | i16 | Background sound palette index; −1 = none |
| 0x06 | 2 | i16 | Sound environment palette index; −1 = none |
| 0x08 | 2 | i16 | Weather palette index; −1 = none |
| 0x0A | 2 | i16 | Transition structure BSP index (0 on every multiplayer map) |
| 0x0C | 2 | i16 | First runtime decal index; −1 = none |
| 0x0E | 2 | u16 | Runtime decal count |
| 0x10 | 24 | | Unused, zero |
| 0x28 | 12 | block | Predicted resources [8] |
| 0x34 | 12 | block | Subclusters [36] |
| 0x40 | 2 | u16 | First lens flare marker index |
| 0x42 | 2 | u16 | Lens flare marker count |
| 0x44 | 12 | block | Surface indices [4]; empty on the disc |
| 0x50 | 12 | block | Mirrors [64]; empty on the disc |
| 0x5C | 12 | block | Portal indices [2]: i16 indices into the cluster portals |

Subcluster (36 bytes): world bounds as 6 × f32 (x min, x max, y min,
y max, z min, z max), then a block of i32 surface indices (global
indices into the BSP's surface list). A surface can be listed by several
subclusters; on Blood Gulch the subclusters list 6325 indices covering
all 5503 surfaces.

Predicted resource (8 bytes): i16 type, i16 resource index, u32 tag
index. A renderer may use it to preload textures; it is not needed to
draw.

Mirror (64 bytes): plane (16), 20 unused bytes, shader tag reference
(16), block of 12-byte points. None on the multiplayer maps.

### Cluster portals (64 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 2 | i16 | Cluster A |
| 0x02 | 2 | i16 | Cluster B |
| 0x04 | 4 | i32 | Plane index into the collision BSP's plane list |
| 0x08 | 12 | 3 × f32 | Centroid |
| 0x14 | 4 | f32 | Bounding radius about the centroid |
| 0x18 | 4 | u32 | Flags (0 on every multiplayer portal) |
| 0x1C | 24 | | Unused, zero |
| 0x34 | 12 | block | Vertices [12]: the portal polygon, 3 × f32 each |

On Blood Gulch every portal vertex lies on its plane (error below
0.00002) and within the bounding radius of the centroid.

### Leaves and how the camera finds its cluster

- Leaf (16 bytes): 6 bytes of per-axis byte bounds (meaning unknown;
  0, 255, 0, 255, 0, 255 on the leaves looked at), 2 bytes padding, i16
  cluster index, i16 surface reference count, i32 first surface
  reference index.
- Surface reference (8 bytes): i32 surface index (global), i32 node
  index (into the collision BSP's 3D nodes; −1 occurs). Leaves use them
  to list the surfaces that cross each leaf (decals and lighting queries
  use this; drawing does not).
- The structure's leaves run parallel to the collision BSP's leaves
  (same count on every map): the game finds the camera's leaf with the
  collision BSP's point test (`collision.md`) and takes that leaf's
  cluster. If the point is in no leaf, it keeps the previous frame's
  leaf. Every leaf has a valid cluster on the disc.
- Nodes (6 bytes each, same count as the collision BSP's 3D nodes):
  contents unknown. A renderer skips them.

### Potentially visible set (cluster data)

- With N clusters and W = ceil(N / 32), the data is N rows of W u32
  words. Bit j of row i (word j / 32, bit j mod 32, least significant
  first) is set when cluster j may be visible from cluster i. Size = 4 ×
  N × W bytes (Blood Gulch: N = 30, 120 bytes).
- On the disc every cluster sees itself and the relation is symmetric.
  Density (set bits over N x N) varies: Blood Gulch 716 of 900 =
  79.56%, Sidewinder 495 of 1225 = 40.40%, Boarding Action 100%, Damnation
  95.83%, Chill Out 98.0%, Hang 'Em High 1 of 1.

### How the game culls (reference only)

1. Find the camera's cluster (above). Its sky index chooses the sky.
2. From the camera's cluster, flood through portals: for each portal
   index of the current cluster, the neighbour is the portal's other
   cluster; skip it if it is already on the current path or not in the
   camera cluster's visible set. Project the portal polygon to the
   screen, intersect it with the current visible region, and recurse
   into the neighbour with the smaller region if the intersection is
   not empty (a degenerate projection passes the region through
   unchanged). Portals farther than the far clip plane are skipped
   unless the cluster's sky has a model. Each reached cluster records
   the union of regions it was reached through, which narrows its
   frustum.
3. For each reached cluster, test each subcluster's box against that
   cluster's frustum; add the surface indices of passing subclusters to
   the visible set. The Xbox game stops at 16384 surfaces and 128
   clusters per frame.
4. Draw as in Drawing order.

A first renderer can skip all of this and draw every material.

## Other blocks

| Block | Element | Contents |
|---|---|---|
| Lens flares | 16 | tag reference to a `lens` tag |
| Lens flare markers | 16 | position 3 × f32; direction as 3 × i8 (component = value / 127); u8 lens flare index |
| Breakable surfaces | 48 | centroid 3 × f32, radius f32, i32 collision surface index, 28 unused |
| Fog planes | 32 | i16 fog region index, i16 runtime material type (−1 on disc), plane (16), block of 12-byte vertices |
| Fog regions | 40 | 36 unused, i16 fog palette index, i16 weather palette index (−1 = none) |
| Fog palette | 136 | name (32 chars), tag reference to a `fog ` tag, 4 unused, scale function name (32 chars), 52 unused |
| Weather palette | 240 | name (32), particle system tag reference (class `rain`; none on every multiplayer map), 4 unused, scale function name (32), 44 unused, wind tag reference (class `wind`), wind direction 3 × f32, wind magnitude f32, 4 unused, wind scale function name (32), 44 unused |
| Weather polyhedra | 32 | centre 3 × f32, radius f32, 4 unused, block of planes [16]; none on the disc's multiplayer maps |
| Pathfinding surfaces | 1 | one byte per collision BSP surface (counts match); for AI |
| Pathfinding edges | 1 | empty on the disc |
| Background sound palette | 116 | name (32), tag reference to an `lsnd` tag, 4 unused, scale function name (32), 32 unused |
| Sound environment palette | 80 | name (32), tag reference to a `snde` tag, 32 unused |
| Markers | 60 | name (32 chars), rotation quaternion i, j, k, w (4 × f32), position 3 × f32 |
| Detail objects | 64 | four blocks (cells, instances, counts, z reference vectors) and 16 unused; one element with all blocks empty on every multiplayer map |
| Runtime decals | 16 | position 3 × f32, u8 decal palette index (scenario's), u8 unused, i8 yaw, i8 pitch |
| Leaf map leaves | 24 | two blocks; empty on the disc |
| Leaf map portals | 24 | i32 plane index, two i32 leaf indices, block of points; empty on the disc |

- Names are NUL-padded 32-byte ASCII strings.
- **Fog lookup.** A cluster's fog field with bit 15 clear names a fog
  region directly. With bit 15 set it names a fog plane: a point is
  in the plane's fog region when its signed distance to the plane plus
  the fog tag's plane distance is below 0 (the fog volume lies behind
  the plane), otherwise in no fog. The fog region's palette entry gives
  the fog tag. On the disc, Beaver Creek, Chill Out and Damnation have
  one fog plane each (horizontal, normal +Z) and every cluster uses it.
- **Weather.** A fog region's weather index, if not −1, overrides the
  cluster's. The palette's wind applies even without a particle
  system. The fields after the wind tag reference are uncertain (Open
  questions).
- **Sound cluster data** is the upper triangle of an N × N table of
  bytes, one per pair of distinct clusters: N(N − 1)/2 bytes (plus
  alignment padding). For clusters i < j the byte is at
  i(N − 1) − i(i + 1)/2 + j − 1. It is an encoded sound propagation
  distance (0 for a cluster to itself; 255 means unreachable); AI uses
  it. Single-cluster maps have size 0.

## Collision BSP and collision materials

- The collision BSP is the single element of the block at 0x0B0. Its
  96 bytes are eight blocks, with element sizes in order: 3D nodes 12,
  planes 16, leaves 8, 2D references 8, 2D nodes 20, surfaces 12,
  edges 24, vertices 16. Their layout and the trace algorithm are
  `collision.md`'s. Its planes are also the cluster portals' planes.
- Collision material (20 bytes): shader tag reference (16), 2 bytes
  padding, i16 material type (observed 1, 2 and 7 on Blood Gulch;
  it matches the material type of the shader, whose list of names is
  defined with the shaders and damage specs). Collision surfaces
  index this list.

## World bounds, lighting, vehicles and sky

- **World bounds** (0x0C8) equal, to the last bit on every multiplayer
  map, the bounding box of all rendered vertex positions. A converter
  can use them for the level's extent and as a sanity check.
- **Default lighting** (0x02C) is all zero on every multiplayer map.
  The game lights objects from the lightmap below them; it uses this
  block instead of its built-in default only when the ambient red is
  not 0.
- **Vehicle floor and ceiling** (0x010, 0x014) are 0 on every
  multiplayer map. Meaning when non-zero: heights that bound where
  vehicles may go (inferred; the vehicle specs decide).
- **Sky hookup.** A cluster's sky index is an index into the scenario's
  sky list (`scenario.md`); −1 means the cluster shows no sky. The game
  uses the sky of the camera's cluster. On the disc: Chill Out, Chiron
  TL-34 (putput) and Rat Race have no sky in any cluster; Beaver Creek
  has two clusters without; every other multiplayer cluster uses sky 0.

## Measured facts

All values are from the Xbox disc's maps. Every multiplayer map has one
BSP, at file offset 0x800, and every material is compressed.

### Per map

| Map | Groups (lit) | Materials | Triangles | Vertices | Lightmap vertices | Clusters | Portals | Pages |
|---|---|---|---|---|---|---|---|---|
| beavercreek | 14 (13) | 65 | 3317 | 4242 | 4043 | 29 | 65 | 13 |
| bloodgulch | 16 (15) | 79 | 5503 | 5790 | 5534 | 30 | 70 | 15 |
| boardingaction | 26 (25) | 173 | 7142 | 12052 | 11452 | 41 | 138 | 25 |
| carousel | 29 (28) | 123 | 2940 | 4692 | 3044 | 36 | 90 | 28 |
| chillout | 10 (9) | 163 | 3108 | 5399 | 4700 | 10 | 14 | 9 |
| damnation | 11 (10) | 69 | 8618 | 11664 | 10076 | 12 | 45 | 10 |
| hangemhigh | 3 (2) | 186 | 3218 | 5381 | 4655 | 1 | 0 | 2 |
| longest | 2 (1) | 63 | 2319 | 3832 | 3455 | 1 | 0 | 1 |
| prisoner | 3 (2) | 79 | 5234 | 9260 | 8996 | 1 | 0 | 2 |
| putput | 10 (9) | 67 | 10494 | 16555 | 15835 | 9 | 0 | 9 |
| ratrace | 12 (11) | 146 | 2692 | 4728 | 3976 | 16 | 36 | 11 |
| sidewinder | 24 (23) | 136 | 10036 | 14234 | 13918 | 35 | 104 | 23 |
| wizard | 12 (11) | 47 | 2972 | 5118 | 4814 | 18 | 40 | 11 |

"Triangles" equals the surface block's count on every map. "Vertices"
is the sum over materials (vertices are not shared between materials).

Distinct shaders per map, by class:

| Map | senv | sotr | sgla | swat | schi | Total |
|---|---|---|---|---|---|---|
| beavercreek | 13 | 4 | | 1 | | 18 |
| bloodgulch | 10 | 7 | | | | 17 |
| boardingaction | 10 | 4 | | | | 14 |
| carousel | 8 | 9 | 1 | | | 18 |
| chillout | 15 | 6 | 1 | | | 22 |
| damnation | 12 | 5 | | | 1 | 18 |
| hangemhigh | 9 | 1 | | | | 10 |
| longest | 9 | 5 | | | | 14 |
| prisoner | 10 | 7 | 1 | | | 18 |
| putput | 12 | 1 | | | | 13 |
| ratrace | 13 | 6 | | | | 19 |
| sidewinder | 12 | 5 | | | | 17 |
| wizard | 7 | 3 | | | | 10 |
| **All 13 (distinct paths)** | **72** | **59** | **2** | **1** | **1** | **135** |

Classes: `senv` shader_environment (all opaque lit geometry), `sotr`
shader_transparent_generic (the Xbox's generic transparent shader:
lights, force fields, moss cards), `sgla` shader_transparent_glass,
`swat` shader_transparent_water, `schi` shader_transparent_chicago. No
other class appears in level geometry.

World bounds (world units):

| Map | x | y | z |
|---|---|---|---|
| beavercreek | −7.423 .. 35.439 | 2.458 .. 23.563 | −1.356 .. 11.085 |
| bloodgulch | 5.907 .. 131.902 | −190.219 .. −45.057 | −0.349 .. 26.233 |
| boardingaction | −57.421 .. 77.410 | −97.281 .. 97.278 | −71.388 .. 65.668 |
| carousel | −16.010 .. 16.010 | −16.010 .. 16.010 | −4.360 .. 2.344 |
| chillout | −10.238 .. 12.815 | −8.177 .. 12.967 | 0 .. 6.691 |
| damnation | −15.871 .. 12.102 | −15.725 .. 16.150 | −50.000 .. 18.200 |
| hangemhigh | 5.900 .. 35.878 | −22.823 .. 15.823 | −9.253 .. 8.183 |
| longest | −17.378 .. 15.622 | −22.442 .. −6.623 | −0.600 .. 4.854 |
| prisoner | −12.600 .. 11.000 | −7.000 .. 7.600 | −0.528 .. 8.712 |
| putput | −19.757 .. 35.895 | −36.589 .. −0.894 | −0.198 .. 4.802 |
| ratrace | −8.844 .. 23.502 | −27.937 .. 3.563 | −3.762 .. 4.786 |
| sidewinder | −54.172 .. 54.767 | −48.468 .. 61.172 | −4.475 .. 7.220 |
| wizard | −12.649 .. 12.647 | −12.651 .. 12.646 | −4.500 .. 4.703 |

### Blood Gulch in detail

- Region: file offset 0x800, size 0x13C800, base 0x81869800. Header: 79
  and 79 descriptors; main structure at 0x818A2120.
- Lightmap bitmap `levels\test\bloodgulch\bloodgulch`, 15 pages (width
  × height): 256×128, 128×256, 64×32, 64×64, 128×128, 32×32, 64×32,
  8×8, 128×128, 32×16, 16×32, 32×16, 32×16, 128×64, 64×128.
- Groups 0–14 are lit (35 `senv` materials, 5371 triangles); group 15
  has no page and 44 `sotr` materials (132 triangles: moss cards,
  teleporter shields, coloured base lights).
- Other counts: collision materials 10, nodes 12071, leaves 5999,
  surface references 14007, clusters 30, portals 70, cluster data 120
  bytes, sound cluster data 435 bytes, weather palette 1, background
  sound palette 1, sound environments 1, runtime decals 8, pathfinding
  surfaces 4916; lens flares, markers, fog, breakables none.

### Hang 'Em High in detail

- Region: file offset 0x800, size 0x90000, base 0x81916000.
- Pages: 256×256 and 512×512. Group 0 (2 materials, 1996 triangles),
  group 1 (8 materials, 864 triangles), group 2 with no page: 176
  materials all using one `sotr` light shader (358 triangles).
- One cluster, no portals, cluster data 4 bytes, sky 0.

### Checks run on the decoded geometry (all 13 maps)

- Every position lies inside the world bounds; their bounding box
  equals them.
- Every normal has length 1 within 0.0012.
- Every triangle index is below its material's vertex count.
- Every lightmap coordinate lies in 0.0019..0.998.
- Facing as stated under Surfaces (one exception in 67593 triangles).
- Material surface ranges are contiguous and cover the surface list.
- Blood Gulch written out as a mesh: 5790 vertices, 5503 triangles,
  bounding box equal to the world bounds, total triangle area 18738
  square world units (the level footprint is 126 × 145 units, so the
  area is plausible for terrain plus cliffs and bases).

## Quirks

- The packed-vector decode never yields exactly 0: a component stored as
  0 comes back as 1/2047 (or 1/1023 for z). Keep the formula; do not
  special-case zero.
- Two buffer descriptor arrays sit in the region header in reverse
  material order, and their first and last words are filler. Use the
  materials' own data references.
- The tag index entry for a BSP has no address.

## Test cases

### Synthetic structure (unit test)

A BSP region of 0x800 bytes at file offset 0x800 with address base
B = 0x819A5800, so that it ends at 0x819A6000. The scenario's reference
holds (0x800, 0x800, 0x819A5800, 0, `sbsp` reference). All bytes not
named are 0. Addresses below are B + relative offset.

| Rel. offset | Contents |
|---|---|
| 0x000 | Header: 0x819A58E0, 1, 0x819A5818, 1, 0x819A5824, 0x73627370 |
| 0x018 | Rendered descriptor: 0xCACACACA, 0x819A5840, 0xCACACACA |
| 0x024 | Lightmap descriptor: 0xCACACACA, 0x819A58C0, 0xCACACACA |
| 0x040 | Four 32-byte compressed vertices (below) |
| 0x0C0 | Four 8-byte lightmap vertices (below) |
| 0x0E0 | Main structure (0x288 bytes) |
| 0x368 | One lightmap group |
| 0x388 | One material |
| 0x488 | Two surfaces |

Main structure: lightmap bitmap reference of class `bitm` with tag index
none; world bounds (0, 1, 0, 1, 0, 0); surfaces block (count 2, address
0x819A5C88); lightmap groups block (count 1, address 0x819A5B68); every
other block count 0.

Lightmap group: bitmap index 0; materials block (count 1, address
0x819A5B88).

Material: shader reference of class `senv` with tag index none;
permutation 0; flags 0; first surface 0; surface count 2; breakable
index −1; rendered vertex buffer (form 1, count 4, offset 0, stale 0,
descriptor 0x819A5818); lightmap vertex buffer (form 3, count 4, offset
0, stale 0, descriptor 0x819A5824); uncompressed data size 0; compressed
data size 160, address 0x819A5840.

Rendered vertices (normal 0x7FC00000, binormal 0x00200000, tangent
0x000003FF on all four):

| Vertex | Position | Texture u, v |
|---|---|---|
| 0 | (0, 0, 0) | (0, 1) |
| 1 | (1, 0, 0) | (1, 1) |
| 2 | (1, 1, 0) | (1, 0) |
| 3 | (0, 1, 0) | (0, 0) |

Lightmap vertices (incident radiosity 0x7FC00000 on all four), raw
(u, v): vertex 0 (0, 32767), vertex 1 (32767, 32767), vertex 2
(32767, 0), vertex 3 (0, 0).

Surfaces: (0, 3, 2) and (0, 2, 1).

| Check | Expected |
|---|---|
| BSP count, header signature | 1; valid |
| Lightmap groups, materials | 1 group with page 0; 1 material, compressed |
| Decoded normal (each vertex) | (0.00048852, 0.00048852, 1.0) |
| Decoded binormal | (0.00048852, −1.0, 0.00097752) |
| Decoded tangent | (1.0, 0.00048852, 0.00097752) |
| Decoded lightmap coordinates | v0 (0.0000153, 1.0), v1 (1.0, 1.0), v2 (1.0, 0.0000153), v3 (0.0000153, 0.0000153) |
| Triangles (world units) | ((0,0,0), (0,1,0), (1,1,0)) and ((0,0,0), (1,1,0), (1,0,0)) |
| Facing | (p1 − p0) × (p2 − p0) = (0, 0, −1) for both, opposite the normal: both front-facing from +Z |
| In metres | positions × the cache_file.md factor |

Decoder unit tests: packed vector 0x00000400 decodes to x = −1.0; packed
0x80000000 to z = −1.0; i16 −32768 to −1.0 and 32767 to 1.0; packed 0 to
(0.00048852, 0.00048852, 0.00097752).

### Disc-backed (slow tests)

| Map | Check | Expected |
|---|---|---|
| bloodgulch | BSP references | 1: offset 0x800, size 0x13C800, base 0x81869800 |
| bloodgulch | counts | as the per-map table; surfaces 5503, clusters 30, portals 70 |
| bloodgulch | first material | group 0 (page 0), shader `levels\test\bloodgulch\shaders\cap_cliff01b` (`senv`), surfaces 0..927, 807 vertices, 807 lightmap vertices, compressed |
| bloodgulch | first rendered vertex, raw | position (63.956402, −93.394165, 20.563074); normal 0x2635FC65, binormal 0x833C5F42, tangent 0x05A1B145; texture (0.171656, 0.277278) |
| bloodgulch | first rendered vertex, decoded | normal (−0.901319, −0.313141, 0.298143), binormal (−0.185149, −0.113825, −0.976540), tangent (0.318026, −0.947240, 0.043988) |
| bloodgulch | first lightmap vertex | raw 0x0D363484, u 25832, v 16895; decoded direction (−0.871031, −0.306302, 0.102639), coordinates (0.788357, 0.515618) |
| bloodgulch | first triangle | indices (0, 1, 2); positions (63.956402, −93.394165, 20.563074), (64.021805, −93.540810, 11.555674), (67.277100, −81.478722, 18.531078) |
| bloodgulch | world bounds | as the bounds table |
| bloodgulch | lightmap pages | 15, sizes as listed |
| hangemhigh | counts | 3 groups, 186 materials, 3218 triangles, 1 cluster, 0 portals, 2 pages (256×256, 512×512) |
| all 13 | every check in "Checks run" | holds |

## Confidence

| Section | Basis |
|---|---|
| Locating a BSP, region header, address base | Checked against the disc (all maps with BSPs) and the reference |
| Top-level layout: offsets and element sizes | Checked against the disc (element sizes from block spacing on Blood Gulch, counts on all 13 maps); names from the reference |
| Material, vertex forms, packing formulas | Checked against the disc (decoded data passes every check); formulas from the reference |
| Winding and facing | Checked against the disc; culling mode from the reference |
| Lightmap group to page mapping, page sizes | Checked against the disc |
| Lightmap orientation | Inferred |
| Light combination | Reference only, in outline; details belong to shaders.md |
| Clusters, subclusters, portals, PVS layout | Checked against the disc; meanings from the reference |
| Culling algorithm | Reference only |
| Fog lookup | Reference only; fog plane data checked on the disc |
| Other blocks | Element sizes checked on the disc where non-empty; inner fields of weather palette, detail objects, leaf map and mirrors from the reference or inferred |
| Collision BSP element sizes | Checked against the disc (Blood Gulch) |
| Default lighting, vehicle floor and ceiling meaning | Values checked (all zero); meaning reference or inferred |

## Open questions

- **Lightmap v orientation.** Stated as "first stored row is v = 0".
  Check by drawing Hang 'Em High with one page and looking for seams, or
  by comparing a lightmap's dark patch with a known shadow.
- **Texture pass scale.** Whether the diffuse pass multiplies by 2
  (making a mid-grey lightmap neutral) is for `shaders.md`; until then a
  first renderer should expose the scale as a setting.
- The 6-byte node records and the 6 byte bounds at the start of each
  leaf: unknown meaning. Nothing found needs them for drawing.
- Weather palette fields after the wind reference: the stored direction
  is not a unit vector (Blood Gulch (3, 0, 0), magnitude 2); the split
  between direction and magnitude may be different.
- Cluster field 0x0A (transition BSP) is always 0 on multiplayer maps;
  its use on campaign maps was not checked.
- Which side of a cluster portal's plane cluster A is on was not
  checked.

### Which map to draw first

**Hang 'Em High.** It has one cluster and no portals, so no visibility
work is needed and nothing can be wrongly culled; its opaque geometry is
only 10 materials on 2 lightmap pages using 9 `senv` shaders (all
metal); its 176 transparent materials share one light shader and can be
skipped at first. Blood Gulch is the better second map: 30 clusters
and 70 portals to test culling, 15 pages, a large terrain using the
environment shader's detail blending, and a sky that most of the level
shows.
