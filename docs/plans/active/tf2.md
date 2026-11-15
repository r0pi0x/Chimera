# Plan: Team Fortress 2 on Chimera

Started 2026-11-15. One long orchestration session on the Linux box
runs it overnight. It plans, has sub-agents write the specs and the
code, verifies, merges each finished step to main and pushes. This file
is the status page: read "Status" and "Log" to catch up.

Goal: Team Fortress 2 is a fifth game library (`games/tf2`, crate and
id `tf2`). It lends its guns, movement, classes' bodies and maps to the
Ultimate Gun Game, as CS:S, Combat Arms and HL2:DM do
([new-game.md](../../new-game.md); HL2:DM is the model port,
[hl2dm.md](hl2dm.md)). Parity first: a TF2 rung plays as the real game,
from specs, measurements and the game's own data.

## Status

| Milestone | State | Summary |
|---|---|---|
| M0 plan and first specs | done (2026-11-15) | this plan; specs/tf2/: install, movement, classes, weapons, ICE |
| M1 a map you can walk on | done (2026-11-15) | `games/tf2`; mount (missing archives skipped); ctf_2fort and cp_badlands converted with team spawns; `tf2:<map>` in the party apps; `$ssbump` drawn |
| M2a the weapon scripts, `chimera_pack tf2` | done (2026-11-15) | our ICE (`game::ice`) with the spec's readings selectable; `game::scripts` (store/plain, install plus key, none); `dump tf2 --script`, `--ice-probe`; `chimera_pack tf2` (`weapon/<id>` for M2's guns with the decrypted script, `players`, `map/<name>`); not published. No reading reproduces the paper's triplets: the user's `--ice-probe` settles the reading |
| M2b TF2 guns as UGG rungs | done (2026-11-15) | the four guns as data with the spec's numbers (the scripts' field mapping a seam), TF2's gun rules as hooks, `c_` weapons bone-merged on the class arms, sounds, TF2 in the party's `GUESTS`, `originals:gun_game_2fort` |
| M3 classes as bodies, TF2's movement | done (2026-11-15) | the classes as data (`game::classes`); `tf2:movement` on Source's movement with TF2's config and the held gun's class (speed, eyes, the Scout's air jump, max health); the class bodies with team skins and animation; TF2's HUD (health, ammo, crosshair) and kill icons |
| M4 more maps and weapons | not started | |

## 1. Rules for this port

- **Clean room.** TF2's and the Source engine's code leaked, and the
  Source SDK is public. Neither is ever read, copied or translated.
  Spec sub-agents read the game's data files in place, public
  documentation (the community TF2 wiki, Valve Developer Community)
  and measurements of the dedicated server, and write only
  `specs/tf2/`. The orchestrator and implementation agents read only
  `specs/` and the repository.
- **No game files, extracted data or keys in the repository.** Assets
  load at run time from the user's install or the asset service. No
  install path, home path or key goes into a commit.
- **The ICE key is the user's.** The `.ctx` scripts are encrypted with
  ICE (specs/tf2/ice.md). The key is never committed, and never
  written into code, tests, docs, specs or commit messages. It is read
  at run time from the local config or the environment (section 3),
  as Combat Arms' keys are.

## 2. The install

| Layout | Steam app | What it has | Use |
|---|---|---|---|
| The retail client (`Team Fortress 2`) | 440 (free to play; needs a Steam login, also in SteamCMD) | everything: maps, models, textures, sounds, HUD resources | the install for play and tests, and the packing source for `chimera_pack tf2` |
| The dedicated server | 232250 (SteamCMD, anonymous) | maps, models, material text, scripts and `items_game.txt`; the texture and sound VPKs' `_dir` files only, without their data archives (install.md 1) | geometry, collision and spawns without textures; measurements with puppet bots (movement.md, classes.md) |

- **Found** as the other games are (`chimera_core::mount::install`):
  `$CHIMERA_TF2_ROOT` for one run, then `[games.tf2] path` in
  `chimera.local.toml`, then the path saved by the first-run dialog,
  then Steam's libraries (app 440). A SteamCMD install outside Steam's
  libraries needs the config line or the variable.
- **Required files**, present in both layouts: `tf/gameinfo.txt`,
  `tf/tf2_misc_dir.vpk`, `hl2/hl2_misc_dir.vpk`.
- **The mount**: the search path from `tf/gameinfo.txt`
  (`source_engine::search_path`, as HL2:DM). An archive listed in a
  `_dir` file but missing on disk is skipped file by file, so the
  dedicated server's layout mounts (install.md, open question 3).
- **The server layout** from SteamCMD, anonymous:
  `steamcmd +force_install_dir <folder> +login anonymous +app_update
  232250 validate +quit` (docs/assets-for-developers.md).

## 3. Decisions

1. **The id is `tf2`**: crate `tf2` (`games/tf2`), maps `tf2:<map>`,
   console namespaces `tf2` and `tf`, the key `[games.tf2]`, the asset
   sets `tf2`. Content folder `tf`. Tick 0.015 s (movement.md, measured).
   0.0254 m per unit.
2. **Maps** go through Source's BSP conversion (`source_engine`), with
   TF2's data: spawns from `info_player_teamspawn` (`TeamNum` 2 RED,
   3 BLU; `StartDisabled`; on control-point maps the round's owner of
   the named point; install.md 2). TF2's maps need LZMA-compressed
   lumps, the LZMA game lump and static props version 10, and LZMA
   entries in the pak (install.md 2). Where the shared conversion lacks
   these, they are small generic extensions in `source_engine` or
   `source_formats`, not TF2 code.
