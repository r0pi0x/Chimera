//! The client's side of Lucker Party (feature `present`): the party's
//! client app (`PARTY`, Lucker Party's `APP`, on Chimera's
//! `chimera_client::client::app`), the shell with this app's hooks and
//! both games' HUDs (`with_huds`:
//! Combat Arms' and CS:S's, the one drawn the local player's game, which
//! follows its gun in the gun game: `chimera_sim::identity`; each shows the
//! minigame's panel; CS:S's body animation for a player holding a CS:S
//! gun), the map files it can read for a server's map check
//! (`map_file_bytes`: CS:S's from its install, so a server on another map
//! than the one loaded is joined; Combat Arms' maps load by id) and the command line as the app plays it
//! (`with_defaults`: the default minigame and its first map). At start it
//! runs the mount doctor (`mount_report` into `net::manifest::LocalMounts`),
//! which a server checks its match against when this client joins.
//!
//! CS:S's HUD here is its health, armour and ammo panels, death notices,
//! damage sprites and weapon selection row (`ui::game_hud`,
//! `hud_sprites`, `weapon_select`), from CS:S's HUD look, HL2:DM's HUD from
//! its own (each map carries every game's, `MapSources::players` and
//! `guests`; up only with its game's HUD: `client::hud::HudLookOwner`), with
//! the shell's crosshair on CS:S's, and the minigame's panel
//! (`client::minigame_panel`). Over all: one kill feed, the
//! shell's, and no radars unless `hud_radar 1` (`PartyHudPlugin`); with
//! `party_swap_effect 1` (default 0) a short flash when the local
//! player's gun changes game, a puff at others' (`client::swap_effect`).
//! The Escape menu is the party's own (`esc_menu`), titled by the app;
//! `start_at_menu` starts an app at the main menu (`main_menu`) over the
//! first of its background maps that loads.
//! Known shortcuts: not CS:S's spectator panel or overview;
//! on Combat Arms' maps there is no CS:S HUD look (the shell's plain HUD
//! with CS:S's guns).

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_client::client::{
    AppHooks, Args,
    app::{AppMaps, GameApp, GameAppHooks, RunningApp, StartMap},
};
use chimera_core::game_registry::Games;
use combat_arms::ui::hud::ItemNames;

/// What this app tells the client shell: its commit, its maps
/// (`MAP_SOURCES`) and the greybox's own hooks (`PARTY`'s).
pub fn client_hooks() -> AppHooks {
    PARTY.app_hooks()
}

/// The bytes of map `id`'s file as this app loads it, for the network's
/// map check (`net::maps::MapFiles`): a CS:S map's, from the install. Combat
/// Arms' maps have no file of their own (the server offers them by id and
/// hash only: `net::maps`), so a client loads them by id. Without this a
/// client joining a server on a map other than the one it has loaded found
/// no copy, asked the server for a file the server doesn't have, and
/// stayed in its own greybox game with a minigame that nobody plays.
pub fn map_file_bytes(id: &str) -> Option<Vec<u8>> {
    let name = id.strip_prefix("cs_source:")?;
    let install = cs_source::game::INFO.install()?;
    cs_source::game::mount::open(&install).ok()?.read(&format!("maps/{name}.bsp")).ok()
}

/// The shell's plugins with every game's HUD (each bundle's `client`):
/// the base game's, Combat Arms' (`ui::with_hud`), then the guests':
/// CS:S's panels, sprites and weapon selection (whose logic is the shell's
/// `WeaponMenuPlugin`, added once, by CS:S's), the minigame's panel on
/// CS:S's HUD, and the rest of CS:S's HUD that follows the gun in hand
/// (the radar, target ID, freeze cam, voice status: each drawing only
/// while CS:S's HUD is the gameplay's), and HL2:DM's panels with the
/// minigame's (M9, drawing while its HUD is the gameplay's: on any map, its
/// look coming with the map, `MapSources::guests`).
pub fn with_huds(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    let mut shell = shell;
    for client in CLIENTS {
        if let Some(hud) = client.hud {
            shell = hud(shell);
        }
    }
    shell
}

/// Each game's front end (its `ui`'s `GameClient`, beside its bundle):
/// the base game's first, then the guests' (HL2:DM's: M9).
pub static CLIENTS: [&chimera_world::game_bundle::GameClient; 3] =
    [&combat_arms::ui::PARTY_CLIENT, &cs_source::ui::party::CLIENT, &hl2dm::ui::party::CLIENT];

