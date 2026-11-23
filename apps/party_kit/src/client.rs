//! The client's side of Lucker Party (feature `present`): the party's
//! client app (`PARTY`, Lucker Party's `APP`, on Chimera's
//! `chimera_client::client::app`), the shell with this app's hooks and
//! both games' HUDs (`with_huds`:
//! Combat Arms' and CS:S's, the one drawn the local player's game, which
//! follows its gun in the gun game: `chimera_sim::identity`; the
//! minigame's panel is the shell's overlay over each; CS:S's body animation for a player holding a CS:S
//! gun), the map files it can read for a server's map check
//! (`map_file_bytes`: CS:S's and HL2:DM's from their installs or the
//! asset store, so a server on another map than the one loaded is joined;
//! Combat Arms' maps load by id) and the command line as the app plays it
//! (`with_defaults`: the default minigame and its first map). At start it
//! runs the mount doctor (`mount_report` into `net::manifest::LocalMounts`),
//! which a server checks its match against when this client joins; again,
//! with the guests' gun names, once a join that bound the asset service
//! has the games' files (`read_mounts_again`).
//!
//! CS:S's HUD here is its health, armour and ammo panels, death notices,
//! damage sprites and weapon selection row (`ui::game_hud`,
//! `hud_sprites`, `weapon_select`), from CS:S's HUD look, HL2:DM's HUD from
//! its own (each map carries every game's, `MapSources::players` and
//! `guests`; up only with its game's HUD: `client::hud::HudLookOwner`), with
//! the shell's crosshair on CS:S's. Over all, on the shell's overlay
//! layer (`client::overlay`, the same whichever HUD is up): the minigame's
//! panel (`client::minigame_panel`) and callouts and the one kill feed;
//! no radars unless `hud_radar 1` (`PartyHudPlugin`); with
//! `party_swap_effect 1` (default 0) a short flash when the local
//! player's gun changes game, a puff at others' (`client::swap_effect`).
//! Number keys switch weapons at once under every game's HUD
//! (`hl2_fast_switch`; CS:S's `hud_fastswitch` is 1 by default).
//! The Escape menu is the party's own (`esc_menu`), titled by the app;
//! `start_at_menu` starts an app at the main menu (`main_menu`) with no
//! map, over the menu's own backdrop; `background_at_hand` says which maps
//! can be its background without a download.
//! Known shortcuts: not CS:S's spectator panel or overview;
//! on Combat Arms' maps there is no CS:S HUD look (the shell's plain HUD
//! with CS:S's guns).

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_client::client::{
    AppHooks, Args,
    app::{GameApp, RunningApp, StartMap},
};
use chimera_core::game_registry::Games;
use chimera_world::{game_bundle::GameClient, map::hud::GameUi};

use crate::{Party, Roster};

/// A party's front end, handed to the kit by the app that composes the
/// games (apps/party_roster's `client::CLIENT`), beside its `Party`.
pub struct PartyClient {
    pub party: &'static Party,
    /// Each game's front end (its `ui`'s `GameClient`): the base game's
    /// first, then the guests'.
    pub clients: &'static [&'static GameClient],
    /// The composition's own front end, after the guests' (`GameClient::
    /// app`) and the base game's (`BaseClient::app`): the guns' names for
    /// the base game's HUD.
    pub front_end: fn(&mut App),
    /// The main menu's look (a game's GameUI), read on a thread at start.
    pub menu_look: fn() -> Option<GameUi>,
}

impl PartyClient {
    /// The base game's front end as the base (its `GameClient::base`), if
    /// it has one.
    pub fn base_client(&self) -> Option<&'static chimera_world::game_bundle::BaseClient> {
        self.clients.iter().find(|c| c.game == self.party.base.info.id)?.base
    }
}

pub use chimera_world::game_bundle::{MenuMusic, SetBaseLoading};

