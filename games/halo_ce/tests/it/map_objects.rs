//! Halo's map objects on synthetic data: teleporter pairs as the engine's
//! teleporters (`game::maps::teleporters`: pairing by team index, placed
//! and sized by scenario.md's rule, the exit's test, message and telefrag
//! by multiplayer_rules.md 6.8) and climbable sheets as ladder brushes
//! (`game::maps::collision::ladder_brush`: the sheet's extent, a thin slab).

use bevy::math::Vec3;
use blam_formats::{
    cache::{TagId, TagRef, class},
    scenario::{NetgameFlag, NetgameFlagType},
};
use chimera_world::map::teleport::{Exit, Outcome, Walker, step, step_counting};
use halo_ce::game::{
    METERS_PER_UNIT,
    maps::{
        collision::{LADDER_SKIN, ladder_brush},
        space,
        teleporters::teleporters,
    },
};

fn flag(t: NetgameFlagType, team: i16, position: [f32; 3], facing: f32) -> NetgameFlag {
    NetgameFlag {
        position,
        facing,
        flag_type: t,
        team_index: team,
        reference: TagRef {
            class: class::ITEM_COLLECTION,
            path_address: 0,
            id: TagId::NONE,
        },
    }
}

#[test]
fn teleporter_pairs_become_pads_by_team_index() {
    use NetgameFlagType::{TeleporterSource as S, TeleporterTarget as T};
    let flags = [
        flag(S, 0, [0.0, 0.0, 0.0], 0.0),
        flag(T, 1, [50.0, 0.0, 0.0], 0.0),
        flag(S, 1, [10.8, 0.0, 0.0], 0.0),
        flag(T, 0, [10.0, 0.0, 0.0], 1.5707964),
        // A source without a target is no pad.
        flag(S, 7, [3.0, 3.0, 0.0], 0.0),
        flag(NetgameFlagType::Hill, 0, [1.0, 1.0, 1.0], 0.0),
    ];
    let t = teleporters(&flags);
    assert_eq!(t.pads.len(), 2);
    assert_eq!(t.pads[0].source, Vec3::ZERO);
    assert_eq!(t.pads[0].dest, space::point([10.0, 0.0, 0.0]));
    assert!((t.pads[0].dest_yaw - t.pads[0].source_yaw - 1.5707964).abs() < 1e-5);
    assert_eq!(t.pads[1].source, space::point([10.8, 0.0, 0.0]));
    assert_eq!(t.pads[1].dest, space::point([50.0, 0.0, 0.0]));
    assert!((t.radius - 0.5 * METERS_PER_UNIT).abs() < 1e-5);
    assert!((t.rearm - METERS_PER_UNIT).abs() < 1e-5);
    assert!(t.keep_velocity);
    assert_eq!(t.telefrag, None, "the count is the blocker's (the pill's rule)");
    let Exit::Pill { radius, standing, crouched, telefrag_ticks, message_every, .. } = t.exit else { panic!("{:?}", t.exit) };
    assert!((radius - 0.2 * METERS_PER_UNIT).abs() < 1e-5);
    assert!((standing - 0.3 * METERS_PER_UNIT).abs() < 1e-5 && (crouched - 0.1 * METERS_PER_UNIT).abs() < 1e-5);
    assert_eq!((telefrag_ticks, message_every), (90.0, 121));
}

/// multiplayer_rules.md 6.8's exit on the pads above: a Spartan standing
/// 0.39 WU from the target flag blocks (0.41 does not; nor does one 0.45
/// away, inside the old "source radius" reading); the blocked message
/// shows on the first refusal and the 122nd; the blocker dies in the tick
/// of the 90th refused try.
#[test]
fn a_teleporters_exit_is_blocked_by_the_arriving_pill() {
    use NetgameFlagType::{TeleporterSource as S, TeleporterTarget as T};
    let t = teleporters(&[flag(S, 0, [0.0, 0.0, 0.0], 0.0), flag(T, 0, [10.0, 0.0, 0.0], 0.0)]);
    let walker = |p: [f32; 3]| Walker { feet: space::point(p), alive: true, ..Default::default() };
    let blocked = |off: f32| {
        let mut w = [walker([0.1, 0.0, 0.0]), walker([10.0, off, 0.0])];
        matches!(step(&t, &mut w, 1.0 / 30.0).0[0].1, Outcome::Blocked { .. })
    };
    assert!(blocked(0.39));
    assert!(!blocked(0.41));
    assert!(!blocked(0.45));
    let mut w = [walker([0.1, 0.0, 0.0]), walker([10.0, 0.1, 0.0])];
    let mut countdown = 0;
    let mut shown = Vec::new();
    let mut killed = None;
    for n in 1..=130 {
        let (out, kill) = step_counting(&t, &mut w, 1.0 / 30.0, &mut countdown);
        if let Outcome::Blocked { message: true, .. } = out[0].1 {
            shown.push(n);
        }
        if !kill.is_empty() && killed.is_none() {
            killed = Some(n);
        }
    }
    assert_eq!(killed, Some(90), "the telefrag in the tick of the 90th refused try");
    assert_eq!(shown, vec![1, 122]);
}

