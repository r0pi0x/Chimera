# Plan: Half-Life 2: Deathmatch on Chimera

Started 2026-11-01. The next game after CS:S and Combat Arms is
Half-Life 2: Deathmatch (user decision, 2026-10-31; docs/backlog.md,
"Decisions"), after the "before the next game" list (console
namespaces, the ECS cleanup), which is done. This is the plan only: no
code lands with it. CLAUDE.md asks for a plan before a new mount
adapter or loader; this is that plan, kept up to date as the work goes.

Goal: HL2:DM is a third game library (`games/hl2dm`, crate `hl2dm`,
id `hl2dm`) that lends its guns, movement, bodies, HUD and maps to the
party apps (Ultimate Gun Game first), and its own faithful app
(`apps/hl2dm`, binary `hl2dm`, with `apps/hl2dm_server`) plays
deathmatch and team deathmatch as the real game does. Verified by
scenario tests from each spec's test cases, heavy tests on the install,
tick-exact comparisons against a real HL2:DM server (a probe, as
`tools/css_probe` is for CS:S) and `refcmp` captures of the real game.

## 1. Goals and non-goals

Goals, in the order they land:

1. **HL2:DM's parts in the party apps.** Its guns in Ultimate Gun
   Game's ladder (`chimera_sim::minigame::auto`), with the identity
   following the gun (`chimera_sim::identity`): HL2:DM's movement, its
   bodies and its HUD while an HL2:DM gun is in hand; its maps in the
   pool (`GameParts::maps`, `map_patterns`). Lucker Party is shelved
   (backlog, 2026-10-31): it gets HL2:DM only through `party_kit`, with
   no work of its own.
2. **Its own app for parity play** (`apps/hl2dm`, `apps/hl2dm_server`):
   - deathmatch and team deathmatch (Combine and Rebels), frag and time
     limits, respawns, the stock maps;
   - every stock weapon: crowbar, stunstick, pistol, .357, SMG1 with its
     grenade, AR2 with its energy ball, shotgun (single and double
     shot), crossbow, RPG with its laser guide, frag grenade, S.L.A.M.
     (tripmine and satchel), and the gravity gun;
   - physics props as weapons: the gravity gun's punt, pick-up and
     throw, props picked up with `+use`, and the damage a thrown prop
     does (with the kill credited to its thrower);
   - the suit: health, armour (batteries, chargers), aux power (sprint,
     flashlight, breath under water), suit zoom;
   - its HUD (health, suit, ammo, secondary ammo, aux power, weapon
     selection, quick info, pickup history, death notices, damage
     indicator, scoreboard), its sounds and its effects;
   - network play on Chimera's network, as CS:S and Combat Arms have.
3. **Parity first, data-driven first** (AGENTS.md "Design principles").
   By default everything plays as the real game, measured and specced;
   numbers live in our TOML (`games/hl2dm/data/`), behaviour in hooks
   registered under `hl2dm`; our own improvements come later as opt-in
   options.

Non-goals (for now):

- Half-Life 2's single-player campaign, NPCs and their AI, vehicles,
  the HEV suit's voice, Garry's Mod's physics gun or anything beyond
  the stock gravity gun.
- Bots in the faithful app (HL2:DM has none; open question 4). The
  party apps' bots stay Combat Arms'.
- Cross-game balance (the party modes stay unbalanced on purpose).
- Community HL2:DM maps beyond what the generic Source map path already
  loads (custom-maps.md covers maps needing another game's content).
- Source TV, demos, replays and HL2:DM's server browser, until the app
  plays.

## 2. What CS:S's Source pieces already cover, and what's HL2:DM's

CS:S proves most of Source's file formats and engine behaviour; almost
all of it sits in `games/cs_source` today, so HL2:DM can't use it until
it is lifted (section 3). Already game-neutral and usable as is:
`source_formats` (VPK, pakfile, LZMA lumps, `.vhv`, `.phy`, WAV/MP3,
the detail lump, DMX/PCF, `.nav`, KeyValues, localization files),
`source_logic` (entity I/O, triggers, movers, breakables, props, fire,
templates, view control), and Chimera's frameworks (weapon parts,
grenades' rules, hits and armour, impacts, lag compensation, ragdolls,
physics props on avian, view-model driver, mode runtime, minigames,
identity, network).

