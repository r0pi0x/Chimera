//! The HUD tags (specs/halo_ce/hud.md, "The HUD tags") as plain data: the
//! shared building blocks (placements, colour blocks, static, meter,
//! number, icon and sound elements) and the five HUD classes, each read
//! whole in the disc's values (pixels of the 640 by 480 canvas, colours as
//! `Argb8`, times in seconds):
//!
//! - `UnitHud` (`unhi`, here): the shield and health panels, the motion
//!   sensor, the blip placement, the warning sounds, the auxiliary meters
//!   (the flashlight);
//! - `weapon::WeaponHud` (`wphi`): flash cutoffs, statics, meters,
//!   numbers, crosshairs, overlays, screen effects, the messaging icon;
//!   `weapon::chain` follows the parent references;
//! - `grenade::GrenadeHud` (`grhi`);
//! - `globals::HudGlobals` (`hudg`: messaging, waypoints, the motion
//!   sensor's numbers, damage indicators) and `globals::HudNumber`
//!   (`hud#`: the digit sprites' metrics).
//!
//! Each reader checks the tag's class and size and refuses a block whose
//! count or address is damaged; enumerations are kept as stored (`Anchor`
//! is decoded with `Anchor::of`, which refuses nothing: an unknown value
//! reads as top left).
//!
//! Known shortcuts:
//! - The message-text (`hmt `) and string-list (`ustr`) tags are not read
//!   (no pickup messages are drawn yet).
//! - Multitexture overlays and their effectors (only the sniper rifle's
//!   angle ticks) keep only their blend and map references; the auxiliary
//!   overlay panel of the unit HUD (empty on the disc) is not read.

pub mod globals;
pub mod grenade;
pub mod weapon;

use std::io;

use crate::cache::{Argb8, MapFile, TagEntry, TagRef, TagStruct, class};
use crate::object::check_class;

/// The unit HUD interface's size.
pub const UNIT_HUD_SIZE: usize = 0x56C;

/// Where a HUD element's anchor point is (the `anchor` fields): the corners
/// and the centre of the safe frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

impl Anchor {
    /// The stored value (0 to 4; anything else reads as top left).
    pub fn of(v: i16) -> Anchor {
        match v {
            1 => Anchor::TopRight,
            2 => Anchor::BottomLeft,
            3 => Anchor::BottomRight,
            4 => Anchor::Center,
            _ => Anchor::TopLeft,
        }
    }
}

/// `Field` for a struct read inline (`TagStruct`).
macro_rules! inline_field {
    ($($t:ty),* $(,)?) => {$(
        impl $crate::cache::Field for $t {
            const SIZE: usize = <$t as $crate::cache::TagStruct>::SIZE;
            fn read(v: &$crate::cache::View<'_>, offset: usize) -> ::std::io::Result<Self> {
                v.read(offset)
            }
        }
    )*};
}
pub(crate) use inline_field;

crate::tag_struct! {
    /// A placement (0x24 bytes): offset from the anchor, scale, scaling
    /// flags (bit 0 don't scale offset, bit 1 don't scale size).
    pub struct Placement: 0x24 {
        0x00 offset: [i16; 2],
        0x04 scale: [f32; 2],
        0x0C scaling_flags: u16,
    }
}

crate::tag_struct! {
    /// A colour block (0x20 bytes): the plain colour, the flash pattern
    /// and the disabled colour.
    pub struct ColorBlock: 0x20 {
        0x00 color: Argb8,
        0x04 flash_color: Argb8,
        /// Seconds; one cycle of the pattern.
        0x08 flash_period: f32,
        /// Seconds between the flashes of a cycle.
        0x0C flash_delay: f32,
        0x10 flash_count: i16,
        /// Bit 0 "reverse colors".
        0x12 flash_flags: u16,
        /// Seconds; one pulse.
        0x14 flash_length: f32,
        0x18 disabled_color: Argb8,
    }
}

impl ColorBlock {
    pub const REVERSE: u16 = 1;
}

crate::tag_struct! {
    /// A multitexture overlay (0x1E0 bytes; only the sniper's angle ticks
    /// carry one on the disc): its blend and its three maps.
    pub struct MultitextureOverlay: 0x1E0 {
        0x04 framebuffer_blend: i16,
        0x2E map_blending: [i16; 2],
        0x64 mask: TagRef,
        0x74 map: TagRef,
    }
}

crate::tag_struct! {
    /// A static element (0x68 bytes): one picture.
    pub struct StaticElement: 0x68 {
        0x00 placement: Placement,
        /// NONE: nothing is drawn.
        0x24 bitmap: TagRef,
        0x34 colors: ColorBlock,
        0x54 sequence: i16,
        0x58 overlays: Vec<MultitextureOverlay>,
    }
}

