# Counter-Strike 1.6: the HUD

Source basis: the install's HUD data files read in place (2026-11-25):
the HUD sprite list and the per-weapon sprite lists, the HUD sprite
atlases (`640hud*.spr`, `320hud*.spr`, the pain sprite) and the client's
shipped `config.cfg`; Half-Life's SDK client HUD (health, armour, ammo,
weapon menu, death notices, damage direction; Valve's licence, read in
this spec session only as reference, nothing reproduced). Counter-Strike
1.6's own client library is closed and was not run on this box (it needs
Steam and 32-bit libraries), so what CS 1.6 does differently from
Half-Life's client, and everything CS-only (money, timer, the dynamic
crosshair), is marked "(ref.)": our best reading, to confirm by captures.
The crosshair, the round timer, the weapon menu and the 320 layout were
then worked out (second session, 2026-11-25) from a community
reimplementation of the CS 1.6 client library (the open-source
"cs16-client" of the Xash3D project, GPL; read only as reference,
nothing reproduced) beside Half-Life's SDK client, and checked against
the install's sprite lists and shipped `config.cfg`. Where that
reimplementation has since changed a behaviour on purpose (the timer's
centring), the older behaviour, which matches the original client, is
the one given here.
Status: draft

## Summary

The HUD is a handful of glowing numbers and icons drawn from grey sprite
atlases tinted with one yellow-orange colour and added onto the frame
(additive), at their own pixel size: health with a cross at the bottom
left, armour with a suit icon beside it, ammunition (clip, a bar,
reserve, the ammunition icon) at the bottom right, the round timer at
the bottom centre, money above the ammunition. Numbers sit dim and
flare up when they change, then fade back over about five seconds. The
kill feed lists the last kills at the top right with each weapon's
`d_` icon; red arcs around the screen centre show where damage came
from. The crosshair is four green lines whose gap opens with movement
and firing.

## Units and conventions

- Screen pixels, origin at the top left, y down. `W`, `H`: the screen's
  width and height.
- Sprite rectangles are `x y width height` in the named atlas
  (`sprites/<file>.spr`, frame 0), from the lists in `sprites/hud.txt`
  (shared) and `sprites/weapon_<id>.txt` (one per weapon); format in
  specs/goldsrc/spr.md, "The HUD's sprite lists".
- Integer division truncates (`25 / 2` = 12).
- "Alpha" `a` (0-255) on an additive sprite or number means its tint is
  scaled by `a / 255` before it is added; on a filled rectangle it is the
  ordinary blend alpha.

## Constants

| Name (ours) | Value | Meaning |
|---|---|---|
| HUD colour | 255 160 0 | every number and icon's tint ("yellowish") |
| warning colour | 250 0 0 | health and its cross at 25 or less, the pain arcs then |
| empty colour | 255 16 16 | weapons without ammunition in the weapon menu ("reddish") |
| bar colour | 0 160 0 | the weapon menu's ammunition bars, filled part ("greenish") |
| dim alpha | 100 | a number at rest |
| fade time | 100 | fade units; they drain at 20 a second (5 s) |
| flare | 128 | alpha added to the dim alpha at the start of a fade (health, armour) |
| ammo flare | 200 | the ammunition's fade value on each update |
| font height | 25 | the `number_0` cell's height at 640 (16 at 320) |
| digit width | 20 | the `number_0` cell's width at 640 (12 at 320) |
| reddish | 255 16 16 | the round timer's warning flash (same as the empty colour) |
| layout switch | 640 | screens 640 wide or more use the 640 entries; narrower the 320 ones |

## Sprites (640 layout)

All HUD atlases are additive (texture format 1) and grey: the drawing
colour is the tint times the grey level. From `sprites/hud.txt` (215
entries; 640 ones listed):

