//! HL2:DM's weapon details on the headless harness (`harness::Hl2Sim`,
//! the greybox; specs/hl2dm/weapons.md and projectiles.md): a call's odd
//! bullets traced as ±3 boxes (weapons.md 5), the melee box hit refined
//! by a line to twice its distance (11), the dead's weapon thrown by the
//! killing hit (9), `cl_defaultweapon` (3), switches refused to a weapon
//! with no ammo (1: `lastinv`, `phys_swap`), the .357's view nudge (7.2),
//! the shove read before the armour (projectiles.md 3), the contact
//! grenade's 3-tick check (4.2) and a TDM leaver's satchels (8.5).

use avian3d::prelude::{AngularVelocity, Collider, LinearVelocity, RigidBody};
use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup, Intent, LocalPlayer, MovementState, UserInfo, Velocity};
use chimera_sim::{
    testkit,
    weapon::{
        Armor, Inventory, Magazine, Unselectable, Weapon, WeaponEvent, WeaponEventKind, WeaponState, drop::Loose, give,
        projectile::{Flyer, FlyerGone, LaunchRequest, Touch},
    },
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use hl2dm::{
    game::{
        movement,
        weapons::{self, launch::Hl2Rounds},
    },
    harness::Hl2Sim,
};

const U: f32 = 0.0254;

fn sim() -> Sim {
    let mut sim = Sim::new(Hl2Sim, GreyboxMapPlugin);
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    sim
}

fn weapon_of(sim: &Sim, p: Entity, id: &str) -> Option<Entity> {
    let w = sim.app.world();
    w.get::<Inventory>(p)?.weapons.iter().copied().find(|e| w.get::<Weapon>(*e).is_some_and(|x| x.id == id))
}

/// A player at the first spawn holding `id`, drawn, looking along -Z.
fn holding(sim: &mut Sim, id: &str) -> (Entity, Entity) {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    let w = match weapon_of(sim, p, id) {
        Some(w) => {
            sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
            w
        }
        None => give(sim.app.world_mut(), p, id).unwrap(),
    };
    sim.seconds(1.5);
    let mut i = sim.intent(p);
    i.yaw = 0.0;
    i.pitch = 0.0;
    (p, w)
}

/// A static block of `size` at `at` (m), in place after a tick.
fn block(sim: &mut Sim, at: Vec3, size: Vec3) -> Entity {
    let e = sim
        .app
        .world_mut()
        .spawn((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z), Transform::from_translation(at)))
        .id();
    sim.ticks(1);
    e
}

/// `n` ticks, `input` before each; `p`'s weapon events by tick.
fn run(sim: &mut Sim, p: Entity, n: u64, mut input: impl FnMut(&mut Sim, u64)) -> Vec<(u64, WeaponEventKind)> {
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut out = Vec::new();
    for t in 0..n {
        input(sim, t);
        sim.ticks(1);
        for e in cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()) {
            if e.owner == p && e.shown() {
                out.push((t, e.kind.clone()));
            }
        }
    }
    out
}

/// How far each `Shot` of `events` went along -Z (onto a wall facing it).
fn depths(events: &[(u64, WeaponEventKind)]) -> Vec<(u64, f32)> {
    events
        .iter()
        .filter_map(|(t, k)| match k {
            WeaponEventKind::Shot { from, to, hit: Some(_), .. } => Some((*t, from.z - to.z)),
            _ => None,
        })
        .collect()
}

/// Weapons.md 5: a shotgun shot's pellets 1, 3 and 5 are ±3 boxes: on a
/// wall square to the shot they stop 3 units short of the lines.
#[test]
fn a_shotguns_odd_pellets_are_boxes() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, weapons::SHOTGUN);
    let wall = sim.position(p).z - 3.0;
    block(&mut sim, Vec3::new(0.0, 1.5, wall), Vec3::new(4.0, 4.0, 0.2));
    let ev = run(&mut sim, p, 2, |s, t| s.intent(p).fire = t == 0);
    let d = depths(&ev);
    assert_eq!(d.len(), 7, "{d:?}");
    let line = d[0].1;
    for (i, (_, depth)) in d.iter().enumerate() {
        let want = if i % 2 == 1 { line - 3.0 * U } else { line };
        assert!((depth - want).abs() < 2e-3, "pellet {i}: {depth} (line {line})");
    }
}

