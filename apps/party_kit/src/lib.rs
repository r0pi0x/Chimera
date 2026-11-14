//! The party kit (apps/party_kit, crate `party_kit`; docs/plans/active/
//! architecture-review.md L2, ultimate-gun-game.md gap 1): what Lucker Party
//! (apps/lucker_party, apps/lucker_party_server) and the Ultimate Gun Game
//! (apps/ultimate_gun_game, apps/ultimate_gun_game_server) share, so the four
//! apps are thin binaries and none depends on another app. Minigames,
//! each a data file (`minigames/`, the `originals` crate) played on
//! Chimera's mode runtime (`chimera_sim::minigame`), with both games' parts:
//! Combat Arms' soldiers, movement, HUD and guns, CS:S's maps, map logic,
//! movement and guns, on CS:S's or Combat Arms' maps. A minigame with
//! `identity = "weapon"` (the gun game) makes each player the game of the
//! gun in hand (`chimera_sim::identity`): its movement and its body (CS:S's
//! player models stay on CS:S's maps for it, `MapSources::players`; CS:S's
//! per-tick body animation for a network server's hitboxes). The games are libraries composed
//! here and nowhere else (checks/tests/it/architecture.rs), as in the
//! mashup demo.
//!
//! - `PartyPlugins`: the simulation: Combat Arms' (`combat_arms::
//!   SimPlugins`, `CombatArmsPlugins`: the default game, for bare map
//!   names), then `PartyPartsPlugin`: CS:S registered as a second game,
//!   Source's map logic, CS:S's guns and their impacts (sounds, decals,
//!   debris, blood, tracers; `weapon::impact` routes each impact to one
//!   game; its grenades' explosion and smoke looks; props' collision and scrape sounds; the hit slowdown and view
//!   flinch of a CS:S-identity player), the minigame runtime
//!   (`MinigamePlugin`), its part of the network (`chimera_net::
//!   minigame_net`) and the original minigames (`OriginalsPlugin`).
//!   The app's gameplay is Combat Arms' until a minigame starts
//!   (`minigame <id>`, `+minigame originals:gun_game_dust2` on the command
//!   line), which takes the rules slot and names the others.
//! - `MAP_SOURCES`: both games' map loaders with Combat Arms' gameplay
//!   dressing, CS:S's terrorist and counter-terrorist spawns as Alpha's and
//!   Bravo's, CS:S's surfaces as Combat Arms' through the mashup's table
//!   (apps/mashup_demo/data/surfaces_cs_source_combat_arms.toml, read from
//!   there: one table for both apps) and CS:S's guns' models kept. On
//!   Combat Arms' maps CS:S is a guest (`CS_SOURCE_PARTS`,
//!   `MapSources::guests`): its player bodies, guns' held and view models
//!   and HUD look are read from the CS:S install and added to the map.
//! - `minigame_of`, `first_map`: the minigame a command line asks for
//!   (`DEFAULT_MINIGAME` without one) and its pool's first map, which the
//!   app and the server load unless the command line names a map.
//! - With `present` (the default; not the dedicated server's,
//!   `server`): `client::PARTY`, the whole client app as Chimera's
//!   `GameApp` (Lucker Party's `client::APP`; the Ultimate Gun Game's,
//!   apps/ultimate_gun_game, is built on it): the shell with this app's
//!   hooks and both games' HUDs (`with_huds`, the local player's game's
//!   drawn), the party's front end (`client::front_end`), and
//!   `with_defaults` (the command line as the app plays it;
//!   `with_defaults_for` another app's minigame).
//! - CS:S's radar, target ID, freeze cam and voice HUD (`client::with_huds`)
//!   draw only while CS:S's HUD is the gameplay's (a CS:S gun in hand);
//!   the radars (CS:S's and Combat Arms') only with `hud_radar 1`, and
//!   one kill feed, the shell's, for both HUDs (`PartyHudPlugin`).
//! - The Tab scoreboard (`chimera_client::client::minigame_scoreboard`):
//!   one for every minigame, whatever game's HUD is up.
//! - Both games' parts registered (`chimera_core::game_parts`, each game's
//!   `parts::PARTS`): what the Ultimate Gun Game builds its ladder from.
//!
//! - `surprise`: the host's surprises on the console (`surprise airstrike`,
//!   `surprise gravity`), from what any client already draws.
//! - Bots: Combat Arms' (the gun game's `bots` slot), with CS:S's guns'
//!   types for their AI rows (data/bot_gun_types.toml), their generated
//!   meshes for the widest hull they may wear (`WIDEST_HULL`); `MaxPlayer` is how
//!   many the server keeps (none for a joiner: `+connect` skips the app's
//!   default of `DEFAULT_BOTS` and its local minigame, the server's comes
//!   over the network: `chimera_net::minigame_net`).
//! - `requirements`, `missing`, `report`: what this machine lacks to play
//!   (CS:S's install, Combat Arms' install and its V1 and V2 keys), told
//!   exactly at start by both binaries; CS:S's missing stops them.
//!   `mount_report`: the same as the network's mount doctor report
//!   (`net::manifest::MountReport`), which the client sends when it joins
//!   (`LocalMounts`, set by the client's front end).
//! - Party options, off by default (parity first): `party_even_footsteps`
//!   (one footstep profile for both games' identities, CS:S's; the
//!   server's, replicated) and, on the client, `party_swap_effect` (a
//!   short flash on a rung's swap and a puff at others' bodies:
//!   `chimera_client::client::swap_effect`).
//!
//! - The host's settings and the vote to skip the map (`host`): the
//!   Escape menu's server side (`esc_menu`, with `present`).
//! - The main menu (`main_menu`, with `present`): Ultimate Gun Game starts
//!   at it (`start_at_menu`) unless the command line plays; Lucker
//!   Party doesn't use it.
//!
//! - A gun game's rung extras (`chimera_sim::minigame::ladder::RungExtras`:
//!   Combat Arms' rungs' fitted parts, character and melee weapon and CS:S's rungs' player model,
//!   `combat_arms::game::rungs`) are seeded anew at every launch of the
//!   apps (`launch_rung_seed`; `CHIMERA_RUNG_SEED=<n>` fixes it); the plugins alone (the tests) keep seed 0.
//!
//! Known shortcuts: maps load by id only (no downloaded map files); the
//! minigame's map pool isn't rotated (its first map plays); the bots fight
//! with whatever their ladder gives but keep the reach of their first
//! life's weapons; Combat Arms' guns show their ids as names (no item
//! table yet: BERETTA_M92FS). The HUD shows the minigame's state
//! (`chimera_sim::minigame::hud`: rung, next gun, leader, time, notices),
//! CS:S's guns' names from CS:S's strings (`weapons::display_names`) and
//! their view models by Chimera's driver with CS:S's rules
//! (`PartyPartsPlugin`), on CS:S's maps only.

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_core::game_parts::GamePartsAppExt;
use chimera_sim::minigame::{MinigameDef, MinigamePlugin};
use chimera_world::{
    game_bundle::GameBundle,
    map::{MapData, MapDressing, MapLoadOptions, MapSources, SpawnTeams, SurfaceTable},
};

