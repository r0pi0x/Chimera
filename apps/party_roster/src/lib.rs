//! The party's roster (apps/party_roster, crate `party_roster`;
//! docs/plans/active/architecture-for-iteration.md, stage 3): which games
//! Lucker Party and the Ultimate Gun Game compose and how, handed to the
//! party kit (apps/party_kit, which names no game) as one descriptor,
//! `PARTY` (`party_kit::Party`), and its front end's, `client::CLIENT`
//! (`party_kit::client::PartyClient`). A game's edit rebuilds this crate,
//! not the kit; a new game is a line here (its bundle, its front end's
//! `GameClient`), not an edit of the kit.
//!
//! Everything of the kit is re-exported (`pub use party_kit::*`), and the
//! kit's items that take the party are here with it given: `load_map`,
//! `first_map`, `requirements`, `PartyPlugins`, `PartyPartsPlugin`, the
//! client app (`client::PARTY`, Lucker Party's `client::APP`) and the
//! dedicated server (`server::PARTY`, `server::SERVER`). The four party
//! apps depend on this crate; the composition tests (tests/it,
//! tests/server) live here.
//!
//! - The games: Combat Arms the base (its soldiers, movement, HUD, bots and
//!   guns; its gameplay until a minigame takes the rules slot), CS:S,
//!   HL2:DM, Halo and TF2 the guests (`GUESTS`: their maps, guns, bodies
//!   and HUDs; `MapSources::guests`), and the original minigames
//!   (`originals`).
//! - The pairs' data: each guest's spawn teams as Combat Arms' Alpha and
//!   Bravo (`TEAMS`), its surfaces as Combat Arms' through the mashup's
//!   table (`SURFACES`: apps/mashup_demo/data/
//!   surfaces_cs_source_combat_arms.toml, read from there).
//! - The base game set for the party (`sim_rules`): Combat Arms' modes off
//!   (`ca_mode none`), its bots given the other games' guns' types
//!   (data/bot_gun_types.toml, `bot_gun_type_of`) and the widest hull
//!   (`WIDEST_HULL`).
//! - What a machine lacks (`missing`, `requirements`): CS:S's install,
//!   Combat Arms' and its V1 and V2 keys.
//! - The modules that are one game's mechanics in the party: the host's
//!   surprises (`surprise`: Combat Arms' air strike), the practice targets'
//!   bodies (`target_bodies`: TF2's classes, Halo's Spartan), the map
//!   tools' spot checks (`spot_checks`: Combat Arms' bots' walker) and
//!   `map_fitness` (a host tool, `--features tools`).

pub use party_kit::*;

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_world::{
    game_bundle::GameBundle,
    map::{MapData, MapDressing, MapLoadOptions, MapSources, SpawnTeams, SurfaceTable},
};

pub mod server;
pub mod spot_checks;
pub mod surprise;
pub mod target_bodies;
#[cfg(feature = "present")]
pub mod client;
#[cfg(feature = "present")]
pub use client::{PartyMousePlugin, client_hooks, with_defaults, with_defaults_for, with_huds};

/// The minigame played without `+minigame`.
pub const DEFAULT_MINIGAME: &str = originals::GUN_GAME_DUST2;

/// The games the party composes besides its base game (Combat Arms,
/// whose gameplay, bots and HUD everything plays on), each one bundle
/// (`chimera_world::game_bundle`, lift C4): CS:S, HL2:DM, Halo (its maps
/// from its disc image, its guns) and TF2 (its maps by id, its four guns:
/// docs/plans/active/tf2.md, M2b), in the order an automatic ladder takes
/// them.
pub static GUESTS: [&GameBundle; 4] = [&cs_source::game::BUNDLE, &hl2dm::game::BUNDLE, &halo_ce::game::BUNDLE, &tf2::game::BUNDLE];

/// The base game's bundle.
pub static BASE: &GameBundle = &combat_arms::game::BUNDLE;

/// Every game the party composes: the guests, then the base.
pub static BUNDLES: [&GameBundle; 5] = [GUESTS[0], GUESTS[1], GUESTS[2], GUESTS[3], BASE];

