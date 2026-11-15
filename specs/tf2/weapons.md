# Team Fortress 2: weapons — Scattergun, Rocket Launcher, Shotgun, Minigun (T-W1)

Source basis: public community documentation of the weapons (the community
TF2 wiki's weapon pages, its pages on damage, critical hits, hitscan,
projectiles and rocket jumping, its talk and user pages on spread and
projectile offsets) and Valve's public update notes; from the dedicated
server install on this box (build 11191711, read in place, nothing
decrypted): the item schema `scripts/items/items_game.txt`, the plain sound
scripts and kill-icon table inside `tf2_misc_dir.vpk`
(`scripts/game_sounds_weapons.txt`, `scripts/mod_textures.txt`), the
sequence lengths and attachments of the class arms and `c_` weapon models,
and the defaults of the server's console variables as printed by the
server's own `help` command. The per-weapon scripts are encrypted and were
not read. Timing values on the wiki are community measurements; where this
spec derives a formula from published numbers it says so.
Status: draft

## Summary

TF2's guns are simple compared with CS:S: no recoil, no movement
inaccuracy, no headshots on these four, no penetration. Feel comes from
three shared rules. **Distance**: every hit is scaled by a distance
modifier between 1.5 (point blank) and 0.5 (1024 units and beyond) that
follows a smooth cubic; the Scout's primary ramps harder up close (×1.75)
and the rocket launcher softer (×1.25). **Crits**: each shot (or, for the
minigun, each second) may roll a random critical hit worth ×3 with no
distance penalty; the chance grows with the damage the player has dealt
lately. **Spread**: pellet weapons scatter in a fixed cone, but the first
pellet after a short pause goes dead centre, and servers may swap the
random scatter for a fixed grid. On top of that the Rocket Launcher's
splash, self-damage and knockback make rocket jumping, and the Minigun
must spin up, slowing and grounding the Heavy while spun.

## Units and conventions

- Hammer units ("units", inches), Z up, X forward, Y left; degrees.
- Time in seconds. Server tick 0.015 s (66.67 Hz). As in
  specs/hl2dm/weapons.md, timers are absolute times; an action due at T
  happens on the first tick with time ≥ T.
