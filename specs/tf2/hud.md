# Team Fortress 2: the player HUD — health, ammo, crosshair, kill feed, damage numbers (T-H1)

Source basis: the retail client install (app 440, `steam.inf`
PatchVersion 11191711), read in place, nothing launched and nothing
decrypted: `resource/clientscheme.res` (colours, fonts, the custom font
files), `scripts/hudlayout.res`, `scripts/hudanimations_manifest.txt` and
`scripts/hudanimations_tf.txt`, `resource/ui/hudplayerhealth.res`,
`hudammoweapons.res`, `hudplayerclass.res`, `hudhealthaccount.res`,
`huddamageaccount.res`, `hudaccountpanel.res`, `healthiconpanel.res`,
`scripts/mod_textures.txt`, `cfg/user_default.scr` (the advanced-options
list and its defaults), `scripts/game_sounds_weapons.txt` (hit sounds),
the HUD materials (`materials/hud/*.vmt`, `materials/vgui/crosshairs/*`,
`materials/sprites/crosshairs.*`) and their texture sizes; the
textures were decoded only to see which cells hold pictures. Public
documentation: the community TF2 wiki (Heads-up display, Advanced
options, List of useful console commands) and the Valve Developer
Community pages on VGUI resource files and HUD animations. No game,
engine or SDK code was read; HUD-mod repositories were not used. The
shared Source mechanisms (proportional layout, scheme lookup, the
animation-script language) are those already written down in
[specs/hl2dm/hud.md](../hl2dm/hud.md) sections 1–3; this spec only says
where TF2 differs.
Status: draft

## Summary

TF2's HUD is drawn in a warm tan-and-brown palette with the TF2 fonts.
Bottom left: the class portrait and a health **cross** that fills with
off-white as health rises, a dark number in the middle, a white cross
behind it that pulses when overhealed and pulses red faster when health
is under 49 % of max. Bottom right: a team-coloured (RED / BLU) rounded
ammo panel with the clip in a big number and the reserve in a small one
beside it (one big number for weapons without a clip, like the
Minigun); the panel pulses red when ammo is low. Top right: the kill
feed, rows of "killer name, weapon icon, victim name" on dark rounded
boxes, names in team colour, a light row for kills involving you, a red
burst behind the icon for crits. Damage numbers float up from enemies
you hit (on by default per the wiki; off in the shipped options list:
Open questions 3); the hit sound is off by default.

## Units and conventions

- **Layout units** as in hl2dm/hud.md section 1: positions and sizes in
  `.res` files are in a 640 × 480 space where **one unit = H / 480
  pixels on both axes** (H = screen height); the screen is W / u units
  wide (853.33 at 16:9). `rN` = N units from the right (or bottom) edge,
  `cN` = N units from the centre, `f0` = the full parent width, `pN` (seen
  in the class panel) = N × the parent's size. Scaled values become whole
  pixels by truncation.
- Child controls of a `resource/ui/*.res` file are positioned relative to
  their parent panel (the first block of the file, itself placed by
  `hudlayout.res` or by its own keys). `zpos` orders siblings (higher on
  top).
- Key suffixes: `_minmode` applies when `cl_hud_minmode` is 1 (default 0;
  this spec gives normal-mode values). `_lodef` / `_hidef` are
  resolution variants; this spec uses the plain key (Open questions 1).
  `[$WIN32]` is true on every PC platform including Linux, `[$X360]` is
  false.
- **Fonts** (hl2dm/hud.md section 2 rule): a font entry with a `yres`
  range is a pixel size, used when H is in the range; an entry with no
  `yres` is a 480-line size scaled by H / 480. All the HUD's TF2-font
  fonts have one entry without `yres`, so they scale with the screen.
- Colours R G B A, 0–255, by their scheme names (section 2). Scheme
  lookup ignores case (the death notice names `HUDBlack`, the scheme
  defines `HudBlack`).
- Time: client seconds per rendered frame.
- Teams: 2 RED, 3 BLU (classes.md). Panels with `teambg_2` / `teambg_3`
  keys swap their image by the local player's team.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| hud_unit | H / 480 | px | one layout unit |
