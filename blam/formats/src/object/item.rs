//! The item layer and the equipment's own fields (specs/halo_ce/objects.md,
//! "Items, equipment, garbage"), absolute offsets: the item layer is 0x17C
//! to 0x308 (weapons add their own part, `crate::weapon`), the equipment's
//! fields 0x308 to 0x320 (a struct of 0x3B0), garbage has none.

use std::io;

use super::{GrenadeType, bit_flags};
use crate::cache::{TagRef, TagStruct, View};
use crate::scenario::coded_enum;

bit_flags! {
    /// The item flags (offset 0x17C).
    pub struct ItemFlags(u32) {
        ALWAYS_MAINTAINS_Z_UP = 0,
        /// Acted on only when no multiplayer game engine is running.
        DESTROYED_BY_EXPLOSIONS = 1,
        UNAFFECTED_BY_GRAVITY = 2,
    }
}

coded_enum! {
    /// An equipment tag's powerup (offset 0x308).
    pub enum Powerup {
        None = 0,
        DoubleSpeed = 1,
        Overshield = 2,
        ActiveCamouflage = 3,
        FullSpectrumVision = 4,
        Health = 5,
        Grenade = 6,
    }
}

crate::tag_struct! {
    /// The item layer (`SIZE` is its end, 0x308).
    pub struct ItemBase: 0x308 {
        0x17C flags: ItemFlags,
        /// Index into the HUD message text list (the pickup prompt).
        0x180 message_index: i16,
        /// Order in the unit's inventory.
        0x182 sort_order: i16,
        /// Scale of the item lying in the world in a multiplayer game (0 =
        /// 1), range 0.5 to 3.
        0x184 scale: f32,
        /// Scale of the number in the pickup prompt.
        0x188 hud_message_value_scale: i16,
        /// Item input sources, A to D, as stored (none on every disc item).
        0x19C input_a: i16,
        0x19E input_b: i16,
        0x1A0 input_c: i16,
        0x1A2 input_d: i16,
        /// Class `foot`: effects of the item hitting a surface.
        0x248 material_effects: TagRef,
        /// Class `snd!`.
        0x258 collision_sound: TagRef,
        /// Seconds: an explosion sets the item off after a random time
        /// between the two.
        0x2E0 detonation_delay_minimum: f32,
        0x2E4 detonation_delay_maximum: f32,
        /// Class `effe`: played when the countdown starts.
        0x2E8 detonating_effect: TagRef,
        /// Class `effe`: played at detonation.
        0x2F8 detonation_effect: TagRef,
    }
}

impl ItemBase {
    /// Read the layer from a view of the tag's main struct.
    pub fn read_view(v: &View<'_>) -> io::Result<ItemBase> {
        <ItemBase as TagStruct>::read(v)
    }
}

crate::tag_struct! {
    /// The equipment's own fields (`SIZE` is their end, 0x320).
    pub struct EquipmentPart: 0x320 {
        0x308 powerup_type: Powerup,
        /// For `Powerup::Grenade`; ignored by the others.
        0x30A grenade_type: GrenadeType,
        /// Seconds the powerup lasts (the engine uses it times 30 as ticks).
        0x30C powerup_time: f32,
        /// Class `snd!`.
        0x310 pickup_sound: TagRef,
    }
}

impl EquipmentPart {
    /// Read the part from a view of the tag's main struct.
    pub fn read_view(v: &View<'_>) -> io::Result<EquipmentPart> {
        <EquipmentPart as TagStruct>::read(v)
    }
}
