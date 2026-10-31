//! Models into Chimera (`game::model`, `game::props`) on a model written
//! here from specs/combat_arms/models_ltb.md's layout (no game data): a
//! two-node skeleton with a rigid piece on the second node, an animation
//! child that turns it, converted to a skinned `MapModel` and an
//! `AnimSet`, placed as a prop and played by Chimera's `animate_props`.

use bevy::prelude::*;
use chimera_world::map::{
    self, MapData, PropRig, PropSequence,
    anim::{Animator, BonePose},
};
use combat_arms::game::{model, props};

#[derive(Default)]
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn i16(&mut self, v: i16) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn f(&mut self, v: &[f32]) -> &mut Self {
        for x in v {
            self.0.extend(x.to_le_bytes());
        }
        self
    }
    fn s(&mut self, s: &str) -> &mut Self {
        self.u16(s.len() as u16);
        self.0.extend(s.as_bytes());
        self
    }
}

/// A model: root, and `arm` 10 u along +X (bind global), a rigid
/// triangle on `arm`; `anims` (name, compression-3 rotations of `arm`
/// per key at 0 and 100 ms); children named.
fn model_file(anims: &[(&str, [[i16; 4]; 2])], children: &[&str], pieces: bool) -> Vec<u8> {
    let mut w = W::default();
    w.u8(1).u8(0).u16(9);
    w.0.extend([0u8; 16]);
    w.u32(25);
    for _ in 0..15 {
        w.u32(0);
    }
    w.s("").f(&[50.0]).u32(0);
    if pieces {
        w.u32(1).s("tri").u32(1).f(&[0.0]).u32(0).u32(0);
        let mut body = W::default();
        body.u32(3).u32(1).u32(1).u32(1).u32(0x13).u32(0).u32(0).u32(0).u32(1);
        for p in [[10., 0., 0.], [20., 0., 0.], [10., 10., 5.]] {
            body.f(&p).f(&[0., 0., -1.]).f(&[0., 0.]);
        }
        body.u16(0).u16(1).u16(2);
        w.u32(1).u32(0).u32(0).u32(0).u32(0).u32(0).u8(0).u32(4).u32(body.0.len() as u32);
        w.0.extend(&body.0);
        w.u8(1).u8(1);
    } else {
        w.u32(0);
    }
    let id = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];
    let arm = [1., 0., 0., 10., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];
    w.s("Scene Root").u16(0).u8(0).f(&id).u32(1);
    w.s("arm").u16(1).u8(0).f(&arm).u32(0);
    w.u32(0);
    w.u32(children.len() as u32 + 1);
    for c in children {
        w.s(c);
    }
    w.u32(anims.len() as u32);
    for (name, rot) in anims {
        w.f(&[8., 8., 8.]).s(name).u32(3).u32(0).u32(2).u32(0).s("").u32(100).s("");
        w.u32(0).u32(0);
        w.u32(1).f(&[10., 0., 0.]).u32(2);
        for q in rot {
            for c in q {
                w.i16(*c);
            }
        }
    }
    w.u32(0);
    w.u32(anims.len() as u32);
    for (name, _) in anims {
        w.s(name).f(&[8., 8., 8.]).f(&[0., 0., 0.]);
    }
    for _ in children {
        w.u32(0);
    }
    w.0
}

/// `arm` turns 90 degrees about +Y over 100 ms (LithTech's frame).
const TURN: [[i16; 4]; 2] = [[0, 0, 0, 32767], [0, 23170, 0, 23170]];

fn files() -> impl Fn(&str) -> Option<Vec<u8>> {
    let parent = model_file(&[("base", [[0, 0, 0, 32767]; 2])], &["Turn_Child.LTB"], true);
    let child = model_file(&[("turn", TURN)], &[], false);
    move |p: &str| match p {
        "props/test/parent.ltb" => Some(parent.clone()),
        "props/test/turn_child.ltb" => Some(child.clone()),
        _ => None,
    }
}

const SCALE: f32 = 0.01;

