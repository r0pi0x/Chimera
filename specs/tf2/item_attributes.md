# Team Fortress 2: item attributes (the batch-1 unlocks' numbers)

Source basis: the retail client's item schema (`scripts/items/items_game.txt`,
read in place: each batch-1 item's `item_class`, `prefab` chain,
`attributes` and `static_attrs`, and each attribute's definition:
`attribute_class`, `description_format`, `stored_as_integer`), Valve's
Source SDK 2013 TF2 game code read in place in a separate spec session
(how an attribute's value is combined, which game number each attribute
class modifies and under what condition; nothing copied, restated in our
words), and the running game (Linux client, listen server on `itemtest`,
muted, driven over RCON through VScript: each unlock created as a live
weapon entity and equipped on the local player, the account's loadout
untouched; clip, max ammo, max speed, max health, projectile speed,
spin-up, charge, damage dealt to a Soldier bot and taken from `hurtme`
read back from the game). Numbers marked **(measured)** were read from
the running game on 2026-11-22; the rest are reference only.
Status: draft

## Summary

An unlock is a stock weapon class plus a list of attributes. Each
attribute names a game number it modifies (its *class*, e.g. the clip, the
fire interval, the damage) and a value; how the value combines is fixed by
the attribute's *format*: percentages multiply, additive values add, flag
values OR. A number is modified at the moment the game asks for it, by
every attribute of that class on the weapon (and, for player numbers, on
every item the player carries that "provides" to him). Most batch-1 items
are nothing more than that; a few carry an attribute whose effect is a
small rule of its own (the Loch-n-Load's shatter, the Quickiebomb's
sticky-destroying, the Bread Bite's max-health drain), and a few have no
stat attributes at all (AWPer Hand, Black Rose, Holy Mackerel,
Rainblower). Section 7 lists, per weapon, what the attributes do not
cover.

The converter reads `items_game.txt` once and writes the stats into an
asset-service pack (user decision); this spec gives what each value means
and how it lands, not a run-time reader.

## Units and conventions

Hammer units (HU), z up, seconds, health points. Server tick 66.67 Hz
(0.015 s). "Base" means the weapon class's own number (its script: a
`tf_weapon_*` class's damage, interval, clip, spread; or a game
constant), "factor" a percentage attribute's stored value (0.75 = -25 %).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| rocket_speed_base | 1100 | HU/s | rocket launch speed before attributes |
| rocket_radius_base | 146 | HU | rocket blast radius vs others before attributes |
| rocket_self_radius | 121 | HU | rocket blast radius for the shooter himself, never modified |
| grenade_speed_base | 1216 | HU/s | grenade launcher forward launch speed (plus 200 up), before attributes |
| grenade_radius_base | 146 | HU | grenade/sticky blast radius vs others before attributes |
| grenade_self_radius | 146 | HU | grenade/sticky radius for the shooter, never modified |
| grenade_fuse | 2.0 | s | grenade launcher pill fuse |
| grenade_fuse_check | 0.2 | s | how often a grenade checks its fuse (it goes off at the first check past it) |
| sticky_arm_base | 0.8 | s | sticky arm time |
| sticky_charge_base | 4.0 | s | sticky full charge time (and forced release) |
| sticky_speed_min / max | 900 / 2400 | HU/s | sticky launch speed at 0 / full charge |
| sticky_max_base | 8 | count | live stickies per launcher |
| minigun_spinup_base | 0.75 | s | minigun wind-up |
| heavy_spun_speed | 110 | HU/s | Heavy's speed cap while spun up |
| jump_impulse | 289 | HU/s | jump (and Scout air dash) vertical speed |
| melee_range | 48 | HU | melee swing trace length |
| sword_range | 72 | HU | sword swing trace length (128 while shield-charging) |
| sword_switch_mult | 1.75 | x | deploy/holster time multiplier when switching to or from a sword |
| switch_base | 0.5 | s | weapon deploy time |
| demo_charge_drain | 1.5 | s | shield charge duration before attributes |
| on_hit_heal_min | 3 | HP | least health an on-hit heal gives |
| regen_period | 1.0 | s | item health regen tick |
| drain_floor | 100 | HP | the Bread Bite's max-health floor |
| drain_deploy_cost | 20 | HP | the Bread Bite's max-health cost per deploy |
| pyro_afterburn | 0.25 | s | afterburn length for a burn-immune target |
| build_rate_base | 1.5 | x | Engineer construction rate per wrench hit (game default) |
| wet_grace | 5.0 | s | time after leaving water a player still counts as wet |

## Behavior

### 1. How attributes combine

**Format → operation.** Every attribute definition carries a
`description_format`; it alone decides the operation (the attribute's
name, "bonus" or "penalty", only decides the tooltip colour):

