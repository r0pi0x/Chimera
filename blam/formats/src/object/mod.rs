//! The object tag family (specs/halo_ce/objects.md): every class that
//! begins with the 0x17C-byte object base (biped, vehicle, weapon,
//! equipment, garbage, projectile, scenery, machine, control, light fixture,
//! placeholder, sound scenery) read as plain data, in the game's own values
//! (world units, radians, per-tick or per-second as each field's doc says;
//! no conversion on load).
//!
//! - `ObjectTag::read(map, tag)` dispatches on the tag's class and returns
//!   the class's struct; `ObjectTag::object` gives the shared `ObjectBase`
//!   whatever the class.
//! - Layers, each readable alone from the tag's main struct view with the
//!   spec's absolute offsets: `ObjectBase` (`base`), `UnitBase` and
//!   `BipedPart` (`unit`), `ItemBase` and `EquipmentPart` (`item`),
//!   `DeviceBase`, `MachinePart` and `ControlPart` (`device`). The class
//!   structs compose them (`Biped { object, unit, own }`, `Weapon { object,
//!   item, own }` in `crate::weapon`).
//! - The object functions (`function`): the definition, and the pure
//!   evaluation of one or all four at a time from incoming inputs, with the
//!   periodic tables the shaders share (`crate::shader::PeriodicTables`).
//!
//! Where another spec owns a derived class's fields (vehicles) the reader
//! stops at the offset the objects spec states: `Vehicle` holds the object
//! and unit layers. The projectile's own fields are `crate::projectile`'s
//! (`Projectile` is re-exported here).
//!
//! Known shortcuts:
//! - Out-of-table enumerations are refused at read time (the error names the
//!   tag and offset), except the three the spec defines an out-of-range
//!   meaning for: a periodic function type of 12 or more (and a negative
//!   one) reads as `One`, a transition type of 6 or more as `Linear`, and a
//!   function input index outside 0 to 8 stays as stored (it evaluates as
//!   0).
//! - Blocks with a stated engine limit (attachments 8, functions 4, change
//!   colours 4, magazines and triggers 2) are read whole and then cut to
//!   the limit, so the block must still lie in the tag data.
//! - The transition functions are computed directly, not through the
//!   reference's 1024-entry byte tables (the spec allows it; the two differ
//!   by at most 2/255).
//! - Item layer inputs (offsets 0x19C to 0x1A2) are read as stored (`i16`):
//!   the spec only says they name no source on the disc.
//! - The control's `type` and `triggers when`, and the machine's collision
//!   response, are read as stored: the spec lists no values.
//! - Change colour evaluation (colour permutation choice, blending, team
//!   colours) is not implemented: the tag's change colours are read, the
//!   per-object colour update belongs to the game crate.

use std::io;

use crate::cache::{ClassCode, MapFile, TagEntry, View, class};
use crate::scenario::coded_enum;

pub mod base;
pub mod device;
pub mod function;
pub mod item;
pub mod unit;

pub use base::{
    Attachment, ChangeColor, ChangeColorPermutation, ObjectBase, ObjectFlags, PredictedResource, PredictedResourceType,
    Widget,
};
pub use device::{
    ControlPart, DeviceBase, DeviceFlags, DeviceInput, MachineFlags, MachinePart, MachineType,
};
pub use function::{
    BoundsMode, FunctionFlags, FunctionOutput, ObjectFunction, TransitionFunction, attachment_function, evaluate_all, object_time,
};
pub use crate::projectile::Projectile;
pub use item::{EquipmentPart, ItemBase, ItemFlags, Powerup};
pub use unit::{
    BipedFlags, BipedInput, BipedPart, BlipSize, CameraTrack, ContactPoint, DialogueVariant, PoweredSeat, Seat,
    SeatFlags, SoundVolume, UnitBase, UnitFlags, UnitHudInterface, UnitInput, UnitTeam, UnitWeapon,
};

