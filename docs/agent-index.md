# Docs index

Generated from the headings of `docs/*.md`: do not edit. Each entry is the
line a section starts at, then the first sentence under it, so a reader
opens the section it needs instead of the whole file. Regenerate with
`cargo run -q -p repo_checks --bin docs_index`; the architecture test
`generated_docs_are_fresh` fails when this file is stale. The per-file
module list is `docs/modules.md`.

## docs/ARCHITECTURE.md

- 1: Architecture | A map of the code
- 6: Crates | A Cargo workspace (docs/plans/active/engine-split.md, "Crates and compile time"); every member is a default member, so `cargo test` and...
- 322: Layers | Exact edges: game_log uses core, console and gameplay (the gameplay's game names players in the log); net uses the engine's simulation...
- 425: Modules | The module list is `docs/modules.md`: one line per source file, grouped by crate folder, generated from the first sentence of each...
- 436: Data flow per tick | Intent buttons are levels, not edges
- 504: Slots | An implementation registers under a namespaced ID (`chimera:noclip`,

## docs/DESIGN.md

- 1: Chimera: design | The design behind Chimera and the games built on it: why the layers, slots, mounts and weapon model look the way they do, and the legal...
- 8: Goal | Pull a "microcosm" out of an existing game (a map, its movement feel, a few weapons, its HUD) and make it playable in Lucker Party
- 21: Vision | Layers, each using only those below it:
- 55: Scope for now | to test the shared core without fighting it, and different enough to expose CS-shaped assumptions
- 67: Engine direction | Leaning Bevy (Rust) for this prototype
- 75: Architecture
- 77: Mount layer | One adapter per game
- 146: Slots | A match is a loadout that picks one implementation per slot
- 164: Intent | Nothing reads the keyboard directly
- 173: Interfaces | Each slot declares what it provides and what it needs, so any implementation that satisfies the contract can be swapped in:
- 190: Bundles | Some parts belong together: MW2's M4 model, reload animation, hands and fire sound
- 196: Weapons and items | "Weapon" is not a type
- 220: Components and categorization | Weapon parts
- 238: ECS style | Prefer, in order: components for facts about an entity, observers for reactions to them (`On<Insert<C>>`, `On<Discard<C>>`), messages...
- 275: Normalization and mixing rules | standard player's health, speed relative to the host's walk speed, fire rate in seconds
- 296: Impacts across games | Any game's gun works on any game's map; where a game lacks something, Chimera has a generic fallback (`chimera_world::map::impact`,
- 319: Hits across games | Any game's gun hits any game's body by one game-agnostic interface (`chimera_core::core`, `chimera_sim::weapon::hits`); nothing is...
- 392: Announcers | Kill-streak and special-kill callouts (Combat Arms' DOUBLE KILL ... UNBELIEVABLE+, FIRST KILL, HEADSHOT, NUTSHOT, REVENGE) are...
- 471: Gun game rules | The party's gun game (`chimera:ladder`, `chimera_sim::minigame::ladder`) follows the common GunGame server conventions, each a setting...
- 591: Automatic gun games (variety) | Ultimate Gun Game's ladder and maps are made, not written (`chimera_sim::minigame::auto`, docs/plans/active/ultimate-gun-game.md):
- 619: One HUD layer over several games' HUDs | In the party apps the HUD follows the gun in hand, so anything that must survive a swap or look the same on every HUD is the shell's...
- 652: The party apps' Escape menu | Ultimate Gun Game and Lucker Party have a menu of their own (`party_kit::esc_menu`), not one game's: Combat Arms' in-game menu yields...
- 678: Multiplayer | Same principles as the main game: the server owns the loadout, rules and outcomes; clients submit intent
- 685: Assets, code and legal | install (the Garry's Mod model)
- 710: Per-game notes | directories are version 2 (`cstrike/cstrike_pak_dir.vpk`, and
- 725: Specs | Behavior worked out from reference material reaches this repository only as specs in `specs/`, written in our own words: constants with...
- 735: First steps (the original plan, 2026-10; kept for the record) | MVP: a Combat Arms slice
- 757: Open questions | by dice, or a mix?

## docs/OBSERVABILITY.md

- 1: Observability: how to check your work | Agents can't see the screen
- 6: 1. Headless scenario tests (fastest, most precise) | ticks, `Intent` driven directly
- 238: 2. Screenshots | With no `--map` the game starts at CS:S's main menu (the greybox loaded unseen behind it), unless `--spawn`, `--look`, `--movement` or...
- 319: 3. Live ECS over HTTP (Bevy Remote Protocol) | With `--features dev`, the game serves JSON-RPC on `localhost:15702` (tool runs: a free port, section 2)
- 342: 3b. The console from outside | action from the start, e.g. to fire in a `--screenshot` run; held console actions work without mouse capture
- 715: 3b2. Network play | Plan and progress: [plans/active/multiplayer.md](plans/active/multiplayer.md)
- 1116: 3c. Performance | Details and baseline numbers: [performance.md](performance.md)
- 1319: 3d. The debug UI | F8 (a bind: `debugui`; F1 and F2 are CS:S's autobuy and rebuy) opens a tabbed window; `debugui <tab>` opens it on a tab (`player`...
- 1366: 3e. Several agents at once (builds and tests) | Source `tools/agent-env.sh` in each agent shell, then run builds and tests through `agent_run`:
- 1390: Shared compile cache (sccache) | worktree's first build reuses the registry crates (Bevy and the rest) that any other worktree already built
- 1433: Build slots (warm checkouts for agents) | A fresh worktree rebuilds every workspace crate: cargo fingerprints them by absolute path, so neither sccache nor a shared...
- 1482: 4. Logs | (movement swaps, mounts, match events) at `info`; per-tick detail at `trace`
- 1494: 5. Exploring game files | same readers as the runtime mount
- 1728: Which files a run reads (`CHIMERA_VFS_LOG`) and the asset store | mount (the game, a test, a tool, a dedicated server): one line per file and process, `<game>\t<path>\t<size>\t<sha256>`, appended...
- 1756: Sweeping every cached map (mapsweep) | (`~/.local/share/chimera/content/cs_source/maps/`) headless, each under
- 1856: 6. Comparing with the real game (refcmp) | reports, per view, brightness (luma), saturation, sharpness (mean absolute Laplacian) and mean absolute difference, plus side-by-side...
- 1930: Comparing the HUD (`refcmp hudcmp`) | (default `<dump>/refcmp/hud/ref`; game imagery, never in the repo)
- 1959: Measuring CS:S behaviour live | The reference CS:S (see refcmp) can also be measured directly over RCON:
- 1977: Tick-exact CS:S comparisons (movecmp) | inputs
- 2049: CS:S demos (demodump) | A SourceTV demo recorded on the probe server (tools/css_probe/README.md, "Recording a SourceTV demo") holds everything the server sent...
- 2090: Playing a demo in our game (`playdemo`) | in the console; a bare name is looked for in the install's `cstrike/`) loads the demo's map and plays it: its players are puppets...
- 2109: Re-simulating a client demo (`demosim`) | A client (POV) demo holds the recording player's user commands every tick (`dem_usercmd` frames: buttons, move amounts, view angles...
- 2180: Planned | the original game's measurements

## docs/backlog.md

- 1: Backlog | Things to build, **in priority order** (top first; reprioritized
- 9: Architecture review (2026-10-20) | Address the feedback in
- 26: Roadmap after the gun game settles (user, 2026-10-29) | per-file packer, local cache mount, recording what a load opens), then the ECS cleanup, then the main menu
- 100: 0. Playtest feedback (2026-10-07), top priority
- 102: Gun game playtest, 2026-10-28 (user) | projectiles" (user, 2026-10-29): fixed, confirm in a playtest.** Not the client's interpolation (every `NetItem` was already drawn...
- 1166: Gun game playtest, 2026-10-25 (user; start after the usage reset) | (promo capture 2026-10-28, cs_office, Ultimate Gun Game)
- 1279: Gaps vs the originals (audit 2026-10-27) | specs/ compared with the code, spec by spec: plain CS:S and Combat Arms matched their specs wherever the docs don't already say...
- 1289: Gaps vs the originals, second pass (2026-10-28) | What a player of the gun game (Ultimate Gun Game, Lucker Party) or of plain CS:S and Combat Arms would notice, where no spec covers the...
- 1506: Decisions 2026-10-13 (user) | steps 1-2 (tools package; move code to its future home in the one crate), then crates
- 1533: Playtest with friends, 2026-10-10 (Discord; build v0.1.0-playtest2 or 1) | mg_jacks_multigames_v1, `setpos -12146.96 15343.41 -927.97; setang
- 1828: 1. Playtest essentials | A (settings, health bars, third person), B (penetration and collaterals, docs/plans/active/weapons.md slice 5) and C (view models) are done
- 1851: 2. Custom maps and minigames | Plan: [plans/active/custom-maps.md](plans/active/custom-maps.md)
- 1909: 2b. HUD and debug views | and the Tab scoreboard are in (`client/game_hud.rs`,
- 1984: 2c. Multiplayer | Plan: [plans/active/multiplayer.md](plans/active/multiplayer.md) (bevy_replicon + renet; Source-style prediction, interpolation and lag...
- 2043: 2d. CS:S demos as a parity check (user idea, 2026-10-14) | Built: `demodump` reads CS:S `.dem` files (`source_formats::dem`; SourceTV and, untested, client demos; OBSERVABILITY.md "CS:S demos")
- 2073: 3. Weapons, remaining | In progress: [plans/active/weapons.md](plans/active/weapons.md)
- 2151: 4. Bots | Parked (user, 2026-10-13): bots are fine for now; focus on what players see
- 2158: 5. Console, remaining | The console and overlays are in (chimera/core/src/console.rs, chimera/client/src/client/console.rs)
- 2171: 6. Sound, remaining | docs/plans/active/sound.md
- 2190: 7. Physics props, remaining | follow-ups (docs/tech-debt.md "Physics shadow"): the push speed limit (spec Q7), the shadow's weight on a prop stood on, the...
- 2220: 8. Visual fidelity | planar reflection, the cheap cubemap pass, under-water fog (world, props, ropes, decals, particles; also in water of other shaders...
- 2277: 9. Performance | Measured and culled (docs/performance.md): `chimera_perf`, `refcmp bench`, PVS culling by world chunk, areaportals (closed doors hide...
- 2373: 10. Long tail | Counts are from de_dust2's entity lump and static prop lump

## docs/economy-data.md

- 1: Economy data | The shop is data loaded at run time, as weapons are (docs/weapon-data.md; docs/plans/active/engine-split.md, "Data-driven first"): a...
- 19: The document
- 62: Rules | helmet already worn; `helmet = true` adds one; over the amount already worn without a helmet it sells the helmet alone for...
- 90: Round numbers | CS:S's rule module's numbers (`rules::rounds::RoundSettings`: timers, start and most money, bonuses and rewards) are
- 98: Not data yet | Bots' buying thresholds and grenade chances (`economy::BOT_*`, ours, the engine's), the default autobuy and rebuy lists (`AUTOBUY_DEFAULT`,

## docs/performance.md

- 1: Performance | How to measure, what the numbers were, and what the map visibility culling does; at the end, how long the tests take to build and run...
- 7: Measuring | per render pass (Vulkan/DX12 timestamps; every camera's run of a pass summed per frame: Bevy reports each as its own measurement under...
- 81: Visibility culling (`render::vis`, `map::vis`) | Source maps carry precomputed visibility: the BSP tree's leaves belong to clusters, and the visibility lump lists, per cluster, every...
- 181: Checks | per map (de_dust2, de_nuke, cs_office, cs_assault), 400 rays from eye height; every world surface a ray reaches (translucent ones on the...
- 240: Baseline and results | RTX 3080, Linux, 1280x720 offscreen, dev box under other agents' build load
- 329: Many brush entities (mg_lego_multigames_v2) | A community minigame map (BSP v20, 578 func_breakable, 170 func_door, 937 brush models, 32k triangles, reflecting water)
- 442: Community maps' slow first views (mapsweep) | The map sweep's screenshots (docs/plans/active/community-maps.md) timed six first views at 45-341 ms a frame
- 510: Cheap wins found | every frame, which re-prepares it for the GPU even when nothing changed; now only when the colour changes
- 548: Frame time pass (executors, dynamic meshes, posing, prop shadows, resolution) | Goal: frame rates near a 240 Hz monitor's at 4K
- 556: What cost the most | motes (one per map on dust2): Bevy's mesh allocator frees and re-allocates a modified mesh in a buffer shared with other meshes, and...
- 630: Before and after | before this pass, multi = this pass with `CHIMERA_EXECUTOR=multi`, new = this pass
- 688: Props that never sleep (2026-10-16) | Counted with `physics: N dynamic bodies, M awake` (`chimera_perf`) and
- 835: Fewer cameras per frame (2026-10-16) | The view-model camera ran its passes every frame: clearing its depth, its (empty) opaque, transparent and post-processing passes, and...
- 884: Back faces in the depth prepass (2026-10-17) | Main views get a depth prepass on maps with water (`render::water`; also with `chimera_skyocclude 1`)
- 933: Load-independent metrics | Every number in "Frame time pass" was taken at load 6-33, where single runs swing 2-10x
- 957: How stable they are under load | per map at load 8-22 (`ALL` rows: every view's frames together; spread
- 998: Baseline | three runs; frame ms p50 / p99 / p99.9, 1% low fps, CPU ms p50, main instructions less ticks per frame p50 / p99, render instructions...
- 1049: Regression gate | fixed inputs: `greybox` (two characters walking, turning and jumping on a fixed pattern), `de_dust2` (rounds, 5 bots a side...
- 1090: Light-style hitches (mg_jacks_multigames_v1) | Report: frame p50 ~5 ms but p99 ~20 ms and a constant stream of hitch log lines
- 1180: Combat Arms map loads (2026-10-19) | A Combat Arms map took 4-8 s to load and spawn and ~1-1.8 GB in the loader (~10 GB and 17 s at 0a9bd397, when every install weapon's...
- 1256: Lean map loads (2026-10-21) | N3 of the architecture review (F1): `MapLoadOptions { hdr_level, textures }` on every `MapLoader`; the dedicated servers and 27 CS:S...
- 1312: Map changes behind the loading screen (L4, 2026-10-23) | Architecture review L4, stage 3
- 1367: Lucker Party (2026-10-24) | The gun game ran at 40-50 fps on the user's Windows PC, which runs CS:S (and our `cs_source` app) smoothly
- 1443: Party frame-rate pass (2026-10-28) | Goal: where the Lucker Party gun game's frame time goes, by code, with pictures to compare before and after
- 1452: Method | plus line tables) with frame pointers, `perf record -e cpu-clock:u
- 1472: Baseline | Medians of the three rounds (range); frame ms from the 4096-frame readout, instructions per frame p50 (main world without its ticks, per...
- 1503: Hot spots, ranked | ms of CPU a frame (inclusive) in the de_dust2 "after" profile, ~125 fps; "before" where a fix moved it
- 1526: Fixed | F, _>(world, |q, world| ..)` lends a `QueryState` kept in the world (one per query type) to the closure, and `first::<F>(world)` is the...
- 1555: Left | The render thread sets the frame now
- 1567: Map changes that slowed the tick (2026-10-28) | Playtest report: the gun game's de_dust2 started at ~240 fps and ran much slower after a few map changes in one session; more bots...
- 1656: Second pass: the cameras (2026-10-29) | Same scenes and tool, with the local player held still (`--spawn` and
- 1769: Third pass: the schedules, the HUD, per-bot animation (2026-10-29) | Same scenes, `--spawn`/`--look` and `+sv_cheats 1 +god` as the second pass (de_dust2 `--spawn -10.69,0.0,-52.07 --look 90,0`, cs_office
- 1902: See a frame yourself | Three features, on every app with a window (`cs_source` at the root,
- 1952: Test cycle | How long `cargo test` takes to build and run, and why the tests are laid out as they are (docs/OBSERVABILITY.md section 1 has the commands)
- 2032: The heavy tier (2026-10-14) | Measured with `cargo nextest run --features dev` (all 1,504 tests passing), a worktree's own `target/`, other agents building (load 24...
- 2077: The push hook (2026-10-19) | Pushes took 20-60 minutes: the selection fell back to the whole suite for a lockfile change or when the compared commit wasn't in the...
- 2124: Warm slots, a lighter commit, push first (2026-10-27) | A one-line fix took ~45 minutes to land: a new worktree's first build
- 2192: Sharing dev and test artifacts (2026-10-30) | For Windows test-run selection, see the repeated
- 2233: The first crate: `source_formats` (2026-10-14) | The repository became a workspace and the pure-data readers moved into
- 2253: `chimera_world` and `chimera_present` (2026-10-14) | setup as above (own `target/`, sccache, `CARGO_BUILD_JOBS=4`, other agents building), a new `pub fn` appended to one file, then reverted...
- 2277: `chimera_sim`, `source_logic` and `cs_source` (2026-10-14) | and the rule-module interface into `chimera/sim` (8.1k), `mount` into
- 2304: `chimera_client` (2026-10-14) | The shell's base (`front_end`, `fonts`, `hud_text`, `widgets`, `binds`,
- 2322: `chimera_net` (2026-10-14) | hostages went to `cs_source::game::net`, the movers to
- 2340: The shell in `chimera_client`, CS:S's front end in `cs_source` (2026-10-15) | The rest of the client shell moved into `chimera/client` (19.3k lines with the base) and CS:S's front end with its HUD into `cs_source`...
- 2368: Workspace-hack (2026-10-15) | that crate's own feature set: 21 third-party crates (`bevy_reflect`,
- 2404: Dedicated servers without Bevy's default features (2026-10-22) | The workspace's `bevy` has no default features (Cargo.toml lists the simulation's: `bevy_asset`, `bevy_log`, `bevy_state`, `bevy_image`,
- 2439: Bots' sight in parallel (2026-10-22) | finds each living bot's nearest living enemy in sight (nearest first, PVS first, then a ray and the smoke test) on the compute task pool:

## docs/tech-debt.md

- 1: Known technical debt | Small, current, honest
- 14: Without a file
- 34: From the code (generated)

## docs/weapon-data.md

- 1: Weapon data | Weapons are data loaded at run time (docs/plans/active/engine-split.md, "Data-driven first"): a TOML document per weapon, parsed and...
- 21: Documents | A document holds a weapon (a top-level `id`), ammo types (`[ammo."<id>"]` tables), or both
- 153: Rules | inches: 0.0254) and health per damage point (hit points of 100: 0.01); both default to 1
- 209: Loading more | data (games/cs_source/tests/data/weapon_rail.toml), buys it, fires it and plays it over the network
- 221: Penetration by material | How bullets pass walls and characters (`PassMaterials`) is a game's document too, read by `PassMaterials::parse` (`weapon::pass_data`)...
