//! The effect family readers (`blam_formats::effect`) on synthetic tags
//! with the numbers of specs/halo_ce/effects.md's test cases (a): the
//! synthetic effect's layout (two locations, one event, a sound and a
//! decal part, one particle entry), a particle and its fade, the point
//! physics' motion table and bounce, a light's colour at an intensity, a
//! decal's intensity byte over its life, material effects and a contrail;
//! and the refusals (a wrong class, too many events).

#![allow(clippy::excessive_precision)]

use blam_formats::cache::{MapFile, class};
use blam_formats::effect::{Contrail, Decal, Effect, Light, MaterialEffects, Particle, PointPhysics};

use crate::map_fixture::{CacheFile, Struct, TagData, c, null_ref};

const SCNR: usize = 0;
const SND: usize = 1;
const DECA: usize = 2;
const PART: usize = 3;
const EFFE: usize = 4;
const PPHY: usize = 5;
const LIGH: usize = 6;
const FOOT: usize = 7;
const CONT: usize = 8;

fn tags() -> Vec<(&'static str, &'static str)> {
    vec![
        ("scnr", "scenarios\\syn"),
        ("snd!", "sound\\test\\fire"),
        ("deca", "effects\\decals\\test"),
        ("part", "effects\\particles\\test"),
        ("effe", "weapons\\test\\effects\\fire"),
        ("pphy", "effects\\point physics\\stones"),
        ("ligh", "weapons\\test\\muzzle flash"),
        ("foot", "sound\\test\\casings"),
        ("cont", "weapons\\test\\tracer"),
    ]
}

fn map(events: usize) -> MapFile {
    let mut td = TagData::new(&tags());
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    let locations = td.block(&[Struct::new(0x20).str(0, "primary trigger").0, Struct::new(0x20).0]);
    let part0 = Struct::new(0x68)
        .i16(0x00, 1)
        .i16(0x04, 0)
        .u32(0x14, c("snd!"))
        .put(0x18, &td.tag_ref(SND))
        .0;
    let part1 = Struct::new(0x68)
        .i16(0x02, 1)
        .i16(0x04, 1)
        .u16(0x06, 1)
        .u32(0x14, c("deca"))
        .put(0x18, &td.tag_ref(DECA))
        .f32s(0x40, &[0.5, 0.5])
        .f32s(0x54, &[1.0, 2.0])
        .0;
    let parts = td.block(&[part0, part1]);
    let entry = Struct::new(0xE8)
        .f32s(0x20, &[1.0, 0.0, 0.0])
        .put(0x54, &td.tag_ref(PART))
        .i16(0x68, 2)
        .i16(0x6C, 3)
        .i16(0x6E, 3)
        .f32s(0x84, &[1.0, 1.0])
        .f32s(0xA0, &[0.1, 0.2])
        .f32s(0xB0, &[1.0, 1.0, 1.0, 1.0])
        .f32s(0xC0, &[0.0, 1.0, 0.0, 0.0])
        .0;
    let particles = td.block(&[entry]);
    let event = Struct::new(0x44)
        .f32s(0x08, &[0.1, 0.1])
        .f32s(0x10, &[0.2, 0.2])
        .put(0x2C, &parts)
        .put(0x38, &particles)
        .0;
    let events = td.block(&vec![event; events]);
    let effe = Struct::new(0x40).i16(0x04, -1).i16(0x06, -1).put(0x28, &locations).put(0x34, &events);
    td.set_struct(EFFE, &effe.0);
    let part = Struct::new(0x164)
        .u32(0x000, 0x288)
        .put(0x004, &null_ref("bitm"))
        .put(0x014, &td.tag_ref(PPHY))
        .put(0x024, &null_ref("foot"))
        .f32s(0x038, &[0.5, 0.5])
        .f32(0x040, 0.1)
        .f32(0x044, 0.2)
        .put(0x048, &null_ref("effe"))
        .put(0x058, &null_ref("effe"))
        .f32s(0x074, &[1.0, 2.0])
        .i16(0x09A, 6)
        .f32(0x0A8, 0.0074)
        .i16(0x0AC, 1)
        .i16(0x0B0 + 0x2A, 3);
    td.set_struct(PART, &part.0);
    let pphy = Struct::new(0x40)
        .u32(0, 3)
        .f32s(0x04, &[284672.03, -0.4118, -0.99908])
        .f32s(0x20, &[2.4, 900.0, 14000.0, 0.1, 0.35]);
    td.set_struct(PPHY, &pphy.0);
    let ligh = Struct::new(0x160)
        .u32(0, 0x21)
        .f32s(0x004, &[10.0, 1.0, 1.0])
        .f32s(0x038, &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.808, 0.357])
        .put(0x0AC, &null_ref("lens"))
        .f32(0x0F4, 45.0);
    td.set_struct(LIGH, &ligh.0);
    let deca = Struct::new(0x10C)
        .put(0x008, &null_ref("deca"))
        .f32s(0x018, &[0.035, 0.065])
        .f32s(0x078, &[300.0, 300.0, 1.0, 1.0])
        .i16(0x0C0, 2)
        .put(0x0D8, &null_ref("bitm"));
    td.set_struct(DECA, &deca.0);
    let mats: Vec<Vec<u8>> = (0..3)
        .map(|i| Struct::new(0x30).put(0, &null_ref("effe")).put(0x10, &if i == 2 { td.tag_ref(SND) } else { null_ref("snd!") }).0)
        .collect();
    let mats = td.block(&mats);
    let kinds: Vec<Vec<u8>> = (0..9).map(|_| Struct::new(0x1C).put(0, &mats).0).collect();
    let kinds = td.block(&kinds);
    td.set_struct(FOOT, &Struct::new(0x0C).put(0, &kinds).0);
    let state = Struct::new(0x68)
        .f32s(0x08, &[0.01, 0.0125])
        .put(0x10, &null_ref("pphy"))
        .f32(0x40, 0.05)
        .f32s(0x44, &[1.0, 1.0, 0.89, 0.28])
        .0;
    let states = td.block(&[state.clone(), state]);
    let cont = Struct::new(0x144)
        .f32(0x004, 60.0)
        .i16(0x018, 4)
        .put(0x030, &null_ref("bitm"))
        .i16(0x084 + 0x2A, 3)
        .put(0x138, &states);
    td.set_struct(CONT, &cont.0);
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

