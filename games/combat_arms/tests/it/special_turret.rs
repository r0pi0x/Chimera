//! specs/combat_arms/special_weapons.md's auto turret cases 64-73
//! (Follow-up (2026-11-21): the deploy, models and animations), with
//! `special_weapons`' helpers: the release on the view model's key, the
//! interruptions, the landing, the unfold, the aim and muzzle per shot,
//! the disappear effect, the HG2 variant's data and the shared tracer
//! count. The models themselves need the install (V2-keyed): the drawn
//! parts are checked on stand-in drawn keys (`a_turret_draws_its_parts`).

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health, Hitgroup};
use chimera_sim::weapon::{
    self, Inventory,
    data::WeaponDataAppExt,
    grenade::{Launch, Throwable},
};
use chimera_world::harness::Sim;
use combat_arms::game::{
    effects::{data::CaWeaponFx, sounds::CaSound},
    inventory::KNIFE,
    special::{
        self, CaDeviceData, CaObject, CaTurretShots, devices,
        turret::{self, TurretPhase},
    },
};

use super::{special_devices::in_a_zone, special_weapons::*};

const TURRET: &str = "combat_arms:auto-turret";

/// The HG2 turret's differences (T5) on the stock one, as an install
/// copy's merge writes them.
const HG2: &str = r#"
id = "test:hg2_turret"
base = "combat_arms:auto-turret"
[hooks."combat_arms:device"]
health = 200.0
once_only = false
[hooks."combat_arms:device".turret]
ready_sound = "guns_snd_throwing/autoturret/set_hg2.wav"
perturb = 0.0
[hooks."combat_arms:device".turret.gun]
damage = 7.0
roll = 0.0
"#;

/// The stock turret with no aim error and no roll, and stand-in models
/// (drawn keys only: nothing loads them here).
const MODELLED: &str = r#"
id = "test:turret_modelled"
base = "combat_arms:auto-turret"
[hooks."combat_arms:device"]
model = "guns_m_hh/base.ltb"
[hooks."combat_arms:device".turret]
head_model = "guns_m_hh/head.ltb"
gun_model = "guns_m_hh/gun.ltb"
"#;

fn tsim() -> Sim {
    let mut sim = sim();
    sim.app.add_weapon_data(&[("hg2", HG2), ("modelled", MODELLED)]).unwrap();
    sim.app
        .init_resource::<Sounds>()
        .init_resource::<Launches>()
        .add_systems(FixedPostUpdate, log_sounds)
        .add_observer(log_launch);
    sim
}

#[derive(Resource, Default)]
struct Sounds(Vec<(f64, CaSound)>);

fn log_sounds(mut s: MessageReader<CaSound>, clock: Res<chimera_core::core::SimClock>, mut log: ResMut<Sounds>) {
    for s in s.read() {
        log.0.push((clock.now, s.clone()));
    }
}

#[derive(Resource, Default)]
struct Launches(Vec<f64>);

fn log_launch(_: On<Launch>, clock: Res<chimera_core::core::SimClock>, mut log: ResMut<Launches>) {
    log.0.push(clock.now);
}

/// A player at the origin facing −Z holding the turret, with a knife to
/// fall back to: (him, the turret, the knife).
fn thrower(sim: &mut Sim) -> (Entity, Entity, Entity) {
    let p = player(sim, 0.0, 0.0, 0.0, 1);
    let k = weapon::give(sim.app.world_mut(), p, KNIFE).unwrap();
    let w = armed(sim, p, TURRET);
    (p, w, k)
}

/// Select weapon `w` by its slot's key.
fn select(sim: &mut Sim, p: Entity, w: Entity) {
    let slot = sim.app.world().get::<weapon::Weapon>(w).unwrap().slot;
    sim.intent(p).select = Some(slot);
    sim.ticks(1);
    sim.intent(p).select = None;
}

/// Press fire for one tick; the time of the press.
fn press(sim: &mut Sim, p: Entity) -> f64 {
    let t0 = now(sim);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    t0
}

