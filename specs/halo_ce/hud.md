# Halo: Combat Evolved: the in-game HUD

Source basis: the game's own map files from the original Xbox disc
(`bloodgulch.map` for every HUD tag, bitmap and string a multiplayer
player on foot sees, read with throwaway scripts; the other twelve
multiplayer maps and `ui.map` for tag counts only), and a decompilation
of an Xbox build of the game's engine (the HUD drawing, the unit and
weapon interface updates, the motion sensor and its rasteriser, the
messaging, waypoint and sound code, the damage-indicator feed), used for
meaning only. The decompilation carries additions of its own for the
desktop ports (wide screens, extra grenade types, custom maps, text chat,
player names); those are ignored here and never described as the game's
behaviour. The decompilation describes a different Xbox build (PAL,
01.01.14.2342) from the disc's (NTSC, 01.10.12.2276); where the disc
differs it wins and the difference is noted.
Status: draft

Sections: Summary; Units and conventions; Constants; The HUD tags (the
shared blocks; the unit HUD interface; the weapon HUD interface; the
grenade HUD interface; the HUD globals; the number, message-text and
string tags); Behaviour (canvas and placement; which HUD is drawn and in
what order; colours, flashing and blending; meters; shield and health;
the weapon HUD; crosshairs; scopes and screen effects; grenades; the
motion tracker; damage indicators; messages and prompts; waypoints and
friend markers; what hides or changes the HUD; split screen; widescreen;
on-gun displays); Per-tick order; Edge cases; Quirks; The disc's values;
Test cases; Verification performed; Confidence per section; Open
questions.

---

## Summary

- The HUD is a handful of tags, all authored in a 640 by 480 pixel
  design canvas: a unit HUD interface (`unhi`: the shield and health
  panels, the motion sensor, the flashlight meter, the warning sounds),
  one weapon HUD interface (`wphi`) per weapon chained to shared parents
  through a parent reference (ammunition plate, numbers, meters,
  crosshairs, warning texts), one grenade HUD interface (`grhi`) per
  grenade type, one HUD globals tag (`hudg`: messages, waypoints, the
  motion sensor's numbers, damage arrows), one digit tag (`hud#`) and the
  message tags (`hmt `, `ustr`).
- Every element is a bitmap sequence (usually one sprite of a combined
  sprite page) placed by one of five anchors (the four corners and the
  centre of the 544 by 408 title-safe frame inside the 640 by 480 screen)
  plus a signed pixel offset, drawn at its native pixel size, tinted by a
  colour with an alpha that doubles as an "add instead of cover" control,
  and flashing with a half-sine pulse pattern that each element's colour
  block describes.
- Meters (shield, health, ammunition bars, heat) are the meter shader's
  rule fed from the HUD: the bar's fill is written into the alpha byte of
  a gradient colour, so one tag field pair (`alpha multiplier`, `alpha
  bias`) says how many units of value fill the bar.
- The player on foot sees, in one local-player window: the shield bar and
  health segments top right, the weapon's plate with its ammunition
  number and loaded-rounds bar and the grenade plate top left, the motion
  tracker bottom left, the weapon's crosshair at the centre, warning
  words near the crosshair, pickup prompts and messages at the left under
  the weapon plate, red damage arrows at the screen edges, and (zoomed)
  the scope.
- Several things differ from what the tag editor's names suggest and are
  stated here deliberately: the unit HUD chosen is the second one
  (`cyborg_mp`) only when two or more players share the screen, not in
  multiplayer as such; the overshield colours are fixed in the engine,
  not read from the tag; the crosshair turns red on a target when the
  player's aim assist has full strength, and nothing in the HUD reads the
  weapon's three reticle range fields; the weapon meters' flash colour is
  effectively never visible; the sniper rifle's second zoom label reads
  "10x" for an 8x zoom.

## Units and conventions

[cache_file.md](cache_file.md) owns the byte conventions (primitives,
blocks, tag references, `argb_color`), the tick (1/30 s) and the world
unit; this spec adds only what the HUD needs.

- **Canvas.** All HUD positions are pixels of the 640 by 480 screen (the
  Xbox renders 640 by 480 whatever the television; a wide television
  stretches it, inferred). Origin top left, x right, y down. An element
  offset is a signed 16-bit pixel count.
- **Safe frame.** The HUD is laid out inside the title-safe frame, the
  rectangle x 48 to 592, y 36 to 444 (544 by 408 pixels), not inside the
  whole screen. A corner anchor measures its offset from the matching
  corner of that frame; the centre anchor is the frame's middle,
  (320, 240). Checked indirectly on the disc: the sniper scope's two
  angle-tick strips and its four side bars come out symmetric about
  x = 320.5 only with the 48-pixel inset applied (Test cases, disc-backed).
- **Anchor enumeration** (the `anchor` fields): 0 top left, 1 top right,
  2 bottom left, 3 bottom right, 4 centre. Bit 0 of a corner value means
  "right", bit 1 means "bottom".
- **Sprites.** A HUD picture is "bitmap tag + sequence index (+ frame)".
  `bitmaps.md` owns the sequence rule (a sequence of whole bitmaps, or a
  sequence of sprites on a page). On the disc about a third of the HUD
  bitmap tags are sprite pages (bitmap type 3: the meters, reticles,
  warning words, digits, icons and arrows; a sprite is a rectangle of a
  page given as fractions) and the rest are ordinary 2D bitmaps with one
  bitmap per sequence (the plates, outlines, ammunition icons, scope
  masks, sensor pictures). A sprite's pixel size is its rectangle fraction
  times the page's pixel size, computed in single precision and
  **truncated toward zero to whole pixels, never rounded**; a whole
  bitmap is drawn at its own pixel size. Two disc sprites show it: the
  plasma heat bar's is 104.99994 pixels wide, so 104 (its rectangle
  runs from x = 52 to 156, not 157), and the "Low Battery" word's is
  49.00006 by 26.99994, so 49 by 26. (The sizes listed in the tables
  below are the authored ones; use these where they differ.) The registration points stored in the bitmaps are not used by
  the HUD (checked in the reference: every HUD placement ignores them).
- **Colours.** Colour fields are `argb_color` (bytes B, G, R, A in the
  file, the value written 0xAARRGGBB below: every hexadecimal colour in
  this spec, flash colours included, is alpha, red, green, blue from the
  most significant byte; 0xFFFF552B is alpha 255, red 255, green 85,
  blue 43, an orange red; a six-digit value is red, green, blue). Alpha is not opacity: see
  Colours, flashing and blending. `real_argb_color` fields (message
  colours) are f32 alpha, red, green, blue.
- **Time.** Tag times are seconds (flash periods, message up and fade
  times); the engine counts ticks (30 per second). A flash period of
  3.0 s is 90 ticks. `game_time` below means the tick counter.
- **Draw flags.** Every drawn element carries three booleans from its
  owner: *flashing*, *disabled* and *several local players*. Flashing
  switches a colour block from its plain colour to its flash pattern;
  disabled replaces the plain colour by the block's disabled colour (or
  blanks a meter); *several local players* is true when two or more local
  players share the screen (split screen).
- "Local player count" below always means the number of local players on
  this machine (one on a PC, up to four in split screen), never the
  number of players in a game.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| CANVAS | 640 x 480 | px | design canvas (and the Xbox back buffer) |
