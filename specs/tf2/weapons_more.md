# Team Fortress 2: weapons, part 2 — Pistol, SMG, Sniper Rifle, Revolver, Grenade Launcher, Stickybomb Launcher, Flame Thrower, Syringe Gun (T-W2)

Source basis: public community documentation (the community TF2 wiki's
pages on each of these weapons and its general pages on damage, critical
hits, headshots, projectiles and afterburn, with their patch histories),
and the retail client install read in place, nothing decrypted: the item
schema `scripts/items/items_game.txt`, the plain sound scripts
(`scripts/game_sounds_weapons.txt`, `scripts/game_sounds_player.txt`) and
kill-icon table (`scripts/mod_textures.txt`) inside the misc VPK, and the
sequences, bones and attachments of the class arms' animation models, the
`c_` weapon models and the projectile models, as printed by our dumper.
The per-weapon scripts are encrypted and were not read. Timing values on
the wiki are community measurements; where this spec derives a number it
says so.
Status: draft

## Summary

This continues [weapons.md](weapons.md) (T-W1) and uses its shared rules
unchanged: the distance modifier (weapons.md §2.1–2.3), pellet summing and
rounding (§2.4), crits and mini-crits (§2.5), random-crit chance and the
per-shot / rapid-fire split (§2.6), no headshots and no friendly fire
(§2.7, with the Sniper Rifle as the exception here), switching (§3, every
weapon here deploys in 0.5 s), one-round-at-a-time reloading (§4),
explosions, self-damage and knockback (§5.2–5.4) and the spread convention
(§6.1). This file only gives what differs.

The three small hitscan guns (Pistol, SMG, Revolver) are single-bullet
weapons whose first shot after a 1.25 s pause is dead accurate. The Sniper
Rifle has no distance modifier; its damage grows with the time spent
scoped (50 to 150), and a scoped head hit is a ×3 crit, its only crit. The
Demoman's grenades fly in an arc, explode on a direct hit before they touch
the world, and otherwise roll and burst on a 2.3 s fuse with no distance
modifier; his stickybombs are launched faster the longer attack is held,
stick to the world, arm after 0.7 s and burst together on attack2, up to
8 at a time. The Flame Thrower is a short cone of drifting flame
particles whose damage depends on the particles' age and on how long the
victim has been burning, and sets victims on fire (afterburn). The Syringe
Gun fires a stream of slow needles that drop with gravity.

## Units and conventions

As weapons.md: Hammer units, Z up, X forward, Y left, degrees; seconds;
server tick 0.015 s; timers are absolute times and an action due at T
happens on the first tick with time ≥ T. "Wiki" is the community TF2 wiki
as of 2024–2026. Spread s is weapons.md §6.1's; the wiki writes a gun's
spread as "N:1", which is s = 2 / N (weapons.md: 30:1 ≈ 0.0675).

