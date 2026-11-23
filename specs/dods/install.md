# Day of Defeat: Source: the install, its files and its maps

Source basis: the dedicated server install (Steam app 232290, SteamCMD,
anonymous; build 11003710 as the server reports it), its directory
listing, the `_dir` VPK's file table, the map files' headers and entity
lumps' class names, and the server's own start-up log. Read in place;
nothing extracted is kept.
Status: draft (measured 2026-11-23)

## Summary

DoD:S is a Source game of the same branch as CS:S: BSP version 20 maps,
a VPK v2 content archive, plain-text KeyValues scripts. The dedicated
server's files are complete enough for everything the party needs
(maps, textures, sounds, models, scripts), unlike TF2's server.

## Layouts

| Layout | Steam app | Root holds |
|---|---|---|
| Retail client | 300 | `dod/`, `hl2/`, `platform/`, the client binaries |
| Dedicated server | 232290 (anonymous) | `dod/`, `hl2/`, `platform/`, `srcds_run`, `srcds_linux` (32-bit) |

## The search path (`dod/gameinfo.txt`)

`SteamAppId` 300. Search paths in order, all `game` (the first three
also `mod`):

1. `dod/custom/*`
2. `dod/dod_english.vpk` (not in the server layout: skip when absent)
3. `dod/dod_pak.vpk` (`dod_pak_dir.vpk` plus `dod_pak_000` ... `_006`)
4. `hl2/hl2_textures.vpk`, `hl2/hl2_sound_vo_english.vpk`,
   `hl2/hl2_sound_misc.vpk`, `hl2/hl2_misc.vpk`
5. `platform/platform_misc.vpk`

`|all_source_engine_paths|` resolves to the install root. Loose folders
also exist (`dod/maps`, `dod/materials`, `dod/resource`, `dod/cfg`).

## Content counts (server layout, `dod_pak_dir.vpk`)

| Kind | Files |
|---|---|
| `.vtf` textures | 1,699 |
| `.vmt` materials | 1,615 |
| `.wav` sounds | 800 |
| `.mp3` | 12 |
| `.mdl` models | 819 |

The VPK is version 2 (the archive index `0x7fff` means "data in the
`_dir` file").

## Maps

Nine stock maps in `dod/maps/`, all `VBSP` version 20: `dod_anzio`,
`dod_argentan`, `dod_avalanche`, `dod_colmar`, `dod_donner`,
`dod_flash`, `dod_jagd`, `dod_kalt`, `dod_palermo`.

Spawn points: `info_player_allies` and `info_player_axis` (20 each on
dod_avalanche and dod_flash). Objectives: `dod_control_point`,
`dod_capture_area` (5 each on those maps), plus on bomb maps the bomb
targets (not surveyed). Other notable classes: `prop_physics_multiplayer`
(over 200 per map), `func_teamblocker` (team-only walls: open for free
for all), `info_ladder` (the server reports it as an unknown entity:
ladders are brush-based in practice), `env_smokestack`, `func_breakable`.

Team numbers as the server uses them: 2 Allies (US), 3 Axis.

## Scripts

`scripts/` in the VPK holds per-weapon scripts `weapon_<id>.txt` (29 in
the manifest, weapons.md), per-class scripts
`playerclass_{us,axis}_<id>.txt` (12, classes.md), sound scripts,
`surfaceproperties.txt`, `hudlayout.res`, `hudanimations.txt`. Each
weapon and class script also exists as a `.ctx` of the same size; the
`.txt` copies are plain KeyValues and are the ones to read.

## Models

- Player bodies: `models/player/{american,german}_{assault,mg,rifleman,rocket,sniper,support}.mdl`,
  **MDL version 44**, about 20 KB each (mesh and hitboxes; animation
  comes from the shared model).
- Shared animations: `models/player/dod_player_shared.mdl`, MDL
  version 48, 3.3 MB.
- View models: `models/weapons/v_<gun>.mdl` (MDL 44; one model holding
  arms and gun): 30cal, amerk (knife), bar, bazooka, c96, colt, frag,
  garand, garand_rg, garand_rglive, k98, k98_rg, k98_rglive, k98_scoped,
  m1carbine, mg42, mp40, mp44, p38, panzerschreck, smoke_ger, smoke_us,
  spade, springfield, stick, thompson, tnt.
- World models `w_<gun>.mdl`; the MGs have extra world models for the
  deployed and reloading states (`w_mg42bd`, `w_30calsr`, `w_30calpr`;
  weapons.md).

## Server facts

- Tick: the server sets 66.7 Hz (0.015 s) at map start.
- The server is 32-bit (`srcds_linux`, `bin/` 32-bit with a `linux64`
  folder of 64-bit server libraries).

## Test cases

| Setup | Expected |
|---|---|
| mount the server layout | `scripts/weapon_garand.txt` reads; `dod_english.vpk` missing is skipped |
| list maps | the nine names above |
| read a map header | `VBSP`, version 20 |
| spawns of dod_avalanche | 20 `info_player_allies`, 20 `info_player_axis` |

## Open questions

1. Does the retail client's layout differ (more VPKs, `dod_english.vpk`
   holding the display strings)? Check when the client is installed.
2. Do any stock maps use LZMA-compressed lumps or pak entries (TF2's
   do)? The headers were not surveyed lump by lump.
