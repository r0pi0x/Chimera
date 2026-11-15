//! Object placements of the scenario (specs/halo_ce/scenario.md, "Fields
//! every placement shares", "Object placements by type", "The other
//! blocks"): the shared 40-byte header (`PlacementHeader`) and each type's
//! own fields, device groups, trigger volumes and decals, and the rotation
//! convention (`Orientation::from_euler`): three angles whose yaw spins the
//! object about its own up axis first, then pitch and roll tilt about the
//! world's Y and X axes.

use crate::cache::{Euler3d, RealRgb};

/// An object's axes in world coordinates: forward F, left L and up U, unit
/// vectors, mutually perpendicular and right-handed (F x L = U). A model
/// point `p` in the object's frame lands at `position + p.x F + p.y L +
/// p.z U` (`Orientation::rotate`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orientation {
    pub forward: [f32; 3],
    pub left: [f32; 3],
    pub up: [f32; 3],
}

impl Orientation {
    /// The identity: F = +X, L = +Y, U = +Z.
    pub const IDENTITY: Orientation = Orientation {
        forward: [1.0, 0.0, 0.0],
        left: [0.0, 1.0, 0.0],
        up: [0.0, 0.0, 1.0],
    };

    /// The axes of the stored rotation (yaw, pitch, roll in radians) by the
    /// spec's table: with `cy`, `sy`, `cp`, `sp`, `cr`, `sr` the cosines and
    /// sines, F = (cy cp, sy cr - cy sp sr, cy sp cr + sy sr), L = (-sy cp,
    /// sy sp sr + cy cr, cy sr - sy sp cr), U = (-sp, -cp sr, cp cr). It is
    /// a rotation about world Z by yaw, then about world Y by -pitch, then
    /// about world X by roll, applied to column vectors right to left.
    pub fn from_euler(yaw: f32, pitch: f32, roll: f32) -> Orientation {
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let (sr, cr) = roll.sin_cos();
        Orientation {
            forward: [cy * cp, sy * cr - cy * sp * sr, cy * sp * cr + sy * sr],
            left: [-sy * cp, sy * sp * sr + cy * cr, cy * sr - sy * sp * cr],
            up: [-sp, -cp * sr, cp * cr],
        }
    }

    /// The same from a stored rotation triple.
    pub fn from_rotation(r: &Euler3d) -> Orientation {
        Self::from_euler(r.yaw, r.pitch, r.roll)
    }

    /// The 3x3 matrix with F, L, U as its columns (`m[row][column]`), taking
    /// object-frame vectors to world vectors.
    pub fn matrix(&self) -> [[f32; 3]; 3] {
        let (f, l, u) = (self.forward, self.left, self.up);
        [[f[0], l[0], u[0]], [f[1], l[1], u[1]], [f[2], l[2], u[2]]]
    }

    /// The same rotation as a unit quaternion `[i, j, k, w]` (vector part
    /// first, as the tags store quaternions), with `w >= 0`.
    pub fn quaternion(&self) -> [f32; 4] {
        let m = self.matrix();
        let trace = m[0][0] + m[1][1] + m[2][2];
        let (i, j, k, w);
        if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            w = 0.25 * s;
            i = (m[2][1] - m[1][2]) / s;
            j = (m[0][2] - m[2][0]) / s;
            k = (m[1][0] - m[0][1]) / s;
        } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
            let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
            w = (m[2][1] - m[1][2]) / s;
            i = 0.25 * s;
            j = (m[0][1] + m[1][0]) / s;
            k = (m[0][2] + m[2][0]) / s;
        } else if m[1][1] > m[2][2] {
            let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
            w = (m[0][2] - m[2][0]) / s;
            i = (m[0][1] + m[1][0]) / s;
            j = 0.25 * s;
            k = (m[1][2] + m[2][1]) / s;
        } else {
            let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
            w = (m[1][0] - m[0][1]) / s;
            i = (m[0][2] + m[2][0]) / s;
            j = (m[1][2] + m[2][1]) / s;
            k = 0.25 * s;
        }
        if w < 0.0 { [-i, -j, -k, -w] } else { [i, j, k, w] }
    }

    /// An object-frame point as a world offset: `p.x F + p.y L + p.z U`.
    pub fn rotate(&self, p: [f32; 3]) -> [f32; 3] {
        let mut o = [0.0; 3];
        for (a, o) in o.iter_mut().enumerate() {
            *o = p[0] * self.forward[a] + p[1] * self.left[a] + p[2] * self.up[a];
        }
        o
    }
}

crate::tag_struct! {
    /// The 40-byte header every placement but decals starts with.
    pub struct PlacementHeader: 0x28 {
        /// Element of the type's palette; -1 or out of range: nothing is
        /// created.
        0x00 palette_index: i16,
        /// Element of the object names block, or -1.
        0x02 name_index: i16,
        /// Bit 0: not automatic; bits 1 to 3: not on easy, normal, hard.
        0x04 flags: u16,
        /// The model variant asked for.
        0x06 variant: i16,
        /// World units; the object's origin.
        0x08 position: [f32; 3],
        /// Yaw, pitch, roll in radians (`orientation`).
        0x14 rotation: Euler3d,
        /// Bit i set: the object is in structure BSP i.
        0x20 on_bsp_flags: u16,
        /// Equipment: bit 0 levitate, bit 1 does not orient to ground.
        0x22 misc_flags: u16,
    }
}

