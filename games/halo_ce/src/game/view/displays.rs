//! The displays on Halo's guns in first person (specs/halo_ce/weapons.md
//! 18, shaders.md "Numeric shaders", "Meter" and "Animation sources",
//! objects.md "Object functions" and "Function input sources"): the
//! assault rifle's round counter and compass, the plasma weapons' gauges,
//! the flamethrower's bars. One mechanism serves all, read from the tags;
//! no gun is named here.
//!
//! **At load** (`dress`, from `load::looks`): each part of a gun's
//! first-person model whose shader is a numeric chicago or generic shader
//! (flag 7, at least one map, no countdown timer) gets every image of its
//! map 0's bitmap as its texture's frames (`MapMesh::frames`), showing the
//! default image (its shader permutation modulo the image count) until the
//! game picks; a meter shader's part gets its meter map baked at
//! `METER_LEVELS` + 1 cuts (`meter_frames`: frame f lit where the texel's
//! blue is below f / `METER_LEVELS`, the rest the background; added).
//! The gun's program (`GunProgram`): its weapon tag's export sources (part
//! 0x028), its object layer's input sources (0x108), its object functions
//! and each display part's rule, kept as the map's memo (`GunDisplays`, by
//! view model key, with the scenario's local north: 0 on another game's
//! map).
//!
//! **Each Halo tick** (`view::drive`, after the weapons' tick): the held
//! weapon's incoming values (`step`: the object layer's sources first,
//! then the weapon's exports where it names one: `exports`), its four
//! outgoing functions (`blam_formats::object::evaluate_all`), and each
//! display part's frame (`frames`), written to the holder's
//! `ViewAnimator::mesh_frames`; the drawing swaps the part's texture. So a
//! picture changes at most once a tick, on the tick of a shot or a reload's
//! last tick, and holds between ticks.
//!
//! - Numeric (`numeric_image`): value = clamp(floor(limit x source + 0.5),
//!   0, limit) in single precision with source function D when map 0 has
//!   exactly 8 images (the compass), else A; divided by the image count
//!   once per digit position; the image is what is left modulo the count.
//! - Meter (`meter_frame`): the value source's function (1 to 4: A to D,
//!   0: 1) rounded to the nearest of the baked cuts.
//! - Compass input (`compass`): the bearing of the holder's aim from
//!   local north, frozen within about 5.7 degrees of vertical.
//!
//! Known shortcuts:
//! - First person only: the third-person held models keep their first
//!   image (other players' weapon state is not on a client: only the own
//!   player's inventory is sent).
//! - Meters are baked at 16 levels (a cut moves in steps of 1/16 of the
//!   bar), drawn added: the background is added too, as the game's blend
//!   adds it, but the frame under the meter is not multiplied by the tint
//!   colour; the brightness, flash, gradient and flash-extension sources
//!   read 1 (none on the disc's weapon meters but the value source).
//! - The compass reads the holder's aim (yaw and pitch), not the weapon's
//!   node 0 as attached to the hand marker.
//! - Exports not modelled read 0: secondary ammunition and rate of fire,
//!   ejection ports, illumination, integrated light; the object layer's
//!   vitality sources read a whole weapon (1), recent damage 0.
//! - A numeric shader with the countdown timer flag is left static (no
//!   global countdown timer); the needler's world-model needle strip (a
//!   texture slide by function C) is not drawn.

use std::collections::BTreeMap;

use blam_formats::{
    bitmap::Bitmap,
    cache::{MapFile, TagRef},
    object::function::{FunctionOutput, ObjectFunction, evaluate_all, object_time},
    shader::{ChicagoShader, MeterShader, PeriodicTables, Shader, TransparentCommon},
    weapon::Weapon,
};

use super::super::weapons::{params::Gun, tick::HaloGun};
use crate::map::{MapAlpha, MapMesh, MapTexture};

/// The cuts a meter is baked at (`meter_frames`): frames 0 to this.
pub const METER_LEVELS: usize = 16;

/// How a display part turns the weapon's functions into its frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    /// A numeric shader (`numeric_image`): its counter limit, the part's
    /// digit position (its shader permutation) and map 0's image count.
    Numeric { limit: u8, position: u16, images: u16 },
    /// A meter (`meter_frame`): its value source (0 none, 1 to 4 A to D).
    Meter { source: i16 },
}

/// A gun's displays and what drives them (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GunProgram {
    /// The weapon layer's export sources A to D (`WeaponInput` codes).
    pub exports: [i16; 4],
    /// The object layer's input sources A to D (`FunctionInput` codes).
    pub inputs: [i16; 4],
    /// Its object functions (at most four used).
    pub functions: Vec<ObjectFunction>,
    /// Each display part: its mesh index in the view model and its rule.
    pub parts: Vec<(usize, Display)>,
}

