//! specs/combat_arms/music_ambience.md's logic test cases (`t01_` is test
//! case 1) that need no install: the music player's fades and the menu
//! playlist (1-5), the cues of a match from its facts (6-11), map music
//! triggers (12, 13: on a world built in code, and over the network),
//! ambient map sounds (14-17) and the volume channels (18). The install
//! cases 19-22 are in heavy::music_ambience.

use bevy::prelude::*;
use chimera_core::core::LocalPlayer;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{
    game::{
        TICK_INTERVAL, movement,
        music::{
            MusicClock,
            ambient::{self, CaAmbient, SoundFx},
            data::MusicData,
            director::{CaMusic, MusicFacts, MusicFiles, Screen, observe},
            mix::CaVolumes,
            playlist::Playlist,
            trigger::{CaMapMusic, CaMusicOn},
        },
        objects::runtime::Props,
        perks::CaPerkState,
        rules::Phase,
        world,
    },
    harness::{CaSim, with_mode},
};
use lithtech_formats::world::{Object, Property, Value, World};

fn close(a: f32, b: f32, tol: f32, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a}, expected {b}");
}

fn data() -> &'static MusicData {
    MusicData::built_in()
}

/// A player of ours with every file of the table present.
fn all_files() -> CaMusic {
    CaMusic::new(data()).with_table_files()
}

/// A player that can open only `files` (path, length in seconds).
fn only(files: &[(&str, f64)]) -> CaMusic {
    let mut m = CaMusic::new(data());
    for (f, len) in files {
        m.files.insert(f, Some(len * 1000.0));
    }
    m
}

/// The gain of `file` in the streams, music master 1.
fn gain_of(m: &CaMusic, file: &str) -> Option<f32> {
    m.mixed(1.0)
        .iter()
        .find(|s| s.file.eq_ignore_ascii_case(file))
        .map(|s| s.gain)
}

/// Step frames every `dt` ms from `from` to `to`.
fn run(m: &mut CaMusic, from: f64, to: f64, dt: f64) {
    let mut t = from;
    while t <= to {
        m.frame(t);
        t += dt;
    }
}

// ---------------------------------------------------------------------------
// 1-5. The player and the playlist

#[test]
fn t01_fade_in() {
    let first = data().playlist[0].file.clone();
    let mut m = only(&[(&first, 143.313)]);
    for (t, want) in [
        (0.0, 0.0),
        (2_500.0, 0.25),
        (5_000.0, 0.5),
        (10_000.0, 1.0),
        (30_000.0, 1.0),
    ] {
        m.frame(t);
        close(gain_of(&m, &first).unwrap(), want, 1e-5, &format!("t = {t}"));
    }
    assert!(m.player.primary.as_ref().unwrap().looping, "one track loops");
}

#[test]
fn t02_cross_fade_timing() {
    let (a, b) = (data().playlist[0].file.clone(), data().playlist[1].file.clone());
    let mut m = only(&[(&a, 60.0), (&b, 50.0)]);
    run(&mut m, 0.0, 49_900.0, 100.0);
    assert!(gain_of(&m, &b).is_none(), "B not yet");
    m.frame(50_000.0);
    assert_eq!(
        m.player.secondary.as_ref().unwrap().file,
        b,
        "B starts with 10 s left of A"
    );
    m.frame(52_500.0);
    close(gain_of(&m, &a).unwrap(), 0.75, 1e-5, "A at 52.5 s");
    close(gain_of(&m, &b).unwrap(), 0.25, 1e-5, "B at 52.5 s");
    run(&mut m, 52_600.0, 60_000.0, 100.0);
    assert!(gain_of(&m, &a).is_none(), "A is closed at 60 s");
    close(gain_of(&m, &b).unwrap(), 1.0, 1e-5, "B at 60 s");
    // B's end is 40 s into it: the next track (A) starts at 50 + 40 s.
    run(&mut m, 60_100.0, 89_900.0, 100.0);
    assert_eq!(m.player.secondary, None, "not before");
    m.frame(90_000.0);
    assert_eq!(m.player.secondary.as_ref().unwrap().file, a, "A again at 90 s");
}

#[test]
fn t03_rotation_order() {
    let mut p = Playlist::new("v", ["a", "b", "c"].map(|n| (n.to_string(), format!("bgm/{n}.mp3"))));
    p.entries[1].ticked = false;
    p.set_jukebox(&["J".to_string()]);
    let all = |_: &str| true;
    let order = |p: &mut Playlist, playable: &dyn Fn(&str) -> bool| -> Vec<String> {
        (0..4).map(|_| p.next(playable, |_| 0).unwrap().0).collect()
    };
    let want = ["bgm/a.mp3", "bgm/c.mp3", "../jukebox/j.mp3", "bgm/a.mp3"];
    assert_eq!(order(&mut p.clone(), &all), want);
    // B's file missing (and ticked): the same order.
    let mut q = p.clone();
    q.entries[1].ticked = true;
    assert_eq!(order(&mut q, &|f| f != "bgm/b.mp3"), want);
}

#[test]
fn t04_random_draw() {
    let all = |_: &str| true;
    let mut p = Playlist::new("v", ["a", "b", "c"].map(|n| (n.to_string(), n.to_string())));
    p.random = true;
    for (k, want) in [(0, "b"), (1, "c"), (2, "a")] {
        p.cursor = 0;
        assert_eq!(p.next(&all, |_| k).unwrap().0, want, "k = {k}");
    }
    // k = 2 lands on the playing track: the start is ignored as the same
    // file, and the next frame draws again.
    let (a, b) = (data().playlist[0].file.clone(), data().playlist[1].file.clone());
    let mut m = only(&[(&a, 100.0), (&b, 100.0)]);
    m.playlist.random = true;
    m.frame(0.0);
    let first = m.player.primary.as_ref().unwrap().file.clone();
    let mut t = 90_000.0;
    // Every draw is made; whatever the frames give, the player never
    // starts its own file over, and some frame crosses to the other.
    let mut crossed = false;
    for _ in 0..200 {
        m.frame(t);
        if m.player.secondary.as_ref().is_some_and(|s| s.file != first) {
            crossed = true;
            break;
        }
        assert!(m.player.secondary.is_none(), "the same file is never started over");
        t += 16.0;
    }
    assert!(crossed, "a different track came up");
}