fn turrets(sim: &mut Sim) -> Vec<(Entity, CaObject)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &CaObject)>().iter(w).map(|(e, o)| (e, o.clone())).collect()
}

fn count(sim: &Sim, w: Entity) -> u32 {
    sim.app.world().get::<Throwable>(w).unwrap().count
}

/// Case 64 (data): our turret's timing and sounds as the spec gives them.
#[test]
fn t64_turret_data() {
    let mut sim = tsim();
    let w = weapon_of(&mut sim, TURRET, None);
    let d = sim.app.world().get::<CaDeviceData>(w).unwrap().params.clone();
    assert_eq!((d.prefire, d.key, d.postfire, d.redraw), (0.0, 3.1, 3.666, 1.333));
    let t = sim.app.world().get::<Throwable>(w).unwrap();
    assert_eq!((t.throw_delay, t.throw_time), (3.1, 3.666));
    let fx = sim.app.world().get::<CaWeaponFx>(w).expect("its sounds");
    assert!(fx.reload_sounds[0].ends_with("autoturret/button.wav"));
    assert!(fx.reload_sounds[1].ends_with("autoturret/unlock.wav"));
    assert!(fx.fire_sound.ends_with("autoturret/throw.wav"));
    assert_eq!(fx.fire_radius, 500.0);
    assert_eq!(fx.impact, "Auto-Turret_disappear");
    let tp = d.turret.as_ref().unwrap();
    assert!(tp.ready_sound.ends_with("autoturret/set.wav"));
    assert_eq!(tp.ready_radius, 1000.0);
    assert_eq!(tp.fire_radius, 2000.0);
}

/// Case 65: the case leaves the hand on the view model's `FIRE_KEY` at
/// 3100 ms into `fire_0` (the frame after the press starts it), never
/// before; the weapon switches away when `fire_0` ends (3666 ms).
#[test]
fn t65_release_on_the_key() {
    let mut sim = tsim();
    let (p, w, _) = thrower(&mut sim);
    let t0 = press(&mut sim, p);
    while now(&sim) < t0 + 3.1 {
        sim.ticks(1);
        assert!(turrets(&mut sim).is_empty(), "nothing before the key: {}", now(&sim) - t0);
    }
    wait(&mut sim, 0.1);
    let launches = sim.app.world().resource::<Launches>().0.clone();
    assert_eq!(launches.len(), 1);
    let dt = launches[0] - t0;
    assert!((3.1..=3.1 + 0.06).contains(&dt), "released {dt} s after the press");
    assert_eq!(turrets(&mut sim).len(), 1, "the case is flying");
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "still in hand");
    while now(&sim) < t0 + 3.666 - 0.02 {
        sim.ticks(1);
    }
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "fire_0 plays to its end");
    wait(&mut sim, 0.2);
    assert_ne!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "switched away after 3666 ms");
}

/// Case 66: a switch before the key keeps the turret (no device, total
/// still 1) and the next throw starts over; a switch after it is too late.
/// Dying before the key: no turret either.
#[test]
fn t66_switch_before_the_release() {
    let mut sim = tsim();
    let (p, w, knife) = thrower(&mut sim);
    press(&mut sim, p);
    wait(&mut sim, 2.5);
    select(&mut sim, p, knife);
    wait(&mut sim, 2.0);
    assert!(turrets(&mut sim).is_empty(), "no turret");
    assert_eq!(count(&sim, w), 1, "the turret is kept");
    select(&mut sim, p, w);
    wait(&mut sim, 2.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "drawn again");
    let t1 = press(&mut sim, p);
    wait(&mut sim, 3.2);
    let launches = sim.app.world().resource::<Launches>().0.clone();
    assert_eq!(launches.len(), 1);
    let dt = launches[0] - t1;
    assert!((3.1..=3.1 + 0.06).contains(&dt), "fire_0 from its start: {dt}");
    select(&mut sim, p, knife);
    wait(&mut sim, 0.5);
    assert_eq!(turrets(&mut sim).len(), 1, "flying");
    assert_eq!(count(&sim, w), 0, "spent");

    // Dying before the key: none.
    let mut sim = tsim();
    let (p, _, _) = thrower(&mut sim);
    press(&mut sim, p);
    wait(&mut sim, 1.0);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 0.0;
    wait(&mut sim, 3.0);
    assert!(turrets(&mut sim).is_empty(), "dead before the key: no turret");
}