/// The map's guns' programs by view model key, and the local north
/// (radians) the compass reads (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GunDisplays {
    pub guns: BTreeMap<String, GunProgram>,
    pub local_north: f32,
}

/// A held weapon's function state: its incoming values and outgoing
/// functions as the last tick left them, and its function clock (ticks).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Functions {
    pub incoming: [f32; 4],
    pub outgoing: [FunctionOutput; 4],
    pub ticks: u32,
}

/// The periodic tables the functions read (made once).
pub fn tables() -> &'static PeriodicTables {
    static TABLES: std::sync::OnceLock<PeriodicTables> = std::sync::OnceLock::new();
    TABLES.get_or_init(PeriodicTables::new)
}

/// The image a numeric shader's part shows (module doc): `limit` the
/// counter limit, `images` map 0's image count, `position` the digit
/// position, `a` and `d` the object's functions A and D.
pub fn numeric_image(limit: u8, images: u16, position: u16, a: f32, d: f32) -> usize {
    let n = u32::from(images);
    if n == 0 {
        return 0;
    }
    let source = if n == 8 { d } else { a };
    let limit = f32::from(limit);
    let mut value = (limit * source + 0.5).floor().clamp(0.0, limit) as u32;
    for _ in 0..position {
        value /= n;
        if value == 0 {
            break;
        }
    }
    (value % n) as usize
}

/// The baked cut nearest the meter value `x` (module doc).
pub fn meter_frame(x: f32) -> usize {
    (x.clamp(0.0, 1.0) * METER_LEVELS as f32).round() as usize
}

/// An animation source's value (shaders.md "Animation sources"): 1 to 4
/// the outgoing function A to D as stored, else 1.
pub fn source_value(source: i16, outgoing: &[FunctionOutput; 4]) -> f32 {
    match source {
        1..=4 => outgoing[(source - 1) as usize].value,
        _ => 1.0,
    }
}

/// The compass input (objects.md "Compass") for a forward axis `f` in
/// Halo's axes and local north `north` (radians); None within about 5.7
/// degrees of vertical (the input keeps its value).
pub fn compass(f: [f32; 3], north: f32) -> Option<f32> {
    use std::f32::consts::PI;
    if f[2].abs() >= 0.995 {
        return None;
    }
    let mut a = f[0].atan2(f[1]) - north;
    if a >= PI {
        a -= 2.0 * PI;
    }
    if a <= -PI {
        a += 2.0 * PI;
    }
    Some((0.5 + a * 0.159_154_94).clamp(0.0, 1.0))
}

/// The forward axis (Halo's axes) of an aim: heading `yaw` (the engine's
/// `Intent::yaw` is Halo's heading, `maps::space`) and `pitch`.
pub fn aim_forward(yaw: f32, pitch: f32) -> [f32; 3] {
    let c = pitch.cos();
    [c * yaw.cos(), c * yaw.sin(), pitch.sin()]
}

/// One weapon export value (weapons.md 17; `WeaponInput` code `code`),
/// None for 0 (the slot is left alone).
pub fn export(code: i16, gun: &Gun, state: &HaloGun) -> Option<f32> {
    Some(match code {
        0 => return None,
        // Heat.
        1 => state.heat,
        // Primary ammunition: loaded / the magazine's size.
        2 => match gun.magazine.as_ref().map(|m| m.rounds_loaded_maximum) {
            Some(size) if size > 0 => f32::from(state.loaded) / f32::from(size),
            _ => 0.0,
        },
        // Primary rate of fire.
        4 => state.rate_of_fire,
        // Ready.
        6 => 1.0,
        // Overheated.
        9 if state.overheated => {
            let t = gun.heat.recovery_threshold;
            if t < 1.0 { (state.heat - t) / (1.0 - t) } else { 0.0 }
        }
        // Primary charged.
        10 => state.charge(gun),
        // Age.
        13 => state.age,
        // Primary firing: the rate of fire on the tick of a shot.
        15 if state.idle_ticks == 0 => state.rate_of_fire,
        _ => 0.0,
    })
}