- Damage in hit points; the four weapons' wielders have 125 (Scout),
  200 (Soldier), 175 (Pyro), 300 (Heavy), 125 (Engineer) maximum health
  (for context only; health is not this spec's).
- "Wiki" means the community TF2 wiki; its numbers reflect the live game
  as of its last edits (2024–2026); dated patch notes are given where they
  matter.
- Spread values `s` are in the Source convention: a pellet's direction is
  forward + a·s·right + b·s·up (then normalised), with a and b random in
  [−1, 1] (see §6 for the distribution), so `s` is the tangent of the
  widest offset on each axis.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| tick | 0.015 | s | server tick |
| dist_near | 0 | units | modifier 1 + R at or below this distance |
| dist_mid | 512 | units | modifier exactly 1 |
| dist_far | 1024 | units | modifier 1 − R at or beyond this distance |
| damage_range R | 0.5 | – | `tf_damage_range` default; half-width of the modifier |
| spread_jitter | 102.4 | units | ±0.1 × dist_far: random distance jitter with spread on |
| spread_off_far_clamp | 921.6 | units | with spread off, farther distances count as this |
| ramp_scale_scout_primary | 1.5 | – | near bonus multiplied by this (175 % at 0) |
| ramp_scale_rocket | 0.5 | – | near bonus multiplied by this (125 % at 0) |
| ramp_scale_default | 1.0 | – | shotgun, minigun (150 % at 0) |
| crit_mult | 3.0 | – | full critical hit |
| minicrit_mult | 1.35 | – | mini-crit (reference) |
| crit_base_chance | 0.02 | – | per roll, ranged weapons |
| crit_max_mult | 4 | – | chance multiplier at ≥ 800 recent damage (8 %) |
| crit_recent_damage | 800 | hp | damage for the full multiplier |
| crit_recent_window | 20 | s | damage history window |
| rapid_crit_roll_interval | 1.0 | s | minigun rolls once per second while firing |
| rapid_crit_duration | 2.0 | s | every shot crits for this long after a success |
| crit_bucket_default / cap / bottom | 300 / 1000 / −250 | – | `tf_weapon_criticals_bucket_*` (Open questions 6) |
| first_pellet_pause | 0.25 | s | pellet weapons (and the minigun) put one pellet dead centre if not fired within this |
| bullet_range | ≈ 56 754 | units | effective maximum trace of a bullet (wiki "Hitscan") |
| deploy | 0.5 | s | weapon switch (draw) time, every stock weapon (since 2015-12-17) |
| holster | 0 | s | stock weapons put away at once |
| rocket_speed | 1100 | units/s | constant, no gravity |
| rocket_offset_stand | (23.5, 12, −3) | units | spawn point from the eye (forward, right, up) |
| rocket_offset_crouch | (23.5, 12, 8) | units | same, crouched |
| blast_radius | 146 | units | rocket explosion |
| blast_edge_fraction | 0.5 | – | damage fraction at the radius edge |
| self_damage_air_scale | 0.60 | – | `tf_damagescale_self_soldier`: Soldier's own blast while airborne |
| force_scale_other | 6.0 | – | `tf_damageforcescale_other` |
| force_scale_self_rj | 10.0 | – | `tf_damageforcescale_self_soldier_rj` |
| force_scale_self_badrj | 5.0 | – | `tf_damageforcescale_self_soldier_badrj` |
| heavy_knockback_scale | 0.5 | – | the Heavy takes half knockback |
| minigun_wind_up | 0.75 | s | spin-up before the first shot |
| minigun_wind_down | 1.15 | s | after releasing fire (1.4 after releasing attack2) |
| minigun_ramp_time | 1.0 | s | damage and accuracy penalty fade over this |
| minigun_ramp_start | 0.5 | – | damage factor at the start of the ramp |
| heavy_speed / spun | 230 / 110 | units/s | Heavy's top speed, normally / while spun or firing |

Console variables (server defaults read from the dedicated server's
`help`):

| Cvar | Default | Effect here |
|---|---|---|
| `tf_damage_disablespread` | 1 | 1: no random distance jitter (§2.3) |
| `tf_damage_range` | 0.5 | R in §2.1 |
| `tf_use_fixed_weaponspreads` | 0 | 1: multi-pellet weapons fire a fixed grid (§6.2) |
| `tf_weapon_criticals` | 1 | random crits on |
| `tf_weapon_criticals_melee` | 1 | melee crits follow `tf_weapon_criticals` (not used here) |
| `tf_weapon_criticals_distance_falloff` | 0 | 0: crits ignore distance |
| `tf_weapon_minicrits_distance_falloff` | 0 | 0: mini-crits ignore fall-off |
| `tf_damagescale_self_soldier` | 0.60 | §5.3 |
| `tf_damageforcescale_other` | 6.0 | §5.4 |
| `tf_damageforcescale_self_soldier_rj` | 10.0 | §5.4 |
| `tf_damageforcescale_self_soldier_badrj` | 5.0 | §5.4 |
| `sv_gravity` | 800 | (rockets ignore it) |

Casual matchmaking servers are documented to run with random bullet
spread on (`tf_use_fixed_weaponspreads` 0); competitive configurations
commonly set it to 1. Chimera's default: follow the cvar defaults above.

## Behavior

### 1. The four weapons at a glance

| | Scattergun | Rocket Launcher | Shotgun | Minigun |
|---|---|---|---|---|
| Class, slot | Scout, primary | Soldier, primary | Engineer primary; Soldier, Pyro, Heavy secondary | Heavy, primary |
| Item defindex, name | 13 `TF_WEAPON_SCATTERGUN` | 18 `TF_WEAPON_ROCKETLAUNCHER` | 9 `TF_WEAPON_SHOTGUN_PRIMARY`, 10 `_SOLDIER`, 11 `_HWG`, 12 `_PYRO` | 15 `TF_WEAPON_MINIGUN` |
| Item class | `tf_weapon_scattergun` | `tf_weapon_rocketlauncher` | `tf_weapon_shotgun` (per-class variants by the game) | `tf_weapon_minigun` |
| Kind | hitscan, 10 pellets | projectile + splash | hitscan, 10 pellets | hitscan, 4 bullets |
| Base damage | 6 / pellet | 90 / rocket | 6 / pellet | 9 / bullet |
| Near ramp at 0 units | 175 % | 125 % | 150 % | 150 % |
| Far fall-off (spread off) | 52.8 % | 52.8 % | 52.8 % | 52.8 % |
| Fire interval | 0.625 s | 0.8 s | 0.625 s | 0.105 s |
| Spread s | 0.0675 | – | 0.0675 | 0.08 |
| Clip / reserve | 6 / 32 | 4 / 20 | 6 / 32 | 200 (no clip) |
| Ammo per shot | 1 | 1 | 1 | 1 (per 4-bullet shot) |
| Reload | one shell: first 0.7, then 0.5 each | one rocket: first 0.92, then 0.8 each | one shell: first 0.87–1.035 by class, then 0.5 (0.51 measured) each | none |
| Crit rolls | every shot | every shot | every shot | once per second, 2 s bursts |
| Deploy | 0.5 s | 0.5 s | 0.5 s | 0.5 s |

The upgradeable (paint-kit) copies of the same weapons, defindex 200
(Scattergun), 205 (Rocket Launcher), 199 (multi-class Shotgun), 202
(Minigun), behave identically.

### 2. Damage and distance (shared)

#### 2.1 The distance modifier

Inputs: the distance d between attacker and victim when the hit lands
(for a rocket: the Soldier's position at the moment of impact, not where
he fired from); the weapon's ramp scale k; R = `tf_damage_range`.

Let t be the normalised distance (§2.3 says how d becomes t). Then:

```math
S(t) = 3t^2 - 2t^3,\qquad M_0(t) = 1 + R\,(1 - 2\,S(t))
```

so M₀(0) = 1.5, M₀(0.5) = 1.0, M₀(0.9) = 0.528, M₀(1) = 0.5. (The wiki
calls the curve "cubic" and gives these four points; the cubic through
them is unique, and it is the smoothstep above.)

The near half is scaled per weapon:

```math
M(t) = \begin{cases} 1 + k\,(M_0(t) - 1) & M_0(t) > 1 \\ M_0(t) & \text{otherwise} \end{cases}
```

k = 1.5 for the Scout's primaries (the Scattergun: 1.75 at 0), 0.5 for
rocket launchers (1.25 at 0), 1 for the Shotgun and the Minigun.

Damage of one hit before crits: base × M. Distance is measured between
the attacker's and the victim's centres (Open questions 2).

#### 2.2 Worked modifiers (spread off)

| d (units) | t | M₀ | Scattergun M | Rocket M | Shotgun/Minigun M |
|---|---|---|---|---|---|
| 0 | 0 | 1.5 | 1.75 | 1.25 | 1.5 |
| 256 | 0.25 | 1.34375 | 1.515625 | 1.171875 | 1.34375 |
| 512 | 0.5 | 1.0 | 1.0 | 1.0 | 1.0 |
| 768 | 0.75 | 0.65625 | 0.65625 | 0.65625 | 0.65625 |
| ≥ 921.6 | 0.9 | 0.528 | 0.528 | 0.528 | 0.528 |

#### 2.3 Random damage spread (`tf_damage_disablespread`)

- **Off (1, the default)**: t = min(d, 921.6) / 1024. The far end stops at
  52.8 %, which is why the wiki's "maximum fall-off" for every weapon is
  52.8 % and not 50 %.
- **On (0)**: t = clamp(d / 1024 + j, 0, 1) with j a random offset in
  [−0.1, +0.1] (±102.4 units) drawn per attack. This makes about ±15 %
  variance and applies even to weapons that ignore distance (melee).
  Distribution of j: Open questions 1.

#### 2.4 Pellets

Each pellet of a shot is traced separately; all pellets of one shot that
hit the same victim add up to one damage event (one damage number, one
pain sound, one knockback). The distance modifier is the same for every
pellet of that shot on that victim. The total is rounded to the nearest
whole hit point (Open questions 3).

#### 2.5 Crits and mini-crits

- **Crit**: damage = 3 × base, with **no** distance modifier (neither ramp-up
  nor fall-off: `tf_weapon_criticals_distance_falloff` 0) and no random
  spread. An exploding crit still loses damage with distance from the
  blast centre (§5.2). Crit sounds and effects: the `…Crit` sound entries
  (§9).
- **Mini-crit** (reference only; none of these four mini-crit on their
  own): damage = 1.35 × base × max(M, 1): fall-off is ignored, ramp-up is
  kept. A crit overrides a mini-crit; they don't stack.
- A player's own crit rocket deals no crit damage to himself.

#### 2.6 Random critical hits

- Chance of one roll, ranged weapons:

```math
p = 0.02 \times \left(1 + 3\,\min\!\left(\frac{D}{800}, 1\right)\right)
```

  with D the damage the player dealt in the last 20 s (2 % at 0, 3.5 % at
  200, 8 % at ≥ 800).
- **Single-shot weapons** (Scattergun, Shotgun, Rocket Launcher) roll once
  per shot, at the moment of firing; the whole shot (all pellets, the
  rocket and its explosion) is a crit or not.
- **Rapid-fire weapons** (the Minigun) roll at most once per second while
  firing; a success makes every shot fired in the next 2 s a crit (the
  crit fire sound loop plays instead of the normal one).
- `tf_weapon_criticals` 0 turns all random crits off.
- A per-player damage "bucket" (cvars `tf_weapon_criticals_bucket_default`
  300, `_cap` 1000, `_bottom` −250) also limits how often crits can
  happen; its rules are not publicly documented (Open questions 6).
  Tests use crits forced on or off.

#### 2.7 No headshots, no damage to teammates

None of the four weapons has a headshot bonus: all hitboxes ×1. With
friendly fire off (the default), bullets and rockets do nothing to
teammates; rockets fly through them. The Soldier's own rockets hurt him
(§5.3).

### 3. Switching

- Every stock weapon deploys in **0.5 s** (Tough Break update,
  2015-12-17: was 0.67 s): the new weapon can fire 0.5 s after the switch
  command. The holster is instant. The draw animation plays meanwhile but
  its length doesn't set the time (the arms' draw sequences are 0.8–1.37 s,
  §8).
