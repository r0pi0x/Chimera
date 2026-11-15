//! The effect family readers on the user's Halo disc image, against
//! specs/halo_ce/effects.md's disc-backed cases (b): the assault rifle's
//! firing effect, its muzzle flash light, the flash and casing particles,
//! the stones' point physics, the pistol bullet's stone impact and the
//! stone bullet hole, the frag grenade's explosion, `globals\cyborg`; a
//! sweep that every effect, particle, point physics, light, decal,
//! material effects and contrail tag of `bloodgulch` parses with
//! references resolving to their class; and every tag the four weapons'
//! firing and impact effects reach parses on `hangemhigh` too. Each test
//! passes with a note when there is no image.

#![allow(clippy::excessive_precision)]

use blam_formats::cache::{MapFile, TagRef, class};
use blam_formats::effect::{Contrail, Decal, Effect, Light, MaterialEffects, Particle, PointPhysics};
use blam_formats::projectile::Projectile;
use blam_formats::weapon::Weapon;

use super::maps::load;
use super::object_install::path_of;

fn find<'a>(map: &'a MapFile, c: blam_formats::cache::ClassCode, path: &str) -> &'a blam_formats::cache::TagEntry {
    map.find(c, path).unwrap_or_else(|| panic!("no {path}"))
}

fn close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() <= 5e-4 * b.abs() + 1e-6, "{what}: {a} vs {b}");
}

#[test]
fn the_assault_rifles_firing_effect() {
    let Some(m) = load("bloodgulch") else { return };
    let e = Effect::read(&m, find(&m, class::EFFECT, "weapons\\assault rifle\\effects\\fire bullet")).unwrap();
    assert_eq!((e.flags, e.loop_start, e.loop_stop), (0, -1, -1));
    assert_eq!(e.locations, ["primary trigger", "primary ejection", "secondary ejection"]);
    assert_eq!(e.events.len(), 4);
    for ev in &e.events {
        assert_eq!((ev.delay.lower, ev.delay.upper, ev.duration.upper, ev.skip_fraction), (0.0, 0.0, 0.0, 0.0));
    }
    let flash = |r: &TagRef| path_of(&m, r).unwrap();
    assert_eq!(e.events[0].parts.len(), 0);
    assert_eq!(e.events[0].particles.len(), 6);
    for p in &e.events[0].particles {
        assert_eq!(flash(&p.particle), "effects\\particles\\flash\\flash h ar");
        assert_eq!(p.camera_mode, 2);
    }
    let e1 = &e.events[1];
    assert_eq!(e1.parts.len(), 2);
    assert_eq!(e1.parts[0].base_class, class::SOUND);
    assert_eq!(path_of(&m, &e1.parts[0].kind).unwrap(), "sound\\sfx\\weapons\\assault rifle\\fire");
    assert_eq!(e1.parts[0].location, 0);
    assert_eq!(path_of(&m, &e1.parts[1].kind).unwrap(), "sound\\sfx\\weapons\\pistol\\eject");
    assert_eq!(e1.parts[1].location, 2);
    assert_eq!(e1.particles.len(), 8);
    let casing = e1.particles.iter().find(|p| flash(&p.particle).ends_with("casing h ar bullet")).unwrap();
    assert_eq!((casing.location, casing.camera_mode), (1, 3));
    close(casing.velocity.lower, 2.0, "casing velocity");
    close(casing.velocity.upper, 2.5, "casing velocity");
    close(casing.velocity_cone, 0.2618, "casing cone");
    close(casing.radius.lower, 0.015, "casing radius");
    assert_eq!(e.events[2].particles.len(), 6);
    assert!(e.events[2].particles.iter().all(|p| p.camera_mode == 1));
    assert_eq!(e.events[3].particles.len(), 4);

    // Its weapon's light attachment at "primary trigger".
    let w = Weapon::read(&m, find(&m, class::WEAPON, "weapons\\assault rifle\\assault rifle")).unwrap();
    let at = w.object.attachments.iter().find(|a| a.attached.class == class::LIGHT).unwrap();
    assert_eq!(path_of(&m, &at.attached).unwrap(), "weapons\\assault rifle\\muzzle flash");
    let l = Light::read(&m, find(&m, class::LIGHT, "weapons\\assault rifle\\muzzle flash")).unwrap();
    assert_eq!(l.flags, 0x21);
    close(l.radius, 2.0, "radius");
    assert_eq!((l.radius_modifier.lower, l.radius_modifier.upper), (1.0, 1.0));
    close(l.color_upper.r, 0.961, "r");
    close(l.color_upper.g, 0.910, "g");
    close(l.color_upper.b, 0.686, "b");
    assert_eq!(l.duration, 0.0);
}

