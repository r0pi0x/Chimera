//! The animation graph tag (`antr`, "model_animations") reader and the
//! animation maths from specs/halo_ce/animations.md: `AnimationGraph::read`
//! gives every block the spec lays out as plain structs (objects, unit
//! seats with their weapon classes and weapon types, weapons, vehicles,
//! devices, unit damage, first-person weapons, sound references, the node
//! list and the animations with their frame data) in the game's own values
//! (no unit or axis conversion).
//!
//! - Index tables: `slots.rs` has one enum per table (`UnitSlot`,
//!   `WeaponClassSlot`, `WeaponTypeSlot`, `WeaponSlot`, `VehicleSlot`,
//!   `DeviceSlot`, `FirstPersonSlot`, `UnitDamageSlot`) and `IndexTable<S>`
//!   whose `get(slot)` is the animation index or `None`. The graph's own
//!   lookups resolve a slot to the `Animation`: `unit_animation`,
//!   `class_animation`, `type_animation`, `weapon_animation`,
//!   `first_person_animation`, `vehicle_animation`, `device_animation`,
//!   `unit_damage_animation`.
//! - Selection: `AnimationGraph::select_unit(seat label, weapon label)`
//!   picks the (seat, weapon class, weapon type) triple as the spec
//!   describes (case-insensitive; **the last match in block order wins**;
//!   `None` leaves the caller's previous selection).
//! - Sampling: `Animation::sample` (an integer frame) and
//!   `Animation::sample_continuous` (a fractional one) give a `Pose`, a
//!   `NodePose` (rotation `[i, j, k, w]` as stored, translation, scale) per
//!   node, from uncompressed frame data and from the compressed key stream
//!   (the form the cinematics use). `Animation::root_motion` gives a
//!   frame's `FrameInfo`.
//! - Blending: `Pose::apply` (base, replacement, overlay by the animation's
//!   type), scaled and continuous overlays, `Pose::blend_from` (the
//!   transition), `Pose::apply_screen` with `ScreenBounds::locate` (the
//!   aim and look grid). `Pose::world_transforms` composes a pose with a
//!   node tree into `model::Transform`s, the same math as
//!   `Model::bind_pose`.
//! - Matching to models: `Animation::fits_model` (the base animation
//!   check), `AnimationGraph::match_nodes` (graph nodes to a model's by
//!   exact name, as first-person graphs are matched).
//! - `playback.rs`: `Playback::advance` and `smooth_step`.
//!
//! Known shortcuts:
//! - A table entry past the animations block reads as stored; the lookups
//!   (`AnimationGraph::animation`) treat it as none, as the spec's edge
//!   cases say, rather than refusing the graph.
//! - The graph's flags, the joint fields of the nodes and the suspension of
//!   vehicles are stored, not interpreted.
//! - Frame data, default data and the compressed stream are copied out of
//!   the map as bytes; their sizes are checked when a frame is sampled (an
//!   error naming the animation), not when the graph is read, so one bad
//!   animation does not refuse its graph.
//! - The 25 Hz flag is stored; no stretching of PAL frames is done.

mod playback;
mod pose;
mod sample;
mod slots;

use std::io;

use crate::cache::{MapFile, TagEntry, TagRef, View, class};

pub use crate::model::Transform;
pub use playback::{PlayEvent, Playback, smooth_step};
pub use pose::{
    AimCell, IDENTITY_ROTATION, NodePose, Pose, lerp3, quat_length, quat_mul, quat_nlerp, quat_normalize,
    transition_weight,
};
pub use sample::{decode_compressed_rotation, split_node_header};
pub use slots::{
    BodyPart, DamageDirection, DamageKind, DeviceSlot, FirstPersonSlot, IndexTable, Slot, UNIT_DAMAGE_SLOTS,
    UnitDamageSlot, UnitSlot, VehicleSlot, WeaponClassSlot, WeaponSlot, WeaponTypeSlot,
};

