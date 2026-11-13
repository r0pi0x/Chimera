//! Animation data and playback (`lithtech_formats::anim` on
//! `ltb`'s animation section) on records written here from the tables of
//! specs/combat_arms/animation.md (no game data): the synthetic fixture
//! in its three compressions, samples, the local matrix, constant
//! channels, playback, looping, a transition, a two-tracker blend, rates,
//! key strings and the weapon rate rule. The install cases are
//! `heavy::ltb_install`.

use lithtech_formats::{
    anim::{self, AnimList, Tracker},
    ltb::Model,
};

use super::ltb::{Fixture, W};

/// The spec's record ("Synthetic fixture") in compression 2, 3 or 0.
fn record(compression: u32) -> Vec<u8> {
    let mut w = W::default();
    w.f32s(&[16., 32., 16.]).str("test").u32(compression).u32(150).u32(2);
    w.u32(0).str("FIRE_KEY").u32(100).str("");
    match compression {
        2 => {
            w.u32(2).i16(16).i16(32).i16(-48).i16(48).i16(32).i16(80);
            w.u32(2).i16(0).i16(0).i16(0).i16(32767).i16(0).i16(23170).i16(0).i16(23170);
        }
        3 => {
            w.u32(2).f32s(&[1., 2., -3., 3., 2., 5.]);
            w.u32(2).i16(0).i16(0).i16(0).i16(32767).i16(0).i16(23170).i16(0).i16(23170);
        }
        _ => {
            w.u8(0).f32s(&[1., 2., -3., 3., 2., 5.]);
            w.f32s(&[0., 0., 0., 1., 0., 0.707_106_8, 0., 0.707_106_8]);
        }
    }
    w.0
}

/// The record embedded in the models_ltb fixture (one node), with its
/// binding: box (16, 32, 16), translation 0.
fn model(compression: u32, extra: Vec<Vec<u8>>) -> Model {
    let mut f = Fixture::spec();
    f.animations = vec![record(compression)];
    f.bindings = vec![vec![("test".into(), [16., 32., 16.], [0.; 3])]];
    for (i, a) in extra.into_iter().enumerate() {
        f.animations.push(a);
        f.bindings[0].push((format!("extra{i}"), [1.; 3], [0.; 3]));
    }
    Model::parse(&f.bytes()).unwrap()
}

fn close(a: &[f32], b: &[f32], tol: f32) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tol)
}

#[test]
fn record_sizes() {
    assert_eq!(record(2).len(), 86);
    assert_eq!(record(3).len(), 98);
    assert_eq!(record(0).len(), 107);
    for (c, bytes) in [(2, 28), (3, 40), (0, 56)] {
        let m = model(c, vec![]);
        assert_eq!(m.track_bytes(), bytes, "compression {c}");
        assert_eq!(m.animations[0].keys.len(), 2);
    }
}

/// The expected samples table, in every compression.
#[test]
fn samples() {
    let rows = [
        (0.0, [1., 2., -3.], [0., 0., 0., 1.]),
        (25.0, [1.5, 2., -1.], [0., 0.195_092, 0., 0.980_786]),
        (50.0, [2., 2., 1.], [0., 0.382_686, 0., 0.923_881]),
        (75.0, [2.5, 2., 3.], [0., 0.555_575, 0., 0.831_474]),
        (100.0, [3., 2., 5.], [0., 0.707_114, 0., 0.707_114]),
    ];
    for c in [2, 3, 0] {
        let m = model(c, vec![]);
        let list = AnimList::build(&m, &[]);
        let e = &list.entries[0];
        assert_eq!((e.dims, e.translation, e.length()), ([16., 32., 16.], [0.; 3], 100));
        for (t, p, q) in rows {
            let (sp, sq) = anim::sample(e, 0, t);
            assert!(close(&sp, &p, 1e-5), "c{c} t{t}: {sp:?}");
            let tol = if c == 0 { 2e-5 } else { 1e-5 };
            // The f32 quaternion is exactly unit; the i16 one 0.707114.
            let q = if c == 0 && t == 100.0 { [0., 0.707_106_8, 0., 0.707_106_8] } else { q };
            assert!(close(&sq, &q, tol), "c{c} t{t}: {sq:?}");
        }
        // u at 25 ms.
        assert!(close(&[anim::key_span(&e.animation, 25.0).2], &[0.25], 1e-6));
    }
}

