# Performance

How to measure, what the numbers were, and what the map visibility culling
does; at the end, how long the tests take to build and run ("Test cycle")
and the edit-to-verified loop as a whole ("Iteration speed"). Tools are listed in [OBSERVABILITY.md](OBSERVABILITY.md) ("3c.
Performance").

**The rule: judge performance by deterministic counts, never by wall
clock.** A tick's or a frame's cost is the instructions its systems
retire in a repeatable scenario ("Work counts": `Sim::work`,
`metrics::work`, `perfgate`, `frame_bench`), which the same build repeats
to under 0.1% on an idle box and a loaded one. Wall time, CPU time and a
whole thread's instructions move with the machine (2-10x, 30%, 5-15%):
print them for reading, never assert on them or gate on them. A test
that compares costs uses `Sim::repeatable` and `Sim::work`, and its
failure prints the systems that grew (`WorkCounts::diff_table`).

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

- Work counts ("Work counts" below): `Sim::work(n)` in a headless test,
  `perfgate` for the simulation's scenarios, `frame_bench` for a frame
  with the renderer: instructions per tick or frame by system, the same
  on every run.

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
  against a stored baseline ("Regression gate"), by work counts since
  2026-11-10 ("Work counts").

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
  problem; `perfgate` counted every thread instead, and now counts the
  systems' own work ("Work counts").
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
game's single-threaded schedules, repeatable (`Sim::repeatable`). Since
2026-11-10 it counts work ("Work counts"): the user-space instructions
of each tick's systems on the schedules' thread, by system, and the
state hash after the run. (Before, it summed every thread's instructions,
`metrics::AllThreads`, with a counter group per thread that made the
kernel multiplex the counters.)

Five runs of the playtest build at 16f4ea34 plus the gate's changes,
three at load 25-35 and two more with 16 busy loops on top:

| scenario | instructions per tick p50 | total (2000 ticks) | spread of total | state hash | CPU ms p50 | wall ms p50 |
|---|---|---|---|---|---|---|
| greybox | 0.82M | 1.6564-1.6602G | 0.24% | the same in all 5 | 0.90-1.16 | 0.90-2.93 |
| de_dust2 | 4.52M | 11.1815-11.1843G | 0.024% | the same | 2.32-3.06 | 2.34-9.40 |
| mg_lego_multigames_v2 | 3.62M | 7.2684-7.2745G | 0.084% | the same | 1.74-2.50 | 1.75-7.70 |

The old gate's spread (six runs at load 30-33, `AllThreads`): 3.8%, 0.8%
and 2.0%, with an 8% tolerance for greybox. What is left on greybox is
Bevy's `mark_dirty_trees`, whose scope tasks wait for each other on
channels a varying number of times (about 1.3k instructions a tick).
So the gate fails on a rise of more than 1% in any scenario
(`tools/perfgate/baseline.json`, per build profile, noting the CPU and
the build: counts differ between profiles and CPU models), and prints
the systems that rose against the baseline's 80 costliest; a state hash
apart from the baseline's is printed too (the scenario played otherwise:
a change of behaviour, or something nondeterministic). Counts include
the counting's own ~2,000 instructions per system run.

- Check: `cargo build --profile playtest --features tools --bin perfgate &&
  target/playtest/perfgate` (exit 1 on a rise; `--scenario de_dust2`
  for one).
- A wanted rise (or a drop): `target/playtest/perfgate --update`, and
  commit `tools/perfgate/baseline.json` with the reason; `--tolerance
  N` with `--update` sets the default tolerance. Scenarios whose map
  isn't installed are skipped, and so is everything where counters
  can't open (Windows, `perf_event_paranoid` 3).
- Determinism: `perfgate --repeat` builds each scenario twice in one
  process, ticks them side by side and reports the first tick their
  state hashes differ and the entities that differ (works without
  counters, on Windows too). All three repeat over 1000 ticks.
- Checked: against a baseline lowered by 2% for de_dust2 (and its
  movement step by 10%) it fails (`+2.06%, tolerance 1%: FAIL`, exit 1)
  with the movement step on top of the systems' diff.

## Work counts (2026-11-10)

What judges a tick's or a frame's cost now: the user-space instructions
its systems retire in a scenario that plays out the same on every run,
counted per system (`chimera_core::metrics::work`). The same build and
scenario count the same to under 0.1%, idle or under load, so a test or a
gate can hold a tight bound, and a failure names the systems that grew.

### Why the earlier counts moved

`ultimate_gun_game`'s `session::a_session_of_matches_stops_making_tables`
failed under a suite's load ("a tick cost 3394439 instructions after a
match, 4303213 after seven more") although it counted instructions: the
main thread's, whose share of the work depends on scheduling. Measured on
the session's greybox match (8 Combat Arms bots, 60 ticks after a match),
the same build, each row a separate process:

| measure | idle | 16 busy loops |
|---|---|---|
| main thread, median tick (the test's patch) | 2.62-2.85M (8.4%) | 2.65-2.79M |
| main thread, mean tick | 3.03-3.23M (6.4%) | 3.03-3.17M |
| work counts, Avian's tree task on (first try) | 589-627M a window (6.3%) | |
| work counts, final | 316.52-316.65M a window (0.04%) | 316.39-316.73M (0.1%) |

(The first two rows already ran on one compute thread; on two, as before,
the spread was the ±15% the report gave.) The state hashes
(`Sim::state_hash`) of all nine runs were the same: the play was
deterministic all along. The cost was not, for these reasons:

- **Work moved between threads.** Bevy's parallel queries hand chunks to
  the compute pool and the main thread does what is left; Bevy's
  transform propagation runs workers that spin while they wait for each
  other (`propagation_worker`); Avian optimizes its collider trees on a
  compute task beside the narrow phase and solver
  (`ColliderTreeOptimization::use_compute_task`), so its work landed in
  whichever system the main thread was running (`respawn_dead` 5.6M
  against 7.9M over the window, the movement step 30.8M against 36.1M,
  with the same state). Counting every thread (`AllThreads`, the old
  `perfgate`) takes in the spinning and the pools' waking and stealing.
- **Task results landed on load-dependent ticks.** The navigation mesh
  (`NavGenTask`), Combat Arms' bodies, held models and view models on
  demand and the view models' timings are polled once a tick
  (`poll_once`), so the tick they land on, and all the play after it,
  depended on the task thread's speed; the stepped map spawn
  (`map::spawn::spawn_steps`) cut its steps by a wall-clock budget.
- **The kernel multiplexed the counters.** The games' `TickMetrics`
  holds a group of four hardware events on the schedules' thread; the
  old `perfgate` opened another group on every thread (`AllThreads`), and
  with the work counter that is more events on the thread than the
  Ryzen's six counters (one taken by the NMI watchdog): the kernel
  time-shares them and the reads are scaled estimates. The same play (the
  same state hash) then counted 0.5-6% apart between runs, spread over
  every system. `perfgate` now takes CPU time from the process clock;
  `work::multiplexed()` says when a count was an estimate (`Sim::work`
  and `perfgate` print a note).
- Not a cause here: std's randomly seeded hash maps cost a little more or
  less per run (probe lengths), under 0.01% of the window.

### What changed

- `metrics::work`: `count_systems(world)` puts each system of the
  schedules that have not run yet behind a wrapper (`Counted`) that reads
  this thread's instruction counter (one counter, `InstructionCounter`:
  a small `read`, no group) before and after the system and its
  commands, less the schedules it runs inside.
  Per thread: `set_enabled`, `WorkCounts::take` (a test's systems run on
  its thread). `WorkCounts::diff_table(&base, n)` prints the systems that
  rose most. The counting costs about 2,000 instructions per system run
  (a tick of the session: 4.1M counted against 2.8M uncounted), the same
  on every run: compare counted with counted.
- `harness::Sim`: one compute thread (`sim_task_pools`; parallel queries
  are plain loops, transform propagation spawns no workers), kept busy
  while counting (`work::set_enabled` holds it: `hold_compute_pool`;
  held for good it stopped any multi-threaded executor in the process,
  2026-11-11: the tasks scopes still spawn,
  Bevy's `mark_dirty_trees` and Avian's broad phase, run on the waiting
  schedule thread, counted with their system, instead of on a pool
  thread whose waking and stealing cost a varying amount); Avian's tree
  optimization in the tick's order; every system wrapped at the start. `Sim::work(n)` returns each tick's
  instructions, the archetypes after it (`steady_ticks`: those that made
  none) and the systems' counts; `Sim::repeatable()` (or
  `CHIMERA_REPEATABLE=1`) inserts `chimera_core::tasks::WaitForTasks`:
  task results land on the tick they're polled (`tasks::poll`) and a map
  spawns in one step; `Sim::state_hash()` hashes every entity's place,
  turn, velocity and health; `harness::own_process(test)` reruns a test
  alone in a child process under `cargo test` (the compute pool is the
  process's: another test's tasks would count).
- Tests on work counts: `session::a_session_of_matches_stops_making_tables`
  (the median tick making no archetype, match 7 under 1.25x match 1's,
  systems' diff on failure; a median, as each match's fights differ),
  `session::a_match_repeats_tick_for_tick` (three sessions in one
  process: the same state hash, the last two within 1%: 233,827,894 and
  233,828,785 instructions), `party_kit`'s `archetypes` (1.17x with 2,000 more archetypes,
  1.60x with 2,000 more tables) and `heavy::bot_cost` (systems growing
  from 2 to 16 bots printed always, no `trace` build needed),
  `cs_source`'s `map_logic::idle_entities_cost_a_tick_little`,
  `ultimate_gun_game`'s `heavy::session_maps`; two tests that timed with
  `Instant`: `chimera_sim`'s `spawns::facing_costs_well_under_a_millisecond`
  (instructions per call) and Combat Arms' ui
  `frame_cost::screens_that_are_down_cost_nothing_per_string`
  (instructions of 20 frames); `perfgate` ("Regression gate": 0.02-0.24%
  between runs, gated at 1%, `--repeat` checks determinism);
  `frame_bench` (frames, below: two runs of de_dust2 gave the same main
  schedule, 13.96M a frame, extraction, 1.25M, meshes drawn and bytes
  uploaded; the render world 11.27M and 11.31M: the GPU driver's command
  submission, `submit_pending_command_buffers`, runs a varying amount of
  user-space code).
- Frames (`ultimate_gun_game`'s `frame_bench`): the main schedule,
  extraction and the render world by work counts with the costliest
  systems, the meshes drawn and the bytes of meshes and textures uploaded
  a frame (Bevy's `RenderAssetBytesPerFrameLimiter` with no limit:
  counting only) as deterministic counts; GPU time from timestamp
  queries (`RenderDiagnosticsPlugin`, Vulkan and DX12) and wall time
  printed apart as timings.

### Held only while counting (2026-11-11)

The first version kept the compute pool's thread parked for good from
every `Sim`'s start. Bevy's multi-threaded executor runs its systems on
that pool, so in any process that also ran one (a test's plain `App`,
`Sim::multi_threaded`, a schedule a plugin made after the `Sim` set its
executors) the schedule waited for good: seven tests timed out after
merging (party_kit's `controls::` and `esc_menu::`, cs_source's
`bot_sight::a_crowd_sees_the_same_on_any_executor`; gdb: the test thread
in `MultiThreadedExecutor::run`, the pool's thread parked in
`occupy_compute_pool`). Now `work::set_enabled(true)` holds the pool
(`hold_compute_pool`) and `set_enabled(false)`, or the counting thread
ending, lets it go; `Sim::work` first puts any schedule made since the
start on one thread, and on a multi-threaded `Sim` counts without holding
(`set_enabled_sharing_pool`, not exact).

`heavy::nav_in_play` checked the task's first tick as a whole (60 M
main-thread instructions) and failed at 64 M: that tick is the match's
start (the bots joining and their first contacts: the narrow phase 14.9
M and `prepare_spots` 13.2 M, which the one compute thread now runs on
the schedules' thread), while starting the task itself (`ensure_nav`, the
collision's copy) costs 1.3 M. It now counts work (`Sim::work` per tick:
its old `ThreadCounters` group beside `TickMetrics`' was multiplexed too)
and bounds the task's start at 5 M (a flood on the main thread costs tens
of millions a tick) and the worst tick after the tenth at 60 M (54 M
measured), printing the costliest systems on a failure.

Combat Arms' `heavy::map_load` judged loads by wall seconds (20 s and
30 s, 50 ms for a cached view model) and failed once at 20.3 s for
junk_flea after the merge. Not a slower load: its load runs on
`std::thread::scope` threads of its own, untouched by these changes, and
measured 3.0-3.3 s idle here, 54.7 G instructions on every run (the same
under the after-push run's load, which took 20 s). It now counts
instructions (`InstructionCounter::current_with_children`: the loading
thread and the helpers it starts) against budgets about 1.4x what was
measured, and the spawn's ticks by `Sim::work`.

### Windows

Hardware counters are Linux's (`perf_event_open`); Windows has thread
CPU time only, in 15.6 ms scheduler steps, which moves with load as wall
time does. There `Sim::work` and `work::now` return None, the tests that
compare costs print `skipping ... no hardware counters` and pass, and
`perfgate` exits 0 with `skipped`. Determinism (`Sim::state_hash`,
`a_match_repeats_tick_for_tick`'s hash check) holds on Windows too. Judge
a change's cost on Linux.

### Left

- Draw calls themselves aren't counted (Bevy has no counter; meshes
  drawn stands in) and neither are GPU work counts: wgpu's pipeline
  statistics (`PIPELINE_STATISTICS_QUERY`: vertex and fragment shader
  invocations) would be, but need the feature asked for at device
  creation.
- The client's live readouts (`chimera_perf`, `refcmp bench`, the hitch
  log) still count whole threads (`metrics::TickMetrics`,
  `frame_metrics`): for reading a run, not for comparing builds.
- Tests on absolute instruction budgets of a single frame or tick with
  the real loading path (`late_build`, `heavy::rung_frames`,
  `heavy::map_swap_frames`, `heavy::nav_in_play`) count the main thread
  only and judge loads that run on tasks by design: far above their
  spread, but not work counts.

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

## Combat Arms character changes (2026-11-09)

A rung change that puts on a body not yet loaded (Ultimate Gun Game, seed
36336, de_dust2, the dev profile on the dev box at load 50-80, walking the
ladder with `ladder_rung` in third person, `chimera_hitch_ratio 2`):

| | before (load 50) | after (load 15-20) |
|---|---|---|
| The frame of the change | 46-78 ms against a median of 18 (main thread 40-61 ms CPU, 130-195 M instructions in 1-2 ticks) | none logged on 11 of 13 rungs; at most 37 ms (31 ms CPU, one rung, the first into Combat Arms); a tick is its usual 10-30 M instructions |
| The frame the body arrives, 0.6-1.2 s later | 63-98 ms (68-76 ms CPU, 150-170 M instructions) | none at the change: it arrived with the ladder (`build_late_bodies` ~5 ms a body) |
| What shows meanwhile | the old body on the new skeleton: limbs garbled, floating; the box gun | the old body on its own skeleton, or the new one already |

Where the time went: `CaAnim::with_family` cloned the whole animation library
(`Rig::list`, now shared: 100+ M instructions, every character change, loaded
or not); `append_late_characters` cloned every character model of the map
(PreUpdate 18 ms, now only when another holds the list).