| health_death_warning | 0.49 | × max health | `HealthDeathWarning`: below this the health panel is in the low state |
| health_bonus_pos_adj | 35 | units | `HealthBonusPosAdj`: how far the overheal cross grows beyond its normal size at the full overheal cap (inferred, 3.3) |
| bonus_pulse_period | 0.4 | s | `HudHealthBonusPulse` cycle |
| dying_pulse_period | 0.25 | s | `HudHealthDyingPulse` cycle |
| low_ammo_pulse_period | 0.25 | s | `HudLowAmmoPulse` cycle |
| delta_lifetime | 1.5 | s | health-gain number and damage number life |
| death_notice_max | 4 | rows | `MaxDeathNotices` |
| death_notice_line | 16 | units | `LineHeight` |
| death_notice_gap | 4 | units | `LineSpacing` |
| death_notice_corner | 3 | units | `CornerRadius` |
| death_notice_icon_scale | 0.35 | – | `IconScale` (3.6) |
| `hud_deathnotice_time` | 6 | s | row life (Source default; Open questions 4) |
| `hud_combattext` | 1 (wiki) / 0 (options list) | – | damage numbers (Open questions 3) |
| `hud_combattext_batching` | 0 | – | 1 sums hits inside a window |
| `hud_combattext_red/green/blue` | 255 / 0 / 0 | – | damage number colour |
| `tf_dingalingaling` | 0 | – | hit sound off |
| `tf_dingaling_volume` | 0.75 | 0–1 | hit sound volume |
| `tf_dingaling_pitchmindmg` / `pitchmaxdmg` | 100 / 100 | pitch | hit sound pitch at low / high damage |
| `tf_dingalingaling_effect` | 0 (wiki) | – | 0 = the default ding; the options list stores −1 |
| `tf_dingalingaling_lasthit` | 0 | – | separate kill sound off |
| `cl_crosshair_file` | "" | – | empty = each weapon's own crosshair (3.4) |
| `cl_hud_minmode` | 0 | – | minimal HUD off |
| `cl_hud_playerclass_use_playermodel` | 1 | – | 3D class portrait on |
| `hud_fastswitch` | 0 | – | – |

## Behavior

### 1. Panels and where they sit

`scripts/hudlayout.res` entries (PC, normal mode). Positions in units;
the 1920 × 1080 column uses u = 2.25 and truncation.

| Element | xpos, ypos | wide × tall | At 1920 × 1080 (px) | Notes |
|---|---|---|---|---|
| `HudPlayerStatus` | 0, 0 | f0 × 480 | full screen | container; holds `HudPlayerHealth` and `HudPlayerClass` (their own `.res` files place them) |
| `HudPlayerHealth` (in `resource/ui/hudplayerhealth.res`) | 0, r120 | 250 × 120 | (0, 810) 562 × 270 | zpos 2 |
| `HudPlayerClass` (in `hudplayerclass.res`) | 0, 0 | f0 × 480 | full screen | portrait children below |
| `HudWeaponAmmo` | r95, r55 | 94 × 45 | (1707, 957) 211 × 101 | children in `hudammoweapons.res` |
| `CHealthAccountPanel` | 76, r152 | 116 × 180 | (171, 738) 261 × 405 | the green "+N" when healed (3.5) |
| `CDamageAccountPanel` | 0, 0 | f0 × 480 | full screen | damage numbers over enemies (3.5) |
| `CHudAccountPanel` | r162, r152 | 116 × 180 | – | Engineer metal; not used by the four guns |
| `HudCrosshair` | 0, 0 | 640 × 480 | full screen | 3.4 |
| `HudDamageIndicator` | – | – | – | the entry exists but is **empty** (3.7) |
| `HudDeathNotice` | r640, 18 | 628 × 468 | (480, 40) 1413 × 1053 | 3.6 |
| `HudItemEffectMeter` | r162, r52 | 100 × 50 | – | per-weapon meters (none for the four guns) |
| `HudMedicCharge` | r138, … | – | – | Medic only |

### 2. Scheme: colours and fonts

#### 2.1 Colours used by these panels (`Colors` section)

