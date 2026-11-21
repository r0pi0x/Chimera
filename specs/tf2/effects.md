# Team Fortress 2: weapon effects (muzzle flashes, tracers, trails, explosions, blood, water, melee impacts)

Source basis: the retail client install read in place with our dumper
(nothing decrypted, nothing committed): the particle manifest and the
particle files `particles/muzzle_flash.pcf`, `bullet_tracers.pcf`,
`rockettrail.pcf`, `explosion.pcf`, `stickybomb.pcf`, `nailtrails.pcf`,
`rocketbackblast.pcf`, `shellejection.pcf`, `flamethrower.pcf`,
`impact_fx.pcf`, decoded element by element; the effect materials
(`materials/effects/*.vmt`) and their textures' headers; the item schema
`scripts/items/items_game.txt`. Valve's Source SDK 2013 (2025 TF2
release) read in place in this spec session: the TF2 weapon base's
muzzle-flash and tracer-name code, the shared bullet-firing routine, the
weapon-script parser (which fields exist and their defaults), the
minigun's, rocket launcher's, syringe gun's and grenade/sticky projectile's
client effects, the rocket projectile's client trail code, the explosion
temp-entity callback, the flame thrower and its flame manager, the shared
impact path and the view-model attachment re-projection. Nothing copied;
restated in our words. Sections 8b, 10.1, 11.1 and 12–14 (2026-11-20,
second spec pass) also read: the client's local projectile temp entities
and their collision, the base projectile's and rocket's and grenade's
client history seeding and draw delays, the entity interpolation
history, the rocket's, grenade's and pipe's explosion paths, the melee
swing trace and its hull-intersection fix-up, the model-decal path and
the TF2 player's decal rules, the crit-boost effect, the flame thrower's
sound loops; and from the install `scripts/decals_subrect.txt`,
`scripts/game_sounds_weapons.txt`, `scripts/surfaceproperties.txt`, the
needle model's bounds and `weapons/crit_power.wav`'s chunks. Sections
11.2 and 14.1 (2026-11-20, tf2-spec-psys) also read: the client's
per-frame order (interpolation, data-changed callbacks, particle
update), the entity post-update and interpolation-list rules, the
rocket's network table, the entity particle property and the particle
system query's model-point picking, the TF2 crit-boost effect's host
entity and view-model weapon split; from the install
`particles/firstperson_weapon_fx.pcf`, `particles/particles_manifest.txt`,
`materials/effects/electric1.vmt`, `sc_softglow.vmt` and the hitbox sets
of `c_soldier_arms.mdl` and `c_rocketlauncher.mdl`; and the particle
library's timing rules (particles.md §9).
**The per-weapon `.ctx` scripts were not read in
this session** (no decryption key was available to the dumper; see Open
questions 1), so every per-gun value that only the scripts hold is marked
*unconfirmed* with the evidence for it.
Status: draft

## Summary

Every TF2 gun's look is a handful of named particle systems from the
`.pcf` files, started at named attachments of the gun model. Bullet guns
draw a muzzle flash at the `muzzle` attachment per shot and a particle
tracer from the muzzle to the hit point for every second bullet (every
fourth for the Minigun), named `<base>_red`/`<base>_blue`, with `_crit`
appended for a critical shot. The Minigun instead runs one continuous
muzzle system while it fires. Projectiles carry trail systems; blasts pick
one of three explosion systems by where they happen. In first person the
muzzle flash, tracer start and flame stream are positioned at the **view
model's** muzzle but drawn **in the world**, at the world FOV
(particles.md §6): that is what sizes them on screen.

## Units and conventions

Hammer units (1 unit = 1 inch), z up, seconds. Particle names are data in
the install's `.pcf` files and are quoted exactly (case as stored).
"Team suffix" is `_red` for RED (team 2) and `_blue` for BLU (team 3).
Attachment axes: a particle system started on an attachment takes the
attachment's position and orientation as its control point 0; its "local"
velocities are along the attachment's axes (x = out of the barrel).

## Constants

| Name (ours) | Value | Unit | Meaning |
|---|---|---|---|
| tracer_every | 2 | bullets | a tracer is drawn for every 2nd bullet (pellet) fired, all guns but the Minigun |
| tracer_every_minigun | 4 | bullets | the Minigun's tracer rate (the generic default) |
| tracer_speed | 5000 / 6000 / 8000 | units/s | travel speed of `bullet_tracer01*` / `bullet_scattergun_tracer01*` and `bullet_shotgun_tracer01*` / `bullet_pistol_tracer01*` |
| muzzle_light | none | – | TF2 makes no dynamic light for muzzle flashes (the generic one is disabled) |
| flash_magnitude | – | – | not used by TF2's particle flashes |

## Behavior

### 1. Per-weapon effect names

The weapon script fields are `MuzzleFlashParticleEffect`, `TracerEffect`,
`ExplosionEffect`, `ExplosionPlayerEffect`, `ExplosionWaterEffect`,
`BrassModel`, `MuzzleFlashModel` (weapon_scripts.md §3). An item's
`visuals` in the item schema can replace the muzzle flash and the tracer
(`muzzle_flash`, `tracer_effect`); **no stock item does** (the schema's
only `muzzle_flash` entries are `muzzle_bignasty`, `dxhr_sniper_muzzleflash`,
`muzzle_raygun_red/blue`, `drg_pomson_muzzleflash`; its only
`tracer_effect` entries are `bullet_bignasty_tracer01`,
`bullet_tracer_raygun`, `bullet_tracer02`), so the stock guns use their
scripts' names.

Columns: **Muzzle** is the per-shot muzzle system; **Tracer base** the
script's tracer name before the team and crit suffixes; **Trail** the
projectile's own system; **Explosion** wall / player-or-air / water.
"Code" means the name is fixed in the game code (confirmed); "pcf" means
inferred from the particle files only (a system of exactly that purpose
and name exists, no other candidate), unconfirmed until the script is
read.

