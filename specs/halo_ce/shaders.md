# Halo: Combat Evolved: shaders

Source basis: the behaviour reference (a decompilation of an early Xbox
development build of the game): its shader tag definitions, the Xbox
rasterizer's environment, model, transparent, water, plasma and fog
passes, the transparent-shader preprocessors that turn tag fields into
combiner programs, the periodic-function tables and the random number
generator they are built with, and the frame's render order. Every tag
layout, enumeration range and measured number was checked against the
retail Xbox disc's 13 multiplayer maps with throwaway scripts. The
per-pixel math is read from the reference's combiner programs (fixed
hardware arithmetic, decoded by hand) and cannot be checked on the disc;
the vertex programs were not readable and what they compute is marked
inferred.
Status: draft

## Summary

- A shader tag is a 40-byte common header followed by one of eight
  class bodies on the disc: environment (`senv`, level surfaces), model
  (`soso`, objects), transparent chicago (`schi`), transparent generic
  (`sotr`), glass (`sgla`), meter (`smet`), plasma (`spla`) and water
  (`swat`). The chicago extended class (`scex`) does not occur on the
  Xbox disc.
- Level surfaces are drawn in passes over a lit depth buffer. The look
  comes down to: frame = clamp(lightmap × material colour × bump shade
  + self-illumination) × clamp(base ⊗ detail ⊗ micro detail), then
  specular and cube-map reflection added, weighted by a specular mask
  kept in the frame's alpha, then atmospheric fog blended over.
  "⊗" is the detail function; on the disc it is always the
  "double biased multiply" 2·a·b, with every step clamped to [0, 1]
  for positive values.
- There is **no overbright** on the Xbox: the lightmap is used as stored
  (0..1), and all math is done on the raw 8-bit stored values with no
  gamma conversion.
- Transparent shaders on the multiplayer maps use only two framebuffer
  blend functions: alpha blend and add.
- All animation is driven by twelve periodic functions read from
  1024-entry byte tables; one tag "period" covers 0.7 of a cosine cycle
  (a quirk of the table construction, kept).

## Units and conventions

- Tag storage, blocks, tag references and data references follow
  `cache_file.md`. Offsets below are from the start of the tag's main
  structure (or block element). All numbers are little-endian. "f32" is
  an IEEE float, "i16"/"u16" 16-bit integers, "rgb" three f32 (red,
  green, blue), "argb" four f32 in the order **alpha, red, green,
  blue**, "ref" a 16-byte tag reference, "block" a 12-byte block field.
- Colours are 0..1 floats. A texel byte b means b/255. Texel channels
  are named r, g, b, a after decoding (`bitmaps.md`).
- **Colour space.** The Xbox has no sRGB decode or encode. Every input
  (texels, lightmap texels, tag colours) is used as its stored value and
  every output is written as is. A faithful renderer uses all textures
  as plain (non-sRGB) values and writes the result without an sRGB
  encode, or equivalently converts to linear for storage only when the
  round trip is exact. The picture's brightness is then the TV's
  response to these values.
- **Arithmetic model (the console's pixel combiners).** Each combining
  step computes a·b + c·d (or a dot product, or a select) from inputs
  that may be remapped: clamp(x) to [0, 1], 1 − clamp(x),
  2·clamp(x) − 1, 1 − 2·clamp(x), clamp(x) − 0.5, 0.5 − clamp(x), x,
  −x. Every intermediate result is clamped to [−1, 1] and may be scaled
  (×½, ×2, ×4, or 2x−1) before clamping. The final output clamps each
  channel to [0, 1]. Precision is about 1/255; tests use a tolerance of
  2/255. Where this spec writes clamp(…) it means this per-step clamp.
- **Time.** Shader animation reads game time in seconds (game ticks /
  30, `cache_file.md`), the same value for every shader in a frame.
- **Texture coordinates.** u, v as stored in the vertex (`structure_bsp.md`,
  `models.md`); scales multiply them; maps wrap unless stated.
- **Angles.** Transparent-map rotation in degrees; water ripple and
  plasma directions in radians or plain vectors as stated.
- **Culling.** Front faces are the ones the game does not cull with its
  Direct3D "cull counter-clockwise" state: clockwise on screen. Level and
  model geometry winding is `structure_bsp.md`'s and `models.md`'s.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| shader_header_size | 0x28 (40) | bytes | common header before every class body |
| periodic_table_size | 1024 | entries | bytes per periodic-function table |
| periodic_index_rate | 25.6 | entries per period | table index = 25.6 × input |
| periodic_table_cycles | 28 | cycles | cosine cycles stored in one table |
| periodic_seed | 0x20F3F660 | — | random seed the tables are built from |
| rng_multiplier | 1664525 | — | linear congruential step: s = s·1664525 + 1013904223 mod 2^32 |
| rng_increment | 1013904223 | — | as above |
| alpha_test_reference | 127 | /255 | alpha test passes when alpha > 127/255 |
| specular_exponent | 8 | — | power of the reflected-light dot product (32 with "extra shiny") |
| decal_z_bias | 8 | console z-bias units | depth bias of decal transparents and alpha-blended model decals |
| plasma_band_halfwidth | 0.5 | alpha units | plasma mask is zero where the map alpha differs from the animated value by 0.5 or more (before sharpening) |
| water_ripple_layers | 4 | layers | ripple layers composited per water shader |
| max_transparent_maps | 4 | maps | texture stages for chicago and generic maps |
| max_generic_stages | 7 | stages | combiner stages for a generic shader (the 8th is the fog/fade stage) |
| max_layer_depth | 4 | levels | transparent extra-layer nesting the game draws |

## Shader classes

| Class | Name | Type number (header 0x24) | Body size (total) | On the disc |
|---|---|---|---|---|
| `shdr` | shader (parent only) | — | 0x28 | never a tag of its own |
| — | screen | 0 | — | not a tag class |
| — | effect | 1 | — | embedded in particle, contrail and weather tags (`effects.md`) |
| — | decal | 2 | — | embedded in decal tags (`effects.md`) |
| `senv` | shader_environment | 3 | 0x344 | yes |
| `soso` | shader_model | 4 | 0x1B8 | yes |
| `sotr` | shader_transparent_generic | 5 | 0x6C | yes |
| `schi` | shader_transparent_chicago | 6 | 0x6C | yes |
| `scex` | shader_transparent_chicago_extended | 6 | 0x78 | no (layout from the reference) |
| `swat` | shader_transparent_water | 7 | 0x140 | yes, 2 maps |
| `sgla` | shader_transparent_glass | 8 | 0x1E0 | yes |
| `smet` | shader_transparent_meter | 9 | 0x104 | yes |
| `spla` | shader_transparent_plasma | 10 | 0x14C | yes |

- The game draws a shader by its header type number, not by its class.
  On the disc the type always matches the class (checked: every one of
  2077 shader tags).
- Transparent types are 1 and 5..10. Types valid on level geometry: 3
  and 5..9. Types valid on models: 3..10.

## Common header (all classes, 0x28 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 2 | u16 | Radiosity flags: bit 0 simple parameterization, bit 1 ignore normals, bit 2 transparent lit |
| 0x02 | 2 | i16 | Detail level: 0 high, 1 medium, 2 low, 3 turd (lightmap baking) |
| 0x04 | 4 | f32 | Power: light the surface emits for the lightmap baker (0 = none) |
| 0x08 | 12 | rgb | Colour of emitted light (baker) |
| 0x14 | 12 | rgb | Tint colour (baker: colour of light passing through transparent surfaces) |
| 0x20 | 2 | u16 | Physics flags (none defined; 0 on the disc) |
| 0x22 | 2 | i16 | Material type: the 33 codes 0..32 of `collision.md` (dirt, sand, stone, ...); what the surface sounds and looks like when hit |
| 0x24 | 2 | i16 | Shader type number (table above), filled at build time |
| 0x26 | 2 | | Padding |

- Only "transparent lit" (bit 2) matters to drawing: a generic or
  chicago shader with it set receives the surface's lighting as vertex
  colour (below). The baker fields (power, colours, detail level) do not
  affect the game's drawing; the lightmaps are already baked.
- Disc: radiosity flags 0, 1, 2, or 4; detail levels 0..3; material
  types within 0..32.

## Enumerations

### Periodic (animation) functions

| Code | Name | Shape over one table cycle (x in cycles) |
|---|---|---|
| 0 | one | constant 1 (no table lookup) |
| 1 | zero | constant 0 |
| 2 | cosine | (cos 2πx + 1)/2, starts at 1 |
| 3 | cosine (variable period) | cosine of a randomly stretched x |
| 4 | diagonal wave | triangle: 2·frac(x) for frac < ½, else 2 − 2·frac(x) |
| 5 | diagonal wave (variable period) | triangle of a randomly stretched x |
| 6 | slide | sawtooth frac(x), rising 0→1 |
| 7 | slide (variable period) | sawtooth of a randomly stretched x |
| 8 | noise | independent random values per table entry |
| 9 | jitter | fixed sum of cosines (below) |
| 10 | wander | same table as jitter |
| 11 | spark | frac(stretched x)² |

How the tables are built (once at start-up):

- Seed the generator with 0x20F3F660. One random value r is: s ←
  s·1664525 + 1013904223 (mod 2^32), r = (s >> 16)/65535, as a 32-bit
  float.
- For each function in code order 0..11: first build a stretched-x
  table X of 1024 entries: X[0] = 0 and X[i+1] = X[i] + ¼·r₁ + ¼ +
  (cos(8.2π i/1024) + 1)·r₂ + (cos(10.2π i/1024) + 1)·r₃ +
  (cos(14.6π i/1024) + 1)·r₄ (four new random values per entry, in that
  order, summed in 32-bit floats); then divide every X[i] by the final
  sum. (Built for every function, used only by the variable ones and
  spark; it consumes 4096 random values each time.)
- Then for entry i: x = i × 0.027343748 (28/1024), xr = 28·X[i], and
  the raw value by function: cos 2πx; cos 2πxr; triangle(x);
  triangle(xr); frac(x); frac(xr); a new random value (noise); for
  jitter and wander (cos(0.89759791x)·cos(25.132742x) +
  cos(43.9823x)·sin(1.5707964x))/2 + sin(πx)·cos(2πx); for spark
  frac(xr)².
- Normalise: for every function except the two slides, map the raw
  values to [0, 1] by (v − min)/(max − min) over the 1024 entries. The
  slides are not normalised. Store byte = clamp(trunc(255·v), 0, 255).

Evaluation at input s (in "periods"):

- Function 0 returns 1 without a table.
- p = 25.6·s; i = floor(p); f = p − i (for negative p, f is still taken
  in [0, 1) and i one lower); a = table[i mod 1024]/255, b =
  table[(i+1) mod 1024]/255.
- Slides: if a > 0.75 and b < 0.25 then b += 1; v = (1 − f)·a + f·b;
  if v > 1 then v −= 1.
- All others: v = clamp((1 − f)·a + f·b, 0, 1).
- **Quirk.** One unit of s advances 25.6 entries, and a table cycle is
  1024/28 = 36.57 entries, so the periodic functions repeat every
  s = 1/0.7 ≈ 1.4286, not every 1. The whole table wraps after s = 40
  (28 cycles). A tag "period" of P seconds therefore cycles every
  1.4286·P seconds. Keep it.
- The input s is (time + phase)/period wherever a tag has those fields;
  a period of 0 is treated as 1 where the game guards it (texture
  animation) and is invalid elsewhere (never 0 on the disc where read).

### Animation sources

| Code | Meaning |
|---|---|
| 0 | none: the value 1 (or white for colour sources) |
| 1 | the object's function A output |
| 2 | B |
| 3 | C |
| 4 | D |

The four outputs (scalars in 0..1, and colours for colour sources) are
the owning object's exported functions (`objects.md`). Level geometry
has no object; every source then reads as none.

Which object owns a drawn model, and when its values are taken:

- An object's own model: that object's outgoing functions A to D and
  its change colours A to D.
- The first-person weapon model: the **held weapon object's** values
  (not the player's unit's); the first-person arms model: the holder
  unit's. A weapon's world model, in a hand or on the ground, uses the
  weapon object's.
- The values are the ones stored by the object's last tick (`objects.md`,
  Per-tick order): nothing is recomputed per frame, so a value-driven
  picture changes at most once per tick. The functions' active flags
  are not passed: every consumer in this spec reads the stored value
  as is (an inactive function's stored value is what its bounds left,
  usually the lower bound).
- The same four values feed every use: numeric shaders, meter
  sources, texture animation sources (model, chicago and generic
  maps), the model shader's self-illumination and change-colour
  sources, fade sources, and the plasma shader's sources. `weapons.md`
  section 18 lists every weapon display built on them.