| Name | R G B A | Used for |
|---|---|---|
| `TanLight` | 235 226 202 255 | ammo numbers, labels |
| `TanDark` | 117 107 94 255 | health and max-health numbers, status icons |
| `TanDarker`, `Black` | 46 43 42 255 | text shadows (TF2's "black" is a warm near-black) |
| `TransparentBlack` | 0 0 0 196 | reserve-ammo shadow |
| `HUDRedTeam` / `HUDBlueTeam` | 180 92 77 127 / 104 124 155 127 | half-alpha team colours |
| `HUDRedTeamSolid` / `HUDBlueTeamSolid` | 180 92 77 255 / 104 124 155 255 | kill feed names (`TeamRed` / `TeamBlue`) |
| `HUDSpectator` | 124 124 124 127 | – |
| `HUDDeathWarning` | 255 0 0 255 | `HealthDeathWarningColor`: the low-health cross |
| `LowHealthRed` | 255 0 0 255 | buildings' low health |
| `HudWhite` | 255 255 255 255 | kill feed icon colour (`IconColor`) |
| `HudOffWhite` | 200 187 161 255 | small health icons over teammates |
| `HudBlack` | 65 65 65 255 | kill feed `LocalPlayerColor` |
| `HudProgressBarActive` / `InActive` | 240 207 78 255 / 140 120 73 255 | meters |
| `HudProgressBarActiveLow` | 240 30 30 255 | meters when low |
| `ProgressOffWhite` | 251 235 202 255 | metal icon |
| `TFOrange` | 145 73 59 255 | menu accents |
| `White` | 235 235 235 255 | – |
| `ItemAttribPositive` / `Negative` | 153 204 255 255 / 255 64 64 255 | item text (reference for "buff" blue and "debuff" red) |

Kill feed row backgrounds are given in `hudlayout.res` directly:
`BaseBackgroundColor` 46 43 42 220, `LocalBackgroundColor` 245 229 196
200. The damage and healing numbers use `PositiveColor` 0 255 0 255 and
`NegativeColor` 255 0 0 255 from their `.res` files; the damage number's
colour is then the `hud_combattext_red/green/blue` cvars (255 0 0).

#### 2.2 Fonts

Custom font files (`CustomFontFiles`, all under `resource/`, loose in
the client's `tf/resource/`): `tf.ttf` (family "Team Fortress", the HUD
icon font), `tfd.ttf` (family "tfd", kill icons as glyphs), `TF2.ttf`
("TF2"), `TF2Secondary.ttf` ("TF2 Secondary"), `TF2Professor.ttf`,
`TF2Build.ttf` ("TF2 Build"), `ocra.ttf`. The install's file names are
lower case (`tf2.ttf`, `tf2build.ttf`, `tf2secondary.ttf`); the lookup
ignores case.

| Font | Family | tall (480-line) | px at 1080 | Weight | Flags | Used by |
|---|---|---|---|---|---|---|
| `HudClassHealth` | TF2 | 16 | 36 | 500 | antialias | current health |
| `HudFontGiantBold` | TF2 Build | 44 | 99 | 500 | antialias | clip, and the single number of no-clip weapons |
| `HudFontMediumSmall` | TF2 | 18 | 40 | 500 | antialias | reserve ammo |
| `HudFontMedium` | TF2 | 24 | 54 | 500 | antialias | "+N" health gain, big damage numbers |
| `HudFontSmall` | TF2 Secondary | 14 | 31 | 500 | antialias | damage numbers |
| `HudFontMediumSmallBold` | TF2 Build | 18 | 40 | 500 | antialias | – |
| `HudFontBiggerBold` | TF2 Build | 35 | 78 | 500 | antialias | – |
| `TFTypeDeath` | tfd | 28 | 63 | 0 | antialias | glyph kill icons (only `ammo_12g` among the entries this spec needs) |
| `Icons` | Team Fortress | 28 | 63 | 0 | additive, antialias | `health_icon` (`b`), plus/minus signs |
| `DefaultSmall` | Verdana | 12 (480–599), 13 (600–767), 14 (768–1023), 20 (1024–1199) px; 9 scaled (other heights) | 20 | 0 | antialias from 768 lines | max health |
| `Default` | Verdana | same sizes as `DefaultSmall` | 20 | 900 | same | kill feed names (`TextFont`) |

Notes: `Default` and `DefaultSmall` have two more entries without
`yres` after the scaled one (Verdana 12 / Arial 12, a narrower
character range); the first matching entry wins, so they are never
used. `tall_hidef` / `tall_lodef` values (e.g. `HudClassHealth` 22) are
ignored here (Open questions 1). Additive is off for every TF2-family
font, unlike HL2.

### 3. The panels

#### 3.1 Animation events (`scripts/hudanimations_tf.txt`)

The manifest loads HL2's `scripts/hudanimations.txt` first, then
`hudanimations_tf.txt`; the HL2 events name panels TF2 does not have
and do nothing. The animation language (Animate / RunEvent / StopEvent,
interpolators, restart rule) is hl2dm/hud.md section 3. TF2's events for
these panels, all Linear:

| Event | Steps |
|---|---|
| `HudHealthBonusPulse` | `PlayerStatusHealthBonusImage` Alpha → 255 over 0–0.2 s; → 0 over 0.2–0.6 s; at 0.4 s run `HudHealthBonusPulseLoop` |
| `HudHealthBonusPulseLoop` | run `HudHealthBonusPulse` at 0 |
| `HudHealthBonusPulseStop` | stop both events at 0 |
| `HudHealthDyingPulse` | same image Alpha → 255 over 0–0.075 s; → 0 over 0.125–0.2 s; at 0.25 s run `HudHealthDyingPulseLoop` |
| `HudHealthDyingPulseLoop` / `Stop` | as above |
| `HudLowAmmoPulse` | `HudWeaponLowAmmoImage` Alpha → 255 over 0–0.075 s; → 0 over 0.125–0.2 s; at 0.25 s run `HudLowAmmoPulseLoop` |
| `HudLowAmmoPulseLoop` / `Stop` | as above |
| `DamagedPlayer` | empty |
| `LevelInit` | empty |

So the bonus pulse repeats every 0.4 s, but because the loop restarts
`HudHealthBonusPulse` at 0.4 s, which removes its own still-running
fade-out (restart rule), each cycle is: up 0.2 s to 255, then down for
0.2 s reaching 255 − 255·(0.2/0.4) = 127.5 → 127, then the next rise
starts from 127. The two fast pulses finish within their 0.25 s cycle
(up 0.075 s, hold 0.05 s, down 0.075 s, off 0.05 s).

#### 3.2 Health: layout (`resource/ui/hudplayerhealth.res`)

Parent `HudPlayerHealth` at (0, r120), 250 × 120. Children (relative):

| Control | Kind | Pos | Size | zpos | Content |
|---|---|---|---|---|---|
| `PlayerStatusHealthImageBG` | image | 73, 33 | 55 × 55 | 3 | `materials/hud/health_bg` (dark cross outline, 128² texture) |
| `PlayerStatusHealthBonusImage` | image | 73, 33 | 55 × 55 | 2 | `materials/hud/health_over_bg` (plain white cross; vertex colour and alpha; hidden at start) |
| `PlayerStatusHealthImage` | image | 75, 35 | 51 × 51 | 4 | filled by code: `materials/hud/health_color` (off-white cross, average 212 199 171) alive, `materials/hud/health_dead` when dead (inferred from the material names) |
| `PlayerStatusHealthValue` | label | 76, 52 | 50 × 18 | 5 | "%Health%", centred, `HudClassHealth`, `TanDark` |
| `PlayerStatusMaxHealthValue` | label | 76, 20 | 50 × 18 | 6 | "%MaxHealth%", centred, `DefaultSmall`, `TanDark` |
| status icons (`PlayerStatusBleedImage`, `…MilkImage`, `…MarkedForDeathImage`, Medic resist icons, rune icons, …) | image | 85 (or 117), 0 | 32 × 32 | 7 | conditions; none are caused by the four guns |

At 1920 × 1080: background cross (164, 884) 123 × 123 px, fill cross
(168, 888) 114 × 114, number box (171, 927) 112 × 40.

#### 3.3 Health: what it shows and when

Inputs: local player health h, max health M (classes.md), alive.

- **Number**: h as a plain integer, centred in its box. Dead: the
  panel's alive visuals are replaced by `health_dead` (assumed; Open
  questions 5).
