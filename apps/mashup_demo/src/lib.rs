//! The first mashup app (apps/mashup_demo, crate `mashup_demo`, binary
//! `mashup_demo`): Counter-Strike: Source's maps played as Combat Arms
//! (docs/DESIGN.md, "Goal": a Combat Arms character on a CS:S map). Two
//! games as libraries, composed here and nowhere else: CS:S gives the map
//! (`cs_source::game::MAP_LOADER`: the world, its collision, entities,
//! sounds, spawn points), its map logic (Source's, `source_logic`) and,
//! offered in Combat Arms' slots, its guns (`cs_source::game::weapons::
//! CsGunsPlugin`); Combat Arms everything else played on it (its
//! movement, the M4A1, knife and grenades with their view models, its
//! soldiers' bodies, its HUD, its bots on navigation generated from the
//! map's collision, Team Death Match). Neither game knows the other
//! (checks/tests/it/architecture.rs).
//!
//! - `MAP_SOURCES`: Chimera's `map::MapSources` with both games' loaders
//!   and Combat Arms as the gameplay: a CS:S map loses CS:S's parts
//!   (bodies, view models, HUD, radio, navigation, round sounds) but its
//!   guns' held and view models (`KEPT`) and gets Combat Arms'
//!   (`combat_arms::game::world::DRESSING`); its terrorist spawns are
//!   Alpha's, its counter-terrorist spawns Bravo's (`TEAMS`); its surface
//!   names are Combat Arms' surface types through this app's table
//!   (`SURFACES`, data/surfaces_cs_source_combat_arms.toml), so Combat
//!   Arms' impacts, footsteps and shoot-through pick the right surface.
//!   Combat Arms' own maps load as in its app.
//! - Units: every loader gives metres (CS:S's inch, Combat Arms'
//!   `meters_per_unit`), and each game's numbers stay in its own unit, so
//!   the soldier is Combat Arms' size on de_dust2 with nothing rescaled
//!   (`map::sources`).
//! - `MashupPlugins`: Combat Arms' simulation (`combat_arms::SimPlugins`,
//!   `CombatArmsPlugins`: the default game, for bare map names), CS:S
//!   registered after it for its install (`map` lists the default game's
//!   maps; `map cs_source:<name>` loads a CS:S one) and
//!   `MashupPartsPlugin`. The app (`APP`) sets Combat Arms as the
//!   gameplay (`chimera_core::gameplay`: its rules, HUD, binds and tick);
//!   CS:S's rules aren't in the app, only its maps, logic and guns.
//! - `MashupPartsPlugin`: what the app adds to Combat Arms' simulation
//!   (the headless tests run it on `combat_arms::harness::CaSim`): the
//!   map logic of Source maps (doors, buttons, breakables, triggers,
//!   movers: `source_logic::logic::LogicPlugin`, with any game's
//!   characters through Chimera's character, damage and +use vocabulary),
//!   CS:S's guns (their data, hooks, accuracy and recoil only) and
//!   `loadout_slot <slot> <weapon>`: a slot of the loadout (Combat Arms'
//!   `weapon::StartingWeapons`) given either game's weapon, the local
//!   player's at once, everyone's from their next spawn (`+loadout_slot
//!   primary cs_source:weapon_ak47`).
//! - `APP`: the binary's app on Chimera's client app (`chimera_client::
//!   client::app`; main.rs is `run(&APP)`): the shell with this app's
//!   hooks (`client_hooks`) and Combat Arms' HUD, CS:S's guns' names in
//!   it.
//!
//! Known shortcuts: maps load by id only (no downloaded map files, no map
//! offer to network clients); CS:S's guns have Combat Arms' effects only
//! where Combat Arms' code reads Chimera's events (no impact effects; their
//! view model is driven by Chimera's generic driver with CS:S's rules
//! (Combat Arms' lets go of it), the HUD names them from CS:S's strings
//! (`weapons::display_names`); their sounds are the map's
//! CS:S entries); their held models are CS:S's, on Combat Arms' bodies only
//! where the bones match; on a Combat Arms map they have no view model.

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_client::client::{
    AppHooks, Args,
    app::{AppMaps, GameApp, GameAppHooks},
};
use chimera_core::{console::ConsoleAppExt, game_registry::GameAppExt};
use chimera_core::core::LocalPlayer;
use chimera_sim::weapon::{Inventory, StartingWeapons, data};
use chimera_world::map::{MapData, MapSources, SpawnTeams, SurfaceTable};

/// This build's version (`version` command, bug reports); stamped by
/// build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