| `description_format` | Operation on the running value | Identity |
|---|---|---|
| `value_is_percentage` | value × factor | 1 |
| `value_is_inverted_percentage` | value × factor (same math; only the tooltip inverts) | 1 |
| `value_is_additive` | value + v | 0 |
| `value_is_additive_percentage` | value + v | 0 |
| `value_is_or` | value bitwise-OR v (both as integers) | 0 |
| `value_is_from_lookup_table` | value = v | — |

**Where the starting value comes from.** The game asks "what is X for this
weapon/player?" with a starting value (the base) and every attribute of
that class folds into it in turn. For a flag-like question (is this a
sword? mini-crit airborne?) the start is 0 and the additive value is the
answer. So two attributes of the same class on one item stack
multiplicatively (percentages) or by sum (additive), and the order never
matters for a pure product or sum.

**Integer questions round once, at the end.** Clip size, max ammo, max
stickies, flags: the start is converted to a float, every attribute folds
in, and the result is rounded to the nearest integer. Clip 12 × 0.4 =
4.8 → 5 (Winger, measured); 6 × 1.33 = 7.98 → 8 (Family Business,
measured). No batch-1 item lands on an exact .5.

**Weapon vs player numbers.** A weapon's own numbers (damage, clip,
interval, spread, projectile speed and radius, spin-up, charge, fuse)
read only that weapon's attributes. A player's numbers (move speed, max
ammo, max health, damage taken, regen, heal rates, capture value, the
intelligence rule) read every attribute on every item he carries:
weapons and wearables "provide" their attributes to their owner. Two
exceptions:

- An item with `provide on active` = 1 provides only while it is the
  active weapon (Skullcutter's speed penalty, Warrior's Spirit's
  damage-taken, Bushwacka's, Claidheamohmor's, Pretty Boy's). Measured:
  Skullcutter out 238 HU/s, holstered 280; Warrior's Spirit out:
  `hurtme 50` takes 65, holstered 50.
- A weapon never provides to another weapon of the same player.

Some player numbers are asked of **the active weapon** only, regardless
of provision: jump height from weapon, damage from melee/ranged (Fists of
Steel), move speed while active (Bread Bite), max-health drain (Bread
Bite), afterburn immunity on a weapon. Those are "while active" by
construction.

**The base is the item class's script.** An unlock's base numbers are its
`item_class`'s weapon script (e.g. the Winger and Pretty Boy's are
`tf_weapon_handgun_scout_secondary`, swords `tf_weapon_sword`, the Direct
Hit `tf_weapon_rocketlauncher_directhit`), read at run time as the stock
scripts are (weapon_scripts.md). The attribute then multiplies that.
The measured clips, speeds and melee damage below all match "stock
class base × attribute", so these classes' bases equal the stock
weapons' (checked for clip, rocket speed and melee damage only).

### 2. The converter's table

Columns: the attribute's name in `items_game` (the key an item lists),
its definition index, its `attribute_class`, the operation (§1), the
number it modifies (ours: an existing TOML field where there is one, else
a proposed field), and when it applies. "Active" = only while the item is
the active weapon. Attributes listed under "ignore" have no gameplay
effect for us.

#### 2.1 Weapon numbers

