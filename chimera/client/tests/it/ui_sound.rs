//! The `ui` bus (`client::ui_sound`) and the shell's screens' cues
//! (`client::ui_cues`): each screen's event plays its cue under a theme
//! with UI sound on and nothing under the shell's theme; `snd_uivolume`
//! scales the bus while a cue plays; the bus ducks under an announcer
//! voice; hover is one voice (a new one cuts the last); a rung up plays
//! the render nearest its place on the ladder, the knife on the last
//! rung, the chain over a quick second one; the music loop on the music
//! bus.

use bevy::prelude::*;
use chimera_client::client::{
    announcer::AnnouncerVoice,
    match_end::{MatchEndView, MatchEndVote},
    prematch_lobby::{LobbyView, LobbyViewRow},
    race_chip::BoardOpen,
    ui_cues::UiCuesPlugin,
    ui_sound::{UiCue, UiMusic, UiSoundPlugin, UiSounds, UiVoices},
};
use chimera_core::core::LocalPlayer;
use chimera_net::prematch::status;
use chimera_present::audio::live_sound::SoundMix;
use chimera_sim::{
    SimBasePlugins,
    minigame::ladder::{Ladder, Rung},
};
use chimera_ui::Theme;
use chimera_world::harness::Sim;

const TABLE: &str = r#"
[bus]
duck_db = 4
[event.ui_hover]
voices = 1
cents = 40
db = 1.2
[event.ui_select]
[event.ui_slider]
rate = "value"
[event.lobby_ready]
[event.lobby_unready]
[event.lobby_peer]
[event.count_tick]
[event.count_tick_3]
[event.count_tick_2]
[event.count_tick_1]
[event.count_go]
[event.rank_up]
file = "rank_p{nn}"
grid = 16
[event.rank_knife]
file = "rank_14_knife"
[event.rank_chain]
[event.tab_open]
on = false
[event.tab_close]
on = false
[event.end_reveal]
[event.vote_cast_1]
[event.vote_cast_2]
[event.vote_cast_3]
[event.vote_cast_4]
[event.vote_other]
[event.vote_lock]
[event.menu_loop]
file = "loop"
loop = true
"#;

const SIGNAL: &str = "name = \"signal\"\n[flags.sound]\nui = true\n";

fn sim(signal: bool) -> Sim {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((UiSoundPlugin, UiCuesPlugin));
    });
    let theme = if signal { Theme::from_toml(SIGNAL).unwrap() } else { Theme::shell_default() };
    sim.app.insert_resource(theme);
    sim.app.insert_resource(UiSounds::silent(TABLE).unwrap());
    sim.app.update();
    sim
}

fn events(sim: &Sim) -> Vec<String> {
    sim.app.world().resource::<UiVoices>().events().iter().map(|e| e.to_string()).collect()
}

fn files(sim: &Sim) -> Vec<String> {
    sim.app.world().resource::<UiVoices>().files().iter().map(|e| e.to_string()).collect()
}

fn row(name: &str, status: u8, local: bool) -> LobbyViewRow {
    LobbyViewRow { name: name.into(), status, local, ..default() }
}

fn lobby(rows: Vec<LobbyViewRow>, countdown: Option<u32>) -> LobbyView {
    LobbyView { shown: true, rows, countdown, ..default() }
}

