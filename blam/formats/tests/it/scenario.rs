//! The scenario, sky, item collection and object header readers on
//! synthetic maps: test case (a) of specs/halo_ce/scenario.md (the minimal
//! scenario with its expected values, the eligibility table, the rotation
//! and heading examples, the teleporter example, item drawing, spawn
//! timing, the sky light direction and the fog factor), then the parts the
//! spec's case leaves out (every block's layout, rejections).

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision, clippy::type_complexity)]

use blam_formats::cache::{MapFile, TagStruct, class};
use blam_formats::scenario::{
    GameType, GameTypes, ItemCollection, NetgameFlag, NetgameFlagType, ObjectHeader, ObjectType, Orientation, Scenario,
    Sky, heading_forward, netgame::teleporter_pairs, teleport_heading,
};
use blam_formats::scenario::{ScenarioType, TriggerVolumeType};

use crate::map_fixture::{CacheFile, Struct, TagData, c, null_ref};

const DEG: f32 = std::f32::consts::PI / 180.0;
const HALF_PI: f32 = 1.570_796_4;

fn close(a: [f32; 3], b: [f32; 3], tol: f32) {
    for k in 0..3 {
        assert!((a[k] - b[k]).abs() <= tol, "{a:?} vs {b:?}");
    }
}

// Tag indices of the synthetic map.
const SCNR: usize = 0;
const SKY: usize = 1;
const ITMC: usize = 2;
const ROCK: usize = 3;
const WEAP: usize = 4;

fn tag_list() -> Vec<(&'static str, &'static str)> {
    vec![
        ("scnr", "scenarios\\syn"),
        ("sky ", "levels\\test\\syn\\syn sky"),
        ("itmc", "item collections\\test\\rifle"),
        ("scen", "scenery\\test\\rock\\rock"),
        ("weap", "weapons\\rifle\\rifle"),
    ]
}

fn start(pos: [f32; 3], facing: f32, team: i16, bsp: i16, types: [i16; 4]) -> Vec<u8> {
    let mut s = Struct::new(0x34)
        .f32s(0, &pos)
        .f32(0x0C, facing)
        .i16(0x10, team)
        .i16(0x12, bsp);
    for (k, t) in types.iter().enumerate() {
        s = s.i16(0x14 + 2 * k, *t);
    }
    s.into()
}

fn scenery(palette: i16, flags_on_bsp: u16) -> Vec<u8> {
    Struct::new(72)
        .i16(0, palette)
        .i16(2, -1)
        .f32s(8, &[10.0, -20.0, 0.5])
        .f32s(0x14, &[HALF_PI, 0.0, 0.0])
        .u16(0x20, flags_on_bsp)
        .into()
}