3. **Weapon numbers come from the game's own scripts.** The per-weapon
   scripts `scripts/tf_weapon_*.ctx` are ICE-encrypted KeyValues.
   - `games/tf2` holds our own ICE implementation, written from
     specs/tf2/ice.md (never from another implementation; the Rust
     `ice-crypt` is GPL-3.0 and no port is in THIRD_PARTY.md). Its
     tests: the paper's known-answer vectors where they reproduce, and a
     fixture encrypted at run time with a throwaway key the test makes.
   - The key: `CHIMERA_TF2_CTX_KEY`, else the TF2 entry of the local
     config's game keys (`[games.tf2.keys]`, as Combat Arms' `V1` and
     `V2`). Never logged.
   - Where the numbers come from, in order:
     1. **the asset service** (the default, how UGG players get every
        game): the host packs the scripts decrypted into each weapon's
        set (`weapon/<id>`), so a client without a key reads them as
        plain KeyValues;
     2. **a local install plus a key**: decrypted in memory at load;
     3. **the specs' numbers** (specs/tf2/weapons.md), the last resort
        for offline development runs, logged loudly.
   - Decrypted text lives only in the host's store, the per-user cache
     or under `target/`: never the repository or a shared scratch folder.
4. **The asset service.** `chimera_pack` learns `tf2` (as `cs_source`,
   `combat_arms` and `hl2dm`): the sets `base`, `common`, `players`,
   `weapon/<id>` (with the decrypted script) and `map/<name>`. Packing
   and tests use a scratch store under `target/`. Nothing goes into the
   live server store until the orchestrator publishes, when M2 is
   ready:
   `CHIMERA_TF2_CTX_KEY=... cargo run --features dev,tools --bin
   chimera_pack -- publish tf2` then `chimera_pack sets tf2 --store
   <server store> --sign` (the key from the host's environment only).
   TF2's first join passes the same no-fetch-in-match guard as the other
   games (`apps/ultimate_gun_game/tests/it/heavy/no_fetch_in_match.rs`).
5. **View models are two models**: the class's arms
   (`c_<class>_arms.mdl`, animations in `c_<class>_animations.mdl`) and
   the weapon (`c_<weapon>.mdl`) bone-merged onto them by bone name
   (install.md 4). The same weapon model is what the third-person
   player holds. If the view-model driver lacks an arms-plus-weapon
   pair, that is a generic extension in Chimera (Combat Arms already
   draws hands and gun parts), not TF2 code.
6. **The rungs' class.** A TF2 gun makes its holder that gun's class:
   Scout for the Scattergun, Soldier for the Rocket Launcher, Heavy for
   the Minigun, and the Engineer's arms for the Shotgun
   (weapons.md 2). Health and speed per class follow classes.md once
   the bodies land (M3); until then the rung uses the party's character.
7. **Not in scope for now**: TF2's own app and server (beside the party
   apps; M5 at the earliest), objectives (flags, control points,
   payloads), buildings, Medic healing, Spy disguise and cloak, Pyro's
   airblast, cosmetics, MvM, bots, random crits beyond the default
   rule, the HUD.

