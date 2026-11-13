# Map pool fitness

Started 2026-11-13. Goal: a bigger Ultimate Gun Game pool (more CS:S maps,
custom arenas, more standard Combat Arms maps) without maps so big that
nobody finds anybody; tiny chaotic maps welcome. This plan measures every
installed map, sets a rule that picks the pool by player count (the
user: "fine to start with, we can tweak later"), and carries it as data;
wired in 2026-11-13 (step 3, "Data shape (as wired)"). Parent plan:
[ultimate-gun-game.md](ultimate-gun-game.md).

How to check: `map_fitness` (apps/party_kit/src/bin/map_fitness.rs,
`--features tools`; docs/OBSERVABILITY.md, "Measuring maps for a gun
game's pool") writes one CSV row per installed map. The sweep of
2026-11-13 is [map-pool-fitness.csv](map-pool-fitness.csv) (map names and
numbers only), readable in the table at the end: the second sweep of
that day, after the spawn spots' fixes (below, "Spawn spots").

## How a map is measured

Each map loads as the dedicated server loads it (no textures, no kit) on
Combat Arms' simulation with the party's parts (the map logic's brush
entities) and the gun game started, then:

- **The walkable area**: the bots' generated navigation mesh (Combat
  Arms' walker, jump-ups included, meeting the map's player clips),
  flooded from the map's spawn points, bounded to the clusters the spawn
  spots' walking player reaches where the map has visibility data; its
  main region (areas walked to from a spawn and
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
- **Spawns**: the map's spawn points (count, least and median distance,
  how many land on the main region), the spawn spots `random_hidden`
  picks from (`spawns::candidates`), how many of those are off the map
  and how many a standing player would be stuck in
  (`party_kit::spot_checks`, also the heavy guard's).

Calibration (wander at 4/8/12 players, s): CA darkhills 0.9/0.4/0.2 and
warehouse 4.6/2.0/1.3 (the small chaotic ones), gg_ arenas 1.9-4.5 at 4,
de_dust2 9.7/4.1/2.6, cs_office 10.5/4.5/2.9, de_nuke 12.4/5.3/3.4,
de_port 11.3/4.8/3.1, grayhammer 15.0/6.4/4.1, cs_militia 8.9/3.8/2.4
(31.2/13.4/8.5 while its walk leaked past the player clips). The stock CS:S maps are made for 10-32 players and
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
(section "Spawn spots": the confined maps, fixed).

Resulting pools by the rule alone, before the deny lists (140 maps measured fit at 8; counts by game CS:S / Combat
Arms / HL2:DM):

- **4 players: 118** (48 / 64 / 6): every installed gg_ arena (42), the
  small and middle-sized Combat Arms maps (warehouse, junk_flea,
  darkhills, bloodmoney and 60 more), cs_assault, cs_italy, cs_militia,
  de_dust2, de_prodigy, de_tides, and HL2:DM's dm_lockdown,
  dm_overwatch, dm_resistance, dm_runoff, dm_steamlab, dm_underpass.
- **8 players: 140**: the above plus cs_compound, cs_havana, cs_office,
  de_aztec, de_cbble, de_chateau, de_dust, de_inferno, de_nuke,
  de_piranesi, de_port, de_train, Combat Arms' cityofwings, grayhammer,
  heavymetal, nemexiswar, nwar, piazza, snowrobot, snowvalley,
  waverider, and dm_powerhouse.
- **12 players: 141**: the above plus Combat Arms' 16rooms (a mode map
  by its look).

Against the first sweep: cs_militia (area 31564 m2 then, its walk on the
hills) is in from 4 players, cs_havana (341 spots off its `.nav` then)
from 8; de_train is out at 4 (its area 3128 to 3598 m2: the walk that
bounds the bots' mesh now fits doorways the grid's centres missed,
wander_4 8.7 to 10.5 s).

The table at the end has each map's pool membership. No custom aim_,
fy_, awp_ or ka_ maps are installed; the gg_ maps (several of them fy_
arenas: gg_fy_funtimes, gg_fy_tactic_fight) are the custom arenas there
are.

## For a human look

- **Area over 15000 m2**: Combat Arms' darkthandark (23651, open: sight
  49 m), deserthole and desertholezone (19710).
- **nemexishq**: no spawn spots at all (spawns at its 8 points only).

## Spawn spots (fixed 2026-11-13)

The first sweep found the spawn spots off the map or stuck on many maps;
each cause and its fix (`chimera_sim::minigame::spawns`, its module doc),
with the spots before and after (the first sweep, then the second; the
CSV is the second):