| Weapon (our TOML) | Muzzle | Tracer base | Trail, other | Explosion |
|---|---|---|---|---|
| Scattergun | `muzzle_scattergun` (pcf; = our TOML) | `bullet_scattergun_tracer01` (pcf) | – | – |
| Shotgun (all classes) | `muzzle_shotgun` (pcf; = TOML) | `bullet_shotgun_tracer01` (pcf) | – | – |
| Pistol (Scout, Engineer) | `muzzle_pistol` (pcf; = TOML) | `bullet_pistol_tracer01` (pcf) | – | – |
| Revolver | `muzzle_revolver` (pcf; = TOML) | `bullet_pistol_tracer01` (pcf, unconfirmed) | – | – |
| SMG | `muzzle_smg` (pcf; = TOML) | `bullet_tracer01` (pcf, unconfirmed) | – | – |
| Minigun | **none per shot** (code: the Minigun never makes the per-shot flash); `muzzle_minigun_constant` while firing (code) | `bullet_tracer01` (pcf, unconfirmed), every 4th bullet | `eject_minigunbrass` at `eject_brass` while firing (code) | – |
| Sniper Rifle | `muzzle_sniperrifle` (pcf; = TOML) | none (unconfirmed: no `bullet_sniper…` system exists; the stock rifle's rail/trail effects are attribute-only) | – | – |
| Rocket Launcher | unconfirmed (no rocket-launcher muzzle system exists in any preloaded `.pcf`; likely none) | – | `rockettrail` at the rocket's `trail` attachment (code); crit: + `critical_rocket_red`/`_blue` at its origin (code); under water or teamless: `rockettrail_underwater` (code); third person only: `rocketbackblast` at the launcher's `backblast` attachment on each shot (code, §4) | `ExplosionCore_Wall` / `ExplosionCore_MidAir` / `ExplosionCore_MidAir_underwater` (pcf names; which field holds which is unconfirmed, §5) |
| Grenade Launcher | `muzzle_grenadelauncher` (pcf; = TOML) | – | `pipebombtrail_red`/`_blue` (code); crit: + `critical_pipe_red`/`_blue` (code) | as the Rocket Launcher (§5; the code reads the Stickybomb Launcher's script for both) |
| Stickybomb Launcher | `muzzle_pipelauncher` (pcf; = TOML) | – | `stickybombtrail_red`/`_blue` (code); crit: + `critical_grenade_red`/`_blue` (code); armed pulse `stickybomb_pulse_red`/`_blue` (code) | as above |
| Syringe Gun | `muzzle_syringe` (pcf; = TOML) | – | `nailtrails_medic_red`/`_blue`, crit `…_crit` (code) | – |
| Flame Thrower | none (its stream is the muzzle effect) | – | stream `new_flame`; crit `new_flame_crit_red`/`_blue`; eyes under water `flamethrower_underwater`; airblast `pyro_blast` (all code; flames.md) | – |

Notes:

- The team suffix of a trail follows the projectile's current team (a
  reflected projectile takes the reflector's colour for pipes and rockets;
  a sticky keeps its owner's).
- Crit projectile glows are separate systems started on the projectile's
  origin and following it; the trail itself does not change.
- Our TOMLs' `muzzle` names agree with the particle files for every gun
  except the Minigun (TOML `muzzle_minigun`, a per-shot system TF2 never
  uses for the stock Minigun: §3).

### 2. Muzzle flashes (per shot)

- **When**: every shot of a bullet gun or launcher (one per trigger pull,
  not per pellet), on every client that sees the shooter, at the moment
  the shot's effects play (the same tick as the fire sound).
- **Where**: the particle system is started attached to the `muzzle`
  attachment of the "effect model" and follows it (moves and turns with
  it for the system's whole life):
  - the shooter seen in first person (the local player, or someone
    spectated in-eye): the first-person weapon model (`c_` weapon merged
    on the arms, viewmodel.md);
  - otherwise the third-person weapon model.
- **The first-person case** is positioned in the view model's space and
  then **re-projected into world space**: the attachment's position, in
  eye space, keeps its forward distance and has its right and up
  components multiplied by tan(world FOV / 2) / tan(view-model FOV / 2)
  (specs/cs_source/view_models.md §5; the same rule applies to the `c_`
  weapon attached to the arms). Its orientation is not changed. The
  system then simulates and draws as a world effect with the world's
  projection (particles.md §6).
- No system in the stock muzzle file is flagged as a view-model effect,
  so none is drawn in the view-model pass.
- Not drawn when the view model is hidden (`r_drawviewmodel 0`).
- No light: TF2's muzzle flashes do not light the scene.

### 3. The Minigun

- No per-shot muzzle flash.
- When the gun enters its firing state (spun up and firing), the client
  starts `muzzle_minigun_constant` on the effect model's `muzzle` and
  `eject_minigunbrass` on its `eject_brass` (both follow). When it leaves
  the firing state (stops firing, runs dry, switches, is stunned or goes
  dormant), both stop emitting; particles already alive finish their
  lives.
- `muzzle_minigun_constant` emits continuously (children: `_core` 120/s,
  `_sparks` 100/s, `_starflash` 256/s, `_smoke` 12/s, `_flare` 128/s;
  particles.md §8.3). The starflash and flare emit only in the system's
  first 0.5 s and the smoke only from 2 s (ramping to full at 4 s); the
  sparks are capped at 5 alive; how the flash builds up over its first
  frames: particles.md §8.3.1.

### 4. Rocket Launcher backblast

On each shot, after the muzzle flash, observers who are **not** the
shooter see `rocketbackblast` started on the launcher's `backblast`
attachment (world model) and following it. The shooter never sees it,
in first or third person.

### 5. Explosions

When a rocket, pipe or sticky explodes, every client:

1. finds whether the blast point is in water (point contents);
2. treats the blast as **in the air** when the surface normal sent with it
   is (near) zero (all three components under 0.05 in magnitude) and as
   **on a surface** otherwise; a blast on a player counts as in the air;
3. picks the effect: in water → the script's water effect; else in the air
   or on a player → the script's player effect; else → the script's wall
   effect. An empty field falls back to `ExplosionCore_wall` (the
   lower-case spelling; the file's system is `ExplosionCore_Wall`; Open
   questions 3);
4. orients the effect to the surface normal (angles from the normal; zero
   angles in the air);
5. plays the script's explosion sound at the point (weapons.md §8 and
   weapons_more.md);
6. on the server, unless the blast hit a player, a `Scorch` decal is
   traced onto the surface hit.

For pipes and stickies the script consulted is the Stickybomb Launcher's
(both projectiles); for rockets, the Rocket Launcher's. The particle files
hold `ExplosionCore_Wall`, `ExplosionCore_MidAir` and
`ExplosionCore_MidAir_underwater` (plus `_Jumper` variants for the
rocket-jumper weapon); which name each stock script puts in each field is
unconfirmed (Open questions 1). Content of the cores: particles.md §8.5.

### 6. Tracers

- **Rate**: a single counter, shared by every bullet the client draws
  (all players, all guns), counts each bullet (each pellet) fired; a
  tracer is drawn when the count before the increment is a multiple of
  the gun's tracer rate: 2 for every TF2 bullet gun except the Minigun,
  4 for the Minigun. So a Scattergun blast (10 pellets) draws 5 tracers;
  a Minigun shot (4 bullets) draws 1 on average; which pellets get them
  depends on all other bullets fired before.
- **Name**: `<tracer base>_<team>` by the **shooter's** team, plus
  `_crit` when the shot is critical (`bullet_scattergun_tracer01_red_crit`).
  Guns without a tracer base draw nothing.
- **Start**: the muzzle attachment of the effect model, as for the flash
  (first person: the view model's muzzle re-projected into world space,
  §2). **End**: the bullet's trace end (the hit point, or the trace's
  full length on a miss).
- **Motion**: the tracer system's single particle travels from start to
  end at its system's maximum speed (5000 for `bullet_tracer01*`, 6000 for
  the scattergun and shotgun tracers, 8000 for the pistol's), drawn as a
  sprite trail (particles.md §5) with the additive `effects/tracer1`
  material, radius 2 (red) or 3 (`bullet_tracer01_blue`), maximum length
  5000 (scattergun: 500). The crit versions add a wide (radius 7) red or
  blue streak and a crit impact glow (`bullet_impact1_<team>_crit`).
  `bullet_scattergun_tracer01_*` also spawns `bullet_scattergun_impact01`
  (8 short sparks) at the end.
- A tracer also plays the near-miss sound to a listener near its line
  (specs/cs_source/tracers.md §5 rules).

### 7. Impacts

Bullet impacts use Source's shared impact path (decal by surface, the
old-style dust, flecks and sparks by surface material letter:
specs/cs_source/impact_effects.md §§2–8) with TF2's surface sounds. The
particle impacts in `impact_fx.pcf` (`impact_concrete`, `impact_metal`,
…) are used only when the client's "new impact effects" switch is on; it
is off by default. A ricochet sound (`Bounce.Shrapnel`) plays for 3 in 10
bullet impacts; impact sounds farther than 1024 units from the viewer are
skipped.

### 8. Blood on a player hit

Source basis for this section: the SDK's TF2 player damage path (trace
attack, damage taken), the TF2 blood temp entity and its client
callback, the shared bleed and blood-decal helpers, the client impact
routine, `scripts/decals_subrect.txt` and `particles/blood_impact.pcf`
read in place.

Three independent things make a hit bleed:

**a) The blood burst (particles; the server decides, every client in the
blood point's PVS draws it, the shooter included).**

- **When**: the victim loses health (> 0) from the damage, and none of:
  he is disguised as the attacker's team; he is invulnerable (Über);
  the damage type is generic (0); the attacker's **active** weapon is a
  Flame Thrower (so flames and afterburn while the Pyro still holds it
  never bleed). A victim with the bleeding condition always bleeds.
  Teammates (friendly fire off) take no damage, so no blood.
- **One burst per damage event**, not per bullet: a Scattergun blast's
  pellets on one victim add up into one damage event, one burst. The
  amount of damage changes nothing about the burst (no size or count
  scaling).
- **Where** (P): bullets: the bullet's hit point on the body; melee: the
  attacker's eye + 48 units along his view (a fixed point, not the hit
  point); explosions: the blast centre (not on the body); anything that
  gives no point: the centre of the victim's bounding box.