## 4. Milestones

### M0: the plan and the first specs (done)

specs/tf2/ ([README](../../../specs/tf2/README.md)): install.md (T0),
movement.md (T1), classes.md (T2), weapons.md (T3), ice.md (T4).
All `draft`.

### M1: a map you can walk on

- The `games/tf2` skeleton per new-game.md: `INFO` (Steam app 440,
  `required` above), `game::mount` from `tf/gameinfo.txt` through
  `mount::served` (the store works), `SimPlugins`, `Tf2Sim`, a test
  that the game registers and (heavy) that the install mounts.
- `MAP_LOADER` for `tf2:<map>`, lean option honoured; spawns from
  `info_player_teamspawn`; a guest `MapDressing`.
- `BUNDLE` in the party kit's list, maps only (no rungs yet).
- ctf_2fort and cp_badlands with textures (from the client),
  lightmaps, displacements, the 3D sky and collision, walked as an
  existing UGG character. Offscreen screenshots.
- Heavy tests: each loads with its spawns (both teams), faces,
  collision, no missing material (client) or stand-ins (server layout).
- `[games.tf2]` in `chimera.local.example.toml`; the install in
  docs/assets-for-developers.md.

What landed (2026-11-15):

- `games/tf2` (crate `tf2`) of new-game.md's shape: `game::INFO` (app
  440, folder `tf`, tick 0.015, namespaces `tf2` and `tf`), `game::mount`
  from `tf/gameinfo.txt` through `mount::served`, `SimPlugins`,
  `game::Tf2Plugins`, `harness::Tf2Sim`, the stand-in `tf2:movement`
  (placeholder, the Soldier's 240 / gravity 800 / jump 289), an empty
  `ui`, and `BUNDLE` (maps only: no guns, no pool maps, no guest dressing).
- Generic: the VPK reader leaves out the files of a numbered archive that
  is missing (`source_formats::vpk`, `Vpk::skipped`): the dedicated
  server's layout mounts, its textures missing (stand-ins).
  `SourceGame::spawn_rule` (`source_engine::content::bsp`): spawn points
  by a game's own rule; TF2's (`maps::team_spawn`): `info_player_teamspawn`
  by `TeamNum` (RED our team 1, BLU 2), not `StartDisabled`, `SpawnMode`
  1 or team 0, and on control-point maps only those whose point their team
  owns at round start (`point_default_owner`): 2fort 15 + 15, badlands
  16 + 16. A material name with its `.vmt` (2fort's water
  `$bottommaterial`) resolves. `$ssbump` (specs/cs_source/shaders.md 2)
  drawn by the world shader (`MapMesh::ssbump`): TF2's concrete and rock
  walls lost the dark speckles a self-shadowed bump map read as a normal
  map gave them. The shared conversion already read the LZMA lumps, the
  LZMA static-prop lump (v10) and the LZMA pak entries.
- The party (M1; since M2b TF2 is one of `party_kit::GUESTS`, and
  `MAP_GAMES` is gone): `party_kit::MAP_GAMES` (TF2), whose loaders join the
  party's map sources (`tf2:ctf_2fort` from `--map`, `changelevel`,
  the server), registered with its map logic; TF2's teams as Combat
  Arms' (RED Alpha, BLU Bravo) and its surfaces as CS:S's. Not in
  `GUESTS` until it lends parts (M2): the startup fetch, the binds and
  an automatic ladder's pool leave it out.
- `dump tf2`; `[games.tf2]` in chimera.local.example.toml; the two
  layouts in docs/assets-for-developers.md.
