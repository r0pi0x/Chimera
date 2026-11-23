# Team Fortress 2: batch-2 mechanics (health on hit and kill, mini-crit conditions, bleed, flares, melee speed buffs)

Source basis: the retail client's item schema (`scripts/items/items_game.txt`,
read in place: each item's `item_class`, prefab chain and attributes, and
each attribute's `attribute_class` and `description_format`), and the
running game (Linux client, listen server on `itemtest`, muted, driven over
RCON through VScript, the same way as [item_attributes.md](item_attributes.md):
each unlock created as a live weapon entity and equipped on the local
player, the account's loadout untouched; puppet bots as targets and as a
teammate; the game's own `player_hurt`, `player_healonhit`, `player_healed`
and `player_death` events read back with their game time; positions,
velocities, conditions, max speed and max health sampled every server
tick). No game code was read for this spec. Numbers marked **(measured)**
were read from the running game on 2026-11-23; "fitted" marks a law
chosen to reproduce every measured value, not read anywhere.
Status: draft

## Summary

Five small mechanic families that the batch-2 unlocks share. Healing on
hit and kill gives health back from damage dealt (per rocket, per needle,
per kill, per backstab) and is capped at max health except where noted.
Mini-crit conditions look at the victim's or the attacker's state (blast
jumping, behind and close, burning). Bleed is a damage-over-time condition
like afterburn: 4 damage every half second for a fixed time, one per
victim, refreshed by a new hit. The flare is a fast, slightly falling
projectile with three variants: the Flare Gun crits burning targets, the
Detonator's flare can be exploded in flight, and the Scorch Shot's flare
bounces off a player and explodes on the ground, knocking him up. The
melee speed buffs are a shared "speed boost" condition (x1.4, capped at
+105) given on a hit, plus a few health- and weapon-driven speeds.

## Units and conventions

Hammer units (HU), z up, seconds, health points. Server tick 66.67 Hz
(0.015 s). Gravity 800 HU/s². "Base" is the weapon class's own number;
attribute values combine as [item_attributes.md](item_attributes.md) §1
says. Rounding of damage is weapons.md §2.4 (whole HP, halves up).
"Damage over time" periods of 0.5 s land on whole server ticks: 34 ticks
= 0.51 s (measured for both bleed and afterburn).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| dot_period | 0.51 | s | bleed and afterburn tick spacing (0.5 s rounded up to 34 ticks) |
| bleed_tick | 4 | HP | damage per bleed tick |
| afterburn_tick | 4 | HP | damage per afterburn tick (flames.md) |
| flare_burn | 7.5 | s | afterburn length given by a flare (15 ticks) |
| flare_speed | 2000 | HU/s | flare launch speed (all three flare guns) |
| flare_gravity | 0.3 | × | flare gravity scale (240 HU/s²) |
| flare_damage | 30 | HP | Flare Gun hit, no distance modifier |
| flare_offset | 23.5 / 12 / −3 | HU | flare start from the eye: forward / right / up |
| flare_blast_radius | 110 | HU | Detonator and Scorch Shot explosion radius (fitted, 104–114) |
| flare_blast_edge | 0.5 | × | explosion damage factor at the radius |
| scorch_push | 100 | HU/s | Scorch Shot hit: horizontal push, and vertical push in the air |
| scorch_push_burning | 400 | HU/s | the same against a burning victim |
| ground_lift | 268.3 | HU/s | least upward push a grounded Scorch victim gets (rises 45 HU) |
| scorch_bounce | 139 / 100 | HU/s | flare after hitting a player: forward / up |
| speed_boost_mult | 1.4 | × | the speed-boost condition's multiplier |
| speed_boost_cap | 105 | HU/s | the speed-boost condition's largest gain (of the class base) |
| black_box_base | 90 | HP | damage that earns the Black Box's full heal (the rocket's base) |
| backscatter_range | 512 | HU | Back Scatter's mini-crit reach (origin to origin) |
| backscatter_angle | 75 | degrees | Back Scatter's "behind" half-angle |
| head_health | 15 | HP | Eyelander max health per head (up to 4) |
| head_speed | 0.08 | × class base | Eyelander speed per head (up to 4) |
| head_cap | 4 | heads | heads that count for health and speed |
| kunai_cap | 3 | × max health | Conniver's Kunai's overheal cap |
| dropped_kit_life | 31–33 | s | Candy Cane's dropped kit, untaken, from the kill |

