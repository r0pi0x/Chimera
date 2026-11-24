# GoldSrc: sprites, SPR version 2

Source basis: public format documentation and community write-ups of
the Half-Life sprite format; all 454 `.spr` files under the HLDS
install's `cstrike/sprites/` and `valve/sprites/` (subfolders included),
and `cstrike/sprites/hud.txt` and `weapon_*.txt`, read with throwaway
scripts outside the repository.
Status: draft

## Summary

A sprite is a 40-byte header, one 256-colour palette, and a list of
paletted frames (or, in principle, frame groups). The header says how the
sprite faces the camera (its orientation type) and how its pixels
blend (its texture format). Muzzle flashes, smoke, puffs, glows and the
HUD's image atlases (`640hud*.spr`) are sprites.

## Units and conventions

As in [bsp.md](bsp.md). Frame origins and sizes are in pixels, and one
pixel is one world unit at scale 1.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| magic | `IDSP` | 4 bytes | bytes 0-3 |
| version | 2 | i32 | every installed sprite |
| header_size | 40 | bytes | before the palette |
| palette_colours | 256 | | the u16 count is 256 in every file |
| frame_header_size | 16 | bytes | origin x, origin y, width, height |

## Layout

### Header (40 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | 4 bytes | `IDSP` |
| 4 | i32 | version (2) |
| 8 | i32 | orientation type (below) |
| 12 | i32 | texture format (below) |
| 16 | f32 | bounding radius: √((w/2)² + (h/2)²) of the largest frame |
| 20 | i32 | largest frame width |
| 24 | i32 | largest frame height |
| 28 | i32 | frame count |
| 32 | f32 | beam length (0 in every file) |
| 36 | i32 | sync type: 0 = all copies animate in step, 1 = each starts at a random point |

Then a u16 colour count (256) and 256 RGB colours (768 bytes): the
palette starts at 42 and the first frame at 810.

### Frames

Each of the frame count entries starts with an i32 kind:

- 0: a single frame: origin x (i32), origin y (i32), width (i32), height
  (i32), then width × height palette indices row by row from the top.
- nonzero: a frame group: a count n (i32), n intervals (f32 each,
  seconds, cumulative), then n single frames as above (without kind
  words). **No installed sprite has a group**; support them for
  completeness.

The origin is the frame's top-left corner relative to the sprite's
centre, with y growing upward: a 48 × 48 frame has origin (−24, 24).
Every installed sprite parses exactly to its end.

Animated sprites play their frames at a rate the entity or effect
chooses (10 frames a second by default for `env_sprite`; open question 1).

### Orientation types

| Value | Name (ours) | Facing | Count in install |
|---|---|---|---|
| 0 | upright facing viewpoint | turns about Z to face the viewer's position; stays vertical | 5 (`ikgrass1-5`) |
| 1 | facing viewpoint | faces the viewer's position fully | 0 |
| 2 | parallel to the view | lies in the camera's view plane (the common billboard) | 443 |
| 3 | oriented | fixed in the world by the entity's angles | 1 (`valve/sprites/oriented.spr`) |
| 4 | parallel, entity roll | parallel to the view, rotated by the entity's roll | 5 (`explode2`, `fire`, `smoke`, `steam1`, `vp_parallel_oriented`) |

### Texture formats

| Value | Name (ours) | Drawing | Count |
|---|---|---|---|
| 0 | normal | opaque, colour = palette[index] | 1 (`lgtning.spr`) |
| 1 | additive | colour = palette[index], added to the frame (black adds nothing) | 394 |
| 2 | index alpha | colour = the palette's **last** entry, alpha = index / 255 | 18 |
| 3 | alpha test | palette[index], index 255 fully transparent | 41 |

Conversion to RGBA: normal → (palette[i], 255); additive → (palette[i],
255) drawn with additive blending; index alpha → (palette[255], i);
alpha test → (palette[i], 0 if i = 255 else 255). An entity's
`rendermode`, `renderamt` and `rendercolor` may override the blend
(open question 2).

