# Arena slices: a big map played in a part cut to size

Started 2026-11-15 (design only; nothing built). Parent plans:
[map-pool-fitness.md](map-pool-fitness.md) (the pool by player count),
[ultimate-gun-game.md](ultimate-gun-game.md); the idea is the backlog
entry "An arena cut from a big map" in [halo-ce.md](halo-ce.md).

## Status

| Milestone | State | Summary |
|---|---|---|
| A1 one hand-drawn slice of Blood Gulch, plain wall | done 2026-11-15 | `halo_ce:bloodgulch@west_base` (9,900 m2): data, solid wall, spots and bots held inside, `NetMinigame::arena` (net64), `arena` command, placeholder look; see "A1 as built" |
| A2 slices in the draw | done 2026-11-16 | `map@id` pool entries judged from measures stored in the slice data, only where the whole map is too big; one per map on a ballot, held back by map; `changelevel map@id`; the Maps picker's children; see "A2 as built". No slice in the data passes yet |
| A3 measuring a slice | done 2026-11-16 | `map_fitness --slice` and `--safety`, `slice_safety`, `[slice]` in map_fit.toml calibrated on 13 maps, slices make their own spawn points; A1's leftovers done; see "A3 as built". West Base fails the measures |
| A4 spawn deaths from play | not started | the share of deaths within 3 s of a spawn, per match, to check A3's thresholds against play |
| A5 the candidate generator | done 2026-11-16 | `map_fitness --candidates <game:map>`, `--pick`, `--slice --remeasure`; Blood Gulch: one slice passes (North Caves, 2-8), Sidewinder none, darkthandark two at 2-4; see "A5 as built" |
| A6 curating Blood Gulch and Sidewinder | not started | a person picks two or three slices per player band from A5's sheets |
| A7 the look | not started | ground line, faint curtain, near glow, in the Signal look (the party UI owner finalises) |
| A8 the world beyond dimmed | not started | a screen pass darkening what lies outside |
| A9 other games' big maps | not started | Combat Arms' open maps, more if the owner wants (question 3) |

## 1. Goal and non-goals

Goal: the party apps' gun game (Ultimate Gun Game, Lucker Party) plays
the big maps it leaves out today by playing a part of one, an arena cut
from the map by a closed boundary and sized to the players present, in
which nobody spawns in plain view of the others. A slice is data: a
boundary drawn in plan view, chosen by a person from candidates a tool
proposes and scores, kept per game beside the pool rules. In play the
boundary is a wall to players, the bots and the spawns stay inside it,
and it looks like the party: a line on the ground and a faint curtain
from afar, a glow when you come close.

Non-goals: a boundary that shrinks or moves during a match (question 2);
slices built at run time from a live analysis (the tool runs offline, a
person picks); objectives, flags or vehicles inside a slice (vehicles are
off in the party apps); slices in a game's own app (Halo's app plays
Halo's maps whole); maps that are big because they leak (those get fixed,
not sliced).

## 2. What exists, and what does not

What the design stands on:

| What | Where | What the design takes from it |
|---|---|---|
| The pool by player count: per map area and mean sight (`Fitness`), wander time `area / ((N - 1) * 2 * sight * 4/pi * speed)`, in at most 10 s, weighted fully from 4 to 8 s; rules (`map_pool.toml`) and measurements (`map_fitness.toml`) per game into `MapPool`; the draw counts the characters present | `chimera_sim::minigame::map_fit`, chimera/sim/data/map_fit.toml, `rotation::MapHistory::next_weighted`, `vote::candidates_weighted` | a slice is one more pool entry with its own `Fitness` |
| The measuring tool: loads a map as the server does, floods the bots' mesh, measures area, sight (16 level rays, capped at 50 m), walks, meeting and wander times, spots; `--dump` writes the spots, floors and a top-down height map as CSV (no picture) | `apps/party_kit/src/bin/map_fitness.rs` | the home of the slice measures and the generator |
| The bots' generated mesh: a 0.5 m grid flood merged into areas up to 4 m a side, linked by shared sides; held to a box (`NavWithin`, Halo's drawn level; the seeds always kept; `cell_limit` grows to the box: Blood Gulch's 78,000 m2, about 27,000 areas); teleporters as links | `chimera_world::map::nav_gen`, `map::nav` (`NavArea`, `Via::Walk`, `Via::Teleport`) | the graph the cuts run on; the box becomes box and polygon |
| Spawning: spots on a 1.5 m grid of the main region (1500 at most), `random_hidden` out of every enemy's sight, `scored` the best of a sample with sight taking points off, the opening placed one by one. On open ground every spot is seen and the least bad wins: the owner's worry | `chimera_sim::minigame::spawns`, `spawn_score` | spots held inside; the measures model `scored` |
| Map extras in the converted-map cache (keyed by loader, converter, format, HDR, textures, kit); spots, bots' mesh and `NavWithin`'s resource made after load, map-scoped | `map::extras`, `map::cache` | the slice stays out of the cache |
| The match on the wire: `NetMinigame` (id, phase, rung extras, rungs, vote, effects); `ChangeLevel` with the map and manifest; a match held until clients have the map | `chimera_net::minigame_net`, `net::mod` | the slice rides in `NetMinigame` |
| Players-only walls (`PLAYER_CLIP_LAYER`, `MapPlayerClips`); physics-only (`PHYSICS_CLIP_LAYER`); map death and teleporters in one server system | `chimera_core::core`, `map::fall_out_of_map`, `map::teleport` | the wall's layer; the put-back |
| Camera-facing strips, sprites, additive surfaces, range fog, full-screen passes (tint, blur; no depth read) | `chimera_present::render` (`beams.rs`, `fog.rs`, `screen_post.rs`) | the look's pieces |
| The Signal look: Cobalt (ink #06080C, fg #E8EDF5, acc #3B7BFF, threat #FF3B4E; fg_5 .07, acc_2 .16), easings (`expo_out`, `pulse`), `ReducedMotion`, the theme the party UI owner keeps | [ugg-ui.md](ugg-ui.md) | the look's numbers |

What does not exist: a polygon type or any notion of "where this match is
played" finer than `NavWithin`'s box; data for slices; a map change that
carries anything besides the map's name; a wall or zone drawn in the
world by any game (no "boundary", "barrier" or "curtain" in the code);
spot-to-spot visibility, cover or boundary measures; a min cut or any
region growing on the mesh; a picture writer in `map_fitness`; a screen
pass that reads depth.

Maps it serves (wander over 10 s at 8 players, or over 15,000 m2): Halo's
Blood Gulch (78,000 m2, sight 30.5 m) and Sidewinder (69,500 m2, 33.9 m);
Combat Arms' darkthandark (23,700 m2, 49 m), deserthole and
desertholezone (19,700 m2), denied as "huge and open"; CS:S's surf_demise
and surf_sedona (courses: not candidates). At 2 to 4 players 31 more
are out by size, a few of them denied variants (14 CS:S, among them
de_dust, de_nuke, de_port; 15 Combat Arms; hangemhigh; dm_powerhouse):
question 3. TF2 has no pool data yet.

## 3. The slice as data

A slice is:

- **Boundary**: one simple polygon in plan view (engine space x and z,
  metres; counter-clockwise, 3 to 64 vertices, no holes), extruded
  vertically without limit: the wall stands from far below the map to
  far above it. A point is inside when its x, z is inside the polygon.
- **Seed**: one point on a floor inside it, where the bots' and the
  spots' floods start (the map's spawn points inside are added).