/// The graph's main struct.
pub const GRAPH_SIZE: usize = 0x80;
pub const ANIMATION_SIZE: usize = 0xB4;
pub const GRAPH_NODE_SIZE: usize = 0x40;
pub const OBJECT_OVERLAY_SIZE: usize = 0x14;
pub const UNIT_SEAT_SIZE: usize = 0x64;
pub const WEAPON_CLASS_SIZE: usize = 0xBC;
pub const WEAPON_TYPE_SIZE: usize = 0x3C;
pub const IK_POINT_SIZE: usize = 0x40;
pub const GRAPH_WEAPONS_SIZE: usize = 0x1C;
pub const GRAPH_VEHICLE_SIZE: usize = 0x74;
pub const SUSPENSION_SIZE: usize = 0x14;
pub const GRAPH_DEVICE_SIZE: usize = 0x60;
pub const FIRST_PERSON_WEAPON_SIZE: usize = 0x1C;
pub const SOUND_REFERENCE_SIZE: usize = 0x14;
/// A stored 16-bit quaternion component `c` decodes to `c / 32767`.
pub const QUAT16_DIVISOR: f32 = 32767.0;
/// Most animations per graph, sound references, nodes per animation: the
/// editor's limits, used as the block caps.
pub const MAX_ANIMATIONS: usize = 256;
pub const MAX_SOUND_REFERENCES: usize = 257;
pub const MAX_NODES: usize = 64;
/// A cap on the number of entries of an index table.
pub const MAX_TABLE_ENTRIES: usize = 256;
/// Animation flag bits.
pub const FLAG_COMPRESSED: u16 = 1;
pub const FLAG_WORLD_RELATIVE: u16 = 2;
pub const FLAG_25_HZ: u16 = 4;
/// The weapon label that selects the type with an empty label.
pub const UNARMED: &str = "unarmed";

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// The animation graph (`antr`).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationGraph {
    pub objects: Vec<ObjectOverlay>,
    /// The seats of a unit graph (stand, crouch, vehicle seats).
    pub units: Vec<UnitSeat>,
    /// At most one.
    pub weapons: Vec<GraphWeapons>,
    /// At most one.
    pub vehicles: Vec<GraphVehicle>,
    /// At most one.
    pub devices: Vec<GraphDevice>,
    pub unit_damage: IndexTable<UnitDamageSlot>,
    /// At most one.
    pub first_person_weapons: Vec<FirstPersonWeapon>,
    pub sounds: Vec<SoundReference>,
    /// World units; 0 on every multiplayer graph.
    pub limp_body_node_radius: f32,
    /// Build-time options; not read at run time.
    pub flags: u16,
    /// May be empty (a model then supplies the node tree).
    pub nodes: Vec<GraphNode>,
    pub animations: Vec<Animation>,
}

/// Which exported function of an object drives an overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectFunction {
    A,
    B,
    C,
    D,
}

/// How an object overlay plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectOverlayMode {
    Frame,
    Scale,
}

/// An object overlay (0x14 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectOverlay {
    pub animation: Option<usize>,
    pub function: ObjectFunction,
    pub mode: ObjectOverlayMode,
}

/// Bounds of an aiming or looking screen (24 bytes): radians per column
/// and row and the frame counts around the centre. Frame `column + row x
/// W` with `W = right + left + 1` columns; see `ScreenBounds::locate`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScreenBounds {
    /// Radians of yaw (toward negative yaw) between columns.
    pub right_yaw_per_frame: f32,
    /// Radians of yaw (toward positive yaw) between columns.
    pub left_yaw_per_frame: f32,
    /// Columns to the right of the centre.
    pub right_frame_count: i16,
    /// Columns to the left of the centre.
    pub left_frame_count: i16,
    /// Radians of pitch (toward negative pitch) between rows.
    pub down_pitch_per_frame: f32,
    /// Radians of pitch (toward positive pitch) between rows.
    pub up_pitch_per_frame: f32,
    /// Rows below the centre.
    pub down_frame_count: i16,
    /// Rows above the centre.
    pub up_frame_count: i16,
}

/// An inverse kinematics point (0x40 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct IkPoint {
    /// The marker on the unit.
    pub marker: String,
    /// The marker on the other object.
    pub attach_to_marker: String,
}

/// A seat of a unit graph (0x64 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct UnitSeat {
    /// `stand`, `crouch`, `alert`, a vehicle seat (`W-driver`).
    pub label: String,
    /// For the `look` overlay.
    pub looking_bounds: ScreenBounds,
    pub animations: IndexTable<UnitSlot>,
    pub ik_points: Vec<IkPoint>,
    pub weapon_classes: Vec<WeaponClass>,
}

