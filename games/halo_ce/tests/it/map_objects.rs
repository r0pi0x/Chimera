//! Halo's map objects on synthetic data: teleporter pairs as the engine's
//! teleporters (`game::maps::teleporters`: pairing by team index, placed
//! and sized by scenario.md's rule) and climbable sheets as ladder brushes
//! (`game::maps::collision::ladder_brush`: the sheet's extent, a thin slab).

use bevy::math::Vec3;
use blam_formats::{
    cache::{TagId, TagRef, class},
    scenario::{NetgameFlag, NetgameFlagType},
};
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
    assert!((t.telefrag.unwrap() - 3.0).abs() < 1e-5, "90 ticks at 30 Hz");
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