- **Players**: the range it is drawn for. Normally not written: it falls
  out of the pool rule applied to the slice's own measurements (below);
  an optional `players = [lo, hi]` narrows it.
- **Measurements**: as a map's (`area_m2`, `sight_m`) plus the spawn
  measures of section 5, written by the tool.
- **Name**: shown to players ("Blood Gulch: Red Hill").

Why a vertical prism over a polygon, not arbitrary surfaces: the test is
a 2D point in polygon (cheap on server and GPU), the wall one thin box
per edge, drawn by hand over a top-down picture in a minute, a few
hundred bytes; and it is what "cut where the ground narrows" means on
the outdoor maps that need it. It cannot put one floor of a building in
and the floor above out; on these maps stacked floors (Blood Gulch's
bases, Sidewinder's tunnels) belong together, and the later extension
is a height range per slice, not surfaces. No lid: Halo jumps 2 m and
others rocket- or grenade-jump; a lid is one more thing to get wrong.

Where it lives: `games/<game>/data/arena_slices.toml`, beside
`map_pool.toml` and `map_fitness.toml`, registered the same way
(`map_fit::MapPoolAppExt`, a new `add_arena_slices`). One file per game,
written by the tool when a person picks a candidate (A5) and editable by
hand (name, players, note); the measurements in it are re-written by
`map_fitness --slices --remeasure`.

```toml
[[slice]]
map = "bloodgulch"
id = "red_hill"
name = "Red Hill"
points = [[-61.0, 12.5], [-18.0, 4.0], [-9.5, 41.0], [-55.0, 58.5]]
seed = [-40.0, 2.1, 30.0]
area_m2 = 9850
sight_m = 22.4
seen_at_spawn = { 4 = 0.04, 8 = 0.09 }
```

How a match names it: a pool entry is `map@slice` inside the pool only
(`bloodgulch@red_hill`); the draw returns it, the map change splits it:
the map's name goes to `changelevel` as today (nothing on the wire, the
asset service, `Loaded` or the map's hash changes) and the slice goes
into the match. A host's console: `changelevel bloodgulch red_hill`, a
server's command line `+arena red_hill`. The recently-played hold-back
compares map names, so two slices of one map are not drawn back to back.

How it reaches clients: in the match, `NetMinigame::arena`, as the
slice's id, name, polygon and seed (a few hundred bytes), not the id
alone. The client needs the wall before its first predicted step, and a
client of another build may have another `arena_slices.toml`; sending
the shape means it never disagrees with the server (as the manifest
sends a definition's text when its hash differs). Map extras need not
travel because each machine loads the map itself; the slice is the
server's choice per match, so it must. The field is new replicated
state: the network protocol's version goes up with it.

What keys the caches: nothing changes in the converted-map cache (the
slice is applied after the load, never as a map extra, so one converted
Blood Gulch serves every slice). The runtime products the slice changes,
the spawn spots and the bots' mesh, are map-scoped today; they become
"map and slice" scoped: a match on the same map with another slice makes
them again (A1 decides whether that is a reload of the map or a rebuild
in place; a reload is simpler and costs one load screen). `map_fitness`
rows for slices are keyed `map@slice`.

## 4. Finding candidates

From the bots' mesh to closed boundaries, offline in `map_fitness`
(A5). The graph is the mesh's main region: a node per area (27,000 on
Blood Gulch), an edge per walk link, its capacity the width of the side
the two areas share (the portal), teleporter links left out (a slice
cuts teleporters, section 7).

1. **Targets by player count.** From `map_fit`'s formula, the area at
   which a slice of mean sight `s` has wander time `W` at N players is
   `A = W * (N - 1) * s * 16.17` (16.17 = 2 * 4/pi * 6.35). The pool
   takes a map up to 10 s and weighs 4 to 8 s fully, so a slice is
   aimed at 8 s at the low end of a player band, which leaves it at 4 s
   or more at the high end when the band's top is at most twice its
   bottom's "others". Four bands: 2-3, 4-6, 7-12, 13-16.

   | band | aimed at | area at sight 15 m | at sight 25 m | the pool keeps it at |
   |---|---|---|---|---|
   | 2-3 | 8 s at 2 | 1,940 m2 | 3,230 m2 | 2-3 |
   | 4-6 | 8 s at 4 | 5,820 | 9,700 | 4-6 |
   | 7-12 | 8 s at 7 | 11,640 | 19,400 | 7-12 |
   | 13-16 | 8 s at 13 | 23,280 | 38,800 | 13-16 |

   For contrast, whole Blood Gulch at 16 players is 10.5 s (just out).
   Sight is not known before the cut, so each target is grown twice:
   once at the whole map's sight, then again at the first cut's.
2. **Seeds.** 24 per map: farthest-point sampling over the main region,
   started at the area of most clearance (below), plus the centre of
   each cluster of the map's spawn points (bases), so every part of the
   map is somebody's middle.
3. **Grow.** From a seed, Dijkstra over walking distance until the areas
   reached sum to 0.7 A (the core) and on to 1.4 A (the outer ring).
   Walking distance, not plan distance, so a cliff's top and its foot
   are far apart and one-way drops are respected.
4. **Cut where the mesh is narrow.** A minimum s-t cut on the graph
   between the core (source) and everything past the outer ring (sink),
   capacities the portal widths: the cheapest set of portals to close is
   the narrowest necks between 0.7 A and 1.4 A, since a side where an
   area has no neighbour (a wall, a cliff, a drop) costs nothing. The
   cut's source side is the slice's region. Necks need no separate
   detection; the clearance map (each area's walking distance to the
   nearest side without a neighbour, a multi-source Dijkstra from those
   sides) only places the seeds and labels the pictures.
5. **Outline.** The region's areas' union is a rectilinear outline
   (along walls it follows the mesh's edge, so along geometry); it is
   simplified (Douglas-Peucker at 2 m, only ever outward into ground the
   region does not walk, never past a cut area's centre) to at most 64
   vertices. Then membership is recomputed by the polygon itself (areas
   whose centre is inside), since the prism also takes areas above and
   below the region in plan view; a candidate whose recomputed area
   differs by more than 15% is dropped.
6. **Keep.** 24 seeds by 4 bands is 96 cuts; candidates overlapping
   another by more than 80% (shared area over the smaller) are merged;
   the top 8 per band by score (section 5) go on the sheet.

Cost on Blood Gulch: a Dijkstra over 27,000 areas and about 110,000
links is a few milliseconds; a cut on at most that graph (Boykov and
Kolmogorov's algorithm, or Dinic's) tens of milliseconds; 96 of each
about 5 s. The measures (section 5) dominate: about a minute on top of
the load and the flood the tool already does. No part runs in a game.

## 5. Scoring

Every measure is computed inside the polygon only: samples, spots and
paths on the slice's region, rays stopped where they leave the polygon
(nobody stands beyond it, so sight past it meets nobody).

Shared inputs, computed once per map and reused by every candidate:
the spawn spots as the game makes them (1.5 m grid); up to 2,000 of
them sampled evenly; a **visibility table** between those 2,000 (a pair
sees when a ray from one's eye, 1.6 m, reaches the other's chest or
head, 1.2 m or 1.6 m, against `SOLID_LAYERS`, within 120 m: about 2
million rays on Blood Gulch, tens of seconds over the worker threads);
16 level rays from eye height at each sample (the existing sight rays,
uncapped, with their hit distances).

Measures already in the tool, inside the slice: `area_m2` (the region's
areas), `sight_m` (mean ray length, each ray clipped at the polygon,
capped at 50 m), `wander_N_s` by the formula, `pair_p50/p90` (walk
between random points), `spots`.

New, for the owner's worry (spawning in view and dying at once):

- **`see_pairs`**: the share of pairs of spawn spots in the slice that
  see each other (from the table). A flat open field is near 1.
- **`seen_at_spawn_N`**: the chance that a fresh spawn is in sight of at
  least one enemy the moment it appears, with N - 1 enemies at random
  spots and the spawn taking the least seen of 16 random spots (what
  `scored` does, in short). Monte Carlo over the table, 2,000 trials, at
  each N of the slice's range.
- **`seen_after_s`**: the median time before a fresh spawn is first in
  an enemy's sight, with the N - 1 enemies walking random routes over
  the region at 6.35 m/s (their position snapped to the nearest sampled
  spot for the table), capped at 20 s, at the top of the slice's range.
  Read against the 2 s of spawn protection.
- **`cover`**: the share of samples with something solid within 3 m in
  at least 4 of their 16 rays (somewhere to duck within a step or two).
- **`long_sight_m`**: the 95th percentile of the clipped ray lengths,
  and the longest pair in the table (the sniper's line).
- **Boundary cost**: the polygon's perimeter split by sampling each edge
  every 0.5 m: a point over walkable floor of the whole map's mesh is
  `open_m`, any other `along_m`; `open_share = open / (open + along)`.
  Open wall is where a player meets an invisible wall in a field.
- **Connectivity**: the slice's region walked to and back from the seed
  (links inside only); areas inside the polygon outside that component
  are pockets (reachable only through the outside, or not at all).
  `pocket_share` is their area over the slice's; no spot may stand in
  one (spots are drawn on the component anyway, so this guards the
  data).

Refused (first numbers):

| measure | refuse when | why this number |
|---|---|---|
| wander at the range's low end | over 10 s | the pool's own rule |
| spots | under 150 | ten per player at 16 with room to choose |
| `seen_at_spawn` at the range's top | over 0.15 | one spawn in seven seen at once is already more than the gg_ arenas show (to measure) |
| `seen_after_s` at the top | under 3 s | protection is 2 s, plus a second to turn |
| `see_pairs` | over 0.5 | half the spots in each other's view is the open field |
| `cover` | under 0.2 | |
| `long_sight_m` p95 | over 60 m | |
| `open_share` | over 0.5 | |
| `pocket_share` | over 0.02 | |

Calibration (A3): run the same measures over whole maps the pool plays
and players liked (de_dust2, cs_office, Combat Arms' warehouse and
darkhills, the gg_ arenas, hangemhigh, chillout, prisoner) and over the
ones it refuses as open (darkthandark, whole Blood Gulch); put each
threshold just outside the worst liked map, and check it refuses the
open ones. The table of those numbers goes into this plan. Then A4's
play numbers (deaths within 3 s of a spawn) on slices against pool maps
correct them. The thresholds are data in chimera/sim/data/map_fit.toml
(`[slice]`), not code.

Ranking among the slices that pass: the pool's weight at the band's
middle count, times `(1 - seen_at_spawn)`, times `(1 - open_share / 2)`.
Simple on purpose: a person picks, the score orders the sheet.

## 6. The curated list

`map_fitness --slices --filter bloodgulch` writes, per map, a sheet: one
PNG per candidate, top-down (the height map the tool already samples,
shaded by height; the slice's region bright, the rest dim; the polygon
in blue with its open edges thicker; spawn spots as dots coloured by
how seen they are; the seed and the map's spawn points marked; a 10 m
grid), titled with its id, band and every measure of section 5, failed
ones in red; a contact sheet of the 8 per band; and a CSV of all
candidates, passing or not. A person picks with `--pick <map>
<candidate> <id> [name]`, which writes the slice into the game's
`arena_slices.toml` with its measurements; editing points by hand and
`--remeasure` is the way to adjust one.

How many: two or three per map per band it can serve (Blood Gulch: all
four bands; Sidewinder: probably three), so a map played often doesn't
always play the same part. A slice the measures refuse may still be
kept with `allow = "reason"`, as `[[allow]]` overrides the pool's flags.

## 7. Playing a slice

Applied on every machine after the map loads and before anything spawns
(a map-scoped resource `PlayArea`: polygon, seed, id; inserted by the
minigame on the server, from `NetMinigame::arena` on a client).

- **Players**: a wall of thin boxes (0.2 m thick, one per edge, from
  1000 m below the slice's lowest floor to 1000 m above its highest) on
  `PLAYER_CLIP_LAYER`: every character's movement meets it, no trace
  does. Each game's movement must be checked against it (Source's
  movement sweeps the map's brushes itself; Halo's movement has its own
  collision): A1 tests all five.
- **Shots, projectiles and grenades** stop at the wall (the owner's
  decision 5): the wall is ordinary solid static collision, so every
  game's traces and projectiles meet it as they meet a level's wall (a
  grenade bounces or bursts on it by its own game's rule), with no
  per-game code. The players-only layer above is therefore not used:
  one solid wall serves movement and shots alike.
- **Vehicles**: none (off in the party apps). If a host turns them on, a
  slice places none; a later milestone gives the wall to vehicles.
- **A character outside** (by its feet, more than 0.5 m past an edge: a
  teleporter, a push, a server move): put back at a spawn spot, no death
  counted, as `fall_out_of_map` does for a character that can't die.
  Checked in the same system. At a match's start everybody is placed
  anyway.
- **Bots' mesh and spawn spots**: `NavGenParams::within` becomes
  "within the box and the polygon": a cell is kept when its floor point
  is inside both; seeds outside the polygon are dropped (the slice's
  seed always kept); `cell_limit` uses the polygon's bounding box when
  smaller. One helper (`nav_gen::held(world)`) replaces the four callers'
  `NavWithin::of`. The map's spawn points outside are not used; a map
  with none inside spawns at the seed until the spots are made.
- **Map objects and pickups outside**: not placed (Halo's netgame
  equipment, `weapon::respawn::MapPickup`); scenery stays (it is drawn
  and solid as before). A dropped weapon that lands outside is removed.
- **Teleporters**: a pad is kept only when its source and destination
  are both inside; the others are not placed (`MapTeleporters` filtered
  before the mesh links them). Blood Gulch's base-to-base pairs go.
- **Kill plane**: unchanged (`KillHeight`). **Network**: only
  `NetMinigame::arena` (section 3); the rest already replicates.

## 8. The look

A first design in the Signal language; the party UI owner finalises the
numbers, which live in the theme as tokens (`[arena]`), not in code.

- **Ground line**: where the wall meets the floor. Each edge sampled
  every 0.5 m, the floor found by a ray down (every floor under it on a
  stacked map), a strip 0.12 m wide 2 cm above it: a core of acc #3B7BFF
  at .9 over an underlay of ink #06080C at .5, 0.35 m wide. Dashes 1.5 m
  on, 0.5 m off, drifting along the edge at 0.2 m/s (none under reduced
  motion). Kept at least 1.5 px wide at any distance (a beam's
  distance-scaled width), so it reads from across the map.
- **Faint curtain**: a vertical band along each edge from the ground
  line up 4 m, acc at .10 at the bottom fading to 0 at the top, with
  fine horizontal lines every 0.5 m in fg at .07 (fg_5). Additive, in
  the scene's fog. From 30 m to 200 m it reads as a pale blue veil
  standing on the line; past 200 m it fades out.
- **Near glow**: within 4 m of the wall, on the curtain's own mesh, a
  disc of hexagon cells (0.4 m) around the point of the wall nearest the
  player's chest, radius 3 m, strength `(1 - d / 4)^2`, acc rising to fg
  at under 0.5 m. Pushing into the wall sends a ring out from the
  contact (the `pulse` easing, 400 ms) and a soft UI sound. Drawn for
  the local player only.
- **The world beyond dimmed**: outside the polygon the frame is darkened
  to .55 and moved halfway to grey, soft over 2 m past the line; the sky
  left alone. A screen pass after the world reads depth, rebuilds each
  pixel's position and tests it against the polygon (up to 64 edges in a
  uniform). A8, separate: it needs the depth the post pass does not read
  today.

Costs: the line is one draw of about 4 triangles per metre of edge (a
400 m boundary is 1,600); the curtain one draw of two triangles per
edge segment, additive, its overdraw only where you look through it;
the glow is the curtain's shader with the player's position as a
uniform, no extra draw; the dimming one full-screen pass of up to 64
edge tests per pixel, well under a millisecond at 1080p. Together under
half a millisecond on the playtest machines (to measure in A7).

Bright and dark maps: additive blue fades on Blood Gulch's sunlit sand
and grass, so the line carries its own dark underlay (contrast against
light ground and dark alike) and the curtain's alpha is a token with a
per-slice override (`look = "bright"` takes .16). On dark maps the
additive line and curtain read as they are. To be checked on Blood
Gulch at noon and on a dark map (A7's pictures).

Accessibility: colour is never the only cue: the dashes, the lines on
the curtain, the hexagons and the contact sound carry it too. Blue on
green and sand stays apart for red-green colour blindness; the shapes
carry it for blue-yellow. Reduced motion: no drift, no ring, the glow
fades without a pulse. A brightness setting for the boundary is the UI
owner's call.

The UI owner's to decide: the colours and alphas, the dash and hexagon
patterns, the sound, whether the dimming is wanted (question 11), and
the slice's name in the vote, loading screen and Tab board.

## 9. Risks and open questions

Risks:

- A movement that walks through the wall (section 7): A1's first test.
- The measures may disagree with play: a slice can pass and still feel
  open. A4 measures spawn deaths in play so the thresholds learn.
- Plan view joins stacked floors (section 3); a candidate that cuts a
  building in plan is dropped by the 15% check, not fixed.
- A small slice has few spots (1.5 m grid: 2,000 m2 is about 900): fine
  at 2-3 players, the floor of 150 guards the rest.
- Same-map slice changes rebuild the spots and the bots' mesh (Blood
  Gulch's flood is the slow part of its load); the rotation's hold-back
  mostly avoids it.
- The map beyond is still drawn whole: a slice costs as much to draw as
  the whole map, which the playtest machines already carry.

Decisions (the owner, 2026-11-15; they answer this plan's twelve
questions and override the sections above where they differ):

1. Curated by hand first; the generator (A5) once those play well.
2. One fixed slice per match.
3. Only the big maps: no slices for maps the pool already plays.
4. A solid wall, nothing that hurts outside it.
5. Bullets and grenades do NOT pass the wall (section 7 proposed that
   they pass): the wall is solid to everything, as a level's own wall
   is. So it is ordinary static collision every game's traces,
   projectiles and movement already meet, not a players-only layer;
   a curated slice should not be so concave that its wall hides one
   part from another (a scoring point for A3 and A5).
6. A display name per slice, generated ("Blood Gulch: Red Hill").
7. No two slices of one map in the same vote, for now. A slice is a
   sub-map: the map is the parent and its slices hang under it (the
   pool entry, the vote, the name and the data follow that hierarchy).
8. No vehicles on slices for now (later perhaps, once vehicles work
   with every character type).
9. The placeholder look ships with this work; design comes after.
10. Never in Halo's own app: that stays faithful.
11. The world beyond dimmed: yes, but show the owner what it looks
    like before it is kept.
12. No hand exception for a whole big map at a player count: which
    entry is drawn (the whole map or one of its slices) is decided by
    a heuristic (the pool's measures today), tuned from playtests to
    what is fun.

## 10. Milestones

Each one agent's work; sizes small, medium, large.

**A1 One hand-drawn slice of Blood Gulch, plain wall** (medium-large).
`PlayArea` and the polygon (chimera_world: point in polygon, `nav_gen::
held`, the wall colliders, the put-back in `fall_out_of_map`);
`arena_slices.toml` for Halo with one slice drawn by hand on the
`--dump` height map; spots, spawn points, pickups and teleporters held
inside (chimera_sim, halo_ce data); `NetMinigame::arena` and the
protocol bump (chimera_net); `+arena <id>` and `changelevel <map> <id>`
(party_kit); a plain look: the ground line in flat acc and a flat
curtain at .1 (no glow, no tokens). Proof: party_kit scenario tests that
each game's character (CS:S, Combat Arms, HL2:DM, TF2, Halo) walking at
the wall stops at it and a client's prediction agrees; a toy map test of
the held flood (cells, seeds); the heavy test on Blood Gulch: every spot
and bot area inside, no teleporter crossing, a bots' match runs; an
offscreen picture. Coordination: `NetMinigame` is shared with the ladder
session (protocol bump).

**A1 as built** (2026-11-15). Where it differs from the above:
- The wall is ordinary static collision (decision 5): one static cuboid
  per edge, 0.5 m thick, 1000 m above and below the seed, on the default
  layer (`play_area::ArenaWall`); not `MapBrushCollider`, so Source's
  movement meets it through its physics casts (its brush sweep does not
  see it), which the tests show stops CS:S, HL2:DM and TF2.
- The slice's types and the play area are `chimera_world::map::
  play_area` (`PlanPolygon`, `ArenaSlice`, `PlayArea`, `ArenaPlan`); the
  data reader `chimera_sim::minigame::arena` (`ArenaSlices`,
  `MapPoolAppExt::add_arena_slices`); the file's `name` is the slice's
  own and the shown name is made with the map's (`map_name` or
  `hud::map_display_name`): "Blood Gulch: West Base".
- The bots' and spots' floods: `NavGenParams::plan` beside `within` (not
  one merged field) and `nav_gen::hold(&mut p, world)` for the four
  callers (Combat Arms' bots' brain one line). The map's spawn points
  outside lose `SpawnPoint` and one is added at the seed, so every flood
  seeded from spawn points starts inside; the spots are also filtered by
  the polygon (a map with its own mesh).
- Starting a match on a slice: the party kit's console command `arena
  <map@slice>` (`+arena` on a server's command line), not `changelevel
  <map> <slice>`: it sets the plan and loads the map (`changelevel`),
  or makes the play area at once on a map just in (`+map
  halo_ce:bloodgulch +arena west_base`). Another map coming in forgets
  the plan. Nothing in the pool, rotation or vote changed (A2).
- The play area is made at the map's spawn (`play_area::map_spawned`,
  queued with the map's resources) or when the plan changes on a client
  (`follow_plan_system`), never in the converted-map cache. The look
  (`chimera_present::render::arena`): a curtain 4 m tall fading upward
  and a foot line, flat accent blue, unlit, from the floor found under
  each 0.5 m of edge.
- Not done in A1, done with A3 (2026-11-16): the client prediction test
  (party_kit `arena_net`: a client running and jumping into the wall as
  CS:S and as Halo is stopped by its own prediction where the server
  stops it, no prediction error; a client joining mid-match has the wall
  from its first tick with a player: `follow_arena` now builds it in the
  same frame the match arrives, it came one tick late); a dropped weapon
  or item found outside is put back at the nearest point inside, 1 m
  from the wall (`arena::put_loose_back_inside`; the wall already stops a
  thrown one, so only one born past the wall gets there). Shots: every
  game's bullet, rocket, grenade and projectile stops at the wall
  (`arena_wall` adds HL2:DM's frag and crossbow, TF2's pipe, Combat Arms'
  M67, Halo's plasma rifle and needler to A1's ten: none hurts past it,
  each hurts without it). A player outside (a missed spawn, a push, a
  fall) is put back at the nearest spawn point, as A1 made it; the spawn
  points now include the slice's own (A3). Teleporters: a pad with
  either end outside is off for the match (A1). Blood Gulch has two pads
  (from the map's data as the tool loads it): (473.8, 5.5, -291.3) to
  (349.1, 2.1, -252.5), both ends outside west_base, and (252.7, 5.5,
  -122.1), on the west base's roof inside the slice, to (384.8, 0.7,
  -133.5) outside: both off for the match; no other Halo map has a slice.

**A2 Slices in the draw** (medium). Slices join `MapPool` as `map@slice`
entries with their own `Fitness`; the draw, the vote and the map change
split them; the hold-back by map. The vote's and loading screen's label
is the party UI owner's (coordinate: the name only). Proof: `map_fit`
unit tests (a slice in at its range, out outside it); party_kit's
`map_pool_by_count` with slices; ultimate_gun_game's `heavy::pool_maps`
loads a drawn slice.

**A2 as built** (2026-11-16). Where it differs from the above:
- **The entry.** A slice is `game:map@id` in the pool, the rotation, the
  vote, the history and a match's map change (not inside the pool only,
  as section 3 had it: the entry in play is `minigame_net::current_entry`,
  the map or `map@id`). The party apps' pool has each map's slices right
  after it (`arena::with_slices`, only with `arena::SlicesInPool`, which
  the party kit's `ArenaPlugin` puts in: a game's own app never has a
  slice in its rotation); `map_fit::entry_map` gives an entry's map.
- **Its fitness** (`map_fit::MapPool::judge` on `map@id`, registered by
  `arena::add` as a `SliceFit`: the data's `players` band and its
  measures). Out while the whole map is in at the count (or has no
  measurements, or is broken): the map's own measurements decide "too
  big"; out unless the slice's data carries the spawn-safety measures
  (`area_m2`, `sight_m`, `spots`, `see_pairs`, `seen_at_spawn` by count,
  `cover`, `long_sight_p95_m`; optional `point_floor_seen`,
  `point_seen_by`) and they pass `[slice]` (`SliceLimits::judge_measures`,
  which `judge` on a `Report` now calls): the measures need the map loaded
  (the visibility table), so the tool writes them into the slice's data
  and the pool reads them, judged against the limits at run time (a
  changed limit applies without measuring again); in at the counts the
  measures suit (`SliceVerdict::players`) narrowed by the data's band; its
  weight by its own wander time, as a map's. Pool rules name a slice
  `map@id`. West Base has no measures in its data: never drawn.
- **The draw.** `vote::candidates_weighted`: one entry per map on a
  ballot (a slice is its map: the pick's siblings and the map played, or
  any of its slices, are left off); `rotation::MapHistory::next_weighted`
  and the ballot's fresh-first both compare recent entries by map.
- **The map change.** `change_match` to `map@id` (a rotation's, a
  vote's, a host's `changelevel map@id`, the `arena` command with a
  minigame playing) plans the slice (`arena::plan`) and changes level to
  the map by its name, so nothing new goes on the wire (net66 kept: the
  slice reaches clients with the match, `NetMinigame::arena`); to a whole
  map it clears the plan. The same map again (another slice of it, or it
  whole after one) is loaded again, and `MapChange::reload` (the map
  file's change tick when it began) holds the match until the load has
  replaced it.
- **Names.** `vote::map_name` (the vote's and the lobby's label; now a
  `Cow`) and `hud::map_display_name` read a slice as "<map's display
  name>: <slice id as words>": `halo_ce:bloodgulch@west_base` is
  "Bloodgulch: West Base". They are pure functions of the id, so the map
  part is the id's words, not the data's `map_name` ("Blood Gulch"; the
  data's full name, "Blood Gulch: West Base", is `ArenaSlice::name`, sent
  with the match). The loading screen shows the map's id (the level
  change carries the map only): "bloodgulch".
- **The Maps picker** (`party_kit::maps_picker`): slices are listed after
  their map, a row one step in (the same row and box), named by id; a
  map's row sets it and its slices; a slice toggles alone; a slice whose
  map is off shows off, and turning it on turns the map on (its other
  slices stay off); the counts are of whole maps. `mp_maps_off` names
  `map@id`, and `MapsOff::has` takes a map's slices with it.
  `ui_scene play maps slices` stages it (the games without slices folded).
- Proof: `map_fit`'s `a_slice_is_in_at_its_range_out_outside_it_and_out_
  when_unsafe`, `arena`'s `a_slices_stored_measures_join_the_pool`, the
  ballot's and the rotation's unit tests; party_kit's `arena_pool` (on
  `NetSim`: the pool at 2 and 12, the ballot, a `changelevel` to a slice
  building the wall on the server and both clients, the hold-back);
  `maps_picker`'s unit tests; ultimate_gun_game's heavy
  `pool_maps::a_drawn_slice_loads_and_plays_inside_its_wall` (a test
  slice with passing made-up measures, drawn at 4 players on Blood
  Gulch).

**A3 Measuring a slice** (medium). `map_fitness --slice <map>@<id>` and
the section 5 measures, the visibility table, the calibration sweep and
its table here, the thresholds in map_fit.toml. Proof: unit tests on toy
maps (a walled maze passes, a flat field fails on `see_pairs` and
`seen_at_spawn`), A1's slice measured.

**A3 as built** (2026-11-16). Where it differs from the above:
- The measures are plain functions in `chimera_sim::minigame::
  slice_safety` (the visibility table `SeeTable`, `Safety`, `Report`,
  `point_safety`, `level_rays`, `choose_points`) over positions and a
  line-of-sight test, so the tool, the game and the unit tests (toy maps
  of walls drawn in plan) measure alike. The limits are `[slice]` in
  chimera/sim/data/map_fit.toml (`map_fit::SliceLimits::judge`: what a
  slice breaks, and the players it suits: from the least count whose
  wander is within `target_s` to the most whose `seen_at_spawn` is within
  its limit).
- `map_fitness --slice <game:map@id>` plays the slice as the party kit's
  `arena` does and measures as before inside it, then the spawn-safety
  part (`map_fitness/safety.rs`): the table (up to `--table`, 1,500 spots;
  1.9 s on West Base), `see_pairs` (equal to the mean share of the other
  spots that see a spot: the first of the owner's two numbers, printed
  with its 90th percentile), `seen_at_spawn` at 2, 4, 8, 12, 16, the spawn
  points' exposure (each one's share of the floor seeing it and the other
  points seeing it; the share of floor seeing any point: the second
  number), cover, long sight, vertical range, a choke share (the most
  crossed mesh area's share of 48 x 48 walks), spawn spacing; three
  times on a slice: the map's points inside, with the points the slice
  makes, and with only its own (as on a map none of whose points is
  inside). `--safety` gives whole maps the same. Output: text, a row per
  report in `safety.csv`, a top-down PNG (written by hand, uncompressed:
  no new dependency). Not built: `seen_after_s` (walking enemies) and the
  boundary cost (`open_share`), `pocket_share`.
- Judged and not: `point_floor_seen` is the mean over the points of the
  floor share seeing one (the share seeing any of them only grows with
  their number, so that one is told, not judged); `see_pairs` replaces a
  separate spot-seen-by limit (the same number).
- Spawn points: a slice whose map gives it fewer than `points_want` (8)
  inside makes the rest once its spots are made (`arena::
  arena_spawn_points`, the server's; `ArenaMadeSpawn` `SpawnPoint`s):
  from the spots at least `wall_gap_m` (4 m) from the wall, each the least
  seen by the points already there, then the least seen by the floor
  (96 spots), then the farthest, `points_apart_m` (12 m) apart where it
  can; seeded by the slice's key, so every machine with the same map,
  slice and spots makes the same (no network change, net65 kept). They
  serve the modes that spawn on points, the put-back, and later floods.
  The spots themselves depend on the match's spawn seed (the minigame's
  id), so the tool (the gun game's) and another minigame may pick other
  points.
- West Base: the map has 29 spawn points inside (with the seed 30), so
  it makes none; made alone it puts 8 points none of which sees another
  (four in and about the base, mean floor share 0.29 against the map's
  points' 0.50).

Calibration sweep (2026-11-16; `--safety`, 1,500 spots in each table;
liked at small counts: CS:S's cs_italy, cs_office, de_cbble, de_dust2,
gg_bk_warehouse_v1, Halo's chillout, hangemhigh, prisoner, wizard,
Combat Arms' warehouse and warehouse_qrt; too open: darkthandark, whole
Blood Gulch and Sidewinder):

| map | see_pairs | seen_at_spawn 4 / 8 / 12 / 16 | cover | long sight p95 m | point floor seen | point seen by | suits |
|---|---|---|---|---|---|---|---|
| cs_italy | 0.09 | 0 / 0 / .002 / .007 | 0.69 | 21 | 0.14 | 0.49 | 4-16 |
| cs_office | 0.08 | 0 / 0 / .001 / 0 | 0.58 | 22 | 0.11 | 0.49 | 5-16 |
| de_cbble | 0.10 | 0 / 0 / .001 / .009 | 0.42 | 31 | 0.12 | 0.49 | 5-16 |
| de_dust2 | 0.10 | 0 / 0 / .005 / .014 | 0.44 | 27 | 0.11 | 0.49 | 4-16 |
| gg_bk_warehouse_v1 | 0.16 | 0 / .004 / .062 / .166 | 0.56 | 20 | 0.12 | 0.39 | 3-12 |
| chillout | 0.17 | .001 / .007 / .072 / .169 | 0.49 | 25 | 0.13 | 0.35 | 3-12 |
| hangemhigh | 0.31 | .002 / .079 / .220 / .359 | 0.25 | 78 | 0.23 | 0.52 | 5-8 |
| prisoner | 0.21 | .001 / .011 / .068 / .178 | 0.32 | 34 | 0.22 | 0.54 | 3-12 |
| wizard | 0.28 | .002 / .119 / .386 / .623 | 0.30 | 47 | 0.27 | 0.47 | 3-8 |
| warehouse (CA) | 0.20 | 0 / .004 / .041 / .101 | 0.53 | 25 | 0.04 | 0.48 | 3-16 |
| warehouse_qrt (CA) | 0.22 | .001 / .003 / .031 / .067 | 0.52 | 25 | 0.20 | 0.21 | 3-16 |
| darkthandark (CA) | **0.41** | .062 / **.428** / .650 / .785 | **0.03** | **120** | **0.52** | **1.00** | fails |
| bloodgulch (whole) | 0.20 | 0 / .032 / .143 / .300 | **0.10** | **120** | 0.22 | 0.36 | fails (and wander) |
| sidewinder (whole) | 0.22 | .002 / .014 / .058 / .093 | **0.09** | **120** | 0.05 | 0.24 | fails |
| **bloodgulch@west_base** | **0.59** | .220 / **.549** / .646 / .670 | **0.13** | **81** | **0.50** (own points 0.29) | **0.66** (own 0) | fails |

The thresholds (`[slice]`), each just outside the worst liked map, as
section 5 says: `see_pairs_max` 0.35 (hangemhigh 0.31; darkthandark 0.41),
`seen_at_spawn_max` 0.15 (the plan's first number: it keeps wizard and
hangemhigh to 8 players, the CS:S maps to 16), `cover_min` 0.2
(hangemhigh 0.25; the open maps 0.03 to 0.10), `long_sight_p95_max_m` 80
(hangemhigh 78), `point_floor_seen_max` 0.3 (wizard 0.27; darkthandark
0.52), `point_seen_by_max` 0.6 (prisoner 0.54), `spots_min` 150. These are
first numbers, to be tuned by playtests (the owner's decision 12; A4's
spawn deaths). What the table says plainly: A1's West Base fails, worse
than every whole map measured, darkthandark included (a spot sees 59% of
the others; a fresh spawn at 8 players is seen at once 55% of the time):
it is the open field the owner feared, its bunker the only cover; the
slice wants redrawing (A5/A6), not a looser threshold. Whole Blood Gulch
and Sidewinder pass the sight-pair measures only because the table stops
at 120 m and they are huge: on big maps the pairs measure size as much as
openness, so cover and long sight are what refuse them.

**A4 Spawn deaths from play** (small). The share of deaths within 3 s of
the victim's spawn, per match, in the match's results and the server's
log, so playtests check A3's thresholds. Coordination: results are the
ladder session's; the number is not shown in the UI. Proof: a scenario
test (a spawn shot at once counts).

**A5 The candidate generator** (large). Seeds, growth, the min cut,
outlines, merging, scoring, the PNG sheets, `--pick`, `--remeasure`
(party_kit's tool only; no game crate). Proof: toy maps (two rooms and a
corridor: the cut is the corridor; a ring: two cuts), Blood Gulch's
sheet as pictures, the run's time.

**A5 as built** (2026-11-16). Where it differs from the above:
- `map_fitness --candidates <game:map> [--band 2-4|4-8] [--top N]
  [--pick n [--id id] [--name name]]` (the tool's `slice_gen` and
  `candidates`, party_kit's binary only). The graph is the bots' mesh's main
  region (walk links, each the width of the shared side). Seeds: the map's
  spawn clusters' middles, then 40 by farthest-point sampling, each cell's
  distance weighed up by the solid within 6 m of it. Growth: Dijkstra from
  the seed with a walking cost up to 5 times dearer over open ground (8
  level rays' mean length to 40 m), so a region fills the sheltered floor
  first; core 0.7, outer ring 1.4 of the target; a minimum cut (Dinic's) at
  portal widths, a roofed cell's sides ten times dearer (no wall through a
  tunnel or a door if anything else will do). Targets: aimed at 8 s at the
  band's low end, at 1, 0.65 and 0.4 of it, grown twice (the map's sight,
  then the first cut's). Outline: a 0.5 m raster closed by 2 m, holes
  filled, grown by a margin of 1, 1.5 or 2 m (the one whose wall stands
  under no roof kept), traced, Douglas-Peucker to at most 16 corners;
  dropped when its area moves over 25% from the cut's (the plan's 15% and
  the margin). Measured with the A3 functions inside the polygon (sight
  stopped by it), on the map's spawn points inside, one at the seed and
  the ones `choose_points` adds; judged by `[slice]`; also refused: a wall
  under a roof over 1 m (a doorway, a tunnel), solidity (area over hull)
  under 0.6 (decision 5). Told, not judged: the wall over open floor
  (`open/wall m`), over a floor above another. A candidate passes a band
  when the counts it suits share two with it. Ranked by the pool's weight
  at the band's middle, `1 - seen_at_spawn` at its top, `1 - open/2`;
  merged over 80% shared. Output: a table, `candidates.csv`, a top-down
  PNG per candidate (the A3 picture: the wall over open floor red, under a
  roof violet) and its `[[slice]]` entry with the measures the pool reads
  (A2's `Entry` fields), in `<out>/candidates/<game>_<map>/`.
- `--pick n` writes candidate n into `games/<game>/data/arena_slices.toml`
  (same map and id replaced; checked by the game's reader). The candidate's
  spawn points are seeded by a provisional key, so the slice as played
  makes other points: `--slice <key> --remeasure` measures it as played
  and rewrites its measured fields (done for each pick).
- Names: a compass word for where it lies, and Caves (a roof over 30% of
  it), Base (five spawn points or more), Heights (8 m of floors), else
  Hollow; ids from the name; `--id`/`--name` override. No map it serves
  has named places, so none is read.
- Not built: `--remeasure` of every slice at once, the contact sheet,
  `seen_after_s`, the plan's four bands (2-4 and 4-8 only).
- Time: Blood Gulch 107 s for both bands (load and flood about 30 s more),
  Sidewinder 67 s, darkthandark 105 s; tests: `party_kit --test it
  slice_gen::` (two rooms and a corridor cut at the corridor, a ring cut
  twice, an open field yields nothing that passes, a walled maze does).

Results (2026-11-16; the best 8 per band printed, "seen-by" the first of
the owner's two numbers, "floor-sees" the second):

| map, band | candidates | pass | best | what refuses the others |
|---|---|---|---|---|
| bloodgulch 2-4 | 37 | 1 | North Caves (1,117 m2, see_pairs .28, cover .61) | see_pairs .56 to .80 and point_floor_seen on every open one; a wall under a roof (a cave's mouth) on the sheltered ones |
| bloodgulch 4-8 | 28 | 1 | North Caves (1,418 m2, suits 2-8) | see_pairs .49 to .83, cover .16 to .19, long sight p95 84 to 95 m; at 6,000 m2 or more no part of the gulch passes |
| sidewinder 2-4 | 36 | 0 | North West Heights (2,014 m2, see_pairs .32, suits 3-4: a wall under a roof 4.5 m); South West Heights (2,505 m2, .31, suits 3-8: roof 10.9 m) | see_pairs .43 to .60 elsewhere |
| sidewinder 4-8 | 16 | 0 | South East Base (8,967 m2, see_pairs .40) | see_pairs .40 to .75, cover .09 to .15, long sight 80 to 118 m |
| darkthandark 2-4 | 30 | 2 | North Hollow (2,017 m2, .35, suits 2-4), North West (1,948 m2, .25, suits 3-4) | see_pairs, point_floor_seen |
| darkthandark 4-8 | 3 | 0 | (14,539 m2, see_pairs .45, cover .10) | cover, long sight, point_seen_by |

Picked: `halo_ce:bloodgulch@north_caves` ("Blood Gulch: North Caves"), the
caves in the north cliff and their two mouths, walled at the mouths (94 m of
its 324 m of wall over open floor). As played (`--slice --remeasure`): 1,369
m2, sight 7.8 m, see_pairs .283 (a spot is seen by 28% of the others),
seen_at_spawn 0 / .003 / .139 at 2 / 4 / 8, cover .52, long sight p95 20 m,
8 spawn points (the seed's and 7 made, none sees more than one other),
each seen by 10 to 33% of the floor; passes for 2 to 8 players. West
Base: no candidate near it passes (the nearest, "West Base 2", 9,634 m2:
see_pairs .80, cover .17); it keeps its id for the `arena` command, carries
no measures (so the pool never offers it) and a `note` saying why.
Pictures: `shots/arena-a5/` beside the disc image (not in the repository).
In the game (scene pictures from two of its spawn points at eye level,
sandbox, `+sandbox halo_ce:bloodgulch +arena north_caves`): both stand in
the caves' dim tunnels; from the first, the tunnel bends away within 20 m
and no other spawn point is in view (the tool: seen by none of the
others); the second stands by the east mouth against rock, the wall's
curtain across the mouth with the sky behind it, nothing of the slice's
other points in view. The pictures agree with the numbers; they also show
what the numbers do not judge: it is a dark, close cave fight (mean sight
8 m), not Blood Gulch's open game, which is for a playtest to call.

What would have to give for Blood Gulch to have more: (a) a wall that may
close a tunnel or a cave mouth (most sheltered candidates fail only that:
North West 1,836 m2 suits 3-8, roof 4.9 m); (b) slices only around the
caves and rocks at 2-8 players, which is what passes now; (c) the open
gulch at 4-8 cannot meet see_pairs .35 or cover .2 at any size the
generator tried: those thresholds are not this map's to meet, and only a
playtest (A4) should move them.

For A6 (the owner): look at `<out>/candidates/<game>_<map>/` after a
`--candidates` run (the PNGs and the table); pick with `map_fitness
--candidates halo_ce:<map> --band <band> --top <N> --pick <n> --id <id>
--name "<name>"` (the run is deterministic: the numbers are the same as
the sheet's), then `map_fitness --slice halo_ce:<map>@<id> --remeasure`.
Worth a look: Sidewinder's two near misses (whether a wall in a tunnel is
acceptable there), darkthandark's two passes (A9).

**A6 Curating Blood Gulch and Sidewinder** (small, a person's picks).
Two or three slices per band; Sidewinder's deny becomes "whole map
denied, slices allowed"; a playtest round with A4's numbers.

**A7 The look** (medium). Ground line with underlay, curtain, near glow,
contact ring and sound, reduced motion; tokens in the theme
(`[arena]`). Built in chimera_present with the party UI owner deciding
the numbers. Proof: offscreen pictures on Blood Gulch (day) and a dark
map, near and far; a reduced-motion test; frame time before and after.

**A8 The world beyond dimmed** (medium). The depth-reading screen pass.
Proof: pictures; frame time at 1080p and 1440p. Only if question 11
says yes.

**A9 Other games' big maps** (small each). Combat Arms' darkthandark,
deserthole, desertholezone after a first look at them; the 2-4 player
slices of the middle-sized maps if question 3 says yes.
