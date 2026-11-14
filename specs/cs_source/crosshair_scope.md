# Counter-Strike: Source: crosshair, scope overlay and zoomed sensitivity

Source basis: Valve's public Source SDK 2013 for the generic client: the
per-frame rule that scales mouse sensitivity by the zoomed field of view,
the player's field-of-view interpolation and minimum, and the mouse
scaling. CS:S's own client (its crosshair drawing, its scope overlay and
any override of the generic rules) is **not** in the SDK. From the CS:S
install I read data only: the weapon scripts (decrypted at read time, values
only), the HUD layout, the options page layout, the scope and crosshair
materials, and the scope textures' size and shape (looked at, not
extracted). The names of the client's console variables come from the
client library's string table (names a player can type; no defaults or code
were taken from it). Everything CS:S-specific is marked *inferred* or
*hypothesis*, with a capture that settles it.
Status: draft

## Summary

- **Crosshair.** Each weapon script gives `CrosshairMinDistance` (the gap
  at rest) and `CrosshairDeltaDistance` (how much a shot opens it). With
  `cl_dynamiccrosshair 1` the gap grows with firing and movement and
  recovers; the exact rule is not in the reference (hypotheses below).
- **Scope.** While a sniper rifle is zoomed, the screen shows a black mask
  with a circular hole of screen height, built from four mirrored quarters
  of `sprites/scope_arc`, a faint dirty-glass layer `overlays/scope_lens`,
  black bars at the sides and thin black cross lines (layout *inferred*).
- **Zoomed sensitivity** (generic SDK rule, high confidence): mouse
  sensitivity is multiplied by `current FOV / default FOV ×
  zoom_sensitivity_ratio` whenever the FOV is not the default.

## Units and conventions

- Screen sizes in pixels; HUD layout files use a 640×480 virtual screen
  scaled to the real one (proportional fonts and positions).
