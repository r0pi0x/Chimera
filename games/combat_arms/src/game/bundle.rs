//! Combat Arms as one bundle for an app made of several games (lift C4,
//! `chimera_world::game_bundle`): its info, parts and maps, its own binds
//! (Shift sprints, C walks: Alt is free look in Chimera), its view
//! models' field of view (its HUD on the client is its front end's
//! `GameClient`, `ui::PARTY_CLIENT`). The party apps play on it as their base game (its
//! plugins, its gameplay's dressing of every map, its bots: `BASE`), so
//! its `sim` adds nothing and it is no guest on other games' maps.
//!
//! As the base (`BASE`): its modes off (`ca_mode none`: its rules leave
//! the players to the composition's minigame, a joiner comes in at a free
//! spawn point), its bots given the other games' guns' AI rows
//! (data/party_bot_gun_types.toml, else by the gun's class,
//! `bot_gun_type_of`) and the widest hull the party's games make
//! (`WIDEST_HULL`).

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_world::game_bundle::{BaseGame, GameBundle};

/// Combat Arms' bundle.
pub static BUNDLE: GameBundle = GameBundle {
    info: &super::INFO,
    parts: &super::parts::PARTS,
    loader: &super::world::MAP_LOADER,
    guest: None,
    kept: &[],
    binds: &[("shift", "+sprint"), ("c", "+walk")],
    view_model_fov: Some(fov),
    view_model_fov_cvar: None,
    base: Some(&BASE),
    sim: |_| {},
    display_names: None,
    player_options: Some(chimera_core::game_data!("data/player_options.toml")),
};


/// Its view models' field of view (`ca_viewmodel_fov`'s default and
/// its default field of view).
pub fn fov() -> crate::map::GameFov {
    let s = super::view_model::settings();
    crate::map::GameFov {
        game: super::GAME,
        fov: s.fov,
        default_fov: s.default_fov,
        follows_player: false,
    }
}

/// Combat Arms as a composition's base game (module doc).
pub static BASE: BaseGame = BaseGame { plugins, compose, finish };

fn plugins() -> PluginGroupBuilder {
    super::CombatArmsPlugins.build().add_before::<super::data::CaDataPlugin>(SimBase)
}

/// The crate's `SimPlugins` (the engine's base and the tick metrics),
/// which the `game` module may not name (it is the crate root's).
struct SimBase;

impl Plugin for SimBase {
    fn build(&self, app: &mut App) {
        app.add_plugins(chimera_sim::SimBasePlugins).add_plugins(chimera_core::metrics::TickMetricsPlugin);
    }
}

/// The widest player the party's games make (half, m): Source's, 16 u.
/// A bot wears it: a mesh for Combat Arms' narrow one (15 u of 0.0185 m)
/// leads a Source hull into gaps it doesn't fit, where it stands pressing
/// on.
pub const WIDEST_HULL: f32 = 16.0 * 0.0254;

/// Combat Arms' bots' gun types for other games' guns
/// (data/party_bot_gun_types.toml).
pub const BOT_GUN_TYPES: chimera_core::game_data::GameData = chimera_core::game_data!("data/party_bot_gun_types.toml");

fn compose(app: &mut App) {
    if let Some(mut s) = app.world_mut().get_resource_mut::<super::rules::CaRuleSettings>() {
        s.mode = "none".into();
    }
    if let Some(mut bots) = app.world_mut().get_resource_mut::<super::bots::BotData>() {
        bots.gun_types.extend(bot_gun_types());
    }
    if let Some(mut bots) = app.world_mut().get_resource_mut::<super::bots::BotSettings>() {
        bots.walker_radius = Some(WIDEST_HULL);
    }
}

/// The guns a ladder may hand out that the table doesn't name (Combat
/// Arms' launchers, its install's weapons): their AI row by class.
fn finish(app: &mut App) {
    let classed: Vec<(String, chimera_core::game_parts::GunClass)> = app
        .world()
        .get_resource::<chimera_sim::weapon::data::WeaponCatalog>()
        .map(|c| c.sets.iter().flat_map(|s| &s.weapons).filter_map(|w| Some((w.id.clone(), w.class?))).collect())
        .unwrap_or_default();
    if let Some(mut bots) = app.world_mut().get_resource_mut::<super::bots::BotData>() {
        for (id, class) in classed {
            if let Some(row) = bot_gun_type_of(class) {
                bots.gun_types.entry(id).or_insert_with(|| row.to_string());
            }
        }
    }
}

/// Combat Arms' bots' AI row for a gun of `class` (None: a grenade, which
/// they don't throw; they knife on its rung).
pub fn bot_gun_type_of(class: chimera_core::game_parts::GunClass) -> Option<&'static str> {
    use chimera_core::game_parts::GunClass::*;
    match class {
        Pistol | HeavyPistol => Some("Pistol"),
        Smg | Carbine | Rifle => Some("Default"),
        Shotgun => Some("SG"),
        Heavy | Launcher | Flamethrower => Some("MG"),
        Marksman | Sniper => Some("SR"),
        Grenade => None,
    }
}

/// `BOT_GUN_TYPES` read: weapon id to Combat Arms' gun type.
pub fn bot_gun_types() -> Vec<(String, String)> {
    let table: toml::Table = toml::from_str(&BOT_GUN_TYPES).expect("data/party_bot_gun_types.toml reads");
    table
        .into_iter()
        .map(|(id, kind)| (id, kind.as_str().expect("a gun type's name").to_string()))
        .collect()
}
