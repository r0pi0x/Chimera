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
spec derives a formula from published numbers it says so. For the
2026-11-15 check, Valve's published Source SDK 2013 (its 2025 update
with TF2's client and server game code) was read in place, never copied:
the game's damage rules, critical-hit rolls, bullet firing and spread,
the rocket and its explosion, knockback, weapon switching and reloading,
and the Minigun's states. The per-weapon numbers (damage, interval,
spread, clip, range) live in the weapon scripts, which the SDK does not
contain; those stay as before.
Status: checked against the SDK's game code (2026-11-15)

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

## Corrections from the SDK (2026-11-15)

- Distance modifier: spread-on jitter: t = clamp(d/1024 + j, 0, 1) -> the
  random point is uniform over the window [c − 0.1, c + 0.1] cut to
  [0, 1] (no pile-up at the ends); jitter distribution open -> uniform.
- Distance: "centre to centre, assumed" -> confirmed (box centres, at
  least 1 unit).
- Mini-crits: 1.35 × base × max(M, 1) -> 1.35 × (the damage after the
  ramp-up) (same numbers; order fixed); `tf_weapon_criticals_distance_falloff`
  1 brings back only the far fall-off for crits, never the ramp-up.
- Rounding: "nearest, halves up, assumed" -> confirmed: 112.5 -> 113
  (tests 15, 17, 20).
- Crit history window: damage of the last 20 s -> damage dealt between
  2 s and 20 s ago; crit bonus damage excluded; a kill counts only the
  victim's remaining health; blasts within 0.1 s counted once.
- Rapid-fire crit roll: "chance p per second, unknown" -> per-second roll
  chance 1 / (2/p − 2) (1.02 % at p = 2 %, 4.35 % at 8 %).
- Crit bucket: "undocumented" -> rules in 2.6.
- Rocket direction: "along the view or to the aim point?" -> toward the
  point a 2000-unit trace from the eye hits (if farther than 200 units),
  else toward the 2000-unit point.
- Rocket spawn: offset point used as is -> pulled back to where a
  world-only line from the eye to it is blocked.
- Rocket size: "assume a point" -> a point (zero size); explodes 1 unit
  out from the surface along its normal; touching the sky removes it.
- Splash distance: victim's centre -> the nearer of the victim's centre
  and its feet (origin); reach test: nearest point of the victim's box
  within 146 (a victim whose centre is past 146 still takes the edge
  damage).
- Self-damage radius: 146 -> 121 (110 × 1.1), with the launcher's base 90
  whatever the rocket's own damage; applied after everyone else.
- Self-damage worked numbers: 76.75 at 43 from the centre -> by the
  distance to the feet (tests 21–23 rewritten).
