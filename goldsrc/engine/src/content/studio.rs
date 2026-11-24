//! A studio model (MDL version 10, specs/goldsrc/mdl.md) as a character
//! body: its skeleton, its meshes skinned to it, its hitboxes with their
//! hit groups, and every sequence of group 0 as animations.
//!
//! - Bones: each bone's bind pose is its default channel values: position
//!   (x, y, z) and the rotation q = q_z · q_y · q_x of its three Euler
//!   angles (mdl.md, "Bones"). A model's origin is the standing box's
//!   centre (bsp.md, "Hull sizes"), Chimera's character origin its feet,
//!   so every root bone is raised by the box's half height
//!   (`entities::STAND_HALF_HEIGHT`) in every frame of every sequence. The
//!   bind pose (the defaults) already stands on the origin (CS 1.6's
//!   player models: `Bip01` 38 units up) and is kept as it is for the
//!   skeleton and the meshes; the animation set's rest pose is raised too,
//!   as its frames are.
//! - Meshes: body number 0's submodels; each vertex in its bone's frame
//!   carried to model space by the bind pose, rigidly skinned to that bone;
//!   texture coordinates texels / size (mdl.md, "Triangle commands"); one
//!   mesh per texture; each triangle wound counter-clockwise about its
//!   corners' normals. The skeleton's space is GoldSrc's (Z up, units, the
//!   model facing +X, as a Source model), so the body's `root` is
//!   Source's: a quarter turn to face −Z, the axis change, the unit.
//! - Hitboxes (mdl.md, "Hitboxes"): groups 1-7 are Chimera's hit groups;
//!   0 and the shield's 8 generic. The shield's box (group 8) is left out:
//!   it counts only while a shield is carried (mdl.md).
//! - Sequences: each blend an animation (frames sampled from the
//!   animation streams, a track left out where a bone keeps its bind
//!   pose, one value where it never changes); a sequence with blends a
//!   one-row grid on the `body_pitch` pose parameter (its blend range,
//!   degrees); its linear movement the animation's speed. A sequence that
//!   aims, shoots, reloads or flinches (`upper_body`) weighs only the
//!   upper body (the bones from `Bip01 Spine` down the tree) so it layers
//!   over a gait sequence, as CS 1.6's players blend gait and weapon
//!   sequences; the others weigh every bone.
//!
//! Known shortcuts:
//! - Sequences in groups 1 and up (sequence files) are left out: no CS
//!   1.6 player model has any.
//! - Chrome (texture flag 2) is drawn with its own coordinates; no bone
//!   controllers; body parts other than choice 0 are not drawn.
//! - Which way the aim blends run against the view's pitch is not
//!   measured (mdl.md, open question 2): drivers leave the parameter at 0.

use std::{collections::HashMap, sync::Arc};

use bevy::{
    math::{Quat, Vec2, Vec3},
    prelude::Transform,
};
use goldsrc_formats::mdl::Mdl;

use super::{METERS_PER_UNIT, entities::STAND_HALF_HEIGHT};
use crate::{
    core::{BodyRegion, Hitbox, Hitgroup},
    map::{
        BoneBox, MapAlpha, MapBone, MapCharacterModel, MapData, MapMesh, MapModel, MapTexture,
        anim::{AnimSet, Animation, Axis, BonePose, PoseParam, Sequence, Track},
    },
    mount::Mount,
};

/// The pose parameter an aiming sequence's blends lie on (degrees).
pub const PITCH_PARAM: &str = "body_pitch";

/// The bone where the upper body starts (`upper_body` sequences).
pub const UPPER_BODY_ROOT: &str = "Bip01 Spine";

/// A body and the textures its meshes name (indices from 0).
#[derive(Clone)]
pub struct Body {
    pub character: MapCharacterModel,
    pub textures: Vec<MapTexture>,
    pub warnings: Vec<String>,
}

/// A rotation from three Euler angles about X, Y and Z (radians): q_z · q_y · q_x.
pub fn euler(a: [f32; 3]) -> Quat {
    (Quat::from_rotation_z(a[2]) * Quat::from_rotation_y(a[1]) * Quat::from_rotation_x(a[0])).normalize()
}

