//! The weapon HUD interface (`wphi`, 0x17C bytes; specs/halo_ce/hud.md,
//! "Weapon HUD interface"): the flash cutoffs, the anchor, the statics,
//! meters, numbers, crosshairs and overlays with their state headers, the
//! valid crosshair types, the screen effects and the messaging icon;
//! `chain` follows the parent references (at most 16 levels).

use std::io;

use crate::cache::{Bounds, MapFile, RealRgb, TagEntry, TagRef, TagStruct, class};
use crate::object::check_class;

use super::{ColorBlock, IconElement, MeterElement, NumberElement, Placement, StaticElement, inline_field};

/// The weapon HUD interface's size.
pub const WEAPON_HUD_SIZE: usize = 0x17C;
/// The longest parent chain followed.
pub const MAX_CHAIN: usize = 16;

crate::tag_struct! {
    /// The flash cutoffs (0x2C bytes).
    pub struct FlashCutoffs: 0x2C {
        /// Bit 0 "use parent HUD flash parameters".
        0x00 flags: u16,
        0x04 total_ammo: i16,
        0x06 loaded_ammo: i16,
        /// Percent.
        0x08 heat: i16,
        /// Percent of battery left.
        0x0A age: i16,
    }
}

inline_field!(FlashCutoffs);

crate::tag_struct! {
    /// A static element with its state header (0xB4 bytes).
    pub struct WeaponStatic: 0xB4 {
        /// The state type (0 total, 1 loaded, 2 heat, 3 battery, ...).
        0x00 state: i16,
        0x02 runtime_flags: u16,
        /// 0 always, 1 one window only, 2 split screen only.
        0x04 map_type: i16,
        0x24 element: StaticElement,
    }
}

crate::tag_struct! {
    /// A meter element with its state header (0xB4 bytes).
    pub struct WeaponMeter: 0xB4 {
        0x00 state: i16,
        0x02 runtime_flags: u16,
        0x04 map_type: i16,
        0x24 element: MeterElement,
    }
}

crate::tag_struct! {
    /// A number element with its state header (0xA0 bytes).
    pub struct WeaponNumber: 0xA0 {
        0x00 state: i16,
        0x02 runtime_flags: u16,
        0x04 map_type: i16,
        0x24 element: NumberElement,
        /// Bit 0 "divide number by clip size".
        0x78 weapon_flags: u16,
    }
}

crate::tag_struct! {
    /// One crosshair picture (0x6C bytes).
    pub struct CrosshairItem: 0x6C {
        0x00 placement: Placement,
        0x24 colors: ColorBlock,
        0x44 frame_rate: i16,
        0x46 sequence: i16,
        /// Bit 0 flashes, 1 not a sprite, 2 only when zoomed, 3 sniper
        /// data, 4 hide outside, 5 one zoom level, 6 not when zoomed.
        0x48 flags: u32,
    }
}

impl CrosshairItem {
    pub const FLASHES: u32 = 1;
    pub const NOT_A_SPRITE: u32 = 1 << 1;
    pub const ONLY_ZOOMED: u32 = 1 << 2;
    pub const ONE_ZOOM_LEVEL: u32 = 1 << 5;
    pub const NOT_ZOOMED: u32 = 1 << 6;
    pub const INVALID: u32 = 1 << 7;
}

crate::tag_struct! {
    /// A crosshairs element (0x68 bytes): a crosshair type and its items on
    /// one sprite page.
    pub struct Crosshair: 0x68 {
        /// The crosshair type (0 aim, 1 zoom, 3 should reload, ...).
        0x00 kind: i16,
        0x02 runtime_flags: u16,
        0x04 map_type: i16,
        0x24 bitmap: TagRef,
        0x34 items: Vec<CrosshairItem>,
    }
}

