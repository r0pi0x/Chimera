//! The grenade HUD interface (`grhi`, 0x1F8 bytes; specs/halo_ce/hud.md,
//! "Grenade HUD interface"): the plate, the count panel, the count
//! number, the flash cutoff and the overlays (whose type mask is the
//! grenade HUD's own: 1 flashing, 2 empty, 4 default, 8 always).

use std::io;

use crate::cache::{MapFile, TagEntry, TagRef, TagStruct, class};
use crate::object::check_class;

use super::{NumberElement, SoundElement, StaticElement, weapon::OverlayItem};

/// The grenade HUD interface's size.
pub const GRENADE_HUD_SIZE: usize = 0x1F8;

crate::tag_struct! {
    /// The grenade HUD interface (`grhi`).
    pub struct GrenadeHud: 0x1F8 {
        /// `Anchor::of`; 0 (top left) on the disc.
        0x000 anchor: i16,
        0x024 background: StaticElement,
        0x08C count_background: StaticElement,
        0x0F4 count: NumberElement,
        /// The plate flashes at or below this count.
        0x148 flash_cutoff: i16,
        0x14C overlay_bitmap: TagRef,
        0x15C overlays: Vec<OverlayItem>,
        0x168 sounds: Vec<SoundElement>,
    }
}

impl GrenadeHud {
    /// Read a `grhi` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<GrenadeHud> {
        let v = check_class(map, tag, class::GRENADE_HUD_INTERFACE, "grenade HUD interface", GRENADE_HUD_SIZE)?;
        <GrenadeHud as TagStruct>::read(&v)
    }
}
