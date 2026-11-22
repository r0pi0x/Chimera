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
says so. For the 2026-11-15 check, Valve's Source SDK 2013 TF2 game code
(the shared, server and client game code of the 2025 release) was read in
place in a separate spec session: the sniper rifle's charge, zoom and dot,
the launchers' charge and detonation, the grenade and sticky projectiles'
physics and explosions, the flame emitter and its damage, afterburn, the
needle projectile, the bullet spread's first-shot rule, the shared damage
and radius-damage rules and the self-knockback. Nothing from it was
copied; the numbers and rules below are restated in our words. The SDK
does not contain the per-weapon scripts (damage, interval, clip, spread,
reload times, blast radius) nor the per-weapon damage-type table, so those
numbers remain the wiki's (and weapon_scripts.md's run-time values).
The 2026-11-20 recheck (spin, air drag, arm time) read the same game
code again and measured the running game (the projectiles' physics
velocity per tick through VScript); the projectile models' collision
data was read from the install in place.
Status: checked against the SDK's game code (2026-11-15)

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
weapons whose first shot after more than 1.25 s without firing is dead
accurate. The Sniper Rifle has no distance modifier; while scoped its
charge grows at 50 per second from 0.3 s after zooming (damage 50 to 150,
full 3.3 s after zooming in), and a scoped head hit at least 0.2 s after
the zoom is a ×3 crit, its only crit. The Demoman's grenades fly in an
arc, explode at full damage on a direct hit before they touch anything
else, and otherwise roll and burst on a 2 s fuse at **60 %** damage, with
no distance modifier; his stickybombs are launched faster the longer
attack is held, stick to the world, arm after **0.8 s** and burst on
attack2 (with any of his weapons out), up to 8 at a time. The Flame
Thrower emits a stream of flame points every 0.02 s that fly about 290
units; each point that overlaps an enemy may hurt him at most once per
0.075 s, by 13 scaled down with the point's age and up with how densely
the victim has been hit (a "heat" measure), and sets him on fire:
afterburn 4 every 0.5 s for 3 s plus 0.4 s per flame hit, at most 10 s.
The Syringe Gun fires slow needles (1000 units/s, 30 % gravity) with a
small random spread.

## Corrections from the SDK (2026-11-15)

- Sniper: zoom field of view 20 — confirmed (was "from memory").
- Sniper: after a scoped shot the scope drops at +0.5 s (was: "for the
  bolt cycle", unspecified) and auto re-zoom brings it back at +1.4 s (was
  +1.5 s, at the interval's end); charging resumes at once on the re-zoom
  (was: the 0.3 s delay again), so D = 50 until +2.4 s (was +2.8 s).
- Sniper: a jump unscopes and empties the charge, and the scope cannot be
  raised while jumping (was: open).
- Sniper: no shot within 0.1 s of zooming in or out; after an unscoped
  shot the scope is blocked for the fire animation (1.67 s) (new).
- Sniper: the headshot's 0.2 s counts from the last zoom change, so it
  restarts at every re-zoom (was: from the scope coming up only).
- Sniper dot: also drawn in the Sniper's own first-person view, clamped
  384 units ahead (was: "not drawn in his own view").
- Sniper: `cl_autorezoom` default 1 — confirmed (was open).
- Sniper, crouched scoped speed: the 80 cap with the normal crouch scaling
  (was "wiki 1.33 % ≈ 4 units/s").
- Grenade fuse: 2.0 s, checked every 0.2 s (≈ 2.1–2.2 s in practice) (was
  2.3 s).
- Grenade fuse explosion: 60 % damage (60 at the centre, crit 180) (was
  100); only a direct hit deals 100.
- Grenade and sticky launch: exactly 200 units/s up plus a random ±10
  units/s right and ±10 up (was "≈ 200", jitter open).
- Grenade and sticky spin: angular impulse (600, random ±1200, 0) degrees
  per second (was "spinning", unspecified), **applied twice** (2026-11-20
  recheck): the body starts at (1200, 2r, 0) degrees/s, §4.1.
- Grenade and sticky: rigid physics bodies with a 4-unit cube collision
  box; the stickies' and grenades' gravity is the physics world's (full
  `sv_gravity`) (was open).
- Grenades and stickies: pass through teammates only for the first 0.25 s
  of their flight, then bounce off them (was: always pass through).
- Grenades and stickies bouncing off a player: velocity mirrored about the
  contact normal × 0.45, × 0.5 more for an enemy, spin × −0.5 (was
  "bounce", unspecified).
- Sticky arm time: 0.8 s (was 0.7 s); the air-detonation radius ramp runs
  0.8 s to 2.8 s (was 0.7 to 2.7).
- Sticky detonation: attack2 works with any of the Demoman's weapons out
  (was: launcher out, open); unarmed stickies stay, with a deny sound.
- Ninth sticky: the oldest explodes at its next fuse check, within 0.2 s
  (was "shortly", open).
- Sticky charge: holding attack after a forced full-charge launch starts
  the next charge as soon as the interval allows (was: must release).
- Sticky distance class: the short-range-reduced ramp, 121.8 % at 0 units
  (146 at 0 for 120 base) (was k = 0.4: 120 %, 144); far end unchanged
  (52.8 %).
- Syringe distance class: the same short-range-reduced ramp, 121.8 % at 0
  (12.18 → 12) (was k = 0.4, 120 %).
- Demoman self-damage: × 0.75 always, grounded or airborne (was × 0.75
  airborne only, "derived"); it uses the launcher's base damage (100 / 120)
  and a 146 radius even for a reduced-damage roller.
