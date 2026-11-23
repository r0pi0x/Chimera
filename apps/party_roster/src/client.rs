//! The party's front end as the roster composes it (feature `present`):
//! the kit's client (`party_kit::client`, re-exported) given this party's
//! games, `CLIENT` (each game's `GameClient`, the main menu's look), and
//! the client app, `PARTY` (Lucker Party's `APP`; the Ultimate Gun Game's,
//! apps/ultimate_gun_game, is built on it).
//!
//! What is one game's in the party's front end lives here
//! (`front_end_parts`; Combat Arms' other looks, mouse look, loading
//! screen and music are its own as the base, `combat_arms_ui::
//! BASE_CLIENT`): the guests' guns' names for Combat Arms' HUD and the shell's (`guest_names`,
//! `sync_gun_names`), HL2:DM's fast switch on (`hl2_fast_switch`), and
//! Combat Arms' air-strike siren silent under UI sound (`mute_siren`).

use bevy::{app::PluginGroupBuilder, prelude::*};
use chimera_client::client::{
    AppHooks, Args,
    app::{AppMaps, GameApp, GameAppHooks},
};
use chimera_core::game_registry::Games;
use chimera_world::game_bundle::GameClient;
use combat_arms_ui::hud::ItemNames;
pub use party_kit::client::*;
use party_kit::controls::GameRows;

use crate::PARTY as THE_PARTY;

/// Each game's front end (its `ui`'s `GameClient`, beside its bundle):
/// the base game's first, then the guests' (HL2:DM's: M9; Halo's: its
/// HUD while a Halo gun is in hand; TF2's: its HUD while a TF2 gun is in
/// hand).
pub static CLIENTS: [&GameClient; 5] = [
    &combat_arms_ui::PARTY_CLIENT,
    &cs_source_ui::party::CLIENT,
    &hl2dm_ui::party::CLIENT,
    &halo_ce_ui::party::CLIENT,
    &tf2_ui::party::CLIENT,
];

/// Each game's own tab on the Controls page, in tab order (after
/// "Shared"): made of each game's `data/player_options.toml` (`party_kit::
/// game_options::controls`), the party installed first.
pub fn controls() -> &'static [GameRows] {
    party_kit::game_options::install(&THE_PARTY);
    party_kit::game_options::controls()
}

/// The party's front end, handed to the kit.
pub static CLIENT: PartyClient = PartyClient {
    party: &THE_PARTY,
    clients: &CLIENTS,
    front_end: front_end_parts,
    // The GameUI look of CS:S's install.
    menu_look: || {
        cs_source::game::INFO
            .install()
            .and_then(|p| cs_source::game::mount::open(&p).ok())
            .and_then(|m| cs_source::game::gameui::load(&m))
    },
};

/// What this app tells the client shell: its commit, its maps
/// (`MAP_SOURCES`) and the greybox's own hooks (`PARTY`'s).
pub fn client_hooks() -> AppHooks {
    PARTY.app_hooks()
}

/// The bytes of map `id`'s file as this app loads it (`party_kit::client::
/// map_file_bytes`).
pub fn map_file_bytes(id: &str) -> Option<Vec<u8>> {
    party_kit::client::map_file_bytes(&THE_PARTY, id)
}

/// The shell's plugins with every game's HUD (`party_kit::client::with_huds`).
pub fn with_huds(shell: PluginGroupBuilder) -> PluginGroupBuilder {
    party_kit::client::with_huds(&CLIENT, shell)
}

/// The party's front end (`party_kit::client::front_end`).
pub fn front_end(app: &mut App, args: &Args) {
    party_kit::client::front_end(&CLIENT, app, args);
}

/// The party's simulation (`party_kit::client::sim`).
pub fn sim(app: &mut App, args: &Args) {
    party_kit::client::sim(&THE_PARTY, app, args);
}

/// Whether map `id` can be the main menu's background without a download
/// (`party_kit::client::background_at_hand`).
pub fn background_at_hand(id: &str) -> bool {
    party_kit::client::background_at_hand(&THE_PARTY, id)
}

