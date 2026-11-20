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
Community pages on VGUI resource files and HUD animations. For the
2026-11-15 check, the SDK's TF2 game code (Valve's Source SDK 2013 with
TF2's client and server code, read in place on the Linux box, nothing
copied) was read for the client HUD's logic: the health and ammo panels,
the crosshair, the kill feed, the damage numbers, the hit sounds and the
damage-direction indicator. HUD-mod repositories were not used. The
shared Source mechanisms (proportional layout, scheme lookup, the
animation-script language) are those already written down in
[specs/hl2dm/hud.md](../hl2dm/hud.md) sections 1–3; this spec only says
where TF2 differs.
Status: checked against the SDK's game code (2026-11-15)

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
you hit (on by default); the hit sound is off by default. Red arcs
around the screen centre point toward where damage came from.

## Corrections from the SDK (2026-11-15)

- Overheal growth "35 units, law inferred" -> `HealthBonusPosAdj` is a
  plain **pixel** count (not scaled); each side grows by
  round(min((h − M) / (cap − M), 1) × 35) px.
- Low health "the bonus image at base size" -> it also grows, by
  round((0.49 M − h) / (0.49 M) × 35) px per side, and the **fill cross
  is tinted red** too; the test is strict h < 0.49 M.
- Max-health label "when visible unknown" -> shown only while alive and
  M − h ≥ 5.
- Dead "assumed" -> the number and max are blank, the dark cross and
  the bonus image hidden, the fill drawn as `health_dead` in full.
- Low ammo "threshold unknown" -> low when (clip + reserve) <
  round(0.40 × (class maximum + clip size)); for a no-clip gun the count
  against round(0.40 × maximum). The image grows up to 5 px per side.
  The Rocket Launcher at 0 + 20 is **not** low.
- Crosshair cvars "unverified" -> `cl_crosshair_red/green/blue` 200,
  `cl_crosshair_scale` 32, alpha 255; size in screen pixels, not scaled.
- `hud_deathnotice_time` "assumed 6" -> 6 s, and **12 s** for rows
  involving the local player; overflow drops the oldest rows that do
  not involve you first.
- Kill-feed icon size "`IconScale` × cell" -> `IconScale` is not read;
  the icon is scaled to the row height minus 2 units, keeping its
  aspect. Icons are looked up as `d_<name>` (`dneg_<name>` on your rows),
  falling back to `d_skull_tf`.
- `hud_combattext` "1 or 0" -> 1 (on); damage numbers rise 32 world
  units above the victim, fade over the second half of their life, use
  the big font for crits and mini-crits, and need a clear line of sight.
- `tf_dingalingaling_effect` "0 or −1" -> 0. Hit-sound pitch maps damage
  10 … 150 onto min … max pitch; a separate kill sound and a repeat
  delay exist.