| SAFE_FRAME | x 48..592, y 36..444 | px | the frame HUD anchors measure from (one local player) |
| SPLIT_INSET | 4 | px | extra inset between the windows of a split screen |
| FLASH_TWO_PI | 6.283 | | the constant in the flash pulse (not exactly 2 pi) |
| HUD_NUMBER_GLYPHS | 15 | sprites | digits 0 to 9, decimal point, colon, minus, "m", "Km" |
| SHIELD_FLASH_BELOW | 0.25 | fraction | shield bar background flashes below this shield |
| SHIELD_HIT_HOLD | 15 | ticks | how long the lost part of a hit shield stays marked |
| SHIELD_LOW / HEALTH_LOW | 0.25 | fraction | warning sound thresholds |
| HEALTH_MINOR_MAJOR | 0.1875 | fraction | a body loss at or above this in one tick is "major damage" |
| OVERSHIELD_STAGES | 5 meters (stage 0 the tag's, stages 1 to 4 red, green, yellow, violet), 3 reachable | | see Shield |
| DEFAULT_SHIELD_SCALE | 255 | | meter value scale when the tag has 0 |
| DEFAULT_HEALTH_SCALE | 8 | segments | meter value scale when the tag has 0 |
| RADAR_RANGE | 20 | WU | motion sensor range (`hudg`, disc) |
| RADAR_VELOCITY_SENSITIVITY | 0.0002 | WU^2/tick^2 | squared speed at which a unit shows (`hudg`, disc) |
| RADAR_SCALE | 32 | | blip layout scale (`hudg`, disc); with the 1/32 factor below, one unit |
| RADAR_BLIP_FIELD_HALF | 42 / 32 | px | half size of the tracker's blip image, one / several local players |
| RADAR_MAX_BLIPS | 16 | | per local player |
| RADAR_HISTORY | 10 | ticks | blip echoes kept |
| RADAR_SCAN_PERIOD | 15 | ticks | the blip list is rebuilt every 15th tick |
| RADAR_DISTANCE_EXPONENT | 0.7 | | blip distance curve |
| RADAR_SWEEP_PERIOD | 2.1 | s (63 ticks) | sweep animation period |
| MESSAGE_LINES | 4 (3 in split screen) | lines | pickup and chat message lines |
| MESSAGE_FADE_POWER | 1.9 | | fade curve exponent |
| DAMAGE_ARROW_TICKS | 30 | ticks | an arrow shows while its counter is 1 to 29 |
| WAYPOINT_NEAR | 15 | WU | nearer than this a waypoint arrow grows |
| FRIEND_MARK_HEIGHT | 0.3 | WU | friend triangle height above the head |

## The HUD tags

Confidence for this whole section: checked against the disc (every
layout below parsed all 5 `unhi`, 17 `wphi`, 2 `grhi`, the `hudg`, the
`hud#`, the `hmt ` and the 8 `ustr` tags of `bloodgulch.map`, with every
padding word zero, every reference resolving to a tag of the expected
class and every enumeration in range); the meaning of fields comes from
the reference unless marked. Offsets are from the start of the struct
shown; all tag classes below have a build-tool "unused" tail that is zero
on the disc and ignored.

### Shared building blocks

**Absolute placement (0x24 bytes).**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 2 | enum16 | anchor | 0 to 4 as above |
| 0x02 | 2 | | padding | |
| 0x04 | 32 | | unused | |

**Placement (0x24 bytes).**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | point2d | offset | x, y in pixels, from the anchor point; see Placement |
| 0x04 | 8 | real_vector2d | scale | width and height multipliers of the drawn picture; 1, 1 on the disc except the messaging placement (0, 0, unused) |
| 0x0C | 2 | flags16 | scaling flags | bit 0 "don't scale offset", bit 1 "don't scale size" (both only matter when several local players share the screen), bit 2 is a PC-only bit, 0 on the disc. Values 0, 2 and 3 occur: 3 on the split-screen unit HUD and on the zoom labels, 2 on the warning words |
| 0x0E | 2 | | padding | |
| 0x10 | 20 | | unused | |

**Colour block (0x20 bytes)** (static elements, numbers, overlay items,
crosshair items, the messaging, objective and timer colours).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | argb | color | the plain colour |
| 0x04 | 4 | argb | flash color | colour at the peak of a flash |
| 0x08 | 4 | f32 | flash period | seconds; one cycle of the pattern |
| 0x0C | 4 | f32 | flash delay | seconds between the flashes of a cycle |
| 0x10 | 2 | i16 | number of flashes | flashes per cycle |
| 0x12 | 2 | flags16 | flash flags | bit 0 "reverse colors" (the pulse goes from the flash colour to the plain colour) |
| 0x14 | 4 | f32 | flash length | seconds; one pulse |
| 0x18 | 4 | argb | disabled color | colour while the element is disabled |
| 0x1C | 4 | | custom | two i16, ticks "up" and "fade", used only by the objective colour (scripts); 0 elsewhere |

**Static element (0x68 bytes).**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 0x24 | placement | placement | |
| 0x24 | 16 | tag ref `bitm` | interface bitmap | NONE means nothing is drawn |
| 0x34 | 0x20 | colour block | colors | |
| 0x54 | 2 | i16 | sequence index | the picture; frame 0 |
| 0x56 | 2 | | padding | |
| 0x58 | 12 | block | multitexture overlays | element 0x1E0, at most 1 on the disc (only the sniper's two angle-tick strips) |
| 0x64 | 4 | | unused | |

**Meter element (0x68 bytes).**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 0x24 | placement | placement | |
| 0x24 | 16 | tag ref `bitm` | meter bitmap | |
| 0x34 | 4 | argb | min color | colour at the bottom of the gradient (its alpha byte is ignored) |
| 0x38 | 4 | argb | max color | colour at the top of the gradient |
| 0x3C | 4 | argb | flash color | colour of the "just lost" part (only its RGB is used) |
| 0x40 | 4 | argb | empty color | colour of the unfilled part; its alpha byte A means "frame shows through 255 - A" (see Meters) |
| 0x44 | 1 | flags8 | meter flags | bit 0 switch colour on state change, bit 1 interpolate between min and max, bit 2 interpolate in HSV space, bit 3 along the farthest hue path, bit 4 invert the interpolation value. Disc: 0 or 1 |
| 0x45 | 1 | u8 | minimum value | floor of the fill alpha byte |
| 0x46 | 2 | i16 | sequence index | |
| 0x48 | 1 | u8 | alpha multiplier | |
| 0x49 | 1 | u8 | alpha bias | |
| 0x4A | 2 | i16 | value scale | units of the owner's value that make a full bar (0 means the owner's default) |
| 0x4C | 4 | f32 | opacity | disc 0 everywhere |
| 0x50 | 4 | f32 | fade | the fraction of the frame left behind a filled bar (0.35 to 0.75 on the disc) |
| 0x54 | 4 | argb | disabled color | not read by the reference's meter drawing |
| 0x58 | 12 | block | multitexture overlays | |
| 0x64 | 4 | | unused | |

**Number element (0x54 bytes).**

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 0x24 | placement | placement | |
| 0x24 | 0x20 | colour block | colors | |
| 0x44 | 1 | i8 | digits | digit places, 1 to 6 |
| 0x45 | 1 | flags8 | number flags | bit 0 show leading zeros, bit 1 show only when zoomed, bit 2 draw a trailing "m" (metres, "Km" above 999) |
| 0x46 | 1 | i8 | fractional digits | 0 to 4 |
| 0x47 | 1 | | padding | |
| 0x48 | 12 | | unused | |

**Icon element (0x10 bytes)** (a pickup prompt's weapon picture, the
button icons).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 2 | i16 | sequence index | into the HUD messaging icon bitmap; NONE means no icon |
| 0x02 | 2 | i16 | width offset | pixels added to (or, with the absolute-width flag, replacing) the icon's advance |
| 0x04 | 4 | point2d | offset | x, y in pixels |
| 0x08 | 4 | argb | color | used when flag bit 1 is set |
| 0x0C | 1 | i8 | frame rate | animation: frame = game_time / frame_rate (0: still) |
| 0x0D | 1 | flags8 | flags | bit 0 use text instead of the picture, bit 1 use this colour, bit 2 absolute width |
| 0x0E | 2 | i16 | text index | into the alternate icon text strings, for flag bit 0 |

**Sound element (0x38 bytes)** (every warning-sound block).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 16 | tag ref `snd!` or `lsnd` | sound | |
| 0x10 | 4 | flags32 | state flags | which interface states play it (the owner's list of states) |
| 0x14 | 4 | f32 | scale | passed to the sound player; 0 on every disc element (sounds still play: open question 7) |
| 0x18 | 32 | | unused | |

**Multitexture overlay (0x1E0 bytes) and effector (0xDC bytes)**
(layout level; only the sniper rifle's two angle-tick statics use one on
the disc).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 2 | flags16 | flags | 0 |
| 0x02 | 2 | enum16 | type | 0 |
| 0x04 | 2 | enum16 | framebuffer blend function | `shaders.md` codes; 0 on the disc |
| 0x08 | 32 | | unused | |
| 0x28 | 6 | 3 x flags16 | map flags | 0 |
| 0x2E | 4 | 2 x enum16 | map blending functions | 0 add, 1 subtract, 2 multiply, 3 multiply 2x, 4 dot; the ticks use (2, 0) |
| 0x34 | 24 | 3 x real_vector2d | map scales | |
| 0x4C | 24 | 3 x real_vector2d | map offsets | |
| 0x64 | 48 | 3 x tag ref `bitm` | maps | mask, ticks, none |
| 0x94 | 6 | 3 x i16 | map clamp | 1 means wrap |
| 0x9C | 184 | | unused | |
| 0x154 | 12 | block | functions | effectors, element 0xDC |
| 0x160 | 128 | | unused | |

Effector element: 0x00 64 bytes unused; 0x40 i16 destination type (0
tint, 1 horizontal offset, 2 vertical offset, 3 alpha); 0x42 i16
destination (0 geometry offset, 1 primary map, 2 secondary map, 3
tertiary map); 0x44 i16 source (0 player pitch, 1 pitch tangent, 2 yaw, 3
loaded ammunition, 4 total ammunition, 5 heat, 6 explicit (the lower input
bound), 7 zoom level); 0x48 and 0x50 two real bounds, input then output;
0x58 64 bytes unused; 0x98 and 0xA4 two RGB colours (tint at the lower and
upper bound); 0xB0 i16 periodic function; 0xB4 f32 period; 0xB8 f32
phase; 0xBC 32 bytes unused. The value is source, mapped linearly from
the input bounds to the output bounds (clamped), added to the map's
offset or used as its alpha or tint. Pitch and zoom level are the
only sources the disc uses. Yaw and the pitch tangent read 0 in the
reference.

### Unit HUD interface (`unhi`, 0x56C bytes)

Five tags on every multiplayer map: `ui\hud\cyborg` and
`ui\hud\cyborg_mp` (the Spartan; `objects.md` lists them as the unit's
`new hud interfaces`), and `ui\hud\warthog driver`, `ui\hud\passenger_2`,
`ui\hud\gunner` (vehicle seats; out of this spec's scope beyond a line).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x000 | 0x24 | absolute placement | placement | anchor for everything below except the motion sensor and the flashlight overlays |
| 0x024 | 0x68 | static element | HUD background | NONE bitmap on the Spartan |
| 0x08C | 0xF0 | metered panel | shield panel | below |
| 0x17C | 0xF0 | metered panel | health panel | below |
| 0x26C | 0xF0 | motion sensor panel | motion sensor | below |
| 0x35C | 0x24 | placement | blip placement | the blip image's centre, from the bottom-left corner of the safe frame (the sensor is always anchored bottom left) |
| 0x380 | 0x40 | auxiliary panel | auxiliary overlays | absolute placement 0x24, a block of overlay elements (0x84: static element, i16 type, flags16, 24 unused), 16 unused; empty on the disc |
| 0x3C0 | 12 | block | warning sounds | sound elements |
| 0x3CC | 12 | block | auxiliary meters | element 0x144 (the flashlight) |
| 0x3D8 | 404 | | unused | |

**Metered panel (0xF0 bytes):** 0x00 static element (background), 0x68
meter element, 0xD0 an extras struct of 16 bytes, 0xE0 16 bytes unused.
Extras for the shield panel: four argb (overcharge min, max, flash,
empty colours): zero on the disc and **not read** by the reference
(Shield). Extras for the health panel: 0xD0 argb medium colour, 0xD4 f32
maximum cutoff, 0xD8 f32 minimum cutoff, 0xDC pad. Extras for an
auxiliary meter: 0xD0 f32 minimum cutoff (flashes at or below it), 0xD4
flags32 (1 on the disc, unread), 8 pad.

**Motion sensor panel (0xF0 bytes):** 0x00 static element (background:
the dark disc), 0x68 static element (foreground: the cone, the range
label), 0xD0 32 bytes unused.

**Auxiliary meter (0x144 bytes):** 0x00 i16 type (0 integrated light), 2
pad, 16 unused, 0x14 a metered panel (0xF0), 0x104 64 bytes unused.

Warning sound states (bit n of a sound's state flags): 0 shield
recharging, 1 shield damage, 2 shield low, 3 shield empty, 4 health low,
5 health empty, 6 minor damage, 7 major damage. The disc uses bits 0 to 4
(Behaviour, shield and health).

### Weapon HUD interface (`wphi`, 0x17C bytes)

17 tags on a multiplayer map: nine weapon HUDs, the vehicle guns, and
shared parents (`ui\hud\master`, `master rounds`, `master plasma`,
`master fuel`, `empty`).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 16 | tag ref `wphi` | parent hud | the chain: a HUD draws its parents' elements first |
| 0x10 | 0x2C | flash cutoffs | flash cutoffs | below |
| 0x3C | 0x24 | absolute placement | placement | anchor of this HUD's statics, meters, numbers, overlays (0 top left on every disc tag; crosshairs always use the centre) |
| 0x60 | 12 | block | statics | element 0xB4 |
| 0x6C | 12 | block | meters | element 0xB4 |
| 0x78 | 12 | block | numbers | element 0xA0 |
| 0x84 | 12 | block | crosshairs | element 0x68 |
| 0x90 | 12 | block | overlays | element 0x68 |
| 0x9C | 4 | flags32 | valid crosshair types | bit n: crosshair state n is evaluated for this HUD (the union over the chain is used) |
| 0xA0 | 12 | block | warning sounds | sound elements; empty on the disc |
| 0xAC | 12 | block | screen effects | element 0xB8; at most the first is used |
| 0xB8 | 132 | | unused | |
| 0x13C | 0x10 | icon element | messaging icon | the weapon's picture in prompts ("Hold [X] to swap for [picture]") |
| 0x14C | 48 | | unused | |

**Flash cutoffs (0x2C bytes):** 0x00 flags16 (bit 0 "use parent HUD flash
parameters"), 0x02 pad, 0x04 i16 total ammo, 0x06 i16 loaded ammo, 0x08
i16 heat (percent), 0x0A i16 age (percent of battery left), 0x0C 32
unused. A state flashes at or below the ammunition cutoffs, at or above
the heat cutoff, and at or below the battery cutoff.

The element headers all start with the same 0x24 bytes: i16 **state
type** (statics, meters, numbers and overlays) or **crosshair type**
(crosshairs), i16 runtime flags (0 on the disc; the reference treats bit
0 as "invalid, skip"), i16 **use on map type**, i16 padding, 28 unused.

*State type* (which weapon quantity drives the element; also indexes the
flash references): 0 total ammo (the reserve), 1 loaded ammo, 2 heat, 3
battery (age), 4 secondary total ammo, 5 secondary loaded ammo, 6
distance to the target, 7 elevation of the target.
*Use on map type* is the index of a bit tested against three conditions:
bit 0 set unless the scenario is the main menu, bit 1 set with one local
player, bit 2 set with several. So 0 means "always", 1 "single window
only", 2 "split screen only".

| Element | Size | Layout after the 0x24 header |
|---|---|---|
| static | 0xB4 | 0x24 static element (0x68), 0x8C 40 unused |
| meter | 0xB4 | 0x24 meter element (0x68), 0x8C 40 unused |
| number | 0xA0 | 0x24 number element (0x54); 0x78 flags16 weapon flags (bit 0 "divide number by clip size"); 0x7A pad; 0x7C 36 unused |
| overlays element | 0x68 | 0x24 tag ref `bitm`; 0x34 block of overlay items (0x88); 0x40 40 unused |
| crosshairs element | 0x68 | 0x24 tag ref `bitm` (the sprite page); 0x34 block of crosshair items (0x6C); 0x40 40 unused |

**Overlay item (0x88 bytes):** 0x00 placement, 0x24 colour block, 0x44
i16 frame rate, 0x46 pad, 0x48 i16 sequence index, 0x4A i16 **type**
(a mask of the draw situations), 0x4C flags32 (bit 0 flashes, bit 1
runtime invalid), 0x50 56 unused. Type mask: bit 0 (1) show when
flashing, bit 1 (2) when empty, bit 2 (4) when reloading or overheated,
bit 3 (8) default (none of those), bit 4 (16) always. The grenade HUD
uses its own mask: 1 flashing, 2 empty, 4 default, 8 always.

**Crosshair item (0x6C bytes):** 0x00 placement, 0x24 colour block, 0x44
i16 frame rate, 0x46 i16 sequence index, 0x48 flags32, 0x4C 32 unused.
Flags: bit 0 flashes when active, bit 1 not a sprite (the sequence index
is a bitmap index of the page's tag), bit 2 show only when zoomed, bit 3
show sniper data, bit 4 hide area outside reticle, bit 5 one zoom level,
bit 6 don't show when zoomed, bit 7 runtime invalid. The disc uses only
0, 1, 4, 5 and 6 (bit 0, bit 2, bit 1 with bit 2, bit 0 with bit 2);
bits 3 to 6 occur on no disc tag.

**Crosshair types** (the enumeration of the crosshairs element's first
field, with the rule that makes each active; Behaviour, crosshairs, says
how they draw):

| Type | Name (tool) | Active when |
|---|---|---|
| 0 | aim | always evaluated; its value is 1 while the aim assist's autoaim level is exactly 1, else 0 |
| 1 | zoom | value = the player's zoom level + 1, or 0 unzoomed |
| 2 | charge | never (the reference sets it false) |
| 3 | should reload (flash ammo) | reserve above 0 and loaded rounds at or below the loaded cutoff |
| 4 | flash heat | heat x 100 at or above the heat cutoff |
| 5 | flash total ammo | reserve at or below the total cutoff and not reloading |
| 6 | flash battery (total battery) | age below 1 and (1 - age) x 100 at or below the age cutoff |
| 7 | reload / overheat | magazine 0 is reloading or chambering |
| 8 | flash when firing and no ammo | loaded 0, reserve 0, primary trigger held; latched, see below |
| 9 | flash when throwing and no grenade | no grenade of any type, no throw in progress, throw control held; latched |
| 10 | low ammo and none left to reload | (reserve or loaded) above 0, reserve 0, loaded at or below the loaded cutoff |
| 11 | should reload, secondary trigger | as 3 for magazine 1 |
| 12 | flash secondary total ammo | as 5 for magazine 1 |
| 13 | secondary reload | magazine 1 reloading |
| 14 | flash when firing secondary and no ammo | as 8 for magazine 1 (the reference tests the primary trigger) |
| 15 | low secondary ammo, none to reload | as 10 for magazine 1 |
| 16 | primary trigger ready | magazine 0 idle (can fire) |
| 17 | secondary trigger ready | magazine 1 idle |
| 18 | flash when firing and battery depleted | age exactly 1 and primary trigger held; latched |

States 11 to 15 and 17 are never active for a weapon with fewer than two
magazines (the reference's guard; the Xbox itself read an empty second
magazine). The crosshairs of the weapons held on foot use only types 0,
1, 3, 6, 8, 9, 10 and 18; the scorpion cannon adds type 7; no disc tag
uses 2, 4, 5 or 11 to 17.

**Screen effect (0xB8 bytes)** (the scope's mask and filters; three
weapon HUDs have one: pistol, rocket launcher, sniper rifle):

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | | unused | |
| 0x04 | 2 | flags16 | mask flags | bit 0 only when zoomed |
| 0x06 | 2 | | padding | |
| 0x08 | 16 | | unused | |
| 0x18 | 16 | tag ref `bitm` | mask, full screen | |
| 0x28 | 16 | tag ref `bitm` | mask, split screen | |
| 0x38 | 8 | | unused | |
| 0x40 | 2 | flags16 | convolution flags | bit 0 only when zoomed |
| 0x42 | 2 | | padding | |
| 0x44 | 8 | real_bounds | convolution radius in bounds | vertical field of view, radians |
| 0x4C | 8 | real_bounds | convolution radius out bounds | the warp radius r, in viewport pixels (section 9) |
| 0x54 | 24 | | unused | |
| 0x6C | 2 | flags16 | light enhancement flags | bit 0 only when zoomed, bit 1 connect to the flashlight, bit 2 uses the convolution mask |
| 0x6E | 2 | i16 | light enhancement script source | which scripted value scales it (0 none) |
| 0x70 | 4 | f32 | light enhancement intensity | |
| 0x74 | 24 | | unused | |
| 0x8C | 2 | flags16 | desaturation flags | bit 0 only when zoomed, bit 1 connect to the flashlight, bit 2 additive, bit 3 uses the convolution mask |
| 0x8E | 2 | i16 | desaturation script source | |
| 0x90 | 4 | f32 | desaturation intensity | |
| 0x94 | 12 | real_rgb_color | desaturation tint | |
| 0xA0 | 24 | | unused | |

### Grenade HUD interface (`grhi`, 0x1F8 bytes)

Two tags (`ui\hud\default` for frag, `ui\hud\plasma_flare` for plasma;
`globals.md` pairs them with the grenade types), also on `ui.map`.

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x000 | 0x24 | absolute placement | placement | 0 (top left) on the disc |
| 0x024 | 0x68 | static element | background | the grenade plate |
| 0x08C | 0x68 | static element | count panel background | the plate's outline |
| 0x0F4 | 0x54 | number element | count number | |
| 0x148 | 2 | i16 | flash cutoff | the plate flashes at or below this count (1) |
| 0x14A | 2 | | padding | |
| 0x14C | 16 | tag ref `bitm` | overlay bitmap | |
| 0x15C | 12 | block | overlay items | element 0x88 (the layout above) |
| 0x168 | 12 | block | warning sounds | empty on the disc |
| 0x174 | 68 | | unused | |
| 0x1B8 | 16 | | messaging icon | an icon element's bytes, junk on the disc (never read: grenade pickups use the item strings) |
| 0x1C8 | 48 | | unused | |

### HUD globals (`hudg`, 0x450 bytes)

One tag, `ui\hud\default`; the game's globals tag lists it among its
interface bitmaps (`globals.md`, index 6).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x000 | 0x120 | messaging parameters | messaging | below |
| 0x120 | 0x9C | waypoint parameters | waypoint | below |
| 0x1BC | 0x104 | multiplayer parameters | multiplayer | f32 HUD scale at 0x1BC (0.75 on the disc; the reference's scale function ignores it and returns 1) then 256 unused |
| 0x2C0 | 0x50 | defaults | defaults | below |
| 0x310 | 0x50 | damage indicators | damage indicators | below |
| 0x360 | 0x68 | timer | timer colours | colour block (running), colour block (time up), 40 unused |
| 0x3C8 | 16 | tag ref `bitm` | carnage report bitmap | `ui\shell\bitmaps\postgame_carnage_report` (the end-of-game screen's, not in play) |
| 0x3D8 | 2 | i16 | loading begin text index | into the item strings (42 on the disc) |
| 0x3DA | 2 | i16 | loading end text index | 43 |
| 0x3DC | 2 | i16 | checkpoint begin text index | 44 |
| 0x3DE | 2 | i16 | checkpoint end text index | 45 |
| 0x3E0 | 16 | tag ref `snd!` | checkpoint sound | null on the disc |
| 0x3F0 | 96 | | unused | |

**Messaging parameters (0x120):** 0x00 absolute placement (0x24; 0 top
left), 0x24 placement (0x24), 0x48 tag ref `font` single-window font,
0x58 tag ref `font` split-screen font, 0x68 f32 up time (s), 0x6C f32
fade time (s), 0x70 real_argb_color state colour, 0x80 real_argb_color
text colour, 0x90 f32 spacing (line pitch multiplier), 0x94 tag ref `ustr`
item messages, 0xA4 tag ref `bitm` messaging icons, 0xB4 tag ref `ustr`
alternate icon text, 0xC4 block of icon elements (0x10) button icons,
0xD0 colour block (help text, with flash), 0xF0 tag ref `hmt ` HUD
messages, 0x100 colour block objective colour (its custom word: up ticks
and fade ticks).

**Waypoint parameters (0x9C):** 0x00 f32 top offset, 0x04 bottom, 0x08
left, 0x0C right (pixels the arrows keep away from the safe frame's
edges), 0x10 32 unused, 0x30 tag ref `bitm` arrow bitmap, 0x40 block of
arrows (element 0x68), 0x4C 80 unused. Arrow element: 0x00 tag string
name (32), 0x20 8 unused, 0x28 argb colour, 0x2C f32 opacity, 0x30 f32
fade, 0x34 three i16 sequence indices (on screen, off screen, occluded),
0x3A pad, 0x3C 16 unused, 0x4C flags32 (bit 0 don't rotate off-screen),
0x50 24 unused.

**Defaults (0x50):** 0x00 tag ref `wphi` default weapon HUD
(`ui\hud\empty`: shown when the player holds nothing), 0x10 f32 motion
sensor range (WU), 0x14 f32 motion sensor velocity sensitivity
(squared WU per squared tick), 0x18 f32 motion sensor scale, 0x1C
rectangle2d default title bounds (425, 0, 444, 597: unused by anything
here), 0x24 44 unused.

**Damage indicators (0x50):** 0x00 i16 top offset, 0x02 bottom, 0x04
left, 0x06 right (10 each on the disc), 0x08 32 unused (the tag has no
fade or flash times: the arrow does not fade), 0x28 tag ref
`bitm` indicator bitmap, 0x38 i16 sequence index (0), 0x3A i16
split-screen sequence index (1), 0x3C argb colour (0xC8FF0000), 0x40 16
unused.

### The number, message-text and string tags

**`hud#`** (one tag, `ui\hud\counter`): 0x00 tag ref `bitm` (a sprite
page whose first sequence holds 15 glyph sprites in the order digits 0 to
9, decimal point (index 10), colon (11), minus (12), "m" (13), "Km"
(14)), 0x10 i8 character width (12), 0x11 i8 screen width (9: the advance
between digits), 0x12 i8 x offset (1), 0x13 i8 y offset (0), 0x14 i8
decimal point width (6), 0x15 i8 colon width (6), 0x16 pad, 0x18 76
unused. The reference reads only the screen width and the decimal point
width; the others are unread.

**`hmt `** (one tag, `ui\hud\hud messages`): 0x00 data reference (20
bytes) UTF-16 text pool, 0x14 block of elements (2 bytes each: u8 type, u8
data), 0x20 block of messages (0x40 bytes each), 0x2C 84 unused. Message
element: 0x00 tag string name (32), 0x20 u16 text start index (in
characters), 0x22 u16 first element index, 0x24 u8 element count, 0x25 3
pad, 0x28 24 unused. Element type 0 is a text run whose `data` is its
length in characters (the NUL included); type 1 is an icon whose `data` is
an icon number: 0 to 17 the console's buttons and sticks (A, B, X, Y,
black, white, left and right trigger, d-pad up, down, left, right,
start, back, left and right thumb, left and right stick, indexing the
globals' button icon list), 18 "action", 19 "throw grenade", 20 "primary
trigger", 21 "integrated light", 22 "jump", 23 "use equipment", 24
"rotate weapons", 25 "rotate grenades", 26 "crouch", 27 "zoom", 28
"accept", 29 "back", 30 "move", 31 "look" (each of 18 to 31 resolves
through the player's control layout to one of the button pictures), 32 to
39 "custom 1 to 8" (the numbers follow the reference's list; those the
disc uses are 18, 24, 32, 33). Thirteen messages on the disc, in
order: pickup, swap powerup, touch device, custom device, swap
weapon, remind to switch weapons, enter vehicle, exit vehicle, flip
vehicle, and four respawn failures (moving, unsafe, combat, vehicle);
their texts are in the disc tables below.

**`ustr`** (unicode string list): 0x00 block of strings, element 0x14 =
one data reference to UTF-16LE text including its terminating NUL. The
HUD's lists are `ui\hud\hud_item_messages` (46 strings: pickups and the
loading and checkpoint texts) and `ui\hud\hud_icon_messages` (27
strings: button and vehicle names).

## Behaviour

Confidence marks: **(disc)** the number or layout is checked against the
disc's tags; **(ref)** the rule is read from the reference only;
**(inf)** inferred. Rules the reference shows only in its additions for
desktop ports are left out.

### 1. Canvas and placement

Inputs: an element's anchor (from its HUD's absolute placement, or fixed
by the code: crosshairs use the centre; the motion sensor, its blips and
the friend markers have their own), its placement offset (ox, oy), its
placement scale (sx, sy), the picture's pixel size (w, h), and the
window's safe frame (x0, y0, x1, y1) (one local player: 48, 36, 592,
444). **(ref; the safe frame constants disc-consistent)**

1. The **anchor point** A, truncated to whole pixels:
   - corners: A.x = x0 + ox for left, x1 - ox for right; A.y = y0 + oy
     for top, y1 - oy for bottom;
   - centre: A = ((x0 + x1) div 2 + ox, (y0 + y1) div 2 + oy).
   Offsets are in the sense of the picture: a positive offset on a right
   or bottom anchor moves the picture *left* or *up*, towards the middle.
2. The **picture rectangle** (the drawn quad): its four corners are A
   plus a bound per axis, times the scale, each product converted to an
   integer (the reference's own float-to-integer conversion; use
   truncation toward zero, and expect at most one pixel of difference on
   half-pixel cases):

   | Anchor | x bound | y bound |
   |---|---|---|
   | top left | 0 .. w | 0 .. h |
   | top right | -w .. 0 | 0 .. h |
   | bottom left | 0 .. w | -h .. 0 |
   | bottom right | -w .. 0 | -h .. 0 |
   | centre | -w/2 .. w/2 | -h/2 .. h/2 |

   So the picture's matching corner (or centre) sits on A. The scale
   used is the placement scale times a draw scale (1, except crosshair
   items in split screen: 0.5).
3. **Texture rectangle.** A sprite maps its own rectangle of the page; a
   whole bitmap maps all of it. HUD pictures are sampled bilinearly
   (point-sampled for meters with one local player).
4. **Viewport.** The window's rectangle is given in screen pixels;
   positions are drawn relative to the player's own viewport (the whole
   screen with one player; the window's pixel rectangle in split screen,
   see section 15).

Draw scale 1 everywhere except where stated: **the HUD does not scale
with the resolution on the Xbox**; it is drawn at 640 by 480 and the
whole image is scaled by the display. How another resolution is mapped is
ours to choose (section 16).

### 2. Which HUD is drawn, and the order

**Gate.** For a local player's window the weapon, unit, waypoint and
damage pieces draw only while: the player has a unit, the camera is the
player's first-person (or follow) camera (not a scripted or neutral
perspective, so not in a cinematic), and the script flag "show HUD" is on
(it is on at the start of a map). With the flag off only the messages
are left, and the unit warning sounds are updated with every state off
(so they stop). **(ref)**

**Unit HUD choice.** The unit's `huds` block (two references on the
Spartan, `objects.md`) is indexed 0 with one local player, 1 with two or
more, clamped to the last entry. So a single window shows `ui\hud\cyborg`
even in a multiplayer game; `ui\hud\cyborg_mp` is the split-screen
variant (smaller offsets, 64-pixel sensor, scaling flags 3). **(ref;
disc-consistent: the second tag is the smaller one)** The same rule picks
a vehicle's.

**Draw order** within one window each frame (later draws on top): **(ref)**

1. teammate markers (team games), section 14;
2. the **weapon crosshairs** of the current weapon's HUD chain, the
   weapon's own HUD first, then each parent;
3. the **weapon HUD** elements, deepest parent first, the weapon's own
   HUD last; inside one HUD: statics, meters, numbers, overlays;
4. the **grenade HUD** (background plate, count plate, count number,
   overlays);
5. the action prompt (pickup text) is set (drawn with the messages);
6. the unit warning sounds are updated;
7. the **unit HUD**: background; shield (meters from the lowest stage
   up, then the shield background); health (meter, then the health
   background); the motion sensor (background, foreground, blips with the
   sweep); the flashlight overlays and meter;
8. waypoints, section 14;
9. the damage arrows;
10. the message lines (the prompt line first, then the item messages).

### 3. Colours, flashing and blending

**Blending (disc: the classes; ref: the HUD's use).** Every HUD picture
uses the framebuffer blend function 7 of `shaders.md`, "alpha-multiply
add". The texel (r, g, b, a) (an `ay8` texel is (y, y, y, y), an `a8y8`
texel (y, y, y, a)) is multiplied channel by channel by the draw colour
(R, G, B, A), and the frame becomes
out.rgb + frame x (1 - out.a), with out.rgb = texel.rgb x colour.rgb and
out.a = texel.a x colour.a. So a colour with alpha 0 (a reticle, a
meter's tint, a warning word) only adds light; alpha 255 on an opaque
texel covers the frame with the colour; the plates (alpha 0xDC) tint and
darken.

**Colour choice** for a static element, number or flash-aware overlay,
in this order: disabled flag: the block's disabled colour; else flashing
flag: the flash function below at the element's flash start tick; else
the plain colour.

**The flash function.** With P the block's flash period, d its delay, l
its length, N its count, r the tick the element started flashing, now
the tick, t = (now - r) / 30:

- phase = t mod P; if P is 0 or less there is no flash: the plain colour.
- If phase is at or above N (d + l): the plain colour.
- Else phase' = phase mod (d + l). If r = 0 (the aim crosshair's special
  call, and any element that started on tick 0): the flash colour (the
  plain colour with "reverse colors").
- Else if phase' < l: f = sqrt(clamp(1 - (cos(6.283 x phase' / l) + 1) / 2,
  0, 1)) (a half-sine pulse, 0 to 1 and back to 0), and the colour is
  the plain colour moved toward the flash colour by f in all four
  channels (with "reverse colors", from the flash colour toward the plain
  colour).
