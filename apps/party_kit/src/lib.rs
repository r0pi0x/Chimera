//! The party kit (apps/party_kit, crate `party_kit`; docs/plans/active/
//! architecture-review.md L2, ultimate-gun-game.md gap 1): what Lucker Party
//! (apps/lucker_party, apps/lucker_party_server) and the Ultimate Gun Game
//! (apps/ultimate_gun_game, apps/ultimate_gun_game_server) share, so the four
//! apps are thin binaries and none depends on another app. It names no
//! game (docs/plans/active/architecture-for-iteration.md, stage 3;
//! checks/tests/it/architecture.rs, `party_kit_names_no_game`): the games
//! come in a `Party`, which the roster (apps/party_roster, `party_roster::
//! PARTY`) composes and the apps hand in, so a game's edit rebuilds the
//! roster, not the kit. Minigames, each a data file (the party's
//! `minigames`), played on Chimera's mode runtime (`chimera_sim::minigame`)
//! with every game's parts. A minigame with `identity = "weapon"` (the gun
//! game) makes each player the game of the gun in hand
//! (`chimera_sim::identity`): its movement and its body.
//!
//! - `Party`: the base game's bundle (whose gameplay, bots and HUD
//!   everything plays on: Combat Arms), the guests' (CS:S, HL2:DM, Halo,
//!   TF2), the map sources, the built-in minigames, and hooks for the
//!   party's own parts (`sim_parts`, `sim_rules`, `requirements`). What a
//!   game brings is its bundle's: the base game's simulation, rules and
//!   bots as a base (`GameBundle::base`), each game's view-model cvar.
//!   `Roster` holds it in the app.
//! - `PartyPlugins(&Party)`: the simulation: the base game's
//!   (`BaseGame::plugins`), then `PartyPartsPlugin(&Party)`: each game's
//!   parts registered, each guest's own simulation (its bundle's `sim`:
//!   its guns and their impacts, movement, view-model rules), the view
//!   models' fields of view, the cross-game balance, the minigame runtime
//!   (`MinigamePlugin`), its part of the network (`chimera_net::
//!   minigame_net`), the arena, each game's binds and the party's
//!   options. Every game's ragdolls and bodies go when their owner
//!   respawns (`map::ragdoll::BodiesGoOnRespawn`, for performance), over
//!   each game's own lifetime. The app's gameplay is the base game's until
//!   a minigame starts (`minigame <id>`), which takes the rules slot and
//!   names the others.
//! - `minigame_of`, `Party::first_map`: the minigame a command line asks
//!   for and its pool's first map, which the app and the server load
//!   unless the command line names a map.
//! - With `present` (the default; not the dedicated server's): the client
//!   app's game-free parts (`client`: the shell with every game's HUD,
//!   `with_huds`, the local player's game's drawn; the party's front end,
//!   `client::front_end`; `with_defaults_for`, the command line as an app
//!   plays it), which the roster's `client::PARTY` (a `GameApp`) uses.
//! - CS:S's radar, target ID, freeze cam and voice HUD (`client::with_huds`)
//!   draw only while CS:S's HUD is the gameplay's (a CS:S gun in hand);
//!   the radars (CS:S's and Combat Arms') only with `hud_radar 1`, and
//!   one kill feed, the shell's, for both HUDs (`PartyHudPlugin`).
//! - The Tab scoreboard (`chimera_client::client::minigame_scoreboard`):
//!   one for every minigame, whatever game's HUD is up.
//! - Both games' parts registered (`chimera_core::game_parts`, each game's
//!   `parts::PARTS`): what the Ultimate Gun Game builds its ladder from.
//!
//! - `arena`: a slice of a big map (`arena halo_ce:bloodgulch@west_base`):
//!   the match on a part of the map, behind a wall.
//! - Bots: `MaxPlayer` is how many the server keeps (none for a joiner:
//!   `+connect` skips the app's default bots and its local minigame, the
//!   server's comes over the network: `chimera_net::minigame_net`).
//! - `Missing`, `report`, `served_by_servers`: what this machine lacks to
//!   play (`Party::requirements`), told exactly at start by both binaries.
//!   `mount_report`: the same as the network's mount doctor report
//!   (`net::manifest::MountReport`), which the client sends when it joins
//!   (`LocalMounts`, set by the client's front end).
//! - Party options, off by default (parity first): `party_even_footsteps`
//!   (one footstep profile for both games' identities, CS:S's; the
//!   server's, replicated) and, on the client, `party_swap_effect` (a
//!   short flash on a rung's swap and a puff at others' bodies:
//!   `chimera_client::client::swap_effect`). On by default (the user's
//!   playtest ask, 2026-11-14): `party_voice_lines_3d` (the player's,
//!   in their userinfo: others' grenade and kill call-outs and radio
//!   calls their game gives only to them or their team, heard in 3D at
//!   them; `chimera_world::map::sound::PartyVoices`, `voice_lines`).
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
//! minigame's map pool isn't rotated (its first map plays); the base game
//! is one, named by the roster (its loading screen, menu music and bots
//! come as the roster's hooks, not yet as `GameBundle`/`GameClient` hooks
//! every game could fill: architecture-pass.md step 9); the pairs' spawn
//! teams and surfaces are the roster's tables, not a neutral vocabulary;
//! `game_options` reads the first party installed into a process-wide
//! cache.

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_core::{
    game_data::GameData,
    game_parts::GamePartsAppExt,
    games_in_play::{GateGameExt as _, GamesInPlay},
};
use chimera_sim::minigame::{MinigameDef, MinigamePlugin};
use chimera_world::{
    game_bundle::GameBundle,
    map::{MapData, MapLoadOptions, MapSources},
};

