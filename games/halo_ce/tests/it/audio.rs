//! Halo's looping sound player, the level's zones and the footsteps' kinds
//! on synthetic data (specs/halo_ce/sounds.md "Looping sounds",
//! "Distance", "Channels, limits, preemption", "Sound environment,
//! background sound, sound scenery"; effects.md "Material effects"): what
//! the player asks of the engine (`SoundControl` starts, changes and
//! stops; `PlaySound` one-shots) frame by frame, for fades at start and
//! stop, start and end sounds, the audibility cut-off, the stereo fade,
//! detail sounds, the per-definition limit and the "fake impulse" flag;
//! the pruned point test; the library's round trip through its map entity.

use std::collections::BTreeMap;

use bevy::prelude::*;
use halo_ce::game::{
    METERS_PER_UNIT,
    audio::{
        level::{Bsp, LevelAudio, prune, zone_side},
        library::{AudioLibrary, HaloLoop, LoopDetail, LoopTrack, SoundInfo},
        looping::{LoopRequest, LoopSource, Loops, step, stereo_fade},
        steps::{JUMP, JUMP_LAND, RUN, Seen, WALK, kinds},
    },
    body::anims::State,
};
use chimera_world::map::{PlaySound, SoundControl};

fn info(entry: &str, class: u8, min: f32, max: f32, stereo: bool) -> SoundInfo {
    SoundInfo {
        entry: format!("halo_ce:{entry}"),
        loop_entry: format!("halo_ce:{entry}#loop"),
        class,
        min,
        max,
        stereo,
        gain: 0.5,
        zero_gain: 1.0,
        one_gain: 1.0,
        zero_pitch: 1.0,
        one_pitch: 1.0,
        pitch: [1.0, 1.0],
        rate: 1.0,
        longest_ms: 500,
        seconds: 1.0,
        loop_seconds: 4.0,
    }
}

fn track(flags: u32, start: &str, looped: &str, end: &str) -> LoopTrack {
    LoopTrack { flags, gain: 1.0, fade_in: 3.0, fade_out: 3.0, start: start.into(), looped: looped.into(), end: end.into(), ..Default::default() }
}

/// A library: `amb` (stereo ambience, fade in and out 3 s), `hum` (mono,
/// 0.5 to 2.5 WU, class 50, a start and an end sound, no fades), `once`
/// (fake impulse), `birds` (ambience with a detail every 2 s), `rocket`
/// (mono weapon-fire class, limit 4).
fn library() -> AudioLibrary {
    let mut sounds = BTreeMap::new();
    sounds.insert("amb".into(), info("amb", 33, 0.9, 5.0, true));
    sounds.insert("hum".into(), info("hum", 50, 0.5, 2.5, false));
    sounds.insert("hum_in".into(), info("hum_in", 50, 0.5, 2.5, false));
    sounds.insert("hum_out".into(), info("hum_out", 50, 0.5, 2.5, false));
    sounds.insert("chirp".into(), info("chirp", 33, 0.9, 5.0, false));
    sounds.insert("whoosh".into(), info("whoosh", 4, 4.0, 70.0, false));
    let mut loops = BTreeMap::new();
    loops.insert("amb".into(), HaloLoop { tracks: vec![track(3, "", "amb", "")], max_distance: 5.0, period_scale: [1.0, 1.0], ..Default::default() });
    loops.insert("hum".into(), HaloLoop { tracks: vec![track(0, "hum_in", "hum", "hum_out")], max_distance: 2.5, period_scale: [1.0, 1.0], ..Default::default() });
    loops.insert("once".into(), HaloLoop { flags: 2, tracks: vec![track(0, "", "hum", "")], max_distance: 2.5, period_scale: [1.0, 1.0], ..Default::default() });
    loops.insert(
        "birds".into(),
        HaloLoop {
            tracks: vec![track(3, "", "amb", "")],
            details: vec![LoopDetail { sound: "chirp".into(), period: [2.0, 2.0], gain: 0.5, yaw: [-3.0, 3.0], pitch: [-1.5, 1.5], ..Default::default() }],
            max_distance: 5.0,
            period_scale: [1.0, 1.0],
            ..Default::default()
        },
    );
    loops.insert("rocket".into(), HaloLoop { tracks: vec![track(0, "", "whoosh", "")], max_distance: 70.0, period_scale: [1.0, 1.0], ..Default::default() });
    AudioLibrary { sounds, loops, ..Default::default() }
}

fn ask(key: u64, sound: &str, source: LoopSource) -> LoopRequest {
    LoopRequest { key, sound: sound.into(), source, scale: 1.0, alternate: false }
}