## Behavior

### 1. Health on hit and on kill

#### 1.1 Black Box (`health on radius damage` 20, class `add_health_on_radius_damage`)

One heal per rocket, after its explosion has damaged enemy players: heal =
20 × min(1, D / 90), D the damage that rocket dealt to enemy players in
total, rounded down; capped at max health (no overheal). The heal event
reports what was actually gained. **(measured)** One target: 103 → 20,
71 → 15, 62 → 13, 54 → 12, 49 → 10. Two targets hit by one rocket: 87 +
87 → 20, 67 + 67 → 20 (one target's 67 alone would give 14), 51 + 51 →
20; one heal event each. At 195 of 200 a 103 hit healed 5.
Clip × 0.75 (4 → 3). A rocket that hurts no enemy heals nothing (only
enemies' damage counts; own blast damage was not counted in any run).

#### 1.2 Blutsauger (`heal on hit for rapidfire` 3, `health drain medic` −2)

- Every needle that damages an enemy heals 3 (the on-hit rule of
  item_attributes.md §4.8 with v = 3). **(measured)** five needles of 12
  → five heals of 3.
- The −2 is an item regeneration (class `add_health_regen`, like the Cozy
  Camper's) added to the Medic's own regeneration (classes.md: 3/s rising
  to 6/s from 5 to 10 s after being hurt). The item part scales as
  item_attributes.md §4.8 (× 0.25 while hurt in the last 5 s, then 0.5 →
  1.0 from 5 to 10 s); fractions accumulate.
  **(measured)** gains per second after a hurt: stock Syringe Gun 3, 3,
  3, 3, 3, 3, 4, 5, 5, 6, 6, 6; Blutsauger 2, 3, 2, 3, 2, 3, 2, 4, 3, 4,
  4, 4, 4, 4 (2.5/s, then 4/s): 3 − 0.5 and 6 − 2.

#### 1.3 Powerjack (`heal on kill` 25)

A kill with it heals 25, capped at max health (no overheal). **(measured)**
Pyro at 160 of 175: → 175; at 175: no gain (the heal event still reports
25). Also, while active: move speed × 1.15 (Pyro 345, measured) and damage
taken × 1.2 (`provide on active`).

#### 1.4 Conniver's Kunai (`sanguisuge`, max health −55)

- Max health 125 − 55 = 70 (measured).
- A backstab kill heals the Spy by the victim's health just before the
  stab, overhealing up to 3 × max health (210). **(measured)** Spy at 20,
  victim at 90 → 110; Spy at 123, victim at 300 → 210 (gain 87).
- The overheal drains at the normal rate, which is computed from the
  normal overheal cap (classes.md: (⌊1.5 × 70 / 5⌋ × 5 − 70) / 15 =
  2.33 HP/s), all the way down from 210. **(measured)** from 210: one
  point every 0.43 s.

#### 1.5 Candy Cane (`drop health pack on kill`)

A kill with it drops a small health kit (the map's small kit,
`item_healthkit_small`, the same model) at the victim, which falls to the
floor. Untaken, it was still there 31.3 s after the kill and gone at
33.3 s **(measured)**. Also blast damage taken × 1.25 (provided).

#### 1.6 Eyelander heads (`decapitate type` 1, sword)

- Base: max health 175 − 25 = 150, no random crits, sword rules
  (item_attributes.md §4.11).
- Each kill with it is a decapitation: the head counter +1 and the killer
  heals 15. For the first four heads each also gives +15 max health and
  +8 % of the class base speed (280 × 0.08 = 22.4). **(measured)** max
  health / speed after 0–4 heads: 150/280, 165/302.4, 180/324.8,
  195/347.2, 210/369.6; at 5, 6 and 7 heads still 210/369.6 while the
  counter kept counting.
- The 15 heals past max: at 205 of 210 a kill gave 220. **(measured)** the
  excess then fell back to 210 within 0.36 s (about 28 HP/s, far faster
  than the normal drain; Open questions).

### 2. Mini-crit and crit conditions

Crits and mini-crits themselves are weapons.md §2.5 (× 3 and × 1.35;
mini-crits ignore the distance fall-off).

#### 2.1 Airborne victim (`mod mini-crit airborne`: Direct Hit, Reserve Shooter)

A hit mini-crits when the victim is in the air after an explosion's push
(item_attributes.md §4.3). **(measured)**

- An enemy rocket's blast launched a Soldier bot; the game put him in its
  "blast jumping" condition at once, and a Reserve Shooter hit while he was
  in the air mini-crit (50). The condition cleared when he landed.
- A target merely off the ground (floating, no blast): a normal hit (82).
- Our probe also gave a floating target the blast-jumping condition by
  script: still a normal hit. The game tracks more than that condition
  (Open questions); an implementation sets the state from a real blast
  push.

Reserve Shooter's other attributes: clip × 0.66 (6 → 4), deploy × 0.8.

#### 2.2 Airborne attacker (`mod crit while airborne`: Market Gardener)

A hit is a full crit while the attacker is blast jumping from his own
blast. **(measured)** rocket jump, then swing in the air: 195 (65 × 3);
on a normal jump: 65. The state lasts from the launching blast to the
landing (still set at the last airborne sample, cleared within 0.15 s of
landing). Also interval × 1.2, no random crits.

#### 2.3 Behind and close (`closerange backattack minicrits`: Back Scatter)

A hit mini-crits when both hold **(measured)**:

- distance between the shooter's and the victim's origins ≤ 512
  (511: mini-crit; 514: none);
- the horizontal angle between the victim's facing and the direction from
  the shooter to the victim is under 75° (74.5°: mini-crit; 75.2°: none).
  Directly behind is 0°.

In the runs the shooter always aimed at the victim, so the shooter's own
facing could not be told apart from his position (Open questions). Also
clip × 0.66 (6 → 4), spread × 1.2, no random crits.

#### 2.4 Burning victim

- Flare Gun: a flare hitting a burning player is a full crit **(measured:
  90)**. This is the flare gun class's own rule; the schema attribute
  `crit_vs_burning_FLARES_DISPLAY_ONLY` only writes the tooltip.
- Detonator, Scorch Shot (`minicrit vs burning player`): every hit and
  explosion on a burning player mini-crits (measured: Detonator 22.5 → 30
  direct, its explosion 22; Scorch Shot 26).
- Axtinguisher and Postal Pummeler: §2.5.

#### 2.5 Axtinguisher (`attack_minicrits_and_consumes_burning`)

- Against a player who is not burning: 65 × 0.67 = 43.55 → 44 (measured).
- Against a burning player: always a mini-crit, the burn is removed (no
  afterburn tick follows, measured), and the damage before the mini-crit
  grows with the burn left. Fitted law: base = 60.5 + 6.3 × R, R the
  seconds of afterburn the victim had left (the burn's length minus 0.5 s
  per tick already taken); then × 1.35 and rounded. **(measured, flare
  burns of 7.5 s; all within rounding)**

| Afterburn ticks taken | R (s) | Measured | Law |
|---|---|---|---|
| 2 | 6.5 | 137 | 136.96 |
| 4 | 5.5 | 128 | 128.45 |
| 5 | 5.0 | 124 | 124.20 |
| 7 | 4.0 | 116 | 115.70 |
| 9 | 3.0 | 107 | 107.19 |
| 11 | 2.0 | 99 | 98.69 |
| 12 | 1.5 | 94 | 94.43 |
| 14 | 0.5 | 86 | 85.93 |

  Relative to the weapon's own damage the law is 43.55 × (1.389 + 0.1447
  R), which the implementation should use, so that a damage attribute
  scales it (Open questions).
- A kill on a burning player gives the killer the speed boost (§5.1) for
  3.0 s (measured: Pyro 300 → 405).
- Holster × 1.35 (`single wep holster time increased`), no random crits.

### 3. Bleed

- A condition on the victim, credited to the attacker who caused it.
  Every 0.51 s it deals 4; the first tick lands in the same tick as the
  hit that caused it. It ticks while the time since it began is less
  than its duration: 5 s → 10 ticks (40), 6 s → 12 ticks (48).
  **(measured)**
- A new bleeding hit on a victim already bleeding adds no tick and no
  second bleed: the end moves to the new hit's time plus the duration, and
  the ticks keep the first hit's cadence. **(measured, Boston Basher hits
  at 0 and 1.29 s: 13 ticks, the last at 6.12 s)**
- Ticks are not crits and do not re-trigger on-hit effects.
- The duration is the item's `bleeding duration` (additive, start 0).

| Item | Hit | Bleed | Other |
|---|---|---|---|
| Boston Basher | 35 (measured) | 5 s (measured) | a swing that hits nobody hits the Scout himself for half damage (18, measured) and bleeds him 5 s (measured) |
| Tribalman's Shiv | 65 × 0.5 = 33 (measured) | 6 s (measured) | — |
| Southern Hospitality | 65 | 5 s (attribute; not run) | fire damage taken × 1.2, no random crits |
| Flying Guillotine | 50 (measured) | 5 s (measured) | a thrown, physics-simulated cleaver (launched at about 1850 HU/s, slowing; Open questions) |
| Wrap Assassin | its ornament | not measured | Open questions |

### 4. Flares

#### 4.1 Flight (Flare Gun, Detonator, Scorch Shot)

- Start: from the eye, 23.5 forward, 12 right, 3 down (in the view's
  axes). Its velocity points from there at what the crosshair is on (the
  eye's trace), speed 2000. **(measured)** at pitch −30: start offset
  (23.5, 12, −3), speed 2000.06.
- Gravity 0.3 × 800 = 240 HU/s² (vz falls 3.6 per tick); no drag.
  **(measured)**
- No lifetime limit was seen: a flare kept in the air was alive after 12 s.
- Touching the world: a Flare Gun flare stops at the surface and is
  removed about 0.1 s later (6 ticks); a Detonator flare is removed with
  no explosion; a Scorch Shot flare explodes (§4.4). **(measured)**
- Firing interval about 2 s (a shot 2.1 s after the last fired, one
  within 1.6 s did not; not measured more finely). Secondary reserve ×
  0.5 (`hidden secondary max ammo penalty`).

#### 4.2 Hit and afterburn

- Flare Gun direct hit 30, the same at 120, 300 and 600 HU (no distance
  modifier). It ignites: afterburn 4 every 0.51 s for 7.5 s (15 ticks,
  60), credited to the flare. **(measured)**
- A flare on a burning player: × 3 (Flare Gun) or × 1.35 (Detonator,
  Scorch Shot) (§2.4), and the burn restarts at 7.5 s (re-ignition rule
  with a longer remaining burn: Open questions).
- Every flare hit pushes the victim by the normal rule (weapons.md §5.4,
  6 × damage); the Scorch Shot adds its own push (§4.4).

#### 4.3 Detonator

- Damage × 0.75: direct hit 22.5 → 23, burning 30 (measured). A direct
  hit does not explode.
- Attack2 while a flare of the player's is in flight explodes it where
  it is. Others: 22.5 × f(r), f(r) = 1 − 0.5 × min(r, 110) / 110, r the
  nearer of the distances from the explosion to the victim's centre and to
  his feet, reached when his box's nearest point is within the radius (as
  weapons.md §5.2); no distance modifier; mini-crit vs burning.
  **(measured; the explosion point read exactly by slowing the flare with
  a projectile-speed attribute on our spawned weapon)**

| r (HU) | Measured | 22.5 × f(r), R = 110 |
|---|---|---|
| 55.5 | 17 | 16.82 |
| 70.0 | 15 | 15.34 |
| 83.9 | 14 | 13.92 |
| 97.1 | 13 | 12.56 |
| 112.5 | 11 | 11.25 |
| 126.2 | 11 | 11.25 (box within reach) |
| 140.9 | none | box's nearest point 114 away |

- Self: the Pyro's own explosion hurts him with the flare class's base 30
  (not × 0.75, as item_attributes.md §3.3 found for launchers), ×
  1.5 (`blast dmg to self increased`), falling off with distance.
  **(measured)** 36 at 41 HU (to his centre), 35 at 44–49, 32 at 56–61.
  These do not fit one straight fall-off from his centre (Open questions).
  Until settled, use 45 × (1 − r / 206) (it gives 36, 35, 35 at 41, 44,
  49) and clamp it to the measured 32 for r from 56 to 61; beyond 61 the
  falloff is unmeasured.
- The Detonator's flare sets its owner alight when its explosion reaches
  him (an afterburn tick on the Pyro, measured).
- Push: others by 6 × damage **(measured: 99.8 for 16.6, 83.4 for
  13.9)**; self: 197 HU/s for 35 self damage (measured, ratio 5.6; Open
  questions).

#### 4.4 Scorch Shot (`mod flaregun fires pellets with knockback` 3)

- Damage × 0.65: direct 19.5 → 20; burning 26 (measured).
- On hitting a player the flare is not removed: it bounces, its velocity
  becoming 139 along its old horizontal direction and 100 up, still with
  0.3 gravity, and it explodes when it touches the world (measured: on
  the floor 1.26 s after launch, about 60 HU past the victim).
- The explosion is the Detonator's (§4.3) with base 19.5, except that the
  player the flare hit takes it in full (r = 0, as weapons.md §5.2's "the
  entity the projectile touched"): **(measured)** 26 = 19.5 × 1.35 on the
  then-burning victim. A flare that hits the world first explodes there.
- **Hit push** (on top of the normal one): horizontal 100 along the flare's
  horizontal direction, and 100 up; × 4 (400 and 400) against a burning
  victim. A victim on the ground gets at least 268.3 up (he rises 45 HU).
  **(measured)** floating victim: (100 forward, 100 up); grounded: (99.4,
  268.3); burning, floating: (399.2, 400).
- Own blast push × 0.65 (`self dmg push force decreased`; not measured).

### 5. Speed buffs

#### 5.1 The speed-boost condition

Gives max speed × m, m = min(1.4, (class base speed + 105) / class base
speed), applied to the player's current max speed (after his weapon's
own multipliers). **(measured, condition added by script)** Scout 400 →
505, Medic 320 → 425, Pyro 300 → 405, Heavy 230 → 322, Soldier 240 →
336; Heavy with the Eviction Notice out 264.5 → 370.3. (Demoman 385,
Engineer and Sniper 405, Spy 425 follow; not run.)

#### 5.2 Escape Plan (`mod shovel speed boost` 2)

While it is active, max speed × a factor from the health fraction h =
health / max **(measured, Soldier 200)**:

| h | Factor | Soldier |
|---|---|---|
| ≥ 0.8 | 1.0 | 240 |
| 0.6 ≤ h < 0.8 | 1.1 | 264 |
| 0.4 ≤ h < 0.6 | 1.2 | 288 |
| 0.2 ≤ h < 0.4 | 1.4 | 336 |
| < 0.2 | 1.6 | 384 |

Edges measured: 160 → 240, 159 → 264, 120 → 264, 119 → 288, 80 → 288,
79 → 336, 40 → 336, 39 → 384. The speed followed a damage within 0.075 s
and a heal within about 0.1 s (it is recomputed with the player's speed,
not instantly). While active the Soldier is marked for death (the silent
kind: every hit on him mini-crits); healing from Medics × 0.1.

#### 5.3 Equalizer (`mod shovel damage boost` 1)

No speed effect (measured: 240 at 20 HP). Damage rises as health falls;
fitted law 65 × (1.65 − 1.15 × health / max) **(measured, Soldier 200:
health → damage 160 → 47, 140 → 55, 120 → 62, 100 → 70, 80 → 77, 60 → 85,
40 → 92, 20 → 100, 1 → 107)**. Healing from Medics × 0.1.

#### 5.4 Disciplinary Action (`speed buff ally`)

- Hitting a teammate deals no damage and gives both the speed boost:
  the teammate 2.0 s, the attacker 3.6 s **(measured: 240 → 336)**.
- Hitting an enemy: damage 65 × 0.75 = 49, no boost **(measured)**.
- Swing range × 1.7 (48 → 81.6; a hit at 80 HU measured) and swing box ×
  1.55 (`melee bounds multiplier`).

#### 5.5 Eviction Notice (`speed_boost_on_hit` 3)

- Active: max speed × 1.15 (Heavy 264.5), max-health drain 5/s and a
  20-point cost per deploy, refilled when holstered (as the Bread Bite,
  item_attributes.md §4.10). **(measured: 300 → 280 on deploy, then −1
  every 0.2 s)**
- Each hit on an enemy gives the speed boost for 3.0 s (measured: 370.3).
- Damage × 0.4 (26 measured), interval × 0.6.

#### 5.6 Gloves of Running Urgently

Active: max speed × 1.3 (299), max-health drain 10/s with the 20-point
deploy cost, refilled when holstered; holster × 1.5. **(measured)**

#### 5.7 Atomizer (`air dash count` 1)

While it is the active weapon the Scout has one more air jump (three
jumps); the third is the same as the second (measured: 262.33 up one
sample after each air jump, the ground jump 271.00 the same way). No self
damage (measured: health unchanged). Holstered: two jumps only
(measured). Deploy × 1.5, damage vs players × 0.85.

## Per-tick order

1. Damage event (hit or explosion): base × attributes, conditions
   (§2) decide crit / mini-crit, damage applied (item_attributes.md §3).
2. Same event: the hit's push (weapons.md §5.4) plus a Scorch Shot push;
   bleed or burn applied (a new bleed ticks at once).
3. After the damage: heal on hit (per needle; per rocket after its whole
   explosion), heal on kill, Kunai, Eyelander head, Candy Cane kit, speed
   boost on hit or kill, the Axtinguisher's burn removal.
4. Each tick: bleed and afterburn ticks every 0.51 s; flares move under
   0.3 gravity; Scorch flares that touch the world explode.
5. Player think: max speed recomputed (Escape Plan tier, speed boost,
   heads, active-weapon multipliers); max-health drain.

## Edge cases

- The Black Box heals from enemy damage only; a rocket that reaches
  several enemies heals once.
- Escape Plan and Equalizer read the current max health (Eyelander-style
  max changes and drains move the fraction).
- Bleed and afterburn on the same victim run independently, each on its
  own cadence.
- The speed boost multiplies the current max speed, so it stacks with an
  active weapon's multiplier but its cap is computed from the class base.
- A teammate hit by the Disciplinary Action takes no damage and gets no
  push.

## Quirks

- Bleed and afterburn ticks every 0.51 s, not 0.5 s: keep (it is the
  tick rate's rounding).
- The Flare Gun's crit on burning targets is not an attribute (its
  attribute is a tooltip).
- Kunai overheal drains at the rate of a 70-HP Spy's normal overheal,
  however high it is.
- Eyelander heads keep counting past 4 with no further effect.

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| Soldier 100/200, Black Box | rocket deals 103 to one enemy | explosion | +20 → 120 |
| same | rocket deals 62 | explosion | +13 |
| same | rocket deals 67 to each of two enemies | explosion | one heal, +20 |
| Soldier 195/200 | rocket deals 103 | explosion | 200 |
| Medic hurt, Blutsauger | wait 0–5 s | each second | +2.5/s on average |
| same | 10 s after the hurt | each second | +4 |
| Blutsauger | needle hits for 12 | hit | +3 |
| Pyro 160/175, Powerjack | kill | — | 175 |
| Spy 20/70, Kunai | backstab a 90-HP victim | — | 110 |
| Spy 123/70 | backstab a 300-HP victim | — | 210 |
| Spy 210/70 | wait 4.3 s | — | 200 |
| Scout, Candy Cane | kill | — | a small kit at the victim; gone 33 s later |
| Demoman 50/150, Eyelander, 0 heads | kill | — | 65/165, speed 302.4 |
| 4 heads | kill | — | max 210, speed 369.6, counter 5 |
| bot blasted into the air by an enemy rocket | Reserve Shooter hit in the air | — | mini-crit |
| floating bot, no blast | Reserve Shooter hit | — | normal |
| Soldier rocket jumping, Market Gardener | hit | — | 195 |
| Soldier normal jump | hit | — | 65 |
| Back Scatter, 300 HU, 74° off the back | hit | — | mini-crit |
| same, 76° | hit | — | normal |
| directly behind, 511 / 514 HU | hit | — | mini-crit / normal |
| Flare Gun, 30 HU away or 600 | hit | — | 30, then 15 ticks of 4 at 0.51 s |
| Flare Gun on a burning player | hit | — | 90 |
| Detonator | hit | — | 23; on a burning player 30 |
| Detonator, explosion 70 HU from an enemy's centre | attack2 | — | 15 |
| same, 97 HU | attack2 | — | 13 |
| same, enemy's box 114 HU away | attack2 | — | nothing |
| flare in flight | 12 s | — | still alive |
| Scorch Shot, floating enemy | hit | — | 20; push (100 forward, 100 up) |
| Scorch Shot, enemy on the ground | hit | — | push up 268.3 |
| Scorch Shot, burning floating enemy | hit | — | 26; push (400, 400) |
| Scorch flare after hitting a player | — | — | moves 139 forward, 100 up; explodes on the floor; full damage to the player it hit |
| Axtinguisher, target not burning | hit | — | 44 |
| Axtinguisher, flare burn, 2 ticks taken | hit | — | 137, burn removed |
| same, 14 ticks taken | hit | — | 86 |
| Axtinguisher kill on a burning player | — | — | killer speed boost 3 s |
| Boston Basher | hit | — | 35, bleed 4 now and every 0.51 s, 10 ticks |
| Boston Basher | hit, hit again 1.29 s later | — | 13 ticks, the last 6.12 s after the first |
| Boston Basher | swing at nothing | — | 18 to self, self bleed 5 s |
| Tribalman's Shiv | hit | — | 33, 12 bleed ticks |
| Flying Guillotine | hit | — | 50, 10 bleed ticks |
| speed boost on Scout / Soldier / Heavy | — | — | 505 / 336 / 322 |
| Escape Plan, Soldier 159 / 119 / 79 / 39 HP | — | — | 264 / 288 / 336 / 384 |
| Equalizer, Soldier at 100 / 1 HP | hit | — | 70 / 107 |
| Disciplinary Action | hit a teammate | — | no damage; teammate boosted 2.0 s, self 3.6 s |
| Eviction Notice, Heavy 300 | deploy | — | max 280, speed 264.5, −5 max/s |
| Eviction Notice | hit an enemy | — | 370.3 for 3.0 s |
| GRU | deploy | — | speed 299, −10 max/s |
| Atomizer active / holstered | three jump presses | — | three / two jumps |

## Open questions

- Airborne mini-crits: a scripted blast-jumping condition on a floating
  bot did not mini-crit while a real blast did. Which extra state the game
  keeps (the push itself, a flag set with it) was not found; implement
  from the real push.
- Back Scatter: whether "behind" uses the shooter's position (as
  assumed) or his aim; and whether the 512 is 3D or horizontal (all runs
  were on one floor).
