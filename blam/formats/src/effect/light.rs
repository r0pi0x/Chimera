//! The light `ligh` (0x160 bytes), the decal `deca` (0x10C), material
//! effects `foot` (0x0C) and the contrail `cont` (0x144)
//! (specs/halo_ce/effects.md, "Lights", "Decals", "Material effects",
//! "Contrails"): the fields the weapons' muzzle lights, bullet holes,
//! casings' sounds and tracers read.

use std::io;

use crate::cache::{Bounds, MapFile, RealArgb, RealRgb, TagEntry, TagRef, TagStruct, View, class};
use crate::object::check_class;

use super::{EffectShader, cap};

crate::tag_struct! {
    /// A light (`ligh`).
    pub struct Light: 0x160 {
        /// Bit 0 dynamic; 1 no specular; 2 don't light own object; 3
        /// supersize in first person; 4 first-person flashlight; 5 don't fade
        /// with active camouflage.
        0x000 flags: u32,
        /// WU.
        0x004 radius: f32,
        /// Radius factor at intensity 0 and 1.
        0x008 radius_modifier: Bounds<f32>,
        0x010 falloff_angle: f32,
        0x014 cutoff_angle: f32,
        /// Bit 0 HSV; bit 1 the long hue path.
        0x034 color_flags: u32,
        /// At intensity 0 and 1.
        0x038 color_lower: RealArgb,
        0x048 color_upper: RealArgb,
        0x0AC lens_flare: TagRef,
        /// Effect lights' lifetime, in **ticks**.
        0x0F4 duration: f32,
        /// A transition function over the lifetime.
        0x0FA falloff_function: i16,
    }
}

impl Light {
    pub const DYNAMIC: u32 = 1;

    /// Read a `ligh` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Light> {
        let v = check_class(map, tag, class::LIGHT, "light", 0x160)?;
        <Light as TagStruct>::read(&v)
    }

    /// Colour (RGB) and radius (WU) at intensity `i` (0..1), interpolated in
    /// RGB (the HSV flags are not followed: no tag of the four weapons
    /// sets them).
    pub fn at(&self, i: f32) -> ([f32; 3], f32) {
        let (a, b) = (self.color_lower, self.color_upper);
        let lerp = |x: f32, y: f32| x + (y - x) * i;
        let radius = (self.radius_modifier.lower * (1.0 - i) + self.radius_modifier.upper * i) * self.radius;
        ([lerp(a.r, b.r), lerp(a.g, b.g), lerp(a.b, b.b)], radius)
    }
}

crate::tag_struct! {
    /// A decal (`deca`).
    pub struct Decal: 0x10C {
        /// Bit 0 geometry inherited by the next in chain; 3 no random
        /// rotation; 4 water effect; 8 preserve aspect.
        0x000 flags: u16,
        /// 0 scratch, 1 splatter, 2 burn, 3 painted sign.
        0x002 kind: i16,
        /// 0 primary, 1 secondary, 2 light, 3 alpha-tested, 4 water.
        0x004 layer: i16,
        0x008 next: TagRef,
        /// WU, half size.
        0x018 radius: Bounds<f32>,
        /// The colour's alpha.
        0x02C intensity: Bounds<f32>,
        0x034 color_lower: RealRgb,
        0x040 color_upper: RealRgb,
        /// Seconds; 0 = permanent.
        0x078 lifetime: Bounds<f32>,
        /// Seconds of fade at the end.
        0x080 decay: Bounds<f32>,
        /// `shaders.md`: 1 multiply, 2 double multiply, 3 add, 6 component max.
        0x0C0 blend: i16,
        0x0D8 map: TagRef,
    }
}

impl Decal {
    pub const MULTIPLY: i16 = 1;
    pub const DOUBLE_MULTIPLY: i16 = 2;
    pub const ADD: i16 = 3;

    /// Read a `deca` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Decal> {
        let v = check_class(map, tag, class::DECAL, "decal", 0x10C)?;
        <Decal as TagStruct>::read(&v)
    }

    /// The intensity byte at `elapsed` seconds of a decal living
    /// `lifetime` with `decay` seconds of fade (None: gone; the spec's
    /// "Lifetime").
    pub fn intensity_at(lifetime: f32, decay: f32, elapsed: f32) -> Option<u8> {
        if lifetime > 0.0 && elapsed >= lifetime {
            return None;
        }
        if lifetime > 0.0 && decay > 0.0 && lifetime - elapsed < decay {
            return Some(((lifetime - elapsed) / decay * 255.0) as u8);
        }
        Some(255)
    }
}

crate::tag_struct! {
    /// One material of a kind (element 0x30): its effect and sound.
    pub struct MaterialEffect: 0x30 {
        0x00 effect: TagRef,
        0x10 sound: TagRef,
    }
}

