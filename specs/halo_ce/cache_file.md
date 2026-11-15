# Halo: Combat Evolved: the map cache file and tag conventions

Source basis: the game's own map files from the original Xbox disc (all
24 `.map` files: 10 campaign levels, 13 multiplayer maps and `ui.map`),
read with throwaway scripts; and a decompilation of an Xbox build of
the game's engine (the cache file loader, the tag system's field types,
the maths types, the physics constants), used for meaning only. The
decompilation describes a different Xbox build (a PAL build, 01.01.14.2342)
from the disc's (NTSC, 01.10.12.2276); where they differ the disc wins
and the difference is noted.
Status: draft

Sections: Summary; Units and conventions; Constants; The map file on
disc; Compression; The decompressed image; Address space; The tag
header; The tag index; Structure BSP sub-files; Where geometry, pixels
and samples live; Tag data conventions; Tag classes; What is on the
disc; Load order; Edge cases; Quirks; Test cases; Confidence per
section; Open questions.

---

## Summary

- A map (`bloodgulch.map`) is a 2048-byte header followed by one zlib
  stream and zero padding. Inflated and appended to the header, it gives
  the *decompressed image*: the file the engine actually reads.
- The decompressed image holds, in order: the header, the structure
  BSPs, all bitmap pixel data, all sound sample data, and last the *tag
  data*. The tag data is loaded whole at the fixed Xbox address
  0x803A6000, and every pointer inside it is an address in that space.
  Each structure BSP has its own address space.
- The tag data starts with a tag header and a tag index: one 32-byte
  entry per tag with its class code, parent class codes, tag id, path and
  the address of its data. Tag data is a tree of fixed-size structs
  linked by *blocks* (count + address), *tag references* (class + path +
  id) and *data references* (size + address, or a file offset).
- The world: 1 world unit = 10 feet = 3.048 m; +Z up, +X forward, +Y
  left (right-handed); angles in radians; 30 ticks per second; many
  speeds in tags are stored per tick.
- Every multiplayer map holds the same 15 weapon, 16 projectile, 14
  equipment, 2 biped and 3 vehicle tags, with main structs identical
  apart from addresses and ids. Any one multiplayer map can donate all
  Halo guns.

## Units and conventions

### Bytes

- Everything is little-endian. Integers are two's complement.
  "u32", "i32", "u16", "i16", "u8", "i8" name widths; "f32" is an IEEE
  754 single.
- Offsets called *file offsets* count from the first byte of the
  **decompressed image** (which begins with the 2048-byte header), never
  from the compressed file, unless a field says otherwise. The only
  offsets into the compressed file are in Compression.
- *Addresses* are 32-bit Xbox memory addresses. The tag data and each
  BSP map them to file offsets as in Address space.
- A *class code* (tag group) is four ASCII characters stored as a u32
  whose big-endian spelling is the code: `bipd` is the value 0x62697064,
  stored as the bytes 64 70 69 62. Codes are case-sensitive (`DeLa`,
  `Soul`) and may contain spaces (`sky `, `fog `, `hmt `) or punctuation
  (`snd!`, `jpt!`, `str#`, `hud#`, `glw!`, `ant!`). In this spec codes
  are written as their four characters.
- "NONE" is all bits set: 0xFFFFFFFF for 32-bit fields, 0xFFFF (-1) for
  16-bit, 0xFF (-1) for 8-bit index fields.
- Alignment: header fields are naturally aligned; the tag header and
  tag index are 4-aligned; every tag's struct address is 4-aligned (about
  half are 8-aligned, so 4 is all a reader may assume); tag path strings
  are packed byte-aligned. Tag data starts on a 512-byte boundary of the
  decompressed image. Struct fields inside tags follow C-style natural
  alignment with explicit padding; readers use the offsets the class
  specs give and never compute alignment themselves.

### The world

| Quantity | Value | How we know |
|---|---|---|
| World unit (WU) | 3.048 m (10 ft) | engine gravity 0.0035651792 WU/tick^2 = 3.20866 WU/s^2 = 9.780 m/s^2 at 3.048 m/WU; the Spartan biped is 0.7 WU (2.13 m, 7 ft) tall |
| Up axis | +Z | engine's identity frame: forward (1,0,0), left (0,1,0), up (0,0,1) |
| Forward | +X at yaw 0 | direction from angles: (cos yaw cos pitch, sin yaw cos pitch, sin pitch) |
| Left | +Y | forward x left = up, so the frame is right-handed |
| Yaw | radians, counter-clockwise seen from above, from +X toward +Y | same formula |
| Pitch | radians, positive up | same formula |
| Angles in tags | radians (the editing tools show degrees; the cache stores radians) | spawn facings 1.5707964 (pi/2), sound cone angles 6.2831855 (2 pi), biped max slope 0.7853982 (pi/4) on the disc |
| Tick | 1/30 s (30 ticks per second) | engine constant; ticks per minute 1800 |
| Display refresh assumed | 60 vblanks per second | engine constant (presentation only) |
| Colour components (real) | 0 to 1 | colour tables on the disc |

Disc check of the unit: the multiplayer Spartan biped
`characters\cyborg_mp\cyborg_mp` has standing collision height 0.7 WU,
crouching 0.5, collision radius 0.2, standing camera height 0.62, crouch
camera height 0.35 (the biped spec owns those fields). At 3.048 m/WU
that is a 2.13 m tall, 0.61 m wide figure with eyes at 1.89 m.

**Per tick versus per second.** The tools that built the maps convert
some fields at build time, so the cache stores them per tick even though
the editor shows per second. Checked on the disc: projectile initial and
final velocities are per tick (pistol bullet 10.0 WU/tick = 300 WU/s,
sniper bullet 33.333 = 1000 WU/s), biped jump velocity is per tick
(0.07 WU/tick). Durations stay in seconds (projectile arming time 1.5 s,
detonation timers 0.5 and 2.0 s), and angular rates stay per second
(needle guidance 1.5708 rad/s, which the engine divides by 30 when it
uses it). Each class spec states the unit of every field it defines;
this spec only fixes the rule that the stored value is what the engine
uses, with no conversion on load.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| HEADER_SIZE | 0x800 (2048) | bytes | the header at the start of both the file and the decompressed image |
| HEAD_MAGIC | 0x68656164 (`head`, bytes 64 61 65 68) | u32 | header offset 0x000 |
| FOOT_MAGIC | 0x666F6F74 (`foot`, bytes 74 6F 6F 66) | u32 | header offset 0x7FC |
| XBOX_CACHE_VERSION | 5 | | header offset 0x004 on every Xbox map |
| TAGS_MAGIC | 0x74616773 (`tags`, bytes 73 67 61 74) | u32 | last field of the tag header |
| SBSP_MAGIC | 0x73627370 (`sbsp`, bytes 70 73 62 73) | u32 | last field of a BSP sub-header |
| TAG_BASE | 0x803A6000 | address | where the tag data's first byte is loaded |
| TAG_CACHE_SIZE | 0x01600000 (22 MiB) | bytes | the memory window tag data and the loaded BSP share |
| TAG_CACHE_END | 0x819A6000 | address | TAG_BASE + TAG_CACHE_SIZE; every BSP ends exactly here |
| TAG_HEADER_SIZE | 0x24 | bytes | the tag header |
| TAG_ENTRY_SIZE | 0x20 | bytes | one tag index entry |
| SECTOR | 512 | bytes | tag data offset and decompressed size are multiples of it |
| FILE_ALIGN | 2048 | bytes | the compressed file's size is a multiple of it |
| FIRST_TAG_SALT | 0xE174 | u16 | the salt (high half) of tag 0's id on every disc map |
| TICK_RATE | 30 | ticks/s | game tick |
| WORLD_UNIT | 3.048 | m | 10 ft |
| GRAVITY | 0.0035651792 | WU/tick^2 | engine default; the globals tag scales it |
| TAG_STRING | 32 | bytes | a short string field: up to 31 characters and a NUL |

