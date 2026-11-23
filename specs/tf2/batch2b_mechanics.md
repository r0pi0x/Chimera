# Team Fortress 2: batch-2 mechanics, part 2 (projectiles, knockback, consumables, meters, sniper and revolver variants)

Source basis: the retail client's item schema (`scripts/items/items_game.txt`,
read in place: each item's class, prefab chain and attributes), its sound
scripts (`scripts/game_sounds_*.txt`), particle files (`particles/*.pcf`,
read in place from the install's archive) and one projectile model's
header, and the running game, measured the same way as
[batch2_mechanics.md](batch2_mechanics.md): the Linux client as a listen
server on `itemtest`, muted, driven over RCON through VScript, each unlock
created as a live weapon entity and equipped on the local player only (the
account's loadout untouched); puppet bots as targets and as a teammate;
`player_hurt`, `player_healed`, `player_healonhit` and `player_death` read
back with their game time; positions, velocities, conditions, max speed,
health and the item meters sampled every server tick. Where a run needed
it, random crits were off (`tf_weapon_criticals 0`) and damage spread off
(`tf_damage_disablespread 1`); some runs kept random crits and the table
says so. No game code was read. **(measured)** marks numbers read from
the running game on 2026-11-23; "fitted" marks a law chosen to reproduce
the measured values, not read anywhere.
Status: draft

## Summary

The rest of batch 2. Most new projectiles are of two kinds: straight
"script" projectiles with a gravity scale (the Huntsman's arrow, the
Crusader's Crossbow's bolt, the Bison's ring), and physics objects that
tumble under full gravity and air drag (the Sandman's ball, the Wrap
Assassin's ornament, the Flying Guillotine, the Loose Cannon's ball, jars).
Several weapons scale their effect with the projectile's flight time (the
crossbow's damage and heal, the ball's slow and its "moonshot", the
ornament's crit). Consumables are taunts that heal or give a condition,
then recharge a per-item meter over a fixed time. The meters (hype, boost,
crikey, heads) fill from damage dealt or kills and change speed, charge
rate or give mini-crits.

## Units and conventions

Hammer units (HU), z up, seconds, HP. Server tick 66.67 Hz (0.015 s).
Gravity 800 HU/s². "Flight time" is measured from the first server tick
the projectile exists to the tick it hits (our sampling sees the
projectile one tick after its launch, so laws written in flight time may
be one tick (0.015 s) off). Distances between players are between their
origins unless stated. Damage rounding and the distance modifier are
[weapons.md](weapons.md) §2 (the smoothstep curve, t = d/1024). Item
attributes combine as [item_attributes.md](item_attributes.md) §1. Bleed
and afterburn tick every 0.51 s ([batch2_mechanics.md](batch2_mechanics.md)
§3).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| arrow_speed | 1800 + 800 c | HU/s | Huntsman arrow, c = charge 0..1 |
| arrow_gravity | 0.5 − 0.4 c | × gravity | Huntsman arrow gravity scale |
| arrow_damage | 50 + 70 c | HP | Huntsman body hit; headshot × 3 |
| bow_charge_time | 1.0 | s | to full charge |
| bow_tire_time | 5.0 | s | held this long, the aim wobbles (±6°) |
| bow_interval | 2.0 | s | from release to the next draw |
| bolt_speed | 2400 | HU/s | Crusader's Crossbow |
| bolt_gravity | 0.2 | × gravity | crossbow bolt |
| bolt_ramp_time | 0.6 | s | flight time at which damage and heal reach their maximum (fitted) |
| bolt_damage | 38 → 75 | HP | enemy hit, by flight time |
| bolt_heal | 75 → 150 | HP | teammate hit, by flight time |
| ball_speed | ≈ 1985 | HU/s | Sandman ball at launch, 5.3° above the aim |
| ball_damage | 15 | HP | Sandman ball (× 1.5 on a moonshot) |
| ball_slow_per_s | 7.5 | s of slow per s of flight | Sandman |
| ball_slow_min / max | 2 / 7 | s | |
| ball_min_flight | ≈ 0.08 | s | shorter flights do not slow |
| moonshot_time | 0.8 | s | flight time for a moonshot (7 s, damage × 1.5) |
| ball_recharge | 10 | s | Sandman |
| ornament_recharge | 7.5 | s | Wrap Assassin (`effect bar recharge rate increased` 0.75 × 10, data) |
| cleaver_damage | 50 | HP | Flying Guillotine, any range |
| cleaver_recharge | 5.1 | s | Flying Guillotine (measured; 5 s plus a tick or a few) |
| bleed_projectile | 5 | s | Flying Guillotine and Wrap Assassin bleed |
| bison_speed | 1200 | HU/s | no gravity |
| bison_damage | 20 | HP | base, with the distance modifier; hits each player up to twice |
| cannon_impact | 50 | HP | Loose Cannon ball hitting a player |
| cannon_fuse | 1.0 | s | from the start of the charge |
| donk_window | 0.5 | s | explosion within this of an impact mini-crits (fitted from 0.45 yes, 0.645 no) |
| fan_knockback_range | ≈ 390 | HU | Force-a-Nature: within 375 knocks back, at 400 not |
| shove_speed | 400 | HU/s | Shortstop shove |
| shove_delay | 0.25 | s | press to push |
| shove_cooldown | 1.5 | s | next shove |
| sandvich_heal | 75 / s × 4 | HP | Sandvich, Robo-Sandvich (eating) |
| lunch_meter_time | 30 / 10 | s | Sandvich and Steak / Dalokohs and Banana recharge |
| drink_time | 8 | s | Bonk's and Crit-a-Cola's condition |
| drink_recharge | ≈ 30 | s | measured refill of the drink meter |
| jar_radius | ≈ 230 | HU | break point to the victim's centre (225 in, 236 out) |
| jar_time | 10 | s | Jarate and Mad Milk |
| jar_recharge | 20 | s | |
| milk_heal | 0.6 | × damage | Mad Milk: the attacker's heal, rounded down |
| hype_per_damage | ≈ 0.26 | % per HP | Soda Popper |
| hype_time | ≈ 10.7 | s | Soda Popper's hype, draining 9.4 %/s |
| boost_speed | 1.62 | HU/s per % | Baby Face's Blaster |
| crikey_per_damage | 1 | % per HP (unrounded) | Cleaner's Carbine |
| crikey_time | 8 | s | Cleaner's Carbine mini-crits |
| decap_rate | 0.5 + 0.25 h | × charge rate | Bazaar Bargain, h heads up to 6 |
| ambassador_head_range | 1200 (1200 yes, 1250 no) | HU | headshots crit up to this |
| ambassador_recover | 1.0 | s | since the last shot, for a headshot |

## Behavior

### 1. Huntsman (56; Fortified Compound 1092, Festive Huntsman 1005 the same)

- **Charge.** Holding attack draws the bow; the charge c rises linearly
  from 0 to 1 over 1.0 s from the moment the draw starts and stays at 1.
  Releasing fires. **(measured)** charge times 0.06, 0.15, 0.225, 0.555,
  0.645, 0.735, 1.0+ gave speeds 1848, 1920, 1980, 2244, 2316, 2388, 2600.
- **Flight.** Speed 1800 + 800 c; gravity scale 0.5 − 0.4 c (vz falls
  5.7, 4.9, 2.5 and 1.2 per tick at c = 0.06, 0.225, 0.735, 1:
  0.475, 0.41, 0.21, 0.1 × 800). The arrow starts 23.5 forward, 8 right
  and 4 up from the eye (first sample) and flies toward what the
  crosshair is on (the eye trace); no drag.
- **Damage.** 50 + 70 c on a body hit, no distance modifier (120 at full
  charge from 300 and 1000 HU); a headshot is a crit, × 3 (282 at c =
  0.63, 273 at c = 0.585; 360 at full). **(measured)** 54 (c 0.06), 60
  (0.15), 89 (0.555), 95 (0.645), 120 (1.0).
- **Tiring.** Held 5 s or more, the arrow leaves up to about 6° off the
  aim in yaw and 5° in pitch, a new random offset each shot (measured
  over 10 shots between 5.05 and 8 s held: yaw −5.6..+5.5°, pitch
  −4.6..+4.3°; at 4.85 s still straight). Speed (2600) and damage (120)
  stay those of a full charge.
- **Rhythm.** The next draw can start 2.0 s after a release (measured:
  next attack time = release + 2.0). An arrow that hits the world stays
  stuck there.

### 2. Crusader's Crossbow (305)

- Bolt 2400 HU/s, gravity 0.2 (2.4 per tick), starting where the arrow
  does. **(measured)**
- Hitting an enemy: damage 38 × (1 + f); a teammate: heal 75 × (1 + f),
  capped at his max health (no overheal); f = min(1, t / 0.6), t the
  flight time. No distance modifier; random crits apply (× 3: a
  225 seen). **(measured)**

| Flight (s) | Damage | Heal |
|---|---|---|
| ≈ 0.015 | 38 | 76 |
| 0.075–0.09 | 43 | 86 |
| 0.225–0.24 | 52 | 104 |
| 0.345–0.405 | 63 | 125 |
| 0.435 | 65 | 131 |
| 0.51 | 69 | — |
| 0.63, 0.84 | 75 | 150 |

  The flight time counts, not the distance from the shooter: a bolt
  moved back along its path by script (same speed, longer flight) hit as
  its time said (0.435 s → 65 and 131).
- No headshots (`sniper no headshots`); reserve × 0.25 (data).
- Next shot 1.74 s after the last (the reload starts on its own 0.24 s
  after the shot; measured).

### 3. Sandman (44): the ball

- Attack2 launches the ball when the meter is full; the meter refills in
  10 s from the launch (measured: ready time = launch + 10.0).
- **Flight** (a physics object): initial speed about 1985, aimed 5.3°
  above the crosshair's pitch (pitch 0 → 5.4° up; −30 → 35° up; +30 →
  24.8° down), full gravity, air drag of about 2.6 × 10⁻⁴ × speed² HU/s²
  (fitted: speed 1975 → 1507 in 0.6 s); first seen 32 forward and 15
  below the eye. It bounces off the world and stays as a pickup.
- **Hit.** 15 damage (× 3 on a random crit: 45); then a slow on the
  victim **(measured)**:
  - flight under about 0.08 s (0.06 measured): no slow;
  - otherwise a movement slow ("stunned" condition, movement amount 127
    of 255, i.e. 50 %), duration = clamp(7.5 × t, 2, 7) s:

| Flight t (s) | Slow (s) |
|---|---|
| 0.09–0.195 | 2.0 |
| 0.345 | 2.588 |
| 0.36 | 2.7 |
| 0.525 | 3.938 |
| 0.645 | 4.838 |
| 0.69 | 5.175 |
| 0.765 (0.78) | 5.85 |
| ≥ 0.81 | 7.0 (moonshot) |

  - **moonshot** (t ≥ 0.8 s; 0.78 no, 0.81 yes): 7 s, damage × 1.5
    (23; 68 as a crit), and the stun carries another flag (a special
    sound; the slow amount is the same 50 %).
- Also max health −15 (110 for the Scout, data and measured).

### 4. Wrap Assassin (648): the ornament

- Same throw and meter as the ball (recharge 0.75 × 10 = 7.5 s, from the
  attribute); a physics object with more drag (about 4.3 × 10⁻⁴ ×
  speed², fitted: 1936 → 1302 in 0.6 s).
- On hitting a player it breaks **(measured, crits on in these runs)**:
  - a direct hit of 15 (no distance modifier), a full crit (45) when the
    flight reached the moonshot time (0.945 and 1.08 s: 45; 0.57 s: 15);
  - a glass burst around the break that hurts players near it,
    including the thrower (4 to the Scout from 100 HU): 6–9 to the
    victim, more with a longer flight (6 at ≤ 0.09 s, 7 at 0.27, 8 at
    0.42, 9 at ≥ 0.57 s);
  - bleed 5 s (10 ticks of 4, the first with the hit).
- A throw that only breaks near a player (1250 HU) gave him 4–9 and no
  bleed.
- Melee damage × 0.35 (data).

### 5. Flying Guillotine (812)

- Attack throws it (the player switches back to his previous weapon
  after throwing); meter refill 5.1 s from the throw (measured).
- Flight: a spinning physics object, about 1840 HU/s with about 7.6°
  above the aim (vz 250 at pitch 0), full gravity; its path wobbles
  (lift from the spin), so only the first ticks are clean.
- Hit: 50 at every flight time measured (0.015 to 1.1 s, no long-range
  crit), plus bleed 5 s. **(measured)** A target given a stun by script
  took 50 (no crit); a crit against a ball-stunned target was not run.

### 6. Righteous Bison (442)

- A ring at 1200 HU/s, no gravity, starting 23.5 forward, 8 right from
  the eye (measured).
- Damage 20 with the distance modifier from the shooter to the victim
  when it hits (weapons.md §2.1; measured 24 at 80, 22 at 250, 21 at
  350–450, 19 at 550, 16 at 650, 12 at 800, 11 at ≥ 900), × 3 on a crit
  (60, no distance modifier). The near half fits k ≈ 0.4–0.5.
- It passes through players and hits each one it overlaps twice, two
  ticks apart (0.03 s; every run), then goes on to the next (two enemies
  in a line: 2 + 2 hits). No ammo: energy 20, 5 a shot (4 shots), fire
  interval 0.8 s; refill after the clip is empty: the first 5 after
  about 0.93 s, then 5 every 0.345 s (measured).

### 7. Loose Cannon (996)

- The ball is a grenade (item_attributes.md: 1216 forward + 200 up) ×
  1.2 forward speed, the pipe's physics.
- **Fuse**: 1.0 s from the start of the charge; holding attack keeps it
  in the barrel while the fuse runs (a tap: alive 0.975 s; held 0.5 s:
  0.585 s; held 0.7 s: 0.39 s).
- **Impact** on a player: 50 damage, and a 0.5 s full movement stop on
  him (movement amount 255; measured). It does not explode on impact.
  No push was seen on a grounded puppet bot (Open questions).
- **Explosion** at the fuse's end: base 60 at the centre (blast rules as
  the grenade); 30 and 34 seen at ~0.8 s after the impact.
- **Double donk**: an explosion within 0.5 s of that ball's impact on a
  player mini-crits on him: 81 = 60 × 1.35 at 0.24 and 0.45 s after the
  impact; 0.645 s: a normal 34. **(measured)**

### 8. Ullapool Caber (307)

- First hit: the melee hit (55 measured) and an explosion at the hit:
  69 to the struck enemy (about 60 HU away), 61 to one about 95 HU away,
  52 to the Demoman himself, who is launched up at 472 HU/s. Later hits:
  the melee hit only (55), until the Demoman resupplies or dies.
  **(measured)**
- Swing interval 0.96 s (× 1.2), deploy × 2 (data).

### 9. Force-a-Nature (45)

- Knockback on hit: an enemy within about 390 HU (375 yes, 400 no; it is
  distance, not damage: half damage at 200 HU still knocked back) is
  thrown away from the shooter and lifted off the ground **(measured,
  grounded Soldier bot, crits and spread off)**:

| d | Damage | Velocity after (horizontal, up) |
|---|---|---|
| 50 | 113 | 858, 401 |
| 150 | 108 | 840, 311 |
| 200 | 104 | 816, 299 |
| 300 | 93 | 754, 273 |
| 325 | 90 | 736, 271 |
| 350 | 80 | 657, 281 |
| 375 | 77 | 639, 279 |
| 200 (damage × 0.5) | 58 | 453, 286 |

  Fitted: horizontal ≈ 7.9 × damage (7.6–8.3); vertical at least the
  ground lift (≈ 270), more at point blank. Beyond the range only the
  normal push (weapons.md §5.4; the grounded speed limit gave 240/218).
- **Self push**, only while airborne (none standing): the shot pushes the
  Scout opposite his aim. Fitted: Δv = −aim × (300 + 183 × sin p), p the
  aim's pitch below the horizon **(measured, from a fall of about −145)**:

| Pitch (down) | Velocity after |
|---|---|
| 89° | (−8 back, 338 up) |
| 60° | (218 back, 270 up) |
| 45° | (297 back, 178 up) |
| 30° | (333 back, 73 up) |
| 0° | (300 back, −119) |
| −45° | (128 back, −247) |

- Stats from data: interval × 0.5, pellets × 1.2, damage × 0.9, clip 2
  (reload all at once).

### 10. Shortstop (220): the shove

- Attack2 shoves after 0.25 s (measured 0.255) with a 1.5 s cooldown;
  1 damage to the player in front (reach: 60 yes, 90 no).
- The victim's new velocity has length 400 **(measured)**: grounded,
  (400 horizontal, 262 up: the ground lift, he leaves the ground);
  airborne at the same height, 25° up (360, 168); airborne 130 above
  the shooter, 48° up (263, 295) along the line of aim. Fitted: 400
  along the aim, at least 25° up; a grounded victim keeps 400 horizontal
  and is lifted (Open questions).
- While active: push and airblast taken × 1.2; reload × 1.5 (data).

### 11. Lunchbox items (Heavy; `tf_weapon_lunchbox`)

- Attack eats: a 4.25 s taunt (cannot move or switch); the heal arrives
  in four steps at 1, 2, 3 and 4 s; then the primary is drawn.
  **(measured, from 50 HP)**:

| Item | Each step | Total | Other |
|---|---|---|---|
| Sandvich (42), Robo-Sandvich (863), Festive (1002) | 75 | 300 | — |
| Dalokohs Bar (159), Fishcake (433) | 25 | 100 | max health +50 from the first step (still on after 38 s; removed when the item was replaced) |
| Second Banana (1190) | 50 | 200 | heal × 0.67 |
| Buffalo Steak Sandvich (311) | 0 | 0 | from 1 s: mini-crit buff (`ENERGY_BUFF`), melee only, speed × 1.3 (230 → 299), for 19 s (15.8 s after the taunt); damage taken × 1.2 (data) |

- Attack2 with a full meter throws the item as a medium health kit
  (the map's `item_healthkit_medium`) from the eye at about 490 HU/s
  forward, and draws the primary; a teammate touching it heals half his
  max health (75 of 150, measured). The Heavy himself can pick it up.
- The meter empties on eating or throwing and refills linearly: 30 s
  (Sandvich, Steak; 3.33 %/s measured), 10 s (Dalokohs, Banana; 10 %/s
  measured) (`item_meter_charge_rate`, data).
- Eating at full health: not tested.

### 12. Drinks (Scout; `tf_weapon_lunchbox_drink`)

- Attack drinks: a 1.2 s taunt; the condition begins 0.9 s in and lasts
  8.0 s. **(measured)**
  - Bonk! Atomic Punch (46): the "phase" condition (damage immunity, no
    attacks; immunity not measured). No slow afterwards in any
    measured number (max speed and movement stun unchanged).
  - Crit-a-Cola (163): the mini-crit buff for 8 s; each attack during it
    marks the Scout for death (silent) for 5 s (`mod_mark_attacker_for_
    death` 5; measured 2.61 → 7.62).
- The drink meter drains during the effect (100 → 0 over the 8 s) and
  then refills over about 30 s (3.31 %/s measured).

### 13. Jars (Jarate 58, Mad Milk 222 and their re-skins)

- Thrown (attack) about 1000 forward and 200 up of the aim, full
  gravity, little drag; the player switches back afterwards. It breaks
  on touching a player or the world.
- Every enemy whose centre is within about 230 HU of the break gets the
  condition for 10.0 s **(measured: 225 in, 236 out; both jars the
  same)**. Line of sight was clear in all runs.
  - Jarate ("urine"): hits on him mini-crit (68 = 50 × 1.35 from a body
    shot).
  - Mad Milk: an attacker's damage on him heals the attacker 60 %,
    rounded down (121 → 72).
- Recharge 20 s from the throw (measured 20.1); `extinguish reduces
  cooldown` 0.8 (data: a teammate extinguished cuts it).

### 14. Meters

- **Soda Popper (448)**: hype ≈ 0.26 % per point of damage dealt
  (60 → 15.4, +41 → 26.3, +37 → 36.3, +29 → 44.1; about 375 damage
  fills it; fitted). At 100 % attack2 starts the hype condition
  (`SODAPOPPER_HYPE`), which lasts while the meter drains at about 9.4
  %/s (about 10.7 s). Extra air jumps during it (5 by the item's
  description) were not measured.
- **Baby Face's Blaster (772)**: boost from damage dealt, about 1 % per
  point (86 → 86.4), at most 99. Max speed = class base × 0.9 + 1.62 ×
  boost (Scout: 360, 384.3 at 15, 408.6 at 30, 457.2 at 60, 520.4 at 99;
  measured). Taking damage removes 4 per point (10 damage: 60 → 20);
  a jump removes 75 % (`hype resets on jump` 75, data; not measured).
- **Cleaner's Carbine (751)**: crikey = the damage dealt before rounding
  (10.2 per 10-damage hit at 300 HU), full at 100. Attack2 when full:
  mini-crit buff for 8.0 s (measured).
- **Bazaar Bargain (402)**: charge rate × (0.5 + 0.25 × heads), heads
  counted up to 6 **(measured, heads set by script)**: 0 → 25/s (6 s to
  full), 1 → 37.5 (4.0 s), 2 → 50 (3.0 s), 3 → 2.37 s, 4 → 1.98 s, 5 →
  1.71 s, 6 and 8 → 1.47 s. Heads come from headshot kills (not run).
- **Kill-based refills**: Diamondback, §16. Others not measured.

### 15. Sniper rifle variants

Stock charge for comparison: 50/s while scoped, 150 at 3.0 s.

- **Machina (526)**: a fully charged shot does 150 × 1.15 = 173 and
  passes through players (the enemy behind took the same 173); a part
  charge (66) stops at the first. Fires only scoped (data).
- **Sydney Sleeper (230)**: charge × 1.25 (150 at 2.37 s); no headshot
  crits, but a headshot mini-crits (150 × 1.35 = 203 at full charge).
  Every hit jarates the victim for max(2, 5 × charge / 150) s (fitted:
  2 s at 8–38, 3 s at ~90, 5 s at 150).
- **Classic (1098)**: charges while attack is held, unscoped too, at
  50/s, fires on release. Body: max(50, charge) × 0.9 (45 tap, 46 at
  51, 135 full); head: a crit only at full charge (450), else
  max(50, charge) (51 at 51). **(measured)**

### 16. Revolver variants (Spy)

- **Ambassador (61)**: base 34 (40 × 0.85). A headshot is a crit if the
  last shot was more than 1.0 s ago (0.99 s: no, 1.14 s: yes) and the
  victim within about 1200 HU (1200 yes, 1250 no); its damage takes the
  far half of the distance modifier: 102 at 200–500, 75 at 700, 59 at
  850, 54 at ≥ 1000 (= 102 × 0.528). **(measured)** Interval × 1.2.
- **Diamondback (525)**: a backstab kill adds one stored crit (measured
  0 → 1); each shot with stored crits is a crit (102) and uses one.
  Base 34. (Sapper kills, data; the cap: not measured.)
- **Enforcer (460)**: base 40 (52 at 300); a shot fired while disguised
  × 1.2 (62; firing ends the disguise, so the next is 52). Interval ×
  1.2; `dmg pierces resists absorbs` (not measured).

### 17. Left over from part 1

- **Back Scatter**: the 512 is three-dimensional: horizontal 500 with
  the shooter 191 above (535) did not mini-crit; horizontal 470, 191
  above (507) did. Position vs aim cannot be told apart in a hit (a hit
  needs aim at the victim); use positions.
- **Eyelander**: a decapitation's 15 past max health drains at 1 HP per
  2 ticks (≈ 30/s: 220 → 210 in 0.35 s), only that heal: the same excess
  set by script drained at the normal overheal rate (7/s at max 210,
  5/s at 150). With room under max the 15 heals normally (160/165 →
  175/180).
- **Flare re-ignition**: a second flare on a burning player adds its 7.5
  s to the burn left, capped at 10 s, and the ticks keep the first
  cadence (first burn 3.18 s old: 20 more ticks, the last 10.08 s after
  the second hit).

### 18. Flare and crit looks (batch 2's polish)

- **Mini-crit hit sound**: `TFPlayer.CritHitMini` (static channel,
  85 dB, volume 1; one of `player/crit_hit_mini.wav`,
  `crit_hit_mini2…5.wav`), the mini-crit twin of `TFPlayer.CritHit`
  (weapons_stock.md §8.3).
- **Floating texts** (`particles/crit.pcf`): `crit_text` (material
  `effects/crit`) and `minicrit_text` (`effects/minicrit`). Both: one
  sprite emitted at once, 24 above the effect's point and locked to it,
  life 2 s, rising (gravity +20 up, no drag); alpha 1 for the first half
  of its life, then fading to 0 at its end; radius 12 (crit) or 8
  (mini-crit), starting at 2 × and easing to 1 × over the first 20 % of
  its life (bias 0.9); colour at birth green (0, 255, 30) for the crit,
  a random mix of (255, 234, 0) and (234, 255, 0) yellow for the
  mini-crit, easing to red (255, 0, 0) over its life. Same file:
  `doubledonk_text`, `hit_text`, `miss_text`, `heal_text`, `stomp_text`.
  Where the game puts the point (the victim's head or centre) and who
  sees it were not measured here.
- **Flare trail** (`particles/rockettrail.pcf`): `flaregun_trail_red` /
  `_blue`: smoke sprites (`effects/rocketrailsmoke`) emitted 400/s, life
  0.6–1.0 s, radius 4–6 (×0.5 at birth growing to ×1), alpha 32–96
  fading in to 5 % of life and out from 10 %, rising (+96 gravity, drag
  0.2), with velocity noise (y −220..120, z −120..120); colour red
  (255, 89, 89)–(251, 0, 0) or blue (89, 171, 255)–(129, 161, 188),
  fading to grey (33, 31, 35) by 30 % of life; child
  `flaregun_sparkles_<team>`: a glow (`effects/sc_softglow`) 64/s, life
  0.1 s, radius up to 13, locked to the point, red (255, 191, 149)–(255,
  30, 0) fading to (205, 0, 0) (the blue one by the same pattern).
- **Crit glow**: `flaregun_crit_red` / `_blue` (same file): bright glows
  (`effects/sc_brightglow_y_nomodel`) 90/s, life 0.2–0.6 s, radius
  0.3–0.5 × 5 at birth, alpha 255, random spin; red (255, 180, 0)–(255,
  0, 0) fading to (255, 0, 0); blue (0, 228, 255)–(0, 84, 255) fading to
  (0, 96, 255).
- **Where they attach**: the flare's model
  (`models/weapons/w_models/w_flaregun_shell.mdl`) has no attachment
  points and one bone, so both follow the projectile's origin.
- **A flare ending on the world**: the particle file defines
  `flaregun_destroyed` (`particles/flamethrower.pcf`): two flame sprites
  (`particle/flamethrowerfire/flamethrowerfire102`) at once, life
  0.7–1.2 s, radius 2–3 growing 3 × → 5 ×, rising (+55, drag 0.115),
  orange (255, 136, 9)–(255, 156, 0) fading to (255, 141, 101), with
  child `flaregun_destroyed_bits`: six black debris chunks
  (`effects/debris/debris_chunk`) thrown up at 200–255 (+111 up),
  gravity −700, colliding with the world. That this system is the one
  shown when a Flare Gun flare stops (measured: removed 6 ticks after
  it stops, part 1 §4.1) was not observed on screen.
- **Burning from another Pyro counts**: a target set alight by a
  teammate Pyro's flame thrower (the burning condition from his first
  flame hit, 0.3 s of flame) took a full crit (90) from our Flare Gun
  0.7 s later; his afterburn then counted as ours and kept the flame
  thrower's tick cadence. **(measured)**

## Per-tick order

1. Weapon think: bow charge, Classic/rifle charge (rate × Bazaar heads,
   × Sydney), fuse of a held Loose Cannon ball, meter drains (hype,
   drink, buffs).
2. Fire: projectile created (speed and gravity from the charge; ball,
   ornament, cleaver and jars as physics objects), or the shove queued
   0.25 s ahead.
3. Projectiles move (gravity scale, drag); touching: arrows and bolts
   hit; ball/ornament/cleaver hit and record the flight time; jars
   break and apply their condition to everyone in reach; Bison rings hit
   each overlapped player (twice), keep going.
4. Damage event: flight-time laws (crossbow, ball, ornament), distance
   modifier (Bison, revolvers), crit rules (Ambassador head, Diamondback
   store, jarate mini-crit, donk), then the push (FaN knockback, shove,
   blast) and conditions (slow, bleed, jarate).
5. After damage: heal from Mad Milk, meters fill (hype, boost, crikey),
   stored crits change, Crit-a-Cola mark on attacking.
6. Taunt-driven consumables: heal steps at 1, 2, 3, 4 s; condition
   starts at 0.9 s (drinks) or 1 s (Steak).

## Edge cases

- A Sandman ball that hits after less than ~0.08 s does damage only.
- A moonshot ball is 7 s whatever the flight beyond 0.8 s.
- Crossbow heal never overheals; a full-health teammate gains nothing.
- The Machina's pierce needs full charge; the Bison always pierces.
- The Ambassador's headshot needs both the 1 s recovery and the range.
- An Enforcer shot that breaks the disguise still gets the bonus.
- Mad Milk heals from any damage on the milked victim, including
  mini-crits from Jarate on him.
- Jars ignore the thrower's team: only enemies get the condition (a
  teammate in reach: not run).

## Quirks

- Bow tiring changes only the direction, not damage or speed.
- The Bison can hit one target twice in a pass (double damage at point
  blank).
- The decapitation's excess drains much faster than ordinary overheal.
- The Baby Face's boost stops at 99, not 100.

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| Huntsman | release at c = 0, 0.5, 1 | — | speed 1800 / 2200 / 2600; gravity 0.5 / 0.3 / 0.1 |
| Huntsman | body hit at c = 0.555 | — | 89 |
| Huntsman | headshot at c = 1 | — | 360 |
| Huntsman | release | +2.0 s | can draw again |
| Huntsman | held 6 s | release | 2600, ≤ 6° off |
| Crossbow | hit enemy after 0.24 s | — | 52 |
| Crossbow | hit teammate at 10/150 after 0.435 s | — | +131 |
| Crossbow | hit teammate after ≥ 0.6 s | — | +150 (capped by max) |
| Sandman ball | hit after 0.06 s | — | 15, no slow |
| Sandman ball | hit after 0.525 s | — | 15, slow 3.94 s at 50 % |
| Sandman ball | hit after 0.85 s | — | 23, slow 7 s |
| Sandman | throw | +10 s | ball ready |
| Wrap Assassin | hit at 300 HU | — | 15 + burst 6 + bleed 10 ticks |
| Guillotine | hit at any range | — | 50 + bleed 10 ticks |
| Bison | ring through two enemies at 270 / 470 HU | — | 22, 22, 20, 20 |
| Bison | 5 shots held | — | 4 fire, 0.8 s apart; refill after |
| Loose Cannon | ball hits enemy, fuse ends 0.45 s later | — | 50, then 81 (mini-crit) |
| Loose Cannon | ball hits enemy, fuse ends 0.65 s later | — | 50, then normal blast |
| Loose Cannon | impact | — | victim cannot move 0.5 s |
| Caber | first hit | — | melee 55 + blast (≈ 69 at 60 HU), self 52, self up 472 |
| Caber | second hit | — | 55 only |
| FaN | hit grounded enemy at 300 for 93 | — | velocity (754 away, 273 up) |
| FaN | hit at 400 | — | normal push only |
| FaN | airborne, aim 89° down | shoot | +483 up |
| Shortstop | shove grounded enemy at 60 | +0.25 s | 1 damage, (400, 262) |
| Sandvich | eat at 50/300 | 1, 2, 3, 4 s | 125, 200, 275, 300 |
| Sandvich | attack2 | — | medium kit thrown; teammate 30/150 → 105 |
| Sandvich | eat | +30 s | meter full |
| Dalokohs | eat at 50/300 | 4 s | 150/350 |
| Steak | eat | 1 s | speed 299, mini-crit buff, melee only, 19 s |
| Bonk | drink | 0.9 → 8.9 s | phase |
| Crit-a-Cola | drink, fire | — | marked for death 5 s from the shot |
| Jar | breaks 225 from an enemy's centre | — | condition 10 s |
| Jar | 236 | — | none |
| Jarate | rifle body hit on a jarated enemy | — | 68 (mini-crit) |
| Mad Milk | 121 damage on a milked enemy | — | attacker +72 |
| Soda Popper | 60 damage | — | hype 15.4 |
| Baby Face | boost 60 | — | Scout max speed 457.2 |
| Baby Face | boost 60, take 10 | — | boost 20 |
| Carbine | 10.2 damage | — | crikey +10.2 |
| Bazaar | 3 heads | scope | full at 2.4 s |
| Machina | full charge through two | — | 173 and 173 |
| Sydney | full charge body | — | 150, jarate 5 s |
| Classic | released at full, head | — | 450 |
| Classic | released at 51, body | — | 46 |
| Ambassador | head at 850, ≥ 1 s since last | — | 59 crit |
| Ambassador | head 0.85 s after last | — | normal body-type damage |
| Ambassador | head at 1250 | — | no crit |
| Diamondback | backstab kill, then shoot | — | crit 102, store back to 0 |
| Enforcer | shot while disguised at 300 | — | 62 |
| Back Scatter | from behind, 3D 535 (horizontal 500) | — | normal |
| Eyelander 4 heads, 205/210 | decapitation | — | 220, back to 210 in 0.35 s |
| Flare on a 3.2-s-old flare burn | hit | — | 20 more ticks |
| Burned by a Pyro's flames | Flare Gun hit | — | 90 (crit) |

## Open questions

- Crossbow: whether the 0.6 s is flight time or path length (the same
  at its constant speed; the attribute for projectile speed did not
  change the bolt's speed, so it could not be split).
- Sandman: the moonshot threshold lies between 0.78 and 0.81 s; whether
  it is exactly 0.8 and whether the 7.5 s/s law reaches 7 before it
  (at 0.8 s it gives 6). The slow's effect on the victim's movement
  (50 % read from the condition's amount) was not measured on a moving
  player.
- Ball and ornament drag laws are fitted to one flight each; physics
  objects also spin and bounce.
- Wrap Assassin's burst: its radius and law (6–9 by flight) and why one
  long run showed two burst hits.
- Flying Guillotine: its flight law; crits against a ball-stunned target.
- Loose Cannon: the impact's push on enemies (none seen on a grounded
  puppet bot, which the 0.5 s full stop may hide); the donk window's
  exact end.
- Force-a-Nature: the knockback law (≈ 7.9 × damage horizontal) and its
  range (between 375 and 400); the self-push law is fitted.
- Shortstop: the push direction law (fitted ≥ 25° up).
- Lunchbox: Dalokohs's max-health bonus did not end within 38 s; its
  real end, and eating at full health.
- Bonk: the immunity, and any after-effect, were not measured.
- Soda Popper: air jumps during hype; hype per damage is not quite
  constant (0.257–0.272).
- Baby Face's: the 99 cap and the jump loss.
- Jar radius: the measure (centre vs box) and line-of-sight rules.
- Diamondback's crit cap; Bazaar heads from kills.
- Floating texts: the game's placement point and audience.
- `flaregun_destroyed` as the world-impact look: from the data's name,
  not seen on screen.