/// Case 67: no placement check: thrown at a wall it comes down to the
/// floor below and lands; it lands upright on a 30 degree ramp facing the
/// throw's horizontal direction, not on a 45 degree one.
#[test]
fn t67_landing() {
    // A wall 50 u ahead (+X).
    let mut sim = tsim();
    let owner = far_player(&mut sim, 1);
    solid(
        &mut sim,
        Vec3::new(50.0 * U, 0.0, -2.0),
        Vec3::new(60.0 * U, 3.0, 2.0),
        None,
    );
    let e = device(&mut sim, TURRET, Some(owner), Vec3::new(0.0, 60.0, 0.0), Vec3::new(600.0, 0.0, 0.0));
    wait(&mut sim, 2.0);
    let o = obj(&sim, e);
    assert!(o.s.landed, "landed below the wall");
    assert!(o.pos.x < 50.0 && (o.pos.y - o.half.y).abs() < 1.0, "on the floor: {}", o.pos);
    assert_near(o.s.facing, Vec3::X, 1e-4, "facing the throw");

    for (deg, lands) in [(30.0f32, true), (45.0, false)] {
        let mut sim = tsim();
        let owner = far_player(&mut sim, 1);
        // A ramp turned about Z, its top 100 u above the floor at x = 0.
        sim.app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(400.0 * U, 10.0 * U, 400.0 * U),
            Transform::from_translation(Vec3::new(0.0, 100.0 * U, 0.0))
                .with_rotation(Quat::from_rotation_z(deg.to_radians())),
        ));
        sim.ticks(1);
        let e = device(&mut sim, TURRET, Some(owner), Vec3::new(0.0, 300.0, 0.0), Vec3::new(0.0, 0.0, -100.0));
        wait(&mut sim, 3.0);
        let o = obj(&sim, e);
        let on_ramp = o.s.landed && o.pos.y > 50.0;
        assert_eq!(on_ramp, lands, "{deg} degrees: {:?} at {}", o.s.landed, o.pos);
        if lands {
            assert_near(o.s.facing, Vec3::NEG_Z, 1e-4, "facing the throw's flat");
            let r = sim.app.world().get::<Transform>(e).unwrap().rotation;
            assert!((r * Vec3::Y).y > 0.9999, "upright");
        }
    }
}

/// The first unfold, ready and shot times of turret `e`.
fn timeline(sim: &mut Sim, e: Entity, secs: f64) -> (f64, Option<f64>, Option<f64>, Option<f64>) {
    let landed = obj(sim, e).s.landed_at;
    let (mut unfold, mut ready, mut shot) = (None, None, None);
    let end = now(sim) + secs;
    while now(sim) < end && alive(sim, e) {
        sim.ticks(1);
        let o = obj(sim, e);
        if unfold.is_none() {
            unfold = o.s.unfold_at;
        }
        if ready.is_none() && o.s.ready {
            ready = Some(now(sim));
        }
        if shot.is_none() && o.s.shots > 0 {
            shot = Some(now(sim));
        }
    }
    (landed, unfold, ready, shot)
}