/// One frame of `dt` seconds.
fn frame(loops: &mut Loops, lib: &AudioLibrary, asks: Vec<LoopRequest>, listener: Vec3, dt: f32) -> (Vec<SoundControl>, Vec<PlaySound>) {
    let (mut c, mut s) = (Vec::new(), Vec::new());
    step(loops, lib, asks, Some(listener), dt, &mut c, &mut s);
    (c, s)
}

fn starts(c: &[SoundControl]) -> Vec<&chimera_world::map::StartSound> {
    c.iter().filter_map(|c| if let SoundControl::Start(s) = c { Some(s) } else { None }).collect()
}

fn volume(c: &[SoundControl]) -> Option<f32> {
    c.iter().find_map(|c| match c {
        SoundControl::Start(s) => s.volume,
        SoundControl::Change { volume, .. } => *volume,
        _ => None,
    })
}

fn stops(c: &[SoundControl]) -> usize {
    c.iter().filter(|c| matches!(c, SoundControl::Stop(_))).count()
}

/// "Fade in at start" and "fade out at stop" (the background sound's
/// flags 3, 3 s): the loop starts at once, unspatialized, dry, looping,
/// its gain rising linearly over 3 s; asked no more it falls over 3 s and
/// its voice stops.
#[test]
fn a_loop_fades_in_and_out() {
    let lib = library();
    let mut loops = Loops::default();
    let (c, _) = frame(&mut loops, &lib, vec![ask(1, "amb", LoopSource::Unspatialized)], Vec3::ZERO, 0.0);
    let st = starts(&c);
    assert_eq!(st.len(), 1);
    assert_eq!((st[0].entry.as_str(), st[0].at, st[0].looping, st[0].dry), ("halo_ce:amb#loop", None, true, true));
    let mut v = 0.0;
    for _ in 0..15 {
        let (c, _) = frame(&mut loops, &lib, vec![ask(1, "amb", LoopSource::Unspatialized)], Vec3::ZERO, 0.1);
        v = volume(&c).unwrap();
    }
    // 1.5 s in: half of the track's 0.5 (gain x gain modifier).
    assert!((v - 0.25).abs() < 0.01, "{v}");
    let mut stopped_at = None;
    for i in 0..40 {
        let (c, _) = frame(&mut loops, &lib, vec![], Vec3::ZERO, 0.1);
        if stops(&c) > 0 {
            stopped_at = Some(i);
            break;
        }
    }
    // From 0.25 the fade of 1/3 per second reaches 0 in 1.5 s.
    let i = stopped_at.expect("stopped");
    assert!((14..=16).contains(&i), "{i}");
    assert!(loops.loops.is_empty());
}

/// A start sound plays as a one-shot and the loop follows it; asked no
/// more, the end sound plays and the loop stops at once (no fade bits).
#[test]
fn start_and_end_sounds() {
    let lib = library();
    let mut loops = Loops::default();
    let near = LoopSource::At(Vec3::new(0.5, 0.0, 0.0));
    let (c, s) = frame(&mut loops, &lib, vec![ask(1, "hum", near)], Vec3::ZERO, 0.0);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].entry, "halo_ce:hum_in");
    assert!(starts(&c).is_empty(), "the loop waits for the start sound");
    let mut started = None;
    for i in 0..20 {
        let (c, _) = frame(&mut loops, &lib, vec![ask(1, "hum", near)], Vec3::ZERO, 0.1);
        if let Some(st) = starts(&c).first() {
            // Mono with a position: positional, the engine's inverse law.
            assert_eq!(st.at, Some(Vec3::new(0.5, 0.0, 0.0)));
            started = Some(i);
            break;
        }
    }
    assert!((8..=10).contains(&started.unwrap()), "{started:?}");
    let (c, s) = frame(&mut loops, &lib, vec![], Vec3::ZERO, 0.1);
    assert_eq!(s.iter().map(|s| s.entry.as_str()).collect::<Vec<_>>(), ["halo_ce:hum_out"]);
    assert_eq!(stops(&c), 1);
}

