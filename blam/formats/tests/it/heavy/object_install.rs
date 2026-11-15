//! The object readers on the user's Halo disc image, against section (b) and
//! "The disc's values" of specs/halo_ce/objects.md: the multiplayer Spartan,
//! the 14 equipment tags, the object base of the 15 weapons and 3 vehicles,
//! a sweep of every object-derived tag of `bloodgulch`, and the same
//! values on other multiplayer maps. Each test passes with a note when
//! there is no image.

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision)]

use std::fmt::Debug;

use blam_formats::cache::{ClassCode, MapFile, TagRef, class};
use blam_formats::object::{
    Biped, Equipment, FunctionInput, GrenadeType, ObjectBase, ObjectTag, ObjectType, Powerup, UnitFlags, UnitInput,
    UnitTeam, Vehicle,
};
use blam_formats::shader::PeriodicFunction;

use super::maps::load;

pub(crate) const SPARTAN: &str = "characters\\cyborg_mp\\cyborg_mp";

pub(crate) fn near(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() <= 1e-5 * b.abs().max(1.0), "{what}: {a} vs {b}");
}

/// The path a reference resolves to, `None` for a null one; the class the
/// reference carries must be the target's own.
pub(crate) fn path_of(map: &MapFile, r: &TagRef) -> Option<String> {
    if r.is_null() {
        return None;
    }
    let t = map.tag(r.id).unwrap_or_else(|| panic!("unresolved {r:?}"));
    assert_eq!(t.class, r.class, "{}: reference class", t.path);
    Some(t.path.clone())
}

/// A debug rendering with every tag id replaced by the path it names and
/// the path addresses dropped, comparable between maps.
pub(crate) fn normalized(map: &MapFile, v: &impl Debug) -> String {
    let s = format!("{v:?}");
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(i) = rest.find("path_address: ") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + "path_address: ".len()..];
        let end = tail.find(", ").unwrap();
        rest = &tail[end + 2..];
    }
    out.push_str(rest);
    let s = out;
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(i) = rest.find("TagId(0x") {
        out.push_str(&rest[..i]);
        let hex = &rest[i + 8..i + 16];
        let id = blam_formats::cache::TagId(u32::from_str_radix(hex, 16).unwrap());
        match map.tag(id) {
            Some(t) => out.push_str(&format!("<{}>", t.path)),
            None => out.push_str(&format!("<none {id}>")),
        }
        rest = &rest[i + 17..];
    }
    out.push_str(rest);
    out
}

fn biped(map: &MapFile, path: &str) -> Biped {
    Biped::read(map, map.find(class::BIPED, path).unwrap_or_else(|| panic!("no {path}"))).unwrap()
}