/// Weapons.md 5 and its edge case: two SMG1 bullets due in one tick are
/// one call, the second a box; a lone bullet is a line.
#[test]
fn a_machine_guns_second_catch_up_bullet_is_a_box() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::SMG1);
    let wall = sim.position(p).z - 3.0;
    block(&mut sim, Vec3::new(0.0, 1.5, wall), Vec3::new(4.0, 4.0, 0.2));
    let ev = run(&mut sim, p, 2, |s, t| {
        s.intent(p).fire = true;
        if t == 1 {
            // A hitch: two bullets due.
            s.app.world_mut().get_mut::<WeaponState>(w).unwrap().next_primary -= 0.15;
        }
    });
    let d = depths(&ev);
    assert_eq!(d.iter().filter(|(t, _)| *t == 0).count(), 1, "{d:?}");
    let pair: Vec<f32> = d.iter().filter(|(t, _)| *t == 1).map(|(_, x)| *x).collect();
    assert_eq!(pair.len(), 2, "{d:?}");
    assert!((d[0].1 - pair[0]).abs() < 2e-3, "the first is a line: {d:?}");
    assert!((pair[0] - 3.0 * U - pair[1]).abs() < 2e-3, "the second a box: {d:?}");
}

/// Weapons.md 11 step 2: a box hit on a target within 45 degrees is
/// re-traced as a line to twice its distance; a pillar on that line past
/// the 75-unit line's reach takes the swing instead of the target.
#[test]
fn a_melee_box_hit_is_refined_by_a_line_to_twice_its_distance() {
    for pillar in [false, true] {
        let mut sim = sim();
        let (p, _) = holding(&mut sim, weapons::CROWBAR);
        let at = sim.position(p);
        let t = sim.spawn_character(at + Vec3::new(0.6, 0.0, -1.9), movement::ID);
        sim.seconds(1.0);
        sim.app.world_mut().entity_mut(t).insert(chimera_core::core::MapControls {
            time_scale: 0.0,
            ..Default::default()
        });
        sim.app.world_mut().get_mut::<Transform>(t).unwrap().translation = at + Vec3::new(0.6, 0.0, -1.9);
        let post = pillar.then(|| block(&mut sim, at + Vec3::new(0.0, 0.5, -2.05), Vec3::new(0.2, 3.0, 0.1)));
        let mut i = sim.intent(p);
        i.yaw = 0.0;
        i.pitch = 0.0;
        let ev = run(&mut sim, p, 2, |s, k| s.intent(p).fire = k == 0);
        let swing = ev.iter().find_map(|(_, k)| match k {
            WeaponEventKind::Swing { hit, at, .. } => Some((*hit, at.map(|a| a.2))),
            _ => None,
        });
        let health = sim.app.world().get::<Health>(t).unwrap().current * 100.0;
        match post {
            Some(post) => {
                assert_eq!(swing, Some((true, Some(post))), "the refined line takes the pillar");
                assert_eq!(health, 100.0, "the target untouched");
            }
            None => {
                assert_eq!(swing.map(|s| s.0), Some(true), "{swing:?}");
                assert!((health - 75.0).abs() < 1e-3, "the box's (or a corner's) hit: {health}");
            }
        }
    }
}

/// Weapons.md 9: the dead's weapon leaves 12 units under the eye at the
/// killing hit's force over 85 kg (a .357's 128 in/s), at most 400 in/s,
/// with a (200, 200, 200) spin.
#[test]
fn the_dead_throw_their_weapon_by_the_killing_hit() {
    for (impulse, speed) in [(10882.81, 10882.81 / 85.0), (100000.0, 400.0)] {
        let mut sim = sim();
        let (p, pistol) = holding(&mut sim, weapons::PISTOL);
        let eye = sim.position(p) + sim.app.world().get::<MovementState>(p).unwrap().eye_offset;
        sim.app.world_mut().write_message(Damage {
            target: p,
            attacker: None,
            amount: 2.0,
            point: eye,
            dir: Vec3::X,
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Bullet,
            weapon: None,
            force: Vec3::X * impulse * U,
        });
        let mut seen = None;
        for _ in 0..3 {
            sim.ticks(1);
            let w = sim.app.world_mut();
            let mut q = w.query::<(&Loose, &Transform, &LinearVelocity, &AngularVelocity)>();
            if let Some((_, t, v, a)) = q.iter(w).find(|(l, ..)| l.weapon == pistol) {
                seen = Some((t.translation, v.0, a.0));
                break;
            }
        }
        let (at, v, spin) = seen.expect("the pistol dropped");
        assert!((v.length() / U - speed).abs() < 1.0 && v.normalize().x > 0.99, "{} in/s: {v}", v.length() / U);
        assert!((spin - Vec3::splat(200f32.to_radians())).length() < 1e-4, "{spin}");
        assert!((at.y - (eye.y - 12.0 * U)).abs() < 0.05, "{at} under the eye {eye}");
    }
}