/// The object base's size, and where each layer's own fields begin
/// (absolute offsets in the tag's main struct).
pub const OBJECT_BASE_SIZE: usize = 0x17C;
/// The unit layer (bipeds, vehicles) is 0x174 bytes from 0x17C.
pub const UNIT_BASE_SIZE: usize = 0x174;
/// The biped's own layer is 0x204 bytes from 0x2F0.
pub const BIPED_OWN_SIZE: usize = 0x204;
/// The item layer (weapons, equipment, garbage) is 0x18C bytes from 0x17C.
pub const ITEM_BASE_SIZE: usize = 0x18C;
/// The device layer (machines, controls, light fixtures) is 0x114 bytes
/// from 0x17C.
pub const DEVICE_BASE_SIZE: usize = 0x114;
/// Where the unit layer's own fields begin.
pub const UNIT_BASE_OFFSET: usize = 0x17C;
/// Where a biped's and a vehicle's own fields begin (the unit layer's end).
pub const UNIT_OWN_OFFSET: usize = 0x2F0;
/// Where the item layer's own fields begin.
pub const ITEM_BASE_OFFSET: usize = 0x17C;
/// Where a weapon's, equipment's and garbage's own fields begin.
pub const ITEM_OWN_OFFSET: usize = 0x308;
/// Where the device layer's own fields begin.
pub const DEVICE_BASE_OFFSET: usize = 0x17C;
/// Where a machine's and a control's own fields begin.
pub const DEVICE_OWN_OFFSET: usize = 0x290;
/// Most attachments an object creates.
pub const MAX_ATTACHMENTS: usize = 8;
/// Most object functions computed.
pub const MAX_FUNCTIONS: usize = 4;
/// Most change colours (A to D).
pub const MAX_CHANGE_COLORS: usize = 4;

/// Struct sizes of the classes, measured on the disc (the first block of a
/// tag starts where its struct ends).
pub mod size {
    /// `bipd`.
    pub const BIPED: usize = 0x4F4;
    /// `vehi`.
    pub const VEHICLE: usize = 0x3F0;
    /// `weap` (14 of 15 tags; see the weapons spec's open question).
    pub const WEAPON: usize = 0x508;
    /// `eqip`.
    pub const EQUIPMENT: usize = 0x3B0;
    /// `garb`.
    pub const GARBAGE: usize = 0x3B0;
    /// `proj`.
    pub const PROJECTILE: usize = 0x24C;
    /// `scen` and `ssce`.
    pub const SCENERY: usize = 0x1FC;
    /// `mach` and `ctrl`.
    pub const MACHINE: usize = 0x324;
    /// `lifi`.
    pub const LIGHT_FIXTURE: usize = 0x2D0;
}