| Entry | Atlas | Rectangle | Use |
|---|---|---|---|
| `number_0`..`number_9` | `640hud7` | x 0, 23, 47, 70, 95, 119, 144, 169, 192, 216; y 0; 20 × 25 | digits |
| `divider` | `640hud7` | 240 0 2 40 | (unused by the parts here, ref.) |
| `cross` | `640hud7` | 48 25 24 24 | health |
| `suit_empty`, `suit_full` | `640hud7` | 24 25 24 24; 0 25 24 24 | armour (vest only) |
| `suithelmet_empty`, `suithelmet_full` | `640hud7` | 24 124 24 24; 0 124 24 24 | armour with helmet |
| `dollar`, `plus`, `minus` | `640hud7` | 192 25 18 25; 223 25 17 23; 210 25 13 23 | money and its change |
| `stopwatch` | `640hud7` | 144 72 24 24 | round timer |
| `bucket1`..`bucket5`, `bucket0` | `640hud7` | 168 72, 188 72, 208 72, 168 92, 188 92, 208 92; 20 × 20 | weapon menu slot numbers 1-6 |
| `selection` | `640hud3` | 0 180 170 45 | weapon menu: the highlight over the chosen weapon |
| `d_<weapon>` | `640hud1`, `640hud2`, `640hud16` | 32-57 × 16 (table below) | kill feed |
| `d_headshot`, `d_skull`, `d_grenade` | `640hud1` | 0 240 36 16; 224 240 32 16; 224 192 32 16 | headshot mark, no weapon, the HE grenade |
| `c4`, `defuser`, `buyzone`, `rescue`, `escape`, `vipsafety`, `bombticking` | `640hud7` | 32 × 32 | objective status icons (not on the party's path) |

The pain arcs are `sprites/640_pain.spr` (in `valve/`, the fallback
folder; `320_pain.spr` below 640): four frames, red on black, additive:
0 front 128 × 48, 1 right 48 × 128, 2 behind 128 × 48, 3 left 48 × 128.

### Per weapon

Each `sprites/weapon_<id>.txt` names (640 entries): `weapon` (the
menu's picture, 170 × 45), `weapon_s` (the same weapon lit, for the
chosen one), `ammo` (24 × 24 in `640hud7`), `crosshair` and `autoaim`
(`crosshairs`, unused by CS 1.6's drawn crosshair, ref.), and for the
scoped guns `zoom`/`zoom_autoaim` (the scope, `sniper_scope`; the XM1014
names `ch_sniper`, unused). The ammunition icons by type (rectangle in
`640hud7`, 24 × 24):

| Icon at | Ammunition | Weapons |
|---|---|---|
| 0 72 | buckshot | m3, xm1014 (and the knife's list, which has no ammunition) |
| 24 72 | .50 AE | deagle |
| 48 72 | 9 mm | glock18, elite, mp5navy, tmp |
| 72 72 | 7.62 NATO | ak47, scout, g3sg1 |
| 96 72 | .45 ACP | usp, mac10, ump45 |
| 120 72 | .357 SIG | p228 |
| 0 96 | 5.56 NATO | m4a1, famas, galil, aug, sg552, sg550, m249 |
| 24 96 | .338 Magnum | awp |
| 120 96 | 5.7 mm | fiveseven, p90 |
| 48 96, 72 96, 144 96 | the grenade itself | flashbang, hegrenade, smokegrenade |
| 96 96 | C4 | c4 |

Kill feed icons (`d_<id>`, 640): `knife` `640hud1` 192 0 48 16; `ak47`
192 80 48 16; `awp` 192 128 48 16; `deagle` 224 16 32 16; `famas`
`640hud2` 192 144 48 16; `fiveseven` `640hud16` 192 0 32 16; `flashbang`
`640hud1` 192 192 48 16; `g3sg1` 192 144 48 16; `galil` `640hud2` 192
160 48 16; `glock18` `640hud1` 192 16 32 16; `m249` 192 160 48 16; `m3`
192 48 48 16; `m4a1` 192 96 48 16; `mp5navy` 192 64 32 16; `p228` 224 32
32 16; `p90` 192 176 48 16; `scout` 192 208 48 16; `sg550` `640hud16`
192 48 48 16; `sg552` `640hud1` 192 112 48 16; `ump45` `640hud16` 192 80
48 16; `usp` `640hud1` 192 32 32 16; `tmp` 224 64 32 16; `xm1014` 192 224
48 16; `aug` 148 240 44 16; `mac10` 109 240 34 16; `elite` 52 240 57 16.
The HE grenade's kill names `grenade` (`d_grenade`); the smoke grenade
has no icon of its own. A weapon whose icon is missing shows `d_skull`.

## The 320 layout

Below 640 wide the client reads the 320 entries of the same lists (a
layout per screen; nothing is mixed except where a 320 entry itself
names a 640 atlas). Every position in Behavior is derived from the
sprites' own sizes, so the same formulas give the 320 layout: the font
height is the `number_0` cell's height and the digit width its width,
both read from the list, and icon sizes are their rectangles'.

| Entry (320) | Atlas | Size | Note |
|---|---|---|---|
| `number_0`..`number_9` | `320hud2` | 12 × 16 | x 0, 12, ... 108; font height 16, digit width 12 |
| `cross`, `suit_full`, `suit_empty`, `stopwatch` | `320hud2` | 16 × 16 | |
| `bucket1`..`bucket5`, `bucket0` | `320hud2` | 12 × 12 | x 108, y 16, 28, ... 76 |
| `selection`, `weapon`, `weapon_s` | `320hud1` | 80 × 20 | |
| `ammo` (per weapon) | `640hud7` | 24 × 24 | the 320 entries name the 640 icons |
| `radar` | `radar320` | 64 × 64 | |

The pain arcs come from `320_pain.spr`. The fixed numbers in the
formulas (the 5 pixel spacings, the 10 pixel menu margins, the colon's
2 × 2 squares, the crosshair) do not change with the layout; the
weapon menu's ammunition bar is the one exception (10 × 2 below 640,
20 × 4 from 640).

At 320 × 240: health baseline y0 = 240 − 16 − 8 = 216; the cross at (8,
216); the number's places from x 16 + 6 = 22 (digits at 22, 34, 46,
ending 58) at y 216 + 3 = 219; the bar at x 64, 1 × 16 (digit width /
10 = 1). Armour icon at x 64 (W / 5), y 216 − 2 = 214; its number from
x 80. Ammunition (icon width 24): a magazine weapon's clip from 320 −
96 − 24 = 200, its bar at 242, the reserve from 242 + 2 + 6 = 250. The
timer's row at 240 − 24 = 216.

## Behavior

### Numbers

A number is drawn digit by digit from the `number_<d>` sprites, left
to right, each digit one digit width further. With "three places" the
number always takes three digit widths: places in front of a shorter
number are left empty (no leading zeros). Zero is drawn as one `0` in
the last place. Health, armour and both ammunition counts use three
places.

### Health

- Baseline `y0 = H − font height − font height / 2` (H − 37 at 640).
- The cross at (cross width / 2, y0): (12, H − 37), 24 × 24.
- The number in three places from x = cross width + digit width / 2
  (34), at y0 + 0.2 × font height truncated (H − 32): it ends at x 94.
- A bar after it: x = 94 + digit width / 2 (104), at y H − 32, one tenth
  of a digit width wide (2) and one font height tall (25), filled in the
  HUD colour 255 160 0 at the number's alpha (ref.: CS 1.6 shows the
  same bar between health and armour).