## Worked examples

| File | Type | Format | Radius | Largest | Frames | Sync | Frame 0 |
|---|---|---|---|---|---|---|---|
| `cstrike/sprites/muzzleflash1.spr` | 2 | 1 additive | 33.941 | 48 × 48 | 1 | 1 | origin (−24, 24), 48 × 48 |
| `cstrike/sprites/rifle_smoke1.spr` | 2 | 1 additive | 80.0 | 128 × 96 | 30 | 1 | origin (−64, 48), 128 × 96 |
| `cstrike/sprites/wall_puff1.spr` | 2 | 1 additive | 50.912 | 72 × 72 | 30 | 1 | origin (−36, 36) |
| `cstrike/sprites/black_smoke1.spr` | 2 | 2 index alpha | 50.912 | 72 × 72 | 30 | 1 | palette a grey ramp ((0,0,0) at 0, (128,128,128) at 128), last entry (255, 255, 255); frame 0 uses 27 indices from 0 to 255 |
| `cstrike/sprites/ch_sniper.spr` | 2 | 3 alpha test | 181.019 | 256 × 256 | 1 | 1 | index 255 (colour (0, 216, 255)) on 64 429 of 65 536 pixels: transparent |
| `cstrike/sprites/ikgrass1.spr` | 0 | 3 alpha test | 71.554 | 64 × 128 | 1 | 1 | origin (−32, 64) |
| `cstrike/sprites/640hud7.spr` | 2 | 1 | 181.019 | 256 × 256 | 1 | 1 | origin (−128, 128): a HUD atlas |

Radius check: 48 × 48 → √(24² + 24²) = 33.941; 64 × 128 → √(32² + 64²)
= 71.554.

## The HUD's sprite lists

`sprites/hud.txt` and `sprites/weapon_<name>.txt` name rectangles in
sprite atlases. Text, whitespace-separated: the first line is the entry
count; each entry is a name, a screen width class (320 or 640: the 320
entries serve screens narrower than 640 pixels, the 640 entries every
wider one), the sprite's file name without folder or
extension (`640hud7` → `sprites/640hud7.spr`), and x, y, width, height
in pixels of frame 0. Example, `weapon_ak47.txt`: 10 entries; at 640,
`weapon` is `640hud10` (0, 0, 170, 45), `weapon_s` `640hud11` (0, 0, 170,
45), `ammo` `640hud7` (72, 72, 24, 24), `crosshair` `crosshairs` (24, 0,
24, 24). The HUD itself is `specs/cs16/view_and_hud.md`'s business (M3).

## Edge cases

- A frame smaller than the largest is placed by its own origin.
- Group intervals are cumulative end times; a group frame is picked by
  time modulo the last interval.
- Sprites may be scaled by the entity (`scale` key); radius scales with
  it.

## Test cases

| Setup | Input | Expected |
|---|---|---|
| muzzleflash1.spr | header | `IDSP`, 2, type 2, format 1, 48 × 48, 1 frame; palette at 42; frame at 810 |
| rifle_smoke1.spr | frames | 30 frames of 128 × 96, origin (−64, 48); file ends after the 30th |
| black_smoke1.spr | to RGBA | every pixel's colour (255, 255, 255), alpha = its index |
| ch_sniper.spr | to RGBA | 64 429 transparent pixels |
| every installed sprite | parse | version 2, colour count 256, no groups, no bytes left over |
| weapon_ak47.txt | entry `ammo` at 640 | `640hud7`, (72, 72, 24, 24) |

## Open questions

1. The frame rate of animated effect sprites (smoke, puffs) is set by
   the effect that spawns them, not the file: `specs/cs16/` measures them.
2. How each render mode combines with each texture format in the
   original renderer (e.g. an additive sprite drawn with
   `rendermode 5`, glow sprites' fade by distance) is to be matched by
   captures.