/// One tick of a held weapon's functions (module doc): the object layer's
/// inputs (`compass_value`: this tick's compass, None to keep), then the
/// weapon's exports, then the four functions.
pub fn step(
    program: &GunProgram,
    exports: &dyn Fn(i16) -> Option<f32>,
    compass_value: Option<f32>,
    tables: &PeriodicTables,
    state: &mut Functions,
) {
    for (slot, &code) in program.inputs.iter().enumerate() {
        let v = match code {
            // Body and shield vitality: a whole weapon.
            1 | 2 => Some(1.0),
            3 | 4 => Some(0.0),
            18 => Some(1.0),
            19 => compass_value,
            _ => None,
        };
        if let Some(v) = v {
            state.incoming[slot] = v;
        }
    }
    for (slot, &code) in program.exports.iter().enumerate() {
        if let Some(v) = exports(code) {
            state.incoming[slot] = v;
        }
    }
    let time = object_time(0, state.ticks);
    state.outgoing = evaluate_all(&program.functions, tables, time, &state.incoming, state.outgoing);
    state.ticks = state.ticks.wrapping_add(1);
}

/// Each display part's frame for the functions `outgoing` (module doc).
pub fn frames(program: &GunProgram, outgoing: &[FunctionOutput; 4]) -> Vec<(usize, usize)> {
    program
        .parts
        .iter()
        .map(|&(mesh, d)| {
            let frame = match d {
                Display::Numeric { limit, position, images } => {
                    numeric_image(limit, images, position, outgoing[0].value, outgoing[3].value)
                }
                Display::Meter { source } => meter_frame(source_value(source, outgoing)),
            };
            (mesh, frame)
        })
        .collect()
}

/// A part of a gun's first-person model: its mesh index, shader and
/// shader permutation.
pub type Part = (usize, TagRef, i16);

/// One image of a bitmap as a texture (sRGB, box-filtered mip levels);
/// `decode` off: a placeholder of its size.
fn image_texture(file: &MapFile, bitmap: &Bitmap, index: usize, name: &str, decode: bool) -> Result<MapTexture, String> {
    let image = bitmap.images.get(index).ok_or_else(|| format!("{name}: no image {index}"))?;
    let (width, height) = (u32::from(image.width), u32::from(image.height));
    let mut t = MapTexture {
        name: format!("{name} (image {index})"),
        srgb: true,
        mips: Vec::new(),
        width,
        height,
        rgba8: Vec::new(),
        bc: None,
    };
    if decode {
        let d = image.decode(file).map_err(|e| format!("{name}: {e}"))?;
        let level = d.faces.first().and_then(|f| f.first()).ok_or_else(|| format!("{name}: no level"))?;
        t.rgba8 = level.rgba[..(level.width * level.height * 4) as usize].to_vec();
        t.mips = t.box_mips();
    }
    Ok(t)
}

/// A meter shader's colours and flags (shaders.md "Meter"), as the cut
/// reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeterColours {
    pub minimum: [f32; 3],
    pub maximum: [f32; 3],
    pub background: [f32; 3],
    pub flash: [f32; 3],
    pub tint: [f32; 3],
    pub tint_mode_2: bool,
    pub flash_negative: bool,
}

impl MeterColours {
    pub fn of(s: &MeterShader) -> Self {
        let rgb = |c: blam_formats::cache::RealRgb| [c.r, c.g, c.b];
        MeterColours {
            minimum: rgb(s.gradient_min_color),
            maximum: rgb(s.gradient_max_color),
            background: rgb(s.background_color),
            flash: rgb(s.flash_color),
            tint: rgb(s.tint_color),
            tint_mode_2: s.flags & MeterShader::TINT_MODE_2 != 0,
            flash_negative: s.flags & MeterShader::FLASH_COLOR_IS_NEGATIVE != 0,
        }
    }
}

/// A meter map (RGBA8) cut at value `x` (shaders.md "Meter", module doc):
/// texels with blue below `x` the gradient colour, the rest the
/// background, each times the texel's alpha and the brightness (1), with
/// the tint colour in tint mode 2; drawn added, alpha 1 (0 where the
/// texel's alpha is 0).
pub fn meter_cut(texels: &[u8], c: &MeterColours, x: f32) -> Vec<u8> {
    // The sources other than the value read 1 (module doc).
    let (beta, phi, g, e) = (1.0f32, 1.0f32, 1.0f32, 1.0f32);
    let sign = if c.flash_negative { -1.0 } else { 1.0 };
    let scale: [f32; 3] = if c.tint_mode_2 { std::array::from_fn(|i| beta * c.tint[i]) } else { [beta; 3] };
    let mut out = Vec::with_capacity(texels.len());
    for t in texels.chunks_exact(4) {
        if t[3] == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let (b, a) = (f32::from(t[2]) / 255.0, f32::from(t[3]) / 255.0);
        let colour: [f32; 3] = if b < x {
            let k = (b / g.max(0.125)).clamp(0.0, 1.0);
            let w = (1.0 - 2.0 * (4.0 * (e - b)).clamp(0.0, 1.0)).clamp(0.0, 1.0);
            std::array::from_fn(|i| c.minimum[i] + (c.maximum[i] - c.minimum[i]) * k + sign * w * phi * c.flash[i])
        } else {
            c.background
        };
        for i in 0..3 {
            out.push(((colour[i] * a * scale[i]).clamp(0.0, 1.0) * 255.0).round() as u8);
        }
        out.push(255);
    }
    out
}