- Colour: the HUD colour above 25 health, the warning colour at 25 or
  less (cross and number).
- Alpha: when the health value changes the fade restarts at the fade
  time (100). While the fade is above zero the alpha is dim + fade / 100
  × 128 (228 at the change), and the fade drains by 20 a second; at zero
  the alpha is dim (100). At 15 health or less the alpha is 255 whatever
  the fade.
- Hidden while dead or spectating, and in the party while no CS 1.6
  weapon is in hand (ours).

### Armour

- The icon at x = W / 5 (128 at 640; ref.: Half-Life's later client
  places it at three icon widths instead; CS 1.6's client is older), y
  = y0 − icon height / 6 (H − 41).
- The empty icon is drawn whole; the full icon over it, cropped from the
  top so that the visible part is the bottom (armour / 100) of it
  (armour above 100 counts as 100): 50 armour shows the lower 12 rows
  lit.
- The helmet pair (`suithelmet_*`) replaces the vest pair when the
  player has a helmet (ref.).
- The number in three places at x = icon x + icon width (152), y = H −
  32. Zero is drawn (`  0`).
- Colour: always the HUD colour. Alpha: as health's (dim 100, 228 on a
  change, fading over 5 s), without the low-health rule.

### Ammunition

- Shown when the weapon in hand has an ammunition type (not the knife).
- Row y = H − 32. Icon width = the weapon's `ammo` rectangle width (24).
- A weapon with a magazine: the clip in three places from x = W − 8 ×
  digit width − icon width (W − 184; 456 at 640), ending at W − 124; a
  bar at x + digit width / 2 (W − 114), digit width / 10 wide (2), one
  font height tall, in the HUD colour at the ammunition alpha; the
  reserve in three places from bar + 2 + digit width / 2 (W − 102) to W
  − 42.