/// This build's version (`version`, bug reports): stamped by build.rs.
pub const VERSION: &str = env!("CHIMERA_VERSION");

/// This build's commit (`git describe`): stamped by build.rs.
pub const BUILD: &str = env!("CHIMERA_GIT");

/// What a party is made of, handed to the kit by the app that composes
/// the games (apps/party_roster's `PARTY`): the kit names no game
/// (checks/tests/it/architecture.rs, `party_kit_names_no_game`), so a
/// game's edit doesn't rebuild it and a new game doesn't touch it. Plain
/// data and function pointers, like the games' own descriptors
/// (`GameBundle`, `GameClient`).
pub struct Party {
    /// The base game: whose gameplay, bots and HUD everything plays on.
    pub base: &'static GameBundle,
    /// The games composed besides the base, in the order an automatic
    /// ladder takes them.
    pub guests: &'static [&'static GameBundle],
    /// Every game: the guests, then the base.
    pub bundles: &'static [&'static GameBundle],
    /// Where the party's maps come from and whose gameplay dresses them.
    pub map_sources: &'static MapSources,
    /// The built-in minigames (file name, text), and the one played
    /// without `+minigame`.
    pub minigames: &'static [(&'static str, GameData)],
    pub default_minigame: &'static str,
    /// The party's own parts, after the guests' (`GameBundle::sim`) and
    /// the balance: the practice targets' bodies.
    pub sim_parts: fn(&mut App),
    /// After the minigame runtime, before the base game's own setting
    /// for the party (`BaseGame::compose`): the built-in minigames'
    /// plugin, the host's surprises.
    pub sim_rules: fn(&mut App),
    /// What this machine lacks to play (`Missing`), read from it.
    pub requirements: fn() -> Vec<Missing>,
}

impl Party {
    /// The base game as a base (`GameBundle::base`).
    pub fn base_game(&self) -> &'static chimera_world::game_bundle::BaseGame {
        self.base.base.unwrap_or_else(|| panic!("{} can't be a party's base game (its bundle's `base`)", self.base.info.id))
    }

    /// What each composed game lends (`chimera_core::game_parts`).
    pub fn parts(&self) -> impl Iterator<Item = &'static chimera_core::game_parts::GameParts> + '_ {
        self.bundles.iter().map(|b| b.parts)
    }

    /// Each game's own binds over the shared ones (each bundle's `binds`;
    /// the base game's first, games without any left out).
    pub fn binds(&self) -> impl Iterator<Item = (&'static str, &'static [(&'static str, &'static str)])> + '_ {
        std::iter::once(self.base).chain(self.guests.iter().copied()).filter(|b| !b.binds.is_empty()).map(|b| (b.info.id, b.binds))
    }

    /// Map `id` (`cs_source:de_dust2`, `combat_arms:warehouse`).
    pub fn load_map(&self, id: &str, hdr_level: u8) -> Result<MapData, String> {
        self.map_sources.load(id, hdr_level)
    }

    /// Map `id` with `options` (the dedicated server's: no textures).
    pub fn load_map_with(&self, id: &str, options: &MapLoadOptions) -> Result<MapData, String> {
        self.map_sources.load_with(id, options)
    }

    /// The built-in minigames, read.
    fn minigame_defs(&self) -> impl Iterator<Item = MinigameDef> + '_ {
        self.minigames.iter().filter_map(|(file, text)| MinigameDef::parse(file, text).ok())
    }

    /// The first map of built-in minigame `id`'s pool.
    pub fn first_map(&self, id: &str) -> Result<String, String> {
        self.minigame_defs()
            .find(|d| d.id == id)
            .map(|d| d.maps[0].clone())
            .ok_or_else(|| format!("no minigame `{id}` (the built-in ones: {})", self.ids().join(", ")))
    }

    /// The built-in minigames' ids.
    pub fn ids(&self) -> Vec<String> {
        self.minigame_defs().map(|d| d.id).collect()
    }

    /// The game `id`'s bundle, if the party composes it.
    pub fn bundle(&self, id: &str) -> Option<&'static GameBundle> {
        self.bundles.iter().copied().find(|b| b.info.id == id)
    }
}