- The Minigun can't be switched away while it is winding up, spun, firing
  or winding down (§7).
- Switching away cancels a reload (no ammo moved for the shell in
  progress; shells already in stay).

### 4. Reloading (shared)

The Scattergun, Shotgun and Rocket Launcher reload **one round at a time**:

1. A reload starts when the player presses reload with clip < max and
   reserve > 0, or automatically when the clip reaches 0 with reserve > 0,
   once the fire interval since the last shot has passed.
2. A **start** phase, then one **insert** per round: at the end of each
   insert, clip + 1, reserve − 1. Inserts repeat until the clip is full or
   the reserve is empty, then a finish animation plays (it does not block
   firing).
3. **Pressing attack with at least one round in the clip interrupts the
   reload at once** (the insert in progress is lost) and fires if the fire
   interval has passed. With an empty clip, attack is ignored and the
   reload continues.
4. Client option `cl_autoreload`: when on, a weapon not full starts
   reloading whenever the player isn't firing (Open questions 7 for its
   default). The server-side empty-clip reload of step 1 happens either
   way.

Times (first = start + first insert, as the wiki measures "first reload"):

| Weapon | Start | Insert | First round in | Full from empty |
|---|---|---|---|---|
| Scattergun | 0.2 | 0.5 | 0.7 | 0.2 + 6 × 0.5 = 3.2 |
| Rocket Launcher | 0.12 | 0.8 | 0.92 | 0.12 + 4 × 0.8 = 3.32 |
| Shotgun, Soldier | 0.4667 | 0.5 | 0.967 (wiki 1.005) | 3.467 |
| Shotgun, Pyro | 0.5 | 0.5 | 1.0 (wiki 1.035) | 3.5 |
| Shotgun, Heavy | 0.3333 | 0.5 | 0.833 (wiki 0.87) | 3.333 |
| Shotgun, Engineer | 0.3333 | 0.5 | 0.833 (wiki 0.87) | 3.333 |

The Shotgun's start equals the length of each class's reload-start
sequence (§8); the wiki's first-reload measurements are those plus
≈ 0.035 s (2–3 ticks of measurement latency), and its 0.51 s per shell is
0.5 plus one tick. The Scattergun's 0.2 likewise equals its start
sequence. The Rocket Launcher's start of 0.12 is only the difference of
the wiki's two measurements; its start sequence is 0.5 s (Open
questions 4).

The Minigun has no clip and never reloads.

### 5. Rocket Launcher: rocket, splash, self-damage, knockback

#### 5.1 Firing and flight

- Fire interval 0.8 s; one rocket per shot from the clip of 4.
- The rocket appears at the eye + 23.5 forward + 12 right − 3 up
  (crouched: + 8 up), i.e. under the weapon on the right; the community
  "Original" variant uses 0 right. (Open questions 5: whether the flight
  direction is the view direction or turned toward the crosshair's aim
  point.)