- A weapon without a magazine (the grenades: they count what is
  carried): the count in three places from W − 4 × digit width − icon
  width (W − 104) to W − 44.
- The icon right after the last number (x W − 42, or W − 44), at y H −
  32 − icon height / 8 (H − 35), tinted like the numbers.
- Alpha: the ammunition fade is set to 200 whenever the weapon in hand
  or its clip changes (every shot, a reload, a switch), drains by 20 a
  second, and the alpha is the larger of 100 and the fade: a shot shows
  the numbers at 200, dimming to 100 over 5 s.
- Both counts draw zero (`  0 |   0`).
- Colour: the HUD colour always (ref.: no red at an empty clip).

### Money (unused in the party)

CS-only (ref.): the `dollar` sign and the amount (up to five places, no
leading zeros) at the right, one row (about 1.25 font heights) above the
ammunition, the sign's left edge about W − 6.5 digit widths. When money
changes, the difference is shown briefly beside it with `plus` (green)
or `minus` (red) and the amount flashes in that colour, then returns to
the HUD colour and fades to dim. The party has no money: not drawn.

### Round timer

CS-only. `fh` is the font height, `dw` the digit width, `sw` the
stopwatch's width (24 at 640, 16 at 320).

- Time left `t` = the round's length minus the time since the server
  last sent it, truncated to whole seconds, at least 0. Minutes `m` =
  t / 60, seconds `s` = t − 60 m. A round of 120 s shows `1:59` from
  its first frame (truncation), `0:00` once it runs out.
- Row y = H − 1.5 × fh, truncated (442 at 480 high; 216 at 240).
- The `stopwatch` at x = W / 2 (its left edge, not the group, sits on
  the centre: the timer reads a little right of centre).
- Minutes in two places from x = W / 2 + sw / 4, right-aligned, no
  leading zero (`0` is drawn as one digit in the second place). The
  minutes take exactly two digit widths, whatever they show; a third
  digit (100 minutes or more) is drawn one place further left.