#[test]
fn the_flash_casing_and_stones() {
    let Some(m) = load("bloodgulch") else { return };
    let f = Particle::read(&m, find(&m, class::PARTICLE, "effects\\particles\\flash\\flash h ar")).unwrap();
    assert_eq!(f.flags, 0x288);
    assert_eq!(path_of(&m, &f.bitmap).unwrap(), "effects\\particles\\flash\\bitmaps\\flash h ar fp");
    assert_eq!(path_of(&m, &f.physics).unwrap(), "effects\\point physics\\cold smoke cloud");
    assert_eq!((f.lifespan.lower, f.lifespan.upper), (0.075, 0.075));
    assert_eq!((f.first_sequence, f.initial_sequences, f.orientation, f.shader.blend), (0, 6, 1, 3));
    close(f.sprite_size_inverse, 1.0 / 135.0, "1 / sprite size (the spec: 0.0074 = 1/135)");
    let c = Particle::read(&m, find(&m, class::PARTICLE, "effects\\particles\\solid\\casing h ar bullet")).unwrap();
    assert_eq!(c.flags, 7);
    assert_eq!((c.lifespan.lower, c.fade_out, c.looping_sequences), (30.0, 1.0, 3));
    assert_eq!(path_of(&m, &c.physics).unwrap(), "effects\\point physics\\stones");
    assert_eq!(path_of(&m, &c.material_effects).unwrap(), "sound\\sfx\\impulse\\shellcasings\\ar_casings");
    assert_eq!((c.animation_rate.lower, c.animation_rate.upper, c.contact_deterioration), (6.0, 30.0, 5.0));
    let s = PointPhysics::read(&m, find(&m, class::POINT_PHYSICS, "effects\\point physics\\stones")).unwrap();
    assert_eq!(s.flags, 3);
    close(s.density, 2.4, "density");
    close(s.air_friction, 900.0, "air friction");
    close(s.water_friction, 14000.0, "water friction");
    close(s.surface_friction, 0.1, "surface friction");
    close(s.elasticity, 0.35, "elasticity");
    close(s.mass_per_r3, 284672.03, "mass");
    close(s.water_buoyancy, -0.4118, "water buoyancy");
    close(s.air_buoyancy, -0.9991, "air buoyancy");
}

#[test]
fn the_pistol_bullets_stone_impact() {
    let Some(m) = load("bloodgulch") else { return };
    let p = Projectile::read(&m, find(&m, class::PROJECTILE, "weapons\\pistol\\bullet")).unwrap();
    let r = &p.own.material_responses[2];
    assert_eq!(path_of(&m, &r.default_effect).unwrap(), "weapons\\pistol\\effects\\impact stone");
    let e = Effect::read(&m, m.tag(r.default_effect.id).unwrap()).unwrap();
    assert_eq!(e.locations, ["reflection", "negative incident", "normal"]);
    assert_eq!(e.events.len(), 2);
    let e0 = &e.events[0];
    close(e0.duration.lower, 0.25, "duration");
    assert_eq!(path_of(&m, &e0.parts[0].kind).unwrap(), "effects\\decals\\bullet holes\\stone");
    assert_eq!(e0.parts[0].location, 1);
    close(e0.parts[0].velocity.lower, 0.1, "trace");
    assert_eq!(path_of(&m, &e0.parts[1].kind).unwrap(), "sound\\sfx\\impulse\\impacts\\granhit");
    assert_eq!(e0.parts[1].location, 0);
    let names: Vec<String> = e0.particles.iter().map(|q| path_of(&m, &q.particle).unwrap()).collect();
    assert!(names.iter().any(|n| n.ends_with("sparks trail")) && names.iter().any(|n| n.ends_with("gravel small")));
    let fl = e0.particles.iter().find(|q| path_of(&m, &q.particle).unwrap().ends_with("flash h ar")).unwrap();
    assert_eq!((fl.location, fl.count.lower.min(fl.count.upper), fl.count.lower.max(fl.count.upper)), (2, 2, 3));
    assert_eq!(path_of(&m, &e.events[1].parts[0].kind).unwrap(), "effects\\decals\\bullet holes\\stone med");
    let d = Decal::read(&m, find(&m, class::DECAL, "effects\\decals\\bullet holes\\stone")).unwrap();
    assert_eq!((d.kind, d.layer, d.blend), (0, 0, 2));
    close(d.radius.lower, 0.035, "radius");
    close(d.radius.upper, 0.065, "radius");
    assert_eq!((d.lifetime.lower, d.decay.lower), (300.0, 1.0));
    assert_eq!(path_of(&m, &d.map).unwrap(), "effects\\decals\\bullet holes\\bitmaps\\stone");
}