/// A weapon class of a seat (0xBC bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponClass {
    /// `rifle`, `pistol`, `missile`, `support`, `unarmed`, `fixed`. Never
    /// compared by the engine (the class is found through its types).
    pub label: String,
    pub grip_marker: String,
    pub hand_marker: String,
    /// For `aim-still` and `aim-move`.
    pub aiming_bounds: ScreenBounds,
    pub animations: IndexTable<WeaponClassSlot>,
    pub ik_points: Vec<IkPoint>,
    pub weapon_types: Vec<WeaponType>,
}

/// A weapon type of a class (0x3C bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponType {
    /// Matched against the weapon tag's label (`ar`, `hp`); empty for
    /// unarmed.
    pub label: String,
    pub animations: IndexTable<WeaponTypeSlot>,
}

/// The weapons block (0x1C bytes): a weapon's own third-person graph.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphWeapons {
    pub animations: IndexTable<WeaponSlot>,
}

/// The vehicles block (0x74 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct GraphVehicle {
    pub steering_bounds: ScreenBounds,
    pub animations: IndexTable<VehicleSlot>,
    pub suspension: Vec<Suspension>,
}

/// A vehicle suspension animation (0x14 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct Suspension {
    pub mass_point_index: i16,
    pub animation: Option<usize>,
    pub full_extension_ground_depth: f32,
    pub full_compression_ground_depth: f32,
}

/// The devices block (0x60 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct GraphDevice {
    pub animations: IndexTable<DeviceSlot>,
}

/// The first-person weapons block (0x1C bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct FirstPersonWeapon {
    pub animations: IndexTable<FirstPersonSlot>,
}

/// A sound reference (0x14 bytes): a `snd!` reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundReference {
    pub sound: TagRef,
}

/// A node of the graph (0x40 bytes). The joint fields are exporter data.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphNode {
    pub name: String,
    pub next_sibling: Option<usize>,
    pub first_child: Option<usize>,
    /// As stored: **0 for node 0** on every disc graph (models store
    /// NONE); `AnimationGraph::parents` corrects it.
    pub parent: Option<usize>,
    /// Bit 0 ball-socket, bit 1 hinge, bit 2 no movement.
    pub flags: u32,
    pub base_vector: [f32; 3],
    /// Radians.
    pub vector_range: f32,
}

/// An animation's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationType {
    /// Sets the pose.
    Base,
    /// Adds a rotation and a translation on top.
    Overlay,
    /// Overwrites only the nodes (and parts) it animates.
    Replacement,
}

/// What a frame's root motion record holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameInfoType {
    None,
    /// dx dy
    DxDy,
    /// dx dy dyaw
    DxDyDyaw,
    /// dx dy dz dyaw
    DxDyDzDyaw,
}

impl FrameInfoType {
    /// Bytes of one record.
    pub fn record_size(self) -> usize {
        match self {
            FrameInfoType::None => 0,
            FrameInfoType::DxDy => 8,
            FrameInfoType::DxDyDyaw => 12,
            FrameInfoType::DxDyDzDyaw => 16,
        }
    }
}

/// One frame's root motion in the unit's own frame (x forward, y left):
/// world units per tick, `dyaw` radians per tick (positive turning left).
/// Fields the record's type does not hold are 0.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameInfo {
    pub dx: f32,
    pub dy: f32,
    pub dz: f32,
    pub dyaw: f32,
}

/// One animation (0xB4 bytes) with its data.
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    /// `stand rifle move-front`; permutations end in `%1`, `%2`.
    pub name: String,
    pub kind: AnimationType,
    /// Frames (ticks).
    pub frame_count: i16,
    /// Bytes per uncompressed frame, as stored (`computed_frame_size` is
    /// what sampling uses; they agree on every disc animation).
    pub frame_size: i16,
    pub frame_info_type: FrameInfoType,
    /// Equals the model's node list checksum; 0 = do not check.
    pub node_list_checksum: u32,
    /// Nodes this animation poses.
    pub node_count_raw: i16,
    /// 0 = play once; > 0 = loop back to this frame.
    pub loop_frame: i16,
    /// The editor's permutation weight.
    pub weight: f32,
    /// Frame of the animation's event (0 = none).
    pub key_frame: i16,
    pub second_key_frame: i16,
    /// The next permutation of the same slot.
    pub next_animation: Option<usize>,
    /// Bit 0 compressed, bit 1 world relative, bit 2 25 Hz.
    pub flags: u16,
    /// Index into the graph's sound references.
    pub sound: Option<usize>,
    pub sound_frame: i16,
    /// Footstep frames (0 = none; may exceed the frame count).
    pub left_foot_frame: u8,
    pub right_foot_frame: u8,
    /// Build-time: the first animation of this permutation chain.
    pub first_permutation: Option<usize>,
    /// Build-time: this permutation's cumulative share of the chain.
    pub cumulative_weight: f32,
    /// Root motion per frame (empty for type none).
    pub frame_info: Vec<FrameInfo>,
    /// Bit n: node n has per-frame translation (bit n of word n / 32).
    pub translation_flags: u64,
    pub rotation_flags: u64,
    pub scale_flags: u64,
    /// Offset of the compressed header inside the frame data (0 on disc).
    pub compressed_data_offset: i32,
    /// The values of the nodes without per-frame data (uncompressed).
    pub default_data: Vec<u8>,
    /// The frames, or the compressed stream.
    pub frame_data: Vec<u8>,
}

