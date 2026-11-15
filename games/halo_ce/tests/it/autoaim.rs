//! Halo's autoaim (specs/halo_ce/weapons.md section 15,
//! `game::weapons::aim`): the parameters in force by weapon and zoom (the
//! sniper rifle's only when zoomed, its deviation cone 0.5 degrees at 2x
//! and 0.125 at 8x, the pistol's 3 degrees, none for the rocket), the
//! bending within the deviation cone, and in the game on `HaloSim`: the
//! autoaim on by default for every Halo gun, a zoomed sniper rifle's shot
//! bent toward a target just beside its aim, and a needle given the
//! autoaim's target (projectiles.md 12); the data's switch off leaves the
//! shot as aimed and the needle without a target.

use bevy::prelude::*;
use halo_ce::game::{
    movement,
    weapons::{
        GUNS, HaloFired, NEEDLER, PISTOL, ROCKET_LAUNCHER, SNIPER_RIFLE,
        aim::{self, assist, bend},
        gun,
        systems::HaloWeapon,
    },
};

use super::weapons::armed;

fn deg(x: f32) -> f32 {
    x.to_radians()
}

#[test]
fn the_parameters_in_force_by_weapon_and_zoom() {
    let sniper = gun(SNIPER_RIFLE);
    assert_eq!(assist(&sniper, -1), None, "the sniper rifle: only when zoomed");
    let z2 = assist(&sniper, 0).unwrap();
    assert!((z2.deviation - deg(0.5)).abs() < 1e-6, "2x: {}", z2.deviation);
    assert!((z2.autoaim_range - 2.0 * sniper.aim_assist.autoaim_range).abs() < 1e-4);
    let z8 = assist(&sniper, 1).unwrap();
    assert!((z8.deviation - deg(0.125)).abs() < 1e-6, "8x: {}", z8.deviation);
    let pistol = assist(&gun(PISTOL), -1).unwrap();
    assert!((pistol.deviation - deg(3.0)).abs() < 1e-6, "pistol: {}", pistol.deviation);
    assert_eq!(assist(&gun(ROCKET_LAUNCHER), -1).unwrap().deviation, 0.0, "rockets fly as aimed");
    for id in GUNS {
        assert!(gun(id).aim_assist.enabled, "{id}: the autoaim on by default");
    }
}

#[test]
fn the_level_and_the_bend() {
    assert_eq!(aim::level(0.4, 1.0), 1.0);
    assert!((aim::level(0.75, 1.0) - 0.5).abs() < 1e-6);
    assert_eq!(aim::level(1.0, 1.0), 0.0);
    assert_eq!(aim::level(0.1, 0.0), 0.0);
    let d = Vec3::X;
    // No target, the origin on the camera ray: the shot as aimed.
    assert!((bend(d, Vec3::ZERO, Vec3::X * 20.0, None, deg(3.0)) - d).length() < 1e-6);
    // A target 2 degrees aside at level 1: turned onto it.
    let t = Vec3::new(deg(2.0).cos(), deg(2.0).sin(), 0.0) * 10.0;
    let b = bend(d, Vec3::ZERO, Vec3::X * 20.0, Some((t, 1.0)), deg(3.0));
    assert!((b.angle_between(t) - 0.0).abs() < 1e-5, "{b}");
    // At level 0.5: halfway (the normalised interpolation).
    let b = bend(d, Vec3::ZERO, Vec3::X * 20.0, Some((t, 0.5)), deg(3.0));
    assert!((b.angle_between(d) - deg(1.0)).abs() < 1e-4, "{b}");
    // Outside the cone: on its edge.
    let b = bend(d, Vec3::ZERO, Vec3::X * 20.0, Some((t, 1.0)), deg(0.5));
    assert!((b.angle_between(d) - deg(0.5)).abs() < 1e-5 && b.y > 0.0, "{b}");
    // Deviation 0: never turned.
    assert_eq!(bend(d, Vec3::ZERO, Vec3::X * 20.0, Some((t, 1.0)), 0.0), d);
    // The convergence: an origin off the ray turns toward the look point.
    let b = bend(d, Vec3::new(0.0, -0.1, 0.0), Vec3::X * 10.0, None, deg(3.0));
    assert!((b.angle_between(Vec3::new(10.0, 0.1, 0.0)) - 0.0).abs() < 1e-5, "{b}");
}

/// `id` held, its data's autoaim switch `on`, aimed `offset` radians to
/// the right of a second Spartan 30 m ahead (zoomed once if `zoom`), one
/// shot: the shooter's aim, the shot's (first projectile's) direction and
/// target, and the target.
fn shoot_beside(id: &str, on: bool, zoom: bool, offset: f32) -> (Vec3, Vec3, Option<Entity>, Entity) {
    shoot_beside_bent(id, on, zoom, offset, true)
}

