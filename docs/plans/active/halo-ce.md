# Plan: Halo: Combat Evolved (Xbox) on Chimera

Started 2026-11-15, from the brief
[docs/tasks/halo-ce-port.md](../../tasks/halo-ce-port.md). One long
orchestration session on the Windows PC runs it: it plans, delegates spec
work and implementation to sub-agents, verifies, merges and pushes, one
milestone after the other. This file is the status page: read "Status"
and "Log" to catch up.

Goal: Halo: Combat Evolved as the original Xbox disc has it is a fourth
game library (`games/halo_ce`, crate and id `halo_ce`) that lends its
guns, movement, bodies, HUD pieces and maps to the Ultimate Gun Game the
way Counter-Strike: Source, Combat Arms and Half-Life 2: Deathmatch do
([new-game.md](../../new-game.md)). Parity first: by default a Halo rung
plays as the real game, from specs and the disc's own data.

## Status

| Milestone | State | Summary |
|---|---|---|
| M0 specs (formats and core rules) | done (2026-11-15) | eighteen specs in `specs/halo_ce/` (its README is the index): the disc image, the cache file and tag conventions, bitmaps, structure BSP, collision, models, shaders, scenario, sounds, animations, objects, globals, movement, health and shields, weapons, projectiles, multiplayer rules, effects; each checked against the disc by its writer, corrected once from the readers' tests; still to write, as their milestones near: the HUD and vehicles |
| M1 a map you can walk on | done (2026-11-15) | every multiplayer map loads from the disc image with textures, one lightmap atlas, the sky model, scenery and collision; Halo's own app and server (`halo_ce`, `halo_ce_server`); six maps in the party apps' pool, played by the other games' characters; summary below |
| M2 Halo guns as UGG rungs | done (2026-11-15) | the assault rifle, pistol, plasma rifle and shotgun as data-driven rungs with their view models, animations, sounds and effects; a Halo rung's holder is a Halo player (movement, shields over health, the Spartan, Halo's HUD); a mixed ladder climbed by real fire as a party test; Halo's sets published to the asset service; summary below |
| M3 the full multiplayer arsenal | in progress (2026-11-15) | simulated: the sniper rifle, rocket launcher, needler, plasma pistol and flamethrower, frag and plasma grenades, explosions, the level's materials at each hit, pickups on the item clock; four more rungs (eight in all); Halo's HUD from the disc's tags (shield, health, ammunition, heat, crosshairs, motion tracker). Being built: the five guns in hand with their sounds and effects, grenades thrown and seen, pickups seen |
| M4 every multiplayer map | done (2026-11-15) | teleporters for every game's characters (`chimera_world::map::teleport`, from the scenario's pairs), climbable surfaces as ladders (Halo's movement climbs them by movement.md, the others by their own rules), no machine play depends on (Wizard's 4 alert lights), twelve of thirteen maps in the pool (Sidewinder waits for vehicles), the pool measured with teleporters as links; the bots' generated mesh held to the drawn level (`nav_gen::NavWithin`, the structure BSP's world bounds) and grown to it (Blood Gulch's 78,000 m2 whole), teleporters as navigation links the bots plan through and walk onto (`Via::Teleport`); ladders for bots open (backlog) |
| M5 feel and parity pass | not started | |
| M6 vehicles | not started | |
| M7 and on (effects, ragdolls, the classic look, the backlog) | not started | |

## 1. The two rules

1. **Clean room.** The behaviour reference is a decompilation under a
   copyleft licence. It lives outside the repository and only spec
   agents open it. Its behaviour reaches the code only as
   `specs/halo_ce/` in our own words (prose, tables, math; no code
   blocks). Implementation agents and the orchestrator read specs and
   this repository only, and are never given the reference's location.
   The spec index and this port's extra rules for spec writers are in
   [specs/halo_ce/README.md](../../../specs/halo_ce/README.md).
2. **No game data in the repository.** Maps, tags, extracted or converted
   files never enter git. The game reads the disc image through its
   mount at run time; converted data goes to the per-user caches.

## 2. Decisions

- **Id and crates (2026-11-15).** The game is `halo_ce`. Its engine
  family gets a loader layer of its own, `blam/` (the engine's public
  name), with `blam_formats` (`blam/formats`): the disc image, the map
  cache files and the tag classes as plain data, no Bevy, on
  `chimera_vfs` and `chimera_formats` only, as `lithtech_formats` is.
  Then `games/halo_ce` (`loaders = ["blam"]`), and `apps/halo_ce` with
  `apps/halo_ce_server` when its own app is worth having (the party apps
  come first; section 4).
- **The install is a disc image (2026-11-15).** `[games.halo_ce] path`
  names the player's own Xbox disc image (`.iso`), or a folder holding
  the disc's files. The image is a mount layer (`blam_formats`' reader
  of the disc's file system as a `FileSource`), so the mount shows the
  disc's files (`maps/<name>.map`, ...) and `mount::served` serves the
  asset store instead of it for a player without the disc (the engine
  layers a store under an install only by a Steam build id, which this
  game has none of). Tools, tests and agents name the image with
  `CHIMERA_HALO_CE_ROOT`, the engine's per-game variable, so no local
  config file changes.
- **Halo's files go to the asset service (user, 2026-11-15).** This
  replaces the brief's "never publish" for this game only: players
  without the disc get Halo's files from the asset service at the join,
  as they get the other games'. The whole disc as the mount shows it is
  published (every map, not only what a match reads today), with small
  sets on top (`base`, `map/<name>`, `weapon/<id>`). How: `chimera_pack`
  runs on the Windows PC against the disc image into a store there;
  only the new records and the game's manifest are copied to the asset
  server's store; the manifest is signed on the server with its key.
  Nothing is built on the server's machine beyond, at most,
  `chimera_pack` itself (user: that machine is busy). The other games'
  manifests and sets are never touched. Ownership rests on the testers'
  word, as in the internal phase of
  [asset-service.md](asset-service.md); before any public exposure the
  user decides again.
- **Spec review (2026-11-15).** [specs/README.md](../../../specs/README.md)
  asks for a human review of each spec before it is implemented. This
  session runs unattended by the user's brief, so the orchestrator
  reviews each spec (for completeness, internal consistency and the
  no-code rule) and the specs stay `draft` until the user reviews them.
- **No releases, tags or Actions; never force-push `main`.** Other
  sessions push to `main` too: merge `origin/main` before each push.
- **Lane.** New code in `blam/` and `games/halo_ce/` (and its apps).
  Shared crates change only through small generic extension points
  (new-game.md). The UGG UI and the platform services belong to another
  session: no restyling, no service changes.

- **What the specs settled (2026-11-15).** One world unit is 3.048 m
  (10 ft); +Z is up, +X forward, +Y left; the game ticks 30 times a
  second. A map is a 2 KB header and one zlib stream, inflated whole.
  Every multiplayer map holds the same 15 weapons, 16 projectiles, 14
  equipment tags, 2 bipeds and 3 vehicles, so one map (Blood Gulch) is
  the donor for Halo's guns on other games' maps. The disc's
  multiplayer has no fuel rod gun and no rocket Warthog. The reference
  describes another Xbox build than the disc; where they differ the
  specs follow the disc.
- **M1's first map is Hang 'Em High** (one cluster, no portals, two
  lightmap pages, ten opaque materials); Blood Gulch second, for
  culling and its sky.

## 3. What is on the disc (measured 2026-11-15)

The image's file system holds 1,166 files, 3.55 GB: the executable, the
map cache files, videos and demo content. The map folder has 24 cache
files: 13 multiplayer maps of 19-24 MB each (`beavercreek`,
`bloodgulch`, `boardingaction`, `carousel`, `chillout`, `damnation`,
`hangemhigh`, `longest`, `prisoner`, `putput`, `ratrace`, `sidewinder`,
`wizard`), the menu's `ui` (13.5 MB) and ten campaign maps (73-178 MB
each). Each multiplayer map is self-contained, so a per-map asset set is
one record of about 20 MB before our compression. The formats' details
are the specs' job (M0).

