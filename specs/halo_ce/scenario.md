# Halo: Combat Evolved: the scenario, the sky and item collections

Source basis: the game's own map files from the original Xbox disc (the
13 multiplayer maps read in full; five campaign maps and `ui.map`
read for the blocks the multiplayer maps leave empty), parsed with
throwaway scripts; and a decompilation of an Xbox build of the game's
engine (the scenario's structure definitions, object placement, the
game engine's spawn and teleporter code, the sky renderer and the fog
code), used for meaning only. The decompilation describes a different
Xbox build (PAL, 01.01.14.2342) from the disc's (NTSC); where they
differ the disc wins and the difference is noted.
Status: draft

Sections: Summary; Units and conventions; Constants; The scenario's
top level; Rotation and facing; Fields every placement shares; Skies
and the sky tag; Player starting locations; Starting profiles and
starting equipment; Netgame flags; Netgame equipment and item
collections; Object placements by type; Scenery as a renderer needs
it; The other blocks; Blocks to step over; Order of use; Measured
facts; Checks run; Quirks; Test cases; Confidence per section; Open
questions.

---

## Summary

- The scenario (`scnr`) is the one tag that describes a level as a
  place: which sky to draw, where players appear, which weapons and
  powerups lie about and how often they come back, where the
  teleporters, flags and hills are, and every placed rock, plant, light
  and vehicle. Its main struct is 0x5B0 bytes: 55 tag blocks, a few tag
  references and data references, and unused ranges, all at fixed
  offsets (section "The scenario's top level").
- A multiplayer map fills only a handful of blocks: skies (0 to 1),
  player starting locations (48 to 76), netgame flags (43 to 191),
  netgame equipment (30 to 52), starting equipment (0 to 3), scenery
  (2 to 50), and on a few maps vehicles, machines, light fixtures,
  sound scenery and decals. Every other block (AI, scripts, cutscenes,
  trigger volumes, weapons, bipeds, controls, device groups, starting
  profiles) is empty on all 13.
- Spawn eligibility is data, not code: each player start, each
  netgame equipment entry and each starting-equipment entry carries up
  to four game type numbers; a game type picks the entries that name it
  or one of three wildcard values.
- Netgame flags are one block with nine types; the team index field
  means something different per type (owning team, ball number, hill
  number, track sequence number, teleporter pair number).
- Orientation is stored as three angles (yaw, pitch, roll) whose
  composition is not the usual one: yaw spins the object about its own
  up axis first, then pitch and roll tilt the result about the world's
  Y and X axes. An object's up vector therefore does not depend on its
  yaw (section "Rotation and facing"). A player start or a netgame flag
  has a single angle, the heading.
- The sky is a separate small tag (`sky `): a model drawn around the
  camera at 1/1024 scale, unlit, with no depth, plus ambient and fog
  values for outdoors and indoors, and the sun's direction and lens
  flare.

## Units and conventions

All of `cache_file.md` applies: little-endian, world unit (1 WU = 3.048
m), +Z up, +X forward at yaw 0, +Y left, angles in radians, 30 ticks
per second, tag blocks (12 bytes), tag references (16), data
references (20), NONE = -1. Specific to this spec:

- Positions are world units in the level's own frame (the same frame
  as the structure BSP's vertices, `structure_bsp.md`).
- Every angle in this tag is radians. A heading (player start, netgame
  flag, netgame equipment) is one f32 in [-pi, pi] on the disc, the
  angle from +X toward +Y seen from above.
- Times in this tag are whole seconds (i16) unless a field says
  otherwise; the game multiplies by 30 to get ticks.
- Team indices: 0 is red, 1 is blue. Starts with team 2 or 3 exist on
  some maps (section "Player starting locations").
- "Disc" in a statement means it was checked on the 13 multiplayer
  maps, and "campaign" that it was checked on the campaign maps `a10`,
  `a30`, `b30`, `c10`, `d20` or `ui`. "Reference" means read from the
  decompilation only.
- Tag paths are shown as the index stores them, with backslashes
  written as slashes in tables.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| SCENARIO_SIZE | 0x5B0 | bytes | the scenario tag's main struct (disc) |
