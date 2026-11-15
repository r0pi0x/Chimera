//! The projectile tag reader on a synthetic tag with the numbers of
//! specs/halo_ce/projectiles.md (the needle's row of "Flight numbers", a
//! response block of three elements): the layout, the lookup by material
//! type, the built-in response, the rejections and the pure formulas on
//! the tag's fields (deceleration, direct-hit scale, timer ticks, the
//! potential response's conditions).

// The spec's numbers are written as it states them, rounded.
#![allow(clippy::approx_constant, clippy::excessive_precision)]

use blam_formats::cache::{MapFile, TagEntry, class};
use blam_formats::collision::MaterialType;
use blam_formats::object::{ObjectTag, ObjectType};
use blam_formats::projectile::{
    NoiseLevel, PotentialFlags, Projectile, ProjectileFlags, ProjectileInput, ResponseType, ScaleEffectsBy, TimerStarts,
};

use crate::map_fixture::{CacheFile, Struct, TagData, null_ref};

const SCNR: usize = 0;
const EFFE: usize = 1;
const JPT: usize = 2;
const SND: usize = 3;
const PROJ: usize = 4;

fn tags() -> Vec<(&'static str, &'static str)> {
    vec![
        ("scnr", "scenarios\\syn"),
        ("effe", "weapons\\test\\effects\\hit"),
        ("jpt!", "weapons\\test\\impact damage"),
        ("snd!", "sound\\test\\flyby"),
        ("proj", "weapons\\test\\needle"),
    ]
}

/// A response element: the default response with its effect, the potential
/// response with its window.
fn response(default: i16, potential: i16, td: &TagData) -> Vec<u8> {
    Struct::new(0xA0)
        .i16(0x02, default)
        .put(0x04, &td.tag_ref(EFFE))
        .i16(0x24, potential)
        .u16(0x26, 1)
        .f32(0x28, 0.2)
        .f32(0x2C, 0.0)
        .f32(0x30, 0.4363)
        .f32(0x34, 0.0)
        .f32(0x38, 0.5)
        .put(0x3C, &null_ref("effe"))
        .i16(0x5C, 1)
        .f32(0x60, 0.2618)
        .f32(0x64, 0.01)
        .put(0x68, &td.tag_ref(EFFE))
        .f32(0x90, 0.7)
        .f32(0x94, 100.0)
        .f32(0x98, 0.3)
        .f32(0x9C, 0.4)
        .into()
}

