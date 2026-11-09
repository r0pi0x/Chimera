//! HL2:DM's projectile effects on the headless harness (specs/hl2dm/
//! effects_sounds.md, H9's cases by number, and projectiles.md's visuals):
//! an explosion's sound and look (39), the explosion entity's rule (41,
//! 42), the rocket's flares and smoke (45), the laser dot (47), the
//! energy ball's body, bounce and rings, the bolt's blur and glow, the
//! contact grenade's smoke and the frag's glow. Particle counts are read
//! right after a tick: the per-frame looks live one frame.

use bevy::prelude::*;
use chimera_core::core::{BlastFlags, Explosion, LocalPlayer, SimClock};
use chimera_sim::{
    testkit,
    weapon::{
        Inventory, Weapon, give,
        grenade::{Detonated, GrenadeKind},
        projectile::{Flyer, FlyerGone, FlyerHit},
    },
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        PlaySound,
        loose::ShownItem,
        particles::{Fade, MapParticles, ParticleMaterial, ParticleMaterials, Particles},
    },
};
use hl2dm::{
    game::{
        effects, movement, projectile_fx,
        weapons::{
            self,
            launch::{BALL_MODEL, BOLT_MODEL, Hl2Rounds, MISSILE_MODEL, SMG1_GRENADE_MODEL, U},
        },
    },
    harness::Hl2Sim,
};
use source_engine::effects::{explosions, impact_effects::MATERIALS as SOURCE_MATERIALS};

fn sim() -> Sim {
    let mut sim = Sim::new(Hl2Sim, GreyboxMapPlugin);
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    let materials = SOURCE_MATERIALS
        .iter()
        .chain(effects::MATERIALS)
        .chain(projectile_fx::MATERIALS)
        .chain(explosions::MATERIALS)
        .map(|n| ParticleMaterial { name: n.to_string(), ..default() })
        .collect();
    sim.app.insert_resource(ParticleMaterials(MapParticles { materials }));
    sim
}

fn material(sim: &Sim, name: &str) -> usize {
    sim.app.world().resource::<ParticleMaterials>().0.find(name).unwrap_or_else(|| panic!("no {name}"))
}

/// The particles of material `name` alive now: (position, half-size, colour grey, alpha).
fn drawn(sim: &Sim, name: &str) -> Vec<(Vec3, f32, f32, f32)> {
    let m = material(sim, name);
    sim.app
        .world()
        .resource::<Particles>()
        .groups
        .iter()
        .flat_map(|g| g.particles.iter())
        .filter(|p| p.material == m)
        .map(|p| (p.position, p.current_size(), p.color.x, p.current_alpha()))
        .collect()
}

fn clear(sim: &mut Sim) {
    sim.app.world_mut().resource_mut::<Particles>().groups.clear();
}

fn detonated(sim: &mut Sim, weapon: &'static str, at: Vec3) {
    sim.app.world_mut().write_message(Detonated {
        kind: GrenadeKind::Blast,
        weapon,
        thrower: None,
        at,
        ground: None,
        projectile: Entity::PLACEHOLDER,
        shake: None,
        flags: Default::default(),
    });
}

fn flagged(sim: &mut Sim, flags: u16, at: Vec3) {
    sim.app.world_mut().write_message(Detonated {
        kind: GrenadeKind::Blast,
        weapon: weapons::RPG,
        thrower: None,
        at,
        ground: None,
        projectile: Entity::PLACEHOLDER,
        shake: None,
        flags: BlastFlags(flags),
    });
}

/// The explosion sounds heard over `ticks` ticks: (entry, where).
fn heard(sim: &mut Sim, ticks: u64) -> Vec<(String, Option<Vec3>)> {
    let mut cursor = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let mut out = Vec::new();
    for _ in 0..ticks {
        sim.ticks(1);
        out.extend(cursor.read(sim.app.world().resource::<Messages<PlaySound>>()).map(|s| (s.entry.clone(), s.at)));
    }
    out
}