/// "At t = 50 the local matrix has m00 = m22 = 0.707104, m02 = 0.707110,
/// m20 = -0.707110, translation (2, 2, 1)".
#[test]
fn local_matrix() {
    let m = model(2, vec![]);
    let list = AnimList::build(&m, &[]);
    let (p, q) = anim::sample(&list.entries[0], 0, 50.0);
    let l = anim::matrix(p, q);
    assert!(close(&[l[0][0], l[2][2], l[0][2], l[2][0]], &[0.707_104, 0.707_104, 0.707_110, -0.707_110], 2e-5));
    assert!(close(&[l[0][3], l[1][3], l[2][3]], &[2., 2., 1.], 1e-6));
    // The node's +Z axis in the parent frame.
    let z = anim::transform_vector(&l, [0., 0., 1.]);
    assert!(close(&z, &[0.707, 0., 0.707], 1e-3));
}

/// Constant channels: one stored value everywhere, none: identity.
#[test]
fn constant_channels() {
    let mut w = W::default();
    w.f32s(&[1.; 3]).str("const").u32(3).u32(0).u32(2).u32(0).str("").u32(100).str("");
    w.u32(1).f32s(&[1., 2., -3.]).u32(0);
    let m = model(2, vec![w.0]);
    let list = AnimList::build(&m, &[]);
    for t in [0.0, 40.0, 100.0] {
        assert_eq!(anim::sample(&list.entries[1], 0, t), ([1., 2., -3.], [0., 0., 0., 1.]));
    }
    // 2 values for 3 keys: refused ("Filling missing keys").
    let mut w = W::default();
    w.f32s(&[1.; 3]).str("bad").u32(3).u32(0).u32(3).u32(0).str("").u32(1).str("").u32(2).str("");
    w.u32(2).f32s(&[0.; 6]).u32(0);
    let mut f = Fixture::spec();
    f.animations = vec![w.0];
    f.bindings = vec![vec![("bad".into(), [1.; 3], [0.; 3])]];
    assert!(Model::parse(&f.bytes()).is_err());
    // Compression 4: refused.
    let mut b = record(2);
    b[18..22].copy_from_slice(&4u32.to_le_bytes());
    let mut f = Fixture::spec();
    f.animations = vec![b];
    assert!(Model::parse(&f.bytes()).is_err());
}

fn run(t: &mut Tracker, list: &AnimList, delta: u32) -> Vec<String> {
    let mut fired = Vec::new();
    t.update(list, delta, &mut |s| fired.push(s.to_string()));
    fired
}

/// "Playback, non-looping, rate 1, Δ = 30" and "Looping, Δ = 30".
#[test]
fn playback() {
    let m = model(2, vec![]);
    let list = AnimList::build(&m, &[]);
    let mut t = Tracker::on(0, false);
    assert_eq!(run(&mut t, &list, 30), ["FIRE_KEY"]);
    assert_eq!(t.time, 30);
    run(&mut t, &list, 30);
    run(&mut t, &list, 30);
    assert_eq!(t.time, 90);
    assert!(!t.done(&list));
    run(&mut t, &list, 30);
    assert_eq!(t.time, 101);
    assert!(t.done(&list));

    let mut t = Tracker::default();
    for _ in 0..3 {
        run(&mut t, &list, 30);
    }
    run(&mut t, &list, 30);
    assert_eq!(t.time, 20);
    assert_eq!(t.pose(&list, 0), anim::sample(&list.entries[0], 0, 100.0));
    assert_eq!(run(&mut t, &list, 30), ["FIRE_KEY"]);
    assert_eq!(t.time, 50);
    assert!(!t.done(&list));
}

/// "Transition: from any animation into this one, Δ = 60".
#[test]
fn transition() {
    // An animation with I = 0 to start from.
    let mut w = W::default();
    w.f32s(&[1.; 3]).str("from").u32(2).u32(0).u32(2).u32(0).str("").u32(500).str("");
    w.u32(1).i16(0).i16(0).i16(0).u32(0);
    let m = model(2, vec![w.0]);
    let list = AnimList::build(&m, &[]);
    let mut t = Tracker::on(1, true);
    run(&mut t, &list, 100);
    t.start(0);
    // Quirk 4: before the next update, the previous entry's next key.
    assert_eq!(t.pose(&list, 0), ([0.; 3], [0., 0., 0., 1.]));
    assert!(run(&mut t, &list, 60).is_empty());
    assert!(close(&[t.blend], &[0.4], 1e-6));
    assert_eq!(t.time, 0);
    // Halfway between (0, 0, 0) and key 0's (1, 2, -3), at 0.4.
    assert!(close(&t.pose(&list, 0).0, &[0.4, 0.8, -1.2], 1e-5));
    run(&mut t, &list, 60);
    assert!(close(&[t.blend], &[0.8], 1e-6));
    assert_eq!(run(&mut t, &list, 60), ["FIRE_KEY"]);
    assert_eq!((t.transition, t.time), (None, 30));
}