#[test]
fn the_multiplayer_spartan() {
    let Some(map) = load("bloodgulch") else { return };
    let tag = map.find(class::BIPED, SPARTAN).unwrap();
    assert_eq!(tag.id.0, 0xE20A_0096);
    assert_eq!(tag.address, 0x8072_917C);
    assert_eq!(tag.index(), 150);
    let b = Biped::read(&map, tag).unwrap();
    let o = &b.object;
    assert_eq!(o.object_type, ObjectType::Biped);
    assert_eq!(o.flags.bits(), 0);
    near(o.bounding_radius, 0.42, "radius");
    assert_eq!(o.bounding_offset, [0.0; 3]);
    assert_eq!(o.origin_offset, [0.0; 3]);
    near(o.acceleration_scale, 0.7, "acceleration scale");
    assert_eq!(o.runtime_flags, 1);
    assert_eq!(path_of(&map, &o.model).as_deref(), Some("characters\\cyborg\\cyborg"));
    assert_eq!(o.model.class, class::MODEL);
    assert_eq!(path_of(&map, &o.animation_graph).as_deref(), Some("characters\\cyborg\\cyborg"));
    assert_eq!(o.animation_graph.class, class::MODEL_ANIMATIONS);
    assert_eq!(path_of(&map, &o.collision_model).as_deref(), Some("characters\\cyborg\\cyborg"));
    assert_eq!(o.collision_model.class, class::MODEL_COLLISION_GEOMETRY);
    assert_eq!(path_of(&map, &o.physics), None);
    assert_eq!(o.physics.class, class::PHYSICS);
    assert_eq!(
        path_of(&map, &o.modifier_shader).as_deref(),
        Some("characters\\cyborg\\shaders\\shield hit")
    );
    assert_eq!(o.modifier_shader.class, class::SHADER_TRANSPARENT_PLASMA);
    assert_eq!(path_of(&map, &o.creation_effect), None);
    near(o.render_bounding_radius, 0.42, "render radius");
    assert_eq!(
        o.inputs(),
        [FunctionInput::RecentShieldDamage, FunctionInput::ShieldVitality, FunctionInput::None, FunctionInput::None]
    );
    assert_eq!((o.hud_text_message_index, o.forced_shader_permutation), (0, 0));
    assert_eq!(
        (
            o.attachments.len(),
            o.widgets.len(),
            o.functions.len(),
            o.change_colors.len(),
            o.predicted_resources.len()
        ),
        (1, 0, 2, 2, 5)
    );
    let predicted: Vec<_> = o
        .predicted_resources
        .iter()
        .map(|p| {
            assert_eq!(p.resource_index, 0);
            let t = map.tag(p.tag).unwrap();
            assert_eq!(t.class, class::BITMAP);
            t.path.clone()
        })
        .collect();
    assert_eq!(
        predicted,
        [
            "characters\\cyborg\\bitmaps\\cyborg",
            "characters\\cyborg\\bitmaps\\cyborg multipurpose",
            "characters\\cyborg\\bitmaps\\detail corroded metal light",
            "characters\\cyborg\\bitmaps\\cyborg reflection armor",
            "characters\\cyborg\\bitmaps\\cubemap mirror surface",
        ]
    );

    let a = &o.attachments[0];
    assert_eq!(path_of(&map, &a.attached).as_deref(), Some("characters\\cyborg\\flashlight_cyborg"));
    assert_eq!(a.attached.class, class::LIGHT);
    assert_eq!(a.marker_name, "flashlight");
    assert_eq!((a.primary_scale, a.secondary_scale, a.change_color), (1, 0, 0));

    for f in &o.functions {
        assert_eq!(f.flags.bits(), 0);
        assert_eq!(f.period, 1.0);
        assert_eq!(f.function_type, PeriodicFunction::One);
        assert_eq!((f.step_count, f.sawtooth_count), (0, 0));
        assert_eq!(f.bounds_mode.code(), 0);
        assert_eq!((f.lower_bound, f.upper_bound), (0.0, 1.0));
        assert_eq!((f.turn_off_with, f.scale_by), (-1, 0.0));
        assert!(f.runtime_is_consistent());
    }
    // The spec says both map linearly; on the disc function B (the shield
    // glow) maps `very early` (2).
    assert_eq!(o.functions[0].map_to.code(), 0);
    assert_eq!(o.functions[1].map_to.code(), 2);
    assert_eq!(o.functions[0].scale_function_by, 3);
    assert_eq!(o.functions[0].usage, "integrated light power");
    assert_eq!(o.functions[1].scale_function_by, 1);
    assert_eq!(o.functions[1].usage, "shield glow source");

    let c = &o.change_colors;
    assert_eq!((c[0].darken_by, c[0].scale_by, c[0].scale_flags), (6, 2, 1));
    for (got, want) in [
        (c[0].color_lower_bound.r, 0.74),
        (c[0].color_lower_bound.g, 0.98),
        (c[0].color_lower_bound.b, 1.0),
        (c[0].color_upper_bound.r, 0.33),
        (c[0].color_upper_bound.g, 0.51),
        (c[0].color_upper_bound.b, 1.0),
    ] {
        near(got, want, "change colour A");
    }
    assert!(c[0].permutations.is_empty());
    assert_eq!((c[1].darken_by, c[1].scale_by, c[1].scale_flags), (0, 7, 0));
    assert_eq!((c[1].color_lower_bound.r, c[1].color_lower_bound.b), (1.0, 1.0));
    assert_eq!((c[1].color_upper_bound.r, c[1].color_upper_bound.b), (0.0, 0.0));
    assert!(c[1].permutations.is_empty());

    // The unit layer.
    let u = &b.unit;
    assert_eq!(u.flags.bits(), 0x8008);
    assert!(u.flags.contains(UnitFlags::FIRES_FROM_CAMERA) && u.flags.contains(UnitFlags::MELEE_ATTACKERS_CANNOT_ATTACH));
    assert_eq!(u.default_team, UnitTeam::Player);
    assert_eq!(u.constant_sound_volume.code(), 0);
    assert_eq!(u.rider_damage_fraction, 0.0);
    assert_eq!(path_of(&map, &u.integrated_light_toggle), None);
    assert_eq!(u.inputs(), [UnitInput::None, UnitInput::None, UnitInput::IntegratedLightPower, UnitInput::None]);
    assert_eq!(u.camera_field_of_view, f32::from_bits(0x3F9C_61AA));
    near(u.camera_field_of_view, 1.2217305, "fov");
    assert_eq!(u.camera_stiffness, 0.0);
    assert!(u.camera_marker_name.is_empty() && u.camera_submerged_marker_name.is_empty());
    assert_eq!((u.pitch_auto_level, u.pitch_range_minimum, u.pitch_range_maximum), (0.0, 0.0, 0.0));
    assert_eq!(u.camera_tracks.len(), 1);
    assert_eq!(path_of(&map, &u.camera_tracks[0].track).as_deref(), Some("camera\\biped_small_tight"));
    assert_eq!(u.seat_acceleration_scale, [0.0; 3]);
    assert_eq!(
        (u.integrated_light_power_on_time, u.integrated_light_power_off_time, u.integrated_light_lifespan),
        (0.0, 0.0, 0.0)
    );
    assert_eq!(u.soft_ping_threshold, 0.0);
    assert_eq!(u.soft_ping_minimum_interrupt_time, f32::from_bits(0x3E99_999A));
    assert_eq!((u.hard_ping_threshold, u.hard_ping_minimum_interrupt_time), (1.0, 0.0));
    for v in [
        u.hard_death_threshold,
        u.feign_death_threshold,
        u.feign_death_time,
        u.distance_of_evade_animation,
        u.distance_of_dive_animation,
        u.stunned_movement_threshold,
        u.feign_death_chance,
        u.feign_repeat_chance,
        u.aiming_velocity_maximum,
        u.aiming_acceleration_maximum,
        u.casual_aiming_modifier,
        u.looking_velocity_maximum,
        u.looking_acceleration_maximum,
        u.ai_vehicle_avoidance_radius,
        u.ai_danger_radius,
    ] {
        assert_eq!(v, 0.0);
    }
    assert_eq!(path_of(&map, &u.spawned_actor), None);
    assert_eq!(path_of(&map, &u.melee_damage).as_deref(), Some("characters\\cyborg\\melee"));
    assert_eq!(u.motion_sensor_blip_size.code(), 0);
    let huds: Vec<_> = u.new_hud_interfaces.iter().map(|h| path_of(&map, &h.hud).unwrap()).collect();
    assert_eq!(huds, ["ui\\hud\\cyborg", "ui\\hud\\cyborg_mp"]);
    assert_eq!(u.dialogue_variants.len(), 1);
    assert_eq!(u.dialogue_variants[0].variant_number, 0);
    assert_eq!(path_of(&map, &u.dialogue_variants[0].dialogue).as_deref(), Some("sound\\dialog\\chief\\chief"));
    assert_eq!((u.grenade_velocity, u.grenade_type, u.grenade_count), (10.0, GrenadeType::Frag, 4));
    assert_eq!((u.soft_ping_interrupt_ticks, u.hard_ping_interrupt_ticks), (9, 0));
    assert!(u.powered_seats.is_empty() && u.seats.is_empty());
    let weapons: Vec<_> = u.weapons.iter().map(|w| path_of(&map, &w.weapon).unwrap()).collect();
    assert_eq!(weapons, ["weapons\\assault rifle\\assault rifle", "weapons\\pistol\\pistol"]);

    // The biped layer.
    let p = &b.own;
    assert_eq!(p.moving_turning_speed, f32::from_bits(0x40BC_9B76));
    near(p.moving_turning_speed, 5.893977, "turning speed");
    assert_eq!(p.flags.bits(), 0x202);
    near(p.stationary_turning_threshold, 1.0471976, "threshold");
    assert_eq!(p.input_a.code() + p.input_b.code() + p.input_c.code() + p.input_d.code(), 0);
    assert_eq!(path_of(&map, &p.dont_use), None);
    for v in [
        p.bank_angle,
        p.bank_apply_time,
        p.bank_decay_time,
        p.pitch_ratio,
        p.max_velocity,
        p.max_sidestep_velocity,
        p.acceleration,
        p.deceleration,
        p.angular_velocity_maximum,
        p.angular_acceleration_maximum,
        p.crouch_velocity_modifier,
    ] {
        assert_eq!(v, 0.0, "flying fields");
    }
    near(p.maximum_slope_angle, 0.7853982, "slope");
    near(p.downhill_falloff_angle, 0.3490658, "downhill falloff");
    near(p.downhill_cutoff_angle, 0.7853982, "downhill cutoff");
    near(p.downhill_velocity_scale, 1.25, "downhill scale");
    near(p.uphill_falloff_angle, 0.3490658, "uphill falloff");
    near(p.uphill_cutoff_angle, 0.7853982, "uphill cutoff");
    near(p.uphill_velocity_scale, 0.65, "uphill scale");
    assert_eq!(path_of(&map, &p.footsteps).as_deref(), Some("globals\\cyborg"));
    assert_eq!(p.jump_velocity, f32::from_bits(0x3D8F_5C29));
    near(p.jump_velocity, 0.07, "jump");
    near(p.jump_velocity_per_second(), 2.1, "jump per second");
    near(p.maximum_soft_landing_time, 0.6, "soft landing time");
    near(p.maximum_hard_landing_time, 1.0, "hard landing time");
    near(p.minimum_soft_landing_velocity, 1.5, "soft landing velocity");
    near(p.minimum_hard_landing_velocity, 5.0, "hard landing velocity");
    near(p.maximum_hard_landing_velocity, 8.0, "max hard landing velocity");
    assert_eq!(p.death_hard_landing_velocity, 0.0);
    near(p.standing_camera_height, 0.62, "standing camera");
    near(p.crouching_camera_height, 0.35, "crouching camera");
    near(p.crouch_transition_time, 0.2, "crouch time");
    near(p.standing_collision_height, 0.7, "standing height");
    near(p.crouching_collision_height, 0.5, "crouching height");
    near(p.collision_radius, 0.2, "radius");
    near(p.autoaim_width, 0.08, "autoaim width");
    assert_eq!(p.runtime_cosine_stationary_turning_threshold, f32::from_bits(0x3EFF_FFFF));
    near(p.runtime_crouch_transition_velocity, 0.16666667, "crouch velocity");
    near(p.runtime_minimum_normal_k, 0.70710677, "min normal k");
    near(p.runtime_downhill_k0, -0.34202012, "downhill k0");
    near(p.runtime_downhill_k1, -0.70710677, "downhill k1");
    near(p.runtime_uphill_k0, 0.34202012, "uphill k0");
    near(p.runtime_uphill_k1, 0.70710677, "uphill k1");
    assert_eq!((p.runtime_pelvis_node, p.runtime_head_node), (0, 12));
    let contacts: Vec<_> = p.contact_points.iter().map(|c| c.marker_name.as_str()).collect();
    assert_eq!(contacts, ["left foot", "right foot"]);

    // The struct ends where the first block begins.
    assert_eq!(o.attachments.len(), 1);
    let v = map.tag_view(tag, 0x4F4).unwrap();
    assert_eq!(v.block(0x140, 0x48).unwrap().addr() - tag.address, 0x4F4);
}