### Detail functions

| Code | Name | Result for value a and detail d |
|---|---|---|
| 0 | double biased multiply | clamp(2·a·d) |
| 1 | multiply | a·d |
| 2 | double biased add | clamp(a + 2·d − 1) |

The neutral detail value is ½ for codes 0 and 2 and 1 for code 1.

### Framebuffer blend functions (transparent shaders)

| Code | Name | New frame colour (src = shader output, dst = frame) |
|---|---|---|
| 0 | alpha blend | src.rgb·src.a + dst·(1 − src.a) |
| 1 | multiply | src·dst |
| 2 | double multiply | src·dst + dst·src = 2·src·dst |
| 3 | add | src + dst |
| 4 | subtract | dst − src |
| 5 | component min | min(src, dst) |
| 6 | component max | max(src, dst) |
| 7 | alpha-multiply add | src + dst·(1 − src.a) (premultiplied over) |

Results clamp to [0, 1] per channel. Only codes 0 and 3 occur on the
multiplayer maps.

### Framebuffer fade modes

| Code | Name | Fade factor k (inferred meaning) |
|---|---|---|
| 0 | none | k = the vertex's base fade (fog and fade source) |
| 1 | fade when perpendicular | as 0, also fading where the surface faces the viewer |
| 2 | fade when parallel | as 0, also fading where the surface is seen edge-on |

How k is applied per blend function is in "Fade and fog stage".

### Other enumerations

| Field | Codes |
|---|---|
| environment type | 0 normal, 1 blended, 2 blended base specular |
| environment reflection type | 0 bumped cube map, 1 flat cube map, 2 bumped radiosity |
| model detail mask | 0 none, 1 reflection mask inverse, 2 reflection mask, 3 self-illumination mask inverse, 4 self-illumination mask, 5 change-colour mask inverse, 6 change-colour mask, 7 multipurpose alpha inverse, 8 multipurpose alpha |
| transparent first-map type | 0 2D map, 1 first map is reflection cube map, 2 first map is object-centred cube map, 3 first map is viewer-centred cube map |
| chicago map colour/alpha function | 0..12, table under "Chicago" |
| glass reflection type | 0 bumped cube map, 1 flat cube map, 2 dynamic mirror |
| generic stage input | 0..24, table under "Generic" |
| generic stage input mapping | 0 clamp(x), 1 1 − clamp(x), 2 2·clamp(x) − 1, 3 1 − 2·clamp(x), 4 clamp(x) − ½, 5 ½ − clamp(x), 6 x, 7 −x |
| generic stage output | 0 discard, 1 scratch colour 0, 2 scratch colour 1, 3 vertex colour 0, 4 vertex colour 1, 5..8 map colour 0..3 |
| generic stage output function | 0 multiply, 1 dot product |
| generic stage output mapping | 0 identity, 1 ×½, 2 ×2, 3 ×4, 4 bias −½, 5 expand 2x − 1 |

## Texture animation block (0x38 bytes)

Used by the model shader, chicago maps and generic maps.

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 2 | i16 | U source (animation source) |
| 0x02 | 2 | i16 | U function (periodic) |
| 0x04 | 4 | f32 | U period, seconds |
| 0x08 | 4 | f32 | U phase, seconds |
| 0x0C | 4 | f32 | U scale, texture units |
| 0x10 | 2 | i16 | V source |
| 0x12 | 2 | i16 | V function |
| 0x14 | 4 | f32 | V period |
| 0x18 | 4 | f32 | V phase |
| 0x1C | 4 | f32 | V scale |
| 0x20 | 2 | i16 | Rotation source |
| 0x22 | 2 | i16 | Rotation function |
| 0x24 | 4 | f32 | Rotation period |
| 0x28 | 4 | f32 | Rotation phase |
| 0x2C | 4 | f32 | Rotation scale, degrees |
| 0x30 | 8 | 2 × f32 | Rotation centre (u, v) |

The texture transform. Inputs: map scale (su, sv), map offset (ou, ov)
and map rotation r₀ (degrees) where the owner has them (else 1, 1, 0,
0, 0), time t:

- Each period of 0 counts as 1. For each of u, v, rotation: value =
  periodic(function, (phase + t)/period) × scale × source value (source
  none = 1).
- U = ou − cx + u-value; V = ov − cy + v-value; θ = rotation-value +
  r₀ in degrees; c = cos θ, s = sin θ (exactly 1 and 0 when θ = 0).
- u′ = c·su·u − s·sv·v + (c·U − s·V + cx);
  v′ = s·su·u + c·sv·v + (c·V + s·U + cy).
- The common disc pattern "function one, scale 1" shifts by a whole
  texture (no visible effect); "rotation function one, scale 360"
  rotates by 360° (none).

## Environment shader (`senv`, 0x344 bytes)

### Layout

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x000 | 40 | | Common header |
| 0x028 | 2 | u16 | Flags: bit 0 alpha tested, bit 1 bump map is specular mask, bit 2 true atmospheric fog |
| 0x02A | 2 | i16 | Type: 0 normal, 1 blended, 2 blended base specular |
| 0x02C | 4 | f32 | Lens flare spacing, world units (0 = one per surface; `effects.md`) |
| 0x030 | 16 | ref | Lens flare (`lens`) |
| 0x040 | 44 | | Unused |
| 0x06C | 2 | u16 | Diffuse flags: bit 0 rescale detail maps, bit 1 rescale bump map |
| 0x06E | 2 | | Padding |
| 0x070 | 24 | | Unused |
| 0x088 | 16 | ref | Base map (`bitm`, 2D) |
| 0x098 | 24 | | Unused |
| 0x0B0 | 2 | i16 | Detail map function (detail functions) |
| 0x0B2 | 2 | | Padding |
| 0x0B4 | 4 | f32 | Primary detail map scale (× base uv) |
| 0x0B8 | 16 | ref | Primary detail map (`bitm`) |
| 0x0C8 | 4 | f32 | Secondary detail map scale |
| 0x0CC | 16 | ref | Secondary detail map (`bitm`) |
| 0x0DC | 24 | | Unused |
| 0x0F4 | 2 | i16 | Micro detail map function (detail functions) |
| 0x0F6 | 2 | | Padding |
| 0x0F8 | 4 | f32 | Micro detail map scale |
| 0x0FC | 16 | ref | Micro detail map (`bitm`) |
| 0x10C | 12 | rgb | Material colour (multiplies the lightmap) |
| 0x118 | 12 | | Unused |
| 0x124 | 4 | f32 | Bump map scale |
| 0x128 | 16 | ref | Bump map (`bitm`) |
| 0x138 | 8 | 2 × f32 | Runtime bump map scale (u, v): the scale the game uses; equals (bump scale, bump scale) on every disc tag |
| 0x140 | 16 | | Unused |
| 0x150 | 2 | i16 | U animation function (periodic) |
| 0x152 | 2 | | Padding |
| 0x154 | 4 | f32 | U animation period, seconds (never 0) |
| 0x158 | 4 | f32 | U animation scale, texture units |
| 0x15C | 2 | i16 | V animation function |
| 0x15E | 2 | | Padding |
| 0x160 | 4 | f32 | V animation period |
| 0x164 | 4 | f32 | V animation scale |
| 0x168 | 24 | | Unused |
| 0x180 | 2 | u16 | Self-illumination flags: bit 0 unfiltered |
| 0x182 | 2 | | Padding |
| 0x184 | 24 | | Unused |
| 0x19C | 12 | rgb | Primary on colour |
| 0x1A8 | 12 | rgb | Primary off colour |
| 0x1B4 | 2 | i16 | Primary animation function |
| 0x1B6 | 2 | | Padding |
| 0x1B8 | 4 | f32 | Primary animation period, seconds |
| 0x1BC | 4 | f32 | Primary animation phase, seconds |
| 0x1C0 | 24 | | Unused |
| 0x1D8 | 12 | rgb | Secondary on colour |
| 0x1E4 | 12 | rgb | Secondary off colour |
| 0x1F0 | 2 | i16 | Secondary animation function |
| 0x1F2 | 2 | | Padding |
| 0x1F4 | 4 | f32 | Secondary animation period |
| 0x1F8 | 4 | f32 | Secondary animation phase |
| 0x1FC | 24 | | Unused |
| 0x214 | 12 | rgb | Plasma on colour |
| 0x220 | 12 | rgb | Plasma off colour |
| 0x22C | 2 | i16 | Plasma animation function |
| 0x22E | 2 | | Padding |
| 0x230 | 4 | f32 | Plasma animation period |
| 0x234 | 4 | f32 | Plasma animation phase |
| 0x238 | 24 | | Unused |
| 0x250 | 4 | f32 | Self-illumination map scale |
| 0x254 | 16 | ref | Self-illumination map (`bitm`) |
| 0x264 | 24 | | Unused |
| 0x27C | 2 | u16 | Specular flags: bit 0 overbright, bit 1 extra shiny, bit 2 lightmap is specular |
| 0x27E | 2 | | Padding (0 on the disc) |
| 0x280 | 16 | | Unused |
| 0x290 | 4 | f32 | Specular brightness, 0..1 |
| 0x294 | 20 | | Unused |
| 0x2A8 | 12 | rgb | Specular perpendicular colour |
| 0x2B4 | 12 | rgb | Specular parallel colour |
| 0x2C0 | 16 | | Unused |
| 0x2D0 | 2 | u16 | Reflection flags: bit 0 dynamic mirror |
| 0x2D2 | 2 | i16 | Reflection type: 0 bumped cube map, 1 flat cube map, 2 bumped radiosity |
| 0x2D4 | 4 | f32 | Lightmap brightness scale, 0..1 |
| 0x2D8 | 28 | | Unused |
| 0x2F4 | 4 | f32 | Perpendicular brightness, 0..1 |
| 0x2F8 | 4 | f32 | Parallel brightness, 0..1 |
| 0x2FC | 16 | | Unused |
| 0x30C | 4 | f32 | Mirror index of refraction (dynamic mirrors; 0 on the disc) |
| 0x310 | 4 | f32 | Mirror depth (0 on the disc) |
| 0x314 | 16 | | Unused |
| 0x324 | 16 | ref | Reflection cube map (`bitm`, cube) |
| 0x334 | 16 | | Unused |

Every byte outside the named fields is zero in all 180 disc tags.

### Textures the passes use when a map is "none"

A missing map is replaced by an image of the globals' default 2D
texture (`rasterizer\default 2d`, four 4×4 images, checked on the
disc): image 0 black (alpha 1), image 1 white, image 2 grey 128/255,
image 3 flat normal (128, 128, 255, alpha 255). The base map uses the
white image, detail and micro detail maps the grey one (128/255 ≈
0.502, so a missing detail map brightens by 2·0.502 = 1.0039 under
double biased multiply), the bump map the flat normal, the
self-illumination map black.

### Texture coordinates

- uv is the rendered vertex's texture coordinate (`structure_bsp.md`).
- Animation offset: Δu = periodic(U function, t / U period) × U scale,
  Δv likewise. No phase, no source.
- Base map: (u + Δu, v + Δv) (inferred: the offset is passed with the
  base transform; see Open questions).
- Primary detail: base coordinate × (primary scale × ku, primary scale
  × kv); secondary and micro likewise with their scales. With the
  "rescale detail maps" flag, ku = base width / detail width and kv =
  base height / detail height (each map its own); otherwise 1.
- Bump map: base coordinate × runtime bump scale. Self-illumination
  map: base coordinate × self-illumination map scale.
- All these maps wrap and are filtered bilinearly with mipmaps; the
  self-illumination map uses point sampling when its "unfiltered" flag
  is set. The lightmap uses its own coordinates, clamped
  (`structure_bsp.md`).

### How a level surface is drawn (passes)

Opaque level geometry is drawn in this order of passes, each over the
whole visible level before the next; later passes test depth "equal"
and do not write depth.

1. **Lightmap pass** (depth test less-or-equal, depth write on, no
   blending, writes rgb and alpha): frame.rgb = L (below). Alpha test
   (alpha > 127/255) when the "alpha tested" flag is set, on the alpha
   described under "Alpha testing".
2. Dynamic light diffuse passes (additive, per light; not needed for a
   first renderer).
3. Decals of the light and alpha-tested layers (`effects.md`).
4. **Diffuse texture pass** (blend frame.rgb ← frame.rgb × D.rgb; frame
   alpha ← D.a when specular masking is on, which it is by default):
   D below.
5. Decals of the primary and secondary layers.
6. Dynamic light specular passes (additive, × frame alpha).
7. **Specular lightmap pass**, only for shaders with specular brightness
   > 0 and the "lightmap is specular" flag (1 disc tag): frame.rgb +=
   S × frame.a.