| Attribute (name) | Idx | Class | Op | Modifies (ours) | Applies |
|---|---|---|---|---|---|
| damage bonus | 2 | mult_dmg | × | per-shot/pellet/projectile/swing base damage (`[damage] amount`, hook `damage`) | at fire time, before distance, crit and resists |
| damage penalty | 1 | mult_dmg | × | same | same; 0 makes a no-damage weapon (jumpers) |
| clip size bonus | 4 | mult_clipsize | ×, round | `[magazine] clip` | always; only if base clip ≥ 0 (melee untouched) |
| clip size penalty | 3 | mult_clipsize | ×, round | `[magazine] clip` | same |
| fire rate bonus | 6 | mult_postfiredelay | × | `[trigger] cycle` (interval between shots / swings / minigun bullets) | always; reload and melee hit delay untouched |
| fire rate penalty | 5 | mult_postfiredelay | × | same | same |
| weapon spread bonus | 106 | mult_spread_scale | × | `[hitscan] spread` | always |
| Projectile speed increased | 103 | mult_projectile_speed | × | rocket speed 1100; grenade forward speed 1216 (not the 200 up) | at launch |
| Blast radius decreased | 100 | mult_explosion_radius | × | blast radius vs others (146) | at explosion; never the shooter's own radius (§3.3) |
| fuse bonus | 787 | fuse_mult | × | grenade fuse 2.0 s | at launch |
| sticky arm time bonus | 126 | sticky_arm_time | + | sticky arm time 0.8 s | per sticky |
| stickybomb charge rate | 670 | stickybomb_charge_rate | × | full-charge time 4.0 s (also the forced release) | always |
| stickybomb_charge_damage_increase | 727 | stickybomb_charge_damage_increase | × (see §4.6) | sticky damage scaled by charge fraction | at launch |
| max pipebombs decreased | 89 | add_max_pipebombs | +, round | live sticky limit 8 | at each launch |
| minigun spinup time increased | 86 | mult_minigun_spinup_time | × | wind-up 0.75 s | each wind-up |
| minigun spinup time decreased | 87 | mult_minigun_spinup_time | × | same | same |
| single wep holster time increased | 772 | mult_switch_from_wep_deploy_time | × | deploy time of the weapon switched **to** after this one | see §4.9 |
| is_a_sword | 781 | is_a_sword | + (flag; value 72 unused) | melee range 48 → 72; switch ×1.75 | always |
| centerfire projectile | 289 | centerfire_projectile | + (flag) | projectile start offset: centre line, no side offset | at launch |
| grenade no spin | 681 | grenade_no_spin | + (flag) | grenade launched with no angular impulse | at launch |
| grenade no bounce | 671 | grenade_no_bounce | + (flag) | first world touch keeps 10 % of velocity | first touch |
| sticky air burst mode | 127 | set_detonate_mode | + (mode) | 2 = grenades shatter on any surface (§4.5) | per grenade |
| stickies detonate stickies | 121 | stickies_detonate_stickies | + (flag) | detonating clears enemy stickies (§4.6) | on detonation |
| override projectile type | 280 | override_projectile_type | + (enum) | 14 = practice sticky (§4.7) | at launch |
| crit mod disabled | 15 | mult_crit_chance | × | random crit chance; 0 = no random crits | always |
| Set DamageType Ignite | 208 | set_dmgtype_ignite | + (flag) | hit ignites (not from damage-over-time) | per hit |
| mod mini-crit airborne | 114 | mini_crit_airborne | + (flag) | mini-crit vs a blast-airborne target (§4.3) | per hit, not in Mannpower |
| crit vs wet players | 438 | crit_vs_wet_players | OR (flag) | full crit vs a wet target (§4.3) | per hit |
| minicrits become crits | 179 | minicrits_become_crits | + (flag) | any mini-crit becomes a full crit | per hit |
| dmg penalty vs players | 138 | mult_dmg_vs_players | × | damage vs players (after crits, §5) | per hit on a player |
| dmg bonus vs buildings | 137 | mult_dmg_vs_buildings | × | damage vs buildings | per hit on a building |
| dmg penalty vs buildings | 775 | mult_dmg_vs_buildings | × | same | same |
| damage applies to sappers | 146 | set_dmg_apply_to_sapper | + (flag) | melee can hit a sapper on a building | per swing |
| crit kill will gib | 309 | crit_kill_will_gib | + (flag) | a crit kill always gibs | on kill |
| heal on hit for rapidfire | 16 | add_onhit_addhealth | +, round | heal per hit (§4.8) | per hit, not burn damage |
| heal on kill | 180 | heal_on_kill | +, round | heal per kill (§4.8) | on kill with this weapon |
| kill refills meter | 2034 | kill_refills_meter | + | shield charge meter refill per melee kill (fraction of 100) | Demoman, melee or charge-impact kill |
| increased jump height from weapon | 524 | mod_jump_height_from_weapon | × | jump (and Scout air-dash) impulse 289 | active |
| extinguish restores health | 783 | extinguish_restores_health | +, round | health per teammate extinguished | flamethrowers (stock too, from the shared prefab) |
| Construction rate increased | 92 | mult_construction_value | × | build speed per wrench hit (base 1.5) | per hit |
| Repair rate decreased | 95 | mult_repair_value | × | repair per wrench hit | per hit |
| flame_* (gravity, drag, up_speed, speed, spread_degree, lifetime, random_life_time_offset) | 839–865 | same | + (start 0) | the flame particle model | stock prefab values; flames.md already has them |

#### 2.2 Player numbers (provided by the item)