- Tests: `tf2`'s fast tier (registration, tick, standing, running and
  jumping on the greybox, the spawn rule); heavy (the mount's order, tf
  over hl2, the missing-archive skip; both maps converted with faces,
  collision, displacements, lightmap, sky, props, entities, spawns,
  `$ssbump`, no missing material; a lean load; the server layout with
  `CHIMERA_TF2_SERVER_ROOT`; the cache key; players standing and walking
  on 2fort); `party_kit`'s `heavy::tf2_maps` (the party on 2fort: 202
  spots, none stuck, a Combat Arms player lands and walks).

Left for later (M3/M4): the navigation files in tf2_misc don't parse
(TF2's per-area data differs from CS:S's; no spec yet), so TF2 maps have
no navigation mesh; one particle file's operator ("Remap Noise to
Scalar") isn't run; spawns don't move with captured points; on the
server's layout ctf_2fort draws fewer triangles (63,686 of 70,883; not
looked into; it collides the same).

### M2: TF2 guns as UGG rungs

- ICE and the `.ctx` reader (section 3, decision 3), with the three
  sources and the log line saying which one served.
- The Scattergun (pellets), Rocket Launcher (projectile with splash,
  self-damage, knockback), Shotgun and Minigun (spin-up, slowed holder)
  as TOML weapons (docs/weapon-data.md, the add-weapon-from-data skill),
  the numbers from the scripts, TF2's shared rules (distance ramp,
  spread, pellets) as registered hooks.
- `c_` weapons on the class's arms, sounds from the plain sound
  scripts.
- `chimera_pack tf2` and a scratch-store test; a gun game ladder with
  TF2 rungs among the other games' (no Halo).

#### M2a: what landed, and the publish (for the orchestrator)

- `games/tf2`: `game::ice` (ICE from ice.md; `Reading`, the spec's
  ambiguous places as switches, 55,296 combinations; `CtxCipher`, a level
  with a reading, named `thin/cycled-0123-bi-123`; `probe_ctx`),
  `game::scripts` (the key from `CHIMERA_TF2_CTX_KEY` or
  `[games.tf2.keys.ctx] key`; `read`, `ScriptSource`, `KvView`;
  `decrypting`, the packer's view), `game::sets` (`M2_WEAPONS`,
  `pieces`). The Shotgun's script is `tf_weapon_shotgun_primary`
  (the Engineer's).
- **The ICE reading is not settled.** None reproduces the paper's
  triplets (ice.md, open question 1). The user runs, with the key in the
  environment only:
  `CHIMERA_TF2_CTX_KEY=... cargo run --features dev --bin dump -- tf2 --ice-probe`
  It prints the name of each level and reading that decrypts the Rocket
  Launcher's script. Then `CtxCipher::DEFAULT` becomes that name (until
  then `CHIMERA_TF2_ICE_READING=<name>` selects it), and
  `CHIMERA_TF2_CTX_KEY=... cargo test -p tf2 --features dev --test it heavy::`
  checks every weapon and class script with it. If it prints nothing,
  open question 1 needs a spec session.