/// The skeleton's space (GoldSrc's axes, units, facing +X) to a
/// character's (metres, Y up, facing −Z).
pub fn root() -> Transform {
    let face = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let axes = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    Transform::from_rotation(face * axes).with_scale(Vec3::splat(METERS_PER_UNIT))
}

/// Whether sequence `name` layers over the gait (module doc).
pub fn upper_body(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("_aim_") || n.contains("_shoot") || n.contains("_reload_") || n.contains("flinch")
}

fn raise(parent: i32, p: Vec3) -> Vec3 {
    if parent < 0 { p + Vec3::Z * STAND_HALF_HEIGHT } else { p }
}

/// Bone `i`'s local pose at frame `f` of blend `blend` of sequence `seq`
/// (raised when a root), from each channel's stored values.
fn locals_of(mdl: &Mdl, anim: &[u8], seq: usize, blend: usize) -> Result<Vec<Vec<BonePose>>, String> {
    let frames = mdl.sequences[seq].frames.max(1) as usize;
    let mut out = Vec::with_capacity(mdl.bones.len());
    for (i, b) in mdl.bones.iter().enumerate() {
        let mut channels = Vec::with_capacity(6);
        for c in 0..6 {
            channels.push(mdl.stored_values(anim, seq, blend, i, c).map_err(|e| e.0)?);
        }
        let value = |c: usize, f: usize| b.value[c] + *channels[c].get(f).unwrap_or(&0) as f32 * b.scale[c];
        out.push(
            (0..frames)
                .map(|f| {
                    let p = raise(b.parent, Vec3::new(value(0, f), value(1, f), value(2, f)));
                    (euler([value(3, f), value(4, f), value(5, f)]), p)
                })
                .collect(),
        );
    }
    Ok(out)
}

/// A bone's frames as a track, or None where it keeps `bind` throughout.
fn track(bone: usize, frames: &[BonePose], bind: BonePose) -> Option<Track> {
    let same_q = |a: Quat, b: Quat| a.dot(b).abs() > 1.0 - 1e-7;
    let same_p = |a: Vec3, b: Vec3| (a - b).length_squared() < 1e-10;
    let rot_const = frames.iter().all(|(q, _)| same_q(*q, frames[0].0));
    let pos_const = frames.iter().all(|(_, p)| same_p(*p, frames[0].1));
    let rotation = if rot_const && same_q(frames[0].0, bind.0) {
        None
    } else if rot_const {
        Some(vec![frames[0].0])
    } else {
        Some(frames.iter().map(|(q, _)| *q).collect())
    };
    let position = if pos_const && same_p(frames[0].1, bind.1) {
        None
    } else if pos_const {
        Some(vec![frames[0].1])
    } else {
        Some(frames.iter().map(|(_, p)| *p).collect())
    };
    (rotation.is_some() || position.is_some()).then_some(Track { bone, rotation, position })
}

/// The bones' default values as local poses, as stored (not raised).
pub fn bind_locals(mdl: &Mdl) -> Vec<BonePose> {
    mdl.bones
        .iter()
        .map(|b| (euler([b.value[3], b.value[4], b.value[5]]), Vec3::new(b.value[0], b.value[1], b.value[2])))
        .collect()
}