#[test]
fn a_climbable_sheet_is_a_thin_ladder_brush_of_its_extent() {
    // An upright 1 m x 3 m sheet in the plane z = 2.
    let sheet = [
        Vec3::new(0.0, 0.0, 2.0),
        Vec3::new(1.0, 0.0, 2.0),
        Vec3::new(1.0, 3.0, 2.0),
        Vec3::new(0.0, 3.0, 2.0),
    ];
    let b = ladder_brush(&sheet).unwrap();
    assert!(b.ladder);
    assert!((b.min - Vec3::new(0.0, 0.0, 2.0 - LADDER_SKIN)).length() < 1e-5, "{}", b.min);
    assert!((b.max - Vec3::new(1.0, 3.0, 2.0 + LADDER_SKIN)).length() < 1e-5, "{}", b.max);
    assert!(b.contains(Vec3::new(0.5, 1.5, 2.0 + LADDER_SKIN * 0.5)));
    assert!(b.contains(Vec3::new(0.5, 1.5, 2.0 - LADDER_SKIN * 0.5)));
    assert!(!b.contains(Vec3::new(0.5, 1.5, 2.1)));
    assert!(!b.contains(Vec3::new(1.1, 1.5, 2.0)));
    assert!(!b.contains(Vec3::new(0.5, 3.1, 2.0)));
    // Either winding (the twin's) gives the same brush.
    let mut twin = sheet;
    twin.reverse();
    let c = ladder_brush(&twin).unwrap();
    for p in [Vec3::new(0.5, 1.5, 2.01), Vec3::new(0.9, 0.1, 1.99), Vec3::new(-0.1, 1.0, 2.0)] {
        assert_eq!(b.contains(p), c.contains(p), "{p}");
    }
    // A triangle on a slant.
    let tri = [Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 1.0)];
    let t = ladder_brush(&tri).unwrap();
    assert!(t.contains((tri[0] + tri[1] + tri[2]) / 3.0));
    assert!(ladder_brush(&[Vec3::ZERO, Vec3::X, Vec3::X * 2.0]).is_none(), "degenerate");
}

/// A Halo player against an upright wall at x = 1 WU (the spec's frame),
/// facing it and looking up, jumping on and holding forward for two
/// seconds: its origin's height (WU), with the wall's face a ladder or not.
fn climb(ladder: bool) -> f32 {
    use crate::movement::{T, aabb, floor};
    use chimera_core::core::MapBrushes;
    let mut t = T::standing_on(vec![floor(), aabb(Vec3::new(1.0, -3.0, -1.0), Vec3::new(3.0, 3.0, 10.0))]);
    if ladder {
        let face = [[1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 1.0, 5.0], [1.0, -1.0, 5.0]].map(space::point);
        let brush = ladder_brush(&face).unwrap();
        t.sim.app.world_mut().insert_resource(MapBrushes(vec![brush]));
    }
    t.face(0.0);
    t.sim.intent(t.p).pitch = 0.8;
    t.throttle(1.0, 0.0);
    t.jump(true);
    t.ticks(2);
    t.jump(false);
    t.ticks(60);
    t.origin().z
}

#[test]
fn a_halo_player_climbs_a_ladder_sheet_by_the_aim() {
    let without = climb(false);
    let with = climb(true);
    eprintln!("after two seconds: {with:.3} WU up a ladder, {without:.3} WU against a wall");
    assert!(without < 0.05, "a plain wall is not climbed: {without}");
    assert!(with > 1.0, "a ladder is climbed: {with}");
}