| Attribute (name) | Idx | Class | Op | Modifies (ours) | Applies |
|---|---|---|---|---|---|
| move speed penalty | 54 | mult_player_movespeed | × | class max speed (final step of the speed rules) | while provided |
| mult_player_movespeed_active | 851 | same | × | max speed | asked of the active weapon only |
| aiming movespeed decreased | 183 | mult_player_aiming_movespeed | × | the spun-up/scoped speed cap (Heavy 110) | while spun up |
| maxammo primary increased | 76 | mult_maxammo_primary | ×, round | class primary reserve | while provided |
| maxammo secondary increased | 78 | mult_maxammo_secondary | ×, round | class secondary reserve | while provided |
| dmg taken increased | 412 | mult_dmgtaken | × | all damage taken (whole amount, crit part too) | while provided |
| dmg taken from bullets increased | 67 | mult_dmgtaken_from_bullets | × | bullet/buckshot damage taken, base part only | while provided |
| dmg taken from fire reduced | 60 | mult_dmgtaken_from_fire | × | fire damage taken (burn and ignite types, afterburn too), base part only | while provided |
| spunup_damage_resistance | 738 | spunup_damage_resistance | × | damage taken, base part, while spun up and the hit leaves ≤ 50 % health | Heavy, spun up |
| dmg from ranged reduced | 205 | dmg_from_ranged | × | non-melee damage taken (blast, bullet, buckshot, ignite, sonic) | active weapon only |
| dmg from melee increased | 206 | dmg_from_melee | × | melee damage taken | active weapon only |
| health from healers reduced | 69 | mult_health_fromhealers | × | heal rate from Medics and dispensers | while provided |
| health from packs increased | 108 | mult_health_frompacks | × | health from a pack | while provided |
| health regen | 57 | add_health_regen | + | HP per second (§4.8) | while provided |
| rocket jump damage reduction | 135 | rocket_jump_dmg_reduction | × | own blast damage when the blast hurt no enemy player | while provided |
| no self blast dmg | 181 | no_self_blast_dmg | + (mode) | own blast damage set to 0 (§3.3) | per own blast from this weapon |
| afterburn immunity | 527 | afterburn_immunity | + (flag) | afterburn lasts 0.25 s, as on a Pyro | active weapon, or any Sniper wearable |
| aiming knockback resistance | 377 | mult_aiming_knockback_resistance | × | knockback received | Sniper while scoped |
| aiming no flinch | 376 | aiming_no_flinch | + (flag) | no view flinch on hit | Sniper scoped and fully charged |
| charge time increased | 202 | mod_charge_time | + | shield charge duration 1.5 s | while provided, with a shield |
| increase player capture value | 68 | add_player_capturevalue | + | counts as 1 + v players on a point | while provided |
| cannot pick up intelligence | 400 | cannot_pick_up_intelligence | + (flag) | may not take the flag | while provided |
| mod_maxhealth_drain_rate | 855 | mod_maxhealth_drain_rate | + | max-health drain per second (§4.10) | active weapon only |
| provide on active | 128 | provide_on_active | + (flag) | provision only while active (§1) | — |

#### 2.3 Ignore (no gameplay effect for us)

`kill eater score type` (292), `kill eater score type 2` (293), `kill
eater kill type` (388) (strange counters); `special taunt` (551);
`disable fancy class select anim` (328); `ragdolls plasma effect` (436)
(a death look, a later effect); `vision opt in flags` (406),
`pyrovision only DISPLAY ONLY` (422), `pyrovision opt in DISPLAY ONLY`
(445) (Pyroland display); `lunchbox adds minicrits` (144, class
`set_weapon_mode`): despite the name, on these items it picks a
class-specific mode — Rainblower 3 = rainbow flame look and hit sound,
Bread Bite 2 = its fist type, no stat; `breadgloves properties` (645,
look only); `custom projectile model` (675, the model path, a look);
`allowed in medieval mode` (2029); `limited quantity item` (692);
`min_viewmodel_offset` (796), `weapon_allow_inspect` (731),
`weapon_uses_stattrak_module` (719), `weapon_stattrak_module_scale`
(724) (view and cosmetics).

### 3. Damage, in order

#### 3.1 The attacker's side (at fire time)

1. Base damage (script) × every `mult_dmg` → the shot's damage. For a
   multi-pellet or minigun weapon this is per pellet/bullet; projectiles
   carry it (stickies then add the charge bonus, §4.6). Rounding is the
   existing rule (weapons.md §2.4: pellets summed, rounded once).
2. Distance modifier (weapons.md §2.1–2.3), unchanged by attributes.
3. Crit decision: random crit chance × `mult_crit_chance` (0 → none;
   crit boosts still crit). Then conditions: `mini_crit_airborne`,
   `crit_vs_wet_players` (§4.3). A mini-crit with
   `minicrits_become_crits` becomes a full crit.
4. Crit/mini-crit multiplier (weapons.md §2.5); the crit part is kept as
   a separate "bonus" amount from here on.
5. Against a player: × `mult_dmg_vs_players`. Against a building: ×
   `mult_dmg_vs_buildings` instead.

#### 3.2 The victim's side

6. Type resists act on the **base part only** (the crit bonus is
   untouched): fire types × `mult_dmgtaken_from_fire`; bullet/buckshot ×
   `mult_dmgtaken_from_bullets`; Heavy spun up and the hit would leave him
   at ≤ 50 % of max health: × `spunup_damage_resistance`.
7. Total (base + bonus) × `mult_dmgtaken`.
8. Victim's active weapon: melee hit × `dmg_from_melee`; blast, bullet,
   buckshot, ignite or sonic hit × `dmg_from_ranged`.
9. Self blast (§3.3).
10. Rounded to whole HP (weapons.md §2.4).

Measured on a Soldier bot (no spread, no random crits): Fire Axe 65,
Homewrecker and Maul 49 (65 × 0.75 = 48.75), Back Scratcher 81 (81.25),
Neon Annihilator 52, Volcano Fragment 52 on the hit (then afterburn),
Warrior's Spirit 85 (84.5).