## The map file on disc

The file is the header (0x800 bytes, uncompressed) followed by the
compressed body. The header is copied unchanged to the start of the
decompressed image, so its offsets describe the decompressed image.

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x000 | 4 | u32 | HEAD_MAGIC `head` |
| 0x004 | 4 | i32 | Cache version. 5 on all 24 disc maps. Other releases of the game use other numbers (the PC releases are not 5); a reader for this port accepts 5 only. |
| 0x008 | 4 | i32 | Decompressed size: header + inflated body, in bytes. A multiple of 512 on the disc (not of 2048). |
| 0x00C | 4 | i32 | Compressed padding: the number of zero bytes after the end of the zlib stream, to the end of the file. Always below 2048. (The decompilation treats this field as reserved; the disc fills it on every map.) |
| 0x010 | 4 | i32 | Tag data offset: file offset of the tag data in the decompressed image. A multiple of 512. |
| 0x014 | 4 | i32 | Tag data size in bytes. Not a multiple of anything in particular. |
| 0x018 | 8 | | Zero. |
| 0x020 | 32 | char[32] | Map name, NUL-terminated, zero-filled: the file name without `.map` (`bloodgulch`, `a10`, `ui`). |
| 0x040 | 32 | char[32] | Build string, NUL-terminated, zero-filled. `01.10.12.2276` on every disc map. |
| 0x060 | 2 | i16 | Map type: 0 campaign, 1 multiplayer, 2 user interface (`ui.map`). Equals the scenario tag's own type field. |
| 0x062 | 2 | | Zero. |
| 0x064 | 4 | u32 | Map checksum. An opaque value set when the map was built; saved games record it and refuse to load against a map whose value differs. Not a CRC32 of any obvious range (see Open questions); a reader stores it and never recomputes it. |
| 0x068 | 0x794 | | Zero on every disc map. |
| 0x7FC | 4 | u32 | FOOT_MAGIC `foot` |

A reader rejects the file unless both magics match, the version is 5,
the name contains a NUL within its 32 bytes, and
`0 <= tag data offset`, `0 <= tag data size`,
`tag data offset + tag data size <= decompressed size`,
`HEADER_SIZE <= tag data offset`.
The engine's own upper limit on the decompressed size is 0x11600000
bytes (a reader may use it as a sanity bound; the largest disc map,
`b40.map`, decompresses to 279 763 456 bytes = 0x10ACDA00). The tag
data size must also be at most TAG_CACHE_SIZE.

## Compression

- The body, from file offset 0x800 to the end of the file, is one zlib
  stream (RFC 1950 wrapper around RFC 1951 deflate, header bytes 78 9C
  on every disc map) followed by zero bytes.
- The compressed size is not stored. It is
  `file size - 0x800 - compressed padding`. Equivalently, inflate until
  the stream's end and check that exactly `compressed padding` bytes
  remain and that they are zero.
- The padding makes the file size a multiple of 2048: padding =
  `(2048 - ((0x800 + compressed size) mod 2048)) mod 2048`. Checked on
  all 24 maps (bloodgulch: compressed 22 567 375, padding 1585, file
  22 571 008).
- Inflating gives `decompressed size - 0x800` bytes. The decompressed
  image is the 0x800-byte header followed by those bytes.
- Only the body is compressed; the header is stored plain and is also
  the first 0x800 bytes of the decompressed image (it is *not* repeated
  inside the stream).
- On the Xbox the game inflated each map once into a cache partition and
  then read the decompressed image by offset. Our reader inflates the
  whole body into memory (the largest image is about 267 MiB) or streams
  it to a cache file; nothing is ever read from the compressed file by
  offset.

## The decompressed image

Regions in file order (checked on bloodgulch, hangemhigh, ui and a10):

| Region | Starts | Contents |
|---|---|---|
| Header | 0x000 | the 0x800-byte header |
| Structure BSPs | 0x800 | each BSP of the scenario, back to back, in the order of the scenario's BSP list; each size a multiple of 2048 |
| Bitmap pixels | right after the last BSP | the pixel data of every bitmap image, back to back, each start 128-byte aligned |
| Sound samples | after the pixels (next 512-byte boundary) | the sample data of every sound permutation, each start 512-byte aligned |
| Tag data | tag data offset | the tag header, tag index, paths, model vertex and index data, then every tag's structs; size = tag data size |
| Tail padding | tag data offset + size | zeros up to the decompressed size (0 to 0x1F8 bytes on the disc) |

Measured on bloodgulch: BSP 0x800..0x13D000, pixels 0x13D000..0x175A900,
samples 0x175AA00..0x233B540, tag data 0x233B600..0x2A46660,
decompressed size 0x2A46800.

Readers do not depend on this order; they follow the offsets in the
scenario's BSP list, the bitmap images and the sound permutations. The
order is useful for checks and for a reader that streams.

## Address space

- The tag data is loaded so that its first byte is at TAG_BASE =
  0x803A6000. This value is the same on all 24 disc maps. Every pointer
  stored in the tag data (block addresses, path pointers, tag struct
  addresses, data addresses, vertex buffer pointers) is an address in
  this space.
- Translation from a tag-data address A to a file offset:
  `offset = A - TAG_BASE + tag data offset`.
  A is valid only if `TAG_BASE <= A` and `A + length <= TAG_BASE +
  tag data size` for the bytes about to be read.
- Deriving the base instead of assuming it: the tag header's first field
  (the tag index address) always equals TAG_BASE + 0x24, because the
  index follows the header directly. A reader may compute
  `base = first field - 0x24` and check it equals 0x803A6000; the PC
  releases use a different base, which is why community tools call this
  value "magic".
- The BSP currently loaded shares the same 22 MiB window: it is copied
  so that it ends at TAG_CACHE_END = 0x819A6000, its own load address
  (given in the scenario) being `0x819A6000 - BSP size`. Addresses inside
  a BSP translate with that BSP's own base:
  `offset = A - BSP load address + BSP file offset`.
  The tag data and the loaded BSP must not overlap
  (`tag data size + BSP size <= TAG_CACHE_SIZE`), true for every BSP on
  the disc.
- Address 0 means "absent" everywhere (a block with count 0, a missing
  data, a null path).

## The tag header

At file offset `tag data offset` (address 0x803A6000), 0x24 bytes:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 4 | address | Tag index address (always 0x803A6024 on the disc) |
| 0x04 | 4 | tag id | The scenario tag's id (always 0xE1740000: the scenario is tag 0 on every disc map) |
| 0x08 | 4 | u32 | Tag checksum. Opaque; a networked host sends it so clients can tell their tags match. Store, never recompute. |
| 0x0C | 4 | i32 | Tag count (1 to 65535) |
| 0x10 | 4 | i32 | Model vertex buffer count |
| 0x14 | 4 | address | Model vertex buffer array: count records of 12 bytes |
| 0x18 | 4 | i32 | Model index buffer count (equal to the vertex buffer count on every disc map) |
| 0x1C | 4 | address | Model index buffer array: count records of 12 bytes |
| 0x20 | 4 | u32 | TAGS_MAGIC `tags` |

