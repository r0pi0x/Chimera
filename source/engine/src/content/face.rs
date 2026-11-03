// Adapted from hl2-rs (MIT, see THIRD_PARTY.md): crates/source-assets/src/eyes.rs
// Adapted from hl2-rs (MIT, see THIRD_PARTY.md): crates/source-assets/src/flexes.rs
//! A Source model's face as stored in its `.mdl`: its eyeballs (the
//! meshes the Eyes shader draws: bone, centre, up and forward, radius,
//! iris scale and z offset; specs/hl2dm/player_models.md 11.4) and its
//! mouths (the Teeth shader's bone and forward, 11.5), read from raw
//! bytes (versions 44-49), each tied to the material its mesh draws with
//! (`MapMesh::material`: the skin table's first family's name), and set on
//! a converted model's meshes (`dress`) as `MapEye` and `MapTeeth`.
//!
//! Its flexes (`flexes`: descriptors, controllers, rules, each mesh's
//! vertex offsets; `MapFace`, `MapMesh::morphs`) and what a game does
//! with them each frame: the rules from controller values to descriptor
//! weights (`run_rules`), the lids of FACS eyeballs (`eyelids`) and each
//! mesh flex's weight on its two sides (`flex_weights`).
//!
//! The eyeball layout and its checks follow hl2-rs's `eyes::decode`, the
//! flex tables, rule program and ramps its `flexes`; the mouths' (header
//! 292, 20 bytes each) are ours.

use bevy::prelude::*;

use crate::map::{
    MapEye, MapMesh, MapMorph, MapTeeth,
    face::{MapEyelids, MapFace, MapFlex, MapFlexController, MapFlexRule},
};

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

fn string(d: &[u8], at: usize) -> Result<String, String> {
    let tail = d.get(at..).ok_or("face: string outside the model")?;
    let end = tail.iter().take(256).position(|b| *b == 0).ok_or("face: unterminated string")?;
    Ok(String::from_utf8_lossy(&tail[..end]).into_owned())
}

fn half(d: &[u8], at: usize) -> Result<f32, String> {
    let b = d.get(at..at + 2).ok_or("face: read past the end")?;
    Ok(half_to_f32(u16::from_le_bytes([b[0], b[1]])))
}

/// An IEEE half float.
fn half_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((h >> 10) & 0x1f) as i32;
    let man = (h & 0x3ff) as f32;
    sign * match exp {
        0 => man * 2f32.powi(-24),
        31 => f32::INFINITY,
        e => (1.0 + man / 1024.0) * 2f32.powi(e - 15),
    }
}

/// One mesh flex as stored: the flex (`MapFace::flexes` index) and its
/// vertices by their index in the model's vertex list (`.vvd` order, as
/// `MapMesh::source_vertices`), offset (Source units) and side.
pub struct StoredMorph {
    pub flex: usize,
    pub vertices: Vec<(u32, Vec3, f32)>,
}

