# Halo: Combat Evolved: bitmaps

Source basis: the original Xbox disc's own map files (all 24 cache files
read with a throwaway reader: 13 multiplayer maps, the front-end map and
10 campaign maps, 14 815 bitmap images in 9 327 bitmap tags), the disc's
main executable (for the one built-in palette), plus a read of the
behaviour reference's bitmap tag definition, its bitmap size and address
rules and its Xbox texture handling. Where they differ the disc wins.
Every format listed below was decoded to a picture and looked at (see
"What was looked at").
Status: draft

## Summary

A bitmap tag holds a list of images ("bitmaps": 2D textures, volume
textures, cube maps, sprite pages, interface images), the animation-like
"sequences" that group them, and nothing else that a game needs at run
time. In a map file the pixels are not inside the tag: each bitmap gives
an absolute offset into the decompressed map image, where its mip chain
sits in exactly the form the Xbox GPU wants. That form is: block
compressed (DXT1, DXT3, DXT5) formats as plain linear rows of 4x4
blocks, and every other format Morton-swizzled (bit-interleaved), with
one exception, a linear interface image. A decoder needs only a few
rules: the format code, the swizzle rule, the stored-level rule, the cube
face layout and one 256-entry built-in palette for bump maps.

## Units and conventions

- Integers are little-endian. "u16", "u32" unsigned, "i16", "i32" signed,
  "f32" IEEE-754 single. Offsets in tables are bytes from the start of
  the structure named.
- Tag primitives (block, data reference, tag reference fields, how an
  address in tag data becomes a file position, how a tag's main
  structure is found) are defined in cache_file.md and are not repeated
  here. The "decompressed map image" is the whole file after the
  body's compression is undone, with the 0x800-byte header still at its
  start (the same offsets cache_file.md uses).
- Image coordinates: x to the right, y downwards. Row 0 is the first row
  stored and is the TOP of the picture at texture coordinate v = 0.
  Pixel (0,0) is the top-left. (Checked: HUD text and sky pictures are
  upright when decoded this way.)
- Colours are written (R, G, B, A), 8 bits each, 0..255.
- A "level" is one image of a mip chain. Level 0 is the full-size image.
  Level dimensions are max(1, W >> l) by max(1, H >> l) by
  max(1, D >> l).
- "Swizzled" means Morton order as defined in the Xbox storage section.
- All sizes and counts in the measured section are for the Xbox disc's
  files; the PC release is not covered.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| bitmap tag class | 'bitm' | | the tag class code |
| fixed part size | 0x6C | bytes | size of the bitmap tag's main structure |
| bitmap element size | 0x30 | bytes | one element of the "bitmaps" block |
| sequence element size | 0x40 | bytes | one element of the "sequences" block |
| sprite element size | 0x20 | bytes | one element of a sequence's "sprites" block |
| bitmap signature | bytes 6D 74 69 62 (u32 0x6269746D) | | first field of every bitmap element; equals the class code 'bitm' |
| pixel start alignment | 512 | bytes | every bitmap's pixel offset is a multiple of this |
| size rounding | 128 | bytes | pixel size is rounded up to a multiple of this; cube faces too |
| built-in palette entries | 256 | entries | 4 bytes each, used by format code 17 |
| cube faces | 6 | | order +X, -X, +Y, -Y, +Z, -Z |
| compressed block | 4 x 4 texels | | 8 bytes (DXT1) or 16 bytes (DXT3, DXT5) |
| largest dimension on disc | 2048 | texels | two bitmaps, 2048 x 128 |

## The bitmap tag: fixed part (0x6C bytes)