#[test]
fn t05_music_slider() {
    let (a, b) = (data().playlist[0].file.clone(), data().playlist[1].file.clone());
    let mut m = only(&[(&a, 60.0), (&b, 50.0)]);
    run(&mut m, 0.0, 50_000.0, 100.0);
    m.frame(55_000.0);
    let mut v = CaVolumes::default();
    v.music_volume = 60;
    let gains: Vec<f32> = m.mixed(v.music()).iter().map(|s| s.gain).collect();
    assert_eq!(gains.len(), 2);
    for g in &gains {
        close(*g, 0.3, 1e-5, "each stream at T/2 with music 60");
    }
    // Switched off: silent, the tracks advance anyway.
    v.music_on = false;
    assert!(m.mixed(v.music()).iter().all(|s| s.gain == 0.0));
    run(&mut m, 55_100.0, 62_000.0, 100.0);
    v.music_on = true;
    let now = m.mixed(v.music());
    assert_eq!(now.len(), 1);
    close(now[0].gain, 0.6, 1e-5, "on again");
    let pos = m.player.primary.as_ref().unwrap().position(62_000.0, Some(50_000.0));
    assert!(
        pos > 11_900.0 && pos < 12_100.0,
        "B's position moved on while muted: {pos}"
    );
}

// ---------------------------------------------------------------------------
// 6-11. Moments of a match

/// Step one frame of facts: the moments, then the player's frame.
fn step(m: &mut CaMusic, prev: &mut MusicFacts, now: MusicFacts, ms: f64) {
    observe(m, prev, &now, ms);
    *prev = now;
    m.frame(ms);
}

fn playing(m: &CaMusic) -> Vec<String> {
    m.mixed(1.0).into_iter().map(|s| s.file).collect()
}

/// The facts of a team mode's round.
fn match_facts(mode: &str, phase: Phase, warmup: bool) -> MusicFacts {
    MusicFacts {
        screen: Screen::Match,
        mode: mode.into(),
        phase,
        warmup,
        alive: true,
        my_team: 1,
        ..Default::default()
    }
}

const SETUP: Phase = Phase::Setup { until: 0.0 };

#[test]
fn t06_room_to_match() {
    let mut m = all_files();
    let mut prev = MusicFacts::default();
    run(&mut m, 0.0, 5_000.0, 100.0);
    assert!(!playing(&m).is_empty(), "the playlist runs in the menu");
    // Room START: the playlist is cut, the loading track at full volume.
    let loading = data().cues.loading.clone();
    step(
        &mut m,
        &mut prev,
        MusicFacts {
            screen: Screen::Loading,
            ..Default::default()
        },
        6_000.0,
    );
    assert_eq!(playing(&m), [loading.clone()]);
    close(gain_of(&m, &loading).unwrap(), 1.0, 1e-6, "loading track");
    // Silent 2.0 s later: it has played out and stays loaded.
    let len = m.files.length_ms(&loading).unwrap();
    assert!(!m.player.primary.as_ref().unwrap().ended(6_000.0 + len - 1.0, Some(len)));
    assert!(
        m.player
            .primary
            .as_ref()
            .unwrap()
            .ended(6_000.0 + len + 0.01, Some(len))
    );
    step(
        &mut m,
        &mut prev,
        MusicFacts {
            screen: Screen::Loading,
            ..Default::default()
        },
        8_100.0,
    );
    assert_eq!(playing(&m), [loading], "no menu update runs on the loading screen");
    // The loading screen closes; the first real round start: the sound
    // and the mode's track at full volume.
    let sound_mark = m.heard.len();
    step(&mut m, &mut prev, match_facts("TMM", Phase::Waiting, false), 9_000.0);
    assert!(
        playing(&m).is_empty(),
        "stopped as the loading screen closes (TMM has no mode cue)"
    );
    step(&mut m, &mut prev, match_facts("TMM", SETUP, false), 10_000.0);
    assert_eq!(m.heard[sound_mark..], ["RoundStartSound"]);
    let track = data().mode_track("TMM", None).unwrap().0.to_string();
    assert_eq!(playing(&m), [track.clone()]);
    close(gain_of(&m, &track).unwrap(), 1.0, 1e-6, "mode track at full volume");
    // Round 2: the sound only, no new music.
    step(&mut m, &mut prev, match_facts("TMM", Phase::Live, false), 11_000.0);
    step(
        &mut m,
        &mut prev,
        match_facts("TMM", Phase::RoundOver { until: 0.0 }, false),
        12_000.0,
    );
    step(
        &mut m,
        &mut prev,
        match_facts("TMM", Phase::Intermission { until: 0.0 }, false),
        13_000.0,
    );
    m.stop();
    let marks = m.heard.len();
    step(&mut m, &mut prev, match_facts("TMM", SETUP, false), 14_000.0);
    assert_eq!(m.heard[marks..], ["RoundStartSound"]);
    assert!(playing(&m).is_empty(), "no music at the second round start");
}

#[test]
fn t07_warm_up() {
    let mut m = all_files();
    let mut prev = MusicFacts {
        screen: Screen::Match,
        mode: "TMM".into(),
        ..Default::default()
    };
    step(&mut m, &mut prev, match_facts("TMM", SETUP, true), 1_000.0);
    assert_eq!(m.heard, ["FirstSpawnSound"], "only the warm-up's sound");
    assert!(playing(&m).is_empty(), "no mode music in the warm-up");
    step(&mut m, &mut prev, match_facts("TMM", Phase::Live, true), 2_000.0);
    step(
        &mut m,
        &mut prev,
        match_facts("TMM", Phase::Intermission { until: 0.0 }, true),
        3_000.0,
    );
    step(&mut m, &mut prev, match_facts("TMM", SETUP, false), 4_000.0);
    assert_eq!(m.heard, ["FirstSpawnSound", "RoundStartSound"]);
    assert_eq!(playing(&m), [data().mode_track("TMM", None).unwrap().0.to_string()]);
}

#[test]
fn t08_dead_at_round_start() {
    let mut m = all_files();
    let mut prev = MusicFacts {
        screen: Screen::Match,
        mode: "TMM".into(),
        ..Default::default()
    };
    let dead = |phase| MusicFacts {
        alive: false,
        ..match_facts("TMM", phase, false)
    };
    step(&mut m, &mut prev, dead(SETUP), 1_000.0);
    assert!(m.heard.is_empty(), "no RoundStartSound");
    assert!(playing(&m).is_empty(), "no mode music");
    // Alive later in the map: the first-round start is spent.
    step(&mut m, &mut prev, match_facts("TMM", Phase::Live, false), 2_000.0);
    step(
        &mut m,
        &mut prev,
        match_facts("TMM", Phase::Intermission { until: 0.0 }, false),
        3_000.0,
    );
    step(&mut m, &mut prev, match_facts("TMM", SETUP, false), 4_000.0);
    assert_eq!(m.heard, ["RoundStartSound"]);
    assert!(
        playing(&m).is_empty(),
        "the mode music does not start later in that map"
    );
}