/// Every screen's events, played through: the lobby, a rung up, Tab, the
/// match end and its vote.
fn play_screens(sim: &mut Sim) {
    let w = sim.app.world_mut();
    w.insert_resource(lobby(vec![row("You", status::NOT_READY, true), row("Ann", status::NOT_READY, false)], None));
    sim.app.update();
    let w = sim.app.world_mut();
    w.insert_resource(lobby(vec![row("You", status::READY, true), row("Ann", status::READY, false)], None));
    sim.app.update();
    for n in [10, 4, 3, 2, 1] {
        let rows = vec![row("You", status::READY, true), row("Ann", status::READY, false)];
        sim.app.world_mut().insert_resource(lobby(rows, Some(n)));
        sim.app.update();
    }
    sim.app.world_mut().insert_resource(LobbyView::default());
    sim.app.update();

    // A rung up on an 8-rung ladder.
    sim.app.world_mut().insert_resource(Ladder { rungs: vec!["a:g"; 8], ..default() });
    let me = sim.app.world_mut().spawn((LocalPlayer, Rung(0))).id();
    sim.app.update();
    sim.app.world_mut().get_mut::<Rung>(me).unwrap().0 = 3;
    sim.app.update();

    // Tab (off in the table: silent).
    sim.app.world_mut().insert_resource(BoardOpen(true));
    sim.app.update();
    sim.app.world_mut().insert_resource(BoardOpen(false));
    sim.app.update();

    // The match end: the reveal, your vote, someone else's, the lock.
    let vote = |mine: Option<usize>, counts: Vec<u32>, winner: Option<usize>| MatchEndView {
        shown: true,
        winner: Some("Ann".into()),
        vote: Some(MatchEndVote { maps: vec!["a".into(), "b".into(), "c".into()], counts, mine, winner }),
        ..default()
    };
    sim.app.world_mut().insert_resource(vote(None, vec![0, 0, 0], None));
    sim.app.update();
    sim.app.world_mut().insert_resource(vote(Some(1), vec![0, 1, 0], None));
    sim.app.update();
    sim.app.world_mut().insert_resource(vote(Some(1), vec![1, 1, 0], None));
    sim.app.update();
    sim.app.world_mut().insert_resource(vote(Some(1), vec![1, 1, 0], Some(1)));
    sim.app.update();
}

#[test]
fn each_screen_event_plays_its_cue_under_signal() {
    let mut sim = sim(true);
    play_screens(&mut sim);
    assert_eq!(
        events(&sim),
        [
            "lobby_ready",
            "lobby_peer",
            "count_tick",
            "count_tick",
            "count_tick_3",
            "count_tick_2",
            "count_tick_1",
            "count_go",
            "rank_up",
            "end_reveal",
            "vote_cast_2",
            "vote_other",
            "vote_lock",
        ]
    );
    assert!(files(&sim).contains(&"rank_p07".to_string()), "rung 3 of 8: p = 3/7, render 7: {:?}", files(&sim));
}

#[test]
fn nothing_plays_under_the_shell_theme() {
    let mut sim = sim(false);
    play_screens(&mut sim);
    sim.app.world_mut().write_message(UiCue::new("ui_select"));
    sim.app.world_mut().insert_resource(UiMusic(Some("menu_loop".into())));
    sim.app.update();
    assert!(events(&sim).is_empty(), "{:?}", events(&sim));
    assert!(sim.app.world().resource::<UiVoices>().music.is_none());
}

#[test]
fn the_ui_volume_scales_the_bus_and_an_announcer_voice_ducks_it() {
    let mut sim = sim(true);
    sim.app.world_mut().resource_mut::<SoundMix>().ui = 0.5;
    sim.app.world_mut().write_message(UiCue::new("ui_select"));
    sim.app.update();
    let heard = |sim: &Sim| {
        let v = sim.app.world().resource::<UiVoices>();
        let voice = v.voices.iter().find(|v| v.event == "ui_select").expect("playing");
        (voice.gain, voice.heard())
    };
    let (gain, now) = heard(&sim);
    assert!((now - gain * 0.5).abs() < 1e-5, "{now} for {gain} at 0.5");
    // The volume moves while it plays.
    sim.app.world_mut().resource_mut::<SoundMix>().ui = 0.25;
    sim.app.update();
    let (gain, now) = heard(&sim);
    assert!((now - gain * 0.25).abs() < 1e-5, "{now} for {gain} at 0.25");
    // An announcer voice: 4 dB down.
    let voice = sim.app.world_mut().spawn(AnnouncerVoice).id();
    sim.app.update();
    let (gain, now) = heard(&sim);
    let duck = 10f32.powf(-4.0 / 20.0);
    assert!((now - gain * 0.25 * duck).abs() < 1e-5, "ducked: {now}");
    sim.app.world_mut().despawn(voice);
    sim.app.update();
    let (gain, now) = heard(&sim);
    assert!((now - gain * 0.25).abs() < 1e-5, "back up: {now}");
}