/// The spec's minimal scenario, plus the other tags a reader needs.
fn synthetic() -> MapFile {
    let mut td = TagData::new(&tag_list());
    let skies = td.block(&[td.tag_ref(SKY).to_vec()]);
    let starts = td.block(&[
        start([1.0, 2.0, 0.5], 0.0, 0, 0, [1, 0, 0, 0]),
        start([-3.0, 4.0, 0.5], HALF_PI, 1, 0, [13, 0, 0, 0]),
    ]);
    let equipment = Struct::new(144)
        .u32(0, 1)
        .i16(4, 12)
        .i16(0x0E, 30)
        .f32s(0x40, &[5.0, 6.0, 1.0])
        .put(0x50, &td.tag_ref(ITMC))
        .into();
    let equipment = td.block(&[equipment]);
    let palette = td.block(&[Struct::new(48).put(0, &td.tag_ref(ROCK)).into()]);
    let scenery_block = td.block(&[scenery(0, 1)]);

    // Netgame flags: the teleporter example and one of each other kind.
    let flag = |pos: [f32; 3], facing: f32, kind: i16, team: i16| -> Vec<u8> {
        Struct::new(0x94)
            .f32s(0, &pos)
            .f32(0x0C, facing)
            .i16(0x10, kind)
            .i16(0x12, team)
            .put(0x14, &null_ref("itmc"))
            .into()
    };
    let flags = td.block(&[
        flag([0.0, 0.0, 0.0], 0.0, 6, 0),
        flag([10.8, 0.0, 0.0], 0.0, 6, 1),
        flag([10.0, 0.0, 0.0], HALF_PI, 7, 0),
        flag([50.0, 0.0, 0.0], 0.0, 7, 1),
        flag([1.0, 1.0, 1.0], 0.5, 0, 1),
        flag([2.0, 2.0, 2.0], 0.0, 8, 3),
    ]);

    let main = Struct::new(0x5B0)
        .put(0x030, &skies)
        .i16(0x03C, 1)
        .f32(0x04C, 3.141_592_7)
        .put(0x354, &starts)
        .put(0x378, &flags)
        .put(0x384, &equipment)
        .put(0x21C, &palette)
        .put(0x210, &scenery_block);
    td.set_struct(SCNR, &main.0);

    // The sky tag: Blood Gulch's values (scenario.md).
    let fog = |color: [f32; 3], density: f32, start: f32, opaque: f32| {
        Struct::new(32)
            .f32s(0, &color)
            .f32(0x14, density)
            .f32(0x18, start)
            .f32(0x1C, opaque)
            .0
    };
    let light = Struct::new(0x74)
        .put(0, &null_ref("lens"))
        .u32(0x4C, 3)
        .f32s(0x50, &[0.906, 0.894, 0.796])
        .f32(0x5C, 1.8)
        .f32(0x60, 50.0)
        .f32(0x68, 10.0 * DEG)
        .f32(0x6C, 40.0 * DEG)
        .f32(0x70, 0.0035);
    let lights = td.block(&[light.into()]);
    let sky = Struct::new(0xD0)
        .put(0x00, &null_ref("mode"))
        .put(0x10, &null_ref("antr"))
        .f32s(0x38, &[0.239, 0.294, 0.373])
        .f32(0x44, 20.0)
        .f32s(0x48, &[0.871, 0.843, 0.753])
        .f32(0x54, 0.2)
        .put(0x58, &fog([1.0, 0.902, 0.769], 0.4, 3.0, 100.0))
        .put(0x78, &fog([0.196, 0.173, 0.329], 1.0, 1.0, 120.0))
        .put(0x98, &null_ref("fog "))
        .put(0xC4, &lights);
    td.set_struct(SKY, &sky.0);

    // Item collection: weights 70, 90, 15, spawn time 60.
    let perm =
        |w: f32, tag: &TagData| -> Vec<u8> { Struct::new(0x54).f32(0x20, w).put(0x24, &tag.tag_ref(WEAP)).into() };
    let perms = td.block(&[perm(70.0, &td), perm(90.0, &td), perm(15.0, &td)]);
    td.set_struct(ITMC, &Struct::new(0x5C).put(0, &perms).i16(0x0C, 60).0);

    // A scenery object tag.
    let rock = Struct::new(0x108)
        .i16(0, 6)
        .f32(4, 2.5)
        .f32s(8, &[0.0, 0.0, 1.25])
        .put(0x28, &td.tag_ref(WEAP))
        .put(0x38, &null_ref("antr"))
        .put(0x70, &null_ref("coll"))
        .f32(0x104, 2.5);
    td.set_struct(ROCK, &rock.0);
    td.set_struct(WEAP, &Struct::new(0x20).0);

    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

#[test]
fn minimal_scenario_values() {
    let map = synthetic();
    let s = Scenario::read(&map).unwrap();
    assert_eq!(s.scenario_type, ScenarioType::Multiplayer);
    assert!((s.local_north - std::f32::consts::PI).abs() < 1e-6);

    // Skies.
    assert_eq!(s.skies.len(), 1);
    let sky_tag = map.tag(s.skies[0].sky.id).unwrap();
    assert_eq!(sky_tag.class, class::SKY);
    assert_eq!(sky_tag.path, "levels\\test\\syn\\syn sky");

    // Player starts.
    assert_eq!(s.player_starts.len(), 2);
    let (a, b) = (&s.player_starts[0], &s.player_starts[1]);
    assert_eq!(a.position, [1.0, 2.0, 0.5]);
    assert_eq!((a.team_index, a.bsp_index), (0, 0));
    assert_eq!(a.game_types, GameTypes([1, 0, 0, 0]));
    assert_eq!(a.forward(), [1.0, 0.0, 0.0]);
    close(b.forward(), [0.0, 1.0, 0.0], 1e-6);
    assert_eq!(b.position, [-3.0, 4.0, 0.5]);
    assert_eq!(b.game_types, GameTypes([13, 0, 0, 0]));
    assert_eq!(s.eligible_starts(GameType::CaptureTheFlag), [0]);
    assert_eq!(s.eligible_starts(GameType::Slayer), [1]);
    assert_eq!(s.eligible_starts(GameType::Race), [1]);

    // Netgame equipment.
    assert_eq!(s.netgame_equipment.len(), 1);
    let e = &s.netgame_equipment[0];
    assert!(e.levitate());
    assert_eq!(e.game_types, GameTypes([12, 0, 0, 0]));
    assert_eq!((e.team_index, e.spawn_time), (0, 30));
    assert_eq!(e.position, [5.0, 6.0, 1.0]);
    assert_eq!(e.facing, 0.0);
    assert_eq!(
        map.tag(e.item_collection.id).unwrap().path,
        "item collections\\test\\rifle"
    );
    assert_eq!(e.item_collection.class, class::ITEM_COLLECTION);
    assert_eq!(e.period_seconds(0), 30);
    assert_eq!(e.period_ticks(0), 900);
    assert!(e.eligible_for(GameType::Slayer));

    // Scenery and its palette.
    assert_eq!(s.scenery_palette.len(), 1);
    let rock = map.tag(s.scenery_palette[0].object.id).unwrap();
    assert_eq!(rock.path, "scenery\\test\\rock\\rock");
    assert_eq!(s.scenery.len(), 1);
    let sc = &s.scenery[0].header;
    assert_eq!((sc.palette_index, sc.name_index, sc.flags, sc.variant), (0, -1, 0, 0));
    assert_eq!(sc.position, [10.0, -20.0, 0.5]);
    assert_eq!(
        (sc.rotation.yaw, sc.rotation.pitch, sc.rotation.roll),
        (HALF_PI, 0.0, 0.0)
    );
    assert_eq!(sc.on_bsp_flags, 1);
    let o = sc.orientation();
    close(o.forward, [0.0, 1.0, 0.0], 1e-6);
    close(o.left, [-1.0, 0.0, 0.0], 1e-6);
    close(o.up, [0.0, 0.0, 1.0], 1e-6);
    assert!(sc.is_created(s.scenery_palette.len(), Some(0)));

    // Everything else is empty.
    assert!(s.bipeds.is_empty() && s.vehicles.is_empty() && s.weapons.is_empty() && s.machines.is_empty());
    assert!(s.starting_equipment.is_empty() && s.trigger_volumes.is_empty() && s.decals.is_empty());
    assert_eq!(s.scripts.0, 0);
    assert_eq!(s.structure_bsp_count.0, 0);
}

#[test]
fn skipped_scenery_copies() {
    let mut h = Scenario::read(&synthetic()).unwrap().scenery[0].header.clone();
    assert!(h.is_created(1, Some(0)));
    h.palette_index = 1;
    assert!(!h.is_created(1, Some(0)), "palette index out of range");
    h.palette_index = -1;
    assert!(!h.is_created(1, Some(0)), "palette index -1");
    h.palette_index = 0;
    h.on_bsp_flags = 0;
    assert!(!h.is_created(1, Some(0)), "on-BSP flags 0");
    assert!(h.is_created(1, None), "types that skip the BSP test");
    h.on_bsp_flags = 2;
    assert!(h.in_bsp(1) && !h.in_bsp(0));
}

#[test]
fn eligibility_table() {
    let games = [
        GameType::CaptureTheFlag,
        GameType::Slayer,
        GameType::Oddball,
        GameType::KingOfTheHill,
        GameType::Race,
    ];
    let rows: [(i16, [bool; 5]); 6] = [
        (0, [false; 5]),
        (1, [true, false, false, false, false]),
        (5, [false, false, false, false, true]),
        (12, [true; 5]),
        (13, [false, true, true, true, true]),
        (14, [false, true, true, true, false]),
    ];
    for (slot, expected) in rows {
        for (g, want) in games.iter().zip(expected) {
            let types = GameTypes([slot, 0, 0, 0]);
            assert_eq!(types.eligible_for(*g), want, "slot {slot} game {g:?}");
            // The slot may sit in any of the four positions.
            assert_eq!(GameTypes([0, 0, 0, slot]).eligible_for(*g), want);
        }
    }
    // Several slots: any may match.
    assert!(GameTypes([2, 3, 0, 0]).eligible_for(GameType::Oddball));
    assert!(!GameTypes([2, 3, 0, 0]).eligible_for(GameType::Race));
    assert!(GameTypes([0; 4]).eligible_when_idle());
    assert!(!GameTypes([12, 0, 0, 0]).eligible_when_idle());
    // A number that is no game type (0) matches nothing.
    assert!(!GameTypes([0, 0, 0, 0]).eligible_for_number(0));
    assert_eq!(GameType::from_code(5), Some(GameType::Race));
    assert_eq!(GameType::from_code(6), None);
}

#[test]
fn rotation_worked_examples() {
    // yaw, pitch, roll in degrees; F, L, U.
    let rows: [([f32; 3], [f32; 3], [f32; 3], [f32; 3]); 4] = [
        ([90.0, 0.0, 0.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        (
            [90.0, 30.0, 0.0],
            [0.0, 1.0, 0.0],
            [-0.8660, 0.0, -0.5],
            [-0.5, 0.0, 0.8660],
        ),
        (
            [90.0, 30.0, 45.0],
            [0.0, 0.7071, 0.7071],
            [-0.8660, 0.3536, -0.3536],
            [-0.5, -0.6124, 0.6124],
        ),
        (
            [45.0, 30.0, 60.0],
            [0.6124, 0.0474, 0.7891],
            [-0.6124, 0.6597, 0.4356],
            [-0.5, -0.75, 0.4330],
        ),
    ];
    for (angles, f, l, u) in rows {
        let o = Orientation::from_euler(angles[0] * DEG, angles[1] * DEG, angles[2] * DEG);
        close(o.forward, f, 1e-4);
        close(o.left, l, 1e-4);
        close(o.up, u, 1e-4);

        // A rotation: orthonormal, right-handed; the quaternion agrees
        // with the matrix on a point.
        let cross = |a: [f32; 3], b: [f32; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        close(cross(o.forward, o.left), o.up, 1e-5);
        let q = o.quaternion();
        let p = [0.3, -0.7, 1.1];
        let by_q = rotate_by_quaternion(q, p);
        close(by_q, o.rotate(p), 1e-5);
        let m = o.matrix();
        let by_m = [
            m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2],
            m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2],
            m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2],
        ];
        close(by_m, o.rotate(p), 1e-6);
    }
    // The up axis does not depend on yaw.
    let a = Orientation::from_euler(0.3, 0.4, 0.5).up;
    let b = Orientation::from_euler(-2.0, 0.4, 0.5).up;
    close(a, b, 1e-6);
    // Gimbal lock at pitch -90 degrees: only roll + yaw matters.
    let g1 = Orientation::from_euler(39.0 * DEG, -90.0 * DEG, -39.0 * DEG);
    let g2 = Orientation::from_euler(0.0, -90.0 * DEG, 0.0);
    close(g1.forward, g2.forward, 1e-5);
    close(g1.up, g2.up, 1e-5);
    // Identity and quaternion of the identity.
    assert_eq!(Orientation::from_euler(0.0, 0.0, 0.0), Orientation::IDENTITY);
    assert_eq!(Orientation::IDENTITY.quaternion(), [0.0, 0.0, 0.0, 1.0]);
}

/// The rotation of `p` by the unit quaternion `[i, j, k, w]`.
fn rotate_by_quaternion(q: [f32; 4], p: [f32; 3]) -> [f32; 3] {
    let (qv, w) = ([q[0], q[1], q[2]], q[3]);
    let cross = |a: [f32; 3], b: [f32; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let t = cross(qv, p).map(|x| 2.0 * x);
    let u = cross(qv, t);
    [p[0] + w * t[0] + u[0], p[1] + w * t[1] + u[1], p[2] + w * t[2] + u[2]]
}

#[test]
fn heading_example() {
    close(heading_forward(1.328_281), [0.2401, 0.9707, 0.0], 1e-4);
    close(heading_forward(HALF_PI), [0.0, 1.0, 0.0], 1e-6);
    close(heading_forward(0.0), [1.0, 0.0, 0.0], 0.0);
}

#[test]
fn teleporter_pairing_and_heading() {
    let s = Scenario::read(&synthetic()).unwrap();
    let pairs = s.teleporter_pairs();
    assert_eq!(pairs.len(), 2);
    assert_eq!((pairs[0].pair, pairs[0].source, pairs[0].target), (0, 0, Some(2)));
    assert_eq!((pairs[1].pair, pairs[1].source, pairs[1].target), (1, 1, Some(3)));
    assert_eq!(s.netgame_flags[4].flag_type, NetgameFlagType::CaptureTheFlag);
    assert_eq!(s.netgame_flags[5].flag_type, NetgameFlagType::Hill);
    assert_eq!(s.netgame_flags[5].team_index, 3);

    // The spec's example: heading 0.5 through A -> T0 becomes 2.0707964.
    let h = teleport_heading(0.5, s.netgame_flags[0].facing, s.netgame_flags[2].facing);
    assert!((h - 2.070_796_4).abs() < 1e-6);
    // Through B -> T1 (both facing 0) the heading is unchanged.
    assert_eq!(teleport_heading(2.0, 0.0, 0.0), 2.0);
    // Blood Gulch's example.
    let h = teleport_heading(1.5872, 1.5872, 3.0917);
    close(heading_forward(h), [-0.9988, 0.0499, 0.0], 1e-4);

    // A source without a target, and the first target in block order wins.
    let mk = |t: NetgameFlagType, team: i16| NetgameFlag {
        position: [0.0; 3],
        facing: 0.0,
        flag_type: t,
        team_index: team,
        reference: blam_formats::cache::TagRef {
            class: class::ITEM_COLLECTION,
            path_address: 0,
            id: blam_formats::cache::TagId::NONE,
        },
    };
    use NetgameFlagType::{TeleporterSource as S, TeleporterTarget as T};
    let flags = [mk(S, 4), mk(T, 5), mk(T, 4), mk(T, 4)];
    let p = teleporter_pairs(&flags);
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].target, Some(2));
    assert_eq!(teleporter_pairs(&flags[..2])[0].target, None);
}