/// Whether map `id`'s game has an install here (`party_kit::client::
/// map_installed`).
pub fn map_installed(id: &str) -> bool {
    party_kit::client::map_installed(&THE_PARTY, id)
}

/// The command line as the app plays it: `DEFAULT_MINIGAME` without
/// `+minigame`, and without `--map` that minigame's first map (a minigame
/// that isn't built in keeps the default's); `DEFAULT_BOTS` bots unless it
/// sets `MaxPlayer` (`with_defaults_for`).
pub fn with_defaults(args: Args) -> Args {
    with_defaults_for(args, crate::DEFAULT_MINIGAME, &|id| {
        crate::first_map(id).or_else(|_| crate::first_map(crate::DEFAULT_MINIGAME))
    })
}

/// `with_defaults` for an app whose minigame without `+minigame` is
/// `minigame`, and whose minigames' first maps `first_map` gives.
pub fn with_defaults_for(args: Args, minigame: &str, first_map: &dyn Fn(&str) -> Result<String, String>) -> Args {
    party_kit::client::with_defaults_for(&THE_PARTY, args, minigame, crate::DEFAULT_BOTS, first_map)
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
            crate::DEFAULT_MINIGAME,
            crate::ids().join(", "),
        )
    },
    version: party_kit::VERSION,
    build: party_kit::BUILD,
    maps: AppMaps {
        load: crate::load_map_with,
        load_file: |id, _, _| Err(format!("{id}: Lucker Party loads maps by id only, not from a file")),
        file_bytes: map_file_bytes,
    },
    // Every install as `cs_source://`, `hl2dm://`, `tf2://` and
    // `combat_arms://` assets.
    installs: || {
        crate::GUESTS
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
        // Every composed game's startup files come before the window when
        // they are read from the asset service (`client::startup`).
        games: Some(|| crate::BUNDLES.iter().map(|b| b.info).collect()),
        // Its view models load on demand, its start map in the app's
        // install data (`on_demand::front_end_context`).
        load_context: Some(combat_arms::game::view_model::on_demand::front_end_context),
        window_icons: WINDOW_ICONS,
        ..GameAppHooks::NONE
    },
};

/// Lucker Party's client app (`PARTY`): the `lucker_party` binary's.
pub static APP: GameApp = PARTY;

/// The party's front end for the kit's menus (`party_kit::client::Front`:
/// the menu's look) and the base game's answers to them (its
/// `BaseClient::menu`: Combat Arms' loading screen raised and lowered, its
/// music told what the main menu wants): part of `front_end`, and alone for a test that
/// builds the menus without the whole front end.
pub struct FrontPlugin;

impl Plugin for FrontPlugin {
    fn build(&self, app: &mut App) {
        // The Controls tabs are made of the games' page data.
        party_kit::game_options::install(&THE_PARTY);
        app.insert_resource(Front(&CLIENT));
        if let Some(base) = CLIENT.base_client() {
            (base.menu)(app);
        }
    }
}

/// The party's front end that names games, after the guests' own and the
/// base game's (`BaseClient::app`: Combat Arms' other looks, the mouse
/// look): the guests' guns' names, HL2:DM's fast switch, the siren under
/// UI sound.
fn front_end_parts(app: &mut App) {
    app.add_systems(Startup, (guest_names, hl2_fast_switch))
        .add_systems(Update, sync_gun_names)
        // Under UI sound our air-strike alert stands in for Combat Arms'
        // siren (`mute_siren`).
        .add_systems(Update, mute_siren.run_if(resource_exists::<chimera_client::client::ui_sound::UiSounds>));
    // A join that binds the asset service: the guests' gun names again
    // once the games' files arrived.
    app.add_observer(read_names_again);
    app.add_plugins(FrontPlugin);
}