#[test]
fn t09_quarantine_end_cue() {
    let mut m = all_files();
    let mut prev = MusicFacts {
        screen: Screen::Match,
        mode: "QRT".into(),
        ..Default::default()
    };
    let live = |left: f64, warmup| MusicFacts {
        round_left: Some(left),
        ..match_facts("QRT", Phase::Live, warmup)
    };
    step(&mut m, &mut prev, match_facts("QRT", SETUP, true), 0.0);
    // The warm-up never triggers it.
    step(&mut m, &mut prev, live(21.0, true), 1_000.0);
    assert!(!playing(&m).contains(&data().cues.quarantine_end));
    step(
        &mut m,
        &mut prev,
        match_facts("QRT", Phase::Intermission { until: 0.0 }, true),
        2_000.0,
    );
    step(&mut m, &mut prev, match_facts("QRT", SETUP, false), 3_000.0);
    let end = data().cues.quarantine_end.clone();
    for (t, left) in [(4_000.0, 23.0), (5_000.0, 22.5)] {
        step(&mut m, &mut prev, live(left, false), t);
        assert!(!playing(&m).contains(&end), "{left} s left: not yet");
    }
    step(&mut m, &mut prev, live(21.0, false), 6_000.0);
    assert_eq!(playing(&m), [end.clone()], "the music is cut and the end cue plays");
    close(gain_of(&m, &end).unwrap(), 1.0, 1e-6, "full volume");
    // Once: a later frame in the window doesn't restart it.
    m.stop();
    step(&mut m, &mut prev, live(20.5, false), 7_000.0);
    assert!(playing(&m).is_empty(), "once per round");
}

#[test]
fn t09b_quarantine_mode_music_fades_in() {
    let mut m = all_files();
    let mut prev = MusicFacts {
        screen: Screen::Loading,
        ..Default::default()
    };
    m.room_start(0.0);
    step(&mut m, &mut prev, match_facts("QRT", Phase::Waiting, false), 3_000.0);
    let track = data().cues.quarantine_mode.clone();
    assert_eq!(playing(&m), [track.clone()]);
    m.frame(8_000.0);
    close(gain_of(&m, &track).unwrap(), 0.5, 1e-5, "10 s fade-in");
}

#[test]
fn t10_seize_and_secure_cue() {
    let mut m = all_files();
    let mut prev = MusicFacts {
        screen: Screen::Match,
        mode: "HQM".into(),
        ..Default::default()
    };
    let live = |left: Option<f64>| MusicFacts {
        capture_left: left,
        ..match_facts("HQM", Phase::Live, false)
    };
    step(&mut m, &mut prev, match_facts("HQM", SETUP, false), 0.0);
    let cue = data().cues.capture_progress.clone();
    for (t, left) in [(1_000.0, 25.0), (2_000.0, 21.0)] {
        step(&mut m, &mut prev, live(Some(left)), t);
        assert!(!playing(&m).contains(&cue), "{left} s left");
    }
    step(&mut m, &mut prev, live(Some(20.0)), 3_000.0);
    assert_eq!(playing(&m), [cue.clone()], "the cue starts at 20");
    step(&mut m, &mut prev, live(Some(12.0)), 4_000.0);
    assert!(playing(&m).contains(&cue));
    // The capture is broken: silence.
    step(&mut m, &mut prev, live(None), 5_000.0);
    assert!(playing(&m).is_empty(), "stopped when the capture timer stops");
}

#[test]
fn t11_result_screen() {
    let mut m = all_files();
    let mut prev = MusicFacts {
        screen: Screen::Match,
        mode: "TDM".into(),
        ..Default::default()
    };
    step(&mut m, &mut prev, match_facts("TDM", Phase::Live, false), 0.0);
    m.start("bgm/modebgm/elimination.mp3", false, 0.0, 0.0);
    let won = MusicFacts {
        screen: Screen::Result,
        result: Some(Some(1)),
        phase: Phase::MatchOver,
        ..match_facts("TDM", Phase::MatchOver, false)
    };
    let mark = m.heard.len();
    step(&mut m, &mut prev, won.clone(), 10_000.0);
    assert!(playing(&m).is_empty(), "music stops on entering the result screen");
    assert_eq!(m.heard[mark..], ["ResultWinSound"], "once");
    for t in (11..=70).step_by(1) {
        step(&mut m, &mut prev, won.clone(), t as f64 * 1_000.0);
        assert!(playing(&m).is_empty(), "silent for the whole screen: {t}");
    }
    assert_eq!(m.heard[mark..], ["ResultWinSound"]);
    // Back in the room: the playlist fades in over 10 s.
    step(&mut m, &mut prev, MusicFacts::default(), 71_000.0);
    let now = m.mixed(1.0);
    assert_eq!(now.len(), 1);
    close(now[0].gain, 0.0, 1e-6, "from silence");
    m.frame(76_000.0);
    close(m.mixed(1.0)[0].gain, 0.5, 1e-4, "half way");
    m.frame(81_000.0);
    close(m.mixed(1.0)[0].gain, 1.0, 1e-4, "full");
    // A loss and a draw.
    let mut m = all_files();
    let mut prev = MusicFacts {
        screen: Screen::Match,
        mode: "TDM".into(),
        ..Default::default()
    };
    let lost = MusicFacts {
        result: Some(Some(2)),
        ..won.clone()
    };
    step(&mut m, &mut prev, lost, 1_000.0);
    assert_eq!(m.heard, ["ResultLoseSound"]);
    let mut m = all_files();
    let mut prev = MusicFacts {
        screen: Screen::Match,
        mode: "TDM".into(),
        ..Default::default()
    };
    step(
        &mut m,
        &mut prev,
        MusicFacts {
            result: Some(None),
            ..won
        },
        1_000.0,
    );
    assert!(m.heard.is_empty(), "a draw plays none");
}

#[test]
fn round_result_sounds() {
    // Team modes by the winning team, the same for both teams (Quirk 6).
    let mut m = all_files();
    m.round_result("TDM", Some(1), 2);
    m.round_result("TDM", Some(2), 1);
    m.round_result("TDM", None, 1);
    m.round_result("FFA", Some(3), 3);
    m.round_result("QRT", Some(1), 1);
    m.round_result("QRT", Some(2), 1);
    m.round_result("BNT", Some(1), 2);
    m.round_result("BNT", Some(1), 1);
    assert_eq!(
        m.heard,
        [
            "RoundAlphaTeamWinSound",
            "RoundBravoTeamWinSound",
            "QRTHumanWinSound",
            "QRTInfectorWinSound",
            "RoundAlphaTeamWinSound"
        ]
    );
    // Arms Race's last stage: the sound and the looping cue; leaving it
    // stops the cue and the mode's start music comes back (it loops).
    let mut m = all_files();
    let mut prev = MusicFacts {
        screen: Screen::Match,
        mode: "VWRM".into(),
        ..Default::default()
    };
    step(&mut m, &mut prev, match_facts("VWRM", SETUP, false), 0.0);
    step(
        &mut m,
        &mut prev,
        MusicFacts {
            last_stage: 1,
            ..match_facts("VWRM", Phase::Live, false)
        },
        1_000.0,
    );
    let last = data().cues.last_stage.clone();
    assert_eq!(playing(&m), [last.clone()]);
    assert!(m.player.primary.as_ref().unwrap().looping);
    assert!(m.heard.contains(&"last_stage_sound".to_string()));
    step(&mut m, &mut prev, match_facts("VWRM", Phase::Live, false), 2_000.0);
    assert_eq!(playing(&m), [data().mode_track("VWRM", None).unwrap().0.to_string()]);
    // A perk's cue replaces the mode music and it does not come back.
    m.perk_cue(true, 3_000.0);
    assert_eq!(playing(&m), [data().cues.war_cry.clone()]);
}