/// Case 68: the unfold starts on the update after the landing with the
/// ready sound (radius 1000); the turret is ready on the frame at 2850 ms
/// with its head and gun along the throw; an enemy in front from the start
/// is not shot before then, and the first shot follows on a later update.
#[test]
fn t68_unfold_timeline() {
    let mut sim = tsim();
    let owner = far_player(&mut sim, 1);
    let enemy = player(&mut sim, 0.0, -300.0, 0.0, 2);
    set_health(&mut sim, enemy, 10000.0);
    let e = landed(&mut sim, "test:turret_exact", Some(owner), 0.0, 0.0);
    let (landed_at, unfold, ready, shot) = timeline(&mut sim, e, 4.0);
    let unfold = unfold.expect("unfolds");
    assert!((unfold - (landed_at + 0.05)).abs() < 1e-6, "the update after landing");
    let ready = ready.expect("ready");
    assert!(ready >= unfold + 2.85 - 1e-6 && ready < unfold + 2.85 + 0.02, "ready at {}", ready - unfold);
    let shot = shot.expect("fires");
    assert!(shot > ready, "no shot before it is ready (and not on its frame)");
    let first_hit = hits(&sim).iter().filter(|h| h.1 == enemy).map(|h| h.0).next().expect("hit");
    assert!(first_hit >= unfold + 2.85, "no damage before 2850 ms");
    let set: Vec<_> = sim
        .app
        .world()
        .resource::<Sounds>()
        .0
        .iter()
        .filter(|(_, s)| s.file.ends_with("autoturret/set.wav"))
        .cloned()
        .collect();
    assert_eq!(set.len(), 1, "the ready sound once");
    assert!((set[0].0 - unfold).abs() < 0.03, "at the unfold's start");
    assert!(set[0].1.server && set[0].1.radii.1 == 1000.0);
    let o = obj(&sim, e);
    assert_near(o.s.head_dir, o.s.facing, 0.3, "head starts along the throw");
}

/// Case 69: destroyed 1000 ms into the unfold (117 explode, x3) it goes
/// with its disappear effect at the floor point, never ready, never
/// firing.
#[test]
fn t69_destroyed_while_unfolding() {
    let mut sim = tsim();
    let owner = far_player(&mut sim, 1);
    let attacker = far_player(&mut sim, 2);
    let enemy = player(&mut sim, 0.0, -300.0, 0.0, 2);
    set_health(&mut sim, enemy, 10000.0);
    let e = landed(&mut sim, "test:turret_exact", Some(owner), 0.0, 0.0);
    while obj(&sim, e).s.unfold_at.is_none_or(|u| now(&sim) < u + 1.0) {
        sim.ticks(1);
    }
    let floor = obj(&sim, e).at() - Vec3::Y * obj(&sim, e).half.y * U;
    sim.app.world_mut().write_message(Damage {
        target: e,
        attacker: Some(attacker),
        amount: 117.0 * 0.01,
        point: floor,
        dir: Vec3::NEG_Y,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Blast,
        weapon: None,
        force: Vec3::ZERO,
    });
    wait(&mut sim, 0.3);
    assert!(!alive(&sim, e), "destroyed");
    let gone: Vec<_> = booms(&sim).into_iter().filter(|b| b.1 == "test:turret_exact").collect();
    assert_eq!(gone.len(), 1, "its disappear effect");
    assert_near(gone[0].3, floor, 0.01, "at the floor point");
    assert!(hits(&sim).iter().all(|h| h.1 != enemy), "never fired");
}

/// A ready turret (landed, unfolded) of `id` at `x`, `z` u.
fn ready(sim: &mut Sim, id: &str, owner: Entity, x: f32, z: f32) -> Entity {
    let e = landed(sim, id, Some(owner), x, z);
    for _ in 0..200 {
        sim.ticks(1);
        if obj(sim, e).s.ready {
            return e;
        }
    }
    panic!("never ready");
}