- Axtinguisher: the law is fitted on 7.5-s flare burns; a flame-thrower
  burn (up to 10 s) and a damage attribute on the axe were not measured.
- Eyelander: the excess over max after a decapitation fell at about 28
  HP/s, not the normal (315 − 210) / 15 = 7 HP/s; heads on death and
  their effect on the shield charge were not measured.
- Detonator self damage: 36 at 41 HU, 35 at 44–49 HU, 32 at 56–61 HU does
  not fit one linear fall-off from the Pyro's centre; the distance used
  for the owner (eye, box, other) is unknown. The self push (197 for 35)
  does not fit 6 × damage either.
- Flare re-ignition: whether a second flare sets the burn to 7.5 s or
  adds (capped at 10 s, like flames) was not separated from the victim's
  death in the one run.
- Flare Gun firing interval: about 2 s, not measured finely.
- Flying Guillotine flight (physics object: launch speed, drag, spin) and
  its long-range crit; the Wrap Assassin's ornament (speed, damage,
  bleed): not measured, a separate projectile spec.
- Candy Cane kit: whether its life (31–33 s after the kill) is 30 s from
  landing.
- Black Box: whether each enemy's share is rounded down separately or
  the total once (the two only differ by a point).
- Conniver's Kunai: whether a non-backstab kill heals.
- Escape Plan's mark for death: only the condition was read; the mini-crit
  on him was not shot at.
- Scorch Shot self push (× 0.65) and its explosion's exact radius (taken
  to be the Detonator's).
