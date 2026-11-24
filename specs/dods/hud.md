# Day of Defeat: Source: the HUD's player status and the kill icons

Source basis: the dedicated server's data files only, read in place
(2026-11-24): the HUD layout file (`scripts/hudlayout.res`), the player
status panel's layout files (`resource/ui/hudplayerstatus*.res`,
`resource/ui/hudcurrentweapon.res`), the client scheme's colour table
(`resource/clientscheme.res`), the shared icon table
(`scripts/mod_textures.txt`), the weapon scripts' `TextureData` and HUD
clip fields (`scripts/weapon_*.txt`), the class scripts' health image
fields (`scripts/playerclass_*.txt`) and the materials they name. No
code was read, and the retail client was not run, so how the panels
animate and what the layout fields mean beyond their evident sense is
marked as an open question. The values are data the game exposes; the
game supplies them at run time, and they are given here to check against.
Status: draft (data files read 2026-11-24; the client's drawing not
observed)

## Summary

DoD:S's HUD while playing is one strip at the bottom left: a class
silhouette that fills with red as health is lost and a thin health bar
beside it, the held gun's picture, the gun's magazine as a picture that
empties shot by shot with the number of spare magazines beside it, and a
segmented stamina bar under them. The kill feed draws each kill as the
killer's name, a picture of the gun and the victim's name, at the top
right. Colours are olive and sand-yellow over translucent panels.

## Units and conventions

- HUD positions and sizes are in the layout files' units: a virtual
  640 x 480 screen, scaled to the real screen's height (the usual Source
  proportional layout). `r<N>` means N units from the right edge,
  `c<N>` N units from the centre.
- A layout file's child positions are relative to its panel's top left.
- Colours are `R G B A`, 0-255.
- Sheet cells are `x y width height` in texels of the named texture
  (`materials/<file>.vtf`, or the `.vmt`'s `$basetexture`, whose path
  may use back slashes).

## Colours (the client scheme)

| Name | RGBA | Used for (our reading) |
|---|---|---|
| `HudBackground`, `HudPanelBackground` | 123 110 59 184 | panel fills; the kill feed's rows |
| `HudBackgroundActiveSolid` | 240 207 78 180 | the kill feed's rows that involve the local player |
| `HudPanelBorder` | 255 255 255 102 | the status strip's dividing lines |
| `HudProgressBarActive` | 240 207 78 255 | stamina slices that are full |
| `HudProgressBarInActive` | 140 120 73 255 | stamina slices that are empty |
| `HudProgressBarActiveLow` | 240 30 30 255 | full slices below the warning level |
| `HudProgressBarInActiveLow` | 240 30 30 99 | empty slices below the warning level |
| `HudHealthGreen` | 85 112 66 255 | health bar above the first warning |
| `HudHealthYellow` | 240 207 78 255 | health bar between the warnings |
| `HudHealthRed` | 180 20 20 255 | health bar below the second warning; lost health on the silhouette |
| `HudHealthBG` | 170 150 73 255 | the health bar's empty part |
| `HudAmmoCount` | 240 207 78 127 | |

The panel's own fields override two of these: the spare-magazine count is
drawn in `240 207 78 102` and its "x" in `255 255 255 102`.

## The player status strip

`HudPlayerStatusPanel` sits at (0, 290), 640 x 190: the bottom 190 units
of the screen, full width. Its children, relative to it (absolute
virtual-screen y in brackets):

| Part | Position | Size | What it is |
|---|---|---|---|
| Main background | 16, 128 [418] | 240 x 32 | a filled panel in `HudBackground` (our reading: the file names no colour), its top right corner cut off by 8 units (a 45-degree chamfer) |
| Health | 42, 91 [381] | 46 x 78 | the class silhouette and the health bar (below) |
| Left line | 88, 131 [421] | 1 x 26 | a divider in `HudPanelBorder` |
| Current weapon | 90, 130 [420] | 67 x 27 | the held gun's picture (below) |
| Right line | 160, 131 [421] | 1 x 26 | a divider in `HudPanelBorder` |
| Ammo | 168, 115 [405] | 84 x 52 | the magazine and the spare count (below) |
| Stamina | 10, 161 [451] | 266 x 32 | the stamina bar (below) |
| MG heat, fire select | r260, 161 | 260 x 32 | the right side's strip for the machine guns' heat and the BAR's and StG44's fire mode (not specified further here) |
| Team icon | 21, 133 | 22 x 22 | hidden in the files (`visible` 0) |