- Else the plain colour (with "reverse colors" the flash colour).

So the usual block (period 3.0, delay 0.225, length 0.225, three flashes)
pulses three times in the first 1.35 s of every 3 s and rests for the
other 1.65 s; the weapon plates (period 3.0, delay 0.225, 3 flashes) and
the shield plate (period 1.35, 3 flashes) behave likewise. **(ref)**

**Flash start ticks.** Each element family keeps, per owner state, the
tick its flashing began (NONE when it is not flashing): the unit's
shield, health and sensor panels (one tick each), the weapon's eight
state references, the grenade plate, the crosshair states. Flashing
starts the phase over; it is not a free-running clock.

### 4. Meters

**(ref; equation: `shaders.md` Meter; parameters disc)** A meter is one
picture (the meter bitmap's sprite) drawn through the meter shader. The
sprite's **blue channel is the position along the bar** (0 at the empty
end, 255 at the full end, in steps the artist laid out) and its alpha is
the bar's shape. The HUD feeds the shader through the **alpha bytes** of
its two colours:

- minimum alpha = max(`minimum value`, clamp(alpha multiplier x min value
  + alpha bias, 0, 255)); maximum alpha likewise from the max value.
  For a plain meter min and max values are equal, except a shield that
  has just been hit (sections 5 and 6).
- The shader's value x = minimum alpha / 255 (texels whose blue channel
  is below x are "filled"); its flash extension e = maximum alpha / 255;
  its gradient is 1 (the colour runs from the minimum colour at blue 0 to
  the maximum colour at blue 1).
- Colours handed over, by meter flags: **bit 0 clear** (shield, weapon
  meters, the flashlight): gradient minimum = (min color RGB, minimum
  alpha), gradient maximum = max color RGB, flash colour = (flash color
  RGB x phi, maximum alpha) with phi = 0 when the caller's reference time
  is negative, else clamp(1 - reference time, 0, 1). In words: the
  flash colour's red, green and blue (as 0xAARRGGBB: the low three
  bytes of the tag's flash color) are each multiplied by phi and the
  result gets the maximum alpha byte as its alpha; the reference time
  is a number of **seconds** (the shield's fade time f, which stays 0
  for the whole 15-tick hold of section 5), or -1 when the caller has no
  fade running, or, for the weapon meters, the flash start tick taken as
  a number. Only these three cases occur, so phi is 1 for a hit shield
  (the fade time is 0 while the lost part shows), 0 for every meter
  with no fade (-1), and 0 for the weapon meters (a tick of 1 or more
  gives clamp(1 - tick) = 0, the quirk below; only tick 0 would give 1).
  Not the other readings: phi is not a pulse over time and does not
  fade across the hold. Worked example, the shield (value scale 198,
  alpha multiplier 1, bias 55, flash color 0x00FFFFFF) hit from 1.0 to
  0.6 on tick T: on ticks T to T + 14 stage 0 has v = 0.6, w = 1.0,
  fading; min value trunc(198 x 0.6) = 118, max value 198; minimum alpha
  118 + 55 = 173, maximum alpha 198 + 55 = 253; reference time 0, so phi
  = 1 and the flash colour is 0xFDFFFFFF (alpha 253, white); the shader
  paints the texels whose blue byte lies from 173 up to 253 (the lost
  part) in that colour and the texels below 173 in the bar's colours.
  From tick T + 15 the stage is no longer fading (the reference time
  is -1, phi 0, the marker gone). **Bit 0 set**
  (health): not flashing: both gradient ends = min color RGB (minimum
  alpha on the minimum), flash alpha = minimum alpha, no flash colour;
  flashing: both ends = max color RGB. (Bits 1 to 4, interpolation by
  the reference value, occur on no disc tag; with bit 1 both ends become
  min color moved toward max color by the value, or 1 - value with
  bit 4.) **Disabled** flag: all three colours are zero (a blank bar).
- Background colour = empty color RGB with alpha = 255 - empty color's
  alpha byte; tint colour = grey of (1 - opacity) with alpha = fade;
  gradient 1; tint mode 2 (the meter shader's flag 3). **(inf)** In the
  shader's terms: a filled texel adds its colour times the tint grey and
  keeps the fraction `fade` of what is behind it (0.5 on the shield, 0.75
  on health and the flashlight, 0.35 on the plasma heat bar, 0.75 on the
  ammunition bars); an empty texel keeps 1 - A/255 of what is behind it
  (the shield's and health's empty colour has alpha 0: the empty part is
  fully transparent; the weapons' bars use 0x0000254A: alpha 0, a dark
  blue-green (0, 37, 74), so their empty part adds a faint blue) and adds
  the empty colour.
- **Weapon meters' flash colour is not visible.** Their reference time is
  the flash start *tick* (a large number), so phi = 0 (a quirk, keep:
  phi is nonzero only for a flash that began in the first tick of the
  map). The ammunition "low" warnings are the flashing plates, numbers
  and warning words instead.

### 5. Hit and recharge bookkeeping of the shield (per tick)

State per local player: L (the shield the HUD last showed), f (a fade
time, seconds, or -1), t (a tick). With s the unit's shield vitality now
(a fraction, up to 3 with an overshield) and now the tick, run once a
tick for each local player with a unit; L and the body figure start at
-1 and take the unit's values on the first update. **(ref)**

1. If L > s (the shield fell): if f < 0 or f > 1 then t = now. If now - t
   < 15: f = 0 (L is **kept** at its old value: the meter shows the lost
   part marked). Else: L = s, f = f + (now - t) / 30, t = now.
2. Else if L < s (the shield rose, a recharge or an overshield pickup):
   L = s, f = -1, t = now.
3. Else: L = s; if f > 0 then f = f + (now - t) / 30; t = now.

So after a hit the lost part of the bar stays marked for 15 ticks (half a
second) from the first loss of the episode, then the marker snaps away
(the fade variable f is carried but, with L = s from then on, the
marker's draw no longer asks for it). While an overshield drains
(`health_shields.md`: 1/1350 per tick above 1.0) the damage code
subtracts the same amount from L each tick (the engine's shield-tick
call), so a draining overshield never flashes. The recharge (the shield
gaining about 1/120 per tick after the six-second delay) is the bar
simply growing, plus the looping "shield charge" sound; there is no
extra recharge animation. **(ref)**

### 6. The unit HUD: shield, health, sounds

**The shield panel** draws when the game type has shields and the script
has not hidden it. Draw flags: flashing when s < 0.25 (or a script blink),
disabled when the unit is dead, several-players per section 15. **(ref)**

- Flash start: the panel's flash tick is set when flashing begins and
  cleared (NONE) when it stops (only for the unit the player controls,
  not a vehicle the unit rides).
- The meter, if the tag has a meter bitmap, is drawn in **stages**
  i = 0, 1, 2, ...: v_i = clamp(s - i, 0, 1), w_i = clamp(L - i, 0, 1),
  the stage is "fading" when w_i > v_i, and its top = w_i if fading else
  v_i. Stop at the first stage whose v_i and top are both 0. Stage 0 uses
  the tag's meter element as it is; stages 1 and up use a copy whose min
  and max colours are fixed (the engine's list of five colours has an
  unused first entry 0, so stage i takes entry i): stage 1 red
  (0x00FF0000), stage 2 green (0x0000FF00), stage 3 yellow (0x00FFFF00),
  stage 4 violet (0x007F00FF), all as 0xAARRGGBB (checked in the disc's
  executable as well as in the reference). The tag's own
  overcharge colour fields are zero on the disc and unread. Each stage
  draws with value scale S (the tag's, or 255 when 0): min value =
  min(trunc(S x v_i), 255), max value = min(trunc(S x top), 255), the
  draw flags above, reference time f if fading else -1, reference value
  v_i. For a Spartan, s reaches 3.0 at most, so three stages exist: the
  normal blue bar, a red bar over it from 1.0 to 2.0, a green bar from
  2.0 to 3.0.
- The shield background plate is drawn **after** the meter (over it).

With the Spartan's numbers (value scale 198, alpha multiplier 1, bias 55):
the bar's fill alpha is trunc(198 s) + 55, so s = 0 gives 55, s = 0.5
gives 154, s = 1 gives 253. The meter bitmap's blue channel therefore
runs from about 55 to 253 along the bar; keep that offset.

**The health panel** draws unless scripted off. Flashing when the unit's
"shield depleted" flag is set (or a script blink), disabled when dead.
Colour by body vitality b with the tag's cutoffs (0.67 maximum, 0.33
minimum): b >= maximum: the max colour (blue 0x2896FF); b <= minimum: the
min colour (red 0xFF0000); between: the medium colour (yellow
0xFFDF00); the meter takes that colour solid (meter flag 0, section 4).
Value scale = the tag's or 8 (segments): value = min(trunc(8 b), 255),
fill alpha = max(31, 30 x value + 15): 31, 45, 75, ... 255 (b = 0.5 gives
135, b = 1 gives 255). The health background flashes (not the meter)
while the shield is depleted. The health panel's `last body vitality` is
updated after it draws. **(ref)**

**Dead unit.** Everything draws disabled (blank meters, the plates in
their disabled colour, which is 0 on the disc: invisible), while the
player's unit still exists. **(ref)**

**Warning sounds (per frame, with the HUD gate and no cinematic).** The
sound state word is rebuilt each frame: bit 0 shield charging (the unit's
damage flag); bit 1 L > s (so it stays set through the 15-tick hold of
section 5); bit 2 0 < s < 0.25; bit 3 s = 0; bit 4 body vitality < 0.25;
bit 5 dead; bit 6 the body fell since the last frame by less than 0.1875;
bit 7 it fell by 0.1875 or more. The shield bits need the game type to
have shields and the shield panel not scripted off; the health bits need
the health panel not scripted off. The unit's warning sound block is then
played by one rule: **(ref)**

- an element whose flags intersect the word plays: a one-shot (`snd!`)
  starts only if it has no handle or its latch is clear, then the latch
  is set; a looping sound (`lsnd`) starts if it has no handle; (the
  latch stays set while the state holds);
- an element whose flags do not intersect: a looping sound is stopped;
  the handle and latch are cleared (so the one-shot may fire again).

On the disc (both Spartan HUDs): `sound\sfx\ui\shield_hit` (`snd!`, state
bit 1), `shield_charge` (`lsnd`, bit 0), `shield_low` (`lsnd`, bit 2),
`shield_depleted` (`lsnd`, bit 3), `health_low_heart` (`lsnd`, bit 4);
at most 12 elements are played per unit HUD. When the unit is dead, or on
media (the reference's flag), no sounds play and the last-unit memory
clears. Sounds are played unspatialised through the interface (`sounds.md`).

### 7. The weapon HUD

**Which HUD.** The weapon in the unit's current slot; if there is none
and the unit sits in a gunner seat, the vehicle's current weapon; if the
unit holds no weapon at all (and is not in a driver or passenger seat),
the globals' default HUD `ui\hud\empty` with a zeroed weapon state;
otherwise nothing. A weapon with no HUD tag (the gravity rifle, ball,
flag) shows no weapon HUD if the unit holds any weapon. **(ref)**

**Weapon interface state** (read from the weapon each frame and each
tick): heat (0..1), age (0 full battery, 1 empty), overheated flag, the
magazine count (at most 2), and per magazine: reloading (the magazine is
reloading or chambering), can fire (idle), rounds loaded, rounds loaded
maximum, reserve (`rounds total`, not counting the loaded rounds:
`weapons.md`). **(ref)**

**The chain.** The weapon's HUD is the root; each HUD's `parent hud` is
followed up to 16 levels (a longer chain is an error). Crosshair states
are evaluated for the union of the chain's valid-type flags; flash
cutoffs of the **root** are used for them. Drawing recurses to the parent
before drawing a HUD's own elements. **(ref)**

**Flash states and values** (computed for a HUD unless its flash flags
bit 0 ("use parent HUD flash parameters") is set and the call came from
a child, in which case the child's arrays are used unchanged; the flag is
set on `master rounds` and `master plasma`, so the weapon's own cutoffs
apply to the shared plates). Cutoffs are the HUD's own: C_t total, C_l
loaded, C_h heat, C_a age. m = local player count > 1. **(ref)**

| State | Flashing | Disabled | Number value | Meter byte |
|---|---|---|---|---|
| 0 total | reserve <= C_t | reserve = 0 | reserve | reserve (as a byte) |
| 1 loaded | loaded <= C_l and not reloading | never | loaded | loaded |
| 2 heat | heat x 100 >= C_h | never | trunc(heat x 255) | trunc(heat x 255) |
| 3 battery | (1 - age) x 100 <= C_a | trunc(age x 100) reaches 100 | trunc((1 - age) x 100) | the same |
| 4 secondary total | as 0 for magazine 1 (none if fewer than 2 magazines) | as 0 | magazine 1 reserve | |
| 5 secondary loaded | as 1 for magazine 1 | never | magazine 1 loaded | |
| 6 distance | never | never | real: distance in metres | |
| 7 elevation | never | never | real: height difference in metres | |

The several-players bit is added to every state. States 6 and 7 hold
NaN unless the aim assist's autoaim level is exactly 1 and it has a
target object; then state 6 = |camera - target origin| x 3.048 (WU to
metres) and state 7 = (target z - camera z) x 3.048, both from the
player's camera position. A number of those states draws only when its
value is not NaN. Each state has a flash start tick (NONE when not
flashing).

**Overlay masks** (for overlay items whose `type` is tested against
them; bits as in the item layout; "always" is always set; "default" is
set when none of flashing, empty, reload is): state 0: flashing when
reserve <= C_t and not reloading, reload when magazine 0 reloads, empty
when reserve = 0. State 1: flashing when loaded <= C_l, reload as above,
empty when loaded = 0. State 2: flashing when heat x 100 >= C_h, reload
when overheated, and "empty" when the age percent is 100 (the reference's
text). State 3: flashing when (1 - age) x 100 <= C_a, reload when
overheated, empty when age percent is 100. States 4 and 5 as 0 and 1 for
magazine 1, set to 0 when the weapon has fewer than two magazines.

**Elements** are skipped when their runtime-invalid flag is set, when
their `use on map type` bit is not set for the window (the rule in the
weapon HUD tag section), or when the state type is not 0 to 7. Then:

- **Static:** drawn with state s's flashing and disabled flags and flash
  start; a static with multitexture overlays also draws the overlays
  (sniper ticks) with the same colour.
- **Meter:** value byte from the table, min = max = the byte, the
  state's flags, reference time = the state's flash start tick as a real
  (the quirk of section 4), reference value 0.
- **Number:** value as in the table. With "divide number by clip size"
  the (integer or real) value is divided by magazine 0's loaded maximum
  (1 if 0); a number with fractional digits uses the real states (6, 7)
  and skips when NaN. The number's "only when zoomed" flag is not read
  by the reference's drawing (open question 4). Drawn by the number
  procedure below.
- **Overlay items:** for each item not invalid whose `type` & the state's
  overlay mask is non-zero: colour = the flash function if the item
  flashes and the state is flashing, else its plain colour; frame =
  ((now - flash start) div frame rate div 30) mod sprite count when the
  item flashes, the state flashes, the frame rate is positive and the
  sequence has sprites, else 0.

**The number procedure** (digit sprites from `hud#`; w_s = the tag's
screen width, 9; w_d = its decimal point width, 6): **(ref)**

1. Colour: disabled, flashing or plain, as for statics.
2. digit count D = digits + (fractional digits, at most 4, plus 1 when
   there are fractional digits and a decimal value) + 1 with the "m"
   flag. "Km" form when the value exceeds 999 with the "m" flag: the
   integer part becomes value div 1000 and the decimal value value x 10.
3. Origin O = the anchor point of the number's placement (section 1,
   no picture).
4. The first glyph's x (cursor): top-left and bottom-left anchors:
   O.x + (D - 2) x w_s + (w_d if fractional digits); centre:
   O.x + ((D - 1) x w_s + w_d) / 2 (truncated); right anchors: O.x.
   Glyphs are drawn with the anchor's bounds (section 1), so for a
   left-anchored number the sprite's left edge is the cursor, for a
   right-anchored one its right edge is. The y is O.y for every glyph.
5. Drawn right to left, the cursor moving left by w_s after each glyph:
   the "m" or "Km" glyph (13 or 14) first when flagged; then, with
   fractional digits and a decimal value >= 0: the fractional digits
   (the decimal value scaled to the digit count, least significant
   first), then the decimal point glyph (after moving the cursor right
   by w_s - w_d), each followed by the same step; then the integer part
   least significant digit first, stopping as soon as the remaining value
   is 0 **unless "show leading zeros"** (so a value of 0 draws nothing
   without that flag); then the minus glyph if the value is negative.
6. Digit scale = the HUD scale (1). The sprites are 18 by 12 pixels with
   the glyph occupying about 12 by 10 inside, so neighbouring sprites
   overlap by half their width, as the italic digits are meant to.

### 8. Crosshairs

**(ref; the elements disc)** Crosshair states live per local player,
updated once a tick, drawn each frame before the weapon HUD.

**Update (per tick).** For each crosshair type n in the union of valid
flags: compute `result` per the table in the tag section; the render bit n
is set when result > 0 (the aim bit is always set). Stored value: aim:
result (0 or 1); zoom: result - 1, i.e. the player's zoom level + 1 or 0
(so 0 unzoomed, 1 first level, 2 second); every other type: NONE when
result is 0, else the tick it became active (kept while it stays
active). If the unit just lost its weapon (none held) the stored values
reset to 0. **(ref)**

**Draw.** For each HUD of the chain (the weapon's, then parents), each
crosshair element whose type is valid, whose render bit is set and whose
map type bit is set (the rule in the weapon HUD tag section), each item:
**(ref)**

- skipped if invalid; skipped if flag bit 2 and the zoom value is 0; skipped
  if flag bit 6 and the zoom value is not 0;
- draw scale 0.5 with several local players unless the item's scaling
  flag bit 1 is set, else 1; the picture is centred at the centre anchor
  plus the item offset (the item's own scale multiplies the draw scale);
- the picture: unless flag bit 1 ("not a sprite"), sprite
  `sequence index` of the element's page, frame by type; with that flag
  the sequence index is a bitmap index of the page's tag (the sniper's
  side bars) drawn whole;
- frame and colour by type:
  - **aim (0):** with "flashes": frame 0, and if the aim value > 0 the
    flash colour (the flash function with start tick 0: solid flash
    colour), else the plain colour; without: frame = the aim value (0 or
    1; the sprite index is taken modulo the sequence's sprite count, so
    a one-sprite sequence draws sprite 0 either way), plain colour.
  - **zoom (1):** with flag bit 5 ("one zoom level") a zoom value of 0 skips
    and the frame is 0; else frame = zoom value, minus 1 when flag bit 2 is
    set (so the first zoom level is frame 0 on a "only when zoomed"
    item). With "flashes" and the aim value > 0: the flash colour; else
    plain.
  - **8, 9, 14, 18 (the firing warnings):** active while their trigger
    condition holds, then latched: once the condition ends the state is
    cleared when now - start >= trunc(flash period x 30) of the item's
    colour block (90 ticks for a 3.0 s period). While active they behave
    as the next group.
  - **all other types:** frame = ((now - start) div frame rate div 30)
    mod sprite count when the frame rate is positive and the sequence has
    sprites, else 0 (so a frame rate of 1 advances one sprite per
    second); with "flashes": the flash function colour at the state's
    start tick, else the plain colour.
- the sprite index is taken modulo the sprite count (a frame past the
  end wraps).

**What the disc's weapons show** (the full table is in The disc's
values):

- the **aim reticle** (type 0, one item at the centre, "flashes"): the
  weapon's reticle in its plain colour (blue 0x2896FF with alpha 0, so it
  adds light; the sniper's is opaque blue 0xFF2896FF), turning solid
  **red (0x00FF0000)** while the aim assist's autoaim level is exactly 1
  (a living enemy-team biped, in line of sight, within half the weapon's
  autoaim range and half its autoaim angle: `weapons.md` section 15);
  friends never change it; the three reticle range fields of the weapon
  tag (4, 16, 80 on the warthog gun; 0 on foot) are not read by the HUD;
- **no bloom.** The reticle's size and shape never change with the
  weapon's error, heat or movement: nothing in the HUD reads them (the
  crosshair states above are the whole input). Spread is not shown. **(ref)**
- **warnings near the reticle**, all in the shared `master` HUDs: "No
  Ammo" (type 8, 30 pixels left of and 30 below the centre, flashes for as
  long as the trigger is pulled on an empty weapon and for up to 3 s
  after), "Reload" (type 3, same spot, while the reserve is above 0 and
  the loaded rounds are at or below the weapon's loaded cutoff), "Low
  Ammo" (type 10, same spot, when only the loaded rounds remain and they
  are at or below the cutoff), "No Grenades" (type 9, 60 below the centre,
  after a throw attempt with none), "Low Battery" (type 6, plasma weapons,
  left-below the centre), "Battery Depleted" (type 18, the plasma
  weapons' master HUD, same area), "No Weapon" (the default HUD, type 8, 40
  below). The plain colour of every warning word is zero (invisible); it
  shows only in its flash pulses (peak blue 0x2896FF at alpha 0).