/// `Tracker::seek` reaches what `update` does after a start, in the
/// transition and past it, looping and not (another machine's copy).
#[test]
fn seek_matches_playback() {
    let mut w = W::default();
    w.f32s(&[1.; 3]).str("from").u32(2).u32(0).u32(2).u32(0).str("").u32(500).str("");
    w.u32(1).i16(0).i16(0).i16(0).u32(0);
    let m = model(2, vec![w.0]);
    let list = AnimList::build(&m, &[]);
    for looping in [true, false] {
        let mut played = Tracker::on(1, true);
        run(&mut played, &list, 100);
        let key = played.b;
        played.start(0);
        played.looping = looping;
        let mut elapsed = 0;
        for _ in 0..20 {
            run(&mut played, &list, 15);
            elapsed += 15;
            let mut seen = Tracker::on(1, true);
            seen.looping = looping;
            seen.seek(&list, 0, Some((1, key)), elapsed);
            assert_eq!((seen.transition, seen.time), (played.transition, played.time), "{elapsed} ms");
            // The tick a loop wraps on, `update` holds the last key (its
            // scan isn't redone): the pose agrees from the next one.
            if played.a != played.b || played.transition.is_some() {
                assert_eq!(seen.pose(&list, 0), played.pose(&list, 0), "{elapsed} ms");
            }
        }
    }
    // No transition: from t = 0 of a playing entry.
    let mut played = Tracker::on(0, true);
    let mut seen = Tracker::on(0, true);
    run(&mut played, &list, 70);
    seen.seek(&list, 0, None, 70);
    assert_eq!((seen.time, seen.pose(&list, 0)), (played.time, played.pose(&list, 0)));
}

/// "Two-tracker blend".
#[test]
fn two_tracker_blend() {
    let mut w = W::default();
    w.f32s(&[1.; 3]).str("still").u32(2).u32(0).u32(1).u32(0).str("");
    w.u32(1).i16(0).i16(0).i16(0).u32(1).i16(0).i16(0).i16(0).i16(32767);
    let m = model(2, vec![w.0]);
    let list = AnimList::build(&m, &[]);
    let base = anim::sample(&list.entries[0], 0, 50.0);
    let layer = anim::sample(&list.entries[1], 0, 0.0);
    let (p, q) = anim::blend_layer(base, layer, 0.5, 1.0, layer);
    assert!(close(&p, &[1., 1., 0.5], 1e-5));
    assert!(close(&q, &[0., 0.195_092, 0., 0.980_786], 1e-5));
    // Weight 0 or multiplier 0: skipped; 2.0: additive.
    assert_eq!(anim::blend_layer(base, layer, 0.0, 1.0, layer), base);
    assert_eq!(anim::blend_layer(base, layer, 1.0, 0.0, layer), base);
    let (pa, qa) = anim::blend_layer(base, layer, 2.0, 1.0, layer);
    assert!(close(&pa, &base.0, 1e-6) && close(&qa, &base.1, 1e-5));
}

/// "Time base and rate": with rate 0.5 and Δ = 3 ms the steps are 1, 2,
/// 1, 2, ... ms; rate 0 does not advance. Test case 10's shape (keys 0,
/// 600, 1566) looping with Δ = 1000.
#[test]
fn rate_and_loop_wrap() {
    let mut w = W::default();
    w.f32s(&[1.; 3]).str("idle_0").u32(3).u32(0).u32(3);
    w.u32(0).str("").u32(600).str("").u32(1566).str("");
    w.u32(1).f32s(&[0.; 3]).u32(1).i16(0).i16(0).i16(0).i16(32767);
    let m = model(2, vec![w.0]);
    let list = AnimList::build(&m, &[]);
    let mut t = Tracker::on(1, true);
    t.set_rate(0.5);
    let mut times = vec![];
    for _ in 0..4 {
        let before = t.time;
        run(&mut t, &list, 3);
        times.push(t.time - before);
    }
    assert_eq!(times, [1, 2, 1, 2]);
    t.set_rate(0.0);
    let before = t.time;
    run(&mut t, &list, 50);
    assert_eq!(t.time, before);

    let mut t = Tracker::on(1, true);
    run(&mut t, &list, 1000);
    assert_eq!((t.time, t.a, t.b), (1000, 1, 2));
    assert!(close(&[t.u], &[400.0 / 966.0], 1e-6));
    run(&mut t, &list, 1000);
    assert_eq!((t.time, t.a, t.b), (434, 2, 2));
    run(&mut t, &list, 1000);
    assert_eq!((t.time, t.a, t.b), (1434, 1, 2));
    assert!(close(&[t.u], &[834.0 / 966.0], 1e-6));
    // Setting a time re-scans without firing.
    t.set_time(&list, 700);
    assert_eq!((t.a, t.b, t.next_key), (1, 2, 2));
    assert_eq!(anim::weapon_rate(2.5), 1.0);
    assert_eq!(anim::weapon_rate(1.5), 1.5);
}