The silhouette's panel reaches above the background (its top at 381,
the background's at 418): the figure stands out of the strip.

### Health

Inside the health panel:

- The class silhouette, at (0, 5), 36 x 72, scaled to fit: the class
  script's `healthimage` (`materials/vgui/<name>`, e.g.
  `cls_us_garand_active`), with its outline `healthimagebg` drawn at the
  same place. The layout file's note says the image part draws "the
  class image and the red damage-taken part": the share of health lost is
  shown in red on the figure (see open question 1 for the direction).
- The health bar, at (36, 20), 8 x 54: a vertical bar whose filled part
  is health / max health of its height, filled from the bottom (our
  reading), coloured by the warnings `FirstWarning` 0.50 and
  `SecondWarning` 0.25: above 50 % green, from 50 % to 25 % yellow, below
  25 % red (`HudHealthGreen`, `Yellow`, `Red`; the file's own
  `fillcolor` is white, open question 2), the rest `HudHealthBG`.

The class silhouettes per class (the class scripts):

| Class | `healthimage` |
|---|---|
| US Rifleman, Assault, Support, Sniper, MG, Rocket | `cls_us_garand_active`, `cls_us_thompson_active`, `cls_us_bar_active`, `cls_us_spring_active`, `cls_us_30cal_active`, `cls_us_bazooka_active` |
| Axis Rifleman, Assault, Support, Sniper, MG, Rocket | `cls_ger_k98_active`, `cls_ger_mp40_active`, `cls_ger_mp44_active`, `cls_ger_k98_scoped_active`, `cls_ger_mg42_active`, `cls_ger_pschreck_active` |

Each with its `_bg` outline (`healthimagebg`).

### Current weapon

At (0, 0), 67 x 27 of the current-weapon panel: the held weapon's
`weapon_s` cell from its script's `TextureData` (all on
`sprites/hud/dod_weapons01`), scaled to fit the box keeping its aspect
(the cells are about 3:1 to 4:1), with an outline from the same cell of
the `_glow` sheet (`weapon_g`, `sprites/hud/dod_weapons01_glow`) behind
it. `mod_textures.txt` names the same cells as `wpn_<name>` (`wpn_tommy`
is the Thompson's `weapon_s`).

### Ammo

The ammo panel (84 x 52) draws:

- The magazine icon: the weapon script's `ammo` cell (on
  `sprites/hud/clips_bullets`) at the large icon's place, (0, 0) 32 x 51,
  or the small icon's, (0, 16) 32 x 25, which one by the cell's shape
  (open question 3). The script's `ammo2` cell, the same size, is the
  empty magazine drawn under it.
- How full the magazine is: the scripts with `HudClipHeight`,
  `HudClipBaseHeight` and `HudClipBulletHeight` (the SMGs, rifles and
  pistols) have `HudClipHeight` equal to the `ammo` cell's height, and
  base + clip size × bullet height = the cell's height (Thompson: 2 + 30 ×
  6 = 182 of 184; Colt: 6 + 7 × 17 = 125 of 121, open question 4). So the
  full icon (`ammo`) shows over the empty one (`ammo2`) to a height of
  `base + rounds × bullet` texels of `HudClipHeight`, from the bottom: the
  magazine empties from the top as it is fired. Weapons without the
  fields (the MGs, rockets, grenades) show the full icon whatever the
  rounds.
- The spare magazines: "x" in `255 255 255 102` and the number of spare
  magazines (reserve rounds / clip size, rounded up) in
  `240 207 78 102`, at (62, 23); a spare magazine picture at (39, 17),
  22 x 24 (`extra_clip`). For a weapon whose rounds are its own count
  (grenades, one-round rockets) the number is the count left.
- The knife's and spade's `ammo` is a font glyph, not a cell: melee
  weapons show no ammo.

### Stamina

Inside the stamina panel (266 x 32):

- A background, (6, 7), 240 x 14, `HudBackground`, its bottom right
  corner cut by 5.
- An icon, (9, 1), 24 x 24: `stamina_icon`, a glyph of the
  `ClientTitleFontPlayerIcons` font, character `/` (the font is the
  client's, absent from the server's files; open question 5).
