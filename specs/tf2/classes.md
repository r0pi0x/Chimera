# Team Fortress 2: the nine classes (T-M2)

Source basis: the dedicated server install (build 11191711), run locally
on a LAN-only port with no clients: one server-side puppet bot per
class, read through the game's public scripting interface (class index,
team number, health and max health, max speed, eye offset, bounds, model)
and measured per tick while their health was set or damaged and while
health kits were spawned on them; the console variable defaults; the
plain-text item schema (`scripts/items/items_game.txt`) for the stock
items; the model archives for model paths. Public community
documentation (the community TF2 wiki: the class pages, Health,
Overheal, Ammo, Metal, Resupply locker, Respawn, Hitbox) for what was not
measured. The class scripts in the archives are encrypted and were not
opened. No code of the game or engine was read.
Status: draft

## Summary

Nine classes share one collision box and one movement controller
(specs/tf2/movement.md) and differ in health (125 to 300), top speed
(230 to 400), eye height (65, 68 or 75) and loadout. Health above the
maximum ("overheal", only from a Medic) is capped at 1.5 × max rounded
down to 5 and drains back to the maximum in 15 s. The Medic alone
regenerates, 3 health a second rising to 6 after he stays undamaged.
Health kits restore 20 %, 50 % or 100 % of max health and never
overheal; ammo packs refill 20 %, 50 % or 100 % of every weapon's
reserve; the resupply cabinet restores everything.

## Units and conventions

- Hammer units, Z up; times in seconds; tick 0.015 s (66.67 Hz).
- Health is an integer. Fractions shown below are internal accumulators.
- **Teams**: the game's team numbers are 0 unassigned, 1 spectator,
  **2 RED, 3 BLU** (measured: a bot created on the red team reports 2,
  on blue 3; the same numbers are the maps' `TeamNum` keys, see
  specs/tf2/install.md 2.3). Team colour on models is the skin: 0 RED,
  1 BLU (install.md 4.5).
- **Class index** (as the game exposes it to scripts and to the
  `info_player_teamspawn` class bits, install.md 2.3): 1 Scout,
  2 Sniper, 3 Soldier, 4 Demoman, 5 Medic, 6 Heavy, 7 Pyro, 8 Spy,
  9 Engineer. (0 is "none"; 10 is the unused Civilian.)

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| hull_stand | (−24,−24,0)–(24,24,82) | units | every class |
| hull_duck | (−24,−24,0)–(24,24,62) | units | every class |
| eye_duck | 45 | units | every class |
| overheal_frac | 1.5 | – | overheal cap = 1.5 × max, rounded down to a multiple of 5 |
| overheal_drain_time | 15 | s | time for a full overheal to drain |
| medic_regen_min / max | 3 / 6 | health/s | Medic only |
| medic_regen_ramp | 5 → 10 | s since last damage | rate 3 until 5 s, rising linearly to 6 at 10 s (measured) |
| kit_small / medium / full | 0.2 / 0.5 / 1.0 | × max health | |
| ammo_small / medium / full | 0.2 / 0.5 / 1.0 | × max reserve | public |
| pickup_respawn | 10 | s | health kits and ammo packs (public) |
| respawn_wave | 10 | s | `mp_respawnwavetime` |

## Behavior

### The class table

Measured on fresh puppet bots (stock loadout, full health, standing):

