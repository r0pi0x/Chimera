# Backlog

Things to build, **in priority order** (top first; reprioritized
2026-10-06). Shortcuts already in the code live in
[tech-debt.md](tech-debt.md); the MVP plan is in
[plans/active/](plans/active/engine-split.md). Move items into a
plan when work starts; delete them when done.

## Architecture pass for many games (2026-11-09)

[plans/active/architecture-pass.md](plans/active/architecture-pass.md):
what game number 30 costs and what must not grow with the number of
games. Steps 1-5 are done (neutral formats out of Source's crate, layers
by path, server commands registered by games, the dedicated server once,
the new-game guide and its check); steps 6-15 in its section 6, in
order: the other two servers on the kit, the client app once, a roster
of game descriptors for the tools and party apps, the party kit's base
game by descriptor, Chimera's remaining Source words.

## Architecture review (2026-10-20)

Address the feedback in
[plans/active/architecture-review.md](plans/active/architecture-review.md),
following its phased roadmap ("Now" N1–N7, "Next" X1–X10, "Later"
L1–L6) in that order; each item names its files (lanes), size and
guard tests, so items in one phase can go to parallel agents. Mark items
done in the review as they land. N5 (CI on GitHub Actions) is deferred (2026-10-24, the user's choice); it waits for
the user's go-ahead: no Actions runs unless asked.

- After the Bevy 0.20 upgrade (L5): move the menus from Bevy UI's
  deprecated `Interaction`/`Button` (`client::widgets::Interaction`) to
  picking's `Hovered` and `ui::Pressed` before Bevy removes them; the
  menus act on a press as the mouse goes down, so each one's input needs
  checking (docs/tech-debt.md). And `bevy_replicon_renet` back to a
  crates.io release once one supports Bevy 0.20.

## Roadmap after the gun game settles (user, 2026-10-29)

- **Parity gaps, ranked (audit 2026-11-12):** the top gaps vs CS:S,
  Combat Arms and HL2:DM for a gun-game player, in
  [plans/active/parity-gaps-ranked.md](plans/active/parity-gaps-ranked.md).
- **Order (user, 2026-10-30):** the asset service's first milestone (M1:
  per-file packer, local cache mount, recording what a load opens), then
  the ECS cleanup, then the main menu. Update (user, 2026-10-31): the
  asset service's M2 (dev server, internal accounts) before the main
  menu; the menu is for Ultimate Gun Game only (decisions in
  [plans/active/main-menu.md](plans/active/main-menu.md) section 10).
  **Lucker Party is shelved for now:** no new work on it until the user
  picks it up again. No PR test runner or CI for now;
  AGENTS.md asks contributors to test before merging (a guideline). A
  friend's test VM may come later.
- **Decisions (user, 2026-10-31):** console namespaces approved as
  designed below (build now); Combat Arms' mission timers, captions,
  messages and objective list get our own simple display with the
  install's HUD fonts, marked as ours (the client code isn't in the
  reference); **the next game is Half-Life 2: Deathmatch**, after the
  "before the next game" list (namespaces, the rest of the ECS cleanup).
- **Console namespaces (user idea, 2026-10-30).** With several games in
  one app, flat command and setting names (`sv_cheats`, `cl_hitmarker`)
  collide. Proposed:
  - Every command and setting gets a namespace like our ids:
    `cs_source:sv_cheats`, `combat_arms:musicvolume`,
    `chimera:asset_url`. The full name always works.
  - Original names stay inside each game's namespace for parity.
  - `scope <game>` makes short names resolve in that game first, then
    `chimera`. The default scope is the current game; in the party apps,
    the game of the gun in hand.
  - Configs and binds with short names keep working, and are saved with
    full names from now on. An ambiguous short name prints a hint.
  - Our general settings move to `chimera:` with no sv_/cl_ prefix.
  Do it with the ECS cleanup, before the next game; the user to confirm
  the design. **Built 2026-10-31** (with `css:`/`ca:` namespace
  aliases): [plans/active/console-namespaces.md](plans/active/console-namespaces.md).
- **First: ECS cleanup where new games plug in (2026-10-29).** Order (user, 2026-10-30): after the current gameplay work, before any new game; performance is fine today. Plan: [plans/active/ecs-cleanup.md](plans/active/ecs-cleanup.md) (steps 1-10 done; its "later" items remain). For
  extensibility more than speed. Counts today: ~350 functions taking the
  whole world (`&mut World`), ~720 queries built inside them, 78
  callback registries of `fn(&mut World, ..)`, 26 observers, ~40
  process-wide statics (architecture-review.md section 4, E2).
  1. A read-only audit classifies each one: fine as it is (map loads,
     spawning, network apply); better as observers or messages (rung,
     death and swap reactions, rules hooks); better as normal systems
     with declared queries (per-tick game logic). **Done 2026-10-29:**
     [plans/active/ecs-cleanup.md](plans/active/ecs-cleanup.md) (the
     inventory, its counts and the ordered steps that follow).
  2. Convert the seams new games use: rung and identity reactions
     (see the rung hook item), the weapon and rules hooks, and the
     statics shared between client and server. The schedule ratchets
     and the listen-server test guard each step.
  3. A `repo_checks` rule: whole-world access only on an allowed list,
     with how to fix it in the failure message. DESIGN.md states the
     preferences: components, observers and messages first. **Done
     2026-10-29, first (the plan's step 1, guards before the
     conversions):** `checks/tests/it/ecs.rs` (callback registries on an
     allowed list, a per-crate ratchet of per-tick exclusive systems),
     `determinism.rs` over every simulation crate, DESIGN.md "ECS
     style".
  4. Speed: convert hot spots only when profsum shows them.
- **Next two games: Half-Life 2: Deathmatch, then Team Fortress 2.** Not
  full remakes: take what Ultimate Gun Game needs from each, meaning the
  character controller, HUD, weapons, player models and maps. Both are
  Source games, so most loaders exist (VPK, BSP, MDL); the work is each
  game's movement, weapons and HUD as specs, then games-as-libraries
  crates. HL2:DM's plan (2026-11-01):
  [plans/active/hl2dm.md](plans/active/hl2dm.md).
- **Then more modern games:** Call of Duty: Modern Warfare 1 or the
  earlier Call of Duties (new engines and formats: plan first).
- **Smaller installs: a hybrid (user decision, 2026-10-29).**
  - *Default:* every player has the games installed on disk and loads
    from them, as now.
  - *Asset-service mode (opt-in per server):* the server and its clients
    load assets, in a more convenient packed form, from a private asset
    service behind a CDN. Whoever runs the server hosts that service and
    gives clients its IP or URL. Before allowing any download, the
    service checks the player owns the game, through their Steam
    account. We don't distribute the service; we keep an internal one
    for our own testing.
  - *To plan* (docs/plans/active/ when started): the packed format and
    per-round manifests, the client cache, how the client proves Steam
    ownership to the host (Steam sign-in, then the owned-apps check per
    game's app id), and the protocol between client, server and service.
    Games not on Steam need another ownership check, or default mode.
  - *Plan:* [plans/active/asset-service.md](plans/active/asset-service.md)
    (2026-10-30: native files content-addressed first, IR transforms
    only where measured; the dev server first, then delivery from the
    game server, then a host's download server).

## 0. Playtest feedback (2026-10-07), top priority

### HL2:DM bodies walking over the network (found 2026-11-13, branch `ca-pose-sync`)

- A walking HL2:DM *target* over the network: the shooter's client draws
  its head 0.11 m on average (up to 0.37 m) from where the server keeps
  it for the tick drawn (lag compensation's rewind); standing, crouched
  and looking down 0.015 m (CS:S bodies: 0, Combat Arms' now 0.001-0.003
  m). Head shots at a walking HL2:DM body's drawn head can hit an arm or
  the chest (`heavy::head_matrix::every_games_gun_hits_a_head_as_drawn_over_the_network`
  prints those rows only). Measure it as
  `heavy::head_matrix::combat_arms_heads_are_drawn_where_lag_compensation_keeps_them`
  does Combat Arms' (the target on the HL2:DM rung): likely the client
  plays the walk cycle at another time than the render tick, as Combat
  Arms' followed trackers did; then assert those rows.

### UGG playtest rehearsal, 2026-11-13 (agent, this box, the playtest6 build)

What the second rehearsal found and didn't fix (fixed ones: the commits
of branch `rehearsal2`; numbers: playtest-hosting.md, "Rehearsal").
Delete each when done.

- **Publish Combat Arms' `base` again** (before Saturday): 8 files came
  on demand at each tester's first start and join (`attributes/clientbutes.txt`,
  7 textures of the install guns' own effect groups), none during a
  match. On the host, from a checkout with `rehearsal2`:
  `chimera_pack sets combat_arms --store <server store> --sign --only base`,
  then the guard (`heavy::no_fetch_in_match`, which passed on the
  current store: it counts only fetches during a match).
- **Two waiting launcher processes per tester** after a token entry (the
  token's restart, then the startup download's): 0.34 GB resident at
  first, 0.2 GB with swap later, for the session. Testers enter the token
  on Friday and start without it on Saturday (one waiter at most).
- **A swap's frames on a loaded machine**: 220-380M instructions at
  worst, 60-240M of it catch-up ticks (15-18 a frame while the box,
  swapping, ran 3 frames a second); the dedicated server's map change
  one tick of 2.7-3.8G instructions (0.3-0.6 s CPU; already listed under
  "Nothing blocks the main thread").
- **Prediction after a rejoin**: a client that reconnected to the
  restarted server on its menu's background map (de_dust2, the new
  server's map) logged 6-8 command clock jumps in its first 6 s (the
  lobby: players frozen); over the 19-minute session 16-25 jumps and
  115-190 "nothing predicted" restarts per client, mostly at loads.
- **A tester's disk**: 3.1 GB after six maps (1.6 GB of it the converted
  maps' cache, `chimera/maps`), 1.6 GB downloaded with prefetch on (it
  fetched every HL2:DM map in the pool, 27-53 MB each); playtest.md's
  "a few GB over the day" holds. Left as a note.

### UGG playtest rehearsal, 2026-11-11 (agent, this box, no-install clients)

What the rehearsal found and didn't fix (fixed ones: the commits of
branch `playtest-rehearsal`; numbers: playtest-hosting.md, "Rehearsal").
Delete each when done.

- **The dedicated server's memory, what's left** (its Combat Arms view
  models and effect file are gone: 1.64-1.78 GB in use over a session of
  four maps, docs/performance.md, "The dedicated server's memory"). The
  character models' animation files (430 MB) pose the hit spheres and the
  weapon data (230 MB) is the weapons: they stay. The playtest build's
  server over six maps of the three games (2026-11-13 rehearsal, HL2:DM
  installed): 1.3 GB resident plus swap at start, 1.9-2.5 GB in play, 2.0
  GB at the end. Left: HL2:DM's parts' 0.33 GB.
- **Client memory**: (the rehearsal's 3.5-5.5 GB per offscreen client
  is down to 2.1 GB in use after each of three maps, the first load's peak
  resident 6.6 -> 4.4 GB: docs/performance.md, "A client's memory" and
  "The dressing's view models and the first load".) Left: the character
  models' animation files (260-370 MB, parsed keys as `f32`s), the Combat
  Arms weapon set (250 MB for 2,363 weapons), the maps' textures decoded on
  the CPU before upload (the first load's largest transient). From the
  asset service a client had 2.3-2.6 GB in use before and after (not
  explained yet: profile it as the installed one was).

### HL2:DM playtest, 2026-11-09 (user, Windows, dm_lockdown)

Delete each when fixed. Check against specs/hl2dm/ and the probe; what's
the original's own behaviour isn't a bug (say so instead).

- **Decided (user, 2026-11-12): the explosion's fire stays cut at the
  world, as the game has it.** The straight edges and steps of an explosive
  drum's blast close to walls are its camera-facing sprites meeting the
  floor, ceiling and walls; `effects/fire_cloud2` and CS:S's
  `particle_noisesphere` don't ask for depth blending, so by their files
  that is the original's look (HL2's smoke, whose material asks, fades).
  Parity: not softened.
- **Decided (user, 2026-11-12): guns that do no damage stay off drawn
  ladders** (the RMS-12, the M136_AT4, the stun launchers; DESIGN.md "Gun
  game rules"), and so do the skins drawn wrong (the Neon Outlaw and the
  other ASW338LM skins painted for a missing UV layout, the AUG A3
  Champions Lightning): `weapon::harm`, Combat Arms'
  data/ladder_exclusions.toml, `ladder_skipped`.
- **Nothing blocks the main thread (user, 2026-11-09).** Rung and character
  changes, a late model's arrival, the first ticks on a map and a map's swap
  (spawned a step a frame behind the loading screen, the simulation waiting,
  no catch-up ticks after) no longer stop a frame (docs/performance.md,
  "Nothing blocks the main thread" and "A map's swap", with numbers and
  guards). Measured in the real app offscreen (2026-11-10, docs/performance.md "The real app's rungs and swaps, offscreen"): each rung's worst frame 45-65 M less ticks, 7-17 ms wall, render thread under 7 ms; a swap's worst frame 53-74 ms wall (main 42-73 ms, render world 50-62 ms), entering cs_office 7.7 s, dm_lockdown 6.9 s, warehouse 2.9 s from the command. Each swap frame over 33 ms profiled and the biggest fixed (2026-11-10, docs/performance.md "A map's swap in the real app"; profiling build): the render world's mesh uploads paced (`client::upload_pace`: Bevy's allocator wrote every new mesh in one frame, up to 385 M and 1.5 s), textures 16 MiB a frame, the first `map`'s map list read ahead, the first tick's install reopening and old rigs/objects freed on tasks, smaller spawn steps and settling slices: render worst 74-102 M (was 211-269), no render frame over 33 ms of CPU; main worst 220-255 M (was 364-439). A rung's worst frame is 25-41 M on that build (perf_report, despawns, text): fine. The frame the whole map goes into the world (2026-11-12, docs/performance.md "A map's colliders after its swap frame"): its colliders, whose mass properties Avian computes twice on insert, were 40-75% of it and now follow over the next frames behind the loading screen, the simulation still paused until the last is in (`map::spawn::Colliders`): that frame 221-232 M to 131-134 M on cs_office, 179-186 to 66-72 on dm_lockdown, 88-95 to 22-25 on warehouse, the collider frames 51-99 M; a swap's worst frame 221-232 to 155-158 on cs_office, 242-254 to 189-257 on dm_lockdown (the first tick's frame), warehouse's a spawn step as before. Its settling's prop shadows and its spawn's steps (2026-11-12, docs/performance.md "A swap's prop meshes and shadows"): behind the loading screen a prop's shadow is redrawn once the prop rests, within a counted budget a frame (dm_lockdown's settling frames over 100 M 25-34 to 2-3, their mean 90-105 M to 59-63 M); the baked props' lit meshes, the bodies' meshes with their per-joint bounds and the surfaces' grid are made by the load, and world-mesh steps go by triangles (the spawn's instructions 415-1017 M to 108-417 M, a swap's worst frame on warehouse 101-135 M to 66-77 M, on de_dust2 122-128 M to 101-111 M). Left: the text sizes no source declares (16.0, 21.2, 23.8, 16.1, 15.3, 16.875, 21.5 px in that run); of a map's swap, the first tick on the map (115-135 M of tick on the profiling build, its frame 155-210 M: the logic world's and Combat Arms' objects' spawns, their colliders' mass properties, a third of them computed twice, the first physics step), the settling's flood of spawn spots (a quarter of it), a spawn frame as long as its 12 ms budget (55-110 M), a dedicated server's map change in one tick, a ragdoll's first frame (convex hulls per death), a match's start (100 M); weapons off the ladder reading their timing on the frame; dedicated servers' nav flood in one tick.
- **A Combat Arms map's ambient loops start again on the next map
  (2026-11-10): fixed, confirm in a playtest.** Swapping from junk_flea to
  dm_lockdown, junk_flea's looping `SoundFX` started anew behind the
  loading screen. The unload stopped them and took the sound bank;
  `ui::music` put a stand-in bank in its place, forgot what it played,
  and started `CaAmbient` again, which still held the old map's sounds
  until `objects::load` rebuilt `CaObjects` on the next map's first tick
  (heard until then). `CaAmbient` is now empty while `CaObjects` isn't
  the loaded map's, and `play_ambient` stops sounds of another map
  (test: combat_arms ui `music::ambient_loops_end_with_their_map`). CS:S
  and HL2:DM are fine: their ambient_generics are events of a logic
  world rebuilt per map and soundscapes reset on the unload (test:
  cs_source `map_logic::ambient_loops_end_with_their_map`).
- **The grenade launcher's projectile is choppy on a client, "maybe all
  projectiles" (user, 2026-10-29): fixed, confirm in a playtest.** Not
  the client's interpolation (every `NetItem` was already drawn between
  snapshots): Combat Arms' objects (rockets, launcher shells, flame puffs,
  crossbow bolts, thrown devices) moved only on the server's 20 Hz
  frames and their `Transform`, which the `NetItem` carries, hopped with
  them, so a client's snapshots (at its update rate, held at the ticks
  between) drew stops and jumps: worst frame step 1.6-1.7x the speed
  with 8-13 still frames per flight at 30 updates/s, 2.9-3.5x and 23-38
  still frames at 66, uneven at 20 (a bolt 7-8x); the listen server's
  host saw the 20 Hz hops too. Now drawn between frames as Combat Arms'
  grenades already were (`special::sync_objects`): every kind 1.0-1.1x,
  none still, at 20/30/66 (`net_projectiles`). Combat Arms' grenades and
  CS:S's (which fly every tick) were smooth before and after
  (`grenades_in_flight_drawn_smoothly`). An option, off by default
  (parity: the original's client makes no copy of its own projectile,
  special_weapons.md "Projectile looks and sounds in flight"):
  `cl_predict_projectiles 1` makes the shooter's own rockets, shells,
  flame puffs and bolts leave the muzzle on the fire frame, his client
  flying its own copy from the shot until the server's rests or goes
  (`net::own_items`, net36); the user decides whether it becomes the
  default. Thrown grenades stay the server's (Source predicts the throw
  only).
- **Spawn facing toward open space (user idea, 2026-10-30; fixed, confirm in a
  playtest).** Was: today the gun
  game spawns face one of eight fixed directions (`minigame::spawns`), so
  a player can face a wall, and views can line up with map edges (the
  de_dust2 seam).
  - At each spawn the server casts about 32 horizontal rays from eye
    height, capped at about 30 m, and smooths each direction's distance
    with its neighbours.
  - The player faces the most open direction, plus about ±10° at random,
    pitch level or slightly down. Optional tiebreak: toward the middle of
    the walkable area.
  - Done per spawn, not precomputed: it costs microseconds. The server
    picks the facing and sends it.
  - Test: a spawn beside a wall faces away from it, and the facing varies
    within its open sector.