#[test]
fn the_campaign_spartan_differs_as_the_spec_says() {
    let Some(map) = load("bloodgulch") else { return };
    let mp = biped(&map, SPARTAN);
    let cp = biped(&map, "characters\\cyborg\\cyborg");
    assert_eq!(map.find(class::BIPED, "characters\\cyborg\\cyborg").unwrap().index(), 1675);
    near(cp.object.bounding_radius, 0.45, "radius");
    near(cp.object.bounding_offset[0], -0.02, "offset");
    near(cp.object.render_bounding_radius, 0.45, "render radius");
    assert_eq!(cp.object.change_colors.len(), 3);
    let c = &cp.object.change_colors[2];
    assert_eq!(c.permutations.len(), 1);
    assert_eq!(c.permutations[0].weight, 1.0);
    for col in [c.permutations[0].color_lower_bound, c.permutations[0].color_upper_bound] {
        // The spec gives three digits (87, 103, 37 of 255).
        assert!((col.r - 0.341).abs() < 1e-3 && (col.g - 0.404).abs() < 1e-3 && (col.b - 0.145).abs() < 1e-3);
    }
    // The spec says 0x447008; the disc has 0x448008 (bits 3, 15 and 22).
    assert_eq!(cp.unit.flags.bits(), 0x448008);
    assert_eq!(
        path_of(&map, &cp.unit.integrated_light_toggle).as_deref(),
        Some("weapons\\assault rifle\\effects\\flashlight")
    );
    assert_eq!(path_of(&map, &cp.unit.melee_damage), None);
    near(cp.own.stationary_turning_threshold, 1.3962634, "threshold");
    near(cp.own.runtime_cosine_stationary_turning_threshold, 0.17364822, "cosine");
    assert_eq!(cp.unit.hard_ping_threshold, 0.0);
    // The rest equals the multiplayer biped's.
    near(cp.own.jump_velocity, mp.own.jump_velocity, "jump");
    assert_eq!(cp.own.flags, mp.own.flags);
}