/// The client after the shell with both games' HUDs (`PARTY`'s front
/// end; `with_huds` puts them among the shell's plugins, each drawing in
/// its own look, `HudLookOwner`): the key binds page, the party's Escape
/// menu, mouse and HUD settings, the minigame scoreboard, the swap
/// effect, the mount doctor and the guests' guns' names, each guest's
/// front end, then Combat Arms' other looks.
pub fn front_end(app: &mut App, _: &Args) {
    // The key binds page (F6, `controls`): shared binds and each
    // game's own.
    app.add_plugins(crate::controls::ControlsPlugin)
        // The party's own Escape menu (Combat Arms' yields Esc to it).
        .add_plugins(crate::esc_menu::EscMenuPlugin)
        .add_plugins(PartyMousePlugin)
        .add_plugins(PartyHudPlugin)
        .add_plugins(chimera_client::client::minigame_scoreboard::MinigameScoreboardPlugin)
        // `party_swap_effect` (default 0): a flash on a rung's swap.
        .add_plugins(chimera_client::client::swap_effect::SwapEffectPlugin)
        .add_systems(Startup, (guest_names, mount_doctor))
        .add_systems(Update, sync_gun_names);
    // The rest of each guest's front end (CS:S's: the map's HUD look
    // is its, and what its bodies play for a player holding its gun,
    // the gun game's `identity = "weapon"`).
    for client in CLIENTS {
        if let Some(add) = client.app {
            add(app);
        }
    }
    // Combat Arms' other looks.
    app.add_plugins(combat_arms::ui::CaUiPlugin);
}

/// The party's HUD settings over both games' HUDs: one kill feed, the
/// shell's (`client::kill_feed`, `SharedKillFeed`: the games' own feeds
/// stay empty), drawn in one place whichever game's HUD is up, so a kill
/// just before a rung swap is still in it after; and the games' radars
/// hidden (`hud_radar` starts at 0; `hud_radar 1` brings CS:S's radar and
/// Combat Arms' corner map back).
pub struct PartyHudPlugin;

impl Plugin for PartyHudPlugin {
    fn build(&self, app: &mut App) {
        use chimera_client::client::{hud, kill_feed};
        app.insert_resource(kill_feed::SharedKillFeed);
        if !app.is_plugin_added::<kill_feed::KillFeedStatePlugin>() {
            app.add_plugins(kill_feed::KillFeedStatePlugin);
        }
        hud::default_radars(app, false);
    }
}

/// The mouse look of the party: the shell's `sensitivity`, `m_yaw` and
/// `m_pitch` (archived, saved in config.cfg), for every game. Combat
/// Arms' options screen sets its own mouse look from its profile, which
/// the party does not keep; left to itself it set the shell's mouse at
/// start and again on every change, over what the config said (the
/// sensitivity was back to the default at each start). The starting
/// values are Combat Arms' defaults, as before, and they are the cvars'
/// defaults, so a config that says nothing keeps them and one that says
/// something wins.
pub struct PartyMousePlugin;

impl Plugin for PartyMousePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(combat_arms::ui::options_apply::ShellMouse)
            .add_systems(Startup, start_mouse);
    }
}

fn start_mouse(
    mouse: Option<ResMut<chimera_client::client::input::MouseSettings>>,
    console: Option<ResMut<chimera_core::console::Console>>,
) {
    let d = combat_arms::ui::options_apply::degrees_per_count(&combat_arms::game::options::CaMouseLook::default());
    if let Some(mut m) = mouse {
        m.sensitivity = 1.0;
        m.m_yaw = d;
        m.m_pitch = d;
    }
    if let Some(mut c) = console {
        c.set_default("sensitivity", "1");
        c.set_default("m_yaw", &d.to_string());
        c.set_default("m_pitch", &d.to_string());
    }
}

/// The mount doctor (`net::manifest`): what this machine has of each game,
/// which a server checks its match against when this client joins.
fn mount_doctor(games: Res<Games>, mut commands: Commands) {
    let report = super::mount_report(&games);
    for g in &report.games {
        let build = g.build.as_deref().map(|b| format!(", build {b}")).unwrap_or_default();
        let state = if g.installed { "installed" } else { "not installed" };
        info!("mount doctor: {} {state}{build}", g.game);
    }
    commands.insert_resource(chimera_net::net::manifest::LocalMounts(report));
}

/// The guests' guns' names for Combat Arms' HUD: its weapon panel and
/// kill feed call a weapon by `ItemNames` (else its data's `hud.name`,
/// which for CS:S and HL2:DM is a string token), so the names come from
/// each game's own strings in its install (its bundle's `display_names`:
/// CS:S's `weapons::display_names`, HL2:DM's; none without the install,
/// the guns then show their ids).
fn guest_names(games: Res<Games>, mut names: ResMut<ItemNames>) {
    for bundle in super::GUESTS {
        let Some(read) = bundle.display_names else { continue };
        let Some(mount) = games.get(bundle.info.id).and_then(|g| g.mount()) else {
            continue;
        };
        names.0.extend(read(&mount));
    }
}

