//! The object tag readers and the object function evaluation on synthetic
//! maps: test case (a) of specs/halo_ce/objects.md (the synthetic biped
//! across the object, unit and biped layers, the rejections, the object
//! function examples), then the other classes and layers.

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision)]

use blam_formats::cache::{MapFile, class};
use blam_formats::object::function::evaluate_all;
use blam_formats::object::{
    Biped, BoundsMode, Control, Equipment, FunctionFlags, FunctionInput, FunctionOutput, Garbage, LightFixture, Machine,
    MachineType, ObjectBase, ObjectFlags, ObjectFunction, ObjectTag, ObjectType, Powerup, Projectile, Scenery,
    SoundScenery, TransitionFunction, UnitFlags, UnitInput, UnitTeam, Vehicle, attachment_function, object_time,
};
use blam_formats::shader::{PeriodicFunction, PeriodicTables};

use crate::map_fixture::{CacheFile, Struct, TagData, block_bytes, null_ref};

const SCNR: usize = 0;
const MODE: usize = 1;
const WEAP: usize = 2;
const TRAK: usize = 3;
const OBJ: usize = 4;

fn tag_list(class: &'static str) -> Vec<(&'static str, &'static str)> {
    vec![
        ("scnr", "scenarios\\syn"),
        ("mode", "characters\\test\\test"),
        ("weap", "weapons\\test\\gun"),
        ("trak", "camera\\test"),
        (class, "objects\\test\\thing"),
    ]
}