- **Melee finale on by default, longer for longer games (user, 2026-10-30):
  fixed, confirm in a playtest.** Both gun games (Lucker Party's
  `gun_game_dust2` and Ultimate Gun Game's automatic definition) end on
  melee by default (`knife_finale = true`; Host settings "Knife finale"
  turn it off). The number of melee rungs is `melee_rungs`, `"auto"` by
  default (7% of the whole ladder, `melee_share`, the power curve's melee band, at least 1) or a number, also a
  Host setting ("Melee rungs"). Each melee rung draws its own weapon from
  the installed games' knives and melee weapons (CS:S's knife, Combat
  Arms' knife-class weapons), none twice while there are enough; the
  infected's hands come up on a melee rung at their odds, once a ladder
  at most; the last rung is melee either way and its kill wins. The
  draw is the host's and replicated (`NetMinigame::rungs`). Docs/DESIGN.md
  "Gun game rules".
- **The All-Star chainsaw's attached effects (user, 2026-10-29): done
  except what the spec doesn't say.** The attached groups and their
  looping sounds are made with the view model (`view_model::attached`,
  `ui::attached_fx`), and a hit's fire sound replaces the last one on
  the fire channel. Left, for a spec session: one fire sound per hit or
  per swing, a hidden group's sound, others hearing the idle, the XK-2's
  five lights (not in the Steam effect file). Plan:
  docs/plans/active/combat-arms.md, "View-model attached effects and
  melee sounds".
- **The infected rung is a melee option, and the hook fires on every rung
  (user, 2026-10-29): done, confirm in a playtest.** *Hook:* the callback
  registry (`RungCharacterHook`, `RungCharacterHooks`, `AsRungCharacter`,
  `add_rung_character`) is gone; arming puts `ladder::OnRung { index,
  extras, from }` on the player, replaced only when the rung or its
  extras change (a respawn on the same rung fires nothing), and games
  react with observers on every rung (`On<Insert<OnRung>>`,
  `On<Discard<OnRung>>`). A network client puts the same on from the
  replicated rung and the server's extras (`ladder::follow_rungs`), so
  its observers fire locally; `OnRung` isn't replicated. Combat Arms'
  observers put `CaRungInfected` on and off and play the infection
  moment (arrivals only, not a player a machine first sees there).
  *Infected:* no longer guaranteed: it is a choice for the ladder's melee
  band (its last melee rungs; today the knife finale's), at Combat Arms'
  data odds (data/rung_melee.toml, 0.25; `rung_extras = { infected = {
  odds = .. } }`; Host settings "Rung extras: infected odds"). The
  shipped gun game has the knife finale off, so to see the infected in a
  playtest turn the knife finale on (Host settings) or set the odds to
  1. The melee band is the ladder's trailing melee rungs (the power
  curve's melee band, `minigame::power`).
- **A vertical seam through the frame on de_dust2: spawns fixed; confirm
  the edge against a CS:S capture.** A 2026-10-30 capture (Lucker Party,
  outside the arch toward B tunnels) showed a hard vertical line down the
  middle of the frame. The spot was a party spawn, `setpos -1031.46
  2303.15 32.67; setang 0 0 0` (eye; between CT spawn and B doors, facing
  east), 0.85 units from the edge at y 2304 between two displacements,
  looking along it: `de_dust/rockwall_blend` north (no `$bumpmap`, so
  compiled without bump lightmaps) and `de_dust/groundsand_blend` south
  (bumped). Everything in that vertical plane (the edge, the tower's
  wall seen end-on) lands on the frame's middle column. *Not a rendering
  fault (2026-10-31):* the albedo debug view shows no line (the same
  `groundsand03a` both sides), and the two flat lightmaps agree along the
  edge (`heavy::map_de_dust2::the_b_mound_seam_is_where_bump_lighting_stops`);
  the line is the bump lighting stopping there, drawn the way
  specs/cs_source/shaders.md says CS:S draws a bumped face next to a flat
  one. *The spawns' part, fixed:* spots were centred in their nav areas,
  which are often symmetric about a line of the map's grid (35 of
  de_dust2's spots sat on that one edge's line; 2.6 times chance within a
  unit of a 64-unit line); each spot is now nudged a seeded amount off
  the centred grid (`minigame::spawns::SPOT_NUDGE`; tests
  `spawns::spots_are_nudged_off_the_areas_centred_grid` and, on
  de_dust2, `heavy::spawns::dust2s_spawn_eyes_keep_off_its_grid_lines`:
  now at chance). Left: `refcmp capture-ref --only seam_b_mound` on the
  Windows PC (views `seam_b_mound`, `seam_b_mound_south` in
  tools/refcmp/de_dust2.toml), then `report`, to see that CS:S draws the
  edge as sharply.
- **Silencers detached from skinned guns (user, 2026-11-13): fixed, confirm
  in a playtest.** On the "Glock 23 _ Champion" and the "L96A1 Desert" a
  fitted silencer floated far off the gun. *Cause:* the install's
  `Glock_23_Champion` record carries the L96A1's part offsets
  (`MuzzlePVPosOff` (0, 0, 4), the scope's and third person's), which
  put its silencer half a metre ahead of the pistol and its red dots off
  the slide; the game reads them as they are. *Fix:* a record whose
  offsets differ from those most records of its model carry, and equal
  those most records of a model of another gun class carry, takes its
  own model's (`data::model_part_offsets`; of the install's records only
  the Champion; test
  `attachments::a_skins_part_offsets_copied_from_another_class_are_its_models`).
  The L96A1 Desert's silencer sits on its barrel in first and third
  person in this build (its record and sockets are the L96A1's; before
  and after screenshots in the agent's report). *Sweep:*
  `heavy::attachment_fit` puts every part a gun can be drawn with (a
  rung's candidates and its drawn defaults) on its view model (idle
  pose) and held model and checks that it touches the gun: on 1875
  guns, 29 gun-part models were off before (the Champion's 13), 16
  after, all `INSTALL_OFF`, whose models' own sockets are off: the
  Azazel Heartseeker's and the Silent Snowfall and Tropical Groza skins'
  `flash` socket lies 23 units ahead of the muzzle, the SIG50 Lava's and
  the Devil Soul P90's `Flash` past their barrels. Open (spec session):
  does the game draw those off too, or does a muzzle part follow the
  weapon's flash point? Parts no rung can draw (slots their records
  close: the Desert Eagles' and Anacondas' red dots, the SR25s'
  silencers) are off on more guns; not swept.
- **Rung attachments may not fit their gun, or not show (user, 2026-10-29): fixed, confirm in a playtest.**
  In the Escape-menu screenshots, the FMG-9's rung listed a holosight and
  the Luger P08's a silencer, but neither shows on the gun, and the user
  doubts they fit. Check, with the real install:
  - *The accepted list:* where each gun's list comes from. The
    modification file's `Weapon#` record is linked by position
    (attachments.md Q13), and a mismatched name is only logged. Count the
    mismatches over the pool; check skin variants (the `_Gold` and `_Red`
    parts on gold and red guns).
  - *What the draw takes:* `rungs::draw_parts`, against each slot's
    rules: requirements, conflicts, the slot's `Kinds`.
  - *What's drawn on screen:* are fitted parts drawn on the view and held
    models (copies included)?
  Fix the draw to take only parts that fit, and draw fitted parts on the
  models; questions the specs leave open go to a spec session.
  With it, odds for the draw (user, 2026-10-29). Each slot keeps the
  gun's stock part most of the time (about 70-80%). Odds per kind:
  sights and scopes most often, then silencers, power parts (ammo,
  magazines) least (about 10%). At most two non-stock parts per gun. The
  odds live in the ladder rules as data. Fitted parts can add to a gun's
  power score once the power curve exists.
  Done: *the lists* are right (position linking holds: of the pool's
  1942 guns 1835 records carry the gun's name, 105 a spelling of it or
  a shared list, 2 swapped, which the game links by position too), and
  Gold/Red parts are listed for every gun of a family; but each gun's
  record closes slots its list names (`SightOption`/`MagazineOption`/
  `MuzzleOPtion` 0, or 2 for a built-in part: the FMG-9 takes no sight
  or magazine, the Luger P08 no silencer, sight or magazine; 1053 guns
  list sights for a closed slot), and the draw ignored that. The draw
  now takes only parts a slot accepts and its record leaves open, never
  in a slot with a default part (a sniper's scope, an event weapon's
  built-in parts stay) (`rungs::slot_candidates`; Gold and Red are
  grades of a part, drawn like it), at the odds of
  `data/attachments/rung_odds.toml` (sight 0.40, silencer 0.30,
  mechanical sight 0.20, magazine and bullet 0.10, at most two; about
  78% of slots stock), tunable per definition (`rung_extras = {
  attachments = { sight = 0.5 } }`). *Shown:* parts weren't drawn at
  all; now silencers and sights are drawn on the view model and the
  held model (`view_model::parts`, `models::load_held_weapon`); the
  muzzle parts' frame is an open question
  (docs/plans/active/combat-arms.md "Attachment models").
- **The rung line lists raw attachment ids: fixed, confirm in a
  playtest.** Done: the install's names (the item file's `Option#`
  records, `ui::install::part_names`) without the class suffix ("S1
  Suppressor I"), readable record names without the install's strings
  (a dedicated server), capped at 48 characters. With rung extras on, the
  gun game panel's top line reads like "Rung 1/12: FMG-9
  (Silencer_A_SMG_Gold, DotSight_Holosight_SMG_Gold, ...)". It runs
  across the whole screen and shows record ids. Show the attachments'
  display names from the install's string table, shortened (or icons),
  and wrap or cap the line.
- **A natural power curve for the random ladder (user idea, 2026-10-29):
  fixed, confirm in a playtest.**
  Weak guns first, power rising, then the hard-hitting ones, then
  specials, a grenade, melee last. Proposed design, waiting on the
  user's three choices:
  - *Score:* time to kill one baseline player (100 health, the same
    armour) at a typical range of 10-20 m: shots to kill (damage per
    shot with pellets and range falloff, over baseline health) times
    the fire interval plus the pre-fire delay. Scaled by the hit rate
    from spread at that range; small headshot and magazine terms.
  - *Bands by category:* the score orders guns only inside a band.
    1 warm-up (pistols, weak SMGs, weakest first); 2 rising (SMGs,
    shotguns, carbines); 3 peak (rifles, LMGs); 4 heavy (snipers,
    heavy calibre); 5 special (launchers, flame; one or two at most);
    6 a grenade; 7 the melee finale. So rung 1 is always a pistol or a
    weak SMG.
  - *Generation:* split the rungs over the bands; draw each band's guns
    at random from its pool, sort them by score, and mix the games. A
    console command prints each rung's band and score.
  - *Decided (user, 2026-10-30):* power keeps climbing: warm-up, rising,
    peak, heavy (snipers and heavy calibre are heavier, not harder), then
    specials, then a grenade rung, then melee (1-3 rungs, see "Melee
    finale on by default"). Band sizes for 10 rungs: 2/2/2/1/1/1/1,
    scaled for other lengths, as data a ladder can tune.
  - *Bands as shares (user, 2026-10-30):* each band has a share of the
    ladder, adding up to 100%. Rung i of N sits at (i + 0.5) / N and takes
    the band covering that point. Each ladder moves the boundaries a few
    percent at random. At least one warm-up rung and one melee rung.
    Defaults (data): warm-up pistols 15%; rising SMGs and shotguns 20%;
    peak rifles and carbines 28%; heavy LMGs, marksman rifles, snipers and
    heavy pistols 15%; specials 8%; grenade 7%; melee 7% (user: melee
    rarer, about 1 on 10-20 rungs and 2 on 30; at least 1).
  - *Done (2026-10-30):* `chimera_sim::minigame::power` with its data,
    chimera/sim/data/power_curve.toml: the bands, the shares (jitter 3%,
    a rung on a boundary takes the band above: 10 rungs 1/2/3/2/1/0/1,
    20 3/4/6/3/1/2/1, 30 4/6/9/4/3/2/2 before jitter), the class map
    (new classes `carbine`, `marksman`, `heavy_pistol`, `launcher`,
    `flamethrower`; Combat Arms' launchers and flamethrower, CS:S's Desert
    Eagle, SG550 and G3SG1 moved; a pistol hitting 50+ is a heavy
    pistol), and the score (DESIGN.md "Gun game rules": time to kill at
    10/15/20 m from damage, falloff, pellets, interval, latency, spread
    as a hit rate; CS:S's accuracy and Combat Arms' damage bands, cone,
    fire rate, projectile and flame read by each game's
    `add_power_reader`). `ladder_power` prints every gun's score and
    band. Ultimate Gun Game's ladder (`auto::ladder`) is drawn on it,
    `RUNGS` 13 in all with the melee band as `melee_rungs`; one melee
    share for both (`melee_rungs = "auto"` on a written ladder is the
    band's count, `power::melee_rungs_for`). A written definition opts
    in with `ladder = "auto"` (`rungs`, `power_curve`), drawn on the
    host at each match's start and replicated with its melee count
    (net38); Lucker Party's `gun_game_dust2` keeps its written ladder.
    Not done: Host settings for the shares; fitted parts adding to the
    score.
- **Spawn protection: 2 s, see-through, shot through (user, 2026-10-29).**
  - The party gun games' protection becomes 2 s (from 1.5 s); retime the
    tests that assume 1.5 s.
  - While protected, a character draws about 50% transparent, on every
    client, so others can see their bullets won't do anything.
  - Bullets pass through a protected player and hit what's behind:
    no damage, blood, impact or decal on them. That part is already
    true for damage and effects. Firing already ends the protection
    (`fire_ends`, the default).
  *Fixed, confirm in a playtest (2026-10-29):* the party gun games
  protect for 2 s; the body draws at alpha 0.5 on every machine
  (`render::tint::fade_protected`, from the replicated protection) and
  fades back over 0.2 s; in the bullets window rounds, swings and thrown
  grenades pass through the player to what is behind (both games'
  deliveries), blasts leave it out; any attack (fire key, a knife's
  stab, a throw) ends the protection. Its blob shadow stays as dark.
- **CS:S rungs get a character too (user, 2026-10-29): fixed, confirm in
  a playtest.** As for Combat Arms rungs, each CS:S rung draws one of
  CS:S's eight player models (the stock T and CT ones; no other is
  loaded), the same for everyone on it (`cs_source::game::rungs`; the
  `character` option covers both games). Not compared: whether the eight
  models' hitboxes are identical.
- **Later: let individual players stand out (user ideas).**
  - Clearer name tags when you aim at someone (their name and more),
    kept unobtrusive.
  - A flaming aura around the current leader.
- **A rung's character: stats only when intrinsic (user, 2026-10-29):
  done, confirm in a playtest.** A drawn Combat Arms body keeps only its
  body type (`FigureType`: sprint, stamina, variable terms), what is its
  own; its armour, protection, penalty, sprint and stamina bonuses, hit
  points and animation rates are the vest's (the body record is the
  vest, gear_perks.md 3), and every rung body wears the baseline vest
  (model 0's Normal, `gear::RUNG_VEST`; `characters::CaRungLook` now
  replicates, so a client derives the same and isn't corrected at a
  swap). The install's two body types are identical, so in practice
  every rung body plays as the stock one.
- **Zombies: a character tied to its weapon (user idea, 2026-10-29):
  done, confirm in a playtest.** Chimera's `ladder::RungCharacter`
  (arriving and leaving: `ladder::OnRung`'s observers); Combat Arms'
  infected rung (`combat_arms::game::rungs`, `rung_extras.infected`; a
  melee rung's choice since 2026-10-29, above): the Quarantine hands alone
  (191, else any install hand: `combat_arms:infected` weapons), an
  infected body with its override's face, the infection sound
  (`infected_male.wav` / `infected_female.wav`, ours: unspecified) and
  `Quarantine_Exp_01` at the player; baseline health and movement, no
  knock-back. Not drawn yet (unspecified, open questions in
  docs/plans/active/combat-arms.md): the screen overlay, the eye
  trails, the override's virus render styles and hands. (All specified
  since: game_modes.md Corrections (2026-10-30) 1-7; the eyes are the
  face's `ClientFX#` groups, `Eye_Red_Tail_L` for the stock infected
  faces, on the `EYE_L`/`EYE_R` sockets: item 7, case 114.) **Eyes done
  (2026-10-31):** the override's face loads with its body group and is
  worn on the infected rung and by Quarantine's infected; its
  `ClientFX#` groups play looping on its `EYE_L`/`EYE_R` sockets, third
  person only (`effects::attached`; tests `attached_effects` t114). Still
  not drawn: the virus render styles and the override's hands. The idea as
  given:
  Some Combat Arms characters can only use certain weapons: the infected
  (zombies) fight with their hands. Make them a special class. A rung
  that gives the infected hands turns the player into the zombie, and a
  zombie never draws a normal weapon.
  - *Hook:* a "becoming the next character" hook in the gun game, so a
    game can play its moment: Combat Arms' infection sound, its
    screen effect (a red tint or a vignette, per the spec), the
    zombies' glowing eyes with trails.
  - *Not copied:* the zombies' large health and their heavy slowdown
    from bullets. Keep the gun game's baseline, so it stays fair.
  - *Specs:* game_modes.md (Quarantine M8, infected knock-back S14),
    gear_perks.md 15 (infection-mode perks), missions.md F4.1; anything
    the specs lack goes to a spec session.
- **Party panels draw washed out with no darkened backdrop (user,
  2026-10-29): fixed, confirm in a playtest.** The Escape menu and the
  Controls page asked for a 0.55 (0.5) black shade and a 0.96 panel, but
  Bevy's UI blends in linear light: the shade left the world at ~70% of
  its brightness and bright text behind the panel showed through as a
  ~25% grey. With `mat_monitorgamma` below 2.2 (1.8 on the dev box) the
  brightness pass lifts the whole frame, menus too, which washes them
  out further. Not the cameras: with every camera's copy back
  (`CHIMERA_SKIP_OUTPUTS=0`) the panel's pixels were the same. Their
  alphas now go through `widgets::display_alpha` (`1 - (1 - a)^2.2`:
  covering as that alpha does blended in display space, VGUI's way).
  Test: chimera_client
  `widgets::display_alpha_darkens_as_a_display_space_blend_would`.
- **A client without the Combat Arms key, or with another install
  version, builds other weapons than the server: fixed, confirm in a
  playtest.** The ~2,300 install copies come from the attribute file
  (V1 key) and registry indices went over the wire unchecked: such a
  client built the wrong weapon or none, and its clips, rates, spread
  and attachment records were its own. Now the server's data documents
  go to a client whose own differ (chimera_sim's `server_docs`,
  chimera_net's `net::server_docs`, net33): the manifest offers each
  set's hash and the registry's; the client asks for the sets that
  differ (Combat Arms' weapons, attachment records, movement:
  `data::served`), rebuilds its registry from them before it enters the
  game, and leaves with the reason if its registry still differs or a
  set is too big to send (4 MB packed; the install's weapons pack to
  ~280 KB from 14.4 MB). Models and sounds are still the client's own.
  The gear and perk records go too (`combat_arms:gear`, net34), so a
  vest's movement penalty and reload and select bonuses agree.
  Tests: combat_arms `server_docs` (0 prediction errors on a copy's
  burst after the documents came, the same clip; nothing sent to a
  client with the same data; refusals),
  `weapon_timings::a_vests_rates_shorten_the_reload_and_draw_on_both_sides`.
- **The gun game's title names Dust II on every map: fixed, confirm in a
  playtest.** The definition's name is now "Gun Game: {map}", `{map}` the
  map played ("Gun Game: Aztec"; `minigame::hud::shown_name`).
- **Bots' generated nav reaches outside de_aztec: fixed, confirm in a
  playtest.** Not over the walls: a one-cell gap a CS:S player doesn't
  fit through (Combat Arms' hull is 30 u, the walk test is at cell
  centres), source `setpos 2923 600 -179`, opens 7900 m2 of jungle, and
  its 0.8 m jump-ups add more; the jungle is in valid clusters. Now a
  generated mesh on a map with visibility data is bounded
  (`nav_gen::generate_within`, `Bounds`, used by `brain::ensure_nav`):
  only cells whose chest is in a cluster a walking player's mesh (0.45 m
  step, 0.4 m radius) reaches from the same seeds, so the floor beyond
  goes and so does everything reached only through it. Bots' mesh areas
  (m2) unbounded -> bounded: de_aztec 2584 (14748) -> 1404 (6116),
  de_dust2 1351 (4948) -> 1278 (4876), cs_office 1097 (4010) -> 1074
  (3966), de_port 3288 (12602) -> 2518 (10300); no floor a player walks
  lost on any of them; what goes inside is jump-up spots in clusters no
  walk enters (wall tops, a few box tops). Tests: chimera_sim
  `nav_bounds` (toy wall climbed by ledges onto an outside floor);
  party_kit `heavy::nav_bounds` (de_aztec: no area in the jungle, four
  bots three minutes never there; the four maps' counts).
- **Some Combat Arms bodies draw grey pieces: fixed, confirm in a
  playtest** (a ghillie-suit specialist; the Ice Mas Knight was fine).
  A body's texture slots 5 on are its record's `AdditionalSkins#`
  (characters.md 2, "the record's additional skins"), which we didn't
  read: the ghillie suits' leaves piece names slot 5
  (`CHARS_T_BODY\CM_Fur_Ghillie_Suit_DF_SD.DTX`) and drew the
  untextured grey. Now `BodyLook` / `ModelRecord::additional_skins` and
  `models::slot`. The sweep of the 572 bodies of the rungs' pool
  (`heavy::body_skins`, each body piece's first texture slot;
  `CHIMERA_BODIES_LOAD=1` also loads each as a rung does, faces and
  head pieces too) on the dev box's install:
  - grey before, textured now: 7 pieces, the leaves of the 7 ghillie
    records (72 `Ghillie_Suit`, 174-176 `Ghillie_Suit_Color_A/B/C`,
    213-215 `Gear_Ghillie_Suit_0_DF/1_DE/2_UR`); no other body, face,
    head piece or attachment loads untextured; no body skin is a sprite,
    past its list, missing or in another archive;
  - the install points at nothing: 279 `Medium_NM` and 280
    `medium_NM_female` name body files it doesn't have
    (`CM_Body_Medium_vest_KE.ltb`, `CW_Body_Medium_Female_KE.ltb`): a
    rung drawing them keeps what its players wore (build 26236938 still
    lacks both; the game would draw the engine's small placeholder
    `models\default.ltb` there, characters.md Corrections (2026-10-30)
    3); the rungs' body pool leaves them out now (`rungs::missing_bodies`);
  - drawn but maybe not as the game: 163 pieces with a second texture
    slot (mostly slot 5 or 6, `textures\128xPV_SpecialSpecR.dtx`: not
    drawn), render styles our table doesn't know, drawn opaque
    (`RS\PV_Char.ltb` 139, `RS\ENV_ALPHA_MASK.ltb` 41, `RS\PV_Char_B.ltb`
    4, `RS\EdgeAmp01.ltb` 2), the leaves blended in the mark's
    `RS\MARKTRANSPARENT.LTB`, and model colours (`ColorSkin{c}_`) not
    read. Open questions: docs/plans/active/combat-arms.md, "Body
    skins". Tests: models `body_texture_slots`, attribute_data
    `body_additional_skins`, `heavy::body_skins`.
- **Firing animation on the match-end screen: fixed, confirm in a
  playtest** (user). The rules hold a frozen player's intent only at the
  simulation's tick (`FreezeTime`, the end freeze), but the input writes
  the raw trigger every frame and the view-model drivers (Chimera's
  `view_anim::drive`, CS:S's activities, Combat Arms' `CaViewKey`
  machine) and the effects read `Intent` in `Update`, so every gun's
  first-person fire animation (muzzle flash, brass, the Combat Arms
  machine's fire keys) played with the trigger held, though no round was
  fired. All of CS:S's and Combat Arms' guns read it alike. Now
  `predict::hold_view` (after `write_local_intent`, before the drivers)
  applies the same hold to the local intent (`FreezeTime` or the server's
  freeze for a network client): the end screen and a round's freeze time.
  The same hold fixes the user's second report (footsteps, bob and
  other movement effects while pressing WASD on the end screen, Combat
  Arms): the move keys are cleared too.
  Tests: cs_source `view_models::a_held_player_holding_fire_shows_nothing`,
  combat_arms `effects_sounds::a_held_player_holding_fire_shows_and_sounds_nothing`.
- **Round music loops and gets stale: fixed, confirm in a playtest.** In
  a party match the start track plays once, then a shuffled playlist of
  Combat Arms' menu tracks (each once before any repeats, cross-faded);
  the last stage's music still takes over and the playlist resumes after
  it. `party_music` 0 off, 1 playlist (default), 2 the original's loop;
  volume is Combat Arms' `musicvolume` option (CaVolumes). Combat Arms' own modes are unchanged.
- **Combat Arms launchers: the boss flame launcher hurt nobody, every
  rocket was a grey ball: fixed, confirm in a playtest; the looks in
  flight need a spec.** The "Hephaestus" is the install's
  `Powered_Suit_Boss_Launcher` (`Guntype` 9; its ammo fires the flame
  puff `Yellow_Flame` with a burning sphere, 10 HP/s, 999 rounds,
  infinite: the 999 is its data). Install weapons we have no document for
  were copied from the first of our documents of their `Guntype`, the
  rocket launcher, so its puffs flew as rockets with no blast and no
  sphere. Now a copy takes the document whose weapon fires the same
  projectile record (the flamethrower), and every impact projectile
  weapon takes from the install its record's speed, lifetime, gravity and
  model (drawn in flight: the LAW and its NPC copy `LAW_npc` show the
  missile, not a ball or the launcher), its ammo's sphere, its `FireSnd`
  and its ImpactFX (the rocket's `gren.wav` and `Grenade_Dirt` at the
  blast, the M32 family's `M32_EXPLOSION.wav` and `M32_Dirt`). Tests:
  combat_arms `install_launchers`. Same fix for every `Guntype` 9 and 15
  copy (the flamethrower skins and `HQRAID_BOSS_FLAME`, the LAW, RPG-7,
  AT4 and RMS12 skins, the boss missile, the M79/MZP-1 family). Left for
  a spec session (docs/plans/active/combat-arms.md, brief 24b): the
  projectile record's trail group (`FXName`), flight sound (`FlySound`),
  flare and light. **Done (2026-10-31)** from special_weapons.md's
  Follow-up (2026-10-30): the record's group rides the projectile
  (`effects::attached`; a flame puff's `Flame_Yellow` plays, so flames
  show), its `FlySound` loops and follows it, its flare and smoke trail,
  its model faces its fire direction, launchers show their FireFX record
  at the fire position, launched ammo's blast plays the surface's impact
  group (tests `attached_effects`, `install_launchers` t48-t53). The
  activate, finish and create sounds are done too (Corrections
  (2026-10-31) 3: `attached_effects` t61-t63, `net_projectiles`). Left:
  the trail key's width
  styles and `UAdd`. The M32 family firing
  copies of itself: fixed with the rest (the shell was drawn by the
  weapon's key, the M32's held model; now `M32_Shell.ltb`). The
  "Anti-Virus Flamethrower" is most likely `RMS12` (its HUD name image,
  `Name_RMS12`): a flamethrower copy now (500 u/s puffs, no ball), but its
  ammo has `ProgDamage` 0, so its spheres do nothing (special_weapons.md
  4, Open question 8: as the data says); `RMS12_Bounty` burns 30 HP/s.
  Its 134 ms single-key fire loop (spec 4) isn't taken yet (the copy has
  the flamethrower's keys, brief 24b item 9).

- **Spawned outside de_aztec: fixed, confirm in a playtest** (user: `setpos
  2519.69 2283.46 210.62`). The gun game's `random_hidden` spots
  (`chimera_sim::minigame::spawns`) come from the map's navigation mesh.
  de_aztec ships none, and with Combat Arms bots in the match their
  brain had already put its own generated mesh in the `NavMesh`
  resource, made for a walker that jumps 0.8 m ledges
  (`bots::brain::nav_params`). That mesh climbs over de_aztec's walls
  onto the jungle outside: its area 43 holds the user's spot, on a floor
  in a valid cluster, so every check passed. Now a generated mesh
  (`NavMesh::generated`) is never used for spawns: they walk their own
  from the spawn points at a player's step, and on a map with visibility
  data a spot must also be in a cluster's leaf (`spawns::inside_map`: not
  solid, not the void). Sweep of every map the party apps rotate to, all
  candidate spots (no cap), with the bots' mesh loaded: 0 outside the map
  and 0 off the walk on every map. The bots' mesh alone would have given
  (spots off the walk / all): de_aztec 4654/7869, de_port 2281/9122,
  cs_havana 1502/4303, warehouse 1110/1251, cs_assault 471/3151, de_dust
  386/3668, de_inferno 366/2620, de_train 296/2000, bloodmoney 270/3578,
  de_piranesi 200/3619, de_prodigy 176/1512, de_dust2 136/3278,
  cs_militia 126/23426, cs_compound 88/7500, de_cbble 68/4135, de_chateau
  61/2422, cs_office 60/2425, de_nuke 52/3261, cs_italy 27/1907, de_tides
  26/1499, darkhills 1/223 (not all of these are outside the map: some
  are ledges only a jump reaches; none of them was in a solid or void
  leaf). `combat_arms:junkflea`, in the Combat Arms parts' pool, doesn't
  load on this box ("no such map (worlds*/junkflea.dat)"): check the
  pool's name for it. The bots' mesh reaching outside: fixed (the entry
  above). Tests: chimera_sim
  `spawns::a_mesh_generated_for_another_walker_is_not_spawned_on`,
  `spawns::spots_in_solid_or_void_leaves_are_not_spawned_in`; party_kit
  `heavy::spawns::every_pool_map_s_spawn_spots_are_inside_it` (the sweep,
  `CHIMERA_SPAWN_SWEEP=<map>` for one).
- **de_aztec's barrels ride on top of the water: fixed, confirm in a
  playtest.** Six of de_aztec's 16 oil drums (`props_c17/oildrum001`, 30
  kg in a 0.44 m³ hull: density 68 kg/m³) float in its shallow canals (the water
  10 to 15 units deep over the floor, surface at z −528). `map::buoyancy`
  took the wet share of the body's box (avian's `ColliderAabb`, which is
  grown for contacts and swept along the velocity) times the hulls'
  volume, so a drum rode high, and its vertical drag was scaled by that
  share (7 %), so it bobbed for ever (±0.3 units at 2 Hz); a sleeping drum
  was woken every step and kicked. Now the lift is the collision hulls'
  volume cut at the surface (`buoyancy::submerged`, at the wet part's
  centre), the bobbing is damped at a 0.3 ratio of its own rate, and a
  drum at rest stays asleep. The drums now lie on their sides 3.6 units
  under (within 0.25 units of where their weight of water is under) and
  sleep within 1.5 s. Tests: cs_source
  `logic_physics::an_oil_drum_floats_at_its_depth_and_stops_bobbing`,
  `heavy::map_water_props::de_aztec_s_drums_float_at_their_depth_and_settle`.
  Open questions for a spec session (physics_props.md has nothing on
  water; none of this was guessed from Source, the figures are ours):
  1. How deep does a prop float in CS:S: is it the share mass / (1000
     kg/m³ × the `.phy` hull's volume), or does vphysics use the surface
     property's density (`metal_barrel`'s) or another water density?
     Measure de_aztec's drum (and a wood crate) at rest in water: how far
     its lowest point is under the surface, upright or on its side.
  2. How fast does a floating prop stop bobbing: dropped from 32 units
     into deep water, the heave's period and how much each bob loses
     (damping ratio), and when it sleeps.
  3. Drag in water: horizontal and angular slowing of a floating prop
     pushed at 100 units/s or spun, and whether it scales with the wet
     part.
  4. Which surface: the water brush's top face, and what a prop in two
     water brushes of different heights does.
- **Combat Arms radio menu: number keys leak through, and raw text
  keys: fixed, confirm in a playtest.** The pick key stayed held after
  the menu closed and the slot bind (read while held) took out the
  weapon; menus now take their keys (`CapturedKeys`, CS:S's radio menu
  too). The chat line held the `$radio_message_...` id inside
  `[Class] Name : $id`, which only a leading `$` resolved; the table
  (`language_pack`, V1-keyed, reads here and has all 27 chat ids) now
  resolves the ids inside radio lines at the viewer's end.
- **CS:S grenades bounce too much.** MATCHES THE SPEC; needs a spec
  re-check on the Windows PC (2026-10-29). Checked rule by rule against
  specs/cs_source/grenades.md section 4 (`chimera/sim/src/weapon/grenade.rs`
  `fly`, `bounce`, `gravity_step`; numbers in `cs_source::game::grenades::
  flight()`, shared by HE, flashbang and smoke): box 4 in, gravity 0.4 x 800,
  elasticity 0.45 x (1.0 world, 0.3 player) capped 0.9, mirror then scale,
  floor second sweep, wall loses the rest of the tick, stop under 30 in/s,
  no ground friction, glass 0.4, 0.015 s ticks, metres (0.0254 per inch)
  all agree. Test `cs_grenades::css_grenades_bounce_as_the_spec_says_on_
  floor_and_wall` follows a tick-by-tick model of the spec: a level throw
  (590.88, 104.19 in/s) bounces to apexes 34.96 / 7.4 / 1.9 / 1.0 in,
  stops 1.6 s after, and a wall turns vx to -265.9. So if it still looks
  too bouncy in play, the spec's numbers need re-checking against the
  reference (a spec session on the Windows PC): elasticity 0.45 and the
  world's 1.0, the 30 in/s stop speed, gravity scale 0.4, whether the
  bounce speed test or the surface elasticity differs in CS:S's own game
  code (the spec's rules come from the SDK template, not CS:S's DLL), and
  whether CS:S bounces off the "grenade" surface property's elasticity
  (not the flat 1.0). Measured data in the spec (Q7: an HE dropped on tile
  stops in 4 contacts, a smoke in 3) is the thing to compare ours with.
- **Ultimate Gun Game's Escape menu (user, 2026-10-29): fixed, confirm
  in a playtest** (`party_kit::esc_menu`, `party_kit::host`; DESIGN.md
  "The party apps' Escape menu"). It's
  Ultimate Gun Game's own, not Combat Arms', drawn above the match-end
  screen. Entries:
  - **Resume.**
  - **Options:** video, audio, mouse and binds, plus each player's own
    party settings (`cl_hitmarker`, `party_swap_effect`, `party_music`).
  - **Host settings:** only for the host or an admin. The match's server
    settings: `party_even_footsteps`, the ladder rules (rung extras,
    demote on suicide, knife finale), map rotation, bots, spawn
    protection.
  - **Vote to skip the map:** on everyone's menu. It passes with a
    majority of the players, then goes to the next map in the rotation.
  - **Quit.**
  - No scoreboard entry, since Tab opens it. Disconnect: done with the
    main menu's M1.
- **RCON admins for the Escape menu's host settings.** Today only the
  host (a listen server's player) sees them; a dedicated server's admin
  sets the same cvars on its console.
- **A main menu for Ultimate Gun Game (and the other party apps).**
  M1 done (2026-10-31: Play, Join by address, Options, Quit, loading and
  failure boxes, Disconnect; CS:S's menu's look over a live map). Next:
  M2 LAN browser, M3 master server; menu music (Combat Arms' menu
  playlist). Plan:
  [docs/plans/active/main-menu.md](plans/active/main-menu.md).
- **Kill feed: Combat Arms gun icons sit at the top of the line: fixed,
  confirm in a playtest.** Rows are one fixed height and each icon sits in
  a line-tall centring box (`kill_feed::icon_slot`).
- **Combat Arms guns' own sounds, scopes and models: audit (2026-10-29),
  what's left.** The six fixes are done (the install's sounds, muzzle
  groups and ammo effect names per weapon, held models on demand, rates
  from the models, our documents' wrong values, `DotSightName`, special
  weapons' scopes). Left:
  - `HHMuzzleFXName` of the crossbow and the M67: effects_sounds.md and
    special_weapons.md case 59 define a third-person muzzle group for
    launchers only (done: `WEAPON_HH` for the LAW, RPG-7, M136 and M79,
    none for the M32 and the flamethrower). Spec question: do another
    player's crossbow shot and grenade throw make their record's
    `HHMuzzleFXName` group, and at which point?
  - the timing work's open questions 5-8 (docs/plans/active/ca-weapon-timings.md).
- **Can Combat Arms guns shoot through glass? Fixed, confirm in a
  playtest.** Checked against weapon_firing.md "Penetration" (Glass,
  surface 40: passes up to 10 u, damage x 0.9, re-perturb 5; Bullet-Proof
  Glass, 41: not shoot-through, stops the round, its own impact
  `Impact_Glass_BulletProof`). What differed, now fixed:
  - CS:S maps: a Combat Arms round broke a window (func_breakable_surf)
    or glass func_breakable but stopped there: brush entities had no
    surface for it (the world's `SurfaceGrid` holds the world's own
    triangles only). Their nodes now carry `map::BrushSurface` (the
    surface most of their faces have: `glass`, Combat Arms' Glass through
    the mashup's table), so the round breaks the pane and goes on (party
    `cross_glass`, mashup `heavy::cs_maps::a_combat_arms_round_breaks_an_office_window_and_goes_through`).
  - Combat Arms maps: glass was read by texture from the nearest drawn
    triangle, typed by its solid polygons only, so non-solid glass and
    glass world models were Unknown (stopped rounds, no glass impact).
    The glass collision sets are now one per type with their surface
    (`MapCollisionSet::surface`, on the collider as `PropSurface`), world
    models' textures are typed too (nodes take theirs as `BrushSurface`),
    and impacts read what was hit before the world (`effects::owned_surface`)
    (combat_arms `world::glass_lets_rounds_through_and_bullet_proof_glass_stops_them`,
    `weapon_firing::t37_*`, `effects_sounds::a_round_on_bullet_proof_glass_takes_its_impact`,
    `heavy::world::glass_sets_and_glass_world_models_name_their_surface`).
  Combat Arms' own glass doesn't break (only `Breakable` objects do, by
  hit points; world_objects.md 11). Surface Unknown (0) is shot through
  as the install's table says (effects_sounds.md Follow-up (2026-10-31b)
  3); the level's own walls also need their far side within the limit
  (our open meshes: `weapons` Known shortcuts).
- **Random attachments on Combat Arms rungs (user idea): fixed, confirm
  in a playtest.** Built as Chimera's per-rung extras hook
  (`ladder::RungExtrasHook`, `rung_extras` in `[rules]`) with Combat
  Arms' draws (`game::rungs`): a fitted set, a character (install
  specialists included, loaded when first worn) and a melee weapon per
  Combat Arms rung, the same for everyone on it, the fitted parts in the
  HUD's rung line (tests `chimera/sim/tests/it/rung_extras.rs`,
  `apps/party_kit/tests/it/rung_extras.rs`). Some Combat
  Arms guns come bare. When the ladder is made, give each Combat Arms
  rung a random fitted set from that gun's accepted attachments
  (`game/attachments.rs`, each weapon's `combat_arms:attachments`), the
  same for everyone who reaches the rung, so it stays fair; shown in the
  HUD's rung text if there's room.
  With it, a random Combat Arms character per Combat Arms rung (user,
  2026-10-29: per rung, the same for everyone on it), drawn from every
  character model the install has, specialists included, not only the
  default set.
  And the melee slot too (user, 2026-10-29): each Combat Arms rung
  draws a random Combat Arms melee weapon (knives and the rest of the
  melee class), the same for everyone on the rung. The ladder's knife
  finale keeps its own knife.
- **de_piranesi's sky blank (grey), the sky showing only at the edges of
  some see-through objects: fixed, confirm in a playtest.** The camera
  pass's skipped copies (docs/performance.md, "Second pass: the
  cameras") left the UI camera copying the frame out, and Bevy
  alpha-blends every copy but a target's bottom camera's over what the
  target holds: the window's clear colour showed wherever the frame's
  alpha is below one, de_piranesi's sky texture (de_dust2's and
  cs_office's skies are opaque, so the frozen-scene checks matched);
  the edges of alpha-to-coverage foliage and fences wrote alpha one.
  The camera copying for skipped ones now replaces the target
  (`render::cameras::skip_covered_outputs`; test
  `chimera/present/tests/it/camera_outputs.rs`).
- **Frame rate drops after a few map changes: fixed, confirm in a
  playtest.** What grew was archetypes and tables (Bevy frees none; the
  characters' optional components make new ones at every map change and
  death), and exclusive systems that built their queries every tick walked
  them all: the tick cost 78% more after ten round trips. They keep their
  queries now (`chimera_core::query_cache`, the party frame-rate pass):
  25% after ten, 28% less from the start (docs/performance.md, "Map
  changes that slowed the tick"). Bots cost about linearly (1.3M
  instructions a tick each at 16), most in Combat Arms' animation.
- **Match end: everyone stops: fixed, confirm in a playtest.** From the
  winning kill every player's and bot's input keeps only the look (server
  side, `core::FreezeTime` for prediction and bots); the win callout
  shows for ~5 s, then the match-end screen (second pass item 4).
- **A new, random ladder on every map: fixed, confirm in a playtest**: Ultimate Gun Game's ladder
  comes from a fixed seed (`SEED`), so every launch plays the same guns,
  and it is made again only by `ugg_new`. Pick a fresh seed at start
  and a new ladder at every map change (show the seed; `ugg_new <seed>`
  replays one); draw from every gun the players' games have (all of
  Combat Arms' weapons too, not a short list); mix in grenade rungs at
  the higher tiers, with enough grenades to get a kill. Done: a fresh
  seed at every launch (said on the console), the next seed's ladder at every rotation, guns
  from every gun each game has (Combat Arms' launchers, crossbow and its
  install's weapons by the `class` in their data), one or two grenade
  rungs (HE, M67) in the top third, the grenade back after each throw
  (`minigame::auto`, `ladder`).
- **A random map every match, from a broad pool: fixed, confirm in a playtest**: the next map is
  picked at random from every installed map gun game can run on (CS:S
  and Combat Arms maps with enough spawns), skipping the last 10 played
  (as many as the pool allows; never the same map twice in a row); the
  host keeps the history for the server's lifetime. Goes with the
  random ladder above. Done: `rotation = "random"` skipping the last min(10, pool - 1) (`minigame::rotation::MapHistory`); the
  pool adds CS:S's installed `de_*`/`cs_*` maps and Combat Arms' lobby
  maps (Junk Flea, Dark Hills, Blood Money; not Oil Rig).
- **Combat Arms characters can't climb a ladder from the front: fixed,
  confirm in a playtest** (cs_office). A Source map's ladder brush is
  solid, so Combat Arms' (round) hull stopped a skin short of its face and
  its box never touched the brush's `Ladder` volume; at a slant only the
  box's corners poked in, so the sides worked. The volume made from a
  ladder brush now reaches 2 Source units past its side faces
  (`combat_arms` `volumes::ladder_from_brush`;
  `movement_volumes::a_solid_map_ladder_brush_is_climbed_from_any_side`,
  party_kit `heavy::css_map_moves` on cs_office's and de_nuke's ladders;
  de_train's checked once).
- **Three level-up banners stacked at a rung up: fixed, confirm in a
  playtest.** A kill that moved the player up a rung showed the shell's
  callout ("Level up: M92FS", "Level 2/10") and, under Combat Arms' HUD,
  that HUD's minigame notice ("Rung up! 2/10: M92FS") over it. Combat
  Arms' HUD now draws the callout itself in its notice looks and place,
  with the shell's hidden while it is up (`client::hud::GameDraws::
  callout`; its notices are no longer drawn there, the callout says the
  same); CS:S's HUD keeps the shell's, and its minigame panel no longer
  lists the notices either (its "Rung up! 3/10: <gun>" was a second
  banner at the top). Both callouts now sit under the announcer's kill
  banner (37 % down; "2 KILLS" covered "Level up" before). One callout
  on every frame of a swap either way (party_kit
  `hud::a_rung_up_across_games_shows_one_level_up_callout`).
- **Two kill feeds on an HL2:DM rung: fixed, confirm in a playtest**
  (the user's recording, 2026-11-12). With HL2:DM's HUD up the shell's
  feed had HL2:DM's own death notices (orange, skull) under it: HL2:DM's
  HUD fed them from the shell's entries without checking the party's
  flag. Every game's own feed now passes one gate
  (`kill_feed::OwnKillFeed`) and declares itself (`GameKillFeeds`); the
  party's tests (`kill_feed::with_every_games_hud_a_kill_is_one_line`,
  `every_party_game_with_a_hud_declares_its_own_feed`) count every feed
  on each game's HUD. Plain HL2:DM keeps its notices.
- **One generic kill feed in the party apps: fixed, confirm in a
  playtest.** A kill made just before a rung swap showed only in the
  previous game's feed, and the two games' feeds sat on opposite sides.
  The entries are now Chimera's (`client::kill_feed::KillFeed`, from
  `core::Died` on host and clients, in no HUD) and the shell draws one
  feed at the top right with each gun's own icon (`WeaponIcons`, its
  game's headshot mark) or name; the party apps set `SharedKillFeed`,
  which empties CS:S's and Combat Arms' own feeds. Plain CS:S and Combat
  Arms keep theirs.
- **Hide the radars in the party apps: fixed, confirm in a playtest.**
  Combat Arms' radar over a CS:S map's overview matched neither style,
  and many maps have no overview. `hud_radar` (`client::hud::Radars`)
  starts at 0 in the party apps; `hud_radar 1` brings CS:S's radar and
  Combat Arms' corner map back. Plain games unchanged.
- **Combat Arms' radar draws as two boxes: fixed, confirm in a
  playtest** (spec corrections 2026-10-28, 1 and 2). The frame `bg.dtx`
  (256 x 256, outline 187 x 187 at texels 35..221 x 34..220) is a
  227 x 226 control at (782, -30) showing its top-left texels at one texel
  per ref px, so the outline lands on pixels 817/1003 across and 4/190
  down at 1024 x 768, around the window. Drawn as the whole image at
  (782, -30) (the same picture); the zoom track is cropped to its top
  192 rows, not squeezed. The map picture's texture window outside the
  image now clamps to the edge texel (`clip_quad.wesl`), as the spec says
  the stock game does; confirm by screenshot at a map corner (Warehouse
  spawn: blank by the transparent margin; an opaque-edged image such as
  `map_lastvegas` streaks). A test checks frame, map quad and window at
  1024x768, 1920x1080 and 2560x1440.
- **Headshot on a kill streak plays two voices: fixed, confirm in a
  playtest** (spec corrections 2026-10-28). One kill raises one event by
  the spec's priority (first kill, revenge, then at count 1 headshot or
  groin shot, else the step) with at most one voice (the event's, else
  headshot, else groin by the killer's body); the skull or nut goes over
  a step, first kill or revenge, not over a single kill's own HEADSHOT;
  levels: steps and head/groin 5, first kill and revenge 6; the groin
  voice follows the killer's body (`KillFacts::killer_female`). The plain-kill
  "KILL" / "N KILLS" banner is in (spec B2a; pack `plain_kill`, the
  server's per-map kill total in `KillFacts::total`, per-digit images in
  three fixed slots, level 1; silent, the install has no `OneKillSound`
  file); the number is the server's count, not a scoreboard entry. Not
  done: VIP kill, ice crush, lucky shot, side log, co-op digits.
- **Combat Arms scopes don't fill the screen: fixed, confirm in a
  playtest.** Per the spec (weapon_classes.md 7.10) Combat Arms does NOT
  stretch the picture to the screen: it is a card of `ZoomSpriteScale`
  (default 0.001) times its texels, scaled by the screen's height through
  the view-model camera's 50 degree FOV, centred, cropped by the screen,
  with the world showing where it is transparent (no black bars). The
  stock 1024 x 1024 lens is about 94 % of the height, and at 1920 x 1080
  thin world strips (about 175 px) show at the sides. Compare in real
  Combat Arms with the L96A1 at 1920 x 1080: if its scope really fills
  the screen there, the stock `ZoomSpriteScale` is larger (the spec
  session couldn't read it: Open question 4) and the per-scope values
  want reading. The range readout stays a stretched HUD item.
- **CS:S guns left no bullet holes on CS:S maps** (de_dust2; Combat
  Arms guns' did): the party loads CS:S's maps for Combat Arms'
  gameplay, so their kit was stripped (`GameKit::stay`) and its impact
  decals never loaded (`KitLoad::Only`/`Players`), while the impacts
  stayed CS:S's to draw (`MapData::impacts`): nothing drew them. Combat
  Arms maps had them as a guest's (`dress_guest`), hence "after a map
  change". Now the impact decals load with any kit and stay with the
  map. Fixed, confirm in a playtest.
- **A Combat Arms character can't break glass by running into it: fixed,
  confirm in a playtest** (cs_office windows). A whole window stops
  either game's character (spec: only a broken one shatters by touch);
  once a bullet cracked it, CS:S's movement (which sweeps the map's
  brushes) walked through and shattered the panes, but the cracked
  window's pane collider was on the default layer (the logic bridge put
  it back every tick), so Combat Arms' movement (which sweeps colliders)
  was stopped by it. The panes are now for shots only
  (`core::TRACE_ONLY_LAYER`, `source_logic` bridge and
  `map::breakables::update_panes`), so any game's character runs through
  and shatters them (party_kit `cross_glass`, `heavy::css_map_moves`).
  Left: break-on-touch glass reads this tick's velocity for the spec's
  "smoothed speed", so a CS:S player (velocity clipped at the glass)
  doesn't break it; a Combat Arms one does (no such glass on cs_office).
- **A bot stays invisible, as either game's character: fixed, confirm in
  a playtest.** A player dead at a map change (in the gun game, always the
  victim of the winning kill, since that shot ends the match and the next
  map loads) lost its ragdoll with the old map (ragdolls are map parts;
  characters stay) but kept `Ragdolled`. With that mark set,
  `render::bodies::attach_bodies` never gave it a body again, for either
  identity, and it never left a ragdoll again either. `remove_ragdolls`
  (`chimera_world::map::ragdoll`) now clears the mark from a living owner
  that has no ragdoll left. Tests: cs_source's `drawn_parts::
  a_character_dead_at_a_map_change_is_drawn_again`, and party_kit's
  `bodies_drawn::every_living_player_stays_drawn_through_swaps_deaths_and_map_changes`.
  That one runs ten players through random kills, identity swaps,
  ragdolls, respawns and six map changes, checking every tick that each
  living player has one mesh-drawn body of its identity's game, with
  nothing above it hidden, on the main layer, and no leftover mark. It
  failed at the first map change before the fix. Ruled out by reading
  the code: CA's hide-while-dead (`object_visibility`, it shows the
  object again regardless of identity), the spectator's in-eye hiding (it
  shows the old target again when the target changes), the local body's
  hiding (`LocalPlayer` only), and PVS culling (characters aren't culled).
- **CS:S's minigame panel drew over the match-end screen: fixed,
  confirm in a playtest** (promo capture). The panel left out only its
  results and notices while the screen was up (`MinigameHud::
  results_shown`), so its title, rung, next gun, leader and "Match over"
  status lines still sat over the screen's title and winner. Both games'
  panels now draw nothing while it is up (cs_source's `minigame_hud`,
  Combat Arms' `minigame_view`). The screen also scales with the
  window's height as the kill feed does (sized for 720 lines; its
  standings were 20 px at 1080; it shrinks when the rows would not fit). Tests: party_kit's `results_hud`,
  chimera_client's `match_end::the_screen_scales_with_the_windows_height`.
- **Kill feed names instead of icons for Combat Arms guns: fixed, confirm
  in a playtest** ("[Ironwood M1866 lv.4]"). The icon was looked up by
  the id's name (`kill_message_<id>.dtx`) and decoded for our own
  weapons only, so the install's copies (2240 of 2298 guns, the
  ladder's most) had none. Each install weapon record names its icon
  (`Weaponkillimage`: copies share their model's art, some files are
  spelled otherwise or sit outside `weapon/`); `ui::install::kill_icons`
  reads it and every weapon's icon is decoded, used by the shared feed,
  Combat Arms' own feed and death screen. 2286 of 2298 now have one
  (the rest: kits, smoke and flash grenades, a missing snowball file).
  The text fallback is the plain name, no brackets. Tests: combat_arms'
  `heavy::kill_icons` (install), `kill_feed::tests` in chimera_client.
- **Grey Combat Arms gun skins: fixed (animated since 2026-10-30), confirm in a
  playtest** ("Prismatic Eco M416", "Drakeveil SIG550", "Dragunov SVD
  Obsidian Dragon" (flat), "M14 Cyanide Echo lv.4(limit)", "Red
  Dominion ARX60 Lv.1 (limit)", "SIG50 Clockwork" (body grey, scope
  textured)). Their `PVSkin#` / `HHSkin#` name a sprite (`.spr`, an
  animated texture sequence: world_dat.md), e.g.
  `GUNS_T_PV_AR\PV_AR_RINBOW_M416.SPR`, `PV_AR_ID_SIG550_BC.SPR`,
  `PV_SG_VARIANTS_DRAGUNOV_SVD.SPR`, `PV_M14REDLIGHT.SPR`,
  `PV_AR_ARX_160_LINE.SPR`, `PV_SIG50_LAVA00.SPR`; our skin lists kept
  `.dtx` names only, so those pieces had no texture and drew the
  untextured grey (an attachment names a `.dtx`: the Clockwork's scope
  showed). A sprite skin now draws its first frame, as map surfaces and
  effects already did (`data::skin_paths`); since 2026-10-30 it plays
  its frames as models_ltb.md "Sprite skins" says (each drawn copy's own
  looping clock, ⌊1000/r⌋ ms a frame, swapped whole:
  `world::model_textures`, Chimera's `render::skin_frames`): 315
  animated pieces in `heavy::weapon_skins` on the dev box. Same
  for both games' boxes (a naming rule, not a key). The sweep of all
  2,298 install weapons (`heavy::weapon_skins`, view and held model,
  each piece's first texture slot) on the dev box's install:
  - sprite skins, drawn grey before, now their first frame: 310 pieces
    (the Prismatic Eco AK47/M416/AUG, Obsidian Gild AN-94, Drakeveil
    CZ805/SIG550/SIG50 Lightning, ACR and SVD Obsidian Dragon, Red
    Dominion ARX160 Lv.1-4 and limits, M14 Cyanide Echo, SIG50
    Clockwork, Weaking Dragon AR-15, UMP-45 Neon Vandal, Cyber ACR, AK47
    Pinkstatic, Knife Gradient, ...);
  - still grey as the data says (parity: the game leaves an unresolved
    skin empty too, textures_dtx.md): 9 skins name a file that isn't
    there (SIG50 Samba Queen's scope `GUNS_T_PV_AR\PV_OPT_DT_SCOPE_
    SAMBAQUEEN.DTX` is in `GUNS_T_PV_SR`; AA-12 Holy Fire's held, G36C
    Orange Radiance's held, MK18 MOD1 camo, Retro Summer P90TR, Advanced
    star(shing) M4A1's sprite frame, the bow controller), 6 pieces of
    Lightning star FMG-9's view name a sprite that isn't there, 13
    pieces' texture slot is past their skin list, 14 held/view models
    name files that aren't there (`HH_M67.LTB` for the infected bombs,
    `HH_M4CQB.LTB`, `HH_CLAYMORE.LTB`, the minigun);
  - drawn but maybe not as the game: 62 pieces with a second texture
    slot (knives, power suits, infected hands: not drawn), and render
    styles our table doesn't know, drawn opaque: `RS\ENV_ALPHA_MASK.ltb`
    (12: power suits, swords), `RS\ItemEdge.ltb` (6), `RS\envmap.ltb`
    (2) (open questions 2, 3).
  The Neon Outlaw ASW338LM and AUG A3 Champions Lightning skins: see "Neon
  Outlaw ASW338LM" above (the data, not our reading).
  Tests: `attribute_data::sprite_skins_and_an_added_weapons_held_look`,
  `model::a_sprite_skin_draws_its_first_frame`; the sweep:
  `cargo test -p combat_arms --features dev,ui --test it
  heavy::weapon_skins -- --nocapture` (`CHIMERA_SKINS_SHOW=a,b`: every
  piece of the guns so named).
- **Combat Arms install guns had no gun in third person (the R870):
  fixed, confirm in a playtest.** A map carries the held models of our
  own weapon documents only; the install's 1,981 copies (the ladder's
  most, e.g. the 11 R870 variants: R870 MCS, R870SE, ...) had none, so
  characters' hands were empty and a dropped one never drew, and their
  look (skins, render styles, animation class) was the default. Our
  `combat_arms:remington_870` does load with the map's kit (its
  `HHModel` resolves and is textured); on a map whose kit doesn't carry
  ours it now loads the same way. Their held model now loads in the
  background the first time a character holds one or it lies loose
  (`characters::held_on_demand`, Chimera's `map::add_held_model` /
  `render::bodies::build_late_held`, kept across map changes), and every
  install weapon takes its `HHSkin#`, `HHRenderStyle#` and `AniType`.
  Tests: `held_on_demand::tests`, chimera_present
  `bodies::late_held_models_are_built` and the attribute test above.
- **Push timing and other derived values from the server: fixed, confirm
  in a playtest.** Decision 2026-10-29: help clients by pushing from the
  server where they would desync. A client that couldn't read a Combat
  Arms copy's view model (no V2 key, a missing file) or read another
  version fired, reloaded and drew on other numbers than the server and
  mispredicted. Now a weapon's server-decided parameters go to a client
  once per weapon id per connection (Chimera's `weapon::params`,
  chimera_net's `net::weapon_params`, `WeaponParams`, net32) and its
  weapons of that id take them over their own, rebuilt ones too; Combat
  Arms' timing (`combat_arms:timing`, `CaTimingValues`) is the first.
  The audit of other values a client derives from its install is in
  docs/plans/active/multiplayer.md "Server-pushed weapon parameters"
  (the weapon registry and the attribute file's numbers are the open
  risk). Tests: `weapon_timings::a_client_without_the_models_predicts_on_the_servers_timing`
  (0 prediction errors over a burst, sent once per id, rebuilt weapons
  keep it), `a_client_with_its_own_models_takes_the_servers_timing`.

### Gun game playtest, 2026-10-25 (user; start after the usage reset)

- **HE grenade on a window, black blobs: fixed, confirm in a playtest**
  (promo capture 2026-10-28, cs_office, Ultimate Gun Game). Three causes,
  none in the glass's material: (1) the party apps never drew CS:S's
  grenade looks (only CS:S's own game added them), so the HE blew the
  panes out with no fireball or smoke; `PartyPartsPlugin` now adds
  `cs_source::game::grenades::GrenadeLooksPlugin` (CS:S's and no game's
  blasts; `grenade_looks::*`). (2) The "black blobs with red specks" were
  the blood burst's goop and chunks, made by blast damage at the
  thrower's own centre, in front of his eyes; blood now comes from bullet
  and knife hits only, as impact_effects.md 10 says
  (`impact_effects::blasts_do_not_bleed_bullet_hits_do`). (3) A blast's
  whole-window shard burst was capped at 48 specks; now
  `MAX_PANE_SHARDS` = 160 (`a_blasted_window_throws_a_full_burst_of_glass_shards`).
  The panes going in one tick is the spec's blast rule. Plain CS:S
  showed (2) and (3) as well.
- **Combat Arms guns can't headshot a CS:S player (2026-10-28): fixed,
  confirm in a playtest.** A Dragunov round on the drawn head box did no
  damage; only the neck box (CS:S's head group) counted. Cause: Combat
  Arms' rounds test a character's box (its movement hull) before its
  hit shapes, as its spec does for its own bodies, whose spheres sit
  inside it; a CS:S model's head box is centred at the hull's top (on
  de_dust2 4 mm above it), so a round at the head missed the box and
  never reached the shapes. Now the box holds another game's shapes
  (`Hitboxes::bounds`, `weapons::box_holding`). The Combat Arms knife
  (and grenades) also tested Combat Arms' own skeleton, which keeps
  being posed out of sight under a CS:S body (why its name tag flinched
  on a knife hit); they now test the worn body's hitboxes
  (`weapons::their_shapes`), and Combat Arms' name tag floats over that
  body's head. Tests: `cross_game::a_combat_arms_round_reaches_a_cs_source_
  head_above_the_hull`, `..._knife_hits_a_cs_source_body_by_its_boxes`,
  `a_swap_leaves_only_the_current_identitys_hitboxes`, and on the real
  models `heavy::cross_hits`. Not changed: the CS:S model plays no flinch
  on a hit (our CS:S bodies have none, for any gun).

- **Combat Arms grenade throw visuals: fixed, confirm in a playtest**
  (promo captures 2026-10-27, M67 on de_dust2). The red arc and red edges
  were the damage overlay lit by hits that lost no health (the capture
  ran under `god`): the HUD read the damage dealt on a listen server; now
  only what reached health (`DamageTaken`; `hud::
  hits_without_health_loss_show_nothing`). The blue and pink square in the
  blast was a refraction key (`EffectShaderID` 100, a distortion map)
  drawn as colour; keys with an effect shader are now left out
  (`effects_sounds::shaded_keys_draw_nothing`). The big sleeve mid-throw
  is the stock throw animation, not a blend or skeleton fault: every node
  matches the game's own sampling and the skinned arm stays intact
  (`heavy::view_model::m67_throw_poses_follow_the_data`); the right
  upper arm sweeps past the eye and its open sleeve end shows for a few
  frames. If it still looks wrong, compare with Combat Arms itself.

- **Combat Arms character models and held guns: fixed, confirm in a
  playtest.** Heads looked thin and the head and backpack vanished up
  close: the body parts were culled by their bind pose's box, and the
  female bodies are bound standing on their origin, half a body above
  where the animation draws them, so the hair, backpack and upper body
  dropped out while still on screen (leaving the narrow face mask); now
  each part's bounds follow its joints (`render::bodies`,
  `body_bounds::body_parts_are_culled_by_their_posed_bounds`). The
  third-person guns were drawn in their bind pose; each `HHModel`'s own
  `Hand` animation turns it half a turn, so they pointed backwards; now
  they are drawn in that pose (`heavy::characters::
  held_weapons_point_forward`; one level per piece:
  `bodies_draw_one_level_per_piece`).
- **Clients don't see glass break properly: fixed, confirm online.** A
  broken window's panes (func_breakable_surf) stayed whole on clients and
  no shards, gibs or falling panes reached them (only a func_breakable's
  removal and the break sound did); now the panes replicate and what
  breaking throws is sent (`net::breakables`; guarded by
  `cs_source net_breakables`).
- **Combat Arms full-auto fire clips (painfully loud): fixed, confirm by
  ear.** The fire sound was already one voice per shooter (its fire
  channel); the pile was every round's impact and casing sound, a new
  voice each at almost full gain up close (Combat Arms' falloff is nearly
  flat near the source): 18 voices at once in 3 s of M4A1 fire with
  second-long stand-ins, where CS:S plays no casing sounds and its
  impacts fall off faster. Now each gun's impacts and casings replace
  their previous one (`effects::sounds::CHANNEL_IMPACT`/`CHANNEL_SHELL`),
  and Chimera's playback limits voices per source (8) and per sourceless
  entry (4) (`fire_voices::full_auto_keeps_a_few_voices`). Check in a
  playtest; if it still peaks, a limiter on the mix is next.
- **Your killing shot with a Combat Arms gun is silent: fixed, confirm
  in a playtest** (user, 2026-10-28, Ultimate Gun Game). The ladder swaps
  the gun on the kill's tick (`minigame::runtime::take_weapon` despawns
  it), and Combat Arms plays fire sounds (and impacts, tracers, casings)
  in `Update` from the round's events by looking up the named gun, so
  where the match runs (single player, listen server) the killing round
  found no gun and drew nothing; the earlier fix (Chimera's `FireSounds`,
  `Fired` carrying the weapon ID) covered CS:S's sounds and the network
  only. Now Combat Arms' effects keep what each gun was past its despawn
  (`effects::KnownWeapons`, by entity and, for others' rounds a client
  hears after their gun changed, by ID). Tests: `party_kit last_shot::`
  (host, victim, network clients), `combat_arms effects_sounds::
  a_round_whose_gun_was_swapped_still_sounds`; CS:S's guns:
  `chimera_sim minigame::the_shooters_own_killing_shot_sounds_once`.
- **Combat Arms grenades: fixed, confirm in a playtest** (user,
  2026-10-27; nothing left). The flight matched grenades.md (arc, bounce,
  rest, fuse: `grenades::t01`-`t49`) but was drawn at the server's 20
  frames a second; now it is drawn on its path between frames
  (`grenades::draw_between_frames`), with the projectile record's model
  in its `World` pose (`data::grenade_values`,
  `heavy::characters::thrown_grenades_draw_their_projectile_model`).
  The spec's sounds, spin and pose (grenades.md Corrections (2026-10-27)
  C1-C3, cases 64-72, `grenades::t64_`-`t72_`): the pin pull plays the
  record's `MiscSound1` (view-model keys 10-14 are `MiscSound1..5`,
  loaded by `data::sound_values`; others hear it 3D at the thrower's
  flash point, radius 2000); the throw call-out "GrenadeThrowing" goes to
  the thrower's team (`grenades::callout`, over the network as
  `CaThrowCallout`); the tumble is set from accumulated angles and a
  client smooths it (`grenades::tumble`). Known shortcut: the call-out
  does not follow the thrower once it has started (Chimera's one-shot
  sounds fix their gain at the start).

### Gaps vs the originals (audit 2026-10-27)

specs/ compared with the code, spec by spec: plain CS:S and Combat Arms
matched their specs wherever the docs don't already say otherwise; the
gaps found were where the party apps put the two games together (sounds
and HUD parts by identity, hits and armour across games: DESIGN.md "Hits
across games", water and ladders across movements) plus a few low items.
All fixed 2026-10-27; confirm in a playtest. What no spec covers is in
the second pass below.

### Gaps vs the originals, second pass (2026-10-28)

What a player of the gun game (Ultimate Gun Game, Lucker Party) or of
plain CS:S and Combat Arms would notice, where no spec covers the
behaviour, so the first pass could not find it. From the code and docs
and general knowledge of the games; most noticeable first. Items marked
(spec first) depend on exact original behaviour or data; the rest are
our design. Not repeated here: what parity-gaps.md, tech-debt.md or the
sections above already list.

- **Gun game: no sound or banner for a rung up or down, the last rung,
  a new leader or the win: done (checked against the code 2026-10-31),
  confirm in a playtest.** Gun game mods on CS:S servers play a
  level-up and level-down sound, name the new gun big, call the leader
  and the last level to everyone and play a winner sound; Combat Arms'
  Arms Race has its last-stage call-out and music. Ours writes
  `MinigameEvent`s (`rung_up`, `rung_down`, `match_won`) that only the
  HUD panels read as small text notices, and only the viewer's own rung
  (`minigame::ladder` `notice`); no leader-change or last-rung notice to
  others. Fix: a gun-game announcer pack (Chimera's announcer is data,
  DESIGN.md "Announcers") or Arms Race's cues, which our Arms Race keeps
  but doesn't show (`rules/arms_race.rs`).
  *Fixed, confirm in a playtest (2026-10-28):* the ladder also sends
  `last_rung` and the lead's `lead_taken`, `lead_lost`, `lead_tied`
  (`minigame::ladder`, replicated like the others); every client's HUD
  data turns the local player's moments into a centre-screen callout
  ("Level up: <gun>", "Knifed! Level down", "Last level!", "<name> is on
  the last level!", "You took the lead", "You lost the lead", "Tied for
  the lead", "You win!", "<name> wins!"), which the shell draws on every
  HUD (`client::callouts`) with a sound from a cue pack (`core::cues`,
  data; CS:S lends stock install sounds, `cs_source/data/cues.toml`; a
  definition's `cues` picks one or `none`). Arms Race's own last-stage
  cue is plain Combat Arms' (its interface sound already plays,
  `music::director`; its text and the files of Combat Arms' level-up and
  result sounds aren't known: music_ambience.md Open question 1), so
  Combat Arms lends no cue pack yet and plain Combat Arms is unchanged.
  (Since then Combat Arms lends one: Arms Race's interface sounds, "No
  music in the party's matches" below.) *2026-10-31:* the shell's callout
  was a fixed 44 px, small on a large screen: it is sized for 720 lines
  and scales with the window (`callouts::line_size`), in the HUD's face
  (CS:S's centre-print Trebuchet MS, `fonts::overlay_font`), its band
  shaded as a display-space blend (`widgets::display_alpha`); test
  chimera_client `callouts::the_callout_scales_with_the_windows_height`.
- **Gun game: the death view fights the 2 s respawn, and there is no
  countdown: done (checked against the code 2026-10-31), confirm in a
  playtest.** The dead player gets their identity's death camera: CS:S's
  2 s death cam, 0.7 s travel and 5 s freeze (`client::spectate`,
  `ui::freeze_cam`) or Combat Arms' 3.5 s pull-back (`game::death_cam`),
  but the minigame respawns after `delay` 2 s (`runtime::respawn_dead`),
  so the freeze cam never freezes and the pull-back is cut off; neither
  `minigame::hud` nor any HUD shows when you come back. Fix: a short
  party death view (killer's name, gun and health) sized to the respawn
  delay, and a countdown.
  *Fixed, confirm in a playtest (2026-10-28):* the minigame tells the
  shell when the dead local player comes back (`core::RespawnDue`, from
  the definition's `delay`); the shell fits CS:S's death and freeze cams
  into that wait (`spectate::fit_to_respawn`: 0.8 s death cam, 0.3 s
  travel, 0.9 s frozen picture with 2 s) and shows "Respawning in N" on
  every HUD (`client::respawn_countdown`). Combat Arms' pull-back is
  kept: its spec's 2 s reference wait also ends it early, after the
  camera has settled (0.7 s). Its camera and death screen now show only
  while Combat Arms' HUD is up, so a CS:S-gun death gets CS:S's view.
  Plain CS:S and Combat Arms set no `RespawnDue` and are unchanged.
  *2026-10-31:* the countdown was a fixed 28 px (small at 1800 lines): it
  scales with the window in the HUD's face (`respawn_countdown::SIZE`;
  test `respawn_countdown::the_countdown_scales_with_the_windows_height`);
  Combat Arms' death screen named the killing gun by its id
  (`m136_at4allstar`), now by its display name (`hud::death::name_weapon`;
  test combat_arms ui `death_screen::the_killing_gun_shows_by_its_display_name`).
- **Kill feed and death screen show no weapon for the other game's
  guns. (Fixed 2026-10-28, confirm in a playtest: each game registers its
  icons in Chimera's `client::weapon_icons::WeaponIcons`, CS:S's font
  glyphs and Combat Arms' install textures, and the other HUD's feed
  draws them, else the gun's name; the Combat Arms death screen shows
  the name alone, no icon, for a CS:S gun.)** In the party half the kills have an empty weapon slot: CS:S's
  death notices give another game's gun no icon (`ui::game_hud::
  kill_icon` returns none), Combat Arms' feed and death screen look up
  art in its own install by id (`ui::install::kill_icon`), which a CS:S
  gun lacks. Fix: each feed draws the gun's own game's icon (CS:S's font
  glyph, Combat Arms' texture), else the display name.
- **Match end is a few lines of text: done (checked against the code
  2026-10-31), confirm in a playtest.** The results replace the notices
  in the minigame panel (`minigame::results`, `hud::MinigameHud::
  results`): no full-screen winner or scoreboard, no winner sound or
  music, no "next map" line, no map vote (minigame.rs shortcut), and the
  Tab board is plain text without colours (`minigame_scoreboard`).
  Combat Arms' own result screen is specced (lobby.md G) if the party
  wants its look.
  *Fixed, confirm in a playtest (2026-10-28):* the win pauses the game
  for everyone: from the winning kill through the match-end hold every
  player's and bot's intent keeps only its look (`minigame::runtime`'s
  end freeze, the server's, with `core::FreezeTime` so a client's
  prediction holds too). The shell draws a
  match-end screen over every HUD for the match-end hold
  (`client::match_end`, from the round's end, after the win's own
  moment, until the next match): the
  winner big, the final standings in the Tab board's columns (place,
  name, kills, deaths, final gun, matches won) with your own row
  highlighted, the next map and a countdown to the change. The server
  picks the next map as the hold begins (`minigame::NextMatch`,
  replicated in `NetMinigame` with the results; each row's character is
  mapped so a client finds its own row). A winner sound plays as it comes
  up, a file from an install named by data (`GameParts::
  match_won_sound`: CS:S's a bot cheer; Combat Arms' `ResultWinSound`
  file isn't in a spec yet), none if missing. Plain CS:S and Combat Arms
  have no minigame and show nothing new. Follow-up: a map vote on that
  screen (the server's rotation picks the map meanwhile), and the Tab
  board's colours.
  *2026-10-31:* its shades blended in linear light left the world at
  ~65% and the standings' panel see-through; they now cover as
  display-space blends (`widgets::display_alpha`), the text is in the
  HUD's face (CS:S's Trebuchet MS; the winner and the highlighted rows
  bold), your row white on a gold band (gold on gold read poorly), and
  the Deaths and Gun columns no longer touch. Not done: the map vote (not
  small: the vote's rules, the candidates and their network part) and the
  Tab board's colours.
- **Spawn protection can't be seen: done (2026-10-29, the user's
  decision: 2 s, see-through, shot through, firing ends it), confirm in
  a playtest.** A protected player (`runtime`
  `Protected`, through `core::God`) looks like anyone else, to them and
  to others, so shots on them read as lost hits. CS:S has none of its
  own (deathmatch plugins tint the player); Combat Arms' look for a
  protected player, if it has one, is in no spec (spec first for that
  look; the party's own marker is ours).
  *Fixed, confirm in a playtest (2026-10-28):* Combat Arms' style, as
  game_modes.md's Corrections (2026-10-28) specify it: an absence. The
  minigame's protection has the spec's two windows (`protection =
  { damage = "4s", bullets = "3s" }`; the gun games keep their 1.5 s for
  both, the first shot ending it: `core::SpawnProtected`); a hit on a protected player, from
  any game's gun, shows nothing on the host or any client (no blood,
  puff, decal, impact sound, hit indicator, name-tag flash or reaction:
  `weapon::impact::hide_protected`) and deals nothing, and the windows
  replicate on each client's clock (`minigame_net::NetProtected`), so
  there is no blood without damage. The visible marker came after
  (2026-10-29, the user's): the protected body draws see-through (see
  "Spawn protection: 2 s, see-through, shot through").
- **Ladder rules players expect from gun game: done (checked against
  the code 2026-10-31), confirm in a playtest.** No cost for a suicide,
  a fall, `kill` or one's own grenade (gun game mods take a level, so
  `kill` isn't a free reset); a late joiner starts on the first rung
  (mods give the lowest or average rung of the others); no grenade or
  knife-only last rungs (the classic HE-then-knife finale). Data options
  on `chimera:ladder` (a `demote_on` list, a join rung, rung kinds).
  *Fixed, confirm in a playtest (2026-10-28):* `chimera:ladder` settings
  (DESIGN.md "Gun game rules"): `demote_on_suicide` (default on) takes a
  rung for a suicide (own grenade or blast, `kill`) or a death with no
  killer (a fall, the world), never below the first, with a "Suicide!
  Level down: <gun>" callout (event `suicide_down`, the knifing's cue);
  nobody's kill, so no streak. `join_rung` (default `lowest`; `average`
  rounded down, `first`) puts a late joiner on the others' lowest rung,
  never the last. `knife_finale` (default off; true: the kit's knife, or
  a weapon id) adds a knife-only last rung whose kill wins. The knife
  kill's stolen rung (`demote_on = "melee_death"`) stays. Grenade rungs
  were already possible (Ultimate Gun Game's). Combat Arms' Arms Race
  keeps its spec's rules (no suicide cost); plain CS:S and Combat Arms
  are unchanged. Since then the knife finale is on by default and its
  melee rungs scale with the ladder (2026-10-30, "Melee finale on by
  default" above), and a power-curve ladder ends on a grenade rung, then
  melee (the HE-then-knife finale). Tests: chimera_sim
  `minigame::a_suicide_or_a_fall_costs_a_rung_but_never_below_the_first`,
  `a_late_joiner_starts_on_the_lowest_rung_the_average_or_the_first`,
  `a_knife_finale_is_a_knife_alone_and_its_kill_wins`.
- **Loudness and hearing range differ by game.** Each game's guns,
  footsteps and impacts play at their own levels and falloff (Combat
  Arms' nearly flat up close, CS:S's own curves), so one game's players
  are heard louder or farther than the other's: footsteps give away one
  identity more. Chimera's mix has no per-game balance. A listening pass
  with both games' guns and steps at set distances, then a per-game gain
  (and a footstep range option).
  *Opt-in setting added (default off), confirm in a playtest
  (2026-10-28):* measured for others' steps as our mixer computes them
  (apps/party_kit/tests/it/footstep_levels.rs, gain = volume x
  `distance_gain` at 5 / 15 / 30 m): CS:S identity running 0.50 volume,
  0.40 / 0.21 / 0 (linear, CompatibilityAttenuation 1.0, silent from
  25.4 m), its jump step 1.0: 0.80 / 0.41 / 0; Combat Arms identity
  running, sprinting and landing all 1.0 volume, 0.94 / 0.45 / 0 (radius
  1.5 to 21.6 m, silent from 20.1 m); walking silent in both (CS:S's
  bands, Combat Arms' walk animation has no step keys). So Combat Arms'
  steps are over twice as loud close by and CS:S's carry about 5 m
  farther. `party_even_footsteps 1` (the server's, replicated) gives
  Combat Arms' others' steps CS:S's profile (`map::sound::EvenFootsteps`:
  its level, 0.2 walk, 0.5 run or sprint, 0.325 crouched, 1.0 a jump's
  or landing's step; perks still scale it), both then 0.40 / 0.21 / 0
  running. Guns and impacts keep their games' levels.
- **Hit feedback differs by the gun in hand: fixed (opt-in), confirm in a playtest.** A Combat Arms gun gets
  its hit indicator and name-tag flash (`ui::hud`), a CS:S gun nothing
  (parity), so CS:S rungs feel unresponsive next to Combat Arms ones.
  An opt-in party hit marker or hit sound for every gun (parity first,
  options later).
  `cl_hitmarker 1` (archived, default 0 everywhere) draws a cross on the
  shooter's own client when its round damages another player: white for
  a hit, yellow for a headshot, red for a kill (`client::hit_marker`,
  over any game's HUD; from `WeaponEvent::Hit`, which a network client
  gets as the server's `HitConfirm`, and `Died`). Nothing for the
  victim, for a hit that dealt no damage (spawn protection) or a prop,
  or where the HUD's game draws its own indicator (Combat Arms'). The
  shell's old always-on "x" is gone (CS:S has no hit feedback). No sound
  yet.
- **Dominations and revenge (plain CS:S, and CS:S identity)** (spec
  first). CS:S marks a player who kills the same enemy several times
  without dying to them as dominating (a kill feed line, the freeze
  panel's nemesis line, a scoreboard icon, a sound) and calls a kill on
  one's dominator revenge; nothing of it here (2b notes only the freeze
  panel's line). Needs the threshold, the sounds, the icons and who
  sees what.
- **No music in the party's matches: done (checked against the code
  2026-10-31), confirm in a playtest.** Combat Arms' music director
  takes its facts from Combat Arms' match (`game::music` reads
  `CaMatchView`'s mode and phase), which a minigame doesn't run, so the
  party plays no start track, round or result sounds, and Arms Race's
  last-stage cue can't follow the ladder's last rung (CS:S plays none in
  a match). Decide which cues a minigame gets and feed them from
  `MinigameHud`.
  *Fixed, confirm in a playtest (2026-10-28):* a minigame's moments
  (`minigame::moments`: the start of play, alive or not, the last rung's
  players, the respawn countdown, the result) go to the director of the
  game whose music the definition picks (`music`; default the first
  that lends one, `GameParts::music`: Combat Arms'), which plays
  music_ambience.md Section 10's cues: the loading track, the round
  start's sound and Arms Race's start track at the start of play if
  alive, the last stage's music while anyone is on the last rung, the
  respawn countdown's ticks, silence on the match-end screen. Combat
  Arms lends a cue pack (rung up and last rung: Arms Race's interface
  sounds) and its result screen's win and lose sounds, both chosen by
  the HUD up (the cue pack by `CuePacks::choose_for`, the match-end
  sound by `PartsRegistry::match_end_sounds`, losers hearing the lose
  sound). The files are inferred by name (Open question 1). Plain Combat
  Arms' own interface sounds still have no files (data/music.toml
  `[interface]`; the party's are `[party.interface]`): a follow-up could
  give it the same inferred files.
- **A rung's swap is a hard cut: done but character choice, confirm in a
  playtest.** Body, HUD and movement change on the
  tick (`identity`): others see the model pop from one game's character
  to the other's, the player's HUD jumps; no swap effect or sound. And
  no character choice: `character` in a definition is informational
  (minigame.rs shortcut), so nobody picks a CS:S class or a Combat Arms
  soldier as both games let them.
  *Opt-in swap effect added (default off), confirm in a playtest
  (2026-10-28):* `party_swap_effect 1` (archived, the party clients'):
  when the local player's game changes on a rung its screen flashes
  white and fades over 0.3 s while the new gun draws; another player's
  swap puffs a pale sphere at its body for 0.5 s (`client::
  swap_effect`); once per swap, nothing for a first identity or a dead
  player. *2026-10-31:* with it, the local swap sounds the cue packs'
  `swap` cue (CS:S's pack: `items/battery_pickup.wav`, ours; Combat
  Arms' pack: `UI_SND/VWRM/WEAPONRACE_CHANGE.WAV` since 2026-10-31; none with `cues =
  "none"`; test chimera_client
  `swap_effect::a_local_swap_sounds_the_swap_cue_once`). Character
  choice still needs the user's decision.
- **Balance across identities is unmeasured.** Speeds, sprint, hull
  height, jump, head size and one-shot ranges change with the rung's
  game; nobody has looked at whether one game's rungs are easier to win
  on.
  *Not wanted (user, 2026-10-28):* the games stay unbalanced against each
  other. Everyone gets every gun in turn, and a really bad or really good
  draw, and finding a way to get the kill with it, is part of the fun.

### Decisions 2026-10-13 (user)

- Refactor: go ahead with engine-split.md "Crates and compile time"
  steps 1-2 (tools package; move code to its future home in the one
  crate), then crates.
- Default binds follow CS:S's config_default.cfg (F1 autobuy, F2 rebuy,
  mouse wheel invprev/invnext, ...): move our debug keys (inspector, debug
  UI, wheel jump) off them. Done 2026-10-13: debug UI F8, inspector F11
  (`chimera_inspector`), collision F3 (`toggle chimera_drawcollision`; F3
  is free in CS:S); saved configs keep their binds.
- `con_enable` stays 1 (deliberate difference, ui-parity.md).
- SourceMod/Mani chat commands from maps stay ignored, as plain CS:S does.
  Later: consider making SourceMod plugins work on our engine (parity on
  community servers, good for users) — an investigation first.
- Networking: investigate whether Steam networking (relays/NAT traversal)
  is possible at all for our own client (expected not: it needs our own
  Steam app id; CS:S's can't be used), prototype if cheap; server size:
  as many players as we can make performant; stress-test it (parity
  first: 32 like CS:S as the first target). Done 2026-10-13: not
  possible without our own Steam app id; UPnP prototyped (`net_upnp`),
  recommendation in docs/plans/active/nat-traversal.md; 32 players fit
  the tick after a capacity pass (multiplayer.md "Capacity"), follow-ups
  in docs/tech-debt.md.
- No new release until the user says the group is testing again.
- Dark wall panels by "MAIN": skipped (map unknown). Skybox inside a
  player: waiting for repro details from the user.

### Playtest with friends, 2026-10-10 (Discord; build v0.1.0-playtest2 or 1)

- Water seen through other parts of the map (user, 2026-10-11,
  mg_jacks_multigames_v1, `setpos -12146.96 15343.41 -927.97; setang
  -29.57 184.04 0`): fixed 2026-10-13, to playtest. It was the flood's
  water (a func_water_analog, drawn under its mover's node) seen through
  the sky from another room: moving water had no visibility clusters, so
  it was never culled. It is now tagged by its moved bounds as brush
  entities are (`heavy::map_community::moving_water_is_culled_on_jacks`).
  Depth and pass order were fine.

- Done 2026-10-12: no weapon held draws no view model and the HUD's
  `crosshair_default` (five dots; `client::hud::crosshair_kind`).
  Check in a playtest that the T-posed hands are gone after a jacks
  strip (the single-player strip test shows no view model).

- Moving under a brush at head height (user, 2026-10-11,
  mg_jacks_multigames_v1 by the climb start: lamps on the pillar whose
  undersides are 64.7 units above the floor, over a 62-unit box with the
  eye at 64): fixed 2026-10-13, to playtest. The eye was kept under a
  ceiling found by a ray straight up from the box's centre, so a lamp
  over the box's top but not over the eye let the near plane cut into it;
  now a flat box as wide as the player's (15 units each side) is swept up
  (`client::view::below_ceiling`; `console_commands::eye_drops_under_a_brush_edge_over_the_box`).
  The movement spec doesn't cover the view against ceilings: our reading
  of the report. Capture to confirm: in CS:S, `setpos -12348.55 14205
  1408.03; setang 0 268.82 0` (feet on the floor, the box's top under the
  lamp's edge, the eye just short of it), `getpos` and a screenshot: does
  the view drop (by how much), or does CS:S do something else (duck)? The
  setpos in the report is a `getpos` line: it prints the eye, and setpos
  takes the feet (both games), so it lands 64 units higher, on top of
  the lamp.

- mg_jacks_multigames_v1 in real CS:S (user, 2026-10-11): done
  2026-10-13, to playtest (multiplayer.md, "mg_jacks_multigames_v1 in a
  network game"). (1) Each game's selection sound (`lj_sound` PlaySound,
  an ambient_generic) played only on the host: map logic's sounds now
  reach clients. (2) Its `say` lines ("<== Spawn: Surf == >", "[==> Map
  by Jackmaster <==]") show in chat, under the host's name on a listen
  server, "Console" on a dedicated one. Capture to confirm: on a CS:S
  listen server with the host dead, do living players read the map's
  lines (ours follow the host's dead-chat rule)? (3) `sv_airaccelerate
  750` (climb and surf spawn modes) is allowed and applied; ours puts it
  back when the map unloads (our rule; CS:S keeps it until a config sets
  it again). (4) Strips: StartTouch fires for anyone spawned or
  teleported into a trigger, and bots are stripped as players are
  (`heavy::map_flows::jacks_strips_everyone_between_games`); the
  "Normal" spawn mode goes to the lobby with no strip (the map's design,
  so survivors keep their guns there, in CS:S too).

- Done 2026-10-12: alt-tab back in grabs the cursor at once
  (`client::input::grab_cursor`).

- Weapons after a round on minigame maps (user, 2026-10-11): players walk
  into the lobby with e.g. a machine gun. In CS:S survivors keep their
  weapons into the next round; minigame maps strip them with map logic
  (`player_weaponstrip` Strip/StripWeaponsAndSuit or `game_player_equip`
  with the strip flag, fired by a trigger in the spawn/lobby or a
  logic_auto at round start). jacks has 4 player_weaponstrips fired by
  triggers' OnStartTouch. Checked on jacks 2026-10-13: a player spawning
  or teleported inside a trigger gets StartTouch, bots too, and the
  strip happens; jacks strips at the slide course's end teleports
  (`stripper_slide`) and on arrival in some arenas (`strippi`,
  `stripzomb`, `stripper_bw`), not in its lobby, and its "Normal" spawn
  mode goes to the lobby without a strip (so survivors walk in armed
  there, in CS:S too). Checked 2026-10-15 on the other maps
  (community-maps.md, round 6): all 29 strip triggers on the 7 mg_ maps
  with strips fire (`heavy::map_flows::every_strip_trigger_strips`);
  mg_lt_galaxy_v5's spawn hall strips survivors at each round start,
  swag strips on the drop from its spawn room; the other 10 mg_ maps have
  no strip, so survivors keep their guns there as in CS:S. Left: compare
  galaxy's hall with CS:S on the reference server.

- AK full-auto sounds choppy (user, 2026-10-11): fixed 2026-10-13, to
  playtest. `Weapon_AK47.Single` is one wave on CHAN_WEAPON at
  PITCH_NORM (CompatibilityAttenuation 0.52), so each shot replacing the
  last is parity; ours stopped the old one on whatever sample it was at
  (a click every shot). A replaced (or stopped) sound now fades out over
  5 ms (`audio::live_sound::fade_out_ramps_to_silence_then_ends`); the own
  sound plays centred through the same path. Capture to confirm: a CS:S
  recording of a full-auto AK burst against ours.

Connections and hosting: done 2026-10-10, to playtest (multiplayer.md,
"Playtest with friends"): Create Server's Game tab lists `settings.scr`'s
options on a fresh config (they were read when the main menu opened,
before the install's GameUI files came in); `net_maxfilesize` 512 on
both ends (150 MB map: `heavy::net_big_map`); a stopped server hangs
up on its clients (three copies of netcode's packet, Ctrl+C on the
dedicated server too) and they show "Server shutting down"; chat says
who joined and left and why. Left: whether the playtest's hang was a
lost packet, a killed process or something else (not reproduced here:
every stop path disconnected at once over loopback).

Gameplay and rules (done 2026-10-10, to playtest: a client's cheats run
on the server with its `sv_cheats`, noclip predicted; `chimera_noblock`
1/2, off by default; chat lines once in the console; `name` saved, a
"Player name" entry on Options > Multiplayer):
- "Gun does not reload after round end" (reporter THE GLITCH; fixed
  2026-10-10, to playtest): reproduced: R in the next round's freeze time
  did nothing (the freeze dropped every key but the look and weapon
  picks, `rules::rounds::hold_frozen`, and a network client the same,
  `net::predict::hold_local`); now reload and lastinv go through, moving
  and attacking stay held (`rounds::survivors_keep_their_clip_and_reload_in_the_freeze`).
  The clip isn't refilled at a round start: survivors keep their weapons
  and ammo as they are (parity, unchanged). Confirmed on the probe server
  2026-10-15: CS:S reloads in the freeze time at once, at the usual pace.

Looks:
- Third-person player models "not rigged right" (fixed 2026-10-10, to
  playtest): the gun hangs on the spine's animated `weapon_bone` (right)
  while the aim layers turn the arms with the look, so aiming up or down
  took the hands off the gun (a pistol's grip 4.8 units from the hand
  looking 40 degrees down). The player models carry IK chains (`rhand`,
  `lhand`) and per-animation IK rules (read from the files: hand poses
  lock the hands to the weapon, the knife's release them, dual pistols
  lock the right hand only, reloads lock to the grips); the hands now
  follow them onto `weapon_bone_RHand`/`_LHand` (where every weapon's
  authored pose has them, within 2 units and 9 degrees; not the
  grenades', whose world model hangs on the hand) with a two-bone IK
  (`map::anim::AnimSet::solve_ik`, `player_anim::lock_hands`;
  `chimera_hand_ik 0` turns it off to compare;
  `animation::hands_stay_on_the_weapon_while_aiming`). World models
  bone-merge: the dual pistols' left gun follows the left hand. Capture
  to confirm: CS:S third person (`thirdperson; cam_idealyaw 130`) with a
  pistol looking 30 degrees up and down, and crouched.
- Other players' weapon draw ("pull out") animation looks wrong in third
  person. Fixed 2026-10-10 (to playtest) what we found: a reload or shot
  gesture went on after drawing another weapon, played on the new
  weapon's arms (a reload up to 2 s); now a weapon change stops them
  (the grenade's throw still plays out;
  `animation::drawing_another_weapon_stops_the_old_ones_gestures`). The
  player models have no draw sequence (spec §14): the new weapon's upper
  body crosses over in its sequences' fade, as before. Capture to
  confirm: in CS:S, a bot switching rifle to pistol (and mid-reload) in
  third person, a few frames over the first half second; if CS:S shows a
  gesture we lack, it needs a spec.
- Standing inside another player (user: seen on mg_jacks_multigames_v1): the screen shows only the skybox, no
  world. Not reproduced on jacks either (2026-10-13): inside a bot
  standing, with `chimera_noblock` 0 and 1, moving and jumping in it; and
  every game's team teleports onto their shared destinations, players
  piled on one point, all eyes stay in the map
  (`heavy::map_flows::jacks_every_game_keeps_players_in_the_map`). The
  flood water (above) drawn over the sky looked like part of this from
  some spots. Not reproduced (2026-10-10, de_dust2): inside a bot standing and
  in noclip, and over the network (host `setpos` onto a client, the client
  walking, ducking and jumping inside it, both screens), the world draws
  and the body's insides show, as in CS:S; first-person spectating too.
  What the report describes matches the camera outside the map (the 3D
  skybox over black: what a `setpos` into the void shows), so something
  moved the player or camera out. Needed: the reporter's F9 bug report
  (`setpos` line, map) or the steps (map, team, spawn, noblock?).
- Crosshair off centre after Options > Multiplayer > Advanced: fixed
  2026-10-10, to playtest. On a scaled display (Windows at 125 %) the
  camera kept the in-game dialog picture's size (physical pixels) after
  going back to the window; the crosshair now uses the UI's size, and
  the camera's is worked out again (`--scale 1.25` reproduces it).
- de_dust2 (same screenshot, near "MAIN" on the wall by long/mid): large
  dark square panels on the wall. The stock CS:S de_dust2 has no "MAIN"
  graffiti and no grey stone there (its overlays: graffiti01-03 are other
  texts; checked against the install's BSP): the screenshot's map is
  another one (a remake or a multigame map's dust2 room). Found and fixed
  meanwhile (2026-10-10): DecalModulate decals and overlays (blood, dirt,
  scratches) had no texture, so their infodecals were never placed and
  their overlays drew untextured; now they multiply the surface 2x as in
  the game (`MapMesh::modulate`, world.wesl;
  `heavy::map_community::modulate_decals_multiply_the_surface`,
  gg_bk_warehouse_v1's blood). Needed: the map's name and a `setpos`
  there (F9 report) to identify the panels.

- Done 2026-10-09, checked against the coordinator's CS:S captures the
  same day: (1) other rooms showing through the sky
  (mg_creative_multigames_v8_ns): placed weapons, water surfaces (one
  mesh per material across rooms), beams/glows and moving props weren't
  PVS-culled; now they are (`vis`). Confirmed: CS:S at `setpos -6332.74
  -3047.50 -9638.97; setang -38.85 18.60 0` shows the same sky view as
  ours (nothing in the sky). Open: whether CS:S also hides things *in* the
  PVS behind a sky face; our `chimera_skyocclude 1` does
  (`render::sky_occluder`), off by default. Capture: a prop in view behind a
  sky face from a leaf whose PVS holds it. (2) The 2D sky's `dn` face
  turned half a turn (`cs_source::sky::FACES`). Not captured yet: CS:S
  looking straight down from noclip over a map with a pictured bottom
  (gg_lego_spacetower2's down-under sky), `refcmp skyconv` view. (3) The
  knife view model: confirmed (de_dust2, `setpos -295 1078 120; setang 0
  0 0`, both hands): the same blade (tan camouflage, serrated back edge),
  hands and placement. Its plain silver look in our earlier shot was the
  CT spawn's shade, not the model. Its idle (12.5 s, 151 frames) turns the
  blade sideways for a while; CS:S's two captures happened to be upright.
  Capture to confirm: the knife held 5-10 s after drawing it. (4) The 3D
  skybox's fog (sky_camera fog start/end divided by its scale, max
  density): confirmed, the distant hills' haze at `setpos -295 1078 500;
  setang 3 0 0` (noclip) matches within a few levels of 255; the
  start/end scale stays. Not `fogblend` (mg_kommando, cs_compound and
  de_port set it; other-maps.md). (5)
  Keyboard > Advanced (fast weapon switch, developer console =
  `con_enable`, ours defaults to 1, CS:S 0): not captured.

- Knife idle (done 2026-10-09, to playtest): the knife holds the draw's
  last frame and idles only `KNIFE_IDLE` (20 s, a guess) after its draw
  or last attack (its only idle, 12.5 s, is still for its first 3.4 s,
  then turns the blade); non-looping idles repeat after the script's
  `IdleInterval` (the TMP's 5 s sway, the only other idle that moves:
  `dump --sequences` prints each idle's motion). Capture to confirm: the
  knife 15, 20, 25 and 30 s after drawing it; the TMP 1-10 s after.
- cl_autowepswitch, the target ID (`hud_centerid`, `mp_playerid`) and
  the freeze cam (`cl_disablefreezecam`) (done 2026-10-09, to playtest):
  Multiplayer > Advanced's three check boxes work. Captures to confirm
  are listed in docs/tech-debt.md's rows for them.

- mg_item_battle_v4b's items (done 2026-10-09, to playtest): a picked-up
  knife's parented entities follow its carrier (`map::entities` anchors),
  OnPlayerPickup fires, and the entities the items drive work (game_ui,
  player_speedmod, point_template/env_entity_maker, func_physbox,
  phys_thruster/keepupright; docs/plans/active/community-maps.md, "Map
  entities"). The carried item sits at the middle of the carrier's box
  (31 units above the feet standing) turned to its yaw: CS:S's captures
  of the car knife (first person at `setang 30 0 0`, third person) match
  it exactly (the car on the floor, roof at the shoulders); at the feet
  the car sank to the knees. Left: playtest the cars and jetpack.
- Inside geometry, checked against CS:S (2026-10-09, mg_item_battle_v4b,
  reference captures in the coordinator's target/inside/): inside the stone
  wall ours already matches (world drawn, sky black). Inside the car
  (`setpos 2241.89 1311.06 79.39; setang 4.92 181.70 0`) CS:S shows the
  world through the car with only the hood's inside in view; ours showed
  the car's insides (wheels, chassis) and no world behind. Retake: our
  `setpos` put the eye 36 units lower than CS:S's then (below).
- `setpos`/`getpos` follow CS:S (2026-10-09, reference captures): `setpos`
  takes the feet (Source's origin; CS:S's camera was 64 above the z
  given, standing and in noclip, on two captures) and `getpos` prints the
  view's origin (the eye; in third person the camera: CS:S's third-person
  `getpos` was exactly its camera). So one `setpos` line gives the same
  view in both, and `setpos` with `getpos`'s numbers lands 64 higher, as
  in CS:S. Ours used the body's centre (36 above the feet) for both.
  Confirmed in CS:S's console: `noclip; setpos -295 1078 500; getpos`
  prints z 564.
- Options faithful to CS:S (done 2026-10-09, to playtest): the Voice
  tab, Multiplayer > Advanced from `cfg/user.scr`, Create Server's
  `< Random Map >`, the aspect ratio, and the cvars behind the greyed
  controls with CS:S's defaults (`fov_desired`, the crosshair's size,
  thickness, dot and custom colour, `snd_musicvolume`,
  `snd_mute_losefocus`, `m_filter`, `m_customaccel`, `cl_radar_locked`,
  `cl_downloadfilter`, water detail, `mp_decals`, `cl_c4progressbar`).
  What stays greyed and why, the deliberate differences (`con_enable`,
  `volume`) and the reference captures still needed (slider ranges,
  the colour list, `cl_crosshairusealpha`'s and `mp_decals`' defaults):
  docs/plans/active/ui-parity.md.

- Video > Advanced's and the Audio tab's remaining settings (shader
  detail, colour correction, motion blur; shadow and model detail done
  2026-10-10, characters' render-to-texture shadows left; speakers,
  sound quality and captions beyond being saved): one by one, per
  docs/plans/active/video-settings.md.
- Ladders (fixed 2026-10-10): standing on a ladder brush's top grabbed
  it as a ladder (its upward face carries the ladder flag): walking
  forward looking down stopped and a jump added the jump-off push. The
  probe now ignores ladder faces 0.7 or more vertical (our choice, the
  spec is silent; 36 stock ladder tops before, 0 now:
  `heavy::map_ladders`). Silent ladders: none found in the sim (every
  stock nav ladder climbed for more than a step period steps); a short
  climb right after a ground step hears none (the shared step timer,
  per the sounds spec), and holding walk or duck silences ladder steps
  (our CS:S walk rule). Confirmed in CS:S (de_train, feet on the ladder
  brush's top at (1607, -114, -136), pitch 30 down): +forward walks off
  the top (220 units in a second), a jump rises 52 units in 0.25 s (a
  normal 302 jump), as ours now. Left: a capture of a
  de_train car ladder's steps (docs/tech-debt.md, movement); walked
  straight at from the nav foot, 37 of de_train's 41 ladders don't
  attach in our sim (a clip face flush with them wins the probe; bots
  sidestep), worth comparing with CS:S.
- `kill` (fixed 2026-10-10): it moved you to a spawn alive; it is now
  CS:S's suicide, so a lone player can end a round. A network client's
  `kill` is a request to the server (2026-10-24, `net::cheats`,
  `PLAYER_COMMANDS`, no `sv_cheats` needed;
  `net_players::a_clients_kill_is_a_request_to_the_server`).

- Spectator mode (done 2026-10-10): CS:S's spectator team (team menu's
  Spectate, `spectate`, `jointeam 1`, `mp_allowspectators`): no body,
  free look first, chase and first person on anyone, `spec_next`/
  `spec_prev`/`spec_mode`/`spec_player`, duck opens our spectator menu
  (players, camera, join a team), the scoreboard's Spectators line,
  "*SPEC*" chat read by the dead and spectators, back on a team (in a
  freeze time at once), over the network (replicated, no team slot,
  not counted for round ends). Second pass (2026-10-10): the spectator
  menu is `bottomspectator.res`'s bar of drop-downs (Options from the
  install's `spectatormenu.res`, the players with `<` `>`, the camera
  from `spectatormodes.res`), the map overview (`overview_mode`: small
  or large, zoom, names, health, tracks), the Esc menu's Team dialog
  offers Spectate, and both team menus grey it under the server's
  `mp_allowspectators 0`. Left: CS:S's starting mode for a new spectator
  (no spec or capture; ours flies free), the overview's sizes and
  defaults against a capture (docs/tech-debt.md), the auto director.

- Decals persist across rounds (bullet holes, blood), as we believe CS:S
  does (players bind `r_cleardecals`, now there; `chimera_round_cleardecals
  1` clears them each round). Left: confirm the game keeps them.

## 1. Playtest essentials

A (settings, health bars, third person), B (penetration and collaterals,
docs/plans/active/weapons.md slice 5) and C (view models) are done. D and
E were paused on 2026-10-06 to free the machine; their partial work is in
agent worktrees under `.claude/worktrees/` (D: agent-a052e118eeb8d4c8d).

- **C. First-person view models**: done (docs/plans/active/view-models.md);
  remaining: check its open questions against CS:S (flash look, shell
  direction, near plane), view-model sound events, brass for other
  players.
- **E. Other maps.** In progress:
  [plans/active/other-maps.md](plans/active/other-maps.md) (catalog).
  Fixed: aztec's walls, props' ambient light on older maps, brush
  entities, additive glows, HDR skies, start-on switchable lights, murky
  water, decals on terrain, the Water shader (section 8), unplaced
  infodecals (131 -> 13 over the stock maps: a face was skipped when all
  its corners were far from the decal; brush entities the decal's probe
  hits take it), the 3D skybox's props unlit by the sun (nuke's cooling
  towers; 2026-10-18 survey of every other stock map in the plan). Left:
  reference views of the 13 stock maps never compared (windows-captures
  sitting 4), the 13 decals left (section 10).

## 2. Custom maps and minigames

Plan: [plans/active/custom-maps.md](plans/active/custom-maps.md).

- A test map with every supported entity (`mashup_logic_test`, generated
  `.vmf`, compiled with Valve's tools; plan section "Test map").
- Entity I/O, triggers and moving brushes are in (`source/logic/src/logic`, slices 3
  and 4 of the plan), and breakables (func_breakable, func_breakable_surf;
  source/logic/src/logic/breakables.rs), model doors (`prop_door_rotating`) and
  prop damage/outputs, skins, body groups and sequences
  (source/logic/src/logic/props.rs), sprites, dust and switchable lights
  (source/logic/src/logic/visuals.rs, lightmap styles relit at run time), env_global,
  trigger_soundscape through the touch code. Prop damage follows
  specs/source/prop_damage.md (source/logic/src/logic/prop_damage.rs: prop data,
  pieces, impact damage, explosive props, client props, Wake/Sleep/motion
  inputs, OnAwakened/OnMotionEnabled, pressure). Left: breakable follow-ups
  (section 7), train
  facing/banking and player train control. (Round restarts re-create
  entities: rounds plan.) Target: two
  real minigame maps from the user's downloads.
- First minigame map: `mg_lego_multigames_v2` (in the user's content
  cache). Fixed: impacts showing the sky on maps with water (decals were
  in the depth prepass), packed materials whose names differ in case.
  The breakable-block room went from 11-17 fps to the vsync cap (about
  240 fps at 720p uncapped; docs/performance.md, "Many brush entities").
  Impacts land on brush entities (doors, breakables) and go with them;
  unbroken brush entities draw merged (`map::merge`: 1768 -> 58 meshes
  at the blocks room).

- The user's ~89 community maps (surf_, bhop_, kz_, gg_, mg_): swept with
  `mapsweep` and their map logic audited with `mapsweep --audit`
  ([plans/active/community-maps.md](plans/active/community-maps.md),
  results and what's left, ranked). Round 2 (done 2026-10-10, to
  playtest): sprite trails, `.pcf` particle systems, smoke stacks, map
  ragdolls (drawn, not simulated), players' and brushes' render looks,
  water buoyancy (boats), motors and constraints, players riding karts,
  monitors, teslas, shooters, point_push, texture toggles, screen
  overlays; the audit's noise noted once per map. Round 3 (done
  2026-10-10, to playtest): map ragdolls simulated (and sent to network
  clients), moving water (func_water_analog, water on movers),
  color_correction (`mat_colorcorrection`), embers, fish pools, muzzle
  flashes, view punches, constraints breaking past their limits,
  monitors' sky, 3D-skybox particles, +use through a usable parent; the
  audit's +use misses were its own aim. Round 4 (2026-10-10, to
  playtest): surf_nebula's dead ramp stops (static props without a
  collision model block nothing), detail props (grass and weeds),
  monitors with their camera's own fog, bots stepping aside on maps
  without a navigation mesh. Top of what's left: particle and ember
  looks checked against CS:S, strike-generator env_beams, and three
  probes on the reference server (the plan's "Probes wanted": landing
  height, touch order of overlapping triggers, static props without a
  collision model). Round 6 (2026-10-15, to playtest): template copies of
  props and physics brushes drawn, solid and shootable (hidden on 12 mg_
  maps: block spawners, boats, karts, items, balls), func_clip_vphysics
  (balls kept on the pitch, boats held at the start), models with odd
  names, targeted func_wall/func_illusionary, server command lines,
  overlay V-flip; three spec questions in the plan.

## 2b. HUD and debug views

- CS:S HUD: health/armour/ammo/money/round-timer panels, death notices
  and the Tab scoreboard are in (`client/game_hud.rs`,
  `games/cs_source/ui/scoreboard.rs`), and the weapon selection and pickup history
  (`client/weapon_select.rs`; `hud_fastswitch`), matched to CS:S
  captures panel by panel (`refcmp hudcmp`). Left of that pass: the
  radar's look (zoom, translucency, markers) is ours. The team menu is in (M; joining a game shows the message of the day, then the team menu, then the class menu: `games/cs_source/ui/join_menus.rs`). The radio
  is in (`games/cs_source/ui/radio.rs`: Z/X/C menus, the calls as console commands,
  "Fire in the hole!" on throws, bots' enemy spotted/down and need
  backup, and bots' own commands and reports: go / stick together /
  follow me from the attackers' leader, cover me, sector clear, in
  position, regroup), with a chat area and hint text (`client/chat.rs`; the automatic
  `cl_autohelp` hints: `rules/hints.rs`, triggers ours until capture C26), the scoreboard
  in the game's `scoreboard.res` look (its status icons, "BOT" latency),
  the spectator bars (`spectator.res`), the radio icon over a teammate's
  head (`sprites/radio`, hidden by walls), and
  text chat (Y / U, `say`, `say_team`). The buy and team menus draw in the game's VGUI look from its `.res` files
  (`client/vgui.rs`), as do the message of the day and the class menu
  (`games/cs_source/ui/join_menus.rs`). Left:
  the scoreboard's row spacing (16 units, a guess) and dead rows' look
  checked against CS:S (its map time left beside the clock is in), the
  spectator menu (`bottomspectator.res`'s bar, done 2026-10-10; its
  values aligned as the file says 2026-10-15) and the freeze cam panel
  (`freezepanel_basic.res`: name, "KILLED YOU", "[F5] Save this moment",
  the killer's health gauge; 2026-10-15, timings now CS:S's server cvars
  and the death cam's measured: specs/cs_source/spectating.md) checked
  against a capture of a human's freeze cam (the avatar and nemesis lines
  are still out), the buy menu's favourites (autobuy and rebuy are in: `cl_autobuy`,
  `cl_rebuy`), checking the widescreen placement against
  the game, other game messages in the chat (team joins, bomb pickups
  and drops are in), bots' answers checked against a bot behaviour spec
  (`ignorerad`, bots answering and carrying out radio commands and
  issuing their own are in: `bot::radio::obey`, `speak`; checking
  when CS:S bots talk needs that spec). The radar is in (`client/radar.rs`: the map overview turning
  with you, team dots, your place name); its range (2200 units) is a guess.
- The game menu (Esc; `games/cs_source/ui/game_menu/`: Create Server; bots; team;
  options; bug report; quit) is in, in the game's GameUI look
  (`SourceScheme.res` frames and colours, `GameMenu.res` entries), its
  dialogs laid out from the install's `.res` files with VGUI's controls
  (`client/widgets.rs`; docs/plans/active/ui-parity.md). Left: what
  that plan lists as left (the console as a VGUI frame, the greyed
  controls whose features may come), binds for
  the keyboard actions greyed in the options (`impulse 100`, ...;
  `invnext`/`invprev`, `autobuy`, `rebuy`, `jpeg`, `nightvision`,
  `impulse 201` and `+voicerecord` are in), the Multiplayer tab's spray
  logo picture and a way to pick it there (sprays are in: `cl_logofile`
  in the console; Import Spray needs an image-to-VTF converter),
  comparing the look with CS:S's
  (refcmp has no menu views).
- The main menu is in: startup without a map shows CS:S's (its
  background picture, the title in its logo font, `GameMenu.res`'s
  entries; Resume and Disconnect only in a game, Find Servers greyed),
  the same menu over the game on Esc, `disconnect` back to it; none of
  our entries (done 2026-10-16: `chimera_quickstart`, `map greybox`,
  `menu bots`, `menu team`, `callvote`, `menu extras`, the console key
  instead). Left: the
  title's exact place and the entries' spacing checked against CS:S at a
  few resolutions (the loading dialog, `LoadingDialogNoBanner.res`,
  and the interface sounds are in; CS:S has no menu music: its install
  holds none), whether keyboard moves in the menu make the rollover
  sound, the server browser (needs networking). The Chimera debug-options page is in
  (`menu debug`, the overlay cvars as check boxes; 2026-10-24). A map
  changed from inside a game shows the loading dialog (2026-10-24).
- Debug overlays: `chimera_drawhitboxes`, `chimera_healthbars`,
  `chimera_drawnav`, `chimera_drawbots`, `chimera_drawsound` (sound
  radii) and `chimera_drawtriggers` exist (2026-10-24); add more as features
  need them.
- Debug UI (F8, `client/debug_ui.rs`) is in. Each bot's row has kick and freeze
  buttons (`bot_kick <name>`, `bot_freeze`/`bot_unfreeze`, 2026-10-24), a "Clear decals" button
  (`r_cleardecals`) and a Fog check box (`fog_enable`, CS:S's name; 2026-10-24).
  Left: starting a trace from the running game (needs
  the profile build), a fixed tick-rate control, a history graph per
  render pass.

## 2c. Multiplayer

Plan: [plans/active/multiplayer.md](plans/active/multiplayer.md)
(bevy_replicon + renet; Source-style prediction, interpolation and lag
compensation of our own; slices 0-9). Slices 0-4: a listen
server (maxplayers 4; map <name>) and the dedicated cs_source_server,
connect <ip[:port]>, characters replicated, usercmds bound to server
ticks with clock sync, the client's own movement predicted and
reconciled (net_graph-style readout, cl_showerror, net_fakelag;
docs/OBSERVABILITY.md, Network play), others interpolated, movers
and props replicated, weapons predicted with lag-compensated hits
(per-tick server hitbox poses), others' shots drawn from the server's
seeds, grenades, drops and pickups. Slices 5-7 done too: rounds, money,
buying, objectives, scoreboard, chat and radio; bots with `bot_quota`;
`changelevel` taking clients along, late joiners' full state, map
downloads (connection or `sv_downloadurl`, hash-checked, cached) and the
GameUI loading dialog for joining and map changes (detailed view:
`chimera_loading_details 1`). Their leftovers too: loading players out
of the game, predicted bomb arming and defusing, names (`name` over the
network), chat and radio flood limits, spawns never inside another
player, `changelevel` to the same map, bot profile names and `kick`,
decals for late joiners. Slice 8 done: Find Servers (Favorites,
History, Lan; Internet waits for a master server), server info queries,
LAN discovery, passwords. Slice 9 (lobby) dropped: standard CS:S. Left:
LAN broadcast between two machines (Windows) untested, CS:S's join rules
(empty team restarts the game) need a spec. Open questions 3 (NAT) and
4 (server size) answered 2026-10-13: Steam networking needs an app id of
our own (docs/plans/active/nat-traversal.md; UPnP mapping prototyped,
`net_upnp 1`), and a server holds 32 players within the tick
(multiplayer.md "Capacity", `heavy::net_capacity`); 1 is moot
(networking came first), 2 was taken as both.

Voice chat (2026-10-15, docs/plans/active/voice-chat.md): K held talks
(`+voicerecord`), Opus over the game's connection, `sv_alltalk`,
`sv_voiceenable`, `voice_*`, the HUD's talking icon and list. To
playtest with friends on Windows and Linux (real microphones and
speakers: only fakes are tested). Per-player mute is in (the game
menu's player list, `voice_mute`; client-side, for the session), and
the open microphone (`voice_vox`, `voice_threshold`), and the Voice
tab's microphone test, meter, boost and transmit volume
(`voice_transmit`, 2026-10-24). Left: icons over talking players' heads, a capture of
CS:S's voice HUD and client `voice_*` defaults (Windows), and checking
the dead-hear-the-dead rule with two clients on the probe server.

Votes (`callvote`, `net::vote`; CS:S's issues `game::votes`, panels
`ui::vote_hud`) are in: kick, change level, restart game, F1/F2, the
vote setup dialog (`ui::game_menu::vote_setup`: `callvote` alone or our
no menu entry, as CS:S), players joining a running vote, bots voting with
`sv_vote_bots_allowed 1` (our rule). Left: CS:S's rules, issues, look
and how its setup dialog opens, from capture C25 (parity-gaps.md).

- Missing server kicks (measured defaults 2026-10-16, objectives.md Q2):
  `mp_autokick` 1 (kick idle and team-killing players) and
  `mp_spawnprotectiontime` 5 (kick a player who team-kills within 5 s of a
  round restart). What counts as idle and how many team kills are
  unmeasured, and not measurable with bots on the probe server (kicks
  very likely spare bots; the probe plugin heals its targets): needs a
  human client (a Windows sitting).

## 2d. CS:S demos as a parity check (user idea, 2026-10-14)

Built: `demodump` reads CS:S `.dem` files (`source_formats::dem`; SourceTV
and, untested, client demos; OBSERVABILITY.md "CS:S demos").

- **`playdemo` in our engine.** Built: `playdemo <file>` / `--playdemo`
  (OBSERVABILITY.md "CS:S demos"). Left: a `demoui` panel, bodies' recorded
  animation where a game sends it, zoom through a sniper's eyes, physics
  props and doors from the demo, and comparing ticks against CS:S's own
  `playdemo` (refcmp). Was: drive our game from a CS:S demo: every
  player's position, angles, weapon, animation sequence, shots
  (`CTEFireBullets`), grenades, smoke clouds, sounds and the round/HUD
  state come from the file; our renderer, animation, effects, HUD and
  audio show them. Uses: compare the same tick against CS:S's own
  `playdemo` (screenshots via refcmp on the Windows PC or an unlocked
  desktop), watch any CS:S demo in our game (CS:S's `playdemo`/`demoui`
  are missing rows in parity-gaps.md), and record our own `.dem` later.
- **Re-simulate recorded input.** Built: `demosim` (OBSERVABILITY.md
  "Re-simulating a client demo"): a client demo's user commands decoded
  and played again through our movement and weapons, compared per
  command; the probe's logs as a real reference (`--probe`). Remaining:
  record a client demo on the Windows PC (bhop, ladders, surf,
  strafe-shooting; the doc says how) and run it, which also confirms the
  usercmd delta base and the own-client prop names
  (`cslocaldata.m_vecOrigin`, `m_nTickBase`) on a real file; other
  players as colliding puppets; doors and movers as recorded. Community
  demos on current map versions next; old pro demos (2005-2010) need the
  pre-Orange-Box network protocol and their own map versions, and often
  run at tick 100.

## 3. Weapons, remaining

In progress: [plans/active/weapons.md](plans/active/weapons.md). The
framework, the knife, every CS:S gun (zoom, silencers, bursts, pellets,
shell-by-shell reloads, the dual Elites), grenades, HUD, deathmatch and
a first bot are in.

- Probe the new guns' unmeasured rules (listed in the weapons plan):
  recoil (the automatics: measured 2026-10-15), the shotgun reload and pellets, the other ammo
  types' penetration. Ammo, box sizes, `primammo`'s fill-up and the
  spawn pistols' reserves: measured and matched 2026-10-14 (M19).
- The AWP's view model is in (MDL v48 reads like v44,
  specs/cs_source/mdl_v48.md); compare its fire and reload against the
  game. (MDL 45–48, sections, `.ani` blocks and the zero-frame cache
  are read: `anim.rs`.)
- Zoom: measure `zoom_sensitivity_ratio` and compare the scope overlay
  (the game's textures, laid out by eye) with CS:S.
- Rounds, money and buying: slice 1 done (`chimera_rounds 1`,
  [plans/active/rounds.md](plans/active/rounds.md)); ammo buying is in.
- Objectives, remaining (bomb and hostages are in: `games/cs_source/src/objectives/`,
  specs/cs_source/objectives.md): measure the spec's open questions on the
  probe (Q1 movement while arming, Q3 blast shape/walls/armour, Q4 beep
  schedule and loudness and where the LED sits, Q5 money, Q6 carrier
  choice, Q7 the bomb's solidity, Q9 hostage models, Q10 follow speeds,
  Q11 rescue rules, Q12 radar (no `sprites/c4` marker is drawn: CS:S
  shows none, playtest 2026-10-14),
  Q14 tick rounding); the planted bomb's own screen (`c4_panel`: the
  view model's is in, `games/cs_source/ui/bomb_fx.rs`), screen shake, the
  explosion's scale (it uses the HE's base explosion effect: the install
  names no C4 effect of its own, no explosion `.pcf` or sprite); hostage
  animation beyond idle/walk/run and a nod (`hostage_anim.rs`: compare
  with CS:S's hostages, which aren't measured; head/aim pose parameters
  toward the leader, flinch and cower), hostages avoiding
  "no hostages" nav areas, crouching and jumping; the game events' fields
  and the log's other lines (kills, chat, buys, connects: objectives.md
  Q17; the spec-named events and `triggered` lines are in,
  `rules/events.rs`); bots leading hostages, buying kits, guarding;
  which `Player.UseDeny` CS:S plays (doors_buttons.md Q2).
- Grenades, remaining (HE, flashbang and smoke are in: `weapon/grenade.rs`,
  `games/cs_source/grenades.rs`): measure the spec's open questions on the
  probe (fuse ticks, release timing, flash amounts and overlay curve, HE vs
  armour, smoke vs bots, HE shake and hearing) and replace the fits
  (tech-debt); bots throwing grenades, the `_thrown` models.
- Death animations on bodies (the grenade gesture is in; the reload gesture is in:
  `<Move>_Reload_<weapon>` by activity, an assumption for the spec's
  `reload_<suffix>`, with the shotguns' `_start/_loop/_end`).
- Impact effects, remaining (specs/cs_source/impact_effects.md; the
  section-by-section list is in `impact_effects.rs`'s `//!` doc):

  | Section | State |
  |---|---|
  | 1 who and when | in, others' shots to the clients in the shooter's PAS (`net::weapons`) |
  | 2 per impact, 3 letters | in, with the ragdoll check (no particles while one awake ragdoll is on the line) |
  | 4-8 old-style machinery, debris, dust, sparks | in; electric flares depth tested, no visibility test |
  | 9 glass | in: shards tumble on three angles and settle flat |
  | 10 blood | in from the spec's table, with the sprite cards' screen-size limits (2026-10-17, every sprite-card material), the goop's spin stop and yaw flip and the mist and goop light tint; `tint_perc` and the screen-size measure are our readings (Q7) |
  | 11 splash | in: water's own effect; slime's `slime_splash_03` from the install's `.pcf` (no stock map has slime) |
  | 12 knife | in; its decal: no spec names it (below) |

  Check the spec's open questions in the game. The muzzle flash's light on
  nearby walls is part of the view-model work (1C follow-up).
- Decals, remaining: on characters (blood; no spec covers model decals,
  overlays_decals.md's open questions); the knife's mark (`ManhackCut` is
  a guess and no spec names it: impact_effects.md 12 points to
  overlays_decals.md, which has no knife entry; a spec session or a
  capture) and lit decals (wood, glass) against the game; tracers are in (every 4th bullet, spec tracers.md; check N and
  the look in play).
- Dropping weapons, remaining (`drop`/G, pickup, death drops, bullets
  and blasts pushing loose weapons and the optional `chimera_usepickup`
  swap are in, `weapon/drop.rs`): the throw and slide are measured (M20, 2026-10-15; ours stops at 72 of 95 units); the re-pick delay, pickup
  reach and mass, friction (0.5) and the box collider are guesses
  (measure on the probe server: how far a shot weapon skids).
- Ragdolls, remaining (`map/ragdoll.rs` has the spec's bodies, joints,
  self-collision pairs, death poses, impulse, bone velocities, bullet and
  blast pushes, separation repair, settling and removal): check the
  spec's open questions (force scale, bone_dt, death-pose facing and
  crouch choice, joint limit convention) in the game.

## 4. Bots

Parked (user, 2026-10-13): bots are fine for now; focus on what players
see. The last pass's notes and open ideas (de_dust2 balance ~71% T,
de_nuke vent stuck spots, team eco) are in git history and
docs/tech-debt.md's bot rows.

## 5. Console, remaining

The console and overlays are in (chimera/core/src/console.rs, chimera/client/src/client/console.rs).

- `net_graph` (chimera/client/src/client/net_graph.rs): compare the
  layout, colours and graph with CS:S's own (a capture of `net_graph 1`
  to `3` on a server); `net_graph 3`'s per-message bands, `net_graph 4`,
  `net_scale`, `net_graphmsecs`, `net_graphtext`, `net_graphsolid`.
  At the default `net_graphheight` 64 its last lines overlap the HUD's
  ammo bottom right (1280x720); check where CS:S's sits.
- `cl_showpos 2`: what CS:S shows with it is unknown here (no public
  description found); look in the game.

## 6. Sound, remaining

docs/plans/active/sound.md.

- Scrapes by ear: tune `games::cs_source::scrapes::friction_energy`
  against CS:S (a crate shoved across concrete, a ragdoll sliding down a
  ramp) once spec open question 8 is measured.
- Room DSP by ear: tune `audio::room::PRESETS` against CS:S (dust2's
  tunnels, nuke's halls); soundscape `dsp_player` and `soundmixer` are
  read but not played (engine-side effects).
- Stock ambient_generics the bomb starts (de_nuke's alarm, dust2's
  fires) now play when it explodes (`BombExplode`); check them by ear. (Prop outputs are in: de_nuke's steam, cs_office's
  projector; env_steam's JetLength/SpreadSpeed/Speed inputs are in, for new
  puffs; heat-wave jets, Type 1, refract the scene: no stock map has one, so unseen.)
- Measure ambient_generic's level for script entries vs raw waves (spec
  open question 11).
- Measure on the probe server: the distance curves (replace the H1/H2
  guesses), CS:S footstep silence rules, the jump sound, wave choice.

## 7. Physics props, remaining

- Player physics shadow (games/cs_source/src/game/shadow.rs, games/cs_source/tests/it/heavy/map_physics_shadow.rs)
  follow-ups (docs/tech-debt.md "Physics shadow"): the push speed limit
  (spec Q7), the shadow's weight on a prop stood on, the controller's
  velocity target when not touching (spec 4.1 step 5), measuring the push
  feel against CS:S on cs_militia/de_inferno crates. CS:S has no +use
  pickup of props (no physgun; +use pushes only under `sv_turbophysics 1`,
  spec 4.2.4, not done).
- Prop damage follow-ups (docs/tech-debt.md "Prop damage"): stress crush, the
  velocity restore after an impact breaks a prop, the chunks' 90°
  realignment, the spec's open questions on the probe server
  (Q1-Q14: damage types, gas-can ignition, player impact rules, client
  break sounds, round restarts of client props).
- Model doors: the spec's open question 8 on the probe server (which
  entries the hardware and surface pick; we read the model's
  `door_options` skin and `hardwareN` blocks, docs/tech-debt.md "Model
  doors").
- Breakables, remaining (source/logic/src/logic/breakables.rs,
  games/cs_source/tests/it/heavy/map_breakables.rs): the GlassBreak/BulletProof
  decals and brush propdata templates are in (their readings in
  docs/tech-debt.md "Breakables"), and so are bulletproof glass's ricochet
  and the computer material's sparks on bullet traces; left: whether the
  ricochet has a look (the specs give it only `Bounce.Shrapnel`), the client's pane
  drawing (spec open question 8:
  edge piece selection, which side draws, the piece's offset; ours in
  docs/tech-debt.md "Breakables"), the spec's open questions on the probe
  server (bullet/knife damage types, broken brush visibility, shots after
  a window breaks).

## 8. Visual fidelity

- **Water surfaces** (specs/cs_source/water.md, `render::water`): refraction,
  planar reflection, the cheap cubemap pass, under-water fog (world,
  props, ropes, decals, particles; also in water of other shaders, de_nuke's
  pool, from its material's fog keys), bottom materials, the
  `$underwateroverlay` screen warp (with its `$bluramount` blur, which is
  what shows on de_port), the intersection view's height fog, the sky
  under water (through the surface from below when the water refracts,
  none in fog-only water), the water cvars, `$blurrefract` with `$refracttint` and the "Water
  detail" choices (`render::water::WATER_DETAIL`, for the Video Advanced
  menu) are in. Left: a Refract shader spec (the warp's
  strength and blur are guesses: de_port's `water_warp01` moves the
  screen about a pixel, its blur is 5 x 5 taps a pixel apart),
  the `$basetexture` variant (no stock material uses it), and a
  refcmp comparison of de_port/de_chateau water (no reference captures
  yet; it would settle de_port's dark speckles: refraction lookups at
  `$refractamount` 5 landing on barely submerged shore, which the spec's
  shore fade leaves unreflected). Water currents: no stock map has
  current contents (spec movement.md open question 15), so not applied.
- **HDR parity**: `mat_hdr_level` 1/2 (default 0, the reference's LDR)
  loads the HDR lightmaps, ambient cubes, world lights and `_hdr` sky,
  renders HDR with bloom and auto exposure within the map-start
  `env_tonemap_controller` bounds (client/hdr.rs). Not matched against the
  game (no HDR reference captures; refcmp runs LDR). Open questions for a
  spec session: the engine's HDR lightmap scale (taken as 1), the exposure
  target (`hdr::EXPOSURE_TARGET` 0.5), metering and adaptation speed
  (Bevy's histogram, 3/1 stops/s), the bloom filter and strength (Bevy's
  `OLD_SCHOOL` at 0.05 x bloom scale), whether bloom precedes exposure,
  HDR cubemaps/envmaps (still LDR), the `<sky>_hdr` material lookup rule,
  `mat_hdr_level` 1's exact look, and the controller's inputs (followed
  through the logic as they fire, `map::TonemapInputs`): what
  `SetTonemapRate` scales (taken as a multiplier on the adaptation speed),
  whether `UseDefaultAutoExposure` means the cvars' 0.5/2 bounds, and
  `SetBloomScaleRange`, `SetTonemapScale`, `BlendTonemapScale` (accepted,
  ignored; no cached map uses them). Then refcmp captures at
  mat_hdr_level 2.
- **Light styles and baked prop light, open questions** for a spec
  session: animated styles step ten pattern letters a second, unblended,
  'm' = as baked, scaling the style's lightmap share in linear light
  (Quake's convention; the engine's rate, interpolation and scaling space
  aren't specced), styles 13-31 without a light `pattern` stay steady,
  and props' probes ignore them. Baked per-vertex prop light (`.vhv`,
  `source/formats/src/vhv.rs`): its decode is taken as the spec's
  (2v)^2.2 (shaders.md 4) for both `sp_` and `sp_hdr_` files (the HDR
  files in the cached maps hold the same bytes as the LDR ones), each set
  falls back to the other, and a file whose checksum or vertex count
  doesn't fit the model is skipped; it comes out at about 0.45-0.75 of
  our probe on average (self-shadowing, or a different scale?). Compare
  a prop on kz_ancient_ruins or surf_demise with CS:S.
- **More refcmp views** across dust2 (mid, long, B, spawns) and other maps.
- **Material effects, open questions** for a spec session
  (specs/cs_source/shaders.md 16-19): texture transform composition and
  the TextureTransform proxy, `$emissiveblend*` (surf_demise's glowing
  banners aren't drawn), `$selfillummask`'s channel and
  `$selfillum_envmapmask_alpha`, detail modes 10/11.

- **Combat Arms mission display: rough edges (2026-11-01).** The
  objective list (top left) overlaps stray "Player" labels drawn there
  on nemexishq (name tags at the screen's corner?); captions, messages
  and objectives show their ids (the string table lacks them; find where
  their texts live); counted objectives' maxima are 0 (mission data not
  read); a lowercase `add <id>` object message runs the console's `add`
  instead of the MissionTrigger's `ADD`.

## 9. Performance

- Done: **Disk guards (2026-10-31: the dev box's disk filled to 100%).**
  `tools/agent-slot.sh trim-incremental` (run by `trim`,
  `tools/clean-worktrees.sh` and `tools/post-push-tests.sh`) trims
  `target/*/incremental` of the main checkout, the post-push slot and the
  slots past `CHIMERA_INCREMENTAL_MAX_GB` (30), oldest first, skipping a
  target a running cargo holds; the map cache and the store's on-demand
  writes skip below the free-space floor (`CHIMERA_DISK_FLOOR_GB`, 20 GB or
  5% of the disk) and the map cache's cap is limited by free space;
  `tools/doctor` and `post-push-tests.sh` warn (OBSERVABILITY 3e). Left:
  nothing runs `clean-worktrees.sh` on a timer; `chimera_pack`'s `put` and
  any other cache don't check space.

- **Flaky: `heavy::net_big_map` under a full heavy run (2026-10-31).**
  In a9caa3b5's after-push run, 150 MB took 133.4 s of game time against
  the 120 s limit; alone, at the same load (~20), it passes in 8.8 s.
  The game-time rate seems to depend on wall time when the heavy slots
  compete. Make the measure independent of wall-clock contention (or
  run it in a slot of its own) if it fails again.
- **`party_kit` `heavy::spawns::every_pool_map_s_spawn_spots_are_inside_it`
  sits at its time limit (2026-11-09).** 449 s, 602 s, then timed out at 723 s
  (67ef9e85's after-push run, 92 min with three agents building). The other
  4496 tests passed. HL2:DM's maps in the pool and the load both add to it.
  Timed out again at b0af9762: it now has a limit of its own, 30 minutes
  (.config/nextest.toml, like the soak's). Split it per map if that isn't
  enough.
- **Once: `combat_arms bots::merc_27_base_point` index out of bounds
  (2026-11-03).** In 931cda5e's after-push run, `chimera/world/src/map/nav.rs:245`
  read area 67113151 of 26529: that's the valid 4287 with bit 26 set. It
  passed 11 reruns and the indices come from safe, deterministic code, so
  it may be a hardware bit flip (no ECC reporting on the box). If it comes
  back, look at the ragdoll solver's `get_pair_unchecked_mut` first, then
  run a memory test.
- **Fewer character archetypes, what's left (2026-11-09).** Parts a
  character gained once are on it from the spawn, parts whose absence meant
  "none" hold a value meaning none (`MaxSpeed`, `HeldMovePenalty`,
  `Shadowed`, ...), and parts that go together go in one move
  (docs/performance.md, "A session's archetypes: the game switch"): three
  laps of real maps make 472 characters' archetypes (1,471 in all), a lap
  after the first 19-43 (the lap test caps it at 100). Left: a switch's own
  steps, each another game's system (avian's `RecomputeMassProperties` after
  Combat Arms' collider changes, the movement swap's removal and insert,
  Combat Arms' rung look and view model, CS:S's rung class, HL2:DM's suit,
  `ArmorPieces`): one Chimera command gathering a character's inserts and
  removals of a tick and applying them together (`insert_by_ids`,
  `remove_by_ids`) would cut most of the rest. Not measured: a quiet tick
  with the bots kicked against the 800 instructions an archetype.
- **The render thread's cost rises over a session, what's left (2026-11-09).**
  The kit's undrawn `PropMaterial`s no longer reach the render world
  (`prop_material::DormantProps`; docs/performance.md, "A session's render
  thread": each GPU buffer of the peak costs about 150-200 instructions a
  frame for good). Left: measure the real app on a display (de_dust2 fresh,
  then after eight other maps, `chimera_mem` and the render instructions) and
  compare frozen-scene screenshots before and after on a Combat Arms and a CS:S
  map; then price what's left of the 14M -> 19M (Bevy's mesh allocator and
  render-asset hash tables keep their peak capacity).
- **The tick and the frame, what's left (2026-11-10).** docs/performance.md,
  "Performance pass": the logic bridge, the push-away, buoyancy, Combat
  Arms' posing math and HL2:DM's per-frame searches were cut (de_dust2's
  CS:S tick -18%, mg_lego's -50%, the party's -13 to -15%), and Combat
  Arms' poses are made on demand ("Poses on demand": 16 of them a tick to
  3.7 with 16 bots). Since done: Chimera's hitscan tests a bound each
  game may declare with its `LazyHitboxes` first, so a ray far from a
  Combat Arms body leaves it unposed ("A broad phase for the hitscan":
  hitbox sets made 0.12-0.16 a tick to 0.02 with 16 bots; the bound is
  loose, 3-4.5 m across a soldier: posing the root's first nodes exactly
  would tighten it), and `sync_movers` passes over idle movers by a
  change stamp from `LogicWorld::get_mut` and their nodes' change ticks
  ("A change stamp for the movers": mg_lego's tick -24%). Left, by size:
  the logic's own mover step and the other passes on mg_lego; particles, steam
  and the other effects still write new mesh data every frame (the blob
  shadows', moving prop shadows', dust's, probe light's and faces' per-frame
  changes are done: docs/performance.md, "Render performance"); `NavMesh::area_at` walks every area (2% of a tick
  with bots: a grid); the ragdolls' pose record and the server's hitbox
  pose each sample a Source body's animation every tick; the server's
  documents are packed with bzip2 at every start (3-7 s of a core on a
  thread: keep the packed bytes by hash). `perfgate`'s stored baseline is
  from 6b95f1c (three times today's counts): take a new one on a playtest
  build (`perfgate --update`).
- **Process memory over a session, what's left (2026-11-09).** After the
  animation sharing and `malloc_trim` (5.5 GB after eight maps, was 15.4) the
  heap still holds the current ladder's Combat Arms model textures on the CPU
  (about 0.9 GB: decoded RGBA and mips, kept to build each map's images again);
  dropping the decoded copy once uploaded would take most of it. On Windows
  the trim doesn't apply: measure the process there over ten maps.

Measured and culled (docs/performance.md): `chimera_perf`, `refcmp bench`,
PVS culling by world chunk, areaportals (closed doors hide what's behind
them; views clipped through openings; window brushes fade in and close
their portal), occluders (func_occluder), prop fade distances with
dithered fade bands. Left:
- Done: physics/animated props fade (fademindist/fademaxdist keys,
  `prop_fade` test); LODs for static, skinned (animated) props and
  CS:S player-model bodies (`render::lod`; levels share the skeleton, as
  the `.vvd`'s bone ids are the model's own at every level; tests in
  `prop_lods.rs`; a level with a negative switch point, as the player
  models' last, -1, is never drawn: `player_lods.rs`). Left: a near/far
  capture of a skinned model; LODs for
  Combat Arms' models and view models.
- cs_assault `refcmp vischeck` fails (57 of 316 views, worst 33.7k
  pixels on the roof): the PVS hides props and buildings seen over the
  walls through sky brushes, consistent with the map's own data
  (performance.md, "Checks"). Capture C27 says whether CS:S hides them
  too; if it shows them, find the engine's rule for static props seen
  past sky brushes. Until then cs_assault's vischeck failure is expected.
- `refcmp ordercheck` on cs_assault: passes (limit 100 pixels) but 8 of 316
  views still differ (re-run 2026-10-25, `capture-ours` works here, no
  reference captures needed): `nav1847_90` 56 pixels, `nav1567_180` 26,
  six others by 1 pixel. Looked at `nav1847_90`: the differing pixels
  are a strip along one edge (x 880-890, y 15 and below, in a 1280x720
  capture) where a dark surface meets lit wood, i.e. a depth tie at a
  geometry edge between two opaque draws, not a blended item. Most likely
  cause: opaque batches draw in pipeline-id order (made in the order the
  camera first saw each material kind), and where two opaque surfaces
  meet at the same depth the one drawn last wins. Next step: capture
  `nav1847_90` alone and after `nav1847_0` (performance.md, "Checks"),
  then query the two entities at that pixel (remote protocol, section 3)
  and compare their materials and pipeline keys; fix by giving opaque
  batches a stable order (as `render::blend_order` does for blended items)
  or by fixing the overlapping geometry. Not worth more than that while
  under the tolerance.
- Measure on the Windows PC (`refcmp bench` there) and set a budget.
- Frame-time follow-ups (performance.md, "Frame time pass"): before/after
  numbers on a quiet machine and on the Windows PC (`refcmp bench`, also
  `CHIMERA_EXECUTOR=multi`, `-- --view-size 3840x2160`); props as
  hierarchies of their own (cheaper collider propagation) only if a map
  shows it (stock maps walk 240-710 colliders a tick: performance.md,
  "Props that never sleep"); a render
  scale (`mat_render_scale`) only if a GPU turns out to be the limit (the
  dev box's RTX 3080 draws dust2 at 4K in about 1.3 ms). Done: props that
  never slept, ragdoll and gib hulls cut, ragdolls' forced sleep, the
  view-model camera off with nothing on its layer, broken props' pieces
  as debris that doesn't collide with debris, ragdoll limbs that jittered
  against the floor (joints now solved in the contacts' passes, no
  gyroscopic torque; performance.md, "Props that never sleep", "Fewer
  cameras per frame"), broken props' pieces with their `.phy` hulls
  (sleeping by props' stillness rule), a toy-scene test of ragdolls'
  forced sleep.
- Party frame rate (performance.md, "Party frame-rate pass"; measure
  each step with `tools/perf-profile.sh` and `--baseline`): after the
  second pass the main thread sets the frame again (render world 44-57%
  fewer instructions, de_dust2 126 -> 150-187 fps, cs_office 110 -> 139
  on a quiet box); after the third pass ticks take half the wall and
  CPU time headless (every schedule on the single-threaded executor:
  the prediction stages ran on Bevy's multi-threaded one). Next: frame
  rates of the third pass on a quiet box and on the Windows PC (the dev
  box was at load 12-40); fewer systems per tick (~0.6-0.8 ms a frame
  of per-system executor cost; merging changes the order of unordered
  neighbours, so each merge against the schedule ratchets); `bevy_ui`
  layout (0.25-0.30 ms, two walks over every node, changed or not:
  despawn Combat Arms' 254 hidden HUD nodes while its HUD is away,
  fewer UI roots: Bevy inserts `Propagate` into all 73 each frame);
  Bevy's
  per-frame `QueryState`s in bevy_audio's `cleanup_finished_audio`
  (0.10 ms) and the screenshot plugin (0.04 ms), upstream; ropes
  rebuilt only while the PVS shows them; a party `perfgate` scenario; whether CPU batching
  stays cheaper on maps with thousands of meshes in view (`refcmp
  bench` with and without `CHIMERA_GPU_PREPROCESS=1` on
  mg_lego_multigames_v2).
  Done: kept query states (`chimera_core::query_cache`: ticks 16-24%
  fewer instructions), `keys_for` without copies, `roster::arm`
  without cloning `BotData`, the profiling tools (`profsum`,
  `perf-profile.sh`, `tracesum --folded`); every camera's meshes
  batched on the CPU (`NoIndirectDrawing`) and only the last camera on
  a target copying out (`render::cameras`); `chimera_perf 3` naming
  modified meshes and materials and logging the UI trees (unnamed
  roots by their component types); every schedule single-threaded
  (`harness::set_executors`, extract excepted), Bevy's idle
  directional-light check off (`render::idle_lights`), HUD writes only
  on change (`client::ui_write`), binds looked up by the keys down,
  Combat Arms' skeleton and animation names through indexes, kept
  queries in the ladder, `sync_visuals`, the scoreboard and
  `late_join` (performance.md, "Third pass").
- `perfgate` in the pre-push hook (budgets per game app): deferred by
  the user (2026-10-15), pushes aren't blocked on performance for now.
  A Windows-side check needs another counter source (none there yet).
- *Done:* the converted-map cache's follow-ups (2026-10-31,
  performance.md, "The converted-map cache, second pass"): a miss's
  entry streamed to the file (peak 2.9 GB to 1.5 GB), textures without
  their decoded texels (de_dust2 986 MB to 399 MB), `chimera:map_cache
  0/1`, the whole heavy tier measured (716 s off, 637-646 s warm; six
  slots 542-544 s but out of memory: five stay), Combat Arms left off.
  Left: the asset store's own cap.
- *Done:* the seven heavy tests that failed with the installs on 5e324989
  (Combat Arms' archive count and view-model timings, the mashup's
  world-only load, party_kit's dust2 gun game and warehouse bodies) were
  fixed by the fix-install-heavy merge (74457ecf), which landed while
  this measurement ran. Post-push runs now use the installs.

## 10. Long tail

Counts are from de_dust2's entity lump and static prop lump.

- **Combat Arms world objects, what item 23 left** (docs/plans/active/
  combat-arms.md row 23): the parity decision on clients' movers (the
  original draws sliding movers 100 ms in the past, snaps rotating ones,
  jumps keyframed ones and pushes its own player per drawn step, never on a
  teleport; ours predicts them: world_objects.md Follow-up (2026-10-31b)
  2, cases 40-45 not reproduced); the display of cameras (which camera a
  client looks through: death_killcam.md; the timers, captions, messages
  and objective list are ours, done: DESIGN "The mission display");
  the co-op parts (sub-mission time, mission manager, mission texts and
  counted maxima).

- **Remaining decals**: 13 infodecals over the stock maps find no
  surface (dust2 5, tides 3, compound 2, dust 2, train 1): by props
  (window frames) or farther than our reach from any face; the probe
  ignores props; whether the engine puts them on props is open
  (overlays_decals.md). Overlays with
  no geometry (train 8, assault 5, others 1) list no faces or faces
  away from their quad: nothing to draw (see plans/active/other-maps.md).
- **Verify inferred Source rules** with the comparison tool, using a local
  copy of a map with test entities added where dust2 has no example: floor
  and ceiling decal orientation, decal reach, overall brightness/tonemapping.
- **Brush entities**: movers (doors, buttons, func_rotating, trains,
  func_brush) move through the logic layer, breakables break; baked brush
  entities draw their render mode (colour/texture/alpha blend, glow/additive
  add, 10 hidden) and rendercolor; mode 1 draws the flat colour and mode 4
  alpha-tests (cs_italy's and de_piranesi's foliage and fences).
- **Fire** (specs/source/fire.md) is in: env_fire (heat, growth, burn damage box with line of sight, fuel, Extinguish, outputs, re-created unlit each round, Q4), env_firesource/env_firesensor, entity flames (Ignite inputs, burning props, the gas can), the `env_fire_large_smoke` and `burning_character` looks (docs/tech-debt.md "Fire"). Next: compare a lit de_dust2 fire with CS:S's (Q1, Q10 operator meanings), and the probe-server questions: burn vs armour, the kill icon and score (Q2), player ignition (Q3), the radius damage of a burning can (Q6), burning-prop light (Q7), buried fires (Q9), floors on de_dust/de_train (Q5).
- Combat Arms: the LAW's steep climb and late launch (seen 2026-10-31).
  Done 2026-10-31: both match the spec, nothing to fix. The install's
  launchers (read through our loaders, `heavy::launchers_install`): LAW,
  RPG-7 and AT4 `Missile` 1300 u/s, `Gravity` 0, cone 1..30; M32, M79,
  MZP_1 and XM_25 shells 1550 u/s, `Gravity` 1; the flame 500 u/s,
  `Gravity` 0; every view model's fire key is at 0 ms and only the
  flamethrower has a pre-fire (200 ms). A level LAW shot through the real
  fire path leaves one update after the press along (0, 0, -1300) within
  the cone's thousandths and flies level. The 75 frames are the LAW's draw
  (1333 ms, about 80 frames at 60 Hz): the run pressed fire on a launcher
  it had just been given, and a press during the draw launches when the
  draw ends (weapon_classes.md). The climb is the 5.5 degree view kick of
  the shot, which follows the shot and turns the camera up while the
  rocket flies straight, or the run's `setang`/`setpos`; the rocket's own
  path has no climb. Pinned by `launcher_aim.rs` (level shot leaves level,
  the kick does not bend the flight, a press during the draw waits for
  it, shells fall, flames and rockets do not).
- Combat Arms: `pv_glass` as its own lit pass. (Done 2026-10-31: a LAW's smoke trail seen on grandcanyon (`+setpos 0 40 0`, fire, `+setpos` aside, several `+screenshot`s a few frames apart; the rocket launches about 75 frames after the press); the held
  launcher's `Flash` socket posed from its model, `CaPose::flash`; devices',
  grenades' and bolts' sounds on network clients, `CaFlightLook` replicated
  and the create sounds ready by entry name.)