/// A perk's buff beginning plays its sound file as music; the mode's
/// music does not come back (Quirk 5) - from the facts and from a living
/// player's perk state.
#[test]
fn a_perk_cue_follows_the_buff() {
    let mut m = all_files();
    let mut prev = MusicFacts {
        screen: Screen::Match,
        mode: "TMM".into(),
        ..Default::default()
    };
    step(&mut m, &mut prev, match_facts("TMM", SETUP, false), 0.0);
    assert_eq!(playing(&m), [data().mode_track("TMM", None).unwrap().0.to_string()]);
    step(
        &mut m,
        &mut prev,
        MusicFacts {
            war_cry: true,
            ..match_facts("TMM", Phase::Live, false)
        },
        1_000.0,
    );
    assert_eq!(playing(&m), [data().cues.war_cry.clone()]);
    // Held: nothing more; ended: the mode music stays gone.
    step(
        &mut m,
        &mut prev,
        MusicFacts {
            war_cry: true,
            ..match_facts("TMM", Phase::Live, false)
        },
        2_000.0,
    );
    step(&mut m, &mut prev, match_facts("TMM", Phase::Live, false), 3_000.0);
    assert_eq!(playing(&m), [data().cues.war_cry.clone()]);
    step(
        &mut m,
        &mut prev,
        MusicFacts {
            eagle_eye: true,
            ..match_facts("TMM", Phase::Live, false)
        },
        4_000.0,
    );
    assert_eq!(playing(&m), [data().cues.eagle_eye.clone()]);

    // The same from a player's perk state.
    let mut sim = sim_with(vec![], "TMM");
    let (p, _) = {
        let w = sim.app.world_mut();
        let p = w.query_filtered::<Entity, With<LocalPlayer>>().single(w).unwrap();
        (p, ())
    };
    sim.app.world_mut().resource_mut::<CaMusic>().files = MusicFiles::from_data(data());
    sim.seconds(0.2);
    let mut state = CaPerkState::default();
    state.war_cry = Some((1e9, p));
    sim.app.world_mut().entity_mut(p).insert(state);
    sim.seconds(0.2);
    let music = sim.app.world().resource::<CaMusic>();
    assert_eq!(
        music.player.primary.as_ref().map(|s| s.file.as_str()),
        Some(data().cues.war_cry.as_str())
    );
}

#[test]
fn a_missing_track_does_nothing() {
    let (a, b) = (data().playlist[0].file.clone(), data().playlist[1].file.clone());
    // The menu entry whose file is missing is never eligible.
    let mut m = only(&[(&b, 50.0)]);
    m.frame(0.0);
    assert_eq!(playing(&m), [b]);
    // No entries at all: the menus are silent.
    let mut m = CaMusic::new(data());
    run(&mut m, 0.0, 5_000.0, 500.0);
    assert!(playing(&m).is_empty());
    // A cue whose file is missing: nothing starts.
    m.files.remove(&a);
    m.room_start(0.0);
    assert!(playing(&m).is_empty());
}

// ---------------------------------------------------------------------------
// 12, 13. Map music triggers

fn prop(name: &str, value: Value) -> Property {
    Property {
        name: name.into(),
        flags: 0,
        len: 0,
        value,
    }
}

/// A `BGMTrigger` named `name` for `game_type`.
fn trigger(name: &str, game_type: &str, file: &str, fade: f32, looping: bool) -> Object {
    Object {
        class: "BGMTrigger".into(),
        properties: vec![
            prop("Name", Value::String(name.into())),
            prop("Pos", Value::Vector([0.0, 66.0, 0.0])),
            prop("GameType", Value::String(game_type.into())),
            prop("FileName", Value::String(file.into())),
            prop("FadingTime", Value::Real(fade)),
            prop("Loop", Value::Bool(looping)),
        ],
    }
}

/// A `SoundFX` named `name` at `pos`.
fn sound_fx(name: &str, pos: [f32; 3], file: &str, outer: f32, inner: f32, looping: bool, start_on: bool) -> Object {
    Object {
        class: "SoundFX".into(),
        properties: vec![
            prop("Name", Value::String(name.into())),
            prop("Pos", Value::Vector(pos)),
            prop("Sound", Value::String(file.into())),
            prop("OuterRadius", Value::Real(outer)),
            prop("InnerRadius", Value::Real(inner)),
            prop("Loop", Value::Bool(looping)),
            prop("StartOn", Value::Bool(start_on)),
        ],
    }
}

fn level(objects: Vec<Object>) -> World {
    let mut w = World::default();
    w.objects.push(Object {
        class: "GameStartPoint".into(),
        properties: vec![
            prop("Name", Value::String("GameStartPoint00".into())),
            prop("Pos", Value::Vector([0.0, 66.0, 0.0])),
            prop("Rotation", Value::Rotation([0.0; 4])),
            prop("GameType", Value::String("Team Death Match".into())),
            prop("Teams", Value::String("JSF".into())),
        ],
    });
    w.objects.extend(objects);
    w
}

fn music_map(objects: Vec<Object>) -> chimera_world::map::MapData {
    let s = movement::movement_data().meters_per_unit;
    world::convert(&level(objects), "music", s, &|_| None)
}

fn send(sim: &mut Sim, target: &str, message: &str) {
    sim.app
        .world_mut()
        .write_message(combat_arms::game::objects::CaObjectMessage {
            target: target.into(),
            message: message.into(),
        });
}