/// The maximum distance cuts a mono loop off: not created beyond it; once
/// playing, leaving the range fades it to 0 over 2 s and its voice ends;
/// back in range it starts again fading in over 0.5 s.
#[test]
fn the_maximum_distance_is_a_cut_off() {
    let lib = library();
    let mut loops = Loops::default();
    let at = |wu: f32| LoopSource::At(Vec3::new(wu * METERS_PER_UNIT, 0.0, 0.0));
    for _ in 0..5 {
        let (c, s) = frame(&mut loops, &lib, vec![ask(1, "rocket", at(80.0))], Vec3::ZERO, 0.1);
        assert!(c.is_empty() && s.is_empty());
    }
    let (c, _) = frame(&mut loops, &lib, vec![ask(1, "rocket", at(60.0))], Vec3::ZERO, 0.1);
    assert_eq!(starts(&c).len(), 1);
    // Out of range: 2 s of fade (volume falls), then the voice stops.
    let mut last = 1.0;
    let mut stopped = None;
    for i in 0..30 {
        let (c, _) = frame(&mut loops, &lib, vec![ask(1, "rocket", at(71.0))], Vec3::ZERO, 0.1);
        if stops(&c) > 0 {
            stopped = Some(i);
            break;
        }
        let v = volume(&c).unwrap();
        assert!(v <= last + 1e-6);
        last = v;
    }
    assert!((18..=20).contains(&stopped.unwrap()), "{stopped:?}");
    // Back: a new voice fading in over 0.5 s.
    let (c, _) = frame(&mut loops, &lib, vec![ask(1, "rocket", at(10.0))], Vec3::ZERO, 0.1);
    let st = starts(&c);
    assert_eq!(st.len(), 1);
    assert!(st[0].volume.unwrap() < 0.5 * 0.5);
    for _ in 0..5 {
        frame(&mut loops, &lib, vec![ask(1, "rocket", at(10.0))], Vec3::ZERO, 0.1);
    }
    let (c, _) = frame(&mut loops, &lib, vec![ask(1, "rocket", at(10.0))], Vec3::ZERO, 0.1);
    assert!((volume(&c).unwrap() - 0.5).abs() < 1e-4);
}

/// A stereo loop with a position is never panned: unspatialized, with the
/// spec's linear fade (`m = 4`, `M = 70`: 1, 0.75, 0.5, 0 at 4, 20.5, 37,
/// 70 WU).
#[test]
fn stereo_loops_fade_linearly() {
    for (d, want) in [(4.0, 1.0), (20.5, 0.75), (37.0, 0.5), (70.0, 0.0), (100.0, 0.0)] {
        assert!((stereo_fade(4.0, 70.0, d) - want).abs() < 1e-6, "{d}");
    }
    assert_eq!(stereo_fade(5.0, 5.0, 4.0), 1.0);
    assert_eq!(stereo_fade(5.0, 5.0, 6.0), 0.0);
    let lib = library();
    let mut loops = Loops::default();
    let src = LoopSource::At(Vec3::new(2.95 * METERS_PER_UNIT, 0.0, 0.0));
    frame(&mut loops, &lib, vec![ask(1, "amb", src)], Vec3::ZERO, 0.0);
    for _ in 0..40 {
        frame(&mut loops, &lib, vec![ask(1, "amb", src)], Vec3::ZERO, 0.1);
    }
    let (c, _) = frame(&mut loops, &lib, vec![ask(1, "amb", src)], Vec3::ZERO, 0.1);
    // Halfway from 0.9 to 5 WU: half of 0.5.
    assert!((volume(&c).unwrap() - 0.25).abs() < 0.01);
    assert!(starts(&c).is_empty());
}

/// Details: the first after its period (2 s), the next after the period
/// plus the detail sound's stored longest length (0.5 s); for an
/// unspatialized loop with no distance, unspatialized, at the detail's
/// gain x the sound's.
#[test]
fn detail_sounds_play_at_their_periods() {
    let lib = library();
    let mut loops = Loops::default();
    let mut times = Vec::new();
    for i in 0..60 {
        let (_, s) = frame(&mut loops, &lib, vec![ask(1, "birds", LoopSource::Unspatialized)], Vec3::ZERO, 0.1);
        for p in s {
            assert_eq!(p.entry, "halo_ce:chirp");
            assert_eq!(p.at, None);
            assert!((p.volume.unwrap() - 0.25).abs() < 1e-6);
            times.push(i);
        }
    }
    assert_eq!(times.len(), 2, "{times:?}");
    assert!((19..=21).contains(&times[0]) && (44..=46).contains(&times[1]), "{times:?}");
}

