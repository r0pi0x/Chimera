//! The animation graph reader and sampler on synthetic data: every case
//! of specs/halo_ce/animations.md's test section "Synthetic graph",
//! "Compressed rotation", "Aiming screen", "First-person smoothing" and
//! "Selection", plus the index tables, permutations, node matching and the
//! corrupt cases, built in code with `map_fixture`.

// The spec states its angles as 1.5708 and 0.7854: the tests use them as written.
#![allow(clippy::approx_constant)]

use blam_formats::animation::{
    AnimationGraph, AnimationType, BodyPart, DamageDirection, DamageKind, DeviceSlot, FirstPersonSlot, FrameInfoType,
    IndexTable, NodePose, PlayEvent, Playback, Pose, ScreenBounds, Slot, UNIT_DAMAGE_SLOTS, UnitDamageSlot, UnitSlot,
    VehicleSlot, WeaponClassSlot, WeaponSlot, WeaponTypeSlot, decode_compressed_rotation, quat_length, quat_mul,
    quat_nlerp, quat_normalize, smooth_step, split_node_header, transition_weight,
};
use blam_formats::cache::MapFile;

use crate::map_fixture::{CacheFile, Struct, TagData, block_bytes};

fn close(a: f32, b: f32, eps: f32, what: &str) {
    assert!((a - b).abs() <= eps, "{what}: {a} vs {b}");
}

fn close_n<const N: usize>(a: [f32; N], b: [f32; N], eps: f32, what: &str) {
    for k in 0..N {
        close(a[k], b[k], eps, &format!("{what}[{k}]"));
    }
}

fn raw_quat(q: [i16; 4]) -> [f32; 4] {
    q.map(|c| f32::from(c) / 32767.0)
}

/// What one animation element holds.
#[derive(Clone)]
struct Anim {
    name: &'static str,
    kind: i16,
    frames: i16,
    fi_type: i16,
    checksum: u32,
    nodes: i16,
    loop_frame: i16,
    key: i16,
    key2: i16,
    next: i16,
    flags: u16,
    sound: i16,
    sound_frame: i16,
    cumulative: f32,
    trans: u64,
    rot: u64,
    scale: u64,
    frame_info: Vec<u8>,
    default: Vec<u8>,
    data: Vec<u8>,
}

impl Anim {
    fn new(name: &'static str) -> Anim {
        Anim {
            name,
            kind: 0,
            frames: 1,
            fi_type: 0,
            checksum: 0,
            nodes: 0,
            loop_frame: 0,
            key: 0,
            key2: 0,
            next: -1,
            flags: 0,
            sound: -1,
            sound_frame: 0,
            cumulative: 1.0,
            trans: 0,
            rot: 0,
            scale: 0,
            frame_info: Vec::new(),
            default: Vec::new(),
            data: Vec::new(),
        }
    }

    fn bytes(&self, td: &mut TagData) -> Vec<u8> {
        let fi = td.data(&self.frame_info);
        let def = td.data(&self.default);
        let data = td.data(&self.data);
        let words = |v: u64| [(v & 0xFFFF_FFFF) as u32, (v >> 32) as u32];
        let mut s = Struct::new(0xB4)
            .str(0x00, self.name)
            .i16(0x20, self.kind)
            .i16(0x22, self.frames)
            .i16(0x24, 0)
            .i16(0x26, self.fi_type)
            .u32(0x28, self.checksum)
            .i16(0x2C, self.nodes)
            .i16(0x2E, self.loop_frame)
            .i16(0x34, self.key)
            .i16(0x36, self.key2)
            .i16(0x38, self.next)
            .u16(0x3A, self.flags)
            .i16(0x3C, self.sound)
            .i16(0x3E, self.sound_frame)
            .i16(0x42, 0)
            .f32(0x44, self.cumulative)
            .put(0x48, &fi)
            .put(0x88, &0i32.to_le_bytes())
            .put(0x8C, &def)
            .put(0xA0, &data);
        for (off, v) in [(0x5C, self.trans), (0x6C, self.rot), (0x7C, self.scale)] {
            let w = words(v);
            s = s.u32(off, w[0]).u32(off + 4, w[1]);
        }
        s.into()
    }
}

/// An index table block of `entries` (NONE = -1).
fn table(td: &mut TagData, entries: &[i16]) -> [u8; 12] {
    let elems: Vec<Vec<u8>> = entries.iter().map(|e| e.to_le_bytes().to_vec()).collect();
    td.block(&elems)
}

fn graph_node(name: &str, sibling: i16, child: i16, parent: i16) -> Vec<u8> {
    Struct::new(0x40)
        .str(0x00, name)
        .i16(0x20, sibling)
        .i16(0x22, child)
        .i16(0x24, parent)
        .into()
}

fn bounds_bytes(b: [f32; 2], counts: [i16; 4], pitch: [f32; 2]) -> Vec<u8> {
    Struct::new(0x18)
        .f32(0x00, b[0])
        .f32(0x04, b[1])
        .i16(0x08, counts[0])
        .i16(0x0A, counts[1])
        .f32(0x0C, pitch[0])
        .f32(0x10, pitch[1])
        .i16(0x14, counts[2])
        .i16(0x16, counts[3])
        .into()
}

/// A map whose tag 1 is a graph with `nodes`, `animations` and the extra
/// block fields `extra` writes into the main struct.
fn graph_map(nodes: &[Vec<u8>], anims: &[Anim], extra: impl FnOnce(&mut TagData, Struct) -> Struct) -> MapFile {
    let mut td = TagData::new(&[("scnr", "levels\\tiny"), ("antr", "a\\g")]);
    let anim_bytes: Vec<Vec<u8>> = anims.iter().map(|a| a.bytes(&mut td)).collect();
    let anims_block = td.block(&anim_bytes);
    let nodes_block = td.block(nodes);
    let main = Struct::new(0x80).put(0x68, &nodes_block).put(0x74, &anims_block);
    let main = extra(&mut td, main);
    td.set_struct(1, &main.0);
    MapFile::from_image(CacheFile::new("tiny", 1, td.finish()).image()).unwrap()
}

fn read(map: &MapFile) -> std::io::Result<AnimationGraph> {
    AnimationGraph::read(
        map,
        map.find(blam_formats::cache::class::MODEL_ANIMATIONS, "a\\g").unwrap(),
    )
}