fn build(class: &'static str, main: impl FnOnce(&mut TagData) -> Vec<u8>) -> MapFile {
    let mut td = TagData::new(&tag_list(class));
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    let bytes = main(&mut td);
    td.set_struct(OBJ, &bytes);
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

/// A function element: period 2.0, cosine, bounds 0 to 1, turn off with -1,
/// the runtime fields (1.0, 0, 0, 0.5).
fn function_element(ty: i16) -> Vec<u8> {
    Struct::new(0x168)
        .f32(0x04, 2.0)
        .i16(0x0A, ty)
        .f32(0x28, 0.0)
        .f32(0x2C, 1.0)
        .i16(0x36, -1)
        .f32(0x138, 1.0)
        .f32(0x144, 0.5)
        .put(0x148, b"usage text")
        .into()
}

/// The spec's synthetic biped (test case (a)): a struct of 0x4F4 bytes with
/// the blocks after it.
fn synthetic_biped() -> MapFile {
    build("bipd", |td| {
        let weapons = td.block(&[
            Struct::new(0x24).put(0, &td.tag_ref(WEAP)).into(),
            Struct::new(0x24).put(0, &null_ref("weap")).into(),
        ]);
        let contacts = td.block(&[
            Struct::new(0x40).str(0x20, "left foot").into(),
            Struct::new(0x40).str(0x20, "right foot").into(),
        ]);
        let functions = td.block(&[function_element(2)]);
        let tracks = td.block(&[Struct::new(0x1C).put(0, &td.tag_ref(TRAK)).into()]);
        Struct::new(0x4F4)
            .i16(0x000, 0)
            .u16(0x002, 0x0005)
            .f32(0x004, 0.5)
            .f32(0x010, 0.25)
            .f32(0x020, 0.8)
            .u32(0x024, 1)
            .put(0x028, &td.tag_ref(MODE))
            .put(0x038, &null_ref("antr"))
            .f32(0x104, 0.5)
            .i16(0x108, 4)
            .i16(0x10A, 2)
            .i16(0x13C, 3)
            .put(0x158, &functions)
            .u32(0x17C, 0x8008)
            .i16(0x180, 1)
            .i16(0x19C, 5)
            .f32(0x1A0, 1.2217305)
            .put(0x1F4, &tracks)
            .f32(0x21C, 0.3)
            .f32(0x220, 1.0)
            .f32(0x2C0, 10.0)
            .i16(0x2C6, 4)
            .put(0x2D8, &weapons)
            .f32(0x2F0, 5.893977)
            .u32(0x2F4, 0x0202)
            .f32(0x2F8, 1.0471976)
            .f32(0x358, 0.7853982)
            .f32(0x364, 1.25)
            .f32(0x370, 0.65)
            .f32(0x3B4, 0.07)
            .f32(0x400, 0.62)
            .f32(0x404, 0.35)
            .f32(0x424, 0.7)
            .f32(0x42C, 0.2)
            .f32(0x4C8, 0.5)
            .put(0x4E8, &contacts)
            .into()
    })
}

fn tag_of(map: &MapFile) -> &blam_formats::cache::TagEntry {
    &map.tags()[OBJ]
}

#[test]
fn spec_biped_object_layer() {
    let map = synthetic_biped();
    let b = ObjectBase::read(&map, tag_of(&map)).unwrap();
    assert_eq!(b.object_type, ObjectType::Biped);
    assert!(b.flags.contains(ObjectFlags::DOES_NOT_CAST_SHADOW));
    assert!(b.flags.contains(ObjectFlags::BRIGHTER_THAN_IT_SHOULD_BE));
    assert!(!b.flags.contains(ObjectFlags::TRANSPARENT_SELF_OCCLUSION));
    assert_eq!(b.flags.unnamed_bits(), 0);
    assert_eq!(b.bounding_radius, 0.5);
    assert_eq!(b.bounding_offset, [0.0, 0.0, 0.25]);
    assert_eq!(b.origin_offset, [0.0; 3]);
    assert_eq!(b.acceleration_scale, 0.8);
    assert_eq!(b.runtime_flags, 1);
    assert_eq!(map.tag(b.model.id).unwrap().class, class::MODEL);
    assert_eq!(map.tag(b.model.id).unwrap().path, "characters\\test\\test");
    assert!(b.animation_graph.is_null());
    assert_eq!(b.render_bounding_radius, 0.5);
    assert_eq!(
        b.inputs(),
        [
            FunctionInput::RecentShieldDamage,
            FunctionInput::ShieldVitality,
            FunctionInput::None,
            FunctionInput::None
        ]
    );
    assert_eq!(b.hud_text_message_index, 3);
    assert_eq!(b.functions.len(), 1);
    let f = &b.functions[0];
    assert_eq!(f.period, 2.0);
    assert_eq!(f.function_type, PeriodicFunction::Cosine);
    assert_eq!((f.lower_bound, f.upper_bound), (0.0, 1.0));
    assert_eq!(f.turn_off_with, -1);
    assert_eq!(f.usage, "usage text");
    assert!(f.runtime_is_consistent());
}

#[test]
fn spec_biped_unit_layer() {
    let map = synthetic_biped();
    let b = Biped::read(&map, tag_of(&map)).unwrap();
    let u = &b.unit;
    assert!(u.flags.contains(UnitFlags::FIRES_FROM_CAMERA));
    assert!(u.flags.contains(UnitFlags::MELEE_ATTACKERS_CANNOT_ATTACH));
    assert_eq!(u.flags.bits(), 0x8008);
    assert_eq!(u.default_team, UnitTeam::Player);
    assert_eq!(u.input_c, UnitInput::IntegratedLightPower);
    assert_eq!(u.inputs()[0], UnitInput::None);
    assert!((u.camera_field_of_view - 1.2217305).abs() < 1e-6);
    assert!((u.soft_ping_minimum_interrupt_time - 0.3).abs() < 1e-6);
    assert_eq!(u.hard_ping_threshold, 1.0);
    assert_eq!(u.grenade_velocity, 10.0);
    assert_eq!(u.grenade_count, 4);
    assert_eq!(u.weapons.len(), 2);
    assert_eq!(map.tag(u.weapons[0].weapon.id).unwrap().path, "weapons\\test\\gun");
    assert!(u.weapons[1].weapon.is_null());
    assert_eq!(u.camera_tracks.len(), 1);
    assert_eq!(map.tag(u.camera_tracks[0].track.id).unwrap().class, class::CAMERA_TRACK);
    assert!(u.seats.is_empty());
}

#[test]
fn spec_biped_biped_layer() {
    let map = synthetic_biped();
    let b = Biped::read(&map, tag_of(&map)).unwrap();
    let p = &b.own;
    assert!((p.moving_turning_speed - 5.893977).abs() < 1e-6);
    assert!(p.flags.contains(blam_formats::object::BipedFlags::USES_PLAYER_PHYSICS));
    assert!(p.flags.contains(blam_formats::object::BipedFlags::USES_LIMP_BODY_PHYSICS));
    assert_eq!(p.flags.bits(), 0x202);
    assert!((p.stationary_turning_threshold - 1.0471976).abs() < 1e-6);
    assert!((p.jump_velocity - 0.07).abs() < 1e-7);
    assert!((p.jump_velocity_per_second() - 2.1).abs() < 1e-5);
    assert_eq!((p.standing_camera_height, p.crouching_camera_height), (0.62, 0.35));
    assert_eq!((p.standing_collision_height, p.collision_radius), (0.7, 0.2));
    assert_eq!(p.maximum_slope_angle, 0.7853982);
    assert_eq!((p.downhill_velocity_scale, p.uphill_velocity_scale), (1.25, 0.65));
    assert_eq!(p.runtime_cosine_stationary_turning_threshold, 0.5);
    let names: Vec<_> = p.contact_points.iter().map(|c| c.marker_name.as_str()).collect();
    assert_eq!(names, ["left foot", "right foot"]);
}

#[test]
fn layer_bases_and_struct_size() {
    use blam_formats::object::*;
    assert_eq!(OBJECT_BASE_SIZE, 0x17C);
    assert_eq!(UNIT_BASE_OFFSET, 0x17C);
    assert_eq!(UNIT_OWN_OFFSET, 0x2F0);
    assert_eq!(UNIT_OWN_OFFSET - UNIT_BASE_OFFSET, UNIT_BASE_SIZE);
    assert_eq!(UNIT_OWN_OFFSET + BIPED_OWN_SIZE, size::BIPED);
    assert_eq!(size::BIPED, 0x4F4);
    assert_eq!(ITEM_BASE_OFFSET + ITEM_BASE_SIZE, ITEM_OWN_OFFSET);
    assert_eq!(DEVICE_BASE_OFFSET + DEVICE_BASE_SIZE, DEVICE_OWN_OFFSET);
    assert_eq!(size::WEAPON, 0x508);
    assert_eq!(size::EQUIPMENT, 0x3B0);
    // A biped view shorter than the struct is refused.
    let mut td = TagData::new(&tag_list("bipd"));
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    td.set_struct(OBJ, &Struct::new(0x4F4).0);
    let map = MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap();
    Biped::read(&map, tag_of(&map)).unwrap();
}

#[test]
fn function_block_is_cut_to_four() {
    let map = build("scen", |td| {
        let functions = td.block(&(0..5).map(|i| function_element(2 + i)).collect::<Vec<_>>());
        Struct::new(0x1FC).put(0x158, &functions).into()
    });
    let b = ObjectBase::read(&map, tag_of(&map)).unwrap();
    assert_eq!(b.functions.len(), 4);
    assert_eq!(b.functions[3].function_type, PeriodicFunction::DiagonalWaveVariablePeriod);
}

#[test]
fn attachments_cut_to_eight_and_change_colors_to_four() {
    let map = build("scen", |td| {
        let atts: Vec<Vec<u8>> = (0..9)
            .map(|i| {
                Struct::new(0x48)
                    .put(0, &td.tag_ref(MODE))
                    .str(0x10, "flashlight")
                    .i16(0x30, 1)
                    .i16(0x32, 2)
                    .i16(0x34, i)
                    .into()
            })
            .collect();
        let attachments = td.block(&atts);
        let perms = td.block(&[
            Struct::new(0x1C).f32(0, 0.333).f32s(4, &[0.1, 0.2, 0.3]).f32s(0x10, &[0.4, 0.5, 0.6]).into(),
            Struct::new(0x1C).f32(0, 1.0).into(),
        ]);
        let colors: Vec<Vec<u8>> = (0..5)
            .map(|i| {
                Struct::new(0x2C)
                    .i16(0, 6)
                    .i16(2, 2)
                    .u32(4, 1)
                    .f32s(8, &[0.74, 0.98, 1.0])
                    .f32s(0x14, &[0.33, 0.51, 1.0])
                    .put(0x20, &if i == 0 { perms } else { block_bytes(0, 0) })
                    .into()
            })
            .collect();
        let colors = td.block(&colors);
        let resources = td.block(&[Struct::new(8).i16(0, 0).i16(2, 0).u32(4, 0xFFFF_FFFF).into()]);
        let widgets = td.block(&[Struct::new(0x20).put(0, &td.tag_ref(MODE)).into()]);
        Struct::new(0x1FC)
            .put(0x140, &attachments)
            .put(0x14C, &widgets)
            .put(0x164, &colors)
            .put(0x170, &resources)
            .into()
    });
    let b = ObjectBase::read(&map, tag_of(&map)).unwrap();
    assert_eq!(b.attachments.len(), 8);
    assert_eq!(b.attachments[7].change_color, 7);
    assert_eq!(b.attachments[0].marker_name, "flashlight");
    assert_eq!((b.attachments[0].primary_scale, b.attachments[0].secondary_scale), (1, 2));
    assert_eq!(b.widgets.len(), 1);
    assert_eq!(b.change_colors.len(), 4);
    let c0 = &b.change_colors[0];
    assert_eq!((c0.darken_by, c0.scale_by), (6, 2));
    assert!(c0.blends_in_hsv() && !c0.long_way_round());
    assert_eq!(c0.color_lower_bound.r, 0.74);
    assert_eq!(c0.color_upper_bound.g, 0.51);
    assert_eq!(c0.permutations.len(), 2);
    assert_eq!(c0.permutations[0].weight, 0.333);
    assert_eq!(c0.permutations[0].color_lower_bound.b, 0.3);
    assert_eq!(c0.permutations[0].color_upper_bound.r, 0.4);
    assert_eq!(c0.permutations[1].weight, 1.0);
    assert!(b.change_colors[1].permutations.is_empty());
    assert_eq!(b.predicted_resources.len(), 1);
    assert!(b.predicted_resources[0].tag.is_none());
}

#[test]
fn rejections() {
    // A weapons block whose third element lies past the tag data.
    let mut td = TagData::new(&tag_list("bipd"));
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    let main = Struct::new(0x4F4);
    let end = td.set_struct(OBJ, &main.0) + 0x4F4;
    td.heap.patch(end - 0x4F4 + 0x2D8, &block_bytes(3, end - 0x24));
    let map = MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap();
    let err = Biped::read(&map, tag_of(&map)).unwrap_err();
    assert!(err.to_string().contains("outside"), "{err}");

    // Object type 12.
    let map = build("scen", |_| Struct::new(0x1FC).i16(0, 12).into());
    let err = ObjectBase::read(&map, tag_of(&map)).unwrap_err();
    assert!(err.to_string().contains("ObjectType code 12"), "{err}");

    // Input A = 40.
    let map = build("scen", |_| Struct::new(0x1FC).i16(0, 6).i16(0x108, 40).into());
    let err = ObjectBase::read(&map, tag_of(&map)).unwrap_err();
    assert!(err.to_string().contains("FunctionInput code 40"), "{err}");

    // A type code that is not the class's.
    let map = build("scen", |_| Struct::new(0x1FC).i16(0, 0).into());
    let err = Scenery::read(&map, tag_of(&map)).unwrap_err();
    assert!(err.to_string().contains("object type"), "{err}");

    // A tag that is not an object.
    let map = build("scen", |_| Struct::new(0x1FC).i16(0, 6).into());
    let err = ObjectTag::read(&map, &map.tags()[MODE]).unwrap_err();
    assert!(err.to_string().contains("not an object"), "{err}");
    let err = Biped::read(&map, tag_of(&map)).unwrap_err();
    assert!(err.to_string().contains("not a biped"), "{err}");
}

#[test]
fn dispatch_by_class() {
    // The biped through the enum, with the object layer whatever the class.
    let map = synthetic_biped();
    let t = ObjectTag::read(&map, tag_of(&map)).unwrap();
    assert!(matches!(t, ObjectTag::Biped(_)));
    assert_eq!(t.object().bounding_radius, 0.5);
    assert_eq!(t.class(), class::BIPED);
    assert_eq!(t.object_type(), ObjectType::Biped);

    let one = |class: &'static str, size: usize, otype: i16| {
        let map = build(class, |_| Struct::new(size).i16(0, otype).f32(4, 1.5).into());
        ObjectTag::read(&map, tag_of(&map)).unwrap()
    };
    assert!(matches!(one("scen", 0x1FC, 6), ObjectTag::Scenery(_)));
    assert!(matches!(one("ssce", 0x1FC, 11), ObjectTag::SoundScenery(_)));
    assert!(matches!(one("proj", 0x24C, 5), ObjectTag::Projectile(_)));
    assert!(matches!(one("vehi", 0x3F0, 1), ObjectTag::Vehicle(_)));
    assert!(matches!(one("garb", 0x3B0, 4), ObjectTag::Garbage(_)));
    assert!(matches!(one("eqip", 0x3B0, 3), ObjectTag::Equipment(_)));
    assert!(matches!(one("mach", 0x324, 7), ObjectTag::Machine(_)));
    assert!(matches!(one("ctrl", 0x324, 8), ObjectTag::Control(_)));
    assert!(matches!(one("lifi", 0x2D0, 9), ObjectTag::LightFixture(_)));
    assert!(matches!(one("weap", 0x508, 2), ObjectTag::Weapon(_)));
    for (cls, size, otype) in [("scen", 0x1FC, 6), ("weap", 0x508, 2), ("eqip", 0x3B0, 3), ("mach", 0x324, 7)] {
        assert_eq!(one(cls, size, otype).object().bounding_radius, 1.5);
    }
    // Type codes and classes pair up.
    for t in ObjectType::ALL {
        assert_eq!(ObjectType::from_class(t.class()), Some(*t));
    }
    assert_eq!(ObjectType::from_code(12), None);
    assert!(ObjectType::Vehicle.is_unit() && ObjectType::Garbage.is_item() && ObjectType::Control.is_device());
    assert!(!ObjectType::Scenery.is_unit());
}