- FOV in degrees, horizontal for a 4:3 screen (Source's convention);
  `default_fov` 90 in CS:S. The player's networked FOV `m_iFOV` 0 means "the
  default".
- Mouse: counts per frame; angles in degrees; `m_yaw` and `m_pitch` (0.022)
  convert scaled counts to degrees.

## Constants

| Name (ours) | Value | Unit | Meaning | Confidence |
|---|---|---|---|---|
| default_fov | 90 | deg | CS:S default FOV | measured (weapons.md M15) |
| zoom_sensitivity_ratio (SDK default) | 1.0 | × | generic default, archived client cvar | SDK |
| zoom_sensitivity_ratio (CS:S default) | unknown (our code uses 1.2) | × | Q3 | open |
| min_fov (generic) | 75 multiplayer, 5 single player | deg | floor on the FOV used for the sensitivity rule | SDK; CS:S override open (Q4) |
| m_yaw, m_pitch | 0.022 | deg per scaled count | | SDK |
| scope_arc texture | 256 × 256, DXT5 | texels | quarter-circle mask | data |
| scope_lens texture | 512 × 512, DXT5 | texels | lens dirt | data |

### Crosshair values per weapon (weapon scripts)

| weapon | CrosshairMinDistance | CrosshairDeltaDistance | WeaponType | FullAuto |
|---|---|---|---|---|
| weapon_ak47 | 4 | 4 | Rifle | 1 |
| weapon_aug | 3 | 3 | Rifle | 1 |
| weapon_awp | 8 | 3 | SniperRifle | 0 |
| weapon_c4 | 6 | 3 | C4 | – |
| weapon_deagle | 8 | 3 | Pistol | 0 |
| weapon_elite | 4 | 3 | Pistol | 0 |
| weapon_famas | 4 | 3 | Rifle | 1 |
| weapon_fiveseven | 8 | 3 | Pistol | 0 |
| weapon_flashbang | 7 | 3 | Grenade | – |
| weapon_g3sg1 | 6 | 4 | SniperRifle | 1 |
| weapon_galil | 4 | 3 | Rifle | 1 |
| weapon_glock | 8 | 3 | Pistol | 0 |
| weapon_hegrenade | 8 | 3 | Grenade | – |
| weapon_knife | 7 | 3 | Knife | – |
| weapon_m249 | 6 | 3 | Machinegun | 1 |
| weapon_m3 | 8 | 6 | Shotgun | 1 |
| weapon_m4a1 | 4 | 3 | Rifle | 1 |
| weapon_mac10 | 9 | 3 | SubMachinegun | 1 |
| weapon_mp5navy | 6 | 2 | SubMachinegun | 1 |
| weapon_p228 | 8 | 3 | Pistol | 0 |
| weapon_p90 | 7 | 3 | SubMachinegun | 1 |
| weapon_scout | 5 | 3 | SniperRifle | 0 |
| weapon_sg550 | 5 | 3 | SniperRifle | 1 |
| weapon_sg552 | 5 | 3 | Rifle | 1 |
| weapon_smokegrenade | 4 | 3 | Grenade | – |
| weapon_tmp | 7 | 3 | SubMachinegun | 1 |
| weapon_ump45 | 6 | 3 | SubMachinegun | 1 |
| weapon_usp | 8 | 3 | Pistol | 0 |
| weapon_xm1014 | 9 | 4 | Shotgun | 1 |

Every script sets both keys. Units: *inferred* pixels at the crosshair's
reference scale (the 640×480 HUD or the `cl_crosshairscale` height).

### Crosshair console variables (names in the CS:S client)

`cl_crosshaircolor` (with `cl_crosshaircolor_r`, `_g`, `_b`),
`cl_crosshairalpha`, `cl_crosshairusealpha`, `cl_crosshairsize`,
`cl_crosshairthickness`, `cl_crosshairdot`, `cl_crosshairscale`,
`cl_scalecrosshair` (the client's strings include a help text for a
"deprecated" crosshair scaling factor, probably one of these two),
`cl_crosshairspreadscale`, `cl_dynamiccrosshair`,
`cl_legacy_crosshair_recoil`, `cl_legacy_crosshair_scale`,
`cl_observercrosshair`, `crosshair`; and, shared with the server,
`weapon_accuracy_model`. The options page has Dynamic, Translucent and Dot
checkboxes, a colour box, and red/green/blue, size and thickness sliders
(`resource/optionssubmultiplayer.res`). Defaults and help texts: Q1.

## Behavior

### 1. Dynamic crosshair (hypotheses; not in the reference)

What is known: the crosshair is four lines around the screen centre with a
gap; `CrosshairMinDistance` and `CrosshairDeltaDistance` are per weapon;
`cl_dynamiccrosshair 0` keeps the gap fixed.

Two hypotheses, to be told apart by Q2's capture:

- **H1 (per-shot, legacy).** The gap starts at `MinDistance`. Each shot
  adds `DeltaDistance` to it, up to a cap; between shots it shrinks back
  toward `MinDistance` at a fixed rate. Moving, crouching and being in the
  air widen or narrow it by fixed multiples. The names
  `cl_legacy_crosshair_recoil` and `cl_legacy_crosshair_scale` suggest this
  older rule is still selectable.
- **H2 (follows inaccuracy).** The gap is `MinDistance` plus the weapon's
  current inaccuracy (weapons.md M1: stored `I` plus the movement term,
  plus `Spread`) converted to screen distance and scaled by
  `cl_crosshairspreadscale`. It then grows on each shot by `InaccuracyFire`,
  jumps on a jump by `InaccuracyJump`, and recovers with `RecoveryTime*`,
  exactly as the inaccuracy does. `DeltaDistance` would then belong to H1
  only. Our current code does this.

Either way: while scoped (Behavior 2) the crosshair is not drawn
(*inferred*); a spectator sees it in first-person view only with
`cl_observercrosshair 1` (*inferred* from the name).

### 2. Scope overlay (layout inferred)

**When.** *Inferred:* the overlay shows while the local player (or the
player a spectator watches in first person) holds a `SniperRifle`
(AWP, Scout, SG550, G3SG1) and its FOV is below the default (zoom levels
40, 10 or 15; weapons.md M15). The AUG and SG552 zoom to 55 without an
overlay. The HUD element is `HudScope` (full 640×480 in `hudlayout.res`).

**Materials** (both unlit, translucent, drawn without depth test):

- `sprites/scope_arc`: a 256×256 mask. Its transparent part is a quarter
  disc centred on the texture's **top-left** corner with radius = the
  texture's width; everything farther from that corner is opaque black.
  The edge is soft: alpha rises from 0 to full over roughly the last 8 % of
  the radius. It takes vertex colour and alpha.
- `overlays/scope_lens`: a 512×512 texture of faint brownish dirt (average
  colour about (59, 41, 18) of 255), mostly about 25 % opaque, somewhat
  denser toward the edges (alpha about 0.2 to 0.6). It takes vertex alpha.

**Layout** (*inferred* from how the game looks; Q5 confirms numbers). With
screen width W and height H (W ≥ H), centre (cx, cy):

1. The hole is a circle of diameter H centred on the screen.
2. Four copies of `scope_arc`, each drawn over an (H/2)×(H/2) square with
   one corner at the centre: bottom-right as stored, bottom-left mirrored
   horizontally, top-right mirrored vertically, top-left mirrored both
   ways. Together they leave the circle clear and the corners of the
   central H×H square black.
3. Two solid black bars fill the rest: x from 0 to cx − H/2 and from
   cx + H/2 to W, full height. Widths: 4:3 → H/6 each; 16:10 → 0.3·H;
   16:9 → about 0.389·H.
4. `scope_lens` covers the central H×H square (only visible through the
   hole).
5. Two thin black lines through the centre: one horizontal across the
   whole screen width, one vertical across the whole height (*inferred*
   1 px; Q5).

### 3. Zoomed mouse sensitivity (SDK rule)

Evaluated on the client once per rendered frame, before input for the next
command is sampled:

1. `D` = the player's default FOV (90). `F` = the player's current FOV:
   the networked FOV (or `D` if it is 0), smoothed during a zoom change:
   from the FOV at the start of the change to the new one over the change's
   duration, along a smooth "ease in and out" spline (3t² − 2t³ of the
   elapsed fraction t). `F` is then **truncated to an integer**.
2. Raise `F` to at least the minimum FOV (generic: 75 in multiplayer, 5 in
   single player; Q4).
3. If a forced sensitivity factor is set (a HUD override, unused in stock
   CS:S), zoom sensitivity = `sensitivity` × factor and stop.
4. If `F` = `D`: zoom sensitivity = "none" (the plain `sensitivity` is
   used).
5. Otherwise: zoom sensitivity = `sensitivity` × (`F` / `D`) ×
   `zoom_sensitivity_ratio`.

Mouse movement per axis = counts × (zoom sensitivity if set, else
`sensitivity`); then × `m_yaw` (horizontal) or `m_pitch` (vertical)
degrees. The mouse-acceleration modes (`m_customaccel` 1-3) take the same
base sensitivity as their starting value.

Because `F` is the smoothed FOV, sensitivity changes gradually while the
zoom animates, and the integer truncation makes it step one degree at a
time.

## Per-tick order

Per client frame: update the FOV smoothing → recompute the zoom
sensitivity (Behavior 3) → sample the mouse into the next user command →
draw the HUD (crosshair or scope) for this frame.

## Edge cases

- Spectating in first person: the FOV (and so the scope state) is the
  watched player's.
- Demo playback can force a FOV override (clamped 10-90); the sensitivity
  rule then uses that.
- `zoom_sensitivity_ratio` 0 makes zoomed aiming impossible (no clamp).

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| sensitivity 2, ratio 1.2, D 90, no min clamp | F 40 (AWP 1) | – | zoom sensitivity 2 × 40/90 × 1.2 = 1.0667 |
| same | F 10 (AWP 2) | – | 0.2667 |
| same | F 15 (Scout/SG550/G3SG1 2) | – | 0.4 |
| same | F 55 (AUG/SG552) | – | 1.4667 |
| same | F 90 | – | plain sensitivity 2 |
| same, generic multiplayer clamp 75 | F 40 | – | 2 × 75/90 × 1.2 = 2.0 |
| zoom animating, F = 63.7 | – | – | F used = 63 |
| 1920×1080 scoped | – | – | hole diameter 1080 px; bars 420 px each side |
| 1024×768 scoped | – | – | hole 768 px; bars 128 px |
| any weapon, `cl_dynamiccrosshair 0` | fire | – | gap stays at the weapon's rest gap |

## Open questions

1. **Cvar defaults and help texts.** In the CS:S client console run `help`
   on each crosshair cvar, `zoom_sensitivity_ratio`, `weapon_accuracy_model`,
   `cl_legacy_crosshair_recoil`, `cl_legacy_crosshair_scale` and
   `cl_crosshairspreadscale`, and record default and description.
2. **Dynamic crosshair rule** (H1 vs H2). Client capture at a fixed
   resolution (e.g. 1024×768, `cl_crosshairscale` default): per-frame
   screenshots (or `host_framerate`-locked recording) while (a) standing and
   firing 1, 3, 10 shots of the AK-47, M4A1, Deagle, M3; (b) running
   without firing; (c) jumping; (d) crouching. Measure the gap in pixels
   per frame and compare with `MinDistance`, `DeltaDistance` and the
   inaccuracy curve of weapons.md M1. Repeat with each legacy cvar toggled.
3. **CS:S default of `zoom_sensitivity_ratio`** (our code assumes 1.2).
   Q1's `help` answers it.
4. **Minimum FOV for the sensitivity rule in CS:S.** If CS:S kept the
   generic 75, every sniper zoom would scale sensitivity by only 75/90.
   Measure: zoom the AWP twice, move the mouse a fixed number of counts
   (a scripted input), and read the yaw change with `cl_showpos 1`; compare
   with 10/90 and 75/90.
5. **Scope geometry.** Screenshots of a zoomed AWP against a bright sky at
   1024×768, 1280×800 and 1920×1080: measure the hole's diameter and
   centre, the bars, the cross lines' thickness (and whether they scale),
   the lens's extent, and whether the overlay tints with anything. Also:
   does the AUG/SG552 zoom show anything, and does the overlay show during
   the zoom animation or only when it ends?
6. **Zoom animation duration** (the smoothing in Behavior 3, step 1): the
   time from the toggle to the final FOV for each zoom step; capture the
   frames after pressing attack2.
