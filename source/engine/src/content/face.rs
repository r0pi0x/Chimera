// Adapted from hl2-rs (MIT, see THIRD_PARTY.md): crates/source-assets/src/eyes.rs
//! A Source model's face as stored in its `.mdl`: its eyeballs (the
//! meshes the Eyes shader draws: bone, centre, up and forward, radius,
//! iris scale and z offset; specs/hl2dm/player_models.md 11.4) and its
//! mouths (the Teeth shader's bone and forward, 11.5), read from raw
//! bytes (versions 44-49), each tied to the material its mesh draws with
//! (`MapMesh::material`: the skin table's first family's name), and set on
//! a converted model's meshes (`dress`) as `MapEye` and `MapTeeth`.
//!
//! The eyeball layout and its checks follow hl2-rs's `eyes::decode`; the
//! mouths' (header 292, 20 bytes each) are ours.

use bevy::prelude::*;

use crate::map::{MapEye, MapMesh, MapTeeth};

fn i32le(d: &[u8], at: usize) -> Result<i32, String> {
    d.get(at..at + 4)
        .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| format!("face: read past the end at {at}"))
}

fn f32le(d: &[u8], at: usize) -> Result<f32, String> {
    Ok(f32::from_bits(i32le(d, at)? as u32))
}

fn vec3(d: &[u8], at: usize) -> Result<Vec3, String> {
    Ok(Vec3::new(f32le(d, at)?, f32le(d, at + 4)?, f32le(d, at + 8)?))
}

fn count(d: &[u8], at: usize, limit: usize) -> Result<usize, String> {
    let n = usize::try_from(i32le(d, at)?).map_err(|_| format!("face: negative count at {at}"))?;
    if n > limit {
        return Err(format!("face: table at {at} exceeds {limit}"));
    }
    Ok(n)
}

fn relative(d: &[u8], base: usize, at: usize) -> Result<usize, String> {
    usize::try_from(base as i64 + i32le(d, at)? as i64).map_err(|_| format!("face: offset at {at} before the file"))
}

fn check(d: &[u8]) -> Result<(), String> {
    if d.get(0..4) != Some(b"IDST") || !(44..=49).contains(&i32le(d, 4)?) {
        return Err("face: not a studio model of version 44-49".into());
    }
    Ok(())
}

/// One eyeball as stored (bone space, Source units) and the material
/// index of the mesh that draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct Eyeball {
    /// The mesh's material index (into the skin table's families).
    pub material: usize,
    pub eye: MapEye,
}

/// Every eyeball of the model's meshes (mesh material type 1), in body
/// part, model and mesh order.
pub fn eyeballs(data: &[u8]) -> Result<Vec<Eyeball>, String> {
    check(data)?;
    let bones = count(data, 156, 4096)?;
    let bodies = count(data, 232, 256)?;
    let start = usize::try_from(i32le(data, 236)?).map_err(|_| "face: body parts offset".to_string())?;
    let mut out = Vec::new();
    for body in 0..bodies {
        let b = start + body * 16;
        let models = count(data, b + 4, 256)?;
        let models_at = relative(data, b, b + 12)?;
        for m in 0..models {
            let m = models_at + m * 148;
            let meshes = count(data, m + 72, 4096)?;
            let mesh_start = relative(data, m, m + 76)?;
            let eyes = count(data, m + 100, 32)?;
            let eye_start = relative(data, m, m + 104)?;
            for mesh in 0..meshes {
                let at = mesh_start + mesh * 116;
                // Material type 1: an eyeball, its parameter the index.
                if i32le(data, at + 24)? != 1 {
                    continue;
                }
                let index = usize::try_from(i32le(data, at + 28)?).map_err(|_| "face: eyeball index".to_string())?;
                if index >= eyes {
                    return Err("face: mesh eyeball index outside its model's table".into());
                }
                let material = usize::try_from(i32le(data, at)?).map_err(|_| "face: mesh material".to_string())?;
                let e = eye_start + index * 172;
                let eye = MapEye {
                    bone: usize::try_from(i32le(data, e + 4)?).map_err(|_| "face: eyeball bone".to_string())?,
                    origin: vec3(data, e + 8)?,
                    z_offset: f32le(data, e + 20)?,
                    radius: f32le(data, e + 24)?,
                    up: vec3(data, e + 28)?,
                    forward: vec3(data, e + 40)?,
                    iris_scale: f32le(data, e + 60)?,
                    iris: None,
                };
                if eye.bone >= bones
                    || eye.radius <= 0.0
                    || eye.iris_scale <= 0.0
                    || eye.up.length_squared() < 0.5
                    || eye.forward.length_squared() < 0.5
                    || !eye.origin.is_finite()
                {
                    return Err("face: an eyeball's bone, basis or sizes are invalid".into());
                }
                out.push(Eyeball { material, eye });
            }
        }
    }
    Ok(out)
}

/// The model's mouths (header 292: bone, forward in the bone's frame,
/// flex descriptor), in file order.
pub fn mouths(data: &[u8]) -> Result<Vec<MapTeeth>, String> {
    check(data)?;
    let bones = count(data, 156, 4096)?;
    let n = count(data, 292, 64)?;
    let at = usize::try_from(i32le(data, 296)?).map_err(|_| "face: mouths offset".to_string())?;
    (0..n)
        .map(|k| {
            let o = at + k * 20;
            let bone = usize::try_from(i32le(data, o)?).map_err(|_| "face: mouth bone".to_string())?;
            if bone >= bones {
                return Err("face: a mouth's bone is outside the skeleton".into());
            }
            Ok(MapTeeth {
                bone,
                forward: vec3(data, o + 4)?,
                descriptor: i32le(data, o + 16)?,
            })
        })
        .collect()
}

/// Set the eyeballs and mouths of the model in `data` on `meshes`
/// (converted with skin family `table`'s names): each eyeball's mesh by
/// its material's name, its iris texture from the material's `$iris`
/// (`iris`); every mesh whose material uses the Teeth shader (`teeth`
/// says which) takes the first mouth.
pub fn dress(
    data: &[u8],
    names: &dyn Fn(usize) -> Option<String>,
    meshes: &mut [MapMesh],
    iris: &mut dyn FnMut(&str) -> Option<usize>,
    teeth: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    for e in eyeballs(data)? {
        let Some(name) = names(e.material) else { continue };
        let name = name.to_lowercase();
        for mesh in meshes.iter_mut().filter(|m| m.material == name) {
            let mut eye = e.eye.clone();
            eye.iris = iris(&mesh.material);
            mesh.eye = Some(Box::new(eye));
        }
    }
    if let Some(mouth) = mouths(data)?.into_iter().next() {
        for mesh in meshes.iter_mut().filter(|m| teeth(&m.material)) {
            mesh.teeth = Some(mouth.clone());
        }
    }
    Ok(())
}