#[test]
fn converts_skeleton_meshes_and_animations() {
    let file = files();
    let loaded = model::load("PROPS\\TEST\\PARENT.LTB", &file).unwrap();
    assert_eq!(loaded.path, "props/test/parent.ltb");
    assert_eq!(loaded.list.entries.len(), 2);
    let m = model::convert(&loaded, SCALE, &|_| String::new());
    let rig = m.rig.as_ref().unwrap();
    assert_eq!(rig.bones.len(), 2);
    assert_eq!(rig.bones[1].parent, Some(0));
    assert!(rig.bones[1].position.abs_diff_eq(Vec3::new(10.0, 0.0, 0.0), 1e-5));
    assert_eq!(rig.root.scale, Vec3::splat(SCALE));
    // Mesh: mirrored Z, metres, swapped winding, all on `arm`.
    let mesh = &m.meshes[0];
    assert!(Vec3::from(mesh.positions[2]).abs_diff_eq(Vec3::new(0.1, 0.1, -0.05), 1e-6));
    assert_eq!(mesh.normals[0], [0.0, 0.0, 1.0]);
    assert_eq!(mesh.indices, [0, 2, 1]);
    assert!(mesh.joints.iter().all(|j| j[0] == 1));
    assert!(mesh.joint_weights.iter().all(|w| w[0] == 1.0));
    // Animations: own then the child's, sampled at 30 per second.
    let set = &rig.animations;
    assert_eq!(set.sequences.len(), 2);
    let turn = set.sequence("TURN").unwrap();
    assert_eq!(turn, 1);
    let a = &set.animations[turn];
    assert_eq!(a.frames, 4);
    assert!((a.fps - 30.0).abs() < 1e-4);
    // Halfway: 45 degrees about +Y in LithTech's frame is -45 in ours.
    let mut pose: Vec<BonePose> = set.defaults.clone();
    set.sample(turn, 0.5, &mut pose);
    let (q, p) = pose[1];
    assert!(p.abs_diff_eq(Vec3::new(10.0, 0.0, 0.0), 1e-5));
    let (axis, angle) = q.to_axis_angle();
    assert!((angle - 45f32.to_radians()).abs() < 1e-3, "{angle}");
    assert!(axis.abs_diff_eq(Vec3::NEG_Y, 1e-3), "{axis}");
    // The end: the spec's quaternion (0, 0.707114, 0, 0.707114).
    set.sample(turn, 1.0, &mut pose);
    assert!(pose[1].0.abs_diff_eq(Quat::from_rotation_y(-90f32.to_radians()), 1e-3));
}

/// models_ltb.md "Child models": a missing child fails the whole model.
#[test]
fn a_missing_child_fails_the_model() {
    let parent = model_file(&[("base", TURN)], &["gone.ltb"], true);
    assert!(model::load("parent.ltb", &|p| (p == "parent.ltb").then(|| parent.clone())).is_err());
}

/// A placed prop's pieces are drawn in their render styles: each
/// piece's render-style index (here 0) into the object's list
/// (`RenderStyle0..`); no list, the default (opaque, lit).
#[test]
fn placed_prop_render_styles() {
    use chimera_world::map::MapAlpha;
    let file = files();
    let loaded = model::load("props/test/parent.ltb", &file).unwrap();
    let mut data = MapData::default();
    let lists: [&[String]; 3] = [&[], &["RS\\additive.ltb".into()], &["RS\\glass.ltb".into(), "RS\\GLOW.LTB".into()]];
    let mut drawn = Vec::new();
    for styles in lists {
        let i = props::place(&mut data, &loaded, &[], styles, "Test", Vec3::ONE, Quat::IDENTITY, None, SCALE, &file);
        let m = &data.models[data.props[i].model];
        assert_eq!(m.meshes.len(), 1);
        drawn.push((m.meshes[0].alpha, m.meshes[0].unlit));
    }
    assert_eq!(drawn, [(MapAlpha::Opaque, false), (MapAlpha::Add, true), (MapAlpha::Blend, false)]);
}

/// A placed prop plays its first sequence through Chimera's
/// `animate_props`: `CaPropsPlugin` starts it.
#[test]
fn placed_prop_animates() {
    let file = files();
    let loaded = model::load("props/test/parent.ltb", &file).unwrap();
    let mut data = MapData::default();
    let i = props::place(&mut data, &loaded, &[], &[], "Test", Vec3::ONE, Quat::IDENTITY, None, SCALE, &file);
    assert_eq!((i, data.props[i].entity, data.entities[0].classname()), (0, Some(0), "Test"));
    assert!(data.props[i].lighting.is_some());

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, props::CaPropsPlugin));
    app.add_message::<map::PropAnimationDone>()
        .add_systems(Update, map::animate_props.after(props::start_props));
    let set = data.models[0].rig.as_ref().unwrap().animations.clone();
    let joints: Vec<Entity> = (0..2).map(|_| app.world_mut().spawn(Transform::default()).id()).collect();
    let prop = app
        .world_mut()
        .spawn(PropRig {
            animator: Animator::new(set),
            joints: joints.clone(),
            seen: None,
            default: None,
            done: false,
        })
        .id();
    app.update();
    app.update();
    assert_eq!(app.world().get::<PropSequence>(prop).unwrap().default, "base");
    assert_eq!(app.world().get::<PropRig>(prop).unwrap().animator.main, Some(0));
    let t = app.world().get::<Transform>(joints[1]).unwrap().translation;
    assert!(t.abs_diff_eq(Vec3::new(10.0, 0.0, 0.0), 1e-5), "{t}");
}

