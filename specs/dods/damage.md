# Day of Defeat: Source: damage, hit zones and being slowed

Source basis: the player models' hitbox sets (read from the dedicated
server's model headers in place), and measurements on the dedicated
server (2026-11-23) with two puppet bots driven by a SourceMod plugin:
the shooter aimed at chosen heights on a stationary target 40, 150 and
1,600 units away and fired single shots; the plugin logged each hit's
hitgroup and pre-scaling damage (the trace-attack hook) and the damage
taken and the target's "slowed until" time (the damage-taken hook).
The target had its health raised so it never died. Measured unless a
line says otherwise.
Status: draft (measured 2026-11-23)

## Summary

DoD:S hit zones are coarser than CS:S's: no stomach zone (the pelvis
and spine count as chest), a head that takes 2.5× damage, and limbs
that take 0.75×. Bullet damage does not fall off with distance. A
melee hit from behind does five times the damage. Every hit slows the
victim for a moment, a leg hit for much longer.

## Units and conventions

Source units (1 unit = 0.0254 m), z up, damage in health points, a
player has 100 health. Tick 0.015 s.

## Hit zones (the player models' `dod` hitbox set)

All twelve class bodies share one skeleton (`ValveBiped`, 56 bones) and
the same 21 hitboxes. Hitgroup ids are Source's (1 head, 2 chest,
3 stomach, 4 left arm, 5 right arm, 6 left leg, 7 right leg).

| Bone | Group | Box min (bone space) | Box max |
|---|---|---|---|
| Pelvis | 2 chest | (−7.75, −3.0, −6.6) | (7.75, 3.0, 4.9) |
| L/R Thigh | 6 / 7 | (3.5, −3.9, −3.9 / −3.1) | (19.5, 3.1, 3.1 / 3.9) |
| L/R Calf | 6 / 7 | (1.52, −3.37, −3.25) | (16.51, 3.63, 3.25) |
| L/R Foot | 6 / 7 | (−1.0, −1.3, −2.0) | (5.0, 2.7, 2.0) |
| L/R Toe0 | 6 / 7 | (−3.2, −1.45, −2.0) | (3.8, 1.05, 2.0) |
| Spine | 2 | (−0.45, −3.0, −7.0) | (4.05, 7.0, 7.0) |
| Spine1 | 2 | (−0.9, −3.0, −7.0) | (4.1, 8.0, 7.0) |
| Spine2 | 2 | (−0.5, −3.9, −7.5) | (7.5, 7.1, 7.5) |
| Spine4 | 2 | (−0.2, −2.8, −7.5) | (3.8, 5.2, 7.5) |
| Neck1 | 1 head | (0.5, −3.7, −2.5) | (3.5, 1.3, 2.5) |
| Head1 | 1 head | (−1.3, −5.15, −3.0) | (7.7, 3.15, 3.0) |
| L UpperArm | 4 | (−3.0, −2.05, −2.45) | (13.0, 2.45, 2.05) |
| L Forearm | 4 | (−0.4, −2.0, −2.8) | (11.2, 2.2, 1.8) |
| L Hand | 4 | (0.0, −1.65, −1.45) | (5.0, 0.35, 2.05) |
| R UpperArm | 5 | (−3.0, −2.05, −2.05) | (13.0, 2.45, 2.45) |
| R Forearm | 5 | (−0.4, −1.9, −1.8) | (11.2, 2.1, 2.4) |
| R Hand | 5 | (0.0, −1.65, −2.05) | (5.0, 0.35, 1.45) |

A second set, `dod_no_arms`, is the same without the six arm boxes
(15 boxes). When the game uses it is an open question (1).

**There is no stomach group (3)**: no box carries it.

## Damage scaling by hit zone (measured)

| Zone | Multiplier | Measured |
|---|---|---|
| head (1) | 2.5 | Garand 80 → 200, at 150 and 800 units |
| chest (2) | 1.0 | every gun, e.g. K98 110 → 110, Garand 80 → 80 |
| arms (4, 5) | 0.75 | Thompson 40 → 30, StG44 50 → 37.5 |
| legs (6, 7) | 0.75 | K98 110 → 82.5, BAR 50 → 37.5, MG42 85 → 63.75, knife 60 → 45 |

Damage is a real number (37.5 and 63.75 were dealt as such; how health
rounds them is open question 2). The server's `sk_player_head` (2),
`sk_player_chest`, `_stomach`, `_arm`, `_leg` (1) cvars exist but do
not give these numbers: DoD:S's own multipliers apply.

## Distance

No falloff: the K98 did 110, the Garand 80, the Thompson 40, the M1
Carbine 40 to the chest at 1,600 units, as at 150. (The weapon scripts
have no range field; the trace length is an open question 3.)

## Melee

- Knife and spade: 60 from the front (chest), 45 to the legs.
- **From behind: 300** (5×), to the chest or the legs (the leg hit
  then scales: 300 → 225). "Behind" was the target facing straight
  away; the angle that counts as behind is open question 4.
- Reach: hits at 40 units; at a 60-unit aim height above the target's
  feet the swing missed (the head was out of the swing's path or reach).
- Damage type: club + never-gib bits (4224 as the server reports it);
  bullets are 2 (bullet), the MG42's bullets carry an extra high bit
  (1073741826).

## Being slowed when hit

Each hit sets the victim's "slowed until" time ahead of the current
time (measured on the target right after the hit):

| Hit zone | Slowed for |
|---|---|
| leg (6, 7) | 2.0 s |
| any other (head, chest, arms) | 0.5 s |

How much the victim is slowed (a speed cap or scale while the time
runs) was not measured: open question 5.

## Friendly fire

`mp_friendlyfire` 0 by default; `dod_friendlyfiresafezone` 100: within
100 units of a teammate no team damage is dealt even with friendly fire
on (the cvar's own description; not measured).

## Test cases

| Setup | Input | Expected |
|---|---|---|
| Garand (80) hits the head | one shot, 150 u | 200 damage, slowed 0.5 s |
| K98 (110) hits a leg | one shot | 82.5 damage, slowed 2.0 s |
| Thompson (40) hits an arm | one shot | 30 damage, slowed 0.5 s |
| K98 hits the chest at 1,600 u | one shot | 110 (no falloff) |
| knife, target facing away | one swing at 40 u | 300 |
| spade, target facing the attacker, leg | one swing | 45 |
| hit on the pelvis | one shot | chest (×1) |

## Open questions

1. When the arm-less hitbox set is used (perhaps while a player is
   prone or deployed, so the arms don't shield the head).
2. Rounding of fractional damage into health (37.5 dealt: health
   dropped by 37 or 38?). The plugin logged health after the hit only
   with a raised pool; re-run reading the health delta.
3. The bullets' maximum range.
4. The backstab cone; whether a crouched or prone victim changes it.
5. The slow's effect on speed (cap, factor, ramp).
6. Penetration: every script has `Penetration` 1; walls were not tested.
7. Explosives (grenades 180, rifle grenades, rockets `mp_rocketdamage`
   150 / `mp_rocketradius` 200): radius falloff and self-damage not
   measured. Fall damage: `mp_falldamage` 0 (the classic Source fall
   damage of CS:S's spec applies unless measured otherwise).