/// Model `mdl`'s sequences as an animation set (module doc).
pub fn anim_set(mdl: &Mdl, warnings: &mut Vec<String>) -> AnimSet {
    let defaults: Vec<BonePose> = mdl
        .bones
        .iter()
        .map(|b| (euler([b.value[3], b.value[4], b.value[5]]), raise(b.parent, Vec3::new(b.value[0], b.value[1], b.value[2]))))
        .collect();
    let parents: Vec<Option<usize>> = mdl.bones.iter().map(|b| (b.parent >= 0).then_some(b.parent as usize)).collect();
    // The upper body: the spine and every bone under it.
    let spine = mdl.bones.iter().position(|b| b.name.eq_ignore_ascii_case(UPPER_BODY_ROOT));
    let upper: Vec<f32> = (0..mdl.bones.len())
        .map(|i| {
            let mut at = Some(i);
            while let Some(j) = at {
                if Some(j) == spine {
                    return 1.0;
                }
                at = parents[j];
            }
            0.0
        })
        .collect();
    let mut set = AnimSet {
        defaults: defaults.clone(),
        bases: vec![defaults.clone()],
        parents: parents.clone(),
        bone_names: mdl.bones.iter().map(|b| b.name.to_ascii_lowercase()).collect(),
        params: vec![PoseParam { name: PITCH_PARAM.into(), start: -90.0, end: 90.0, looping: 0.0 }],
        ..Default::default()
    };
    let mut names: HashMap<String, usize> = HashMap::new();
    for (si, s) in mdl.sequences.iter().enumerate() {
        if s.group != 0 {
            warnings.push(format!("sequence {}: in group {}, not read", s.name, s.group));
            continue;
        }
        let blends = s.blends.max(1) as usize;
        let duration = s.duration();
        let lm = Vec3::from(s.linear_movement);
        let speed = if duration > 0.0 && s.motion_type != 0 { lm.length() / duration } else { 0.0 };
        let motion = if speed > 0.0 { Vec2::new(lm.x, lm.y) / duration } else { Vec2::ZERO };
        let mut anims = Vec::with_capacity(blends);
        for blend in 0..blends {
            let locals = match locals_of(mdl, &mdl.bytes, si, blend) {
                Ok(l) => l,
                Err(e) => {
                    warnings.push(format!("sequence {} blend {blend}: {e}", s.name));
                    vec![vec![(Quat::IDENTITY, Vec3::ZERO)]; 0]
                }
            };
            let tracks = locals
                .iter()
                .enumerate()
                .filter_map(|(bone, frames)| track(bone, frames, defaults[bone]))
                .collect();
            set.animations.push(Animation {
                name: if blends > 1 { format!("{}@{blend}", s.name) } else { s.name.clone() },
                fps: s.fps,
                frames: s.frames.max(1) as usize,
                delta: false,
                base: 0,
                speed,
                motion,
                tracks,
                ik_targets: Vec::new(),
            });
            anims.push(set.animations.len() - 1);
        }
        let weights = if upper_body(&s.name) { upper.clone() } else { vec![1.0; mdl.bones.len()] };
        let axis0 = if blends > 1 {
            Axis {
                param: Some(0),
                keys: Vec::new(),
                range: (s.blend_start[0], s.blend_end[0]),
            }
        } else {
            Axis::default()
        };
        names.entry(s.name.to_ascii_lowercase()).or_insert(set.sequences.len());
        set.sequences.push(Sequence {
            name: s.name.clone(),
            activity: if s.activity != 0 { s.activity.to_string() } else { String::new() },
            activity_weight: s.activity_weight,
            looping: s.looping(),
            fade_in: 0.1,
            fade_out: 0.1,
            grid: (blends, 1),
            anims,
            axes: [axis0, Axis::default()],
            bone_weights: weights,
            ..Default::default()
        });
    }
    set.names = names;
    set
}

/// The texture records of `mdl`, or of its `<name>T.mdl` beside `path`
/// when it has none.
fn textures_of(mount: &Mount, mdl: &Mdl, path: &str) -> Result<Mdl, String> {
    if !mdl.textures.is_empty() {
        return Ok(mdl.clone());
    }
    let t = format!("{}t.mdl", path.trim_end_matches(".mdl").trim_end_matches(".MDL"));
    let bytes = mount.read(&t).map_err(|e| format!("{t}: {e}"))?;
    Mdl::parse(bytes).map_err(|e| format!("{t}: {e}"))
}