- **Fill**: the off-white cross is cropped from the bottom up to the
  fraction f = clamp(h / M, 0, 1) of its height, so it empties
  downward as the player is hurt (public: "the cross … turns red if low
  enough", screenshots show a partly filled cross; the crop direction
  and whether it is drawn over the dark background cross are inferred).
- **Max health** label: shows M above the cross. When it is visible is
  not in the data (Open questions 6).
- **Overheal** (h > M): `PlayerStatusHealthBonusImage` becomes visible,
  tinted white, and grows: its size goes from the 55-unit base toward
  55 + 2·35 at the overheal cap 1.5 M, centred on the same point, with
  `HudHealthBonusPulse` running (inferred from `HealthBonusPosAdj` 35
  and the public description "extends beyond borders if overhealed";
  the exact growth law is Open questions 7). The event starts when h
  first exceeds M and `HudHealthBonusPulseStop` runs when it drops back
  to M or below.
- **Low health**: when h < 0.49 M (and alive) the same bonus image is
  shown at its base size tinted `HUDDeathWarning` (255 0 0) and
  `HudHealthDyingPulse` runs (the fast red flash behind the cross);
  `HudHealthDyingPulseStop` when h rises to 0.49 M or more (inferred
  pairing of the `HealthDeathWarning` keys with the events; strict "<"
  is a guess: Open questions 8).
- Low and overheal can't both hold. The pulse events only animate the
  image's alpha; its colour and size are set by the panel.