/// Declares a flags newtype from a spec's bit table: `Name(bits)` with a
/// constant per named bit, `bits`, `contains`, `is_empty`, `unnamed_bits`
/// and a `Field` impl reading the stored width.
macro_rules! bit_flags {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident($ty:ty) {
            $( $(#[$vmeta:meta])* $konst:ident = $bit:literal ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        $vis struct $name(pub $ty);

        impl $name {
            $( $(#[$vmeta])* pub const $konst: $name = $name(1 << $bit); )*

            /// Every bit the spec names.
            pub const NAMED: $ty = 0 $( | (1 << $bit) )*;

            /// The stored bits.
            pub const fn bits(self) -> $ty {
                self.0
            }

            /// Every bit of `other` is set.
            pub const fn contains(self, other: $name) -> bool {
                self.0 & other.0 == other.0
            }

            /// No bit is set.
            pub const fn is_empty(self) -> bool {
                self.0 == 0
            }

            /// The set bits the spec names no flag for (0 on every disc tag).
            pub const fn unnamed_bits(self) -> $ty {
                self.0 & !Self::NAMED
            }
        }

        impl $crate::cache::Field for $name {
            const SIZE: usize = <$ty as $crate::cache::Field>::SIZE;
            fn read(v: &$crate::cache::View<'_>, offset: usize) -> ::std::io::Result<Self> {
                <$ty as $crate::cache::Field>::read(v, offset).map($name)
            }
        }
    };
}
pub(crate) use bit_flags;

coded_enum! {
    /// The object type code (object base offset 0), equal to the tag's
    /// class on every tag.
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
        /// `plac`; none on the disc.
        Placeholder = 10,
        SoundScenery = 11,
    }
}

impl ObjectType {
    /// The class a tag of this type has.
    pub const fn class(self) -> ClassCode {
        match self {
            ObjectType::Biped => class::BIPED,
            ObjectType::Vehicle => class::VEHICLE,
            ObjectType::Weapon => class::WEAPON,
            ObjectType::Equipment => class::EQUIPMENT,
            ObjectType::Garbage => class::GARBAGE,
            ObjectType::Projectile => class::PROJECTILE,
            ObjectType::Scenery => class::SCENERY,
            ObjectType::Machine => class::DEVICE_MACHINE,
            ObjectType::Control => class::DEVICE_CONTROL,
            ObjectType::LightFixture => class::DEVICE_LIGHT_FIXTURE,
            ObjectType::Placeholder => PLACEHOLDER,
            ObjectType::SoundScenery => class::SOUND_SCENERY,
        }
    }

    /// The type of a tag of class `c`, if it is an object class.
    pub fn from_class(c: ClassCode) -> Option<ObjectType> {
        ObjectType::ALL.iter().copied().find(|t| t.class() == c)
    }

    /// Bipeds and vehicles.
    pub const fn is_unit(self) -> bool {
        matches!(self, ObjectType::Biped | ObjectType::Vehicle)
    }

    /// Weapons, equipment and garbage.
    pub const fn is_item(self) -> bool {
        matches!(self, ObjectType::Weapon | ObjectType::Equipment | ObjectType::Garbage)
    }

    /// Machines, controls and light fixtures.
    pub const fn is_device(self) -> bool {
        matches!(self, ObjectType::Machine | ObjectType::Control | ObjectType::LightFixture)
    }
}

/// The placeholder class `plac` (on no disc map, so not in the class table).
pub const PLACEHOLDER: ClassCode = ClassCode::new(b"plac");

coded_enum! {
    /// Where an incoming function value comes from (the object base's
    /// inputs A to D, offsets 0x108 to 0x10E).
    pub enum FunctionInput {
        /// The input keeps its previous value (0 at creation).
        None = 0,
        BodyVitality = 1,
        /// Capped at 1 (an overshield reads 1).
        ShieldVitality = 2,
        RecentBodyDamage = 3,
        RecentShieldDamage = 4,
        RandomConstant = 5,
        UmbrellaShieldVitality = 6,
        ShieldStun = 7,
        RecentUmbrellaShieldVitality = 8,
        UmbrellaShieldStun = 9,
        RegionDamage0 = 10,
        RegionDamage1 = 11,
        RegionDamage2 = 12,
        RegionDamage3 = 13,
        RegionDamage4 = 14,
        RegionDamage5 = 15,
        RegionDamage6 = 16,
        RegionDamage7 = 17,
        /// 1 while the object is not dead.
        Alive = 18,
        Compass = 19,
    }
}

coded_enum! {
    /// A grenade type (the unit's, and the grenade equipment's).
    pub enum GrenadeType {
        Frag = 0,
        Plasma = 1,
    }
}

/// A tag's main struct of at least `size` bytes, after checking the tag is
/// an object (derives from `obje`, or is `plac`).
pub(crate) fn object_view<'a>(map: &'a MapFile, tag: &TagEntry, size: usize) -> io::Result<View<'a>> {
    let v = map.tag_view(tag, size)?;
    if !tag.is_a(class::OBJECT) && tag.class != PLACEHOLDER {
        return Err(v.error(0, format_args!("tag is {} `{}`, not an object", tag.class, tag.path)));
    }
    Ok(v)
}

/// The tag is of class `want`; the error names the class it is.
pub(crate) fn check_class<'a>(
    map: &'a MapFile,
    tag: &TagEntry,
    want: ClassCode,
    name: &str,
    size: usize,
) -> io::Result<View<'a>> {
    if tag.class != want {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {} `{}` is not a {name}", map.header().name, tag.class, tag.path),
        ));
    }
    map.tag_view(tag, size)
}

/// The object type code equals the tag's class on every tag; a mismatch is
/// damage.
pub(crate) fn check_type(v: &View<'_>, object: &ObjectBase, want: ObjectType) -> io::Result<()> {
    if object.object_type == want {
        Ok(())
    } else {
        Err(v.error(
            0,
            format_args!("object type {:?} in a {:?} tag", object.object_type, want),
        ))
    }
}

/// A biped tag: the object, unit and biped layers.
#[derive(Clone, Debug, PartialEq)]
pub struct Biped {
    pub object: ObjectBase,
    pub unit: UnitBase,
    pub own: BipedPart,
}

/// A vehicle tag as far as the objects spec goes: the object and unit
/// layers (vehicles.md owns the rest, from 0x2F0).
#[derive(Clone, Debug, PartialEq)]
pub struct Vehicle {
    pub object: ObjectBase,
    pub unit: UnitBase,
}