#### 3.3 The shooter's own blast (all launchers)

When a rocket, grenade or sticky hurts its own shooter, the game first
damages everyone else with the item's numbers, then hurts the shooter
with **the class script's base damage** (no `mult_dmg`, no charge bonus)
over a **fixed radius** (rockets 121, grenades and stickies 146; the
radius attribute does not apply). So self-damage and blast jumps are the
same for every launcher of a class, whatever its damage or radius
attributes. Measured, rocket into the floor at the feet, full health:
Rocket Launcher 86 taken, Direct Hit 86 (its ×1.25 and ×0.3 radius do
not apply), Liberty Launcher 64 (= 86 × 0.75, its
`rocket_jump_dmg_reduction`; its own ×0.75 damage does not apply), Rocket
Jumper 0.

Then: if the blast hurt no enemy player, × `rocket_jump_dmg_reduction`
(provided by the item). Then `no_self_blast_dmg`: mode 2 zeroes the
shooter's damage from any of his own blasts while the weapon that fired
it carries it (Rocket Jumper: every rocket it fires); mode 1 zeroes only
the practice-sticky kind (Sticky Jumper). The push appears to come from
the damage before this zeroing, so the jumpers jump like stock (reference
order, not measured; Open questions).

### 4. Attribute families

#### 4.1 Clip, ammo, fire rate, spread

- Clip = round(base × Π factors). Measured: Winger 5, Pretty Boy's 9,
  Liberty Launcher 5, Loch-n-Load 3, Quickiebomb 4, Family Business 8;
  stock Pistol 12, Rocket Launcher 4, Grenade Launcher 4, Stickybomb 8,
  Heavy Shotgun 6.
- Max reserve = round(class reserve × factor). Measured: Rocket Jumper
  60 (20 × 3), Sticky Jumper 72 (24 × 3).
- Interval = script interval × Π fire-rate factors (0.85 = 15 % faster,
  1.2 = 20 % slower). For melee it is the swing-to-swing interval; the
  hit's delay after the swing starts does not scale. For the minigun it
  is the bullet interval.
- Spread = script spread × factor (Tomislav 0.8).

#### 4.2 Projectiles and explosions

- Rocket speed = 1100 × factor. Measured: Liberty 1540, Direct Hit 1980.
- Grenade forward speed = 1216 × factor (the extra 200 up and the
  ±10 random side and up components are untouched). Measured: the
  Loch-n-Load's pill was ~1.27 × the stock's speed a few ticks after
  launch (drag acts on both).
- Blast radius vs others = 146 × factor: Direct Hit 43.8, Loch-n-Load
  109.5, Iron Bomber 124.1. The sticky air-detonation radius ramp
  (weapons_more.md §5.3) multiplies this.
- Fuse = 2.0 × factor; the pill goes off at its first fuse check after
  that (checks every 0.2 s from launch). Measured lifetimes: stock 2.16
  s, Iron Bomber 1.575 s (1.4 + check granularity).

#### 4.3 Crit conditions

- *Airborne* (Direct Hit): the victim is off the ground, not in water,
  and is in the "blast-jumping" state (sent airborne by any explosion's
  push, his own included) or using a rocket pack. A hit then mini-crits.
- *Wet* (Neon Annihilator): the victim is covered in Jarate, Mad Milk or
  gas, or is in water at any depth, or left the water less than 5 s ago.
  A hit then fully crits.
- *Promotion* (Bushwacka): any mini-crit, from any source (a buff,
  Jarate, a condition), becomes a full crit.
- `crit mod disabled` = 0: no random crits (Rocket Jumper, Sticky
  Jumper, Neon, Claidheamohmor, Bushwacka).

#### 4.4 Movement

- Max speed: the class speed rules run as today, then × every
  `mult_player_movespeed` provided, then × the active weapon's
  `mult_player_movespeed_active`. Measured: Skullcutter active 238 (280 ×
  0.85), holstered 280; Bread Bite active 299 (230 × 1.3).
- Spun-up cap: Heavy 110 × `mult_player_aiming_movespeed`. Measured:
  Brass Beast 44, stock and Tomislav 110.
- Jump: vertical impulse 289 × the active weapon's
  `mod_jump_height_from_weapon` (× surface factor); the Scout's air dash
  uses the same factor. Measured: Winger 361.25 vs 289 (sampled a tick
  apart under 800 gravity: 295.25 vs 235).

#### 4.5 Grenades: shatter, no bounce, no spin

- *Shatter* (`sticky air burst mode` = 2 on a grenade launcher): the
  pill, on its first physics contact with anything that is not a direct
  enemy hit (world, prop, teammate), vanishes in a puff without any
  blast damage. Only direct hits on enemies explode.
- *No bounce*: on the first world contact the pill keeps 10 % of its
  velocity (it stops nearly dead and rolls little); later contacts are
  normal.
