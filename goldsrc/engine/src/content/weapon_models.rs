//! A GoldSrc game's weapon models (specs/goldsrc/mdl.md), for any GoldSrc
//! game's guns:
//!
//! - **View models** (`view_model`, `v_*.mdl`): the model's origin is the
//!   eye, facing +X with Z up, so its bones and bind-pose meshes go to the
//!   eye's space by the studio root (`studio::root`: a quarter turn to
//!   face −Z, the axis change, the unit), unraised (`studio::
//!   anim_set_lifted` with no lift); its sequences with their events
//!   (5001-5031 muzzle flashes, 5004 sounds: the game's view rules read
//!   them), its attachments on their bones. Body 0's choice of every part
//!   (the hands and the gun). Built in the hand its meshes sit on (CS
//!   1.6's left of the eye), mirrored to the player's hand
//!   (`allow_flipping`, `cl_righthand`).
//! - **Held models** (`held`, `p_*.mdl`): GoldSrc draws them on the
//!   holder's skeleton by bone name (each bone the player has follows
//!   the player's). Each vertex is put in the frame of its nearest
//!   ancestor bone (itself included) that the holder's skeleton has (a
//!   hand), by the model's own bind pose; the hand with the most
//!   vertices holds the model (`MapHeldModel::bone`), any other (the
//!   Elites' left gun) is a merged part on its own hand. The muzzle is
//!   the first attachment, in the same frame.
//! - **Dropped models** (`dropped`, `w_*.mdl`): a held model's loose form
//!   (`map::loose::loose_key`), in the model's own space, as it lies.
//! - **Sounds** (`sounds`): the waves a model's events name.
//! - **Muzzle flashes** (`muzzle_flash`): a muzzle flash event's options,
//!   a two-digit code (mdl.md, "Events": tens the scale, units which
//!   `sprites/muzzleflash<n>.spr`), as a flash of the kit
//!   (`map::MapMuzzleFlash`, one per code: `flash_kind`): the sprite's
//!   first frame, its size the sprite's times a tenth of the scale.
//!
//! Known shortcuts:
//! - No bone controllers; body parts other than choice 0 are not drawn.
//! - The muzzle flash's scale (a tenth of the code's tens) and its light
//!   are ours (mdl.md open question 3); its sprite's later frames are not
//!   played.

use bevy::math::{Mat4, Vec3};
use bevy::prelude::Transform;
use goldsrc_formats::mdl::Mdl;

use super::studio;
use crate::{
    map::{MapAttachment, MapBone, MapFlashLight, MapHeldModel, MapModel, MapMuzzleFlash, MapTexture, MapViewModel},
    mount::Mount,
};

/// The sound event's number (mdl.md, "Events").
pub const SOUND_EVENT: i32 = 5004;

fn read(mount: &Mount, path: &str) -> Result<(Mdl, Mdl), String> {
    let bytes = mount.read(path).map_err(|e| format!("{path}: {e}"))?;
    let mdl = Mdl::parse(bytes).map_err(|e| format!("{path}: {e}"))?;
    let tex = studio::textures_of(mount, &mdl, path)?;
    Ok((mdl, tex))
}

/// The view model at `path` under `key` (module doc), with its textures
/// (the meshes' indices from 0) and the waves its events play.
pub fn view_model(mount: &Mount, path: &str, key: &str, decode: bool) -> Result<(MapViewModel, Vec<MapTexture>, Vec<String>), String> {
    let (mdl, tex) = read(mount, path)?;
    let mut warnings = Vec::new();
    let set = studio::anim_set_lifted(&mdl, &mut warnings, 0.0);
    let root = studio::root();
    let to_eye = |p: Vec3| root.rotation * p * super::METERS_PER_UNIT;
    let (meshes, bounds) = studio::meshes(&mdl, &tex, 1, &|_, p, n| Some((0, to_eye(p), root.rotation * n)))
        .pop()
        .unwrap_or_default();
    if meshes.is_empty() {
        return Err(format!("{path}: no meshes"));
    }
    let attachments = mdl
        .attachments
        .iter()
        .filter(|a| a.bone >= 0 && (a.bone as usize) < mdl.bones.len())
        .map(|a| MapAttachment {
            name: a.name.clone(),
            bone: a.bone as usize,
            local: Transform::from_translation(Vec3::from(a.position)),
        })
        .collect();
    let model = MapViewModel {
        key: key.to_string(),
        right_handed: right_handed(&mdl),
        model: MapModel { meshes, bounds, ..Default::default() },
        bones: studio::map_bones(&mdl),
        root,
        animations: Some(std::sync::Arc::new(set)),
        allow_flipping: true,
        attachments,
        light_origin: Vec3::ZERO,
    };
    Ok((model, studio::textures(&tex, path, decode), sounds(&mdl)))
}

