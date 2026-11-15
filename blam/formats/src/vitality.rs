//! A collision model's vitality fields (specs/halo_ce/health_shields.md,
//! "The collision model's vitality fields", with the offsets of
//! specs/halo_ce/collision.md): a `coll` tag's flags, body and shield
//! groups, per-material damage multipliers and per-region thresholds, read
//! beside `collision::read_model` (which reads the same tag's geometry) as
//! `Vitality`, plain data in the game's own values: vitality in points,
//! times in seconds, thresholds in points or fractions as each field's doc
//! says.
//!
//! - `Vitality::read(map, tag)`; `material(i)` and `region(i)` give the
//!   entries, `indirect_material()` the one an area, melee or fall hit uses,
//!   `shield_material_type()` and `MaterialVitality::material_type()` turn a
//!   stored code into `collision::MaterialType`.
//! - Formulas on tag fields alone: `Vitality::recharge_ticks` and
//!   `stun_ticks` (seconds to ticks, truncating) and
//!   `runtime_recharge_velocity` (the engine's per-tick rate).
//!
//! Known shortcuts:
//! - Only the fields the spec lists are read; the remaining bytes (0x010 to
//!   0x233 of the Spartan's model) are zero on the disc and unknown.
//! - Blocks are capped like `collision::read_model`'s (256 materials,
//!   regions and nodes); a region's permutations are not read here (that is
//!   `collision::read_model`).

use std::io;

use crate::cache::{MapFile, TagEntry, TagRef, TagStruct, View, class};
use crate::collision::MaterialType;
use crate::object::{TransitionFunction, bit_flags};

/// The collision model's main struct size.
pub const COLL_SIZE: usize = 0x298;
/// A material element's size.
pub const MATERIAL_SIZE: usize = 0x48;
/// A region element's size.
pub const REGION_SIZE: usize = 0x54;
/// A node element's size.
pub const NODE_SIZE: usize = 0x40;
/// Our sanity bound on each of the three blocks.
pub const MAX_ENTRIES: usize = 256;
/// Game ticks per second.
pub const TICKS: f32 = 30.0;

bit_flags! {
    /// The collision model flags (offset 0x000).
    pub struct ModelFlags(u32) {
        /// Hits on attached objects also go through this object's shield.
        TAKES_SHIELD_DAMAGE_FOR_CHILDREN = 0,
        /// Hits on attached objects also go to this object's body.
        TAKES_BODY_DAMAGE_FOR_CHILDREN = 1,
        /// Friendly hits ignore the leak and failing rules.
        ALWAYS_SHIELDS_FRIENDLY_DAMAGE = 2,
        /// An area hit on this object is repeated on each attached object.
        PASSES_AREA_DAMAGE_TO_CHILDREN = 3,
        /// When this object is hit, its parents never take the body damage.
        PARENT_NEVER_TAKES_BODY_DAMAGE_FOR_US = 4,
        /// Body damage is zero unless the damage effect detonates explosives.
        ONLY_DAMAGED_BY_EXPLOSIVES = 5,
        /// A vehicle without a driver takes no body damage.
        ONLY_DAMAGED_WHILE_OCCUPIED = 6,
    }
}

bit_flags! {
    /// A model material's flags (element 0x20).
    pub struct MaterialFlags(u32) {
        /// A body hit on this material can be a headshot.
        HEAD = 0,
    }
}

bit_flags! {
    /// A region's flags (element 0x20).
    pub struct RegionFlags(u32) {
        /// Not read by the damage code.
        LIVES_UNTIL_OBJECT_DIES = 0,
        /// The object dies when the region is destroyed.
        FORCES_OBJECT_TO_DIE = 1,
        /// The region is destroyed when the object dies.
        DIES_WHEN_OBJECT_DIES = 2,
        /// Not read by the damage code.
        DIES_WHEN_OBJECT_IS_DAMAGED = 3,
        /// While the shield is broken the region shows permutation 1.
        DISAPPEARS_WHEN_SHIELD_IS_OFF = 4,
        INHIBITS_MELEE_ATTACK = 5,
        INHIBITS_WEAPON_ATTACK = 6,
        INHIBITS_WALKING = 7,
        /// The unit drops its weapon on its next update.
        FORCES_DROP_WEAPON = 8,
        /// Read by the unit's dialogue.
        CAUSES_HEAD_MAIMED_SCREAM = 9,
    }
}