- Demoman self-knockback: scale 9, grounded or airborne (was "own scale,
  unknown"; not the Soldier's 10 / 5).
- Splash distance to a player: the nearer of his centre and his feet
  (stated here for the Demoman's own blasts; weapons.md has the shared
  rule).
- Flame points: emitted every 0.02 s of fire, i.e. every 2 server ticks
  (≈ 33 a second) (was "about 33 a second", wiki).
- Flame lifetime: 0.6 ± 0.1 s, uniform (was 0.6 + 0–0.1).
- Flame spread: 2.8° scale per axis, a sum of two uniform draws (was "a
  cone of 2.8° half-angle", open).
- Flame drag: per tick, speed × (1 − 8.5 · tick) (was exponential
  e^(−8.5 t)); reach ≈ 40 + 287 + 12 ≈ 340 units ahead of the eye (was
  "≈ 350").
- Flame origin: 40 forward and 5 right of the eye (was "≈ 50 ahead").
- Flame and the Pyro's velocity: only its component along the flame's
  direction is added, undiminished (was open).
- Flame hit size: a box of half-size 12 units (was open).
- Flame age factor: max(0.5, 1 − age / life), 0.5 at half the point's
  own life (≈ 0.3 s) (was max(0.5, 1 − age / 1.0), 0.5 at 0.5 s).
- Flame density ramp: a per-victim heat index (+1 per touch, −1.25 per
  tick, factor 0.5 → 1 over heat 10 → 50) (was "0.5 → 1 over 1.5 s of
  continuous hits").
- Flame damage: at least 1 per hit (new).
- Flame ammo: 14 per second of nominal emission, 0.28 per emission:
  ≈ 9.3 a second at 66.7 tick, 200 lasts ≈ 21.4 s (was 12.5/s, 16 s).
- Flame crit: a full crit, ×3 by the shared rule (the wiki's ×1.5 question
  closed).
- Flames and teammates: no effect at all; extinguishing is the airblast's
  (was: flames extinguish burning teammates).
- Afterburn duration: 3 s on ignition, + 0.4 s per damaging flame hit,
  capped at 10 s, − 0.5 s per burn tick (was: max(remaining, 3) + tick,
  growth open).
- Afterburn after one flame hit: 3.4 s, 7 ticks, 28 damage (was 6 ticks,
  24).
- Afterburn on an enemy Pyro: a fixed 0.25 s burn with no damage (was
  "immune", no burn).
- Afterburn is put out by water at waist depth (was "up to the eyes").
- Airblast: also delays the next flame by 1.0 s (new).
- Syringe speed: 1000 (was 990).
- Syringe gravity: 0.3 of `sv_gravity` (240 units/s²) — confirmed (was a
  community figure).
- Syringe spread: random ±1.5° in pitch and in yaw, uniform (was open).
- Syringe start: 16 forward, 6 right, 8 down from the eye, aimed at the
  point under the crosshair (up to 2000 units away) (was open).
- Syringe size: a 2-unit cube (was open).
- Pistol, SMG, Revolver: the spread does not grow with consecutive shots;
  only the first-shot rule exists, and it needs strictly more than 1.25 s
  (was open).

## Units and conventions

As weapons.md: Hammer units, Z up, X forward, Y left, degrees; seconds;
server tick 0.015 s; timers are absolute times and an action due at T
happens on the first tick with time ≥ T. "Wiki" is the community TF2 wiki
as of 2024–2026. Spread s is weapons.md §6.1's; the wiki writes a gun's
spread as "N:1", which is s = 2 / N (weapons.md: 30:1 ≈ 0.0675).

Distance modifier: M(d, k) is weapons.md §2.1's modifier with near-range
scale k, R = 0.5, and (spread off, the default) t = min(d, 921.6) / 1024.
So M(0) = 1 + 0.5 k, M(512) = 1, M(≥ 921.6) = 0.528.

**Short-range-reduced modifier** M_s(d) (the class the game gives the
Stickybomb Launcher and the Syringe Gun): the far half is M₀ unchanged; in
the near half the normalised distance is compressed toward 0.5 before the
curve, instead of the result being scaled:

```math
M_s(d) = \begin{cases} M_0(0.35 + 0.3\,t) & t < 0.5 \\ M_0(t) & t \ge 0.5 \end{cases},\qquad t = \frac{\min(d,\ 921.6)}{1024}
```

with M₀ weapons.md §2.1's unscaled curve. M_s(0) = M₀(0.35) = 1.21825,
M_s(256) = 1.11166, M_s(512) = 1, M_s(768) = 0.65625, M_s(≥ 921.6) =
0.528. (The wiki's "120 %" is this 121.8 % rounded.) With the random
damage spread on, the ±0.1 jitter of weapons.md §2.3 is applied to the
compressed value. The SDK release lacks the per-weapon damage-type table
that assigns a weapon to a class; the assignment here (stickies and
syringes reduced, grenades none, Pistol/SMG/Revolver standard) is
inferred from the code's special cases and the wiki's numbers (Open
questions 19).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| deploy | 0.5 | s | every weapon here (weapons.md §3) |
| accurate_pause | 1.25 | s | Pistol, SMG, Revolver: a shot more than this long after the previous one is dead centre (SDK: strictly greater) |
| near_compress | 0.35 + 0.3 t | – | short-range-reduced modifier's near-half input (Syringe Gun, Stickybomb Launcher): 121.8 % at 0 units |
| pistol_damage / interval / spread | 15 / 0.15 / 0.04 | hp, s, – | script values (wiki) |
| smg_damage / interval / spread | 8 / 0.105 / 0.025 | hp, s, – | script values (wiki) |
| revolver_damage / interval / spread | 40 / 0.5 / 0.025 | hp, s, – | script values (wiki) |
| sniper_base | 50 | hp | any body shot, uncharged or unscoped (SDK) |
| sniper_charge_max | 150 | hp | charge damage at full charge (SDK) |
| sniper_charge_rate | 50 | hp/s | charge growth while scoped, per tick (SDK); clamped to at most 100/s with modifiers |
| sniper_charge_delay | 0.3 | s | after raising the scope by attack2 (the zoom's attack2 cooldown); none after an auto re-zoom (SDK) |
| sniper_headshot_delay | 0.2 | s | since the last zoom change before a head hit crits (SDK) |
| sniper_fire_after_zoom | 0.1 | s | no shot within this of a zoom toggle (SDK) |
| sniper_unzoom_after_shot | 0.5 | s | scoped shot to scope dropping (SDK) |
| sniper_rezoom_delay | 0.9 | s | scope dropping to auto re-zoom: 1.4 s after the shot (SDK) |
| sniper_interval | 1.5 | s | between shots (script value, wiki) |
| sniper_zoom_fov | 20 | deg | scoped field of view, reached in 0.1 s (SDK) |
| sniper_zoom_speed | 80 | units/s | movement cap while scoped (SDK) |
| sniper_dot_size | 6 (others), 2 (own view, ≤ 384 units ahead) | units | dot sprite size (SDK) |
| grenade_damage | 100 | hp | direct hit, at the centre; no distance modifier (script value, wiki) |
| grenade_fuse_scale | 0.6 | – | damage of a grenade that explodes on its fuse (SDK) |
| launch_speed | 1200 forward + 200 up (+ ±10 right, ±10 up random) | units/s | grenade launch velocity (SDK) |
| launch_offset | 16 forward, 8 right, −6 up | units | from the eye, clipped by an 8-unit box trace from the eye (SDK) |
| launch_spin | (600, ±1200, 0), added twice: (1200, 2r, 0) | deg/s | angular velocity on launch, r a random integer in [−1200, 1200], body axes (SDK; measured) |
| air_density | 2.0 | – | physics world's air density, scales all drag (§4.4) |
| sticky_drag_lin | (1.43, 1.41, 1.45) × 10⁻⁶ | per (unit/s) per tick, × ρ | per body axis (§4.4, fitted) |
| grenade_drag_lin | (0.75, 1.91, 1.90) × 10⁻⁶ | per (unit/s) per tick, × ρ | per body axis (§4.4, fitted) |
| sticky_drag_ang | (7.27, 7.36, ~7.3) × 10⁻⁷ | per (deg/s) per tick, × ρ | per body axis (§4.4, fitted) |
| pipe_box | 4 | units | grenade and sticky collision cube (half-size 2) (SDK) |
| grenade_fuse | 2.0 (checked every 0.2) | s | from launch (SDK) |
| teammate_pass | 0.25 | s | grenades and stickies pass teammates only this long (SDK) |
| player_bounce | 0.45 (× 0.5 more on enemies) | – | speed kept bouncing off a player (SDK) |
| blast_radius | 146 | units | grenade and stickybomb (script value, wiki); the Demoman's own blast always 146 (SDK) |
| demo_self_scale | 0.75 | – | Demoman's own grenade or sticky, always (SDK) |
| demo_self_push | 9 | – | knockback scale of the Demoman's own blast, grounded or airborne (SDK) |
| sticky_damage | 120 | hp | at the centre, M_s within 5 s of launch (script value, wiki; 5 s SDK) |
| sticky_flat_after | 5.0 | s | stickies older than this use modifier 1 (SDK) |
| sticky_arm | 0.8 | s | from launch until it can be detonated (SDK) |
| sticky_charge_time | 4.0 | s | hold time for the fastest launch; fires by itself then (SDK) |
| sticky_speed_min / max | 900 / 2400 forward (+ 200 up, + jitter) | units/s | launch speed uncharged / fully charged (SDK) |
| sticky_max | 8 | – | live stickies per Demoman (SDK) |
| sticky_air_radius | 0.85 → 1 over 2.0 s from arming | – | radius factor of an un-stuck sticky (SDK) |
| flame_damage | 13 | hp | one flame hit on one victim, before the age and heat factors (SDK; the script's damage is not used) |
| flame_hit_interval | 0.075 | s | per victim, between flame hits (SDK) |
| flame_emit | 0.02 | s | nominal interval between flame points (every 2 ticks at 0.015) (SDK) |
| flame_ammo | 14 per s × 0.02 = 0.28 | ammo per point | fractional, accumulated (SDK) |
| flame_speed / drag / up / spread / life | 2450 / 8.5 / 50 / 2.8 / 0.6 ± 0.1 | units/s, 1/s, units/s, deg, s | flame point (item schema `static_attrs`; use confirmed by the SDK) |
| flame_origin | 40 forward, 5 right | units | from the eye (SDK) |
| flame_box | 12 | units | half-size of a flame point's hit box (SDK) |
| flame_points_max | 30 | – | live points per Pyro (SDK) |
| flame_age_floor | 0.5 at half the point's life | – | damage factor of a point by age (SDK) |
| flame_heat | +1 per touch (+2 for a point under 0.02 s old), −1.25 per tick; factor 0.5 → 1 over 10 → 50 | – | per-victim density ramp (SDK) |
| airblast_cost / refire / flame_after | 20 / 0.75 / 1.0 | ammo, s, s | (SDK) |
| afterburn_damage / interval | 4 / 0.5 | hp, s | 8 hp/s (SDK) |
| afterburn_start / per_hit / max | 3 / 0.4 / 10 | s | duration on ignition, added per flame hit, cap (SDK) |
| afterburn_pyro | 0.25 | s | an enemy Pyro's fixed burn, no damage (SDK) |
| syringe_damage | 10 | hp | per needle, M_s (script value, wiki) |
| syringe_interval | 0.105 | s | (script value, wiki) |
| syringe_speed | 1000 | units/s | (SDK) |
| syringe_gravity | 0.3 | × sv_gravity | 240 units/s² at 800 (SDK) |
| syringe_spread | ±1.5 | deg | uniform, pitch and yaw separately (SDK) |
| syringe_offset | 16 forward, 6 right, −8 up | units | from the eye (SDK) |

## Behavior

### 1. The eight weapons at a glance

| | Pistol | SMG | Sniper Rifle | Revolver |
|---|---|---|---|---|
| Item defindex, `name` | 22 `TF_WEAPON_PISTOL` (Engineer), 23 `TF_WEAPON_PISTOL_SCOUT` (Scout) | 16 `TF_WEAPON_SMG` | 14 `TF_WEAPON_SNIPERRIFLE` | 24 `TF_WEAPON_REVOLVER` |
| `item_class` | `tf_weapon_pistol` | `tf_weapon_smg` | `tf_weapon_sniperrifle` | `tf_weapon_revolver` |
| Class, slot | Scout and Engineer, secondary | Sniper, secondary | Sniper, primary | Spy, secondary (his first weapon) |
| Kind | hitscan, 1 bullet | hitscan, 1 bullet | hitscan, 1 bullet, charge | hitscan, 1 bullet |
| Base damage | 15 | 8 | 50–150 by charge | 40 |
| Near ramp at 0 (k) | 150 % (1) | 150 % (1) | none (modifier 1; SDK) | 150 % (1) |
| Far fall-off | 52.8 % | 52.8 % | none (SDK) | 52.8 % |
| Fire interval | 0.15 s | 0.105 s | 1.5 s | 0.5 s |
| Spread s (wiki N:1) | 0.04 (50:1) | 0.025 (80:1) | 0 (Open questions 4) | 0.025 (80:1) |
| Clip / reserve | 12 / 36 Scout, 12 / 200 Engineer | 25 / 75 | 25, no clip | 6 / 24 |
| Reload | whole clip: 1.005 s Scout, 1.035 s Engineer | whole clip 1.1 s | none | whole clip 1.133 s |
| Crit rolls | rapid-fire (once a second, 2 s bursts) | rapid-fire (Open questions 5) | never random; scoped headshot = crit (SDK) | every shot |

| | Grenade Launcher | Stickybomb Launcher | Flame Thrower | Syringe Gun |
|---|---|---|---|---|
| Item defindex, `name` | 19 `TF_WEAPON_GRENADELAUNCHER` | 20 `TF_WEAPON_PIPEBOMBLAUNCHER` | 21 `TF_WEAPON_FLAMETHROWER` | 17 `TF_WEAPON_SYRINGEGUN_MEDIC` |
| `item_class` | `tf_weapon_grenadelauncher` | `tf_weapon_pipebomblauncher` | `tf_weapon_flamethrower` | `tf_weapon_syringegun_medic` |
| Class, slot | Demoman, primary (`anim_slot` secondary) | Demoman, secondary (`anim_slot` primary) | Pyro, primary | Medic, primary |
| Kind | arcing grenade, splash | charged sticky, splash on command | flame points | arcing needle projectile |
| Base damage | 100 direct, 60 on the fuse | 120 | 13 per flame hit (≥ 1; ×age ×heat) | 10 |
| Near ramp at 0 | none (modifier 1) | 121.8 % (M_s) within 5 s | none; age and heat factors instead | 121.8 % (M_s) |
| Far fall-off | none | 52.8 % within 5 s | none | 52.8 % |
| Fire interval | 0.6 s | 0.6 s (after launch) | a point every 0.02 s; ≤ 1 hit per victim per 0.075 s | 0.105 s |
| Clip / reserve | 4 / 16 | 8 / 24 | 200, no clip | 40 / 150 |
| Reload | one at a time: first 1.24, then 0.6 | one at a time: first 1.09, then 0.67 | none | whole clip 1.305 s |
| Crit rolls | every shot | every shot | rapid-fire | rapid-fire |

The fire intervals, clips, reloads, spreads and base damages (except the
Sniper Rifle's charge and the flame's 13) are per-weapon script values
that the SDK does not contain; they stay the wiki's and are applied from
the scripts at run time (weapon_scripts.md).

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
  point's numbers as `static_attrs`: `flame_speed` 2450,
  `flame_drag` 8.5, `flame_up_speed` 50, `flame_gravity` 0,
  `flame_spread_degree` 2.8, `flame_lifetime` 0.6,
  `flame_random_life_time_offset` 0.1. Item 21 also attaches
  `models/weapons/c_models/c_flamethrower/c_flamethrower_pilotlight.mdl`
  (`visuals` → `attached_models`). The SDK's flame emitter reads exactly
  these attributes; without them every value would be 0.
- `anim_slot`: the Grenade Launcher animates as a secondary
  (`ACT_SECONDARY_*`: the Demoman's `g_*` sequences) and the Stickybomb
  Launcher as a primary (`ACT_PRIMARY_*`: `sb_*`), the reverse of their
  item slots.

### 2. Pistol, SMG, Revolver (single-bullet hitscan)

All three are weapons.md §6.1's bullet: one trace from the eye, hitboxes,
no penetration, lag compensation; the damage of the hit is
base × M(d, 1), rounded (weapons.md §2.4; Open questions on rounding are
weapons.md's 3). Head hits are body hits for all three stock guns (SDK:
no hit locations for them).

- **First shot**: if more than 1.25 s (strictly) have passed since the
  gun last fired, the shot goes exactly along the crosshair; otherwise
  its direction gets the random spread of weapons.md §6.1 with the s
  above: on each of the two screen axes an offset (u₁ + u₂) × s with u₁,
  u₂ uniform in [−0.5, 0.5] (a triangular spread reaching ±s). The spread
  does not grow with consecutive shots (SDK). (This is weapons.md's
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
  every shot. Crit = 3 × base, no distance modifier. (Whether a gun is
  rapid-fire is a flag in its script.)
- The Spy's Revolver is the only gun he has; while cloaked he can neither
  fire nor reload (cloak is a later spec).

### 3. Sniper Rifle

#### 3.1 Zoom

- attack2 toggles the scope, at most once per 0.3 s. Scoped: field of
  view 20, reached over 0.1 s (and back to the default over 0.1 s on
  unscoping), the view model hidden, a scope overlay drawn, movement
  capped at 80 units/s (forward/back speed; crouched, the usual crouch
  scaling of movement.md applies on top of the cap).
- The scope can be raised only with ammo left, and not while the Sniper
  is in a jump (from pressing jump until landing; falling without a jump
  does not block it). Jumping while scoped drops the scope and empties
  the charge.
- A zoom toggle (in or out) blocks firing for 0.1 s (it never shortens a
  longer wait).
- Unscoping (attack2 again, a weapon switch, death, being stunned) drops
  the charge to 0. While scoped and within the shot interval after a
  shot, the rifle cannot be put away.
- **After a scoped shot** (with ammo left): the scope stays up for 0.5 s
  (the first tick after +0.5 s), then drops for the bolt cycle; jumping is
  disabled during those 0.5 s. If the client option `cl_autorezoom` is on
  (its default, 1), the scope comes back 0.9 s later (the first tick after
  +1.4 s from the shot); with it off, attack2 is blocked for 1.0 s after
  the scope drops. Pressing attack2 while a drop or a re-zoom is pending
  cancels the re-zoom (and blocks attack2 until 0.3 s after the cancelled
  re-zoom time). The last round's scoped shot drops the scope after 0.5 s
  with no re-zoom.
- **After an unscoped shot** the scope cannot be raised for the length of
  the fire animation (1.6667 s, §9.2).
- **The dot**: while scoped, a dot is drawn where the Sniper's aim trace
  (from his eye along his eye angles, stopped by players, buildings and
  the world, not by friendly combat items) ends, moved 4 units back toward
  him. Others see it at size 6 at any distance. In his own first-person
  view it is drawn too, at size 2, with the trace limited to 384 units
  (so it floats at most 384 units ahead). It is drawn as two sprites, the
  team's dot sprite at full size with half alpha and a white core scaled
  by the charge fraction c / 150 (at least 0.1). It is not drawn in a
  third-person camera. Its look beyond that is a later spec.

#### 3.2 Charge

The charge c is a stored number, changed once per server tick:

- while scoped, not in the 0.5 s after a scoped shot, and not within the
  0.3 s attack2 cooldown after raising the scope: c ← min(150, c + 50 ×
  tick);
- otherwise c decays by 75 per second (in practice it is already 0:
  every unscope and every shot set it to 0).

So from raising the scope with attack2, with τ the time since then:

```math
c(\tau) = \min\!\big(150,\ \max(0,\ 50\,(\tau - 0.3))\big)
```

reaching 50 at τ = 1.3 s and 150 at τ = 3.3 s (the wiki agrees: "charging
starts 0.3 s after zooming; damage starts rising after 1 s of charging;
full at 3.3 s from zooming in"). After an **auto re-zoom** there is no
0.3 s wait: the charge rises from the re-zoom tick (1.4 s after the
shot), so it passes 50 at 2.4 s and is full at 4.4 s after the shot. The
HUD meter shows c / 150. An optional full-charge bell (client option
`tf_sniper_fullcharge_bell`, default off) plays `TFPlayer.ReCharged`. The
shot's base damage:

```math
D = \max(50,\ c)
```

(unscoped c is 0, so D = 50). No distance modifier (factor 1 at every
range: the stock rifle's damage carries no distance flag, SDK), no random
damage spread (Open questions 4). The script's own damage number is not
used.

#### 3.3 Headshots

- A hit on the head hitbox (hitgroup 1, classes.md "Hitboxes") counts as a
  head hit only while the Sniper is scoped (unscoped, hit locations are
  ignored). It is a **headshot crit** if, at the shot, the Sniper's field
  of view is below his default and at least 0.2 s have passed since his
  field of view last changed (the zoom in, or the auto re-zoom). A
  headshot is a crit: damage 3 × D (150 at no charge, 450 at full charge,
  enough to kill any class at any overheal). The charge does not gate it.
- A head hit that is not a crit (too soon after a zoom) deals D.
- An invulnerable (übercharged) victim takes no headshot.
- The Sniper Rifle never rolls random crits (SDK: its crit check fails for
  anything but a qualifying headshot). A crit boost from elsewhere makes
  every hit 3 × D (reference).
- Shots pass through teammates and stop at the first enemy or solid
  (SDK).
- The kill feed uses the kill icon `headshot` for a headshot kill and
  `sniperrifle` otherwise (§9.5).

#### 3.4 Ammo

25 rounds, no clip, no reload (the reload key does nothing): each shot
takes one from the 25; the bolt cycle is part of the 1.5 s interval (its
fire animation, 1.6667 s, is longer: Quirks).

### 4. Grenade Launcher

#### 4.1 Firing and flight

- Every 0.6 s while attack is held, one grenade from the clip of 4.
- **Start**: from the eye (the shooting position) offset 16 forward,
  8 right (mirrored for a left-handed view model) and 6 down; a box of
  half-size 8 is traced from the eye to that point against brushes and
  the grenade starts where it stops. If that trace starts inside a solid,
  nothing is fired.
- **Velocity**: 1200 along the aim's forward, plus 200 along the aim's
  up, plus a random r₁ along the aim's right and r₂ along the aim's up,
  each uniform in [−10, 10] (|v| ≈ 1216.6, the wiki's speed). **Spin**: the
  game draws one angular velocity (600, r, 0) degrees per second, r a
  random integer in [−1200, 1200], about the projectile's own axes
  (x its forward, y its left, z its up; the projectile starts with the
  eye's angles), and **adds it to the body twice** (once when the body
  gets its launch velocity, once more right after), so the projectile
  leaves with **(1200, 2r, 0) degrees/s**: 1200 about its forward, an
  even number in [−2400, 2400] about its left, |ω| from 1200 to about
  2683. (Measured 2026-11-20 with air drag off: exactly (1200, 374, 0);
  with it on, (1195, −1631, 0) one tick after launch.) The
  `grenade_no_spin` attribute (not on stock items) zeroes it.
- It is a **rigid physics body**: gravity is the physics world's
  (sv_gravity, 800), plus the physics engine's **air drag** (§4.4) and
  its bounce and friction on surfaces. The physics body's shape and mass
  come from the projectile model's own collision model, not the 4-unit
  cube (that is the game's box for traces and touches): grenade and
  sticky both 5 kg, linear and angular damping 0, inertia scale 1; the
  grenade's surface material `demoman_grenade`, the sticky's `default`
  (their bounce and friction: Open questions 7).
- **Players**: grenades are not solid to players. Each physics step a ray
  along the grenade's motion over that step looks for players and
  buildings: an enemy it meets is touched (§4.2); a teammate it meets is
  passed through during the first 0.25 s of the grenade's life and
  bounced off after that. A bounce off a player mirrors the velocity about
  the contact normal and keeps 0.45 of it (an enemy: a further × 0.5),
  and the spin becomes −0.5 × itself. The owner is always passed through.
- Touching the sky removes the grenade silently.

#### 4.2 Direct hit, rollers, fuse

- **Direct hit**: if the grenade touches an enemy player or building
  **before it has touched anything physical** (world, props, objects), it
  explodes at once with its **full damage** (100); the struck enemy takes
  factor 1 regardless of distance.
- **Roller**: its first physical contact marks it; after that it never
  explodes on touching a player again (it bounces off them as above).
  Hitting an enemy building physically still makes it explode on the next
  tick, at its reduced damage.
- **Fuse**: a grenade that hasn't exploded explodes on the first fuse
  check after 2.0 s from launch. The fuse is checked every 0.2 s (the
  first check 0.2 s after launch), so with think times on whole ticks the
  explosion comes about 2.1–2.2 s after launch (Open questions 20). A
  fuse explosion uses **60 %** of the grenade's damage (60 at the centre).

#### 4.3 Explosion

- weapons.md §5.2's splash: radius 146, factor f(r) = 1 − 0.5 r / 146,
  with r to the nearer of the enemy's centre and his feet (origin), and a
  clear line from the centre (other players and projectiles don't block
  it).
- Damage to an enemy: G × f(r) with G = 100 for a direct hit, 60 on the
  fuse. **No distance modifier**: M = 1 at every range. Crit: 3 × that
  (300 direct, 180 at the centre on the fuse).
- With the random damage spread on (`tf_damage_disablespread` 0), the
  grenade's variance is a fifth of the usual: at most about ±3 % (the
  SDK's grenade-launcher case; the wiki's "±2 %" is this). With the
  default 1, none.
- Self-damage, knockback: §6.

#### 4.4 Air drag in flight (grenades and stickies)

(Measured 2026-11-20 in the running game, per server tick through
VScript's physics velocity of the body, with `air_density` 0, 2 and 4:
with 0 there is no loss at all and gravity is exactly 800, so all the
loss is the physics engine's air drag; it scales linearly with the
density. The game code sets no drag of its own; the engine computes it
from the body's collision model, so the coefficients below are fitted,
not read.)

- Air density ρ: the physics world's, **2.0** by default (`air_density`,
  a cheat command). Drag coefficient 1 (the default; the models set
  none).
- **Order within a physics step** (dt = one tick, 0.015 s): gravity
  first (v_z −= 800 dt), then the whole velocity vector is scaled by
  (1 − f_lin) (its direction kept). Likewise the angular velocity is
  scaled by (1 − f_ang).
- **Linear**, with v = (v_x, v_y, v_z) the velocity in the body's own
  axes (forward, left, up), units/s:

```math
f_{lin} = \rho\,\big(c_x|v_x| + c_y|v_y| + c_z|v_z|\big)
```

  per tick. A **quadratic** drag (the per-tick fraction grows with
  speed), weighted per body axis (the projected areas of the collision
  model; it changes as the body tumbles).
  - Sticky: c = (1.43, 1.41, 1.45) × 10⁻⁶ per (unit/s) per tick, nearly
    the same on every axis (fit over 184 ticks, 0.8 % rms error). At ρ =
    2: 0.29 % per tick per 1000 units/s moving along an axis, up to 0.50
    % per 1000 units/s moving along a body diagonal; a sticky at 940
    units/s loses 0.27–0.47 % a tick (measured 0.4–0.5 % typical).
  - Grenade: c = (0.75, 1.91, 1.90) × 10⁻⁶ (its forward x is its long
    axis: it drags half as much moving end-first; fit over 86 ticks, 1 %
    rms).
  - Per second (for another tick length): divide c by 0.015.
- **Angular**, with ω = (ω_x, ω_y, ω_z) in degrees/s about the body's
  own axes:

```math
f_{ang} = \rho\,\big(a_x|\omega_x| + a_y|\omega_y| + a_z|\omega_z|\big)
```

  per tick, the whole ω scaled by (1 − f_ang), so its direction in the
  body is kept and the per-axis ratio stays constant through the flight.
  - Sticky: a_x = 7.27 × 10⁻⁷, a_y = 7.36 × 10⁻⁷ per (degree/s) per tick
    (fit over 264 ticks of three launches, 0.02 % rms: the sum of the
    absolute components, not the vector's length). a_z was not measured
    (a launch never spins about z); take 7.3 × 10⁻⁷. At ρ = 2 the
    launch spin (1200, ±1600, 0) loses 0.41 % a tick, (1200, 0, 0) 0.17 %.
  - Grenade: **a_x = 9.45 × 10⁻⁷, a_y = a_z = 3.98 × 10⁻⁷** per
    (degree/s) per tick, the whole ω scaled by (1 − f_ang) as for the
    sticky (fitted 2026-11-21 on ω_x alone, which only drag changes:
    131 ticks of two launches, 0.11 % rms; the perpendicular part fitted
    separately gave the same factor, 9.40 / 4.01 × 10⁻⁷). The grenade's
    body x (its long axis, about which it is launched spinning at 1200)
    has the least inertia, so ω also **precesses in the body**, which is
    why a plain per-axis fit failed (Open questions 22, answered):
    - Its inertia is symmetric about x: I_y = I_z = I_⊥, I_x = 0.288
      I_⊥ ((I_⊥ − I_x) / I_⊥ = 0.712, measured with `air_density` 0:
      ω_x stays exactly 1200.0 for 69 ticks while (ω_y, ω_z) turns
      −12.81° a tick, i.e. 0.712 × ω_x × dt, from +y toward −z).
    - **Order in a tick**: drag first (the f_ang above from the tick's
      starting ω), then the rotation of the rigid body, integrated in
      **n equal sub-steps**, n = 1 + floor(|ω| × dt / 4.775°) with |ω|
      the tick's starting speed (4.775° = 1/12 radian; at dt = 0.015,
      n = 1 + floor(|ω| / 318.3 degrees/s): 4 at 1205, 5 up to 1590,
      6 from 1594, 7 up to 2227, 8 from 2233, measured). Each sub-step
      is a forward (explicit) Euler step of the torque-free rigid body:
      with φ = 0.712 × ω_x × dt / n in radians, ω_y ← ω_y + φ ω_z and
      ω_z ← ω_z − φ ω_y (both from the sub-step's old values); ω_x
      unchanged. So each tick ω_⊥ turns by n × atan(φ) and its length
      grows by (1 + φ²)^(n/2) ≈ 1 + θ² / (2n), θ = nφ: with no drag
      ×1.00626 a tick at n = 4 up to ×1.00313 at n = 8 (measured
      1.00626, 1.00502, 1.00418, 1.00358, 1.00313 at n = 4–8). This
      growth is the engine's integration, not physics; it is part of how
      the real grenade tumbles.
    - Check (forward simulation of this rule from a launch's first
      measured ω over its first second, ρ = 2): within 4–15 degrees/s of
      the measured ω on every tick (launches with |ω| 1384 and 2646).
    - The sticky shows none of this (its ω keeps its direction in the
      body): its inertia is the same about x and y.
- Damping (the engine's other speed loss) is 0 for both models.
- Worked (sticky, ρ = 2, measured): a sticky launched level at about
  1030 units/s lost about 0.4 % a tick on top of gravity: its horizontal
  speed went 1009 → 802 over 59 ticks (0.9 s), 21 % gone.

### 5. Stickybomb Launcher

#### 5.1 Charging and launching

- Pressing attack (clip ≥ 1, interval passed) starts a **charge**; the
  charge sound `Weapon_StickyBombLauncher.ChargeUp` plays and the arms play
  `sb_autofire` (4.0 s). Releasing attack launches one sticky; holding for
  4.0 s launches it by itself at full charge. While attack stays held after
  that forced launch, a new charge begins as soon as the 0.6 s interval
  has passed.
- Launch speed by held time h (SDK, linear):

```math
v_{fwd}(h) = 900 + 1500\,\min(h / 4,\ 1)
```

  along the aim's forward, plus exactly 200 up and the same ±10 random
  jitter, spin, start point and hull check as the grenade (§4.1).
  **The physics engine then caps the body's speed at 2000 units/s, on
  the whole vector's length, direction kept** (measured 2026-11-21,
  VScript physics velocity on the first tick of a stock sticky
  auto-launched at full charge, aimed 10° up: 1984.7 units/s after
  that tick's gravity and drag, which back out to a launch of 2000.4
  along (0, 0.967, 0.255), the direction of the uncapped
  2400 forward + 200 up (2408) vector; its spin, (1196.66, −706.03, 0),
  is the launch (1200, 2r, 0) after exactly one tick of §4.4's drag).
  So every charge above h ≈ 2.91 s (v_fwd ≈ 1990, |v| = 2000 with the
  200 up) leaves at 2000; the angular cap (3600 degrees/s) is never
  reached. The
  charge changes nothing but the speed (no damage effect on the stock
  launcher).
- The next charge can start 0.6 s after the launch. A weapon switch
  cancels a charge; reloading cannot start while charging.
- Physics as the grenade's (rigid body, 4-unit cube, the world's
  gravity); the owner is always passed, teammates for the first 0.25 s.

#### 5.2 Sticking

- On a physical contact with the world (brushes and static props) or a
  dynamic prop, the sticky's motion is switched off at the contact: it
  stops dead and stays there, and it remembers the surface normal for its
  explosion. Moving brush entities (doors, carts, trains) and other
  physics objects only bounce it.
- Players never hold it: it bounces off enemies (velocity mirrored, × 0.45
  × 0.5) and, after 0.25 s, off teammates (× 0.45), with no explosion.
- It never explodes on contact.

#### 5.3 Arming and detonation

- A sticky is **armed** 0.8 s after its launch (age ≥ 0.8 s; the
  detonation check itself asks for an age strictly over the arm time).
  The arm time L is 0.8 plus the launcher's additive `sticky_arm_time`
  attribute: none on the stock launcher; −0.2 on the Quickiebomb Launcher
  (L = 0.6); +0.8 on the Scottish Resistance (L = 1.6). (Powerup mode
  only: × 0.5 with the Haste rune, × 0.75 with King.) If L ≤ 0.5, an
  attack2 while the sticky is unarmed marks it to blow up the moment it
  arms.
- The armed **pulse** (a ring effect) is the client's own, timed from
  when that client first received the sticky: effects.md §11.3 (first
  ring ≈ 1.0 s after the launch for the stock launcher at defaults).
- **attack2** with **any** of the Demoman's weapons out (the launcher, the
  Grenade Launcher, his melee), if he can attack (not stunned, taunting,
  etc.), detonates every armed sticky of his at once. If at least one is
  unarmed, those stay and the deny sound plays (at most once a second)
  instead of the detonate sound `Weapon_StickyBombLauncher.ModeSwitch`;
  the armed ones still go off. attack2 with no stickies out does nothing.
  Holding attack2 detonates each sticky as soon as it arms.
- **Limit 8**: launching a ninth sticky sets the oldest one's fuse to
  "now", armed or not; it explodes at its next fuse check, within 0.2 s.
- When the Demoman dies or changes class, all his stickies vanish without
  exploding (they break apart with gibs).
- **Shot stickies**: a sticky that is **stuck** is destroyed without an
  explosion by an enemy's bullet, buckshot or melee hit (and an enemy
  blast that also ignites); syringes count as bullets (Open questions 16).
  An enemy blast pushes it with 0.15 of its force (bullets 2×, buckshot
  0.75×): if the pushed force exceeds 1500 it comes unstuck, flies, and can
  re-stick to the world only after 1 s. A sticky in flight cannot be shot.

#### 5.4 Explosion damage

- Splash as the grenade (radius 146, f(r) to 0.5 at the edge, r to the
  nearer of the victim's centre and feet).
- Damage to an enemy, with a the sticky's age at detonation and d the
  Demoman-to-victim centre distance at detonation:

```math
D = 120 \times f(r) \times \begin{cases} M_s(d) & a < 5\ \text{s} \\ 1 & a \ge 5\ \text{s} \end{cases}
```

  (146.19 at 0 units, 120 at 512, 63.36 at ≥ 921.6, all at f = 1). Crit
  (decided at launch, carried by the sticky): 360 × f(r), no distance
  modifier.
- **Air-detonation radius**: a sticky detonated while it is not stuck to
  anything uses a smaller radius:

```math
r_{max} = 146 \times \left(0.85 + 0.15\,\min\!\left(\max\!\left(\frac{a - 0.8}{2},\ 0\right),\ 1\right)\right)
```

  (124.1 at arming, 146 from 2.8 s on). Stuck stickies always use 146.
- There is **no damage ramp-down for fresh stickies** and no charge
  effect on damage; only the air radius above (SDK).

### 6. Demoman self-damage and blast jumping

- The Demoman's own grenade or sticky hurts him (never crit-boosted,
  weapons.md §5.3) with **no distance modifier and no random spread**, as
  a separate step after the blast has hurt everyone else: the radius is
  always 146 and the damage is the launcher's **base** damage (100 for
  the Grenade Launcher, 120 for the Stickybomb Launcher), even for a
  grenade exploding on its 60 % fuse. With r to the nearer of his centre
  and his feet: damage = base × f(r) × **0.75**, grounded or airborne
  (SDK). (The wiki's lower ends, 45 for stickies and ≈ 38 for grenades,
  are base × 0.5 × 0.75; its upper ends, 114 and 73, are not explained:
  Open questions 9.)
- **Knockback** on himself, as weapons.md §5.4's formula with **scale 9**
  whether grounded or airborne (the Soldier's own-rocket scales 10 / 5 do
  not apply), from the self-damage amount; a crouched Demoman's hull
  height counts as 55 in the formula (as the Soldier's). Others are pushed
  with the shared scale (6). A **sticky jump**: stickies at the feet, jump
  (or crouch jump), detonate in the air; with several stickies each
  explosion adds its own damage and push in the same tick.

### 7. Flame Thrower

#### 7.1 Firing

- While attack is held with ammo and the Pyro's eyes are not under water:
  if the line from his eye to the muzzle (the weapon's visual end) is
  blocked by something that can't be damaged (a wall he is pressed
  against), nothing is emitted. **The "muzzle" the server tests is a
  fixed point of the eye's frame, not an animated attachment: 40 units
  along the aim, 5 to the right, 0 up** (the flame point's own start
  point, below) — the same standing and crouched and at every pitch
  (measured 2026-11-21, see "Blocked at a wall" below). Otherwise the Pyro emits one **flame
  point** per attack, every 0.02 s nominally, which on 0.015 s ticks is
  every 2 ticks (≈ 33 a second). A press shorter than 0.2 s still emits
  for 0.2 s.
- **Ammo**: each emission adds 14 × 0.02 = 0.28 to an accumulator; whole
  units are taken from the 200 (the remainder is kept, truncated to two
  decimals). At 2 ticks per emission that is ≈ 9.33 ammo a second, ≈ 21.4 s
  for 200.
- **A flame point** starts 40 units forward and 5 right of the eye (along
  the eye's aim); its base velocity is 2450 along the aim deviated by a
  random spread: on each screen axis an offset (u₁ + u₂) × 0.0489 (2.8°
  in radians) with u₁, u₂ uniform in [−0.5, 0.5] (a triangular spread up
  to about 2.8° per axis). Its life is 0.6 s plus a uniform random value in
  [−0.1, 0.1]. At most 30 points live per Pyro.
- **Motion, each tick** (dt = 0.015): the step velocity is the base
  velocity + (0, 0, 50) (the upward drift) + the Pyro's velocity at the
  point's emission projected onto the point's base direction (this part
  never decays for the stock weapon); the point moves by dt × that. Then
  the base velocity's speed is reduced to speed × (1 − 8.5 × dt) (no
  gravity: `flame_gravity` 0). A swept box of half-size 12 is traced
  against the world for each step: on hitting it, the point slides on
  (its velocity loses the component into the surface, it is placed 12
  units off the surface, and the Pyro's velocity part is dropped); a point
  that starts inside a solid, or whose centre is in water, is removed;
  points are removed when their life ends.
- **Reach**: a stationary Pyro's point travels
  2450 × 0.015 × (1 − 0.8725ⁿ) / 0.1275 after n ticks: 285 units at 0.5 s,
  287 at 0.6 s, 288 at 0.7 s, rising ≈ 30 units meanwhile. With the 40-unit
  start and the 12-unit box, flames reach about 340 units ahead of the
  eye (the wiki's "≈ 350"). A simplest port may instead test a cone of
  that length every flame hit interval.
- **Blocked at a wall** (measured 2026-11-21, tools/tf2ref, our own Pyro
  with the stock Flame Thrower forced, against 2fort's flat wall at
  y = −831.97; VScript every tick: the weapon's firing state, the ammo,
  and whether a flame point was made that tick; the player moved 0.25 or
  0.5 units a tick toward or away from the wall while attack is held):
  - The test is a **line** (not a box) from the eye to the point E = eye
    + 40 × forward + 5 × right (eye frame, no up offset). Measured
    thresholds of the eye's distance D to the wall (± 0.5 units, the
    tick's position lag): looking straight at it 40.0–40.5 standing and
    40.0–40.5 crouched (eye 68 / 45 above the feet: no change); pitch
    ±30° 35.0–35.5 (= 40 cos 30° = 34.6 plus the lag; up and down alike,
    so no vertical offset); pitch ±60°, ±85° never blocked down to
    D = 25 (the hull's own limit is 24); turned 40° so the right side
    faces the wall 34.0–34.5 (40 cos 40° + 5 sin 40° = 33.8), the other
    way 27.5–28.0 (30.6 − 3.2 = 27.4). The point does not follow the
    animated gun.
  - **While blocked nothing happens at all**: the weapon's firing state
    stays idle (it never enters its start or firing state), no ammo is
    taken, no point is made. So the client starts no stream system and
    plays no start or loop sound (they follow that state; a third-person
    picture against the wall showed no flame at all). Attack held, the
    weapon starts normally (state "start", the start sound, one ammo
    in the first tick, as from idle) on the first tick the line is clear.
  - **Walking into the wall while firing**: the check is made at each
    emission; the first emission whose line is blocked returns the
    weapon to idle in that tick, as a release does (measured: last point
    and last ammo at D = 40.0, idle on the next emission tick at 39.5);
    no ammo is taken for it.

How every client runs these points and draws the stream on them:
[flames.md](flames.md).

#### 7.2 Damage

- **Hit test**: each tick, for each enemy player (or enemy building) the
  Pyro's flames touch, the points are searched oldest first for one whose
  swept 12-unit box (from its previous to its current position) meets the
  victim's hitboxes; the first such point is the one that hits. Teammates
  are never hurt and nothing happens to them (except that a teammate
  Sniper's arrow is lit). Flames pass through players.
- **Heat** (per victim, per Pyro): a victim already being burned gains
  heat H on every touch: +1, or up to +2 for a point less than 0.02 s old
  (2 − age / 0.02). Every tick H drops by 1.25 (not below 0). A victim
  enters with H = 1 at his first damaging hit and is forgotten when more
  than 0.075 s pass without a damaging hit.
- A touch deals damage only if at least 0.075 s have passed since this
  Pyro's last damaging hit on that victim (Open questions 14). Its damage:

```math
D = \max\!\big(1,\ 13 \times g_{age} \times g_{heat}\big),\qquad g_{age} = \max\!\left(0.5,\ 1 - \frac{age}{L}\right),\qquad g_{heat} = 0.5 + 0.5\,\mathrm{clamp}\!\left(\frac{H - 10}{40},\ 0,\ 1\right)
```

  with age and L the hitting point's age and own life (so 0.5 at half its
  life, ≈ 0.3 s). Range 6.5–13 per hit (at least 1). The heat factor is
  what makes the damage rise while a victim stays in a dense stream
  (Open questions 14 on its rate).
- No distance modifier. Crits: rapid-fire (weapons.md §2.6); a crit hit
  is a full crit, 3 × the hit's damage by the shared rule. Each hit also
  halves the victim's received healing for 2 s (reference).
- **Ignition**: every damaging hit on an enemy (a Pyro included) feeds
  the burn (§7.3).

#### 7.3 Afterburn

- **Duration**: the first hit that ignites a victim gives him 3 s of burn;
  then every damaging flame hit (the first included) adds 0.4 s; the
  total is capped at 10 s. So one hit gives 3.4 s; at a hit every 0.075 s
  the burn grows ≈ 4.33 s per second of contact and reaches 10 s after
  ≈ 1.5 s.
- **Ticks**: a burning player takes 4 damage every 0.5 s, the first
  0.5 s after ignition; each tick also removes 0.5 s from the remaining
  burn. The burn ends as soon as the remaining time is ≤ 0 (checked
  before the tick's damage), or when the victim is waist-deep in water.
  The damage is credited to the Pyro who lit him, with the Flame
  Thrower's kill icon (`flamethrower`).
- Afterburn never crits (it can mini-crit: 5 per tick, reference).
- **Enemy Pyros** still take the direct flame damage; they burn for a
  fixed 0.25 s (the burning look only) and take no afterburn.
- Extinguished also by picking up a health kit, being healed by a Medic
  long enough, and a friendly Pyro's airblast (reference).

#### 7.4 Airblast (reference only)

attack2: costs 20 ammo, refires after 0.75 s and blocks flames for 1.0 s,
a cone ahead of the Pyro that pushes enemies and reflects projectiles
(stickies are pushed, not reflected) and puts out burning teammates;
sound `Weapon_FlameThrower.AirBurstAttack`, arms sequence `ft_alt_fire`
(1.0 s). Not specified further here.

### 8. Syringe Gun

- Every 0.105 s while attack is held, one needle from the clip of 40.
- **Needle**: starts 16 forward, 6 right (mirrored for a left-handed view
  model) and 8 down from the eye; aimed from there at the point where a
  trace from the eye along the aim (up to 2000 units, stopped by any
  solid or player) ends, then its pitch and its yaw each get a random
  offset uniform in [−1.5°, 1.5°]. It flies at 1000 units/s with gravity
  0.3 × sv_gravity (240 units/s²) as a 2-unit cube, no drag.
- Passes through teammates; stops at the first enemy or the world (the
  world removes it at once; the sky removes it silently).
- Damage on hit: 10 × M_s(d) with d the Medic-to-victim centre distance at
  impact (12.18 at 0, 10 at 512, 5.28 at ≥ 921.6). Crit 30 (rapid-fire
  crits). Needles can't be reflected by airblast; they destroy stuck
  stickies (§5.3).
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
no arms counterpart: how it turns, §9.6.)

Projectiles (one bone each; collision boxes from the SDK: grenade and
sticky a 4-unit cube, needle a 2-unit cube):

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
| reload (the owner's view-model events, timed: [viewmodel_sounds.md](viewmodel_sounds.md) §3–4) | Scout `Weapon_Pistol.ReloadScout` → `pistol_reload_scout.wav` at 0.033 s, then `Weapon_Pistol.ClipIn` → `pistol_clipin.wav` at 0.600 s; Engineer only `Weapon_Pistol.ClipIn` at 0.467 s | `Weapon_SMG.ClipOut` → `smg_clip_out.wav`, `Weapon_SMG.ClipIn` → `smg_clip_in.wav` | `Weapon_SniperRifle.BoltBack`, `.BoltForward` (the bolt in the fire animation) | `Weapon_Revolver.Reload1` / `2` / `3` → `revolver_reload_cylinder_open/arm/close.wav` |
| world reload (others hear it at the reload's start) | `Weapon_Pistol.WorldReload` → `pistol_worldreload.wav` for both classes (measured: the Engineer's stock Pistol too); `…WorldReloadEngineer` → `pistol_clipin.wav` exists but stock doesn't use it | `Weapon_SMG.WorldReload` → `smg_worldreload.wav` | `…WorldReload` → `sniper_worldreload.wav` | `…WorldReload` → `revolver_worldreload.wav` |
| draw | `Weapon_Pistol.Draw` → `weapons/draw_secondary.wav`; Engineer `Weapon_Pistol.Draw2` → `draw_pistol_engineer.wav` | `Weapon_SMG.Draw` → `draw_secondary.wav` | `Weapon_SniperRifle.Draw` → `draw_primary.wav` | `Weapon_Revolver.Draw` → `draw_secondary.wav` |

| Use | Grenade Launcher | Stickybomb Launcher | Flame Thrower | Syringe Gun |
|---|---|---|---|---|
| shot | `Weapon_GrenadeLauncher.Single` → `grenade_launcher_shoot.wav` | `Weapon_StickyBombLauncher.Single` → `stickybomblauncher_shoot.wav` | start `Weapon_FlameThrower.FireStart` → `flame_thrower_start.wav` (static channel); loop `.FireLoop` → `flame_thrower_loop.wav` (weapon channel); end `.FireEnd` → `flame_thrower_end.wav` (the stock gun plays `.WindDown`, the same wave and settings; the burst's timing: [effects.md](effects.md) §14.2) | `Weapon_SyringeGun.Single` → `syringegun_shoot.wav` |
| crit | `…SingleCrit` → `grenade_launcher_shoot_crit.wav` | `…SingleCrit` → `stickybomblauncher_shoot_crit.wav` | `.FireLoopCrit` → `flame_thrower_loop_crit.wav` | `…SingleCrit` → `syringegun_shoot_crit.wav` |
| empty | – | – | – | `Weapon_SyringeGun.ClipEmpty` → `pistol/pistol_empty.wav` |
| reload | `Weapon_GrenadeLauncher.DrumOpen` / `.DrumLoad` / `.DrumClose` (→ `grenade_launcher_drum_*.wav`), `.DrumStart` / `.DrumStop` (volume 0.4) | `Weapon_StickyBombLauncher.BoltBack` / `.BoltForward` | – | `Weapon_SyringeGun.Reload1` / `2` / `3` → `syringegun_reload_air1.wav`, `_glass2.wav`, `_air2.wav` |
| world reload | `…WorldReload` → `grenade_launcher_worldreload.wav` | `…WorldReload` → `stickybomblauncher_worldreload.wav` | – | `…WorldReload` → `syringegun_worldreload.wav` |
| draw | `Weapon_GrenadeLauncher.Draw` → `draw_secondary.wav` | (none of its own; Open questions 18) | `Weapon_FlameThrower.Draw` → `draw_primary.wav` | `Weapon_SyringeGun.Draw` → `draw_primary.wav` |
| special | – | charge `Weapon_StickyBombLauncher.ChargeUp` → `stickybomblauncher_charge_up.wav`; detonate `Weapon_StickyBombLauncher.ModeSwitch` → `stickybomblauncher_det.wav` | pilot light `Weapon_FlameThrower.PilotLoop` → `flame_thrower_pilot.wav` (74 dB, volume 0.25; a 12.77 s wave played once at each draw and release, not looped: effects.md §14.2); flame hits `.FireHit` → `flame_thrower_fire_hit.wav`; airblast `.AirBurstAttack` → `flame_thrower_airblast.wav` | – |
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

#### 9.6 The Grenade Launcher's drum (`procedural_chamber`, measured)

Measured 2026-11-21: tools/tf2ref `gl_drum.tf2` (the Demoman's own view,
60 frames a second, two shots then a reload of both) and
`gl_drum_bot.tf2` (a puppet Demoman seen from 35 units, four shots and
the reload), the model files, and the server's view model read every
tick through VScript.

- **The drum** is the 1,998 vertices of
  `c_grenadelauncher.mdl` bound wholly to `procedural_chamber`: a
  cylinder of radius about 5.3 round the bone's own z axis (z 6.4 to
  14.0 in the model's space, the barrel's direction, +z toward the
  muzzle) with **six chambers, 60° apart** (its outline repeats every
  60°). `procedural_chamber` is the child of `weapon_bone_2` with no
  offset and no turn; `weapon_bone_2` (no vertices of its own) sits at
  (0, 0.05, 9.43) in the model's space, on the drum's axis.
- **Code, not animation.** The model has no bone controller and no
  procedural rule for the bone, and the arms' sequences (`g_draw`,
  `g_idle`, `g_fire`, `g_reload_start`, `_loop`, `_end`) keep
  `weapon_bone_2` exactly on `weapon_bone_1` every tick: no sequence
  turns the drum. The turn is the game's own, on top of the animated
  pose, and only on the owner's view model.
- **Per shot, first person:** the drum turns **one chamber, 60°,
  smoothly**, in the direction that makes the chambers on the side
  facing the Demoman rise: **clockwise seen from behind the gun**
  (looking toward the muzzle), and so clockwise as the player sees it
  (the camera is behind and to the left of the gun). It starts in the
  frame after the shot and settles about 8 frames later (0.13–0.15 s
  after the shot): slow at first, fastest about halfway, slow into the
  end. Because the drum repeats every 60°, it looks the same before and
  after each shot, so the turn may be kept as a 0–60° angle that starts
  again at 0 for the next shot.
  *Re-measured 2026-11-21* with the stock gun forced (tf2ref
  `stock tf_weapon_grenadelauncher 19`, so not an unlock from the
  account's loadout), `gl_drum.tf2` otherwise unchanged: at 60 fps the
  shot is frame 9 (muzzle flash) and the drum is still by frame 17–18;
  a second run at `host_framerate 0.004` (250 pictures a second of game
  time) put the shot at frame 16, almost no turn through frame 25
  (36 ms), the chambers visibly sliding up and toward the top strap
  between frames 32 and 46 (the lower chamber rising into the place of
  the middle one, the upper one passing under the strap), and still
  again by frames 48–54 (0.13–0.15 s). The direction is read against
  the back plate's ring, which the kick moves together with the drum.
- **Reload, first person:** no turn (re-checked 2026-11-21, the same
  60 fps run: `+reload` at frame 109, grenades seated about frames 150
  and 188; the chamber mouths seen past the back plate keep their
  places against the frame's rear post from frame 136 to 212).
  Through each grenade's insertion and between grenades the chamber mouths seen past the back plate
  stay where they are (any whole-chamber jump would look the same, so
  none is needed).
- **Draw and idle:** no turn.
- **Third person (what others see):** the drum never turns. In the
  0.2 s after each of a puppet's shots and through its whole reload the
  chambers stay put against the frame; only the owner's view model
  turns its drum.

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
| Grenade, fuse, at the centre (60) | 60 | 60 | 60 | 180 |
| Sticky, age < 5 s (120, M_s) | 146.19 → 146 (wiki 144) | 120 | 63.36 → 63 (wiki 64) | 360 |
| Sticky, age ≥ 5 s | 120 | 120 | 120 | 360 |
| Syringe (10, M_s) | 12.18 → 12 | 10 | 5.28 → 5 | 30 |
| Flame hit (13 × g_age × g_heat, ≥ 1) | 6.5–13 | – (reach ≈ 340) | – | 3 × the hit |
| Afterburn | 4 per 0.5 s | | | never crits |

## Per-tick order

Single-bullet gun (Pistol, SMG, Revolver, Sniper Rifle): weapons.md's
hitscan order, with these changes:

1. Sniper Rifle: advance the zoom state: a due scope drop (with or without
   a pending re-zoom), a due re-zoom, then attack2 (toggle, or cancel a
   pending re-zoom). Then: in a jump → drop the scope, c = 0; else, past
   the attack2 cooldown → scoped and not just after a scoped shot: c +=
   50 × tick (max 150), otherwise c decays.
2. If attack is held, the interval has passed and ammo ≥ 1: crit decision
   (Revolver: roll now; Pistol, SMG: burst or once-a-second roll; Sniper:
   none), remove 1 round, next fire time = now + interval.
3. Direction: dead centre if more than 1.25 s since the last shot
   (Sniper: always centre), else spread s. Record the shot time.
4. Rewind, trace, restore; damage: base × M (Sniper: D, ×3 if a scoped
   head hit ≥ 0.2 s after the last zoom change); round; apply.
5. Sniper: if scoped, schedule the scope drop at +0.5 s (and the re-zoom
   0.9 s after it), block jumping until the drop, c = 0; if unscoped,
   block attack2 for the fire animation's length.

Grenade and sticky launcher:

1. Grenade: attack held, interval passed, clip ≥ 1 → crit roll, launch
   (§4.1). Sticky: attack pressed (or still held) with the interval passed
   → begin charge; on release, or at 4.0 s → crit roll, launch at v(h)
   (§5.1); if that makes 9 live, set the oldest's fuse to now.
2. attack2 with any Demoman weapon out: detonate every armed sticky.
3. Each physics step, per projectile: the physics move (gravity, drag,
   bounces, spin); the player ray along the step (enemy touch → a
   grenade not yet marked explodes at full damage; bounce off enemies and
   off teammates after 0.25 s); a grenade's first physical contact marks
   it; a sticky's contact with the world or a dynamic prop stops it.
4. Every 0.2 s per projectile: fuse check (a grenade past 2.0 s, a sticky
   whose fuse was set to now) → explode.
5. Explosion: as weapons.md §5.2 with the damage of §4.3 / §5.4; then the
   owner's own share (§6); knockback.

Flame Thrower:

1. While attack held with ammo, eyes above water and the muzzle clear:
   every 0.02 s (2 ticks), crit decision, emit one point, add 0.28 ammo
   use.
2. Move points (step velocity, world sweep, drag); drop the expired ones,
   those in water and those that started in a solid.
3. For each enemy touched by a point (oldest point first): add heat; if
   ≥ 0.075 s since the last damaging hit on him, deal D of §7.2 and feed
   his burn (+3 s on ignition, +0.4 s).
4. After all entities: forget victims not hit for over 0.075 s; every
   victim's heat −1.25.
5. Each burning player: if remaining burn ≤ 0 or waist-deep in water →
   end; else every 0.5 s: 4 damage, remaining −0.5 s.

Syringe Gun: as the hitscan gun's steps 2–3 but spawn a needle; each
later tick move it under 0.3 gravity; on contact with an enemy apply
10 × M_s(d) (d at impact) and remove it; on the world remove it.

## Edge cases

- **Grenade direct hit after a bounce**: none: a grenade that has touched
  anything physical explodes only on its fuse (at 60 %), even if it then
  rolls into an enemy; it bounces off him (× 0.225 of its speed).
- **Grenade hitting an enemy building after a bounce**: explodes on the
  next tick at its reduced (60 %) damage.
- **Grenade touching a teammate**: passes through during its first 0.25 s,
  bounces off after; never explodes.
- **Grenade or sticky fired from inside a wall**: if the 8-unit box trace
  from the eye starts in a solid, nothing is launched (the round is still
  used: Open questions 8).
- **Sticky on an enemy**: bounces off; if it then lands on the world it
  sticks.
- **Detonating before arming**: deny sound; those stickies stay; a later
  attack2 (or holding attack2) detonates them once armed.
- **Sticky fired at full charge without releasing**: launched at 4.0 s;
  still holding, the next charge starts 0.6 s later (and fires itself 4 s
  after that).
- **Sticky crit**: decided at launch; a crit sticky detonated 10 s later is
  still a crit (360 × f).
- **Sticky distance for the 5 s rule**: the Demoman's position at
  detonation counts (as the rocket's at impact); a sticky exactly 5.0 s
  old is already flat.
- **Sniper scoped < 0.3 s**: c = 0, D = 50; a head hit before 0.2 s is a
  body hit (50).
- **Sniper unscoped head hit**: 50, no crit.
- **Sniper shot right after the auto re-zoom** (at +1.5 s): scoped for
  0.1 s only, so a head hit is not a crit and the charge is 0.1 × 50 = 5
  → D = 50.
- **Sniper in a jump**: cannot scope; jumping while scoped unscopes and
  empties the charge; after a scoped shot he cannot jump for 0.5 s.
- **Pistol held**: fires at 0.15 s intervals; tapping faster does not fire
  faster (since 2009-08-13).
- **Flame Thrower under water** (eyes under): it emits nothing; points
  entering water die.
- **Flame Thrower against a wall**: nothing is emitted while the eye-to-
  muzzle line is blocked by the world.
- **Afterburn on a Pyro**: 0.25 s of burning look, no afterburn damage;
  direct flame damage still applies.
- **Clip reload interrupted by a weapon switch**: nothing moved (weapons.md
  §3).

## Quirks

- The Sniper Rifle's fire animation (1.6667 s) is longer than its interval
  (1.5 s): the next shot can come before the bolt animation ends. Keep the
  timing.
- After an auto re-zoom the charge restarts with no 0.3 s wait, but the
  headshot's 0.2 s does restart: a shot at the interval's end (+1.5 s)
  can't headshot-crit. Keep.
- `Weapon_SniperRifle.BoltForward` plays `sniper_bolt_back.wav` and
  `.BoltBack` plays `sniper_bolt_forward.wav` (swapped in the data). Use
  the entries as they are.
- The Grenade Launcher animates as a secondary and the Stickybomb Launcher
  as a primary (`anim_slot`), the reverse of their slots.
- The Stickybomb Launcher's charge animation `sb_autofire` is exactly the
  4.0 s charge.
- Pistol, SMG and Revolver need more than 1.25 s for a dead-accurate
  shot, not the pellet weapons' 0.25 s.
- A rolled grenade does 60 % on its fuse, but the Demoman's own share of
  it is still computed from 100 (§6).
- The flame emitter's nominal 0.02 s interval is shorter than a tick, so
  the real rate (and the ammo use) depends on the tick rate: 33.3 points
  a second at 66.7 tick.
- `Weapon_FlameThrower.Fire` and `.WindDown` are copies of `.FireStart`
  and `.FireEnd`.

## Test cases

Spread off, crits off unless said, no random pellet spread unless said,
f = 1 unless said, 66.7 tick (0.015 s).

| # | Setup | Input | Expected |
|---|---|---|---|
| 1 | Pistol, victim at 0 / 512 / 1024 | hit | 22.5 → 23 (or 22, weapons.md OQ 3) / 15 / 8 |
| 2 | SMG, 0 / 512 / 1024 | hit | 12 / 8 / 4 |
| 3 | Revolver, 0 / 256 / 512 / 1024 | hit | 60 / 53.75 → 54 / 40 / 21 |
| 4 | Revolver crit, 1024 | hit | 120 |
| 5 | Pistol, Scout, hold attack 2 s from full | – | 12 shots in 1.65 s (first at 0, every 0.15 s), then the reload; clip 12 again at last shot + interval + 1.005 s; reserve 36 → 24 |
| 6 | Pistol, Engineer, clip 0, reserve 200 | – | clip 12 after 1.035 s, reserve 188 |
| 7 | SMG, clip 3, reserve 1 | reload | after 1.1 s clip 4, reserve 0 |
| 8 | Revolver, last shot 1.26 s ago | fire | dead centre; a second shot 0.5 s later has spread ≤ 0.025 per axis |
| 8b | Revolver, last shot exactly 1.25 s ago | fire | spread applies (strictly more than 1.25 s needed) |
| 9 | Pistol, rapid-fire crit roll succeeds at t | keep firing | every shot from t to t + 2 s deals 45 |
| 10 | Sniper, unscoped, body or head at 2000 units | fire | 50 |
| 11 | Sniper, scoped 0.1 s, head | fire | 50 (headshot not yet allowed; also: no shot until 0.1 s after the zoom) |
| 12 | Sniper, scoped 0.25 s, head | fire | 150 (crit of D = 50) |
| 13 | Sniper, scoped 1.3 / 2.3 / 3.3 / 5.0 s, body | fire | 50 / 100 / 150 / 150 (± one tick's 0.75) |
| 14 | Sniper, scoped 3.3 s, head, 3000 units | fire | 450 |
| 15 | Sniper, scoped 2.3 s, head | fire | 300 |
| 16 | Sniper, scoped, fire at t = 0, attack2 not pressed again, `cl_autorezoom` 1 | – | still scoped until the first tick after +0.5 s, unscoped until the first tick after +1.4 s, then scoped with c rising at once: D = 50 until +2.4 s, 150 from +4.4 s; jump does nothing until +0.5 s |
| 16b | Same, `cl_autorezoom` 0 | – | unscoped at +0.5 s; attack2 works again from +1.5 s |
| 16c | Same as 16, head hit at +1.5 s | fire | 50, not a crit (0.1 s since the re-zoom) |
| 17 | Sniper scoped | hold forward | speed 80; unscoped 300 |
| 17b | Sniper scoped | jump | unscoped, c = 0; attack2 during the jump does not scope |
| 17c | Sniper, unscoped shot | attack2 at +1.0 s / +1.7 s | nothing / scopes |
| 18 | Sniper, 25 rounds | 25 shots | then empty sound; no reload |
| 19 | Sniper, teammate between him and the enemy | fire | the enemy is hit, the teammate isn't |
| 20 | Sniper, many shots with crits on | – | no random crit ever |
| 21 | Grenade fired level from standing on flat ground, jitter off | – | initial velocity (1200, 0, 200) in the aim frame from 16 / 8 / −6 off the eye; flight under gravity 800 (drag: P1) |
| 22 | Grenade direct hit on an enemy, Demoman at 0 / 512 / 1500 | – | 100 / 100 / 100 |
| 23 | Grenade lands, rolls into an enemy at 1.0 s | – | no explosion then (it bounces off him); explodes at the first fuse check after 2.0 s (≈ 2.1–2.2 s) where it is, 60 at the centre |
| 24 | Grenade's fuse ends 73 units from an enemy's centre | – | 60 × 0.75 = 45 |
| 25 | Grenade's fuse ends 146 / 147 units away | – | 30 / 0 |
| 26 | Crit grenade, direct / on the fuse at the centre | – | 300 / 180 |
| 27 | Grenade Launcher clip 0, reserve 16 | – | clip 1 at +1.2 s (wiki 1.24), 4 at +3.0 s; reserve 12 |
| 27b | Grenade passes a teammate at 0.1 s / at 0.5 s of flight | – | passes through / bounces off (× 0.45), no explosion |
| 28 | Sticky, attack pressed and released at once, jitter off | – | forward speed 900, up 200 |
| 29 | Sticky, held 2 s / 4 s | – | 1650 forward + 200 up / 2400 + 200 up, the latter capped to 2000 on the vector's length (§5.1); at 4 s it launches without release; still held, the next charge starts at 4.6 s |
| 29e | Sticky, full charge, aimed 10° up, ρ = 2 | first tick | speed 2000 before the tick (direction of the uncapped vector), 1984.7 after its gravity and drag |
| 29f | Grenade in flight, `air_density` 0, ω (1200, 107.9, −24.5) | one tick | n = 4 sub-steps; ω_x 1200; (ω_y, ω_z) turned −12.81°, length × 1.00626 |
| 29g | Grenade in flight, ρ = 2, ω (1195.0, −2302.3, 521.6) | one tick | drag f = 2 × (9.45e-7 × 1195.0 + 3.98e-7 × 2823.9) = 0.00451, then n = 9 sub-steps of the precession; ω_x → 1189.6 |
| 29b | Sticky launched with r = 187 | – | spin (1200, 374, 0) degrees/s about its forward, left, up |
| 29c | Sticky in flight, ρ = 2, body-frame velocity (1000, 0, 0) / (577, 577, 577), ω (1200, 1600, 0) | one tick | speed × (1 − 0.00286) / × (1 − 0.00495); ω × (1 − 2 × (7.27e-7 × 1200 + 7.36e-7 × 1600)) = × (1 − 0.00410) |
| 29d | Sticky in flight, `air_density` 0 | – | no speed or spin loss; only gravity (800) |
| 30 | Sticky, launch, attack2 at +0.5 s, again at +0.9 s | – | deny sound and nothing at 0.5 s; explodes at 0.9 s |
| 30b | Sticky armed, Demoman holding the Grenade Launcher | attack2 | it explodes |
| 31 | 8 stickies out | launch a 9th | the oldest explodes within 0.2 s (its next fuse check), armed or not; 8 remain |
| 32 | Sticky stuck, age 2 s, Demoman 0 / 256 / 512 / 1024 from the enemy at the centre | detonate | 146 / 133 / 120 / 63 |
| 33 | Same, age 6 s | detonate | 120 / 120 / 120 / 120 |
| 34 | Crit sticky, age 1 s, 73 units from the enemy, Demoman 1500 away | detonate | 360 × 0.75 = 270 |
| 35 | Sticky in the air (not stuck), age 0.8 s, enemy 130 units away | detonate | outside the 124.1 radius: 0 |
| 36 | Same, age 2.8 s | detonate | inside 146: 120 × (1 − 0.5·130/146) × M_s |
| 37 | Sticky hits an enemy in flight | – | bounces off (velocity mirrored, × 0.225), no explosion |
| 38 | Demoman dies with 5 stickies out | – | all 5 vanish, no explosions |
| 39 | Demoman airborne / grounded, own sticky at 0 from his centre or feet | detonate | 120 × 0.75 = 90 either way |
| 39b | Demoman, own grenade explodes on its fuse at his feet | – | 100 × 0.75 = 75 (not 60 × 0.75) |
| 40 | Stickybomb Launcher clip 0, reserve 24 | – | first at +1.09 s, then every 0.67 s; full at 5.78 s; reserve 16 |
| 40b | Stuck sticky shot by an enemy Pistol / an enemy rocket nearby | – | gone, no explosion / pushed (and unstuck only if the push exceeds 1500) |
| 41 | Flame Thrower, enemy at 100 units, continuous | 1.5 s | damaging hits every 0.075 s (5 or 6 ticks: Open questions 14), each max(1, 13 × g_age × g_heat), between 6.5 × g_age and 13 × g_age; ignited |
| 42 | Flame Thrower, stationary, enemy's near side at 360 units | hold | no damage (beyond reach ≈ 340) |
| 43 | Flame Thrower, 200 ammo | hold | ≈ 21.4 s of fire (714 points at 0.28 each, ±1 for the truncated remainder) |
| 44 | Enemy lit by one flame hit, then out of the flame | – | burn 3.4 s: 4 at +0.5, +1.0, …, +3.5 (7 ticks, 28) |
| 45 | Enemy in flames 10 s, then out | – | burn capped at 10 s: 20 more ticks, 80 |
| 46 | Enemy Pyro hit by flames | – | direct damage only, 0.25 s burn look, no afterburn |
| 46b | Pyro facing a flat wall, eye 39.5 / 40.5 units from it (pitch 0, standing or crouched) | hold attack 1 s | nothing: state idle, ammo unchanged, no points, no stream or sound / fires normally |
| 46c | The same at pitch 30° (or −30°), eye 34.5 / 35.5 from the wall; turned 40° with the right side toward the wall, 33.5 / 34.5 | hold | blocked / fires |
| 46d | Firing, walking into the wall | – | the first emission whose eye-to-E line is blocked returns the weapon to idle that tick, no ammo for it; it restarts (start state, start sound) on the first clear tick while attack is held |
| 47 | Burning player steps into water to the waist | – | burn ends |
| 48 | Syringe at 0 / 512 / 1024 (Medic at impact) | hit | 12 / 10 / 5 |
| 49 | Syringe Gun, hold attack, clip 40 | – | 40 needles in 4.095 s, then reload 1.305 s |
| 50 | Syringe fired level, spread off | – | flies at 1000; drops 120 t² units below its line after t seconds (240 / 2) |
| 51 | Any of the eight | switch to it | first attack possible 0.5 s later |
| 52 | Grenade Launcher, own view, drum at rest | one shot | the drum turns 60° clockwise (seen from behind) about `procedural_chamber`'s z axis, starting the frame after the shot, smooth, done 0.13–0.15 s after it; looks as before the shot afterwards |
| 53 | Grenade Launcher, own view | two shots 0.8 s apart | two separate 60° turns, one after each shot |
| 54 | Grenade Launcher, own view, clip 2 of 4 | reload both | no drum turn at any time in the reload |
| 55 | Grenade Launcher, another player watching | shots and a reload | the drum never turns on the world model |

Probe checks (live dedicated server, bots; never ports 27015/27030):

| # | How | Expected |
|---|---|---|
| P1 | Log a grenade's and a sticky's position each tick after launch (uncharged and full charge), and a grenade's bounces on flat ground | the physics drag, bounce and friction (Open questions 7) |
| P2 | Sniper: damage on a bot at scoped times 0.2, 0.3, 1.3, 2.3, 3.3 s; head vs body; a head hit 0.1 s after an auto re-zoom | §3.2–3.3 |
| P3 | Flame Thrower: damage log per hit on a bot at 50/150/250/330 units, from the first hit for 2 s | g_age, g_heat's rise, the hit interval (Open questions 14) |
| P4 | Afterburn duration after 1 hit, 0.5 s, 1.5 s, 10 s of contact | §7.3 |
| P5 | Demoman's own sticky at his feet: grounded vs airborne damage, velocity after | 90 both, scale 9 |
| P6 | Grenade: time from launch to the fuse explosion | Open questions 20 |

## Open questions

1. **Rounding** follows weapons.md Open questions 3 (the wiki's 22 for the
   Pistol's 22.5, 64 for the sticky's 63.36 and 144 for its 146.19
   disagree with half-up).
2. **Pistol, SMG, Revolver spread**: s = 2/N from the wiki's "N:1" uses
   weapons.md's reading of the Scattergun's 30:1; the script's spread
   value would settle it (the SDK has the distribution, §2, but not the
   number).
3. **Sniper zoom**: settled by the SDK (20°, `cl_autorezoom` 1, jump
   rules). Left: the crouched scoped speed (the 80 cap with movement.md's
   crouch scaling) is not measured.
4. **Sniper spread**: assumed none, scoped or not (the script's value);
   with `tf_damage_disablespread` 0 the rifle gets the usual random
   ±damage variance only if its damage carries no other exemption (the
   SDK applies the random variance to every non-self hit; not checked for
   the rifle).
5. **SMG crits**: rapid-fire is a flag in each weapon's script, which the
   SDK doesn't include; assumed rapid-fire (same 0.105 s interval as the
   Minigun).
6. **Holding attack on the Revolver** and attack during a clip reload: not
   settled here; assumed it refires, and that attack with rounds left
   cancels a clip reload as it does a shell reload.
7. **Grenade and sticky physics**: they are rigid bodies in the physics
   engine; the air drag (linear and angular), the surface bounce and the
   friction come from the engine and the collision material, not the game
   code. Fit them from P1.
   **Air drag answered (2026-11-20, tf2-spec-sticky, measured):** §4.4.
   Left: the bounce and friction on surfaces (the `demoman_grenade` and
   `default` surface materials).
8. **Projectile sizes and blocked launches**: the collision boxes are
   settled (grenade and sticky 4-unit cube, needle 2-unit cube); whether a
   launch blocked inside a wall still uses up the round is not checked.
9. **Demoman self-damage**: the SDK gives 0.75 always and base damage × f;
   the wiki's upper ends (114 stickies, 73 grenades) are unexplained (maybe
   several explosions or old values). P5.
10. **Grenade damage variance**: settled: the random-spread variance at a
    fifth of its size (§4.3), only with `tf_damage_disablespread` 0.
11. **Sticky arm time**: settled, 0.8 s (the 2014 notes' value); the
    wiki's 0.7 is out of date.
12. **Sticky detonation**: settled (any weapon out; the ninth sticky's
    oldest at its next 0.2 s fuse check).
13. **Flame points**: settled (spread per axis, the Pyro's velocity along
    the flame only, hit box 12, linear per-tick drag). Left: the client's
    particle look.
14. **Flame damage rate**: the per-victim interval is "at least 0.075 s",
    which on 0.015 s ticks is 5 ticks if the time difference compares
    exactly and 6 if it falls just short in floating point; and the heat
    index gains +1 per touch call, which depends on how many times per
    tick the engine reports the flame touching a victim (once per tick
    would never let the heat rise against its 1.25 per tick drain; twice
    per tick gives +0.75 per tick, full damage after ≈ 1 s of dense
    contact, close to the wiki's 0.9 s). Port: two touches per tick until
    P3 says otherwise.
15. **Afterburn**: settled (3 s + 0.4 s per hit, cap 10). Left: the
    healing and health-kit extinguish rules (reference only here).
16. **Syringe** speed, gravity and spread settled (1000, 0.3, ±1.5°).
    Left: whether a needle counts as a bullet for destroying stickies
    (depends on the missing damage-type table; the wiki says it does).
17. **`ft_idle` vs `df_idle`** share `ACT_PRIMARY_VM_IDLE` with equal weight
    in the Pyro's animation file; how the game picks the stock one
    (activity weight lists, a per-item remap) is unknown; choose by name.
18. **Explosion sound** of grenades and stickies (`Weapon_Grenade_Pipebomb.Explode`
    vs the rocket's `BaseExplosionEffect.Sound`) and the Stickybomb
    Launcher's draw sound: by name only; check on the live game.
19. **Distance classes**: the SDK release lacks the per-weapon damage-type
    table. Grenades having no distance modifier and stickies having one
    only before 5 s are in the code; the reduced near ramp for stickies
    and syringes and the standard one for the Pistol, SMG and Revolver are
    inferred from the wiki's 120 % / 150 % figures matching the code's two
    rules.
20. **Grenade fuse timing**: the fuse is 2.0 s but is checked only every
    0.2 s from launch; with think times rounded to ticks the explosion
    comes at ≈ 2.1–2.2 s (the wiki's 2.3 s may include other delays).
    P6.
21. **The dot's look** (measured 2026-11-20, tools/tf2ref `dot.tf2`: a
    RED Sniper on ctf_2fort's bridge, scoped 25 degrees down at the floor
    about 177 units off, 1024x768, field of view 20): in the Sniper's own
    view the team sprite is a soft red disc about 37 pixels across there,
    which is 2 units wide (the size is the sprite's full width). The core
    (`effects/sniperdot`) is drawn red-orange, not white, over the disc,
    and grows in size with the charge: a speck 0.5 s after the scope came
    up, about half the disc at 1.5 s, the whole disc at 4 s. Others' view
    unmeasured: a puppet bot could not be made to scope (`bot_forceattack2`
    and `bot_mimic` both tried).
22. **The grenade's angular drag** (§4.4): *answered 2026-11-21*
    (tools/tf2ref, VScript: the grenade's physics angular velocity in
    its own axes every tick, stock launcher, `air_density` 2 and 0,
    five launches). The model's axes are its principal axes (x the long
    one, I_x = 0.288 I_y, I_y = I_z); drag a = (9.45, 3.98, 3.98) ×
    10⁻⁷ on the whole ω as for the sticky; the turning of ω in the body
    is the torque-free precession, integrated in 1 + floor(|ω| dt /
    4.775°) forward-Euler sub-steps a tick (§4.4). Left open: the linear
    drag's c was fitted while the body tumbled under this rule; it was
    not refitted.
23. **The physics engine's speed limits**: *answered 2026-11-21*
    (VScript, the first tick of a full-charge stock sticky): it leaves
    at 2000 units/s, the cap applied to the vector's length with its
    direction kept (§5.1). The spin (at most ~2683) is under the angular
    cap (3600 degrees/s).
24. **The drum's turn** (§9.6): *answered 2026-11-21* by re-measuring
    with the stock gun forced: clockwise seen from behind the gun (and
    by the player), 60° a shot, smooth over about 0.13–0.15 s with slow
    ends, no turn on reload. Left open only the exact easing function
    (read from pictures of a kicking view model) and whether a shot
    inside a running turn adds to it (impossible with the stock gun's
    0.6 s refire).