/// Cases 70 and 71: the aim snaps to the target on each shot only (head
/// flat, gun along the line), and the ray starts at the gun's flash
/// socket after the turn; with no target the gun stays.
#[test]
fn t70_71_aim_per_shot_and_muzzle() {
    let mut sim = tsim();
    let owner = far_player(&mut sim, 1);
    let e = ready(&mut sim, "test:turret_exact", owner, 0.0, 0.0);
    let o = obj(&sim, e);
    assert_near(o.s.facing, Vec3::NEG_Z, 1e-4, "facing its throw (none: -Z)");
    // To its right (+X), 500 u.
    let enemy = player(&mut sim, 500.0, 0.0, 0.0, 2);
    set_health(&mut sim, enemy, 10000.0);
    let mut seen = Vec::new();
    for _ in 0..60 {
        let before = obj(&sim, e);
        put(&mut sim, enemy, 500.0, before.s.shots as f32 * 5.0);
        sim.ticks(1);
        let o = obj(&sim, e);
        if o.s.shots == before.s.shots {
            assert_eq!(o.s.gun_dir, before.s.gun_dir, "turns only when it fires");
        } else {
            seen.push(o.clone());
        }
    }
    assert!(seen.len() >= 3, "fired");
    let o = &seen[0];
    let t = match &o.kind {
        special::Kind::Device(d) => d.turret.clone().unwrap(),
        _ => unreachable!(),
    };
    assert!(o.s.head_dir.x > 0.99, "head to the right at once: {}", o.s.head_dir);
    let g = turret::place(o.pos, o.s.facing, t.gun_at) * U;
    let muzzle = turret::place(g / U, o.s.gun_dir, t.flash) * U;
    assert_near(o.s.shot_from, muzzle, 1e-4, "from the flash socket");
    // The flash socket's place on a level gun along -Z: 35.1 ahead, 8.8 up,
    // 1.6 to its right.
    let level = turret::place(Vec3::ZERO, Vec3::NEG_Z, t.flash);
    assert_near(level, Vec3::new(1.6, 8.8, -35.1), 1e-4, "the socket");
    // Gone (no target): the gun stays where it last fired.
    let last = obj(&sim, e).s.gun_dir;
    put(&mut sim, enemy, 5000.0, 5000.0);
    wait(&mut sim, 0.5);
    assert_eq!(obj(&sim, e).s.gun_dir, last, "stays");
}

/// Case 72: the HG2 turret's numbers (200 HP, 7 per ray, its own ready
/// sound) and no `OnceActivate`: a second HG2 leaves the first, a second
/// stock turret removes the first with its disappear effect.
#[test]
fn t72_hg2_variant() {
    let mut sim = tsim();
    let owner = far_player(&mut sim, 1);
    let enemy = player(&mut sim, 0.0, -300.0, 0.0, 2);
    set_health(&mut sim, enemy, 10000.0);
    let e = ready(&mut sim, "test:hg2_turret", owner, 0.0, 0.0);
    assert_eq!(obj(&sim, e).health, 200.0);
    wait(&mut sim, 0.5);
    let shots: Vec<_> = hits(&sim).into_iter().filter(|h| h.1 == enemy).collect();
    assert!(!shots.is_empty());
    for h in &shots {
        assert!(in_a_zone(h.3, 7.0), "7 a ray: {}", h.3);
    }
    let sounds = &sim.app.world().resource::<Sounds>().0;
    assert!(sounds.iter().any(|(_, s)| s.file.ends_with("set_hg2.wav")));
    throw(&mut sim, owner, "test:hg2_turret");
    assert!(alive(&sim, e), "a second HG2 leaves the first");
    // A second stock turret removes the first with its disappear effect.
    let mut sim = tsim();
    let owner = far_player(&mut sim, 1);
    let first = landed(&mut sim, TURRET, Some(owner), 0.0, 0.0);
    throw(&mut sim, owner, TURRET);
    assert!(!alive(&sim, first), "replaced");
    let gone: Vec<_> = booms(&sim).into_iter().filter(|b| b.1 == TURRET).collect();
    assert_eq!(gone.len(), 1, "with its disappear effect");
}

/// A device of `id` thrown by `owner` by its weapon (`devices::launch`).
fn throw(sim: &mut Sim, owner: Entity, id: &str) {
    let w = weapon_of(sim, id, Some(owner));
    devices::launch(
        sim.app.world_mut(),
        Launch {
            weapon: w,
            thrower: owner,
            eye: Vec3::new(0.0, 1.0, 3.0),
            yaw: 0.0,
            pitch: 0.0,
            velocity: Vec3::ZERO,
            command: 3,
        },
    );
    sim.ticks(3);
}