| SCENARIO_TYPE | 0 solo, 1 multiplayer, 2 main menu | enum | field at 0x3C; 1 on all 13 multiplayer maps, 2 on `ui`, 0 on the campaign (disc) |
| MAX_SKIES | 8 | count | most sky references (reference; the tool's limit) |
| MAX_SCENERY | 2000 | count | most scenery placements (reference) |
| MAX_PLAYER_STARTS | 256 | count | reference |
| MAX_NETGAME_FLAGS | 200 | count | reference; Blood Gulch has 191 (disc) |
| MAX_NETGAME_EQUIPMENT | 200 | count | reference; the most on the disc is 52 |
| MAX_PALETTE | 100 | count | most entries of one object palette (reference) |
| ITEM_DEFAULT_PERIOD | 30 | s | respawn period when neither the entry nor its collection names one (reference) |
| TICKS_PER_SECOND | 30 | ticks/s | `cache_file.md` |
| TELEPORT_RADIUS | 0.5 | WU | a unit this close to a source flag is teleported (reference) |
| TELEPORT_REARM | 1.0 | WU | a unit must leave this radius of the flag it arrived at before that flag works again (reference) |
| SKY_SCALE | 1/1024 | factor | the sky model is drawn this much smaller than its authored size (reference) |
| SKY_FOLLOW | 1023/1024 = 0.9990234375 | factor | the sky model's origin sits at this fraction of the camera position (reference) |
| SKY_FLARE_DISTANCE | 1023.875 | WU | how far from the camera a sky light's lens flare is placed (reference) |
| FOG_SNAP_DISTANCE | 15 | WU | a camera move this long or longer jumps fog values; shorter moves blend (reference) |
| HILL_MAX_POINTS | 12 | count | flags used per hill (reference) |
| RACE_MAX_FLAGS | 32 | count | highest race track team index + 1 (reference) |
| ODDBALL_MAX_BALLS | 16 | count | highest oddball team index + 1 (reference) |

## The scenario's top level

One struct of 0x5B0 bytes at the tag's address. Every unused range was
zero on all 19 maps read (13 multiplayer, `a10`, `a30`, `b30`, `c10`,
`d20`, `ui`). "Elem" is the block's element size in bytes. Every block
is a 12-byte tag block (count, address, zero).

| Offset | Size | Type | Field | Elem | Meaning |
|---|---|---|---|---|---|
| 0x000 | 16 | tag ref | (deprecated) | | class `sbsp`, null on every map |
| 0x010 | 16 | tag ref | (deprecated) | | class `sbsp`, null on every map |
| 0x020 | 16 | tag ref | (deprecated) | | class `sky `, null on every map |
| 0x030 | 12 | block | skies | 16 | one tag reference (class `sky `) per element; 0 or 1 on the disc, at most 8 |
| 0x03C | 2 | enum | type | | 0 solo, 1 multiplayer, 2 main menu |
| 0x03E | 2 | flags | flags | | 0 on every map |
| 0x040 | 12 | block | child scenarios | 32 | tag reference `scnr` (16) + 16 unused; campaign only (a10 has 3) |
| 0x04C | 4 | f32 | local north | | radians; the heading that counts as north for the compass (reference); disc values below |
| 0x050 | 20 | | unused | | zero |
| 0x064 | 136 | | unused | | zero |
| 0x0EC | 12 | block | predicted resources | 8 | i16 type (0 bitmap, 1 sound), i16 index of the picture within its bitmap, u32 tag id; only `ui` has any (384, all bitmaps) |
| 0x0F8 | 12 | block | functions | ? | count 0 on all 19 maps; element size not measured |
| 0x104 | 20 | data ref | editor scenario data | | authoring data; ignore (438 bytes in Blood Gulch) |
| 0x118 | 12 | block | comments | 48 | authoring notes: position (12), 16 unused, text data reference (20); campaign only; ignore |
| 0x124 | 224 | | unused | | zero |
| 0x204 | 12 | block | object names | 36 | name (32 bytes), i16 object type and i16 placement index (both runtime scratch); 0 on all 13 multiplayer maps (a10 has 379) |
| 0x210 | 12 | block | scenery | 72 | placements |
| 0x21C | 12 | block | scenery palette | 48 | tag reference (class `scen`) + 32 unused |
| 0x228 | 12 | block | bipeds | 120 | placements; 0 on all 13 |
| 0x234 | 12 | block | biped palette | 48 | class `bipd` (Blood Gulch, Prisoner, Rat Race, Sidewinder and Wizard list one, unused) |
| 0x240 | 12 | block | vehicles | 120 | placements |
| 0x24C | 12 | block | vehicle palette | 48 | class `vehi` |
| 0x258 | 12 | block | equipment | 40 | placements; 0 on all 13 |
| 0x264 | 12 | block | equipment palette | 48 | class `eqip` |
| 0x270 | 12 | block | weapons | 92 | placements; 0 on all 13 |
| 0x27C | 12 | block | weapon palette | 48 | class `weap` (Damnation lists 2, unused) |
| 0x288 | 12 | block | device groups | 52 | name (32), f32 initial value, u32 flags, 12 unused; 0 on all 13 |
| 0x294 | 12 | block | machines | 64 | placements |
| 0x2A0 | 12 | block | machine palette | 48 | class `mach` |
| 0x2AC | 12 | block | controls | 64 | placements; 0 on all 13 |
| 0x2B8 | 12 | block | control palette | 48 | class `ctrl` |
| 0x2C4 | 12 | block | light fixtures | 88 | placements |
| 0x2D0 | 12 | block | light fixture palette | 48 | class `lifi` |
| 0x2DC | 12 | block | sound scenery | 40 | placements |
| 0x2E8 | 12 | block | sound scenery palette | 48 | class `ssce` |
| 0x2F4 | 84 | | unused | | seven unused blocks; zero |
| 0x348 | 12 | block | starting profiles | 104 | campaign loadouts; 0 on all 13 |
| 0x354 | 12 | block | player starting locations | 52 | the multiplayer spawn points |
| 0x360 | 12 | block | trigger volumes | 96 | boxes for scripts; 0 on all 13 |
| 0x36C | 12 | block | recorded animations | 64 | campaign only; each has a data reference after its fixed part |
| 0x378 | 12 | block | netgame flags | 148 | flag, teleporter, ball, hill and track points |
| 0x384 | 12 | block | netgame equipment | 144 | multiplayer item spawns |
| 0x390 | 12 | block | starting equipment | 204 | what a player spawns holding, per game type |
| 0x39C | 12 | block | BSP switch trigger volumes | 8 | campaign only |
| 0x3A8 | 12 | block | decals | 16 | placements |
| 0x3B4 | 12 | block | decal palette | 16 | class `deca` tag reference only (no padding) |
| 0x3C0 | 12 | block | detail object collection palette | 48 | class `dobc` + 32 unused; 0 on all 13 |
| 0x3CC | 84 | | unused | | zero |
| 0x420 | 12 | block | AI actor palette | 16 | campaign only; class `actv` |
| 0x42C | 12 | block | AI encounters | 176 | campaign only; owns sub-blocks |
| 0x438 | 12 | block | AI command lists | 96 | campaign only |
| 0x444 | 12 | block | AI animation references | 60 | campaign only |
| 0x450 | 12 | block | AI script references | 40 | campaign only |
| 0x45C | 12 | block | AI recording references | 40 | campaign only |
| 0x468 | 12 | block | AI conversations | 116 | campaign only |
| 0x474 | 20 | data ref | script syntax data | | 380 076 bytes (a header and 19 001 nodes of 20 bytes) on every map, scripts or not; ignore |
| 0x488 | 20 | data ref | script string constants | | 1024 bytes on the multiplayer maps; ignore |
| 0x49C | 12 | block | scripts | 92 | 0 on all 13 |
| 0x4A8 | 12 | block | script globals | 92 | 0 on all 13 |
| 0x4B4 | 12 | block | script references | 40 | 0 on all 13 |
| 0x4C0 | 12 | block | script source files | 52 | 0 on all 13 |
| 0x4CC | 24 | | unused | | zero |
| 0x4E4 | 12 | block | cutscene flags | 92 | campaign only; see "Blocks to step over" |
| 0x4F0 | 12 | block | cutscene camera points | 104 | campaign only |
| 0x4FC | 12 | block | cutscene titles | 96 | campaign only |
| 0x508 | 108 | | unused | | zero |
| 0x574 | 16 | tag ref | custom object names | | class `ustr`; null on every map checked |
| 0x584 | 16 | tag ref | in-game help text | | class `ustr`; null |
| 0x594 | 16 | tag ref | HUD messages | | class `hmt `; null |
| 0x5A4 | 12 | block | structure BSPs | 32 | `structure_bsp.md` ("Locating a BSP in the map file"); 1 on every multiplayer map |

Element sizes of the empty-on-the-disc blocks that the multiplayer
maps never fill come from the reference; those the campaign maps
fill and that have no sub-blocks were checked there (child scenarios 32,
starting profiles 104, trigger volumes 96, BSP switch volumes 8,
comments 48, cutscene flags 92 and camera points 104). The AI and
script blocks own sub-blocks, so their sizes could not be checked by
contiguity and are reference values.

Local north on the disc: pi on beavercreek, bloodgulch, hangemhigh and
wizard; pi/2 on longest and sidewinder; 0 on the other seven. Nothing
a multiplayer match needs reads it; it feeds a compass-style object
function (reference).

## Rotation and facing

Two kinds of angle appear in the scenario.

**A heading** is a single f32 (player start, netgame flag, netgame
equipment). The forward direction is
(cos h, sin h, 0), a unit vector in the ground plane; the up
direction is (0, 0, 1). A spawned player's body points along this
vector. h = 0 faces +X, pi/2 faces +Y (left of +X).

**An object rotation** is three f32 stored in the order yaw, pitch,
roll (a placement's 0x14..0x1F). With cy = cos yaw, sy = sin yaw, cp,
sp for pitch and cr, sr for roll, the object's three axes in world
coordinates are:

| Axis | x | y | z |
|---|---|---|---|
| forward F | cy cp | sy cr - cy sp sr | cy sp cr + sy sr |
| left L | -sy cp | sy sp sr + cy cr | cy sr - sy sp cr |
| up U | -sp | -cp sr | cp cr |

A model point p = (px, py, pz) in the object's own frame lands at
position + px F + py L + pz U. F, L, U are unit length, mutually
perpendicular and right-handed (F x L = U). As a rotation matrix with F,
L, U as columns, the same thing is the product of three rotations
applied right to left to column vectors: first a rotation about the
world Z axis by yaw (counter-clockwise from above), then a rotation
about the world Y axis by -pitch (a positive pitch lifts the forward
axis), then a rotation about the world X axis by roll (right-handed:
+Y toward +Z). The consequences a converter must respect:

- With pitch = roll = 0 the object simply turns about vertical: F = (cos
  yaw, sin yaw, 0). Almost every scenery rotation on the disc is
  yaw-only.
- The up axis U does not depend on yaw at all. Pitch and roll tilt the
  object about the world's Y and X axes after it has been spun: they
  are "align to the ground" tilts, not turns about the object's own
  left and forward axes. A converter that composes the angles in the
  usual yaw-then-pitch-then-roll body-axis order gets different
  orientations as soon as yaw is not 0 and a tilt is not 0.
- At pitch = +-pi/2 the yaw and roll rotations act about the same axis
  (gimbal lock); only their sum (roll + yaw for pitch = -pi/2, roll -
  yaw for pitch = +pi/2) matters. The tools write such pairs as
  opposite numbers, for instance Blood Gulch's tilted teleporter base:
  yaw 39, pitch -86, roll -39 degrees.

Worked examples (angles in degrees, results to four places):

| yaw | pitch | roll | F | L | U |
|---|---|---|---|---|---|
| 90 | 0 | 0 | (0, 1, 0) | (-1, 0, 0) | (0, 0, 1) |
| 90 | 30 | 0 | (0, 1, 0) | (-0.8660, 0, -0.5) | (-0.5, 0, 0.8660) |
| 90 | 30 | 45 | (0, 0.7071, 0.7071) | (-0.8660, 0.3536, -0.3536) | (-0.5, -0.6124, 0.6124) |
| 45 | 30 | 60 | (0.6124, 0.0474, 0.7891) | (-0.6124, 0.6597, 0.4356) | (-0.5, -0.75, 0.4330) |

The second row shows the point of the convention: yaw 90 degrees turns
the forward axis to +Y, and pitch 30 then leaves F unchanged but tips
the up axis back toward -X, because pitch tilts about the world Y axis.
A body-axis (yaw, then pitch about the object's left axis) composition
would have lifted F out of the ground plane instead.

Evidence for the convention: the game's own placement code builds the
matrix this way (reference), and on the disc Wizard's four alert-light
machines, one in each corner of the square room at (+-9.7, +-9.7),
carry yaw near 0 or pi, pitch near +-45 degrees and roll near +-90
degrees; with this composition each light's up axis points straight
into its own corner (for the corner at (-9.66, 9.77): U = (-0.69, 0.72,
-0.04)), the symmetric arrangement a level designer would make, while
the body-axis composition gives all four the same up axis (about -Y).

A heading's use elsewhere: a netgame flag's facing is used by the
teleporter (below) and nowhere else on the disc build; netgame
equipment facing is stored and ignored by this build (section "Netgame
equipment").

## Fields every placement shares

Every placement block except decals starts with the same 40-byte
header (0x28 bytes). Sound scenery and equipment placements are only
this header; the other types append their own fields after it.

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 2 | i16 | palette index | element of the type's own palette block; -1 or out of range: nothing is created |
| 0x02 | 2 | i16 | name index | element of "object names", or -1; -1 on every placement on the disc |
| 0x04 | 2 | flags | placement flags | bit 0: not automatic (placed only when something creates it by name); bit 1: not on easy; bit 2: not on normal; bit 3: not on hard. 0 on every disc placement |
| 0x06 | 2 | i16 | variant | the model variant (permutation) the placement asks for; 0 on every disc placement |
| 0x08 | 12 | 3 x f32 | position | world units; the object's origin |
| 0x14 | 12 | 3 x f32 | rotation | yaw, pitch, roll in radians; see "Rotation and facing" |
| 0x20 | 2 | flags | on-BSP flags | bit i set: the object is in structure BSP i; see below |
| 0x22 | 2 | flags | miscellaneous flags | equipment placements: bit 0 levitate, bit 1 does not orient to ground; 0 on every disc placement |
| 0x24 | 4 | | unused | zero |

- A palette entry is 48 bytes: a tag reference to the object tag
  (class `scen`, `bipd`, `vehi`, `eqip`, `weap`, `mach`, `ctrl`,
  `lifi` or `ssce` by palette) and 32 unused bytes (zero). On the disc
  every reference is non-null and its class equals the palette's class
  (checked on all 13 maps). Palette entries need not be used: Blood
  Gulch's scenery palette lists 19 types and places 15.
- On-BSP flags: for scenery and light fixtures (the two types the game
  removes and re-creates when the BSP changes) the game recomputes this
  when it loads a BSP: bit i is set when the placement's position, or
  its bounding offset carried through its rotation, lies inside a leaf of
  BSP i, and these placements are only created while the current BSP's
  bit is set (reference). So a scenery object whose flag is 0 is buried
  in solid space and never appears. On the disc 259 of 264 scenery
  placements have bit 0 set, and all 46 light fixtures. The five scenery
  placements that do not: Beaver Creek's 'plant fern' number 13, and
  Blood Gulch's three 'a50_rock_large' numbers 19 to 21 and its
  tilted 'teleporter_base' number 24. A converter should skip a scenery
  or light fixture placement whose bit for the BSP in use is clear. The
  other types (vehicle, machine, sound scenery, and the empty ones) are
  created without this test, and the disc agrees: their field is 0 on
  every placement.
- Difficulty flags matter only to the campaign. Multiplayer ignores
  flags 1 to 3 and the variant.
- Creation order (reference): at load the automatic placements of the
  types that are not scenery or light fixtures are created first, one
  type after another in the engine's type order (biped, vehicle,
  weapon, equipment, machine, control, sound scenery), each in block
  order; then scenery and light fixtures, with the on-BSP test above.
  The order has no visible effect on a one-BSP map.

## Skies and the sky tag

### The skies block (0x30)

Elements are bare tag references to `sky ` tags. A structure BSP
cluster's sky index (`structure_bsp.md`, "Clusters") selects an element;
-1 selects none. The disc: 11 of 13 maps have exactly one sky
reference, which is element 0; Chiron TL-34 (`putput`) and Rat Race
have none. The 11 references name 10 distinct sky tags (Longest and
Sidewinder share one):

| Map | Sky tag | Its model (class `mode`) |
|---|---|---|
| beavercreek | levels/test/beavercreek/beavercreek sky | sky/mp clear afternoon/mp clear afternoon |
| bloodgulch | levels/test/bloodgulch/bloodgulch | sky/mp clear afternoon/mp clear afternoon |
| boardingaction | levels/test/boardingaction/boardingaction space | sky/space/space |
| carousel | levels/test/carousel/carousel | sky/sky_x10/sky_x10 |
| chillout | levels/test/chillout/chillout sky | null |
| damnation | sky/mp_dusk/mp_dusk | sky/mp_dusk/mp_dusk |
| hangemhigh | levels/test/hangemhigh/hangemhigh | sky/mp clear afternoon/mp clear afternoon |
| longest, sidewinder | sky/sky_sidewinder/sky_sidewinder | sky/sky_sidewinder/sky_sidewinder |
| prisoner | levels/test/prisoner/prisoner sky | sky/mp clear afternoon/mp clear afternoon |
| wizard | levels/test/wizard/wizard sky | sky/mp clear afternoon/mp clear afternoon |

### The sky tag (class `sky `)

Main struct 0xD0 bytes (the lights block is its last field); every
field below was read on all 10 distinct sky tags.

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 16 | tag ref | model | class `mode` on the Xbox; null on Chill Out's sky |
| 0x10 | 16 | tag ref | animation graph | class `antr`; null on every disc sky |
| 0x20 | 24 | | unused | zero |
| 0x38 | 12 | 3 x f32 | indoor ambient radiosity colour | r, g, b in 0..1 |
| 0x44 | 4 | f32 | indoor ambient radiosity power | |
| 0x48 | 12 | 3 x f32 | outdoor ambient radiosity colour | |
| 0x54 | 4 | f32 | outdoor ambient radiosity power | |
| 0x58 | 32 | fog | outdoor fog | layout below |
| 0x78 | 32 | fog | indoor fog | same layout |
| 0x98 | 16 | tag ref | indoor fog screen | class `fog `; null on every disc sky |
| 0xA8 | 4 | | unused | zero |
| 0xAC | 12 | block | shader functions | element 36 (all unused bytes in the reference); 0 on the disc |
| 0xB8 | 12 | block | animations | element 36: i16 animation index into the graph, 2 pad, f32 period in seconds, 28 unused; 0 on the disc |
| 0xC4 | 12 | block | lights | element 116 (0x74); 1 per sky, 2 on Boarding Action |

Fog (32 bytes):

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 12 | 3 x f32 | colour | r, g, b in 0..1 |
| 0x0C | 8 | | unused | zero |
| 0x14 | 4 | f32 | maximum density | 0..1; the share of the fog colour at full fog |
| 0x18 | 4 | f32 | start distance | WU: no fog nearer than this |
| 0x1C | 4 | f32 | opaque distance | WU: full density at and beyond this; 0 means no fog |

Light (116 bytes):

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 16 | tag ref | lens flare | class `lens`; null on Boarding Action's second light |
| 0x10 | 32 | string | marker name | a marker on the sky model whose position gives the direction; empty on every disc light |
| 0x30 | 28 | | unused | zero |
| 0x4C | 4 | flags | flags | bit 0 affects exteriors, bit 1 affects interiors (names inferred from the editing tools; 3 on the sun, 1 on Boarding Action's fill light) |
| 0x50 | 12 | 3 x f32 | colour | r, g, b |
| 0x5C | 4 | f32 | power | multiplier of the colour |
| 0x60 | 4 | f32 | test distance | 50 on every disc light but carousel's (40) |
| 0x64 | 4 | | unused | zero |
| 0x68 | 4 | f32 | direction yaw | radians |
| 0x6C | 4 | f32 | direction pitch | radians, positive up |
| 0x70 | 4 | f32 | diameter | radians; the sun's angular diameter (0.0035 = 0.2 degrees; 0.0009 on Prisoner and Wizard; 0.698 on Boarding Action's fill light) |

The direction is the direction from the viewer toward the light:
(cos yaw cos pitch, sin yaw cos pitch, sin pitch). Blood Gulch's sun has
yaw 10 degrees and pitch 40 degrees, direction (0.7544, 0.1330,
0.6428), colour (0.906, 0.894, 0.796), power 1.8.

Disc values per sky (all numbers as stored):

| Sky | Indoor ambient colour / power | Outdoor ambient colour / power | Outdoor fog colour; density, start, opaque | Indoor fog colour; density, start, opaque | Sun yaw / pitch (deg), power |
|---|---|---|---|---|---|
| bloodgulch | (0.239, 0.294, 0.373) / 20 | (0.871, 0.843, 0.753) / 0.2 | (1.0, 0.902, 0.769); 0.4, 3, 100 | (0.196, 0.173, 0.329); 1, 1, 120 | 10 / 40, 1.8 |
| beavercreek | as bloodgulch | as bloodgulch | (0.620, 0.635, 0.804); 0.3, 5, 50 | (0.196, 0.173, 0.329); 1, 0.9, 120 | 50 / 80, 1.8 |
| hangemhigh | as bloodgulch | as bloodgulch | (0.533, 0.643, 0.710); 0.2, 0, 32 | (0.196, 0.173, 0.329); 1, 1, 120 | 50 / 75, 1.7 |
| prisoner | as bloodgulch | as bloodgulch | (0.804, 0.851, 0.878); 0.2, 0, 12 | (0.804, 0.851, 0.878); 0.2, 40, 80 | 50 / 75, 10 |
| wizard | as bloodgulch | as bloodgulch | (0.588, 0.690, 0.722); 0.1, 0, 8 | (0.714, 0.784, 0.804); 0.2, 40, 80 | 50 / 40, 10 |
| chillout | (0, 0, 0) / 0 | (0.871, 0.843, 0.753) / 0 | (0.620, 0.635, 0.804); 1, 5, 90 | (0.600, 0.745, 0.890); 0.2, 0, 18 | 50 / 75, 1.7 |
| damnation | (0.875, 0.875, 0.875) / 0 | (1, 1, 1) / 0.1 | (0.373, 0.404, 0.553); 0.5, 0, 160 | (0.196, 0.173, 0.329); 1, 0.9, 120 | -138 / 26, 1.1 |
| longest, sidewinder | (0, 0, 0) / 0 | (0.529, 0.526, 0.541) / 0.2 | (0.600, 0.557, 0.573); 0.5, 0, 160 | (0.196, 0.173, 0.329); 1, 1, 120 | -138 / 26, 1.1 |
| boardingaction | (0, 0, 0) / 0 | (0.992, 0.996, 0.871) / 0.17 | (0.059, 0.059, 0.075); 0.2, 0, 70 | (0, 0, 0); 1, 1, 120 | 265 / 10, 1.4; fill light (0.196, 0.220, 0.396) power 0.6 pitch 90, no flare |
| carousel | (0.290, 0.326, 0.616) / 0.12 | same | (0, 0, 0); 0.001, 0, 10000 | (0, 0, 0); 1, 1, 120 | 75 / 90, power 0 |

(Indoor fog (1, 1, 120) with the colour (0.196, 0.173, 0.329) is the
tool's default and appears unedited on many skies.)

### How a renderer uses the sky

Reference unless marked; the disc fixes only the data.

1. **Which sky.** The camera's cluster has a sky index; that index
   picks the sky element. If the sky exists and has a model, the model
   is drawn (a cluster with sky index -1, or a sky without a model,
   draws no sky model).
2. **The sky model is drawn first**, before objects and the level, so
   that everything else covers it, and it is drawn immediately with no
   depth test, no depth write and no fog. It is lit with a full white
   ambient and nothing else: draw it unlit (its own textures are all the
   colour it has).
3. **Placement.** The model's nodes get the model's rest pose
   (plus animation, none on the disc). Then every node transform is
   post-multiplied by one view matrix: uniform scale 1/1024, no
   rotation (the sky does not turn with the camera; it is fixed in
   world orientation), and translation 1023/1024 times the camera's
   position. So the sky model is authored about 1024 times larger than
   the world, is centred almost on the camera, and shifts only by
   1/1024 of the camera's own distance from the origin (the faint
   parallax: 0.15 WU at Blood Gulch's far end), and its orientation is
   the world's.
4. **Lens flares.** For each sky light with a lens flare: the direction
   is the light's marker on the model when the marker name is not empty
   (direction from the camera to the marker's transformed position);
   otherwise the direction from yaw and pitch above. The flare is placed
   at camera position + direction x 1023.875, facing back toward the
   camera, drawn white at full scale. (Flare drawing, occlusion and
   fading are `effects.md`'s; `lens` tags are not specced here.)
5. **Fog.** The outdoor fog applies when the camera's cluster has a
   sky; the indoor fog of sky 0 when it does not. The fog factor of a
   point is clamp((d - start) / (opaque - start), 0, 1) x max density,
   where d is the point's depth along the camera's forward axis (not the
   straight-line distance); the result blends toward the fog colour. A
   density of 0 or less is treated as 1; an opaque distance of 0
   switches fog off; the fog distances are clamped so opaque is not
   below start. The values do not jump when the
   camera moves between outdoors and indoors: while the camera moved
   less than 15 WU since the last update, start and opaque distances
   move toward the target by at most the distance moved (in WU), and
   density, colour and the screen-fog mix by at most 5 percent of it;
   a bigger jump (a teleport, a respawn) snaps to the target. With no
   sky at all (`putput`, `ratrace`) there is no fog.
6. **Indoor fog screen.** A sky with a `fog ` tag in the indoor fog
   screen field adds a full-screen fog effect while the camera is
   indoors. Null on every disc sky, so unused here.
7. **Ambient radiosity colours and powers, and each light's colour,
   power, test distance, diameter and flags** have no reader in the
   engine's per-frame code (reference); they are inputs to the level
   compiler's lighting pass. The game lights objects from the lightmap
   under them (`structure_bsp.md`). A first renderer can ignore them.

## Player starting locations

Block at 0x354, element 52 bytes (0x34). Count on the disc: 48 to 76
(table below).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 12 | 3 x f32 | position | WU; where the player's feet (the biped's origin) appear |
| 0x0C | 4 | f32 | facing | heading in radians in [-pi, pi]; forward is (cos, sin, 0) |
| 0x10 | 2 | i16 | team index | 0 red, 1 blue; 2 and 3 occur |
| 0x12 | 2 | i16 | BSP index | element of the structure BSP block; 0 on every start on the disc |
| 0x14 | 8 | 4 x i16 | game types | up to four game type numbers; unused slots hold 0 (none) |
| 0x1C | 24 | | unused | zero |

On the disc only the first game type slot is ever nonzero; the other
three are 0.

### Game types

The numbers the game types and wildcards carry (they are the game
engine's own numbering, used by player starts, netgame equipment and
starting equipment alike):

| Value | Meaning |
|---|---|
| 0 | none (a free slot) |
| 1 | capture the flag |
| 2 | slayer |
| 3 | oddball |
| 4 | king of the hill |
| 5 | race |
| 6, 7 | terminator and a stub engine; never used by a map |
| 12 | all game types |
| 13 | all game types except capture the flag |
| 14 | all game types except capture the flag and race |

Values 8 to 11 are not used. Values found on the disc: starts use 1,
5, 12, 13 and 14; netgame equipment and starting equipment also use 2,
3 and 4.

**Eligibility.** A set of four slots is eligible for the game type g in
play when any slot s satisfies: s equals g; or s is 12; or s is 13 and g
is not 1; or s is 14 and g is neither 1 nor 5. A 0 slot never matches a
running game. (When no game is running, for instance in a solo map,
a set is eligible only if every slot is 0; no multiplayer entry then
qualifies.)

How spawn points are then chosen (random ratings, distance to
enemies, vehicles standing on the spot) is `multiplayer_rules.md`. What
the scenario fixes: only eligible starts are candidates; in a capture
the flag game a candidate's team index must equal the player's team,
and in the other game types the team index of a start is not tested
(reference; the port's optional "force team spawns" rule is not part of
the disc game). A converter should keep all four slots, the team index
and the heading, and let the game rules filter.

## Starting profiles and starting equipment

**Starting profiles** (0x348, element 104): campaign loadouts, empty on
the 13 multiplayer maps. Fields (campaign-checked sizes; the weapon
references were null on `a10`): name (32 bytes); f32 health modifier
at 0x20 and f32 shield modifier at 0x24 (both 1.0 on a10); primary
weapon at 0x28: tag reference `weap` (16), i16 rounds loaded at 0x38,
i16 rounds total at 0x3A; secondary weapon at 0x3C in the same layout
(0x4C and 0x4E); two bytes of grenade counts at 0x50 and 0x51; 22
unused bytes.

**Starting equipment** (0x390, element 204 = 0xCC): what a player holds
when spawning in a multiplayer game.

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | flags | flags | bit 0: no grenades; bit 1: plasma grenades in place of fragmentation grenades (their count is moved) |
| 0x04 | 8 | 4 x i16 | game types | as above |
| 0x0C | 48 | | unused | zero |
| 0x3C | 96 | 6 x tag ref | item collections | six tag references, class `itmc`; null ones keep the class |
| 0x9C | 48 | | unused | zero |

Use (reference): the entries are walked in block order; the first whose
game types are eligible for the game in play wins and the walk stops. For
each of its first five collections that is not null, one item is drawn
from the collection (see "Item collections") and put in the player's
hands; a later item of a kind the player already has is discarded. The
first becomes the primary weapon. Then the flags adjust the grenade
counts the game variant already set. (The sixth collection is never
visited by the game; the disc uses at most two.)

On the disc: 0 to 3 entries per map (Rat Race has none; what a
player holds there is not established). All entries use
`item collections/single weapons/...` (pistol, assault rifle, plasma
pistol, plasma rifle, needler, shotgun); examples: Blood Gulch has
(CTF, no flags, assault rifle then pistol) and (all-but-CTF, flag 1
"no grenades", plasma pistol); Hang 'Em High has (all-but-CTF-and-race
`14`, flag 1, pistol), (CTF, flag 2, plasma rifle) and (race, flag 1,
plasma pistol). The flag values seen are 0, 1 and 2.

## Netgame flags

Block at 0x378, element 148 (0x94).

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 12 | 3 x f32 | position | WU |
| 0x0C | 4 | f32 | facing | heading in radians |
| 0x10 | 2 | enum | type | see below |
| 0x12 | 2 | i16 | team index | meaning depends on the type |
| 0x14 | 16 | tag ref | reference | class `itmc` (one flag on Blood Gulch carries a stale class); null on every disc flag |
| 0x24 | 112 | | unused | zero |

Type values and what the team index counts. Counts are over the 13
maps; the per-map counts are in "Measured facts".

| Type | Name | Team index is | Use |
|---|---|---|---|
| 0 | capture the flag flag | the owning team (0 red, 1 blue) | where a team's flag sits and is returned to; every map has exactly one per team |
| 1 | capture the flag vehicle | | never present on the disc |
| 2 | oddball ball spawn | the ball's number (0 to 15) | where a ball is created; 2 to 10 per map |
| 3 | race track | the checkpoint's sequence number | a checkpoint; touched within 2.5 WU from a vehicle, or 1.5 WU and 0.6 WU of height on foot (reference) |
| 4 | race vehicle | | where race vehicles appear; 16 on Blood Gulch (team 1) and on Sidewinder (team 0), none elsewhere |
| 5 | bank (the engine's name for it) | | never present |
| 6 | teleporter source | the pair number | step on it to be teleported |
| 7 | teleporter target | the pair number | where the source's pair delivers you |
| 8 | hill | the hill's number | one corner of a hill's outline |

**Teleporter pairs.** A source and a target belong together when their
team indices are equal. On every map each team index that has a source
has exactly one target (checked on all 10 maps that have
teleporters; Hang 'Em High, Longest and Prisoner have none). The
reference: each tick,
for each living player, find a source flag whose position lies within
0.5 WU (3D distance from the player's origin) of the player; ignore it
if it is the flag the player last arrived at (a flag stays disarmed for
a player until the player has moved more than 1 WU away from it). Take
the first target flag in block order whose team index equals the
source's. If a living biped or a vehicle occupies the destination (a
pill the size of the player's, with its bottom-sphere centre at the
target flag, overlaps it: `multiplayer_rules.md` 6.8 has the numbers),
the teleport is refused and a message shows. Otherwise the player's
position becomes exactly the target's position, and the player's
heading (the angle of the body's facing vector in the ground plane)
becomes old heading + target facing - source facing; the new facing
vector is (cos, sin, 0) of that angle. The player is then treated as
having arrived at the first source flag within 1 WU of the new position
(so arriving beside a source does not bounce back). The facing of the
flags is how the designer says "the pad faces this way".

Disc example, Blood Gulch: source team 0 at (95.58, -155.45, 1.81)
facing 1.5872 and target team 0 at (82.83, -114.53, 0.70) facing
3.0917. A player walking straight out of the pad (heading 1.5872)
arrives at the target with heading 1.5872 + 3.0917 - 1.5872 = 3.0917, a
forward vector of (-0.9988, 0.0499, 0).

**Hills.** A hill is the set of type 8 flags with the same team index.
The game takes the first 12 of them in block order, projects them to
the XY plane and keeps their convex hull; one flag alone becomes a 2 WU
square about it; the hill's vertical extent runs from the lowest flag
minus 0.1 WU to the highest plus 0.8 WU (reference). On the disc a hill
has up to 34 flags: Blood Gulch's ten hills have 34, 29, 21, 19, 15
and five of 7 or 8 flags; Sidewinder's largest has 23 and Chiron
TL-34's 18. A hill with more than 12 loses its later flags (5 of Blood
Gulch's hills, 6 of Sidewinder's, 1 of Chiron TL-34's).

## Netgame equipment and item collections

### Netgame equipment (0x384, element 144 = 0x90)

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 4 | flags | flags | bit 0: levitate (the item is created at rest, hanging in the air, not falling). 5 of 535 disc entries (all powerups) |
| 0x04 | 8 | 4 x i16 | game types | as in "Game types" |
| 0x0C | 2 | i16 | team index | 0 on all 535 disc entries; not read |
| 0x0E | 2 | i16 | spawn time | seconds; 0 means "use the collection's" |
| 0x10 | 4 | i32 | runtime spawned item | scratch; 0 on the disc |
| 0x14 | 44 | | unused | zero |
| 0x40 | 12 | 3 x f32 | position | WU |
| 0x4C | 4 | f32 | facing | heading in radians; 0 on 66 of the 80 entries of Blood Gulch and Hang 'Em High, otherwise about +-1.5 (or -0.69, -0.05, 0.02) |
| 0x50 | 16 | tag ref | item collection | class `itmc`; non-null on all 535 |
| 0x60 | 48 | | unused | zero |

**What a spawn does** (reference). While a match is running and the
entry's game types are eligible, the entry has a period in seconds: its
own spawn time if not 0, else its collection's spawn time if not 0,
else 30. Whenever the game time in ticks is an exact multiple of
period x 30, one item is drawn from the collection and created at the
entry's position (a fresh object: with the default orientation, as this
build ignores the facing; later builds apply it). A levitate entry is
created at rest. The item's "last owned" time is advanced by period -
30 s so that the game's cleanup of items nobody picked up (after 30 s
unowned) removes it about when the next one is due. Consequences: an
item respawns on the global clock (multiples of the period from the
start of the match), not a fixed delay after being picked up; and an
entry whose period is 30 s (the common 0, 0 case) keeps a full 30 s
of life. The gametype's "no weapons on the map" option skips entries
whose drawn item is a weapon.

Spawn times on the disc: 0 on 458 entries, 45 on 40, 30 on 17, 180
on 11, 120 on 7, 90 on 2. Effective periods then come from the
collection: 60 s (over shield and camouflage), 120 s (rocket launcher),
180 s (plasma grenades, the shield-camouflage pair), 30 s otherwise.

Game types on the 535 entries: 386 `12`, 73 `1`, 65 `13`, 2 `14`, and
combinations of two or three explicit types (`2,3` three times, `1,5`
twice, `1,2,3` twice, `4,5` twice).

### Item collection (class `itmc`)

Main struct 0x5C bytes.

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 12 | block | permutations | element 84 (0x54) |
| 0x0C | 2 | i16 | spawn time | seconds; 0 means unspecified (then 30); 60, 120, 180 on the disc |
| 0x0E | 2 | | pad | zero |
| 0x10 | 76 | | unused | zero |

Permutation (84 bytes):

| Offset | Size | Type | Field | Meaning |
|---|---|---|---|---|
| 0x00 | 32 | | unused | zero |
| 0x20 | 4 | f32 | weight | relative chance |
| 0x24 | 16 | tag ref | item | class `weap` or `eqip` (object tags) |
| 0x34 | 32 | | unused | zero |

**Drawing** (reference): total = the sum of the weights, truncated to a
whole number; draw an integer r uniformly in [0, total) from the game's
random stream; walk the permutations in order subtracting each weight
(as a real number) from r; the first permutation after which r is
negative is the result. A collection with no permutation yields nothing.

On the disc all 16 collections used by the 13 maps (listed in
"Measured facts") have 1 permutation of weight 100 except:
`powerups/shield-invisibility` (camouflage 50, over shield 50, spawn
time 180) and `powerups/group all powerups` (camouflage 70, over
shield 90, health pack 15; Chiron TL-34 only). Items they reach:
`weapons/<name>/<name>` weapons (assault rifle, pistol, needler,
plasma pistol, plasma rifle, rocket launcher, shotgun, sniper rifle,
flamethrower), `weapons/frag grenade/frag grenade` and `weapons/plasma
grenade/plasma grenade` (both class `eqip`), `powerups/health pack`,
`powerups/over shield` and `powerups/active camouflage` (class `eqip`).

## Object placements by type

All sizes are element sizes; offsets are within one element; the
shared 0x28-byte header comes first. "Disc" says what the multiplayer
maps hold.

### Scenery (0x210, 72 bytes)

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0x00 | 40 | header | as above; palette is the scenery palette |
| 0x28 | 32 | permutation data | four 4-byte change-colour slots, 8 bytes of region permutations, 8 unused; all zero on the disc and never read |

Disc: 264 placements over 13 maps (2 to 50 per map), 0 name indices,
0 flags, rotations: yaw in (-pi, pi], pitch in [-1.51, 0.85], roll in
[-pi, pi]; pitch and roll are 0 for most placements.

### Bipeds (0x228) and vehicles (0x240), 120 bytes

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0x00 | 40 | header | as above |
| 0x28 | 32 | permutation data | as scenery |
| 0x48 | 4 | f32 body vitality | initial body health as a fraction (campaign); 0 on the disc |
| 0x4C | 4 | flags | unit flags (campaign); 0 on the disc |
| 0x50 | 40 | | unused, zero |

Disc: no bipeds. Blood Gulch and Sidewinder place 8 vehicles each:
2 warthogs (`vehicles/warthog/warthog`), 4 ghosts
(`vehicles/ghost/ghost_mp`), 2 scorpions
(`vehicles/scorpion/scorpion_mp`), every one with flags 0, no name,
no variant, and every byte after the rotation zero. **The disc's vehicle
placements carry no per-game-type spawn flags and no team** (the PC game
has them in these bytes; here they are zero): the converter places
every vehicle placement unconditionally, and it is the game's
multiplayer rules that decide which vehicles a game variant wants.
The on-BSP flags are not tested for vehicles (they are 0 on all of
them); a vehicle placement's position is the vehicle's **centre of
mass**, not its model origin. The exact creation rule is `vehicles.md`
(Multiplayer, "Which placements spawn").
The race game's own vehicle spots are the type 4 netgame flags.

### Equipment (0x258, 40 bytes) and sound scenery (0x2DC, 40 bytes)

Header only. Equipment's miscellaneous flags: bit 0 levitate (created
at rest; otherwise the item is lifted 0.05 WU at creation), bit 1 does
not orient to ground. None on the disc. Sound scenery: 8 on Beaver
Creek (`sound/sfx/ambience/multiplayer/stream`), 2 on Blood Gulch
(`sound/sfx/ui/teleporter_loop`), 3 on Damnation
(`sound/sfx/ambience/multiplayer/damnation_falls`); rotation is zero
and only the position matters.

### Weapons (0x270, 92 bytes)

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0x00 | 40 | header | as above |
| 0x28 | 32 | permutation data | as scenery |
| 0x48 | 2 | i16 rounds total | spare rounds |
| 0x4A | 2 | i16 rounds loaded | rounds in the magazine |
| 0x4C | 2 | flags | bit 0 initially at rest, bit 1 obsolete, bit 2 does accelerate |
| 0x4E | 2 | | pad |
| 0x50 | 12 | | unused |

Reference only: no multiplayer map places a weapon (Damnation lists 2
palette entries and places none); weapons arrive through netgame
equipment.

### Machines (0x294, 64 bytes), controls (0x2AC, 64 bytes)

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0x00 | 40 | header | as above |
| 0x28 | 2 | i16 power group | element of device groups; -1 none |
| 0x2A | 2 | i16 position group | element of device groups; -1 none |
| 0x2C | 4 | flags | device flags: bit 0 initially open, bit 1 initially off, bit 2 changes only once, bit 3 position reversed, bit 4 not usable from any side |
| 0x30 | 1 | flags | machines: bit 0 does not operate automatically, bit 1 one-sided, bit 2 never appears locked, bit 3 opened by melee. Controls: a 2-byte flags word here |
| 0x34 | | | machines: 12 unused bytes to 0x40. Controls: i16 custom name index at 0x34, then 10 unused bytes |

Disc: only Wizard has machines: 4, all of palette entry 0
(`levels/test/wizard/devices/wiz_alert_light/wiz_alert_light`, the alert
light in each corner of the room; the palette also lists
`levels/a10/devices/alert_light/alert_light`), groups -1, all flags 0.
No controls.

### Light fixtures (0x2C4, 88 bytes)

| Offset | Size | Field | Meaning |
|---|---|---|---|
| 0x00 | 40 | header | as above |
| 0x28 | 8 | device | power group, position group (i16 each), device flags (u32), as machines |
| 0x30 | 12 | 3 x f32 | colour |
| 0x3C | 4 | f32 | intensity |
| 0x40 | 4 | f32 | falloff angle |
| 0x44 | 4 | f32 | cutoff angle |
| 0x48 | 16 | | unused |

Disc: Derelict (carousel) 13, Damnation 28, Rat Race 5, all with
groups -1, device flags 0, colour, intensity and both angles 0 (the
light presumably comes from the fixture tag, not the placement). On-BSP
flag 1.

### Device groups (0x288, 52 bytes)

Name (32 bytes), f32 initial value, u32 flags, 12 unused bytes; none on
the disc.

## Scenery as a renderer needs it

A first map converter that places static models does this per scenery
placement:

1. Read palette index i (i16 at element offset 0). If it is -1 or not
   below the palette's count, skip. Skip the placement if its on-BSP
   bit for the BSP in use is clear.
2. The palette entry's tag reference names the `scen` tag. The scenery
   tag begins with the shared object header (the full object tag layout
   is `objects.md`'s). Offsets within the tag's main struct, all checked
   on the disc:

| Offset | Size | Type | Field |
|---|---|---|---|
| 0x00 | 2 | enum | object type (6 for scenery; 0 biped, 1 vehicle, 2 weapon, 3 equipment, 4 garbage, 5 projectile, 7 machine, 8 control, 9 light fixture, 11 sound scenery) |
| 0x02 | 2 | flags | object flags |
| 0x04 | 4 | f32 | bounding radius (WU) |
| 0x08 | 12 | 3 x f32 | bounding offset: centre of the bounding sphere relative to the origin (non-zero for 6 of the 19 scenery tags in Blood Gulch's palette, always along +Z or -Z) |
| 0x14 | 12 | 3 x f32 | origin offset (0 for every disc scenery) |
| 0x20 | 4 | f32 | acceleration scale |
| 0x28 | 16 | tag ref | model, class `mode` (non-null on all 19 scenery tags of Blood Gulch's palette) |
| 0x38 | 16 | tag ref | animation graph, class `antr` (null on most; one plant has one) |
| 0x70 | 16 | tag ref | collision model, class `coll` |
| 0x80 | 16 | tag ref | physics, class `phys` |
| 0x90 | 16 | tag ref | modifier shader (class `shdr` family; `spla` on 10) |
| 0xA0 | 16 | tag ref | creation effect, class `effe` |
| 0x104 | 4 | f32 | render bounding radius (equals the bounding radius on every scenery checked) |

3. Place the model's tag (models are `models.md`'s) with the object's
   origin at the placement position and axes F, L, U from "Rotation and
   facing": a model vertex at model-space p goes to position + p.x F +
   p.y L + p.z U (the origin offset above is not applied to the model;
   it is zero anyway). The model is drawn at its authored scale.
4. A scenery tag whose model reference is null draws nothing. Objects
   with a collision model also block movement and projectiles (not every
   scenery tag has one: Blood Gulch's 'shrub_large', 'plant fern',
   'teleporter', beacons and floor arrows have a model and no collision
   model); collision is
   `collision.md`'s.
5. Many scenery tags are shared across maps (the same `scenery/rocks/...`
   path in different maps), and the tags' contents are byte-identical
   apart from addresses and ids (`cache_file.md`, donor-map question).

Static lighting: a placed object's lighting comes from the lightmap
sample under it (`structure_bsp.md`), not from the scenario.

## The other blocks

**Trigger volumes** (0x360, 96 bytes; none on the 13 maps; campaign
layout): i16 type (0 axis-aligned box, 1 oriented box), 2 pad, name (32
bytes), 12 unused, forward vector (3 x f32 at 0x30), up vector (3 x f32
at 0x3C), position (3 x f32 at 0x48), extents (3 x f32 at 0x54). The
volume is the box with one corner at the position and sides extents
along the forward, left and up vectors (on a10: forward (1, 0, 0), up
(0, 0, 1)). Used by scripts only.

**Decals** (0x3A8, 16 bytes) and the decal palette (0x3B4, 16 bytes:
only a tag reference, class `deca`). Placement: i16 palette index; two
signed bytes (called yaw and pitch in the tools; the scale is not
established; Blood Gulch's five distinct pairs are (-120, 42), (70,
42), (8, 45), (-61, -26) and (56, -42)); then the position, 3 x f32 at
offset 4. Counts: Beaver Creek 8, Blood Gulch 8, Boarding Action 55,
Hang 'Em High 2, Longest 2, Chiron TL-34 7, Sidewinder 24. The
reference's game never reads these: the decals it draws are the
structure BSP's own decal block (`structure_bsp.md`; 8 on Blood Gulch as
well). Treat the scenario's decal blocks as authoring leftovers.

**Detail object collection palette** (0x3C0, 48 bytes): tag reference
(class `dobc`) + 32 unused. Empty on all 13; the campaign (a30) lists
grass collections. The BSP's detail-object block is what places them.

**Cutscene flags, camera points, BSP switch volumes, predicted
resources**: see the top-level table and the next section.

**Structure BSPs** (0x5A4, 32 bytes): file offset, size, load address,
unused, `sbsp` tag reference; fully specified in `structure_bsp.md`.
One element on every multiplayer map; the player starts' BSP index is
0 everywhere.

## Blocks to step over

Not used by a multiplayer match; the element sizes are enough to skip
them.

| Block | Elem | One line |
|---|---|---|
| child scenarios | 32 | tag reference + 16 unused |
| functions | ? | scenario-wide function curves; never present |
| comments | 48 | editor notes with a text data reference |
| recorded animations | 64 | a recorded unit's control stream (data reference) |
| AI actor palette | 16 | tag references to actor variants |
| AI encounters | 176 | squads, firing positions, move positions (sub-blocks) |
| AI command lists | 96 | scripted command sequences |
| AI animation / script / recording references | 60 / 40 / 40 | names the scripts use |
| AI conversations | 116 | speaking parts and lines |
| script syntax data, string constants | data | compiled script nodes and strings |
| scripts, globals, references, source files | 92, 92, 40, 52 | script tables |
| cutscene flags | 92 | name (32 at 4), position (3 x f32 at 0x24), yaw and pitch (2 x f32 at 0x30), unused |
| cutscene camera points | 104 | flags, name (32 at 4), position at 0x28, yaw/pitch/roll at 0x34, field of view (radians) at 0x40 |
| cutscene titles | 96 | on-screen chapter titles |
| BSP switch trigger volumes | 8 | i16 trigger volume, source BSP, destination BSP, cutscene flag |
| predicted resources | 8 | i16 type, i16 picture index, u32 tag id; a prefetch list |

## Order of use

At map load (reference):

1. Read the scenario; resolve the structure BSP block and load the BSP
   (`structure_bsp.md`).
2. Create the automatic placements of the types that are not scenery or
   light fixtures, in type order, each in block order (on a multiplayer
   disc map: vehicles, machines, sound scenery).
3. For scenery and light fixtures: set each placement's on-BSP bit for
   this BSP from its position, then create those with the bit set.
4. Draw: sky model first, then objects and level (section "How a
   renderer uses the sky").

Each tick of a running match (reference):

1. For each living player, the teleporter check ("Netgame flags").
2. For each netgame equipment entry in block order whose game types
   are eligible: if the game time is a multiple of the entry's period
   in ticks, draw an item from its collection and create it.

When a player spawns (reference): pick an eligible start
(`multiplayer_rules.md`), create the biped at the start's position
facing (cos h, sin h, 0), then give the starting equipment of the first
eligible entry.

## Measured facts

All 13 multiplayer maps, read on the disc. Map names are the files'
names; carousel is Derelict, putput is Chiron TL-34.

**Player starts** by game type (CTF is `1`; `5` is race; `12`, `13`
and `14` are the wildcards; "t" is the team index).

| Map | Starts | CTF red / blue | Race | Wildcard starts (type: team count) |
|---|---|---|---|---|
| beavercreek | 70 | 26 / 22 | - | 12: t0 4, t1 8; 13: t0 6, t1 4 |
| bloodgulch | 71 | 17 / 21 | 17 | 14: t0 8, t1 8 |
| boardingaction | 73 | 27 / 25 (+5 other teams) | - | 13: t0 16 |
| carousel | 63 | 18 / 18 | - | 13: t0 27 |
| chillout | 58 | 16 / 17 | - | 13: t0 25 |
| damnation | 67 | 19 / 18 | - | 13: t1 30 |
| hangemhigh | 67 | 19 / 20 | - | 13: t0 10, t1 18 |
| longest | 71 | 20 / 23 | - | 13: t0 28 |
| prisoner | 58 | 16 / 18 | - | 13: t0 21, t1 1, t2 1, t3 1 |
| putput | 59 | 16 / 16 | - | 13: t0 15, t1 12 |
| ratrace | 48 | 0 / 0 | - | 12: t0 14, t1 17; 13: t0 5, t1 8, t3 4 |
| sidewinder | 76 | 21 / 23 | 16 | 14: t0 16 |
| wizard | 48 | 16 / 16 | - | 13: t0 1, t1 1, t2 1, t3 13 |

Blood Gulch's race starts are teams 0 to 3: 2, 7, 6, 2. Boarding
Action has 5 more CTF starts with team index 2 (2) and 3 (3).
Prisoner and Wizard have wildcard starts of teams 2 and 3, Rat Race of
team 3. Facings are spread over [-pi, pi] on every map.

**Netgame flags** by type (team indices omitted; one capture flag per
team on every map).

| Map | CTF | Tele src | Tele tgt | Oddball | Race track | Race veh | Hill flags (hills) | Total |
|---|---|---|---|---|---|---|---|---|
| beavercreek | 2 | 2 | 2 | 8 | 11 | - | 72 (10 hills) | 97 |
| bloodgulch | 2 | 2 | 2 | 9 | 5 | 16 | 155 (10 hills) | 191 |
| boardingaction | 2 | 8 | 8 | 10 | 13 | - | 36 (9 hills) | 77 |
| carousel | 2 | 4 | 4 | 5 | 10 | - | 54 (10 hills) | 79 |
| chillout | 2 | 3 | 3 | 8 | 12 | - | 21 (4 hills) | 49 |
| damnation | 2 | 2 | 2 | 8 | 4 | - | 61 (8 hills) | 79 |
| hangemhigh | 2 | 0 | 0 | 5 | 11 | - | 74 (14 hills) | 92 |
| longest | 2 | 0 | 0 | 8 | 5 | - | 36 (9 hills) | 51 |
| prisoner | 2 | 0 | 0 | 2 | 9 | - | 34 (6 hills) | 47 |
| putput | 2 | 30 | 30 | 5 | 9 | - | 58 (9 hills) | 134 |
| ratrace | 2 | 4 | 4 | 7 | 4 | - | 22 (4 hills) | 43 |
| sidewinder | 2 | 4 | 4 | 8 | 5 | 16 | 127 (10 hills) | 166 |
| wizard | 2 | 4 | 4 | 5 | 4 | - | 40 (9 hills) | 59 |

**Everything else.** "Scenery" is placements / distinct tags actually
used / palette entries. Every sky is element 0 of the skies block.

| Map | Netgame equipment | Vehicles | Scenery | Machines | Light fixtures | Sound scenery | Decals | Sky tag |
|---|---|---|---|---|---|---|---|---|
| beavercreek | 35 | - | 30 / 7 / 9 | 0 | 0 | 8 | 8 | levels/test/beavercreek/beavercreek sky |
| bloodgulch | 35 | 2 warthog, 4 ghost, 2 scorpion | 50 / 15 / 19 | 0 | 0 | 2 | 8 | levels/test/bloodgulch/bloodgulch |
| boardingaction | 52 | - | 29 / 6 / 7 | 0 | 0 | 0 | 55 | levels/test/boardingaction/boardingaction space |
| carousel | 37 | - | 40 / 5 / 5 | 0 | 13 | 0 | 0 | levels/test/carousel/carousel |
| chillout | 33 | - | 8 / 3 / 4 | 0 | 0 | 0 | 0 | levels/test/chillout/chillout sky |
| damnation | 46 | - | 12 / 5 / 6 | 0 | 28 | 3 | 0 | sky/mp_dusk/mp_dusk |
| hangemhigh | 45 | - | 8 / 2 / 2 | 0 | 0 | 0 | 2 | levels/test/hangemhigh/hangemhigh |
| longest | 40 | - | 4 / 2 / 2 | 0 | 0 | 0 | 2 | sky/sky_sidewinder/sky_sidewinder |
| prisoner | 51 | - | 2 / 1 / 1 | 0 | 0 | 0 | 0 | levels/test/prisoner/prisoner sky |
| putput | 40 | - | 32 / 2 / 2 | 0 | 0 | 0 | 7 | none |
| ratrace | 30 | - | 8 / 3 / 3 | 0 | 5 | 0 | 0 | none |
| sidewinder | 47 | 2 warthog, 4 ghost, 2 scorpion | 35 / 10 / 12 | 0 | 0 | 0 | 24 | sky/sky_sidewinder/sky_sidewinder |
| wizard | 44 | - | 6 / 2 / 2 | 4 | 0 | 0 | 0 | levels/test/wizard/wizard sky |

Every map has exactly one structure BSP and 0 weapons, bipeds, equipment
placements, controls, device groups, trigger volumes, scripts,
encounters, starting profiles and detail-object palette entries.
Wizard has 4 machines; carousel, damnation and ratrace have light
fixtures. All 13 have scenario type 1 (multiplayer).

**Netgame equipment by item collection** (collection path is `item
collections/<group>/<name>`; "single weapons" unless stated):

| Map | Entries by collection |
|---|---|
| beavercreek | assault rifle 2, frag grenades 8, needler 3, plasma grenades 8, plasma rifle 2, powerup health pack 4, powerup invisibility 3, powerup super shield 2, rocket launcher 1, sniper rifle 2 |
| bloodgulch | assault rifle 2, frag grenades 8, pistol 4, plasma grenades 8, plasma rifle 2, powerup health pack 2, powerup super shield 1, rocket launcher 1, shield-invisibility 1, shotgun 4, sniper rifle 2 |
| boardingaction | assault rifle 2, frag grenades 8, needler 4, plasma grenades 16, plasma rifle 3, powerup health pack 7, powerup invisibility 2, powerup super shield 2, rocket launcher 4, shotgun 2, sniper rifle 2 |
| carousel | assault rifle 1, flame thrower 1, frag grenades 8, needler 2, pistol 4, plasma grenades 8, powerup health pack 1, powerup invisibility 3, powerup super shield 3, rocket launcher 1, shotgun 2, sniper rifle 3 |
| chillout | assault rifle 1, frag grenades 8, needler 2, pistol 2, plasma grenades 8, plasma rifle 2, powerup health pack 4, powerup invisibility 2, powerup super shield 1, rocket launcher 1, shotgun 1, sniper rifle 1 |
| damnation | assault rifle 4, frag grenades 16, pistol 1, plasma grenades 8, plasma pistol 1, plasma rifle 3, powerup health pack 4, powerup invisibility 1, powerup super shield 1, rocket launcher 1, shotgun 4, sniper rifle 2 |
| hangemhigh | assault rifle 2, frag grenades 16, needler 3, pistol 2, plasma grenades 8, plasma rifle 2, powerup health pack 2, powerup invisibility 1, rocket launcher 1, shield-invisibility 1, shotgun 4, sniper rifle 3 |
| longest | assault rifle 5, frag grenades 16, pistol 2, plasma grenades 8, plasma rifle 1, powerup health pack 4, powerup invisibility 1, powerup super shield 1, shotgun 2 |
| prisoner | assault rifle 2, frag grenades 12, plasma grenades 16, plasma rifle 3, powerup health pack 5, powerup invisibility 1, powerup super shield 3, rocket launcher 1, shotgun 4, sniper rifle 4 |
| putput | assault rifle 1, frag grenades 8, group all powerups 1, pistol 2, plasma grenades 8, plasma pistol 4, plasma rifle 1, powerup health pack 13, rocket launcher 1, shotgun 1 |
| ratrace | assault rifle 2, frag grenades 8, pistol 4, plasma grenades 8, plasma rifle 1, powerup health pack 2, powerup invisibility 1, powerup super shield 1, shotgun 3 |
| sidewinder | assault rifle 2, frag grenades 16, needler 4, pistol 2, plasma grenades 8, plasma rifle 2, powerup health pack 2, powerup invisibility 1, powerup super shield 2, rocket launcher 2, shotgun 4, sniper rifle 2 |
| wizard | assault rifle 3, frag grenades 8, needler 2, pistol 6, plasma grenades 8, plasma rifle 3, powerup health pack 4, powerup invisibility 2, powerup super shield 2, rocket launcher 2, shield-invisibility 2, shotgun 2 |

Collections referenced anywhere: 16, listed with their spawn times:
assault rifle, flame thrower (Derelict only), frag grenades,
needler, pistol, plasma grenades (180 s), plasma pistol, plasma rifle,
rocket launcher (120 s), shotgun, sniper rifle (all "single weapons",
the others 0 = default); `powerups/powerup health pack`,
`powerups/powerup invisibility` (60 s), `powerups/powerup super shield`
(60 s), `powerups/shield-invisibility` (180 s),
`powerups/group all powerups`.

### Blood Gulch in detail

71 player starts: CTF red 17, CTF blue 21, race 17 (teams 0 to 3: 2, 7,
6, 2) and 16 of type 14 (8 red, 8 blue). 191 netgame flags: 2 capture
flags (blue at (40.24, -79.12, -0.10), red at (95.69, -159.45,
-0.10), facing 0), 2 teleporter sources and 2 targets (above), 9
oddball spawns, 5 race checkpoints, 16 race vehicle spots, 155 hill
flags in 10 hills. 35 netgame equipment entries, 8 vehicles, 50
scenery placements (15 of 19 palette types), 2 sound scenery, 8
decals, 1 sky (`levels/test/bloodgulch/bloodgulch`), 1 BSP with world
bounds x 5.91 .. 131.90, y -190.22 .. -45.06, z -0.35 .. 26.23.

Netgame equipment: "spawn" is the entry's field in seconds (0 means the
collection's), "period" the resulting period, "lev" a levitate flag.

| # | Collection | x | y | z | Game types | Spawn | Period | Flag |
|---|---|---|---|---|---|---|---|---|
| 0 | single weapons/shotgun | 38.43 | -79.19 | -0.29 | all | 0 | 30 |  |
| 1 | single weapons/shotgun | 41.83 | -79.24 | -0.29 | all | 0 | 30 |  |
| 2 | single weapons/assault rifle | 40.13 | -77.84 | -0.29 | all | 0 | 30 |  |
| 3 | single weapons/plasma rifle | 40.16 | -80.45 | -0.29 | all | 0 | 30 |  |
| 4 | single weapons/assault rifle | 95.66 | -158.28 | -0.29 | all | 0 | 30 |  |
| 5 | single weapons/plasma rifle | 95.56 | -160.91 | -0.29 | all | 0 | 30 |  |
| 6 | single weapons/shotgun | 97.14 | -159.55 | -0.29 | all | 0 | 30 |  |
| 7 | single weapons/shotgun | 93.81 | -159.38 | -0.29 | all | 0 | 30 |  |
| 8 | powerups/shield-invisibility | 59.00 | -116.85 | 1.84 | all | 0 | 180 |  |
| 9 | powerups/powerup health pack | 37.05 | -82.16 | 1.70 | all | 0 | 30 |  |
| 10 | powerups/powerup health pack | 98.70 | -156.56 | 1.70 | all | 0 | 30 |  |
| 11 | single weapons/frag grenades | 95.58 | -157.23 | 1.70 | all | 0 | 30 |  |
| 12 | single weapons/frag grenades | 95.72 | -157.42 | 1.70 | all | 0 | 30 |  |
| 13 | single weapons/frag grenades | 95.40 | -157.43 | 1.70 | all | 0 | 30 |  |
| 14 | single weapons/frag grenades | 95.57 | -157.64 | 1.70 | all | 0 | 30 |  |
| 15 | single weapons/frag grenades | 40.06 | -81.15 | 1.71 | all | 0 | 30 |  |
| 16 | single weapons/frag grenades | 39.94 | -80.97 | 1.71 | all | 0 | 30 |  |
| 17 | single weapons/frag grenades | 40.21 | -80.94 | 1.71 | all | 0 | 30 |  |
| 18 | single weapons/frag grenades | 40.08 | -80.73 | 1.71 | all | 0 | 30 |  |
| 19 | single weapons/pistol | 95.50 | -162.58 | 1.78 | all | 0 | 30 |  |
| 20 | single weapons/pistol | 40.08 | -76.14 | 1.78 | all | 0 | 30 |  |
| 21 | single weapons/rocket launcher | 72.38 | -126.42 | 1.20 | all | 90 | 90 |  |
| 22 | single weapons/sniper rifle | 43.10 | -82.11 | 1.70 | all | 120 | 120 |  |
| 23 | single weapons/sniper rifle | 92.59 | -156.46 | 1.70 | all | 120 | 120 |  |
| 24 | powerups/powerup super shield | 109.01 | -108.25 | 2.06 | all | 0 | 60 |  |
| 25 | single weapons/plasma grenades | 62.21 | -155.56 | 5.94 | all | 0 | 180 |  |
| 26 | single weapons/plasma grenades | 62.07 | -155.36 | 5.96 | all | 0 | 180 |  |
| 27 | single weapons/plasma grenades | 62.41 | -155.37 | 5.99 | all | 0 | 180 |  |
| 28 | single weapons/plasma grenades | 62.27 | -155.20 | 6.01 | all | 0 | 180 |  |
| 29 | single weapons/plasma grenades | 74.43 | -78.35 | 5.71 | all | 0 | 180 |  |
| 30 | single weapons/plasma grenades | 74.62 | -78.21 | 5.72 | all | 0 | 180 |  |
| 31 | single weapons/plasma grenades | 74.63 | -78.49 | 5.75 | all | 0 | 180 |  |
| 32 | single weapons/plasma grenades | 74.84 | -78.36 | 5.76 | all | 0 | 180 |  |
| 33 | single weapons/pistol | 69.83 | -88.16 | 5.66 | all | 0 | 30 |  |
| 34 | single weapons/pistol | 57.76 | -150.00 | 5.75 | all | 0 | 30 |  |

Player starts (type is the first game type slot; the other three are
0; BSP index 0):

| # | x | y | z | Facing | Team | Type |
|---|---|---|---|---|---|---|
| 0 | 98.49 | -157.64 | 1.70 | 1.3283 | 0 | CTF |
| 1 | 96.82 | -156.99 | 1.70 | 1.5708 | 0 | CTF |
| 2 | 94.42 | -156.70 | 1.70 | 2.0944 | 0 | CTF |
| 3 | 91.89 | -157.78 | 1.70 | 2.3217 | 0 | CTF |
| 4 | 99.19 | -146.67 | 0.14 | 2.4435 | 0 | CTF |
| 5 | 100.96 | -146.25 | 0.29 | 2.4435 | 0 | CTF |
| 6 | 43.91 | -80.68 | 1.70 | -1.0472 | 1 | CTF |
| 7 | 41.54 | -82.79 | 1.70 | -1.0472 | 1 | CTF |
| 8 | 38.13 | -82.84 | 1.70 | -1.9951 | 1 | CTF |
| 9 | 44.15 | -77.48 | 1.70 | 0.4110 | 1 | CTF |
| 10 | 39.59 | -89.48 | 0.09 | -0.8727 | 1 | CTF |
| 11 | 38.03 | -89.64 | 0.04 | -0.8727 | 1 | CTF |
| 12 | 92.34 | -160.96 | 1.70 | 2.2689 | 0 | CTF |
| 13 | 93.97 | -161.08 | 1.70 | 2.2689 | 0 | CTF |
| 14 | 97.29 | -163.54 | 1.70 | -0.9944 | 0 | CTF |
| 15 | 99.34 | -161.35 | 1.70 | 2.2689 | 0 | CTF |
| 16 | 93.74 | -163.40 | 1.70 | -2.0647 | 0 | CTF |
| 17 | 82.68 | -161.43 | 0.10 | 2.0944 | 0 | CTF |
| 18 | 84.15 | -160.48 | 0.05 | 2.2689 | 0 | CTF |
| 19 | 86.75 | -157.66 | 0.05 | 2.2689 | 0 | CTF |
| 20 | 89.48 | -153.50 | 0.29 | 2.2689 | 0 | CTF |
| 21 | 91.82 | -150.55 | 0.64 | 2.2689 | 0 | CTF |
| 22 | 97.55 | -148.04 | 0.07 | 2.2689 | 0 | CTF |
| 23 | 42.06 | -75.99 | 1.70 | -1.0472 | 1 | CTF |
| 24 | 41.99 | -77.40 | 1.70 | -1.0472 | 1 | CTF |
| 25 | 38.59 | -76.47 | 1.70 | -1.0472 | 1 | CTF |
| 26 | 36.77 | -76.89 | 1.70 | -1.0472 | 1 | CTF |
| 27 | 50.48 | -86.17 | 0.11 | -1.0472 | 1 | CTF |
| 28 | 48.85 | -87.47 | 0.08 | -1.0472 | 1 | CTF |
| 29 | 46.09 | -88.70 | 0.04 | -1.0472 | 1 | CTF |
| 30 | 42.51 | -90.53 | 0.17 | -1.0472 | 1 | CTF |
| 31 | 30.55 | -89.44 | 0.13 | -0.9995 | 1 | CTF |
| 32 | 26.04 | -65.91 | 1.16 | -1.5708 | 0 | race |
| 33 | 29.08 | -65.80 | 0.72 | -1.5708 | 1 | race |
| 34 | 32.59 | -65.86 | 0.37 | -1.5795 | 2 | race |
| 35 | 22.97 | -65.89 | 1.70 | -1.5708 | 3 | race |
| 36 | 42.91 | -67.76 | 0.52 | -2.0853 | 0 | normal |
| 37 | 29.27 | -81.39 | 0.17 | 0.2093 | 0 | normal |
| 38 | 41.10 | -90.28 | 0.18 | 1.7268 | 0 | normal |
| 39 | 52.06 | -82.22 | 0.12 | 2.6765 | 0 | normal |
| 40 | 51.90 | -76.42 | 0.08 | -2.8901 | 0 | normal |
| 41 | 37.43 | -68.11 | 0.26 | -1.3262 | 0 | normal |
| 42 | 29.60 | -76.35 | 0.30 | -0.3412 | 0 | normal |
| 43 | 36.90 | -90.21 | 0.07 | 1.0919 | 0 | normal |
| 44 | 98.66 | -149.65 | 0.08 | -1.9336 | 1 | normal |
| 45 | 92.50 | -149.39 | 0.51 | -1.0553 | 1 | normal |
| 46 | 85.24 | -157.77 | -0.02 | -0.3566 | 1 | normal |
| 47 | 84.90 | -161.71 | 0.11 | 0.3106 | 1 | normal |
| 48 | 92.53 | -169.50 | 0.62 | 0.9801 | 1 | normal |
| 49 | 97.65 | -169.62 | 0.26 | 1.7194 | 1 | normal |
| 50 | 105.03 | -162.21 | 0.02 | 2.6852 | 1 | normal |
| 51 | 105.56 | -157.58 | 0.20 | -2.8188 | 1 | normal |
| 52 | 32.07 | -88.90 | 0.10 | -0.9862 | 1 | CTF |
| 53 | 102.86 | -154.92 | -0.01 | 1.6216 | 0 | race |
| 54 | 100.52 | -174.04 | 0.74 | 1.5161 | 1 | race |
| 55 | 105.29 | -173.66 | 0.16 | 1.6630 | 2 | race |
| 56 | 110.14 | -173.32 | 0.62 | 1.6188 | 3 | race |
| 57 | 49.58 | -66.11 | 0.85 | -1.5625 | 2 | race |
| 58 | 53.67 | -66.00 | 0.94 | -1.6562 | 2 | race |
| 59 | 57.52 | -65.80 | 1.23 | -1.5214 | 2 | race |
| 60 | 61.28 | -65.60 | 1.70 | -1.5111 | 2 | race |
| 61 | 89.49 | -173.46 | 0.27 | 1.6273 | 1 | race |
| 62 | 84.16 | -173.48 | 0.23 | 1.5741 | 1 | race |
| 63 | 79.38 | -173.40 | 0.34 | 1.6273 | 1 | race |
| 64 | 74.32 | -173.37 | 0.72 | 1.5667 | 1 | race |
| 65 | 68.91 | -173.69 | 1.76 | 1.5925 | 1 | race |
| 66 | 41.88 | -81.03 | 1.70 | -1.0102 | 1 | CTF |
| 67 | 38.90 | -81.09 | 1.70 | -2.2928 | 1 | CTF |
| 68 | 36.69 | -80.84 | 1.70 | -2.7915 | 1 | CTF |
| 69 | 38.24 | -74.93 | 1.70 | 1.9123 | 1 | CTF |
| 70 | 42.14 | -75.05 | 1.70 | 1.0308 | 1 | CTF |

### Hang 'Em High in detail

67 player starts: CTF red 19, CTF blue 20, type 13 red 10, blue 18. 92
netgame flags: 2 capture flags, 5 oddball spawns, 11 race
checkpoints, 74 hill flags in 14 hills; no teleporters. 45 netgame
equipment entries, 8 scenery (2 types), no vehicles, 2 decals, 1 sky
(`levels/test/hangemhigh/hangemhigh`), 1 BSP with world bounds x 5.90
.. 35.88, y -22.82 .. 15.82, z -9.25 .. 8.18.

| # | Collection | x | y | z | Game types | Spawn | Period | Flag |
|---|---|---|---|---|---|---|---|---|
| 0 | single weapons/assault rifle | 27.99 | -2.55 | -7.95 | all | 0 | 30 |  |
| 1 | single weapons/shotgun | 20.44 | -19.50 | -8.27 | non-CTF | 0 | 30 |  |
| 2 | single weapons/shotgun | 24.53 | 11.55 | -8.69 | non-CTF | 0 | 30 |  |
| 3 | single weapons/plasma rifle | 16.08 | 3.30 | -3.47 | all | 0 | 30 |  |
| 4 | single weapons/sniper rifle | 20.97 | -1.03 | -4.45 | all | 30 | 30 |  |
| 5 | single weapons/rocket launcher | 15.34 | -10.06 | -5.58 | all | 0 | 120 |  |
| 6 | single weapons/needler | 22.21 | 15.00 | -5.13 | all | 0 | 30 |  |
| 7 | single weapons/needler | 34.95 | 9.97 | -5.13 | all | 0 | 30 |  |
| 8 | powerups/powerup invisibility | 11.11 | -12.59 | -7.95 | all | 0 | 60 | lev |
| 9 | powerups/shield-invisibility | 15.98 | -7.18 | -3.47 | all | 0 | 180 |  |
| 10 | single weapons/pistol | 31.92 | 4.51 | -3.91 | all | 0 | 30 |  |
| 11 | single weapons/frag grenades | 13.90 | 11.20 | -3.36 | all | 0 | 30 |  |
| 12 | single weapons/frag grenades | 14.17 | 11.22 | -3.36 | all | 0 | 30 |  |
| 13 | single weapons/frag grenades | 14.18 | 10.92 | -3.36 | all | 0 | 30 |  |
| 14 | single weapons/frag grenades | 13.90 | 10.91 | -3.36 | all | 0 | 30 |  |
| 15 | single weapons/frag grenades | 31.77 | -17.16 | -2.79 | all | 0 | 30 |  |
| 16 | single weapons/frag grenades | 31.77 | -17.38 | -2.79 | all | 0 | 30 |  |
| 17 | single weapons/frag grenades | 31.57 | -17.37 | -2.79 | all | 0 | 30 |  |
| 18 | single weapons/frag grenades | 31.58 | -17.13 | -2.79 | all | 0 | 30 |  |
| 19 | single weapons/sniper rifle | 20.44 | -19.49 | -8.27 | CTF | 0 | 30 |  |
| 20 | single weapons/sniper rifle | 24.53 | 11.53 | -8.69 | CTF | 0 | 30 |  |
| 21 | single weapons/pistol | 9.94 | -7.30 | -6.43 | all | 0 | 30 |  |
| 22 | single weapons/assault rifle | 12.98 | -2.53 | -7.95 | all | 0 | 30 |  |
| 23 | powerups/powerup health pack | 19.75 | -2.44 | -9.25 | all | 0 | 30 |  |
| 24 | single weapons/shotgun | 32.09 | -3.86 | -5.58 | all | 0 | 30 |  |
| 25 | single weapons/shotgun | 32.13 | -1.38 | -5.58 | all | 0 | 30 |  |
| 26 | single weapons/frag grenades | 9.48 | 1.89 | -6.43 | all | 30 | 30 |  |
| 27 | single weapons/frag grenades | 9.48 | 2.15 | -6.43 | all | 30 | 30 |  |
| 28 | single weapons/frag grenades | 9.72 | 2.14 | -6.43 | all | 30 | 30 |  |
| 29 | single weapons/frag grenades | 9.72 | 1.89 | -6.43 | all | 30 | 30 |  |
| 30 | single weapons/frag grenades | 34.44 | -8.39 | -3.91 | all | 30 | 30 |  |
| 31 | single weapons/frag grenades | 34.21 | -8.39 | -3.91 | all | 30 | 30 |  |
| 32 | single weapons/frag grenades | 34.44 | -8.61 | -3.91 | all | 30 | 30 |  |
| 33 | single weapons/frag grenades | 34.22 | -8.62 | -3.91 | all | 30 | 30 |  |
| 34 | powerups/powerup health pack | 32.11 | -2.63 | -5.58 | all | 0 | 30 |  |
| 35 | single weapons/plasma rifle | 19.60 | -18.51 | -5.58 | all | 0 | 30 |  |
| 36 | single weapons/plasma grenades | 23.51 | 3.15 | -3.47 | all | 30 | 30 |  |
| 37 | single weapons/plasma grenades | 23.28 | 3.17 | -3.47 | all | 30 | 30 |  |
| 38 | single weapons/plasma grenades | 23.28 | 3.37 | -3.47 | all | 30 | 30 |  |
| 39 | single weapons/plasma grenades | 23.48 | 3.35 | -3.47 | all | 30 | 30 |  |
| 40 | single weapons/plasma grenades | 15.16 | -2.50 | -5.58 | all | 30 | 30 |  |
| 41 | single weapons/plasma grenades | 15.17 | -2.29 | -5.58 | all | 30 | 30 |  |
| 42 | single weapons/plasma grenades | 15.39 | -2.29 | -5.58 | all | 30 | 30 |  |
| 43 | single weapons/plasma grenades | 15.39 | -2.50 | -5.58 | all | 30 | 30 |  |
| 44 | single weapons/needler | 29.90 | 15.00 | -5.13 | all | 0 | 30 |  |

| # | x | y | z | Facing | Team | Type |
|---|---|---|---|---|---|---|
| 0 | 27.91 | -15.58 | -2.79 | 1.5904 | 0 | non-CTF |
| 1 | 21.86 | -19.46 | -7.01 | 1.5791 | 1 | non-CTF |
| 2 | 22.92 | 11.58 | -7.01 | -1.5774 | 0 | non-CTF |
| 3 | 31.03 | -2.48 | -9.25 | -3.1174 | 1 | non-CTF |
| 4 | 31.10 | -7.02 | -7.95 | 2.2692 | 1 | non-CTF |
| 5 | 6.89 | 15.08 | -6.43 | -0.8662 | 0 | non-CTF |
| 6 | 18.90 | -19.51 | -7.01 | 1.5710 | 1 | non-CTF |
| 7 | 6.80 | -22.14 | -7.95 | 0.7088 | 1 | non-CTF |
| 8 | 11.08 | -16.75 | -7.95 | 1.5308 | 1 | non-CTF |
| 9 | 34.80 | -15.41 | -3.91 | 3.0506 | 1 | non-CTF |
| 10 | 26.06 | 11.57 | -7.01 | -1.5713 | 0 | non-CTF |
| 11 | 35.26 | 15.29 | -7.95 | -2.4570 | 0 | non-CTF |
| 12 | 6.83 | 9.56 | -6.43 | 0.0000 | 0 | non-CTF |
| 13 | 10.84 | 2.19 | -7.95 | -1.0337 | 0 | non-CTF |
| 14 | 6.79 | 3.06 | -6.43 | 0.0000 | 0 | non-CTF |
| 15 | 28.73 | -17.06 | -2.80 | 2.0936 | 1 | CTF |
| 16 | 29.80 | -17.15 | -2.80 | 3.1308 | 1 | CTF |
| 17 | 32.02 | -17.08 | -2.79 | -0.0846 | 1 | CTF |
| 18 | 31.30 | -15.61 | -1.87 | 2.2820 | 1 | CTF |
| 19 | 32.49 | -12.97 | -3.91 | 1.6598 | 1 | CTF |
| 20 | 34.68 | -14.25 | -3.91 | 1.8135 | 1 | CTF |
| 21 | 27.90 | -13.04 | -3.91 | 2.0222 | 1 | CTF |
| 22 | 26.32 | -16.29 | -3.91 | 2.2191 | 1 | CTF |
| 23 | 13.92 | 10.98 | -3.36 | -1.0855 | 0 | CTF |
| 24 | 14.54 | 9.36 | -3.36 | -1.0892 | 0 | CTF |
| 25 | 16.89 | 11.04 | -4.37 | 0.0000 | 0 | CTF |
| 26 | 15.02 | 10.97 | -4.38 | -1.5643 | 0 | CTF |
| 27 | 15.07 | 8.13 | -4.90 | -1.6957 | 0 | CTF |
| 28 | 12.02 | 8.06 | -5.76 | 1.5878 | 0 | CTF |
| 29 | 13.07 | 10.23 | -6.43 | 1.5562 | 0 | CTF |
| 30 | 13.02 | 9.03 | -3.36 | 0.0000 | 0 | CTF |
| 31 | 32.25 | -13.99 | -1.87 | 3.0931 | 1 | CTF |
| 32 | 28.53 | -14.10 | -2.79 | 2.1587 | 1 | CTF |
| 33 | 30.62 | -11.87 | -3.91 | 1.6138 | 1 | CTF |
| 34 | 26.67 | -14.73 | -3.91 | 1.9496 | 1 | CTF |
| 35 | 26.53 | -17.46 | -3.91 | 1.8132 | 1 | CTF |
| 36 | 32.53 | -11.63 | -3.91 | 1.7947 | 0 | non-CTF |
| 37 | 34.23 | -11.94 | -3.91 | 1.9399 | 0 | non-CTF |
| 38 | 31.10 | -17.13 | -2.79 | -3.1307 | 1 | CTF |
| 39 | 32.04 | -18.26 | -3.91 | -3.1258 | 1 | CTF |
| 40 | 30.57 | -18.34 | -3.91 | -3.0885 | 1 | CTF |
| 41 | 29.03 | -18.32 | -3.91 | -3.1412 | 1 | CTF |
| 42 | 27.39 | -18.26 | -3.91 | 2.5237 | 1 | CTF |
| 43 | 33.81 | -13.12 | -3.91 | 1.9381 | 1 | CTF |
| 44 | 27.87 | -15.56 | -2.79 | 2.1905 | 1 | CTF |
| 45 | 16.96 | 8.11 | -3.36 | -1.5295 | 0 | CTF |
| 46 | 10.97 | 8.80 | -6.43 | -1.5095 | 0 | CTF |
| 47 | 11.02 | 7.24 | -6.43 | -1.5324 | 0 | CTF |
| 48 | 12.04 | 10.35 | -6.43 | 1.5168 | 0 | CTF |
| 49 | 13.20 | 12.99 | -6.43 | 0.0000 | 0 | CTF |
| 50 | 13.22 | 12.05 | -6.43 | 0.0000 | 0 | CTF |
| 51 | 10.32 | 11.10 | -6.43 | -0.6965 | 0 | CTF |
| 52 | 16.05 | 8.08 | -3.36 | -1.6405 | 0 | CTF |
| 53 | 16.11 | 11.01 | -4.38 | 3.1100 | 0 | CTF |
| 54 | 12.82 | 7.16 | -7.95 | 0.0000 | 0 | CTF |
| 55 | 16.14 | 14.13 | -6.43 | -1.5801 | 0 | CTF |
| 56 | 35.16 | 4.97 | -7.95 | 2.2724 | 1 | non-CTF |
| 57 | 34.75 | 3.24 | -3.91 | 3.1081 | 1 | non-CTF |
| 58 | 28.17 | -10.00 | -7.95 | -3.1286 | 1 | non-CTF |
| 59 | 25.21 | -22.25 | -7.95 | 2.2803 | 1 | non-CTF |
| 60 | 25.20 | -16.84 | -7.95 | -3.0803 | 1 | non-CTF |
| 61 | 6.79 | -7.93 | -7.95 | -1.0905 | 1 | non-CTF |
| 62 | 10.69 | -5.99 | -7.95 | 0.0000 | 1 | non-CTF |
| 63 | 6.90 | -6.01 | -6.43 | 0.0000 | 1 | non-CTF |
| 64 | 6.77 | 0.78 | -6.43 | -0.2962 | 1 | non-CTF |
| 65 | 16.72 | 14.68 | -7.95 | 0.0000 | 1 | non-CTF |
| 66 | 17.75 | 10.07 | -7.95 | 0.0000 | 1 | non-CTF |

## Checks run

A script parsed the scenario of all 13 multiplayer maps (and read the
block counts, element contiguity and sample elements of `a10`, `a30`,
`b30`, `c10`, `d20` and `ui`). All of these held:

- Every block's address and count lie inside the tag data; every
  fixed block element size reproduced the gap between consecutive
  blocks wherever blocks have no sub-blocks.
- Every palette index of scenery, vehicle, machine, light fixture and
  sound scenery placements is in range (0 errors in 343 placements),
  and every placement's name index is -1.
- Every referenced tag exists in the tag index with the expected
  class: palette entries (class equals the palette's, 100 percent),
  sky references (`sky `), netgame equipment and starting equipment
  (`itmc`), item collection permutations (`weap` or `eqip`).
- Every player start, netgame equipment entry and netgame flag lies
  inside the BSP's world bounds (`structure_bsp.md`, 0x0C8) on every
  map (0 outside out of 829 starts, 535 equipment entries and 1164
  flags); scenery, vehicles, machines, light fixtures and sound
  scenery all lie inside the bounds grown by 0.5 WU.
- Every heading is within [-pi, pi]; every team index is 0 to 3 (flags
  0 to 29); every start's game type is one of 1, 2, 3, 4, 5, 12, 13,
  14; every flag type is 0 to 8; BSP index 0 everywhere.
- Every unused range of the main struct is zero on all 19 maps, and
  so are the unused bytes of every element read.
- Every teleporter source has exactly one target with the same team
  index.

Not checked on the disc: the weapon, biped, control, equipment
placement layouts and the starting profile's meaning of the grenade
counts (no multiplayer instance); the function and recorded-animation
elements; the yaw-pitch-roll convention against rendered geometry (the
evidence above is indirect); and every spawn and respawn rule, which
is reference behaviour.

## Quirks

- **Netgame equipment's facing is stored but ignored** by this build;
  items spawn with the default orientation. Keep the number.
- **Item respawn is on the global clock**, not a delay after pickup:
  an entry with a 120 s period creates an item at 0, 120, 240 s ...
  whether or not the previous one was taken (reference).
- **Hill flags past the twelfth are ignored** for a hill (reference):
  Blood Gulch's big hills are cut.
- **Every start carries the BSP index 0**; for a one-BSP map the field
  is inert.
- **The disc's vehicle placements have no multiplayer data**; the PC
  game's per-game-type vehicle flags do not exist here.
- **Pitch and roll tilt about world axes**, not body axes (above).
- **On-BSP flag 0 means do not create** a scenery or light fixture
  placement; it is how buried decoration is left out.
- **Light fixtures place colour 0**: do not read it as an unlit light.

## Test cases

### (a) Synthetic scenario (unit tests)

A hand-built tag, the tag data being a main struct of 0x5B0 bytes with
every field not named below zero (blocks empty: count 0, address 0).
Addresses are chosen by the test; the parsed values are what a reader
must produce.

| Field | Bytes / value |
|---|---|
| 0x03C type | 1 |
| 0x030 skies block | count 1; element: class `sky `, tag id of a tag with path `levels/test/syn/syn sky` |
| 0x354 player starts | count 2 (element 52 each) |
| start 0 | position (1.0, 2.0, 0.5), facing 0.0, team 0, BSP 0, game types (1, 0, 0, 0) |
| start 1 | position (-3.0, 4.0, 0.5), facing 1.5707964, team 1, BSP 0, game types (13, 0, 0, 0) |
| 0x384 netgame equipment | count 1 (element 144) |
| equipment 0 | flags 1, game types (12, 0, 0, 0), team 0, spawn time 30, position (5.0, 6.0, 1.0), facing 0.0, item collection `item collections/test/rifle` (class `itmc`) |
| 0x21C scenery palette | count 1: tag reference class `scen`, path `scenery/test/rock/rock` |
| 0x210 scenery | count 1 (element 72): palette 0, name -1, flags 0, variant 0, position (10.0, -20.0, 0.5), rotation (1.5707964, 0.0, 0.0), on-BSP flags 1 |

Expected parsed values:

| Query | Expected |
|---|---|
| scenario type | multiplayer |
| sky count; sky 0 path | 1; `levels/test/syn/syn sky` |
| player start 0 forward vector | (1.0, 0.0, 0.0) |
| player start 1 forward vector | (cos 1.5707964, sin 1.5707964, 0) = (-0.0000, 1.0000, 0) within 1e-6 |
| eligible starts for CTF (game type 1) | start 0 only (start 1's 13 means every game but CTF) |
| eligible starts for slayer (2) | start 1 only |
| eligible starts for race (5) | start 1 only |
| netgame equipment 0 period for slayer | 30 s = 900 ticks (spawn time 30) |
| netgame equipment 0 levitate | true |
| scenery 0 palette path | `scenery/test/rock/rock` |
| scenery 0 axes | F = (0, 1, 0), L = (-1, 0, 0), U = (0, 0, 1) within 1e-6 |
| scenery 0 created | yes (on-BSP bit 0 set, palette 0 in range) |
| a copy of scenery 0 with palette index 1 | skipped (index out of range) |
| a copy of scenery 0 with on-BSP flags 0 | skipped |

Eligibility table (rows: slot value; columns: game types 1 CTF, 2 slayer,
3 oddball, 4 king, 5 race):

| Slot | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| 0 | no | no | no | no | no |
| 1 | yes | no | no | no | no |
| 5 | no | no | no | no | yes |
| 12 | yes | yes | yes | yes | yes |
| 13 | no | yes | yes | yes | yes |
| 14 | no | yes | yes | yes | no |

Rotation worked examples (tolerance 1e-4): the four rows of the table
in "Rotation and facing". Heading example: facing 1.328281 gives
forward (0.2401, 0.9707, 0).

Teleporter example: source A (team 0) at (0, 0, 0) facing 0; source B
(team 1) at (10.8, 0, 0) facing 0; target T0 (team 0) at (10, 0, 0)
facing 1.5707964; target T1 (team 1) at (50, 0, 0) facing 0. A player
at (0.6, 0, 0) is not teleported (outside 0.5 WU). A player at (0.3, 0,
0) with heading 0.5 is teleported to (10, 0, 0) with heading 0.5 +
1.5707964 - 0 = 2.0707964. It arrives 0.8 WU from B, which is within 1
WU, so B counts as the flag it arrived at: standing at (10.5, 0, 0)
(0.3 WU from B) it is not teleported on. After it has gone to (9.5, 0,
0) (1.3 WU from B, which re-arms B) and comes back to (10.6, 0, 0)
(0.2 WU from B) it is teleported to (50, 0, 0) with its heading
unchanged (B and T1 both face 0).

Item collection drawing: permutations with weights 70, 90, 15 (total
175): r in 0..69 gives the first, 70..159 the second, 160..174 the
third. Weights 50, 50: r = 49 first, r = 50 second. A single weight
100: always the one item.

Spawn timing: an entry with spawn time 0 whose collection has spawn
time 60 (period 1800 ticks) creates items at ticks 0, 1800, 3600; with
spawn time 45 on the entry: ticks 0, 1350, 2700; with both 0: every 900.

Sky light direction: yaw 10 degrees, pitch 40 degrees gives (0.7544,
0.1330, 0.6428); the flare is placed at camera + 1023.875 x that.

Fog: start 3, opaque 100, density 0.4: a point at depth 3 or less has
factor 0; at depth 51.5 it is 0.2 (half-way, times 0.4); at 100 or
more 0.4.

### (b) Disc-backed expectations (slow tests)

`bloodgulch`:

| Check | Expected |
|---|---|
| scenario type | 1 |
| sky references | 1, class `sky `, path `levels/test/bloodgulch/bloodgulch`; its model `sky/mp clear afternoon/mp clear afternoon` |
| sky fog (outdoor) | colour (1.0, 0.902, 0.769), density 0.4, start 3, opaque 100 (to the stored f32 precision) |
| sky light 0 | direction yaw 0.1745 pitch 0.6981, colour (0.906, 0.894, 0.796), power 1.8, lens flare tag `sky/clear afternoon/sun` |
| player starts | 71; CTF red 17, CTF blue 21, race 17, type 14: 8 + 8 |
| first player start | (98.4934, -157.6390, 1.7047), facing 1.328281, team 0, BSP 0, game types (1, 0, 0, 0) |
| netgame flags | 191; capture flags 2, teleporter sources 2, targets 2, oddball 9, race track 5, race vehicle 16, hill 155 |
| first netgame flag | type 0, team 1, (40.2406, -79.1232, -0.1), facing 0 |
| netgame equipment | 35 |
| first netgame equipment | `item collections/single weapons/shotgun`, position (38.4336, -79.1941, -0.2860), game types (12, 0, 0, 0), spawn time 0, flags 0, facing 1.5384 |
| starting equipment | 2 entries: ((1), flags 0, assault rifle, pistol) and ((13), flags 1, plasma pistol) |
| vehicles | 8: 2 warthog, 4 ghost_mp, 2 scorpion_mp; first at (29.137, -90.834, 0.2864), yaw -0.8483 |
| scenery | 50 placements, 19 palette entries; first: boulder, position (37.9339, -88.6921, 0.0876), rotation (2.3610, 0.1120, 0.1017), on-BSP 1 |
| other | machines 0, light fixtures 0, sound scenery 2, decals 8, BSPs 1 |
| bounds | every start and equipment position inside the BSP's world bounds |

`hangemhigh`: 67 player starts (CTF red 19, blue 20, type 13: 10 and
18), 92 netgame flags (capture flags 2, oddball 5, race track 11, hill
74 in 14 hills, no teleporters), 45 netgame equipment, 8 scenery
placements of 2 types, no vehicles, 2 decals, sky
`levels/test/hangemhigh/hangemhigh`, 1 BSP. First player start
(27.9078, -15.5764, -2.7938), facing 1.590419, team 0, game types (13,
0, 0, 0). First netgame equipment entry:
`item collections/single weapons/assault rifle`, position (27.9907,
-2.5534, -7.9494), game types (12, 0, 0, 0), spawn time 0, flags 0,
facing 1.5536. First netgame flag: type 3 (race track), team 0, (20.2444,
12.5342, -7.9494), facing 0. First scenery:
`scenery/floor_arrow/floor_arrow` at (31.9421, -12.9793, -3.8979), yaw
-1.5708, on-BSP 1.

All 13 maps: the counts in "Measured facts", the checks in "Checks
run", and every teleporter source having one target.

## Confidence per section

| Section | Level |
|---|---|
| Top-level layout, offsets, sizes of the blocks the multiplayer maps fill | checked against the disc (all 13 maps) |
| Top-level layout, blocks empty on the multiplayer maps | element sizes of non-empty campaign blocks checked on 5 campaign maps and `ui`; the rest reference |
| Placement header, scenery, vehicle, machine, light fixture, sound scenery | checked against the disc |
| Weapon, biped, equipment, control placements, device groups, starting profiles | reference (profiles also campaign-checked for size) |
| Game type numbers and eligibility | numbers checked on the disc; wildcard meanings and the matching rule reference |
| Player start fields, netgame flag fields, netgame equipment fields, item collections | checked against the disc |
| Netgame flag type meanings, teleporter matching, hills, spawn period behaviour | reference; teleporter pairing consistency checked on the disc |
| Rotation convention | reference, indirectly supported by the disc (Wizard's corner lights) |
| Sky tag layout (0x00 to 0xD0) | checked against the disc (all 10 sky tags) |
| Sky light flag names, fields the engine does not read | inferred |
| How the sky is drawn, fog, lens flare placement | reference |
| Scenery tag object header offsets | checked against the disc (five maps' object tags) |
| Decals, detail objects, trigger volumes, blocks to step over | partly campaign-checked; the rest reference |
| Measured facts, checks | checked against the disc |

## Open questions

- **Rotation convention.** The composition above comes from the
  engine's own matrix routine; the disc agrees only indirectly. A test
  against rendered geometry (a boulder with yaw and tilt standing in the
  right way on a slope in Blood Gulch, or the tilted 'teleporter_base'
  number 24) would settle it.
- **Meaning of netgame flag type 1 and 5**, and whether the race
  vehicle flags' team index (1 on Blood Gulch, 0 on Sidewinder) means
  anything. The disc never uses types 1 and 5.
- **Decal placement bytes**: the scale of the two signed bytes after the
  palette index, and whether any of these decals are drawn.
- **Item respawn on the PAL reference versus the NTSC disc**: the spawn
  clock and "last owned" arithmetic come from the other build.
  `multiplayer_rules.md` should measure respawn times in a running game
  if one becomes available.
- **Light fixture colour 0**: where the light's colour comes from for
  placements that store 0 (probably the fixture's own tag, `objects.md`).
- **Sky ambient radiosity values and sun colour/power** are never read
  by the reference; they may matter for dynamic objects in the Xbox
  renderer through the lightmap compiler's output only.
- **Weapon placement field order** (rounds total versus loaded) has no
  disc instance.
- **Functions block element size** (count 0 on all 19 maps).