impl Animation {
    /// Nodes this animation poses.
    pub fn node_count(&self) -> usize {
        self.node_count_raw.max(0) as usize
    }

    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }

    /// The root node's transform is a world transform.
    pub fn is_world_relative(&self) -> bool {
        self.flags & FLAG_WORLD_RELATIVE != 0
    }

    fn bit(&self, word: u64, node: usize) -> bool {
        node < self.node_count() && node < 64 && word >> node & 1 != 0
    }

    /// Node `node` has per-frame rotation (bits at or above the node
    /// count are ignored).
    pub fn has_rotation(&self, node: usize) -> bool {
        self.bit(self.rotation_flags, node)
    }

    pub fn has_translation(&self, node: usize) -> bool {
        self.bit(self.translation_flags, node)
    }

    pub fn has_scale(&self, node: usize) -> bool {
        self.bit(self.scale_flags, node)
    }

    fn counts(&self) -> (usize, usize, usize) {
        let n = self.node_count();
        let c = |f: u64| (0..n).filter(|&i| self.bit(f, i)).count();
        (c(self.rotation_flags), c(self.translation_flags), c(self.scale_flags))
    }

    /// `8 R + 12 T + 4 S` over the nodes below the node count.
    pub fn computed_frame_size(&self) -> usize {
        let (r, t, s) = self.counts();
        8 * r + 12 * t + 4 * s
    }

    /// `8 (n - R) + 12 (n - T) + 4 (n - S)` (uncompressed).
    pub fn computed_default_size(&self) -> usize {
        let (r, t, s) = self.counts();
        let n = self.node_count();
        8 * (n - r) + 12 * (n - t) + 4 * (n - s)
    }

    /// The root motion for the tick spent on `frame`.
    pub fn root_motion(&self, frame: usize) -> Option<FrameInfo> {
        self.frame_info.get(frame).copied()
    }

    /// The base-animation check of a model's skeleton (the spec's "Matching
    /// animations to a model"): a base animation whose node count equals
    /// the model's and whose checksum is 0, or the model's is 0, or they
    /// are equal. Otherwise the object takes its bind pose.
    pub fn fits_model(&self, model_node_count: usize, model_checksum: u32) -> bool {
        self.kind == AnimationType::Base
            && self.node_count() == model_node_count
            && (self.node_list_checksum == 0 || model_checksum == 0 || self.node_list_checksum == model_checksum)
    }
}

/// The result of matching a model's nodes to a graph's by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeMatch {
    /// Per model node, the graph node of the same name.
    pub graph_nodes: Vec<Option<usize>>,
}

impl NodeMatch {
    /// Every model node has a namesake in the graph.
    pub fn is_complete(&self) -> bool {
        self.graph_nodes.iter().all(Option::is_some)
    }

    /// The model nodes without a namesake.
    pub fn missing(&self) -> Vec<usize> {
        (0..self.graph_nodes.len())
            .filter(|&i| self.graph_nodes[i].is_none())
            .collect()
    }

    /// One value per model node, taken from `per_graph_node` (a graph's
    /// per-node values: poses, matrices): the namesake's. `None` when a
    /// model node has no namesake or its index is past the values.
    pub fn select<T: Clone>(&self, per_graph_node: &[T]) -> Option<Vec<T>> {
        self.graph_nodes
            .iter()
            .map(|g| g.and_then(|g| per_graph_node.get(g)).cloned())
            .collect()
    }
}

