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
| M2a the weapon scripts, `chimera_pack tf2` | done (2026-11-15) | ICE (`game::ice`, ported from the designer's public-domain reference, 2026-11-15; the published triplets pass); `game::scripts` (store/plain, install plus key, none); `dump tf2 --script`, `--ice-probe`; `chimera_pack tf2` (`weapon/<id>` for M2's guns with the decrypted script, `players`, `map/<name>`); not published. Every `.ctx` script (88) decrypts with Thin-ICE and the user's key |
| M2b TF2 guns as UGG rungs | done (2026-11-15) | the four guns as data with the spec's numbers (the scripts' field mapping a seam), TF2's gun rules as hooks, `c_` weapons bone-merged on the class arms, sounds, TF2 in the party's `GUESTS`, `originals:gun_game_2fort` |
| More guns (track 1, tf2-guns2) | done (2026-11-15) | the next eight guns (specs/tf2/weapons_more.md) as data and hooks, lent as rungs; the Minigun held up in first person; muzzle flashes and explosions |
| M3 classes as bodies, TF2's movement | done (2026-11-15) | the classes as data (`game::classes`); `tf2:movement` on Source's movement with TF2's config and the held gun's class (speed, eyes, the Scout's air jump, max health); the class bodies with team skins and animation; TF2's HUD (health, ammo, crosshair) and kill icons |
| M4a more maps in the party pool | done (2026-11-15) | 14 koth_, cp_ and ctf_ maps in an automatic pool by data/map_pool.toml and data/map_fitness.toml (a sweep of 130 maps); played free for all: touch-opened doors start open, spawn rooms' door brushes block nobody; `map/<name>` sets checked in a scratch store, not published |
| SDK parity (tf2-parity) | done (2026-11-15) | the specs' "Corrections from the SDK" in the code: the Minigun, the blasts and knockback, movement, the scripts' mapping, grenades and stickies; the flame's points, heat and afterburn, the scope's timing, the needle's and the rocket's aim (what's left: the section) |
| M4b the nine stock melee weapons | done (2026-11-16) | specs/tf2/weapons_stock.md (T10): the Bat, Shovel, Fire Axe, Bottle, Fists, Wrench, Bonesaw, Kukri and Knife as data on Chimera's delayed swing (the smack), the Knife's backstab, the class's arms and sequences, sounds, kill icons with `backstab`; each TF2 rung carries its class's melee, TF2's kit knife the Knife |
| Follow-ups (tf2-followups) | done (2026-11-16) | the drawn class's melee tested; spawn spots in open space (no vent-duct spawns); TF2's class parts on and off in one move; the duct's material checked |
| M4 more weapons, effects, the probe | not started | |

## 1. Rules for this port

- **Clean room.** TF2's and the Source engine's code leaked; the leak
  is never read, copied or translated. Valve's public Source SDK 2013,
  which has held TF2's client and server game code since 2025-02-18
  (non-commercial licence), is reference material for spec sessions
  only (user, 2026-11-15). Spec sub-agents may read it in place, along
  with the game's data files, public documentation (the community TF2
  wiki, Valve Developer Community) and measurements of the dedicated
  server. They write only `specs/tf2/`, in their own words, never
  copying or translating code. The orchestrator and implementation
  agents read only `specs/` and the repository.
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
6. **The rungs' class.** A TF2 gun makes its holder one of the classes
   that carry it in stock TF2 (games/tf2/data/classes.toml `[guns]`, one
   class or a list; weapons.md 1, weapons_more.md 1, classes.md "Stock
   loadout"): Scout for the Scattergun, Soldier for the Rocket Launcher,
   Heavy for the Minigun. A gun of several classes gets its class **at
   random, before the match** (the user's rule, 2026-11-15): when the
   ladder is drawn, TF2's rung extras (`classes::HOOK`, Chimera's
   `ladder::RungExtrasHook`, as Combat Arms' rung characters and CS:S's
   rung models) draw one of the gun's classes for each TF2 rung from the
   ladder's seeded generator, kept in the ladder's extras the server
   shares with its clients (`NetMinigame::rung_extras`); every player
   and bot on the rung is that class on every machine (`RungClass`
   from `ladder::OnRung`), and the class shows after the gun in the
   HUD's rung line. Never rolled per respawn. The offer names only the
   drawn classes' body sets (`bodies::asset_names`). Where nothing is
   drawn (a kit gun, the sandbox, `rung_extras = false`) a gun is its
   first class, its own.
   - **The Shotgun** (items 9, 10, 12, 11: Engineer, Soldier, Pyro,
     Heavy) stays one rung gun, `tf2:tf_weapon_shotgun_primary`: the
     class scripts agree on every mapped number (weapon_scripts.md 3),
     so its numbers are the Engineer's script's; arms and first-person
     sequences (weapons.md 8.2: the `SECONDARY` ones and their names for
     the three secondaries, `view::CLASS_SEQUENCES`), the reload's start
     (4: 0.4667 / 0.5 / 0.3333 / 0.3333) and the third-person stance
     (secondary but for the Engineer) follow the drawn class.
   - **The Pistol** (items 23 and 22: Scout, Engineer) likewise one gun,
     `tf2:tf_weapon_pistol_scout` (the scripts agree too): the
     Engineer's arms and `pstl_` sequences and his clip reload of
     1.035 s (weapons_more.md 1) follow his class; he carries the
     Scout's 36 rounds, not his 200 (a shortcut).
   - The view models of a gun's other classes are keyed `<gun>#<class>`
     (Chimera's `view_anim::ViewVariant`, a generic extension) and load
     where that class's arms are here (its `players/<class>` set, which
     the offer names when drawn); a gun's `weapon/<id>` set keeps its own
     class's arms only.
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

- `games/tf2`: `game::ice` (ICE, since 2026-11-15 a port of the
  designer's public-domain reference, THIRD_PARTY.md; `CtxCipher`, the
  level, `thin` by default; `probe_ctx`),
  `game::scripts` (the key from `CHIMERA_TF2_CTX_KEY` or
  `[games.tf2.keys.ctx] key`; `read`, `ScriptSource`, `KvView`;
  `decrypting`, the packer's view), `game::sets` (`M2_WEAPONS`,
  `pieces`). The Shotgun's script is `tf_weapon_shotgun_primary`
  (the Engineer's).
- **ICE settled (2026-11-15).** The prose-only ICE never reproduced the
  paper's triplets (ice.md, open question 1); the user approved the
  designer's public-domain reference as a source and `game::ice` is now
  its port: the three triplets pass, and `dump tf2 --ice-probe` with the
  user's key decrypts every one of the install's 88 `.ctx` scripts at
  Thin-ICE (level 0, the default; `CHIMERA_TF2_ICE_LEVEL=ice` would pick
  level 1). `CHIMERA_TF2_CTX_KEY=... cargo test -p tf2 --features dev
  --test it heavy::` checks every weapon and class script.
- **The publish** (not done; after the probe, with the key from the
  host's environment only; `chimera_pack` warns loudly and leaves the
  scripts out without one):
  ```
  CHIMERA_TF2_CTX_KEY=... \
    cargo run --features dev,tools --bin chimera_pack -- publish tf2
  CHIMERA_TF2_CTX_KEY=... \
    cargo run --features dev,tools --bin chimera_pack -- sets tf2 --store <server store> --sign
  ```
  `base` and `common` come from client runs' `CHIMERA_VFS_LOG` logs
  (`pack tf2 --set base --from <log>`, then `lean`), as for HL2:DM.

#### M2b: what landed (2026-11-15)

- **The guns as data** (`games/tf2/data/weapons`, `game::weapons`):
  `tf2:tf_weapon_scattergun`, `tf2:tf_weapon_rocketlauncher`,
  `tf2:tf_weapon_shotgun_primary` (the Engineer's) and
  `tf2:tf_weapon_minigun` with specs/tf2/weapons.md's numbers, classes
  shotgun, launcher, shotgun, heavy. `weapons::script_numbers` asks
  `scripts::read` for each gun's script at every load of the guns and
  logs where the numbers came from. Since 2026-11-15 the scripts' numbers
  are applied (specs/tf2/weapon_scripts.md, see "Real stats" below).
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

Left (M2b): the knockback per point is ours (open question 9,
probe P3), bullets push nobody; the Minigun's first-person idle is `m_idle` again (upright, barrels up,
right of centre: a capture of the real game showed the same on
2026-11-15; tf2-guns2's spun pose and `view_offset` are gone); no explosion look (particles,
scorch), no muzzle flash seen in first person (both drawn since
tf2-guns2; no scorch yet); the rocket's crouched offset and the
per-class shotgun starts wait for classes (M3); a TF2 rung has no knife.

#### More guns: what landed (tf2-guns2, 2026-11-15)

- **The Minigun in first person**: its `m_idle` (and the ends of `m_draw`
  and `m_spool_down`, which settle into it) holds the gun upright below
  the eye with the left hand off its grip, consistently across those
  sequences as we decode them: not the bone merge, not the include's
  root (the same in the spun sequences, which aim right), not
  `min_viewmodel_offset` (the minimal view models' only). The first
  person shows the spun pose (`m_spool_up` to draw, `m_spool_idle` to
  idle and wind down), raised by the gun's `[models] view_offset`
  (`game::view`), matched by eye. The converted-map cache key now hashes
  `data/` (a view model's data change was served stale).
- **Eight guns** (specs/tf2/weapons_more.md): the Scout's Pistol, the SMG,
  the Sniper Rifle, the Revolver, the Grenade and Stickybomb Launchers,
  the Flame Thrower and the Syringe Gun, as data with the spec's numbers
  (data/weapons), their classes' arms and sequences (picked by name: the
  Pyro's `ft_idle` shares its activity), sounds, kill icons, gun classes,
  `weapon/tf_weapon_*` sets; lent through `PARTS` (the automatic ladders
  draw them) and the classes table. Hooks: `tf2:sniper` (the scoped
  charge 50 to 150, the headshot crit after 0.2 s scoped; Chimera's scope
  at 20 degrees and 80 units/s, auto re-zoom), `tf2:grenade` (explodes on
  an enemy before touching anything, else bounces and rolls to its 2.3 s
  fuse; flat 100), `tf2:sticky` (charge 900 to 2400 over 4 s, sticks to
  the world, arms at 0.7 s, attack2 detonates, at most 8, the air radius,
  the 5 s ramp), `tf2:flame` (a 350-unit cone tested each 0.075 s with
  the age and burn ramps, afterburn 4 per 0.5 s for 3 to 10 s) and
  `tf2:needle` (990 units/s falling at 0.3 of gravity, 10 by distance).
  The rocket, grenade and sticky share one explosion (`weapons::blast`).
- **Looks** (`weapons::looks`): TF2's `ExplosionCore_MidAir` and each
  gun's `muzzle_*` from its own `.pcf` files, loaded with its maps;
  Source's explosion on maps with its sprites; explosions sent to clients
  (the `tf2_looks` net part). (tf2-polish, 2026-11-15) The same on every
  map: TF2's systems (and the Flame Thrower's `flamethrower` /
  `flamethrower_blue`, a piece per round) load onto other games' maps from
  TF2's own files (`maps::add_guest_looks`, `pcf::
  add_guest_particle_systems`), no Source explosion; flashes start at the
  gun's `muzzle` attachment; TF2 is an impact provider (Source's impact
  effects and sounds, `looks::STYLE`), so its hits are never Combat
  Arms' or the map's game's; the `players` set holds the files.
- **Generic**: `chimera:projectile`'s `up_speed`. Network version net63.
- **Tests**: `tests/it/weapons_more.rs` (13 scenario tests from the
  spec's cases), the heavy `guns` tests over all twelve, `party_kit`'s
  `tf2_rungs::the_next_eight_tf2_guns_are_rungs_and_kill`.

Left: the flash is drawn in the world pass at the re-projected muzzle,
not on the view-model layer; the headshot kill icon isn't told apart; no laser
dot or charge meter (no TF2 HUD); stickies aren't destroyed by bullets,
a ninth blows the oldest at once; the grenade's bounce (restitution,
friction) and the projectiles' boxes are ours; no crits; Pyros aren't
immune to afterburn, nothing puts a burn out but death.

#### Real stats from the scripts (2026-11-15)

- specs/tf2/weapon_scripts.md (a spec session with the key): the
  scripts' fields, their meaning and units, and the mapping onto the
  TOML: `Damage` (per pellet, blast, needle, or per second for the
  flame; not the Sniper Rifle's), `BulletsPerShot`, `Spread`,
  `TimeFireDelay`, `clip_size`, `TimeReload` (shell guns), `DamageRadius`
  (Demoman launchers); `ProjectileType` and `WeaponType` checked.
- `script_numbers::GunNumbers`: the documents TF2's guns register from,
  the scripts' (through `INFO.mount()`: the store, or the install with
  the key; an encrypted install copy without a key falls through to the
  store's plain one) over the data, each gun's source logged at start;
  registered as the server documents `tf2:weapons`, so a client without
  scripts plays with the server's numbers. The harness keeps the
  committed numbers (`GunNumbers::builtin`). `dump tf2 --gun-numbers`
  shows each gun's source and what differs from the data (names only).
- With the user's key all twelve guns read their script; the mapped
  numbers equal the data's except the SMG's, Minigun's and Syringe Gun's
  interval (nominal, not tick-rounded: weapon_scripts.md open question
  3) and the Rocket Launcher's per-rocket reload (open question 6), as
  the spec predicted.
- Scratch store (2026-11-15): TF2 packed from two client runs' reads
  (`base` from de_dust2: 758 files, every gun dressed; `map/ctf_2fort`:
  3,220 files) and `sets tf2 --only weapon/` and `--only players`; a
  client with no install, no key and the store alone
  (`CHIMERA_ASSET_USE_INSTALL=0`) loads ctf_2fort and reads all twelve
  guns' numbers from the stored scripts. Not yet in the live store: the
  commands are in docs/playtest-hosting.md, "TF2". The fetch guard
  (`no_fetch_in_match`) plays `tf2:ctf_2fort` last when TF2 is installed
  and published.

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
max-health label, low-ammo pulse, damage numbers, health-gain number
and each gun's own crosshair (the class portrait is drawn as its 2D
picture since tf2-polish) (hud.md open questions); the
kill feed's row look is the party's; overheal and the Medic's
regeneration have no source yet (no Medi Gun); the rocket jump tuned on
the probe server.

### M4a: more maps in the party pool (done 2026-11-15)

- **The pool** (games/tf2/data/map_pool.toml): every installed map denied
  but 14 allowed: the start set but cp_well (ctf_2fort, cp_badlands,
  koth_viaduct, koth_harvest_final, koth_nucleus, cp_granary, ctf_turbine,
  ctf_doublecross, koth_lakeside_final, cp_process_final) and four stock
  KOTH maps the sweep rates well (koth_king, koth_highpass,
  koth_sharkbay, koth_suijin). Lent through `parts::add_map_pool` (from
  `party_sim`, as HL2:DM and Halo): `PARTS.maps` stays empty, so a machine
  without TF2 (install or store) is offered none, and one with it gets
  the allowed maps its mount lists (`auto::InstalledMaps`), drawn by the
  players present.
- **The measurements** (games/tf2/data/map_fitness.toml, `map_fitness
  --game tf2`, which learned TF2): every koth_, cp_, ctf_ and arena_ map
  (130 rows). The pool's:

  | map | area m2 | sight m | spawn points | spots | off | stuck | wander 4/8/12 s | in from |
  |---|---|---|---|---|---|---|---|---|
  | ctf_2fort | 7748 | 7.6 | 30 | 4053 | 0 | 0 | 21.1/9.1/5.8 | 8 |
  | cp_badlands | 13948 | 11.1 | 32 | 12198 | 0 | 0 | 26.0/11.2/7.1 | 12 |
  | koth_viaduct | 6294 | 10.8 | 32 | 4799 | 0 | 0 | 12.0/5.1/3.3 | 8 |
  | koth_harvest_final | 6135 | 12.9 | 30 | 3971 | 0 | 0 | 9.8/4.2/2.7 | 4 |
  | koth_nucleus | 6278 | 11.2 | 36 | 6038 | 0 | 0 | 11.5/4.9/3.1 | 8 |
  | cp_granary | 14918 | 12.2 | 30 | 9561 | 0 | 0 | 25.3/10.8/6.9 | 12 |
  | ctf_turbine | 5661 | 9.9 | 24 | 3500 | 0 | 0 | 11.8/5.1/3.2 | 8 |
  | ctf_doublecross | 8528 | 11.5 | 32 | 6572 | 0 | 0 | 15.3/6.6/4.2 | 8 |
  | koth_lakeside_final | 8327 | 12.3 | 26 | 5996 | 0 | 0 | 14.0/6.0/3.8 | 8 |
  | cp_process_final | 12900 | 11.2 | 32 | 10461 | 0 | 0 | 23.8/10.2/6.5 | 12 |
  | koth_king | 8267 | 12.2 | 32 | 7416 | 0 | 0 | 14.0/6.0/3.8 | 8 |
  | koth_highpass | 7360 | 10.4 | 32 | 6356 | 0 | 0 | 14.5/6.2/4.0 | 8 |
  | koth_sharkbay | 7176 | 10.8 | 34 | 5046 | 0 | 0 | 13.6/5.8/3.7 | 8 |
  | koth_suijin | 8951 | 15.1 | 32 | 5678 | 0 | 0 | 12.2/5.2/3.3 | 8 |

  (spots as counted since tf2-followups' open-space filter, below.)

  Left out (the file's comment says which and why): attack/defend cp_
  maps and every arena_ map (the round logic opens their gates and arena
  doors, which a party never runs: the spots stay in the spawn rooms or the
  first stage, arena_well 206 m2, cp_dustbowl 949 m2), maps whose spots
  cover a small part of their area (koth_badlands 410 spots on 6612 m2,
  koth_sawmill, cp_freight_final1, cp_burghausen, cp_premuda, arena_perks),
  cp_mojave (no spawn point at round start), cp_well (of the start set:
  the bots' mesh splits it in two at the middle, each team's half about
  8000 m2 and never joined, its 15510 m2 their sum; found by the walk-out
  test, not looked into further), the largest (cp_snowplow,
  cp_sulfur, koth_megaton, ctf_gorge) and the holiday twins.
- **Free for all** (`maps::free_for_all_doors`, through a new generic hook,
  `SourceGame::entity_rule`: a game's edits to its maps' entities before
  the spawns and the logic read them): every door a trigger opens on touch
  starts open (`spawnpos` 1); the triggers' team filters
  (`filter_activator_tfteam`, unknown to the logic) let anyone through, so
  the doors open for everyone and close again behind the last toucher.
  The spawn spots, walked from the spawn points through the world as it
  stands, had stayed in the spawn rooms (ctf_2fort 202 spots, 362 m2;
  now 4054, 7748 m2). TF2's volumes (`func_respawnroom`, `_visualizer`,
  `func_regenerate`, `func_nobuild`, the flag zones) neither draw nor
  collide (`source_engine::content::bsp::brush_entities`): the spawn rooms'
  door brushes, solid to the room's enemies in TF2, shut nobody out.
  Spawning anywhere (`points = "scored"`/`"random_hidden"`) keeps out of
  the spawn rooms in a free for all (2026-11-21, a playtest's "you spawn
  inside the normal spawns"): the conversion keeps `func_respawnroom`'s
  volumes (`MapEntity::hulls`), TF2 registers the class as its spawn rooms
  (`maps::SPAWN_ROOMS`, `chimera_world::map::SpawnRooms`, any game can),
  and the spots leave them out unless the match is a team one
  (`chimera_sim::minigame::spawns`). Test: party_kit
  `heavy::tf2_maps::a_free_for_all_spawns_outside_the_spawn_rooms`.
- **Fixed on the way**: a material path with `..` (koth_viaduct's
  `models/props_trainyard/../props_forest/train_gate001_forest`) resolves
  (`chimera_vfs::normalize` steps up a folder); `chimera_pack sets tf2
  --only map/<name>` records that map's set alone (it rewrote the weapon
  and players sets too).
- **Tests**: `party_kit` `heavy::tf2_maps::a_player_walks_out_of_every_
  spawn_room_of_the_tf2_pool` (every pool map: spots at least 2000, none
  stuck, every spawn point on the main region; from each spawn room a
  player walks the bots' generated mesh toward the middle and gets out,
  20 m or more from every spawn point of its room; and the room's own
  region holds the middle), `tf2` `spawns::doors_
  a_trigger_opens_on_touch_start_open`, `chimera_vfs`
  `normalize_steps_up_for_dot_dot`; UGG's `heavy::pool_maps` checks TF2's
  manifest too (`CHIMERA_POOL_ONLY=tf2:` with `CHIMERA_POOL_MAPS=all`; in
  that mode a map is judged at the least count it is in at). Against a
  scratch store (the TF2 files the client runs read, packed, then `sets
  tf2 --only map/<name>`): all load with spots and 0 files read outside
  their sets.
- **The packing** (for the orchestrator; not done): each pool map's set,
  `--pack` packing the files it reads that the store lacks (or after
  `publish tf2`, the whole install, without it):
  ```
  for m in ctf_2fort cp_badlands koth_viaduct koth_harvest_final koth_nucleus \
           cp_granary ctf_turbine ctf_doublecross koth_lakeside_final \
           cp_process_final koth_king koth_highpass koth_sharkbay koth_suijin; do
    cargo run --features dev,tools --bin chimera_pack -- sets tf2 --store <server store> --only map/$m --pack --sign
  done
  ```
  (`--only` is a prefix: `map/koth_viaduct` records `koth_viaduct_event`
  too, harmless.) No key is needed for map sets.
- Left: the pool's maps play without their objectives (no points, flags or
  carts) and without the round logic (setup gates stay shut, so
  attack/defend and arena maps are out); spawns don't follow captured
  points; the navigation files still don't parse (the party generates
  one).

### M4b: the nine stock melee weapons (done 2026-11-16)

specs/tf2/weapons_stock.md (T10, draft).

- **As data** (data/weapons, the base `tf2:melee`, `weapons::melee`):
  `tf2:tf_weapon_bat` (35 every 0.5 s), `_shovel`, `_fireaxe`, `_bottle`,
  `_fists`, `_wrench`, `_bonesaw`, `_club` (the Kukri) (65 every 0.8 s)
  and `_knife` (40 every 0.8 s): a 48-unit line then a +-18 box, no
  facing limit, the collision box, whole points, no knockback; slot 2,
  never dropped, no gun class (never a ladder gun). The scripts' `Damage`,
  `TimeFireDelay` (hit and miss refires) and `SmackDelay` (0.2 s without
  one) apply at run time (`script_numbers::apply_melee`; a melee script on
  a gun, or a gun's on a melee weapon, is refused).
- **The hit delay** (open question 8) is a generic melee field,
  `Swing::delay` (`delay` in the TOML): the swing starts
  (`WeaponEventKind::SwingStarted`, `sound_swing` on every swing, both
  timers as after a miss) and is traced `delay` later along the view
  then (`WeaponState::pending_swing`; 14 ticks, the Knife's 7), the hit
  refire counted from the start; a draw or the owner's death cancels it.
- **The backstab** is a generic rule, `Backstab::Behind` (`backstab = {
  behind, facing, same_way, health, credit }`): d.v > 0, a.d > 0.5, a.v >
  -0.3 flattened, decided at the smack from the positions then (open
  question 4), 6 x the victim's current health, the kill credited to
  `tf2:backstab` (its kill icon); CS:S's knife keeps its number
  (`Backstab::Facing`). The raised knife (`melee::backstab_ready`):
  `knife_backstab_up`, `_idle`, `_down` and the stab `knife_backstab` by
  the weapon's `ViewPose`.
- **Crits** (`tf2:melee`): rolled per swing before it starts (`prepare`):
  x3 damage, the crit swing sound and sequence, `TFPlayer.CritHit` on the
  hit (and on a backstab). Random crits stay **off** (`MeleeCrits`, the
  party's choice, as for the guns); with them on the chance is the base
  15 % (`crit_chance`, no damage history yet). The Knife never rolls.
- **First person** (`game::view`, `MELEE_SEQUENCES`): each class's arms
  with the `c_` model bone-merged (the Fists the Heavy's arms alone, in
  skin 2, `props::load_view_model_skinned`); draw, idle, the weighted
  `HITCENTER` swings, the crit swing, the Fists' `f_swing_left` /
  `f_swing_right`. Draw sounds by the draw sequences' event times
  (`[sounds] draw`).
- **Sets**: `weapon/tf_weapon_bat` ... `weapon/tf_weapon_knife`
  (`sets::M2_WEAPONS`: the arms, the `c_` model, the sounds, the script).
  Packing (not done; the script needs the key, so through the keyed
  helper):
  ```
  for w in bat shovel fireaxe bottle fists wrench bonesaw club knife; do
    chimera-tf2-keyed chimera_pack sets tf2 --store <server store> --only weapon/tf_weapon_$w --pack --sign
  done
  ```
  (the plain form: `cargo run --features dev,tools --bin chimera_pack --
  sets tf2 --store <server store> --only weapon/tf_weapon_<id> --pack
  --sign`; `players` again too, for the `backstab` kill icon's cell:
  `--only players --pack --sign`).
- **The party**: each TF2 rung's players carry its class's stock melee
  (data/classes.toml `melee`; `classes::HOOK`'s `RungExtras::melee`, the
  mechanism Combat Arms' rung melee uses: `runtime::KitMelee` swaps the
  kit's weapon of its slot): the Scout's Bat with the Scattergun, the
  drawn class's with the Shotgun; `rung_extras = { melee = false }` keeps
  the kit's. TF2's `PARTS.knife` is the Knife, so an automatic gun game's
  kit by game gives TF2 rungs the Knife, which the class's melee replaces.
  The finale's melee pool is every allowed melee weapon (no per-game
  slot): the nine join it. `originals:gun_game_2fort` is unchanged (its
  kit names no TF2 weapon, so its TF2 rungs still carry no melee).
- **Tests**: `tests/it/melee.rs` (the spec's cases 1-14, 16-20, 22-26,
  30, 32, a switch cancelling the smack), the script mapping's 33-35
  (`weapon_scripts.rs`), `party_kit` `tf2_rungs::a_tf2_rung_carries_its_
  classs_melee` and `the_shotguns_drawn_class_carries_its_own_melee`
  (tf2-followups: over rung seeds the Shotgun's drawn class carries its
  own melee, the Soldier the Shovel and the Heavy the Fists, in the
  rung's extras and in hand; it already followed, nothing to fix).
- Left: the Bottle's break (cosmetic), the Wrench's buildings, the Fists
  alternating with both buttons held (attack2 wins), a damage history for
  the crit chance, the BLU fists' skin (3) and the arms' team skins, the
  swing's start not sent to other clients (their third-person gesture and
  the crit sequence come at the smack), `tf_damage_disablespread` 0's
  jitter (36), the world hit's decal, probe checks P1-P4. The spec's
  cases 4 and 5 (held swings at ticks 0, 34, 67, 100) don't follow from
  its own rule (next attack = now + interval, the guns' firing-tick timing
  the SDK check confirmed): ours swing at 0, 34, 68, 102; for the spec's
  next revision or the probe (P2).

### Follow-ups (tf2-followups, 2026-11-16)

- **Melee follows the drawn class**: it did already (`classes::HOOK`
  draws the class, then takes that class's `melee`); a party test now
  holds it over rung seeds: the Shotgun's rung, each of its four classes
  drawn, carries that class's melee (Soldier the Shovel, Heavy the Fists,
  Pyro the Fire Axe, Engineer the Wrench) in its extras and in hand, the
  kit's knife swapped (`party_kit` `tf2_rungs::the_shotguns_drawn_class_
  carries_its_own_melee`).
- **No spawn in a vent duct** (a player spawned in ctf_turbine's): the
  party's spawn spots keep to open space, a generic filter for every
  game's maps (`chimera_sim::minigame::spawns::open_at`): at least 2.2 m
  of headroom (the tallest player, TF2's 82 units, and some), and under
  a ceiling lower than 2.6 m at least five of eight level rays from the
  chest running 3 m free. turbine's ducts are 96 units (2.44 m) square
  inside: the walk went in and the 72-unit standing box fitted; their
  middles, corners, crossings and mouths now fail it. Spots before and
  after: ctf_turbine 3671 to 3500 (none left in the ducts; the
  `--dump`'s spots against the ducts' faces), ctf_turbine_winter 3913 to
  3733, the other 12 pool maps 0 to 57 fewer (koth_suijin 57, cp_granary
  21, koth_nucleus 17, cp_badlands 15; the least, ctf_2fort, 4053, over
  the walk-out test's 2000); data/map_fitness.toml's spots updated for
  the 19 maps measured. The other games: a sample of 18 CS:S maps 0 to
  9 fewer each (de_nuke 9, de_port 3; measured with and without the
  filter, as CS:S's file is older than its spots), HL2:DM's seven maps
  1 to 20 (dm_steamlab), 17 Combat Arms maps 0 to 68 (oilrig and oilrigq
  about 3 %, junk_flea 33; against their files' counts, so at most
  that); Halo not measured (no install here). Test: `chimera_sim`
  `spawns::spots_stand_in_open_space_not_in_ducts_or_crawlspaces` (a
  duct, a crawlspace, a low hallway and a low room on the greybox).
- **TF2's class parts in one move**: on top of the archetype work's
  movement bundle (`slots::MovementImpl::leave`, the class layer with
  `HealthBefore`), the view kick's spring (`recoil::Tf2Punch`, put on at
  the first kick and kept) and the view model's class (`ViewVariant`, put
  on at the first TF2 gun and kept) go on and off with the class layer
  too (`movement::ClassParts`, `put_on`, `leave`, `take_off`); a body's
  team skin goes on with its class's name in one move where the map has
  TF2 bodies (`bodies::wear_bodies`; it went on a frame later, a move of
  its own). `ultimate_gun_game`'s session test (the census, its bounds
  untouched): 1,074 archetypes after four matches and 1,104 after eight,
  318 of them characters', before; 1,045 and 1,083, 293 characters',
  after (the growth over the last four matches 30 and 38: which ladders
  the matches draw moves it). Its headless world has no TF2 bodies, so
  the skin's move shows only in the real app (put on with the name
  whatever the bodies, it read 1,088 and 1,129: the skin then came and
  went where it never had). Test: `tf2` `movement::the_class_parts_go_
  on_and_off_with_the_movement_in_one_move`.
- **The duct's look** (white-grey faces with flat triangles): drawn as
  the map says. Its faces' material is a cubemap patch in the map's pak
  (`maps/ctf_turbine/props/metalduct001a_<x>_<y>_<z>`, its `$envmap` the
  nearest cubemap) of `props/metalduct001a`, a `LightmappedGeneric` with
  `$basetexture props/metalduct001a` (in tf2_textures, 512 x 512),
  `$bumpmap`, `env_cubemap` and `$normalmapalphaenvmapmask`, all found
  and handled; the base texture itself is light grey sheet metal with
  an X crease on each panel (a duct's cross-break), which is the
  "faceted" look, and the ducts' lightmaps are brighter than the map's
  average (0.28 against 0.21). Nothing to fix; not nodraw or a tools
  texture.

### SDK parity (tf2-parity, 2026-11-15)

The code made to follow each spec's "Corrections from the SDK
(2026-11-15)" list (movement, weapons, weapons_more, weapon_scripts),
each with a scenario test from the spec's own cases. The view model and
the Minigun's pose stay the view-model track's.

- **Minigun** (weapons.md 7, `weapons::minigun`): no gameplay wind-down
  (idle at once on release, 0.2 s after the last shot when firing; only
  the switch waits for the 1.33 s spool-down); a release during the
  wind-up lets it complete; the first shot 0.1 s after the 0.75 s
  wind-up (57 ticks, test 33); the rev-up penalty g(tau) 0.5 until
  0.2 s then linear to 1 at 1 s, the spread x1.5 to x1, tau the larger of
  the wind less 0.75 and the firing run (tests 13, 14, 49); crouched, the
  movement's crouch scaling on 110 (not 0).
- **Blasts and knockback** (weapons.md 5.2-5.4, `weapons::blast`): r to
  the nearer of the centre and the feet, reached by the box's nearest
  point (f clamped at the edge); the owner after everyone, his own
  radius and base (the rocket's 121 and 90); the push min(1000, D x s x
  the standing hull / the hull), from 10 below the blast, D unrounded, a
  crouched owner counting 55 tall, the Heavy half (test 43); bullets
  push at scale 6 from the shooter (test 47); the blast-jumping state
  and the air-duck count reset; off the ground above 250 units/s up.
- **Fire timing**: every TF2 gun `set` (the next shot from the firing
  tick, the interval rounded up to whole ticks).
- **Movement** (movement.md, `source_movement` config, generic): the
  backpedal clamp on the ground after acceleration (the input untouched,
  so the Scout's air jump is 400 every way; tests 25, 26, 28), the
  ground speed clamp (27), the raised acceleration of wishes under 40
  (32), the ground left above 250 (the 0.25 band), the bunny-hop cap on
  P (a spun Heavy 132, 31), two air duck releases per air time reset by
  the air jump (34, 35), the 0.3 s re-duck delay and no re-duck during
  an unduck (36), no duck in water; the zoomed rifle's 80 now reaches the
  movement (`MaxSpeed` in `select_class`; it moved at 300 before).
- **Scripts' mapping** (weapon_scripts.md, `script_numbers`): `Damage` an
  integer (cut toward zero); the flame's `Damage` ignored (13 a flame);
  `TimeReload` the Grenade Launcher's insert only; `Range` the bullets'
  trace; `PunchAngle` the view kick (`tf2:gun` `punch`, `weapons::recoil`:
  P - 1 to P + 1 whole degrees up after each shot, on Source's punch
  spring, `source_movement::punch_spring`; the shots follow it).
- **Grenades and stickies** (weapons_more.md 4-6): the grenade's fuse
  2.1 s (2.0 at its tenth 0.2 s check) at 60 % to others; stickies arm
  at 0.8 s (the air radius 0.8 to 2.8 s); the sticky's and the needle's
  short-range-reduced modifier M_s (121.8 % at 0, `damage::COMPRESSED`);
  the Demoman's own blast the base 100 / 120 x f to 146 x0.75 grounded
  or airborne, pushed at scale 9.
- **Flame Thrower** (weapons_more.md 7, `weapons::flame`): flame points
  every 0.02 s (2 ticks) from 40 forward and 5 right at 2450, slowing by
  8.5 a second (reach about 340), the Pyro's velocity along them, life
  0.6 +- 0.1, a 12-unit swept box; 0.28 ammo a point (about 9.3 a
  second); a hit at most every 0.075 s of max(1, 13 x g_age x g_heat)
  with the per-victim heat (+1 a touching point, up to +2 fresh, -1.25 a
  tick); the afterburn 3 s + 0.4 a hit up to 10, 4 every 0.5 s taking
  0.5 off (one hit: 7 ticks, 28), out in waist-deep water; an enemy Pyro
  only the 0.25 s look; teammates nothing (tests 41-44, 46).
- **Sniper Rifle** (weapons_more.md 3, `weapons::sniper`): after a scoped
  shot the scope stays 0.5 s (no jump), drops, re-zooms at +1.4 s with
  the charge rising at once; the charge stored (0.3 s wait only after a
  raise by attack2); the headshot's 0.2 s from the last zoom change; no
  shot within 0.1 s of a zoom change; a jump drops the scope and empties
  the charge, no scope in a jump; an unscoped shot blocks the scope
  1.67 s (tests 16, 17b, 17c).
- **Syringe Gun and rocket aim** (generic `chimera:projectile`
  `aim_trace`, `aim_min`, `start_trace`, `jitter`): the needle at 1000
  toward the crosshair's point (a 2000-unit line) +-1.5 degrees in pitch
  and yaw; the rocket toward the 2000-unit line's point beyond 200
  units (else its end), its start pulled back by the world.
- Not done (with why): the grenades' and stickies' launch jitter and
  spin, the 0.25 s teammate pass and the bounces off players (the
  flyers have no per-flyer pass time or player bounce yet); the ninth
  sticky's 0.2 s fuse check and attack2 detonating with any Demoman
  weapon, the deny sound (not in the corrections' main list; left for
  the Demoman's next pass); the flame's points sliding on walls (they
  stop) and the airblast; crits stay off (the party's choice); the
  Spy's and the Medic's speed rules (no such kits yet); the fixed
  pellet spread (`tf_use_fixed_weaponspreads`, default off).

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

1. ICE: settled 2026-11-15. The prose alone never reproduced the test
   vectors; the port of the designer's public-domain reference does, and
   decrypts every `.ctx` script at Thin-ICE.
2. Does the engine fall through to later search paths for a file whose
   archive is missing (install.md)? Our mount does; it matters only for
   the server layout.
3. Spawn choice on control-point maps at round start (install.md 2):
   which teamspawns a gun game uses (both teams' at once, for free for
   all).

## 7. Backlog

- View models per specs/tf2/viewmodel.md, beyond the Minigun's rest pose
  (done: `m_draw`, `m_idle`, no `view_offset`, `m_spool_idle` while
  spun): the draw at x1.34 holding its last frame, the 0.2 s smoothstep
  cross-fade for sequences not flagged "snap", `m_spool_up` cut at 0.75
  s, `m_idle` sent 2 s after the wind-down, the procedural barrel spin
  (up to 20 rad/s, 0.1 rad/s a tick), and its 24 muzzle test cases as a
  heavy test. The FOV already follows the player's base (`GameFov::
  follows_player`): 54 unzoomed at any `fov_desired`.
- Rendering (from M1's screenshots): fences and grates in 2fort and some
  ground decals draw as a coarse dither pattern; check how TF2's VMTs'
  `$alphatest` and `$translucent` are handled. Badlands' track bed and
  some floor patches show dotted dark speckle, maybe the same cause.
- Map pool by players (M4a): each pool map enters a draw only at the
  counts its measured wander time allows (map_fit's rule, 10 s; measured
  by `map_fitness` as the other games' maps): koth_harvest_final from 4
  players; ctf_2fort, ctf_turbine, ctf_doublecross and the KOTH maps
  (viaduct, nucleus, lakeside_final, king, highpass, sharkbay, suijin)
  from 8; the 5CP maps (cp_badlands, cp_granary, cp_process_final) from
  12. None is drawn for a lobby of fewer than 8 but koth_harvest_final.
- Candidates for arena slices (docs/plans/active/arena-slices.md) rather
  than whole-map play at small counts: the 5CP maps (cp_badlands,
  cp_granary, cp_process_final: the middle point and its two second
  points), cp_well (its whole map splits in two on our mesh; a slice of
  one half and the middle would play), ctf_2fort (the bridge and the two
  courtyard fronts), ctf_doublecross, and the attack/defend maps' first
  stages (cp_dustbowl, cp_gorge, cp_steel) once a slice can stand in for
  their setup gates; the left-out large ones (cp_snowplow, cp_sulfur,
  koth_megaton, ctf_gorge) only as slices.

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
- 2026-11-15: ICE replaced by a port of the designer's public-domain
  reference (user-approved source, THIRD_PARTY.md): the triplets pass and
  all 88 of the install's `.ctx` scripts decrypt at Thin-ICE.
- 2026-11-15: M2b landed: the four guns as data with TF2's rules as hooks
  (the distance modifier, the first pellet, the rocket's blast and jump,
  the Minigun's spin), the `c_` weapons bone-merged on the class arms
  (generic `map::merge_by_bone_name`; the include's sequences over the
  arms' stubs), sounds, TF2 in the party's `GUESTS` and
  `originals:gun_game_2fort`; the scripts' numbers a logged seam.
  Screenshots in UGG on ctf_2fort and de_dust2.
- 2026-11-15: more guns (tf2-guns2): the Minigun held up in first person
  (the spun pose, a `view_offset`); the Pistol, SMG, Sniper Rifle,
  Revolver, Grenade and Stickybomb Launchers, Flame Thrower and Syringe
  Gun from weapons_more.md as rungs with their hooks; TF2's muzzle flashes
  and explosions. Screenshots on ctf_2fort.
- 2026-11-15: the weapon scripts' numbers: specs/tf2/weapon_scripts.md,
  `script_numbers` applies them at run time (store or install plus key),
  served to clients as server documents.
- 2026-11-15: M3 landed: the classes as data, TF2's movement on Source's
  with the gun's class (speed, eyes, the Scout's air jump, max health),
  the class bodies with team skins and animation, TF2's HUD and kill
  icons. Screenshots in UGG on ctf_2fort and de_dust2 (each gun's HUD, a
  third-person Heavy, a Scout mid air jump).
- 2026-11-15: specs/tf2/viewmodel.md (T9), the first spec checked against
  the Source SDK 2013's TF2 game code (updated 2025-02-18; read in place by
  a spec session, never copied). The Minigun's real rest pose is `m_idle`
  as we decode it (raised, barrels up; only the barrels and a glove on
  screen at `viewmodel_fov` 54): draw `m_draw`, idle `m_idle`, no
  `view_offset`; TF2 has no sway, its bob is CS:S's, and the view-model
  FOV is 54 whenever the player is not zoomed, whatever their base FOV.
- 2026-11-15: the gameplay specs (movement, classes, weapons, weapons_more,
  weapon_scripts, hud, player_animation) checked against the SDK's TF2
  game code; each lists its "Corrections from the SDK". The largest for
  the code: the Minigun has no gameplay wind-down (no slowdown, jump or
  re-spin lock after release; first shot about 0.85 s), its rev-up curve;
  the blast (owner radius 121, distance to the nearer of centre and feet,
  push capped at 1000, Heavy half, bullets push too, leave-ground 250);
  next-shot timing from the firing tick ("set", not "accumulate"); the
  grenade fuse 2.0 s at 60 % damage, sticky arming 0.8 s, Demoman self
  damage x0.75 always; the flame every 0.02 s; Medic regen 6 to 12/s.
  The SDK holds no class or weapon scripts: per-class health and speed
  and per-gun numbers stay measured or from the scripts.
- 2026-11-15: SDK parity, first landing (tf2-parity): the Minigun's
  stop at once and rev-up curve, the blasts' feet distance, owner radius
  and capped push (the Heavy's half, bullets pushing), `set` fire timing,
  TF2's movement corrections (backpedal, ground clamp, re-duck delay,
  air ducks by release, the 250 ground leave, the slow-wish boost), the
  scripts' Range / PunchAngle / TimeReload / Damage rules with a view
  kick, the grenade fuse and sticky arming, the Demoman's self-damage.
  Tests whose numbers changed: the Minigun's first shot (50 -> 57 ticks),
  its wind-down (gone), the grenade's fuse (2.3 -> 2.1 s), the sticky's
  arming (0.7 -> 0.8 s), the self-damage worked case (76.75 -> 74.01 by
  the feet), the mapped Scattergun script (Range and PunchAngle mapped,
  TimeReload not).
- 2026-11-15: SDK parity, second landing: the Flame Thrower as flying
  points (cadence, ammo 0.28 a point, damage by age and heat, the
  afterburn's 3 s + 0.4 a hit, 7 ticks for one hit, enemy Pyros only
  lit), the Sniper's scope timing (drop at +0.5 s, re-zoom at +1.4 s,
  jumps, the 0.1 s fire block, the unscoped shot's 1.67 s), the needle at
  1000 toward the crosshair +-1.5 degrees, the rocket toward the
  crosshair's point (generic `chimera:projectile` options). Tests whose
  numbers changed: the flame (cone ticks -> points, 41/42/44 rewritten),
  the scope after a shot (16), the needle's speed (990 -> 1000), the
  rocket's direction (24), the flame's cycle (0.08 -> 0.02).
- 2026-11-16: the bodies per class on the asset service (the user's design):
  `players/<class>` (a class's player model, animations, materials and
  first-person arms; 4.6 to 7.9 MB stored beyond `base`) and `players`
  (what every class reads, the HUD's look, the kill icons). A match's
  offer names the class sets its TF2 guns make (`bodies::asset_names`, an
  `AssetHooks` entry), with the classes they need (`needs` in
  data/classes.toml: data only, so the Spy's disguise will add every
  class there); a machine reading TF2 from the store loads only the bodies
  whose files are here (generic `Mount::held`, tracked for the map cache),
  so a body is never fetched mid-match. `chimera_pack sets --pack` packs
  what a set reads that the store lacks. The kill icons register from the
  store once their files are here (`hud::kill_icon_pictures`), not only
  from an install at startup.

- 2026-11-15: M4a: TF2's maps in the party's automatic pool: 14 koth_,
  cp_ and ctf_ maps allowed (data/map_pool.toml) from a `map_fitness` sweep
  of 130 (data/map_fitness.toml); free for all (touch-opened doors start
  open, `SourceGame::entity_rule`; spawn rooms' door brushes non-solid);
  a spawn-room walk-out heavy test (cp_well out: its halves never join);
  sets checked in a scratch store; screenshots of each pool map in UGG.
- 2026-11-16: M4b: specs/tf2/weapons_stock.md (T10) and the nine stock
  melee weapons on Chimera's delayed swing (`Swing::delay`) and backstab
  rule (`Backstab::Behind`), crits off by default, the raised knife, the
  class's arms and sequences, the `backstab` kill icon; each TF2 rung
  carries its class's melee (rung extras), the Knife TF2's kit knife.
  Screenshots in UGG on ctf_2fort.
- 2026-11-16: tf2-followups: a party test that the Shotgun's drawn class
  carries its own melee (it did); the party's spawn spots keep to open
  space (`spawns::open_at`: headroom, and room around under a low
  ceiling), so none is left in ctf_turbine's vent ducts (3671 spots to
  3500); the ducts' look is their own material (light grey cross-broken
  sheet metal), nothing missing; TF2's class parts (the class layer, the
  view kick's spring, the view model's class) on and off with the
  movement in one move, a body's skin with its name.
- 2026-11-18: the voices' waves in the sets: each class's pain, death and
  kill lines (data/voices.toml) in its `players/<class>`, the lines any
  class plays (drowning, the fall splat) in `players`
  (`sets::class_files`, `voices::SHARED`); a client no longer fetches them
  on demand once the sets are republished.
