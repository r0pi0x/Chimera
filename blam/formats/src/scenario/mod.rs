//! The scenario tag (`scnr`) of a multiplayer map (specs/halo_ce/scenario.md):
//! `Scenario::read` reads its main struct (0x5B0 bytes) as plain data, in
//! the game's own values (world units, radians as stored, no axis change):
//! skies, player starting locations, starting profiles and equipment,
//! netgame flags and equipment, the object palettes and placements (scenery,
//! bipeds, vehicles, equipment, weapons, machines, controls, light fixtures,
//! sound scenery), device groups, trigger volumes and decals. The blocks a
//! multiplayer match leaves empty (AI, scripts, cutscenes, ...) are counts.
//! Submodules: `netgame` (game types and eligibility, starts, flags,
//! netgame and starting equipment), `placements` (the shared placement
//! header, each type's own fields, the rotation convention), `sky` (the
//! `sky ` tag), `item_collection` (the `itmc` tag) and `object_header`
//! (the few fields of an object tag the scenery placement needs).
//!
//! - `Scenario::read(map)` reads the map's scenario; `Scenario::bsps`
//!   hands back the structure BSP list the cache reader already holds.
//! - Helpers the spec defines as data rules: `GameTypes::eligible_for`,
//!   `Scenario::teleporter_pairs`, `NetgameEquipment::period_seconds`,
//!   `PlacementHeader::is_created`, `Orientation::from_euler`,
//!   `heading_forward`.
//!
//! Known shortcuts:
//! - The 32 bytes of permutation data of scenery, biped, vehicle and weapon
//!   placements are skipped (the spec: zero on the disc, never read).
//! - Block element layouts the spec leaves unmeasured (functions, the AI
//!   and script blocks, cutscene and comment blocks) are read as counts
//!   only; so is the detail object collection palette.
//! - A machine's flags byte at 0x30 is read as one byte (the spec's size);
//!   a control's as a 16-bit word.

use std::io;

use crate::cache::{Bsp, Field, MapFile, TagRef, TagStruct, View};

