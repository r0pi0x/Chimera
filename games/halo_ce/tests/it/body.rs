//! The Spartan body's pure pieces (`game::body`): the stored quaternion
//! convention and the axis mapping on a tiny synthetic skeleton, joints
//! from a vertex's two nodes, the hit regions by node name, the movement
//! state rule (animations.md "Bipeds"), playback's frame step
//! ("Playback"), and a body's state, blend and aim screen ticked on a
//! synthetic animation set (state to slot by the spec's rule).

use bevy::math::{Quat, Vec2, Vec3};
use blam_formats::model::{Model, Node, Transform, Vertex, rotation_axes};
use chimera_core::core::BodyRegion;
use chimera_world::map::anim::{AnimSet, Animation, Sequence, Track};
use halo_ce::game::{
    METERS_PER_UNIT,
    body::{
        BLUE, COLOURS, RED,
        anims::{self, HaloAnim, Inputs, Landing, State, advance, key, move_state},
        colour,
        hits::region_of_name,
        skeleton::{bones, globals, joints, root, usual},
    },
};

fn close(a: Vec3, b: Vec3) -> bool {
    a.distance(b) < 1e-5
}

/// A stored quaternion's usual rotation turns the axes as the model
/// spec's matrix does (columns forward, left, up of the conjugate).
#[test]
fn stored_quaternions_turn_as_the_spec_says() {
    for q in [[0.1f32, 0.2, 0.3, 0.9], [0.0, 0.0, 0.6, 0.8], [-0.5, 0.5, 0.5, 0.5]] {
        let [f, l, u] = rotation_axes(q);
        let r = usual(q);
        assert!(close(r * Vec3::X, Vec3::from(f)), "{q:?} forward");
        assert!(close(r * Vec3::Y, Vec3::from(l)), "{q:?} left");
        assert!(close(r * Vec3::Z, Vec3::from(u)), "{q:?} up");
    }
    assert_eq!(usual([0.0; 4]), Quat::IDENTITY);
}

/// The root maps Halo's forward, left and up to the engine's -Z, -X, +Y
/// and world units to metres.
#[test]
fn the_root_maps_halos_axes_and_units() {
    let r = root();
    assert!(close(r.rotation * Vec3::X, Vec3::NEG_Z));
    assert!(close(r.rotation * Vec3::Y, Vec3::NEG_X));
    assert!(close(r.rotation * Vec3::Z, Vec3::Y));
    assert_eq!(r.scale, Vec3::splat(METERS_PER_UNIT));
    assert!(close(
        r.transform_point(Vec3::new(0.0, 0.0, 0.7)),
        Vec3::new(0.0, 0.7 * METERS_PER_UNIT, 0.0)
    ));
}

fn node(name: &str, parent: Option<usize>, t: [f32; 3], q: [f32; 4]) -> Node {
    Node {
        name: name.into(),
        next_sibling: None,
        first_child: None,
        parent,
        translation: t,
        rotation: q,
        distance_from_parent: 0.0,
        inverse: Transform::IDENTITY,
    }
}

/// Two nodes: a pelvis 0.4 up, a head 0.2 above it in a frame turned 90
/// degrees about up; its bones compose as the model's bind pose.
#[test]
fn nodes_become_bones() {
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let model = Model {
        flags: 0,
        node_list_checksum: 0,
        detail_cutoffs: [0.0; 5],
        node_counts: [0; 5],
        base_map_u_scale: 1.0,
        base_map_v_scale: 1.0,
        markers: vec![],
        nodes: vec![
            node("pelvis", None, [0.0, 0.0, 0.4], [0.0, 0.0, s, s]),
            node("head", Some(0), [0.2, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]),
        ],
        regions: vec![],
        geometries: vec![],
        shaders: vec![],
    };
    let b = bones(&model);
    assert_eq!(b.len(), 2);
    assert_eq!(b[1].parent, Some(0));
    let g = globals(&b, &b.iter().map(|b| (b.rotation, b.position)).collect::<Vec<_>>());
    let want = model.bind_pose();
    for (n, w) in want.iter().enumerate() {
        assert!(
            close(g[n].1, Vec3::from(w.position)),
            "node {n}: {} vs {:?}",
            g[n].1,
            w.position
        );
        assert!(close(g[n].0 * Vec3::X, Vec3::from(w.forward)), "node {n} forward");
    }
    // The root takes a bone to where the map loader puts the same point.
    let at = halo_ce::game::maps::space::point(want[1].position);
    assert!(close(root().transform_point(g[1].1), at), "{at}");
}