| Area | Covered by CS:S's code | HL2:DM's own |
|---|---|---|
| Mount | `games/cs_source/src/game/mount.rs` mounts `cstrike/` then `hl2/`'s `hl2_textures`, `hl2_sound_vo_english`, `hl2_sound_misc`, `hl2_misc` VPKs and `platform/`; VPK v2 in `source_formats::vpk` | `hl2mp/` first (loose, `custom/`, `hl2mp_pak_dir.vpk`), then the same `hl2/` and `platform/` layers, `download/` and Chimera's content cache last. Better: read the install's `hl2mp/gameinfo.txt` SearchPaths, the game's own order (lift S1). Confirmed by H0 (specs/hl2dm/source_deltas.md, "gameinfo.txt"): `hl2mp/custom/*`, `hl2mp_english.vpk` (absent), `hl2mp_pak.vpk`, four `hl2_complete` VPKs (only `_misc` present), the four `hl2/` VPKs, `platform_misc.vpk`, then loose `hl2mp/`, `hl2_complete/`, `hl2/` (its `scripts/` hold the sound manifest, prop data and surface properties), `platform/`, `hl2mp/download` last: every VPK before any loose folder |
| Game info | `cs_source::game::INFO` (`GameInfo`) | `hl2dm::game::INFO`: id `hl2dm`, `short_name` "HL2:DM", `content_dir` `hl2mp`, Steam app 320, `required` `hl2mp/hl2mp_pak_dir.vpk` and `hl2/hl2_misc_dir.vpk`, `maps/<name>.bsp`, `cfg/config_default.cfg` binds, its movement cvars, 0.0254 m per unit, tick from the spec (expected 0.015 s), `console_namespaces` (open question 1) |
| Maps | `bsp.rs` (BSP v20 to `MapData`), `lightmap.rs`, `ambient.rs`, `overlays.rs`, `detail.rs`, `decals.rs`, `sky.rs`, `water.rs`, `ropes.rs`, `soundscape.rs`, `fire.rs`, `beams.rs`, `trails.rs`, `steam.rs`, `sprites.rs`, `dust.rs`, `breakables.rs`, `nav.rs`; the converted-map cache (`map::cache`) | the stock maps (dm_lockdown, dm_overwatch, dm_powerhouse, dm_resistance, dm_runoff, dm_steamlab, dm_underpass, halls3: confirmed by H0, all BSP v20 with LDR and HDR lighting; halls3 is outside the map cycle); its entities beyond specs/source/: `info_player_deathmatch`/`_combine`/`_rebel`, placed `weapon_*` and `item_*` that respawn, `prop_physics_respawnable`, chargers, `func_useableladder` (stock maps have no combine ball launcher); its `MapLoader` and a `MapDressing` for its players on other games' maps |
| Materials | `material.rs` (VMT/VTF, lenient VMT, stand-in shaders), Chimera's Source shaders in `chimera_present` (specs/cs_source/shaders.md) | HL2's character shaders (`Eyes`, `Teeth`; no player model uses `EyeRefract`) and flexed faces, not specced (CS:S's models have no flexes: specs/cs_source/mdl_v48.md); of the stock maps' world shaders only `UnlitTwoTexture` (dm_underpass) is new (H0) |
| Models and animation | `props.rs` (MDL via `vmdl`, LODs, skins, body groups, `.phy`, ragdolls), `anim.rs` (v44-48 animation, sequences, events, attachments, include models), `propdata.rs`, `surfaceprops.rs`; CS:S's hostages already play HL2 citizen sequences (`hostage_anim.rs`) | HL2:DM's player models (Combine soldier, super soldier and metro police; the Rebels' male and female citizens), their include models, hitbox sets and ragdolls; the multiplayer animation state (activities per held weapon kind, aim and move pose parameters, fire, reload and jump gestures) instead of CS:S's (`player_anim.rs`); view models (`v_*.mdl`) and their activities |
| Movement | Source movement (`movement.rs` with `Tracer`, `drown.rs`, `shadow.rs`, `pushaway.rs`; `SourceMovementConfig` already holds CS:S's values, stamina and the bunny-hop cap among them) | HL2:DM's numbers (walk, run and sprint speeds, `+walk`/`+speed` meaning), sprint drawing aux power, the jump's forward boost and its quirks, drowning through aux power, fall damage: a config and a few hooks, not a second movement |
| Physics props as weapons | avian bodies for `prop_physics*` (specs/cs_source/physics_props.md), damage impulses, prop-on-player crush damage (`FixedPostUpdate`: "new contacts of props ──> impact Damage"), `prop_physics_multiplayer` push-away, `NetProp` replication | the gravity gun (punt, pull, hold, throw, limits by mass and size, what it can't hold), `+use` carry, physics damage to players by mass and speed with the thrower credited, the combine ball held and launched; who simulates a held prop on the network |
| The suit | `core::Health`, `weapon::Armor` with coverage by identity (`hits::absorb`, DESIGN.md "Hits across games") | the suit's armour split (its own rule; may not fit `absorb`'s shape: lift C3), aux power (one pool for sprint, flashlight and breath), batteries, health kits and vials, wall chargers, suit zoom |
| Weapons | the weapon frame and parts (`chimera_sim::weapon`), `Hitscan`, `Melee`, `Magazine`, `Throwable` (grenades), `ViewPunch`, lag comp, TOML data (docs/weapon-data.md); Combat Arms' projectiles and bolts (`games/combat_arms/src/game/special/`) | crowbar and stunstick (melee); pistol, .357, SMG1, AR2, shotgun (hitscan, spread cones, double shot); SMG1 grenade, AR2 ball, RPG missile with laser guide, crossbow bolt (projectiles: lift C1); frag (throw, roll, lob); S.L.A.M. (tripmine beam, satchel and detonator); ammo types and carry limits; pickups that stay or respawn |
| HUD | `hud.rs` and `vgui.rs` read `hudlayout.res`, `clientscheme.res` (fonts, TTFs), `.res` layouts and localization; `ui/game_hud.rs` draws panels from a layout (L6, not lifted); `client::hud`, kill feed, crosshair | HL2's panels and their behaviour: health, suit, ammo and secondary ammo, aux power with its device labels, HL2's weapon selection (columns of icons from the HalfLife2 font), quick info brackets, pickup history, death notices with font icons, damage indicator, suit zoom, scoreboard, team menu; `scripts/hudanimations.txt` if CS:S's path doesn't read it yet |
| Sounds | sound scripts (`sound.rs`), soundscapes, WAV/MP3, impact, scrape and pain sounds (`impacts.rs`, `scrapes.rs`, `pain.rs`), footsteps in movement | HL2:DM's script entries (weapons, the suit, pickups, chargers), footstep loudness profile, pain and death sounds by model, the combine ball's and the gravity gun's loops |
| Effects | `impact_effects.rs` (particles by surface, blood, splashes), `pcf.rs`, `map::particles`, tracers, muzzle flashes, decals | the AR2's tracers and impacts, the combine ball's dissolve, explosions (RPG, frag, SMG1 grenade, S.L.A.M.), the RPG's laser dot and smoke trail, the crossbow's bolt and its glow, the gravity gun's beams and glow |
| Rules | CS:S's deathmatch respawns and game end (`rules.rs`, `rules/game_end.rs`), Chimera's mode runtime (`chimera_sim::mode`) on which Combat Arms' TDM plays, `chimera_sim::spawning` | HL2:DM's DM and TDM as modes on the mode runtime (`mp_teamplay`, `mp_fraglimit`, `mp_timelimit`, `mp_forcerespawn`, `mp_weaponstay`), its spawn-point choice, team models and `cl_playermodel`, suicide and team-kill scoring, intermission |
| Tools | `cs_source_tools`: `refcmp`, `movecmp`, `mapsweep`, `hudcmp`; `source_tools`: RCON | an HL2:DM driver for each (section 6) |

## 3. Lifts: what moves because two Source games need it

The rule (DESIGN.md, AGENTS.md "Generalize for the second game"): at
the second use, lift the piece out of `games/cs_source`, generic with
the game supplying data and hooks, CS:S's behaviour unchanged and its
tests passing; never copy, never depend game on game. Most of what
HL2:DM needs is Source behaviour rather than CS:S's, so it goes to a
**new Bevy-level crate in `source/`** (architecture-review.md A5;
combat-arms.md "Loader-specific: Source behaviour, not CS:S"), called
`source/engine` (`source_engine`) here (open question 8): Source's
conversion into Chimera's types and Source engine behaviour every
Source game shares. Its simulation part uses the engine crates,
`source_formats` and `source_logic`, never `chimera_present`; its
drawn and HUD readers' front end sit behind a `ui` feature as a game's
do. What every game could use (not only Source games) goes to Chimera.

How to lift, to keep the risk down: **move first, generalize second.**
Each lift is two commits: (1) the files move as they are, re-exported
at their old `cs_source::game::*` paths (as `source_formats` was split
off), with every test unchanged; (2) the CS:S names inside become
CS:S's data or hooks. One lift per area at a time, each owned on the
board (section 8). Risky ones (S2, S6, C5) each alone, after a full
`cargo nextest run --features dev` baseline.

### Into `source_engine`

| S | Piece | From (lines) | CS:S inside that becomes data or a hook | Risk, and the tests that guard it | First need |
|---|---|---|---|---|---|
| S0 | The crate itself | new: `source/engine/` | `checks/tests/it/architecture.rs` (`workspace_crates`, layering, `server_builds_without_presentation`), `determinism.rs` and `ecs.rs` rows, hakari, ARCHITECTURE.md's crate paragraph | Low: `cargo test -p repo_checks` | M1 |
| S1 | The search path | `game/mount.rs` (148) | `cstrike` and the VPK names as the game's data, or read from `gameinfo.txt` SearchPaths; `import_map` and the content cache are already generic | Low/medium: `mount_cs_source.rs`, `mount_install.rs`, heavy `served.rs`; CS:S's mount order must stay identical (the asset store's manifests list paths, not layers, so they don't change) | M0/M1 |
| S2 | BSP to `MapData` and the map's parts | `bsp.rs` (2692), `lightmap.rs` (545), `ambient.rs` (691), `overlays.rs` (268), `detail.rs` (270), infodecal half of `decals.rs`, `sky.rs`, `water.rs` (621), `ropes.rs` (253), `soundscape.rs` (391), `fire.rs` (638), `beams.rs`, `trails.rs`, `steam.rs`, `sprites.rs`, `dust.rs`, `breakables.rs` (348), `nav.rs` | CS:S's class models (`bsp::CLASS_MODELS`), hostages, bomb and buy zones, spawn classes and team numbers become CS:S's dressing (`MapDressing`) and data; `cache::key(game, CONVERTER)` keeps each game's key | High: about 40 heavy map tests (`map_stock.rs`, `map_de_dust2.rs`, `map_surfaces.rs`, `map_props.rs`, `map_cache.rs`, ...), `refcmp` on dust2, `mapsweep`. The converted-map cache's key must not change for CS:S (or it is one re-convert) | M1 |
| S3 | Materials | `material.rs` (2224) | none expected; the map's game for its pak | Medium: heavy `map_surfaces.rs`, `map_material_extras.rs`, `refcmp report` | M1 |
| S4 | Models, animation, prop data, surfaces, particles | `props.rs` (2359), `anim.rs` (1072), `propdata.rs` (466), `surfaceprops.rs` (148), `pcf.rs` (460) | CS:S's model paths in `load_view_model` callers; the hitbox set's region mapping by bone (already generic) | Medium: `animation.rs`, `drawn_parts.rs`, `ragdoll*.rs`, `phy_spec.rs`, `player_lods.rs`, heavy `map_props.rs`, `map_prop_damage.rs` | M1 |
| S5 | Sound scripts and Source's impact, scrape and pain sounds | `sound.rs` (596), `impacts.rs` (530), `scrapes.rs` (435), `pain.rs` (317: L3) | the entry names by hitgroup and helmet (`Player.Damage*`), `Player.FallDamage` become the game's data | Low/medium: `map_sound_fx.rs`, `pain_sounds.rs`, heavy `map_sound_stock.rs`, party `identity_sounds.rs` | M3 |
| S6 | Source movement | `movement.rs` (2635, with `Tracer`: L1), `drown.rs` (149), `shadow.rs` (497), `pushaway.rs` (252) | `SourceMovementConfig` per movement id (`cs_source:movement`, `hl2dm:movement`); CS:S's stamina and bunny-hop cap stay its config; the hooks HL2:DM needs (sprint drawing a pool, the jump's boost, drowning damage) as named parts | High: `source_movement*.rs` (four files), `movement.rs`, `prediction.rs`, `net_prediction.rs`, heavy `map_courses.rs`, `map_ladders.rs`, `map_surf.rs`, `movecmp` against the probe, party `cross_water_ladders.rs` | M3 |
| S7 | HUD and VGUI readers; the panel HUD | `hud.rs` (323), `vgui.rs` (595), `gameui.rs` (752); `ui/game_hud.rs` (L6), `ui/hud_sprites.rs` (L8) | file names (`cstrike_english.txt`, the panel names `HudHealth`...), `LOW_HEALTH`, notice time and count, money and round clock panels become CS:S's data | Medium: `cs_source` ui tests (`--features dev,ui`), `hudcmp`, party `hud.rs`, `kill_icons.rs` | M9 |
| S8 | View-model rules, bob and sway | `view_anim.rs` (437), `view_motion.rs` (266) | Source's activity names (shared); CS:S's knife idle, silencer and burst rules stay CS:S's `ViewAnimRules`; HL2MP's bob (specs/cs_source/view_models.md's B13 row) as the other model | Medium: `view_models.rs`, `view_model_spec.rs`, party `drawn_ladder.rs` | M4 |
| S9 | Player animation's shared half (L14) | `player_anim.rs` (964) | the pose parameters, aim and move layering, hand IK and the per-tick server pose (`SimAnimPlugin`) are Source's; the choice of sequence (CS:S's names by weapon suffix, HL2:DM's activities by weapon kind) is each game's hook | Medium: `animation.rs`, `drawn_parts.rs`, party `bodies_drawn.rs`, `identity.rs` | M8 |
| S10 | Impact looks | `impact_effects.rs` (2282), runtime half of `decals.rs` (L11) | Source's material letters to effects are shared; CS:S's blood and particle tables become its data | Medium: `impact_effects.rs`, heavy `map_surfaces.rs`, party `cross_game.rs`, `cross_glass.rs` | M10 |
| S11 | Tools' Source half | `cs_source_tools`: `refcmp`, `movecmp`, `mapsweep`, `hudcmp` drivers | the game's folder, process name, probe plugin and map list as arguments; a `source_engine` tools crate or `source_tools` (if it needs no Bevy) | Low: the tools' own tests | M0 (movecmp), M10 (refcmp) |

Stays CS:S's: everything in combat-arms.md "CS:S-specific: stays"
(objectives, hostages, radio, buy menu, CS:S's rules and bots, its
menus' looks), plus `weapons.rs` and `grenades.rs` (CS:S's guns as
data and hooks), `hit_reaction.rs`, `rungs.rs`, `parts.rs`, `demo.rs`
(its `CsMapper`), `resim.rs`, `votes.rs` (unless HL2:DM's votes match).

### Into Chimera

| C | Piece | From | Why now | Risk, and the guards |
|---|---|---|---|---|
| C1 | Projectiles: a flying body with a model, speed, gravity, drag, bounce, a fuse or impact trigger, guidance by a hook (the RPG's dot), and a blast | Combat Arms' `combat_arms:projectile`/`:bolt` (`games/combat_arms/src/game/special/`) and `weapon::grenade`'s flight | HL2:DM's SMG1 grenade, AR2 ball, RPG, crossbow bolt and frag are the second user; one predicted and replicated path for every game | Medium: Combat Arms' `special_weapons.rs`, `net_projectiles.rs`, `launcher_aim.rs`, `grenades.rs`; CS:S's `cs_grenades.rs`; `parity_pin_*` |
| C2 | Armour rules per game | `chimera_sim::weapon::hits::absorb` | HL2's suit splits damage by its own ratio and bonus; if the spec's formula doesn't fit `absorb`, the split becomes the armour game's hook (`ArmorRule` by game, CS:S's and Combat Arms' as now); damage classes HL2:DM adds (crush, shock, dissolve, blast) join the open set | Medium: `parity_pin_*` in both games, party `cross_game.rs` |
| C3 | Map pickups that respawn | `chimera_sim::weapon::equip` (`MapWeapon`, put back at round restart) | HL2:DM's placed weapons, ammo, health kits and batteries respawn after a time and may stay (`mp_weaponstay`); generalize "put back at restart" to "put back after a delay, by rule" | Low/medium: `map_game_entities.rs`, heavy `map_loose.rs`, `map_net_weapons.rs` |
| C4 | A game bundle for the composer (UGG gap 2) | `apps/party_kit/src/lib.rs`: `PartyPartsPlugin`, `MAP_SOURCES`, `GAME_PARTS`, `GAME_BINDS`, `add_view_model_fovs`, `CS_SOURCE_PARTS`, `client::with_huds` | three games make "name both games" untenable: each game exports one bundle (its sim parts plugin, `MapLoader`, guest `MapDressing`, HUD plugins, binds layer, view-model FOV, bot gun types) behind one Chimera interface, and the kit loops over the linked bundles | Medium: every `apps/party_kit/tests/it/` file (`cs_parts.rs`, `identity.rs`, `net_party.rs`, `listen_server.rs`, `rung_*.rs`, `startup.rs`), `apps/ultimate_gun_game/tests/it/` |
| C5 | Teams and the game's end (L15's rest) | `games/cs_source/src/rules.rs`, `rules/game_end.rs`, `rules/teams` | only if the mode runtime lacks a frag or time limit, intermission and team assignment that HL2:DM's TDM can use (Combat Arms' TDM may already cover it: check first) | High if moved: `rounds.rs`, `teams.rs`, heavy `bot_rounds.rs`; avoid unless needed |
| C6 | Held bodies on the network | `chimera_net`'s `NetProp` | a prop a player holds (gravity gun, `+use`) is simulated by the server and interpolated for everyone, the holder included (expected parity: the spec says); Chimera owns prop replication, so the seam (who holds it, its target pose) goes there; the gravity gun's rules stay `hl2dm`'s | Medium: `chimera_net` tests, `net_breakables.rs`, `logic_physics.rs` |
| C7 | `chimera_pack`'s game list | `src/bin/chimera_pack.rs` (`games() -> [Game; 3]`, `record_pieces` by game id) | a third game: the list from the registered `GameInfo`s (app id from `steam`), the pieces' sets as a hook the game provides | Low: its unit tests, heavy `served.rs` |
| C8 | Console names shared by Source games | `chimera_core::console` (`SOURCE_NAMESPACE` = `cs_source`) | done 2026-11-01 (engine-names branch): `fps_max`, `rate`, `map` ... are `chimera:` (`chimera_core::console_names`); HL2:DM registers its own `hl2dm:sv_gravity` (section 5, option C) | Low/medium: `console_scope.rs`, console unit tests, `persist.rs` |

Not lifted (HL2:DM's own, in `games/hl2dm`): the gravity gun's rules
and physics damage credit (unless a second game needs them), aux
power, the suit's pickups and chargers, HL2:DM's modes, its weapons'
hooks, its rung extras (`rungs.rs`: a player model per rung, from its
data), its parts (`parts.rs`).

## 4. Specs needed

HL2:DM's behaviour reference is Valve's publicly released Source SDK
2013 (its multiplayer tree holds HL2:DM's game code), plus measurements
of the real game. Under the hard rule, that code never enters this
repository, is never translated line by line, and implementation
sessions read `specs/` only. So: separate spec sessions, where the
reference is, write `specs/hl2dm/*.md` in our own words
(specs/README.md's template and rules: constants with units, formulas
as math, per-tick order, edge cases, quirks, numbered test cases, open
questions; no code blocks). The user reviews each before it is
implemented.

**Where they run.** On this box, as separate sessions (user decision,
2026-11-01, open question 6): the public SDK is cloned outside the
repository and read in place; each spec session works in its own agent
slot on its own branch, writes only `specs/hl2dm/` (and this plan's
board), commits and leaves the merge to the coordinator. Nothing derived
from the SDK is written anywhere else; implementation sessions never
open it. Measurements (the probe, refcmp) are not code reading and run
where the game is (section 6).

**What exists.** `specs/source/` (entity I/O, triggers, doors and
buttons, breakables, fire, physics brushes and constraints, prop
damage, particles and smoke, visual and game entities, view control)
applies to any Source game. Several of `specs/cs_source/` are Source
engine behaviour that CS:S happened to need (`mdl_v48`, `animation`,
`shaders`, `physics_props`, `ragdolls`, `sounds`, `water`, `ropes`,
`overlays_decals`, `sprites_dust`, `shadows_sky`, `nav`,
`impact_effects`, `view_models`, `movement`): H0 says which hold for
HL2:DM unchanged and what differs, so nothing is specced twice.

### The waves

| H | Spec file | Wave | Covers | Unblocks |
|---|---|---|---|---|
| H0 | `source_deltas.md` | 0 | **landed (draft, 2026-11-01)**: each spec in specs/cs_source/ and specs/source/ judged same, same-re-derive, differs or CS:S-only, with the numbers; install facts (search path, VPKs, stock maps, BSP and MDL versions, builds, tick, cvars, player models, weapon scripts, ammo table, HUD resources, the server and SourceMod) | S-lifts' scope, M0, M1 |
| H1 | `movement.md` | 1 | starts from source_deltas.md 1.1–1.2 (gravity 600, jump 160, hull 72/36, speeds 150/190/320, sprint on aux power, no stamina, `sv_accelerate` 10, `sv_stopspeed` 100, input speeds 450, fixed 10 fall damage, the HL2 fall thresholds); writes: the probe measurements of the jump (bunny-hop cap or none, `player_limit_jump_speed`, the back-hop, the single-player-only forward boost), the ducked hull, air duck lift and duck input scale, **`func_useableladder`** with `info_ladder_dismount` (mount cone and distance, dismount choice, jump-off; contents ladders not climbable), water jump and swim, the always-allowed acceleration, step sounds' timing; drowning goes to H2 | S6, M3 |
| H2 | `suit.md` | 1 | health and armour limits; the armour rule (80 % absorbed, 1 armour per point, 2 against blasts, at least 1, what bypasses it; source_deltas.md "Behavior") and whether `hits::absorb` can express it (C2); aux power (pool 0–100, recharge 12.5/s after 0.5 s with no device, sprint 25/s, breather 6.7/s, what stops sprint); breath (no air used while aux power lasts, then 7 s, then 10 a second, recovery); the flashlight (`mp_flashlight`, no aux drain in HL2:DM: confirm); suit zoom (`impulse 50`, `+zoom`); health kits 25, vials 10, batteries 15, chargers (75 suit, 50 health: rates, recharge); death state | C2, M3 |
| H3 | `weapons.md` | 1 | inventory and slots (`bucket`, `bucket_position`, `weight`, `cl_autowepswitch`, `cl_defaultweapon`, `phys_swap`, `lastinv`), switching and draw times, pickup and drop, the spawn loadout, ammo carry limits (source_deltas.md's ammo table), `mp_weaponstay` and weapon/item respawn (20 s, 30 s); pistol (no spread despite its accuracy ramp; click refire 0.1 s), .357, SMG1, AR2, shotgun (7 and 12 pellets), with the scripts' player damage, the hitgroup cvars (head 3), damage against non-players per ammo type, view kick, reloads and auto-reload (3 s); crowbar (25, 75, 0.4) and stunstick (40, 75, 0.8) with the hull sweep; reads the plain `.txt` scripts only | M4 |
| H4 | `projectiles.md` | 2 | SMG1 grenade (100 damage, radius 250), AR2 energy ball (radius 10, life 4 s, mass 150: charge, launch, bounces, dissolve), RPG (1500 units/s, guidance by the laser dot, losing the dot), crossbow (3500 in air, 1500 in water, gravity, zoom, sticking), frag as a **physics object** (fuse 2.5 s, throw 1200, lob 350 + 50 up, roll 700 when ducked, blips, live drop on death; CS:S's bounce rule does not apply), S.L.A.M. (tripmine beam, satchel throw and detonator, arming), blast damage and push | C1, M5 |
| H5 | `gravity_gun.md` | 2 | punt and pull (`physcannon_*` cvars: max mass 250, reach 250, forces 700–1500, pull 4000, cone 0.97), pick-up (reach, mass and size limits, what it never holds), the held object's motion and rotation, throw (`player_throwforce` 1000), drop on damage, the combine ball in it; **`+use` carry is off by default** (`hl2mp_allow_pickup` 0): say what `+use` on a prop does then; physics damage to players (energy scale 4, the impact table, the thrower's credit and how long it lasts); `prop_physics_respawnable` (329 placed); what the server simulates and what clients see | C6, M6 |
| H6 | `rules.md` | 1 | DM and TDM (`mp_teamplay`), teams and their models (round-robin replacement on a team mismatch, 5 s model and team change limits, `cl_playermodel` fallback), spawn-point choice (team spawns, random start, validity, the 48-unit telefrag fallback), death view and respawn (no freeze cam; observer after 3 s; any button or `mp_forcerespawn` after 5 s; `valve.rc`'s `mp_forcerespawn 0`), frag and time limits, intermission (`mp_chattime` 10) and map change (halls3 not in the cycle), restart (`mp_restartgame`, `mp_readyrestart`), scoring (suicides, team kills, world kills), chat, game events and log lines, the server cvars | M7 |
| H7 | `player_models.md` | 2 | the 19 player models and team mapping; animation through `hl2mp/`'s include overrides (`models/player/male_anims.mdl`, `female_anims.mdl`: 105 sequences, 8 pose parameters) and the activity-based state (per `anim_prefix`), gestures; hitboxes and hitgroups; `hl2mp_ragdoll` bodies; the Eyes and Teeth shaders and flexes at rest (citizens: 54 controllers); view models (right-handed, no mirroring; FOV 75 and `viewmodel_fov` 54; the HL2MP bob row) | S9, M8 |
| H8 | `hud.md` | 3 | each HUD panel's behaviour (when it shows, flashes, fades; `hl2mp/`'s `scripts/hudanimations.txt` and `hudlayout.res`), the scheme's fonts (GorDIN from `hl2_complete/resource/`, HalfLife2, HL2MP, HL2cross), weapon selection from the scripts' font icons, quick info, pickup history, death notices and their icons, damage indicator, aux power panel, flashlight, suit zoom overlay, crosshair (`Crosshairs` font), scoreboard, team menu and MOTD | S7, M9 |
| H9 | `effects_sounds.md` | 3 | muzzle flashes, tracers (every 2nd for SMG1/AR2, `AR2Tracer`, whizz by `Bullets.DefaultNearmiss`), impacts (old-style as impact_effects.md; AR2's own), blood particles, explosions, the dissolve, the gravity gun's beams, the RPG's dot and trail, the UnlitTwoTexture shader (dm_underpass); weapon, suit, pickup, charger sound entries; footsteps per model type (`NPC_Citizen`/`NPC_CombineS`/`NPC_MetroPolice` `.RunFootstep*`, none while ducked), pain and death (`.Die`), loudness and fall-off | S5, S10, M10 |
| H10 | `maps_entities.md` | 3 | HL2:DM's map entity classes beyond specs/source/: `weapon_*` and `item_*` pickups (healthkit 76, battery 44, ammo boxes) and their respawn, `item_ammo_crate`, `item_suitcharger`, `item_healthcharger`, `prop_physics_respawnable`, `func_useableladder` placement data, spawn classes, `env_headcrabcanister` (dm_runoff, 6), `env_flare`, `phys_motor`, `point_angularvelocitysensor`, `func_clip_vphysics`, `game_ragdoll_manager`, `func_dustcloud`, `env_lightglow`; the stock maps' counts (source_deltas.md). No stock map has a combine ball launcher or `func_physbox_multiplayer` (both dropped) | C3, M7 |

H1, H2, H3 and H6 can run side by side (wave 1, after H0); wave 2 reads
wave 1's files; wave 3 last. Board numbers are H0-H10.

### The brief (one per H item)

Each brief is the shared text below with the item's "Write" and
"Cover" lines from the table filled in. The coordinator writes them
out in full on this page before the wave runs, as combat-arms.md's
Wave 5 briefs are.

```text
You are a spec session for Chimera, re-implementing Half-Life 2:
Deathmatch from specs. Read CLAUDE.md and specs/README.md first and
follow their rules exactly. Your reference is Valve's public Source SDK
2013 (its multiplayer game code, which holds HL2:DM's) and the game
installed on this PC; ask me in chat where the SDK is; never write that
path, or any machine path, into the repo or a commit. Read
specs/hl2dm/source_deltas.md and the specs it names for your topic
first: this spec continues them and must not contradict them; where
it does, say so in "Open questions".

Write specs/hl2dm/<file>: <the item's "Covers" list>.

Describe behaviour in your own words: exact constants with units
(Source units, degrees, seconds, ticks), cvar names and their defaults
(players can type them: they are data), formulas as math, per-tick
order, server or client, edge cases, quirks players rely on. Never
paste code, translate it line by line, or copy identifiers, comments or
strings except names the game exposes (cvars, entity classes and keys,
sound entries, model and file names inside the game's archives); no
fenced code blocks except ```math. End with a numbered list of test
cases an implementer without the game can check (setup, input, ticks,
expected numbers), then some the Linux box can check against its own
install or the probe server (say the Steam build you checked), then
"Open questions".

Write only that file; change nothing else. Commit with the repo's
identity (CLAUDE.md "Commit identity"; check `git config user.name` is
r0pi0x, else stop and ask me) in UTC: in PowerShell run `$env:TZ="UTC"`
before `git commit`. End the message with "Co-Authored-By: Claude Opus
5.5 <noreply@anthropic.com>". Never skip hooks. Then `git pull
--rebase` and `git push` to main (or to `specs-incoming` if I say so).
```

Every implementation brief for HL2:DM says: read the spec and our code
only; lift what CS:S already has (section 3) before using it, never
copy it or depend on `cs_source`; record any assumption in the code's
`//!` doc and send the question back for a follow-up spec session.

## 5. Install, assets, console names

**The install.** HL2:DM is Steam app 320; its dedicated server is
232370. Neither is on this box yet.

- The game (app 320) needs an account that owns it: the user logs into
  SteamCMD in a Terminal tab (as for CS:S), then `+app_update 320
  validate` with the Linux build into a games folder outside the repo;
  its path goes into `chimera.local.toml` as `[games.hl2dm] path`, or
  is found through Steam's library folders and the app manifest
  (`appmanifest_320.acf`), which also gives the build id for the mount
  doctor and the asset manifests. The Windows PC installs it through
  Steam for spec sessions and `refcmp` captures there.
- The dedicated server (232370) installs anonymously, like CS:S's
  232330 for `tools/css_probe`, and runs the probe (section 6). It is a
  measuring tool, never the source of a player's assets (open question
  7 asks whether heavy tests may read its files when the game isn't
  installed).
- M0 records the install's facts here (Steam build, `gameinfo.txt`'s
  search paths, the VPKs with sizes, the stock maps and their BSP
  versions, the player and weapon model lists): file names and hashes
  are facts and may be committed, contents never. **Recorded by H0** in
  specs/hl2dm/source_deltas.md, Part 2: game build 24600434, server
  build 24600426, `PatchVersion` 10889068; the search path and VPK sizes;
  the 8 stock maps (BSP v20, revisions, sizes, spawn counts); tick 0.015 s
  (the server also honours `-tickrate`, CS:S's doesn't); the cvars that
  differ from CS:S; the 19 player models; the weapon scripts (plain
  `.txt` beside encrypted `.ctx`: Chimera reads the `.txt`, no key); the
  ammo table. Left for M0 over RCON: the engine's rate cvars and the
  shipped values of the registered defaults (source_deltas.md, Open
  questions 1).
- **The probe on HL2:DM** (H0): the dedicated server's `srcds_run`
  starts the 32-bit `srcds_linux` (64-bit libraries ship, a 64-bit
  launcher doesn't), so the probe runs as CS:S's: Metamod:Source and
  SourceMod 1.12, 32-bit. The SourceMod 1.12 package on this box has
  HL2:DM builds for 32 and 64 bits and `hl2mp` SDK-tools offsets for
  both; whether they match build 24600426 is the first thing M0 checks.
  The replay bot should be a SourceMod fake client, not HL2:DM's new
  NextBot bots.

**The asset service.** The packer is generic (asset-service.md,
"Decisions": a new game needs its mount description, its Steam app id
and a packer run). For HL2:DM:

- The mount description is `hl2dm::game::mount::open_install` (the
  search path, from S1) behind `chimera_core::mount::served::open`, so
  the store layers under or instead of the install like the others;
  tests ask `served::usable("hl2dm", path, "hl2mp")`.
- `chimera_pack` learns the game (C7): `pack hl2dm`, `publish hl2dm`
  (app 320, the whole install, manifest `complete`), `sets hl2dm`
  (`weapon/<id>` per weapon: view and world models, materials, sound
  entries' waves; `players`: the player models), and `base` and the
  maps' sets from `CHIMERA_VFS_LOG` logs of the heavy tests. The
  records of `hl2/` that CS:S already published are shared by id, so
  HL2:DM adds mostly `hl2mp/`.
- Accounts: `chimera_pack user add <name> --games hl2dm,...` for
  testers (internal phase); ownership by app 320 when Steam sign-in
  lands (asset-service.md 4.4 already lists it).
- `docs/assets-for-developers.md` gets HL2:DM's sets and sizes;
  `assets.lock` its pinned build once published.

**Console names.** HL2:DM's names are `hl2dm:` (`GameInfo::
console_namespaces`, canonical first; an alias such as `hl2mp` is open
question 1): its `mp_*` rule cvars, `cl_playermodel`, its HUD's
`hud_*`, its weapons' `sk_*` skill values if the spec keeps them as
cvars. Today a name registered without a namespace is CS:S's
(`console::SOURCE_NAMESPACE`), which covers the Source names Chimera's
engine implements (movement: `sv_gravity`, `sv_accelerate`; network:
`rate`, `cl_interp`; video and audio: `fps_max`, `volume`, `mat_*`).
With a second Source game that is wrong for HL2:DM's app, which
doesn't register CS:S at all. Options (open question 2):

- **A. Keep them CS:S's** and give `hl2dm:` aliases for the same names.
  Smallest change; `hl2dm:sv_gravity` and `cs_source:sv_gravity` would
  be one value in the party apps, though each real game has its own.
- **B. Move the engine-level Source names to `chimera:`**, keeping
  their Source spelling (`chimera:fps_max`), the old full names as
  hidden aliases (`OLD_NAMES`), short names resolving as before.
  Clean for engine-wide settings (video, audio, network), wrong for
  game-tuned ones (gravity, speeds), whose values differ per game.
- **C. Split by kind (recommended).** Engine-wide settings (video,
  audio, network, input) go to `chimera:` as in B; game-tuned ones
  (movement and rules values each game sets) are registered by each
  Source game in its own namespace, backed by its own config
  (`SourceMovementConfig` per movement id after S6), so
  `cs_source:sv_gravity` and `hl2dm:sv_gravity` are separate values,
  and the scope (the gun in hand) picks the short name's game. Saved
  configs and binds keep working through the old names.

## 6. Milestones and tests

Each milestone ends with its tests passing and its docs true. Scenario
tests run on `harness::Hl2Sim` (games/hl2dm/src/harness.rs: Chimera's
`SimBasePlugins` with HL2:DM's parts, as `CsSim` and `CaSim`), one
binary per crate (`games/hl2dm/tests/it/`, slow ones in `heavy/`
skipping without the install or the store), network tests on
`NetSim`. Comparisons with the real game: the **probe** (a SourceMod
plugin of ours on the HL2:DM dedicated server, replaying per-tick
inputs on a bot and logging its state, as `tools/css_probe` does; its
setup in `tools/hl2dm_probe/README.md`, or `css_probe` generalized by
S11) and **refcmp** (the same views captured in the real HL2:DM client
over RCON and in Chimera). The game runs here (Linux build, the user's
Steam login) or on the Windows PC.

| M | What | Tests |
|---|---|---|
| M0 | Install and look: HL2:DM and its dedicated server on this box; `dump` lists the install; the install facts recorded in this plan; S0 and S1 (the crate, the search path from `gameinfo.txt`) | heavy `games/hl2dm/tests/it/heavy/mount.rs` (the search order, every stock map present, `served`'s stand-in); `cs_source`'s mount tests unchanged |
| M1 | Maps load: S2, S3, S4 lifted (move, then generalize); `games/hl2dm` skeleton (`INFO`, `MAP_LOADER`, `Hl2Sim`, placeholder movement and a test weapon); `apps/hl2dm` and `apps/hl2dm_server` play the greybox and every stock map; architecture entries | heavy `map_stock.rs` (each stock map converts: faces, props, entities, lights, spawns), `map_cache.rs` (its key), `mapsweep --game hl2dm` warnings listed; CS:S's whole suite and `refcmp report` on dust2 unchanged; `server_builds_without_presentation` for `hl2dm_server` |
| M2 | Specs: wave 0 (H0), then wave 1 (H1, H2, H3, H6) | the user's review |
| M3 | Movement and the suit: S5, S6 lifted; `hl2dm:movement` from H1; aux power, armour (C2), pickups and chargers from H2 | scenario tests from H1's and H2's cases (`movement.rs`, `suit.rs`); `movecmp --game hl2dm` against the probe (walk, sprint, jump, the jump boost, duck, ladders, water) tick for tick; `net_prediction.rs` with sprint and aux power |
| M4 | Bullets and melee (H3): pistol, .357, SMG1 (no grenade yet), AR2 (no ball yet), shotgun, crowbar, stunstick as TOML in `games/hl2dm/data/weapons/` with hooks; S8; `parts.rs` and `rungs.rs`; C4 (the composer's bundle); **HL2:DM's guns in Ultimate Gun Game** | scenario tests from H3's cases (`weapons.rs`, `melee.rs`); probe fire timings and spread (as `css_probe`'s `firedrop`); party `hl2dm_parts.rs` (like `cs_parts.rs`), `identity.rs` across three games, `net_gun_game.rs` with an HL2:DM rung; `ultimate_gun_game` tests with the install |
| M5 | Projectiles (H4): C1, then SMG1 grenade, AR2 ball, RPG and its dot, crossbow, frag, S.L.A.M. | scenario tests from H4's cases; a network test per projectile (`net_projectiles.rs`); Combat Arms' `special_weapons.rs` unchanged; probe trajectories (grenade arcs, missile and bolt speeds) |
| M6 | Physics as weapons (H5): gravity gun, `+use` carry, physics damage and credit, C6 | scenario tests from H5's cases (`gravity_gun.rs`); punt and throw velocities against the probe; a `NetSim` test of a held prop seen by holder and others; heavy `map_props.rs` on a stock HL2:DM map |
| M7 | Rules and maps (H6, H10): DM and TDM on the mode runtime, spawns, respawn, limits, intermission, team models, C3 (respawning pickups) | scenario tests from H6's and H10's cases (`rules.rs`, `pickups.rs`); `net.rs` (join, team, respawn); heavy `map_pickups.rs` (each stock map's pickups and their timers); the faithful app plays a match with real players |
| M8 | Bodies (H7): player models, multiplayer animation state (S9), hitboxes, ragdolls, view models; HL2:DM's guest dressing on CS:S's and Combat Arms' maps | scenario tests from H7's cases (`animation.rs`, `hitboxes.rs`); heavy `player_models.rs` (every model loads with its activities and hitbox set); party `bodies_drawn.rs`, `rung_characters.rs` with HL2:DM rungs; screenshots |
| M9 | HUD (H8): S7, HL2:DM's panels behind its `ui` feature | `games/hl2dm/tests/ui/` from H8's cases; `refcmp hudcmp` against the real HUD; party `hud.rs` with the HL2:DM identity |
| M10 | Effects and sounds (H9): S10, the rest of S5 | scenario tests from H9's cases; `refcmp` views of each stock map (`tools/refcmp/dm_lockdown.toml`, ...), report within the dust2 thresholds; heavy `sound_stock.rs` (every weapon entry resolves) |
| M11 | Assets: `chimera_pack publish hl2dm`, `sets hl2dm`, the heavy tests from the store alone; HL2:DM's maps in UGG's pool | heavy tests with `CHIMERA_ASSET_USE_INSTALL=0`; `served.rs` |

M1 to M4 are the critical path to the party goal; M5 to M11 can
overlap once their specs land (one lift per area at a time).

## 7. Open questions for the user

1. **Id and namespace.** `hl2dm` as the game id, map ids
   (`hl2dm:dm_lockdown`) and canonical console namespace, with `hl2mp`
   (the game's folder) as an alias? Or another alias?
2. **Engine-level Source console names** (section 5): keep them CS:S's
   (A), move them all to `chimera:` (B), or split engine-wide settings
   to `chimera:` and game-tuned values per game (C, recommended)?
3. **How far the faithful app goes.** The backlog said "not full
   remakes: take what Ultimate Gun Game needs". This plan includes
   `apps/hl2dm` with DM, TDM, every weapon, the suit and the HUD. Do
   its menus (GameUI main menu, create server, options, server
   browser) come too, as CS:S's, or the shell's plain menus?
4. **Bots in the faithful app.** H0 found that the current HL2:DM
   ships bots (NextBot: `hl2mp_bot_add`, `hl2mp_bot_quota`,
   `hl2mp_bot_difficulty`, gravity-gun-only and melee-only modes; they
   generate `.nav` files, none ship). Parity would mean HL2:DM's own bots
   eventually (a spec wave of their own), or Combat Arms' bots as an
   opt-in option for playtests, or none for now?
5. **The odd weapons in Ultimate Gun Game.** The gravity gun needs
   props to throw (CS:S's maps have few): leave it out of the ladder,
   give it a class of its own, or only on HL2:DM's maps? The same for
   S.L.A.M. (a grenade rung?) and the stunstick (a second melee next
   to the crowbar as HL2:DM's knife?).
6. **Where spec sessions run.** The Windows PC with the SDK cloned
   outside the checkout (as Combat Arms' reference is there), or a
   separate session on this box with the SDK in a folder outside the
   repo that no implementation session reads? The plan assumes the PC.
7. **The dedicated server's files.** May heavy tests read maps and
   models from the dedicated server install (232370, free) when the
   game isn't installed, or only the game (320) and the asset store?
   (H0: the server has the same maps, `hl2mp` pak and HL2 misc VPK, but
   only the directory files of HL2's texture and sound VPKs, so maps
   and models load from it, textures and sounds don't.)
8. **The shared crate's name.** `source/engine` (`source_engine`) for
   Source behaviour shared by CS:S and HL2:DM, or another name?
9. **The big lifts first.** BSP conversion (S2) and Source movement
   (S6) leave `cs_source` before HL2:DM can load a map or move. Agreed
   to do them as "move, then generalize", each alone after a full
   baseline, even though they touch CS:S's most tested code?
10. **Install now?** Should this box get HL2:DM (your SteamCMD login)
    and its dedicated server now, so M0 can start alongside wave 0?

## 8. Task board

Status values: not started / spec in progress / spec landed /
implementing (owner branch) / done.

| Item | Spec | Status | Lift |
|---|---|---|---|
| Wave 0 spec | H0 `source_deltas.md` | spec landed (branch `hl2dm-h0`, 2026-11-01; user review pending) | – |
| Wave 1 specs | H1 `movement.md`, H2 `suit.md`, H3 `weapons.md`, H6 `rules.md` | spec landed (branch `hl2dm-w1`, 2026-11-01; user review pending; corrections to H0 listed in each) | – |
| Wave 2 specs | H4 `projectiles.md`, H5 `gravity_gun.md`, H7 `player_models.md` | spec landed (branch `hl2dm-w2`, 2026-11-01; user review pending; corrections to H0 and wave 1 listed in each) | – |
| Wave 3 specs | H8 `hud.md`, H9 `effects_sounds.md`, H10 `maps_entities.md` | spec landed (branch `hl2dm-w3`, 2026-11-01; user review pending; corrections to H0 and waves 1-2 listed in each) | – |
| M0 install and look | H0 (install facts) | done (branch `hl2dm-m0`, 2026-11-01): S0, S1, `games/hl2dm` skeleton, `dump hl2dm`, heavy `mount.rs`; left for M1/M3: the RCON reads and the probe (below) | S0, S1, (S11) |
| M1 maps load | H0 | done (branch `hl2dm-m1`, 2026-11-01): S2, S3, S4 lifted, CS:S's maps identical; `games/hl2dm` maps, `Hl2Sim`, stand-in movement and pistol; `apps/hl2dm`, `apps/hl2dm_server`; `mapsweep --game hl2dm` | S2, S3, S4, (S11: mapsweep) |
| M3 movement, suit | H1, H2 | done (branch `hl2dm-m3`, 2026-11-03): S5 (scripts, soundscapes), S6, C2, C8; `hl2dm:movement`, the suit, pickups and chargers; the probe | S5, S6, C2, C8 |
| M4 bullets, melee, UGG | H3 | done (branch `hl2dm-m4`, 2026-11-03): see Progress; left: probe checks P4, P11, P12, P14, P15 (the rest run: branch `hl2dm-probe`), C7 (no need in M4) | S8, C4 |
| M5 projectiles | H4 | done (branch `hl2dm-m5`, 2026-11-03): see Progress (effects: `hl2dm-fx`); left: probe checks P5-P8, P9's under-water speed and damage, P10-P12, P14, P15 (the rest run: branch `hl2dm-probe`), the effects (M10) | C1 |
| M6 gravity gun | H5 | done (branch `hl2dm-m6`, 2026-11-03): see Progress; `hl2mp_allow_pickup` 1's carry and the crate-on-head damage: branch `hl2dm-probe3`; left: first-impact 10× and breaks, probe checks P7, P8, P12-P15, punts into players | C6 |
| M7 rules, pickups | H6, H10 | done (branch `hl2dm-m7`, 2026-11-03): see Progress; set pieces built (branches `hl2dm-setpieces`, `hl2dm-setpieces2`, 2026-11-03): see Progress; left: rules P6, P9-P10, P12-P14 (P5, P7, P8 and TDM's friendly fire and spawns run: branch `hl2dm-probe3`) | C3 (C5 not needed) |
| M8 bodies | H7 | done (branch `hl2dm-m8`, 2026-11-03): see Progress; left: probe checks (P6-P11), the items listed there | S9 |
| M9 HUD | H8 | done (branch `hl2dm-m9`, 2026-11-03): see Progress; left: the items listed there | S7 (the look reader and the animation engine; CS:S's menu readers stay) |
| M10 effects, sounds | H9 | done in part (branch `hl2dm-m10`, 2026-11-03): see Progress; left: explosions, ball, dissolve, RPG, crossbow, gravity-gun effects (M5, M6), the shield wall shader, refcmp (shells, remote tracers and impact sounds: done 2026-11-04, branch `hl2dm-netfx`) | S10, S5 |
| M11 assets | none | done (branch `hl2dm-m11`, 2026-11-03): see Progress; left: the other heavy tiers' (party, UGG) HL2:DM reads from the store | C7 |

## Progress

- 2026-11-01: plan written (no code).
- 2026-11-01: wave 0 (H0) written: specs/hl2dm/README.md and
  specs/hl2dm/source_deltas.md. Main findings: gravity 600, jump 160
  (about 20 units), hull 72/36, walk/run/sprint 150/190/320 with sprint
  on aux power, no stamina and no bunny-hop cap in the SDK, fixed 10
  fall damage, `func_useableladder` ladders only, footsteps per model and
  none while ducked, no penetration and player damage from the scripts,
  pistol and .357 without spread, head ×3, suit absorbs 80 %, `+use`
  carry off by default, no freeze cam, HL2:DM's animation reached
  through `hl2mp/`'s overrides of HL2's include models, MDL versions
  44–48 all present, FOV 75, HL2:DM ships NextBot bots. Briefs H1–H10
  refined in section 4 accordingly.
- 2026-11-01: wave 1 (H1, H2, H3, H6) written: specs/hl2dm/movement.md,
  suit.md, weapons.md, rules.md (draft). Main findings: no bunny-hop
  cap, no forward jump boost in multiplayer (`player_limit_jump_speed`
  unread); sprint starts only on the `+speed` press and wins over walk;
  ladders pull onto the line and climb at exactly 200; armour fits
  `hits::absorb`'s shape (C2) given separate armour and health quanta and
  the bypass classes; health keeps fractional damage; drowning after aux
  power and 7 s of air; `impulse 50` isn't suit zoom (`+zoom` is);
  pistol has its 1°–6° spread after all; bullets do the script damage to
  props too; 2 spawn grenades; dead players never become observers, so
  `mp_forcerespawn` always applies (shipped `valve.rc` sets 0: button
  respawn after 3 s); no 5 s team-change limit; spawn spots occupied
  within 128 units. Each spec lists its corrections to H0 and its probe
  cases.
- 2026-11-01: wave 2 (H4, H5, H7) written: specs/hl2dm/projectiles.md,
  gravity_gun.md, player_models.md (draft). Main findings: one blast rule
  (linear fall-off to the victim's box, no head bonus, props absorb
  mass/350); any damage shoves players 5 u/s per point (bullets and melee
  too, correcting weapons.md); SMG1 grenade 1000 u/s, no arming; AR2 ball
  1000 u/s, 1000 dissolve damage, never its owner; RPG 150/300, dot always
  on, missile can't be shot down; crossbow 100 ×hitgroup, glances below
  cos 0.5; frag fuse from release, 150/250, throw adds 0.1 lift; S.L.A.M.
  tripmine 200/200 credited to the placer; 0.5 s spawn grace from every
  explosive. Gravity gun: punt impulse 15000 + 600·min(m, 250), pull
  between 250 and 1000, held objects weigh 1 kg and deal no damage, no
  drop on damage, launch 1500 to 100 kg falling to 700, physics-kill
  credit 1 s (2 s for explosive props), `+use` carry off (35 kg, 128 when
  on), respawnable props 60 s by default and only when broken. Bodies:
  HL2's five-activity state (no walk, swim or ladder animation), weapon
  kind from a per-class table not `anim_prefix`, pose parameters not
  networked nor lag-compensated, only head ×3 matters, ragdoll force on
  the pelvis, fixed 0.45 s view bob ignoring `cl_bob*`. Each spec lists
  its corrections to H0 and wave 1 and its probe cases.
- 2026-11-01: wave 3 (H8, H9, H10) written: specs/hl2dm/hud.md,
  effects_sounds.md, maps_entities.md (draft). Main findings: HUD:
  the shared Source HUD rules (640x480 layout, scheme lookup,
  hudanimations events) specced once; below FOV 90 (default 75) every
  weapon but the crossbow shows the default crosshair; `hud_fastswitch 1`
  draws no selection; no team menu and no flashlight indicator in
  HL2:DM. Effects and sounds: the server draws no shots, clients
  re-trace ignoring players; only the shooter's own tracers whizz (AR2
  with `Bullets.GunshipNearmiss`); no ricochet sound; no third-person
  muzzle sprites; dissolve removes the body at 2.0 s; no pain voice;
  LDR path only (refcmp with `mat_hdr_level 0`). Map entities: all 107
  classes of the 8 stock maps classified; pickups respawn at their first
  rest spot (items 2000 ticks, weapons 1333), relocation beyond 64
  units; ammo crate and charger entity sides; `env_headcrabcanister` in
  full (dm_runoff's map canisters likely removed at load); `env_shake`,
  `env_microphone`, `npc_combine_camera` added. Each spec lists its
  corrections to H0 and waves 1-2 and its probe cases.

- 2026-11-01: M0 built (branch `hl2dm-m0`). S0: the crate `source/engine`
  (`source_engine`), with the architecture rules (beside `source_logic`
  under the games; CS:S and HL2:DM may use it, Combat Arms never). S1:
  `source_engine::search_path` parses a `gameinfo.txt`'s SearchPaths
  (`|gameinfo_path|`, `|all_source_engine_paths|`, `custom/*`, the VPK
  `_dir` name, missing entries skipped, `gamebin` and write-only lines
  left out, a folder named twice mounted once) into a `Mount`; CS:S's
  mount moved onto the same builder with its own list
  (`cs_source::game::mount::SEARCH_PATH`), layer for layer as before (its
  order isn't its `gameinfo.txt`'s: loose `cstrike/` before its VPK;
  switching is a parity change of its own, in its `//!` doc). Also lifted
  (second use): `maps::{map_names, map_cycle, parse_map_cycle,
  import_map_into}`, re-exported at CS:S's old paths. `games/hl2dm`:
  `game::INFO` (id `hl2dm`, namespaces `hl2dm` and `hl2mp`, app 320,
  folder `hl2mp`, tick 0.015) and `game::mount` (`open` through
  `served::open`, `open_install`, `search_path`); `chimera_pack` knows
  the game (`pack`/`publish hl2dm`, no sets yet); `dump hl2dm` prints the
  search path and the install's summary. Installs are found as every
  game's, plus a new `CHIMERA_<GAME>_ROOT` override for one run
  (`CHIMERA_HL2DM_ROOT`; the SteamCMD install here is outside Steam's
  libraries). The search path as loaded on build 24600434: `hl2mp/
  hl2mp_pak_dir.vpk`, `hl2_complete/hl2_complete_misc_dir.vpk`, `hl2/
  hl2_textures_dir.vpk`, `hl2/hl2_sound_vo_english_dir.vpk`, `hl2/
  hl2_sound_misc_dir.vpk`, `hl2/hl2_misc_dir.vpk`, `platform/
  platform_misc_dir.vpk`, then loose `hl2mp/`, `hl2_complete/`, `hl2/`,
  `platform/`, `hl2mp/download`, then Chimera's content cache (13
  layers; `custom/` holds only a readme, the English and three
  `hl2_complete` archives are missing). Left from M0's list: the RCON
  reads of the shipped cvar values and the probe's first start (Open
  questions 1 and 2 of source_deltas.md), which need servers on UDP
  ports and go with M3's probe work.

- 2026-11-01: M1 built (branch `hl2dm-m1`). Lifts, each "move, then
  generalize", CS:S's converted maps compared part by part (digests of
  meshes, textures and texels, lightmap, entities, props and models, kit,
  sounds, spawns, warnings, and the whole map) on de_dust2 in all four kit
  modes, cs_office, de_nuke, cs_militia, de_port at HDR 2 and the guest
  dress: identical after each; `refcmp report` on de_dust2 gives the
  baseline's numbers to three decimals. CS:S's and Combat Arms' whole
  suites (`cargo nextest run -p cs_source -p combat_arms --features dev`,
  heavy tests on the installs): 2936 passed, 11 skipped before; 2893
  passed, 11 skipped after, the 43 missing being the moved files' unit
  tests, which pass in `source_engine` (48 there). Order S3, S4, S2 (the three are
  one cycle of calls; the leaves first):
  - S3: `material.rs`, `water.rs` to `source_engine::content`; no CS:S
    names inside.
  - S4: `props.rs`, `anim.rs`, `propdata.rs`, `surfaceprops.rs`, `pcf.rs`
    with the map readers they call (`ambient.rs`, `breakables.rs`,
    `trails.rs`) and `bsp.rs`'s helpers (`to_engine`, `Bounds`,
    `playable_bounds`, `parse_vector`, into `content`); a character
    body's game is `props::load_character`'s argument.
  - S2: `bsp.rs` and `lightmap`, `overlays`, `detail`, `decals`, `sky`,
    `ropes`, `beams`, `steam`, `sprites`, `dust`, `nav`; what it read of
    CS:S is a `bsp::SourceGame` the game passes: id and converter version
    (the cache key, `cache::key(game.id, game.converter)`; each game's
    build.rs hashes its own sources and `source/engine/src`), spawn
    classes and teams, `sv_gravity` (bodies, ropes, gibs), and hooks
    called where CS:S's code was (`kit`, `extra_particles`, `held`,
    `sounds`). Left in CS:S: `fire.rs` (S10; the conversion never called
    it), `soundscape.rs` (with sounds, S5; CS:S's `sounds` hook calls
    it). Determinism counts moved with the files (cs_source 139 -> 50,
    source_engine 89).

  `games/hl2dm`: `game::maps` (`HL2DM`: spawns `info_player_deathmatch`,
  gravity 600, an empty kit), `MAP_LOADER`, `SimPlugins`,
  `game::Hl2dmPlugins`, `harness::Hl2Sim`; `hl2dm:movement` is
  Chimera's placeholder under HL2:DM's id with H1's numbers
  (`placeholder::Tuning`: run 190, sprint 320, gravity 600, jump 160);
  `hl2dm:weapon_pistol` as data (8 a bullet, head x3, clip 18, 150
  carried). `apps/hl2dm` (binary `hl2dm`, FOV 75) and
  `apps/hl2dm_server` play the greybox and every stock map. `mapsweep`
  moved to the sandbox crate's host tools with `--game`.

  The stock maps as converted (build 24600434; every entity count equals
  source_deltas.md's table): dm_lockdown 22 169 triangles, 452 props, 648
  entities (60 lights), 33 spawns; dm_overwatch 8 822, 277, 364 (26), 17;
  dm_powerhouse 42 975, 567, 1400 (326), 18; dm_resistance 27 067, 549,
  630 (108), 20; dm_runoff 23 912, 649, 608 (45), 9; dm_steamlab 39 798,
  1221, 1090 (135), 24; dm_underpass 21 466, 692, 878 (86), 17; halls3
  10 028, 305, 337 (37), 10. No material missing.

  `mapsweep --game hl2dm --stock` (no map cache): 8 of 8 load, no
  failures. Load warnings: static props' baked light (`sp_<n>.vhv`)
  not matching their model, lit by the probe instead (dm_resistance 8,
  dm_runoff 7, halls3 6: `tree_deciduous_card_01.mdl`, checksums
  e0297c46 against the model's 16fedd26, same vertex count); overlays
  without geometry (dm_resistance 1, dm_runoff 8, dm_steamlab 2);
  infodecals without a surface (dm_resistance 1, dm_underpass 14).
  Classes nothing handles yet: the pickups (`item_healthkit` 76,
  `item_battery` 44, `item_ammo_*`, `item_box_buckshot`,
  `item_rpg_round`, `item_suitcharger` 12, `item_healthcharger` 8,
  `item_healthvial`, `item_ammo_crate`: M7, H10), `func_useableladder`
  (26 on 6 maps: M3, H1), `game_ragdoll_manager`, `env_flare`
  (dm_resistance 7), `info_lighting_relative`, `env_microphone`,
  `npc_combine_camera` (dm_underpass), `point_angularvelocitysensor`
  (H10). Map logic notes: ladder dismount points (6 maps),
  inputs to names nothing has (2), ambient_generic presets, a trigger
  naming a filter, `trigger_hurt` radiation timing (dm_underpass).

  For M2/M3 (and later): HL2:DM's kit is empty (no bodies, HUD, held or
  view models, impact looks) and its maps are silent until S5 (sound
  scripts, soundscapes) lands; team spawns and `info_player_start` wait
  for H6's spawn choice (M7); the movement is the placeholder until S6
  and H1 (M3), the pistol a stand-in until H3 (M4). dm_runoff's first
  spawn starts the camera inside a wooden box-like room (unchecked
  whether that is the map's hut or a prop around the spawn). The rest of
  the tools' Source half (S11: refcmp, movecmp, hudcmp drivers) is
  still CS:S's.

- 2026-11-03: M4 built (branch `hl2dm-m4`). **Weapons as data**
  (games/hl2dm/data/weapons): pistol, .357, SMG1, AR2, shotgun,
  crowbar, stunstick with H3's numbers; the install's plain scripts and
  view models agree (heavy `scripts.rs` through the new
  `source_engine::weapon_script`: bucket, position, weight, clip,
  `default_clip`, ammo, damage, models, names, sounds; draw and reload
  lengths from each `v_*.mdl`). Hooks: `hl2dm:inventory` (holstered
  auto-reload), `hl2dm:pistol` (accuracy ramp 1-6 degrees, click refire,
  attack2 block, dry fire), `hl2dm:punch` and `hl2dm:machine_gun` (HL2's
  punch spring, kicks moving the shot), `hl2dm:shotgun` (the pump). Chimera
  gained, generic: `SpreadShape::Cone` (HL2's cone, `FireBullets::shape`
  on the wire), `ShellReload::insert_first`, `AltShot` (the double shot),
  `WeaponEventKind::Action`, `drop::PickupRules` (touch delay, several to
  a slot, owner pickups removing the weapon, the dead dropping what they
  hold, dropped weapons gone after 31 s). The game's rules
  (`Hl2WeaponsPlugin`): loadout by model kind (crowbar for citizens,
  stunstick otherwise; SMG1 in hand, pistol last: no physcannon or frag
  yet), ammo per player (`Hl2Ammo`), the next best weapon on an empty
  one; the damage shove of HL2:DM players, 5 u/s per point
  (projectiles.md 3). Tests: hl2dm `weapons.rs`, `melee.rs` (H3 cases 1-26,
  32-35, 44, 49-50; the shove).
  **S8**: `source_engine::view_anim` (Source's activities, the events'
  effects, shared reactions) and `view_motion` (moved from CS:S, now
  per-game `MotionRules`: CS:S's cvar bob and history sway, HL2's fixed
  0.45 s bob and spring sway with pitch offsets, player_models.md 12);
  CS:S's rules keep the knife, silencer, Elites; HL2:DM's view models
  (`game::view`) are right-handed, never mirrored, 54 at 75 degrees, in
  its maps' kit and as a guest on other maps (`maps::dress_guest`).
  **C4**: `chimera_world::game_bundle` (`GameBundle`; the front end apart,
  `GameClient`, since a game's parts never name its `ui`); each game
  exports `BUNDLE`; `party_kit` loops over its guests (CS:S, HL2:DM) for
  map loaders, guests' dressing, kept models, parts, binds, view-model
  fields of view, the simulation, names and the client. The base game
  (Combat Arms) and the pairs' data (spawn teams, surface tables) stay
  named. **UGG**: `parts.rs` (pistol, heavy-pistol .357, SMG1,
  shotgun, AR2 rifle, the crowbar, `hl2dm:movement`, dm_lockdown and
  dm_overwatch, `dm_*`), `rungs.rs` with data/rung_characters.toml (a
  model per rung from the 19, Combine or Rebels; the Combine's give the
  stunstick), the guns on the power curve; no bodies or HUD (M8, M9: the
  fallbacks). Party tests: `hl2dm_parts.rs`, identity across three
  games, an HL2:DM rung over the network; UGG `parts.rs` with three
  games; hl2dm heavy and UGG tests on the installs. C7 (the plan's
  `chimera_pack` list; the brief's "held props" is C6) wasn't needed.
  Left: the probe checks (fire timings, spread: P1-P17 of weapons.md)
  wait for M3's probe; armour (the suit, C2) and under-water refusal are
  M3's; the box pellets, the melee re-trace, the 400 in/s drop velocity,
  `mp_weaponstay` and 20 s respawns (M7), a remote client's double shot
  drawing 7 pellets (`//!` docs list them).

- 2026-11-03: M3 built (branch `hl2dm-m3`). Lifts, each "move, then
  generalize", after a baseline of CS:S's, Combat Arms', source_engine's
  and hl2dm's suites (2958 passed, 11 skipped):
  - S5 (the part M3 needs): `sound.rs` and `soundscape.rs` to
    `source_engine::content`; CS:S's own entries stay in its
    `sound::load`. CS:S's map sounds compared entry by entry (waves by
    content), soundscapes and step remaps, on de_dust2, cs_office,
    de_nuke, cs_militia, de_port, de_aztec, cs_italy, de_train:
    identical. `impacts.rs`, `scrapes.rs` and `pain.rs` stay CS:S's (M10).
  - S6: `movement.rs` (with its tracer), `drown.rs`, `shadow.rs`,
    `pushaway.rs` to `source_engine::source_movement` (a module name of
    its own in the layering table); then a config per movement id
    (`SourceMovementAppExt::source_movement`; CS:S keeps the
    `SourceMovementConfig` resource and its bare cvars) and HL2's parts
    as config switches (fall thresholds and fixed or fractional damage,
    swim lift, model footsteps, no CS:S drowning, no contents ladders,
    `func_useableladder`s with forced moves and dismounts) and
    `SourceSpeed` (a game's max speed, before `SourceMovementStep`).
    CS:S's movement traced over 17 500 scripted ticks (ground, steps,
    ledges, a ladder, both pools, both configs: position, velocity,
    ground, duck, eye, roll, water, ladder, step timer, surface, fall
    speed): bit for bit after the move and after the generalization;
    CS:S, Combat Arms and source_engine suites with heavy tests: 2943
    passed (the baseline less hl2dm's). `movecmp` wasn't run: it starts
    CS:S's probe server on UDP 27030, which this session doesn't bind.
  - C2: damage classes FALL, DROWN, RADIATION; `Coverage::except`;
    `absorb_quanta` with the armour's own quantum
    (`ArmorPieces::armour_quantum`); `core::DamageRemainder` in
    `apply_damage`; `weapon::ViewZoom` (the suit zoom's FOV). CS:S's
    kevlar and Combat Arms' gear unchanged (both suites pass).
  - C8: HL2:DM's movement cvars are `hl2dm:` (`hl2dm:sv_gravity` ...,
    its own config), with `hl2dm:hl2_walkspeed`/`normspeed`/`sprintspeed`,
    `mp_falldamage`, `sv_ladder_useonly`, `sv_autoladderdismount`,
    `mp_flashlight`, `sv_infinite_aux_power`.

  `games/hl2dm`: `game::movement` (H1 on Source movement, the speed
  selection and the sprint's aux power), `game::suit` (armour, the
  health remainder, aux devices, breath and drowning, flashlight, suit
  zoom, spawn reset), `game::pickups` (kits, vials, batteries, suit and
  health chargers with the citadel and Kleiner flags), the maps' sounds
  (Source's scripts with HL2:DM's entries, soundscapes). Tests:
  `tests/it/movement.rs` (H1 rows 1-47, 49-61 but 48 and 62),
  `suit.rs` (H2 tests 1-11, 14-18, 25-27, 29-35, 40-50, 52),
  `net_prediction.rs` (walk, sprint, aux power, duck and jump predicted
  bit for bit at 100 ms), heavy `map_ladders.rs` (26 ladders, 131
  dismount points; dm_resistance's), unit tests in source_engine
  (ladders), chimera_sim (the suit's coverage), chimera_core (the
  remainder).

  The probe (tools/hl2dm_probe, port 27040, dedicated server build
  24600426): Metamod and SourceMod 1.12's 32-bit HL2:DM builds load; an
  `hl2mp_bot_add` bot driven by the plugin. Matched tick for tick:
  ground acceleration (run, walk, sprint), friction, the standing and
  ducked jumps (grounded at tick 34 and 35), the air duck's 36 lift and
  eyes, the duck curve, ducked acceleration, the sprint's aux power
  (first tick free, standing free, tap cost, recharge delay, empty at
  268), the sprint kept ducked, no jump cap with `player_limit_jump_speed`
  either way, dm_resistance's ladder (mount, 200 climb, every direction
  rule incl. back climbing up, jump-off, the top dismount), falls (no
  damage at 526.5, 10 above, progressive 62 of 62.5), run footsteps on
  ticks 5, 32, 59 at 0.2 and none ducked, and the cvars at start. Found:
  `sk_suitcharger` is 30 on the running server (the spec says 75: a spec
  follow-up), `mp_forcerespawn` 0 on the dedicated server. Results in
  movement.md and suit.md "Open questions".

  Left: probe cases P11, P14-P16 (movement), P1-P2, P6-P11, P13 (suit:
  damage by kind, drowning, chargers), the back-hop; `movecmp --game
  hl2dm` (S11: the tool's driver is CS:S's); the suit zoom's owner (the
  crossbow, H4) and `toggle_zoom`; `impulse 100`'s bind and network path;
  items drawn and resting where they fall (M7); `+use` deny sounds (M9);
  `trigger_hurt`'s poison and radiation kinds (absorbed as generic now);
  devices across death (suit.md Quirks, unconfirmed); H1 rows 48 (heavy,
  dm_lockdown's ladder brushes) and 62 (the water jump); the waist-water
  step counter; `player_old_armor`.

- 2026-11-03: M7 built (branch `hl2dm-m7`). **C3** (`chimera_sim::weapon::
  respawn`): with a game's `RespawnRules`, a map pickup records the spot
  where it first rests (rest checks every 0.1 s), a taken item hides there
  and comes back after its delay (a think on the nearest tick: 2000 at 30
  s), a taken placed weapon leaves a copy appearing 1333 ticks later
  (`Copied`: no map outputs), and at each pickup's own check (30 s items,
  20 s weapons, first on the tick it rests) one at rest farther than 64
  units goes back; `PickupRules::weapon_stay` (`mp_weaponstay`: copies at
  once, owners refused). **Pickups** (`game::pickups`, `ammo`,
  `ammo_crates`): items are physics objects; the ammo box table and crate
  types as data (data/items.toml; the M5/M6 ammo types' limits in
  ammo.toml); boxes used up whole, `OnCacheInteraction`, `OnPlayerTouch`,
  `ammo_pickup`; `item_ammo_crate`'s timeline (ammo on T + 63, `Close`
  T + 105, full T + 182) and crowbar use; suit chargers' `OnFull` (at load,
  on refill), `OnHalfEmpty`, `OnEmpty` (dm_lockdown's megacharger light);
  `sv_hl2mp_item_respawn_time`, `sv_hl2mp_weapon_respawn_time`,
  `mp_weaponstay`. **Rules** (`game::rules`): DM and TDM as two modes on
  the mode runtime (`hl2dm:dm`, `hl2dm:tdm`: one live round from the map's
  load, decided by the limits, its round over the intermission, the match
  over the map change; C5 wasn't needed, nothing lifted from CS:S): joining
  (DM team 0, every player its own engine team so `say_team` reaches only
  its sender; TDM the smaller team, random on a tie), the 19 models
  (data/player_models.toml) with the shared round-robin counters, the 5 s
  model timer reset by spawns, `cl_playermodel` (userinfo), `jointeam`,
  `spectate`, `joingame` (0.3 s rate, a TDM suicide charged to the new
  team, a frag back when going to spectate; `Console::player_command`, so
  a client's go to its server), the spawn ring walk (the empty slot, the
  first spot doubled), 128-unit occupancy, the 48-unit telefrag
  (`worldspawn`), team classes falling back to deathmatch's, then
  `info_player_start`; the respawn (buttons released once, then a press,
  or `mp_forcerespawn`'s 5 s, after more than 3 s: ticks 201 and 334); the
  0.5 s pure-blast grace; the 5 s suicide limit; scoring (team kills -1
  frag and +1 to the killer's team, suicides -1 to the victim's team, no
  frags or deaths in the intermission); death-notice names; `player_death`,
  `player_team`, `player_spawn`, `round_start`, log lines with HL2:DM's
  team names; frag (DM player, TDM team) and whole-minute time limits; the
  frozen intermission (`mp_chattime`) and the next map (`nextlevel`, the
  cycle, the first when off it); `mp_restartgame` (the map's entities
  again through `core::restart_round`), `mp_readyrestart` (`player_say`,
  now fired by `net::chat`), `timeleft`. The dead stay at their body (M8's
  `core::OrbitDeadView`; M7's own copy dropped at the merge). HL2:DM's chat
  formats in its map kit, the living reading the dead
  (`SayFormats::dead_heard`). The apps start the rules on their first
  map (`rules::AutoStart`). Tests: `pickups.rs` (H10 cases 1-9, 11, 12,
  14, 16, 17, 20, 23), `rules.rs` (H6 tests 1, 2, 4, 5, 7-21, 24-56 in
  part: the ring walk's unit test, 26-29, 30-35, 36-43, 44-46, 53-56),
  `net.rs` (join on a team, `jointeam` from a client, respawn), heavy
  `map_pickups.rs` (every stock map's items, chargers, crates: 329 items,
  200 ammo boxes; a kit back after 2000 ticks, a weapon's copy after
  1333). Also: the heavy app test `every_stock_map_plays` put players
  half a hull too low since M3's Source movement (fixed separately).

  The probe (port 27040): `mp_forcerespawn` reads 0 again on a plain
  start; killing the `hl2mp_bot_add` bot (`sm_slay`) crashed the
  dedicated server twice, so P2-P14 of rules.md and P1-P10 of
  maps_entities.md weren't measured (the respawn and copy ticks are the
  spec's). Left: H10's set pieces (sections 8-16: canisters, flares, the
  fan's motor and sensor, the microphone, the camera, `env_shake`,
  `game_ragdoll_manager`), `prop_physics_respawnable` (section 5, with
  M6), the respawn light, chargers' `Recharge`/`SetCharge` inputs and
  value outputs, HL2:DM's scoreboard and death notices (M9), the
  server rewriting a remote client's `cl_playermodel`, the probe cases.
- 2026-11-03: the set pieces of H10 sections 4-5 and 8-16 (branch
  `hl2dm-setpieces`; what M7 and M6 left). **Generic** (Chimera,
  source_logic): a game's own map classes (`map::entities::
  GameEntityClasses`): the logic delivers their inputs as
  `MapEntityInput`, tells of the entities (`MapEntitySpawned`, also a
  `point_template`'s copies, `MapEntityRemoved`) and takes outputs back by
  map index, by key (`FireKeyedOutput`) or with a number
  (`FireEntityOutputValue`); an AI sound list (`AiSound`), heard by
  **`env_microphone`** (`logic::sensors`: mask, sensitivity, range, smoothing,
  `OnHeardSound`, `SoundLevel`, `Enable`/`Disable`) and **`point_
  angularvelocitysensor`** (a prop body's spin reported by the bridge, the
  spec's change-then-interval firing, `Test`, `TestWithInterval`);
  **`prop_physics_respawnable`** (`RespawnTime`, kept name and outputs,
  the body put back at its home when its spot is free, 1 s retries; unit
  test); **`env_flare`** (a `Part`; the loader draws its red flare and white
  core as sprites, `Start`/`Die`); `env_shake`'s amplitude cap 16 and its
  reach to network clients (`MapFx::Shake`); `npc_combine_camera` as a
  still model (the loader). **HL2:DM** (`game::set_pieces`): the canister
  (`FireCanister` once, flight of `FlightTime` at `FlightSpeed`, incoming
  sound, sweep, blast with the squared falloff, 750 shove, player push 250
  to 1000, shake, landing, `OnLaunched`/`OnImpacted`/`OnOpened`; dm_runoff's
  strike: the map's six are removed at load by their template, so the first
  strike fires nothing and the next the six it spawned), the ragdoll
  manager (`RagdollLimit`), bullet and blast AI sounds; the chargers'
  `Recharge`/`SetCharge` and value outputs (`OutRemainingCharge`,
  `OutRemainingHealth`); the respawn light (`ui::lights`, listen/standalone
  only). Tests: source_logic `set_piece_tests` (4), hl2dm `set_pieces.rs`
  (5), `pickups.rs` `charger_inputs_and_value_outputs`, heavy
  `map_set_pieces.rs` (every stock map: no census class without a handler,
  329 respawnable props, 7 flares drawn, the ragdoll managers' 4, the
  camera's model, dm_runoff's two strikes, dm_lockdown's fan). Left: the
  canister's model, trail, smoke and explosion effect, the 3D skybox arc,
  headcrabs; the camera's sweep, pings and photos; the flare's smoke, light
  and jitter; chargers' dial and loop-sound stop; the fan: the physics
  motor spins the fan between 1500 and a crawl (`phys_motor` in
  `map::controllers`), so the sensor's `OnGreaterThan` is unit-tested only;
  the physics shake and rope shake of `env_shake` (flags 8, 16); the
  microphone's speaker routing; the respawn light on network clients.
- 2026-11-03: dm_lockdown's fan holds its speed (branch
  `hl2dm-setpieces2`). Cause of the flicker between 1500 and a crawl: the
  fan's `phys_hinge` (spawnflags 9) joins it to the world with "No
  Collision", which our joint kept only against its stand-in anchor body,
  so the fan rubbed on the two static props of its housing (and the
  brushes) every step. Now a body joined to the world with "No Collision"
  doesn't touch the world's brushes or static props
  (`map::controllers::NoWorldContacts`, `WorldCollider`, in
  `MapCollisionHooks`). `phys_motor` also follows spec 10 now: its target
  ramps (spinup 0: speed per second, test case 35) and the drive closes
  `inertiafactor` × step of the gap, capped (hinge motors keep their
  at-once spin). CS:S stock maps have no `phys_motor` and no constraint
  with flag 1 (their entity lumps scanned), so only community maps' motors
  see the ramp. Tests: chimera_world `motor_tests` (3), cs_source
  `a_motor_holds_its_speed_on_a_world_hinge`, heavy `lockdown_fan_is_watched`
  (29 of 30 samples from 1.5 s to 4.5 s above 1450, the other a tick's
  dip where it bats the healthkit lying in it; `OnGreaterThan` once,
  `OnLessThan` after the motor stops). The schedule ratchet's FixedUpdate
  fell to 223 pairs, 37 opposite (the world-contact sync lives in
  `sync_joints`: a system of its own reordered FixedPostUpdate).
- 2026-11-03: the set pieces' looks (branch `hl2dm-setpieces2`).
  **Canister**: drawn on every machine as shown items (`ShownBody`, so
  `NetItem` takes them to clients): `headcrabcannister01a` in flight on
  its line, spinning 90 deg/s, then `headcrabcannister01b` at the impact
  with its angles and origin, posed at four frames of `open` through the
  opening (`map::posed_model`, `props::load_held_posed`; a skeleton-less
  held model stays in model space), smoking until `SmokeLifetime` or
  `StopSmoke`; `game::set_piece_fx` draws the trail (`sprites/smoke`,
  additive, 32 ± 15 to 200 wide over 3 s), the landed smoke (8/s, 2 s,
  grey 0.7 to 0.6, 32 to 64) and the impact's explosion (`CanisterBlast`,
  sent to clients; none with flag 524288, so none on dm_runoff, where the
  map's `env_fade` whites the view at the landing). **Flares**: smoke
  (8/s, red, rising) and, without flag 1, the dynamic light (the stock
  ones have flag 1). **Chargers**: their fixed models from the loader,
  `idle` held at the dial's frame (`map::PropCycle`, 1 − display/max,
  the display running 1/7 a tick ahead while used, 10/7 the citadel's;
  host only), the loop sound a long sound under the charger's key,
  stopped at switch-off. **Respawn light**: `RespawnLight` from the host,
  sent to network clients (net part `hl2dm_set_pieces`). Tests: hl2dm
  `set_pieces.rs` `a_canister_is_shown_in_flight_and_landed`,
  `a_canister_with_impact_effects_blasts_and_stops_smoking`,
  `pickups.rs` `a_chargers_dial_and_loop`, `set_piece_fx` unit tests,
  heavy `the_canisters_models_are_loaded` (the doors move 44.5 units
  between the first and last frame), dm_underpass's 698 props (the five
  chargers). Live shots: dm_runoff's second strike (trail, canister,
  landed open canisters smoking), a dm_resistance flare, dm_lockdown's
  fan spinning. Left: the 3D skybox arc, headcrabs, the dust cloud of
  the impact effects, the opening as a played sequence, the landed
  canister's collision; the flare's jitter and burn-out shrink; the
  dial on network clients; the camera's sweep.
- 2026-11-03: M8 built (branch `hl2dm-m8`). **S9**:
  `source_engine::player_anim`, the shared half of CS:S's `player_anim`
  (the inputs from the simulation and the weapons' events, the drawn
  update at the frame rate, a server's per-tick pose into `SimAnimator`,
  the hands' IK locks), generic over a game's `PlayerAnimRules` (a
  component per body; `PlayerAnimPlugin`/`SimAnimPlugin` per game, with a
  filter: CS:S's hostages); CS:S's `PlayerAnim` keeps its sequences by
  weapon suffix, its tests and the schedule ratchets unchanged.
  **Bodies** (`hl2dm::game::bodies`): the 19 player models in HL2:DM's
  maps' kit and as a guest on other games' maps (all 19, named by path;
  the DM default, the Combine soldier, unnamed), animated through
  `hl2mp/`'s include overrides (the search path's order; includes are
  case- and slash-insensitive), the weapons' world models held on them;
  `Hl2Model` worn by name (`BodyName`) and replicated (net part
  `hl2dm_bodies`). **Animation** (`game::player_anim`, `game::look`):
  H7's state (five activities, the weapon kind's fixed table, cycle 0 on
  every change, rate = speed / the blended authored ground speed with no
  clamp, `move_yaw` from the gait yaw, the feet planted to 45 degrees or
  3 s then 360 degrees/s with the un-normalized fade, `aim_pitch`,
  `aim_yaw`, fire and reload gestures on overlay layers, the autoplay
  head and spine sequences: `Animator::autoplay`); clients' heads and
  eyes toward the first player within 128 units in front (else 512
  ahead) at 130 degrees/s. **Hitboxes**: each model's set, only the head
  x3 (the weapons' data), Combine pelvis and chest generic. **Ragdolls**
  (`map::ragdoll::RagdollGameRules`): HL2:DM's push body 0 with the hit's
  own force, aim at no death pose and stay after the respawn until the
  owner dies again (`Outlived`). **The dead view** (`core::OrbitDeadView`
  in the shell's spectate, HL2:DM's own app): its ragdoll from 96 units
  behind, 14 above, along the view. **Faces** (`map::face`, adapted from
  hl2-rs): the Eyes shader (the eyeball's own normal flattened along its
  up, half-Lambert, the iris projected toward `EyeTarget`, clamped to the
  30 degree cone), Teeth (lit toward the mouth's forward times the mouth
  descriptor: dark at rest), flexes (descriptors, controllers, rules,
  FACS lids; each body's own copy of a face mesh moved on the CPU when
  its `FaceWeights` change), blinks every U(1.5, 4) s of 0.2 s, every
  other controller 0. Also: VMT conditions with a shader-model suffix
  (`">=dx90_20b"`, the Combine's base textures) count as their level;
  `AnimSet::ground_motion` (blended authored velocity); a body name a map
  lacks falls back to the identity's game's body; Combat Arms' dressing
  takes off only its own body names. Party apps: HL2:DM rungs draw their
  model (UGG on de_dust2 checked by screenshot).

  Found against the spec (the install, build 24600434): `male_02.mdl` has
  three more own sequences (`Envelope_Anim`, `Choked_Barnacle`,
  `Crushed_Barnacle`), so its merged indices are 3 past the others' (1.2
  says the same on all 19; we choose by activity and name, so nothing
  depends on them); iris scales are 1.4706, 1.5152 and 1.5873 on either
  sex (11.4's per-sex values are two of them); the blended ground speed
  halfway N-NE is the vector's, 193.82 (open question 2, by the files);
  an eyeball's stored forward is its gaze (out of the face).

  Tests: hl2dm `animation.rs` (H7 cases 1, 3-35), `hitboxes.rs`
  (54-56), `ragdolls.rs` (39, 42), heavy `player_models.rs` (every model
  with its 105 animation sequences, activities, pose ranges, hitbox set,
  ragdoll and masses, textures, eyes and teeth; the run cycles' ground
  speeds; case 36's rules and lids), chimera_world `face` (37, 38),
  chimera_client `dead_view.rs`, party `bodies_drawn.rs` and
  `rung_characters.rs` with HL2:DM rungs (over the network). Left: the
  probe checks P6-P11; the server's state takes the command's own eye
  angles (the spec: the previous command's), lag compensation rewinds
  the posed hitboxes whole (the spec keeps current pose parameters,
  case 57); a ragdoll's push is the killing hit's force, not the
  command's sum (shotgun); no glint; a ragdoll's face keeps its last
  weights; `flex_smooth`; the lids take the rest gaze; the dead view's
  pull-in is the shell's sphere, not a box; every map load converts all
  19 models; dissolving ragdolls (M5/M10); `cl_playermodel` and team
  models (M7).
- 2026-11-03: M9 built (branch `hl2dm-m9`). **S7** (partial, as HL2:DM
  needs it): `source_engine::hud::look` (colours with the first of a
  duplicate name winning, the scheme's fonts, font files by family and by
  file, the layout through `parse_pc`, icons from the texture lists, a
  brace glyph's quoting) moved out of CS:S's `game::hud` (CS:S's `load`
  calls it, behaviour kept; its scopes, scoreboard pictures, screens and
  menus stay); `source_engine::hud::anim`, the HUD animation scripts
  (`hudanimations.txt`: events, `Animate`/`RunEvent`/`StopEvent`/
  `StopAnimation`/`StopPanelAnimations`, Linear/Accel/Deaccel, restart
  rules, the start value taken on the first frame, platform conditions on
  event and command lines); `WeaponScript::icons`. `vgui.rs`/`gameui.rs`
  stay CS:S's until a second game draws menus. **The look**
  (`hl2dm::game::hud`, in the map's kit): the install's layout, scheme,
  fonts (HalfLife2 from `hl2mp/`, GorDIN by weight, HL2cross from Half-Life
  2's), icons, each weapon's script icons (`weapon_smg1/weapon`), the
  strings, the chat box's place (`basechat.res`) and the animation script
  (kept in the strings under `@hudanimations`, so it travels with the
  map's cache). **The HUD** (`hl2dm::ui`, feature `ui`): `Hl2Hud`, one
  state machine that decides which named event a change starts (health:
  above or below 20, low pulse; suit; ammo and its weapon switches; aux
  power with its device lines and low colour; team line fade; selection
  open, fade and close; damage by angle; spawn reset) and what each
  variable draws, as a `Frame` (a list of boxes, text, sprites and rings
  in window pixels, so the layout is tested without a window): numeric
  panels with their glow, aux bar, damage flashes, buckets selection with
  `slotN`, `invnext`/`invprev`, attack-confirm and `cancelselect` taking
  the shell weapon menu's place (`hud_fastswitch` 0 and 1), pickup history
  (five rows, "FULL"), death notices (skull, icons by weapon name), quick
  info and its blink, crosshair by FOV (the default sprite below 90, the
  spec's quirk), suit zoom overlay, target ID, the scoreboard (DM and
  TDM sections, the DM header on the spectators, sorted) held with
  `+showscores` and up at the intermission. `game::standing` replicates
  each player's HL2:DM team and the match (team play, intermission, team
  scores). The app adds `Hl2dmUiPlugin`; party apps `Hl2dmHudPlugin`
  (`ui::party::CLIENT`, `GameParts::hud` is HL2:DM's id). The chat is the
  shell's, in the `HudChat` box. Tests: `games/hl2dm/tests/ui/` (H8 test
  cases 5-8 as the animator's, 6, 8-23, 24-34, 35-44, 45-51, 52-57,
  58-63, 64-66, 67, 68 on a stand-in look carrying the spec's numbers and
  our own script of its events; 1-4 as the look's unit tests), the
  modules' unit tests, heavy `hud_look.rs` (layout numbers, first colours,
  fonts by height and their files, icons, strings, every event the HUD
  starts is in the script), party `hud.rs`, `rules.rs` (standing),
  `source_engine`'s animator tests. Checked in the app on dm_lockdown and
  dm_overwatch (listen server and two clients, screenshots): the panels,
  fire flashes, empty ammo red, low health box and bracket, pickup rows,
  death notice with the skull, the scoreboard, the intermission and the
  dead view (the camera orbits the player's own ragdoll: it is in frame).
  Found against the spec: `hud_textures.txt` has `"character" "{"` glyphs
  (quoted braces break the KeyValues reader: now escaped before parsing);
  the layout's panels are conditioned on the block name (`HudHealth
  [!$DECK]`), which `parse_pc` takes; the scoreboard header token is
  `%s1    -   %s2 players` (name and count); `HudPlayerDeath` ends at
  255 alpha red (added, so the scene still shows; but the live capture
  of the dead view about 4 s in is solid red with the ragdoll barely
  visible, so the blend or the alpha's end needs a look against the
  real game). Left: the secondary
  ammo panel waits for M5's secondary ammo (the `WeaponNow::secondary`
  field is None); history items (battery, kit) are told by whole steps of
  armour 15 and health 25, not the server's item message; ammo-denied
  rows ("FULL") have their rules but nothing feeds them (the server's
  refusal); the damage flashes are straight bands and the fullscreen one
  only is additive; the scoreboard has no avatars, its server name is
  empty and its rows start 26 units down; target ID has no trace (the
  state and drawing are built, nothing fills it: needs a view ray); MOTD,
  hints, the console selection styles, the spectator GUI; HL2:DM's HUD on
  other games' maps (the map's look is the other game's: needs a second
  look resource); `refcmp hudcmp` (no reference captures of the real
  client were made: not launched); the probe checks P1-P9; the lift of
  the menu readers; `hurtme` is not a directed hit, so its damage message
  draws no side flash (real hits do).
- 2026-11-03: M10 built in part (branch `hl2dm-m10`). **S10 and the rest
  of S5** (move and generalize in one commit; CS:S's tests unchanged):
  `impact_effects.rs`, `impacts.rs` and `scrapes.rs` moved to
  `source_engine::effects`, re-exported at their old `cs_source::game`
  paths. Each game adds the plugins with an `ImpactStyle` (id, ricochet,
  bullet splash scale, pellet sound grouping, damage unit, the flesh
  surface; `ImpactStyles` in the app, the effect systems added once and
  asking `ImpactRoute` which game draws a shot); `surfaces(game)` and
  `as_game` give any game's surface classes; `HitSoundHere` marks the
  characters whose bullet hit sounds as the surface's flesh impact (CS:S's
  hostages, HL2:DM's players). CS:S keeps its tracers (one every 4th), its
  effect materials, its ricochet and its pain module. A bug found on the
  way: guarding the effect plugin by `Particles` skipped its systems in an
  app with the presentation (use a marker). Also in the lift:
  `SoundScripts::load_with` (an entry defined twice keeps the first, for
  HL2:DM's `NPC_Citizen.die`; CS:S keeps the last) and `$gender` waves
  (entry `<name>` male, `<name>$female`).
  **HL2:DM** (`game::effects`, `game::sounds`, `game::flashes`): its style
  (no ricochet, splash 4 to 8, every pellet's sound), tracers by weapon
  (counter first, every 2nd or 4th bullet, the AR2's own look), the
  shooter's own near-miss sounds (24 and 72 units, 0.1 s), the AR2's flat
  flash and the stunstick's glow and sparks, impact decals and effect
  materials in the kit, every weapon's sound entries, the flesh impact
  from a hit player, `HL2Player.BurnPain`, death voices by model kind
  (female citizens' variant, none under `Dissolving`), footsteps by model
  kind, and five muzzle-flash kinds picked by the view model's event
  option (`MapData::kit.muzzle_flash_kinds`, `ViewModelEventKind::
  MuzzleFlashOf`, `MapMuzzleFlash::alt_textures` and `no_world_sprites`;
  the renderer draws a sprite per texture and keeps CS:S's flash as it
  was). Tests: `games/hl2dm/tests/it/effects.rs` (H9 cases 1-3, 7-9,
  13, 18-26, 35), heavy `sound_stock.rs` (every entry resolves, the first
  `NPC_Citizen.die`, decals), unit tests. The dead view's fade: the
  spec's red is additive (`vgui/white_additive`, alpha 255 at 4.3 s: R
  saturates, G and B keep the scene). Cause of the solid red: that
  material names no `$basetexture` (Source draws it with the white
  `vgui/white`), so the picture was never loaded and the HUD fell back to
  an alpha blend of the opaque red; the look now loads `vgui/white` for it
  (heavy `hud_look.rs`, `the_additive_picture_is_there`). Not re-captured in
  the app (box loaded). Spec against the install: the heavy test found
  every entry; `NPC_Citizen.die` waves carry `$gender`, which the loader
  now expands. Left for M5 (clear hooks: `Dissolving`, `ImpactStyle`,
  `flat_quad`, `Tracer`): explosions and their sounds, the energy ball's
  look and rings, the dissolve (colour, float, sparks), the RPG's laser
  and trail, the crossbow's blur and glow, the SMG1 grenade's smoke; M6:
  the gravity gun's sprites and beams. Left in H9: shells (view-model
  event numbers unread), smoke puffs of the pistol and .357, the muzzle
  core's colour, remote shots' re-trace and tracer flags, wall-blood and
  the shooter's predicted blood offset (blood is the server's copy at the
  hit), the shield wall's `UnlitTwoTexture` shader, impact sounds of
  remote shots, `refcmp` views (no reference captures: the real game was
  not launched), the probe checks P7-P11.

- 2026-11-03: M5 built (branch `hl2dm-m5`). **C1** (Chimera,
  `chimera_sim::weapon::projectile`): a flyer (`Flyer`: speed, gravity by
  Source's exact toss, a box, sphere or point swept against
  `SOLID_LAYERS`, never its owner, optionally past loose items, a grace
  through everything, a lifetime, `keep_speed`) with what a touch does
  (`Touch`: explode through a `grenade::BlastAt`, bounce, a bolt's
  glance or stick, report) and a direct hit's damage through the shared
  armour step (`Direct`, hitboxes or the box's height bands); games steer
  in `FlyerSet::Think` and answer `FlyerHit` in `FlyerSet::Touch`; made
  from a weapon's frame rounds (`chimera:projectile`, `Launcher`, with
  `LaunchUnderWater`) or a game's `LaunchRequest`; drawn on clients from
  `NetItem` (so are a game's own bodies marked `ShownBody`). Also lifted
  from Combat Arms' special weapons and shared with them: `ray_box` (its
  `box_entry`), `ray_sphere` (`sphere_entry`), `facing`. Generic beside it:
  `grenade::SourceRadius` (Source's radius damage as `Blast::radius_rule`),
  `weapon::Inflicted` (where a blast's or projectile's damage came from,
  for HL2:DM's shove), `weapon::HolsterLocked`. Combat Arms' objects keep
  their own 20 Hz frames and units (moving them onto flyers would change
  their timing; `special_weapons.rs` and `net_projectiles.rs` pass
  unchanged); CS:S's grenades unchanged (`radius_rule: None`).

  `games/hl2dm` (H4): `launch` (HL2's blast: linear to the box, the eye
  line, mass/350 shields, force 300 a point to 30000; `Hl2Rounds`, a
  weapon's own count of its second or only ammo); `alt_fire` (the SMG1's
  contact grenade, the AR2's charged ball: constant 1000, 4 s, 1000 crush
  through the suit, the seek after counted world bounces); `rpg` (the
  toss, ignition at tick 20, the dot, beam riding from the owner's centre,
  12.5 % a tick, the reload ended only with attack up, can't be put away);
  `crossbow` (bolts on `chimera:projectile`: hitboxes, ×3 head, glance or
  stick, 1500 under water; the reload strictly after the fire animation:
  168 ticks a held shot; the press zoom to 20); `frag` (pull-back, throw,
  lob, roll as the view model's events time them, an avian 1 kg body
  through players with the 0.2 bounce and 0.1 tap, fuse and blips on the
  7-tick think grid, martyrdom); `slam` (modes by the 42-unit line, the
  trip mine's arming on the think grid, its beam checks, 17 ticks to
  200 in 200 for its placer; satchels, the detonator, harmless pops).
  The damage shove reads `Inflicted` (blasts push from their point, the
  shooter's own too). Found while testing: the guns' and melee's
  `armor_ratio` 0.4 gave the suit every point of a hit while it lasted
  (a pistol body hit on 100 armour cost 8 armour and no health); now 1
  (health 20 %, as suit.md and projectiles.md 11 say).

  Tests: hl2dm `projectiles.rs` (H4 cases 1-5, 15-23, 27, 32, 34-37,
  39-41, 43-44, 47-49, 52-62 and a pistol on the suit), `net_projectiles.rs`
  (each projectile drawn on a client from its `NetItem`, moving, the
  weapon's predicted rounds the server's), unit tests (seek 24, 26;
  guidance 29-31; throws 44-48; the blast's units); chimera_sim
  projectile unit tests; Combat Arms' `special_weapons.rs`,
  `net_projectiles.rs`, `launcher_aim.rs`, `grenades.rs` and CS:S's
  grenade tests pass unchanged. Left: the probe trajectories (P1-P15), the
  frag (2 grenades) in the spawn loadout with the physcannon (M6/M7's
  loadout), the sky test, the water rule of blasts, the spawn blast grace
  (M7), death-notice names (test 64, M9), DSP hearing (63), effects,
  trails, the dot's drawing, pinned ragdolls and dissolves (M10), the
  gravity gun's catches (M6), test 38 (under water: the marker is the
  movement's water level, not checked headless), 42 (props). Merged
  with M7: ammo boxes and crates of the projectiles' types fill the
  weapon's own rounds (`ammo::give`), a pool waiting for its weapon goes
  into them on pickup; `impulse 101` fills them (`weapon::FilledUp`). The projectile weapons
  have no gun class yet, so Ultimate Gun Game's ladder doesn't take them
  (open question 5's choice).

- 2026-11-03: M6 built (branch `hl2dm-m6`). **C6** (Chimera,
  `chimera_world::map::held`): `Held` on a body a player holds (its
  holder, the game's target pose, the controller's arrival time and
  speed and turn limits, the target relative to the holder's view),
  `hold`/`release` keeping its mass (inertia scaled with it), damping and
  gravity (`HeldSaved`), the controller before each physics step (one
  tick to the target, capped), no contacts with the holder or his shadow
  (`MapCollisionHooks`), `PhysicsInfluence` (who last moved a body by
  hand, when). The holder's movement and shadow sweep (`source_engine`)
  and his shots (`chimera_sim::weapon::deliver`) pass what he holds; the
  bridge's impacts skip held bodies and scale players' impacts and
  stress by a game's `source_logic::logic::PlayerImpactScale` (CS:S 1,
  unchanged). `chimera_net`: `NetHold` (prop index, the holder's client
  id, the view offset) written by the server; the holder's client draws
  the prop at its own predicted eye and view (it follows a turn at once),
  the others draw the snapshots; each client marks its copy `Held` by
  the holder's character.

  `games/hl2dm` (H5): `weapon_physcannon` (`weapons::physcannon`, data
  `physcannon.toml`): punt (±8 box then line of 250; 15000 + 600
  min(M, 250) along the view, a downward z × −0.65, the 2000 cap; the
  0-damage hit naming the punter, influence, flag 64 unpins, the 0.5 s
  repunt guard; 1 crush to a breakable; dry fire at players, the world,
  ragdolls), grab and pull (line, ±4 box, 0.97 cone; the hold test: 250
  kg, flags 8/64/512, ground, held, flesh; TooHeavy once a press; pull
  4000 every 7 ticks), the hold (24 + 22.627 + extent, pitch clamp 75,
  the wall rule, the horizontal push-out, view-relative rotation, the
  smoothed error and its dead zone, drops for the ground, a lost or
  pinned body), drop (285, 720), launch (the spline, U(−600, 600) spin,
  the 250 reach), claws (open, 0.5 s close, the punt's and launch's
  delays and suppression), instant draw, the holster lock, `phys_swap`,
  the forced drop at death; frags caught or punted (thrower, 1.5 s fuse,
  fast blips); combine balls caught (no mass limit, the catcher's, no
  life while held, gone 8 s after the catch), launched at 959.2 with 1500
  after the first contact and 4 s of life, dropped at 1500 within ±55
  degrees. Debris props are in the gun's traces (the ragdoll layer).
  `game::physics`: players' impact scale 4, the credit (within 1 s the
  influencer, later the prop: a world kill; the influencer's own prop
  spares him only within the bridge's measurement, 5 ticks), death
  notice `physics`; multiplayer props in modes 1 and 2 solid to players
  (Corrections 1); `+use` at props (`HL2Player.Use`, `OnPlayerUse` with
  256, `HL2Player.UseDeny` for an unusable one). The spawn loadout now
  gives the frag (2 grenades: the grant's 1 and the frag's) and the gun
  last, in hand, the SMG1 the last weapon (weapons.md 3, M5's leftover;
  M7's rules agree). View-model events: punt/launch the secondary
  attack, grab/drop the primary.

  Tests: hl2dm `gravity_gun.rs` (H5 cases 1-8, 10, 11, 13, 14, 16, 17,
  21, 22, 23, 25, 26, 31, 33-36, 41, 42, 44, 47, 48, 54-58, 60, the
  spawn), `net_gravity_gun.rs` (two clients: the holder draws it at its
  view, the other the snapshots; both marked; let go), heavy
  `map_props.rs` (dm_lockdown 200 physics props, 140 holdable;
  dm_overwatch 132, 123; small light ones grabbed, held, launched at
  1500, punted at the cap or the formula); unit tests (cases 1-3, 12,
  18-20, 23, 27, 28; the controller). `weapons.rs`' and `skeleton.rs`'
  spawn tests follow the new loadout.

  Found against the spec: case 4's crate at 260 has its face at 250,
  which the ±8 punt box reaches at 242 (the test uses 270); spec says the
  traces miss. Left: `hl2mp_allow_pickup` 1's carry (section 8, steps
  1-6; off by default), `prop_physics_respawnable` (13), the 10× first
  impact and first-impact breaks after a launch (9), an explosive
  prop's 2 s influencer rule (the logic's last attacker only), several
  bodies per object (mass shares), `preferred_carryangles` and the
  sawblade's spin, the contact factor and contact slide of the
  controller, punting a free ball (no collider), a held ball's blast
  (its end is a sound and removal), held items' 48-unit touch box, the
  hold's idle shake and the claws' pose, the TDM friendly-fire case 43
  (the engine's rule after the credit, not tested), `+use` carry's
  throw; effects and sounds the gun needs from M10: the beams from the
  claws and the core glow by effect state, the `PhyscannonImpact` launch
  effect (beam to the point, sparks), the white fire fade, the
  client-local `Weapon_PhysCannon.HoldSound` loop (pitch to 100, volume
  0.8 over 0.5 s) and the held ball's loop pitch ramp. The probe checks
  (P1-P17) weren't run (the probe server crashed on a bot's death in M7;
  P2-P12 need a bot holding and punting).

- **M11 assets (branch `hl2dm-m11`, 2026-11-03).** C7: `chimera_pack`'s
  game list already had HL2:DM (app 320, its search path); this adds its
  pieces' sets to `record_pieces`: `weapon/<id>` for the 12 weapons (view
  and world models, the projectile models, materials, the waves of the
  sound entries and melee sounds) and `players` (the 19 models). Build
  24600434 published into the dev server's store and signed: 30,792
  files, 30,390 records, 1.9 GB stored (only 863 records new: `hl2/` is
  CS:S's), sets `weapon/*` (12, 13 MB), `players` (166 records, 18 MB),
  `base` (1,069 records, 30 MB: what every one of the eight stock maps
  reads, less the weapon and player sets), `map/<name>` for the eight
  stock maps (the app on the map, 90-153 MB each) and `heavy-tests`
  (6,598 records, 366 MB, the hl2dm crate's heavy tier). The hl2dm app
  has no menu yet, so `base` is the intersection of the eight map logs.
  `heavy/served.rs` (hl2dm): dm_lockdown converts from a scratch store
  alone and from the service on 127.0.0.1 with no install, the same map.
  The hl2dm heavy tier from a scratch server alone (no install, Steam
  folder, keys or content cache; empty cache): 27 of 29 pass, no asset
  misses; the two failures (`heavy::scripts`: weapon data against the
  scripts) fail the same with the install. UGG's pool already takes
  HL2:DM's maps (`map_patterns` `dm_*`, the installed maps from the
  mount, which the store stands in for); halls3 isn't a `dm_*` name and
  stays out. Left: the party and UGG heavy tests' reads of HL2:DM from
  the store weren't run.
- 2026-11-03: The probe made reliable and run (branch `hl2dm-probe`,
  dedicated server build 24600426, port 27040). **The crash**: an
  `hl2mp_bot_add` bot's NextBot behaviour segfaults the server in its
  action update as soon as the bot dies (`sm_slay` or anything else:
  the core dump's stack is the bot's behaviour update, not SourceMod).
  `nb_player_stop 1` stops the behaviour and keeps the bot running the
  plugin's commands: it dies and respawns safely, again and again
  (`probe.setup` sets it with `mp_forcerespawn 1`, which the server
  resets at the map load; the README has it). The plugin (version 2)
  now also logs the active weapon, tracked entities, damage, traced hits
  with hitgroups, bullets' temp entities (seed, spread), deaths and
  spawns, the server tick and the bot's tick base, and gives, hurts,
  kills, places, spawns props and reads or sets any property;
  `tools/hl2dm_probe/cases.py` runs the cases below.

  **The bot's clock**: its tick base runs 4 ticks ahead of the server's
  (`sv_clockcorrection_msecs` 60). Timers its commands start run on that
  clock, so counted in server ticks they land 4-5 ticks after the specs'
  numbers (frag explosion 173 not 169, its blips 0, 74-75, 96, 117, 137-138,
  159; RPG ignition 23-24 not 20; a kit back 2004-2005 not 2000; a
  weapon's copy 1337-1338 not 1333); a death from a console kill (outside a
  command) respawns 5 ticks sooner (329). The specs' numbers hold in the
  command's own clock (an in-tick death: 334 exactly). Chimera runs one
  clock, so nothing changes; whether a human client's tick base sits
  ahead the same way wasn't measured.

  | Case | Real server | Spec | Ours |
  |---|---|---|---|
  | rules P2-P4 respawn | force 334 after an in-tick death; held button never; press at +200: 201, +199: nothing (then 334); release then press +201: 202; observer mode 0 throughout | same | same (tests) |
  | suit tests 1, 2, run-outs | 94/76, 94/52; armour 10: 80/0, blast 75/0; armour 5, 7 dmg: 98/0 | same | same |
  | suit test 6 (four 8-bullets on 100) | armour 94, 88, 82, 76 | 93, 86, 79, 72 | **was the spec's; fixed** |
  | armour paid per bullet D | trunc(0.8 D), at least 1 (24: 19, 63: 50); blast trunc(1.6 D) | ⌈0.8 D⌉ | **fixed** (`absorb_quanta`) |
  | suit P2 remainder | 0.8 kept across death; next 1-bullet: 99 | same | same |
  | suit P6 drowning | aux −0.1005 a tick, 0 at 14.87-14.93 s, hits 10 at 21.9 s then +1.005 s, `pl_drown*`; recovery +10 at once, then 2.01 s | same | same (tests) |
  | suit P10 chargers | suit: 1 per 7 ticks, pool **30**; health: 1 per 7, pool 50; refill 30 s from emptying, held use doesn't push it | pool 75 (`sk_suitcharger`, the user's open question) | 75, kept |
  | suit P11 megacharger (8192) | +2 armour, +5 health a charge, pool 200, armour to 200 | same | same |
  | suit P12, weapons P3, projectiles P13, gravity P1, maps P10 cvars | all as listed but `sk_suitcharger` 30; `physcannon_*` flags only "sv" (not cheat) | cheat-flagged | – |
  | movement rows 59-61 (water) | sink −7.2, −13.968 … −48; jump 116.8 then 104.2; forward 22.8, 44.232, 64.378 | same | same (tests) |
  | weapons P1, P2 spread | pistol cone 0.00872 (first shot), +0.00538 a fast click to 0.05234; .357 0 | same (penalty 0.185, 0.37 per click) | same |
  | weapons P5 spawn ammo | pistol 150, SMG1 45, .357 6, buckshot 6, grenades 2 | 2 (open question 2) | 2 |
  | weapons P6, P7 | pistol 33/34, .357 50, SMG1 5 (first sometimes 6), AR2 7-7-6, shotgun 59, double 70; clicks 8 | same | same (tests) |
  | weapons P8, P9 | reloads 96, 245, 100, 105, shotgun first shell 34; switch 54 (pistol), 56 (SMG1) | same | same |
  | weapons P10, P13, P17 | guns dropped with their clip, crowbar and gravity gun not, gone after 31 s; SMG1 back after 3.1 s: 45, after 2.85 s: 38; `drop` does nothing | same | same |
  | weapons P16, projectiles P7 | bullets push: pistol 40 (5 per damage), .357 375, shotgun 45 a pellet, crowbar ≈123 | P16: unchanged; P7: as measured | push (M4's shove) |
  | projectiles P1, P2 frag | start eye + 18 forward, 8 right; ≈1160 forward, 120 up, and ≈2.2° to the right (lateral 3.8 % of the speed, every throw); lob ≈340-380, 43 up | (1200, 0, 120), (350, 0, 50) | the spec's |
  | projectiles P3, P4, P9 | contact grenade 1000, gravity 400; missile (300, 0, 128) with gravity 600 then 1500; bolt 3500, gravity 30 | same | same |
  | projectiles P9 crossbow held | a bolt every **130** ticks; the reload's load sound 69 ticks after the shot, the bolt in at 130 (the reload starts about 7 ticks after the shot) | 168 (reload from tick 45, after a 0.6667 s fire animation) | **was 168; fixed** (fire animation 0.1 s in crossbow.toml) |
  | weapons test 14 SMG1 clip | 45 shots, last 220 ticks after the first | 220 ±1 | same |
  | gravity P2 level punts | 10/30/100/250 kg: 1842/1084/748/660 a tick after (2000/1100/750/660 less a tick of drag) | same | same |
  | gravity P3 punt 45° down, 30 kg | 827 forward, 577 up (the crate on the floor) | ≈ 506 up | – |
  | gravity P4, P5 held, launched | held 66.5 ahead at eye height; launched 1469 (30 kg), 1324 (250 kg) | ≈ 66.6; 1500, 1327 | same |

  Fixed: HL2:DM's armour pays its cost truncated to whole points, at
  least one (`hits::absorb_quanta` with an armour quantum; CS:S and
  Combat Arms unchanged), suit tests 6, 8, 9 and the hits unit test to
  the measured values; the crossbow's fire animation 0.1 s (data), so a
  held crossbow fires every 130 ticks (projectiles test 36). Not measured: head hits (the stopped bot doesn't
  animate: level shots at head height pass over it), crowbar reach (P14),
  ladder brushes (movement P11), P15, the client cases (P16), H1's
  back-hop, weapons P4, P11 (copies: maps P2 above), P12, projectiles
  P5-P8, P9's water speed and damage, P10-P12, P14-P15, gravity P6-P17, rules P5-P14, maps P3-P9, M8's
  P6-P11, M9's P1-P9, M10's P7-P11.

- 2026-11-03: projectile effects built (branch `hl2dm-fx`; M5/M10's
  left-overs). **Task first:** the heavy script checks were the test's
  fault, not the data's (`scripts::differences` now reads a rocket's,
  grenade's and S.L.A.M.'s ammo, clip and default clip from their
  projectile hook where the script names them; the S.L.A.M.'s view model
  has no `ACT_VM_DRAW`, its draws are per mode: the test checks
  `ACT_SLAM_THROW_DRAW` = 1.0 s and the trip mine's 0.8333 s where found).
  **Lift:** CS:S's explosion particles (`explosion`, `blast_direction`,
  `FxMaterials`) moved to `source_engine::effects::explosions` (CS:S
  re-exports them). **HL2:DM** (`game::projectile_fx`, `game::dissolve`):
  explosions of the frag, SMG1 grenade, RPG and S.L.A.M. on `Detonated`
  (the core and flash, `BaseExplosionEffect.Sound` on each machine; the
  RPG's and S.L.A.M.'s by the explosion entity's rule, drawn 24 off the
  surface found 8 above to 40 below), no more server `BaseGrenade.Explode`
  for the rocket, contact grenade and S.L.A.M. (the frag's stays); the
  rocket's 8 flares and smoke puffs per 4 units; the ball's animated body,
  flicker and blur, its `cball_bounce` quads and sparks, its two
  `lgtning` rings; the bolt's 20-halo blur and its stuck glow and sparks;
  the contact grenade's smoke trail; the frag's and satchel's red glow and
  the frag's trail; the RPG's laser dot (the owner's from the current
  view, others' at the server's dot). The ball's kill: the victim is
  `Dissolving` (no voice), its ragdoll takes `Dissolve`: black over 1 s,
  fade 1.9 to 2.0 s, floating (+0.02 g, 50 units/s cap), sparks and
  glows around each body part, removed 134 ticks on (`RagdollSet`,
  `despawn_ragdoll` made public in `map::ragdoll`). A remote client now
  draws any blast `Detonated` of a game's own projectile (it dropped them
  without a throwable template). Tests: `projectile_fx.rs` (H9 39, 41,
  42, 45, 47 and the ball, bolt, smoke, frag looks), `ragdolls.rs`
  (H9 9, 11; unit tests for 10, 12). Left: the RPG's beam and glow on the
  view model (needs attachments from the render layer), the held ball's
  model, the shot satchel's pop look (H9 40: it has no `Detonated`), the
  dissolve of the victim's dropped weapon, the ball's bounce, end and the
  stuck bolt's glow on a dedicated server's clients (they read the host's
  flyer messages), the RPG's scorch, `refcmp` captures.
  **Gravity gun** (`game::physcannon_fx`, after merging M6): a punt's or
  launch's spark burst (16-32 trails back toward the gun) and electric
  sparks, the punter's `orangelight1` beam (12 to 4 units, 0.1 s) and the
  white fire fade (245, 245, 255, 32 over 0.1 s, `ScreenFades`); the
  client-side `HoldSound` loop (0 to 0.8 and 50 to 100 over 0.5 s, back
  over 1 s) and a held ball's loop (pitch to 150 in the last second).
  Tests in `gravity_gun.rs` (a punt's burst, beam and fade; the loop's
  ramps) and unit tests. Left: the claws' core glow, glows, end caps and
  beams (persistent sprites on a model's attachments need the render
  layer), the punt's end at the object's centre.

- 2026-11-03: Probe round 2 (branch `hl2dm-probe2`, build 24600426).
  The plugin's tracked-entity rows end with the angles, `probe_prop`
  takes spawnflags (72 hangs a prop until the gun punts it); `cases.py`
  gains `heads`, `frag_drag` and hanging-crate punts; README notes how to
  aim at a stopped bot's head and fit a spinning grenade's velocity.

  | Case | Real server | Spec | Ours |
  |---|---|---|---|
  | projectiles P2 frag throw | (1200, 0, 120) + the thrower's velocity, lob (350, 0, 50); round 1's 2.2° right was the origin circling the mass centre (3-5 units) | same, no drag | **air drag added**: (0.07 + 0.00009 \|v\|) v per second (frag.toml) |
  | gravity P3 punt 45° down | hanging crate: (772, 488) a tick after = spec less drag and gravity; on the floor (827, 577) from the spun edge digging in | ≈ 506 up | hanging: same; floor (771, 813): **open** (solver contact) |
  | gravity P3 spin | about the box trace's end (2964°/s at 21.8°; inertia 0.714 of a solid box) | "at the trace end" | **was the contact point; fixed**; a prop set moving by the punt was pushed as 1 kg (cap): **fixed** |
  | weapons P15 head | pistol 24, .357 225, crowbar 25 (generic) | same | same |
  | bolt head (test 39) | 100, generic hitgroup on head, chest, legs | 300 | **was 300; fixed** (crossbow.toml `head = 1`) |
  | weapons P16 / projectiles P7 push | 5 a point (pistol 40, .357 375, pellet 45, crowbar 125 less a tick); blast 5 a point from the blast point lowered 10 | same | same |
  | projectiles P6 frag blast | 150 − 0.6 d within the frag's position (125.6/126.4, 74.3/75.1) | same | same |
  | projectiles OQ1 / P12 ceiling | rocket and contact grenade against a ceiling hurt by distance | open | same (new test) |
  | projectiles P8 ball | kills through 200 armour; owner untouched; ends 272 server ticks after creation (4.005 s + the command clock), removed 33 later | same | same |
  | projectiles P10 mine | created 16 after the press, armed 143-144 (139 + clock), placer in the beam trips at the first check (+7), blast 17 later | same | same |
  | projectiles P11, P14 | shot satchel harmless; pin pulled on the last grenade at death: a live grenade | same | same |
  | movement back-hop, P11 | no gain (376 kept); ladder-contents brushes block, never climbed | same | same |
  | gravity P6, P10/P11, P16, P17 | drop cap 283/280; punt into a bot 50 + 10, the punter credited; a crate dropped 300 on a head 10 + 20; TooHeavy once a press; draw and punt on one command: a punt | 285; 500 for the drop (test 39) | 285; 60 for the punt; **20 for the drop: open** |
  | suit P7, P9 | flashlight on keeps aux 100; a sprinter killed respawns at aux 97.5 (detach paid on the first tick) | same (Quirks) | flashlight same; **sprint across death was cleared; fixed** |
  | weapons P4, P12, P14 | crate takes its prop data's bullet scale (8 → 6); weaponstay: taken, a copy stays, owned refused; crowbar hits a box 26 aside, not 36 | 8 | prop data; same; same |
  | rules P11 | map change 71 s after the load with `mp_timelimit 1` | ≈ 70 | – |
  | maps P3 | items settle from their placed origins | same | – |

  Fixed: the frag's air drag (`frag::air_drag`, test `a_frag_slows_in_the_air`,
  tests 44, 47 at a tick of drag); the gravity gun's point impulse at the
  box trace's end and a just-enabled prop's own mass and inertia
  (`a_hanging_crate_punted_from_above`; the gun tests' map now has HL2:DM's
  physics gravity); the bolt's 100 anywhere (test 39); a sprint kept
  across death and a dead player's suit left alone (`a_sprinter_killed_pays_the_detach_at_the_respawn`);
  a blast point inside the world still hurts (test). Not measurable here:
  TDM cases (rules P5-P10, P13-P14: `hl2mp_bot_add` bots are kicked at
  once with `mp_teamplay 1`), movement P15 and projectiles P5 (two
  driven players), P16 and HUD/effects (a client). Left: a punted crate
  on the floor (avian's edge contact), the crate-on-a-head damage (ours
  20, real 30), bolt speed under water (P9: dm_underpass's pool spots
  put walls in the way), the ladder brushes against bullets, gravity
  P7-P9, P12-P15, suit P8, the breather half of P9, P13, maps P4-P9.

- 2026-11-03: view models and the listen server (branch `hl2dm-visual`).
  **The RPG drawn black, the frag not in hand** (dm_lockdown): not a
  material fault. `heavy/view_models.rs` loads all 12 view models as a
  map's kit does: every mesh resolves to its base texture, the hands
  included. Both are the lighting at the spawn corridor (where they were
  seen): dm_lockdown has no `light`s, only 57 `light_spot`s, and none
  reaches that corridor unoccluded, so a view model there gets the
  ambient cube alone (0.01-0.1 per side; view_models.md 9 as specced).
  The RPG's `rocket launcher sheet` is the darkest weapon texture (mean
  sRGB about 17, vtex reflectivity 0.005; the AR2's 0.16, the SMG1's
  0.01): there it is black, and the AR2 is near black too. In the lit
  wash room (`setpos -3619 2600 0`) both the RPG and the frag (in the
  fist) draw textured. Its material is `$phong` with a flat bump map
  (exponent 25, boost 2, fresnel ranges 0.2/0.5/1.5): the Phong ("skin")
  shader would add highlights, but shaders.md open question 14 leaves it
  unspecified; a spec of it (HL2's view models, the citizens) is the
  next step for the look.
  **The listen server's host frozen after a client joined**: not
  reproduced. The hl2dm app hosting dm_lockdown (`-port 27100`) with one,
  two and then a third client (one leaving, another joining), under
  `host_timescale 0.2`, firing, holding props: the host stayed at 60 fps
  (`Time` delta 16-17 ms over the remote protocol), moved, and each of
  its screenshots changed, also with the client windows stacked over it.
  The earlier run's "frozen" frames differ by 0.1-0.3 % of their pixels:
  frames that changed while what was asked of the game didn't happen (a
  punt and a ball that weren't launched in them). No change made.
  **Glow sprites drawn blended**: `Sprite`-shader materials take their
  blend from the render mode, and the stuck bolt's glow
  (`light_glow02_noz`, glow mode) and the punt beam (`orangelight1`)
  loaded as their VMT says, blended: a black square around the bolt.
  `effects::ADDITIVE_SPRITES` loads them additive; `heavy/
  effect_materials.rs` checks it. Seen while taking promo shots: the stuck
  bolt stands vertical in the wall whatever its flight (not looked into).
- 2026-11-03: **Source's Phong model shading** (branch `phong`,
  specs/cs_source/shaders_phong.md, which answered shaders.md open
  question 14). VertexLitGeneric `$phong` materials with a `$bumpmap`,
  lightwarp or `$basemapalphaphongmask` are read into `map::MapPhong`
  (`source_engine::content::material`, routed by `map::phong::route`;
  the frag grenade's body, `$phong` without a bump map, stays on the plain
  model shader) and drawn by prop.wesl's Phong path: forced half-Lambert
  diffuse per pixel with the normal-mapped normal (screen-space tangent
  frame) from the probe's ambient cube and 4 lights (and the scene's point
  lights), the reflection-vector Phong lobe with the normal alpha mask,
  the three-point fresnel ramp, `$phongboost`, the exponent rule
  (`$phongexponent` over the map, 150 with neither) and tint (raw
  `$phongtint` or the albedo tint), the rim light (max with the specular,
  plus the ambient cube along the view on upward-facing pixels), the
  envmap's own mask rules, lightwarp and phong warp, the cheap variant.
  The spec's test cases run on a CPU mirror (`map::phong`;
  chimera/present/tests/it/phong_spec.rs); `heavy/view_models.rs` checks
  the stock routing (15 view model materials drawn by the default skins
  plus 2 other skins' = the spec's 17). Missing parameters take the spec's
  stated defaults (its open question 1). `CHIMERA_PHONG=0` draws them
  plain for comparisons. In dm_lockdown's wash room (`setpos -3619 2600
  0`) the SMG1's barrel and the RPG's tube and rings take a soft highlight
  band and the normal maps' relief; the Combine soldier
  (`cl_playermodel models/combine_soldier.mdl`, `thirdperson`) changes
  little under the room's light; CS:S's de_dust2 view model draws the same
  with and without it (no CS:S material is phong). Left: a refcmp against
  HL2:DM (open question 9).
- 2026-11-04: **The crossbow bolt gone at the wash room's half wall**
  (branch `hl2dm-bolt2`; the vertical bolt was fixed before: `y_forward`).
  The bolt was drawn on the listen host (its flyer is the server's own
  `ShownItem`), in flight and stuck in the world; the half wall
  (`setpos -3619 2600 0`) is a func_brush, a kinematic body, and a bolt
  sticks only in static bodies, so it was removed on touch as if the
  brush moved. Now a kinematic body at rest (its `MovingSolid` and
  velocities zero) is a non-moving solid (spec 7.4): the bolt sticks
  there; a moving one still takes it away (projectiles test
  `a_bolt_sticks_in_a_brush_entity_at_rest_and_not_in_a_moving_one`).
  Side views on dm_lockdown: the stuck bolt lies along its flight, its
  pointed tip at the wall and its tail out.
- 2026-11-04: **The gravity gun's claws, the RPG's scorch, the satchel's
  pop** (branch `hl2dm-fx2`; H9 11, 40-42, 48, 49). **Chimera**
  (`map::attached`): sprites and beams that stay on a character's weapon
  attachments (`AttachedFx`, by index or name), drawn each frame as
  one-frame particles on the first-person view model (posed, offset and
  mirrored as drawn; on the view model's layer: `ParticleGroup::
  view_model`, a third particle mesh layer in `render::particles`) or
  at the held world model's attachments (`MapHeldModel::attachments`,
  read by `source_engine`'s `load_held`, spawned as `HeldAttachment`s);
  a beam to a world point is re-projected and drawn in the world pass.
  Beams follow visual_entities.md 1 (fractal noise, strip twice the
  width, texture per 100 units) with the caller's segment count.
  **HL2:DM** (`physcannon_fx`): the effect state (none, ready, holding
  while holding or searching) per gun, the server's replicated to
  clients (`Hl2GunEffect`, net part `hl2dm_physcannon_fx`); the core
  moving to the state's width and alpha, the six glows, the end caps and
  the noisy beams, the view model's set in first person (attachment 1 is
  `muzzle` there, `core` on `w_physics`) and the world model's (`fork3`
  too) for everyone else; a punt's or launch's blast sprite; the punt
  beam now starts at the view model's attachment 1, others' sparks at the
  world gun's. **Scorch** (`projectile_fx::scorches`, the server's): an
  explosion by the entity rule (RPG, satchel, trip mine) puts `Scorch`
  where its line from 8 above to 40 below that hits; none in mid-air or
  on a wall well above the floor. **Pop** (`slam::SatchelPop`, sent to
  clients by net part `hl2dm_projectile_fx`): a shot satchel's scale-0
  explosion, the flash overlay and the debris (`explosions::pop`, the
  explosion's sparks and flecks split out as `debris`) and
  `BaseExplosionEffect.Sound` on every machine, `BaseGrenade.Explode` as
  a `GameSound` (clients hear it now), a `Scorch` under it. Tests:
  `gravity_gun.rs` (view model, world model, blast), `net_gravity_gun.rs`
  (B sees A's beams), `projectile_fx.rs` (41/42 scorch, 40 pop),
  `projectiles.rs` (60: pop, scorch, sound), heavy `view_models.rs` (the
  models' fork attachments), `effect_materials.rs`; unit tests for the
  core's and blast's ramps (48, 49) and the pieces by state. Left: the
  "closed" state (nothing in the spec enters it), the punt's end at the
  object's centre, the SMG1 grenade's scorch along its flight (4.3),
  scorches on props and brush entities, the RPG's beam and glow on the
  view model (now buildable on `map::attached`). Seen in the app
  (dm_lockdown's wash room): idle, small orange glows on the claws; held
  (a barrel, a crate), a bright core over the muzzle and thin crackling
  beams from the fork tips, partly hidden by the claws (the sprites are
  depth tested against the view model; the game draws them after it);
  an RPG on the floor leaves the scorch, faintly fringed light at its
  edge. The pop wasn't captured.
- 2026-11-04: **Others' tracers and impact sounds, shells** (branch
  `hl2dm-netfx`; H9 1-3, 5). Chimera: a gun's `[hitscan] tracer_every`
  (and `redrawn_through_characters`) is data (`Hitscan`); a network
  server counts every bullet it fires and flags each round it sends
  (`FireBullets::flags`, `weapon::remote::shot_flags`: tracer picked,
  anything hit); a client traces the round again as flagged (a
  `WeaponEventKind::Redrawn` before its `Shot` events, through every
  player for HL2:DM's guns, no hits told when the server's hit nothing).
  CS:S and Combat Arms are unchanged (no `tracer_every`: their own
  counts). HL2:DM: its tracer cadence comes from the guns' data; a
  flagged remote round draws a streak on every bullet (a shotgun shot
  always 7), never whizzes; the impact sounds of remote rounds play
  (they did before too: only untested) and land on what is behind a
  player. Shells: HL2's numbered brass event (6001, option the type) is
  read (`game::shells`): the SMG1's type 0 (`shell.mdl`) on every fire
  sequence, the shotgun's type 2 (`shotgun_shell.mdl`) at 0.3125 of the
  pump, with HL2's motion (`MapShellPhysics` gained world-axis speed,
  roll spin, a random life, a sure bounce sound at 200 units/s and no
  shooter's velocity; CS:S's values unchanged). Other players eject
  nothing (spec 5: parity). Found on the way: HL2:DM's view models name
  their muzzle event (`AE_MUZZLEFLASH`, number 0), which the flash
  reader (numbers 5001...) never matched: no first-person flash sprite
  was drawn; now read (heavy test). The "front-facing Combine shot" (the
  AR2's flat flash facing the hit surface, `gun_impacts`) is drawn for
  remote rounds by the same path. Tests: `tests/it/net_effects.rs` (the
  SMG1's flagged rounds, the shotgun's 7 tracers, the AR2's streak, the
  re-trace through players; cases 18, 20, 26), `effects.rs`
  (`each_guns_tracer_cadence_is_its_data`), heavy `view_models.rs`
  (each weapon's shell type and flash events from the install, both
  shell models load), unit tests (brass event, bounce sound case 34).
  Captures on dm_lockdown: the SMG1's shells fly out right of the gun
  and are drawn textured; a remote player's tracer seen from a second
  client. Left: the glass and water exceptions of the server's count,
  the remote tracer's start at the world model's muzzle (the eye when no
  flash was drawn there), the shell fade length (open question 4), the
  pistol's and .357's smoke, refcmp.
- 2026-11-04: **The claws' sprites over the view model** (branch
  `hl2dm-claws`; H9 11). `render::particles`: the view-model layer's
  particle meshes (`map::attached`'s first-person sprites and beams) are
  drawn without the depth test (`ParticleDrawMaterial::over_model`,
  compare always, no depth write), so the claws no longer cut the core
  and the glows; the world's and the sky's stay depth tested (others'
  guns). H9's units say the gravity gun's sprites ("glow quads") are full
  widths, as read; the core's alpha (128 ready, 255 holding) and the
  additive blend (colour × alpha) were already the spec's. Tests:
  `chimera_present`'s `particle_depth.rs` (the depth state, and only the
  view-model layer's mesh over the model). Seen in the app: idle, an
  orange glow over the muzzle in front of the claws; holding a crate, a
  big bright orange-white core over the muzzle with the beams over the
  claws; the punt's blast over the gun.
- 2026-11-04: **The RPG's laser, the pistol's and .357's smoke, the SMG1
  grenade's scorch** (branch `hl2dm-rpglaser`; H9 9, 47, Constants;
  H4 4.3, 6.1). The rocket already followed the dot (6.4: guidance and
  beam riding were there). **The laser** (`game::rpg_fx`): on while the
  RPG in hand has its dot, `Hl2LaserOn` on the player (the server's,
  replicated: net part `hl2dm_rpg_fx`), with `Weapon_RPG.LaserOn`/`Off`
  as `GameSound`s; the dot is now made only after the 1.1 s draw (the
  deploy's next attack). HL2:DM has no toggle (6.1: attack2 does nothing),
  so none was built. Every machine draws each lit laser's dot at the end
  of the dot's own line (`rpg::DotTracer`, shared with the server: through
  windows, past its owner): the local player's from its current view,
  others' from their drawn eye (`RenderedView`), so a network client now
  sees others' dots (before: only the host). The view model's beam
  (`effects/laser1_noz`, attachment 2 `laser` to 3 `laser_end`, 2 wide,
  red, brightness 128-200 a frame, 4 segments) and its `redglow1` glow
  (8 ± 2) through `map::attached`; others' on the world model's 4 to 5
  (`effects/laser1`, 1 wide). The install's `w_rocket_launcher.mdl`
  has 5 attachments (`muzzle`, `missile`, `exhaust`, `guide`,
  `guide_target`), so 4 and 5 exist: the spec's open question 6 (3
  attachments) looks answered by the model; not checked against the real
  game. The gravity gun's claws and the laser both write `AttachedFx`:
  `effects::clear_attached` empties it each frame, the writers add in
  `AttachedPieces`. **Smoke** (`flashes::muzzle_smoke`): a first-person
  `PISTOL` flash puts a `particle_smokegrenade` puff 8 units ahead of the
  muzzle 3 times in 4, a `357` one always (the table's numbers).
  **SMG1 grenade scorch** (`projectile_fx::scorches`): a 60-unit line
  from where it went off along the velocity it last flew with: `Scorch`
  on the world, `SmallScorch` on props and brush entities, none when it
  had stopped. Tests: `tests/it/rpg_laser.rs` (on after the draw and
  its sounds, off when put away; the view model's beam and glow; the
  world model's on attachments 4-5 and none without; the smoke, 3 in 4;
  the flight scorch; NetSim: B sees A's laser state and draws A's dot
  where the server's is, A its own), unit tests (dot size, beam pieces,
  smoke puffs), heavy `view_models.rs` (the RPG's attachments),
  `effect_materials.rs` (laser materials load, additive). Seen in the app
  (dm_lockdown's wash room): the view-model laser is a thin red line from
  the tube's side with a red glow at its start; the dot is a small red
  point, plain on the darker tiles and barely visible on the bright white
  tiles; the .357's puff is a clear grey cloud ahead of the muzzle, the
  pistol's faint (its material's own alpha 0.27). The satchel's pop was
  caught this time (a burst of 14 screenshots over the console): spark
  streaks and flecks over the new scorch, then the debris falling. Left:
  the beam's fade length, the owner's laser sounds played at once
  (predicted), a dormant owner's networked dot point, refcmp.

- 2026-11-04: **Probe round 3** (branch `hl2dm-probe3`, build 24600426;
  plugin version 3: `cmd` rows run a client's or the server's command on
  an input row, `probe_score`, deaths' attackers and `player_team` in the
  "V" lines, damage hooked once per client across map changes).
  `cases.py tdm tdm_spawns crate_hits floor_punts use_carry`; the numbers
  in specs/hl2dm/rules.md Open questions 14 and gravity_gun.md 12.

  | Case | Real server | Ours before | Ours now |
  |---|---|---|---|
  | TDM bots | kept with `hl2mp_bot_join_after_player 0` (the bot manager's kick, not the rules'); `hl2mp_bot_add combine`/`rebels` | – | – |
  | rules P5, tests 10-13 | alive change: death, the new team −1; dead changes free (1 s, over 5 s); own team nothing; 0.15 s repeat ignored; 3.9 s after a `kill`: alive; `spectate`: frag back, old team −1 | same | same; pinned (`tdm_team_changes_as_the_probe_measured`) |
  | friendly fire, P7 | ff 0: a teammate's bullet traces, deals nothing; ff 1: 8; team kill −1 frag and its team +1; enemy kill +1, +1; a world kill moves neither team | same | same; pinned (`tdm_friendly_fire_and_kills_as_the_probe_measured`) |
  | TDM spawns (dm_runoff) | Combine only on `info_player_combine` (48), Rebels on `info_player_rebel` (36) | same (tests 26-29) | same |
  | gravity P10 crate on a head | 30 kg at 284/389/524/663 in/s: 5+10, 5+10, 10+20, 10+50; 10 kg at 420: 10; 100 kg at 572: 20+100, at 780: 100+500; 250 kg at 582: 100+500 | 5+5, 5+10, **10+10**, **10+20**; **5**; 20+100, 100+500; 100+500 (CS:S's 30 kg contact shares, 33.5 %/48 %) | **all as measured** (`source_logic` bridge's `PLAYER_CONTACT_SHARES` by mass: 10, 30, 100, 500 kg; CS:S's tests unchanged); pinned (`a_crate_falling_on_a_head_hurts_as_the_probe_measured`). Ours lands heavy crates a second time 0.7 s on (5+10); the real crate stays on the head |
  | gravity crates punted into a bot | 10 kg 1500: 20+10; 30 kg 1000: 20+10+5; 100 kg 730: 50+10+10; 250 kg 650: 100+5+10, more after | not compared (round 2's 1050 punt: 50+10 both) | – (Open) |
  | gravity P3 on the floor | 45° down at 40: (773, 898) then (695, 20, 871), 1114 in/s at 51.4°, 1490°/s | (761, 792), 1098 at 46°, 1670°/s | unchanged (the solver's edge contact); pinned within 3 % and 6° (`a_crate_on_the_floor_punted_from_above`) |
  | gravity 8, P9 `hl2mp_allow_pickup` | 0: click, stays; 40 kg: deny; 1: carried 66.5 ahead at eye height, (63.9, 37.0) up 30, level crate, turns with yaw; throw 199 (20 kg), 276 (10 kg); `+use`/secondary drop from rest; sprint 190; weapon kept, can't fire, redeployed at the throw | **no carry** | **built** (`game::carry`, data/use_carry.toml, `hl2dm:hl2mp_allow_pickup`, `hl2dm:player_throwforce`); pinned (`use_carries_with_allow_pickup_as_the_probe_measured`) |

  Guesses where the spec is silent: the throw's random torque is read as
  degrees per second (its units aren't given); the carry's error grace
  (1 s) is the gun's. Left: punts into players (the multi-contact pattern
  of a horizontal hit, Open), heavy crates' second landing, the floor
  punt's edge contact, the carry's drop-while-touching rule, the hidden
  view model and HUD while carrying.

- 2026-11-04: **`+use` carry, the three shortcuts** (branch `hl2dm-carry2`).
  A drop while the crate touches anything but a sensor zeroes its velocity
  and spin (gravity_gun.md 8 step 4; otherwise cut to 285 and 720). The
  weapon is holstered in the spec's words, so the view model
  (`ViewModelHidden` on the player, new in `chimera_sim::weapon`, read by the
  view animator) and the weapon-selection HUD (`hide_selection`) are hidden
  while carrying; the spec says nothing more about what shows. A network
  client predicts the carry: `Carried` is a predicted part (`predicted_net`),
  `carry_weapon` (`Predict::Weapons`, after the weapon frame, server and
  client alike) holds the weapon off and deploys it again at a throw or a
  drop by secondary or a fresh `+use`, and `select_speed` reads `Carried`
  (sprint stays 190). Pinned by `a_client_predicts_the_carry_as_the_server_runs_it`
  (`net_gravity_gun.rs`: SMG1 in hand, carry, sprint, throw, fire held; no
  prediction error, every predicted tick the server's, the deploy 0.8333 s
  on) and `a_carried_crate_dropped_while_touching_stops_dead`. Left: a
  client doesn't predict the pick-up itself (the server picks the prop).

- 2026-11-04: **Hitscan, drop and projectile details** (branch
  `hl2dm-hitscan`; weapons.md 1, 3, 5, 7.2, 9, 11, 12; projectiles.md 3,
  4.2, 5.4, 8.5, 9.2). Chimera: `Hitscan::odd_box` (a call's odd bullets,
  pellets or a tick's catch-up rounds, swept as ±3 boxes; hitboxes grown
  by it), `Swing::refine` (a box hit re-traced to twice its distance, then
  its 8 corners), a character's swing `Damage::force`, `lagcomp::
  RewindChoice` from the data's `[lag_compensation]` with `RewindSpeed`
  and `Inventory::last_shot` (CS:S has none: its spec 4.6 rewinds every
  player, unchanged), others drawing an `AltShot` round with its pellets,
  `drop::DeathPush`, `Unselectable`, `ArmourShare`. HL2:DM: the data sets
  them; `cl_defaultweapon` (userinfo) picks the spawn's weapon; a weapon
  with no ammo refuses `lastinv` and `phys_swap` (`lastinv` otherwise
  matched the spec); the dead's weapon flies by the killing hit / 85 kg,
  at most 400 in/s, spinning 200 deg/s an axis (unit not in the spec);
  the .357 nudges the local player's view by a whole -1..1 degree; the
  shove's size reads the damage before the suit's share; the contact
  grenade explodes when stopped on a 3-tick check; the ball seeks with a
  swept box, skipping TDM teammates and the intermission; the frag spins
  about its own axes (the roll turned (0, yaw, -90): a forward roll, it
  rolled backwards before); a TDM leaver's satchels go off (DM: they
  stay, as before). The SMG1 grenade has no team test in the spec, so it
  has none. Tests: `tests/it/weapon_details.rs` (boxes for the shotgun's
  odd pellets and a catch-up bullet, the refined swing, the death throw,
  `cl_defaultweapon` test 3, refused switches, the nudge, the shove on
  armour, the stopped grenade, the leaver's satchels), `net_effects.rs`
  (a remote double shot's 12 tracers; whom a shot rewinds, and no rewind
  for attack2 alone), unit tests (`lagcomp`, the ball's swept box, the
  frag's spins). Left: the death force is the killing hit's, not the
  tick's sum; turned hitboxes meet a box as grown on their own axes;
  other clients draw catch-up rounds as lines; the throw and lob are made
  unturned (only the roll's orientation is specced).
- 2026-11-04: **Probe round 4** (branch `hl2dm-probe4`, build 24600426;
  plugin version 4: `probe_clientcvar`, `probe_model`, a tracked player's
  hits logged once). `cases.py crate_hits gravity_more log_lines ready
  spawns_more watcher models`; the numbers in specs/hl2dm/rules.md Open
  questions 15 and gravity_gun.md 13.

  | Case | Real server | Ours before | Ours now |
  |---|---|---|---|
  | crates punted into a bot (P10, P11) | 10 kg 1500: 20+10; 30 kg 1000: 20+10+5, 1100: 50+10+5; 100 kg 730: 50+10+10; 250 kg 650: 100+5+10 | 10+50; 10+50, –; 20+100; 50+500 (the landing's two contacts) | **as measured** (`source_logic` bridge: a side hit, normal within 0.7 of level, is three contacts, `PLAYER_SIDE_SHARES` by mass, mid-band fits; landings keep their two, CS:S's drops unchanged); pinned (`a_crate_punted_into_a_player_hurts_as_the_probe_measured`) |
  | P14 multiplayer crate punted into a bot | as prop_physics: 20+10+5, 50+10+10 | same rule | same; pinned (same test) |
  | heavy crate on a head | stays on it | 100 kg at 780 and 250 kg at 582 rose about 30 units and landed again 0.7 s on (5+10): the player's shadow, pushed 5 units down (no floor under it), was driven back up in one step and threw the crate | **stays** (`source_movement::shadow`: a shadow pushed below where it was sent is put back up, not driven); pinned (the drop test: no later hits, its bottom on the head) |
  | P3 floor punt | 1114 in/s at 51.4°, 1490°/s; first tick (773, 898) | 1098 at 46°, 1670°/s; (773, 728) | unchanged: the solver. Restitution alone moves it (0.5: 52° but 590°/s), friction not at all; no setting gives both. Test unchanged (3 %, 6°) |
  | P8 pull from 600 | 75.7, 72.0 ... 46.6, 121.8 ... (81.3 a tug every 7 ticks less 5.5 a tick of friction) | – | within 5.3 every tick (81.0 on the first); pinned (`a_crate_pulled_from_600_as_the_probe_measured`) |
  | P12 caught ball | 959 until the first bounce, then 1500 | same | same (the test's doc names the probe) |
  | P13 respawnable, no RespawnTime | back 60.08 s on; blocked: 1 s retries | 60 s, 1 s retries | same; pinned (`a_respawnable_prop_without_a_time_comes_back_after_60_s`) |
  | P15 `Break` on prop_physics | stays: drawn, not solid, no damage | removed with its pieces | **stays** (`KeepPropsOnBreak`, HL2:DM's: any break but lethal damage keeps the prop, broken); pinned (`the_break_input_leaves_the_prop`) |
  | P7 held against a frozen crate | gap 41.6: drop 57 ticks after the grace (the formula's); 14: never in 333; 11.6: after 266 | the formula | unchanged (the small gaps don't fit the formula with our gap estimate; not settled) |
  | rules P6 Combine models in turn | prisonguard, super soldier, police, combine_soldier | same (tests 4-6) | same |
  | rules P9 telefrag | kills at 60 and 62 units from the spot (the box within 48), not at 66 | origin within 48 (60 lived) | **the box within 48**; pinned (`spawn_spots_as_the_probe_measured`) |
  | rules P10 | a body and a spectator on a spot take it | the body yes, a spectator no | **both**; the telefrag skips them; pinned (same test) |
  | rules P12 log lines | kill, `say`, suicide, team change as specced; DM `say_team`: no line; TDM's: `say_team` | no chat lines | **chat lines** (`limits::log_says`, from `game_log::Said`, written by `net::chat::deliver_say`); pinned (`the_log_lines_as_the_probe_measured`) |
  | rules P13 watcher | a spectating bot kept its dead target 7.5 s | moves on when the target is respawnable | unchanged: a bot's observer may not think as a human's (Open questions 8) |
  | rules P14 ready-up | as specced; restart 5.8 s after the last `ready` | same (tests 44-46) | same |

  Fits and guesses: the side-hit shares are mid-band fits to one punt
  per mass (10, 30, 100, 250 kg; heavier clamp to 250's); the 0.7 split
  between a side hit and a landing is ours; heavier crates past 250 kg
  and glancing hits weren't measured. Left: the bot pushed back by a
  punted crate (650 in/s for 30 kg, 1110 for 100 kg on the hit's tick;
  ours only by the damage shove, 4 ticks later as the impact is measured
  over 4 steps), punted crates bouncing back off the shadow in ours (the
  real one keeps on forward), the floor punt's edge contact, P7's small
  gaps, P13 with a human spectator, the chat flood limit.

- 2026-11-08: **Gravity gun's physics rules** (branch `hl2dm-physrules`,
  probe round 5: `cases.py phys_rules phys_rules2`, gravity_gun.md Open
  questions 14).

  | Item | Spec or measured | Ours before | Ours now |
  |---|---|---|---|
  | launched prop's first impact | 10x its impact damage; "onfirstimpact break" breaks (9); measured on a crate and a drum | 1x, no break | as spec (`FirstImpactScale`, `LaunchedFromHold`); pinned (`a_launchs_first_impact_counts_ten_times`, `first_impact_props_break_only_after_a_launch`) |
  | explosive prop's credit | last attacker, influencer within 2 s, break's attacker (9) | last attacker, break's | as spec (`BlastInfluenceWindow`); pinned (`an_explosive_props_blast_credit`) |
  | several bodies | shares of 2.5, held weights of 4 | every object one body, ragdolls dry | a non-flesh prop_ragdoll: as spec (`punt_shares`); pinned (`an_object_of_several_bodies`) |
  | sawblade, carry angles | (−5, 0, 0) in the view frame, 5000 deg/s about z; measured (−5, −180, 0), 3250 deg/s; preferred angles in the yaw frame | kept rotation, random spin | as measured (the cap and drag give 3280); pinned (`launch_spin_by_the_props_interactions`, `preferred_carry_angles_hold_in_the_yaw_frame`) |
  | held item's touch box | 48; measured: a battery taken 4 and 6 ticks after the grab from 100 and 130 | 24, never taken | 48; pinned (`a_held_battery_is_taken_on_its_way_in`) |
  | free ball | punt adds 959.2; measured 1959 through its bounces | no punt | as measured; pinned (`a_free_ball_can_be_punted`) |
  | held ball's end | as any ball's (projectiles.md 5.6) | sound and removal | `FlyerGone` expired; pinned (`a_held_balls_end_is_a_balls_end`) |
  | ball on props | 1000 crush; knocked (5.3) | crush only (the shortcut's note was stale); no knock | crush; knock a whole elastic hit of 150 kg (a model); pinned (`the_ball_crushes_and_knocks_props`) |
  | punt into a player | bot's fastest 340, 454, 795, 1094 in/s by 10, 30, 100, 250 kg (round 4's 650, 1110 were two ticks' steps added); crate after 8, 165, 337, 445 | crate back off the shadow, bot by the shove only, 4 ticks late | knocked on the hit's tick, hits on that tick, crate forward (`ShadowKnock`, HL2:DM's own; CS:S adds neither system nor rule); pinned (`a_punted_crate_knocks_a_player_and_goes_on`) |
  | blast and water, doors | 2 steps 2-4 | no rule; a door shields 0.25 | as spec (`SourceRadius::water_lift`, `stoppers`); pinned (`blast_rules`) |
  | missile, bolt, ball on the sky | removed silently (6.5, 7.4); ball's end (5.2) | a wall | as spec (`Flyer::sky_end`, `MapSkyFaces`); pinned (`blast_rules`) |
  | launch's distance check on a client | spec's rule, the held entity networked (14) | the server's alone | the client reads the snapshot's pose (`map::held::ServerPose`); pinned (`a_client_predicts_the_launchs_distance_check`) |

  Fits and guesses: `PROP_KNOCK`'s elasticities (0.07, 0.15) are fits to one
  punt per mass; its 50 in/s floor and the side test (0.7) are ours; the
  ball's knock is a model; a held item's box above and below is the spec's
  ±48 (test 59), not measured apart from the sideways reach. Left: a grab
  isn't predicted (section 3: "the client predicts nothing of it", and the
  object is the server's choice); `paintsplat` and the other interactions
  stock models name; a `+use` carry and `preferred_carryangles` (section 8
  doesn't say); the knock on a moving player and glancing hits, unmeasured.

## Decision log

- 2026-10-31 (user): Half-Life 2: Deathmatch is the next game, after
  console namespaces and the ECS cleanup.
- 2026-11-01: Source behaviour two Source games need goes to a new
  Bevy-level crate in `source/`, not Chimera and not `games/hl2dm`
  (architecture-review.md A5); what any game could use goes to
  Chimera. Lifts move first and generalize second.

- **2026-11-01 (user):** install both now (done: the game, app 320, and
  the dedicated server, 232370, on the dev box); engine-level Source
  console names (map, quit, fps_max, video, network) move to `chimera:`
  while game-tuned values (`sv_gravity`, ...) stay per game (open
  question 2, option C); `apps/hl2dm` is faithful DM/TDM play first, its
  GameUI menus and server browser later (question 3); spec sessions run
  on this box as separate sessions, with the public Source SDK 2013
  cloned outside the repository (question 6).

- **2026-11-03 (user):** code from approved permissive sources listed in
  `THIRD_PARTY.md` may be copied or adapted with credit (a header in each
  file; checked by `architecture.rs`). Only hl2-rs (MIT) is approved. It
  is a reference implementation for Source shaders (Eyes, Teeth, iris,
  flexes), model lighting, animation layers, sky and HUD, and is usable
  directly with credit. The Source SDK 2013 and everything unlisted stay
  spec-only.