## 4. Milestones

Each milestone: specs first (from the reference and the disc's data),
then implementation from the specs, tests, screenshots, merge, push, and
its summary here.

### M0: groundwork (specs only)

Spec waves, each file in `specs/halo_ce/` (index and status in its
README). Wave 0 fixes the conventions every later spec leans on.

| Wave | File | Covers |
|---|---|---|
| 0 | `disc_image.md` | the Xbox disc image's file system: volume descriptor, directory tree, sectors, where a game partition starts in each kind of dump |
| 0 | `cache_file.md` | the map cache file: header, compression, the tag index, tag paths, how tag data, blocks, data references and tag references are stored and addressed; the tag classes and which ones a multiplayer match needs; the disc's maps; world units, axes and the tick |
| 1 | `bitmaps.md` | bitmap tags: types, pixel formats (compressed, palettized, swizzled), mipmaps, cube maps, sprites and sequences |
| 1 | `structure_bsp.md` | the level's structure: render geometry, materials and lightmaps, clusters, portals and visibility, fog, weather, the sky's hookup, detail objects, sound environments |
| 1 | `collision.md` | collision BSPs (the level's and a model's): nodes, planes, leaves, 2D references, surfaces, edges, vertices, materials; how a trace walks them |
| 1 | `models.md` | render models: nodes, regions and permutations, geometry parts, compressed vertices, levels of detail, markers, shaders |
| 1 | `shaders.md` | the shader classes and what each draws (environment, model, transparent kinds, glass, metal, plasma, water), enough for the classic look |
| 1 | `scenario.md` | the scenario: player spawns by game type, netgame flags and equipment, object palettes and placements (scenery, weapons, vehicles, machines), the sky, starting equipment |
| 1 | `sounds.md` | sound tags: classes, pitch ranges, permutations, the Xbox's compressed format, looping sounds, distance and falloff |
| 1 | `animations.md` | animation graphs: node animations and their compressed forms, the unit, weapon and first-person tables, frame events |
| 2 | `objects.md` | object, unit and biped tags as data: what a player's body is (bounds, camera, seats, attachments), items and equipment, scenery |
| 2 | `movement.md` | the player's movement: speeds, acceleration, gravity, jump, crouch, air control, steps and slopes, falling, the per-tick order |
| 2 | `health_shields.md` | body and shield vitality, recharge delay and rate, overshield and camouflage, damage by material and region (headshots), falling and melee damage, death |
| 2 | `weapons.md` | weapons: triggers, rates of fire, error and spread, magazines and reload, heat and overheating, charging, zoom, readying, melee, with every multiplayer weapon's numbers from the disc |
| 2 | `projectiles.md` | projectiles and damage effects: speed, arc, range, guiding, sticking, bouncing, detonation, radius damage and falloff; grenades |
| 2 | `multiplayer_rules.md` | spawning and respawn times, pickups and their respawn, powerups, the game types' rules that matter to a gun game |
| 3 | `effects.md` | effects, particles, lights, decals, contrails: what a muzzle flash, impact and explosion are made of (M2-M7) |
| 3 | `hud.md` | the HUD's pieces: shield and health meters, ammo, crosshairs, the motion tracker, pickup and damage indicators (M3) |
| 3 | `vehicles.md` | vehicles: seats, physics, turrets (M6) |

Waves 0 and 1 are what M1 needs; wave 2 what M2 and M3 need; wave 3
comes as its milestone nears.

### M1: a map you can walk on

- `blam_formats`: the disc image as a mount layer, the cache file and
  tag index, bitmaps, the structure BSP, collision, the scenario's
  spawns, the sky. Fixtures are written by its tests; `heavy::` tests
  read the disc image and skip without it.
- `games/halo_ce` skeleton: `INFO`, the mount through `mount::served`,
  `SimPlugins`, the harness, a `MapLoader` returning `MapData` in metres
  (world and kit apart, lean option honoured), a `MapDressing`.
- One map (Blood Gulch or Hang 'Em High) with textures, lightmaps, the
  sky and collision, walked as an existing UGG character. Offscreen
  screenshots in the milestone's summary.

### M1's summary (2026-11-15)

- **What landed.** `blam_formats` (`blam/formats`, plain data, 333 tests,
  120 of them on the disc): the Xbox disc image as a mount layer, the
  map cache file (inflated whole; tag index, address spaces, typed tag
  reads), and readers for bitmaps, the structure BSP, collision BSPs,
  the scenario and sky, shaders, models, sounds, animations, objects,
  weapons, projectiles, damage effects, a collision model's vitality,
  the globals and the saved game type. `games/halo_ce`: the map loader
  (`game::maps`: meshes per shader with their textures, block-compressed
  data handed over untouched, the lightmap pages packed into one atlas,
  collision with surface names, slayer and team spawns, fog, the sky
  model as a 3D skybox, scenery as props with collision), through the
  converted-map cache. `apps/halo_ce` and `apps/halo_ce_server`. In the
  party apps (`party_kit`): Halo's bundle, its surfaces as impact
  classes, team and free-for-all spawns, six maps in the pool
  (data/map_pool.toml says why the other seven wait: teleporters, and
  Sidewinder's vehicles), bots on the navigation generated from
  collision.
- **Measured.** A map converts in 0.12-0.16 s and loads from the cache
  in 0.02 s. Blood Gulch: 5,503 triangles, 22 textures, 46 scenery
  props; Hang 'Em High: 3,218 triangles, 12 textures.
- **Screenshots** (taken with `halo_ce --map halo_ce:<map> --screenshot`
  and the Ultimate Gun Game on a Halo map; kept outside the repository
  and posted to the playtest channel): Blood Gulch's red base under the
  sky with trees and boulders; Hang 'Em High; a Counter-Strike rung
  standing on Blood Gulch in the Ultimate Gun Game.
- **Open after M1** (each in the loader's "Known shortcuts" or the
  backlog): no bump shading, self-illumination, specular or reflections;
  one detail map, in linear light; animated textures frozen; one fog for
  a map; no visibility culling; breakable surfaces solid; ladders plain
  triangles; scenery lit by one probe; the sky's ring and sun glow to be
  checked against the real game (M5); teleporters and lifts (M4);
  Blood Gulch is in the pool but its measured walk-to-meet time keeps
  it out of the automatic draw by player count (the pool's rule, the
  other session's).

### M2: Halo guns as UGG rungs (the vertical slice)

- Assault rifle, pistol, plasma rifle, shotgun as data-driven weapons
  (the `add-weapon-from-data` skill): first-person models and
  animations, sounds, muzzle effects, hitscan and projectiles, spread,
  heat and overcharge by the specs.
- A Halo rung makes you a Halo player: the Spartan's body with basic
  animations, Halo's movement, shields over health.
- UGG runs a match whose ladder mixes Halo rungs with the other games'.
- The game in `chimera_pack`; Halo's asset sets published (section 2).

### M2's summary (2026-11-15)

- **What landed** (all in `games/halo_ce` unless said):
  - `game::movement`: Halo's movement from movement.md, its numbers in
    data/movement.toml; exact at 30 Hz, scaled under another tick.
  - `game::vitality` and `game::damage`: body and shield, recharge,
    overshield, camouflage, stun; the seam `HaloHit` every Halo damage
    source delivers through. The shield is the engine's `Armor` with its
    own ratio (`weapon::ArmorRatio`, new in `chimera_sim`), so another
    game's bullet takes the shield first.
  - `game::weapons`: the four guns as data (data/weapons/*.toml, checked
    against the disc by a heavy test) run by Halo's own per-tick rules
    beside the engine's frame (`weapon::OwnFrame`, new in `chimera_sim`);
    per-tick projectiles; melee; lent as rungs.
  - `game::body`: the Spartan with third-person animation from the
    graph's unit tables, hit boxes with the head tagged, armour colours.
  - `game::view`: the guns in hand with the arms, played one frame a
    tick; the held model on the Spartan's hand.
  - `game::fx`: sounds and effects from the donor map's effect tags;
    every shot shown on every machine (net part `halo_ce_shots`).
  - `game::ticks`: Halo's 30 Hz ticks counted exactly from the engine's
    tick index at any tick rate.
  - `blam_formats`: readers for effects, particles, lights, decals and
    material effects, and for the HUD tags.
  - The asset store: `chimera_pack pack|sets halo_ce`, `chimera_assets
    sign`; Halo's `base` (the donor map) fetched with a match that uses
    Halo, not at start-up. Published 2026-11-15: 30 records, 2.4 GB,
    the manifest signed on the server; verified by fetching `base` and a
    map set through the service.
  - The network version is `net56` (Halo's replicated state and shots).
- **What the integration pass found** (each fixed, each with a test):
  vitality and stun stepped once per engine tick instead of once per
  Halo tick, so the shield delay ran 2.2 times fast in the party apps;
  Combat Arms' armour step removed Halo's shield pieces, so other
  games' rounds passed the shield (`weapon::PiecesOf`, and one condition
  in Combat Arms' `armour_follows_identity`); no Halo gun in hand on
  Halo's own maps in the party (`BUNDLE.kept`).
- **Tests.** `halo_ce`: 217 with the disc. `blam_formats`: 353. The
  whole workspace's fast tier on Windows: 6,041 passed (2026-11-15).
- **Screenshots** (outside the repository; posted to the playtest
  channel): the assault rifle in hand with Halo's HUD on Hang 'Em High;
  a Halo rung firing in an Ultimate Gun Game match; the Spartan.
- **Open after M2**: the parity list from the specs' second correction
  (the field of view's real angles, the blocked steps, the shot
  direction table: being done); no supported way to start a match on a
  chosen rung (a dev command would help screenshots and tests); the
  Spartan's left-hand IK; the rifle's ammunition counter on the gun is a
  static texture; death animations; the full-body shield shader.

### M3: the full multiplayer arsenal

Sniper rifle, rocket launcher, needler, plasma pistol, fuel rod gun,
flamethrower if the disc has it; frag and plasma grenades; melee;
pickups; the HUD pieces a Halo rung shows, on UGG's HUD hooks.

### M4: maps

Every multiplayer map in the UGG pool with spawns and bot navigation,
map objects (teleporters, lifts), and per-map tests that each pool map
loads with spawns.

### M5: feel and parity

Measured parity where the user can capture the real game: movement
curves, weapon timings, damage, shield timings, sound falloff; a probe
or capture tool if one is needed.

### M6: vehicles (only if the earlier milestones are solid)

Warthog, Ghost, Banshee and Scorpion as a UGG mode or rung variant.

### M7 and on

Effects and decals, ragdolls and death animations, the classic look
throughout, Custom Edition maps if the user wants them, and this plan's
backlog.

## 5. How the session works

- **Agents.** Spec agents (the reference and the disc's data; write only
  `specs/halo_ce/`) and implementation agents (specs and this repository
  only), each with a self-contained brief. Models by task: the strongest
  for the orchestrator's architecture and reviews, a strong one for
  specs, seams and hard debugging, lighter ones for routine
  implementation and mechanical work. The number of agents building at
  once starts at two and follows the PC's measured memory and CPU.
- **Branches.** One branch per step, merged into `main` when its tests
  pass, pushed, then the affected tests after the push
  (`scripts\affected-tests.ps1`); a failure on `main` is fixed at once.
- **Recovery.** The PC locks at night and may stop processes: long
  commands run in the background with logs, work is committed in small
  steps, and after an interruption the session checks what stopped and
  runs it again.

## 6. Open questions

- ~~How the asset store's build id is named for a game without a Steam
  build~~ Decided (M2, the game in `chimera_pack`): the build string of
  the disc's map headers (`01.10.12.2276`; another region's disc has
  another), read from 2 KB of one map (`game::mount::build`); the store
  serves Halo instead of the disc, never under it (a player has the
  whole disc or none of it). What is packed and the sets:
  [assets-for-developers.md](../../assets-for-developers.md), "Halo".
- The movement, weapon and vitality specs' numbers come from the disc
  and their rules from the reference; nothing is measured in the real
  game yet (M5).
- Halo's own app and server (`apps/halo_ce`, `apps/halo_ce_server`):
  after the party apps play it.

## Backlog

- **Bots up Halo's ladders**: the bots' generated mesh has walks and
  teleporters (`Via::Teleport`, `nav_gen::link_map`) but no ladders: the
  generator makes no `NavLadder` from the map's ladder volumes
  (`core::MapLadders`), and Combat Arms' path search and follower take
  walks and teleporters only. Needed: a ladder link from each climbable
  sheet's foot to the floor over its top (as HL2:DM's `to_floors` finds
  it), and the follower facing the sheet and pressing on while Combat
  Arms' movement climbs the made ladder (docs/DESIGN.md, "Ladders across
  games"). No pool map's spawns need one (every map's spawns reach each
  other on walks and teleporters: `heavy::halo_maps`).
- **Sidewinder kept out of the pool** (games/halo_ce/data/map_pool.toml):
  a vehicle map too large on foot; in with vehicles (M6).
- **Teleporters' extras**: the blocked message, the "teleporter activate"
  sound for the teleported player, the pads' shimmer (Blood Gulch's
  `teleporter_loop` sound scenery plays as any sound scenery would, when
  sound scenery is placed); the edge neighbour's hold on a ladder
  (movement.md). Spec questions: whether a player can mount a ladder
  from the floor (the ground test keeps the floor, the higher normal);
  movement.md says only Boarding Action has climbable surfaces, the disc
  has them on nine maps (collision.md's 120 surfaces).
- **Blood Gulch in a draw**: it is in the pool (allowed), but the pool's
  count rule keeps it out of every draw by players present (its wander
  time, 21 s at 8 players, over the 10 s target: a vehicle map walked).
  It plays when named (`map halo_ce:bloodgulch`, the host's vote list);
  a draw needs either a smaller measured area (the flood bounded, above)
  or a per-map exception in the pool's rules (the user's call).
- **Scale for other games' players**: the levels are built for a 2.1 m
  Spartan running 6.9 m/s (movement.md); not rescaled. A Counter-Strike
  player (1.83 m, 6.35 m/s) or a Combat Arms soldier (about 1.8 m, a
  similar run) is a little smaller and slower than the maps expect:
  doors and corridors are roomy, the open maps (Blood Gulch) feel long
  on foot, and Halo's jumps (its higher, floatier jump) are not all
  made by the other games' jump; steps and ramps are fine.

## Log

- 2026-11-15: plan written; the reference cloned and the disc image
  extracted outside the repository; the Windows checkout's line endings
  and missing crate fixed so the hooks pass; wave 0 and the first of
  wave 1 started.
- 2026-11-15: ten specs landed (each checked against the disc by its
  writer and read by the orchestrator); `blam_formats` (the disc image
  reader, then the cache file and tag-reading foundation: 76 tests, 27
  of them on the disc's 24 maps) and the `halo_ce` skeleton merged.
  Measured on the PC: a first Bevy build in a new worktree takes 12
  minutes at six jobs, a plain-data crate seconds, so format readers
  run in parallel worktrees and game-crate work reuses one warm
  worktree.
- 2026-11-15: Halo's maps in the party apps (M1's last step): its
  bundle among `party_kit::GUESTS` (loader, client, its map pool), its
  spawns the slayer starts plus capture the flag's red and blue bases as
  Combat Arms' Alpha and Bravo, its surface names (the engine's classes)
  through the Source-to-Combat Arms table and registered as classes for
  other games' impacts, its pool the disc's maps its
  data/map_pool.toml allows (beavercreek, bloodgulch, damnation,
  hangemhigh, longest, prisoner), measured by `map_fitness`
  (data/map_fitness.toml). No map is claimed: without the disc image
  nothing changes. Bots use the generated mesh (backlog). A Combat Arms
  soldier walks and shoots on Hang 'Em High (party_kit's
  `heavy::halo_maps`).
- 2026-11-15: M0 and M1 done (their summaries above). M2 under way:
  Halo's movement and vitality with the damage seam landed and are wired
  to each other (falls through the shield, stun into movement); the
  first four weapons' simulation, the Spartan's body and Halo in the
  asset store are being written. A Windows-only failure of the session
  log's tests on `main` (an append-only handle cannot be truncated; a
  locked marker read through a second handle) was fixed in passing, as
  it blocked the commit hook.
- 2026-11-15: M2 done (its summary above); Halo's sets published to
  the asset service. M3's simulation and Halo's HUD landed; the
  vehicles spec (M6) is being written. The specs were corrected a
  second time from the implementations' findings.
- 2026-11-15: M4 (but the bots' bound): `chimera_world::map::teleport`
  (pads in a map's typed extras, acted on by one system for every
  character with its movement's footing: radius, re-arm, velocity kept
  or zeroed, the look turned, blocking and telefrags), Halo's teleporter
  pairs as its pads (`game::maps::teleporters`), climbable sheets as
  ladder brushes every movement climbs (`game::maps::collision`), Halo's
  movement climbing by the aim (movement.md); `map_fitness` counts
  teleporters as links; putput, boardingaction, carousel, chillout,
  ratrace and wizard joined the pool. Heavy tests: each map's teleporter
  count, a Halo player on each of the 61 pads arriving on the ground at
  its target, CS:S and Combat Arms characters through Chill Out's and
  Wizard's, boardingaction's 40 ladder sheets, spawns for both teams on
  all 13 maps, every pool map's spawns reachable with teleporter links.
- 2026-11-15: M4 done: the bots on Halo's maps. Measured first: the
  bots' walker (0.8 m rises, 45 degree floors) never left Blood Gulch's
  drawn level (its flood unlimited and held to the drawn box are the
  same 78,000 m2); it stopped at the flood's 300,000-cell limit (75,000
  m2), which left 972 spawn spots off its region. So Halo's maps say
  where they are played, the structure BSP's world bounds
  (`maps::collision::nav_within`, the engine's `nav_gen::NavWithin`, a
  map extra): a generated mesh keeps cells inside that box only (a guard
  against the material-less walls and lid) and may grow to its whole
  footprint (`NavGenParams::cell_limit`). Every held area's chest is in
  a leaf with a cluster by the collision BSP's point test. Teleporters
  are links of the bots' mesh (`nav::Via::Teleport`,
  `nav_gen::link_map`): Combat Arms' bots plan through them and walk
  onto the pad (a bot on putput left its room through one in 10 s);
  the flood is seeded at their exits too. `map_fitness`: Blood Gulch
  77,996 m2, no spot off, wander 52.7/22.6/14.4 s; the other maps
  unchanged. Ladders for bots stay open (backlog).