- **The publish** (not done; after the probe, with the key from the
  host's environment only; `chimera_pack` warns loudly and leaves the
  scripts out without one):
  ```
  CHIMERA_TF2_CTX_KEY=... CHIMERA_TF2_ICE_READING=<the probe's name> \
    cargo run --features dev,tools --bin chimera_pack -- publish tf2
  CHIMERA_TF2_CTX_KEY=... CHIMERA_TF2_ICE_READING=<the probe's name> \
    cargo run --features dev,tools --bin chimera_pack -- sets tf2 --store <server store> --sign
  ```
  (`CHIMERA_TF2_ICE_READING` only until the probe's name is the default.)
  `base` and `common` come from client runs' `CHIMERA_VFS_LOG` logs
  (`pack tf2 --set base --from <log>`, then `lean`), as for HL2:DM.

#### M2b: what landed (2026-11-15)

- **The guns as data** (`games/tf2/data/weapons`, `game::weapons`):
  `tf2:tf_weapon_scattergun`, `tf2:tf_weapon_rocketlauncher`,
  `tf2:tf_weapon_shotgun_primary` (the Engineer's) and
  `tf2:tf_weapon_minigun` with specs/tf2/weapons.md's numbers, classes
  shotgun, launcher, shotgun, heavy. `weapons::script_numbers` asks
  `scripts::read` for each gun's script at every load of the guns and
  logs (a warning) where the numbers came from: "spec fallback" until
  `numbers_from_script` maps the scripts' fields (no spec names them yet).
- **The rules as hooks** (scenario tests on `Tf2Sim`,
  `tests/it/weapons.rs`): `tf2:gun`, the distance modifier (smoothstep,
  the 921.6 clamp, the near half x1.5 / x0.5 / x1) on each target's summed
  pellets rounded to whole points, the first pellet dead centre after
  0.25 s, the reload's start by class; `tf2:rocket`, the rocket on
  Chimera's flyers (1100 u/s from the spec's offset, through teammates)
  and its blast (146 units, 50 % at the edge, the touched player full,
  the owner's own x0.6 airborne, the push 6 / 10 / 5 per point: a rocket
  jump lifts the Soldier); `tf2:minigun`, the spin (0.75 s up, 1.15 / 1.4
  down, 110 u/s, no jump, no switch) and the 2017 damage ramp. Shells one
  at a time, a shot stops them (Chimera's `ShellReload`). Random crits
  and the damage spread's jitter stay off (the server defaults are spread
  off; crits off is the party's choice); the power curve reads the
  modifier, the blast and the spin-up.
- **View models** (`game::view`): the class's arms with the `c_` weapon
  bone-merged by bone name; the draw, idle, fire, reload start/shell/end
  and the Minigun's spin up/idle/down picked by the spec's activities;
  `tf2_viewmodel_fov` 54. The same `c_` model is the held one (on a
  body with a `weapon_bone`: TF2's, M3).
- **Sounds**: the spec's entries per gun (fire, empty, reload, draw, the
  Minigun's wind-up, spin, fire, dry and wind-down loops on the weapon
  channel, the rocket's `BaseExplosionEffect.Sound` heard by every client).
- **The party**: TF2 in `party_kit::GUESTS` (`MAP_GAMES` gone), its
  `ui::party::CLIENT` in `CLIENTS` (no HUD of its own), `BUNDLE` with its
  parts (the four guns, `tf2:movement`: playable in an automatic ladder),
  `GUEST` dressing, `kept`, names, the view-model FOV. `weapon/<id>` sets
  renamed to the guns' ids past `tf2:` (`weapon/tf_weapon_scattergun`), as
  a match's offer names them, and read as a match loads the gun
  (`maps::add_guns`; heavy test `guns::each_guns_set_covers_what_a_match_
  loads_for_it`). `originals:gun_game_2fort` (minigames/
  gun_game_2fort.toml): TF2's four guns among CS:S's, HL2:DM's and Combat
  Arms', no Halo (`party_kit` `tf2_rungs.rs`: a TF2 rung gives its gun
  and TF2's identity, a kill climbs to the next game's, the rocket kills).
- **Generic**: `DamageEffect::by_distance` (a game's damage per target
  by the centres' distance), `Hitscan::centre_first`, the flyers'
  `offset`/`offset_crouched`/`pass_teammates`, the placeholder movement
  flying after a push that cleared `on_ground`, `map::merge_by_bone_name`
  (arms plus a bone-merged weapon as one view model), and Source's
  animation loader putting an include's real sequence in place of the
  model's same-named 1-frame stub (install.md 4.3). Network version
  `net57`.

Left (M2b): the scripts' field mapping (needs a spec written with the key,
then the ICE probe and the publish above; the sets are now named
`weapon/tf_weapon_*`); the knockback per point is ours (open question 9,
probe P3), bullets push nobody; the Minigun's first-person idle sits low
and angled up (the Heavy arms' `m_idle` as decoded puts `weapon_bone`
0.9 m below the eye); no explosion look (particles, scorch), no muzzle
flash seen in first person; the rocket's crouched offset and the
per-class shotgun starts wait for classes (M3); a TF2 rung has no knife.

### M3: the classes as bodies, TF2's movement

The class player models with animation (activities by weapon, pose
parameters), health per class, TF2's movement numbers (movement.md:
the jump, the air duck, the Scout's air jump, the bunny-hop cap, fall
damage), the rocket jump tuned on the probe server.

#### M3: what landed (2026-11-15)

- **The classes as data** (data/classes.toml, `game::classes`): the
  nine classes' index, max health, top speed, eye height, air jumps,
  player model and arms (classes.md), and each gun's class (`[guns]`: a
  new gun joins with a line; a gun not listed is its `tf2:gun` hook's
  `arms`). Already listed for the other track's guns: the Pistol (Scout),
  SMG and Sniper Rifle (Sniper), Revolver (Spy), Grenade and Stickybomb
  Launchers (Demoman), Flame Thrower (Pyro), Syringe Gun (Medic).