Distance modifier: M(d, k) is weapons.md §2.1's modifier with near-range
scale k, R = 0.5, and (spread off, the default) t = min(d, 921.6) / 1024.
So M(0) = 1 + 0.5 k, M(512) = 1, M(≥ 921.6) = 0.528.

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| deploy | 0.5 | s | every weapon here (weapons.md §3) |
| accurate_pause | 1.25 | s | Pistol, SMG, Revolver: first shot after this long is dead centre (wiki "spread recovery") |
| ramp_scale_syringe_sticky | 0.4 | – | near bonus ×0.4: 120 % at 0 units (Syringe Gun, Stickybomb Launcher) |
| pistol_damage / interval / spread | 15 / 0.15 / 0.04 | hp, s, – | |
| smg_damage / interval / spread | 8 / 0.105 / 0.025 | hp, s, – | |
| revolver_damage / interval / spread | 40 / 0.5 / 0.025 | hp, s, – | |
| sniper_base | 50 | hp | any body shot, uncharged or unscoped |
| sniper_charge_max | 150 | hp | charge damage at full charge |
| sniper_charge_rate | 50 | hp/s | charge growth while scoped |
| sniper_charge_delay | 0.3 | s | scoped time before charging starts |
| sniper_headshot_delay | 0.2 | s | scoped time before a head hit counts |
| sniper_interval | 1.5 | s | between shots |
| sniper_zoom_fov | 20 | deg | scoped field of view (Open questions 3) |
| sniper_zoom_speed | 80 | units/s | movement cap while scoped (movement.md, measured) |
| grenade_damage | 100 | hp | at the centre; no distance modifier |
| grenade_speed | 1200 forward + 200 up | units/s | launch velocity (|v| = 1216.6; derived, §6.2) |
| grenade_fuse | 2.3 | s | from launch |
| blast_radius | 146 | units | grenade and stickybomb (weapons.md §5.2's falloff, 0.5 at the edge) |
| demo_self_air_scale | 0.75 | – | Demoman's own blast while airborne (derived, Open questions 9) |
| sticky_damage | 120 | hp | at the centre, k = 0.4 within 5 s of launch |
| sticky_flat_after | 5.0 | s | stickies older than this use modifier 1 |
| sticky_arm | 0.7 | s | from launch until it can be detonated |
| sticky_charge_time | 4.0 | s | hold time for the fastest launch; fires by itself then |
| sticky_speed_min / max | 900 / 2400 forward (+ ≈ 200 up) | units/s | launch speed uncharged / fully charged (Open questions 7) |
| sticky_max | 8 | – | live stickies per Demoman |
| sticky_air_radius_start | 0.85 | – | radius factor of an un-stuck sticky at arm time, to 1 over 2 s |
| flame_particle_damage | 13 | hp | one flame tick on one victim, fresh particle, full ramp |
| flame_tick | 0.075 | s | flame damage at most once per this per victim (5 ticks) |
| flame_ammo_rate | 12.5 | ammo/s | while firing |
| flame_speed / drag / up / spread / life | 2450 / 8.5 / 50 / 2.8 / 0.6 (+0–0.1) | units/s, 1/s, units/s, deg, s | flame particle (item schema `static_attrs`) |
| flame_age_floor | 0.5 at 0.5 s | – | damage factor of a particle by age |
| flame_ramp | 0.5 → 1 over 1.5 s | – | damage factor by time a victim has been hit continuously |
| airblast_cost / refire | 20 / 0.75 | ammo, s | (reference) |
| afterburn_damage / interval | 4 / 0.5 | hp, s | 8 hp/s |
| afterburn_min / max | 3 / 10 | s | duration from the Flame Thrower |
| syringe_damage | 10 | hp | per needle, k = 0.4 |
| syringe_interval | 0.105 | s | |
| syringe_speed | 990 (wiki also 1000) | units/s | |

## Behavior

### 1. The eight weapons at a glance

| | Pistol | SMG | Sniper Rifle | Revolver |
|---|---|---|---|---|
| Item defindex, `name` | 22 `TF_WEAPON_PISTOL` (Engineer), 23 `TF_WEAPON_PISTOL_SCOUT` (Scout) | 16 `TF_WEAPON_SMG` | 14 `TF_WEAPON_SNIPERRIFLE` | 24 `TF_WEAPON_REVOLVER` |
| `item_class` | `tf_weapon_pistol` | `tf_weapon_smg` | `tf_weapon_sniperrifle` | `tf_weapon_revolver` |
| Class, slot | Scout and Engineer, secondary | Sniper, secondary | Sniper, primary | Spy, secondary (his first weapon) |
| Kind | hitscan, 1 bullet | hitscan, 1 bullet | hitscan, 1 bullet, charge | hitscan, 1 bullet |
| Base damage | 15 | 8 | 50–150 by charge | 40 |
| Near ramp at 0 (k) | 150 % (1) | 150 % (1) | none (modifier 1) | 150 % (1) |
| Far fall-off | 52.8 % | 52.8 % | none | 52.8 % |
| Fire interval | 0.15 s | 0.105 s | 1.5 s | 0.5 s |
| Spread s (wiki N:1) | 0.04 (50:1) | 0.025 (80:1) | 0 (Open questions 4) | 0.025 (80:1) |
| Clip / reserve | 12 / 36 Scout, 12 / 200 Engineer | 25 / 75 | 25, no clip | 6 / 24 |
| Reload | whole clip: 1.005 s Scout, 1.035 s Engineer | whole clip 1.1 s | none | whole clip 1.133 s |
| Crit rolls | rapid-fire (once a second, 2 s bursts) | rapid-fire (Open questions 5) | never random; scoped headshot = crit | every shot |

| | Grenade Launcher | Stickybomb Launcher | Flame Thrower | Syringe Gun |
|---|---|---|---|---|
| Item defindex, `name` | 19 `TF_WEAPON_GRENADELAUNCHER` | 20 `TF_WEAPON_PIPEBOMBLAUNCHER` | 21 `TF_WEAPON_FLAMETHROWER` | 17 `TF_WEAPON_SYRINGEGUN_MEDIC` |
| `item_class` | `tf_weapon_grenadelauncher` | `tf_weapon_pipebomblauncher` | `tf_weapon_flamethrower` | `tf_weapon_syringegun_medic` |
| Class, slot | Demoman, primary (`anim_slot` secondary) | Demoman, secondary (`anim_slot` primary) | Pyro, primary | Medic, primary |
| Kind | arcing grenade, splash | charged sticky, splash on command | flame particles | arcing needle projectile |
| Base damage | 100 | 120 | 13 per flame tick (6.5–13) | 10 |
| Near ramp at 0 (k) | none (modifier 1) | 120 % (0.4) within 5 s | none; age and burn ramp instead | 120 % (0.4) |
| Far fall-off | none | 52.8 % within 5 s | none | 52.8 % |
| Fire interval | 0.6 s | 0.6 s (after release) | damage tick 0.075 s | 0.105 s |
| Clip / reserve | 4 / 16 | 8 / 24 | 200, no clip | 40 / 150 |
| Reload | one at a time: first 1.24, then 0.6 | one at a time: first 1.09, then 0.67 | none | whole clip 1.305 s |
| Crit rolls | every shot | every shot | rapid-fire | rapid-fire |

The upgradeable copies (the 190–212 range) and the festive and botkiller
variants are the same weapons; this spec covers the
`baseitem` 1 items above.

Item schema notes:

- The Pistol prefab `weapon_pistol` lists both Scout and Engineer, but the
  two stock items override `used_by_classes`: 22 is the Engineer's, 23
  (`hidden` 1) the Scout's. Same gun, same model, different reserve and
  reload time.
- The Sniper Rifle prefab carries the tag `can_headshot` (the only weapon
  here that has it).
- The Flame Thrower resolves through prefabs `weapon_flamethrower` →
  `weapon_baseflamethrower` → `weapon_newflame`; the last holds the flame
  particle's numbers as `static_attrs`: `flame_speed` 2450,
  `flame_drag` 8.5, `flame_up_speed` 50, `flame_gravity` 0,
  `flame_spread_degree` 2.8, `flame_lifetime` 0.6,
  `flame_random_life_time_offset` 0.1. Item 21 also attaches
  `models/weapons/c_models/c_flamethrower/c_flamethrower_pilotlight.mdl`
  (`visuals` → `attached_models`).
- `anim_slot`: the Grenade Launcher animates as a secondary
  (`ACT_SECONDARY_*`: the Demoman's `g_*` sequences) and the Stickybomb
  Launcher as a primary (`ACT_PRIMARY_*`: `sb_*`), the reverse of their
  item slots.

### 2. Pistol, SMG, Revolver (single-bullet hitscan)

All three are weapons.md §6.1's bullet: one trace from the eye, hitboxes,
no penetration, lag compensation; the damage of the hit is
base × M(d, 1), rounded (weapons.md §2.4; Open questions on rounding are
weapons.md's 3).

- **First shot**: if the gun has not fired for 1.25 s, the shot goes
  exactly along the crosshair; otherwise its direction gets the random
  spread of weapons.md §6.1 with the s above. (This is weapons.md's
  `first_pellet_pause` with 1.25 s instead of 0.25 s.)
- **Automatic**: holding attack fires every interval (the Pistol fires at
  its fixed 0.15 s whether the button is held or tapped; patch of
  2009-08-13). The Revolver is semi-automatic in feel only through its
  0.5 s interval: holding attack refires every 0.5 s (Open questions 6).
- **Reload**: the whole clip at once: when it starts (weapons.md §4 step 1
  conditions: key press with clip < max and reserve > 0, or clip 0 after
  the interval), after the reload time the clip becomes
  min(max, clip + reserve) and the reserve drops by what was moved.
  Pressing attack during the reload does nothing (no round to fire mid
  reload unless clip ≥ 1: then, as weapons.md §4 step 3, attack interrupts
  it and nothing is moved; Open questions 6).
- **Crits**: Pistol and SMG are rapid-fire weapons (weapons.md §2.6: roll
  once a second while firing, 2 s of crits on success); the Revolver rolls
  every shot. Crit = 3 × base, no distance modifier.
- The Spy's Revolver is the only gun he has; while cloaked he can neither
  fire nor reload (cloak is a later spec).

### 3. Sniper Rifle

#### 3.1 Zoom

- attack2 toggles the scope. Scoped: field of view 20 (Open questions 3),
  the view model hidden, a scope overlay drawn, movement capped at
  80 units/s (movement.md, measured; the wiki's 27 % of 300), crouched
  slower (wiki: 1.33 % ≈ 4 units/s; Open questions 3).
- Unscoping (attack2 again, a weapon switch, death) drops the charge to 0.
- **After a scoped shot** the Sniper leaves the scope for the bolt cycle and
  comes back into it when the 1.5 s interval has passed, if the client
  option `cl_autorezoom` is on (its default: Open questions 3); the charge
  restarts from 0 either way.
- Others see a **laser dot** where the Sniper's aim trace (from his eye
  along his view) meets the world or a player, while he is scoped; it is
  not drawn in his own first-person view. Its look is a later spec.

#### 3.2 Charge

Let τ be the time since the scope came up (or since the last shot while
scoped). The charge:

```math
c(\tau) = \min\!\big(150,\ \max(0,\ 50\,(\tau - 0.3))\big)
```

reaches 50 at τ = 1.3 s and 150 at τ = 3.3 s (the wiki: "charging starts
0.3 s after zooming; damage starts rising after 1 s of charging (33 %);
full at 3 s, 3.3 s from zooming in"). The HUD meter shows c / 150. The
shot's base damage:

```math
D = \begin{cases} \max(50,\ c(\tau)) & \text{scoped} \\ 50 & \text{unscoped} \end{cases}
```

No distance modifier (factor 1 at every range), no random damage spread
(Open questions 4).

#### 3.3 Headshots

- A hit on the head hitbox (hitgroup 1, classes.md "Hitboxes") is a
  **headshot** only if the Sniper is scoped and has been for at least
  0.2 s. A headshot is a crit: damage 3 × D (150 at no charge, 450 at
  full charge, enough to kill any class at any overheal).
- Unscoped (or scoped less than 0.2 s) head hits are body hits: D.
- The Sniper Rifle never rolls random crits (wiki; its prefab has no crit
  tag). A crit boost from elsewhere makes every hit 3 × D (reference).
- Shots pass through teammates (wiki) and stop at the first enemy or solid.
- The kill feed uses the kill icon `headshot` for a headshot kill and
  `sniperrifle` otherwise (§10).

#### 3.4 Ammo

25 rounds, no clip, no reload: each shot takes one from the 25; the
bolt cycle is part of the 1.5 s interval (its fire animation, 1.6667 s, is
longer: Quirks).

### 4. Grenade Launcher

#### 4.1 Firing and flight

- Every 0.6 s while attack is held, one grenade from the clip of 4.
- The grenade starts from an offset under the gun (community figure
  forward 16, right 8, up −6 from the eye; Open questions 8) with
  velocity 1200 along the aim's forward plus 200 along the aim's up (so
  |v| = √(1200² + 200²) = 1216.6, the wiki's speed; the split is derived
  from that number and is an assumption: Open questions 7), plus a small
  random jitter (Open questions 7). It is a physical body: full gravity
  (sv_gravity 800), drag (Open questions 7), spinning; it bounces off the
  world.
- It passes through teammates and its owner (as the rocket, weapons.md
  §5.1).

#### 4.2 Direct hit, rollers, fuse

- **Direct hit**: if the grenade touches an enemy player or building
  **before it has touched the world**, it explodes at once at the contact
  point; the struck player takes full splash (factor 1).
- **Roller**: once it has touched the world, it never explodes on contact
  again; later contacts with enemies only make it bounce (it still
  collides with them).
- **Fuse**: a grenade that hasn't exploded explodes 2.3 s after launch,
  wherever it is.

#### 4.3 Explosion

- weapons.md §5.2's splash: radius 146, factor f(r) = 1 − 0.5 r / 146 to
  each enemy's centre with a clear line from the centre.
- Damage to an enemy: 100 × f(r). **No distance modifier**: M = 1 at every
  range (wiki "Damage": grenade launchers always use 1). Crit: 300 × f(r).
- The wiki notes a ±2 % damage variance on grenades since 2014-12-22;
  with `tf_damage_disablespread` 1 (the default), assume none (Open
  questions 10).
- Self-damage, knockback: §6.

### 5. Stickybomb Launcher

#### 5.1 Charging and launching

- Pressing attack (clip ≥ 1, interval passed) starts a **charge**; the
  charge sound `Weapon_StickyBombLauncher.ChargeUp` plays and the arms play
  `sb_autofire` (4.0 s). Releasing attack launches one sticky; holding for
  4.0 s launches it by itself at full charge.
- Launch speed by held time h (linear, wiki "about 2.3 to 2.6 × as fast
  fully charged"; numbers Open questions 7):

```math
v_{fwd}(h) = 900 + 1500\,\min(h / 4,\ 1)
```

  along the aim's forward, plus ≈ 200 up (as the grenade), from the same
  offset as the grenade.
- The next charge can start 0.6 s after the launch.
- Gravity and drag as the grenade's; passes through teammates and the owner.

#### 5.2 Sticking

- On touching the world (and world brushes or static props), the sticky
  stops dead and stays there.
- On touching an enemy player or building it **bounces off** without
  exploding; it also bounces off some map entities (spawn doors, payload
  carts: later).
- It never explodes on contact.

#### 5.3 Arming and detonation

- A sticky is **armed** 0.7 s after its launch (wiki's current value; Open
  questions 11).
- **attack2**, with the Stickybomb Launcher out, detonates every armed
  sticky of this Demoman at once (sound `Weapon_StickyBombLauncher.ModeSwitch`
  on the launcher; each sticky makes its explosion). Unarmed stickies
  stay. attack2 with no armed sticky does nothing. (Detonating with
  another weapon out: Open questions 12.)
- **Limit 8**: launching a ninth sticky detonates the oldest one shortly
  afterwards (delay: Open questions 12), armed or not.
- When the Demoman dies or changes class, all his stickies vanish without
  exploding.
- Enemy bullets (hitscan), syringes and melee hits destroy a sticky
  without an explosion (wiki; optional for a first port). Enemy explosions
  push stickies but don't destroy them.

#### 5.4 Explosion damage

- Splash as the grenade (radius 146, f(r) to 0.5 at the edge).
- Damage to an enemy, with a the sticky's age at detonation and d the
  Demoman-to-victim distance at detonation:

```math
D = 120 \times f(r) \times \begin{cases} M(d,\ 0.4) & a \le 5\ \text{s} \\ 1 & a > 5\ \text{s} \end{cases}
```

  (144 at 0 units, 120 at 512, 63.36 at ≥ 921.6, all at f = 1). Crit
  (decided at launch, carried by the sticky): 360 × f(r), no distance
  modifier.
- **Air-detonation radius** (since 2014-12-22): a sticky detonated while it
  is not stuck to anything uses a smaller radius:

```math
r_{max} = 146 \times \left(0.85 + 0.15\,\min\!\left(\frac{a - 0.7}{2},\ 1\right)\right)
```

  (124.1 right at arming, 146 from 2.7 s on). Stuck stickies always use 146.
- There is **no damage ramp-down for fresh stickies** today: a June 2014
  change that weakened fresh stickies was reverted; only the air radius
  above remains (Open questions 11).

### 6. Demoman self-damage and blast jumping

- The Demoman's own grenade or sticky hurts him with **no distance
  modifier**: base × f(r) (100 or 120), never crit-boosted (weapons.md
  §5.3), and × 0.75 if he is airborne. The 0.75 is derived from the wiki's
  self-damage ranges (stickies 45–114: 45 = 120 × 0.5 × 0.75; grenades
  38–73: 38 ≈ 100 × 0.5 × 0.75); Open questions 9.
- Knockback as weapons.md §5.4, with the Demoman's own blasts on himself
  at their own scale (the Soldier's `_soldier` cvars don't apply; Open
  questions 9). A **sticky jump**: stickies at the feet, jump (or crouch
  jump), detonate in the air; with several stickies each explosion adds
  its own damage and push in the same tick.

### 7. Flame Thrower

This is a reduction of the public description to what a simple
implementation can do.

#### 7.1 Firing

- While attack is held with ammo: the Pyro emits flame particles from the
  weapon's muzzle direction (the eye's aim) and uses 12.5 ammo per second
  (1 every 0.08 s; 200 ammo last 16 s). The wiki gives about 33 particles a
  second.
- Each particle: initial speed 2450 along the aim with a random cone of
  2.8° (half-angle; Open questions 13) plus the Pyro's own velocity
  (Open questions 13), drifting up at 50 units/s, no gravity, slowed by
  drag 8.5 per second (taken as exponential: v(t) = v₀ e^(−8.5 t)),
  living 0.6 s plus a random 0–0.1 s. It passes through players (it does
  not stop on hitting one) and stops at the world.
- Range: with that drag a particle travels 2450 / 8.5 × (1 − e^(−8.5 t)):
  286 units in 0.6 s, 288 in 0.7 s; from the muzzle (≈ 50 units ahead of
  the eye) this matches the wiki's "≈ 350 units when stationary".
  A simplest port may instead test a 350-unit cone of 2.8° half-angle
  every flame tick, with particle age taken as distance / (350 / 0.6).

#### 7.2 Damage

- Each enemy is damaged at most once per flame tick (0.075 s, 5 server
  ticks) while any particle overlaps him; the damage of that tick:

```math
D = 13 \times g_{age} \times g_{burn},\qquad g_{age} = \max\!\left(0.5,\ 1 - \frac{age}{1.0}\right),\qquad g_{burn} = 0.5 + 0.5\,\min\!\left(\frac{T}{1.5},\ 1\right)
```

  with age the youngest touching particle's age in seconds (the wiki: 100 %
  for new particles down to 50 % at 0.5 s; the shape between is assumed
  linear) and T the time the victim has been hit without a break (reset
  when no flame touches him for a flame tick). Per-tick range 6.5–13: the
  wiki's numbers. (Wiki "Damage" says full after 0.9 s and the weapon page
  1.5 s, 0.75 s very close: Open questions 14.)
- No distance modifier. Crits: rapid-fire (weapons.md §2.6); a crit tick is
  3 × the tick's damage — the wiki's 9.75–19.5 per tick suggests a crit
  flame is only ×1.5 of the normal one: Open questions 14.
- **Ignition**: a hit enemy (not a Pyro) is set on fire (§7.3).
- Flames go through teammates, do them nothing, and extinguish burning
  teammates (reference).

#### 7.3 Afterburn

- A burning player takes 4 damage every 0.5 s (8 hp/s) until the burn
  ends. The damage is credited to the Pyro who lit him, with the
  Flame Thrower's kill icon (`flamethrower`).
- Duration from the Flame Thrower (since 2017-10-20): at least 3 s, growing
  with the time in direct contact with flame, at most 10 s. Simple form:
  each flame tick that hits him sets the remaining burn time to
  max(remaining, 3) and then adds the tick's length to it, capped at 10
  (exact growth: Open questions 15).
- Afterburn never crits (it can mini-crit: 5 per tick, reference). Enemy
  Pyros are immune.
- Extinguished by: water up to the eyes, picking up a health kit, being
  healed for a full second, a friendly Pyro's airblast (reference).

#### 7.4 Airblast (reference only)

attack2: costs 20 ammo, refires after 0.75 s, a cone ahead of the Pyro
that pushes enemies and reflects projectiles; sound
`Weapon_FlameThrower.AirBurstAttack`, arms sequence `ft_alt_fire`
(1.0 s). Not specified further here.

### 8. Syringe Gun

- Every 0.105 s while attack is held, one needle from the clip of 40.
- Needle: a projectile from an offset under the gun (Open questions 8),
  990 units/s along the aim (wiki infobox; the projectiles page says
  1000), falling with gravity (fraction of sv_gravity: Open questions 16,
  a community figure is 0.3), small random spread (Open questions 16).
  Passes through teammates; stops at the first enemy or the world.
- Damage on hit: 10 × M(d, 0.4) with d the Medic-to-victim distance at
  impact (12 at 0, 10 at 512, 5.28 at ≥ 921.6). Crit 30 (rapid-fire
  crits). Needles can't be reflected by airblast; they destroy stickies.
- Whole-clip reload 1.305 s; the arms' `sg_reload` is 1.5 s.

### 9. Assets (from the install)

#### 9.1 Models

All are `c_` weapons bone-merged onto the class arms (install.md §4.2).

| Weapon | `model_player` | Bones | `muzzle` (bone, position) | Other attachments | `min_viewmodel_offset` |
|---|---|---|---|---|---|
| Pistol | `models/weapons/c_models/c_pistol/c_pistol.mdl` | `weapon_bone`, `vm_weapon_bone`, `c_weapon_stattrack` | `weapon_bone` (0, 5.96, 9.19) | `eject_brass` (1.01, 6.19, 3.46) | 10 0 −10 |
| SMG | `models/weapons/c_models/c_smg/c_smg.mdl` | `weapon_bone`, `weapon_bone_1`, `vm_weapon_bone`, `vm_weapon_bone_1`, `c_weapon_stattrack` | `weapon_bone` (0, 2.61, 26.49) | `eject_brass` (−0.70, 2.60, 5.04) | 10 0 −7 |
| Sniper Rifle | `models/weapons/c_models/c_sniperrifle/c_sniperrifle.mdl` | `weapon_bone`, `vm_weapon_bone`, `vm_weapon_bone_1`…`_3`, `c_weapon_stattrack` | `weapon_bone` (0, 3.77, 45.82) | `eject_brass` (0, 3.80, 6.80) | 10 0 −10 |
| Revolver | `models/weapons/c_models/c_revolver/c_revolver.mdl` | `weapon_bone`, `weapon_bone_1`…`_4`, `c_weapon_stattrack` | `weapon_bone` (0, 2.72, 14.78) | `eject_brass` (0, 1.92, 1.59) | 10 0 −6 |
| Grenade Launcher | `models/weapons/c_models/c_grenadelauncher/c_grenadelauncher.mdl` | `weapon_bone`, `weapon_bone_1`…`_4`, `procedural_chamber`, `c_weapon_stattrack` | `weapon_bone_1` (0, 8.23, 28.87) | – | 10 0 −10 |
| Stickybomb Launcher | `models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl` | `weapon_bone`, `weapon_bone_1`, `vm_weapon_bone`, `vm_weapon_bone_1`, `c_weapon_stattrack` | `weapon_bone` (0, 1.91, 24.79) | – | 10 0 −10 |
| Flame Thrower | `models/weapons/c_models/c_flamethrower/c_flamethrower.mdl` (+ `c_flamethrower_pilotlight.mdl` attached, same bones and attachments) | `weapon_bone`, `weapon_bone_1`, `weapon_bone_L`, `c_weapon_stattrack` | `weapon_bone` (0.64, 2.35, 58.05), tilted 8° | – | 10 0 −10 |
| Syringe Gun | `models/weapons/c_models/c_syringegun/c_syringegun.mdl` | `weapon_bone`, `vm_weapon_bone`, `vm_weapon_bone_1`, `_2` | `weapon_bone` (0.19, 6.06, 21.34) | – | 10 0 −12 |

(All also have `pedestal_0` and `icon_camera`, all but the Syringe Gun
`unusual_0`…`5`. The Revolver and Grenade Launcher have a 30-frame
looping `idle` of their own (cylinder/drum); the others a 1-frame `idle`.
The Grenade Launcher's drum turns through `procedural_chamber`, which has
no arms counterpart.)

Projectiles (one bone each; their collision size: Open questions 8):

| Projectile | Model |
|---|---|
| Grenade | `models/weapons/w_models/w_grenade_grenadelauncher.mdl` (inferred by name; `w_grenade_pipebomb.mdl` also ships) |
| Stickybomb | `models/weapons/w_models/w_stickybomb.mdl` (`w_stickybomb_d.mdl`, `…2`, `…3`, `…_defender` are unlocks') |
| Needle | `models/weapons/w_models/w_syringe_proj.mdl` |

Arms: `models/weapons/c_models/c_<class>_arms.mdl` for scout, engineer,
sniper, spy, demo, pyro, medic; the sequences are in
`c_<class>_animations.mdl` (install.md §4.3).

#### 9.2 First-person sequences

Durations (frames − 1) / fps; all at 30 fps; "loop" = looping flag.

| Class / weapon | Draw | Idle | Fire | Reload | Other |
|---|---|---|---|---|---|
| Scout / Pistol | `p_draw` 0.6667 | `p_idle` 2.0 loop | `p_fire` 0.2667 | `p_reload` 1.2 | – |
| Engineer / Pistol | `pstl_draw` 0.7667 | `pstl_idle` 1.7667 loop | `pstl_fire` 0.2 | `pstl_reload` 1.2333 | – |
| Sniper / SMG | `smg_draw` 0.9667 | `smg_idle` 1.6667 loop | `smg_fire` 0.1 | `smg_reload` 1.2333 | – |
| Sniper / Sniper Rifle | `draw` 1.0 | `idle` 2.0 loop | `fire` 1.6667 (shot and bolt) | – | none for zoom: the view model is hidden while scoped |
| Spy / Revolver | `draw` 0.8 | `idle` 1.3333 loop | `fire` 0.5 | `reload` 1.3333 | `reload2` 1.3333 (`ACT_SECONDARY_VM_RELOAD2`) |
| Demoman / Grenade Launcher | `g_draw` 1.0 | `g_idle` 1.6667 loop | `g_fire` 0.6 | `g_reload_start` 0.6, `g_reload_loop` 0.6667, `g_reload_end` 0.6333 | `g_auto_fire` 3.9667 (`ACT_SECONDARY_VM_PULLBACK`, unused by the stock gun) |
| Demoman / Stickybomb Launcher | `sb_draw` 1.0 | `sb_idle` 1.6667 loop | `sb_fire` 0.6 | `sb_reload_start` 0.3333, `sb_reload_loop` 0.6667, `sb_reload_end` 0.5333 | `sb_autofire` 4.0 (`ACT_PRIMARY_VM_PULLBACK`): the charge |
| Pyro / Flame Thrower | `ft_draw` 1.0 | `ft_idle` 2.0 loop | `ft_fire` 0.3 loop | – | `ft_alt_fire` 1.0 (`ACT_PRIMARY_VM_SECONDARYATTACK`, airblast) |
| Medic / Syringe Gun | `sg_draw` 0.8667 | `sg_idle` 3.0 loop (61 frames at 20 fps) | `sg_fire` 0.2333 | `sg_reload` 1.5 | – |

Activities follow the slot (`anim_slot`): Pistol, SMG, Revolver and the
Grenade Launcher use `ACT_SECONDARY_VM_DRAW`, `…_IDLE`,
`…_PRIMARYATTACK`, `…_RELOAD` (the Grenade Launcher's
`ACT_SECONDARY_RELOAD_START` / `_FINISH` too); Sniper Rifle, Stickybomb
Launcher, Flame Thrower, Syringe Gun the `ACT_PRIMARY_*` set. The Pyro's
animation file also has `df_idle` on `ACT_PRIMARY_VM_IDLE` (the Dragon's
Fury's): pick `ft_idle` by name, not by activity (Open questions 17).

As in weapons.md, the sequence lengths are animation only: deploy is
0.5 s, and the reload times are §1's (the clip reloads finish before
their animations: Pistol 1.005 vs 1.2, Syringe Gun 1.305 vs 1.5).

#### 9.3 Reload times (one round at a time)

As weapons.md §4 (attack with ≥ 1 round interrupts, etc.):

| Weapon | Start | Insert | First round in | Full from empty |
|---|---|---|---|---|
| Grenade Launcher | 0.6 (= `g_reload_start`) | 0.6 | 1.2 (wiki 1.24) | 0.6 + 4 × 0.6 = 3.0 |
| Stickybomb Launcher | 0.42 (derived: wiki 1.09 − 0.67; `sb_reload_start` is 0.3333) | 0.67 (≈ `sb_reload_loop` 0.6667) | 1.09 | 0.42 + 8 × 0.67 = 5.78 |

#### 9.4 Sounds (`scripts/game_sounds_weapons.txt`)

All shots on the weapon channel at 94 dB.

| Use | Pistol | SMG | Sniper Rifle | Revolver |
|---|---|---|---|---|
| shot | `Weapon_Pistol.Single` → `weapons/pistol_shoot.wav` | `Weapon_SMG.Single` → `weapons/smg_shoot.wav` | `Weapon_SniperRifle.Single` → `weapons/sniper_shoot.wav` | `Weapon_Revolver.Single` → `weapons/revolver_shoot.wav` |
| crit shot | `…SingleCrit` → `pistol_shoot_crit.wav` | `…SingleCrit` → `smg_shoot_crit.wav` | `…SingleCrit` → `sniper_shoot_crit.wav` | `…SingleCrit` → `revolver_shoot_crit.wav` |
| empty | `Weapon_Pistol.ClipEmpty` → `weapons/pistol/pistol_empty.wav` | `Weapon_SMG.ClipEmpty` (same wave) | `Weapon_SniperRifle.ClipEmpty` (same) | `Weapon_Revolver.ClipEmpty` (same) |
| reload | Scout `Weapon_Pistol.ReloadScout` → `pistol_reload_scout.wav`; Engineer `Weapon_Pistol.ClipIn` → `pistol_clipin.wav` | `Weapon_SMG.ClipOut` → `smg_clip_out.wav`, `Weapon_SMG.ClipIn` → `smg_clip_in.wav` | `Weapon_SniperRifle.BoltBack`, `.BoltForward` (the bolt in the fire animation) | `Weapon_Revolver.Reload1` / `2` / `3` → `revolver_reload_cylinder_open/arm/close.wav` |
| world reload | `Weapon_Pistol.WorldReload` → `pistol_worldreload.wav`; Engineer `…WorldReloadEngineer` → `pistol_clipin.wav` | `Weapon_SMG.WorldReload` → `smg_worldreload.wav` | `…WorldReload` → `sniper_worldreload.wav` | `…WorldReload` → `revolver_worldreload.wav` |
| draw | `Weapon_Pistol.Draw` → `weapons/draw_secondary.wav`; Engineer `Weapon_Pistol.Draw2` → `draw_pistol_engineer.wav` | `Weapon_SMG.Draw` → `draw_secondary.wav` | `Weapon_SniperRifle.Draw` → `draw_primary.wav` | `Weapon_Revolver.Draw` → `draw_secondary.wav` |

| Use | Grenade Launcher | Stickybomb Launcher | Flame Thrower | Syringe Gun |
|---|---|---|---|---|
| shot | `Weapon_GrenadeLauncher.Single` → `grenade_launcher_shoot.wav` | `Weapon_StickyBombLauncher.Single` → `stickybomblauncher_shoot.wav` | start `Weapon_FlameThrower.FireStart` → `flame_thrower_start.wav` (static channel); loop `.FireLoop` → `flame_thrower_loop.wav` (weapon channel); end `.FireEnd` → `flame_thrower_end.wav` | `Weapon_SyringeGun.Single` → `syringegun_shoot.wav` |
| crit | `…SingleCrit` → `grenade_launcher_shoot_crit.wav` | `…SingleCrit` → `stickybomblauncher_shoot_crit.wav` | `.FireLoopCrit` → `flame_thrower_loop_crit.wav` | `…SingleCrit` → `syringegun_shoot_crit.wav` |
| empty | – | – | – | `Weapon_SyringeGun.ClipEmpty` → `pistol/pistol_empty.wav` |
| reload | `Weapon_GrenadeLauncher.DrumOpen` / `.DrumLoad` / `.DrumClose` (→ `grenade_launcher_drum_*.wav`), `.DrumStart` / `.DrumStop` (volume 0.4) | `Weapon_StickyBombLauncher.BoltBack` / `.BoltForward` | – | `Weapon_SyringeGun.Reload1` / `2` / `3` → `syringegun_reload_air1.wav`, `_glass2.wav`, `_air2.wav` |
| world reload | `…WorldReload` → `grenade_launcher_worldreload.wav` | `…WorldReload` → `stickybomblauncher_worldreload.wav` | – | `…WorldReload` → `syringegun_worldreload.wav` |
| draw | `Weapon_GrenadeLauncher.Draw` → `draw_secondary.wav` | (none of its own; Open questions 18) | `Weapon_FlameThrower.Draw` → `draw_primary.wav` | `Weapon_SyringeGun.Draw` → `draw_primary.wav` |
| special | – | charge `Weapon_StickyBombLauncher.ChargeUp` → `stickybomblauncher_charge_up.wav`; detonate `Weapon_StickyBombLauncher.ModeSwitch` → `stickybomblauncher_det.wav` | idle `Weapon_FlameThrower.PilotLoop` → `flame_thrower_pilot.wav` (74 dB, volume 0.25); flame hits `.FireHit` → `flame_thrower_fire_hit.wav`; airblast `.AirBurstAttack` → `flame_thrower_airblast.wav` | – |
| projectile | bounce `Weapon_Grenade_Pipebomb.Bounce` → `grenade_impact.wav` (74 dB, pitch 96–100); explosion: Open questions 18 | explosion: Open questions 18 | – | – |

Explosion candidates: `Weapon_Grenade_Pipebomb.Explode` → random of
`weapons/pipe_bomb1.wav`, `pipe_bomb2.wav`, `pipe_bomb3.wav` (95 dB), or
the rocket's `BaseExplosionEffect.Sound` (weapons.md §8.3). Burning:
`Player.OnFire` (body channel, `ambient/fire/fire_small_loop1/2.wav`) and
`TFPlayer.FirePain` (`player/fire.wav`) in `game_sounds_player.txt`.

The pairing of entries to weapons is by name (the weapon scripts are
encrypted), as weapons.md Open questions 13.

#### 9.5 Kill icons (`scripts/mod_textures.txt`, `HUD/d_images`; cell x, y, w, h)

| Kill-icon name | Cell | Used for |
|---|---|---|
| `pistol`, `pistol_scout` | 0, 32, 96, 32 (both) | Engineer's / Scout's Pistol |
| `smg` | 0, 128, 96, 32 | SMG |
| `sniperrifle` | 16, 96, 32, 32 | Sniper Rifle body-shot kill |
| `headshot` | 120, 352, 42, 32 | Sniper Rifle headshot kill |
| `revolver` | 96, 32, 96, 32 | Revolver |
| `tf_projectile_pipe` | 0, 288, 96, 32 | grenade (kills name the projectile) |
| `tf_projectile_pipe_remote` | 96, 224, 96, 32 | stickybomb |
| `flamethrower` | 0, 416, 96, 32 | Flame Thrower, and its afterburn |
| `syringegun_medic` | 0, 352, 96, 32 | Syringe Gun |

Drawn as hud.md §3.6 (crit kills with the `crit` cell behind; a headshot
kill is a crit kill).

### 10. Worked damage

Spread off, no crits, f = 1 (direct hit or at the blast centre), rounded
half up (weapons.md Open questions 3) unless shown:

| Weapon | 0 units | 512 | 1024 | Crit (any range) |
|---|---|---|---|---|
| Pistol (15, k 1) | 22.5 → 23 (wiki 22) | 15 | 7.92 → 8 | 45 |
| SMG (8, k 1) | 12 | 8 | 4.224 → 4 | 24 |
| Revolver (40, k 1) | 60 | 40 | 21.12 → 21 | 120 |
| Sniper Rifle, unscoped or uncharged | 50 | 50 | 50 | – (no random crits) |
| Sniper Rifle, full charge, body / head | 150 / 450 | 150 / 450 | 150 / 450 | – |
| Grenade, direct (100) | 100 | 100 | 100 | 300 |
| Sticky, age ≤ 5 s (120, k 0.4) | 144 | 120 | 63.36 → 63 (wiki 64) | 360 |
| Sticky, age > 5 s | 120 | 120 | 120 | 360 |
| Syringe (10, k 0.4) | 12 | 10 | 5.28 → 5 | 30 |
| Flame tick (13 × age × burn) | 6.5–13 | – (range ≈ 350) | – | 3 × (Open questions 14) |
| Afterburn | 4 per 0.5 s | | | never crits |

## Per-tick order

Single-bullet gun (Pistol, SMG, Revolver, Sniper Rifle): weapons.md's
hitscan order, with these changes:

1. Sniper Rifle: advance the scope state (attack2 toggles; τ, c(τ));
   during the bolt cycle after a scoped shot, re-scope at the interval's
   end if auto re-zoom is on.
2. If attack is held, the interval has passed and ammo ≥ 1: crit decision
   (Revolver: roll now; Pistol, SMG: burst or once-a-second roll; Sniper:
   none), remove 1 round, next fire time += interval.
3. Direction: dead centre if ≥ 1.25 s since the last shot (Sniper: always
   centre), else spread s. Record the shot time.
4. Rewind, trace, restore; damage: base × M (Sniper: D, ×3 if a scoped
   head hit after ≥ 0.2 s scoped); round; apply.
5. Sniper: if scoped, leave the scope and reset τ.

Grenade and sticky launcher:

1. Grenade: attack held, interval passed, clip ≥ 1 → crit roll, launch
   (§4.1). Sticky: attack pressed → begin charge; on release or at 4.0 s →
   crit roll, launch at v(h) (§5.1); if 9 live, schedule the oldest's
   detonation.
2. attack2 with the Stickybomb Launcher out: detonate every armed sticky.
3. Each later tick, per projectile: gravity, drag, move; on contact:
   grenade → explode if it hasn't touched the world and hit an enemy, else
   bounce (and mark it as having touched the world); sticky → stick to the
   world, bounce off enemies. Grenade: explode when its fuse ends.
4. Explosion: as weapons.md §5.2 with the damage of §4.3 / §5.4 and the
   self-damage of §6; knockback.

Flame Thrower:

1. While attack held with ammo: use ammo at 12.5/s; emit particles.
2. Move particles; drop the expired ones and those hitting the world.
3. Every 5th tick (0.075 s): per enemy touched by a particle, D of §7.2,
   ignite (or extend the burn); advance each victim's T, reset T for
   enemies not touched this flame tick.
4. Every 0.5 s of each burning player's burn: 4 damage; end the burn when
   its time runs out.

Syringe Gun: as the hitscan gun's steps 2–3 but spawn a needle; each later
tick move it under gravity, on contact with an enemy apply 10 × M(d, 0.4)
(d at impact).

## Edge cases

- **Grenade direct hit after a bounce**: none: a grenade that has touched
  the world only explodes on its fuse, even if it then rolls into an enemy.
- **Grenade touching a teammate**: passes through; it is not a bounce.
- **Sticky on an enemy**: bounces off; if it then lands on the world it
  sticks.
- **Detonating before arming**: nothing for those stickies; a later
  attack2 detonates them once armed.
- **Sticky fired at full charge without releasing**: launched at 4.0 s; the
  player must release and press again to charge the next one.
- **Sticky crit**: decided at launch; a crit sticky detonated 10 s later is
  still a crit (360 × f).
- **Sticky distance for the 5 s rule**: the Demoman's position at
  detonation counts (as the rocket's at impact).
- **Sniper scoped < 0.3 s**: c = 0, D = 50; a head hit before 0.2 s is a
  body hit (50).
- **Sniper unscoped head hit**: 50, no crit.
- **Pistol held**: fires at 0.15 s intervals; tapping faster does not fire
  faster (since 2009-08-13).
- **Flame Thrower under water**: it does not fire (reference; later).
- **Afterburn on a Pyro**: none; direct flame damage still applies.
- **Clip reload interrupted by a weapon switch**: nothing moved (weapons.md
  §3).

## Quirks

- The Sniper Rifle's fire animation (1.6667 s) is longer than its interval
  (1.5 s): the next shot can come before the bolt animation ends. Keep the
  timing.
- `Weapon_SniperRifle.BoltForward` plays `sniper_bolt_back.wav` and
  `.BoltBack` plays `sniper_bolt_forward.wav` (swapped in the data). Use
  the entries as they are.
- The Grenade Launcher animates as a secondary and the Stickybomb Launcher
  as a primary (`anim_slot`), the reverse of their slots.
- The Stickybomb Launcher's charge animation `sb_autofire` is exactly the
  4.0 s charge.
- Pistol, SMG and Revolver need 1.25 s for a dead-accurate shot, not the
  pellet weapons' 0.25 s.
- `Weapon_FlameThrower.Fire` and `.WindDown` are copies of `.FireStart`
  and `.FireEnd`.

## Test cases

Spread off, crits off unless said, no random pellet spread unless said,
f = 1 unless said.

| # | Setup | Input | Expected |
|---|---|---|---|
| 1 | Pistol, victim at 0 / 512 / 1024 | hit | 22.5 → 23 (or 22, weapons.md OQ 3) / 15 / 8 |
| 2 | SMG, 0 / 512 / 1024 | hit | 12 / 8 / 4 |
| 3 | Revolver, 0 / 256 / 512 / 1024 | hit | 60 / 53.75 → 54 / 40 / 21 |
| 4 | Revolver crit, 1024 | hit | 120 |
| 5 | Pistol, Scout, hold attack 2 s from full | – | 12 shots in 1.65 s (first at 0, every 0.15 s), then the reload; clip 12 again at last shot + interval + 1.005 s; reserve 36 → 24 |
| 6 | Pistol, Engineer, clip 0, reserve 200 | – | clip 12 after 1.035 s, reserve 188 |
| 7 | SMG, clip 3, reserve 1 | reload | after 1.1 s clip 4, reserve 0 |
| 8 | Revolver, last shot ≥ 1.25 s ago | fire | dead centre; a second shot 0.5 s later has spread ≤ 0.025 |
| 9 | Pistol, rapid-fire crit roll succeeds at t | keep firing | every shot from t to t + 2 s deals 45 |
| 10 | Sniper, unscoped, body or head at 2000 units | fire | 50 |
| 11 | Sniper, scoped 0.1 s, head | fire | 50 (headshot not yet allowed) |
| 12 | Sniper, scoped 0.25 s, head | fire | 150 (crit of D = 50) |
| 13 | Sniper, scoped 1.3 / 2.3 / 3.3 / 5.0 s, body | fire | 50 / 100 / 150 / 150 |
| 14 | Sniper, scoped 3.3 s, head, 3000 units | fire | 450 |
| 15 | Sniper, scoped 2.3 s, head | fire | 300 |
| 16 | Sniper, scoped, fire, attack2 not pressed again, auto re-zoom on | – | out of scope after the shot, back in at +1.5 s, c = 0, D = 50 until +2.8 s |
| 17 | Sniper scoped | hold forward | speed 80; unscoped 300 |
| 18 | Sniper, 25 rounds | 25 shots | then empty sound; no reload |
| 19 | Sniper, teammate between him and the enemy | fire | the enemy is hit, the teammate isn't |
| 20 | Sniper, many shots with crits on | – | no random crit ever |
| 21 | Grenade fired level from standing on flat ground, no drag | – | initial velocity (1200, 0, 200) in the aim frame; flight follows gravity 800 |
| 22 | Grenade direct hit on an enemy, Demoman at 0 / 512 / 1500 | – | 100 / 100 / 100 |
| 23 | Grenade lands, rolls into an enemy at 1.0 s | – | no explosion then; explodes at 2.3 s where it is |
| 24 | Grenade explodes 73 units from an enemy's centre | – | 100 × 0.75 = 75 |
| 25 | Grenade explodes 146 / 147 units away | – | 50 / 0 |
| 26 | Crit grenade, direct | – | 300 |
| 27 | Grenade Launcher clip 0, reserve 16 | – | clip 1 at +1.2 s (wiki 1.24), 4 at +3.0 s; reserve 12 |
| 28 | Sticky, attack pressed and released at once | – | forward speed 900 |
| 29 | Sticky, held 2 s / 4 s | – | 1650 / 2400; at 4 s it launches without release |
| 30 | Sticky, launch, attack2 at +0.5 s, again at +0.8 s | – | nothing at 0.5 s; explodes at 0.8 s |
| 31 | 8 stickies out | launch a 9th | the oldest explodes shortly after; 8 remain |
| 32 | Sticky stuck, age 2 s, Demoman 0 / 512 / 1024 from the enemy at the centre | detonate | 144 / 120 / 63 |
| 33 | Same, age 6 s | detonate | 120 / 120 / 120 |
| 34 | Crit sticky, age 1 s, 73 units from the enemy, Demoman 1500 away | detonate | 360 × 0.75 = 270 |
| 35 | Sticky in the air (not stuck), age 0.7 s, enemy 130 units away | detonate | outside the 124.1 radius: 0 |
| 36 | Same, age 2.7 s | detonate | inside 146: 120 × (1 − 0.5·130/146) × M |
| 37 | Sticky hits an enemy in flight | – | bounces off, no explosion |
| 38 | Demoman dies with 5 stickies out | – | all 5 vanish, no explosions |
| 39 | Demoman airborne, own sticky at 0 from his centre | detonate | 120 × 0.75 = 90 (grounded: 120) |
| 40 | Stickybomb Launcher clip 0, reserve 24 | – | first at +1.09 s, then every 0.67 s; full at 5.78 s; reserve 16 |
| 41 | Flame Thrower, enemy at 100 units, continuous | 1.5 s | flame ticks every 0.075 s rising from ≈ 6.5 × g_age to 13 × g_age; ignited |
| 42 | Flame Thrower, enemy at 450 units | hold | no damage (beyond reach) |
| 43 | Flame Thrower, 200 ammo | hold | 16 s of fire |
| 44 | Enemy lit for one flame tick, then out of the flame | – | burns ≥ 3 s: 4 at +0.5, +1.0, … (6 ticks, 24) |
| 45 | Enemy in flames 10 s, then out | – | burns 10 s more: 20 ticks, 80 |
| 46 | Enemy Pyro hit by flames | – | direct damage only, no afterburn |
| 47 | Burning player picks up a health kit | – | burn ends |
| 48 | Syringe at 0 / 512 / 1024 (Medic at impact) | hit | 12 / 10 / 5 |
| 49 | Syringe Gun, hold attack, clip 40 | – | 40 needles in 4.095 s, then reload 1.305 s |
| 50 | Syringe fired level | – | drops below the aim line over distance (gravity), flies at 990 |
| 51 | Any of the eight | switch to it | first attack possible 0.5 s later |

Probe checks (live dedicated server, bots; never ports 27015/27030):

| # | How | Expected |
|---|---|---|
| P1 | Log a grenade's and a sticky's position each tick after launch (uncharged and full charge) | the launch velocity split, gravity, drag (Open questions 7) |
| P2 | Sniper: damage on a bot at scoped times 0.2, 0.3, 1.3, 2.3, 3.3 s; head vs body | §3.2–3.3 |
| P3 | Flame Thrower: damage log per tick on a bot at 50/150/250/340 units | g_age, g_burn, range (Open questions 14) |
| P4 | Afterburn duration after 0.1 s, 1 s, 3 s, 10 s of contact | Open questions 15 |
| P5 | Demoman's own sticky at his feet: grounded vs airborne damage, velocity after | 0.75, knockback scale (Open questions 9) |
| P6 | Ninth sticky: ticks until the oldest explodes | Open questions 12 |

## Open questions

1. **Rounding** follows weapons.md Open questions 3 (the wiki's 22 for the
   Pistol's 22.5 and 64 for the sticky's 63.36 disagree with half-up).
2. **Pistol, SMG, Revolver spread**: s = 2/N from the wiki's "N:1" uses
   weapons.md's reading of the Scattergun's 30:1; the distribution and
   whether the spread grows with consecutive shots (the wiki only says
   accuracy is restored after 1.25 s) are not documented.
3. **Sniper zoom**: the 20° zoom field of view is from memory of common
   community configs, not from the install (the weapon scripts are
   encrypted); the client's `cl_autorezoom` default (believed 1), the
   crouched-and-scoped speed (wiki 1.33 % seems wrong; check), whether the
   Sniper can scope in the air, and whether the bolt cycle unscopes at the
   shot or later. Check on the Windows client.
4. **Sniper spread**: assumed none, scoped or not; whether the random damage
   spread (`tf_damage_disablespread` 0) applies to the rifle is not stated.
5. **SMG crits**: the wiki's crit page lists Pistol, Minigun, Syringe Gun,
   Flame Thrower as rapid-fire but doesn't name the SMG; assumed
   rapid-fire (same 0.105 s interval as the Minigun).
6. **Holding attack on the Revolver** and attack during a clip reload: the
   wiki doesn't say whether holding refires; assumed it does. Whether
   attack with rounds left cancels a clip reload (as it does a shell
   reload) is not documented.
7. **Grenade and sticky launch**: the 1200 + 200 up split is fitted to the
   wiki's 1216.6; the sticky's endpoints 900 / 2400 are inferred from the
   wiki's 925.38 / 2409.2 (which imply ≈ 210 up) and conflict with the
   Stickybomb page's "805 uncharged". The random jitter, the drag and
   whether gravity is the full sv_gravity are unknown (P1).
8. **Projectile offsets and sizes**: grenade/sticky (16, 8, −6) and the
   needle's offset are community figures not verified; collision sizes of
   the three projectiles are not documented (the models' visual bounds were
   not read). P1.
9. **Demoman self-damage and knockback**: the 0.75 airborne factor is
   derived from the wiki's ranges (the upper ends 73 and 114 don't fit a
   simple model); the server cvar behind it, if any, and the knockback
   scale of his own blasts are unknown (P5).
10. **Grenade damage variance** of ±2 % (wiki) may be a separate rule from
    `tf_damage_disablespread`; assumed off with the default.
11. **Sticky arm time**: 0.7 s (wiki infobox) vs 0.8 s ("base arm time" in
    the 2014-12-22 notes). The air-radius ramp's start (0.85 at arm time)
    uses the same number.
12. **Sticky detonation**: the delay before the oldest sticky blows when a
    ninth is launched; whether attack2 detonates with another weapon out
    (believed: launcher must be out). P6.
13. **Flame particles**: whether 2.8° is a half-angle or full cone, whether
    the Pyro's velocity is added (`flame_ignore_player_velocity` exists as
    an attribute, suggesting it is added by default), the particle hit
    size, and the drag law (exponential assumed).
14. **Flame damage ramps**: the burn ramp's length (1.5 s weapon page, 0.9 s
    damage page, 0.75 s very close), the shape of the age factor between
    0 and 0.5 s, and the crit factor (the wiki's 9.75–19.5 crit per tick is
    1.5 × the normal, not 3 ×). P3.
15. **Afterburn duration growth** between 3 and 10 s by contact time: the
    exact rule is not public. P4.
16. **Syringe** gravity fraction (community 0.3), random spread, speed (990
    vs 1000).
17. **`ft_idle` vs `df_idle`** share `ACT_PRIMARY_VM_IDLE` with equal weight
    in the Pyro's animation file; how the game picks the stock one
    (activity weight lists, a per-item remap) is unknown; choose by name.
18. **Explosion sound** of grenades and stickies (`Weapon_Grenade_Pipebomb.Explode`
    vs the rocket's `BaseExplosionEffect.Sound`) and the Stickybomb
    Launcher's draw sound: by name only; check on the live game.