crate::tag_struct! {
    struct Kind: 0x1C {
        0x00 materials: Vec<MaterialEffect>,
    }
}

crate::tag_struct! {
    struct RawMaterialEffects: 0x0C {
        0x00 kinds: Vec<Kind>,
    }
}

/// Material effects (`foot`): per kind (0 walk ... 8 impact ...), per
/// material type in code order, an effect and a sound.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialEffects {
    pub kinds: Vec<Vec<MaterialEffect>>,
}

impl MaterialEffects {
    /// The impact kind (items, particles hitting the level).
    pub const IMPACT: usize = 8;

    /// Read a `foot` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<MaterialEffects> {
        let v = check_class(map, tag, class::MATERIAL_EFFECTS, "material effects", 0x0C)?;
        let raw = <RawMaterialEffects as TagStruct>::read(&v)?;
        let kinds = cap(&v, 0, raw.kinds, 13, "kinds")?;
        let kinds = kinds
            .into_iter()
            .map(|k| cap(&v, 0, k.materials, 33, "materials"))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(MaterialEffects { kinds })
    }

    /// Kind `kind`'s element for material type `material`.
    pub fn get(&self, kind: usize, material: usize) -> Option<&MaterialEffect> {
        self.kinds.get(kind)?.get(material)
    }
}

crate::tag_struct! {
    /// A contrail point state (element 0x68).
    pub struct PointState: 0x68 {
        /// Seconds.
        0x00 duration: Bounds<f32>,
        0x08 transition_duration: Bounds<f32>,
        0x10 physics: TagRef,
        /// WU, full width.
        0x40 width: f32,
        0x44 color_lower: RealArgb,
        0x54 color_upper: RealArgb,
        0x64 scale_flags: u32,
    }
}

crate::tag_struct! {
    struct RawContrail: 0x144 {
        0x000 flags: u16,
        0x002 scale_flags: u16,
        0x004 point_rate: f32,
        0x008 point_velocity: Bounds<f32>,
        0x010 velocity_cone: f32,
        0x014 inherited_velocity: f32,
        0x018 render_type: i16,
        0x01C repeats_u: f32,
        0x020 repeats_v: f32,
        0x024 animation_u: f32,
        0x028 animation_v: f32,
        0x02C animation_rate: f32,
        0x030 bitmap: TagRef,
        0x040 first_sequence: i16,
        0x042 sequence_count: i16,
        0x138 point_states: Vec<PointState>,
    }
}

/// A contrail (`cont`): a ribbon through points emitted behind an object.
#[derive(Clone, Debug, PartialEq)]
pub struct Contrail {
    /// Bit 0 first point unfaded; bit 1 last point unfaded; bit 6 fades slowly.
    pub flags: u16,
    pub scale_flags: u16,
    /// Points per second.
    pub point_rate: f32,
    /// WU per second.
    pub point_velocity: Bounds<f32>,
    pub velocity_cone: f32,
    pub inherited_velocity: f32,
    /// 0 vertical, 1 horizontal, 2 media mapped, 3 ground mapped, 4 viewer facing.
    pub render_type: i16,
    pub repeats_u: f32,
    pub repeats_v: f32,
    pub animation_u: f32,
    pub animation_v: f32,
    pub animation_rate: f32,
    pub bitmap: TagRef,
    pub first_sequence: i16,
    pub sequence_count: i16,
    pub shader: EffectShader,
    pub point_states: Vec<PointState>,
}

impl Contrail {
    /// Read a `cont` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Contrail> {
        let v = check_class(map, tag, class::CONTRAIL, "contrail", 0x144)?;
        Self::read_view(&v)
    }

    pub fn read_view(v: &View<'_>) -> io::Result<Contrail> {
        let n = v.i32(0x138)?;
        if !(0..=16).contains(&n) {
            return Err(v.error(0x138, format_args!("{n} point states outside 0..=16")));
        }
        let r = <RawContrail as TagStruct>::read(v)?;
        Ok(Contrail {
            flags: r.flags,
            scale_flags: r.scale_flags,
            point_rate: r.point_rate,
            point_velocity: r.point_velocity,
            velocity_cone: r.velocity_cone,
            inherited_velocity: r.inherited_velocity,
            render_type: r.render_type,
            repeats_u: r.repeats_u,
            repeats_v: r.repeats_v,
            animation_u: r.animation_u,
            animation_v: r.animation_v,
            animation_rate: r.animation_rate,
            bitmap: r.bitmap,
            first_sequence: r.first_sequence,
            sequence_count: r.sequence_count,
            shader: EffectShader {
                flags: v.u16(0x84 + 0x28)?,
                blend: v.i16(0x84 + 0x2A)?,
                fade_mode: v.i16(0x84 + 0x2C)?,
                map_flags: v.u16(0x84 + 0x2E)?,
            },
            point_states: r.point_states,
        })
    }
}
