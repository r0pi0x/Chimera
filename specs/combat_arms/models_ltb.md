# Combat Arms: model files (`.ltb`)

Source basis: the engine's model loader and its renderer's mesh loaders, the model packer that writes the format, and the game code that reads model dimensions and sockets. Checked against the Steam install (build 25595979) on the Windows PC: every unencrypted model in it was parsed end to end, and the encrypted ones were checked as described under Test cases.
Status: draft

Sections: Summary; Units and conventions; Constants; File layout (header, counts block, model properties, pieces and levels of detail, mesh bodies, vertex formats, skeleton, weight sets, child models, animation section, sockets, animation bindings); Behaviour (level-of-detail choice, skinning, textures and render styles, bounds and dimensions, hit boxes, child models); Converting to a right-handed Y-up frame; Other files with model-like names; Edge cases; Quirks; Test cases; Synthetic fixture; Open questions.

---

## Summary

- A Combat Arms model is a LithTech Jupiter "D3D model" `.ltb`: a 20-byte file header, a model version (23 or 25, same layout), a block of counts, then one pass through pieces, the node tree, weight sets, child-model names, animations, sockets and per-animation boxes. There is no table of contents, no offsets and no compression: a reader walks the sections in order, and the only section it cannot skip blindly is the animation data (its size follows from counts inside it).
- Geometry is stored ready for Direct3D 9: each level of detail of a piece is one of four mesh kinds (rigid, skinned, vertex-animated, empty), with float vertices, a 16-bit triangle list and, for skinned meshes, either bone sets (groups of triangles sharing at most four bones) or a matrix palette.
- The skeleton is a depth-first tree of named nodes, each with its model-space bind matrix. Sockets hang off nodes. The stand and crouch boxes the movement code uses are the per-animation dimensions of the animations named `LST` and `LC`.
- Every model in the install uses the same layout. In the Steam build all unencrypted models are version 23 or 25; character, attachment and most weapon archives are encrypted (rez V2) and decrypt, where checked, to version 25 files.

## Units and conventions