- *No spin*: launched with no tumble.

#### 4.6 Sticky launcher

- Arm time = 0.8 + v (Quickiebomb -0.2 → 0.6 s). Quirk: the
  air-detonation radius ramp still starts at 0.8 s.
- Full charge time = 4.0 × factor (Quickiebomb 0.3 → 1.2 s). Launch
  speed ramps 900 → 2400 over that time; at that time the sticky is
  forced out. Measured: forced release 4.07 s (stock) and 1.26 s
  (Quickiebomb) after the press, including RCON latency.
- Charge damage: damage = d + d × (f − 1) × c, with d the sticky's damage
  after `mult_dmg`, f the factor (1.35) and c the charge fraction 0…1.
  Quickiebomb at full charge: base × 0.85 × 1.35.
- Max live stickies = round(8 + v); launching past it detonates the
  oldest. Sticky Jumper 2 (measured).
- *Destroy enemy stickies* (Quickiebomb): when one of its stickies
  explodes, every enemy sticky within that explosion's radius, in clear
  line of sight (world geometry only), is removed without exploding.

#### 4.7 Jumpers

Rocket Jumper and Sticky Jumper: damage × 0 (no damage to anyone), no
random crits, own blasts do no damage (§3.3), reserve × 3, cannot carry
the intelligence. The Sticky Jumper fires the *practice* sticky: its own
model and kill-feed kind, otherwise a sticky. Blast jumps match stock.

#### 4.8 Health gains

- *On hit* (Pretty Boy's 3): heal = max(3, trunc(v × min(1, dealt /
  base dealt))) per damaging hit, not from burn damage, capped at max
  health (no overheal). With v = 3 every hit heals 3.
- *On kill* (Warrior's Spirit 50): heal = min(v, max overheal − health),
  given as normal healing capped at max health.
- *Regen* (Cozy Camper 4): once per second, gain v × s, s = 0.25 if hurt
  in the last 5 s, else rising linearly from 0.5 at 5 s to 1.0 at 10 s
  since the last hurt; fractions accumulate, whole points heal, up to max
  health.
- *From packs* (Back Scratcher 1.5): pack heal = ceil(max health × pack
  ratio) × 1.5, then capped at max health.
- *From healers* (Back Scratcher 0.25): the Medic's and dispenser's heal
  rate on him × 0.25.

#### 4.9 Switching

Deploy time = 0.5 × multipliers. The switched-*from* weapon's
`mult_switch_from_wep_deploy_time` counts only if it had been out longer
than 0.5 s (Fists of Steel 2.0 → the next weapon takes 1.0 s; Bread Bite
1.5 → 0.75 s). A sword on either side multiplies by 1.75 (switching to
or from the Skullcutter or Claidheamohmor takes 0.875 s). The draw
animation plays at the matching rate.

#### 4.10 Bread Bite: max-health drain

While the Bread Bite is the active weapon, max health drains at 10/s
(fractions accumulated); each deploy after a refill costs 20 at once.
Current health falls in proportion (the fraction health/max is kept,
health never below 1). Max health never falls below 100. When holstered,
max health refills at the same 10/s, health rising in proportion (up to
the normal overheal cap). Measured: Heavy 300 → 270 about 1 s after
deploy, then 239 → 199 over 4.0 s; health tracked max at full health.

#### 4.11 Melee extras

- Sword (item class `tf_weapon_sword` with `is_a_sword`): swing range 72
  (128 while shield-charging), the 1.75 switch rule (§4.9), and a kill
  counts as a decapitation (the head-off death look). These two swords
  collect no heads.
- Ignite on hit (Volcano Fragment): the hit sets the victim alight as a
  flame hit would (afterburn per flames.md); damage-over-time ticks do
  not re-ignite.
- Hits sappers (Homewrecker, Maul, Neon): the swing first traces for
  buildings ignoring players, so a sapper on a building is hit.

#### 4.12 Sniper wearables

- Darwin's Danger Shield: fire damage taken × 0.5 (base part, afterburn
  included), and afterburn on him lasts a fixed 0.25 s.
- Cozy Camper: regen 4/s (§4.8); while scoped, knockback received × 0.8;
  while scoped and fully charged, no flinch from hits.

### 5. The 33 items (31 entries), resolved

Def index, item class, attributes (gameplay only), and what is **not**
an attribute.