/// CS:S's spawn teams as Combat Arms': terrorists (1) Alpha, counter-
/// terrorists (2) Bravo; TF2's the same way (RED 1 Alpha, BLU 2 Bravo); Halo's capture the flag starts (`halo_ce::game::
/// maps::spawns`: red 1, blue 2) likewise.
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
    SpawnTeams {
        game: halo_ce::game::GAME,
        from: 1,
        to: combat_arms::game::rules::state::ALPHA,
    },
    SpawnTeams {
        game: halo_ce::game::GAME,
        from: 2,
        to: combat_arms::game::rules::state::BRAVO,
    },
    SpawnTeams {
        game: tf2::game::GAME,
        from: 1,
        to: combat_arms::game::rules::state::ALPHA,
    },
    SpawnTeams {
        game: tf2::game::GAME,
        from: 2,
        to: combat_arms::game::rules::state::BRAVO,
    },
];

/// The Source games' surfaces (CS:S's, HL2:DM's and TF2's: Source's names and
/// material letters) as Combat Arms' surface types: the mashup's table.
/// Halo's maps name their surfaces with the engine's generic names
/// (games/halo_ce/data/surfaces.toml: concrete, metal, dirt, ...), which
/// are Source's too: the same table.
pub const SURFACES: &[SurfaceTable] = &[
    SurfaceTable {
        map: cs_source::game::GAME,
        gameplay: combat_arms::game::GAME,
        text: chimera_core::game_data!("../mashup_demo/data/surfaces_cs_source_combat_arms.toml"),
    },
    SurfaceTable {
        map: hl2dm::game::GAME,
        gameplay: combat_arms::game::GAME,
        text: chimera_core::game_data!("../mashup_demo/data/surfaces_cs_source_combat_arms.toml"),
    },
    SurfaceTable {
        map: halo_ce::game::GAME,
        gameplay: combat_arms::game::GAME,
        text: chimera_core::game_data!("../mashup_demo/data/surfaces_cs_source_combat_arms.toml"),
    },
    SurfaceTable {
        map: tf2::game::GAME,
        gameplay: combat_arms::game::GAME,
        text: chimera_core::game_data!("../mashup_demo/data/surfaces_cs_source_combat_arms.toml"),
    },
];

/// What of the guests' parts stays on their own maps: their guns' held
/// and view models (Halo's too: its guns in hand on its maps; TF2's).
pub static KEPT: [&str; 4] = [GUESTS[0].kept[0], GUESTS[1].kept[0], GUESTS[2].kept[0], GUESTS[3].kept[0]];

/// The guests' players, guns and HUD looks on the maps of other games
/// (Halo's: its Spartan body; TF2's: its guns, no bodies yet).
pub static GUEST_DRESSING: [&MapDressing; 4] =
    [GUESTS[0].guest.unwrap(), GUESTS[1].guest.unwrap(), GUESTS[2].guest.unwrap(), GUESTS[3].guest.unwrap()];

/// Where this app's maps come from (a bare map name is the first guest's,
/// CS:S's) and whose gameplay dresses them.
pub static MAP_SOURCES: MapSources = MapSources {
    // Last, the code-built maps' kit (the range, the greybox: dressed as
    // any other game's map).
    loaders: &[
        GUESTS[0].loader,
        GUESTS[1].loader,
        GUESTS[2].loader,
        GUESTS[3].loader,
        BASE.loader,
        &chimera_world::map::sources::CODE_MAP_KIT,
    ],
    gameplay: Some(&combat_arms::game::world::DRESSING),
    teams: TEAMS,
    surfaces: SURFACES,
    kept: &KEPT,
    // A player holding a guest's gun is that game's (the gun game's
    // `identity = "weapon"`): its player bodies and HUD look stay on its
    // maps.
    players: true,
    // On the other games' maps (Combat Arms' Warehouse, CS:S's on
    // HL2:DM's and back) a guest's gun's holder is its game's: its player
    // models, its guns' view and held models, its HUD look, read from its
    // install behind the map.
    guests: &GUEST_DRESSING,
};

