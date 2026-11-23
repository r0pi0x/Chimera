# Day of Defeat: Source: classes and their loadouts

Source basis: the dedicated server's per-class scripts
(`scripts/playerclass_*.txt`, all twelve read in place), the server's
cvar list, and puppet-bot measurements on the dedicated server
(2026-11-23: each Allied class spawned and run forward; see
movement.md for the method). Values from the scripts are data the game
exposes and are given to check against; the game supplies them at run
time.
Status: draft (scripts read and partly measured 2026-11-23)

## Summary

Each team has six classes. A class fixes the player's body, primary
weapon, sidearm or rifle grenade, melee weapon and grenades; there is no
buying and no picking up of other primaries during a life (a dropped
primary can be picked up; open question 3). Classes are mirrored across
the teams: the same role has the same shape of loadout with its own
nation's guns. Every class measured moves at the same base speed.

## The classes

Team numbers: 2 Allies (US), 3 Axis. Class index as the server reports
it (`m_iPlayerClass` after joining) is 0 for the riflemen; the order of
the rest is an open question (2).

| Role | Allies class (select command) | Axis class (select command) | Limit cvar suffix |
|---|---|---|---|
| Rifleman | `cls_garand` | `cls_k98` | `rifleman` |
| Assault | `cls_tommy` | `cls_mp40` | `assault` |
| Support | `cls_bar` | `cls_mp44` | `support` |
| Sniper | `cls_spring` | `cls_k98s` | `sniper` |
| Machine gunner | `cls_30cal` | `cls_mg42` | `mg` |
| Rocket | `cls_bazooka` | `cls_pschreck` | `rocket` |

Class limits: `mp_limit_<team>_<suffix>` (`allies` / `axis`), default
-1 (no limit). `mp_allowrandomclass` 1 allows a random pick;
`mp_combinemglimits` 0 (a leftover for two Axis MG classes).

## Loadouts (the scripts' fields)

Fields per class: `primaryweapon`, `secondaryweapon`, `meleeweapon`,
`grenadetype` + `numgrens`, `grenadetype2` + `numgrens2`, `playermodel`,
`helmetgroup` 0, `hairgroup` 1, `drophelmet`, `limitcvar`, the MG42
class's `mergemgclass` 1.

| Class | Primary | Secondary | Melee | Grenades | Body model |
|---|---|---|---|---|---|
| US Rifleman | garand | riflegren_us | amerknife | none | american_rifleman |
| US Assault | thompson | colt | none | 1 frag_us + 1 smoke_us | american_assault |
| US Support | bar | none | amerknife | 2 frag_us | american_support |
| US Sniper | spring | colt | amerknife | none | american_sniper |
| US MG | 30cal | colt | amerknife | none | american_mg |
| US Rocket | bazooka | m1carbine | amerknife | none | american_rocket |
| Axis Rifleman | k98 | riflegren_ger | spade | none | german_rifleman |
| Axis Assault | mp40 | p38 | none | 1 frag_ger + 1 smoke_ger | german_assault |
| Axis Support | mp44 | none | spade | 2 frag_ger | german_support |
| Axis Sniper | k98_scoped | p38 | spade | none | german_sniper |
| Axis MG | mg42 | p38 | spade | none | german_mg |
| Axis Rocket | pschreck | c96 | spade | none | german_rocket |

Body models are `models/player/<name>.mdl`. The weapons' numbers are in
weapons.md. Notes:

- The Assault classes have **no melee weapon**; their SMG's alt fire is
  the melee (weapons.md, open question).
- The riflemen's "secondary" is the rifle grenade launcher, a separate
  weapon (2 rounds, weapons.md), not a pistol.
- The Rocket classes carry the M1 Carbine (US) and C96 (Axis) as
  sidearms.

## Health and speed

- Every class spawns with 100 health (measured, all six Allied classes
  and the Axis rifleman).
- Run speed 220 u/s for every class measured, holding its primary: the
  Thompson, BAR, Springfield, .30 cal, bazooka and Garand (Allies), the
  K98 and MP40 (Axis). DoD:S's speed does **not** depend on the class or
  the held gun in these cases (unlike TF2).
- The rocket classes with the launcher shouldered move slower
  (weapons.md, "Rockets"); the MG classes cannot move while deployed.

## Test cases

| Setup | Expected |
|---|---|
| join Allies as Rifleman | primary garand, secondary riflegren_us, melee amerknife, no grenades, 100 hp |
| join Axis as Assault | mp40, p38, no melee, 1 frag_ger and 1 smoke_ger |
| any class, hold primary, run forward | 220 u/s |

## Open questions

1. The Axis classes other than the rifleman and assault were not
   measured for speed (the probe's team switch did not respawn the bot
   in time); assume 220 until measured.
2. The class index order (0 = rifleman) for the other five classes.
3. Picking up a dead player's primary (and whether it changes the
   class's look): not measured.
4. Health regeneration: none seen; `mp_bandage_heal_amount` 40 exists
   but no class carries bandages (likely unused).