- **TF2's movement** (`game::movement`): Source's movement with TF2's
  config (movement.md): gravity 800, accelerate 10 and 10, the 48 x 48 x
  82 box (62 ducked, eyes 45), the jump 289, the 0.2 s duck counted from
  the press's tick, the whole 20-unit lift in the air and two air ducks,
  no ground jump fully ducked, backwards 0.9 and ducked a third of the
  class speed, the bunny-hop cap 1.2 x the tick's top speed, the 520
  ceiling, fall damage max health x speed / 6000 above 650 with its +-20 %
  spread (`tf2:tf_fall_damage_disablespread`). The class of the gun in
  hand (`select_class`, predicted) gives the speed (or the weapon's own
  `max_speed`: the spun Minigun's 110), the eyes and the Scout's air jump
  (vz 268.33, the horizontal velocity the keys' wish). The server gives
  each TF2 player its class's max health, the share of health kept; a
  player leaving TF2's movement gets back the max it had.
- **Bodies** (`game::bodies`, `game::player_anim`): the classes' player
  models for the guns a map's kit wants, worn by the gun's class by name
  (the Soldier the unnamed default), the team's skin (0 RED, 1 BLU); the
  head hitbox on `bip_head`; animation on Source's shared player
  animation: `ACT_MP_<state>_<kind>`, the jump's start then float, the move
  grid by `move_x`/`move_y`, `body_pitch`/`body_yaw` with the aim matrix
  autolayers, fire, reload and landing gestures; the `c_` gun on
  `weapon_bone`.
- **HUD** (`ui`, `game::hud`): health (the cross filled by the share of
  max, the number, the overheal and low-health pulses), ammo (clip and
  reserve, or one number) in the team's colour, the crosshair (the
  sheet's "( · )"), in the install's scheme fonts and colours from the
  map's kit, under its own `HudRoot` by H/480. Its guns' kill icons cut
  from `mod_textures.txt`'s cells of `hud/d_images` for every app's feed
  (the feed's rows are the party's).
- **Generic**: `SourceMovementConfig` fields (`half_width`, `duck_time`,
  `duck_counts_first_tick`, `bunnyhop_cap_scale`, `back_speed`,
  `air_jumps`/`air_jump_speed`, `no_ducked_jump`, `air_ducks_max`,
  `fall_by_max_health`/`fall_spread`) and the per-character
  `SourceTraits` (speed, eyes, air jumps); Source's model loader cuts each
  `.vtx` model's meshes to its `.mdl` model's (an empty body-group choice
  listing a mesh shifted every later one: the Heavy didn't load); the
  HUD look reader takes `CustomFontFiles` blocks and `dfile`/`dnegfile`
  sheets; Combat Arms takes off only the skins it set (it took TF2's team
  skin off every frame). Network version `net62`.
- **Tests**: movement.md's cases 1-3, 5-8, 9-16, 18-20, 22, 23 and
  classes.md's 1 (`tests/it/movement.rs`); the HUD's 1-6, 8-11
  (`ui::hud`); heavy: the bodies' sequences, poses, head and skins, a
  running Scout's grid, the HUD look and the kill icons
  (`heavy/bodies.rs`); party: a TF2 rung's class health and speed, back
  on the next game's rung, TF2's HUD following its gun, the overlay
  guard with TF2's HUD.

Left (M3): the Heavy's spun ("deployed") sequences and spin gestures, the
flinch, the air jump's own sequence, a weighted pick among sequences of
one activity; body groups (the Soldier's rocket in hand); the
max-health label, low-ammo pulse, damage numbers, health-gain number,
class portrait and each gun's own crosshair (hud.md open questions); the
kill feed's row look is the party's; overheal and the Medic's
regeneration have no source yet (no Medi Gun); the rocket jump tuned on
the probe server.

### M4 and on

More maps in the pool (koth_, cp_, ctf_ with the map pool's fitness
data), more weapons (each class's stock set), effects, the dedicated
server probe for parity checks, TF2's own app.

## 5. How the session works

- Specs first, by sub-agents that don't build; one building sub-agent at
  a time (the box runs other agents' builds), in an agent slot
  (`tools/agent-slot.sh acquire tf2-<step>`) with
  `source tools/agent-env.sh && agent_run cargo ...`.
- Each step merges to main when its tests pass, then
  `tools/post-push-tests.sh`; a FAIL is fixed at once.
- Ports: never UDP 27015 or 27030 (the probe servers use 272xx).

## 6. Open questions

1. ICE: the paper's test vectors don't reproduce from its prose alone
   (ice.md, open question 1). The readings the spec lists are tried
   against a real `.ctx` with the user's key: KeyValues text settles it.
2. Does the engine fall through to later search paths for a file whose
   archive is missing (install.md)? Our mount does; it matters only for
   the server layout.
3. Spawn choice on control-point maps at round start (install.md 2):
   which teamspawns a gun game uses (both teams' at once, for free for
   all).

## 7. Backlog

- Rendering (from M1's screenshots): fences and grates in 2fort and some
  ground decals draw as a coarse dither pattern; check how TF2's VMTs'
  `$alphatest` and `$translucent` are handled. Badlands' track bed and
  some floor patches show dotted dark speckle, maybe the same cause.

## Log

- 2026-11-15: plan written; five specs landed (install, movement and
  classes measured on the dedicated server with puppet bots, weapons
  from public documentation and the item schema, ICE from the paper).
  The dedicated server lacks textures and sounds, so the retail client
  (app 440) was installed for them. Two decisions from the user: the
  weapon scripts are decrypted with our own ICE and a locally supplied
  key, and the asset service is the default source of game data
  (section 3).
- 2026-11-15: M1 landed: `games/tf2` with its mount, maps (team spawns)
  and stand-in movement; ctf_2fort and cp_badlands walked in the
  Ultimate Gun Game (`--map tf2:ctf_2fort`); generic: missing VPK
  archives skipped, `SourceGame::spawn_rule`, `$ssbump`.
- 2026-11-15: M2a landed: ICE written from ice.md (it matches the spec's
  own derived values; none of 55,296 readings of its ambiguous places
  reproduces the paper's triplets, so `dump tf2 --ice-probe` with the
  user's key settles the reading), the script reader with its three
  sources, `chimera_pack tf2` with M2's weapon sets (packed into a
  scratch store by a heavy test and read back from it alone; not
  published). Generic: `[games.<id>]` may hold keys without `path`, a key
  without an IV; `chimera_core::mount::Access` re-exported.
- 2026-11-15: M2b landed: the four guns as data with TF2's rules as hooks
  (the distance modifier, the first pellet, the rocket's blast and jump,
  the Minigun's spin), the `c_` weapons bone-merged on the class arms
  (generic `map::merge_by_bone_name`; the include's sequences over the
  arms' stubs), sounds, TF2 in the party's `GUESTS` and
  `originals:gun_game_2fort`; the scripts' numbers a logged seam.
  Screenshots in UGG on ctf_2fort and de_dust2.
- 2026-11-15: M3 landed: the classes as data, TF2's movement on Source's
  with the gun's class (speed, eyes, the Scout's air jump, max health),
  the class bodies with team skins and animation, TF2's HUD and kill
  icons. Screenshots in UGG on ctf_2fort and de_dust2 (each gun's HUD, a
  third-person Heavy, a Scout mid air jump).