/// The party: the games above, handed to the kit.
pub static PARTY: Party = Party {
    base: BASE,
    guests: &GUESTS,
    bundles: &BUNDLES,
    map_sources: &MAP_SOURCES,
    minigames: originals::MINIGAMES,
    default_minigame: DEFAULT_MINIGAME,
    base_plugins: || {
        combat_arms::game::CombatArmsPlugins
            .build()
            .add_before::<combat_arms::game::data::CaDataPlugin>(combat_arms::SimPlugins)
    },
    sim_parts,
    sim_rules,
    finish,
    requirements,
};

/// Map `id` (`cs_source:de_dust2`, `combat_arms:warehouse`) for the party.
pub fn load_map(id: &str, hdr_level: u8) -> Result<MapData, String> {
    PARTY.load_map(id, hdr_level)
}

/// Map `id` with `options` (the dedicated server's: no textures).
pub fn load_map_with(id: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    PARTY.load_map_with(id, options)
}

/// The first map of built-in minigame `id`'s pool.
pub fn first_map(id: &str) -> Result<String, String> {
    PARTY.first_map(id)
}

/// The original minigames' ids.
pub fn ids() -> Vec<String> {
    PARTY.ids()
}

/// What each composed game lends (its capability claim, `chimera_core::
/// game_parts`), in the order an automatic ladder takes them.
pub static GAME_PARTS: [&chimera_core::game_parts::GameParts; 5] =
    [BUNDLES[0].parts, BUNDLES[1].parts, BUNDLES[2].parts, BUNDLES[3].parts, BUNDLES[4].parts];

/// Each game's own binds, over the shared ones (`Console::layers`, applied
/// by the local input to a player whose identity is that game): where the
/// games disagree about a key (each bundle's `binds`; `Party::binds`).
/// Combat Arms: Shift sprints, C walks (Alt is free look in Chimera).
/// CS:S: Shift walks (`+speed`), Ctrl ducks. HL2:DM: Shift sprints
/// (`+speed`), Ctrl ducks, Alt walks, Z zooms the suit (`+zoom`; CS:S's Z
/// is its radio, a game's own layer). Halo: F melees, G throws a grenade.
/// The Controls page (`controls`) edits each.
pub static GAME_BINDS: [(&str, &[(&str, &str)]); 4] = [
    (BASE.info.id, BASE.binds),
    (BUNDLES[0].info.id, BUNDLES[0].binds),
    (BUNDLES[1].info.id, BUNDLES[1].binds),
    (BUNDLES[2].info.id, BUNDLES[2].binds),
];

/// The simulation: Combat Arms' (the engine's base, then its parts, the
/// default game), then `PartyPartsPlugin`.
pub struct PartyPlugins;

impl PluginGroup for PartyPlugins {
    fn build(self) -> PluginGroupBuilder {
        party_kit::PartyPlugins(&PARTY).build()
    }
}

/// What the party adds to Combat Arms' simulation (the headless tests run
/// it on `combat_arms::harness::CaSim`): the kit's `PartyPartsPlugin` with
/// this party.
pub struct PartyPartsPlugin;

impl Plugin for PartyPartsPlugin {
    fn build(&self, app: &mut App) {
        party_kit::PartyPartsPlugin(&PARTY).build(app);
    }

    fn finish(&self, app: &mut App) {
        party_kit::PartyPartsPlugin(&PARTY).finish(app);
    }
}

/// The widest player the party's games make (half, m): Source's, 16 u.
pub const WIDEST_HULL: f32 = 16.0 * 0.0254;

/// Combat Arms' bots' gun types for CS:S's guns (data/bot_gun_types.toml).
pub const BOT_GUN_TYPES: chimera_core::game_data::GameData = chimera_core::game_data!("data/bot_gun_types.toml");

/// Bots the app keeps without a `MaxPlayer` of the user's: a gun game of
/// four with the player.
pub const DEFAULT_BOTS: u32 = 3;

/// Each game's own view-model field of view cvar (`viewmodel_fov` is
/// CS:S's, `hl2dm_viewmodel_fov` HL2:DM's, `tf2_viewmodel_fov` TF2's; the
/// kit adds the fields of view themselves, `add_view_model_fovs`), and
/// the bodies a sandbox's practice target may wear.
fn sim_parts(app: &mut App) {
    view_model_fov_cvars(app);
    target_bodies::add(app);
}