/// A unit's current animation set: element indices of the seat, of the
/// weapon class in it and of the weapon type in the class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitSelection {
    pub seat: usize,
    pub weapon_class: usize,
    pub weapon_type: usize,
}

impl AnimationGraph {
    /// Read the animation graph tag `tag` (class `antr`) of `map`.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<AnimationGraph> {
        if tag.class != class::MODEL_ANIMATIONS {
            return Err(err(format!(
                "{}: {} `{}` is not an animation graph",
                map.header().name,
                tag.class,
                tag.path
            )));
        }
        let v = map.tag_view(tag, GRAPH_SIZE)?;
        let objects = v
            .block(0x00, OBJECT_OVERLAY_SIZE)?
            .iter()
            .map(|e| read_object_overlay(&e))
            .collect::<io::Result<_>>()?;
        let units = v
            .block(0x0C, UNIT_SEAT_SIZE)?
            .iter()
            .map(|e| read_unit_seat(&e))
            .collect::<io::Result<_>>()?;
        let weapons = v
            .block_max(0x18, GRAPH_WEAPONS_SIZE, 1)?
            .iter()
            .map(|e| {
                Ok(GraphWeapons {
                    animations: read_table(&e, 0x10)?,
                })
            })
            .collect::<io::Result<_>>()?;
        let vehicles = v
            .block_max(0x24, GRAPH_VEHICLE_SIZE, 1)?
            .iter()
            .map(|e| read_vehicle(&e))
            .collect::<io::Result<_>>()?;
        let devices = v
            .block_max(0x30, GRAPH_DEVICE_SIZE, 1)?
            .iter()
            .map(|e| {
                Ok(GraphDevice {
                    animations: read_table(&e, 0x54)?,
                })
            })
            .collect::<io::Result<_>>()?;
        let unit_damage = read_table(&v, 0x3C)?;
        let first_person_weapons = v
            .block_max(0x48, FIRST_PERSON_WEAPON_SIZE, 1)?
            .iter()
            .map(|e| {
                Ok(FirstPersonWeapon {
                    animations: read_table(&e, 0x10)?,
                })
            })
            .collect::<io::Result<_>>()?;
        let sounds = v
            .block_max(0x54, SOUND_REFERENCE_SIZE, MAX_SOUND_REFERENCES)?
            .iter()
            .map(|e| {
                Ok(SoundReference {
                    sound: e.tag_ref(0x00)?,
                })
            })
            .collect::<io::Result<_>>()?;
        let nodes = v
            .block_max(0x68, GRAPH_NODE_SIZE, MAX_NODES)?
            .iter()
            .map(|e| read_node(&e))
            .collect::<io::Result<_>>()?;
        let animations = v
            .block_max(0x74, ANIMATION_SIZE, MAX_ANIMATIONS)?
            .iter()
            .map(|e| read_animation(&e))
            .collect::<io::Result<_>>()?;
        Ok(AnimationGraph {
            objects,
            units,
            weapons,
            vehicles,
            devices,
            unit_damage,
            first_person_weapons,
            sounds,
            limp_body_node_radius: v.f32(0x60)?,
            flags: v.flags16(0x64)?,
            nodes,
            animations,
        })
    }

    /// Animation `index` of the graph; `None` past the block (an index
    /// table entry pointing there counts as none).
    pub fn animation(&self, index: usize) -> Option<&Animation> {
        self.animations.get(index)
    }

    /// The animation an index table's slot holds.
    pub fn resolve<S: Slot>(&self, table: &IndexTable<S>, slot: S) -> Option<&Animation> {
        self.animation(table.get(slot)?)
    }

    /// Each graph node's parent, node 0's stored 0 corrected to `None`
    /// (the spec's quirk: never treat node 0 as its own child). Parents
    /// at or after their child, or past the nodes, read as none.
    pub fn parents(&self) -> Vec<Option<usize>> {
        self.nodes
            .iter()
            .enumerate()
            .map(|(n, node)| node.parent.filter(|&p| n > 0 && p < n))
            .collect()
    }

    /// The graph nodes matching `names` (a model's nodes, in order) by
    /// exact, case-sensitive name; the first graph node of a name.
    pub fn match_nodes<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> NodeMatch {
        NodeMatch {
            graph_nodes: names
                .into_iter()
                .map(|name| self.nodes.iter().position(|n| n.name == name))
                .collect(),
        }
    }

    /// `match_nodes` over a render model's nodes.
    pub fn match_model(&self, model: &crate::model::Model) -> NodeMatch {
        self.match_nodes(model.nodes.iter().map(|n| n.name.as_str()))
    }

    /// The unit's animation set for a seat label and a weapon label: every
    /// seat whose label equals `seat_label` (ignoring ASCII case), every
    /// weapon class of it and every weapon type of the class, matching a
    /// type whose label equals `weapon_label`, or (for `unarmed`) whose
    /// label is empty. **Every match overwrites the previous one: the last
    /// in block order wins.** `None` when nothing matches: the caller
    /// keeps its previous selection (`select_unit(..).or(previous)`).
    pub fn select_unit(&self, seat_label: &str, weapon_label: &str) -> Option<UnitSelection> {
        let unarmed = weapon_label.eq_ignore_ascii_case(UNARMED);
        let mut found = None;
        for (s, seat) in self.units.iter().enumerate() {
            if !seat.label.eq_ignore_ascii_case(seat_label) {
                continue;
            }
            for (c, class) in seat.weapon_classes.iter().enumerate() {
                for (t, ty) in class.weapon_types.iter().enumerate() {
                    if ty.label.eq_ignore_ascii_case(weapon_label) || (unarmed && ty.label.is_empty()) {
                        found = Some(UnitSelection {
                            seat: s,
                            weapon_class: c,
                            weapon_type: t,
                        });
                    }
                }
            }
        }
        found
    }

    /// The seat of a selection.
    pub fn seat(&self, sel: UnitSelection) -> Option<&UnitSeat> {
        self.units.get(sel.seat)
    }

    /// The weapon class of a selection.
    pub fn weapon_class(&self, sel: UnitSelection) -> Option<&WeaponClass> {
        self.seat(sel)?.weapon_classes.get(sel.weapon_class)
    }

    /// The weapon type of a selection.
    pub fn weapon_type(&self, sel: UnitSelection) -> Option<&WeaponType> {
        self.weapon_class(sel)?.weapon_types.get(sel.weapon_type)
    }

    /// A seat slot's animation (flying, opening, look, airborne-dead...).
    pub fn unit_animation(&self, sel: UnitSelection, slot: UnitSlot) -> Option<&Animation> {
        self.resolve(&self.seat(sel)?.animations, slot)
    }

    /// A weapon class slot's animation (idle, move-front, ready...).
    pub fn class_animation(&self, sel: UnitSelection, slot: WeaponClassSlot) -> Option<&Animation> {
        self.resolve(&self.weapon_class(sel)?.animations, slot)
    }

    /// A weapon type slot's animation (reload-1, fire-1, melee...).
    pub fn type_animation(&self, sel: UnitSelection, slot: WeaponTypeSlot) -> Option<&Animation> {
        self.resolve(&self.weapon_type(sel)?.animations, slot)
    }

    /// A weapon's own graph slot (the first and only weapons element).
    pub fn weapon_animation(&self, slot: WeaponSlot) -> Option<&Animation> {
        self.resolve(&self.weapons.first()?.animations, slot)
    }

    /// A first-person slot (the first and only element).
    pub fn first_person_animation(&self, slot: FirstPersonSlot) -> Option<&Animation> {
        self.resolve(&self.first_person_weapons.first()?.animations, slot)
    }

    pub fn vehicle_animation(&self, slot: VehicleSlot) -> Option<&Animation> {
        self.resolve(&self.vehicles.first()?.animations, slot)
    }

    pub fn device_animation(&self, slot: DeviceSlot) -> Option<&Animation> {
        self.resolve(&self.devices.first()?.animations, slot)
    }

    /// A damage reaction of the unit damage table.
    pub fn unit_damage_animation(&self, slot: UnitDamageSlot) -> Option<&Animation> {
        self.resolve(&self.unit_damage, slot)
    }

    /// The permutation of a slot to play, for a uniform random number `r`
    /// in `[0, 1)`: walk the chain from `first` (linked by next
    /// animation) and take the first animation whose cumulative weight is
    /// at least `r`; the last of the chain when none is. A chain that
    /// loops stops at its first repeat; an index past the block is none.
    pub fn choose_permutation(&self, first: usize, r: f32) -> Option<usize> {
        let mut i = first;
        let mut last = None;
        for _ in 0..=self.animations.len() {
            let a = self.animation(i)?;
            last = Some(i);
            if a.cumulative_weight >= r {
                return Some(i);
            }
            match a.next_animation {
                Some(n) if n != first && self.animation(n).is_some() => i = n,
                _ => break,
            }
        }
        last
    }
}

