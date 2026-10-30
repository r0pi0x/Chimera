# Performance

How to measure, what the numbers were, and what the map visibility culling
does; at the end, how long the tests take to build and run ("Test cycle"). Tools are listed in [OBSERVABILITY.md](OBSERVABILITY.md) ("3c.
Performance").

## Measuring

- `chimera_perf 1` in the console: frame times, main-world CPU, GPU time
  per render pass (Vulkan/DX12 timestamps; every camera's run of a pass
  summed per frame: Bevy reports each as its own measurement under the
  pass's name), entities, meshes and triangles drawn, cameras drawing,
  the camera's visibility cluster. `chimera_perf_log 1`
  logs it as text once a second; `bugreport` saves it in report.txt.
- `refcmp bench --views tools/refcmp/<map>.toml`: 200 frames per view
  after settling, vsync off, offscreen at 1280x720 (`-- --view-size
  3840x2160` for another size); prints avg/p95/max frame ms, main-world
  and render-world ms, process CPU and GPU ms, drawn meshes/total,
  triangles drawn, map parts potentially visible. Build the profile you
  want first (`cargo build --features dev`, or `cargo build --profile
  playtest` and run `target/playtest/refcmp`).
- A/B: `-- +r_novis 1` (no PVS culling) or `CHIMERA_MERGED_WORLD=1` (the
  world as one mesh per material, nothing culled: the code before
  chunking); `CHIMERA_MERGE_BRUSHES=0` (brush entities not merged);
  `CHIMERA_GPU_PREPROCESS=1` (Bevy's GPU mesh preprocessing and
  indirect draws instead of batching each camera's meshes on the CPU),
  `CHIMERA_SKIP_OUTPUTS=0` (every camera copies its frame out, not only
  the last on each target; "Party frame-rate pass", second pass).
- One second of a frame's spans to look at yourself (any app, Windows
  too): `--features trace` and `trace_capture 1`, or Tracy ("See a frame
  yourself" below).
- Per-system CPU time: a `--features profile` build writes a Chrome
  trace (`TRACE_CHROME=<file>`, else `trace-*.json` in the working
  directory); `tracesum <file> --skip 200` prints the main and render
  worlds' time per frame and the top spans by self time (OBSERVABILITY.md
  3c has the commands); https://ui.perfetto.dev shows the timeline.
- `REFCMP_OUT=<dir>` keeps refcmp's captures and reports in your own
  folder (the CS:S references stay shared).
- **Which code takes the time** (Linux): `tools/perf-profile.sh` runs a
  game build with line tables and frame pointers (the `profiling` profile;
  build line in the script) under `perf record` (user space only, so
  `perf_event_paranoid` 2 is enough), vsync off and uncapped, samples
  from `--after` seconds for `--seconds`, and hands `perf script` to
  `profsum` (`chimera/tools/src/bin/profsum.rs`), which writes into
  `--out`: `table.md` (each thread's CPU ms per frame; our code by the
  deepest frame of ours on the stack, callees included; Bevy's systems
  where no frame is ours; functions by self and by total time; ms per
  frame and %), `flame.svg` (a flame graph, the plumbing between a thread
  and its systems left out, coloured by crate family: ours, Bevy, wgpu
  and the driver, std and the allocator), `top.svg` (bars of the top
  rows) and `top.json`. `--baseline <earlier out>/top.json` adds before,
  after and delta columns and before/after bars: run it on the build
  before a fix, then after, same scene. `profsum <perf.txt> --focus
  <function>` shows who calls something (e.g. `QueryState::new`).
  `rsvg-convert -w 1800 flame.svg -o flame.png` for a picture to post.
  Windows (no perf): a span trace (`--features trace`, `trace_capture`,
  `scripts\profile.ps1`), `tracesum <trace> --folded spans.txt` and
  `profsum spans.txt --unit us --frames <N> --out <dir>`: the same
  tables and pictures by Bevy span (system, schedule, render pass), with
  tracing's overhead (frames 2-5x slower). `tracesum` also gives each
  span's p99 over frames and its share of the main world's frame.
  Shortcuts: the distribution's glibc has no frame pointers, so samples
  in malloc, free and memcpy lose their callers (`[libc.so.6] (callers
  unknown)`); DWARF unwinding (`--call-graph dwarf,65528`) lost most of
  the main thread's stacks and 15-50% of samples here.

- Load-independent numbers: `chimera_perf`, `chimera_perf_log`, the debug
  UI's Perf tab and `refcmp bench` also give percentiles (p50 to p99.9, 1%
  low fps) of each world's thread CPU time and user-space instructions
  per frame and per tick (Linux hardware counters), and `bench.json` /
  `bench.csv`; `chimera_hitch_ratio` logs what a slow frame spent its
  time on; `perfgate` fails when the simulation's instructions per tick
  rise ("Load-independent metrics" below).

Frame times move with machine load (other builds on the dev box): compare
runs taken back to back, and trust ratios more than absolute numbers.
On a busy box, interleave the builds over several rounds and compare
medians and minimums ("Frame time pass" below), or compare
instructions, which hardly move with load ("Load-independent metrics").

## Visibility culling (`render::vis`, `map::vis`)

Source maps carry precomputed visibility: the BSP tree's leaves belong to
clusters, and the visibility lump lists, per cluster, every cluster
potentially visible from anywhere inside it (PVS, run-length encoded).
`games/cs_source/bsp.rs::visibility` reads it into the game-independent
`MapVisibility` (tree, leaf clusters, per-cluster bit sets).

- World meshes (merged per material by the importer) are cut into chunks
  on a 512-unit grid (`vis::CHUNK_SIZE`, by triangle centroid); each
  chunk knows the clusters its triangles' boxes touch. Chunks have tight
  bounds, so Bevy's frustum culling works on them too.
- Static props, prop entities that don't move, sprites (glows included:
  no occlusion rays while culled), ropes, dust volumes and static props'
  dynamic shadows are tagged by the clusters their boxes touch.
- Each frame `vis::cull` finds the main camera's cluster (tree descent);
  when it changes it shows the tagged parts whose clusters are potentially
  visible and hides the rest. In solid or outside the map (no cluster),
  or with `r_novis 1`, everything is drawn.
- The water's reflection camera (`render::water`) draws only for a
  reflecting volume the main camera can see (in its frustum and in its
  cluster's PVS; the spec renders the views of the volume the camera
  sees). While it draws, parts potentially visible from the clusters
  around its surface are drawn too (`water::ReflectionClusters`: what is
  reflected is seen from the surface). Its mirrored eye is under the
  surface, often in solid: looking it up there turned culling off for the
  whole map whenever any reflecting water lay below the eye. The
  view-model and sky cameras draw only their own layers and are ignored.
  Cost (2026-10-09, playtest build, `refcmp bench` on de_port's 24
  views, with and without `+r_WaterDrawReflection 0`; instruction counts,
  since the box was loaded): views facing away from the water are the
  same (render world 16.4M vs 15.4M instructions per frame), views with
  the water in sight pay 25-45% more render-world instructions (all
  views: 34.0M vs 26.6M; main world 8.3M vs 7.4M) and 112 more meshes on
  average; GPU time stays about 1 ms at 1280x720 either way, so the
  reflection keeps the spec's 1024x1024 target. The cheaper settings are
  the game's: `r_WaterDrawReflection 0`, or a material without
  `$reflecttexture` ("Simple reflections" alone still reflects, as
  `$forceexpensive` defaults on: water.md open question 1).
- Meshes of entities with their own node (movers, breakables) stay whole;
  the node is tagged by the clusters its meshes' bounds touch, found again
  whenever it moves (`map::tag_moved_brush_entities`, from its
  `BrushEntityBounds` and `GlobalTransform`), so doors and breakables are
  culled like the world around them (props riding them with them). The
  logic hides removed ones with `LogicHidden`, as for props.