/// The front end an app runs (`PartyClient`), for the kit's systems:
/// inserted by `front_end`.
#[derive(Resource, Clone, Copy)]
pub struct Front(pub &'static PartyClient);

/// What an app tells the client shell: its commit, its maps and the
/// greybox's own hooks (the app's `GameApp`'s).
pub fn client_hooks(app: &GameApp) -> AppHooks {
    app.app_hooks()
}

/// The bytes of map `id`'s file as this app loads it, for the network's
/// map check (`net::maps::MapFiles`): a CS:S, HL2:DM or TF2 map's, from the
/// install (or the asset store standing in for it). Combat Arms' maps have
/// no file of their own (the server offers them by id and hash only:
/// `net::maps`), so a client loads them by id. Without this a client
/// joining a server on a map other than the one it has loaded found no
/// copy, asked the server for a file the server doesn't have, and stayed
/// in its own greybox game with a minigame that nobody plays; an HL2:DM
/// map (offered with its hash, never sent) disconnected every player
/// ("The server has no file to send", rehearsal 2026-11-12). Halo's maps,
/// like Combat Arms', are offered by id and hash only (`GameInfo::
/// map_file` names none: a disc's file is never sent), so none here.
pub fn map_file_bytes(party: &Party, id: &str) -> Option<Vec<u8>> {
    let (game, name) = id.split_once(':')?;
    let info = party.bundle(game)?.info;
    let file = (info.map_file)(name)?;
    (info.open)(&info.install()?).ok()?.read(&file).ok()
}

/// The shell's plugins with every game's HUD (each bundle's `client`):
/// the base game's, Combat Arms' (`ui::with_hud`), then the guests':
/// CS:S's panels, sprites and weapon selection (whose logic is the shell's
/// `WeaponMenuPlugin`, added once, by CS:S's), CS:S's guns' names for the
/// shell's minigame panel, and the rest of CS:S's HUD that follows the gun in hand
/// (the radar, target ID, freeze cam, voice status: each drawing only
/// while CS:S's HUD is the gameplay's), and HL2:DM's panels (M9, drawing while its HUD is the gameplay's: on any map, its
/// look coming with the map, `MapSources::guests`).
pub fn with_huds(front: &PartyClient, shell: PluginGroupBuilder) -> PluginGroupBuilder {
    let mut shell = shell;
    for client in front.clients {
        if let Some(hud) = client.hud {
            shell = hud(shell);
        }
    }
    shell
}

/// The client after the shell with both games' HUDs (`PARTY`'s front
/// end; `with_huds` puts them among the shell's plugins, each drawing in
/// its own look, `HudLookOwner`): the key binds page, the party's Escape
/// menu, mouse and HUD settings, the minigame scoreboard, the swap
/// effect, the mount doctor and the guests' guns' names, each guest's
/// front end, then Combat Arms' other looks.
pub fn front_end(front: &'static PartyClient, app: &mut App, args: &Args) {
    app.insert_resource(Front(front));
    // Invite links (`chimera-hub://`) that open the game: the one this
    // start was opened with first; the first run shows it (`onboarding`).
    app.add_plugins(chimera_client::client::link_handler::LinkHandlerPlugin);
    if let Some(link) = &args.opened_link {
        app.world_mut().resource_mut::<chimera_client::client::link_handler::OpenedLinks>().0.push(link.clone());
    }
    // The key binds page (F6, `controls`): shared binds and each
    // game's own.
    app.add_plugins(crate::controls::ControlsPlugin)
        // The party's own Escape menu (Combat Arms' yields Esc to it).
        .add_plugins(crate::esc_menu::EscMenuPlugin)
        // Settings in the Signal look (the menus' Options and Host
        // settings under Ultimate Gun Game's theme).
        .add_plugins(crate::settings::SettingsPlugin)
        // The report dialog (F10 in Ultimate Gun Game, the Escape menu's
        // "Report a bug") in the Signal look.
        .add_plugins(crate::feedback_signal::FeedbackSignalPlugin)
        // The party panel, the follow prompt (provisional:
        // docs/plans/active/parties.md).
        .add_plugins(crate::party_signal::PartySignalPlugin)
        // The main menu's notice after a session that died (a crash, a
        // freeze, a power loss): send its report?
        .add_plugins(crate::crash_signal::CrashSignalPlugin)
        // The first run: from an invite to the main menu, signed in
        // (docs/plans/active/onboarding.md).
        .add_plugins(crate::onboarding::OnboardingPlugin)
        // Settings > Account, from the chip; the base files up front.
        .add_plugins((crate::account_page::AccountPagePlugin, crate::base_download::BaseDownloadPlugin))
        .add_plugins(PartyHudPlugin)
        .add_plugins(chimera_client::client::minigame_scoreboard::MinigameScoreboardPlugin)
        // `party_swap_effect` (default 0): a flash on a rung's swap.
        .add_plugins(chimera_client::client::swap_effect::SwapEffectPlugin)
        .add_systems(Startup, mount_doctor);
    // A join that binds the asset service: the mount doctor again once
    // the games' files arrived.
    app.add_observer(read_mounts_again);
    // The rest of each guest's front end (CS:S's: the map's HUD look
    // is its, and what its bodies play for a player holding its gun,
    // the gun game's `identity = "weapon"`).
    for client in front.clients {
        if let Some(add) = client.app {
            add(app);
        }
    }
    // The base game's own as the base (its other looks, the mouse look's
    // starting values), then the composition's own.
    if let Some(base) = front.base_client() {
        (base.app)(app);
    }
    (front.front_end)(app);
}

/// The party's HUD settings over both games' HUDs: one kill feed, the
/// shell's (`client::kill_feed`, `SharedKillFeed`: the games' own feeds
/// stay empty), drawn in one place whichever game's HUD is up, so a kill
/// just before a rung swap is still in it after; and the games' radars
/// hidden (`hud_radar` starts at 0; `hud_radar 1` brings CS:S's radar and
/// Combat Arms' corner map back); the hit marker on (`cl_hitmarker` starts
/// at 1). Neither has an Options row.
pub struct PartyHudPlugin;

impl Plugin for PartyHudPlugin {
    fn build(&self, app: &mut App) {
        use chimera_client::client::{hud, kill_feed};
        app.insert_resource(kill_feed::SharedKillFeed);
        if !app.is_plugin_added::<kill_feed::KillFeedStatePlugin>() {
            app.add_plugins(kill_feed::KillFeedStatePlugin);
        }
        hud::default_radars(app, false);
        // The hit marker on in the party (Chimera's default is off, CS:S's
        // parity); it has no Options row.
        app.insert_resource(chimera_client::client::hit_marker::HitMarkerSetting { on: 1 });
        if let Some(mut console) = app.world_mut().get_resource_mut::<chimera_core::console::Console>() {
            console.set_default("cl_hitmarker", "1");
        }
    }
}

/// After a join bound the asset service and the games' files arrived
/// (`net::assets::AssetsArrived`): the mount doctor again, now that the
/// games read from the store.
fn read_mounts_again(
    _: On<chimera_net::net::assets::AssetsArrived>,
    games: Res<Games>,
    roster: Res<Roster>,
    mut commands: Commands,
) {
    examine_mounts(roster.0, &games, &mut commands);
}

/// The mount doctor (`net::manifest`): what this machine has of each game,
/// which a server checks its match against when this client joins.
fn mount_doctor(games: Res<Games>, roster: Res<Roster>, mut commands: Commands) {
    examine_mounts(roster.0, &games, &mut commands);
}

fn examine_mounts(party: &Party, games: &Games, commands: &mut Commands) {
    let report = super::mount_report(party, games);
    for g in &report.games {
        let build = g.build.as_deref().map(|b| format!(", build {b}")).unwrap_or_default();
        let state = if g.installed { "installed" } else { "not installed" };
        info!("mount doctor: {} {state}{build}", g.game);
    }
    commands.insert_resource(chimera_net::net::manifest::LocalMounts(report));
}

/// The window's icon, one at random per start: the logo's three
/// characters on their avatar grounds (assets/icon; the .exe's `app.ico`
/// is the Operator).
pub const WINDOW_ICONS: &[&[u8]] = &[
    include_bytes!("../assets/icon/operator.png"),
    include_bytes!("../assets/icon/trooper.png"),
    include_bytes!("../assets/icon/vanguard.png"),
];

/// The party's simulation (`PartyPlugins`), Combat Arms' gameplay until
/// the minigame (`+minigame`, its console line) takes the rules slot and
/// names the others, the Escape menu titled by the app, and a ladder's
/// rung extras that differ every launch.
pub fn sim(party: &'static Party, app: &mut App, _: &Args) {
    use chimera_core::gameplay::{Gameplay, GameplayAppExt};
    let title = app.world().resource::<RunningApp>().0.title;
    app.add_plugins(super::PartyPlugins(party))
        .set_gameplay(Gameplay::of(party.base.info))
        .insert_resource(crate::esc_menu::EscMenuTitle(title.into()))
        .insert_resource(super::launch_rung_seed());
}

/// A party app's start at its main menu (`main_menu`; its
/// `GameAppHooks::start`) when the command line doesn't play
/// (`main_menu::starts_at_menu`): no map at all, so the menu shows at once
/// over its own backdrop; the menu then loads a background map in the
/// background if one is at hand (`main_menu`, "Background"). None: the
/// command line plays (the usual start map).
pub fn start_at_menu(_: &'static GameApp, args: &Args) -> Option<StartMap> {
    if !crate::main_menu::starts_at_menu(args) {
        // A joiner with no map of its own (`with_defaults_for`: the default
        // map's game isn't installed) waits on the greybox.
        let no_map = args.game_map().is_none() && joins_server(&args.console);
        return no_map.then_some(StartMap::Greybox);
    }
    Some(StartMap::Empty)
}

/// Whether map `id` can be the main menu's background without a download
/// (`main_menu::BackgroundMaps`): its game installed (`map_installed`),
/// or served from the asset store alone with the map's set and the game's
/// `base` whole in the store (`mount::served::map_held`: from earlier
/// joins or a fetch ahead). Asked with fetches
/// held, so even finding the game's build asks no service.
pub fn background_at_hand(party: &Party, id: &str) -> bool {
    let Some((game, name)) = id.split_once(':') else {
        return false;
    };
    let _hold = chimera_core::mount::assets::cached::hold_fetches(Some(game));
    let Some(info) = party.bundle(game).map(|b| b.info) else {
        return false;
    };
    if info.install().is_none() {
        return false;
    }
    if !info.served_alone() {
        return true;
    }
    chimera_core::mount::served::map_held(game, name)
}

/// Whether the command line joins a server (`+connect <address>`).
pub fn joins_server(console: &[String]) -> bool {
    console.iter().any(|c| c.split_whitespace().next().is_some_and(|w| w.eq_ignore_ascii_case("connect")))
}

/// The command line as an app plays it: `minigame` without `+minigame`,
/// and without `--map` that minigame's first map (`first_map`);
/// `DEFAULT_BOTS` bots unless it sets `MaxPlayer`. A joiner (`+connect`)
/// gets neither the minigame nor the bots: the server plays its minigame
/// (`minigame_net`) and fills its own slots; the map is the default's,
/// loaded at once, until the server says its own (none, the greybox,
/// without its game's install).
pub fn with_defaults_for(
    party: &Party,
    mut args: Args,
    minigame: &str,
    bots: u32,
    first_map: &dyn Fn(&str) -> Result<String, String>,
) -> Args {
    let joining = joins_server(&args.console);
    if !joining && !args.console.iter().any(|c| c.to_lowercase().starts_with("maxplayer")) {
        args.console.insert(0, format!("MaxPlayer {bots}"));
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
        args.map = first_map(&minigame).ok().filter(|m| !joining || map_installed(party, m));
    }
    args
}

/// Whether map `id`'s game has an install here: its map loads at once,
/// without a download (a game read from the asset store would fetch it
/// before the join's download, with no progress shown).
pub fn map_installed(party: &Party, id: &str) -> bool {
    let Some(info) = id.split_once(':').and_then(|(g, _)| party.bundle(g)).map(|b| b.info) else {
        return true;
    };
    info.install().is_some() && !info.served_alone()
}