- **Direction**: d = the unit vector from the victim's box centre to
  (the inflictor's box centre − 10 units up) — toward the shooter, or
  the rocket. The burst is oriented with its forward axis along d (out
  the entry side, back at the shooter).
- **Which systems** (no team variants: the `red` in the names is the
  blood's colour):
  1. The victim's eyes under water (water level 3): `water_blood_impact_red_01`
     and nothing else.
  2. Else if the birthday mode is on: 1 in 5 hits `bday_blood`.
  3. Else if the viewing player sees through Pyro vision:
     `pyrovision_blood`.
  4. Else if low violence (below): `lowV_blood_impact_red_01`
     (under water `lowV_water_blood_impact_red_01`).
  5. Else `blood_impact_red_01`.
  Then (not under water) a **spray** out of the far side: its axis s
  starts as −d (the shot's direction of travel). If |s · the viewer's
  forward| > 0.5 (the spray would point along the view), s is pushed
  sideways by right × k, k = U(0.5, 1.5) + 0.25 × D / 512 (D = the
  viewer's distance to P): to the right when D ≥ 512 and s · right > 0,
  or when D < 512 on a coin flip; to the left otherwise (s is not
  renormalised; only its direction matters). The spray system is
  `blood_spray_red_01` when D < 400, else `blood_spray_red_01_far`
  (`lowV_…` in low violence). Each viewer chooses for himself, so two
  viewers see different sprays.
- Both systems sit at P in the world (they do not follow the body).
- **Contents** (`particles/blood_impact.pcf`, all start at once; "local"
  axes: x forward along the system's axis, z its up):

| System | Material | Count | Life | Radius | Motion | Colour, alpha |
|---|---|---|---|---|---|---|
| `blood_impact_red_01` | (root draws nothing) | – | – | – | – | – |
| `_goop` (child) | `particle/blood1/blood_goop3` SpriteCard sheet, translucent, overbright 3; sequences 0–3 played once over its life at 30 frames/s | 1 | U(0.4, 0.5) | **U(6, 30)** | within 2 units; speed U(0, 32) random + local (16..32, 0, 24..50); gravity −400; drag 0.025; spin ≤ 4°/s | (194,58,5)..(255,31,0); alpha 225–255, in over 0–0.1 of life, out 0.6–1 |
| `_chunk` (child) | goop sheet, sequences 1–3 | 32 at once | U(0.1, 0.7) | U(0.3, 3) | speed U(50, 150) random + local z U(20, 100); gravity −400; drag 0.1 | (242,20,20)..(255,6,6); alpha in over 0–0.5, out 0.5–1 |
| `_droplets` (child) | `effects/blooddrop001` UnlitGeneric translucent; **trail** | 54/s for 0.25 s (≈ 14) | U(0.3, 1.75) | U(0.25, 1.5) × 3 → 0 (bias 0.85) | speed U(48, 50) random + local (90..100, 0, 48..50); gravity −400 | (164,6,6)..(188,0,0), 25 % tinted by lighting; alpha 120–200; τ U(0.05, 0.075) |
| `blood_spray_red_01` | `particle/blood1/blood_goop3_spray` SpriteCard (goop sheet, sequences 8–12, translucent, **overbright 31**, `$minsize` 0.002) | 255/s for 0.2 s (≈ 51) | U(0.1, 0.7) | U(0.3, 2) | speed U(2, 12) random + local (261..311, 0, 42..84); gravity −400; drag 0.1 | (88,22,22)..(255,6,6); alpha 200–255, in 0–0.5, out 0.5–1 |
| `blood_spray_red_01_far` | as above | as above | as above | U(0.3, 3) | local (261..421, 0, 22..64) | (49,0,0)..(195,0,0) |
| `water_blood_impact_red_01` | `effects/softglow_translucent`, alpha | 1 (+ `_chunk`, `_goop` water children) | U(1, 2) | U(4, 6) × 4 → 8 | speed U(3, 15) + local (10..90, ±5, ±5); gravity +5; drag 0.125 | (170,40,40)..(253,43,0); alpha 64–128, from 0 in 0–0.125, out 0.25–1 |

  The ~20-pixel dark red puff at 180 units of the measurement (Open
  questions 8) is the size of the goop (radius 6–30, of which the sheet
  frame covers only part) and the spray; a "speck" means the goop child
  is missing or drawn far too small.
- **Timing**: the burst is a temp entity from the server, so it shows
  when the client's interpolated clock reaches the tick of the damage
  (§11): about 0.1 s (6 frames at 60) plus network time after the hit
  for everyone, the shooter included. The body decal (b) on the
  shooter's own screen is immediate (his client makes it).

**b) The decal on the body (the shared impact path).** Each bullet or
pellet that hits an enemy player, and each melee hit on one, makes one
impact on the body: the surface is flesh (material letter F), so the
decal is `Impact.Flesh` = one of `decals/flesh/blood1`…`blood5` (the
`_subrect` versions of TF2's decal list), placed on the model at the
hit, with no particle or old-style effect (letter F has none). Made by
the shooter's client at once and by others from the impact message.
Skipped in low violence, Pyro vision and during a truce. A disguised spy
gets no impact; an invulnerable player gets a ricochet spark 8 units
back along the shot instead.

*How the body decal is placed and kept* (answers "does TF2 put blood on
models": yes, this way, and only this way):
- The decal name comes from the hit surface's material letter: the
  impact asks for the generic bullet decal and TF2's decal list
  (`scripts/decals_subrect.txt`, its translation table) maps letter F to
  `Impact.Flesh` (one of five `decals/flesh/blood1_subrect`…`5`, chosen
  at random, equal weights), B to `Impact.BloodyFlesh`. The class models'
  hitboxes carry the `flesh` surface (letter F).
- Placement: a ray from the shot's start through the hit point, extended
  8 units past it and then lengthened by 10 %, is clipped against the
  model's hitboxes; where it meets them the decal is projected onto the
  **model's mesh** (a model decal: it follows the animated body, is
  stored on that model instance and skins with it). If the ray meets no
  hitbox, no decal and no impact effects. A ray starting inside the
  victim makes none.
- Not placed on a player who is cloaked (or mid-cloak), disguised, a
  giant (MvM), or invulnerable (the ricochet spark instead). Not drawn
  at all when `r_drawmodeldecals` is 0 (default 1).
- **Removed** (all of a player's model decals at once) when he: is
  healed to full health or more (health rose and is ≥ max), becomes
  invulnerable, starts cloaking, changes or drops disguise (or its
  class), respawns as a new model. The ragdoll is a separate model: the
  living body's decals do not carry over (inferred: decals live on the
  model instance).
- No decal from blasts, fire, or the blood burst itself; the burst (a)
  and the world decals (c) are separate.
- The engine caps decals per model instance (oldest replaced); the cap
  is the engine's, not in the reference (inferred about 50; Open
  questions 15).

**c) Blood on the world behind (server, traced hits only).** For a
bullet, pellet or melee hit (not blasts, not fire): n traces from the
hit point onward along the shot (the shot direction plus U(−σ, σ) on
each axis, not renormalised), 172 units, world brushes only, ignoring
grates; each that hits places the `Blood` decal (`decals/blood1`…`6`).
n, σ = 1, 0.1 when the hit's damage < 10; 2, 0.2 when < 25; else 4, 0.3.
The damage here is the weapon's own per-hit damage before range falloff
and crit multipliers (inferred: those are applied later in the damage
path): SMG 8, Scattergun/Shotgun pellet 6, Minigun bullet 9 → 1 decal;
Sniper Rifle body shot 50 and melee 65 → 4. Not on teammates. Gated by
`violence_hblood` only.

**Violence settings**: low violence is on when any of `violence_hblood`,
`violence_ablood`, `violence_hgibs`, `violence_agibs` is 0, when the
engine runs in low-violence mode, or when the local player sees through
Pyro vision. Low violence swaps the burst and spray for their `lowV_`
versions and drops the body decal; the world decals of (c) follow
`violence_hblood` alone. All four default to 1.

**Bleed ticks** (the bleeding condition's slash damage) additionally
spawn the old Source blood effect at the victim's eyes, amount = damage
× 5 capped at 255 (specs/cs_source/impact_effects.md has the old
effect's family; its TF2 look is unconfirmed).

### 9. Under water (rockets)

- **The rocket ignores water.** It collides with solids only (world,
  enemy players and buildings), is neither stopped nor slowed by water,
  and explodes where it touches the river bed (weapons.md §5.1).
- **Trail choice, once**: when the rocket first appears on a client (and
  again when it is reflected), if the point contents at its origin
  include water (or slime), it gets `rockettrail_underwater` at its
  `trail` attachment instead of `rockettrail`; a teamless rocket also
  gets it. The choice is not revisited: a rocket fired from above into
  water keeps `rockettrail` (its smoke continues under water, as
  measured), and one fired under water keeps the underwater trail after
  it leaves. The two differ only in the smoke's shorter life (0.5–1.2 s)
  and the added bubbles (particles.md §8.5).
- **The long white trail** is the trail's smoke: 150 puffs/s left in
  place along the flight (not attached), white-grey after 0.1 of their
  life, living up to 1.2 s; it starts where the drawn rocket starts,
  behind the shooter (§11), so from the shooter's view it runs from the
  gun to the rocket.
- **The launch glow** (an orange glow round the launcher for ~6 frames)
  is not a muzzle effect: it is the trail's `rockettrail_fire` glow
  (radius 5 × 3 = 15, additive) on the drawn rocket passing beside the
  camera during the interpolation delay (§11). It shows above water too
  (measured, §11).
- **The blast**: each client tests the point contents of the blast point
  for **water** (slime does not count here); in water the script's water
  effect is used (§5; ours' data: `ExplosionCore_MidAir_underwater`, the
  field's content unconfirmed), oriented by the surface normal like the
  others. Its look on the river bed: the mid-air set's flash — an orange
  glow whose radius grows from 7 to 224 units in 0.1 s — the core flash and
  embers, plus 64 rising bubbles (particles.md §8.5). Seen from above the
  surface the burst is hidden; the eye just above or below shows it.
- **Other effects by water**: the Flame Thrower's stream by the Pyro's
  eyes (flames.md); blood by the victim's eyes (§8a).

### 10. Melee hits

- **On the world** (and on anything not of the attacker's team): the
  shared impact path (§7) runs with the damage type "club", on the
  **attacker's own client only** (inferred: the impact is made in the
  predicted swing; the server sends none, so other players see no melee
  decal). Decal by the surface's material letter: C → `Impact.Concrete`
  (`decals/concrete/shot1`–`5`), M and V → `Impact.Metal`, W →
  `Impact.Wood` (`decals/wood/shot1`–`5`: the faint scratch on 2fort's
  wooden walls), Y → `Impact.Glass`, N → `Impact.Sand`, D →
  `Impact.Dirt`, F → `Impact.Flesh`, B → `Impact.BloodyFlesh`; other
  letters keep `Impact.Concrete` (inferred). These are the same decals
  bullets leave, one per hit, at the swing trace's hit point. The
  old-style effects follow the letter at scale 1 as for a bullet
  (impact_effects.md §§3–8: flecks and dust for C, T, W; dust for D, N;
  sparks for M, V; electric sparks for P); the surface's impact sound
  plays within 1024 units; no ricochet sound (bullets only). The weapon
  plays its own "hit world" sound.
- **On an enemy player**: the body decal of §8b at the hit (attacker's
  client), the blood burst of §8a at the attacker's eye + 48 forward,
  and the world decals of §8c (4 traces for stock melee damage).
- Teammates: no impact, no blood.

#### 10.1 Where the swing's impact lands (the contact point)

The swing is resolved once, at the smack (weapons_stock.md), on the
attacker's machine and the server alike:

- Start S = the attacker's eye; direction f = his view; reach R = 48
  units (72 for swords, 128 while shield-charging); the swing box a cube
  of half-size 18 (36 wide), both scaled by melee attributes and by a
  player model scale above 1.
- **1. Line first.** A line from S to S + R f against solids (world,
  players of both teams, buildings; not the attacker). If it hits, the
  contact is that line's hit point on the surface, with the surface's
  normal. This is the usual case when the crosshair is on the target.
- **2. Box if the line missed.** The 36-unit cube is swept from S to
  S + R f. If it touches nothing: a miss (no impact).
  - It touched a **player, building or other non-brush entity**: the
    contact is the **cube's centre where the sweep stopped** (C), not a
    point on the victim: a point on the view line, up to 18 units short of
    the victim's box along each axis; normal = the face touched. No
    correction.
  - It touched the **world or a brush entity**: a correction looks for a
    real surface point. Let E = S + 2 (C − S) (twice as far as the
    cube's centre). A line S → E: if it hits, its hit is the contact.
    Otherwise eight lines from S to E + k, k each corner of the box
    x, y ∈ {−24, +24}, z ∈ {0, 62} (the crouched player box: the corners
    never point below E); of those that hit, the one nearest to S is the
    contact. If none hits, C stays (a point in the air).
- **Impact effects from the contact** (attacker's client, §10): the
  impact ray runs from S through the contact point, extended 8 units
  past it, lengthened 10 %; on the world the decal is centred on the
  contact point when that ray reaches the surface (so with C left in the
  air there is usually no decal); flecks, dust or sparks start at the
  contact point with the ray's surface normal (only when the ray found
  the surface); the surface sound plays at the ray's hit, else at the
  contact point. On a player the body decal follows §8b with this ray:
  a box-sweep hit whose cube centre is off the body leaves no body decal.
- Quirks: a glancing swing that the line misses leaves its decal
  where a corner line hit, up to 2R from the eye and biased upward
  (corner offsets 0 or +62 in z); and the doubled line can reach 96
  units, farther than the swing's own reach.

### 11. When a rocket and its blast show on a client

- **No spawn delay, arming time or first-frame offset** in the rocket
  itself: on the server it exists and flies at 1100 units/s from the
  tick of the shot (weapons.md §5.1); it may explode in its first
  ticks.
- **The client draws it in the past.** Every client (the shooter
  included: the rocket is not predicted) draws the rocket, and anything
  attached to it (its trail), at the interpolated time now − lerp, with
  lerp = max(`cl_interp`, `cl_interp_ratio` / `cl_updaterate`); TF2's
  defaults give max(0.1, 2/20) = **0.1 s**. When the rocket first
  appears the client gives it a history point one second earlier at its
  first known position minus its initial velocity, so the drawn rocket
  is on its flight line from the start: it appears about 110 units
  **behind** its launch point (behind the shooter's eye) and overtakes
  the camera in the first ~5 frames. The model is not drawn for its
  first 0.2 s; the trail is.
- **The blast is delayed the same way**: it is a temp entity and shows
  when the client's interpolated clock reaches the tick it happened in,
  so the drawn rocket reaches the blast point as the blast appears.
- **Measured** (2026-11-20, tools/tf2ref, real TF2, a Soldier on 2fort's
  bridge looking 25° down, the crosshair on the floor ~160 units ahead,
  stepped at 60 frames a second, `+attack` at frame 1): with the defaults
  (`cl_interp` 0.1, `cl_updaterate` 20) the blast shows at **frame 15**
  (twice); with `cl_interp` 0 and `cl_updaterate` 66 (lerp 0.03, set
  from the spectator team) at **frame 11**. The flight itself is ~0.12 s
  (7 frames). The 4-frame difference is the lerp difference (0.07 s);
  the rest is the shot reaching the server (~1 frame on a listen
  server) and waiting for a snapshot (up to 1/`cl_updaterate`). With the
  defaults the trail's orange fire glow stays at the right edge of the
  view for frames 2–10 (the rocket behind and beside the camera); with
  lerp 0.03 for frames 2–6.
- So ours' 7 frames against TF2's 13 is the missing interpolation delay:
  parity is to draw rockets (and other server projectiles) and their
  trails at now − lerp, seeded backwards along the initial velocity, and
  to play their blasts (and the other server temp effects: blood §8a,
  the scorch) when the interpolated clock reaches their tick.
- The landing point: with the crosshair on a floor nearer than 200
  units the rocket aims at the 2000-unit point along the view, so from
  a standing eye 25° down it lands ~139 units out horizontally against
  the crosshair's ~146 (weapons.md §5.1): nearer, not farther. The
  "farther" of the earlier comparison is not explained by the rocket
  (Open questions 9).

#### 11.1 The first moments of a rocket, pipe or sticky on a client

(Answers Open questions 11.)

- **Interpolation delay, exactly**: an entity's drawn time is the
  client's time minus L, L = (round(lerp / tick) + 1) ticks, with lerp =
  max(`cl_interp`, `cl_interp_ratio` / `cl_updaterate`) and round to
  nearest. TF2's defaults: lerp 0.1, tick 0.015 → round(6.67) = 7, + 1 =
  **8 ticks = 0.12 s**.
- **Seeded history.** When the projectile first arrives (client time
  t_a), the client throws away the history the arrival made and puts
  exactly two points in it: (s0 − 1 s, p0 − v0) and (s0, p0), where p0
  is the first received position, s0 its simulation time (the server
  time of that state) and v0 the launch velocity the server sends with
  the projectile (units/s; for pipes and stickies their launch velocity,
  gravity ignored). The angles get two equal points (no turn).
- **Used from the first frame.** There is no special first frame and no
  hold at the launch point: for drawn time r < s0 the projectile is drawn
  at p0 − v0 (s0 − r), on its launch line behind p0, moving forward at
  the launch speed from its very first interpolated frame (whether the
  arrival frame itself is interpolated: §11.2, Open questions 16). Later
  snapshots add points and the usual interpolation (a smooth curve
  through the three newest points) takes over.
- **p0 is not the launch point**: it is the server's position in the
  first snapshot that carries the projectile, the launch point plus the
  flight between the shot's tick and that snapshot (snapshots every 1/20
  s by default: 0 to about 3 ticks, so up to ~50 units for a rocket).
  Because the seed extrapolates back with the same velocity, the drawn
  rocket passes its launch point at drawn time ≈ the shot's tick
  (exactly, for a straight rocket; a pipe is off by the gravity drop of
  those few ticks).
- **First person, defaults**: at arrival (≈ 1–2 frames after the shot on
  a listen server) r ≈ s0 − 0.1, so the drawn rocket is ≈ 110 units
  behind its launch point (≈ 86 behind the eye) and passes the camera
  in the next ~5–6 frames at 60 frames/s (the "launch glow" beside the
  view, §9). A pipe (≈ 1200 units/s) and a sticky likewise start behind
  the eye.
- **Model hidden** for a time counted from the arrival on that client
  (the client's own clock): rockets **0.2 s**, pipes and stickies
  **0.1 s**. So the pipe "first shows at the barrel at frame 8" (arrival
  ~frame 2 + 6 frames; by then the drawn pipe is about at the launch
  point) and the rocket "first shows at frame 13, ~150 units out" are
  the hide window ending, not a hold.
- **The trail** is created at arrival (rocket: at the model's `trail`
  attachment; pipes and stickies: at their origin), both following the
  drawn projectile, and emits from the frame after arrival (§11.2). Behind-the-eye particles are
  simply outside the view. (The earlier inferred nuance here, "the
  system is created before that frame's interpolation", is replaced by
  §11.2: it is created after the interpolation pass, emits nothing in
  its first frame, and its children start a frame later.)
- Needles are not networked entities: §12.
- So ours' "hold at the launch point until the drawn time reaches the
  shot" is not TF2's; parity is the seeded back-extrapolation above.

#### 11.2 A rocket's first drawn places and its trail's first frames

(Answers the tf2-psys and tf2-leftovers spec questions on the launch glow;
refines §11.1. Timing rules of the particle systems: particles.md §9.)

**What the first snapshot carries.** The rocket's origin is sent in
**whole units** (each component rounded to an integer), its angles, its
launch velocity v0 (each component in 12 bits over −3000 to +3000
units/s: steps of about 1.465 units/s) and its simulation time s0 (the
server time of its last move, whole ticks). p0 = the launch point + v0 ×
k × 0.015, k = the ticks the rocket has flown by the snapshot that first
carries it. Snapshots reach a client every 1/`cl_updaterate` s (20/s: 3
or 4 server ticks apart at 66.67 ticks/s), so k is 0 to 4: p0 is 0 to
~66 units past the launch point for a rocket (16.5 units a tick), most
often within 3 ticks (≤ 49.5). Whether the rocket already moves in the
tick it is fired depends on the server's entity order, which the
reference does not settle.

**Order inside the client's arrival frame** (from the client's frame
code):

1. The packet is read: the rocket is created at p0 and its history
   seeded (§11.1).
2. The interpolation pass: every entity **on the interpolation list**
   is moved to its drawn place.
3. Bones and attachments are set up from those places.
4. Data-changed callbacks: here the rocket creates its trail
   (`rockettrail`, or `rockettrail_underwater` when its place is in
   water, §9), with control point 0 on its `trail` attachment as the
   rocket stands **now** (after step 2).
5. The particle update: every system's control points are read again
   from their attachments, then systems are simulated. The new trail is
   in its first frame: zero time, its 1 initial smoke particle at the
   point, no emission, children not stepped (particles.md §9.1).

**Which place the rocket has at step 2 in the arrival frame.** An entity
goes onto the interpolation list when its received state is latched,
and only if it already counts as drawn at that moment (it is registered
for rendering); the engine's order between that registration and the
latch of a new entity is outside the reference. So two cases:

- **(A) interpolated from arrival:** drawn at p0 − v0 (s0 − r) with r =
  client time − L (§11.1), about 110–130 units behind the launch point;
  the trail starts there.
- **(B) not yet on the list:** drawn at **p0** (its received place) until
  the next snapshot is latched (≤ 1/`cl_updaterate` later: up to 0.05 s,
  3 frames at 60), which puts it on the list; from that frame it is
  drawn on its seeded line, at that moment r < s0 still, so it **jumps
  back** behind the launch point and comes forward at the launch speed
  as in §11.1.

**The capture fits (B), not (A)** (inferred; Open questions 16). TF2
shows a large glow (~10 units) with short spark streaks ~50 units ahead
of the launcher from the frame after the shot for a few frames, and then
the trail's glow passing the camera at the view's right edge (frames
2–10, §11). Frame by frame at 60 frames/s with the defaults, case (B):

| Frame | Rocket drawn at | Trail |
|---|---|---|
| a (arrival) | p0 | system made at p0 (+ the `trail` attachment's offset); 1 smoke puff at p0; nothing else |
| a+1 … until the next snapshot (1–3 frames) | p0 (not moving) | parent smoke 2–3 a frame, all at p0; the children's first step (no spread): `rockettrail_fire` 2 a frame at p0 (`effects/sc_brightglow_y_nomodel`, radius 5 × 3 = 15 at birth shrinking to 0, alpha 64, life 0.2 s), `rockettrail_burst` 2 a frame 10 units along the point's x axis from p0, flying out at 10–30 forward, ±110 sideways, drawn as short sprite trails |
| the latch frame | jumps to p0 − v0 (s0 − r), ~110–150 behind p0 | the point jumps back; this frame's particles at fractions k/n of the straight jump from p0 to the new place (none at p0, the last on the rocket); the burst sparks alive are carried back with the point (they are locked to it); fire glows and smoke stay where they were made |
| later | forward at 1100 units/s, passing the launch point ≈ L after the shot's tick | normal: each frame's particles along the frame's path, the newest on the rocket |

So the "glow ahead of the launcher" is the fire glows and burst sparks
made at p0 while the rocket stands there unseen (model hidden 0.2 s), a
few frames' worth, fading over their 0.2 s life; the white smoke trail
then starts from p0 too (puffs left there) and reaches back to the eye
when the rocket jumps back. In case (A) nothing of the trail would be
ahead of the launch point until about L (≈ 7 frames) after the shot.

**Parity, therefore (case B):** a projectile's first received place p0
is drawn as received (not interpolated) until its second snapshot; its
trail is made at p0 in the arrival frame with only its initial
particles; the children start emitting the next frame, with no spread,
at the point's place then; at the second snapshot the drawn place goes
over to the seeded interpolation (jump back, then forward), and that
frame's particles spread along the jump. Under water the same with
`rockettrail_underwater` (its bubbles child too).

#### 11.3 A sticky's armed pulse: when it shows

(Answers the sticky-recheck spec question, 2026-11-20: the measured
~1.03 s from the launch flash to the first ring against weapons_more.md's
0.8 s arm time. Read from the game code and the `stickybomb.pcf` file;
checked against both captures.)

- **Who decides.** The pulse is the client's own: no message from the
  server starts it. Each client keeps, per sticky, the time it first
  received it (its arrival, t_a, on that client's clock: §11.1). On every
  frame after that, if the sticky's pulse hasn't played and the client's
  time − t_a ≥ L (L = the sticky's arm time, weapons_more.md §5.3: 0.8
  stock, 0.6 Quickiebomb), it starts `stickybomb_pulse_red` (`_blue`)
  following the sticky's origin, once. (The server arms the sticky L after
  its own creation; the client's pulse is therefore late by the arrival
  delay and does not follow the interpolation delay.)
- **The system** (`stickybomb_pulse_*`): no initial particles; a
  continuous emitter of 10 a second from its time 0 for 0.7 s (so 7
  rings, at most 3 alive); each ring lives exactly 0.1 s, locked to the
  sticky, radius growing from 0 to 15 over its life, alpha fading in over
  the first 20 % of its life and out over the rest, colour fading to
  (255, 62, 62) (the red system).
- **Why the first ring is late** (particles.md §9): the frame the system
  is made it runs with zero time (nothing); the emitter's running total
  reaches one particle only once 0.1 s of system time has passed, and
  that particle is born at the end of that step, age 0, radius 0
  (invisible); it is first seen the frame after, a ring of radius 15 ×
  (frame time / 0.1). So the first visible ring comes at

```math
t_{ring} \approx t_a + L + q + 0.1 + \Delta
```

  with q the wait for the first frame past t_a + L (0 to one frame) and
  Δ one frame. Then a ring every 0.1 s for the 0.7 s.
- **The sticky's model** is hidden until t_a + 0.1 (§11.1), so the ring
  comes L + q + Δ after the sticky is first seen.
- **The captures, explained** (30 frames/s, Δ = 1/30, the launch flash
  in frame 0, arrival ≈ 2 frames later as the 0.1 s model hide shows the
  sticky at frame 5): stock, L = 0.8: first ring ≈ 2 + 24 (+ q: 0–1) + 3 + 1 =
  30–31 frames after the flash (1.0–1.03 s; measured ~1.03), 25–26 frames
  after the sticky first shows (measured 26). Quickiebomb, L = 0.6: 19–20
  frames after the sticky shows (measured 20), 24 after the launch
  (0.8 s: the "Measured" note below, which was the Quickiebomb).
- **For ours**: start the pulse system L after the sticky's arrival on
  that client (not after the shot, not at L + a tuned delay) and let the
  particle timing rules make the rest; a fixed "after the launch" of
  0.87 s matches the stock capture only at this arrival delay and frame
  rate.

### 12. Syringe Gun needles: what is drawn and what a hit leaves

- **The server's needle is invisible**; every client draws its own
  needle, a local client-only model (`w_syringe_proj.mdl`, skin 0 RED,
  1 BLU) with the `nailtrails_medic_<team>` trail (`…_crit` on a crit;
  §1), created from an effect message the server sends to the players
  that can hear the shot's start or end point (to all when the start and
  the aim's end differ in water contents). The shooter's own client
  makes his when it predicts the shot (inferred: the server's copy is
  held back from the predicting player as for his other predicted
  effects). Start: for the local player in first person the shot's own
  start point (16 forward, 6 right, 8 down from the eye,
  weapons_more.md §8); for anyone seen from outside, the `muzzle`
  attachment of his held world weapon. Velocity the server's (1000
  along the aim plus spread), gravity 0.3 × 800 = 240 units/s² down,
  life **6 s** from launch.
- **Its collision**: each step it traces a line from where it was to
  where it is, against solids (world, props, any player's box; not the
  shooter). To save work it traces only every 5th frame
  (`cl_fasttempentcollision` 5), each trace covering the whole path
  since the last one, so the hit point is still exact. On a hit the
  needle's trail stops emitting (its particles finish) and the impact
  below runs once.
  - **World** (not sky): the needle stops dead at the hit point,
    keeping its flight orientation, and stays there, drawn, until its
    6 s from launch run out (no fade). Its model reaches 5.9 units
    behind and 3.25 ahead of its origin along its axis (the model's
    bounds), so either about 6 units of it stand out of the wall or it
    is mostly inside, by which end is the tip (unconfirmed; Open
    questions 13: the capture saw none).
  - **Anything else** (a player of either team, a prop): the needle
    disappears at once.
- **The impact** (the shared impact path of §7 with the needle's data):
  hit point = the trace's end, start = the start of that trace's
  segment, surface = the surface the trace hit. In the field where a
  bullet impact carries its damage type the needle puts the
  **shooter's team number** (RED 2, BLU 3).
  - Nothing happens when the hit entity is on the shooter's team
    (teammates: the needle just vanishes), or is a Spy disguised as the
    shooter's team. The world and teamless things always take it.
  - **Decal**: the generic bullet decal translated by the surface's
    material letter, exactly as a bullet's: `Impact.Concrete`
    (`decals/concrete/shot1`–`5`), `Impact.Wood`, `Impact.Metal`, … (the
    letter table of §10). The capture's "small dark hole, red centre
    head-on" on 2fort's wood is `Impact.Wood`. One per needle, placed by
    the ray from the segment's start through the hit point + 8 units,
    lengthened 10 %.
  - **Old-style effects** by letter at scale 1 (wood and concrete
    flecks and dust, sparks on metal, dust on dirt and sand;
    impact_effects.md), only when that ray found the surface. Particle
    impacts only with `cl_new_impact_effects` 1 (off by default).
  - **Sound**: the surface's bullet-impact sound at the hit, skipped
    beyond 1024 units from the viewer. **Ricochet quirk**: the 3-in-10
    `Bounce.Shrapnel` is gated on the damage type being "bullet", whose
    value is 2: RED's team number. So RED needles ricochet-sound on 30 %
    of their impacts, BLU needles never.
- **On an enemy player**:
  - Client side (every client with the needle): the impact above on the
    player. The touched thing is the player's **box**, which has no
    surface of its own; the engine reports the default surface (letter
    C) for it (inferred: the engine's box clip is outside the
    reference). The decal then is the letter-C one (`Impact.Concrete`)
    projected onto the body (§8b placement), and concrete flecks at the
    box's surface — but only if the ray (segment start → box point + 8,
    + 10 %) reaches a hitbox; the box is wider than the body, so many
    hits make neither. The sound: the hitbox's surface (`Flesh.BulletImpact`)
    when the ray met a hitbox, else the default surface's
    `Default.BulletImpact` (Open questions 14).
  - Server side: the needle's hit is a traced hit: the blood burst of
    §8a at the needle's server position at the touch (its origin, not a
    point on the body), oriented toward the attacker as §8a; the world
    blood decals of §8c with the needle's per-hit damage 10 → **2 traces,
    σ 0.2**; the usual victim conditions (§8a).
- No scorch, no explosion, no particle impact from the server for a
  needle; the server's needle on the world is simply removed.

### 13. Where a blast is centred (rockets, pipes, stickies)

(Answers Open questions 6 and 7.) No blast traces back along the flight
except the pipe's direct-hit line below; all move the centre **1 unit
out along the surface normal** of the trace they use.

- **Rocket** (a zero-size point): it explodes when its movement touches
  a solid (world, an enemy player's box, a building; teammates are not
  solid to it). The touch gives the exact surface point where the move
  stopped and that surface's normal. Centre = that point + 1 unit along
  the normal. The normal is sent with the blast (on a player: the normal
  of his box's face that was touched; the client still uses the player
  effect, oriented by that normal). Sky: removed, no blast. Scorch
  (§5 step 6): placed from the same touch, i.e. on the surface at the
  touch point; not when the rocket touched a player.
- **Pipe hitting an enemy directly** (before its first world contact):
  a line from 32 units behind the pipe's centre along its velocity to 32
  units ahead (64 long), against solids, not the pipe itself. If it hits:
  centre = hit + 1 unit along that surface's normal, normal = that
  surface's, the "target" = whatever the line hit (a player → player
  effect; the world, if a wall lies in the line before the player → wall
  effect and scorch). If it misses (the pipe's 4-unit box grazed the
  player off the line): centre = the pipe's centre, normal zero, no
  target → in-air effect, no scorch.
- **Fuse or command** (a pipe's timer, a detonated sticky, a pipe that
  touched a moving train or a dynamic prop that blows grenades): a line
  straight **down** from 8 units above the projectile's centre to 24
  units below it (32 long), against solids including players, not the
  projectile itself.
  - Hit: centre = the hit + 1 unit along its normal (a pipe resting on
    a floor: about 1 unit above the floor, a little below its own
    centre); normal = the floor's; target = a player if the line hit one.
    So a pipe or sticky blowing up in the air **within 24 units above a
    floor snaps its blast down onto the floor** and uses the wall
    effect there.
  - Miss: centre unchanged, normal zero (in-air effect).
  - A **stuck sticky** (stuck to the world or a dynamic prop) sends
    instead the normal of the surface it stuck to, pointing out of that
    surface; the position still comes from the downward line. A sticky
    on a wall with no floor within 24 units below explodes at its own
    centre with the wall's normal (wall effect, oriented on the wall). A
    sticky knocked loose by a blast loses its stored normal.
  - A sticky on a ceiling: the line starts 8 units above its centre,
    inside the ceiling; a line that starts inside solid and leaves it
    reports no hit (inferred), so it explodes at its own centre with
    the ceiling's normal (pointing down).
  - **Scorch**: on what the downward line hit, unless a player; none
    when it missed (no scorch for that wall sticky; a sticky on the
    floor scorches the floor under it).
- **Under water**: nothing about the centre changes. Each client tests
  the point contents **at the centre** for water (slime does not count);
  in water the script's water explosion field is used, oriented by the
  normal as above. A rocket on a river bed: centre 1 unit above the bed,
  in water → water effect along the bed's normal; a pipe or sticky lying
  on the bed: the downward line finds the bed → the same. A blast whose
  centre is above the surface (a rocket hitting a wall just above the
  waterline) uses the normal effects even if the surface is 1 unit
  below. The water field's content: ours' data
  `ExplosionCore_MidAir_underwater` (the only underwater core in the
  particle files; the scripts' value still unread, Open questions 1).
  The sound is the script's explosion sound either way.

### 14. Crit sounds and glow on the held weapon

- **Crit boost loop** (`Weapon_General.CritPower` → `weapons/crit_power.wav`,
  8.25 s mono 44.1 kHz, one cue at sample 0: loops whole,
  specs/source/sound_looping.md; static channel, 74 dB, volume 1, pitch
  100). It is the **boost's** sound, not the crit shot's: random crits
  never play it.
  - Each client decides for each player it sees. The boost "shows" when
    the player has any crit-boost condition (Kritzkrieg's charge, first
    blood, the round-win bonus time, on-kill and capture crits, pumpkin
    and power-up crits, the demo charge's crit) or Crit-a-Cola's
    mini-crit buff or the Sniper's rage charge, **and** his held weapon
    can be crit-boosted, **and** he is not cloaked; for any player but
    the local one, also not disguised.
  - On the change from not showing to showing it starts, **at once at
    full volume** (no fade), attached to the **player** (the sound
    follows him; every client's own copy, heard by that client only).
    Also the glow particle `critgun_weaponmodel_red`/`_blu` (his team; a
    disguised enemy shows his disguise team's) starts on the model drawn
    for him (for the local player his view model, the arms, drawn in the
    view model's pass; the held world weapon for others): §14.1.
  - It **stops at once** (cut, no fade) when the boost stops showing:
    the condition ends, he cloaks, he (as seen by others) disguises, or
    he switches to a weapon that cannot be crit-boosted. Switching
    between two boostable weapons keeps the loop running uninterrupted
    (it is started only if none is playing); the glow moves to the new
    weapon. A disguise change by the local player restarts the glow and
    the loop from the start.
  - An enemy who is cloaked, blinking or disguised shows neither glow
    nor loop to the other team at all.
- **Crit fire loops of continuous weapons** (separate from the boost):
  the Minigun swaps Fire ↔ FireCrit by its networked crit flag
  (minigun.md). The Flame Thrower: when firing starts, its start sound
  (`Weapon_FlameThrower.FireStart`, inferred from the script's slots)
  plays at volume 1 fading to 0 over 3.5 s while its loop fades in from
  0 to 1 over 3.5 s: `.FireLoop`, or `.FireLoopCrit` when the burst is
  critical. When the burst's crit flag flips while firing, both stop at
  once and the other loop starts at full volume with no start sound
  (no `.FireEnd` either). Releasing the trigger cuts the loop and plays
  `.FireEnd`. A boosted Pyro's every burst is critical, so he hears
  FireLoopCrit plus CritPower.
- One-shot guns play the script's crit shot sound (`…SingleCrit`) for a
  crit shot instead of `…Single` (weapons_more.md §9.4); no loop.

#### 14.1 The crit glow's systems and how they sit on the model

- **File**: `particles/firstperson_weapon_fx.pcf`, preloaded (listed with
  `!` in `particles/particles_manifest.txt`); a DX8 copy
  `firstperson_weapon_fx_dx80.pcf` holds the same names. Each of
  `critgun_weaponmodel_red` / `_blu` has one child, `..._red_glow` /
  `..._blu_glow` (delay 0). Neither sets "view model effect"; no
  initial particles.
- **Host entity and point.** The system is started on an entity with
  "follow the entity's origin": control point 0 is that entity's origin
  every frame (no attachment) and carries the entity itself as the
  point's object, which the model initializer and lock below use. The
  entity: for the **local player, his first view-model entity**, which
  for every stock item (all attach to the hands) holds the class's arms
  model (`c_<class>_arms.mdl`, e.g. `c_soldier_arms.mdl`); the weapon's
  own `c_` model is a separate entity merged onto it and is **not** the
  host. For anyone else, his **active weapon** entity (its world model,
  for stock items the same `c_` model, e.g. `c_rocketlauncher.mdl`). The
  local player's system is put in the view model's translucent render
  group: drawn in the view-model pass with the view model's FOV, after
  the world; its particles are placed on the view model's own bones (in
  the view model's space), so no re-projection (particles.md §6) applies.
- **Placement** (`Position on Model Random`, point 0, hitbox scale 1,
  direction bias (1, 1, 1), not forced inside): for each new particle, 6
  tries (a non-zero bias raises the tries to at least 5, plus the first);
  each try picks one hitbox of the host model's current hitbox set
  uniformly, a point uniform inside its box (each axis between the box's
  min and max × the model scale) and puts it through that hitbox's bone's
  current world transform; its score is U(0, 72) + (point − point 0's
  position) · (1, 1, 1); the best-scoring try is kept. The bias is in
  **world** axes (quirk: favours the +x +y +z side of the host's origin,
  so it shifts as the player turns). Hitbox sets read: `c_soldier_arms.mdl`
  47 boxes (root, upper/lower arms, wrists, hands, `weapon_bone`, a
  `rocket` box, and 36 finger boxes), so in first person the glow sits
  mostly on the hands and fingers; `c_rocketlauncher.mdl` 2 boxes (the
  whole gun on `weapon_bone`, −8.3..8.3, −7.6..15.0, −36.3..21.4, and a
  small one on `vm_weapon_bone_1`).
- **Following**: `Movement Lock to Bone` (point 0): each particle keeps
  its place relative to its hitbox's bone and moves with it (inferred
  from the name and the hitbox-relative output of the placement).
- **Values** (both teams alike but colours):

| | `critgun_weaponmodel_<team>` | `..._<team>_glow` |
|---|---|---|
| Emission | 60/s, endless; max 80 | 60/s, endless; max 25 |
| Material | `effects/electric1` (UnlitGeneric, additive), animated sprites, 31 frames/s fitted to life | `effects/sc_softglow` (SpriteCard, additive, no depth blend, `$maxsize` 0.35, fade 0.475–0.65) |
| Radius, life | U(4, 6); U(0.1, 0.2) s | 8; U(0.6, 1.0) s |
| Alpha | 255; fade in 0–0.5 of life, out 0.5–1.0 | 12; Alpha Fade and Decay with start alpha 0, fading out over the whole life |
| Colour RED | (255,148,70)..(255,108,0) → (176,0,0) from 0.2 to 1.0 of life | (255,31,31)..(255,108,0) → (249,53,0) |
| Colour BLU | (125,125,255)..(116,162,255) → (0,100,249) | (125,125,255)..(116,162,255) → (0,100,249) |
| Motion | drag 0.15, no speed; random roll 0–360 | same |
| Oscillate Scalar on radius (field 3) | rate 64, frequency U(0.5, 2), × 16, phase 0.5, over the whole life | rate U(32, 64), frequency U(0.5, 2), × 12 |
| Oscillate Scalar on alpha (field 7) | rate U(1, 128), frequency U(2, 8), × 64, phase 0.5 | rate U(1, 2), frequency U(2, 8), × 64 |

  The oscillator adds rate × h × W(x) to the field each step, with
  x = frequency × (multiplier × the system's time + phase) here
  ("proportional" 0), W a parabolic sin(πx), alpha clamped to 0–1:
  particles.md §10.9 (formula and worked numbers for these four).

## Per-frame order

1. Shot (prediction or network): fire sound, muzzle flash system started
   (§2), bullets traced; for each bullet: impact, then tracer decision
   (§6) and tracer start.
2. Each frame: attached systems update their control points from the
   attachment (first-person ones re-projected with this frame's FOVs)
   before simulating; then all systems draw in the world's translucent
   pass, sorted (particles.md).

## Edge cases

- Holstering or dying stops the Minigun's constant systems; per-shot
  systems already started finish.
- A shooter who is dormant on this client (outside its view set) gets no
  muzzle start: his tracer starts at the bullet's own start point (the
  eye).
- Tracers through broken glass: not special in TF2's routine.

## Quirks

- The shared tracer counter makes tracers of one gun depend on every other
  gun firing at the same time.
- Blue standard tracers are wider (radius 3) than red ones (2).
- A blast on a player uses the in-air effect even if the player stands
  against a wall.

## Measured in the running game (tools/tf2ref, 2026-11-20)

Captured at 1024x768, stepped frames (30 or 60 a second), on 2fort's
bridge; facts of the picture, not of the code.

- **Stickybomb, first person**: the sticky's model (a team-coloured spiked
  ball) is seen in flight and where it lands; its trail glow is barely
  visible (a faint team tint at the sticky, no glowing disc). The armed
  pulse comes 0.8 s after the launch (frame 26 at 30 a second, the launch
  at frame 2) — that capture held the **Quickiebomb** Launcher (arm time
  0.6); the stock launcher's first ring is ~1.03 s after the launch
  flash (2026-11-20 recheck; §11.3) — and repeats every 0.1 s: per pulse a thin orange ring the
  sticky's size, then a translucent team-coloured disc about 3 times
  wider, then nothing for a frame.
- **Rocket into water** (2fort's river, the surface near z = -140): seen
  from above the surface the burst under it is hidden, only the trail
  shows; with the eye just above the surface the burst shows through the
  water as an orange glow with a few sparks and floating specks. The
  rocket's ordinary smoke trail goes on after it enters the water.
- **Bullet on wood** (SMG, bridge side wall, 100 units): a small light
  bullet-hole decal, a faint pale puff and a few dark splinters falling
  from the hole for about 0.15 s.
- **The Flame Thrower's pilot** (`new_flame_pilot`, a sprite trail locked
  to the muzzle with no movement operator): no streak is seen while the
  gun is still.
- **A rocket on the floor at 150 units, first 50 ms**: with
  `mat_disable_bloom 1` the first frame is still a pale wide glow with a
  small orange core; from the second frame on it is saturated orange with
  long yellow streaks. The burst raises the frame's mean brightness (0-1)
  by about 0.13 with bloom and without; bloom lifts the whole frame by
  about 0.05 either way.
- **Projectiles leaving the gun, first person** (2026-11-20,
  `launch.tf2`: one shot each from the bridge at 0 −300 0, 10° down along
  it, the view model drawn, every frame at 60 a second, the shot pressed
  at frame 1):
  - Rocket: from frame 2 a large yellow glow covers the launcher's front
    and the right of the view for about 10 frames (centred where the
    launch point, 23.5 forward, 12 right, 3 down, projects); the rocket's
    model first shows at frame 13, already about 150 units out; its grey
    smoke runs from the rocket back to the launcher; no streaks trail it.
    So the trail starts at the launch point and stays there until the
    drawn time reaches the shot, then follows the rocket; nothing of it is
    seen behind the eye.
  - Grenade Launcher: no muzzle flash is seen; the pipe first shows at the
    barrel's mouth at frame 8 and flies forward, a little left, arcing
    down.
  - Stickybomb Launcher (tapped): no muzzle flash is seen; the sticky first
    shows about 45 units ahead 8 frames after the release.
  - Syringe Gun (a puppet Medic watched in its eyes, fov 75): each needle
    first shows at the barrel about 6 frames after its shot and flies
    off with a faint thin trail.
  - Read together: a projectile is drawn at its launch point until the
    drawn time reaches its first moment, its model shown at once (it is
    inside the gun until then), the rocket's hidden longer (its first
    0.2 s).
- **Needles in the shooter's own view** (2026-11-21, `needles_fp.tf2`:
  the local Medic with the stock Syringe Gun swapped in, 2fort's bridge,
  10 degrees down, fov 90, view-model FOV 54, 60 frames a second, one
  tap, then the same while strafing right): the needle first shows one
  frame after the shot at the **left edge of the gun's drum**, below and
  left of where the muzzle attachment projects (the drum's front, where
  the flash would be), already about 30 units out, and flies up and left
  toward the crosshair; it never passes through the flash's place. Its
  places frame by frame fit the start of §12 (16 forward, 6 right, 8
  down of the eye) moved one frame along its flight, not the muzzle.
  The `muzzle_syringe` flash itself is barely visible (its core's alpha
  is tiny, particles.md 8). Strafing changes none of this.
- **Needles on a wall** (a puppet Medic firing 0.3 s into a wooden wall
  about 150 units off, then a free spectator beside the marks): no needle
  stays in the wall. Each hit leaves a small dark hole mark (its centre
  red seen head-on) and a few wood splinters; the marks are still there
  4 s later; seen from about 60° off the wall's normal they are flat dots,
  no shaft standing out.

## Test cases

| Setup | Expected |
|---|---|
| RED Scout fires the Scattergun, not crit, counter at 0 | 5 systems `bullet_scattergun_tracer01_red`, for pellets 1, 3, 5, 7, 9; one `muzzle_scattergun` |
| BLU Heavy fires 8 Minigun shots (32 bullets), counter at 0 | 8 `bullet_tracer01_blue` (unconfirmed base), no per-shot flash; `muzzle_minigun_constant` running once firing starts |
| Crit Shotgun shot, RED | tracers named `bullet_shotgun_tracer01_red_crit` |
| Rocket explodes on a wall (normal (1,0,0)) | wall effect oriented to (1,0,0); `Scorch` decal |
| Rocket explodes on a player | player effect; no decal |
| Rocket explodes mid-air (normal (0,0,0)) | player effect at zero angles |
| Pipe explodes under water | water effect of the Stickybomb Launcher's script |
| Soldier fires; another player watches | the watcher sees `rocketbackblast` at `backblast`; the Soldier never does |
| First person, world FOV 90, Scattergun muzzle in eye space (44.29, 5.22, −4.28) | flash and tracer start at (44.29, 10.25, −8.40) in eye space (right, up × 1.9626) |
| SMG bullet (8 damage) hits a BLU Heavy, normal violence, eyes in air, viewer 180 units away | one `blood_impact_red_01` and one `blood_spray_red_01` at the hit point, ~0.1 s after the hit; one `Impact.Flesh` decal on the body at once (shooter's view); 1 `Blood` world trace of 172 units onward along the shot, noise 0.1 |
| Same, viewer 500 units away | spray is `blood_spray_red_01_far` |
| Scattergun, 7 pellets hit one enemy | one burst + one spray (one damage event); 7 body decals; 7 world-decal traces (6 damage each: 1 per pellet) |
| Sniper body shot (50) | 4 world-decal traces, noise 0.3 |
| Victim's eyes under water | `water_blood_impact_red_01` only, no spray |
| `violence_hblood 0` | `lowV_blood_impact_red_01` + `lowV_blood_spray_red_01`; no body decal; no world decals |
| Rocket blast damages an enemy at 80 units | burst at the blast centre, axis toward the rocket; no body decal, no world blood decals |
| Wrench hits an enemy | burst at the Engineer's eye + 48 along his view; body decal at the swing's hit; 4 world-decal traces (65 damage) |
| Pyro's flames or afterburn while he holds the Flame Thrower | no blood |
| Teammate shot, friendly fire off | no blood, no body decal |
| Shotgun pellet into a teammate standing on a wall | no impact on the teammate |
| Melee swing hits a wooden wall (letter W) | one `Impact.Wood` decal at the hit and wood flecks, on the attacker's client only; "hit world" sound; no ricochet sound |
| Melee on a concrete wall / metal | `Impact.Concrete` + flecks / `Impact.Metal` + sparks |
| Rocket fired with the launch point under water | `rockettrail_underwater` (smoke + fire + burst + bubbles) for its whole flight |
| Rocket fired from above into water | `rockettrail` for its whole flight; it does not stop at the surface; it explodes on the bed |
| Blast point inside water (river bed) | the water effect (ours: `ExplosionCore_MidAir_underwater`), oriented to the bed's normal |
| Rocket fired, lerp 0.1, first interpolated client frame | the drawn rocket (and its trail point) 110 units behind its server position on its line; model hidden until 0.2 s |
| Rocket received at p0 = launch + (49.5, 0, 0), case B (§11.2), 60 frames/s | arrival frame: drawn at p0, `rockettrail` made there with 1 smoke puff, no fire or burst; next frame: 2 fire glows and 2 burst sparks at p0 (no spread), smoke at p0; at the second snapshot the drawn place jumps ~110–150 units back and that frame's particles lie along the jump, the newest on the rocket |
| Rocket's first snapshot, origin (100.4, 0.6, −20.5) on the server | received as whole units per component |
| Local Soldier crit-boosted, first person | `critgun_weaponmodel_<team>` + `_glow` on the arms view model's hitboxes (mostly hands and fingers), drawn in the view-model pass; 60 + 60 particles/s |
| Enemy Soldier crit-boosted, seen in third person | the same systems on his rocket launcher's world model hitboxes (2 boxes, nearly all on the gun's box) |
| Rocket blast at server time T, lerp 0.1 | the blast effect starts when the client's interpolated time reaches T (≈ T + 0.1 s + snapshot wait) |
| Rocket into the floor ~160 units ahead (25° down), 60 frames/s, `+attack` at frame 1 | blast at frame ~15 with lerp 0.1, ~11 with lerp 0.03 |
| Interpolation amount, defaults (`cl_interp` 0.1, `cl_interp_ratio` 2, `cl_updaterate` 20), tick 0.015 | 8 ticks = 0.12 s |
| Rocket first received at p0 = (100, 0, 0), s0 = 10.0, v0 = (1100, 0, 0); drawn time 9.9 | drawn at (−10, 0, 0); at 9.95: (45, 0, 0); at 10.0: p0; never held at a fixed point |
| Pipe received at client time 5.0 | model not drawn before 5.1; trail emitting from 5.0 |
| Rocket received at client time 5.0 | model not drawn before 5.2 |
| Stock sticky received at client time 5.0, 60 frames/s | pulse system made on the first frame at or after 5.8 (nothing drawn); first ring seen ≈ 5.8 + 0.1 + 1/60; rings every 0.1 s, 7 in all |
| Quickiebomb sticky received at client time 5.0 | pulse system made at 5.6 |
| RED needle hits a wooden wall (letter W) | `Impact.Wood` decal at the hit, wood flecks, `Wood.BulletImpact` within 1024; `Bounce.Shrapnel` on 3 in 10; needle stays stuck until 6 s after its launch; trail stops emitting |
| BLU needle hits the same wall | as above, never `Bounce.Shrapnel` |
| Needle reaches a teammate | needle vanishes; no decal, no sound, no blood |
| Needle (10 damage) hits an enemy | needle vanishes; server: one blood burst at the needle's server position, 2 world blood traces σ 0.2 |
| Rocket touches a floor at (0, 0, 0), normal (0, 0, 1) | blast centre (0, 0, 1), normal (0, 0, 1), wall effect, `Scorch` at (0, 0, 0) |
| Rocket touches a player's box face at x = 24, normal (1, 0, 0) | centre 1 unit outside that face; player effect oriented to (1, 0, 0); no scorch |
| Pipe on its fuse, centre 2 units above a floor | blast 1 unit above the floor, floor normal, wall effect, scorch on the floor |
| Pipe on its fuse 20 units above a floor | blast snapped to 1 unit above the floor (floor within 24 below) |
| Pipe on its fuse 30 units above a floor | blast at its centre, zero normal, in-air effect, no scorch |
| Sticky stuck to a wall, floor 100 units below, detonated | blast at the sticky's centre, the wall's outward normal, wall effect, no scorch |
| Pipe grazes an enemy, its 64-unit line misses | blast at the pipe's centre, in-air effect, no scorch |
| Rocket hits a river bed under 50 units of water | centre 1 unit above the bed, water effect along the bed's normal |
| Melee: line from the eye hits a wall 40 units ahead | contact on the wall on the view line; decal and flecks there |
| Melee: line misses, cube stops at C touching a player | contact C (the cube centre); body decal only if the eye→C ray (+8, +10 %) meets a hitbox |
| Melee: line misses, cube touches the world, doubled line hits | contact = the doubled line's hit (≤ 96 units out) |
| Medic gets Kritzkrieg crits holding the Syringe Gun | `Weapon_General.CritPower` loop starts at full volume, `critgun_weaponmodel_<team>` on the gun; stops cut when the boost ends |
| Boosted Medic switches to the Bonesaw (boostable) | the loop goes on without restarting; the glow moves to the Bonesaw |
| Boosted enemy Spy cloaks | glow and loop stop for every viewer |
| Random crit shot, no boost | no CritPower loop; `…SingleCrit` shot sound only |
| Flame Thrower burst turns critical while firing | FireLoop cut, FireLoopCrit at full volume at once, no FireStart, no FireEnd |
| Player healed to full health | all his body decals removed |

## Open questions

1. **The scripts' effect fields.** Read `MuzzleFlashParticleEffect`,
   `TracerEffect`, `ExplosionEffect`, `ExplosionPlayerEffect`,
   `ExplosionWaterEffect`, `BrassModel` and `MuzzleFlashModel` of the
   twelve stock guns with the dumper (`dump tf2 --script <name>`) once the
   local `.ctx` key is configured for it, and replace every "pcf" and
   "unconfirmed" mark in §1. In particular: the Rocket Launcher's muzzle
   flash (a model flash? none?), the SMG's, Revolver's and Minigun's
   tracer bases, the Sniper Rifle's tracer, and which explosion name sits
   in which field.
2. Whether any stock gun has a `MuzzleFlashModel` (a model flash in
   addition to or instead of the particle one).
3. The fallback name's case (`ExplosionCore_wall` vs the file's
   `ExplosionCore_Wall`): particle names are looked up case-insensitively
   in Source as far as we know; unconfirmed.
4. The trail tracer's lifetime and the length of its streak are set by
   the particle library (start-to-end mover and trail renderer;
   particles.md Open questions).
5. (Implementation note.) Whether bullets of a gun with no tracer (the
   Sniper Rifle) advance the shared tracer counter (§6 says "every bullet
   the client draws"). Ours counts only bullets of guns with a tracer.
6. (Implementation note.) What surface normal a pipe on its fuse and a
   detonated sticky send (§5 step 2 reads the normal "sent with it").
   Ours sends none for them (the in-air effect, no scorch) until a spec
   says; rockets and direct pipe hits send the surface they touched.
   **Answered (§13):** a fused pipe or detonated sticky sends the normal of
   a floor found within 24 units below it (or zero); a stuck sticky sends
   the normal of the surface it stuck to.
7. (Implementation note.) How far the `Scorch` decal's trace reaches
   (§5 step 6). Ours traces 16 units from the blast's centre back along
   the surface normal.
   **Answered (§13):** no separate trace: the scorch uses the blast's own
   trace (the rocket's touch, the pipe's 64-unit line, or the fused
   projectile's line from 8 above to 24 below its centre).
8. **Blood on a player hit** (measured 2026-11-20, tools/tf2ref
   `impacts.tf2`: an SMG round into a BLU Heavy about 180 units off,
   every frame at 60 frames a second): a dark red puff about 20 pixels
   across at the hit (on the Heavy's hand and belt) from the first frame,
   fading over about 8 frames, and a bright red splat on the body by
   frame 8. Ours (Source's shared impact table, `blood_impact_red_01`)
   draws a speck of a few pixels. Which system or table TF2 uses is
   unspecified.
   **Answered (§8):** TF2 does not use the shared impact path's blood; it
   sends its own blood temp entity from the damage path, drawing
   `blood_impact_red_01` (goop radius 6–30, 32 chunks, droplet trails)
   plus `blood_spray_red_01`/`_far` (overbright 31: bright red), about
   0.1 s after the hit (§11); the body gets an `Impact.Flesh` decal at
   once from the bullet's impact. Which of the two the "dark puff from
   the first frame" and the "bright splat by frame 8" are is not
   settled by that capture; by the timing rule the decal comes first and
   the burst and spray ~6 frames later.
9. The "rocket meets the floor farther away" of the streaks comparison:
   §11 explains "later", not "farther" (TF2's rocket should land a few
   units nearer than the crosshair). Compare the blast's screen position
   with ours at the same eye, with the 200-unit aim rule in both.
10. Whether the server's blood temp entity reaches the shooter himself
   on a dedicated server (the engine may hold back a shooter's own
   events during his command; the listen-server capture shows him blood).

11. **Where a projectile drawn late starts** (§11 against the captures
   above). §11 gives the client a history point one second back along the
   rocket's velocity, so the drawn rocket would start about 110 units
   behind its launch point. The first-person captures show the rocket's
   trail glowing at the launch point from the frame after the shot and
   nothing behind the eye; a pipe and a needle show at the barrel as their
   drawn time reaches the shot. Ours holds a projectile at its launch point
   until then (measured). Unexplained: whether the history point exists
   but is not used for the first interpolation.
   **Answered (§11.1):** the history point exists and is used from the
   first drawn frame: the projectile is drawn behind its launch point and
   moves forward at once; the "launch point" look is the hidden model
   (rockets 0.2 s, pipes and stickies 0.1 s from arrival) and the trail's
   glow passing the camera. Needles are client-made from their start (§12).
12. (Implementation note.) Ours' needle impacts: §12 replaces any
   needle-specific guess; ours' decal on the world should follow the
   letter table.
13. **Stuck needles.** By the reference a needle hitting the world stays
   drawn until 6 s after its launch; the 2026-11-20 capture (a puppet
   Medic, then a free spectator beside the marks) saw none. Check from the
   shooter's own first-person view without changing view mode, and which
   end of `w_syringe_proj.mdl` (bounds −5.9 to +3.25 along x) is the tip.
14. The surface material reported for a hit on a player's box (we infer
   the default surface, letter C): shoot needles into a standing bot and
   look for concrete flecks and bullet-hole decals on the body, and
   which impact sound plays.
15. The engine's per-model decal cap (`r_maxmodeldecal`, believed 50).

16. **Is a new projectile interpolated in its arrival frame?** (§11.2.)
   The SDK's client puts an entity on the interpolation list when its
   state is latched only if it is already registered for drawing; the
   engine's order for a new entity is outside the reference. The launch
   captures fit "no" (case B: drawn at its received place until the
   second snapshot). Check: `launch.tf2` with `cl_interp 0.5` — case B
   keeps the glow at p0 for the same few frames and the rocket then
   jumps ~550 units back; case A shows nothing ahead of the launcher for
   ~0.5 s. With `cl_updaterate 66` case B's glow at p0 lasts about one
   frame's worth of particles.
   **Observed (2026-11-20, tf2-psys2, `launch.tf2`'s rocket at 60
   frames/s; `cl_interp` and `cl_updaterate` only change before the map
   loads, `notconnected`):** not case A: with `cl_interp 0.5` the glow
   ahead of the launcher shows from frame 3 as with the defaults. But the
   rocket stays at p0 longer than one snapshot: with the defaults the
   fire glows and fresh burst sparks are made at p0 through frame ~9 and
   the rocket is seen moving on from frame ~10–12 (no jump back seen);
   with `cl_updaterate 66` (p0 next to the launcher) the same, at p0
   through frame ~9; with `cl_interp 0.5` the glow builds at p0 through
   frame ~19, the model (shown from frame 14, after its 0.2 s) standing
   still there, then gone from p0 at frame 20 (thrown back behind the
   eye), and an orange glow passes the camera's top right at frames
   24–26. So the place is held while the drawn time is before about the
   first snapshot's time at the defaults (when the drawn time reaches
   p0's), but not as long with lerp 0.5 (released after ~0.3 s, the
   rocket thrown back). Ours (implementation) draws the copy at p0 while
   the drawn time is before its arrival, then interpolates: it matches
   the defaults, not lerp 0.5.
17. The `Oscillate Scalar` operator's formula (rate, frequency,
   multiplier, start phase, proportional start/end times) used by the
   crit glow on radius and alpha (§14.1): not read; implement as a
   per-particle sine on the field until a capture or a spec settles it.
   **Answered (2026-11-20, tf2-spec-flashup, read from the particle
   library):** particles.md §10.9 (Oscillate Vector too).