fn read_screen_bounds(v: &View<'_>, at: usize) -> io::Result<ScreenBounds> {
    Ok(ScreenBounds {
        right_yaw_per_frame: v.f32(at)?,
        left_yaw_per_frame: v.f32(at + 0x04)?,
        right_frame_count: v.i16(at + 0x08)?,
        left_frame_count: v.i16(at + 0x0A)?,
        down_pitch_per_frame: v.f32(at + 0x0C)?,
        up_pitch_per_frame: v.f32(at + 0x10)?,
        down_frame_count: v.i16(at + 0x14)?,
        up_frame_count: v.i16(at + 0x16)?,
    })
}

fn read_table<S>(v: &View<'_>, at: usize) -> io::Result<IndexTable<S>> {
    let block = v.block_max(at, 2, MAX_TABLE_ENTRIES)?;
    let entries = block.iter().map(|e| e.index16(0)).collect::<io::Result<Vec<_>>>()?;
    Ok(IndexTable::from_entries(entries))
}

fn read_ik_points(v: &View<'_>, at: usize) -> io::Result<Vec<IkPoint>> {
    v.block(at, IK_POINT_SIZE)?
        .iter()
        .map(|e| {
            Ok(IkPoint {
                marker: e.tag_string(0x00)?.to_owned(),
                attach_to_marker: e.tag_string(0x20)?.to_owned(),
            })
        })
        .collect()
}