- It flies in a straight line at 1100 units/s, no gravity, no drag, until
  it touches the world or an enemy player or building. It passes through
  teammates and through its owner.
- On touch it explodes at the contact point.

#### 5.2 Explosion damage

- Every enemy within 146 units of the explosion centre with a clear line
  from the centre takes damage; so does the owner (§5.3). The player the
  rocket touched directly takes full splash (factor 1).
- Splash factor, linear from 1 at the centre to 0.5 at the edge, with r
  the distance from the explosion centre to the victim's centre:

```math
f(r) = 1 - 0.5\,\frac{r}{146}
```

- Damage to an enemy: 90 × M(attacker→victim distance, k = 0.5) × f(r);
  crit: 270 × f(r).
- The wiki also lists "1 % per 2.88 units" (≈ 144 units for 50 %); see
  Open questions 8.

#### 5.3 Self-damage

- The Soldier's own blast uses **no distance modifier** (factor 1) and
  the same splash falloff: 90 × f(r), never crit-boosted.
- If the Soldier is **airborne** when the blast hits him, it is
  × 0.60 (`tf_damagescale_self_soldier`), even if the rocket also hit an
  enemy.
- Wiki ranges, consistent with this: self-damage 27–89 (89 at r ≈ 3;
  27 = 90 × 0.5 × 0.6 at the edge while airborne); "rocket jump" 27–46 (a
  rocket at the feet of a standing Soldier, whose centre is ≈ 41–43 units
  up: 90 × (1 − 0.5 × 43/146) × 0.6 ≈ 46).

#### 5.4 Knockback and blast jumping

- Any damage pushes the victim away from its source (the explosion
  centre for splash) with a velocity change proportional to the damage;
  the Heavy takes half; crouched players are pushed farther (smaller
  hull); crits add no extra knockback to their own owner.
- Scales: enemies' blasts and bullets `tf_damageforcescale_other` 6;
  the Soldier's own rocket `tf_damageforcescale_self_soldier_rj` 10 when
  he is airborne (a real rocket jump) and
  `tf_damageforcescale_self_soldier_badrj` 5 when he is on the ground.
  The velocity per damage point and scale is not publicly documented:
  Open questions 9 (probe P3).
- **Blast jumping**: a player launched off the ground by his own
  explosion is in a "blast jumping" state until he lands; while airborne
  his own rockets hurt him × 0.6 (§5.3). Crouch-jumping before the blast
  gives a markedly longer jump.

### 6. Bullets and spread (Scattergun, Shotgun, Minigun)

#### 6.1 The shot

- Every pellet is a trace from the eye along its own direction, up to
  ≈ 56 754 units, against players' hitboxes (bullet hitboxes, not the
  bounding box) and the world, with lag compensation. No penetration:
  the first player or solid stops it.
- **First pellet**: if the weapon has not fired in the last 0.25 s, the
  first pellet of the shot flies exactly along the crosshair; the rest
  scatter. (For the Minigun this concerns only the first shot after a
  pause.)
- **Random spread** (`tf_use_fixed_weaponspreads` 0): every other pellet
  gets direction forward + a·s·right + b·s·up, a and b random
  (Open questions 1). s = 0.0675 (Scattergun, Shotgun: widest ≈ 3.9°),
  0.08 (Minigun: ≈ 4.6°). The wiki writes these as "30:1" and "25:1",
  which are twice 1/s.

#### 6.2 Fixed spread (`tf_use_fixed_weaponspreads` 1)

- Multi-pellet shots (Scattergun, Shotgun) put their pellets on a 3 × 3
  grid instead: one pellet on each grid point, extra pellets on the
  centre point, so the 10-pellet weapons put 2 pellets in the centre and
  one on each of the other 8 points. The pattern is the same every shot.
- Grid points at (a, b) ∈ {−1, 0, 1}², scaled by 0.5 × s: corners at
  0.5·s on both axes (community analysis; Open questions 10).
- Whether the Minigun's 4 bullets also use the grid: Open questions 10.

### 7. Minigun

#### 7.1 States

| State | Enters when | Lasts | Movement | Sounds |
|---|---|---|---|---|
| idle | – | – | 230 | – |
| winding up | attack or attack2 pressed | 0.75 s | 110 | `Weapon_Minigun.WindUp` |
| spun (not firing) | wind-up done, attack2 held | while held | 110 | `Weapon_Minigun.Spin` loop |
| firing | spun and attack held with ammo | while held | 110 | `Weapon_Minigun.Fire` loop (`…FireCrit` during a crit burst) |
| dry | spun, attack held, ammo 0 | while held | 110 | `Weapon_Minigun.ClipEmpty` loop |
| winding down | both buttons released | 1.15 s (1.4 s after an attack2-only spin) | 110 | `Weapon_Minigun.WindDown` |

- Firing never starts before the wind-up has finished; holding attack
  from idle fires its first shot 0.75 s after the press.
- Switching between attack and attack2 while spun goes straight between
  spun and firing, no wind-up.
- While in any state but idle the Heavy **cannot jump** and **cannot
  switch weapons**; crouched, he can't move at all (speed 0). A wind-up
  begun in the air keeps the air speed until landing.
- Spinning without firing uses no ammo.

#### 7.2 Firing

- A shot every 0.105 s while firing: 4 bullets, 1 ammo. 200 ammo last
  21 s of continuous fire (200 shots, 800 bullets).
- Damage per bullet 9 × M (k = 1), spread s = 0.08.
- **Rev-up penalty** (since 2017-10-20): during the first 1.0 s of being
  spun up (counted from the end of the wind-up whether or not firing; it
  resets only when the gun winds down), damage is scaled by