#[test]
fn vehicle_and_projectile_stop_at_their_offsets() {
    let map = build("vehi", |td| {
        let seats = td.block(&[
            Struct::new(0x11C)
                .u32(0, 0b100)
                .str(4, "driver")
                .str(0x24, "driver seat")
                .f32s(0x64, &[200.0, 200.0, 150.0])
                .f32(0x7C, 60.0)
                .f32(0x80, 30.0)
                .f32(0xF0, -1.0)
                .f32(0xF4, 1.0)
                .put(0xF8, &null_ref("actv"))
                .into(),
        ]);
        let powered = td.block(&[Struct::new(0x44).f32(4, 1.0).f32(8, 0.1).into()]);
        Struct::new(0x3F0)
            .i16(0, 1)
            .u32(0x17C, 0x41)
            .f32(0x184, 0.18)
            .i16(0x298, 2)
            .i16(0x198, 1)
            .put(0x2CC, &powered)
            .put(0x2E4, &seats)
            .into()
    });
    let v = Vehicle::read(&map, tag_of(&map)).unwrap();
    assert!(v.unit.flags.contains(UnitFlags::CIRCULAR_AIMING));
    assert_eq!(v.unit.rider_damage_fraction, 0.18);
    assert_eq!(v.unit.motion_sensor_blip_size, blam_formats::object::BlipSize::Large);
    assert_eq!(v.unit.input_a, UnitInput::DriverSeatPower);
    assert_eq!(v.unit.powered_seats[0].power_up_time, 1.0);
    assert_eq!(v.unit.powered_seats[0].power_down_time, 0.1);
    let s = &v.unit.seats[0];
    assert!(s.flags.contains(blam_formats::object::SeatFlags::DRIVER));
    assert_eq!((s.label.as_str(), s.marker_name.as_str()), ("driver", "driver seat"));
    assert_eq!(s.acceleration_scale, [200.0, 200.0, 150.0]);
    assert_eq!((s.yaw_rate, s.pitch_rate, s.yaw_minimum, s.yaw_maximum), (60.0, 30.0, -1.0, 1.0));
    assert!(s.built_in_actor.is_null());
    let _ = Projectile::read;
}