/// This build's commit (`git describe`: client hooks, bug reports);
/// stamped by build.rs.
pub const BUILD: &str = env!("CHIMERA_GIT");

/// The map the app starts on without `--map`.
pub const DEFAULT_MAP: &str = "cs_source:de_dust2";

/// Bots the app keeps without a `MaxPlayer` of the user's (Combat Arms'
/// Bot Deathmatch quota): two a side with the player.
pub const DEFAULT_BOTS: u32 = 3;

/// CS:S's spawn teams as Combat Arms': terrorists (1) Alpha, counter-
/// terrorists (2) Bravo (`combat_arms::game::rules::state`).
pub const TEAMS: &[SpawnTeams] = &[
    SpawnTeams {
        game: cs_source::game::GAME,
        from: 1,
        to: combat_arms::game::rules::state::ALPHA,
    },
    SpawnTeams {
        game: cs_source::game::GAME,
        from: 2,
        to: combat_arms::game::rules::state::BRAVO,
    },
];

/// CS:S's surfaces as Combat Arms' surface types: this app's table
/// (data/surfaces_cs_source_combat_arms.toml), neither game's.
pub const SURFACES: &[SurfaceTable] = &[SurfaceTable {
    map: cs_source::game::GAME,
    gameplay: combat_arms::game::GAME,
    text: include_str!("../data/surfaces_cs_source_combat_arms.toml"),
}];

/// What of CS:S's parts stays on its maps: its weapons' held and view
/// models (keyed by weapon id), for its guns in Combat Arms' slots.
pub const KEPT: &[&str] = &["cs_source:weapon_"];

/// Where this app's maps come from and whose gameplay they get.
pub static MAP_SOURCES: MapSources = MapSources {
    loaders: &[&cs_source::game::MAP_LOADER, &combat_arms::game::world::MAP_LOADER],
    gameplay: Some(&combat_arms::game::world::DRESSING),
    teams: TEAMS,
    surfaces: SURFACES,
    kept: KEPT,
    players: false,
    guests: &[],
};

/// Map `id` (`cs_source:de_dust2`, `combat_arms:warehouse`; a bare name is
/// CS:S's) for Combat Arms' gameplay.
pub fn load_map(id: &str, hdr_level: u8) -> Result<MapData, String> {
    MAP_SOURCES.load(id, hdr_level)
}

/// What this app tells the client shell: its commit, its maps
/// (`MAP_SOURCES`) and the greybox's own hooks (`APP`'s).
pub fn client_hooks() -> AppHooks {
    APP.app_hooks()
}

/// The game binary's app (`mashup_demo`, main.rs), on Chimera's client
/// app (`chimera_client::client::app`): `with_defaults`' command line,
/// `DEFAULT_MAP` without `--map`, `MashupPlugins` with Combat Arms the
/// gameplay on every map (its rules, HUD, binds and tick; CS:S's parts in
/// the app are its maps, logic and guns only), both installs as assets,
/// the shell with Combat Arms' HUD, CS:S's guns' names in it
/// (`cs_source_names`) and Combat Arms' other looks.
pub static APP: GameApp = GameApp {
    bin: "mashup_demo",
    info: &combat_arms::game::INFO,
    title: "Lucker Party: mashup demo",
    about: || {
        format!(
            "mashup_demo: Counter-Strike: Source's maps played as Combat Arms (its soldier, movement, M4A1,\n\
             knife and grenades, HUD, bots and Team Death Match). Starts on {DEFAULT_MAP} with {DEFAULT_BOTS} bots;\n\
             --map cs_source:<name> (or combat_arms:<name>) and +MaxPlayer <n> change them.\n"
        )
    },
    version: VERSION,
    build: BUILD,
    maps: AppMaps {
        load: |id, options| MAP_SOURCES.load_with(id, options),
        load_file: |id, _, _| Err(format!("{id}: the mashup loads maps by id only, not from a file")),
        file_bytes: |_| None,
    },
    // Both installs as `cs_source://` and `combat_arms://` assets.
    installs: || {
        vec![
            chimera_present::assets::InstallSource::of(&cs_source::game::INFO),
            chimera_present::assets::InstallSource::shared(combat_arms::game::INFO.id, combat_arms::game::world::mount),
        ]
    },
    sim: |app, _| {
        use chimera_core::gameplay::{Gameplay, GameplayAppExt};
        app.add_plugins(MashupPlugins).set_gameplay(Gameplay::of(&combat_arms::game::INFO));
    },
    hud: combat_arms::ui::with_hud,
    front_end: |app, _| {
        app.add_systems(Startup, cs_source_names).add_plugins(combat_arms::ui::CaUiPlugin);
    },
    hooks: GameAppHooks {
        args: Some(with_defaults),
        default_map: Some(DEFAULT_MAP),
        ..GameAppHooks::NONE
    },
};