/// Name, powerup, time, grenade type, pickup sound.
#[allow(clippy::type_complexity)]
const EQUIPMENT: [(&str, i16, f32, i16, Option<&str>); 14] = [
    (
        "powerups\\assault rifle ammo\\assault rifle ammo",
        0,
        0.0,
        0,
        Some("sound\\sfx\\weapons\\weapon_pickup_ammo\\ar_ammo"),
    ),
    (
        "powerups\\pistol ammo\\pistol ammo",
        0,
        0.0,
        0,
        Some("sound\\sfx\\weapons\\weapon_pickup_ammo\\pistol_ammo"),
    ),
    (
        "powerups\\shotgun ammo\\shotgun ammo",
        0,
        0.0,
        0,
        Some("sound\\sfx\\weapons\\weapon_pickup_ammo\\shotgun_ammo"),
    ),
    ("powerups\\active camouflage", 3, 45.0, 0, Some("sound\\sfx\\ui\\pickup_invis")),
    ("powerups\\over shield", 2, 60.0, 0, Some("sound\\sfx\\ui\\pickup_dbl_shield")),
    ("powerups\\health pack", 5, 1.0, 0, Some("sound\\sfx\\ui\\pickup_health")),
    (
        "weapons\\frag grenade\\frag grenade",
        6,
        0.0,
        0,
        Some("sound\\sfx\\weapons\\weapon_pickup_ammo\\grenade_ammo"),
    ),
    (
        "powerups\\rocket launcher ammo\\rocket launcher ammo",
        0,
        0.0,
        0,
        Some("sound\\sfx\\weapons\\weapon_pickup_ammo\\rocket_ammo"),
    ),
    (
        "powerups\\sniper rifle ammo\\sniper rifle ammo",
        0,
        0.0,
        0,
        Some("sound\\sfx\\weapons\\weapon_pickup_ammo\\sniper_ammo"),
    ),
    (
        "weapons\\plasma grenade\\plasma grenade",
        6,
        0.0,
        1,
        Some("sound\\sfx\\weapons\\weapon_pickup_ammo\\grenade_plas_ammo"),
    ),
    ("powerups\\flamethrower ammo\\flamethrower ammo", 0, 0.0, 0, None),
    (
        "powerups\\needler ammo\\needler ammo",
        0,
        0.0,
        0,
        Some("sound\\sfx\\weapons\\weapon_pickup_ammo\\needle_ammo"),
    ),
    ("powerups\\double speed", 1, 60.0, 0, Some("sound\\sfx\\ui\\pickup_dbl_time")),
    ("powerups\\full-spectrum vision", 4, 60.0, 0, None),
];

#[test]
fn the_fourteen_equipment_tags() {
    let Some(map) = load("bloodgulch") else { return };
    assert_eq!(map.of_class(class::EQUIPMENT).count(), 14);
    // Tag order of Blood Gulch.
    let order: Vec<_> = map.of_class(class::EQUIPMENT).map(|t| t.path.as_str()).collect();
    assert_eq!(order, EQUIPMENT.iter().map(|e| e.0).collect::<Vec<_>>());
    for (path, powerup, time, grenade, sound) in EQUIPMENT {
        let e = Equipment::read(&map, map.find(class::EQUIPMENT, path).unwrap()).unwrap();
        assert_eq!(e.own.powerup_type.code(), powerup, "{path}");
        assert_eq!(e.own.powerup_type, Powerup::from_code(powerup).unwrap());
        assert_eq!(e.own.powerup_time, time, "{path}");
        assert_eq!(e.own.grenade_type.code(), grenade, "{path}");
        assert_eq!(path_of(&map, &e.own.pickup_sound).as_deref(), sound, "{path}");
    }

    // Other values: object flags, radius, acceleration scale, message index,
    // scale, item flags, detonation.
    let eq = |p: &str| Equipment::read(&map, map.find(class::EQUIPMENT, p).unwrap()).unwrap();
    for (p, flags) in [
        ("powerups\\active camouflage", 4),
        ("powerups\\over shield", 4),
        ("powerups\\health pack", 4),
        ("weapons\\frag grenade\\frag grenade", 4),
        ("weapons\\plasma grenade\\plasma grenade", 4),
        ("powerups\\double speed", 0),
        ("powerups\\pistol ammo\\pistol ammo", 0),
    ] {
        assert_eq!(eq(p).object.flags.bits(), flags, "{p}");
    }
    for (p, r) in [
        ("powerups\\assault rifle ammo\\assault rifle ammo", 0.2),
        ("powerups\\pistol ammo\\pistol ammo", 0.2),
        ("powerups\\rocket launcher ammo\\rocket launcher ammo", 0.2875),
        ("powerups\\sniper rifle ammo\\sniper rifle ammo", 0.1),
        ("powerups\\active camouflage", 0.1),
        ("powerups\\over shield", 0.1),
        ("weapons\\frag grenade\\frag grenade", 0.1),
        ("powerups\\health pack", 0.17),
    ] {
        near(eq(p).object.bounding_radius, r, p);
    }
    for (p, a) in [
        ("powerups\\active camouflage", 0.5),
        ("powerups\\over shield", 0.5),
        ("weapons\\frag grenade\\frag grenade", 0.5),
        ("powerups\\health pack", 0.75),
        ("weapons\\plasma grenade\\plasma grenade", 0.15),
        ("powerups\\assault rifle ammo\\assault rifle ammo", 1.0),
        ("powerups\\pistol ammo\\pistol ammo", 1.0),
        ("powerups\\shotgun ammo\\shotgun ammo", 1.0),
        ("powerups\\sniper rifle ammo\\sniper rifle ammo", 1.0),
        ("powerups\\rocket launcher ammo\\rocket launcher ammo", 0.9),
        ("powerups\\needler ammo\\needler ammo", 1.5),
        ("powerups\\flamethrower ammo\\flamethrower ammo", 0.5),
        ("powerups\\double speed", 0.0),
        ("powerups\\full-spectrum vision", 0.0),
    ] {
        near(eq(p).object.acceleration_scale, a, p);
    }
    for (p, m, s) in [
        ("powerups\\active camouflage", 30, 0.0),
        ("powerups\\over shield", 38, 0.0),
        ("powerups\\health pack", 36, 0.0),
        ("weapons\\frag grenade\\frag grenade", 2, 1.75),
        ("weapons\\plasma grenade\\plasma grenade", 40, 1.25),
        ("powerups\\pistol ammo\\pistol ammo", 0, 0.0),
    ] {
        let e = eq(p);
        assert_eq!(e.item.message_index, m, "{p}");
        near(e.item.scale, s, p);
    }
    for (p, f, det) in [
        ("weapons\\frag grenade\\frag grenade", 2, Some("weapons\\frag grenade\\effects\\explosion")),
        ("weapons\\plasma grenade\\plasma grenade", 2, Some("weapons\\plasma grenade\\effects\\explosion")),
        (
            "powerups\\rocket launcher ammo\\rocket launcher ammo",
            2,
            Some("weapons\\rocket launcher\\effects\\rocket explosion"),
        ),
        ("powerups\\pistol ammo\\pistol ammo", 0, None),
    ] {
        let e = eq(p);
        assert_eq!(e.item.flags.bits(), f, "{p}");
        assert_eq!(path_of(&map, &e.item.detonation_effect).as_deref(), det, "{p}");
        if f == 2 {
            near(e.item.detonation_delay_minimum, 0.4, p);
            near(e.item.detonation_delay_maximum, 0.8, p);
        }
    }
    assert_eq!(
        path_of(&map, &eq("weapons\\plasma grenade\\plasma grenade").item.detonating_effect).as_deref(),
        Some("weapons\\plasma grenade\\effects\\detonation start")
    );
    assert_eq!(path_of(&map, &eq("weapons\\frag grenade\\frag grenade").item.detonating_effect), None);
    // The two powerup models are swapped on the disc.
    assert_eq!(
        path_of(&map, &eq("powerups\\active camouflage").object.model).as_deref(),
        Some("powerups\\over shield\\over shield")
    );
    assert_eq!(
        path_of(&map, &eq("powerups\\over shield").object.model).as_deref(),
        Some("powerups\\active camoflage\\active camoflage")
    );
    for p in ["powerups\\double speed", "powerups\\full-spectrum vision"] {
        let e = eq(p);
        assert_eq!(path_of(&map, &e.object.model).as_deref(), Some("item collections\\powerups\\powerups"));
        assert_eq!(path_of(&map, &e.object.collision_model), None);
    }
    for (p, m) in [
        ("powerups\\pistol ammo\\pistol ammo", "powerups\\pistol ammo\\pistol ammo"),
        // The spec says the model has the tag's own path; the health pack's
        // is spelt without a space on the disc.
        ("powerups\\health pack", "powerups\\healthpack\\healthpack"),
    ] {
        let e = eq(p);
        assert_eq!(path_of(&map, &e.object.model).as_deref(), Some(m));
        assert_eq!(path_of(&map, &e.object.collision_model).as_deref(), Some(m));
    }
    for p in ["weapons\\frag grenade\\frag grenade", "weapons\\plasma grenade\\plasma grenade"] {
        assert_eq!(path_of(&map, &eq(p).object.collision_model), None);
    }
}