- The bar, (31, 9), 210 x 10, drawn as vertical slices 2 units wide
  with 2-unit gaps (53 slices, starting every 4 units, the last ending at
  the bar's edge): the first floor(53 × stamina / 100) slices
  in `HudProgressBarActive`, the rest `HudProgressBarInActive`; below
  the warning level 0.35 the `...Low` pair instead.

## The kill feed

`HudDeathNotice`: right-justified, the whole screen's width, at most 4
notices, rows 19 units tall, names in the scheme's `Default` font; the
rows' background `HudBackground`, and `HudBackgroundActiveSolid` for a
row involving the local player. Each notice: killer's name, the weapon's
icon, victim's name.

### Kill icons

`mod_textures.txt`'s `d_<weapon>` cells, all on
`sprites/hud/death_icons`; `<weapon>` is the weapon's script name
(`scripts/weapon_<weapon>.txt`):

| Icon | Cell (x y w h) | Icon | Cell (x y w h) |
|---|---|---|---|
| `d_thompson` | 337 0 175 61 | `d_mp40` | 3 0 113 51 |
| `d_bar` | 323 60 189 34 | `d_mp44` | 0 112 157 59 |
| `d_garand` | 326 96 186 39 | `d_k98` | 0 323 190 38 |
| `d_spring` | 309 145 203 48 | `d_k98_scoped` | 0 274 190 47 |
| `d_m1carbine` | 318 199 194 46 | `d_c96` | 0 170 165 48 |
| `d_30cal` | 311 249 201 83 | `d_mg42` | 0 361 206 56 |
| `d_bazooka` | 307 336 205 38 | `d_pschreck` | 0 419 201 57 |
| `d_colt` | 231 0 90 61 | `d_p38` | 0 51 98 60 |
| `d_amerknife` | 121 0 96 45 | `d_spade` | 173 166 130 57 |
| `d_frag_us` | 229 229 76 49 | `d_frag_ger` | 122 50 97 40 |
| `d_riflegren_us` | 166 115 134 43 | `d_riflegren_ger` | 12 224 165 44 |
| `d_punch` | 225 371 121 48 | | |

Also: `d_skull_dod` (a glyph, `S` of `ClientTitleFontPlayerIcons`: a death
with no weapon) and `d_dod_bomb_target` (`sprites/hud/explosion`, 0 0 32
32). The `_live` grenades (a thrown-back grenade) have scripts but no
icon of their own (open question 6). `d_punch` is the SMGs' stock
strike's (our reading of the name). The icons are wider than tall
(about 2:1 to 5:1) and scaled to the row's height.

## Test cases

1. Each of the 22 weapons of our roster (weapons.md's table, the
   explosives included) has a `d_<weapon>` cell, inside the 512-wide sheet.
2. A Thompson with 30 rounds shows its full magazine icon to 182 of 184
   texels; with 15, to 92; with 0, the empty icon only (2 of 184).
3. A Thompson with 210 reserve rounds shows "x 7"; with 1, "x 1".
4. Health 100: the bar full and green; 50: half, yellow (at the
   first warning, our reading: green above it only); 20: red.
5. Stamina 100: 53 slices active; 50: 26; 30: 15, in the low colours.
6. The strip's main background spans x 16-256, y 418-450 of the
   640 x 480 screen; the stamina bar x 41-251, y 460-470.

## Open questions

1. The silhouette's red damage part: drawn from the top down, or from
   the bottom up, and whether the outline (`_bg`) is over or under it.
   Needs the retail client.
2. The health bar's own `fillcolor` (white) against the scheme's three
   health colours and the warnings: which wins.
3. Which of the large and small magazine boxes a weapon uses (the large
   one is 32 x 51, the tall magazines; the small one 32 x 25).
4. The Colt's base + 7 × bullet height (125) exceeds its cell (121): how
   the drawing clips it.
5. `ClientTitleFontPlayerIcons` (the stamina icon, the skull, the fire
   select glyphs) is in the client's files only.
6. A thrown-back grenade's kill icon (`frag_us_live` has no `d_` cell).
7. Fading and flashing (`scripts/hudanimations.txt` holds the HUD's
   animations; not read for this spec).