crate::tag_struct! {
    /// A meter element (0x68 bytes): a bar through the meter shader rule.
    pub struct MeterElement: 0x68 {
        0x00 placement: Placement,
        0x24 bitmap: TagRef,
        0x34 min_color: Argb8,
        0x38 max_color: Argb8,
        0x3C flash_color: Argb8,
        /// Its alpha byte A: the frame shows through 255 - A.
        0x40 empty_color: Argb8,
        /// Bit 0 switch colour on state change, 1 interpolate, 2 HSV, 3
        /// farthest hue, 4 invert.
        0x44 flags: u8,
        0x45 minimum: u8,
        0x46 sequence: i16,
        0x48 alpha_multiplier: u8,
        0x49 alpha_bias: u8,
        /// Units of value that make a full bar (0: the owner's default).
        0x4A value_scale: i16,
        0x4C opacity: f32,
        /// The fraction of the frame left behind a filled texel.
        0x50 fade: f32,
        0x54 disabled_color: Argb8,
        0x58 overlays: Vec<MultitextureOverlay>,
    }
}

crate::tag_struct! {
    /// A number element (0x54 bytes): digits drawn with the HUD number
    /// tag's glyphs.
    pub struct NumberElement: 0x54 {
        0x00 placement: Placement,
        0x24 colors: ColorBlock,
        0x44 digits: i8,
        /// Bit 0 leading zeros, bit 1 only when zoomed, bit 2 trailing "m".
        0x45 flags: u8,
        0x46 fractional_digits: i8,
    }
}

impl NumberElement {
    pub const LEADING_ZEROS: u8 = 1;
    pub const ONLY_ZOOMED: u8 = 2;
    pub const METRES: u8 = 4;
}

crate::tag_struct! {
    /// An icon element (0x10 bytes): a prompt's picture or a button icon.
    pub struct IconElement: 0x10 {
        0x00 sequence: i16,
        0x02 width_offset: i16,
        0x04 offset: [i16; 2],
        0x08 color: Argb8,
        0x0C frame_rate: i8,
        /// Bit 0 text instead of the picture, 1 use this colour, 2 absolute
        /// width.
        0x0D flags: u8,
        0x0E text_index: i16,
    }
}

crate::tag_struct! {
    /// A sound element (0x38 bytes): a warning sound and the states that
    /// play it.
    pub struct SoundElement: 0x38 {
        /// `snd!` or `lsnd`.
        0x00 sound: TagRef,
        0x10 state_flags: u32,
        0x14 scale: f32,
    }
}

inline_field!(Placement, ColorBlock, StaticElement, MeterElement, NumberElement, IconElement);

crate::tag_struct! {
    /// A metered panel (0xF0 bytes): a background, a meter and 16 bytes
    /// of extras. On the health panel the extras are the medium colour and
    /// the two cutoffs; on the shield panel the same bytes are its
    /// overcharge colours (zero on the disc, unread); on an auxiliary meter
    /// `medium_color`'s bytes are its minimum cutoff (`aux_cutoff`).
    pub struct MeteredPanel: 0xF0 {
        0x00 background: StaticElement,
        0x68 meter: MeterElement,
        0xD0 medium_color: Argb8,
        0xD4 max_cutoff: f32,
        0xD8 min_cutoff: f32,
    }
}

impl MeteredPanel {
    /// An auxiliary meter's minimum cutoff (offset 0xD0 as an f32).
    pub fn aux_cutoff(&self) -> f32 {
        let c = self.medium_color;
        f32::from_bits(u32::from_le_bytes([c.b, c.g, c.r, c.a]))
    }
}

crate::tag_struct! {
    /// The motion sensor panel (0xF0 bytes).
    pub struct SensorPanel: 0xF0 {
        /// The dark disc.
        0x00 background: StaticElement,
        /// The cone and the range label.
        0x68 foreground: StaticElement,
    }
}

crate::tag_struct! {
    /// An auxiliary meter (0x144 bytes; the flashlight).
    pub struct AuxMeter: 0x144 {
        /// 0 integrated light.
        0x00 kind: i16,
        0x14 panel: MeteredPanel,
    }
}

inline_field!(MeteredPanel, SensorPanel);

crate::tag_struct! {
    /// The unit HUD interface (`unhi`, 0x56C bytes).
    pub struct UnitHud: 0x56C {
        /// The anchor of everything but the motion sensor (`Anchor::of`).
        0x000 anchor: i16,
        0x024 background: StaticElement,
        0x08C shield: MeteredPanel,
        0x17C health: MeteredPanel,
        0x26C sensor: SensorPanel,
        /// The blip image's centre, from the bottom-left corner.
        0x35C blip_placement: Placement,
        0x3C0 sounds: Vec<SoundElement>,
        0x3CC aux_meters: Vec<AuxMeter>,
    }
}

impl UnitHud {
    /// Read a `unhi` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<UnitHud> {
        let v = check_class(map, tag, class::UNIT_HUD_INTERFACE, "unit HUD interface", UNIT_HUD_SIZE)?;
        <UnitHud as TagStruct>::read(&v)
    }
}