/// Tag, class, bounding radius, model, collision, animation, physics:
/// "same" is the tag's own path.
const SAME: &str = "=";

#[allow(clippy::type_complexity)]
const WEAPONS_AND_VEHICLES: [(&str, bool, f32, Option<&str>, Option<&str>, Option<&str>, Option<&str>); 18] = [
    ("weapons\\assault rifle\\assault rifle", false, 0.6, Some(SAME), Some(SAME), None, None),
    ("weapons\\pistol\\pistol", false, 0.1, Some(SAME), Some(SAME), Some(SAME), None),
    ("weapons\\shotgun\\shotgun", false, 0.2, Some(SAME), Some(SAME), Some(SAME), None),
    ("weapons\\sniper rifle\\sniper rifle", false, 0.4125, Some(SAME), Some(SAME), Some(SAME), None),
    ("weapons\\rocket launcher\\rocket launcher", false, 0.2875, Some(SAME), Some(SAME), None, None),
    ("weapons\\plasma pistol\\plasma pistol", false, 0.2, Some(SAME), Some(SAME), None, None),
    ("weapons\\plasma rifle\\plasma rifle", false, 0.2, Some(SAME), Some(SAME), None, None),
    ("weapons\\needler\\needler", false, 0.2, Some(SAME), Some(SAME), None, None),
    ("weapons\\flamethrower\\flamethrower", false, 0.2, Some(SAME), Some(SAME), None, None),
    ("weapons\\gravity rifle\\gravity rifle", false, 0.2, Some(SAME), None, None, None),
    ("weapons\\ball\\ball", false, 0.3, Some(SAME), Some(SAME), None, None),
    ("weapons\\flag\\flag", false, 0.45, Some(SAME), None, None, None),
    ("vehicles\\warthog\\warthog gun", false, 0.2, None, None, None, None),
    ("vehicles\\scorpion\\scorpion cannon", false, 0.2, None, None, None, None),
    ("vehicles\\ghost\\ghost gun", false, 0.0, None, None, None, None),
    ("vehicles\\warthog\\warthog", true, 1.1, Some(SAME), Some(SAME), Some(SAME), Some(SAME)),
    (
        "vehicles\\scorpion\\scorpion_mp",
        true,
        2.2,
        Some("vehicles\\scorpion\\scorpion_mp\\scorpion_mp"),
        Some("vehicles\\scorpion\\scorpion_mp\\scorpion_mp"),
        Some("vehicles\\scorpion\\scorpion"),
        Some("vehicles\\scorpion\\scorpion"),
    ),
    (
        "vehicles\\ghost\\ghost_mp",
        true,
        0.8,
        Some("vehicles\\ghost\\ghost"),
        Some(SAME),
        Some("vehicles\\ghost\\ghost"),
        Some("vehicles\\ghost\\ghost"),
    ),
];