- Damage indicator "none found, draw none" -> TF2 draws one (built-in
  defaults; 3.7).

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
| health_bonus_pos_adj | 35 | px (not scaled) | `HealthBonusPosAdj` (built-in default 25; the file sets 35): how far each side of the bonus cross grows at full overheal or at 0 health (3.3) |
| health_think | 0.05 | s | the health panel re-reads health this often |
| low_ammo_fraction | 0.40 | – | `hud_lowammowarning_threshold`: low below this share of the full ammo (3.5) |
| low_ammo_max_grow | 5 | px | `hud_lowammowarning_maxposadjust`: the low-ammo image's growth per side at 0 ammo |
| ammo_think | 0.1 | s | the ammo panel re-reads ammo this often |
| bonus_pulse_period | 0.4 | s | `HudHealthBonusPulse` cycle |
| dying_pulse_period | 0.25 | s | `HudHealthDyingPulse` cycle |
| low_ammo_pulse_period | 0.25 | s | `HudLowAmmoPulse` cycle |
| delta_lifetime | 1.5 | s | health-gain number and damage number life (the `.res` value; built-in default 2.0) |
| death_notice_max | 4 | rows | `MaxDeathNotices` |
| death_notice_line | 16 | units | `LineHeight` |
| death_notice_gap | 4 | units | `LineSpacing` |
| death_notice_corner | 3 | units | `CornerRadius` |
| death_notice_icon_scale | 0.35 | – | `IconScale` (3.6) |
| `hud_deathnotice_time` | 6 | s | row life; doubled for rows involving the local player; 0 turns the feed off |
| `hud_combattext` | 1 | – | damage numbers on |
| `hud_combattext_healing` | 1 | – | heal numbers over heal targets (Medic) |
| `hud_combattext_batching` | 0 | – | 1 sums hits inside a window |
| `hud_combattext_batching_window` | 0.2 | s | the window (0.1 … 2) |
| `hud_combattext_red/green/blue` | 255 / 0 / 0 | – | damage number colour |
| `tf_dingalingaling` | 0 | – | hit sound off |
| `tf_dingaling_volume` | 0.75 | 0–1 | hit sound volume |
| `tf_dingaling_pitchmindmg` / `pitchmaxdmg` | 100 / 100 | pitch (1 … 255) | hit sound pitch at ≤ 10 / ≥ 150 damage |
| `tf_dingaling_pitch_override` | −1 | pitch | > 0: every hit sound at this pitch |
| `tf_dingalingaling_repeat_delay` | 0 | s | minimum gap between hit sounds; 0 = every hit |
| `tf_dingalingaling_effect` | 0 | 0 … 8 | 0 = the default ding |
| `tf_dingalingaling_lasthit` | 0 | – | separate kill sound off |
| `tf_dingaling_lasthit_volume` | 0.75 | 0–1 | kill sound volume |
| `tf_dingaling_lasthit_pitchmindmg` / `pitchmaxdmg` | 100 / 100 | pitch | kill sound pitch |
| `tf_dingalingaling_last_effect` | 0 | 0 … 8 | which kill sound |
| `cl_crosshair_red` / `green` / `blue` | 200 / 200 / 200 | 0–255 | crosshair colour (alpha 255) |
| `cl_crosshair_scale` | 32 | px | crosshair size (32 = the picture's own size) |
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
| `PlayerStatusHealthImage` | image | 75, 35 | 51 × 51 | 4 | filled by code: `materials/hud/health_color` (off-white cross, average 212 199 171) alive, `materials/hud/health_dead` when dead (SDK confirms) |
| `PlayerStatusHealthValue` | label | 76, 52 | 50 × 18 | 5 | "%Health%", centred, `HudClassHealth`, `TanDark` |
| `PlayerStatusMaxHealthValue` | label | 76, 20 | 50 × 18 | 6 | "%MaxHealth%", centred, `DefaultSmall`, `TanDark` |
| status icons (`PlayerStatusBleedImage`, `…MilkImage`, `…MarkedForDeathImage`, Medic resist icons, rune icons, …) | image | 85 (or 117), 0 | 32 × 32 | 7 | conditions; none are caused by the four guns |

At 1920 × 1080: background cross (164, 884) 123 × 123 px, fill cross
(168, 888) 114 × 114, number box (171, 927) 112 × 40.

#### 3.3 Health: what it shows and when

Inputs: local player health h, max health M (classes.md), the overheal
cap C (the class's maximum buffed health: 1.5 M rounded down to a
multiple of 5, never below M or the current h), alive. The panel
re-reads these every 0.05 s.

- **Number**: h as a plain integer, centred in its box, while h > 0.
  At h ≤ 0 the number and the max-health label are blank.
- **Fill**: the off-white cross (`health_color`) is drawn from height
  (1 − f)·H down to the bottom of its box, with the texture cropped the
  same way, f = min(h / M, 1): it empties from the top down as the
  player is hurt, the lower part staying. It is drawn white (255 255 255)
  normally and **tinted `HUDDeathWarning` (255 0 0) in the low state**.
  At h ≤ 0 it is replaced by the whole `health_dead` picture in white.
- **Background cross** (`PlayerStatusHealthImageBG`): shown while h > 0,
  hidden at h ≤ 0.
- **Max health** label: shows M above the cross only while h > 0 and
  M − h ≥ 5; otherwise blank.
- **Overheal** (h > M): `PlayerStatusHealthBonusImage` is shown, drawn
  white, and grows about its centre: each side moves out by
  a = round(min((h − M) / (C − M), 1) × 35) **pixels** (the 35 is a
  plain pixel count, not a layout unit; the file sets 35, the built-in
  default is 25), so its size is the base size + 2a. On entering this
  state `HudHealthDyingPulseStop` and `HudHealthBonusPulse` run.
- **Low health**: when h < 0.49 M (strict) and h > 0, the same bonus
  image is shown, drawn `HUDDeathWarning` (255 0 0), and grows by
  a = round((0.49 M − h) / (0.49 M) × 35) px per side: the lower the
  health, the larger the red cross. On entering this state
  `HudHealthBonusPulseStop` and `HudHealthDyingPulse` run. The health
  number's colour does not change.
- **Otherwise** (0.49 M ≤ h ≤ M, or dead): the bonus image is put back
  to its base bounds and hidden, and both pulses are stopped.
- Low and overheal can't both hold. The pulse events only animate the
  image's alpha; its colour and size are set by the panel.

| Class | M | Low below (0.49 M) | Cap C (1.5 M, rounded down to 5) |
|---|---|---|---|
| Scout, Engineer, Sniper, Spy | 125 | 61.25 → low at ≤ 61 | 185 |
| Soldier | 200 | 98 → low at ≤ 97 | 300 |
| Pyro, Demoman | 175 | 85.75 → low at ≤ 85 | 260 |
| Heavy | 300 | 147 → low at ≤ 146 | 450 |
| Medic | 150 | 73.5 → low at ≤ 73 | 225 |

#### 3.4 Crosshair (`HudCrosshair`)

- Full-screen panel; the crosshair is drawn centred on the screen.
- `cl_crosshair_file` "" (default): each weapon's own crosshair, named in
  its weapon script (`TextureData`; a script's `DrawCrosshair` 0 hides
  it). The scripts are encrypted (install.md Part 6), so which picture
  each of the four guns uses is not in the plain data (Open questions
  2). It is drawn centred, at the picture's own texel size × (scale /
  32) × the weapon's crosshair scale (1 for the stock guns), in screen
  **pixels** (not layout units): a 32 × 32 cell is 32 × 32 px at any
  resolution with the default scale. The pictures are cells of the sprite sheet
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
  dot in brackets. This picture is drawn as a square of side
  2 × round(32 × (scale / 32) × weapon scale) px centred on the screen:
  64 × 64 px with the defaults (twice the weapon pictures' size).
- Colour: (`cl_crosshair_red`, `green`, `blue`, 255), defaults 200 200
  200; size: `cl_crosshair_scale`, default 32. The picture is drawn with
  vertex colour = that colour.
- Hidden when the player is dead, while taunting, as a Halloween ghost,
  during a competitive round-start countdown of 10 s or less, and while
  a scope or the scoreboard covers it (Sniper; not these guns).

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
- **Low ammo** (checked every 0.1 s): let n = loaded + reserve for a
  clip gun, or the ammo count for a no-clip gun, and F = the class's
  maximum ammo for that pool + the clip size (clip guns only). The gun
  is low when n < round(0.40 × F). On becoming low the image
  `HudWeaponLowAmmoImage` is put at its base bounds, shown, given the
  foreground colour 255 0 0, and `HudLowAmmoPulse` starts; while low it
  grows about its centre by a = round((0.40 F − n) / (0.40 F) × 5) px
  per side (pixels, not units). When not low it goes back to its base
  bounds, is hidden, and `HudLowAmmoPulseStop` runs. Because it sits
  **under** the background (zpos 0 vs 1), what shows is its rim around
  the panel, up to 5 px wide, pulsing. Whether the red foreground colour
  tints the picture is not settled (Open questions 9).
- The panel is hidden while dead, for weapons that use no ammo (melee),
  for the Medi Gun, for weapons whose ammo is metal, and while the match
  summary shows.

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
  killer, assister, victim, weapon kill-icon name, crit flag). A row
  lives `hud_deathnotice_time` (6) seconds, or **twice that (12 s)** when
  the local player is the killer, assister or victim. Expired rows are
  removed first (a row goes when the time is strictly past its end).
  Then, while there are more than 4 rows, the oldest rows **not**
  involving you are removed first (never the newest row); if still more
  than 4, the oldest rows go. `hud_deathnotice_time` 0 turns the feed
  off.