/// `shoot_beside` with the bend's switch (`aim::Bend`) set to `bend`.
fn shoot_beside_bent(id: &str, on: bool, zoom: bool, offset: f32, bend: bool) -> (Vec3, Vec3, Option<Entity>, Entity) {
    let (mut sim, p, g) = armed(id);
    sim.app.insert_resource(halo_ce::game::weapons::aim::Bend(bend));
    if !on {
        let mut w = sim.app.world_mut().get_mut::<HaloWeapon>(g).unwrap();
        let mut data = (*w.0).clone();
        data.aim_assist.enabled = false;
        w.0 = std::sync::Arc::new(data);
    }
    let at = sim.position(p) - Vec3::Z * 30.0;
    let t = sim.spawn_character(at, movement::ID);
    sim.ticks(2);
    if zoom {
        sim.intent(p).secondary = true;
        sim.ticks(1);
        sim.intent(p).secondary = false;
        sim.ticks(1);
    }
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let to = sim.position(t) - eye;
    let i = &mut *sim.intent(p);
    i.yaw = (-to.x).atan2(-to.z) - offset;
    i.pitch = (to.y / to.xz().length()).atan();
    let (yaw, pitch) = (i.yaw, i.pitch);
    let aim = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Vec3::NEG_Z;
    let mut fired = sim.app.world().resource::<Messages<HaloFired>>().get_cursor_current();
    sim.intent(p).fire = true;
    let mut shot = None;
    for _ in 0..10 {
        sim.ticks(1);
        if let Some(f) = fired.read(sim.app.world().resource::<Messages<HaloFired>>()).find(|f| f.owner == p) {
            shot = Some((f.projectiles[0].0, f.target));
            break;
        }
    }
    let (d, target) = shot.expect("fired");
    (aim, d, target, t)
}

#[test]
fn a_zoomed_sniper_shot_is_bent_toward_a_target_beside_its_aim() {
    // 0.3 degrees to the right at 2x (autoaim angle 0.5 degrees), the
    // switch on: the target is taken, but its aim point is on its pill
    // pulled toward the ray by up to the autoaim width (0.08 WU), and the
    // ray passes that close to it here, so the aim point is on the ray and
    // the shot keeps its line (weapons.md 15, the fourth errata).
    let (aim, d, _, _) = shoot_beside(SNIPER_RIFLE, true, true, deg(0.3));
    let turned = aim.angle_between(d);
    assert!(turned < deg(0.01), "turned {} degrees", turned.to_degrees());
    // The switch off: exactly as aimed.
    let (aim, d, _, _) = shoot_beside(SNIPER_RIFLE, false, true, deg(0.3));
    assert!(aim.angle_between(d) < 1e-5, "as aimed: {}", aim.angle_between(d).to_degrees());
}

#[test]
fn a_needle_takes_the_autoaims_target() {
    let (_, _, target, t) = shoot_beside(NEEDLER, true, false, deg(3.0));
    assert_eq!(target, Some(t), "the autoaim's target");
    let (_, _, target, _) = shoot_beside(NEEDLER, false, false, deg(3.0));
    assert_eq!(target, None, "the switch off: no target, no guidance");
}

/// weapons.md 15's test rows (fourth errata): the aim point on the
/// pelvis-to-head pill (Halo's axes, z up): a camera above the head aims
/// at the head raised toward the ray by the 0.06 it is off (under the
/// width 0.08); a camera crossing the pill aims where it crosses.
#[test]
fn the_aim_point_is_on_the_pelvis_to_head_pill() {
    use halo_ce::game::weapons::aim::{AUTOAIM_WIDTH, HEAD_HEIGHT, PELVIS_HEIGHT, aim_point};
    let (pelvis, head) = (Vec3::new(20.0, 0.0, PELVIS_HEIGHT), Vec3::new(20.0, 0.0, HEAD_HEIGHT));
    let p = aim_point(pelvis, head, AUTOAIM_WIDTH, Vec3::new(0.0, 0.0, 0.62), Vec3::X);
    assert!(p.abs_diff_eq(Vec3::new(20.0, 0.0, 0.62), 1e-5), "{p}");
    let p = aim_point(pelvis, head, AUTOAIM_WIDTH, Vec3::new(0.0, 0.0, 0.45), Vec3::X);
    assert!(p.abs_diff_eq(Vec3::new(20.0, 0.0, 0.45), 1e-5), "{p}");
    // Far above: the head, raised by the width alone.
    let p = aim_point(pelvis, head, AUTOAIM_WIDTH, Vec3::new(0.0, 0.0, 2.0), Vec3::X);
    assert!(p.abs_diff_eq(Vec3::new(20.0, 0.0, 0.64), 1e-5), "{p}");
}


/// The bend is off by default (mouse and keyboard: `aim::Bend`): a shot
/// with a target just inside the autoaim cone flies along the aim, while
/// the needles still take the target (guidance is not the bend); with the
/// switch on, both.
#[test]
fn by_default_shots_fly_straight_and_needles_still_home() {
    let (aim, d, _, _) = shoot_beside_bent(SNIPER_RIFLE, true, true, deg(0.3), false);
    assert!(aim.angle_between(d) < 1e-5, "straight: {}", aim.angle_between(d).to_degrees());
    let (_, _, target, t) = shoot_beside_bent(NEEDLER, true, false, deg(3.0), false);
    assert_eq!(target, Some(t), "the needle's guidance keeps its target");
    let (aim, d, target, t) = shoot_beside_bent(NEEDLER, true, false, deg(3.0), true);
    assert_eq!(target, Some(t));
    assert!(aim.angle_between(d) > 1e-4, "bent with the switch on");
}