#[test]
fn item_layer_and_equipment() {
    let map = build("eqip", |td| {
        Struct::new(0x3B0)
            .i16(0, 3)
            .u16(2, 4)
            .f32(4, 0.1)
            .u32(0x17C, 2)
            .i16(0x180, 30)
            .i16(0x182, 2)
            .f32(0x184, 1.75)
            .i16(0x188, 100)
            .put(0x258, &null_ref("snd!"))
            .f32(0x2E0, 0.4)
            .f32(0x2E4, 0.8)
            .put(0x2E8, &null_ref("effe"))
            .put(0x2F8, &td.tag_ref(MODE))
            .i16(0x308, 6)
            .i16(0x30A, 1)
            .f32(0x30C, 45.0)
            .put(0x310, &null_ref("snd!"))
            .into()
    });
    let e = Equipment::read(&map, tag_of(&map)).unwrap();
    assert!(e.object.flags.contains(ObjectFlags::BRIGHTER_THAN_IT_SHOULD_BE));
    assert!(e.item.flags.contains(blam_formats::object::ItemFlags::DESTROYED_BY_EXPLOSIONS));
    assert_eq!((e.item.message_index, e.item.sort_order, e.item.scale), (30, 2, 1.75));
    assert_eq!(e.item.hud_message_value_scale, 100);
    assert_eq!((e.item.detonation_delay_minimum, e.item.detonation_delay_maximum), (0.4, 0.8));
    assert!(e.item.detonating_effect.is_null());
    assert!(!e.item.detonation_effect.is_null());
    assert_eq!(e.own.powerup_type, Powerup::Grenade);
    assert_eq!(e.own.grenade_type, blam_formats::object::GrenadeType::Plasma);
    assert_eq!(e.own.powerup_time, 45.0);
    assert!(e.own.pickup_sound.is_null());

    // A powerup type outside 0 to 6 is refused.
    let map = build("eqip", |_| Struct::new(0x3B0).i16(0, 3).i16(0x308, 7).into());
    assert!(Equipment::read(&map, tag_of(&map)).is_err());

    // Garbage has the item layer only.
    let map = build("garb", |_| Struct::new(0x3B0).i16(0, 4).u32(0x17C, 1).into());
    let g = Garbage::read(&map, tag_of(&map)).unwrap();
    assert!(g.item.flags.contains(blam_formats::object::ItemFlags::ALWAYS_MAINTAINS_Z_UP));
}