- Knockback: "per point unknown" -> speed = min(1000, damage × scale ×
  (48·48·82)/(hull volume)); direction from 10 units below the
  inflictor's centre; crouched self-blast counts a 55-unit-tall hull
  (others' crouch 62); cap 1000 units/s per hit.
- Knockback damage: -> others: the damage after distance and crit;
  self: the damage after × 0.6, before rounding.
- Bullets' knockback: "scale 6, size open" -> same formula, scale 6,
  pushed from the shooter's centre lowered 10; the stock Scattergun has
  no extra knockback.
- Leaving the ground: -> a vertical speed above 250 units/s.
- Blast-jumping state: "launched by his own explosion" -> set by any
  push from his own blast (even standing), cleared on landing.
- Pellet offsets: "distribution open" -> each axis the sum of two
  uniform values in [−0.5, 0.5] (triangular on [−1, 1]).
- First accurate pellet: 0.25 s for all -> 0.25 s for multi-pellet shots
  (Minigun too), 1.25 s for one-bullet shots.
- Fixed spread: 3 × 3 grid, corners 0.5·s -> fixed list of 10: centre,
  ±0.5·s on each axis, ±0.425·s on both axes (the 0.85 diagonals),
  centre; a competitive match forces it on.
- Next fire time: last due time + interval -> the firing tick's time +
  interval (intervals round up to whole ticks: 0.625 s fires every 42
  ticks).
- Minigun first shot: 0.75 s (50 ticks) after the press -> about 0.85 s
  (57 ticks): the gun enters firing at 0.75 s and fires 0.1 s later.
- Minigun wind-down: 1.15 s / 1.4 s -> no gameplay wind-down: the gun is
  idle (jump, speed, re-spin) at once; only switching weapons waits for
  the spool-down animation.
- Minigun released during wind-up: -> the wind-up still completes, then
  it winds down.
- Minigun rev-up damage: 0.5 + 0.5·min(τ, 1) -> 0.5 until τ = 0.2 s,
  then linear to 1 at τ = 1.0 s; τ = the larger of (time since the wind-up
  began − 0.75) and the time firing (test 14: 27 -> 25).
- Minigun rev-up spread: "how much wider, open" -> × 1.5 at τ = 0,
  linear to × 1 at τ = 1 s.
- Minigun crits during the rev-up: "ignore g?" -> crits multiply the
  reduced damage (g applies).
- Deploy: "0.5 s after the switch" -> the later of 0.5 s after the switch
  and the weapon's own pending next-shot time.
- `cl_autoreload` default: open -> 1.
- Reload phase times: "start = sequence length" -> the matching
  first-person sequence's length when the view model has one, else the
  script's start and per-round times (open for the Rocket Launcher).

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
| crit_recent_window | 2 – 20 | s | damage dealt this long ago counts (newer than 2 s doesn't) |
| crit_chance_clamp | 0.01 – 0.99 | – | rapid-fire p is clamped to this before the per-second chance |
| rapid_crit_roll_interval | 1.0 | s | minigun rolls once per second while firing |
| rapid_crit_duration | 2.0 | s | every shot crits for this long after a success |
| crit_bucket_default / cap / bottom | 300 / 1000 / −250 | hp | `tf_weapon_criticals_bucket_*` (2.6) |
| first_pellet_pause | 0.25 | s | multi-pellet shots (Minigun too) put the first pellet dead centre if not fired within this |
| first_bullet_pause | 1.25 | s | same for one-bullet shots (not used by these four) |
| bullet_range | ≈ 56 754 | units | effective maximum trace of a bullet (wiki "Hitscan"; the script's `Range`, 8192 if a script gives none: Open questions 15) |
| deploy | 0.5 | s | weapon switch (draw) time, every stock weapon (since 2015-12-17) |
| holster | 0 | s | stock weapons put away at once |
| rocket_speed | 1100 | units/s | constant, no gravity |
| rocket_offset_stand | (23.5, 12, −3) | units | spawn point from the eye (forward, right, up) |
| rocket_offset_crouch | (23.5, 12, 8) | units | same, crouched |
| rocket_aim_trace | 2000 | units | the rocket heads for what this trace from the eye hits |
| rocket_aim_min | 200 | units | a hit nearer than this (10 % of the trace) is ignored |
| blast_radius | 146 | units | rocket explosion, others |
| blast_radius_self | 121 | units | 110 × 1.1: the rocket's reach and fall-off on its owner |
| blast_edge_fraction | 0.5 | – | damage fraction at the radius edge (Open questions 8) |
| self_damage_air_scale | 0.60 | – | `tf_damagescale_self_soldier`: Soldier's own blast while airborne |
| force_scale_other | 6.0 | – | `tf_damageforcescale_other` |
| force_scale_self_rj | 10.0 | – | `tf_damageforcescale_self_soldier_rj` |
| force_scale_self_badrj | 5.0 | – | `tf_damageforcescale_self_soldier_badrj` |
| force_ref_volume | 48 × 48 × 82 | units³ | the standing hull; knockback × this / the victim's hull volume |
| force_self_crouch_height | 55 | units | a crouched owner's hull height for his own blast's push |
| force_cap | 1000 | units/s | most speed one hit can add |
| force_lower | 10 | units | the push comes from this far below the inflictor's centre |
| lift_speed | 250 | units/s | vertical speed above which a player leaves the ground |
| heavy_knockback_scale | 0.5 | – | the Heavy takes half knockback (not from his own blasts) |
| minigun_wind_up | 0.75 | s | spin-up before the gun counts as spun |
| minigun_fire_step | 0.1 | s | from spun to the first shot |
| minigun_penalty_time | 1.0 | s | damage and spread penalty fade by this spin time |
| minigun_penalty_flat | 0.2 | s | damage stays at the floor until this spin time |
| minigun_damage_floor | 0.5 | – | damage factor at the start of the penalty |
| minigun_spread_ceiling | 1.5 | – | spread factor at spin time 0 |
| heavy_speed / spun | 230 / 110 | units/s | Heavy's top speed, normally / from wind-up start to wind-down |

Console variables (server defaults read from the dedicated server's
`help`):

| Cvar | Default | Effect here |
|---|---|---|
| `tf_damage_disablespread` | 1 | 1: no random distance jitter (§2.3) |
| `tf_damage_range` | 0.5 | R in §2.1 (development-only: not settable on a release server) |
| `tf_use_fixed_weaponspreads` | 0 | 1: multi-pellet weapons fire a fixed pattern (§6.2); a matchmaking match's type overrides it |
| `tf_weapon_criticals` | 1 | random crits on; 0: off (crit boosts still crit) |
| `tf_weapon_criticals_melee` | 1 | melee crits follow `tf_weapon_criticals` (not used here) |
| `tf_weapon_criticals_distance_falloff` | 0 | 0: crits ignore distance |
| `tf_weapon_minicrits_distance_falloff` | 0 | 0: mini-crits ignore fall-off |
| `tf_damagescale_self_soldier` | 0.60 | §5.3 |
| `tf_damageforcescale_other` | 6.0 | §5.4 |
| `tf_damageforcescale_self_soldier_rj` | 10.0 | §5.4 |
| `tf_damageforcescale_self_soldier_badrj` | 5.0 | §5.4 |
| `sv_gravity` | 800 | (rockets ignore it) |

The three `tf_damageforcescale_*` and `tf_damagescale_self_soldier` are
development-only cheats in the game (fixed on a release server). The
crit bucket cvars are cheats. In a matchmaking match the match type, not
`tf_use_fixed_weaponspreads`, decides the pellet pattern: competitive
matches use the fixed pattern, casual and the others random. Chimera's
default: follow the cvar defaults above.

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
| Reload | one shell: first 0.7, then 0.5 each | one rocket: first 0.84, then 0.81 each (measured, §4) | one shell: first 0.87–1.035 by class, then 0.5 (0.51 measured) each | none |
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

The SDK confirms the rule below (it is the game's damage rules, applied
to every hit whose damage type asks for the distance modifier: these
four guns' bullets and the rocket's blast). The game works with the
"closeness" c = 1 − t (1 at point blank, 0 at 1024 and beyond) and a
damage swing of ±R × damage along the smoothstep; written in t it is
exactly the formula of this section. Let t be the normalised distance
(§2.3 says how d becomes t). Then:

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

k = 1.5 for the Scattergun (1.75 at 0; the game picks it by weapon kind,
so it is the Scattergun family only, not the Shotgun), 0.5 for rocket
launchers (1.25 at 0), 1 for the Shotgun and the Minigun. Melee hits
carry no distance modifier at all. Agrees with the SDK.

Damage of one hit before crits: base × M. Distance is measured between
the attacker's and the victim's world-space box centres, never less than
1 unit (confirmed by the SDK). The modifier multiplies the hit's damage
as it arrives (for a blast: after the splash factor of §5.2).

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
  (In the game: the point used is max(c, 0.1) with c = 1 − d/1024
  clamped to [0, 1]; the same thing.)
- **On (0)**: per damage event, t is drawn **uniformly** from the window
  [d/1024 − 0.1, d/1024 + 0.1] cut to [0, 1] (the window shrinks at the
  ends; values are not clamped onto the ends). This makes about ±15 %
  variance. Hits without the distance modifier (melee) use the window
  around t = 0.5, so they also vary by ±14.8 % (M from 0.852 to 1.148).
- Neither applies to damage a player does to himself, nor to fall or
  drowning damage.

#### 2.4 Pellets

Each pellet of a shot is traced separately; all pellets of one shot that
hit the same victim add up to one damage event (one damage number, one
pain sound, one knockback). The distance modifier is the same for every
pellet of that shot on that victim. Agrees with the SDK (the Shotgun
family and the Minigun gather a shot's pellets per victim before
applying them).

Rounding (SDK): damage stays fractional through every multiplier; the
victim's health drops by the final damage rounded to the nearest whole
point, halves up (112.5 → 113). This settles the former open question 3.

#### 2.5 Crits and mini-crits

Order in the game's damage rules: the hit's damage (after splash) →
the distance modifier or random spread (§2.1–2.3) → the crit or mini-crit
bonus, computed on that result → rounding (§2.4).

- **Crit**: damage = 3 × base, with **no** distance modifier (neither ramp-up
  nor fall-off) and no random spread. With
  `tf_weapon_criticals_distance_falloff` 1 a crit takes the far fall-off
  (the M < 1 half) but still never the ramp-up. An exploding crit still
  loses damage with distance from the blast centre (§5.2). Crit sounds and
  effects: the `…Crit` sound entries (§8.3).
- **Mini-crit** (reference only; none of these four mini-crit on their
  own): damage = 1.35 × (base × M if M > 1, else base): fall-off is
  ignored, ramp-up is kept; `tf_weapon_minicrits_distance_falloff` 1
  restores the fall-off. A crit overrides a mini-crit; they don't stack.
- A player's own rocket deals him neither crit damage nor any distance
  modifier: damage to oneself skips that whole step.
- The winning team during the post-round "humiliation" fires only crits;
  a crit-boosted player fires only crits, even with random crits off.

#### 2.6 Random critical hits

- Chance of one roll, ranged weapons:

```math
p = 0.02 \times \left(1 + 3\,\min\!\left(\frac{D}{800}, 1\right)\right)
```

  with D the damage the player dealt that is between 2 s and 20 s old
  (2 % at 0, 3.5 % at 200, 8 % at ≥ 800). Agrees with the SDK. D counts
  each damage event's damage without its crit bonus, a killing hit only
  up to the health the victim had, and blast hits within 0.1 s of each
  other once (the largest). Damage of the last 2 s doesn't count yet.
- **Single-shot weapons** (Scattergun, Shotgun, Rocket Launcher) roll once
  per shot, at the moment of firing: chance p. The whole shot (all
  pellets, the rocket and its explosion) is a crit or not.
- **Rapid-fire weapons** (the Minigun) roll at most once per second while
  firing (the 1 s timer restarts at every roll, won or lost). The roll's
  chance is set so that about a fraction p of firing time is critting:
  with p clamped to [0.01, 0.99],

```math
q = \frac{1}{2/p - 2}
```

  (1.02 % per roll at p = 2 %, 4.35 % at 8 %). A success makes every shot
  fired in the next 2 s a crit (the crit fire sound loop plays instead of
  the normal one).
- `tf_weapon_criticals` 0 turns all random crits off.
- **Bucket** (anti-cheat; cheat cvars `tf_weapon_criticals_bucket_default`
  300, `_cap` 1000, `_bottom` −250): each weapon has a bucket starting at
  300. Every crit roll first adds the shot's base damage (damage ×
  pellets: 60 for the Scattergun, 90 for a rocket, 36 for a Minigun shot;
  added on every shot of the Minigun, also between its rolls) up to 1000.
  When a roll succeeds, the crit costs 3 × the shot's damage × m, m
  growing linearly from 1 to 3 as the weapon's ratio of successful rolls
  to all rolls goes from 0.1 to 1 (1 below 0.1); a rapid-fire weapon pays
  for its whole 2 s burst up front (shot damage × 2 / its fire interval,
  but never more than 1000 / 3 before the × 3). If the cost exceeds the
  bucket the crit is refused; otherwise it is taken (bucket floored at
  −250).
- **Observed rate** (anti-cheat): a winning roll is also refused if, this
  round, the player's random ranged crit damage ÷ 3 as a share of all his
  ranged damage (crit damage counted ÷ 3) exceeds p + 0.1.
- Rolls use a seed from the user command's random seed mixed with the
  weapon and player indices, so client and server agree. Tests use crits
  forced on or off.

#### 2.7 No headshots, no damage to teammates

None of the four weapons has a headshot bonus: all hitboxes ×1. With
friendly fire off (the default), bullets and rockets do nothing to
teammates; rockets fly through them. A bullet's trace, though, is not
told to skip teammates (outside Mann vs. Machine), so a teammate in the
line stops the pellet without being hurt (SDK reading; check live). The
Soldier's own rockets hurt him (§5.3).

### 3. Switching

- Every stock weapon deploys in **0.5 s** (Tough Break update,
  2015-12-17: was 0.67 s): the new weapon's next shot is at the later of
  0.5 s after the switch and the next-shot time it already had (so
  switching back and forth can't fire faster than the weapon's
  interval). The holster is instant. The draw animation plays at 0.67/0.5
  = 1.34 × its speed; its length doesn't set the time (the arms' draw
  sequences are 0.8–1.37 s, §8). Agrees with the SDK.
- The Minigun can't be switched away while it is winding up, spun, firing
  or dry-firing, nor until its spool-down animation has finished after
  it stops (§7).
- Switching away (and deploying) cancels a reload (no ammo moved for the
  shell in progress; shells already in stay).

### 4. Reloading (shared)

The Scattergun, Shotgun and Rocket Launcher reload **one round at a time**:

1. A reload starts when the player presses reload with clip < max and
   reserve > 0, or automatically when the clip reaches 0 with reserve > 0,
   once the fire interval since the last shot has passed (SDK: a reload
   can't begin before the weapon's next-shot time).
2. A **start** phase, then one **insert** per round: at the end of each
   insert, clip + 1, reserve − 1. Inserts repeat until the clip is full or
   the reserve is empty, then a finish animation plays (it does not block
   firing).
3. **Pressing attack with at least one round in the clip interrupts the
   reload at once** (the insert in progress is lost) and fires if the fire
   interval has passed: the next shot is at the later of now and the
   next-shot time from before the reload. With an empty clip, attack is
   ignored and the reload continues. Agrees with the SDK.
4. Client option `cl_autoreload` (default **1**, from the SDK's client):
   when on, a clip weapon not full acts as if reload were held whenever
   neither attack button is held. The empty-clip reload of step 1
   happens either way.
5. Phase times (SDK): the start phase lasts the length of the
   first-person reload-start sequence, each insert the length of the
   reload-loop sequence, when the view model has those sequences;
   otherwise the weapon script's start and per-round times are used.
   Attributes scale them; stock weapons have none. The view model's
   playback rate follows any scaling.

Times (first = start + first insert, as the wiki measures "first reload"):

| Weapon | Start | Insert | First round in | Full from empty |
|---|---|---|---|---|
| Scattergun | 0.2 | 0.5 | 0.7 | 0.2 + 6 × 0.5 = 3.2 |
| Rocket Launcher | 0.5 (measured; see below) | 0.8 (the round counts 0.33 s in) | 0.84 (measured) | 0.5 + 0.33 + 3 × 0.81 ≈ 3.27 (measured) |
| Shotgun, Soldier | 0.4667 | 0.5 | 0.967 (wiki 1.005) | 3.467 |
| Shotgun, Pyro | 0.5 | 0.5 | 1.0 (wiki 1.035) | 3.5 |
| Shotgun, Heavy | 0.3333 | 0.5 | 0.833 (wiki 0.87) | 3.333 |
| Shotgun, Engineer | 0.3333 | 0.5 | 0.833 (wiki 0.87) | 3.333 |

The Shotgun's start equals the length of each class's reload-start
sequence (§8); the wiki's first-reload measurements are those plus
≈ 0.035 s (2–3 ticks of measurement latency), and its 0.51 s per shell is
0.5 plus one tick. The Scattergun's 0.2 likewise equals its start
sequence, consistent with step 5. The Rocket Launcher's start of 0.12 is
only the difference of the wiki's two measurements; its start sequence
is 0.5 s, which by step 5 would give a first rocket at 1.3 s (Open
questions 4). Measured on the live client (2026-11-20, the own view
model's sequence and cycle and the clip read every tick): the start
sequence plays whole (34 ticks), then the loop sequence once per round,
each next loop on the tick the last one's 0.8 s are over (54 ticks
apart), and the clip counts the round 22 ticks (0.33 s) into each loop,
not at its end; the finish sequence follows on the tick after the last
loop is over (55 ticks after it started). So the first rocket is in 0.84 s
after the reload starts and the next every 0.81 s, both phases timed by
the sequences as step 5 says, the round counted early. The Scattergun,
measured the same way: start 14 ticks, a loop every 34 ticks, the round
counted at each loop's end, the finish on that tick.

The Minigun has no clip and never reloads.

### 5. Rocket Launcher: rocket, splash, self-damage, knockback

#### 5.1 Firing and flight

- Fire interval 0.8 s; one rocket per shot from the clip of 4. The crit
  roll is made when firing and carried by the rocket.
- The rocket's intended start is the eye + 23.5 forward + 12 right − 3 up
  (crouched: + 8 up instead of − 3), along the view's own axes, i.e. under
  the weapon on the right (agrees with the SDK). With the player's
  flipped-view-model option the right offset is mirrored to the left; the
  "Original" variant uses 0 right. A line from the eye to that point,
  against world brushes only, decides the actual start: if blocked, the
  rocket starts where it is blocked.
- **Direction (SDK; was open question 5)**: a 2000-unit line is traced
  from the eye along the view (it skips teammates; it stops at the world
  and at enemies). If it hits something more than 200 units away (more
  than 10 % of its length), the rocket flies from its start toward that
  hit point; otherwise toward the trace's 2000-unit end point. So the
  rocket meets the crosshair at the aimed surface (or 2000 units out) and
  not 12 units to its right.
- It flies in a straight line at 1100 units/s, no gravity, no drag, not
  slowed by water. It is a **point** (zero-size box) for collision. It
  touches the world, enemy players and enemy buildings; it passes through
  teammates, its owner, other projectiles, dropped weapons and triggers.
- On touch it explodes 1 unit out from the contact point along the
  surface normal. Touching the sky removes it without an explosion.
  Inside a no-grenades zone it fizzles (removed, no damage).
- (Client) the rocket model isn't drawn for its first 0.2 s.

#### 5.2 Explosion damage

- **Who is reached**: every damageable entity (players, buildings) whose
  **box's nearest point** is within 146 units of the explosion point,
  except the owner, who is handled after everyone else (§5.3).
- **Line of sight**: a line from the explosion point to the entity's aim
  point (its body centre) must not be blocked; players and projectiles
  don't block it (only the world and other solids do).
- **Fall-off distance r**: 0 for the entity the rocket touched (full
  damage); for any other player the **smaller** of the distances from
  the explosion point to the player's centre and to the player's origin
  (his feet); for buildings, to where the line of sight met them.
- Splash factor, linear from 1 at r = 0 to the edge factor e at r = 146,
  **clamped** there (an entity reached by its box but with r > 146 takes
  the edge damage):

```math
f(r) = 1 - (1 - e)\,\frac{\min(r, 146)}{146}
```

  e = 0.5 for the rocket. (SDK: e is 0.5 when the damage type carries
  the game's "half fall-off" flag, otherwise damage ÷ radius = 90/146 ≈
  0.616; the table of weapon damage types is not in the published SDK,
  and the wiki's 50 % says the rocket carries the flag: Open questions 8.)
- Damage to an enemy: 90 × f(r) × M(attacker→victim distance, k = 0.5);
  crit: 270 × f(r). Teammates take nothing (friendly fire off).
- The wiki's "1 % per 2.88 units" (≈ 144 units for 50 %) is consistent
  with measuring to the nearer of centre and feet.

#### 5.3 Self-damage

- After the others, the owner is checked with a **smaller radius, 121
  units** (110 × 1.1), and the launcher's own base damage (90) whatever
  the rocket's damage was (so a launcher with modified damage or radius
  rocket-jumps like the stock one). He is reached if his box's nearest
  point is within 121 units; r is the smaller of the distances to his
  centre and his feet, as in §5.2 (0 if the rocket touched him), and

```math
f_{self}(r) = 1 - (1 - e)\,\frac{\min(r, 121)}{121}
```

- His damage is 90 × f_self(r): **no distance modifier, no random
  spread, never crit** (§2.5).
- If the Soldier is **airborne** (not on the ground, not in water) when
  the blast hits him, it is × 0.60 (`tf_damagescale_self_soldier`), even
  if the rocket also hit an enemy. If he survives, a "rocket jump" game
  event fires.
- Worked: a rocket at the feet of a Soldier standing on the floor (r ≈ 1,
  the explosion being 1 unit above the floor) takes 90 × (1 − 0.5/121) =
  89.63 → 90; airborne, the same is 53.8 → 54. The wiki's ranges (27–89,
  "rocket jump" 27–46) were measured, and are not all reproduced by this;
  P2/P3 check.

#### 5.4 Knockback and blast jumping

From the SDK (settles the former open question 9). Every damage event a
player takes (bullets as well as blasts; not while the game forbids
movement, e.g. pre-round) adds a velocity:

```math
\Delta v = \min\!\left(1000,\; D \cdot s \cdot \frac{48 \cdot 48 \cdot 82}{V}\right) \cdot \hat{u}
```

- **Direction û**: from the point 10 units below the inflictor's centre
  to the victim's centre (unit vector). The inflictor is the rocket (at
  its explosion point) for a blast and the shooter himself for bullets.
  Blasts below or level with a victim thus push slightly upward.