/// Declares a C-like enum from a spec's table of codes, with `from_code`,
/// `code`, `ALL`, and a `Field` impl reading an i16 that errors (naming the
/// tag and offset) outside the table.
macro_rules! coded_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $var:ident = $code:literal ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $vis enum $name { $( $(#[$vmeta])* $var, )* }

        impl $name {
            /// Every value, in code order.
            pub const ALL: &'static [$name] = &[ $( $name::$var, )* ];

            /// The value of a stored code, if it is in the table.
            pub fn from_code(code: i16) -> Option<Self> {
                match code { $( $code => Some($name::$var), )* _ => None }
            }

            /// The stored code.
            pub fn code(self) -> i16 {
                match self { $( $name::$var => $code, )* }
            }
        }

        impl $crate::cache::Field for $name {
            const SIZE: usize = 2;
            fn read(v: &$crate::cache::View<'_>, offset: usize) -> ::std::io::Result<Self> {
                let code = v.i16(offset)?;
                Self::from_code(code).ok_or_else(|| {
                    v.error(offset, format_args!("{} code {code} is outside the table", stringify!($name)))
                })
            }
        }
    };
}
pub(crate) use coded_enum;

/// Implements `Field` for a `TagStruct` that sits inline in another
/// struct (read through `View::read`).
macro_rules! nested_field {
    ($($t:ty),* $(,)?) => {$(
        impl $crate::cache::Field for $t {
            const SIZE: usize = <$t as $crate::cache::TagStruct>::SIZE;
            fn read(v: &$crate::cache::View<'_>, offset: usize) -> ::std::io::Result<Self> {
                v.read::<$t>(offset)
            }
        }
    )*};
}
pub(crate) use nested_field;

pub mod item_collection;
pub mod netgame;
pub mod object_header;
pub mod placements;
pub mod sky;

pub use item_collection::{ItemCollection, ItemPermutation};
pub use netgame::{
    GameType, GameTypes, NetgameEquipment, NetgameFlag, NetgameFlagType, PlayerStart, StartingEquipment,
    StartingProfile, TeleporterPair, heading_forward, teleport_heading,
};
pub use object_header::{ObjectHeader, ObjectType};
pub use placements::{
    Control, Decal, DeviceGroup, EquipmentPlacement, LightFixture, Machine, Orientation, PlacementHeader, Scenery,
    SoundScenery, TriggerVolume, TriggerVolumeType, UnitPlacement, WeaponPlacement,
};
pub use sky::{Fog, Sky, SkyAnimation, SkyLight};

/// The scenario's main struct, in bytes.
pub const SCENARIO_SIZE: usize = 0x5B0;
/// Most sky references.
pub const MAX_SKIES: usize = 8;
/// Most scenery placements.
pub const MAX_SCENERY: usize = 2000;
/// Most player starting locations.
pub const MAX_PLAYER_STARTS: usize = 256;
/// Most netgame flags.
pub const MAX_NETGAME_FLAGS: usize = 200;
/// Most netgame equipment entries.
pub const MAX_NETGAME_EQUIPMENT: usize = 200;
/// Most entries of one object palette.
pub const MAX_PALETTE: usize = 100;

coded_enum! {
    /// The scenario's type (0x3C).
    pub enum ScenarioType {
        Solo = 0,
        Multiplayer = 1,
        MainMenu = 2,
    }
}

/// A tag block field read only for its count (blocks a multiplayer match
/// does not use).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockCount(pub usize);

impl Field for BlockCount {
    const SIZE: usize = 12;
    fn read(v: &View<'_>, offset: usize) -> io::Result<Self> {
        let raw = v.raw_block(offset)?;
        usize::try_from(raw.count)
            .map(BlockCount)
            .map_err(|_| v.error(offset, format_args!("negative block count {}", raw.count)))
    }
}

crate::tag_struct! {
    /// A sky reference of the skies block (16 bytes: the reference alone).
    pub struct SkyReference: 16 {
        0x00 sky: TagRef,
    }
}

crate::tag_struct! {
    /// An object palette entry (48 bytes): the object tag's reference; the
    /// rest is unused.
    pub struct PaletteEntry: 48 {
        0x00 object: TagRef,
    }
}

crate::tag_struct! {
    /// A decal palette entry (16 bytes: the reference alone).
    pub struct DecalPaletteEntry: 16 {
        0x00 decal: TagRef,
    }
}

crate::tag_struct! {
    /// An object name (36 bytes; the two runtime scratch words after the
    /// name are not read).
    pub struct ObjectName: 36 {
        0x00 name: String,
    }
}

crate::tag_struct! {
    /// The scenario tag's main struct (0x5B0 bytes): the blocks and fields
    /// of specs/halo_ce/scenario.md, "The scenario's top level".
    pub struct Scenario: 0x5B0 {
        0x030 skies: Vec<SkyReference>,
        0x03C scenario_type: ScenarioType,
        0x03E flags: u16,
        0x040 child_scenarios: BlockCount,
        /// Radians; the heading that counts as north for the compass.
        0x04C local_north: f32,
        0x0EC predicted_resources: BlockCount,
        0x0F8 functions: BlockCount,
        0x118 comments: BlockCount,
        0x204 object_names: Vec<ObjectName>,
        0x210 scenery: Vec<Scenery>,
        0x21C scenery_palette: Vec<PaletteEntry>,
        0x228 bipeds: Vec<UnitPlacement>,
        0x234 biped_palette: Vec<PaletteEntry>,
        0x240 vehicles: Vec<UnitPlacement>,
        0x24C vehicle_palette: Vec<PaletteEntry>,
        0x258 equipment: Vec<EquipmentPlacement>,
        0x264 equipment_palette: Vec<PaletteEntry>,
        0x270 weapons: Vec<WeaponPlacement>,
        0x27C weapon_palette: Vec<PaletteEntry>,
        0x288 device_groups: Vec<DeviceGroup>,
        0x294 machines: Vec<Machine>,
        0x2A0 machine_palette: Vec<PaletteEntry>,
        0x2AC controls: Vec<Control>,
        0x2B8 control_palette: Vec<PaletteEntry>,
        0x2C4 light_fixtures: Vec<LightFixture>,
        0x2D0 light_fixture_palette: Vec<PaletteEntry>,
        0x2DC sound_scenery: Vec<SoundScenery>,
        0x2E8 sound_scenery_palette: Vec<PaletteEntry>,
        0x348 starting_profiles: Vec<StartingProfile>,
        0x354 player_starts: Vec<PlayerStart>,
        0x360 trigger_volumes: Vec<TriggerVolume>,
        0x36C recorded_animations: BlockCount,
        0x378 netgame_flags: Vec<NetgameFlag>,
        0x384 netgame_equipment: Vec<NetgameEquipment>,
        0x390 starting_equipment: Vec<StartingEquipment>,
        0x39C bsp_switch_trigger_volumes: BlockCount,
        0x3A8 decals: Vec<Decal>,
        0x3B4 decal_palette: Vec<DecalPaletteEntry>,
        0x3C0 detail_object_collection_palette: BlockCount,
        0x420 ai_actor_palette: BlockCount,
        0x42C ai_encounters: BlockCount,
        0x438 ai_command_lists: BlockCount,
        0x444 ai_animation_references: BlockCount,
        0x450 ai_script_references: BlockCount,
        0x45C ai_recording_references: BlockCount,
        0x468 ai_conversations: BlockCount,
        0x49C scripts: BlockCount,
        0x4A8 script_globals: BlockCount,
        0x4B4 script_references: BlockCount,
        0x4C0 script_source_files: BlockCount,
        0x4E4 cutscene_flags: BlockCount,
        0x4F0 cutscene_camera_points: BlockCount,
        0x4FC cutscene_titles: BlockCount,
        /// The structure BSP list's length; `Scenario::bsps` has the list.
        0x5A4 structure_bsp_count: BlockCount,
    }
}

impl Scenario {
    /// Read the map's scenario, checking each block against the spec's
    /// most-elements limits.
    pub fn read(map: &MapFile) -> io::Result<Scenario> {
        let tag = map.scenario();
        let v = map.tag_view(tag, SCENARIO_SIZE)?;
        let s = <Scenario as TagStruct>::read(&v)?;
        let limits: [(&str, usize, usize, usize); 8] = [
            ("skies", s.skies.len(), MAX_SKIES, 0x030),
            ("scenery", s.scenery.len(), MAX_SCENERY, 0x210),
            (
                "player starting locations",
                s.player_starts.len(),
                MAX_PLAYER_STARTS,
                0x354,
            ),
            ("netgame flags", s.netgame_flags.len(), MAX_NETGAME_FLAGS, 0x378),
            (
                "netgame equipment",
                s.netgame_equipment.len(),
                MAX_NETGAME_EQUIPMENT,
                0x384,
            ),
            ("scenery palette", s.scenery_palette.len(), MAX_PALETTE, 0x21C),
            ("vehicle palette", s.vehicle_palette.len(), MAX_PALETTE, 0x24C),
            ("machine palette", s.machine_palette.len(), MAX_PALETTE, 0x2A0),
        ];
        for (name, count, max, offset) in limits {
            if count > max {
                return Err(v.error(offset, format_args!("{name}: {count} elements, over the maximum {max}")));
            }
        }
        Ok(s)
    }

    /// The map's structure BSP list (read when the map was loaded).
    pub fn bsps(map: &MapFile) -> io::Result<&[Bsp]> {
        map.bsps()
    }

    /// Every teleporter source with its target, in block order of the
    /// sources.
    pub fn teleporter_pairs(&self) -> Vec<TeleporterPair> {
        netgame::teleporter_pairs(&self.netgame_flags)
    }

    /// The player starts eligible for `game` (their indices, in block
    /// order).
    pub fn eligible_starts(&self, game: GameType) -> Vec<usize> {
        (0..self.player_starts.len())
            .filter(|&i| self.player_starts[i].game_types.eligible_for(game))
            .collect()
    }
}