```math
g(\tau) = 0.5 + 0.5\,\min(\tau / 1.0,\ 1)
```

  with τ the spun-up time, and the cone of fire tightens over the same
  second from wider to s (how much wider: Open questions 11). Before
  2017-10-20 (from 2014-06-18), the second counted from the start of
  firing.
- Crits: rapid-fire rules (§2.6). Whether a crit burst also ignores g:
  Open questions 11.

### 8. Assets (from the install)

#### 8.1 Models

All four use the `c_` model scheme: the item schema's prefab gives
`model_player` and `attach_to_hands 1`; in first person the class's arms
model is shown and the `c_` weapon model is bone-merged onto it (bones
`weapon_bone`, …); in the world view the same `c_` model is bone-merged
onto the player model. The prefab's `static_attrs` `min_viewmodel_offset`
is the view offset used with minimal view models.

| Weapon | Prefab | `model_player` | `min_viewmodel_offset` |
|---|---|---|---|
| Scattergun | `weapon_scattergun` | `models/weapons/c_models/c_scattergun.mdl` | 10 0 −11 |
| Rocket Launcher | `weapon_rocketlauncher` | `models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl` | 10 −3 −10 |
| Shotgun | `weapon_shotgun` | `models/weapons/c_models/c_shotgun/c_shotgun.mdl` | 10 0 −9 |
| Minigun | `weapon_minigun` | `models/weapons/c_models/c_minigun/c_minigun.mdl` | 10 0 −9 |

Arms models: `models/weapons/c_models/c_scout_arms.mdl`,
`c_soldier_arms.mdl`, `c_pyro_arms.mdl`, `c_heavy_arms.mdl`,
`c_engineer_arms.mdl` (same folder); each pulls its sequences from
`c_<class>_animations.mdl` beside it.