#[test]
fn hover_is_one_voice() {
    let mut sim = sim(true);
    for _ in 0..3 {
        sim.app.world_mut().write_message(UiCue::new("ui_hover"));
        sim.app.update();
    }
    let v = sim.app.world().resource::<UiVoices>();
    assert_eq!(v.events().iter().filter(|e| **e == "ui_hover").count(), 3, "each hover plays");
    assert_eq!(v.voices.iter().filter(|v| v.event == "ui_hover").count(), 1, "a new hover cuts the last");
    // Two selects at once both sound (two voices).
    sim.app.world_mut().write_message(UiCue::new("ui_select"));
    sim.app.world_mut().write_message(UiCue::new("ui_select"));
    sim.app.update();
    let v = sim.app.world().resource::<UiVoices>();
    assert_eq!(v.voices.iter().filter(|v| v.event == "ui_select").count(), 2);
    // Hover's spread stays within its 40 cents.
    for p in v.played.iter().filter(|p| p.event == "ui_hover") {
        assert!((p.speed.log2() * 1200.0).abs() <= 40.0 + 1e-3, "{}", p.speed);
    }
}

#[test]
fn the_slider_rate_follows_its_value() {
    let mut sim = sim(true);
    for v in [0.0, 0.5, 1.0] {
        sim.app.world_mut().write_message(UiCue::at("ui_slider", v));
        sim.app.update();
    }
    let speeds: Vec<f32> = sim.app.world().resource::<UiVoices>().played.iter().map(|p| p.speed).collect();
    assert_eq!(speeds, [0.5, 1.25, 2.0]);
}

#[test]
fn rank_up_picks_the_render_by_the_ladders_length() {
    for (rungs, to, want) in [
        (8, 1, "rank_p02"),
        (8, 6, "rank_p14"),
        (8, 7, "rank_14_knife"),
        (14, 6, "rank_p07"),
        (14, 13, "rank_14_knife"),
        (30, 15, "rank_p08"),
        (30, 28, "rank_p15"),
        (30, 29, "rank_14_knife"),
    ] {
        let mut sim = sim(true);
        sim.app.world_mut().insert_resource(Ladder { rungs: vec!["a:g"; rungs], ..default() });
        let me = sim.app.world_mut().spawn((LocalPlayer, Rung(to - 1))).id();
        sim.app.update();
        sim.app.world_mut().get_mut::<Rung>(me).unwrap().0 = to;
        sim.app.update();
        assert_eq!(files(&sim), [want], "{rungs} rungs, up to {to}");
    }
    // A second rung up at once chains.
    let mut sim = sim(true);
    sim.app.world_mut().insert_resource(Ladder { rungs: vec!["a:g"; 14], ..default() });
    let me = sim.app.world_mut().spawn((LocalPlayer, Rung(0))).id();
    sim.app.update();
    for r in [1, 2] {
        sim.app.world_mut().get_mut::<Rung>(me).unwrap().0 = r;
        sim.app.update();
    }
    assert_eq!(events(&sim), ["rank_up", "rank_up", "rank_chain"]);
}

#[test]
fn the_menu_loop_plays_on_the_music_bus() {
    let mut sim = sim(true);
    sim.app.world_mut().resource_mut::<SoundMix>().music = 0.5;
    sim.app.world_mut().insert_resource(UiMusic(Some("menu_loop".into())));
    sim.app.update();
    let v = sim.app.world().resource::<UiVoices>();
    let m = v.music.as_ref().expect("the loop plays");
    assert_eq!(m.file, "loop");
    assert!((m.heard() - 0.5).abs() < 1e-5, "at snd_musicvolume: {}", m.heard());
    assert!(m.ends.is_infinite(), "it loops");
    sim.app.world_mut().insert_resource(UiMusic(None));
    sim.app.update();
    assert!(sim.app.world().resource::<UiVoices>().music.is_none(), "stopped");
}