#[test]
fn the_frag_explosion_and_the_cyborgs_footsteps() {
    let Some(m) = load("bloodgulch") else { return };
    let e = Effect::read(&m, find(&m, class::EFFECT, "weapons\\frag grenade\\effects\\explosion")).unwrap();
    assert_eq!(e.flags, 2);
    assert_eq!(e.danger_radius, 8.0);
    assert_eq!((e.events.len(), e.events[0].parts.len(), e.events[0].particles.len()), (1, 7, 1));
    let f = MaterialEffects::read(&m, find(&m, class::MATERIAL_EFFECTS, "globals\\cyborg")).unwrap();
    assert_eq!(f.kinds.len(), 6);
    assert!(f.kinds.iter().all(|k| k.len() == 32));
    assert_eq!(path_of(&m, &f.get(1, 2).unwrap().sound).unwrap(), "sound\\sfx\\impulse\\footsteps\\cyborg\\grtcement\\run");
    assert_eq!(path_of(&m, &f.get(0, 28).unwrap().effect).unwrap(), "effects\\footsteps water");
}

/// Every tag of the family on a map parses, references resolving.
fn sweep(m: &MapFile) -> usize {
    let mut n = 0;
    for t in m.tags() {
        let refs: Vec<TagRef> = match t.class {
            c if c == class::EFFECT => Effect::read(m, t).unwrap().references(),
            c if c == class::PARTICLE => {
                let p = Particle::read(m, t).unwrap();
                vec![p.bitmap, p.physics, p.material_effects, p.collision_effect, p.death_effect]
            }
            c if c == class::POINT_PHYSICS => {
                PointPhysics::read(m, t).unwrap();
                vec![]
            }
            c if c == class::LIGHT => vec![Light::read(m, t).unwrap().lens_flare],
            c if c == class::DECAL => {
                let d = Decal::read(m, t).unwrap();
                vec![d.next, d.map]
            }
            c if c == class::MATERIAL_EFFECTS => MaterialEffects::read(m, t)
                .unwrap()
                .kinds
                .iter()
                .flatten()
                .flat_map(|e| [e.effect, e.sound])
                .collect(),
            c if c == class::CONTRAIL => {
                let c = Contrail::read(m, t).unwrap();
                std::iter::once(c.bitmap).chain(c.point_states.iter().map(|s| s.physics)).collect()
            }
            _ => continue,
        };
        n += 1;
        for r in refs {
            let _ = path_of(m, &r);
        }
    }
    n
}

#[test]
fn every_effect_family_tag_parses() {
    let Some(m) = load("bloodgulch") else { return };
    // 318 effe, 96 part, 30 pphy, 32 ligh, 39 deca, 5 foot, 8 cont.
    assert_eq!(sweep(&m), 318 + 96 + 30 + 32 + 39 + 5 + 8);
    let Some(h) = load("hangemhigh") else { return };
    assert!(sweep(&h) > 400);
}
