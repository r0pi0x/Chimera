# Half-Life 2: Deathmatch: HUD, weapon selection, death notices, scoreboard and MOTD (H8)

Source basis: the public Source SDK 2013 multiplayer game code — the
shared client HUD framework (element visibility, HUD textures, the
numeric panel, crosshair, pickup history, weapon-selection base, chat
base, scoreboard and text-window bases), the Half-Life 2 client HUD
elements (health, suit, ammo and secondary ammo, aux power, flashlight,
damage indicator, quick info, suit zoom, weapon selection), the HL2:DM
client (death notices, target ID, team line, chat, scoreboard, text window
and its background), the VGUI panel and animation-controller code, and the
server-side senders of the HUD's messages (player and HL2 player client
data, battery and health pickups, ammo denial) — read in place in a
separate spec session. From the game's files on this box (game build
24600434, dedicated server build 24600426, `steam.inf` PatchVersion
10889068 in both, checked): `scripts/hudlayout.res`,
`scripts/hudanimations.txt`, `scripts/mod_textures.txt`,
`resource/clientscheme.res`, `resource/ui/scoreboard.res`,
`resource/ui/textwindow.res`, `resource/ui/motd.res` and the weapon
scripts in `hl2mp/hl2mp_pak_dir.vpk`; Half-Life 2's loose
`scripts/hud_textures.txt`, `resource/ui/basechat.res`,
`resource/valve_english.txt` and `resource/hl2_english.txt`;
`hl2mp/resource/hl2mp_english.txt`; the font files' family names and
character sets (`hl2mp/resource/halflife2.ttf`, `hl2mp.ttf`,
`hl2/resource/hl2crosshairs.ttf`, `hl2/resource/halflife2.ttf`, the five
GorDIN files in `hl2_complete/resource/`), all read for numbers and names
only; the game's client library was searched for HUD element and cvar
names only. Nothing was launched.
Status: draft

## Summary

HL2:DM's HUD is Half-Life 2's: translucent rounded boxes in the
bottom corners with a dim-yellow label and large HalfLife2-font digits
that glow (the "blur") and flash when they change, all driven by the
named events of `scripts/hudanimations.txt`; an aux power bar that
appears only while aux power is below 100 and grows a line per active
suit device; HL2's crosshair brackets (quick info) showing health and
clip; Half-Life 2's horizontal weapon selection (a row of six small
numbered boxes, the chosen bucket opened into a column of large icon
boxes, the chosen weapon's name under it); a pickup history on the right;
death notices in the top right with HL2MP font icons; red screen-edge
damage flashes. HL2:DM adds the death notices, target ID, team line,
chat colours, a rounded-corner scoreboard with Deathmatch or team
sections, and the MOTD window. There is no team menu, no flashlight
indicator and no posture or squad panel in play.

This spec continues [source_deltas.md](source_deltas.md) (H0: "HUD
resources", "Weapon scripts" icon table, "Default cvars"),
[weapons.md](weapons.md) (H3: selection commands and their server side),
[suit.md](suit.md) (H2: aux power, devices, suit zoom),
[rules.md](rules.md) (H6: death-notice weapon names, chat formats and
colours, intermission scoreboard, MOTD at join) and
[projectiles.md](projectiles.md) (H4: death-notice names). No Source HUD
spec exists yet: the shared Source mechanisms (proportional layout,
scheme lookup, animation scripts) are stated here once, in sections 1–3,
and hold for CS:S's HUD as well (`specs/cs_source/objectives.md` uses the
same 640 × 480 convention). Combat Arms' HUD specs do not apply.

## Units and conventions

- **HUD units.** Positions and sizes are in the 640 × 480 proportional
  space of `hudlayout.res`. One unit is u = H / 480 screen pixels (H =
  screen height) on **both** axes; the virtual screen is W / u units wide
  (853.33 at 16:9). `rN` means "N units from the right edge" (x = W −
  scaled(N)), `cN` "N units from the centre" (x = W/2 + scaled(N)).
  Scaled values become whole pixels by truncation (as Chimera's CS:S HUD
  already does); values that are kept as fractions are said.
- Pixels: screen pixels, origin top-left, y down.
- Time: client time in seconds (the client's current time, advanced
  every rendered frame; not tick-quantised unless said).
- Colours: R G B A, 0–255. Scheme colour names (`FgColor`, `BrightFg`,
  ...) are those of `hl2mp`'s `resource/clientscheme.res`, section 2.
- Conditional keys in the layout and scheme files: `[$WIN32]` is true on
  every PC platform (Linux included), `[$X360]` and `[$OSX]` false,
  `[$DECK]` true only on a Steam Deck (Open questions 2). This spec gives
  the PC, non-Deck values; Deck values are noted where a test may need
  them.
- Builds read: game 24600434, dedicated server 24600426.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| hud_unit | H / 480 | px | one layout unit |
| health_low | 20 | hp | below: `HealthLow` pulse; at or above on any change: `HealthIncreasedAbove20` |
| armour_low | 20 | armour | below, on a decrease: `SuitArmorLow` |
| quickinfo_health_warn | 25 | hp | at or below: red bracket and `HUDQuickInfo.LowHealth` |
| quickinfo_clip_warn | 0.25 | of max clip | at or below (max clip > 1): red bracket and `HUDQuickInfo.LowAmmo` |
| quickinfo_idle | 1.0 | s | after this without a health or clip change the brackets dim |
| quickinfo_dim_alpha | 64 | alpha | dimmed panel alpha, reached over 2.0 s |
| quickinfo_full_alpha | 255 | alpha | restored over 0.5 s |
| quickinfo_zoom_fade | 0.25 | s | panel alpha to 0 while suit-zoomed |
| quickinfo_bracket_alpha | 138 | alpha | normal bracket colour alpha |
| aux_chunks | 10 | chunks | BarWidth 92 / (BarChunkWidth 6 + BarChunkGap 3), floored |
| aux_low_chunks | 2 | chunks | lit chunks ≤ this (10 / 4 floored) = low (red) |
| dmg_high | 25 | hp | health damage above this in one message = "high" flash |
| `hud_deathnotice_time` | 6 | s | death notice life; 0 disables new notices |
| death_notice_max | 4 | lines | `MaxDeathNotices` |
| `hud_drawhistory_time` | 5 | s | pickup history entry life (ammo denied: half) |
| history_alpha_rate | 80 | alpha / s | entry alpha = min(255, 80 × seconds left) |
| `hud_fastswitch` | 0 | – | 0 buckets, 1 fast switch, 2 and 3 console styles |
| `hud_showemptyweaponslots` | 1 | – | empty boxes for missing positions inside the open bucket |
| selection_timeout | 0.5 | s | buckets: idle time before the fade-out event |
| selection_close | 1.25 | s | buckets: idle time at which the selection closes |
| fastswitch_timeout | 1.5 + 1.5 | s | fast switch: display time + fade (nothing is drawn, 5.3) |
| `hud_quickinfo` | 1 | – | 0 hides the brackets but keeps the centre sprite |
| `crosshair` | 1 | – | 0 hides crosshair and quick info |
| `cl_observercrosshair` | 1 | – | crosshair while roaming as an observer |
| `hud_centerid` | 1 | – | target ID centred (1) or bottom-left (0) |
| `hud_showtargetid` | 1 | – | registered, never read (Quirks) |
| zoom_fade | 0.4 | s | suit zoom overlay in and out |
| target_id_range | 1500 | in | trace length from eye + 10 |
| team_colour Unassigned / Spectator / Combine / Rebels | 255 178 0 / 204 204 204 / 153 204 255 / 255 64 64 | RGB | death notices, scoreboard, target ID, team line |
| chat_colour Rebels | 255 63 63 | RGB | chat only (one off from the team colour; others equal) |
| death_icon_colour | 255 80 0 255 | RGBA | every death-notice icon |
| scoreboard_refresh | 1.0 | s | while shown |

## Corrections to H0

