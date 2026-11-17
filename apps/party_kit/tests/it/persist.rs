//! What a player sets in the party survives a restart: the mouse
//! sensitivity (the shell's, over Combat Arms' options, which used to set
//! the mouse look back to its defaults at each start), the gun view FOV of
//! each game, and the binds (shared and each game's own). The config is
//! written as `host_writeconfig` writes it (`config_text`) and read as the
//! shell reads it at start (its lines run from the console's queue after
//! the startup systems), headless. The volumes and the brightness too:
//! Combat Arms' options apply, with its profile unloaded (`soundvolume`
//! 0), set the master volume to 0 at each start.

use bevy::prelude::*;
use bevy::audio::GlobalVolume;
use chimera_client::client::{
    audio::SoundOptionsPlugin,
    gamma::GammaPlugin,
    input::{MouseSettings, mouse_cvars},
};
use chimera_present::audio::live_sound::SoundMix;
use chimera_core::console::{Console, ConsolePlugin, config_text};
use chimera_world::map::{GameViewModelFovs, ViewModelSettings};
use combat_arms::{game::options::CaOptionsPlugin, ui::options_apply::CaOptionsApplyPlugin};
use party_kit::{PartyMousePlugin, controls::cvar_value};

/// The party's settings, as far as they are saved: a console, the shell's
/// mouse, Combat Arms' options (and what applies them), the party's mouse
/// and both games' view-model FOV; `config` is the config.cfg to read at
/// start (None: first start).
fn app(config: Option<&str>) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ConsolePlugin))
        .init_resource::<MouseSettings>()
        .insert_resource(combat_arms::game::view_model::settings());
    mouse_cvars(&mut app);
    app.add_plugins((SoundOptionsPlugin, GammaPlugin));
    app.add_plugins((CaOptionsPlugin, CaOptionsApplyPlugin, PartyMousePlugin));
    combat_arms::game::view_model::register_fov_cvar(&mut app);
    party_kit::add_view_model_fovs(&mut app);
    if let Some(text) = config {
        let lines: Vec<String> = text.lines().map(String::from).collect();
        app.add_systems(Startup, move |mut console: ResMut<Console>| {
            for l in &lines {
                console.submit(l.clone());
            }
        });
    }
    app.update();
    app.update();
    app
}

fn value(app: &mut App, name: &str) -> String {
    cvar_value(app.world_mut(), name).unwrap_or_default()
}

fn run(app: &mut App, line: &str) {
    app.world_mut().resource_mut::<Console>().submit(line);
    app.update();
    app.update();
}

#[test]
fn sensitivity_fov_and_binds_survive_a_restart() {
    let mut first = app(None);
    // First start: the party's defaults.
    assert_eq!(value(&mut first, "sensitivity"), "1");
    assert_eq!(value(&mut first, "viewmodel_fov"), "54");
    for line in [
        "sensitivity 2.5",
        "viewmodel_fov 90",
        "ca_viewmodel_fov 100",
        "bind q lastinv",
        "bind x +duck",
        "bindgame cs_source shift +speed",
        "bindgame combat_arms c +walk",
    ] {
        run(&mut first, line);
    }
    // Combat Arms' options changing (a setting touched) must not take the
    // mouse from the shell, as they did.
    first.world_mut().resource_mut::<combat_arms::game::options::CaMouseLook>().sensitivity = 40;
    first.update();
    assert_eq!(value(&mut first, "sensitivity"), "2.5", "Combat Arms' options leave the shell's mouse alone");
    let config = config_text(first.world_mut());
    assert!(config.contains("sensitivity 2.5"), "{config}");
    assert!(config.contains("viewmodel_fov 90") && config.contains("ca_viewmodel_fov 100"), "{config}");

    // The next start reads it back.
    let mut second = app(Some(&config));
    assert_eq!(value(&mut second, "sensitivity"), "2.5");
    assert_eq!(second.world().resource::<MouseSettings>().sensitivity, 2.5);
    assert_eq!(value(&mut second, "viewmodel_fov"), "90");
    assert_eq!(value(&mut second, "ca_viewmodel_fov"), "100");
    let fovs = second.world().resource::<GameViewModelFovs>().clone();
    let base = *second.world().resource::<ViewModelSettings>();
    assert_eq!(fovs.settings_for(base, Some("cs_source")).fov, 90.0);
    assert_eq!(fovs.settings_for(base, Some("combat_arms")).fov, 100.0);
    let console = second.world().resource::<Console>();
    assert_eq!(console.binds.get("q").map(String::as_str), Some("lastinv"));
    assert_eq!(console.binds.get("x").map(String::as_str), Some("+duck"));
    assert_eq!(console.layers["cs_source"].get("shift").map(String::as_str), Some("+speed"));
    assert_eq!(console.layers["combat_arms"].get("c").map(String::as_str), Some("+walk"));
}