8. **Reflection lightmap mask pass**, for shaders with a reflection
   (perpendicular or parallel brightness > 0) and lightmap brightness
   scale < 1: frame.a ← frame.a × M.
9. Dynamic mirrors (none on the multiplayer maps).
10. **Reflection pass**, for shaders with perpendicular or parallel
    brightness > 0 and a cube map: frame.rgb += R × frame.a.
11. Transparent level geometry is queued (draw order section).
12. **Fog pass** over all opaque level geometry (fog section).

So, ignoring dynamic lights and decals, a level pixel ends as:

colour = clamp( clamp(L) × clamp(D.rgb) + R × A ), then fogged, where
A = D.a × M is the specular/reflection mask.

### Lightmap pass value L

- B = bump map texel (or flat normal), n = 2·B.rgb − 1 (tangent space,
  no renormalisation).
- l = the lightmap vertex's incident light direction in tangent space,
  normalised per pixel (through a normalisation cube map).
- w = a per-vertex weight in 0..1 from the lightmap vertex (inferred:
  how directional the baked light is; see Open questions).
- shade = (1 − w) + w·clamp(n·l), with n·l clamped to [0, 1] by the
  combiner (it is a dot product whose negative part is lost when
  read as an unsigned input).
- With the "bump map is specular mask" flag the bump map is not bound
  here (flat normal: n·l = l.z).
- L = lightmap.rgb × shade × material colour + SI, clamped to [0, 1].
- Without a self-illumination map SI = 0.
- The lightmap texel is used as stored: **no ×2 or other scale.**

Self-illumination SI (only when a self-illumination map is set):

- Values: p = periodic(primary function, (t + primary phase)/primary
  period), likewise q for secondary and z for plasma.
- Primary colour P = off·(1 − p) + on·p; secondary colour Q likewise
  with q. Plasma uses its on and off colours unblended.
- With I = the self-illumination map texel:
  - d = z − I.a; m₁ = (1 − 2|d|)² (computed so that it is 1 at d = 0
    and at |d| = 1, 0 at |d| = ½);
  - m₂ = m₁²; m₃ = (2·m₂ − 1)² if m₂ ≥ ½, else 0 (a narrow band:
    non-zero only for |d| < 0.0795);
  - plasma colour C = plasma on × m₃ + plasma off (the off colour is
    added unconditionally);
  - SI = I.r·P + I.g·Q + I.b·C, each step clamped to [0, 1].
- So the red channel of the map paints the primary colour, green the
  secondary, blue the plasma colour, and alpha holds the "plasma"
  threshold that the animated value sweeps through.
- Because L is multiplied by D in the texture pass, self-illumination
  is tinted by the base texture.

### Diffuse texture pass value D

Inputs: T = base texel, P = primary detail texel, S = secondary detail
texel, U = micro detail texel.

Detail blend, by type:

| Type | Detail colour E.rgb | Mask alpha E.a |
|---|---|---|
| 0 normal | S.rgb·(1 − S.a) + P.rgb·S.a | S.a·(1 − S.a) + P.a·S.a, then × T.a |
| 1 blended | S.rgb·(1 − T.a) + P.rgb·T.a | S.a·(1 − T.a) + P.a·T.a |
| 2 blended base specular | as blended | T.a |

- Normal: the secondary detail map's alpha chooses between the
  detail maps (1 = primary); the base alpha is the specular mask.
- Blended: the base map's alpha chooses (1 = primary); the base alpha
  is not a mask.
- Blended base specular: base alpha chooses the detail and is the mask.

Then:

- C₁ = detail function(T.rgb, E.rgb) with the detail map function.
- D.rgb = detail function(C₁, U.rgb) with the micro detail function.
- D.a = E.a × U.a.
- Every step clamps to [−1, 1] and the output to [0, 1], so
  2·T·E above 1 is lost (it saturates) before the micro detail is
  applied.

### Specular lightmap pass value S (1 tag on the disc)

Per pixel, with n the bump normal (flat (0, 0, 1) when "bump map is
specular mask"), V the normalised tangent-space eye vector and l the
normalised incident direction:

- c = n·V; R = 2·n·c − V (reflection of the eye vector).
- Self-shadowing: σ = clamp(8·l.z)·clamp(8·V.z), each factor clamped
  to [−1, 1] (z is the tangent-space height): 1 when both vectors are
  clearly above the surface, fading over the last 1/8 near the horizon.
  When both are below it the product is +1 again (kept as decoded).
- Lightmap intensity λ = clamp(0.502·(lm.r + lm.g + lm.b)); with "bump
  map is specular mask" λ is multiplied by the bump map texel (the
  mask).
- F = parallel colour·(1 − c) + perpendicular colour·c.
- k = 2, or 4 with the "overbright" flag; h = clamp(R·l)^8 (^32 with
  "extra shiny").
- S = clamp(k·F·λ) × (σ × brightness) × h × 0.502.
- The pass alpha-tests on an internal value; pixels with zero result
  are skipped (no visible effect).

### Reflection mask value M

- λ = clamp(lm.r·0.502 + lm.g·0.690 + lm.b·0.314) (constant
  (128, 176, 80)/255).
- M = (1 − λ)·s + λ where s = lightmap brightness scale.
- So reflections in dark areas are scaled down to s.

### Reflection pass value R

- Effective type: "bumped cube map" or "bumped radiosity" becomes "flat
  cube map" when the bump map is none or the "bump map is specular
  mask" flag is set. The two bumped types draw the same way.
- Bumped: N = the bump normal turned into world space by the vertex's
  tangent frame; E = vector from the surface to the eye; the cube map is
  sampled with the reflection of E about N. The fresnel cosine is c =
  N·(−camera forward) (the camera's forward axis, not the per-pixel eye
  vector).
- Flat with no bump map: the cube map is sampled with a per-vertex
  reflection vector; c = (vertex normal)·(−camera forward) (which
  per-vertex vector is inferred).
- Flat with a bump map present (forced flat by the specular-mask flag):
  c = (2·B − 1)·(per-vertex normalised vector, inferred to be the
  tangent-space eye vector) using the bump texel B. Quirk; keep.
- K = cube texel; K⁸ = K raised to the 8th power per channel.
- a = c²; tint = parallel colour·(1 − a) + perpendicular colour·a
  (these are the **specular** colours at 0x2A8/0x2B4); brightness β =
  parallel brightness·(1 − a) + perpendicular brightness·a.
- R = (K⁸·(1 − tint) + K·tint) × β, times the bump texel when "bump
  map is specular mask" is set.
- The tint colour therefore chooses, per channel, between a
  high-contrast (K⁸) and a plain (K) reflection.

### Alpha testing

- The lightmap pass alpha is the **bump map's alpha** (flat default
  alpha 1 when there is no bump map, or when the "bump map is specular
  mask" flag is set). Alpha-tested level surfaces (29 disc tags) cut
  out where the bump map alpha ≤ 127/255.
- Later passes test depth "equal", so cut-out pixels get no texture,
  specular or fog from this surface.

### Environment shaders on objects

Scenery and other objects may use environment shaders. They are then
drawn in one pass like a model shader (below) with: base map, primary
detail map with the detail function and no mask, no change colour, no
self-illumination, the reflection cube map, vertex lighting instead of
the lightmap, and per-vertex fog unless "true atmospheric fog" is set
(then the hardware's per-vertex fog with the atmospheric colour).

### Minimum faithful approximation (environment)

One base texture, optional second texture:

- Base: base map. Second texture: primary detail map (or, for type 0
  when only a secondary exists, the secondary), uv scale = its detail
  scale, combined as 2·base·detail clamped (all disc tags use double
  biased multiply). Without any detail map: no second texture.
- Lighting: lightmap × material colour (white on 177 of 180 tags), no
  scale; final = clamp(lightmap) × clamp(2·base·detail).
- Alpha mode: opaque; mask (reference 0.5, i.e. > 127/255) when the
  alpha-tested flag is set, **taking the mask from the bump map's
  alpha**, not the base map.
- Double-sided: no (level geometry carries both faces when needed).
- Unlit: no. Tint: material colour. UV transform: offset (Δu, Δv) from
  the u/v animation (constant on the disc).
- Optional additive extras: self-illumination (add before the texture
  multiply), and the cube reflection added with weight (base alpha for
  normal type) × reflection brightness.

## Model shader (`soso`, 0x1B8 bytes)

### Layout

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x000 | 40 | | Common header |
| 0x028 | 2 | u16 | Flags: bit 0 detail after reflection, bit 1 two-sided, bit 2 not alpha-tested, bit 3 alpha-blended decal, bit 4 true atmospheric fog, bit 5 disable two-sided culling |
| 0x02A | 2 | | Padding |
| 0x02C | 12 | | Unused |
| 0x038 | 4 | f32 | Translucency, 0..1 (light passing to the back side) |
| 0x03C | 16 | | Unused |
| 0x04C | 2 | i16 | Change colour source (animation source; colour) |
| 0x04E | 30 | | Unused |
| 0x06C | 2 | u16 | Self-illumination flags: bit 0 no random phase |
| 0x06E | 2 | | Padding |
| 0x070 | 2 | i16 | Self-illumination colour source (animation source; colour) |
| 0x072 | 2 | i16 | Self-illumination animation function (periodic) |
| 0x074 | 4 | f32 | Self-illumination animation period, seconds |
| 0x078 | 12 | rgb | Self-illumination colour lower bound |
| 0x084 | 12 | rgb | Self-illumination colour upper bound |
| 0x090 | 12 | | Unused |
| 0x09C | 4 | f32 | Map u scale (0 means 1 in the tool; never 0 on the disc) |
| 0x0A0 | 4 | f32 | Map v scale |
| 0x0A4 | 16 | ref | Base map (`bitm`) |
| 0x0B4 | 8 | | Unused |
| 0x0BC | 16 | ref | Multipurpose map (`bitm`) |
| 0x0CC | 8 | | Unused |
| 0x0D4 | 2 | i16 | Detail function (detail functions) |
| 0x0D6 | 2 | i16 | Detail mask (model detail mask) |
| 0x0D8 | 4 | f32 | Detail map scale |
| 0x0DC | 16 | ref | Detail map (`bitm`) |
| 0x0EC | 4 | f32 | Detail map v scale (multiplies the detail scale for v) |
| 0x0F0 | 12 | | Unused |
| 0x0FC | 56 | anim | Texture animation block |
| 0x134 | 8 | | Unused |
| 0x13C | 4 | f32 | Reflection falloff distance, world units |
| 0x140 | 4 | f32 | Reflection cutoff distance, world units (0 = no distance fade) |
| 0x144 | 4 | f32 | Perpendicular brightness, 0..1 |
| 0x148 | 12 | rgb | Perpendicular tint colour |
| 0x154 | 4 | f32 | Parallel brightness, 0..1 |
| 0x158 | 12 | rgb | Parallel tint colour |
| 0x164 | 16 | ref | Reflection cube map (`bitm`, cube) |
| 0x174 | 16 | | Unused |
| 0x184 | 4 | | Leftover, unused: 1.0 or a byte-swapped 1.0, or 0 |
| 0x188 | 16 | | Leftover tag reference, unused: class bytes stored unswapped, garbage name, id none. Do not resolve |
| 0x198 | 32 | | Unused |

### Texture coordinates

- Base and multipurpose maps: the texture transform of the animation
  block with su = object base-map scale × map u scale, sv = object
  scale × map v scale (the object's scale is 1 except where an object
  sets it), offsets and rotation 0, animation sources from the object.
- Detail map: model uv × (detail scale, detail scale × detail v scale).
- Reflection: a cube map sampled with the per-vertex reflection vector.

### Multipurpose map channels on the Xbox

Read from the disc build's model pixel program:

| Channel | Meaning |
|---|---|
| red | reflection (specular) mask |
| green | self-illumination mask |
| blue | not used |
| alpha | change-colour mask |

The detail masks read: reflection mask = red, self-illumination mask =
green, change-colour mask = alpha, multipurpose alpha = alpha (masks 5,
6 and 7, 8 therefore give the same result on the Xbox). The PC release
documents a different channel order; the disc's multipurpose maps are
authored for the order above.

### What it draws

Per object (once per draw):

- Self-illumination colour: φ = 0 with "no random phase", else a random
  value in [0, 1] from the generator seeded with the object's unique
  identifier; f = periodic(function, t / period + φ); C_si = lower +
  (upper − lower)·f; multiplied per channel by the source colour when
  the source is A..D.
- Change colour: the source colour (A..D), else white.
- Reflection distance fraction: with cutoff ≠ 0, ρ = clamp((d −
  cutoff)/(falloff − cutoff)) with d the object centroid's depth along
  the camera forward axis; else ρ = 1. (On the disc falloff < cutoff,
  so reflections are full nearer than the falloff distance and gone
  beyond the cutoff.)
- Perpendicular (brightness, tint) and parallel (brightness, tint) are
  multiplied by the object's reflection tint (argb from its lighting,
  `structure_bsp.md` lighting block) and the brightnesses by ρ.