/// At most the class's per-definition limit (weapon fire: 4) of one
/// looping sound's loops play: the nearest.
#[test]
fn the_class_limit_keeps_the_nearest() {
    let lib = library();
    let mut loops = Loops::default();
    let asks = || (0..6).map(|i| ask(i, "rocket", LoopSource::At(Vec3::new(i as f32 * 3.0, 0.0, 0.0)))).collect::<Vec<_>>();
    let (c, _) = frame(&mut loops, &lib, asks(), Vec3::ZERO, 0.0);
    let st = starts(&c);
    assert_eq!(st.len(), 4);
    assert!(st.iter().all(|s| s.at.unwrap().x < 12.0));
}

/// "Fake impulse": the sounds play once (not looping) and the loop ends.
#[test]
fn a_fake_impulse_plays_once() {
    let lib = library();
    let mut loops = Loops::default();
    let src = LoopSource::At(Vec3::X);
    let (c, _) = frame(&mut loops, &lib, vec![ask(1, "once", src)], Vec3::ZERO, 0.0);
    assert!(!starts(&c)[0].looping);
    let mut ended = false;
    for _ in 0..60 {
        let (c, _) = frame(&mut loops, &lib, vec![ask(1, "once", src)], Vec3::ZERO, 0.1);
        assert!(starts(&c).is_empty(), "never again while asked");
        ended |= stops(&c) > 0;
    }
    assert!(ended);
}

/// The pruned point test: a cluster's zone per leaf; a node whose sides
/// are one zone is that zone, a solid side takes its sibling's.
#[test]
fn the_point_test_prunes_to_zones() {
    // Node 0: x >= 0 front (node 1), back leaf 0 (zone 0).
    // Node 1: y >= 0 front leaf 1 (zone 1), back solid.
    let nodes = [([1.0, 0.0, 0.0, 0.0], i32::MIN, 1), ([0.0, 1.0, 0.0, 0.0], -1, i32::MIN + 1)];
    let node = |n: u32| nodes.get(n as usize).map(|(p, b, f)| (*p, *b, *f));
    let leaf = |l: u32| zone_side(l as usize);
    let (tree, root) = prune(&Bsp { node: &node, leaf: &leaf, nodes: 2 });
    assert_eq!(tree.len(), 1, "node 1 collapses to zone 1");
    let level = LevelAudio { zones: vec![[0, -1], [1, 0]], backgrounds: vec!["a".into(), "".into()], tree, root, ..Default::default() };
    assert_eq!(level.zone([-1.0, 0.0, 0.0]), Some(0));
    assert_eq!(level.zone([1.0, 1.0, 0.0]), Some(1));
    assert_eq!(level.zone([1.0, -1.0, 0.0]), Some(1));
    assert_eq!(level.background(0), Some("a"));
    assert_eq!(level.background(1), None);
    // One zone everywhere: no tree.
    let one = LevelAudio { zones: vec![[0, 0]], root: zone_side(0), ..Default::default() };
    assert_eq!(one.zone([5.0, 5.0, 5.0]), Some(0));
}

/// The library survives its map entity's text (TOML) unchanged.
#[test]
fn the_library_round_trips() {
    let mut lib = library();
    lib.footsteps = vec![vec!["a".into(), String::new()]];
    lib.level = Some(LevelAudio { backgrounds: vec!["amb".into()], zones: vec![[0, -1]], root: zone_side(0), clusters: 3, ..Default::default() });
    let back = AudioLibrary::decode(&lib.encode()).unwrap();
    assert_eq!(back, lib);
}

/// The footsteps' kinds: a foot frame reached runs standing and walks
/// crouching (one per foot), leaving the ground upward jumps, entering a
/// landing lands; a frame seen again makes nothing.
#[test]
fn footstep_kinds() {
    let a = Seen { sequence: Some(3), frame: 4, state: State::MoveFront, air_ticks: 0 };
    let b = Seen { frame: 5, ..a };
    assert_eq!(kinds(&a, &b, 1, false, false), [RUN]);
    assert_eq!(kinds(&a, &b, 2, true, false), [WALK]);
    assert_eq!(kinds(&a, &b, 3, false, false), [RUN, RUN]);
    assert!(kinds(&b, &b, 1, false, false).is_empty());
    let air = Seen { air_ticks: 1, ..a };
    assert_eq!(kinds(&a, &air, 0, false, true), [JUMP]);
    assert!(kinds(&a, &air, 0, false, false).is_empty());
    let land = Seen { state: State::LandSoft, ..a };
    assert_eq!(kinds(&a, &land, 0, false, false), [JUMP_LAND]);
    let hard = Seen { state: State::LandHard, ..a };
    assert_eq!(kinds(&a, &hard, 0, false, false), [JUMP_LAND]);
}

