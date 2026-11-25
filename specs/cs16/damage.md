# Counter-Strike 1.6: damage

Source basis: the community reimplementation of the CS 1.6 game library
(ReGameDLL_CS, open source; read in a separate spec session as reference
only, its original-game behaviour, never its optional fixes or
additions), then checked against the game: the hit boxes and their
groups in all nine player models of the install, and the server's
console variables (HLDS build 10211, `mp_friendlyfire` 0). Numbers
marked "(ref.)" were not measured in the running game. No code is
reproduced here.
Status: draft

## Summary

A bullet is a line trace that may pass through up to a gun-specific
number of surfaces. At each hit its damage is first reduced for the
distance travelled (range modifier to the power distance/500, in whole
points), then scaled by the hit zone, then split between health and
armour by an armour ratio that depends on the **attacker's weapon**.
Walls cost the bullet penetration power per material and a damage
factor. Grenade blasts fall off linearly and ignore walls.

## Units and conventions

- Health 100, armour 0-100, damage in points. Health only ever loses
  whole points: every damage event is truncated to an integer before it
  is subtracted (a hit worth 27.9 takes 27).
- Distances in units; a player's position for blasts is the origin (the
  box centre).

## Hit zones

The player models' hit boxes (21 per model, the same groups in all nine
models of the install, checked) give the zone:

| Group | Zone | Boxes (on the skeleton) | Damage multiplier | Armour protects |
|---|---|---|---|---|
| 0 | generic | none in the models; blasts and falls use it | 1 | yes (any armour) |
| 1 | head | head | 4 | only with a helmet |
| 2 | chest | upper spine ×2, neck | 1 | yes |
| 3 | stomach | pelvis, lower spine | 1.25 | yes |
| 4 | left arm | left clavicle, upper arm, forearm, hand | 1 | yes |
| 5 | right arm | right clavicle, upper arm, forearm, hand | 1 | yes |
| 6 | left leg | left thigh, calf, foot | 0.75 | no |
| 7 | right leg | right thigh, calf, foot | 0.75 | no |
| 8 | shield | a large box on the left hand | bullets stop, no damage | |

A hit on the shield zone of a player carrying a shield (and not holding
the C4) does no damage, stops the bullet, plays a ricochet and sparks,
and kicks the shield holder's view pitch by damage × U(−0.15, 0.15)
(the reference then forces it to −4 whenever it is below 4: a bug that
always kicks up by 4; kept as reference behaviour, open question).

### The victim's view kick on a hit (ref.)

Only when the hit "bleeds" (no armour on that zone, not a teammate with
friendly fire off):

- head (no helmet): punch pitch = −0.5 × damage (after × 4), not below
  −12; punch roll = damage × U(−1, 1), within ±9;
- chest or stomach (no armour): punch pitch = −0.1 × damage, not below
  −4.

A helmeted head hit sparks instead of bleeding.

## Armour

Kevlar alone protects zones 0, 2, 3, 4, 5; with a helmet also the head.
Legs are never protected. When the armour reaches 0 the player loses
the helmet too.

### Bullets and the knife

D is the damage after the hit-zone multiplier. The armour ratio R (the
share that still goes to health):

R = (0.5, + 0.2 if the **victim** carries a shield; 0.25 instead of 0.5
for the VIP) × the attacker's current weapon's multiplier:

| Attacker's weapon | Multiplier | R (no shield) |
|---|---|---|
| AWP | 1.95 | 0.975 |
| knife, Scout | 1.7 | 0.85 |
| G3SG1 | 1.65 | 0.825 |
| AK-47, Galil | 1.55 | 0.775 |
| Deagle, M249, P90, Five-SeveN | 1.5 | 0.75 |
| SG550 | 1.45 | 0.725 |
| M4A1, AUG, FAMAS, SG552 | 1.4 | 0.7 |
| P228 | 1.25 | 0.625 |
| Glock, Elites | 1.05 | 0.525 |
| MAC-10 | 0.95 | 0.475 |
| others (USP, TMP, MP5, UMP45, M3, XM1014) | 1 | 0.5 |

With armour A > 0 on a protected zone:

