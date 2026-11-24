# GoldSrc: WAD3 texture archives

Source basis: public format documentation and community write-ups of
the Half-Life WAD format; every WAD in the HLDS install's `cstrike/` (23
files) and `valve/` (8 files: `halflife.wad`, `liquids.wad`,
`decals.wad`, `xeno.wad`, `gfx.wad`, `fonts.wad`, `cached.wad`,
`spraypaint.wad`), read with throwaway scripts outside the repository.
Status: draft

## Summary

A WAD3 file is a 12-byte header, the entries' data packed one after
another, and a directory of 32-byte records at the end. Maps find their
non-embedded textures in WADs by name; `decals.wad` holds the decals,
`gfx.wad`, `cached.wad` and `fonts.wad` hold menu pictures and fonts.
Every image carries its own 256-colour palette; there is no shared
palette.

## Units and conventions

As in [bsp.md](bsp.md): little-endian, fixed-size NUL-padded names.
Names compare case-insensitively.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| magic | `WAD3` | 4 bytes | bytes 0-3 |
| header_size | 12 | bytes | magic, entry count, directory offset |
| dir_entry_size | 32 | bytes | one directory record |
| type_qpic | 0x42 (66) | u8 | a plain picture |
| type_miptex | 0x43 (67) | u8 | a mip-mapped texture |
| type_font | 0x46 (70) | u8 | a font sheet |
| mip_levels | 4 | | full size, ½, ¼, ⅛ |
| palette_colours | 256 | | RGB, 3 bytes each |
| transparent_index | 255 | | for `{` textures |

## Layout

### Header (12 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 4 bytes | `WAD3` |
| 4 | i32 | entry count |
| 8 | i32 | directory offset |

### Directory record (32 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | i32 | data offset (from file start) |
| 4 | i32 | size on disk |
| 8 | i32 | size in memory (equal to size on disk in every file) |
| 12 | u8 | type (0x42, 0x43, 0x46) |
| 13 | u8 | compression (0 in every file; nothing else is defined) |
| 14 | 2 bytes | padding |
| 16 | 16 bytes | name, NUL-terminated; 10 records have leftover bytes after the NUL (ignore them) |

In every installed WAD the first entry starts at 12 and the directory
starts where the last entry ends and runs to the end of the file.

Worked example: `cstrike/cstrike.wad`: 2 681 364 bytes, 123 entries,
directory at 2 677 428 (ending at 2 677 428 + 123 × 32 = 2 681 364), all
mip textures. `valve/halflife.wad`: 3 116 entries, all mip textures,
directory at 37 814 384. `valve/gfx.wad`: 7 entries, five pictures
(`LAMBDA`, `LAMBDA64`, `LAMBDA32`, `PAUSED`, `BACKTILE`) and two fonts
(`CONCHARS`, `CREDITSFONT`).

### Mip textures (type 0x43)

All offsets are relative to the start of the entry.

| Offset | Type | Field |
|---|---|---|
| 0 | 16 bytes | name (equals the directory name, ignoring case, in all 5 413 textures) |
| 16 | u32 | width |
| 20 | u32 | height |
| 24 | 4 × u32 | offset of mip level 0, 1, 2, 3 |
| 40 | | level 0: width × height palette indices, row by row from the top |
| | | level 1: (width / 2) × (height / 2), then level 2 (÷4), level 3 (÷8) |
| after level 3 | u16 | colour count: 256 in every texture |
| +2 | 768 bytes | 256 RGB colours |
| +770 | | padding: 2 bytes in 5 394 of 5 413 textures (making the entry a multiple of 4); a few entries carry more trailing bytes (up to 980), ignored |

The mip offsets are always 40, 40 + w·h, 40 + w·h + w·h/4 and 40 + w·h +
w·h/4 + w·h/16. Widths and heights are multiples of 16 in every
installed texture (16 to 512; the most common is 128 × 128, then 64 ×
64 and 256 × 256). An entry's size is 40 + w·h × 85/64 + 2 + 768 +
padding.

Worked example: `+0CSTRIKE_IE1LT` in `cstrike/cstrike.wad` (the first
entry): 3 532 bytes at offset 12; 64 × 32; mip offsets 40, 2 088, 2 600,
2 728; the colour count at entry offset 2 760 is 256; colour 0 is (58,
55, 57) and colour 255 is (51, 51, 51). Ten of its level-0 pixels use
index 255, and it is shown opaque: index 255 is transparent only in `{`
textures.

### Converting to RGBA