/// Which hand a view model was built in: where its vertices sit across the
/// view (GoldSrc's +Y is left) posed at its first sequence's first frame
/// (its bind pose may hold the gun any way), on the right or not.
fn right_handed(mdl: &Mdl) -> bool {
    let Some(pose) = studio::posed_globals(mdl, 0, 0) else { return true };
    let (mut sum, mut n) = (0.0f32, 0usize);
    for sub in mdl.submodels_for_body(0) {
        for (v, b) in sub.vertices.iter().zip(&sub.vertex_bones) {
            if let Some((q, p)) = pose.get(*b as usize) {
                sum += (*p + *q * Vec3::from(*v)).y;
                n += 1;
            }
        }
    }
    n == 0 || sum / n as f32 <= 0.0
}

/// The muzzle flash events' numbers (attachments 0-3).
pub const MUZZLE_EVENTS: [i32; 4] = [5001, 5011, 5021, 5031];

/// The kit's muzzle flash kind of a muzzle flash event's options (module
/// doc): `<game>:<code>`.
pub fn flash_kind(game: &str, options: &str) -> String {
    format!("{game}:{}", options.trim())
}

/// The muzzle flash codes `set`'s sequences' events use, each once.
pub fn flash_codes(set: &crate::map::anim::AnimSet) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in set.sequences.iter().flat_map(|s| &s.events) {
        let code = e.options.trim().to_string();
        if MUZZLE_EVENTS.contains(&e.event) && !code.is_empty() && !out.contains(&code) {
            out.push(code);
        }
    }
    out
}

/// The sprite a muzzle flash code draws (`sprites/muzzleflash<units>.spr`).
pub fn flash_sprite(code: &str) -> Option<String> {
    let n: u32 = code.trim().parse().ok()?;
    Some(format!("sprites/muzzleflash{}.spr", (n % 10).max(1)))
}

/// The muzzle flash of code `code` (module doc), with its texture (the
/// flash's index 0: the caller moves it); None when the code or its
/// sprite isn't there.
pub fn muzzle_flash(mount: &Mount, code: &str, decode: bool) -> Option<(MapMuzzleFlash, MapTexture)> {
    let n: u32 = code.trim().parse().ok()?;
    let scale = n / 10;
    let path = flash_sprite(code)?;
    let spr = goldsrc_formats::spr::Sprite::parse(&mount.read(&path).ok()?).ok()?;
    let frame = *spr.all_frames().first()?;
    let texture = MapTexture {
        name: path.clone(),
        srgb: true,
        mips: Vec::new(),
        width: frame.width,
        height: frame.height,
        rgba8: if decode { spr.rgba(frame) } else { Vec::new() },
        bc: None,
    };
    let size = frame.width.max(frame.height) as f32 * scale.max(1) as f32 * 0.1 * super::METERS_PER_UNIT;
    let exp = 2f32.powi(5) / 255.0 / 16.0;
    let flash = MapMuzzleFlash {
        texture: Some(0),
        sprites: vec![(0.0, size)],
        scale: (0.9, 1.1),
        color: [1.0, 1.0, 1.0],
        life: 0.05,
        light: Some(MapFlashLight {
            color: Vec3::new(255.0, 192.0, 64.0) * exp,
            radius: (32.0 * super::METERS_PER_UNIT, 64.0 * super::METERS_PER_UNIT),
            life: 0.05,
        }),
        ..Default::default()
    };
    Some((flash, texture))
}

/// The waves `mdl`'s sequences' sound events name, each once, lower-case.
pub fn sounds(mdl: &Mdl) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in &mdl.sequences {
        for e in s.events.iter().filter(|e| e.event == SOUND_EVENT) {
            let w = e.options.trim().to_ascii_lowercase();
            if !w.is_empty() && !out.contains(&w) {
                out.push(w);
            }
        }
    }
    out
}