/// Test 39 and 40's sound: an explosion plays `BaseExplosionEffect.Sound`
/// where it is, and draws its core (smoke puffs and the glow).
#[test]
fn an_explosion_is_heard_and_drawn_39() {
    let mut sim = sim();
    sim.seconds(0.5);
    clear(&mut sim);
    let at = Vec3::new(0.0, 1.0, 0.0);
    detonated(&mut sim, weapons::FRAG, at);
    let sounds = heard(&mut sim, 1);
    let booms: Vec<_> = sounds.iter().filter(|(e, _)| e == projectile_fx::EXPLOSION_SOUND).collect();
    assert_eq!(booms.len(), 1);
    assert!(booms[0].1.unwrap().distance(at) < 1e-3);
    assert!(!drawn(&sim, "particle/particle_noisesphere").is_empty(), "the core's smoke");
    assert!(!drawn(&sim, "sprites/light_glow02_add_noz").is_empty(), "the flash");
}

/// Tests 41 and 42: a missile on a wall well above the floor is drawn at
/// its own point; one on the floor is drawn 24 units up.
#[test]
fn the_rockets_explosion_follows_the_entity_rule_41_42() {
    let mut sim = sim();
    sim.seconds(0.5);
    detonated(&mut sim, weapons::RPG, Vec3::new(0.0, 100.0 * U, 0.0));
    let sounds = heard(&mut sim, 1);
    let boom = sounds.iter().find(|(e, _)| e == projectile_fx::EXPLOSION_SOUND).unwrap();
    assert!((boom.1.unwrap().y - 100.0 * U).abs() < 1e-3, "{:?}", boom.1);
    detonated(&mut sim, weapons::RPG, Vec3::new(0.0, 0.5 * U, 0.0));
    let sounds = heard(&mut sim, 1);
    let boom = sounds.iter().find(|(e, _)| e == projectile_fx::EXPLOSION_SOUND).unwrap();
    assert!((boom.1.unwrap().y - 24.0 * U).abs() < 1e-3, "{:?}", boom.1);
}

/// A shown projectile of `key` at `at`, moved by hand.
fn shown(sim: &mut Sim, key: &str, at: Vec3) -> Entity {
    sim.app.world_mut().spawn((ShownItem(key.into()), Transform::from_translation(at), GlobalTransform::from_translation(at))).id()
}

fn put(sim: &mut Sim, e: Entity, at: Vec3) {
    let mut ent = sim.app.world_mut().entity_mut(e);
    ent.insert((Transform::from_translation(at), GlobalTransform::from_translation(at)));
}

/// Test 45: a missile moving 25 units a frame: 8 flare sprites at 3, 6,
/// ... 24 behind, 6 puffs of smoke.
#[test]
fn a_missile_leaves_flares_and_smoke_45() {
    let mut sim = sim();
    sim.seconds(0.5);
    let m = shown(&mut sim, MISSILE_MODEL, Vec3::new(0.0, 5.0, 0.0));
    sim.ticks(2);
    let mut x = 0.0;
    for _ in 0..2 {
        x += 25.0 * U;
        put(&mut sim, m, Vec3::new(x, 5.0, 0.0));
        clear(&mut sim);
        sim.ticks(1);
    }
    let flares: usize = (1..=4).map(|i| drawn(&sim, &format!("effects/muzzleflash{i}")).len()).sum();
    assert_eq!(flares, 8);
    let puffs = drawn(&sim, "particle/particle_smokegrenade").len() + drawn(&sim, "particle/particle_noisesphere").len();
    assert!((6..=7).contains(&puffs), "{puffs}");
    // The flares trail from the missile: the nearest 3 units behind it.
    let behind = (1..=4)
        .flat_map(|i| drawn(&sim, &format!("effects/muzzleflash{i}")))
        .map(|(p, ..)| (p.x - x).abs() / U)
        .fold(f32::MAX, f32::min);
    assert!(behind > 2.9 && behind < 3.1, "{behind}");
}