#[test]
fn the_fov_stays_in_its_range() {
    let mut app = app(None);
    run(&mut app, "viewmodel_fov 30");
    assert_eq!(value(&mut app, "viewmodel_fov"), "54", "CS:S's own is the least");
    run(&mut app, "viewmodel_fov 500");
    assert_eq!(value(&mut app, "viewmodel_fov"), "110");
    run(&mut app, "ca_viewmodel_fov 10");
    assert_eq!(value(&mut app, "ca_viewmodel_fov"), "63.74");
}

/// A fresh profile hears the game at half volume, whatever Combat Arms'
/// unloaded profile says (its `soundvolume` 0 had muted the party); a
/// volume above 1 (up to 200 %) is saved, read back, left alone by
/// Combat Arms' options changing, and reaches the mixer above 1. The
/// brightness and the music volume likewise.
#[test]
fn volumes_and_brightness_are_the_shells() {
    let mut first = app(None);
    assert_eq!(value(&mut first, "volume"), "0.5", "a fresh profile starts at half volume");
    assert!((first.world().resource::<GlobalVolume>().volume.to_linear() - 0.5).abs() < 1e-6);
    for line in ["volume 1.6", "snd_musicvolume 1.4", "mat_monitorgamma 1.8"] {
        run(&mut first, line);
    }
    // Combat Arms' options changing must not take them from the shell.
    {
        let mut levels = first.world_mut().resource_mut::<combat_arms::game::options::CaSoundLevels>();
        levels.master = 0.0;
        levels.music = 0.0;
    }
    first.world_mut().resource_mut::<combat_arms::game::options::CaGamma>().0 = 1.0;
    first.update();
    assert_eq!(value(&mut first, "volume"), "1.6", "Combat Arms' options leave the shell's volume alone");
    assert_eq!(value(&mut first, "mat_monitorgamma"), "1.8");
    let config = config_text(first.world_mut());
    assert!(config.contains("volume \"1.6\"") || config.contains("volume 1.6"), "{config}");

    let mut second = app(Some(&config));
    assert_eq!(value(&mut second, "volume"), "1.6");
    assert_eq!(value(&mut second, "snd_musicvolume"), "1.4");
    assert_eq!(value(&mut second, "mat_monitorgamma"), "1.8");
    let gain = second.world().resource::<GlobalVolume>().volume.to_linear();
    assert!((gain - 1.6).abs() < 1e-6, "the mixer's master is above 1: {gain}");
    let music = second.world().resource::<SoundMix>().gain(&format!("ca:bgm:a.wav{}", chimera_present::audio::live_sound::MUSIC_TAG));
    assert!((music - 1.4).abs() < 1e-6, "Combat Arms' party music follows snd_musicvolume: {music}");
}

/// Every cvar the party's Options page sets is saved in config.cfg (an
/// option that isn't comes back at its default after a restart, as
/// `fps_max` and `party_music` did): each row's owner registered as the
/// client registers it, on the party's simulation.
#[test]
fn every_option_is_archived() {
    use chimera_client::client::{hit_marker::HitMarkerPlugin, hud, perf, swap_effect::SwapEffectPlugin, video::VideoPlugin};
    use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
    use combat_arms::harness::CaSim;
    let sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, party_kit::PartyPartsPlugin))
            .add_plugins((VideoPlugin, SoundOptionsPlugin, GammaPlugin, HitMarkerPlugin, SwapEffectPlugin))
            // `cl_chat` (the chat's).
            .add_plugins(chimera_client::client::chat::GameMessagesPlugin)
            .init_resource::<MouseSettings>();
        mouse_cvars(app);
        perf::frame_limit_cvar(app);
        hud::default_radars(app, false);
        halo_ce::ui::hud::filter_cvar(app);
        // "Use my installed games" (the network's).
        chimera_net::net::assets::install_setting(app);
        // "Ask before following the party leader" (the party's).
        chimera_net::net::party::ask_setting(app);
    });
    let console = sim.app.world().resource::<Console>();
    let tabs = party_kit::esc_menu::option_tabs_for(true);
    assert!(!tabs.is_empty());
    for (tab, settings) in tabs {
        for s in settings {
            let cvar = console.cvar(&s.cvar).unwrap_or_else(|| panic!("{tab}: {} has no cvar here", s.cvar));
            assert!(cvar.archive, "{tab}: {} ({}) isn't saved in config.cfg", s.label, s.cvar);
        }
    }
}