#[test]
fn devices() {
    let map = build("mach", |td| {
        Struct::new(0x324)
            .i16(0, 7)
            .u32(0x17C, 1)
            .f32(0x180, 0.5)
            .i16(0x198, 1)
            .i16(0x19A, 3)
            .put(0x1A0, &td.tag_ref(MODE))
            .f32(0x200, 2.0)
            .f32(0x21C, 3.0)
            .f32(0x274, 0.25)
            .f32(0x28C, 60.0)
            .i16(0x290, 1)
            .u16(0x292, 0b101)
            .f32(0x294, 2.0)
            .i16(0x290 + 0x58, 4)
            .i16(0x290 + 0x5A, 9)
            .i32(0x290 + 0x90, 60)
            .into()
    });
    let m = Machine::read(&map, tag_of(&map)).unwrap();
    assert!(m.device.flags.contains(blam_formats::object::DeviceFlags::POSITION_LOOPS));
    assert_eq!(m.device.power_transition_time, 0.5);
    assert_eq!(m.device.input_a, blam_formats::object::DeviceInput::Power);
    assert_eq!(m.device.input_b, blam_formats::object::DeviceInput::Position);
    assert!(!m.device.positive_start_effect.is_null());
    assert_eq!((m.device.delay_time, m.device.automatic_activation_radius), (2.0, 3.0));
    assert_eq!((m.device.runtime_maximum_power_acceleration, m.device.runtime_delay_ticks), (0.25, 60.0));
    assert_eq!(m.own.machine_type, MachineType::Platform);
    assert!(m.own.flags.contains(blam_formats::object::MachineFlags::IS_PATHFINDING_OBSTACLE));
    assert!(m.own.flags.contains(blam_formats::object::MachineFlags::IS_ELEVATOR));
    assert_eq!(m.own.door_open_time, 2.0);
    assert_eq!((m.own.collision_response, m.own.elevator_node, m.own.runtime_door_open_ticks), (4, 9, 60));

    let map = build("ctrl", |_| {
        Struct::new(0x324).i16(0, 8).i16(0x290, 1).i16(0x292, 2).f32(0x294, 5.0).into()
    });
    let c = Control::read(&map, tag_of(&map)).unwrap();
    assert_eq!((c.own.control_type, c.own.triggers_when, c.own.call_value), (1, 2, 5.0));
    assert!(c.own.on_effect.class.as_u32() == 0);

    let map = build("lifi", |_| Struct::new(0x2D0).i16(0, 9).f32(0x21C, 1.0).into());
    let l = LightFixture::read(&map, tag_of(&map)).unwrap();
    assert_eq!(l.device.automatic_activation_radius, 1.0);

    // A machine type outside the table is refused.
    let map = build("mach", |_| Struct::new(0x324).i16(0, 7).i16(0x290, 3).into());
    assert!(Machine::read(&map, tag_of(&map)).is_err());
    let _ = SoundScenery::read;
}