- to health H = R × D; armour cost C = (D − H) × 0.5;
- if C > A: the armour is used up (A = 0) and H = D − 2A;
- else A −= C (A is kept as a fraction internally, shown whole), with
  C at least 1 if it were negative.
- Health loses trunc(H).

### Blasts (HE grenade, C4)

Explosion damage to players: zone generic, R = 0.5 (VIP 0.25), cost
factor 0.5; if C > A: H = D − A (not 2A) and A = 0; else as above.

### Not absorbed

Falling and drowning damage ignore armour.

## Teams

- Friendly fire off (`mp_friendlyfire 0`, the default): a teammate's
  bullets and knife do nothing (no view kick either). A teammate's HE
  grenade does nothing; the thrower's own does.
- Friendly fire on: teammates' damage (bullets, knife) × 0.35, after the
  zone multiplier and before armour.

## Range falloff

At each surface a bullet hits, with d the length of the segment the
bullet travelled since it started or last penetrated:

  damage = trunc(damage × range_modifier^(d / 500))

The damage carried along is a whole number throughout (each step
truncates). Range modifiers per gun: weapons.md. Example: AK-47 (36,
0.98) at 1000 units: trunc(36 × 0.98²) = trunc(34.57) = 34.

Shotgun pellets have no range modifier: each does trunc(20 × (1 −
d / range)).

## Wall penetration

A bullet gets a **count** (how many surfaces it may hit: 1, 2 or 3 by
gun) and, from its bullet type, a **power** and a **maximum distance**:

| Bullet type | Power (units) | Max distance (units) | Guns |
|---|---|---|---|
| 9 mm | 21 | 800 | Glock, Elites, TMP, MP5 |
| .45 ACP | 15 | 500 | USP, MAC-10, UMP45 |
| .357 SIG | 25 | 800 | P228 |
| .50 AE | 30 | 1000 | Deagle |
| 5.7 mm | 30 | 2000 | Five-SeveN, P90 |
| 5.56 mm | 35 | 4000 | Galil, FAMAS, M4A1, SG552, AUG, SG550, M249 |
| 7.62 mm | 39 | 5000 | AK-47, Scout, G3SG1 |
| .338 Magnum | 45 | 8000 | AWP |

The bullet's damage factor after a hit starts at 0.5. Then, for each
segment (at most count segments):

1. Trace a line from the current start to start + direction × remaining
   distance (the first: from the eye, distance 8192 or 4096). It hits
   players' hit boxes and the world/brush entities.
2. The material of the surface hit (from the texture's entry in
   `materials.txt`, specs/goldsrc/mount.md; a living thing counts as
   flesh) scales the power (kept as a whole number, truncated) and sets
   the damage factor:

   | Material (letter) | Power × | Damage factor |
   |---|---|---|
   | metal (M) | 0.15 | 0.2 |
   | concrete (C) | 0.25 | unchanged |
   | grate (G) | 0.5 | 0.4 |
   | vent (V) | 0.5 | 0.45 |
   | tile (T) | 0.65 | 0.3 |
   | computer (P) | 0.4 | 0.45 |
   | wood (W) | 1 | 0.6 |
   | anything else (dirt, glass, snow, slosh, flesh, unknown) | 1 | unchanged |

3. Nothing hit: the bullet ends.
4. Count −= 1. Segment length d = the hit fraction × the remaining
   distance. Range falloff (above) on the carried damage. If d is above
   the bullet type's maximum distance, this is the last hit (count = 0).
5. If the thing hit is not the world or a brush entity (a player), or
   this is the last hit: the power becomes 42 (for the rest of the
   bullet), the damage factor 0.75, and the distance factor 0.75.
   Otherwise the distance factor is 0.5.
6. The thing hit takes the carried damage (hit zone, armour and so on).
7. The next segment starts power units beyond the hit point along the
   direction (no check that this point is out of the wall: a line that
   starts inside a solid passes out of it freely and only stops on
   entering the next solid, so the wall's thickness beyond the power
   distance is not tested (ref., see Open questions)). The remaining
   distance becomes (remaining − d) × the distance factor.
