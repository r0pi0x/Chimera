//! The client's side of Lucker Party (feature `present`): the shell with
//! this app's hooks and both games' HUDs (`ClientPlugin`, `with_huds`:
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
//! `hud_sprites`, `weapon_select`), from the map's CS:S HUD look (kept on
//! CS:S's maps, `MapSources::players`; put away while Combat Arms' HUD
//! draws: `client::hud::HudLookOwner`), with the shell's crosshair, and
//! the minigame's panel (`ui::minigame_hud`). Over both: one kill feed, the
//! shell's, and no radars unless `hud_radar 1` (`PartyHudPlugin`); with
//! `party_swap_effect 1` (default 0) a short flash when the local
//! player's gun changes game, a puff at others' (`client::swap_effect`). Known
//! The Escape menu is the party's own (`esc_menu`), titled by the app;
//! `client_app_at_menu` starts the app at the main menu (`main_menu`)
//! over the first of its background maps that loads.
//! Known shortcuts: not CS:S's spectator panel or overview;
//! on Combat Arms' maps there is no CS:S HUD look (the shell's plain HUD
//! with CS:S's guns).

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_client::client::{AppHooks, Args, ClientPlugins};
use chimera_core::game_registry::Games;
use combat_arms::ui::hud::ItemNames;
use chimera_world::greybox;

/// What this app tells the client shell: its commit, its maps
/// (`MAP_SOURCES`) and the greybox's own hooks.
pub fn client_hooks() -> AppHooks {
    AppHooks {
        build: super::BUILD,
        load_map: super::load_map_with,
        load_map_file: |id, _, _| Err(format!("{id}: Lucker Party loads maps by id only, not from a file")),
        map_file_bytes,
        unload_greybox: greybox::unload,
        respawn_greybox: greybox::respawn,
    }
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

/// The shell's plugins with both games' HUDs: Combat Arms' (`ui::
/// with_hud`), then CS:S's panels, sprites and weapon selection (whose
/// logic is the shell's `WeaponMenuPlugin`, added once, by CS:S's) and the
/// minigame's panel on CS:S's HUD.
pub fn with_huds(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    use chimera_client::client::{hud::HudPlugin, view::ViewPlugin, weapon_menu::WeaponMenuPlugin};
    use cs_source::ui::{game_hud::GameHudPlugin, hud_sprites::HudSpritesPlugin, minigame_hud::MinigameHudPlugin};
    combat_arms::ui::with_hud(shell)
        .disable::<WeaponMenuPlugin>()
        .add_after::<HudPlugin>(GameHudPlugin)
        .add_after::<GameHudPlugin>(HudSpritesPlugin)
        .add_after::<HudSpritesPlugin>(MinigameHudPlugin)
        .add_after::<ViewPlugin>(cs_source::ui::weapon_select::WeaponSelectPlugin)
        // The rest of CS:S's HUD that follows the gun in hand: the radar,
        // the names of the players aimed at, the freeze cam and the voice
        // status (each draws only while CS:S's HUD is the gameplay's).
        .add_after::<chimera_client::client::game_text::GameTextPlugin>(cs_source::ui::radar::RadarPlugin)
        .add_after::<MinigameHudPlugin>(cs_source::ui::target_id::TargetIdPlugin)
        .add_after::<cs_source::ui::target_id::TargetIdPlugin>(cs_source::ui::freeze_cam::FreezeCamPlugin)
        .add_after::<cs_source::ui::freeze_cam::FreezeCamPlugin>(cs_source::ui::voice_hud::VoiceHudPlugin)
}

/// The client: the shell's plugins with the app's hooks and both games'
/// HUDs (`with_huds`), the map's HUD look CS:S's (`HudLookOwner`).
pub struct ClientPlugin {
    pub args: Args,
}

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(with_huds(
            ClientPlugins {
                args: self.args.clone(),
                hooks: client_hooks(),
            }
            .build(),
        ))
        .insert_resource(chimera_client::client::hud::HudLookOwner(cs_source::game::GAME))
        // What CS:S's bodies play, for a player holding a CS:S gun (the
        // gun game's `identity = "weapon"`).
        .add_plugins(cs_source::game::player_anim::PlayerAnimPlugin)
        // The key binds page (F6, `controls`): shared binds and each
        // game's own.
        .add_plugins(crate::controls::ControlsPlugin)
        // The party's own Escape menu (Combat Arms' yields Esc to it).
        .add_plugins(crate::esc_menu::EscMenuPlugin)
        .add_plugins(PartyMousePlugin)
        .add_plugins(PartyHudPlugin)
        .add_plugins(chimera_client::client::minigame_scoreboard::MinigameScoreboardPlugin)
        // `party_swap_effect` (default 0): a flash on a rung's swap.
        .add_plugins(chimera_client::client::swap_effect::SwapEffectPlugin)
        .add_systems(Startup, (cs_source_names, mount_doctor))
        .add_systems(Update, sync_gun_names);
    }
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