/// An equipment tag: the object and item layers and the powerup fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Equipment {
    pub object: ObjectBase,
    pub item: ItemBase,
    pub own: EquipmentPart,
}

/// A garbage tag: the object and item layers (no fields of its own).
#[derive(Clone, Debug, PartialEq)]
pub struct Garbage {
    pub object: ObjectBase,
    pub item: ItemBase,
}

/// A scenery tag: the object layer (128 bytes of padding follow).
#[derive(Clone, Debug, PartialEq)]
pub struct Scenery {
    pub object: ObjectBase,
}

/// A sound scenery tag: the object layer (128 bytes of padding follow).
#[derive(Clone, Debug, PartialEq)]
pub struct SoundScenery {
    pub object: ObjectBase,
}

/// A placeholder tag (none on the disc): the object layer only.
#[derive(Clone, Debug, PartialEq)]
pub struct Placeholder {
    pub object: ObjectBase,
}

/// A machine tag: the object and device layers and the machine's part.
#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
    pub object: ObjectBase,
    pub device: DeviceBase,
    pub own: MachinePart,
}

/// A control tag (from the reference only: no disc tag).
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    pub object: ObjectBase,
    pub device: DeviceBase,
    pub own: ControlPart,
}

/// A light fixture tag: the object and device layers (its own 0x40 bytes
/// are zero on the disc and unknown).
#[derive(Clone, Debug, PartialEq)]
pub struct LightFixture {
    pub object: ObjectBase,
    pub device: DeviceBase,
}

macro_rules! simple_reads {
    ($(
        $(#[$m:meta])*
        $ty:ident, $class:expr, $name:literal, $size:expr, $otype:expr;
    )*) => {$(
        impl $ty {
            $(#[$m])*
            pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Self> {
                let v = check_class(map, tag, $class, $name, $size)?;
                let object = ObjectBase::read_view(&v)?;
                check_type(&v, &object, $otype)?;
                Ok($ty { object })
            }
        }
    )*};
}

simple_reads! {
    /// Read a `scen` tag.
    Scenery, class::SCENERY, "scenery", size::SCENERY, ObjectType::Scenery;
    /// Read an `ssce` tag.
    SoundScenery, class::SOUND_SCENERY, "sound scenery", size::SCENERY, ObjectType::SoundScenery;
    /// Read a `plac` tag's object layer.
    Placeholder, PLACEHOLDER, "placeholder", OBJECT_BASE_SIZE, ObjectType::Placeholder;
}

impl Biped {
    /// Read a `bipd` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Self> {
        let v = check_class(map, tag, class::BIPED, "biped", size::BIPED)?;
        let object = ObjectBase::read_view(&v)?;
        check_type(&v, &object, ObjectType::Biped)?;
        Ok(Biped {
            object,
            unit: UnitBase::read_view(&v)?,
            own: BipedPart::read_view(&v)?,
        })
    }
}

impl Vehicle {
    /// Read a `vehi` tag's object and unit layers.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Self> {
        let v = check_class(map, tag, class::VEHICLE, "vehicle", size::VEHICLE)?;
        let object = ObjectBase::read_view(&v)?;
        check_type(&v, &object, ObjectType::Vehicle)?;
        Ok(Vehicle {
            object,
            unit: UnitBase::read_view(&v)?,
        })
    }
}

impl Equipment {
    /// Read an `eqip` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Self> {
        let v = check_class(map, tag, class::EQUIPMENT, "equipment", size::EQUIPMENT)?;
        let object = ObjectBase::read_view(&v)?;
        check_type(&v, &object, ObjectType::Equipment)?;
        Ok(Equipment {
            object,
            item: ItemBase::read_view(&v)?,
            own: EquipmentPart::read_view(&v)?,
        })
    }
}

impl Garbage {
    /// Read a `garb` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Self> {
        let v = check_class(map, tag, class::GARBAGE, "garbage", size::GARBAGE)?;
        let object = ObjectBase::read_view(&v)?;
        check_type(&v, &object, ObjectType::Garbage)?;
        Ok(Garbage {
            object,
            item: ItemBase::read_view(&v)?,
        })
    }
}