#[test]
fn item_collection_draw_and_spawn_timing() {
    let map = synthetic();
    let s = Scenario::read(&map).unwrap();
    let tag = map.tag(s.netgame_equipment[0].item_collection.id).unwrap();
    let col = ItemCollection::read(&map, tag).unwrap();
    assert_eq!(col.spawn_time, 60);
    assert_eq!(col.permutations.len(), 3);
    assert_eq!(col.total_weight(), 175);
    let weights: Vec<f32> = col.permutations.iter().map(|p| p.weight).collect();
    assert_eq!(weights, [70.0, 90.0, 15.0]);
    assert_eq!(map.tag(col.permutations[0].item.id).unwrap().class, class::WEAPON);
    let which = |r: u32| col.pick(r).map(|p| p.weight);
    assert_eq!(which(0), Some(70.0));
    assert_eq!(which(69), Some(70.0));
    assert_eq!(which(70), Some(90.0));
    assert_eq!(which(159), Some(90.0));
    assert_eq!(which(160), Some(15.0));
    assert_eq!(which(174), Some(15.0));
    assert_eq!(which(175), None);

    let mut two = col.clone();
    two.permutations.truncate(2);
    two.permutations[0].weight = 50.0;
    two.permutations[1].weight = 50.0;
    assert_eq!(two.pick(49).unwrap().weight, 50.0);
    assert!(std::ptr::eq(two.pick(49).unwrap(), &two.permutations[0]));
    assert!(std::ptr::eq(two.pick(50).unwrap(), &two.permutations[1]));
    two.permutations.truncate(1);
    two.permutations[0].weight = 100.0;
    assert!((0..100).all(|r| std::ptr::eq(two.pick(r).unwrap(), &two.permutations[0])));
    two.permutations.clear();
    assert_eq!((two.pick(0).is_none(), two.total_weight()), (true, 0));

    // Spawn timing: the entry's own time, else the collection's, else 30.
    let mut e = s.netgame_equipment[0].clone();
    e.spawn_time = 0;
    let at = |e: &blam_formats::scenario::NetgameEquipment, coll: i16, ticks: &[u32]| -> Vec<bool> {
        ticks.iter().map(|&t| e.spawns_at_tick(t, coll)).collect()
    };
    assert_eq!(at(&e, 60, &[0, 1800, 3600, 900]), [true, true, true, false]);
    e.spawn_time = 45;
    assert_eq!(at(&e, 60, &[0, 1350, 2700, 1800]), [true, true, true, false]);
    e.spawn_time = 0;
    assert_eq!(at(&e, 0, &[0, 900, 1800, 450]), [true, true, true, false]);
    assert_eq!(e.period_ticks(0), 900);
}