/// The model at `path` on `mount` as a body of game `game` (module doc);
/// `decode` off: its textures placeholders of their sizes.
pub fn character(mount: &Mount, game: &'static str, path: &str, decode: bool) -> Result<Body, String> {
    let bytes = mount.read(path).map_err(|e| format!("{path}: {e}"))?;
    let mdl = Mdl::parse(bytes).map_err(|e| format!("{path}: {e}"))?;
    let tex_file = textures_of(mount, &mdl, path)?;
    let mut warnings = Vec::new();
    let set = anim_set(&mdl, &mut warnings);
    // The bind pose (unraised) in skeleton space.
    let bind = bind_locals(&mdl);
    let mut globals: Vec<BonePose> = Vec::with_capacity(mdl.bones.len());
    for (i, b) in mdl.bones.iter().enumerate() {
        let (q, p) = bind[i];
        globals.push(match (b.parent >= 0).then(|| globals.get(b.parent as usize)).flatten() {
            Some((pq, pp)) => (*pq * q, *pp + *pq * p),
            None => (q, p),
        });
    }
    let root = root();
    let to_char = |p: Vec3| root.rotation * p * METERS_PER_UNIT;
    // The textures, each once.
    let skin = tex_file.skins.first().cloned().unwrap_or_default();
    let textures: Vec<MapTexture> = tex_file
        .textures
        .iter()
        .map(|t| MapTexture {
            name: format!("{path}/{}", t.name.to_ascii_lowercase()),
            srgb: true,
            mips: Vec::new(),
            width: t.width,
            height: t.height,
            rgba8: if decode { t.rgba() } else { Vec::new() },
            bc: None,
        })
        .collect();
    let mut by_texture: Vec<Option<MapMesh>> = vec![None; textures.len().max(1)];
    let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
    for sub in mdl.submodels_for_body(0) {
        let vertex = |i: i16| -> Option<(usize, Vec3)> {
            let i = usize::try_from(i).ok()?;
            let bone = *sub.vertex_bones.get(i)? as usize;
            let (q, p) = *globals.get(bone)?;
            Some((bone, p + q * Vec3::from(*sub.vertices.get(i)?)))
        };
        let normal = |i: i16| -> Vec3 {
            let Ok(i) = usize::try_from(i) else { return Vec3::Z };
            let bone = sub.normal_bones.get(i).copied().unwrap_or(0) as usize;
            let q = globals.get(bone).map_or(Quat::IDENTITY, |g| g.0);
            (q * Vec3::from(sub.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]))).normalize_or_zero()
        };
        for mesh in &sub.meshes {
            let ti = skin.get(mesh.skin_ref.max(0) as usize).copied().unwrap_or(0).max(0) as usize;
            let Some(tex) = tex_file.textures.get(ti) else { continue };
            let slot = &mut by_texture[ti.min(textures.len().saturating_sub(1))];
            let out = slot.get_or_insert_with(|| MapMesh {
                material: tex.name.to_ascii_lowercase(),
                texture: Some(ti),
                color: [255; 3],
                alpha: if tex.flags & 64 != 0 {
                    MapAlpha::Mask(0.5)
                } else if tex.flags & 32 != 0 {
                    MapAlpha::Add
                } else {
                    MapAlpha::Opaque
                },
                unlit: tex.flags & 4 != 0,
                ..Default::default()
            });
            for tri in mesh.triangles() {
                let corners: Vec<(usize, Vec3, Vec3)> = tri
                    .iter()
                    .filter_map(|c| vertex(c.vertex).map(|(b, p)| (b, p, normal(c.normal))))
                    .collect();
                if corners.len() != 3 {
                    continue;
                }
                let face = (corners[1].1 - corners[0].1).cross(corners[2].1 - corners[0].1);
                let along: Vec3 = corners.iter().map(|c| c.2).sum();
                let order = if face.dot(along) < 0.0 { [0, 2, 1] } else { [0, 1, 2] };
                for k in order {
                    let (bone, p, n) = corners[k];
                    let c = tri[k];
                    let at = to_char(p);
                    lo = lo.min(at);
                    hi = hi.max(at);
                    out.indices.push(out.positions.len() as u32);
                    out.positions.push(at.to_array());
                    out.normals.push((root.rotation * n).to_array());
                    out.uvs.push([c.s as f32 / tex.width.max(1) as f32, c.t as f32 / tex.height.max(1) as f32]);
                    out.joints.push([bone as u16, 0, 0, 0]);
                    out.joint_weights.push([1.0, 0.0, 0.0, 0.0]);
                }
            }
        }
    }
    let meshes: Vec<MapMesh> = by_texture.into_iter().flatten().filter(|m| !m.indices.is_empty()).collect();
    if meshes.is_empty() {
        return Err(format!("{path}: no meshes"));
    }
    // Hitboxes, the shield's left out.
    let group = |g: i32| match g {
        1 => Hitgroup::Head,
        2 => Hitgroup::Chest,
        3 => Hitgroup::Stomach,
        4 => Hitgroup::LeftArm,
        5 => Hitgroup::RightArm,
        6 => Hitgroup::LeftLeg,
        7 => Hitgroup::RightLeg,
        _ => Hitgroup::Generic,
    };
    let kept: Vec<&goldsrc_formats::mdl::Hitbox> = mdl
        .hitboxes
        .iter()
        .filter(|h| h.group != 8 && h.bone >= 0 && (h.bone as usize) < mdl.bones.len())
        .collect();
    let hitboxes: Vec<Hitbox> = kept
        .iter()
        .map(|h| {
            let (q, p) = globals[h.bone as usize];
            let (a, b) = (Vec3::from(h.mins), Vec3::from(h.maxs));
            Hitbox::cuboid(
                to_char(p + q * ((a + b) / 2.0)),
                (b - a) / 2.0 * METERS_PER_UNIT,
                root.rotation * q,
                BodyRegion::from_hitgroup(group(h.group)),
            )
        })
        .collect();
    let boxes: Vec<BoneBox> = kept
        .iter()
        .map(|h| {
            let (a, b) = (Vec3::from(h.mins), Vec3::from(h.maxs));
            BoneBox {
                bone: h.bone as usize,
                center: (a + b) / 2.0,
                half: (b - a) / 2.0,
                group: group(h.group),
            }
        })
        .collect();
    let bones: Vec<MapBone> = mdl
        .bones
        .iter()
        .zip(&bind)
        .map(|(b, (q, p))| MapBone {
            name: b.name.clone(),
            parent: (b.parent >= 0).then_some(b.parent as usize),
            position: *p,
            rotation: *q,
        })
        .collect();
    Ok(Body {
        character: MapCharacterModel {
            game: Some(game),
            team: None,
            name: None,
            class: None,
            model: MapModel {
                meshes,
                bounds: if lo.x <= hi.x { (lo, hi) } else { Default::default() },
                ..Default::default()
            },
            hitboxes,
            bones,
            root,
            animations: Some(Arc::new(set)),
            boxes,
            ragdoll: None,
        },
        textures,
        warnings,
    })
}