/// The mashup's simulation: Combat Arms' (the engine's base, then its
/// parts, registering it as the default game), then CS:S registered as
/// a game whose maps this app plays, then `MashupPartsPlugin`.
pub struct MashupPlugins;

impl PluginGroup for MashupPlugins {
    fn build(self) -> PluginGroupBuilder {
        combat_arms::game::CombatArmsPlugins
            .build()
            .add_before::<combat_arms::game::data::CaDataPlugin>(combat_arms::SimPlugins)
            .add(MashupPartsPlugin)
    }
}

/// What the mashup adds to Combat Arms' simulation: CS:S registered as a
/// second game (its install; not through the group, where a second
/// `RegisterGame` would replace Combat Arms'), Source's map logic, CS:S's
/// guns and `loadout_slot` (the crate's docs). After Combat Arms' parts
/// (the default game, its weapons first in the registry).
pub struct MashupPartsPlugin;

impl Plugin for MashupPartsPlugin {
    fn build(&self, app: &mut App) {
        app.register_game(&cs_source::game::INFO);
        // CS:S's guns' view models: Chimera's generic driver with CS:S's
        // rules plays them (Combat Arms' driver lets go of a character
        // holding another game's gun).
        cs_source::game::view_anim::add_rules(app);
        app.add_plugins((
            source_logic::logic::LogicPlugin,
            cs_source::game::weapons::CsGunsPlugin,
        ))
        .console_command(
            "chimera:loadout_slot",
            "loadout_slot <primary|secondary|melee|throw|0-3> <weapon id>: that loadout slot gets the \
             weapon (either game's: cs_source:weapon_ak47, combat_arms:...), yours at once, everyone's \
             from their next spawn; no arguments: the loadout.",
            |w, a| {
                let catalog = w.get_resource::<data::WeaponCatalog>().cloned().unwrap_or_default();
                let Some(mut kit) = w.get_resource_mut::<StartingWeapons>() else {
                    return Err("no loadout".into());
                };
                match (a.first(), a.get(1)) {
                    (None, _) => Ok(Some(kit.all.join(" "))),
                    (Some(slot), Some(id)) => {
                        let slot = slot_number(slot).ok_or_else(|| format!("no slot `{slot}`"))?;
                        set_slot(&mut kit, &catalog, slot, id)?;
                        let line = kit.all.join(" ");
                        // The local player takes the loadout now.
                        let local: Vec<Entity> = w
                            .query_filtered::<Entity, (With<LocalPlayer>, With<Inventory>)>()
                            .iter(w)
                            .collect();
                        for p in local {
                            combat_arms::game::inventory::respawn_inventory(w, p);
                        }
                        Ok(Some(line))
                    }
                    (Some(_), None) => Err("loadout_slot <slot> <weapon id>".into()),
                }
            },
        );
    }
}

/// A loadout slot by name or number (a weapon's data `slot`: 0 primary,
/// 1 secondary, 2 melee, 3 throw).
pub fn slot_number(name: &str) -> Option<u8> {
    match name.to_ascii_lowercase().as_str() {
        "primary" | "0" => Some(0),
        "secondary" | "1" => Some(1),
        "melee" | "2" => Some(2),
        "throw" | "grenade" | "3" => Some(3),
        _ => None,
    }
}

/// Put weapon `id` (any registered game's) in the loadout's `slot`: in
/// place of the weapon there, else in slot order.
pub fn set_slot(kit: &mut StartingWeapons, catalog: &data::WeaponCatalog, slot: u8, id: &str) -> Result<(), String> {
    let weapon = catalog.get(id).ok_or_else(|| format!("no weapon `{id}`"))?;
    if weapon.slot != slot {
        return Err(format!("{id} is a slot {} weapon, not slot {slot}", weapon.slot));
    }
    let slot_of = |w: &str| catalog.get(w).map_or(u8::MAX, |d| d.slot);
    kit.all.retain(|w| slot_of(w) != slot);
    let at = kit.all.iter().position(|w| slot_of(w) > slot).unwrap_or(kit.all.len());
    kit.all.insert(at, weapon.id.as_str());
    Ok(())
}