/// Case 73: one count of rays for all the server's turrets: the tracer
/// rides every third ray of the combined sequence.
#[test]
fn t73_tracer_counter() {
    let mut sim = tsim();
    let owner = far_player(&mut sim, 1);
    let a = ready(&mut sim, "test:turret_exact", owner, -100.0, 0.0);
    let b = ready(&mut sim, "test:turret_exact", owner, 100.0, 0.0);
    let enemy = player(&mut sim, 0.0, -300.0, 0.0, 2);
    set_health(&mut sim, enemy, 100000.0);
    let mut serials = Vec::new();
    let mut last = [0, 0];
    for _ in 0..120 {
        sim.ticks(1);
        for (i, e) in [a, b].into_iter().enumerate() {
            let s = sim.app.world().get::<CaTurretShots>(e).unwrap().clone();
            if s.shots != last[i] {
                last[i] = s.shots;
                serials.push((i, s.serial));
            }
        }
    }
    serials.sort_by_key(|s| s.1);
    let n: Vec<u32> = serials.iter().map(|s| s.1).collect();
    assert!(n.len() >= 6);
    assert_eq!(n, (n[0]..n[0] + n.len() as u32).collect::<Vec<_>>(), "one count for both");
    assert!(serials.iter().any(|s| s.0 == 0) && serials.iter().any(|s| s.0 == 1));
    let tracers: Vec<u32> = n.iter().copied().filter(|s| special::turret::tracer(*s, 3)).collect();
    assert!(tracers.iter().all(|s| s % 3 == 0));
}

/// Removal (T3 8): its owner gone, silently; its last round fired, with
/// its disappear effect on the next update.
#[test]
fn a_turret_goes_silently_with_its_owner_and_with_its_effect_when_empty() {
    let mut sim = tsim();
    let owner = far_player(&mut sim, 1);
    let e = landed(&mut sim, TURRET, Some(owner), 0.0, 0.0);
    sim.app.world_mut().despawn(owner);
    wait(&mut sim, 0.2);
    assert!(!alive(&sim, e), "gone with its owner");
    assert!(booms(&sim).iter().all(|b| b.1 != TURRET), "silently");

    let mut sim = tsim();
    let owner = far_player(&mut sim, 1);
    let enemy = player(&mut sim, 0.0, -300.0, 0.0, 2);
    set_health(&mut sim, enemy, 100000.0);
    let e = ready(&mut sim, "test:turret_exact", owner, 0.0, 0.0);
    sim.app.world_mut().get_mut::<CaObject>(e).unwrap().s.ammo = 1;
    let mut last = None;
    for _ in 0..60 {
        sim.ticks(1);
        if !alive(&sim, e) {
            break;
        }
        if obj(&sim, e).s.ammo == 0 && last.is_none() {
            last = Some(frame(&sim));
        }
    }
    let last = last.expect("its last round");
    assert!(!alive(&sim, e), "gone when empty");
    let gone: Vec<_> = booms(&sim).into_iter().filter(|b| b.1 == "test:turret_exact").collect();
    assert_eq!(gone.len(), 1, "with its disappear effect");
    assert!(gone[0].0 > last, "on the update after its last round");
}