crate::tag_struct! {
    /// A model material's damage fields (element 0x48).
    pub struct MaterialVitality: 0x48 {
        0x00 name: String,
        0x20 flags: MaterialFlags,
        /// As stored (`material_type()` decodes it).
        0x24 material_type_code: i16,
        /// Fraction, 0 to 1 despite the tag's name ("percentage"): the share
        /// of a hit that leaks through the shield to the body.
        0x28 shield_leak_fraction: f32,
        0x2C shield_damage_multiplier: f32,
        0x3C body_damage_multiplier: f32,
    }
}

impl MaterialVitality {
    /// The material type, `None` for a code outside 0 to 32.
    pub fn material_type(&self) -> Option<MaterialType> {
        MaterialType::from_code(self.material_type_code)
    }

    /// Whether a body hit here can be a headshot.
    pub fn is_head(&self) -> bool {
        self.flags.contains(MaterialFlags::HEAD)
    }
}

crate::tag_struct! {
    /// A region's damage fields (element 0x54).
    pub struct RegionVitality: 0x54 {
        0x00 name: String,
        0x20 flags: RegionFlags,
        /// Fraction of the body: a counter above this destroys the region (0
        /// is never).
        0x28 damage_threshold: f32,
        /// `effe`: played when the region is destroyed.
        0x38 destroyed_effect: TagRef,
    }
}

crate::tag_struct! {
    /// A node's precomputed damage part (element 0x40): picks the flinch and
    /// death animation set.
    pub struct NodeDamagePart: 0x40 {
        0x00 name: String,
        0x32 damage_part: i16,
    }
}

crate::tag_struct! {
    /// The collision model's body and shield groups, flags, materials,
    /// regions and nodes' damage parts (absolute offsets of the `coll`
    /// main struct).
    pub struct Vitality: 0x298 {
        0x000 flags: ModelFlags,
        /// Index of the material used when a hit has none (area, melee,
        /// falls, riders); -1 is none.
        0x004 indirect_damage_material: i16,
        /// Points.
        0x008 maximum_body_vitality: f32,
        /// Not read by the damage code.
        0x00C body_system_shock: f32,
        /// Fraction: friendly body damage is scaled by 1 minus this.
        0x044 friendly_damage_resistance: f32,
        /// `effe`: played at the hit point when the event asks for it.
        0x070 localized_damage_effect: TagRef,
        /// Points.
        0x080 area_damage_effect_threshold: f32,
        /// `effe`.
        0x084 area_damage_effect: TagRef,
        /// Points: the damaged effect plays once when the body drops below
        /// it (0: never).
        0x094 body_damaged_threshold: f32,
        /// `effe`.
        0x098 body_damaged_effect: TagRef,
        /// `effe`: played on death.
        0x0A8 body_depleted_effect: TagRef,
        /// Points: if negative and the body falls below it, the object is
        /// destroyed (0: never).
        0x0B8 body_destroyed_threshold: f32,
        /// `effe`.
        0x0BC body_destroyed_effect: TagRef,
        /// Points; 0 means no shield.
        0x0CC maximum_shield_vitality: f32,
        /// As stored: the type whose damage modifier applies to shield
        /// damage (`shield_material_type()` decodes it).
        0x0D2 shield_material_type_code: i16,
        0x0EC shield_failure_function: TransitionFunction,
        /// Fraction: 0 means the shield never fails.
        0x0F0 shield_failure_threshold: f32,
        /// Fraction.
        0x0F4 failing_shield_leak_fraction: f32,
        /// Points: shield damage at or above this restarts the shield delay.
        0x108 minimum_stun_damage: f32,
        /// Seconds: the shield delay.
        0x10C stun_time: f32,
        /// Seconds: empty to full.
        0x110 recharge_time: f32,
        /// Fraction: the damaged effect plays once when the shield drops
        /// below it.
        0x184 shield_damaged_threshold: f32,
        /// `effe`.
        0x188 shield_damaged_effect: TagRef,
        /// `effe`: played when the shield breaks.
        0x198 shield_depleted_effect: TagRef,
        /// `effe`: played when recharge starts after a break.
        0x1A8 shield_recharging_effect: TagRef,
        /// Fraction per tick: precomputed `1 / (recharge time x 30)`; the
        /// engine uses this, not the time.
        0x1C0 runtime_shield_recharge_velocity: f32,
        0x234 materials: Vec<MaterialVitality>,
        0x240 regions: Vec<RegionVitality>,
        0x28C nodes: Vec<NodeDamagePart>,
    }
}