/// The party an app runs (`Party`), for the kit's systems: inserted by
/// `PartyPartsPlugin`.
#[derive(Resource, Clone, Copy)]
pub struct Roster(pub &'static Party);

/// The simulation: the base game's (`BaseGame::plugins`), then
/// `PartyPartsPlugin`.
pub struct PartyPlugins(pub &'static Party);

impl PluginGroup for PartyPlugins {
    fn build(self) -> PluginGroupBuilder {
        (self.0.base_game().plugins)().add(PartyPartsPlugin(self.0))
    }
}

/// Each game's guns at the game's own view-model field of view, for a
/// player of that game (each bundle's `view_model_fov`), and
/// `r_drawviewmodel` for all, and each game's own cvar for it (its
/// bundle's `view_model_fov_cvar`; all saved in the config). After the
/// base game's view model settings are in.
pub fn add_view_model_fovs(party: &Party, app: &mut App) {
    let fovs = [party.base].iter().chain(party.guests.iter()).filter_map(|b| b.view_model_fov.map(|f| f())).collect();
    app.insert_resource(chimera_world::map::GameViewModelFovs(fovs));
    for cvar in [party.base].iter().chain(party.guests.iter()).filter_map(|b| b.view_model_fov_cvar) {
        cvar(app);
    }
    // Every game's view model hidden or shown (screenshots of a map
    // without a gun in hand), as CS:S's own console has it.
    chimera_core::console::resource_cvar::<chimera_world::map::ViewModelSettings, u8>(
        app,
        "r_drawviewmodel",
        "0: no view model.",
        |s| &mut s.draw,
    );
}

/// What the party adds to the base game's simulation: each game's parts
/// registered (the capability reports: `chimera_sim::minigame::auto`),
/// each guest's simulation (its bundle's `sim`: its guns, impacts,
/// movement, view-model rules and motion, on Chimera's generic driver
/// with its game's rules), the cross-game balance, the minigame runtime
/// and its part of the network, a slice of a big map, each game's binds,
/// the party's options, the party's own parts (`Party::sim_parts`,
/// `sim_rules`) and the base game set for the party (`BaseGame::compose`,
/// `finish`).
pub struct PartyPartsPlugin(pub &'static Party);

impl Plugin for PartyPartsPlugin {
    fn build(&self, app: &mut App) {
        let party = self.0;
        app.insert_resource(Roster(party));
        game_options::install(party);
        for parts in party.parts() {
            app.register_parts(parts);
        }
        // Each guest's systems run only while its game is in play
        // (`chimera_core::games_in_play`): a match plays a few of the
        // games composed. The base game is always in play.
        for bundle in party.guests {
            app.gate_game(bundle.game(), bundle.sim);
        }
        if let Some(mut games) = app.world_mut().get_resource_mut::<GamesInPlay>() {
            games.bare_maps = party.map_sources.loaders.first().map(|l| l.game);
        }
        chimera_sim::in_play::watch(app);
        add_view_model_fovs(party, app);
        // The cross-game balance and its switch (data/party_balance.toml).
        balance::add(app);
        (party.sim_parts)(app);
        app.add_plugins(MinigamePlugin);
        (party.sim_rules)(app);
        (party.base_game().compose)(app);
        app.add_plugins((
            // The match and the ladder over the network (a part: nothing
            // runs without a `NetPlugin`).
            chimera_net::minigame_net::MinigameNetPlugin,
            // A slice of a big map (`arena <map@slice>`).
            arena::ArenaPlugin,
        ));
        if let Some(mut console) = app.world_mut().get_resource_mut::<chimera_core::console::Console>() {
            for (game, binds) in party.binds() {
                console.ship_layer(game, binds);
            }
        }
        even_footsteps(app);
        voice_lines(app);
        // Every game's ragdolls and bodies go when their owner respawns
        // (performance; a previous rung's character's too), over each
        // game's own lifetime (TF2's timer, HL2:DM's until the next death).
        app.insert_resource(chimera_world::map::ragdoll::BodiesGoOnRespawn);
        // The host's settings and the skip-map vote (the Escape menu's).
        host::build(app);
    }

    fn finish(&self, app: &mut App) {
        host::finish(app);
        (self.0.base_game().finish)(app);
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

/// `party_voice_lines_3d` on by default here (Chimera registers it at 0,
/// the standalone games' parity): a player hears others' voice lines in
/// 3D at them (`chimera_world::map::sound::PartyVoices`).
fn voice_lines(app: &mut App) {
    use chimera_world::map::sound::{PARTY_VOICES, PartyVoices};
    app.insert_resource(PartyVoices { on: 1 });
    if let Some(mut console) = app.world_mut().get_resource_mut::<chimera_core::console::Console>() {
        console.set_default(PARTY_VOICES, "1");
    }
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
    /// The game it is of, and that game's name in words.
    pub game: &'static str,
    pub name: &'static str,
    /// The install itself is missing (or isn't the game's); else a part of
    /// it (a key).
    pub install: bool,
}

/// What a client lacks (`requirements`), told: a missing install never
/// stops it (`served_by_servers`), so the menu and Join work with no
/// install and no config. A server's lacks stay fatal (the party's
/// `server::requirements`).
pub fn client_requirements(party: &Party) -> (String, bool) {
    use chimera_core::mount::config::LocalConfig;
    let service = chimera_net::net::assets::has_token()
        || LocalConfig::load().is_ok_and(|c| c.assets.resolve().remote.is_some());
    report(&served_by_servers((party.requirements)(), service))
}

/// A missing install never stops the game: a server that offers its game
/// files (`net::assets`) provides what isn't installed when the player
/// joins, so the menu and Join work with no install and no config. The
/// lines stay as warnings, saying how. `service`: signed in to a hub, a
/// token entered or an asset service configured: then the lines say the files come
/// from the service, without the install hints (the config file).
pub fn served_by_servers(mut out: Vec<Missing>, service: bool) -> Vec<Missing> {
    for m in out.iter_mut().filter(|m| m.install) {
        if service {
            m.fatal = false;
            m.text = format!("{} isn't installed here: its files come from the asset service when you join.", m.name);
        } else if m.fatal {
            m.fatal = false;
            m.text.push_str(
                " A server that offers its game files can provide them: sign in with your hub invite (the first \
                 run, or +chimera:hub <invite link>), then join it.",
            );
        }
    }
    out
}

/// The mount doctor's report for the network (`net::manifest`): each
/// registered game's install (`manifest::examine`), with what `requirements`
/// finds: an install that isn't the game's counts as missing, a missing key
/// is a note on its game's line.
pub fn mount_report(party: &Party, games: &chimera_core::game_registry::Games) -> chimera_net::net::manifest::MountReport {
    let mut report = chimera_net::net::manifest::examine(games);
    for m in (party.requirements)() {
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

pub mod arena;
pub mod balance;
pub mod game_options;
pub mod host;
pub mod server;
#[cfg(feature = "present")]
pub mod controls;
#[cfg(feature = "present")]
pub mod esc_menu;
#[cfg(feature = "present")]
pub mod esc_signal;
#[cfg(feature = "present")]
pub mod feedback_signal;
#[cfg(feature = "present")]
pub mod party_signal;
#[cfg(feature = "present")]
pub mod crash_signal;
#[cfg(feature = "present")]
pub mod onboarding;
#[cfg(feature = "present")]
pub mod account_page;
#[cfg(feature = "present")]
pub mod base_download;
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
pub mod server_list;
#[cfg(feature = "present")]
pub mod signal_pages;
#[cfg(feature = "present")]
pub mod guns_picker;
#[cfg(feature = "present")]
pub mod maps_picker;
#[cfg(feature = "present")]
pub mod loading_signal;
#[cfg(feature = "present")]
pub mod client;
#[cfg(feature = "present")]
pub mod ui_cues;
#[cfg(feature = "present")]
pub use client::{PartyHudPlugin, joins_server, start_at_menu, with_defaults_for};

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
    fn a_command_line_names_its_minigame() {
        let lines = ["MaxPlayer 3".to_string(), "minigame originals:gun_game_dust2".to_string()];
        assert_eq!(super::minigame_of(&lines).as_deref(), Some("originals:gun_game_dust2"));
        assert_eq!(super::minigame_of(&lines[..1]), None);
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