- **zoom labels**: the pistol and the rocket launcher show a "2x" label
  (hud_reticles_scope sequence 1, frame 0) at (+105, +105) from the centre
  (one window) or (+67, +67) (split screen) while zoomed; the sniper
  rifle shows "2x" or "10x" (frame 0 or 1) at (+150, +104) (one window)
  or (+95, +60) (split screen).

### 9. Scopes and screen effects

**Sniper rifle (disc, ref).** Zoomed, the weapon HUD's crosshair
elements with map type 1 draw the scope furniture (all with flags
"not a sprite" and "only when zoomed", plain colour 0x78000000: a black
tint at alpha 120): a vertical bar 189 pixels left and right of the centre
(64 by 256 bitmaps), two horizontal ticks (128 by 8 bitmaps, at +/- 93
pixels), a vertical tick (8 by 128, 57 below the centre); the split
screen has the smaller set (`sniper_scope_crosshairs_sm`). A **caption**
sprite ("TARGET DISTANCE:" over "TARGET ELEVATION:", 128 by 32) draws at
(+63, +162) from the centre, only when zoomed, plain colour 0x01000000
(invisible) and the flash colour (0x782896FF) while the aim value is > 0:
it appears with a target. Beside it two numbers (state 6 and 7, 4 digits
and 3 fractional digits, leading zeros, "only when zoomed" flagged,
colour 0x802896FF) show the target's distance and the height difference
in metres, drawn only while a target holds the aim assist at full
strength; their top-left origins are (397, 349) and (397, 364) from the
safe frame's top left (screen x 445, y 385 and 400). Two **angle-tick strips**
(statics, state 3 map type 1, top-left (92, 85) and (445, 85), 8 by 256,
screen x 140 and 493, y 121) carry one multitexture overlay each: a mask
and a tick bitmap whose second map slides vertically with the player's
pitch (the effector maps pitch from -1.4923 rad (offset 1.0) to +1.4923
rad (offset 0.0)) and fades in with the zoom level (alpha 0 at level -1,
1 from level 0), so the pitch ladder appears only zoomed.

**The screen effect** (`wphi` block; pistol, rocket launcher, sniper
rifle): once a frame per view, from the weapon HUD of the current weapon
(the same selection as section 7, with the HUD shown), **after** the
whole 3D scene (level, objects, the first-person weapon, particles,
lens flares) and **before** the HUD and the screen flash, which it never
touches. **(ref; masks and values disc)**

*Which parameters are in force* (zoomed = the player's zoom level is
not none; every disc screen effect has all its "only when zoomed" bits
set, so nothing at all happens unzoomed):

- **Mask**: the full-screen mask with one view, the split-screen mask
  with several. **It is not a HUD picture** (no placement, tint or HUD
  blend): it is a texture read by the post-processing pass below.
- **Convolution** (one view only, never in split screen): radius r =
  the out upper bound when the in bounds are equal, else the out bounds
  interpolated by t = clamp((v - in lower) / (in upper - in lower), 0,
  1), v the render camera's vertical field of view (the unit's nominal
  angle after the conversion of `weapons.md` section 12, divided by the
  zoom). r > 0 selects the **warp** convolution (the only kind the HUD
  uses).
- **Light enhancement** intensity I_L = the tag's intensity x (the
  flashlight power, 0..1, when bit 1 is set) x clamp(script value of
  the source, 0, 1); used when above 0. **Desaturation** intensity I_D
  likewise, with its tint and "additive" bit. The script values (four,
  indices 0 to 3) are all set to 1 when a map loads and change only by
  script, so in multiplayer the factor is 1.

| Weapon HUD | Masks (full / split) | Radius out bounds | r at 2x | r at 8x | Light enhancement | Desaturation |
|---|---|---|---|---|---|---|
| pistol | `pistol_scope_mask` (256 x 256, A8Y8) / `pistol_scope_mask split` (256 x 256, AY8) | 0 to 15 | 10.81 | - | none | none |
| rocket launcher | the pistol's two masks | 0 to 11 | 7.92 | - | none | none |
| sniper rifle | `sniper_scope_mask2` (512 x 256, A8Y8) / `sniper_scope_mask_sm` (256 x 256, AY8) | 0 to 11 | 7.92 | 11 | flags 3 (zoomed only, flashlight), source 1, intensity 1; does **not** use the mask | flags 15 (zoomed only, flashlight, additive, uses the mask), source 1, intensity 1, tint (0, 1, 0) |

All three: in bounds (1.1234, 0.1146) rad (64.4 to 6.6 degrees),
convolution and mask flags "only when zoomed". At 2x v = 0.39672 and
t = 0.7203; at 8x v = 0.09747 and t clamps to 1. No other weapon or
vehicle HUD on the disc has a screen effect.

**The mask's two channels.** The full-screen masks are A8Y8: a grey
level Y and a separate alpha A (sampled as RGB = Y, Y, Y and alpha =
A); the split-screen masks are AY8 (one channel serving as both, Y =
A). They mean different things:

- **Y (the grey level) darkens**: the final picture is multiplied by
  (1 - Y). Y is 0 in the scope's window, 1 on a thin frame line, and
  about 0.5 outside (so the outside is dimmed to half, not blacked out).
- **A (the alpha) selects the blur**: A = 0 keeps the scene sharp, A = 1
  shows the convolution's blurred picture, values between blend
  linearly. A is 0 only in a small soft-edged centre and 1 over most of
  the screen.

**How the mask is addressed.** One mask texel per viewport pixel, the
mask centred on the viewport: viewport pixel column x shows mask column
x - (W - S_w) / 2 and row y shows mask row y - (H - S_h) / 2 (W, H the
viewport's size, S the mask's), exactly on texel centres; in split
screen the mask is further shifted by the offset between the view's and
the window's centres. It is **not** stretched. Filtering is bilinear and
addressing **clamps** to the edge, so every pixel beyond the texture
takes the mask's border texel: on all four disc full-screen and split
masks the whole border is Y = 128 or 132 (0.50 or 0.52) with A = 255
(AY8: 128 or 132 for both), i.e. "dim to half and blur". (So on a 640
by 480 view the 512 by 256 sniper mask covers columns 64 to 575 and rows
112 to 367, and the pistol's 256 by 256 mask columns 192 to 447 and
rows 112 to 367; everything outside is border.) The Xbox's views are
640 by 480 (or its split-screen parts); at another resolution (ours, not
the original's) treat a mask texel and r as one 480-line pixel: scale
both by the view height / 480 about the centre, so the window keeps its
size relative to the screen's height.

**The pass, step by step** (one view with a convolution; two passes,
the first drawing into an off-screen copy, the second back into the
frame; texture reads bilinear and clamped):

1. For every pixel p of the view, three samples of the current picture
   are taken: S0 at p itself, S1 at c + (p - c) x (1 - r / W, 1 - r /
   H) and S2 at c + (p - c) x (1 + r / W, 1 + r / H), c the view's
   centre (each axis scaled about the centre: S1 is pulled toward the
   centre and S2 pushed away from it by r x (distance from the centre on
   that axis) / (view size on that axis) pixels, which is r / 2 pixels
   at every edge of the view and 0 at the centre).
2. Blurred value B = (S0 + S1 + S2) / 3 (equal weights).
3. Out = (1 - A) x S0 + A x B, A the mask's alpha at p.
4. The second pass repeats 1 to 3 on the first pass's output with the
   same r and mask, so where A = 1 the picture is two compounded radial
   blurs (samples spread over -2 to +2 steps with weights 1, 2, 3, 2, 1
   over 9), while where A = 0 it stays exactly the original scene.
5. On the second (last) pass only: the colour filters (below), then the
   darkening: written colour = colour x (1 - Y). (The first pass writes
   its colour unchanged.)

With no convolution (split screen) the first pass copies the frame and
the second writes the copy back through step 5 alone: the scene, dimmed
outside the window, filters applied, no blur.

**Light enhancement** (per channel, on the colour C after the blur):
C_L = (1 - a_L) x C + a_L x (1 - (1 - C)^4), with a_L = I_L, or with
the "uses mask" bit a_L = I_L x clamp(1 - 2 Y, 0, 1) (full where Y = 0,
none where Y >= 0.5). At a_L = 1 it brightens darks strongly: 0.1
becomes 0.344, 0.2 becomes 0.590, 0.5 becomes 0.938.

**Desaturation**: L = (C_L.r + C_L.g + C_L.b) / 3 (equal weights);
a_D = I_D, or I_D x clamp(1 - 2 Y, 0, 1) with the "uses mask" bit.
Additive: colour = C_L + a_D x L x tint. Not additive: colour = (1 -
a_D) x C_L + a_D x L x tint. The result is clamped to 0..1, then step
5's darkening.

So the sniper's night vision (flashlight on while zoomed, power 1): the
whole view is brightened by the curve, then inside the frame (Y = 0)
the green channel gains the brightened grey level L (a dark grey 0.1
becomes (0.344, 0.688, 0.344)); in the dimmed border (Y about 0.5) no
green is added and the brightened picture is halved; the frame line
stays black. While the flashlight power ramps, a_L and a_D follow it.

**The expected pictures** (one 640 by 480 view, centre (320, 240);
pixel positions are the mask's, mapped as above; they agree with the
scope furniture of the HUD):

- **Sniper rifle, 2x (r = 7.92).** A sharp, full-brightness rectangle,
  columns 127 to 512 and rows 120 to 359 (mask Y = 0), outlined by a
  black line two pixels wide just outside it (columns 125, 126, 513,
  514; rows 118, 119, 360, 361); the HUD's vertical bars at +/- 189
  pixels sit on its left and right edges. Outside the rectangle the
  scene is at half brightness (x 0.498) out to the screen's edges, and
  fully blurred. Inside, the blur weight A is 0 in a central band
  (about columns 234 to 405 on the middle row, rows 226 to 253 on the
  middle column), 0.5 at about columns 208 and 431 and rows 200 and 279,
  and 1 from about column 144 / 495 and row 136 / 343 outward. The blur
  is radial: per pass the outer samples sit d x 7.92 / 640 pixels either
  side horizontally (d the distance from the centre): 2.4 pixels at the
  rectangle's side edges, 4.0 at the screen's side edges, 2.0 at the
  rectangle's top and bottom (d x 7.92 / 480); after the two passes the
  spread is twice that. The look is a soft smear growing outward, never
  darkness: the only black is the frame line.
- **Sniper rifle, 8x (r = 11).** The same frame, dimming and blur
  weights; the smear is 11 / 7.92 = 1.39 times wider (3.3 pixels per
  pass at the rectangle's side edges, 5.5 at the screen's).
- **Sniper rifle with night vision**: as above plus the brightening
  everywhere and the green tint inside the rectangle only.
- **Pistol, 2x (r = 10.81).** A full-brightness disc of radius about
  119 pixels (mask Y = 0 for columns and rows 9 to 246 through the
  centre) centred on the view, ringed by a dark line about two pixels
  wide (one texel of Y = 1 and a partial one, Y 0.4 to 0.9, beside it
  as the circle crosses the texel grid), and outside the
  disc the scene at x 0.482 brightness (Y = 132) out to the screen's
  edges. The blur weight is 0 only within about 6 pixels of the centre,
  0.5 at about 37 pixels, 1 beyond about 70 pixels: most of the disc is
  softly blurred, sharp only at the crosshair. Per pass the outer
  samples are d x 10.81 / 640 pixels either side horizontally: 1.2
  pixels at d = 70, 2.0 at the disc's edge, 5.4 at the screen's sides.
- **Rocket launcher, 2x (r = 7.92).** The pistol's disc, ring, dimming
  and blur weights with the smaller radius (1.5 pixels at the disc's
  edge).
- **Split screen** (any of them zoomed): no blur; the split mask's
  window (the sniper's: columns 11 to 244, rows 53 to 202 of its 256 by
  256 mask; the pistol's: about 76 texels in radius) at full
  brightness, the rest dimmed by Y = A = 128 or 132 (to about half);
  night vision as above.

**Certainty.** From the reference: which textures and passes are bound,
the mask's texel-per-pixel centred addressing with clamping and
bilinear filtering, the three sample positions, and the combiner
arithmetic of steps 2 to 5 and the filters (read from the pass's
register-combiner setup: the alpha portion's input reads the mask's
blue, which is Y for these formats). From the disc: the masks' formats
and pixels, the values in the table. Inferred: that the two passes'
sample positions are assigned to the stages as described (the
in-place sample is the one with no offset, which is the one the mask
mixes against); that A8Y8 samples as (Y, Y, Y, A) on the Xbox (the
usual luminance-alpha meaning). A port that is unsure of the blur
should keep the darkening by Y exactly and may degrade the blur to "no
blur" without changing the picture's layout.

### 10. Grenades

**(ref; values disc)** Drawn after the weapon HUD when the unit has a
current grenade type with a HUD tag, is not in a vehicle seat that
forbids it and the weapon does not prevent throwing. count = the current
type's grenade count. Flags: flashing when count <= the HUD's flash cutoff
(1); disabled when count = 0. Flash start tick latch as for the weapon
states.

1. the background plate (if its bitmap exists), then the count plate
   (the outline), each with the flags above;
2. the count number (if digits > 0): value = count; the number procedure
   with the grenade HUD's anchor (top left);
3. overlay items (if the overlay bitmap exists) with the grenade mask:
   flashing (1) when count <= cutoff, empty (2) when count = 0, default (4)
   when neither, always (8): the frag HUD's icon appears in an alert
   colour (0xFFFFE6E6) at count 1 and 0, and in its normal colour
   (0xFFE6F0FF) at 2 or more; likewise the plasma HUD's.

The type shown is the unit's current grenade type, selected and
switched by the rules of `weapons.md` section 14 ("Selecting the grenade
type"): frag after a stock spawn, plasma after a spawn with both kinds
(infinite grenades), the other kind automatically once the shown kind
runs out and the other has grenades, and still the empty kind (count 0,
disabled colours) when both are empty. Nothing is drawn while the type
is none (the spawn tick, or a spawn with no grenades). With count 0 the plates
draw in their disabled colours (0xC8FF552B and 0xDCFF552B, an orange red)
and the number shows "0" in its disabled colour 0xFFFFE6E6; the overlay
mask at count 0 is flashing + empty + always, so item 0 (type 1) still
draws, in its alert colour.

### 11. The motion tracker

**(ref; numbers disc)** Drawn with the unit HUD when the game type shows
the motion sensor to this player (`multiplayer_rules.md` section 7: the
sensor widget is on when the variant's radar flag is set or the objective
indicator is "motion tracker") and the script has not hidden it.

**Panel.** Anchored bottom left regardless of the unit HUD's anchor. The
background (the dark disc, `hud_weapon_backgrounds` sequence 3 on the
Spartan, 128 by 128) and the foreground (`hud_unit_backgrounds` sequence
6: the view cone and the "15m" label, 128 by 128) are static elements at
offset (0, 0): one window rectangle (48, 316) to (176, 444). The blip
placement (41, 41) from the same corner gives the disc's centre: pixel
(89, 403). Split screen uses the second HUD's 64 by 64 pieces and blip
placement (31, 31). The panel's flash only runs when a script blinks it.

**Who shows** (`multiplayer_rules.md` owns the rule; the numbers are `hudg`
values of the disc): a unit shows when it is alive and (its primary
trigger is held, or it is throwing a grenade (any grenade throw state but
idle and ending), or its squared speed is at least 0.0002 (WU/tick)^2),
and its horizontal distance to the viewer's camera is at most 20 WU (the
reference ignores height in multiplayer). Camouflage hides a unit in
single player but not in a game. At most 16 blips per viewer.

**The blip list.** Every tick the tracker advances a ring of 10
history entries (the index moves each tick). On ticks that are a multiple
of 15 (and tick 0) the list is rebuilt: every unit that shows and is in
range takes a blip slot, in object order, until 16 are taken: its type
(below) and size. On the other 14 ticks the new entry is a copy of the
previous one. Each frame, for the newest entry only: the reference point
is the camera position; for each blip's unit, if it still shows and is
within the range (horizontal distance), its stored position is
refreshed, else the blip is removed from that entry (older entries keep
it: a unit that stops leaves a short fading echo). A stored position is
the horizontal displacement (dx, dy) from the reference point, each
component quantised to a signed byte: trunc(component / 20 x 127) (so
the resolution is 0.157 WU), and the entry also stores the yaw (the
facing yaw + pi/2). **Blip types:** self (the viewer's own unit), friend
(same team), enemy, vehicle friend, vehicle enemy, goal (custom): colours
(1, 0.5, 0) orange, (1, 1, 0) yellow, (1, 0, 0) red, (1, 1, 0), (1, 0, 0),
(0.5, 0.5, 1) blue-violet. A unit is an enemy of the viewer when
`multiplayer_rules.md`'s team rule says so (everyone is in free-for-all).
A vehicle with a gunner or driver takes the seated unit's team; an empty
vehicle is "vehicle friend" (a drop ship is "vehicle enemy"). The
viewer's own unit is among the scanned units, so while it moves or fires
it adds a "self" blip at the disc's centre (open question 10). **Blip
size** class (the unit's `motion sensor blip size`: 0 medium, 1 small, 2
large; the Spartan is 0) gives a base size 0, -0.75 or 1.

**Drawing a blip** (per history entry h = 0 newest to 9 oldest, per blip
in it):

1. Decode the stored (dx, dy): each byte x 20 / 127 WU (the world frame:
   +x forward at yaw 0, +y left).
2. Rotate by -yaw_h, yaw_h being that entry's stored facing + pi/2. The
   result: with the viewer facing any direction, a unit straight ahead of
   the viewer lands straight **up** from the disc's centre, a unit to the
   viewer's left lands to the **left**, behind lands **down**.
3. d = the length of the rotated vector, floored at 0.015625 only for
   the division in step 4 (the position is the unit direction, the
   vector divided by d, times r: a zero vector, a unit within 0.157 WU
   on both axes, is drawn exactly at the centre). If d >= 20
   the blip is not drawn (it can leave the disc between the scan and the
   draw as the viewer moves). The comparison is strict in the drawing
   (d squared below 400 draws) and not strict in the scan (a unit whose
   squared horizontal distance is at most 400 is taken). The decode
   multiplies the byte by 20 and by the single-precision 1/127
   (0.0078740157), which makes byte 127 exactly 20.0 (not 19.99999), so
   **a unit at exactly 20 WU along an axis is scanned (byte 127) but its
   blip is not drawn: it does not show.** A unit just inside the range
   decodes shorter (the quantisation truncates toward zero: 19.9 WU is
   byte 126, 19.84 WU), so it does show.
4. Distance curve: the drawn radius is r = 42 x (d / 20)^0.7 pixels from
   the disc's centre for the layout scale 32 (general form: the blip's
   clip position is (scale / 32) x (d / 20)^0.7 of the half field, and the
   field is 2 x 42 pixels, or 2 x 32 in split screen). So a unit at 10 WU
   (byte 63, decoded 9.92 WU) is 25.7 pixels out and one at 1 WU (byte 6,
   decoded 0.945 WU) is 5.0 pixels out (5.2 if it were not quantised;
   linear would be 21 and 2.1): near units are pushed outward.
5. Brightness w x w with w = (10 - h) / 10 (1.0 for the newest entry,
   0.01 for the oldest). Size = base + p x q with q = (1 - w)^3.5 x 7 + 1
   (1.0 for the newest, 5.84 for the oldest: the echo grows as it fades)
   and p = 1 (for a goal blip p = (sin(0.1047198 x tick) + 1) / 3 + 1, a
   slow pulse).
6. The blip is the `hud_sensor_blip` sprite (a soft dot, 32 by 32; goals
   use `hud_sensor_blip_custom`) drawn as a square of half-extent size x
   0.0625 of the half field, i.e. size x 2.625 pixels (1.0 gives a 5 pixel
   square), tinted by the type's colour times the brightness, **added**
   into the tracker's layer.

**The sweep.** One image pass over the blip layer: let
t = game_time / 30 mod 2.1 s; s = 1 / ((t + 0.0625) x 1.1) while
t < 2.0375, else 0.4. The sweep bitmap (`hud_sweeper`, 64 by 64, an
alpha ramp from 0.27 at the centre to 1 at a thin ring about 0.45 of the
image out) is sampled over a window of half-size s / 2 about its centre;
the layer is blended with it (the sweep adds its light blue
(0.4588, 0.7294, 1.0) and scales what is already there by its alpha:
the blips are dim (27 percent) except where the ring passes, so the
ring "reveals" them), then multiplied by the disc mask
(`hud_sweeper_mask`), then added to the frame. The ring starts at the
centre and reaches the disc's edge after about 0.9 s of each 2.1 s
period; the rest of the cycle the blips are dim. **(inf: the
compositing of the sweep follows the reference's blend setup; its look
should be compared with the real game, open question 5.)**

### 12. Damage indicators

**(ref; numbers disc)** When the player takes damage (`effects.md`
owns the screen flash, camera shake and the sound) from a source object
(total damage above 0), the player's four counters (top, left, bottom,
right) get 1 for the directions the source lies in; a damage effect
with the "draw indicators down" flag sets only the bottom one. The
direction: the attacker's origin minus the head position, expressed in
the camera's frame as (right, forward, up) and normalised; if |up|
exceeds 0.5 the top (attacker above) or bottom (below) counter is set;
and with a = atan2(forward, right), if a < pi/4 or a > 3 pi/4 the
**left** counter is set when |a| > pi/2 else the **right** counter.
(That is the reference's literal test; it leaves a hole in front of the
player and sends everything behind to the left or right. Open question 6.)

Each frame the counters of a living player's window are read and
incremented by the elapsed ticks (saturating at 255); a counter that
is 1 to 29 (so for just under one second after the damage) draws an
arrow; a counter at 0 or at 30 and above draws nothing. When the player
has no unit, all counters are cleared. Every arrow is the
`hud_damage_arrows` sprite (sequence 0, 231 by 40: a wedge pointing at
the screen edge; split screen: sequence 1, 165 by 29), **centred** on its
point, coloured 0xC8FF0000 (red, alpha 200, no fade, no flash), at scale
1, rotated about its centre: top, rotated 180 degrees, at y = the
window's top edge + 10, x = the viewport's middle; left, rotated 90
degrees, at x = the window's left edge + 10, y = the viewport's middle;
bottom, not rotated, at y = the window's bottom edge - 10; right,
rotated 270 degrees, at x = the window's right edge - 10 (the offsets
are the globals' four 10s). With one window the centres are (320, 46),
(58, 240), (320, 434) and (582, 240).

### 13. Messages and prompts

**(ref; texts disc)** The window's message block starts at the globals'
messaging placement: top left, offset (0, 60): pixel (48, 96) in one
window. Lines are in the globals' font (single window: `ui\large_ui`;
split screen: `ui\small_ui`). One window: line height = leading +
descending + ascending height of the font = 0 + 4 + 17 = 21. Split screen:
leading + ascending = 3 + 14 = 17, and 17 pixels are subtracted from the
first line's top. The next line's top = trunc(previous top + spacing 1.35
x line height) (one window: tops 96, 124, 152, 180, 208: 28 pixels
apart). Text is left-aligned from the block's x; colours below.

