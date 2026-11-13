# Map pool fitness

Started 2026-11-13. Goal: a bigger Ultimate Gun Game pool (more CS:S maps,
custom arenas, more standard Combat Arms maps) without maps so big that
nobody finds anybody; tiny chaotic maps welcome. This plan measures every
installed map, proposes a rule that picks the pool by player count, and
the data shape that would carry it. Nothing here is wired in yet:
`auto::map_pool` and the games' `GameParts` are unchanged. Parent plan:
[ultimate-gun-game.md](ultimate-gun-game.md).

How to check: `map_fitness` (apps/party_kit/src/bin/map_fitness.rs,
`--features tools`; docs/OBSERVABILITY.md, "Measuring maps for a gun
game's pool") writes one CSV row per installed map. The sweep of
2026-11-13 is [map-pool-fitness.csv](map-pool-fitness.csv) (map names and
numbers only), readable in the table at the end.

## How a map is measured

Each map loads as the dedicated server loads it (no textures, no kit) on
Combat Arms' simulation with the party's parts (the map logic's brush
entities) and the gun game started, then:

- **The walkable area**: the bots' generated navigation mesh (Combat
  Arms' walker, jump-ups included), flooded from the map's spawn points,
  bounded to the clusters a walking player reaches where the map has
  visibility data; its main region (areas walked to from a spawn and
  back) is `area_m2`. Disconnected scenery is left out.
- **Walking times** at one run speed for every game, CS:S's 250 units/s
  (6.35 m/s), Dijkstra over the areas: between two random points of the
  area (`pair`, area-weighted samples), between spawn points
  (`spawn_pair`), from spawn points to the hub (`spawn_hub`; the hub is
  the area with the least mean walk to everywhere: the place crossings
  pass nearest, our stand-in for "most visited").
- **Meeting times**: `meet_N`, with N players spread at random (as
  `random_hidden` spawns them), the median walk to the nearest other
  player (a lower bound: as if you knew where they were); and `wander_N`,
  players wandering blind meet like a gas's molecules collide:
  `area / ((N - 1) * 2 * sight * 4/pi * speed)`, where `sight` is the
  mean of 16 level rays from eye height at the samples, capped at 50 m.
  `wander_N` is the rule's measure: it is the mean time between a
  player's meetings, and it rewards open maps where you see each other
  from afar.
- **Spawns**: the map's spawn points (count, least and median distance),
  the spawn spots `random_hidden` picks from (`spawns::candidates`), how
  many of those are off the map (below) and how many a standing player
  would be stuck in.

Calibration (wander at 4/8/12 players, s): CA darkhills 0.9/0.4/0.2 and
warehouse 4.6/2.0/1.3 (the small chaotic ones), gg_ arenas 1.9-4.5 at 4,
de_dust2 9.5/4.1/2.6, cs_office 10.5/4.5/2.9, de_nuke 12.9/5.5/3.5,
de_port 11.3/4.9/3.1, grayhammer 15.0/6.4/4.1, cs_militia (leaking, see
below) 31.2/13.4/8.5. The stock CS:S maps are made for 10-32 players and
come out as such.

## The rule

A map is in the pool at N players when:

1. It is a deathmatch-shaped map: not a course or minigame map (CS:S's
   `bhop_`, `kz_`, `surf_`, `mg_`, `test_`), not a mode variant (Combat
   Arms' `_cp` missions, `boss*`, `zombie*`, `infected*`, `*defence`,
   `training`, `testing_t`, `_qrt`/`q` quarantine twins of a map in the
   list). These go by name, as data (below), not by numbers.
2. It loaded with a walkable region and at least one spawn point.
3. Nothing flags it for a human look: area over 15000 m2 (a leak or open
   terrain the numbers can't judge), or 5% or more of its spawn spots off
   the map.
4. And it is small (1500 m2 or less, or `wander_4` 3 s or less: always
   in, chaos welcome), or `wander_N` is at most **10 s**.

Why 10 s: the ladder is 13 rungs and a knife, and a match is 600 s at
most; a party match should end in about five minutes, so the leader needs
a kill about every 20 s. A player wins about half its fights (the leader
somewhat more), so it needs a meeting about every 10 s; with the fight
itself (2-5 s) and the 2 s respawn and 2 s protection that is a kill
every 15-25 s. 12-15 s (the first guess) let in nearly every map at 4
players; 10 s keeps the stock CS:S maps for 8 or more players, which is
what they were made for, except de_dust2 (9.5 s), which stays at 4.

Spawn counts: `random_hidden` spreads spawns over the mesh's spots (up to
1500), so the map's spawn point count doesn't limit the players. It does
where a map has no spots (Combat Arms' nemexishq: 8 spawn points only,
then 8 players at most), and the spots must be where the players can walk
(the confined maps below).

Resulting pools (138 maps measured fit at 8; counts by game CS:S / Combat
Arms / HL2:DM):

- **4 players: 118** (48 / 64 / 6): every installed gg_ arena (42), the
  small and middle-sized Combat Arms maps (warehouse, junk_flea,
  darkhills, bloodmoney and 60 more), cs_assault, cs_italy, de_dust2,
  de_prodigy, de_tides, de_train, and HL2:DM's dm_lockdown, dm_overwatch,
  dm_resistance, dm_runoff, dm_steamlab, dm_underpass.
- **8 players: 138**: the above plus cs_compound, cs_office, de_aztec,
  de_cbble, de_chateau, de_dust, de_inferno, de_nuke, de_piranesi,
  de_port, cs_havana (in by the numbers; its off-map spots below), Combat
  Arms' cityofwings, grayhammer, heavymetal, nemexiswar, nwar, piazza,
  snowrobot, snowvalley, waverider, and dm_powerhouse.
- **12 players: 139**: the above plus Combat Arms' 16rooms (flag it:
  91 stuck spots, a mode map by its look).

The table at the end has each map's pool membership. No custom aim_,
fy_, awp_ or ka_ maps are installed; the gg_ maps (several of them fy_
arenas: gg_fy_funtimes, gg_fy_tactic_fight) are the custom arenas there
are.

## For a human look

- **cs_militia**: 22061 of its 24827 spawn spots are off the map's own
  navigation file; its walk leaks onto the hills around the town (area
  31564 m2, against about 6000 played). Out of every pool until fixed.
- **cs_havana**: 341 of 2811 spots off its navigation file, a patch at
  floor level x -2315..-1306, y 1248..1696 (Source units). It appears
  only with the party's map logic loaded (without it: 3 spots off, area
  3758 m2; with it: 5228 m2), so something the map logic does (a door or
  brush entity opened or removed) lets the walk into a space that isn't
  played. The reported stuck position (setpos -1832.60 1460.23 75.87) is
  1.9 m above one of those spots (-1824 1465 0); at that point a standing
  player's box meets nothing in the server's world (nothing at all within
  4 m but the floor 1.93 m below, on any physics layer), so the player
  was stuck in that off-map patch, not inside a wall or prop the server
  has; whatever held them is either missing from the server's collision
  or the client's alone.
- **de_chateau**: 17 spots off its navigation file (around 12 839 24 and
  720 1012 31); **de_port** 7 (on roofs at z 512-672). Under the 5%
  line, but real.
- **Spawn spots confined** (the spots' walker, a walking player 0.8 m
  wide, can't leave where the bots' can, so `random_hidden` spawns
  everyone in a part of the map): Combat Arms' warehouse (the spots only
  in the two spawn rooms: 199 of 1820 m2; with a 0.6 m walker it reaches
  1373 m2), sector25 (184 of 1824), castle (350 of 3306), battleship,
  battleship1, cargoship, twinship (about 350-500 of 2300), killland,
  lastvegas, twotowers, crystaltower, rapidstream, waverider, downhill,
  grayhammer, and CS:S gg_lego_spacetower2. A doorway narrower than the
  walker, most likely; worth fixing before these join the pool.
- **Spawn points off the region**: de_piranesi 14 of 40, de_aztec,
  de_cbble, de_chateau 5 each, Combat Arms' hellgate 11 of 24 (spots
  still cover the map; only the fallback uses the points).
- **Area over 15000 m2**: Combat Arms' darkthandark (23651, open: sight
  49 m), deserthole and desertholezone (19710).
- **nemexishq**: no spawn spots at all (spawns at its 8 points only).

Stuck spawn spots (a standing CS:S player's box from a step over the
feet to the head meets a wall, prop or ceiling; the spots are checked
with a capsule, which misses a box's corners): 985 of 402942 spots, on
145 of 222 maps; most often the map's solid brushes or surfaces (449 of
the listed examples), then static props (96) and brush entities (34).
Per map at most about 1% (16rooms 91 of 10698; de_port 17 of 6490;
de_piranesi 17 of 3598; dm_runoff 23 of 4446). The table's `stuck`
column counts them; the CSV's `stuck_at` names a few with what they meet.

## Maps that don't load or can't be measured

- Combat Arms **hgoilrig**: loads, but none of its 16 spawn points stands
  on a walkable floor (no walkable region from them; the lobby lists it
  and the pool already leaves it out).
- CS:S **bhop_myztek**: no walkable region from its 40 spawn points.
- CS:S **test_hardware**, **test_speakers**: no spawn points.
- HL2:DM **halls3**: the map's file is missing from the store (its record
  isn't there); not a dm_ map anyway.
