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
| C7 | `chimera_pack`'s game list | `src/bin/chimera_pack.rs` (`games() -> [Game; 2]`, `record_pieces` by game id) | a third game: the list from the registered `GameInfo`s (app id from `steam`), the pieces' sets as a hook the game provides | Low: its unit tests, heavy `served.rs` |
| C8 | Console names shared by Source games | `chimera_core::console` (`SOURCE_NAMESPACE` = `cs_source`) | decide where HL2:DM's `sv_gravity`, `fps_max`, `rate` live (section 5, open question 2) | Low/medium: `console_scope.rs`, console unit tests, `persist.rs` |

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
| M0 install and look | H0 (install facts) | done (branch `hl2dm-m0`, 2026-11-01): S0, S1, `games/hl2dm` skeleton, `dump hl2dm`, heavy `mount.rs`; left for M1/M3: the RCON reads and the probe (below) | S0, S1, (S11) |
| M1 maps load | H0 | not started | S2, S3, S4 |
| M3 movement, suit | H1, H2 | not started | S5, S6, C2, C8 |
| M4 bullets, melee, UGG | H3 | not started | S8, C4, C7 |
| M5 projectiles | H4 | not started | C1 |
| M6 gravity gun | H5 | not started | C6 |
| M7 rules, pickups | H6, H10 | not started | C3, (C5) |
| M8 bodies | H7 | not started | S9 |
| M9 HUD | H8 | not started | S7 |
| M10 effects, sounds | H9 | not started | S10, S5 |
| M11 assets | none | not started | C7 |

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