/// The ball's body quad, flicker and blur while it moves; its bounce
/// (`cball_bounce`) and its rings.
#[test]
fn the_ball_glows_bounces_and_ends_in_rings() {
    let mut sim = sim();
    sim.seconds(0.5);
    let b = shown(&mut sim, BALL_MODEL, Vec3::new(0.0, 1.0, 0.0));
    sim.ticks(2);
    let mut x = 0.0;
    for _ in 0..2 {
        x += 16.0 * U;
        put(&mut sim, b, Vec3::new(x, 1.0, 0.0));
        clear(&mut sim);
        sim.ticks(1);
    }
    let body = drawn(&sim, "effects/ar2_altfire1");
    assert_eq!(body.len(), 1);
    assert!(body[0].1 / U >= 9.0 && body[0].1 / U <= 11.0, "half 10 + sin: {}", body[0].1 / U);
    assert_eq!(drawn(&sim, "effects/combinemuzzle1").len(), 1, "the flicker");
    assert_eq!(drawn(&sim, "effects/ar2_altfire1b").len(), 8, "the blur");
    // A bounce on the world: the two flat quads.
    clear(&mut sim);
    let flyer = {
        let clock = sim.app.world().resource::<SimClock>().clone();
        sim.app.world_mut().spawn(Flyer::new(weapons::AR2, None, Vec3::X, &clock)).id()
    };
    let hit = FlyerHit {
        flyer,
        weapon: weapons::AR2,
        owner: None,
        other: None,
        point: Vec3::new(5.0, 0.0, 0.0),
        normal: Vec3::Y,
        velocity: Vec3::X,
        character: false,
        world: true,
        gone: false,
    };
    sim.app.world_mut().write_message(hit);
    sim.ticks(1);
    let big = drawn(&sim, "effects/combinemuzzle1_nocull");
    assert_eq!(big.len(), 1);
    assert!((big[0].1 / U - 160.0).abs() < 1.0, "{}", big[0].1 / U);
    assert_eq!(drawn(&sim, "effects/combinemuzzle2_nocull").len(), 1);
    // Its end: two rings of 48 segments.
    sim.app.world_mut().write_message(FlyerGone { flyer, weapon: weapons::AR2, owner: None, at: Vec3::new(5.0, 1.0, 0.0), expired: true });
    sim.ticks(1);
    clear(&mut sim);
    sim.ticks(1);
    assert_eq!(drawn(&sim, "sprites/lgtning").len(), 96);
    sim.seconds(0.6);
    clear(&mut sim);
    sim.ticks(1);
    assert!(drawn(&sim, "sprites/lgtning").is_empty(), "the rings are over after 0.5 s");
}

/// The bolt's blur: 20 halos of half-size 3, the first 24 units ahead.
#[test]
fn a_bolt_blurs_and_a_stuck_one_glows() {
    let mut sim = sim();
    sim.seconds(0.5);
    let b = shown(&mut sim, BOLT_MODEL, Vec3::new(0.0, 1.0, 0.0));
    sim.ticks(2);
    let mut x = 0.0;
    for _ in 0..2 {
        x += 40.0 * U;
        put(&mut sim, b, Vec3::new(x, 1.0, 0.0));
        clear(&mut sim);
        sim.ticks(1);
    }
    let halos = drawn(&sim, "effects/muzzleflash1");
    assert_eq!(halos.len(), 20);
    assert!(halos.iter().all(|h| (h.1 / U - 3.0).abs() < 0.01));
    let ahead = halos.iter().map(|h| (h.0.x - x) / U).fold(f32::MIN, f32::max);
    assert!((ahead - 24.0).abs() < 0.5, "{ahead}");
    let greys: Vec<f32> = halos.iter().map(|h| h.2).collect();
    assert!((greys[0] - 0.25).abs() < 1e-4 && (greys[19] - 0.05).abs() < 1e-4);
    // Stuck in the world: the glow, alpha 128 of 255.
    let clock = sim.app.world().resource::<SimClock>().clone();
    let mut f = Flyer::new(weapons::CROSSBOW, None, Vec3::X, &clock);
    f.stuck_until = Some(clock.now + 30.0);
    let flyer = sim.app.world_mut().spawn(f).id();
    clear(&mut sim);
    sim.app.world_mut().write_message(FlyerHit {
        flyer,
        weapon: weapons::CROSSBOW,
        owner: None,
        other: None,
        point: Vec3::new(10.0, 1.0, 0.0),
        normal: Vec3::NEG_X,
        velocity: Vec3::X,
        character: false,
        world: true,
        gone: true,
    });
    sim.ticks(1);
    let glow = drawn(&sim, "sprites/light_glow02_noz");
    assert_eq!(glow.len(), 1);
    assert!((glow[0].3 - 0.5).abs() < 0.02, "{}", glow[0].3);
}