- surf_surreal: a model inside it panics the model reader (vmdl, index
  out of range) while loading; the map itself loads and is measured.

Every other map loads (219 of 222); load times headless median 4.7 s
(longest surf_demise 69 s, converting), the two floods median 0.3 s
(longest snowvalley 6.5 s).

## Data shape (proposed, not wired)

Data first: the measurements are run-time data per game, the rule's
numbers are the minigame's, and the pool is computed from both when a
match is made, with no map list in Rust.

- `games/<game>/data/map_fitness.toml`, generated by `map_fitness --write
  <game>` (a flag to add) from the user's install, one table per map:

  ```toml
  # Generated by map_fitness (docs/plans/active/map-pool-fitness.md).
  [maps.de_dust2]
  area_m2 = 4876
  sight_m = 10.5
  wander_s = { 4 = 9.5, 8 = 4.1, 12 = 2.6 }
  spawns = 40
  spots = 3328
  spots_off = 0
  stuck = 9
  ```

  Only maps the shipped data covers are judged; a map the user has and
  the file doesn't (a new custom map) is measured once at run time by
  the host (the floods take a second or two; the result kept in the
  local cache, never the repo), or falls back to the name rules.
- `games/<game>/data/map_pool.toml`, hand-written: the name rules and
  the overrides, each with its reason, the way
  `ladder_exclusions.toml` lists draw exclusions:

  ```toml
  deny = [
    { maps = ["bhop_*", "kz_*", "surf_*", "mg_*", "test_*"], reason = "a course or minigame map" },
    { maps = ["cs_militia"], reason = "spawns off the map until the walk is bounded" },
  ]
  allow = [
    { maps = ["darkhills"], reason = "chaos wanted" },
  ]
  ```

  They replace `GameParts::map_patterns` (which then matches everything a
  game installs) and the claimed `maps` list stays as the fallback order.
- Registered as the exclusions are (`DrawExclusionsAppExt`):
  `add_map_fitness(game, text)` and `add_map_pool_rules(game, text)` into
  one resource; the rule's numbers (`target_s = 10`, `small_m2 = 1500`,
  `chaos_s = 3`, `look_area_m2 = 15000`, `look_off_share = 0.05`) are
  the minigame definition's (`[maps] fit = { target_s = 10 }` in
  `MinigameDef`), so another minigame can be stricter.