A `screenshot` costs the main thread about 1 G instructions and 100 ms (it
landed on these frames while measuring: don't count it).

Preloading (`characters::preload`, the match's ladder: here 10 bodies and 26
weapon keys): a ladder is made as its match starts, once the map is in, so
the load runs in the first seconds of play, not behind the loading screen:
about 13 s at load 70 from the moment the ladder is known. A body's load on
one thread is 560 ms on average, 1,015 ms at most, its textures 39 MB
(`heavy::ladder_bodies`, 41 bodies at load 60). Memory: the process grew about
1.6 GB per ladder (4.6 GB in game, 7.9 GB after a second ladder's load, load
20; about what walking the ladder on demand reached before). Nothing is
forgotten across ladders in a session (`characters::preload`'s shortcut): see
the backlog. CS:S's and HL2:DM's rungs add nothing: their kits load with the
map.

## Nothing blocks the main thread (2026-11-09)

The user's bar: a load never costs a visible frame. Measured with a new per-frame log
(`CHIMERA_FRAME_LOG`, `tools/frame_cut.py`, OBSERVABILITY.md 3c) on the
`profiling` build under `perf`, Ultimate Gun Game, seed 36336, de_dust2, 1280x720, vsync
off, first person, bots playing, load 30-60: the whole walk of the ladder's 13 rungs, then
`map` to dm_lockdown, warehouse and cs_office. Numbers are the worst single frame, in
user-space instructions on that thread (the render thread's alone are stable to about
1%; the main thread's move 5-30% with what it picks up from the task pools, and a frame
after a stall runs many ticks, so "less ticks" takes out the fixed ticks').

**A trap while measuring:** the first log wrote its lines from the render thread, and
under the box's load a `write` to the root disk sat 200-560 ms in the file system's
journal (`handle_reserve_ticket`, read from `/proc/<pid>/task/<tid>/wchan`): the render world's `Cleanup` showed
500 ms of wall time at 0.1 ms of CPU, and the main thread, waiting for it, then ran 16-17
catch-up ticks. Stalls of that shape that come and go on this box may be its disk, not us:
the same goes for anything that logs from a world's thread into a file. The log now
writes from a thread of its own, and runs keep the game's own log on tmpfs.

What was on the main thread (each with the fix):

| What | Worst single frame | Fix |
|---|---|---|
| The bots' navigation mesh (`ensure_nav`, `nav_gen`: rays and box sweeps of a flood) in one tick when bots first play on a map | 3-6.9 G, a 1.5-2.5 s frame in play | The flood on a task (`nav_gen::NavGenTask`: the `NavGenJob` on the async compute pool, yielding every 500 cells, against `CollisionSnapshot`, a copy of Avian's collider trees, poses and shapes taken once), the same mesh; the main thread only polls it, behind the loading screen (`MapHooks::settle`) after a `map` command or a server's map change, or in play on a map nothing loaded behind a screen (the command line's `--map`). There the bots stood while it was flooded a slice a tick (`CELLS_PER_TICK`, 22.5 M a tick median): de_dust2 headless, a bot first walking 7.3-7.6 s after the round's start (10-13 s in the real app at load 40-60), now 0.43-1.33 s, the main thread 0.2-0.5 M a tick while it waits (one tick taking the snapshot, the rest polling). A front end only (`SliceFloods`, the shell's: a listen server too); a dedicated server floods in one tick |
| The spawn spots' mesh and tests (`spawns::build`) in the first respawn | 1.2 G | the same, behind the loading screen (`settle_spots`) or a slice a tick from the match's start (`prepare_spots`); the same spots; the map's spawn points for a respawn before they are ready |
| A game's HUD on its first frame in play: Bevy makes a glyph atlas (a 1 MB image) for each new font and size its texts use, and rasterizes them, on `text_system` (the 37 ms rung: the first into Combat Arms) | 116-122 M less ticks, about 30 ms | every game's HUD shown for 4 frames behind the loading screen (`client::hud::warm_huds`, `gameplay::HudWarm`): 70 M. Texts a HUD shows only in some states (a weapon's name, a lead message, the kill feed, the scoreboard): each HUD's fonts and sizes declared and laid out ahead, the atlases filled on a task (`client::text_warm`, below) |
| Building a late model (`build_late*`: texels and mip levels, vertices, a skinned mesh's bounds, a deep copy of the view model) | 170 M main, 324 M render in one frame (a frame of up to 336 ms wall) | `render::late_build`: all on a task with reserved handles; only `Assets::insert` on the main thread, 4 MB and 12 materials per queue per frame |
| Keeping those meshes in the main world: extraction copies each whole | in the above | meshes the main world doesn't read are `RENDER_WORLD` only (a face with flexes stays) |
| The rung's weapon reading its view model for its timing (`weapons::timing`, an LTB out of a keyed archive, in the middle of a tick): **a 30 ms frame on most rungs into a new Combat Arms gun** | 46-102 M | `timing::read_ahead`: the ladder's read on tasks when the ladder is known. Off the ladder it still reads on the frame |
| A body's rig replacing its alias in the late index (freeing a whole animation library) | 48-78 M | `map::drop_later` |
| A map's physics props settling after it spawns, every shadow redrawn at once (`update_prop_shadows`) | 300-500 M for several frames | `REDRAWS_PER_FRAME` (6), each prop's latest pose; the rest wait |
| A map's old models freed in its swap frame | 300 M of 2.4 G | `map::drop_later` for map-scoped resources |
| A ladder's draw looking weapons up in a list of thousands (`WeaponSet::get`, `rungs::draw`) | 367 M at a match's start | an index by id: 100 M |
| A game's HUD font files made new fonts each time the HUD came back, and so new glyph atlases (`take_hud_fonts`) | | once per file (`UiFonts::game_loaded`) |

Before and after, each rung's worst main frame less ticks (M instructions) over the second
after it, and the worst whole (ticks and all):

| Rung | Before, less ticks | whole | After, less ticks | whole |
|---|---|---|---|---|
| 1 hl2dm:weapon_pistol | 24.2 | 46.4 | 18.9 | 26.7 |
| 2 combat_arms:p226_og_cs | 117.0 | 141.1 | 116.2 | 141.5 |
| 3 combat_arms:scorpion_mod | 75.8 | 126.0 | 23.4 | 48.7 |
| 4 cs_source:weapon_tmp | 36.5 | 71.7 | 48.0 | 70.6 |
| 5 combat_arms:ice_dragon_m4a1 | 82.1 | 108.6 | 34.6 | 66.3 |
| 6 combat_arms:harlequin_s_arx-160 | 102.5 | 123.6 | 32.6 | 60.9 |
| 7 combat_arms:starcombat_xm8 | 40.7 | 87.0 | 21.9 | 49.9 |
| 8 combat_arms:groza_bloodhound_lv.4_limit_ | 66.9 | 85.0 | 26.9 | 36.1 |
| 9 combat_arms:trg-21_event | 58.8 | 82.5 | 21.5 | 41.1 |
| 10 combat_arms:assassin_bowgun_nor | 31.9 | 59.6 | 26.1 | 43.3 |
| 11 combat_arms:rms12 | 40.7 | 92.5 | 62.2 | 76.1 |
| 12 combat_arms:uzun_tamburali_bombaatar | 46.4 | 79.6 | 36.3 | 84.6 |
| 13 combat_arms | 33.6 | 69.4 | 37.7 | 48.3 |

With the HUDs warmed behind the loading screen (a `map` command first, as a game started from the menu does; a run at load 40-60 that ran 11-17 catch-up ticks in 600 of its frames, so read the "less ticks" column): rung 2 70 M (text 4 of its 70 samples, 17 of 53 before), the others 23-76 M. Rungs 4, 11 and 13 rose in one run and not in the next: their worst frames were text making atlases for a string at a size not shown before (rung 11's 62 M), and rung 13 drew another gun each run (the ladder is drawn again when its match restarts). The once-a-second `chimera_perf` readout (`perf_report`, computed for `bugreport` even when nothing shows it) is 5-20 M in whichever frame it lands.

Behind the loading screen (`client::console`'s `Settling`, at most 10 s): dm_lockdown 3.2 s, warehouse 0.9 s, cs_office 3.9 s, de_dust2 6.6 s at load 40-60; the screen turned over at a median 40-53 ms a frame meanwhile (its worst frames the swap's catch-up ticks, 470-760 M; since, a frame behind the loading screen advances the simulation by one tick at most: "A map's swap", below).

Other cases (worst frame anywhere in the run, M instructions):

| Case | Before | After |
|---|---|---|
| A body or gun arriving from the ladder, main thread | 170 | no frame stands out: `build_late*` is under 3 M with a body's 39 MB (`tests/it/late_build.rs`: 3.5 M, with 432 MB of texels) |
| The same, render thread | 324 (336 ms wall) | 49.5, not a load's: p99.9 32.7 |
| The first ticks on a map, whole frame | 6,857 / 4,760 / 3,711 | 1,327 at startup, 918 at most afterwards |
| A map's swap frame (`change_map_with`) | 2,507 (5.6 s wall at load 80) / 2,354 / 1,454 | 2,118 / 2,079 / 918 (0.3-0.5 s wall at load 30): less the frees, and no nav in the next frames; since spread over frames ("A map's swap", below) |
| Frames over 100 M in the maps run | 615 of 5,718 | 245 of 5,908 |
| The render world's first frame of a map | 1,236 / 1,036 | 1,213 / 1,035 |

What did not move, and its size (`docs/backlog.md`, "Nothing blocks the main thread"):

- **A map's swap** (below, "A map's swap"): spread over 4-10 frames behind the loading screen
  now; what is left in one frame is the frame the whole map goes into the world (the
  spawn's commands together: 90-270 M dev-build main, the colliders' mass properties in
  Avian's insert hook a third of it, which Avian's API gives no way around; the render
  world's first look at every new entity, 220-350 M offscreen) and the first tick on the map
  (45-270 M dev, was 110-335 M: no longer the map's colliders' mass properties again, but the
  colliders the tick's own spawns make from the map's entities (the logic's world, pickups),
  their props' shadows redrawn as they settle, the first physics step).
- **A HUD text at a font and size nobody declared**: still 20-60 M on its first frame; each one logs `text warm: N glyph atlases made on a text's first frame (... at [sizes] px)`, so a playtest's log names what is left (`client::text_warm`, below). Declared now: the shell's kill feed, minigame scoreboard, minigame panel, match-end screen, chat, hints and `game_text`, every text that exists empty (callouts, the respawn countdown, hidden HUDs' labels), and Combat Arms' HUD (its panels' fonts and each text of the install's HUD, score, result, team list and death screen layouts). Since declared too: every game menu layout's label, button and rich-text fonts (`client::vgui::warm_fonts`: CS:S's buy and team menus, scoreboard, spectator bars, message of the day), CS:S's texts spawned on demand (`cs_source::ui::ON_DEMAND_FONTS`: the radio and plain menus, the target's name, the centre print, the radar's and overview's place names, the voice and vote panels, the plain scoreboard; its death notices' names and icon glyphs when the app has no shared feed) and the games' kill-feed icon fonts and glyph kill marks at the sizes the feed draws them (`WeaponIcons::register_glyph_fonts`: CS:S's and HL2:DM's). Not declared: other games' HUD texts drawn by `hud_text` (its own rasterizer, not Bevy's atlases), and any text whose size no source names (the log says which).
- A match's start: 100 M (`ladder::start`, drawing every rung's extras), a `+map`'s first listing of Combat Arms' maps
  (292 M, opening the install's archives: `read_map_names`, once a session), and the first ladder's
  `markers::show` opening its archives on the frame (about 40 M).
- A weapon off the ladder: its timing and its first draw still read or wait as before.
- Pipelines: no frame in these runs shows pipeline creation on the main or render thread (Bevy compiles them on
  tasks, and a new material draws once ready); not measured on Windows, where DX12's and the
  driver's costs differ.
- Dedicated servers flood at once: a server's first bot tick is still 3-7 G (nobody watches its frames).

Guards: `chimera_client`'s `each_hud_is_shown_then_the_playing_one_stays` and `text_warm` (`chimera/client/tests/it/text_warm.rs`: after the warm-up the feed's first kill (its gun's icon glyph and headshot mark too), the board, the panel and a game menu's first opening make no atlas, Bevy's `FontAtlasSet` counted; every atlas put in ahead was under Bevy's own key; an undeclared size is noticed; in play no frame lays out more than `GLYPHS_PER_FRAME` glyphs), `combat_arms`' `ui::text_warm` (its drawn HUD, with and without the warm-up: the feed's first line makes an atlas only without), `chimera/present/tests/it/late_build.rs` (three models far over any game's size come
in with no frame over 8 M instructions; here 3.5 M), `apps/ultimate_gun_game/tests/it/heavy/rung_frames.rs`
(with the installs: de_dust2 headless, a bot walking a seeded ladder, each rung's worst
main-thread frame under 45 M, ticks included; measured 12.5-27.7 M. It covers the
simulation and the bodies' and held models' drawing, not the HUD, the render thread or
install guns: that ladder draws only Combat Arms' own guns, so the real-app numbers above
remain the check for those), `chimera_sim`'s
`spots_made_a_slice_a_tick_are_the_same_spots`, `a_sliced_flood_makes_the_same_mesh` and
`a_flood_on_a_task_makes_the_same_mesh` (the snapshot's queries make the world's mesh, bounded or
not, ignored colliders alike), and `apps/ultimate_gun_game/tests/it/heavy/nav_in_play.rs` (with the
installs: de_dust2 headless, no loading screen, a bot walking within 6 s of the round's start, no
tick over 60 M meanwhile).

### HUD texts' glyph atlases ahead (`client::text_warm`)

Bevy 0.20 keeps glyphs in atlases (`FontAtlasSet`, keyed by the font's data, its
physical size, variations, hinting and smoothing) and fills them on the main thread in
`text_system`, the frame a text first lays out: a new font and size makes a 512 x 512
RGBA atlas (`FontAtlas::new`: 15.7 M instructions, nearly all filling the 1 MB image)
and every glyph is rasterized with swash the first time it is drawn at that size
(about 0.35 M a glyph at 30 px, less smaller). Its types are public (`FontAtlasSet`,
`FontAtlas`, `FontAtlasKey`), but laying glyphs out takes the pipeline's own
shaping, so the warm-up uses the pipeline itself:

- **Declared:** each HUD registers a source (`TextWarmAppExt::warm_texts`, a function
  from the world to the fonts it draws at the window's size, from its layout data and
  scheme fonts), and every text that exists but has laid out nothing yet counts too.
- **Laid out ahead:** each new font and size, with printable ASCII and the source's own
  characters (the guns' names), by a hidden text off screen: the same face, physical
  size, hinting and target as the HUD's, so the glyphs land where its text looks.
- **Atlases off the main thread:** empty atlas images are filled on tasks
  (`AtlasPool`, 4 ready) and one is put in under the key Bevy will look the font up
  by (`atlas_key`) before the hidden text lays out; a wrong key is counted
  (`TextWarm::missed`, 0 in the tests) and Bevy makes its own.
- **When:** behind the loading screen (`MapHooks::settle`, 128 glyphs a frame, 256 before "A map's swap in the real app"), and in
  play 16 glyphs a frame, the sources asked again when the window, the fonts or the
  map's HUD change: so the command line's `--map` (no loading screen) is covered too.

Measured headless (instructions on the main thread; Bevy's text pipeline at opt-level 3,
default face): the frame that first shows the kill feed's line, the scoreboard and the
panel 39.9 M without the warm-up, 8.9 M after it; a frame warming in play 29.1 M with
32 glyphs and the atlas made on the frame, 7.5 M with 16 glyphs and the atlas from the
pool (an idle frame of that world 0.7 M). With the feed's icon glyphs declared too (a headshot
kill: the gun's glyph and the mark, at their own sizes) the same first show is 72.2 M without the
warm-up and 9.2 M after it, no atlas made; a game menu's first opening (a label in a scheme font of
its own size and a button) 23.8 M without, 3.5 M after (`a_menus_first_opening_makes_no_atlas_after_the_warm_up`). The real app's walk (each rung's worst frame
less ticks, `CHIMERA_FRAME_LOG`), taken offscreen (`--offscreen`, docs/OBSERVABILITY.md
section 2: the whole client, HUD and pipelined render thread, no display):

### The real app's rungs and swaps, offscreen (2026-11-10)

`ultimate_gun_game --offscreen 1920x1080 --map cs_source:de_dust2 --frame-log ...`,
dev build, RTX 3080, box at load 10-16 (another agent building): `+after 2 ugg_new 36336 13
cs_source:de_dust2`, then `ladder_rung 2` ... `ladder_rung 13` 3 s apart from 22 s, then `map
cs_source:cs_office`, `map hl2dm:dm_lockdown`, `map combat_arms:warehouse` 20 s apart (each
marked with an `echo` whose log time, through the frame log's `clock` line, finds its frames).
A frame in play there: main 12 M instructions less ticks (p99 24 M), 3.8 ms; render world 3.9 ms.

Each rung's second after `ladder_rung`, worst main frame (M instructions less ticks / whole),
main and render world ms, wall ms:

| Rung | Less ticks | Whole | Main ms | Render ms | Wall ms |
|---|---|---|---|---|---|
| 2 | 51.0 | 51.0 | 10.3 | 5.2 | 10.9 |
| 3 | 57.8 | 66.8 | 8.8 | 5.2 | 9.5 |
| 4 | 65.1 | 79.1 | 15.5 | 5.6 | 16.6 |
| 5 | 59.9 | 59.9 | 13.1 | 5.9 | 13.7 |
| 6 | 47.9 | 47.9 | 6.7 | 5.8 | 7.4 |
| 7 | 45.4 | 77.7 | 13.1 | 4.5 | 13.8 |
| 8 | 60.7 | 69.9 | 13.7 | 5.7 | 14.3 |
| 9 | 59.2 | 65.0 | 8.7 | 5.4 | 9.3 |
| 10 | 53.3 | 59.5 | 13.0 | 5.2 | 13.6 |
| 11 | 63.6 | 63.6 | 9.6 | 5.5 | 10.6 |
| 12 | 49.3 | 57.2 | 12.6 | 5.9 | 13.5 |
| 13 | 48.6 | 48.6 | 14.9 | 6.9 | 15.9 |

No rung frame reached 17 ms wall; the render thread never over 7 ms. The HUD texts made one
glyph atlas on a text's first frame in the rungs (`text warm: 1 glyph atlases ... at [16.875]
px`, rung 12) and five at the match's start (16.0, 21.2, 23.8, 16.1, 15.3 px: sizes no source
declares), and one in the first frame of the cs_office swap (21.5 px). What the rest of a rung's
45-65 M is wasn't profiled here (a `perf` run cut to those frames, "Nothing blocks the main
thread", would say).

A map's swap, 20 s from the `map` command (the loading screen, the load, the settling, the first
seconds of play): frames, frames over 33 ms wall, worst wall, worst main (its M instructions),
worst render world, median wall in the first 8 s; the command to the log's `map settled`:

| Swap to | Frames | Over 33 ms | Worst wall | Worst main | Worst render | Median first 8 s | Entered |
|---|---|---|---|---|---|---|---|
| cs_source:cs_office | 3058 | 9 | 74 | 73 (461 M) | 62 | 6.8 | 7.7 s (load 1.8 s, settled in 0.97 s) |
| hl2dm:dm_lockdown | 2934 | 7 | 71 | 66 (483 M) | 62 | 8.2 | 6.9 s (load 1.0 s, settled in 1.13 s) |
| combat_arms:warehouse | 3835 | 6 | 53 | 42 (201 M) | 50 | 4.0 | 2.9 s (settled in 0.22 s) |

So in the real app a swap no longer stops a frame: the worst is 53-74 ms, with the render
world's worst (50-62 ms, the new map's first frames drawn: pipelines and uploads) as long as the
main world's.

### A map's swap (`map::prepare::begin_map_change`)

A `map` command's map (and a server's change on a client or listen server) came into the
world in one frame, `spawn_map` and its commands at once: measured headless (the gun game's
simulation with the drawing's asset stores, dev build, `heavy::map_swap_frames`) that frame
was 1.0-2.9 G instructions on the main thread, 0.2-1.5 s wall at load 15-35. In it
(cs_office, M):

| What | Swap frame | Now |
|---|---|---|
| Prop shadows (`apply_shadow_detail`: `build_posed`'s silhouettes, the atlas image) | 900 | on the loading task (`shadows::ShadowsAhead`) |
| The sky's cube (`sky_image`) | 450 | on the loading task (`ImageParts::sky`) |
| What characters draw (`build_character_assets`: bodies, held models, view models) | 200-450 | steps: `BODIES_PER_STEP`, `HELD_PER_STEP`, the rest |
| Props (`spawn_prop`) | 80-260 | `PROPS_PER_STEP` a step |
| World meshes (`spawn_world_meshes`: chunks, materials, merging) | 160-230 | `WORLD_MESHES_PER_STEP` a step |
| Where decals land (`DecalSurfaces::new`) | 150 | on the loading task |
| Surfaces' colours (`SurfaceColors::new`) | 95 | on the loading task |
| The spawn's commands applied (entities, hierarchy, colliders' hooks) | 85-210 | a frame of its own |
| The first tick: a Source map's light field building its occluders on first use | 90 | on the loading task (the field used once) |
| The first tick: Avian computing every new collider's mass properties again | 55-75 | none: the hook's are kept (`spawn::counted_colliders`, below) |

Now (`console::finish_map_load`): the load's task prepares those parts too; then the map is
spawned a step at a time (`spawn::SpawnProgress`; the same phases in the same order, the
drawer's in steps of their own) until `SPAWN_BUDGET` (12 ms; 20 ms before "A map's swap in the real app") of a frame is spent, the
commands held in a queue of the spawn's own until the last step and applied together in the
next frame, so the world never holds part of a map; the simulation waits meanwhile
(`Time<Virtual>` paused: its first tick on the map is after the whole map is in, as before).
Until the game is entered a frame advances the simulation by at most one tick
(`Time<Virtual>::max_delta`, `LoadCaps`: the long frames behind the loading screen left
470-760 M of catch-up ticks owed) and the GPU is sent at most `LOAD_UPLOAD_BYTES` (16 MiB
since "A map's swap in the real app", below; 32 MiB before) a frame (Bevy's
`RenderAssetBytesPerFrame`; both given back on entering the game), new meshes at most
`upload_pace::MESH_BYTES_PER_FRAME` (6 MiB).
`CHIMERA_SPAWN_AT_ONCE=1` spawns in one frame as before (the A/B; its parts still
prepared ahead). The world is the same: `heavy::map_spawn_dump` with
`CHIMERA_SPAWN_DUMP_VIA=change` and `=stepped` gives the same entities, components and
resources for de_dust2 and de_nuke (but the stepped spawn's own resource, one entity id
earlier, and the asset slots Bevy reuses), and the prepared parts the same as computed ones
(`CHIMERA_SPAWN_DUMP_PREPARED=1`).

The map's colliders get their mass properties once: Avian's insert hook computes them as
each collider goes into the world (`ColliderMassProperties`, from the shape at its scale
then), and its `update_collider_mass_properties` computed them all again in the first tick
(the collider and its `ColliderDensity` new there, `Changed`). Avian 0.8 has no way to skip
either (`NoAutoMass`, a zero density, a static body or no body at all leave them computed;
only a `Sensor` doesn't, and that changes collision), so the spawn's last command
(`spawn::counted_colliders`) puts back both change ticks of each collider its commands
inserted and nothing changed after: that system and the size margins (the same value
again) are the only readers, and a later change (a scale differing from the hook's, a
density, a shape) marks it anew and is computed as before. Guards: `chimera_client`'s
`map_load_steps` (no map collider computed again in the ticks after a stepped spawn, each
holding what Avian computes from its shape; physics props of a convex model weigh, turn and
fall the same as with every collider marked changed after the spawn, as before).

| Map | First tick before | After |
|---|---|---|
| de_dust2 | 153-157 | 85-100 |
| cs_office | 219-236 | 163-188 |
| dm_lockdown | 223-334 | 191-268 |
| warehouse | 109-127 | 43-50 |

(Headless as below, M, three runs each at load 20-30.) Offscreen (`heavy::map_swap_render`), the settling
frames' worst main-thread frame went from 140 / 247 / 331 / 124 M to 109 / 214 / 310 / 49 M. The frame the map goes into the world
did not move (de_dust2 149-156, cs_office 250-297, dm_lockdown 155-210, warehouse 71-89):
its third in Avian's insert hook stays, as the hook can't be told to skip a collider, and
inserting the colliders earlier would let the world hold part of the map.

Measured headless (dev build, M instructions on the main thread, the map command's frames
until in the game; load 13-15):

| Map | Swap frame before | At once, parts ahead | Now: worst frame | Spawn frames | Spawn ms (now / at once) |
|---|---|---|---|---|---|
| de_dust2 | 1,834 | 864-875 | 151-217 | 5-10 | 111-315 / 104-109 |
| cs_office | 2,900 | 1,303-1,321 | 223-274 | 6-9 | 158-269 / 151-242 |
| dm_lockdown | 2,839 | 850 | 237-331 | 4 | 146 / 112 |
| warehouse | 1,037 | 711 | 126-280 | 4 | 115 / 91 |

The spawn's instructions in all are the same as at once (de_dust2 853 M over 5 frames, 864
in one); its wall time moves with the box's load more than with the steps. The worst frame
left is the frame the map goes into the world (cs_office 250-300 M), the first tick
(dm_lockdown 190-270 M since its colliders' mass isn't computed again, above), or a frame of
steps at the budget.

The render world, offscreen (`heavy::map_swap_render`, `CHIMERA_RENDER_BENCH=1`: Bevy's
renderer on the GPU without a window, a camera drawing into an image from the map's first
spawn, every schedule single-threaded so that one thread's counter takes all; dev build,
M; at once / now):

| Map | Main, worst | Extraction, worst | Render world, worst | Spawn + settle wall ms |
|---|---|---|---|---|
| de_dust2 | 883 / 163 | 32 / 10 | 629 / 262 | 871 / 896 |
| cs_office | 1,370 / 306 | 57 / 18 | 885 / 234 | 870 / 1,119 |
| dm_lockdown | 915 / 338 | 42 / 16 | 682 / 346 | 815 / 1,122 |
| warehouse | 729 / 217 | 24 / 21 | 441 / 221 | 492 / 605 |

What waits for a display: the real app's walk (`CHIMERA_FRAME_LOG`, `tools/frame_cut.py`, a
`profiling` build under `perf`) on a window, the render thread pipelined as the app runs it,
and the loading screen's frame times; time to enter each map there (the offscreen wall times
above are a dev build's, at load 15-30, and moved by 0.1-0.3 s between runs).

Guards: `apps/ultimate_gun_game/tests/it/heavy/map_swap_frames.rs` (the four maps in a row
through the shell's `map` flow; no frame from the command to the game over
`WORST_LOAD_FRAME`, 500 M; measured 150-340 M, 150-300 M with the colliders' mass kept) and `chimera_client`'s `map_load_steps` (a
map spawned in steps: no tick and no part of the map while it spawns, the same parts as a
spawn in one go, one tick's advance a frame while settling, given back in the game).

### A map's swap in the real app (2026-11-10)

What each frame over 33 ms of a swap was, and the fixes. Ultimate Gun Game offscreen at
1920x1080 (`--offscreen`, `--frame-log`), the same script as above (`+after` a `map` to
cs_office, dm_lockdown and warehouse 20 s apart, each marked by an `echo`), on the
**`profiling` build** (optimized, frame pointers: what a player runs, not the dev build of the
tables above), RTX 3080, box at load 15-35 (another agent building), sampled with `perf record -F
1999 -e cpu-clock:u --call-graph fp -k CLOCK_MONOTONIC` and cut to each frame by its
`CLOCK_MONOTONIC` span (the frame log's `begun_s`; tools/frame_cut.py's method, the swap's
window found by the `echo` line's time through the log's `clock` line). Wall times on this box
move 2-5x with its load; the instructions and each thread's CPU time are what to compare.

| Frame | Before (worst, M instructions) | What was in it | Fix |
|---|---|---|---|
| Render world while the map's steps add meshes | 150-385 M, 45-120 ms; with slab growth up to 1.5 s of CPU (kernel time in the driver) | Bevy's mesh allocator (`allocate_and_free_meshes`) packs and writes **every** mesh extracted in the frame, `Mesh::write_packed_vertex_buffer_data`: Bevy's per-frame budget (`RenderAssetBytesPerFrame`, which `LoadCaps` sets) holds back images and `RenderMesh`es but not the allocator | `client::upload_pace`: the meshes over 6 MiB a frame are held in the render world and handed to the allocator in later frames, oldest first (only while the load caps uploads or some are held); `Settling` waits for them (`uploads_settled`) |
| Render world, textures | 25-30 samples of 40 ms frames | `GpuImage::prepare_asset` (`create_texture_with_data`) at 32 MiB a frame | `LOAD_UPLOAD_BYTES` 16 MiB |
| The `map` command's frame | 300 M, 58 ms | `read_map_names`: the first `map` of a session opening Combat Arms' install to list its maps (`MapLists`) | every game's list and cycle read on a task at startup (`console::read_map_lists_ahead`) |
| The first tick on the map | 320-420 M (tick 300-350 M), 100 ms | Combat Arms' radar (`radar::install_image`) opening the whole install again (`GameInfo::mount`: every archive's directory) for the map's image, a third of it; each character's old `CaAnim` freeing the old map's rig, the last hold on an animation library (27 samples, 13 ms); the old `CaObjects` freed (old map's entities); the logic world's spawn (`LogicWorld::spawn_map`); the colliders the tick's spawns make | the radar and the other on-the-frame reads (markers, death tips, radio, the HUD's art, fonts) use the shared mount (`world::mount`, `CaData::mount`); the old rig and objects freed on a task (`map::drop_later`). Now 120-125 M of tick (186 M whole, office) |
| The frame the map goes into the world | 206-242 M, 60 ms | the spawn's commands: Avian's insert hook computing every collider's mass properties (`MassProperties::from_compound`, `ConvexPolyhedron::mass_properties`: a third), the entities, the collider tree's observer; `retire` shedding what the shadows never read (`shed_unread`: meshes, collision, texels, 3-8 ms); the props' first shadow draws; `calculate_bounds` | the shed parts and the old shadow source freed on a task. Left: 220-255 M (below) |
| Spawn steps | 120-185 M, 30-90 ms | `spawn_prop` evaluating each prop's light probe (`LightProbe::eval`, 64 props a step), a body's skinned bounds (`generate_skinned_mesh_bounds`, 2 a step), Combat Arms' sounds; `SPAWN_BUDGET` 20 ms plus the frame's own work | `SPAWN_BUDGET` 12 ms, `PROPS_PER_STEP` 16, `BODIES_PER_STEP` 1, `WORLD_MESHES_PER_STEP` 150 (fewer meshes a step too) |
| Settling (behind the loading screen) | 150-240 M, 35-50 ms, 8-12 frames a swap on dm_lockdown | the HUD's glyphs rasterized (`GLYPHS_PER_SETTLE_FRAME` 256: 10 ms), the spawn spots' flood (`CELLS_PER_LOAD_FRAME` 250), the props' shadows redrawn as they settle (`update_prop_shadows`, `silhouette`: a quarter of the settling), the text sources' strings | 128 glyphs and 160 cells a frame. Left: the prop shadows |

Before and after, three runs each (worst over the 20 s from the command, M instructions;
render CPU ms; render frames over 33 ms of CPU; seconds from the command to `map settled`):

| Swap to | Main worst before | after | Render worst before | after | Render CPU before | after | Render >33 ms before | after | Entered before | after |
|---|---|---|---|---|---|---|---|---|---|---|
| cs_office | 364-388 | 220-225 | 211-223 | 74-82 | 67-70 | 16-25 | 5-6 | 0 | 7.0-8.1 | 5.8-7.7 |
| dm_lockdown | 408-439 | 228-255 | 265-269 | 90-102 | 82-93 | 21-32 | 4-7 | 0 | 6.6-8.2 | 4.4-8.7 |
| combat_arms:warehouse | 219-224 | 108-165 | 137-187 | 81-86 | 58-134 | 29-76 | 4-6 | 0-1 | 3.6-5.3 | 3.8-6.3 |

Main-thread frames over 33 ms of CPU while the map spawns and settles: cs_office 5-7 before,
2-4 after; dm_lockdown 12-17 before, 1-8 after; warehouse 3-4 before, 0-6 after (the box's load
moves these most). The spawn takes more frames (cs_office 40 steps before, 82 after) but no
longer: its frames were already over budget.

After merging main's render pass (probe light in a table, shadows and dust placed by shaders;
two runs): main worst 219-250 M, render worst 76-93 M (render CPU 20-50 ms), the spawn and
settling with 2-5 frames over 33 ms wall a swap on the quieter run.

What is left over 33 ms, and why:

- **The frame the map goes into the world** (220-255 M, about 35-55 ms here): a third is
  Avian's insert hook computing each new collider's mass properties, which Avian 0.8 can't be
  told to skip; the rest the entities themselves and their observers. Splitting the commands
  over frames would let the world hold part of a map (systems reading the new map's resources,
  inserted last, with entities of it already there), so it stays one frame; moving the colliders'
  inserts to the following frames (the simulation still paused) would take a second queue
  through every collider spawn (`spawn_world_collision`, `spawn_entity_nodes`,
  `insert_prop_collider`).
- **The first tick on the map** (120-125 M of tick): the logic world's spawn
  (`source_logic::bridge::load`), Combat Arms' objects (`objects::load`), the colliders those
  spawns make and their mass, the first physics step.
- **Settling's prop shadows** (`update_prop_shadows`, 6 redraws a frame, each `silhouette`):
  in chimera_present's shadows, where another change was under way; left to it.
- **In play, not the swap**: a ragdoll's first frame (its bodies' convex hulls,
  `ragdoll::BodyShape::new`, at each death: 150-280 M dev build) and dm_lockdown's play frames
  over 33 ms on the loaded box (40-140 a run, CPU far under wall: the box, mostly).

A rung change, profiled the same way (each rung's second after `ladder_rung`, the profiling
build): the worst frame 25-41 M (7-15 ms wall), the gun's old entities despawned, the HUD's
text measured, and the once-a-second `chimera_perf` readout (`perf_report`'s percentiles, about
5 M) when it lands there: no frame near 33 ms, nothing changed.

The profiling build hung at startup twice in about twenty runs (after the adapter line, every
thread in a futex wait, under memory pressure with 3-10 GB swapped): not looked into.

Guards: `chimera_client`'s `upload_pace` tests (a frame writes at most the budget, the rest follow
in order, at least one a frame; a held mesh removed or extracted again is dropped; settled only
after two clear render frames, and at once without a render world) and
`map_lists_are_read_ahead_off_the_frame`.

Held or deferred meshes come in after their entities were first specialized, and Bevy's
`specialize_material_meshes` skips an entity whose mesh isn't on the GPU without retrying it: a
map swapped in drew almost none of its world (de_dust2 from the greybox, dm_lockdown after
junk_flea, bloodmoney's ground, 2026-11-10) until something else re-specialized those entities.
`upload_pace::respecialize_late_meshes` marks the entities of each mesh that came late as
needing specialization (Bevy's `DirtySpecializations`) in the frame it arrives. Guards:
`late_meshes_are_reported_once_they_arrive` and cs_source_app's `heavy::map_swap_draws` (the
binary offscreen, `map de_dust2` from the greybox: mean grey about 37 of 255, 1 without it).

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
  the world. The bots' navigation mesh (`bots::brain::ensure_nav`) was
  generated on the main thread in a 0.2-0.3 s tick when the first bot
  played (WAREHOUSE 550 areas, TERRORDOME 980); a front end now takes it a
  slice a tick ("Nothing blocks the main thread").

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
the spawn and the navigation tick too; peak 3 and 3.5 GB): room for a
busy box, a regression check rather than a target. WAREHOUSE then
JUNK_FLEA peaked at 2,267-2,716 MB over eight runs of the same code
(2026-11-03), so its budget went from 2.5 to 3 GB.

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

## A session's frame rate (2026-11-09)

Playtest report (Windows, Ultimate Gun Game): about 30 fps after ten games,
built up gradually, each game on another map; it stayed after disconnecting
and starting again in the same process. Measured on the real app (RTX 3080
with Vulkan, playtest build, 1280x720, the default 3 bots, `god`, 80 s a map
through `ugg_new <seed> 13 <map>`, the same path as a rotation; `chimera_perf_log`
and `chimera_census`, load 15-100 from other agents: instructions are what to
read, and a sampler of the main and render threads).

**What grows.** Over 15 maps:

| de_dust2 after | archetypes | tables | main w/o ticks | render instr | images / meshes | |
|---|---|---|---|---|---|---|
| fresh | 2,356 | 925 | 11.5M | 14.5M | 2,186 / 2,827 | before |
| 1 other map | 3,094 | 1,618 | 13.3M | 14.8M | 2,298 / 2,893 | |
| 5 | 4,226 | 2,686 | 15.2M | 18.6M | 2,588 / 3,049 | |
| 9 | 5,022 | 3,447 | 11.4M | 16.9M | 2,911 / 3,228 | |
| 13 | 5,554 | 3,945 | 13.5M | 22.3M | 3,099 / 3,375 | |
| fresh | 2,429 | 837 | 12.8M | 15.7M | 2,589 / 3,139 | after |
| 1 | 3,014 | 1,152 | 12.1M | 16.9M | 2,890 / 3,303 | |
| 5 | 3,793 | 1,510 | 12.9M | 22.1M | 3,089 / 3,431 | |
| 9 | 4,654 | 1,604 | 14.1M | 19.9M | 3,111 / 3,351 | |
| 13 | 5,224 | 1,731 | 14.3M | 24.5M | 2,815 / 3,127 | |

(`w/o ticks` and render move with the frame rate on a loaded box: the table
below holds one process still.)

Entities (de_dust2 4,482-4,585), cameras, observers, systems, resources: flat.
Disconnecting and starting again changes nothing: the same process keeps its
archetypes, tables and models. What grew: **tables and archetypes** (Bevy frees
none), and **an old ladder's models** (`characters::preload` and the on-demand
loaders kept every ladder's, and each map built them all).

**What costs the frame: the tables.** `chimera_census age <n>` makes a fresh
game's world as old as a long session's on the spot (one character through `n`
new archetypes, a table each), so the frame can be measured with nothing else
changed. de_dust2, one process, instructions p50 and main-thread CPU p50 (load
20-60):

| new tables | frame ms p50 | main CPU ms | tick instr | main w/o ticks | render instr |
|---|---|---|---|---|---|
| 0 | 16.7 | 8.2 | 8.3M | 10.5M | 15.0M |
| 5,000 | 19.9 | 14.4 | 16.6M | 14.6M | 15.5M |
| 10,000 | 25.5 | 20.7 | 24.2M | 17.8M | 15.3M |
| 20,000 | 33.1 (30 fps) | 26.8 | 39.8M | 22.6M | 15.1M |

That is the report's 30 fps, linearly: 1,500 instructions a table a tick, and
more ticks a frame as frames slow. No system stands out (a sampler spreads the
main thread over every system): every query matching characters steps over
each table it matched, on every run (Bevy's `fold_over_table_range`), about
600 systems a tick. A real session made about 200 tables a map in 80 s of
play (3,945 after 20 minutes), more in a full game (a rung change and a death
each visit a new combination of about 40 components that come and go): ten
games of ten minutes is in that table's range.

**The fix.** Those components (spawn protection, the gun's game's movement, HUD
and view parts, the body model and its hit boxes, `Dead`, `Ragdolled`, `God`,
`Animator`, ...; `chimera_census varies` lists them) are sparse sets now: a
combination makes an archetype, not a table.

| | before | after |
|---|---|---|
| Fast test (`session`, 8 matches): tables | 1,112 -> 2,272 | 196 -> 237 (flat from match 5) |
| The same, instructions a tick | 7.5M -> 9.1M | 6.7M -> 7.3M |
| Real app, 20,000 new archetypes: tick | 8.3M -> 39.8M (4.8x) | 10.3M -> 18.2M (1.8x) |
| The same, main w/o ticks | 10.5M -> 22.6M (2.2x) | 11.9M -> 19.9M (1.7x) |
| Real app, tables holding characters after 15 maps | not counted | 290 |

"After" is on newer main (the character preloading came in between); its tick
starts higher for that. The tables the real app still makes after the fix
(1,731 after 15 maps, 290 of them characters') are the maps' own kinds of
entities and stop growing when the maps repeat (the lap test below). Three laps of real
maps, headless (`heavy::session_maps`, 6 bots, the server's map change, each map
the same ladder every lap; a tick is the median of 300, 2026-11-09):

| lap | de_dust2 | cs_office | dm_lockdown | warehouse | de_inferno |
|---|---|---|---|---|---|
| entities, first | 1,111 | 1,223 | 1,157 | 701 | 1,528 |
| entities, third | 1,117 | 1,223 | 1,155 | 704 | 1,528 |
| tick instr, first | 10.7M | 15.9M | 14.4M | 10.3M | 20.3M |
| tick instr, second | 12.6M | 19.1M | 17.6M | 11.4M | 20.7M |
| tick instr, third | 13.6M | 18.1M | 19.4M | 10.9M | 21.3M |
| tables, first | 294 | 390 | 515 | 527 | 589 |
| tables, third | 601 | 605 | 605 | 605 | 605 |

(Archetypes 1,456 after the first map, 4,068 at the second lap's start, 4,892
at the end: the first lap makes nearly all of them, and the test compares a tick
with the lap before from the second lap on.) Each archetype costs every tick
whether anything is in it: with the bots kicked (no character on the map) a
de_dust2 tick was 5.5M instructions at 1,474 archetypes and 7.4M at 3,839, about
800 instructions an archetype on every map (on de_inferno 440). So de_dust2, the
first map, pays all of the first lap's on its second visit (up to ~1.35x with
nothing else on the map). A tick's mean is no measure of that: in one 200-tick
window on de_dust2 single ticks ranged from 6.7M to 24M (deaths, grenades,
ragdolls), so a map's mean moved by a fifth from run to run (warehouse 9.3M to
13.6M on the first lap) and with the ladder; the test's first version
(the mean, a new ladder each lap, the first lap against the second) failed by
turns on de_dust2 and warehouse at each of the merges after it. Medians of the
same ladder stay within 3% between runs.

**The ladders' models.** A map's unload (`characters::preload::forget_old`) now
forgets what older ladders brought but the ladder just played and what's worn
or held; it loads again if wanted (`Combat Arms: forgot 10 bodies and 21 held
models` in the log; the table above's stores; each ladder kept was the character
change's 1.6 GB).

**Left** (backlog): the archetypes' own cost (table above, and "A session's
archetypes" below for what was cut; Bevy's per-frame
queries that walk them all, bevy_audio's cleanup a tenth of the main thread at
20,000); the render thread's slow rise and process memory (5.9 -> 14.5 GB over
eight maps with the stores flat): the next section. On Windows a player can
send a bugreport's `report.txt` or `chimera_perf_log 1` (archetypes are in it),
and `chimera_census` at the start and at 30 fps; `chimera_census age 20000` in a
fresh game shows what a long session cost before this fix.

## A session's archetypes (2026-11-09)

What the section above left: archetypes, which Bevy never frees and each
costs a tick about 800 instructions (queries walk every matching one,
empty or not): 4,892 after three laps of the lap test, nearly all of them
characters'.

**What made them.** `chimera_census made [<component>]` (new) puts each
archetype holding `Health` down to the insert or removal that first reached
it (Bevy's cached archetype moves); counted per settled state (where a
character is at a tick's end) against the transient ones. In the fast
`session` test only 236 of 1,637 characters' archetypes were ever occupied
at a tick's end: the rest were steps of a rung change or a respawn (about
twelve single inserts and removals: the rung's look, `MaxSpeed`, the
movement marker, `HeldMovePenalty`, hit boxes, `RecomputeMassProperties`,
...), each made again for every combination of the parts that didn't take
part. What multiplied the combinations: parts a character gains once and
keeps from then on (`ViewPunch` at the first shot, `Hl2Punch`, `NextPush`,
`CaLastPrimary`, `OthersSteps`, `CaKillMe`, `WantHeld`, `Rebased`,
`Drowning`), sleeping (`Sleeping` and `SolverBodyIndex` came and went on
standing characters), and rung parts taken off between rungs (`KitMelee`,
`CaRungParts`). Each doubled what was visited before it, and new bots on
every map visit it all again.

**The cut.** Those parts are on every character from the spawn
(`CharacterPartsExt::on_every_character`: required by `Intent`, so they come
in the spawn's move), `KitMelee` holds an `Option` and `CaRungParts` an
empty value in place of being removed, characters never sleep
(`SleepingDisabled` in `character_bundle`), and spawn protection with `God`
and HL2's suit parts go on in one move (`insert_if_new` keeps a client's).
DESIGN.md "ECS style" has the rule; the lap test fails when a lap after the
first adds 150 characters' archetypes or more, and its message lists the
moves (`census::made_by`).

Three laps of real maps (`heavy::session_maps`, as in the table above;
before is that table's run, after is this branch, archetypes at each map's
end, instructions the median tick):

| | before | after |
|---|---|---|
| archetypes after the first map | 1,456 | 988 |
| archetypes after lap 1 / 2 / 3 | 4,068 / not counted / 4,892 | 1,584 / 1,612 / 1,664 |
| characters' after lap 1 / 2 / 3 | | 591 / 617 / 667 |
| de_dust2 tick, lap 1 / 2 / 3 | 10.7M / 12.6M / 13.6M | 10.0M / 13.6M / 10.7M |
| cs_office | 15.9M / 19.1M / 18.1M | 16.3M / 16.2M / 15.5M |
| dm_lockdown | 14.4M / 17.6M / 19.4M | 15.0M / 14.0M / 16.0M |
| warehouse | 10.3M / 11.4M / 10.9M | 10.2M / 8.7M / 7.9M |
| de_inferno | 20.3M / 20.7M / 21.3M | 19.3M / 18.4M / 16.9M |

The fast `session` test (8 matches, greybox): 2,316 archetypes after four
matches and rising before, 1,180 after eight now and none new from the
sixth. At the measured 800 instructions an archetype, the 3,200 fewer
after three laps are about 2.5M instructions a tick with nobody on the map.

**Who pays per archetype.** `perf` (instructions, user space) over 3,000
ticks of the fast test's world fresh (949 archetypes, 4.8M a tick) and with
20,000 more on one character (`census::age`, sparse: 20.8M a tick, 800 an
archetype): the cost is spread over every system whose query matches
characters, each about the same; the five that gained most were Source
movement's main query, `ladder::write_status`, the minigame's roster query
(`Team`, `JoinOrder`, `Name`, `MatchesWon`), `perks::track_last_primary`
(whose `collect` walks the archetypes twice, the second time for
`size_hint`) and `moments::update`'s rung query; then Bevy's observer
lookup per archetype.

## A session's archetypes: the game switch (2026-11-09)

What the section above left: a switch between games still put its parts on
one at a time. Counted in the fast `session` test (8 matches, greybox) by
which characters' archetypes were ever occupied at a tick's end: 73 of 530,
the other 457 in-between steps of a switch, a rung swap or a respawn,
`chimera_census made` naming their moves (`-SourceMovement` 43,
`+RecomputeMassProperties` 35, `+CaMovement` 32, `+HeldMovePenalty` 21,
`+Hitboxes` 21, `-Hl2Model` and `-RungModel` 18 each, `-MaxSpeed` 14,
`±PhysicsTouch+Shadowed` 24, `-RungClass` and `-PlayerClass` 12 each, ...).

**The cut.** Parts whose absence meant "none" are on every character from
the spawn with a value meaning none: `MaxSpeed(Option)`, Combat Arms'
`HeldMovePenalty` (no penalty) and `HandlingOf`, the Source movement's
`PhysicsTouch` and `Shadowed(Option<Entity>)` (the shadow body comes and
goes, the part stays). Parts that go together go in one move: a movement's
marker with its `MovementSlot` (`slots::set_movement`), and CS:S's and
HL2:DM's movements, both `SourceMovement`, switch by a new value in place
(`MovementImpl::marker`); a body's `BodyModel`, `Hitboxes` and `Animator`
(`map::bodies::attach_hitboxes`); Combat Arms' `Hitboxes` with
`CaHitboxes`; a rung look's removal (`RungModel` with `Hl2Model`, `RungClass`
with `PlayerClass`).

| | before | after |
|---|---|---|
| fast test: archetypes after 8 matches / characters' | 1,138 / 530 | 959 / 351 |
| fast test: of the characters', occupied at a tick's end | 73 | 64 |
| real maps: archetypes after lap 1 / 2 / 3 | 1,584 / 1,612 / 1,664 | 1,401 / 1,428 / 1,471 |
| real maps: characters' after lap 1 / 2 / 3 | 591 / 617 / 667 | 410 / 429 / 472 |
| de_dust2 tick, lap 1 / 2 / 3 | 10.0M / 13.6M / 10.7M | 10.1M / 10.3M / 10.5M |
| cs_office | 16.3M / 16.2M / 15.5M | 15.6M / 15.1M / 15.5M |
| dm_lockdown | 15.0M / 14.0M / 16.0M | 16.6M / 15.1M / 14.9M |
| warehouse | 10.2M / 8.7M / 7.9M | 7.7M / 7.7M / 8.4M |
| de_inferno | 19.3M / 18.4M / 16.9M | 18.0M / 17.8M / 17.5M |

(Real maps: `heavy::session_maps`, before is the section above's run;
ticks are medians and move by a few percent between runs.) The guards are
tightened to match: the lap test fails at 100 characters' archetypes a lap
after the first (was 150), the fast test at 1,150 archetypes after eight
matches and 100 after the fourth (was 1,600 and 200).

**Per archetype.** Of the five queries that paid most per archetype, three
walked them twice: a query's `collect` asks the iterator's `size_hint`,
which sums every matched archetype's length before the walk
(`perks::track_last_primary`, `runtime::players`, `wins::publish`), and
`ladder::write_status` iterated its climbers twice. Each now walks once.

**Left** (backlog): what still makes most of the in-between archetypes is
the switch's own steps, each a different game's system: avian's
`RecomputeMassProperties` after Combat Arms' collider changes (34; it
comes and goes on every crouch too), the movement swap's removal and
insert (`-SourceMovement` 30, `+CaMovement` 31: Bevy moves an entity once
per insert or removal), Combat Arms' rung look and view model
(`+CaRungLook`, `±OwnViewAnim`), CS:S's rung class, HL2:DM's suit and
`ArmorPieces` (whose absence means "the armour's own kit", so it can't
simply stay). Folding those into one move would need a Chimera command that
gathers a switch's inserts and removals per character and applies them
together (`insert_by_ids` and `remove_by_ids`).

## A session's memory (2026-11-09)

What the section above left: Ultimate Gun Game's process memory rising over a
session, and the render thread's cost. The real app, playtest build, 1280x720,
the default bots, `god`, 90 s a map through `ugg_new <seed> 13 <map>` (the
`after` console command times each line; `+wait` frames held up the map
change), de_dust2 then cs_office, dm_lockdown, warehouse, de_inferno, de_nuke,
cs_militia, de_aztec, de_dust2. The box at load 30-80 from other agents, so
instructions and bytes, not times.

**Tools.** `chimera_mem [<n>]` (`client::memory`) splits the process by kind
of mapping (`/proc/self/smaps`: heap, anonymous, device, mapped files) and adds
glibc's counts (`mallinfo2`), wgpu's allocator report (blocks, allocations by
label), the image and mesh stores' bytes, the render world's entities,
textures, mesh slabs and pipelines, and how many `PropMaterial`s the store
holds against those an entity draws. A sampling heap profiler preloaded into
the game (not in the repository: a few lines of C over glibc's `malloc`,
`backtrace` every 256 KB, dumps of what's live by stack) said what holds the
heap.

**What held it.** Mapped files (100-300 MB) and the driver's device mappings
(256-290 MB) stay flat; the rise was all heap. Live heap by stack between the
fifth and the eighth map: 1.31 GB more under `AnimList::build` in
`characters::models::load_body`, 0.4 GB of everything else. Each Combat Arms
body loaded on demand parsed its skeleton's animation files (child models)
again, about 80 MB of animation keys a body, and its rig kept them as long as
it lived: a ladder brings ten bodies, and the old ladders' rigs stayed in the
late model index (`CaModelIndex::rigs`; forgetting a ladder drops its groups,
not its rigs). Second, glibc kept what a map change freed: "free in arenas"
reached 5.9 GB, counted resident or swapped.

**The fixes.** Child models load once per install data
(`model::ChildModels`, `load_sharing`) and an animation is an `Arc` shared by
its model and every `AnimList` built on it, so a body costs its own file
only. 20 s after each map spawns, the client gives glibc's free pages back to
the system (`memory::release_free`, `malloc_trim` on a thread; Linux with glibc
only: Windows' heap returns large blocks itself).

| map (process: resident + swapped, GB) | before | after | malloc in use, before | after |
|---|---|---|---|---|
| de_dust2 (start) | 4.26 | 2.80 | 2.12 | 2.19 |
| cs_office | 7.16 | 4.06 | 4.18 | 3.41 |
| dm_lockdown | 9.44 | 4.75 | 5.68 | 4.21 |
| warehouse | 9.96 | 5.07 | 6.46 | 4.49 |
| de_inferno | 12.34 | 5.85 | 7.90 | 5.32 |
| de_nuke | 13.21 | 5.91 | 8.24 | 5.28 |
| cs_militia | 15.16 | 5.66 | 8.68 | 5.02 |
| de_aztec | 15.03 | 5.33 | 8.72 | 4.71 |
| de_dust2 | 15.36 | 5.52 | 9.27 | 4.85 |

What the heap still gains after the first maps (1.36 GB between the second and
the last map, flat from the fifth): the current ladder's Combat Arms models'
textures kept on the CPU (decoded RGBA 0.6 GB, their mips 0.15 GB, view
models' 0.15 GB), which a later map's spawn builds its images from again.

**The render thread**: the next section.

## A session's render thread (2026-11-09)

What the section above left: de_dust2's render world at 13.5-14.4M
instructions a frame at the start, 18.4-19.3M after eight other maps, 24-25M
on cs_militia and de_aztec. Sampled (instructions, user space) on de_dust2 at
the end: wgpu's trackers 24% (dropping and clearing each pass's
`ResourceMetadata<Buffer>` and `<Texture>`, `BufferUsageScope::clear`, the
vectors' resize), Bevy's mesh allocator and render-asset hash lookups 14%
(`MeshAllocationKey` tables, `RenderMesh` and `PreparedMaterial` maps). wgpu
sizes its trackers by the most buffers and textures ever alive at once
(`TrackerIndexAllocator::size`, which never falls), and Bevy's hash tables
keep their peak capacity. The peak: about 11,000 GPU buffers after the second
map (2,530 at the start), nearly all `PropMaterial`s' uniforms (10,027 in the
store on dm_lockdown, 306 drawn): every material of every body, skin family,
held, loose, gib and view model the map's kit and the ladder's late models
carry was prepared (a uniform buffer and a bind group each), drawn or not.

**One buffer's price.** `cargo run -p chimera_present --features dev --example
material_price -- [--views <k>] [<n>...]` renders offscreen (a GPU, no window
or display: it ran on the dev box with the desktop session down), `k` cameras
drawing 300 cubes of their own `PropMaterial`s into images, while the store
holds `n` more that nothing draws, phase by phase; instructions a frame on the
main thread (the render world runs there, unpipelined), 200-300 frames each
(RTX 3080, Vulkan, dev profile):

| undrawn materials held | 1 view | 4 views | 8 views |
|---|---|---|---|
| 0 (start) | 10.61M | 37.43M | 80.35M |
| 12,000 | 12.50M | 39.09M | 81.90M |
| 0 (dropped) | 12.49M | 39.08M | 79.75M |
| 12,000 again | 12.69M | | 80.05M |
| 24,000 | 14.88M | | 85.16M |

About 150-200 instructions a frame for each buffer of the peak, and it stays
after the buffers are freed: only a new peak costs more. So what matters is
never making the peak: 11,000 undrawn materials are about 2M a frame of the
render thread's rise, for the rest of the process. The rest of the rise (the
hash tables, the session's other assets) is not priced yet.

**The fix.** The kit's models' materials wait outside the store
(`prop_material::DormantProps`: a handle reserved in the store, the material
beside it) until an entity draws one (`wake_drawn`, an observer on
`MeshMaterial3d<PropMaterial>`); bodies, held, loose and gib models draw
copies (`probe_lit::instance`), which go into the store as before, and a view
model draws its kit's material when shown. What changes every material (the
fog setting, underwater fog, animated textures) changes the waiting ones too;
one goes with its last handle (`DormantProps::prune`). A late model's
materials no longer reach the render thread when it comes in
(`late_build`). `chimera_mem` prints the store, the drawn and the dormant.

Headless, with the drawing's assets (`heavy::session_materials` in
ultimate_gun_game: a ladder on each map, the server's map change, 4 bots; no
render world, so materials in the store, not GPU buffers):

| map | store before | after | drawn | dormant after |
|---|---|---|---|---|
| de_dust2 | 2,114 | 153 | 120 | 1,961 |
| warehouse | 1,631 | 30 | 30 | 1,601 |
| de_dust2 again | 2,123 | 163 | 130 | 1,961 |

The test takes about 13 minutes by design (four maps, a full ladder each), so nextest gives it, and `session_maps`, 60 minutes (`.config/nextest.toml`). (dm_lockdown, in the test's list, was not installed on the box.) The test
fails if the store holds 600 more than is drawn, or grows on the same map
after a lap.

**Waits for a display:** the real app's render thread before and after over a
session of maps (de_dust2 fresh and after eight other maps, as above), and frozen-scene screenshots before
and after on a Combat Arms and a CS:S map (no visual change is expected: the
same materials draw, made at the same moment).

## Performance pass (2026-11-10)

A pass over the server tick for what scales with a map's entities and with
the games composed, each change measured before and after. The box ran at
load 15-40 the whole time (other agents building), so the numbers are
user-space instructions, which move under 1% between runs here:
`perfgate` (the CS:S scenarios, 2,000 ticks each, every thread; a
`profiling` build, which is the playtest build with line tables, here
with frame pointers too) and `perf record -e instructions:u` on it and on
`ultimate_gun_game_server` (6 bots on de_dust2), folded by the innermost
system on the stack.

**Where a tick went** (before; share of the tick's instructions):

| | de_dust2, CS:S, 10 bots (5.9M a tick) | mg_lego_multigames_v2, nobody on it (8.4M) |
|---|---|---|
| the logic bridge (`source_logic::logic::bridge::pre` and `post`) | 21% | 63% |
| Source movement (`source_movement::step`) | 20% | 2% |
| of that, the push-back from props (`pushaway::push_back`) | 4% | |
| `pushaway::push_props` | 3% | |
| avian's narrow phase and contact solver | 14% | 1% |
| bots (`bot::think`) | 6% | |
| ragdoll joints | 6% | |
| buoyancy (`map::buoyancy::float_bodies`) | 0% (no water) | 12% |
| Bevy's transform propagation (per frame, not per tick) | 4% | 9% |

### The logic bridge

Map logic (`source_logic`) is a world of its own that the bridge runs
twice a tick (before and after movement) and then brings the ECS in line
with. Both halves read everything every time:

- **Scans.** Some forty passes a phase each asked for every entity
  (`LogicWorld::ids`: a list made by walking every slot) and looked at each
  one's class: thinks, movers, cameras, triggers, breakables, props, fires,
  joints, areaportals, occluders, the monitor's camera. Every slot now has
  its class number beside it and every class a list of its slots
  (`LogicWorld::ids_of` with `classes::kinds`' masks, kept by `sync_kind`
  wherever a slot is filled, emptied or its class set; debug builds check
  them at every frame's end), so a pass visits only its classes'
  entities, in the same order. The passes still filter as before: the
  mask only has to hold their classes.
- **A hash map per phase.** `sync_movers` made a `HashMap<EntId, Entity>` of
  every mover's node each phase (SipHash: a tenth of mg_lego's tick); the
  nodes are found by slot now. `rides_body` searched the list of parent
  links for every mover (`body_riders`: by slot).
- **A search per prop.** `sync_props` searched the props' states for each
  prop's (`states.iter().find`), quadratic in a map's props: a binary
  search (the states are in entity order).
- **Writes' tests through `EntityWorldMut`.** Every mover's and prop's node
  got some ten component lookups and comparisons each phase to find, almost
  always, nothing to write. The bridge now tells the nodes with something
  to write first: the movers' in one walk over the nodes through a kept
  query (`MoverWants::differ`), a prop's with one kept-query read
  (`prop_differs`), each the writes' own tests in their order; only those
  nodes run the old code. `SyncEveryNode` (a resource) runs the old code
  for every node.

| `perfgate`, instructions for 2,000 ticks | de_dust2 | mg_lego_multigames_v2 | greybox |
|---|---|---|---|
| before | 13.40G | 17.72G | 1.29G |
| scans by class (a byte per slot), nodes by slot, the prop search | 12.32G | 13.20G | 1.30G |
| class lists, nodes with nothing to write passed over, riders by slot | 11.56G (-13.7%) | 10.93G (-38.3%) | 1.29G |

(Two runs each; they differ by under 0.5%. The median tick: de_dust2 5.9M
to 4.9M, mg_lego 8.4M to 5.4M.) What is left of the bridge on mg_lego (about
40% of its tick) is `sync_movers` working out what each of its thousands of
movers wants, twice a tick (its pose's rotation, its colour, its panes), and
the logic's own mover step; passing those over needs the logic to say which
entities changed since the last phase (a stamp set by `get_mut`): done
since ("A change stamp for the movers" below).

Guards: `source_logic`'s `scans_by_class_match_full_scans` (every class's
scan against a filtered full scan, through kills, a flame made at run time
and a round restart; `kinds::PUSHERS` against `movers::pusher`) and
`body_riders_answer_as_the_search`; `cs_source`'s
`map_logic::skipping_unchanged_nodes_changes_nothing` (a door, a brush a
timer switches and colours, a vent shot to pieces, a prop switched, skinned
and killed, a round restart: every mover's and prop's node the same after
each of 260 ticks with and without the skip; it fails with a test of
`differ` or `prop_differs` taken out) and `map_logic::idle_entities_cost_a_tick_little`
(1,200 idle brushes and 2,400 points: the tick with the skip must cost less
than with every node written through; it prints both, 9.0M against 11.9M
instructions in a test build).

### Players and props (`source_movement::pushaway`)

Every Source player asks about every multiplayer physics prop each tick
(the push-back that goes into its move input, `push_back`) and every 0.1 s
(`push_props`), and each answer started with the prop's world box: eight
corners through its transform. A test that can only say "surely apart"
comes first now (`surely_apart`: the cube that holds the prop's box at any
turn, with slack for rounding, against the player's box); a prop it doesn't rule out
gets the exact box as before, so the results are the same
(`the_far_test_never_drops_an_overlap`: 40,000 places, turns and scales,
none of the overlapping ones ruled out; the test caught the first try, a
sphere, which is smaller than the box a turned prop's corners span).

| `perfgate`, instructions for 2,000 ticks | de_dust2 (10 bots) |
|---|---|
| before (with the bridge's change) | 11.56G |
| after | 10.98G (-5.0%) |

### Buoyancy (`map::buoyancy::float_bodies`)

On a map with water, each physics step gathered every collider of every
body (its box, its volume, its hulls made and kept on first sight) and only
then dropped the bodies that aren't dynamic: nearly all of a map's
colliders. It asks for the body's kind first now; what floats, and how, is
unchanged (the same colliders in the same order for each dynamic body;
`logic_physics`' and HL2:DM's floating tests). The hull cache holds
dynamic bodies' colliders only.

| `perfgate`, instructions for 2,000 ticks | mg_lego_multigames_v2 |
|---|---|
| before (with the bridge's change) | 10.93G |
| after | 8.94G (-18.2%) |

Together with the two above: de_dust2 13.40G to 10.98G (-18%), mg_lego
17.72G to 8.94G (-50%); the median tick 5.9M to 4.5M and 8.4M to 4.3M.

### Combat Arms' characters on the server

`ultimate_gun_game_server`, 6 bots on de_dust2, sampled (the bots'
navigation flood left out): Combat Arms' `characters::drive` 18% of the
tick (every character's skeleton posed every tick: four trackers' keys
blended per node, the nodes' matrices multiplied down the tree, the hit
spheres read off it, twice on a server), its grenades' server frames 8%,
`perks::perk_tick` 5%, Source movement 7%, the ragdolls' pose record
6%, the bots 5%, the logic bridge 9%.

- **The matrix product** (`lithtech_formats::anim::mul`, one per node per
  pose): a sum over an iterator per element, not inlined; written out now,
  each element's products added in the same order, so the bits are the
  same (`mul_gives_the_sum_s_bits`, 20,000 matrices against the sum).
- **`CaData::install_weapon`** searched the install's 2,400 weapons making
  each one's slug (a new string) on every call, and a zoomed sniper's perk
  tick asks every tick (3% of the sampled tick): an index by id, made on
  first use (`the_install_weapon_index_finds_what_the_search_finds`).
- **The grenades' server frames** (20 a second) posed every character's hit
  spheres (`GrenadeWorld::bodies`) for the lasting explosions and
  over-time entries, with none of either there: only when one is. For
  another game's body each hitbox's zone factor read the male table's
  regions from their node names again (`HitSphere::region`: a lower-cased
  copy and several searches per sphere): the character data keeps them
  (`CharacterData::male_zones`).

`party_kit`'s `heavy::bot_cost` (the gun game on de_dust2 with Combat Arms
bots, a test build, the schedules' thread's instructions a tick over 600
ticks; two runs each, within 2%):

| bots | before | the matrix product | and the index, the grenades' bodies, the zones |
|---|---|---|---|
| 2 | 5.65M | 5.49M | 5.30M (-6%) |
| 4 | 9.68M | 9.31M | 8.62M (-11%) |
| 8 | 15.54M | 15.12M | 14.01M (-10%) |
| 16 | 25.21M | 24.18M | 22.17M (-12%) |

Left there: the pose itself (`Rig::layered`'s four trackers a node, the
matrices turned back into rotations and translations, the hit spheres found
by name through a lower-cased copy: `pose_of` is a sixth of `drive`), done
for every character every tick whether anything asks for its spheres (made
on demand since: "Poses on demand" below); the
ragdolls' pose record samples every Source body's animation each tick and
a server poses its hitboxes from a second sample (`pose_sim_hitboxes`).

### Poses on demand (Combat Arms' characters)

What read a Combat Arms character's pose, and when:

- **`characters::hitboxes`**, every tick, every character: Chimera's
  `core::Hitboxes` from the spheres, for other games' guns. It also posed
  the spec's skeleton (`skeleton::fallback`, a name index made per call)
  under a modelled body only to throw it away, and read each sphere's
  region through a lower-cased copy of its name.
- **Shots.** Chimera's hitscan (`weapon::deliver`) and flyers test every
  boxed character's hitboxes on each ray; Combat Arms' own rounds
  (`weapons::Delivery::bodies`) made every living character's spheres
  per round, though every test of them comes after a test of that
  character's box. A bot's round reads the server's pose
  (`server_spheres`), a player's the client's.
- **Lag compensation** (`weapon::lagcomp::record`, a network server)
  kept every living character's hitboxes every tick, read only by a
  remote player's shot.
- **The drawing**: `Animator::fixed`, every tick for every body with a
  model; `render::bodies::pose_bodies` uses it only for bodies some
  camera drew (others every few frames).
- **Sockets**: the HUD's name tags (every other character every frame,
  though most tags don't show), attached effect groups (characters
  wearing one), third-person flashes (per shot), drops at the left foot,
  a death's body yaw.

Ragdolls and grenades don't read it (the grenades' bodies are the spec's
skeleton; a corpse is posed by its own death animation).

So the pose is made when asked (`characters::CaPose`): each tick `drive`
keeps what it is made from (the four trackers, aim, crouch, legs' yaw,
the worn face's and held weapon's sockets: a few hundred bytes) and the
first reader poses it, the client's view and the server's from one
unaimed pose as before, the same bits whenever it is made. The hitboxes
are `core::LazyHitboxes` (`later::Later`, Chimera's made-on-first-use
cell; shots read them and plain `Hitboxes` through `core::HitShapes`),
lag compensation keeps them unmade until a rewind, the drawing's pose is
a `Later` made when `pose_bodies` draws the body, Combat Arms' rounds make
a body's spheres when the ray reaches its box, and a name tag reads the
pose (and casts its sight ray) only when it shows. The regions are read
without a copy. `CaPoseEveryTick` (a resource) makes everything in the
tick, as before.

`party_kit`'s `heavy::bot_cost`, 16 Combat Arms bots, the instructions a
tick in posing (`drive`, `hitboxes`, and the poses made where asked),
counted around them in a test build (two runs each):

| | `drive` | `hitboxes` | posing where asked | together | poses made a tick |
|---|---|---|---|---|---|
| before (main at 259bf019) | 3.65-4.46M | 0.76-0.79M | | 4.4-5.2M | 16 |
| `CaPoseEveryTick` | 3.47M | 0.19M | | 3.67M | 16 |
| on demand | 0.46-0.48M | 0.05M | 0.68-0.74M | 1.18-1.27M | 3.7 |

2, 4 and 8 bots: 0.14M, 0.19-0.20M and 0.50-0.69M on demand against
0.42M, 0.77-1.01M and 1.62-1.95M every tick. About 4M of a 20-25M tick
with 16 bots (16-18%), three quarters of it the pose; the whole tick's
mean and median move by about as much from one run to the next (the
fights differ), so the table counts the posing itself. What still poses
in this match: Chimera's hitscan tests every boxed character's hitboxes
on each ray (all sixteen on a tick with another game's shot: 1 in 18
here; since then only those a ray comes near: "A broad phase for the
hitscan"), each shot's third-person flash poses its shooter, and the poses
Combat Arms' rounds reach.

Guards (`combat_arms`' `characters` tests):
`poses_on_demand_match_poses_every_tick` (two players with a model, 300
ticks of walking, crouching, looking and rifle fire at each other, one a
bot: the hit spheres, the server's, the sockets, the hitboxes, the
drawn pose and every tick's health bit for bit against
`CaPoseEveryTick`, read every tick and every seventh) and
`a_pose_nobody_reads_is_not_made`; `chimera_core`'s `later` tests (made
once, from any thread).

### Searches of the map's entities every frame (HL2:DM's set pieces)

`hl2dm::game::set_piece_fx`'s `flares` looked through every map entity for
`env_flare`s each frame, and `canisters` for the sky camera (a class-name
search of some thousand entities, each a search of its keys), on every map
of every game and on the dedicated servers: 6% of the sampled Ultimate Gun
Game server's tick on de_dust2, which has neither. The flares' entities
are found when the map's entities change (`MapEntities`' change tick), the
sky camera only when a canister in the sky is there to draw.

| `heavy::bot_cost`, instructions a tick | 2 bots | 4 | 8 | 16 |
|---|---|---|---|---|
| before (with the changes above) | 5.30M | 8.62M | 14.01M | 22.17M |
| after | 5.11M | 8.40M | 13.78M | 21.87M |

### A frame offscreen (`frame_bench`), and the glows' rays

No window can open on the dev box, so a frame in play had no profile
there. `cargo build --profile profiling -p ultimate_gun_game --example
frame_bench` is one: the gun game's simulation with Bevy's whole renderer,
six bots on a seeded ladder, a camera at a bot's eyes drawing into an
image, frames of 1/60 s, each frame's main schedule, extraction and
render world counted apart on one thread (as `heavy::map_swap_render`;
no HUD, no local player, no view model, one camera). Its fights differ
from run to run, so its medians move by about a tenth: it is for `perf`,
not for comparing builds by its printout. de_dust2, 1280x720, RTX 3080:
main schedule 11-13M instructions a frame at the median (with its 1.1
ticks), extraction 0.8M, render world 8.4-9.5M; 4.6-5.4 ms of wall time a
frame with everything on one thread at load 20.

What the main schedule's instructions went to (3,000 frames sampled): the
ticks' systems as above (Combat Arms' `drive` 7.6%, its bots 4.9%, Source
movement 4.9%, the pose record 3.5%), and per frame `glow::glow_visibility`
5.4%, `shadows::update_prop_shadows` 4.2% (its silhouettes),
`update_character_blobs` 3.2%, Bevy's transform propagation 6%, its
visibility 5%, `dust::update_dust` 1.8%, the two searches above 2.0%. The
render world: submitting the frame 15%, the mesh allocator 11% (the blob
shadows, prop shadows, dust and particles are new mesh data every frame),
the opaque pass 10%, the prepass 9%, preparing `PropMaterial`s and their
bind groups 14% (each body's probe light is in its materials, so a moving
body's eight are prepared again every frame), light clustering 8% (with
no light in view).

**The glows.** Every glow sprite in the potentially visible set got five
rays a frame to its occlusion proxy, then the result was scaled by how
much of the proxy is on the screen: nothing, for the glows behind or
beside the eye. The screen test comes first now and the rays are cast only
when it isn't 0 (`seen_fraction`; the product is the same). In the
sampled frames `glow_visibility` went from 5.4% of the main schedule's
instructions to 1.7%, the two searches from 2.0% to 0.05%, the whole from
57.2k samples to 53.3k (-7%).

### A flood every tick (found after merging main at e863d9c6)

`bots::brain::nav_whole` (a dedicated server's and the tests' whole
navigation flood in one tick) took one step of a new `NavGenJob` each
tick; since the flood went onto a task (8a162689) a bounded flood is two
steps (the walk that finds the bound's clusters, then the mesh), so on a
map with visibility data the first step never ended in a mesh and every
tick flooded the map again: 2.15G instructions a tick on de_dust2
(`heavy::rung_frames`' worst frames, limit 45M), `heavy::bot_cost` and
`bots_on_rungs` past their 12 minutes, `nav_bounds` with no mesh. Both
steps run on one job now; `bot_cost` is back at 38 s (16 bots 21.0M a
tick).

### The session's maps after the pass

`heavy::session_maps` (three laps of real maps, six bots on a ladder, a
test build, the median tick's instructions) after the pass and main's
merge, against the table of "A session's archetypes: the game switch" the
day before:

| | before, lap 1 / 2 / 3 | after |
|---|---|---|
| de_dust2 | 10.1M / 10.3M / 10.5M | 7.5M / 7.3M / 7.9M |
| cs_office | 15.6M / 15.1M / 15.5M | 11.1M / 12.4M / 12.4M |
| dm_lockdown | 16.6M / 15.1M / 14.9M | 10.3M / 11.1M / 11.4M |
| warehouse | 7.7M / 7.7M / 8.4M | 5.8M / 6.3M / 6.5M |
| de_inferno | 18.0M / 17.8M / 17.5M | 13.8M / 12.8M / 13.9M |

A quarter off every map. `heavy::rung_frames`' worst frames (a bot walking
the ladder on de_dust2, ticks included): 8.1-17.7M, were 12.5-27.7M.

## The tick's two largest leftovers (2026-11-10)

The two largest items of the backlog's "The tick and the frame, what's
left": Chimera's hitscan posing every Combat Arms body on each ray, and
`sync_movers` working out every mover's wants twice a tick. Measured on
this branch before and after, after main's new build settings (dev LTO
off, split debug info: the test build's counts are higher than the
older tables above, so compare within this section only).

### A broad phase for the hitscan

Chimera's hitscan (`weapon::deliver`'s `trace`) tested every boxed
character's hitboxes on each ray, so every Combat Arms body's
`core::LazyHitboxes` were made (its pose with them) on every tick with
another game's shot. A game now declares, beside the lazy hitboxes, a box
that holds them whatever the pose turns out to be (`core::HitBound`,
itself made on first use: `LazyHitboxes`' second field); the trace tests
the ray against it first, with the same limit as the shapes (what it hit
so far) and the same growth for a box trace, and makes the shapes only
when the ray can enter it (`HitShapes::may_meet`). A shape inside the box
can't be entered by a ray that misses the box, so the hits, their order
and the damage are the same; the box is widened by a millimetre (more for
a far origin) for rounding. Characters rewound by lag compensation and
flyers' hitbox tests are as before.

Combat Arms' bound (`characters::hit_bound`): each sphere's centre stays
in a ball around its root's place (the root's translation, known
exactly) as wide as the translations' lengths from there down to its node
add up to (`skeleton::Rig::reach`; for the spec's skeleton
`skeleton::FallbackReach`): every local matrix is a rotation and a
translation, and the aim turns whole subtrees about one of their nodes,
which keeps every node's distance from its parent. The translations are
the pose's own (`Tracker::position`, `blend_layer_position`: no rotation
sampled), for the spheres' nodes and their ancestors only: 18-20k
instructions a call against 135-154k for the pose it stands for
(`heavy::characters::reach_holds_every_node_of_the_install_s_poses`, a
test build). It is loose: 3-4.5 m across a standing soldier (the head's
ball is 83 units, 1.5 m, around the feet; the hands' 122), so a ray within
a couple of metres of a body still poses it.

`party_kit`'s `heavy::bot_cost` now also counts the poses and hitbox sets
the ticks made (kept from each tick and read after the next: shots read
the last tick's), and `CHIMERA_BOT_COST_GUNS=cs_source` puts every bot on
the ladder's first CS:S gun (the default ladder starts on one already, so
the two runs are much alike). Instructions a tick on the schedules'
thread, 600 ticks, mean (the fights differ from run to run by more than
the change: 16 bots ran 23-29M before it on the same build), and the
hitbox sets made a tick:

| bots | before: mean | hitbox sets a tick | after: mean | hitbox sets a tick |
|---|---|---|---|---|
| 2 | 5.46M / 5.48M | 0.00 | 5.41M / 5.49M | 0.00 |
| 4 | 9.06M / 11.39M | 0.00 | 9.22M / 12.93M | 0.00 |
| 8 | 15.78M / 18.96M | 0.03 | 13.40M / 17.86M | 0.00 |
| 16 | 23.35M / 23.48M | 0.16 | 21.12M / 26.22M | 0.02 |
| 2, CS:S guns | 5.37M / 5.33M | 0.00 | 5.38M / 5.35M | 0.00 |
| 16, CS:S guns | 19.76M / 17.94M | 0.12 | 20.73M / 17.89M | 0.02 |

So seven in eight of the hitbox sets this match made are no longer made
(each a pose and its spheres, 135k+ instructions, for 18k for a bound),
some 15k instructions a tick with 16 bots: well under the run-to-run
spread. Shots at another game's bodies are rare in these 600 ticks (a
tick with one in some hundred); the saving is per ray, so it grows with
the shooting. The poses made a tick (now counted whichever skeleton was
posed, the client's view or the server's for bots' rounds) are 0.7, 0.07,
1.3 and 3.5 with 2, 4, 8 and 16 bots: Combat Arms' own rounds, which test
a body's box first.

CS:S's bodies keep ready `Hitboxes` (posed each tick on the server) and
declare no bound: the path is the same for them, so `perfgate`'s de_dust2
doesn't move (10.75G and 10.72G for 2,000 ticks before, 10.76G and 10.71G
after).

Guards: `cs_source`'s `weapons::a_far_ray_leaves_a_body_unmade_and_a_near_one_hits_as_before`
(a chest, a head and an arm out of the hull hit with lazy, bounded shapes
as with ready ones: the same damage and regions in the same order; the
body aside never made, a wide shot making nothing); `party_kit`'s
`cross_game::a_cs_source_round_poses_only_the_combat_arms_bodies_it_passes`
(a CS:S round at a Combat Arms head: that body posed, one 3 m aside
not); `combat_arms`' `characters::the_hit_bound_holds_every_sphere` (300
ticks of a duel, a body with a model and one on the spec's skeleton) and
`heavy::characters::reach_holds_every_node_of_the_install_s_poses` (the
install's bodies, a spread of their animations layered, crossing, looping,
crouched, aimed and dead: 0.9M nodes inside their bounds);
`lithtech_formats`' `anim::positions_are_the_poses_translations`.

### A change stamp for the movers

`sync_movers` worked out each mover's wants (its pose's rotation, its
colour, its panes) every phase and then looked at every mover's node
through a kept query to find, almost always, nothing to write. Now:

- Every slot of the logic world carries a change stamp, set whenever its
  entity may change (`LogicWorld::get_mut`, a spawn, a round restart's
  copy); `mark` and `changed_since` say which entities may differ from
  when a reader last looked. A mover's entity-derived wants (`MoverLook`)
  are kept between phases and worked out again only when its entity is
  stamped.
- A mover whose node read as wanted at the last phase (nothing written)
  is passed over without a look while nothing its wants or the node's
  test depend on changed: its entity, whether it rides a physics body
  (asked every phase: it depends on other entities), its brushes' count,
  its node's components (the change ticks since the last phase's writes:
  a filtered query, `MoverViewChanged`, with the world's last change tick
  set to the tick taken after the writes) and its archetype (a component
  added or taken away). Nodes two movers share are written every phase,
  as before. The rest are tested one by one through the kept query.

On mg_lego_multigames_v2, 767 movers, idle: some 500 of 767,000 mover
phases saw a stamped entity. `perfgate` (a playtest build, 2,000 ticks,
two runs each):

| instructions for 2,000 ticks | mg_lego_multigames_v2 | median tick | de_dust2 |
|---|---|---|---|
| before | 8.84G / 8.75G | 4.27M / 4.22M | 10.75G / 10.72G |
| the wants kept by the stamp | 7.95G / 7.95G | 3.82M / 3.40M | 10.71G / 10.73G |
| and settled movers passed over | 6.71G / 6.73G (-24%) | 3.27M / 3.25M | 10.76G / 10.71G |

(With the wants kept, a phase of `sync_movers` on mg_lego was 42k
instructions to find the nodes, 217k for the wants, 250k for the walk over
the nodes and 28k for the writes: the walk is what the second step
passes over.) `map_logic::idle_entities_cost_a_tick_little` now prints
7.0M instructions a tick with the skip against 13.6M writing every node
through (a test build).

Guards: `cs_source`'s `map_logic::skipping_unchanged_nodes_changes_nothing`
(every node the same after each of 260 ticks with and without the skips,
`SyncEveryNode` turning both off; now also with idle movers' nodes changed
from outside: a place, a visibility, the brushes taken away, a collider
switched off, a tint, each put back as before) and the other map-logic
tests.

## Render performance (2026-11-10)

The render world's three items from "A frame offscreen (`frame_bench`)"
above, and the light clustering. `frame_bench` (de_dust2, six bots, 600
frames at 1280x720, a `profiling` build, load 25-45) now also prints how
many prop materials and meshes changed a frame (each one prepared or
allocated again in the render world). Its fights differ from run to run;
medians of interleaved runs:

| | prop materials changed a frame | meshes changed a frame | main schedule p50 | render world p50 |
|---|---|---|---|---|
| before | 26.6-26.7 | 10.5-11.4 | 10.91-11.60M (median 11.24M, 9 runs) | 8.72-10.90M (median 9.08M) |
| after | 0.0 | 3.2-5.5 | 10.13-10.33M (median 10.20M, 3 runs; -9%) | 6.56-6.82M (median 6.63M; -27%) |

**A moving model's light, a face's aim: in a table, not in the
material.** A changed material is prepared again: for a non-bindless one
like `PropMaterial` that is a new uniform buffer and a new bind group. A
body's probe light was in each of its eight or so materials, rewritten
every 0.25 m it moved, and every face part's eye centre, up and iris
planes were written into its material every frame (`aim_faces`, three
parts a CS:S body: the 18 materials a frame of the table). Both now live
in one storage buffer every prop material binds (`probe_lit::ProbeTable`,
`render::gpu_table::SlotTable`): each lit entity (`ProbeLit`, a view
model's body) and face part (`bodies::FaceSlot`) has a slot, the material
says which (`PropParams::probe_slot`, `face_slot`, set once), and
prop.wesl reads the slot's rows where it read the material's. The buffer
is rewritten when a row changed, at the same size (Bevy writes a
same-sized `ShaderBuffer` into its GPU buffer), so the material and its
bind group stay. Guard: `prop_relight::moving_on_changes_the_probe_table_not_the_material`.

**Moving shadows: projected, not rebuilt.** A character's blob and a
moving physics prop's shadow were clipped against the world again
whenever it moved (every frame for a walking bot) and the mesh rewritten:
Bevy frees a rewritten mesh and allocates it again in its slabs. Now a
moving shadow's frame (origin, axes, size, reach, fade start, atlas cell,
the caster's lit faces) goes into a second table
(`shadows::ShadowFrames`, its slot the entity's `MeshTag`) and
shadow.wesl projects it per pixel onto receiver geometry kept around it
(`region_mesh`: the world's triangles in the frame's box grown by
`REGION_MARGIN`, 1 m), rebuilt only when the frame leaves that box: about
every 10-20 frames for a running bot instead of every frame. Resting
props' shadows keep their clipped meshes. Guards:
`prop_shadows::a_walking_character_s_blob_moves_by_its_frame` (6 m in 60
frames: at most 8 rewrites, 60 before) and
`a_moving_prop_keeps_its_shadow_mesh_and_moves_it` (the frame follows the
crate each tick, at most 3 rewrites in 20 ticks).

**Dust: quads placed by the shader.** A dust volume's mesh was its motes'
camera-facing quads, rewritten every frame. Now its mesh is only the
quads' corners and numbers (made again when the motes outgrow it), and a
storage buffer of the motes in view (position, half-width, colour, far to
near) and the camera's axes is rewritten instead; dust.wesl places the
quads (`dust::DustMotes`, an extension of the volume's unlit
`StandardMaterial`, whose fragment stage is unchanged).

**Light clustering with nothing to cluster.** Bevy clusters every 3D
camera's view into its default grid every frame, lights or not. While the
world has no point, spot or rect light, light probe or clustered decal,
the cameras now cluster into one cluster (`cameras::cluster_by_lights`);
the default grid comes back with the first light. The cluster count is a
uniform, not a shader setting, so the switch changes no pipeline, and a
single cluster draws any light correctly (only slower), so a light that
appears after the switch in its frame is still drawn.

**The picture.** `cs_source` offscreen at 1280x720 on de_dust2, third
person (`--spawn -7.5,0.6,-27.4 --look 0,-15 +thirdperson`, 300 frames),
before against after: the body's probe light and the blob shadow on the
floor as before (the body's shading the same; its outline moves by a
pixel between any two runs, the animation's phase, as does a swaying
cable). One difference is real: the blob's fade, at most 7 of 255, in
the parts of the floor and the wall beside the player where a receiver
triangle spans the fade's start (a projected shadow's fade is worked out
per pixel, a clipped one's per vertex; noted in `shadows`' known
shortcuts). Elsewhere the two pictures are the same. The view model
(probe lit from the table too) and refcmp's `dust_long_doors` view of
de_dust2 (`--views`): the same picture but for the motes, which drift (121
pixels, the motes' own); the motes are drawn as before.

Left: particles, steam and the other effects still write their meshes
every frame (most of the 3-5 meshes a frame left); a moving prop's
render-to-texture silhouette still rewrites its atlas image's cell.

## The dedicated server's memory (2026-11-11)

What the playtest rehearsal left (playtest-hosting.md, "Rehearsal"):
`ultimate_gun_game_server`'s resident plus swapped memory 2.4 GB at the
start, 5.24 GB after 3 maps, 5.53 GB after 6, about 100 MB more a map.

**The repro.** `heavy::server_map_memory` in ultimate_gun_game: the server
as its binary composes it (`ServerPlugin` over the party's `GameServer`, the
launch hook, 4 bots), a new ladder on another map each time (`ugg_new <seed>
13 <map>`, the rotation's path), laps over cs_office, warehouse, de_inferno
and de_dust2 in one process, the session the same on every run
(`Sim::repeatable`). After each map: glibc's bytes in use (`mallinfo2`),
what the process holds of its heap and anonymous mappings (resident or
swapped, `/proc/self/smaps`), the entities; a heap profile (jemalloc
preloaded with `prof:true`, a `prof.dump` after each map, `jeprof --base`
between visits of the same map) said what holds the bytes. Dev profile;
bytes, not times.

**What it found.** Not a leak of the live heap: after the first lap a map
had in use on its next visit what it had on the last (3-11 MB more a lap of
four maps, the archetypes' tables Bevy keeps; under 1 MB a map from the
third lap). Two things held the rest:

- glibc kept what a map change freed: 0.8 GB "free in arenas" after the
  first map, 1.3-1.9 GB after a few, counted resident or swapped, more after
  each map, the way the client's did before it gave them back (the section
  above): the server never did.
- The server decoded the Combat Arms models' textures it never draws: the
  world's textures were loaded as sizes only (`MapLoadOptions::textures`
  off), but not the character models' shared conversion (410 MB of RGBA
  texels and mips held for the process's life after the first map with
  Combat Arms bodies), nor the view models, hands, held models and
  effects' looks that dress every other game's map for Combat Arms
  (`world::DRESSING`: about 1.2 GB more for each map while it played,
  converted again for each map).

**The fixes.** Combat Arms' models' textures follow the load: a map loaded
without textures is `world::SizesOnly` while it loads (a memo), so
`model_textures` and the effects' looks keep sizes only, for the shared
character models too (`shared_character_models(.., decode)`: a load with
textures converts them again with texels, which serve both). The dedicated
server gives glibc's free pages back 20 s of its time after each map comes
in (`metrics::memory::release_after_change`, now the client's and the
server's), and maps its blocks of 4 MB and more for good
(`big_blocks_mapped`, `M_MMAP_THRESHOLD`: freed, they go back at once).

| | before | after |
|---|---|---|
| in use, first lap (MB) | 2,851-3,304 | 2,024-2,216 |
| in use, third lap (MB) | 3,141-3,323 | 2,057-2,237 |
| heap and anonymous held, first map (MB) | 3,688 | 2,166 |
| held after 4, 8, 12 maps (MB) | 4,744, 4,836, 4,878 | 2,224, 2,256, 2,313 |
| held after 28 maps (MB) | | 2,318 (2,157-2,392 on the last lap) |
| free in arenas (MB) | 844-1,863 | given back |

The test fails if the server has 2.5 GB in use on any map (2.9 GB with
HL2:DM installed), holds 500 MB more than it has in use, or a map has 32 MB
more in use on its third visit than on its second.

**With HL2:DM installed** (`CHIMERA_HL2DM_ROOT`, as the after-push run had
it): its parts join the ladder and its kit dresses every other game's map
for its players, 0.45 GB more in use (2.61-2.69 GB), of which 0.12 GB were
the guests' textures: HL2:DM's and CS:S's dressing of another game's map
(`dress_guest`) decoded them on a load without textures. They follow the
load now (`dress_guest_with(.., lean)`, `MaterialLoader::lean`): 2.35-2.56
GB. The rest is HL2:DM's bodies' and models' animations and its sounds.

**Left** (bounded, not per map): of the 2.2 GB in use, the Combat Arms view
models' animations sampled for each map's dressing (510 MB, converted again
at each map change), the character models' animation files (430 MB, once
per process: `model::ChildModels`), the effects' library (270 MB) and the
merged weapon data (230 MB). Whether the server needs the view models at
all (their timing is read from them) is open (backlog).

## A client's memory (2026-11-11)

What the playtest rehearsal left (playtest-hosting.md, "Rehearsal"): 3.5-5.5
GB resident per offscreen client at 640x360; the tester guide asks for 16 GB.

**The session.** `ultimate_gun_game --offscreen 640x360 +fps_max 30` (dev
build, the installs) joined to a temporary `ultimate_gun_game_server` on a
test port, the server's stdin a fifo: `ugg_new 7 13 cs_source:cs_office`
before the join, `ugg_new 7 13 combat_arms:warehouse` and `ugg_new 7 13
cs_source:de_dust2` two minutes apart (the same ladder each time). Every 30
s `chimera_mem 8` over the remote protocol (now with the largest groups of
images the main world keeps) and `VmRSS`; a second run with jemalloc
preloaded (`prof:true,lg_prof_sample:20,lg_prof_interval:31`), its last
dump before each map change read with `jeprof --collapsed` and summed by
allocation site. Bytes, not times.

**Where it went.** Not Bevy's main-world copies: the image store held
109-119 MB (Bevy's font atlases, which glyphs are added to on the CPU:
`MAIN_WORLD`), the mesh store 9-72 MB (72 MB on cs_office). glibc's trim
after a load (`release_after_change`) works: 20 s after a map spawns the
process holds what malloc has in use plus the driver's and the files'
mappings (0.6 GB); a load peaks before it (7.3 GB on the first map). The
heap in use, by site (jemalloc, MB; cs_office, warehouse, de_dust2):

| | before | after |
|---|---|---|
| Combat Arms bodies loaded on demand, their textures | 496, 1003, 952 | 57, 116, 119 |
| the shared character models' textures (`shared_character_models`) | 0, 406, 406 | 0, 41, 41 |
| view models loaded on demand, their textures (and their parts' copies) | 327, 661, 489 | 24, 61, 43 |
| held models loaded on demand, their textures | 27, 45, 39 | 2, 15, 15 |
| Combat Arms weapon sets | 494 | 249 |
| view models' animations (each map's dressing, on demand) | 393, 207, 391 | 343, 151, 381 |
| character models' animations (`ltb`; not changed here, the bodies loaded by then differ) | 259 | 365 |
| Source animations | 114, 130, 0 | 122, 132, 0 |
| the map's spawn (meshes, materials, entities) | 285, 261, 178 | 292, 263, 179 |
| the app (plugins, the GPU driver's) | 247 | 204 |
| the rest (Source and Combat Arms loaders, text atlases, sounds, the render world, ...) | 812, 922, 1,184 | 818, 920, 1,178 |
| total | 3,454, 4,635, 4,639 | 2,476, 2,517, 2,774 |

- The Combat Arms models kept past their map (bodies, held and view models
  loaded on demand, kept across map changes in `LateCharacterModels`,
  `LateHeldModels` and the view models' cache; the shared character models
  for the process's life) kept each texture three times: its DXT blocks,
  the RGBA8 texels decoded from them and their box-filtered mips (5.3
  bytes a texel beside DXT1's half byte). The GPU takes the blocks; nothing
  read the texels.
- The Combat Arms weapon set was parsed twice: the startup map (the menu's
  background) loads with install data of its own, freed after, and its
  weapon set (leaked, as every set) stayed beside the app's.

**The fixes.** A model loaded to be kept (`load_body`, `load_held_weapon`,
the view models' `load_weapon`, `shared_character_models`) is
`world::BlocksOnly` while it loads: a texture whose file holds whole DXT
blocks keeps those alone (`MapTexture::keep_blocks_only`, not decoded at
all); the drawing decodes them (`MapTexture::decoded`, `to_image`) where the
GPU takes no BC. A weapon set is parsed once per process for the same
documents (`store::interned`). The client's malloc in use and resident
(`chimera_mem`, glibc, 120 s after each map change):

| map | in use, before | after | resident, before | after |
|---|---|---|---|---|
| cs_office (the first) | 3.18 GB | 2.36 GB | 3.95 GB | 3.15 GB |
| warehouse | 4.32 GB | 2.39 GB | 5.17 GB | 3.21 GB |
| de_dust2 | 4.34 GB | 2.52 GB | 4.98 GB | 3.27 GB |

A map change's peak before the trim went too (5.1 and 7.3 GB 30 s into
warehouse and de_dust2 before, 3.1 GB after); the first map's is 6.1 GB
(7.5 before). The tests: combat_arms' `heavy::kept_textures` (a body, view
and held model loaded on demand keep no decoded texels beside whole
blocks: a body 38 MB -> 3.6 MB), `world::tests::blocks_only_keeps_the_blocks_alone`,
`data::tests::equal_weapon_documents_share_one_parsed_set`, chimera_present's
`images::tests::a_blocks_only_texture_is_decoded_where_bc_does_not_go_up`.

**Left** (backlog): the Combat Arms view models' animations, converted
again for each map's dressing beside the ones loaded on demand (260-290 MB
on another game's map), and the character models' animation files (260-370
MB); the weapon set itself (250 MB for 2,363 weapons); the main world's
meshes (72 MB on cs_office: merged brushes, breakables and face flexes read
or rewrite theirs on the CPU, so not all can drop them).

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

### The heavy tier: a sixth slot, and what conversion costs (2026-10-30)

Task brief [11](tasks/11-heavy-tier-time.md). The Linux dev box (12
threads, 31 GB, ~10 GB of swap already in use by idle processes), with
the installs (`CHIMERA_CONFIG` at the main checkout's local config, as
`tools/post-push-tests.sh` sets it), a warm build in an agent slot, no
other cargo or nextest process running and the 1-minute load under 4 at
each start (checked by a loop on the process list and `/proc/loadavg`).
`cargo nextest run --features dev --profile bench --no-fail-fast -E
'test(/^heavy::/)'`: 579 heavy tests, 4,053 others filtered out. CPU is
the run's children's user + system time (`times` after cargo), memory
sampled every 2 s: the summed RSS of the slot's processes and the box's
`MemAvailable`. A temporary line in `cs_source::bsp::load_level_bytes_with`
(not committed) logged each CS:S map load's `LoadTimer` total per test.

Every run: 572 passed, 7 failed, the same 7 each time and with five slots
too, so not caused by the change measured (main at 5e324989; to fix
separately): `combat_arms::it` `heavy::mount::install_mounts_its_archives`
(109 archives, 108 expected) and `heavy::timings::every_copy_times_on_its_own_view_model`
(4 copies); `mashup_demo::it` `heavy::dust2::a_mashup_map_loads_only_its_world`
("no bodies, no decals"); `party_kit::server` `heavy::dust2::` (three
gun-game tests: `combat_arms:beretta_m92fs` doesn't kill) and
`heavy::warehouse::a_cs_source_gun_on_warehouse_has_a_cs_source_body_and_each_player_a_different_one`.

| | 5 heavy slots (now) | 6 heavy slots |
|---|---|---|
| wall, three runs | 944.8, 940.1, 938.7 s | 801.5, 798.7, 805.7 s |
| median wall (spread) | **940.1 s** (6.1 s) | **801.5 s** (7.0 s): -14.7% |
| CPU (user + system), median | 4,265 s (4.5 of 12 threads busy on average) | 4,511 s |
| heavy tests' times summed | 3,153, -, 3,119 s | 3,357, 3,345, 3,381 s |
| peak summed RSS of the run | 20.4-21.3 GB | 20.0-23.3 GB |
| least memory available | 2,484, 2,884, 1,988 MB | **445, 0, 3,466 MB** |
| swap in use, start to peak | +0.1 to +1.3 GB | +1.4, +2.0, **+6.0 GB** |
| load, start / end (1, 5, 15 min) | 0.8-2.2 / 2.1-2.2, 4.3-4.6, 5.0-5.5 | 1.5-2.6 / 2.4-2.6, 5.6-5.8, 5.8-6.3 |

(The second five-slot run's per-test times weren't kept.) **Decision:
`max-threads = 5` stays.** Six slots gave more than the 10% and far more
than the spread, but the box ran out of available memory in two of three
runs and swapped 6 GB in the third: the criterion was at least 4 GB
available throughout, which even five slots miss on this box (2-2.9 GB;
the 2026-10-14 runs had 2.8-4 GB). A machine with more memory, or the
cache below making the map tests shorter (fewer of the big ones at
once), may change that.

The slowest tests (five slots, run 3; run 1 within 5 s of each):

| test | s |
|---|---|
| `party_kit::server` `spawns::every_pool_map_s_spawn_spots_are_inside_it` | 122.5 |
| `combat_arms::it` `body_skins::every_rung_bodys_skins_resolve` | 114.6 |
| `cs_source::it` `net_soak::soak_four_clients_and_bots_over_map_changes` | 114.6 |
| `cs_source::it` `map_de_nuke::ladders_can_be_climbed` | 47.3 |
| `party_kit::it` `bot_cost::each_bot_costs_about_the_same` | 37.7 |
| `combat_arms::it` `modes_part_two::bots_play_no_objective_on_grayhammer` | 36.7 |
| `cs_source::it` `net_capacity::a_full_server_keeps_its_tick` | 33.8 |
| `cs_source::it` `map_flows::jacks_every_game_keeps_players_in_the_map` | 33.2 |
| `cs_source::it` `served::de_dust2_loads_from_the_store_alone` | 30.9 |
| `cs_source::it` `map_flows::template_copies_are_drawn_and_solid` | 29.2 |
| `party_kit::server` `spawns::the_gun_game_spawns_spread_over_dust2` | 26.9 |
| `cs_source::it` `map_community::visual_entities_load` | 26.7 |
| `cs_source::it` `map_sound_stock::stock_soundscapes_proxies_and_room_settings` | 26.6 |
| `party_kit::it` `nav_bounds::bounding_keeps_every_floor_a_player_walks` | 26.0 |
| `party_kit::it` `css_map_moves::both_movements_shatter_a_cracked_cs_office_window_running_through_it` | 25.8 |

By test binary (summed, run 1): `cs_source::it` 2,000 s, `combat_arms::it`
510, `party_kit::it` 263, `party_kit::server` 213, `mashup_demo::it` 75,
`cs_source::ui` 51, the rest 42. The suite is shorter than on 10-14
(3,150 s summed against 3,035-3,457 then, with ~40% more heavy tests):
the lean load path and the courses' shorter runs.

**What converting CS:S maps costs** (five slots, run 1; runs 2 and 3
within 2%): 567 loads of 107 different maps took **999 s, 32% of the
3,153 s** of heavy test time. Only 169 s of it is each map's first load
in the run; **830 s is converting a map again** that an earlier test of
the same run had already converted. de_dust2 alone: 153 loads, 256 s
(66 of them in `map_de_dust2`, median 1.85 s each), de_nuke 40 loads 68 s,
cs_office 32 loads 45 s. By test file: `map_community` 203 s,
`map_de_dust2` 122, `map_courses` 112, `map_flows` 80, `map_stock` 68,
`map_surf` 61. Each test is its own process (nextest), so the
`OnceLock` a test file keeps its map in doesn't carry from one test to
the next. Combat Arms' loader wasn't timed. That is what the
converted-map cache would take back: its design is in
[asset-service.md](plans/active/asset-service.md), section 10,
"Converted-map cache"; the expected gain is ~15-20% off heavy wall
(about 940 s to 750-800 s) with five slots, from that 830 s at a hit
costing a fifth to a quarter of a conversion (to be measured).

### The converted-map cache (2026-10-31)

Task brief [11](tasks/11-heavy-tier-time.md), the design built
([asset-service.md](plans/active/asset-service.md), section 10;
chimera_assets `map_cache`, chimera_world `map::cache`). The Linux dev
box (12 threads, 31 GB), installs configured, warm build in an agent
slot; other agents' builds came and went (the 1-minute load is given).

**Per load** (cs_source `heavy::map_cache`, `--nocapture`; median of
the runs, the box at load 4-8): cold is a conversion with the cache off,
a miss converts and writes the entry, warm reads it back.

| | cold | miss and write | warm | entry |
|---|---|---|---|---|
| de_dust2, whole (textures decoded) | 1.62-1.65 s | 2.8-3.0 s | **0.22 s** | 986 MB |
| de_dust2, lean (`MapLoadOptions::lean`) | 1.18-1.19 s | 1.4 s | **0.11-0.15 s** | 227 MB |
| Combat Arms WAREHOUSE, world and its effect looks (opt-in) | 0.30 s | 0.67 s | 0.16 s | 222 MB |

A whole de_dust2 entry is textures 759 MB (the BC blocks and the RGBA8
copies with their mips), the kit 129 (bodies, held models), sounds 42,
the lightmap 36, the rest under 10. Where a warm whole load's time goes
(`CHIMERA_MAP_CACHE_LOG=1`): mapping 0.00 s, checksums 0.04 s (16 MB
pieces on threads), checking 2,557 inputs 0.03 s, decoding 0.15 s. The
first version read the file whole and decoded on one thread: 1.45 s,
no faster than 12 threads converting; reading the pieces on threads, the
big lists (textures, models, sounds) decoded element by element on
threads (`pod_serde::par_vec`) and the file mapped instead of read
brought it to 0.22 s. Combat Arms stays off by default: 0.14 s a load
for 222 MB a map (the effect looks, 0.03 s, are added after either).

Peak memory of one dust2 test (`map_de_dust2::ambient_samples_lie_in_their_leaf`,
`ru_maxrss`): 1.29 GB cold, 2.83 GB on a miss (the encoded entry beside
the map), 2.01 GB warm, of which the mapped entry's pages (about 1 GB)
are the system's file cache, shared by every process reading the entry,
not the process's own.

**The heavy tier** (cs_source's 363 heavy tests, the part with the map
loads; `cargo nextest run -p cs_source --features dev --profile bench -E
'test(/^heavy::/)'`, five heavy slots; cache folder emptied before the
cold run; runs interleaved off, cold, warm, off, warm):

| | wall | tests summed | CPU (user + sys) | peak summed RSS | least available | load start / end |
|---|---|---|---|---|---|---|
| off (1) | 708.5 s | 2,009 s | 2,371 s | 22.0 GB | 1.0 GB | 18.5 / 7.2 (other builds) |
| cold | 413.9 s | 1,113 s | 1,825 s | 11.4 GB | 10.8 GB | 7.2 / 8.3 |
| warm (1) | 375.3 s | 974 s | 1,556 s | 10.0 GB | 13.3 GB | 8.3 / 8.7 |
| off (2) | 438.6 s | 1,224 s | 1,678 s | 8.5 GB | 13.3 GB | 8.7 / 4.4 |
| warm (2) | 373.6 s | 970 s | 1,536 s | 10.6 GB | 13.5 GB | 4.4 / 11.9 |

Against the quiet off run: **wall -15% (438.6 to 374 s)**, the tests'
own time -21% (1,224 to 972 s), CPU -8%; a cold run is already faster
than off (-6%: after a map's first load in the run the rest hit). The
67 de_dust2 tests: 180 s to 101-105 s summed, median 2.09 s to 0.84 s
(they load whole maps). The wall falls less than the sum: the longest
tests (the network soak, de_nuke's ladders) aren't map loads. Peak RSS
counts the mapped entries' file pages, so it rose (8.5 to 10-11 GB)
while available memory didn't fall (13.3-13.5 GB at worst). The 363
tests fill the cache with 47 entries, 32 GB (`CHIMERA_MAP_CACHE_GB`
defaults to 40). All passed in every run.

### The converted-map cache, second pass (2026-10-31)

The follow-ups of the section above (backlog, "Converted-map cache, what's
left"). The Linux dev box (12 threads, 31 GB, ~15 GB of swap already in
use by idle processes); other agents' builds and tests came and went, so
each tier run below started only after a minute with no other checkout
building or testing and the 1-minute load under 5, and none ran beside
another checkout's (sampled every 2 s).

**A miss streams its entry to the file** (chimera_assets `map_cache`,
`ChunkWriter`: the payload a 16 MB chunk at a time, each hashed on a
thread, the header after it; format 3), instead of holding the encoded
entry beside the map. **Textures keep only their BC blocks** when their
RGBA8 image and mips are those blocks decoded (chimera_world
`map::texture_serde`, checked when the entry is written); a hit decodes
them again with the converters' decoder (texpresso), the biggest first.
de_dust2 whole, the old binary and the new interleaved (`heavy::map_cache`
with `CHIMERA_MAP_CACHE_LOG=1`, the box at load 10-16; peak memory of
`map_de_dust2::ambient_samples_lie_in_their_leaf`, `ru_maxrss`, two runs
each):

| | before | after |
|---|---|---|
| entry | 986 MB (textures 759) | **399 MB** (textures 172) |
| hit | 0.21-0.29 s (decode 0.14-0.17) | 0.24-0.68 s (decode 0.19-0.51) |
| hit, process CPU | 1.7-1.9 s | 2.2-2.4 s |
| peak RSS: cold / miss / hit | 1.33 / **2.87-2.92** / 2.08 GB | 1.33 / **1.50-1.51** / 1.51 GB |

A hit now decodes 630 MB of texels (about 0.5 s of CPU on threads), so
it is slower on a busy box (0.24 s with cores free, 0.5-0.7 s at load
10-15) but still 3-7x faster than converting (1.6-2.6 s); the lean entry
(no textures decoded) is unchanged, 227 MB, 0.10-0.11 s. Kept over the
bigger entry: a third of the disk and of the page cache per map, and a
miss's peak is now the conversion's plus 0.2 GB. A miss takes as long as
before (2.0-3.6 s, noisy).

**The whole heavy tier, warm** (every crate's `heavy::`, 625 tests:
`cargo nextest run --features dev --profile bench --no-fail-fast -E
'test(/^heavy::/)'`, the cache in a scratch folder filled by a first run,
24-28 GB; `.config/nextest.toml`'s `max-threads` set to 5 or 6 for the
run). Two changes in between: the first runs showed party_kit's
pool-map spawn sweep (2-3 minutes) always the last test, ending each run
alone, so it and Combat Arms' rung skins now start early (`priority =
90`); the runs below have it.

| | 5 heavy slots | 6 heavy slots | 5 slots, cache off |
|---|---|---|---|
| wall (nextest) | 646.2, 637.4 s | 544.4, 542.1 s (**-15.5%**) | 716.0 s |
| tests summed | 2,302, 2,274 s | 2,448, 2,442 s | 2,596 s |
| CPU (user + sys) | 3,241, 3,193 s | 3,363, 3,359 s | 3,282 s |
| peak summed RSS | 21.0, 19.6 GB | 22.5, 22.4 GB | 19.8 GB |
| least memory available | 1.4, 2.5 GB | **0, 0 GB** | 1.7 GB |
| swap, start to peak | +0.3, +2.1 GB | **+4.8, +3.4 GB** | +1.6 GB |
| 1-minute load, median | 5.6, 5.7 | 6.9, 8.7 | 5.1 |

All passed in every run. The cache takes the tier from 716 s to 637-646 s
(-10%; the tests' own time -12%, cs_source's -21%: 1,234 s to 969-989 s);
what is left is mostly not map loads (the soak, the pool-map sweep,
Combat Arms' rung skins, 105-120 s each, run whole beside the rest).
Before the priority change a warm six-slot run took 620 s (the sweep
last again); the earliest five-slot runs, 961-1,175 s, ran beside other
checkouts' tests and are not comparable. **Decision: `max-threads = 5`
stays.** Six slots are clearly faster (-15.5%, twice), but in both runs
available memory reached 0 and 3-5 GB went to swap, the same as on
10-30; the criterion stays at least 4 GB available throughout, which
five slots don't meet either on this box with 15 GB of idle processes
swapped (1.4-2.5 GB). A machine with more memory may take six.

**Combat Arms stays off by default** (`CHIMERA_MAP_CACHE_CA=1` to turn
it on). Its biggest maps (`load_world_in`, the whole world, a second
cold load timed; box at load 15):

| map (file) | cold | miss | hit | entry |
|---|---|---|---|---|
| 16ROOMS (83 MB) | 0.42 s | 0.58 s | 0.11 s | 245 MB |
| TERRORDOME (77 MB) | 1.46-1.55 s | 2.43 s | 0.49-0.51 s | 640 MB |
| NEMEXISWAR (66 MB) | 1.10-1.44 s | 3.53 s | 0.52-0.55 s | 641 MB |
| DESERTFOX_CP (64 MB) | 1.10-1.21 s | 2.93 s | 0.32-0.34 s | 418 MB |
| NEMEXISWARQ (59 MB) | 1.02-1.14 s | 3.65 s | 0.30-0.31 s | 634 MB |
| BLOODMONEY (58 MB) | 0.51-0.53 s | 0.96 s | 0.26-0.28 s | 416 MB |
| WAREHOUSE | 0.28 s | 0.39 s | 0.08 s | 222 MB |

A hit saves 0.2-1.0 s a load, a miss costs 0.2-2.5 s more than
converting, and the entries stay big: Combat Arms' textures keep their
texels (their mips are box-filtered, not decoded blocks, and their RGBA8
isn't texpresso's decoding of the blocks), and a TERRORDOME lightmap is
244 MB. Combat Arms' 155 heavy tests, five slots, interleaved: off
152.0, 150.7 s; on and warm 150.0, 148.4 s (-1.5%; the tests summed
681-686 s against 676-681 s; user CPU 734-735 s against 621-625 s); a cold
cache 170.5 s; it adds ~5 GB to the cache. Too little for the disk and
the misses; a player loading a big map again saves under a second.

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
changes to foundational workspace crates. Since 2026-11-10 the dev profile also splits
that debug info out of the binaries on Linux and builds without LTO;
`devfull` keeps it unsplit ("Iteration speed", "Debug info and LTO,
taken").

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

## Iteration speed (2026-11-10)

The whole loop, from an edit to knowing it works, measured on the Linux
dev box (12 threads, 31 GB) in an agent slot while two or three other
agents, the main checkout's commit checks and the after-push tests ran:
the 1-minute load is given with each number, and comparisons were taken
interleaved. Times are wall time; `CARGO_BUILD_JOBS` 4-6, sccache and
mold as `tools/agent-env.sh` sets them in a slot.

### Where the time goes

| Loop | Time | What it is made of |
|---|---|---|
| An edit in `combat_arms` (the most edited crate: 1,096 file changes in three weeks, 102k lines), then one test (`cargo test -p combat_arms --features dev --test it <test>`) | 31.8, 32.9, 41.0, 58.5 s (load 8-17) | rustc on the library 25.2 s (incremental; the edit was one new function), the test crate and its link 5.8 s, the test 0.1 s |
| The same, then every target (`--no-run`: library, its unit-test build, `it`, `ui`) | 36.8-57.9 s (load 13-29) | the library twice (with and without `cfg(test)`) |
| The commit hook after that edit | 168 s (load 14) | type-checking 11 dependents' targets, then Combat Arms' fast tier |
| Combat Arms' fast tier alone (1,659 tests in `it`) | 119, 130 s `cargo test`; 148-174 s nextest with 8 or 12 at once (load 13-25) | the tests' own simulation, at the dev profile's `opt-level = 1`; a test process starts in 20-40 ms |
| An edit in `chimera_sim`, then `cargo test --workspace --features dev --no-run` | 181-335 s (load 11-71) | the crate chain one after another (core, world, sim, net, client, the games, the party kit, the apps), then 69 test binaries and apps linked: 27.9 GB written |
| A slot's first build after merging main (`cargo test --workspace --features dev --no-run`) | 1,284 s (load 7-15) | 891 units one at a time (below); the registry crates from sccache took the first 186 s |
| Another selection after a workspace build (`-p <crate>`, `cargo build`) | 48-355 s each (below) | its own copy of the engine |
| The tests after a push (`tools/post-push-tests.sh`) | 52-125 minutes (the last 14 runs); the last: 16 min building, 53 min running 5,334 tests | the heavy tests: 10,873 s of test time on five slots, one test 1,096 s by itself |

Three things were broken rather than slow, and each made everything else
slower:

- **The shared jobserver was empty.** `tools/agent-env.sh` gives every
  agent's cargo one FIFO of 12 tokens. It held 0 (counted with `FIONREAD`,
  no build attached), 27 hours after it was made: a cargo killed mid-build
  (a tool timeout) keeps the tokens it took, and nothing returned them. A
  cargo without tokens runs one job at a time: the after-merge build's 891
  units took 1,283.1 s summed in 1,284.1 s of wall time (`--timings`),
  with `CARGO_BUILD_JOBS=5`.
- **Every selection had its own build of the engine.** A package that
  got a feature in one selection and not in another was built again, with
  every crate above it. After a full workspace test build:

  | then | before | after |
  |---|---|---|
  | `cargo test -p combat_arms --features dev --no-run` | 48.3 s (chimera_sim, chimera_net, combat_arms) | 2.4 s, nothing compiled |
  | `cargo test -p chimera_sim --features dev --no-run` | 137.2 s | 1.1 s |
  | `cargo test -p cs_source --features dev --no-run` | 272.4 s (8 crates, chimera_world up) | 0.8 s |
  | `cargo test -p cs_source --features dev,ui --no-run` | 243.9 s (10 crates, chimera_core up) | 1.0 s |
  | `cargo build --features dev` | 355.3 s (25 crates) | 17.6 s (five binaries no test builds, linked) |

  (Before at load 26-59, after at load 32-35, `CARGO_BUILD_JOBS=4`.) An
  agent that tests its crate with `-p`, commits (the hook checks the
  dependents) and runs the game paid for each of them after every edit
  low in the graph, and kept an incremental cache for each: one slot had
  five builds of `chimera_net` (770 MB of cache each) and six of
  `chimera_sim` from one day's work.
- **The disk filled** (8.4 GB free of 930 during these measurements; it
  had been 100% on 2026-10-31). The build folders held 493 GB, 238 GB of
  it incremental caches (a slot's: 57-65 GB after a day). A slot over
  its 60 GB limit was swept by size on its next `acquire`, and
  `cargo sweep` never removes incremental caches: a dry run on a free
  74 GB slot (28 GB of built crates and binaries, 44 GB of caches) would
  have removed 27.27 GiB, all of it built crates, so the next holder
  compiled everything again and was over the limit again.

### What changed

- `tools/jobserver-heal.sh`, run by each `source tools/agent-env.sh` (at
  most every 20 s; 42 ms): counts the tokens without taking them out and
  puts back the ones no running build can hold (no cargo attached: the
  full set exactly; with builds attached: the full set minus one per
  running job of each cargo). Measured on private FIFOs holding 0 and 12
  tokens, interleaved: the workspace build after a `chimera_sim` edit
  204.8 and 311.5 s drained (load 11, 71) against 180.7 and 334.9 s full
  (load 12, 17); a `combat_arms` edit with `-p combat_arms --no-run`
  51.0, 56.5, 42.4 s drained against 57.9, 55.1, 36.8 s full. So no
  measurable gain on an edit: one crate's rustc, or a chain of crates
  that each wait for the one below, has nothing to run beside it, and six
  links of 700-900 MB at once pushed the box to load 85 with 45-64% of
  CPU time waiting on the disk. Where the graph is wide (a new slot's 600
  registry crates, the after-push build of every test binary) the serial
  build above is what it costs; that case was not timed again with a
  full FIFO.
- `tools/agent-slot.sh`: a slot's trim removes old incremental sessions
  (past `CHIMERA_INCREMENTAL_MAX_GB`) before it sweeps by size.
- One build for every selection (Cargo.toml files only). What differed:
  `bevy_winit`'s `default`, an empty flag only the debug UI's `bevy_egui`
  asks for, so a selection without the client shell built another
  `bevy_winit`, another Bevy (the 850 MB dylib) and all our crates on it;
  the format crates' empty `dev`, on with `--workspace --features dev`
  and off with `-p`; `chimera_sim`'s `testkit`, `chimera_world`'s
  `render`, the games' `ui`, the `default` of `cs_source_app` and
  `party_kit`. Now every crate's `dev` turns on the `dev` of each
  workspace crate under it, the crates that take the presentation's Bevy
  for their tests take `bevy_winit` and `chimera_world`'s `render` too,
  and a game's tests build it with its front end (a dev-dependency on
  itself: `cargo test -p cs_source --features dev` now builds and runs
  `tests/ui/` as well; that the simulation builds without `ui` is still
  checked by the dedicated servers' builds). `architecture.rs`:
  `selections_share_one_build` compares every package's features in each
  member's `-p` test build with the workspace's (`cargo tree`, 7 s) and
  names the package that differs; `workspace_crates` checks `dev`
  reaches every workspace dependency. The dedicated servers stay a build
  of their own (no presentation), as before.

### Tried, not changed

- **The runner for a crate's fast tier.** Combat Arms' 1,659 tests:
  `cargo test` 130.5 and 119.3 s, nextest with 8 processes 173.9 and
  163.1 s, with 12 processes 148.4 s (load 13-25). The hook keeps
  `cargo test`.
- **Per-test process start.** A filter that matches nothing: 0.04 s; a
  test that builds a `Sim`: 0.06-0.25 s. Not where the fast tier's time
  is.
- **Debug info out of the test binaries.** Combat Arms' `it` is 702 MB,
  73 MB of it code (an app's test binary 860 MB: `.debug_str` 326 MB,
  `.debug_info` 169, `.debug_line` 86, `.debug_ranges` 76). Its link
  replayed five times each, interleaved (load 21): mold 0.80-1.46 s
  (median 0.95) as it is, 0.48-0.82 s (median 0.69) with
  `--strip-debug`, 137 MB; LLD 1.10-2.52 s against 0.80-0.94 s, 128 MB.
  A quarter of a second a link, so not a time win by itself; it is a
  disk and memory one (the 69 binaries of a workspace test build: 27.9 GB,
  about 5.5 GB stripped, rewritten after every edit low in the graph).
  Left as a proposal: it costs line numbers in backtraces or a rebuild of
  everything (below).
- **Where an incremental rebuild of `combat_arms` goes** (the stable
  compiler's `-Ztime-passes` with `RUSTC_BOOTSTRAP=1`, for measuring
  only; the library alone, one new function, load 10): 19.3 s, of which
  **`LLVM_thinlto` 12.3 s**, writing the crate's metadata 2.0 s, macro
  expansion 0.9 s, the dependency graph and query cache written back
  1.3 s; type checking and borrow checking under 0.4 s each. The dev
  profile's `opt-level = 1` with incremental code generation units makes
  rustc run ThinLTO over the whole crate's units on every build, changed
  or not. With `-Clto=off` on that crate, back to back (load 14-17):
  18.5, 20.5, 15.7 s against 26.8, 31.4, 26.9 s (medians 18.5 and 26.9 s,
  -31%); a full compile of the library 111.7 s against 130.1 s. Not
  switched on: Cargo allows `lto` only for a whole profile, so it also
  takes the cross-unit inlining out of Bevy and Avian (built with 256
  units), and the tests' and the game's run time with it was not
  measured (proposal 1).
- **The parallel front end** (`-Zthreads=8`, the same way): the library
  from scratch 73.5 and 73.3 s against 75.7 s with one thread; after an
  edit 18.8 s against 19.9-20.7 s. Nothing: the time is in LLVM, not in
  the front end. (Changing the flag throws the incremental cache away, so
  each switch cost a full compile.) No reason for a nightly toolchain
  here.

### Proposals that need a decision

1 and 2 were measured and taken the same day (below, "Debug info and
LTO, taken"); 3 is no longer needed.

1. **`lto = "off"` in `[profile.dev]`** (one line; `test` and `devfull`
   inherit it). Expected: 30-60% off every rebuild of a workspace crate
   after an edit (8-12 s of Combat Arms' 20-27 s; the same share of each
   crate up the chain, so of the 181-335 s workspace rebuild too). Cost:
   one rebuild of everything on every machine, and slower code in debug
   builds, ours and Bevy's, by an amount to measure first:
   `CARGO_PROFILE_DEV_LTO=off cargo test -p combat_arms --features dev
   --test it -- --skip heavy::` against the same without (119-130 s now),
   and `refcmp bench` on a dev build. If Bevy's share of the loss is the
   larger, add `codegen-units = 16` (or 1) under
   `[profile.dev.package."*"]`: the registry crates are built once and
   come from sccache.
2. **Debug info out of the linked test binaries.** Either `strip =
   "debuginfo"` in `[profile.test]` (both systems; a test's backtrace
   keeps function names and the panic its file and line, the frames lose
   theirs), or `-Csplit-debuginfo=unpacked` in the slots' environment
   (Linux only, beside mold in `tools/agent-env.sh`; everything kept).
   Expected: a workspace test build's binaries 27.9 GB to about 5.5 GB, a
   slot's whole build 52 GB to about 30 GB (under the 60 GB limit with
   its caches, so no sweeps), a quarter of a second a link, and no 28 GB
   rewritten after an engine edit. Cost: one rebuild of everything.
3. **The slot's size limits**, if 2 is not taken: one workspace test
   build is 34 GB of `deps/` and 18 GB of incremental caches, and the
   hook's type-check build comes on top, so `AGENT_SLOT_MAX_GB=60` is
   passed in normal use. Raise it to 80 (four slots, the after-push slot
   and the main checkout: about 450 GB), or lower
   `CHIMERA_INCREMENTAL_MAX_GB` to 20.
4. **Run `tools/jobserver-heal.sh` on the box's FIFO now.** It runs by
   itself once this branch is merged and an agent sources
   `tools/agent-env.sh`; the measurements here used private FIFOs so the
   other agents' builds were left as they were. With the tokens back, up
   to 12 compile jobs run at once across all agents again, which is what
   the jobserver was sized for, and more memory in use than in the last
   day.

### Debug info and LTO, taken (2026-11-10)

Measured in a slot (mold, sccache, `CARGO_BUILD_JOBS=6`) with the other
agents running; comparisons interleaved, the 1-minute load with each.

**Debug info: `split-debuginfo = "unpacked"` in `[profile.dev]`** (so
`test` too; `devfull` sets `"off"`). Three ways tried on the whole
workspace's test build (`cargo test --workspace --features dev
--no-run`, 71 binaries):

| | the 71 binaries | the largest (an app's `it`) | what a binary keeps |
|---|---|---|---|
| as it was (line tables in each binary) | 29.6 GB | 873 MB (`.debug_str` 333, `.debug_info` 169, `.debug_line` 86, `.debug_ranges` 76, code 73) | everything |
| split, DWARF 4 (taken) | 12.3 GB, plus 2.0 GB of `.dwo` files once per crate | 395 MB; the app 449 MB (`.debug_line` 84, `.debug_ranges` 76, `.debug_addr` 25, `.debug_info` 2) | line tables in the binary; the rest in the `.dwo` files |
| split, DWARF 5 (`-Cdwarf-version=5`) | 38.9 GB | the app 1,191 MB: `.debug_str` 653, `.debug_names` 194 | rejected: the name index brings the strings back into every binary |
| `strip = "debuginfo"` (not built; from the replayed links) | about 5.5 GB | 137 MB | function names only: a panic keeps its file and line, the frames lose theirs |

Split keeps more than strip: the line tables stay in each binary, so a
failing test's panic and every frame of its backtrace keep their file
and line (checked: a probe test panicking two calls deep printed
`panicked at source/formats/src/lib.rs:29:42` and `at ./src/lib.rs:29:42`,
`31:64` on its frames; in a probe crate, a frame in a dependency that
came from sccache kept its line too), and what a
debugger reads beyond lines (the variables) is in the `.dwo` files. The
other 7 GB that strip would save are those line tables and address
ranges. The slot after a whole workspace test build: `deps/` 19 GB
instead of 34, incremental caches 10 GB fresh (15 before, after a day
of builds), so about 30 GB instead of about 50 (`AGENT_SLOT_MAX_GB` 60
now has room for the hook's build and a day's caches; unchanged).

- **Windows.** Cargo passes `split-debuginfo` only where the target
  supports the value (`rustc --print split-debuginfo`; MSVC supports
  `packed` only, its `.pdb`), so Windows builds get the flag not at all
  (checked: `cargo build -v --target x86_64-pc-windows-msvc` on a
  probe crate with the setting: no `-C split-debuginfo` on its command
  line). No target-specific config needed.
- **sccache** caches split builds (a registry crate cleaned and rebuilt:
  3 of 3 hits, 0 misses), but restores only the rlib, not the `.dwo`
  files: line tables are in the rlib's objects, so backtraces stay
  whole; a debugger loses the variables of crates that came from the
  cache. Hence `devfull` (`debug = 2`, for inspecting locals) sets
  `split-debuginfo = "off"` and keeps all of it in the binary as before
  (checked: `.debug_info` in its binary, no `.dwo`, gdb's `info scope`
  lists a function's locals). In a slot the workspace crates build
  incrementally (not through sccache), their `.dwo` files beside the
  incremental session; a session `trim-incremental` removes takes its
  `.dwo` files with it (only variables, again, until the crate rebuilds).
- Cost: one rebuild of everything (the slot's: 20-28 minutes at load
  20-50; 572 registry crates missed the cache once).

**LTO: `lto = "off"` in `[profile.dev]`, with `codegen-units = 1` for
the registry crates** (`[profile.dev.package."*"]`, and
`lithtech_formats`, whose cipher must inline into its loop). `lto` was
not set: the default (`false`) is ThinLTO across a crate's own units,
which at `opt-level` above 0 and with incremental's units ran on every
build. Three builds side by side (same split debug info): as it was
("LTO"), `lto = "off"` alone ("off"), and off with one unit per registry
crate ("off + 1 unit", taken):

| | LTO | off | off + 1 unit |
|---|---|---|---|
| Combat Arms' tests after a one-line edit in `combat_arms` (`cargo test -p combat_arms --features dev --test it --no-run`) | 28.2, 31.3, 24.6, 36.8, 41.5, 42.7 s (median 34.1) | 11.2, 11.4, 22.9 s | 10.5, 10.8, 11.0, 14.2 s (median 10.9; load 6-10) |
| Combat Arms' fast tier (1,662 tests, the run alone) | 55.9, 55.9, 57.3 s | 67.7, 67.8, 68.0 s (+21%) | 62.3, 62.5 s (+10%) |
| cs_source's fast tier (525 tests) | 17.3, 18.0 s | | 19.2, 19.2 s (+9%) |
| party_kit `heavy::bot_cost`, instructions per tick (mean), 2 / 16 bots | 5.05M / 19.34M | 6.11M / 22.27M (+21% / +15%) | 5.41M / 21.37M (+7% / +10%) |
| `cs_source --offscreen 1280x720 --frames 1200 --map cs_source:de_dust2 +fps_max 0`, frame p50 / p90 | 3.18, 3.19 / 4.36-4.47 ms | 3.91, 3.99 / 5.39-5.44 ms (+24%) | 3.20, 3.22 / 4.62-4.75 ms (+1% / +5%) |
| the same, main world p50 / render world p50 | 2.43-2.49 / 2.61 ms | 3.14-3.18 / 3.19-3.25 ms | 2.61-2.65 / 2.59-2.60 ms (+7% / 0) |
| the main menu, 900 frames, frame p50 | 1.55, 1.57 ms | 2.05, 2.09 ms (+33%) | |

So most of what `lto = "off"` cost was Bevy's and the other registry
crates' 256 units losing their inlining across units; one unit each
gives it back (the render world as fast as before), and they are built
once a slot and come from sccache after (their first build: 13.0 minutes
with the three crates' tests, against 12.2 with 256 units). What is
left is our own crates at `opt-level = 1` without ThinLTO: the
simulation 7-10% slower, the fast tiers 9-10% (6 s of Combat Arms' 57),
against 20-30 s off each rebuild of a crate after an edit. Taken. If
the fast tier's share grows, `opt-level = 2` for the game crates in the
test profile is the next thing to measure ("What to do next", 5).

### What to do next, by expected gain

1. **The after-push tests** (52-125 minutes a push; the push is when a
   change is known to work). One test,
   `ultimate_gun_game::it heavy::session_materials`, is 1,047-1,096 s of
   a 2,300-3,200 s run, and three session tests of that binary 1,560 s;
   `cs_source::it`'s 363 heavy tests grew from 1,174 s (2026-10-31) to
   3,200-10,000 s summed with the community maps now present and the box
   busier; a run overlaps the agents' builds, which each slow the other.
   Shorten or split the session tests (one map lap instead of the
   ladder), give the heavy tier its own time or machine, and build its
   test binaries with the jobserver full (16 minutes now).
2. **Proposal 1**, measured and switched on: the largest cut to the edit
   loop found here. Done (above, "Debug info and LTO, taken").
3. **`combat_arms` as more than one crate** (the architecture plan's step
   14 asked for a measurement first): an edit costs 25 s of rustc before
   any test runs, and what is left after `lto = "off"` (metadata, macro
   expansion, the caches written back: about 7 s) grows with the crate,
   not with the edit. Its data tables and its `ui` are the natural
   first cuts; the tests (70k lines in one `it` crate, 5.8 s a rebuild
   and link) can follow the same cut.
4. **The crate chain.** core, world, sim, net, client and the games each
   wait for the one below, so an engine edit's rebuild is serial however
   many cores are free: 181-335 s. Narrower dependencies (a game on
   `chimera_net`'s types without its whole crate, `chimera_client`
   beside the games instead of under them) let cargo run them side by
   side.
5. **The fast tier's own time.** Combat Arms' 1,659 tests take two
   minutes on 12 threads, in our own code at `opt-level = 1`: try
   `opt-level = 2` for the game crates in the test profile against the
   longer compile, and find the tests that simulate seconds of play for
   one assertion (109 fast tests over 2 s were 41% of the fast tier's
   test time in the 2026-10-31 run).
6. **Seeing a change without restarting the game**: Bevy 0.20's
   `hotpatching` feature (systems patched into the running app by the
   Dioxus CLI's `dx serve --hot-patch`) for a human at the game; not
   tried here (no display on this box).