- Not culled: the 3D skybox (drawn by the sky camera only where the
  camera's leaf sees sky), physics props and their shadows (they move),
  characters, runtime decals, particles.
- Areaportals: the BSP's areas (leaf lump), areas lump (20), areaportals
  lump (21) and clip portal vertices (41) give each leaf an area and each
  func_areaportal/func_areaportalwindow an opening between two areas
  (`vis::MapAreas`). Logic closes a portal while its linked door
  (`target`) is fully closed, or by Open/Close/Toggle when unlinked
  (`vis::AreaPortalStates`); a portal with glass or a grate in its
  opening is never closed (`see_through`). Windows (func_areaportalwindow):
  their brush (`target`) draws with an alpha rising from
  `TranslucencyLimit` at `FadeStartDist` to opaque at `FadeDist` from the
  opening (`vis::fade_windows`; hidden at alpha 0, blended in between),
  and past `FadeDist` the portal closes behind the opaque brush. Windows
  without a brush stay open.
  Each frame `vis::cull` floods from the camera's area through open
  portals, each portal narrowing a screen rectangle (its opening's
  projected bounds, intersected with the rectangle it was reached by;
  an opening crossing the eye's plane covers the screen), and draws only
  clusters in the PVS that have a leaf in a reached area. The water
  reflection camera floods whole areas (no rectangle). `r_portalsopenall 1`
  goes back to PVS only.
- Occluders (func_occluder): the occlusion lump (9) gives each occluder
  its polygons (indices into the vertex lump); the entity's
  `occludernumber` names it, `StartActive` and Activate/Deactivate/Toggle
  switch it (`vis::OccluderStates`). Each frame, while the main view has
  active occluder polygons in sight (wholly in front of the eye), static
  props and prop entities that stay put (`vis::Occludee`, their bounds;
  not world chunks: occluders are documented as hiding models, and
  stock occluders stick out of walls over windows, so hiding chunks
  showed holes) are hidden when
  the bounds lie wholly behind a polygon's plane and their screen
  rectangle lies inside its projected outline (conservative: no merging
  of neighbouring occluders). Not while the water reflection draws.
  `r_occlusion 0` turns them off; `chimera_perf 1` shows how many parts
  they hide. de_aztec's occluders were compiled without polygons, so they
  do nothing there (nor in the game, presumably).
- LODs: static and skinned (animated) props and CS:S bodies pick a `.vtx` level by screen size (`render::lod`). Not done: per-leaf frustum culling. See tech-debt.

Prop fade distances: static props' `fademaxdist` (static prop lump) and
prop entities' `fademaxdist` key hide the prop beyond that distance from
its origin. Between `fademindist` and `fademaxdist` the prop's meshes are
dithered out (`vis::fade_band`: a Bevy `VisibilityRange` per mesh, a 4x4
screen-door pattern in `prop.wesl`/`prop_prepass.wesl`, and for opaque
props a dither-only fragment stage added to Bevy's depth-only prepass).
Dithering keeps props in the opaque passes (no blending, no sorting) at
the cost of a stipple instead of a smooth alpha. A negative `fademindist`
(or one not below the far distance) pops at `fademaxdist` as before.
Fades apply with `r_novis 1` too. Physics, animated and riding props
fade the same way (their `fademindist`/`fademaxdist` keys; the distance
is from the moving node, re-read every frame; test
`chimera/present/tests/it/prop_fade.rs`). Cost: one distance per fading
prop per frame, nothing else. Props with lower `.vtx` levels draw them by
screen size (`render::lod`, test `prop_lods.rs`).

### Checks

- `cargo test --features dev --test it map_vis::`: from every spawn and ~250 nav area centres
  per map (de_dust2, de_nuke, cs_office, cs_assault), 400 rays from eye height; every
  world surface a ray reaches (translucent ones on the way, then the
  first opaque one) must be in a potentially visible chunk, unless the ray
  passed through solid first (sky brushes, which the game draws as sky and
  we don't draw). `CHIMERA_VIS_FULL=1`: every nav area, 2048 rays.
- `refcmp vischeck`: renders with culling off and on must match. Results
  (de_dust2: 316 views, de_nuke: 311): a handful of views differ by a few
  to a few hundred pixels, all inspected: geometry visible through sky
  brushes and sub-pixel cracks (culling hides it, as the game would).
  Three real bugs it found are fixed: physics props settling differently
  between runs (`host_timescale 0` now freezes physics from the first
  frame), the first view rendering before shader pipelines were ready,
  and captures depending on the views before them (below).
- `refcmp ordercheck`: the same views captured in order and in reverse
  order (culling on) must match (up to 100 pixels): a view's capture must
  not depend on what was drawn before it. Three causes, fixed
  (2026-10-16, `render::blend_order`), all in how Bevy keeps each view's
  depth-sorted items (blended and transmissive) from frame to frame and
  sorts them stably by view depth: items at the same depth kept the
  order earlier frames left (de_nuke's door windows: two glass faces 5 cm
  apart, seen edge-on, at one depth; view `nav1627_90` differed by 16k
  pixels after `nav1627_0`); an item's centre is worked out only when it
  is queued (it comes into view, or its mesh or material changes), so an
  entity that moved since sorted by where it was; and our sprites, which
  add in the transmissive phase, summed in a different order where they
  overlap at one depth (cs_assault's rows of lamp glows: up to 28k pixels
  off by one or two levels). Map overlays and infodecals were blended in
  the alpha-mask phase, which orders batches by pipeline id, made in the
  order the camera first saw each kind, so overlapping decals stacked
  differently (de_nuke `spawn36_90`'s gravel overlays, 930 pixels). Now
  each item's centre is taken again every frame, ties go
  farther-from-the-eye first, then by entity, and map decals draw first
  among blended items, in the map's order. de_nuke before: ordercheck 9
  of 311 views differ (worst 15997 pixels), vischeck 21 differ (worst
  15997, over the limit); after: ordercheck 0, vischeck 12 differ, worst
  644 (the sky-brush views): it passes. cs_assault: ordercheck 13 of 312
  views differ (worst 28190) before the glow fix, 9 of 316 after (worst
  50 pixels: `nav1847_90` by one level, `nav1567_180` 27 pixels on a
  rail's highlight; not found yet, under the tolerance).
- cs_assault's vischeck fails (57 of 316 views differ after the fixes
  above, the four `pvs_*` views included; 56 of 312 in its first run;
  worst `nav1371_90` on the roof, 33.7k pixels: the water tower, rooftop
  props and buildings seen over the walls; then `nav1371_180` 22.0k, `nav2507_180` 20.2k, the T spawns looking west
  14-18k). Not a culling bug, from the map data (2026-10-16): every static
  prop's clusters here cover the leaves the map's static prop lump lists
  for it (all 561; the same on de_dust2, cs_office, cs_compound, de_port,
  and 675 of 676 on de_nuke: one car's lump lists a leaf next to its
  box), so the game's PVS can't show a prop ours hides; every culled prop
  seen in these views is seen along lines that cross solid leaves (sky
  brushes over the walls), which vvis's PVS doesn't see through and which
  the game draws as sky; and the ray check above passes on cs_assault
  with every nav area (1562 eyes, 2048 rays). The 3D skybox isn't culled,
  and prop fades apply with culling off too. Whether CS:S shows the water
  tower from the roof is capture C27 (docs/plans/active/parity-gaps.md;
  refcmp views `pvs_*` in tools/refcmp/cs_assault.toml).

## Baseline and results

RTX 3080, Linux, 1280x720 offscreen, dev box under other agents' build
load. Frame ms averaged over the views of each map's refcmp file.

Playtest build, de_dust2 (32 views), two interleaved runs each at low
machine load. Frame ms is wall time; CPU ms is process CPU per frame over
all threads (Bevy's worker threads included); GPU ms from timestamps.

| de_dust2 | frame ms (avg) | CPU ms | GPU ms | meshes drawn | triangles drawn |
|---|---|---|---|---|---|
| before (merged per material) | 5.8 / 6.2 | 12.0 / 12.7 | 0.27 | 552 | 108k |
| chunks 512 units + PVS | 6.9 / 4.5 | 13.3 / 10.0 | 0.26-0.31 | 718 | 66k |
| chunks 1024 units + PVS | 9.2 (loaded) / 4.7 | 10.2 | 0.26 | 575 | 67k |
| chunks 4096 units + PVS | 5.2 / 4.3 | 11.3 / 9.6 | 0.25 | 427 | 72k |

After merging main (water with its reflection camera, movers), playtest
build, each map's refcmp views (de_nuke 27, cs_office 23), runs taken
under heavy machine load (load 8-27): read the ratios, not the absolute
times.

| map | build | frame ms avg | CPU ms | GPU ms | meshes drawn | triangles drawn |
|---|---|---|---|---|---|---|
| de_nuke | before (merged) | 58 / 101 | 77 / 107 | 3.0 / 6.9 | 1623 | 595k |
| de_nuke | 512 + PVS | 32 / 42 | 49 / 63 | 1.9 / 3.4 | 1027 | 157k |
| de_nuke | 4096 + PVS | 27 / 52 | 46 / 64 | 1.6 / 3.7 | 652 | 162k |
| cs_office | before (merged) | 45 / 57 | 65 / 67 | 2.1 / 3.5 | 1075 | 303k |
| cs_office | 512 + PVS | 28 / 47 | 45 / 55 | 1.1 / 2.4 | 835 | 150k |
| cs_office | 4096 + PVS | 34 / 62 | 46 / 57 | 1.5 / 2.0 | 634 | 154k |

The GPU does little (a 3080 draws dust2 in a quarter of a millisecond):
frames are CPU bound, by per-entity work, so culling pays by drawing fewer
entities as well as fewer triangles; chunks that are too small add
entities. 512 and 4096 units measure about the same; 512 stays the
default (finer culling, and what vischeck and the ray tests checked).
`CHIMERA_CHUNK_SIZE=<units>` overrides it for measurements.

Areaportals (playtest build, `refcmp bench`, load 14-20 on 12 cores, so
frame times swing 2x between runs; back-to-back runs with and without
`+r_portalsopenall 1`):

| map (views) | areaportals | frame ms avg (runs) | meshes drawn | map parts drawn (mean) |
|---|---|---|---|---|
| de_nuke (27) | on | 7.6 / 7.1 | 548 | 723 |
| de_nuke (27) | open all | 16.2 / 8.8 | 548 | 726 |
| cs_italy (6) | on | 8.2 | 412 | 539 |
| cs_italy (6) | open all | 7.6 | 412 | 545 |
| cs_office (11) | (has none) | 21.2 | 380 | 583 |

At these views the gain is in the noise: their doors are open or out of
view, the PVS already culls most of what lies beyond, and what remains
beyond an opening but outside its screen rectangle is mostly outside the
frustum anyway (meshes drawn don't change). The win is behind shut doors
(cs_militia: 54 map parts behind its front door, cs_assault: 15; see
`games/cs_source/tests/it/heavy/map_areaportals.rs`). `refcmp vischeck` on de_nuke lists the same
15 views, pixel for pixel, with and without `+r_portalsopenall 1`, so none
come from areaportals; one of them, nav1627_90 (16k pixels inside the
glass of the door beside the A site door), is over vischeck's 0.5% limit.
It isn't culling: rays from that eye through the glass (a scratch test
like `map_vis`) reach no culled chunk or prop, and capturing the view
alone gives the same image with and without culling (equal to the
culled run's). Capturing nav1627_0 then nav1627_90 without culling
reproduces the 15997 pixels, also with game time running: something the
previous view drew changes how the glass draws next (backlog).

Occluders (playtest build, `refcmp bench` over each map's 24 spawn views
in `tools/refcmp/`, two back-to-back pairs with `+r_occlusion 1` and `0`;
load 8-15 on 12 cores, so frame times are noise; the counts are exact):

| map | occluders | frame ms avg (runs) | meshes drawn | triangles drawn | map parts drawn (mean) |
|---|---|---|---|---|---|
| cs_assault | on | 9.9 / 8.2 | 364 | 37k | 778 |
| cs_assault | off | 6.9 / 8.8 | 369 | 39k | 784 |
| cs_compound | on | 32.0 / 19.2 | 439 | 29k | 1149 |
| cs_compound | off | 14.7 / 21.1 | 458 | 40k | 1168 |
| de_port | on | 14.1 / 19.6 | 1162 | 69k | 3192 |
| de_port | off | 14.9 / 15.2 | 1162 | 69k | 3192 |

Props only (see above). cs_compound's spawn views look along occluded
walls (27% fewer triangles: the hidden props are detailed); de_port's
occluders guard views its spawns don't have; de_aztec's have no
polygons. A first version also hid world chunks (cs_compound: 327
meshes, 28k triangles) but showed holes through windows in walls whose
occluder brushes stick out of them (`refcmp vischeck` on cs_assault and
cs_compound, up to 73% of a view). With props only, `refcmp vischeck`
(unculled run: `r_novis 1`, `r_occlusion 0`) passes on cs_compound (19
of 320 views differ, at most a few hundred pixels) and de_port (2 of
312); cs_assault differs with occluders off too (backlog).

## Many brush entities (mg_lego_multigames_v2)

A community minigame map (BSP v20, 578 func_breakable, 170 func_door, 937
brush models, 32k triangles, reflecting water). Looking into its room of
breakable blocks (`tools/refcmp/mg_lego_multigames_v2.toml`, view
`blocks_room`) it ran at 11-17 fps. Text readouts (`chimera_perf_log 1`,
window 1920x1080, playtest build):

| | fps | frame ms | main world ms | GPU ms | meshes drawn | vis |
|---|---|---|---|---|---|---|
| before | 14 | 70-72 | 12.2-14.2 | 4.6 (bin_unpacking 4.2) | 1964/4737 | off (2539 parts all drawn) |
| after | 60 (vsync) | 16.7 | 4.2-6.1 | 0.5-0.9 | 1767/4737 | cluster 1088, 626/3306 parts |

Root causes, from a trace of that view (`tracesum`, 200 frames):

- Every brush entity's mesh had a material asset of its own (a mesh per
  material per entity: 2209 WorldMaterials for 14 distinct materials), so
  nothing batched: each mesh its own bin, bind group and draw. Render
  world 59 ms a frame: `write_binned_instance_buffers` 13.4 ms (opaque)
  + 5.9 ms (prepass), `prepare_preprocess_bind_groups` 6.5, `unpack_bins`
  6.3, `queue_submit` 6.9. Now meshes with the same material description
  share one asset (`spawn_map` keys them by the material's full
  description and the skybox flag): 14 assets, render world about 5 ms
  plus waiting for vsync.
- Visibility culling was off: the map has visibility, and the camera's
  cluster was found, but the water reflection camera (active whenever a
  reflecting volume lay anywhere below the eye) had its mirrored eye in
  solid, which drew everything. Fixed as above (reflection only for water
  in sight; its culling from its surface's clusters).
- Brush entities were never culled; now by their bounds' clusters.
- The main world paid for the slow frames: at 15 fps each frame ran four
  fixed ticks (logic, movement, physics). In the logic bridge,
  `sync_movers` looked each mover's node up in a list (quadratic in
  movers) and rebuilt and re-inserted every mover's `MovingSolid` brushes
  each call (twice a tick); now a map lookup, and the brushes only when
  the mover moved or changed (`logic: sync_movers` span 0.19 ms a call).

`refcmp bench` (offscreen 1280x720, vsync off, playtest build), mean of
the file's 4 views, back-to-back runs at load 8-14 on 12 cores:

| build | blocks_room ms | mean frame ms | main world ms | GPU ms | meshes drawn (mean) |
|---|---|---|---|---|---|
| before | 59.4 / 59.5 | 25.8 / 25.7 | 5.9 / 6.0 | 1.97 / 1.81 | 1471 |
| after | 4.2 (one loaded run: 16) | 3.1 | 1.8 | 0.22 | 512 |

So about 240 fps at blocks_room and 320 fps on average at 720p here
(the GPU does under 0.5 ms; frames are CPU bound). No regression on the
stock maps (same runs, mean over each file's views; noise between runs
is larger than the difference):

| map | before frame ms | after frame ms | meshes drawn before -> after |
|---|---|---|---|
| de_dust2 (32 views) | 4.07 / 4.80 | 5.32 / 3.78 | 248 -> 248 |
| de_nuke (27 views) | 8.35 / 10.58 | 7.90 / 14.03 (loaded) | 548 -> 489 (its doors culled) |

Pictures are unchanged: refcmp report mean abs diff de_dust2 0.0308
(32 views), de_nuke 0.0250 (27), de_aztec 0.0409 (13), each view within
0.0005 of the shared captures; de_port's 24 views (reflecting water)
differ from the old build by at most 28 pixels. `refcmp vischeck` on the
lego map: 48 views, two differ by 1732 and 1965 pixels (0.2%):
blocks_room and blocks_room_right, a roof and walls seen through a
ladder grate (`boharox/simpsons/{sewerladder`, alpha-tested) in a world
brush. That brush was compiled as solid: rays from the eye through the
grate cross a solid leaf (no cluster) and every cluster behind it is
missing from the eye cluster's PVS (cluster 1088: 1299, 1300, 773 are
listed, then solid, then 746, 749, 750, 752 are not; every culled hit
lies behind solid). So the map's compiler let the grate block
visibility, and CS:S, which draws only the leaves in the PVS, doesn't
draw what lies behind it either. Our culling uses the map's PVS as it
is and computes no visibility of its own, so there is nothing for
see-through brushes to block or not; the difference stays (like sky
brushes, see "Checks").

At that view about 1770 meshes were still drawn: the wall of blocks is
~580 breakables, a mesh per material each.

Brush entities drawn merged (`map::merge`): while a brush entity is
where the map put it, whole and shown, its opaque and alpha-tested
meshes are drawn through combined meshes per material and 512-unit
chunk under the map's root of static parts, culled by the clusters their parts touch;
its own meshes stay spawned but hidden. When it moves (doors, trains),
breaks or is removed or turned off (`LogicHidden`: func_brush toggles),
or shows broken panes, its triangles leave the combined meshes (their
index buffers are rewritten without its ranges) and its own meshes are
drawn; back home and shown (a shut door, a round restart) it rejoins.
Blended meshes, the 3D skybox, water and areaportal window brushes
(whose alpha changes per brush) stay on their own. The logic doesn't
change brush entities' render mode or colour at run time; if it ever
does, that must split the entity out too. On the lego map 767 brush
entities go into 136 combined meshes; de_nuke 22 in 41; de_dust2 has
none.

`refcmp bench` (playtest build, 1280x720, vsync off, two interleaved
runs, `CHIMERA_MERGE_BRUSHES=0` for the unmerged runs, load 7-9 on 12
cores):

| map (views) | merged | blocks_room ms | mean frame ms | main ms | meshes drawn (blocks_room / mean) |
|---|---|---|---|---|---|
| lego (4) | no | 4.33 / 3.60 | 3.57 / 3.82 | 2.10 / 2.44 | 1768 / 512 |
| lego (4) | yes | 4.46 / 3.69 | 3.61 / 3.42 | 2.11 / 1.96 | 58 / 28 |
| de_nuke (27) | no | | 10.29 / 10.42 | 8.00 / 8.12 | 489 |
| de_nuke (27) | yes | | 10.39 / 10.15 | 8.07 / 7.86 | 486 |
| de_dust2 (32) | no | | 8.55 / 8.69 | 6.80 / 6.94 | 248 |
| de_dust2 (32) | yes | | 8.81 / 8.67 | 7.08 / 6.93 | 248 |

Draws at blocks_room fall 30-fold, frame times don't move: at 720p this
view was no longer bound by draws (GPU 0.2-0.3 ms) but by the main
world's per-entity work, which hidden meshes still cost a little
(visibility checks skip them early). Pictures are unchanged: captures of
the lego and de_nuke views with and without merging are identical
except 11 pixels in one de_nuke view (smoke); `refcmp vischeck` on the
lego map gives the same two views as before.

## Community maps' slow first views (mapsweep)

The map sweep's screenshots (docs/plans/active/community-maps.md) timed
six first views at 45-341 ms a frame. Traces (`--features profile`,
`tracesum`, 150 frames after 200) found two real costs; the rest were
the machine's load at the moment of the sweep.

- **mg_swag_multigames_v1** (341 ms): `avian3d::...::solve_swept_ccd`
  313 ms a frame. The map places about 300 weapons; each loose item had
  swept CCD with no velocity threshold, so every resting one computed a
  non-linear time of impact against everything its box touched (the
  world's colliders included) every tick: 19 ms a tick, and the slow
  frames ran up to 17 ticks each (Bevy's 250 ms catch-up), which made
  them slower still. Loose items now sweep only above 0.5 m/s or 6 rad/s
  (`map::loose`; slower, an item moves under a centimetre a tick, which
  the contact margin catches). `heavy::map_community::
  resting_placed_weapons_cost_no_sweeps`: 55 ms a tick before, 0.1-3.5
  ms after (the bound depends on load).
- **surf_demise** (98 ms): `propagate_parent_transforms` 9.6 ms a frame
  plus avian's `propagate_collider_transforms` 3.5 ms. 37k map parts
  (world chunks, 309k triangles, and static props) were children of the
  map's root, and a func_rotating under the same root changed a transform
  every frame: Bevy then rewrites the root's global transform and
  recomputes every child of it, once per propagation (several a frame:
  each fixed tick's physics propagates too). Parts that never move
  (world chunks, static props, merged brush entities, ropes, prop
  shadows) now sit under a root of their own beside the map's
  (`spawn_map`, "Map static parts"), which Bevy skips whole while
  nothing under it changes. After: both systems under 1 ms a frame.
- **gg_simpsons_arabtoon** (311 ms; 1,700 faces, no props),
  **surf_sacrifice** (102 ms), **mg_creative_multigames_v8_ns** (45 ms):
  no system stands out in their traces; at a quiet moment they run at
  the 60 Hz cap with both builds. Their sweep numbers were load spikes.
- **mg_kommando** (58 ms): about 40 ms with both builds at load 12-15.
  Sampling it (below) caught every thread waiting (worker threads
  parked, the main thread waiting in the executor, the render thread
  waiting): the frame waits on presentation or the GPU, not on CPU work.
  Not investigated further.

A main-thread schedule span with a lot of self time in `tracesum`
(kommando's `PostUpdate`, 15 ms) is the main thread waiting for worker
systems or being descheduled under load, not work of its own: check the
workers' spans inside the same window first. CPU sampling without `perf`:
`ptrace_scope` 1 lets a parent trace its child, so run the dev build
(debug info) under `gdb -batch` with a command file of `thread apply all
bt` / `continue` pairs and send the child SIGINT once a second.

Old (before these fixes) and new playtest builds, interleaved, each
map's first spawn, 600 frames at 1280x720, `mat_vsync 0` (60 Hz is the
compositor's cap here), the last `chimera_perf_log` line; two rounds at
different machine loads:

| map | round 1 old | round 1 new | round 2 old | round 2 new |
|---|---|---|---|---|
| mg_swag_multigames_v1 | 75.1 (load 4) | 16.8 | 309.2 (load 14) | 52.3 |
| gg_simpsons_arabtoon | 16.7 | 16.7 | 17.2 | 17.5 |
| surf_sacrifice | 16.7 | 16.7 | 55.0 (load 15) | 50.0 |
| surf_demise | 16.7 (load 2) | 20.8 (load 10) | 252.7 (load 14) | 27.5 |
| mg_kommando | 42.6 (load 12) | 41.6 | 56.7 | 38.8 |
| mg_creative_multigames_v8_ns | 19.5 (load 12) | 27.0 | 20.3 | 17.0 |

Frame ms. Under load the old builds' extra per-tick cost multiplies
(more ticks per frame); the new ones stay near the cap. Pictures:
`refcmp` captures of de_dust2 (32 views) and de_nuke (27) with both
builds report the same metrics (one view's sharpness ratio 0.652 vs
0.653); 17 and 26 captures are identical, the others differ in at most 43
pixels (particles, glows).

## Cheap wins found

- Glow sprites: the occlusion test borrowed the sprite material mutably
  every frame, which re-prepares it for the GPU even when nothing changed;
  now only when the colour changes. Culled glows skip their five rays.
- Particles and dust: each particle material's mesh and each dust volume's
  mesh was rewritten every frame, even with nothing in it (13 mesh assets
  modified per frame in a dust2 bot match). Any modified mesh makes every
  mesh entity re-check its pipeline (`AssetChanged<Mesh3d>` in each
  material's `check_entities_needing_specialization`) and the mesh is
  uploaded again. Now an empty mesh is written once, and hidden dust
  volumes wait (2-5 per frame left, from live particles).
- Contacts nobody uses: avian computed contacts every step for each
  character capsule and mover against the world and static props, though
  two non-dynamic bodies never get constraints. The collision hooks now
  drop pairs without a dynamic body unless a side reports contacts
  (`map::contact_filter`).
- Redundant writes: the sky and view-model cameras got their target,
  tonemapping and projection re-inserted every frame, the radar its
  material; now only on change.

Measuring here: the dev box runs other agents' builds (load 4-16 on 12
cores), which moves frame times by 2-4x between runs, more than these
changes. `chimera_perf 3` counts (above) are exact. Before/after frame
times on a quiet machine are still to be taken.

Tried and dropped: one physics substep while no dynamic body is awake
(characters and movers are kinematic with zero velocity). It made
sleeping ragdolls wake again every step (`bullets_push_a_dead_body`
failed), so substeps stay at avian's 6. Props, movers and the map's collision
as hierarchies of their own instead of children of the map's root (avian
walks every tree holding a collider each tick in
`propagate_collider_transforms`, about 0.3 ms per tick on dust2 under
load): a dust2 physics prop then settled differently
(`physics_props_settle_and_get_pushed`), so it is left for later. (Parts
that never move, static props included, now have a root of their own:
see "Community maps' slow first views".)

## Frame time pass (executors, dynamic meshes, posing, prop shadows, resolution)

Goal: frame rates near a 240 Hz monitor's at 4K. Measured with
`refcmp bench` (playtest build, vsync off) on de_dust2 (six `survey_*`
views, static and with rounds and 10 bots), de_nuke (27 views),
cs_office (23), mg_lego_multigames_v2 (4) and mg_kommando (8 views at
two spawns), plus traces of a `--features profile` build.

### What cost the most

- **A dust volume's mesh**, rewritten every frame with its visible
  motes (one per map on dust2): Bevy's mesh allocator frees and
  re-allocates a modified mesh in a buffer shared with other meshes,
  and with a size that changed every frame that cost 3.5-4.5 ms of the
  render world per frame (`allocate_and_free_meshes`, every frame of a
  trace at `floor_mid`), plus knock-on costs in
  `prepare_clusters_for_gpu_clustering` (2.2 ms) and
  `prepare_assets<GpuImage>` (0.8 ms). It came and went between runs
  (one dust2 run of the old build: 14.7 ms frames at 5.1 ms main
  world; the next 4.9 ms), presumably depending on whether the new size
  fit the hole the old one left. Dust and particle meshes are now padded
  to power-of-two sizes with degenerate triangles
  (`render::dust::write_dynamic_mesh`): in the same trace 0.33 ms, 0.53 ms and
  0.17 ms.
- **The executor.** A frame runs about 1500 systems (main and render
  world: ~200 of ours, ~180 avian per tick, ~60 render-graph systems
  for each of four cameras, ~100 asset systems), nearly
  all tiny (ours: 0.01-0.04 ms each). Bevy's multi-threaded executor
  hands each to a worker and wakes the next; that cost more than the
  systems, and much more when the machine is busy (main world 4.3 ms at
  load 3, 10-30 ms at load 10-20 for the same views). Every per-frame
  schedule (First..Last, the fixed ones, `Render`, `RenderGraph`,
  `Core3d`, `Core2d`) now runs on Bevy's single-threaded executor
  (`main.rs`, with `mashup::single_threaded_schedules` for the main
  world's, which the dedicated server uses too); systems that iterate many entities still spread over the
  task pool themselves (transform propagation, visibility), and the main
  and render worlds still run side by side. `CHIMERA_EXECUTOR=multi`
  brings back Bevy's default. On macOS the render schedules keep Bevy's
  executor: the window surface must be created on the main thread, and
  only the multi-threaded executor hands such systems to it.
- **Bodies nobody sees** were posed every frame: the local player's
  hidden body (51 joints written per frame on dust2) and bots out of
  view. `map::pose_bodies` now poses a body every frame only while some
  camera drew one of its meshes the frame before, else every 4th frame
  (`UNSEEN_POSE_INTERVAL`); hitboxes don't read the joints (they pose
  from `SkeletonPose` every tick), and body culling uses the mesh's own
  bounds, not the joints. Ragdolls and body turns write only real
  changes (a ragdoll at rest no longer dirties ~50 joints a frame).
  `map::pose_tests` checks it.
- **Prop shadows of physics props that never sleep.** Seven props on
  cs_office rocked by a tenth of a millimetre and a hundredth of a degree
  forever (avian never put them to sleep; `settle_props` does since), so every frame
  `update_prop_shadows` redrew their shadows: silhouettes (0.8 ms for a
  1074-triangle desk in a 256-texel cell), meshes, and then the whole
  shadow atlas converted from floats to bytes and uploaded again: 16 ms a
  frame (timed in the system). Now a prop's shadow is redrawn only once
  it has moved by a quarter of one of its shadow texels since the last
  redraw (`shadows::moved_visibly`; a desk's texel is 7.5 mm, and the
  receiver blurs over several), and a redrawn cell writes only its part
  of the atlas image, which stays in the main world for that: 1.0 ms a
  frame (2.7 ms when all seven redraw). mg_kommando has 74 props a frame
  riding its train and rotators, which do move. A redrawn shadow
  rewrites its own mesh asset in place (a new `Mesh3d` handle each frame
  made Bevy re-specialize the entity, and under the single-threaded
  executors a moving prop's shadow was never drawn until it rested;
  `chimera/present/tests/it/prop_shadows.rs`).
- **Resolution** barely matters on the dev box's RTX 3080: GPU time for
  the survey views 0.37 ms at 1280x720, 0.40 at 1920x1080, 0.49 at
  2560x1440, 0.53 at 3840x2160 (`--view-size`), frame times unchanged
  (CPU bound). At 4K the largest pass is `msaa_writeback` (0.27 ms of
  1.0 at `survey_b_site`: the sky and view-model cameras draw over the
  main one, each copying the resolved image back into the multisampled
  one), then the opaque pass (0.19). `mat_antialias` (Options > Video,
  0/2/4, default 4) sets every camera's MSAA; 4K with it off: 0.45 ms
  GPU instead of 0.52. A render scale wasn't worth adding until a GPU is
  the limit (backlog).

Tried and dropped: disabling unused Bevy plugins (animation, scenes, UI
widgets): no measurable change; glTF and picking can't go (the PBR and
UI plugins need them). A mesh kept alive to stop the dust's buffer
emptying (the first guess at the allocator cost) changed nothing.

### Before and after

`refcmp bench`, playtest build, 1280x720, vsync off; old = the build
before this pass, multi = this pass with `CHIMERA_EXECUTOR=multi`, new =
this pass. The builds ran interleaved, several rounds each, at load 6-25
on 12 cores (other agents building), so single runs swing 2-3x: medians
and (in brackets) minimums over the rounds. Frame = wall time per frame,
main / render = each world's time, CPU = process CPU over all threads.

| map (views, rounds) | build | frame ms | main ms | render ms | CPU ms |
|---|---|---|---|---|---|
| de_dust2 (6, 6) | old | 28.7 (9.5) | 26.6 (8.2) | | 28.5 (18.3) |
| | multi | 22.4 (9.1) | 20.6 (8.0) | 18.0 (7.0) | 28.1 (17.0) |
| | new | 14.6 (5.4) | 12.3 (4.5) | 11.7 (4.8) | 17.8 (11.2) |
| de_dust2 bots (6, 4) | old | 34.2 (18.3) | 32.6 (17.2) | | 38.3 (27.6) |
| | multi | 38.4 (12.4) | 36.1 (11.3) | 20.6 (9.8) | 38.5 (24.1) |
| | new | 25.7 (9.3) | 23.5 (7.9) | 17.7 (7.9) | 26.9 (19.1) |
| de_nuke (27, 3) | old | 11.8 (11.3) | 9.3 (8.7) | | 22.1 (21.7) |
| | multi | 13.2 (11.2) | 10.3 (8.6) | 11.8 (9.3) | 26.3 (22.1) |
| | new | 10.0 (8.0) | 6.6 (5.6) | 8.5 (6.5) | 18.2 (14.7) |
| cs_office (23, 3) | old | 30.2 (29.5) | 29.2 (28.4) | | 38.4 (37.9) |
| | multi | 24.0 (13.0) | 23.2 (12.5) | 12.0 (6.8) | 36.0 (25.7) |
| | new | 35.6 (12.7) | 33.7 (12.2) | 21.2 (6.8) | 36.8 (22.3) |
| mg_lego_multigames_v2 (4, 4) | old | 7.6 (5.7) | 5.5 (3.6) | | 15.2 (13.5) |
| | multi | 7.3 (5.8) | 5.3 (3.8) | 6.5 (5.2) | 15.3 (13.6) |
| | new | 5.7 (4.7) | 3.4 (2.7) | 5.0 (4.0) | 11.0 (8.5) |
| mg_kommando (8, 3) | old | 13.5 (11.6) | 13.0 (11.2) | | 26.5 (22.2) |
| | multi | 19.5 (11.3) | 18.9 (10.9) | 9.6 (5.4) | 34.1 (21.8) |
| | new | 19.9 (10.4) | 18.7 (10.0) | 11.2 (4.7) | 27.7 (17.0) |

The bot rounds differ run to run (who is alive, where), so read that row
loosely. With the prop shadow fix (then + shadows; old and new as above,
three interleaved rounds at load 17-33):

| map (views, rounds) | build | frame ms | main ms | render ms | CPU ms |
|---|---|---|---|---|---|
| cs_office (23, 3) | old | 19.7 (15.1) | 19.0 (13.5) | | 37.0 (26.1) |
| | new | 13.1 (12.9) | 12.5 (12.0) | 6.8 (6.5) | 22.1 (21.2) |
| | + shadows | 9.8 (5.7) | 6.1 (2.7) | 8.6 (4.6) | 16.8 (9.3) |
| mg_kommando (8, 3) | old | 28.4 (11.5) | 27.2 (11.1) | | 35.2 (22.5) |
| | new | 33.2 (11.6) | 31.5 (11.1) | 17.1 (5.8) | 33.7 (19.9) |
| | + shadows | 13.9 (7.8) | 10.6 (5.1) | 11.4 (6.7) | 20.2 (14.8) |
 Pictures are unchanged: refcmp mean abs diff de_dust2 0.0308
(32 views), de_nuke 0.0248 (27); dust2 captures of the old and new
builds differ by at most 662 pixels a view, less than two runs of the
old build differ from each other (dust motes, glows: up to 845).

Left: per-tick work matters once frames are slow (2-3 ticks a frame):
the logic bridge's sync (`logic: sync to the ECS`, ~0.27 ms a call on
cs_office, twice a tick) and transform propagation twice a tick (ours
after restoring eased transforms, and avian's); these measurements
predate the static parts' own root (above), which keeps the world out of
that propagation. Physics props that never sleep: see "Props that
never sleep" below. On a quiet machine (load 3) the old
build drew dust2's 32 views at 4.9 ms a frame (main world 4.3): the
per-frame cost a 240 Hz monitor (4.2 ms) has to beat; new numbers on a
quiet machine and on the Windows PC are still to be taken.

## Props that never sleep (2026-10-16)

Counted with `physics: N dynamic bodies, M awake` (`chimera_perf`) and
`bodies_awake` per frame in `refcmp bench` (both new), and headless 4-20 s
after loading. The "seven props on cs_office" above slept since
`settle_props` (2026-10-14): 0 awake in the app at every view. Still
awake for good, all `prop_physics_multiplayer`:

- **de_inferno potted plants 666 and 654** wobbled by up to ten degrees
  at 1.5-2 rad/s forever. Their floor brushes' top faces have six and
  nine corners, and parry's contact manifolds between convex polyhedra
  clip only a face's first four corners (a TODO in parry's
  `ConvexPolyhedron::local_support_feature`): the strip of floor kept
  missed the pot, one contact point was left, and it hopped around the
  octagonal base every tick. The pot's own base (eight corners) had the
  same problem. Fix: `map::collision::contact_hulls` cuts props', the
  world's and brush entities' hulls along diagonals of their wider faces
  into pieces whose faces have four corners at most (the same solid;
  pieces are checked before parry builds them, since a hull whose merged
  faces don't have simple rims makes parry's `from_convex_mesh` loop
  forever). The plants now sleep about 1 s after loading and again 1.3 s
  after a bullet push.
- **cs_italy glass jugs 511, 512 and de_inferno milk carton 761** were
  placed sunk 5-17 cm into a static prop (a cabinet's base, a junk pile):
  pushed out at avian's overlap speed every tick, they jittered and
  crawled, and with the cut hulls one fell through the floor and kept
  falling. Fix: `settle_props` puts a prop the map placed more than 1 in
  deep into something that doesn't move to sleep in its first 0.1 s
  (`WedgeCheck`), and one that stays that deep for 1 s later (after a
  shot) too. A shot still wakes and moves them.

A knocked de_dust2 oil drum (375) now tips over on the ramp near (34, 3,
-70) and rolls about 19 m before it sleeps (18 s; the test allows 24).

`refcmp bench`, dev build, same views, before / after (load 15-30, so
read the counts and instructions, not the times):

| map (views) | bodies awake, mean (max) | instructions per tick |
|---|---|---|
| cs_italy (6) | 2.0 (2) / 0 (0) | 7.27M / 6.77M |
| de_inferno (29) | 0.29 (2) / 0 (0) | 11.38M / 11.04M |
| cs_office (23) | 0 / 0 | |
| de_nuke (27) | 0 / 0 | 11.15M / 10.75M |

(de_nuke had nothing awake either way: about 4% is run-to-run spread.)
`heavy::map_prop_settle::map_settles_with_no_body_awake` checks that
de_inferno, cs_italy, cs_office and de_nuke have no body awake 10 s in,
and that the plant and a wedged jug still wake, move and sleep again
when shot.

Hierarchies: avian's `propagate_collider_transforms` walks every
collider under a root each tick, whether it moved or not: 240 (de_dust2),
429 (cs_office), 498 (cs_italy), 580 (de_inferno) and 712 (de_nuke)
colliders, about two thirds of them static props under "Map static
parts". Bevy's propagation skips trees that didn't change and visits
only the moving prop's own subtree. At these counts neither is worth
restructuring (props as roots of their own changed how a dust2 prop
settled before, see "Cheap wins found"); a map with thousands of static
props with collision would be the case for it.

Ragdolls and broken props' pieces (2026-10-16). Ragdoll hulls and gib
hulls now go through `contact_hulls` too: a CS:S player's ragdoll has 7
of its 16 bodies cut (16 hulls become 29 pieces). That changed nothing
measurable: killed at 10 spawns each on de_dust2 and de_inferno, every
ragdoll slept 5.25-5.7 s after dying with cut hulls or not, and in both
a limb or two kept jittering against the floor (over 10 rad/s at times,
pressed 1-3 cm in) until then: something else (joints against contacts),
not the four-corner clipping. What did matter: the forced sleep
(`ragdoll_sleepaftertime`, 5 s still) zeroed the bodies' velocities
marked as changed, which avian takes as a push and wakes the body again,
so on real maps no ragdoll ever slept (de_dust2: still awake 9 s in);
it now writes them unmarked as `settle_props` does. Gib pieces have no
hulls on CS:S yet: piece and chunk models load without their `.phy`, so
their bodies are boxes (four corners a face). `heavy::map_debris_settle`
checked that the 20 ragdolls sleep within 7 s (now: see below) and
twelve de_inferno props' pieces within 5 s.

Broken props' pieces collided with each other (2026-10-17). A broken
prop's pieces start where they were in the whole prop, their boxes
overlapping, and collided with each other: they piled up, and a piece
resting only on others (de_inferno crate 121's piece 4: touching two
pieces and nothing else) rocked at 1-2.5 rad/s, which kept the whole pile
(one island) awake. How long it lasted hung on the solver's order: with
110 more world brushes restored (none near the crate) the twelve props'
pieces slept in 1.0-15.1 s instead of 1.2-6.9 s (crate 113 3.99 s,
5.7 rad/s; 121 15.1 s). Source's break pieces are debris (prop_damage.md
7.3: `debris` > 0, or a `fadetime`; a block with neither taken as debris,
inferred), and debris doesn't collide with debris (ragdolls.md 4): pieces
now keep to the world and props (`MapBreakPiece::debris`; chunks always),
filtering out the ragdoll layer, so they no longer collide with other
pieces or ragdolls either. All twelve sleep in 0.8-2.9 s, with no
awake piece over 0.05 rad/s after 3 s and no two of a prop's pieces
touching, whichever order the props are broken in (map order, reverse,
every other one first; the test runs all three). Ragdolls' limb jitter
is something else: different ragdolls already didn't collide and a
ragdoll's own bodies collide only for its `.phy` pairs. Pieces loaded
with their `.phy` hulls (tried again with debris apart) no longer pile
up, but five of the twelve props have a light piece still rocking alone
on the floor 5 s on (0.3-1.9 rad/s), so they stayed boxes (now hulls:
see below).

Ragdoll limbs jittered against the floor (2026-10-17). Ragdoll joints
were avian XPBD constraints solved after the contacts in each substep:
rigid position corrections whose velocity (Δx/h) stayed in the bodies,
while the contacts are soft and relaxed. Wherever a limb's joints and
the floor disagreed (a hand held 4 cm into a displacement by its arm,
an arm folded at two limits) they undid each other every substep, and
the bodies kept 0.3-3 m/s and 2-16 rad/s while not moving at all: with
the forced sleep off, 1 of 20 ragdolls slept within 8 s, with limits
or self-collision off still none. The joints are now solved in the
contacts' own passes (warm start, biased soft pass, relax; ball and
socket as one 3x3 row, each Euler angle's limits as rows of their own
along the dual axes, speculative before the limit), twice as stiff as
the contacts as Box2D makes them. That alone let 31 of 40 sleep within
8 s. What was left was mostly the 1 kg clavicles trembling at their
limits at up to 7 rad/s: avian's gyroscopic torque (it keeps the
angular momentum's size, not the energy) fed them; ragdoll bodies now
turn without it. And a ragdoll whose bodies all keep within props'
stillness bounds (1 in, 5 degrees) for 1 s sleeps, as props do.
`heavy::map_debris_settle` with the forced sleep off, 60 ragdolls
(10 spawns on de_dust2 and de_inferno, three orders): all asleep in
1.9-7.0 s, median 3.2 s, 51 by 5 s; after 3 s no body over 1.74 rad/s
(a hand swinging from its wrist, rotdamping 1, dying down by 7 s).
`ragdoll::a_ragdoll_kept_moving_is_put_to_sleep_after_five_seconds` (a
toy scene) covers the forced rule, which the map test turns off.

Broken props' pieces with their hulls (2026-10-17). Piece and chunk
models now load with their `.phy` (prop_damage.md 7.3: pieces are
physics objects), and pieces sleep by props' stillness rule (1 in, 5
degrees for 1 s; `prop_physics::SettlesLikeProp`). The ragdoll fix's
other ingredient, no gyroscopic torque, was tried and left out: it
doesn't help pieces. `heavy::map_debris_settle`, twelve de_inferno props
in three break orders (36 breaks), time until every piece sleeps:

| pieces | asleep in | breaks still awake at 5 s |
|---|---|---|
| boxes (before) | 0.8-2.9 s | 0 |
| hulls | 1.5-4.9 s | 14 (light pieces rocking alone, 0.3-1.9 rad/s) |
| hulls, no gyroscopic torque | 1.8-9.2 s | 12 still awake at 15 s (up to 1.6 rad/s) |
| hulls, stillness sleep (kept) | 1.5-4.2 s, median 2.4 s | 0 |
| hulls, both | 1.7-4.8 s | 0 |

The rocking pieces are curved pot shards and thin planks that keep
rolling slowly on their curve, above avian's sleep threshold but over
5 degrees a second, so only the stillness rule's time stops them once
they slow down.

## Fewer cameras per frame (2026-10-16)

The view-model camera ran its passes every frame: clearing its depth,
its (empty) opaque, transparent and post-processing passes, and the copy
of the resolved image back into the multisampled one
(`msaa_writeback`), with nothing on its layer whenever no view model is
drawn (spectating in free look, dead, in menus, `r_drawviewmodel 0`, a
scoped AWP, no weapon). `render::view_model::view_model_cameras_draw`
now turns it off unless something visible is on `VIEW_MODEL_LAYER` (the
view model, a shell, the under-water overlay, a muzzle flash), and the
post-processing that goes on the last camera drawing the view (night
vision, colour correction) then goes on the world's camera; they are
placed after it in PostUpdate, so the frame it turns on or off has them
right. In HDR (`mat_hdr_level` 1-2) it always draws: its auto exposure
keeps a state per camera. Alive with a weapon nothing changes.

`refcmp bench`, de_dust2 (32 views; refcmp hides the view model), dev
build, three interleaved rounds at load 10-16; p50 over all frames,
median of the rounds (range):

| | cameras drawing | render-world instructions / frame | GPU ms | render world ms |
|---|---|---|---|---|
| 1280x720 before | 4 | 15.48M (15.47-15.51) | 0.97 (0.92-1.03) | 4.46 (4.42-5.61) |
| 1280x720 after | 3 | 13.47M (13.40-13.55) | 0.84 (0.83-1.74) | 4.73 (4.21-6.77) |
| 3840x2160 before | 4 | 15.53M (15.48-15.67) | 1.60 (1.60-1.63) | 4.66 (4.43-4.79) |
| 3840x2160 after | 3 | 13.54M (13.50-13.60) | 1.31 (1.29-2.50) | 4.43 (3.56-4.69) |

So 13% fewer render-world instructions and 14-19% less GPU time a frame
whenever no view model is drawn (wall times move with the load). GPU
times before this were misread: Bevy reports each camera's run of a pass
as its own measurement under the pass's name, and `chimera_perf` and
`bench` took the latest (the view-model camera's empty passes: dust2's
"0.2 ms") or a moving average over the cameras; they now sum each
frame's (`perf::frame_mean`, `frame_latest`), and both show cameras
drawing (`cameras_drawing` in `bench.json`). Pictures: screenshots of
the old and new builds (de_dust2 with and without the view model, night
vision with and without, shooting with shells in the air, spectating;
de_port under water with and without; surf_demise's colour correction
with and without) differ by no more than two runs of the old build do
(0 pixels for spectating and the colour correction ones); dead, the
ragdoll lands differently (its hulls are cut, above) and the death
camera that looks at it with it; `refcmp
capture-ours` of `survey_b_site` matches to the pixel. de_port's
under-water overlay turned out not to show at all, before as after: it
was drawn, but its warp moves the screen by about a pixel and its
`$bluramount` was ignored (now a blur; `render::water`'s doc,
`chimera/present/tests/it/underwater_overlay.rs`); the camera's part
there is covered by `chimera/present/tests/it/view_model_camera.rs`.

## Back faces in the depth prepass (2026-10-17)

Main views get a depth prepass on maps with water (`render::water`;
also with `chimera_skyocclude 1`). Bevy's main pass culls back faces by
default, but its prepass pipeline culls nothing unless the material sets
a cull mode, and the world and prop materials only ever set one for
`$nocull` (none) and mirrored view models (front). So on those maps every
back face was drawn into the prepass's depth and then culled in the main
pass: where one lay in front of something (a one-sided face seen from
behind, the inner side of a house wall seen through its window), the
pixel kept the cleared colour, which composites to the sky (alpha 0) or
shows black. cs_militia's windows showed the sky instead of the boards
behind them; de_aztec's `cam2` a sky wedge across a beam. Now both
materials set the cull mode on every pipeline
(`render::prop_material::face_cull`): back faces culled except `$nocull`
materials, as Source does. Maps without water (de_dust2) never had a
prepass and are unchanged.

What was culled before and now: main pass, world and props: back faces
(Bevy's default; unchanged). Prepass: nothing before, now the same faces
as the main pass. Two-sided everywhere, before and after: `$nocull`
materials, ropes, sprites, particles, beams, detail sprites, dust
and steam, the sky occluder, func_breakable_surf's cracked and edge
pieces. Winding: the heavy test
`world_and_props_wind_their_fronts_counter_clockwise` (de_dust2: 27663 of
27685 world and 31027 of 31068 static prop triangles wind
counter-clockwise toward their normals) confirms the faces kept are the
fronts.

GPU time of the prepass (`chimera_perf 2`, `early prepass`, dev build,
three interleaved rounds of 900 frames each, mean of the last five
readouts; the shared GPU's clock moved totals 2x between rounds, so
compare within a round):

| view | prepass ms before | after | prepass / opaque pass before | after |
|---|---|---|---|---|
| de_aztec (-400 -900 -100) | 0.31, 0.33, 0.23 | 0.26, 0.23, 0.14 | 0.31-0.39 | 0.21-0.27 |
| cs_militia windows | 1.52, 1.54, 0.31 | 1.29, 0.57, 0.63 | 0.97-1.50 | 0.75-0.88 |
| de_port (700 2700 235) | 0.30, 0.32, 0.18 | 0.25, 0.14, 0.12 | 0.44-0.59 | 0.41-0.48 |

So the prepass costs about a fifth less relative to the opaque pass
(fewer fragments; the vertex work is the same). Frame times and
`refcmp bench` (de_aztec, de_port) moved within the load's noise (load
20-28). `refcmp report`: de_aztec mean abs diff 0.0405 -> 0.0402 (cam2
0.029 -> 0.026, cam1 0.019 -> 0.018), de_nuke 0.0242 and de_dust2 0.0309
unchanged; captures of de_inferno and cs_italy (no references) differ
from the old build's by at most 0.2% of pixels (moving water and
particles).

## Load-independent metrics

Every number in "Frame time pass" was taken at load 6-33, where single
runs swing 2-10x. What now measures besides wall time (`chimera/core/src/metrics/`,
`chimera/client/src/client/frame_metrics.rs`; OBSERVABILITY.md 3c says where each shows):

- **Thread CPU time** of the main world (between marks before `First` and
  after `Last`), the render world (before `ExtractCommands` to after
  `Cleanup`, on the render thread) and each tick (`FixedFirst` to
  `FixedLast`), from `CLOCK_THREAD_CPUTIME_ID` (Windows:
  `GetThreadTimes`, in 15.6 ms steps).
- **Hardware counters** (Linux, `perf-event2`, user space only, which
  `perf_event_paranoid` 2 allows): instructions retired, cycles, cache
  misses, branch misses, one counter group per thread (one `read` per
  sample). Unavailable elsewhere: the readouts say why.
- **Distributions** over the last 4096 frames and ticks: p50, p90, p99,
  p99.9, max, and "1% low" fps (1000 / the mean of the slowest 1% of
  frames).
- **Hitch log** (`chimera_hitch_ratio`, default 2): a frame over twice the
  median logs its main-world schedules and render sets by wall and CPU
  ms, ticks run, and with `--features profile` its spans by self time.
- **`perfgate`**: instructions per tick of fixed headless scenarios
  against a stored baseline ("Regression gate").

### How stable they are under load

`refcmp bench` (playtest build, 1280x720), the same build three times
per map at load 8-22 (`ALL` rows: every view's frames together; spread
= (max - min) / mean over the runs):

| metric | de_dust2 | de_nuke | cs_office (6 runs, load 8-15) |
|---|---|---|---|
| frame ms p50 | 3.85-15.36 (141%) | 4.62-15.51 (122%) | 5.03-19.60 (160%) |
| frame ms p99.9 | 15.9-49.1 (106%) | 8.2-42.1 (151%) | 13.7-100.6 (170%) |
| main CPU ms p50 | 102% | 105% | 2.44-7.90 (130%) |
| render CPU ms p50 | 78% | 75% | 4.44-9.85 (90%) |
| main instructions p50 (less ticks) | 29% | 11% | 8.5-15.2M (60%) |
| render instructions p50 / p99 | 0.6% / 0.5% | 2.0% / 0.9% | 1.0% / 0.8% |
| tick instructions (mean, main thread) | 12% | 6.6% | 5.7-6.6M (14%) |
| GPU ms p50 | 0.22-0.55 (97%) | 83% | 0.25-0.62 |

An earlier round of the same (three runs at load 20-33) gave render
instructions 0.3-0.8% and frame ms p50 27-87%.

What this says:

- **The render world's instructions per frame are the stable number**
  (under 1% between runs whose frame times differ 3-4x): compare them
  between builds. It runs everything on one thread.
- **The main world's instructions move with load** (5-30%): its
  parallel queries (transform propagation, visibility) hand chunks to
  the task pool and the main thread runs whatever share is left when
  the pool is busy, and frame-rate dependent work (particles, posing,
  interpolation) scales with the frame time. Its ticks add 2-3 per slow
  frame, so the bench also reports it less its ticks' instructions
  (`main_frame_instructions`, "main ins*"), which halves the spread.
  Main-thread tick counts (`tick ins`) have the same parallel-share
  problem; `perfgate` counts every thread instead.
- **CPU time** moves almost as much as wall time on this box (clock
  speed, caches, hyperthread siblings shared with other builds), and
  GPU time doubles at load 20 (timestamps include waits). Wall and CPU
  times are for comparing within one run.
- One frame time number to watch per run is the 1% low: dust2's run 1
  ran p50 3.85 ms (260 fps) but 1% low 44 fps.

### Baseline

`refcmp bench`, playtest build at 6b95f1c plus this work, 1280x720,
three runs; frame ms p50 / p99 / p99.9, 1% low fps, CPU ms p50, main
instructions less ticks per frame p50 / p99, render instructions per
frame p50 / p99, mean instructions per tick (main thread), GPU ms p50:

| map, run (load) | frame ms | 1% low | main CPU | render CPU | main instr | render instr | tick instr | GPU |
|---|---|---|---|---|---|---|---|---|
| de_dust2 1 (22) | 3.85 / 17.40 / 28.92 | 44 | 1.84 | 3.42 | 6.71M / 13.30M | 13.65M / 17.29M | 3.06M | 0.22 |
| de_dust2 2 (10) | 5.38 / 8.64 / 15.92 | 88 | 2.96 | 4.72 | 7.29M / 13.58M | 13.71M / 17.20M | 3.07M | 0.25 |
| de_dust2 3 (22) | 15.36 / 31.33 / 49.09 | 26 | 5.27 | 7.47 | 8.90M / 20.40M | 13.63M / 17.29M | 3.46M | 0.55 |
| de_nuke 1 (17) | 4.62 / 7.15 / 8.21 | 125 | 1.80 | 4.19 | 7.05M / 15.97M | 18.66M / 28.57M | 5.12M | 0.27 |
| de_nuke 2 (9) | 6.77 / 11.32 / 17.11 | 75 | 3.13 | 6.14 | 6.74M / 14.36M | 18.91M / 28.32M | 5.12M | 0.26 |
| de_nuke 3 (18) | 15.51 / 31.51 / 42.13 | 27 | 5.41 | 9.02 | 7.52M / 18.28M | 19.05M / 28.33M | 5.46M | 0.56 |
| cs_office 2 (8) | 19.60 / 55.91 / 96.45 | 14 | 7.90 | 9.85 | 15.21M / 35.91M | 18.66M / 31.44M | 6.58M | 0.62 |
| cs_office 3 (15) | 18.54 / 37.23 / 52.94 | 24 | 7.34 | 9.28 | 14.45M / 33.70M | 18.67M / 31.34M | 6.59M | 0.62 |
| cs_office 4 (9) | 5.30 / 51.82 / 100.61 | 13 | 2.95 | 4.68 | 9.00M / 28.56M | 18.55M / 31.33M | 5.92M | 0.27 |
| cs_office 5 (9) | 5.03 / 10.83 / 13.70 | 84 | 2.44 | 4.44 | 8.52M / 26.25M | 18.53M / 31.28M | 5.71M | 0.25 |

(Load = the 1-minute average when the run started; cs_office runs 4
and 5 with the final build, the others with marks between the render
sets, below.) Read the render instructions and the tick column across
builds; the main world's instructions only for large changes.

Marks between the render world's sets (each set's share in the hitch
log) were opt-in at first: with them on, two of five cs_office benches
(2026-10-09) quit on a wgpu validation error ("Indirect draw uses bytes
840..4700 using count 193 which overruns indirect buffer of size
4660"), and without them none of seven did. Extra systems reorder the
single-threaded executor's unordered systems, so the guess was an order
of Bevy's render systems that breaks indirect draws.

Rechecked 2026-10-15 (same Bevy, 0.19.1): it doesn't reproduce. With
the marks, 0 of 7 cs_office benches (dev build) and 0 of 8 (playtest
build) failed at load 17-44, and 0 of 6 with prop shadows redrawn with
a new mesh handle every frame as in that build (fixed hours later:
"Frame time pass"). The `Render` schedule's order with and without the
marks (`CHIMERA_SYSTEM_DUMP`, OBSERVABILITY.md) differs only in systems
that don't touch the indirect buffers: asset preparation of materials,
view preparation (TAA jitter, decals, depth usages), the phase sorts
among themselves, pipeline preparation, and the cleanup's
`reset_render_asset_bytes_per_frame`; batching, the phase buffers'
collection and `write_indirect_parameters_buffers` keep their order.
Bevy's ambiguity detection (`CHIMERA_AMBIGUITY=1`) reports one
unordered pair in `Render`, ours (`render_start` and
`apply_extract_commands`, harmless). So the marks are on by default
now; `CHIMERA_RENDER_PHASES=0` leaves them out if the error comes back
(then: the frame's views and phases, and whether a view's binned phase
was drawn without being batched that frame).

### Regression gate

`perfgate` (games/cs_source/tools/src/bin/perfgate.rs, on
`chimera_tools::perfgate`) runs three headless scenarios with
fixed inputs: `greybox` (two characters walking, turning and jumping on
a fixed pattern), `de_dust2` (rounds, 5 bots a side, `mp_freezetime 2;
mp_roundtime 2`), `mg_lego_multigames_v2` (the map's logic and props, no
players), each 300 ticks of warm-up and 2000 measured ticks, on the
game's single-threaded schedules. It counts user-space instructions per
tick summed over every thread of the process (`metrics::AllThreads`):
the main thread alone moves with how much parallel work it picked up,
the sum doesn't.

Six runs at load 30-33 (playtest build):

| scenario | instructions per tick p50 | total (2000 ticks) | spread of total | main thread total spread | CPU ms p50 spread | wall ms p50 spread |
|---|---|---|---|---|---|---|
| greybox | 0.84M | 1.79-1.86G | 3.8% | 4.0% | 30% | 34% |
| de_dust2 | 17.9M | 35.59-35.86G | 0.8% | 3.7% | 6% | 30% |
| mg_lego_multigames_v2 | 6.6M | 13.91-14.20G | 2.0% | 2.2% | 2.7% | 61% |

So the gate compares the total: a rise of more than 3% fails (8% for
greybox, whose small ticks are dominated by the task pools' own
bookkeeping); the stored numbers are `tools/perfgate/baseline.json`,
per build profile (counts differ between `playtest` and the dev
profile's opt-level 1) and noting the CPU (counts also differ between
CPU models and thread counts: take a baseline on the machine that
checks). Its de_dust2 p99 is 26-27M per tick (rounds starting, bots
seeing each other); the run takes about a minute plus map loading.

- Check: `cargo build --profile playtest --features tools --bin perfgate &&
  target/playtest/perfgate` (exit 1 on a rise; `--scenario de_dust2`
  for one).
- A wanted rise (or a drop): `target/playtest/perfgate --update`, and
  commit `tools/perfgate/baseline.json` with the reason; `--tolerance
  N` with `--update` sets the default tolerance. Scenarios whose map
  isn't installed are skipped, and so is everything where counters
  can't open (Windows, `perf_event_paranoid` 3).
- Checked: against a baseline lowered by 8% for de_dust2 it fails
  (`+8.14%, tolerance 3%: FAIL`, exit 1).

## Light-style hitches (mg_jacks_multigames_v1)

Report: frame p50 ~5 ms but p99 ~20 ms and a constant stream of hitch
log lines. The map has 379 lights; faces lit by the animated styles 1,
3, 6 and 10 hold ~83k lightmap texels (150k with block padding) and
switchable styles 32-35 ~59k more, in a 1024x3171 atlas.

What the hitch frames were (playtest build, 1920x1080, `+chimera_perf_log
1`, load 13-38): the main world's `Update` took 22-26 ms of thread CPU
and 200-220M instructions in them against ~12M in an ordinary frame
(`hitch: ... main world 34.11 ms (cpu 30.73, 217.03M instr ...): Update
27.15 (cpu 25.74), ...`), and a profile trace put `light_styles::write_patches`
in the render world at 4.5 ms per frame on average. Timing each light
step directly: ten steps a second (every pattern letter), 233-665 face
blocks rewritten each, which cost

- `animate` (main thread): 110-200M instructions a step, 9-37 ms at that
  load: each texel's light summed from its styles, then encoded for the
  five images (plain f16, Source LDR flat, three LDR directional pages),
  with a `powf` per channel in the LDR encoding;
- `write_patches` (render thread): 1200-2300 `queue.write_texture`
  calls (a block per image), 17-31M instructions, 4-25 ms: wgpu makes
  a staging buffer for every call.

Ten of those a second at 200 fps is 5% of frames, hence a p99 four
times the p50. Nothing else was periodic: with the light steps gone
the hitch lines that remain have thread CPU far below wall time (the
loaded box), and instructions per frame no different from the median.

The fix (`render::light_styles`, run-time path only):

- **States kept.** A block's light depends only on the levels of the
  styles lighting it, and patterns loop through a few letters, so each
  block state is encoded once (on the compute task pool) and kept
  (`StyledLightmaps::states`, at most 128 MB; 15 MB on this map once
  every state has been seen, after the longest pattern's first loop).
  A step then copies kept bytes.
- **One upload a step.** The step's blocks are shelf-packed into one
  buffer (rows 256-byte aligned) for all images (`LightmapUploads`);
  the render world writes it into one GPU buffer it keeps, then records
  a buffer-to-texture copy per rectangle and image in one command
  encoder.
- **Touching blocks merged.** Blocks side by side with the same rows
  (then stacked with the same columns) go as one rectangle: 450 blocks
  become ~165 copies per image here.
- **Only images something samples.** The plain lightmap is drawn only by
  standard materials, used when the world material is missing; it is
  no longer rewritten otherwise (5 images per block to 4).
- **LDR encoding by table.** `source_ldr_texel` and the directional
  encoding look up the 4096 stored levels instead of calling `powf`
  (same bytes: `source_ldr_tables_match_the_formula`); the three
  directional pages are encoded once per texel, not once per page.

A step now costs the main thread 1-3M instructions (cold: ~20M there
plus the pool's share) and the render thread 3-8M (400-680 copies,
~10k instructions each in wgpu; drawing them would remove that: tech-debt.md).

Before (926f4d3's build) and after, same binaries one after the other,
2400 frames at 1920x1080 each, `chimera_perf_log`'s last line (wall
times move with the load given: compare instructions):

| map, place | build (load) | frame ms p50 / p90 / p99 / max | main instr less ticks p50 / p99 | render instr p50 / p99 | hitch lines a minute |
|---|---|---|---|---|---|
| mg_jacks spawn | before (13) | 17.3 / 28.4 / 54.1 / 706 | 11.7M / 214.9M | 14.4M / 48.6M | 39 |
| | after (26) | 21.6 / 36.5 / 60.2 / 1591 | 11.1M / 32.2M | 14.3M / 21.3M | 40 |
| mg_jacks bhop (`setpos -1373 3677 -951`) | before (24) | 16.8 / 28.3 / 75.2 / 1274 | 11.8M / 215.1M | 13.1M / 47.4M | 188 |
| | after (17) | 17.2 / 28.4 / 95.4 / 917 | 10.8M / 28.1M | 13.0M / 20.1M | 195 |
| mg_jacks build (`setpos -12068 5061 -100`) | before (18) | 18.4 / 53.3 / 171.6 / 1396 | 13.1M / 223.9M | 17.5M / 55.3M | 322 |
| | after (13) | 16.8 / 25.5 / 46.2 / 473 | 11.0M / 26.4M | 17.4M / 24.5M | 85 |
| bhop_myztek spawn | before (17) | 17.0 / 50.6 / 250.3 / 542 | 12.1M / 537.9M | 15.1M / 127.8M | 528 |
| | after (25) | 21.1 / 43.2 / 69.0 / 659 | 11.3M / 30.1M | 15.1M / 40.6M | 376 |
| de_dust2 spawn (no styles) | before (36) | 27.5 / 46.8 / 84.6 / 1105 | 13.9M / 29.2M | 17.1M / 18.3M | 31 |
| | after (31) | 18.2 / 29.7 / 90.8 / 204 | 13.5M / 25.8M | 17.1M / 18.4M | 155 |

The p99 of instructions per frame falls 7-18x on the light-style maps
(main) and 2-3x (render); p50s are unchanged, and dust2's instructions
are the same. The wall-time columns and hitch counts (frames over twice
the median) follow the box's load at 13-36 and say little here; on an
idle machine the step frames were the p99. Pictures: `refcmp report`
mean abs diff de_dust2 0.030835 before and after (32 views), de_nuke
0.023818 (27); bhop_myztek's spawn looks the same.

Regression checks (`render::light_styles` unit tests):
`animate_packs_the_blocks_whose_style_stepped` (a step's blocks in one
buffer, rows 256-aligned, bytes as the whole-image encoders write them),
`touching_blocks_merge_into_one_copy`,
`shelf_packing_keeps_blocks_apart_and_dense`,
`source_ldr_tables_match_the_formula`; `kept_states_skip_relighting`
checks that a state seen before is copied, not worked out again.

## Combat Arms map loads (2026-10-19)

A Combat Arms map took 4-8 s to load and spawn and ~1-1.8 GB in the
loader (~10 GB and 17 s at 0a9bd397, when every install weapon's view
model loaded with each map; f5d1909e had cut that to our 13). Measured
with both keys, the dev profile, other agents building (load 20-48).

Where the time went (main, `world::load_from`'s parts timed one by one):

| | WAREHOUSE | JUNK_FLEA | TERRORDOME |
|---|---|---|---|
| weapon data (once per session; the app reads it at startup) | 2.0 s | 2.1 s | 2.0 s |
| read and parse the world | 0.1 s | 0.1 s | 0.7 s |
| convert (`world::convert`) | 0.45 s | 0.40 s | 3.9 s: 2.6 s lighting the lightmap texels (a `powf` each, 80 million), 0.4 s the atlas, 0.4 s textures |
| effects | 0.14 s | 0.13 s | 0.11 s |
| view models (14, with hands) | 0.43 s, +110 MB | 0.42 s | 0.35 s |
| character bodies and held weapons | 1.0 s, +340 MB | 1.0 s | 0.6 s |
| peak memory | 1,076 MB | 1,044 MB | 1,833 MB |

and in the app, on the main thread, the spawn (`spawning map` to the
props line): WAREHOUSE 2.5 s, JUNK_FLEA 1.9 s, TERRORDOME 4.5 s, most of
it the lightmap: its plain and world images (the same texels when the
game has no encoding of its own) each converted to f16 one channel at a
time and copied once more on the way into `Assets<Image>`.

What changed:

- View models load on demand (`view_model::on_demand`): a weapon's when
  it first wants drawing (the local player's inventory, a character's
  shown weapon), in the background, once per session, through
  `map::add_view_model` with its own textures; `render::view_model::
  build_late` builds it on the next frame (with no map's drawing set
  up, the greybox, on assets of its own: `assets_without_map`), the
  placeholder box stands in meanwhile (the M4A1 with hands: 30 ms).
- Character bodies and held weapons convert once per session on a
  thread beside the world (`characters::models::shared_character_models`)
  and are copied into each map.
- Lightmap texels are looked up per byte (`LightLut`) and kept as bytes
  until the atlas is laid out (a quarter of the memory), copied by rows;
  each texture file is read once (meshes differ by blend and sky too)
  and decoded with its mip chain on up to 8 threads (`decode_all`,
  `MapTexture::box_mips`: the drawing's own box filter, now shared).
- Spawn: one lightmap image when plain and world copies agree, built
  without copies, f16 on threads for big atlases.
- Not done: a disk cache of converted maps (DESIGN.md allows one, per
  user). A map now converts in 0.2-1.3 s, about what reading back its
  textures, mips and f32 lightmap (hundreds of MB) would take, and the
  session's first load is bound by the character models beside it, not
  the world. The bots' navigation mesh (`bots::brain::ensure_nav`) is
  still generated on the main thread: a 0.2-0.3 s tick when the first
  bot plays (WAREHOUSE 550 areas, TERRORDOME 980).

After (`heavy::map_load`, a process per test; the first load includes
the 2 s of weapon data the app reads at startup):

| | WAREHOUSE | JUNK_FLEA | TERRORDOME |
|---|---|---|---|
| first load of a session | 2.9 s (was ~4.2) | - | 3.0-3.3 s (was ~7.6) |
| a second map after it | 0.25 s | 0.31 s (was ~2.1) | - |
| loader's peak memory | 922 MB (was 1,076) | 1,181 MB after both | 1,569 MB with a second map (was 1,833 alone) |
| spawn into a simulation (collision) | 0.34 s | - | 0.65 s |

The app (`--map combat_arms:<name> --frames 300 --screenshot`, start
to the `loaded` line, which includes process start and the weapon data;
the spawn; the process's peak memory), main / this change:

| | WAREHOUSE | JUNK_FLEA | TERRORDOME |
|---|---|---|---|
| loaded after | 3.9 s / 3.4 s | 3.5 s / 3.0 s | 6.9 s / 3.0 s |
| spawn (main thread) | 2.5 s / 0.27 s | 1.9 s / 0.22 s | 4.5 s / 0.6 s |
| peak memory | 2,503 / 2,193 MB | 2,305 / 2,122 MB | 3,570 / 3,393 MB |

`heavy::map_load` keeps loads under budget (20 s, 30 s for TERRORDOME,
the spawn and the navigation tick too; peak 2.5 and 3.5 GB): room for a
busy box, a regression check rather than a target.

## Lean map loads (2026-10-21)

N3 of the architecture review (F1): `MapLoadOptions { hdr_level, textures }`
on every `MapLoader`; the dedicated servers and 27 CS:S heavy files that
never draw or read texels load with `textures` off (each texture a
placeholder of the right size: no texels, no mip levels), and CS:S's
textures decode together on up to 8 threads (`MaterialLoader::
finish_textures` on `map::parallel::par_map`) instead of one at a time.
Measured on this box with other agents building (load 8-22), the dev
profile, in `target/` of the worktree.

The dedicated server (`cs_source_server -port 27141 +map <map>`, peak RSS
three seconds after `loaded`, two runs each):

| | de_dust2 | de_nuke | surf_sedona |
|---|---|---|---|
| before | 1,123 / 1,132 MB | 1,587 / 1,592 MB | 2,681 / 2,662 MB |
| after (lean) | 496 / 496 MB | 573 / 571 MB | 1,171 / 1,203 MB |

One load, the same process, lean and whole alternating
(`MapData::load_times`): de_dust2 1.7-1.8 s lean, 1.8-2.2 s whole (the
decode, on threads now, is the `cubemaps` lap's 0.36-0.39 s); de_nuke
2.2-2.5 s against 3.0-3.1 s; surf_sedona about 10 s either way (the
lumps' inflating, 2.5-3.4 s, and the packed files, 2.9-4.6 s, dominate; the
texture decode of ~1 s is what lean saves). So the time a lean load saves
is 0.4-1.3 s; the memory is the larger win. `heavy::map_spawn_dump`
(five maps, every entity, component and asset count, and the systems'
order) is identical before and after, and went from 45 s to 29 s as the
decode moved onto threads.

The 149 CS:S heavy tests that load lean (`net_soak` and `net_capacity`
excluded: minutes each, one process per test, five at a time, the binary
before and after, by `rusage`):

| | before | after |
|---|---|---|
| peak RSS, median over the tests | 1,297 / 1,875 / 1,540 MB (three thirds) | 605 / 599 / 649 MB |
| peak RSS, largest | 6,765 MB | 2,441 MB |
| CPU time, summed | 2,440 s | 2,506 s |
| wall time, summed | 2,112 s | 3,950 s |

The last two rows are not a result: the box's load average went from 8-12
to 18-23 between the runs, and a test is mostly its simulation, not its
load. What is measured is the memory (2x on the median, 2.8x at the top),
which is what lets more heavy tests share a box.

Combat Arms (`heavy::map_load::a_lean_load_keeps_the_textures_sizes_and_
drops_their_texels`, WAREHOUSE as a session's first load): peak 1,378 MB
lean against 1,475 MB whole; its textures are small (DTX at the shipped
detail level), the time is the lightmap and the weapon data. Both dedicated
servers load lean.

Next on the loads (not done): the lumps' inflating and the packed files'
LZMA on sedona-like maps (6-8 s of a 10 s load), the navigation mesh, and
the heavy tier's own simulation time.

## Map changes behind the loading screen (L4, 2026-10-23)

Architecture review L4, stage 3. A `map` command or a joined server's map
already parsed and converted on the async pool while the loading screen
drew; the frame that swapped the map in (`console::finish_map_load`:
`change_map`, `spawn_map`) still did everything that only needs the map's
data on the main thread. Timed inside that frame (temporary `Instant`
marks, dev profile, this box):

| de_dust2 | ms |
|---|---|
| world collision (trimeshes, smoothed copy, terrain, hulls) | 82 |
| texture and cubemap images | 75 |
| lightmap layers | 93 |
| rest of the drawn phases (sky, water, meshes, materials) | 130 |
| props | 67 |
| resources, commands | 80 |

de_nuke's props took 10.3 s: 46 props whose collision pieces are beveled
one after another (`MapBrush::from_planes`, the edge bevels with many
planes; 68 ms to 1.6 s per prop).

Now (`map::prepare`): what the spawn computes from the data alone (the
world's colliders and terrain, each model's collider and hull, each
prop's placed brushes, the drawer's images and lightmap layers:
`MapHooks::prepare`, `render::prepare_images`) is computed on the loading
task after the load (`MapPrepare::run`), the props' brushes and models'
colliders on up to 8 threads (`parallel::par_map`), and the spawn takes
them (`Prepared::take`); a spawn without them (startup `--map`, the
servers, tests) computes them in place with the same functions.

The swap frame, `+map <name>` from the main menu (`chimera_perf_log 1`,
the hitch log's frame and its `Update`), one run each:

| | de_dust2 | de_nuke | warehouse (Combat Arms app) |
|---|---|---|---|
| before | 495 ms (Update 442) | 10,699 ms (Update 10,568) | 244 ms (Update 237) |
| after | 166 ms (Update 134) | 510 ms (Update 342) | 118 ms (Update 110) |

During the load the frames stay at the display's 60 fps (p50 16.7 ms)
before and after; de_nuke's preparation runs about 8.7 s on the loading
task (the bevels on threads), so its loading screen shows that much
longer than its `loaded` line (2.1 s), where before the game froze for
10.7 s. A joining client loads the same way (checked: a listen server on
de_dust2, a client `+connect`s, loads, `joined the game`, its swap frame
186 ms). `heavy::map_spawn_dump` is identical before and after on its five
maps, plain and with `CHIMERA_SPAWN_DUMP_PREPARED=1`.

Left: the bevels themselves (`from_planes` scales with the planes cubed;
a cache by model, piece and rotation would make de_nuke's a few
milliseconds but rounds differently: needs the movement tests' check);
the dedicated servers' level change (`swap_map`) and the startup `--map`
still compute in place (on threads now for the props); meshes and
materials are still built in the swap frame.

## Lucker Party (2026-10-24)

The gun game ran at 40-50 fps on the user's Windows PC, which runs CS:S
(and our `cs_source` app) smoothly. Measured here with `chimera_perf_log
1` (playtest build, 1280x720, de_dust2, the default 3 bots; `cs_source`
with `+bot_quota 3`), the last readout of 2400 frames, three interleaved
rounds (load 4-19 on 12 cores; frames sit at the 60 Hz cap here, so read
the CPU and instruction columns):

| build (round, load) | main CPU ms p50 | main instr less ticks p50 | tick CPU ms / instr p50 | render CPU ms p50 | render instr p50 |
|---|---|---|---|---|---|
| party before (r1, 8) | 14.04 | 109.7M | 2.07 / 8.31M | 5.86 | 17.7M |
| party after (r1, 5) | 5.44 | 15.1M | 1.77 / 6.73M | 5.29 | 17.8M |
| cs_source (r1, 4) | 3.91 | 12.8M | 0.93 / 3.17M | 4.24 | 14.7M |
| party before (r2, 4) | 13.70 | 110.2M | 2.13 / 8.39M | 5.58 | 17.7M |
| party after (r2, 5) | 5.57 | 15.3M | 1.80 / 6.72M | 5.30 | 17.8M |
| cs_source (r2, 4) | 4.10 | 12.5M | 0.97 / 3.22M | 4.79 | 14.7M |
| party before (r3, 19) | 21.41 (frame p50 24.5) | 110.7M | 3.01 / 8.66M | 8.15 | 19.9M |
| party after (r3, 14) | 7.25 (frame p50 16.7) | 16.4M | 2.38 / 6.74M | 7.34 | 18.8M |
| cs_source (r3, 13) | 5.47 (frame p50 16.7) | 13.3M | 1.32 / 3.28M | 6.49 | 15.5M |

(These runs had `chimera_perf 3`, whose logging adds 3-5M instructions
to the main world; the first, without it: party 104.5M, cs_source 7.4M.)
Warehouse (the party on Combat Arms' map, r3): main CPU 19.4 -> 5.8 ms,
main instructions 101.7M -> 7.6M, ticks 8.15M -> 5.79M; its render world
(9.4 ms, 27.5M) is now the larger, Combat Arms' map itself.

What cost it, from traces (`--features profile`, `tracesum`):

- **Combat Arms' front end cloned its UI data every frame.** The options
  screen's, the lobby's and the chat panel's per-frame systems
  (`ui::options::update`, `ui::lobby::update`, `ui::talk::update`)
  each cloned `CaUi`, the install's whole string table with it, even
  with the screen down: 6.7 + 5.0 + 4.4 ms a frame in a trace, about
  95M instructions; the main world's other work was a tenth of that.
  On a slower CPU that alone is the 40-50 fps. They now borrow it, and
  the options screen and the lobby clone it only while they are up (the
  lobby's `sync` cloned it for nothing). The Combat Arms app paid the
  same. `combat_arms` test `ui::frame_cost` compares frames with a
  200 000-string table against frames with ours.
- **Bots' senses rebuilt their queries every tick.** `bots::brain::think`
  made seven `QueryState`s a tick (each walks every archetype; the party
  has many) and cloned `BotData`: 0.55 ms a tick in a trace, now 0.05
  (the queries kept in the system's `Local`, the data borrowed).
- **Combat Arms' animation posed every body twice a tick** (the
  client's aim and the server's, which differ only in the aim):
  `CaAnim::posed_both` poses the layers once.
- Bullet marks rewrote their material and transform every frame (a
  material borrowed mutably is prepared for the GPU again): now only on
  a change. Probe-lit models' materials are only borrowed mutably when
  probe lit (`render::probe_lit::relight`).

Left (the party's main world is still ~35% over `cs_source`'s, its
ticks 1.8x, its render world ~15%; frame times on the box sat at the
60 Hz cap for both after the fix). Ideas, each a small measure-once task:

- Combat Arms' animation (`characters::drive`, 0.38 ms a tick traced):
  `Rig::posed` clones every node's and socket's name per pose and
  `Posed::node` finds names by a case-blind linear search; share the
  names (`Arc`) and look the aim and hit nodes up once per rig.
- Combat Arms' exclusive systems that build `world.query` states every
  call (perks `supplies` and `perk_tick`, `roster::keep_quota` and
  `arm`, `objects::gather`, `grenades::server_frames`,
  `weapon::give_starting_weapons`: 0.02-0.05 ms each a tick traced):
  keep them in a `Local` as `bots::brain::SnapQueries` does. (The
  minigame runtime's `respawn_dead`, `end_protection` and `join`, also
  on this list then, are normal systems since ecs-cleanup step 6.)
- The predicted schedules with both games' guns (`run_predicted`, 0.65
  vs 0.30 ms a tick): Select has 13 systems (CS:S 1), Weapons 17 (8).
- The render world: 65k triangles drawn vs 31k (Combat Arms' soldiers),
  5-6 materials still modified a frame (tracers and particles: real
  changes, but worth a check), `write_binned_instance_buffers` and
  `prepare_preprocess_bind_groups` higher than CS:S's.
- A `perfgate` scenario for the party (it lives in a game's tools crate,
  which can't depend on the app; a party binary of its own).

## Party frame-rate pass (2026-10-28)

Goal: where the Lucker Party gun game's frame time goes, by code, with
pictures to compare before and after. Scenes: `lucker_party +minigame
originals:gun_game_dust2 +MaxPlayer 4` on de_dust2 and with `--map
cs_source:cs_office` (four Combat Arms bots and the idle local player;
the ladder hands out both games' guns, so identities mix), 1280x720,
vsync off, no frame cap.

### Method

`tools/perf-profile.sh` ("Measuring"): the `profiling` build (playtest
plus line tables) with frame pointers, `perf record -e cpu-clock:u
--call-graph fp -F 499` from 40 s after start for 20 s, then `profsum`.
"Before" is main at 1aa9e649 plus the readout and `--window` vsync
changes (no behaviour change), "after" this pass; the two binaries ran
interleaved, three rounds per map, at load 9-17 on 12 threads (other
agents building and testing). The per-second `chimera_perf_log`
readouts of the same window give the frame and instruction numbers.
DWARF unwinding was tried first and lost most of the main thread's
stacks; the distribution's glibc has no frame pointers, so samples
inside malloc, free and memcpy keep no callers (about 1 ms a frame on
the render thread and 1.8 on the task pool: `[libc.so.6] (callers
unknown)`, "allocator and memcpy" in the pictures). A span trace
(`--features trace`, `trace_capture 3`, `tracesum --folded`, `profsum
--unit us`) gave the same ranking by system with call counts, but
tracing made frames 5-7x slower (18 fps), so its milliseconds aren't the
game's.

### Baseline

Medians of the three rounds (range); frame ms from the 4096-frame
readout, instructions per frame p50 (main world without its ticks, per
tick, render world), GPU from timestamps:

| map | build | frame p50 ms | frame p99 ms | main CPU p50 ms | render CPU p50 ms | main instr w/o ticks | tick instr | render instr | GPU p50 ms |
|---|---|---|---|---|---|---|---|---|---|
| de_dust2 | before | 8.70 (7.64-9.56) | 43.9 (43.6-60.3) | 6.94 (5.98-7.64) | 6.58 (6.17-7.38) | 11.63M (11.58-11.97) | 9.53M (9.23-9.55) | 20.71M (17.87-21.11) | 1.53 |
| de_dust2 | after | 7.51 (7.34-8.09) | 25.3 (9.8-40.7) | 5.46 (5.45-6.17) | 6.35 (6.19-6.56) | 10.53M (10.47-10.62) | 7.10M (7.04-7.16) | 20.96M (19.27-21.00) | 1.54 |
| cs_office | before | 9.35 (9.06-9.55) | 36.4 (33.9-55.5) | 7.64 (7.39-7.84) | 7.64 (7.42-7.69) | 15.17M (14.93-15.23) | 12.83M (12.82-12.97) | 25.22M (25.19-25.64) | 1.84 |
| cs_office | after | 9.46 (7.74-12.74) | 54.4 (10.0-65.0) | 7.41 (6.08-9.53) | 7.95 (6.76-10.56) | 14.78M (13.37-15.06) | 10.77M (10.53-10.79) | 25.26M (25.14-26.29) | 2.04 |

Wall times swung 1.5x between rounds of the same build (cs_office's
"after" run 1 started at load 9 and ran at 52 fps, its run 3 at 126):
read the instruction columns. The ticks run 24% (de_dust2) and 16%
(cs_office) fewer instructions; the main world's frame without ticks
5-10% fewer; the render world is unchanged. On the quietest runs the
frame went from 7.64 to 7.34 ms (de_dust2) and 9.06 to 7.74 ms
(cs_office), 1% lows from 11-16 to 22-87 fps on de_dust2; the GPU took
1.5-1.9 ms of a 7-9 ms frame, so the game is CPU bound, and with the
main world's work cut, the **render thread is now the limit** (its CPU
time per frame is the frame time; the main world waits for it).

Where a frame's CPU went after (de_dust2, perf, 18.1 ms of CPU a frame
over all threads at ~125 fps): render thread 6.9 ms (Bevy's render
world 5.2, wgpu and the driver 0.4, unsymbolized and libc 1.1), main
thread 5.8 (our code 1.9, Bevy 3.5), task pool 5.2 (wgpu's command
encoding 1.8, Bevy's parallel systems 1.4, libc 1.8), audio 0.15
(pictures: `families.svg`).

### Hot spots, ranked

ms of CPU a frame (inclusive) in the de_dust2 "after" profile, ~125
fps; "before" where a fix moved it. Fix ideas with an expected gain and
effort (S/M/L):

| # | where | ms/frame | why | fix | gain | effort |
|---|---|---|---|---|---|---|
| 1 | wgpu command encoding (`CommandEncoder::finish`, task pool and render thread) | 1.93 | every camera's passes encoded and validated each frame (4 cameras drawing) | fewer views: check which of the 4 cameras must draw (the party HUD's?), as "Fewer cameras per frame" did | 0.3-0.5 per camera | M |
| 2 | `bevy_render::renderer::render_system` (submit, present) | 1.87 (of it `Queue::submit` 0.90) | buffer and bind-group lifetimes, destroying last frame's buffers | follows from 3-5 and fewer views | - | - |
| 3 | `write_binned_instance_buffers` | 0.63 | instance buffers rewritten per phase per view | fewer views; fewer mesh instances changing (14 meshes and 6 materials modified a frame, `chimera_perf`) | 0.2 | M |
| 4 | `prepare_preprocess_bind_groups` | 0.56 | Bevy makes the GPU-preprocessing bind groups again every frame for every view | upstream (cache them while the buffers stay the same); fewer views | 0.4 | L (Bevy) |
| 5 | `prepare_clusters_for_gpu_clustering` | 0.33 | clustering buffers made per view each frame | lights off the views that need none (sky, view model) if Bevy allows | 0.2 | M |
| 6 | `MeshAllocator` | 0.30 | meshes modified every frame (dust, particles, prop shadows, tracers) reallocate | find the 14 a frame (log them in `chimera_perf 3`), write only real changes | 0.2 | S |
| 7 | fixed ticks' executor (`run_fixed_main_schedule`, not in a system of ours) | 1.17 (before 1.61) | hundreds of small systems a tick (avian's, both games'), each with the executor's bookkeeping | fewer, larger systems in the per-tick schedules (Select 13, Weapons 17 with both games' guns, "Lucker Party"); run_if on whole sets | 0.3 | M |
| 8 | new `QueryState`s (`world.query` in exclusive systems) | **before 1.23, after 0.40** | each matches all 2263 archetypes | fixed here (below); left: Bevy's own commands (`check_dir_light_mesh_visibility` 0.09, bevy_audio's `despawn_all_where` 0.12), the minigame HUD, moments and ladder (0.12) | 0.25 more | S |
| 9 | `bevy_ui` layout | 0.38 | Combat Arms' HUD and the party's panels, laid out every frame | hide or despawn UI trees not shown; fewer nodes | 0.2 | M |
| 10 | our measurement (`client::perf`, `frame_metrics`, `Percentiles::of`) | 0.33 | the readout's percentiles and mesh counts, with `chimera_perf_log 1` on | only while shown; sort less often | 0.2 (only when measuring) | S |
| 11 | Combat Arms' animation (`characters::drive`) | 0.18 | `Rig::posed` clones every node's name, `Posed::node` searches names case-blind | names shared, nodes looked up once per rig | 0.1 | S |
| 12 | glows' ray casts (`glow_visibility`) | 0.10 | five rays per visible glow, as CS:S's occlusion | parity: leave | - | - |
| 13 | binds (`keys_for`) | **before 0.09, after 0** | a lowercased copy of every bind per lookup, several lookups a frame | fixed here | - | - |
| 14 | `roster::arm` cloning `BotData` | **before 0.05, after 0** | the bots' whole table cloned whenever a bot re-armed (every level) | fixed here | - | - |

### Fixed

- **Kept query states** (`chimera_core::query_cache`): `cached_query::<D,
  F, _>(world, |q, world| ..)` lends a `QueryState` kept in the world
  (one per query type) to the closure, and `first::<F>(world)` is the
  first entity matching a filter. A new state matches itself against
  every archetype (2263 in the party: Combat Arms' and CS:S's
  characters, weapons, props and UI), tens of µs a call. Used in the
  per-tick and per-frame exclusive systems the profile named: the
  minigame runtime's `players`, `join` and `end_protection`, the logic
  bridge's `sync_controls`, `snapshot`, `sync_soundscapes` and
  `apply_effects`, the bots' `keep_quota` and `arm`,
  `give_starting_weapons`, the perks' `perk_tick` and `supplies`,
  Combat Arms' objects, the console's `local_userinfo` and overlay, and
  the local-player lookups of chat, voice, the death camera, the lobby,
  music. New `QueryState`s per frame: 1.23 -> 0.40 ms (picture
  `querystate-callers`); the ticks 24% fewer instructions on de_dust2.
  Same results as a new state (`query_cache` tests: entities added to
  known and new archetypes, mutable access, nested calls).
- `binds::keys_for` compares commands without lowercased copies (0.09
  ms a frame; a test checks it against `norm` with non-ASCII text).
- `roster::arm` borrows `BotData` (`resource_scope`) instead of cloning
  it (0.05 ms).

No behaviour or look changes: the same queries over the same entities,
the same comparisons. The fast tier of the changed crates passes
(chimera_core, chimera_client, chimera_sim, chimera_net, source_logic,
combat_arms; `profsum`'s own tests with `--features tools`).

### Left

The render thread sets the frame now. In order: count and cut the
cameras drawing (1, 3-5: 4 cameras drawing in a party match; each costs
encoding, a preprocessing bind group set, clustering and instance
buffers), the meshes and materials modified every frame (6), then the
per-tick executor cost (7) and the UI layout (9); the remaining new
query states and Combat Arms' animation are small (8, 11). A party
`perfgate` scenario would hold the tick instructions ("Lucker Party"
above). On the Windows PC: `scripts\profile.ps1 lucker_party` gives the
span version of these pictures (`tracesum --folded`, `profsum --unit
us`).
## Map changes that slowed the tick (2026-10-28)

Playtest report: the gun game's de_dust2 started at ~240 fps and ran much
slower after a few map changes in one session; more bots seemed slower
too. Measured with `chimera_census` (OBSERVABILITY.md 3c: entities by
archetype, observers, systems per schedule, resources, assets; `diff`
lists new archetypes and tables) after each of N map changes, the
readout's load-independent columns, and the tick's instructions per
system (`metrics::span_cost`, with `trace`) and gdb samples of a
headless run.

**What grew: archetypes and tables, nothing else.** Entities (4,200-4,400
on dust2), observers (239), systems (1,475), resources and the asset
stores stayed flat over 30 map changes (Images +8 a load, small: not
looked into); GPU memory and RSS too. Bevy never frees an archetype or
a table, and the party's characters carry ~130 components, dozens of
which come and go (spawn protection's `Protected`, `SpawnProtected`,
`God`; avian's `Sleeping`; the body model `map::unload_map` takes off and
the next map puts back; the movement and view parts of the gun's game,
CS:S's or Combat Arms', switched with every rung): each map change and
death visits component sets never seen, so a session made ~50 new
archetypes and ~30 tables per round trip, nearly all characters', nearly
all empty (2,250 archetypes and 950 tables at the start, 3,070 and 1,690
after ten dust2 -> cs_office -> dust2 trips).

**What it cost: the tick.** `world.query::<..>()` builds a new
`QueryState`, which walks every archetype holding the query's rarest
component; exclusive systems that build their queries each tick (the
minigame runtime's `players`, `join`, `end_protection`, `respawn_dead`;
Combat Arms' perks and bot roster; the local player's lookup in a dozen
places) paid that every tick, a character query ~700k instructions at
2,600 archetypes. The fix is the kept query states ("Party frame-rate
pass" above, `chimera_core::query_cache`, found the same day from the
frame's side), which look only at archetypes made since their last use;
this pass added the minigame HUD's and moments' lookups, and the test
and tools to watch the growth. The real app (playtest build, the default 3 bots, `map` between de_dust2 and
cs_office, each row 65 s after arriving on dust2; tick instructions p50
over the last 4,096 ticks, main world's instructions less ticks p50):

| round trips | before: archetypes / tables | tick instr | main instr | after: archetypes / tables | tick instr | main instr |
|---|---|---|---|---|---|---|
| 0 | 2,256 / 951 | 7.30M | 10.27M | 2,253 / 947 | 5.26M | 10.19M |
| 1 | 2,523 / 1,191 | 8.70M | 11.10M | 2,530 / 1,197 | 5.84M | 10.85M |
| 3 | 2,834 / 1,478 | 10.91M | 12.04M | 2,719 / 1,372 | 6.05M | 9.58M |
| 5 | 2,964 / 1,596 | 11.80M | 12.29M | 2,860 / 1,504 | 6.12M | 9.73M |
| 10 | 3,071 / 1,692 | 13.00M | 11.98M | 2,973 / 1,608 | 6.60M | 9.41M |

(The render world's instructions stayed at 19-27M in both.) The tick
cost 78% more after ten trips before, 25% after, and 28% less from the
start; after ten trips it is half what it was. At 240 fps a frame's main
thread does its own work plus 0.28 ticks: 12.3M -> 15.6M instructions a
frame after ten trips before (about 240 -> 190 fps where the main thread
is the limit), 11.7M -> 11.2M now. Frame rates on this box (60 Hz
display, load 15-20, another agent profiling) said nothing; the user's
playtest will. Headless, `party_kit` `archetypes`: eight bots playing
while their component sets go through 2,047 new combinations, as
sparse-set markers (archetypes, no tables) 5.3M -> 13.4M instructions a
tick before (2.5x), 4.4M -> 5.2M now (1.19x, the test's limit 1.3x); as
table markers, 24.6M a tick at 4,800 archetypes and 2,400 tables before,
8.4M now.

What still grows (`query_cache`'s known shortcuts): a kept query steps
over every empty table it matches each time it runs (avian's solver,
transform propagation and every character system: +25-70k instructions
per call with 2,000 more character tables), and each new archetype
makes it look again at those with its rarest component. Fewer tables
would need the characters' optional components as sparse sets, or
fewer of them (a backlog item).

Per bot (`party_kit` `heavy::bot_cost`: de_dust2, the gun game, 600
ticks; instructions per tick on the schedules' thread, this build):

| bots | tick instr | per bot over 2 |
|---|---|---|
| 2 | 5.43M | |
| 4 | 11.37M | 2.97M |
| 8 | 17.17M | 1.96M |
| 16 | 23.41M | 1.28M |

Roughly linear, about 1.3M a bot at 16 (the 2 -> 4 step also starts the
fights). What grows with the bots (`--features dev,trace`, instructions
per tick from 2 to 16 bots): Combat Arms' animation
(`characters::drive`, 0.76M -> 5.15M: `Rig::posed`, the "Left" item of
"Lucker Party (2026-10-24)"), the bots' brains (`brain::think`, 0.35M ->
2.77M), the grenades' server frames (`grenades::server_frames`, 0.29M ->
1.69M: every character's body shapes each frame), the narrow phase
(0.03M -> 0.71M), CS:S's push-away (0.13M -> 0.62M), hit boxes (0.02M ->
0.39M). No single obvious per-bot hot spot outside the animation.

### Second pass: the cameras (2026-10-29)

Same scenes and tool, with the local player held still (`--spawn` and
`--look` at a fixed place, `+sv_cheats 1 +god`: a random spawn moved
the render world's instructions by 1M a frame between runs).

**The cameras.** A party match draws four, all on the window, sharing
its intermediate textures: the 3D sky's (`render::sky`, order -1: the
2D sky cubemap and the 3D skybox's layer, behind everything; on while
the eye's leaf sees sky, indoors on cs_office too), the world's (the
local player's `FirstPersonCamera`, 0), the view model's
(`render::view_model`, 1: the weapon, shells, muzzle flashes; already
off with nothing on its layer: "Fewer cameras per frame") and the UI's
(`client::debug`, 100: Bevy UI, both games' HUDs, the party's panels,
egui). The C4 screen's camera, monitors' and water reflections' are
off or absent here. Each must draw (its own projection, depth or
layer); what each cost was measured by turning it off
(`render_instr`, de_dust2: sky -2.5M, view model -1.8M of 17.5M a
frame) and then by what in it scaled with views:

- **Bevy's GPU mesh preprocessing**: per view and per frame it made the
  preprocessing bind groups again (`prepare_preprocess_bind_groups`
  0.66 ms), wrote every phase's instance buffers
  (`write_binned_instance_buffers` 0.59) and encoded compute passes over
  the instances (bin unpacking, uniform allocation, culling, indirect
  parameters: more GPU time than the opaque pass, 0.4 + 0.3 ms). Now
  every camera has `NoIndirectDrawing` (`render::cameras::CamerasPlugin`,
  a component Bevy requires on every `Camera`): each view's visible
  meshes are batched on the CPU, as on hardware without compute shaders.
  Same draws; a few hundred meshes drawn a frame batch faster than the
  GPU path's per-view setup.
- **Copies out to the window**: every camera ended with Bevy's
  `upscaling` blit to the swapchain, overwritten by the next camera's.
  `render::cameras::skip_covered_outputs` (in `Last`, after everything
  that turns cameras on and off) sets `CameraOutputMode::Skip` on an
  active camera when a later active camera on the same target, HDR and
  MSAA, drawing the whole target, copies out after it (the UI's; the
  view model's or the world's when the UI's is off; each target of a
  capture its own), and turns it back on otherwise. The camera left
  copying replaces the target (`BlendState::REPLACE`): Bevy blends every
  copy but the target's bottom camera's over what the target holds, so
  with the bottom ones skipped the frame was blended over the window's
  clear colour where its alpha is below one. Stock de_dust2's and
  cs_office's skies are opaque (the frozen scenes below matched);
  de_piranesi's isn't: its sky showed grey, the sky only at the edges of
  alpha-to-coverage foliage (2026-10-28 playtest, fixed the next day).
- Tried and left: `ClusterConfig::Single` on the sky and view-model
  cameras (no gain; `ClusterConfig::None` fails wgpu validation: the
  world and prop shaders read the clusters), and light clustering is per
  view in Bevy 0.20 whatever the view holds.

A/B in one binary: `CHIMERA_GPU_PREPROCESS=1` (Bevy's GPU preprocessing
again), `CHIMERA_SKIP_OUTPUTS=0` (every camera copies out). Test:
`chimera/present/tests/it/camera_outputs.rs`.

Medians of two quiet rounds (load 5-9, `chimera_perf_log`, 20 s from
40 s after start; range), and of the three perf rounds at load 18-31
for the instructions:

| map | build | fps | frame p50 ms | render CPU p50 ms | render instr / frame | GPU p50 ms |
|---|---|---|---|---|---|---|
| de_dust2 | before | 126 (126-126) | 7.61 (7.61-7.62) | 6.22 | 17.5M (perf rounds 18.8M) | 1.45 |
| de_dust2 | after | 168 (150-187) | 5.98 (5.76-6.20) | 4.15 | 9.8M (perf rounds 10.1M) | 0.96 |
| cs_office | before | 110 (107-114) | 8.76 (8.11-9.41) | 7.56 | 25.0M (perf rounds 26.5M) | 1.83 |
| cs_office | after | 139 (138-140) | 7.90 (7.50-8.30) | 4.57 | 10.7M (perf rounds 10.8M) | 1.02 |

So 44% (de_dust2) and 57% (cs_office) fewer render-world instructions a
frame, a third (de_dust2) to almost half (cs_office) less GPU time, and the render thread's CPU from 6.2-7.6
to 4.2-4.6 ms: the **main thread sets the frame again** (cs_office: its
14M instructions a frame without ticks plus 11M a tick). The sky's and
the view model's cameras batched on the CPU with the skipped copies
alone gave 17.6 -> 16.1M on de_dust2 and 24.9 -> 22.0M on cs_office;
the rest is the world's camera batched on the CPU too. In the perf profiles (de_dust2, quiet round): render thread
8.19 -> 5.90 ms of CPU a frame, `prepare_preprocess_bind_groups` 0.66 ->
0.03, `write_binned_instance_buffers` 0.59 -> 0.01, `Queue::submit` 1.08
-> 0.56, `CommandEncoder::finish` on the task pool 1.29 -> 0.78,
`create_bind_group` 0.81 -> 0.23; `upscaling` 0.02 -> 0.01; cs_office
9.32 -> 6.64 ms. (The main thread's ms a frame there rose because that
run drew fewer frames under perf, so more ticks a frame.)

No visible change: the frozen scenes (`+host_timescale 0`, the CA HUD,
the 3D sky; de_dust2 and cs_office) are identical to the pixel before
and after; live captures over the remote console (`setpos`, `give`,
`+attack2`, `host_timescale 0`, `screenshot`; AWP with the CS:S HUD and
an M4A1 and L96A1 with Combat Arms', each unscoped and scoped, both
maps) differ only in the HUD's clock (17x21 pixels) and, for Combat
Arms' scoped view model, its sway, as two runs of the old build do.

**Meshes and materials modified a frame** (`chimera_perf 3` now names
each modified mesh or material by an entity drawing it, and logs the UI
trees laid out): on de_dust2 13.9 `PropMaterial`s of "Body"
(`render::probe_lit::relight`: a moving character's body re-sampled
from the light field every 0.25 m, 8 materials each, and held weapons
1 each), 5 rope meshes (`render::rope_sway`: ropes within
`rope_wind_dist` swaying), 3 character blob shadows (moved over 5 mm),
2.2 decal `StandardMaterial`s (Combat Arms' marks fading), 1 dust mesh.
All real changes: none wrote the same data (a relight's new probe
values differed from the material's in 300 of 300 sampled). Cheaper
writes need sharing, not skipping: one probe per body instead of a copy
in each of its materials (8 writes to 1), ropes rebuilt only while
their PVS shows them.

**`bevy_ui` layout** (0.30-0.33 ms): 372 UI nodes in 73 roots, 316 of
them in 37 hidden roots (the largest 163 and 91 nodes, unnamed); about
12 roots get one node or text changed every frame (unnamed nodes, some
texts set to "" again). Not changed this pass: name the trees, write
texts only when they change, and give hidden trees `Display::None`
where nothing depends on their size.

Pictures (ms of CPU a frame, before grey, after blue; per thread by
crate family; flame graphs before and after): `top.svg`, `families.svg`,
`flame.svg` from `profsum --baseline`.

### Third pass: the schedules, the HUD, per-bot animation (2026-10-29)

Same scenes, `--spawn`/`--look` and `+sv_cheats 1 +god` as the second
pass (de_dust2 `--spawn -10.69,0.0,-52.07 --look 90,0`, cs_office
`--spawn -28.24,-8.43,39.42 --look 0,0`), "before" main at 19418530.
The box ran at load 12-40 the whole session (other agents building), so
the game's wall times swung 3x between runs of one build: read the
instruction columns and the headless tick numbers, which ran
interleaved with the same inputs.

**Executors (item 7 of the hot spots).** What was structural: schedules
nobody had put on the single-threaded executor ran on Bevy's
multi-threaded one, which hands every system to a worker and wakes the
next: prediction's `Predict::Select`, `Movement` and `Weapons` (every
tick, and per replayed command on a client: 13, ~10 and 17 systems in
the party), `StateTransition`, `SpawnScene`, egui's passes, and the
render sub-app's update schedule (one system that runs `Render`) and
its `First`. `harness::set_executors` now sets every schedule of a world
(`single_threaded_schedules`; the client does the render world's too);
`Sim::multi_threaded` and `CHIMERA_EXECUTOR=multi` still put Bevy's back
on all of them. It also makes the prediction stages' unordered systems
run in one order, the topological sort's, where the workers took them in
any (the schedule ratchets' pairs are unchanged; prediction's still have
no pair the server and client order differently). The extract schedule
stays on Bevy's executor: on one thread its ~150 systems added ~0.6 ms a
frame to the main thread (perf, both maps), which sets the frame.
The rest of the executor's cost is per system and real: about 0.6-0.8
ms a frame of `SingleThreadedExecutor::run`, `system_type`,
`run_without_applying_deferred`, `apply_deferred` and run conditions on
the main thread (perf self time), i.e. the ~600 systems a tick each
touched once. Cutting that needs fewer systems (merging them changes the
order of their unordered neighbours: 718 ambiguous pairs in Combat Arms'
`FixedUpdate` alone, `tests/it/schedules.rs`), not done here.

Headless, all threads (`perfgate`, playtest build; and `party_kit`
`heavy::bot_cost`, a dev build; medians of 3 before and 2 after rounds,
interleaved):

| scenario | instructions per tick, all threads | main thread p50 | cpu ms p50 | wall ms p50 | wall ms p99 |
|---|---|---|---|---|---|
| perfgate de_dust2 (CS:S, 10 bots), before | 13.14G / 2000 ticks | 4.24M | 2.35 | 2.18 | 5.67 |
| perfgate de_dust2, after | 12.93G | 5.60M | 1.34 | 1.19 | 2.43 |
| perfgate greybox, before | 1.34G / 2000 | 543k | 0.66 | 0.59 | 1.95 |
| perfgate greybox, after | 1.24G | 583k | 0.33 | 0.25 | 0.31 |

| party gun game on de_dust2 (`bot_cost`) | 2 bots | 4 | 8 | 16 |
|---|---|---|---|---|
| wall ms per tick, before | 2.70 | 3.81 | 4.91 | 7.14 |
| wall ms per tick, after | 1.58 | 1.96 | 2.73 | 4.70 |
| schedules thread's instructions per tick, before | 5.46M | 12.88M | 18.49M | 27.18M |
| the same, after | 5.56M | 11.52M | 17.76M | 30.41M |

Instructions hardly move in all (2-8% fewer) while CPU and wall time
per tick halve: the multi-threaded executor's cost was waking and
waiting (low-IPC handoffs, syscalls), not instructions. The prediction
stages' systems now count on the main thread (`main thread p50` up, the
party's per-thread instructions flat to +10% with 16 bots) and the
fights make the per-bot rows noisy (±10% between rounds of one build).

**Bevy's own per-frame `QueryState`s.** `check_dir_light_mesh_visibility`
queued a command every frame that built a new query over
`ViewVisibility` (all 2,000+ archetypes), with no shadow-casting light
in the scene: 0.13 ms a frame. `render::idle_lights` runs it only while
a directional light casts shadows or still lists cascade entities (the
same results). Left, also Bevy's: bevy_audio's `cleanup_finished_audio`
(`despawn_all_where`, ~0.10 ms; its system is private), the screenshot
plugin's `clear_screenshots` (~0.04). Ours, now kept states: the
ladder's ammo refill, the logic bridge's `sync_visuals`, Combat Arms'
scoreboard and music `late_join`.

**The HUD (item 9).** `chimera_perf 3` names an unnamed root by its
own component types: the trees were Combat Arms' HUD (`InstallRoot`,
163 nodes, and `MinigameRoot`, 91, hidden while a CS:S gun is held),
its scope overlay, the match-end, crosshair, callout, console, net
graph and respawn panels. Changed every frame for nothing: the shell's
HUD texts (`HealthText`, `AmmoText`, `CenterText`, `KillfeedText` set
to "" again), the crosshair's lines (`CrosshairLine`, 5 a frame), CS:S's
HUD parts (`GameHudPart`, 5) and damage bars (`Pain`, 2), the voice icon,
`OverlayText` and the debug HUD. `client::ui_write` (`set_text`, `edit`)
writes only a change; now only the perf overlay itself changes a frame.
`ui_layout_system` stayed at 0.25-0.30 ms: its cost is two walks over
every node every frame (taffy's `round_layout` and Bevy's
`update_uinode_geometry_recursive`, 70% of it), changed or not, and
`Display::None` skips neither in Bevy 0.20, so hidden trees didn't get
it. What would cut it: fewer nodes laid out (Combat Arms' 254 hidden
nodes despawned while its HUD is away) or fewer roots (Bevy inserts
`Propagate` into every root every frame: 73 roots, ~0.07 ms of
commands). No visible change: the warm-up scenes (`+host_timescale 0`,
Combat Arms' HUD, the minigame panel, the scoreboard, cs_office) are
identical to the pixel before and after; live captures with an AK-47
(CS:S's HUD) and an M4A1 (Combat Arms') show the same HUDs, the
differences being the live game's (kill feed, chat, blood, the health
panel's flash).

**Per bot (item 11, and the "biggest per-bot grower").** Combat Arms'
posed skeletons share their rig's names, parents and sockets
(`skeleton::Shape`, made once in `Rig::new`) instead of cloning every
name per pose, and look names up through a case-blind index; the
driver's animation names (`AnimList::find`, several a tick per body,
each a scan of a few hundred entries) too. In the perf rounds
(load 12-13, ms of main-thread CPU a frame): `characters::drive`
0.16 -> 0.11 (de_dust2) and 0.21 -> 0.10 (cs_office),
`eq_ignore_ascii_case` 0.08 -> 0.01 and 0.11 -> 0.03. One probe per body instead of a copy in each
of its 8 materials (`probe_lit::relight`) was not done: it needs the
probe moved out of `PropMaterial` into a per-entity uniform the prop
shader reads, a change to every probe-lit draw, not a safe one here.

**Binds.** `binds::pressed` and `just_pressed` compared every bind with
the command per call, several calls a frame (0.09-0.13 ms); they now look up
the binds of the keys down (none or a few). Same answers
(`pressed_is_any_bound_key_held`, `key_names_are_unique`).

The game, perf rounds at load 12-26 (`chimera_perf_log` window of each
round; instructions p50, main thread):

| map | build | main world w/o ticks | tick (main thread) | render world |
|---|---|---|---|---|
| de_dust2 | before | 10.16M (9.73-10.69) | 7.52M (7.15-7.80) | 10.20M |
| de_dust2 | after | 9.25M (8.66-9.31) | 6.97M (6.92-7.02) | 10.61M |
| cs_office | before | 13.38M (13.18-14.60) | 10.91M (10.80-11.27) | 10.57M |
| cs_office | after | 12.39M (11.92-13.05) | 11.32M (11.23-11.37) | 10.56M |

The main world's frame without ticks runs 7-10% fewer instructions
(the light check, the HUD writes, binds); the tick's main-thread count
fell on de_dust2 and rose 4% on cs_office although the prediction
stages' work moved onto that thread (it was on the workers). Frame
rates on the loaded box said nothing either way (77-217 fps in rounds
of the same build); a quiet-box or Windows run is the check.

Pictures (`perf/pass3` of the session; profsum `--baseline`, before grey,
after blue): `top`, `families`, `flame` per map; the headless ticks'
wall time; the frozen and live HUD captures.

## See a frame yourself

Three features, on every app with a window (`cs_source` at the root,
`lucker_party`, `combat_arms_app`, `mashup_demo`):

- `trace`: spans for every system, schedule and render step, and the
  console command `trace_capture [seconds]` (default 1, at most 30),
  which records one window of them into `trace-<map>-<unix time>.json`
  in the working directory (`CHIMERA_TRACE_DIR` names another);
  `CHIMERA_TRACE_SECONDS=1` records one by itself, `CHIMERA_TRACE_AFTER`
  seconds after start (default 10). A second of the gun game is
  40-110 MB of JSON (5-10 MB gzipped). The file has span names, their
  fields (system names) and thread names only: no source paths or user
  names (`client::trace_capture`).
- `profile`: `trace`, plus Bevy's `trace_chrome` writing the whole run
  (`TRACE_CHROME=<file>`; large: keep runs short), and the hitch log's
  spans.
- `tracy`: `trace`, plus Bevy's `trace_tracy`: the Tracy profiler shows
  the frames live (spans per thread, frame marks, and through Bevy's
  render diagnostics, which the client turns on, the GPU's passes; not
  tried on the dev box, which has no Tracy). Bevy 0.20 uses `tracy-client` 0.19 (`tracy-client-sys`
  0.30.1, Tracy 0.14.1): use the **Tracy 0.14.1** profiler (Windows:
  `windows-0.14.1.zip` from https://github.com/wolfpld/tracy/releases;
  another version won't connect). Start the game, then Tracy's Connect
  (localhost).

Windows, from the repo root (`scripts\profile.ps1` pulls and builds the
playtest profile with the feature; a first build takes a while):

```
.\scripts\profile.ps1 lucker_party          # 1 s at 20 s in -> trace-de_dust2-<time>.json
.\scripts\profile.ps1 lucker_party --map combat_arms:warehouse
.\scripts\profile.ps1 -tracy lucker_party   # live in Tracy 0.14.1
.\scripts\profile.ps1                       # the CS:S app (combat_arms, mashup_demo too)
```

Or by hand: `cargo run --profile playtest -p lucker_party --features
trace --bin lucker_party`, then `` ` `` (console) `trace_capture 1` at the
moment to look at. Linux: `scripts/profile.sh`, the same arguments.

Reading it: open https://ui.perfetto.dev, "Open trace file", pick the
JSON (it stays in the browser). Each thread is a track: `main` runs the
main world (`main app` per frame, its schedules and systems as nested
slices), `Render thread` the render world (`sub app: name=RenderApp`),
`Compute Task Pool (n)` the parallel parts. Click a slice for its
length; select a range and the table below sums the slices by name
(sort by wall duration to find what a frame spends). On the dev box,
`tracesum <file> --skip 0` prints the same summary as text (systems by
self time per frame).

## Test cycle

How long `cargo test` takes to build and run, and why the tests are laid
out as they are (docs/OBSERVABILITY.md section 1 has the commands).
Measured 2026-10-08 on the dev box (12 cores, 31 GB) while other agents
built and tested: load averages from 9 to 60, given with each number, so
compare ratios. `CARGO_BUILD_JOBS=6`, `--features dev`, a worktree's own
`target/`.

Where the time went:

- Every integration test file was its own test binary: 61 of them, each
  a link of the whole game (about 1 GB with debug info). Rebuilding the
  tests after a change in `src/` relinked all 61: 213 s (load 52), the
  test binaries' units summing to 835 s on 6 jobs.
- `build.rs` watched `.git/HEAD`, which doesn't exist in a worktree (`.git`
  is a file there): cargo reran it on every build and relinked every
  target, even with nothing changed. A no-op `cargo test --no-run` took
  410 s (load 55); it now takes 1 s. It also watched `.git/index`, so
  every `git add` (and so every commit's hook) relinked everything.
- `cargo test` built a test harness for each binary target too (five
  without any tests).
- Running: `cargo test` runs one test binary after another: 618 s of
  running (load 13), de_dust2's 60 map tests alone 162-224 s. The whole suite's tests add up to 2600 s of
  test time (nextest, load 20); the slowest are real-map tests (de_nuke's
  ladders 123 s, dust2 props 102 s) and bot simulations (bot_radio's
  entity-id checks 80 s).

What changed:

- One test binary, `tests/it/` (`main.rs` declares each file as a
  module); the real-map and long bot tests under `heavy`.
- `build.rs` watches this checkout's HEAD and branch ref through
  `git rev-parse --git-path`.
- `test = false` for the binaries without unit tests (`cs_source`, formerly `mashup`; `dump`,
  `movecmp`, `refcmp`, `tracesum`; checks/tests/it/architecture.rs checks they
  stay without).
- Two tiers: the fast tier before each commit, the full suite before a
  push.
- (2026-10-13) The binaries no test runs are behind features
  (`required-features` in Cargo.toml): the debugging tools (`refcmp`,
  `movecmp`, `perfgate`, `tracesum`, `mapsweep`) need `--features tools`,
  the dedicated server `--features server`. Cargo builds every binary of
  the package for its integration tests, so before this each of them
  relinked the whole game on every `cargo test` (and `mapsweep` built a
  test harness too). Only `mashup`, `dump` (run by `mount_cs_source`) and
  `testmap` (no game; its unit tests check the committed `.vmf`) are
  built now. A new `pub fn` in `client/window_icon.rs`, then removed
  again, `cargo test --features dev --no-run`, `CARGO_BUILD_JOBS=4`,
  sccache, interleaved: before 232 s (load 36), 226 s (load 28), 179 s
  (load 24), 230 s (load 27); after 132 s (load 16-25), 122 s (load
  16-20), 119 s (load 25), 151 s (load 26-30): about 45% off a
  source-change test cycle.

| | before | after |
|---|---|---|
| cold `cargo test --no-run` (all dependencies) | 41 min (load ~25) | the same; dependencies dominate |
| no-op `cargo test --no-run` in a worktree | 410 s (load 55) | 1 s |
| after touching `src/lib.rs` | 213 s (load 52), 61 test links | 26 s (load 14), 1 test link |
| after touching one test file | one binary, 15-60 s | 7 s (load 13) |
| fast tier run (`-- --skip heavy::`, 692 tests) | - | 11 s (load 9), 31 s (load 34) |
| full suite run, `cargo nextest run` | 234 s (load 23) | 200 s (load 33), 269 s (load 20) |
| full suite run, `cargo test` | 618 s (load 13) | 445 s (load 12) |
| `cargo test --features dev` as the pre-commit hook ran it | 905 s (load 13): 267 s relinking, 618 s running | fast tier: 11-31 s plus a build |

The full suite in one libtest process is about twice as slow as
nextest's process per test, probably because the heavy tests then share
one process's task pools and allocator. Hence nextest for the full suite
(the pre-push hook uses it when installed).

Linking the test binary (1.0 GB), replaying its link command: LLD (rustc's
default here) 3.0 s, mold 3.0.0 1.6 s; without debug info 0.8 s and 0.4 s.
With one test binary that is a second per change, so mold stays an
opt-in line in `.cargo/config.toml`. Dropping dependencies' debug info
(`split-debuginfo` or `debug = "line-tables-only"`) would save about as
much but costs a full rebuild and backtraces into Bevy; not done then
(done 2026-10-27 for tests, and mold in the agents' slots: "Warm slots"
below). sccache (a compile cache shared by worktrees) could make a new
worktree's first build cheaper; not tried then (docs/OBSERVABILITY.md 3e).

### The heavy tier (2026-10-14)

Measured with `cargo nextest run --features dev` (all 1,504 tests
passing), a worktree's own `target/`, other agents building (load 24 at
the start, 4-13 at the end): **1,812 s wall**. The heavy tests' times
summed to 3,468 s of the suite's 4,314; with at most 2 heavy tests at
once, the wall time was about that sum / 2. A temporary timing line in
the CS:S loader (`cs_source::bsp::load_level_bytes`, its `LoadTimer`
total per test) put converting maps at 1,157 s of the heavy 3,468 (a
third): nearly all of `map_community` (203 of 235 s) and `map_stock` (62
of 68), over half of `map_de_dust2` (161 of 283: 64 tests, each loading
dust2), a fifth of `map_courses` (212 of 1,067); the rest is
simulation. Peak RSS per heavy test (sampled each second from
`/proc`): median 1.1 GB, a quarter over 1.7 GB, ten over 3.3 GB (the
soak 6.6 GB with five apps, `map_stock::water_materials` 4.8 GB,
surf_sedona's 1 GB of textures 4.7 GB).

What changed:

- The tests that looped over maps run one test per map: `map_ladders`
  (18 stock maps; `listed_maps` for `CHIMERA_LADDER_MAPS`) and
  `map_surf`'s rides (19 maps; `listed_maps` for `CHIMERA_SURF_MAPS`).
  `map_de_nuke`'s ladder tests climb in their worlds on threads (each
  try already built its own world from the one loaded map). The
  courses (`map_courses`) were already one test per map; a course's
  checks share one world's state, so they stay one test.
- `.config/nextest.toml`: 8 tests at once, 5 heavy slots; the tests
  over 3 GB take 2 slots, de_nuke's threaded ones and the soak 3; the
  soak and the three longest courses start first (`priority`); a test
  is called slow after 120 s and killed after 20 minutes.

Result, the same box and similar load (32 at the start, 10 at the end):
**1,009 s wall** (-44%), heavy tests summing to 3,457 s, swap not
growing (at least 2.8 GB available throughout; the 2-slot run before
touched 69 MB with the other agents' builds). Again after merging main's
render move (1,541 tests, load 16 at the start, 5 at the end): 896 s,
heavy tests summing to 3,035 s, at least 4 GB available. The slowest
tests left:
the network soak (178-266 s by load; one run of map changes, not
splittable), `kz_bhop_izanami` (84-161 s) and `kz_bhop_skodna` (60-129
s), `map_flows::jacks_every_game_keeps_players_in_the_map` (54-104 s),
de_nuke's ladders (72-96 s). Next: a converted-map cache or a headless
load path (architecture-review.md, P4b step 2) would take most of the
remaining third spent loading.

### The push hook (2026-10-19)

Pushes took 20-60 minutes: the selection fell back to the whole suite
for a lockfile change or when the compared commit wasn't in the clone,
and Combat Arms' map tests took the box's memory. At 0a9bd397 a Combat
Arms map load (`world::load_from`) took ~17 s alone and peaked at ~10
GB (9.6 GB for WAREHOUSE, sampled from `/proc`), 15 of the 17 s in
`view_model::add_view_models` (every weapon's view model, for every
map), and `heavy::world::every_plain_map_converts` loaded the 33 plain
maps in one test (1,050 s at load ~20) beside up to four other such
tests. Loading maps on threads in one process didn't help then (5 maps:
75 s on five threads, 83 s on one, 16 GB). Main's keyed-install work
then gave the map sweeps `load_world` (no view models: 1.3-2.3 GB, the
plain maps in 23 s) and split the keyed maps per archive. Then
view models loaded on demand ("Combat Arms map loads").

What changed here:

- `.config/nextest.toml`: hung tests killed after 5 minutes (12 heavy,
  30 the soak) instead of 20; Combat Arms' map sweeps (two loads at
  once on threads) take two heavy slots and start early.
- `affected`: a Cargo.lock change selects the crates whose resolved
  dependencies changed (`lock_affected`; e42d8b89's, the V2 cipher
  crates: 5 crates instead of every one); a compared commit missing from
  the clone is fetched from the pushed-to remote first instead of
  failing the selection; a selection of everything says so with every
  reason.
- The capacity test reruns itself in a child process under `cargo
  test` (its instruction budget counts the whole process; counting the
  server's own thread instead isn't steadier, its share of the parallel
  work grows with load); the soak's frame-time growth check uses CPU
  time, not wall time.

Full suite, `cargo nextest run --features dev`, own `target/`, other
agents building:

| | before (0a9bd397) | after (merged with main) |
|---|---|---|
| wall | 2,083 s (load ~20); 2,969 s (load 13-34), the map test killed at 20 minutes | 1,757 s (load 15-25), 2,833 tests |
| longest test | 1,050 s / 1,202 s (killed): Combat Arms' plain maps | 358 s: the soak |
| Combat Arms' heavy tests, summed | 1,895 s | 261 s |
| least memory available | 0 MB, 27 GB of swap used | 10.5 GB |

The heavy tests now sum to ~5,500 s of the suite's ~7,200: with five
heavy slots the wall time can't go much under ~1,100 s without more
slots (memory allowed it in the last run) or cheaper map loads.

### nextest on the Windows fast tier (2026-10-30)

`cargo nextest run` runs test binaries in parallel; `cargo test` runs one
binary after another. On the Windows box (Ryzen 7 7800X3D, 8 cores / 16
threads, 32 GB), a warm target, `CARGO_BUILD_JOBS=8`, three alternating
rounds of the same 4,020 fast-tier tests (601 heavy tests filtered out),
one failure in both modes (Combat Arms' `inventory::t36_rate_cap`):

| round | `cargo test` | `cargo nextest` |
|---|---|---|
| 1 | 113.75 s | 115.84 s |
| 2 | 104.39 s | 114.29 s |
| 3 | 104.71 s | 114.57 s |
| median | 113.75 s | 115.84 s |
| mean | 107.62 s | 114.90 s |

No gain: by mean nextest was 7% slower, by median 2%. Load was not
recorded; the rounds alternated so both modes saw the same conditions. The fast tier's
wall time is a few long tests (`party_kit`'s net tests, `combat_arms`'
inventory, `repo_checks`' architecture) and `.config/nextest.toml` caps
it at 8 test processes, so running binaries side by side does not shorten
it. Keep `cargo test` as the Windows default. nextest stays the Linux
full-suite runner (above), and the affected runner's `-Nextest` option
filters by module: `scripts/affected-tests.ps1 -Files
chimera/client/tests/it/metrics.rs -Nextest` ran 39 tests (`repo_checks`
plus `metrics::`), 12 heavy skipped, 5.9 s end to end once warm.

Warming the `-p` configuration after a workspace-wide build cost one
1 m 33 s rebuild of 8 workspace crates; repeats then took 5.9 s, and a
switch back to the workspace build cost a 3.7 s `repo_checks` relink.
That is a one-time switch per revision, not a per-run cost.

### Warm slots, a lighter commit, push first (2026-10-27)

A one-line fix took ~45 minutes to land: a new worktree's first build
10-20 minutes, the pre-commit hook 5-15 (every affected crate's fast
tier), the pre-push hook ~38 (15 compiling, 23 running 3,763 tests).
Measured on the dev box (12 threads, 31 GB) with three other agents
building and testing (load given with each number).

What changed:

- Build slots (`tools/agent-slot.sh`, docs/OBSERVABILITY.md 3e):
  worktrees at fixed paths whose `target/` stays warm, incremental
  builds on, sccache for the registry crates.
- The hooks build with the agents' settings (`agent-env.sh`) in agent
  checkouts. Before, the agent built with `CARGO_INCREMENTAL=0` and the
  hook with cargo's default (incremental): the profile differs, so each
  workspace crate was built twice per change, once by each (checked on
  repo_checks: switching recompiles, switching back doesn't).
- pre-commit checks the affected crates' tests (`cargo check --all-targets`:
  no codegen or linking; `CHIMERA_COMMIT_DEPENDENTS=build` builds them) and
  runs only
  the changed crates' fast tier and repo_checks (`affected --direct`).
  The old hook on a commit that selected everything (it changed the
  hooks): 6,629 s (load 30-56), the whole fast tier from a worktree's
  cold workspace crates.
- pre-push keeps identity, personal traces and repo_checks; the build
  checks and the affected tests (heavy too) run after the push,
  `tools/post-push-tests.sh`, in their own warm checkout.
- `[profile.test] debug = "line-tables-only"`.
- mold for links, in slots (where `ld.mold` is installed).

| `cargo test -p chimera_sim --features dev --test it <one test>` | time |
|---|---|
| new slot, cold (483 units; sccache for registry crates) | 1,964 s (load 10-56) |
| one-line change in chimera_sim, slot (incremental) | 5.5 s, 4.3 s (load 9-12) |
| the same with `CARGO_INCREMENTAL=0` (a worktree's settings) | 25.0 s, 46.6 s (load 6-9) |
| second agent: new branch from main (31 files ahead) in the slot, one-line change | 28.5 s, then 6.3 s (load 9-10) |

Debug info in test builds, `cargo test --workspace --features dev
--no-run` in a slot (incremental), after a one-line change in
chimera_core (19 units: every test binary relinks), interleaved:

| | full debug info (before) | `line-tables-only` |
|---|---|---|
| rebuild after the change | 383 s (load 18-23), 427 s (load 20-23) | 197 s (load 13-20), 237 s (load 18-20) |
| the build's artifacts (`target/debug/deps`) | ~68 GB, plus ~15 GB incremental | +23 GB beside them |
| a test binary (cs_source's) | ~1 GB | 478 MB |
| switching profile: every unit again (528) | | 4,781 s once (load 22-48) |

The hooks, a one-line change in chimera_sim staged in a warm slot (load
25-35):

| | time |
|---|---|
| old pre-commit (`CHIMERA_COMMIT_TESTS=affected`): 20 crates' fast tier, 3,374 tests | 749 s |
| new pre-commit: those 20 crates' tests built, chimera_sim's and repo_checks' run, 118 tests | 638 s right after a profile switch, 226 s the next change |
| new pre-push (identity, traces, repo_checks) | 38 s |
| `post-push-tests.sh`, nothing to test but repo_checks: its build checks from a test-only slot | 1,376 s (the first run; the checks' builds then stay warm) |

At the time of this measurement, `cargo run`/`build` kept full debug info,
so a slot that also ran the game built the dependencies a second time.
The default dev profile now uses line tables too (see below).

From 2026-10-30 the dependents are type-checked with
`cargo check --all-targets` instead of built; the 226 s row is the old
behaviour and the new number is still to be measured
(docs/plans/active/test-speed.md).

Linking cs_source's test binary (478 MB, line tables), replaying its
link command three times under load 33: LLD 4.3, 2.8, 3.5 s; mold 2.5,
2.3, 3.0 s. Small per link (every test binary relinks after a change
below it), so mold is only in the slots' environment, where the one full
rebuild a rustflags change costs comes with a new slot anyway;
`.cargo/config.toml` stays without it (Windows and other clones).

### Sharing dev and test artifacts (2026-10-30)

For Windows test-run selection, see the repeated
[affected-test measurements](plans/active/test-selection-verification.md):
median 211.55 s for the full fast tier versus 15.48 s for a client-test
scope, including selection overhead. Use `scripts/affected-tests.ps1
-Files chimera/client/tests/it/metrics.rs` for that scope, or `-Staged`
for staged changes. This speeds up narrow test runs, not compilation;
the report includes the failures and limitations.

The dev and test profiles both use `debug = "line-tables-only"`. Ordinary
`cargo build`/`run` and `cargo test` can reuse dependency artifacts when
their features and other compiler settings match. Backtraces retain file
and line information; inspecting local variables requires full debug info:

```
cargo run --profile devfull --features dev
```

`devfull` inherits dev's optimization and incremental settings but uses
`debug = 2`, with its own `target/devfull` directory. Tests already used
line tables, so their debug-info setting is unchanged. A dev-only cache
will need one rebuild when adopting the default; keep the warm target.
This does not eliminate rebuilds caused by different feature sets or
changes to foundational workspace crates.

Windows measurement, same warm worktree, `cargo test -p source_formats
--no-run` followed by `cargo build -p source_formats` (Cargo JSON artifact
records, unchanged sources):

| dev debug info | build time | rebuilt artifacts | fresh artifacts shared with the test build |
|---|---|---|---|
| full (before) | 16.78 s | 21 | 4 |
| line tables (after) | 4.35 s | 1 | 24 |

The remaining compiled artifact was `source_formats` itself; all 24
dependency artifacts were fresh and their paths matched the test build.
This measures the first test-to-dev transition, not a no-op build or a
whole-game speedup. Both profiles cache subsequent builds; matching them
avoids paying for a second dependency build when first switching modes.

### The first crate: `source_formats` (2026-10-14)

The repository became a workspace and the pure-data readers moved into
`source/formats` (docs/ARCHITECTURE.md, "Crates"). Measured in a
worktree's own `target/` with sccache, `CARGO_BUILD_JOBS=4`, other
agents building (load 9-16), one edit each:

| Edit | Command | Time |
|---|---|---|
| a function in `source_formats` | `cargo test -p source_formats` (build and run its 15 tests) | 0.3-0.9 s |
| the same | `cargo test --features dev --no-run` (the whole workspace) | 34-38 s: it and `mashup` rebuild |
| a leaf in `mashup` (`games/cs_source/sky.rs`) | `cargo test --features dev --no-run` | 33-57 s |

Running the fast tier then adds about two minutes (`it` 117-154 s at
load 15-20), so checking a format change goes from about three minutes
to a second. A change in `source_formats` still rebuilds `mashup` above
it (a dependent crate always does), so the hooks still test both; the
win grows as more crates are cut and edits land in crates with fewer
dependents (engine-split.md, "Crates and compile time").

### `chimera_world` and `chimera_present` (2026-10-14)

`map` moved into `chimera/world` (15.6k lines), `render`, `audio` and
`map_plugin` into `chimera/present` (20.7k); `mashup` keeps 134k. Same
setup as above (own `target/`, sccache, `CARGO_BUILD_JOBS=4`, other
agents building), a new `pub fn` appended to one file, then reverted and
rebuilt untimed before the next sample:

| Edit | Command | Time (load) |
|---|---|---|
| a leaf in `render` (`render/fish.rs`), before the cut | `cargo test --features dev --no-run` | 210 s (11-33) |
| a leaf in `render`, after | the same | 30-72 s (14-27) |
| a leaf in `map` (`map/fish.rs`), after | the same | 37-64 s (15-28) |
| a leaf in a game (`games/cs_source/ui/window_icon.rs`) | the same | 127-176 s (13-27) |
| a leaf in `map` | `cargo test -p chimera_world --features dev` (build and run its 41 tests) | 8.6 s (32-34) |
| a leaf in `render` | `cargo test -p chimera_present --features dev` (99 tests) | 12 s (28-32) |

An edit in either crate rebuilds it and the crates above (`mashup`
recompiles incrementally, nothing in it changed); an edit in `mashup`
still costs `mashup`'s own 134k lines. The first `cargo test -p
chimera_world` builds Bevy once more (about 20 minutes at load 20): a
crate alone unifies Bevy's features without what `mashup`'s other
dependencies turn on, so it has its own Bevy build, cached afterwards.

### `chimera_sim`, `source_logic` and `cs_source` (2026-10-14)

`logic` moved into `source/logic` (24.6k lines), `weapon`, `movement`
and the rule-module interface into `chimera/sim` (8.1k), `mount` into
`chimera_core`, and CS:S's simulation with its rules and bots into
`games/cs_source` (39.5k: the game's parts, `rules`, `objectives`,
`bot`); `mashup` keeps 61.5k (client 23.3k, the CS:S front end 17.9k,
net 13.8k, the binaries). Same setup (own `target/`, sccache,
`CARGO_BUILD_JOBS=4`, other agents building), a new `pub fn` appended
to one file, then reverted and rebuilt untimed; two samples each:

| Edit | Command | Time (load) |
|---|---|---|
| a leaf in `chimera_sim` (`movement/noclip.rs`) | `cargo test --features dev --no-run` | 20 s, 126 s (9-32) |
| the same | `cargo test -p chimera_sim --features dev` (build and run) | 1.6-1.9 s |
| a leaf in `source_logic` (`logic/value.rs`) | `cargo test --features dev --no-run` | 19-28 s (11-23) |
| the same | `cargo test -p source_logic --features dev` (216 tests) | 1.8-2.5 s |
| a leaf in `cs_source` (`game/radio.rs`) | `cargo test --features dev --no-run` | 22-27 s (12-23) |
| the same | `cargo test -p cs_source --features dev` (135 tests) | 3.1-3.5 s |
| a leaf in CS:S's front end (`games/cs_source/ui/window_icon.rs`, in `mashup`) | `cargo test --features dev --no-run` | 18-41 s (13-23) (was 127-176 s) |
| a leaf in the client (`client/gamma.rs`) | the same | 77-78 s (13-23) |

A game edit's test build fell from 127-176 s to about 25 s; the crates
above an edited one recompile incrementally (nothing they use changed).
What stays slow is `mashup`'s own code (the client, net), the next cuts
(engine-split.md, "Remaining couplings").

### `chimera_client` (2026-10-14)

The shell's base (`front_end`, `fonts`, `hud_text`, `widgets`, `binds`,
`vgui`'s drawing: 4.2k lines) moved into `chimera/client`; the rest of
the shell and CS:S's front end stay in `mashup` (it needs `net`).
Same setup, a new `pub fn` appended, two samples each (load 4-17):

| Edit | Command | Time |
|---|---|---|
| a leaf in CS:S's front end (`games/cs_source/ui/window_icon.rs`) | `cargo test --features dev --no-run` | 18-20 s |
| a leaf in the shell (`client/chat.rs`, `client/gamma.rs`) | the same | 36-64 s (was 77-78 s) |
| a leaf in `chimera_client` (`fonts.rs`, `widgets.rs`) | the same | 15-37 s |
| the same | `cargo test -p chimera_client --features dev` (28 tests) | 2 s |

`mashup` is one crate less big but still one crate: a shell edit
rebuilds it whole (the front end with it), so the win is small until the
shell and the front end are crates of their own.

### `chimera_net` (2026-10-14)

`net` moved into `chimera/net` (13.4k lines; CS:S's round, bomb and
hostages went to `cs_source::game::net`, the movers to
`source_logic::logic::net`). Same setup, a new `pub fn` appended to one
file, then reverted and timed again (load 17-19):

| Edit | Command | Time |
|---|---|---|
| a leaf in `chimera_net` (`net/delta.rs`) | `cargo test --features dev --no-run` | 27 s, 32 s |
| the same | `cargo test -p chimera_net --features dev` (29 tests) | 6 s |
| a leaf in the client (`client/fonts.rs`, before it moved to `chimera_client`) | `cargo test --features dev --no-run` | 19 s, 23 s |

An edit in `chimera_net` recompiles `source_logic`, `cs_source` and
`mashup` above it (incrementally). The first `cargo test -p chimera_net
--features dev` builds its own Bevy (`-p` unifies only its features:
31 minutes on this loaded box); later ones take seconds.

### The shell in `chimera_client`, CS:S's front end in `cs_source` (2026-10-15)

The rest of the client shell moved into `chimera/client` (19.3k lines
with the base) and CS:S's front end with its HUD into `cs_source`
behind its `ui` feature (22.1k lines beside the simulation's 40.5k);
`mashup`'s library is the app's glue (1.4k lines; the binaries 5.6k).
Same setup, a new `pub fn` appended to one file, then reverted and timed
again (load 6-18):

| Edit | Command | Time |
|---|---|---|
| a leaf in CS:S's front end (`games/cs_source/src/ui/window_icon.rs`) | `cargo test --features dev --no-run` | 14 s, 15 s (was 18-20 s) |
| the same | `cargo test -p cs_source --features dev,ui` (249 tests) | 5 s, 5 s |
| a leaf in the shell (`chimera/client/src/client/chat.rs`) | `cargo test --features dev --no-run` | 21 s, 22 s (was 19-23 s, 36-64 s before `chimera_net`) |
| the same | `cargo test -p chimera_client --features dev` (85 tests) | 6 s, 3 s |
| a leaf in CS:S's simulation (`games/cs_source/src/game/pain.rs`) | `cargo test --features dev --no-run` | 23 s, 19 s (was 22-27 s) |
| the same | `cargo test -p cs_source --features dev` (136 tests, no `ui`) | 3 s, 3 s |

A front end edit no longer rebuilds the shell, an edit in the shell no
longer rebuilds `mashup`'s old 23k-line shell. A simulation edit
recompiles the front end too (one crate, `cs_source` with `ui` in the
game's build), which costs no more than the old `mashup` rebuild did;
`-p cs_source` without `ui` checks the simulation alone. The first
`cargo test -p cs_source --features dev` and `-p chimera_client` build
their own feature sets of the dependencies (9 minutes here for
`cs_source` on a loaded box); later ones take seconds. (Fixed by the
workspace-hack, below.)

### Workspace-hack (2026-10-15)

`cargo test -p <crate>` used to build Bevy and its neighbours again with
that crate's own feature set: 21 third-party crates (`bevy_reflect`,
`bevy_winit`, `indexmap`, `hashbrown`, ...) got fewer features than in the
workspace's build, and every proc macro's dependencies a different
debuginfo setting (cargo keeps a build-time dependency's debuginfo only
when a runtime dependency shares it, and `-p` has fewer). Two fixes:

- `chimera/workspace-hack` (`cargo hakari`, .config/hakari.toml): every
  member depends on it, so third-party crates build with the workspace's
  features whatever is selected. Hakari simulates each member with all
  its features, so Bevy's development features moved to `chimera_dev`
  (every crate's `dev`/`profile` turns them on through `chimera_core`),
  which hakari leaves out: the hack never turns on dynamic linking, the
  remote protocol or Chrome traces. `source_formats` and `source_tools`
  stay off it (no Bevy). The dedicated server's build (`-p
  cs_source_server`) keeps the same crates (only the hack added) with
  nearly the same features (`bevy_reflect`'s `reflect_documentation`,
  `bevy_utils`' `debug`).
- `[profile.dev.build-override] debug = false`: build-time units never
  carry debuginfo, shared or not.

Measured (load 9-26), the first `cargo test -p chimera_world --features
dev` after a warm `cargo test --workspace --features dev --no-run`:

| | Time | Crates compiled |
|---|---|---|
| before | 560 s | 134 (Bevy, wgpu, avian3d, ...) |
| the hack alone | 507 s | 57 (every proc macro, then Bevy) |
| the hack and the build-override | 3 s | 3 (the hack, `chimera_core`, `chimera_world`) |

Then `-p cs_source --features dev` 15 s (4 crates), `-p chimera_client
--features dev` 7 s (2), `-p source_formats` 5 s. The workspace's own
build after the change: one full rebuild (16 min here).

### Dedicated servers without Bevy's default features (2026-10-22)

The workspace's `bevy` has no default features (Cargo.toml lists the
simulation's: `bevy_asset`, `bevy_log`, `bevy_state`, `bevy_image`,
`bevy_mesh`, `bevy_camera`, `bevy_light`, `bevy_window`, ...), so
`cargo build --release -p cs_source_server` (or `-p combat_arms_server`)
compiles no `bevy_render`, `wgpu`, `naga`, `winit`, `bevy_pbr`, `bevy_ui`,
`bevy_audio` or `rodio`. What the clients need they ask for: `chimera_present`
and `chimera_client` take `bevy` with `default` and `wav` and `avian3d` with
`debug-plugin`, `bevy_picking` and `bevy_scene` (its default features minus
the solver's, which every build keeps); every simulation crate takes the same
two as dev-dependencies, so its own tests still share the clients' builds.
`chimera_world`'s `render` feature (turned on by `chimera_present` and by its
own dev-dependency) holds the three things that need `bevy_pbr`/`bevy_render`:
the greybox's `StandardMaterial`s, `MapAlpha::shader_alpha_mode` and
`MonitorFog::distance_fog`; the texture types come from `wgpu-types`
directly. Hakari leaves `bevy`, `bevy_internal`, `bevy_render`, `bevy_winit`,
`bevy_window`, `bevy_input`, `bevy_light`, `bevy_picking`, `wgpu`, `wgpu-core`
and `avian3d` out of the hack (a hack entry is a normal dependency of every
member: with them the servers compiled them all). `architecture::
server_builds_without_presentation` fails if a server's tree has Bevy's render,
window or audio crates again.

Measured (this box, `cargo build --release`, `-j 4`, load 19-28 from other
builds, sccache), before and after:

| | before | after |
|---|---|---|
| `cs_source_server` packages in `cargo tree` | 689 | 495 |
| `combat_arms_server` packages in `cargo tree` | 647 | 447 |
| `cs_source_server` binary | 115.3 MB (76.3 stripped) | 86.9 MB (58.9 stripped) |
| `combat_arms_server` binary | 117.4 MB | 89.0 MB |
| `cs_source_server` RSS after loading de_dust2 (8 players, idle) | 441 MB | 427 MB |
| both servers, release build from an empty target | 38.5 min, 502 crates compiled (cold sccache) | 32.3 min, 185 compiled and 294 from sccache (warm from the first build): not a like-for-like time, the crates removed are the render stack (about 190 of the 689) |

### Bots' sight in parallel (2026-10-22)

`bot::think` (games/cs_source/src/bot/mod.rs) starts with a pass that
finds each living bot's nearest living enemy in sight (nearest first, PVS
first, then a ray and the smoke test) on the compute task pool:
`par_iter_mut` over the bots, each writing only its own `Bot::sighted`,
which the loop after it takes. A bot's result depends on no other bot's,
and the sight used state nothing in `think` changes, so the
pick is what the serial loop gave, in any thread order (X2): the test
`bot_sight::a_crowd_sees_the_same_on_any_executor` runs twelve bots twice on
the single-threaded executor and once on Bevy's multi-threaded one and
compares targets and aim bit for bit. Combat Arms' bots (`bots::brain`) are
one exclusive system with state shared between bots' decisions; they stay
serial.

Measured on `heavy::net_capacity` (de_dust2, 4 clients, the rest bots in
deathmatch, a timer around the sight code alone; this box at load 7-34
from other builds, so only the sight timer, which counts its own wall time,
is read, not the tick's):

| characters | sight, serial | sight, parallel | tick instructions p50 (all threads) |
|---|---|---|---|
| 32 | 164 us a tick | 256 us (load 2x the serial run's) | 28.4 M, 26.6 M |
| 64 | 186 us | 121 us | 36.5 M, 38.2 M |
| 128 | | 216 us | 82 M, 82 M |
| 256 | 1,842 us | 1,019 us | 209 M, 201 M |

The sight is 3-5% of a 32-64 character tick (the PVS and the nearest-first
order keep it light on de_dust2: most bots have an enemy in view after a
trace or two), so the tick's median does not show it under load; at 256 the
server's main-thread tick p50 was 16.7 and 23.6 ms serial against 13.3 and
13.9 ms parallel in two runs each. The instruction count (the stable
figure) is the same to within 1-3%: the pool adds almost no work. The case
that motivated it (a minigame lobby, 64 bots in each other's PVS and in the
way of each other's rays, ~25 ms a tick) is not measurable here, its map
(mg_jacks_multigames_v1) not being on this box.
