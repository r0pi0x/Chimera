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

## What is left (per weapon)

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