// Object functions.

fn tables() -> PeriodicTables {
    PeriodicTables::new()
}

/// Period 2.0 (stored 1 / period 0.5), bounds 0 to 1, mode 0, linear.
fn periodic(ty: PeriodicFunction) -> ObjectFunction {
    ObjectFunction {
        period: 2.0,
        function_type: ty,
        ..ObjectFunction::default()
    }
    .with_runtime()
}

fn at(f: &ObjectFunction, t: f32) -> FunctionOutput {
    f.evaluate(&tables(), 0, t, &[0.0; 4], &[FunctionOutput::default(); 4])
}

fn check(f: &ObjectFunction, t: f32, value: f32, active: bool) {
    let o = at(f, t);
    assert!((o.value - value).abs() < 1e-6, "t {t}: {} vs {value}", o.value);
    assert_eq!(o.active, active, "t {t}");
}

#[test]
fn periodic_function_examples() {
    let one = periodic(PeriodicFunction::One);
    for t in [0.0, 1.0, 123.4] {
        check(&one, t, 1.0, true);
    }
    let cos = periodic(PeriodicFunction::Cosine);
    check(&cos, 0.0, 1.0, true);
    check(&cos, 2.5, 217.0 / 255.0, true);
    check(&cos, 5.0, 127.0 / 255.0, true);
    check(&cos, 7.5, 37.0 / 255.0, true);
    check(&cos, 10.0, 0.0, false);
    let diag = periodic(PeriodicFunction::DiagonalWave);
    check(&diag, 0.0, 0.0, false);
    check(&diag, 2.5, 63.0 / 255.0, true);
    check(&diag, 5.0, 127.0 / 255.0, true);
    check(&diag, 10.0, 1.0, true);
    let slide = periodic(PeriodicFunction::Slide);
    check(&slide, 2.5, 223.0 / 255.0, true);
    check(&slide, 5.0, 191.0 / 255.0, true);
    check(&slide, 7.5, 159.0 / 255.0, true);

    let mut inv = periodic(PeriodicFunction::Cosine);
    inv.flags = FunctionFlags::INVERT;
    check(&inv, 2.5, 1.0 - 217.0 / 255.0, true);
    let mut sq = periodic(PeriodicFunction::Cosine);
    sq.square_wave_threshold = 0.5;
    check(&sq, 2.5, 1.0, true);
    check(&sq, 5.0, 0.0, false);
}