#[test]
fn vertices_skin_to_their_two_nodes() {
    let v = Vertex {
        position: [0.0; 3],
        normal: [0.0, 0.0, 1.0],
        binormal: [0.0; 3],
        tangent: [0.0; 3],
        texcoord: [0.0; 2],
        node0: 6,
        node1: Some(12),
        weight0: 0.75,
    };
    assert_eq!(joints(&v), ([6, 12, 0, 0], [0.75, 0.25, 0.0, 0.0]));
    let rigid = Vertex {
        node1: None,
        weight0: 1.0,
        ..v
    };
    assert_eq!(joints(&rigid), ([6, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]));
}

#[test]
fn hit_regions_follow_the_node_names() {
    assert_eq!(region_of_name("bip01 head"), BodyRegion::Head);
    assert_eq!(region_of_name("bip01 spine1"), BodyRegion::Chest);
    assert_eq!(region_of_name("bip01 pelvis"), BodyRegion::Stomach);
    assert_eq!(region_of_name("bip01 l calf"), BodyRegion::LeftShin);
    assert_eq!(region_of_name("bip01 r foot"), BodyRegion::RightFoot);
    assert_eq!(region_of_name("bip01 r upperarm"), BodyRegion::RightUpperArm);
    assert_eq!(region_of_name("bip01 l hand"), BodyRegion::LeftHand);
}

#[test]
fn armour_colours_are_the_specs() {
    // objects.md: index 3 is 0x0201E3.
    let c = colour(3);
    assert!((c[0] - 2.0 / 255.0).abs() < 1e-6 && (c[2] - 227.0 / 255.0).abs() < 1e-6);
    assert_eq!(colour(RED), [1.0, 0.0, 0.0]);
    assert_eq!(colour(BLUE), [0.0, 0.0, 1.0]);
    assert_eq!(COLOURS.len(), 20);
}

/// animations.md "Bipeds": front or back when |forward| >= |left|, else
/// left or right; below 0.1 idle.
#[test]
fn the_throttle_picks_the_move_state() {
    assert_eq!(move_state(Vec2::new(0.05, 0.05)), State::Idle);
    assert_eq!(move_state(Vec2::new(1.0, 0.0)), State::MoveFront);
    assert_eq!(move_state(Vec2::new(-0.5, 0.5)), State::MoveBack);
    assert_eq!(move_state(Vec2::new(0.2, 0.5)), State::MoveLeft);
    assert_eq!(move_state(Vec2::new(0.0, -1.0)), State::MoveRight);
}

/// animations.md "Playback": one frame a tick; past the end the loop
/// frame (held at the last frame when past it), else frame 0.
#[test]
fn frames_advance_and_loop() {
    assert_eq!(advance(3, 10, 0), 4);
    assert_eq!(advance(9, 10, 0), 0);
    assert_eq!(advance(9, 10, 4), 4);
    assert_eq!(advance(49, 50, 58), 49);
}

/// A one-bone set: idle rotates the bone 0.1 rad about Z each frame index,
/// move-front 1 rad; an aim screen 3 columns by 3 rows.
fn set() -> AnimSet {
    let anim = |name: &str, frames: usize, f: &dyn Fn(usize) -> Quat| Animation {
        name: name.into(),
        fps: 30.0,
        frames,
        tracks: vec![Track {
            bone: 0,
            rotation: Some((0..frames).map(f).collect()),
            position: None,
        }],
        ..Default::default()
    };
    let mut s = AnimSet {
        defaults: vec![(Quat::IDENTITY, Vec3::ZERO)],
        bases: vec![vec![(Quat::IDENTITY, Vec3::ZERO)]],
        parents: vec![None],
        ..Default::default()
    };
    s.animations.push(anim("bind", 1, &|_| Quat::IDENTITY));
    s.animations
        .push(anim("idle", 4, &|f| Quat::from_rotation_z(0.1 * f as f32)));
    s.animations.push(anim("move", 4, &|_| Quat::from_rotation_z(1.0)));
    // The aim grid: frame = column + 3 row; pitch rows turn about Y by
    // (row - 1) x 0.5 rad.
    let mut aim = anim("aim", 9, &|f| Quat::from_rotation_y((f / 3) as f32 * 0.5 - 0.5));
    aim.delta = true;
    s.animations.push(aim);
    let mut seq = |name: String, a: usize| {
        s.sequences.push(Sequence {
            name: name.clone(),
            looping: true,
            anims: vec![a],
            grid: (1, 1),
            ..Default::default()
        });
        s.names.insert(name, s.sequences.len() - 1);
    };
    seq("bind".into(), 0);
    seq(key("stand", "ar", "idle"), 1);
    seq(key("stand", "ar", "move-front"), 2);
    seq(key("stand", "ar", "aim-still"), 3);
    let i = s.names[&key("stand", "ar", "aim-still")];
    s.sequences[i].grid = (3, 3);
    s.sequences[i].axes[0].keys = vec![0.5, 0.5, 1.0, 1.0];
    s.sequences[i].axes[1].keys = vec![0.5, 0.5, 1.0, 1.0];
    s
}