fn map_with(responses: usize, bad_response: Option<i16>) -> MapFile {
    let mut td = TagData::new(&tags());
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    let elems: Vec<Vec<u8>> = (0..responses)
        .map(|i| {
            let r = response(if i == 0 { 0 } else { 2 }, if i == 2 { 4 } else { 0 }, &td);
            match bad_response {
                Some(code) if i == 1 => Struct(r).i16(0x02, code).into(),
                _ => r,
            }
        })
        .collect();
    let block = td.block(&elems);
    let main = Struct::new(0x24C)
        .i16(0, 5)
        .f32(4, 0.5)
        .u32(0x17C, 0x0B)
        .i16(0x180, 2)
        .i16(0x182, 4)
        .i16(0x184, 1)
        .i16(0x186, 2)
        .i16(0x188, 3)
        .i16(0x18A, 0)
        .put(0x18C, &null_ref("effe"))
        .f32(0x19C, 8.0)
        .f32(0x1A0, 0.1)
        .f32(0x1A4, 1.5)
        .f32(0x1A8, 2.25)
        .put(0x1AC, &td.tag_ref(EFFE))
        .f32(0x1BC, 0.75)
        .f32(0x1C0, 0.75)
        .f32(0x1C4, 0.0666667)
        .f32(0x1C8, 20.0)
        .f32(0x1CC, 0.0)
        .f32s(0x1D0, &[0.0, 100.0])
        .f32(0x1D8, 0.4)
        .f32s(0x1DC, &[0.0, 10.0])
        .f32(0x1E4, 0.133333)
        .f32(0x1E8, 0.133333)
        .f32(0x1EC, 1.5708)
        .i16(0x1F0, 1)
        .put(0x1F4, &null_ref("effe"))
        .put(0x204, &td.tag_ref(SND))
        .put(0x214, &td.tag_ref(JPT))
        .put(0x224, &td.tag_ref(JPT))
        .put(0x240, &block);
    td.set_struct(PROJ, &main.0);
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

fn tag(map: &MapFile) -> &TagEntry {
    &map.tags()[PROJ]
}

#[test]
fn reads_the_projectile_part() {
    let map = map_with(3, None);
    let p = Projectile::read(&map, tag(&map)).unwrap();
    assert_eq!(p.object.object_type, ObjectType::Projectile);
    assert_eq!(p.object.bounding_radius, 0.5);
    let o = &p.own;
    assert_eq!(o.flags.bits(), 0x0B);
    assert!(o.flags.contains(ProjectileFlags::ORIENTED_ALONG_VELOCITY));
    assert!(o.flags.contains(ProjectileFlags::AI_MUST_USE_BALLISTIC_AIMING));
    assert!(o.flags.contains(ProjectileFlags::HAS_SUPER_COMBINING_EXPLOSION));
    assert!(!o.flags.contains(ProjectileFlags::DETONATION_MAX_TIME_IF_ATTACHED));
    assert_eq!(o.flags.unnamed_bits(), 0);
    assert_eq!(o.detonation_timer_starts, TimerStarts::WhenAtRest);
    assert_eq!(o.impact_noise, NoiseLevel::Quiet);
    assert_eq!(
        [o.input_a, o.input_b, o.input_c, o.input_d],
        [
            ProjectileInput::RangeRemaining,
            ProjectileInput::TimeRemaining,
            ProjectileInput::Tracer,
            ProjectileInput::None
        ]
    );
    assert!(o.super_detonation.is_null());
    assert_eq!((o.ai_perception_radius, o.collision_radius), (8.0, 0.1));
    assert_eq!((o.arming_time, o.danger_radius), (1.5, 2.25));
    assert_eq!(map.tag(o.effect.id).unwrap().class, class::EFFECT);
    assert_eq!((o.detonation_timer.lower, o.detonation_timer.upper), (0.75, 0.75));
    assert_eq!((o.minimum_velocity, o.maximum_range), (0.0666667, 20.0));
    assert_eq!((o.air_gravity_scale, o.water_gravity_scale), (0.0, 0.4));
    assert_eq!((o.air_damage_range.lower, o.air_damage_range.upper), (0.0, 100.0));
    assert_eq!((o.water_damage_range.lower, o.water_damage_range.upper), (0.0, 10.0));
    assert_eq!((o.initial_velocity, o.final_velocity), (0.133333, 0.133333));
    assert_eq!(o.guided_angular_velocity, 1.5708);
    assert_eq!(o.detonation_noise, NoiseLevel::Medium);
    assert!(o.detonation_started.is_null());
    assert_eq!(map.tag(o.flyby_sound.id).unwrap().class, class::SOUND);
    assert_eq!(
        map.tag(o.attached_detonation_damage.id).unwrap().class,
        class::DAMAGE_EFFECT
    );
    assert_eq!(
        map.tag(o.impact_damage.id).unwrap().path,
        "weapons\\test\\impact damage"
    );
    assert_eq!(o.material_responses.len(), 3);
}

#[test]
fn reads_the_material_responses_and_looks_them_up() {
    let map = map_with(3, None);
    let p = Projectile::read(&map, tag(&map)).unwrap().own;
    let dirt = p.response(MaterialType::Dirt).unwrap();
    assert_eq!(dirt.default_response, ResponseType::Disappear);
    assert!(!dirt.has_potential());
    let stone = p.response(MaterialType::Stone).unwrap();
    assert_eq!(stone.default_response, ResponseType::Reflect);
    assert_eq!(stone.potential_response, ResponseType::Attach);
    assert!(stone.has_potential());
    assert!(stone.potential_flags.contains(PotentialFlags::ONLY_AGAINST_UNITS));
    assert_eq!(stone.skip_fraction, 0.2);
    assert_eq!((stone.between_angle.lower, stone.between_angle.upper), (0.0, 0.4363));
    assert_eq!((stone.and_velocity.lower, stone.and_velocity.upper), (0.0, 0.5));
    assert!(stone.potential_effect.is_null());
    assert_eq!(map.tag(stone.default_effect.id).unwrap().class, class::EFFECT);
    assert_eq!(stone.scale_effects_by, ScaleEffectsBy::Angle);
    assert_eq!((stone.angular_noise, stone.velocity_noise), (0.2618, 0.01));
    assert_eq!(map.tag(stone.detonation_effect.id).unwrap().class, class::EFFECT);
    assert_eq!(stone.initial_friction, 0.7);
    assert_eq!(stone.maximum_distance, 100.0);
    assert_eq!((stone.parallel_friction, stone.perpendicular_friction), (0.3, 0.4));

    // A type past the block's count has no element: the built-in response.
    assert!(p.response(MaterialType::Snow).is_none());
    let none = p.response_or_built_in(Some(MaterialType::HunterShield));
    assert_eq!(none.default_response, ResponseType::Disappear);
    assert!(!none.has_potential());
    assert!(none.default_effect.is_null() && none.potential_effect.is_null() && none.detonation_effect.is_null());
    assert_eq!(
        (
            none.initial_friction,
            none.parallel_friction,
            none.perpendicular_friction
        ),
        (0.0, 0.0, 0.0)
    );
    // The "none" material type too.
    assert_eq!(p.response_or_built_in(None), none);
    assert_eq!(
        p.response_or_built_in(Some(MaterialType::Stone)).default_response,
        ResponseType::Reflect
    );
}

#[test]
fn composes_with_the_object_dispatch() {
    let map = map_with(1, None);
    let t = ObjectTag::read(&map, tag(&map)).unwrap();
    assert_eq!(t.object().bounding_radius, 0.5);
    let ObjectTag::Projectile(p) = t else {
        panic!("not a projectile")
    };
    assert_eq!(p.own.maximum_range, 20.0);
}

#[test]
fn refuses_damage() {
    // A response code outside the enumeration names the tag and offset.
    let map = map_with(3, Some(9));
    let err = Projectile::read(&map, tag(&map)).unwrap_err();
    assert!(err.to_string().contains("ResponseType"), "{err}");
    assert!(err.to_string().contains("weapons\\test\\needle"), "{err}");
    // A class that is not a projectile.
    let err = Projectile::read(&map, &map.tags()[JPT]).unwrap_err();
    assert!(err.to_string().contains("not a projectile"), "{err}");
    // Too many responses.
    let map = map_with(65, None);
    let err = Projectile::read(&map, tag(&map)).unwrap_err();
    assert!(err.to_string().contains("material responses"), "{err}");
}

/// The part of a spec flight row: speeds, windows.
fn flight(v0: f32, vf: f32, air: (f32, f32)) -> blam_formats::projectile::ProjectilePart {
    let map = map_with(0, None);
    let mut p = Projectile::read(&map, tag(&map)).unwrap().own;
    p.initial_velocity = v0;
    p.final_velocity = vf;
    p.air_damage_range.lower = air.0;
    p.air_damage_range.upper = air.1;
    p
}

#[test]
fn deceleration_matches_the_flight_table() {
    // Shotgun pellet: 4.6667 to 3.3333 over 1.5 - 3: 3.5555552.
    let p = flight(4.666667, 3.333333, (1.5, 3.0));
    assert!(
        (p.air_deceleration() - 3.5555552).abs() < 1e-5,
        "{}",
        p.air_deceleration()
    );
    // Its step: lower / initial velocity.
    assert!((p.deceleration_step(p.air_damage_range) - 1.5 / 4.666667).abs() < 1e-6);
    // Plasma rifle bolt: 1.6667 to 0.8333 over 20 - 50: 0.0347222.
    let p = flight(1.666667, 0.833333, (20.0, 50.0));
    assert!((p.air_deceleration() - 0.0347222).abs() < 1e-6);
    // Warthog bullet: 9.7667 to 0.3333 over 0 - 100: 0.4763834; no lower
    // bound: the timer is active at once (step 0).
    let p = flight(9.7667, 0.33333, (0.0, 100.0));
    assert!((p.air_deceleration() - 0.4763834).abs() < 1e-4);
    assert_eq!(p.deceleration_step(p.air_damage_range), 0.0);
    // Rocket: 0.4 to 0.3333 over 0 - 100: 0.0002444.
    let p = flight(0.4, 0.333333, (0.0, 100.0));
    assert!((p.air_deceleration() - 0.0002444).abs() < 1e-6);
    // Charged bolt: 0.5 to 0.3 over 5 - 20: 0.0053333.
    let p = flight(0.5, 0.3, (5.0, 20.0));
    assert!((p.air_deceleration() - 0.0053333).abs() < 1e-6);
    // Equal speeds, or an empty window: no deceleration.
    assert_eq!(flight(10.0, 10.0, (5.0, 15.0)).air_deceleration(), 0.0);
    assert_eq!(flight(33.3333, 33.3333, (1000.0, 1000.0)).air_deceleration(), 0.0);
    assert_eq!(flight(1.0, 0.5, (3.0, 3.0)).air_deceleration(), 0.0);
}

#[test]
fn direct_hit_scale_follows_the_speed() {
    // Warthog bullet (test case (b)): speeds at impact and the scales.
    let p = flight(9.7667, 0.33333, (0.0, 100.0));
    for (speed, scale) in [(9.51669, 0.9735), (8.17026, 0.8308), (6.17785, 0.6196)] {
        assert!((p.direct_hit_scale(speed) - scale).abs() < 1e-4, "{speed}");
    }
    // Plasma rifle bolt: 1.55899 at update 4: 0.8708; 1.17848: 0.4142.
    let p = flight(1.666667, 0.833333, (20.0, 50.0));
    assert!((p.direct_hit_scale(1.55899) - 0.8708).abs() < 1e-4);
    assert!((p.direct_hit_scale(1.17848) - 0.4142).abs() < 1e-4);
    // Shotgun pellet: restored to the start speed clamps to 1; the final
    // speed is 0; a lower speed clamps to 0.
    let p = flight(4.666667, 3.333333, (1.5, 3.0));
    assert_eq!(p.direct_hit_scale(4.66668), 1.0);
    assert!(p.direct_hit_scale(3.30001) == 0.0);
    // v0 = vf: always 1 (pistol bullet).
    let p = flight(10.0, 10.0, (0.0, 100.0));
    assert_eq!(p.direct_hit_scale(10.0), 1.0);
    assert_eq!(p.direct_hit_scale(3.0), 1.0);
}

#[test]
fn timers_in_ticks_and_the_potential_response() {
    let map = map_with(3, None);
    let p = Projectile::read(&map, tag(&map)).unwrap().own;
    assert!(p.has_timer());
    // 0.75 s is 22.5: truncated to 22 ticks (the spec's 23 updates come from
    // the 1 / (0.75 x 30) step, not from this).
    assert_eq!((p.timer_ticks().lower, p.timer_ticks().upper), (22, 22));
    let stone = p.response(MaterialType::Stone).unwrap();
    // Only against units; window 0 to 0.4363 rad and 0 to 0.5 WU/tick.
    assert!(stone.potential_applies(0.2, 0.1, true));
    assert!(!stone.potential_applies(0.2, 0.1, false));
    assert!(!stone.potential_applies(0.5, 0.1, true));
    assert!(!stone.potential_applies(0.2, 0.6, true));
    // The dirt element has no potential response.
    assert!(
        !p.response(MaterialType::Dirt)
            .unwrap()
            .potential_applies(0.0, 0.0, true)
    );
    // A window whose upper bound is 0 is no condition.
    let mut open = stone.clone();
    open.between_angle.upper = 0.0;
    open.and_velocity.upper = 0.0;
    assert!(open.potential_applies(1.5, 99.0, true));
}