/// The held model at `path` under `key` on a skeleton whose bones are
/// named `skeleton` (module doc), with its textures.
pub fn held(mount: &Mount, path: &str, key: &str, skeleton: &[&str], decode: bool) -> Result<(MapHeldModel, Vec<MapTexture>), String> {
    let (mdl, tex) = read(mount, path)?;
    let globals = studio::bind_globals(&mdl);
    let has = |i: usize| skeleton.iter().any(|n| n.eq_ignore_ascii_case(&mdl.bones[i].name));
    // Each bone's anchor: its nearest ancestor the holder has.
    let anchor: Vec<Option<usize>> = (0..mdl.bones.len())
        .map(|i| {
            let mut at = Some(i);
            while let Some(j) = at {
                if has(j) {
                    return Some(j);
                }
                at = (mdl.bones[j].parent >= 0).then_some(mdl.bones[j].parent as usize);
            }
            None
        })
        .collect();
    let anchors: Vec<usize> = {
        let mut a: Vec<usize> = anchor.iter().flatten().copied().collect();
        a.sort_unstable();
        a.dedup();
        a
    };
    if anchors.is_empty() {
        return Err(format!("{path}: no bone the holder has"));
    }
    let frame = |a: usize| Mat4::from_rotation_translation(globals[a].0, globals[a].1).inverse();
    let frames: Vec<Mat4> = anchors.iter().map(|a| frame(*a)).collect();
    let lists = studio::meshes(&mdl, &tex, anchors.len(), &|bone, p, n| {
        let a = (*anchor.get(bone)?)?;
        let list = anchors.iter().position(|x| *x == a)?;
        Some((list, frames[list].transform_point3(p), frames[list].transform_vector3(n).normalize_or_zero()))
    });
    let size = |l: &(Vec<crate::map::MapMesh>, (Vec3, Vec3))| l.0.iter().map(|m| m.indices.len()).sum::<usize>();
    let main = (0..lists.len()).max_by_key(|i| size(&lists[*i])).unwrap_or(0);
    if size(&lists[main]) == 0 {
        return Err(format!("{path}: no meshes"));
    }
    let to_main = |bone: usize, local: Vec3| -> Transform {
        let g = Mat4::from_rotation_translation(globals[bone].0, globals[bone].1) * Mat4::from_translation(local);
        Transform::from_matrix(frames[main] * g)
    };
    let attachments: Vec<(String, Transform)> = mdl
        .attachments
        .iter()
        .filter(|a| a.bone >= 0 && (a.bone as usize) < mdl.bones.len())
        .map(|a| (a.name.clone(), to_main(a.bone as usize, Vec3::from(a.position))))
        .collect();
    let own = MapModel { meshes: lists[main].0.clone(), bounds: lists[main].1, ..Default::default() };
    let merged: Vec<(String, MapModel)> = lists
        .iter()
        .enumerate()
        .filter(|(i, l)| *i != main && size(l) > 0)
        .map(|(i, l)| (mdl.bones[anchors[i]].name.clone(), MapModel { meshes: l.0.clone(), bounds: l.1, ..Default::default() }))
        .collect();
    // The whole model in the main hand's frame (a loose one, or held whole).
    let whole = if merged.is_empty() {
        own.clone()
    } else {
        let all = studio::meshes(&mdl, &tex, 1, &|_, p, n| Some((0, frames[main].transform_point3(p), frames[main].transform_vector3(n).normalize_or_zero())));
        let (meshes, bounds) = all.into_iter().next().unwrap_or_default();
        MapModel { meshes, bounds, ..Default::default() }
    };
    Ok((
        MapHeldModel {
            key: key.to_string(),
            model: whole,
            bone: mdl.bones[anchors[main]].name.clone(),
            muzzle: attachments.first().map(|a| a.1),
            attachments,
            own: (!merged.is_empty()).then_some(own),
            merged,
            nose: None,
            turned: Vec::new(),
        },
        studio::textures(&tex, path, decode),
    ))
}

/// The dropped model at `path` (`w_*.mdl`) as held model `key`'s loose
/// form (`map::loose::loose_key`), with its textures: body 0's meshes in
/// the model's own space (its bind pose: lying as on the ground, facing
/// +X), the nose +X.
pub fn dropped(mount: &Mount, path: &str, key: &str, decode: bool) -> Result<(MapHeldModel, Vec<MapTexture>), String> {
    let (mdl, tex) = read(mount, path)?;
    let (meshes, bounds) = studio::meshes(&mdl, &tex, 1, &|_, p, n| Some((0, p, n))).pop().unwrap_or_default();
    if meshes.is_empty() {
        return Err(format!("{path}: no meshes"));
    }
    Ok((
        MapHeldModel {
            key: crate::map::loose::loose_key(key),
            model: MapModel { meshes, bounds, ..Default::default() },
            bone: String::new(),
            muzzle: None,
            attachments: Vec::new(),
            own: None,
            merged: Vec::new(),
            nose: Some(Vec3::X),
            turned: Vec::new(),
        },
        studio::textures(&tex, path, decode),
    ))
}

/// A skeleton's bone names.
pub fn bone_names(bones: &[MapBone]) -> Vec<&str> {
    bones.iter().map(|b| b.name.as_str()).collect()
}