Attachments on the weapon models (all on `weapon_bone`; position in the
bone's frame, units):

| Model | Muzzle | Others |
|---|---|---|
| `c_scattergun.mdl` | `muzzle` (0, 3.97, 23.38) | `eject_brass` (−3.05, 4.57, 8.63) |
| `c_rocketlauncher.mdl` | `muzzle` (0, 6.69, 9.91) | `backblast` (0, 6.69, −35.43) |
| `c_shotgun.mdl` | `muzzle` (0, 3.73, 34.92) | `eject_brass` (−0.8, 3.25, 10.76) |
| `c_minigun.mdl` | `muzzle` (0, −7.76, 57.76) | `eject_brass` (−8.57, −10.99, 5.85) |

(Each also has `pedestal_0`, `unusual_0`…`unusual_5` and `icon_camera`,
for cosmetics and icons.) `c_minigun.mdl` has its own barrel sequences
for the world view: `idle`, `spool_up` (`ACT_MP_ATTACK_STAND_PREFIRE`),
`fire_loop` (`ACT_MP_ATTACK_STAND_PRIMARY`, looping), `spool_down`
(`ACT_MP_ATTACK_STAND_POSTFIRE`), each 30 frames at 30 fps (0.9667 s).

Rocket projectile: `models/weapons/w_models/w_rocket.mdl`, one bone,
attachment `trail` (0, 0, −9.64) for the smoke trail; the model's bounds
±18.57 × ±3.71 × ±3.72 (visual; its collision size: Open questions 12).

#### 8.2 First-person sequences (from the class animation models)

Durations are (frames − 1) / fps; "loop" marks looping sequences.

| Class / weapon | Draw | Idle | Fire | Reload start | Reload loop | Reload end | Other |
|---|---|---|---|---|---|---|---|
| Scout / Scattergun | `sg_draw` 1.0 | `sg_idle` 1.8667 loop | `sg_fire` 0.6 | `sg_reload_start` 0.2 | `sg_reload_loop` 0.5 | `sg_reload_end` 0.5769 | – |
| Soldier / Rocket Launcher | `dh_draw` 0.8 | `dh_idle` 1.3333 loop | `dh_fire` 1.0 | `dh_reload_start` 0.5 | `dh_reload_loop` 0.8 | `dh_reload_finish` 0.7667 | – |
| Soldier / Shotgun | `draw` 0.8333 | `idle` 1.25 loop | `fire` 0.8333 | `reload_start` 0.4667 | `reload_loop` 0.5 loop | `reload_end` 0.7 | – |
| Pyro / Shotgun | `draw` 1.3333 | `idle` 1.8333 loop | `fire` 0.8333 | `reload_start` 0.5 | `reload_loop` 0.5 loop | `reload_end` 0.9667 | – |
| Heavy / Shotgun | `draw` 1.0 | `idle` 2.0 loop | `fire` 0.8333 | `reload_start` 0.3333 | `reload_loop` 0.5 loop | `reload_end` 0.7429 | – |
| Engineer / Shotgun | `fj_draw` 1.3667 | `fj_idle` 2.1667 loop | `fj_fire` 0.8333 | `fj_reload_start` 0.3333 | `fj_reload_loop` 0.5 loop | `fj_reload_end` 1.0 | `fj_fire_alt` 0.8333 |
| Heavy / Minigun | `m_draw` 1.0 | `m_idle` 1.6667 loop | `m_fire` 0.5667 loop | – | – | – | `m_spool_up` 1.0, `m_spool_idle` 0.3 loop, `m_spool_down` 1.3333 |

Activities: primary weapons use `ACT_PRIMARY_VM_DRAW`, `…_IDLE`,
`…_PRIMARYATTACK`, `ACT_PRIMARY_RELOAD_START`, `ACT_PRIMARY_VM_RELOAD`,
`ACT_PRIMARY_RELOAD_FINISH`; the secondary Shotgun the same with
`SECONDARY`; the Minigun's spin `ACT_PRIMARY_ATTACK_STAND_PREFIRE`
(`m_spool_up`), `ACT_PRIMARY_VM_SECONDARYATTACK` (`m_spool_idle`),
`ACT_PRIMARY_ATTACK_STAND_POSTFIRE` (`m_spool_down`). The Engineer's
Shotgun shares the Frontier Justice's sequences (`fj_`). The sequence
lengths are animation only: the deploy is 0.5 s whatever the draw length,
the Minigun's spin-up 0.75 s against a 1.0 s `m_spool_up`.

So the four Shotguns differ only in arms, animations, the draw sound
(§9) and the reload start time (§4); damage, spread, interval, clip and
reserve are the same.

#### 8.3 Sounds (`scripts/game_sounds_weapons.txt`)

| Use | Scattergun | Rocket Launcher | Shotgun | Minigun |
|---|---|---|---|---|
| shot | `Weapon_Scatter_Gun.Single` → `weapons/scatter_gun_shoot.wav` | `Weapon_RPG.Single` → `weapons/rocket_shoot.wav` | `Weapon_Shotgun.Single` → `weapons/shotgun_shoot.wav` | `Weapon_Minigun.Fire` → `weapons/minigun_shoot.wav` (loop) |
| crit shot | `…SingleCrit` → `scatter_gun_shoot_crit.wav` | `Weapon_RPG.SingleCrit` → `rocket_shoot_crit.wav` | `…SingleCrit` → `shotgun_shoot_crit.wav` | `Weapon_Minigun.FireCrit` → `minigun_shoot_crit.wav` |
| empty | `Weapon_Scatter_Gun.Empty` → `weapons/shotgun_empty.wav` | – | `Weapon_Shotgun.Empty` → `weapons/shotgun_empty.wav` | `Weapon_Minigun.ClipEmpty` → `weapons/minigun_empty.wav` |
| reload (per round) | `Weapon_Scatter_Gun.Reload` → `weapons/scatter_gun_reload.wav` | `Weapon_RPG.Reload` → `weapons/rocket_reload.wav` | `Weapon_Shotgun.Reload` → `weapons/shotgun_reload.wav` | – |
| world reload | `…WorldReload` → `scatter_gun_worldreload.wav` | `Weapon_RPG.WorldReload` → `rocket_reload.wav` | `…WorldReload` → `shotgun_worldreload.wav` | – |
| pump / cock | `Weapon_Scatter_Gun.Pump` → `scatter_gun_worldreload.wav` | – | `Weapon_Shotgun.Pump` → `shotgun_cock.wav`; Engineer: `Weapon_ShotgunEngineer.Cock_Back` / `.Cock_Forward` → `shotgun_cock_back.wav` / `shotgun_cock_forward.wav` | – |
| draw | `Weapon_Scatter_Gun.Draw` → `weapons/draw_secondary.wav` | `Weapon_RPG.Draw` → `weapons/draw_primary.wav` | `Weapon_Shotgun.Draw` → `draw_secondary.wav`; Pyro `Weapon_ShotgunPyro.Draw` → `draw_shotgun_pyro.wav` | `Weapon_Minigun.Draw` → `weapons/draw_minigun_heavy.wav` |
| spin | – | – | – | `Weapon_Minigun.WindUp` → `minigun_wind_up.wav`; `Weapon_Minigun.Spin` → `minigun_spin.wav`; `Weapon_Minigun.WindDown` → `minigun_wind_down.wav` |
| explosion | – | `BaseExplosionEffect.Sound` → random of `weapons/explode1.wav`, `explode2.wav`, `explode3.wav` | – | – |

Shots are on the weapon channel at 94 dB; the minigun loops on the static
channel. The pairing of entries to weapons is by name (the weapon scripts
that name them are encrypted): Open questions 13. No flight-loop entry
exists for the stock rocket (it flies silently; its trail is a particle
effect).

#### 8.4 Kill icons (`scripts/mod_textures.txt`, `HUD/d_images`)

`scattergun`; `tf_projectile_rocket` (rocket kills name the projectile);
`shotgun_primary`, `shotgun_soldier`, `shotgun_pyro`, `shotgun_hwg` (all
the same image cell); `minigun`. The stock items have no `item_iconname`
in the schema.

## Per-tick order

Hitscan pellet weapon (Scattergun, Shotgun; the Minigun from "firing"):

1. Read buttons. If reloading and attack is pressed with clip ≥ 1, end the
   reload (the insert in progress is lost).
2. If attack is held, the fire interval since the last shot has passed and
   clip ≥ 1 (Minigun: spun and ammo ≥ 1): decide crit (single-shot: roll
   now with p of §2.6; Minigun: inside a burst, or roll if ≥ 1 s since the
   last roll).
3. Remove 1 ammo; next fire time = last fire time + interval; play the
   fire sequence and the (crit) shot sound.
4. Note whether the last shot was more than 0.25 s ago (dead-centre first
   pellet), then record now as the last shot time.
5. Rewind other players (lag compensation).
6. For each pellet: its direction (§6), trace, record the hit victim.
7. Per victim: sum the pellets' base damage, apply the distance modifier
   (or ×3 for a crit; Minigun rev-up factor g), round, apply as one damage
   event with its knockback; add the dealt damage to the attacker's 20 s
   crit history.
8. Restore players. If clip is now 0 and reserve > 0, the reload starts
   once the interval has passed.

Rocket Launcher:

1–4 as above (the rocket carries the crit decision).
5. Spawn the rocket at the eye offset (§5.1), velocity 1100 along its
   direction.
6. Each later tick: move the rocket; on contact with the world or an
   enemy, explode at the contact point: collect players and buildings
   within 146 units with a clear line from the centre; for each, damage
   (§5.2, the touched player f = 1; the owner §5.3), knockback (§5.4).

## Edge cases

- **Distance for a rocket** is taken when it explodes: a Soldier who fires
  from 1500 units and walks to 300 units before impact deals the 300-unit
  damage.
- **Minigun out of ammo**: it keeps spinning with the empty loop while
  attack is held; no shots.
- **Crit burst across a weapon switch**: the Minigun can't be switched
  while spinning, so a burst ends unused when the spin ends.
- **Firing while the clip is empty during a reload**: ignored; the reload
  continues.
- **Rocket at the owner's feet in the air**: f uses the distance to his
  centre, × 0.6; with crouch the centre is lower, so r and the damage
  change (Open questions 9).
- **Spread on, distance near 0 or past 1024**: t is clamped to [0, 1], so
  random spread can't push the modifier above 1.5 or below 0.5 (before
  the per-weapon k).