/// CS:S's guns' names for Combat Arms' HUD: its weapon panel and kill feed
/// call a weapon by `ItemNames` (else its data's `hud.name`, which for CS:S
/// is a string token), so the names come from CS:S's own strings in its
/// install (`weapons::display_names`; none without the install, the guns
/// then show their ids).
fn cs_source_names(games: Res<Games>, mut names: ResMut<ItemNames>) {
    let Some(mount) = games.get(cs_source::game::GAME).and_then(|g| g.mount()) else {
        return;
    };
    names.0.extend(cs_source::game::weapons::display_names(&mount));
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

/// The game's app (the binary's, and other party apps' built on it): map
/// `args.game_map()` loaded (an error ends the process), both installs as
/// asset sources, the window titled `title`, the party's simulation
/// (`PartyPlugins`), the presentation, Combat Arms' gameplay until a
/// minigame takes it, the network and the client shell with both games'
/// HUDs. `chimera_client::client::app::run` runs it.
pub fn client_app(args: Args, title: &str) -> App {
    let id = args.game_map().unwrap_or("cs_source:de_dust2").to_string();
    match super::load_map(&id, 0) {
        Ok(data) => client_app_on(args, title, &id, data),
        Err(e) => {
            eprintln!("error: --map {id}: {e}");
            std::process::exit(1);
        }
    }
}

/// `client_app` starting at the party's main menu (`main_menu`): the map
/// behind it the first of data/main_menu.toml's background maps that
/// loads (the greybox if none does), Play starting `minigame`.
pub fn client_app_at_menu(args: Args, title: &str, minigame: &str) -> App {
    let data = crate::main_menu::MenuData::shipped();
    let mut app = match crate::main_menu::background_map(&data, |id| super::load_map(id, 0)) {
        Some((id, map)) => client_app_on(args, title, &id, map),
        None => {
            eprintln!("warning: none of the main menu's background maps loads: the greybox behind it");
            let mut args = args;
            args.map = None;
            client_app_with(args, title, None)
        }
    };
    app.add_plugins(crate::main_menu::MainMenuPlugin {
        at_start: true,
        minigame: minigame.into(),
        keep_recent: true,
    });
    app
}

/// `client_app` on map `id` (its data loaded).
fn client_app_on(args: Args, title: &str, id: &str, data: chimera_world::map::MapData) -> App {
    eprintln!(
        "loaded {id}: {} triangles, {} textures, {} spawns, {} warnings",
        data.triangle_count(),
        data.textures.len(),
        data.spawns.len(),
        data.warnings.len()
    );
    for w in &data.warnings {
        eprintln!("  warning: {w}");
    }
    client_app_with(args, title, Some(data))
}

fn client_app_with(args: Args, title: &str, map: Option<chimera_world::map::MapData>) -> App {
    use chimera_core::gameplay::{Gameplay, GameplayAppExt};
    use chimera_present::{MapPlugin, PresentationPlugins};
    use super::PartyPlugins;
    let mut app = App::new();
    app.add_plugins(match map {
        Some(data) => MapPlugin::new(data),
        None => MapPlugin::empty(),
    });
    app.add_plugins((
        // Both installs as `cs_source://` and `combat_arms://` assets
        // (before `AssetPlugin`).
        chimera_present::assets::InstallSourcesPlugin(vec![
            chimera_present::assets::InstallSource::of(&cs_source::game::INFO),
            chimera_present::assets::InstallSource::shared(combat_arms::game::INFO.id, combat_arms::game::world::mount),
        ]),
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: title.into(),
                    resolution: match (args.window, args.scale) {
                        (Some(s), scale) => bevy::window::WindowResolution::new(s.x, s.y)
                            .with_scale_factor_override(scale.unwrap_or(1.0)),
                        (None, Some(scale)) => bevy::window::WindowResolution::default().with_scale_factor_override(scale),
                        (None, None) => default(),
                    },
                    resizable: args.window.is_none(),
                    ..default()
                }),
                ..default()
            })
            // Warnings and errors also show in the in-game console.
            .set(bevy::log::LogPlugin {
                custom_layer: chimera_client::client::console::log_layer,
                ..default()
            }),
        // Avian as Chimera runs it (rendering eases between ticks).
        chimera_world::physics_plugins(),
        PartyPlugins,
    ))
    // What the map's drawing needs.
    .add_plugins(PresentationPlugins)
    // Combat Arms' gameplay until the minigame (`+minigame`, its console
    // line) takes the rules slot and names the others.
    .set_gameplay(Gameplay::of(&combat_arms::game::INFO))
    .insert_resource(chimera_core::core::BuildVersion(super::VERSION))
    .insert_resource(crate::esc_menu::EscMenuTitle(title.into()))
    // A ladder's rung extras differ every launch.
    .insert_resource(super::launch_rung_seed())
    // Network play: a listen server, or a client (`connect`).
    .add_plugins(chimera_net::net::NetPlugin)
    // The client shell with Combat Arms' HUD, then its other looks.
    .add_plugins(ClientPlugin { args })
    .add_plugins(combat_arms::ui::CaUiPlugin);
    app
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
/// says its own.
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
        args.map = first_map(&minigame).ok();
    }
    args
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
        assert_eq!(join.map.as_deref(), Some("cs_source:de_dust2"), "the default map meanwhile");
        assert!(!super::joins_server(&super::with_defaults(Args::default()).console));
    }
}