#[test]
fn sky_values_and_formulas() {
    let map = synthetic();
    let s = Scenario::read(&map).unwrap();
    let sky = Sky::read(&map, map.tag(s.skies[0].sky.id).unwrap()).unwrap();
    assert!(sky.model.is_null() && sky.animation_graph.is_null() && sky.indoor_fog_screen.is_null());
    assert_eq!(sky.indoor_ambient_power, 20.0);
    assert_eq!(sky.outdoor_ambient_color.g, 0.843);
    assert_eq!(sky.outdoor_fog.color.g, 0.902);
    assert_eq!(
        (
            sky.outdoor_fog.max_density,
            sky.outdoor_fog.start_distance,
            sky.outdoor_fog.opaque_distance
        ),
        (0.4, 3.0, 100.0)
    );
    assert_eq!(sky.indoor_fog.opaque_distance, 120.0);
    assert_eq!(sky.lights.len(), 1);
    assert_eq!((sky.shader_functions.0, sky.animations.len()), (0, 0));
    let l = &sky.lights[0];
    assert_eq!(
        (l.flags, l.power, l.test_distance, l.marker_name.as_str()),
        (3, 1.8, 50.0, "")
    );
    close(l.direction(), [0.7544, 0.1330, 0.6428], 1e-4);
    assert!(l.lens_flare.is_null());

    // Fog: start 3, opaque 100, density 0.4.
    let f = &sky.outdoor_fog;
    assert_eq!(f.factor(3.0), 0.0);
    assert_eq!(f.factor(0.0), 0.0);
    assert!((f.factor(51.5) - 0.2).abs() < 1e-6);
    assert!((f.factor(100.0) - 0.4).abs() < 1e-6);
    assert!((f.factor(500.0) - 0.4).abs() < 1e-6);
    let mut g = f.clone();
    g.opaque_distance = 0.0;
    assert_eq!(g.factor(50.0), 0.0, "an opaque distance of 0 switches fog off");
    g.opaque_distance = 100.0;
    g.max_density = 0.0;
    assert_eq!(g.factor(100.0), 1.0, "density 0 counts as 1");
    g.start_distance = 150.0;
    assert_eq!((g.factor(149.0), g.factor(150.0)), (0.0, 1.0), "opaque raised to start");

    // The wrong class is refused.
    let err = Sky::read(&map, map.tag(s.scenery_palette[0].object.id).unwrap()).unwrap_err();
    assert!(err.to_string().contains("not a sky"), "{err}");
}