A *buffer record* is 12 bytes: a u32 the console overwrote at load time
(junk on disc), the address of the buffer's data (in tag data), and
another u32 of junk. Only the middle word means anything to us. How
model parts select a buffer, and the vertex formats, belong to the
models spec.

## The tag index

At the tag index address, `tag count` entries of 0x20 bytes:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 4 | class code | The tag's class |
| 0x04 | 4 | class code | Parent class, or NONE |
| 0x08 | 4 | class code | Grandparent class, or NONE |
| 0x0C | 4 | tag id | This tag's id |
| 0x10 | 4 | address | Path string (NUL-terminated) |
| 0x14 | 4 | address | The tag's main struct; 0 for every `sbsp` tag (a BSP's struct lives in its sub-file) |
| 0x18 | 8 | | Zero on every disc map |

**Tag ids.** A tag id is a u32: low 16 bits the *index* (the entry's
position in the index, 0-based), high 16 bits a *salt*. On every disc
map the index part equals the entry's position, tag 0's salt is 0xE174,
and the salt rises by 1 per entry with occasional steps of 2 (bloodgulch:
1803 steps of 1, 2 of 2). Readers never predict salts: to resolve an
id, take its low 16 bits as the index, check it is below the tag count,
and check the entry's stored id equals the whole id; otherwise the id is
invalid (treat as no tag). NONE (0xFFFFFFFF) is "no tag". Tag ids are
per map: the same tag has different ids in different maps.

**Paths.** Lower-case ASCII (no character above 0x7E or below 0x20 on
the disc), directories separated by backslash, no extension, no leading
separator, at most 122 characters on the disc (`weapons\pistol\pistol`).
A tag is identified by (class, path); the same path may appear with
different classes (`levels\test\bloodgulch\bloodgulch` is both the
`scnr` and the `sky ` tag), and no (class, path) pair repeats. On every
disc map the strings are stored right after the index, in index order,
packed with no gaps (each one starts right after the previous one's
NUL). Lookups by path are case-insensitive in the engine.