/// Type one, period 1.0, scale function by input A.
fn chain() -> ObjectFunction {
    ObjectFunction {
        period: 1.0,
        scale_function_by: 1,
        ..ObjectFunction::default()
    }
    .with_runtime()
}

fn with_a(f: &ObjectFunction, a: f32) -> FunctionOutput {
    f.evaluate(&tables(), 0, 0.0, &[a, 0.0, 0.0, 0.0], &[FunctionOutput::default(); 4])
}

fn is(o: FunctionOutput, value: f32, active: bool) {
    assert!((o.value - value).abs() < 1e-6, "{} vs {value}", o.value);
    assert_eq!(o.active, active);
}

#[test]
fn function_chains() {
    let f = chain();
    is(with_a(&f, 0.4), 0.4, true);
    is(with_a(&f, 0.0), 0.0, false);

    let mut always = chain();
    always.flags = FunctionFlags::ALWAYS_ACTIVE;
    is(with_a(&always, 0.0), 0.0, true);

    let mut fit = ObjectFunction {
        bounds_mode: BoundsMode::ScaleToFit,
        lower_bound: 0.2,
        upper_bound: 0.8,
        ..chain()
    }
    .with_runtime();
    is(with_a(&fit, 0.5), 0.5, true);
    is(with_a(&fit, 0.0), 0.2, false);
    fit.flags = FunctionFlags::ALWAYS_ACTIVE;
    is(with_a(&fit, 0.0), 0.2, true);

    let mut norm = ObjectFunction {
        bounds_mode: BoundsMode::ClipAndNormalize,
        lower_bound: 0.2,
        upper_bound: 0.8,
        ..chain()
    }
    .with_runtime();
    assert!((norm.runtime_inverse_range - 1.6666666).abs() < 1e-5);
    norm.runtime_inverse_range = 1.6667;
    let o = with_a(&norm, 0.5);
    assert!((o.value - 0.5).abs() < 1e-4 && o.active);

    let scaled = ObjectFunction { scale_by: 0.5, ..chain() }.with_runtime();
    is(with_a(&scaled, 0.4), 0.2, true);

    let stepped = ObjectFunction { step_count: 9, ..chain() }.with_runtime();
    assert_eq!(stepped.runtime_step_factor, 0.125);
    is(with_a(&stepped, 0.3), 0.25, true);

    let late = ObjectFunction {
        map_to: TransitionFunction::Late,
        ..chain()
    }
    .with_runtime();
    let o = with_a(&late, 0.5);
    assert!((o.value - 0.25).abs() < 0.01 && o.active);

    // Add the outgoing function A (input 5), computed as function B.
    let add = ObjectFunction { add: 5, ..chain() }.with_runtime();
    let prev = [
        FunctionOutput { value: 0.5, active: true },
        FunctionOutput::default(),
        FunctionOutput::default(),
        FunctionOutput::default(),
    ];
    let o = add.evaluate(&tables(), 1, 0.0, &[0.4, 0.0, 0.0, 0.0], &prev);
    is(o, 0.9, true);
    // The sum is capped at 1.
    let o = add.evaluate(&tables(), 1, 0.0, &[0.8, 0.0, 0.0, 0.0], &prev);
    is(o, 1.0, true);
}

#[test]
fn additive_accumulates_modulo_one() {
    let f = ObjectFunction {
        flags: FunctionFlags::ADDITIVE,
        ..chain()
    };
    let mut out = [FunctionOutput::default(); 4];
    let mut seen = Vec::new();
    for _ in 0..4 {
        out = evaluate_all(std::slice::from_ref(&f), &tables(), 0.0, &[0.3, 0.0, 0.0, 0.0], out);
        seen.push(out[0].value);
        assert!(out[0].active);
    }
    for (got, want) in seen.iter().zip([0.3, 0.6, 0.9, 0.2]) {
        assert!((got - want).abs() < 1e-6, "{seen:?}");
    }
}