| Class | M | Low below (0.49 M) | Cap (1.5 M, rounded down to 5) |
|---|---|---|---|
| Scout, Engineer, Sniper, Spy | 125 | 61.25 → low at ≤ 61 | 185 |
| Soldier | 200 | 98 → low at ≤ 97 | 300 |
| Pyro, Demoman | 175 | 85.75 → low at ≤ 85 | 260 |
| Heavy | 300 | 147 → low at ≤ 146 | 450 |
| Medic | 150 | 73.5 → low at ≤ 73 | 225 |

#### 3.4 Crosshair (`HudCrosshair`)

- Full-screen panel; the crosshair is drawn centred on the screen.
- `cl_crosshair_file` "" (default): each weapon's own crosshair, named in
  its weapon script. The scripts are encrypted (install.md Part 6), so
  which picture each of the four guns uses is not in the plain data
  (Open questions 2). The pictures are cells of the sprite sheet
  `materials/sprites/crosshairs` (`UnlitGeneric`, translucent, vertex
  colour, 128 × 128 texture). Cells found in it (x, y, w, h in texels):

| Cell | Picture |
|---|---|
| 0, 0, 32, 32 | two curved brackets with a small dot between them, "( · )" |
| 32, 0, 32, 32 | empty |
| 64, 0, 32, 32 | a thin cross, longer horizontally (x 65–94, y 8–20) |
| 0, 32, 32, 32 | empty |
| 32, 32, 32, 32 | a circle (ring) |
| 0, 64, 32, 32 | an outlined plus sign |
| 64, 64, 64, 64 | a large ring made of four arcs (one per 32² quadrant) |

  `mod_textures.txt`'s `crosshair_default` names the empty cell (32, 0)
  32 × 32.
- `cl_crosshair_file` set to `crosshair1` … `crosshair7` or `default`
  replaces every weapon's crosshair with
  `materials/vgui/crosshairs/<name>` (64 × 64 textures; `UnlitGeneric`,
  translucent, vertex colour, no depth test). Public: `default` is the
  dot in brackets.
- Colour and size come from `cl_crosshair_red/green/blue` and
  `cl_crosshair_scale` (defaults not in the plain data: Open questions 2).
  The picture is drawn with vertex colour = that colour.
- Hidden when the player is dead and while a scope or the scoreboard
  covers it (Sniper; not these guns).

#### 3.5 Ammo (`HudWeaponAmmo`, `resource/ui/hudammoweapons.res`)

Parent at (r95, r55), 94 × 45. Children (relative):

| Control | Pos | Size | zpos | Content |
|---|---|---|---|---|
| `HudWeaponAmmoBG` | 4, 0 | 90 × 45 | 1 | team background: `materials/hud/ammo_red_bg` (RED), `ammo_blue_bg` (BLU) |
| `HudWeaponLowAmmoImage` | 4, 0 | 90 × 45 | 0 | `ammo_red_bg` on RED, `ammo_blue_bg` on BLU (the `teambg_*` keys override the base `image`); hidden; the low-ammo pulse |
| `AmmoInClip` | 4, 0 | 55 × 40 | 5 | "%Ammo%", `HudFontGiantBold`, `TanLight`, aligned bottom-right (south-east) |
| `AmmoInClipShadow` | 5, 1 | 55 × 40 | 5 | same text in `Black` (46 43 42) |
| `AmmoInReserve` | 59, 8 | 40 × 27 | 7 | "%AmmoInReserve%", `HudFontMediumSmall`, `TanLight`, aligned bottom-left (south-west) |
| `AmmoInReserveShadow` | 60, 9 | 40 × 27 | 7 | same in `TransparentBlack` |
| `AmmoNoClip` | 0, 2 | 84 × 40 | 5 | "%Ammo%" for weapons without a clip, `HudFontGiantBold`, `TanLight`, south-east |
| `AmmoNoClipShadow` | 1, 3 | 84 × 40 | 5 | same in `Black` |

All six labels start hidden; the panel shows one set:

- **Clip weapon** (Scattergun, Rocket Launcher, Shotgun): `AmmoInClip`
  and its shadow show the loaded count, `AmmoInReserve` and its shadow
  the reserve. The shadow is drawn first (same zpos; file order) so the
  number reads as tan with a dark drop shadow 1 unit right and down.
- **No clip** (Minigun): only `AmmoNoClip` and its shadow, showing the
  ammo count (200 at spawn), right-aligned in the wider box. The reserve
  labels stay hidden.
- **Background**: the team image. Its material draws a distance-field
  rounded panel from `hud/gradient_red` / `hud/gradient_blue` masked by
  `hud/ammo_area_mask`, with a soft edge, a light outline (0.7 0.7 0.5)
  and a soft black drop shadow (alpha 0.45) — these are material
  parameters of the UnlitGeneric distance-alpha path; Chimera may bake
  them into one texture.