| Item | Idx | Class | Attributes → effective numbers | Not an attribute (needs its own spec or nothing) |
|---|---|---|---|---|
| Winger | 449 | tf_weapon_handgun_scout_secondary | dmg ×1.15; clip 5; jump ×1.25 active | — |
| Pretty Boy's Pocket Pistol | 773 | tf_weapon_handgun_scout_secondary | provide on active; heal 3 on hit; interval ×0.85; clip 9 | — (schema current as read; older versions differ) |
| Holy Mackerel | 221 | tf_weapon_bat_fish | none (strange counters only) | Bat stats; the class's hit announcements in the kill feed (look) |
| Liberty Launcher | 414 | tf_weapon_rocketlauncher | speed 1540; dmg ×0.75; clip 5; self blast ×0.75 | — |
| The Original | 513 | tf_weapon_rocketlauncher | centre-fire | the rocket starts on the eye's centre line (measured lateral offset 0 vs ~9 HU stock at 254 HU out); its own models |
| Rocket Jumper | 237 | tf_weapon_rocketlauncher | dmg ×0; no random crits; own blasts 0 (mode 2); reserve 60; no intelligence | — |
| Direct Hit | 127 | tf_weapon_rocketlauncher_directhit | radius ×0.3; speed 1980; dmg ×1.25; mini-crit airborne | its own kill icon kind only; the class's script gives base numbers |
| Pain Train | 154 | tf_weapon_shovel | capture value +1; bullet damage taken ×1.1 (always) | — (Soldier and Demoman) |
| Rainblower | 741 | tf_weapon_flamethrower | none (flame prefab values as stock) | rainbow flame particles and hit sound, bubble-wand taunt model (look); Pyroland display flags |
| Homewrecker / Maul | 153 / 466 | tf_weapon_fireaxe | buildings ×2; players ×0.75; hits sappers | — |
| Neon Annihilator | 813 | tf_weapon_breakable_sign | players ×0.8; crit vs wet; hits sappers; no random crits | plasma ragdoll (look) |
| Back Scratcher | 326 | tf_weapon_fireaxe | dmg ×1.25; healers ×0.25; packs ×1.5 | — |
| Sharpened Volcano Fragment | 348 | tf_weapon_fireaxe | ignite; dmg ×0.8 | afterburn length/damage from flames.md |
| Loch-n-Load | 308 | tf_weapon_grenadelauncher | clip 3; buildings ×1.2; speed ×1.25; radius ×0.75; shatter; no spin | — |
| Iron Bomber | 1151 | tf_weapon_grenadelauncher | radius ×0.85; no bounce; fuse ×0.7 | its pill model (look) |
| Sticky Jumper | 265 | tf_weapon_pipebomblauncher | dmg ×0; no random crits; own practice stickies 0 (mode 1); reserve 72; practice sticky; max 2; no intelligence | practice sticky model |
| Quickiebomb Launcher | 1150 | tf_weapon_pipebomblauncher | arm 0.6; charge 1.2 s; charge dmg up to ×1.35; destroys enemy stickies; clip 4; dmg ×0.85 | its sticky model |
| Scotsman's Skullcutter | 172 | tf_weapon_sword | sword; provide on active; dmg ×1.2; speed ×0.85 (active) | sword rules (§4.11) |
| Claidheamohmor | 327 | tf_weapon_sword | kill refills 25 % charge; sword; charge +0.5 s; provide on active; damage taken ×1.15 (active); no random crits | shield charge itself (batch 3) |
| Brass Beast | 312 | tf_weapon_minigun | dmg ×1.2; spin-up 1.125 s; spun speed 44; ×0.8 taken when spun and ≤ 50 % | — |
| Tomislav | 424 | tf_weapon_minigun | spin-up 0.6 s; no spin sounds; interval ×1.2; spread ×0.8 | no wind/spin loop sounds (sound flag) |
| Family Business | 425 | tf_weapon_shotgun | dmg ×0.85; clip 8; interval ×0.85 | — |
| Warrior's Spirit | 310 | tf_weapon_fists | provide on active; damage taken ×1.3 (active); dmg ×1.3; heal 50 on kill | — |
| Fists of Steel | 331 | tf_weapon_fists | ranged taken ×0.6, melee taken ×2.0 (active); next deploy ×2 | — |
| Apoco-Fists | 587 | tf_weapon_fists | crit kills gib | Fists stats; its taunt |
| Bread Bite | 1100 | tf_weapon_fists | speed ×1.3 active; max-health drain 10/s (§4.10); next deploy ×1.5 | — |
| Jag | 329 | tf_weapon_wrench | build ×1.3; interval ×0.85; repair ×0.8; dmg ×0.75; buildings ×0.67 | — |
| AWPer Hand | 851 | tf_weapon_sniperrifle | none | a Sniper Rifle with its own models and sounds |
| Bushwacka | 232 | tf_weapon_club | provide on active; damage taken ×1.2 (active); mini-crits → crits; no random crits | — |
| Darwin's Danger Shield | 231 | tf_wearable | afterburn immunity; fire taken ×0.5 | a wearable, no weapon slot use |
| Cozy Camper | 642 | tf_wearable | regen 4/s; scoped knockback ×0.8; scoped full-charge no flinch | a wearable |
| Black Rose | 727 | tf_weapon_knife | none | a Knife with its own models; taunt |