| finding | cause | fix | before | after |
|---|---|---|---|---|
| cs_militia's spots on the hills | Source maps' player clips are not in the physics world (Source's movement sweeps `core::MapBrushes` itself): the spots' walk crossed the clip walls round the yard onto the hills and the flat beyond; no wall or clip of the world's own bounds the yard | the walk and the spot check meet the clips (`nav_gen::PlayerClips`, `Clipped`); the spots are drawn on the map's own `.nav` where it ships one (`MapData::floors`, `MapFloors`, loaded whatever the kit, apart from the bots' mesh) | 22061 of 24827 off its `.nav`; area 31564 m2 | 0 of 2739; area 4676 m2 |
| cs_havana's floor patch x -2315..-1306, y 1248..1696 | the patch is inside one big player clip (brush 1568 of 3217, about x -2688..-992, y 896..1584, z -224..640: an out-of-bounds yard). The map logic opens it: havana's `logic_auto` picks one of its blockade layouts at map spawn (`lc_blockage`; `block_a`..`d` disabled, `door_b`/`c`/`d` func_brushes enabled) and opens a route past the yard (bots' area 3758 m2 bare, 5228 with the logic); the walk then entered the clip, which it didn't see | as cs_militia's | 341 of 2811 off its `.nav` | 0 of 1917 |
| de_chateau, de_port (roofs), cs_compound, cs_italy, cs_office, de_cbble, de_piranesi, de_tides | the walk reached floors the map's `.nav` leaves out (roofs, ledges) | the spots on the map's `.nav` | 17, 7, 2, 1, 1, 1, 1, 1 off | 0 each |
| stuck spots | the check was a capsule (radius 0.4 m), shorter than a box at the corners, and missed the player clips | a standing player's box, Source's 32 by 72 units from a stair step to the head (`spawns::standing_box`, `stuck_at`), and the clips | 985 of 402942 spots, 145 of 222 maps | 0, 0 maps |
| spots confined (Combat Arms' warehouse in its two spawn rooms, 199 of 1820 m2; sector25, castle, battleship, battleship1, cargoship, twinship, killland, lastvegas, twotowers, crystaltower, rapidstream, waverider, downhill, grayhammer, gg_lego_spacetower2) | not the doorways' width, mostly: the spawn rooms open by sills a little above the stair step (warehouse's 0.69 m: the floor at -107.05, the next at -106.42), castle's stairs climb more than a walking step a cell, and lego bricks are jumped; the walker only walked (0.45 m steps). The grid's centres missing a doorway's middle added a little (warehouse 199 to 227 m2 with the box sliding alone) | the walker climbs what the map's game's players climb (`SpawnClimbs`, per game: Combat Arms' and CS:S's 0.8 m jump-ups; bounded to the clusters a walk reaches where the map has visibility data, so a jump over a low wall doesn't leave it), and its box may stand 0.2 m off a cell's centre (`NavGenParams::slide`) | e.g. warehouse 199, sector25 184, castle 350, gg_lego_spacetower2 652 m2 | warehouse 1919 (its bots' area 1820), sector25 1918, castle 3464, gg_lego_spacetower2 1419; every listed map's spots over at least its bots' area |
| map spawn points off the region (de_piranesi 14 of 40, de_aztec, de_cbble, de_chateau 5, hellgate 11 of 24) | they float over their floors (de_piranesi's T and CT points 1.5-1.7 m, hellgate's 3.2 m) and were measured where they stand, not where a spawn there lands | none needed: dropped to their floors (`spot_checks::dropped`) every one is on the main region, so the gun game keeps them (it spawns at points only while the spots are being made, or on a map with none) | 14, 5, 5, 5, 11 off | 0 |

The client and the server load the same collision at havana's reported
spot: the same 3217 player-solid brushes with the clip among them, and
the same colliders (`map_fitness --probe` with and without `--client`).
The earlier "nothing there" came from asking the physics world alone,
which lacks the player clips; the player held a CS:S gun (Source's
movement, which sweeps the clips) and was inside the clip on both sides.
What does differ is the two movements: Combat Arms' movement asks the
physics world only, so on a CS:S map a player holding a Combat Arms gun
walks through every player clip (onto militia's hills, into havana's
yard). Left for its own change (a collider for the clips on the
characters' clip layer), not made here.

What still shows (the second sweep): 181 spots off a map's main region,
all on four maps whose spawn points have no walkable region (the spots
are the points: bhop_myztek 40, surf_nsz_fix 61 and surf_threnody 64,
these two off their `.nav` too, Combat Arms' hgoilrig 16); none on a
pool map. The main region of a map without a `.nav` is the bots' mesh
fitting doorways as the spots' walker does (`spot_checks::bots_mesh`,
its box sliding half its radius): without it a gap the spots' walk took
and the grid's centres missed counted spots off on 30 maps (dredge 158 of
981, towerdefence 239 of 330, ghosttown 53, gg_lego_mafia_arena 35); those
rows were measured again.

The heavy guard `party_kit` `heavy::spawn_spots` loads every map of the
pool as the dedicated server does and fails on a spot off its `.nav`, a
stuck spot, or more than 1% (at least 10) off the main region
(`party_kit::spot_checks`, what `map_fitness` reports).

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

## Data shape (as wired)

Data first: the measurements are run-time data per game, the rule's
numbers are data the minigame definition tunes, and the pool is computed
from both at every map change, with no map list in Rust
(`chimera_sim::minigame::map_fit`).

- `games/<game>/data/map_fitness.toml`, generated by `map_fitness
  --write <repo>` after a sweep, or `--csv <sweep.csv> --write <repo>`
  from a sweep's CSV without loading anything (the committed files are
  this sweep's), one table per map:

  ```toml
  [maps.de_dust2]
  area_m2 = 4894
  sight_m = 10.4
  spot_area_m2 = 5116
  spawns = 40
  spots = 2205
  spots_off = 0
  stuck = 0
  wander_s = { 4 = 9.7, 8 = 4.1, 12 = 2.6 }
  ```

  (`loads = false` for a map that loaded without a walkable region.)
  `wander_s` is for reading: the host works `wander_N` out again from
  `area_m2` and `sight_m` at any count. A map the file lacks (installed
  later) is in the draw at `unknown_weight`, said once in the log.
- `games/<game>/data/map_pool.toml`, hand-written, the same form as
  `ladder_exclusions.toml`: `[[deny]]` and `[[allow]]` tables of `maps`
  (`*` anywhere: `bhop_*`, `*_cp`) and their `reason`. Allow wins over
  deny and over the measurements' flags (area over `look_area_m2`, spots
  off the map), not over the count. A game with rules offers every
  installed map they don't deny (`auto::report`); `GameParts::
  map_patterns` is gone, the claimed `maps` stay first in the order.
- Registered by each game as its exclusions are (`map_fit::
  MapPoolAppExt`: `add_map_pool_rules`, `add_map_fitness`, from
  `parts::add_map_pool`) into one resource, `MapPool`.
- The rule's numbers: chimera/sim/data/map_fit.toml (`target_s = 10`,
  `small_m2 = 1500`, `chaos_s = 3`, `look_area_m2 = 15000`,
  `look_off_share = 0.05`, `speed_mps = 6.35`, `sight_cap_m = 50`, the
  weights below), tuned by a definition's `map_fit = { target_s = 8 }`
  (`map_fit = true`/`false`; absent: on with `rotation = "random"`, so
  Ultimate Gun Game's definition text is unchanged and older net42
  clients still read it).
- The weights (`weights`, `small_scale`, `unknown_weight`): the curve
  over `wander_N` is `[[0, 0.2], [2, 0.3], [4, 1.0], [8, 1.0], [10,
  0.5]]` (straight between the points), a small map's weight a quarter
  of it, a map without measurements 0.5. With this sweep the small maps
  take about 11% of draws at 4 players, 5% at 8, 8% at 12.
- At every map change (the rotation, the end-of-match vote's ballot, a
  host's `changelevel` with no map) the host counts the characters
  present, humans and bots (`map_fit::present`), and draws by the
  weights (`MapHistory::next_weighted`, `vote::candidates_weighted`),
  the last `avoid_recent` played held back as before. `auto::map_pool`
  keeps its callers; `auto::map_pool_for` takes the count. The console's
  `map_pool [players]` lists the pool at a count.
- The asset offer's prefetch lists the maps let in at the present count,
  likeliest first (`map_fit::likeliest`).

Pool sizes with the deny lists as committed (CS:S / Combat Arms /
HL2:DM), from the second sweep after the spawn spot fixes (cs_militia,
cs_havana and the confined Combat Arms maps off the deny lists, dredge
on it until looked at): 4 players 116 (48 / 62 / 6), 8 players 138 (60 / 71 / 7), 12
players 138 (before the fixes: 104, 122, 122).

## Steps

1. Done: the tool and the 2026-11-13 sweep (this file, the CSV).
2. Done: the spawn findings (section "Spawn spots"), with the heavy
   guard `heavy::spawn_spots` over the pool.
3. Done (2026-11-13, branch `map-pool-by-count`): `map_fitness
   --write`, the two data files per game, the registration, the draw by
   player count (the rotation, the vote, `changelevel` with no map),
   `map_pool`; tests: `map_fit`'s unit tests (the pool by count, the
   rules, the fallback, the settings), `rotation`'s and `vote`'s
   weighted draws and hold-back, party_kit's `map_pool_by_count`
   (scenario: the vote and the rotation never take a map too big for
   the players present), ultimate_gun_game's `heavy::pool_maps` (the
   real server at 4/8/12 bots: each drawn map loads, has spots and its
   published sets cover what its load reads; sampled, `CHIMERA_POOL_MAPS=all`
   for every map). The spawn fixes landed: cs_militia, cs_havana
   and the confined Combat Arms maps are off the deny lists, the data
   written from the second sweep's CSV (`--csv ... --write`).
4. Optional: a lobby option to show the pool, and the run-time
   measurement of maps the data lacks.

## The sweep of 2026-11-13

Times in seconds at 6.35 m/s; `pair` and `spawn-pair` p50/p90; `meet`
and `wander` at 4/8/12 players; `spots`: the spawn spots
`random_hidden` picks from; `off map` and `stuck` among them; 4/8/12:
in the pool at that many players.

| game | map | loads | area m2 | pair p50/p90 s | spawn-pair p50/p90 s | spawn-hub p50 s | meet 4/8/12 s | sight m | wander 4/8/12 s | spawns | spots | off map | stuck | 4 | 8 | 12 | note |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| cs_source | bhop_addict_v2_3xl | yes | 2185 | 4.5/8.0 | 2.5/4.4 | 3.1 | 2.5/1.6/1.3 | 21.9 | 2.1/0.9/0.6 | 13 | 1200 | 0 | 0 | - | - | - | course/minigame map (bhop_*) |
| cs_source | bhop_backport_css | yes | 72 | 0.6/1.3 | 0.6/1.3 | 0.6 | 0.5/0.0/0.0 | 10.0 | 0.1/0.1/0.0 | 15 | 44 | 0 | 0 | - | - | - | course/minigame map (bhop_*); tiny region (spawn room only?) |
| cs_source | bhop_flatzone | yes | 1334 | 3.8/6.9 | 0.0/0.6 | 2.5 | 1.9/1.3/1.2 | 17.5 | 1.6/0.7/0.4 | 22 | 739 | 0 | 0 | - | - | - | course/minigame map (bhop_*) |
| cs_source | bhop_myztek | no | -0 | 0.0/0.0 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 40 | 0 | 40 | 0 | - | - | - | course/minigame map (bhop_*); no walkable region from the spawn points; 40 of 40 spawns off the region; 40 of 0 spawn spots off the map |
| cs_source | cs_assault | yes | 4412 | 8.8/17.6 | 22.2/31.7 | 13.9 | 4.7/2.6/2.1 | 14.7 | 6.2/2.7/1.7 | 40 | 2950 | 0 | 0 | yes | yes | yes |  |
| cs_source | cs_compound | yes | 7638 | 11.4/18.7 | 18.6/20.1 | 14.6 | 6.4/4.2/3.2 | 14.4 | 10.9/4.7/3.0 | 32 | 3605 | 0 | 0 | - | yes | yes |  |
| cs_source | cs_havana | yes | 4258 | 11.1/17.6 | 16.0/19.6 | 10.6 | 6.5/3.8/2.7 | 7.8 | 11.2/4.8/3.1 | 40 | 1917 | 0 | 0 | - | yes | yes |  |
| cs_source | cs_italy | yes | 3244 | 12.6/22.5 | 18.6/23.9 | 10.6 | 7.0/3.9/2.8 | 7.2 | 9.2/4.0/2.5 | 40 | 1621 | 0 | 0 | yes | yes | yes |  |
| cs_source | cs_militia | yes | 4676 | 9.2/32.5 | 1.0/2.2 | 10.7 | 6.3/3.7/2.8 | 10.8 | 8.9/3.8/2.4 | 32 | 2739 | 0 | 0 | yes | yes | yes |  |
| cs_source | cs_office | yes | 3994 | 13.3/20.9 | 15.4/20.1 | 10.8 | 6.7/3.7/2.5 | 7.8 | 10.5/4.5/2.9 | 40 | 1771 | 0 | 0 | - | yes | yes |  |
| cs_source | de_aztec | yes | 6122 | 13.2/22.9 | 26.9/30.9 | 16.7 | 7.5/4.1/2.9 | 11.7 | 10.8/4.6/2.9 | 40 | 2521 | 0 | 0 | - | yes | yes |  |
| cs_source | de_cbble | yes | 6508 | 15.0/25.6 | 24.1/34.0 | 13.7 | 7.6/4.6/3.1 | 10.7 | 12.5/5.4/3.4 | 40 | 3080 | 0 | 0 | - | yes | yes |  |
| cs_source | de_chateau | yes | 4206 | 14.4/28.9 | 26.9/28.9 | 19.1 | 7.8/4.4/3.1 | 6.8 | 12.8/5.5/3.5 | 40 | 1676 | 0 | 0 | - | yes | yes |  |
| cs_source | de_dust | yes | 5944 | 13.6/23.4 | 25.6/28.6 | 13.2 | 7.3/4.2/2.8 | 11.7 | 10.4/4.5/2.8 | 41 | 2583 | 0 | 0 | - | yes | yes |  |
| cs_source | de_dust2 | yes | 4894 | 12.7/20.5 | 17.2/19.9 | 11.9 | 7.2/3.8/2.6 | 10.4 | 9.7/4.1/2.6 | 40 | 2205 | 0 | 0 | yes | yes | yes |  |
| cs_source | de_inferno | yes | 3991 | 12.6/20.8 | 22.7/25.4 | 12.3 | 6.9/4.0/3.0 | 7.1 | 11.7/5.0/3.2 | 40 | 2125 | 0 | 0 | - | yes | yes |  |
| cs_source | de_nuke | yes | 5286 | 12.8/23.4 | 28.7/31.8 | 20.7 | 6.8/3.8/2.9 | 8.8 | 12.4/5.3/3.4 | 40 | 3063 | 0 | 0 | - | yes | yes |  |
| cs_source | de_piranesi | yes | 5286 | 11.9/19.0 | 25.3/28.4 | 18.4 | 6.9/4.0/2.9 | 9.4 | 11.6/5.0/3.2 | 40 | 2449 | 0 | 0 | - | yes | yes |  |
| cs_source | de_port | yes | 10262 | 18.2/28.9 | 30.0/33.1 | 20.5 | 9.3/5.6/4.0 | 18.7 | 11.3/4.8/3.1 | 40 | 5579 | 0 | 0 | - | yes | yes |  |
| cs_source | de_prodigy | yes | 2160 | 17.3/32.0 | 15.6/20.5 | 12.3 | 7.2/3.3/2.2 | 5.6 | 7.9/3.4/2.2 | 40 | 1173 | 0 | 0 | yes | yes | yes |  |
| cs_source | de_tides | yes | 2449 | 10.4/16.8 | 11.9/13.6 | 8.4 | 5.3/2.8/2.0 | 7.5 | 6.8/2.9/1.8 | 40 | 1033 | 0 | 0 | yes | yes | yes |  |
| cs_source | de_train | yes | 3598 | 11.4/19.4 | 20.4/24.2 | 11.6 | 7.0/3.9/2.7 | 7.1 | 10.5/4.5/2.9 | 40 | 1850 | 0 | 0 | - | yes | yes |  |
| cs_source | gg_4mida_arena | yes | 1453 | 5.6/8.3 | 8.0/9.3 | 5.9 | 3.1/1.8/1.3 | 8.8 | 3.4/1.5/0.9 | 42 | 1185 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_beacon | yes | 1736 | 5.2/9.4 | 6.1/7.8 | 3.6 | 3.1/1.9/1.3 | 8.1 | 4.4/1.9/1.2 | 32 | 1078 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_bk_warehouse_v1 | yes | 1591 | 5.1/8.2 | 8.7/10.4 | 5.0 | 3.0/1.8/1.3 | 7.3 | 4.5/1.9/1.2 | 24 | 985 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_blue_arena_32a | yes | 984 | 4.8/8.4 | 7.2/8.5 | 4.0 | 2.2/1.2/0.9 | 7.9 | 2.6/1.1/0.7 | 36 | 626 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_cb_arctic | yes | 959 | 4.1/7.0 | 3.5/5.3 | 2.7 | 2.2/1.2/0.9 | 9.0 | 2.2/0.9/0.6 | 32 | 758 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_churches_x_final_fixed | yes | 1361 | 4.9/8.7 | 7.6/8.8 | 6.2 | 2.9/1.6/1.3 | 7.3 | 3.8/1.6/1.0 | 32 | 808 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_construct_city | yes | 1064 | 3.9/6.8 | 7.4/8.6 | 4.4 | 2.2/1.4/1.0 | 6.8 | 3.2/1.4/0.9 | 48 | 613 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_deagle7k | yes | 1680 | 5.3/8.6 | 7.6/9.0 | 4.7 | 3.1/1.9/1.3 | 7.9 | 4.4/1.9/1.2 | 44 | 927 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_desert_paintball | yes | 1440 | 4.0/6.8 | 6.0/7.7 | 4.0 | 2.3/1.5/1.1 | 9.6 | 3.1/1.3/0.8 | 88 | 847 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_dev_moment_v1 | yes | 833 | 3.2/5.5 | 5.1/6.4 | 3.9 | 1.8/1.2/0.8 | 7.3 | 2.4/1.0/0.6 | 40 | 469 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_dinoiceworld | yes | 1236 | 4.0/6.7 | 5.7/7.6 | 3.5 | 2.3/1.4/1.1 | 7.8 | 3.3/1.4/0.9 | 32 | 924 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_fusion_trx | yes | 1058 | 3.8/6.7 | 7.6/8.9 | 4.9 | 2.2/1.3/1.0 | 9.2 | 2.4/1.0/0.6 | 28 | 418 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_future | yes | 2158 | 6.0/9.9 | 7.3/9.2 | 4.5 | 3.3/1.9/1.3 | 11.4 | 3.9/1.7/1.1 | 56 | 1495 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_fy_funtimes | yes | 1596 | 5.3/8.8 | 7.8/9.8 | 4.3 | 3.0/1.7/1.3 | 9.5 | 3.5/1.5/0.9 | 32 | 909 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_fy_tactic_fight | yes | 1786 | 4.5/7.3 | 5.6/6.9 | 3.2 | 2.7/1.8/1.3 | 11.1 | 3.3/1.4/0.9 | 32 | 793 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_hex | yes | 815 | 3.3/5.5 | 7.1/7.9 | 3.9 | 1.9/1.2/0.9 | 6.6 | 2.6/1.1/0.7 | 24 | 619 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_iceworld_l33t | yes | 1326 | 4.6/7.5 | 7.0/8.5 | 4.1 | 2.6/1.5/1.2 | 9.4 | 2.9/1.2/0.8 | 24 | 938 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_ilu | yes | 1100 | 4.1/7.9 | 8.4/10.1 | 4.8 | 2.3/1.3/1.1 | 7.0 | 3.2/1.4/0.9 | 40 | 700 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_legendary_fun_v1a | yes | 1069 | 4.0/6.6 | 6.8/7.6 | 3.6 | 2.4/1.3/1.0 | 7.8 | 2.8/1.2/0.8 | 32 | 622 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_lego_2floor_v2 | yes | 2330 | 5.2/8.2 | 6.9/9.1 | 5.6 | 3.3/2.0/1.5 | 11.2 | 4.3/1.8/1.2 | 32 | 1317 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_lego_mafia_arena | yes | 1502 | 6.4/10.1 | 5.0/6.4 | 3.2 | 3.6/2.1/1.5 | 8.5 | 3.7/1.6/1.0 | 32 | 946 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_lego_spacetower2 | yes | 1408 | 4.5/6.9 | 6.3/8.2 | 4.3 | 2.7/1.7/1.2 | 17.5 | 1.7/0.7/0.5 | 90 | 1187 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_mario_vs_wario | yes | 1898 | 4.6/7.7 | 6.7/8.5 | 4.4 | 2.8/1.7/1.3 | 8.8 | 4.5/1.9/1.2 | 40 | 1038 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_mario_world_2 | yes | 2661 | 5.1/8.0 | 6.9/8.6 | 4.1 | 3.3/2.2/1.7 | 13.7 | 4.0/1.7/1.1 | 56 | 1538 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_minesweeper | yes | 1200 | 4.1/6.6 | 5.4/7.2 | 3.8 | 2.3/1.3/1.1 | 7.0 | 3.5/1.5/1.0 | 32 | 661 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_mr_pillar_v1 | yes | 842 | 3.5/6.3 | 7.8/9.0 | 4.6 | 2.0/1.2/1.0 | 7.0 | 2.5/1.1/0.7 | 40 | 586 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_nukkon_hdr | yes | 1067 | 4.7/8.1 | 8.3/9.7 | 5.2 | 2.5/1.4/1.1 | 7.6 | 2.9/1.2/0.8 | 36 | 761 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_nutty | yes | 1276 | 4.4/8.0 | 7.5/8.5 | 4.1 | 2.6/1.6/1.2 | 8.7 | 3.0/1.3/0.8 | 32 | 949 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_pokemonz | yes | 1168 | 4.3/7.6 | 6.5/10.2 | 5.0 | 2.5/1.6/1.1 | 7.1 | 3.4/1.4/0.9 | 102 | 672 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_sex_fix | yes | 895 | 4.6/7.0 | 5.8/7.3 | 3.5 | 2.4/1.3/1.0 | 6.5 | 2.8/1.2/0.8 | 60 | 557 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_simpsons_arabtoon | yes | 412 | 2.8/5.8 | 6.9/8.0 | 3.9 | 1.5/0.8/0.6 | 5.0 | 1.7/0.7/0.5 | 50 | 246 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_simpsons_bam | yes | 1031 | 3.2/5.7 | 5.4/7.7 | 3.7 | 2.3/1.5/1.1 | 9.7 | 2.2/0.9/0.6 | 48 | 649 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_simpsons_d | yes | 817 | 4.0/6.8 | 5.8/6.9 | 3.4 | 2.3/1.3/0.9 | 5.7 | 2.9/1.3/0.8 | 40 | 405 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_simpsons_dusty_2 | yes | 824 | 4.2/7.3 | 7.6/8.6 | 4.5 | 2.1/1.1/0.8 | 8.7 | 2.0/0.8/0.5 | 24 | 491 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_simpsons_funfreaks_v2 | yes | 808 | 3.1/5.1 | 4.7/6.5 | 3.5 | 1.8/1.1/0.8 | 8.7 | 1.9/0.8/0.5 | 68 | 523 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_spacoid_pg | yes | 1314 | 4.6/7.5 | 3.7/5.3 | 2.5 | 2.6/1.6/1.3 | 7.9 | 3.4/1.5/0.9 | 32 | 830 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_tbr_water_basin | yes | 728 | 3.8/6.2 | 4.8/6.7 | 3.4 | 2.1/1.2/0.9 | 5.7 | 2.6/1.1/0.7 | 31 | 588 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_tip_octal | yes | 1356 | 4.4/7.3 | 6.0/8.5 | 4.7 | 2.7/1.5/1.1 | 9.0 | 3.1/1.3/0.8 | 40 | 990 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_toondorf | yes | 921 | 4.2/7.0 | 6.8/8.8 | 4.6 | 2.3/1.3/0.9 | 6.7 | 2.8/1.2/0.8 | 42 | 549 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_towerwars_v2 | yes | 674 | 3.4/5.6 | 5.3/6.5 | 3.1 | 1.9/1.1/0.7 | 6.2 | 2.2/1.0/0.6 | 32 | 395 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_usp_deagle | yes | 1268 | 4.5/7.9 | 8.0/9.3 | 4.9 | 2.6/1.4/1.1 | 7.9 | 3.3/1.4/0.9 | 23 | 728 | 0 | 0 | yes | yes | yes |  |
| cs_source | gg_wolfenstein_3d | yes | 1058 | 3.7/6.5 | 5.1/6.5 | 3.1 | 2.2/1.3/0.8 | 8.9 | 2.4/1.0/0.7 | 24 | 612 | 0 | 0 | yes | yes | yes |  |
| cs_source | kz_11342 | yes | 38 | 0.6/1.0 | 0.6/0.6 | 0.6 | 0.0/0.0/0.0 | 3.3 | 0.2/0.1/0.1 | 50 | 25 | 0 | 0 | - | - | - | course/minigame map (kz_*); tiny region (spawn room only?); 50 of 50 spawns off the region |
| cs_source | kz_ancient_ruins | yes | 2054 | 4.9/8.6 | 0.7/1.3 | 1.9 | 2.8/1.8/1.3 | 14.1 | 3.0/1.3/0.8 | 5 | 870 | 0 | 0 | - | - | - | course/minigame map (kz_*) |
| cs_source | kz_bhop_izanami | yes | 8232 | 16.8/38.3 | 0.0/0.0 | 32.7 | 7.9/4.1/3.0 | 20.7 | 8.2/3.5/2.2 | 2 | 4244 | 0 | 0 | - | - | - | course/minigame map (kz_*) |
| cs_source | kz_bhop_sakura | yes | 158 | 1.3/3.1 | 0.0/0.6 | 0.6 | 0.6/0.4/0.0 | 6.2 | 0.5/0.2/0.1 | 6 | 87 | 0 | 0 | - | - | - | course/minigame map (kz_*); tiny region (spawn room only?) |
| cs_source | kz_bhop_skodna | yes | 1299 | 5.5/13.3 | 0.0/0.0 | 3.0 | 2.5/1.3/1.2 | 29.1 | 0.9/0.4/0.3 | 7 | 743 | 0 | 0 | - | - | - | course/minigame map (kz_*) |
| cs_source | kz_hikari_od_nh_v2 | yes | 4380 | 7.3/15.2 | 0.6/0.9 | 12.8 | 4.0/2.5/1.9 | 25.0 | 3.6/1.5/1.0 | 10 | 3228 | 0 | 0 | - | - | - | course/minigame map (kz_*) |
| cs_source | kz_rockb1ock | yes | 283 | 1.6/3.2 | 1.3/2.0 | 1.3 | 0.9/0.6/0.4 | 7.5 | 0.8/0.3/0.2 | 32 | 187 | 0 | 0 | - | - | - | course/minigame map (kz_*) |
| cs_source | mg_3k_smash_lego_copter | yes | 1993 | 4.8/11.0 | 6.3/8.1 | 5.9 | 3.4/1.9/1.3 | 43.4 | 0.9/0.4/0.3 | 40 | 1161 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_boatrace_scramble | yes | 900 | 3.1/6.4 | 0.6/1.2 | 0.9 | 1.8/1.1/0.8 | 23.7 | 0.8/0.3/0.2 | 20 | 706 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_crazykart_v1_1 | yes | 1940 | 4.4/7.6 | 1.2/2.0 | 3.4 | 2.5/1.4/1.3 | 20.3 | 2.0/0.8/0.5 | 32 | 1057 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_creative_multigames_v8_ns | yes | 1476 | 3.7/6.2 | 4.7/6.2 | 2.9 | 2.1/1.3/1.1 | 16.1 | 1.9/0.8/0.5 | 64 | 866 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_escape_prison_beta | yes | 798 | 2.5/5.0 | 1.9/3.8 | 1.3 | 1.9/1.3/0.6 | 13.9 | 1.2/0.5/0.3 | 64 | 441 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_item_battle_v4b | yes | 13000 | 11.6/19.4 | 17.8/23.6 | 11.6 | 6.8/4.4/3.5 | 23.3 | 11.5/4.9/3.1 | 32 | 8420 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_jacks_multigames_v1 | yes | 612 | 1.9/3.1 | 1.3/2.5 | 1.3 | 1.3/0.6/0.6 | 8.7 | 1.4/0.6/0.4 | 55 | 356 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_kommando | yes | 2832 | 3.9/6.6 | 2.1/3.9 | 1.9 | 3.1/2.0/1.6 | 33.8 | 1.7/0.7/0.5 | 32 | 2079 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_lego_course | yes | 108 | 0.8/1.3 | 0.6/1.3 | 0.6 | 0.5/0.0/0.0 | 5.5 | 0.4/0.2/0.1 | 72 | 52 | 0 | 0 | - | - | - | course/minigame map (mg_*); tiny region (spawn room only?) |
| cs_source | mg_lego_multigames_v2 | yes | 2542 | 5.0/8.7 | 6.3/8.7 | 4.6 | 3.1/1.9/1.3 | 22.4 | 2.3/1.0/0.6 | 65 | 1410 | 0 | 0 | - | - | - | course/minigame map (mg_*); 1 of 65 spawns off the region |
| cs_source | mg_lt_galaxy_v5 | yes | 1098 | 3.2/5.4 | 2.8/4.1 | 2.1 | 1.8/1.2/0.9 | 20.3 | 1.1/0.5/0.3 | 64 | 1019 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_n64_goldeneye_v2 | yes | 245 | 1.1/1.7 | 1.1/1.7 | 0.6 | 0.7/0.6/0.4 | 5.8 | 0.9/0.4/0.2 | 40 | 127 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_randomizer_v5 | yes | 122 | 1.1/1.9 | 0.6/1.3 | 0.6 | 0.6/0.0/0.0 | 5.7 | 0.4/0.2/0.1 | 50 | 70 | 0 | 0 | - | - | - | course/minigame map (mg_*); tiny region (spawn room only?) |
| cs_source | mg_starwars_v1 | yes | 11994 | 16.4/36.6 | 37.1/39.5 | 20.8 | 7.5/4.3/3.4 | 20.8 | 11.9/5.1/3.2 | 32 | 8182 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_swag_multigames_v1 | yes | 186 | 1.3/2.5 | 1.3/2.5 | 1.3 | 0.6/0.6/0.0 | 7.1 | 0.5/0.2/0.1 | 50 | 106 | 0 | 0 | - | - | - | course/minigame map (mg_*); tiny region (spawn room only?) |
| cs_source | mg_wipeout | yes | 249 | 1.7/3.5 | 0.6/1.3 | 1.9 | 0.9/0.6/0.4 | 21.8 | 0.2/0.1/0.1 | 36 | 182 | 0 | 0 | - | - | - | course/minigame map (mg_*) |
| cs_source | mg_wipeout2 | yes | 54 | 0.6/1.4 | 0.6/1.4 | 0.6 | 0.0/0.0/0.0 | 3.9 | 0.3/0.1/0.1 | 32 | 38 | 0 | 0 | - | - | - | course/minigame map (mg_*); tiny region (spawn room only?) |
| cs_source | surf_apollo | yes | 1161 | 3.8/7.7 | 0.7/1.5 | 3.8 | 1.9/1.2/0.9 | 12.5 | 1.9/0.8/0.5 | 64 | 756 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_boreas | yes | 488 | 2.4/4.3 | 1.9/3.8 | 1.9 | 1.3/0.6/0.6 | 10.5 | 1.0/0.4/0.3 | 162 | 276 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_botanica | yes | 3316 | 7.6/12.2 | 0.6/0.6 | 4.1 | 4.0/2.3/1.7 | 32.0 | 2.1/0.9/0.6 | 64 | 1867 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_demise | yes | 74817 | 31.0/65.0 | 0.6/1.3 | 52.1 | 16.7/10.9/8.1 | 33.0 | 46.7/20.0/12.7 | 64 | 54970 | 0 | 0 | - | - | - | course/minigame map (surf_*); area over 15000 m2 (a leak or open terrain) |
| cs_source | surf_halloween_tf2 | yes | 928 | 2.6/4.7 | 0.6/1.3 | 3.3 | 2.0/1.2/0.9 | 7.4 | 2.6/1.1/0.7 | 26 | 511 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_happyhands | yes | 637 | 2.6/4.3 | 1.2/2.2 | 1.3 | 1.4/0.9/0.6 | 12.8 | 1.0/0.4/0.3 | 60 | 437 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_hellenic | yes | 7154 | 15.6/27.7 | 0.6/1.3 | 4.4 | 7.1/3.9/2.9 | 47.2 | 3.1/1.3/0.9 | 64 | 6472 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_holiday | yes | 417 | 2.1/3.9 | 0.7/1.7 | 1.4 | 1.2/0.6/0.6 | 8.9 | 1.0/0.4/0.3 | 77 | 231 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_inferno | yes | 658 | 1.9/4.3 | 1.3/1.9 | 0.6 | 1.3/0.6/0.6 | 19.4 | 0.7/0.3/0.2 | 78 | 382 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_jive | yes | 245 | 1.5/3.1 | 0.0/0.0 | 0.4 | 0.8/0.5/0.4 | 13.5 | 0.4/0.2/0.1 | 64 | 166 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_kismet | yes | 68 | 0.6/1.3 | 0.6/1.3 | 0.6 | 0.0/0.0/0.0 | 7.0 | 0.2/0.1/0.1 | 100 | 42 | 0 | 0 | - | - | - | course/minigame map (surf_*); tiny region (spawn room only?) |
| cs_source | surf_nebula | yes | 1833 | 6.3/11.6 | 1.0/1.8 | 4.1 | 3.5/1.8/1.3 | 29.1 | 1.3/0.6/0.4 | 78 | 1351 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_nsz_fix | yes | 1976 | 4.2/7.0 | 0.6/0.8 | 2.6 | 2.5/1.5/1.3 | 39.2 | 1.0/0.4/0.3 | 61 | 0 | 61 | 0 | - | - | - | course/minigame map (surf_*); 61 of 61 spawns off the region; 61 of 0 spawn spots off the map; no spawn spots: the spawn points only |
| cs_source | surf_sacrifice | yes | 5512 | 7.0/11.7 | 5.5/9.4 | 4.3 | 4.1/2.7/2.1 | 29.6 | 3.8/1.6/1.0 | 60 | 5278 | 0 | 0 | - | - | - | course/minigame map (surf_*); 23 of 60 spawns off the region |
| cs_source | surf_sedona | yes | 17188 | 18.3/58.0 | 2.9/5.1 | 5.6 | 8.4/5.4/4.1 | 22.9 | 15.5/6.6/4.2 | 64 | 15285 | 0 | 0 | - | - | - | course/minigame map (surf_*); area over 15000 m2 (a leak or open terrain) |
| cs_source | surf_slob | yes | 317 | 1.9/3.1 | 0.6/1.3 | 0.6 | 1.0/0.6/0.5 | 12.2 | 0.5/0.2/0.1 | 64 | 195 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_stickybutt_alpha | yes | 886 | 4.0/7.7 | 2.6/5.2 | 2.5 | 2.2/1.5/1.1 | 28.7 | 0.6/0.3/0.2 | 34 | 1291 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_surreal | yes | 597 | 3.2/7.3 | 0.7/1.2 | 5.2 | 1.5/0.9/0.6 | 7.7 | 1.6/0.7/0.4 | 62 | 426 | 0 | 0 | - | - | - | course/minigame map (surf_*) |
| cs_source | surf_threnody | yes | 3624 | 12.4/27.7 | 0.0/0.0 | 18.3 | 5.0/2.5/1.9 | 28.7 | 2.6/1.1/0.7 | 64 | 0 | 64 | 0 | - | - | - | course/minigame map (surf_*); 64 of 64 spawns off the region; 64 of 0 spawn spots off the map; no spawn spots: the spawn points only |
| cs_source | test_hardware | no | 0 | 0.0/0.0 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0 | 0 | 0 | 0 | - | - | - | course/minigame map (test_*); no spawn points |
| cs_source | test_speakers | no | 0 | 0.0/0.0 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0 | 0 | 0 | 0 | - | - | - | course/minigame map (test_*); no spawn points |
| combat_arms | 16rooms | yes | 11441 | 16.1/26.7 | 16.2/26.2 | 11.3 | 10.1/6.0/4.5 | 7.3 | 32.1/13.8/8.8 | 48 | 11545 | 0 | 0 | - | - | yes |  |
| combat_arms | battleship | yes | 2421 | 6.8/14.1 | 2.6/2.9 | 8.8 | 3.7/2.3/1.7 | 10.6 | 4.7/2.0/1.3 | 9 | 1677 | 0 | 0 | yes | yes | yes |  |
| combat_arms | battleship1 | yes | 2290 | 6.3/12.9 | 2.6/2.9 | 7.5 | 3.4/2.2/1.6 | 10.2 | 4.6/2.0/1.3 | 9 | 1588 | 0 | 0 | yes | yes | yes |  |
| combat_arms | battleship2 | yes | 2620 | 7.4/15.7 | 15.3/17.0 | 8.8 | 3.9/2.4/1.8 | 10.3 | 5.2/2.2/1.4 | 24 | 1777 | 0 | 0 | yes | yes | yes |  |
| combat_arms | blacklung | yes | 1175 | 6.0/10.8 | 0.6/0.6 | 3.9 | 3.1/1.6/1.2 | 6.6 | 3.6/1.6/1.0 | 8 | 110 | 0 | 0 | yes | yes | yes |  |
| combat_arms | bloodmoney | yes | 5530 | 11.2/17.7 | 14.5/15.1 | 7.3 | 6.2/3.4/2.5 | 11.8 | 9.7/4.1/2.6 | 16 | 3634 | 0 | 0 | yes | yes | yes |  |
| combat_arms | bloodoffice | yes | 470 | 3.7/6.5 | 5.2/6.8 | 4.8 | 1.8/0.9/0.6 | 5.7 | 1.7/0.7/0.5 | 24 | 236 | 0 | 0 | yes | yes | yes |  |
| combat_arms | bloodycanyon | yes | 1256 | 4.0/6.7 | 4.8/6.5 | 2.9 | 2.3/1.4/1.1 | 9.1 | 2.9/1.2/0.8 | 24 | 854 | 0 | 0 | yes | yes | yes |  |
| combat_arms | bossbattle3_cp | yes | 2021 | 4.4/8.2 | 0.6/1.3 | 4.4 | 2.5/1.8/1.3 | 21.5 | 1.9/0.8/0.5 | 8 | 1151 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | bossbattle4_cp | yes | 2021 | 4.4/8.2 | 0.6/1.3 | 4.4 | 2.5/1.8/1.3 | 21.5 | 1.9/0.8/0.5 | 8 | 1151 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | bossbattle_cp | yes | 28 | 0.0/0.3 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 2.3 | 0.2/0.1/0.1 | 8 | 14 | 0 | 0 | - | - | - | mode variant / not a standard map; tiny region (spawn room only?) |
| combat_arms | bossrush | yes | 1868 | 4.4/7.6 | 0.6/0.6 | 6.7 | 2.5/1.6/1.3 | 20.6 | 1.9/0.8/0.5 | 8 | 1064 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | bossship | yes | 2706 | 6.5/13.2 | 2.6/2.9 | 8.8 | 3.6/2.3/1.7 | 17.2 | 3.2/1.4/0.9 | 9 | 1765 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | brushwood | yes | 2690 | 6.9/12.8 | 16.6/18.2 | 9.7 | 3.9/2.5/1.8 | 10.1 | 5.5/2.4/1.5 | 24 | 2285 | 0 | 0 | yes | yes | yes |  |
| combat_arms | cabinfever | yes | 4830 | 7.4/12.7 | 0.4/0.9 | 1.5 | 4.3/2.8/2.1 | 20.5 | 4.9/2.1/1.3 | 8 | 137 | 0 | 0 | yes | yes | yes |  |
| combat_arms | cargoship | yes | 2277 | 5.3/11.1 | 1.1/2.3 | 11.6 | 3.6/2.2/1.7 | 9.0 | 5.2/2.2/1.4 | 24 | 1239 | 0 | 0 | yes | yes | yes |  |
| combat_arms | castle | yes | 3306 | 11.6/19.4 | 24.4/25.1 | 18.7 | 7.3/4.5/3.3 | 11.5 | 5.9/2.5/1.6 | 24 | 2553 | 0 | 0 | yes | yes | yes |  |
| combat_arms | cityofwings | yes | 5528 | 16.8/31.0 | 23.0/29.6 | 8.8 | 7.5/3.9/2.9 | 9.6 | 11.9/5.1/3.2 | 23 | 3111 | 0 | 0 | - | yes | yes |  |
| combat_arms | coldseed | yes | 1996 | 7.0/12.2 | 12.9/14.0 | 6.9 | 3.9/2.5/1.8 | 7.3 | 5.6/2.4/1.5 | 24 | 1442 | 0 | 0 | yes | yes | yes |  |
| combat_arms | costarecon | yes | 6260 | 8.4/14.1 | 15.8/17.1 | 8.2 | 5.0/3.2/2.5 | 14.9 | 8.7/3.7/2.4 | 23 | 4636 | 0 | 0 | yes | yes | yes |  |
| combat_arms | costarecon_cp | yes | 1413 | 3.7/6.5 | 0.5/0.6 | 2.5 | 2.1/1.4/1.1 | 14.0 | 2.1/0.9/0.6 | 12 | 939 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | costarecon_q | yes | 6260 | 8.4/14.1 | 15.8/17.1 | 8.2 | 5.0/3.2/2.5 | 14.9 | 8.7/3.7/2.4 | 23 | 4635 | 0 | 0 | - | - | - | quarantine variant of costarecon |
| combat_arms | crystaltower | yes | 1714 | 5.4/11.5 | 0.6/1.0 | 4.4 | 3.8/2.3/1.6 | 8.1 | 4.4/1.9/1.2 | 24 | 1058 | 0 | 0 | yes | yes | yes |  |
| combat_arms | dam | yes | 1573 | 8.7/19.0 | 17.5/18.8 | 9.3 | 4.0/2.1/1.4 | 11.1 | 2.9/1.3/0.8 | 24 | 885 | 0 | 0 | yes | yes | yes |  |
| combat_arms | darkforest | yes | 8252 | 8.9/15.5 | 18.4/22.2 | 11.0 | 5.3/3.4/2.6 | 28.4 | 6.0/2.6/1.6 | 23 | 7266 | 0 | 0 | yes | yes | yes |  |
| combat_arms | darkhills | yes | 348 | 1.7/3.0 | 0.7/1.7 | 0.6 | 1.1/0.6/0.5 | 7.9 | 0.9/0.4/0.2 | 16 | 234 | 0 | 0 | yes | yes | yes |  |
| combat_arms | darkthandark | yes | 23651 | 15.3/27.6 | 3.8/5.2 | 2.6 | 8.5/5.6/4.4 | 49.3 | 9.9/4.2/2.7 | 24 | 16234 | 0 | 0 | - | - | - | area over 15000 m2 (a leak or open terrain) |
| combat_arms | deathroom | yes | 1199 | 7.2/15.6 | 11.1/15.5 | 7.5 | 2.6/1.4/1.0 | 7.2 | 3.4/1.5/0.9 | 24 | 711 | 0 | 0 | yes | yes | yes |  |
| combat_arms | deathroom_cp | yes | 1204 | 7.1/15.5 | 0.0/0.0 | 6.3 | 2.8/1.5/1.1 | 7.2 | 3.5/1.5/0.9 | 8 | 704 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | desertfox_cp | yes | 5528 | 12.9/23.2 | 0.6/0.7 | 19.3 | 7.5/4.4/3.1 | 8.0 | 14.2/6.1/3.9 | 8 | 3424 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | deserthole | yes | 19710 | 13.1/22.7 | 35.2/38.6 | 18.9 | 7.6/4.9/3.8 | 23.5 | 17.3/7.4/4.7 | 24 | 14886 | 0 | 0 | - | - | - | area over 15000 m2 (a leak or open terrain) |
| combat_arms | desertholezone | yes | 19713 | 13.1/22.6 | 26.3/27.8 | 15.5 | 7.6/4.9/3.8 | 23.6 | 17.2/7.4/4.7 | 24 | 14915 | 0 | 0 | - | - | - | area over 15000 m2 (a leak or open terrain) |
| combat_arms | downhill | yes | 5342 | 15.7/22.8 | 15.9/17.8 | 18.4 | 6.2/3.5/2.6 | 18.0 | 6.1/2.6/1.7 | 24 | 4290 | 0 | 0 | yes | yes | yes |  |
| combat_arms | dredge | yes | 1440 | 7.0/16.6 | 1.9/3.8 | 7.5 | 3.7/1.9/1.3 | 7.3 | 4.1/1.8/1.1 | 24 | 981 | 0 | 0 | yes | yes | yes |  |
| combat_arms | freezer | yes | 1978 | 7.4/12.1 | 12.2/14.3 | 5.3 | 3.9/2.4/1.8 | 7.9 | 5.2/2.2/1.4 | 24 | 1816 | 0 | 0 | yes | yes | yes |  |
| combat_arms | ghosttown | yes | 5343 | 9.6/18.1 | 17.6/20.3 | 11.5 | 6.0/3.6/2.6 | 11.1 | 9.9/4.2/2.7 | 24 | 3655 | 0 | 0 | yes | yes | yes |  |
| combat_arms | grandcanyon | yes | 3421 | 11.2/20.0 | 21.1/24.8 | 12.5 | 6.5/3.7/2.5 | 7.7 | 9.1/3.9/2.5 | 24 | 2430 | 0 | 0 | yes | yes | yes |  |
| combat_arms | gravedigger | yes | 1310 | 8.2/16.5 | 13.5/15.2 | 9.7 | 4.0/1.9/1.3 | 5.3 | 5.1/2.2/1.4 | 23 | 799 | 0 | 0 | yes | yes | yes |  |
| combat_arms | graveyard | yes | 4103 | 7.9/13.5 | 12.7/15.8 | 7.4 | 4.2/2.5/1.9 | 12.6 | 6.7/2.9/1.8 | 16 | 3006 | 0 | 0 | yes | yes | yes |  |
| combat_arms | gravitya | yes | 1915 | 8.7/17.0 | 16.3/18.4 | 9.6 | 5.1/3.0/2.1 | 4.5 | 8.8/3.8/2.4 | 24 | 1144 | 0 | 0 | yes | yes | yes |  |
| combat_arms | gravityb | yes | 946 | 2.5/4.6 | 1.3/3.3 | 1.8 | 1.9/1.2/0.9 | 7.7 | 2.5/1.1/0.7 | 24 | 536 | 0 | 0 | yes | yes | yes |  |
| combat_arms | grayhammer | yes | 5477 | 16.2/29.7 | 27.3/30.3 | 14.5 | 8.6/4.4/3.2 | 7.5 | 15.0/6.4/4.1 | 24 | 3440 | 0 | 0 | - | yes | yes |  |
| combat_arms | grayhammerq | yes | 5476 | 16.2/29.7 | 27.3/30.3 | 14.5 | 8.6/4.3/3.1 | 7.6 | 14.9/6.4/4.1 | 24 | 3426 | 0 | 0 | - | - | - | quarantine variant of grayhammer |
| combat_arms | hallowravine | yes | 1084 | 2.2/3.9 | 1.1/1.9 | 1.3 | 1.8/1.2/0.8 | 14.1 | 1.6/0.7/0.4 | 26 | 809 | 0 | 0 | yes | yes | yes |  |
| combat_arms | heavymetal | yes | 6029 | 10.7/21.3 | 0.4/1.1 | 16.6 | 6.3/4.1/3.1 | 9.2 | 13.5/5.8/3.7 | 24 | 3963 | 0 | 0 | - | yes | yes |  |
| combat_arms | hellgate | yes | 478 | 3.1/5.8 | 4.0/5.5 | 2.7 | 1.6/0.9/0.7 | 11.3 | 0.9/0.4/0.2 | 24 | 353 | 0 | 0 | yes | yes | yes |  |
| combat_arms | hgoilrig | no | -0 | 0.0/0.0 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 16 | 0 | 16 | 0 | - | - | - | no walkable region from the spawn points; 16 of 16 spawns off the region; 16 of 0 spawn spots off the map |
| combat_arms | hiredguns2 | yes | 1240 | 8.4/17.0 | 15.3/18.1 | 10.4 | 4.6/2.4/1.6 | 5.2 | 5.0/2.1/1.4 | 24 | 773 | 0 | 0 | yes | yes | yes |  |
| combat_arms | infectedship_cp | yes | 2669 | 6.8/10.3 | 4.0/5.5 | 5.1 | 4.1/2.5/1.8 | 17.4 | 3.2/1.4/0.9 | 8 | 510 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | istanbul | yes | 2681 | 9.0/15.6 | 20.9/22.2 | 11.3 | 4.6/2.6/1.9 | 7.3 | 7.6/3.2/2.1 | 24 | 1601 | 0 | 0 | yes | yes | yes |  |
| combat_arms | istanbulq | yes | 2681 | 9.0/15.6 | 20.9/22.2 | 11.3 | 4.6/2.6/1.9 | 7.3 | 7.6/3.2/2.1 | 24 | 1601 | 0 | 0 | - | - | - | quarantine variant of istanbul |
| combat_arms | jscssnowstorm | yes | 3504 | 6.7/11.2 | 3.7/5.1 | 3.6 | 3.7/2.2/1.7 | 13.6 | 5.3/2.3/1.4 | 20 | 2078 | 0 | 0 | yes | yes | yes |  |
| combat_arms | junk_flea | yes | 1343 | 5.5/10.6 | 8.8/10.2 | 5.2 | 2.9/1.9/1.4 | 6.7 | 4.1/1.8/1.1 | 24 | 894 | 0 | 0 | yes | yes | yes |  |
| combat_arms | junk_flea2 | yes | 1698 | 5.7/10.3 | 10.1/12.0 | 5.8 | 3.4/2.0/1.6 | 6.2 | 5.7/2.4/1.5 | 24 | 1226 | 0 | 0 | yes | yes | yes |  |
| combat_arms | killland | yes | 1733 | 3.1/5.7 | 0.6/1.3 | 3.8 | 2.5/1.9/1.3 | 12.7 | 2.8/1.2/0.8 | 23 | 442 | 0 | 0 | yes | yes | yes |  |
| combat_arms | lastvegas | yes | 1665 | 4.1/7.2 | 0.8/9.2 | 5.5 | 3.3/2.1/1.5 | 7.5 | 4.6/2.0/1.3 | 24 | 1150 | 0 | 0 | yes | yes | yes |  |
| combat_arms | mousetrap | yes | 988 | 5.6/11.6 | 12.2/14.3 | 7.0 | 2.6/1.6/1.2 | 5.9 | 3.5/1.5/0.9 | 24 | 740 | 0 | 0 | yes | yes | yes |  |
| combat_arms | nemexishq | yes | 2152 | 5.8/11.9 | 0.0/0.5 | 9.5 | 2.9/1.7/1.3 | 14.4 | 3.1/1.3/0.8 | 8 | 0 | 0 | 0 | yes | yes | yes | no spawn spots: the spawn points only |
| combat_arms | nemexislab_cp | yes | 782 | 4.2/7.8 | 0.5/0.6 | 5.0 | 2.1/1.1/0.8 | 6.8 | 2.4/1.0/0.6 | 8 | 432 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | nemexiswar | yes | 10094 | 16.7/28.8 | 25.5/28.4 | 15.0 | 9.2/4.8/3.5 | 15.4 | 13.6/5.8/3.7 | 24 | 6873 | 0 | 0 | - | yes | yes |  |
| combat_arms | nemexiswarq | yes | 10131 | 16.7/28.7 | 25.7/28.3 | 16.3 | 9.2/5.0/3.6 | 15.1 | 13.8/5.9/3.8 | 24 | 6892 | 0 | 0 | - | - | - | quarantine variant of nemexiswar |
| combat_arms | neptune | yes | 2362 | 6.8/10.1 | 3.6/4.5 | 4.7 | 5.5/3.4/2.3 | 7.2 | 6.8/2.9/1.9 | 24 | 1066 | 0 | 0 | yes | yes | yes |  |
| combat_arms | noxiousdamnation | yes | 887 | 2.9/4.9 | 2.2/4.1 | 2.0 | 1.7/1.1/0.7 | 14.9 | 1.2/0.5/0.3 | 50 | 980 | 0 | 0 | yes | yes | yes |  |
| combat_arms | nwar | yes | 10193 | 16.5/28.3 | 25.7/27.5 | 15.1 | 8.5/4.5/3.4 | 15.5 | 13.5/5.8/3.7 | 24 | 6859 | 0 | 0 | - | yes | yes |  |
| combat_arms | oilrig | yes | 3159 | 10.1/20.0 | 10.7/12.4 | 7.3 | 4.6/2.6/1.9 | 13.2 | 4.9/2.1/1.3 | 24 | 2239 | 0 | 0 | yes | yes | yes |  |
| combat_arms | oilrigq | yes | 3160 | 10.2/19.7 | 10.7/12.7 | 7.4 | 5.0/2.8/2.1 | 12.8 | 5.1/2.2/1.4 | 24 | 2259 | 0 | 0 | - | - | - | quarantine variant of oilrig |
| combat_arms | overdose | yes | 1303 | 9.0/20.4 | 1.0/17.4 | 1.0 | 4.3/2.3/1.6 | 6.5 | 4.1/1.8/1.1 | 24 | 769 | 0 | 0 | yes | yes | yes |  |
| combat_arms | piazza | yes | 3282 | 11.4/18.4 | 19.6/20.9 | 15.3 | 6.6/3.7/2.6 | 6.5 | 10.4/4.5/2.8 | 24 | 2019 | 0 | 0 | - | yes | yes |  |
| combat_arms | piazzaq | yes | 3282 | 11.4/18.4 | 19.6/20.9 | 15.3 | 6.6/3.7/2.6 | 6.5 | 10.4/4.5/2.8 | 24 | 2019 | 0 | 0 | - | - | - | quarantine variant of piazza |
| combat_arms | pumpjack | yes | 2476 | 8.7/15.3 | 18.1/19.4 | 9.8 | 4.6/2.9/2.1 | 6.7 | 7.6/3.3/2.1 | 24 | 1481 | 0 | 0 | yes | yes | yes |  |
| combat_arms | railwaydefence | yes | 2404 | 9.0/19.1 | 0.6/1.2 | 11.9 | 3.4/2.0/1.6 | 12.2 | 4.1/1.7/1.1 | 8 | 886 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | rapidstream | yes | 2828 | 8.1/13.8 | 17.7/18.3 | 9.3 | 4.7/2.7/2.1 | 8.6 | 6.8/2.9/1.9 | 24 | 1977 | 0 | 0 | yes | yes | yes |  |
| combat_arms | rattlesnake | yes | 3504 | 12.2/26.3 | 26.8/27.9 | 20.9 | 5.8/2.9/2.2 | 9.4 | 7.7/3.3/2.1 | 24 | 2333 | 0 | 0 | yes | yes | yes |  |
| combat_arms | roadkill | yes | 3564 | 9.7/20.3 | 24.0/25.5 | 13.2 | 4.5/2.6/2.0 | 12.4 | 5.9/2.5/1.6 | 24 | 3602 | 0 | 0 | yes | yes | yes |  |
| combat_arms | roadkill2 | yes | 3588 | 9.4/19.9 | 23.5/25.2 | 13.2 | 4.5/2.6/2.1 | 12.7 | 5.8/2.5/1.6 | 24 | 3620 | 0 | 0 | yes | yes | yes |  |
| combat_arms | ruralestate | yes | 5361 | 12.3/20.4 | 11.5/17.3 | 9.5 | 7.2/3.8/2.8 | 11.3 | 9.7/4.2/2.7 | 24 | 3700 | 0 | 0 | yes | yes | yes |  |
| combat_arms | sandhog | yes | 2694 | 7.1/12.5 | 15.5/17.6 | 9.1 | 3.9/2.4/1.7 | 9.3 | 6.0/2.6/1.6 | 24 | 1880 | 0 | 0 | yes | yes | yes |  |
| combat_arms | sandhog_cp1 | yes | 1137 | 5.0/10.9 | 0.4/0.7 | 9.6 | 2.5/1.3/1.0 | 9.4 | 2.5/1.1/0.7 | 8 | 716 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | sector25 | yes | 1824 | 8.2/16.9 | 17.3/19.0 | 10.5 | 4.0/2.2/1.6 | 7.7 | 4.9/2.1/1.3 | 24 | 1258 | 0 | 0 | yes | yes | yes |  |
| combat_arms | shortfuse | yes | 3239 | 11.3/21.5 | 17.5/19.7 | 13.4 | 6.0/3.2/2.1 | 8.4 | 8.0/3.4/2.2 | 24 | 2146 | 0 | 0 | yes | yes | yes |  |
| combat_arms | shortfuse2 | yes | 3614 | 10.9/19.1 | 17.2/19.6 | 12.4 | 6.2/3.4/2.3 | 7.8 | 9.5/4.1/2.6 | 24 | 2296 | 0 | 0 | yes | yes | yes |  |
| combat_arms | slaughterhouse | yes | 2519 | 9.7/17.4 | 14.4/16.6 | 8.3 | 4.7/2.6/1.8 | 7.4 | 7.0/3.0/1.9 | 18 | 1650 | 0 | 0 | yes | yes | yes |  |
| combat_arms | snowrobot | yes | 7982 | 12.8/22.1 | 13.2/19.2 | 11.3 | 7.2/4.2/3.2 | 13.7 | 12.0/5.1/3.3 | 24 | 6576 | 0 | 0 | - | yes | yes |  |
| combat_arms | snowstorm | yes | 3344 | 6.9/11.5 | 0.8/1.3 | 3.7 | 3.7/2.2/1.7 | 14.3 | 4.8/2.1/1.3 | 8 | 1998 | 0 | 0 | yes | yes | yes |  |
| combat_arms | snowstorm_qrt | yes | 3700 | 7.1/11.9 | 1.4/2.3 | 4.3 | 3.9/2.4/1.9 | 13.4 | 5.7/2.4/1.6 | 32 | 2317 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | snowvalley | yes | 7992 | 12.9/21.9 | 13.2/19.3 | 11.7 | 7.5/4.4/3.2 | 13.5 | 12.2/5.2/3.3 | 24 | 6561 | 0 | 0 | - | yes | yes |  |
| combat_arms | snowvalleyq | yes | 7984 | 12.8/22.2 | 13.2/19.2 | 11.3 | 7.3/4.2/3.2 | 13.9 | 11.8/5.1/3.2 | 24 | 6564 | 0 | 0 | - | - | - | quarantine variant of snowvalley |
| combat_arms | square | yes | 2910 | 8.5/15.7 | 23.3/25.1 | 12.4 | 4.8/3.0/2.2 | 8.4 | 7.1/3.0/1.9 | 24 | 2528 | 0 | 0 | yes | yes | yes |  |
| combat_arms | terrordome | yes | 3760 | 11.2/17.9 | 12.5/17.3 | 8.7 | 6.1/3.7/2.5 | 8.0 | 9.6/4.1/2.6 | 28 | 2275 | 0 | 0 | yes | yes | yes |  |
| combat_arms | testing_t | yes | 2364 | 8.3/16.5 | 10.4/12.6 | 7.1 | 4.4/2.4/1.9 | 7.7 | 6.3/2.7/1.7 | 24 | 1483 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | towerbridge | yes | 1811 | 6.9/15.3 | 13.7/17.8 | 8.2 | 3.4/2.1/1.6 | 9.2 | 4.0/1.7/1.1 | 24 | 1305 | 0 | 0 | yes | yes | yes |  |
| combat_arms | towerdefence | yes | 161 | 1.8/3.1 | 0.6/1.4 | 1.5 | 0.8/0.4/0.3 | 4.2 | 0.8/0.3/0.2 | 8 | 330 | 0 | 0 | - | - | - | mode variant / not a standard map; tiny region (spawn room only?) |
| combat_arms | training | yes | 720 | 2.5/4.2 | nan/nan | 1.6 | 1.4/0.9/0.7 | 13.3 | 1.1/0.5/0.3 | 1 | 544 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | twinhigh | yes | 460 | 1.6/3.0 | 0.6/1.3 | 1.3 | 1.3/0.6/0.6 | 47.2 | 0.2/0.1/0.1 | 24 | 251 | 0 | 0 | yes | yes | yes |  |
| combat_arms | twinship | yes | 2277 | 5.3/11.1 | 1.1/2.3 | 11.6 | 3.6/2.2/1.7 | 9.0 | 5.2/2.2/1.4 | 24 | 1239 | 0 | 0 | yes | yes | yes |  |
| combat_arms | twintower | yes | 744 | 3.8/8.9 | 0.6/0.6 | 6.0 | 2.5/1.3/0.8 | 23.8 | 0.6/0.3/0.2 | 26 | 443 | 0 | 0 | yes | yes | yes |  |
| combat_arms | twotowers | yes | 2051 | 7.5/16.4 | 2.3/6.4 | 11.6 | 4.2/2.3/1.6 | 10.9 | 3.9/1.7/1.1 | 24 | 1359 | 0 | 0 | yes | yes | yes |  |
| combat_arms | vertigo | yes | 1359 | 5.4/9.9 | 10.0/10.8 | 5.4 | 2.9/1.8/1.3 | 10.2 | 2.7/1.2/0.7 | 16 | 791 | 0 | 0 | yes | yes | yes |  |
| combat_arms | vertigo_q | yes | 1356 | 5.3/9.8 | 10.0/10.8 | 5.4 | 2.8/1.8/1.3 | 10.4 | 2.7/1.2/0.7 | 16 | 791 | 0 | 0 | - | - | - | quarantine variant of vertigo |
| combat_arms | warehouse | yes | 1820 | 6.6/11.4 | 14.5/14.9 | 7.5 | 4.0/2.4/1.7 | 8.1 | 4.6/2.0/1.3 | 24 | 1270 | 0 | 0 | yes | yes | yes |  |
| combat_arms | warehouse_qrt | yes | 1817 | 6.7/11.5 | 6.5/10.3 | 5.0 | 4.0/2.5/1.8 | 8.2 | 4.6/2.0/1.2 | 20 | 1288 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | warhead | yes | 2940 | 12.2/26.5 | 19.6/21.0 | 15.0 | 5.5/3.0/2.1 | 7.4 | 8.2/3.5/2.2 | 24 | 1481 | 0 | 0 | yes | yes | yes |  |
| combat_arms | waverider | yes | 4192 | 13.7/24.0 | 23.3/24.9 | 13.3 | 7.6/4.4/3.1 | 8.5 | 10.1/4.3/2.8 | 24 | 3370 | 0 | 0 | - | yes | yes |  |
| combat_arms | zombieroom | yes | 299 | 1.2/1.9 | 0.0/0.4 | 0.6 | 1.0/0.6/0.6 | 6.2 | 1.0/0.4/0.3 | 24 | 160 | 0 | 0 | - | - | - | mode variant / not a standard map |
| combat_arms | zombiewar | yes | 2239 | 3.1/5.6 | 0.8/1.9 | 0.6 | 2.5/1.7/1.3 | 19.9 | 2.3/1.0/0.6 | 16 | 1234 | 0 | 0 | - | - | - | mode variant / not a standard map |
| hl2dm | dm_lockdown | yes | 3121 | 11.4/19.8 | 12.4/20.7 | 9.6 | 5.8/3.1/2.3 | 7.7 | 8.4/3.6/2.3 | 33 | 2230 | 0 | 0 | yes | yes | yes |  |
| hl2dm | dm_overwatch | yes | 3095 | 8.7/14.8 | 12.0/17.7 | 7.4 | 4.5/2.4/1.8 | 11.9 | 5.3/2.3/1.5 | 17 | 2437 | 0 | 0 | yes | yes | yes |  |
| hl2dm | dm_powerhouse | yes | 4122 | 11.2/17.8 | 12.5/17.8 | 9.5 | 6.0/3.6/2.6 | 7.1 | 12.0/5.2/3.3 | 18 | 2593 | 0 | 0 | - | yes | yes |  |
| hl2dm | dm_resistance | yes | 1902 | 6.6/10.8 | 7.2/11.4 | 6.1 | 3.4/1.9/1.4 | 8.9 | 4.4/1.9/1.2 | 20 | 1410 | 0 | 0 | yes | yes | yes |  |
| hl2dm | dm_runoff | yes | 6139 | 13.6/27.7 | 11.2/24.2 | 6.6 | 6.2/3.4/2.6 | 15.2 | 8.3/3.6/2.3 | 9 | 4719 | 0 | 0 | yes | yes | yes |  |
| hl2dm | dm_steamlab | yes | 2033 | 9.4/14.9 | 10.0/15.1 | 8.2 | 4.9/2.6/1.8 | 7.0 | 6.0/2.6/1.6 | 24 | 1328 | 0 | 0 | yes | yes | yes |  |
| hl2dm | dm_underpass | yes | 1980 | 9.3/16.1 | 11.1/17.9 | 9.3 | 4.7/2.7/1.9 | 6.9 | 5.9/2.5/1.6 | 17 | 1451 | 0 | 0 | yes | yes | yes |  |
| hl2dm | halls3 | no | 0 | 0.0/0.0 | 0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0.0 | 0.0/0.0/0.0 | 0 | 0 | 0 | 0 | - | - | - | not a dm_ map; maps/halls3.bsp: hl2dm: maps/halls3.bsp: record 123291a97597a53481da4097460439b380ae9219f371aec8d00d87634f6166b7: No such file or directory (os error 2); maps/halls3.bsp: hl2dm: maps/halls3.bsp: record 123291a97597; maps/halls3.bsp: hl2dm: maps/halls3.bsp: record 123291a97597 |