/// CS:S's guns' names for Combat Arms' HUD (`ItemNames`), from CS:S's own
/// strings in its install (`weapons::display_names`).
fn cs_source_names(
    games: Res<chimera_core::game_registry::Games>,
    mut names: ResMut<combat_arms::ui::hud::ItemNames>,
) {
    let Some(mount) = games.get(cs_source::game::GAME).and_then(|g| g.mount()) else {
        return;
    };
    names.0.extend(cs_source::game::weapons::display_names(&mount));
}

/// The command line as the app plays it: `DEFAULT_MAP` without `--map`,
/// `DEFAULT_BOTS` bots unless the command line sets `MaxPlayer`.
pub fn with_defaults(mut args: Args) -> Args {
    args.map.get_or_insert_with(|| DEFAULT_MAP.into());
    if !args.console.iter().any(|c| c.to_lowercase().starts_with("maxplayer")) {
        args.console.insert(0, format!("MaxPlayer {DEFAULT_BOTS}"));
    }
    args
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_build_is_stamped() {
        assert!(!super::BUILD.is_empty() && !super::VERSION.is_empty());
        assert_eq!(super::client_hooks().build, super::BUILD);
    }

    #[test]
    fn it_starts_on_dust2_with_bots() {
        let args = super::with_defaults(chimera_client::client::Args::default());
        assert_eq!(args.map.as_deref(), Some(super::DEFAULT_MAP));
        assert_eq!(args.console, ["MaxPlayer 3"]);
        let mut own = chimera_client::client::Args::default();
        own.console.push("maxplayer 9".into());
        own.map = Some("combat_arms:warehouse".into());
        let own = super::with_defaults(own);
        assert_eq!((own.map.as_deref(), own.console.len()), (Some("combat_arms:warehouse"), 1));
    }

    #[test]
    fn a_slot_takes_either_games_weapon() {
        let catalog = chimera_sim::weapon::data::WeaponCatalog {
            sets: vec![
                combat_arms::game::data::CaData::builtin().weapon_set(),
                cs_source::game::weapons::cs_weapons(),
            ],
        };
        let mut kit = chimera_sim::weapon::StartingWeapons {
            team: Vec::new(),
            all: combat_arms::game::inventory::STARTING_WEAPONS.to_vec(),
        };
        let primary = super::slot_number("primary").unwrap();
        super::set_slot(&mut kit, &catalog, primary, cs_source::game::weapons::AK47).unwrap();
        assert_eq!(kit.all[0], cs_source::game::weapons::AK47, "{:?}", kit.all);
        assert_eq!(kit.all.len(), combat_arms::game::inventory::STARTING_WEAPONS.len());
        assert!(super::set_slot(&mut kit, &catalog, primary, cs_source::game::weapons::USP).is_err(), "a pistol isn't primary");
        assert!(super::set_slot(&mut kit, &catalog, 1, "nothing").is_err());
        super::set_slot(&mut kit, &catalog, 1, cs_source::game::weapons::USP).unwrap();
        assert_eq!(kit.all[1], cs_source::game::weapons::USP, "in slot order: {:?}", kit.all);
    }

    #[test]
    fn the_surface_table_reads() {
        let rows = super::SURFACES[0].rows().unwrap();
        assert_eq!(rows.get("sand", None), Some("Desert_Sand"));
        assert_eq!(rows.get("wood_crate", None), Some("Wood"));
        assert_eq!(rows.get("something_else", Some('M')), Some("Metal"));
        let tables = combat_arms::game::effects::data::CaFxTables::built_in();
        for name in rows.names.values().chain(rows.materials.values()).chain(&rows.default) {
            assert!(tables.surface_id(name).is_some(), "{name} is a Combat Arms surface type");
        }
    }

    #[test]
    fn both_games_are_registered_combat_arms_first() {
        let mut app = bevy::prelude::App::new();
        app.add_plugins(chimera_core::game_registry::RegisterGame(&combat_arms::game::INFO))
            .add_plugins(super::MashupPartsPlugin);
        let ids: Vec<&str> = app
            .world()
            .resource::<chimera_core::game_registry::Games>()
            .all()
            .map(|g| g.id)
            .collect();
        assert_eq!(ids, [combat_arms::game::GAME, cs_source::game::GAME]);
    }

    #[test]
    fn both_games_have_loaders_and_combat_arms_plays() {
        let s = &super::MAP_SOURCES;
        assert!(s.loader("cs_source").is_some() && s.loader("combat_arms").is_some());
        assert_eq!(s.gameplay.map(|g| g.game), Some(combat_arms::game::GAME));
    }
}
