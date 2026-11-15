//! The HUD globals (`hudg`, 0x450 bytes) and the HUD number tag (`hud#`,
//! 0x64 bytes) of specs/halo_ce/hud.md: the messaging parameters, the
//! waypoint arrows, the multiplayer HUD scale, the defaults (the empty
//! weapon HUD, the motion sensor's range, velocity sensitivity and scale),
//! the damage indicators, the timer colours and the text indices; the
//! digit sprites' page and metrics.

use std::io;

use crate::cache::{Argb8, MapFile, RealArgb, Rect2d, TagEntry, TagRef, TagStruct, class};
use crate::object::check_class;

use super::{ColorBlock, IconElement, Placement, inline_field};

/// The HUD globals' size.
pub const HUD_GLOBALS_SIZE: usize = 0x450;
/// The HUD number tag's size.
pub const HUD_NUMBER_SIZE: usize = 0x64;

crate::tag_struct! {
    /// The messaging parameters (0x120 bytes).
    pub struct Messaging: 0x120 {
        0x00 anchor: i16,
        0x24 placement: Placement,
        0x48 font: TagRef,
        0x58 split_font: TagRef,
        /// Seconds.
        0x68 up_time: f32,
        0x6C fade_time: f32,
        0x70 state_color: RealArgb,
        0x80 text_color: RealArgb,
        /// Line pitch multiplier.
        0x90 spacing: f32,
        0x94 item_messages: TagRef,
        0xA4 icons: TagRef,
        0xB4 alternate_icon_text: TagRef,
        0xC4 button_icons: Vec<IconElement>,
        0xD0 help_color: ColorBlock,
        0xF0 hud_messages: TagRef,
        0x100 objective_color: ColorBlock,
    }
}

crate::tag_struct! {
    /// A waypoint arrow (0x68 bytes).
    pub struct WaypointArrow: 0x68 {
        0x00 name: String,
        0x28 color: Argb8,
        0x2C opacity: f32,
        0x30 fade: f32,
        /// On screen, off screen, occluded.
        0x34 sequences: [i16; 2],
        0x38 occluded_sequence: i16,
        /// Bit 0 don't rotate off-screen.
        0x4C flags: u32,
    }
}

crate::tag_struct! {
    /// The waypoint parameters (0x9C bytes).
    pub struct Waypoints: 0x9C {
        /// Pixels the arrows keep from the safe frame: top, bottom, left,
        /// right.
        0x00 top: f32,
        0x04 bottom: f32,
        0x08 left: f32,
        0x0C right: f32,
        0x30 bitmap: TagRef,
        0x40 arrows: Vec<WaypointArrow>,
    }
}

crate::tag_struct! {
    /// The defaults (0x50 bytes).
    pub struct Defaults: 0x50 {
        /// Shown when the player holds nothing (`ui\hud\empty`).
        0x00 weapon_hud: TagRef,
        /// World units.
        0x10 sensor_range: f32,
        /// Squared world units per squared tick.
        0x14 sensor_velocity_sensitivity: f32,
        0x18 sensor_scale: f32,
        0x1C title_bounds: Rect2d,
    }
}

crate::tag_struct! {
    /// The damage indicators (0x50 bytes).
    pub struct DamageIndicators: 0x50 {
        0x00 top: i16,
        0x02 bottom: i16,
        0x04 left: i16,
        0x06 right: i16,
        0x28 bitmap: TagRef,
        0x38 sequence: i16,
        0x3A split_sequence: i16,
        0x3C color: Argb8,
    }
}

inline_field!(Messaging, Waypoints, Defaults, DamageIndicators);

crate::tag_struct! {
    /// The HUD globals (`hudg`).
    pub struct HudGlobals: 0x450 {
        0x000 messaging: Messaging,
        0x120 waypoints: Waypoints,
        /// 0.75 on the disc; unread by the game (always 1).
        0x1BC multiplayer_scale: f32,
        0x2C0 defaults: Defaults,
        0x310 damage: DamageIndicators,
        0x360 timer: ColorBlock,
        0x380 timer_up: ColorBlock,
        0x3C8 carnage_report: TagRef,
        0x3D8 loading_begin_text: i16,
        0x3DA loading_end_text: i16,
        0x3DC checkpoint_begin_text: i16,
        0x3DE checkpoint_end_text: i16,
        0x3E0 checkpoint_sound: TagRef,
    }
}

impl HudGlobals {
    /// Read a `hudg` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<HudGlobals> {
        let v = check_class(map, tag, class::HUD_GLOBALS, "HUD globals", HUD_GLOBALS_SIZE)?;
        <HudGlobals as TagStruct>::read(&v)
    }
}

crate::tag_struct! {
    /// The HUD number tag (`hud#`): the digit page and its metrics.
    pub struct HudNumber: 0x64 {
        /// A sprite page whose first sequence holds 15 glyphs: 0 to 9,
        /// ".", ":", "-", "m", "Km".
        0x00 bitmap: TagRef,
        0x10 character_width: i8,
        /// The advance between digits.
        0x11 screen_width: i8,
        0x12 x_offset: i8,
        0x13 y_offset: i8,
        0x14 decimal_point_width: i8,
        0x15 colon_width: i8,
    }
}

impl HudNumber {
    /// Read a `hud#` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<HudNumber> {
        let v = check_class(map, tag, class::HUD_NUMBER, "HUD number", HUD_NUMBER_SIZE)?;
        <HudNumber as TagStruct>::read(&v)
    }
}