/// The model's flexes: its `MapFace` (none without descriptors) and every
/// mesh flex's vertices.
pub fn flexes(data: &[u8]) -> Result<(Option<MapFace>, Vec<StoredMorph>), String> {
    check(data)?;
    if u32::from_le_bytes(data.get(152..156).ok_or("face: header")?.try_into().unwrap()) & 0x4000 != 0 {
        return Err("face: pre-converted fixed-point flexes".into());
    }
    let mut face = MapFace::default();
    let n = count(data, 260, 1024)?;
    let base = i32le(data, 264)?.max(0) as usize;
    for i in 0..n {
        let at = base + i * 4;
        face.descriptors.push(string(data, relative(data, at, at)?)?);
    }
    let n = count(data, 268, 1024)?;
    let base = i32le(data, 272)?.max(0) as usize;
    for i in 0..n {
        let at = base + i * 20;
        face.controllers.push(MapFlexController {
            name: string(data, relative(data, at, at + 4)?)?,
            min: f32le(data, at + 12)?,
            max: f32le(data, at + 16)?,
        });
    }
    let n = count(data, 276, 4096)?;
    let base = i32le(data, 280)?.max(0) as usize;
    for i in 0..n {
        let at = base + i * 12;
        let descriptor = i32le(data, at)?.max(0) as usize;
        let ops = count(data, at + 4, 1024)?;
        let start = relative(data, at, at + 8)?;
        let ops = (0..ops)
            .map(|j| Ok((i32le(data, start + j * 8)?, i32le(data, start + j * 8 + 4)?)))
            .collect::<Result<Vec<_>, String>>()?;
        face.rules.push(MapFlexRule { descriptor, ops });
    }
    let mut morphs = Vec::new();
    let bodies = count(data, 232, 256)?;
    let start = i32le(data, 236)?.max(0) as usize;
    for body in 0..bodies {
        let b = start + body * 16;
        let models = count(data, b + 4, 256)?;
        let models_at = relative(data, b, b + 12)?;
        for m in 0..models {
            let mat = models_at + m * 148;
            let model_base = (i32le(data, mat + 84)?.max(0) / 48) as u32;
            let meshes = count(data, mat + 72, 4096)?;
            let mesh_start = relative(data, mat, mat + 76)?;
            for mesh in 0..meshes {
                let at = mesh_start + mesh * 116;
                let mesh_base = i32le(data, at + 12)?.max(0) as u32;
                let n = count(data, at + 16, 4096)?;
                if n == 0 {
                    continue;
                }
                let flex_start = relative(data, at, at + 20)?;
                for f in 0..n {
                    let fat = flex_start + f * 60;
                    let descriptor = i32le(data, fat)?.max(0) as usize;
                    let pair = i32le(data, fat + 28)?.max(0) as usize;
                    if descriptor >= face.descriptors.len() {
                        return Err("face: a mesh flex's descriptor is outside the table".into());
                    }
                    let verts = count(data, fat + 20, 1 << 20)?;
                    let vat = relative(data, fat, fat + 24)?;
                    let stride = match data.get(fat + 32) {
                        Some(0) => 16,
                        Some(1) => 18,
                        _ => return Err("face: unknown vertex animation type".into()),
                    };
                    let targets = [f32le(data, fat + 4)?, f32le(data, fat + 8)?, f32le(data, fat + 12)?, f32le(data, fat + 16)?];
                    let flex = face.flexes.len();
                    face.flexes.push(MapFlex { descriptor, pair, targets });
                    let vertices = (0..verts)
                        .map(|v| {
                            let at = vat + v * stride;
                            let index = u16::from_le_bytes(data.get(at..at + 2).ok_or("face: vertex")?.try_into().unwrap());
                            let side = *data.get(at + 3).ok_or("face: vertex")? as f32 / 255.0;
                            let delta = Vec3::new(half(data, at + 4)?, half(data, at + 6)?, half(data, at + 8)?);
                            Ok((model_base + mesh_base + index as u32, delta, side))
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    morphs.push(StoredMorph { flex, vertices });
                }
            }
        }
    }
    // The FACS lids of the eyeballs.
    face.eyelids = eyeball_lids(data)?;
    Ok(((!face.descriptors.is_empty()).then_some(face), morphs))
}

/// Each eyeball's lids (those with FACS lids).
fn eyeball_lids(data: &[u8]) -> Result<Vec<MapEyelids>, String> {
    let bodies = count(data, 232, 256)?;
    let start = i32le(data, 236)?.max(0) as usize;
    let mut out = Vec::new();
    for body in 0..bodies {
        let b = start + body * 16;
        let models = count(data, b + 4, 256)?;
        let models_at = relative(data, b, b + 12)?;
        for m in 0..models {
            let m = models_at + m * 148;
            let eyes = count(data, m + 100, 32)?;
            let eye_start = relative(data, m, m + 104)?;
            for k in 0..eyes {
                let at = eye_start + k * 172;
                if data.get(at + 140).copied().unwrap_or(1) != 0 {
                    continue;
                }
                let ints = |o: usize| -> Result<[i32; 3], String> { Ok([i32le(data, o)?, i32le(data, o + 4)?, i32le(data, o + 8)?]) };
                let floats = |o: usize| -> Result<[f32; 3], String> { Ok([f32le(data, o)?, f32le(data, o + 4)?, f32le(data, o + 8)?]) };
                out.push(MapEyelids {
                    radius: f32le(data, at + 24)?,
                    up: vec3(data, at + 28)?,
                    forward: vec3(data, at + 40)?,
                    upper_flex: ints(at + 68)?,
                    lower_flex: ints(at + 80)?,
                    upper_target: floats(at + 92)?,
                    lower_target: floats(at + 104)?,
                    upper_lid: i32le(data, at + 116)?,
                    lower_lid: i32le(data, at + 120)?,
                });
            }
        }
    }
    Ok(out)
}

/// The model's mesh flexes on its converted meshes (`MapMesh::morphs`):
/// each output vertex that is one of a flex's model vertices
/// (`source_vertices`) takes its offset, in the mesh's space by
/// `to_mesh` (the converter's axes, units and turn).
pub fn dress_morphs(morphs: &[StoredMorph], meshes: &mut [MapMesh], to_mesh: &dyn Fn(Vec3) -> Vec3) {
    for mesh in meshes.iter_mut() {
        let mut at: std::collections::BTreeMap<u32, Vec<u32>> = std::collections::BTreeMap::new();
        for (i, v) in mesh.source_vertices.iter().enumerate() {
            at.entry(*v).or_default().push(i as u32);
        }
        for m in morphs {
            let mut out = MapMorph { flex: m.flex as u16, ..Default::default() };
            for (v, delta, side) in &m.vertices {
                for i in at.get(v).into_iter().flatten() {
                    out.vertices.push(*i);
                    out.deltas.push(to_mesh(*delta).to_array());
                    out.sides.push(*side);
                }
            }
            if !out.vertices.is_empty() {
                mesh.morphs.push(out);
            }
        }
    }
}

fn remap_clamped(value: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    if a == b {
        return if value >= b { d } else { c };
    }
    let t = ((value - a) / (b - a)).clamp(0.0, 1.0);
    c + (d - c) * t
}

/// The descriptors' weights from the controllers' values (`values`, one
/// per `face.controllers`, in their own ranges): the face's rules
/// (Source's flex rule program).
pub fn run_rules(face: &MapFace, values: &[f32]) -> Vec<f32> {
    let mut dest = vec![0.0; face.descriptors.len()];
    let value = |i: i32| -> Option<f32> { usize::try_from(i).ok().and_then(|i| values.get(i).copied()) };
    let controller = |i: i32| usize::try_from(i).ok().and_then(|i| face.controllers.get(i));
    for rule in &face.rules {
        if rule.descriptor >= dest.len() {
            continue;
        }
        let mut stack = [0f32; 32];
        let mut k = 0usize;
        for &(op, data) in &rule.ops {
            match op {
                4..=7 | 13 | 14 if k >= 2 => {
                    let (a, b) = (stack[k - 2], stack[k - 1]);
                    stack[k - 2] = match op {
                        4 => a + b,
                        5 => a - b,
                        6 => a * b,
                        7 => {
                            if b > 0.0001 {
                                a / b
                            } else {
                                0.0
                            }
                        }
                        13 => a.max(b),
                        _ => a.min(b),
                    };
                    k -= 1;
                }
                8 if k >= 1 => stack[k - 1] = -stack[k - 1],
                1 if k < 32 => {
                    stack[k] = f32::from_bits(data as u32);
                    k += 1;
                }
                2 if k < 32 => {
                    if let Some(v) = value(data) {
                        stack[k] = v;
                        k += 1;
                    }
                }
                3 if k < 32 => {
                    if let Some(v) = usize::try_from(data).ok().and_then(|i| dest.get(i)) {
                        stack[k] = *v;
                        k += 1;
                    }
                }
                15 | 16 if k < 32 => {
                    if let Some(v) = value(data) {
                        stack[k] = if op == 15 { remap_clamped(v, -1.0, 0.0, 1.0, 0.0) } else { remap_clamped(v, 0.0, 1.0, 0.0, 1.0) };
                        k += 1;
                    }
                }
                18 => {
                    let m = data.max(0) as usize;
                    if m <= k && m > 0 {
                        let km = k - m;
                        for i in km + 1..k {
                            stack[km] *= stack[i];
                        }
                        k = km + 1;
                    }
                }
                19 => {
                    let m = data.max(0) as usize;
                    if k > m && m > 0 {
                        let km = k - m;
                        let dv: f32 = stack[km..k].iter().product();
                        stack[km - 1] *= 1.0 - dv;
                        k -= m;
                    }
                }
                17 if k >= 5 => {
                    let selected = stack[k - 1] as i32;
                    if let (Some(mut v), Some(scale)) = (value(selected), value(data)) {
                        let (x, y, z, w) = (stack[k - 5], stack[k - 4], stack[k - 3], stack[k - 2]);
                        v = if v <= x || v >= w {
                            0.0
                        } else if v < y {
                            remap_clamped(v, x, y, 0.0, 1.0)
                        } else if v > z {
                            remap_clamped(v, z, w, 1.0, 0.0)
                        } else {
                            1.0
                        };
                        stack[k - 5] = v * scale;
                        k -= 4;
                    }
                }
                20 | 21 if k >= 3 => {
                    let (Some(close_c), Some(close)) = (controller(data), controller(stack[k - 1] as i32)) else {
                        continue;
                    };
                    let close_v = remap_clamped(value(data).unwrap_or(0.0), close_c.min, close_c.max, 0.0, 1.0);
                    let close = remap_clamped(value(stack[k - 1] as i32).unwrap_or(0.0), close.min, close.max, 0.0, 1.0);
                    let up_down = stack[k - 3] as i32;
                    let eye = controller(up_down).map_or(0.0, |c| remap_clamped(value(up_down).unwrap_or(0.0), c.min, c.max, -1.0, 1.0));
                    stack[k - 3] = if op == 20 {
                        if eye > 0.0 {
                            (1.0 - eye) * (1.0 - close_v) * close
                        } else {
                            (1.0 - close_v) * close
                        }
                    } else if eye < 0.0 {
                        (1.0 + eye) * close_v * close
                    } else {
                        close_v * close
                    };
                    k -= 2;
                }
                _ => {}
            }
        }
        dest[rule.descriptor] = stack[0];
    }
    dest
}

/// The FACS lids of each eyeball into `weights` (descriptor weights): a
/// lid's height is the weighted asin of its raiser, neutral and lowerer
/// heights, its descriptor the lid point's height along the eye's up, the
/// eyes looking along their rest forward.
pub fn eyelids(face: &MapFace, weights: &mut [f32]) {
    for lids in &face.eyelids {
        let r = lids.radius;
        let (forward, up) = (lids.forward.normalize_or(Vec3::X), lids.up.normalize_or(Vec3::Z));
        let angle = |flex: &[i32; 3], target: &[f32; 3], weights: &[f32]| -> f32 {
            (0..3)
                .map(|i| {
                    let w = usize::try_from(flex[i]).ok().and_then(|d| weights.get(d)).copied().unwrap_or(0.0);
                    (target[i] / r).clamp(-1.0, 1.0).asin() * w
                })
                .sum()
        };
        let upper = angle(&lids.upper_flex, &lids.upper_target, weights);
        let lower = angle(&lids.lower_flex, &lids.lower_target, weights);
        for (lid, a) in [(lids.upper_lid, upper), (lids.lower_lid, lower)] {
            if let Some(w) = usize::try_from(lid).ok().and_then(|d| weights.get_mut(d)) {
                *w = up.dot(forward * (r * a.cos()) + up * (r * a.sin()));
            }
        }
    }
}

/// Each mesh flex's weight on its own side and its pair's from the
/// descriptors' weights (its ramp over `targets`).
pub fn flex_weights(face: &MapFace, descriptors: &[f32]) -> Vec<[f32; 2]> {
    let ramp = |f: &MapFlex, v: f32| {
        let [t0, t1, t2, t3] = f.targets;
        if !(t0 < v && v < t3) {
            0.0
        } else if v < t1 {
            (v - t0) / (t1 - t0)
        } else if v > t2 {
            (t3 - v) / (t3 - t2)
        } else {
            1.0
        }
    };
    face.flexes
        .iter()
        .map(|f| {
            let w1 = ramp(f, descriptors.get(f.descriptor).copied().unwrap_or(0.0));
            let w3 = if f.pair != 0 { ramp(f, descriptors.get(f.pair).copied().unwrap_or(0.0)) } else { w1 };
            [w1, w3]
        })
        .collect()
}