fn sim_with(objects: Vec<Object>, mode: &str) -> Sim {
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(music_map(objects)));
    sim.ticks(3);
    with_mode(sim.app.world_mut(), mode);
    let p = sim.spawn_character(Vec3::new(0.0, 2.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    sim.app.world_mut().resource_mut::<MusicClock>().manual = true;
    sim.seconds(0.5);
    sim
}

fn music_on(sim: &mut Sim) -> Vec<CaMusicOn> {
    let w = sim.app.world_mut();
    let mut msgs = w.resource_mut::<Messages<CaMusicOn>>();
    msgs.drain().collect()
}

#[test]
fn t12_map_trigger_late_join() {
    let objects = vec![
        trigger("Start", "Team Death Match", "BGM\\Start_BGM.mp3", 4.0, false),
        trigger("Last", "Team Death Match", "BGM\\Last_BGM.mp3", 4.0, true),
    ];
    let mut sim = sim_with(objects, "TDM");
    // Keep the messages for the test: the client side of this world is
    // the same machine, so it also plays them.
    sim.app.world_mut().resource_mut::<CaMusic>().files = MusicFiles::from_data(data());
    send(&mut sim, "Start", "ON");
    sim.seconds(0.2);
    let started = sim
        .app
        .world()
        .resource::<CaMusic>()
        .player
        .primary
        .clone()
        .expect("Start_BGM plays");
    assert_eq!(started.file, "bgm/start_bgm.mp3");
    send(&mut sim, "Start", "OFF");
    send(&mut sim, "Last", "ON");
    sim.seconds(0.2);
    // Only the trigger that is on and looping goes to a joiner.
    let joiner = sim
        .app
        .world()
        .resource::<CaMapMusic>()
        .for_joiner()
        .cloned()
        .expect("a trigger for a joiner");
    assert_eq!(
        joiner,
        CaMusicOn {
            looping: true,
            fade: 4.0,
            file: "BGM\\Last_BGM.mp3".into()
        }
    );
    // The joiner starts it with a 4 s fade-in, directly on its player.
    let mut m = all_files();
    m.screen = Screen::Match;
    m.start_direct(
        &combat_arms::game::music::data::normalize(&joiner.file),
        joiner.looping,
        joiner.fade as f64,
        0.0,
    );
    m.frame(1_000.0);
    close(
        gain_of(&m, "bgm/last_bgm.mp3").unwrap(),
        0.25,
        1e-5,
        "1 s into the 4 s fade-in",
    );
    assert!(m.player.primary.as_ref().unwrap().looping);
    // The menu layer's name is bypassed: the same file through it starts.
    assert_eq!(m.current, "");
    // The finished non-looping track alone is not sent to a joiner.
    send(&mut sim, "Last", "OFF");
    sim.seconds(0.2);
    assert_eq!(sim.app.world().resource::<CaMapMusic>().for_joiner(), None);
}

/// 5. A client that comes in gets the trigger that is on and looping as
/// the server takes its join (`trigger::late_join`, an observer of its
/// `Player`): sent within the update that put the `Player` on, as the
/// polling system did later in it; one that comes in with none on gets
/// nothing.
#[test]
fn t12_a_joiner_gets_the_looping_trigger_at_its_join() {
    use bevy::ecs::message::Messages;
    use bevy_replicon::prelude::{ClientId, SendTargets, ToClients};
    use chimera_net::net::server::Player;
    let objects = vec![trigger("Last", "Team Death Match", "BGM\\Last_BGM.mp3", 4.0, true)];
    let mut sim = sim_with(objects, "TDM");
    if !sim.app.world().contains_resource::<Messages<ToClients<CaMusicOn>>>() {
        sim.app.add_message::<ToClients<CaMusicOn>>();
    }
    let join = |sim: &mut Sim| {
        sim.app.world_mut().resource_mut::<Messages<ToClients<CaMusicOn>>>().clear();
        let client = sim.app.world_mut().spawn(Player { name: "joiner".into(), character: Entity::PLACEHOLDER }).id();
        let sent: Vec<CaMusicOn> = sim
            .app
            .world_mut()
            .resource_mut::<Messages<ToClients<CaMusicOn>>>()
            .drain()
            .filter(|m| matches!(m.targets, SendTargets::Single(ClientId::Client(c)) if c == client))
            .map(|m| m.message)
            .collect();
        sent
    };
    assert!(join(&mut sim).is_empty(), "nothing on: nothing sent");
    send(&mut sim, "Last", "ON");
    sim.seconds(0.2);
    assert_eq!(
        join(&mut sim),
        [CaMusicOn {
            looping: true,
            fade: 4.0,
            file: "BGM\\Last_BGM.mp3".into()
        }],
        "sent as the join is taken"
    );
}

#[test]
fn t13_map_trigger_wrong_mode() {
    let objects = vec![trigger(
        "Dark",
        "HungerGame",
        "BGM\\Hunted_DarkHill_BGM.mp3",
        1.0,
        false,
    )];
    let mut sim = sim_with(objects, "TDM");
    sim.app.world_mut().resource_mut::<CaMusic>().files = MusicFiles::from_data(data());
    music_on(&mut sim);
    send(&mut sim, "Dark", "ON");
    sim.seconds(0.3);
    assert!(music_on(&mut sim).is_empty(), "no message in a TDM game");
    assert!(sim.app.world().resource::<CaMapMusic>().triggers.is_empty());
    assert!(
        sim.app.world().resource::<CaMusic>().player.primary.is_none(),
        "no music"
    );
    // The same trigger in a Hunted game plays.
    let objects = vec![trigger(
        "Dark",
        "HungerGame",
        "BGM\\Hunted_DarkHill_BGM.mp3",
        1.0,
        false,
    )];
    let mut sim = sim_with(objects, "HG");
    sim.app.world_mut().resource_mut::<CaMusic>().files = MusicFiles::from_data(data());
    send(&mut sim, "Dark", "ON");
    sim.seconds(0.3);
    let s = sim
        .app
        .world()
        .resource::<CaMusic>()
        .player
        .primary
        .clone()
        .expect("it plays in Hunted");
    assert_eq!(s.file, "bgm/hunted_darkhill_bgm.mp3");
    assert!(!s.looping);
}

/// A trigger over the network: every client gets it, and a client that
/// comes in later gets the one trigger that is on and looping.
#[test]
fn map_triggers_over_the_network() {
    let map = music_map(vec![
        trigger("Start", "Team Death Match", "BGM\\Start_BGM.mp3", 0.0, false),
        trigger("Last", "Team Death Match", "BGM\\Last_BGM.mp3", 4.0, true),
    ]);
    let link = LinkConditions {
        latency: std::time::Duration::from_millis(10),
        jitter: std::time::Duration::ZERO,
        loss: 0.0,
    };
    let m1 = map.clone();
    let mut sim = NetSim::new(CaSim, link, 3, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(m1.clone()));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(30);
    with_mode(sim.server.app.world_mut(), "TDM");
    for app in [&mut sim.server.app, &mut sim.clients[0].app] {
        app.world_mut().resource_mut::<CaMusic>().files = MusicFiles::from_data(data());
        app.world_mut().resource_mut::<MusicClock>().manual = true;
    }
    sim.ticks(30);
    sim.server
        .app
        .world_mut()
        .write_message(combat_arms::game::objects::CaObjectMessage {
            target: "Last".into(),
            message: "ON".into(),
        });
    sim.ticks(90);
    let client = sim.clients[0].app.world().resource::<CaMusic>();
    let s = client
        .player
        .primary
        .clone()
        .or(client.player.secondary.clone())
        .expect("the client plays it");
    assert_eq!(s.file, "bgm/last_bgm.mp3");
    // A client that joins later is told.
    sim.add_client(|_| {});
    sim.clients[1].app.world_mut().resource_mut::<CaMusic>().files = MusicFiles::from_data(data());
    sim.clients[1].app.world_mut().resource_mut::<MusicClock>().manual = true;
    sim.until_joined(600);
    sim.ticks(60);
    let late = sim.clients[1].app.world().resource::<CaMusic>();
    let s = late
        .player
        .primary
        .clone()
        .or(late.player.secondary.clone())
        .expect("the late joiner plays it");
    assert_eq!(s.file, "bgm/last_bgm.mp3");
    assert!(s.looping);
}

// ---------------------------------------------------------------------------
// 14-17. Ambient sounds

#[test]
fn t14_ambient_gain() {
    // R 6000, r 4000 (sent as 170/255).
    let fx = SoundFx::from_props(
        0,
        "birds",
        &Props(vec![
            ("Sound".into(), "SND\\x.wav".into()),
            ("OuterRadius".into(), "6000".into()),
            ("InnerRadius".into(), "4000".into()),
        ]),
        Vec3::ZERO,
        true,
        0,
    );
    for (d, want) in [
        (3000.0, 1.0),
        (4200.0, 0.59),
        (4400.0, 0.16),
        (4600.0, 0.0),
        (5900.0, 0.0),
    ] {
        close(fx.gain_at(d), want, 0.006, &format!("d = {d}"));
        // With effects at 50 % half of each.
        close(fx.gain_at(d) * 0.5, want * 0.5, 0.003, &format!("half, d = {d}"));
    }
    assert_eq!(fx.gain_at(6000.0), 0.0);
}

#[test]
fn t15_ambient_gain_rounding() {
    close(
        ambient::wire_inner(1000.0, 2000.0),
        996.078,
        0.01,
        "the rebuilt inner radius",
    );
    let g = |d| ambient::gain(d, 1000.0, 2000.0);
    close(g(1200.0), 0.5557, 1e-3, "1200");
    close(g(1400.0), 0.0397, 1e-3, "1400");
    assert_eq!(g(1415.0), 0.0);
    close(g(1414.0), 0.0, 1e-3, "zero at about 1414.22");
}

#[test]
fn t16_inner_not_below_outer() {
    let g = |d| ambient::gain(d, 600.0, 500.0);
    assert_eq!(g(450.0), 1.0);
    assert_eq!(g(499.0), 1.0);
    assert_eq!(g(501.0), 0.0);
    // The census: Volume scales; radii are clamped to 1..65000.
    assert_eq!(ambient::clamp_radii(-5.0, 100_000.0), (1.0, 65_000.0));
}

#[test]
fn t17_map_sounds_reach_everyone() {
    // A looping SoundFX 20 000 u from the player, on at the load.
    let s = movement::movement_data().meters_per_unit;
    let objects = vec![
        sound_fx(
            "far",
            [20_000.0, 66.0, 0.0],
            "SND\\Far\\loop.wav",
            6000.0,
            4000.0,
            true,
            true,
        ),
        sound_fx("off", [0.0, 66.0, 100.0], "SND\\Off.wav", 500.0, 100.0, true, false),
        sound_fx("once", [0.0, 66.0, 200.0], "SND\\Once.wav", 500.0, 100.0, false, true),
    ];
    let mut sim = sim_with(objects, "TDM");
    let amb = sim.app.world().resource::<CaAmbient>().clone();
    let live: Vec<&str> = amb.live().map(|f| f.name.as_str()).collect();
    assert_eq!(live, ["far"], "looping and on: live, audible or not");
    assert_eq!(amb.sounds.len(), 3);
    // Muted where the player is; no sound for it there.
    let here = Vec3::new(0.0, 66.0, 0.0);
    assert!(amb.audible(here, 1.0).is_empty());
    // Walking within R makes it audible, without any new message: the same
    // list answers.
    let near = Vec3::new(20_000.0 - 3_000.0, 66.0, 0.0);
    let hear = amb.audible(near, 1.0);
    assert_eq!(hear.len(), 1);
    close(hear[0].1, 1.0, 1e-6, "inside the inner radius");
    let _ = s;
    // ON starts the switched-off one: it becomes live.
    send(&mut sim, "off", "ON");
    sim.seconds(0.3);
    let amb = sim.app.world().resource::<CaAmbient>();
    assert_eq!(amb.live().count(), 2);
    send(&mut sim, "off", "OFF");
    send(&mut sim, "far", "KILL");
    sim.seconds(0.3);
    assert_eq!(sim.app.world().resource::<CaAmbient>().live().count(), 0);
}

/// A client of a network game holds the server's map sounds as they are
/// now, a joiner too.
#[test]
fn map_sounds_over_the_network() {
    let map = music_map(vec![
        sound_fx("a", [100.0, 66.0, 0.0], "SND\\a.wav", 500.0, 100.0, true, true),
        sound_fx("b", [200.0, 66.0, 0.0], "SND\\b.wav", 500.0, 100.0, true, false),
    ]);
    let link = LinkConditions {
        latency: std::time::Duration::from_millis(10),
        jitter: std::time::Duration::ZERO,
        loss: 0.0,
    };
    let m1 = map.clone();
    let mut sim = NetSim::new(CaSim, link, 3, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(m1.clone()));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(30);
    let names = |app: &App| -> Vec<String> {
        app.world()
            .resource::<CaAmbient>()
            .live()
            .map(|s| s.name.clone())
            .collect()
    };
    assert_eq!(names(&sim.clients[0].app), ["a"]);
    sim.server
        .app
        .world_mut()
        .write_message(combat_arms::game::objects::CaObjectMessage {
            target: "b".into(),
            message: "ON".into(),
        });
    sim.server
        .app
        .world_mut()
        .write_message(combat_arms::game::objects::CaObjectMessage {
            target: "a".into(),
            message: "OFF".into(),
        });
    sim.ticks(120);
    assert_eq!(names(&sim.server.app), ["b"]);
    assert_eq!(names(&sim.clients[0].app), ["b"], "the client follows");
    sim.add_client(|_| {});
    sim.until_joined(600);
    sim.ticks(60);
    assert_eq!(names(&sim.clients[1].app), ["b"], "a late joiner has what still runs");
}

// ---------------------------------------------------------------------------
// 18. Volume channels

#[test]
fn t18_window_minimized() {
    let mut v = CaVolumes::default();
    v.set_music(80);
    v.set_sound(70);
    close(v.music(), 0.8, 1e-6, "music");
    close(v.sound(), 0.7, 1e-6, "effects");
    v.minimized = true;
    assert_eq!((v.music(), v.sound()), (0.0, 0.0));
    v.minimized = false;
    close(v.music(), 0.8, 1e-6, "restored music");
    close(v.sound(), 0.7, 1e-6, "restored effects");
    // The effects slider does not touch the music.
    v.set_sound(0);
    close(v.music(), 0.8, 1e-6, "music alone");
}

// --- A party match (Section 10, `director::observe_party`): a minigame's
// moments (Chimera's `minigame::moments`) with Combat Arms' music.

use chimera_sim::minigame::moments::{MinigameMoments, MomentPhase};

/// The facts of a party match whose music is `music`'s.
fn party(music: Option<&'static str>, phase: MomentPhase, alive: bool) -> MusicFacts {
    let moments = MinigameMoments {
        playing: true,
        music,
        phase,
        alive,
        ..Default::default()
    };
    let screen = if phase == MomentPhase::Over && moments.result.is_some() { Screen::Result } else { Screen::Match };
    MusicFacts {
        screen,
        mode: data().party.mode.clone(),
        alive,
        party: Some(moments),
        ..Default::default()
    }
}

fn with(mut f: MusicFacts, edit: impl FnOnce(&mut MinigameMoments)) -> MusicFacts {
    edit(f.party.as_mut().unwrap());
    let p = f.party.as_ref().unwrap();
    if p.phase == MomentPhase::Over && p.result.is_some() {
        f.screen = Screen::Result;
    }
    f
}

const CA: Option<&str> = Some("combat_arms");

/// Cases 6, 24-26 in a party match: going in plays the loading track;
/// the start of play (alive) `RoundStartSound` and Arms Race's start
/// track; a player on the last rung `last_stage_bgm` looping (its sound is
/// the cue pack's, not here), a second one leaves it playing, nobody on
/// it any more: the start track from its start (Arms Race loops).
#[test]
fn a_party_match_gets_arms_races_cues() {
    // `party_music 2`: the original's cues.
    let mut m = all_files();
    m.party_music = 2;
    let mut prev = MusicFacts::default();
    let loading = MusicFacts {
        screen: Screen::Loading,
        ..party(CA, MomentPhase::Waiting, true)
    };
    step(&mut m, &mut prev, loading, 0.0);
    assert_eq!(playing(&m), [data().cues.loading.clone()], "the loading track");
    step(&mut m, &mut prev, party(CA, MomentPhase::Waiting, true), 2_500.0);
    assert!(playing(&m).is_empty(), "stopped as the loading screen closes");
    step(&mut m, &mut prev, party(CA, MomentPhase::Live, true), 3_000.0);
    let start = data().mode_track("VWRM", None).unwrap().0.to_string();
    assert_eq!(playing(&m), [start.clone()], "the start track at the start of play");
    assert!(m.heard.contains(&"RoundStartSound".to_string()));
    let last = with(party(CA, MomentPhase::Live, true), |p| p.last_rung = 1);
    step(&mut m, &mut prev, last.clone(), 10_000.0);
    assert_eq!(playing(&m), [data().cues.last_stage.clone()], "the last stage's music");
    assert!(!m.heard.contains(&"last_stage_sound".to_string()), "its sound is the cue pack's");
    let two = with(party(CA, MomentPhase::Live, true), |p| p.last_rung = 2);
    step(&mut m, &mut prev, two, 30_000.0);
    let started = m.mixed(1.0)[0].started;
    assert_eq!(started, 10_000.0, "not restarted by a second player");
    step(&mut m, &mut prev, party(CA, MomentPhase::Live, true), 40_000.0);
    assert_eq!(playing(&m), [start], "nobody on it: the start track again");
    assert_eq!(m.mixed(1.0)[0].started, 40_000.0, "from its start");
}

/// Case 8 in a party match: dead at the start of play, no sound and no
/// music that match.
#[test]
fn dead_at_the_start_of_play_no_party_music() {
    let mut m = all_files();
    let mut prev = MusicFacts::default();
    step(&mut m, &mut prev, party(CA, MomentPhase::Waiting, false), 0.0);
    step(&mut m, &mut prev, party(CA, MomentPhase::Live, false), 1_000.0);
    assert!(playing(&m).is_empty());
    assert!(!m.heard.contains(&"RoundStartSound".to_string()));
    step(&mut m, &mut prev, party(CA, MomentPhase::Live, true), 3_000.0);
    assert!(playing(&m).is_empty(), "alive later: still none (one start of play a match)");
}

/// The respawn countdown (death_killcam.md 9): `RespawnCountSound` at
/// each whole second; the match-end screen (case 27): music stopped and
/// paused (the screen plays the win or lose sound), nothing until the
/// next match, whose start of play has its music again.
#[test]
fn the_party_countdown_ticks_and_the_end_screen_is_silent() {
    let mut m = all_files();
    let mut prev = MusicFacts::default();
    step(&mut m, &mut prev, party(CA, MomentPhase::Waiting, true), 0.0);
    step(&mut m, &mut prev, party(CA, MomentPhase::Live, true), 1_000.0);
    for (i, s) in [2, 2, 1, 1].into_iter().enumerate() {
        let dead = with(party(CA, MomentPhase::Live, false), |p| p.respawn_in = Some(s));
        step(&mut m, &mut prev, dead, 2_000.0 + i as f64 * 500.0);
    }
    assert_eq!(m.heard.iter().filter(|k| *k == "RespawnCountSound").count(), 2, "one a whole second");
    let over = with(party(CA, MomentPhase::Over, true), |p| p.result = Some(Some(false)));
    step(&mut m, &mut prev, over.clone(), 10_000.0);
    assert!(playing(&m).is_empty(), "stopped on the match-end screen");
    step(&mut m, &mut prev, over, 20_000.0);
    assert!(playing(&m).is_empty(), "no playlist there");
    assert!(!m.heard.iter().any(|k| k.starts_with("Result")), "the screen's sound, not the director's");
    step(&mut m, &mut prev, party(CA, MomentPhase::Waiting, true), 30_000.0);
    step(&mut m, &mut prev, party(CA, MomentPhase::Live, true), 31_000.0);
    assert_eq!(playing(&m), [data().mode_track("VWRM", None).unwrap().0.to_string()], "the next match's music");
}

/// Another game's music (or none): Combat Arms' director stays silent in
/// a party match, the loading track and playlist too.
#[test]
fn another_games_party_music_leaves_combat_arms_silent() {
    for music in [None, Some("cs_source")] {
        let mut m = all_files();
        let mut prev = MusicFacts::default();
        let loading = MusicFacts {
            screen: Screen::Loading,
            ..party(music, MomentPhase::Waiting, true)
        };
        step(&mut m, &mut prev, loading, 0.0);
        step(&mut m, &mut prev, party(music, MomentPhase::Live, true), 1_000.0);
        let last = with(party(music, MomentPhase::Live, true), |p| p.last_rung = 1);
        step(&mut m, &mut prev, last, 2_000.0);
        assert!(playing(&m).is_empty() && m.heard.is_empty(), "{music:?}");
    }
}

// --- The party's playlist (`party_music`, director module doc).

/// The table's files plus three more tracks of a minute: six in the
/// party playlist.
fn six_tracks() -> CaMusic {
    let mut m = all_files();
    for n in ["bgm/bgm_03.mp3", "bgm/bgm_05.mp3", "bgm/bgm_06.mp3"] {
        m.files.insert(n, Some(60_000.0));
    }
    m
}

const LIVE: MomentPhase = MomentPhase::Live;

/// Run the live party match from `from` to `to` ms in 250 ms frames; the
/// files that came up in turn (the music's name changes).
fn live_until(m: &mut CaMusic, prev: &mut MusicFacts, facts: &MusicFacts, from: f64, to: f64) -> Vec<String> {
    let mut seen = Vec::new();
    let mut last = m.current.clone();
    let mut t = from;
    while t <= to {
        step(m, prev, facts.clone(), t);
        if last != m.current && !m.current.is_empty() {
            seen.push(m.current.clone());
        }
        last = m.current.clone();
        t += 250.0;
    }
    seen
}

fn started(m: &mut CaMusic, prev: &mut MusicFacts) {
    step(m, prev, party(CA, MomentPhase::Waiting, true), 0.0);
    step(m, prev, party(CA, LIVE, true), 1_000.0);
}

/// The start track plays once (no loop) and a different track follows;
/// no playlist track repeats before every one has played, and the second
/// pass is another full set; the fade is the menu's cross-fade.
#[test]
fn the_party_plays_the_start_track_once_then_a_shuffled_playlist() {
    let mut m = six_tracks();
    let mut prev = MusicFacts::default();
    started(&mut m, &mut prev);
    let start = data().mode_track("VWRM", None).unwrap().0.to_string();
    assert_eq!(playing(&m), [start.clone()]);
    assert!(!m.mixed(1.0)[0].looping, "the start track does not loop");
    let live = party(CA, LIVE, true);
    // Before it is near its end (17.8 s long, a 10 s fade): still alone.
    let seen = live_until(&mut m, &mut prev, &live, 1_250.0, 8_000.0);
    assert!(seen.is_empty(), "{seen:?}");
    assert_eq!(playing(&m), [start.clone()]);
    // Near its end the next track cross-fades in over the menu fade.
    let seen = live_until(&mut m, &mut prev, &live, 8_250.0, 9_000.0);
    assert_eq!(seen.len(), 1);
    assert_ne!(seen[0], start);
    assert_eq!(playing(&m).len(), 2, "cross-fading");
    // Two passes of the six tracks.
    let mut order = seen;
    order.extend(live_until(&mut m, &mut prev, &live, 9_250.0, 1_000_000.0));
    assert!(!order[1..].contains(&start), "the start track does not come back: {order:?}");
    assert!(order.len() >= 12, "{order:?}");
    for pass in order[..12].chunks(6) {
        let mut sorted = pass.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 6, "every track once a pass: {order:?}");
    }
    assert_ne!(order[5], order[6], "not the track just played");
}

