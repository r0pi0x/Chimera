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
restated in our words. **The per-weapon `.ctx` scripts were not read in
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
  particles.md §8.3).

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
  at frame 2) and repeats every 0.1 s: per pulse a thin orange ring the
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
| Rocket fired, lerp 0.1, first client frame with the rocket | the drawn rocket (and its trail point) 110 units behind its server position on its line; model hidden until 0.2 s |
| Rocket blast at server time T, lerp 0.1 | the blast effect starts when the client's interpolated time reaches T (≈ T + 0.1 s + snapshot wait) |
| Rocket into the floor ~160 units ahead (25° down), 60 frames/s, `+attack` at frame 1 | blast at frame ~15 with lerp 0.1, ~11 with lerp 0.03 |

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
7. (Implementation note.) How far the `Scorch` decal's trace reaches
   (§5 step 6). Ours traces 16 units from the blast's centre back along
   the surface normal.
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
