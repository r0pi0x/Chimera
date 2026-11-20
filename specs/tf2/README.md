# Team Fortress 2 specs

Behaviour and data layouts of Team Fortress 2 (TF2, Steam app 440, its
dedicated server app 232250, game folder `tf`), written in our own words
for Chimera's TF2 game library (`games/tf2`). The plan, its milestones
and the decisions are in
[docs/plans/active/tf2.md](../../docs/plans/active/tf2.md). The rules
for writing specs are in [specs/README.md](../README.md).

Spec sessions for TF2 run on the Linux box as sub-agents of the
orchestrating session. They read the game's data files in place, public
documentation, and measurements of the dedicated server. Since 2026-11-15 they
may also read the Source SDK 2013 (updated 2025-02-18 with TF2's client
and server game code, under the SDK's non-commercial licence) in place,
outside this repository, as reference for specs only; nothing of it is
copied or translated (specs/README.md). They never read leaked code.
Implementation sessions read only these files and our code.

## Read first

TF2 is a Source game, like CS:S and HL2:DM. Most of Source's engine
behaviour is already specced in [specs/source/](../source/), the
Source-engine parts of [specs/cs_source/](../cs_source/) and
[specs/hl2dm/source_deltas.md](../hl2dm/source_deltas.md).
**[install.md](install.md) records the install's facts and what differs
from HL2:DM.** Every later TF2 spec continues it.

## Index

| T | File | Status | Covers |
|---|---|---|---|
| T0 | [install.md](install.md) | draft | the two install layouts (client and dedicated server), gameinfo's search path, the VPKs, the maps (BSP v20 with LZMA lumps, static props v10), ctf_2fort and cp_badlands in detail, `info_player_teamspawn`, materials and shaders, the `c_` arms-plus-weapon rig versus `v_` models, player models, `items_game.txt`'s layout, which scripts are plain and which encrypted |
| T1 | [movement.md](movement.md) | checked against the SDK's game code | movement cvars as measured, per-class speeds, duck (0.2 s, air duck), jump (289), the Scout's air jump, the bunny-hop cap (1.2 x), fall damage |
| T2 | [classes.md](classes.md) | checked against the SDK's game code | the nine classes: health, overheal and its decay, speeds, eye heights, models, stock loadouts, Medic regeneration, health and ammo pickups, teams |
| T3 | [weapons.md](weapons.md) | checked against the SDK's game code | shared gun rules (distance ramp, damage spread, crits, pellets, self-damage and knockback, switching) and the Scattergun, Rocket Launcher, Shotgun and Minigun with their assets and sounds |
| T4 | [ice.md](ice.md) | draft | the ICE block cipher from its published paper, and how the encrypted `.ctx` scripts use it; the paper's test vectors (not yet reproduced from the description alone: see its open questions) |
| T5 | [weapons_more.md](weapons_more.md) | checked against the SDK's game code | the Pistol, SMG, Sniper Rifle (charge, headshots), Revolver, Grenade Launcher, Stickybomb Launcher, Flame Thrower and Syringe Gun: numbers, projectiles, assets, sounds, kill icons |
| T6 | [hud.md](hud.md) | checked against the SDK's game code | the scheme's fonts and colours, the layout's scaling, the health, ammo, crosshair and kill feed panels, their animation events, the kill icons, the hit sound |
| T7 | [player_animation.md](player_animation.md) | checked against the SDK's game code | the class player models' animations: include models, pose parameters, the move and aim grids, sequences by weapon kind, gestures, the spun minigun set, the hand and arm pose libraries (`r_hand_grip`, `r_arm`), what moves the legs at rest (the feet yaw lag), which weapons a player shows, the held weapon's bone, hitboxes, ragdolls |
| T8 | [weapon_scripts.md](weapon_scripts.md) | checked against the SDK's game code | the per-weapon `.ctx` scripts' fields: meaning, unit, type, which guns carry them, and how each maps onto our weapon TOML and hook params (applied, checked or ignored); no values |
| T9 | [viewmodel.md](viewmodel.md) | checked against the SDK's game code | the first-person view model: the class arms plus the bone-merged `c_` weapon, placement at the eye (no offset), bob, no sway, `viewmodel_fov` 54, draw and idle sequences and their cross-fades, the Minigun's rest pose (`m_idle`, barrels up), its spin sequences and procedural barrel; why ours showed the spun pose too big |
| T10 | [weapons_stock.md](weapons_stock.md) | draft | the nine stock melee weapons (Bat, Shovel, Fire Axe, Bottle, Fists, Wrench, Bonesaw, Kukri, Knife): damage, interval, smack delay, reach, melee crits, the Spy's backstab (three angle conditions, 6 x current health), the Fists' left and right punches, the Bottle's break, their scripts' mapping, sequences, sounds, models and kill icons; the rest of the stock loadout (out) |
| T11 | [faces.md](faces.md) | draft | the players' faces: the idle expression scene the server keeps looping per class and state (normal, hurt, round lost, comp winner, scoped, Minigun), how its presets set the flex controllers (resting values per class; the Engineer's `defaultFace` about 1.0; the ramps' Catmull-Rom curve bulges above its samples), firing faces, no blinking, eye aim (300 u, living players only, no head turn), dead players' and ragdolls' faces, the Pyro; the scenes.image, binary scene and .vfe layouts |
| T12 | [effects.md](effects.md) | draft | each stock gun's effect names (muzzle flash, tracer base and its team/crit suffixes, trails, crit glows, explosions by wall/air/water, backblast, the Minigun's continuous flash and brass), the tracer rate (every 2nd bullet, the Minigun's every 4th), where first-person effects start; blood (burst, body decals, world decals), under water, melee contact points, when projectiles and blasts show on a client (seeded interpolation), needle impacts, blast centres, crit-boost loops; per-gun script fields still unconfirmed |
| T13 | [particles.md](particles.md) | draft | particle rendering: SpriteCard blend modes, overbright and add-self, sheet frames, size clamps and fades, depth feathering, renderers, the extra operators, and why first-person particles are drawn at the world FOV (worked examples: Scattergun, Shotgun, Minigun flashes) |
| T14 | [flames.md](flames.md) | draft | the Flame Thrower's points as every client runs them, the damage volume, and the `new_flame` particle system driven by the points as control points: how the visible fire lines up with where it hurts |
| T15 | [hdr_bloom.md](hdr_bloom.md) | draft | integer HDR, auto exposure (the luminance histogram, the 2 %-at-60 % target, its goal smoothing, the map's range), the bloom (soft-threshold downsample, 13-tap blur, gamma-space add, clamps), `env_tonemap_controller`'s inputs and every map's values (2fort 0.8–1.1, bloom 0.5), `mat_hdr_level` 0/1/2, `mat_disable_bloom`, LDR maps, particles' overbright under HDR |
| T16 | [minigun.md](minigun.md) | draft (checked against the SDK and measured) | the Minigun's five states and every transition by attack/attack2/ammo with times, its six sounds (entries, channels, levels, which loop by their WAV cue, one sound at a time, cut rules, the wind-down cut by the barrel), running dry, attack2 refused when empty, the auto-switch, what other players hear (state-driven, no sound messages), the barrel spin's numbers; with [../source/sound_looping.md](../source/sound_looping.md) on how Source decides a WAV loops |