/// Weapons.md 3, test 3: `cl_defaultweapon weapon_smg1` spawns with the
/// SMG1 in hand (drawn at the spawn), the pistol the last weapon; its first
/// shot 56 ticks on. Unset: the gravity gun, the SMG1 the last.
#[test]
fn cl_defaultweapon_names_the_weapon_in_hand_3() {
    let mut sim = sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    let mut info = UserInfo::default();
    info.0.insert("cl_defaultweapon".into(), "weapon_smg1".into());
    sim.app.world_mut().entity_mut(p).insert(info);
    let mut deployed = None;
    let mut first = None;
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    for t in 0..120u64 {
        sim.intent(p).fire = true;
        sim.ticks(1);
        for e in cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()) {
            let smg = sim.app.world().get::<Weapon>(e.weapon).is_some_and(|w| w.id == weapons::SMG1);
            match e.kind {
                WeaponEventKind::Deployed if smg && e.owner == p => deployed = deployed.or(Some(t)),
                WeaponEventKind::Fired { .. } if e.owner == p => first = first.or(Some((t, smg))),
                _ => {}
            }
        }
    }
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(weapons::SMG1));
    let w = sim.app.world();
    let last = w.get::<Inventory>(p).unwrap().last.and_then(|l| w.get::<Weapon>(l)).map(|w| w.id);
    assert_eq!(last, Some(weapons::PISTOL));
    let (d, (f, smg)) = (deployed.expect("SMG1 drawn"), first.expect("fired"));
    assert!(smg && f - d == 56, "drawn on {d}, first shot on {f}");
    // The default: the gravity gun, the SMG1 last.
    let mut sim = self::sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    let w = sim.app.world();
    assert_eq!(testkit::active_id(w, p), Some(weapons::PHYSCANNON));
    let last = w.get::<Inventory>(p).unwrap().last.and_then(|l| w.get::<Weapon>(l)).map(|w| w.id);
    assert_eq!(last, Some(weapons::SMG1));
}

/// Weapons.md 1: `lastinv` and `phys_swap` are refused to a weapon with
/// none of its ammo (clip, reserve, grenades), and go once it has some.
#[test]
fn lastinv_and_phys_swap_refuse_a_weapon_with_no_ammo() {
    let mut sim = sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    let smg = weapon_of(&sim, p, weapons::SMG1).unwrap();
    let empty = |sim: &mut Sim, clip: u32| {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(smg).unwrap();
        m.clip = clip;
        m.reserve = 0;
        sim.app.world_mut().get_mut::<Hl2Rounds>(smg).unwrap().count = 0;
    };
    empty(&mut sim, 0);
    let press = |sim: &mut Sim| {
        sim.intent(p).last_weapon = true;
        sim.ticks(2);
        sim.intent(p).last_weapon = false;
        sim.ticks(2);
    };
    press(&mut sim);
    assert!(sim.app.world().get::<Unselectable>(smg).is_some());
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(weapons::PHYSCANNON), "lastinv refused");
    weapons::physcannon::phys_swap(sim.app.world_mut(), p);
    sim.ticks(2);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(weapons::PHYSCANNON), "phys_swap refused");
    // A grenade alone makes it selectable.
    sim.app.world_mut().get_mut::<Hl2Rounds>(smg).unwrap().count = 1;
    sim.ticks(1);
    press(&mut sim);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(weapons::SMG1), "lastinv with a grenade");
    assert!(sim.app.world().get::<Unselectable>(smg).is_none());
}

/// Weapons.md 7.2: each .357 shot turns the shooter's own view (its
/// machine's local player) by a whole -1, 0 or +1 degree in pitch and yaw;
/// another player's view stays.
#[test]
fn the_357_nudges_its_shooters_own_view() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::MAGNUM);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    let mut turns = Vec::new();
    for _ in 0..8 {
        let before = sim.app.world().get::<Intent>(p).unwrap().clone();
        sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 6;
        run(&mut sim, p, 2, |s, k| s.intent(p).fire = k == 0);
        let after = sim.app.world().get::<Intent>(p).unwrap().clone();
        let yaw = (after.yaw - before.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let turn = Vec2::new(-(after.pitch - before.pitch), yaw).map(f32::to_degrees);
        turns.push(turn);
        run(&mut sim, p, 60, |s, _| s.intent(p).fire = false);
    }
    for t in &turns {
        assert!(
            [t.x, t.y].iter().all(|a| (a - a.round()).abs() < 1e-3 && a.round().abs() <= 1.0),
            "whole degrees of -1..1: {turns:?}"
        );
    }
    assert!(turns.iter().any(|t| t.length() > 0.5), "{turns:?}");
    // Not the local player: no nudge.
    let mut sim = self::sim();
    let (p, _) = holding(&mut sim, weapons::MAGNUM);
    let before = sim.app.world().get::<Intent>(p).unwrap().clone();
    for _ in 0..4 {
        run(&mut sim, p, 60, |s, k| s.intent(p).fire = k == 0);
    }
    let after = sim.app.world().get::<Intent>(p).unwrap().clone();
    assert_eq!((before.yaw, before.pitch), (after.yaw, after.pitch));
}