- **Low ammo**: `HudLowAmmoPulse` runs while ammo is low and
  `HudLowAmmoPulseStop` when it is not. The pulsing image sits
  **under** the background (zpos 0 vs 1) at the same rectangle; with the
  RED team's own red image it is invisible unless the background is
  translucent, so the visible effect is a flicker of the panel edge
  (inferred; Open questions 9). The low threshold is not in the plain
  data (Open questions 9).
- The panel is hidden while dead, and for weapons that use no ammo
  (melee).

At 1920 × 1080: panel (1707, 957); clip box (1716, 957) 123 × 90; reserve
box (1839, 975) 90 × 60.

#### 3.6 Kill feed (`HudDeathNotice`)

Keys: `MaxDeathNotices` 4, `IconScale` 0.35, `LineHeight` 16,
`LineSpacing` 4, `CornerRadius` 3, `RightJustify` 1, `TextFont`
`Default`, `TeamBlue` `HUDBlueTeamSolid`, `TeamRed`
`HUDRedTeamSolid`, `IconColor` `HudWhite`, `LocalPlayerColor`
`HUDBlack`, `BaseBackgroundColor` 46 43 42 220, `LocalBackgroundColor`
245 229 196 200.

- **Rows** come from each player death (the server's death event:
  killer, assister, victim, weapon kill-icon name, crit flag). At most 4
  rows; a fifth removes the oldest. Each lives `hud_deathnotice_time`
  seconds.
- **Row layout** (inferred from the keys; the Source death-notice
  layout is hl2dm/hud.md section 10): row i (0 = oldest) at panel y =
  i × (16 + 4) units; each row is right-justified to the panel's right
  edge (screen r12); left to right: killer name, icon, victim name. A
  rounded box (radius 3 units) fills the row behind the text in
  `BaseBackgroundColor`; the box is 16 units tall.
- **Names** in `Default`, each in its team's solid colour (RED 180 92 77,
  BLU 104 124 155).
- **Icon**: `mod_textures.txt` entry named by the kill-icon name, drawn
  from `materials/hud/d_images` (512 × 512) for normal rows. The entry's
  pixel size × `IconScale` gives its size in layout units (inferred:
  a 96 × 32 cell becomes 33.6 × 11.2 units, 75 × 25 px at 1080),
  vertically centred in the row, tinted `HudWhite`. Entries for the four
  guns (x, y, w, h in texels of `d_images` / `dneg_images`):

| Kill-icon name | Cell | Used for |
|---|---|---|
| `scattergun` | 96, 192, 96, 32 | Scattergun |
| `tf_projectile_rocket` | 0, 224, 96, 32 | Rocket Launcher (the rocket names the kill) |
| `shotgun_primary`, `shotgun_soldier`, `shotgun_pyro`, `shotgun_hwg` | 0, 256, 96, 32 (all four) | Shotgun by class |
| `minigun` | 0, 384, 96, 32 | Minigun |
| `skull` | 96, 288, 96, 32 | suicides and unknown names (inferred) |
| `crit` | 192, 409, 64, 34 | drawn behind the icon on crit kills |

- **Local player involved** (killer, assister or victim is you): the
  row background is `LocalBackgroundColor` (light tan 245 229 196 200)
  and the icon comes from the same cell of `materials/hud/dneg_images`
  (a dark "negative" version that reads on the light box).
  `LocalPlayerColor` (`HudBlack` 65 65 65) is the colour for text that is
  not a team name on that row (inferred; Open questions 10).
- **Crit kill**: the `crit` cell (a red burst, 64 × 34) is drawn centred
  behind the weapon icon (public: crit kills show a red glow behind the
  icon; placement inferred).
- **Assist**: "killer + assister" both named in team colour before the
  icon (public behaviour; spacing not in the data).
- Both texture materials are `UnlitGeneric`, translucent, vertex colour
  and vertex alpha, no depth test, **not** additive.

#### 3.7 Damage direction

`HudDamageIndicator` has an empty entry in `hudlayout.res` and no TF2
event or material for it was found in the plain data (no
`HudTakeDamage*` events in `hudanimations_tf.txt`). Whether TF2 draws
HL2-style screen-edge flashes, a different indicator or none is Open
questions 11. Until checked, Chimera draws none for TF2.

#### 3.8 Damage numbers and the health-gain number