/// The contact grenade's smoke trail: 48 puffs a second.
#[test]
fn the_contact_grenade_trails_smoke() {
    let mut sim = sim();
    sim.seconds(0.5);
    let g = shown(&mut sim, SMG1_GRENADE_MODEL, Vec3::new(0.0, 2.0, 0.0));
    clear(&mut sim);
    let mut total = 0;
    for i in 0..30 {
        put(&mut sim, g, Vec3::new(i as f32 * 0.3, 2.0, 0.0));
        sim.ticks(1);
    }
    for g in &sim.app.world().resource::<Particles>().groups {
        total += g.particles.len();
    }
    // 30 frames of 0.015 s: 21.6 puffs from the first frame on, all alive
    // (life 1 s).
    assert!((18..=24).contains(&total), "{total}");
    // Particles_and_smoke.md 5: along the path, through a sine, near fade,
    // both smoke materials.
    let groups = &sim.app.world().resource::<Particles>().groups;
    let smoke: Vec<_> = groups.iter().filter(|g| g.motion.near_fade.is_some()).flat_map(|g| &g.particles).collect();
    assert_eq!(smoke.len(), total);
    let xs: Vec<f32> = smoke.iter().map(|p| p.position.x).collect();
    let span = xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min);
    assert!(span > 5.0, "spread along the 8.7 m flown: {span}");
    assert!(smoke.iter().all(|p| matches!(p.fade, Fade::Sine)));
    let (a, b) = (material(&sim, "particle/particle_smokegrenade"), material(&sim, "particle/particle_noisesphere"));
    assert!(smoke.iter().any(|p| p.material == a) && smoke.iter().any(|p| p.material == b));
}

/// The frag's red glow while it flies.
#[test]
fn a_frag_glows_red() {
    let mut sim = sim();
    sim.seconds(0.5);
    let _g = shown(&mut sim, weapons::FRAG, Vec3::new(0.0, 2.0, 0.0));
    sim.ticks(2);
    clear(&mut sim);
    sim.ticks(1);
    assert_eq!(drawn(&sim, "sprites/redglow1").len(), 1);
}

/// Test 47: the owner's laser dot at the end of a trace from the view,
/// 4 units off the wall, full width 12 to 20.
#[test]
fn the_laser_dot_follows_the_view_47() {
    let mut sim = sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    let w = give(sim.app.world_mut(), p, weapons::RPG).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(weapons::RPG));
    sim.app.world_mut().get_mut::<Hl2Rounds>(w).unwrap().count = 3;
    assert!(sim.app.world().get::<Weapon>(w).is_some());
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    let wall = Vec3::new(0.0, 1.5, greybox::NORTH_WALL_Z);
    testkit::aim_at(&mut sim, p, wall);
    clear(&mut sim);
    sim.ticks(1);
    let dots = drawn(&sim, "sprites/redglow1");
    assert_eq!(dots.len(), 1, "{dots:?}");
    let (at, half, ..) = dots[0];
        // The first thing on the view's line (a crate at z -19.5 in the
    // greybox): 4 units off its face.
    assert!((at.z + 19.5 - 4.0 * U).abs() < 0.01, "{at}");
    assert!(half / U >= 6.0 && half / U <= 10.0, "{}", half / U);
    // Not the owner's RPG in hand: no dot.
    let pistol = give(sim.app.world_mut(), p, weapons::PISTOL).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(pistol);
    sim.seconds(1.5);
    clear(&mut sim);
    sim.ticks(1);
    assert!(drawn(&sim, "sprites/redglow1").is_empty());
}

/// The decals placed over `ticks` ticks.
fn scorches(sim: &mut Sim, ticks: u64) -> Vec<chimera_world::map::PlaceDecal> {
    use chimera_world::map::PlaceDecal;
    let mut cursor = sim.app.world().resource::<Messages<PlaceDecal>>().get_cursor_current();
    let mut out = Vec::new();
    for _ in 0..ticks {
        sim.ticks(1);
        out.extend(cursor.read(sim.app.world().resource::<Messages<PlaceDecal>>()).cloned());
    }
    out
}

/// Tests 41 and 42's scorch: a missile on the floor leaves a `Scorch` on
/// it; one well above the floor (the line from 8 above to 32 below misses)
/// leaves none; a frag's own probe isn't this rule's.
#[test]
fn the_rocket_scorches_the_floor_but_not_a_wall_41_42() {
    use chimera_world::map::DecalGroup;
    let mut sim = sim();
    sim.seconds(0.5);
    detonated(&mut sim, weapons::RPG, Vec3::new(0.0, 0.5 * U, 0.0));
    let marks = scorches(&mut sim, 1);
    assert_eq!(marks.len(), 1);
    assert!(matches!(&marks[0].group, DecalGroup::Named(g) if g == "Scorch"));
    assert!(marks[0].point.y.abs() < 1e-3 && marks[0].normal.y > 0.99, "{:?}", marks[0]);
    detonated(&mut sim, weapons::RPG, Vec3::new(0.0, 100.0 * U, 0.0));
    assert!(scorches(&mut sim, 1).is_empty(), "no scorch in the air");
    detonated(&mut sim, weapons::SLAM, Vec3::new(0.0, 16.0 * U, 0.0));
    assert_eq!(scorches(&mut sim, 1).len(), 1, "a satchel's on the floor below");
}