/// This build's version (`version`, bug reports): stamped by build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

/// This build's commit (`git describe`): stamped by build.rs.
pub const BUILD: &str = env!("CHIMERA_GIT");

/// The minigame played without `+minigame`.
pub const DEFAULT_MINIGAME: &str = originals::GUN_GAME_DUST2;

/// The games the party composes besides its base game (Combat Arms,
/// whose gameplay, bots and HUD everything plays on), each one bundle
/// (`chimera_world::game_bundle`, lift C4): CS:S and HL2:DM, in the order
/// an automatic ladder takes them. The kit loops over these; only the
/// base game and the pairs' data (spawn teams, surface names) are named.
pub static GUESTS: [&GameBundle; 2] = [&cs_source::game::BUNDLE, &hl2dm::game::BUNDLE];

/// The base game's bundle.
pub static BASE: &GameBundle = &combat_arms::game::BUNDLE;

/// Every game the party composes: the guests, then the base.
pub static BUNDLES: [&GameBundle; 3] = [GUESTS[0], GUESTS[1], BASE];

/// CS:S's spawn teams as Combat Arms': terrorists (1) Alpha, counter-
/// terrorists (2) Bravo.
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

/// The Source games' surfaces (CS:S's and HL2:DM's: Source's names and
/// material letters) as Combat Arms' surface types: the mashup's table.
pub const SURFACES: &[SurfaceTable] = &[
    SurfaceTable {
        map: cs_source::game::GAME,
        gameplay: combat_arms::game::GAME,
        text: include_str!("../../mashup_demo/data/surfaces_cs_source_combat_arms.toml"),
    },
    SurfaceTable {
        map: hl2dm::game::GAME,
        gameplay: combat_arms::game::GAME,
        text: include_str!("../../mashup_demo/data/surfaces_cs_source_combat_arms.toml"),
    },
];