/// The last stage takes over, and once nobody is on it the playlist
/// resumes with a track not yet played this pass (not the start track).
#[test]
fn the_last_stage_interrupts_the_playlist_and_it_resumes() {
    let mut m = six_tracks();
    let mut prev = MusicFacts::default();
    started(&mut m, &mut prev);
    let live = party(CA, LIVE, true);
    let seen = live_until(&mut m, &mut prev, &live, 1_250.0, 25_000.0);
    let first = seen[0].clone();
    let last = with(party(CA, LIVE, true), |p| p.last_rung = 1);
    step(&mut m, &mut prev, last.clone(), 26_000.0);
    assert_eq!(playing(&m), [data().cues.last_stage.clone()]);
    // The last-stage music does not hand over to the playlist by itself.
    let seen_last = live_until(&mut m, &mut prev, &last, 26_250.0, 200_000.0);
    assert!(seen_last.is_empty(), "{seen_last:?}");
    step(&mut m, &mut prev, live.clone(), 201_000.0);
    let back = playing(&m);
    assert_eq!(back.len(), 1);
    let start = data().mode_track("VWRM", None).unwrap().0.to_string();
    assert_ne!(back[0], start, "the playlist, not the start track");
    assert_ne!(back[0], data().cues.last_stage, "stopped");
    assert_ne!(back[0], first, "one not yet played this pass");
}