/// Number keys switch weapons at once in the party (the user's choice):
/// CS:S's `hud_fastswitch` is 1 by default in every app
/// (`cs_source_ui::weapon_select::FastSwitch`); HL2:DM's HUD has its own,
/// `hl2dm:hud_fastswitch` (0 in HL2:DM, as in its game; not saved), set
/// to 1 here and made its default.
fn hl2_fast_switch(
    cvars: Option<ResMut<hl2dm_ui::systems::HudCvars>>,
    console: Option<ResMut<chimera_core::console::Console>>,
) {
    if let Some(mut c) = cvars {
        c.fastswitch = 1;
    }
    if let Some(mut c) = console {
        c.set_default("hl2dm:hud_fastswitch", "1");
    }
}

/// The mouse look of the party: Combat Arms' as the base
/// (`combat_arms_ui::BaseMousePlugin`).
pub use combat_arms_ui::BaseMousePlugin as PartyMousePlugin;

/// After a join bound the asset service and the games' files arrived
/// (`net::assets::AssetsArrived`): the guests' gun names again, now that
/// the games read from the store.
fn read_names_again(_: On<chimera_net::net::assets::AssetsArrived>, games: Res<Games>, names: Option<ResMut<ItemNames>>) {
    if let Some(mut names) = names {
        read_guest_names(&games, &mut names);
    }
}

/// The guests' guns' names for Combat Arms' HUD: its weapon panel and
/// kill feed call a weapon by `ItemNames` (else its data's `hud.name`,
/// which for CS:S and HL2:DM is a string token), so the names come from
/// each game's own strings in its install (its bundle's `display_names`:
/// CS:S's `weapons::display_names`, HL2:DM's; none without the install,
/// the guns then show their ids).
fn guest_names(games: Res<Games>, mut names: ResMut<ItemNames>) {
    read_guest_names(&games, &mut names);
}

fn read_guest_names(games: &Games, names: &mut ItemNames) {
    for bundle in crate::GUESTS {
        let Some(read) = bundle.display_names else { continue };
        let Some(mount) = games.get(bundle.info.id).and_then(|g| g.mount()) else {
            continue;
        };
        names.0.extend(read(&mount));
    }
}

/// The guns' full names for the shell (the Tab scoreboard, callouts, kill
/// feed, match-end screen): CS:S's (the minigame panel's) and Combat Arms'
/// (`ItemNames`), merged as either changes; the shell derives their short
/// names (`minigame_scoreboard::fill_short_names`).
fn sync_gun_names(
    cs: Res<cs_source_ui::minigame_hud::WeaponNames>,
    ca: Res<ItemNames>,
    mut out: ResMut<chimera_client::client::minigame_scoreboard::GunNames>,
) {
    if !(cs.is_changed() || ca.is_changed()) {
        return;
    }
    out.full.extend(cs.0.iter().map(|(k, v)| (k.clone(), v.clone())));
    out.full.extend(ca.0.iter().map(|(k, v)| (k.clone(), v.clone())));
}

/// Under UI sound the air strike's own alert (`air_alert`, from the
/// match's effects: `chimera_client::client::ui_cues`) stands in for
/// Combat Arms' siren (`surprise::ALARM`), which this client then doesn't
/// play (`MutedSounds`); without UI sound the siren plays as before.
pub fn mute_siren(
    theme: Option<Res<chimera_client::client::ui_sound::Theme>>,
    sounds: Option<Res<chimera_client::client::ui_sound::UiSounds>>,
    muted: Option<Res<chimera_present::audio::live_sound::MutedSounds>>,
    mut commands: Commands,
) {
    let on = chimera_client::client::ui_sound::active(theme.as_deref(), sounds.as_deref());
    let siren = crate::surprise::alarm_entry();
    let has = muted.as_ref().is_some_and(|m| m.0.contains(&siren));
    if on != has {
        let mut list = muted.map(|m| m.0.clone()).unwrap_or_default();
        list.retain(|e| *e != siren);
        if on {
            list.push(siren);
        }
        commands.insert_resource(chimera_present::audio::live_sound::MutedSounds(list));
    }
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
        assert_eq!(super::client_hooks().build, party_kit::BUILD);
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