#[test]
fn weapon_and_vehicle_object_bases() {
    let Some(map) = load("bloodgulch") else { return };
    for (path, vehicle, radius, model, coll, anim, phys) in WEAPONS_AND_VEHICLES {
        let (c, t) = if vehicle { (class::VEHICLE, ObjectType::Vehicle) } else { (class::WEAPON, ObjectType::Weapon) };
        let tag = map.find(c, path).unwrap_or_else(|| panic!("no {path}"));
        let o = ObjectBase::read(&map, tag).unwrap();
        assert_eq!(o.object_type, t, "{path}");
        near(o.bounding_radius, radius, path);
        near(o.render_bounding_radius, radius, path);
        let want = |w: Option<&str>| w.map(|w| if w == SAME { path.to_string() } else { w.to_string() });
        assert_eq!(path_of(&map, &o.model), want(model), "{path} model");
        assert_eq!(path_of(&map, &o.collision_model), want(coll), "{path} collision");
        assert_eq!(path_of(&map, &o.animation_graph), want(anim), "{path} animation");
        assert_eq!(path_of(&map, &o.physics), want(phys), "{path} physics");
        // The class of each reference field is its expected one.
        assert_eq!(o.model.class, class::MODEL);
        assert_eq!(o.collision_model.class, class::MODEL_COLLISION_GEOMETRY);
        assert_eq!(o.animation_graph.class, class::MODEL_ANIMATIONS);
        assert_eq!(o.physics.class, class::PHYSICS);
        if !vehicle {
            assert_eq!(o.inputs()[..3], [FunctionInput::None; 3].map(|x| x), "{path}");
            let d = if path == "weapons\\assault rifle\\assault rifle" { FunctionInput::Compass } else { FunctionInput::None };
            assert_eq!(o.input_d, d, "{path}");
        } else {
            assert_eq!(o.inputs(), [FunctionInput::None; 4], "{path}");
        }
    }

    // The weapons' item layers: message index and scale, sort order, flags.
    for (path, msg, scale, object_flags) in [
        ("weapons\\assault rifle\\assault rifle", 4, 1.8, 4),
        ("weapons\\pistol\\pistol", 20, 2.25, 4),
        ("weapons\\shotgun\\shotgun", 26, 1.7, 0),
        ("weapons\\sniper rifle\\sniper rifle", 28, 1.8, 4),
        ("weapons\\rocket launcher\\rocket launcher", 24, 1.7, 4),
        ("weapons\\plasma pistol\\plasma pistol", 6, 2.0, 4),
        ("weapons\\plasma rifle\\plasma rifle", 22, 1.8, 4),
        ("weapons\\needler\\needler", 18, 1.3, 4),
        ("weapons\\flamethrower\\flamethrower", 12, 2.0, 0),
        ("weapons\\gravity rifle\\gravity rifle", 16, 0.0, 0),
        ("weapons\\ball\\ball", 1, 2.2, 4),
        ("weapons\\flag\\flag", 10, 0.0, 0),
        ("vehicles\\warthog\\warthog gun", 0, 0.0, 0),
        ("vehicles\\scorpion\\scorpion cannon", 0, 0.0, 0),
        ("vehicles\\ghost\\ghost gun", 0, 0.0, 0),
    ] {
        let w = match ObjectTag::read(&map, map.find(class::WEAPON, path).unwrap()).unwrap() {
            ObjectTag::Weapon(w) => w,
            other => panic!("{other:?}"),
        };
        assert_eq!(w.item.message_index, msg, "{path}");
        near(w.item.scale, scale, path);
        assert_eq!(w.object.flags.bits(), object_flags, "{path} object flags");
        assert_eq!([w.item.input_a, w.item.input_b, w.item.input_c, w.item.input_d], [0; 4], "{path}");
        let sort = if path.ends_with("plasma pistol") { 2 } else { 0 };
        assert_eq!(w.item.sort_order, sort, "{path}");
        let item_flags = if path.ends_with("flag\\flag") { 1 } else { 0 };
        assert_eq!(w.item.flags.bits(), item_flags, "{path} item flags");
        assert_eq!(w.item.hud_message_value_scale, if path.ends_with("flamethrower") { 100 } else { 0 }, "{path}");
    }
}

#[test]
fn the_three_vehicles_unit_layer() {
    let Some(map) = load("bloodgulch") else { return };
    // Flags, aiming velocity and acceleration, blip, input A, C, scale,
    // seats, hud text index.
    #[allow(clippy::type_complexity)]
    let rows: [(&str, u32, f32, f32, i16, UnitInput, UnitInput, [f32; 3], usize, i16); 3] = [
        (
            "vehicles\\warthog\\warthog",
            0x41,
            3.1415927,
            10.471975,
            0,
            UnitInput::DriverSeatPower,
            UnitInput::None,
            [200.0, 200.0, 150.0],
            3,
            16,
        ),
        (
            "vehicles\\scorpion\\scorpion_mp",
            0x1,
            1.5707964,
            13.962634,
            2,
            UnitInput::None,
            UnitInput::DriverSeatPower,
            [200.0, 200.0, 150.0],
            5,
            23,
        ),
        (
            "vehicles\\ghost\\ghost_mp",
            0,
            0.0,
            0.0,
            2,
            UnitInput::DriverSeatPower,
            UnitInput::None,
            [100.0, 100.0, 100.0],
            1,
            21,
        ),
    ];
    for (path, flags, av, aa, blip, ia, ic, scale, seats, hud) in rows {
        let v = Vehicle::read(&map, map.find(class::VEHICLE, path).unwrap()).unwrap();
        let u = &v.unit;
        assert_eq!(u.flags.bits(), flags, "{path}");
        assert_eq!(u.default_team, UnitTeam::Default, "{path}");
        assert_eq!(u.constant_sound_volume.code(), 1, "{path}");
        near(u.rider_damage_fraction, 0.18, path);
        near(u.camera_field_of_view, 1.2217305, path);
        assert_eq!(u.seat_acceleration_scale, scale, "{path}");
        if av != 0.0 {
            near(u.aiming_velocity_maximum, av, path);
            near(u.aiming_acceleration_maximum, aa, path);
        }
        assert_eq!(u.motion_sensor_blip_size.code(), blip, "{path}");
        assert_eq!(u.input_a, ia, "{path}");
        assert_eq!(u.input_c, ic, "{path}");
        assert_eq!((u.powered_seats.len(), u.weapons.len(), u.seats.len()), (1, 1, seats), "{path}");
        assert_eq!(v.object.hud_text_message_index, hud, "{path}");
    }
    let w = Vehicle::read(&map, map.find(class::VEHICLE, "vehicles\\warthog\\warthog").unwrap()).unwrap();
    assert_eq!((w.unit.powered_seats[0].power_up_time, w.unit.powered_seats[0].power_down_time), (1.0, 0.1));
    let g = Vehicle::read(&map, map.find(class::VEHICLE, "vehicles\\ghost\\ghost_mp").unwrap()).unwrap();
    assert_eq!((g.unit.powered_seats[0].power_up_time, g.unit.powered_seats[0].power_down_time), (2.0, 1.0));
}

