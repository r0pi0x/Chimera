# What Chimera gives a game

A catalog for game authors: what already exists, where it lives, and
how to use it in one line. Read the module's `//!` doc for the detail
([modules.md](modules.md) lists every file). Before writing something,
look here; if it's missing, see "Generalize on the second user" below.

## The game's seams

| Capability | Where | Use it like this |
|---|---|---|
| Registering a game | `chimera_core::game_registry` (`GameInfo`, `RegisterGame`) | `RegisterGame(&INFO)` first in your plugin group; `INFO` says id, tick, unit, install, maps |
| Install discovery | `chimera_core::mount::install`, `mount::config` | nothing to write: `CHIMERA_<ID>_ROOT`, `[games.<id>] path`, Steam via `INFO.steam` |
| Mounts and archives | `chimera_vfs` (`Mount`, `FileSource`, `LooseDir`), `chimera_core::mount::served` | `served::open(GAME, install, open_install)`; push one `FileSource` per archive |
| Slots (movement, HUD, ...) | `chimera_core::slots` | `app.register_movement::<M>("<id>:movement")`; the loadout picks by id |
| Gameplay choice | `chimera_core::gameplay` | `set_gameplay(Gameplay::of(&INFO))` in your harness; never assume you're the only game |
| What a game lends | `chimera_core::game_parts` (`GameParts`) | list your guns, movement, HUD in `PARTS`; the Ultimate Gun Game's ladder and sandbox find them |
| Party bundle | `chimera_world::game_bundle` (`GameBundle`, `GameClient`) | `BUNDLE` in `games/<id>`, `CLIENT` in `<id>_ui`, one line each in `apps/party_roster` |
| Data documents | `chimera_core::game_data!` | `const D: GameData = game_data!("data/x.toml")`; edits need no rebuild in dev |
| Headless tests | `chimera_world::harness::Sim`, `greybox` | `Sim::new(<Game>Sim, GreyboxMapPlugin)`, `spawn_character`, `intent`, `seconds` |

## Movement and bodies

| Capability | Where | Use it like this |
|---|---|---|
| Movement contract | `chimera_core::slots` (`register_movement`), `core::{Intent, MovementState, Velocity}`, `core::Predict::Movement` | a marker component and a system in `Predict::Movement` reading `Intent`, writing `Transform`/`Velocity`/`MovementState` |
| Placeholder movement | `chimera_sim::movement::placeholder` | `app.placeholder_movement(ID, Tuning { .. })` until your own exists (what `tools/new-game` uses) |
| Source movement | `source_engine::source_movement` | `app.source_movement(ID, Some(config), Some(GAME))` for a Source game |
| Envelope / character fit | `chimera_core::envelope` | `add_movement_envelope(GAME, ..)` from your movement data: other games' characters fit your maps and yours theirs |
| External pushes | `chimera_core::core::ExternalPush` | a knockback sets `lift` or `ground_limit`; your movement honours it (`take_on_ground`) |
| Characters | `chimera_core::character` | `spawn_character(..)`; `CharacterPartsExt::on_every_character` for your own parts |
| Bodies and view models | `chimera_world::map::{bodies, view_model, held}` | load models into the map's kit; worn by `GameIdentity` / `BodyName` |

## Weapons and damage

| Capability | Where | Use it like this |
|---|---|---|
| Weapons as data | `chimera_sim::weapon::data`, [weapon-data.md](weapon-data.md) | one TOML document per gun in `data/weapons/`; `app.add_weapon_data(..)`; the `add-weapon-from-data` skill |
| Hitscan, hit zones, armour | `chimera_sim::weapon::{hits, deliver}`, `Hitgroup` | `[hitscan]` and `[damage] hitgroups` in the data |
| Projectiles | `chimera_sim::weapon::{projectile, flight}` | `[hooks."chimera:projectile"]` in the data (weapon-data.md, "Chimera's flyers"): speed, gravity, what a touch does |
| Blasts and grenades | `chimera_sim::weapon::grenade` (`Blast`, `Throwable`) | a `Blast` with radius and falloff; a thrown weapon as data |
| Lag compensation | `chimera_sim::weapon::lagcomp` | a network server traces remote players' shots where they saw them; nothing to add for hitscan |
| Behaviour hooks | `chimera_sim::weapon::data` (registered hooks by name) | `[hooks."<id>:<hook>"]` in the data, the hook registered in Rust |

## Effects, sound, HUD

| Capability | Where | Use it like this |
|---|---|---|
| Decals | `chimera_world::map::decal` | put `MapDecals` on the map; write `PlaceDecal` on a hit |
| Particles | `chimera_world::map::{particles, emitters}` | build a group with your numbers, add to `Particles` |
| Tracers, muzzle flashes, impacts | `chimera_world::map::{tracer, muzzle, impact, material_fx}` | entries in the map's kit; the shared hit path triggers them |
| Sound | `chimera_world::map::sound`, `chimera_present::audio` | named sound entries in the kit; write `PlaySound` |
| Source hearing | `chimera_world::map::hearing`, `chimera_present::audio::hearing` | Source's distance curve and the blast/flash ear ringing (`HearingEffect`) |
| HUD hooks | `chimera_client::client::{hud, hud_minimal}`, `GameParts::hud` | `with_hud` in `<id>_ui` adds your HUD; none: the minimal HUD shows |
| Layouts | `chimera_client::client::{canvas, vgui}`, `front_end` | reference-canvas widget trees, or the game's own VGUI menus |
| UI kit | `chimera_ui` | the shell's widgets, theme, pickers and forms for any new screen |

## Rules, network, assets

| Capability | Where | Use it like this |
|---|---|---|
| Modes and rules | `chimera_sim::{mode, rule_module, spawning}` | `add_modes` with data and hooks, gated by `gameplay_is(GAME)` |
| Minigames | `chimera_sim::minigame` | a definition file; put your guns into existing modes, don't add new ones unasked |
| Net prediction | `chimera_core::core::PredictedAppExt` | `app.predicted_net::<YourMovementState>()` |
| Replicating your own state | `chimera_net::net::parts` | `app.net_part("<id>", register)`; tests on `chimera_net::harness::NetSim` |
| Dedicated server and app | `chimera_net::dedicated`, `chimera_client::client::app` | a `GameServer` / `GameApp` static and one call (new-game.md, M1) |
| Map loading | `chimera_world::map::sources` (`MapLoader`), `map::kept` | `MAP_LOADER.load` returns `MapData` in metres; guest dressings through `kept::dress_cached` |
| Asset packs and converters | `chimera_assets::{pack, sets}`, `src/bin/chimera_pack.rs` | your game's row in `chimera_pack` (owner reviewed); clients fetch sets on demand |
| Per-game gating | `docs/plans/active/game-logic-distribution.md` (landing) | once it lands: put your systems in your game's system set; an absent game costs nothing |

## Generalize on the second user

A game may build anything it needs inside its own crate first. When a
**second** game needs the same thing, it moves into Chimera (or the
loader family's shared crate, e.g. `source/engine`), and both games use
it from there ([DESIGN.md](DESIGN.md), "Components and
categorization": "a slot interface is not proven until two games
implement it"). Never make one game depend on another or copy its code:
the architecture check refuses it. Infrastructure (caches, loading,
performance) goes in Chimera from the start.