Per vertex (vertex program; inferred from its inputs):

- Diffuse light V₀ = ambient + Σ distant lights colour·max(0, n·−dir)
  (up to 2) + Σ point lights (up to 2: colour × distance falloff 1 −
  d²/r², × spot cone falloff), from the object's lighting
  (`objects.md`, `structure_bsp.md`). Translucency changes how light
  wraps to the back (Open questions).
- Reflection tint V₁ = lerp(parallel, perpendicular, fresnel term of
  n·eye), its alpha the matching brightness.
- Planar fog fraction p (0 when there is no planar fog).

Per pixel, with T base, X detail, Mp multipurpose, K reflection cube:

- tint = white·(1 − Mp.a) + change colour·Mp.a.
- lit = (V₀ + Mp.g·C_si) × tint.
- Detail mask m from the detail mask (none = 1); detail X′ =
  neutral·(1 − m) + X·m, neutral = ½ for double biased multiply and
  add, 1 for multiply.
- Without "detail after reflection": c = detail(T, X′) × lit + K·V₁ ×
  (Mp.r × V₁.a).
- With "detail after reflection": c = detail(T × lit + K·V₁·(Mp.r ×
  V₁.a), X′).
- Output alpha = T.a.
- Fog (not true atmospheric): with F = atmospheric fog density at the
  object's centroid depth (fog section) and per-vertex p:
  out = (1 − p)·c·(1 − F) + p·g₀ + F·atmospheric colour − p·g₁, where
  g₀ = clamp(planar colour − F·mix) and g₁ = clamp(−(planar colour −
  F·mix)), mix = atmospheric colour·(1 − e) + planar colour·e, e =
  clamp(camera height above the fog plane / atmospheric maximum
  distance) (1 when the fog is "atmosphere dominant"). With no planar
  fog (p = 0) this is c·(1 − F) + atmospheric colour·F.
- True atmospheric fog: out = c·(1 − f) + atmospheric colour·f with f
  the per-vertex fog.

State:

- Depth test less-or-equal; depth write on, except alpha-blended
  decals (no write, z-bias 8).
- Alpha test (alpha > 127/255) unless "not alpha-tested" or
  "alpha-blended decal"; alpha-blended decals blend src·a + dst·(1 − a).
- Culling is always back faces; the two-sided flags have no effect when
  drawing (two-sided model parts carry both windings; Open questions).
- Transparent parts of models (other shader classes) are queued with
  the transparent geometry.

### Minimum faithful approximation (model)

- Base: base map; uv transform: map scales and the animation block.
- Tint: none, except change colour × multipurpose alpha (if the
  converter supports a mask, else tint the whole base where the change
  colour source is set: 57 disc tags).
- Second texture: detail map with its scale, 2·base·detail (or base ×
  detail for multiply).
- Lighting: vertex/object lighting, no scale.
- Alpha mode: mask at 0.5 from base alpha unless "not alpha-tested";
  blend for "alpha-blended decal". Double-sided: no.
- Unlit: no; self-illumination (multipurpose green × colour) can be an
  emissive term.

## Transparent shaders: shared block

Chicago, chicago extended and generic shaders start with the same
block after the header:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x28 | 1 | u8 | Numeric counter limit (0..255) |
| 0x29 | 1 | u8 | Flags: bit 0 alpha-tested, bit 1 decal, bit 2 two-sided, bit 3 first map is in screen space, bit 4 draw before water, bit 5 ignore effect, bit 6 scale first map with distance, bit 7 numeric |
| 0x2A | 2 | i16 | First map type (0..3) |
| 0x2C | 2 | i16 | Framebuffer blend function (0..7) |
| 0x2E | 2 | i16 | Framebuffer fade mode (0..2) |
| 0x30 | 2 | i16 | Framebuffer fade source (animation source) |
| 0x32 | 2 | | Padding |
| 0x34 | 4 | f32 | Lens flare spacing, world units |
| 0x38 | 16 | ref | Lens flare (`lens`) |
| 0x48 | 12 | block | Extra layers: elements are one 16-byte shader reference each, maximum 4 |
| 0x54 | 12 | block | Maps, maximum 4 (element per class) |

- **Two-sided**: no culling; else back faces culled.
- **Alpha-tested**: alpha test > 127/255 on the shader's output alpha.
- **Decal**: the surface is drawn immediately (not sorted, not queued),
  without fog, with depth bias 8.
- **Draw before water**: sorted before water surfaces.
- **Ignore effect**: not replaced while the object has a modifier or
  camouflage effect.
- **First map in screen space**: map 0's coordinates come from the
  screen-space projection of the vertex (view forward and left axes)
  instead of its uv.
- **Scale first map with distance**: map 0's scale is multiplied by the
  surface's view depth.
- **Numeric**: the shader shows one digit of a number (below).
- Extra layers are other shaders drawn first, with the same geometry.
- First map type: map 0 is a 2D map (type 0) or a cube map (types 1..3,
  sampled with reflection, object-centred or viewer-centred vectors;
  inferred from the names; none on the disc). The other maps are always
  2D.
- Transparents never write depth; they test less-or-equal.

### Numeric shaders

**Which image a transparent map shows.** A chicago or generic shader
samples one *image* (one element of the bitmap tag's images block, not a
sequence) of each map's bitmap. Its index i is chosen once per draw of a
part and used for all of the shader's maps, each taking i modulo its
own bitmap's image count:

- By default i = the part's **shader permutation** (the permutation of
  the model's shader reference, `models.md` Shaders; the level's own
  index for level geometry). So a non-numeric shader drawn with
  permutation 2 shows image 2 of a multi-image bitmap.
- A shader with the numeric flag, at least one map, and an owning
  object (level geometry has none: it keeps the default) instead
  computes i as below, with k = the part's shader permutation read as
  the **digit position** (0 units, 1 tens, ...). The same shader is
  referenced once per digit position by the model, each part drawing
  one digit.

The digit:

- frame count n = number of images in map 0's bitmap.
- With the extra flag "numeric countdown timer" (chicago extra flags
  bit 1) the digit comes from the global countdown timer: position 0
  units of milliseconds, 1 hundredths, 2 tenths, 3 seconds, 4 tens of
  seconds (mod 6), 5 minutes, 6 tens of minutes (mod 6), 7 hours, 8
  tens of hours. (Chicago only; generic shaders have no extra flags.)
- Otherwise value = clamp(floor(limit × source + 0.5), 0, limit),
  single precision, with source = the owning object's outgoing
  function **D when n is exactly 8**, else **function A** (the image
  count picks the function: an 8-image bitmap is a compass dial, any
  other a digit strip); divide value by n, k times (integer division);
  i = value mod n.
- So every digit position always draws (leading zeros show), a value is
  capped at the limit, and positions beyond the model's parts are simply
  not drawn.
- The picture changes by switching images only: no texture offset is
  involved, and the maps' own texture transforms apply unchanged.
- 13 disc tags per map use it (limits 60 and 8). The assault rifle's
  counter (limit 60, 10 images, function A = rounds loaded / 60) and
  compass (limit 8, 8 images, function D = the compass input) are worked
  through in `weapons.md` section 18.

### Fade and fog stage

After the maps are combined (chicago or generic), one more stage
applies the fade factor k, a per-vertex value in 0..1 (inferred: 1
before fog, multiplied by the fade source value, by 1 − camouflage
intensity unless the chicago "don't fade active camouflage" extra flag
is set, by 1 − atmospheric fog, and for fade modes 1 and 2 by a view
angle term). The stage moves the output towards the blend function's
neutral value:

| Blend function | Fade |
|---|---|
| alpha blend | out.a ← out.a·k |
| multiply, component min | out.rgb ← out.rgb·k + (1 − k) (towards white) |
| double multiply | out.rgb ← out.rgb·k + ½·(1 − k) (towards ½) |
| add, subtract, component max | out.rgb ← out.rgb·k (towards black) |
| alpha-multiply add | out.rgb ← out.rgb·k and out.a ← out.a·k |

- Fade mode none reads the base factor, "fade when perpendicular" and
  "fade when parallel" read two other per-vertex factors.
- Exception: a sky surface with alpha blend instead blends towards the
  planar fog colour by the planar fog density at the camera
  (out = out·(1 − f) + planar colour·f; inferred from the inputs).
- The stage runs only while environment fog drawing is on (always in
  the game).

## Transparent chicago (`schi`, 0x6C bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 40 | | Common header |
| 0x28 | 56 | | Shared transparent block (maps element 0xDC bytes) |
| 0x60 | 4 | u32 | Extra flags: bit 0 don't fade active camouflage, bit 1 numeric countdown timer |
| 0x64 | 8 | | Unused |

Map element (0xDC bytes):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 2 | u16 | Flags: bit 0 unfiltered, bit 1 alpha replicate, bit 2 u-clamped, bit 3 v-clamped |
| 0x02 | 42 | | Unused |
| 0x2C | 2 | i16 | Colour function (how the next map joins; 0..12) |
| 0x2E | 2 | i16 | Alpha function (0..12) |
| 0x30 | 36 | | Unused |
| 0x54 | 4 | f32 | Map u scale |
| 0x58 | 4 | f32 | Map v scale |
| 0x5C | 4 | f32 | Map u offset |
| 0x60 | 4 | f32 | Map v offset |
| 0x64 | 4 | f32 | Map rotation, degrees |
| 0x68 | 4 | f32 | Mipmap bias (0 on the disc; not applied by the Xbox) |
| 0x6C | 16 | ref | Map (`bitm`) |
| 0x7C | 40 | | Unused |
| 0xA4 | 56 | anim | Texture animation block |

### Combine

- Maps 0..3 (at most 4) are sampled with their texture transforms; map
  0 wraps unless its type is a cube map (then clamps); others wrap
  unless their clamp flags are set. Unfiltered maps use point
  minification and mip selection (magnification stays bilinear).