fn view_model_fov_cvars(app: &mut App) {
    cs_source::game::view_anim::viewmodel_fov_cvar(app);
    hl2dm::game::view::viewmodel_fov_cvar(app);
    tf2::game::view::viewmodel_fov_cvar(app);
}

/// Each game's guns at the game's own view-model field of view, for a
/// player of that game (`party_kit::add_view_model_fovs`), and each game's
/// own cvar for it (all saved in the config). After Combat Arms' view
/// model settings are in.
pub fn add_view_model_fovs(app: &mut App) {
    party_kit::add_view_model_fovs(&PARTY, app);
    view_model_fov_cvars(app);
}

/// The original minigames, the host's surprises (`surprise airstrike`,
/// `surprise gravity`), and Combat Arms set for the party: its modes off
/// (`ca_mode none`: its rules leave the players to the minigame, a joiner
/// comes in at a free spawn point) and its bots given CS:S's guns' types
/// (`BOT_GUN_TYPES`) and the widest hull.
fn sim_rules(app: &mut App) {
    app.add_plugins((originals::OriginalsPlugin, surprise::SurprisePlugin));
    if let Some(mut s) = app.world_mut().get_resource_mut::<combat_arms::game::rules::CaRuleSettings>() {
        s.mode = "none".into();
    }
    if let Some(mut bots) = app.world_mut().get_resource_mut::<combat_arms::game::bots::BotData>() {
        bots.gun_types.extend(bot_gun_types());
    }
    // A bot wears the hull of its gun's game: a mesh for Combat Arms'
    // narrow one (15 u of 0.0185 m) leads a Source hull into gaps it
    // doesn't fit, where it stands pressing on.
    if let Some(mut bots) = app.world_mut().get_resource_mut::<combat_arms::game::bots::BotSettings>() {
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
    if let Some(mut bots) = app.world_mut().get_resource_mut::<combat_arms::game::bots::BotData>() {
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
    let table: toml::Table = toml::from_str(&BOT_GUN_TYPES).expect("data/bot_gun_types.toml reads");
    table
        .into_iter()
        .map(|(id, kind)| (id, kind.as_str().expect("a gun type's name").to_string()))
        .collect()
}

/// What a machine needs for the party: Counter-Strike: Source's install
/// (`cstrike` in `css`, the folder a config or Steam names), Combat Arms'
/// (`Game` in `ca`) and Combat Arms' two archive keys (`keys`: the names
/// configured, `V1` and `V2`). Pure: `requirements` reads the machine.
pub fn missing(css: Option<&std::path::Path>, ca: Option<&std::path::Path>, keys: &[&str]) -> Vec<Missing> {
    let mut out = Vec::new();
    let mut add = |fatal: bool, text: String| {
        let (game, install) = if fatal {
            (cs_source::game::GAME, true)
        } else {
            (combat_arms::game::GAME, !text.contains(" key isn't"))
        };
        let name = if fatal { "Counter-Strike: Source" } else { "Combat Arms" };
        out.push(Missing { fatal, text, game, name, install })
    };
    match css {
        None => add(
            true,
            "Counter-Strike: Source isn't installed: install it from Steam (app 240), or name its folder under \
             `[games.cs_source]` `path` in chimera.local.toml, or run `chimera_install <folder>`. Its maps and \
             guns are the gun game's."
                .into(),
        ),
        Some(p) if !p.join("cstrike").is_dir() => add(
            true,
            format!(
                "Counter-Strike: Source's folder {} has no `cstrike` folder in it: point `[games.cs_source]` `path` \
                 in chimera.local.toml (or `chimera_install`) at the folder that holds `cstrike` and `hl2`.",
                p.display()
            ),
        ),
        Some(_) => {}
    }
    match ca {
        None => add(
            false,
            "Combat Arms isn't set up: put its install folder under `[games.combat_arms]` `path` in \
             chimera.local.toml. Without it the soldiers, their guns and the HUD's art don't load."
                .into(),
        ),
        Some(p) if !p.join("Game").is_dir() => add(
            false,
            format!(
                "Combat Arms' folder {} has no `Game` folder in it: point `[games.combat_arms]` `path` in \
                 chimera.local.toml at the folder that holds `Game`.",
                p.display()
            ),
        ),
        Some(_) => {}
    }
    for scheme in ["V1", "V2"] {
        if ca.is_some() && !keys.contains(&scheme) {
            add(
                false,
                format!(
                    "Combat Arms' {scheme} key isn't in chimera.local.toml (`[games.combat_arms.keys.{scheme}]` with \
                     `key` and `iv`; see chimera.local.example.toml): what is in its {scheme} archives (some \
                     soldiers, guns and sounds) won't load, and players made of them look wrong."
                ),
            );
        }
    }
    out
}

/// `missing` for this machine: its config (`chimera.local.toml`) and
/// Steam's library; a game read from the asset store isn't missing.
pub fn requirements() -> Vec<Missing> {
    use chimera_core::mount::config::LocalConfig;
    let config = LocalConfig::load().unwrap_or_default();
    let css = config.game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref());
    let ca = config.game_path(combat_arms::game::GAME);
    let keys: Vec<&str> = config
        .game_keys(combat_arms::game::GAME)
        .map(|k| k.keys().map(String::as_str).collect())
        .unwrap_or_default();
    let served =
        |game: &str, path: Option<&std::path::Path>| path.is_some_and(|p| chimera_core::mount::served::alone(game, p));
    let (css_served, ca_served) = (
        served(cs_source::game::GAME, css.as_deref()),
        served(combat_arms::game::GAME, ca.as_deref()),
    );
    let mut out = missing(css.as_deref(), ca.as_deref(), &keys);
    // A game read from the asset store (the service) has no install folder
    // and needs no keys (its records are stored decrypted).
    out.retain(|m| !(m.game == cs_source::game::GAME && css_served || m.game == combat_arms::game::GAME && ca_served));
    out
}

/// What a client lacks (`requirements`), told (`party_kit::client_requirements`).
pub fn client_requirements() -> (String, bool) {
    party_kit::client_requirements(&PARTY)
}

/// The mount doctor's report for the network (`party_kit::mount_report`).
pub fn mount_report(games: &chimera_core::game_registry::Games) -> chimera_net::net::manifest::MountReport {
    party_kit::mount_report(&PARTY, games)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_command_line_names_its_minigame_and_its_first_map() {
        let lines = ["MaxPlayer 3".to_string(), "minigame originals:gun_game_dust2".to_string()];
        assert_eq!(super::minigame_of(&lines).as_deref(), Some("originals:gun_game_dust2"));
        assert_eq!(super::minigame_of(&lines[..1]), None);
        assert_eq!(super::first_map(super::DEFAULT_MINIGAME).unwrap(), "cs_source:de_dust2");
        assert!(super::first_map("originals:nothing").is_err());
    }

    #[test]
    fn every_cs_source_gun_has_a_bot_gun_type_combat_arms_knows() {
        let ca = combat_arms::game::bots::BotData::built_in();
        let types = super::bot_gun_types();
        for gun in cs_source::game::weapons::guns() {
            let row = types.iter().find(|(id, _)| *id == gun.id);
            let (_, kind) = row.unwrap_or_else(|| panic!("{} has no bot gun type", gun.id));
            assert!(ca.guns.contains_key(kind.as_str()), "{}: {kind} isn't one of Combat Arms' gun types", gun.id);
        }
    }

    #[test]
    fn what_a_machine_lacks_is_named_exactly() {
        use std::path::Path;
        let both = ["V1", "V2"];
        let nothing = super::missing(None, None, &[]);
        assert_eq!(nothing.len(), 2);
        assert!(nothing[0].fatal && nothing[0].text.contains("Counter-Strike: Source isn't installed"));
        assert_eq!((nothing[0].game, nothing[0].install), ("cs_source", true));
        assert_eq!((nothing[1].game, nothing[1].install), ("combat_arms", true));
        assert!(!nothing[1].fatal && nothing[1].text.contains("`[games.combat_arms]` `path`"));
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        // A folder that exists but isn't the game's.
        let wrong = super::missing(Some(here), Some(here), &both);
        assert_eq!(wrong.len(), 2, "{wrong:?}");
        assert!(wrong[0].text.contains("has no `cstrike` folder") && wrong[0].fatal);
        assert!(wrong[1].text.contains("has no `Game` folder"));
        // Installed, one key missing: only that key is named.
        let dir = here.join("../../target/party_roster_missing_test");
        std::fs::create_dir_all(dir.join("cstrike")).unwrap();
        std::fs::create_dir_all(dir.join("Game")).unwrap();
        let one = super::missing(Some(&dir), Some(&dir), &["V1"]);
        assert_eq!(one.len(), 1);
        assert!(one[0].text.contains("V2 key isn't in chimera.local.toml") && !one[0].fatal, "{one:?}");
        assert_eq!((one[0].game, one[0].install), ("combat_arms", false), "a key is a note, not the install");
        assert!(super::missing(Some(&dir), Some(&dir), &both).is_empty());
        let (text, fatal) = super::report(&nothing);
        assert!(fatal && text.starts_with("error: Counter-Strike") && text.contains("\nwarning: Combat Arms"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A tester with no install and no config reaches the menu and can
    /// join: no missing install stops the game (a server offering its game
    /// files provides them, `net::assets`); each still says what is missing
    /// and how to get it, signed in or not.
    #[test]
    fn no_install_never_stops_the_game() {
        for service in [false, true] {
            let out = super::served_by_servers(super::missing(None, None, &[]), service);
            assert_eq!(out.len(), 2);
            assert!(out.iter().all(|m| !m.fatal), "{out:?}");
            assert!(out[0].text.contains("Counter-Strike: Source isn't installed"));
            if service {
                // The files come from the service: no config-file hint.
                for m in &out {
                    assert!(m.text.contains("asset service") && !m.text.contains("chimera.local.toml"), "{}", m.text);
                }
            } else {
                assert!(out[0].text.contains("+chimera:hub <invite link>"), "{}", out[0].text);
                assert!(out[0].text.contains("chimera.local.toml"), "the plain warning stays");
            }
            let (text, fatal) = super::report(&out);
            assert!(!fatal && text.starts_with("warning: Counter-Strike"), "{text}");
        }
        // A missing key isn't an install: left as it was.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/party_roster_served_test");
        std::fs::create_dir_all(dir.join("cstrike")).unwrap();
        std::fs::create_dir_all(dir.join("Game")).unwrap();
        let keys = super::served_by_servers(super::missing(Some(&dir), Some(&dir), &["V1"]), false);
        assert!(keys[0].text.ends_with("look wrong."), "{keys:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn both_games_have_loaders_and_combat_arms_dresses() {
        let s = &super::MAP_SOURCES;
        assert!(s.loader("cs_source").is_some() && s.loader("combat_arms").is_some());
        assert_eq!(s.gameplay.map(|g| g.game), Some(combat_arms::game::GAME));
    }

    /// Halo is composed for its maps: its loader, its capture the flag
    /// teams as Combat Arms' two, its surfaces through the Source table.
    #[test]
    fn halo_lends_its_maps() {
        let s = &super::MAP_SOURCES;
        assert!(s.loader(halo_ce::game::GAME).is_some());
        assert!(super::BUNDLES.iter().any(|b| b.info.id == halo_ce::game::GAME));
        assert!(super::GAME_PARTS.iter().any(|p| p.game == halo_ce::game::GAME));
        let teams: Vec<(u8, u8)> =
            s.teams.iter().filter(|t| t.game == halo_ce::game::GAME).map(|t| (t.from, t.to)).collect();
        use combat_arms::game::rules::state::{ALPHA, BRAVO};
        assert_eq!(teams, [(1, ALPHA), (2, BRAVO)]);
        assert!(s.surfaces.iter().any(|t| t.map == halo_ce::game::GAME && t.gameplay == combat_arms::game::GAME));
    }
}