/// What of the guests' parts stays on their own maps: their guns' held
/// and view models.
pub static KEPT: [&str; 2] = [GUESTS[0].kept[0], GUESTS[1].kept[0]];

/// The guests' players, guns and HUD looks on the maps of other games.
pub static GUEST_DRESSING: [&MapDressing; 2] = [GUESTS[0].guest.unwrap(), GUESTS[1].guest.unwrap()];

/// Where this app's maps come from (a bare map name is the first guest's,
/// CS:S's) and whose gameplay dresses them.
pub static MAP_SOURCES: MapSources = MapSources {
    loaders: &[GUESTS[0].loader, GUESTS[1].loader, BASE.loader],
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

/// Map `id` (`cs_source:de_dust2`, `combat_arms:warehouse`) for the party.
pub fn load_map(id: &str, hdr_level: u8) -> Result<MapData, String> {
    MAP_SOURCES.load(id, hdr_level)
}

/// Map `id` with `options` (the dedicated server's: no textures).
pub fn load_map_with(id: &str, options: &MapLoadOptions) -> Result<MapData, String> {
    MAP_SOURCES.load_with(id, options)
}

/// What each composed game lends (its capability claim, `chimera_core::
/// game_parts`), in the order an automatic ladder takes them.
pub static GAME_PARTS: [&chimera_core::game_parts::GameParts; 3] = [BUNDLES[0].parts, BUNDLES[1].parts, BUNDLES[2].parts];

/// The simulation: Combat Arms' (the engine's base, then its parts, the
/// default game), then `PartyPartsPlugin`.
pub struct PartyPlugins;

impl PluginGroup for PartyPlugins {
    fn build(self) -> PluginGroupBuilder {
        combat_arms::game::CombatArmsPlugins
            .build()
            .add_before::<combat_arms::game::data::CaDataPlugin>(combat_arms::SimPlugins)
            .add(PartyPartsPlugin)
    }
}

/// Each game's own binds, over the shared ones (`Console::layers`, applied
/// by the local input to a player whose identity is that game): where the
/// games disagree about a key (each bundle's `binds`). Combat Arms: Shift
/// sprints, C walks (Alt is free look in Chimera). CS:S: Shift walks
/// (`+speed`), Ctrl ducks. HL2:DM: Shift sprints (`+speed`), Ctrl ducks,
/// Alt walks, Z zooms the suit (`+zoom`; CS:S's Z is its radio, a game's own layer). The shared binds are the default ones (the gameplay game's);
/// the Controls page (`controls`) edits each.
pub static GAME_BINDS: [(&str, &[(&str, &str)]); 3] = [
    (BUNDLES[2].info.id, BUNDLES[2].binds),
    (BUNDLES[0].info.id, BUNDLES[0].binds),
    (BUNDLES[1].info.id, BUNDLES[1].binds),
];

/// The widest player the party's games make (half, m): Source's, 16 u.
pub const WIDEST_HULL: f32 = 16.0 * 0.0254;

/// Combat Arms' bots' gun types for CS:S's guns (data/bot_gun_types.toml).
pub const BOT_GUN_TYPES: &str = include_str!("../data/bot_gun_types.toml");

/// Bots the app keeps without a `MaxPlayer` of the user's: a gun game of
/// four with the player.
pub const DEFAULT_BOTS: u32 = 3;

/// Each game's guns at the game's own view-model field of view, for a
/// player of that game (each bundle's `view_model_fov`: `viewmodel_fov` is
/// CS:S's, `ca_viewmodel_fov` Combat Arms', both saved in the config;
/// HL2:DM's 54 at its 75). After Combat Arms' view model settings are in.
pub fn add_view_model_fovs(app: &mut App) {
    let fovs = [BASE].iter().chain(GUESTS.iter()).filter_map(|b| b.view_model_fov.map(|f| f())).collect();
    app.insert_resource(chimera_world::map::GameViewModelFovs(fovs));
    cs_source::game::view_anim::viewmodel_fov_cvar(app);
}

/// What the party adds to Combat Arms' simulation (the headless tests run
/// it on `combat_arms::harness::CaSim`), after Combat Arms' parts: CS:S
/// registered as a second game (its install and maps), Source's map
/// logic, CS:S's guns, the minigame runtime and the original minigames;
/// Combat Arms' modes off (`ca_mode none`: its rules leave the players
/// to the minigame, a joiner comes in at a free spawn point) and its bots
/// given CS:S's guns' types (`BOT_GUN_TYPES`) and, by their data's class,
/// those of the other guns a ladder may hand out (`bot_gun_type_of`).
pub struct PartyPartsPlugin;

impl Plugin for PartyPartsPlugin {
    fn build(&self, app: &mut App) {
        // What each game lends a game made of several (the capability
        // reports: `chimera_sim::minigame::auto`).
        for parts in GAME_PARTS {
            app.register_parts(parts);
        }
        // Each guest's parts in the simulation (its bundle's `sim`: CS:S's
        // guns, impacts, movement, view-model rules and motion; HL2:DM's
        // guns, rung extras, view models). A guest's guns' view models
        // play on Chimera's generic driver with its game's rules (Combat
        // Arms' driver lets go of a character holding another game's gun).
        for bundle in GUESTS {
            (bundle.sim)(app);
        }
        add_view_model_fovs(app);
        app.add_plugins((
            MinigamePlugin,
            originals::OriginalsPlugin,
            // The match and the ladder over the network (a part: nothing
            // runs without a `NetPlugin`).
            chimera_net::minigame_net::MinigameNetPlugin,
            // The host's surprises (`surprise airstrike`, `surprise gravity`).
            surprise::SurprisePlugin,
        ));
        if let Some(mut s) = app.world_mut().get_resource_mut::<combat_arms::game::rules::CaRuleSettings>() {
            s.mode = "none".into();
        }
        if let Some(mut console) = app.world_mut().get_resource_mut::<chimera_core::console::Console>() {
            for (game, binds) in &GAME_BINDS {
                console.ship_layer(game, binds);
            }
        }
        if let Some(mut bots) = app.world_mut().get_resource_mut::<combat_arms::game::bots::BotData>() {
            bots.gun_types.extend(bot_gun_types());
        }
        // A bot wears the hull of its gun's game: a mesh for Combat Arms'
        // narrow one (15 u of 0.018 m) leads a Source hull into gaps it
        // doesn't fit, where it stands pressing on.
        if let Some(mut bots) = app.world_mut().get_resource_mut::<combat_arms::game::bots::BotSettings>() {
            bots.walker_radius = Some(WIDEST_HULL);
        }
        even_footsteps(app);
        // The host's settings and the skip-map vote (the Escape menu's).
        host::build(app);
    }

    fn finish(&self, app: &mut App) {
        host::finish(app);
        // The guns a ladder may hand out that the table doesn't name (Combat
        // Arms' launchers, its install's weapons): their AI row by class.
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
}

/// `party_even_footsteps` (the server's, replicated; default 0: each
/// game's steps as its own): 1 makes both games' others' steps fall off
/// with one profile, CS:S's (`chimera_world::map::sound::EvenFootsteps`),
/// so neither identity is heard farther. Measured in tests/it/
/// footstep_levels.rs.
fn even_footsteps(app: &mut App) {
    use chimera_world::map::sound::EvenFootsteps;
    app.init_resource::<EvenFootsteps>();
    if app.world().get_resource::<chimera_core::console::Console>().is_some() {
        chimera_core::console::resource_cvar::<EvenFootsteps, u8>(
            app,
            "chimera:party_even_footsteps",
            "1: every game's footsteps carry as far and as loud as CS:S's (one profile for both identities); 0: each game's own.",
            |s| &mut s.on,
        );
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
    let table: toml::Table = toml::from_str(BOT_GUN_TYPES).expect("data/bot_gun_types.toml reads");
    table
        .into_iter()
        .map(|(id, kind)| (id, kind.as_str().expect("a gun type's name").to_string()))
        .collect()
}

/// The minigame a command line's console lines ask for (`minigame <id>`,
/// from `+minigame <id>`), if any.
pub fn minigame_of(console: &[String]) -> Option<String> {
    console.iter().find_map(|line| {
        let mut words = line.split_whitespace();
        let first = words.next()?;
        first.eq_ignore_ascii_case("minigame").then(|| words.next().map(|w| w.trim_matches('"').to_string()))?
    })
}

/// Something this machine lacks to play Lucker Party, in words that say
/// what and how to get it (`requirements`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    /// Nothing loads without it (else the game runs, looking or sounding
    /// wrong).
    pub fatal: bool,
    pub text: String,
    /// The game it is of.
    pub game: &'static str,
    /// The install itself is missing (or isn't the game's); else a part of
    /// it (a key).
    pub install: bool,
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
        out.push(Missing { fatal, text, game, install })
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

/// What a client lacks (`requirements`), told: a missing install never
/// stops it (`served_by_servers`), so the menu and Join work with no
/// install and no config. A server's lacks stay fatal (`server::requirements`).
pub fn client_requirements() -> (String, bool) {
    use chimera_core::mount::config::LocalConfig;
    let service = chimera_net::net::assets::has_token()
        || LocalConfig::load().is_ok_and(|c| c.assets.resolve().remote.is_some());
    report(&served_by_servers(requirements(), service))
}

/// A missing install never stops the game: a server that offers its game
/// files (`net::assets`) provides what isn't installed when the player
/// joins, so the menu and Join work with no install and no config. The
/// lines stay as warnings, saying how. `service`: an asset token is set
/// or an asset service is configured: then the lines say the files come
/// from the service, without the install hints (the config file).
pub fn served_by_servers(mut out: Vec<Missing>, service: bool) -> Vec<Missing> {
    for m in out.iter_mut().filter(|m| m.install) {
        if service {
            m.fatal = false;
            let name = if m.game == cs_source::game::GAME { "Counter-Strike: Source" } else { "Combat Arms" };
            m.text = format!("{name} isn't installed here: its files come from the asset service when you join.");
        } else if m.fatal {
            m.fatal = false;
            m.text.push_str(
                " A server that offers its game files can provide them: start the game once with \
                 +chimera:asset_token <token> (from the server's admin), then join it.",
            );
        }
    }
    out
}

/// The mount doctor's report for the network (`net::manifest`): each
/// registered game's install (`manifest::examine`), with what `requirements`
/// finds: an install that isn't the game's counts as missing, a missing key
/// is a note on Combat Arms' line.
pub fn mount_report(games: &chimera_core::game_registry::Games) -> chimera_net::net::manifest::MountReport {
    let mut report = chimera_net::net::manifest::examine(games);
    for m in requirements() {
        if let Some(g) = report.games.iter_mut().find(|g| g.game == m.game) {
            if m.install {
                g.installed = false;
            } else {
                g.notes.push(m.text);
            }
        }
    }
    report
}

/// `requirements` told to the user: each on its own line (`error:` for what
/// stops the game, `warning:` for what only spoils it), and whether any
/// stops it.
pub fn report(problems: &[Missing]) -> (String, bool) {
    let text = problems
        .iter()
        .map(|m| format!("{}: {}", if m.fatal { "error" } else { "warning" }, m.text))
        .collect::<Vec<_>>()
        .join("\n");
    (text, problems.iter().any(|m| m.fatal))
}

/// The first map of built-in minigame `id`'s pool.
pub fn first_map(id: &str) -> Result<String, String> {
    originals::MINIGAMES
        .iter()
        .filter_map(|(file, text)| MinigameDef::parse(file, text).ok())
        .find(|d| d.id == id)
        .map(|d| d.maps[0].clone())
        .ok_or_else(|| format!("no minigame `{id}` (the originals: {})", ids().join(", ")))
}

/// The original minigames' ids.
pub fn ids() -> Vec<String> {
    originals::MINIGAMES
        .iter()
        .filter_map(|(file, text)| MinigameDef::parse(file, text).ok())
        .map(|d| d.id)
        .collect()
}

pub mod host;
pub mod spot_checks;
pub mod surprise;
pub mod server;
#[cfg(feature = "present")]
pub mod controls;
#[cfg(feature = "present")]
pub mod esc_menu;
#[cfg(feature = "present")]
pub mod esc_signal;
#[cfg(feature = "present")]
pub mod settings;
#[cfg(feature = "present")]
pub mod main_menu;
#[cfg(feature = "present")]
pub mod menu_backdrop;
#[cfg(feature = "present")]
pub mod menu_backdrops;
#[cfg(feature = "present")]
pub mod signal_menu;
#[cfg(feature = "present")]
pub mod client;
#[cfg(feature = "present")]
pub mod ui_cues;
#[cfg(feature = "present")]
pub use client::{
    PartyHudPlugin, PartyMousePlugin, client_hooks, joins_server, start_at_menu, with_defaults, with_defaults_for, with_huds,
};

#[cfg(test)]
mod tests {
    #[test]
    fn the_rung_seed_variable_is_a_number_or_nothing() {
        assert_eq!(super::parse_rung_seed(Some(" 42 ")), Some(42));
        assert_eq!(super::parse_rung_seed(Some("0")), Some(0));
        assert_eq!(super::parse_rung_seed(Some("abc")), None);
        assert_eq!(super::parse_rung_seed(Some("")), None);
        assert_eq!(super::parse_rung_seed(None), None);
    }

    #[test]
    fn the_build_is_stamped() {
        assert!(!super::BUILD.is_empty() && !super::VERSION.is_empty());
    }

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
        let dir = here.join("../../target/party_kit_missing_test");
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
    /// and how to get it, with or without an asset token.
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
                assert!(out[0].text.contains("+chimera:asset_token <token>"), "{}", out[0].text);
                assert!(out[0].text.contains("chimera.local.toml"), "the plain warning stays");
            }
            let (text, fatal) = super::report(&out);
            assert!(!fatal && text.starts_with("warning: Counter-Strike"), "{text}");
        }
        // A missing key isn't an install: left as it was.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/party_kit_served_test");
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
}

/// The environment variable that fixes a launch's rung seed (a number).
pub const RUNG_SEED_ENV: &str = "CHIMERA_RUNG_SEED";

/// The seed `text` names (`CHIMERA_RUNG_SEED`'s value): a whole number;
/// none for anything else, so a typo keeps the random launch seed.
pub fn parse_rung_seed(text: Option<&str>) -> Option<u64> {
    text?.trim().parse().ok()
}

/// `CHIMERA_RUNG_SEED`, when set to a number: the apps' fixed rung seed,
/// so the bodies, attachments and melee a ladder draws repeat from launch
/// to launch (before/after screenshots of one scene).
pub fn rung_seed_override() -> Option<u64> {
    parse_rung_seed(chimera_core::env::var(RUNG_SEED_ENV).ok().as_deref())
}

/// The seed for the ladders' rung extras: `CHIMERA_RUNG_SEED` when set
/// (`rung_seed_override`), else a fresh one (the clock's and the
/// process's, mixed): the apps' at launch.
pub fn launch_rung_seed() -> chimera_sim::minigame::ladder::RungSeed {
    if let Some(seed) = rung_seed_override() {
        return chimera_sim::minigame::ladder::RungSeed(seed);
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    chimera_sim::minigame::ladder::RungSeed(
        chimera_sim::minigame::auto::Rng::new(nanos ^ ((std::process::id() as u64) << 32)).next(),
    )
}
