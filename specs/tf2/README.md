# Team Fortress 2 specs

Behaviour and data layouts of Team Fortress 2 (TF2, Steam app 440, its
dedicated server app 232250, game folder `tf`), written in our own words
for Chimera's TF2 game library (`games/tf2`). The plan, its milestones
and the decisions are in
[docs/plans/active/tf2.md](../../docs/plans/active/tf2.md). The rules
for writing specs are in [specs/README.md](../README.md).

Spec sessions for TF2 run on the Linux box as sub-agents of the
orchestrating session. They read the game's data files in place, public
documentation, and measurements of the dedicated server. They never read
TF2's, the Source engine's or the Source SDK's code. Implementation
sessions read only these files and our code.

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
| T1 | [movement.md](movement.md) | draft | movement cvars as measured, per-class speeds, duck (0.2 s, air duck), jump (289), the Scout's air jump, the bunny-hop cap (1.2 x), fall damage |
| T2 | [classes.md](classes.md) | draft | the nine classes: health, overheal and its decay, speeds, eye heights, models, stock loadouts, Medic regeneration, health and ammo pickups, teams |
| T3 | [weapons.md](weapons.md) | draft | shared gun rules (distance ramp, damage spread, crits, pellets, self-damage and knockback, switching) and the Scattergun, Rocket Launcher, Shotgun and Minigun with their assets and sounds |
| T4 | [ice.md](ice.md) | draft | the ICE block cipher from its published paper, and how the encrypted `.ctx` scripts use it; the paper's test vectors (not yet reproduced from the description alone: see its open questions) |