/// The Tab scoreboard's gun names: CS:S's (the minigame panel's) and
/// Combat Arms' (`ItemNames`), merged as either changes.
fn sync_gun_names(
    cs: Res<cs_source::ui::minigame_hud::WeaponNames>,
    ca: Res<ItemNames>,
    mut out: ResMut<chimera_client::client::minigame_scoreboard::GunNames>,
) {
    if !(cs.is_changed() || ca.is_changed()) {
        return;
    }
    out.0.extend(cs.0.iter().map(|(k, v)| (k.clone(), v.clone())));
    out.0.extend(ca.0.iter().map(|(k, v)| (k.clone(), v.clone())));
}

/// The party's client app (Lucker Party's, `APP`; another party app's is
/// `GameApp { title, hooks: .., ..PARTY }`, apps/ultimate_gun_game) on
/// Chimera's client app (`chimera_client::client::app`): what this
/// machine lacks told first (`crate::client_requirements`: a missing
/// install is a warning, a server's offer brings the files), the command line as it plays (`with_defaults`), the map
/// (`cs_source:de_dust2` without one; an error ends the process), every
/// install as asset sources, the party's simulation (`PartyPlugins`),
/// Combat Arms' gameplay until a minigame takes it, the network and the
/// client shell with every game's HUD (`with_huds`), then the party's
/// front end (`front_end`).
pub const PARTY: GameApp = GameApp {
    bin: "lucker_party",
    info: &combat_arms::game::INFO,
    title: "Lucker Party",
    about: || {
        format!(
            "lucker_party: minigames with Counter-Strike: Source's and Combat Arms' parts. Starts {} on\n\
             its first map; +minigame <id> picks another ({}), --map cs_source:<name> (or\n\
             combat_arms:<name>) the map.\n",
            super::DEFAULT_MINIGAME,
            super::ids().join(", "),
        )
    },
    version: super::VERSION,
    build: super::BUILD,
    maps: AppMaps {
        load: super::load_map_with,
        load_file: |id, _, _| Err(format!("{id}: Lucker Party loads maps by id only, not from a file")),
        file_bytes: map_file_bytes,
    },
    // Every install as `cs_source://`, `hl2dm://` and `combat_arms://`
    // assets.
    installs: || {
        super::GUESTS
            .iter()
            .map(|b| chimera_present::assets::InstallSource::of(b.info))
            .chain([chimera_present::assets::InstallSource::shared(
                combat_arms::game::INFO.id,
                combat_arms::game::world::mount,
            )])
            .collect()
    },
    sim,
    hud: with_huds,
    front_end,
    hooks: GameAppHooks {
        args: Some(with_defaults),
        // What this machine lacks, said exactly (a joiner without an install
        // gets the files from a server that offers them: `net::assets`).
        requirements: Some(crate::client_requirements),
        default_map: Some("cs_source:de_dust2"),
        server: Some("lucker_party_server"),
        ..GameAppHooks::NONE
    },
};

/// Lucker Party's client app (`PARTY`): the `lucker_party` binary's.
pub static APP: GameApp = PARTY;

/// The party's simulation (`PartyPlugins`), Combat Arms' gameplay until
/// the minigame (`+minigame`, its console line) takes the rules slot and
/// names the others, the Escape menu titled by the app, and a ladder's
/// rung extras that differ every launch.
pub fn sim(app: &mut App, _: &Args) {
    use chimera_core::gameplay::{Gameplay, GameplayAppExt};
    let title = app.world().resource::<RunningApp>().0.title;
    app.add_plugins(super::PartyPlugins)
        .set_gameplay(Gameplay::of(&combat_arms::game::INFO))
        .insert_resource(crate::esc_menu::EscMenuTitle(title.into()))
        .insert_resource(super::launch_rung_seed());
}

