//! The device layer and the machine's and control's own parts
//! (specs/halo_ce/objects.md, "Scenery, sound scenery and devices"): the
//! device layer is 0x17C to 0x290 (absolute offsets), the machine's and the
//! control's parts 0x290 to 0x324 (offsets relative to 0x290, as the spec
//! gives them). Their behaviour belongs to a later spec.

use std::io;

use super::{DEVICE_OWN_OFFSET, bit_flags};
use crate::cache::{TagRef, TagStruct, View};
use crate::scenario::coded_enum;

bit_flags! {
    /// The device flags (offset 0x17C).
    pub struct DeviceFlags(u32) {
        POSITION_LOOPS = 0,
        POSITION_ANIMATION_NOT_INTERPOLATED = 1,
    }
}

bit_flags! {
    /// The machine flags.
    pub struct MachineFlags(u16) {
        IS_PATHFINDING_OBSTACLE = 0,
        NOT_AN_OBSTACLE_WHEN_OPEN = 1,
        IS_ELEVATOR = 2,
    }
}

coded_enum! {
    /// Where a device-layer incoming function value comes from.
    pub enum DeviceInput {
        None = 0,
        Power = 1,
        ChangeInPower = 2,
        Position = 3,
        ChangeInPosition = 4,
        Locked = 5,
        Delay = 6,
    }
}

coded_enum! {
    /// A machine's type.
    pub enum MachineType {
        Door = 0,
        Platform = 1,
        Gear = 2,
    }
}

crate::tag_struct! {
    /// The device layer (`SIZE` is its end, 0x290).
    pub struct DeviceBase: 0x290 {
        0x17C flags: DeviceFlags,
        /// Seconds.
        0x180 power_transition_time: f32,
        0x184 power_acceleration_time: f32,
        0x188 powered_position_transition_time: f32,
        0x18C powered_position_acceleration_time: f32,
        0x190 depowered_position_transition_time: f32,
        0x194 depowered_position_acceleration_time: f32,
        0x198 input_a: DeviceInput,
        0x19A input_b: DeviceInput,
        0x19C input_c: DeviceInput,
        0x19E input_d: DeviceInput,
        /// Each `snd!` or `effe`.
        0x1A0 positive_start_effect: TagRef,
        0x1B0 negative_start_effect: TagRef,
        0x1C0 positive_stop_effect: TagRef,
        0x1D0 negative_stop_effect: TagRef,
        0x1E0 depowered_effect: TagRef,
        0x1F0 repowered_effect: TagRef,
        /// Seconds.
        0x200 delay_time: f32,
        0x20C delay_effect: TagRef,
        /// World units.
        0x21C automatic_activation_radius: f32,
        // The runtime fields, filled by the build (per tick).
        0x274 runtime_maximum_power_acceleration: f32,
        0x278 runtime_power_velocity: f32,
        0x27C runtime_depowered_position_acceleration: f32,
        0x280 runtime_depowered_position_velocity: f32,
        0x284 runtime_powered_position_acceleration: f32,
        0x288 runtime_powered_position_velocity: f32,
        0x28C runtime_delay_ticks: f32,
    }
}

impl DeviceBase {
    /// Read the layer from a view of the tag's main struct.
    pub fn read_view(v: &View<'_>) -> io::Result<DeviceBase> {
        <DeviceBase as TagStruct>::read(v)
    }
}

crate::tag_struct! {
    /// A machine's own part (offsets relative to 0x290; 0x94 bytes).
    pub struct MachinePart: 0x94 {
        0x00 machine_type: MachineType,
        0x02 flags: MachineFlags,
        /// Seconds.
        0x04 door_open_time: f32,
        /// As stored (the spec lists no values).
        0x58 collision_response: i16,
        0x5A elevator_node: i16,
        /// Runtime.
        0x90 runtime_door_open_ticks: i32,
    }
}

impl MachinePart {
    /// Read the part from a view of the tag's main struct.
    pub fn read_view(v: &View<'_>) -> io::Result<MachinePart> {
        <MachinePart as TagStruct>::read(&v.sub(DEVICE_OWN_OFFSET, 0x94)?)
    }
}

crate::tag_struct! {
    /// A control's own part (offsets relative to 0x290; from the reference
    /// only, no disc tag).
    pub struct ControlPart: 0x94 {
        /// As stored (the spec lists no values).
        0x00 control_type: i16,
        0x02 triggers_when: i16,
        0x04 call_value: f32,
        0x58 on_effect: TagRef,
        0x68 off_effect: TagRef,
        0x78 denied_effect: TagRef,
    }
}

impl ControlPart {
    /// Read the part from a view of the tag's main struct.
    pub fn read_view(v: &View<'_>) -> io::Result<ControlPart> {
        <ControlPart as TagStruct>::read(&v.sub(DEVICE_OWN_OFFSET, 0x94)?)
    }
}
