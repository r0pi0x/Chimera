# Team Fortress 2: weapon effects (muzzle flashes, tracers, trails, explosions)

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