**Order.** The scenario is entry 0. The rest appear to be in the order
the build tool reached them (the globals and the interface roots of Tag
classes come after the scenario's own tags); nothing may depend on it.

## Structure BSP sub-files

The scenario tag has a block of structure BSP references at scenario
offset 0x5A4 (the scenario spec owns the rest of the scenario). Each
element is 0x20 bytes:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 4 | i32 | File offset of the BSP sub-file in the decompressed image |
| 0x04 | 4 | i32 | Its size in bytes |
| 0x08 | 4 | address | Its load address (= 0x819A6000 - size on the disc) |
| 0x0C | 4 | | Zero |
| 0x10 | 16 | tag reference | The `sbsp` tag |

A BSP sub-file begins with a 0x18-byte sub-header:

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 4 | address | The `sbsp` tag's main struct (in the BSP's own space) |
| 0x04 | 4 | i32 | Count of buffer list A |
| 0x08 | 4 | address | Buffer list A (12-byte buffer records; starts at load address + 0x18) |
| 0x0C | 4 | i32 | Count of buffer list B (equal to A's count on the disc) |
| 0x10 | 4 | address | Buffer list B (follows list A directly) |
| 0x14 | 4 | u32 | SBSP_MAGIC `sbsp` |

The decompilation names list A the BSP's render vertex buffers and list
B its lightmap vertex buffers (in one place) or index buffers (in
another); which is right is for the structure BSP spec. The rest of the
sub-file is the `sbsp` tag's structs, addressed in the BSP's space; its
tag references point into the tag index like any other.

`ui.map` has a stub BSP of 0x800 bytes with both counts 0.

## Where geometry, pixels and samples live

- **Model (object) geometry**: inside the tag data. The model vertex and
  index buffer arrays named in the tag header point at vertex and index
  data that sit in the tag data between the path strings and the first
  tag struct (bloodgulch: vertex buffer array 0x803C6374, vertex data
  from 0x803C7460, index buffer array 0x80648680, index data
  0x80649754..0x8068C318, first tag struct 0x8068C318). There is no
  separate model data section as in the PC releases.
- **Level (BSP) geometry**: inside each BSP sub-file, addressed in the
  BSP's own space.
- **Bitmap pixels**: outside the tag data. Each bitmap image element (in
  the `bitm` tag's image block) carries a pixel offset and a pixel size;
  on the disc the offset is an absolute file offset in the decompressed
  image, and the images tile the pixel region with only alignment gaps.
  The `bitm` tag's own "pixel data" data reference has size 0 and
  address 0 on the disc, and its file-offset word holds a leftover value
  from the source tag file (bloodgulch `sky\planets\threshold clear
  afternoon`: 0x68AEB). Do not add it. (The decompilation adds that word
  to each image's offset when a bitmap is first used; on the disc this
  would land the images on each other, so the disc's offsets are taken
  as absolute. See Open questions.) Pixel formats belong to the bitmaps
  spec.
- **Sound samples**: outside the tag data. Each sound permutation's
  samples data reference has address 0 and its file offset is an
  absolute offset in the decompressed image (512-byte aligned). Sample
  formats belong to the sounds spec.
- Everything else (strings, scripts, animation data, collision, mouth
  and subtitle data of sounds) is inside the tag data and addressed.

## Tag data conventions

A tag is a fixed-size main struct at the entry's address, plus whatever
its blocks and data references point to. Each class spec gives the
struct's size and its fields as `offset, type, name, unit, meaning`,
using the primitive names below. A struct never contains pointers other
than the block, tag reference and data reference fields below, the
buffer records above, and fields the class spec names as runtime-only.

### Primitive types

| Type (ours) | Size | Layout | Notes |
|---|---|---|---|
| i8 / u8 | 1 | | "char integer" |
| i16 / u16 | 2 | | "short integer" |
| i32 / u32 | 4 | | "long integer" |
| f32 | 4 | IEEE single | "real" |
| fraction | 4 | f32 | a real meant to lie in 0..1 |
| angle | 4 | f32 radians | |
| enum16 | 2 | i16 | an enumeration; values are listed per field; -1 sometimes means none |
| flags8 / flags16 / flags32 | 1 / 2 / 4 | bit field | bit 0 is the least significant bit; unlisted bits are zero |
| tag string | 32 | char[32] | NUL-terminated ASCII, zero-filled; up to 31 characters |
| class code | 4 | u32 | see Bytes |
| tag id | 4 | u32 | index + salt, see The tag index |
| point2d | 4 | i16 x, i16 y | |
| rectangle2d | 8 | i16 top, i16 left, i16 bottom, i16 right | top/left first |
| real_point2d | 8 | f32 x, y | |
| real_point3d | 12 | f32 x, y, z | |
| real_vector2d | 8 | f32 i, j | |
| real_vector3d | 12 | f32 i, j, k | |
| real_quaternion | 16 | f32 i, j, k, w | vector part first, w last |
| real_euler_angles2d | 8 | f32 yaw, pitch | radians |
| real_euler_angles3d | 12 | f32 yaw, pitch, roll | radians |
| real_plane2d | 12 | f32 i, j, d | normal then distance: points p on the plane satisfy n.p = d |
| real_plane3d | 16 | f32 i, j, k, d | same |
| real_rgb_color | 12 | f32 red, green, blue | 0..1 |
| real_argb_color | 16 | f32 alpha, red, green, blue | alpha first |
| real_hsv_color | 12 | f32 hue, saturation, value | 0..1 each |
| real_ahsv_color | 16 | f32 alpha, hue, saturation, value | |
| rgb_color | 4 | u32 0x??RRGGBB | bytes in file: B, G, R, then a byte to ignore |
| argb_color | 4 | u32 0xAARRGGBB | bytes in file: B, G, R, A |
| short_bounds | 4 | i16 lower, i16 upper | |
| real_bounds | 8 | f32 lower, f32 upper | |
| angle_bounds | 8 | f32 lower, f32 upper | radians |
| fraction_bounds | 8 | f32 lower, f32 upper | |
| real_rectangle2d | 16 | f32 x0, x1, y0, y1 | lower then upper per axis |
| real_rectangle3d | 24 | f32 x0, x1, y0, y1, z0, z1 | |
| index8 / index16 / index32 | 1 / 2 / 4 | signed | an element index into a named block; NONE (-1) means none |
| block flags8/16/32 | 1 / 2 / 4 | bit field | one bit per element of a named block |
| tag block | 12 | see below | |
| tag reference | 16 | see below | |
| data reference | 20 | see below | |
| padding | n | | bytes the format reserves; zero on disc, ignored |

### Tag block (12 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x0 | 4 | i32 | Element count (0 = empty) |
| 0x4 | 4 | address | First element; 0 when the count is 0 |
| 0x8 | 4 | u32 | Zero on the disc (a definition pointer in the editing tools) |

Elements are a C array: element i is at `address + i * element size`,
the element size being fixed per block (given by the class spec). A
block's elements are themselves structs that may contain blocks, tag
references and data references. A reader checks
`0 <= count` and that `count * element size` bytes at the address lie in
the tag data (or the BSP, for blocks inside a BSP), before touching
any element. Each block definition also has a maximum count; class specs
give it where it matters, and a reader treats a larger count as damage.

### Tag reference (16 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x0 | 4 | class code | The referenced tag's class |
| 0x4 | 4 | address | Its path string, or 0 |
| 0x8 | 4 | i32 | Path length; 0 on the disc (do not use) |
| 0xC | 4 | tag id | The referenced tag, or NONE |

- The tag id is what resolves the reference. The path pointer is the
  same address as the target's index entry path (shared, not a copy);
  readers may use the id alone.
- A null reference has id NONE and path 0, and keeps a class code (the
  field's expected class, which may be an abstract parent such as
  `shdr`). Bloodgulch has 4354 null references.
- On the disc a non-null reference's class always equals the target's
  own class (5142 of 5142 in bloodgulch), never a parent. A field that
  accepts several classes (any shader, any object) says so in its spec;
  a reader checks the target's class or one of its parents is accepted.

### Data reference (20 bytes)

| Offset | Size | Type | Meaning |
|---|---|---|---|
| 0x00 | 4 | i32 | Size in bytes |
| 0x04 | 4 | u32 | Flags; 0 in every field checked on the disc |
| 0x08 | 4 | i32 | File offset (meaning per field, below) |
| 0x0C | 4 | address | Address of the bytes, or 0 |
| 0x10 | 4 | u32 | Zero on the disc (a definition pointer in the tools) |

Three cases, decided by the field (class specs say which):

1. **In tag data (the default).** The address is nonzero and the size
   bytes are at that address. The file-offset word is a leftover from
   the source tag file and is ignored (bloodgulch: a sound's mouth data
   of 46 bytes at address 0x80A973C0 carries file offset 0x9318).
2. **External, by file offset.** Sound permutation samples: address 0,
   size bytes at the file offset in the decompressed image.
3. **Not in the cache.** Address 0 on a field that is not external:
   the data was stripped when the map was built, even when the size is
   nonzero (the `bitm` "import data" field has sizes such as 0x68A7F
   and no bytes). Treat as empty.

Size 0 means empty in all cases.

### Strings and text

- Short strings are 32-byte tag strings (above). Some structs use other
  fixed lengths; the class spec gives them.
- Unicode text (`ustr` tags and others) is UTF-16 little-endian inside
  data references; the class spec owns its layout.

### Enums, flags and indices

- Enums are i16 unless a spec says otherwise. Values out of range are
  damage; a reader clamps or rejects as the class spec says.
- Flags are named per bit; bit n has value `1 << n`.
- Index fields refer to an element of a named block of the same tag
  (or, where stated, of another tag); NONE (-1 at the field's width)
  means none. A reader checks every index against the block count.

### Runtime fields

Some struct fields are filled by the build tool after editing
(precomputed cosines, node indices, per-tick conversions) and some are
scratch space the engine writes while running. Class specs mark both.
On the disc the build-tool fields are filled and must be used as stored;
the engine's scratch fields hold whatever the build left (often zero, or
console fill patterns such as 0xCACACACA and 0x00AAFF55 in buffer
records) and are ignored.

## Tag classes

### Classes on the disc

Every class code found in the tag index of any of the 24 maps, with the
parent and grandparent codes the index stores for it, and its name.
Classes marked * appear only as parents, never as a tag's own class.

| Code | Name | Parent | Grandparent |
|---|---|---|---|
| `obje`* | object | | |
| `unit`* | unit | `obje` | |
| `item`* | item | `obje` | |
| `devi`* | device | `obje` | |
| `shdr`* | shader | | |
| `bipd` | biped | `unit` | `obje` |
| `vehi` | vehicle | `unit` | `obje` |
| `weap` | weapon | `item` | `obje` |
| `eqip` | equipment | `item` | `obje` |
| `garb` | garbage | `item` | `obje` |
| `mach` | device_machine | `devi` | `obje` |
| `ctrl` | device_control | `devi` | `obje` |
| `lifi` | device_light_fixture | `devi` | `obje` |
| `proj` | projectile | `obje` | |
| `scen` | scenery | `obje` | |
| `ssce` | sound_scenery | `obje` | |
| `senv` | shader_environment | `shdr` | |
| `soso` | shader_model | `shdr` | |
| `sotr` | shader_transparent_generic | `shdr` | |
| `schi` | shader_transparent_chicago | `shdr` | |
| `sgla` | shader_transparent_glass | `shdr` | |
| `smet` | shader_transparent_meter | `shdr` | |
| `spla` | shader_transparent_plasma | `shdr` | |
| `swat` | shader_transparent_water | `shdr` | |
| `scnr` | scenario | | |
| `sbsp` | scenario_structure_bsp | | |
| `sky ` | sky | | |
| `matg` | globals | | |
| `mode` | model | | |
| `coll` | model_collision_geometry | | |
| `antr` | model_animations | | |
| `phys` | physics | | |
| `pphy` | point_physics | | |
| `bitm` | bitmap | | |
| `snd!` | sound | | |
| `lsnd` | sound_looping | | |
| `snde` | sound_environment | | |
| `effe` | effect | | |
| `jpt!` | damage_effect | | |
| `part` | particle | | |
| `pctl` | particle_system | | |
| `cont` | contrail | | |
| `ligh` | light | | |
| `lens` | lens_flare | | |
| `glw!` | glow | | |
| `mgs2` | light_volume | | |
| `deca` | decal | | |
| `ant!` | antenna | | |
| `flag` | flag | | |
| `fog ` | fog | | |
| `wind` | wind | | |
| `foot` | material_effects | | |
| `dobc` | detail_object_collection | | |
| `itmc` | item_collection | | |
| `actr` | actor | | |
| `actv` | actor_variant | | |
| `udlg` | dialogue | | |
| `trak` | camera_track | | |
| `colo` | color_table | | |
| `font` | font | | |
| `str#` | string_list | | |
| `ustr` | unicode_string_list | | |
| `hmt ` | hud_message_text | | |
| `hudg` | hud_globals | | |
| `hud#` | hud_number | | |
| `grhi` | grenade_hud_interface | | |
| `unhi` | unit_hud_interface | | |
| `wphi` | weapon_hud_interface | | |
| `metr` | meter | | |
| `DeLa` | ui_widget_definition | | |
| `Soul` | ui_widget_collection | | |
| `vcky` | virtual_keyboard | | |
| `mply` | multiplayer_scenario_description | | |

So the object family is: object > unit > {biped, vehicle}; object > item
> {weapon, equipment, garbage}; object > device > {machine, control,
light fixture}; object > {projectile, scenery, sound scenery}. Shaders:
shader > the eight shader classes. A class with no parent stores NONE in
both parent slots; a class with one parent stores NONE in the second.
Two levels are all the index can hold, and all the game uses.

Classes the game knows that no disc map contains (for completeness;
inferred, not checked): `cdmg` continuous_damage_effect, `elec`
lightning, `rain` weather_particle_system, `plac` placeholder, `boom`
spheroid, `devc` input_device_defaults, `ngpr`
preferences_network_game, `tagc` tag_collection. The PC releases add
classes the Xbox does not have (for example `mod2`, the PC model format,
which replaces `mode`; and `scex`); a reader for this port refuses them.

### What a map holds, and what a multiplayer match reads

A map holds exactly the tags reachable by tag references from a small
set of roots; nothing else. Measured by following every tag reference
in the tag data and in the BSPs:

- Multiplayer maps: roots are the scenario (tag 0), `globals\globals`
  (`matg`), and three interface tags the engine loads by name:
  `ui\multiplayer_game_text` (`ustr`), `ui\shell\bitmaps\white` (`bitm`)
  and `ui\shell\multiplayer` (`Soul`). From these, every tag is reached
  (bloodgulch 1806 of 1806; hangemhigh 1681 of 1681). The scenario
  alone reaches 1420 (bloodgulch) and 1031 (hangemhigh); the globals
  bring in the rest of the shared tags (for example the frag grenade
  projectile and its effects, and the default HUD `ui\hud\default`,
  neither of which the scenario reaches).
- Campaign maps: the same with `ui\shell\solo` instead of
  `ui\shell\multiplayer` (a10: 3357 of 3357).
- `ui.map`: the scenario, the globals, `ui\shell\main_menu` (`Soul`) and
  a dozen string, sound and keyboard tags the menus load by name.

So a multiplayer match reads the scenario's closure plus the globals'
closure plus those three interface tags: in practice, every tag in the
file. The engine keeps all of a map's tag data loaded for the whole
match, and one BSP at a time.

### Counts per class

| Class | bloodgulch | hangemhigh | ui | a10 |
|---|---|---|---|---|
| `DeLa` | 30 | 30 | 485 | 83 |
| `Soul` | 1 | 1 | 1 | 1 |
| `actr` | 0 | 0 | 0 | 9 |
| `actv` | 3 | 3 | 0 | 16 |
| `ant!` | 3 | 3 | 0 | 3 |
| `antr` | 19 | 18 | 0 | 49 |
| `bipd` | 2 | 2 | 0 | 14 |
| `bitm` | 368 | 329 | 195 | 485 |
| `coll` | 36 | 25 | 0 | 47 |
| `colo` | 4 | 4 | 4 | 4 |
| `cont` | 8 | 8 | 0 | 6 |
| `deca` | 39 | 37 | 5 | 62 |
| `effe` | 318 | 318 | 29 | 255 |
| `eqip` | 14 | 14 | 2 | 6 |
| `flag` | 1 | 1 | 0 | 0 |
| `fog ` | 0 | 0 | 0 | 1 |
| `font` | 4 | 4 | 4 | 4 |
| `foot` | 5 | 5 | 0 | 6 |
| `garb` | 0 | 0 | 0 | 2 |
| `grhi` | 2 | 2 | 2 | 2 |
| `hmt ` | 1 | 1 | 1 | 2 |
| `hud#` | 1 | 1 | 1 | 1 |
| `hudg` | 1 | 1 | 1 | 1 |
| `itmc` | 12 | 13 | 0 | 0 |
| `jpt!` | 66 | 66 | 5 | 43 |
| `lens` | 22 | 20 | 5 | 39 |
| `lifi` | 0 | 0 | 0 | 0 |
| `ligh` | 32 | 29 | 7 | 48 |
| `lsnd` | 14 | 13 | 2 | 50 |
| `mach` | 0 | 0 | 0 | 19 |
| `matg` | 1 | 1 | 1 | 1 |
| `metr` | 2 | 2 | 2 | 2 |
| `mgs2` | 7 | 7 | 0 | 7 |
| `mode` | 63 | 46 | 5 | 76 |
| `mply` | 0 | 0 | 1 | 0 |
| `part` | 96 | 96 | 32 | 105 |
| `pctl` | 7 | 7 | 2 | 13 |
| `phys` | 3 | 3 | 0 | 5 |
| `pphy` | 30 | 30 | 13 | 30 |
| `proj` | 16 | 16 | 2 | 12 |
| `sbsp` | 1 | 1 | 1 | 9 |
| `scen` | 19 | 2 | 1 | 33 |
| `schi` | 20 | 19 | 4 | 27 |
| `scnr` | 1 | 1 | 1 | 1 |
| `senv` | 14 | 10 | 0 | 45 |
| `sgla` | 6 | 6 | 0 | 8 |
| `sky ` | 1 | 1 | 1 | 2 |
| `smet` | 7 | 7 | 0 | 4 |
| `snd!` | 315 | 314 | 29 | 1468 |
| `snde` | 1 | 1 | 0 | 2 |
| `soso` | 106 | 89 | 10 | 127 |
| `sotr` | 26 | 17 | 3 | 39 |
| `spla` | 1 | 1 | 0 | 2 |
| `ssce` | 1 | 0 | 0 | 7 |
| `str#` | 1 | 1 | 1 | 1 |
| `swat` | 0 | 0 | 0 | 0 |
| `trak` | 5 | 5 | 1 | 7 |
| `udlg` | 1 | 1 | 0 | 10 |
| `unhi` | 5 | 5 | 0 | 9 |
| `ustr` | 8 | 8 | 122 | 18 |
| `vcky` | 0 | 0 | 1 | 0 |
| `vehi` | 3 | 3 | 0 | 13 |
| `weap` | 15 | 15 | 0 | 6 |
| `wind` | 1 | 1 | 0 | 0 |
| `wphi` | 17 | 17 | 1 | 10 |
| **total** | **1806** | **1681** | **983** | **3357** |

Other multiplayer maps differ from these only in level-specific classes:
`lifi` (carousel 1, damnation 2, ratrace 1), `mach` (wizard 2), `swat`
(beavercreek 1, damnation 1), `fog ` (beavercreek, boardingaction,
chillout, damnation 1 each), `ssce` (beavercreek, bloodgulch, damnation
1 each), `sky ` (putput and ratrace have none), and the counts of
models, shaders, bitmaps, sounds, scenery, decals and lights.

## What is on the disc

All 24 maps: version 5, build `01.10.12.2276`, tag base 0x803A6000,
scenario at tag 0 with id 0xE1740000, zlib header 78 9C, file size a
multiple of 2048. Multiplayer scenarios are at
`levels\test\<map>\<map>`, campaign ones at `levels\<map>\<map>`, the
menu's at `levels\ui\ui`.

| Map | Type | File size | Decompressed | Tags | BSPs | Shown in game as |
|---|---|---|---|---|---|---|
| a10 | 0 | 173 903 872 | 274 734 080 | 3357 | 9 | Pillar of Autumn |
| a30 | 0 | 152 610 816 | 213 065 216 | 3215 | 2 | Halo |
| a50 | 0 | 176 224 256 | 250 138 624 | 3311 | 4 | Truth and Reconciliation |
| b30 | 0 | 162 877 440 | 224 887 296 | 3325 | 2 | The Silent Cartographer |
| b40 | 0 | 186 873 856 | 279 763 456 | 3647 | 13 | Assault on the Control Room |
| c10 | 0 | 166 926 336 | 252 559 360 | 3369 | 6 | 343 Guilty Spark |
| c20 | 0 | 76 980 224 | 142 009 856 | 1860 | 4 | The Library |
| c40 | 0 | 166 295 552 | 255 975 936 | 3208 | 13 | Two Betrayals |
| d20 | 0 | 131 012 608 | 205 501 952 | 2743 | 5 | Keyes |
| d40 | 0 | 170 405 888 | 274 594 816 | 2978 | 10 | The Maw |
| beavercreek | 1 | 22 122 496 | 43 565 056 | 1755 | 1 | Battle Creek |
| bloodgulch | 1 | 22 571 008 | 44 328 960 | 1806 | 1 | Blood Gulch |
| boardingaction | 1 | 22 075 392 | 45 346 304 | 1735 | 1 | Boarding Action |
| carousel | 1 | 20 215 808 | 42 367 488 | 1716 | 1 | Derelict |
| chillout | 1 | 20 336 640 | 41 967 104 | 1702 | 1 | Chill Out |
| damnation | 1 | 23 756 800 | 47 968 768 | 1758 | 1 | Damnation |
| hangemhigh | 1 | 21 338 112 | 43 700 224 | 1681 | 1 | Hang 'Em High |
| longest | 1 | 20 512 768 | 42 141 696 | 1696 | 1 | Longest |
| prisoner | 1 | 21 323 776 | 43 974 656 | 1685 | 1 | Prisoner |
| putput | 1 | 19 777 536 | 40 668 672 | 1675 | 1 | Chiron TL-34 |
| ratrace | 1 | 23 840 768 | 47 403 008 | 1710 | 1 | Rat Race |
| sidewinder | 1 | 24 823 808 | 48 048 128 | 1757 | 1 | Sidewinder |
| wizard | 1 | 19 687 424 | 40 681 984 | 1704 | 1 | Wizard |
| ui | 2 | 14 145 536 | 33 582 080 | 983 | 1 (stub) | main menu |

(The "BSPs" column is the number of `sbsp` tags, which equals the
scenario's BSP list length on the maps checked. The display names are
general knowledge, not read from the files.)

### The donor-map question

**Yes.** Every one of the 13 multiplayer maps contains all of the tags
below (13 of 13 for every row), and their main structs are identical
across maps apart from addresses and tag ids (compared on bloodgulch,
hangemhigh, sidewinder and chillout for the assault rifle, sniper rifle,
pistol bullet, rocket, the multiplayer Spartan and the overshield). One
multiplayer map file can donate every multiplayer weapon; `bloodgulch`
is a good default (it is also the largest by tag count).

| Class | Tags (paths), same in all 13 multiplayer maps |
|---|---|
| `weap` (15) | `weapons\assault rifle\assault rifle`, `weapons\pistol\pistol`, `weapons\shotgun\shotgun`, `weapons\sniper rifle\sniper rifle`, `weapons\rocket launcher\rocket launcher`, `weapons\plasma pistol\plasma pistol`, `weapons\plasma rifle\plasma rifle`, `weapons\needler\needler`, `weapons\flamethrower\flamethrower`, `weapons\gravity rifle\gravity rifle`, `weapons\ball\ball`, `weapons\flag\flag`, `vehicles\warthog\warthog gun`, `vehicles\scorpion\scorpion cannon`, `vehicles\ghost\ghost gun` |
| `proj` (16) | `weapons\assault rifle\bullet`, `weapons\pistol\bullet`, `weapons\shotgun\pellet`, `weapons\sniper rifle\sniper bullet`, `weapons\rocket launcher\rocket`, `weapons\plasma pistol\bolt`, `weapons\plasma rifle\bolt`, `weapons\plasma rifle\charged bolt`, `weapons\needler\needle`, `weapons\flamethrower\flame`, `weapons\frag grenade\frag grenade`, `weapons\plasma grenade\plasma grenade`, `vehicles\warthog\bullet`, `vehicles\scorpion\bullet`, `vehicles\scorpion\tank shell`, `vehicles\ghost\ghost bolt` |
| `eqip` (14) | `weapons\frag grenade\frag grenade`, `weapons\plasma grenade\plasma grenade`, `powerups\active camouflage`, `powerups\over shield`, `powerups\health pack`, `powerups\double speed`, `powerups\full-spectrum vision`, `powerups\assault rifle ammo\assault rifle ammo`, `powerups\pistol ammo\pistol ammo`, `powerups\shotgun ammo\shotgun ammo`, `powerups\sniper rifle ammo\sniper rifle ammo`, `powerups\rocket launcher ammo\rocket launcher ammo`, `powerups\needler ammo\needler ammo`, `powerups\flamethrower ammo\flamethrower ammo` |
| `bipd` (2) | `characters\cyborg_mp\cyborg_mp`, `characters\cyborg\cyborg` |
| `vehi` (3) | `vehicles\warthog\warthog`, `vehicles\scorpion\scorpion_mp`, `vehicles\ghost\ghost_mp` |

Notes for the plan: the Xbox maps have no rocket warthog, no fuel rod
gun and no banshee in multiplayer (those came with the PC release); the
flamethrower and the gravity rifle tags are present in every map even
though no stock map places them. `ui.map` holds no weapons (only the two
grenade projectiles and equipment). A donor map must be read whole: a
weapon's closure (its model, animations, sounds, effects, bitmaps,
projectiles, damage effects, HUD) spreads over hundreds of tags.

## Load order

1. Read the first 0x800 bytes; check the magics, version 5 and the name;
   read the fields.
2. Inflate the zlib stream from offset 0x800; check it ends exactly
   `compressed padding` bytes before the end of the file and that the
   output is `decompressed size - 0x800` bytes. Form the decompressed
   image (header + output).
3. Check the tag data range lies in the image; read the tag header at
   the tag data offset; check `tags`, check `first field - 0x24 =
   0x803A6000`, and that the tag index, buffer arrays and every entry's
   path and struct address lie in the tag data.
4. Read the tag index; resolve each path; check each entry's id has the
   entry's index in its low half; check the scenario id names entry 0
   and its class is `scnr`.
5. From the scenario, read the BSP list; for each BSP check its file
   range lies in the image, its sub-header ends in `sbsp`, its load
   address plus size equals 0x819A6000, and its tag reference names an
   `sbsp` tag.
6. Read tags on demand through the class specs, translating every
   address and checking every block, reference and index as above.

## Edge cases

- **Addresses out of range.** Any address, count or size that would read
  outside the tag data (or outside the BSP for BSP addresses) means a
  damaged map: refuse the map, or refuse that tag, never read wild.
- **Invalid tag id** in a reference (index beyond the count, or salt
  mismatch): treat as a null reference. The engine logs once and uses an
  empty tag (all blocks empty, no references).
- **`sbsp` index entries** have struct address 0; reading a BSP's struct
  goes through the scenario's BSP list, not the index.
- **The same path with different classes** is normal; always look up by
  (class, path).
- **Null reference class codes** can be abstract parents (`shdr`) or
  the field's single allowed class; do not resolve them.
- **Leftover file offsets** in data references: only the external fields
  (sound samples) use the file offset; all others ignore it.
- **Bitmap pixel data** reference of size 0: normal on the disc; pixels
  are found through each image's own offset.
- **Tail padding** after the tag data and **padding** after the zlib
  stream are zeros; a reader may check them and must not depend on
  their exact length beyond the rules above.
- **Decompressed size** is a multiple of 512 on the disc, but a reader
  must not require it (the synthetic test is not).

## Quirks

- The header's 0x0C field (compressed padding) is used by nothing in the
  game we know of; it is kept as a check.
- The tag reference's length word is 0 on the disc although the tools
  fill it in tag files; do not use it.
- Junk in runtime words (buffer records' first and third words,
  bitmap images' runtime fields, data references' file offsets) is
  build-tool leftover or console fill and must be ignored, never
  "fixed".
- The scenario's first three tag references (at scenario offsets 0x00,
  0x10, 0x20) are obsolete and null on the disc (`sbsp`, `sbsp`, `sky `
  class codes, NONE ids); the real sky and BSP lists are blocks.

