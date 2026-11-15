//! A tiny reader of an object tag's header essentials (specs/halo_ce/
//! scenario.md, "Scenery as a renderer needs it"): the object type, flags,
//! bounding sphere, and the model, animation graph and collision model
//! references, as the scenery placement needs them. The full object reader
//! (units, items, devices) is a later step from the objects spec; this one
//! is small and named so it can be replaced by it.

use std::io;

use crate::cache::{MapFile, TagEntry, TagRef, TagStruct};

/// Bytes of the object tag's main struct this reader views (through the
/// render bounding radius at 0x104).
pub const OBJECT_HEADER_SIZE: usize = 0x108;

coded_enum! {
    /// The object type (header 0x00).
    pub enum ObjectType {
        Biped = 0,
        Vehicle = 1,
        Weapon = 2,
        Equipment = 3,
        Garbage = 4,
        Projectile = 5,
        Scenery = 6,
        Machine = 7,
        Control = 8,
        LightFixture = 9,
        SoundScenery = 11,
    }
}

crate::tag_struct! {
    /// The shared object header's essentials, at the start of every object
    /// tag's main struct.
    pub struct ObjectHeader: 0x108 {
        0x00 object_type: ObjectType,
        0x02 flags: u16,
        /// World units.
        0x04 bounding_radius: f32,
        /// Centre of the bounding sphere relative to the origin.
        0x08 bounding_offset: [f32; 3],
        /// Not applied to the model (0 on every disc scenery).
        0x14 origin_offset: [f32; 3],
        /// Class `mode`.
        0x28 model: TagRef,
        /// Class `antr`.
        0x38 animation_graph: TagRef,
        /// Class `coll`.
        0x70 collision_model: TagRef,
        0x104 render_bounding_radius: f32,
    }
}

impl ObjectHeader {
    /// Read the header of the object tag `tag` of `map` (any class that
    /// derives from `obje`).
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<ObjectHeader> {
        let v = map.tag_view(tag, OBJECT_HEADER_SIZE)?;
        if !tag.is_a(crate::cache::class::OBJECT) {
            return Err(v.error(0, format_args!("tag is {} `{}`, not an object", tag.class, tag.path)));
        }
        <ObjectHeader as TagStruct>::read(&v)
    }
}