// The Spartan's warning sounds (low_health.md 3): the unit HUD's five
// elements as the tags give them, by the state word of the local unit.
mod warnings {
    use halo_ce::game::audio::{
        library::Warning,
        warnings::{CHARGING, DEAD, EMPTY_SHIELD, FELL, LOW_BODY, LOW_SHIELD, Vitals, Warnings, word},
    };

    /// hit, charge, low, depleted, heartbeat (the disc's flags).
    fn elements() -> Vec<Warning> {
        let w = |sound: &str, looped, flags| Warning { sound: sound.into(), looped, flags, scale: 0.0 };
        vec![w("hit", false, FELL), w("charge", true, CHARGING), w("low", true, LOW_SHIELD), w("empty", true, EMPTY_SHIELD), w("heart", true, LOW_BODY)]
    }

    fn unit(shield: f32, body: f32) -> Vitals {
        Vitals { alive: true, shield, body, charging: false }
    }

    /// One tick at `now` with the unit as `v`: the plays.
    fn step(w: &mut Warnings, v: Vitals, now: i64) -> (Vec<usize>, Vec<usize>) {
        w.tick(v.shield, now);
        let p = w.frame(&elements(), &v);
        (p.loops, p.shots)
    }

    #[test]
    fn the_word_follows_the_thresholds_strictly() {
        assert_eq!(word(&unit(1.0, 1.0), false), 0);
        assert_eq!(word(&unit(0.25, 1.0), false), 0, "s = 0.25 is not low");
        assert_eq!(word(&unit(0.2, 1.0), false), LOW_SHIELD);
        assert_eq!(word(&unit(0.0, 1.0), false), EMPTY_SHIELD, "empty, not low (the low loop needs s > 0)");
        assert_eq!(word(&unit(0.5, 0.25), false), 0, "b = 0.25 is not low");
        assert_eq!(word(&unit(0.5, 0.2), false), LOW_BODY);
        assert_eq!(word(&Vitals { alive: false, ..unit(0.0, 0.0) }, true), DEAD);
        assert_eq!(word(&Vitals { charging: true, ..unit(0.5, 1.0) }, true), CHARGING | FELL);
    }

    /// The spec's fight: a hit plays `hit` once; under a quarter `low`
    /// loops; at zero `empty` replaces it; a low body adds the heart;
    /// recharge adds `charge` and the alarms stop as s passes 0 and 0.25.
    #[test]
    fn a_fight_plays_and_stops_the_loops() {
        let mut w = Warnings::default();
        assert_eq!(step(&mut w, unit(1.0, 1.0), 0), (vec![], vec![]));
        // Hit to 0.6: the one-shot once; the 15-tick hold must not repeat it.
        assert_eq!(step(&mut w, unit(0.6, 1.0), 1), (vec![], vec![0]));
        for t in 2..16 {
            assert_eq!(step(&mut w, unit(0.6, 1.0), t), (vec![], vec![]), "latched at {t}");
        }
        // The hold ends; the memory follows; a new hit is a new episode.
        assert_eq!(step(&mut w, unit(0.6, 1.0), 17), (vec![], vec![]));
        assert_eq!(step(&mut w, unit(0.2, 1.0), 18), (vec![2], vec![0]), "a new episode, and low");
        assert_eq!(step(&mut w, unit(0.0, 1.0), 19).0, vec![3], "empty: the low loop stops");
        assert_eq!(step(&mut w, unit(0.0, 0.3), 20).0, vec![3]);
        assert_eq!(step(&mut w, unit(0.0, 0.2), 21).0, vec![3, 4], "the heartbeat joins");
        // Recharging: the charge loop, low again from s > 0, silent from 0.25.
        let rising = |s| Vitals { charging: true, ..unit(s, 0.2) };
        assert_eq!(step(&mut w, rising(0.1), 300).0, vec![1, 2, 4]);
        assert_eq!(step(&mut w, rising(0.5), 301).0, vec![1, 4]);
        // Full: only the heartbeat (the body does not regenerate).
        assert_eq!(step(&mut w, unit(1.0, 0.2), 302).0, vec![4]);
    }

    #[test]
    fn death_stops_everything_and_clears_the_memory() {
        let mut w = Warnings::default();
        step(&mut w, unit(0.0, 0.2), 0);
        let dead = Vitals { alive: false, ..unit(0.0, 0.0) };
        assert_eq!(step(&mut w, dead, 1), (vec![], vec![]));
        assert_eq!(w, Warnings::default());
        // Respawned at full: the first frame marks nothing.
        assert_eq!(step(&mut w, unit(1.0, 1.0), 100), (vec![], vec![]));
    }
}