fn read_object_overlay(v: &View<'_>) -> io::Result<ObjectOverlay> {
    Ok(ObjectOverlay {
        animation: v.index16(0x00)?,
        function: match v.enum16_below(0x02, 4)? {
            0 => ObjectFunction::A,
            1 => ObjectFunction::B,
            2 => ObjectFunction::C,
            _ => ObjectFunction::D,
        },
        mode: match v.enum16_below(0x04, 2)? {
            0 => ObjectOverlayMode::Frame,
            _ => ObjectOverlayMode::Scale,
        },
    })
}

fn read_unit_seat(v: &View<'_>) -> io::Result<UnitSeat> {
    Ok(UnitSeat {
        label: v.tag_string(0x00)?.to_owned(),
        looking_bounds: read_screen_bounds(v, 0x20)?,
        animations: read_table(v, 0x40)?,
        ik_points: read_ik_points(v, 0x4C)?,
        weapon_classes: v
            .block(0x58, WEAPON_CLASS_SIZE)?
            .iter()
            .map(|e| read_weapon_class(&e))
            .collect::<io::Result<_>>()?,
    })
}

fn read_weapon_class(v: &View<'_>) -> io::Result<WeaponClass> {
    Ok(WeaponClass {
        label: v.tag_string(0x00)?.to_owned(),
        grip_marker: v.tag_string(0x20)?.to_owned(),
        hand_marker: v.tag_string(0x40)?.to_owned(),
        aiming_bounds: read_screen_bounds(v, 0x60)?,
        animations: read_table(v, 0x98)?,
        ik_points: read_ik_points(v, 0xA4)?,
        weapon_types: v
            .block(0xB0, WEAPON_TYPE_SIZE)?
            .iter()
            .map(|e| {
                Ok(WeaponType {
                    label: e.tag_string(0x00)?.to_owned(),
                    animations: read_table(&e, 0x30)?,
                })
            })
            .collect::<io::Result<_>>()?,
    })
}

fn read_vehicle(v: &View<'_>) -> io::Result<GraphVehicle> {
    Ok(GraphVehicle {
        steering_bounds: read_screen_bounds(v, 0x00)?,
        animations: read_table(v, 0x5C)?,
        suspension: v
            .block(0x68, SUSPENSION_SIZE)?
            .iter()
            .map(|e| {
                Ok(Suspension {
                    mass_point_index: e.i16(0x00)?,
                    animation: e.index16(0x02)?,
                    full_extension_ground_depth: e.f32(0x04)?,
                    full_compression_ground_depth: e.f32(0x08)?,
                })
            })
            .collect::<io::Result<_>>()?,
    })
}