nested_field!(PlacementHeader);

impl PlacementHeader {
    /// The object's axes (`Orientation::from_euler` of `rotation`).
    pub fn orientation(&self) -> Orientation {
        Orientation::from_rotation(&self.rotation)
    }

    /// The palette entry the placement names, if the index is in range of a
    /// palette of `palette_len` entries.
    pub fn palette_entry(&self, palette_len: usize) -> Option<usize> {
        usize::try_from(self.palette_index).ok().filter(|&i| i < palette_len)
    }

    /// Whether bit `bsp` of the on-BSP flags is set.
    pub fn in_bsp(&self, bsp: usize) -> bool {
        bsp < 16 && self.on_bsp_flags & (1 << bsp) != 0
    }

    /// The converter's rule for creating the placement: the palette index is
    /// in range of `palette_len`, and, when `bsp` is given (scenery and
    /// light fixtures: the BSP in use), that BSP's on-BSP bit is set.
    pub fn is_created(&self, palette_len: usize, bsp: Option<usize>) -> bool {
        self.palette_entry(palette_len).is_some() && bsp.is_none_or(|b| self.in_bsp(b))
    }
}

crate::tag_struct! {
    /// A scenery placement (72 bytes); the 32 bytes of permutation data
    /// after the header are zero on the disc and not read.
    pub struct Scenery: 72 {
        0x00 header: PlacementHeader,
    }
}

crate::tag_struct! {
    /// A biped or vehicle placement (120 bytes).
    pub struct UnitPlacement: 120 {
        0x00 header: PlacementHeader,
        /// Initial body health as a fraction (campaign; 0 on the disc).
        0x48 body_vitality: f32,
        /// Unit flags (campaign; 0 on the disc).
        0x4C flags: u32,
    }
}

crate::tag_struct! {
    /// An equipment placement (40 bytes: the header alone).
    pub struct EquipmentPlacement: 40 {
        0x00 header: PlacementHeader,
    }
}

crate::tag_struct! {
    /// A sound scenery placement (40 bytes: the header alone).
    pub struct SoundScenery: 40 {
        0x00 header: PlacementHeader,
    }
}

crate::tag_struct! {
    /// A weapon placement (92 bytes; no multiplayer map places one, the
    /// field order is the reference's).
    pub struct WeaponPlacement: 92 {
        0x00 header: PlacementHeader,
        0x48 rounds_total: i16,
        0x4A rounds_loaded: i16,
        /// Bit 0 initially at rest, bit 1 obsolete, bit 2 does accelerate.
        0x4C flags: u16,
    }
}

crate::tag_struct! {
    /// A machine placement (64 bytes).
    pub struct Machine: 64 {
        0x00 header: PlacementHeader,
        /// Element of device groups; -1 none.
        0x28 power_group: i16,
        0x2A position_group: i16,
        /// Bit 0 initially open, 1 initially off, 2 changes only once, 3
        /// position reversed, 4 not usable from any side.
        0x2C device_flags: u32,
        /// Bit 0 does not operate automatically, 1 one-sided, 2 never
        /// appears locked, 3 opened by melee.
        0x30 machine_flags: u8,
    }
}

crate::tag_struct! {
    /// A control placement (64 bytes).
    pub struct Control: 64 {
        0x00 header: PlacementHeader,
        0x28 power_group: i16,
        0x2A position_group: i16,
        0x2C device_flags: u32,
        0x30 control_flags: u16,
        0x34 custom_name_index: i16,
    }
}

crate::tag_struct! {
    /// A light fixture placement (88 bytes). The disc stores colour,
    /// intensity and angles as 0: the light comes from the fixture's tag.
    pub struct LightFixture: 88 {
        0x00 header: PlacementHeader,
        0x28 power_group: i16,
        0x2A position_group: i16,
        0x2C device_flags: u32,
        0x30 color: RealRgb,
        0x3C intensity: f32,
        0x40 falloff_angle: f32,
        0x44 cutoff_angle: f32,
    }
}

crate::tag_struct! {
    /// A device group (52 bytes; none on the multiplayer maps).
    pub struct DeviceGroup: 52 {
        0x00 name: String,
        0x20 initial_value: f32,
        0x24 flags: u32,
    }
}

coded_enum! {
    /// A trigger volume's shape.
    pub enum TriggerVolumeType {
        AxisAlignedBox = 0,
        OrientedBox = 1,
    }
}

crate::tag_struct! {
    /// A trigger volume (96 bytes; none on the multiplayer maps): the box
    /// with one corner at `position` and sides `extents` along the forward,
    /// left and up vectors.
    pub struct TriggerVolume: 96 {
        0x00 volume_type: TriggerVolumeType,
        0x04 name: String,
        0x30 forward: [f32; 3],
        0x3C up: [f32; 3],
        0x48 position: [f32; 3],
        0x54 extents: [f32; 3],
    }
}

crate::tag_struct! {
    /// A decal placement (16 bytes). The game draws the structure BSP's own
    /// decals instead; these are authoring leftovers.
    pub struct Decal: 16 {
        0x00 palette_index: i16,
        /// The tools' yaw byte (scale not established).
        0x02 yaw_byte: i8,
        /// The tools' pitch byte.
        0x03 pitch_byte: i8,
        0x04 position: [f32; 3],
    }
}