impl Vitality {
    /// Read a `coll` tag's vitality fields.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Vitality> {
        if tag.class != class::MODEL_COLLISION_GEOMETRY {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: {} `{}` is not a collision model",
                    map.header().name,
                    tag.class,
                    tag.path
                ),
            ));
        }
        let v = map.tag_view(tag, COLL_SIZE)?;
        Self::read_view(&v)
    }

    /// Read from a view of the tag's main struct (0x298 bytes); the three
    /// blocks are bounded (`MAX_ENTRIES`) and the indirect damage material
    /// is checked against the materials.
    pub fn read_view(v: &View<'_>) -> io::Result<Vitality> {
        for (off, size) in [(0x234, MATERIAL_SIZE), (0x240, REGION_SIZE), (0x28C, NODE_SIZE)] {
            v.block_max(off, size, MAX_ENTRIES)?;
        }
        let out = <Vitality as TagStruct>::read(v)?;
        let n = out.materials.len() as i64;
        let i = out.indirect_damage_material as i64;
        if i < -1 || (n > 0 && i >= n) {
            return Err(v.error(0x004, format_args!("indirect damage material {i} of {n} materials")));
        }
        Ok(out)
    }

    /// The type shield damage is modified by, `None` for a code outside 0 to
    /// 32.
    pub fn shield_material_type(&self) -> Option<MaterialType> {
        MaterialType::from_code(self.shield_material_type_code)
    }

    /// The material an area, melee or fall hit uses.
    pub fn indirect_material(&self) -> Option<&MaterialVitality> {
        usize::try_from(self.indirect_damage_material)
            .ok()
            .and_then(|i| self.materials.get(i))
    }

    pub fn material(&self, index: usize) -> Option<&MaterialVitality> {
        self.materials.get(index)
    }

    pub fn region(&self, index: usize) -> Option<&RegionVitality> {
        self.regions.get(index)
    }

    /// Whether the model has a shield (maximum shield vitality above 0).
    pub fn has_shield(&self) -> bool {
        self.maximum_shield_vitality > 0.0
    }

    /// The shield delay in ticks: `trunc(stun time x 30)` in single
    /// precision (6.0 s is 180).
    pub fn stun_ticks(&self) -> i32 {
        (self.stun_time * TICKS) as i32
    }

    /// The recharge time in ticks (4.0 s is 120), truncated.
    pub fn recharge_ticks(&self) -> i32 {
        (self.recharge_time * TICKS) as i32
    }

    /// The per-tick recharge rate from the tag's recharge time, `1 /
    /// (recharge time x 30)`; the stored `runtime_shield_recharge_velocity`
    /// is what the engine uses.
    pub fn computed_recharge_velocity(&self) -> f32 {
        1.0 / (self.recharge_time * TICKS)
    }
}
