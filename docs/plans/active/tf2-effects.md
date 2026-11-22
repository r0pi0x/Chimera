# Plan: TF2's weapon effects from its own data

Started 2026-11-18, from the playtest's "missing explosion and visual
effects (the sticky bomb launcher, and likely others)" (docs/backlog.md).

## What was wrong, generally

- **A kept guest dressing dropped its particle systems.**
  `map::kept::append` copied a guest's particle materials but not its
  `MapData::particle_systems`, so TF2's explosion, muzzle flashes and
  flames (`pcf::add_guest_particle_systems`, run by its dressing on the
  empty map) reached no map but TF2's own once dressings were kept
  (2026-11-17). Every game whose dressing adds systems lost them the same
  way. Fixed: `kept::add_particle_systems` (test
  `a_guests_particle_systems_are_appended`).
- **Stickies went off only with the launcher out.** A ladder's rung swap
  took the launcher and the stickies stayed forever, never exploding.
  Fixed: attack2 with any TF2 weapon out (the game: any of the
  Demoman's), test `stickies_detonate_with_another_tf2_weapon_out`.

## What is left (per weapon; as first found, see Progress)

Effect names are a table in `games/tf2/src/game/weapons/looks.rs`, not
data; the weapon scripts' own names (`TracerEffect`,
`MuzzleFlashParticleEffect`, `ExplosionEffect`, `ExplosionPlayerEffect`,
`ExplosionWaterEffect`: specs/tf2/weapon_scripts.md) are read nowhere.

| Weapon | Missing |
|---|---|
| Scattergun, Shotgun, Minigun, Pistol, SMG, Revolver, Sniper Rifle | tracers (none of TF2's guns writes a `map::tracer::Tracer`) |
| Rocket Launcher | muzzle flash (no `MUZZLE` entry), the rocket's trail |
| Grenade and Stickybomb Launchers | the pipes' trails, the stickies' glow |
| Every explosive | one explosion (`ExplosionCore_MidAir`) whatever it hit; no scorch decal |
| Syringe Gun | the needles' trail |
| Flame Thrower | the airblast's effect |

## Steps

1. Read the scripts' effect fields into the weapons' data (one `[looks]`
   table per gun: muzzle, tracer, explosion by context), with a spec
   section first saying what each field means and when the game uses
   which explosion; `looks.rs` then reads the data, and
   `PARTICLE_SYSTEMS` is built from it.
2. Tracers: the spec's rate and look for TF2's bullet guns (a spec
   question first: how often a tracer is drawn), through Chimera's
   `Tracer`.
3. Trails and glows on flyers (a system following a `ShownItem`:
   `StartParticleSystem::follow`).
4. Scorch decals on explosions (the map's decal path, TF2's group).

## Progress

- 2026-11-18: step 1 begun. Each gun's muzzle flash is data now
  (`[hooks."tf2:looks"]` `muzzle`, read by `looks::LooksParams`;
  `PARTICLE_SYSTEMS` became `looks::particle_systems()`, built from the
  data). Test `every_bullet_gun_and_launcher_names_its_muzzle_in_data`.
- 2026-11-19: the specs arrived (specs/tf2/effects.md, particles.md;
  flames.md is its own task). Done from them, all data in
  `[hooks."tf2:looks"]`: the first-person flash at the view model's
  muzzle re-projected into the world view and drawn as a world system
  with the world FOV (it was the view-model camera's, 1.96x too big at
  90; `StartParticleSystem::view_model` is gone); the Minigun's
  `while_firing` constant muzzle and brass instead of a per-shot flash;
  tracers (`tracer`, `tracer_every`, one shared count, `_red`/`_blue`,
  control point 1 and "move particles between 2 control points" in
  Chimera's psys); the explosion by water, air or player, or surface
  (`explosion`, `explosion_of`) oriented to the normal, and the
  `Scorch` decal; the Rocket Launcher's backblast for others; sprite
  cards' size limits as fractions of the distance. Tests: tf2 `effects`,
  `flashes`, `looks` unit tests; chimera_present psys and
  `particle_depth`; chimera_world `particles`. Left: trails and glows on
  flyers (step 3), the scripts' own names (effects.md Open questions 1),
  sheet frame blending, `$addself`, the flames (flames.md).
- 2026-11-20 to 11-21 (tf2-parity's passes): step 3 done: `[hooks."tf2:looks"]
  trail` on every flyer (`rockettrail`/`_underwater`, `pipebombtrail_`,
  `stickybombtrail_`, `nailtrails_medic_` per team), the sticky's armed
  pulse (`armed`), late copies' first frames (effects.md 11.1-11.3);
  `$addself` as particles.md 2 (particle.wesl); the flames (flames.md).
- 2026-11-22 (tf2-trails): the rocket's trail on its model's `trail`
  attachment (effects.md 1; the data's `at`, `LooseAsset::attachments`),
  not the model's bounds centre; sprite-sheet frames mixed with the next
  (particles.md 3, `$blendframes`, every Source game's sprite cards).
  Tests `effects::a_rockets_trail_rides_its_models_trail_attachment_1`,
  `particles::tests::sheet_frames_mix_with_the_next_3`. Left, all needing
  measurements (tf2-parity.md, "Measurements for the remaining effects"):
  the airblast's `pyro_blast` (no airblast yet: weapons_more.md 7.4 is
  reference only), the scripts' own names, crit glows on flyers (crits
  are off), the sheet frame rate (particles.md Open questions 1).

## Spec gaps (as of 2026-11-18; answered by specs/tf2/effects.md and particles.md on 2026-11-19 except effects.md Open questions 1)

Implementation reads specs only, and specs/tf2 has no value for any of
these (weapon_scripts.md only says the script fields exist and are
ignored; weapons.md 8.1 gives the rocket's `trail` attachment and says
its trail is a particle effect, without a name). A spec session with the
install must write them (per gun, values only) before tracers, trails and
explosion variants can be data:

1. **Tracers**: each bullet gun's `TracerEffect` name (a particle system
   or a streak?), whether a tracer is drawn every shot or every Nth,
   its speed and where it starts (the `muzzle` attachment is known), and
   the Sniper Rifle's.
2. **Trails**: the particle system of the rocket (attachment `trail`
   known), the pipe grenade, the sticky (and its glow), the syringe
   needle, per team if it differs.
3. **Explosions**: `ExplosionEffect`, `ExplosionPlayerEffect`,
   `ExplosionWaterEffect` per explosive, and the rule choosing between
   them (hit a player, water, midair, ground); the scorch decal.
4. **Rocket launcher muzzle flash** system name; the airblast effect.

Mechanisms ready to take the data once it exists: `LooksParams` (add
`tracer`, `trail`, `explosion*` fields), `StartParticleSystem::follow`
for a trail on a flyer (needs a flyer-following start: a small addition
in Chimera when the first trail name arrives), Chimera's
`map::tracer::Tracer` for streak tracers (its look comes from the data).