/// `body` added to `map`'s kit, its textures appended to the map's (its
/// meshes' indices moved past the map's own).
pub fn add_to(map: &mut MapData, body: Body) {
    let offset = map.textures.len();
    let mut c = body.character;
    for m in &mut c.model.meshes {
        if let Some(t) = &mut m.texture {
            *t += offset;
        }
    }
    map.textures.extend(body.textures);
    map.warnings.extend(body.warnings);
    map.kit.characters.push(c);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_turns_z_after_y_after_x() {
        let q = euler([0.0, 0.0, std::f32::consts::FRAC_PI_2]);
        assert!((q * Vec3::X - Vec3::Y).length() < 1e-5);
        let q = euler([std::f32::consts::FRAC_PI_2, 0.0, std::f32::consts::FRAC_PI_2]);
        // X first: Y to Z, then Z about Z stays Z.
        assert!((q * Vec3::Y - Vec3::Z).length() < 1e-5);
    }

    #[test]
    fn the_root_faces_minus_z_with_z_up() {
        let r = root();
        assert!((r.rotation * Vec3::X - Vec3::NEG_Z).length() < 1e-5, "{}", r.rotation * Vec3::X);
        assert!((r.rotation * Vec3::Z - Vec3::Y).length() < 1e-5);
    }

    #[test]
    fn weapon_sequences_layer_over_the_gait() {
        assert!(upper_body("ref_aim_ak47"));
        assert!(upper_body("crouch_shoot_knife"));
        assert!(upper_body("gut_flinch"));
        assert!(!upper_body("run"));
        assert!(!upper_body("crouch_idle"));
    }
}