- **Row layout**: row i (0 = oldest) at y = start + i × (16 + 4) units
  (`LineHeight`, `LineSpacing`, both scaled; the start height is the
  viewport's), right-justified to the panel's right edge (screen r12).
  Left to right: a margin of 10 width-units (640-wide scale), killer
  name, a space, the icon, a space, victim name, a space, the margin.
  The rounded box (radius 3 units) spans the row's full width from 1 px
  below its top to 1 px above its bottom, in `BaseBackgroundColor` (or
  `LocalBackgroundColor`).
- **Names** in `Default`, each in its team's solid colour (RED 180 92 77,
  BLU 104 124 155).
- **Icon**: the HUD texture named `d_` + the kill-icon name (from
  `mod_textures.txt`, drawn from `materials/hud/d_images`, 512 × 512) for
  normal rows. `IconScale` is **not read**: the icon is scaled to the row
  height minus 2 units (height-scaled), keeping its aspect: a 96 × 32
  cell is 14 units tall and 42 wide (about 31 × 94 px at 1080),
  vertically centred in the row, tinted `HudWhite`. An unknown name
  falls back to `d_skull_tf`. Entries for the four
  guns (x, y, w, h in texels of `d_images` / `dneg_images`):

| Kill-icon name | Cell | Used for |
|---|---|---|
| `scattergun` | 96, 192, 96, 32 | Scattergun |
| `tf_projectile_rocket` | 0, 224, 96, 32 | Rocket Launcher (the rocket names the kill) |
| `shotgun_primary`, `shotgun_soldier`, `shotgun_pyro`, `shotgun_hwg` | 0, 256, 96, 32 (all four) | Shotgun by class |
| `minigun` | 0, 384, 96, 32 | Minigun |
| `skull` | 96, 288, 96, 32 | the fallback is `d_skull_tf` (SDK); which cell that names is in `mod_textures.txt` |
| `crit` | 192, 409, 64, 34 | drawn behind the icon on crit kills |