**The prompt line** (the "state message") takes the first line when
there is one. The reference picks it each frame from what the player can
do (the action the unit would take, `objects.md`'s hud text message
index) in this order: respawn failures (when dead); otherwise by the
player's action result: pick up a powerup or weapon ("pickup": "Hold
[action] to pick up [name or weapon picture]"), swap for a powerup ("swap
powerup": "Press [action] to swap [held] for [new]"), exit a vehicle,
touch a device ("Press [action] to touch device [name]", or the control's
own text: custom device), swap for a weapon (the held weapon's `messaging
icon` picture replaces the name), add a weapon to the inventory,
enter or evict from a vehicle ("Press [action] to enter [seat] seat of
[vehicle]"), flip a vehicle; otherwise a game type's custom text; otherwise,
when the held weapon is empty (loaded maximum above 0, nothing loaded and
no reserve, or battery spent) and another weapon in the inventory is not,
"remind to switch weapons" ("Press [rotate weapons] to switch to [picture]").
The prompt text is the `hmt ` message's elements in order: text runs
and icon elements: button icons come from the globals' button icon list
(an icon number 18 "action" maps through the player's control layout to
the button picture, e.g. the X button), "custom 1" and "custom 2" take
the item name (a picture from the weapon HUD's messaging icon, or text
from the object's `hud text message index` into the alternate icon
strings, or a scenario name) and a second name. Icon pictures come from
the globals' messaging icon sprite page. An icon's pixel placement:
at x = cursor + the icon's x offset, y = line bottom - its y offset, then
the cursor advances by the sprite's width + the width offset (or just the
width offset with "absolute width"); the text then continues three
pixels back (the reference's tightening). The prompt text's colour is the
help colour block when a script help text is up (flashing when
scripted), else the messaging "state color" (0.5 alpha, rgb (0.4588,
0.7294, 1.0)).

**Item messages** (pickups): when the player picks up a weapon,
ammunition, grenade or powerup the HUD adds a message keyed by the item
tag: its `message index` (item layer 0x180) is an index into
`ui\hud\hud_item_messages`; ammunition pickups use the weapon's index
plus 1 and the count (divided by the item's `hud message value scale`, at
least 1), e.g. "Picked up an assault rifle" (4), "Picked up %d rounds for
assault rifle" (5). A grenade pickup uses the grenade's index (2 or 40)
and +1 for a count above 1. A message with the same item replaces its
older line (and its count adds). Up to four messages per player (three
in split screen), newest first, sorted by time (ties by item then
creation order); a full list overwrites the oldest. **Timing:** a message
is fully drawn (text colour (0.5, 0.4588, 0.7294, 1.0) alpha-first)
for the globals' up time (2.0 s = 60 ticks), then fades: alpha x
((1 - (elapsed - 60) / 60)^1.9) over the fade time (2.0 s), and is
removed after up + fade (4.0 s = 120 ticks). With a prompt line up the
messages start at the next line (prompt line height x 1.35 below); one
fewer message fits (the leave-blank rule keeps the first line blank for a
moment after a prompt ends). The loading and checkpoint texts (items 42
to 45) are single-player.

**Other text** (informational: the party app draws its own): the
multiplayer score and the match timer, the kill messages, player names and
chat are not part of the HUD tags and are drawn by the game engine's own
text (kill and event lines go through the same message lines); the
scripted timer (number glyphs, two digits each of minutes, seconds,
hundredths, its colour block `timer` with the "time up" block when it
runs out) and the objective text (the objective colour block's up and
fade ticks) are campaign scripting.

### 14. Waypoints and friend markers

**Teammate triangle (ref).** In games with team indicators (and in
single player), every living teammate gets the `hud_multiplayer` bitmap
(a green downward triangle, 64 by 32) at the screen position of the point
0.3 WU above the head when it is on screen. Its top is cut by a fraction
growing with distance: the sprite's v range starts at (1 - fade) / 2 of
the way down with fade = clamp(1 - (depth - near) x 100 / (far - near),
0.075, 1) (so far markers are shorter), drawn at scale 1.

**Waypoints (ref; disc arrows).** In multiplayer the waypoints are the
game type's goals (`multiplayer_rules.md` section 7.1: which objects,
where, who sees each), drawn when the variant's objective indicator is
"nav points" and the viewer has a living unit, every goal the viewer may
see, each by its arrow name; the game types name only `default`,
`flag_blue`, `ball_blue`, `crown_blue` and `target_blue`, so **every
objective marker is blue (0x2896FF) for both teams**. The disc's ten
arrows (`default`, `flag`, `target`,
`ball`, `crown`, each `_blue` or `_red`): colour 0x2896FF (blue team) or
0xFF552B (red team), opacity 0.5, fade 0, sequences on screen / off
screen / occluded: default (1, 0, 6), flag (2, 0, 6), target (3, 0, 6),
ball (4, 0, 6), crown (5, 0, 6) in `hud_waypoints` (0 a downward
triangle, 1 a double chevron, 2 a flag, 3 a target ring, 4 a skull, 5 a
crown, 6 a diamond; each 34 by 34). Per frame: its world point (for a
goal, the point the game type set, already raised 0.63 WU) is
projected; the waypoint is "occluded" when a line-of-sight ray from the
player's head to the point (ignoring the player's own unit) hits
anything (for a goal there is no reference object, so the carried flag's
carrier or the ball's holder occludes it too: the arrow is drawn with its
occluded sequence, the diamond), and "on screen" otherwise; "off
screen" when it projects outside the screen or the ellipse below. Scale = 0.5 beyond 15 WU, else
(1 - d / 15)^0.7 + 0.5, d the distance from the player's camera
position to the point (the drawn metres use the same d x 3.048). The ellipse has semi-axes a = (window width -
(left + right offset)) / 2 = (544 - 40) / 2 = 252 and b = (408 - 40) / 2 =
184, centred at the window centre; a waypoint outside it (or off screen)
is moved onto it along the ray from the centre (factor sqrt(a^2 b^2 /
(b^2 x^2 + a^2 y^2))) and its arrow is rotated to point outward
(-atan2(x, y)) unless "don't rotate off-screen". The arrow's colour is
its colour times (1 - fade) with alpha 255 x round(opacity) (the
reference rounds the opacity first: 0.5 rounds to 0 or 1, open question
9). On screen the distance in metres (3 integer digits, 1 fractional,
trailing "m", leading zeros) is drawn under the arrow. Which objective
each game type marks is `multiplayer_rules.md`'s.

### 15. What hides or changes the HUD; split screen

- Script flags (campaign): show HUD, show HUD help text, per-panel show
  and blink flags (health, shield, motion sensor), show crosshair. A
  multiplayer game uses none.
- **Dead** (no unit): no weapon, unit, waypoint or damage pieces; the
  messages and respawn failure prompts show; the damage counters clear.
- **In a cinematic or a scripted or neutral camera:** none of the pieces
  of section 2 draw and the message block is hidden; the unit warning
  sounds keep being updated.
- **Zoomed:** nothing in the HUD turns off; the crosshair items'
  "only when zoomed" and "don't show when zoomed" flags choose (section 8),
  the sniper's angle ticks fade in (alpha by the zoom level), the sniper's
  range numbers keep their place, and the screen effects (section 9)
  start. The motion tracker, shield, health and ammunition stay.
- **In a vehicle:** the unit's HUD chain adds the vehicle's HUD (the
  driver's `warthog driver` shows a reticle picture; seats have their own);
  out of this spec's scope.
- **Split screen** (two to four local players on one console): each
  window is a part of the screen: with n = 2 the screen is split into
  upper and lower windows: the window rectangle for player 0 is (48, 36)
  to (588, 236) and for player 1 (48, 244) to (588, 444) (the frame split
  at its middle with a 4 pixel inset between and on the right), the
  viewport (pixel rectangle) y 0 to 240 and 240 to 480; with three or
  four players the windows are halves of that. Anchors measure from the
  window rectangle, drawn relative to the window's viewport. Differences:
  the second unit HUD (section 2), the second font (small), the message
  count (3), crosshair items drawn at 0.5 size (unless "don't scale
  size"), elements marked "split screen only" drawn and "one window only"
  ones not, offsets multiplied by the HUD scale unless "don't scale
  offset" (the reference's scale function returns 1, the globals' field
  says 0.75: open question 3), motion sensor image 64 pixels with
  the blip field 32 pixels half-size, and the message icons drawn at 0.75
  size.
  Convolution (the zoom warp) is off.

### 16. Widescreen (ours to choose)

The original draws everything at 640 by 480 and lets the television
stretch it. For another aspect ratio our rule (not the original's):

- Keep the 640 by 480 design canvas and the 48 and 36 pixel safe insets.
- Uniform scale by height only: u = window height / 480; the virtual
  canvas is (window width / u) wide; the safe frame is x 48 to
  (window width / u - 48), y 36 to 444; centre anchors use the middle of
  that frame.
- Left anchors keep their pixel offsets from the left safe edge, right
  anchors from the right safe edge, so the shield and the sensor move to
  the new corners; the centre elements (crosshair, warnings) stay central.
- Elements the original lays out relative to the *whole* 4:3 picture
  rather than a corner (the sniper scope's furniture, anchored top left
  but symmetric about the centre of the 640 canvas) must be shifted by
  half the added width to stay symmetric about the screen's centre.
- The HUD never scales with the window width (the shell's overlay rule
  in `docs/DESIGN.md` is its own and separate).

A 4:3 window reproduces the original exactly.

### 17. On-gun displays (the first-person model)

**(ref; shaders and tags disc)** The assault rifle's loaded-round counter,
the compass strip and the small icons on its display are **not HUD**: they
are shaders on the first-person model `weapons\assault rifle\fp\fp`.
The tag that owns the counter is the shader `weapons\assault
rifle\fp\shaders\numbers` (a transparent-chicago shader with the "numeric"
flag, limit 60, drawing the digits of `numbers_plate`; its two digit
positions come from the model's geometry). Its number is read by the
numeric-shader rule of `shaders.md` ("Numeric shaders": value = clamp(floor
(limit x source + 0.5), 0, limit), source = the object's function A when
the digit bitmap has other than 8 frames) from **function A of the
weapon object**. The weapon object's function 0 (usage "ammo display")
scales its output by the object's incoming input **B**; the weapon part
exports "primary ammunition" into incoming B (the weapon's input sources
0x28 of the weapon part: A = 14 integrated light, B = 2 primary
ammunition, C = 12 illumination, D = 0): primary ammunition = rounds
loaded / magazine size. So the display shows the loaded rounds (0 to 60):
loaded / 60 x 60, rounded. The compass strip is limit 8 and reads function
D, which scales by input D = 19 (compass: the holder's facing, object
layer). The plasma rifle's and plasma pistol's on-gun gauges are meter
shaders (`weapons\plasma rifle\fp\shaders\gauge`, `weapons\plasma
pistol\shaders\pp gauge`) whose value source is function B (the rifle:
input B is "illumination", which tracks heat) or D (the pistol: input D
is "age", the battery); the flamethrower's three digit meters (`ft
hundreds`, `ft tens`, `ft ones`) take value sources D, A and C of its
object's functions (the plasma and flamethrower routing is inferred from
the same pattern; their object functions were not traced). The model
shows these only in first person and the HUD never draws them.

## Per-tick order

The HUD has a tick part (state that must advance at the fixed rate) and a
frame part (drawing). **(ref)**

Once per game tick (`hud update`), in this order:

1. the weapon crosshair states of every local player (section 8: the
   `result` of each valid crosshair type; latches start and clear);
2. the unit HUD bookkeeping of every local player (section 5: L, f, t,
   and the first-update initialisation);
3. the waypoint list's per-player update (drop waypoints whose object is
   dead or gone; refresh each one's occlusion test);
4. the messaging globals' per-tick counter reset (message creation
   order numbers restart);
5. in a game forced to a single screen, the warning sounds of every
   local player (shown HUD off).

Once per drawn frame, per local player's window (section 2's order):

6. the motion tracker's clock: for the first local player only and not
   while the game is paused: the sweep value (section 11) is refreshed
   and, if the tick differs from the last update's, the blip history
   advances by one entry (rebuilt on every 15th tick, copied otherwise);
7. the drawing steps 1 to 10 of section 2, each reading the tick
   state above and the live unit and weapon state; the unit warning
   sounds are updated between the grenade plate and the unit HUD.

Messages (pickups) are added by the game events that cause them, at any
point of the tick; the message and prompt lines read the current tick.

## Edge cases

- **No weapon (hands empty):** the default HUD (`ui\hud\empty`) draws only
  its crosshairs: a small aim dot, "No Weapon" (type 8, always active
  because loaded and reserve are 0 and the interface state is zero:
  it shows only while the primary trigger is held, then latches 3 s) and
  "No Grenades". No plate, no grenade plate while the unit has no
  grenade type with a HUD.
- **Weapon swap:** the weapon HUD's flash start ticks are per state, not
  per weapon: a plate that was flashing for one weapon keeps its phase for
  the next weapon until the state stops flashing. The crosshair states
  reset (to 0) only when the unit ends up holding nothing. The new
  weapon's cutoffs and chain apply from the next tick's update; a frame
  drawn between the swap and that update draws the new HUD's elements
  with the previous states.
- **Number wider than its digits:** only `digits` digits draw (the value
  modulo 10^digits); "Km" appears only with the trailing-"m" flag.
- **Value 0:** without "show leading zeros" a number of value 0 draws
  nothing at all (the grenade count and ammunition numbers on the disc
  carry the flag, so they draw 0 and 000).
- **A sequence index outside the bitmap's sequences, or no sprites:** the
  element draws nothing (the reference reports once, silently).
- **Overshield stages beyond the third:** unreachable (the shield tops
  out at 3.0); the colour table is kept for completeness.
- **Dead and respawning:** the player's counters reset and the L, f, t
  state re-initialises when the next unit appears (the last-unit memory
  is NONE).
- **Meter byte overflow:** the weapon meters take the raw count as a
  byte (cast, not clamped) and clamp only the alpha result to 255; the
  disc's counts (at most 60 loaded rounds, 255 for heat) stay below 256.
  The unit meters clamp the value itself to 255.
- **Secondary magazine:** no disc HUD element uses states 4 or 5 or the
  secondary crosshair types, so the second magazine never shows.
- **A weapon swapped during a latched warning:** the latch lives in the
  crosshair state, which resets only when the unit ends up holding no
  weapon; a type that the new weapon's chain does not list is neither
  updated nor drawn.
- **Missing parents, or a parent chain that loops:** the chain stops at 16
  levels (the reference reports an error); none on the disc.

## Quirks

Behaviour that looks wrong and is part of how the game feels; keep it
unless said:

1. **The unit HUD pair is chosen by local player count**, not by game
   type (section 2).
2. **The overshield's colours are in the engine** (stage 1 red, 2 green,
   3 yellow, 4 violet), the tag's overcharge fields are zero and
   ignored.
3. **A hit marks the lost shield for 15 ticks and then snaps.** The fade
   variable the update maintains never shows (section 5).
4. **Weapon meters never show their flash colour** (the reference time is
   a tick, not a fraction). Keep: ammunition bars never flash; their
   plates and numbers do.
5. **The crosshair turns red only through the aim assist** (autoaim level
   exactly 1, an enemy-team target in the cone), never for friends, and
   never by the weapon's reticle ranges.
6. **The sniper's second zoom label says "10x"** while the zoom is 8x
   (`weapons.md`).
7. **A value of 0 prints nothing** without "show leading zeros".
8. **Digits overlap:** the glyph sprites are 18 pixels wide and advance 9
   (screen width), so each 12 pixel glyph overlaps its neighbour's
   sprite; the italic art hides it.
9. **The number cursor formulas are not the pictures' extents:** for a
   left anchor the rightmost sprite starts (D - 2) advances right of the
   offset, so a 3-digit number's leftmost sprite starts one advance left
   of the offset (the disc's offsets are authored for it, and the layout
   check shows the digits inside their plates).
10. **Warning words are invisible until they flash:** their plain colour
    is all zero and the flash colour peaks at alpha 0 (pure additive).
11. **The "empty" overlay state of the heat meter reads the battery**
    (section 7, as the reference is written).
12. **The damage-arrow direction test has a hole in front and sends
    everything behind to one side** (open question 6).
13. **The motion tracker shows the viewer's own moving unit** as an
    orange blip in the centre (open question 10).
14. **The shield flash plate and the health plate flash together** only
    when the shield is gone: the health plate's flash is "shield
    depleted", not "health low" (low health is only the colour change
    and the heart-beat sound).

## The disc's values

Everything here is read from `bloodgulch.map` (the other twelve
multiplayer maps hold the same tag counts; `ui.map` holds one `wphi`, two
`grhi`, the `hudg`, the `hud#`, the `hmt ` and 122 `ustr`). Rectangles are
for one local player (safe frame 48, 36, 592, 444), pixels, (left, top,
right, bottom), from the placement rule of Behaviour section 1; picture
sizes are the sprites' pixel sizes. "BG" is the page
`ui\hud\bitmaps\combined\hud_weapon_backgrounds` and "UB" is
`hud_unit_backgrounds`, "UM" `hud_unit_meters`, "AM" `hud_ammo_meters`,
"AA" `hud_ammo_alphas`, "AO" `hud_ammo_outlines`, "AT"
`hud_ammo_type_icons`, "RT" `hud_reticles`, "RW" `hud_reticle_warnings`,
all in `ui\hud\bitmaps\combined\`.

### Tags and counts

| Class | Count on a multiplayer map | Tags |
|---|---|---|
| `unhi` | 5 | `ui\hud\cyborg`, `ui\hud\cyborg_mp`, `ui\hud\warthog driver`, `ui\hud\passenger_2`, `ui\hud\gunner` |
| `wphi` | 17 | the nine weapons' HUDs (below), `vehicles\warthog\warthog gun`, `vehicles\scorpion\scorpion cannon`, `vehicles\ghost\ghost gun`, `ui\hud\master`, `ui\hud\master rounds`, `ui\hud\master plasma`, `ui\hud\master fuel`, `ui\hud\empty` |
| `grhi` | 2 | `ui\hud\default`, `ui\hud\plasma_flare` |
| `hudg` | 1 | `ui\hud\default` |
| `hud#` | 1 | `ui\hud\counter` |
| `hmt ` | 1 | `ui\hud\hud messages` |
| `ustr` | 8 | the two HUD lists below and six menu lists |

Parsed element counts over the 17 `wphi`: statics 15, meters 8,
numbers 6, crosshairs 31 (39 items), overlays 3 (3 items), screen
effects 3; over the two `grhi`: overlay items 4; the `hudg` has 18 button
icons and 10 waypoint arrows; the 8 `ustr` have 181 strings.

### The Spartan's unit HUDs

Both have anchor 1 (top right) and no HUD background bitmap. Colours as
0xAARRGGBB; "flash" is the colour block's flash colour with period /
delay / flashes / length.

| Piece | `ui\hud\cyborg` (one window) | `ui\hud\cyborg_mp` (split screen) |
|---|---|---|
| shield background | UB seq 0 (128 x 32), offset (0, 1), colour 0xDCAAD5FF, flash 0xFFFF552B 1.35 / 0.225 / 3 / 0.225; rect (464, 37, 592, 69) | UB seq 1 (128 x 32), (-7, 1), scaling flags 3, same colours |
| shield meter | UM seq 0 (122 x 31), (7, 0); min 0x0000509F, max 0x002896FF, flash 0x00FFFFFF, empty 0, flags 0, minimum 0, alpha 1 x v + 55, value scale 198, fade 0.5; rect (463, 36, 585, 67) | UM seq 1 (122 x 31), (0, 0), flags 3, same numbers |
| health background | UB seq 2 (128 x 16), (-5, 18), colour 0xDCAAD5FF, flash 0xFFFF552B 1.0 / 0 / 1 / 1.0; rect (469, 54, 597, 70) | no bitmap, (28, -3) |
| health meter | UM seq 2 (88 x 14), (36, 17); min 0x00FF0000, max 0x002896FF, flash 0x00FF552B, flags 1, minimum 31, alpha 30 x v + 15, value scale 0 (8), fade 0.75; rect (468, 53, 556, 67) | UM seq 3 (88 x 14), (29, 11), flags 3, same numbers |
| health extras | medium 0x00FFDF00, maximum cutoff 0.67, minimum cutoff 0.33 | same |
| sensor background | BG seq 3 (128 x 128), offset (0, 0), anchor bottom left: colour 0x28061428, flash 0x28FF0000 3.0 / 0.225 / 3 / 0.15; rect (48, 316, 176, 444) | BG seq 4 (64 x 64) |
| sensor foreground | UB seq 6 (128 x 128), colour 0xA06986A0, flash 0xFF00FF00 3.0 / 0.225 / 3 / 0.15 | UB seq 7 (64 x 64) |
| blip placement | (41, 41): the disc's centre at pixel (89, 403) | (31, 31) |
| flashlight (auxiliary meter, type 0) | background UB seq 4 (128 x 8), (-7, 34), colour 0xFFAAD5FF, flash 0xFFFF552B 3.0 / 0.225 / 3 / 0.225, rect (471, 70, 599, 78); meter UM seq 4 (130 x 10), (-7, 33), min 0x0000509F, max 0x002896FF, alpha 1 x v + 0, value scale 255, fade 0.75, rect (469, 69, 599, 79); minimum cutoff 0.25 | background UB seq 5, (-13, 21), flags 3; meter UM seq 5, (-13, 20), flags 3 |
| auxiliary overlays | none | none |

The flashlight meter draws while the unit's integrated light is on at
full power (its background flashes at or below 25 percent battery and is
shown briefly, flashing, when the player presses the key with under 20
percent); its value is the battery.

Warning sounds (both): `sound\sfx\ui\shield_hit` (`snd!`, state bit 1),
`sound\sfx\ui\shield_charge` (`lsnd`, 0), `sound\sfx\ui\shield_low`
(`lsnd`, 2), `sound\sfx\ui\shield_depleted` (`lsnd`, 3),
`sound\sfx\ui\health_low_heart` (`lsnd`, 4); scale 0 each. The three
vehicle unit HUDs have no sounds and no meters on the pieces checked
(the Warthog driver shows only a reticle picture, `RT` sequence 14, at
(0, -40) from the bottom centre, anchor 4).

### The nine weapons' HUDs

"Cutoffs" are total / loaded / heat / age. All crosshair colours are
0xAARRGGBB; "plain / flash" lists the colour block's colour and flash
colour. The shared chain is described below the table of the leaves.

| Weapon (`wphi` tag) | Parent | Cutoffs | Valid crosshair types (bits) | Messaging icon sequence |
|---|---|---|---|---|
| `weapons\assault rifle\assault rifle` | `master rounds` | 61 / 10 / 0 / 0 | 0 | 8 |
| `weapons\pistol\pistol` | `master rounds` | 13 / 4 / 0 / 0 | 0, 1 | 9 |
| `weapons\shotgun\shotgun` | `master rounds` | 6 / 2 / 0 / 0 | 0 | 12 |
| `weapons\plasma rifle\plasma rifle` | `master plasma` | 0 / 0 / 75 / 10 | 0, 6 | 11 |
| `weapons\rocket launcher\rocket_launcher` | `master rounds` | 1 / 1 / 0 / 0 | 0, 1 | 5 |
| `weapons\sniper rifle\sniper rifle` | `master rounds` | 5 / 1 / 0 / 0 | 0, 1 | 4 |
| `weapons\plasma pistol\plasma pistol` | `master plasma` | 0 / 0 / 75 / 10 | 0, 6 | 6 |
| `weapons\needler\needler` | `master rounds` | 21 / 8 / 0 / 0 | 0 | 10 |
| `weapons\flamethrower\flame thrower` | `master fuel` | 100 / 10 / 0 / 0 | none | 7 |

(The messaging icon is the sequence of `hud_msg_icons`; the table lists it
for the pickup prompt.)

**Leaf elements.** Statics and meters: state 1 (loaded rounds), map type 1
(one window), top-left anchor; the plate is the weapon's "alphas" outline
and the bar the weapon's meter. Plain/flash is 0x00000000 / 0xFFFF552B for
the statics (period 1.35, delay 0.225, 3 flashes, length 0.225) and the
meters are min = max = 0x002896FF with flash 0x00FF552B, empty 0x0000254A,
minimum 0, value scale 0 (the raw count), fade 0.75, opacity 0.

| Weapon | Static (AA seq, size, offset, rect) | Meter (AM seq, size, offset, rect, alpha = m x v + b) |
|---|---|---|
| assault rifle | 1, 128 x 32, (0, 20): (48, 56, 176, 88) | 1, 114 x 27, (-1, 20): (47, 56, 161, 83), 4 x v + 2 (60 rounds: 242) |
| pistol | 5, 128 x 16, (5, 21): (53, 57, 181, 73) | 5, 108 x 17, (4, 20): (52, 56, 160, 73), 20 x v + 0 (12 rounds: 240) |
| shotgun | 7, 128 x 16, (3, 21): (51, 57, 179, 73) | 7, 130 x 18, (2, 21): (50, 57, 180, 75), 20 x v + 0 |
| rocket launcher | 2, 128 x 16, (4, 20): (52, 56, 180, 72) | 2, 109 x 18, (3, 20): (51, 56, 160, 74), 120 x v + 1 (2 rounds: 241) |
| sniper rifle | 6, 128 x 32, (7, 21): (55, 57, 183, 89) | 6, 107 x 22, (6, 21): (54, 57, 161, 79), 60 x v + 1 |
| needler | 4, 128 x 16, (4, 20): (52, 56, 180, 72) | 4, 111 x 17, (3, 20): (51, 56, 162, 73), 12 x v + 6 (20 rounds: 246) |
| plasma rifle, plasma pistol | none (in `master plasma`) | none (in `master plasma`) |
| flamethrower | none | none |

**Leaf crosshairs** (centre anchor; all items one per element unless
noted; "sprite" = sequence of the page; rectangles centred as computed):

| Weapon | Element: type, map, page, sequence (picture size), offset, flags, plain / flash | Rect |
|---|---|---|
| assault rifle | 0, any, RT seq 0 (66 x 66), (0, 0), flashes: 0x002896FF / 0x00FF0000 (1.0 / 0 / 1 / 1.0) | (287, 207, 353, 273) |
| pistol | 0, any, RT seq 8 (28 x 28), same colours | (306, 226, 334, 254) |
| pistol | 1, one window, `sniper\hud_reticles_scope` seq 1 (31 x 24 "2x"), (105, 105), flags "only when zoomed", scaling flags 3: 0x3C2896FF / 0x78FF0000 | (410, 333, 440, 357) |
| pistol | 1, split screen, the same page and sequence, (67, 67) | (372, 295, 402, 319) |
| shotgun | 0, any, RT seq 10 (64 x 64) | (288, 208, 352, 272) |
| plasma rifle | 0, any, RT seq 4 (64 x 64) | (288, 208, 352, 272) |
| plasma rifle | 6, any, RW seq 0 "Low Battery" (49 x 27), (-50, 24), scaling flags 2: 0x00000000 / 0x002896FF (4.05 / 0.225 / 3 / 0.225) | (246, 251, 294, 277) |
| rocket launcher | 0, any, RT seq 9 (64 x 64); zoom items as the pistol's (one window, split screen) | (288, 208, 352, 272) |
| sniper rifle | 0, any, scope page seq 0 (18 x 18), opaque 0xFF2896FF / 0xFFFF0000 | (311, 231, 329, 249) |
| sniper rifle | 1, one window, scope page seq 1 ("2x", "10x"), (150, 104), 0xB42896FF | (455, 332, 485, 356) |
| sniper rifle | 1, split screen, (95, 60) | (400, 288, 430, 312) |
| sniper rifle | 0, one window, `sniper_scope_crosshairs2`, five whole bitmaps, "not a sprite" + "only when zoomed", 0x78000000 / 0x78FF0000: bitmap 0 (64 x 256) at (-189, 0); 1 (64 x 256) at (189, 0); 2 (128 x 8) at (-93, 0); 3 (128 x 8) at (93, 0); 4 (8 x 128) at (0, 57) | (99, 112, 163, 368); (477, 112, 541, 368); (163, 236, 291, 244); (349, 236, 477, 244); (316, 233, 324, 361) |
| sniper rifle | 0, split screen, `sniper_scope_crosshairs_sm`, five bitmaps: 32 x 128 at (-101, 18) and (115, 18); 128 x 4 at (-33, 0) and (33, 0); 4 x 64 at (0, 42), scaling flags 2 | not computed (split-screen window) |
| sniper rifle | 0, any, `sniper\caption` (128 x 32), (63, 162), "flashes" + "only when zoomed": 0x01000000 / 0x782896FF | (319, 386, 447, 418) |
| plasma pistol | 0, any, RT seq 3 (64 x 64); type 6 at (-36, 32) | (288, 208, 352, 272); (260, 259, 308, 285) |
| needler | 0, any, RT seq 6 (64 x 64) | (288, 208, 352, 272) |

Leaf numbers: the sniper rifle has two (both state 6 and 7, "show
leading zeros" + "only when zoomed", 4 digits, 3 fractional, colour
0x802896FF / 0x80004080 and 0x802896FF / 0x802896FF, period 0, top-left
origins (397, 349) and (397, 364)). Leaf statics of the sniper rifle:
the two angle-tick strips (`weapons\sniper rifle\bitmaps\angle_ticks_black`
sequence 0, 8 x 256, state 3, map type 1, offsets (92, 85) and (445, 85),
rects (140, 121, 148, 377) and (493, 121, 501, 377), colour 0xFFFFFFFF,
flash 0x78FFFFFF, disabled 0x96FFFFFF, flags "reverse colors", each with one
multitexture overlay: maps `angle_ticks_mask` and `angle_ticks` (or
`angle_ticks2`), blend functions (multiply, add), offsets (0, 0) and
(0, -0.232), scale (1, 1) and (1, 2), clamp (wrap on the first), two
effectors (pitch to vertical offset of the second map: input -1.4923 to
1.4923, output 1 to 0; zoom level to its alpha: input -1 to 0, output 0
to 1)).

**The shared parents.**

`ui\hud\master` (no parent; cutoffs all 0; valid types 9): crosshair type
9 ("No Grenades", RW seq 5, 60 x 24), centre offset (0, 60), scaling
flags 2, flags "flashes", 0x00000000 / 0x002896FF (3.0 / 0.225 / 3 /
0.35); rect (290, 288, 350, 312).

`ui\hud\master rounds` (parent `master`; flash flag "use parent"; valid
types 3, 8, 10):

| Element | Definition | Rect |
|---|---|---|
| static 0 | state 0, any map, BG seq 1 (128 x 32), (12, 1), colour 0xDC2896FF, flash 0xDCFF552B (3.0 / 0.225 / 3 / 0.225), disabled 0xDCFF552B | (60, 37, 188, 69) |
| static 1 | state 0, any map, AO seq 2 (128 x 32), (12, 1), colour 0xDCAAD5FF, same flash | (60, 37, 188, 69) |
| number 0 | state 0 (reserve), one window, (29, 3), 3 digits, leading zeros, colour 0xFFE6F0FF, flash 0xFFFFE6E6 (1.0 / 0 / 1 / 1.0) | digits at x 68, 77, 86, y 39 |
| number 1 | state 1 (loaded), split screen, (25, 3), 2 digits, leading zeros | single-window n/a |
| number 2 | state 0 divided by clip size, split screen, (65, 3), 2 digits | n/a |
| overlay 0 | state 0, one window, AT seq 0 (32 x 16), (53, 4), type 16 (always), plain 0xFFE6F0FF | (101, 40, 133, 56) |
| overlay 1 | state 0, split screen, AT seq 0, (36, 4) | n/a |
| crosshair | type 8 "No Ammo" RW seq 2 (42 x 25) at (-30, 30); type 3 "Reload" RW seq 1 (44 x 15) at (-30, 30); type 10 "Low Ammo" RW seq 7 (42 x 24) at (-30, 30); each scaling flags 2, "flashes", 0x00000000 / 0x002896FF (3.0 / 0.225 / 3 / 0.35) | (269, 258, 311, 282); (268, 263, 312, 277); (269, 258, 311, 282) |

So one window shows: the plate and outline (the reserve count in three
digits, the ammunition icon), and the leaf's loaded-rounds bar below it;
split screen shows the loaded count and the number of clips (reserve
divided by the magazine size) as two digits each instead of the bar.

`ui\hud\master plasma` (parent `master`; flash flag "use parent"; cutoffs
heat 75, age 10; valid types 18):

| Element | Definition | Rect |
|---|---|---|
| static 0, 1 | state 3 (battery), any map: BG seq 1 and AO seq 2 (12, 1), as the rounds plate | (60, 37, 188, 69) |
| static 2 | state 2 (heat), one window, AO seq 0 (128 x 32), (4, 21), colour 0xFFAAD5FF, no flash (period 0) | (52, 57, 180, 89) |
| static 3 | state 2, one window, BG seq 2 (128 x 32), (4, 21), colour 0xDC2896FF, flash as the plates | (52, 57, 180, 89) |
| static 4 | state 2, split screen, AO seq 1 (128 x 8), (9, 21), scaling flags 3 | n/a |
| meter 0 | state 2, one window, AM seq 0 (104.99994 x 16: 104 x 16 pixels), (4, 21), min 0x002896FF, max 0x00FF0000, flash 0x00FF552B, empty 0, alpha 1 x v, fade 0.35 | (52, 57, 156, 73) |
| meter 1 | state 2, split screen, AM seq 8 (130 x 10), (8, 20), value scale 255, fade 0.75 | n/a |
| number 0 | state 3 (battery percent), any map, (26, 3), 3 digits, leading zeros, colour 0xFFE6F0FF | digits at x 65, 74, 83, y 39 |
| overlay 0 | state 3, any map, AT seq 4 (32 x 16), (51, 2), type 16, "flashes", 0xFFE6F0FF | (99, 38, 131, 54) |
| crosshair | type 18 "Battery Depleted" RW seq 10 (56 x 27), (-40, 36), scaling flags 3, "flashes", 0x00000000 / 0x002896FF (4.05 / 0.225 / 3 / 0.225) | (252, 263, 308, 289) |

So the plasma weapons show a battery percentage on the plate, a heat bar
under it that goes from blue (cold) to red (hot) and a heat outline that
flashes at 75 percent heat.

`ui\hud\empty` (the default, no parent; valid types 0, 8, 9): aim dot
RT seq 7 (28 x 28), colour 0x002896FF, flags 0 (no flash: the frame is
the aim value modulo one sprite); "No Weapon" RW seq 6 (50 x 27) at
(0, 40); "No Grenades" RW seq 5 at (0, 65). `ui\hud\master fuel` has
nothing.

### The grenade HUDs

Anchor 0 (top left), both tags the same except the picture:

| Piece | Definition | Rect |
|---|---|---|
| plate | BG seq 0 (128 x 32), (82, 1), colour 0xC82896FF, flash 0xC8FF552B (3.0 / 0.225 / 3 / 0.225), disabled 0xC8FF552B | (130, 37, 258, 69) |
| outline | AO seq 3 (128 x 32), (82, 1), colour 0xDCAAD5FF (frag) / 0xDCAAD5FF (plasma), flash 0xDCFF552B (frag) / 0xDCAAD5FF (plasma) | (130, 37, 258, 69) |
| count | (107, 3), 1 digit, leading zeros, 0xFFE6F0FF / 0xFFFFE6E6 | glyph sprite (146, 39, 164, 51) |
| overlays | AT seq 2 (frag) at (90, 4) or seq 3 (plasma) at (88, 2); item 0 type 1 (flashing) colour 0xFFFFE6E6, item 1 type 4 (default) colour 0xFFE6F0FF | (138, 40, 170, 56) frag; (136, 38, 168, 54) plasma |

Flash cutoff 1 (the plate flashes at one grenade or none; disabled at
none).

### HUD globals (`ui\hud\default`)

| Field | Value |
|---|---|
| messaging placement | anchor 0, offset (0, 60) |
| fonts | `ui\large_ui` (one window; ascending 17, descending 4, leading 0), `ui\small_ui` (split screen; 14, 3, 3) |
| up time, fade time, spacing | 2.0 s, 2.0 s, 1.35 |
| state colour = text colour | alpha 0.5, rgb (0.4588, 0.7294, 1.0) |
| help colour block | 0x8075BAFF / 0x80FFFFFF, period 4.05, delay 0.225, 3 flashes, length 0.225 |
| objective colour | all zero |
| item messages / icon text | `ui\hud\hud_item_messages`, `ui\hud\hud_icon_messages` |
| messaging icons | `ui\hud\bitmaps\combined\hud_msg_icons` |
| HUD messages | `ui\hud\hud messages` |
| waypoint offsets (top, bottom, left, right) | 20, 20, 20, 20 |
| waypoint bitmap | `hud_waypoints`: arrows `default`, `default_red`, `flag_blue`, `flag_red`, `target_blue`, `target_red`, `ball_blue`, `ball_red`, `crown_blue`, `crown_red` (colours 0x002896FF blue, 0x00FF552B red; opacity 0.5; fade 0; sequence triples (1, 0, 6), (1, 0, 6), (2, 0, 6), (2, 0, 6), (3, 0, 6), (3, 0, 6), (4, 0, 6), (4, 0, 6), (5, 0, 6), (5, 0, 6)) |
| multiplayer HUD scale | 0.75 (unread in the reference) |
| default weapon HUD | `ui\hud\empty` |
| motion sensor range, velocity sensitivity, scale | 20, 0.0002, 32 |
| default title bounds | (425, 0, 444, 597) (unused) |
| damage indicators | offsets 10, 10, 10, 10; bitmap `hud_damage_arrows`; sequences 0 and 1; colour 0xC8FF0000 |
| timer colours | running 0xFF75BAFF, flash 0xFFFFE6E6 (3.0 / 0.225 / 3 / 0.225); time up 0xFFFFE6E6, flash 0xFFFF552B (0.25 / 0.125 / 1 / 0.125) |
| carnage report bitmap | `ui\shell\bitmaps\postgame_carnage_report` |
| loading begin, end; checkpoint begin, end | 42, 43, 44, 45 |
| checkpoint sound | null |
| button icons (18, icon element: sequence, width offset, offset, colour, frame rate, flags, text index) | 0 A (0, -6, (-2, -5), 0xFF4BEF50, 0, 2, 0); 1 B (1, ..., 0xFFFA5B45); 2 X (2, ..., 0xFF3D9BFA); 3 Y (3, ..., 0xFFFBDC2F); 4 black (13, ..., 0xFF000000, flags 2, text 1); 5 white (13, ..., 0xFFFFFFFF, text 2); 6 and 7 left and right trigger (21 and 22, flags 0, text 3 and 4); 8 to 11 d-pad (sequence 0, flags 1 "use text", text 5 to 8, no offsets); 12 start (18, text 9); 13 back (17, text 10); 14 and 15 left and right thumb (19 and 20, width offset 28, frame rate 10, flags 4 "absolute width", text 11 and 12: three-frame animated sprites); 16 and 17 left and right stick (15 and 16, text 11 and 12). Flags 2 = "use this colour" |

### Pictures (sprite sizes, pixels, the sequences the disc uses)

| Bitmap (`ui\hud\bitmaps\`) | Sequences used: size |
|---|---|
| `combined\hud_unit_backgrounds` | 0 128 x 32, 1 128 x 32, 2 128 x 16, 3 128 x 16, 4 128 x 8, 5 128 x 8, 6 128 x 128, 7 64 x 64 |
| `combined\hud_unit_meters` (page 256 x 256) | 0 122 x 31, 1 122 x 31, 2 88 x 14, 3 88 x 14, 4 130 x 10, 5 130 x 10, 6 130 x 34 and 7 88 x 14 (vehicle HUDs) |
| `combined\hud_weapon_backgrounds` | 0 to 2 128 x 32, 3 128 x 128, 4 64 x 64 |
| `combined\hud_ammo_outlines` | 0 128 x 32, 1 128 x 8, 2 128 x 32, 3 128 x 32 |
| `combined\hud_ammo_alphas` | 0 128 x 16, 1 128 x 32, 2 128 x 16, 3 128 x 16, 4 128 x 16, 5 128 x 16, 6 128 x 32, 7 128 x 16 |
| `combined\hud_ammo_type_icons` | 0 32 x 16 ("x" and a bullet), 1 64 x 16 (a wide capsule; no nine-weapon HUD uses it), 2 32 x 16 (frag grenade), 3 32 x 16 (plasma grenade, a dot), 4 32 x 16 (battery) |
| `combined\hud_ammo_meters` | 0 105 x 16 (104.99994: 104 pixels wide in the game), 1 114 x 27, 2 109 x 18, 3 105 x 13, 4 111 x 17, 5 108 x 17, 6 107 x 22, 7 130 x 18, 8 130 x 10 |
| `combined\hud_reticles` | 0 66 x 66 (assault rifle), 2 (warthog gun), 3 (plasma pistol), 4 (plasma rifle), 6 (needler), 9 (rocket launcher), 10 (shotgun) all 64 x 64, 7 and 8 28 x 28 (empty, pistol), 11 (scorpion), 13 (ghost), 14 18 x 10 |
| `combined\hud_reticle_warnings` | 0 "Low Battery", 1 "Reload", 2 "No Ammo", 3 "Low Fuel", 4 "No Fuel", 5 "No Grenades", 6 "No Weapon", 7 "Low Ammo", 8 a small triangle, 9 five progress frames (30 x 6), 10 "Battery Depleted" |
| `combined\hud_counter_numbers` | 15 glyph sprites 18 x 12 (the "Km" one 19 x ...): 0 to 9, ".", ":", "-", "m", "Km" |
| `sniper\hud_reticles_scope` | 0 18 x 18, 1 two sprites: "2x" 31 x 24, "10x" 43 x 24 |
| `combined\hud_damage_arrows` | 0 231 x 40, 1 165 x 29 |
| `combined\hud_waypoints` | 34 x 34 each: 0 downward triangle, 1 double chevron, 2 flag, 3 target ring, 4 skull, 5 crown, 6 diamond |
| `hud_sweeper`, `hud_sweeper_mask` | 64 x 64 each |
| `hud_sensor_blip`, `hud_sensor_blip_custom` | 32 x 32 |
| `hud_multiplayer` | 64 x 32 |

### Strings

`ui\hud\hud_item_messages` (46): 0 "Picked up a flamethrower", 1 "Picked
up the odd ball", 2 "Picked up a fragmentation grenade", 3 "Picked up %d
fragmentation grenades", 4 and 5 assault rifle (single, "%d rounds for"),
6 and 7 plasma pistol, 8 and 9 energy sword, 10 and 11 flag, 12 and 13
flamethrower ("%d canisters for flamethrower"), 14 and 15 fuel rod gun,
16 and 17 gravity rifle, 18 and 19 needler, 20 and 21 pistol, 22 and 23
plasma rifle, 24 and 25 rocket launcher, 26 and 27 shotgun, 28 and 29
sniper rifle, 30 "Picked up active camouflage" and 31 a placeholder, 32
"Picked up double speed" and 33 placeholder, 34 "Picked up full-spectrum
vision" and 35 placeholder, 36 "Picked up a health pack" and 37
placeholder, 38 "Picked up an over shield" and 39 placeholder, 40 "Picked up
a plasma grenade", 41 "Picked up %d plasma grenades", 42 "Loading...", 43
"Loading... done", 44 "Checkpoint... ", 45 "Checkpoint... done". Items'
`message index` on the disc: assault rifle 4, pistol 20, shotgun 26,
plasma rifle 22, rocket launcher 24, sniper rifle 28, plasma pistol 6,
needler 18, flamethrower 12 (value scale 100), gravity rifle 16, ball 1,
flag 10, frag grenade 2, plasma grenade 40, active camouflage 30, over
shield 38, health pack 36; the ammunition crates have 0 (their message
is their weapon's index plus 1).

`ui\hud\hud_icon_messages` (27): 0 a placeholder, 1 "black", 2 "white",
3 "left trigger", 4 "right trigger", 5 to 8 d-pad up, down, left, right,
9 "start", 10 "back", 11 "left thumbstick", 12 "right thumbstick", 13
"driver", 14 "gunner", 15 "side", 16 "Warthog", 17 "Pelican", 18
"Banshee", 19 "gun turret", 20 "lifepod", 21 "Ghost", 22 "Wraith", 23
"Scorpion", 24 "Kestrel", 25 "cargo", 26 "switch".

`ui\hud\hud messages` (13 messages, 53 elements, 744 characters of
text): pickup "Hold [action] to pick up [name]"; swap powerup "Press
[action] to swap [name] for [name]"; touch device "Press [action] to touch
device [name]"; custom device " Press [action] to [name]"; swap weapon
"Hold [action] to swap for [name]"; remind to switch weapons "Press
[rotate weapons] to switch to [name]"; enter vehicle "Press [action] to
enter [name] seat of [name]"; exit vehicle "Press [action] to exit
[name]"; flip vehicle "Press [action] to flip [name]"; respawn failures:
"Waiting to respawn...teammate moving too fast", "...too many enemies
nearby", "...teammate in combat", "...teammate must exit vehicle". In
elements the icons are 18 (action), 24 (rotate weapons), 32 and 33
(custom 1 and 2, the names).

### HUD number tag (`ui\hud\counter`)

Bitmap `hud_counter_numbers`; character width 12, screen width 9, x
offset 1, y offset 0, decimal point width 6, colon width 6.

## Test cases

Worked out from the reference and the disc. The first group are unit
tests on synthetic tags and rules; the second group are slow tests on
the user's own disc image.

### (a) Synthetic tags and rules

**S1. Parse a weapon HUD interface with one number element and one
crosshair.** A 0x17C-byte `wphi`: parent reference NONE; flash cutoffs
(flags 1, total 5, loaded 3, heat 0, age 0); placement anchor 0;
statics, meters, overlays, warning sounds, screen effects empty; numbers
count 1; crosshairs count 1; valid crosshair types 1 (bit 0). Number
element (0xA0 bytes): header state 1, runtime 0, map type 1; placement
offset (29, 3), scale (1, 1), scaling flags 0; colour 0xFFE6F0FF, flash
0xFFFFE6E6, period 1.0, delay 0, flashes 1, flags 0, length 1.0,
disabled 0xFFFFE6E6; digits 3, flags 1, fractional 0; weapon flags 1.
Crosshair element (0x68): type 0, map 0, a page reference, one item
(0x6C): offset (0, 0), scale (1, 1), colour 0x002896FF, flash 0x00FF0000,
period 1.0, delay 0, flashes 1, length 1.0, frame rate 0, sequence 3,
flags 1.

| Expected parse | Value |
|---|---|
| parent | none |
| flash cutoffs | 1, 5, 3, 0, 0 |
| numbers[0] | state 1, map type 1, offset (29, 3), digits 3, leading zeros true, show only when zoomed false, trailing m false, fractional 0, divide by clip size true |
| numbers[0] colour | plain 0xFFE6F0FF, flash 0xFFFFE6E6 |
| crosshairs[0] | type 0 (aim), map type 0, items 1 |
| item 0 | offset (0, 0), sequence 3, flashes true, not a sprite false, plain 0x002896FF, flash 0x00FF0000 |
| valid types | bit 0 only |

**S2. Parse a unit HUD interface's shield meter.** A 0x56C-byte `unhi`,
anchor 1; the meter element at 0xF4: offset (7, 0), scale (1, 1), flags 0,
min 0x0000509F, max 0x002896FF, flash 0x00FFFFFF, empty 0, meter flags 0,
minimum value 0, sequence 0, alpha multiplier 1, alpha bias 55, value
scale 198, opacity 0, fade 0.5. Expected: shield meter offset (7, 0), alpha
multiplier 1, bias 55, value scale 198, fade 0.5; with a 122 x 31 sprite
the rectangle is (463, 36, 585, 67).

**S3. Placement rectangles** (one window, safe frame 48, 36, 592, 444):

| Anchor | Offset | Picture | Rectangle |
|---|---|---|---|
| top right (1) | (7, 0) | 122 x 31 | (463, 36, 585, 67) |
| bottom right (3) | (10, 20) | 64 x 32 | (518, 392, 582, 424) |
| bottom left (2) | (41, 41) | point only | (89, 403) |
| top left (0) | (12, 1) | 128 x 32 | (60, 37, 188, 69) |
| centre (4) | (0, 0) | 66 x 66 | (287, 207, 353, 273) |
| centre (4) | (0, 0) | 28 x 28 | (306, 226, 334, 254) |
| top right (1) | (-5, 18) | 128 x 16 | (469, 54, 597, 70) |
| centre (4) | (-30, 30) | 42 x 25 | (269, 258, 311, 282) with one pixel of tolerance in y (the half-pixel) |

**S4. Meter fill** (the unit HUD meters, value from the fraction):

| Shield s | Value byte trunc(198 s) | Fill alpha (1 x v + 55) |
|---|---|---|
| 0 | 0 | 55 |
| 0.1 | 19 | 74 |
| 0.5 | 99 | 154 |
| 1.0 | 198 | 253 |
| 2.5 (overshield) | stage 0: 198; stage 1 (red): 198; stage 2 (green): v = 0.5, 99 | 253, 253, 154 |

| Body b | Segments trunc(8 b) | Fill alpha max(31, 30 v + 15) | Colour |
|---|---|---|---|
| 1.0 | 8 | 255 | blue 0x2896FF (b at or above 0.67) |
| 0.5 | 4 | 135 | yellow 0xFFDF00 |
| 0.34 | 2 | 75 | yellow |
| 0.33 | 2 | 75 | red 0xFF0000 (b at or below 0.33) |
| 0.1 | 0 | 31 | red |
| 0 | 0 | 31 | red |

Weapon meters: assault rifle with 60 loaded: 4 x 60 + 2 = 242, 30 loaded:
122; rocket launcher 2 loaded: 120 x 2 + 1 = 241, 1: 121, 0: 1; pistol 12
loaded: 240; plasma heat meter: heat 1.0 gives byte 255, alpha 255, heat
0.5 gives byte 127.

**S5. Shield hit episode.** A Spartan at s = 1.0 (L = 1.0, f = -1) loses
shield to s = 0.6 on tick 200 and takes no more damage. Ticks 200 to 214:
L = 1.0 (held), f = 0; stage 0 draws min alpha 173 (trunc(198 x 0.6) =
118, plus 55) and max alpha 253, flash phi = 1 (the lost part is marked in
the flash colour 0x00FFFFFF at full strength). Tick 215: L = 0.6,
f = 0.5, t = 215; from tick 216 on stage 0 draws min = max = 173 and no
mark. A recharge starting at tick 380 (the six-second delay of
`health_shields.md` from 200): each tick s rises, L = s, f = -1.

**S6. Flash function** with the weapon plate block (colour 0x00000000
treated as RGBA (0, 0, 0, 0), flash 0x002896FF, period 3.0, delay 0.225,
3 flashes, length 0.225, not reversed): the colour is the flash colour
(0, 0.1569, 0.5882, 1.0) at t = 0.1125 s after the start and again at
0.5625 and 1.0125; the plain colour at t = 0 (and 0.225, 0.45, 0.675),
at 1.35 and at 2.0; at 1.0 s the pulse is at 0.9848 of the way (phase'
0.1 of 0.225). With "reverse colors" the plain and flash colours swap
roles. A block with period 0 never flashes.

**S7. Flashing thresholds** (assault rifle: total 61, loaded 10): reserve
61: state 0 flashing, 62: not; loaded 10 and not reloading: state 1
flashing, 11: not; loaded 10 while reloading: not. Plasma (heat 75, age
10): heat 0.75 flashes (75 >= 75), 0.74 not; age 0.9 flashes ((1 - 0.9) x
100 = 10, with floating-point care: use age 0.91 for a certain yes, 0.89
for a certain no).

**S8. Crosshair states.** zoom level NONE: zoom value 0; level 0: 1;
level 1: 2. Frames: a zoom item with "only when zoomed": level NONE:
skipped; level 0: frame 0 ("2x"); level 1: frame 1 ("10x"). Type 8 on an
assault rifle with 0 loaded and 0 reserve: trigger pulled at tick 100,
released at tick 110: active at 100; still drawn at tick 189 (now - start
= 89 < 90); cleared at tick 190 (90 >= trunc(3.0 x 30)); a new pull at
tick 150 (while still latched) keeps the original start. The aim item
turns solid 0x00FF0000 exactly when the autoaim level is 1.0, and is
0x002896FF for 0.99.

**S9. Number layout** (top-left anchor, hud# screen width 9): origin
(77, 39) (offset (29, 3)), 3 digits with leading zeros, value 7: sprite
left edges 86, 77, 68 (digits 7, 0, 0 reading right to left), each sprite
18 x 12 at y 39 (rects (86, 39, 104, 51), (77, 39, 95, 51), (68, 39, 86,
51)). Without leading zeros: only the 7 at 86; value 0 without the flag:
nothing. Two digits at origin (73, 39) (offset (25, 3)): 60 draws at 73
and 64. A bottom-right or top-right anchored number puts its first
(rightmost) glyph's right edge on the origin x and steps left by 9.
Value 12 with the trailing-"m" flag at 3 digits: "m" first, then 2 and 1.
Fractional example (the sniper's range number, 4 digits + 3 fractional,
leading zeros, top-left origin (445, 385) from offset (397, 349)):
D = 4 + 3 + 1 = 8, the first glyph's cursor = 445 + (8 - 2) x 9 + 6 =
505. The real 12.345 gives value 12 and decimal value 3450 (the real
times 10 000, modulo 10 000, then divided by 10 to 345 for three
digits). Sprite left edges, right to left: "5" at 505, "4" at 496, "3" at
487 (the cursor is then at 478); it moves forward 9 and back 6 to 481
for the decimal point, then back 9 to 472: "2" at 472, "1" at 463 and,
because of the leading-zeros flag, "0" at 454 and "0" at 445: the text
"0012.345".

**S10. Motion tracker blips** (viewer at the origin, centre of the disc
at pixel (89, 403), range 20, scale 32, one window; offsets are from the
centre, y down; the stored position is quantised to a signed byte per
component):

| Target at (dx, dy) WU | Viewer facing | Quantised | Pixel offset from centre |
|---|---|---|---|
| (10, 0) | 0 (looking +x) | (63, 0) = 9.92 WU | (0, -25.7): up |
| (0, 10) | 0 | (0, 63) | (-25.7, 0): left |
| (-10, 0) | 0 | (-63, 0) | (0, +25.7): down |
| (0, 10) | pi/2 (looking +y) | (0, 63) | (0, -25.7): up |
| (19.9, 0) | 0 | (126, 0) = 19.8 WU | (0, -41.8) |
| (0.1, 0) | 0 | (0, 0) | (0, 0): **exactly the centre**. The zero vector is divided by the floored d (0.015625) and scaled by the curve, so it stays zero; the floor only prevents the division by zero and moves nothing. (No non-zero quantised vector is shorter than 0.157 WU, so the floor never changes a drawn position.) |
| (20, 0) | 0 | (127, 0) decodes to exactly 20.0 | scanned (squared distance 400 is within the range) but **not drawn**: d >= 20 |
| (19.99, 0) | 0 | (126, 0) = 19.84 WU (truncation) | (0, -41.8): drawn |
| (25, 0) | 0 | out of range | not drawn |

(Pixel offset = 42 x (d / 20)^0.7 along the direction; 32 in place of 42
in split screen.) History: entry h = 0 draws brightness 1.0 and size r =
1.0 (plus the base size); h = 1: 0.81 and 1.0022; h = 5: 0.25 and 1.6187;
h = 9: 0.01 and 5.8411. A medium unit's drawn square half-extent for the
newest entry is 1.0 x 2.625 = 2.6 pixels. Colours: enemy (1, 0, 0), friend
(1, 1, 0), self (1, 0.5, 0). Sweep: t = 0 s: window half-size s / 2 =
7.27 (s = 14.545); t = 1.0: s = 0.8556; t >= 2.0375 until 2.1: s = 0.4;
t = 2.1 wraps to t = 0.

**S11. Shows on the tracker:** a unit moving at 0.0142 WU/tick shows
(0.0142^2 = 0.000202 >= 0.0002), at 0.0141 it does not; a stationary unit
holding the trigger shows; a stationary unit throwing a grenade shows
(any state but idle and ending); a unit 20.1 WU away does not (horizontal
distance), whatever its height; a unit at exactly 20 WU is scanned
(squared distance 400) but, along an axis, its blip decodes to exactly
20 and is not drawn.

**S12. Messages:** "Picked up %d rounds for assault rifle" (string 5)
for an ammunition pickup of 60 rounds shows "60" (value scale 0 reads as
1); the flamethrower's 500 canisters (value scale 100) show "5". A second
pickup of the same item within the message's life adds its count and
refreshes it. A message created at tick 0 is at full alpha through tick
60, alpha 0.5^1.9 = 0.268 at tick 90, and removed at tick 120.

**S13. Scope screen effect** (640 by 480, one view; mask values as
decoded from the disc; colours in 0..1):

| Case | Expected |
|---|---|
| sniper rifle zoomed 2x, pixel (320, 240), scene colour 0.6 everywhere | 0.6 (A = 0, Y = 0): unchanged |
| same, pixel (20, 240) (beyond the mask: border texel Y = 128, A = 255), uniform scene 0.6 | 0.6 x (1 - 128/255) = 0.2988 (the blur of a uniform scene is the scene) |
| same, pixel (126, 240) (frame line, Y = 255) | 0 |
| sniper 2x, pixel (130, 240) (inside the frame, A = 255, Y = 0), one pass | S0 at x 130, S1 at x 132.35, S2 at x 127.65 (320 - 190 x (1 -/+ 7.92 / 640)), y 240 for all three; out = (S0 + S1 + S2) / 3 |
| sample positions, r = 7.92, pixel (640, 480) corner | S1 at (636.04, 476.04), S2 at (643.96, 483.96) (clamped to the edge texel) |
| sample positions, r = 10.81 (pistol), pixel (320 + 119, 240) | S1 at x 437.0, S2 at x 441.0 (+/- 2.01) |
| night vision a_L = 1, a_D = 1 additive tint (0, 1, 0), C = (0.1, 0.1, 0.1), Y = 0 | (0.3439, 0.6878, 0.3439) |
| same, Y = 128/255 (border) | a_D = 0: (0.3439, 0.3439, 0.3439) x 0.498 = 0.1713 each |
| light enhancement only, C = 0.5, a_L = 0.5 | 0.5 x 0.5 + 0.5 x 0.9375 = 0.71875 |
| split screen, sniper zoomed | no blur (r unused); pixels in the window unchanged, outside x (1 - 128/255) |
| sniper unzoomed | no pass at all (every bit "only when zoomed") |

### (b) Disc-backed expectations (slow; `bloodgulch.map`)

- Tag counts: 5 `unhi`, 17 `wphi`, 2 `grhi`, 1 `hudg`, 1 `hud#`, 1 `hmt `,
  8 `ustr`; every reference of every one of them resolves to a tag of the
  expected class (`bitm` for pictures, `snd!` or `lsnd` for sounds, `wphi`
  for parents and the default HUD, `font`, `ustr`, `hmt `); every padding
  region is zero.
- Assault rifle `wphi`: chain assault rifle, `master rounds`, `master`;
  the leaf has 1 static (AA seq 1), 1 meter (AM seq 1), 0 numbers, 1
  crosshair with 1 item (RT seq 0, flags 1), 0 overlays, 0 screen effects;
  cutoffs 61 and 10; valid types bit 0; messaging icon sequence 8. Parent
  `master rounds`: 2 statics, 0 meters, 3 numbers, 3 crosshairs (types 8,
  3, 10), 2 overlays, valid types 0x508, flash flag 1; `master`: 1
  crosshair (type 9), valid types 0x200. The single-window rectangles of
  the table above: plate (60, 37, 188, 69), bar plate (48, 56, 176, 88),
  bar (47, 56, 161, 83), reticle (287, 207, 353, 273), "No Ammo"
  (269, 258, 311, 282).
- Plasma rifle `wphi`: chain plasma rifle, `master plasma`, `master`; the
  leaf has 0 statics, 0 meters, 0 numbers, 2 crosshairs (types 0 and 6),
  cutoffs 0, 0, 75, 10; `master plasma`: 5 statics, 2 meters, 1 number, 1
  crosshair (type 18), 1 overlay, valid types 0x40000. Reticle RT seq 4
  (288, 208, 352, 272); "Low Battery" (246, 251, 294, 277); heat bar
  (52, 57, 156, 73); battery digits at x 65, 74, 83.
- Sniper rifle: 3 statics, 1 meter, 2 numbers, 6 crosshairs, 1 screen
  effect; the two angle-tick strips (140, 121, 148, 377) and (493, 121,
  501, 377) and the two side bars (99, 112, 163, 368), (477, 112, 541, 368)
  are mirror images about x = 320.5 and 320 (the safe frame inset check).
- The Spartan's shield meter rectangle (463, 36, 585, 67), health bar
  (468, 53, 556, 67), sensor panel (48, 316, 176, 444), blip centre
  (89, 403).
- Grenade HUDs: plate (130, 37, 258, 69) on both, count glyph sprite
  (146, 39, 164, 51), frag icon (138, 40, 170, 56), plasma icon (136, 38,
  168, 54).
- `hudg`: range 20, sensitivity 0.0002, scale 32; 18 button icons; 10
  waypoint arrows; damage offsets 10; strings 46 and 27; `hmt `: 13
  messages and 53 elements; `hud#`: widths 12 and 9, decimal 6.
- All 13 multiplayer maps carry the same counts of the seven classes.

## Verification performed

- **Layouts.** Every `unhi`, `wphi`, `grhi`, `hudg`, `hud#`, `hmt ` and
  `ustr` tag of `bloodgulch.map` was parsed with the layouts above by a
  throwaway script: 0 problems among about 1 400 checks (reference
  classes, enumeration ranges, flag ranges, the sequence indices of the
  waypoint arrows, the button icon count, the 15 digit sprites, UTF-16
  strings ending in one NUL, the message element counts, and every
  padding region zero). Element strides were confirmed by the distance
  between consecutive blocks (statics 0xB4, numbers 0xA0).
- **Tag counts** of the seven classes were read on the 13 multiplayer maps
  and `ui.map`.
- **Bitmaps.** The HUD's pictures (unit backgrounds and meters, weapon
  plates and outlines, ammo alphas, meters and icons, reticles, warning
  words, digits, scope labels, caption, damage arrow, waypoints, sweep,
  mask, blip, friend triangle, the scope masks) were decoded to images and
  looked at; the descriptions above name what each depicts (for example
  warning sequence 0 reads "Low Battery", digit sprite 13 is "m").
- **Layout.** A script laid out the assault rifle's HUD with the rules of
  section 1 (the unit HUD, the weapon plates, the grenade plate, the
  motion sensor, the reticle) on the 640 x 480 canvas and composited the
  pictures: the shield and health land top right, the weapon plate, the
  loaded-rounds bar and the grenade plate top left (the grenade plate
  overlaps the weapon plate's end, as the artwork nests), the sensor
  bottom left with the blip centre in the middle of the disc, the reticle
  at the centre. The nine weapons' HUDs were laid out the same way: no
  element lies outside the 640 x 480 canvas; the only elements outside the
  safe frame are the health background (5 pixels past the right edge, its
  authored offset) and the flashlight pieces (7 past it).
- **Symmetry.** The sniper's scope furniture is symmetric about the
  screen centre only with the 48 and 36 pixel inset applied.

## Confidence per section

| Section | Confidence |
|---|---|
| The HUD tags: every layout and value | checked against the disc (meaning of a few fields from the reference: unread fields are said so) |
| 1 Canvas and placement | reference only for the formula and safe frame; consistent with the disc (the symmetry check) |
| 2 Which HUD, the order | reference only (the unit HUD pair choice is surprising: open question 2) |
| 3 Colours, flashing, blending | reference only (the blend class from `shaders.md`); the flash formula is exact as read |
| 4 Meters | the inputs checked against the disc's numbers (the flash expression read from the reference and the sprite truncation from the disc's sprite fractions in single precision); the shader mapping reference only; the tint and frame retention inferred |
| 5, 6 Shield, health, sounds | reference only; thresholds and counts consistent with the disc's tags; the overshield colour list also checked in the disc's executable |
| 7 Weapon HUD | reference only; every disc element's consistency checked (states and maps used as the rules say) |
| 8 Crosshairs | reference only; the disc elements checked for consistency |
| 9 Scopes, screen effects | layout checked against the disc; the mask bitmaps decoded (formats A8Y8 and AY8, both channels); the pass's textures, addressing, sample positions and combiner arithmetic from the reference (Y darkens, A selects the blur); the stage assignment of the samples and the A8Y8 sampling inferred (see Certainty); the vertical field of view from the reference and the disc's executable |
| 10 Grenades | reference only; values disc |
| 11 Motion tracker | numbers checked against the disc; the blip rule and the equality behaviour at the range (scan not strict, draw strict, decode exactly 20 at byte 127, checked in single precision) reference only; the sweep compositing inferred |
| 12 Damage indicators | values disc; the direction rule reference only (suspect) |
| 13 Messages | texts disc; layout rules reference only |
| 14 Waypoints, friend markers | arrows disc; rules reference only |
| 15 Hides, split screen | reference only |
| 16 Widescreen | ours |
| 17 On-gun displays | tags and values checked against the disc (the functions, input sources and shader flags); the shader rule from `shaders.md` |

## Open questions

1. **Float-to-integer conversion.** Quad corners and the waypoint alpha
   pass through the engine's own conversion (truncation or round to
   nearest even). Check: screenshot the "2x" label (odd 31 x 24 size,
   half-pixel bounds) and a waypoint on a real Xbox build or an
   emulator, read its pixel rectangle.