/// The drawn parts (`special::turret::draw`) on stand-in drawn forms: the
/// unfold's frames in turn with the object's own model hidden, then the
/// ready base with the head and gun turned to the server's directions and
/// the gun's fire frame on a new shot.
#[test]
fn a_turret_draws_its_parts() {
    use chimera_world::map::loose::{LooseAsset, LooseAssets};
    let mut sim = tsim();
    let w = weapon_of(&mut sim, "test:turret_modelled", None);
    let d = sim.app.world().get::<CaDeviceData>(w).unwrap().params.clone();
    let keys = turret::TurretKeys::of(&d).expect("modelled");
    let mut assets = LooseAssets::default();
    for k in keys.unfold.iter().chain([&keys.ready, &keys.head]).chain(&keys.gun) {
        assets.0.insert(
            k.clone(),
            LooseAsset {
                frame: Transform::from_translation(Vec3::new(0.0, -0.1, 0.0)),
                half: Vec3::splat(0.1),
                surface: None,
                nose: None,
            },
        );
    }
    sim.app.insert_resource(assets);
    let e = sim
        .app
        .world_mut()
        .spawn((
            Transform::default(),
            Visibility::default(),
            CaTurretShots {
                weapon: "test:turret_modelled".into(),
                phase: TurretPhase::Unfolding,
                ..default()
            },
        ))
        .id();
    let own = sim.app.world_mut().spawn((Transform::default(), Visibility::default(), ChildOf(e))).id();
    sim.ticks(2);
    let parts = |sim: &Sim| {
        let p = sim.app.world().get::<turret::TurretParts>(e).expect("its parts");
        (p.unfold.clone(), p.ready, p.head, p.gun.clone())
    };
    let shown = |sim: &Sim, x: Entity| sim.app.world().get::<Visibility>(x) == Some(&Visibility::Inherited);
    let (unfold, ready, head, gun) = parts(&sim);
    assert_eq!(unfold.len(), turret::unfold_frames(d.turret.as_ref().unwrap()));
    assert!(shown(&sim, unfold[0]) && !shown(&sim, ready) && !shown(&sim, head), "the unfold's first frame");
    assert_eq!(sim.app.world().get::<Visibility>(own), Some(&Visibility::Hidden), "the case's own model goes");
    sim.seconds(1.0);
    let at = unfold.iter().position(|u| shown(&sim, *u)).expect("a frame");
    assert!((18..=22).contains(&at), "a second in: frame {at}");
    {
        let mut s = sim.app.world_mut().get_mut::<CaTurretShots>(e).unwrap();
        s.phase = TurretPhase::Ready;
        s.head = [1.0, 0.0, 0.0];
        s.gun = [1.0, 0.0, 0.0];
        s.shots = 1;
    }
    sim.ticks(1);
    assert!(unfold.iter().all(|u| !shown(&sim, *u)) && shown(&sim, ready) && shown(&sim, head));
    assert!(shown(&sim, gun[1]) && !shown(&sim, gun[0]), "the fire animation from its start");
    let r = sim.app.world().get::<Transform>(head).unwrap().rotation;
    assert!((r * Vec3::NEG_Z - Vec3::X).length() < 1e-4, "the head faces +X");
    sim.seconds(0.5);
    assert!(shown(&sim, gun[0]), "idle after it");
}

/// Its zone effect (`Ind_Mine_SH`) once it unfolds, for its owner's team
/// only (6.0, T3 3).
#[test]
fn a_turrets_zone_effect_is_its_teams() {
    use combat_arms::game::effects::attached::CaAttachedGroups;
    let mut sim = tsim();
    let me = sim
        .app
        .world_mut()
        .spawn((chimera_core::core::Team(1), chimera_core::core::LocalPlayer))
        .id();
    let turret = |sim: &mut Sim, team: u8| {
        sim.app
            .world_mut()
            .spawn(CaTurretShots {
                weapon: TURRET.into(),
                team,
                phase: TurretPhase::Unfolding,
                ..default()
            })
            .id()
    };
    let mine = turret(&mut sim, 1);
    let theirs = turret(&mut sim, 2);
    sim.ticks(1);
    let groups = |sim: &Sim, e: Entity| {
        sim.app
            .world()
            .get::<CaAttachedGroups>(e)
            .map(|g| g.0.iter().map(|a| a.group.clone()).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    assert_eq!(groups(&sim, mine), vec!["Ind_Mine_SH".to_string()]);
    assert!(groups(&sim, theirs).is_empty(), "not the other team's");
    // He changes team: it follows him.
    sim.app.world_mut().entity_mut(me).insert(chimera_core::core::Team(2));
    sim.ticks(1);
    assert!(groups(&sim, mine).is_empty());
    assert_eq!(groups(&sim, theirs), vec!["Ind_Mine_SH".to_string()]);
}