1. **HUD resources.** H0 lists the custom font files; the scheme also
   names `resource/GorDIN-Regular.ttf`, `-Bold`, `-Semibold`, `-Black` and
   `-Light` (entries 11–15). They resolve, ignoring case, to the lowercase
   files in `hl2_complete/resource/` (search path 16). `HL2crosshairs.ttf`
   (family `HL2cross`) is **not** in `hl2mp/`: it resolves to Half-Life
   2's loose `hl2/resource/hl2crosshairs.ttf`. `hl2mp/resource/halflife2.ttf`
   wins over Half-Life 2's file of the same name (path 13/14 before 17)
   and is a reduced copy: it has no characters 0x5C, 0xA9 or 0xAE (Half-
   Life 2's has them), so HL2's flashlight glyphs do not exist in HL2:DM.
2. **`hl2_complete/resource/`'s HUD files are not used.** Its
   `hudlayout.res`, `hudanimations.txt` and `hud_textures.txt` sit under
   `resource/`, but the client reads `scripts/hudlayout.res`,
   `scripts/hudanimations.txt` (there is no `scripts/hudanimations_manifest.txt`
   anywhere on the path), `scripts/hud_textures.txt` and
   `scripts/mod_textures.txt`; its `resource/clientscheme.res` is shadowed
   by `hl2mp`'s VPK copy. Only its GorDIN TTFs matter.
3. **Weapon script `autoaim` icons** differ per weapon (H0 gave one
   rectangle for all): `sprites/crosshairs` at (0, 48) for pistol and
   stunstick, (0, 96) for the crossbow, (48, 72) for crowbar, S.L.A.M. and
   gravity gun (all 24 × 24); the crossbow's `zoom_autoaim` is (24, 112)
   104 × 16. None is drawn in HL2:DM (6.3).

## Corrections to wave 1/2

1. **suit.md, "Flashlight": "The HUD's flashlight element is hidden
   while `mp_flashlight` is 0."** HL2:DM has no flashlight HUD element at
   all. The server does set a "hide flashlight" HUD bit from
   `mp_flashlight`, but no element in HL2:DM's client reads it; HL2's
   flashlight panel exists only in Episode builds (its `HudFlashlight`
   entry in `hudlayout.res` is unused), and HL2:DM's HalfLife2 font has
   no flashlight glyphs (Correction to H0 1). The flashlight's only HUD
   trace would be the aux panel's FLASHLIGHT line, which never shows in
   HL2:DM (suit.md: no device). Nothing to draw.
2. **weapons.md, "Selection commands", `hud_fastswitch` 1:** "if there is
   none different, `Player.DenyWeaponSelection`" — only when the bucket
   holds **no** selectable weapon. If the only selectable weapon in it is
   the one in hand, nothing happens and no sound plays (5.4).
3. **weapons.md:** with `hud_fastswitch` 0, a `slotN` that finds no
   selectable weapon in the bucket opens nothing but still plays
   `Player.WeaponSelectionMoveSlot` (5.4). With any setting, a confirmed
   selection plays `Player.WeaponSelected` and then
   `Player.WeaponSelectionClose` (5.5).

## Behavior

### 1. Layout space, element visibility and the reset at spawn (shared Source)

- Every element is a panel on the full-screen HUD viewport, placed by its
  `hudlayout.res` entry (`xpos`, `ypos`, `wide`, `tall` in units, rule
  above). Element-specific keys (`text_xpos`, `digit_xpos`, `BarInsetX`,
  ...) are also units, kept as fractions until drawn.
- `PaintBackgroundType` 2 = the rounded box: the panel's background
  colour filled with rounded corners of c = max(⌊scaled(8)⌋ / 2, 8)
  pixels (c = 8 up to 1079 lines, 9 at 1080, never under 8),
  drawn with the corner textures Chimera's CS:S HUD already uses.
- **Hidden rules.** Every element is hidden when the whole HUD is hidden
  (no local player, not in game, or the server's hide-all bit) and, per
  element: "dead" = hidden while the local player's health ≤ 0 and not
  alive; "health" = hidden when the server hides health (it does so for
  observers); "selection" = hidden when the server hides weapon selection
  (while carrying a prop with `+use`, gravity_gun.md, and in vehicles);
  "misc" = hidden with miscellaneous status; "crosshair" = hidden when
  the server hides the crosshair. HL2:DM's player spawn clears all hide
  bits. Per element:

| Element | Hidden when |
|---|---|
| `HudHealth` | health hidden, dead |
| `HudSuit` | health hidden (**not** when dead: it stays while armour > 0) |
| `HudAmmo` | health hidden, dead, selection hidden |
| `HudAmmoSecondary` | health hidden, dead, selection hidden |
| `HudSuitPower` | health hidden, dead |
| `HudDamageIndicator` | health hidden (not when dead) |
| `HudZoom` | health hidden, dead |
| `HudWeaponSelection` | selection hidden, dead, in a vehicle |
| `HudHistoryResource` | misc hidden |
| `HudDeathNotice` | misc hidden |
| `TargetID` | misc hidden |
| `HUDQuickInfo` | crosshair hidden |
| `HudCrosshair` | crosshair hidden, dead (with the exceptions in 6.3) |

- **Spawn reset.** Every spawn sends the client a HUD reset (before the
  spawn's entity data in the same update). The reset (a) cancels every
  running animation and queued animation event, (b) resets each element
  (stated per element below; e.g. health's remembered value becomes
  "none", the secondary ammo panel's alpha 0, the pickup history is
  emptied), then (c) runs every animation the resets started to its end
  value at once. Death notices are **not** cleared by it (only at a map
  load).

### 2. The client scheme (hl2mp's `resource/clientscheme.res`)

**Lookup.** A colour or font name is looked up in the scheme's
`BaseSettings`, then `Colors`; a value that is itself a name is resolved
again. `BaseSettings` lists several names twice; the **first** occurrence
is the one used (Open questions 1 gives the check). Resolved values:

| Name | Value | Used for |
|---|---|---|
| `FgColor`, `Panel.FgColor` | 255 220 0 100 | HUD labels, digits, unselected icons (later duplicates: `Orange`, `OrangeDim`) |
| `BgColor`, `Panel.BgColor` | 0 0 0 76 | HUD boxes (later duplicates: `TransparentBlack`, `blank`) |
| `BrightFg` | 255 220 0 255 | flashes |
| `DamagedBg` | 180 0 0 200 | low health box |
| `DamagedFg` | 180 0 0 230 | low health digits, empty ammo |
| `BrightDamagedFg` | 255 0 0 255 | flashes |
| `SelectionNumberFg`, `SelectionTextFg` | 255 220 0 255 | selection numbers and name (later: 200 alpha) |
| `SelectionEmptyBoxBg`, `SelectionBoxBg`, `SelectionSelectedBoxBg` | 0 0 0 80 each | selection boxes (later: selected 0 0 0 190) |
| `ZoomReticleColor` | 255 220 0 255 | suit zoom lines |
| `Normal` | 255 208 64 255 | crosshair, quick info, history |
| `Caution` | 255 48 0 255 | warnings, history "full"/no-ammo |
| `Yellowish` | 255 160 0 255 | – |
| `SectionedListPanel.BgColor` | 0 0 0 90 (`TransparentLightBlack`) | scoreboard background |
| `team0`, `team1`, `team2` | 204 204 204 / 255 64 64 / 153 204 255 | unused by HL2:DM's code (it uses the team colours of the Constants table) |

**Fonts.** A font has one or more numbered entries; the entry used is the
first, in file order, whose `yres` range contains the screen height, an
entry with no `yres` matching any height. Sizes of entries **with** `yres`
are in pixels; sizes of entries **without** it are 480-line sizes scaled
by H / 480 (Chimera's CS:S rule). `additive` fonts are blended additively.
The HUD's fonts (PC, non-Deck):

| Font | Family | Entries: `tall` (yres) | Weight | Flags | Used by |
|---|---|---|---|---|---|
| `Default` | GorDIN | 12 (1–599), 14 (600–767), 19 (768–1023), 21 (1024–1199), 10 scaled (other) | 600, 600, 700, 700, 700 | last entry additive | panel labels, aux text, team line |
| `HudNumbers` | HalfLife2 | 32 scaled | 0 | additive | main digits |
| `HudNumbersGlow` | HalfLife2 | 32 scaled | 0 | additive, blur 4, scanlines 2 | digit glow |
| `HudNumbersSmall` | HalfLife2 | 16 scaled | 1000 | additive | reserve count, history counts |
| `HudNumbersTimer` | GorDIN | 15 scaled | 0 | additive | death notice names |
| `WeaponIcons` | HalfLife2 | 64 scaled | 0 | additive | selection icons, history weapon and item icons |
| `WeaponIconsSelected` | HalfLife2 | 64 scaled | 0 | additive, blur 5, scanlines 2 | selected weapon glow |
| `WeaponIconsSmall` | HalfLife2 | 32 scaled | 0 | additive | ammo icons |
| `WeaponIconsSmallSecondary` | HalfLife2 | 25 scaled | 0 | additive | crossbow, RPG, SMG1-grenade ammo icons |
| `Crosshairs` | HalfLife2 | 40 (1–1599), 80 (1600–3199), 120 (3200–4799), 17 scaled (other) | 0 | additive; first entry not antialiased | crosshair `Q` |
| `QuickInfo` | HL2cross | 28 scaled | 0 | additive | brackets |
| `HudSelectionNumbers` | GorDIN | 11 scaled | 700 | additive | bucket numbers |
| `HudSelectionText` | GorDIN | 12 (1–599), 12 (600–767), 14 (768–1023), 17 (1024–1199), 7 scaled (other) | 600 (last 700) | additive | selected weapon name |
| `HL2MPTypeDeath` | HL2MP | 32 scaled | 0 | additive | death-notice icons |
| `TargetID` | GorDIN | 24 scaled | 0 | additive | target ID |
| `ChatFont` | Verdana | 12 (480–599), 13 (600–767), 14 (768–1023), 20 (1024–1199), 9 scaled (other) | 700 | drop shadow (first four) | chat |
| `CenterPrintText` | GorDIN | 18 scaled | 700 | additive | centre print |
| `HudHintTextLarge`, `HudHintTextSmall` | GorDIN | 15, 13 scaled | 700, 0 | additive | key hints |
| `ClientTitleFont` | HL2MP | 46 scaled | 0 | – | – |

So at 1080 lines `Default` is GorDIN 21 px bold and **not** additive,
while at 1440 lines it is the scaled entry, 30 px and additive. GorDIN's
weights map to its five files: 300 Light, 400 Regular, 600 Semibold, 700
Bold, 900 Black (weight 0 = Regular, 1000 = Black).

**Glyphs used (data of the game).** HalfLife2 (`hl2mp`'s file):
weapon letters `a`–`o` (H0's icon table), `*` battery, `+` health kit,
`Q` crosshair, digits for the numbers. HL2cross: `(` `)` brackets, `[`
`]` full, `{` `}` empty. HL2MP: death icons (4.3).

### 3. Animation scripts (`scripts/hudanimations.txt`; shared Source)

- An **event** is a list of commands. `Animate panel variable target
  interpolator start duration` adds one animation that begins `start`
  seconds after the event starts; `RunEvent name t`, `StopEvent name t`,
  `StopAnimation panel variable t` and `StopPanelAnimations panel t` are
  queued and run on the first frame at or after `t`.
- **Values.** Targets that are scheme colour names are resolved when the
  script loads. Positions accept `r`/`c` prefixes; positions and sizes are
  scaled units. Colours interpolate per channel; the result is truncated
  to whole numbers when applied.
- **Start value.** An animation takes as its start value whatever the
  variable holds on the first frame at or after its start time, so chained
  lines (`... 0.0 0.1`, then `... 0.1 0.75`) continue from where the
  previous one ended, and a zero-length line sets its value at once.
- **Interpolators**, with x = (now − start) / duration in [0, 1]:
  Linear f = x; Accel f = x²; Deaccel f = √x. Value = start + (target −
  start) · f; after the end the target is set and the animation removed.
- **Overlap.** Animations accumulate in a list in the order started; each
  frame they apply in list order, so when two touch the same variable
  the one started later wins while both run. Starting an event first
  removes every running animation and queued command that the **same**
  event started earlier (restart). `StopEvent` does the same for the named
  event. `StopAnimation` removes only the **first** running animation of
  that panel's variable that belongs to a different event.
  `StopPanelAnimations` removes all of the panel's animations.
- Panel names match ignoring case (`Hudammo` = `HudAmmo`). An event
  naming an element that does not exist does nothing; an event the code
  starts that the file doesn't define (`HintMessageLower`,
  `HintMessageRaise`, `WeaponPickup`) does nothing.

### 4. Health, suit, ammo and secondary ammo panels

#### 4.1 The numeric panel (shared by all four)

- Background: rounded box in `BgColor` (animatable `BgColor`).
- Label at (`text_xpos`, `text_ypos`) in `Default`, drawn in the panel's
  **foreground** colour (`FgColor` variable). The `TextColor` and
  `Ammo2Color` variables the events animate are never drawn (Quirks).
- Value at (`digit_xpos`, `digit_ypos`) in `HudNumbers`, foreground
  colour, as a plain integer with no padding: its left edge is always at
  `digit_xpos` (100 and 5 start at the same x).
- Glow: with `Blur` = b, the value is drawn again in `HudNumbersGlow` ⌊b⌋
  times at full foreground colour, plus once at alpha × (b − ⌊b⌋) when
  the fraction is non-zero. b = 0 draws no glow.
- Reserve (ammo panel only, when shown): at (`digit2_xpos`,
  `digit2_ypos`) in `HudNumbersSmall`, foreground colour.

#### 4.2 Layout

| Element | Pos | Size | Label (token) at | Digits at | Reserve at |
|---|---|---|---|---|---|
| `HudHealth` | 16, 432 | 102 × 36 | 8, 20 (`#Valve_Hud_HEALTH` "HEALTH") | 50, 2 | – |
| `HudSuit` | 140, 432 | 108 × 36 | 8, 20 (`#Valve_Hud_SUIT` "SUIT") | 50, 2 | – |
| `HudAmmo` | r150, 432 | 136 × 36 (animated, 4.5) | 8, 20 (`#Valve_Hud_AMMO` "AMMO") | 44, 2 | 98, 16 |
| `HudAmmoSecondary` | r76, 432 | 60 × 36 | 8, 22 (`#Valve_Hud_AMMO_ALT` "ALT") | 36, 2 | – |

Deck: health (16, 426) 130 × 42, digits (66, 0), label (8, 23); suit
(150, 426) 120 × 42; ammo r150 426, 152 × 42, label (8, 24), digits (55, 0),
reserve (100, 12); secondary r82 426, 70 × 42, label (8, 24), digits (42, 0).

#### 4.3 Health (`HudHealth`)

- Shows max(health, 0). Each frame the shown value is compared with the
  last one; on a change:
  - new value ≥ 20 → event `HealthIncreasedAbove20` (also when health
    **fell** but stays ≥ 20): stops `HealthLoop`, `HealthPulse`,
    `HealthLow`; `BgColor` → `BgColor` at once; `FgColor` → `FgColor`
    over 0.03 s; `Blur` 3 over 0.1 s, then 0 deaccel over 2.0 s.
  - 0 < new value < 20 → `HealthIncreasedBelow20` (`FgColor` → `BrightFg`
    0–0.25 s, → `FgColor` linear 0.3–1.05 s; `Blur` 3, then 0 over 2.0 s)
    **and** `HealthLow` (stops `HealthDamageTaken`, `HealthPulse`,
    `HealthLoop`; `BgColor` → `DamagedBg` 0–0.1 s, → `BgColor` deaccel
    0.1–1.85 s; `FgColor` → `BrightFg` 0–0.2 s, → `DamagedFg` linear
    0.2–1.4 s; `Blur` 5 0–0.1 s, → 3 deaccel 0.1–1.0 s; `RunEvent
    HealthPulse` at 1.0 s).
  - new value 0 → no event.
- `HealthPulse`: `Blur` 5 (0–0.1 s), → 2 deaccel (0.1–0.9 s); `BgColor` →
  100 0 0 80 (0–0.1 s), → `BgColor` deaccel (0.1–0.9 s); runs `HealthLoop`
  at 0.8 s, which runs `HealthPulse` again: the box pulses dark red every
  0.8 s and the digits stay `DamagedFg` until health changes to ≥ 20.
- Damage message (4.7) with health damage > 0 → `HealthDamageTaken`:
  `FgColor` → `BrightFg` 0–0.25 s, → `FgColor` 0.3–1.05 s; `Blur` 3, then
  0 over 2.0 s.
- Reset at spawn: remembered value "none", so the spawn's 100 runs
  `HealthIncreasedAbove20` (a 2 s glow at every spawn).

#### 4.4 Suit / armour (`HudSuit`)

- The server sends the armour value whenever it differs from the last
  value sent (checked every tick in the player's pre-think; the "last
  sent" value is forgotten at spawn, so a spawn always sends it). The
  panel remembers the last shown value (initially "none" = −1, not reset
  at spawn) and, when a newer one has arrived:
  - new = 0 → `SuitPowerZero`: stops `SuitLoop`, `SuitPulse`,
    `SuitArmorLow`, `SuitDamageTaken`; `Alpha` → 0 over 0.4 s.
  - new < old → `SuitDamageTaken` (`FgColor` → `BrightFg` 0–0.25, →
    `FgColor` 0.3–1.05; `Blur` 3 → 0 over 2.0 s), and if new < 20 also
    `SuitArmorLow` (same lines; it stops `SuitDamageTaken`, so the result
    is the same flash; no red: Quirks).
  - new > old: if old was "none" or 0, or new ≥ 20 →
    `SuitPowerIncreasedAbove20` (stops `SuitLoop`, `SuitPulse`,
    `SuitPowerZero`; `Alpha` 255 at once; `BgColor` reset at once;
    `FgColor` → `FgColor` 0.05 s; `Blur` 3 → 0 over 2.0 s), else
    `SuitPowerIncreasedBelow20` (stops `SuitPowerZero`; `Alpha` 255;
    `FgColor` → `BrightFg` → `FgColor`; `Blur` 3 → 0).
- The panel is drawn only while a new value is pending or its alpha > 0:
  with armour 0 it is invisible; it stays visible while dead if armour
  > 0. Values above 100 (citadel chargers) are shown as they are.

#### 4.5 Ammo (`HudAmmo`)

Each frame, with the weapon in hand:

- **Hidden** (no box, no text) when there is no weapon or it uses no
  primary ammo type: crowbar, stunstick, gravity gun and **S.L.A.M.**
  (whose primary ammo is None). Hiding does not change the remembered
  weapon (Quirks).
- Clip weapons (clip size ≥ 0: pistol, .357, SMG1, AR2, shotgun,
  crossbow): value = clip, reserve shown = carried ammo of the type.
  No-clip weapons (frag, RPG): value = carried ammo, no reserve number.
- **Same weapon as remembered:** a change of the value runs `AmmoEmpty`
  (new value 0: `FgColor` → `BrightDamagedFg` 0–0.2 s, → `DamagedFg`
  accel 0.2–1.4 s), `AmmoDecreased` (stops `AmmoIncreased`; `Blur` 7 at
  once, → 0 deaccel 0.1–1.6 s) or `AmmoIncreased` (`FgColor` → `BrightFg`
  0–0.15, → `FgColor` deaccel 0.15–1.65; `Blur` 5 at once, → 0 accel
  0.01–1.51). A reserve change runs `Ammo2Empty`, `Ammo2Decreased` or
  `Ammo2Increased` (they animate only `Ammo2Color`: no visible effect).
  The digits therefore turn and **stay red** after the clip reaches 0
  until an increase (reload) or a weapon change resets `FgColor`.
- **Different weapon:** the same value-change events can fire (the
  comparison is against the previous weapon's numbers, Quirks); then
  `WeaponUsesClips` (clip weapons: `Position` r150 432, `Size` 132 × 36,
  deaccel 0.4 s) or `WeaponDoesNotUseClips` (`Position` r118 432, `Size`
  100 × 36, deaccel 0.4 s); then `WeaponChanged` (`BgColor` → 250 220 0 80
  0–0.1 s, → `BgColor` deaccel 0.1–1.1 s; `FgColor` → `BrightFg` 0–0.1 s,
  → `FgColor` 0.2–1.7 s). The weapon is remembered.
- **Ammo icon:** the "ammo" icon of the first owned weapon (inventory
  order) whose primary ammo is this type (else its "ammo2" icon if its
  secondary is), drawn in the foreground colour with its top-left at
  x = `text_xpos` + (label width − icon width) / 2,
  y = `text_ypos` − (label height + icon height / 2), clipped to the
  panel (Open questions 3).
- Deck: `WeaponUsesClips` r162 426, 144 × 42; `WeaponDoesNotUseClips` r130
  426, 120 × 42.

#### 4.6 Secondary ammo (`HudAmmoSecondary`)

- Drawn while there is a weapon (box enabled) — visibility comes from its
  `Alpha` and colours, which the events set.
- When the weapon uses a secondary ammo type (SMG1: `SMG1_Grenade`,
  AR2: `AR2AltFire`, S.L.A.M.: `slam`), value = carried count of it;
  changes run `AmmoSecondaryEmpty` (`FgColor` → `BrightDamagedFg` →
  `DamagedFg` accel 0.2–1.4; `Blur` 7 → 0), `AmmoSecondaryDecreased`
  (`Blur` 7 → 0 deaccel 0.1–1.6) or `AmmoSecondaryIncreased` (`FgColor`
  flash 0.15/1.5 deaccel; `Blur` 5 → 0 accel).
- On a weapon change: to a weapon with secondary ammo →
  `WeaponUsesSecondaryAmmo`: stops one `HudAmmo` `Position` and one
  `Size` animation of another event (3) and all of this panel's
  animations; `BgColor` → 250 220 0 60 0–0.1 s, → `BgColor` 0.1–1.1 s;
  `FgColor` → `BrightFg` → `FgColor` (0.2–1.7 s); `Alpha` 255 over 0.1 s;
  and moves `HudAmmo` to `Position` r222 432 (deaccel 0.5 s), `Size`
  132 × 36 (deaccel 0.4 s). To one without → `WeaponDoesNotUseSecondaryAmmo`:
  all this panel's animations stopped; `FgColor` and `BgColor` → 0 0 0 0
  over 0.4 s; `Alpha` 0 over 0.1 s.
- Icon: the weapon's "ammo2" icon (SMG1 `t`, AR2 `z`; S.L.A.M. has none),
  placed as in 4.5.
- Reset at spawn: `Alpha` 0, remembered weapon none.

Resulting resting layouts (non-Deck), x from the right edge:

| Weapon in hand | `HudAmmo` | `HudAmmoSecondary` |
|---|---|---|
| pistol, .357, shotgun, crossbow | r150, 132 × 36, reserve shown | hidden |
| SMG1, AR2 | r222, 132 × 36, reserve shown | r76, 60 × 36 |
| frag, RPG | r118, 100 × 36, no reserve | hidden |
| S.L.A.M. | hidden | r76, slam count |
| crowbar, stunstick, gravity gun | hidden | hidden |

#### 4.7 The damage message

After each tick in which the player took damage (or its damage-type bits
changed), the server sends, at the start of the player's next
processing: the armour taken by the **last** hit (0–255), the health
taken summed over the hits since the last message (each hit's whole part,
0–255), the hit's damage types filtered to drown, poison, acid, burn,
slow burn, nerve gas, radiation and shock, and the position of the last
hit's inflictor (its origin). Fall damage's inflictor is the world (origin
0 0 0).

### 5. Weapon selection (`HudWeaponSelection`)

#### 5.1 Data

Each weapon's bucket, position, print name (localization token, e.g.
`#HL2_SMG1` "SMG\n(SUBMACHINE GUN)", two lines), and three icons from its
script: `weapon` (`WeaponIcons` glyph), `weapon_s` (`WeaponIconsSelected`,
the same letter with blur and scanlines) and `weapon_small`. Buckets 0–5
exist; HL2:DM fills 0–4 (weapons.md §1).

#### 5.2 Layout (`hud_fastswitch` 0, "buckets")

Panel at y = 16 units, x = 0, as wide as the screen. Keys: `SmallBoxSize`
32, `LargeBoxWide` 112, `LargeBoxTall` 84, `BoxGap` 8,
`SelectionNumberXPos` 4, `SelectionNumberYPos` 4, `TextYPos` 64 (all
units, fractional). With s, L_w, L_t, g the scaled sizes (pixels, L_w and
L_t truncated):

```math
x_0 = \left\lfloor \frac{W_{px} - \left(5\,(s + g) + L_w\right)}{2} \right\rfloor
```

Then for buckets 0–5 left to right:

- the **open bucket** (the highlighted weapon's bucket): a column of large
  boxes L_w × L_t at x, one per position 0..(highest position the player
  owns in that bucket), stacked downward from y = 0 with gap g; a
  position with no owned weapon draws an empty box (`SelectionEmptyBoxBg`)
  when `hud_showemptyweaponslots` is 1 and is skipped otherwise; the
  first box drawn carries the bucket number (bucket + 1); x advances by
  L_w + g;
- any other bucket: one small box s × s, in `SelectionBoxBg` with its
  number if the player owns any weapon in it (selectable or not), else in
  `SelectionEmptyBoxBg` without a number; x advances by s + g.

So bucket 5 is always an empty small box.

**Box drawing.** A box is the rounded box in its colour with alpha
multiplied by a/255; its number (if any) is drawn at (box + 4, box + 4)
units in `HudSelectionNumbers`, `SelectionNumberFg` with the same alpha
factor. Alphas come from the animation variables: highlighted weapon's
box a = `SelectionAlpha`; other boxes a = `SelectionAlpha` × `Alpha` /
255; small and empty boxes use `Alpha`.

**Icons** (centred: x + (L_w − w)/2, y + (L_t − h)/2, w and h the glyph's
advance width and the font height):

- highlighted weapon: colour `SelectedFgColor` (scheme `FgColor`, 255 220
  0 100) with alpha replaced by a; the `weapon_s` glyph is drawn, then
  the `weapon` glyph over it, same colour;
- other weapons: `weapon` glyph in the panel foreground colour (animated
  by the events) with alpha × a / 255;
- a weapon that cannot be selected (weapons.md "Selectable"): red
  255 0 0 with that alpha.

**Name.** Only under the highlighted weapon: its print name in
`HudSelectionText`, colour `TextColor`, first line starting at
x_box + (L_w − width of the first line) / 2, y_box + `TextYPos`; after a
line break the next line starts at y + 1.1 × font height, centred on the
width of the **last** line (equal to centring each line for the two-line
names). The number of characters drawn is the name's length times
`TextScan` (always 1 in HL2:DM).

At 640 × 480: x₀ = 164; with bucket 1 open: bucket 0 small box 164–196,
bucket 1 column 204–316 (pistol box y 16–100 with "2" at (208, 20); .357
box y 108–192), buckets 2–5 small boxes at 324, 364, 404, 444 (444–476).

#### 5.3 Open, fade and close (buckets)

- **Opening** (first `slotN`/`invnext`/`invprev` while closed) runs
  `OpenWeaponSelectionMenu`: stops `CloseWeaponSelectionMenu`,
  `WeaponPickup`, `FadeOutWeaponSelectionMenu`; over 0.1 s linear:
  `Alpha` → 128, `SelectionAlpha` → 255, `FgColor` → `FgColor`,
  `TextColor` → `BrightFg`, `TextScan` → 1.
- Every slot command (and `invnext`/`invprev`) restarts the idle clock.
  Idle > 0.5 s → `FadeOutWeaponSelectionMenu` once: `FgColor`, `TextColor`
  → 0 0 0 0 and `Alpha`, `SelectionAlpha` → 0, linear, starting 0.5 s
  later and lasting 1.0 s. Idle > 1.25 s → the selection closes
  (`CloseWeaponSelectionMenu`: the same four to 0 over 0.1 s) and the
  panel stops drawing at once. A command during the fade re-runs
  `OpenWeaponSelectionMenu`.
- Net effect: full brightness until 1.0 s after the last command, a fade
  to 75 % (by 1.25 s), then gone.
- `WeaponHighlight` (blur and box flash) belongs to the console style and
  is not used by buckets.

#### 5.4 Commands

- `slotN` (N = 1..6; `slot7`–`slot10` and `slot0` find nothing) is
  ignored while dead, frozen, without a local player or while the HUD's
  weapon selection is hidden (`+use` carry); a numbered menu (`HudMenu`)
  open takes the key instead.
  - **Buckets (0):** if the selection is open and the highlighted weapon is
    in bucket N−1, search from the next position; else from position 0.
    The lowest position ≥ start whose weapon can be selected wins; none →
    search again from 0. Found → open the selection if closed and
    highlight it. Either way `Player.WeaponSelectionMoveSlot` plays.
  - **Fast switch (1):** start after the weapon in hand if it is in bucket
    N−1, else from the start of the bucket; take the next selectable
    weapon in (bucket, position) order; if that is not in bucket N−1, take
    the bucket's first selectable weapon. Found and different from the
    weapon in hand → switch at once (no sound from the HUD). None found →
    `Player.DenyWeaponSelection`. Found but it is the weapon in hand →
    nothing. Nothing is drawn (the panel's draw time runs, its drawing
    has no fast-switch style: Quirks).
- `invnext` / `invprev`: the next / previous selectable weapon after the
  highlighted one (selection open) or the one in hand (closed) in (bucket,
  position) order, wrapping; highlight it, open if closed,
  `Player.WeaponSelectionMoveSlot`. With `hud_fastswitch` ≥ 1 it is then
  confirmed at once (5.5).
- `lastinv`: switches to the last weapon at once (weapons.md).
- `cancelselect` (and Escape while open): closes the selection with
  `Player.WeaponSelectionClose`; while closed it passes Escape on.
- `hud_fastswitch` 2 and 3 select the console styles (a plus-shaped
  cluster of 78–95 × 50 boxes around the crosshair; a carousel); not
  needed for parity (Open questions 8).

#### 5.5 Confirm

While the selection is open, an attack or attack2 press (the HUD sees it
before the command is built) is removed from that command and confirms:
a highlighted weapon that cannot be selected → `Player.DenyWeaponSelection`
and the selection stays open; otherwise the switch goes into the next
command, `Player.WeaponSelected` plays, and the selection closes
(`Player.WeaponSelectionClose` plays too).

### 6. Crosshair, quick info and suit zoom

#### 6.1 Position

The crosshair point is the screen centre (W/2, H/2) in pixels; the
autoaim offset is never non-zero in HL2:DM.

#### 6.2 Quick info brackets (`HUDQuickInfo`)

- Needs `crosshair` 1, a weapon in hand, and the crosshair not hidden.
- Centre sprite: the `crosshair` texture of `scripts/hud_textures.txt`
  (`sprites/qi_center`, 40 × 40 pixels unscaled) drawn with its top-left
  at (cx, cy − h_b/2) in `Normal` with alpha 138 (h_b = `QuickInfo` font
  height). Drawn even with `hud_quickinfo` 0.
- With `hud_quickinfo` 1: left bracket at x = cx − 2·w_l, right bracket at
  x = cx + w_r, both at y = cy − h_b/2 (w = glyph advance widths).
  Each is a vertical two-part bar: the top fraction p of the glyph height
  drawn with the "empty" glyph (`{` / `}`), the rest with the "full" one
  (`[` / `]`): health p = 1 − clamp(health/100, 0, 1); clip p = 1 −
  clamp(clip / max clip, 0, 1), p = 0 (full) for weapons without a clip.
  Colour `Normal` alpha 138 normally.
- **Warnings.** On a health change to ≤ 25 (from above): a blink starts
  (fade budget 255) and `HUDQuickInfo.LowHealth` plays once; a change to
  > 25 clears the warning. On a clip change, with max clip > 1 and
  clip / max clip ≤ 0.25: blink and `HUDQuickInfo.LowAmmo` once; above
  0.25 clears it. Thresholds: pistol ≤ 4, SMG1 ≤ 11, AR2 ≤ 7, shotgun and
  .357 ≤ 1; crossbow (max 1), frag, RPG and melee never warn. (Changes
  are checked while drawing; the first check after a spawn compares with
  100 health and 0 ammo.)
- **Blink:** while the budget is > 0 the whole "full" glyph is drawn in
  `Caution` with alpha from k = ⌊|sin(8t)| · 128⌋ (t = client time); the
  budget loses 200 × frame time each frame and, once at or below one
  frame's loss, ends only on a frame with k < 40 (about 1.3 s). After the
  blink, while still warning, the two-part bar is drawn in `Caution` with
  the same alpha rule. The alpha is the low byte of 255 · k (Quirks):

```math
\alpha = (255\,k) \bmod 256 = \begin{cases} 0 & k = 0\\ 256 - k & 1 \le k \le 128 \end{cases}
```

- **Panel alpha** (multiplies everything above): when the suit zoom
  starts → 0 over 0.25 s; when it ends → 255 over 0.5 s. Not zoomed: 1.0 s
  after the last health or clip change → 64 over 2.0 s; the next change →
  255 over 0.5 s.

#### 6.3 Crosshair (`HudCrosshair`)

- Drawn when `crosshair` 1, not paused, not frozen, the local player is
  the view entity, not in a VGUI input mode, and alive (or observing
  in-eye, or roaming with `cl_observercrosshair` 1); hidden while dead.
- Which icon, each frame from the weapon in hand:
  - the player's current FOV ≥ 90: the script's `crosshair` icon in
    `Normal` (255 208 64 255): the `Crosshairs` font `Q` for all weapons
    but crowbar and S.L.A.M., which use `sprites/crosshairs` (0, 48) 24 × 24;
  - FOV < 90 (which includes HL2:DM's **default FOV of 75**, and every
    zoom): the script's `zoom` icon in white (crossbow: `Crosshairs` `Q`),
    else the default crosshair, `crosshair_default` of
    `scripts/hud_textures.txt` (`sprites/crosshairs` (0, 48) 24 × 24) in
    white 255 255 255 255 (Open questions 4);
  - no weapon: the default crosshair, white.
- Drawn centred on the crosshair point: top-left (round(cx) − ⌊w/2⌋,
  round(cy) − ⌊h/2⌋); sprite icons keep their pixel size at every
  resolution; the font `Q` is 40 px tall below 1600 lines.
- The `autoaim` icons are never used (no autoaim target in DM).

#### 6.4 Suit zoom overlay (`HudZoom`)

Follows the suit zoom state (suit.md; not the crossbow's zoom). Let t be
the time since the state last changed and s = clamp(t / 0.4, 0, 1).

- Zoomed: α = s, r = s. Released: α = (1 − s)/4, r = 1 − s/2, and
  nothing is drawn once s = 1.
- Two outlined circles around the crosshair point, radius 66u·r (48
  segments) and 74u·r (64 segments), and tick marks: 1 px wide,
  `DashHeight` 4u tall, centred on the crosshair row, at x = round(cx ±
  k·G) for k = 2, 3, … while k < ⌊max(cx, W − cx) / G⌋, G = 16u · max(r,
  0.1); all in `ZoomReticleColor` with alpha 64·α.
- Four screen quarters darkened with `vgui/zoom` (mirrored so its corner
  sits at the screen corners), vertex colour black with alpha α.
- `BorderThickness` 88 in the layout is unused.

### 7. Aux power and team line

#### 7.1 Aux power (`HudSuitPower`)

- Drawn while the aux value differs from the last value seen or
  `AuxPowerColor`'s alpha > 0.
- On a value change: reaching 100 from below → `SuitAuxPowerMax`
  (`BgColor` and `AuxPowerColor` → 0 0 0 0 over 0.4 s: the panel fades
  out); dropping below 100 from 100 or from "none" (after a reset) →
  `SuitAuxPowerNotMax` (`BgColor` → `BgColor`, `AuxPowerColor` → 255 220
  0 220, 0.4 s). Then the active-device count n (sprint, breather;
  flashlight never in HL2:DM) is compared with the last: on a change
  `SuitAuxPowerNoItemsActive` / `OneItemActive` / `TwoItemsActive` /
  `ThreeItemsActive` animate `Size` and `Position` linearly over 0.4 s to:

| n | Size | Position | Bottom edge |
|---|---|---|---|
| 0 | 102 × 26 | 16, 400 | 426 |
| 1 | 102 × 36 | 16, 390 | 426 |
| 2 | 102 × 46 | 16, 380 | 426 |
| 3 | 102 × 56 | 16, 370 | 426 |

  (The layout's own start is (16, 396) 102 × 26.)
- Bar: chunks = ⌊92 / 9⌋ = 10; lit = ⌊10 · a/100 + 0.5⌋; chunk i at
  x = 8 + 9i, y = 15, 6 × 4 units; lit chunks in `AuxPowerColor`, the
  rest in its RGB with alpha 70 (`AuxPowerDisabledAlpha`).
- Low: lit ≤ 2 (aux < 25). The low state is re-evaluated when drawn, but
  a change is acted on only while a device is active or aux < 100:
  entering low → `SuitAuxPowerDecreasedBelow25` (`AuxPowerColor` → 255 0 0
  220, 0.4 s); leaving → `SuitAuxPowerIncreasedAbove25` (→ 255 220 0 220).
- Text in `Default`, colour `AuxPowerColor`: `#Valve_Hud_AUX_POWER` "AUX
  POWER" at (8, 4); while n > 0, one line per active device at (8, 22),
  (8, 32), … (`text2_gap` 10) in the order `#Valve_Hud_OXYGEN`,
  `#Valve_Hud_FLASHLIGHT`, `#Valve_Hud_SPRINT`.
- `AuxPowerLowColor`, `AuxPowerHighColor` in the layout are unused; the
  starting `AuxPowerColor` is 255 0 0 255 (Quirks: a red flash on the
  first HUD start).
- Deck: (16, 386) 112 × 54, bar (8, 18) 102 × 5, text2 (8, 26) gap 14;
  item sizes 112 × 30/44/56/70 at y 390/372/364/350.

#### 7.2 Team line (`TeamDisplay`, team play only)

- Shown while team play is on and the local player is alive: the text
  `#Team` ("Team %s1") with the team's name, in the team's colour, in
  `Default`, on a rounded `BgColor` box sized to the text plus 8 units on
  each side, at (16, 410) (Deck 400).
- When aux power drops below 100 → `FadeOutTeamLine` (`Alpha` 0 over
  0.25 s); back at 100 → `FadeInTeamLine` (255 over 0.5 s); so it never
  overlaps the aux panel.

### 8. Damage indicator (`HudDamageIndicator`)

On each damage message (4.7):

1. Local health ≤ 0 → `HudPlayerDeath` (stops `HealthLoop`,
   `HealthPulse`; `DmgFullscreenColor` → 255 0 0 96 deaccel 0.1–0.3 s,
   then → 255 0 0 255 deaccel 0.3–4.3 s), and stop.
2. Inflictor position exactly (0, 0, 0) and no drown bit → nothing (falls,
   world damage).
3. Nothing if both the health and armour amounts are 0.
4. Direction: d = normalise(inflictor − eye); f = horizontal view forward
   (pitch dropped, normalised), l = the horizontal left vector. With
   a = d·f, b = d·l:

```math
\theta = \left(\operatorname{atan2}(-b,\,-a) + \pi\right)\cdot\frac{180}{\pi} \in (0, 360]
```

   θ = 360 straight ahead, 90 left, 180 behind, 270 right.
5. High = health amount > 25. The first row that matches wins (bits:
   the message's filtered damage types; angle bounds inclusive):

| Event | Needs |
|---|---|
| `HudTakeDamageDrown` | drown bit |
| `HudTakeDamagePoison` | poison bit |
| `HudTakeDamageBurn` | burn bit |
| `HudTakeDamageRadiation` | radiation or acid bit |
| `HudTakeDamageHighLeft` | high, 45 ≤ θ ≤ 135 |
| `HudTakeDamageHighRight` | high, 225 ≤ θ ≤ 315 |
| `HudTakeDamageHigh` | high (any angle) |
| `HudTakeDamageLeft` | 45 ≤ θ ≤ 135 |
| `HudTakeDamageRight` | 225 ≤ θ ≤ 315 |
| `HudTakeDamageBehind` | 135 ≤ θ ≤ 225 |
| `HudTakeDamageFront` | otherwise (empty event: no effect) |

Event contents (linear unless said):

| Event | Variable | Steps |
|---|---|---|
| `HudTakeDamageLeft` / `Right` | `DmgColorLeft` / `Right` | 255 88 0 200 at once; → 255 0 0 200 over 0.3 s; → 255 0 0 0 deaccel 0.3–0.8 s |
| `HudTakeDamageBehind` | both sides | runs Left and Right |
| `HudTakeDamageHighLeft` / `HighRight` | `DmgHighColorLeft` / `Right` | 255 88 0 255 at once; → 255 0 0 200 over 0.4 s; → alpha 0 deaccel 0.4–2.8 s |
| `HudTakeDamageHigh` | `DmgFullscreenColor` | 255 88 0 200; → 255 0 0 200 over 0.4; → alpha 0 deaccel 0.4–2.8 |
| `HudTakeDamageDrown` | – | runs Behind |
| `HudTakeDamageBurn` | `DmgFullscreenColor` | 255 0 0 200; → alpha 0 deaccel 0.2–0.6 |
| `HudTakeDamagePoison` | `DmgFullscreenColor` | 255 236 128 240; → alpha 0 deaccel 0.4–1.2 |
| `HudTakeDamageRadiation` | `DmgFullscreenColor` | → 255 255 255 128 deaccel 0–0.1; → alpha 0 deaccel 0.1–0.5; runs Behind |

Drawing (material `vgui/white_additive`, additive; panel = whole screen,
W × H pixels), only while any of the five colours has alpha > 0:

- full screen quad in `DmgFullscreenColor`;
- each side: if either side's high colour alpha exceeds its normal colour
  alpha, **both** sides use the high shape: from the screen edge to the
  centre line (x 0..W/2 for the left), full height, alpha 1 at the edge
  and 0 at the centre, colour = that side's high colour; else the normal
  shape: a trapezoid between x₁ = 30u and x₂ = 66u (right side mirrored:
  W − 30u, W − 66u) whose outer edge spans y 100u–340u with alpha 0 and
  whose inner edge spans y 120u–320u (inset (240 − 200)/2 = 20u) with full
  alpha, colour = that side's normal colour. The vertex alpha is the
  colour's alpha times the shape's 0/1.

### 9. Pickup history (`HudHistoryResource`)

- Panel (r252, 40), 248 × 320 (Deck r272, gap 64); `history_gap` 56,
  `icon_inset` 38, `text_inset` 36 (the key appears twice, 36 then 26;
  the first is used: Open questions 1), number font `HudNumbersSmall`,
  text font `Default`. No background.
- **Entries:**
  - weapon picked up (the client sees a weapon become the local player's,
    any way, including the spawn loadout): its `weapon` glyph
    (`WeaponIcons`), unless the weapon is exhaustible (frag) or the same
    weapon entity is already listed; drawn in `Caution` if it has no
    ammo, else `Normal`; also plays `Player.PickupWeapon` (client);
  - ammo: whenever a networked ammo count of the local player rises, an
    entry with the increase and the ammo icon of the first owned weapon
    using that type (4.5 rule; no such weapon → the entry takes a slot but
    draws nothing); an existing "ammo denied" entry of the same type is
    killed and its slot reused;
  - item: the server's item message names the class; `item_battery` →
    `WeaponIcons` `*`, `item_healthkit` → `+`; `item_healthvial` has no
    icon in `hud_textures.txt`, so it adds nothing;
  - ammo denied (a pickup refused because the type is full, weapons.md):
    ignored if an ammo entry of that type is listed; refreshed to 2.5 s
    if already listed; else added, life 2.5 s, drawn in `Caution` with the
    text `#hl2_AmmoFull` ("FULL"); it never overwrites a live entry in
    its slot.
- **Slots.** A counter picks the slot for the next entry; slot k is drawn
  at panel y = 320u − 56u·(k + 1) (bottom-up). Before adding, if
  56·(k + 1) > 320 the counter restarts at 0 — so at most 5 rows (k 0–4),
  and the 6th entry overwrites slot 0. The counter returns to 0 when every
  entry has expired.
- **Life:** 5 s (`hud_drawhistory_time`); each draw clamps the remaining
  time to at most `hud_drawhistory_time`. Alpha = min(255, 80 × seconds
  left): full until 3.19 s are left, then linear to 0.
- **Drawing:** icon top-left at x = 248u − icon width − 38u, y = slot y
  (no half-height offset in HL2:DM); count (if > 0) at x = 248u − 36u
  (i.e. r40 on screen), y = slot y − (number font height − icon
  height)/2; "FULL" likewise in `Default`.
- Not drawn on the first frame after a reset; cleared at each spawn.

### 10. Death notices (`HudDeathNotice`)

- From the `player_death` event (rules.md): killer = the `attacker`
  user id's player (0 = none), victim, weapon name. Ignored if
  `hud_deathnotice_time` is 0.
- If the list already holds 4, the oldest is removed first. A new notice
  lives `hud_deathnotice_time` (6) seconds; expired ones are removed after
  each draw (no fade).
- **Suicide** = no killer, or killer = victim: only the victim's name and
  the skull. Otherwise killer name, icon, victim name.
- **Icon:** `death_<weapon>` from `scripts/mod_textures.txt`, font
  `HL2MPTypeDeath`; missing names and every suicide use `d_skull`
  (`sprites/640hud1` (192, 224) 32 × 16, scaled by H/480):

| Weapon name | Character | | Weapon name | Character |
|---|---|---|---|---|
| `357` | `.` | | `physics` | `9` |
| `ar2` | `2` | | `combine_ball` | `8` |
| `crossbow_bolt` | `1` | | `smg1_grenade` | `7` |
| `smg1` | `/` | | `stunstick` | `!` |
| `shotgun` | `0` | | `slam` | `*` |
| `rpg_missile` | `3` | | `crowbar` | `6` |
| `grenade_frag` | `4` | | `pistol` | `-` |

  No icon exists for `physcannon`, `physbox`, `door`, `trigger_hurt`,
  `worldspawn`, `world` or `crossbow`: those kills show the skull
  (between two names when there is a killer).
- **Layout** (panel = whole screen): line i (0 = oldest) at
  y = ⌊16u⌋ + i·(15u + ⌊4u⌋); right-justified: the victim's name ends at
  W − ⌊16u⌋; to its left the icon (font glyph: advance width × font
  height; sprite: size × H/480), then the killer's name. Names in
  `HudNumbersTimer`, each in its player's team colour (Unassigned
  yellow in DM); icon colour 255 80 0 255.
- Console line per notice: "killer killed victim with weapon." for kills;
  every suicide prints the "suicided" form (the "died" form never
  appears: Quirks).

### 11. Target ID (`TargetID`)

- Each frame the local player traces a line from eye + 10 units to eye +
  1500 units along the view (solid mask, ignoring itself); if it starts
  clear and hits a non-world entity, that is the target. Not in chase or
  death-cam observer modes.
- Only players produce text: DM, or TDM and not a teammate:
  `#Playerid_diffteam` ("Enemy: %s1"); TDM teammate: `#Playerid_sameteam`
  ("Friend: %s1 Health: %s2") with the health as **health / max health
  printed as a whole percentage** (100/100 → "1%"). Colour: the target's
  team colour. Font `TargetID`.
- `hud_centerid` 1: centred horizontally at y = 260u; 0: at (10u, 420u).
- The text vanishes the first frame nothing is targeted (Quirks).

### 12. Chat

Half-Life 2's `resource/ui/basechat.res` (loose; HL2:DM has none):
`HudChat` box (10, 275) 280 × 120 rounded `BgColor`, history text
(10, 17) inside it, 260 × 75, `ChatFont`, bottom-left aligned, wrapped;
input line (10, 395) 260 × 2; filters button (225, 2) 45 × 15. The
`HudChat` entry of `hudlayout.res` is 4 × 4 and hidden (unused). Formats,
the sound and colours are rules.md's; note the chat's Rebels red is
255 63 63, the other HUD parts' 255 64 64.

### 13. Scoreboard (`+showscores`, intermission)

- Layout `resource/ui/scoreboard.res`: 480 × 360, re-centred on the
  screen at every refresh (so at ((W − 480u)/2, (H − 360u)/2), not the
  file's c−260, 42); server name label (3, 2) 250 × 24 (from the
  `server_spawn` event's host name); player list (8, 0) 464 × 360, rows
  16 units apart; no growth.
- Background: `SectionedListPanel.BgColor` 0 0 0 90 filling a box whose
  four corners are cut by a 7-step staircase (offsets 0, 1, 2, 3, 4, 6, 9,
  10 **pixels**, unscaled); a 1-pixel border in scheme `FgColor` 255 220 0
  100 following the same outline.
- Columns (units): avatar 36 (2 × 18), name 284 (320 − 36), score 40,
  deaths 46, latency 46; header row: name empty, `#PlayerScore` "Score",
  `#PlayerDeath` "Deaths", `#PlayerPing` "Latency", right-aligned.
- Sections: DM: free-for-all (team 0, no section colour), then
  Spectators; TDM: Combine, Rebels, Spectators, each in its team colour.
  All always visible. Section headers: TDM: "<team name>    -   N
  player(s)" (`#ScoreBoard_Player` / `#ScoreBoard_Players`, N = the team's
  players) with the team's score in the score column and its average
  ping; DM: the text "Deathmatch    -   N player(s)" (N = all connected
  players) is written into the **Spectators** header and the free-for-all
  header stays empty (Quirks; Open questions 6).
- Rows: every connected player, in its team's section, text in its team
  colour; score = frags, deaths, latency in ms (blank if < 1; `BOT` for a
  bot with ping < 1); avatar image. Sorted by frags descending, then
  deaths ascending, then player index ascending. The local player's row
  is selected (highlighted). Refreshed every 1 s while shown and on game
  events.
- Shown while `+showscores` is held, and by the server at intermission
  (rules.md).
- **Avatars** (answered 2026-11-09). Only players with a Steam account id
  get one: the client asks Steam for that account's small avatar and
  draws it in the avatar cell. A bot (account id 0), a player without
  one, or a client without Steam running gets **nothing**: the cell stays
  empty (no placeholder image, no default silhouette) and keeps its width,
  so names do not shift left. An avatar still loading draws nothing
  either. Drawn avatar: a square of 16 units (16 × screen height / 480 px)
  at 11 units from the cell's left edge and 0.5 unit down (room left for
  the friend frame); when the row's player is the viewer's Steam friend,
  the frame `vgui/ico_friend_indicator_scoreboard` (the hud texture
  `ico_friend_indicator_avatar`) is drawn first at the cell's left edge,
  27.5 × 17 units. The image is tinted white (unchanged). For Chimera
  without Steam: leave the cell empty, which is what the game shows for
  bots.
- **Layout per mode** (the rest of this section, restated): header row
  (always shown): avatar cell, empty name, "Score", "Deaths", "Latency"
  right-aligned. DM: one free-for-all section (team 0) then Spectators.
  TDM: Combine (153 204 255), then Rebels (255 64 64), then Spectators
  (204 204 204); each section always shown, even empty, its header line
  in the team colour: "<team>    -   N player(s)" in the name column,
  the team's score in the Score column, the team's average ping in
  Latency (blank below 1), Deaths empty. Spectators' header gets no score
  or ping in TDM. Rows inside a section: frags descending, deaths
  ascending, then the order the rows were first added to the board
  (players are added in index order on each refresh, so this is the
  index order unless a row was created later, e.g. a player who joined
  after the board was built); dead players are not moved. There is no class,
  voice, friends or MVP column, and no "dead" mark.

### 14. MOTD and team menu

- **No team menu.** HL2:DM's client creates only the scoreboard, the
  info (MOTD) window, the spectator GUI and spectator menu, and the nav
  progress panel. Team changes are console (`jointeam`) or model changes
  (rules.md).
- **MOTD** (shown to a joining player, rules.md): the shared text window
  with `resource/ui/textwindow.res`: full-screen 640 × 480 frame (scaled
  to fit, centred), title label (76, 22) 450 × 48, message area (76, 116)
  480 × 240 (HTML, or plain text for a text MOTD), OK button (76, 364)
  128 × 20 `#PropertyDialog_OK` that closes it. Background: HL2:DM's own
  frame — inset 20, title bar 42, main area 376, corner pieces 10 (all
  scaled by min(W/640, H/480)), images `gfx/vgui/round_corner_nw/ne/sw/se`,
  `gfx/vgui/solid_background`, `gfx/vgui/trans_background` tinted white
  with `BgColor`'s alpha, logo `gfx/vgui/hl2mp_logo` (a square of the
  title bar's height) tinted `FgColor`, caption at x inset 76. Its own
  background paint is empty. Pressing the key bound to `showscores` opens
  the scoreboard over it. The default MOTD files are `cfg/motd_default.txt`
  (HTML) and `cfg/motd_text_default.txt`.
- **MOTD in detail** (answered 2026-11-09).
  - *When*: every time a client becomes active on a map, right after its
    first spawn in that map: first join **and again after every
    `changelevel`** (each new map re-activates every client, and the
    server sends the window again). The player is already spawned and
    alive behind it.
  - *What the server sends*: title = the server's `hostname` value
    ("Half-Life 2 Deathmatch" from `valve.rc` unless the server sets
    another; the text "MESSAGE OF THE DAY" only if the cvar were
    missing), content type "string-table entry", entry `motd`, no exit
    command (closing it runs nothing), "unload on close" =
    `sv_motd_unload_on_dismissal` (0). The server fills the `motd` entry
    at map load from `cfg/<motdfile>` (`motdfile` "motd.txt"), else
    `<motdfile>` at the game root, else, only while `motdfile` is at its
    default, `cfg/motd_default.txt`; the `motd_text` entry likewise from
    `motdfile_text` / `cfg/motd_text_default.txt`. Files over 2048 bytes
    are not loaded (the entry stays empty).
  - *What the client shows*: the title in the `MessageTitle` label (font
    `MenuTitle`, west-aligned). The entry's text decides the area: empty
    → the area stays blank; starting `http://` or `https://` → that page
    in the HTML area; starting `<` → the text as an HTML page; anything
    else → plain text in the text area (read-only, multi-line, scrolled to
    the top). If the player set `cl_disablehtmlmotd 1`, an HTML MOTD is
    replaced by the `motd_text` entry as plain text (blank if that is
    empty). Links to `steam://` are refused inside the page.
  - *The frame pictures* (s = min(W/640, H/480); the 640 × 480 window is
    centred, offsets ox = (W − 640s)/2, oy = (H − 480s)/2; at 4:3 both
    0): corners `gfx/vgui/round_corner_nw` at (ox + 20s, oy + 20s),
    `_ne` at (W − ox − 30s, oy + 20s), `_sw` at (ox + 20s, H − oy − 30s),
    `_se` at (W − ox − 30s, H − oy − 30s), each 10s square;
    `gfx/vgui/solid_background` strips: top (ox + 30s, oy + 20s) of
    width W − 2ox − 60s, height 10s; title band (ox + 20s, oy + 30s),
    width W − 2ox − 40s, height 42s; bottom strip (ox + 30s,
    H − oy − 30s), width W − 2ox − 60s, height 10s; main area
    (ox + 20s, H − oy − 30s − 376s), width W − 2ox − 40s, height 376s.
    At 4:3 the title band ends at y 72s and the main area starts at 74s:
    a 2-unit see-through line between them. `gfx/vgui/trans_background`
    bands fill the margins: top (0, 0, W, oy + 20s), bottom (0,
    H − oy − 20s, W, oy + 20s), left (0, oy + 20s, ox + 20s,
    H − 2oy − 40s), right (W − ox − 20s, oy + 20s, ox + 20s,
    H − 2oy − 40s). Logo `gfx/vgui/hl2mp_logo` 42s square at
    (ox + 25s, oy + 25s), tinted `FgColor` (255 220 0 100 or Orange 255 176 0 255, the same duplicate as Open questions 1). All the corner and solid
    pieces are tinted (255, 255, 255, `BgColor` alpha); the textures are
    black (solid opaque, corners black with a rounded alpha mask), so the
    frame is black at `BgColor`'s alpha (Open questions 1 decides 76 or
    196). `trans_background` is black at alpha about 1/255 and not
    tinted: the margins are practically clear. The viewport's own
    backdrop colour `ViewportBG` is `Blank` (0 0 0 0): the game stays
    visible around and through the frame. Drawn over the frame: the
    title, HTML/text area and OK button (positions as above, scaled
    by s and offset by (ox, oy)). An empty caption label sits at
    (ox + 76s, oy + 25s), screen-wide, 42s tall.
  - *Keys*: the window takes the mouse and the keyboard focus (game keys
    do nothing while it is up; the OK button holds focus): Enter or Space
    press OK; the key bound to `showscores` opens the scoreboard over it
    (the scoreboard hides again when that key is released); gamepad A or
    B (Xbox or Steam Controller) close it. Closing (OK or those buttons)
    hides it and runs no command. Escape is not handled by the window
    (Open questions 12).

### 15. Hints

`env_hudhint` key hints (specs/source/game_entities.md) use
`HudHintKeyDisplay`: (r120, r340) 100 × 200, text (8, 8), gaps 8,
`TextColor` 255 170 0 220, rounded box; `KeyHintMessageShow`: `Alpha` 255
over 0.5 s, flashes to 255 220 0 255 at 0.5 s and 1.5 s (0.2 s each),
`Alpha` 0 over 1.0 s from 12 s; `KeyHintMessageHide`: `Alpha` 0 over 0.5 s.
`HudHintDisplay` (c−240, c60) 480 × 100 with `HintMessageShow` (size 0.3 s,
flashes at 2.0 and 3.0 s, hidden from 10 s) is the hint panel for HUD
hint messages.

## Per-frame order

Client side, once per rendered frame:

1. Network messages of the frame: HUD reset (then its element resets and
   instant completion, 1), battery, damage, item pickup, ammo denied,
   chat; game events (`player_death` → death notice).
2. Entity data: health, aux, devices, zoom flag, ammo counts (an ammo
   rise adds history entries), weapon ownership (pickup entries).
3. Commands built from input: slot commands, the selection's attack
   capture and confirm (5.5).
4. Element thinks (health, suit, ammo, secondary ammo, aux, selection
   timeout, quick-info alpha, team line, target trace): each starts
   events on changes.
5. Animation update: queued commands due by now run, then every active
   animation applies in list order (3).
6. Paint, back to front in element order: damage indicator, numeric
   panels, aux, selection, history, death notices (expired ones removed
   after), crosshair, quick info (it evaluates its warnings while
   painting), zoom overlay, target ID, scoreboard and MOTD on top.

Whether an event started in step 4 shows its first value in the same
frame or the next is not visible at test precision (one frame).

## Edge cases

- **Several hits in one tick:** one damage message; health amounts add,
  armour amount and position are the last hit's; the flash is chosen once.
- **Damage directly above or below** (no horizontal component): θ comes
  out as 0, matches no side: no flash (unless high: full-screen).
- **Quick double switch** (two weapon changes within 0.5 s, the second to
  a weapon with secondary ammo): `WeaponUsesSecondaryAmmo` stops only the
  first foreign `HudAmmo` position animation it finds, which may be the
  older one; the newer `WeaponUsesClips` animation, started later, then
  wins and the ammo panel can rest at r150 overlapping the secondary
  panel until the next switch.
- **Death while the selection is open:** hidden while dead; the open
  state is not cleared by the HL2 selection itself.
- **Respawn within 4.3 s of death:** the reset cancels `HudPlayerDeath`
  and clears the red at once.
- **Ammo pickup for a type no owned weapon uses** (loadout .357 and
  buckshot, H0 test 19): a blank history row that still takes a slot.
- **History overflow:** the 6th entry within the life time overwrites
  slot 0 even if it is still live.
- **More than 4 kills at once:** only the last 4 shown.
- **Long names:** death-notice lines are not clipped; long killer names
  extend left off-screen.
- **`hud_fastswitch` changed while the selection is open:** the open
  state persists; drawing follows the new style.
- **Spectator team in DM:** the Spectators section carries the
  "Deathmatch" header.
- **Armour above 100** shows as is; quick info has no armour.

## Quirks

Keep all of these (parity first).

- **Labels never change colour on their own.** The events' `TextColor`
  and `Ammo2Color` lines have no visible effect: labels and reserve
  numbers use the panel foreground colour.
- **`HealthIncreasedAbove20` on any change ≥ 20**, including damage: a
  2-second glow after every hit above 20 health.
- **Empty clip digits stay red** until a reload or a switch.
- **Weapon-change flashes compare with the previous weapon's numbers**
  (an increase/decrease flash can fire on a switch).
- **Hidden ammo panel keeps its remembered weapon:** SMG1 → crowbar →
  SMG1 shows no `WeaponChanged` flash on the return.
- **Armour low has no red** (the low-armour event is the normal damage
  flash).
- **Suit panel visible while dead** if armour > 0.
- **Aux panel's first appearance is red** (starting colour 255 0 0 255)
  for 0.4 s at the very first HUD start.
- **Quick-info blink alpha** uses the low byte of 255·k, so it runs
  128–255 (brightest near the sine's zeros), not 0–128.
- **Default FOV draws the default crosshair:** below FOV 90 every weapon
  but the crossbow shows the white 24 × 24 default sprite instead of its
  scripted crosshair (Open questions 4).
- **Fast switch draws no selection** at all.
- **Selection fade is cut off:** the panel closes at 1.25 s, a quarter of
  the way into its 1-second fade.
- **Teammate health in target ID** reads "1%" at full health.
- **Target ID doesn't linger:** its 0.5 s hold re-uses a remembered target
  that is never stored.
- **`hud_showtargetid` does nothing.**
- **DM scoreboard header on the Spectators section.**
- **Death-notice console line** never uses its "died" form.
- **Vials leave no history entry**, kits and batteries do.

## Test cases

Implementer checks (no game needed). Screen 640 × 480 (u = 1) unless
said; times in seconds of client time; "event E at t" means the HUD
starts event E at t.

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | 1920 × 1080 | lay out `HudHealth`, `HudAmmo` (layout start) | – | u = 2.25; health box x 36, y 972, 229 × 81; ammo x 1920 − 337 = 1583, 306 × 81 |
| 2 | scheme lookup | `FgColor`, `BgColor`, `SelectionSelectedBoxBg` | – | 255 220 0 100; 0 0 0 76; 0 0 0 80 (first occurrences) |
| 3 | fonts at 1080 / 1440 / 600 lines | `Default` | – | GorDIN 21 px w700 not additive / 30 px additive / 14 px w600 |
| 4 | fonts | `HudNumbers` at 1080; `Crosshairs` at 1080 and 2160 | – | 72 px; 40 px and 80 px |
| 5 | Deaccel animation 0 → 200 alpha over 1.0 s | – | t = 0.25 | 100 (√0.25 = 0.5); Accel: 12.5 (12 after truncation) |
| 6 | `HealthDamageTaken` started at t0, `FgColor` at 255 220 0 100 | – | t0 + 0.25; t0 + 0.675 | 255 220 0 255; 255 220 0 177 (0.5 of the way back: 255 − 77.5 → 177) |
| 7 | event E running, E started again | – | – | the first run's animations and queued commands removed; only the new run applies |
| 8 | health panel shows 100 | health becomes 80 | – | `HealthIncreasedAbove20` (not a "decreased" event); damage message with 20 → `HealthDamageTaken` too |
| 9 | health 100 | health becomes 15 | t = 1.0, 1.8, 2.6 | `HealthIncreasedBelow20` + `HealthLow` at 0; `HealthPulse` at 1.0, 1.8, 2.6 …; digits end `DamagedFg` 180 0 0 230 |
| 10 | as 9 | health becomes 25 | – | `HealthIncreasedAbove20`: pulse loop stopped, `BgColor` 0 0 0 76 at once |
| 11 | health 5 | health becomes 0 | – | no event; panel hidden (dead) |
| 12 | armour 0 (panel invisible) | battery: 15 | – | `SuitPowerIncreasedAbove20` (old was 0): alpha 255 |
| 13 | armour 30 | damage → 18 | – | `SuitDamageTaken` and `SuitArmorLow` |
| 14 | armour 10 | battery → 15 | – | `SuitPowerIncreasedBelow20` |
| 15 | armour 18 | damage → 0 | t + 0.4 | `SuitPowerZero`; alpha 0 |
| 16 | pistol in hand, clip 18, reserve 150 | – | – | `HudAmmo` value 18 at (r150 + 44, 434), reserve 150 at (r150 + 98, 448); secondary hidden |
| 17 | pistol | switch to SMG1 (clip 45, reserve 0 extra, 1 grenade) | 0.5 s | `WeaponUsesClips` and `WeaponChanged` from the ammo panel, `WeaponUsesSecondaryAmmo` from the secondary: ammo panel at r222, 132 × 36; secondary at r76 shows 1 |
| 18 | SMG1 | switch to frag (3 grenades) | 0.4 s | ammo panel r118, 100 × 36, value 3, no reserve; secondary alpha 0 |
| 19 | S.L.A.M. in hand, 2 slams | – | – | ammo panel hidden; secondary shows 2, no icon |
| 20 | pistol clip 1 | fire | – | value 0: `AmmoEmpty`; digits red until reload (`AmmoIncreased`) |
| 21 | aux 100 | sprint starts, aux 99.6 | – | `SuitAuxPowerNotMax`; n 0 → 1: `SuitAuxPowerOneItemActive` (102 × 36 at 16, 390 over 0.4 s); line "SPRINT" at (8, 22) |
| 22 | aux values 100, 55, 25, 24.99, 0 | – | – | lit chunks 10, 6, 3, 2, 0; low (red) for 24.99 and 0 |
| 23 | aux back to 100, no device | – | 0.4 s | `SuitAuxPowerMax`: alpha 0; panel no longer drawn |
| 24 | weapon selection, 640 × 480, owns crowbar, gravity gun, pistol, .357, SMG1, frag | `slot2` | – | open; x₀ 164; boxes as 5.2's example; pistol highlighted; name "9MM PISTOL" centred under 204–316 at y 80 |
| 25 | as 24 | `slot2` again | – | .357 highlighted (box y 108–192) |
| 26 | as 24 at 1920 × 1080 | `slot2` | – | small 72, large 252 × 189, gap 18; x₀ 609; y 36 |
| 27 | owns frag (bucket 4 pos 0) and S.L.A.M. (pos 2), no RPG | `slot5` | – | column: frag box, empty box, S.L.A.M. box; with `hud_showemptyweaponslots` 0: frag box then S.L.A.M. box directly below |
| 28 | selection opened by the last `slotN` at t0 | no input | t0 + 0.5, + 1.0, + 1.25 | fade event started at 0.5 (values start moving at 1.0); at 1.25 alpha 75 % of full; closed and not drawn after 1.25 |
| 29 | selection open on SMG1 with no ammo of any kind | `+attack` | – | `Player.DenyWeaponSelection`, stays open, no shot |
| 30 | selection open on pistol | `+attack` | – | switch to pistol in the next command; `Player.WeaponSelected` then `Player.WeaponSelectionClose`; attack not fired |
| 31 | `hud_fastswitch 1`, holding SMG1, owns AR2 | `slot3` | – | switch to AR2 at once; nothing drawn |
| 32 | `hud_fastswitch 1`, holding pistol, no .357 | `slot2` | – | nothing, no sound |
| 33 | `hud_fastswitch 1`, bucket 4 empty | `slot5` | – | `Player.DenyWeaponSelection` |
| 34 | `hud_fastswitch 0`, bucket 4 empty | `slot5` | – | no selection opens; `Player.WeaponSelectionMoveSlot` plays |
| 35 | quick info, health 30 → 25 | – | – | blink in `Caution`, `HUDQuickInfo.LowHealth` once |
| 36 | k values 0, 1, 64, 128 | blink alpha | – | 0, 255, 192, 128 |
| 37 | pistol clip 18 → 5 → 4 | – | – | warning at 4 (4/18 = 0.222 ≤ 0.25), `HUDQuickInfo.LowAmmo`; not at 5 |
| 38 | crossbow clip 1 → 0 | – | – | no ammo warning (max clip 1) |
| 39 | health 60, SMG1 clip 15/45 | brackets | – | left bracket: top 40 % empty glyph; right: top 66.7 % empty |
| 40 | no health or clip change | – | t = 1.0, 3.0 | panel alpha starts dropping after 1.0 s, 64 at 3.0 |
| 41 | FOV 90, pistol | – | – | crosshair `Crosshairs` `Q`, 255 208 64 255 |
| 42 | FOV 75, pistol / crossbow / crowbar | – | – | white 24 × 24 default sprite / white `Q` / white 24 × 24 default sprite |
| 43 | suit zoom starts at t0 | – | t0 + 0.2 | radii 33u and 37u; reticle alpha 32; corner shade alpha 0.5 |
| 44 | zoomed, released at t1 | – | t1 + 0.2; t1 + 0.4 | α 0.125 (reticle alpha 8), radii 49.5u and 55.5u; nothing drawn |
| 45 | eye at origin, yaw 0; inflictor (0, 100, 0) / (−100, 0, 0) / (100, 100, 0) / (100, −100, 0) / (100, 0, 0); health damage 10 | damage message | – | Left (θ 90) / Behind (180) / Left (45) / Right (315) / Front (360): no effect |
| 46 | inflictor (0, 100, 0), health damage 30 | – | – | `HudTakeDamageHighLeft`; both sides drawn with the high shape (right side's high alpha 0: invisible) |
| 47 | inflictor (100, 0, 0), damage 30 | – | – | `HudTakeDamageHigh`: full-screen flash |
| 48 | `HudTakeDamageLeft` at t0 | – | t0 + 0.15; t0 + 0.425; t0 + 0.8 | left colour 255 44 0 200; alpha 100; alpha 0 |
| 49 | fall damage 10 | message | – | no flash (inflictor at 0 0 0) |
| 50 | killed (health 0 when the message arrives) | – | t + 0.15; t + 1.3 | `HudPlayerDeath`: full-screen alpha 48; then 175 |
| 51 | left normal shape at 640 × 480 | – | – | outer edge x 30, y 100–340 alpha 0; inner edge x 66, y 120–320 full alpha; right side x 610 and 574 |
| 52 | history: SMG1 ammo box +45 with SMG1 owned | – | – | row 0 at panel y 264 (screen 304), SMG1 ammo glyph `r` at x 248 − w − 38, "45" at panel x 212 (screen r40) |
| 53 | history entry added at t0 | – | t0 + 1.8125; t0 + 4; t0 + 5 | alpha 255; 80; removed |
| 54 | ammo denied for SMG1 twice 1 s apart | – | – | one red row with "FULL", life 2.5 s from the second touch (starts at alpha 200) |
| 55 | 6 pickups within 2 s | – | – | rows 0–4 bottom-up, the 6th replaces row 0 |
| 56 | picks up `item_healthvial`, then `item_healthkit` | – | – | vial: nothing; kit: `+` glyph row |
| 57 | picks up a frag weapon with 1 grenade | – | – | no weapon row; one ammo row "1" |
| 58 | 640 × 480, A kills B with the shotgun, DM | – | – | line 0 y 16; "B" ends at x 624; glyph `0` left of it in 255 80 0 255; "A" left of that; both names 255 178 0 |
| 59 | 5 kills within 6 s | – | – | 4 lines; the first removed; lines at y 16, 35, 54, 73 |
| 60 | B dies to `trigger_hurt` / gravity gun kill / `kill` | – | – | skull between names / skull between names / skull, victim only |
| 61 | notice at t0 | – | t0 + 6.0 + one frame | removed (no fade) |
| 62 | TDM, teammate at 100 health, aimed at 600 units | – | – | "Friend: name Health: 1%" centred at y 260, team colour |
| 63 | DM, enemy at 1600 units | – | – | nothing (range 1500) |
| 64 | DM scoreboard: A 5/2, B 5/1, C 7/9, spectator S | – | – | order C, B, A; header row over the free-for-all section empty; "Deathmatch    -   4 players" over the Spectators section |
| 65 | TDM | open scoreboard | – | sections Combine (153 204 255), Rebels (255 64 64), Spectators; each header "<team>    -   N players" with team score and ping |
| 66 | scoreboard at 1920 × 1080 | – | – | 1080 × 810 at (420, 135); rows 36 px apart; corner staircase 10 px |
| 67 | TDM, alive, aux 100 → 90 | – | 0.25 s | team line alpha 0; aux 100 again → 255 over 0.5 s |
| 68 | spawn | – | – | HUD reset: history empty, secondary alpha 0, damage colours 0; `HealthIncreasedAbove20` glow; death notices kept |
| 69 | `mp_flashlight 1`, flashlight on | – | – | no flashlight indicator anywhere; aux panel shows no FLASHLIGHT line |
| 70 | chat from a Rebel | – | – | name colour 255 63 63 (death notices and scoreboard: 255 64 64) |

Checks on this install, marked **(install)**, and in the running game or
on the probe server, marked **(probe)** (game build 24600434, dedicated
server build 24600426):

| # | How | Expected |
|---|---|---|
| I1 (install) | read `hl2mp/hl2mp_pak_dir.vpk`'s `scripts/hudlayout.res`, `scripts/hudanimations.txt`, `resource/clientscheme.res` | the numbers of sections 2–9 (e.g. `HudSuitPower` `BarWidth` 92, `HudDamageIndicator` `dmg_tall2` 200, `HudWeaponSelection` `LargeBoxWide` 112, `HudHistoryResource` `history_gap` 56); `hudanimations.txt` 862 lines |
| I2 (install) | search the path for `scripts/hudanimations_manifest.txt` | absent: only `scripts/hudanimations.txt` loads |
| I3 (install) | character sets of `hl2mp/resource/halflife2.ttf` and `hl2/resource/halflife2.ttf` | HL2:DM's lacks 0x5C, 0xA9, 0xAE; HL2's has them |
| I4 (install) | `scripts/mod_textures.txt` | the 14 `death_*` entries of section 10 plus `d_skull` and the other `d_*` sprites, `voice_player`, `voice_self` |
| I5 (install) | Half-Life 2's loose `scripts/hud_textures.txt` | `item_battery` `*`, `item_healthkit` `+`, no `item_healthvial`; `crosshair_default` (0, 48) 24 × 24 |
| I6 (install) | family names of the GorDIN, HalfLife2, HL2MP, HL2cross files | GorDIN (Light, Regular, Semibold, Bold, Black), HalfLife2, HL2MP, HL2cross |
| P1 (probe) | screenshot the HUD at 1920 × 1080 with health 100, armour 50, SMG1 | box colour 0 0 0 76 over the scene, digits 255 220 0 at about 100 alpha (settles Open questions 1); positions of test 1 and 17 |
| P2 (probe) | `fov_desired 75` vs `90`, pistol | default sprite vs `Q` (Open questions 4) |
| P3 (probe) | take 30 damage from the left; from behind; fall | half-screen left gradient; full-screen flash; nothing |
| P4 (probe) | open the selection with `slot2`, wait | fade begins about 1.0 s after the press, gone at 1.25 s |
| P5 (probe) | DM scoreboard with a spectator | where the "Deathmatch - N players" text appears (Open questions 6) |
| P6 (probe) | spawn and watch the history | which loadout rows appear (Open questions 5) |
| P7 (probe) | `mp_flashlight 1`, toggle the flashlight | no indicator (Correction to wave 1/2 1) |
| P8 (probe) | look at a teammate in TDM | "Health: 1%" |
| P9 (probe) | `hud_fastswitch 1`, `slot3` | weapon changes, no selection drawn |

## Open questions

1. **Duplicate scheme keys.** The scheme lists `FgColor`, `BgColor`,
   `Panel.FgColor`, `Panel.BgColor` and the four selection colours twice
   with different values; the VGUI scheme code is not in the SDK. The
   first occurrence is assumed (it gives HL2's familiar dim-yellow on a
   faint box; the second would give orange on 196-alpha black and invisible
   panel backgrounds). The same is assumed for `text_inset` (36 vs 26) in
   the layout. P1 settles it.
2. **Conditional keys on Linux.** `[$WIN32]` is assumed true on Linux
   (the PC values); if Linux took the "else" branch, `HudFlashlight`,
   `HudZoom` `DashHeight` and `HudWeaponSelection` `ypos` would differ. P1
   and P4 settle it.
3. **Ammo icon above the label.** The formula puts the icon's top above
   the label, partly outside the panel; VGUI clips painting to the panel,
   so how much shows is resolution- and glyph-dependent. Compare with a
   screenshot.
4. **Crosshair below FOV 90.** The SDK switches to the default sprite
   below FOV 90, which is every HL2:DM player at the default
   `fov_desired` 75. Whether the shipped client behaves the same, and what
   `sprites/crosshairs` (0, 48) and `sprites/qi_center` look like, needs
   P2.
5. **Order of the spawn's reset and loadout data.** The reset is assumed
   to be processed before the spawn's entity data (messages before the
   snapshot), so the loadout fills the history (weapons, ammo, blank rows
   for .357 and buckshot, wrapping after 5). P6.
6. **DM scoreboard section header.** Read as described (the
   "Deathmatch" text lands on the Spectators section). Plausible but
   surprising; P5.
7. **Fonts without `yres`.** The "scaled from 480 lines" rule for entries
   without `yres` is Chimera's CS:S rule (by capture); the 2024 scheme's
   extra "proportional" entries (`Default` 10, `Crosshairs` 17, ...) are
   assumed to follow it. A 1440-line capture would confirm `Default` at
   30 px.
8. **Console selection styles** (`hud_fastswitch` 2, 3, and the Steam
   Controller forcing fast switch) are not specced in detail; ask whether
   they are wanted.
9. **Spectator GUI** (`resource/ui/spectator.res`, `bottomspectator.res`)
   is shown only to Spectator-team members (rules.md); not covered here.
10. **Damage message timing vs health.** `HudPlayerDeath` needs the
    local health to read ≤ 0 when the damage message is handled; if the
    message is handled before the snapshot that carries health 0, the
    death fade would not play and a normal flash would. P3 with a lethal
    hit settles it.
11. **Steam Deck** values are listed but not verified; Chimera's Deck
    support decides whether they matter.

12. **MOTD and Escape.** The window handles Enter/Space (OK), the
    `showscores` key and gamepad A/B; Escape is not handled by it. Whether
    Escape then opens the game menu over the MOTD (expected, the engine's
    own handling) and whether the MOTD is still there after the menu is
    closed needs a look in the game. Also whether keyboard movement is
    really blocked while it is up (expected: the window holds keyboard
    focus).
