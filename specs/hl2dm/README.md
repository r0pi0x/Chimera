# Half-Life 2: Deathmatch specs

Behaviour of Half-Life 2: Deathmatch (HL2:DM, Steam app 320, game folder
`hl2mp`), written in our own words for Chimera's third game library
(`games/hl2dm`). The plan, the waves and the briefs are in
[docs/plans/active/hl2dm.md](../../docs/plans/active/hl2dm.md), section 4.
The rules for writing specs are in [specs/README.md](../README.md).

Spec sessions for HL2:DM run on the Linux box as separate sessions (plan,
decision log, 2026-11-01); implementation sessions read only these files
and our code.

## Read first

HL2:DM is a Source game, like CS:S. Most of Source's engine behaviour is
already specced in [specs/source/](../source/) and in the Source-engine
parts of [specs/cs_source/](../cs_source/). **[source_deltas.md](source_deltas.md)
(wave 0, H0) says, spec by spec, what holds for HL2:DM as written and what
differs, with the numbers, and records the install's facts.** Every later
HL2:DM spec continues it and must not contradict it.

## Index

| H | File | Wave | Status | Covers |
|---|---|---|---|---|
| H0 | [source_deltas.md](source_deltas.md) | 0 | draft | each existing Source spec: same or what differs; install facts (search path, VPKs, stock maps, BSP and MDL versions, builds, tick, default cvars, player models, weapon scripts, SourceMod on the probe server) |
| H1 | [movement.md](movement.md) | 1 | draft | movement deltas: speeds, sprint, gravity and jump, ladders (`func_useableladder`), water and drowning, fall damage |
| H2 | [suit.md](suit.md) | 1 | draft | health, armour split, aux power, flashlight, breath, pickups and chargers |
| H3 | [weapons.md](weapons.md) | 1 | draft | inventory, switching, ammo, bullet weapons, melee |
| H6 | [rules.md](rules.md) | 1 | draft | DM and TDM, spawns, respawn, limits, intermission, scoring, teams, models |
| H4 | [projectiles.md](projectiles.md) | 2 | draft | SMG1 grenade, AR2 ball, RPG, crossbow, frag, S.L.A.M. |
| H5 | [gravity_gun.md](gravity_gun.md) | 2 | draft | gravity gun, `+use` carry (off by default), physics damage and credit |
| H7 | [player_models.md](player_models.md) | 2 | draft | models, animation, hitboxes, ragdolls, view models |
| H8 | [hud.md](hud.md) | 3 | draft | HUD panels, weapon selection, death notices, scoreboard |
| H9 | [effects_sounds.md](effects_sounds.md) | 3 | draft | effects and sounds |
| H10 | [maps_entities.md](maps_entities.md) | 3 | draft | HL2:DM's map entities beyond specs/source/ |
| – | [game_ui.md](game_ui.md) | – | draft | GameUI data: main menu, Multiplayer options (models, sprays), Create Server |

The "Covers" lines are short; the full brief for each item is in the plan.

## Conventions shared by every HL2:DM spec

- Units: Source units (inches, 1 unit = 0.0254 m), Z up, X forward, Y
  left; angles in degrees; time in seconds and server ticks.
- Tick: 0.015 s (66.67 Hz) by default, as CS:S; HL2:DM's server honours a
  `-tickrate` launch option, CS:S's does not (source_deltas.md, "Tick and
  network").
- Gravity: 600 units/s² (CS:S: 800). Every number derived from gravity in
  a CS:S spec must be re-derived, not copied.
- Steam builds checked: say which in each spec (H0 used game build
  24600434, dedicated server build 24600426, `PatchVersion` 10889068).
