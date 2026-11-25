# Counter-Strike 1.6: view and HUD

Source basis: the community reimplementation of the CS 1.6 game library
(ReGameDLL_CS, open source; read in a separate spec session as reference
only) for the server-side view parts; the install's HUD sprite list
(`sprites/hud.txt`) and the client's shipped `config.cfg` as data. The
client library (crosshair, HUD drawing, view bob) is not part of that
reference and the client could not be run on this box yet, so the HUD
layout and the dynamic crosshair are left to captures (Open questions).
No code is reproduced here.
Status: draft

## Summary

The view sits 17 units above the player's origin (box centre) standing,
12 above the ducked origin, and eases between them over the 0.4 s duck.
Recoil, hits and landings add a punch angle to the view and the shots;
it decays linearly plus proportionally. Zoom changes the field of view
(90 default). Flashbangs fade the screen white. The HUD is drawn from
sprite sheets listed in `sprites/hud.txt`, at a 320 or 640 layout by the
screen width.

## View

| Item | Value | Notes |
|---|---|---|
| eye, standing | origin + 17 (feet + 53) | movement.md |
| eye, ducked | ducked origin + 12 (feet + 30) | eased over 0.4 s with 3u² − 2u³ |
| eye, dead | origin − 8 | |
| field of view | 90 | reset on every weapon draw |
| zoom AUG, SG552 | 55 | one step |
| zoom AWP | 40, then 10 | |
| zoom Scout, G3SG1, SG550 | 40, then 15 | |
| punch decay | L → max(L − (10 + 0.5 L) dt, 0) | per command, as a vector |
| landing punch | roll = falling speed × 0.013 | when the landing makes a sound (movement.md) |
| hit punch | head: pitch −0.5 × damage (≥ −12), roll ± damage (within ±9); chest/stomach unarmoured: pitch −0.1 × damage (≥ −4) | damage.md |
| strafe roll | none | `sv_rollangle` 0 |

The punch is added to the view angles used both for drawing and for
shooting (weapons.md, "Recoil"). The AWP and Scout drop to 90 on a shot
and return to the previous zoom when the bolt cycle ends.

### Flashbang fade

A blinded player's screen is overlaid with white at an alpha of 255
(flash in front) or 200 (behind), held for the hold time then faded to
nothing over the fade time (weapons.md, "Flashbang"). Spectators
watching that player in first person see the same fade.

## HUD sprites

`sprites/hud.txt` (the install's) starts with the entry count (215),
then one entry per line: name, layout resolution (320 or 640), sprite
file (in `sprites/`, without `.spr`), and the rectangle x, y, width,
height in that sprite. The 640 entries are used at screen widths of 640
and above (ref.). Groups (640 layout):

- numbers `number_0`..`number_9` (20 × 25 cells in `640hud7`),
  `divider`, `dollar`, `plus`, `minus`;
- health `cross`, armour `suit_full`/`suit_empty`,
  `suithelmet_full`/`suithelmet_empty` (24 × 24);
- round and objective icons `c4`, `defuser`, `stopwatch`, `buyzone`,
  `rescue`, `escape`, `vipsafety`, `bombticking`, `hostage` (32 × 32);
- kill-feed weapon icons `d_<weapon id>` (32-57 × 16; `d_headshot`,
  `d_skull`, `d_grenade` for the HE);
- weapon selection `bucket0`..`bucket5`, `selection`;
- the radar `radar` (128 × 128 in `radar640`);
- damage-type icons `dmg_*` (Half-Life's, 64 × 64).

Per-weapon lists `sprites/weapon_<id>.txt` give each gun's selection
and ammunition icons in the same line format.

## Open questions

1. HUD layout (positions, colours, the health/armour/money/ammo
   placement, fades) and the dynamic crosshair (gap growing with
   movement, firing and the `cl_dynamiccrosshair` 1 default; colour
   `cl_crosshair_color` "50 250 50", size "auto", translucent) live in
   the client library: capture them once the client runs (it needs a
   running Steam client and 32-bit libraries this box lacks).
2. View bob (`cl_bob`, `cl_bobcycle`, `cl_bobup`) and the client's own
   view-model punch: read the client's defaults and capture.
3. Whether the client scales the punch differently for the view model
   than for the view.