/// Spec 8 (game_entities.md 6 step 3): each flag leaves out its part: "no
/// sound" the boom, "no fireball" the flash and the core, "no fireball
/// smoke" the puffs, "no particles" the spark streaks and flecks.
#[test]
fn a_blasts_flags_leave_parts_out() {
    const FIRE: &str = "effects/fire_cloud2";
    const NOISE: &str = "particle/particle_noisesphere";
    const GLOW: &str = "sprites/light_glow02_add_noz";
    let at = Vec3::new(0.0, 100.0 * U, 0.0);
    // What a blast with `flags` draws (fire, puffs, glows) and how many booms.
    let run = |flags: u16| {
        let mut sim = sim();
        sim.seconds(0.5);
        clear(&mut sim);
        flagged(&mut sim, flags, at);
        let booms = heard(&mut sim, 1).iter().filter(|(e, _)| e == projectile_fx::EXPLOSION_SOUND).count();
        (drawn(&sim, FIRE).len(), drawn(&sim, NOISE).len(), drawn(&sim, GLOW).len(), booms)
    };
    let (fire, puffs, glows, booms) = run(0);
    assert!(fire >= 32 + 8 && puffs == 4 + 8 + 32 && glows == 1 && booms == 1, "{fire} {puffs} {glows} {booms}");
    let fire_all = fire;
    let (fire, puffs, glows, booms) = run(BlastFlags::NO_SOUND);
    assert!(fire >= 32 + 8 && puffs == 44 && glows == 1 && booms == 0, "{fire} {puffs} {glows} {booms}");
    let (fire, puffs, glows, booms) = run(BlastFlags::NO_FIREBALL);
    assert!((8..=16).contains(&fire) && puffs == 0 && glows == 0 && booms == 1, "sparks left: {fire} {puffs} {glows} {booms}");
    let (fire, puffs, glows, _) = run(BlastFlags::NO_FIREBALL_SMOKE);
    assert!(fire >= 32 + 8 && puffs == 0 && glows == 1, "{fire} {puffs} {glows}");
    let (fire, puffs, glows, _) = run(BlastFlags::NO_PARTICLES);
    assert!(fire == 32 && fire < fire_all && puffs == 44 && glows == 1, "{fire} {puffs} {glows}");
}

/// A map blast's own flags reach what draws it: an `env_explosion`'s "no
/// decal" leaves no scorch where another leaves one (dm_steamlab's 5), and an
/// `explode_fire` prop's blast (silent) has no boom.
#[test]
fn a_map_blast_keeps_its_flags() {
    let blast = |flags: u16| Explosion {
        origin: Vec3::new(0.0, 16.0 * U, 0.0),
        damage: 0.0,
        radius: 100.0 * U,
        attacker: None,
        inflictor: None,
        sound: None,
        weapon: None,
        round_scorch: false,
        through_walls: false,
        bell_sigma: None,
        armor_ratio: None,
        flags: BlastFlags(flags),
    };
    let mut sim = sim();
    sim.seconds(0.5);
    sim.app.world_mut().write_message(blast(0));
    assert_eq!(scorches(&mut sim, 2).len(), 1, "a plain one scorches");
    sim.app.world_mut().write_message(blast(BlastFlags::NO_DECAL));
    assert!(scorches(&mut sim, 2).is_empty(), "no decal");
    sim.app.world_mut().write_message(blast(0));
    let booms = |s: &[(String, Option<Vec3>)]| s.iter().filter(|(e, _)| e == projectile_fx::EXPLOSION_SOUND).count();
    assert_eq!(booms(&heard(&mut sim, 2)), 1, "a plain one booms");
    sim.app.world_mut().write_message(blast(BlastFlags::NO_SOUND));
    assert_eq!(booms(&heard(&mut sim, 2)), 0, "silent");
}