#[test]
fn object_header_essentials() {
    let map = synthetic();
    let s = Scenario::read(&map).unwrap();
    let tag = map.tag(s.scenery_palette[0].object.id).unwrap();
    let h = ObjectHeader::read(&map, tag).unwrap();
    assert_eq!(h.object_type, ObjectType::Scenery);
    assert_eq!(h.bounding_radius, 2.5);
    assert_eq!(h.bounding_offset, [0.0, 0.0, 1.25]);
    assert_eq!(h.origin_offset, [0.0; 3]);
    assert_eq!(h.render_bounding_radius, 2.5);
    assert_eq!(map.tag(h.model.id).unwrap().class, class::WEAPON);
    assert!(h.animation_graph.is_null() && h.collision_model.is_null());
    // A tag of another family is refused.
    let err = ObjectHeader::read(&map, map.tag(s.skies[0].sky.id).unwrap()).unwrap_err();
    assert!(err.to_string().contains("not an object"), "{err}");
    assert_eq!(ObjectType::from_code(10), None);
    assert_eq!(ObjectType::from_code(11), Some(ObjectType::SoundScenery));
}

/// A scenario with the given bytes patched into an otherwise minimal one.
fn with_main(patch: impl FnOnce(&mut Struct)) -> MapFile {
    let mut td = TagData::new(&tag_list());
    let mut main = Struct::new(0x5B0).i16(0x03C, 1);
    patch(&mut main);
    td.set_struct(SCNR, &main.0);
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

#[test]
fn other_blocks_and_placement_layouts() {
    let mut td = TagData::new(&tag_list());
    let header = |palette: i16| {
        Struct::new(0x28)
            .i16(0, palette)
            .i16(2, -1)
            .f32s(8, &[1.0, 2.0, 3.0])
            .f32s(0x14, &[0.1, 0.2, 0.3])
            .u16(0x20, 1)
            .u16(0x22, 2)
    };
    let vehicle: Vec<u8> = Struct::new(120).put(0, &header(3).0).f32(0x48, 0.5).u32(0x4C, 9).into();
    let weapon: Vec<u8> = Struct::new(92)
        .put(0, &header(1).0)
        .i16(0x48, 300)
        .i16(0x4A, 60)
        .u16(0x4C, 5)
        .into();
    let machine: Vec<u8> = Struct::new(64)
        .put(0, &header(0).0)
        .i16(0x28, 2)
        .i16(0x2A, -1)
        .u32(0x2C, 0x11)
        .u8(0x30, 8)
        .into();
    let control: Vec<u8> = Struct::new(64)
        .put(0, &header(0).0)
        .i16(0x28, -1)
        .i16(0x2A, 1)
        .u32(0x2C, 4)
        .u16(0x30, 0x103)
        .i16(0x34, 7)
        .into();
    let fixture: Vec<u8> = Struct::new(88)
        .put(0, &header(0).0)
        .i16(0x28, -1)
        .i16(0x2A, -1)
        .f32s(0x30, &[0.25, 0.5, 0.75])
        .f32(0x3C, 2.0)
        .f32(0x40, 0.4)
        .f32(0x44, 0.8)
        .into();
    let group: Vec<u8> = Struct::new(52).str(0, "door").f32(0x20, 1.0).u32(0x24, 3).into();
    let volume: Vec<u8> = Struct::new(96)
        .i16(0, 1)
        .str(4, "zone")
        .f32s(0x30, &[1.0, 0.0, 0.0])
        .f32s(0x3C, &[0.0, 0.0, 1.0])
        .f32s(0x48, &[4.0, 5.0, 6.0])
        .f32s(0x54, &[7.0, 8.0, 9.0])
        .into();
    let decal: Vec<u8> = Struct::new(16)
        .i16(0, 2)
        .u8(2, 0x88)
        .u8(3, 42)
        .f32s(4, &[1.5, 2.5, 3.5])
        .into();
    let name: Vec<u8> = Struct::new(36).str(0, "alarm").into();
    let profile: Vec<u8> = Struct::new(104)
        .str(0, "starter")
        .f32(0x20, 1.0)
        .f32(0x24, 2.0)
        .put(0x28, &td.tag_ref(WEAP))
        .i16(0x38, 12)
        .i16(0x3A, 108)
        .put(0x3C, &null_ref("weap"))
        .u8(0x50, 2)
        .u8(0x51, 1)
        .into();
    let mut eq = Struct::new(0xCC).u32(0, 2).i16(4, 14).i16(6, 2);
    for k in 0..6 {
        eq = eq.put(0x3C + 16 * k, &if k == 1 { td.tag_ref(ITMC) } else { null_ref("itmc") });
    }
    let blocks: [(usize, Vec<Vec<u8>>); 11] = [
        (0x240, vec![vehicle]),
        (0x270, vec![weapon]),
        (0x294, vec![machine]),
        (0x2AC, vec![control]),
        (0x2C4, vec![fixture]),
        (0x288, vec![group]),
        (0x360, vec![volume]),
        (0x3A8, vec![decal.clone(), decal]),
        (0x204, vec![name]),
        (0x348, vec![profile]),
        (0x390, vec![eq.into()]),
    ];
    let mut main = Struct::new(0x5B0);
    for (off, elems) in blocks {
        let b = td.block(&elems);
        main = main.put(off, &b);
    }
    // Count-only blocks: scripts 3 (garbage address: not followed), BSPs.
    main = main.put(0x49C, &crate::map_fixture::block_bytes(3, 0xDEAD));
    td.set_struct(SCNR, &main.0);
    let map = MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap();
    let s = Scenario::read(&map).unwrap();

    let v = &s.vehicles[0];
    assert_eq!(v.header.palette_index, 3);
    assert_eq!(v.header.name_index, -1);
    assert_eq!(v.header.position, [1.0, 2.0, 3.0]);
    assert_eq!(v.header.rotation.pitch, 0.2);
    assert_eq!((v.header.on_bsp_flags, v.header.misc_flags), (1, 2));
    assert_eq!((v.body_vitality, v.flags), (0.5, 9));
    let w = &s.weapons[0];
    assert_eq!((w.rounds_total, w.rounds_loaded, w.flags), (300, 60, 5));
    let m = &s.machines[0];
    assert_eq!(
        (m.power_group, m.position_group, m.device_flags, m.machine_flags),
        (2, -1, 0x11, 8)
    );
    let ct = &s.controls[0];
    assert_eq!((ct.power_group, ct.position_group, ct.device_flags), (-1, 1, 4));
    assert_eq!((ct.control_flags, ct.custom_name_index), (0x103, 7));
    let lf = &s.light_fixtures[0];
    assert_eq!((lf.color.r, lf.color.g, lf.color.b), (0.25, 0.5, 0.75));
    assert_eq!((lf.intensity, lf.falloff_angle, lf.cutoff_angle), (2.0, 0.4, 0.8));
    let g = &s.device_groups[0];
    assert_eq!((g.name.as_str(), g.initial_value, g.flags), ("door", 1.0, 3));
    let tv = &s.trigger_volumes[0];
    assert_eq!(tv.volume_type, TriggerVolumeType::OrientedBox);
    assert_eq!(tv.name, "zone");
    assert_eq!(
        (tv.forward, tv.up, tv.position, tv.extents),
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0])
    );
    assert_eq!(s.decals.len(), 2);
    assert_eq!(
        (s.decals[0].palette_index, s.decals[0].yaw_byte, s.decals[0].pitch_byte),
        (2, -120, 42)
    );
    assert_eq!(s.decals[0].position, [1.5, 2.5, 3.5]);
    assert_eq!(s.object_names[0].name, "alarm");
    let p = &s.starting_profiles[0];
    assert_eq!(p.name, "starter");
    assert_eq!((p.health_modifier, p.shield_modifier), (1.0, 2.0));
    assert_eq!(map.tag(p.primary_weapon.id).unwrap().class, class::WEAPON);
    assert_eq!((p.primary_rounds_loaded, p.primary_rounds_total), (12, 108));
    assert!(p.secondary_weapon.is_null());
    assert_eq!((p.frag_grenades, p.plasma_grenades), (2, 1));
    let se = &s.starting_equipment[0];
    assert_eq!(se.flags, 2);
    assert_eq!(se.game_types, GameTypes([14, 2, 0, 0]));
    assert!(se.item_collections.0[0].is_null());
    assert_eq!(
        map.tag(se.item_collections.0[1].id).unwrap().class,
        class::ITEM_COLLECTION
    );
    assert_eq!(s.scripts.0, 3);
}