| Class | Index | Max health | Overheal cap | Top speed | Eye standing | Eye ducked | Player model | Arms model |
|---|---|---|---|---|---|---|---|---|
| Scout | 1 | 125 | 185 | 400 | 65 | 45 | `models/player/scout.mdl` | `models/weapons/c_models/c_scout_arms.mdl` |
| Soldier | 3 | 200 | 300 | 240 | 68 | 45 | `models/player/soldier.mdl` | `models/weapons/c_models/c_soldier_arms.mdl` |
| Pyro | 7 | 175 | 260 | 300 | 68 | 45 | `models/player/pyro.mdl` | `models/weapons/c_models/c_pyro_arms.mdl` |
| Demoman | 4 | 175 | 260 | 280 | 68 | 45 | `models/player/demo.mdl` | `models/weapons/c_models/c_demo_arms.mdl` |
| Heavy | 6 | 300 | 450 | 230 | 75 | 45 | `models/player/heavy.mdl` | `models/weapons/c_models/c_heavy_arms.mdl` |
| Engineer | 9 | 125 | 185 | 300 | 68 | 45 | `models/player/engineer.mdl` | `models/weapons/c_models/c_engineer_arms.mdl` |
| Medic | 5 | 150 | 225 | 320 | 75 | 45 | `models/player/medic.mdl` | `models/weapons/c_models/c_medic_arms.mdl` |
| Sniper | 2 | 125 | 185 | 300 | 75 | 45 | `models/player/sniper.mdl` | `models/weapons/c_models/c_sniper_arms.mdl` |
| Spy | 8 | 125 | 185 | 320 | 75 | 45 | `models/player/spy.mdl` | `models/weapons/c_models/c_spy_arms.mdl` |

- The overheal cap column is the wiki's; it equals ⌊1.5 × max / 5⌋ × 5
  (Scout 187.5 → 185, Pyro 262.5 → 260).
- Health, speed, eye heights and models are the bots' measured values;
  they agree with the wiki's classes table (speeds as % of 300) and the
  Valve level-design reference's view heights. The Demoman's and Heavy's
  model files are `demo.mdl` and `heavy.mdl`, not the class names.
- The eye height does not come from the model: the models' own header
  eye positions are unrelated values (the game sets the view offset per
  class).
- The Engineer's Gunslinger replaces the arms with
  `models/weapons/c_models/c_engineer_gunslinger.mdl` (an unlock; later).
- Every class uses the same collision box (also used for projectiles,
  melee and the map), 48 × 48 × 82 standing and 48 × 48 × 62 ducked
  (measured on all nine). Bullets use the model's hitboxes instead
  (Hitboxes).

### Stock loadout

From `items_game.txt` (item `name`, defindex), all `baseitem` 1; models
and item classes are listed in install.md 5.3.

