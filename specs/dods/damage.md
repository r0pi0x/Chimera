# Day of Defeat: Source: damage, hit zones and being slowed

Source basis: the player models' hitbox sets (read from the dedicated
server's model headers in place), and measurements on the dedicated
server (2026-11-23) with two puppet bots driven by a SourceMod plugin:
the shooter aimed at chosen heights on a stationary target 40, 150 and
1,600 units away and fired single shots; the plugin logged each hit's
hitgroup and pre-scaling damage (the trace-attack hook) and the damage
taken and the target's "slowed until" time (the damage-taken hook).
The target had its health raised so it never died. The slow's speed and
the explosions were measured on 2026-11-23 with `tools/dods_probe`
(the slowed-until time set on a running bot; grenades and rockets set
off beside a second bot); melee reach, the backstab cone, the SMGs'
stock strike and bodies in a blast's way on 2026-11-24 (`measure.py
stock reach backstab shield rocketworld`), the backstab on crouched and
prone victims and its timing later the same day (`measure.py
backstab2`). Measured unless a line says
otherwise.
Status: draft (measured 2026-11-23 and 2026-11-24)

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
  then scales: 300 → 225).
- **What counts as behind** (measured with standing victims, pitch 0
  on their side): the dot product of the attacker's aim direction (with
  its pitch) and the victim's horizontal facing is **above 0.3**. It is
  the attacker's aim that counts, not the line between them: a victim
  40 to the left and 40 ahead facing 105° off the attacker's yaw (60°
  off the line) took 60, facing 60° off the yaw (105° off the line)
  took 300.

| Attacker's pitch | Backstab up to (victim facing off the attacker's yaw) | Not from | cos(pitch) × cos(limit) |
|---|---|---|---|
| 0 | 72.5° | 73° | 0.300 |
| 20° down | 71° | 71.5° | 0.306 / 0.298 |
| 40° down | 66° | 68° | 0.312 / 0.287 |

- **Crouched and prone victims** (measured 2026-11-24): the same rule.
  A crouched victim 40 ahead: backstab up to 71° (pitch 20° down) and
  66° (40° down), not from 71.5° / 68°, as standing. A prone victim 50
  ahead, the attacker's aim 45° down: backstab facing 0° and 60° off,
  not 66°, 70° or 180° (cos 45° × cos 64.9° = 0.3); 40 ahead with the
  aim 45° or 55° down, facing away: backstab.
- **The hit lands 14 ticks (0.21 s) after the press**, and a swing
  blocks every attack for 0.4 s from the press (knife, spade and the
  SMGs' stock strike alike; the knife script's `FireDelay` 0.1 is not
  the swing's interval). **A backstab is a slower swing** (measured
  2026-11-24, knife and spade, standing, crouched and prone victims):
  its 300 lands **27 ticks (0.405 s)** after the press and it blocks
  attacks for **0.9 s**; the kind of swing is settled on the press (a
  victim facing away in reach then), not when it lands.
- **Reach** (horizontal distance between the two players' origins, both
  standing, the attacker's aim level): hits up to **92**, misses from
  **94** (knife, spade and Thompson stock alike); with the aim 15° down
  90 hits, 92 misses. Aim level at 80 units hits the head (150 from the
  front), 15° down the chest. The swing also hit a victim 45° off the
  aim at 56 units (40 ahead, 40 to the side): it is not a thin line
  along the aim (its shape is open question 4).
- **The SMGs' stock strike** (Thompson, MP40 alt fire) is a melee hit
  of 60 before zones with the knife's timing and reach, and **no
  backstab** (60 from behind, measured).
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

While the time runs the victim's horizontal speed is held to **120 u/s**
(measured, `tools/dods_probe/measure.py slow`):

- running at 220: the speed drops to 120.00 on the next tick (a cap on
  the speed itself, not on the acceleration: no friction ramp), holds
  there, and when the time is up the normal acceleration takes it back
  to 220 (138.6 four ticks after, 193.2 eight after, 220 by twelve);
- sprinting: the same cap of 120, and the sprint still drains stamina
  (0.3 a tick) while the cap holds;
- below 120 nothing changes: ducked (73.3) and slow walking (100) keep
  their speed;
- a slow doesn't change the stamina.

## Explosions (measured)

Grenades and rockets do blast damage around the point where they go off.
Each victim gets two damage events on the same tick:

1. the **blast** (damage type 64): health damage, falling off linearly
   with distance, `damage × (1 − r / radius)`;
2. a **stun** event (damage type 32768) of two thirds of that amount that
   takes **no health** (health unchanged after it, measured); it feeds
   the view shake of the `dod_stun_*` cvars (how is unmeasured).

| Explosive | Damage | Radius | Source |
|---|---|---|---|
| frag and stick grenade | 150 | 375 | the grenade entity's damage and radius fields, and the falloff below |
| rifle grenade (US and German) | 150 | 375 | the entity's fields (falloff not measured separately) |
| bazooka, Panzerschreck | 150 | 200 | `mp_rocketdamage` / `mp_rocketradius` |

The scripts' `Damage` (frags 180, rifle grenades 100, launchers 100) is
not what the blast does: a frag going off in the thrower's hand took
143.6-149.7 of its health (four throws), a direct rocket hit 129.5.

Measured frag stun amounts with the grenade lying on the ground and the
victim standing at a distance d (feet level with it); the row below is
the blast they stand for (the stun × 1.5):

| d | 0 | 50 | 100 | 150 | 200 | 250 | 300 | 350 | 400 |
|---|---|---|---|---|---|---|---|---|---|
| stun | 87.2 | 79.9 | 70.0 | 57.4 | 45.0 | 31.6 | 19.2 | 5.7 | 0 |
| ×1.5 | 131 | 120 | 105 | 86 | 68 | 47 | 29 | 8.5 | 0 |

From 100 units out this is 150 × (1 − r / 375) with r the distance to a
point about 40 units above the victim's feet (its body's middle); the
near values fit that point too (r ≈ 47 at d = 0).

- **The blast needs a clear line** from the explosion to the victim:
  a grenade resting on the ground did no health damage to victims 50 and
  100 units away (stun only), and a grenade going off in the thrower's
  hand hurt only the thrower and victims within about 25 units. The stun
  has no such check.
- **Players don't shield**: a frag going off in the air 150 units from
  a victim did the same 89.5-89.9 with the thrower standing aside or
  between them at 40, 75 or 110 units from the grenade (two runs each);
  only the world blocks the line. The victim's point the distance and
  line go to is about 40-43 units above its feet (the blast at 150
  units and the raised and lowered placements fit 43 within the
  placement noise).
- **Rockets on the world**: a rocket explodes where it meets the world
  (weapons.md); its stun event is not two thirds of the blast there
  (92.5-98.7 blast with a 35.9-41.1 stun to a thrower about 66 units
  away: open question 8).
- The thrower is hurt by its own grenade (measured: 85-150 to a thrower
  next to it).
- Repeats of one placement gave the same ratio (blast / stun = 1.50)
  but not always the same amount (87.6-136.7 at r ≈ 80): the victim was
  placed in the same tick as the blast, so part of the spread is our
  placement; use the ground table above.

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
| running 220, hit in the chest | next tick | speed 120 for 0.5 s, then back to 220 by Source's acceleration |
| running 220, hit in a leg | next tick | speed 120 for 2.0 s |
| ducked 73.3, hit | | speed unchanged |
| frag on the ground, victim 200 away, clear line | goes off | 67.5 health (stun event 45) |
| frag, victim 400 away | goes off | nothing |
| rocket, victim 210 away | goes off | nothing (radius 200) |
| knife, victim 40 ahead facing 72° off the attacker's yaw, aim level | swing | 300 (backstab) |
| knife, the same at 73° | swing | 60 |
| knife, aim 40° down, victim facing 66° / 68° off | swing | backstab / not |
| knife, victim 40 left and 40 ahead facing 105° off the attacker's yaw | swing | 60 |
| knife, victim standing 92 / 94 away, aim level | swing | hit / miss |
| knife | press | hit 0.21 s later; next attack 0.4 s after the press |
| knife or spade, victim 40 ahead facing away | press | 300 lands 0.405 s (27 ticks) later; next attack 0.9 s after the press |
| knife, crouched victim 40 ahead, aim 20° down, facing 71° / 71.5° off | swing | 300 / 60 |
| knife, prone victim 50 ahead, aim 45° down, facing 60° / 66° off | swing | 300 / 60 |
| Thompson stock, victim facing away | alt fire | 60 |
| frag in the air, victim 150 away, a player between | goes off | ≈90 to the victim (not shielded) |

## Open questions

1. When the arm-less hitbox set is used (perhaps while a player is
   prone or deployed, so the arms don't shield the head).
2. Rounding of fractional damage into health (37.5 dealt: health
   dropped by 37 or 38?). The plugin logged health after the hit only
   with a raised pool; re-run reading the health delta.
3. The bullets' maximum range.
4. Answered: the backstab cone (above), for crouched and prone victims
   too. Open: a victim looking up or down (the victim bots looked
   level), what the press-time check of the backstab swing looks at
   (the victim's place then, or a trace), and the swing's exact shape
   (it reaches 45° off the aim at 56 units).
5. Answered (2026-11-23): a 120 u/s cap on the horizontal speed while
   the slowed time runs (above).
6. Penetration: every script has `Penetration` 1; walls were not tested.
7. Explosives: answered for damage, radius, falloff and self-damage
   (above). Fall damage: `mp_falldamage` 0 (the classic Source fall
   damage of CS:S's spec applies unless measured otherwise).
8. The blast's line check: players don't block it (answered); the
   point is about 40-43 above the feet. Open: why a grenade resting on
   the ground hurt nobody 50-100 away (the ground under it?) while one
   in the hand hurt only within about 25 units; the rocket's stun
   amount (not two thirds of its blast); whether the stun's view shake
   scales with its amount (client).