- **Damage numbers** (`CDamageAccountPanel`, `resource/ui/huddamageaccount.res`):
  when `hud_combattext` is 1, each hit you deal on an enemy spawns
  "−N" (N = damage) above the victim, in `hud_combattext_red/green/blue`
  (255 0 0), font `HudFontSmall` (`HudFontMedium` for the "big" variant;
  which hits are big is not in the data, Open questions 12), rising and
  fading out over `delta_lifetime` 1.5 s. With `hud_combattext_batching`
  1, hits on the same target within `hud_combattext_batching_window`
  are summed into one number. Pellet weapons (Scattergun, Shotgun,
  Minigun) produce one number per shot (the pellets' sum) (public
  behaviour; batching of pellets inferred).
- **Health gain** (`CHealthAccountPanel`, `hudhealthaccount.res`): when
  your health rises (kit, Medic, dispenser), "+N" appears at x 13, y 50
  in the panel (screen (89, r102) units) and drifts to y 0 over 1.5 s,
  in `PositiveColor` 0 255 0 255, font `HudFontMedium`; health losses
  use `NegativeColor` 255 0 0 255 (whether losses are shown is Open
  questions 12).

#### 3.9 Hit sound

With `tf_dingalingaling` 1, every hit you deal on an enemy plays
`Player.HitSoundDefaultDing` (`)ui/hitsound.wav`, CHAN_STATIC, no
attenuation (SNDLVL_NONE), volume 1) at `tf_dingaling_volume` 0.75, its
pitch interpolated from `tf_dingaling_pitchmindmg` (low damage) to
`tf_dingaling_pitchmaxdmg` (high damage), both 100 by default. Other
effects (`tf_dingalingaling_effect` 1–8) are `Player.HitSoundElectro`,
`…Notes`, `…Percussion`, `…Retro`, `…Space`, `…Beepo`, `…Vortex`,
`…Squasher` (files `ui/hitsound_*.wav`). Off by default.

#### 3.10 Class portrait (`HudPlayerClass`, for completeness)

`PlayerStatusClassImage` (25, r88) 75 × 75 (`../hud/class_<class><team>`
images) over `PlayerStatusClassImageBG` (9, r60) 100 × 50
(`../hud/character_red_bg` / `character_blue_bg`). With
`cl_hud_playerclass_use_playermodel` 1 (default) a 3D model panel
`classmodelpanel` (0, r214) 100 × 200 shows the player model instead,
over `classmodelpanelBG` (−1, r60) 109 × 50
(`character_red_bg_clipped` / `…blue…`). Chimera may draw the 2D image.

## Per-frame order

1. Read local state: alive, team, class, health h, max M, active weapon
   (clip, reserve, uses clip), crosshair choice.
2. Health: update number, fill fraction and the overheal / low state;
   start or stop the bonus or dying pulse on a state change.
3. Ammo: choose clip or no-clip labels, set the numbers and the team
   background; start or stop the low-ammo pulse on a change.
4. Advance HUD animations (hl2dm/hud.md section 3).
5. Expire death-notice rows, damage numbers and health-gain numbers.
6. Draw by zpos: health panel, class portrait, ammo panel, crosshair,
   damage numbers, kill feed.

## Edge cases

- Overheal state is decided on h > M, not on the cap: a Heavy at 301
  pulses as much as one at 450.
- A class change changes M at once: a Heavy (300) at 100 health is not
  low (147 threshold); the same 100 health as a Scout (125) is not low
  either (61), but 60 is.
- Minigun: the ammo panel never shows a reserve; spinning without
  firing does not change the number (weapons.md 7.1).
- Rocket Launcher with 0 loaded and 20 in reserve shows "0" big and "20"
  small; the low-ammo pulse is likely on (Open questions 9).
- Kill by the world (fall) or a suicide by your own rocket: one name and
  the skull or rocket icon (public behaviour: rocket suicides show the
  rocket icon; Open questions 10).

## Quirks

- `crosshair_default` in `mod_textures.txt` points at an empty cell of
  the sprite sheet; keep it as data, never draw it as "the" default.
- The low-ammo image is below its own background (zpos 0 < 1).
- The overheal and low-health effects share one image control
  (`PlayerStatusHealthBonusImage`), only recoloured.