2. **The unit HUD pair.** The reference picks index 1 only for several
   local players. Check: in a one-player network game compare the shield
   bar's width and the sensor's size (128 pixel against 64 pixel sensor).
3. **HUD scale in split screen.** The `hudg` field says 0.75; the
   reference's scale function returns 1. Check: a two-player split-screen
   screenshot of the shield bar's width and the message font.
4. **Numbers flagged "only when zoomed".** The reference's draw code does
   not read the flag. Check: with the sniper rifle, aim at an enemy
   unzoomed and see whether the distance numbers show.
5. **The sweep's compositing.** The reference blends the blip layer with
   the sweep by the sweep's alpha (27 percent inside the ring). Check: a
   screenshot of the tracker with a moving enemy at several sweep phases.
6. **Damage arrow direction.** The reference's test leaves no arrow for
   damage from the front and sends damage from behind left or right.
   Check: take hits from the front, back and sides and note the arrows.
7. **Warning sound scale 0.** Every warning sound element stores scale 0;
   the sounds are assumed to play at normal level. Check: whether the
   shield-low loop is audible and at what level.
8. **Script value for the night-vision scale.** Settled from the
   reference: the four script values are set to 1 at every map load and
   only scripts change them (section 9).
9. **Waypoint opacity rounding** (0.5 to 0 or 255 alpha). Check: a flag
   waypoint's brightness against the background.