## Test cases

### (a) Synthetic cache file

Build it in memory. All values little-endian. The tag data is 0x100
bytes; the decompressed image is 0x900 bytes.

**Header (0x800 bytes, zero except):**

| Offset | Bytes / value |
|---|---|
| 0x000 | 64 61 65 68 (`head`) |
| 0x004 | 5 |
| 0x008 | 0x900 |
| 0x00C | P (computed below) |
| 0x010 | 0x800 |
| 0x014 | 0x100 |
| 0x020 | `tiny` and NULs |
| 0x040 | `01.10.12.2276` and NULs |
| 0x060 | i16 1 |
| 0x064 | 0xDEADBEEF |
| 0x7FC | 74 6F 6F 66 (`foot`) |

**Tag data (address 0x803A6000 = image offset 0x800), zero except:**

| Rel. offset | Contents |
|---|---|
| 0x00 | 0x803A6024 (tag index address) |
| 0x04 | 0xE1740000 (scenario id) |
| 0x08 | 0x12345678 (tag checksum) |
| 0x0C | 2 (tag count) |
| 0x10..0x1F | 0, 0, 0, 0 (no model buffers) |
| 0x20 | 0x74616773 (`tags`) |
| 0x24 | entry 0: 0x73636E72 (`scnr`), 0xFFFFFFFF, 0xFFFFFFFF, 0xE1740000, 0x803A6064, 0x803A6090, 0, 0 |
| 0x44 | entry 1: 0x77656170 (`weap`), 0x6974656D (`item`), 0x6F626A65 (`obje`), 0xE1750001, 0x803A6073, 0x803A60C0, 0, 0 |
| 0x64 | `scenarios\tiny` and NUL (15 bytes, to 0x72) |
| 0x73 | `weapons\gun` and NUL (12 bytes, to 0x7E) |
| 0x90 | scenario struct, block field: 2, 0x803A60D8, 0 |
| 0x9C | scenario struct, tag reference: 0x77656170, 0x803A6073, 0, 0xE1750001 |
| 0xAC | scenario struct, data reference: 8, 0, 0, 0x803A60E8, 0 |
| 0xC0 | weapon struct: f32 1.5 (0x3FC00000) |
| 0xC4 | weapon struct, null tag reference: 0x70726F6A (`proj`), 0, 0, 0xFFFFFFFF |
| 0xD8 | block element 0: i16 1, i16 -1, f32 2.0 (0x40000000) |
| 0xE0 | block element 1: i16 2, i16 0, f32 -0.5 (0xBF000000) |
| 0xE8 | data bytes 01 02 03 04 05 06 07 08 |