/// Projectiles.md 3: the shove reads the damage arriving at the suit,
/// before the armour's share: a pistol body hit on 100 armour shoves 40
/// units/s as on none (health takes only 20 % of it).
#[test]
fn the_shove_reads_the_damage_before_the_armour() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons::PISTOL);
    let t = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 1.6, movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Armor>(t).unwrap().amount = 1.0;
    let body = sim.position(t) + Vec3::Y * 0.2;
    testkit::aim_at(&mut sim, p, body);
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 1;
    let mut speeds = Vec::new();
    run(&mut sim, p, 3, |s, k| {
        s.intent(p).fire = k == 0;
        speeds.push(s.app.world().get::<Velocity>(t).unwrap().0.xz().length() / U);
    });
    speeds.push(sim.app.world().get::<Velocity>(t).unwrap().0.xz().length() / U);
    let health = sim.app.world().get::<Health>(t).unwrap().current * 100.0;
    assert!(health > 97.0 && health < 100.0, "the suit took most: {health}");
    assert!((speeds[1] - 40.0).abs() < 2.0, "{speeds:?}");
}

/// Projectiles.md 4.2: every 3 ticks of flight a contact grenade whose
/// speed is exactly 0 explodes.
#[test]
fn a_stopped_contact_grenade_explodes_on_its_third_tick() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, weapons::SMG1);
    let clock = *sim.app.world().resource::<chimera_core::core::SimClock>();
    let mut f = Flyer::new(weapons::SMG1, Some(p), Vec3::ZERO, &clock);
    f.half = Vec3::splat(3.0 * U);
    f.touch = Touch::Explode;
    f.blast = Some(weapons::launch::blast(100.0, 250.0, None, None));
    let start = sim.position(p) + Vec3::new(0.0, 2.0, -3.0);
    sim.app.world_mut().write_message(LaunchRequest {
        flyer: f,
        start,
        shown: None,
        tag: None,
    });
    let mut cursor = sim.app.world().resource::<Messages<FlyerGone>>().get_cursor_current();
    let mut gone = None;
    for t in 0..10u64 {
        sim.ticks(1);
        if cursor.read(sim.app.world().resource::<Messages<FlyerGone>>()).any(|g| g.weapon == weapons::SMG1 && !g.expired) {
            gone = gone.or(Some(t));
        }
    }
    // Made on tick 0, moves from tick 1: its third move on tick 3.
    assert_eq!(gone, Some(3));
}

/// Projectiles.md 8.5: a TDM player leaving alive is killed on the way,
/// so its satchels go off 0.2 s on; a DM leaver's stay live.
#[test]
fn a_tdm_leavers_satchels_go_off_and_a_dm_leavers_stay() {
    use hl2dm::game::{
        rules::{self, Hl2Player, Hl2Rules},
        weapons::slam::{Hl2Satchel, SlamAction, SlamParams},
    };
    for teamplay in [true, false] {
        let mut sim = sim();
        sim.ticks(1);
        sim.app.world_mut().resource_mut::<Hl2Rules>().teamplay = teamplay as u8;
        rules::start(sim.app.world_mut());
        sim.ticks(1);
        let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
        sim.seconds(1.0);
        assert!(sim.app.world().get::<Hl2Player>(p).is_some(), "in the game");
        let params: SlamParams = weapons::launch::hook_params(weapons::SLAM, weapons::SLAM_HOOK);
        let at = sim.position(p) + Vec3::new(2.0, 0.0, 0.0);
        sim.app.world_mut().write_message(SlamAction::Satchel {
            thrower: p,
            at,
            velocity: Vec3::ZERO,
            params,
        });
        sim.seconds(1.0);
        let count = |sim: &mut Sim| {
            let w = sim.app.world_mut();
            w.query::<&Hl2Satchel>().iter(w).count()
        };
        assert_eq!(count(&mut sim), 1);
        sim.app.world_mut().despawn(p);
        sim.ticks(20);
        assert_eq!(count(&mut sim), if teamplay { 0 } else { 1 }, "teamplay {teamplay}");
    }
}