| Class | Primary | Secondary | Melee | PDA / other |
|---|---|---|---|---|
| Scout | Scattergun, 13 | Pistol, 23 (`TF_WEAPON_PISTOL_SCOUT`) | Bat, 0 | – |
| Soldier | Rocket Launcher, 18 | Shotgun, 10 (`TF_WEAPON_SHOTGUN_SOLDIER`) | Shovel, 6 | – |
| Pyro | Flame Thrower, 21 | Shotgun, 12 (`TF_WEAPON_SHOTGUN_PYRO`) | Fire Axe, 2 | – |
| Demoman | Grenade Launcher, 19 | Stickybomb Launcher, 20 | Bottle, 1 | – |
| Heavy | Minigun, 15 | Shotgun, 11 (`TF_WEAPON_SHOTGUN_HWG`) | Fists, 5 | – |
| Engineer | Shotgun, 9 (`TF_WEAPON_SHOTGUN_PRIMARY`) | Pistol, 22 | Wrench, 7 | Construction PDA, 25 (slot `pda`); Destruction PDA, 26 (`pda2`); Toolbox, 28 (`building`) |
| Medic | Syringe Gun, 17 | Medi Gun, 29 | Bonesaw, 8 | – |
| Sniper | Sniper Rifle, 14 | SMG, 16 | Kukri, 3 (`TF_WEAPON_CLUB`) | – |
| Spy | – | Revolver, 24 (slot `secondary`, used as the Spy's first weapon) | Knife, 4 | Disguise Kit, 27 (`pda`); Invisibility Watch, 30 (`pda2`); Sapper, 735 (`building`) |

The 190–212 "Upgradeable" copies and 199 (multi-class Shotgun) are the
same weapons as named items (`baseitem` 0); a fresh player gets the
`baseitem` 1 items above.

### Overheal

- Only healing sources that allow it (the Medic's Medi Gun; a dispenser
  and kits never overheal) can raise health above max, up to the cap
  above.
- **Decay:** whenever health is above max, it drains toward max at a
  constant rate of (cap − max) / 15 health per second, applied as whole
  points with the fraction carried, so a full overheal is gone in 15 s
  whatever the class. Measured: a Heavy set to 450 loses 1 point every
  0.1 s (10 per second: 449 after 6 ticks, then one every 6–7 ticks, 400
  at 3.0 s); a Scout set to 185 loses 1 every 0.25 s (4 per second) and
  is back to 125 after 14.985 s.
- Decay stops at max; it never takes health below max.
- Whether decay pauses while a Medic keeps healing the player is not
  measured (Open questions 2).

### Medic regeneration

The Medic heals himself once a second (not other classes):

```math
r(t) = 3 + 3\cdot\operatorname{clamp}\!\left(\frac{t - 5}{5},\, 0,\, 1\right)\ \text{health/s}
```

with t the seconds since he last took damage. Each regeneration tick
adds the whole part of (carried fraction + r) and carries the rest; it
stops at max (no overheal). Measured after a 100-point hit at full
health (140 → 40; the regeneration timer runs on its own one-second
cycle, so the first gain came 0.57 s after the hit): +3 at 0.57, 1.58,
2.58, 3.59, 4.59 and 5.60 s, then +4 (6.60 s), +4 (7.60), +6 (8.61),
+5 (9.61), +6 and +6 every second after. The model above (rate 3 to
5 s, linear to 6 at 10 s, floor with carry) reproduces all 16 samples;
the wiki says "3 per second, rising over ten seconds to 6", which
describes the end points but not the 5 s plateau (Open questions 3). The
wiki also says healing a patient doubles it; not measured (later, with
the Medi Gun).

### Health kits

Entities `item_healthkit_small`, `item_healthkit_medium`,
`item_healthkit_full` (measured: spawned on a hurt bot), models
`models/items/medkit_small.mdl`, `medkit_medium.mdl`, `medkit_large.mdl`.

- Gives 20 % / 50 % / 100 % of max health, never above max (no
  overheal); a player at or above max cannot take one (public).
- Halves round up: measured at 5 health, a Scout gained 25 / 63 / 120
  (to 125), a Demoman 35 / 88 / 170 (to 175). Since every base max
  health is a multiple of 25, only .5 fractions occur with base classes
  (Open questions 4).
- A kit also removes burning and bleeding, even from an overhealed player
  (public, 2012 change).
- Respawns 10 s after being taken (public).

| Class | Small | Medium | Full |
|---|---|---|---|
| Scout, Engineer, Sniper, Spy (125) | 25 | 63 | 125 |
| Soldier (200) | 40 | 100 | 200 |
| Pyro, Demoman (175) | 35 | 88 | 175 |
| Heavy (300) | 60 | 150 | 300 |
| Medic (150) | 30 | 75 | 150 |

### Ammo packs

Entities `item_ammopack_small`, `item_ammopack_medium`,
`item_ammopack_full` (names from public entity documentation; not
spawned in this session), models `models/items/ammopack_small.mdl`,
`ammopack_medium.mdl`, `ammopack_large.mdl`.

- Refills 20 % / 50 % / 100 % of each weapon's **reserve** capacity (not
  the loaded clip), all weapons at once (public). Engineer metal: 20 % /
  50 % / 100 % of the 200 maximum = 40 / 100 / 200.
- A dead player drops a medium ammo pack (gives 50 % reserve, and 100
  metal); weapons cannot be picked up for ammo (public, since 2015).
- Respawns 10 s after being taken. Rounding of fractional ammo is not
  documented (Open questions 4).

### Resupply cabinet

A trigger in the spawn room (`func_regenerate` in public entity
documentation, tied to a cabinet prop that opens) that, on touch,
restores full health (not above max), full reserve ammo, full clips and
200 metal, and removes afterburn, bleeding and the jar effects (public).
It also lets a player change items without changing class (a loadout
matter, later). Not usable during sudden death or by the losing team in
humiliation (public).

### Respawn (reference)

`mp_respawnwavetime` 10, `mp_disable_respawn_times` 0, `mp_forcerespawn`
1. Players respawn in waves; a dead player waits for the wave after the
next, so 10–20 s at the default; small teams (under 8) get shorter waves,
down to 5 s at 3 players; control point ownership shortens it on some
maps (public). A later spec covers the exact schedule.