- Ordinary texture: RGB = palette[index], alpha 255.
- `{` texture: index 255 is fully transparent (alpha 0, and its colour
  should not bleed: use black or the neighbours' colour for filtering);
  every other index is opaque.
- The smaller mip levels may be used as they are or regenerated; the
  original uses the stored ones.
- Prefix meanings (`!`, `+n`, `-n`, `sky`, tool textures) are the map's
  business ([bsp.md](bsp.md) "Texture name prefixes").

### Decals (`decals.wad`)

Decals are mip textures whose names mostly begin with `{` (blood,
scorches, bullet holes) and whose **palette index is a coverage value**:
index 0 is fully transparent, 255 fully covered, and the decal's colour
is the palette's last entry (index 255). Their palettes are a grey ramp
from (255, 255, 255) at index 0 downward, with the tint as the last
entry.

Worked example: `{247` in `valve/decals.wad` (the second entry): 64 ×
64, 6 252 bytes; palette entries 0, 1, 128, 254 are (255, 255, 255),
(254, 254, 254), (126, 126, 126), (0, 0, 0), and entry 255, the tint,
is (44, 0, 0) (dark red); its corner pixel is index 0 (clear); it uses
36 distinct indices. RGBA = (tint, index).

`cstrike/decals.wad` (225 entries) shadows `valve/decals.wad` (222) by
the mount's order (mount.md).

### Pictures (type 0x42)

| Offset | Type | Field |
|---|---|---|
| 0 | u32 | width |
| 4 | u32 | height |
| 8 | | width × height indices |
| then | u16 | colour count (256) |
| then | 768 bytes | palette |
| then | | 2 bytes padding |

Examples: `LAMBDA` 128 × 128 (17 164 bytes), `CONBACK` and `LOADING` in
`cached.wad` 320 × 200 (64 780 bytes). Index 255 is transparent in
pictures drawn as overlays (to confirm per use; the party does not need
them).

### Fonts (type 0x46)

| Offset | Type | Field |
|---|---|---|
| 0 | u32 | a nominal width (not the sheet's width; varies) |
| 4 | u32 | sheet height in pixels |
| 8 | u32 | row count |
| 12 | u32 | row height in pixels |
| 16 | 256 × (u16, u16) | per character: start offset into the sheet (y × 256 + x), width in pixels |
| 1040 | | the sheet: 256 × height indices (always 256 wide) |
| then | u16 | colour count |
| then | 768 bytes | palette |

Examples: `CONCHARS` (gfx.wad): nominal 12, height 128, 6 rows of 12; `A`
at offset 196 (x 196, row 0), 7 wide; colour count 256 and 2 bytes of
padding. `FONT0` (fonts.wad): nominal 16, height 128, 6 rows of 14; `A`
at 7 184 (row starting at y 28, x 16), 9 wide; here the colour count field
reads 768 and 64 extra bytes follow the palette (treat the palette as
always 256 colours and ignore the rest). Index 255 is transparent in
font sheets.

## Edge cases

- Lookups by name are case-insensitive (`halflife.wad` names are upper
  case, maps reference them in any case).
- The same name may exist in several WADs; see mount.md for which one
  wins.
- Names are at most 15 characters plus the NUL.
- Compression other than 0 is not defined; reject such entries.

## Test cases

| Setup | Input | Expected |
|---|---|---|
| `cstrike/cstrike.wad` | header | `WAD3`, 123 entries, directory at 2 677 428 |
| `cstrike/cstrike.wad` | entry `+0CSTRIKE_IE1LT` | type 0x43, offset 12, size 3 532, 64 × 32, mips at 40 / 2 088 / 2 600 / 2 728, colour 0 = (58, 55, 57) |
| same | to RGBA | 2 048 opaque pixels (no `{` prefix, index 255 stays opaque) |
| `valve/decals.wad` | entry `{247` | 64 × 64, tint (44, 0, 0), pixel (0, 0) alpha 0 |
| `valve/gfx.wad` | entry `CONCHARS` | type 0x46, height 128, 6 rows of 12, `A` at 196 width 7 |
| every installed WAD | every mip texture | mip offsets as the formula, colour count 256 |
| `valve/halflife.wad` | look up `!toxicgrn` | finds `!TOXICGRN`, 6 252 bytes |

## Open questions

1. Whether index 255 is transparent in `qpic` pictures is per use in the
   original's menus; not needed by the party.
2. `FONT0`/`FONT1`'s colour count of 768 and the trailing 64 bytes: a
   tool difference; harmless if the palette is read as 256 colours.