Attribute values are stored in the schema as given; "measured" rows in
§4 confirm the conversion for clip (5 items), reserve (2), rocket speed
(2), grenade fuse, sticky charge, sticky limit, spin-up (2), spun speed,
move speed (2), jump, melee damage (6 items), damage taken (1) and own
blast (4).

## Per-tick order

Attributes add no tick steps of their own; they are folded in wherever
the base number is read:

1. At fire: interval, clip use, spread, projectile speed and start
   offset, projectile damage (and sticky charge bonus).
2. In flight: fuse checks every 0.2 s; first-touch rules (shatter, no
   bounce).
3. On hit: §3.1 then §3.2 order; on-hit heal after the damage.
4. On kill: heal on kill, charge refill, gib.
5. Player think: max speed (each time it is recomputed: deploy, holster,
   spin state), regen once a second, max-health drain on each health
   query (effectively each tick while active).

## Edge cases

- An attribute with factor 0 on damage still fires projectiles and
  pushes (jumpers).
- Attributes on a holstered `provide on active` weapon do nothing; the
  switch takes effect at the moment the weapon becomes (in)active.
- Two batch-1 items never stack the same class on one player in a party
  game unless a player carries both (e.g. Pain Train and Darwin's are
  different classes); if they do, factors multiply.
- `mult_dmg_vs_buildings` replaces `mult_dmg_vs_players` for building
  targets; neither applies to the other.

## Quirks

- Self blast damage ignores the item's damage and radius (§3.3): keep.
- Grenade fuse resolves on 0.2 s checks (§4.2): keep for parity.
- The Quickiebomb's shorter arm time does not move the air-detonation
  radius ramp, which still starts at 0.8 s: keep.
- `lunchbox adds minicrits` is a mode switch, not mini-crits.

## Test cases

| Setup | Input | After | Expected |
|---|---|---|---|
| Winger equipped | read clip | — | 5 (12 × 0.4 = 4.8 → 5) |
| Family Business | read clip | — | 8 (7.98 → 8) |
| Pretty Boy's | read clip | — | 9 |
| Loch-n-Load | read clip | — | 3 |
| Quickiebomb | read clip | — | 4 |
| Rocket Jumper equipped | ammo pack | — | primary reserve 60 |
| Sticky Jumper | ammo pack | — | secondary reserve 72 |
| Liberty Launcher | fire | first tick | rocket speed 1540 |
| Direct Hit | fire | first tick | rocket speed 1980; radius vs others 43.8 |
| Soldier 200 HP, stock RL | rocket at own feet | blast | 86 taken |
| same, Direct Hit | same | blast | 86 taken |
| same, Liberty Launcher | same | blast | 64 taken |
| same, Rocket Jumper | same | blast | 0 taken |
| Iron Bomber | fire into the air | — | detonates at the first 0.2 s check after 1.4 s |
| Quickiebomb | hold fire | — | forced launch at 1.2 s, 2400 HU/s |
| Quickiebomb, charge c | launch | — | damage = 0.85 b (1 + 0.35 c) |
| Sticky Jumper | fire 3 | — | 2 live, the oldest gone |
| Brass Beast | spin up | — | spun at 1.125 s; max speed 44 |
| Tomislav | spin up | — | spun at 0.6 s |
| Skullcutter | active / holstered | — | max speed 238 / 280 |
| Bread Bite, Heavy 300 | deploy | 1 s | max health 270; then −10/s, floor 100 |
| Winger active | jump | first tick | vertical 361.25 |
| Fire Axe / Homewrecker / Back Scratcher / Neon / Volcano / Warrior's Spirit | hit a Soldier, no spread, no crit | hit | 65 / 49 / 81 / 52 / 52 / 85 |
| Warrior's Spirit active / holstered | take 50 generic | — | 65 / 50 |
| Pretty Boy's | hit for 1 damage | — | heal 3 |
| Brass Beast spun, 300 HP | take a 100-damage bullet hit at 180 HP | — | 80 (the hit leaves ≤ 150) |
| Darwin's | take a 10 burn tick | — | 5; afterburn ends 0.25 s after the last flame |

## Open questions

- Damage-taken stacking on crits: `mult_dmgtaken_from_*` skip the crit
  bonus, `mult_dmgtaken` does not (reference only; measure a crit rocket
  on a Pain Train Soldier).
- Rocket Jumper knockback equal to stock was taken from the reference's
  order, not measured (compare jump apex stock vs Rocket Jumper).
- Pretty Boy's Pocket Pistol: the schema read is the current item
  (provide on active, +3 on hit, ×0.85 interval, ×0.75 clip); the
  community wiki describes older versions. The schema wins.
- The Original's exact start offset (centre line, forward distance) was
  measured only laterally; the forward/up parts are assumed to be the
  stock rocket's.
- Fire-rate factors on the minigun: whether the 1.2 also slows the
  first bullet after spin-up (not measured).