/// A meter's frames (module doc): its map's first image cut at each level.
fn meter_frames(file: &MapFile, s: &MeterShader, name: &str, decode: bool) -> Result<Vec<MapTexture>, String> {
    let tag = file.tag(s.meter_map.id).ok_or_else(|| format!("{name}: no meter map"))?;
    let bitmap = Bitmap::read(file, tag).map_err(|e| format!("{name}: {e}"))?;
    let name = tag.path.replace('\\', "/");
    let base = image_texture(file, &bitmap, 0, &name, decode)?;
    Ok((0..=METER_LEVELS)
        .map(|f| {
            let mut t = MapTexture {
                name: format!("{name} (cut {f}/{METER_LEVELS})"),
                ..base.clone()
            };
            if decode {
                t.rgba8 = meter_cut(&base.rgba8, &MeterColours::of(s), f as f32 / METER_LEVELS as f32);
                t.mips = t.box_mips();
            }
            t
        })
        .collect())
}

/// A numeric shader's map 0 and counter limit, if `shader` is one that
/// picks from an object's functions (module doc).
fn numeric(shader: &Shader) -> Option<(TagRef, u8)> {
    let (common, map) = match shader {
        Shader::TransparentChicago(s) => {
            if s.extra_flags & ChicagoShader::NUMERIC_COUNTDOWN_TIMER != 0 {
                return None;
            }
            (&s.common, s.maps.first()?.map)
        }
        Shader::TransparentGeneric(s) => (&s.common, s.maps.first()?.map),
        _ => return None,
    };
    (common.flags & TransparentCommon::NUMERIC != 0).then_some((map, common.numeric_counter_limit))
}

/// The displays of a gun's first-person model (module doc): `parts` its
/// meshes' shaders, `meshes` the model's meshes with their looks applied
/// (frames and default images set here), new textures appended to
/// `textures`. None when the model has no display.
pub fn dress(
    file: &MapFile,
    weapon: &Weapon,
    parts: &[Part],
    meshes: &mut [MapMesh],
    textures: &mut Vec<MapTexture>,
    warnings: &mut Vec<String>,
    decode: bool,
) -> Option<GunProgram> {
    let mut program = GunProgram {
        exports: [weapon.own.input_a, weapon.own.input_b, weapon.own.input_c, weapon.own.input_d].map(|i| i.code()),
        inputs: weapon.object.inputs().map(|i| i.code()),
        functions: weapon.object.functions.clone(),
        parts: Vec::new(),
    };
    for &(index, shader_ref, permutation) in parts {
        let Some(mesh) = meshes.get_mut(index) else { continue };
        let Some(shader) = file.tag(shader_ref.id).and_then(|t| Shader::read(file, t).ok()) else { continue };
        let name = mesh.material.clone();
        let made = if let Some((map, limit)) = numeric(&shader) {
            let bitmap = file
                .tag(map.id)
                .ok_or_else(|| format!("{name}: no map 0"))
                .and_then(|t| Bitmap::read(file, t).map_err(|e| format!("{name}: {e}")));
            let path = file.tag(map.id).map_or_else(|| name.clone(), |t| t.path.replace('\\', "/"));
            bitmap.and_then(|b| {
                let n = b.images.len().min(usize::from(u16::MAX));
                let frames = (0..n).map(|k| image_texture(file, &b, k, &path, decode)).collect::<Result<Vec<_>, _>>()?;
                let position = u16::try_from(permutation).unwrap_or(0);
                Ok((frames, Display::Numeric { limit, position, images: n as u16 }, usize::from(position) % n.max(1)))
            })
        } else if let Shader::Meter(s) = &shader {
            meter_frames(file, s, &name, decode).map(|f| (f, Display::Meter { source: s.value_source.code() }, 0))
        } else {
            continue;
        };
        match made {
            Ok((frames, display, shown)) if !frames.is_empty() => {
                let first = textures.len();
                textures.extend(frames);
                mesh.frames = (first..textures.len()).collect();
                mesh.texture = mesh.frames.get(shown).copied();
                if matches!(display, Display::Meter { .. }) {
                    mesh.alpha = MapAlpha::Add;
                }
                program.parts.push((index, display));
            }
            Ok(_) => {}
            Err(e) => warnings.push(format!("display not drawn: {e}")),
        }
    }
    (!program.parts.is_empty()).then_some(program)
}