- `auto::map_pool` gains the player count (the lobby's, or `MaxPlayer`)
  and filters by it; the map vote's candidates come from the same pool,
  and a server whose player count grows past a map's limit keeps the
  match and picks from the bigger pool next rotation.

## Steps

1. Done: the tool and the 2026-11-13 sweep (this file, the CSV).
2. Fix the spawn findings (another session): cs_militia and cs_havana's
   off-map spots, the confined Combat Arms maps, the stuck spots (test
   the spots with the box the movement uses).
3. `map_fitness --write`, the two data files per game, the registration
   and `map_pool` by player count; a scenario test that a big map isn't
   drawn at 4 players and a small one is at 12.
4. Optional: a lobby option to show the pool, and the run-time
   measurement of maps the data lacks.

## The sweep of 2026-11-13

Times in seconds at 6.35 m/s; `pair` and `spawn-pair` p50/p90; `meet`
and `wander` at 4/8/12 players; `spots`: the spawn spots
`random_hidden` picks from; `off map` and `stuck` among them; 4/8/12:
in the pool at that many players.

| game | map | loads | area m2 | pair p50/p90 s | spawn-pair p50/p90 s | spawn-hub p50 s | meet 4/8/12 s | sight m | wander 4/8/12 s | spawns | spots | off map | stuck | 4 | 8 | 12 | note |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| cs_source | bhop_addict_v2_3xl | yes | 2185 | 4.5/8.0 | 2.5/4.4 | 3.1 | 2.5/1.6/1.3 | 21.9 | 2.1/0.9/0.6 | 13 | 1316 | 0 | 1 | - | - | - | course/minigame map (bhop_*); 1 of 13 spawns off the region |
| cs_source | bhop_backport_css | yes | 72 | 0.6/1.3 | 0.6/1.3 | 0.6 | 0.5/0.0/0.0 | 10.0 | 0.1/0.1/0.0 | 15 | 36 | 0 | 0 | - | - | - | course/minigame map (bhop_*); tiny region (spawn room only?) |
| cs_source | bhop_flatzone | yes | 1334 | 3.8/6.9 | 0.0/0.6 | 2.5 | 1.9/1.3/1.2 | 17.5 | 1.6/0.7/0.4 | 22 | 736 | 0 | 0 | - | - | - | course/minigame map (bhop_*) |
| cs_source | bhop_myztek | no | -0 | 0.0/0.0 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 40 | 0 | 40 | 0 | - | - | - | course/minigame map (bhop_*); no walkable region from the spawn points; 40 of 40 spawns off the region; 40 of 0 spawn spots off the map |
| cs_source | cs_assault | yes | 4484 | 8.9/18.3 | 21.6/32.3 | 14.3 | 4.7/2.8/2.2 | 14.4 | 6.4/2.7/1.7 | 40 | 2608 | 0 | 7 | yes | yes | yes |  |
| cs_source | cs_compound | yes | 7649 | 11.3/18.7 | 18.7/20.1 | 15.0 | 6.5/4.1/3.0 | 14.2 | 11.1/4.8/3.0 | 32 | 7109 | 2 | 13 | - | yes | yes |  |
| cs_source | cs_havana | yes | 5228 | 12.2/19.2 | 16.0/19.7 | 11.3 | 7.0/3.8/2.8 | 9.0 | 12.0/5.1/3.3 | 40 | 2811 | 341 | 7 | - | - | - | 341 of 2811 spawn spots off the map |
| cs_source | cs_italy | yes | 3227 | 12.4/22.1 | 18.4/23.6 | 10.4 | 7.0/3.8/2.6 | 7.2 | 9.3/4.0/2.5 | 40 | 1817 | 1 | 10 | yes | yes | yes |  |
| cs_source | cs_militia | yes | 31564 | 42.5/90.0 | 1.1/94.7 | 3.5 | 17.7/9.3/7.0 | 20.8 | 31.2/13.4/8.5 | 32 | 24827 | 22061 | 32 | - | - | - | area over 15000 m2 (a leak or open terrain); 22061 of 24827 spawn spots off the map |
| cs_source | cs_office | yes | 3966 | 13.3/20.9 | 15.4/20.1 | 10.8 | 7.0/3.8/2.5 | 7.8 | 10.5/4.5/2.9 | 40 | 2514 | 1 | 9 | - | yes | yes |  |
| cs_source | de_aztec | yes | 6116 | 13.7/25.0 | 29.2/32.9 | 16.7 | 7.9/4.1/2.9 | 11.6 | 10.9/4.7/3.0 | 40 | 3389 | 0 | 6 | - | yes | yes | 5 of 40 spawns off the region |
| cs_source | de_cbble | yes | 6501 | 15.1/25.6 | 24.1/34.0 | 13.7 | 7.4/4.5/3.2 | 10.5 | 12.8/5.5/3.5 | 40 | 3940 | 1 | 9 | - | yes | yes | 5 of 40 spawns off the region |
| cs_source | de_chateau | yes | 4218 | 14.4/28.8 | 26.6/28.8 | 18.9 | 7.6/4.4/3.0 | 6.9 | 12.7/5.4/3.5 | 40 | 2498 | 17 | 15 | - | yes | yes | 5 of 40 spawns off the region |
| cs_source | de_dust | yes | 5671 | 13.4/22.8 | 25.5/28.6 | 13.8 | 7.2/4.0/3.0 | 11.2 | 10.5/4.5/2.9 | 41 | 3335 | 0 | 9 | - | yes | yes |  |
| cs_source | de_dust2 | yes | 4876 | 12.6/20.4 | 17.2/19.9 | 11.9 | 7.1/3.8/2.5 | 10.5 | 9.5/4.1/2.6 | 40 | 3328 | 0 | 9 | yes | yes | yes |  |
| cs_source | de_inferno | yes | 3842 | 11.9/20.2 | 22.7/25.4 | 12.3 | 6.0/3.7/2.6 | 7.4 | 10.8/4.6/2.9 | 40 | 2127 | 0 | 5 | - | yes | yes |  |
| cs_source | de_nuke | yes | 5345 | 13.5/24.2 | 28.7/31.8 | 18.0 | 7.6/4.4/3.2 | 8.5 | 12.9/5.5/3.5 | 40 | 3036 | 0 | 8 | - | yes | yes |  |
| cs_source | de_piranesi | yes | 5289 | 12.4/20.2 | 27.1/30.0 | 18.1 | 7.2/3.9/2.9 | 9.4 | 11.6/5.0/3.2 | 40 | 3598 | 1 | 17 | - | yes | yes | 14 of 40 spawns off the region |
| cs_source | de_port | yes | 10300 | 18.3/29.1 | 29.9/33.0 | 20.6 | 9.3/5.4/3.8 | 18.7 | 11.3/4.9/3.1 | 40 | 6490 | 7 | 17 | - | yes | yes |  |
| cs_source | de_prodigy | yes | 2142 | 17.5/32.2 | 15.6/20.5 | 12.7 | 7.0/3.3/2.2 | 5.6 | 7.9/3.4/2.2 | 40 | 1281 | 0 | 7 | yes | yes | yes |  |
| cs_source | de_tides | yes | 2438 | 10.4/16.8 | 11.9/13.7 | 8.4 | 5.4/2.8/2.0 | 7.3 | 6.9/2.9/1.9 | 40 | 1505 | 1 | 9 | yes | yes | yes |  |
| cs_source | de_train | yes | 3128 | 11.2/19.6 | 20.3/24.2 | 12.5 | 6.4/3.6/2.7 | 7.4 | 8.7/3.7/2.4 | 40 | 1712 | 0 | 3 | yes | yes | yes |  |
| cs_source | gg_4mida_arena | yes | 1451 | 5.6/8.3 | 7.9/9.2 | 5.8 | 3.0/1.8/1.3 | 8.8 | 3.4/1.5/0.9 | 42 | 1024 | 0 | 3 | yes | yes | yes |  |
| cs_source | gg_beacon | yes | 1736 | 5.2/9.4 | 6.1/7.8 | 3.6 | 3.1/1.9/1.3 | 8.1 | 4.4/1.9/1.2 | 32 | 853 | 0 | 4 | yes | yes | yes |  |
| cs_source | gg_bk_warehouse_v1 | yes | 1591 | 5.1/8.2 | 8.7/10.4 | 5.0 | 3.0/1.8/1.3 | 7.3 | 4.5/1.9/1.2 | 24 | 878 | 0 | 4 | yes | yes | yes |  |
| cs_source | gg_blue_arena_32a | yes | 984 | 4.8/8.4 | 7.2/8.5 | 4.0 | 2.2/1.2/0.9 | 7.9 | 2.6/1.1/0.7 | 36 | 695 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_cb_arctic | yes | 972 | 4.1/6.9 | 3.5/5.3 | 2.7 | 2.1/1.2/0.9 | 9.1 | 2.2/0.9/0.6 | 32 | 740 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_churches_x_final_fixed | yes | 1348 | 4.9/8.6 | 7.6/8.8 | 6.2 | 2.9/1.7/1.2 | 7.4 | 3.8/1.6/1.0 | 32 | 703 | 0 | 3 | yes | yes | yes |  |
| cs_source | gg_construct_city | yes | 1064 | 3.9/6.8 | 7.4/8.6 | 4.4 | 2.2/1.3/1.0 | 6.8 | 3.2/1.4/0.9 | 48 | 622 | 0 | 1 | yes | yes | yes |  |
| cs_source | gg_deagle7k | yes | 1678 | 5.2/8.6 | 7.5/9.0 | 4.5 | 3.0/1.8/1.3 | 8.0 | 4.3/1.9/1.2 | 44 | 916 | 0 | 1 | yes | yes | yes |  |
| cs_source | gg_desert_paintball | yes | 1440 | 4.0/6.8 | 6.0/7.7 | 4.0 | 2.3/1.5/1.1 | 9.6 | 3.1/1.3/0.8 | 88 | 711 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_dev_moment_v1 | yes | 833 | 3.2/5.5 | 5.1/6.4 | 3.9 | 1.8/1.2/0.8 | 7.3 | 2.4/1.0/0.6 | 40 | 446 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_dinoiceworld | yes | 1236 | 4.1/6.7 | 5.7/7.6 | 3.5 | 2.4/1.4/1.1 | 7.8 | 3.3/1.4/0.9 | 32 | 761 | 0 | 3 | yes | yes | yes |  |
| cs_source | gg_fusion_trx | yes | 1041 | 3.8/6.7 | 7.5/8.9 | 4.8 | 2.2/1.3/1.0 | 9.2 | 2.3/1.0/0.6 | 28 | 562 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_future | yes | 2158 | 6.0/9.9 | 7.3/9.2 | 4.5 | 3.3/1.9/1.3 | 11.4 | 3.9/1.7/1.1 | 56 | 1461 | 0 | 4 | yes | yes | yes |  |
| cs_source | gg_fy_funtimes | yes | 1596 | 5.3/8.8 | 7.8/9.8 | 4.3 | 3.0/1.7/1.3 | 9.5 | 3.5/1.5/0.9 | 32 | 815 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_fy_tactic_fight | yes | 1782 | 4.6/7.3 | 5.6/6.9 | 3.2 | 2.8/1.8/1.4 | 10.8 | 3.4/1.5/0.9 | 32 | 1089 | 0 | 1 | yes | yes | yes |  |
| cs_source | gg_hex | yes | 814 | 3.4/5.6 | 7.0/7.8 | 3.9 | 2.0/1.2/0.9 | 6.6 | 2.6/1.1/0.7 | 24 | 629 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_iceworld_l33t | yes | 1326 | 4.6/7.5 | 7.0/8.5 | 4.1 | 2.6/1.5/1.2 | 9.4 | 2.9/1.2/0.8 | 24 | 970 | 0 | 2 | yes | yes | yes |  |
| cs_source | gg_ilu | yes | 1092 | 4.1/7.8 | 8.5/10.1 | 4.9 | 2.2/1.4/1.1 | 6.9 | 3.3/1.4/0.9 | 40 | 720 | 0 | 5 | yes | yes | yes |  |
| cs_source | gg_legendary_fun_v1a | yes | 1069 | 4.0/6.6 | 6.8/7.6 | 3.6 | 2.4/1.3/1.0 | 7.8 | 2.8/1.2/0.8 | 32 | 558 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_lego_2floor_v2 | yes | 2330 | 5.2/8.2 | 6.9/9.1 | 5.6 | 3.3/2.0/1.5 | 11.2 | 4.3/1.8/1.2 | 32 | 1118 | 0 | 1 | yes | yes | yes |  |
| cs_source | gg_lego_mafia_arena | yes | 1502 | 6.4/10.1 | 5.0/6.4 | 3.2 | 3.6/2.1/1.5 | 8.5 | 3.7/1.6/1.0 | 32 | 900 | 0 | 13 | yes | yes | yes |  |
| cs_source | gg_lego_spacetower2 | yes | 1408 | 4.5/6.9 | 6.3/8.2 | 4.3 | 2.7/1.7/1.2 | 17.5 | 1.7/0.7/0.5 | 90 | 466 | 0 | 0 | yes | yes | yes | spawn spots confined (652 of 1408 m2) |
| cs_source | gg_mario_vs_wario | yes | 1890 | 4.6/7.8 | 6.7/8.5 | 4.6 | 2.7/1.7/1.3 | 8.8 | 4.4/1.9/1.2 | 40 | 1023 | 0 | 3 | yes | yes | yes |  |
| cs_source | gg_mario_world_2 | yes | 2661 | 5.1/8.0 | 6.9/8.6 | 4.1 | 3.3/2.2/1.7 | 13.7 | 4.0/1.7/1.1 | 56 | 1356 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_minesweeper | yes | 1200 | 4.1/6.6 | 5.4/7.2 | 3.8 | 2.3/1.3/1.1 | 7.0 | 3.5/1.5/1.0 | 32 | 577 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_mr_pillar_v1 | yes | 238 | 1.1/1.9 | 0.6/1.0 | 0.5 | 0.8/0.6/0.4 | 6.0 | 0.8/0.3/0.2 | 40 | 53 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_nukkon_hdr | yes | 1067 | 4.7/8.0 | 8.3/9.6 | 5.1 | 2.5/1.4/1.1 | 7.6 | 2.9/1.2/0.8 | 36 | 727 | 0 | 3 | yes | yes | yes |  |
| cs_source | gg_nutty | yes | 1240 | 4.6/7.9 | 7.5/8.3 | 4.1 | 2.6/1.6/1.2 | 8.0 | 3.2/1.4/0.9 | 32 | 849 | 0 | 1 | yes | yes | yes |  |
| cs_source | gg_pokemonz | yes | 1165 | 4.3/7.6 | 6.5/10.2 | 4.9 | 2.5/1.6/1.2 | 7.1 | 3.4/1.4/0.9 | 102 | 603 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_sex_fix | yes | 894 | 4.5/7.0 | 5.8/7.3 | 3.5 | 2.4/1.3/1.0 | 6.5 | 2.8/1.2/0.8 | 60 | 514 | 0 | 1 | yes | yes | yes |  |
| cs_source | gg_simpsons_arabtoon | yes | 460 | 3.0/6.7 | 6.9/8.0 | 3.9 | 1.5/0.7/0.6 | 5.1 | 1.9/0.8/0.5 | 50 | 219 | 0 | 3 | yes | yes | yes |  |
| cs_source | gg_simpsons_bam | yes | 1024 | 3.3/5.7 | 5.7/7.7 | 4.1 | 2.3/1.5/1.2 | 9.8 | 2.1/0.9/0.6 | 48 | 508 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_simpsons_d | yes | 817 | 4.0/6.8 | 5.8/6.9 | 3.4 | 2.3/1.3/0.9 | 5.7 | 2.9/1.3/0.8 | 40 | 373 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_simpsons_dusty_2 | yes | 825 | 4.2/7.3 | 7.6/8.6 | 4.5 | 2.1/1.1/0.8 | 8.6 | 2.0/0.8/0.5 | 24 | 379 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_simpsons_funfreaks_v2 | yes | 802 | 3.1/5.1 | 4.7/6.5 | 3.3 | 1.8/1.1/0.9 | 8.6 | 1.9/0.8/0.5 | 68 | 500 | 0 | 1 | yes | yes | yes |  |
| cs_source | gg_spacoid_pg | yes | 1314 | 4.6/7.5 | 3.7/5.3 | 2.5 | 2.6/1.6/1.3 | 7.9 | 3.4/1.5/0.9 | 32 | 684 | 0 | 1 | yes | yes | yes |  |
| cs_source | gg_tbr_water_basin | yes | 721 | 3.9/6.4 | 4.8/6.7 | 3.4 | 2.2/1.2/0.9 | 5.7 | 2.6/1.1/0.7 | 31 | 556 | 0 | 1 | yes | yes | yes |  |
| cs_source | gg_tip_octal | yes | 1354 | 4.4/7.3 | 6.0/8.5 | 4.5 | 2.7/1.5/1.1 | 9.0 | 3.1/1.3/0.8 | 40 | 946 | 0 | 1 | yes | yes | yes |  |
| cs_source | gg_toondorf | yes | 920 | 4.2/7.0 | 6.8/8.7 | 4.6 | 2.3/1.3/0.9 | 6.7 | 2.8/1.2/0.8 | 42 | 521 | 0 | 2 | yes | yes | yes |  |
| cs_source | gg_towerwars_v2 | yes | 674 | 3.4/5.6 | 5.3/6.5 | 3.1 | 1.9/1.1/0.7 | 6.2 | 2.2/1.0/0.6 | 32 | 378 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_usp_deagle | yes | 1268 | 4.5/7.9 | 8.0/9.3 | 4.9 | 2.6/1.4/1.1 | 7.9 | 3.3/1.4/0.9 | 23 | 641 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_wolfenstein_3d | yes | 1058 | 3.7/6.5 | 5.1/6.5 | 3.1 | 2.2/1.3/0.8 | 8.9 | 2.4/1.0/0.7 | 24 | 568 | 0 | 0 | yes | yes | yes |  |
| cs_source | kz_11342 | yes | 38 | 0.6/1.0 | 0.6/0.6 | 0.6 | 0.0/0.0/0.0 | 3.3 | 0.2/0.1/0.1 | 50 | 18 | 0 | 0 | - | - | - | course/minigame map (kz_*); tiny region (spawn room only?) |
| cs_source | kz_ancient_ruins | yes | 2054 | 4.9/8.6 | 0.7/1.3 | 1.9 | 2.8/1.8/1.3 | 14.1 | 3.0/1.3/0.8 | 5 | 1040 | 0 | 3 | - | - | - | course/minigame map (kz_*) |
| cs_source | kz_bhop_izanami | yes | 7868 | 16.1/36.9 | 0.0/0.0 | 34.6 | 7.9/3.8/2.9 | 21.4 | 7.6/3.2/2.1 | 2 | 3059 | 0 | 0 | - | - | - | course/minigame map (kz_*) |
| cs_source | kz_bhop_sakura | yes | 158 | 1.3/3.1 | 0.0/0.6 | 0.6 | 0.6/0.4/0.0 | 6.2 | 0.5/0.2/0.1 | 6 | 90 | 0 | 0 | - | - | - | course/minigame map (kz_*); tiny region (spawn room only?) |
| cs_source | kz_bhop_skodna | yes | 1299 | 5.5/13.3 | 0.0/0.0 | 3.0 | 2.5/1.3/1.2 | 29.1 | 0.9/0.4/0.3 | 7 | 742 | 0 | 0 | - | - | - | course/minigame map (kz_*); 1 of 7 spawns off the region |
| cs_source | kz_hikari_od_nh_v2 | yes | 5372 | 7.8/14.8 | 0.6/0.9 | 10.6 | 4.5/2.8/2.0 | 24.9 | 4.5/1.9/1.2 | 10 | 38 | 0 | 0 | - | - | - | course/minigame map (kz_*); spawn spots confined (54 of 5372 m2) |
| cs_source | kz_rockb1ock | yes | 283 | 1.6/3.1 | 1.3/2.0 | 1.3 | 0.9/0.6/0.4 | 7.5 | 0.8/0.3/0.2 | 32 | 149 | 0 | 0 | - | - | - | course/minigame map (kz_*) |
| cs_source | mg_3k_smash_lego_copter | yes | 1993 | 4.8/11.0 | 6.3/8.1 | 5.9 | 3.4/1.9/1.3 | 43.4 | 0.9/0.4/0.3 | 40 | 1029 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_boatrace_scramble | yes | 900 | 3.1/6.4 | 0.6/1.2 | 0.9 | 1.8/1.1/0.8 | 23.7 | 0.8/0.3/0.2 | 20 | 610 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_crazykart_v1_1 | yes | 1940 | 4.4/7.6 | 1.2/2.0 | 3.4 | 2.5/1.4/1.3 | 20.3 | 2.0/0.8/0.5 | 32 | 1065 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_creative_multigames_v8_ns | yes | 1486 | 3.7/6.1 | 4.7/6.0 | 2.9 | 2.1/1.3/1.1 | 16.2 | 1.9/0.8/0.5 | 64 | 831 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_escape_prison_beta | yes | 798 | 2.5/5.0 | 1.9/3.8 | 1.3 | 1.9/1.3/0.6 | 13.9 | 1.2/0.5/0.3 | 64 | 441 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_item_battle_v4b | yes | 12998 | 11.6/19.5 | 17.9/23.5 | 11.6 | 6.9/4.4/3.5 | 23.2 | 11.5/4.9/3.1 | 32 | 7963 | 0 | 3 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_jacks_multigames_v1 | yes | 612 | 1.9/3.1 | 1.3/2.5 | 1.3 | 1.3/0.6/0.6 | 8.7 | 1.4/0.6/0.4 | 55 | 342 | 0 | 0 | - | - | - | course/minigame map (mg_*); 55 of 55 spawns off the region |
| cs_source | mg_kommando | yes | 2832 | 3.9/6.6 | 2.1/3.9 | 1.9 | 3.1/2.0/1.6 | 33.8 | 1.7/0.7/0.5 | 32 | 1871 | 0 | 2 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_lego_course | yes | 108 | 0.8/1.3 | 0.6/1.3 | 0.6 | 0.5/0.0/0.0 | 5.5 | 0.4/0.2/0.1 | 72 | 48 | 0 | 0 | - | - | - | course/minigame map (mg_*); tiny region (spawn room only?) |
| cs_source | mg_lego_multigames_v2 | yes | 2542 | 5.0/8.7 | 6.3/8.7 | 4.6 | 3.1/1.9/1.3 | 22.4 | 2.3/1.0/0.6 | 65 | 1396 | 0 | 0 | - | - | - | course/minigame map (mg_*); 1 of 65 spawns off the region |
| cs_source | mg_lt_galaxy_v5 | yes | 1542 | 3.9/6.4 | 3.0/4.3 | 2.1 | 2.2/1.4/1.1 | 19.5 | 1.6/0.7/0.4 | 64 | 1291 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_n64_goldeneye_v2 | yes | 244 | 1.1/1.7 | 1.1/1.7 | 0.6 | 0.7/0.6/0.5 | 5.8 | 0.9/0.4/0.2 | 40 | 123 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_randomizer_v5 | yes | 122 | 1.1/1.9 | 0.6/1.3 | 0.6 | 0.6/0.0/0.0 | 5.7 | 0.4/0.2/0.1 | 50 | 70 | 0 | 0 | - | - | - | course/minigame map (mg_*); 50 of 50 spawns off the region; tiny region (spawn room only?) |
| cs_source | mg_starwars_v1 | yes | 11993 | 16.4/36.5 | 37.1/39.5 | 20.8 | 7.5/4.4/3.4 | 20.8 | 11.9/5.1/3.2 | 32 | 6624 | 0 | 3 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_swag_multigames_v1 | yes | 186 | 1.3/2.5 | 1.3/2.5 | 1.3 | 0.6/0.6/0.0 | 7.1 | 0.5/0.2/0.1 | 50 | 99 | 0 | 0 | - | - | - | course/minigame map (mg_*); 50 of 50 spawns off the region; tiny region (spawn room only?) |
| cs_source | mg_wipeout | yes | 9242 | 13.9/41.5 | 0.6/1.3 | 15.9 | 8.2/4.7/3.4 | 32.1 | 5.9/2.5/1.6 | 36 | 6298 | 0 | 1 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_wipeout2 | yes | 54 | 0.6/1.4 | 0.6/1.4 | 0.6 | 0.0/0.0/0.0 | 3.9 | 0.3/0.1/0.1 | 32 | 30 | 0 | 0 | - | - | - | course/minigame map (mg_*); tiny region (spawn room only?) |
| cs_source | surf_apollo | yes | 354 | 1.8/3.1 | 1.2/1.9 | 0.6 | 1.0/0.6/0.6 | 14.6 | 0.5/0.2/0.1 | 64 | 185 | 0 | 0 | - | - | - | course/minigame map (surf_*); 32 of 64 spawns off the region |
| cs_source | surf_boreas | yes | 488 | 2.4/4.3 | 1.9/3.8 | 1.9 | 1.3/0.6/0.6 | 10.5 | 1.0/0.4/0.3 | 162 | 253 | 0 | 0 | - | - | - | course/minigame map (surf_*); 162 of 162 spawns off the region |
| cs_source | surf_botanica | yes | 3373 | 7.6/12.2 | 0.6/0.6 | 3.5 | 3.8/2.3/1.8 | 31.2 | 2.2/1.0/0.6 | 64 | 1863 | 0 | 1 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_demise | yes | 448 | 2.0/3.3 | 0.6/0.6 | 0.6 | 1.2/0.7/0.6 | 39.6 | 0.2/0.1/0.1 | 64 | 368 | 0 | 0 | - | - | - | course/minigame map (surf_*); 64 of 64 spawns off the region |
| cs_source | surf_halloween_tf2 | yes | 1737 | 5.0/12.2 | 0.6/1.3 | 1.5 | 3.5/2.1/1.4 | 8.7 | 4.1/1.8/1.1 | 26 | 717 | 0 | 1 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_happyhands | yes | 637 | 2.6/4.3 | 1.2/2.2 | 1.3 | 1.4/0.9/0.6 | 12.8 | 1.0/0.4/0.3 | 60 | 408 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_hellenic | yes | 7154 | 15.6/27.7 | 0.6/1.3 | 4.4 | 7.1/3.9/2.9 | 47.2 | 3.1/1.3/0.9 | 64 | 5958 | 0 | 6 | - | - | - | course/minigame map (surf_*); 64 of 64 spawns off the region |
| cs_source | surf_holiday | yes | 449 | 2.4/4.2 | 1.2/1.8 | 1.8 | 1.3/0.6/0.6 | 8.9 | 1.0/0.4/0.3 | 77 | 234 | 0 | 1 | - | - | - | course/minigame map (surf_*); 65 of 77 spawns off the region |
| cs_source | surf_inferno | yes | 658 | 1.9/4.3 | 1.3/1.9 | 0.6 | 1.3/0.6/0.6 | 19.4 | 0.7/0.3/0.2 | 78 | 372 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_jive | yes | 245 | 1.5/3.1 | 0.0/0.0 | 0.4 | 0.8/0.5/0.4 | 13.5 | 0.4/0.2/0.1 | 64 | 154 | 0 | 0 | - | - | - | course/minigame map (surf_*); 64 of 64 spawns off the region |
| cs_source | surf_kismet | yes | 68 | 0.6/1.3 | 0.6/1.3 | 0.6 | 0.0/0.0/0.0 | 7.0 | 0.2/0.1/0.1 | 100 | 42 | 0 | 0 | - | - | - | course/minigame map (surf_*); tiny region (spawn room only?) |
| cs_source | surf_nebula | yes | 957 | 2.7/6.2 | 1.0/1.8 | 1.2 | 1.9/1.1/0.8 | 27.6 | 0.7/0.3/0.2 | 78 | 672 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_nsz_fix | yes | 13522 | 11.0/18.2 | 0.4/0.6 | 7.0 | 6.4/4.0/3.2 | 29.2 | 9.6/4.1/2.6 | 61 | 9507 | 0 | 0 | - | - | - | course/minigame map (surf_*); 12 of 61 spawns off the region |
| cs_source | surf_sacrifice | yes | 5512 | 7.0/11.7 | 5.5/9.4 | 4.3 | 4.1/2.7/2.1 | 29.6 | 3.8/1.6/1.0 | 60 | 5122 | 0 | 1 | - | - | - | course/minigame map (surf_*); 60 of 60 spawns off the region |
| cs_source | surf_sedona | yes | 12103 | 12.3/21.6 | 2.8/5.0 | 3.0 | 7.4/4.4/3.5 | 22.4 | 11.1/4.8/3.0 | 64 | 8292 | 0 | 8 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_slob | yes | 317 | 1.9/3.1 | 0.6/1.3 | 0.6 | 1.0/0.6/0.5 | 12.2 | 0.5/0.2/0.1 | 64 | 192 | 0 | 0 | - | - | - | course/minigame map (surf_*); 64 of 64 spawns off the region |
| cs_source | surf_stickybutt_alpha | yes | 886 | 4.0/7.7 | 2.6/5.2 | 2.5 | 2.2/1.5/1.1 | 28.7 | 0.6/0.3/0.2 | 34 | 1270 | 0 | 3 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_surreal | yes | 597 | 3.2/7.3 | 0.7/1.2 | 5.2 | 1.5/0.9/0.6 | 7.7 | 1.6/0.7/0.4 | 62 | 388 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_threnody | yes | 3624 | 12.5/27.7 | 0.0/0.0 | 18.3 | 5.0/2.5/1.9 | 28.7 | 2.6/1.1/0.7 | 64 | 2075 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | test_hardware | no | 0 | 0.0/0.0 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0 | 0 | 0 | 0 | - | - | - | course/minigame map (test_*); no spawn points |
| cs_source | test_speakers | no | 0 | 0.0/0.0 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0 | 0 | 0 | 0 | - | - | - | course/minigame map (test_*); no spawn points |
| combat_arms | 16rooms | yes | 11441 | 16.1/26.7 | 16.2/26.2 | 11.3 | 10.1/6.0/4.5 | 7.3 | 32.1/13.8/8.8 | 48 | 10698 | 0 | 91 | - | - | yes |  |
| combat_arms | battleship | yes | 2421 | 6.8/14.1 | 2.6/2.9 | 8.8 | 3.7/2.3/1.7 | 10.6 | 4.7/2.0/1.3 | 9 | 187 | 0 | 1 | yes | yes | yes | spawn spots confined (351 of 2421 m2) |
| combat_arms | battleship1 | yes | 2290 | 6.3/12.9 | 2.6/2.9 | 7.5 | 3.4/2.2/1.6 | 10.2 | 4.6/2.0/1.3 | 9 | 187 | 0 | 1 | yes | yes | yes | spawn spots confined (351 of 2290 m2) |
| combat_arms | battleship2 | yes | 2620 | 7.4/15.7 | 15.3/17.0 | 8.8 | 3.9/2.4/1.8 | 10.3 | 5.2/2.2/1.4 | 24 | 469 | 0 | 3 | yes | yes | yes | spawn spots confined (779 of 2620 m2) |
| combat_arms | blacklung | yes | 1175 | 6.0/10.8 | 0.6/0.6 | 3.9 | 3.1/1.6/1.2 | 6.6 | 3.6/1.6/1.0 | 8 | 117 | 0 | 0 | yes | yes | yes |  |
| combat_arms | bloodmoney | yes | 5530 | 11.2/17.7 | 14.5/15.1 | 7.3 | 6.2/3.4/2.5 | 11.8 | 9.7/4.1/2.6 | 16 | 3204 | 0 | 5 | yes | yes | yes |  |
| combat_arms | bloodoffice | yes | 470 | 3.7/6.5 | 5.2/6.8 | 4.8 | 1.8/0.9/0.6 | 5.7 | 1.7/0.7/0.5 | 24 | 207 | 0 | 0 | yes | yes | yes |  |
| combat_arms | bloodycanyon | yes | 1256 | 4.0/6.7 | 4.8/6.5 | 2.9 | 2.3/1.4/1.1 | 9.1 | 2.9/1.2/0.8 | 24 | 784 | 0 | 5 | yes | yes | yes |  |
| combat_arms | bossbattle3_cp | yes | 2021 | 4.4/8.2 | 0.6/1.3 | 4.4 | 2.5/1.8/1.3 | 21.5 | 1.9/0.8/0.5 | 8 | 1130 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | bossbattle4_cp | yes | 2021 | 4.4/8.2 | 0.6/1.3 | 4.4 | 2.5/1.8/1.3 | 21.5 | 1.9/0.8/0.5 | 8 | 1130 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | bossbattle_cp | yes | 28 | 0.0/0.3 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 2.3 | 0.2/0.1/0.1 | 8 | 10 | 0 | 0 | - | - | - | mode variant / not a standard map; tiny region (spawn room only?) |
| combat_arms | bossrush | yes | 1868 | 4.4/7.6 | 0.6/0.6 | 6.7 | 2.5/1.6/1.3 | 20.6 | 1.9/0.8/0.5 | 8 | 1066 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | bossship | yes | 2706 | 6.5/13.2 | 2.6/2.9 | 8.8 | 3.6/2.3/1.7 | 17.2 | 3.2/1.4/0.9 | 9 | 145 | 0 | 0 | - | - | - | mode variant / not a standard map; spawn spots confined (277 of 2706 m2) |
| combat_arms | brushwood | yes | 2690 | 6.9/12.8 | 16.6/18.2 | 9.7 | 3.9/2.5/1.8 | 10.1 | 5.5/2.4/1.5 | 24 | 2059 | 0 | 13 | yes | yes | yes |  |
| combat_arms | cabinfever | yes | 4830 | 7.4/12.7 | 0.4/0.9 | 1.5 | 4.3/2.8/2.1 | 20.5 | 4.9/2.1/1.3 | 8 | 135 | 0 | 2 | yes | yes | yes |  |
| combat_arms | cargoship | yes | 2277 | 5.3/11.1 | 1.1/2.3 | 11.6 | 3.6/2.2/1.7 | 9.0 | 5.2/2.2/1.4 | 24 | 48 | 0 | 0 | yes | yes | yes | spawn spots confined (501 of 2277 m2) |
| combat_arms | castle | yes | 3306 | 11.6/19.4 | 24.4/25.1 | 18.7 | 7.3/4.5/3.3 | 11.5 | 5.9/2.5/1.6 | 24 | 303 | 0 | 0 | yes | yes | yes | spawn spots confined (350 of 3306 m2) |
| combat_arms | cityofwings | yes | 5528 | 16.8/31.0 | 23.0/29.6 | 8.8 | 7.5/3.9/2.9 | 9.6 | 11.9/5.1/3.2 | 23 | 1935 | 0 | 9 | - | yes | yes |  |
| combat_arms | coldseed | yes | 1996 | 7.0/12.2 | 12.9/14.0 | 6.9 | 3.9/2.5/1.8 | 7.3 | 5.6/2.4/1.5 | 24 | 1278 | 0 | 6 | yes | yes | yes |  |
| combat_arms | costarecon | yes | 6260 | 8.4/14.1 | 15.8/17.1 | 8.2 | 5.0/3.2/2.5 | 14.9 | 8.7/3.7/2.4 | 23 | 4218 | 0 | 9 | yes | yes | yes |  |
| combat_arms | costarecon_cp | yes | 1413 | 3.7/6.5 | 0.5/0.6 | 2.5 | 2.1/1.4/1.1 | 14.0 | 2.1/0.9/0.6 | 12 | 973 | 0 | 1 | - | - | - | mode variant / not a standard map |
| combat_arms | costarecon_q | yes | 6260 | 8.4/14.1 | 15.8/17.1 | 8.2 | 5.0/3.2/2.5 | 14.9 | 8.7/3.7/2.4 | 23 | 4218 | 0 | 9 | - | - | - | quarantine variant of costarecon |
| combat_arms | crystaltower | yes | 1714 | 5.4/11.5 | 0.6/1.0 | 4.4 | 3.8/2.3/1.6 | 8.1 | 4.4/1.9/1.2 | 24 | 561 | 0 | 2 | yes | yes | yes | spawn spots confined (851 of 1714 m2) |
| combat_arms | dam | yes | 1573 | 8.7/19.0 | 17.5/18.8 | 9.3 | 4.0/2.1/1.4 | 11.1 | 2.9/1.3/0.8 | 24 | 762 | 0 | 5 | yes | yes | yes |  |
| combat_arms | darkforest | yes | 8252 | 8.9/15.5 | 18.4/22.2 | 11.0 | 5.3/3.4/2.6 | 28.4 | 6.0/2.6/1.6 | 23 | 6461 | 0 | 14 | yes | yes | yes |  |
| combat_arms | darkhills | yes | 348 | 1.7/3.0 | 0.7/1.7 | 0.6 | 1.1/0.6/0.5 | 7.9 | 0.9/0.4/0.2 | 16 | 175 | 0 | 0 | yes | yes | yes |  |
| combat_arms | darkthandark | yes | 23651 | 15.3/27.6 | 3.8/5.2 | 2.6 | 8.5/5.6/4.4 | 49.3 | 9.9/4.2/2.7 | 24 | 15527 | 0 | 6 | - | - | - | area over 15000 m2 (a leak or open terrain) |
| combat_arms | deathroom | yes | 1199 | 7.2/15.6 | 11.1/15.5 | 7.5 | 2.6/1.4/1.0 | 7.2 | 3.4/1.5/0.9 | 24 | 431 | 0 | 1 | yes | yes | yes |  |
| combat_arms | deathroom_cp | yes | 1204 | 7.1/15.5 | 0.0/0.0 | 6.3 | 2.8/1.5/1.1 | 7.2 | 3.5/1.5/0.9 | 8 | 199 | 0 | 1 | - | - | - | mode variant / not a standard map; spawn spots confined (396 of 1204 m2) |
| combat_arms | desertfox_cp | yes | 5528 | 12.9/23.2 | 0.6/0.7 | 19.3 | 7.5/4.4/3.1 | 8.0 | 14.2/6.1/3.9 | 8 | 2948 | 0 | 13 | - | - | - | mode variant / not a standard map |
| combat_arms | deserthole | yes | 19710 | 13.1/22.7 | 35.2/38.6 | 18.9 | 7.6/4.9/3.8 | 23.5 | 17.3/7.4/4.7 | 24 | 15673 | 0 | 22 | - | - | - | area over 15000 m2 (a leak or open terrain) |
| combat_arms | desertholezone | yes | 19713 | 13.1/22.6 | 26.3/27.8 | 15.5 | 7.6/4.9/3.8 | 23.6 | 17.2/7.4/4.7 | 24 | 15687 | 0 | 12 | - | - | - | area over 15000 m2 (a leak or open terrain) |
| combat_arms | downhill | yes | 5342 | 15.7/22.8 | 15.9/17.8 | 18.4 | 6.2/3.5/2.6 | 18.0 | 6.1/2.6/1.7 | 24 | 1846 | 0 | 2 | yes | yes | yes | spawn spots confined (2504 of 5342 m2) |
| combat_arms | dredge | yes | 1440 | 7.0/16.6 | 1.9/3.8 | 7.5 | 3.7/1.9/1.3 | 7.3 | 4.1/1.8/1.1 | 24 | 458 | 0 | 6 | yes | yes | yes |  |
| combat_arms | freezer | yes | 1978 | 7.4/12.1 | 12.2/14.3 | 5.3 | 3.9/2.4/1.8 | 7.9 | 5.2/2.2/1.4 | 24 | 1671 | 0 | 11 | yes | yes | yes |  |
| combat_arms | ghosttown | yes | 5343 | 9.6/18.1 | 17.6/20.3 | 11.5 | 6.0/3.6/2.6 | 11.1 | 9.9/4.2/2.7 | 24 | 2819 | 0 | 11 | yes | yes | yes |  |
| combat_arms | grandcanyon | yes | 3421 | 11.2/20.0 | 21.1/24.8 | 12.5 | 6.5/3.7/2.5 | 7.7 | 9.1/3.9/2.5 | 24 | 1375 | 0 | 4 | yes | yes | yes |  |
| combat_arms | gravedigger | yes | 1310 | 8.2/16.5 | 13.5/15.2 | 9.7 | 4.0/1.9/1.3 | 5.3 | 5.1/2.2/1.4 | 23 | 750 | 0 | 1 | yes | yes | yes |  |
| combat_arms | graveyard | yes | 4103 | 7.9/13.5 | 12.7/15.8 | 7.4 | 4.2/2.5/1.9 | 12.6 | 6.7/2.9/1.8 | 16 | 2733 | 0 | 12 | yes | yes | yes |  |
| combat_arms | gravitya | yes | 1915 | 8.7/17.0 | 16.3/18.4 | 9.6 | 5.1/3.0/2.1 | 4.5 | 8.8/3.8/2.4 | 24 | 619 | 0 | 2 | yes | yes | yes |  |
| combat_arms | gravityb | yes | 946 | 2.5/4.6 | 1.3/3.3 | 1.8 | 1.9/1.2/0.9 | 7.7 | 2.5/1.1/0.7 | 24 | 509 | 0 | 0 | yes | yes | yes |  |
| combat_arms | grayhammer | yes | 5477 | 16.2/29.7 | 27.3/30.3 | 14.5 | 8.6/4.4/3.2 | 7.5 | 15.0/6.4/4.1 | 24 | 1756 | 0 | 11 | - | yes | yes | spawn spots confined (2666 of 5477 m2) |
| combat_arms | grayhammerq | yes | 5476 | 16.2/29.7 | 27.3/30.3 | 14.5 | 8.6/4.3/3.1 | 7.6 | 14.9/6.4/4.1 | 24 | 1767 | 0 | 13 | - | - | - | quarantine variant of grayhammer; spawn spots confined (2666 of 5476 m2) |
| combat_arms | hallowravine | yes | 1084 | 2.2/3.9 | 1.1/1.9 | 1.3 | 1.8/1.2/0.8 | 14.1 | 1.6/0.7/0.4 | 26 | 771 | 0 | 1 | yes | yes | yes |  |
| combat_arms | heavymetal | yes | 6029 | 10.7/21.3 | 0.4/1.1 | 16.6 | 6.3/4.1/3.1 | 9.2 | 13.5/5.8/3.7 | 24 | 3044 | 0 | 16 | - | yes | yes |  |
| combat_arms | hellgate | yes | 478 | 3.1/5.8 | 4.0/5.5 | 2.7 | 1.6/0.9/0.7 | 11.3 | 0.9/0.4/0.2 | 24 | 289 | 0 | 3 | yes | yes | yes | 11 of 24 spawns off the region |
| combat_arms | hgoilrig | no | -0 | 0.0/0.0 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 16 | 0 | 16 | 0 | - | - | - | no walkable region from the spawn points; 16 of 16 spawns off the region; 16 of 0 spawn spots off the map |
| combat_arms | hiredguns2 | yes | 1240 | 8.4/17.0 | 15.3/18.1 | 10.4 | 4.6/2.4/1.6 | 5.2 | 5.0/2.1/1.4 | 24 | 633 | 0 | 4 | yes | yes | yes |  |
| combat_arms | infectedship_cp | yes | 2669 | 6.8/10.3 | 4.0/5.5 | 5.1 | 4.1/2.5/1.8 | 17.4 | 3.2/1.4/0.9 | 8 | 442 | 0 | 0 | - | - | - | mode variant / not a standard map; spawn spots confined (757 of 2669 m2) |
| combat_arms | istanbul | yes | 2681 | 9.0/15.6 | 20.9/22.2 | 11.3 | 4.6/2.6/1.9 | 7.3 | 7.6/3.2/2.1 | 24 | 1218 | 0 | 3 | yes | yes | yes |  |
| combat_arms | istanbulq | yes | 2681 | 9.0/15.6 | 20.9/22.2 | 11.3 | 4.6/2.6/1.9 | 7.3 | 7.6/3.2/2.1 | 24 | 1218 | 0 | 3 | - | - | - | quarantine variant of istanbul |
| combat_arms | jscssnowstorm | yes | 3504 | 6.7/11.2 | 3.7/5.1 | 3.6 | 3.7/2.2/1.7 | 13.6 | 5.3/2.3/1.4 | 20 | 2198 | 0 | 2 | yes | yes | yes |  |
| combat_arms | junk_flea | yes | 1343 | 5.5/10.6 | 8.8/10.2 | 5.2 | 2.9/1.9/1.4 | 6.7 | 4.1/1.8/1.1 | 24 | 695 | 0 | 4 | yes | yes | yes |  |
| combat_arms | junk_flea2 | yes | 1698 | 5.7/10.3 | 10.1/12.0 | 5.8 | 3.4/2.0/1.6 | 6.2 | 5.7/2.4/1.5 | 24 | 990 | 0 | 6 | yes | yes | yes |  |
| combat_arms | killland | yes | 1733 | 3.1/5.7 | 0.6/1.3 | 3.8 | 2.5/1.9/1.3 | 12.7 | 2.8/1.2/0.8 | 23 | 450 | 0 | 1 | yes | yes | yes | spawn spots confined (593 of 1733 m2) |
| combat_arms | lastvegas | yes | 1665 | 4.1/7.2 | 0.8/9.2 | 5.5 | 3.3/2.1/1.5 | 7.5 | 4.6/2.0/1.3 | 24 | 537 | 0 | 4 | yes | yes | yes | spawn spots confined (740 of 1665 m2) |
| combat_arms | mousetrap | yes | 988 | 5.6/11.6 | 12.2/14.3 | 7.0 | 2.6/1.6/1.2 | 5.9 | 3.5/1.5/0.9 | 24 | 709 | 0 | 3 | yes | yes | yes |  |
| combat_arms | nemexishq | yes | 2152 | 5.8/11.9 | 0.0/0.5 | 9.5 | 2.9/1.7/1.3 | 14.4 | 3.1/1.3/0.8 | 8 | 0 | 0 | 0 | yes | yes | yes | no spawn spots: the spawn points only |
| combat_arms | nemexislab_cp | yes | 782 | 4.2/7.8 | 0.5/0.6 | 5.0 | 2.1/1.1/0.8 | 6.8 | 2.4/1.0/0.6 | 8 | 444 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | nemexiswar | yes | 10094 | 16.7/28.8 | 25.5/28.4 | 15.0 | 9.2/4.8/3.5 | 15.4 | 13.6/5.8/3.7 | 24 | 5957 | 0 | 13 | - | yes | yes |  |
| combat_arms | nemexiswarq | yes | 10131 | 16.7/28.7 | 25.7/28.3 | 16.3 | 9.2/5.0/3.6 | 15.1 | 13.8/5.9/3.8 | 24 | 5983 | 0 | 13 | - | - | - | quarantine variant of nemexiswar |
| combat_arms | neptune | yes | 2362 | 6.8/10.1 | 3.6/4.5 | 4.7 | 5.5/3.4/2.3 | 7.2 | 6.8/2.9/1.9 | 24 | 955 | 0 | 2 | yes | yes | yes |  |
| combat_arms | noxiousdamnation | yes | 887 | 2.9/4.9 | 2.2/4.1 | 2.0 | 1.7/1.1/0.7 | 14.9 | 1.2/0.5/0.3 | 50 | 904 | 0 | 0 | yes | yes | yes |  |
| combat_arms | nwar | yes | 10193 | 16.5/28.3 | 25.7/27.5 | 15.1 | 8.5/4.5/3.4 | 15.5 | 13.5/5.8/3.7 | 24 | 5974 | 0 | 9 | - | yes | yes |  |
| combat_arms | oilrig | yes | 3159 | 10.1/20.0 | 10.7/12.4 | 7.3 | 4.6/2.6/1.9 | 13.2 | 4.9/2.1/1.3 | 24 | 1981 | 0 | 10 | yes | yes | yes |  |
| combat_arms | oilrigq | yes | 3160 | 10.2/19.7 | 10.7/12.7 | 7.4 | 5.0/2.8/2.1 | 12.8 | 5.1/2.2/1.4 | 24 | 1964 | 0 | 6 | - | - | - | quarantine variant of oilrig |
| combat_arms | overdose | yes | 1303 | 9.0/20.4 | 1.0/17.4 | 1.0 | 4.3/2.3/1.6 | 6.5 | 4.1/1.8/1.1 | 24 | 474 | 0 | 2 | yes | yes | yes |  |
| combat_arms | piazza | yes | 3282 | 11.4/18.4 | 19.6/20.9 | 15.3 | 6.6/3.7/2.6 | 6.5 | 10.4/4.5/2.8 | 24 | 1589 | 0 | 7 | - | yes | yes |  |
| combat_arms | piazzaq | yes | 3282 | 11.4/18.4 | 19.6/20.9 | 15.3 | 6.6/3.7/2.6 | 6.5 | 10.4/4.5/2.8 | 24 | 1594 | 0 | 5 | - | - | - | quarantine variant of piazza |
| combat_arms | pumpjack | yes | 2476 | 8.7/15.3 | 18.1/19.4 | 9.8 | 4.6/2.9/2.1 | 6.7 | 7.6/3.3/2.1 | 24 | 1110 | 0 | 5 | yes | yes | yes |  |
| combat_arms | railwaydefence | yes | 2404 | 9.0/19.1 | 0.6/1.2 | 11.9 | 3.4/2.0/1.6 | 12.2 | 4.1/1.7/1.1 | 8 | 946 | 0 | 1 | - | - | - | mode variant / not a standard map |
| combat_arms | rapidstream | yes | 2828 | 8.1/13.8 | 17.7/18.3 | 9.3 | 4.7/2.7/2.1 | 8.6 | 6.8/2.9/1.9 | 24 | 988 | 0 | 8 | yes | yes | yes | spawn spots confined (1336 of 2828 m2) |
| combat_arms | rattlesnake | yes | 3504 | 12.2/26.3 | 26.8/27.9 | 20.9 | 5.8/2.9/2.2 | 9.4 | 7.7/3.3/2.1 | 24 | 1921 | 0 | 9 | yes | yes | yes |  |
| combat_arms | roadkill | yes | 3564 | 9.7/20.3 | 24.0/25.5 | 13.2 | 4.5/2.6/2.0 | 12.4 | 5.9/2.5/1.6 | 24 | 3361 | 0 | 16 | yes | yes | yes |  |
| combat_arms | roadkill2 | yes | 3588 | 9.4/19.9 | 23.5/25.2 | 13.2 | 4.5/2.6/2.1 | 12.7 | 5.8/2.5/1.6 | 24 | 3350 | 0 | 21 | yes | yes | yes |  |
| combat_arms | ruralestate | yes | 5361 | 12.3/20.4 | 11.5/17.3 | 9.5 | 7.2/3.8/2.8 | 11.3 | 9.7/4.2/2.7 | 24 | 2818 | 0 | 7 | yes | yes | yes |  |
| combat_arms | sandhog | yes | 2694 | 7.1/12.5 | 15.5/17.6 | 9.1 | 3.9/2.4/1.7 | 9.3 | 6.0/2.6/1.6 | 24 | 1621 | 0 | 11 | yes | yes | yes |  |
| combat_arms | sandhog_cp1 | yes | 1137 | 5.0/10.9 | 0.4/0.7 | 9.6 | 2.5/1.3/1.0 | 9.4 | 2.5/1.1/0.7 | 8 | 607 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | sector25 | yes | 1824 | 8.2/16.9 | 17.3/19.0 | 10.5 | 4.0/2.2/1.6 | 7.7 | 4.9/2.1/1.3 | 24 | 111 | 0 | 1 | yes | yes | yes | spawn spots confined (184 of 1824 m2) |
| combat_arms | shortfuse | yes | 3239 | 11.3/21.5 | 17.5/19.7 | 13.4 | 6.0/3.2/2.1 | 8.4 | 8.0/3.4/2.2 | 24 | 1496 | 0 | 14 | yes | yes | yes |  |
| combat_arms | shortfuse2 | yes | 3614 | 10.9/19.1 | 17.2/19.6 | 12.4 | 6.2/3.4/2.3 | 7.8 | 9.5/4.1/2.6 | 24 | 1797 | 0 | 8 | yes | yes | yes |  |
| combat_arms | slaughterhouse | yes | 2519 | 9.7/17.4 | 14.4/16.6 | 8.3 | 4.7/2.6/1.8 | 7.4 | 7.0/3.0/1.9 | 18 | 1292 | 0 | 2 | yes | yes | yes |  |
| combat_arms | snowrobot | yes | 7982 | 12.8/22.1 | 13.2/19.2 | 11.3 | 7.2/4.2/3.2 | 13.7 | 12.0/5.1/3.3 | 24 | 5175 | 0 | 19 | - | yes | yes |  |
| combat_arms | snowstorm | yes | 3344 | 6.9/11.5 | 0.8/1.3 | 3.7 | 3.7/2.2/1.7 | 14.3 | 4.8/2.1/1.3 | 8 | 2125 | 0 | 2 | yes | yes | yes |  |
| combat_arms | snowstorm_qrt | yes | 3700 | 7.1/11.9 | 1.4/2.3 | 4.3 | 3.9/2.4/1.9 | 13.4 | 5.7/2.4/1.6 | 32 | 2246 | 0 | 3 | - | - | - | mode variant / not a standard map |
| combat_arms | snowvalley | yes | 7992 | 12.9/21.9 | 13.2/19.3 | 11.7 | 7.5/4.4/3.2 | 13.5 | 12.2/5.2/3.3 | 24 | 5196 | 0 | 8 | - | yes | yes |  |
| combat_arms | snowvalleyq | yes | 7984 | 12.8/22.2 | 13.2/19.2 | 11.3 | 7.3/4.2/3.2 | 13.9 | 11.8/5.1/3.2 | 24 | 5175 | 0 | 19 | - | - | - | quarantine variant of snowvalley |
| combat_arms | square | yes | 2910 | 8.5/15.7 | 23.3/25.1 | 12.4 | 4.8/3.0/2.2 | 8.4 | 7.1/3.0/1.9 | 24 | 1937 | 0 | 7 | yes | yes | yes |  |
| combat_arms | terrordome | yes | 3760 | 11.2/17.9 | 12.5/17.3 | 8.7 | 6.1/3.7/2.5 | 8.0 | 9.6/4.1/2.6 | 28 | 2207 | 0 | 4 | yes | yes | yes |  |
| combat_arms | testing_t | yes | 2364 | 8.3/16.5 | 10.4/12.6 | 7.1 | 4.4/2.4/1.9 | 7.7 | 6.3/2.7/1.7 | 24 | 970 | 0 | 6 | - | - | - | mode variant / not a standard map |
| combat_arms | towerbridge | yes | 1811 | 6.9/15.3 | 13.7/17.8 | 8.2 | 3.4/2.1/1.6 | 9.2 | 4.0/1.7/1.1 | 24 | 1259 | 0 | 11 | yes | yes | yes |  |
| combat_arms | towerdefence | yes | 161 | 1.8/3.1 | 0.6/1.4 | 1.5 | 0.8/0.4/0.3 | 4.2 | 0.8/0.3/0.2 | 8 | 95 | 0 | 1 | - | - | - | mode variant / not a standard map; tiny region (spawn room only?) |
| combat_arms | training | yes | 720 | 2.5/4.2 | / | 1.6 | 1.4/0.9/0.7 | 13.3 | 1.1/0.5/0.3 | 1 | 499 | 0 | 2 | - | - | - | mode variant / not a standard map |
| combat_arms | twinhigh | yes | 460 | 1.6/3.0 | 0.6/1.3 | 1.3 | 1.3/0.6/0.6 | 47.2 | 0.2/0.1/0.1 | 24 | 251 | 0 | 0 | yes | yes | yes |  |
| combat_arms | twinship | yes | 2277 | 5.3/11.1 | 1.1/2.3 | 11.6 | 3.6/2.2/1.7 | 9.0 | 5.2/2.2/1.4 | 24 | 48 | 0 | 0 | yes | yes | yes | spawn spots confined (501 of 2277 m2) |
| combat_arms | twintower | yes | 744 | 3.8/8.9 | 0.6/0.6 | 6.0 | 2.5/1.3/0.8 | 23.8 | 0.6/0.3/0.2 | 26 | 426 | 0 | 0 | yes | yes | yes |  |
| combat_arms | twotowers | yes | 2051 | 7.5/16.4 | 2.3/6.4 | 11.6 | 4.2/2.3/1.6 | 10.9 | 3.9/1.7/1.1 | 24 | 703 | 0 | 7 | yes | yes | yes | spawn spots confined (974 of 2051 m2) |
| combat_arms | vertigo | yes | 1359 | 5.4/9.9 | 10.0/10.8 | 5.4 | 2.9/1.8/1.3 | 10.2 | 2.7/1.2/0.7 | 16 | 545 | 0 | 1 | yes | yes | yes |  |
| combat_arms | vertigo_q | yes | 1356 | 5.3/9.8 | 10.0/10.8 | 5.4 | 2.8/1.8/1.3 | 10.4 | 2.7/1.2/0.7 | 16 | 545 | 0 | 1 | - | - | - | quarantine variant of vertigo |
| combat_arms | warehouse | yes | 1820 | 6.6/11.4 | 14.5/14.9 | 7.5 | 4.0/2.4/1.7 | 8.1 | 4.6/2.0/1.3 | 24 | 147 | 0 | 1 | yes | yes | yes | spawn spots confined (199 of 1820 m2) |
| combat_arms | warehouse_qrt | yes | 1817 | 6.7/11.5 | 6.5/10.3 | 5.0 | 4.0/2.5/1.8 | 8.2 | 4.6/2.0/1.2 | 20 | 716 | 0 | 3 | - | - | - | mode variant / not a standard map |
| combat_arms | warhead | yes | 2940 | 12.2/26.5 | 19.6/21.0 | 15.0 | 5.5/3.0/2.1 | 7.4 | 8.2/3.5/2.2 | 24 | 903 | 0 | 1 | yes | yes | yes |  |
| combat_arms | waverider | yes | 4192 | 13.7/24.0 | 23.3/24.9 | 13.3 | 7.6/4.4/3.1 | 8.5 | 10.1/4.3/2.8 | 24 | 958 | 0 | 7 | - | yes | yes | spawn spots confined (1297 of 4192 m2) |
| combat_arms | zombieroom | yes | 299 | 1.2/1.9 | 0.0/0.4 | 0.6 | 1.0/0.6/0.6 | 6.2 | 1.0/0.4/0.3 | 24 | 159 | 0 | 1 | - | - | - | mode variant / not a standard map |
| combat_arms | zombiewar | yes | 2239 | 3.1/5.6 | 0.8/1.9 | 0.6 | 2.5/1.7/1.3 | 19.9 | 2.3/1.0/0.6 | 16 | 1214 | 0 | 0 | - | - | - | mode variant / not a standard map |
| hl2dm | dm_lockdown | yes | 3119 | 11.0/19.8 | 12.5/20.9 | 10.3 | 5.3/3.0/2.3 | 8.0 | 8.1/3.5/2.2 | 33 | 1970 | 0 | 8 | yes | yes | yes |  |
| hl2dm | dm_overwatch | yes | 3088 | 8.7/15.0 | 12.0/17.6 | 7.4 | 4.4/2.5/1.8 | 11.4 | 5.6/2.4/1.5 | 17 | 2175 | 0 | 6 | yes | yes | yes |  |
| hl2dm | dm_powerhouse | yes | 4113 | 11.1/17.8 | 12.1/16.0 | 7.1 | 6.1/3.7/2.7 | 6.9 | 12.3/5.3/3.3 | 18 | 2510 | 0 | 11 | - | yes | yes |  |
| hl2dm | dm_resistance | yes | 1917 | 6.3/10.6 | 6.9/10.5 | 6.0 | 3.4/1.9/1.3 | 9.4 | 4.2/1.8/1.1 | 20 | 1318 | 0 | 8 | yes | yes | yes |  |
| hl2dm | dm_runoff | yes | 6179 | 13.7/27.8 | 11.2/24.3 | 6.5 | 6.4/3.4/2.6 | 15.0 | 8.5/3.6/2.3 | 9 | 4446 | 0 | 23 | yes | yes | yes |  |
| hl2dm | dm_steamlab | yes | 1995 | 9.3/14.6 | 10.0/15.1 | 8.2 | 4.6/2.5/1.8 | 7.3 | 5.6/2.4/1.5 | 24 | 1188 | 0 | 8 | yes | yes | yes |  |
| hl2dm | dm_underpass | yes | 2176 | 9.8/17.9 | 11.7/20.4 | 8.8 | 5.5/2.7/2.0 | 7.1 | 6.3/2.7/1.7 | 17 | 1213 | 0 | 5 | yes | yes | yes |  |
| hl2dm | halls3 | no | 0 | 0.0/0.0 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0 | 0 | 0 | 0 | - | - | - | not a dm_ map; maps/halls3.bsp: hl2dm: maps/halls3.bsp: record 123291a97597a53481da4097460439b380ae9219f371aec8d00d87634f6166b7: No such file or directory (os error 2) |