#[test]
fn damaged_scenarios_are_refused_with_the_tag_and_offset() {
    // A netgame flag type outside the table.
    let mut td = TagData::new(&tag_list());
    let flag = Struct::new(0x94).i16(0x10, 9).into();
    let flags = td.block(&[flag]);
    td.set_struct(SCNR, &Struct::new(0x5B0).put(0x378, &flags).0);
    let map = MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap();
    let e = Scenario::read(&map).unwrap_err().to_string();
    assert!(e.contains("scenarios\\syn") && e.contains("NetgameFlagType"), "{e}");

    // A scenario type outside the table.
    let map = with_main(|m| {
        m.put_mut(0x03C, &7i16.to_le_bytes());
    });
    assert!(Scenario::read(&map).unwrap_err().to_string().contains("ScenarioType"));

    // A block over its maximum (201 netgame flags) or with a negative count.
    let map = with_main(|m| {
        m.put_mut(0x378, &crate::map_fixture::block_bytes(201, 0x803A_6000));
    });
    assert!(Scenario::read(&map).is_err());
    let map = with_main(|m| {
        m.put_mut(0x49C, &crate::map_fixture::block_bytes(-1, 0));
    });
    assert!(
        Scenario::read(&map)
            .unwrap_err()
            .to_string()
            .contains("negative block count")
    );

    // A scenario struct too short for its layout.
    let mut td = TagData::new(&tag_list());
    let at = td.set_struct(SCNR, &Struct::new(0x100).0);
    let _ = at;
    let map = MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap();
    assert!(Scenario::read(&map).is_err());

    // Class codes are carried by the helper constants.
    assert_eq!(c("scnr"), class::SCENARIO.as_u32());
    // `TagStruct` sizes are the spec's.
    assert_eq!(<Scenario as TagStruct>::SIZE, 0x5B0);
}