fn tag(map: &MapFile, i: usize) -> &blam_formats::cache::TagEntry {
    &map.tags()[i]
}

#[test]
fn reads_the_synthetic_effect() {
    let m = map(1);
    let e = Effect::read(&m, tag(&m, EFFE)).unwrap();
    assert_eq!(e.locations, vec!["primary trigger".to_string(), String::new()]);
    assert_eq!((e.loop_start, e.loop_stop), (-1, -1));
    assert_eq!(e.events.len(), 1);
    let ev = &e.events[0];
    assert_eq!((ev.skip_fraction, ev.delay.lower, ev.delay.upper), (0.0, 0.1, 0.1));
    assert_eq!((ev.duration.lower, ev.duration.upper), (0.2, 0.2));
    let p0 = &ev.parts[0];
    assert_eq!((p0.environment, p0.mode, p0.location), (1, 0, 0));
    assert_eq!(p0.base_class, class::SOUND);
    let p1 = &ev.parts[1];
    assert_eq!((p1.environment, p1.mode, p1.location, p1.flags), (0, 1, 1, 1));
    assert_eq!(p1.base_class, class::DECAL);
    assert_eq!(p1.velocity.lower, 0.5);
    assert_eq!((p1.radius_modifier.lower, p1.radius_modifier.upper), (1.0, 2.0));
    let q = &ev.particles[0];
    assert_eq!((q.count.lower, q.count.upper, q.distribution), (3, 3, 2));
    assert_eq!((q.radius.lower, q.radius.upper), (0.1, 0.2));
    assert_eq!(q.direction_vector, [1.0, 0.0, 0.0]);
    assert_eq!(e.references().len(), 3);
}

#[test]
fn refuses_too_many_events_and_the_wrong_class() {
    let m = map(33);
    assert!(Effect::read(&m, tag(&m, EFFE)).is_err());
    let m = map(1);
    assert!(Effect::read(&m, tag(&m, PART)).is_err());
    assert!(Particle::read(&m, tag(&m, EFFE)).is_err());
}

#[test]
fn reads_a_particle_and_its_fade() {
    let m = map(1);
    let p = Particle::read(&m, tag(&m, PART)).unwrap();
    assert_eq!(p.flags, 0x288);
    assert!(p.has(Particle::ANIMATE_ONCE_PER_FRAME) && p.has(Particle::SELF_ILLUMINATED));
    assert_eq!((p.lifespan.lower, p.initial_sequences, p.orientation), (0.5, 6, 1));
    assert_eq!(p.shader.blend, 3);
    assert_eq!(p.sprite_size_inverse, 0.0074);
    // The spec's motion table: fade 0, 1/3, 1, 5/6 after 0, 1, 5, 10
    // updates; radius factor 1 + age / 0.5.
    for (n, fade, r) in [(0, 0.0, 0.02), (1, 0.3333, 0.02133), (5, 1.0, 0.02667), (10, 0.8333, 0.03333)] {
        let a = n as f32 / 30.0;
        assert!((p.fade(a, 0.5) - fade).abs() < 1e-3, "fade at {n}");
        assert!((0.02 * p.radius_factor(a, 0.5) - r).abs() < 1e-4, "radius at {n}");
    }
}