- Byte order: little-endian everywhere. Floats are IEEE-754 binary32. There is **no alignment padding** after the 20-byte file header: a 1-byte field is followed directly by the next field.
- Integer names: u8, u16, u32 unsigned; i32 signed; f32 float.
- "string": a u16 length n, then n bytes of 8-bit text, **no** terminator. n = 0 is the empty string.
- "vec3": 3 × f32 (x, y, z). "quat": 4 × f32 in the order (x, y, z, w), w being the scalar part. "mat4": 16 × f32, described under Skeleton.
- Distance: LithTech world units ("u"), the unit of the movement and weapon specs. A character's standing box is 30 × 100 × 30 u. Nothing in the file scales it.
- Axes: +Y up, +X right, +Z forward; the frame is **left-handed**. Transforms act on column vectors (p' = M p); the translation sits in the fourth column. Conversion to a right-handed Y-up frame is its own section below.
- Angles appear only inside quaternions and matrices. Times inside the animation section are milliseconds (see the animation spec).
- File names inside the game's archives are given as archive paths, e.g. `PROPS\HEADQUATERS\HQ_FLAG01.LTB`. Name lookups are case-insensitive.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| model file type | 1 | | Byte 0 of every model file (Direct3D model) |
| container version | 9 | | u16 at byte 2; mesh loaders refuse anything else |
| minimum model version | 23 | | Lower versions are refused |
| observed model versions | 23, 25 | | Identical layout |
| max pieces | 64 | | More is a load error |
| max child models | 32 | | Count includes the model itself; 0 is a load error |
| texture slots per piece | 4 | | Fixed-size list in each LOD record |
| bone-set slots | 4 | | Unused slots hold 0xFF |
| unused bone marker | 0xFF | | In bone sets |
| max string the engine keeps | 4095 | bytes | Longer strings are truncated on load (binding names: 255) |
| mesh kind: rigid | 4 | | |
| mesh kind: skinned | 5 | | |
| mesh kind: vertex-animated | 6 | | |
| mesh kind: empty | 7 | | No geometry |
| LOD bias console variable | `ModelLODOffset`, default 0 | LODs | Added to the chosen LOD index |
| stand animation name | `LST` | | Source of the stand box |
| crouch animation name | `LC` | | Source of the crouch box |
| fallback stand / crouch half-extents | (15, 50, 15) / (15, 40, 15) | u | Used by the game only if either animation is missing |

## File layout

Sections follow each other with nothing between them, in the order of the subsections below. Offsets are absolute only in the header and counts block; after that every size is variable and offsets are relative to the start of the record being described.

### File header (20 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0 | 1 | u8 | File type. 1 = Direct3D model. The loader refuses any other value. (Render-style files use 5; see "Other files".) |
| 1 | 1 | u8 | Padding, 0. |
| 2 | 2 | u16 | Container version, 9. Checked by the mesh loaders for every non-empty LOD. |
| 4 | 1 | u8 | Reserved, 0. |
| 5 | 3 | | Padding, 0. |
| 8 | 12 | 3 × u32 | Reserved, 0. |

So bytes 0..3 of every model are `01 00 09 00` and bytes 4..19 are zero in every file observed. A reader should check bytes 0 and 2..3 and ignore the rest.

### Model version and counts block (offset 20, 64 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 20 | 4 | u32 | Model version. Must be ≥ 23. Observed 23 and 25. |
| 24 | 4 | u32 | Total keyframes over the model's own animations |
| 28 | 4 | u32 | Number of the model's own animations |
| 32 | 4 | u32 | Number of nodes |
| 36 | 4 | u32 | Number of pieces |
| 40 | 4 | u32 | Number of child models, counting the model itself |
| 44 | 4 | u32 | Total triangles over all LODs |
| 48 | 4 | u32 | Vertex count before the packer split vertices on UV seams (hint; differs from the stored count) |
| 52 | 4 | u32 | Number of vertex weights (hint) |
| 56 | 4 | u32 | Total LODs over all pieces |
| 60 | 4 | u32 | Number of sockets |
| 64 | 4 | u32 | Number of weight sets |
| 68 | 4 | u32 | Number of strings (hint) |
| 72 | 4 | u32 | Sum of string lengths (hint) |
| 76 | 4 | u32 | Bytes of vertex-animation positions in the animation section |
| 80 | 4 | u32 | Bytes of node transform tracks in the animation section |

The engine uses this block only to size one memory allocation; nothing is located by it. In every observed file the keyframe, animation, node, piece, child, triangle, LOD and socket counts equal what the rest of the file contains, and the two byte counts equal the sums defined under "Animation section". The three fields marked hint do not need to match anything. A reader may use the exact ones as a consistency check.

### Model properties (offset 84)

| Order | Type | Meaning |
|---|---|---|
| 1 | string | Command string. Usually empty. If it parses as the single token `ShadowEnable` (case-insensitive) the model casts shadows; nothing else in it is used. |
| 2 | f32 | Visibility radius in u (culling sphere around the object position). Usually 96. |
| 3 | u32 | Oriented-box count B. 0 in every observed file. |
| 4 | B × 68 bytes | Oriented boxes, each: vec3 position, vec3 size, 3 × vec3 basis, u32 node, f32 radius. The engine reads and discards them. |
| 5 | u32 | Piece count P (≤ 64). |

Then P pieces.

### Piece

| Order | Type | Meaning |
|---|---|---|
| 1 | string | Piece name (used by game code to hide or show parts). |
| 2 | u32 | LOD count L (≥ 1 in practice). |
| 3 | L × f32 | LOD distances in u. The first is always 0. |
| 4 | u32 | Old minimum LOD offset. Ignored. |
| 5 | u32 | Old maximum LOD offset. Ignored. |
| 6 | L × LOD record | One per LOD, in increasing distance order. |

### LOD record

| Order | Type | Meaning |
|---|---|---|
| 1 | u32 | Number of texture slots used, T (1 or 2 observed, at most 4). |
| 2 | 4 × i32 | Texture indices (slots beyond T are filler, typically 1, 2, 3). |
| 3 | i32 | Render-style index. |
| 4 | u8 | Render priority (0, 1 or 2 observed; see Open questions). |
| 5 | u32 | Mesh kind: 4, 5, 6 or 7. Any other value is a load error. |
| 6 | u32 | Body size S in bytes, counted from the byte after this field to the end of the body. |
| 7 | S bytes | Mesh body (next subsection). For kind 7, S = 0. |
| 8 | u8 | Used-node count U. |
| 9 | U × u8 | Node indices this LOD needs transforms for (the leaves of the used part of the tree). A rigid mesh lists its one node; vertex-animated meshes list none. |

Items 1 to 4 are stored once per LOD, but the engine keeps a single copy per piece, so the last LOD's values win; the packer writes the first LOD's values into every LOD, so they always agree. A reader can skip a body it does not understand with S, which makes the format forward-skippable at the LOD level.

### Mesh bodies

All counts are u32 unless stated. "Streams" means the four u32 vertex-stream flag words described under "Vertex formats". After the header fields come, for each of the four streams whose flag word is non-zero (in stream order), `vertex count × stride` bytes; then the triangle list: 3 × u16 per triangle, absolute vertex indices, a plain list (no strips, no fans).

**Rigid (kind 4).** All vertices follow one node.

| Order | Type | Meaning |
|---|---|---|
| 1 | u32 | Vertex count V |
| 2 | u32 | Triangle count N |
| 3 | u32 | Max bones per triangle (always 1) |
| 4 | u32 | Max bones per vertex (always 1) |
| 5 | 4 × u32 | Stream flags |
| 6 | u32 | Node index the mesh follows |
| 7 | | Vertex streams (stride without blend data), then N triangles |

Size check: S = 36 + V·stride + 6N.

**Skinned (kind 5).**

| Order | Type | Meaning |
|---|---|---|
| 1 | u32 | Vertex count V |
| 2 | u32 | Triangle count N |
| 3 | u32 | Max bones per triangle, Bt |
| 4 | u32 | Max bones per vertex, Bv |
| 5 | u8 | Re-indexed flag (palette meshes only) |
| 6 | 4 × u32 | Stream flags |
| 7 | u8 | Palette flag: 0 = bone sets, non-zero = matrix palette |

If the palette flag is 0 (bone sets; every skinned mesh in the Steam build's unencrypted archives, and all but three in the encrypted ones):

| Order | Type | Meaning |
|---|---|---|
| 8 | | Vertex streams with k = Bt − 1 blend weights (Bt from 1 to 4), then N triangles |
| 9 | u32 | Bone-set count G |
| 10 | G × 12 bytes | Bone sets, each: u16 first vertex, u16 vertex count, 4 × u8 node indices (unused slots 0xFF, used ones in increasing order), u32 end of the set's triangles in the index list, counted in **indices** (3 per triangle) |

Bone set g owns vertices [first, first + count) and the indices from the previous set's end (0 for the first) up to its own end. Sets are contiguous in both vertices and indices; together they cover all V vertices and all 3N indices; every index of a set falls inside that set's vertex range. Size check: S = 34 + V·stride + 6N + 4 + 12G.

If the palette flag is non-zero:

| Order | Type | Meaning |
|---|---|---|
| 8 | u32 | Lowest palette index used |
| 9 | u32 | Highest palette index used |
| 10 | | Only if the re-indexed flag is non-zero: u32 count R, then R × u32 node indices (palette entry i drives node list[i]) |
| 11 | | Vertex streams with k = Bv − 1 blend weights (Bv from 2 to 4) plus 4 index bytes, then N triangles |

No bone sets follow. Size check: S = 34 + 8 + (4 + 4R if re-indexed) + V·stride + 6N.

**Vertex-animated (kind 6).** The vertex positions come from the animation section, per keyframe, for one node.

| Order | Type | Meaning |
|---|---|---|
| 1 | u32 | Vertex count V (after seam splitting) |
| 2 | u32 | Animated vertex count V₀ (before splitting; the animation stores this many positions per keyframe) |
| 3 | u32 | Triangle count N |
| 4 | u32 | Max bones per triangle (0) |
| 5 | u32 | Max bones per vertex (0) |
| 6 | 4 × u32 | Stream flags |
| 7 | u32 | Node whose animation track holds the vertex positions |
| 8 | u32 | Node the mesh is drawn with (always 0, the root) |
| 9 | | Vertex streams (stride without blend data), then N triangles |
| 10 | u32 | Duplicate count D |
| 11 | D × (u16, u16) | Pairs (source, destination): after positions 0..V₀−1 are filled from the animation, copy vertex `source` to vertex `destination` |

Size check: S = 44 + V·stride + 6N + 4 + 4D.

**Empty (kind 7).** S = 0 and no body; the used-node count still follows. Used when a piece disappears at a far LOD. Not observed in the Steam build.

### Vertex formats

Each stream flag word is a bit set:

| Bit | Meaning |
|---|---|
| 0x001 | Position |
| 0x002 | Normal |
| 0x004 | Colour |
| 0x008 | Point size |
| 0x010 | 1 UV set |
| 0x020 | 2 UV sets |
| 0x040 | 3 UV sets |
| 0x080 | 4 UV sets |
| 0x100 | Basis vectors (tangent and binormal) |

**Every LOD in the install has flags (0x13, 0, 0, 0)**: one stream with position, normal and one UV set. Within a vertex the fields come in this order, each present only when its condition holds:

| Field | Size | Condition | Encoding |
|---|---|---|---|
| position | 12 | bits 0x1 and 0x2 both set | vec3, model space at bind pose |
| blend weights | 4k | skinned meshes | k × f32 (k as given per kind above) |
| blend indices | 4 | palette meshes | 4 × u8 palette indices, matching the weights in order |
| normal | 12 | bits 0x1 and 0x2 both set | vec3, model space, normally unit length |
| UVs | 8u | u = 1, 2, 3 or 4 from the lowest UV bit set | u × (f32 u, f32 v) |
| basis | 24 | bit 0x100 | vec3 tangent, vec3 binormal |

```math
\text{stride} = 24\,[\text{pos} \wedge \text{normal}] + 4k + 4\,[\text{palette}] + 8u + 24\,[\text{basis}]
```

For the format the install uses, stride = 32 + 4k (+4 for palette meshes): 32 for rigid and vertex-animated meshes, 32 / 36 / 40 / 44 for bone-set meshes with Bt = 1 / 2 / 3 / 4, and 40 / 44 / 48 for palette meshes with Bv = 2 / 3 / 4.

UVs are handed to Direct3D unchanged: origin at the top-left of the texture, v growing downwards. Values outside [0, 1] occur (textures wrap).

Bits the install never uses, for completeness: the game sizes a vertex from position, normal, blend data, UVs and basis only; it does **not** count colour or point size, while the packer writes a 4-byte colour (after the normal) when bit 0x4 is set. A file with bit 0x4 would therefore not load in the game. A reader should reject flag words other than 0 and 0x13 (or implement the table above and treat 0x4 and 0x8 as unsupported).

### Skeleton (node tree)

The node tree follows the last piece. It is written depth-first, pre-order, starting with the root; each node record is followed immediately by its children's records.

| Order | Type | Meaning |
|---|---|---|
| 1 | string | Node name |
| 2 | u16 | Node index (equals the node's position in depth-first order; the engine renumbers by that order anyway) |
| 3 | u8 | Flags. 0x2 = use only the rotation part of animation data for this node. Observed 0 and 2. |
| 4 | mat4 | Bind transform: the node's **model-space** (global) transform at bind pose |
| 5 | u32 | Child count C, then C child node records |

- The number of node records read must equal the counts block's node count (it always does). The root is usually named `Scene Root` with an identity matrix.
- Parent links are implicit: node i's parent is the node whose child list contains it; the root has none. Node indices used everywhere else (rigid nodes, bone sets, used-node lists, sockets, animation tracks, weight sets) are depth-first positions.
- Matrix layout: the 16 floats are row-major, `m[r][c]` at position 4r + c. Rows 0..2 hold the rotation/scale in columns 0..2 and the translation in column 3; row 3 is (0, 0, 0, 1) in every file. A point p in the node's space goes to model space as M p:

```math
\begin{pmatrix} x' \\ y' \\ z' \end{pmatrix} = \begin{pmatrix} m_{00} & m_{01} & m_{02} \\ m_{10} & m_{11} & m_{12} \\ m_{20} & m_{21} & m_{22} \end{pmatrix} \begin{pmatrix} x \\ y \\ z \end{pmatrix} + \begin{pmatrix} m_{03} \\ m_{13} \\ m_{23} \end{pmatrix}
```

- The node's local (parent-relative) bind transform is
```math
L_i = G_{\text{parent}(i)}^{-1}\, G_i, \qquad L_{\text{root}} = G_{\text{root}}
```
and its bind offset from its parent is the translation column of L_i (0 for the root).

### Weight sets

| Order | Type | Meaning |
|---|---|---|
| 1 | u32 | Weight-set count W |
| 2 | W × record | Each: string name, u32 count n, n × f32 |

Each set's n must equal the node count, or the model fails to load. They are per-node blend weights used when layering animations (animation spec). Observed names: `upper`, `Lower` and others; many models have none.

### Child models

| Order | Type | Meaning |
|---|---|---|
| 1 | u32 | Child count K, counting the model itself (1 ≤ K ≤ 32) |
| 2 | (K − 1) × string | File names of children 1 .. K−1 (child 0 is the model itself and has no name) |

Names are bare file names such as `st_m_child.ltb` or `Ani_AR_M4A1_SH.ltb`, resolved in the folder of the model that names them, case-insensitively. Behaviour is under "Child models" below.

### Animation section

Its content belongs to specs/combat_arms/animation.md; here only where it is and how to get past it. It starts right after the child names with a u32 animation count A. Its length is not stored, so a reader walks it as follows (all sizes in bytes):

1. For each of the A animations: vec3 dimensions (12), string name, u32 compression mode c, u32 blend time, u32 keyframe count K, then K × (u32 time, string).
2. Then one track per node, in depth-first node order:
   - c = 0: a u8 flag. Flag 0: 28K bytes. Flag non-zero (vertex-animated node): K times (u32 count n, then 12n bytes).
   - c = 1, 2 or 3: u32 count a, then a·p bytes; u32 count b, then b·q bytes; with p = 6 if c = 2, else 12; and q = 8 if c is 2 or 3, else 16.
   - Any other c is a load error.

The counts block's last field equals the sum over all tracks of the 28K, a·p and b·q byte runs, and the field before it the sum of the 12n vertex runs. The animation dimensions (step 1) are the bounding boxes described under "Bounds and dimensions". In the observed files the section ranges from about 100 bytes (props) to almost all of the file (animation libraries of tens of megabytes).

### Sockets

| Order | Type | Meaning |
|---|---|---|
| 1 | u32 | Socket count |
| 2 | per socket: u32 | Node index (must be < node count, else load error) |
| 3 | string | Socket name |
| 4 | quat | Rotation relative to the node |
| 5 | vec3 | Position relative to the node, in the node's space |
| 6 | vec3 | Scale (1, 1, 1 in every observed file) |

### Animation bindings

The last section: for each of the K child models in order (child 0 = the model itself), a u32 count, then that many records of: string animation name, vec3 dimensions, vec3 translation. The file ends here; observed files have no trailing bytes.

For child 0 the list repeats the model's own animations one for one, in the same order, with the same dimensions, plus a translation. For children 1 .. K−1 the lists are empty in every observed file.

## Behaviour

### Load order

1. Check the header (file type 1) and the version (≥ 23); read the counts block.
2. Command string, visibility radius, oriented boxes (discarded).
3. Pieces, each with its LODs; every non-empty LOD checks the container version 9.
4. Node tree; build the depth-first node list and parent links.
5. Weight sets (each must have one weight per node).
6. Child-model names.
7. Animations.
8. Sockets.
9. Bindings for each child.
10. If any read ran past the end of the data, the load fails.
11. Parse the command string; compute each node's bind offset from its parent.

Then, when the model is instanced: load the children (below), build the combined animation list and apply the bindings.

### Level-of-detail choice

Each piece chooses its LOD per frame from the distance d between the camera and the model's position (not its bounds). With LOD distances t₀ = 0 < t₁ < … < t_{L−1}:

```math
\ell(d) = \begin{cases} \min\{\, i \in [0, L-2] : d < t_{i+1} \,\} & \text{if such } i \text{ exists and } d > 0 \\ L-1 & \text{if } d \ge t_{L-1} \text{ and } d > 0 \\ 0 & \text{if } d \le 0 \end{cases}
```
```math
\ell_{\text{final}} = \operatorname{clamp}(\ell(d) + \text{ModelLODOffset},\ 0,\ L-1)
```

The distance is forced to 0 (so LOD 0) for "really close" objects (the first-person view model) and for models drawn as part of menu screens. Most pieces have one LOD; character bodies with three LODs use distances (0, 400, 800).

### Skinning

Let G_j be node j's bind transform from the file and A_j its animated model-space transform for the current frame (the animation spec says how A is built; at bind pose A_j = G_j). The engine draws with

```math
K_j = A_j\, G_j^{-1}
```

applied to bind-pose model-space vertices.

- **Rigid:** every vertex uses K of the mesh's node: p' = K_node p, n' = rot(K_node) n.
- **Bone sets:** for a vertex in set g with used bones b_0 < b_1 < … < b_{m−1} (m = number of non-0xFF slots, m ≤ Bt) and stored weights w_0 … w_{k−1}:

```math
p' = \sum_{j=0}^{m-2} w_j\, K_{b_j} p \;+\; \Big(1 - \sum_{j=0}^{m-2} w_j\Big) K_{b_{m-1}} p
```

  Only the first m − 1 stored weights are used; stored weights beyond them are 0 in practice. With m = 1 the vertex follows b_0 fully. Normals use the same weights with the rotation parts.
- **Palette:** with indices i_0..i_3 and stored weights w_0 … w_{k−1} (k = Bv − 1), the vertex blends K over nodes ν(i_0) … ν(i_k), the last weight being 1 − Σw; ν(i) = list[i] when the re-indexed flag is set, otherwise ν(i) = i. Indices beyond i_k are ignored.
- **Vertex-animated:** positions 0 .. V₀−1 for the current frame come from the named node's track (animation spec), then the duplicate pairs are copied; the mesh is drawn with K of its draw node (the root).

In every file checked the stored weights of a vertex sum to at most 1.

### Textures and render styles

A piece does not name its textures or render style. Its texture indices (slots 0 .. T−1) and its render-style index point into lists the game gives the model object when it creates it: the object's skin list (texture file names, `.dtx`) and render-style list (render-style `.ltb` files, see "Other files"). For weapons these lists are the `PVSkin*` / `HHSkin*` and `PVRenderStyle*` / `HHRenderStyle*` attributes (specs/combat_arms/attribute_data.md); characters get theirs from the model attributes. A missing entry draws the slot with no texture; a missing render style falls back to the engine's default style. Several pieces commonly share index 0.

### Bounds and dimensions

The file stores no mesh bounding box. What it does store:

- **Visibility radius** (model properties): a culling sphere radius around the object position; an instanced model uses the largest radius of itself and its children.
- **Per-animation dimensions:** half-extents (half-width X, half-height Y, half-depth Z) of an axis-aligned box centred on the object position. This is the box the game gives the object (the movement spec's hull). The value used for an animation is the one in the bindings when the binding applies, otherwise the animation's own; in every file the two agree.
- **Per-animation translation** (bindings only): a vec3 added to the root node's animated position while that animation plays, blended between the previous and current animation like the rest of the pose. Character animation libraries use (0, −half-height, 0), e.g. (0, −50, 0) for `LST` and (0, −40, 0) for `LC`, which puts the feet of the animated skeleton at the bottom of the box.

**Stand and crouch boxes:** the server and the client's mover both look up the animations named `LST` (stand) and `LC` (crouch) in the character's combined animation list (its own animations followed by its children's) and take their dimensions. If either is missing the server uses (15, 50, 15) and (15, 40, 15). In the male and female character animation libraries the values are exactly (15, 50, 15) and (15, 40, 15); the small and big creature libraries use (80, 100, 50) / (80, 70, 50) and (128, 150, 70) / (128, 120, 80).

Props carry their size the same way: for example the headquarters flag's single animation has dimensions (66.2, 32, 41.4), which the headquarters game mode reads at run time.

### Hit boxes and collision

There are no hit or collision boxes in a Combat Arms model. The oriented-box list is always empty and the engine throws it away. Hit detection uses the character's hit-box dimensions and offset from game data and a per-node hit radius from the model attribute file (specs/combat_arms/attribute_data.md); the models contribute only node positions. World collision uses the object's dimensions (above).

### Child models

- Child models provide extra animations (and must provide a matching skeleton): a body model typically has one animation of its own and names an animation library as child 1; a first-person weapon model names its animation file the same way.
- Children are loaded by name relative to the parent's folder, one level only: a child that itself has children is an error, and a missing child makes the whole model fail to load.
- The combined animation list is the model's own animations, then child 1's, then child 2's, and so on. Animation tracks map to nodes by **index**: a child must have the same node count and the same tree shape; node names may differ (they do in 14 of 283 observed parent/child pairs).
- After combining, each child's binding list (if non-empty) overrides the dimensions and translation of the animation with the same name from that child.

## Converting to a right-handed Y-up frame

Mirror the Z axis, S = diag(1, 1, −1). LithTech's forward +Z becomes −Z, which is forward in Bevy and glTF.

| Data | Conversion |
|---|---|
| positions, normals, tangents, binormals, socket positions, binding translations, node translations | (x, y, z) → (x, y, −z) |
| mat4 (bind transforms) | M' = S M S: negate m₀₂, m₁₂, m₂₀, m₂₁ and m₂₃ |
| quat (sockets, animation) | (x, y, z, w) → (−x, −y, z, w) |
| triangles | swap the second and third index of every triangle |
| dimensions (half-extents) | unchanged |
| UVs | unchanged |

Winding: in the file, for every triangle (a, b, c) the vector (p_b − p_a) × (p_c − p_a), computed with the usual component formula, points along the vertex normals, i.e. front faces are clockwise as seen from outside in the left-handed frame (Direct3D's default). After mirroring, the same order would point against the mirrored normals; swapping two indices restores it, giving counter-clockwise front faces in the right-handed frame.

Units can stay as they are (the other Combat Arms specs use u); if metres are wanted, see Open questions.

## Other files with model-like names

- **Render-style files** (`RS\*.LTB`, file type 5 at byte 0, container version 3): not models. The model loader refuses them. Their format and meaning: "Render styles and sprite skins (added 2026-10-30)" below (this replaces the earlier "not covered here").
- **Text-header `.ltb` files:** six files in `FX` (`264`, `SKYFRAME`, `TEST\PATH_FUEL`, `TEST\RING`, `TORNADORING1`, `TORNADORING2`) begin with a u16-prefixed `LTBHeader` string instead of the header above. The model loader refuses them (byte 0 is not 1); they are unused leftovers.
- **Old version:** `FX\WATERFALL2.LTB` is a model of version 18, which the loader refuses.
- **`.LTA` / `.LTC`** (in `FX`): the packer's text source and a compressed form of it. The engine never loads them.
- Models are only loaded from files ending in `.ltb`.

## Render styles and sprite skins (added 2026-10-30)

Worked out from the engine's render-style loader and render-state set-up, its sprite loader and model sprite update, and the render-style files of the install's `RS` archive (66 files, read with a throwaway reader that is not in the repo). Answers the "Weapon skins" questions 1 to 3 and "Body skins" questions 3 and 4 of the Combat Arms plan.

### Render-style file (`RS\*.LTB`)

All numbers little-endian; enums are 32-bit; a "bool" is 1 byte followed by 3 bytes of padding unless said.

1. **Header** (20 bytes): byte 0 file type (5), byte 1 unused, u16 at 2 version (3), byte 4 reserved, bytes 5–7 padding, three u32 reserved at 8, 12, 16. Anything else is refused.
2. u32 total size of the styles that follow (at most 1,000,000), u32 style count S (at most 32).
3. S times: u32 size of this style's lighting-material and pass data, then
   - **lighting material** (68 bytes): ambient, diffuse, emissive, specular as four floats each (r, g, b, a, 0..1), then the specular power (float);
   - u32 pass count P (1..4), then P passes, each a **pass record** (532 bytes, below), then 1 byte "has shader options"; if non-zero: 1 byte use-vertex-shader, i32 vertex shader id, 1 byte use-pixel-shader, i32 pixel shader id (10 bytes, unpadded);
   - after the size counted above: **effect options** (8 bytes: bool use-effect-shader, i32 effect shader id).
   The first style whose stages the device supports is used; the rest are fallbacks (every stock file has S = 1).
4. **Pass record**: four **texture stages** of 120 bytes, then blend mode, depth mode, cull mode (enums), texture factor (u32 ARGB), alpha reference (u32, 0..255), bool dynamic lighting, depth test function, alpha test function, fill mode (enums), bool bump-env-map, u32 bump stage, float bump scale, float bump offset.
5. **Texture stage** (120 bytes): eleven enums (texture, colour op, colour arg 1, colour arg 2, alpha op, alpha arg 1, alpha arg 2, coordinate source, U address, V address, filter), bool use-UV-transform, sixteen floats (a 4 × 4 matrix, row-major, applied to the generated coordinates as a row vector), bool projected, u32 coordinate count.

Enum values:

| Field | Values |
|---|---|
| texture | 0 none, 1–4 the piece's texture slot 0–3 (so 1 is the piece's first slot, 2 its second) |
| colour op | 0 off, 1 arg1, 2 arg2, 3 arg1 × arg2, 4 2 × arg1 × arg2, 5 lerp by current alpha: arg1 · a_cur + arg2 · (1 − a_cur), 6 lerp by the texture factor's alpha, 7 arg1 + arg2, 8 dot3, 9 bump-env, 10 bump-env with luminance, 11 arg1 + arg2 − 0.5, 12 2 × that, 13 arg1 − arg2, 14 arg1 + arg1.a · arg2, 15 arg1 + (1 − arg1.a) · arg2, 16 lerp by this stage's texture alpha |
| colour and alpha args | 0 current (the previous stage's result; the lit vertex colour at stage 0), 1 lit vertex colour, 2 this stage's texture, 3 the texture factor |
| alpha op | 0 off (alpha unchanged), 1 arg1, 2 arg2, 3 arg1 × arg2, 4 lerp by current alpha, 5 lerp by factor alpha, 6 add, 7 add − 0.5, 8 2 × that, 9 subtract, 10 lerp by texture alpha |
| coordinate source | 0–3 the mesh's UV sets 1–4; 4 camera-space normal, 5 camera-space position, 6 camera-space reflection vector, 7 world-space normal, 8 world-space position, 9 world-space reflection vector (x, y used as u, v unless transformed) |
| address | 0 wrap, 1 clamp, 2 mirror, 3 mirror once |
| filter | 0 point, 1 bilinear, 2 trilinear, 3 anisotropic, 4 point with point mips |
| blend | 0 none (opaque), 1 add (src + dst), 2 saturate, 3 alpha blend (src · a + dst · (1 − a)), 4 by source colour, 5 by destination colour, 6 src·src + dst·dst, 7 src·src + dst, 8 src · a, 9 src · a + dst, 10 multiply (src · dst) |
| depth | 0 test and write, 1 test only, 2 none |
| cull | 0 none (two-sided), 1 and 2 one-sided (opposite windings) |
| test functions | 0 off, 1 <, 2 ≤, 3 >, 4 ≥, 5 =, 6 ≠ (alpha test against the alpha reference / 255) |

- **Dynamic lighting** on: the vertex colour is fixed-function lighting with the style's material: emissive + material ambient × the scene's ambient + Σ over lights of material diffuse × light colour × max(0, N·L), plus specular (material specular, power) when the specular colour is not black. Off: no lighting (the vertex colour is white). A later pass that is not lit itself stays unlit (an engine option to light it like the first pass is off by default).
- **Shaders are never used**: the stock `default` style asks for pixel shader 0, but the game registers no shaders, so every style draws with the fixed-function stages above.
- Passes draw in order over the same piece; each pass's blend combines its result with what is already in the frame.

### Stock styles named by weapons and bodies

T1 = the piece's first texture slot (the colour map), T2 = its second slot, L = the lit vertex colour, F = the texture factor (stock 0x80808080 = 0.502 grey, alpha 0.502, unless said). Every one below is opaque (blend none, depth test and write, one-sided) unless said.

| Style | Material (ambient / diffuse / emissive / specular, power) | Result |
|---|---|---|
| `default` | 0.294 / 0.761 / 0.275 / 0.392, 10 | colour 2 × T1 × L (clamped), alpha T1.a; no alpha test. The style of the sprite-skinned guns |
| `marktransparent` | 1 / 0.8 / 0 / 0, 20 | colour **T1 alone (unlit)**, alpha T1.a; **alpha-tested** (kept where alpha ≥ 160/255), not blended. The team mark's and the ghillie leaves' style: hard-edged cut-outs, no sorting |
| `env_alpha_mask` | 1 / (0, 0, 0.251) / (0.408, 0.4, 0.369) / (0.278, 0.275, 0.255), 20 | stage 0: E = T2 sampled by the camera-space reflection vector (clamped), × L; stage 1: colour T1 + T1.a × E (T1 is **not** lit; its alpha masks the reflection). No alpha test |
| `itemedge` | 1 / (0.863, 0.859, 0.847) / (0.408, 0.4, 0.369) / (0.278, 0.275, 0.255), 20 | T1 × L, then **three times** + R, where R = T2 sampled by the camera-space normal (x, y as u, v; mirrored addressing): a rim glow added 3 ×; alpha T1.a, alpha-tested > 160/255 |
| `edgeamp01` | as `itemedge` | the same with R added once |
| `envmap` | 0.537 / 1 / 0 / 0, 20 | two passes: pass 1 writes 2 × L (no texture); pass 2 (unlit, depth test only) computes lerp(T1, S, T1.a) with S = T2 sampled by the camera-space reflection vector mapped by u = 0.5x + 0.5, v = 0.5y + 0.5 (a sphere map), and **multiplies** it onto the frame: result 2L × lerp(T1, S, T1.a) |
| `pv_char` | 1 / 0.8 / 0.416 / (0.353, 0.349, 0.271), 20 | (T1 × L + T1.a × W) × (1 + T1.a²), with W = T2 sampled by the world-space reflection vector (clamped): the colour map's alpha is a gloss mask. Alpha unused (no blend, no test) |
| `pv_char_b` | 0.333 / 0.875 / 0.416 / 0.443, 50 | three passes: (1) lit, opaque: c = lerp(F, T1, F.a) blended toward L by c's alpha (T1.a²), then a dot3 of the result with itself (a grey intensity); (2) unlit, added (blend add, depth test only): (T1 × F)² + its alpha × F, plus T2 per stage 2; (3) unlit, multiplied onto the frame: 2T1 combined with T2 sampled by the world-space normal. Exact stage table in the file; a faithful copy should evaluate the stages as data |
| `tfactor_blend_host` | 1 / 0.8 / 0 / 0, 10 | 2 × T1 × L, as `default` but with the factor 0x1E3C3C3C (unused by its stage) |
| `tfactor_blend_virus` | as host | as host, factor 0x1E644646 (unused) |

- **Second texture slots** (the question of pieces with T = 2): slot 0 is always the colour map (sampled with the mesh's UV set 1); slot 1 is only ever an environment or rim map sampled with **generated** coordinates (reflection vector or normal), never with the mesh's UVs. Under a style that names only texture 1 (`default`, `marktransparent`, the `tfactor_blend_*` and `translucent*` styles) the second slot is not drawn at all. `128XPV_SPECIALSPECR.DTX` and `CLOUDS.DTX` are therefore reflection pictures for `pv_char`/`env_alpha_mask`/`envmap` pieces.

### Sprite skins (`.spr`)

- **File**: five u32: frame count n (1..4096), frame rate r (frames per second, > 0), colour-keyed flag, translucent flag, colour key; then n frames, each a u16 length and that many bytes of texture path (no terminator). Nothing follows the frames. The two flags and the key matter only for sprite objects; a model skin ignores them (the frame texture's own alpha counts).
- **As a model skin**: a skin slot whose file ends in `.spr` loads the sprite and starts the slot's own frame clock at 0 when the model object is created (or the skin set again). Every client frame in which the model is not paused, its clock advances by the frame's time in whole milliseconds; with p = ⌊1000 / r⌋ ms per frame and length p · n,

  t ← (t + Δ_ms) mod (p · n),  frame = ⌊t / p⌋ mod n,

  and the slot's texture is **replaced** by that frame's texture (no blending between frames). It always loops. Each model object has its own clocks (two guns with the same skin need not be in step). The same holds for character and attachment models (`ATTACH_T\GODSET.SPR`).

## Follow-up (2026-10-31): lighting materials, `pv_char_b`, second slots, world sprites

From the engine's render-state set-up for a render-style pass, its texture-binding of the model's skin slots, its render-style lookup tables, the deferred-renderer switch, the world renderer's section textures and sprite tracker, and the game's skin-list assembly for faces, head pieces, goggles, backpacks and flags; data from the install (Steam build 26236938). Answers the plan's leftovers after "Weapon skins" (`pv_char_b`'s passes, the styles' own materials, slot 1 on faces, head pieces and attached parts) and whether map sprites animate. Where "Render styles and sprite skins (added 2026-10-30)" disagrees, this wins.

1. **File layout correction.** The "has shader options" byte (and its 10 bytes when non-zero) follows **each** pass record, not the pass list: a style is the 68-byte material, the pass count, then per pass 532 bytes + 1 (+ 10), then the 8-byte effect options. `RS\PV_CHAR_B.LTB` is 1,711 bytes = 20 + 8 + 4 + 68 + 4 + 3 × 533 + 8.

2. **Fixed-function path confirmed.** The engine also contains a deferred renderer that draws models with its own shaders, but it needs shader files that the install does not ship; when they are missing it switches itself off. Every model therefore draws with the fixed-function stages and per-pass states described here.

3. **The lighting material, for every style.** For a pass with dynamic lighting on, the engine sets the style's material and turns lighting on:
   - the lit vertex colour L = clamp(M_e + M_a × A + Σ_k M_d × C_k × max(0, N·D_k) × att_k), per channel, where A is the ambient light the engine gives the model, C_k, D_k, att_k each light's colour, direction and falloff (lights contribute no ambient term of their own); its alpha is the material's diffuse alpha (1 in every stock style);
   - **specular**: when the material's specular colour is not black (and its power > 0), a separate specular colour S = Σ_k M_s × C_k × max(0, N·H_k)^p (H the half vector toward the camera) is **added to the pixel after all texture stages**, so it is not multiplied by the texture and not masked by any alpha; when it is black, nothing is added.
   - A pass with lighting off has no lighting: L is white and no specular. (An engine option, off by default, would light a later unlit pass of a style whose first pass is lit, with ambient 0, diffuse 1, no emissive, no specular.)
   - So the styles' lit colour, with "Σ" the sum over lights of C·max(0, N·D)·att:

| Style | L (per channel) | Specular added after the stages |
|---|---|---|
| `default` | 0.275 + 0.294 A + 0.761 Σ | 0.392 Σ_s, power 10 |
| `marktransparent` | A + 0.8 Σ (but its stage ignores L) | none |
| `env_alpha_mask` | (0.408, 0.400, 0.369) + A + (0, 0, 0.251) Σ: lights add only a little blue; L only scales the reflection (T1 is unlit) | (0.278, 0.275, 0.255) Σ_s, power 20 |
| `itemedge`, `edgeamp01` | (0.408, 0.400, 0.369) + A + (0.863, 0.859, 0.847) Σ | (0.278, 0.275, 0.255) Σ_s, power 20 |
| `envmap` (pass 1 only) | 0.537 A + Σ | none |
| `pv_char` | 0.416 + A + 0.8 Σ | (0.353, 0.349, 0.271) Σ_s, power 20 |
| `pv_char_b` (pass 1 only) | 0.416 + 0.333 A + 0.875 Σ | 0.443 Σ_s, power 50 |
| `pv_glass` | A·0.541 + Σ | 0.145 Σ_s, power 100 |
| `tfactor_blend_host`, `_virus` | A + 0.8 Σ | none |

   (Σ_s is the specular sum S without M_s.) `pv_glass`, used by two Bravo face pieces: one lit pass, colour 2 × T1 × L, then lerp(T2, that, T1.a) = T2 · T1.a + (2T1L) · (1 − T1.a) with T2 sampled by the camera-space normal (x, y as u, v, wrap), alpha T1.a, blend 7 of the enum table.

4. **`pv_char_b`'s three passes, exactly** (replaces the table's summary). F = 0x80808080 (0.502 in every channel and alpha). Each stage's result is clamped to 0..1.
   - **Pass 1** (lit, opaque, depth test and write, one-sided): stage 0 c = T1 · F.a + F · (1 − F.a) = 0.502 T1 + 0.250, alpha a = T1.a²; stage 1 m = c · a + L · (1 − a); stage 2 d = clamp(4 · Σ_{r,g,b} (m − 0.5)²) written to all channels (a dot product of m with itself as signed values: a grey). The frame gets d (+ specular).
   - **Pass 2** (unlit, **added** to the frame, depth test only): stage 0 0.502 T1, alpha T1.a; stage 1 its square (0.502 T1)²; stage 2 that + T1.a × F = (0.502 T1)² + 0.502 T1.a. The second slot is bound at stage 2 but nothing reads it.
   - **Pass 3** (unlit, **multiplied** onto the frame, depth test only): stage 0 s₀ = clamp(2 T1), alpha T1.a; stage 1 s₁ = clamp(s₀ + (1 − T1.a) · s₀), alpha a₁ = clamp(4 T1.a − 1); stage 2 s₂ = clamp(s₁ + (1 − a₁) · T2), T2 sampled by the world-space normal (x, y as u, v, clamped).
   - Final: clamp(d + spec + (0.502 T1)² + 0.502 T1.a) × s₂.

5. **Second slots on faces, head pieces and attached parts.** A piece's texture list [a, b] means: the style's texture 1 is skin slot a, its texture 2 is skin slot b. So "slot 1 drawn as a reflection" needs a piece whose second entry is 1 under a style that samples texture 2 (`pv_char`, `pv_char_b`, `env_alpha_mask`, `itemedge`, `edgeamp01`, `envmap`, `pv_glass`, among the stock styles). Where the slots come from:
   - **Face pieces** (`Face#`): the record's whole `Skin#` list fills slots 0, 1, 2 … in order (a colour list `ColorSkin{m}_#` replaces it when the model colour selects one), and its `RenderStyle#` list the style slots. Stock faces that draw slot 1 as a second texture: `pv_char` 13 pieces (the Alpha and Alpha Gear faces' headsets and goggles; slot 1 `textures\128xCharSkin.dtx`), `itemedge` 4 (the new-type infected ship faces, `TEXTURES\128xEdgeSpec02Y.dtx`), `pv_glass` 2 (the Bravo faces, `textures\128xspec_suit.DTX`), `env_alpha_mask` 2 (the specialist faces, `textures\128xSpecialSpecR.dtx`), `edgeamp01` 1 (`SPY_FIINAL_Face`, `TEXTURES\128xEdgeSpec02R.dtx`); the summer and carnival Ophelia eyes are [0, 2] under `pv_char` (slot 2 = `Skin2`); `Mei_Assault02`'s piece is [0, 1] under `pv_char` with no `Skin1` (an empty slot, item 6). Every other face piece naming slot 1 is under a style that does not sample texture 2 (`transparent`, `translucent`, `default`, the `tfactor_blend_*` styles), or uses slot 1 as its colour map (first entry 1, e.g. `CM_Head_PIX`).
   - **Head pieces** (`Hair#`): every stock record has per-skeleton model files, and for such a record the game fills **only slot 0** (one skin: `Skin{c}` by camouflage index c, or by figure type when `SkinForFiguretype` is set, a colour list overriding; out of range → 0) plus slots 7 and 8 (clan emblem and background); slot 1 is **never** filled. No stock head piece samples it anyway: the pieces naming slot 1 are under `transparent`, `translucent` or `default`; the RoboSkull/Hell Knight helmets' pieces are [0, 0] under `pv_char`, so their reflection picture is their own colour map.
   - **Goggles** (`Goggle#`): the `Skin#` list (or the colour list) is copied **starting at the camouflage index** c (c ≥ the list's length → 0): slot k = `Skin{c+k}` (Quirk: with c > 0 the slots shift). **Backpacks** (`Backpack#`) and **flags** (`Flag#`): the whole list, slot k = `Skin{k}`, backpacks also slots 7, 8 for the clan pictures. Stock: no goggle or backpack piece samples a second texture (the masks are [0, 1] under `translucent`, the rank backpacks [0, 0] under `default`); the team flags (`flag01_back`, `flag01_ground`) are [0, 1] under `edgeamp01` with `Skin1` `TEXTURES\128xEdgeSpec01.dtx`.
   - **Weapon parts** (`AttachModel#`/`AttachHHModel#`): no part piece has two slots, and part records name only `default` and `transparent`; slot 1 of a part is only ever a colour map (first entry 1), from `AttachSkin{c}_1` (`attachments.md`).

6. **An empty slot.** A slot with no skin binds no texture; the fixed-function stage then reads it as opaque white (Direct3D 9 behaviour, Open question 10). So an untextured colour map draws as plain L (a lit grey), and an empty reflection slot under `pv_char` adds T1.a × 1 (white where the colour map's alpha is high).

7. **Sprites on world surfaces animate.** A world render section (world and world-model geometry) whose texture name ends in `.spr` loads the sprite and keeps its **own** frame clock, started at 0 when the map loads; each time that section's render block is drawn (normal drawing, not the glow pass) the clock advances by the frame time in whole milliseconds and the section's texture becomes the current frame, by the same rule as model sprite skins (p = ⌊1000 / r⌋ ms a frame, length p · n, looping, no blending). A section that is not drawn (out of view) does not advance. Two sections with the same sprite run separate clocks; both texture slots of a section can be sprites. Stock: `blacklung` (`TEXTURES\Objects\EquipmentOn.spr` 204 triangles, `MineDoor.spr`, `Mine\MineElectricM.spr`, `Wall\MetalFenceWall01_RED.spr`, `Metal\metal_grate_01_RED.spr`, `vehicles\generator01.spr`), `grayhammer` (`sub_ctrlbox\FanSpin01..03.spr`), `shortfuse` (`ETC\Func_Bar.spr`, `ETC\Planka.spr`), `sandhog_cp1` (the BTR-90 engine states, `ETC\Laser_Sensor.spr`), `bloodmoney` (`Sprites\BloodMoney\Door_keypad.spr`).

8. Expect: test cases R6–R10.

## Edge cases

- **Encrypted archives.** Models in V2-keyed archives (ATTACH_M, CHARS_M_BODY, CHARS_M_FACE, CHARS_M_HAND, CHARS_M_HEAD, CHARS_T_BODY, GUNS_M_HH, GUNS_M_PV_AR, GUNS_M_PV_MELEE, GUNS_M_PV_SMG, GUNS_M_PV_SR, GUNS_M_PV_Throwing; from Steam build 26236938 GUNS_M_HH and GUNS_M_PV_SR ship unkeyed) must be decrypted by the rez layer first (specs/combat_arms/rez_archive.md). Decryption leaves the size unchanged; the last `size mod 16` bytes of such a file are already plaintext.
- **No whole-file compression** exists. A file whose bytes 0..3 are not `01 00 09 00` is either another kind of file or still encrypted.
- **Counts block mismatches**: never seen except in the three hint fields; a reader should trust the sections themselves.
- **Strings longer than the engine keeps** (4095, or 255 for binding names) are cut on load; none observed.
- **Zero-length normals:** some vertices have normal (0, 0, 0) (e.g. in the M4A1 first-person model). Do not normalise blindly.
- **Non-unit quaternions:** the engine turns quaternions into matrices with a formula that divides by |q|², so a non-unit quaternion means the rotation of its normalised form. Example: the headquarters flag's socket stores (0, 0, 0, 0.7071), which is the identity.
- **Socket placement** (used for muzzle flash, attachments, name tags): world socket rotation = node rotation × socket rotation; world socket position = node position + node rotation applied to the socket position. The socket's scale replaces the node's (it is not multiplied).
- **Socket count 0** is allowed. **Animation count 0** is allowed (one first-person model has none).
- **Trailing bytes** after the bindings are not checked by the engine; a strict reader may reject them, as none exist.
- **Rigid meshes with a node beyond the node count, triangle indices ≥ V, or bone-set ranges that do not tile the mesh** never occur; the engine does not check them. A reader should reject them.

## Quirks

- **Duplicate texture data per LOD**: every LOD repeats the texture and render-style fields, but only one copy is kept (see LOD record). Keep: harmless.
- **Misspelt shadow commands**: some models' command strings read `ShadoeEnable`, `Shadow_Enable` or `shdowEnable`; those models do **not** cast shadows. Keep (it is the shipped look).
- **Old LOD offsets**: two per piece, written and ignored (some files have (0, 2)). Ignore.
- **Node index field**: written, then overwritten by depth-first numbering. Ignore it, or check it.
- **Version 23 vs 25**: no difference in the reader. Treat both the same.
- **The packer splits vertices on UV seams**, so the stored vertex count is larger than the counts block's vertex field; vertex-animated meshes keep the unsplit count V₀ for their animation data and copy the duplicates.
- **Odd skeleton names**: some characters' trees look misnamed (e.g. `Right_pelvis` is a child of `Left_pelvis`). They are data; keep them.

## Test cases

Measured on the Steam build 25595979 (manifest build id). Files in unencrypted archives were read straight from the install. Files in V2-keyed archives (marked "V2") could not be decrypted in this session; their values were measured on an unencrypted copy of the same file that has the same size and whose unencrypted tail (`size mod 16` bytes) is identical to the Steam copy's, so the coordinator should re-measure them after decryption. Coordinates are as stored (left-handed). Floats to 3 decimals unless exact; compare with a tolerance of 0.001.

1. **Census of the unencrypted archives.** Over all `.LTB` files in PROPS, MODELS, FX, GUNSMITH_NPC, GUNS_M_PV_ETC, GUNS_M_PV_LAUNCHER, GUNS_M_PV_MG, GUNS_M_PV_OPTION, GUNS_M_PV_Pistol, GUNS_M_PV_SG, GUNS_PT, GUNS_SC, PS and RS: 815 files are models that parse exactly to their last byte; 161 have version 23 and 654 version 25; all have bytes 0..19 = `01 00 09 00` followed by 16 zero bytes; all have 0 oriented boxes; there are 2713 pieces, each with 1 LOD: 1733 rigid, 858 skinned (all with bone sets) and 122 vertex-animated, 0 empty; every LOD's stream flags are (0x13, 0, 0, 0); every LOD's body size equals the bytes its body actually takes. The other 72 `.LTB` files are refused: 65 render-style files in RS (byte 0 = 5), the 6 text-header files in FX listed above, and `FX\WATERFALL2.LTB` (version 18).
2. **World prop, rigid: `PROPS\HEADQUATERS\HQ_FLAGPOLE01.LTB`** (headquarters flagpole). 4,873 bytes; version 25; visibility radius 270.872; 1 piece `HQ_flagpole01`, 1 LOD, rigid, 116 vertices, 86 triangles, follows node 1, body size 4,264, texture slots 1 (index 0), render style 0; 2 nodes: `Scene Root` (no parent), `flagpole01` (parent `Scene Root`, flags 2); bind transform of `flagpole01` has rows (1, 0, 0, 0), (0, 0, 1, 0), (0, −1, 0, 0), (0, 0, 0, 1); sockets `HQ_pole_top` at (0, 0, 252) and `HQ_pole_bottom` at (0, 0, 4), both on `flagpole01` with rotation (0.707, 0, 0, 0.707); 1 animation `HQ_Flagpole01`, 1 keyframe, dimensions (10, 270, 10); binding translation (0, 0, 0); vertex bounds (−2.146, 0, −1.858) to (2.146, 268.190, 1.858).
3. **Socket in model space.** In test 2, `HQ_pole_top` at bind pose is at model-space (0, 252, 0) (node matrix applied to (0, 0, 252)), which is (0, 252, 0) after conversion too; `HQ_pole_bottom` is at (0, 4, 0).
4. **World prop, skinned: `PROPS\HEADQUATERS\HQ_FLAG01.LTB`** (headquarters flag). 19,572 bytes; version 25; radius 96; 1 piece `flag`, 1 LOD, skinned with bone sets, Bt = 3, Bv = 3, stride 40, 56 vertices, 60 triangles, 2 bone sets: (first 0, count 32, nodes 2, 3, 4, index end 108) and (32, 24, nodes 1, 2, 3, 180); 5 nodes: `Scene Root` (none), `D_flag` (parent `Scene Root`), `Bone01` (`D_flag`), `Bone02` (`Bone01`), `Bone03` (`Bone02`); 1 socket `HQ_flag_C` on `D_flag`, rotation (0, 0, 0, 0.707); 1 animation `HQ_flag01`, 111 keyframes, compression 0, dimensions (66.2, 32, 41.4); counts block: keyframes 111, transform-track bytes 15,540 (= 111 × 5 × 28); vertex bounds (1.2, 252, 0) to (71.2, 300, 0).
5. **Vertex-animated prop: `PROPS\SAND_HOG\SH_LAPTOP.LTB`** (laptop). 213,233 bytes; version 25; radius 27.252; 6 pieces: `d_Normal_Screen`, `d_HackScreen`, `d_CopyScreen`, `d_Gauge`, `d_ConfirmScreen` (each vertex-animated, 4 vertices, V₀ = 4, 2 triangles, 0 duplicates, body size 188, animated by nodes 1 to 5 in that order) and `Laptop_Body` (rigid, 125 vertices, 60 triangles, node 6); 7 nodes, root `Scene Root` with flags 2 and six children (`d_Normal_Screen` … `Laptop_Bone`); 1 animation `Base_Anim`, 642 keyframes, dimensions (14.5, 21.8, 13.9); counts block vertex-animation bytes 154,080 (= 5 × 4 × 642 × 12) and track bytes 35,952.
6. **First-person weapon, unencrypted: `GUNS_M_PV_PISTOL\PV_PST_M92FS_SH.LTB`** (the M92FS first-person model). 60,570 bytes; version 25; 5 pieces, all rigid, 1 LOD: `Object04` 7 v / 5 t (node 24), `Object05` 42 / 34 (node 26), `Object02` 1001 / 708 (node 24), `Object06` 322 / 254 (node 28), `Object07` 108 / 79 (node 31); 51 nodes, including `Bip01` (parent `Scene Root`), `Bip01 R Hand` (parent `Bip01 R Forearm`), `Box06` (parent `Box05`); sockets `flash` at (0, 0.078125, 0.640625) and `breach` at (0.171875, 0.0625, −0.140625), both on `Box06` (node 27); 1 own animation (`CM_HND_SP_DF_Beretta_M92F_FS_CH`, 1 keyframe, dimensions (2, 2, 2)); 2 child models, child 1 `Ani_PST_M92FS_SH.ltb`; vertex bounds (4.131, 3.206, 0.049) to (5.397, 3.968, 0.372).
7. **Its animation child: `GUNS_M_PV_PISTOL\ANI_PST_M92FS_SH.LTB`.** 191,067 bytes; 3 skinned pieces (`arm_SH` 1650 v / 1332 t, `Object01` 510 / 608, `PV_COLT` 1095 / 754); 51 nodes with the same tree shape as test 6; 13 animations, all compression 3: `select`, `idle_0`, `idle_1`, `prefire`, `fire_0`, `postfire`, `reload`, `dryfire`, `PreReload`, `PostReload`, `PreRush`, `rush`, `PostRush`; all dimensions (2, 2, 2) and all binding translations (−0.4, −3.6, −1.2); no sockets. Combined list of test 6 + this file: 14 animations, the M92FS model's own first.
8. **First-person weapon, V2: `GUNS_M_PV_AR\PV_AR_M4A1_SH.LTB`** (the M4A1's first-person model). 112,806 bytes; version 25; 1 piece `Object011`, skinned with bone sets, Bt = 2, Bv = 1, stride 36, 2690 vertices, 1617 triangles, 8 bone sets, texture index 1; 50 nodes, including `Box01` (parent `Scene Root`), `Box12` (parent `Box01`), `Bip01 L Clavicle` (parent `Bip01 Neck`); sockets `Flash` at (2.984, 0.140625, 0) and `Breach` at (0.328125, 0.0625, −0.109375), both on `Box12` (node 2); 1 own animation `CW_HND_SP_DF_M4A1_CH` (compression 3, 1 keyframe, dimensions (2, 2, 2)); child 1 `Ani_AR_M4A1_SH.ltb`; vertex bounds (0.542, 2.474, 0.260) to (0.933, 3.975, 4.638); some normals are (0, 0, 0).
9. **Its animation child, V2: `GUNS_M_PV_AR\ANI_AR_M4A1_SH.LTB`.** 305,925 bytes; 3 skinned pieces: `PV_M4a1` 1372 v / 807 t (Bt 2), `arm_SH` 1650 / 1332 (Bt 3), `Object01` 510 / 608 (Bt 3); 50 nodes; 13 animations (same names as test 7), 236 keyframes in total, compression 3; `fire_0` has a keyframe at time 0 with the string `FIRE_KEY`; all binding translations (−0.4, −3.6, −1.2); vertex bounds (−4.955, 2.866, −0.364) to (7.472, 4.369, 0.359).
10. **Character body, V2: `CHARS_M_BODY\CM_BODY_ALPHATEAM_KI.LTB`** (an Alpha Team male body). 276,818 bytes; version 25; radius 96; 8 pieces, 1 LOD each: `Clan_Emblem_Up` rigid 10 v / 9 t (node 12), `Friendly_Sign_LV3` rigid 24 / 18 (node 25), `Clan_Emblem_Down` rigid 10 / 9 (node 12), `alpha_vst` skinned 2999 / 2802 (Bt 4, stride 44, 44 bone sets), `alpha_leg` skinned 1268 / 1279 (Bt 3, 16 sets), `alpha_gear` rigid 87 / 42 (node 13, 2 texture slots), `alpha_protector` skinned 36 / 58 (Bt 2, 1 set), `alpha_magazine` rigid 1575 / 597 (node 5); 6009 vertices and 4814 triangles in total (counts block: 4814); 53 nodes, including `Scene Root` (none), `Head` (parent `Neck`), `Right_hand` (parent `Right_arml`); 2 weight sets `upper` and `Lower` of 53 weights; 18 sockets: `Eyes`, `LeftHand`, `RightHand`, `1W_N`, `P_BAG_N`, `NAME_socket`, `Left_armu1`, `Left_arml1`, `Right_armu1`, `Right_arml1`, `Left_legu1`, `Left_legl1`, `Right_Legu1`, `Right_legl1`, `Face_Eyelid1`, `Nuts`, `eye_l`, `eye_r` (`RightHand` on node 27 at (7.085, 1.661, 1.277)); 1 own animation (dimensions (2, 2, 2)); child 1 `st_m_child.ltb`, with an empty binding list; vertex bounds (−36.613, −54.615, −11.978) to (36.612, 32.998, 13.859).
11. **Stand and crouch boxes, V2: `CHARS_M_BODY\ST_M_CHILD.LTB`** (the male character animation library). 53 nodes, same tree shape as test 10; `LST` dimensions (15, 50, 15), binding translation (0, −50, 0); `LC` dimensions (15, 40, 15), translation (0, −40, 0); all animations compression 0. Caveat: the Steam copy is 66,900,324 bytes, larger than the measured copy (64,326,833 bytes, 1504 animations), so the coordinator must check the animation count and these values on the decrypted Steam file. For test 10's combined list, looking up `LST` must return (15, 50, 15).
12. **Unencrypted character: `GUNSMITH_NPC\NPC_MODEL.LTB`** (the gunsmith NPC). 543,330 bytes; version 25; 9 pieces of one LOD: `cap` skinned 812 / 822, `base` skinned 3501 / 3320, `eye_1` rigid 140 / 168, `Cylinder02` skinned 1370 / 1710, `eye_el` rigid 109 / 133, `Plane05` rigid 9 / 5, `table` rigid 1689 / 1202, `Plane02` rigid 4 / 2, `Plane06` skinned 4 / 2; 69 nodes, including `Point01` (parent `Scene Root`), `bone_03` (parent `Point01`), `Bip01 Pelvis` (parent `Bip01`); no sockets; 1 own animation `npc_model` (92 keyframes, dimensions (15, 50, 15)); child 1 `npc_ani.ltb` (`GUNSMITH_NPC\NPC_ANI.LTB`, 2,954,119 bytes, 69 nodes, 11 animations starting `start`, `idle_1`, `idle_2`).
13. **Three LODs, V2: `CHARS_M_BODY\CM_HV_UR_BRANDON_JS.LTB`.** 392,166 bytes; pieces `Urban_Leg` and `Urban_Torso` have 3 LODs at distances (0, 400, 800) and old LOD offsets (0, 2); `Urban_Leg` LODs have 1564 / 1425, 936 / 711 and 631 / 427 vertices / triangles, `Urban_Torso` 2370 / 2010, 1355 / 1004, 885 / 602; the other three pieces have 1 LOD; child 1 `ST_M_child.ltb`.
14. **LOD choice** with distances (0, 400, 800) and bias 0: d = 0 → 0; d = 399.9 → 0; d = 400 → 1; d = 799.9 → 1; d = 800 → 2; d = 5000 → 2. With bias +1: d = 0 → 1, d = 800 → 2. With bias −1: d = 400 → 0. A piece with one LOD always gives 0.
15. **Winding.** For every triangle of `PROPS\HEADQUATERS\HQ_FLAGPOLE01.LTB` (86), `GUNS_M_PV_PISTOL\PV_PST_M92FS_SH.LTB` (1080) and `MODELS\SPHERE.LTB` (840), (p_b − p_a) × (p_c − p_a) has a positive dot product with the sum of the three vertex normals. After conversion with index swap, the same holds for the converted data.
16. **Bone sets.** For every bone-set mesh in tests 4 and 8 to 10: sets tile the vertices and indices exactly, every index of a set lies in its vertex range, used bones are increasing, and every vertex's stored weights sum to at most 1.
17. **Parent and child skeletons.** For tests 6/7, 8/9 and 10/11 (and 12 with its child), parent and child have equal node counts and equal depth-first parent lists.
18. **Body sizes.** In tests 2, 4 and 5, rigid bodies are 36 + 32V + 6N, bone-set bodies 34 + V·stride + 6N + 4 + 12G, and vertex-animated bodies 44 + 32V + 6N + 4 + 4D (4,264, 2,662 and 188).
19. **Synthetic fixture** (next section): a reader produces exactly the values listed there and consumes all 540 bytes.
20. **Refusals** (each a modified copy of the fixture): byte 0 = 5 → refused as not a model; version 22 → refused; piece count 65 → refused; LOD mesh kind 8 → refused; child count 0 → refused; a weight set with fewer weights than nodes → refused; a socket on node 1 when there is one node → refused; container version 8 with a rigid LOD → refused; the file cut short anywhere → refused.
21. **Conversion.** A vertex (1, 2, 3) with normal (0, 0, 1) becomes (1, 2, −3) with normal (0, 0, −1); triangle (0, 1, 2) becomes (0, 2, 1); quaternion (0.1, 0.2, 0.3, 0.927) becomes (−0.1, −0.2, 0.3, 0.927); a bind matrix with translation column (1, 2, 3) becomes one with (1, 2, −3); test 2's `flagpole01` rows become (1, 0, 0, 0), (0, 0, −1, 0), (0, 1, 0, 0).

Render styles and sprite skins (added 2026-10-30), read from the install:

- **R1.** `RS\MARKTRANSPARENT.LTB`: 645 bytes, 1 style of 605 bytes, 1 pass; stage 0 texture 1, colour op 1 (arg texture), alpha op 1 (texture); blend 0, depth 0, cull 1, alpha test 4 with reference 160, dynamic lighting on; material diffuse 0.8. A texel of alpha 159 is discarded, 160 drawn at full texture colour.
- **R2.** `RS\DEFAULT.LTB`: 655 bytes (shader options present: pixel shader on, id 0); material ambient 0.294, diffuse 0.761, emissive 0.275, specular 0.392, power 10; stage 0 colour op 4 (2×), filter 1. A texel 0.4 under a lit colour 0.6 → 0.48.
- **R3.** `RS\ITEMEDGE.LTB`: stages 1–3 texture 2, colour op 7 (add), coordinates from the camera-space normal, mirrored; alpha test 3 (>) reference 160.
- **R4.** `RS\ENVMAP.LTB`: 1178 bytes, 2 passes; pass 2 blend 10 (multiply), depth 1, stage 1 texture 2 with coordinates from the camera-space reflection and the transform scale 0.5, offset 0.5.
- **R5.** A sprite with n = 5, r = 10 (100 ms a frame) as a model skin: at 0–99 ms frame 0, at 250 ms frame 2, at 520 ms frame 0 (looped), with 16 ms frames advancing by 16 ms each; the frame change swaps the whole texture.

Weapon skins on Steam build 26236938 (added 2026-10-30, Open question 8), read from the install:

- **W1.** "Neon Outlaw ASW338LM" is record `ASW338LMspunk` (`Weapon2338`): `PVModelNormal` `GUNS_M_PV_SR\PV_SR_ASW338LM_KI.ltb`, `PVSkin1` `GUNS_T_PV_SR\PV_SR_ASW338LMspunk.dtx` (1024 × 1024, BGRA, one level; the archive has it), `HHModel` `GUNS_M_HH\HH_SR_ASW338LM_KI.ltb`, `HHSkin0` `GUNS_T_HH\HH_SR_ASW338Lspunk.dtx`, which is **not in the install** (no held texture of that skin exists under any spelling), so its held model is untextured. The view model's three drawn pieces all name texture slot 1 in `RS\default.ltb` style 0; the model has one UV set.
- **W2.** Every ASW338LM record shares that view model. Drawing the model's UV triangles over each skin: `PV_SR_Asw338lm.dtx` (DXT1), the camo 01 and 02, Four-Leaf, Horus and Obsidian Circuit (`..._ASW338LMbloodbat`) skins fit the UV islands; the six newest, `ASW338LMspunk`, `Royal Gold ASW338LM` (`..._ASW338LMgold`), `ASW338LMgraf`, `ASW338LMsakura`, `ASW338LMsea` and `ASW338LMweastland` (records 2320 and 2335–2339, all 1024 × 1024 BGRA), are painted for a different layout that neither the view nor the held ASW338LM model of the install has. Drawn from the files as named, those skins land scattered over the gun; that is the data, not a reading error (same header, same model, same UVs as the skins that fit).
- **W3.** "AUG A3 Champions Lightning" is record `AUGA3ChampionsLighting` (sic): view model `GUNS_M_PV_AR\PV_AUG_A3_CHAMPIONS_LEAGUE.ltb`, `PVSkin1` `GUNS_T_PV_AR\PV_AUG_A3_CHAMPIONS_LIGHTING.dtx`, **not in the install** (only its held texture `GUNS_T_HH\HH_AUG_A3_CHAMPIONS_LIGHTING.dtx` is): its first-person gun is untextured. Its siblings (`_BLACK`, `_HOLY`, `_INK`, `_LEAGUE`) have both.

Lighting materials, `pv_char_b`, second slots, world sprites (Follow-up (2026-10-31)), from the install:

- **R6.** `RS\PV_CHAR_B.LTB`: 1,711 bytes, 3 passes, each followed by a zero shader-options byte; pass 1 lit, blend 0, depth 0; pass 2 unlit, blend 1, depth 1, stage 2 texture 2 with colour op 14 on (current, factor); pass 3 unlit, blend 10, depth 1, stage 2 texture 2, colour op 15, coordinates from the world-space normal, clamped.
- **R7.** `pv_char_b` (logic), T1 = (0.6, 0.4, 0.2), T1.a = 0.8, lit colour L = 0.5, T2 = 0.5, no specular: pass 1 d = 0.045; pass 2 adds (0.492, 0.442, 0.412); pass 3 multiplies by (1, 0.96, 0.48) → (0.537, 0.467, 0.219). With T1.a = 0.3 instead: d = 0.001, pass 2 (0.241, 0.191, 0.161), pass 3 factor (1, 1, 1) → (0.242, 0.192, 0.162).
- **R8.** Lighting (logic): `default` with A = 0.2 and one white light at N·D = 0.5 → L = 0.275 + 0.059 + 0.381 = 0.714; `env_alpha_mask` with the same → L = (0.608, 0.600, 0.694); `pv_char_b` pass 2 and 3 get no lighting whatever the lights.
- **R9.** Skin slots (data): face `Face160` (Alpha Gear) piece `alpha_goggle` [0, 1] under `pv_char` → its reflection is `textures\128xCharSkin.dtx`; head piece `Hair331` (Summer Claire, camouflage 1) → slot 0 = its `Skin1`, slot 1 empty; a goggle with `Skin0..2` = a, b, c at camouflage 1 → slots 0, 1 = b, c; camouflage 5 → a, b, c.
- **R10.** World sprite (logic): `grayhammer`'s `TEXTURES\Objects\sub_ctrlbox\FanSpin01.spr` is 8 frames at 19 per second (p = 52 ms, length 416 ms, frames `64xFan03`, `64xFan03b`, `64xFan04`, `64xFan04b`, `64xFan01`, `64xFan01b`, `64xFan02`, `64xFan02b`); drawn every frame from load: at 100 ms frame 1, at 450 ms frame 0 (looped). Turned away for 1 s and back: it resumes where it stopped.

## Synthetic fixture

A minimal valid model a reader's unit tests can build in code: one rigid triangle on a single root node, one animation `LST` with one keyframe, one socket and one binding. Total 540 bytes. All values little-endian.

| Offset | Size | Field | Value |
|---|---|---|---|
| 0 | 1 | file type | 1 |
| 1 | 1 | padding | 0 |
| 2 | 2 | container version (u16) | 9 |
| 4 | 16 | reserved and padding | all 0 |
| 20 | 4 | model version | 25 |
| 24 | 60 | counts block, 15 × u32 | 1, 1, 1, 1, 1, 1, 3, 3, 1, 1, 0, 4, 22, 0, 28 |
| 84 | 2 | command string length | 0 |
| 86 | 4 | visibility radius (f32) | 10.0 |
| 90 | 4 | oriented boxes | 0 |
| 94 | 4 | piece count | 1 |
| 98 | 6 | piece name | length 4, `Body` |
| 104 | 4 | LOD count | 1 |
| 108 | 4 | LOD distance 0 (f32) | 0.0 |
| 112 | 8 | old LOD offsets | 0, 0 |
| 120 | 4 | texture slots used | 1 |
| 124 | 16 | texture indices (4 × i32) | 0, 0, 0, 0 |
| 140 | 4 | render-style index | 0 |
| 144 | 1 | render priority | 0 |
| 145 | 4 | mesh kind | 4 (rigid) |
| 149 | 4 | body size | 138 |
| 153 | 4 | vertex count | 3 |
| 157 | 4 | triangle count | 1 |
| 161 | 4 | max bones per triangle | 1 |
| 165 | 4 | max bones per vertex | 1 |
| 169 | 16 | stream flags | 0x13, 0, 0, 0 |
| 185 | 4 | node | 0 |
| 189 | 32 | vertex 0 | position (0, 0, 0), normal (0, 0, 1), UV (0, 0) |
| 221 | 32 | vertex 1 | position (10, 0, 0), normal (0, 0, 1), UV (1, 0) |
| 253 | 32 | vertex 2 | position (0, 10, 0), normal (0, 0, 1), UV (0, 1) |
| 285 | 6 | triangle (3 × u16) | 0, 1, 2 |
| 291 | 1 | used-node count | 1 |
| 292 | 1 | used node | 0 |
| 293 | 12 | root node name | length 10, `Scene Root` |
| 305 | 2 | node index (u16) | 0 |
| 307 | 1 | node flags | 0 |
| 308 | 64 | bind transform | identity (1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1) |
| 372 | 4 | child node count | 0 |
| 376 | 4 | weight sets | 0 |
| 380 | 4 | child models (incl. self) | 1 |
| 384 | 4 | animation count | 1 |
| 388 | 12 | dimensions | (15, 50, 15) |
| 400 | 5 | animation name | length 3, `LST` |
| 405 | 4 | compression mode | 0 |
| 409 | 4 | blend time | 200 |
| 413 | 4 | keyframe count | 1 |
| 417 | 4 | keyframe time | 0 |
| 421 | 2 | keyframe string | length 0 |
| 423 | 1 | track flag (node 0) | 0 |
| 424 | 12 | position key | (0, 0, 0) |
| 436 | 16 | rotation key | (0, 0, 0, 1) |
| 452 | 4 | socket count | 1 |
| 456 | 4 | socket node | 0 |
| 460 | 7 | socket name | length 5, `Flash` |
| 467 | 16 | socket rotation | (0, 0, 0, 1) |
| 483 | 12 | socket position | (0, 0, 10) |
| 495 | 12 | socket scale | (1, 1, 1) |
| 507 | 4 | bindings for child 0 | 1 |
| 511 | 5 | binding name | length 3, `LST` |
| 516 | 12 | binding dimensions | (15, 50, 15) |
| 528 | 12 | binding translation | (0, −50, 0) |
| 540 | | end of file | |

Expected from a reader: 1 piece `Body` with 1 rigid LOD of 3 vertices and 1 triangle following node 0, body size 138 (equal to the bytes consumed); 1 node `Scene Root` with no parent; 0 weight sets; no child names; 1 animation `LST` whose effective dimensions are (15, 50, 15) and translation (0, −50, 0); socket `Flash` on node 0 at (0, 0, 10), which is (0, 0, 10) in model space and (0, 0, −10) after conversion; vertex bounds (0, 0, 0) to (10, 10, 0); the triangle's (p₁ − p₀) × (p₂ − p₀) = (0, 0, 100), along the normals; after conversion the triangle is (0, 2, 1) and the normals (0, 0, −1). The anim-track byte count in the counts block (28) equals 1 keyframe × 28.

## Open questions

1. **V2 decryption of the Steam copies.** No V2 key was available to this session, so tests 8 to 11 and 13 were measured on unencrypted copies of the same files (equal size, identical plaintext tails). Re-measure them on the decrypted Steam files. `ST_M_CHILD.LTB` differs in size on Steam (66,900,324 bytes), so its animation count, and possibly the `LST` / `LC` boxes, may differ.
2. **Version 23 vs 25.** The reader treats them alike and every version-23 file parses with the same layout; what the packer changed between them (perhaps only data it computes) is not known.
3. **Vertex colour.** The packer can write a colour per vertex but the game does not count it, so such files would not load. None exist; whether any tool-side variant ever produced them is unknown.
4. **Palette meshes.** Only three skinned LODs in the encrypted archives use a matrix palette (with re-indexed bones); none were seen in unencrypted ones. Their weight order is taken from the packer; worth an on-screen check once characters render.
5. **Render priority.** Its exact effect on draw order (how it interacts with render-style sorting) belongs to the renderer and is not specified here.
6. **Metric scale.** The file gives no physical unit. If the project wants metres, a scale has to be chosen (for example from the 100 u standing height); other specs keep u.
7. **The text-header `.ltb` files** in FX look like a later LithTech model container. The game cannot load them; if they ever matter, they need their own spec.
8. **Answered (2026-10-30, Steam build 26236938)**, test cases W1–W3: record `ASW338LMspunk`; its view skin is laid out for UVs the install's ASW338LM model doesn't have (so are five other new ASW338LM skins), its held skin is missing; the AUG A3 Champions Lightning's view skin is missing. Was: **Weapon "Neon Outlaw ASW338LM"** (plan "Weapon skins" 4): not in this box's install; which record and pieces it has needs the Windows PC's.
9. **Rim and reflection pictures' look.** The camera-space-normal coordinates of `itemedge` use x, y in −1..1 with mirrored addressing; confirm on screen that the rim picture lines up as described.
10. **An empty texture slot** (Follow-up (2026-10-31) 6): read as opaque white by the fixed-function stages (the graphics API's behaviour, not traced in the game); check `Mei_Assault02`'s face in game (a white gloss where its colour map's alpha is high).