#[test]
fn turn_off_with_follows_the_named_function() {
    let a = chain();
    let b = ObjectFunction {
        turn_off_with: 0,
        ..chain()
    };
    let fs = [a, b];
    let out = evaluate_all(&fs, &tables(), 0.0, &[0.0; 4], [FunctionOutput::default(); 4]);
    assert_eq!((out[0].value, out[0].active), (0.0, false));
    assert_eq!((out[1].value, out[1].active), (0.0, false));
    let out = evaluate_all(&fs, &tables(), 0.0, &[0.7, 0.0, 0.0, 0.0], [FunctionOutput::default(); 4]);
    assert!((out[0].value - 0.7).abs() < 1e-6 && out[0].active);
    assert!((out[1].value - 0.7).abs() < 1e-6 && out[1].active);
    // A turn off index outside 0..3 is none.
    let odd = ObjectFunction {
        turn_off_with: 9,
        ..chain()
    };
    assert!(with_a(&odd, 0.7).active);
}

#[test]
fn inputs_and_periods() {
    // Scale period by an input: a larger value lengthens the period.
    let mut f = periodic(PeriodicFunction::DiagonalWave);
    f.scale_period_by = 1;
    let t = tables();
    let base = at(&periodic(PeriodicFunction::DiagonalWave), 2.5).value;
    let half = f.evaluate(&t, 0, 2.5, &[1.0, 0.0, 0.0, 0.0], &[FunctionOutput::default(); 4]).value;
    assert_eq!(base, half, "an input of 1 leaves the period");
    let zero = f.evaluate(&t, 0, 2.5, &[0.0; 4], &[FunctionOutput::default(); 4]).value;
    assert_eq!(base, zero, "an input of 0 leaves the period");
    let double = f.evaluate(&t, 0, 5.0, &[2.0, 0.0, 0.0, 0.0], &[FunctionOutput::default(); 4]).value;
    assert_eq!(base, double, "twice the period, twice the time");
    // An input index outside 0..8 reads 0.
    let mut g = chain();
    g.scale_function_by = 99;
    is(with_a(&g, 0.5), 0.0, false);
    // A period of 0 holds the argument at 0.
    let mut z = periodic(PeriodicFunction::Cosine);
    z.period = 0.0;
    let z = z.with_runtime();
    assert_eq!(z.runtime_inverse_period, 0.0);
    check(&z, 100.0, 1.0, true);
    // The clock.
    assert_eq!(object_time(0, 30), 1.0);
    assert_eq!(object_time(2, 6), 4.0);
    // A consumer asking for function NONE gets 1.0, active.
    let outs = [FunctionOutput { value: 0.3, active: false }; 4];
    assert_eq!(attachment_function(&outs, 0), FunctionOutput { value: 1.0, active: true });
    assert_eq!(attachment_function(&outs, 3).value, 0.3);
}

#[test]
fn transitions() {
    use TransitionFunction::*;
    for v in [0.0f32, 0.25, 0.5, 1.0] {
        assert_eq!(Linear.apply(v), v);
        assert!((Early.apply(v) - v.sqrt()).abs() < 1e-6);
        assert!((Late.apply(v) - v * v).abs() < 1e-6);
        assert!((VeryLate.apply(v) - v.powi(4)).abs() < 1e-6);
        assert!((VeryEarly.apply(v) - v.powf(0.25)).abs() < 1e-6);
    }
    assert!((Cosine.apply(0.5) - 0.5).abs() < 1e-6);
    assert!(Cosine.apply(0.0).abs() < 1e-6 && (Cosine.apply(1.0) - 1.0).abs() < 1e-6);
}

#[test]
fn lenient_enumerations_in_a_tag() {
    // Periodic type 12 reads as one, map to 6 as linear; bounds mode 3 is
    // refused.
    let build_fn = |ty: i16, map_to: i16, mode: i16| {
        build("scen", |td| {
            let functions = td.block(&[Struct::new(0x168)
                .i16(0x0A, ty)
                .i16(0x1E, map_to)
                .i16(0x26, mode)
                .i16(0x36, -1)
                .into()]);
            Struct::new(0x1FC).i16(0, 6).put(0x158, &functions).into()
        })
    };
    let map = build_fn(12, 6, 0);
    let b = ObjectBase::read(&map, tag_of(&map)).unwrap();
    assert_eq!(b.functions[0].function_type, PeriodicFunction::One);
    assert_eq!(b.functions[0].map_to, TransitionFunction::Linear);
    let map = build_fn(2, 5, 3);
    assert!(ObjectBase::read(&map, tag_of(&map)).is_err());
}