/// A party app's start at its main menu (`main_menu`; its
/// `GameAppHooks::start`) when the command line doesn't play
/// (`main_menu::starts_at_menu`): the first of data/main_menu.toml's
/// background maps that loads, else no map at all behind it. None: the
/// command line plays (the usual start map).
pub fn start_at_menu(game: &'static GameApp, args: &Args) -> Option<StartMap> {
    if !crate::main_menu::starts_at_menu(args) {
        // A joiner with no map of its own (`with_defaults_for`: the default
        // map's game isn't installed) waits on the greybox.
        let no_map = args.game_map().is_none() && joins_server(&args.console);
        return no_map.then_some(StartMap::Greybox);
    }
    let data = crate::main_menu::MenuData::shipped();
    let load = |id: &str| (game.maps.load)(id, &chimera_world::map::MapLoadOptions::hdr(0));
    Some(match crate::main_menu::background_map(&data, load) {
        Some((id, map)) => StartMap::loaded(&id, map),
        None => {
            eprintln!("warning: none of the main menu's background maps loads: the greybox behind it");
            StartMap::Empty
        }
    })
}

/// Whether the command line joins a server (`+connect <address>`).
pub fn joins_server(console: &[String]) -> bool {
    console.iter().any(|c| c.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("connect")))
}

/// The command line as the app plays it: `DEFAULT_MINIGAME` without
/// `+minigame`, and without `--map` that minigame's first map (a minigame
/// that isn't built in keeps the default's); `DEFAULT_BOTS` bots unless it
/// sets `MaxPlayer`. A joiner (`+connect`) gets neither the minigame nor
/// the bots: the server plays its minigame (`minigame_net`) and fills its
/// own slots; the map is the default's, loaded at once, until the server
/// says its own (none, the greybox, without its game's install).
pub fn with_defaults(args: Args) -> Args {
    with_defaults_for(args, super::DEFAULT_MINIGAME, &|id| {
        super::first_map(id).or_else(|_| super::first_map(super::DEFAULT_MINIGAME))
    })
}

/// `with_defaults` for an app whose minigame without `+minigame` is
/// `minigame`, and whose minigames' first maps `first_map` gives.
pub fn with_defaults_for(mut args: Args, minigame: &str, first_map: &dyn Fn(&str) -> Result<String, String>) -> Args {
    let joining = joins_server(&args.console);
    if !joining && !args.console.iter().any(|c| c.to_lowercase().starts_with("maxplayer")) {
        args.console.insert(0, format!("MaxPlayer {}", super::DEFAULT_BOTS));
    }
    let minigame = match super::minigame_of(&args.console) {
        Some(id) => id,
        None if joining => minigame.to_string(),
        None => {
            args.console.push(format!("minigame {minigame}"));
            minigame.to_string()
        }
    };
    if args.map.is_none() {
        // A joiner without the map's game installed waits on the greybox
        // (`start_at_menu`): the server's offer brings the files
        // (`net::assets`).
        args.map = first_map(&minigame).ok().filter(|m| !joining || map_installed(m));
    }
    args
}

/// Whether map `id`'s game has an install here: its map loads at once,
/// without a download (a game read from the asset store would fetch it
/// before the join's download, with no progress shown).
pub fn map_installed(id: &str) -> bool {
    let info = match id.split_once(':').map(|(g, _)| g) {
        Some(cs_source::game::GAME) => &cs_source::game::INFO,
        Some(combat_arms::game::GAME) => &combat_arms::game::INFO,
        Some(hl2dm::game::GAME) => &hl2dm::game::INFO,
        _ => return true,
    };
    info.install().is_some() && !info.served_alone()
}

#[cfg(test)]
mod tests {
    use chimera_client::client::Args;

    #[test]
    fn it_starts_the_gun_game_on_its_first_map() {
        let args = super::with_defaults(Args::default());
        assert_eq!(args.map.as_deref(), Some("cs_source:de_dust2"));
        assert_eq!(args.console, ["MaxPlayer 3", "minigame originals:gun_game_dust2"]);
        let mut own = Args::default();
        own.map = Some("combat_arms:warehouse".into());
        own.console.push("minigame originals:gun_game_dust2".into());
        own.console.push("maxplayer 0".into());
        let own = super::with_defaults(own);
        assert_eq!(own.map.as_deref(), Some("combat_arms:warehouse"));
        assert_eq!(own.console.len(), 2);
        assert_eq!(super::client_hooks().build, crate::BUILD);
    }

    #[test]
    fn a_joiner_leaves_the_minigame_and_the_bots_to_the_server() {
        let mut join = Args::default();
        join.console.push("connect 127.0.0.1:28930".into());
        let join = super::with_defaults(join);
        assert!(super::joins_server(&join.console));
        assert_eq!(join.console, ["connect 127.0.0.1:28930"], "no local minigame or bots");
        let installed = super::map_installed("cs_source:de_dust2");
        assert_eq!(
            join.map.as_deref(),
            installed.then_some("cs_source:de_dust2"),
            "the default map meanwhile, if its game is installed"
        );
        assert!(!super::joins_server(&super::with_defaults(Args::default()).console));
    }
}
