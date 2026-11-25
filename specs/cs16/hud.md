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
| font height | 25 | the `number_0` cell's height at 640 (13 at 320, ref.) |
| digit width | 20 | the `number_0` cell's width at 640 |
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

CS-only (ref.): bottom centre on the health baseline: the `stopwatch`
icon, then minutes and two-digit seconds separated by a colon made of
two small squares in the HUD colour (`2:45`); the group is centred
on W / 2. Dim alpha at rest; under 20 s (ref.) it turns the warning
colour. In the party it shows the minigame's round time left when the
minigame has one, else it is hidden.

### Crosshair

CS 1.6 draws its crosshair itself (ref.; the weapon lists' `crosshair`
sprite is not what is seen): four lines, above, below, left and right of
the screen centre, each one pixel thick, leaving a square gap at the
centre.

- Colour `cl_crosshair_color` "50 250 50" (the shipped config); with
  `cl_crosshair_translucent` 1 (the default) the lines are added onto
  the frame rather than drawn opaque.
- Size `cl_crosshair_size` "auto": small below 640 wide, medium below
  1024, large at 1024 and more (ref.); the line length grows with the
  size (ref.: about 5, 7 and 9 pixels? to capture).
- With `cl_dynamiccrosshair` 1 (default) the gap is per weapon and
  opens while the player moves, is in the air or keeps firing, and
  closes back over a fraction of a second (ref.): it follows the
  weapon's inaccuracy, the same states weapons.md's spread uses
  (standing, moving, in the air, ducked, consecutive shots). The exact
  per-weapon gaps and growth rates are open (question 3).
- Hidden while a scope's overlay is up (weapons.md, zoom) and while the
  view is not the player's own.

### Weapon menu (bucket list)

Shown while choosing a weapon (the slot keys or the wheel) when fast
switching is off (`hud_fastswitch` 0, the shipped default):

- Top row from (10, 10): one slot number sprite per slot (`bucket1` for
  slot 1 ... `bucket5`, `bucket0` for a sixth), each 20 × 20 and 5 apart;
  the open slot's cell is as wide as its weapons' pictures (170).
- Under a closed slot: one 20 × 20 filled box per weapon it holds, 5
  apart down the column, in the HUD colour at alpha 128, or the empty
  colour at alpha 96 when the weapon has no ammunition.
- Under the open slot, from y 10 + 20 + 10: each weapon's `weapon`
  picture (170 × 45), 5 apart; the chosen one as `weapon_s` at full
  tint with `selection` drawn over it; the others at alpha 192, or in
  the empty colour at 128 without ammunition.
- Slot numbers are drawn at full tint. The list stays until the choice
  is confirmed (fire) or cancelled; a menu not touched for some seconds
  closes (ref., time to capture).
- Each picture may carry an ammunition bar (10 × 2 at 640, scale 2:
  20 × 4, ref.), greenish for the filled share, at least one pixel when
  any is left.

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

- Above 640 wide nothing scales: the 640 sprites are drawn at their own
  pixel size, positions follow the formulas with the real W and H. At
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
| every weapon in `games/cs16/data/weapons` but the knife | its `weapon_<id>.txt` | `weapon`, `weapon_s`, `ammo` rectangles present at 640 |

## Open questions

1. CS 1.6's client against Half-Life's: whether the health bar is drawn,
   where armour sits (W / 5 or three icon widths), the helmet icon rule,
   the kill feed's colours and headshot mark. Capture a client at 640 ×
   480 and 1920 × 1080.
2. Money and the round timer's exact places, the timer's colon, its
   warning time and colour, the money change flash.
3. The dynamic crosshair: line length per size, base gap per weapon,
   how the gap follows movement, jumping, ducking and firing, and how
   fast it closes. Capture by moving and firing each gun with
   `cl_dynamiccrosshair` 1.
4. The weapon menu's close time and its ammunition bars' look in CS.
5. The console font's line height at 640 (the kill feed's row height).