/// The Options' volumes go to 200 % and show as percentages; the
/// settings audit's labels: game-specific rows name their games,
/// brightness is "Gamma" (lower is brighter), Combat Arms' loop is named.
#[test]
fn options_rows_read_right() {
    let tabs = party_kit::esc_menu::option_tabs_for(true);
    let row = |cvar: &str| tabs.iter().flat_map(|(_, s)| s.iter()).find(|s| s.cvar == cvar).unwrap_or_else(|| panic!("{cvar}")).clone();
    for cvar in ["volume", "snd_musicvolume", "snd_uivolume"] {
        let s = row(cvar);
        assert!(matches!(s.pick, party_kit::host::Pick::Range { min: 0.0, max: 2.0, .. }), "{cvar}: 0 to 200 %");
        assert_eq!(s.shown("1.6"), "160%");
        assert_eq!(s.shown("0.5"), "50%");
    }
    assert_eq!(row("mat_monitorgamma").label, "Gamma");
    // The hit marker, others' voice lines and the radar have no row (the
    // user's call): the party keeps its defaults (`party_defaults_hold`).
    for gone in ["cl_hitmarker", "party_voice_lines_3d", "hud_radar"] {
        assert!(tabs.iter().flat_map(|(_, s)| s.iter()).all(|s| s.cvar != gone), "{gone} has no Options row");
    }
    let party: Vec<&str> = tabs.iter().find(|(n, _)| n == "Party").map(|(_, s)| s.iter().map(|s| s.cvar.as_str()).collect()).unwrap_or_default();
    // The chat's row joined the page with the Signal chat (ugg-ui M10).
    assert_eq!(party, ["party_swap_effect", "party_music", "party_ask_follow", "cl_chat"], "the Party page keeps the swap effect, the music and the follow choice");
    let music = row("party_music");
    assert_eq!(music.shown("2"), "Combat Arms' loop");
    assert_eq!(row("fov_desired").shown("90"), "90", "a plain range stays a number");
}

/// A fresh profile's volumes as the Options page shows them: game 50 %,
/// music 100 %, UI 80 % (not 5 %: the 0.05 is a step, never a default);
/// the start's repair leaves any volume not both 0 alone (a 0.05 a
/// player set stays theirs) and puts the bug's 0 and 0 back to 50 %.
#[test]
fn a_fresh_profile_hears_half_volume() {
    let mut app = app(None);
    let tabs = party_kit::esc_menu::option_tabs_for(true);
    let row = |cvar: &str| tabs.iter().flat_map(|(_, s)| s.iter()).find(|s| s.cvar == cvar).unwrap().clone();
    for (cvar, shown) in [("volume", "50%"), ("snd_musicvolume", "100%"), ("snd_uivolume", "80%")] {
        let v = value(&mut app, cvar);
        assert_eq!(row(cvar).shown(&v), shown, "{cvar} = {v}");
    }
    use chimera_client::client::console::repair_options;
    assert!(repair_options(Some(0.05), Some(1.0), None).is_empty(), "a player's 5 % is theirs");
    assert_eq!(repair_options(Some(0.0), Some(0.0), None), ["volume 0.5", "snd_musicvolume 1"]);
}

/// The settings without an Options row keep the party's defaults: the
/// hit marker on, the radars off (others' voice lines on: voice_lines.rs).
#[test]
fn the_party_defaults_hold_without_their_rows() {
    use chimera_client::client::hit_marker::{HitMarkerPlugin, HitMarkerSetting};
    use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
    use combat_arms::harness::CaSim;
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, party_kit::PartyPartsPlugin, HitMarkerPlugin, party_kit::PartyHudPlugin));
    });
    sim.ticks(2);
    let w = sim.app.world_mut();
    assert_eq!(cvar_value(w, "cl_hitmarker").as_deref(), Some("1"), "the hit marker on");
    assert_eq!(w.resource::<HitMarkerSetting>().on, 1);
    assert_eq!(w.resource::<Console>().cvar("cl_hitmarker").unwrap().default, "1", "1 is the party's default (a config leaves it out)");
    assert_eq!(cvar_value(w, "hud_radar").as_deref(), Some("0"), "the radars off");
}