### Hitboxes

- Movement, projectiles, melee and player-player blocking use the
  shared box (Constants).
- Bullets test the player model's hitboxes (one set per class, 18–19
  boxes; install.md 4.5). The **head is hitgroup 1**, the box on bone
  `bip_head`; that is what a headshot checks (Sniper Rifle, Ambassador;
  later specs). Stock weapons other than the Sniper Rifle deal no
  location-dependent damage (public).
- A disguised Spy keeps his own hitboxes (public).

## Per-tick order

Per player, after movement:

1. Overheal decay (if health > max).
2. Medic regeneration when its one-second timer fires.
3. Pickups touched this tick (kits, ammo packs, resupply).

The order within a tick only matters at the cap and is not measured.

## Edge cases

- A Medic at max health accumulates nothing; regeneration never
  overheals.
- A kit taken at max − 1 health gives 1 (no overheal) and is used up
  (assumed: the public sources only say kits do not overheal; Open
  questions 6).
- Changing class at a resupply cabinet respawns the player (not a
  heal); same-class item changes there keep the health ratio (later).

## Quirks

- Overheal drains at a rate fixed by the cap, not by the amount
  currently above max, so a small overheal disappears in under 15 s.
- The Spy has no primary slot; his Revolver is his first weapon.

## Test cases

| # | Setup | Input | After | Expected |
|---|---|---|---|---|
| 1 | a player of each class spawns | – | – | health = max = 125/200/175/175/300/125/150/125/125 (Scout…Spy, table order); top speed 400/240/300/280/230/300/320/300/320; eye 65/68/68/68/75/68/75/75/75; box 48 × 48 × 82 |
| 2 | each class fully ducked | – | – | eye 45, box height 62 |
| 3 | bot on RED / BLU | – | – | team 2 / 3 |
| 4 | Heavy at 450 | – | 0.09 s / 1 s / 15 s | 449 / 440 / 300 |
| 5 | Scout at 185 | – | 0.24 s / 15 s | 184 / 125 |
| 6 | Medic at 150 takes 100 damage | – | 7 s | 40 + 3·6 + 4 = 62 (the 7th regeneration) |
| 7 | Medic, as case 6 | – | 16 regenerations (15.65 s after the hit) | 113 (gains 3,3,3,3,3,3,4,4,6,5,6,6,6,6,6,6 at one-second intervals) |
| 8 | Scout at 5 health | touches a small / medium / full kit | – | 30 / 68 / 125 |
| 9 | Demoman at 5 | small / medium / full kit | – | 40 / 93 / 175 |
| 10 | Heavy at 300 (full) | walks over a kit | – | kit not taken |
| 11 | Engineer with 0 metal | small / medium / full ammo pack | – | 40 / 100 / 200 metal |
| 12 | Soldier at 150 with 0 reserve rockets | resupply cabinet | – | 200 health, full reserve and clip |
| 13 | Medic at 120, undamaged 12 s | – | next regeneration tick | +6 |

## Open questions

1. **Team colours as RGB** (HUD, names, glow): not in the dedicated
   server install (no client scheme files); read them from the retail
   client's scheme resource files.
2. Does overheal decay continue while a Medic is healing the player
   (the beam outpacing it), and is its rate based on the cap of the
   current max (items that change max health)?
3. **Medic regeneration curve.** Measured: 3/s for the first 5 s, linear
   to 6/s at 10 s, floor with carry. Check the plateau with another hit
   and a finer sampling, and the carry's starting value; check the
   wiki's "doubled while healing".
4. **Rounding of kits and ammo for non-base max values** (round half up
   vs ceiling; only .5 fractions are possible with base classes) and of
   ammo pack fractions (e.g. Scattergun reserve 32 × 0.2 = 6.4).
5. **Ammo pack and resupply entity names**: from public documentation;
   confirm in the stock maps' entity lumps (most are LZMA-compressed,
   install.md 2.1).
6. Pickup refusal when a weapon's reserve is already full but health is
   not (and vice versa): kits and packs are separate entities, so assume
   each is refused only when its own resource is full.