## Quirks

- The 52.8 % "maximum fall-off" with spread off (not 50 %) comes from the
  921.6-unit clamp. Keep it.
- The Minigun wind-up animation (1.0 s) is longer than the wind-up (0.75 s);
  the first shot comes before the animation ends. Keep the timing, not the
  animation length.
- Rockets fly from the right of the view, so a rocket aimed close past a
  corner on the right hits the wall. Keep.

## Test cases

Spread off (`tf_damage_disablespread` 1), no crits, no random spread of
pellets unless said, damage rounded at the end.

| # | Setup | Input | Expected |
|---|---|---|---|
| 1 | Scattergun, victim at 0 units | all 10 pellets hit | 10 × 6 × 1.75 = 105 |
| 2 | Scattergun, 512 | 10 hit | 60 |
| 3 | Scattergun, 1024 | 10 hit | 10 × 6 × 0.528 = 31.68 → 32 |
| 4 | Scattergun, 256 | 10 hit | 60 × 1.515625 = 90.94 → 91 |
| 5 | Scattergun, 768 | 10 hit | 60 × 0.65625 = 39.38 → 39 |
| 6 | Scattergun, 2000 | 10 hit | 32 (clamped at 921.6) |
| 7 | Scattergun, 0, crit | 10 hit | 180 |
| 8 | Scattergun, 1024, crit | 10 hit | 180 (no fall-off) |
| 9 | Scattergun, 1024, mini-crit (reference) | 10 hit | 81; at 0: 141.75 → 142 |
| 10 | Shotgun, 0 / 512 / 1024 | 10 hit | 90 / 60 / 32 |
| 11 | Shotgun, 256 / 768 | 10 hit | 80.63 → 81 / 39.38 → 39 |
| 12 | Minigun spun > 1 s, 0 / 512 / 1024 | one shot, 4 hit | 54 / 36 / 19.008 → 19 |
| 13 | Minigun, first shot at τ = 0 after wind-up, 512 | 4 hit | 36 × 0.5 = 18 |
| 14 | Minigun, τ = 0.5 s, 512 | 4 hit | 36 × 0.75 = 27 |
| 15 | Rocket direct hit, Soldier 0 / 512 / 1024 from victim at impact | – | 112.5 → 112 or 113 (Open questions 3) / 90 / 47.52 → 48 |
| 16 | Rocket, victim 256 away | direct | 90 × 1.171875 = 105.47 → 105 |
| 17 | Rocket, Soldier 512 away, explosion 73 units from victim's centre | splash | 90 × 0.75 = 67.5 → 68 (or 67) |
| 18 | Rocket, explosion 146 from victim's centre, Soldier 512 away | – | 45 |
| 19 | Rocket, explosion 147 from victim's centre | – | 0 (outside) |
| 20 | Rocket crit, explosion 73 from victim, Soldier 1500 away | – | 270 × 0.75 = 202.5 |
| 21 | Soldier on the ground, his rocket explodes 43 from his centre | – | 90 × (1 − 0.5·43/146) = 76.75 → 77 |
| 22 | Same, airborne | – | 77 × 0.6 ≈ 46 |
| 23 | Same, crit rocket | – | 77 (self never gets crit damage) |
| 24 | Rocket fired standing, eye at (0,0,64), view along +X | – | rocket spawns at (23.5, −12, 61) (right = −Y), moving at 1100 along +X (or toward the aim point, Open questions 5) |
| 25 | Rocket fired from 1100 units in front of a wall | – | explodes 1.0 s later (67 ticks) |
| 26 | Teammate between Soldier and wall | fire | the rocket passes through the teammate, no damage |
| 27 | Scattergun clip 0, reserve 32 | – | reload starts by itself; clip 1 at +0.7 s, 2 at +1.2, …, 6 at +3.2; reserve 26 |
| 28 | Scattergun clip 2, reloading, third shell 0.3 s in | press attack | the reload stops at once, a shot fires (clip 1); shell lost |
| 29 | Scattergun clip 0 reloading, first shell 0.1 s in | press attack | ignored; clip 1 at 0.7 s |
| 30 | Rocket Launcher clip 0, reserve 20 | – | clip 1 at +0.92, 4 at +3.32; reserve 16 |
| 31 | Shotgun as Soldier / Pyro / Heavy / Engineer, clip 0 | – | first shell at 0.967 / 1.0 / 0.8333 / 0.8333 s, then every 0.5 s |
| 32 | Any of the four | switch to it | first shot possible 0.5 s later (34 ticks) |
| 33 | Minigun idle | press and hold attack | first shot at +0.75 s (50 ticks), then every 0.105 s (7 ticks; 0.105/0.015 = 7) |
| 34 | Minigun, 200 ammo | hold attack 30 s | 200 shots, then the empty loop; spun the whole time |
| 35 | Minigun spinning (attack2) | press jump | no jump; release attack2 → wind-down 1.4 s, jump allowed after it |
| 36 | Minigun firing | press weapon switch | refused |
| 37 | Heavy, minigun spun, crouched | hold forward | speed 0; standing: 110 |
| 38 | Scattergun, last shot ≥ 0.25 s ago | fire with random spread | pellet 1 exactly on the crosshair; others within ±0.0675 tangent on each axis |
| 39 | Scattergun, `tf_use_fixed_weaponspreads` 1 | fire twice | identical pattern: 2 pellets at centre, 8 at the 3×3 grid's other points |
| 40 | Crit chance, 0 / 200 / 800 / 2000 damage in last 20 s | – | 2 % / 3.5 % / 8 % / 8 % |
| 41 | Minigun, crit roll succeeds at t | keep firing | every shot from t to t + 2 s is a crit (27 per bullet, no distance modifier), the crit fire loop plays |
| 42 | `tf_damage_disablespread` 0, Scattergun at 512, 1000 shots | – | damages spread over ≈ 60 × [0.85, 1.15] (t in [0.4, 0.6]: M₀ from 1.148 to 0.852; with k = 1.5 near: up to 1.222) |
| 43 | Heavy hit by a rocket | – | half the knockback a Soldier would get from the same damage |