crate::tag_struct! {
    /// One overlay picture (0x88 bytes).
    pub struct OverlayItem: 0x88 {
        0x00 placement: Placement,
        0x24 colors: ColorBlock,
        0x44 frame_rate: i16,
        0x48 sequence: i16,
        /// The draw situations it shows in (a mask).
        0x4A kind: i16,
        /// Bit 0 flashes, bit 1 runtime invalid.
        0x4C flags: u32,
    }
}

crate::tag_struct! {
    /// An overlays element (0x68 bytes).
    pub struct Overlays: 0x68 {
        0x00 state: i16,
        0x02 runtime_flags: u16,
        0x04 map_type: i16,
        0x24 bitmap: TagRef,
        0x34 items: Vec<OverlayItem>,
    }
}

crate::tag_struct! {
    /// A screen effect (0xB8 bytes): the scope's mask and filters.
    pub struct ScreenEffect: 0xB8 {
        /// Bit 0 only when zoomed.
        0x04 mask_flags: u16,
        0x18 mask_full: TagRef,
        0x28 mask_split: TagRef,
        0x40 convolution_flags: u16,
        /// Vertical field of view, radians.
        0x44 convolution_in: Bounds<f32>,
        0x4C convolution_out: Bounds<f32>,
        0x6C light_flags: u16,
        0x6E light_source: i16,
        0x70 light_intensity: f32,
        0x8C desaturation_flags: u16,
        0x8E desaturation_source: i16,
        0x90 desaturation_intensity: f32,
        0x94 desaturation_tint: RealRgb,
    }
}

crate::tag_struct! {
    /// The weapon HUD interface (`wphi`).
    pub struct WeaponHud: 0x17C {
        /// Drawn before this HUD's own elements.
        0x00 parent: TagRef,
        0x10 cutoffs: FlashCutoffs,
        /// The anchor of the statics, meters, numbers and overlays
        /// (`Anchor::of`); crosshairs use the centre.
        0x3C anchor: i16,
        0x60 statics: Vec<WeaponStatic>,
        0x6C meters: Vec<WeaponMeter>,
        0x78 numbers: Vec<WeaponNumber>,
        0x84 crosshairs: Vec<Crosshair>,
        0x90 overlays: Vec<Overlays>,
        /// Bit n: crosshair state n is evaluated.
        0x9C valid_crosshairs: u32,
        0xA0 sounds: Vec<super::SoundElement>,
        0xAC screen_effects: Vec<ScreenEffect>,
        0x13C messaging_icon: IconElement,
    }
}

impl WeaponHud {
    /// Bit 0 of the cutoffs' flags.
    pub const USE_PARENT_FLASH: u16 = 1;

    /// Read a `wphi` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<WeaponHud> {
        let v = check_class(map, tag, class::WEAPON_HUD_INTERFACE, "weapon HUD interface", WEAPON_HUD_SIZE)?;
        <WeaponHud as TagStruct>::read(&v)
    }
}

/// The chain of `tag`: itself, then each parent, up to `MAX_CHAIN`
/// levels (a longer chain or a loop is an error, as is a parent that does
/// not resolve to a `wphi`).
pub fn chain<'a>(map: &'a MapFile, tag: &'a TagEntry) -> io::Result<Vec<(&'a TagEntry, WeaponHud)>> {
    let mut out: Vec<(&TagEntry, WeaponHud)> = Vec::new();
    let mut at = Some(tag);
    while let Some(t) = at {
        if out.len() == MAX_CHAIN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: weapon HUD `{}`: a parent chain longer than {MAX_CHAIN}", map.header().name, tag.path),
            ));
        }
        let hud = WeaponHud::read(map, t)?;
        at = if hud.parent.is_null() {
            None
        } else {
            Some(map.tag(hud.parent.id).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, format!("{}: weapon HUD `{}`: its parent is missing", map.header().name, t.path))
            })?)
        };
        out.push((t, hud));
    }
    Ok(out)
}

/// A `TagRef` resolved in `map` (None when null or dangling).
pub fn resolve<'a>(map: &'a MapFile, r: &TagRef) -> Option<&'a TagEntry> {
    if r.is_null() { None } else { map.tag(r.id) }
}