const LAND: Landing = Landing {
    soft_min: 1.5,
    hard_min: 5.0,
    soft_ticks: 18,
    hard_ticks: 30,
};

fn angle_z(q: Quat) -> f32 {
    let (axis, angle) = q.to_axis_angle();
    if axis.z < 0.0 { -angle } else { angle }
}

/// Idle plays from frame 0 one frame a tick; moving switches to
/// move-front at frame 0 with a 6-tick blend from the last pose (1/6 of
/// the new pose on its first tick, all of it on the sixth).
#[test]
fn a_body_picks_its_state_and_blends_into_it() {
    let s = set();
    let mut a = HaloAnim::default();
    let still = Inputs {
        grounded: true,
        ..Default::default()
    };
    let p = a.tick(&s, "ar", &still, &LAND);
    assert_eq!((a.state, a.frame), (State::Idle, 0));
    assert!(angle_z(p[0].0).abs() < 1e-5);
    a.tick(&s, "ar", &still, &LAND);
    let p = a.tick(&s, "ar", &still, &LAND);
    assert_eq!(a.frame, 2);
    assert!((angle_z(p[0].0) - 0.2).abs() < 1e-4, "{}", angle_z(p[0].0));
    let moving = Inputs {
        throttle: Vec2::new(1.0, 0.0),
        ..still
    };
    let p = a.tick(&s, "ar", &moving, &LAND);
    assert_eq!((a.state, a.frame), (State::MoveFront, 0));
    // nlerp from 0.2 rad toward 1 rad at 1/6: about 0.333 rad.
    let first = angle_z(p[0].0);
    assert!(first > 0.25 && first < 0.4, "{first}");
    for _ in 0..5 {
        a.tick(&s, "ar", &moving, &LAND);
    }
    assert!(a.blend.is_none());
    let p = a.tick(&s, "ar", &moving, &LAND);
    assert!((angle_z(p[0].0) - 1.0).abs() < 1e-4);
    // A label without tables keeps the state's last animation.
    let before = a.sequence;
    a.tick(&s, "zz", &moving, &LAND);
    assert_eq!(a.sequence, before);
}

/// More than 3 ticks in the air is airborne; a landing faster than the
/// soft minimum lands soft for its recovery ticks.
#[test]
fn airborne_and_landing() {
    let s = set();
    let mut a = HaloAnim::default();
    let air = Inputs {
        grounded: false,
        vertical: -3.0,
        ..Default::default()
    };
    for t in 1..=4 {
        a.desire(&air, &LAND);
        assert_eq!(a.air_ticks, t);
    }
    assert_eq!(a.desire(&air, &LAND), State::Airborne);
    let ground = Inputs {
        grounded: true,
        ..Default::default()
    };
    assert_eq!(a.desire(&ground, &LAND), State::LandSoft);
    for _ in 1..LAND.soft_ticks {
        assert_eq!(a.desire(&ground, &LAND), State::LandSoft);
    }
    assert_eq!(a.desire(&ground, &LAND), State::Idle);
    let _ = s;
}

/// The aim screen (animations.md "Aiming and looking screens"): pitch 0
/// is the centre row (no turn), half a frame's pitch up blends rows 1 and
/// 2, past the top clamps to the last row.
#[test]
fn the_aim_screen_turns_by_pitch() {
    let s = set();
    let screen = s.names[&key("stand", "ar", "aim-still")];
    let turn = |pitch: f32| {
        let mut pose = vec![(Quat::IDENTITY, Vec3::ZERO)];
        anims::apply_screen(&s, screen, 0.0, pitch, &mut pose);
        let (axis, angle) = pose[0].0.to_axis_angle();
        if axis.y < 0.0 { -angle } else { angle }
    };
    assert!(turn(0.0).abs() < 1e-5);
    assert!((turn(0.25) - 0.25).abs() < 1e-3, "{}", turn(0.25));
    assert!((turn(2.0) - 0.5).abs() < 1e-4);
    assert!((turn(-2.0) + 0.5).abs() < 1e-4);
}