- Start: C = map 0's texel (rgb and alpha).
- For i = 0 .. count − 2: N = map i+1's texel; C.rgb ← colour
  function of map i applied to (C, N); C.a ← alpha function of map i
  applied to (C.a, N.a). If map **i** has "alpha replicate", N.rgb is
  replaced by N.a in the colour function (the flag of the earlier map
  affects the later map's input; quirk, 1 disc map).
- The last map's functions are not used.
- Each step clamps to [−1, 1]; the output clamps to [0, 1].

| Code | Name | Colour (C current, N next) | Alpha (same formula on alphas) |
|---|---|---|---|
| 0 | current | C | C.a |
| 1 | next map | N | N.a |
| 2 | multiply | C·N | C.a·N.a |
| 3 | double multiply | 2·C·N | 2·C.a·N.a |
| 4 | add | C + N | C.a + N.a |
| 5 | add signed current | N + 2C − 1 | N.a + 2C.a − 1 |
| 6 | add signed next map | C + 2N − 1 | C.a + 2N.a − 1 |
| 7 | subtract current | N − C | N.a − C.a |
| 8 | subtract next map | C − N | C.a − N.a |
| 9 | blend current alpha | N·C.a + C·(1 − C.a) | N.a·C.a + C.a·(1 − C.a) |
| 10 | blend current alpha inverse | C·C.a + N·(1 − C.a) | C.a·C.a + N.a·(1 − C.a) |
| 11 | blend next map alpha | N·N.a + C·(1 − N.a) | N.a·N.a + C.a·(1 − N.a) |
| 12 | blend next map alpha inverse | C·N.a + N·(1 − N.a) | C.a·N.a + N.a·(1 − N.a) |

- Then the fade stage, then the framebuffer blend function.
- Lighting: none, except shaders with "transparent lit" (header bit 2)
  are meant to get vertex lighting; chicago's combine has no input for
  it, so it has no effect on the picture.

### Chicago extended (`scex`, 0x78 bytes; not on the Xbox disc)

Header, shared block (maps = the four-stage maps), 0x60 block of
two-stage maps (same 0xDC element, maximum 2), 0x6C u32 extra flags,
0x70 8 unused bytes. The game reads it as a chicago shader (its
four-stage maps).

### Minimum faithful approximation (chicago)

- Base: map 0 with its texture transform (scale, offset, rotation,
  animation evaluated per frame). Second texture: map 1 when map 0's
  colour function is multiply or double multiply (the common cases);
  otherwise drop maps 1..3 (or add map 1 for "add").
- Alpha mode: blend for blend function 0, add for 3; multiply / double
  multiply map to a multiplicative blend. Mask if alpha-tested.
- Double-sided: flag bit 2. Unlit: yes. Tint: none.

## Transparent generic (`sotr`, 0x6C bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 40 | | Common header |
| 0x28 | 56 | | Shared transparent block (maps element 0x64 bytes) |
| 0x60 | 12 | block | Stages (0x70 bytes each, maximum 7) |

Map element (0x64 bytes):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 2 | u16 | Flags: bit 0 unfiltered, bit 1 u-clamped, bit 2 v-clamped |
| 0x02 | 2 | | Padding |
| 0x04 | 4 | f32 | Map u scale |
| 0x08 | 4 | f32 | Map v scale |
| 0x0C | 4 | f32 | Map u offset |
| 0x10 | 4 | f32 | Map v offset |
| 0x14 | 4 | f32 | Map rotation, degrees |
| 0x18 | 4 | f32 | Mipmap bias |
| 0x1C | 16 | ref | Map (`bitm`) |
| 0x2C | 56 | anim | Texture animation block |

Stage element (0x70 bytes):

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 2 | u16 | Flags: bit 0 colour mux, bit 1 alpha mux, bit 2 A-out controls constant colour 0 animation |
| 0x02 | 2 | | Padding |
| 0x04 | 2 | i16 | Constant colour 0 animation source (colour) |
| 0x06 | 2 | i16 | Constant colour 0 animation function (periodic) |
| 0x08 | 4 | f32 | Constant colour 0 animation period, seconds |
| 0x0C | 16 | argb | Constant colour 0 lower bound |
| 0x1C | 16 | argb | Constant colour 0 upper bound |
| 0x2C | 16 | argb | Constant colour 1 |
| 0x3C | 16 | 4 × (i16, i16) | Colour inputs A, B, C, D: (input, mapping) each |
| 0x4C | 2 | i16 | Colour output AB (generic stage output) |
| 0x4E | 2 | i16 | Colour output AB function (0 multiply, 1 dot product) |
| 0x50 | 2 | i16 | Colour output CD |
| 0x52 | 2 | i16 | Colour output CD function |
| 0x54 | 2 | i16 | Colour output AB/CD mux-sum |
| 0x56 | 2 | i16 | Colour output mapping (0..5) |
| 0x58 | 16 | 4 × (i16, i16) | Alpha inputs A, B, C, D: (input, mapping) |
| 0x68 | 2 | i16 | Alpha output AB |
| 0x6A | 2 | i16 | Alpha output CD |
| 0x6C | 2 | i16 | Alpha output AB/CD mux-sum |
| 0x6E | 2 | i16 | Alpha output mapping (0..5) |

Stage inputs:

| Code | As a colour input | As an alpha input |
|---|---|---|
| 0 | zero | zero |
| 1 | one | one |
| 2 | one half | one half |
| 3 | negative one | negative one |
| 4 | negative one half | negative one half |
| 5..8 | map colour 0..3 (rgb) | map alpha 0..3 |
| 9 | vertex colour 0 rgb | vertex alpha 0 |
| 10 | vertex colour 1 rgb | vertex alpha 1 |
| 11 | scratch colour 0 rgb | scratch alpha 0 |
| 12 | scratch colour 1 rgb | scratch alpha 1 |
| 13 | constant colour 0 rgb | constant alpha 0 |
| 14 | constant colour 1 rgb | constant alpha 1 |
| 15..18 | map alpha 0..3, replicated | map **blue** 0..3 |
| 19 | vertex alpha 0, replicated | vertex blue 0 |
| 20 | vertex alpha 1, replicated | vertex blue 1 |
| 21 | scratch alpha 0, replicated | scratch blue 0 |
| 22 | scratch alpha 1, replicated | scratch blue 1 |
| 23 | constant alpha 0, replicated | constant blue 0 |
| 24 | constant alpha 1, replicated | constant blue 1 |

So codes 5..14 name "the colour" and 15..24 "the alpha" for colour
inputs, but for **alpha inputs** codes 5..14 read alpha and 15..24 read
the **blue** channel. The constants 0..4 are passed through the mapping
as exact math (e.g. "one" with mapping 3 is −1).

Each stage, for colour and alpha separately:

- a, b, c, d = the four mapped inputs.
- ab = a·b, or the 3-component dot product a·b replicated to rgb when
  the AB function is "dot product" (colour only); cd likewise.
- s = ab + cd; with the mux flag instead s = (scratch alpha 0 ≥ ½) ? cd
  : ab (the test reads scratch alpha 0's top bit, i.e. ≥ 128/255).
- Apply the output mapping to ab, cd and s (identity, ×½, ×2, ×4, −½,
  2x − 1), clamp each to [−1, 1], and write them to their output
  registers (discard = not written). All of a stage's reads happen
  before its writes.
- Registers start as: map colours = the sampled maps, vertex colours =
  per-vertex values (vertex colour 0 is the lighting for "transparent
  lit" shaders; otherwise inferred to be white/fade values), scratch =
  0, constants as below. Map registers may be overwritten by stages.
- Constant colour 0 per stage: f = object function A when bit 2 is set
  and the shader has an object, else periodic(function, t / period);
  c0 = lower + (upper − lower)·f per argb component, clamped to [0, 1],
  then rgb × the source colour when the source is A..D. Constant colour
  1 = the stage's colour 1.
- With no stages, the output is map 0 (rgb and alpha).
- Output: scratch colour 0 rgb and scratch alpha 0, then the fade
  stage and the blend function.

### Minimum faithful approximation (generic)

- Base: map 0 with its texture transform. Tint: constant colour 0 of
  the first stage that reads constant colour 0 multiplied with map 0
  (common for light cards); else white.
- Alpha mode: blend (0) or add (3) from the blend function. Double-sided:
  flag bit 2. Unlit: yes.
- 128 of 267 disc tags have no stages (exactly map 0); 76 use all 7
  stages (energy and shield effects) and need the full combiner to look
  right.

## Glass (`sgla`, 0x1E0 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x000 | 40 | | Common header |
| 0x028 | 2 | u16 | Flags: bit 0 alpha-tested, bit 1 decal, bit 2 two-sided, bit 3 bump map is specular mask |
| 0x02A | 2 | | Padding |
| 0x02C | 40 | | Unused |
| 0x054 | 12 | rgb | Background tint colour |
| 0x060 | 4 | f32 | Background tint map scale |
| 0x064 | 16 | ref | Background tint map (`bitm`) |
| 0x074 | 20 | | Unused |
| 0x088 | 2 | u16 | Reflection flags (none used) |
| 0x08A | 2 | i16 | Reflection type: 0 bumped cube map, 1 flat cube map, 2 dynamic mirror |
| 0x08C | 16 | argb | Perpendicular colour (alpha = brightness) |
| 0x09C | 16 | argb | Parallel colour (alpha = brightness) |
| 0x0AC | 16 | ref | Reflection map (`bitm`, cube) |
| 0x0BC | 4 | f32 | Bump map scale |
| 0x0C0 | 16 | ref | Bump map (`bitm`) |
| 0x0D0 | 128 | | Unused |
| 0x150 | 2 | u16 | Diffuse flags (none used) |
| 0x152 | 2 | | Padding |
| 0x154 | 4 | f32 | Diffuse map scale |
| 0x158 | 16 | ref | Diffuse map (`bitm`) |
| 0x168 | 4 | f32 | Diffuse detail map scale |
| 0x16C | 16 | ref | Diffuse detail map (`bitm`) |
| 0x17C | 32 | | Unused |
| 0x19C | 4 | f32 | Specular map scale (not drawn by the Xbox) |
| 0x1A0 | 16 | ref | Specular map (none on the disc) |
| 0x1B0 | 4 | f32 | Specular detail map scale (not drawn) |
| 0x1B4 | 16 | ref | Specular detail map (none on the disc) |
| 0x1C4 | 28 | | Unused |

Layers, drawn in this order (each only if it has content):

1. **Tint** (if a tint map or a non-black tint colour): frame ← frame ×
   lerp(1, tint colour × tint map texel, v) with v a per-vertex factor
   (inferred: 1 − fog). Disc: no tint maps; tint colours white or a pale
   blue.
2. **Reflection** (if perpendicular or parallel alpha > 0, and a
   reflection map or the mirror type): same per-pixel math as the
   environment reflection, with K the reflection cube (bumped by the
   glass bump map scaled by the bump map scale, flat otherwise, mirror
   = the mirror image), a = c², tint = lerp(parallel rgb, perpendicular
   rgb, a), β = lerp(parallel alpha, perpendicular alpha, a);
   R = (K⁸·(1 − tint) + K·tint) × β × (bump texel when the mask flag is
   set, else a per-vertex factor); frame += R (additive).
3. **Diffuse** (if a diffuse or detail map): colour = clamp(2 × diffuse
   × detail) × (lightmap + vertex lighting), alpha = diffuse.a ×
   detail.a × vertex alpha; blended src·a + dst·(1 − a), alpha test > 0.
   Missing maps use the defaults (white / grey). Glass on level geometry
   is lit by the lightmap of its lightmap group (`structure_bsp.md`).
- Culling per the two-sided flag. Disc: every glass has a reflection
  map and a diffuse map; 13 bumped, 68 flat.

Minimum: base = diffuse map, blend, two-sided per flag, lit; add the
reflection cube × perpendicular alpha as an additive layer if the
converter allows a second pass.

## Meter (`smet`, 0x104 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 40 | | Common header |
| 0x28 | 2 | u16 | Flags: bit 0 decal, bit 1 two-sided, bit 2 flash colour is negative, bit 3 tint mode 2, bit 4 unfiltered |
| 0x2A | 2 | | Padding |
| 0x2C | 32 | | Unused |
| 0x4C | 16 | ref | Meter map (`bitm`) |
| 0x5C | 32 | | Unused |
| 0x7C | 12 | rgb | Gradient minimum colour |
| 0x88 | 12 | rgb | Gradient maximum colour |
| 0x94 | 12 | rgb | Background colour |
| 0xA0 | 12 | rgb | Flash colour |
| 0xAC | 12 | rgb | Tint colour |
| 0xB8 | 4 | f32 | Meter transparency |
| 0xBC | 4 | f32 | Background transparency |
| 0xC0 | 24 | | Unused |
| 0xD8 | 2 | i16 | Meter brightness source |
| 0xDA | 2 | i16 | Flash brightness source |
| 0xDC | 2 | i16 | Value source |
| 0xDE | 2 | i16 | Gradient source |
| 0xE0 | 2 | i16 | Flash-extension source |
| 0xE2 | 2 | | Padding |
| 0xE4 | 32 | | Unused |

Meters draw bars on weapon and vehicle models (ammunition, heat).
Inputs from the sources (animation sources: 1 to 4 read the owning
object's stored outgoing function A to D as is; 0 or no object gives 1):
brightness β, flash brightness φ, value x, gradient g, flash extension
e. The disc's weapon meters are in `weapons.md` section 18. With the meter map texel
Mt, its blue channel is the position along the bar:

- gradient colour G = lerp(minimum, maximum, clamp(Mt.b /
  max(g, ⅛)))
- flash weight w = clamp(1 − 2·clamp(4·(e − Mt.b))) (1 beyond the
  flash extension, with a ⅛-wide edge); colour = G + w·φ·flash colour
  (minus with "flash colour is negative").
- Texels with Mt.b ≥ x show the background instead: colour =
  background, alpha = background transparency in tint mode 2, else 0;
  texels below x have alpha = meter transparency (tint mode 2) or β.
- colour × Mt.a; texels with Mt.a = 0 are discarded.
- Blend: tint mode 2: frame = colour × (β·tint colour) + frame × alpha;
  otherwise frame = colour × β + frame × tint colour.

Minimum: base = meter map, add blend, unlit, colour the bar by the
gradient minimum colour, ignore the value (or cut by value with a mask).

## Plasma (`spla`, 0x14C bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x000 | 40 | | Common header |
| 0x028 | 4 | | Unused |
| 0x02C | 2 | i16 | Intensity source (animation source) |
| 0x02E | 2 | | Padding |
| 0x030 | 4 | f32 | Intensity exponent |
| 0x034 | 2 | i16 | Offset source |
| 0x036 | 2 | | Padding |
| 0x038 | 4 | f32 | Offset amount, world units |
| 0x03C | 4 | f32 | Offset exponent |
| 0x040 | 32 | | Unused |
| 0x060 | 4 | f32 | Perpendicular brightness (alpha) |
| 0x064 | 12 | rgb | Perpendicular tint colour |
| 0x070 | 4 | f32 | Parallel brightness |
| 0x074 | 12 | rgb | Parallel tint colour |
| 0x080 | 2 | i16 | Tint colour source (colour) |
| 0x082 | 58 | | Unused |
| 0x0BC | 4 | | Unknown, 0x00000600 in every disc tag, not read |
| 0x0C0 | 4 | f32 | Primary noise map animation period, seconds |
| 0x0C4 | 12 | vector | Primary noise map animation direction |
| 0x0D0 | 4 | f32 | Primary noise map scale |
| 0x0D4 | 16 | ref | Primary noise map (`bitm`, 3D) |
| 0x0E4 | 32 | | Unused |
| 0x104 | 4 | | Unknown, 0x00000600 in every disc tag, not read |
| 0x108 | 4 | f32 | Secondary noise map animation period |
| 0x10C | 12 | vector | Secondary noise map animation direction |
| 0x118 | 4 | f32 | Secondary noise map scale |
| 0x11C | 16 | ref | Secondary noise map (`bitm`, 3D) |
| 0x12C | 32 | | Unused |

Plasma is the shield-hit and energy effect drawn over a model as a
modifier (`objects.md`), one tag per map (`characters\cyborg\shaders\
shield hit`).

- Intensity I = (source value)^(intensity exponent), 1 without a source;
  offset o = (source value)^(offset exponent) × offset amount (vertices
  pushed out along their normals by o; treated as 0 below 0.0005).
- Tint τ = the tint source colour or white.
- 3D noise coordinates: n₁ = position × primary scale + (t / primary
  period) × primary direction; n₂ likewise (position in model space,
  inferred).
- Per vertex (inferred): colour = τ·lerp(parallel, perpendicular,
  facing), alpha = I·lerp(parallel brightness, perpendicular
  brightness, facing), with facing the |normal·eye| term.
- Per pixel with noise values a = noise₁, b = noise₂: d = a − b; m₁ =
  (1 − 2|d|)²; m₂ = (2·m₁ − 1)² if m₁ ≥ ½ else 0; colour = (m₁ ×
  vertex colour + ½·m₂) × m₁²; alpha = vertex alpha × (second vertex
  alpha factor, inferred fog).
- Blend src·a + dst (additive), no culling, depth test, no depth write.

Minimum: base = primary noise map (as 2D if needed), add blend, tint =
perpendicular colour × perpendicular brightness, unlit, double-sided.

## Water (`swat`, 0x140 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x000 | 40 | | Common header |
| 0x028 | 2 | u16 | Flags: bit 0 base map alpha modulates reflection, bit 1 base map colour modulates background, bit 2 atmospheric fog, bit 3 draw before fog |
| 0x02A | 2 | | Padding |
| 0x02C | 32 | | Unused |
| 0x04C | 16 | ref | Base map (`bitm`) |
| 0x05C | 16 | | Unused |
| 0x06C | 16 | argb | View perpendicular tint colour |
| 0x07C | 16 | argb | View parallel tint colour |
| 0x08C | 16 | | Unused |
| 0x09C | 16 | ref | Reflection map (`bitm`, cube) |
| 0x0AC | 16 | | Unused |
| 0x0BC | 4 | f32 | Ripple animation angle, radians |
| 0x0C0 | 4 | f32 | Ripple animation velocity, texture units per second |
| 0x0C4 | 4 | f32 | Ripple scale (× uv) |
| 0x0C8 | 16 | ref | Ripple maps (`bitm`, several images) |
| 0x0D8 | 2 | i16 | Ripple mipmap levels (1..4 used) |
| 0x0DA | 2 | | Padding |
| 0x0DC | 4 | f32 | Ripple mipmap fade factor |
| 0x0E0 | 4 | f32 | Ripple mipmap detail bias |
| 0x0E4 | 64 | | Unused |
| 0x124 | 12 | block | Ripples (0x4C bytes each, 4 used) |
| 0x130 | 16 | | Unused |

Ripple element (0x4C bytes): 0x00 4 unused; 0x04 f32 contribution
factor; 0x08 32 unused; 0x28 f32 animation angle (radians); 0x2C f32
animation velocity; 0x30 2 × f32 map offset (u, v); 0x38 i16 map
repeats; 0x3A i16 map index (image of the ripple maps); 0x3C 16
unused.

Drawing:

- **Ripple texture**, built once per frame for the first water drawn,
  for each of the ripple mipmap levels (up to 4): four layers, layer i
  = image (map index) of the ripple maps at coordinate uv × repeats +
  t × velocity × (cos angle, sin angle) + offset, expanded to a normal
  2x − 1. Missing layers have contribution 0 and repeats 1; if the
  first two contributions are both 0 the second is set to 1, likewise
  for the last two. With c0..c3 the contributions:
  - pair A = N₀·(c₁/(c₀+c₁)) + N₁·(c₀/(c₀+c₁)) (quirk: the weights
    are swapped, each layer gets the other's share);
  - pair B likewise with c₂, c₃;
  - with Σ = c₀ + c₁ + c₂ + c₃: N = A·(c₂+c₃)/Σ + B·(c₀+c₁)/Σ (also
    swapped);
  - stored texel = ½·N + ½, then for mip level j of n levels (n > 1)
    blended towards the flat normal (½, ½, 1): texel·(1 − φ) + flat·φ
    with φ = j/(n − 1) × fade factor; with one level no fade.
- **Opacity** (flag bit 0): frame alpha ← base.a × lerp(parallel alpha,
  perpendicular alpha, c) with c a per-pixel facing cosine.
- **Background modulation** (flag bit 1): frame ← frame × base.rgb
  (towards white by the fog when flag bit 2).
- **Reflection**: the ripple texture, scaled by the ripple scale and
  scrolled by t × ripple velocity × (cos, sin) of the ripple angle, with
  mip bias −(detail bias), bumps a cube-map reflection; K = cube texel;
  colour = K⁸·(1 − tint) + K·tint with tint = lerp(parallel rgb,
  perpendicular rgb, clamp(−camera forward · surface plane normal))
  (one value per surface, white if the plane is unknown); added to the
  frame, weighted by the frame alpha when flag bit 0 is set.
- No culling; depth write unless "draw before fog" (then a depth-only
  pre-pass is drawn before fog and the colour later).
- Disc: two tags (beavercreek, damnation), on sky geometry
  (`sky\dusk\shaders\water dusk` on damnation: 4 ripples of
  contribution 0.3, scale 220, angle 4.712, velocity 0.03, 4 mip
  levels, fade 0.95, bias 0.6).

Minimum: base = reflection cube's average colour or the base map, blend
by base alpha, unlit, double-sided.

## Fog and lighting inputs from the level

The numbers come from the scenario's fog and sky (`scenario.md`) and
the lightmaps (`structure_bsp.md`).

### Atmospheric fog

- Fog depth is measured along the camera's forward axis (planar depth,
  not radial distance): z = (P − camera)·forward.
- t = clamp((z − minimum distance)/(maximum distance − minimum
  distance), 0, 1).
- The density lookup is the globals' `rasterizer\atmospheric fog
  density` bitmap: 16×16, a linear ramp in u (texel i = 17i/255) and
  constant in v, sampled bilinearly with clamping. With the texture at
  coordinate t this gives r(t) = clamp((16t − ½)/15); a renderer may use
  r(t) = t (error under 0.034).
- f = maximum density × r(t). A maximum density of 0 or less means 1.
  If the maximum distance is 0 (or atmospheric fog is off) there is no
  atmospheric fog.
- Level geometry: a separate pass after the opaque passes:
  frame = frame·(1 − f) + atmospheric colour·f (premultiplied; with
  planar fog the two are composited, below).
- Models: f is evaluated once per object at its centroid depth (model
  section), not per pixel; shaders with "true atmospheric fog" use
  per-vertex fog instead.
- Transparents: faded towards their blend's neutral value (fade stage),
  not towards the fog colour.

### Planar fog (brief)

- A fog volume below a plane (the BSP's fog planes, `structure_bsp.md`
  and `scenario.md`) with a colour, maximum density, maximum depth
  (below the plane) and maximum distance.
- The density comes from `rasterizer\planar fog density` (16×16, an
  s-shaped curve in u and a ramp in v) looked up with (depth below the
  plane / maximum depth, view distance / maximum distance) and scaled by
  the maximum density. The eye's own depth under the plane mixes in.
- Level pass, with fa atmospheric and fp planar fraction and e the
  camera's height above the plane / atmospheric maximum distance: out
  colour = atmospheric colour·fa·(1 − (1 − e)·fp) + planar colour·fp·
  (1 − fa·e); out alpha = 1 − (1 − fa)(1 − fp); frame = out + frame·(1
  − alpha).
- Blood Gulch's BSP has no fog planes (`structure_bsp.md`); whether
  any multiplayer map uses planar fog was not surveyed. A first renderer
  can skip it.

### Lightmap colour and gamma

- The lightmap texel (16-bit colour pages, `structure_bsp.md` and
  `bitmaps.md`) multiplies the textures directly: no scale, no gamma
  conversion, no overbright. Lighting above 1 cannot be represented.
- Material colour multiplies the lightmap (white on 177 of 180
  environment tags).
- The display's gamma is the TV's; no software gamma ramp is part of
  the shading.

### Vertex colours

- Level geometry has no vertex colour. Its lightmap vertex carries the
  incident light direction used for bump shading and specular.
- Models and transparents get computed per-vertex values (lighting,
  reflection tint, fog, fade). Transparent level geometry with
  "transparent lit" gets the material's lighting block
  (`structure_bsp.md`).

## Draw order and state

One view (window) of a frame, in order:

1. The sky (`scenario.md`; its geometry uses these shader classes and
   is flagged "sky" for fog purposes).
2. Opaque objects (model shaders and environment shaders on objects),
   depth test less-or-equal and depth write; transparent parts and
   effects are queued. First-person models use their own depth range.
3. Level lightmap pass (depth write), shadows, dynamic diffuse lights.
4. Decals: light layer, alpha-tested layer.
5. Level diffuse texture pass.
6. Decals: primary and secondary layers.
7. Dynamic specular, specular lightmap, reflection lightmap mask,
   mirrors, reflections.
8. Transparent level geometry is queued (decal-flagged ones drawn now).
9. Level fog pass.
10. Weather, particles, particle systems, contrails (queued as
    transparent groups).
11. Queued transparents, part one; water decals; detail objects;
    transparents, part two; fog screens; lens flares; then the HUD.

Queued transparents are sorted:

- "Draw before water" shaders first, then water, then everything else
  back to front by the centroid's depth along the camera forward axis
  (farthest first), ties by source object; first-person geometry
  last.
- Each queued group is drawn with its extra layers first (recursively,
  4 deep at most), then itself.
- Transparents test depth less-or-equal and never write it.
- Decal-flagged transparents (and alpha-blended model decals) use z-bias
  8 and are not sorted or fogged.

Per-class state summary:

| Class | Depth write | Sorted | Cull | Alpha test | Blend |
|---|---|---|---|---|---|
| senv (level) | lightmap pass only | no | back | bump alpha > 127 if flagged | passes as above |
| soso | yes (not alpha-blended decal) | no | back | base alpha > 127 unless flagged | none / alpha blend for decals |
| schi, sotr | no | yes (decals no) | back, or none if two-sided | > 127 if flagged | blend function |
| sgla | no | yes | per flag | diffuse layer > 0 | multiply, add, alpha blend layers |
| smet | no | yes, unless decal | per flag | texels with alpha 0 discarded | constant-colour blends |
| spla | no | yes | none | no | additive by alpha |
| swat | yes unless draw before fog | before all but "draw before water" | none | no | multiply, add |

## Measured facts

Checked on the disc's 13 multiplayer maps (each map's copy of a shared
tag counted in that map).

### Shader tags per class

| Map | senv | soso | sotr | schi | sgla | smet | spla | swat | Total |
|---|---|---|---|---|---|---|---|---|---|
| beavercreek | 15 | 98 | 21 | 19 | 6 | 7 | 1 | 1 | 168 |
| bloodgulch | 14 | 106 | 26 | 20 | 6 | 7 | 1 | 0 | 180 |
| boardingaction | 14 | 90 | 19 | 19 | 6 | 7 | 1 | 0 | 156 |
| carousel | 12 | 90 | 25 | 19 | 7 | 7 | 1 | 0 | 161 |
| chillout | 18 | 89 | 19 | 18 | 7 | 7 | 1 | 0 | 159 |
| damnation | 17 | 92 | 21 | 23 | 6 | 7 | 1 | 1 | 168 |
| hangemhigh | 10 | 89 | 17 | 19 | 6 | 7 | 1 | 0 | 149 |
| longest | 13 | 89 | 20 | 18 | 6 | 7 | 1 | 0 | 154 |
| prisoner | 11 | 89 | 22 | 19 | 7 | 7 | 1 | 0 | 156 |
| putput | 15 | 89 | 14 | 17 | 6 | 7 | 1 | 0 | 149 |
| ratrace | 16 | 91 | 21 | 17 | 6 | 7 | 1 | 0 | 159 |
| sidewinder | 15 | 93 | 23 | 20 | 6 | 7 | 1 | 0 | 165 |
| wizard | 10 | 91 | 19 | 19 | 6 | 7 | 1 | 0 | 153 |
| **all** | 180 | 1196 | 267 | 247 | 81 | 91 | 13 | 2 | 2077 |

No `scex` and no bare `shdr` tags.

### Environment shaders (180)

| Property | Histogram |
|---|---|
| type | normal 176, blended 4, blended base specular 0 |
| flags | none 151, alpha tested 29; bump-is-specular-mask 0; true atmospheric fog 0 |
| detail function | double biased multiply 180 |
| micro detail function | double biased multiply 180 |
| diffuse flags | none 174, rescale detail maps 6 |
| base map | present 179 |
| primary detail map | present 16 (scales 4..100) |
| secondary detail map | present 11 |
| micro detail map | present 118 (scales 4..60, mostly 16) |
| bump map | present 144 (scale 1 on 167 tags; 3..55 otherwise) |
| self-illumination map | present 22 |
| reflection cube map | present 157; reflection type bumped cube map 180 |
| lens flare | present 2 |
| material colour | white 177; grey 0.424, grey 0.475, red (1, 0, 0) one each |
| specular flags | none 179, lightmap is specular 1 |
| specular brightness | 1.0 104, 0.5 23, 0.4 16, 0 30, other 7 |
| reflection lightmap brightness scale | 0 157, 0.25 11, 0.125 8, 0.5 3, 0.1 1 |
| perpendicular / parallel brightness | most common (0.35, 0.25) 58, (0.45, 0.35) 28, (0, 0) 24 |
| u/v animation | function one on all; v scale −1 on 8, else 1 |
| self-illumination animation | primary: one 176, cosine-variable 1; plasma: slide 13, jitter 2, one 165 |

### Transparent shaders (514)

| Blend function | chicago | generic | all |
|---|---|---|---|
| 0 alpha blend | 82 | 97 | 179 |
| 3 add | 165 | 170 | 335 |
| others | 0 | 0 | 0 |

- Chicago: map counts 1:187, 2:21, 3:24, 4:15; colour functions used
  between maps: add 87, multiply 25, double multiply 2; alpha
  functions: blend next map alpha 65, current 20, add 14, multiply 13,
  double multiply 2. First map type 2D on all. Fade mode none 243,
  parallel 4. Extra flags: don't fade camouflage 78.
- Generic: map counts 1:139, 2:52, 4:76; stage counts 0:128, 1:11,
  2:39, 4:13, 7:76; fade mode none 253, perpendicular 1, parallel 13.
- No extra layers on any transparent shader.

### Other classes

- Model: multipurpose map 1046 of 1196, detail map 216, reflection cube
  818; change colour source none 1139, A 18, C 39; detail masks none
  1142, reflection inverse 26, reflection 26, change colour 2; detail
  functions double biased multiply 1170, double biased add 26;
  translucency non-zero on 5.
- Glass: reflection type bumped 13, flat 68; two-sided 39.
- Meter: flags decal + tint mode 2 (39), decal + unfiltered (26), none
  (26).

### Shaders on Blood Gulch and Hang 'Em High level geometry

Material count in brackets (`structure_bsp.md` materials).

| Map | Class | Shader | Base map (map 0 for transparents) |
|---|---|---|---|
| bloodgulch | senv | `levels\test\bloodgulch\shaders\cap_cliff01b` [7] | `levels\test\bloodgulch\bitmaps\blood cliff rock` |
| bloodgulch | senv | `levels\test\beavercreek\shaders\beavercreek boulder` [1] | `levels\test\beavercreek\bitmaps\beavercreek boulder` |
| bloodgulch | senv | `levels\test\bloodgulch\shaders\blood ground` [1] | `levels\test\bloodgulch\bitmaps\blood ground` |
| bloodgulch | senv | `levels\b30\shaders\metal flat generic` [8] | `levels\b30\bitmaps\metal flat generic` |
| bloodgulch | senv | `levels\b30\shaders\metal panels generic ext clean` [2] | `levels\b30\bitmaps\metal panels generic ext clean` |
| bloodgulch | senv | `levels\b30\shaders\metal panels generic ext` [4] | `levels\b30\bitmaps\metal panels generic ext` |
| bloodgulch | senv | `levels\test\bloodgulch\shaders\cap_metal` [4] | `levels\test\bloodgulch\bitmaps\cap_metal` |
| bloodgulch | senv | `levels\b30\shaders\metal panels ext unearthed` [1] | `levels\b30\bitmaps\metal panels ext unearthed` |
| bloodgulch | senv | `levels\test\bloodgulch\shaders\cap_ramp` [5] | `levels\test\bloodgulch\bitmaps\cap_ramp01` |
| bloodgulch | senv | `levels\test\bloodgulch\shaders\cap_ramp_blue` [2] | `levels\test\bloodgulch\bitmaps\cap_ramp02` |
| bloodgulch | sotr | `levels\test\bloodgulch\shaders\cap_moss01b` [1] | `levels\test\bloodgulch\bitmaps\cap_moss01b` |
| bloodgulch | sotr | `scenery\teleporter_shield\shaders\teleporter_shield` [2] | `levels\a50\devices\prison door\bitmaps\energy plasma` |
| bloodgulch | sotr | `levels\test\bloodgulch\shaders\greenlight` [1] | `levels\c10\bitmaps\c10_inv` |
| bloodgulch | sotr | `levels\test\bloodgulch\shaders\bloodgulch light red` [8] | `levels\test\bloodgulch\bitmaps\mp lights small strips red` |
| bloodgulch | sotr | `levels\test\bloodgulch\shaders\bloodgulch light yellow` [12] | `levels\b30\bitmaps\lights small strips` |
| bloodgulch | sotr | `levels\test\bloodgulch\shaders\bloodgulch light blue` [8] | `levels\b30\bitmaps\lights small strips` |
| bloodgulch | sotr | `levels\test\bloodgulch\shaders\bloodgulch light black` [12] | `levels\test\prisoner\bitmaps\black light` |
| hangemhigh | senv | `levels\b30\shaders\metal plate floor` [1] | `levels\b30\bitmaps\metal plate floor` |
| hangemhigh | senv | `levels\b30\shaders\metal flat generic` [2] | `levels\b30\bitmaps\metal flat generic` |
| hangemhigh | senv | `levels\b30\shaders\metal strips wide` [1] | `levels\b30\bitmaps\metal strips wide` |
| hangemhigh | senv | `levels\b30\shaders\metal panels generic` [1] | `levels\b30\bitmaps\metal panels generic` |
| hangemhigh | senv | `levels\b30\shaders\metal strips wide a` [1] | `levels\b30\bitmaps\metal strips wide 1` |
| hangemhigh | senv | `levels\b30\shaders\decorative wide circle` [1] | `levels\b30\bitmaps\metal strips decorative circle` |
| hangemhigh | senv | `levels\b30\shaders\metal strips narrow` [1] | `levels\b30\bitmaps\metal strips narrow` |
| hangemhigh | senv | `levels\b30\shaders\metal strips decorative wide` [1] | `levels\b30\bitmaps\metal strips decorative wide` |
| hangemhigh | senv | `levels\b30\shaders\metal panels generator` [1] | `levels\b30\bitmaps\metal panels generator` |
| hangemhigh | sotr | `levels\test\hangemhigh\shaders\hangemhigh blue light` [176] | `levels\b30\bitmaps\lights small strips` |

### Disc checks performed

A script parsed every shader tag of every class in all 13 multiplayer
maps with the layouts above and checked:

- Every tag reference read as a map (base, detail, micro, bump,
  self-illumination, cube, multipurpose, chicago/generic/glass/water/
  meter/plasma maps) is none or resolves to a tag of class `bitm`, and
  the reference's own class field says `bitm`; lens flare references
  resolve to `lens`; extra layers to shader tags. **0 failures.**
- Enumerations within the ranges listed: shader type (per class as in
  the class table), detail level 0..3, material type 0..32, environment
  type 0..2, detail and micro functions 0..2, reflection type 0..2,
  periodic functions 0..11 (environment u/v and self-illumination, model
  self-illumination, every texture animation block, generic constant
  animation), animation sources 0..4, model detail mask 0..8, first map
  type 0..3, blend function 0..7, fade mode 0..2, chicago colour and
  alpha functions 0..12, generic inputs 0..24, mappings 0..7, outputs
  0..8, output functions 0..1, output mappings 0..5, glass reflection
  type 0..2. **0 failures.**
- Colours within 0..1: header emitted and tint colours, environment
  self-illumination on/off colours, specular colours, model
  self-illumination bounds and reflection tints, generic stage
  constants, glass and water argb colours; environment specular and
  reflection brightnesses and lightmap brightness scale within 0..1.
  **0 failures.**
- Scales non-negative: every environment detail, micro, bump and
  self-illumination scale (all positive in fact). **0 failures.**
- A byte-coverage pass recorded which bytes are ever non-zero per
  class; every such byte lies inside a named field above except the
  documented leftovers (model 0x184..0x197, plasma 0x0BC and 0x104).

## Test cases

### (a) Synthetic: environment shader parse

A 0x344-byte buffer, all zero except (little-endian):

| Offset | Bytes / value | Expected field |
|---|---|---|
| 0x00 | u16 1 | radiosity flags = simple parameterization |
| 0x02 | i16 2 | detail level low |
| 0x22 | i16 2 | material type stone |
| 0x24 | i16 3 | type environment |
| 0x28 | u16 1 | flags: alpha tested only |
| 0x2A | i16 1 | type blended |
| 0x88 | ref (`bitm`, id of tag X) | base map = X |
| 0xB0 | i16 0 | detail function double biased multiply |
| 0xB4 | f32 8.0 | primary detail scale 8 |
| 0xB8 | ref (`bitm`, Y) | primary detail = Y |
| 0xC8 | f32 60.0 | secondary scale 60 |
| 0xCC | ref none (id 0xFFFFFFFF) | secondary detail none |
| 0xF8 | f32 16.0 | micro scale 16 |
| 0x10C | f32 1, 1, 1 | material colour white |
| 0x124 | f32 40.0 | bump scale 40 |
| 0x138 | f32 40.0, 40.0 | runtime bump scale (40, 40) |
| 0x150 | i16 0; 0x154 f32 1.0; 0x158 f32 1.0 | u animation one, period 1, scale 1 |
| 0x15C | i16 6; 0x160 f32 2.0; 0x164 f32 0.5 | v animation slide, period 2, scale 0.5 |
| 0x19C | f32 1, 0.5, 0 | primary on colour |
| 0x1B4 | i16 2; 0x1B8 f32 1.0 | primary function cosine, period 1 |
| 0x27C | u16 2 | specular flags: extra shiny |
| 0x290 | f32 0.5 | specular brightness 0.5 |
| 0x2D2 | i16 1 | reflection type flat cube map |
| 0x2D4 | f32 0.25 | lightmap brightness scale 0.25 |
| 0x2F4 | f32 0.35; 0x2F8 f32 0.25 | perpendicular 0.35, parallel 0.25 |

Expected: every listed field as stated, every other field zero/none;
the animation offset at t = 1 s is Δu = 1.0, Δv = 0.5 ×
periodic(slide, 0.5) = 0.5 × 0.3475 = 0.1737.

### (a) Synthetic: transparent chicago parse

A 0x6C-byte main buffer plus two 0xDC-byte map elements:

| Where | Offset | Value | Expected |
|---|---|---|---|
| main | 0x24 | i16 6 | type chicago |
| main | 0x28 | u8 0 | numeric limit 0 |
| main | 0x29 | u8 0x05 | flags alpha-tested and two-sided |
| main | 0x2A | i16 0 | first map 2D |
| main | 0x2C | i16 3 | blend add |
| main | 0x2E | i16 2 | fade when parallel |
| main | 0x30 | i16 1 | fade source A |
| main | 0x48 | block count 0 | no extra layers |
| main | 0x54 | block count 2 | two maps |
| main | 0x60 | u32 1 | don't fade active camouflage |
| map 0 | 0x00 | u16 0x0C | u- and v-clamped |
| map 0 | 0x2C | i16 2 | colour function multiply |
| map 0 | 0x2E | i16 11 | alpha function blend next map alpha |
| map 0 | 0x54, 0x58 | f32 2.0, 1.0 | u scale 2, v scale 1 |
| map 0 | 0x64 | f32 90.0 | rotation 90° |
| map 0 | 0x6C | ref (`bitm`, Z) | map Z |
| map 0 | 0xA4 + 0x2C | f32 360.0 | rotation animation scale 360 (function one) |
| map 1 | 0x2C, 0x2E | i16 0, 0 | functions unused (last map) |
| map 1 | 0x54, 0x58 | f32 1.0, 1.0 | scale 1 |

Expected texture transform for map 0 (t = 0, sources none, rotation
animation one × 360 → θ = 360 + 90 = 450°, c = 0, s = 1 within float
error): (u, v) = (0.25, 0.5) maps to (−0.5, 0.5).

### Combine math (tolerance 2/255)

| Case | Inputs | Expected |
|---|---|---|
| env normal, dbm | base (0.6, 0.4, 0.2, a 1), primary 0.5, secondary a = 1, micro 0.5, lightmap (0.8, 0.6, 0.4), shade 1, no SI | (0.48, 0.24, 0.08) |
| env saturation | base 0.9, primary 0.8, secondary a = 1, micro 0.5, lightmap 0.5 | 0.5 (2·0.9·0.8 saturates to 1; not 0.72) |
| env blended | base (0.6, 0.4, 0.2, a 0.25), primary 0.8, secondary 0.4, micro 0.5, lightmap 1 | detail 0.5, result (0.6, 0.4, 0.2) |
| detail multiply | base 0.6, detail 0.5 | 0.3 |
| detail biased add | base 0.6, detail 0.75 | 1.0 (0.6 + 0.5 clamps) |
| self-illumination | lightmap 0.2, SI map (1, 0, 0, 0), primary on (1, 0.5, 0), function one, diffuse D (0.6, 0.4, 0.2) | L = (1, 0.7, 0.2); colour (0.6, 0.28, 0.04) |
| plasma band | d = 0 / 0.05 / 0.1 | m₃ = 1 / 0.0975 / 0 |
| reflection mask | lightmap (0, 0, 0), scale 0.25 | M = 0.25 |
| reflection mask | lightmap (1, 1, 1), scale 0.25 | M = 1 |
| env reflection tint | K = 0.5, tint 1 / 0, β 0.4, mask 1 | R = 0.2 / 0.5⁸ × 0.4 = 0.00156 |
| atmospheric fog | min 10, max 110, density 0.8, depth 60, frame (1, 0, 0), fog colour (0, 0, 1) | f = 0.4, (0.6, 0, 0.4) |
| chicago add | map 0 (0.2, 0.4, 0.6, a 0.5), map 1 (0.5, 0.5, 0.5, a 1), colour add, alpha multiply, blend add, k = 1, frame 0.1 | C = (0.7, 0.9, 1.0), a 0.5; frame (0.8, 1.0, 1.0) |
| double multiply blend | src 0.75, frame 0.4 | 0.6 |
| subtract blend | src 0.3, frame 0.5 | 0.2 |
| fade, multiply | out 0.2, k 0.25 | 0.8 |
| fade, double multiply | out 1.0, k 0.5 | 0.75 |
| water ripple weights | c = (1, 3, 0, 0) → after defaults (1, 3, 0, 1); N₀ = (1, 0, 0), N₁ = (0, 1, 0), N₂ = N₃ = (0, 0, 0) | A = (0.75, 0.25, 0), B = 0, Σ = 5, N = A/5 = (0.15, 0.05, 0), texel (0.575, 0.525, 0.5) |

### Periodic functions (input s in periods; tolerance 1/255)

| Function | s = 0 | 0.1 | 0.25 | 0.5 | 1.0 | 1.25 | 10 |
|---|---|---|---|---|---|---|---|
| one | 1 | 1 | 1 | 1 | 1 | 1 | 1 |
| zero | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| cosine | 1.0000 | 0.9489 | 0.7255 | 0.2063 | 0.3435 | 0.8510 | 0.9961 |
| diagonal wave | 0.0000 | 0.1366 | 0.3475 | 0.6988 | 0.5984 | 0.2471 | 0.0000 |
| slide | 0.0000 | 0.0664 | 0.1718 | 0.3475 | 0.6988 | 0.8745 | 0.9961 |
| jitter, wander | 0.6471 | 0.5264 | 0.5357 | 0.2000 | 0.5561 | 0.5804 | 0.5098 |

- Cosine at s = 1/0.7 = 1.428571 is 0.9961 (a full cycle) and at
  s = 0.714286 is 0.0 (the trough): the 0.7 quirk.
- Exact tables (implementations that rebuild them): the first six bytes
  of the cosine table are 255, 253, 247, 238, 226, 210; slide 0, 6, 13,
  20, 27, 34; diagonal wave 0, 13, 27, 41, 55, 69; jitter 165, 163, 146,
  125, 116, 123; noise 220, 20, 6, 250, 75, 185 (depends only on the
  generator sequence: the noise values are the 1024 values drawn after
  nine stretched-x tables, 36864 + 4096 values in).
- The variable-period and spark tables depend on 32-bit float summation
  order; test them with tolerance 2/255: spark at s = 0.5 is 0.3608,
  cosine-variable at s = 0.25 is 0.2651, slide-variable at s = 0.5 is
  0.5914.

### Numeric shaders and image choice

| Setup | Input | Expected image index |
|---|---|---|
| numeric chicago, limit 60, map 0 with 10 images, object functions A = 32/60 = 0.53333336, D = 0.9 | part permutation 0 / 1 / 2 | 2 / 3 / 0 (function A; 60 x A + 0.5 = 32.5) |
| same shader, map 0 with 8 images | permutation 0 | function D: floor(60 x 0.9 + 0.5) = 54, 54 mod 8 = 6 |
| limit 8, 8 images, D = 0.75 / 1.0 / 0.0 / 0.0624 / 0.0625 | permutation 0 | 6 / 0 / 0 / 0 / 1 |
| numeric flag set, drawn as level geometry (no object) | part permutation 3, 10 images | 3 (the default) |
| not numeric, 4-image bitmap | part permutation 6 | 2 (6 mod 4) |
| numeric chicago with the countdown timer flag, timer 83 456 ms | positions 0 to 5 | 6, 5, 4, 3, 2, 1 (ms units, hundredths, tenths, seconds, tens of seconds mod 6, minutes) |

More cases, on the disc's assault rifle, are in `weapons.md` (Test
cases, On-model displays).

### (b) Disc-backed (slow tests, `bloodgulch`)

| Tag | Expected |
|---|---|
| `levels\test\bloodgulch\shaders\blood ground` | class `senv`, header type 3, material type 1 (sand), detail level 2, radiosity flags 1; type 1 blended; flags 0; base `levels\test\bloodgulch\bitmaps\blood ground`; detail function 0; primary scale 100, `levels\b30\bitmaps\detail sand`; secondary scale 60, `levels\a30\bitmaps\detail grass`; micro function 0, scale 12, `levels\b30\bitmaps\detail ground`; no bump map (runtime bump (1, 1)); material colour white; v animation scale −1; specular brightness 0; no cube map |
| `levels\test\bloodgulch\shaders\cap_cliff01b` | class `senv`, material type 2 (stone), detail level 2; type 0 normal; flags 0; base `levels\test\bloodgulch\bitmaps\blood cliff rock`; primary scale 8, `levels\b30\bitmaps\detail cliff rock`; secondary none (scale 1); micro scale 60, `levels\b30\bitmaps\detail cliff rock smooth`; bump scale 40, `levels\a30\bitmaps\cliff rock bump`, runtime (40, 40); no cube map |
| `scenery\teleporter_shield\shaders\teleporter_shield` | class `sotr`, power 10, flags 4 (two-sided), blend 3 add, fade mode 2, 4 maps (map 0 and 1 `levels\a50\devices\prison door\bitmaps\energy plasma` with slide animations of periods 10/6 and 11/7 s), 7 stages; stage 0 flags 4, lower (0.4, 0, 0, 0.35), upper (1, 0, 0, 1) as argb |
| `levels\b30\shaders\metal flat generic` | class `senv`, material type 7, type 0, micro scale 16 `levels\b30\bitmaps\detail metal sratches`, bump `levels\b30\bitmaps\metal flat generic bump` scale 1, specular brightness 1, perpendicular colour (0.376, 0.482, 0.651), parallel (0.227, 0.427, 0.302), reflection perpendicular 0.45, parallel 0.35, cube `levels\b30\bitmaps\cube map forerunner interiors` |
| class counts | 14 senv, 106 soso, 26 sotr, 20 schi, 6 sgla, 7 smet, 1 spla, 0 swat |
| level geometry | the 17 shaders in the Blood Gulch table above, with those base maps and material counts |

## Confidence

| Section | Confidence |
|---|---|
| Class list, type numbers, common header | checked against the disc (all 13 maps) |
| Every tag layout except `scex` | field positions and ranges checked against the disc; meanings from the reference |
| `scex` layout | reference only (not on the disc) |
| Enumerations and their ranges | checked on the disc (ranges); names and meanings from the reference |
| Periodic tables and evaluation | reference only (constants are the binary's); numbers computed from that description |
| Environment pass order and combine math | reference only (decoded combiner programs); not checkable on the disc |
| Environment bump weight w, per-vertex vectors | inferred |
| Model pixel math, multipurpose channels | reference only (Xbox build) |
| Model vertex lighting, fresnel, translucency | inferred |
| Chicago and generic combine tables | reference only |
| Fade factors per mode | inferred (the reference shows which vertex value, not its formula) |
| Glass, meter, plasma, water math | reference only; per-vertex parts inferred |
| Numeric shaders, image choice, whose function values a drawn model gets | reference; the disc's numeric and meter tags, their bitmaps and the models' permutations checked on bloodgulch (`weapons.md` section 18) |
| Default textures, fog density ramps | checked on the disc (`rasterizer\...` bitmaps in bloodgulch) |
| Fog formulas | reference only |
| Draw order and sorting | reference only |
| Measured facts | checked against the disc |

## Open questions

- **Build differences.** The reference is an early Xbox development
  build; the disc is retail. Layouts match the disc everywhere checked,
  but the pixel math (the combiner programs) could have changed before
  release. Comparing captures of the original console (Blood Gulch
  ground, a bumped metal wall with a cube reflection, a teleporter
  shield) with a renderer built from this spec would settle it.
- **PC look versus Xbox look.** The later PC release documents a
  different multipurpose channel order and its own shader programs; the
  reference describes the Xbox. Where community knowledge of the PC look
  disagrees with this spec (multipurpose channels, the specular
  lightmap, fog), this spec follows the Xbox build.
- **Bump shading weight w.** The lightmap pass blends the bump term in
  by a per-vertex value from the lightmap vertex; the vertex program
  that computes it was not readable. The best guess is the length of
  the lightmap vertex's incident direction (how directional the baked
  light is). Check against `structure_bsp.md`'s lightmap vertex data
  (are the incident vectors shorter than 1?).
- **Detail coordinates and animation.** Whether the environment u/v
  animation offset is added before the detail scale (so detail maps
  scroll faster) is a vertex-program question. On the disc every offset
  is a whole number, so it never shows.
- **Periodic period quirk.** The 0.7-cycles-per-period behaviour comes
  from the binary's constants (25.6 entries per unit, 28 cycles in 1024
  entries). Watch a pulsing light on the console to confirm.
- **Fade factors.** Which per-vertex terms make up the transparent fade
  (fog, view angle for modes 1 and 2, fade source, camouflage) is
  inferred from the constants passed; the exact view-angle formula is
  unknown.
- **Model translucency and vertex lighting.** The model vertex program
  (ambient + 2 distant + 2 point lights, translucency, fresnel for the
  reflection tint) is inferred; the exact falloff and fresnel curves are
  unknown.
- **Two-sided models.** The model draw always culls back faces; whether
  two-sided model parts duplicate their triangles at build time is
  `models.md`'s to confirm.
- **Specular lightmap vectors.** Which per-vertex vectors feed the
  specular lightmap pass (eye and incident direction in tangent space)
  is inferred; only one disc shader uses the pass.
- **Water ripple weights** are swapped between the layers of each pair
  (decoded as written); visible only when contributions differ. Both
  disc water shaders use equal contributions, so it does not show.
- **First-map cube types** (chicago/generic types 1..3) are not used on
  the disc; their exact vectors are not described.
- **Glass flat reflection with a bump map** uses a dot product of the
  bump texel with a per-vertex vector for the fresnel cosine; quirk kept
  as decoded.