Confidence: checked against the disc (every field's range over all 9 327
tags; the data-reference fields hold the same values in every tag).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 2 | enum | type | what the tag's images are, see "Bitmap types" |
| 0x02 | 2 | enum | format | the author's format choice (an import hint, see below) |
| 0x04 | 2 | enum | usage | how mip levels were generated (build-time only) |
| 0x06 | 2 | flags | flags | import options, build-time only (see below) |
| 0x08 | 4 | f32 | detail fade factor | 0..1, build-time only |
| 0x0C | 4 | f32 | sharpen amount | 0..1, build-time only |
| 0x10 | 4 | f32 | bump height | in texture repeats, build-time only |
| 0x14 | 2 | enum | sprite budget size | 0..4 = 32, 64, 128, 256, 512 pixel pages (import only) |
| 0x16 | 2 | u16 | sprite budget count | pages allowed (import only) |
| 0x18 | 2 | i16 | color plate width | pixels, retained (the authoring image's size) |
| 0x1A | 2 | i16 | color plate height | pixels, retained |
| 0x1C | 20 | data ref | compressed color plate data | STRIPPED (see below) |
| 0x30 | 20 | data ref | processed pixel data | STRIPPED (see below) |
| 0x44 | 4 | f32 | blur filter size | pixels, 0..10, build-time only |
| 0x48 | 4 | f32 | alpha bias | -1..1, build-time only |
| 0x4C | 2 | i16 | mipmap count | the author's request; 0 means all levels; build-time only |
| 0x4E | 2 | enum | sprite usage | 0..2, background colour of sprite plates (import only) |
| 0x50 | 2 | i16 | sprite spacing | pixels between sprites on a page (import only) |
| 0x52 | 2 | pad | | always 0 on disc |
| 0x54 | 12 | block | sequences | elements of 0x40 bytes |
| 0x60 | 12 | block | bitmaps | elements of 0x30 bytes |

Notes on the fixed part.

- The two data references at 0x1C and 0x30 are stripped in every map. In
  every one of the 9 327 tags: flags 0, address 0, definition 0, so
  there is no data to read. The colour-plate reference keeps its
  original size and a position field of 0x6C (108); the pixel-data
  reference keeps size 0 and a position field equal to 108 plus the
  plate's size. These leftovers are authoring-time numbers and mean
  nothing; ignore both references and never try to read data through
  them. The real pixels are reached only through each bitmap element's
  own offset (next section).
- "color plate width/height" are left as the authoring image's size and
  are not the size of any stored bitmap.
- The 12-byte block fields follow cache_file.md. Their third dword (the
  definition pointer) is 0 on disc.
- Everything marked "build-time only" or "import only" has no effect on
  decoding or rendering: the maps contain the finished mip chains. A
  loader may keep the values for display and otherwise skip them.
- Observed values (14 files, 4 638 tags in the multiplayer maps and the
  front-end map): bump height is nonzero only on height-map-usage
  bitmaps (for example 0.004); blur filter size 2.0 occurs on some of
  those too; alpha bias, detail fade and sharpen are nearly always 0.

### Enumerations

type (u16 at 0x00; the tag-level type):

| Value | Meaning | Tags in the 14 files |
|---|---|---|
| 0 | 2D textures | 3 178 |
| 1 | 3D textures | 41 |
| 2 | cube maps | 231 |
| 3 | sprites | 1 174 |
| 4 | interface bitmaps | 14 |

format (u16 at 0x02; an import hint only; the per-bitmap format code
below is authoritative and the two do not map one to one):

| Value | Meaning |
|---|---|
| 0 | compressed with color-key transparency (DXT1 intent) |
| 1 | compressed with explicit alpha (DXT3 intent) |
| 2 | compressed with interpolated alpha (DXT5 intent) |
| 3 | 16-bit color |
| 4 | 32-bit color |
| 5 | monochrome |

usage (u16 at 0x04; how the importer built mip levels and whether it
treated the source as a height map):

| Value | Meaning | Tags in the 14 files |
|---|---|---|
| 0 | alpha-blend: mip averaging ignores zero-alpha pixels | 270 |
| 1 | default: ordinary down-sampling | 4 106 |
| 2 | height map: the source was a height map, converted to a bump map | 142 |
| 3 | detail map: mip colour fades toward grey, alpha toward white | 93 |
| 4 | light map: no mips | 13 |
| 5 | vector map: pixels are vectors, renormalised after down-sampling | 14 |

flags (u16 at 0x06), import options: bit 0 enable diffusion dithering,
bit 1 disable height map compression, bit 2 uniform sprite sequences,
bit 3 filthy sprite bug fix. Observed values are only 0, 2, 4 and 8
(bit 1 on height maps and a vector map group, bit 3 on some sprite
groups). They change only how the importer packed sprites; the stored
rectangles already include their effect.

sprite usage (u16 at 0x4E): 0 blend/add/subtract/max, 1 multiply/min,
2 double multiply. Import only.

### Sequences block (0x40-byte elements)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | char[32] | name | zero-padded text; often empty or like "import #3" |
| 0x20 | 2 | i16 | first bitmap index | index into the bitmaps block |
| 0x22 | 2 | i16 | bitmap count | number of bitmaps (frames) in the sequence |
| 0x24 | 16 | | unused | zeros |
| 0x34 | 12 | block | sprites | elements of 0x20 bytes |

Two disjoint kinds exist (checked on all 4 638 tags of the 14 files):

- In a tag whose type is not 3 (sprites): bitmap count is 1 or more,
  first bitmap index starts at 0 for the first sequence and each next
  sequence starts where the previous ended, and the sprites block is
  empty.
- In a tag of type 3 (sprites): first bitmap index and bitmap count are
  both 0 and the sprites block has one or more elements.

### Sprites block (0x20-byte elements)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 2 | i16 | bitmap index | which page (element of the bitmaps block) holds the sprite |
| 0x02 | 2 | | pad | |
| 0x04 | 4 | | unused | zeros |
| 0x08 | 4 | f32 | left | fraction of the page width, 0 at the page's left edge |
| 0x0C | 4 | f32 | right | fraction of the page width |
| 0x10 | 4 | f32 | top | fraction of the page height, 0 at the page's top edge |
| 0x14 | 4 | f32 | bottom | fraction of the page height |
| 0x18 | 4 | f32 | registration x | see below |
| 0x1C | 4 | f32 | registration y | see below |

The rectangle (left, right, top, bottom) is the sprite's texture
coordinate rectangle on its page; v grows downward, so top < bottom.
It already includes the importer's spacing border. The registration
point is the sprite's anchor, measured from the rectangle's top-left
corner and in the same page-fraction units (so a sprite is placed with
its anchor at the thing's position: the sprite's drawn extent runs from
-registration to rectangle size minus registration, each axis scaled by
the page's pixel size). In the reference, decals place their quad by
exactly this rule. All 965 sprites in bloodgulch.map have the page index
inside the bitmaps block and zero in the unused words.

### Bitmaps block (0x30-byte elements)

Confidence: checked against the disc for every field below (see the
measured section for the distributions).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | u32 | signature | bytes 6D 74 69 62 (the class code 'bitm') |
| 0x04 | 2 | i16 | width | pixels of level 0 |
| 0x06 | 2 | i16 | height | pixels of level 0 |
| 0x08 | 2 | i16 | depth | slices of level 0; 1 for 2D and cube maps |
| 0x0A | 2 | enum | type | 0 = 2D, 1 = 3D (volume), 2 = cube map |
| 0x0C | 2 | enum | format | pixel format code, see the format table |
| 0x0E | 2 | flags | flags | see below |
| 0x10 | 2 | i16 | registration x | pixels |
| 0x12 | 2 | i16 | registration y | pixels |
| 0x14 | 2 | i16 | mipmap count | number of levels AFTER level 0 |
| 0x16 | 2 | | unused | uninitialised junk: ignore (see below) |
| 0x18 | 4 | u32 | pixels offset | absolute offset of the pixel data in the decompressed map image |
| 0x1C | 4 | u32 | pixels size | bytes the pixel data occupies, including padding |
| 0x20 | 4 | u32 | owner tag id | the bitmap tag's own id (same as in the tag index) |
| 0x24 | 4 | i32 | (runtime) | always -1 (0xFFFFFFFF) |
| 0x28 | 4 | u32 | (runtime) | always 0 |
| 0x2C | 4 | u32 | (runtime) | a stale memory address, constant within a map (0x024F0040 or 0x02270040), different between maps: ignore |

Fields 0x20 to 0x2F are filled in by the game at run time and arrive as
stale values in the file. A loader should not interpret them.

Flags (u16 at 0x0E):

| Bit | Mask | Meaning | On disc |
|---|---|---|---|
| 0 | 0x01 | dimensions are all powers of two | always set |
| 1 | 0x02 | block compressed | set exactly on codes 14, 15, 16 |
| 2 | 0x04 | palettized | set exactly on code 17 |
| 3 | 0x08 | swizzled (Morton) | set on every format except 14, 15, 16 and the one linear case |
| 4 | 0x10 | linear (row-major, un-swizzled) | one bitmap (hud_health_base) |
| 5 | 0x20 | v16u16 (signed 2-channel) | never |
| 6 | 0x40 | allocated (run time) | never |
| 7 | 0x80 | cached (run time) | set on all 14 815 bitmaps; mask it out |

The values seen are 0x83 (compressed), 0x89 (swizzled), 0x8D (swizzled
palettized) and 0x91 (the linear one). A decoder should decide the
layout from bits 1, 3 and 4 (compressed, swizzled, linear) and the
format code; if neither bit 3 nor bit 4 nor bit 1 is set the texture
would be plain row-major.

The unused word at 0x16 holds 0x12 on about half the bitmaps, 0 on
about a third, 0xFFFF on about one in seven, and a few other values
(0x22, 0x10, 0x14, 0x21, 0x0156, 0x77EA, ...). It does not correlate with
anything and is not used.

Mipmap count: the number of levels after the first. A bitmap with count
n has n + 1 levels, and the largest legal count is floor(log2 of the
largest of width, height, depth). Verified from the pixel sizes of every
bitmap. Count 0 means a single level (all lightmap pages, interface
images, many sprite pages and decals).

## Where the pixel data lives

Confidence: checked against the disc (pictures decode from it).

- The element's pixels offset is an absolute byte offset into the
  decompressed map image (not an address in tag data, and not relative to
  anything in the tag). The data lies in the region of the file before
  the tag data. Nothing about it passes through address translation.
- The offset is a multiple of 512 on all 14 815 bitmaps. Between one
  bitmap's data and the next in file order the gap is 0, 128, 256 or 384
  bytes: the padding that rounds the next start up to a multiple of 512.
  No two bitmaps overlap or share an offset, even when their content is
  identical.
- Pixels size is the total of the stored levels (and, for a cube map, of
  all six faces) rounded up to a multiple of 128. It was checked on every
  bitmap of every map (rules in the Xbox storage section) and is a
  useful consistency check for a loader: after computing the expected
  size, a disagreement means the layout rules were misapplied.
- Each map carries its own copy of every bitmap it uses: the same tag
  path appears in many maps with its own pixel data. The 13 multiplayer
  maps and the front-end map hold 6 697 bitmap images in 4 638 tags, but
  only 1 050 distinct (path, index, format, size) images in 644 distinct
  tag paths. Nothing in a map refers to another map's pixels.
- In the front-end map the first pixel data starts at file offset 0x1000;
  in the multiplayer maps further in (bloodgulch.map: 0x13D000).

## Pixel formats

The format code is the u16 at 0x0C of the bitmap element. Texel size is
bits per texel; all multi-byte texels are little-endian. Expansion to
RGBA8 uses these helpers: expand5(v) = (v << 3) | (v >> 2) for 5-bit
values; expand6(v) = (v << 2) | (v >> 4) for 6-bit values; expand4(v) =
v * 17 for 4-bit values; a 1-bit alpha is 0 or 255.

| Code | Name | Bits | Storage | Layout of a texel | RGBA8 |
|---|---|---|---|---|---|
| 0 | a8 (alpha only) | 8 | swizzled | one byte A | (0, 0, 0, A) |
| 1 | y8 (intensity) | 8 | swizzled | one byte Y | (Y, Y, Y, 255) |
| 2 | ay8 (combined alpha-intensity) | 8 | swizzled | one byte V | (V, V, V, V) |
| 3 | a8y8 (separate alpha-intensity) | 16 | swizzled | byte 0 = Y, byte 1 = A | (Y, Y, Y, A) |
| 4, 5, 7, 12, 13 | unused | | | | invalid, never occur |
| 6 | r5g6b5 | 16 | swizzled | u16: R bits 15..11, G bits 10..5, B bits 4..0 | (expand5 R, expand6 G, expand5 B, 255) |
| 8 | a1r5g5b5 | 16 | swizzled | u16: A bit 15, R 14..10, G 9..5, B 4..0 | (expand5 R, expand5 G, expand5 B, A ? 255 : 0) |
| 9 | a4r4g4b4 | 16 | swizzled (or linear) | u16: A 15..12, R 11..8, G 7..4, B 3..0 | (expand4 R, expand4 G, expand4 B, expand4 A) |
| 10 | x8r8g8b8 | 32 | swizzled | bytes in file order B, G, R, X | (R, G, B, 255): the X byte is ignored |
| 11 | a8r8g8b8 | 32 | swizzled | bytes in file order B, G, R, A | (R, G, B, A) |
| 14 | DXT1 | 4 per texel | block, linear | 8-byte blocks | see "Block compression" |
| 15 | DXT3 | 8 per texel | block, linear | 16-byte blocks | see "Block compression" |
| 16 | DXT5 | 8 per texel | block, linear | 16-byte blocks | see "Block compression" |
| 17 | p8-bump | 8 | swizzled | one byte, an index into the built-in palette | the palette entry, see "The bump palette" |

Where the channel goes matters and differs between the one-byte and
two-byte monochrome formats:

- code 1 (y8): intensity goes to all three colour channels and alpha is
  fully opaque.
- code 2 (ay8): the one byte is BOTH the intensity of all three colour
  channels AND the alpha. HUD images and soft particles use it so that
  "dark" is also "transparent".
- code 3 (a8y8): two independent bytes, intensity first (low byte of the
  u16), alpha second (high byte). Checked on HUD pictures: the high byte
  traces the lettering and shapes.
- code 0 (a8): only alpha is stored. The reference's converter fills the
  colour with 0; see Open questions about what the Xbox hardware returned
  and what to use.

Format-by-format evidence:

- Codes 14, 15, 16, 11, 10, 6, 8, 3, 2, 1, 17 and 0 were decoded to
  pictures from bloodgulch.map and the front-end map and looked right
  (named in "What was looked at"). Code 9 was looked at on the linear
  interface image (hud_health_base) and on a swizzled one in the
  front-end map (the menu option backgrounds).
- The expansion formulas (replicating the top bits into the bottom bits)
  are what the reference's converter does; a hardware sampler may differ
  by one unit on some codes. Treat them as inferred for exactness and
  accept +-1 in comparisons against anything captured from a console.
- x8r8g8b8's unused byte is 0xFF in every one of the 857 984 texels of
  base levels in the 14 files, so ignoring it and honouring it agree.

### Block compression (codes 14, 15, 16)

Confidence: layouts checked on the disc by pictures and by the exact
agreement of stored sizes; the DXT rounding rules are the standard S3TC
ones (reference only).

All three operate on 4 x 4 texel blocks stored in row-major order of
blocks (block row 0 first, left to right). Inside a block, texels run in
row-major order, texel i being at column i mod 4 and row i div 4.
Level width and height not divisible by 4 are never met on disc (see the
measured section) but are handled by padding with partial blocks:
ceil(W/4) by ceil(H/4) blocks, the unused texels of an edge block simply
not output.

Colour part (8 bytes; the whole block for DXT1, the second half of a DXT3
or DXT5 block):

- bytes 0..1: colour 0 as u16 r5g6b5; bytes 2..3: colour 1 as u16
  r5g6b5; bytes 4..7: a u32 of sixteen 2-bit indices, texel 0 in bits
  1..0, texel 1 in bits 3..2, and so on up to texel 15 in bits 31..30.
- Expand both endpoints to 8 bits per channel with expand5/expand6.
  Write c0 and c1 for the expanded endpoints and compare the two u16
  values as unsigned integers.
- Four-colour mode (c0 value greater than c1 value, and ALWAYS for DXT3
  and DXT5 regardless of the order): index 0 is c0, index 1 is c1, index
  2 is floor((2*c0 + c1) / 3) per channel, index 3 is floor((c0 + 2*c1)
  / 3) per channel; alpha 255.
- Three-colour mode (DXT1 only, c0 value less than or equal to c1 value):
  index 0 is c0, index 1 is c1, index 2 is floor((c0 + c1) / 2) per
  channel (alpha 255), index 3 is transparent: (0, 0, 0, 0). This is the
  1-bit alpha ("color-key") rule: a DXT1 bitmap's alpha is 255 or 0 and
  every zero-alpha texel is black. It is used on the disc (33 DXT1 bitmaps
  in bloodgulch.map have such texels, for example the energy shield
  sparks, blood splats and muzzle flashes; 32 225 of 893 462 base-level
  blocks use index 3 in three-colour mode).

DXT3 block (16 bytes): bytes 0..7 are explicit alpha, bytes 8..15 the
colour part. Alpha is sixteen 4-bit values in row-major texel order, two
bytes per texel row, the texel at the lowest column in the low nibble of
the row's first byte (equivalently: texel i's nibble is bits 4i+3..4i of
the u64 formed by bytes 0..7 little-endian). The alpha output is
expand4(nibble) = nibble * 17.

DXT5 block (16 bytes): byte 0 = alpha endpoint a0, byte 1 = alpha
endpoint a1, bytes 2..7 a 48-bit little-endian value of sixteen 3-bit
indices (texel 0 in bits 2..0, texel i in bits 3i+2..3i), then the
colour part in bytes 8..15. The eight alpha values:

- if a0 > a1: index 0 is a0, index 1 is a1, and index k for k = 2..7 is
  floor(((8 - k) * a0 + (k - 1) * a1) / 7).
- if a0 <= a1: index 0 is a0, index 1 is a1, index k for k = 2..5 is
  floor(((6 - k) * a0 + (k - 1) * a1) / 5), index 6 is 0 and index 7 is
  255.

Verified visually on leaf, fern and decal alpha channels (DXT3 and DXT5
alphas clean and sharp). Hardware decoders differ from these integer
rules by at most a unit in the interpolated colours and alphas; compare
with a tolerance of 1 against anything not produced by this rule.

Pass-through: DXT data needs no decoding to use on any GPU with BC1/BC2/BC3
support; upload the stored levels as they are (limited to the stored
levels, next section). Decode to RGBA8 only for tests, tools and
fallback.

### The bump palette (code 17, "p8-bump")

Confidence: the table is checked against the disc (it equals the 1 024
bytes in the disc's executable exactly, 256 of 256 entries); the rule
below reproduces the table exactly (also checked).

The format has no palette data in the bitmap or the map. A texel is one
byte, an index into a built-in palette of 256 entries that is the same
for every bump map and lives in the game's main executable. It is the
palette of normal vectors: entry (R, G, B) is a unit vector (x, y, z)
encoded as byte = round(127.5 + 127.5 * component), x in red, y in
green, z in blue; the vector is flat-up (0, 0, 1) = (128, 128, 255) at
entry 124. The decoded RGBA8 image is just the table lookup, alpha
included. To get a vector back: component = byte / 127.5 - 1.

The table can be generated, so no table has to be carried:

1. A grid index runs over the integers -9 to 9 on each of two axes: i
   along red (x), j along green (y).
2. The 19 coordinate bytes for i (or j) = -9, -8, ... , 9 are: 25, 47,
   66, 82, 96, 107, 116, 122, 126, 128, 129, 133, 139, 148, 159, 173,
   189, 208, 229. (They are 127.5 + 127.5 * 0.8 * sign(i) * (i / 9)^2,
   rounded to nearest, with the two end values 25 and 229 written as the
   tie rounds down there.)
3. Entries are produced in this order: for j from -9 to 9 (outer), for i
   from -9 to 9 (inner), keeping the pair only when |i| + |j| <= 11.
   That keeps 249 pairs, numbered entry 0 to 248.
4. For a kept pair the entry is R = coordinate byte of i, G = coordinate
   byte of j, B = floor(127.5 + 127.5 * z + 0.5) with z = sqrt(1 - x^2 -
   y^2), x = 0.8 * sign(i) * (i / 9)^2 and y likewise from j (ideal,
   unrounded, values); alpha = 255.
5. Entries 249 to 254 are (0, 0, 0, 0). Entry 255 is R = 128, G = 128,
   B = 255, alpha = 0 (a flat normal that is fully transparent).

Blue by (|i|, |j|), hexadecimal (the table is symmetric in the signs and
in swapping i and j; "-" marks pairs that are not kept):

| |j| \ |i| | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | FF | FF | FF | FE | FD | FB | F7 | EF | E2 | CC |
| 1 | FF | FF | FF | FE | FD | FB | F7 | EF | E2 | CC |
| 2 | FF | FF | FF | FE | FD | FB | F7 | EF | E2 | CC |
| 3 | FE | FE | FE | FE | FD | FB | F6 | EE | E2 | - |
| 4 | FD | FD | FD | FD | FC | F9 | F5 | ED | - | - |
| 5 | FB | FB | FB | FB | F9 | F7 | F2 | - | - | - |
| 6 | F7 | F7 | F7 | F6 | F5 | F2 | - | - | - | - |
| 7 | EF | EF | EF | EE | ED | - | - | - | - | - |
| 8 | E2 | E2 | E2 | E2 | - | - | - | - | - | - |
| 9 | CC | CC | CC | - | - | - | - | - | - | - |

Anchor entries to test an implementation (R, G, B, A):

| Entry | Value |
|---|---|
| 0 | (122, 25, 204, 255) |
| 4 | (133, 25, 204, 255) |
| 5 | (116, 47, 226, 255) |
| 103 | (122, 126, 255, 255) |
| 115 | (25, 128, 204, 255) |
| 124 | (128, 128, 255, 255) |
| 133 | (229, 128, 204, 255) |
| 248 | (133, 229, 204, 255) |
| 249 | (0, 0, 0, 0) |
| 255 | (128, 128, 255, 0) |

On disc 130 bump bitmaps occur in the multiplayer maps and the front-end
map; their indices stay within 0..248 except the value 255, which 13 of
them use. A bump map in a map file has already been converted from the
author's height map; a renderer never converts height to normals. In the
128 height-map-usage bitmaps seen, 124 are code 17, the rest (14 in all
24 maps, one debugging bitmap) are code 11.

## Xbox storage

Confidence: checked against the disc for everything except where marked.

### Which formats are swizzled

By the flags (and by format code): codes 14, 15, 16 are never swizzled;
every other code is swizzled except bitmaps with the linear flag (bit 4).
The only linear bitmap on disc (hud_health_base, type 4 tag, a4r4g4b4,
128 x 32, no mip chain) is stored as plain top-to-bottom rows of
width * 2 = 256 bytes; its stored size 0x2000 = 128 * 32 * 2 leaves no
room for row padding. Whether a linear image of another width would
carry a padded pitch cannot be told from the disc (see Open questions);
the flag is set by the engine itself on every bitmap of an interface-type
tag and means that no power-of-two restriction applies to that tag.

### The swizzle rule (Morton order, per level)

For one level of width W, height H and depth D (D = 1 for 2D images) the
texel at (x, y, z) is stored at texel number n (the byte offset in the
level is n times the texel size, from the level's start):

1. Build three bit masks by stepping k = 0, 1, 2, ... and handing out
   successive bit positions of n. At each step: if 2^k < W, the next free
   position goes to the x mask; then if 2^k < H, the next free position
   goes to the y mask; then if 2^k < D, the next free position goes to
   the z mask. Stop at the first step in which nothing was handed out.
   A dimension of 1 therefore gets an empty mask.
2. n is x deposited into the x mask's positions (its lowest bit goes to
   the lowest set position of the mask, and so on), OR y deposited into
   the y mask, OR z deposited into the z mask.

So x and y alternate from the least significant bit, x first; when one
dimension runs out the other keeps going alone in the higher bits. With
W = H it is the usual bit interleave, n = x0 + 2*y0 + 4*x1 + 8*y1 + ...;
a 3D image interleaves x, y, z, x, y, z, ... The rule is applied to each
level separately with that level's own W, H, D. The decoder reverses it:
for every output (x, y, z) compute n and read that texel.

Worked masks: 4 x 4: x mask 0101 (binary, bits 0 and 2), y mask 1010.
8 x 2: step 0 hands bit 0 to x and bit 1 to y; step 1 (2^1 = 2 < 8 but
not < 2) hands bit 2 to x; step 2 hands bit 3 to x: x mask 1101, y mask
0010. 2 x 8: x mask 0001, y mask 1110 (which makes the order the same as
plain rows of two). 1 x N and N x 1 levels are plain linear order.

Evidence: all non-square swizzled levels decode consistently (a mip
level agrees with the 2 x 2 average of the one before it to within 1
unit on average, only possible when the swizzle is right), and the
lightmap pages (non-square r5g6b5 pages such as 256 x 128) decode into
clean packed charts with an exact one-texel checkerboard filler. The 3D
axis order (x lowest, then y, then z) follows the reference; the disc's
3D images are noise, a spherical gradient and constants, which look the
same under any axis order, so the order itself is only reference-checked.

### Levels and their order

Confidence: checked on every bitmap of all 24 maps (the size rule).

For a 2D or 3D bitmap the stored levels follow one after another from
the pixels offset with no padding or header between them:

- Uncompressed (codes 0 to 3, 6 to 11, 17): ALL n + 1 levels down to the
  last (1 x 1 x 1, or a non-square tail such as 1 x 2) are stored. Level
  l takes W_l * H_l * D_l * (bits / 8) bytes.
- Block compressed (14, 15, 16): only the levels whose larger dimension
  is at least 4 are stored, that is, a level is stored while
  max(W_l, H_l) >= 4. Smaller levels (2 x 2, 1 x 1, and the non-square
  1 x 2, 2 x 1 and the like) are NOT in the file even though the mipmap
  count counts them: a 64 x 64 DXT1 with count 6 has 5 stored levels
  (64, 32, 16, 8, 4). A stored level takes ceil(W_l / 4) * ceil(H_l / 4)
  blocks times 8 (DXT1) or 16 (DXT3, DXT5) bytes; a level with one
  dimension below 4 (for example 1 x 8) occupies whole blocks that cover
  4 columns. The bytes just after the last stored level, inside the
  size's rounding, are zero or belong to the padding; do not read them as
  levels. This held for all 8 641 DXT bitmaps of the 24 maps. A consumer
  that needs the missing tail can drop it (limit the mip count to the
  stored levels, which is what a GPU upload does) or build it from the
  last stored level.
- Total stored bytes S = sum of the stored levels. Pixels size = S
  rounded up to a multiple of 128. (For 3D images with mipmaps the slices
  of one level are contiguous, level after level, and depth halves per
  level down to 1.)

Mipmap count: counts levels beyond the base. Dimensions needn't be
square: a 256 x 512 image has 10 levels of which the larger side drops
from 512 to 1 (count 9). Non-power-of-two dimensions do not occur on the
disc (see Measured facts); the engine's flag bit 0 says "all dimensions
are powers of two" and is set on every bitmap; the format would also
allow non-power-of-two only for the interface type, linear.

### Cube maps

Each of the six faces is stored as a complete chain of its own (all its
levels one after the other, as above); the six face chains are laid out
face after face. A face chain's start is at a multiple of the stride:
stride = S_face rounded up to a multiple of 128, where S_face is the sum
of the stored levels' bytes of ONE face. Face f starts at f * stride from
the pixels offset; level l of face f starts at that plus the sum of the
sizes of levels 0..l-1. Pixels size = 6 * stride. Each face level is
swizzled (or block compressed) on its own, exactly as a 2D image of that
size. Width equals height on every cube map on the disc. Checked: all
287 cube maps in the 14 files have the pixels size equal to 6 times the
rounded face chain, and the mip levels of every face agree with their
face's base (face pictures: see "What was looked at").

Face order and orientation (checked on the disc's vector-normalisation
cube map, whose texel colour is the direction it stands for, so each
face's meaning can be read off directly):

| Face | Axis the face looks along | Moving right in the image (x grows) goes toward | Moving down (y grows) goes toward |
|---|---|---|---|
| 0 | +X | -Z | -Y |
| 1 | -X | +Z | -Y |
| 2 | +Y | +X | +Z |
| 3 | -Y | +X | -Z |
| 4 | +Z | +X | -Y |
| 5 | -Z | -X | -Y |

These are the usual faces of GPU cube maps (identical image layout to
Direct3D and OpenGL cube faces), in the cube map's own frame (x right,
y up, z forward in the sampling space). The centre texel of face f is
(255, 127, 127), (1, 127, 129), (129, 255, 129), (129, 1, 127), (129, 127,
255), (127, 127, 1), as expected for a unit vector pointing along +X,
-X, +Y, -Y, +Z, -Z in the byte encoding of the bump palette section.
How the cube map's frame maps onto world axes is the shader's job (see
shaders.md), not the bitmap's.

### 3D (volume) textures

A 3D bitmap (type 1) is stored swizzled as one volume per level: level l
is W_l x H_l x D_l texels with x, y, z interleaved (the rule above) and no
separate padding between slices. Depth is a power of two and halves with
each level down to 1. Only three formats occur on the disc (ay8, a8y8,
x8r8g8b8), all of them with all levels stored (for 32 x 32 x 32: 6
levels). "Slice order": a decoded slice z is the set of texels with that
z. Plain (non-swizzled) volumes, whose slices would be consecutive images,
do not occur on the disc.

### Row order, padding and alignment summary

- Rows: top row first, in every format; compressed block rows top first.
- Padding between mip levels: none. Between cube faces: up to 127 bytes
  (each face chain rounded to 128). Between bitmaps: up to 511 bytes
  (each bitmap starts at a multiple of 512). After the last level: up to
  127 bytes of padding that is part of pixels size.
- Pixel data is raw bytes: no compression, no endianness swap.

## Bitmap types and usages: what they mean to a renderer

Confidence: reference only for the build-time meaning; the type and
format sets checked against the disc.

Tag types (the tag's type, which decides how its bitmaps are used):

- 2D textures (0): ordinary textures, power-of-two sizes, with mip
  chains; the typical case (3 178 tags).
- 3D textures (1): volume textures. Each bitmap's depth is its slice
  count; a sequence's bitmaps came from slices at import. Used by the
  shield and distance-attenuation effects.
- Cube maps (2): each bitmap is one cube map (six faces, as above); used
  for reflections, spotlights, headlight gels and the vector
  normalisation lookup.
- Sprites (3): each bitmap is a texture PAGE that holds many small
  pictures; the sequences' sprites say where each picture is on which
  page. Used by particles, decals and the HUD's combined pages. The
  pages themselves are ordinary 2D bitmaps (mostly DXT3 and DXT1 with a
  short mip chain, or 16-bit).
- Interface bitmaps (4): HUD and menu pictures that need no mip chain
  and no power-of-two size. The engine marks every bitmap of such a tag
  linear. On disc only one bitmap uses this and it is power-of-two; all
  the front-end's menu pictures are ordinary 2D or sprite tags.

Usage (build time only; the shipped mip chain is already built). What it
tells a renderer:

- alpha-blend (0), default (1): nothing to do.
- height map (2): the source was a height map; the shipped bitmap is
  already the palettized bump map (code 17). Nothing to convert; the
  "bump height" field says how tall the bump looks (in texture repeats)
  and can be passed to the shader that scales the bump.
- detail map (3): its mip chain fades toward grey (colour) and white
  (alpha) so that a detail texture vanishes with distance; no run-time
  work, the fade is in the data. Detail maps use the same formats (DXT1
  mostly, one 16-bit format).
- light map (4): no mip chain (count 0), 16-bit r5g6b5 swizzled pages.
  Chart packing, borders and filler are covered in the lightmaps part
  of the measured section; how the pages are applied belongs in
  structure_bsp.md.
- vector map (5): pixels are direction vectors biased into 0..255; the
  mip chain is renormalised. Use point or bilinear sampling without
  gamma or sRGB treatment: a vector map is data, not colour.

Colour space: nothing in the bitmap says sRGB or linear. The game
treats the stored bytes as display-referred and samples them without
conversion; a port that wants gamma-correct lighting decides that
for itself. Alpha is straight (not premultiplied) in every format.

Alpha conventions: DXT1 gives 0 or 255 (color key), DXT3 gives 16 evenly
spaced levels, DXT5 gives smooth alpha, ay8 and a8y8 give an 8-bit alpha
tied (or not) to intensity, a1r5g5b5 gives 0 or 255, a4r4g4b4 gives 16
levels. Whether a surface is alpha-tested, blended or additive comes from
its shader tag, not from the bitmap (shaders.md).

Sprite sequences and animation frames (reference only): a thing that
draws a bitmap asks for a (sequence index, frame index) pair.

1. If the tag has no sequences, the frame index is the bitmap index.
2. Otherwise the sequence is sequences[sequence index mod count of
   sequences]. If the sequence's bitmap count is above 0, the bitmap is
   first bitmap index + (frame index mod bitmap count) (frames wrap).
   Else, if the sequence has sprites, the frame index picks a sprite and
   the sprite's bitmap index names the page (and the sprite's rectangle
   says where on it). The frame is NOT wrapped in this case.
3. If still nothing is chosen, the frame index is used as the bitmap
   index.
4. The result is valid only if it is inside the bitmaps block; otherwise
   nothing is drawn.

A negative sequence index (some HUD elements pass -1) means "no
sequence": use rule 1 (the engine then logs that a sequence was asked
for although the tag has them, once, silently).

A registration point on a bitmap element (pixels, from the top-left of
the bitmap) is the anchor an interface picture is positioned by.

## Measured facts

Confidence: checked against the disc. "14 files" means the 13
multiplayer maps (beavercreek, bloodgulch, boardingaction, carousel,
chillout, damnation, hangemhigh, longest, prisoner, putput, ratrace,
sidewinder, wizard) and ui.map.

Bitmap images per file (tags in brackets where measured): beavercreek 501,
bloodgulch 516 (368 tags, 23 119 360 bytes of pixels), boardingaction
502, carousel 495, chillout 476, damnation 498, hangemhigh 462, longest
469, prisoner 464, putput 465, ratrace 484, sidewinder 510, wizard 471,
ui 384 (195 tags, 19 480 832 bytes). Total 6 697 images in 4 638 tags.

Format by bitmap type, all 14 files (every file holds its own copies,
so these are instances) and, in the last column, distinct images by (tag
path, index, format, size):

| Code | Format | 2D | 3D | Cube | Total | Distinct (2D / 3D / Cube) |
|---|---|---|---|---|---|---|
| 0 | a8 | 5 | 0 | 0 | 5 | 1 / 0 / 0 |
| 1 | y8 | 30 | 0 | 13 | 43 | 3 / 0 / 1 |
| 2 | ay8 | 754 | 14 | 0 | 768 | 56 / 1 / 0 |
| 3 | a8y8 | 365 | 13 | 0 | 378 | 27 / 1 / 0 |
| 6 | r5g6b5 | 172 | 0 | 39 | 211 | 160 / 0 / 3 |
| 8 | a1r5g5b5 | 92 | 0 | 0 | 92 | 7 / 0 / 0 |
| 9 | a4r4g4b4 | 16 | 0 | 0 | 16 | 3 / 0 / 0 |
| 10 | x8r8g8b8 | 338 | 56 | 89 | 483 | 30 / 4 / 7 |
| 11 | a8r8g8b8 | 193 | 0 | 14 | 207 | 41 / 0 / 1 |
| 14 | DXT1 | 2 557 | 0 | 132 | 2 689 | 309 / 0 / 17 |
| 15 | DXT3 | 1 595 | 0 | 0 | 1 595 | 322 / 0 / 0 |
| 16 | DXT5 | 80 | 0 | 0 | 80 | 16 / 0 / 0 |
| 17 | p8-bump | 130 | 0 | 0 | 130 | 40 / 0 / 0 |
| | all | 6 327 | 83 | 287 | 6 697 | 1 015 / 6 / 29 |

Headlines:

- Block compression carries 64 percent of the images (DXT1 40 percent,
  DXT3 24 percent, DXT5 1 percent); swizzled monochrome (ay8 and a8y8)
  17 percent; the rest are 16 and 32-bit formats.
- Codes 4, 5, 7, 12, 13 never occur. Every uncompressed non-linear
  bitmap is swizzled.
- 3D images exist in only three formats and are 32 x 32 x 32 (ay8:
  distance attenuation; a8y8: plasma shield noise) or 4 x 4 x 4
  (x8r8g8b8 "default 3d"). There are 83 instances in the 14 files.
- Cube maps are x8r8g8b8 64 x 64 (vector normalisation, plus a 4 x 4
  one), DXT1 64 x 64 (vehicle reflections), r5g6b5 64 x 64, y8 128 x 128
  (spotlight), a8r8g8b8 32 x 32 (active camouflage distortion).
- No bitmap has a non-power-of-two width or height on any of the 24
  discs' maps (the campaign maps included). No block-compressed bitmap
  has a width or height that is not a multiple of 4 (the smallest
  compressed side is 4, e.g. 4 x 4 level 0 of DXT1; there are none with
  a side of 1 or 2 at level 0; tiny compressed images such as 8 x 256
  and 8 x 512 exist).
- Largest dimension: 2048 (two DXT1 bitmaps, 2048 x 128: the ring
  structure of the sky). Next 1024 (126 instances), then 512 (620), 256
  (1 555), 128 (1 925), 64 (1 467), 32 (671), 16, 8, 4. Largest pixel
  data: 699 136 bytes for the 1024 x 512 DXT3 on-screen keyboard
  background; 1024 x 512 DXT5 (postgame carnage report, 524 288 bytes).
  Largest area: 1024 x 512 and 512 x 512 (a lightmap page, 524 288
  bytes in hangemhigh).
- Interface-type tags (type 4): 14 tags, all the same HUD health image.
  Linear bitmaps: 14, one per file, 128 x 32 a4r4g4b4.
- Mipmap counts: all values from 0 to 11 occur. Count 0 for lightmaps,
  interface images and some small decals; sprite pages mostly 1 or 2;
  textures the full chain except where the artist set fewer (leaf
  texture 256 x 256 with count 2 for example).
- Sequences: a non-sprite tag has sequences with bitmap count 1 (one
  bitmap per sequence is by far the commonest; 334 of 334 non-sprite
  sequences in bloodgulch.map are one-frame) and a sprite tag has one
  sequence per sprite group with the sprites listing pages. The names in
  sequences are mostly empty or "import #N".

### Lightmap bitmaps

Each multiplayer map has exactly one bitmap tag with usage light map: its
path is "levels\test\<map name>\<map name>" (for example the one in
bloodgulch.map is "levels\test\bloodgulch\bloodgulch"), type 2D,
format hint 16-bit, and its bitmaps are the lightmap pages. All pages in
every map are format 6 (r5g6b5), swizzled (flags 0x89), mip count 0,
stored with no padding other than the usual 128 rounding (pixels size is
width * height * 2 exactly). Pages are of mixed sizes from 8 x 8 up to
512 x 512, many of them non-square.

| Map | Pages | Sizes (width x height, count) | Pixel bytes |
|---|---|---|---|
| bloodgulch | 15 | 256x128 x1, 128x256 x1, 128x128 x2, 128x64 x1, 64x128 x1, 64x64 x1, 64x32 x2, 32x32 x1, 32x16 x3, 16x32 x1, 8x8 x1 | 252 032 |
| hangemhigh | 2 | 256x256 x1, 512x512 x1 | 655 360 |
| beavercreek | 13 | 128x64 x4, 32x32 x3, 128x128 x1, 64x64 x1, 16x16 x1, 64x32 x1, 8x8 x1, 16x8 x1 | 117 632 |
| boardingaction | 25 | 256x256 x4, 64x32 x4, 256x128 x3, 128x128 x3, 32x16 x3, 16x32 x2, 128x64 x2, 64x64 x2, 16x16 x1, 32x32 x1 | 892 416 |
| carousel | 28 | 64x64 x10, 16x8 x4, 32x16 x3, 256x128 x2, 128x64 x2, 16x32 x2, 64x32 x2, 32x32 x2, 128x256 x1 | 329 728 |
| chillout | 9 | 128x128 x3, 256x128 x2, 64x64 x1, 64x32 x1, 16x16 x1, 32x32 x1 | 244 224 |
| damnation | 10 | 64x64 x4, 256x256 x2, 128x128 x1, 64x32 x1, 128x64 x1, 16x16 x1 | 348 672 |
| longest | 1 | 256x256 | 131 072 |
| prisoner | 2 | 256x256 x2 | 262 144 |
| putput | 9 | 128x128 x7, 128x64 x1, 64x128 x1 | 262 144 |
| ratrace | 11 | 64x64 x3, 64x32 x3, 256x256 x1, 32x32 x1, 64x128 x1, 128x128 x1, 128x64 x1 | 235 520 |
| sidewinder | 23 | 64x64 x9, 128x64 x5, 256x256 x2, 32x16 x2, 64x32 x2, 64x128 x1, 128x128 x1, 8x8 x1 | 477 312 |
| wizard | 11 | 64x64 x3, 32x16 x2, 32x32 x2, 256x128 x1, 128x128 x1, 32x8 x1, 32x64 x1 | 133 632 |

In bloodgulch.map the first page is 256 x 128, the second 128 x 256 and
the others follow in the order of the table's first column of the tag.
Texels that no chart uses are filled with a deliberate checkerboard of
two colours: 0xFFE0 (yellow, 255 255 0) and 0xF800 (red, 255 0 0) in
r5g6b5, alternating every texel in both directions (in Morton order the
four texels of a 2 x 2 group read E0 FF, 00 F8, 00 F8, E0 FF). Pages are
therefore recognisable on sight and a renderer must never sample the
filler: lightmap coordinates stay inside charts. The charts are
shelf-packed rectangles and triangular patches (rows of rectangles of
varying height), contents warm cream to dark brown on bloodgulch and pale
blue grey on hangemhigh. No colour space or exposure scale is stored
here; it comes with the BSP's lightmap data (structure_bsp.md).

## Edge cases

- Reading beyond a stored level: never; use the size rule. A level the
  file does not store (a DXT level smaller than one block in both
  dimensions) must not be read at all, not even zeros.
- A format code outside 0..3, 6, 8..11, 14..17 is invalid. The loader
  should refuse the bitmap and not guess.
- A depth above 1 on a type other than 3D, or a cube map whose width and
  height differ, or a mip count above floor(log2(largest dimension)), is
  invalid and does not occur on the disc.
- A 1-pixel-wide or 1-pixel-tall uncompressed swizzled level is plain
  linear order (the rule gives an empty mask for the dimension of 1).
- A block-compressed bitmap smaller than one block per side (width or
  height below 4) does not occur at level 0 on the disc. If one did, the
  rule's ceil(W/4) gives one block and the decode keeps only the texels
  inside the level.
- The stored width and height of non-square bitmaps are "wide" and
  "tall" as they read: width is x. Do not swap.
- Texel (0,0) of every level is the top-left; for mip levels the same
  swizzle masks of that level apply (never the base level's masks).
- A cube map decoded face by face needs the same per-face stride for
  every face, including faces that are entirely black.

## Quirks

- Mip count counts levels the file does not store (compressed tails). Keep
  the count as a statement of the logical chain and use the stored-level
  rule for what to read.
- The format enumeration in the tag has holes (4, 5, 7, 12, 13 named
  "unused"); keep the numbering.
- The 0x80 flag is set on every shipped bitmap and the word at 0x16, and
  the dwords from 0x20 on, are junk: do not validate them.
- The tag's own format hint disagrees with the bitmaps' real formats
  (for example height-map groups of "32-bit color" turn out as code 17,
  "monochrome" groups hold x8r8g8b8 volumes). Trust the element.
- Every map duplicates the pixel data of shared bitmaps; two maps' copies
  of one path are not guaranteed identical in size (per-map builds), so
  key caches by (map, tag) unless pixel offsets and sizes agree.
- The a8 format's colour is a platform question (Open questions).

## Test cases

### Synthetic images (unit tests)

Each case is described byte by byte, with the expected decoded RGBA.
"2x1" cases use W = 2, H = 1 (a 2 x 1 swizzled image is stored in plain
order, so the first texel is x = 0). Hex byte strings are in file order.

16 and 8-bit formats, one image each, 2 x 1:

| Case | Format code | Bytes | Expected (x=0), (x=1) |
|---|---|---|---|
| alpha only | 0 | 00 FF | (0,0,0,0), (0,0,0,255) |
| intensity | 1 | 00 80 | (0,0,0,255), (128,128,128,255) |
| combined alpha-intensity | 2 | 40 FF | (64,64,64,64), (255,255,255,255) |
| separate alpha-intensity | 3 | 10 80 FF 00 | (16,16,16,128), (255,255,255,0) |
| r5g6b5 primaries | 6 | 00 F8 E0 07 | (255,0,0,255), (0,255,0,255) |
| r5g6b5 mid | 6 | 10 84 1F 00 | (132,130,132,255), (0,0,255,255) |
| a1r5g5b5 | 8 | 00 FC E0 03 | (255,0,0,255), (0,255,0,0) |
| a1r5g5b5 low blue | 8 | 10 00 FF FF | (0,0,132,0), (255,255,255,255) |
| a4r4g4b4 | 9 | 21 84 0F F0 | (68,34,17,136), (0,0,255,255) |
| x8r8g8b8 | 10 | 10 20 30 FF 10 20 30 00 | (48,32,16,255), (48,32,16,255) |
| a8r8g8b8 | 11 | 10 20 30 40 00 00 00 00 | (48,32,16,64), (0,0,0,0) |

The r5g6b5 mid case: u16 0x8410 gives R field 16, G field 32, B field
16, hence 132, 130, 132. The a4r4g4b4 first texel is u16 0x8421: A = 8
(136), R = 4 (68), G = 2 (34), B = 1 (17).

Swizzled 4 x 4 a8r8g8b8: source the 16 texels in file order, texel k
being the four bytes k, 00, 00, FF (so blue = k, R = G = 0, alpha 255).
After unswizzling, blue at each position (rows y = 0..3, columns
x = 0..3) must read:

| y \ x | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| 0 | 0 | 1 | 4 | 5 |
| 1 | 2 | 3 | 6 | 7 |
| 2 | 8 | 9 | 12 | 13 |
| 3 | 10 | 11 | 14 | 15 |

Swizzled 8 x 2 a8r8g8b8, the same 16 source texels:

| y \ x | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| 0 | 0 | 1 | 4 | 5 | 8 | 9 | 12 | 13 |
| 1 | 2 | 3 | 6 | 7 | 10 | 11 | 14 | 15 |

A 2 x 8 image with the same source reads in plain order: row y holds
blue 2y and 2y + 1. A swizzled 2 x 2 x 2 volume with source texel k
(four bytes k, 00, 00, FF): slice z, row y, column x holds blue
x + 2y + 4z.

Block compression: each block is described, then the expected 4 x 4
texels (R,G,B,A), row by row. Endpoints below use red = 0xF800 and
blue = 0x001F.

DXT1 four-colour block, bytes 00 F8 1F 00 E4 1B 50 FA (colour 0 red,
colour 1 blue, so colour 0 > colour 1 as unsigned: four-colour mode;
index bytes E4 = indices 0,1,2,3; 1B = 3,2,1,0; 50 = 0,0,1,1; FA =
2,2,3,3):

| Row | Texel 0 | Texel 1 | Texel 2 | Texel 3 |
|---|---|---|---|---|
| 0 | 255,0,0,255 | 0,0,255,255 | 170,0,85,255 | 85,0,170,255 |
| 1 | 85,0,170,255 | 170,0,85,255 | 0,0,255,255 | 255,0,0,255 |
| 2 | 255,0,0,255 | 255,0,0,255 | 0,0,255,255 | 0,0,255,255 |
| 3 | 170,0,85,255 | 170,0,85,255 | 85,0,170,255 | 85,0,170,255 |

DXT1 three-colour block with colour key, bytes 1F 00 00 F8 E4 FF 00 55
(colour 0 blue, colour 1 red: colour 0 < colour 1, so three-colour mode;
index bytes E4 = 0,1,2,3; FF = all 3; 00 = all 0; 55 = all 1):

| Row | Texel 0 | Texel 1 | Texel 2 | Texel 3 |
|---|---|---|---|---|
| 0 | 0,0,255,255 | 255,0,0,255 | 127,0,127,255 | 0,0,0,0 |
| 1 | 0,0,0,0 | 0,0,0,0 | 0,0,0,0 | 0,0,0,0 |
| 2 | 0,0,255,255 | 0,0,255,255 | 0,0,255,255 | 0,0,255,255 |
| 3 | 255,0,0,255 | 255,0,0,255 | 255,0,0,255 | 255,0,0,255 |

DXT3 block, bytes 10 32 54 76 98 BA DC FE then 1F 00 00 F8 E4 1B 50 FA
(alpha nibbles 0 to 15 in texel order; colour 0 blue, colour 1 red,
which in DXT3 still means four-colour mode: index 2 = (85,0,170),
index 3 = (170,0,85)):

| Row | Texel 0 | Texel 1 | Texel 2 | Texel 3 |
|---|---|---|---|---|
| 0 | 0,0,255,0 | 255,0,0,17 | 85,0,170,34 | 170,0,85,51 |
| 1 | 170,0,85,68 | 85,0,170,85 | 255,0,0,102 | 0,0,255,119 |
| 2 | 0,0,255,136 | 0,0,255,153 | 255,0,0,170 | 255,0,0,187 |
| 3 | 85,0,170,204 | 85,0,170,221 | 170,0,85,238 | 170,0,85,255 |

DXT5 block, eight-value alpha, bytes FF 00 88 C6 FA 88 C6 FA then 1F 00
00 F8 E4 1B 50 FA (a0 = 255 > a1 = 0; alpha indices 0..7 twice; alphas
255, 0, 218, 182, 145, 109, 72, 36):

| Row | Texel 0 | Texel 1 | Texel 2 | Texel 3 |
|---|---|---|---|---|
| 0 | 0,0,255,255 | 255,0,0,0 | 85,0,170,218 | 170,0,85,182 |
| 1 | 170,0,85,145 | 85,0,170,109 | 255,0,0,72 | 0,0,255,36 |
| 2 | 0,0,255,255 | 0,0,255,0 | 255,0,0,218 | 255,0,0,182 |
| 3 | 85,0,170,145 | 85,0,170,109 | 170,0,85,72 | 170,0,85,36 |

DXT5 block, six-value alpha, bytes 10 F0 88 C6 FA 88 C6 FA then the same
colour part (a0 = 16 <= a1 = 240; alphas for indices 0..7 are 16, 240,
60, 105, 150, 195, 0, 255):

| Row | Texel 0 | Texel 1 | Texel 2 | Texel 3 |
|---|---|---|---|---|
| 0 | 0,0,255,16 | 255,0,0,240 | 85,0,170,60 | 170,0,85,105 |
| 1 | 170,0,85,150 | 85,0,170,195 | 255,0,0,0 | 0,0,255,255 |
| 2 | 0,0,255,16 | 0,0,255,240 | 255,0,0,60 | 255,0,0,105 |
| 3 | 85,0,170,150 | 85,0,170,195 | 170,0,85,0 | 170,0,85,255 |

Two-level mip chain, swizzled a8r8g8b8, base 2 x 2, mipmap count 1,
stored bytes (20 of them, pixels size 128 after rounding):
level 0: 00 00 FF FF, 00 FF 00 FF, FF 00 00 FF, FF FF FF FF; level 1:
10 20 30 FF. Level 0 is (x=0,y=0) red (255,0,0,255), (1,0) green
(0,255,0,255), (0,1) blue (0,0,255,255), (1,1) white (255,255,255,255)
(the four texels are in the 2 x 2 swizzled order, which is the plain
order x then y); level 1 (1 x 1) is (48,32,16,255). Level 1 starts at byte
16.

Compressed chain with the stored-level rule: DXT1 8 x 8, mipmap count 3
(logical levels 8x8, 4x4, 2x2, 1x1). Stored bytes, 40 of them (pixels
size 128): level 0 is four blocks in the order top-left, top-right,
bottom-left, bottom-right: 00 F8 00 F8 00 00 00 00 (solid red), E0 07 E0
07 00 00 00 00 (solid green), 1F 00 1F 00 00 00 00 00 (solid blue), FF
FF FF FF 00 00 00 00 (solid white); level 1 (4 x 4, one block): 10 84 10
84 00 00 00 00 (solid 132,130,132). Expected: level 0 pixel (0,0) =
(255,0,0,255), (4,0) = (0,255,0,255), (0,4) = (0,0,255,255), (4,4) =
(255,255,255,255); level 1 every pixel (132,130,132,255). The decoder
must report 2 stored levels, must not read beyond byte 40, and a loader
that limits the GPU mip count to the stored levels uses 2.

Cube map: a 2 x 2 a8r8g8b8 cube, mipmap count 0: face chain one level of
16 bytes, stride 128 bytes, pixels size 768. Face f starts at 128 * f.
With face f's four texels all equal to (f * 40, 0, 0, 255) (bytes 00 00
(f*40) FF), the decoded top-left of face 3 is (120, 0, 0, 255) and the
data for face 5 begins at byte 640.

Linear interface image: a4r4g4b4 4 x 2 with the linear flag, rows of 8
bytes, no padding: 21 84 0F F0 ... decoded in row-major order; the
first row's first two texels are (68,34,17,136) and (0,0,255,255) as in
the 16-bit table.

Bump palette lookups (code 17, 2 x 1 swizzled, no flags needed beyond
the format): bytes 00 7C decode to the palette entries 0 and 124:
(122,25,204,255) and (128,128,255,255); bytes F8 FF decode to entries 248
and 255: (133,229,204,255) and (128,128,255,0). The generated table must
equal the anchor table in the palette section.

Size rules (a 5-case table to test the size model): expected pixels size
equals the stated number.

| Bitmap | Chain | Pixels size |
|---|---|---|
| 256 x 256 DXT1 count 8 | stored 256..4 (7 levels): 32768+8192+2048+512+128+32+8 = 43 688, rounded to 128 | 43 776 (0xAB00) |
| 512 x 512 DXT3 count 9 | stored 512..4 (8 levels): 349 520 | 349 568 (0x55580) |
| 256 x 512 DXT1 count 9 | stored 8 levels (256x512 to 2x4... max side >= 4): 87 384 | 87 424 (0x15580) |
| 64 x 64 DXT1 cube count 6 | one face chain 64..4: 2 736, stride 2 816, x 6 | 16 896 (0x4200) |
| 32 x 32 a8r8g8b8 count 5 | levels 32..1: 5 460 | 5 504 (0x1580) |

### Disc-backed expectations (slow tests, bloodgulch.map)

All pixel colours are from the base level unless noted; (x, y) is the
column and row. DXT values use the integer rules above; compare with
tolerance 1 per channel. Offsets are the element's pixels offset.

| Tag (class bitm) | Image | Expected |
|---|---|---|
| levels\test\bloodgulch\bitmaps\blood ground | 1 bitmap, 512 x 512, format 15 (DXT3), mipmap count 9, flags 0x83, offset 0x15A1A00, pixels size 0x55580 (349 568) | (0,0) = (107,93,74,221); (200,300) = (132,101,66,85); (256,256) = (198,134,82,255); (511,511) = (101,87,68,221). Level 1 (256 x 256) (0,0) = (99,85,66,238) and (128,128) = (198,138,82,255). 8 stored levels. |
| levels\test\bloodgulch\bitmaps\blood cliff rock | 1 bitmap, width 256, height 512, format 14 (DXT1), count 9, flags 0x83, offset 0x14D2800, size 0x15580 (87 424) | (0,0) = (118,65,38,255); (130,77) = (104,52,29,255); (255,511) = (107,65,41,255). Level 1 (128 x 256): (0,0) = (132,77,41,255). 8 stored levels (the last stored is 2 x 4; 1 x 2 and 1 x 1 are not). |
| levels\test\bloodgulch\bloodgulch (the lightmap tag) | 15 bitmaps, usage 4, type 0; bitmap 0: 256 x 128, format 6, count 0, flags 0x89, offset 0x1494E00, size 0x10000 | bitmap 0: (0,0) = (222,207,181,255); (100,60) = (255,247,222,255); (255,127) = (255,255,0,255) (filler). Bitmap 1 is 128 x 256, offset 0x14A4E00, size 0x10000: (0,0) = (255,239,214,255); (60,100) = (41,44,33,255). Sizes of all 15 pages: see the lightmap table; the sum of pixels sizes is 252 032. |
| levels\b30\bitmaps\metal flat generic bump | 1 bitmap, 256 x 256, format 17, count 8, flags 0x8D, offset 0x41CC00, size 0x15580 | texel (0,0) is index 124: (128,128,255,255); texel (188,79) is index 103: (122,126,255,255); the most common index is 124 (23 043 of 65 536 base texels). |
| rasterizer\vector normalization | 2 bitmaps. Bitmap 0: type 2 (cube), 64 x 64, format 10, count 6, flags 0x89, offset 0xFDA000, size 0x20100. Bitmap 1: cube, 4 x 4, count 2, offset 0xFFA200, size 0x300 | bitmap 0: (32,32) of faces 0..5 = (255,127,127), (1,127,129), (129,255,129), (129,1,127), (129,127,255), (127,127,1). (0,0) of faces 0..5 = (202,201,201), (54,201,55), (55,202,55), (55,54,201), (55,201,202), (201,201,54). (63,63) of face 0 = (202,55,55). |

Further checks that need no hand-written values: for every bitmap of
every map, the pixels size must equal the value the size model predicts;
the pixels offset must be a multiple of 512; offset + size must lie
inside the decompressed image; bitmaps sorted by offset must not
overlap. Of the 9 327 tags, every colour-plate and pixel-data data
reference must be empty (address 0).

## What was looked at

The pictures were decoded with throwaway scripts and viewed. From
bloodgulch.map: DXT1 (sky stars, tree trunks, ground and rock textures,
the three planet sprites), DXT3 (planets, tree leaves, ferns and grass
cards with their alpha, ground), DXT5 (metal plates, bullet-hole decals,
glass), a8 (a cloud mask), y8 (a lens flare, snow treads), ay8 (HUD unit
backgrounds, bars, smoke puffs), a8y8 (HUD meters and warning lettering),
r5g6b5 (a cube map, detail map, lightmap pages), a1r5g5b5 (muzzle
flashes), a4r4g4b4 (the health base, linear), x8r8g8b8 (flares, noise,
the vector normalisation cube), a8r8g8b8 (camouflage, jackal shield
mask), p8-bump (teleporter and metal bump maps, whose normal-map look
confirms the palette), 3D (plasma noise, distance attenuation, default
constants), cube maps (all six faces of five cubes, face order and
orientation from the vector normalisation one). From ui.map: DXT1 sky
and star pictures, DXT5 carnage report, a1r5g5b5 plasma bolts, a4r4g4b4
menu bars, x8r8g8b8 gradient. From hangemhigh.map: the two lightmap
pages. Mip levels were checked numerically on bloodgulch.map, the
front-end map and hangemhigh.map: for every uncompressed format a level
agrees with the average of the level above within 1 unit (usually 0.1 to
0.3) down to 1 x 1; for DXT the stored levels agree to within the
encoder's noise and the unstored tails are zero. Not looked at as
pictures: any bitmap of the campaign maps (the layout rules were checked
there by stored size only).

## Open questions

- Colour of a8 (alpha-only) texels. The reference's converter writes
  (0, 0, 0, A). A different console-side reading ((255, 255, 255, A)) is
  conceivable because some D3D implementations return white colour for
  alpha-only surfaces. Only 5 bitmaps on the multiplayer discs use code 0
  (cloud masks in a sky) and a shader that samples only alpha does not
  care. Check by seeing whether any shader samples colour from an a8
  bitmap; until then use (0, 0, 0, A).
- Pitch of linear textures. The one disc example has no row padding.
  If a linear image of another width ever shipped (hardware wants a pitch
  that is a multiple of 64 bytes), the layout is unknown; only widths
  whose natural pitch (width times texel size) is a multiple of 64 are
  known to be un-padded.
- Axis order of the 3D swizzle. x lowest, then y, then z, from the
  reference; the disc's volumes cannot distinguish the orders. Re-check if
  an anisotropic volume is ever found (none in the 24 maps).
- Hardware rounding of DXT interpolation and 5/6-bit expansion. The
  integer rules in this spec are the usual ones; the console's hardware
  may differ by one unit. Check by comparing against a screenshot or
  texture dump from the real console, if one becomes available.
- Whether the missing tail levels of compressed bitmaps (2 x 2, 1 x 1)
  should be synthesised by the port or ignored. The game never reads
  them (the file does not have them), so ignoring them is faithful.
- Whether the cube-face orientation above is mirrored on a given API:
  it is the orientation of the data as stored; a port whose cube
  sampling convention has the opposite handedness must flip faces
  accordingly (the stored face contents themselves are not ambiguous,
  see the vector normalisation map).
- Meaning of the retained colour plate size fields (width, height at
  0x18, 0x1A): they equal the authoring image's size, nothing uses them.
  Whether any tool wants them is outside this spec.