fn expected_class(r: &TagRef, want: &[ClassCode], map: &MapFile, what: &str) {
    if r.is_null() {
        return;
    }
    let t = map.tag(r.id).unwrap_or_else(|| panic!("{what}: unresolved {r:?}"));
    assert!(want.iter().any(|c| t.is_a(*c)), "{what}: {} is a {}", t.path, t.class);
}

/// Check one reference of a tag against the classes it may name.
fn r(map: &MapFile, tag: &str, field: &str, rf: &TagRef, want: &[ClassCode]) {
    expected_class(rf, want, map, &format!("{tag} {field}"));
}

#[test]
fn every_object_tag_of_bloodgulch_reads() {
    let Some(map) = load("bloodgulch") else { return };
    let mut n = 0;
    let mut per_class = std::collections::BTreeMap::new();
    let snd_or_eff = [class::SOUND, class::EFFECT];
    for tag in map.of_kind(class::OBJECT) {
        let t = ObjectTag::read(&map, tag).unwrap_or_else(|e| panic!("{}: {e}", tag.path));
        n += 1;
        *per_class.entry(tag.class.to_string()).or_insert(0) += 1;
        let o = t.object();
        let p = tag.path.as_str();
        padding_is_zero(&map, tag, &t);
        assert_eq!(o.object_type.class(), tag.class, "{p}");
        assert_eq!(t.class(), tag.class);
        r(&map, p, "model", &o.model, &[class::MODEL]);
        r(&map, p, "animation", &o.animation_graph, &[class::MODEL_ANIMATIONS]);
        r(&map, p, "collision", &o.collision_model, &[class::MODEL_COLLISION_GEOMETRY]);
        r(&map, p, "physics", &o.physics, &[class::PHYSICS]);
        r(&map, p, "modifier", &o.modifier_shader, &[class::SHADER]);
        r(&map, p, "creation effect", &o.creation_effect, &[class::EFFECT]);
        // The reference field classes name their expected class.
        assert_eq!(o.model.class, class::MODEL, "{p}");
        assert_eq!(o.flags.unnamed_bits(), 0, "{p}: flags only in bits 0 to 3");
        assert!(o.bounding_radius.is_finite() && (0.0..100.0).contains(&o.bounding_radius), "{p}");
        near(o.render_bounding_radius, o.bounding_radius, p);
        for a in &o.attachments {
            r(&map, p, "attachment", &a.attached, &[class::LIGHT, class::SOUND_LOOPING, class::EFFECT, class::CONTRAIL, class::PARTICLE_SYSTEM, class::LIGHT_VOLUME]);
            assert!((0..=4).contains(&a.primary_scale) && (0..=4).contains(&a.secondary_scale), "{p}");
            assert!((0..=4).contains(&a.change_color), "{p}");
        }
        for w in &o.widgets {
            r(&map, p, "widget", &w.widget, &[class::ANTENNA, class::LIGHT_VOLUME, class::FLAG]);
        }
        for f in &o.functions {
            assert!(f.runtime_is_consistent(), "{p}");
            for i in [f.scale_period_by, f.scale_function_by, f.add, f.scale_result_by] {
                assert!((0..=8).contains(&i), "{p}: function input {i}");
            }
            assert!((-1..=3).contains(&f.turn_off_with), "{p}");
            assert!(f.flags.unnamed_bits() == 0, "{p}");
            assert!(f.upper_bound >= f.lower_bound, "{p}");
            assert!(f.period.is_finite() && f.period >= 0.0, "{p}");
        }
        for c in &o.change_colors {
            assert!((0..=8).contains(&c.darken_by) && (0..=8).contains(&c.scale_by), "{p}");
            assert_eq!(c.scale_flags & !3, 0, "{p}");
            let mut last = 0.0;
            for pm in &c.permutations {
                assert!(pm.weight >= last && pm.weight <= 1.0, "{p}");
                last = pm.weight;
            }
            if !c.permutations.is_empty() {
                assert_eq!(c.permutations.last().unwrap().weight, 1.0, "{p}");
            }
        }
        for pr in &o.predicted_resources {
            assert_eq!(map.tag(pr.tag).unwrap().class, class::BITMAP, "{p}");
        }
        match &t {
            ObjectTag::Biped(b) => unit_checks(&map, p, &b.unit, o),
            ObjectTag::Vehicle(v) => unit_checks(&map, p, &v.unit, o),
            ObjectTag::Equipment(e) => {
                r(&map, p, "pickup", &e.own.pickup_sound, &[class::SOUND]);
                assert!(e.item.flags.unnamed_bits() == 0, "{p}");
            }
            ObjectTag::Weapon(w) => {
                assert!(w.item.flags.unnamed_bits() == 0, "{p}");
                r(&map, p, "ready", &w.own.ready_effect, &snd_or_eff);
            }
            ObjectTag::Garbage(_)
            | ObjectTag::Scenery(_)
            | ObjectTag::SoundScenery(_)
            | ObjectTag::Projectile(_) => {}
            other => panic!("{p}: unexpected {other:?}"),
        }
    }
    assert_eq!(n, 70);
    assert_eq!(per_class["scen"], 19);
    assert_eq!(per_class["bipd"], 2);
    assert_eq!(per_class["weap"], 15);
    assert_eq!(per_class["eqip"], 14);
    assert_eq!(per_class["proj"], 16);
    assert_eq!(per_class["vehi"], 3);
    assert_eq!(per_class["ssce"], 1);
}