**File:** compress the 0x100 tag-data bytes as one zlib stream (any
level) of C bytes; P = (2048 - ((0x800 + C) mod 2048)) mod 2048; the
file is header + stream + P zero bytes; its size is a multiple of 2048.

| Setup | Input | Expected |
|---|---|---|
| the file above | parse | name `tiny`, build `01.10.12.2276`, type multiplayer, checksum 0xDEADBEEF, decompressed size 0x900, tag count 2, tag checksum 0x12345678 |
| same | tag 0 | class `scnr`, parents none, id 0xE1740000, path `scenarios\tiny`, address 0x803A6090, is the scenario |
| same | tag 1 | class `weap`, parents `item` then `obje`, id 0xE1750001, path `weapons\gun`; is-a `item`, is-a `obje`, not is-a `unit` |
| same | find (`weap`, `WEAPONS\GUN`) | tag 1 (case-insensitive) |
| same | translate 0x803A60E8 | image offset 0x8E8 |
| same | scenario block at +0x00 with element size 8 | count 2; element 0 = (1, -1, 2.0); element 1 = (2, 0, -0.5) |
| same | scenario tag reference at +0x0C | class `weap`, resolves to tag 1, path `weapons\gun` |
| same | scenario data reference at +0x1C | 8 bytes 01..08 |
| same | weapon struct f32 at +0 | 1.5 |
| same | weapon null reference at +4 | null, class `proj`, resolves to nothing |
| same, but the decompressed image given directly | parse image | same results |
| first byte changed | parse | rejected (magic) |
| 0x7FC changed | parse | rejected (footer) |
| version 7 | parse | rejected (version) |
| P one too small (one padding byte missing) | parse | rejected (padding) |
| stream truncated by 1 byte | parse | rejected (inflate) |
| tag data size 0x200 | parse | rejected (tag data beyond image) |
| entry 1 id 0xE1760001 (salt mismatch) | resolve the scenario's tag reference | invalid, treated as null |
| scenario block count 6 (elements would end at 0x803A6108, past the tag data end 0x803A6100) | read the block | rejected (outside tag data) |
| scenario block count -1 | read the block | rejected (negative count) |
| tag index address 0x803A6028 | parse | rejected (base check) |
| `tags` magic changed | parse | rejected |