#[test]
fn point_physics_moves_as_the_spec_table() {
    let m = map(1);
    let ph = PointPhysics::read(&m, tag(&m, PPHY)).unwrap();
    assert_eq!((ph.flags, ph.density, ph.elasticity), (3, 2.4, 0.35));
    let (mut x, mut v) = ([0.0, 0.0, 1.0], [2.0, 0.0, 1.0]);
    let pp = Particle::read(&m, tag(&m, PART)).unwrap();
    let rows = [(1, [0.06634, 0.0, 1.02962], [1.99012, 0.0, 0.88873]), (5, [0.32878, 0.0, 1.11151], [1.95622, 0.0, 0.45060]), (10, [0.65136, 0.0, 1.13281], [1.92273, 0.0, -0.08606])];
    let mut n = 0;
    for (want, wx, wv) in rows {
        while n < want {
            n += 1;
            let r = 0.02 * pp.radius_factor(n as f32 / 30.0, 0.5);
            (x, v) = ph.step_air(r, x, v, 1.0 / 30.0);
        }
        for i in 0..3 {
            assert!((x[i] - wx[i]).abs() < 1e-4, "x after {want}: {x:?}");
            assert!((v[i] - wv[i]).abs() < 1e-4, "v after {want}: {v:?}");
        }
    }
    let b = ph.bounce([1.0, 0.0, -2.0], [0.0, 0.0, 1.0]);
    assert!((b[0] - 0.9).abs() < 1e-6 && (b[2] - 0.7).abs() < 1e-6);
    // A vacuum particle keeps its speed.
    let vac = PointPhysics { flags: PointPhysics::NO_GRAVITY, mass_per_r3: 0.0, air_friction: 0.0, ..ph };
    let (mut x, mut v) = ([0.0; 3], [3.0, 0.0, 0.0]);
    for _ in 0..10 {
        (x, v) = vac.step_air(0.05, x, v, 1.0 / 30.0);
    }
    assert!((x[0] - 1.0).abs() < 1e-5 && v == [3.0, 0.0, 0.0]);
}

#[test]
fn light_decal_material_effects_and_contrail() {
    let m = map(1);
    let l = Light::read(&m, tag(&m, LIGH)).unwrap();
    assert_eq!((l.flags, l.radius, l.duration), (0x21, 10.0, 45.0));
    // The spec's effect light at 15 of 45 ticks, linear fall-off.
    let (c, r) = l.at(1.0 - 15.0 / 45.0);
    assert!((c[0] - 0.6667).abs() < 1e-3 && (c[1] - 0.5387).abs() < 1e-3 && (c[2] - 0.2380).abs() < 1e-3);
    assert_eq!(r, 10.0);
    let d = Decal::read(&m, tag(&m, DECA)).unwrap();
    assert_eq!((d.radius.lower, d.radius.upper, d.blend, d.lifetime.lower), (0.035, 0.065, 2, 300.0));
    // The spec's intensity byte at 0, 150, 165 and 180 s of 180 / 20.
    assert_eq!(Decal::intensity_at(180.0, 20.0, 0.0), Some(255));
    assert_eq!(Decal::intensity_at(180.0, 20.0, 150.0), Some(255));
    assert_eq!(Decal::intensity_at(180.0, 20.0, 165.0), Some(191));
    assert_eq!(Decal::intensity_at(180.0, 20.0, 180.0), None);
    let f = MaterialEffects::read(&m, tag(&m, FOOT)).unwrap();
    assert_eq!(f.kinds.len(), 9);
    assert!(f.get(MaterialEffects::IMPACT, 2).is_some_and(|e| !e.sound.id.is_none()));
    assert!(f.get(MaterialEffects::IMPACT, 3).is_none());
    let c = Contrail::read(&m, tag(&m, CONT)).unwrap();
    assert_eq!((c.point_rate, c.render_type, c.shader.blend, c.point_states.len()), (60.0, 4, 3, 2));
    assert_eq!(c.point_states[0].width, 0.05);
}