impl Machine {
    /// Read a `mach` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Self> {
        let v = check_class(map, tag, class::DEVICE_MACHINE, "machine", size::MACHINE)?;
        let object = ObjectBase::read_view(&v)?;
        check_type(&v, &object, ObjectType::Machine)?;
        Ok(Machine {
            object,
            device: DeviceBase::read_view(&v)?,
            own: MachinePart::read_view(&v)?,
        })
    }
}

impl Control {
    /// Read a `ctrl` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Self> {
        let v = check_class(map, tag, class::DEVICE_CONTROL, "control", size::MACHINE)?;
        let object = ObjectBase::read_view(&v)?;
        check_type(&v, &object, ObjectType::Control)?;
        Ok(Control {
            object,
            device: DeviceBase::read_view(&v)?,
            own: ControlPart::read_view(&v)?,
        })
    }
}

impl LightFixture {
    /// Read a `lifi` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Self> {
        let v = check_class(map, tag, class::DEVICE_LIGHT_FIXTURE, "light fixture", size::LIGHT_FIXTURE)?;
        let object = ObjectBase::read_view(&v)?;
        check_type(&v, &object, ObjectType::LightFixture)?;
        Ok(LightFixture {
            object,
            device: DeviceBase::read_view(&v)?,
        })
    }
}

/// An object tag of any class, as its class's struct.
#[derive(Clone, Debug, PartialEq)]
// Plain data read once per tag; boxing the big variants would only add
// indirection for every user.
#[allow(clippy::large_enum_variant)]
pub enum ObjectTag {
    Biped(Biped),
    Vehicle(Vehicle),
    Weapon(crate::weapon::Weapon),
    Equipment(Equipment),
    Garbage(Garbage),
    Projectile(crate::projectile::Projectile),
    Scenery(Scenery),
    Machine(Machine),
    Control(Control),
    LightFixture(LightFixture),
    Placeholder(Placeholder),
    SoundScenery(SoundScenery),
}

impl ObjectTag {
    /// Read the object tag `tag` as its class's struct. A class that is
    /// not an object class (not `obje` or deriving from it, and not `plac`)
    /// is an error.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<ObjectTag> {
        Ok(match tag.class {
            class::BIPED => ObjectTag::Biped(Biped::read(map, tag)?),
            class::VEHICLE => ObjectTag::Vehicle(Vehicle::read(map, tag)?),
            class::WEAPON => ObjectTag::Weapon(crate::weapon::Weapon::read(map, tag)?),
            class::EQUIPMENT => ObjectTag::Equipment(Equipment::read(map, tag)?),
            class::GARBAGE => ObjectTag::Garbage(Garbage::read(map, tag)?),
            class::PROJECTILE => ObjectTag::Projectile(crate::projectile::Projectile::read(map, tag)?),
            class::SCENERY => ObjectTag::Scenery(Scenery::read(map, tag)?),
            class::DEVICE_MACHINE => ObjectTag::Machine(Machine::read(map, tag)?),
            class::DEVICE_CONTROL => ObjectTag::Control(Control::read(map, tag)?),
            class::DEVICE_LIGHT_FIXTURE => ObjectTag::LightFixture(LightFixture::read(map, tag)?),
            class::SOUND_SCENERY => ObjectTag::SoundScenery(SoundScenery::read(map, tag)?),
            PLACEHOLDER => ObjectTag::Placeholder(Placeholder::read(map, tag)?),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{}: {} `{}` is not an object class",
                        map.header().name,
                        tag.class,
                        tag.path
                    ),
                ));
            }
        })
    }

    /// The object layer every class begins with.
    pub fn object(&self) -> &ObjectBase {
        match self {
            ObjectTag::Biped(t) => &t.object,
            ObjectTag::Vehicle(t) => &t.object,
            ObjectTag::Weapon(t) => &t.object,
            ObjectTag::Equipment(t) => &t.object,
            ObjectTag::Garbage(t) => &t.object,
            ObjectTag::Projectile(t) => &t.object,
            ObjectTag::Scenery(t) => &t.object,
            ObjectTag::Machine(t) => &t.object,
            ObjectTag::Control(t) => &t.object,
            ObjectTag::LightFixture(t) => &t.object,
            ObjectTag::Placeholder(t) => &t.object,
            ObjectTag::SoundScenery(t) => &t.object,
        }
    }

    /// The object type (equal to the tag's class).
    pub fn object_type(&self) -> ObjectType {
        self.object().object_type
    }

    /// The class of the tag read.
    pub fn class(&self) -> ClassCode {
        self.object_type().class()
    }
}