10. **The viewer's own blip.** The reference scans every living unit
    including the viewer's own; whether an orange dot shows at the
    tracker's centre while moving. Check: a moving player on an empty map.
11. **Friends and the reticle.** The reticle turns red for enemy-team
    targets only; whether friends change it in team games. Check: aim at a
    teammate.
12. **The meter blend** (frame retention `fade`, tint) is inferred from
    the rasteriser setup. Check: the shield bar over a bright sky against
    a dark floor.
13. **Message line positions** use the font tag's heights; check the
    first and second line's pixel rows against a screenshot (96 and 124
    expected in one window).
14. **Whether the objective, timer and help text apply in multiplayer**
    (scripted only here); none of this spec's tests need them.
15. **What the scope masks do.** Settled from the reference's pass
    setup and the masks' two channels (section 9): the grey level Y
    dims (x (1 - Y): half outside the window, black on the frame line),
    the alpha A blends in a radial blur; nothing is blacked out. Still
    to check on hardware: a zoomed sniper screenshot should show the
    full-brightness rectangle of columns 127 to 512 and rows 120 to 359
    with a two-pixel black outline, half-bright blurred surroundings
    and a sharp centre band; the pistol a 238-pixel bright disc.
16. **Overshield colours.** Stage 1 red and stage 2 green follow the disc
    executable's colour list (an earlier draft had them shifted by one,
    green and yellow); check on a real game that the shield bar shows a
    red layer from 1.0 to 2.0 and a green one above 2.0.