### (b) Disc-backed (slow tests; need the user's own disc files)

**bloodgulch.map**

| Check | Expected |
|---|---|
| file size | 22 571 008 |
| header: version, decompressed size, padding | 5, 0x02A46800 (44 328 960), 1585 |
| header: tag data offset, size | 0x0233B600, 0x0070B060 |
| header: name, build, type, checksum | `bloodgulch`, `01.10.12.2276`, 1, 0x035E2DA3 |
| compressed stream length | 22 567 375 |
| tag header | index 0x803A6024, scenario 0xE1740000, checksum 0xEF031F2E, 1806 tags, 359 vertex buffers at 0x803C6374, 359 index buffers at 0x80648680 |
| tags 0..4 (class, path) | `scnr` `levels\test\bloodgulch\bloodgulch`; `sky ` `levels\test\bloodgulch\bloodgulch`; `mode` `sky\mp clear afternoon\mp clear afternoon`; `sotr` `sky\clear afternoon\shaders\threshold clear afternoon`; `bitm` `sky\planets\threshold clear afternoon` |
| tags 0..4 ids | 0xE1740000, 0xE1750001, 0xE1760002, 0xE1770003, 0xE1780004 |
| tag 0 path address, struct address | 0x803B41E4, 0x8068C318 |
| tag 3 parents | `shdr`, none |
| last tag (1805) | `snde` `sound\sound environments\mountains`, id 0xE883070D |
| globals | tag 1378 `matg` `globals\globals`, id 0xE6D60562 |
| `weapons\assault rifle\assault rifle` | tag 237, id 0xE26100ED |
| counts per class | the bloodgulch column above (e.g. `bitm` 368, `snd!` 315, `weap` 15) |
| scenario sky block (scenario +0x30) | 1 element: `sky ` tag 1, path address 0x803B4206 (= tag 1's), length 0 |
| scenario BSP list (scenario +0x5A4) | 1 element: file offset 0x800, size 0x13C800, load address 0x81869800, tag `levels\test\bloodgulch\bloodgulch` id 0xE85A06E4 |
| BSP sub-header | 0x818A2120, 0x4F, 0x81869818, 0x4F, 0x81869BCC, `sbsp` |
| scenario block counts at +0x354, +0x378, +0x384 | 71, 191, 35 |
| first element at +0x354 (0x34-byte elements) | position (98.4934, -157.6390, 1.70473), facing 1.328281 rad |
| first bitmap image offset | 0x13D000 (= BSP end) |
| last sound permutation in the tag order | 2880 bytes at file offset 0x0233AA00 |
| `characters\cyborg_mp\cyborg_mp` | parents `unit`, `obje` |

**hangemhigh.map**

| Check | Expected |
|---|---|
| file size, decompressed size, padding | 21 338 112, 0x029AD000 (43 700 224), 1029 |
| tag data offset, size | 0x022F6200, 0x006B6C08 |
| name, type, checksum | `hangemhigh`, 1, 0x8313BD85 |
| tag header | 1681 tags, checksum 0x10B5ACA4, 323 vertex buffers at 0x803C3B2C, 323 index buffers at 0x8062CE40 |
| tags 0..4 | as bloodgulch with `hangemhigh` in tags 0 and 1; tags 2..4 the same paths |
| scenario struct address | 0x8066DCF8 |
| last tag (1680) | `snde` `sound\sound environments\concert_hall` |
| BSP list | 1 element: file offset 0x800, size 0x90000, load address 0x81916000 |
| scenario block counts at +0x354, +0x378, +0x384 | 67, 92, 45 |
| counts per class | the hangemhigh column above |

**ui.map**: decompressed size 0x02006C00, padding 27, tag data offset
0x01E75C00, size 0x00190F04, type 2, checksum 0xCDC1A39A, 983 tags, tag
checksum 0x6B0B0215, 34 vertex and 34 index buffers, scenario
`levels\ui\ui`; BSP list 1 element of size 0x800 at file offset 0x800,
load address 0x819A5800, sub-header counts 0 and 0.

**a10.map**: decompressed size 0x10601C00, padding 666, tag data offset
0x0F881200, size 0x00D808B0, type 0, checksum 0xE6AC8E18, 3357 tags, tag
checksum 0x99A1B67E, 623 vertex and 623 index buffers; 9 BSPs, the first
`levels\a10\a10a` at file offset 0x800, size 0x5F1000, load address
0x813B5000, the last `levels\a10\x10hangar` at 0x31F9800, size
0x2A2000; every load address + size = 0x819A6000.

**All 24 maps**: the per-map table above; every multiplayer map holds
the donor table's tags.

## Confidence per section

| Section | Confidence |
|---|---|
| Header layout, magics, version, sizes, padding rule, name, build, type | checked against all 24 maps |
| Header checksum and tag checksum meaning | reference only (what they are used for); their computation unknown |
| Compression | checked against all 24 maps (inflated and compared lengths) |
| Decompressed image regions | checked on bloodgulch, hangemhigh, ui, a10 |
| Address base and translation | checked against all 24 maps (base), and by every read in the scripts |
| Tag header, tag index, ids, paths | checked against all 24 maps |
| BSP list element and sub-header | checked on bloodgulch, hangemhigh, ui, a10 (all BSPs end at 0x819A6000); names of the two buffer lists from the reference only |
| Model geometry location | checked (buffer arrays and data addresses lie in tag data) on bloodgulch and hangemhigh |
| Bitmap pixel and sound sample location | checked on bloodgulch, hangemhigh, ui, a10 (offsets tile their regions) |
| Block, tag reference, data reference layouts | checked on the disc (scenario, bitmap, sound, colour table fields; all tag references in bloodgulch) |
| Primitive types | sizes and orders from the reference; argb order (alpha first) confirmed on the disc's colour tables; radians confirmed on the disc |
| Class codes and parents | checked against all 24 maps; names from general knowledge of the game's tag groups (they match the reference's group names where it has them) |
| Classes absent from the disc | inferred |
| Roots and reachability | checked on bloodgulch, hangemhigh, ui, a10 (by matching reference-shaped words; see Open questions) |
| Counts per class, per-map table, donor table | checked against the disc (all 13 multiplayer maps for the donor table) |
| World unit | inferred from the engine's gravity constant (reference) and checked against biped sizes on the disc |
| Axes, angle convention | reference only (engine maths); radians checked on the disc |
| Tick rate | reference only |
| Per-tick storage of speeds | checked on the disc for projectile velocities and biped jump velocity |