Probe checks (live dedicated server, bots; never port 27015/27030):

| # | How | Expected |
|---|---|---|
| P1 | Scattergun on a bot at measured 0/256/512/768/1024/1500, spread off, crits off | tests 1–6 (rounding of .5 and the curve's middle points) |
| P2 | Rocket on a bot standing still at known offsets from the impact point | f linear to 0.5 at 146 (or 144) |
| P3 | Soldier's own rocket at his feet: standing, crouched, airborne; read velocity right after | velocity per damage point for scales 10 / 5 and the crouch factor (Open questions 9) |
| P4 | Log first-shell times of each reload | §4 table |
| P5 | Minigun: ticks from press to first shot; damage of shots over the first second | 50 ticks; g(τ) |
| P6 | Spawn a rocket and log its first position | the offset of §5.1 and its direction |

## Open questions

1. **Random distributions.** The distribution of the distance jitter j
   (uniform?) and of the pellet offsets a, b (uniform each in [−1, 1], or
   the sum of two halves as in Source's other games) are not documented.
   Tests check ranges and seeding, not exact values.
2. **Distance points.** "Distance between attacker and target" is assumed
   centre to centre (world-space bounding-box centres). Could be the
   attacker's eye or origin; P1 at short range tells.
3. **Rounding.** The wiki's tables show 112 for 112.5 (rocket ramp-up)
   but 11 for 10.5 (one Scattergun pellet) and 151 for 1.35 × 112: either
   truncation in some places or table inconsistencies. Assume round to
   nearest, halves up, of the summed damage; P1/P2 settle it.
4. **Rocket Launcher reload start.** The wiki's 0.92 / 0.8 imply a 0.12 s
   start; its start sequence is 0.5 s. Either the measurement started
   later (e.g. from the end of the 0.8 s fire interval) or the rocket's
   start isn't its animation's. P4.
5. **Rocket direction.** From the side offset, does the rocket fly along
   the view (and so land 12 units right of the crosshair at every range)
   or is it turned toward the point the crosshair's trace hits (meeting
   the crosshair there)? Community reports suggest the latter; P6.
6. **Crit bucket.** The `tf_weapon_criticals_bucket_*` cvars (300, 1000,
   −250) show a damage bucket limiting crits; its rules (what fills it,
   what a crit costs, what an unaffordable crit does) are not in public
   docs. Implement the chance formula only, until measured.
   Also the per-second roll chance of rapid-fire weapons (is it the same
   p, or a value giving ~p of shots?) is not documented.
7. **`cl_autoreload` default** is a client setting not in the server
   install; check on the Windows PC's client (`help cl_autoreload`).
8. **Splash radius**: 146 with linear fall-off to 50 % (wiki: "50 % at
   2.8 m"), but the wiki also lists "1 % per 2.88 units" (144 units for
   50 %). Possibly the falloff is measured to the nearest point of the
   victim's box, not its centre (which would also change §5.3's worked
   numbers). P2.
9. **Knockback.** The scales are known (6, 10, 5, Heavy × 0.5), not the
   base velocity per damage point, the effect of the hull (crouch), the
   direction (centre to centre or with an upward bias) or a speed cap.
   P3 must measure these before rocket jumping can be matched.
10. **Fixed spread grid.** The 0.5 scale of the grid comes from a wiki talk
    page (community analysis), and the pellet order on it isn't given.
    Whether the Minigun (4 bullets) uses it is unknown.
11. **Minigun rev-up accuracy**: how much wider the cone is at τ = 0 is
    not documented (only that it tightens over the second); whether a
    crit burst ignores the damage factor g is not documented (crits
    ignore distance, so likely ignore g too). Measure on the probe.
12. **Rocket collision size** against the world and players is not
    documented (the projectile "hitbox" used against players is a large
    one shared by all classes, smaller crouched). Assume a point (zero-size
    trace) against the world and players' bounding boxes.
13. **Sound pairing** is by entry name; check on the live game that the
    stock items play exactly these entries, and which reload entry the
    first-person animations trigger (by animation events) vs the server.
14. **Wiki versions.** The numbers are the wiki's as of 2026 and its
    patch history: rocket reserve 20 (since 2009-02-02), minigun spin and
    speed (2010-04-28), rev-up penalty (2014-06-18, reworked 2017-10-20),
    damage spread off by default (2015-07-02), deploy 0.5 s
    (2015-12-17). The wiki says all its timings are community
    measurements; the Shotgun page's "every class reloads at the same
    speed" (2010-09-30) contradicts its per-class first-reload table
    (explained in §4 as start-sequence lengths).