- **V**, the victim's hull volume: 48 × 48 × 82 standing (factor 1),
  48 × 48 × 62 crouched (× 1.3226). For the owner's **own** blast a
  crouched hull counts as 48 × 48 × 55 (× 1.4909): crouching gives a
  markedly longer rocket jump.
- **D**: for others, the damage after the distance modifier and any crit
  (unrounded; a crit pushes 3 ×); for the owner's own blast, the damage
  after the × 0.6 airborne scale (unrounded).
- **s**: others' blasts and bullets `tf_damageforcescale_other` 6; the
  Soldier's own blast `tf_damageforcescale_self_soldier_rj` 10 when he
  is airborne, `tf_damageforcescale_self_soldier_badrj` 5 when on the
  ground. A Soldier's own non-blast damage pushes nothing.
- **Heavy**: × 0.5 on any push from someone else.
- **Cap**: 1000 units/s per damage event (on the magnitude, before the
  Heavy's half).
- The velocity is simply added. A vertical speed above 250 units/s takes
  the player off the ground at his next movement (the movement spec's
  rule).
- Bullets: one push per victim per shot (the summed pellets). The stock
  Scattergun has no extra knockback (that belongs to other scatterguns'
  attributes).
- **Blast jumping**: any push from the Soldier's own blast (even standing)
  sets his "blast jumping" state; it is cleared when he is on the ground
  again. While airborne his own rockets hurt him × 0.6 (§5.3). The push
  also resets his count of air crouches.

Worked: an enemy rocket hitting a standing Soldier directly at 512 units
(90 damage): 540 units/s; a crit (270): 1620 → capped 1000; on a Heavy:
270 (crit: 500). A Soldier's own rocket at his feet, airborne (D = 53.8):
538 units/s standing, 802 crouched; on the ground (D = 89.6, s = 5): 448.

### 6. Bullets and spread (Scattergun, Shotgun, Minigun)

#### 6.1 The shot

- Every pellet is a trace from the eye along its own direction, up to
  the weapon's range (Open questions 15), against players' hitboxes
  (bullet hitboxes, not the bounding box) and the world, with lag
  compensation. No penetration: the first player or solid stops it.
  The base direction is the view plus the view punch (§6.3).
- **First pellet**: if the weapon has not fired for more than 0.25 s
  (shots of more than one pellet, the Minigun's included) or more than
  1.25 s (one-bullet shots), the first pellet of the shot flies exactly
  along the crosshair; the rest scatter. (For the Minigun this concerns
  only the first shot after a pause.) "Fired" is the time of the last
  shot of this weapon. Agrees with the SDK.
- **Random spread** (`tf_use_fixed_weaponspreads` 0): every other pellet
  gets direction normalise(forward + a·s·right + b·s·up), with **a and b
  each the sum of two independent uniform numbers in [−0.5, 0.5]**
  (triangular on [−1, 1], most pellets near the centre). The random
  numbers are reseeded per pellet from the user command's random seed
  (0–255) plus the pellet's index, so client and server agree.
  s = 0.0675 (Scattergun, Shotgun: widest ≈ 3.9°), 0.08 (Minigun: ≈ 4.6°;
  wider while revving, §7.2). The wiki writes these as "30:1" and "25:1",
  which are twice 1/s.

#### 6.2 Fixed spread (`tf_use_fixed_weaponspreads` 1)

- Applies to shots of more than one pellet whose damage type is
  buckshot (Scattergun, Shotgun). Pellet i (from 0) takes (a, b) from
  this fixed list, scaled by 0.5 (so the offsets on each axis are
  0.5·a·s and 0.5·b·s), wrapping past the tenth:

| Pellet | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---|---|---|---|---|---|---|---|---|---|
| a | 0 | 1 | −1 | 0 | 0 | 0.85 | 0.85 | −0.85 | −0.85 | 0 |
| b | 0 | 0 | 0 | −1 | 1 | −0.85 | 0.85 | −0.85 | 0.85 | 0 |

  So 2 pellets in the centre (the first and the last), 4 at ±0.5·s on
  one axis, 4 at ±0.425·s on both. No randomness, no first-pellet rule
  (pellet 1 is central anyway). The pattern is the same every shot.
- Whether the Minigun's bullets count as buckshot is in the missing
  damage-type table: Open questions 10.

#### 6.3 View punch

- After each shot the weapon's script value "punch angle", if above 0,
  kicks the view pitch up by a random whole number of degrees in
  [punch − 1, punch + 1]; the next shot's direction includes the punch.
  The stock values are in the encrypted scripts; the wiki's "no recoil"
  implies 0 for these four (Open questions 15).

### 7. Minigun

#### 7.1 States

From the SDK (the game has five states; there is no "winding down"
state):

| State | Enters when | Lasts | Movement | Sounds |
|---|---|---|---|---|
| idle | – | – | 230 | wind-down sound while the barrel still turns, else none |
| winding up | attack or attack2 pressed in idle | 0.75 s, then spun or firing by the button held | 110 | `Weapon_Minigun.WindUp` |
| spun (not firing) | wind-up done with attack2 (not attack) held; or attack2 while firing | while held | 110 | `Weapon_Minigun.Spin` loop |
| firing | wind-up done with attack held; or attack while spun, with ammo | while held | 110 | `Weapon_Minigun.Fire` loop (`…FireCrit` while the current shot is a crit) |
| dry | firing or spun-with-attack at ammo 0 | while held | 110 | `Weapon_Minigun.ClipEmpty` loop |

- **Wind-up**: a press in idle starts it and sets the next attack time
  0.75 s ahead. At that time the gun enters spun or firing and its next
  attack time is set **0.1 s** later; the first shot is fired then. So a
  held attack from idle fires about **0.85 s** after the press (tick 57 at
  0.015 s), then every fire interval (0.105 s). Letting go during the
  wind-up does not cancel it: the wind-up completes, then the gun stops.
- Switching between attack and attack2 while spun goes straight between
  spun and firing, no wind-up (spun → firing fires at the next tick the
  pending next-attack time allows; firing → spun waits 0.1 s).
- **Stopping**: when neither button is held, the gun goes straight to
  idle once its idle time has passed: 0.2 s after the last shot when it
  was firing, at once when it was only spun. There is **no gameplay
  wind-down**: in idle the Heavy has his normal speed and can jump, and a
  new press winds up again at once (0.75 s). The wind-down sound and the
  spool-down animation (`m_spool_down`, 1.33 s) are cosmetic, except
  that switching weapons waits until that animation ends.
- While in any state but idle the Heavy **cannot jump** (the jump button
  is dropped from his commands) and **cannot switch weapons**. His speed
  cap of 110 comes from an "aiming" condition added when the wind-up
  starts and removed when the gun goes idle. (Crouched, his speed is the
  movement spec's crouch rule applied to that cap; the former "speed 0
  crouched" isn't in the weapon code: Open questions 16.)
- Spinning without firing uses no ammo. Firing uses 1 ammo per shot.
- Dry: with ammo 0 the gun keeps spinning with the empty loop while
  attack is held, firing nothing; ammo arriving puts it back to firing.

#### 7.2 Firing

- A shot every 0.105 s while firing: 4 bullets, 1 ammo. 200 ammo last
  21 s of continuous fire (200 shots, 800 bullets).
- Damage per bullet 9 × M (k = 1), spread s = 0.08.
- **Rev-up penalty** (since 2017-10-20; exact rule from the SDK): the
  spin time at a shot is

```math
\tau = \max(T_{wind} - 0.75,\ T_{fire})
```

  with T_wind the time since the current wind-up began and T_fire the
  time since the first shot of the current firing run (0 at that first
  shot). T_wind counts from the press in idle; after a dry spell it
  counts as 0 until the next wind-up from idle. T_fire restarts when the
  gun goes to spun (attack2 only) or dry. Each bullet's damage is
  scaled by g and the spread s by h:

```math
g(\tau) = \begin{cases} 0.5 & \tau \le 0.2 \\ 0.5 + 0.5\,\frac{\tau - 0.2}{0.8} & 0.2 < \tau < 1 \\ 1 & \tau \ge 1 \end{cases}
\qquad
h(\tau) = 1.5 - 0.5\,\min(\tau, 1)
```

  So the first shot after a held press (τ ≈ 0.1) does half damage with
  1.45 × spread; spinning on attack2 for at least 1.75 s from the press
  makes the first shot full. Before 2017-10-20 (from 2014-06-18), the
  second counted from the start of firing.
- Crits: rapid-fire rules (§2.6). g is applied to the bullet's base
  damage before the crit, so a crit during the penalty is 3 × 9 × g
  (settles open question 11).

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
±18.57 × ±3.71 × ±3.72 (visual; it collides as a point, §5.1).

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
(§8.3) and the reload start time (§4); damage, spread, interval, clip and
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
3. Remove 1 ammo; play the fire sequence and the (crit) shot sound.
   (SDK: the next fire time is set from the time of **this** tick, now +
   interval, after the shot, not from the previous due time; held fire
   therefore repeats every interval rounded up to whole ticks: 0.625 s →
   42 ticks (0.63 s), 0.8 s → 54 ticks (0.81 s), 0.105 s → 7 ticks.
   This corrects the former "last fire time + interval".)
4. Bullet damage per pellet = the script damage (Minigun: × g(τ), and
   the spread × h(τ), §7.2). Note whether the last shot was more than
   0.25 s ago (dead-centre first pellet).
5. Rewind other players (lag compensation).
6. For each pellet: its direction (§6), trace, record the hit victim.
7. Per victim: sum the pellets' damage; distance modifier (or none for a
   crit) and then the crit / mini-crit bonus (§2.5); round halves up;
   apply as one damage event; its knockback (§5.4) from the summed,
   modified damage; add the dealt damage to the attacker's crit history.
8. Restore players. Record now as the last shot time. If clip is now 0
   and reserve > 0, the reload starts once the interval has passed.

Rocket Launcher:

1–3 as above (the rocket carries the crit decision).
4. Trace 2000 units from the eye for the aim point; compute the start
   from the offset (§5.1), pulled back to a world hit between the eye and
   it; spawn the rocket there with velocity 1100 toward the aim point.
5. Each later tick: move the rocket; on contact with the world or an
   enemy, explode 1 unit out from the contact point: for every entity
   but the owner whose box is within 146 units and visible from the
   point, damage (§5.2, the touched one r = 0) and knockback (§5.4); then
   the owner within 121 units (§5.3) and his push.

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
- **Rocket at the owner's feet in the air**: r is the smaller of the
  distances to his centre and his feet, so a rocket at the feet does
  nearly full self-damage (× 0.6 airborne) and nearly full push; the
  crouched hull counts 55 tall for the push.
- **A victim reached by its box but with r past the radius** (a blast
  beside a tall box): takes the edge damage (f clamped at the edge).
- **Spread on, distance near 0 or past 1024**: the random window is cut
  to [0, 1], so random spread can't push the modifier above 1.5 or below
  0.5 (before the per-weapon k); near the ends the draws are uniform on
  the shorter window.
- **Rocket fired into a wall at point blank**: the start point is pulled
  back to the wall; the aim trace hit nearer than 200 units is ignored,
  so the rocket heads for the 2000-unit point and explodes at once.
- **Minigun tapped**: a press shorter than 0.75 s still costs the full
  wind-up (110 speed, no jump) before the gun goes idle.

## Quirks

- The 52.8 % "maximum fall-off" with spread off (not 50 %) comes from the
  921.6-unit clamp. Keep it.
- The Minigun wind-up animation (1.0 s) is longer than the wind-up (0.75 s);
  the first shot (≈ 0.85 s) comes before the animation ends. Keep the
  timing, not the animation length.
- The Minigun's wind-down is only sound and animation; the Heavy is free
  (speed, jump, re-spin) at once, but can't switch weapons until the
  spool-down animation ends. Keep.
- The crit chance multiplier is stored as a byte that is written on a 1–6
  scale and read on a 1–4 scale; the result is exactly the 1 + 3·x of §2.6.
  Keep the formula, not the encoding.
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
| 13 | Minigun, first shot of a held press (τ ≈ 0.1), 512 | 4 hit | 36 × 0.5 = 18 (spread × 1.45) |
| 14 | Minigun, τ = 0.5 s, 512 | 4 hit | 36 × 0.6875 = 24.75 → 25 (spread × 1.25) |
| 15 | Rocket direct hit, Soldier 0 / 512 / 1024 from victim at impact | – | 112.5 → 113 / 90 / 47.52 → 48 |
| 16 | Rocket, victim 256 away | direct | 90 × 1.171875 = 105.47 → 105 |
| 17 | Rocket, Soldier 512 away, explosion level with the victim's centre, 73 units away (feet farther) | splash | 90 × 0.75 = 67.5 → 68 |
| 18 | Rocket, explosion level with the victim's centre, 146 away, Soldier 512 away | – | 45 |
| 19 | Rocket, explosion level with a standing victim's centre, 160 away horizontally (box face 136 away), Soldier 512 away | – | 45 (reached by the box, f clamped at the edge); at 171 (face 147 away): 0 |
| 20 | Rocket crit, explosion level with the victim's centre, 73 away, Soldier 1500 away | – | 270 × 0.75 = 202.5 → 203 |
| 21 | Soldier on the ground, his rocket explodes on the floor beside his feet, 43 units from his origin (farther from his centre) | – | 90 × (1 − 0.5·43/121) = 74.01 → 74 |
| 22 | Same distances, airborne | – | 74.01 × 0.6 = 44.4 → 44 |
| 23 | Same as 21, crit rocket | – | 74 (self never gets crit damage) |
| 24 | Rocket fired standing, eye at (0,0,64), view along +X, nothing within 2000 | – | rocket spawns at (23.5, −12, 61) (right = −Y), moving at 1100 toward (2000, 0, 64): direction ∝ (1976.5, 12, 3) |
| 25 | Rocket fired with the eye 1100 units from a wall, view square to it | – | travels 1076.5 units: explodes 0.979 s later (on the 66th tick) |
| 26 | Teammate between Soldier and wall | fire | the rocket passes through the teammate, no damage |
| 27 | Scattergun clip 0, reserve 32 | – | reload starts by itself; clip 1 at +0.7 s, 2 at +1.2, …, 6 at +3.2; reserve 26 |
| 28 | Scattergun clip 2, reloading, third shell 0.3 s in | press attack | the reload stops at once, a shot fires (clip 1); shell lost |
| 29 | Scattergun clip 0 reloading, first shell 0.1 s in | press attack | ignored; clip 1 at 0.7 s |
| 30 | Rocket Launcher clip 0, reserve 20 | – | clip 1 at +0.84, 4 at +3.27 (measured, §4); reserve 16 |
| 31 | Shotgun as Soldier / Pyro / Heavy / Engineer, clip 0 | – | first shell at 0.967 / 1.0 / 0.8333 / 0.8333 s, then every 0.5 s |
| 32 | Any of the four | switch to it | first shot possible 0.5 s later (34 ticks) |
| 33 | Minigun idle | press and hold attack | spun at +0.75 s (50 ticks); first shot 0.1 s later, ≈ +0.855 s (57 ticks); then every 0.105 s (7 ticks) |
| 34 | Minigun, 200 ammo | hold attack 30 s | 200 shots, then the empty loop; spun the whole time |
| 35 | Minigun spinning (attack2) | press jump | no jump; release attack2 → idle at once, a jump the next tick works, speed back to 230 |
| 36 | Minigun firing | press weapon switch | refused; after release, refused until the spool-down animation ends (≈ 1.33 s), then allowed |
| 37 | Heavy, minigun spun, standing | hold forward | speed 110 (crouched: the movement spec's crouch rule, Open questions 16) |
| 38 | Scattergun, last shot > 0.25 s ago | fire with random spread | pellet 1 exactly on the crosshair; others within ±0.0675 tangent on each axis, denser toward the centre (triangular) |
| 39 | Scattergun, `tf_use_fixed_weaponspreads` 1 | fire twice | identical pattern: tangents (0,0), (±0.03375, 0), (0, ±0.03375), (±0.0286875, ±0.0286875), (0,0) in pellet order (§6.2) |
| 40 | Crit chance, 0 / 200 / 800 / 2000 damage dealt 2–20 s ago | – | 2 % / 3.5 % / 8 % / 8 %; 800 dealt 1 s ago: 2 % |
| 41 | Minigun spun > 1 s, crit roll succeeds at t | keep firing | every shot from t to t + 2 s is a crit (27 per bullet, no distance modifier), the crit fire loop plays; next roll at t + 1 s at the earliest |
| 42 | `tf_damage_disablespread` 0, Scattergun at 512, 1000 shots | – | damages spread over ≈ 60 × [0.85, 1.15] (t in [0.4, 0.6]: M₀ from 1.148 to 0.852; with k = 1.5 near: up to 1.222) |
| 43 | Heavy hit by a rocket | – | half the knockback a Soldier would get from the same damage |
| 44 | Enemy rocket direct hit on a standing Soldier at 512 | – | 90 damage; +540 units/s away from the point 10 below the rocket's centre; crit: 270 damage, push capped at 1000 |
| 45 | Soldier airborne, own rocket at his feet (r ≈ 1) | – | 89.63 × 0.6 = 53.8 → 54 damage; push 538 units/s standing, 802 crouched; blast-jumping state set |
| 46 | Soldier on the ground, own rocket at his feet (r ≈ 1) | – | 90 damage; push 448 units/s (scale 5); leaves the ground if its vertical part exceeds 250 |
| 47 | Scattergun on a standing enemy at 512, all 10 hit | – | 60 damage, push 360 units/s from the shooter's centre lowered 10 |
| 48 | Rapid-fire crit roll, p = 2 % / 8 % | – | per-second chance 1/98 ≈ 1.02 % / 1/23 ≈ 4.35 % |
| 49 | Minigun spun on attack2 for 2 s from the press, then attack | – | first shot full damage (36 at 512) and spread 0.08 |
| 50 | Rocket aimed at a wall 150 units ahead (aim trace hit nearer than 200) | – | flies toward the 2000-unit point along the view, not to the wall's crosshair point |
| 51 | Rocket Launcher fired at t, switched away at t + 0.1, back at t + 0.2 | – | next shot at t + 0.8 (its pending time), not t + 0.7 |

Probe checks (live dedicated server, bots; never port 27015/27030):

| # | How | Expected |
|---|---|---|
| P1 | Scattergun on a bot at measured 0/256/512/768/1024/1500, spread off, crits off | tests 1–6 (rounding of .5 and the curve's middle points) |
| P2 | Rocket on a bot standing still at known offsets from the impact point | f linear to 0.5 at 146, measured to the nearer of centre and feet (the edge factor: Open questions 8) |
| P3 | Soldier's own rocket at his feet: standing, crouched, airborne; read velocity and health right after | tests 45–46 (121-unit self radius, push formula of §5.4) |
| P4 | Log first-shell times of each reload | §4 table |
| P5 | Minigun: ticks from press to first shot; damage of shots over the first second | ≈ 57 ticks; g(τ) of §7.2 |
| P6 | Spawn a rocket and log its first position | the offset of §5.1 and its direction |

## Open questions

1. **Random distributions.** Settled by the SDK: the distance jitter is
   uniform over its (cut) window; pellet offsets are each the sum of two
   uniform halves (§6.1). Exact values depend on the game's random
   generator and the command seed; tests check ranges and seeding, not
   exact values.
2. **Distance points.** Settled: box centre to box centre (§2.1).
3. **Rounding.** Settled: the final damage rounds to nearest, halves up
   (§2.4). The wiki's 112 for 112.5 is a table error.
4. **Rocket Launcher reload start.** The SDK times each reload phase by
   the matching first-person sequence when the view model has it, else by
   the script's times (§4 step 5). Its start sequence is 0.5 s (first
   rocket at 1.3 s) but the wiki measures 0.92 s; either the script value
   is used (the weapon's own model may lack the sequence) or the
   measurement is off. P4. Answered by measurement (2026-11-20, §4):
   the sequences time it (0.5 s start, 0.8 s a round), but each rocket
   counts 0.33 s into its loop, so the first is in at 0.84 s.
5. **Rocket direction.** Settled: toward the 2000-unit aim trace's hit
   (§5.1).
6. **Crit bucket.** Settled (§2.6).
7. **`cl_autoreload` default.** Settled: 1.
8. **Splash edge factor.** The distance measure is settled (nearer of
   centre and feet; reach by the box). The edge factor is 0.5 only if the
   rocket's damage type carries the "half fall-off" flag, else 90/146 ≈
   0.616 (55.5 at the edge); the per-weapon damage-type table is not in
   the published SDK. The wiki's 50 % says 0.5. P2.
9. **Knockback.** Settled (§5.4).
10. **Fixed spread.** The pattern is settled (§6.2). Whether the Minigun
    uses it depends on whether its damage type includes buckshot (the
    missing damage-type table).
11. **Minigun rev-up.** Settled (§7.2).
12. **Rocket collision size.** Settled: a point against the world and
    enemies' collision boxes. Whether players present a larger box to
    projectiles than their movement hull is not answered by the code read.
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
15. **Script numbers.** Damage, interval, pellets, spread, clip, range,
    punch angle and the reload times of these four live in the weapon
    scripts, which the SDK doesn't contain; the values here are the
    wiki's. The SDK's default range when a script gives none is 8192
    units; the wiki's ≈ 56 754 is unconfirmed. "No recoil" means a punch
    angle of 0; unconfirmed.
16. **Spun and crouched.** The weapon code only caps the Heavy at 110
    while spun; the former "speed 0 when crouched" is not in it. Check
    the movement spec and the live game.
17. **Teammates and bullets.** The bullet trace does not skip teammates
    outside Mann vs. Machine, so a teammate should absorb a pellet
    (§2.7); confirm on the live game.
18. **Measured on the live client (2026-11-19, a listen server on
    ctf_2fort, the own player, every frame at 60 fps).** Reloading step 4
    (`cl_autoreload` 1) holds: one Scattergun shot, attack let go, and the
    reload's start shows about 0.7 s after the shot (the 0.625 s interval,
    then the next tick), one round in and the finish; back at the idle
    pose about 1.73 s after the shot. The view kick (`PunchAngle` P): the
    Scattergun's punch pitch was -4, -4, -4, -2, -4 degrees over five
    shots (a whole number from -(P+1) to -(P-1), P 3), held for the shot's
    tick and the next, then on the punch spring (-3.942 on the third
    tick).
