# Halo: Combat Evolved specs

Behaviour and file formats of Halo: Combat Evolved as the original Xbox
disc has it, written in our own words for Chimera's fourth game library
(`games/halo_ce`, with the loader crate `blam_formats`). The plan, the
waves and the milestones are in
[docs/plans/active/halo-ce.md](../../docs/plans/active/halo-ce.md). The
rules for writing specs are in [specs/README.md](../README.md); this
port adds the ones below.

## This port's rules for spec writers

- The behaviour reference is a decompilation under a copyleft licence.
  Nothing of it enters this repository: no code, no pseudocode that
  follows it line by line, no function, type or file names, no comments
  or strings. Describe what happens, organised by behaviour. Names that
  are data the game itself exposes are fine: tag class codes and names,
  tag field names as the tags carry them, tag paths, map and file names.
- No code blocks at all (a test fails on one): prose, tables and math.
- **The disc is the ground truth for formats and numbers.** The
  reference may describe another build of the game than the Xbox disc.
  Check every layout and every number against the disc's own files (read
  them with throwaway scripts kept outside the repository) and say in
  the spec what was checked against which map. Where the reference and
  the disc differ, the disc wins and the spec says so.
- Each spec states its confidence per section (checked against the disc,
  read from the reference only, inferred) and ends with open questions.
- One conventions source: [cache_file.md](cache_file.md) defines the
  units, axes, the tick, and how tag data, blocks, data references and
  tag references are stored. Other specs refer to it and don't define
  them again.
- Specs stay `draft` until the user reviews them; the orchestrating
  session checks each for completeness and consistency before code is
  written from it.

## Index

| File | Wave | Status | Covers |
|---|---|---|---|
| [disc_image.md](disc_image.md) | 0 | draft | the Xbox disc image's file system |
| [cache_file.md](cache_file.md) | 0 | draft | the map cache file, the tag index, tag data conventions, tag classes, units and tick |
| [bitmaps.md](bitmaps.md) | 1 | draft | bitmap tags and pixel formats |
| [structure_bsp.md](structure_bsp.md) | 1 | draft | the level's render geometry, lightmaps, clusters and visibility |
| [collision.md](collision.md) | 1 | draft | collision BSPs and how a trace walks them |
| [models.md](models.md) | 1 | draft | render models |
| [shaders.md](shaders.md) | 1 | draft | the shader classes and what each draws |
| [scenario.md](scenario.md) | 1 | draft | spawns, netgame equipment, object placements, the sky |
| [sounds.md](sounds.md) | 1 | draft | sound tags and the Xbox's compressed format |
| animations.md | 1 | planned | animation graphs and compressed node animations |
| [objects.md](objects.md) | 2 | draft | object, unit, biped, item and scenery tags as data |
| [movement.md](movement.md) | 2 | draft | the player's movement |
| health_shields.md | 2 | planned | vitality, shields, damage by material and region |
| [weapons.md](weapons.md) | 2 | draft | weapons, with every multiplayer weapon's numbers |
| projectiles.md | 2 | planned | projectiles, damage effects, grenades |
| multiplayer_rules.md | 2 | planned | spawning, pickups, powerups, game type rules |
| effects.md | 3 | planned | effects, particles, lights, decals |
| hud.md | 3 | planned | the HUD's pieces |
| vehicles.md | 3 | planned | vehicles |

A file's link is added here when it lands.