/// `party_music 2`: Arms Race's loop (the start track repeats, also
/// after the last stage); 0: nothing; the match end silences all.
#[test]
fn party_music_two_loops_the_start_track_and_zero_is_silent() {
    let start = data().mode_track("VWRM", None).unwrap().0.to_string();
    let mut m = six_tracks();
    m.party_music = 2;
    let mut prev = MusicFacts::default();
    started(&mut m, &mut prev);
    assert!(m.mixed(1.0)[0].looping);
    let seen = live_until(&mut m, &mut prev, &party(CA, LIVE, true), 1_250.0, 120_000.0);
    assert!(seen.is_empty(), "{seen:?}");
    assert_eq!(playing(&m), [start]);

    let mut m = six_tracks();
    m.party_music = 0;
    let mut prev = MusicFacts::default();
    started(&mut m, &mut prev);
    assert!(playing(&m).is_empty());
    let seen = live_until(&mut m, &mut prev, &party(CA, LIVE, true), 1_250.0, 60_000.0);
    assert!(seen.is_empty());
    let last = with(party(CA, LIVE, true), |p| p.last_rung = 1);
    step(&mut m, &mut prev, last, 61_000.0);
    assert!(playing(&m).is_empty(), "off is off");

    // The match end stops the playlist.
    let mut m = six_tracks();
    let mut prev = MusicFacts::default();
    started(&mut m, &mut prev);
    live_until(&mut m, &mut prev, &party(CA, LIVE, true), 1_250.0, 30_000.0);
    let over = with(party(CA, MomentPhase::Over, true), |p| p.result = Some(Some(false)));
    let seen = live_until(&mut m, &mut prev, &over, 30_250.0, 100_000.0);
    assert!(seen.is_empty() && playing(&m).is_empty());
}

/// Combat Arms' own modes (no party moments) keep the original: the start
/// track loops and comes back after the last stage, whatever `party_music`.
#[test]
fn combat_arms_own_modes_keep_the_original_loop() {
    let start = data().mode_track("VWRM", None).unwrap().0.to_string();
    let mut m = six_tracks();
    assert_eq!(m.party_music, 1);
    m.screen = Screen::Match;
    m.round_start("VWRM", false, true, 0.0);
    assert!(m.mixed(1.0)[0].looping);
    run(&mut m, 0.0, 120_000.0, 250.0);
    assert_eq!(playing(&m), [start.clone()]);
    m.last_stage(1, 121_000.0);
    assert_eq!(playing(&m), [data().cues.last_stage.clone()]);
    m.last_stage(0, 122_000.0);
    assert_eq!(playing(&m), [start]);
}