- **Local player involved** (killer, assister or victim is you): the
  row background is `LocalBackgroundColor` (light tan 245 229 196 200)
  and the icon is the `dneg_` + name texture (same cell of
  `materials/hud/dneg_images`, a dark "negative" version), falling back
  to the `d_` one if there is none. Names keep their team colours;
  other text on the row (e.g. "fell to a clumsy, painful death") is
  `LocalPlayerColor` (`HudBlack` 65 65 65) on your rows and white on
  others. A player on no team is drawn in `LocalPlayerColor` on your
  rows, white otherwise.
- **Crit kill** (the event's damage bits include critical): the `d_crit`
  (or `dneg_crit`) texture is drawn first at exactly the weapon icon's
  position and size, then the icon over it.
- **Assist**: the killer's name becomes "killer + assister" (one string,
  " + " between, in the killer's team colour); the row counts as yours
  if you are the assister.
- **Suicide or world kill** (no killer, or killer = victim): the killer
  name is blank; a fall death adds the localised fall text after the
  icon.
- Both texture materials are `UnlitGeneric`, translucent, vertex colour
  and vertex alpha, no depth test, **not** additive.

#### 3.7 Damage direction

`HudDamageIndicator` has an empty entry in `hudlayout.res`, so its
built-in defaults apply (SDK). It draws, for each damage message the
server sends the local player (with the "damage taken" bit set), one
quad of the material `vgui/damageindicator`, white, additive, around
the screen centre:

- Size by damage d (clamped to 0 … 100): width 10 + 90·d/100 units and
  height 20 + 80·d/100 units (scaled), each half cut to whole pixels.
- Direction: the unit vector from the view to the damage origin (the
  view itself if the origin is zero), plus a random ±0.1 on each axis,
  normalised again, then flattened onto the view's horizontal forward
  and right axes. The quad sits on a circle around the screen centre,
  rotated to face outward toward that direction (straight ahead = top).
- Radius: from 140 units at the start to 120 units at 0.1 s, then held.
- Life: 1 s at no damage to 2 s at 100 or more (linear). Alpha 1 for the
  first 70 % of the life, then linear to 0 at the end.

Chimera may draw this with its own arc picture of the same shape.

#### 3.8 Damage numbers and the health-gain number

- **Damage numbers** (`CDamageAccountPanel`, `resource/ui/huddamageaccount.res`):
  on each damage event (the server's "player hurt") with damage N > 0,
  while you are alive, where the attacker is you (or, for a Medic, your
  heal target), the victim is not the attacker, and the victim is not
  an enemy Spy who is cloaked or disguised as your team. If
  `hud_combattext` is 1 (default) and a line from the victim's centre to
  your view hits no world brush, "−N" spawns in
  `hud_combattext_red/green/blue` (255 0 0). It lives in the world: at
  the victim's origin raised by the victim's hull height plus 1 … 16
  units (1 when the victim is at your position, 16 at 200 units or more
  away, linear in squared distance), and rises 32 world units over its
  life, projected to the screen each frame. Life `delta_lifetime` (1.5
  s). Alpha, with p = the fraction of life gone: 255 while p ≤ 0.5;
  after that the game computes 255 × (0.5 − p) / 0.5 (negative), cut to
  an integer, and stores it in an 8-bit channel, which wraps: the
  visible alpha is 256 + that value, so it falls from 255 just after
  the half to 1 at the end (and is 0 at exactly p = 0.5). Font
  `HudFontSmall`; the big font `HudFontMedium` when the hit was a crit
  or a mini-crit. With `hud_combattext_batching` 1, a number from the
  same victim within the window (0.2 s) of the previous one is merged
  into it. One damage event per victim per shot, so a pellet shot gives
  one number (the pellets' sum).
- **Health gain** (`CHealthAccountPanel`, `hudhealthaccount.res`): on the
  server's heal-on-hit / pickup heal event for the local player, "+N"
  appears at x 13, y 50 in the panel (screen (89, r102) units) and
  drifts to y 0 over 1.5 s, in `PositiveColor` 0 255 0 255, font
  `HudFontMedium`. It is driven by that event, not by any rise of
  health, and losses are not shown here.

#### 3.9 Hit sound

Triggered by the same damage events and filters as the damage numbers
(attacker you, N > 0, not yourself, not a hidden enemy Spy; no line of
sight needed). On each one, if `tf_dingalingaling` or
`tf_dingalingaling_lasthit` is 1 and either
`tf_dingalingaling_repeat_delay` is 0 or more than that delay has passed
since the last one:

- **Kill sound**: if the hit left the victim at 0 or less health (or was
  a Dead Ringer feign) and `tf_dingalingaling_lasthit` is 1, play
  `Player.KillSoundDefaultDing` (or the `tf_dingalingaling_last_effect`
  choice: `…Electro`, `…Notes`, `…Percussion`, `…Retro`, `…Space`,
  `…Beepo`, `…Vortex`, `…Squasher`) at `tf_dingaling_lasthit_volume`;
  the repeat delay never blocks it.
- **Hit sound** otherwise, if `tf_dingalingaling` is 1:
  `Player.HitSoundDefaultDing` (`)ui/hitsound.wav`, CHAN_STATIC, no
  attenuation (SNDLVL_NONE)) or the `tf_dingalingaling_effect` choice
  (1–8: `Player.HitSoundElectro`, `…Notes`, `…Percussion`, `…Retro`,
  `…Space`, `…Beepo`, `…Vortex`, `…Squasher`, files `ui/hitsound_*.wav`)
  at `tf_dingaling_volume` 0.75.
- **Pitch**: an override cvar above 0 wins; else
  pitch = min + (max − min) × clamp((N − 10) / (150 − 10), 0, 1), with
  min and max the `…pitchmindmg` / `…pitchmaxdmg` cvars (both 100).
- The sound plays only for the local player. Both are off by default.

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
5. Expire death-notice rows (then trim to 4), damage numbers,
   health-gain numbers and damage-indicator arcs.
6. Draw by zpos: health panel, class portrait, ammo panel, crosshair,
   damage numbers, damage indicator, kill feed.

The health panel does steps 1–2 only every 0.05 s and the ammo panel
step 3 every 0.1 s; animations and drawing run every frame.

## Edge cases

- Overheal state is decided on h > M, not on the cap: a Heavy at 301
  pulses as much as one at 450.
- A class change changes M at once: a Heavy (300) at 100 health is not
  low (147 threshold); the same 100 health as a Scout (125) is not low
  either (61), but 60 is.
- Minigun: the ammo panel never shows a reserve; spinning without
  firing does not change the number (weapons.md 7.1).
- Rocket Launcher with 0 loaded and 20 in reserve shows "0" big and "20"
  small; not low (20 ≥ round(0.40 × (20 + 4)) = 10). Low at 9 or fewer
  rockets in all.
- Minigun (200 maximum, no clip): low below 80.
- Kill by the world (fall) or a suicide by your own rocket: the killer
  name is blank, the victim's name with the weapon's icon (the rocket's
  for a rocket suicide); a fall adds the fall text.
- A sixth kill not involving you while four rows involve you: the
  newest row stays and the oldest of your rows is removed only if no
  other row can go.

## Quirks

- `crosshair_default` in `mod_textures.txt` points at an empty cell of
  the sprite sheet; keep it as data, never draw it as "the" default.
- The low-ammo image is below its own background (zpos 0 < 1); only its
  growth (up to 5 px per side) makes it show.
- The overheal growth is in raw pixels, so the bonus cross grows by the
  same 35 px at every resolution (relatively more at 720p).
- The custom-file crosshair is drawn twice the size of the weapon
  crosshair at the same `cl_crosshair_scale`.
- `IconScale` in the kill feed's settings has no effect.
- The damage and health numbers' fade is an 8-bit wrap of a negative
  alpha (3.8); it looks like a linear fade over the second half, with a
  single fully transparent instant at the half. Keep the look (a linear
  fade from 255 to 0 over the second half is indistinguishable).
- The overheal and low-health effects share one image control
  (`PlayerStatusHealthBonusImage`), only recoloured.

## Measured in the running game: the Sniper's scope (tools/tf2ref, 2026-11-20)

`scope.tf2`: a RED Sniper on 2fort's bridge looking down it at
1024x768, the HUD on (the team and class joined and the menus closed
before `cl_drawhud 1`, `hud_classautokill 0`, so no class menu covers
it), attack2 tapped, pictures 0.5, 1.8 and 4 s after. Facts of the
picture and of the install's files, not of the code.

- **The overlay**: a round lens filling the screen's height between
  black bars; its edge soft over about 20 pixels with an orange and
  olive fringe, the view inside a little darker and warped near the rim.
  Its material is a refracting one (`hud/scope_sniper_ul` and its
  mirrored siblings, one per quarter, the tint texture 512 x 512 per
  quarter); the plain fallback each quarter names, `hud/scope_sniper_ul_dx80`
  (512 x 512, black with alpha: opaque outside the ring, about 0.2
  inside), is that ring without the warp and tint. Thin dark cross hairs
  run through the middle. Drawn at 768 lines a quarter's 512 texels shrink
  to 384 pixels: soft, never blocky.
- **The crosshair**: TF2 still draws the rifle's own thin cross (white,
  wider than tall, about 30 x 13 pixels) at the middle under the scope,
  with the laser dot's red glow on it. 3.4 says a scope hides it; the
  capture shows it (Open questions).
- **The charge meter**: `hud/sniperscope_numbers2` (128 x 256 texels; a
  wedge of 13 thin vertical stripes at texels 6-68 across, its top at
  row 128, growing taller to the right; a lightning bolt at 75-89 and
  "100%" at 95-123), drawn additively at half a layout unit a texel, its
  top left corner 64 units right of the screen's middle and 64 up, so the
  wedge starts just right of the middle at its height. The stripes light
  from the left as the charge grows (0.5 s: the first fifth or so; 1.8 s:
  about two thirds; 4 s: all of them); the unlit ones show faintly, about
  a fifth as bright; the bolt and "100%" show only at a full charge. The
  material scrolls a mask (`hud/sniperscope_numbers`) by the charge; how
  exactly is not measured (the lit edge at a given charge is within about
  10 texels of the charge's share of the wedge).

## Test cases

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | 1920 × 1080 | – | – | u = 2.25; ammo panel at (1707, 957) 211 × 101 px; health panel (0, 810) 562 × 270; kill feed panel (480, 40) |
| 2 | 1280 × 720 | – | – | u = 1.5; ammo panel x = 1280 − 142 = 1138, y = 720 − 82 = 638, 141 × 67 |
| 3 | Heavy at 150 / 300 | – | – | health text "150", max label "300"; fill 0.5, white; not low (150 ≥ 147); no overheal; no pulse |
| 4 | Heavy at 146 / 300 | – | – | low on: bonus image and fill red (255 0 0), bonus grows round(1/147 × 35) = 0 px; `HudHealthDyingPulse` running |
| 4b | Heavy at 1 / 300 | – | – | low; bonus grows round(146/147 × 35) = 35 px per side |
| 5 | Heavy at 147 / 300 | – | – | low off |
| 5b | Scout at 121 / 125 | – | – | max label blank (125 − 121 < 5); at 120 it shows "125" |
| 5c | dead (h 0) | – | – | number and max blank, `health_dead` drawn, background cross hidden |
| 6 | Scout at 185 / 125 | – | – | overheal on, white bonus image grown 35 px per side (base 55 units + 70 px), `HudHealthBonusPulse` running; text "185" |
| 6b | Scout at 155 / 125 | – | – | bonus grows round(30/60 × 35) = 18 px per side |
| 7 | Scout overhealed, decays to 125 | – | – | `HudHealthBonusPulseStop` when h ≤ 125; bonus image hidden |
| 8 | bonus pulse | start at t = 0 | 0.2 / 0.4 / 0.6 s | image alpha 255 / 127 / 255 |
| 9 | dying pulse | start at t = 0 | 0.075 / 0.1 / 0.2 / 0.25 s | alpha 255 / 255 / 0 / restarts |
| 10 | Scout with Scattergun 6/32, RED | – | – | big "6" in TF2 Build 99 px (1080p), small "32" in TF2 40 px; red background image; not low (38 ≥ round(0.4 × 38) = 15) |
| 10b | Scout with Scattergun 2/12 | – | – | 14 < 15: low; the low image grows round((15.2 − 14) / 15.2 × 5) = 0 px; `HudLowAmmoPulse` running |
| 11 | Heavy with Minigun, 200 ammo, BLU | – | – | only `AmmoNoClip` "200"; reserve labels hidden; blue background |
| 12 | Soldier kills a BLU Scout with a rocket | – | – | row: "Soldier-name" (180 92 77), `tf_projectile_rocket` icon from `d_images` (0, 224, 96, 32), "Scout-name" (104 124 155) on 46 43 42 220 |
| 13 | as 12, the local player is the victim | – | – | row background 245 229 196 200; icon from `dneg_images` same cell |
| 14 | as 12, crit | – | – | `d_crit` cell (192, 409, 64, 34) drawn at the icon's rectangle, under it |
| 15 | 5 kills within 6 s, none involving you | – | – | 4 rows; the first is gone |
| 15b | you kill someone at t = 0 | – | 6.5 s / 12.5 s | row still shown / gone |
| 16 | Engineer shotgun kill, Heavy shotgun kill | – | – | icon names `shotgun_primary`, `shotgun_hwg`; both the same cell (0, 256) |
| 17 | hit for 54 with `hud_combattext` 1 | – | 0 / 0.375 / 1.125 / 1.5 s | "−54" in 255 0 0 above the victim, alpha 255 / 255 / 129 (256 − 127) / gone; risen 0 / 8 / 24 / – world units |
| 17b | crit hit for 150 | – | – | "−150" in `HudFontMedium` |
| 17c | hit an enemy behind a wall (no clear brush line) | – | – | no number; hit sound still plays if on |
| 18 | `tf_dingalingaling` 1, a hit | – | – | `Player.HitSoundDefaultDing` at volume 0.75, pitch 100 |
| 18b | pitch min 50, max 150; hits of 5, 80, 200 | – | – | pitch 50, 100, 150 |
| 18c | `tf_dingalingaling_lasthit` 1, the killing hit | – | – | `Player.KillSoundDefaultDing` only |
| 21 | damage 50 from straight behind | – | 0 / 0.1 / 1.5 s | an arc 55 × 60 units below the centre at radius 140 → 120 units; alpha 1 until 1.05 s, 0 at 1.5 s |
| 19 | Heavy healed from 200 to 260 by a kit | – | 0 / 1.5 s | "+60" (green) at panel y 50 → 0, gone at 1.5 s |
| 20 | font sizes at 1440 lines | – | – | `HudClassHealth` 48 px; `Default` 9 × 3 = 27 px (scaled entry, antialias) |

## Open questions

1. **`_lodef` / `_hidef` keys** (`tall_hidef` 22 for `HudClassHealth`,
   `HudFontGiantBold` `tall_lodef` 52, ammo label variants): when do they
   apply on PC? Assumed never (console leftovers). Check: screenshot a
   1080p and a 720p client and measure the health digits.
2. **Per-weapon crosshairs.** Which sprite-sheet cell each stock gun
   uses (in the encrypted weapon scripts' `TextureData`). The cvar
   defaults are settled (200 / 200 / 200, scale 32). Check: a centred
   screenshot per weapon.
3. ~~`hud_combattext` default~~ Settled: 1 (the `.scr` list is the
   dialog's, not the default); `tf_dingalingaling_effect` 0.
4. ~~`hud_deathnotice_time`~~ Settled: 6 s, 12 s for your rows.
5. ~~Dead state~~ Settled (3.3).
6. ~~Max-health label~~ Settled: alive and M − h ≥ 5.
7. ~~Overheal growth~~ Settled: linear in (h − M) / (C − M), up to 35 px
   per side, centred.
8. ~~Low-health comparison~~ Settled: strict <; the fill is tinted red,
   the number is not.
9. **Low ammo**: threshold settled (3.5). Open: whether the image's red
   foreground colour tints the team picture or leaves it as drawn; check
   with a screenshot at 1 rocket.
10. ~~Kill-feed details~~ Settled (3.6), except the localised fall text's
    wording (a localisation string, not in the code).
11. ~~Damage direction indicator~~ Settled (3.7); open only what
    `vgui/damageindicator` looks like (a material in the install).
12. ~~Damage numbers~~ Settled (3.8).

13. **The crosshair behind a scope** (measured above): 3.4 hides it while
   a scope covers it, but real TF2 draws the Sniper Rifle's thin cross
   under the scope. Ours hides it behind every game's scope (the
   project's choice: a scope draws its own cross hairs).
- **The scope at other aspects**: measured only at 4:3 (1024x768), where
  the four quarters fill the whole screen (the lens edge sits at about
  0.29 of each quarter's width, as in the texture). Whether a wider screen
  stretches them the same way (a wide ellipse) or keeps the lens round is
  not measured (the headless display stays 4:3); ours stretches them.
