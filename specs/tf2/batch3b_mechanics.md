# Team Fortress 2: batch-3 mechanics, part 2 (shield entity, fire weapons, movement kit, Cow Mangler, Heatmaker, Razorback)

Source basis: the retail client's item schema (`scripts/items/items_game.txt`,
read in place: each item's class, prefab chain, attributes and static
attributes), the English resource file's item and attribute texts, the
console variables the game lists, and the running game, measured the
same way as [batch3_mechanics.md](batch3_mechanics.md): the Linux client
as a listen server, muted, driven over RCON through VScript; unlocks
created as live entities and equipped on the local player only (the
account's loadout untouched); puppet bots as targets, teammates and,
with `bot_forceattack` (and `bot_forceattack2` for alt-fire), as
attackers; `player_hurt`, `player_healed` and `player_death` read back
with their game time; position, velocity, conditions, buttons, max speed,
health and the weapon and item meters sampled every server tick.
`itemtest` for everything on flat ground; `ctf_2fort` for the
falling and flying tests (`itemtest`'s ceiling is about 290 HU above
its floor). Random crits were off (`tf_weapon_criticals 0`) for every
crit result below, and fall damage spread off
(`tf_fall_damage_disablespread 1`) where a section says so.
No game code was read. **(measured)** marks numbers read from the running
game on 2026-11-23/24; "fitted" marks a law chosen to reproduce the
measured values, not read anywhere; "(data)" marks values read from the
item schema or the resource file and not measured.
Status: draft

**The shield route.** Part 1 could not equip a shield (VScript cannot
attach a wearable to a human player). This pass used a route that does
equip one properly: a TF2 bot (`tf_bot_add`, not tied to any account)
given the shield by the game's own server-side item grant for bots
(VScript's bot "generate and wear item" call). The grant attaches the
wearable, sets the "shield equipped" flag and the shield's attributes,
exactly as a loadout would. The bot was told to ignore enemies; its own
logic presses attack2 every 0.1–0.2 s, so a charge was started by
filling its meter to 100 (the next press then charges), and its melee
swing by the bot "press fire" call, timed by a script hook on the
impact's damage event or on the meter falling below a set value. The
same route gave the Razorback to a bot Sniper (§10). The reference
account's loadout was never changed.

## Summary

The Demoman's shield, now measured on a real shield: attack2 starts a
charge only from a full meter, the charge drives the Demoman forward by
itself, removes his debuffs (burning, bleeding, Jarate, Mad Milk, gas;
not marked-for-death), a melee swing ends it at once, and the swing's
crit depends on how much meter is left (mini-crit at 75 and below, full
crit at 40 and below; the Tide Turner never more than a mini-crit).
Bumping an enemy does 8 + 0.7 × (meter used) damage, capped at 50
(× 1.7 for the Splendid Screen, + 10 % per Eyelander head up to 5).
The Pyro's fire family reuses the stock airblast: the Phlogistinator
trades it for a 300-damage rage meter and a crit taunt, the Backburner
crits from behind and pays 50 ammo per airblast, the Degreaser deploys
in 0.2 s and holsters in 0.35 s, the Dragon's Fury fires a 3000 HU/s
fireball every 0.8 s (0.53 s after a hit) that triples on burning
targets, the Manmelter turns teammates it extinguishes into stored
crits, and the Gas Passer coats enemies in a 5 s cloud that ignites
them on their next damage. The movement kit: the B.A.S.E. Jumper holds
the fall to about 106 HU/s and the air speed to 75, once per air time;
the Mantreads stomp for 10 + 3 × fall damage and cut knockback and
airblast to a quarter; the Thermal Thruster hops, then 0.65 s later
throws the Pyro 810 HU/s along his aim, stomps like the Mantreads and
pushes enemies around the landing. The Cow Mangler fires four 1100 HU/s
shots on a 20-point energy clip and a 2 s charged shot that mini-crits
and ignites; the Hitman's Heatmaker's focus (35 per kill, 10 s) keeps
the scope up; the Razorback eats one backstab and stalls the Spy 2 s.

## Units and conventions

Hammer units (HU), z up, seconds, HP. Server tick 66.67 Hz (0.015 s).
Meters run 0–100. Distances between players are origin to origin on
flat ground. Velocities are read at the end of a tick (after that
tick's gravity half-step). Attribute values combine as
[item_attributes.md](item_attributes.md) §1. The stock flame thrower,
flames, afterburn and airblast are
[weapons_more.md](weapons_more.md) §7 and [flames.md](flames.md);
the shield charge as its condition runs it, the banners' pulsed buffs
and the Spy's cloak are [batch3_mechanics.md](batch3_mechanics.md);
fall damage is [movement.md](movement.md) (H_max × v / 6000 above
650 HU/s, ± 20 % spread).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| charge_start_meter | 100 | meter | attack2 starts a charge only from a full meter |
| charge_mini_below | 75 | meter | at the swing, meter ≤ this: mini-crit |
| charge_crit_below | 40 | meter | at the swing, meter ≤ this: full crit (not the Tide Turner) |
| impact_base | 8 | HP | impact damage at no meter used (fitted) |
| impact_slope | 0.7 | HP per meter point used | (fitted) |
| impact_cap | 50 | HP | before the shield's and heads' multipliers |
| impact_head_bonus | 0.1 | × per head | Eyelander heads, up to 5 (× 1.5) |
| phlog_rage_per_damage | 1/3 | meter per HP | fire damage only (300 HP fills) |
| phlog_taunt_invuln | 2.6 | s | invulnerable from the taunt's start |
| phlog_crit_time | 10 | s | rage drain; the crit condition lingers ≈ 1.2 s after |
| backburner_back_angle | ≈ 35 | degrees | attacker within this of the victim's back: crit |
| airblast_cost | 20 / 50 / 25 / 5 | ammo | stock, Backburner, Degreaser, Dragon's Fury |
| degreaser_deploy | 0.2 | s | switching to it (0.5 × 0.4) |
| degreaser_holster | 0.35 | s | switching from it to another weapon (0.5 × 0.7) |
| df_speed | 3000 | HU/s | Dragon's Fury fireball |
| df_range | 500 | HU | then it stops (`tf_fireball_distance` 500) |
| df_refill | 125 | meter per s | pressure, 0.8 s empty to full |
| df_refill_hit | 187.5 | meter per s | from a hit on, × 1.5 |
| df_refill_airblast | 62.5 | meter per s | after an airblast, × 0.5 (1.6 s) |
| df_burning_mult | 3 | × | damage against a burning target |
| manmelter_interval | 2.0 | s | between shots |
| manmelter_speed | 3000 | HU/s | flare × 1.5 |
| extinguish_heal | 20 | HP | Manmelter, Dragon's Fury, stock flame throwers (data) |
| gas_meter_time | 60 | s | Gas Passer empty to full by time |
| gas_meter_damage | 750 | HP | empty to full by damage dealt (data) |
| gas_cloud_life | 5 | s | the cloud |
| gas_coat_time | 10 | s | the gas condition on an enemy |
| gas_cloud_radius | ≈ 220 | HU | coats within (213 in, 238 out) |
| gas_burn | 6 | s | afterburn when a coated enemy is hurt |
| chute_fall | ≈ 106 | HU/s | fall speed while open (fitted: capped at 100 before gravity's half-step) |
| chute_air_speed | 75 | HU/s | horizontal speed while open |
| stomp_base | 10 | HP | Mantreads and Thermal Thruster stomp |
| stomp_mult | 3 | × | the stomper's own fall damage, no 650 threshold subtracted |
| mantreads_force | 0.25 | × | knockback from damage |
| mantreads_airblast | 0.25 | × | airblast power |
| thruster_hop | 350 | HU/s | upward, on the press |
| thruster_delay | 0.65 | s | press to launch |
| thruster_speed | 810 | HU/s | along the aim |
| thruster_charge | 50 | meter | one launch; 2 stored |
| thruster_refill | 3.33 | meter per s | 15 s per launch |
| thruster_stun | 0.5 | s | after the launch |
| thruster_land_speed | 300 | HU/s | horizontal speed kept on landing |
| thruster_push_radius | ≈ 220 | HU | enemies pushed on landing (200 in, 250 out) |
| mangler_speed | 1100 | HU/s | shot |
| mangler_energy | 20 / 5 | points | clip / per shot (4 shots) |
| mangler_interval | 0.8 | s | between shots; one shot back every 0.81 s |
| mangler_charge | 2.0 | s | charged shot from press to shot |
| mangler_charge_speed | 80 | HU/s | the Soldier's max speed while charging |
| heatmaker_kill_rage | 35 | meter | per kill (assist 15, data) |
| heatmaker_drain | 10 | per s | focus lasts 10 s |
| razorback_stall | 2.0 | s | the blocked Spy cannot attack |
| razorback_recharge | 30 | s | meter 0 to 100 (3.33 per s) |

## Behavior

### 1. The shield entity (Chargin' Targe 131, Splendid Screen 406, Tide Turner 1099)

On a bot Demoman with a real shield **(measured)**:

- **Start.** Attack2 starts the charge only when the meter is 100: a bot
  pressing attack2 every 0.1–0.2 s with the meter refilling (78 → 99.6)
  never charged until it reached 100. Any active weapon works.
- **Forward drive.** The charge moves the Demoman forward along his view
  with no movement key held: 0 → 218 → 732 → 750 HU/s over the first
  three ticks, then 750 until the end. When the meter runs out his speed
  is not clamped but decays by ground friction (218, 160, 111, 74 …
  HU/s a tick-group apart). A full charge from a standstill covered
  about 1150 HU.
- **Debuffs removed on the start tick:** burning (real afterburn, its
  ticks stop), the healing debuff that comes with it, bleeding, Jarate,
  Mad Milk and gas. Marked-for-death stays.
- **A melee swing ends the charge on the swing's tick** (meter to 0, the
  crit-boost condition then lingers 0.3 s, covering the smack 0.2–0.27 s
  later).
- **The swing's crit** follows the meter at the swing, the same with or
  without an impact (no random crits):

| Meter at the swing | Chargin' Targe, Splendid Screen | Tide Turner |
|---|---|---|
| above 75 (77, 80, 85, 90 measured) | normal (bottle 65) | normal |
| 75 down to 41 (76*, 74, 70, 60, 50, 45, 43, 42) | mini-crit (88) | mini-crit |
| 40 and below (41*, 39, 36, 33, 30, 10) | full crit (195) | mini-crit (88) |

  \* the sampled meter is one tick before the swing is processed, which
  takes one more point: 76 sampled → 75 at the swing, 41 → 40.
- **Impact.** Running into an enemy ends the charge (the Demoman stops
  about 48 HU from him, box against box) and deals impact damage (its
  own damage kind, never a crit) by the meter used, u = 100 − meter at
  the impact:

```math
D_{impact} = \min\big(50,\ 8 + 0.7\,u\big) \times m_{shield} \times (1 + 0.1\,\min(heads, 5))
```

  (fitted; m_shield 1, the Splendid Screen 1.7). Measured: u = 11 → 16,
  14 → 18, 21 → 23, 26 → 26, 32 → 30, 40 → 36, 44 → 39, 48 → 42,
  53 → 45, 57 → 48, 62 → 50, 71 → 50 (Targe and Tide Turner alike);
  Splendid Screen 11 → 27, 21 → 39, 26 → 45, 53 → 77, 71 → 85, 93 → 85;
  Eyelander (Targe) u = 26 / 71 with 1, 2, 4, 6 heads: 29 / 55, 31 / 60,
  37 / 70, 39 / 75. Impacts happened even from 60 HU away (u = 11).
- **The Splendid Screen's knock** pushes the victim far enough that a
  swing on impact missed at u ≥ 53 (no melee hit landed); the Targe's
  victim stayed in reach.

### 2. Phlogistinator (594)

**(measured)** unless marked.

- No airblast: attack2 with rage below 100 does nothing (no ammo taken,
  no condition).
- **Rage** ("Mmmph"): + 1/3 point per HP of fire damage dealt, the
  unrounded damage (a 12.65 flame hit → + 4.22; each 4 afterburn tick
  → + 1.333); 300 HP fills it. No rage is gained while it drains.
  Only fire damage counts (data: `burn damage earns rage`).
- **Attack2 at 100:** a taunt starts at once; on its first tick the Pyro
  gets the taunting, mega-heal, invulnerable and crit-boost (rage)
  conditions; invulnerable and mega-heal end 2.61 s later, the taunt
  2.76 s later. He is not healed (50/175 stayed 50). Damage taken
  during it is 0 (a scripted 40 → 0).
- The rage drains from the taunt's start, 100 to 0 in 10.0 s; the
  crit-boost condition ends 11.2 s after the start (≈ 1.2 s after the
  rage). Flame hits under it are full crits (12.65 → 38, 3 ×); afterburn
  ticks stay 4 (never crit).
- No random crits (data: `crit mod disabled` 0).

### 3. Backburner (40, Festive 1146)

- **Crits from behind (measured):** with the Pyro 100 HU behind a bot
  whose facing was re-set every tick, every flame hit crit (38, then
  18–19) when the angle between the victim's facing and the direction
  from the victim to the Pyro was 150°–180°; none at 140° and below
  (except sometimes the stream's first hit), mixed at 142°–148° on
  either side. Fitted: a crit when the Pyro is within ≈ 35° of the
  victim's back direction, judged per hit (a hit's own point position
  likely decides near the edge).
- **Airblast** costs 50 (20 × 2.5), same 0.75 s interval **(measured)**.
- Attributes (data): `airblast cost increased` 2.5,
  `mod flamethrower back crit` 1, no random crits.

### 4. Degreaser (215)

- **Switching (measured, the next-attack time after a switch):** to the
  Degreaser 0.2 s (stock 0.5); from it to the Flare Gun or melee
  0.35 s (stock 0.5). Data: `single wep deploy time decreased` 0.4,
  `switch from wep deploy time decreased` 0.7.
- **Airblast** costs 25 **(measured)**.
- **Afterburn** × 0.34 (data `weapon burn dmg reduced`): ticks reported
  as 1 HP each, every 0.5 s **(measured)**; 1.36 unrounded.

### 5. Dragon's Fury (1178)

**(measured)** unless marked.

- **Shot:** one fireball per press, 1 ammo (40 max, data `hidden primary
  max ammo bonus` 0.2 of 200), starting 23.5 HU ahead, 7 right, 9 below
  the eye; 3000 HU/s with no gravity; it passes through players (a bot at
  300 HU was hit and the ball flew on) and stops 500 HU from its start
  (`tf_fireball_distance`), then vanishes.
- **Damage:** about 30 point-blank, 28 at 200 HU, 27 at 300, 24 at 450
  (its own kill type); × 3 to a burning target (85 at 200, 77/75 at 450).
  A hit ignites: 5 afterburn ticks of 4 (≈ 2.5 s).
- **Pressure meter:** firing empties it (100 → 0); it refills at 125 per
  s (0.8 s) and the next shot needs it full. From a hit on, the refill
  is × 1.5 (187.5 per s): a shot hitting at 200 HU refilled in 0.555 s,
  at 80 HU 0.539 s, at 450 HU 0.58 s (the boost starts at the hit).
- **Airblast** (attack2): the stock airblast (weapons_more.md §7.4),
  5 ammo (data `airblast cost scale hidden` 0.25), empties the meter and
  the refill after it runs at 62.5 per s (1.6 s).
- Extinguishing a teammate restores 20 HP (data).

### 6. Manmelter (595)

**(measured)** unless marked.

- **Shot:** a flare (weapons and batch-2 flare rules) at 3000 HU/s
  (× 1.5), 30 damage, every 2.0 s; no ammo is used (the secondary ammo
  stayed 50); no crit against burning targets (a burning target took 30).
- **Attack2 on a burning teammate:** the airblast's sphere reach
  (within 250 HU in front: extinguished; at 300 not), the teammate is
  extinguished, the Pyro gains + 1 stored crit and + 20 HP.
- **Stored crits:** each shot while one is stored is a full crit (90)
  and uses one. No random crits (data).
- "Reloads automatically when not active" (data text); not measured.

### 7. Gas Passer (1180)

**(measured)** unless marked.

- **Meter:** starts empty (data), fills by time in 60 s (1.667 per s)
  and by damage (750 HP fills, data); spawning and resupply do not
  change it (data). A throw empties it.
- **Throw:** a jar from 16 HU ahead of the eye at about 1800–1950 HU/s
  (a physics object; it covered 263 HU in 0.15 s); on contact a cloud
  forms at the impact, 30 HU above it, and lasts 5.0 s.
- **Coating:** enemies within ≈ 220 HU of the cloud's centre (213 in,
  238 out) get the gas condition for 10 s; one entering the cloud while
  it lasts is coated then. Teammates and the thrower are not.
- **Ignition:** the next damage a coated enemy takes (any source; a
  scripted 10 HP was enough) removes the gas and sets him burning for
  6.0 s (11–12 ticks of 4), credited to the thrower. Pyros too (data
  text).
- After the throw the Pyro switches back to his primary 0.7 s later.

### 8. B.A.S.E. Jumper (1101)

**(measured)** on a Soldier. The Demoman wears it as a primary (data).

- **Open:** jump pressed while airborne. Two conditions come on: "active"
  (open) and "deployed" (used this air time). The fall speed is then
  held: velocity reads − 112 at every tick end and the Soldier falls
  1.06 HU a tick (106 HU/s); fitted: the vertical speed is capped at
  − 100 before gravity's half-step.
- **Air control while open:** a held movement key brings the horizontal
  speed to 75 within ≈ 0.1 s (33.7 after one step); releasing it stops
  him within one sample (75 → 0); forward and back alike. Opening with
  400 horizontal left 298 at once, and that speed was lost on release
  too (the run met a wall; Open questions).
- **Close:** jump again while open: "active" goes, "deployed" stays, he
  falls with normal gravity keeping his horizontal speed. A further jump
  press does not reopen it; "deployed" goes only on landing, and one
  that stays airborne (stuck) never reopens.
- Landing with the chute open (≈ 106 HU/s) is far below the 650 HU/s
  fall-damage threshold, so it never hurts; landing closes it.

### 9. Mantreads (444) and the Thermal Thruster (1179)

#### 9.1 Stomp (both) **(measured, spread off)**

- Landing on an enemy's head deals 10 + 3 × the stomper's fall damage
  computed without the 650 threshold, the stomper's own max health:

```math
D_{stomp} = 10 + 3\,\frac{H_{max}\,v}{6000}\qquad (v > 650)
```

  Soldier (200): v = 708 → 80, 900 → 99, 1068 → 116, 1332 → 143; with
  max health 100, 1068 → 63; Pyro (175) on the Thermal Thruster,
  890 → 87. At 504 and 624 HU/s: nothing. The stomper takes no fall
  damage and stands on the victim. Without the attribute landing on a
  player does nothing. The damage has its own kill type.

#### 9.2 Mantreads **(measured)**

- Knockback from damage × 0.25 (a 60 blast in the air: 139.6 → 35.1
  horizontal); on the ground the reduced push did not lift him and was
  lost to friction.
- Airblast power × 0.25 before the airblast's ground lift
  (weapons_more.md §7.4): an airblasted Soldier got 111.8 horizontal and
  100 up (√(150² − 100²)) instead of 591.6 and 100.
- Data: `mod_air_control_blast_jump` 3 (air control × 3 while blast
  jumping; not measured: the blast jump test failed on 2fort).

#### 9.3 Thermal Thruster **(measured)**

- Switching to it is slow (data `holster_anim_time` 0.8).
- **Launch:** attack with a charge (meter ≥ 50): at once a hop (vertical
  speed set to 350) and the launch 0.65 s later. The launch sets the
  velocity to 810 along the aim plus an upward part, and gives a stun
  condition for 0.5 s and the rocket-pack condition until landing. The
  upward part (fitted on the table):

| Aim pitch | Velocity at launch (horizontal, vertical) |
|---|---|
| 5° down | 810, 315 (aim treated as level) |
| level | 810, 315 |
| 5° up | 806.9, 385.6 (= 70.6 + 315) |
| 10° up | 797.7, 455.6 (= 140.7 + 315) |
| 20° up | 761.2, 389.5 (= 277.0 + 112.5) |
| 25° up | 734.1, 454.8 |
| 30° up | 701.5, 517.5 |
| 60° up | 405.0, 814.0 |
| 89° up | 14.2, 922.4 |
| 20° down | 761.2, − 164.5 (= − 277.0 + 112.5) |

  Fitted: 810 × aim + 315 up when the aim is within about 10° of level
  (a downward aim then counted as level), + 112.5 up otherwise.
- **Flight:** the horizontal speed holds (810 for the whole arc; no air
  drag), normal gravity. A second press in flight does nothing (no
  re-launch; an attribute allows it, data).
- **Meter:** 100 holds two launches; each takes 50; it refills 3.33 per s
  (15 s a launch).
- **Landing:** the horizontal speed is cut to 300 on the landing tick,
  then ground friction. Enemies within ≈ 220 HU of the landing point
  (200 pushed, 250 not) are pushed away and up (≈ 270–290 horizontal,
  ≈ 270–285 up) and take no damage unless stomped (§9.1).

### 10. Razorback (57)

**(measured)** on a bot Sniper given the Razorback by the bot item grant,
the local player a Spy with the stock Knife:

- The first backstab is blocked: no damage, the shield's meter drops
  100 → 0, and the Spy cannot attack for 2.0 s (his next-attack time
  set 1.985 s ahead); his speed and conditions do not change.
- The next backstab in that time kills (the shield is broken).
- The meter refills at 3.33 per s (30 s; data `item_meter_charge_rate`
  30). Setting the meter back to 100 by script did not restore the
  block: the shield keeps its own broken state (Open questions).
- Data: no overheal from Medics (`patient overheal penalty` 0); a
  bot's Razorback did not replace its SMG (both carried).

### 11. Cow Mangler 5000 (441)

**(measured)** unless marked.

- **Shots:** an energy ball at 1100 HU/s (no gravity), from 23.5 ahead
  and 8 right of the eye; 4 shots of 5 energy from 20; one every 0.8 s;
  86–97 damage at 400 HU (rocket-like); no ammo (data).
- **Energy back:** 5 at a time, the first 1.67 s after the last shot,
  then every 0.81 s.
- **Charged shot:** attack2 with full energy (15 refused): the Soldier's
  max speed drops to 80 while it charges; the shot fires 2.0 s after the
  press, uses all 20 energy, flies like a normal shot, mini-crits
  (97 → 131) and ignites (6 s of afterburn, 12 ticks of 4). Its own
  kill type.
- **Crits become mini-crits:** under a crit boost a shot did 122
  (90 × 1.35) although the event flagged it as a crit.
- Data: 20 % damage to buildings, disables buildings 4 s (charged), the
  shots cannot be reflected; no random crits.

### 12. Hitman's Heatmaker (752)

**(measured)** unless marked.

- **Focus meter:** + 35 per kill (a 40 body-shot kill: 0 → 35); assists
  + 15 (data). At 100, reload starts focus: the focus condition on that
  tick, the meter drains 10 per s (10.0 s), the condition ends 0.33 s
  after it is empty.
- **While focused** the scope stays up after a shot.
- **Charge rate:** 50 per s, 150 in 3.0 s, with or without focus (four
  runs); the item text's "+25 % faster charge" did not show (Open
  questions).
- Body shots × 0.8 (unscoped 50 → 40); decapitates on a headshot kill
  (data).

## Per-tick order

1. Input: attack2 (shield start, Phlogistinator taunt, Manmelter
   extinguish, charged shot), jump (chute), attack (thruster hop).
2. Timers and meters: the thruster's launch 0.65 s after its hop;
   charge meter, pressure, gas, focus, Phlogistinator rage, energy,
   Razorback refills; the charged shot's 2 s.
3. Movement: the charge's forced forward move; the chute's fall cap and
   75 air speed; the thruster's flight.
4. Hits: impact on bumping (then the charge ends); Dragon's Fury
   through players; gas cloud coating; landing stomp and push.
5. Damage: shield impact multipliers, crit tier from the meter at the
   swing, the Backburner's behind test per hit, × 3 against burning,
   crits to mini-crits (Mangler), gas ignition, the Razorback's block.
6. After damage: rage, meter boosts (Dragon's Fury hit), kill rewards
   (focus).

## Edge cases

- A charge only starts at exactly 100 meter.
- A swing ends the charge even with nobody in reach.
- An impact with more than 75 meter left gives no crit to the swing
  that follows.
- The chute cannot be reopened in the same air time.
- The Dragon's Fury ball passes through its first victim; only one hit
  per victim was seen.
- The Gas Passer's coating ends on the first damage taken, which then
  ignites.
- The thruster's launch cannot be repeated in the air.

## Quirks

- The Splendid Screen's knock can push the victim out of the follow-up
  swing's reach.
- A Backburner stream's first hit sometimes crits at angles where the
  rest do not.
- A level or slightly downward thruster aim gets 315 up; a steeper one
  only 112.5 (keep: it is how the jump feels).
- The Cow Mangler's boosted shots are flagged as crits but do mini-crit
  damage.

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| shield, meter 99 | attack2 | — | no charge |
| shield, meter 100, no keys | attack2 | 3 ticks | speed 750 forward |
| charging, burning + bleeding + Jarate | — | start tick | all removed, marked-for-death kept |
| Targe, charging, meter 74 | swing | smack | bottle 88 (mini-crit) |
| Targe, charging, meter 30 | swing | smack | 195 (crit) |
| Tide Turner, meter 10 | swing | smack | 88 |
| any, meter 85 | swing | — | 65, charge ended at the swing |
| Targe, impact at meter 47 | — | — | 45 impact |
| Targe, impact at meter 29 | — | — | 50 |
| Splendid Screen, impact at meter 29 | — | — | 85 |
| Eyelander 4 heads, impact at meter 29 | — | — | 70 |
| Phlogistinator | deal 300 fire damage | — | rage 100 |
| Phlogistinator, rage 100 | attack2 | 2.6 s | invulnerable, then crits until 11.2 s |
| Phlogistinator, rage 60 | attack2 | — | nothing |
| Backburner, behind (≤ 30° off) | flame | — | crit hits |
| Backburner, 40° off the back | flame | — | normal hits |
| Backburner, 200 ammo | attack2 | — | 150 |
| Degreaser | switch to it / from it | — | 0.2 s / 0.35 s |
| Dragon's Fury | fire at 200 HU | — | 28, ignited, refill 0.555 s |
| Dragon's Fury, burning target | fire at 200 HU | — | 85 |
| Dragon's Fury | attack2 | — | 5 ammo, refill 1.6 s |
| Manmelter, burning teammate at 100 HU | attack2 | — | extinguished, + 1 crit, + 20 HP |
| Manmelter, 1 crit stored | fire | — | 90 crit, 0 stored |
| Gas Passer, full | throw at enemy | — | gas 10 s; cloud 5 s |
| gassed enemy | take 10 | — | gas gone, burning 6 s |
| chute open | fall | 1 s | 106 HU lower |
| chute open | forward | 0.2 s | 75 horizontal |
| chute closed in air | jump | — | stays closed |
| Mantreads, 200 max | land on enemy at 900 | — | 99 (no spread) |
| Mantreads | 60 blast in air | — | push × 0.25 |
| Mantreads | stock airblast, on ground | — | 111.8 out, 100 up |
| Thruster, level aim | attack | 0.65 s | velocity 810, 315 up; meter 50 |
| Thruster, 30° up | attack | 0.65 s | 701.5, 517.5 |
| Thruster, in flight | attack | — | nothing |
| Thruster landing | enemy at 120 HU | — | pushed, no damage |
| Cow Mangler | hold attack | — | 4 shots 0.8 s apart |
| Cow Mangler, full | attack2 | 2.0 s | shot, 131 mini-crit, burning; energy 0 |
| Cow Mangler, energy 15 | attack2 | — | refused |
| Heatmaker | kill | — | focus + 35 |
| Heatmaker, 100 | reload | 10 s | focus over; scope kept after shots |
| Razorback | backstab | — | blocked, meter 0, Spy 2 s no attack |
| Razorback broken | backstab | — | kill |

## Open questions

- The impact damage law's exact form (a line fitted on integers; the
  meter is read one tick early) and whether it scales on distance or
  on time charged.
- The Backburner's exact behind test (which position it uses; the
  stream's first hit).
- The B.A.S.E. Jumper with entry speed: 400 became 298 on opening; how
  speed above 75 decays without input (the run met a wall).
- The thruster's two upward parts: the boundary between + 315 and
  + 112.5 (somewhere between 10° and 20° up, and between 5° and 20°
  down; the 15° up and 10° down runs failed).
- The Heatmaker's charge rate under focus (measured unchanged; the text
  says + 25 %).
- The Razorback's recharge: whether the block returns when the meter is
  full again after 30 s (setting the meter by script did not).
- The Mantreads' blast-jump air control (× 3) and the thruster's
  landing push direction law.
- The Gas Passer's damage fill rate (750 HP, data) and the Manmelter's
  reload while holstered.
- The Phlogistinator's mega-heal condition did not heal; what it does
  besides that (data text: "Invulnerable while taunting").