fn read_node(v: &View<'_>) -> io::Result<GraphNode> {
    Ok(GraphNode {
        name: v.tag_string(0x00)?.to_owned(),
        next_sibling: v.index16(0x20)?,
        first_child: v.index16(0x22)?,
        parent: v.index16(0x24)?,
        flags: v.flags32(0x28)?,
        base_vector: v.vector3(0x2C)?,
        vector_range: v.f32(0x38)?,
    })
}

/// A node flag set: two u32 words, node n at bit n mod 32 of word n / 32.
fn read_flag_words(v: &View<'_>, at: usize) -> io::Result<u64> {
    Ok(u64::from(v.u32(at)?) | u64::from(v.u32(at + 4)?) << 32)
}

fn read_animation(v: &View<'_>) -> io::Result<Animation> {
    let name = v.tag_string(0x00)?.to_owned();
    let kind = match v.enum16_below(0x20, 3)? {
        0 => AnimationType::Base,
        1 => AnimationType::Overlay,
        _ => AnimationType::Replacement,
    };
    let frame_count = v.i16(0x22)?;
    if frame_count < 0 {
        return Err(v.error(0x22, format_args!("animation `{name}` has {frame_count} frames")));
    }
    let frame_info_type = match v.enum16_below(0x26, 4)? {
        0 => FrameInfoType::None,
        1 => FrameInfoType::DxDy,
        2 => FrameInfoType::DxDyDyaw,
        _ => FrameInfoType::DxDyDzDyaw,
    };
    let node_count_raw = v.i16(0x2C)?;
    if !(0..=MAX_NODES as i16).contains(&node_count_raw) {
        return Err(v.error(
            0x2C,
            format_args!("animation `{name}` poses {node_count_raw} nodes (at most {MAX_NODES})"),
        ));
    }
    let frame_info = read_frame_info(v, &name, frame_count as usize, frame_info_type)?;
    Ok(Animation {
        kind,
        frame_count,
        frame_size: v.i16(0x24)?,
        frame_info_type,
        node_list_checksum: v.u32(0x28)?,
        node_count_raw,
        loop_frame: v.i16(0x2E)?,
        weight: v.f32(0x30)?,
        key_frame: v.i16(0x34)?,
        second_key_frame: v.i16(0x36)?,
        next_animation: v.index16(0x38)?,
        flags: v.flags16(0x3A)?,
        sound: v.index16(0x3C)?,
        sound_frame: v.i16(0x3E)?,
        left_foot_frame: v.u8(0x40)?,
        right_foot_frame: v.u8(0x41)?,
        first_permutation: v.index16(0x42)?,
        cumulative_weight: v.f32(0x44)?,
        frame_info,
        translation_flags: read_flag_words(v, 0x5C)?,
        rotation_flags: read_flag_words(v, 0x6C)?,
        scale_flags: read_flag_words(v, 0x7C)?,
        compressed_data_offset: v.i32(0x88)?,
        default_data: v.data(0x8C)?.to_vec(),
        frame_data: v.data(0xA0)?.to_vec(),
        name,
    })
}

fn read_frame_info(v: &View<'_>, name: &str, frames: usize, kind: FrameInfoType) -> io::Result<Vec<FrameInfo>> {
    let bytes = v.data(0x48)?;
    let size = kind.record_size();
    if bytes.len() != frames * size {
        return Err(v.error(
            0x48,
            format_args!(
                "animation `{name}`: {} bytes of frame info, {frames} frames of {size} bytes expected",
                bytes.len()
            ),
        ));
    }
    if size == 0 {
        return Ok(Vec::new());
    }
    let f = |r: &[u8], k: usize| f32::from_le_bytes([r[4 * k], r[4 * k + 1], r[4 * k + 2], r[4 * k + 3]]);
    Ok(bytes
        .chunks_exact(size)
        .map(|r| match kind {
            FrameInfoType::None => FrameInfo::default(),
            FrameInfoType::DxDy => FrameInfo {
                dx: f(r, 0),
                dy: f(r, 1),
                ..FrameInfo::default()
            },
            FrameInfoType::DxDyDyaw => FrameInfo {
                dx: f(r, 0),
                dy: f(r, 1),
                dyaw: f(r, 2),
                dz: 0.0,
            },
            FrameInfoType::DxDyDzDyaw => FrameInfo {
                dx: f(r, 0),
                dy: f(r, 1),
                dz: f(r, 2),
                dyaw: f(r, 3),
            },
        })
        .collect())
}