- The colon: two 2 × 2 squares at x = (end of the minutes' two places)
  + sw / 4, at y + fh / 4 and y + fh − fh / 4 (640: y + 6 and y + 19;
  320: y + 4 and y + 12). They are filled additively in the timer's
  colour at alpha 100 (so they are dimmer than the digits).
- Seconds in two places with a leading zero (`05`), from x = (end of
  the minutes' places) + sw / 2.
- Colour: the HUD colour scaled by 100/255 (dim alpha 100), always:
  the timer never flares.
- Warning: while t is 20 or less the colour flashes between the
  reddish 255 16 16 and the HUD colour (both scaled by 100/255). A
  timer adds the frame time each frame; when it exceeds s / 40 + 0.1
  seconds it resets and the colour flips, so a phase lasts 0.6 s at 20
  s left and speeds up to 0.1 s at 0. The phase is kept between rounds;
  the first warning phase after the client starts is the reddish one.
  Above 20 s: the HUD colour.
- Shown from the first round-time message the server sends (each round
  start, freeze time included, and on joining) and never hidden by
  running out (it stays at `0:00`). Hidden while the server hides the
  timer (or the whole HUD), while the player has no suit (dead), and
  while spectating someone in first person.

At 640 × 480 with `2:45`: stopwatch at (320, 442); `2` at x 346 (the
places are 326 and 346); colon squares at x 372, y 448 and 461;
seconds `4` at 378, `5` at 398. In the party it shows the minigame's
round time left when the minigame has one, else it is hidden.

### Crosshair

CS 1.6 draws its crosshair itself (the weapon lists' `crosshair`
sprite is not what is seen): four lines, left, right, above and below
the screen centre, each one pixel thick at every resolution, leaving a
gap at the centre. Everything below runs once per drawn frame (frame
rate dependent, see Quirks).

**Console variables** (registered defaults; the shipped `config.cfg`
has the same values):

| Variable | Default | Meaning |
|---|---|---|
| `cl_crosshair_color` | "50 250 50" | three integers red green blue, each clamped to 0-255 |
| `cl_crosshair_translucent` | 1 | 1: the lines are added onto the frame, colour × alpha / 255; 0: blended normally at that alpha. Any other value prints its usage and is reset to 1 |
| `cl_crosshair_size` | "auto" | "auto", "small", "medium", "large" or 0-3 (0 auto, 1 small, 2 medium, 3 large); anything else prints its usage and keeps the previous size |
| `cl_dynamiccrosshair` | 1 | 0: movement no longer widens the gap (firing still does) |

Under night vision the colour is 250 50 50 and the lines are blended,
whatever the variables say.

**Per-weapon gap** (pixels at the 640 scale). Each weapon has a base
gap `b` and a step `k` it opens by per shot:

| Weapon | b | k | Movement rule |
|---|---|---|---|
| p228 | 8 | 3 | air, speed (any), duck |
| glock18 | 8 | 3 | air, speed (any), duck; × 1.4 in semi-automatic (not in burst) |
| usp | 8 | 3 | air, speed (any), duck; × 1.4 without the silencer |
| deagle | 8 | 3 | air, speed (any), duck |
| fiveseven | 8 | 3 | air, speed (any), duck |
| elite | 4 | 3 | none |
| m3 | 8 | 6 | none |
| xm1014 | 9 | 4 | none |
| mac10 | 9 | 3 | air |
| tmp | 7 | 3 | air |
| mp5navy | 6 | 3 | air |
| ump45 | 6 | 3 | air |
| p90 | 7 | 3 | air, speed (170) |
| galil | 4 | 3 | air, speed (140) |
| famas | 4 | 3 | air, speed (140); × 1.4 in automatic (not in burst) |
| ak47 | 4 | 4 | air, speed (140) |
| m4a1 | 4 | 3 | air, speed (140); × 1.4 always (Quirks) |
| sg552 | 5 | 3 | air, speed (140) |
| aug | 3 | 3 | air, speed (140) |
| m249 | 6 | 3 | air, speed (140) |
| knife | 7 | 3 | none |
| hegrenade, flashbang, smokegrenade | 8 | 3 | none |
| c4 | 6 | 3 | none |
| scout, awp, g3sg1, sg550 | (5 3, 8 3, 6 4, 5 3) | | no crosshair at all, scoped or not |

A weapon outside the table: b 4, k 3, no movement rule.

**This frame's base** `d`, an integer, starts at b. With
`cl_dynamiccrosshair` 1 and a weapon that has a movement rule, then in
this order, each product truncated to an integer:

1. In the air (not on the ground) with the "air" rule: d × 2.
2. Otherwise (on the ground, or no "air" rule): if ducked and the rule
   has "duck": d × 0.5; else if the rule has "speed (v)" and the
   player's speed (the length of the whole velocity, vertical included)
   is v or more: d × 1.5. "Speed (any)" means v = 0: the pistols widen
   by 1.5 whenever they are on the ground and not ducked, standing still
   included (Quirks).
3. The weapon's × 1.4 when its state calls for it.

**Gap state** `g` (fractional, kept between frames, per client; starts
at 0):

1. If the player fired since the last frame (one or more shots: each
   bullet of a burst counts; the knife and the grenades never do): g = min(g + k, 15),
   and the crosshair alpha `a` = max(a − 40, 120). Several shots in one
   frame open it by one step only.
2. Otherwise: g = g − (0.013 g + 0.1), and a = a + 2.
3. g = max(g, d); a = min(a, 255). (a starts at 200.)
4. Line length `L` = (g − d) × 0.5 + 5, truncated: 5 at rest; it grows
   only while the gap is above this frame's base (after shots, and
   while a larger base shrinks back, e.g. on landing).

So movement changes the gap at once (the base jumps), firing opens it
by k per shot up to 15 (a base above 15 stays), and it closes at about
1.3 % plus 0.1 pixel per frame: from 15 to an AK-47's 4 takes 51
frames (0.51 s at 100 fps, 0.85 s at 60).

**Scale** by `cl_crosshair_size` and the screen width W, as a scale
base S:

| Size | S |
|---|---|
| auto, W ≤ 640 | W (no scaling) |
| auto, 640 < W < 1024 | 800 |
| auto, W ≥ 1024 | 640 |
| small | 1024 |
| medium | 800 |
| large | 640 |

When W ≠ S the drawn gap G = W × g / S (fractional) and the drawn
length = W × L / S truncated; otherwise G = g. Thickness stays one
pixel. Rest lengths with "auto": 5 up to 1023 wide, 8 at 1024, 10 at
1280, 15 at 1920.

**Placement.** With the centre (cx, cy) = (W / 2, H / 2) in integers,
each start truncated to a pixel:

- left: from x = cx − G − length + 1, row cy, length × 1;
- right: from x = cx + G, row cy;
- top: column cx, from y = cy − G − length + 1, 1 × length;
- bottom: column cx, from y = cy + G.

The left line's inner pixel is at cx − G and the right line's at cx + G
(likewise vertically), in colour `cl_crosshair_color` at alpha a.

**Hidden** when the field of view is 40 or less (the scoped zoom
steps; the AUG's and SG552's 55 keep it), for the four sniper rifles
always, while a shield is raised, while the HUD's crosshair is hidden
by the server, and while spectating (except in first person, where the
watched player's weapon is used).

### Weapon menu (bucket list)

Fast switching (`hud_fastswitch`, default 0, also 0 in the shipped
config): with it on, a slot key whose slot holds exactly one weapon
switches to it at once; a slot holding several opens the menu as below
(ref.: the original client may cycle instead; see Open questions).

A slot key (or the next/previous weapon command) opens the menu on
that slot's first weapon; the same slot's key again moves to the next
weapon in it, wrapping to the first. A slot with no weapons opens the
menu with no slot open (all columns narrow). Sounds: `wpn_hudon` on
opening or changing slot, `wpn_moveselect` on moving within a slot,
`wpn_select` on confirming, `wpn_hudoff` on cancelling (all in
`sound/common/`).

**Closing.** There is no timeout: the menu stays until the attack
button confirms the highlighted weapon (the click is swallowed, no
shot), the cancel command (`cancelselect`, bound to Escape) closes it,
the player dies, the server hides the weapons HUD, or the round resets.

**Layout** (`bw` × `bh` the bucket sprite, 20 × 20 at 640, 12 × 12 at
320; `pw` × `ph` the open slot's first weapon picture, 170 × 45 at 640,
80 × 20 at 320; `rw` the radar sprite's width, 128 at 640, 64 at 320):

- Five slots, numbered `bucket1`..`bucket5` (`bucket0` is not drawn).
- Columns start at x = rw + 10 (138 at 640, 74 at 320: right of the
  radar's place, whether the radar shows or not). Each column is bw + 5
  wide, the open slot's pw + 5.
- Top row at y 10: each slot's number sprite at its column's x, additive
  in the HUD colour at full tint.
- Under it, from y = bh + 10 (30 at 640, 22 at 320; directly below the
  number):
  - a closed slot: one bw × bh filled box per weapon it holds, top to
    bottom, bh + 5 apart, additive in the HUD colour at alpha 128, or in
    the reddish colour at alpha 96 when that weapon has no ammunition
    (neither in the clip nor in reserve; a weapon using none always
    counts as having some);
  - the open slot: each weapon's picture, ph + 5 apart. The highlighted
    one is its `weapon_s` with `selection` drawn over it at the same
    place; the others their `weapon`. All of them, the highlighted one
    included, in the HUD colour scaled by 192/255, or the reddish colour
    scaled by 128/255 without ammunition.
- An ammunition bar on each open-slot picture whose weapon has reserve
  ammunition (none: no bar): 20 × 4 at 640 (10 × 2 at 320), at the
  picture's top, starting half a bar width in (x + 10 at 640, x + 5 at
  320). The filled share f = reserve / the ammunition type's maximum
  carry, clamped to 0-1; its width is f × bar width truncated, at least
  1 pixel, in the greenish 0 160 0 at full alpha; the rest of the bar
  in the HUD colour at alpha 128. Both additive.

### Kill feed

- At most four lines; a fifth pushes out the oldest; each lasts 6 s
  (`hud_deathnotice_time`, ref. for CS).
- Lines from y 34 (32 + 2) down, one row apart (the taller of the icon
  height 16 and the console font's line height); names' text 4 pixels
  below the line's top.
- Right-aligned 4 pixels from the screen's right edge: the killer's
  name, 5 pixels, the weapon's `d_` icon, then (CS, ref.) `d_headshot`
  after it for a headshot kill, then the victim's name.
- A suicide or a world kill shows no killer name; a kill with an unknown
  weapon shows `d_skull`.
- Names in team colours (ref.: terrorists 255 64 64, counter-terrorists
  153 204 255); the icons additive in 255 80 0 (ref.), a team kill's
  icon 10 240 10 (ref.).

### Damage direction

- On each damage message with a source position: the direction to the
  source, flat in the view's frame; its forward share f and right share
  r (dot products with the view's forward and right). A source within 50
  units lights all four arcs fully (strength 1). Otherwise: f above 0.3
  raises the front arc's strength to at least f, f below −0.3 the
  behind arc's to at least |f|; likewise r for right and left.
- Each arc is drawn while its strength is above 0.4, tinted with the
  health colour (HUD colour, or warning colour at 25 health or less)
  scaled by max(strength, 0.5), additive; strength drains by 2 a second
  (a full hit shows for 0.3 s).
- Places (frame size w × h): front at (W/2 − w/2, H/2 − 3h) (at 640 × 480:
  (256, 96)); right at (W/2 + 2w, H/2 − h/2) ((416, 176)); behind at
  (W/2 − w/2, H/2 + 2h) ((256, 336)); left at (W/2 − 3w, H/2 − h/2)
  ((176, 176)).

### Flashbang fade

As view_and_hud.md, "Flashbang fade": white over everything, the HUD
included.

### Damage type icons

Half-Life's `dmg_*` icons (poison, drown, burn and so on) are in the
list but no CS 1.6 weapon deals those types: not drawn in the party.

## Edge cases

- Above 640 wide nothing scales but the crosshair: the 640 sprites are
  drawn at their own pixel size, positions follow the formulas with the
  real W and H. At
  1920 × 1080 the HUD occupies a thin band at the bottom (see Quirks).
- Health above 999 or reserve above 999 would overflow three places;
  the numbers grow to the right (ref.).
- The ammunition row's icon comes from the weapon's own list, so two
  weapons with the same ammunition share an icon.

## Quirks

- The HUD does not scale with resolution: on today's screens it is
  small. Parity keeps it at its pixel size; a scale option (off by
  default) is ours to add later.
- The UMP45's lit menu picture (`weapon_s`) is the same rectangle in
  the same atlas as its plain one.
- The knife's list names an ammunition icon (buckshot's) though it has
  no ammunition: never drawn.
- The crosshair's closing and its alpha recovery run per drawn frame,
  not per second: at a higher frame rate it closes faster (0.51 s from
  15 to 4 at 100 fps, 1.7 s at 30). Keep, at the frame rate the game
  draws.
- The pistols' "speed" rule has a threshold of 0, so standing still on
  the ground already widens them by 1.5 and ducking (× 0.5 of the base)
  narrows them to a third of that: a Deagle shows 12 standing, 4
  ducked, 16 in the air.
- The M4A1's × 1.4 is meant to depend on its silencer, but the test
  looks at the USP's silencer state, which an M4A1 never has: the M4A1
  is widened by 1.4 with the silencer on or off. Keep.
- The round timer starts at the screen's centre instead of being
  centred on it, so it sits right of centre.
- In the weapon menu the highlighted weapon is no brighter than the
  others (192/255); only its lit picture and the `selection` frame mark
  it.

## Test cases

| Setup | Input | Expected |
|---|---|---|
| 640 × 480 | health 100 | cross at (12, 443); digits `1`,`0`,`0` at x 34, 54, 74, y 448; bar at (104, 448) 2 × 25 |
| 640 × 480 | health 7 | digit `7` alone at x 74 (two empty places), warning colour, alpha 255 |
| health 60, changed 2.5 s ago | alpha | 100 + 50/100 × 128 = 164 |
| health 60, unchanged for 6 s | alpha | 100 |
| 640 × 480 | armour 50, vest | empty suit at (128, 439); lit part rows 12-23 of 24; number at (152, 448) |
| 1920 × 1080 | armour | icon x 384 |
| 640 × 480, AK-47 | clip 30, reserve 90 | `30` in places 2-3 from 456 (digits at 476, 496); bar at (526, 448); `90` digits at 558, 578; icon (72 72 in `640hud7`) at (598, 445) |
| 640 × 480, HE grenade | one carried | `1` at x 576 (places from 536); icon (72 96) at (596, 445) |
| AK-47 | a shot, then 2 s | alpha 200, then 160 |
| kill with the AWP | icon | `d_awp`: `640hud1` 192 128 48 16 |
| kill with the HE grenade | icon | `d_grenade` |
| headshot kill with the deagle | icons | `d_deagle` then `d_headshot` |
| damage from straight ahead, 200 units away | arcs | front strength 1, others 0; drawn 0.3 s |
| damage from 30 units | arcs | all four |
| 640 × 480, AK-47, standing still on the ground | no shots | base 4, gap 4, lines 5 long: left x 312-316, right 324-328, top y 232-236, bottom 244-248, row/column 240/320 |
| AK-47 at rest | one shot | gap 8, lines (8 − 4) × 0.5 + 5 = 7; alpha 200 → 160 |
| AK-47, gap 8 | one frame without a shot | gap 8 − (0.104 + 0.1) = 7.796 |
| AK-47 | gap 15, no more shots | back to 4 after 51 frames |
| AK-47 running at 250 | on the ground | base 6; in the air: 8 |
| M4A1 standing | silencer on or off | base 5 (4 × 1.4 truncated); running 8, in the air 11 |
| Deagle | standing, ducked, in the air | base 12, 4, 16 |
| Glock18 standing | semi-automatic, burst | base 16, 12 |
| MP5 | standing, in the air | base 6, 12 |
| M3 at rest | three shots in three frames | gap 8 → 14 → 15 (clamped; the decay frames between them aside) |
| AWP, Scout, SG550, G3SG1 | any | no crosshair |
| AUG zoomed (fov 55) | | crosshair drawn |
| `cl_crosshair_size` auto, 1920 × 1080, AK-47 at rest | | scale 3: gap 12, lines 15 |
| `cl_crosshair_size` auto, 800 × 600 | | scale 1: gap and lines as at 640 |
| `cl_crosshair_size` small, 640 × 480 | | scale 640 / 1024: gap 2.5, lines 3 |
| `cl_dynamiccrosshair` 0, AK-47 in the air | | base 4; a shot still opens it to 8 |
| `cl_crosshair_translucent` 2 | | usage printed, reset to 1 |
| 640 × 480 | round time 165 s just sent | `2:44`: stopwatch (320, 442), `2` at 346, colon squares (372, 448), (372, 461), `4` 378, `4` 398 |
| 640 × 480 | 65 s left | `1:05`: seconds `0` at 378, `5` at 398 |
| | 25 s left | HUD colour × 100/255 |
| | 20 s left | flashing; reddish phase 0.6 s, then HUD colour 0.6 s (at 19 s: 0.575 s) |
| | time ran out | `0:00`, still drawn |
| 640 × 480, slot 2 holds only the usp | press 2 | slot numbers at x 138, 163, 338, 363, 388 (slot 2's column 175 wide); the usp's `weapon_s` and `selection` at (163, 30) |
| 640 × 480, menu on slot 2 | slot 1 holds an AK-47 with ammunition | a 20 × 20 box at (138, 30) at alpha 128 |
| 640 × 480, menu on slot 1, AK-47 with 45 of 90 reserve | | bar at (148, 30): 10 px greenish, 10 px HUD colour at 128 |
| menu open | 10 s untouched | still open |
| `hud_fastswitch` 1, slot 2 holds only the usp | press 2 | switches to the usp without the menu |
| 320 × 240 | health 100 | cross (8, 216); digits at 22, 34, 46, y 219; bar (64, 219) 1 × 16 |
| 320 × 240 | menu | columns from x 74; the open slot's pictures from y 22, 25 apart; ammo bar 10 × 2 at x + 5 |
| 320 × 240 | timer `2:44` | stopwatch (160, 216); `2` at 176; colon x 192, y 220 and 228; seconds 196, 208 |
| every weapon in `games/cs16/data/weapons` but the knife | its `weapon_<id>.txt` | `weapon`, `weapon_s`, `ammo` rectangles present at 640 |

## Open questions

1. CS 1.6's client against Half-Life's: whether the health bar is drawn,
   where armour sits (W / 5 or three icon widths), the helmet icon rule,
   the kill feed's colours and headshot mark. Capture a client at 640 ×
   480 and 1920 × 1080.
2. Money's exact place and the money change flash.
3. Whether the original client's speed rule for the pistols really
   starts at 0 (standing still widened by 1.5) or the reimplementation
   differs; and whether `hud_fastswitch` 1 with several weapons in a
   slot opens the menu (as here) or cycles. Capture a Deagle's
   crosshair standing and ducked at 640 × 480, and press 4 with two
   grenades.
4. The console font's line height at 640 (the kill feed's row height).