/// "Edge cases": N <= 1 never advances and is always done.
#[test]
fn single_key_never_moves() {
    let m = model(2, vec![]);
    let mut f = Fixture::spec();
    f.bindings = vec![vec![("LST".into(), [15., 50., 15.], [0., -50., 0.])]];
    let single = Model::parse(&f.bytes()).unwrap();
    let list = AnimList::build(&single, &[Some(&m)]);
    assert_eq!(list.offsets, [Some(0), Some(1)]);
    assert_eq!(list.find("TEST"), Some(1));
    let mut t = Tracker::on(0, true);
    run(&mut t, &list, 500);
    assert_eq!((t.time, t.u), (0, 0.0));
    assert!(t.done(&list));
    // The root translation from the binding on node 0.
    assert_eq!(t.pose(&list, 0).0, [0., -50., 0.]);
}

/// "String keys": commands at `;` and control characters, arguments at
/// spaces, grouping and `%`.
#[test]
fn key_strings() {
    assert_eq!(anim::key_commands("FIRE_KEY"), [vec!["FIRE_KEY"]]);
    assert_eq!(anim::key_commands("sound_key 3"), [vec!["sound_key", "3"]]);
    assert_eq!(
        anim::key_commands("FX \"a b\";RS 1 (x y)\nSHELL_CASING"),
        [vec!["FX", "a b"], vec!["RS", "1", "x y"], vec!["SHELL_CASING"]]
    );
    assert_eq!(anim::key_commands("a%;b ; ;"), [vec!["a;b"]]);
    assert!(anim::key_commands("").is_empty());
}

/// Node evaluation and skinning matrices: at bind pose S = identity, and
/// a rotation-only node keeps its bind offset.
#[test]
fn node_evaluation() {
    use super::ltb::{IDENTITY, node};
    let mut f = Fixture::spec();
    let child_bind = [1., 0., 0., 5., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];
    f.nodes = (node("Scene Root", 0, 0, IDENTITY, &[node("arm", 1, 2, child_bind, &[])]), 2);
    let mut rec = super::ltb::lst_record();
    rec.extend([0u8]);
    rec.extend([7f32, 7., 7., 0., 0., 0., 1.].iter().flat_map(|v| v.to_le_bytes()));
    f.animations = vec![rec];
    let m = Model::parse(&f.bytes()).unwrap();
    let list = AnimList::build(&m, &[]);
    let t = Tracker::default();
    let pose: Vec<_> = (0..2).map(|j| t.pose(&list, j)).collect();
    // The root carries the binding's (0, -50, 0); the arm ignores its
    // animated (7, 7, 7) and sits 5 along X.
    let g = anim::globals(&m, &pose);
    assert_eq!([g[1][0][3], g[1][1][3], g[1][2][3]], [5., -50., 0.]);
    let s = anim::skin_matrices(&m, &g);
    assert_eq!([s[1][0][3], s[1][1][3], s[1][2][3]], [0., -50., 0.]);
    let bind = anim::globals(&m, &[([0.; 3], [0., 0., 0., 1.]), ([5., 0., 0.], [0., 0., 0., 1.])]);
    for s in anim::skin_matrices(&m, &bind) {
        assert_eq!(s, anim::IDENTITY);
    }
}

