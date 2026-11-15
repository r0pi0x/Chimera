//! The sky tag (`sky `; specs/halo_ce/scenario.md, "The sky tag"): the sky
//! model and animation graph references, the ambient radiosity colours and
//! powers, the outdoor and indoor fog, and the lights block (the sun with
//! its direction, colour and lens flare), as stored. `Fog::factor` and
//! `SkyLight::direction` are the spec's two formulas on these fields.

use std::io;

use crate::cache::{MapFile, RealRgb, TagEntry, TagRef, TagStruct, class};

use super::BlockCount;

/// The sky tag's main struct, in bytes.
pub const SKY_SIZE: usize = 0xD0;

crate::tag_struct! {
    /// A fog (32 bytes).
    pub struct Fog: 32 {
        0x00 color: RealRgb,
        /// 0..1: the share of the fog colour at full fog.
        0x14 max_density: f32,
        /// World units: no fog nearer than this.
        0x18 start_distance: f32,
        /// World units: full density at and beyond this; 0 means no fog.
        0x1C opaque_distance: f32,
    }
}

nested_field!(Fog);

impl Fog {
    /// The fog factor of a point at `depth` along the camera's forward
    /// axis: `clamp((depth - start) / (opaque - start), 0, 1) x density`.
    /// A density of 0 or less counts as 1; an opaque distance of 0 (or
    /// less) switches fog off (factor 0); an opaque distance below the
    /// start is raised to it.
    pub fn factor(&self, depth: f32) -> f32 {
        if self.opaque_distance <= 0.0 {
            return 0.0;
        }
        let density = if self.max_density <= 0.0 { 1.0 } else { self.max_density };
        let start = self.start_distance;
        let opaque = self.opaque_distance.max(start);
        let t = if opaque > start {
            ((depth - start) / (opaque - start)).clamp(0.0, 1.0)
        } else if depth >= start {
            1.0
        } else {
            0.0
        };
        t * density
    }
}

crate::tag_struct! {
    /// A sky light (116 bytes).
    pub struct SkyLight: 0x74 {
        /// Class `lens`; null on Boarding Action's second light.
        0x00 lens_flare: TagRef,
        /// A marker on the sky model giving the direction; empty on every
        /// disc light.
        0x10 marker_name: String,
        /// Bit 0 affects exteriors, bit 1 affects interiors (names
        /// inferred).
        0x4C flags: u32,
        0x50 color: RealRgb,
        /// Multiplier of the colour.
        0x5C power: f32,
        0x60 test_distance: f32,
        /// Radians.
        0x68 direction_yaw: f32,
        /// Radians, positive up.
        0x6C direction_pitch: f32,
        /// Radians: the sun's angular diameter.
        0x70 diameter: f32,
    }
}

impl SkyLight {
    /// The direction from the viewer toward the light: `(cos yaw cos
    /// pitch, sin yaw cos pitch, sin pitch)`.
    pub fn direction(&self) -> [f32; 3] {
        let (sy, cy) = self.direction_yaw.sin_cos();
        let (sp, cp) = self.direction_pitch.sin_cos();
        [cy * cp, sy * cp, sp]
    }
}

crate::tag_struct! {
    /// A sky animation (36 bytes; none on the disc).
    pub struct SkyAnimation: 36 {
        /// Index into the animation graph.
        0x00 animation_index: i16,
        /// Seconds.
        0x04 period: f32,
    }
}

crate::tag_struct! {
    /// The sky tag's main struct (0xD0 bytes).
    pub struct Sky: 0xD0 {
        /// Class `mode`; null on Chill Out's sky.
        0x00 model: TagRef,
        /// Class `antr`; null on every disc sky.
        0x10 animation_graph: TagRef,
        0x38 indoor_ambient_color: RealRgb,
        0x44 indoor_ambient_power: f32,
        0x48 outdoor_ambient_color: RealRgb,
        0x54 outdoor_ambient_power: f32,
        0x58 outdoor_fog: Fog,
        0x78 indoor_fog: Fog,
        /// Class `fog `; null on every disc sky.
        0x98 indoor_fog_screen: TagRef,
        /// Element size 36, all unused; count only (0 on the disc).
        0xAC shader_functions: BlockCount,
        0xB8 animations: Vec<SkyAnimation>,
        0xC4 lights: Vec<SkyLight>,
    }
}

impl Sky {
    /// Read the `sky ` tag `tag` of `map`.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Sky> {
        let v = map.tag_view(tag, SKY_SIZE)?;
        if tag.class != class::SKY {
            return Err(v.error(0, format_args!("tag is {} `{}`, not a sky", tag.class, tag.path)));
        }
        <Sky as TagStruct>::read(&v)
    }
}