8. The carried damage × the damage factor (truncated).

Each segment's damage is applied to each entity as one event (hits on
the same player from one segment summed).

Example: AK-47 (36, count 2, 7.62 mm) through a wooden crate 200 units
away into a player 100 units behind it: first hit trunc(36 × 0.98^0.4)
= 35 on the crate (factor 0.6 → 21); the next segment starts 39 units
inside, its length to the player ≈ 61: trunc(21 × 0.98^0.122) = 20 to
the player, times the zone multiplier.

## Blast damage (HE)

Damage 100 at the explosion point (moved off the surface, weapons.md),
radius 350 = 100 × 3.5. Each entity that can take damage whose origin is
within the radius takes 100 − distance × (100 / 350), no line of sight
needed (walls do not block). Not into or out of water: an entity in
water when the blast is not, or deep in water (eyes under) when the
blast is dry, is skipped. Damage through the blast armour rule. (A
weaker blast, under 80, uses a different rule with a random radius and
line of sight; no CS grenade uses it.)

## Being hit: flinch and slowdown

Every damage event from a player's bullet or knife (ref.):

- Large flinch if the zone is not a leg, the victim is not ducked, and
  the attacker holds a Scout, AUG, SG550, Galil, FAMAS, AWP, M3, M4A1,
  G3SG1, Deagle, SG552 or AK-47: if the victim's speed is below 300,
  push them 170 units/s away from the attacker (horizontal and vertical,
  along the attacker-to-victim line) and set their speed modifier to
  0.65.
- Otherwise: speed modifier 0.5.
- Recovery: movement.md, "Being hit".

## Fall damage

movement.md, "Fall damage and landing": (falling speed − 500) / 4.8,
truncated, no armour.

## Test cases

| Setup | Hit | Expected |
|---|---|---|
| AK-47 at 0 range, no armour | chest | 36 |
| AK-47 at 0 range, no armour | head | 144 |
| AK-47 at 0 range, no armour | stomach | 45 |
| AK-47 at 0 range, no armour | leg | 27 |
| AK-47 at 0 range, armour 100 | chest | health −27 (27.9), armour 95.95 (shown 95) |
| AK-47 at 0 range, armour 100, no helmet | head | 144 (no protection) |
| AK-47 at 0 range, armour 100 + helmet | head | health −111 (111.6), armour −16.2 |
| AWP at 0 range, armour 100 | chest | health −112 (112.125) |
| USP (34) at 0 range, armour 100 | chest | health −17, armour −8.5 |
| AK-47 at 1000 units, no armour | chest | 34 |
| Deagle (54, 0.81) at 500 units, no armour | chest | 43 (43.74) |
| AK-47, armour 2, chest at 0 range | | cost 4.05 > 2: health −32, armour 0, helmet lost |
| HE at 0 units, no armour | | 100 |
| HE at 0 units, armour 100 | | health −50, armour 75 |
| HE at 175 units, armour 10 | | D 50: H 25, C 12.5 > 10: H = 40, armour 0 |
| HE with friendly fire off, thrown by a teammate | | none |
| HE thrown by the victim | | damage as usual |
| friendly fire on, teammate's AK-47, chest, no armour | | trunc(36 × 0.35) = 12 |
| AK-47 bullet hitting metal then a player behind | | power 39 × 0.15 = 5 units jumped; damage factor 0.2 |
| Pistol (count 1) at a wall | | no penetration |
| Knife stab from behind, armour 100, chest | | trunc(195 × 0.85) = 165 |

## Open questions

1. None of these numbers were measured in the running game yet. With a
   working client: shoot a bot or a second client at measured distances
   with `mp_logdetail 3` (the server logs each hit's health and armour
   damage), and wallbang crates and doors to check the penetration
   table, the start-inside-solid rule and thick walls.
2. Whether the shield hit box (group 8, present in every player model)
   counts when its owner carries no shield (then it would be an
   unprotected ×1 zone) or the engine ignores it.
3. The shield view-kick rule as the reference has it (always −4).
4. Textures missing from `materials.txt` fall in the "anything else"
   row (power and factor unchanged); confirm on a map.