/// The byte ranges the spec calls padding are zero on every tag.
fn padding_is_zero(map: &MapFile, tag: &blam_formats::cache::TagEntry, t: &ObjectTag) {
    let size = match t {
        ObjectTag::Biped(_) => 0x4F4,
        ObjectTag::Vehicle(_) => 0x3F0,
        ObjectTag::Weapon(_) => 0x508,
        ObjectTag::Equipment(_) | ObjectTag::Garbage(_) => 0x3B0,
        ObjectTag::Projectile(_) => 0x24C,
        ObjectTag::Scenery(_) | ObjectTag::SoundScenery(_) => 0x1FC,
        _ => return,
    };
    let v = map.tag_view(tag, size).unwrap();
    let mut ranges = vec![0x48..0x70, 0xB0..0x104, 0x110..0x13C];
    match t {
        ObjectTag::Biped(_) | ObjectTag::Vehicle(_) => ranges.extend([0x278..0x280, 0x29A..0x2A8]),
        ObjectTag::Weapon(_) | ObjectTag::Equipment(_) | ObjectTag::Garbage(_) => {
            ranges.extend([0x18A..0x19C, 0x1A4..0x248, 0x268..0x2E0]);
            match t {
                ObjectTag::Equipment(_) => ranges.push(0x320..0x3B0),
                ObjectTag::Garbage(_) => ranges.push(0x308..0x3B0),
                _ => {}
            }
        }
        ObjectTag::Scenery(_) | ObjectTag::SoundScenery(_) => ranges.push(0x17C..0x1FC),
        _ => {}
    }
    for r in ranges {
        let bytes = v.slice(r.start, r.len()).unwrap();
        assert!(bytes.iter().all(|&b| b == 0), "{}: padding {:#x}..{:#x} is not zero", tag.path, r.start, r.end);
    }
}

fn unit_checks(map: &MapFile, p: &str, u: &blam_formats::object::UnitBase, o: &ObjectBase) {
    r(map, p, "toggle", &u.integrated_light_toggle, &[class::EFFECT]);
    r(map, p, "melee", &u.melee_damage, &[class::DAMAGE_EFFECT]);
    r(map, p, "actor", &u.spawned_actor, &[class::ACTOR_VARIANT]);
    for t in &u.camera_tracks {
        r(map, p, "track", &t.track, &[class::CAMERA_TRACK]);
        assert!(!t.track.is_null(), "{p}");
    }
    for h in &u.new_hud_interfaces {
        r(map, p, "hud", &h.hud, &[class::UNIT_HUD_INTERFACE]);
    }
    for d in &u.dialogue_variants {
        r(map, p, "dialogue", &d.dialogue, &[class::DIALOGUE]);
    }
    for w in &u.weapons {
        r(map, p, "weapon", &w.weapon, &[class::WEAPON]);
    }
    for s in &u.seats {
        r(map, p, "seat actor", &s.built_in_actor, &[class::ACTOR_VARIANT]);
    }
    // No slot is named in two layers (the unit's A to D against the
    // object's).
    for (a, b) in o.inputs().iter().zip(u.inputs()) {
        assert!(*a == FunctionInput::None || b == UnitInput::None, "{p}: slot named twice");
    }
}

#[test]
fn the_spartan_and_the_weapons_match_on_other_maps() {
    let Some(base) = load("bloodgulch") else { return };
    let spartan = normalized(&base, &biped(&base, SPARTAN));
    let equipment: Vec<String> = base
        .of_class(class::EQUIPMENT)
        .map(|t| normalized(&base, &Equipment::read(&base, t).unwrap()))
        .collect();
    let bipeds: Vec<(String, String)> =
        base.of_class(class::BIPED).map(|t| (t.path.clone(), normalized(&base, &Biped::read(&base, t).unwrap()))).collect();
    let vehicles: Vec<(String, String)> = base
        .of_class(class::VEHICLE)
        .map(|t| (t.path.clone(), normalized(&base, &Vehicle::read(&base, t).unwrap())))
        .collect();
    for other in ["hangemhigh", "sidewinder", "chillout"] {
        let Some(map) = load(other) else { return };
        assert_eq!(normalized(&map, &biped(&map, SPARTAN)), spartan, "{other}: Spartan");
        for (path, want) in &bipeds {
            let t = map.find(class::BIPED, path).unwrap();
            assert_eq!(&normalized(&map, &Biped::read(&map, t).unwrap()), want, "{other}: {path}");
        }
        for (path, want) in &vehicles {
            let t = map.find(class::VEHICLE, path).unwrap();
            assert_eq!(&normalized(&map, &Vehicle::read(&map, t).unwrap()), want, "{other}: {path}");
        }
        let got: Vec<String> = base
            .of_class(class::EQUIPMENT)
            .map(|t| normalized(&map, &Equipment::read(&map, map.find(class::EQUIPMENT, &t.path).unwrap()).unwrap()))
            .collect();
        assert_eq!(got, equipment, "{other}: equipment");
    }
}

#[test]
fn every_object_tag_of_every_multiplayer_map_reads() {
    for name in super::maps::MULTIPLAYER {
        let Some(map) = load(name) else { return };
        let mut n = 0;
        for tag in map.of_kind(class::OBJECT) {
            let t = ObjectTag::read(&map, tag).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(t.class(), tag.class, "{name}: {}", tag.path);
            n += 1;
        }
        assert!(n >= 50, "{name}: {n} object tags");
    }
}