/// `AnimList::find` (an index made on the first call) gives what a
/// case-blind scan gives: the first entry of a name in any case, none for
/// a missing one, and a scan again once entries are added or removed.
#[test]
fn find_matches_a_case_blind_scan() {
    let m = model(2, vec![record(2), record(3)]);
    let mut list = AnimList::build(&m, &[]);
    let rename = |list: &mut AnimList, i: usize, n: &str| std::sync::Arc::make_mut(&mut list.entries[i].animation).name = n.to_string();
    rename(&mut list, 0, "LST");
    rename(&mut list, 1, "lst");
    rename(&mut list, 2, "Run_Forward");
    let scan = |list: &AnimList, q: &str| list.entries.iter().position(|e| e.name().eq_ignore_ascii_case(q));
    for q in ["LST", "lst", "Lst", "RUN_FORWARD", "run_forward", "nope", "", &"x".repeat(80)] {
        assert_eq!(list.find(q), scan(&list, q), "{q}");
    }
    assert_eq!(list.find("lst"), Some(0), "the first of a name");
    list.entries.remove(0);
    assert_eq!(list.find("LST"), Some(0), "scanned after a change in count");
    assert_eq!(list.find("run_forward"), Some(1));
}

/// A list shares its model's animations (each is megabytes, and every body
/// built from one model holds one: docs/performance.md, "A session's memory").
#[test]
fn lists_share_the_models_animations() {
    let m = model(2, Vec::new());
    let (a, b) = (AnimList::build(&m, &[]), AnimList::build(&m, &[]));
    assert!(std::sync::Arc::ptr_eq(&a.entries[0].animation, &b.entries[0].animation));
    assert!(std::sync::Arc::ptr_eq(&a.entries[0].animation, &m.animations[0]));
}


/// The sum over k, as `mul` was written before it was unrolled.
fn mul_by_sum(a: &lithtech_formats::ltb::Mat4, b: &lithtech_formats::ltb::Mat4) -> lithtech_formats::ltb::Mat4 {
    let mut o = [[0.0; 4]; 4];
    for (r, row) in o.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    o
}

#[test]
fn mul_gives_the_sum_s_bits() {
    let mut x = 0x2545_f491_4f6c_dd1du64;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        // Signed, of many magnitudes, zeros of both signs among them.
        match x % 11 {
            0 => 0.0,
            1 => -0.0,
            _ => ((x >> 11) as f32 / (1u64 << 53) as f32 - 0.5) * 10f32.powi((x % 7) as i32 - 3),
        }
    };
    for _ in 0..20_000 {
        let a: lithtech_formats::ltb::Mat4 = std::array::from_fn(|_| std::array::from_fn(|_| next()));
        let b: lithtech_formats::ltb::Mat4 = std::array::from_fn(|_| std::array::from_fn(|_| next()));
        let (got, want) = (lithtech_formats::anim::mul(&a, &b), mul_by_sum(&a, &b));
        for r in 0..4 {
            for c in 0..4 {
                assert_eq!(got[r][c].to_bits(), want[r][c].to_bits(), "{a:?} x {b:?} at {r},{c}");
            }
        }
    }
}

/// `Tracker::position` and `blend_layer_position` are `pose`'s and
/// `blend_layer`'s translations, bit for bit: through play, a loop's
/// wrap, a transition, and every kind of layer.
#[test]
fn positions_are_the_poses_translations() {
    let mut w = W::default();
    w.f32s(&[1.; 3]).str("from").u32(2).u32(0).u32(2).u32(0).str("").u32(500).str("");
    w.u32(1).i16(0).i16(0).i16(0).u32(0);
    let m = model(2, vec![w.0]);
    let list = AnimList::build(&m, &[]);
    let mut t = Tracker::on(1, true);
    let mut other = Tracker::default();
    for step in 0..40u32 {
        if step == 11 {
            t.start(0);
        }
        run(&mut t, &list, 7 + step * 13 % 50);
        run(&mut other, &list, 31);
        for node in 0..m.nodes.len() {
            let (p, q) = t.pose(&list, node);
            assert_eq!(t.position(&list, node), p, "step {step} node {node}");
            let layer = other.pose(&list, node);
            let base = list.entries[other.previous].animation.key_value(node, 0);
            for (wt, mul) in [(0.0, 1.0), (0.5, 1.0), (1.0, 0.3), (2.0, 1.0), (0.7, 0.0)] {
                assert_eq!(
                    anim::blend_layer_position(p, layer.0, wt, mul, base.0),
                    anim::blend_layer((p, q), layer, wt, mul, base).0,
                    "weight {wt} multiplier {mul}"
                );
            }
        }
    }
}