## Test cases

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | 1920 × 1080 | – | – | u = 2.25; ammo panel at (1707, 957) 211 × 101 px; health panel (0, 810) 562 × 270; kill feed panel (480, 40) |
| 2 | 1280 × 720 | – | – | u = 1.5; ammo panel x = 1280 − 142 = 1138, y = 720 − 82 = 638, 141 × 67 |
| 3 | Heavy at 150 / 300 | – | – | health text "150"; fill 0.5; not low (150 ≥ 147); no overheal; no pulse |
| 4 | Heavy at 146 / 300 | – | – | low on: bonus image red (255 0 0), `HudHealthDyingPulse` running |
| 5 | Heavy at 147 / 300 | – | – | low off |
| 6 | Scout at 185 / 125 | – | – | overheal on, white bonus image at its largest (55 + 70 units, inferred law), `HudHealthBonusPulse` running; text "185" |
| 7 | Scout overhealed, decays to 125 | – | – | `HudHealthBonusPulseStop` when h ≤ 125; bonus image hidden |
| 8 | bonus pulse | start at t = 0 | 0.2 / 0.4 / 0.6 s | image alpha 255 / 127 / 255 |
| 9 | dying pulse | start at t = 0 | 0.075 / 0.1 / 0.2 / 0.25 s | alpha 255 / 255 / 0 / restarts |
| 10 | Scout with Scattergun 6/32, RED | – | – | big "6" in TF2 Build 99 px (1080p), small "32" in TF2 40 px; red background image |
| 11 | Heavy with Minigun, 200 ammo, BLU | – | – | only `AmmoNoClip` "200"; reserve labels hidden; blue background |
| 12 | Soldier kills a BLU Scout with a rocket | – | – | row: "Soldier-name" (180 92 77), `tf_projectile_rocket` icon from `d_images` (0, 224, 96, 32), "Scout-name" (104 124 155) on 46 43 42 220 |
| 13 | as 12, the local player is the victim | – | – | row background 245 229 196 200; icon from `dneg_images` same cell |
| 14 | as 12, crit | – | – | `crit` cell (192, 409, 64, 34) drawn behind the icon |
| 15 | 5 kills within 6 s | – | – | 4 rows; the first is gone |
| 16 | Engineer shotgun kill, Heavy shotgun kill | – | – | icon names `shotgun_primary`, `shotgun_hwg`; both the same cell (0, 256) |
| 17 | hit for 54 with `hud_combattext` 1 | – | 0 / 1.5 s | "−54" in 255 0 0 above the victim; gone at 1.5 s |
| 18 | `tf_dingalingaling` 1, a hit | – | – | `Player.HitSoundDefaultDing` at volume 0.75, pitch 100 |
| 19 | Heavy healed from 200 to 260 by a kit | – | 0 / 1.5 s | "+60" (green) at panel y 50 → 0, gone at 1.5 s |
| 20 | font sizes at 1440 lines | – | – | `HudClassHealth` 48 px; `Default` 9 × 3 = 27 px (scaled entry, antialias) |

## Open questions

1. **`_lodef` / `_hidef` keys** (`tall_hidef` 22 for `HudClassHealth`,
   `HudFontGiantBold` `tall_lodef` 52, ammo label variants): when do they
   apply on PC? Assumed never (console leftovers). Check: screenshot a
   1080p and a 720p client and measure the health digits.
2. **Per-weapon crosshairs and crosshair cvars.** Which sprite-sheet cell
   each stock gun uses (in the encrypted weapon scripts), and the
   defaults of `cl_crosshair_red/green/blue` and `cl_crosshair_scale`
   (recalled community values 200 / 200 / 200 and 32, unverified).
   Check in the live client: `cl_crosshair_red` etc. in the console, and
   a centred screenshot per weapon.
3. **`hud_combattext` default.** The wiki's Advanced options table says
   1; the shipped `cfg/user_default.scr` lists 0 (and
   `tf_dingalingaling_effect` −1 against the wiki's 0). The `.scr` is the
   options dialog's list and may not be the cvar default. Check with the
   console on a fresh config.
4. `hud_deathnotice_time` default in TF2 (6 assumed from Source).
5. Dead state of the health panel (`health_dead` image; number hidden?).
6. When `PlayerStatusMaxHealthValue` is visible (always, only when hurt,
   only when overhealed).
7. The overheal cross's growth law (linear in (h − M)/(cap − M) from 55
   to 55 + 2 × 35 units, or a fixed size) and whether it stays centred.
8. The low-health comparison (< or ≤ 0.49 M) and whether the number also
   turns red; whether the fill cross is tinted.
9. **Low ammo**: the threshold (fraction of clip? of total?) and whether
   it applies to no-clip weapons; what the pulse looks like given the
   image is below the background.
10. Kill-feed details: suicide and world-kill rows, where `LocalPlayerColor`
    is used, assister separator, exact spacing between name, icon and
    name, and the crit burst placement.
11. **Damage direction indicator**: TF2 has one in play (red arcs around
    the crosshair, from memory of the live game) but no plain-data
    resource was found; find its material and behaviour in the live
    client (screenshots while taking damage from four directions).
12. Damage numbers: which hits use the big font, motion path and fade
    curve, whether health losses show in `CHealthAccountPanel`.