## Open questions

1. **Header checksum (0x64) and tag checksum.** Neither is CRC32 of the
   compressed body, the decompressed body, the tag data, or BSP + tag
   data (tried both plain and inverted). We only store and compare them;
   if a feature ever needs to produce maps the real game accepts, find
   the algorithm (the PC tools' checksum would be the next guess).
2. **Bitmap pixel offsets.** The reference adds the bitmap tag's pixel
   data file-offset word to each image's offset when the image is first
   used; on the disc that word is a leftover and the image offsets are
   already absolute (they tile the pixel region exactly). Possibly the
   reference's build wrote maps differently, or the word is cleared
   somewhere we did not find. Check by decoding one known image (for
   example a HUD bitmap) at both offsets; the bitmaps spec should do so.
3. **BSP buffer lists A and B.** Render vertex buffers and lightmap
   vertex buffers, or vertex and index buffers? Resolve in the structure
   BSP spec by decoding one buffer of each list.
4. **Reachability was measured heuristically**: by finding 16-byte
   words shaped like tag references (a class code, then the target's
   path address, then the target's id with a matching class) anywhere in
   the tag data and BSPs. It covers every tag, so the conclusion holds,
   but individual edges are not attributed to named fields.
5. **Engine-loaded roots.** The three interface tags in multiplayer maps
   are loaded by name by the engine. The HUD and menu specs should list
   every tag the engine loads by name rather than by reference.
6. **Salt steps of 2.** Two places in bloodgulch's index where the salt
   skips a value; harmless, cause unknown.
7. **Other regions' discs.** Only the NTSC disc (build 01.10.12.2276) was
   checked. The reference knows a PAL build (01.01.14.2342) and another
   NTSC build (01.08.15.1749); tag ids differ between builds, and
   localised discs keep maps in language-specific folders. Supporting
   them needs their files.