/// A 4 x 4 32-bit texture (specs/combat_arms/textures_dtx.md, "Synthetic
/// fixtures": the header, then 16 pixels), all one colour.
fn dtx(bgra: [u8; 4]) -> Vec<u8> {
    let mut f = vec![0u8; 164];
    f[4] = 4;
    f[6] = 4;
    f[8..12].copy_from_slice(&(-5i32).to_le_bytes());
    f[12] = 1;
    f[16] = 8;
    f[25] = 1;
    f[26] = 3;
    for _ in 0..16 {
        f.extend(bgra);
    }
    f
}

/// A sprite file naming `frames` (five 32-bit fields, then each frame's
/// path with its 16-bit length: `effects::library::sprite_texture`'s
/// reading).
fn sprite(frames: &[&str]) -> Vec<u8> {
    let mut w = W::default();
    // What the five fields mean isn't in the specs; the reader skips them.
    w.u32(0).u32(0).u32(0).u32(0).u32(0);
    for f in frames {
        w.s(f);
    }
    w.0
}

/// A model skin that is a sprite (`.spr`: the animated weapon skins,
/// e.g. the "Prismatic" M416's) draws its first frame's texture, not the
/// untextured grey; a skin that isn't there stays untextured.
#[test]
fn a_sprite_skin_draws_its_first_frame() {
    let base = files();
    let red = dtx([0, 0, 255, 255]);
    let sprite = sprite(&["GUNS_T_HH\\HH_TEST_00.DTX", "GUNS_T_HH\\HH_TEST_01.DTX"]);
    let file = move |p: &str| match p {
        "guns_t_hh/hh_test.spr" => Some(sprite.clone()),
        "guns_t_hh/hh_test_00.dtx" => Some(red.clone()),
        _ => base(p),
    };
    let loaded = model::load("props/test/parent.ltb", &file).unwrap();
    for (skin, textured) in [("GUNS_T_HH\\HH_TEST.spr", true), ("GUNS_T_HH\\HH_GONE.spr", false)] {
        let mut data = MapData::default();
        let i = props::place(&mut data, &loaded, &[skin.to_string()], &[], "Test", Vec3::ONE, Quat::IDENTITY, None, SCALE, &file);
        let mesh = &data.models[data.props[i].model].meshes[0];
        assert_eq!(mesh.texture.is_some(), textured, "{skin}");
        if textured {
            let t = &data.textures[mesh.texture.unwrap()];
            assert_eq!((t.width, t.height), (4, 4));
            assert_eq!(&t.rgba8[..4], &[255, 0, 0, 255], "the first frame's red");
        }
    }
}

/// models_ltb.md "Sprite skins": a sprite skin's frames, each its texture,
/// play on the drawn copy's clock at ⌊1000 / r⌋ ms a frame
/// (`MapMesh::skin_frames`: Chimera's `render::skin_frames` swaps them);
/// a frame that doesn't resolve is left out, and one frame left draws
/// still.
#[test]
fn a_sprite_skin_plays_its_frames() {
    let base = files();
    let (red, green) = (dtx([0, 0, 255, 255]), dtx([0, 255, 0, 255]));
    let header = |n: u32, rate: u32, frames: &[&str]| {
        let mut w = W::default();
        w.u32(n).u32(rate).u32(0).u32(0).u32(0);
        for f in frames {
            w.s(f);
        }
        w.0
    };
    let two = header(3, 4, &["T\\A-1.DTX", "T\\GONE.DTX", "T\\A-2.DTX"]);
    let one = header(2, 10, &["T\\A-1.DTX", "T\\GONE.DTX"]);
    let file = move |p: &str| match p {
        "t/two.spr" => Some(two.clone()),
        "t/one.spr" => Some(one.clone()),
        "t/a-1.dtx" => Some(red.clone()),
        "t/a-2.dtx" => Some(green.clone()),
        _ => base(p),
    };
    let loaded = model::load("props/test/parent.ltb", &file).unwrap();
    let mut data = MapData::default();
    let i = props::place(&mut data, &loaded, &["T\\TWO.SPR".to_string()], &[], "Test", Vec3::ONE, Quat::IDENTITY, None, SCALE, &file);
    let mesh = data.models[data.props[i].model].meshes[0].clone();
    let frames = mesh.skin_frames.clone().expect("frames");
    assert_eq!(frames.frame_ms, 250);
    let colours: Vec<[u8; 4]> = frames.frames.iter().map(|&t| data.textures[t].rgba8[..4].try_into().unwrap()).collect();
    assert_eq!(colours, [[255, 0, 0, 255], [0, 255, 0, 255]], "red, then green; the missing frame left out");
    assert_eq!(mesh.texture, Some(frames.frames[0]), "drawn from its first frame");
    assert_eq!(mesh.material, "t/two.spr");
    let i = props::place(&mut data, &loaded, &["T\\ONE.SPR".to_string()], &[], "Test", Vec3::ONE, Quat::IDENTITY, None, SCALE, &file);
    let mesh = &data.models[data.props[i].model].meshes[0];
    assert!(mesh.texture.is_some() && mesh.skin_frames.is_none(), "one frame: still");
}