fn f32s(vs: &[f32]) -> Vec<u8> {
    vs.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn i16s(vs: &[i16]) -> Vec<u8> {
    vs.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// The spec's animation `test`: two nodes, three frames.
fn test_animation() -> Anim {
    let mut default = i16s(&[0, 0, 0, 32767]);
    default.extend(f32s(&[0.0, 0.0, 0.5, 1.0, 1.0]));
    assert_eq!(default.len(), 28);
    let frames: [([i16; 4], [f32; 3]); 3] = [
        ([0, 0, 0, 32767], [0.1, 0.0, 0.0]),
        ([0, 0, 23170, 23170], [0.2, 0.0, 0.0]),
        ([0, 0, 32767, 0], [0.3, 0.0, 0.0]),
    ];
    let mut data = Vec::new();
    for (q, t) in frames {
        data.extend(i16s(&q));
        data.extend(f32s(&t));
    }
    assert_eq!(data.len(), 60);
    Anim {
        frames: 3,
        nodes: 2,
        key: 1,
        trans: 2,
        rot: 2,
        default,
        data,
        ..Anim::new("test")
    }
}

fn test_graph() -> AnimationGraph {
    let nodes = [graph_node("root", -1, 1, 0), graph_node("arm", -1, -1, 0)];
    read(&graph_map(&nodes, &[test_animation()], |_, s| s)).unwrap()
}

#[test]
fn minimal_graph_reads() {
    let g = test_graph();
    assert_eq!(g.nodes.len(), 2);
    assert_eq!(g.nodes[0].name, "root");
    assert_eq!(g.nodes[0].first_child, Some(1));
    // The stored parent of node 0 is 0; `parents` corrects it.
    assert_eq!(g.nodes[0].parent, Some(0));
    assert_eq!(g.parents(), vec![None, Some(0)]);
    let a = &g.animations[0];
    assert_eq!(a.name, "test");
    assert_eq!(a.kind, AnimationType::Base);
    assert_eq!(a.frame_count, 3);
    assert_eq!(a.frame_info_type, FrameInfoType::None);
    assert_eq!(a.key_frame, 1);
    assert_eq!(a.next_animation, None);
    assert_eq!(a.sound, None);
    assert_eq!(a.cumulative_weight, 1.0);
    assert_eq!((a.rotation_flags, a.translation_flags, a.scale_flags), (2, 2, 0));
    assert_eq!(a.computed_frame_size(), 20);
    assert_eq!(a.computed_default_size(), 28);
    assert_eq!(a.default_data.len(), 28);
    assert_eq!(a.frame_data.len(), 60);
    assert!(a.has_rotation(1) && !a.has_rotation(0) && !a.has_scale(1));
}

#[test]
fn minimal_graph_samples() {
    let g = test_graph();
    let a = &g.animations[0];
    let expect = [
        ([0.0, 0.0, 0.0, 1.0], [0.1, 0.0, 0.0], [1.0, 0.0, 0.0], [0.1, 0.0, 0.5]),
        (
            [0.0, 0.0, 0.707_114, 0.707_114],
            [0.2, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.2, 0.0, 0.5],
        ),
        ([0.0, 0.0, 1.0, 0.0], [0.3, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.3, 0.0, 0.5]),
    ];
    for (f, (q, t, fwd, origin)) in expect.iter().enumerate() {
        let p = a.sample(f).unwrap();
        assert_eq!(p.len(), 2);
        // Node 0 from the default data in every frame.
        assert_eq!(p.nodes[0].rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(p.nodes[0].translation, [0.0, 0.0, 0.5]);
        assert_eq!(p.nodes[0].scale, 1.0);
        close_n(p.nodes[1].rotation, *q, 1e-5, "node 1 rotation");
        close_n(p.nodes[1].translation, *t, 1e-6, "node 1 translation");
        assert_eq!(p.nodes[1].scale, 1.0);
        // The forward axis in node 0's frame and the origin in the object's.
        let local = p.nodes[1].local();
        close_n(local.forward, *fwd, 1e-5, "forward axis");
        let world = p.world_transforms(&g.parents()).unwrap();
        close_n(world[1].position, *origin, 1e-5, "origin");
        close_n(world[0].position, [0.0, 0.0, 0.5], 1e-6, "root origin");
    }
    // A frame past the animation is an error naming it.
    let e = a.sample(3).unwrap_err().to_string();
    assert!(e.contains("test"), "{e}");
}

#[test]
fn playback_events() {
    let g = test_graph();
    let a = &g.animations[0];
    let mut p = Playback::start();
    assert_eq!(p.advance(a), PlayEvent::KeyFrame);
    assert_eq!(p.frame, 1);
    assert_eq!(p.advance(a), PlayEvent::EndsNext);
    assert_eq!(p.frame, 2);
    assert_eq!(p.advance(a), PlayEvent::Restarted);
    assert_eq!(p.frame, 0);

    let mut looping = a.clone();
    looping.loop_frame = 1;
    let mut p = Playback::start();
    p.advance(&looping);
    p.advance(&looping);
    assert_eq!(p.advance(&looping), PlayEvent::Looped);
    assert_eq!(p.frame, 1);

    // A loop frame at or past the end holds the last frame for ever.
    looping.loop_frame = 58;
    let mut p = Playback { frame: 2 };
    assert_eq!(p.advance(&looping), PlayEvent::Looped);
    assert_eq!(p.frame, 2);

    // A key frame at 0 never fires; one on the last frame of a play-once
    // animation reports "ends next" instead.
    let mut at_end = a.clone();
    at_end.key_frame = 2;
    let mut p = Playback::start();
    p.advance(&at_end);
    assert_eq!(p.advance(&at_end), PlayEvent::EndsNext);

    // Sound: starts when the frame equals the sound frame, before advancing.
    let mut loud = a.clone();
    loud.sound = Some(0);
    loud.sound_frame = 0;
    let mut p = Playback::start();
    assert!(p.sound_starts(&loud));
    p.advance(&loud);
    assert!(!p.sound_starts(&loud));
}

#[test]
fn overlays_and_transition() {
    let g = test_graph();
    let mut a = g.animations[0].clone();
    a.kind = AnimationType::Overlay;
    let mut base = Pose::identity(2);
    base.nodes[1].translation = [1.0, 2.0, 3.0];
    let mut p = base.clone();
    assert!(p.apply_overlay(&a, 1).unwrap());
    close_n(
        p.nodes[1].rotation,
        [0.0, 0.0, 0.707_114, 0.707_114],
        1e-6,
        "overlay rotation",
    );
    close_n(p.nodes[1].translation, [1.2, 2.0, 3.0], 1e-6, "overlay translation");
    assert_eq!(p.nodes[0], base.nodes[0], "node 0 unchanged");

    // Two quarter turns.
    let mut p = base.clone();
    p.nodes[1].rotation = [0.0, 0.0, 0.707_107, 0.707_107];
    p.apply_overlay(&a, 1).unwrap();
    close_n(p.nodes[1].rotation, [0.0, 0.0, 1.0, 0.0], 2e-5, "two quarter turns");

    // The same animation as a replacement sets the parts it has.
    a.kind = AnimationType::Replacement;
    let mut p = base.clone();
    p.apply(&a, 1).unwrap();
    close_n(p.nodes[1].translation, [0.2, 0.0, 0.0], 1e-6, "replacement translation");
    assert_eq!(p.nodes[0], base.nodes[0]);

    // An overlay with more nodes than the pose is not applied.
    let mut small = Pose::identity(1);
    assert!(!small.apply_overlay(&a, 1).unwrap());

    // Transition: A to B over 3 ticks.
    let from = Pose::identity(2);
    let mut to = Pose::identity(2);
    to.nodes[1].rotation = [0.0, 0.0, 0.707_107, 0.707_107];
    to.nodes[1].translation = [0.3, 0.0, 0.0];
    let mut p = to.clone();
    p.blend_from(&from, transition_weight(0, 3));
    close_n(
        p.nodes[1].rotation,
        [0.0, 0.0, 0.252_725, 0.967_538],
        1e-5,
        "blend rotation",
    );
    close_n(p.nodes[1].translation, [0.1, 0.0, 0.0], 1e-6, "blend translation");
    let mut p = to.clone();
    p.blend_from(&from, transition_weight(2, 3));
    assert_eq!(p.nodes[1].translation, [0.3, 0.0, 0.0]);
    close_n(
        p.nodes[1].rotation,
        quat_normalize(to.nodes[1].rotation),
        1e-6,
        "blend end",
    );
}

#[test]
fn continuous_samples() {
    let g = test_graph();
    let a = &g.animations[0];
    let p = a.sample_continuous(0.5).unwrap();
    close_n(
        p.nodes[1].rotation,
        [0.0, 0.0, 0.382_685, 0.923_879],
        1e-5,
        "half rotation",
    );
    close_n(p.nodes[1].translation, [0.15, 0.0, 0.0], 1e-6, "half translation");
    // Past the end: the last frame, fraction 1, the next wrapping to 0.
    let p = a.sample_continuous(3.0).unwrap();
    close_n(p.nodes[1].translation, [0.1, 0.0, 0.0], 1e-6, "wrapped");
    // The last frame's fraction blends toward frame 0.
    let p = a.sample_continuous(2.5).unwrap();
    close_n(p.nodes[1].translation, [0.2, 0.0, 0.0], 1e-6, "wrap blend");
    // A scaled overlay of weight 0.5 is not renormalised; the continuous
    // one is.
    let mut o = a.clone();
    o.kind = AnimationType::Overlay;
    let mut p = Pose::identity(2);
    p.apply_scaled_overlay(&o, 1, 0.5).unwrap();
    let r = p.nodes[1].rotation;
    close_n(r, [0.0, 0.0, 0.353_557, 0.5 + 0.353_557], 1e-5, "scaled overlay");
    assert!(quat_length(r) < 0.999);
    close_n(p.nodes[1].translation, [0.1, 0.0, 0.0], 1e-6, "scaled translation");
    let mut p = Pose::identity(2);
    p.apply_continuous_scaled_overlay(&o, 1.0, 0.5).unwrap();
    close(
        quat_length(p.nodes[1].rotation),
        1.0,
        1e-5,
        "continuous scaled is normalised",
    );
}

#[test]
fn frame_info_and_flags() {
    let mut a = test_animation();
    a.fi_type = 3;
    a.frame_info = f32s(&[
        1.0, 2.0, 3.0, 4.0, //
        5.0, 6.0, 7.0, 8.0, //
        9.0, 10.0, 11.0, 12.0,
    ]);
    let nodes = [graph_node("root", -1, 1, 0), graph_node("arm", -1, -1, 0)];
    let g = read(&graph_map(&nodes, &[a.clone()], |_, s| s)).unwrap();
    let m = g.animations[0].root_motion(1).unwrap();
    assert_eq!((m.dx, m.dy, m.dz, m.dyaw), (5.0, 6.0, 7.0, 8.0));
    assert_eq!(g.animations[0].root_motion(3), None);

    // dx dy dyaw has no dz.
    a.fi_type = 2;
    a.frame_info = f32s(&[1.0, 2.0, 0.25, 1.0, 2.0, 0.5, 1.0, 2.0, 0.75]);
    let g = read(&graph_map(&nodes, &[a.clone()], |_, s| s)).unwrap();
    let m = g.animations[0].root_motion(2).unwrap();
    assert_eq!((m.dx, m.dy, m.dz, m.dyaw), (1.0, 2.0, 0.0, 0.75));

    // A size that does not match the type refuses the graph.
    a.frame_info.pop();
    let e = read(&graph_map(&nodes, &[a.clone()], |_, s| s))
        .unwrap_err()
        .to_string();
    assert!(e.contains("frame info"), "{e}");

    // Flag bits above the node count are ignored (and so are the sizes).
    let mut b = test_animation();
    b.rot |= 1 << 40;
    b.trans |= 1 << 33;
    let g = read(&graph_map(&nodes, &[b], |_, s| s)).unwrap();
    let a = &g.animations[0];
    assert!(!a.has_rotation(40) && !a.has_translation(33));
    assert_eq!(a.computed_frame_size(), 20);
    a.sample(2).unwrap();
}

#[test]
fn corrupt_graphs() {
    let nodes = [graph_node("root", -1, 1, 0), graph_node("arm", -1, -1, 0)];
    // An animation type out of range.
    let mut a = test_animation();
    a.kind = 3;
    assert!(read(&graph_map(&nodes, &[a], |_, s| s)).is_err());
    // More nodes than 64.
    let mut a = test_animation();
    a.nodes = 65;
    let e = read(&graph_map(&nodes, &[a], |_, s| s)).unwrap_err().to_string();
    assert!(e.contains("65"), "{e}");
    // Short frame data: reads, but sampling names the animation.
    let mut a = test_animation();
    a.data.truncate(50);
    let g = read(&graph_map(&nodes, &[a], |_, s| s)).unwrap();
    assert!(g.animations[0].sample(0).is_ok());
    let e = g.animations[0].sample(2).unwrap_err().to_string();
    assert!(e.contains("`test`") && e.contains("frame data"), "{e}");
    // Short default data.
    let mut a = test_animation();
    a.default.truncate(20);
    let g = read(&graph_map(&nodes, &[a], |_, s| s)).unwrap();
    assert!(g.animations[0].sample(0).unwrap_err().to_string().contains("default"));
    // A block count past the editor's limit.
    let e = read(&graph_map(&nodes, &[test_animation()], |_, s| {
        s.put(0x68, &block_bytes(65, 0x803A_6000))
    }))
    .unwrap_err();
    assert!(e.to_string().contains("maximum"), "{e}");
}

#[test]
fn compressed_rotation_examples() {
    let q = decode_compressed_rotation([0x80, 0x5A, 0x00, 0x00, 0xA8, 0x05]);
    close_n(q, [0.707_205, 0.0, 0.0, 0.707_205], 1e-6, "first example");
    close_n(quat_normalize(q), [0.707_107, 0.0, 0.0, 0.707_107], 1e-6, "normalised");
    // A negative field: 0xC00 widens to 0xC00C = -16372.
    let x: u64 = 0xC00 << 36;
    let w = [(x >> 32) as u16, (x >> 16) as u16, x as u16];
    let b = [w[0].to_le_bytes(), w[1].to_le_bytes(), w[2].to_le_bytes()].concat();
    let q = decode_compressed_rotation(b.try_into().unwrap());
    close(q[0], -0.499_649, 1e-6, "negative field");
    // The disc example: D3 44 3F 38 64 A4.
    let q = decode_compressed_rotation([0xD3, 0x44, 0x3F, 0x38, 0x64, 0xA4]);
    close_n(q, [0.537_736, 0.402_448, 0.497_177, 0.548_967], 1e-6, "disc example");
    close_n(
        quat_normalize(q),
        [0.537_824, 0.402_513, 0.497_258, 0.549_056],
        1e-6,
        "disc normalised",
    );
    assert_eq!(split_node_header(0x0000_A002), (10, 2));
    assert_eq!(split_node_header(0x0000_0033), (0, 51));
}

fn encode_rotation(i: u64, j: u64, k: u64, w: u64) -> [u8; 6] {
    let x = (i << 36) | (j << 24) | (k << 12) | w;
    let w = [(x >> 32) as u16, (x >> 16) as u16, x as u16];
    [w[0].to_le_bytes(), w[1].to_le_bytes(), w[2].to_le_bytes()]
        .concat()
        .try_into()
        .unwrap()
}

/// A compressed stream of one node with rotation keys at frames 3 and 9
/// (N = 10), one translation key pair at frames 4 and 8, nothing else.
fn compressed_stream(d: [u8; 6], k0: [u8; 6], k1: [u8; 6], t: [[f32; 3]; 3]) -> Vec<u8> {
    let mut s = vec![0u8; 0x2C];
    let put = |s: &mut Vec<u8>, at: usize, v: i32| s[at..at + 4].copy_from_slice(&v.to_le_bytes());
    // Rotation: headers (1), times (2), defaults (1), keys (2).
    s.extend(0x0000_0002u32.to_le_bytes());
    let rot_times = s.len();
    s.extend([3u16, 9].iter().flat_map(|t| t.to_le_bytes()));
    let rot_defaults = s.len();
    s.extend(d);
    let rot_keys = s.len();
    s.extend(k0);
    s.extend(k1);
    // Translation: headers (1), times (2), defaults (1), keys (2).
    let trans_headers = s.len();
    s.extend(0x0000_0002u32.to_le_bytes());
    let trans_times = s.len();
    s.extend([4u16, 8].iter().flat_map(|t| t.to_le_bytes()));
    let trans_defaults = s.len();
    s.extend(f32s(&t[0]));
    let trans_keys = s.len();
    s.extend(f32s(&t[1]));
    s.extend(f32s(&t[2]));
    let end = s.len();
    for (at, v) in [
        (0x00, rot_times),
        (0x04, rot_defaults),
        (0x08, rot_keys),
        (0x0C, trans_headers),
        (0x10, trans_times),
        (0x14, trans_defaults),
        (0x18, trans_keys),
        (0x1C, end),
        (0x20, end),
        (0x24, end),
        (0x28, end),
    ] {
        put(&mut s, at, v as i32);
    }
    s
}

#[test]
fn compressed_sampling() {
    let d = encode_rotation(0, 0, 0, 0x7FF);
    let k0 = encode_rotation(0, 0, 0x5A8, 0x5A8);
    let k1 = encode_rotation(0, 0, 0x7FF, 0);
    let t = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
    let a = Anim {
        frames: 10,
        nodes: 1,
        flags: 1,
        rot: 1,
        trans: 1,
        data: compressed_stream(d, k0, k1, t),
        ..Anim::new("packed")
    };
    let nodes = [graph_node("root", -1, -1, 0)];
    let g = read(&graph_map(&nodes, &[a], |_, s| s)).unwrap();
    let a = &g.animations[0];
    assert!(a.is_compressed());
    let (rd, r0, r1) = (
        decode_compressed_rotation(d),
        decode_compressed_rotation(k0),
        decode_compressed_rotation(k1),
    );
    let rot = |f: usize| a.sample(f).unwrap().nodes[0].rotation;
    close_n(rot(0), quat_normalize(rd), 1e-6, "frame 0");
    close_n(rot(2), quat_nlerp(rd, r0, 2.0 / 3.0), 1e-6, "frame 2");
    close_n(rot(3), quat_normalize(r0), 1e-6, "frame 3");
    close_n(rot(6), quat_nlerp(r0, r1, 0.5), 1e-6, "frame 6");
    close_n(rot(9), quat_normalize(r1), 1e-6, "frame 9");
    // Every sample is a unit quaternion.
    for f in 0..10 {
        close(quat_length(rot(f)), 1.0, 1e-5, "unit");
    }
    // Translation keys at 4 and 8: before the first, from the default.
    let tr = |f: usize| a.sample(f).unwrap().nodes[0].translation;
    close_n(tr(0), [0.0; 3], 1e-6, "t0");
    close_n(tr(2), [0.5, 0.0, 0.0], 1e-6, "t2");
    close_n(tr(4), [1.0, 0.0, 0.0], 1e-6, "t4");
    close_n(tr(6), [1.5, 0.0, 0.0], 1e-6, "t6");
    // At the last key, interpolating toward the default for one frame.
    close_n(tr(8), [2.0, 0.0, 0.0], 1e-6, "t8");
    close_n(tr(9), [2.0, 0.0, 0.0], 1e-6, "t9 holds");
    assert_eq!(a.sample(0).unwrap().nodes[0].scale, 1.0);
    // The continuous sample: rotation at the integer frame, translation at x.
    let c = a.sample_continuous(5.0).unwrap();
    close_n(c.nodes[0].translation, [1.25, 0.0, 0.0], 1e-6, "continuous translation");
    let c = a.sample_continuous(6.5).unwrap();
    close_n(
        c.nodes[0].rotation,
        rot(6),
        1e-6,
        "continuous rotation ignores the fraction",
    );
    // A stream cut short: that node gets identity, zero, one.
    let mut cut = a.clone();
    cut.frame_data.truncate(0x40);
    let p = cut.sample(5).unwrap();
    assert_eq!(p.nodes[0], NodePose::IDENTITY);
    // A header offset past the stream: that node is identity too.
    let mut bad = a.clone();
    bad.frame_data[0x00..0x04].copy_from_slice(&0x7FFF_i32.to_le_bytes());
    assert_eq!(bad.sample(5).unwrap().nodes[0].rotation, [0.0, 0.0, 0.0, 1.0]);
    // A stream shorter than its header is an error.
    let mut tiny = a.clone();
    tiny.frame_data.truncate(0x10);
    assert!(tiny.sample(0).unwrap_err().to_string().contains("header"));
    // So is a compressed data offset past the stream.
    let mut off = a.clone();
    off.compressed_data_offset = 1 << 20;
    assert!(off.sample(0).is_err());
}

#[test]
fn aiming_screen_cells() {
    let b = ScreenBounds {
        right_yaw_per_frame: 1.5708,
        left_yaw_per_frame: 1.5708,
        right_frame_count: 1,
        left_frame_count: 1,
        down_pitch_per_frame: 1.5708,
        up_pitch_per_frame: 1.5708,
        down_frame_count: 1,
        up_frame_count: 1,
    };
    assert_eq!((b.columns(), b.rows()), (3, 3));
    let c = b.locate(0.5, -0.3).unwrap();
    assert_eq!((c.column, c.row), (1, 0));
    close(c.u, 0.318_310, 1e-5, "u");
    close(c.p, 0.809_014, 1e-5, "p");
    assert_eq!(c.frames, [1, 2, 4, 5]);
    let w = c.weights();
    for (got, want) in w.iter().zip([0.130_193, 0.060_793, 0.551_497, 0.257_517]) {
        close(*got, want, 1e-5, "weight");
    }
    close(w.iter().sum::<f32>(), 1.0, 1e-6, "weights sum");
    let c = b.locate(2.0, 0.0).unwrap();
    assert_eq!(c.column, 1);
    assert_eq!(c.u, 1.0);
    let c = b.locate(-2.0, 0.0).unwrap();
    assert_eq!(c.column, 0);
    assert_eq!(c.u, 0.0);
    let c = b.locate(0.0, 1.0).unwrap();
    assert_eq!(c.row, 1);
    close(c.p, 0.636_620, 1e-5, "pitch fraction");
    // The centre cell.
    let c = b.locate(0.0, 0.0).unwrap();
    assert_eq!((c.column, c.row, c.u, c.p), (1, 1, 0.0, 0.0));
    assert_eq!(c.frames[0], 4);
    // Counts that leave no grid: outside it.
    let none = ScreenBounds {
        left_frame_count: -1,
        ..ScreenBounds::default()
    };
    assert_eq!(none.columns(), 0);
    assert_eq!(none.locate(0.1, 0.1), None);
    // Zero counts: a single cell.
    let one = ScreenBounds::default().locate(0.1, 0.1).unwrap();
    assert_eq!((one.column, one.row, one.frames), (0, 0, [0, 0, 0, 0]));
}

/// A 3 x 3 overlay grid of one node: frame `k` turns it by `k` quarter
/// turns' worth of an arbitrary angle about the up axis.
#[test]
fn aiming_screen_applies() {
    let b = ScreenBounds {
        right_yaw_per_frame: 1.5708,
        left_yaw_per_frame: 1.5708,
        right_frame_count: 1,
        left_frame_count: 1,
        down_pitch_per_frame: 1.5708,
        up_pitch_per_frame: 1.5708,
        down_frame_count: 1,
        up_frame_count: 1,
    };
    // One node, rotation and translation per frame; column 0 turns -90
    // degrees, column 2 +90 (about the up axis), the middle none; rows
    // shift the translation in z by -1, 0, +1.
    let s = 0.707_106_8;
    let mut data = Vec::new();
    for row in 0..3 {
        for col in 0..3 {
            let q = match col {
                0 => [0.0, 0.0, -s, s],
                1 => [0.0, 0.0, 0.0, 1.0],
                _ => [0.0, 0.0, s, s],
            };
            let q: Vec<i16> = q.iter().map(|c| (c * 32767.0) as i16).collect();
            data.extend(i16s(&q));
            data.extend(f32s(&[0.0, 0.0, row as f32 - 1.0]));
        }
    }
    let a = Anim {
        kind: 1,
        frames: 9,
        nodes: 1,
        rot: 1,
        trans: 1,
        default: f32s(&[1.0]),
        data,
        ..Anim::new("aim")
    };
    let nodes = [graph_node("root", -1, -1, 0)];
    let g = read(&graph_map(&nodes, &[a], |_, s| s)).unwrap();
    let a = &g.animations[0];
    // Centre: unchanged.
    let mut p = Pose::identity(1);
    assert!(p.apply_screen(a, &b, 0.0, 0.0).unwrap());
    close_n(p.nodes[0].rotation, [0.0, 0.0, 0.0, 1.0], 1e-4, "centre");
    close_n(p.nodes[0].translation, [0.0; 3], 1e-4, "centre translation");
    // A full column step left reaches the +90 degree frame.
    let mut p = Pose::identity(1);
    p.apply_screen(a, &b, 1.5708, 0.0).unwrap();
    close_n(p.nodes[0].rotation, [0.0, 0.0, s, s], 1e-3, "left");
    // Pitch half a row up shifts z by half.
    let mut p = Pose::identity(1);
    p.apply_screen(a, &b, 0.0, 0.7854).unwrap();
    close(p.nodes[0].translation[2], 0.5, 1e-3, "half row");
    // Fewer frames than the grid: skipped.
    let mut short = a.clone();
    short.frame_count = 8;
    assert!(!Pose::identity(1).apply_screen(&short, &b, 0.0, 0.0).unwrap());
    // A base animation is not a screen.
    let mut base = a.clone();
    base.kind = AnimationType::Base;
    assert!(!Pose::identity(1).apply_screen(&base, &b, 0.0, 0.0).unwrap());
}

#[test]
fn quaternion_helpers() {
    // The stored product with the overlay on the left: in the game's
    // convention two quarter turns about the same axis give a half turn.
    let q = [0.0, 0.0, 0.707_107, 0.707_107];
    close_n(quat_mul(q, q), [0.0, 0.0, 1.0, 0.0], 1e-5, "half turn");
    // nlerp takes the short way: -q and q give q.
    let n = quat_nlerp([0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, -1.0], 0.5);
    assert_eq!(n, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(quat_normalize([0.0; 4]), [0.0, 0.0, 0.0, 1.0]);
    // Raw decoding of the stored components.
    close_n(
        raw_quat([0, 0, 23170, 23170]),
        [0.0, 0.0, 0.707_114, 0.707_114],
        1e-6,
        "raw",
    );
}

#[test]
fn smoothing_steps() {
    // Push back after one shot: velocity 0.05, position 0.
    let want = [0.035, 0.06, 0.075, 0.08, 0.075, 0.06, 0.035, 0.005_314, 0.0, 0.0];
    let (mut p, mut v) = (0.0, 0.05);
    for (tick, w) in want.iter().enumerate() {
        (p, v) = smooth_step(p, v, 0.0, 0.01, 0.2, 0.0, 1.0);
        close(p, *w, 1e-5, &format!("push back tick {}", tick + 1));
    }
    // The velocity is not clamped with the position: 0 from tick 11.
    (p, v) = smooth_step(p, v, 0.0, 0.01, 0.2, 0.0, 1.0);
    assert_eq!((p, v), (0.0, 0.0), "tick 11");
    // Position x with throttle 1 from rest.
    let want = [0.12, 0.32, 0.6, 0.859_473, 0.992_456, 1.0, 0.972_982, 1.0];
    let (mut p, mut v) = (0.0, 0.0);
    for (tick, w) in want.iter().enumerate() {
        (p, v) = smooth_step(p, v, 1.0, 0.08, 0.5, -1.0, 1.0);
        close(p, *w, 1e-5, &format!("position tick {}", tick + 1));
    }
    assert_eq!(v, 0.0, "velocity 0 at tick 8");
}

fn unit_graph() -> AnimationGraph {
    // Four animations to point the tables at.
    let anims: Vec<Anim> = ["a0", "a1", "a2", "a3"].iter().map(|n| Anim::new(n)).collect();
    let map = graph_map(&[], &anims, |td, main| {
        let type_ = |td: &mut TagData, label: &str, slots: &[i16]| -> Vec<u8> {
            let t = table(td, slots);
            Struct::new(0x3C).str(0x00, label).put(0x30, &t).into()
        };
        let class = |td: &mut TagData, label: &str, slots: &[i16], types: Vec<Vec<u8>>| -> Vec<u8> {
            let t = table(td, slots);
            let ty = td.block(&types);
            Struct::new(0xBC)
                .str(0x00, label)
                .put(0x60, &bounds_bytes([1.5708, 1.5708], [1, 1, 1, 1], [1.5708, 1.5708]))
                .put(0x98, &t)
                .put(0xB0, &ty)
                .into()
        };
        let seat = |td: &mut TagData, label: &str, slots: &[i16], classes: Vec<Vec<u8>>| -> Vec<u8> {
            let t = table(td, slots);
            let c = td.block(&classes);
            Struct::new(0x64)
                .str(0x00, label)
                .put(0x20, &bounds_bytes([1.2217, 1.2217], [1, 1, 1, 1], [0.7854, 0.7854]))
                .put(0x40, &t)
                .put(0x58, &c)
                .into()
        };
        // stand: rifle (ar, ne), pistol (hp, ne); crouch: unarmed (empty).
        let ar = type_(td, "ar", &[0, 1]);
        let ne1 = type_(td, "ne", &[2]);
        let rifle = class(td, "rifle", &[0, -1, 1], vec![ar, ne1]);
        let hp = type_(td, "hp", &[3]);
        let ne2 = type_(td, "ne", &[1, 1, 1]);
        let pistol = class(td, "pistol", &[3], vec![hp, ne2]);
        let stand = seat(td, "stand", &[-1; 12], vec![rifle, pistol]);
        let un = type_(td, "", &[]);
        let unarmed = class(td, "unarmed", &[], vec![un]);
        // A seat with a look slot and flying slots, a short table.
        let mut slots = vec![-1i16; 30];
        slots[UnitSlot::Look as usize] = 2;
        slots[UnitSlot::Hovering as usize] = 3;
        let crouch = seat(td, "crouch", &slots, vec![unarmed]);
        // A second `stand` seat after the first, with a class `pistol`
        // (types `hp`): the later seat wins for `hp`.
        let hp2 = type_(td, "HP", &[2]);
        let pistol2 = class(td, "pistol", &[], vec![hp2]);
        let stand2 = seat(td, "STAND", &[], vec![pistol2]);
        let units = td.block(&[stand, crouch, stand2]);
        // Weapons, first person (28 slots), vehicles, devices, damage.
        let w = table(td, &[0, 1, 2, -1, -1, -1, -1, -1, -1, 3, 0]);
        let wblock = td.block(&[Struct::new(0x1C).put(0x10, &w).into()]);
        let mut fp = vec![-1i16; 17];
        fp[FirstPersonSlot::Ready as usize] = 1;
        fp[FirstPersonSlot::Melee as usize] = 2;
        let fpt = table(td, &fp);
        let fpblock = td.block(&[Struct::new(0x1C).put(0x10, &fpt).into()]);
        let vt = table(td, &[0, 99]);
        let vblock = td.block(&[Struct::new(0x74).put(0x5C, &vt).into()]);
        let dt = table(td, &[1]);
        let dblock = td.block(&[Struct::new(0x60).put(0x54, &dt).into()]);
        let mut dmg = vec![-1i16; UNIT_DAMAGE_SLOTS];
        dmg[((2 * 4 + 3) * 11) + 2] = 3; // s-kill, back, head
        let dm = table(td, &dmg);
        main.put(0x0C, &units)
            .put(0x18, &wblock)
            .put(0x24, &vblock)
            .put(0x30, &dblock)
            .put(0x3C, &dm)
            .put(0x48, &fpblock)
    });
    read(&map).unwrap()
}

#[test]
fn unit_selection_and_fallback() {
    let g = unit_graph();
    assert_eq!(g.units.len(), 3);
    assert_eq!(g.units[0].weapon_classes[0].weapon_types[1].label, "ne");
    // `ne` is under both rifle and pistol: the pistol class, later, wins.
    let s = g.select_unit("stand", "ne").unwrap();
    assert_eq!((s.seat, s.weapon_class, s.weapon_type), (0, 1, 1));
    assert_eq!(g.weapon_class(s).unwrap().label, "pistol");
    let s = g.select_unit("stand", "ar").unwrap();
    assert_eq!((s.seat, s.weapon_class, s.weapon_type), (0, 0, 0));
    // No match: nothing; the caller keeps its previous selection.
    assert_eq!(g.select_unit("stand", "xx"), None);
    let prev = g.select_unit("stand", "ar");
    assert_eq!(g.select_unit("stand", "xx").or(prev), prev);
    assert_eq!(g.select_unit("flee", "ar"), None);
    // Case-insensitive on both labels, and a later seat of the same label
    // wins over an earlier one.
    let s = g.select_unit("Stand", "hp").unwrap();
    assert_eq!((s.seat, s.weapon_class, s.weapon_type), (2, 0, 0));
    // Unarmed selects the type with an empty label.
    let s = g.select_unit("crouch", "unarmed").unwrap();
    assert_eq!((s.seat, s.weapon_class, s.weapon_type), (1, 0, 0));
    assert_eq!(g.weapon_type(s).unwrap().label, "");
    assert_eq!(g.weapon_class(s).unwrap().label, "unarmed");
    // Unarmed finds no empty-label type in `stand`.
    assert_eq!(g.select_unit("stand", "unarmed"), None);
}

#[test]
fn index_tables_by_slot() {
    let g = unit_graph();
    let s = g.select_unit("stand", "ar").unwrap();
    let name = |a: Option<&blam_formats::animation::Animation>| a.map(|a| a.name.clone());
    // The weapon class table is short: slots past it are none; NONE is none.
    assert_eq!(name(g.class_animation(s, WeaponClassSlot::Idle)), Some("a0".into()));
    assert_eq!(name(g.class_animation(s, WeaponClassSlot::Gesture)), None);
    assert_eq!(name(g.class_animation(s, WeaponClassSlot::TurnLeft)), Some("a1".into()));
    assert_eq!(name(g.class_animation(s, WeaponClassSlot::MoveFront)), None);
    assert_eq!(name(g.class_animation(s, WeaponClassSlot::Zapping)), None);
    assert_eq!(name(g.type_animation(s, WeaponTypeSlot::Reload1)), Some("a0".into()));
    assert_eq!(name(g.type_animation(s, WeaponTypeSlot::Reload2)), Some("a1".into()));
    assert_eq!(name(g.type_animation(s, WeaponTypeSlot::Melee)), None);
    // The seat's table is 12 long.
    assert_eq!(g.units[0].animations.len(), 12);
    assert_eq!(name(g.unit_animation(s, UnitSlot::Look)), None);
    let c = g.select_unit("crouch", "unarmed").unwrap();
    assert_eq!(name(g.unit_animation(c, UnitSlot::Look)), Some("a2".into()));
    assert_eq!(name(g.unit_animation(c, UnitSlot::Hovering)), Some("a3".into()));
    // The weapon's own table.
    assert_eq!(name(g.weapon_animation(WeaponSlot::Idle)), Some("a0".into()));
    assert_eq!(name(g.weapon_animation(WeaponSlot::Fire1)), Some("a3".into()));
    assert_eq!(name(g.weapon_animation(WeaponSlot::Reload1)), None);
    // First person: slot 10 ready, 13 melee (a 17-entry table).
    assert_eq!(
        name(g.first_person_animation(FirstPersonSlot::Ready)),
        Some("a1".into())
    );
    assert_eq!(
        name(g.first_person_animation(FirstPersonSlot::Melee)),
        Some("a2".into())
    );
    assert_eq!(name(g.first_person_animation(FirstPersonSlot::Idle)), None);
    assert_eq!(name(g.first_person_animation(FirstPersonSlot::ThrowGrenade)), None);
    assert_eq!(
        name(g.first_person_animation(FirstPersonSlot::OverheatedSuperchargeEnter)),
        None
    );
    // An index past the animations block is none; devices and vehicles.
    assert_eq!(name(g.vehicle_animation(VehicleSlot::Steering)), Some("a0".into()));
    assert_eq!(g.vehicles[0].animations.get(VehicleSlot::Roll), Some(99));
    assert_eq!(name(g.vehicle_animation(VehicleSlot::Roll)), None);
    assert_eq!(name(g.device_animation(DeviceSlot::Position)), Some("a1".into()));
    // Unit damage: s-kill back head.
    let slot = UnitDamageSlot::new(DamageKind::SoftKill, DamageDirection::Back, BodyPart::Head);
    assert_eq!(slot.index(), (2 * 4 + 3) * 11 + 2);
    assert_eq!(name(g.unit_damage_animation(slot)), Some("a3".into()));
    let other = UnitDamageSlot::new(DamageKind::SoftPing, DamageDirection::Front, BodyPart::Gut);
    assert_eq!(g.unit_damage_animation(other), None);
    assert_eq!(g.unit_damage.len(), UNIT_DAMAGE_SLOTS);
}

#[test]
fn slot_enumerations() {
    assert_eq!(UnitSlot::COUNT, 30);
    assert_eq!(WeaponClassSlot::COUNT, 55);
    assert_eq!(WeaponTypeSlot::COUNT, 10);
    assert_eq!(WeaponSlot::COUNT, 11);
    assert_eq!(VehicleSlot::COUNT, 8);
    assert_eq!(DeviceSlot::COUNT, 2);
    assert_eq!(FirstPersonSlot::COUNT, 28);
    assert_eq!(UnitSlot::Look.index(), 9);
    assert_eq!(UnitSlot::User0.index(), 13);
    assert_eq!(UnitSlot::FlyingFront.index(), 23);
    assert_eq!(WeaponClassSlot::MoveFront.index(), 8);
    assert_eq!(WeaponClassSlot::ThrowGrenade.index(), 20);
    assert_eq!(WeaponClassSlot::AimStill.index(), 25);
    assert_eq!(WeaponClassSlot::Melee.index(), 39);
    assert_eq!(WeaponClassSlot::Zapping.index(), 51);
    assert_eq!(WeaponTypeSlot::Melee.index(), 8);
    assert_eq!(WeaponSlot::Fire2.index(), 10);
    assert_eq!(FirstPersonSlot::Ready.index(), 10);
    assert_eq!(FirstPersonSlot::Melee.index(), 13);
    assert_eq!(FirstPersonSlot::ThrowGrenade.index(), 16);
    assert_eq!(FirstPersonSlot::Enter.index(), 23);
    assert_eq!(FirstPersonSlot::OverheatedSuperchargeEnter.index(), 27);
    assert_eq!(FirstPersonSlot::from_index(7), Some(FirstPersonSlot::ReloadEmpty));
    assert_eq!(FirstPersonSlot::from_index(28), None);
    assert_eq!(WeaponClassSlot::MoveFront.label(), "move-front");
    assert_eq!(FirstPersonSlot::OverheatedExit.label(), "o-h-exit");
    // Every slot's position matches its enumeration order.
    for (i, s) in UnitSlot::ALL.iter().enumerate() {
        assert_eq!(s.index(), i);
    }
    for i in 0..UNIT_DAMAGE_SLOTS {
        assert_eq!(UnitDamageSlot::from_index(i).unwrap().index(), i);
    }
    assert_eq!(UnitDamageSlot::from_index(UNIT_DAMAGE_SLOTS), None);
    let t: IndexTable<UnitSlot> = IndexTable::from_entries(vec![Some(4), None]);
    assert_eq!(t.get(UnitSlot::AirborneDead), Some(4));
    assert_eq!(t.get(UnitSlot::LandingDead), None);
    assert_eq!(t.get(UnitSlot::Hovering), None);
}

#[test]
fn permutations() {
    let mut a = Anim::new("idle");
    a.next = 1;
    a.cumulative = 1.0 / 3.0;
    let mut b = Anim::new("idle%1");
    b.next = 2;
    b.cumulative = 2.0 / 3.0;
    let mut c = Anim::new("idle%2");
    c.cumulative = 1.0;
    let g = read(&graph_map(&[], &[a, b, c], |_, s| s)).unwrap();
    assert_eq!(g.choose_permutation(0, 0.0), Some(0));
    assert_eq!(g.choose_permutation(0, 0.3), Some(0));
    assert_eq!(g.choose_permutation(0, 0.34), Some(1));
    assert_eq!(g.choose_permutation(0, 0.9), Some(2));
    assert_eq!(g.choose_permutation(9, 0.5), None);
    // A chain that loops stops.
    let mut x = Anim::new("x");
    x.next = 1;
    x.cumulative = 0.1;
    let mut y = Anim::new("y");
    y.next = 0;
    y.cumulative = 0.2;
    let g = read(&graph_map(&[], &[x, y], |_, s| s)).unwrap();
    assert_eq!(g.choose_permutation(0, 0.9), Some(1));
}

#[test]
fn matching_nodes_and_models() {
    let nodes = [
        graph_node("frame bone24", -1, 1, 0),
        graph_node("frame r wriste", 2, -1, 0),
        graph_node("frame gun", -1, -1, 1),
    ];
    let g = read(&graph_map(&nodes, &[test_animation()], |_, s| s)).unwrap();
    let m = g.match_nodes(["frame gun", "frame bone24"]);
    assert_eq!(m.graph_nodes, vec![Some(2), Some(0)]);
    assert!(m.is_complete());
    assert_eq!(m.select(&["a", "b", "c"]), Some(vec!["c", "a"]));
    // Names compare exactly, case included.
    let m = g.match_nodes(["frame gun", "Frame Bone24", "muzzle"]);
    assert_eq!(m.graph_nodes, vec![Some(2), None, None]);
    assert!(!m.is_complete());
    assert_eq!(m.missing(), vec![1, 2]);
    assert_eq!(m.select(&[1, 2, 3]), None);
    // The graph's parents: node 0's stored 0 is no parent.
    assert_eq!(g.parents(), vec![None, Some(0), Some(1)]);

    // The base-animation fit rule.
    let mut a = g.animations[0].clone();
    assert!(a.fits_model(2, 0x1234));
    a.node_list_checksum = 0x1234;
    assert!(a.fits_model(2, 0x1234));
    assert!(a.fits_model(2, 0));
    assert!(!a.fits_model(2, 0x9999));
    assert!(!a.fits_model(3, 0x1234));
    a.node_list_checksum = 0;
    assert!(a.fits_model(2, 0x9999));
    a.kind = AnimationType::Overlay;
    assert!(!a.fits_model(2, 0));
}

#[test]
fn rejects_other_classes() {
    let map = graph_map(&[], &[], |_, s| s);
    let e = AnimationGraph::read(&map, map.scenario()).unwrap_err().to_string();
    assert!(e.contains("not an animation graph"), "{e}");
}
