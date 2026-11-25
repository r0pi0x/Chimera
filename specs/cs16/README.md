# Counter-Strike 1.6: specs

What Counter-Strike 1.6 does, in our own words, for the implementation sessions
that build `games/cs16` (docs/CONTRIBUTING-GAMES.md). Written by spec
sessions only, by the rules of [specs/README.md](../README.md): no code,
no pseudocode that mirrors it, no names, comments or strings copied from
the game's code, no keys, no assets or extracted files.

| Spec | Covers | Status |
|---|---|---|
| [install.md](install.md) | where the install comes from, its layout, how files are found, the file formats by role | draft |
| [movement.md](movement.md) | the player's movement: cvar defaults, speed by weapon, jump penalty, duck, ladders, water, fall damage, per-command order, test cases | draft |
| [weapons.md](weapons.md) | every gun: numbers, accuracy and spread, recoil kicks, fire modes, zoom, silencers, reloads, knife, grenades | draft |
| [damage.md](damage.md) | hit zones, armour by weapon, range falloff, wall penetration, blasts, flinch | draft |
| [view_and_hud.md](view_and_hud.md) | view heights, zoom, punch, flash fade, HUD sprites (layout and crosshair still to capture) | draft |
| [hud.md](hud.md) | the HUD: health, armour, ammunition, money (unused), timer, crosshair, weapon menu, kill feed icons, damage direction; 640 layout, colour and fades | draft |

Add one file per topic (sounds, a format, the round rules) as the
work needs it, from the template in [specs/README.md](../README.md).