/// Effects 10 (projectiles.md 5.3): the energy ball's victim's dropped
/// weapon can't be taken and dissolves as the ragdoll does: white to
/// black over the first second, faded out by 2.0 s, sparks and glows
/// about it, and gone (with its weapon) 134 ticks after the kill. A
/// bullet's victim's stays as any other.
#[test]
fn the_balls_victims_weapon_dissolves() {
    use chimera_world::map::{
        RenderLook,
        loose::LooseItem,
        particles::{MapParticles, ParticleMaterial, ParticleMaterials, Particles},
    };
    use hl2dm::game::dissolve::Hl2DissolvingItem;
    for ball in [true, false] {
        let mut sim = sim();
        let materials = ["effects/spark", "effects/combinemuzzle2"]
            .iter()
            .map(|n| ParticleMaterial { name: n.to_string(), ..default() })
            .collect();
        sim.app.insert_resource(ParticleMaterials(MapParticles { materials }));
        let (p, pistol) = holding(&mut sim, weapons::PISTOL);
        let point = sim.position(p);
        sim.app.world_mut().write_message(Damage {
            target: p,
            attacker: None,
            amount: 1000.0,
            point,
            dir: Vec3::X,
            hitgroup: Hitgroup::Generic,
            kind: if ball { DamageKind::Crush } else { DamageKind::Bullet },
            weapon: ball.then_some(weapons::AR2),
            force: Vec3::ZERO,
        });
        sim.ticks(3);
        let w = sim.app.world_mut();
        let loose: Vec<Entity> = w.query_filtered::<Entity, With<LooseItem>>().iter(w).collect();
        assert_eq!(loose.len(), 1, "the pistol lies there");
        let item = loose[0];
        if !ball {
            assert!(w.get::<Loose>(item).is_some_and(|l| l.weapon == pistol), "anyone can take it");
            assert!(w.get::<Hl2DissolvingItem>(item).is_none());
            continue;
        }
        assert!(w.get::<Loose>(item).is_none(), "nobody can take it");
        assert!(w.get::<Hl2DissolvingItem>(item).is_some());
        sim.app.world_mut().resource_mut::<Particles>().groups.clear();
        sim.seconds(1.2);
        let look = *sim.app.world().get::<RenderLook>(item).expect("its look");
        assert_eq!((look.color, look.alpha), ([0; 3], 255), "black after a second");
        let glows = sim.app.world().resource::<ParticleMaterials>().0.find("effects/combinemuzzle2").unwrap();
        let near = sim
            .app
            .world()
            .resource::<Particles>()
            .groups
            .iter()
            .flat_map(|g| g.particles.iter())
            .filter(|p| p.material == glows)
            .count();
        assert!(near > 0, "glows about it");
        // 3 + 80 ticks so far; at 134 after the kill it and its weapon go.
        sim.ticks(134 - 3 - 80 - 2);
        assert!(sim.app.world().get_entity(item).is_ok(), "still there just before");
        sim.ticks(4);
        assert!(sim.app.world().get_entity(item).is_err(), "gone 134 ticks on");
        assert!(sim.app.world().get_entity(pistol).is_err(), "and its weapon");
    }
}

/// Projectiles.md 5.1: the ball's launch fades the shooter's screen white
/// (255, 255, 255, alpha 64 over 0.1 s).
#[test]
fn the_balls_launch_fades_the_shooters_screen_white() {
    use chimera_world::map::logic_view::ScreenFades;
    let mut sim = sim();
    sim.app.init_resource::<ScreenFades>();
    let (p, ar2) = holding(&mut sim, weapons::AR2);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    sim.app.world_mut().get_mut::<Hl2Rounds>(ar2).unwrap().count = 1;
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut at = None;
    for k in 0..60 {
        sim.intent(p).secondary = k < 2;
        sim.ticks(1);
        let launched = cursor
            .read(sim.app.world().resource::<Messages<WeaponEvent>>())
            .any(|e| matches!(e.kind, WeaponEventKind::Action("ball")));
        if launched {
            at = Some(k);
            break;
        }
        let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
        assert!(sim.app.world_mut().resource_mut::<ScreenFades>().at(now).is_none(), "no fade before the launch");
    }
    assert!(at.is_some(), "the ball left");
    let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
    let (color, alpha, _) = sim.app.world_mut().resource_mut::<ScreenFades>().at(now).expect("a fade");
    assert_eq!(color, [255, 255, 255]);
    assert!(alpha > 0 && alpha <= 64, "{alpha}");
}
