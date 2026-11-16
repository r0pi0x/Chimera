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
| M3 the full multiplayer arsenal | done (2026-11-15) | the sniper rifle, rocket launcher, needler, plasma pistol and flamethrower beside M2's four, frag and plasma grenades, explosions, the level's materials at each hit, pickups on the item clock; eight rungs (every held gun but the flamethrower); each gun in hand with its states, sounds and effects, each projectile with its own look, grenades and items seen on every machine; Halo's HUD from the disc's tags; summary below |
| M4 every multiplayer map | done (2026-11-15) | teleporters for every game's characters (`chimera_world::map::teleport`, from the scenario's pairs), climbable surfaces as ladders (Halo's movement climbs them by movement.md, the others by their own rules), no machine play depends on (Wizard's 4 alert lights), twelve of thirteen maps in the pool (Sidewinder waits for vehicles), the pool measured with teleporters as links; the bots' generated mesh held to the drawn level (`nav_gen::NavWithin`, the structure BSP's world bounds) and grown to it (Blood Gulch's 78,000 m2 whole), teleporters as navigation links the bots plan through and walk onto (`Via::Teleport`); ladders for bots open (backlog) |
| M5 feel and parity pass | not started | |
| M6 vehicles | in progress (2026-11-15) | the Warthog, the Ghost and the Scorpion on the disc's maps in Halo's app, driven by vehicles.md's mass-point step (its simulation tables matched), seats, the seat camera, the wheels and suspension shown, splatter and riders' damage; being done: seats on every machine and vehicles in the party apps; next: the guns and turret, the riders' seated poses |
| M7 and on (effects, ragdolls, the classic look, the backlog) | in progress (2026-11-15) | Halo's own game types in its app and server (Slayer, Capture the Flag, King of the Hill, Oddball; Race waits for its vehicles); being done: the Spartan's death, the explosions' particle states and the scopes by the corrected specs |

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

### M3's summary (2026-11-15)

- **What landed** (all in `games/halo_ce` unless said):
  - `game::weapons`: the nine held guns and the two grenades as data
    (data/weapons/*.toml, data/grenades.toml, checked against the disc),
    with charging (the plasma pistol's overcharge), heat, zoom levels,
    guided needles, rockets, the flame; per-tick projectiles with the
    level's material at each hit (bounce, stick, detonate), explosions
    as area damage through `damage::HaloHit`, grenades as a count thrown
    beside the gun in hand, pickups on the item clock.
  - Eight rungs lent to the party apps (`game::parts`): every held gun
    but the flamethrower (its flame ends 2.5 WU out, and no stock map
    places it).
  - `ui::hud`: Halo's HUD from the disc's interface tags: shield and
    health meters, ammunition and heat, each gun's crosshair, grenade
    counts, the motion tracker.
  - `game::view` and `game::fx`: each gun in hand with its states (the
    sniper's zoom, the launcher's reload, the overcharge), each
    projectile's own look (guided shots steered to the server's target),
    particle systems (`blam_formats::effect::system`), detonations,
    plasma countdowns and pickups sent to every client (net part
    `halo_ce_objects`); thrown grenades and items are the server's
    entities, drawn through the engine's `NetItem`.
  - The specs' second and third corrections applied: the field of view's
    real angles (70 nominal is 48.11 degrees vertical), the mover's
    blocked steps, the spread's direction table.
  - The network version is `net58` (Halo's shots carry their trigger and
    target; its objects are sent to every client).
  - Shared code: the held model's scale by game
    (`chimera/world/src/map/loose.rs`), a Halo tab on the party's
    Controls page and a grenade bind.
- **What the merge found**: with nine Halo guns and TF2's in one ladder,
  Ultimate Gun Game's bound on tables made over a session of matches
  failed (104 new archetypes between the fourth and eighth match, the
  bound 100). Halo's parts now go on and off a character in one move
  (movement, shield and vitality together; the kick kept from spawn with
  a flag): 70. The bound was not raised. Most of the rest is other
  games' parts and the physics engine's.
- **Tests.** `halo_ce`: 246 with the disc. `blam_formats`: 363.
- **Screenshots** (outside the repository; posted to the playtest
  channel): the rocket launcher in hand with Halo's HUD, a frag
  grenade's burst, needles in flight, the sniper's zoom levels.
- **Open after M3**: switching the grenade type, the scopes' masks, some
  sprites drawn soft-edged, the rocket's trail, respawning in the
  offline app; the corrected specs' second list (ladders mounted from
  the floor, the grenades' real material tables, guidance, autoaim on by
  default, teleporter exits: done, the log's last entries).

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

Stage 1 (2026-11-15), `game::vehicles`: the scenario's Warthogs on Blood
Gulch in Halo's app. The physics is vehicles.md's own mass-point step at
30 Hz (springs and friction per tick, not the physics engine's solver)
on the engine's spatial queries, writing a kinematic body that
replicates as a moving map prop, so no network message changed. Against
the spec's simulation table: speeds, distances, stopping, steering, ride
height, tyre depths and settling match to the stated digits; the 1 WU
drop's lowest point is -0.1215 against the spec's -0.131 (asked of the
spec). Seats by the spec's search (driver and passenger; the rider's
movement suspended), the driver's camera from the seat's track, a moving
Warthog's splatter through `HaloHit`, 18% of hull hits to riders. On
this disc the Banshee is campaign only: multiplayer's vehicles are the
Warthog, the Ghost and the Scorpion.

Stage 2 (2026-11-15): the Warthog by the corrected spec (the tick's
order, the freeze, placement at the centre of mass by the vehicle set,
the drop case's corrected numbers), the seat camera from the seat's
track with its pull-in (`vehicles::camera`), the wheels, steering and
suspension posed on the rigged prop (`vehicles::look`; a client
estimates them from the replicated motion), the Ghost (hover) and the
Scorpion (treads) as data, their simulation tables matched (the Ghost's
peak yaw rate is 4.21 degrees a tick against the spec's 3.9, which its
own angles contradict: asked of the spec). Heavy tests: placements by
kind on all 13 maps; each kind resting and driven on Blood Gulch,
Sidewinder and Hang 'Em High.

Still to do: the seated state on every machine and vehicles in the
party apps (being done), the guns and the gunner's turret, the riders'
seated poses and the entering and leaving animations, Sidewinder into
the pool, Race.

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
- **An arena cut from a big map** (the user's idea, 2026-11-15; a
  party-app feature for any game's big map, not only Halo's): Blood
  Gulch (78,000 m2) and Sidewinder (69,500 m2) are six to forty times
  the other eleven maps (1,700 to 11,400 m2), so a gun game never draws
  them. Instead of leaving them out, play a slice of one, sized to the
  players present (the pool's own target: the wander time under 10 s).
  Parts: (1) candidates: closed boundaries in plan view over the bots'
  generated mesh, cut where the mesh is narrow (a boundary across a
  pass needs less wall than one across a field), each scored by the
  `map_fitness` measures recomputed inside it (area, mean sight, wander
  time) plus a new one, the share of spawn-spot pairs that see each
  other, so a flat open slice where players spawn in each other's view
  is refused; (2) a curated list per map chosen from the scored
  candidates (the algorithm proposes, a person picks), kept as data
  beside the pool rules; (3) play: the boundary solid to players, the
  bots' mesh and the spawn spots held inside it (`nav_gen::NavWithin`
  is the box-shaped start of this), items and objectives outside it
  left out; (4) the look, in the party apps' own style (the party UI's
  owner draws it): a line along the ground and a faint curtain that
  read from afar, the wall glowing near a player who approaches it,
  the world beyond it dimmed. Until then the simple rule holds: the
  pool leaves the big maps out of every draw.
- **Vehicles in the party apps are off** (the user, 2026-11-15): not
  wanted in the gun game for now; a switch in Halo's party bundle,
  default off, places no vehicle on a Halo map there. Halo's own app
  and server have them.
- **Grenades over time on the grenade rung** (the user, 2026-11-15;
  high): a thrown grenade comes back at once under the ladder's
  `refill_ammo` (Halo's rung keeps 4), so it can be spammed. Wanted: one
  grenade every N seconds up to the maximum (N a data value), as a rule
  of the grenade band so every game's grenade rung paces the same.
- **The envelope for the other games' characters**: only Halo's
  movement takes the map's movement envelope; a CS:S character on
  Combat Arms' warehouse reaches 92% of what the soldier does. Source's
  box sizes are one configuration per movement, not per character:
  mover work.
- **Special rungs**: Combat Arms' turret, Halo's grenade rung, a TF2
  engineer's sentry later; the grenade rung is weapon data
  (`weapons/rung.rs`, an opt-in `rung_hint`), not a rung extra. When
  the third arrives, see what the ladder's rung extras lack.
- **The user's decisions for the party apps** (2026-11-15):
  - Order of work: first what Halo needs to play well in Ultimate Gun
    Game (the HUD layout, scopes, audio, kill feed icons, the balance
    follow-ups), then the arena cut from a big map as its own piece;
    until then the two big maps stay out of draws and the eleven others
    play.
  - Field of view (a lean, not final): the user's setting for every
    game's gun; Halo changes it only while zoomed, by the spec's
    magnification (done: `ui::party_view`; the gun in hand keeps Halo's
    own view-model angle).
  - Autoaim's bend off for mouse and keyboard (done: `halo_autoaim`);
    guided projectiles keep their target.
  - No Halo grenades in the party apps by default (done:
    `halo_party_grenades`, the counter hidden).
  - A grenade rung (done: `halo_ce:frag_grenade`, the pistol with an
    empty magazine, left click a dry fire, the grenade button throws,
    frags back one every 5 s up to 4, an opt-in key hint at the bottom
    centre).
  - Gravity: left alone (the user, after the corrected numbers): every
    character keeps its own gravity and jump on every map. Gravity
    9.78 m/s2 (Halo), 20.3 (CS:S, TF2), 19.8 (Combat Arms), 15.2
    (HL2:DM); standing jumps 1.99 m, 1.45, 1.33, 0.89 and 0.54 m. A
    map-owned gravity would have a CS:S character jump 3.0 m on a Halo
    map; to revisit only if play shows people reaching where they
    should not.
  - Kill feed icons from the disc's HUD (done: `ui::kill_icons`).
- **Balance in the party apps** (measured from code and data,
  2026-11-15; nothing changed yet, the user decides): a Halo-rung holder
  has 200 effective hit points against other games' guns (the shield is
  100 armour that absorbs all of a hit, then 100 of body) and the shield
  refills in 10 s; Halo's guns do about twice to other games' characters
  what they do to a Spartan (one Halo point is 1.33 hit points, times
  the Spartan's flesh modifier: the pistol kills in 2 body shots or any
  headshot, the sniper rifle in one body shot); explosions and melee
  kill other characters in one hit (a frag within 7.6 m); the shield is
  kept as armour on the next rung (a bug: to fix); the ladder's power
  reader scores against a plain 100-point target and misreads the plasma
  pistol, the needler and the assault rifle (to fix). Proposed: one rule
  (a character's full vitality in its own game is 100 in the party
  apps), behind a switch.
- **Play as the campaign's characters** (the user's idea; built
  2026-11-15/16: Elite, Grunt, Jackal and Marine on main, see the log;
  the Hunter and the spoken lines under way). On a Halo rung in the party apps the body
  is sometimes not the Spartan but one of the disc's campaign
  characters (Elite, Grunt, Jackal, Hunter, Marine, perhaps the Flood),
  by a chance weighted toward who carries that gun in the campaign (the
  needler: Grunt, Jackal or Elite; the plasma rifle: Elite; the assault
  rifle: Marine). What is known: the multiplayer maps hold only the
  Spartan; the campaign's bipeds, their animation graphs and dialogue
  are in the campaign maps, which are published in the asset store with
  the rest of the disc but are large, so which map is the donor and what
  a player downloads for it is the first cost (a generated set of just
  those bipeds would be the lean way, if the asset service takes
  generated sets). The user's decisions (2026-11-16):
  - *Which guns each can hold*: faithful to the game: a character is
    offered only for the guns it carries there (its animation graph's
    weapon labels and the campaign's own pairings decide; the Hunter's
    gun is part of its arm).
  - *Movement*: as the character moves in the game, as far as the disc
    states it (a Grunt slow and wobbly): each character's own speeds,
    turning and body from its biped document, not the Spartan's.
  - *Hit boxes*: each character's own.
  - *Shields and health*: whatever the character has in the game (an
    Elite's shield, a Grunt's none). Unfair at first; tuned later
    through the party's balance tables.
  - *First person*: the Spartan's hands for now; later first-person
    hands made for each character (our own work: the disc has none).
  - *Voice*: the character's chatter plays and every player hears it.
  - *Downloads*: split so each character is fetched as its own unit
    when it is needed, never a whole campaign map (generated sets: the
    asset service's side).
  Still to settle: deaths, flinches and seats per character (each has
  its own animations); where it is chosen (a rung extra of Halo's, as
  the ladder's rung characters are, replicated with the match, off by
  default until played). The spec is written
  (specs/halo_ce/characters.md, 2026-11-15). What it found that bears
  on the decisions above:
  - *Movement*: only the Spartan uses the player's physics; every
    other character moves by its animations' own motion (four
    directions, no air control, turn speed limited). Playing as one
    needs that mover built; it is the larger part of the work.
    **Decided otherwise (the owner, 2026-11-15)**: player expression
    comes first. A character a player wears keeps the player's mover
    (the Spartan's acceleration, air control and jump, every direction,
    no freeze after a landing), and its aim is never speed-limited.
    Only the speed differs: halfway between the Spartan's and the
    character's own (one blend factor in data, to tune). The body's
    animations bridge the gap: the character's own move animations at
    a rate matched to its ground speed, the nearest one where a
    direction has none. The animation-driven mover is not built; this
    replaces the *Movement* decision above.
  - *Voice*: a body a player controls speaks only its pain, death,
    scream and maimed lines; all other chatter is the AI's. Faithful
    means those four kinds only (heard by everyone); anything more is
    our own addition.
  - *Weapons with no first-person model*: the Grunt's fuel rod gun,
    the Hunter's cannon and the energy sword have none, and no
    first-person animation to time their ready, reload or melee. They
    need a rule of ours (borrowed timings, no model in the hand).
  - *Sizes*: Elite 6.7 MB, Grunt 12.2 MB, Jackal 8.5 MB, Hunter 4.6 MB
    plus its 1.0 MB cannon, Marine 59.6 MB (57 MB of it six voices),
    Flood combat forms 3.1 and 2.8 MB. Voice is most of every unit:
    voices become their own units, fetched after the body.
  - *Order*: the Elite first (shield and health 100/100; holds the
    plasma rifle and needler, which the multiplayer set already has),
    then the Grunt and Jackal, the Hunter last (no first-person view
    at all).
  Stage one (branch `halo-char-elite`): the Elite. Its body and voice are
  units of their own (`chimera_pack sets halo_ce --only character/`,
  converter `halo_ce:character_unit`: small map files cut from d20,
  `game::campaign::unit`), loaded on demand; a player's `BodyKind`
  (replicated) gives its model, graph, hit boxes, deaths, vitality (100 /
  100), pill and camera. Movement (the owner's rule, 2026-11-16: player
  expression over fidelity): every body moves on the Spartan's mover, only
  its speed differs, halfway between the Spartan's and its own (a blend
  in data/campaign.toml: the Elite at 107.5 %), with any movement number
  overridable there; its legs play at the ground speed. The host setting
  `halo_ce:campaign_bodies` (off by default) draws it on the plasma
  rifle and needler rungs by data/campaign.toml; `halo_ce:body` forces
  one. Next: the voice, the Elite's hand marker for the held gun.
- **Sidewinder kept out of the pool** (games/halo_ce/data/map_pool.toml):
  a vehicle map too large on foot; in with vehicles (M6).
- **Teleporters' extras**: the blocked message is counted by the
  spec's rule (multiplayer_rules.md 6.8) but no HUD text shows it; the
  "teleporter activate" sound for the teleported player; the pads'
  shimmer (Blood Gulch's `teleporter_loop` sound scenery plays as any
  sound scenery would, when sound scenery is placed); the exit's crouch
  test knows crouched or not, not the crouch fraction.
- **Autoaim's shortcuts**: the aim point is the character's centre;
  magnetism (the stick's slowdown) is not implemented; where the
  convergence point goes on a miss is not in the spec (the end of the
  128 WU ray is used).
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
- 2026-11-15: M3 done (its summary above): the five guns in hand, the
  projectiles' looks, grenades and items on every machine (`net58`),
  after Halo's parts were made to go on and off a character in one move
  (Ultimate Gun Game's bound on tables over a session). M6 stage 1: the
  Warthog (its section above). Being done: the corrected specs' second
  list, and the vehicles spec's gaps for stage 2.
- 2026-11-15: the corrected specs' second list: ladders mounted from
  the floor (a ladder beats the floor in the ground test), the aim's
  climb wish and the edge neighbour's hold, all four of Prisoner's
  ladders climbed in a heavy test; the grenades' and the needle's
  material tables from the disc (no stand-ins), the speed clamp only
  on a slowed segment; guidance by the corrected rules; autoaim on by
  default (`weapons::aim`; guided projectiles take its target);
  teleporter exits by the arriving pill (`teleport::Exit::Pill`, the
  engine's default unchanged); the tracker's quantised examples as
  tests. Spec questions left: the needle's heading at update 60 (57.0
  stated, 55.5 from its own positions), the tracker's (0.1, 0) blip.
  The vehicles spec was corrected from the Warthog (the drop case's
  number was the spec's mistake).
- 2026-11-15: the specs corrected a fourth time (the scopes' masks dim
  and blur, nothing is blacked out; particle systems' states; grenade
  selection; the autoaim's aim point). Landed: the grenade kind's
  switch (`net59`) and Halo's app's respawn; vehicles stage 2 (the
  Ghost, the Scorpion, the seat camera, the model's overlays); Halo's
  game types (`game::rules`, `net60`): Slayer, Capture the Flag, King
  of the Hill and Oddball on the engine's mode runtime, from the disc's
  game type records, a match played to its end by real fire on Hang
  'Em High; the party apps keep their own rules. Held back and being
  redone by the corrected specs: the scope's mask (drawn as first
  inferred it blacked out the zoomed view) and the particle systems'
  states (the frag's smoke drew as one grey sheet).
- 2026-11-15: the Spartan's death (`body::death`, `net61`): a body of
  its own on every machine playing the death the killing hit picks
  (direction and body part; the airborne death, landing), lying where it
  fell for 900 idle ticks, its sound at the animation's sound frame; in
  Halo's own app the grenades and the weapon in hand drop as items (a
  dropped item stays in the air where it was dropped: items do not fall
  yet). Not done: flinches on a hit (non-lethal hits do not reach
  clients), limp bodies, the unit's dialogue. Asked for by the user:
  Halo's HUD in the party apps with the shield and health at the bottom
  left and no motion tracker (the shared kill feed is top right): being
  done.
- 2026-11-15: explosions and trails by the corrected effects spec (the
  earlier grey sheet was the spec's sizes read wrongly, not a sprite
  bug: a frag is now a fireball and drifting smoke), the scopes' masks
  dimming the view outside the window (the blur and night vision not
  drawn: the engine's screen pass would need a two-pass blur), grenade
  selection by weapons.md 14, the autoaim's aim point and its bend off
  by default, Halo's party HUD layout (the shield and health bottom
  left, no tracker: the party's kill feed is top right). The user's
  decisions for the party apps and the balance findings are in the
  backlog above.
- 2026-11-15: Halo's lasting sounds (`game::audio`: the level's
  background sound, sound scenery, projectile loops, footsteps by
  material; reverb not applied, weapon loops not played) and the
  shaders' looks (`maps::materials`: self-illumination on 16 level
  shaders, cube reflections on 57, the model's multipurpose masks,
  scrolling transparents; 334 shaders on the 13 maps). Left of the
  look: bump shading (needs the lightmap's incident direction per
  vertex), the indoor/outdoor fog switch, lens flares (a `lens` reader),
  the transparents' later maps and periodic animations. Blood Gulch's
  converted load went from 0.48 s to about 0.64 s (the cause not found;
  from the cache 0.03 to 0.04 s); the frame time did not rise.
- 2026-11-15: vehicles on every machine (`net62`): the seats
  replicate, any game's character rides (the engine's `Carried`), a
  rider's client is not predicted (its input shows after its ping plus
  about 100 ms), stale seats cleaned on death, disconnect, round
  restart and map change; a crash in a party round restart from the
  Ghost's steering animation fixed. By the user's decision vehicles are
  off in the party apps (`halo_ce:party_vehicles`, default 0: no
  vehicle is placed there); Sidewinder stays out of the pool (bots do
  not drive). The game type's vehicle set is not yet applied at a
  game's start. Testing is kept light today by the user's instruction
  (targeted tests only).
- 2026-11-15: the cross-game balance (the user's decision after the
  balance report): a Halo player's effective hit points against other
  games' guns 200 -> 133 (an AK-47 kills in 4 body hits, 3 on others,
  6 before; an AWP still needs 2: one at an incoming scale of 1.754),
  Halo against Halo bit-identical; Halo's damage to other characters
  one point a hit point (the assault rifle 10 bullets, the pistol 3
  body shots, melee no longer one hit but from behind, blasts no
  longer lethal to their edge; the sniper rifle still one body shot);
  the shield no longer left as armour on the next rung; a per-game
  damage table for the party apps with HL2:DM at 1.5 (SMG1 20 -> 14
  hits, AR2 10 -> 7, the .357 one body shot); the ladder's power
  readers corrected (the plasma pistol from the top of its band to the
  middle, the needler up, the assault rifle last of the peak band by
  its burst model: `burst_seconds` to revisit). A player without the
  disc was verified end to end (Halo's base 22.5 MB in 2 s from the
  asset service, nothing missing from the published sets). Bots fire
  the battery guns now; the engine's `ShotMade` fact for bots is being
  added.
- 2026-11-15: by the user's instruction work moves fast now and is
  validated at milestones: commits skip the hook's tests by its own
  switch, each change runs its new tests, `main` is compile-checked at
  each landing, and the whole fast tier with Halo's and the party's
  heavy tests runs once Halo is fully in Ultimate Gun Game. Landed this
  way: Halo in the party apps (the HUD clear of the chat and feed at
  4:3 too, zooms on the player's own field of view after Combat Arms'
  zoom filter stopped removing other games' zooms, no grenades by
  default, kill-feed icons from the disc, crosshairs readable on bright
  maps) and the bots on Halo's guns (the engine's `ShotMade` fact and
  `FiresOnRelease`: the sniper rifle 3 -> 15 kills a minute, the plasma
  pistol 0 -> 4). Measured: a Spartan-sized walker reaches 64% of
  de_dust2 (87% with jump-ups), 56% of cs_office (78%) and 7% of
  Combat Arms' warehouse; the user made the fix a party essential: on
  any map a character's movement body no wider or taller than the
  map's own characters (hit boxes and model unchanged). Next after it:
  the grenade rung (the pistol with an empty magazine), then the
  milestone's full validation.
- 2026-11-15: the last two party essentials. The grenade rung
  (`halo_ce:frag_grenade`: the pistol with an empty magazine, left
  click a dry fire, the grenade button throws, shown as the frag
  grenade; bots press the grenade action through `weapon::AttackAction`:
  13 throws, 5 kills, no self-kill in a minute; an opt-in rung hint,
  the kit's key cap and a verb at the bottom centre, on arrival and on
  a dry fire). The map's movement envelope (`chimera_core::envelope`):
  a Spartan-sized walker's reach under the rule 91% of de_dust2 (64%
  before), 90% of cs_office (56%), and with jump-ups 100% of Combat
  Arms' warehouse (7%); in simulation the old body also completed the
  24 sampled paths, so the gain is shown by the flood and by synthetic
  corridors, steps and ceilings. The displays on the guns (the assault
  rifle's counter and compass, the plasma gauges, the needler's
  needles) are specified (weapons.md 18) and being built. With these
  Halo is in Ultimate Gun Game: the milestone's full validation runs
  next.
- 2026-11-15: the milestone's validation, Halo in Ultimate Gun Game.
  The whole fast tier of every crate, Halo's and the format readers'
  tests with the disc and the party's heavy Halo tests ran on `main`:
  six failures. Four were Halo's tests' own stale counts after the
  grenade rung (its parts are eight guns and a grenade; its view and
  held models ten), corrected; two are TF2's on `main` (a parts count
  of 4 against 12 lent, its flamethrower's power 0), the other
  session's. After the corrections Halo's suite passes whole (442 with
  the disc). The displays on the guns landed with it (the assault
  rifle's counter and compass, the plasma gauges, the flamethrower's
  bars; first person: other players' weapon state does not reach a
  client). Next by the user's order: the backlog's high items (timed
  grenades on the grenade rung), then the arena cut from a big map.
- 2026-11-16: after the milestone. Landed: the grenade rung's timed
  supply; a command-only sandbox for the party apps (`+sandbox`,
  `rung`: `chimera_sim::minigame::rung_command`); hit flinches from the
  replicated vitality and the body's corrected rules (bodies take
  shots, 900 ticks at rest, the death lines); the balance check as a
  reference page (`docs/plans/active/party-balance.md`). Built and held
  off `main` until a playtest build is cut: the sandbox's rung picker
  (`sandbox-rung-picker`: a stopgap dialog on the maps picker's
  pattern; it lists 459 guns, about 408 of them Combat Arms' variants)
  and the arena's first milestone (`arena-a1`: "Blood Gulch: West
  Base", about 9,900 m2, a solid wall every game's movement, bullets
  and projectiles stop at; its plan with the user's decisions is
  `docs/plans/active/arena-slices.md`). Being built on a branch: the
  scopes' edge blur. With the other session (the party UI, the ladder,
  the asset service and TF2 are theirs): the sandbox's design was
  handed to them at the user's request (the backlog has the options
  the user liked), on-demand asset sets for a picked gun are theirs to
  build and the rung switch that waits for one is this side's, after
  theirs lands; arena A2 touches the draw and is to be sequenced with
  their ladder-draw work; the damage tables are to become one shared
  table with a host toggle after the playtest (its default is the
  user's call). Parked by the user: vehicles' guns, seated poses and
  Race for Halo's own app; other players' on-gun counters; gravity and
  jumps (left as each game's own). Tool runs of the games on the PC are
  muted (the user's instruction).
- 2026-11-16: the campaign's characters landed (net65): Elite, Grunt,
  Jackal and Marine as party bodies (`game::campaign`, data/campaign.toml),
  each with its own model, hit boxes, shield and health, footsteps, pain,
  death and scream lines, its gun in its own hand, its unit and voices as
  generated asset sets cut from the campaign maps (not published yet),
  drawn by weights per gun with the host's `halo_ce:campaign_bodies 1`
  (off by default), and offered as the sandbox's practice targets. The
  owner's movement decision: the player's mover with per-character
  numbers in data, aim never limited. With it landed: the arena slice A1
  (net64), the scopes' edge blur, the engine's worn-first build of late
  bodies and held models (a worn body drawn in about a second, where
  the sandbox took 46 s), and the sandbox's mechanics (practice targets,
  props, blast measurement, the scene-shot tool: `--scene`), whose
  screens are the party UI's. Fixed on the way: a crash when a body's
  model changed before its death (stale skeleton poses); Combat Arms'
  map-unload cleanup dropping other games' held models; the sandbox's
  ladder read-ahead (25 to 80 ms a frame, now about 4). Under way: the
  Hunter with a first-person view made from its own body and a rung of
  its own, lines spoken on becoming a character and on a kill (the
  owner's request), the late-built models' texture and order fixes,
  more sandbox mechanics. Decided by the owner: balancing is on by
  default; publish the characters' sets (through the asset service's
  session, once the Hunter's and the widened voice sets are cut).