/// Test 40: a shot satchel's pop draws the flash overlay and the debris
/// (sparks, flecks) but no fireballs or smoke puffs, and plays the
/// explosion sound where it is.
#[test]
fn a_satchel_pop_is_the_flash_and_debris_40() {
    use chimera_world::map::particles::Shape;
    let mut sim = sim();
    sim.seconds(0.5);
    clear(&mut sim);
    let at = Vec3::new(0.0, 1.0 * U, 0.0);
    sim.app.world_mut().write_message(hl2dm::game::weapons::slam::SatchelPop { at });
    let sounds = heard(&mut sim, 1);
    assert_eq!(sounds.iter().filter(|(e, _)| e == projectile_fx::EXPLOSION_SOUND).count(), 1);
    assert!(drawn(&sim, "particle/particle_noisesphere").is_empty(), "no smoke");
    assert!(!drawn(&sim, "sprites/light_glow02_add_noz").is_empty(), "the flash");
    let fire = material(&sim, "effects/fire_cloud2");
    let ps = sim.app.world().resource::<Particles>();
    let fire_parts: Vec<_> = ps.groups.iter().flat_map(|g| g.particles.iter()).filter(|p| p.material == fire).collect();
    assert!(!fire_parts.is_empty(), "spark streaks");
    assert!(fire_parts.iter().all(|p| matches!(p.shape, Shape::Trail { .. })), "no fireballs");
}

/// Effects 9: a ball held in the gravity gun draws no body quad; its
/// flicker and blur stay.
#[test]
fn a_held_ball_has_no_body_quad() {
    use hl2dm::game::projectile_fx::Hl2BallHeld;
    let mut sim = sim();
    sim.seconds(0.5);
    let b = shown(&mut sim, BALL_MODEL, Vec3::new(0.0, 1.0, 0.0));
    sim.app.world_mut().entity_mut(b).insert(Hl2BallHeld);
    sim.ticks(2);
    let mut x = 0.0;
    for _ in 0..2 {
        x += 16.0 * U;
        put(&mut sim, b, Vec3::new(x, 1.0, 0.0));
        clear(&mut sim);
        sim.ticks(1);
    }
    assert!(drawn(&sim, "effects/ar2_altfire1").is_empty(), "no body quad");
    assert_eq!(drawn(&sim, "effects/combinemuzzle1").len(), 1, "the flicker");
    assert_eq!(drawn(&sim, "effects/ar2_altfire1b").len(), 8, "the blur");
}

/// Projectiles.md 7.4: a bolt that hits a body sends a "BoltImpact" where
/// a line 128 units on along its flight meets the world (a wall 40 units
/// behind the body), and none when the world is farther; one that sticks
/// in the world sends its own at the hit.
#[test]
fn a_bolt_through_a_body_sends_its_impact_from_the_wall_behind() {
    use hl2dm::game::projectile_fx::FlyerFx;
    let mut sim = sim();
    sim.seconds(0.5);
    let mut cursor = sim.app.world().resource::<Messages<FlyerFx>>().get_cursor_current();
    let clock = sim.app.world().resource::<SimClock>().clone();
    let flyer = sim.app.world_mut().spawn(Flyer::new(weapons::CROSSBOW, None, Vec3::NEG_Z, &clock)).id();
    let wall = greybox::NORTH_WALL_Z;
    let mut impacts = |sim: &mut Sim, z: f32| {
        sim.app.world_mut().write_message(FlyerHit {
            flyer,
            weapon: weapons::CROSSBOW,
            owner: None,
            other: None,
            point: Vec3::new(0.0, 1.5, z),
            normal: Vec3::Z,
            velocity: Vec3::new(0.0, 0.0, wall.signum()) * 80.0,
            character: true,
            world: false,
            gone: true,
        });
        sim.ticks(1);
        cursor
            .read(sim.app.world().resource::<Messages<FlyerFx>>())
            .filter_map(|f| match f {
                FlyerFx::BoltImpact { point, .. } => Some(*point),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let near = impacts(&mut sim, wall - wall.signum() * 40.0 * U);
    assert_eq!(near.len(), 1, "the wall 40 units on");
    assert!((near[0].z - wall).abs() < 0.05, "{near:?}");
    assert!(impacts(&mut sim, wall - wall.signum() * 200.0 * U).is_empty(), "nothing within 128");
}

/// The ball's rings draw `sprites/lgtning`, whose file (Sprite, no
/// `$additive`) would blend its black texels over the scene: beams add
/// (visual_entities.md 1). The playtest saw black stripes.
#[test]
fn the_rings_material_adds() {
    assert!(hl2dm::game::effects::ADDITIVE_SPRITES.contains(&"sprites/lgtning"));
}
