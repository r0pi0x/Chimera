//! specs/combat_arms/attachments.md's test cases, numbered as there, on
//! Combat Arms' simulation (`CaSim`) and the greybox: the M4A1 and L96A1
//! from the game's data with the parts their players chose (`CaLoadout`),
//! and test weapons made from them as data where a case names another
//! weapon (the M24, the Barrett, the AK-47, an event weapon) or aims at one
//! body part ("exact": no roll, spread or kick, as weapon_firing.rs does).
//! Where a case states the meters' or the kick's values, the weapon's own
//! parts (as fitting left them in the sim) are stepped as the systems step
//! them. Cases 30 and 31 (the HUD) are tests/ui/attachments.rs's; the
//! install's modification file (36, 37) is read from documents written
//! here. A network test checks the fitted parts are the server's and
//! predicted.
//!
//! Units: world units of 1.8 cm, hit points (health x 100). Tolerances:
//! 1e-3 HP for damage, 1e-5 for the meters, 1e-3 degrees for view angles
//! and the field of view, one tick (15 ms) for times, 1e-3 s for reload
//! lengths.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{Health, Intent, MovementState, PropSurface, SimClock, Team};
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, predict::NetGraph},
};
use chimera_sim::weapon::{
    self, Hitscan, Inventory, Magazine, SpreadShape, ViewPunch, Weapon, Zoomed, data::WeaponDataAppExt,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        attachments::{
            self as at, CaDefaultLoadout, CaFitted, CaLoadout, CaModTable, CaMods, CaNpc, CaSpecialRoll, FireSound,
            ModRecord, ObserverSound, PartModel,
        },
        data::{
            attributes::{ModFile, WeaponsFile},
            merge_weapon_docs_with_mods,
        },
        inventory::{self, Dropped},
        movement::{self, CaMovement, StunPenalty},
        weapons::{
            self as ca, CaKick, CaRecoil, CaSpread, CaZoom, FireMode, MALE, RIFLE, RecoilParams, SpreadScales,
        },
    },
    harness::CaSim,
};
use lithtech_formats::attributes::Document;

const U: f32 = 0.018;
const HP: f32 = 0.01;
const L96A1: &str = "combat_arms:l96a1";
const M60: &str = "combat_arms:m60";

/// The M4A1 with no damage roll, no spread and no kick (its accepted list
/// is the M4A1's).
const EXACT: &str = r#"
id = "test:m4a1_exact"
base = "combat_arms:m4a1"
[hooks."combat_arms:damage"]
roll = 0.0
[hooks."combat_arms:spread"]
cone = { min = 0, max = 0, inc = 1.3, dec = 0.3 }
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;

/// The M4A1 without the recoil's return.
const NO_RETURN: &str = r#"
id = "test:m4a1_no_return"
base = "combat_arms:m4a1"
[hooks."combat_arms:recoil"]
jitter_wait_recover = 0.0
jitter_recover = 0.0
"#;

/// The M4A1 with no firing recovery (all rounds within DecDelay) and no
/// kick.
const STEADY: &str = r#"
id = "test:m4a1_steady"
base = "combat_arms:m4a1"
[hooks."combat_arms:spread"]
dec_delay = 100000.0
[hooks."combat_arms:recoil"]
pitch = 0.0
"#;

/// An M4A1 that lists the unused, non-integrated "Sniper Silencer".
const LISTS_SNIPER_SILENCER: &str = r#"
id = "test:m4a1_sniper_silencer"
base = "combat_arms:m4a1"
[hooks."combat_arms:attachments"]
accepts = ["Sniper Silencer"]
"#;

/// An event weapon: four named defaults (muzzle, magazine, bullet,
/// mechanical sight), the M4A1's list.
const EVENT: &str = r#"
id = "test:m4a1_event"
base = "test:m4a1_exact"
[hooks."combat_arms:attachments"]
defaults = { muzzle = "Silencer_B_AR", magazine = "Magazine_A_AR", bullet = "Bullet_FT_I_AR", mech_sight = "MechSight_Aim_I_AR" }
"#;

/// The M24 (clip 7): "Sniper Scope", magazines A at column 4 and C at
/// column 1.
const M24: &str = r#"
id = "test:m24"
base = "combat_arms:l96a1"
[magazine]
clip = 7
[hooks."combat_arms:attachments"]
accepts = ["Sniper Scope", { name = "Magazine_A_SR", variant = 4 }, { name = "Magazine_C_SR", variant = 1 }]
"#;

/// The Barrett with its two-level scope.
const BARRETT: &str = r#"
id = "test:barrett"
base = "combat_arms:l96a1"
[hooks."combat_arms:zoom"]
reload_after_fire = false
[hooks."combat_arms:attachments"]
accepts = ["Barrett Scope"]
defaults = { sight = "Barrett Scope" }
"#;

/// The AK-47: its red dot at variant 2 (the AK mount), silenced radius
/// 1600.
const AK47: &str = r#"
id = "test:ak47"
base = "combat_arms:m4a1"
[hooks."combat_arms:attachments"]
fire_sound_radius = 2000
silenced_radius = [1600, 1600]
accepts = ["Silencer_A_AR", { name = "DotSight_A_AR", variant = 2 }]
"#;

/// Case 19's weapon: the M4A1 listing a test bullet with a reload rate.
const RATE_BULLET: &str = r#"
id = "test:m4a1_rate_bullet"
base = "combat_arms:m4a1"
[hooks."combat_arms:attachments"]
accepts = ["Test_Rate_Bullet"]
"#;

/// A Remington 870 with no spread, roll, kick or spacing that takes the
/// test pellet parts (case 38).
const PELLETS: &str = r#"
id = "test:r870_pellets"
base = "combat_arms:remington_870"
[hooks."combat_arms:damage"]
roll = 0.0
fire_delay = 0.0
[hooks."combat_arms:spread"]
cone = { min = 0, max = 0, inc = 5.0, dec = 1.0 }
[hooks."combat_arms:recoil"]
pitch = 0.0
[hooks."combat_arms:attachments"]
accepts = ["Test_Pellet_Mag", "Test_Pellet_Muzzle"]
"#;

fn sim() -> Sim {
    let mut sim = Sim::new(CaSim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app
        .add_weapon_data(&[
            ("exact", EXACT),
            ("no_return", NO_RETURN),
            ("steady", STEADY),
            ("sniper_silencer", LISTS_SNIPER_SILENCER),
            ("event", EVENT),
            ("m24", M24),
            ("barrett", BARRETT),
            ("ak47", AK47),
            ("rate_bullet", RATE_BULLET),
            ("pellets", PELLETS),
        ])

        .unwrap();
    sim
}

/// A Combat Arms player at `at` looking along `yaw` that chose `parts` for
/// weapon `id`, settled, with health `health` (1.0 = 100 HP) and no
/// armour.
fn player(sim: &mut Sim, at: Vec3, yaw: f32, health: f32, id: &str, parts: &[&str]) -> Entity {
    let p = sim.spawn_character(at, movement::ID);
    sim.app.world_mut().entity_mut(p).insert(CaLoadout::default().with(id, parts));
    sim.intent(p).yaw = yaw;
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert((
        Health {
            current: health,
            max: health,
        },
        combat_arms::game::weapons::gear_armour(0.0, &Vec::new()),
    ));
    p
}

/// Give `p` the weapon `id`, fitted and drawn.
fn arm(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = weapon::give(sim.app.world_mut(), p, id).expect("unknown weapon");
    sim.seconds(1.3);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "{id} drawn");
    w
}

/// A player in the open holding `id` with `parts`.
fn holding(sim: &mut Sim, id: &str, parts: &[&str]) -> (Entity, Entity) {
    let p = player(sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0, id, parts);
    let w = arm(sim, p, id);
    (p, w)
}

fn mods(sim: &Sim, w: Entity) -> CaMods {
    sim.app.world().get::<CaMods>(w).unwrap().clone()
}

/// The fitted part's name in each slot.
fn fitted(sim: &Sim, w: Entity) -> [Option<String>; at::SLOTS] {
    let m = mods(sim, w);
    std::array::from_fn(|s| m.part(s).map(|p| p.record.name.clone()))
}

fn name(n: &str) -> Option<String> {
    Some(n.to_string())
}

fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current
}

fn mag(sim: &Sim, w: Entity) -> Magazine {
    sim.app.world().get::<Magazine>(w).unwrap().clone()
}

fn set_mag(sim: &mut Sim, w: Entity, clip: u32, reserve: u32) {
    let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
    m.clip = clip;
    m.reserve = reserve;
}

fn eye(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) + sim.state(p).eye_offset
}

fn geometry(world: &World, p: Entity) -> (Vec3, Vec3, Vec<(Vec3, f32)>) {
    ca::body_geometry(
        world.get::<Transform>(p).unwrap(),
        world.get::<Intent>(p).unwrap(),
        world.get::<MovementState>(p).unwrap(),
        world.get::<ColliderAabb>(p),
        *MALE,
    )
    .unwrap()
}

fn sphere(sim: &Sim, shooter: Entity, p: Entity, name: &str) -> Vec3 {
    super::aim_points::point(sim.app.world(), shooter, p, name)
}

fn aim(sim: &mut Sim, shooter: Entity, point: Vec3) {
    let d = point - eye(sim, shooter);
    let mut i = sim.intent(shooter);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
}

/// Fire one round and let it land: the hit points `target` lost.
fn shoot(sim: &mut Sim, shooter: Entity, target: Entity) -> f32 {
    let before = health(sim, target);
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    // The chain's round, 3 updates after the press (weapon_firing.md
    // Corrections 10).
    sim.ticks(4);
    let lost = (before - health(sim, target)) / HP;
    sim.seconds(0.2);
    lost
}

fn shoot_at(sim: &mut Sim, shooter: Entity, target: Entity, name: &str) -> f32 {
    let at = sphere(sim, shooter, target, name);
    aim(sim, shooter, at);
    shoot(sim, shooter, target)
}

/// Move `target` along -Z until the impact point on its `name` sphere's
/// ray is `units` from `shooter`'s eye.
fn place_at(sim: &mut Sim, shooter: Entity, target: Entity, name: &str, units: f32) {
    for _ in 0..6 {
        let from = eye(sim, shooter);
        let dir = (sphere(sim, shooter, target, name) - from).normalize();
        let (min, max, _) = geometry(sim.app.world(), target);
        // The reported point: the widened box's entry (weapon_firing.md
        // Corrections 8).
        let (c, half) = ((min + max) / 2.0, (max - min) / 2.0 * Vec3::new(ca::HITBOX_WIDEN, 1.0, ca::HITBOX_WIDEN));
        let t = ca::box_entry(c - half, c + half, from, dir, f32::INFINITY).unwrap();

        let err = t - units * U;
        sim.app.world_mut().get_mut::<Transform>(target).unwrap().translation.z += err / dir.z.abs();
        sim.ticks(1);
    }
}

fn close(a: f32, b: f32, tol: f32, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a}, expected {b}");
}

/// A shooter holding `id` with `parts`, looking down -Z, and a target
/// (10000 HP) facing it `units` away in the open lane at x = 30.
fn range_lane(sim: &mut Sim, id: &str, parts: &[&str], units: f32) -> (Entity, Entity) {
    let shooter = player(sim, Vec3::new(30.0, 1.0, 37.0), 0.0, 1.0, id, parts);
    let target = player(sim, Vec3::new(30.0, 1.0, 37.0 - units * U), std::f32::consts::PI, 100.0, id, &[]);
    arm(sim, shooter, id);
    (shooter, target)
}

/// The damage `id` with `parts` deals to a chest `units` away (roll 1.0),
/// the target set up by `victim`.
fn chest_damage(id: &str, parts: &[&str], units: f32, victim: impl FnOnce(&mut World, Entity)) -> f32 {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, id, parts, units);
    victim(sim.app.world_mut(), target);
    place_at(&mut sim, shooter, target, "Torso", units);
    shoot_at(&mut sim, shooter, target, "Torso")
}

fn as_player(_: &mut World, _: Entity) {}

fn as_human_ai(w: &mut World, e: Entity) {
    w.entity_mut(e).insert(CaNpc { zombie: false });
}

fn as_zombie_ai(w: &mut World, e: Entity) {
    w.entity_mut(e).insert(CaNpc { zombie: true });
}

/// Attack2 down for one tick.
fn press_zoom(sim: &mut Sim, p: Entity) {
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
}

fn cz(sim: &Sim, w: Entity) -> CaZoom {
    sim.app.world().get::<CaZoom>(w).unwrap().clone()
}

/// Press zoom and wait for the transition: the FOV it settles at.
fn zoom_step(sim: &mut Sim, p: Entity, w: Entity) -> f32 {
    press_zoom(sim, p);
    for _ in 0..100 {
        if cz(sim, w).transition.is_none() {
            break;
        }
        sim.ticks(1);
    }
    cz(sim, w).fov
}

/// The weapon's spread part with its meters at rest.
fn spread(sim: &Sim, w: Entity) -> CaSpread {
    let mut s = sim.app.world().get::<CaSpread>(w).unwrap().clone();
    s.fire = 0.0;
    s.movement = 0.0;
    s.last_shot = None;
    s
}

fn scales(sim: &Sim) -> SpreadScales {
    *sim.app.world().resource::<SpreadScales>()
}

/// The first kick (view rise, degrees) of the M4A1 with `parts`, from
/// its fitted recoil part.
fn first_kick(parts: &[&str]) -> f32 {
    let mut sim = sim();
    let (_, w) = holding(&mut sim, RIFLE, parts);
    let r = sim.app.world().get::<CaRecoil>(w).unwrap().clone();
    r.kick(&mut CaKick::default(), false, 0.0).to_degrees()
}

/// The view's (pitch, yaw) kick after each of `rounds` held rounds of the
/// M4A1 without return, with `parts` (degrees).
fn kicks(parts: &[&str], rounds: usize) -> Vec<(f32, f32)> {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, "test:m4a1_no_return", parts);
    sim.intent(p).fire = true;
    let mut out = Vec::new();
    let mut clip = mag(&sim, w).clip;
    while out.len() < rounds {
        sim.ticks(1);
        let now = mag(&sim, w).clip;
        if now < clip {
            clip = now;
            let punch = sim.app.world().get::<ViewPunch>(p).map_or(Vec3::ZERO, |p| p.0);
            out.push((punch.x.to_degrees(), punch.y.to_degrees()));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Fitting

/// 1. M4A1 + Silencer_A_AR: in the muzzle slot; 31.5 at d = 100.
#[test]
fn t01_fitting_accepted() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", &["Silencer_A_AR"], 100.0);
    let w = sim.app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
    assert_eq!(fitted(&sim, w)[at::MUZZLE], name("Silencer_A_AR"));
    place_at(&mut sim, shooter, target, "Torso", 100.0);
    close(shoot_at(&mut sim, shooter, target, "Torso"), 31.5, 1e-3, "damage");
}

/// 2. M4A1 + Silencer_A_SR (not on its list): the muzzle slot empty;
/// damage 35, sound radius 2000, first kick 1.300.
#[test]
fn t02_fitting_not_listed() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", &["Silencer_A_SR"], 100.0);
    let w = sim.app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
    assert_eq!(fitted(&sim, w), [None, None, None, None, None]);
    assert!(sim.app.world().get::<CaFitted>(w).unwrap().given);
    place_at(&mut sim, shooter, target, "Torso", 100.0);
    close(shoot_at(&mut sim, shooter, target, "Torso"), 35.0, 1e-3, "damage");
    assert_eq!(mods(&sim, w).fire_sound(), FireSound { silenced: None, radius: 2000 });
    close(first_kick(&["Silencer_A_SR"]), 1.300, 1e-3, "first kick");
}

/// 3. "Sniper Silencer" (Integrated 0) on a weapon that lists it: not
/// fitted.
#[test]
fn t03_fitting_not_integrated() {
    let mut sim = sim();
    let (_, w) = holding(&mut sim, "test:m4a1_sniper_silencer", &["Sniper Silencer"]);
    assert_eq!(fitted(&sim, w)[at::MUZZLE], None);
}

/// 4. Default fill: the L96A1 (the M24's default too) with no chosen sight
/// holds "Sniper Scope" and zooms to 20; an event weapon's four defaults
/// fill their slots; a chosen muzzle part it lists replaces the default
/// muzzle only.
#[test]
fn t04_default_fill() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, L96A1, &[]);
    assert_eq!(fitted(&sim, w)[at::SIGHT], name("Sniper Scope"));
    close(zoom_step(&mut sim, p, w), 20.0, 1e-3, "zoomed FOV");

    let mut sim = self::sim();
    let (_, w) = holding(&mut sim, "test:m4a1_event", &[]);
    assert_eq!(
        fitted(&sim, w),
        [
            name("Silencer_B_AR"),
            None,
            name("Magazine_A_AR"),
            name("Bullet_FT_I_AR"),
            name("MechSight_Aim_I_AR")
        ]
    );

    let mut sim = self::sim();
    let (_, w) = holding(&mut sim, "test:m4a1_event", &["Silencer_A_AR"]);
    assert_eq!(
        fitted(&sim, w),
        [
            name("Silencer_A_AR"),
            None,
            name("Magazine_A_AR"),
            name("Bullet_FT_I_AR"),
            name("MechSight_Aim_I_AR")
        ]
    );
}

/// 5. Silencer_A_AR then Silencer_B_AR: B fitted; first kick 1.300, the
/// side kick from shot 2.
#[test]
fn t05_same_slot_twice() {
    let mut sim = sim();
    let (_, w) = holding(&mut sim, RIFLE, &["Silencer_A_AR", "Silencer_B_AR"]);
    assert_eq!(fitted(&sim, w)[at::MUZZLE], name("Silencer_B_AR"));
    let k = kicks(&["Silencer_A_AR", "Silencer_B_AR"], 2);
    close(k[0].0, 1.300, 1e-3, "first kick");
    close(k[0].1, 0.0, 0.0, "no side kick on shot 1");
    close(k[1].1.abs(), 1.2, 1e-3, "side kick on shot 2");
}

// ---------------------------------------------------------------------------
// Damage

/// 6. Silencer_A_AR by range: 31.5 at 100, 22.05 at 2800, 11.025 at 3200;
/// the head at 100: 189.
#[test]
fn t06_silencer_damage_by_range() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", &["Silencer_A_AR"], 100.0);
    for (d, want) in [(100.0, 31.5), (2800.0, 22.05), (3200.0, 11.025)] {
        place_at(&mut sim, shooter, target, "Torso", d);
        close(shoot_at(&mut sim, shooter, target, "Torso"), want, 1e-3, &format!("at {d}"));
    }
    place_at(&mut sim, shooter, target, "Face_Eyelid1", 100.0);
    close(shoot_at(&mut sim, shooter, target, "Face_Eyelid1"), 189.0, 1e-3, "head at 100");
}

/// 7. Hits to kill (unarmoured torso, d <= 2500): Silencer_A_AR 4 (68.5,
/// 37, 5.5, dead); Silencer_A_AR_Red 36.75 a hit, 3.
#[test]
fn t07_hits_to_kill() {
    for (part, steps) in [
        ("Silencer_A_AR", &[68.5, 37.0, 5.5, 0.0][..]),
        ("Silencer_A_AR_Red", &[63.25, 26.5, 0.0][..]),
    ] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", &[part], 1000.0);
        sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 1.0;
        place_at(&mut sim, shooter, target, "Torso", 1000.0);
        for (i, want) in steps.iter().enumerate() {
            shoot_at(&mut sim, shooter, target, "Torso");
            close(health(&sim, target) / HP, *want, 1e-3, &format!("{part}, hit {}", i + 1));
        }
    }
}

/// 8. Bullet_PVP_Damage_III_AR: 42.7 against a player, 35 against a human
/// AI.
#[test]
fn t08_target_group_damage() {
    let id = "test:m4a1_exact";
    close(chest_damage(id, &["Bullet_PVP_Damage_III_AR"], 100.0, as_player), 42.7, 1e-3, "player");
    close(chest_damage(id, &["Bullet_PVP_Damage_III_AR"], 100.0, as_human_ai), 35.0, 1e-3, "human AI");
}

/// 9. Silencer_A_AR + Bullet_PVP_Damage_III_AR against a player: 35 x 0.9
/// x 1.22 = 38.43.
#[test]
fn t09_stacked_damage() {
    let parts = ["Silencer_A_AR", "Bullet_PVP_Damage_III_AR"];
    close(chest_damage("test:m4a1_exact", &parts, 100.0, as_player), 38.43, 1e-3, "stacked");
}

/// 10. MechSight_Aim_I_AR: 33.95 against a player and an AI.
#[test]
fn t10_mechanical_sight_cost() {
    let id = "test:m4a1_exact";
    close(chest_damage(id, &["MechSight_Aim_I_AR"], 100.0, as_player), 33.95, 1e-3, "player");
    close(chest_damage(id, &["MechSight_Aim_I_AR"], 100.0, as_human_ai), 33.95, 1e-3, "AI");
}

/// 11. Bullet_FT_I_AR: 105 against a zombie AI, 35 against a player.
#[test]
fn t11_nz_only_bonus() {
    let id = "test:m4a1_exact";
    close(chest_damage(id, &["Bullet_FT_I_AR"], 100.0, as_zombie_ai), 105.0, 1e-3, "zombie AI");
    close(chest_damage(id, &["Bullet_FT_I_AR"], 100.0, as_player), 35.0, 1e-3, "player");
}

// ---------------------------------------------------------------------------
// Ray length

/// 12. Bullet_FT_I_AR: the client ray 4375; a hit at 3600 deals 35 (past
/// the last band); a target at 4400 isn't hit.
#[test]
fn t12_bullet_range_client() {
    let mut sim = sim();
    let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", &["Bullet_FT_I_AR"], 3600.0);
    let w = sim.app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
    assert_eq!(mods(&sim, w).damage().client_range(3500.0), 4375.0);
    place_at(&mut sim, shooter, target, "Torso", 3600.0);
    close(shoot_at(&mut sim, shooter, target, "Torso"), 35.0, 1e-3, "at 3600");
    place_at(&mut sim, shooter, target, "Torso", 4400.0);
    close(shoot_at(&mut sim, shooter, target, "Torso"), 0.0, 0.0, "at 4400");
}

/// A static panel `units` thick across the lane at z.
fn panel(sim: &mut Sim, z: f32, units: f32, surface: &str) {
    sim.app.world_mut().spawn((
        Transform::from_xyz(30.0, 1.5, z),
        RigidBody::Static,
        Collider::cuboid(3.0, 3.0, units * U),
        PropSurface(surface.into()),
    ));
    sim.ticks(1);
}

/// 13. Bullet_FT_I_AR, 6 units of wood, then a torso at 3800: no damage
/// (the server's own ray is 3500); without the panel the shooter's hit at
/// 3800 deals 35.
#[test]
fn t13_bullet_range_server_ray() {
    for (wood, want) in [(true, 0.0), (false, 35.0)] {
        let mut sim = sim();
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", &["Bullet_FT_I_AR"], 3800.0);
        place_at(&mut sim, shooter, target, "Torso", 3800.0);
        if wood {
            panel(&mut sim, 32.0, 6.0, "Wood");
        }
        close(shoot_at(&mut sim, shooter, target, "Torso"), want, 1e-3, &format!("wood {wood}"));
    }
}

/// 14. Bullet_QR_I_AR: the client ray 35000; Bullet_QR_II_AR (the
/// misspelled key): 3500.
#[test]
fn t14_range_x10_and_the_dead_key() {
    for (part, want) in [("Bullet_QR_I_AR", 35000.0), ("Bullet_QR_II_AR", 3500.0)] {
        let mut sim = sim();
        let (_, w) = holding(&mut sim, RIFLE, &[part]);
        assert_eq!(fitted(&sim, w)[at::BULLET], name(part));
        assert_eq!(mods(&sim, w).damage().client_range(3500.0), want, "{part}");
    }
}

// ---------------------------------------------------------------------------
// Magazines and rates

/// 15. Magazine_A_AR: clip 40, total 120 at spawn; 28 fired: 12/92; a
/// reload: 40/92.
#[test]
fn t15_extended_magazine() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, "test:m4a1_exact", &["Magazine_A_AR"]);
    let m = mag(&sim, w);
    assert_eq!((m.size, m.clip, m.clip + m.reserve), (40, 40, 120));
    sim.intent(p).fire = true;
    for _ in 0..1000 {
        sim.ticks(1);
        if mag(&sim, w).clip <= 12 {
            break;
        }
    }
    sim.intent(p).fire = false;
    // The last fire cycle ends before the reload request.
    sim.seconds(0.2);
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.clip + m.reserve), (12, 92));
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
    sim.seconds(2.7);
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.clip + m.reserve), (40, 92));
}

/// 16. Magazine_C_AR: clip 24; Magazine_D_AR: 21; Magazine_B_AR_Red: 70,
/// total still 120.
#[test]
fn t16_short_magazines() {
    for (part, size) in [("Magazine_C_AR", 24), ("Magazine_D_AR", 21), ("Magazine_B_AR_Red", 70)] {
        let mut sim = sim();
        let (_, w) = holding(&mut sim, RIFLE, &[part]);
        let m = mag(&sim, w);
        assert_eq!((m.size, m.clip, m.clip + m.reserve), (size, size, 120), "{part}");
    }
}

/// 17. Columns: the M24 (clip 7) + Magazine_A_SR (column 4): 12; +
/// Magazine_C_SR (column 1): 6; the L96A1 (clip 5) + Magazine_C_SR
/// (column 0): 4.
#[test]
fn t17_magazine_column() {
    for (id, part, size) in [("test:m24", "Magazine_A_SR", 12), ("test:m24", "Magazine_C_SR", 6), (L96A1, "Magazine_C_SR", 4)] {
        let mut sim = sim();
        let (_, w) = holding(&mut sim, id, &[part]);
        assert_eq!(fitted(&sim, w)[at::MAGAZINE], name(part));
        assert_eq!(mag(&sim, w).size, size, "{id} + {part}");
    }
}

/// 18. Reload lengths from the M4A1's 2300 ms: A 2555.6, B 2875, C 1916.7,
/// D 1769.2, D Red 1437.5 ms; a reload with A takes its length.
#[test]
fn t18_reload_time() {
    for (part, ms) in [
        ("Magazine_A_AR", 2555.6),
        ("Magazine_B_AR", 2875.0),
        ("Magazine_C_AR", 1916.7),
        ("Magazine_D_AR", 1769.2),
        ("Magazine_D_AR_Red", 1437.5),
    ] {
        let mut sim = sim();
        let (p, w) = holding(&mut sim, "test:m4a1_exact", &[part]);
        close(mag(&sim, w).reload_time * 1000.0, ms, 0.1, part);
        if part == "Magazine_A_AR" {
            // One round out, then a reload: the clip full after its length.
            set_mag(&mut sim, w, 39, 80);
            sim.intent(p).reload = true;
            sim.ticks(1);
            sim.intent(p).reload = false;
            // The reload starts on the request's tick.
            let start = sim.app.world().resource::<SimClock>().now;
            let mut done = None;
            for _ in 0..400 {
                sim.ticks(1);
                if mag(&sim, w).clip == 40 {
                    done = Some(sim.app.world().resource::<SimClock>().now);
                    break;
                }
            }
            let took = done.expect("reloaded") - start;
            assert!((took - 2.5556).abs() <= TICK_INTERVAL + 1e-6, "reload took {took} s");
        }
    }
}

/// 19. A rate field in a stat-only slot does nothing: a test bullet with
/// `Reloadanirate` +0.5, the reload still 2300 ms.
#[test]
fn t19_stat_only_rates_ignored() {
    let mut sim = sim();
    sim.app.world_mut().resource_mut::<CaModTable>().insert(ModRecord {
        name: "Test_Rate_Bullet".into(),
        kinds: Some(at::BULLET as i32),
        integrated: 1,
        reload_ani_rate: 0.5,
        ..default()
    });
    let (_, w) = holding(&mut sim, "test:m4a1_rate_bullet", &["Test_Rate_Bullet"]);
    assert_eq!(fitted(&sim, w)[at::BULLET], name("Test_Rate_Bullet"));
    close(mag(&sim, w).reload_time * 1000.0, 2300.0, 0.1, "reload");
}

// ---------------------------------------------------------------------------
// Recoil

/// 20. Silencer_A_AR, held with no return: the view rises 1.170, 1.150,
/// 1.131, 1.111, 1.092, 1.074, 1.056, 1.038, 1.020, 0.158, 0; 10.000 in
/// all.
#[test]
fn t20_silencer_recoil() {
    let k = kicks(&["Silencer_A_AR"], 11);
    let want = [1.170, 1.150, 1.131, 1.111, 1.092, 1.074, 1.056, 1.038, 1.020, 0.158, 0.0];
    let mut last = 0.0;
    for (i, w) in want.iter().enumerate() {
        close(k[i].0 - last, *w, 1e-3, &format!("round {}", i + 1));
        last = k[i].0;
    }
    close(k[10].0, 10.0, 1e-3, "in all");
}

/// 21. Last wins (Q1), the first kick: Silencer_A_AR + Magazine_A_AR
/// 1.300; + DotSight_A_AR_Red 1.261; + MechSight_Aim_II_AR 1.300;
/// Silencer_A_AR_Red alone 1.040.
#[test]
fn t21_recoil_last_wins() {
    for (parts, want) in [
        (&["Silencer_A_AR", "Magazine_A_AR"][..], 1.300),
        (&["Silencer_A_AR", "DotSight_A_AR_Red"][..], 1.261),
        (&["Silencer_A_AR", "MechSight_Aim_II_AR"][..], 1.300),
        (&["Silencer_A_AR_Red"][..], 1.040),
    ] {
        close(first_kick(parts), want, 1e-3, &format!("{parts:?}"));
    }
}

/// 22. Silencer_A_AR from J = 0, one shot: up 1.170 and sideways exactly
/// 1.2 (Q11); Silencer_B_AR: no side kick on shot 1.
#[test]
fn t22_side_kick_from_shot_1() {
    let k = kicks(&["Silencer_A_AR"], 1);
    close(k[0].0, 1.170, 1e-3, "up");
    close(k[0].1.abs(), 1.2, 1e-3, "sideways");
    let k = kicks(&["Silencer_B_AR"], 1);
    close(k[0].1, 0.0, 0.0, "no side kick");
}

// ---------------------------------------------------------------------------
// Spread

/// Firing-meter recovery from F = 0.30 in `mode`: the first step's
/// change and how many sub-steps until the cone is `n`.
fn recovery(parts: &[&str], mode: FireMode, n: f32) -> (f32, u32) {
    let mut sim = sim();
    let (_, w) = holding(&mut sim, RIFLE, parts);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let cone = s.cone_params(mode);
    s.fire = 0.30;
    s.last_shot = Some(-1.0);
    s.fire_substep(&sc, cone, false, 0.30, 0.0);
    let first = s.fire - 0.30;
    let mut steps = 1;
    while s.cone(mode) > n {
        steps += 1;
        s.fire_substep(&sc, cone, false, 0.30, steps as f64 * ca::SPREAD_SUBSTEP);
        assert!(steps < 100);
    }
    (first, steps)
}

/// 23. DotSight_ITECH_AR, not zoomed: -0.015 a sub-step, the cone back to
/// 3 after 20 (stock 26); MechSight_Aim_III_AR: 17.
#[test]
fn t23_recovery_with_a_red_dot() {
    let (step, n) = recovery(&["DotSight_ITECH_AR"], FireMode::Normal, 3.0);
    close(step, -0.015, 1e-5, "a sub-step");
    assert_eq!(n, 20, "ITECH");
    assert_eq!(recovery(&[], FireMode::Normal, 3.0).1, 26, "stock");
    assert_eq!(recovery(&["MechSight_Aim_III_AR"], FireMode::Normal, 3.0).1, 17, "Aim III");
}

/// 24. MechSight_Aim_III_AR, M = 0.25 to standing still: 0.19825, 0.1465,
/// 0.09475, 0.05116, 0.02763, 0.01492, 0.00806; the cone 3 at sub-step 7
/// (stock 11).
#[test]
fn t24_movement_recovery() {
    let mut sim = sim();
    let (_, w) = holding(&mut sim, RIFLE, &["MechSight_Aim_III_AR"]);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let (cone, st) = (s.cone_params(FireMode::Normal), s.states(FireMode::Normal));
    s.movement = 0.25;
    let want = [0.19825, 0.1465, 0.09475, 0.05116, 0.02763, 0.01492, 0.00806];
    for (i, want) in want.iter().enumerate() {
        s.move_substep(&sc, cone, st.run, 0.0, 0.0, false, st.jump);
        close(s.movement, *want, 1e-5, &format!("sub-step {}", i + 1));
        if i == 5 {
            assert!(s.cone(FireMode::Normal) > 3.0, "not yet at 6");
        }
    }
    close(s.cone(FireMode::Normal), 3.0, 0.0, "at 7");
}

/// 25. Nothing in the sight slot: zoom does nothing (FOV 90, no overlay,
/// so the crosshair stays), the cone stays 3..100.
#[test]
fn t25_no_sight_no_zoom() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, RIFLE, &[]);
    press_zoom(&mut sim, p);
    sim.ticks(10);
    let z = cz(&sim, w);
    assert_eq!((z.level, z.fov, z.zoomed()), (0, 90.0, false));
    assert!(sim.app.world().get::<Zoomed>(p).is_none(), "no overlay");
    let s = spread(&sim, w);
    assert_eq!(s.cone(FireMode::Normal), 3.0);
    let mut full = s.clone();
    full.fire = 1.0;
    assert_eq!(full.cone(FireMode::Normal), 100.0);
}

/// The cone the frame shows for the active weapon (thousandths).
fn shown_cone(sim: &Sim, w: Entity) -> i32 {
    let SpreadShape::Disc { inaccuracy, .. } = sim.app.world().get::<Hitscan>(w).unwrap().spread else {
        panic!("a disc")
    };
    (inaccuracy * 1000.0).round() as i32
}

/// 26. DotSight_A_AR, zoomed, standing, both meters 0, holding fire with
/// no recovery: cones 2, 3, 4, 6, 7, 8, 10, 10.
#[test]
fn t26_zoomed_cone_through_a_red_dot() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, "test:m4a1_steady", &["DotSight_A_AR"]);
    close(zoom_step(&mut sim, p, w), 50.0, 1e-3, "zoomed");
    sim.intent(p).fire = true;
    let mut cones = Vec::new();
    let mut clip = mag(&sim, w).clip;
    while cones.len() < 8 {
        let cone = shown_cone(&sim, w);
        sim.ticks(1);
        let now = mag(&sim, w).clip;
        if now < clip {
            cones.push(cone);
            clip = now;
        }
    }
    assert_eq!(cones, [2, 3, 4, 6, 7, 8, 10, 10]);
}

/// 27. DotSight_ITECH_AR zoomed: +0.049875 a shot (Q4); from 0.30,
/// -0.016125 a sub-step, the cone 2 after 17; DotSight_A_AR zoomed:
/// -0.01125, 24.
#[test]
fn t27_zoomed_recovery_with_itech() {
    let mut sim = sim();
    let (_, w) = holding(&mut sim, RIFLE, &["DotSight_ITECH_AR"]);
    let (sc, mut s) = (scales(&sim), spread(&sim, w));
    let (cone, st) = (s.cone_params(FireMode::Zoomed), s.states(FireMode::Zoomed));
    s.fire_substep(&sc, cone, true, st.fire, 0.0);
    close(s.fire, 0.049875, 1e-6, "growth a shot");
    let (step, n) = recovery(&["DotSight_ITECH_AR"], FireMode::Zoomed, 2.0);
    close(step, -0.016125, 1e-5, "ITECH sub-step");
    assert_eq!(n, 17, "ITECH");
    let (step, n) = recovery(&["DotSight_A_AR"], FireMode::Zoomed, 2.0);
    close(step, -0.01125, 1e-5, "A sub-step");
    assert_eq!(n, 24, "A");
}

// ---------------------------------------------------------------------------
// Zoom

/// 28. Zoom steps: the Barrett 30, 10, 90; the L96A1's Sniper Scope (the
/// M24's) 20, 90; M4A1 + DotSight_ITECH_AR 75, 90.
#[test]
fn t28_zoom_steps() {
    for (id, parts, want) in [
        ("test:barrett", &[][..], &[30.0, 10.0, 90.0][..]),
        (L96A1, &[][..], &[20.0, 90.0][..]),
        (RIFLE, &["DotSight_ITECH_AR"][..], &[75.0, 90.0][..]),
    ] {
        let mut sim = sim();
        let (p, w) = holding(&mut sim, id, parts);
        for f in want {
            close(zoom_step(&mut sim, p, w), *f, 1e-3, &format!("{id}"));
        }
    }
}

/// 29. M4A1 + DotSight_A_AR: in 90 to 50 in 0.150 s, out 50 to 90 in
/// 0.0857 s; the weapon hidden (the overlay up) from the press until the
/// zoom-out ends.
#[test]
fn t29_zoom_timing() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, RIFLE, &["DotSight_A_AR"]);
    let overlay = |sim: &Sim| sim.app.world().get::<Zoomed>(p).is_some_and(|z| z.scope);
    for (target, secs) in [(50.0, 0.150), (90.0, 0.0857)] {
        press_zoom(&mut sim, p);
        let mut ticks = 1;
        while cz(&sim, w).transition.is_some() {
            assert!(overlay(&sim), "hidden while moving");
            sim.ticks(1);
            ticks += 1;
        }
        close(cz(&sim, w).fov, target, 1e-3, "arrived");
        let took = ticks as f64 * TICK_INTERVAL;
        assert!(took >= secs - 1e-6 && took <= secs + TICK_INTERVAL + 1e-6, "{target}: {took} s, expected {secs}");
        assert_eq!(overlay(&sim), target == 50.0, "the overlay after reaching {target}");
    }
}

// ---------------------------------------------------------------------------
// Sound and models

/// 32. Silenced sound: the M4A1 with any silencer: silenced sound 1 at
/// 1400 (stock 2000); the AK-47 1600; others hear the silenced sound and
/// see no muzzle light.
#[test]
fn t32_silenced_sound() {
    for part in ["Silencer_A_AR", "Silencer_B_AR_Gold", "Silencer_C_AR_Red"] {
        let mut sim = sim();
        let (_, w) = holding(&mut sim, RIFLE, &[part]);
        let m = mods(&sim, w);
        assert_eq!(m.fire_sound(), FireSound { silenced: Some(1), radius: 1400 }, "{part}");
        assert_eq!(m.observer_sound(), ObserverSound::Silenced { level: 1 });
        assert!(!m.muzzle_light());
    }
    let mut sim = sim();
    let (_, w) = holding(&mut sim, RIFLE, &[]);
    assert_eq!(mods(&sim, w).fire_sound(), FireSound { silenced: None, radius: 2000 });
    assert!(mods(&sim, w).muzzle_light());
    let mut sim = self::sim();
    let (_, w) = holding(&mut sim, "test:ak47", &["Silencer_A_AR"]);
    assert_eq!(mods(&sim, w).fire_sound(), FireSound { silenced: Some(1), radius: 1600 });
}

/// 33. Part models: M4A1 + DotSight_A_AR: variant 0 at the breach socket;
/// AK-47: variant 2; M4A1 + Magazine_A_AR: none.
#[test]
fn t33_part_model_variant() {
    let breach = |variant| PartModel {
        slot: at::SIGHT,
        name: "DotSight_A_AR".into(),
        socket: "breach".into(),
        variant,
        visible: true,
    };
    let mut sim = sim();
    let (_, w) = holding(&mut sim, RIFLE, &["DotSight_A_AR"]);
    assert_eq!(mods(&sim, w).part_models(), [breach(0)]);
    let mut sim = self::sim();
    let (_, w) = holding(&mut sim, "test:ak47", &["DotSight_A_AR"]);
    assert_eq!(mods(&sim, w).part_models(), [breach(2)]);
    let mut sim = self::sim();
    let (_, w) = holding(&mut sim, RIFLE, &["Magazine_A_AR"]);
    assert_eq!(mods(&sim, w).part_models(), []);
}

// ---------------------------------------------------------------------------
// Pickup and stun

/// 34. A dropped M4A1 with Magazine_A_AR, clip 37, total 97: the picker
/// has 37 / 97 with a 40-round clip; after a reload 40 / 97.
#[test]
fn t34_picked_up_weapon() {
    let mut sim = sim();
    let dropper = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0, "test:m4a1_exact", &["Magazine_A_AR"]);
    let w = arm(&mut sim, dropper, "test:m4a1_exact");
    set_mag(&mut sim, w, 37, 60);
    let picker = player(&mut sim, Vec3::new(32.0, 1.0, 30.0), 0.0, 1.0, "test:m4a1_exact", &[]);
    // A primary drawn since spawning: the pickup swaps with it (inventory.md
    // Corrections 2026-10-22 2).
    arm(&mut sim, picker, "test:m4a1_exact");
    inventory::drop_item(sim.app.world_mut(), dropper, w, false).unwrap();
    let item = sim
        .app
        .world_mut()
        .query::<(Entity, &Dropped)>()
        .iter(sim.app.world())
        .find(|(_, d)| d.weapon == w)
        .map(|(e, _)| e)
        .unwrap();
    assert!(inventory::pick_up(sim.app.world_mut(), picker, item));
    sim.seconds(1.5);
    assert_eq!(sim.app.world().get::<Inventory>(picker).unwrap().active, Some(w));
    assert_eq!(fitted(&sim, w)[at::MAGAZINE], name("Magazine_A_AR"));
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.clip + m.reserve, m.size), (37, 97, 40));
    sim.intent(picker).reload = true;
    sim.ticks(1);
    sim.intent(picker).reload = false;
    sim.seconds(2.7);
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.clip + m.reserve), (40, 97));
}

fn speed_limit(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<CaMovement>(p).unwrap().speed_limit
}

/// 35. Bullet_PVP_Stun_I_AR against a player: 31.5; a draw <= 0.5 slows
/// the victim by the stun penalty 0.20 for 1.0 s (P_stun); 0.6 doesn't.
#[test]
fn t35_stun_bullet() {
    for (draw, stunned) in [(0.5, true), (0.6, false)] {
        let mut sim = sim();
        sim.app.insert_resource(CaSpecialRoll(Some(draw)));
        let (shooter, target) = range_lane(&mut sim, "test:m4a1_exact", &["Bullet_PVP_Stun_I_AR"], 100.0);
        // From another team (the stun's condition).
        sim.app.world_mut().entity_mut(target).insert(Team(1));
        place_at(&mut sim, shooter, target, "Torso", 100.0);
        let before = speed_limit(&sim, target);
        let chest = sphere(&sim, shooter, target, "Torso");
        aim(&mut sim, shooter, chest);
        let hp = health(&sim, target);
        sim.intent(shooter).fire = true;
        sim.ticks(1);
        sim.intent(shooter).fire = false;
        // The round on the 3rd update after the press (weapon_firing.md
        // Corrections 10).
        sim.ticks(3);
        let hit_at = sim.app.world().resource::<SimClock>().now;
        sim.ticks(1);
        close((hp - health(&sim, target)) / HP, 31.5, 1e-3, &format!("damage, draw {draw}"));
        let stun = sim.app.world().get::<StunPenalty>(target).copied();
        if !stunned {
            assert_eq!(stun, None, "no stun at {draw}");
            continue;
        }
        let stun = stun.expect("stunned");
        close(stun.penalty, 0.20, 1e-6, "penalty");
        close((stun.until - hit_at) as f32, 1.0, 1e-4, "duration");
        let slowed = speed_limit(&sim, target);
        assert!(slowed < before, "slowed: {slowed} vs {before}");
        // Back after the second.
        sim.seconds(1.1);
        close(speed_limit(&sim, target), before, 1e-4, "after 1 s");
        // The penalty adds into movement's sum: twice the stun, twice the
        // slowdown.
        let now = sim.app.world().resource::<SimClock>().now;
        sim.app.world_mut().entity_mut(target).insert(StunPenalty {
            penalty: 0.40,
            until: now + 10.0,
        });
        sim.ticks(1);
        close(before - speed_limit(&sim, target), 2.0 * (before - slowed), 1e-3, "linear in P_stun");
    }
}

// ---------------------------------------------------------------------------
// Records

/// 36. A weapon with no accepted list (the M60's is empty, ours for a
/// missing record, Q2) is given normally; every part offered is rejected.
#[test]
fn t36_missing_record() {
    let mut sim = sim();
    let (_, w) = holding(&mut sim, M60, &["Silencer_A_AR", "Magazine_A_AR"]);
    assert_eq!(fitted(&sim, w), [None, None, None, None, None]);
    assert_eq!(mag(&sim, w).size, sim.app.world().get::<at::CaAttachSlots>(w).unwrap().stock.clip);
}

/// Our M4A1 document renamed to `name` (an install weapon's slug).
fn our_doc(name: &str) -> String {
    include_str!("../../data/weapons/m4a1.toml").replace("combat_arms:m4a1", &format!("combat_arms:{name}"))
}

/// The accepted list a merged document has.
fn merged_accepts(ours: &str, weapons: &str, mods: &str) -> Vec<toml::Value> {
    let w = WeaponsFile::read(&Document::parse(weapons.as_bytes()).unwrap()).unwrap();
    let m = ModFile::read(&Document::parse(mods.as_bytes()).unwrap());
    let docs = merge_weapon_docs_with_mods(&[("ours.toml", ours)], &w, Some(&m), 0.018).unwrap();
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    // It still loads.
    chimera_sim::weapon::data::WeaponSet::parse(&refs, &[]).unwrap();
    let t: toml::Table = docs[0].1.parse().unwrap();
    t["hooks"]["combat_arms:attachments"]["accepts"].as_array().unwrap().clone()
}

const INSTALL_WEAPONS: &str = r#"[Ammo0] Name="a" [Weapon0] Name="Other" AmmoName0="a" [Weapon1] Name="M6A2_Event" AmmoName0="a""#;

/// 36 (the install's file): the modification file one record short: the
/// last weapon accepts nothing.
#[test]
fn t36_missing_record_in_the_install() {
    let mods = r#"[Mod0] Name="Silencer_A_AR" Kinds=0 Type=0 Integrated=1 [Weapon0] Name="Other" ModName0="Silencer_A_AR""#;
    assert!(merged_accepts(&our_doc("m6a2_event"), INSTALL_WEAPONS, mods).is_empty());
}

/// 37. Link by position (Q13): weapon 1 is "M6A2_Event", modification
/// record 1 "M6A2_(Event)": that record's list is the weapon's (an unknown
/// name dropped with its variant index).
#[test]
fn t37_link_by_position() {
    let mods = r#"
        [Mod0] Name="Silencer_A_AR" Kinds=0 Type=0 Integrated=1
        [Mod1] Name="DotSight_A_AR" Kinds=1 Type=1 Integrated=1 ZoomLevel=1 ZoomFov0=50.0
        [Weapon0] Name="Other" ModName0="DotSight_A_AR"
        [Weapon1] Name="M6A2_(Event)" ModName0="Silencer_A_AR" ModName1="nothing" ModModel1=7 ModName2="DotSight_A_AR" ModModel2=2
    "#;
    let accepts = merged_accepts(&our_doc("m6a2_event"), INSTALL_WEAPONS, mods);
    let got: Vec<(String, i64)> = accepts
        .iter()
        .map(|a| (a["name"].as_str().unwrap().to_string(), a["variant"].as_integer().unwrap()))
        .collect();
    assert_eq!(got, [("Silencer_A_AR".to_string(), 0), ("DotSight_A_AR".to_string(), 2)]);
}

/// 28 (the install's data). A machine gun the install fits a sight (its
/// record's `DefaultScopeAttach`, the sight on its accepted list) zooms,
/// though our M60 document has no zoom table (every sight zooms,
/// "Sights"); and a record without the field leaves the weapon unsighted
/// (the install's record decides the fitted sight).
#[test]
fn t28_install_sight_on_a_machine_gun_zooms() {
    use combat_arms::{
        game::{data::{CaData, InstallAttributes}, weapons::zoom::CaZoom},
        harness::CaSimWith,
    };
    let mods = r#"
        [Mod0] Name="DotSight_A_AR" Kinds=1 Type=1 Integrated=1 ZoomLevel=1 ZoomFov0=50.0
        [Weapon0] Name="M60" ModName0="DotSight_A_AR"
    "#;
    for (record, want) in [
        (r#"[Weapon0] Name="M60" AmmoName0="a" Guntype=5 DefaultScopeAttach="DotSight_A_AR""#, Some(vec![50.0])),
        (r#"[Weapon0] Name="M60" AmmoName0="a" Guntype=5"#, None),
    ] {
        let text = format!("[Ammo0] Name=\"a\" {record}");
        let attributes = InstallAttributes {
            weapons: Some(WeaponsFile::read(&Document::parse(text.as_bytes()).unwrap()).unwrap()),
            mods: Some(ModFile::read(&Document::parse(mods.as_bytes()).unwrap())),
            ..default()
        };
        let mut sim = Sim::with(CaSimWith(CaData::with_attributes(attributes)), |app| {
            app.add_plugins(GreyboxMapPlugin);
        });
        sim.set_tick_interval(TICK_INTERVAL);
        let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0, M60, &[]);
        let w = arm(&mut sim, p, M60);
        let scope = sim.app.world().get::<CaZoom>(w).expect("a zoom").scope.clone();
        assert_eq!(scope.map(|s| s.fov), want, "{record}");
    }
}

/// weapon_classes.md Corrections (2026-10-30) 2: a special weapon zooms
/// exactly like a gun through the scope its record names
/// (`DefaultScopeAttach`, the XM25's `XM25 Scope`), with that part's
/// levels; one whose record names none doesn't zoom.
#[test]
fn special_weapon_zooms_through_its_scope() {
    use combat_arms::{
        game::{data::{CaData, InstallAttributes}, weapons::zoom::CaZoom},
        harness::CaSimWith,
    };
    const XM25: &str = "combat_arms:xm_25";
    let mods = r#"
        [Mod0] Name="XM25 Scope" Kinds=1 Type=1 Integrated=1 ZoomLevel=1 ZoomFov0=30.0
        [Weapon0] Name="XM_25" ModName0="XM25 Scope"
    "#;
    for (record, want) in [
        (r#"[Weapon0] Name="XM_25" AmmoName0="a" DefaultScopeAttach="XM25 Scope""#, Some(30.0)),
        (r#"[Weapon0] Name="XM_25" AmmoName0="a""#, None),
    ] {
        let text = format!("[Ammo0] Name=\"a\" {record}");
        let attributes = InstallAttributes {
            weapons: Some(WeaponsFile::read(&Document::parse(text.as_bytes()).unwrap()).unwrap()),
            mods: Some(ModFile::read(&Document::parse(mods.as_bytes()).unwrap())),
            ..default()
        };
        let mut sim = Sim::with(CaSimWith(CaData::with_attributes(attributes)), |app| {
            app.add_plugins(GreyboxMapPlugin);
        });
        sim.set_tick_interval(TICK_INTERVAL);
        let p = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0, XM25, &[]);
        let w = arm(&mut sim, p, XM25);
        let scope = sim.app.world().get::<CaZoom>(w).and_then(|z| z.scope.clone());
        assert_eq!(scope.map(|s| s.fov), want.map(|f| vec![f]), "{record}");
        // The zoom key: the scope's FOV, as a gun's.
        sim.intent(p).secondary = true;
        sim.ticks(1);
        sim.intent(p).secondary = false;
        sim.seconds(0.5);
        assert_eq!(sim.app.world().get::<Zoomed>(p).map(|z| z.fov), want, "{record}: zoomed");
    }
}

// ---------------------------------------------------------------------------
// Network

/// The fitted parts are the server's: a client's own weapon has the same
/// parts and their effects (clip size, the sight's zoom), predicted
/// without error, zooming and firing.
#[test]
fn fitted_parts_are_the_servers_and_predicted() {
    let link = LinkConditions {
        latency: Duration::from_millis(50),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 7, 1, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.server
        .app
        .insert_resource(CaDefaultLoadout(CaLoadout::default().with("m4a1", &["Magazine_A_AR", "DotSight_A_AR"])));
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(240);
    let theirs = sim.character_of(0).unwrap();
    let me = sim.local_player(0).unwrap();
    let held = |w: &World, p: Entity| w.get::<Inventory>(p).unwrap().active.unwrap();
    let (server, client) = (sim.server.app.world(), sim.clients[0].app.world());
    let (sw, cw) = (held(server, theirs), held(client, me));
    assert_eq!(client.get::<Weapon>(cw).unwrap().id, RIFLE);
    assert_eq!(client.get::<CaFitted>(cw), server.get::<CaFitted>(sw), "the server's parts");
    assert_eq!(client.get::<Magazine>(cw).unwrap().size, 40, "the client's clip size");
    assert_eq!(client.get::<CaZoom>(cw).unwrap().scope.as_ref().map(|s| s.fov.clone()), Some(vec![50.0]));
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    // Zoom in, fire a few rounds, zoom out.
    let set = |sim: &mut NetSim, f: &dyn Fn(&mut Intent)| {
        let me = sim.local_player(0).unwrap();
        f(&mut sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap());
    };
    set(&mut sim, &|i| i.secondary = true);
    sim.step();
    set(&mut sim, &|i| i.secondary = false);
    sim.ticks(20);
    let me = sim.local_player(0).unwrap();
    assert_eq!(sim.clients[0].app.world().get::<Zoomed>(me).map(|z| z.fov), Some(50.0), "zoomed on the client");
    set(&mut sim, &|i| i.fire = true);
    sim.ticks(30);
    set(&mut sim, &|i| i.fire = false);
    set(&mut sim, &|i| i.secondary = true);
    sim.step();
    set(&mut sim, &|i| i.secondary = false);
    sim.ticks(60);
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    println!("attachments: {} states checked, {} errors ({:?})", g.checked, g.errors, g.last_differing);
    assert!(g.checked > 0);
    assert_eq!(g.errors, 0, "mispredicted ({:?})", g.last_differing);
    let clip = |w: &World, e: Entity| w.get::<Magazine>(e).unwrap().clip;
    let (server, client) = (sim.server.app.world(), sim.clients[0].app.world());
    assert!(clip(server, sw) < 40, "fired");
    assert_eq!(clip(client, cw), clip(server, sw));
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-19): 38-40

/// The test parts with `VectorsPerRoundPer` `v`: a magazine and a muzzle
/// part.
fn pellet_parts(sim: &mut Sim, mag: f32, muzzle: f32) {
    let mut table = sim.app.world_mut().resource_mut::<CaModTable>();
    table.insert(ModRecord {
        name: "Test_Pellet_Mag".into(),
        kinds: Some(at::MAGAZINE as i32),
        kind: Some(at::MAGAZINE_TYPE),
        integrated: 1,
        vectors_per_round_per: mag,
        ..default()
    });
    table.insert(ModRecord {
        name: "Test_Pellet_Muzzle".into(),
        kinds: Some(at::MUZZLE as i32),
        kind: Some(at::SILENCER),
        integrated: 1,
        vectors_per_round_per: muzzle,
        ..default()
    });
}

/// 38. A pellet-count mismatch: 8 rays, the magazine's 0.25 alone → the
/// client sends 10, the server expects 8: nothing; the muzzle's alone →
/// 8 sent, 10 expected: nothing; both → 10 rays land (9 each).
#[test]
fn t38_pellet_count_mismatch() {
    for (mag, muzzle, want) in [(0.25, 0.0, 0.0), (0.0, 0.25, 0.0), (0.25, 0.25, 90.0), (0.0, 0.0, 72.0)] {
        let mut sim = sim();
        pellet_parts(&mut sim, mag, muzzle);
        let (shooter, target) = range_lane(&mut sim, "test:r870_pellets", &["Test_Pellet_Mag", "Test_Pellet_Muzzle"], 500.0);
        place_at(&mut sim, shooter, target, "Upper_torso", 500.0);
        close(shoot_at(&mut sim, shooter, target, "Upper_torso"), want, 1e-2, &format!("magazine {mag}, muzzle {muzzle}"));
    }
}

/// 39. The burst return with a muzzle part: `TripleShotMaxCamRecoilPitch`
/// 8 and the muzzle's `MaxCamRecoilPitchPer` 0.25 → a cap of rad 10.
#[test]
fn t39_burst_return_with_a_muzzle_part() {
    let mut r = CaRecoil::new(RecoilParams {
        max_pitch: 5.0,
        burst_max_pitch: 8.0,
        ..default()
    });
    r.mods.return_max = 0.25;
    close(r.return_cap(true), 10f32.to_radians(), 1e-7, "burst");
    close(r.return_cap(false), 6.25f32.to_radians(), 1e-7, "normal");
}

/// 40. A switch after a kick: the M4A1 with a silencer kicks, then the
/// knife in hand: the next return steps with the knife's (the record
/// defaults: no muzzle term) cap and rates.
#[test]
fn t40_switch_after_a_kick() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, RIFLE, &["Silencer_A_AR"]);
    assert!(sim.app.world().get::<CaRecoil>(w).unwrap().mods != default(), "the silencer's terms");
    sim.intent(p).fire = true;
    sim.ticks(4);
    sim.intent(p).fire = false;
    let knife = sim
        .app
        .world()
        .get::<Inventory>(p)
        .unwrap()
        .weapons
        .iter()
        .copied()
        .find(|e| sim.app.world().get::<CaRecoil>(*e).is_none() && *e != w)
        .expect("a weapon with no recoil table");
    let slot = sim.app.world().get::<chimera_sim::weapon::Weapon>(knife).unwrap().slot;
    sim.intent(p).select = Some(slot);
    sim.ticks(1);
    sim.intent(p).select = None;
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(knife));
    let mut k = sim.app.world().get::<CaKick>(p).unwrap().clone();
    assert!(k.pitch > 0.0, "kicked");
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now + TICK_INTERVAL;
    let (down, _) = ca::DEFAULT_RECOIL.recover(&mut k, TICK_INTERVAL as f32, now, false);
    let before = sim.app.world().get::<ViewPunch>(p).unwrap().0.x;
    sim.ticks(1);
    let moved = before - sim.app.world().get::<ViewPunch>(p).unwrap().0.x;
    close(moved, down, 1e-6, "the knife's return");
    close(sim.app.world().get::<CaKick>(p).unwrap().pitch, k.pitch, 1e-7, "J");
}

/// Corrections (2026-10-22) 1: an item whose five parts equal the carried
/// primary's is not a swap (the carried weapon stays, its parts too, and
/// takes the item's total and clip); a different sight is the swap, the
/// item's parts arriving with it.
#[test]
fn c1_the_fitted_set_decides_the_swap() {
    for (item_sight, swaps) in [("DotSight_A_AR", false), ("DotSight_B_AR", true)] {
        let mut sim = sim();
        let dropper = player(&mut sim, Vec3::new(30.0, 1.0, 30.0), 0.0, 1.0, "test:m4a1_exact", &["Silencer_A_AR", item_sight]);
        let item_gun = arm(&mut sim, dropper, "test:m4a1_exact");
        set_mag(&mut sim, item_gun, 20, 30);
        let picker = player(&mut sim, Vec3::new(32.0, 1.0, 30.0), 0.0, 1.0, "test:m4a1_exact", &["Silencer_A_AR", "DotSight_A_AR"]);
        let mine = arm(&mut sim, picker, "test:m4a1_exact");
        inventory::drop_item(sim.app.world_mut(), dropper, item_gun, false).unwrap();
        let item = sim
            .app
            .world_mut()
            .query::<(Entity, &Dropped)>()
            .iter(sim.app.world())
            .find(|(_, d)| d.weapon == item_gun)
            .map(|(e, _)| e)
            .unwrap();
        assert!(inventory::pick_up(sim.app.world_mut(), picker, item));
        sim.seconds(1.5);
        let active = sim.app.world().get::<Inventory>(picker).unwrap().active.unwrap();
        let m = mag(&sim, active);
        if swaps {
            assert_eq!(active, item_gun, "{item_sight}: swapped");
            assert_eq!(fitted(&sim, active)[at::SIGHT], name("DotSight_B_AR"), "the item's parts arrive");
            assert_eq!(m.clip, 20);
            assert!(sim.app.world().get_entity(mine).is_ok(), "the old one lies dropped");
        } else {
            assert_eq!(active, mine, "{item_sight}: the carried weapon stays");
            assert_eq!((m.clip, m.clip + m.reserve), (20, 50));
            assert_eq!(fitted(&sim, mine)[at::SIGHT], name("DotSight_A_AR"));
            assert!(sim.app.world().get_entity(item).is_err(), "the item is used up");
        }
    }
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-30): placing part models.

/// 41. A socket at (10, 2, 30) whose local axes are x (0, 0, 1), y (0, 1,
/// 0), z (-1, 0, 0) (yaw +90 degrees), `MuzzlePVPosOff` (-2.2, 0, 0) and no
/// rotation offset -> the part's origin at (10, 2, 27.8), its axes the
/// socket's. In our right-handed frame (z mirrored) the same socket is at
/// (10, 2, -30) with x along -z.
#[test]
fn t41_part_frame() {
    use combat_arms::game::{attachments::PartOffset, view_model::parts};
    let socket = Mat4::from_rotation_translation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), Vec3::new(10.0, 2.0, -30.0));
    assert!((socket.transform_vector3(Vec3::X) - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-5);
    let off = PartOffset { pos: [-2.2, 0.0, 0.0], rot: [0.0; 3] };
    let m = parts::first_person_frame(socket, &off);
    let at = m.transform_point3(Vec3::ZERO);
    assert!((at - Vec3::new(10.0, 2.0, -27.8)).length() < 1e-4, "{at}");
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        assert!((m.transform_vector3(axis) - socket.transform_vector3(axis)).length() < 1e-5, "no turn");
    }
}

/// 42. Socket yaw 0.3, pitch 0.2, roll 0 and `ScopePVRotOff` (0.1, 0, 0)
/// -> yaw 0.4, pitch 0.2, roll 0 (angles added, rebuilt yaw-pitch-roll);
/// the same numbers as `ScopeHHRotOff` -> the socket's rotation times
/// rotation(pitch 0, yaw 0.1, roll 0). The game's angles in our mirrored
/// frame: yaw and pitch negated.
#[test]
fn t42_rotation_offsets() {
    use combat_arms::game::{attachments::PartOffset, view_model::parts};
    let game = |yaw: f32, pitch: f32, roll: f32| Quat::from_euler(EulerRot::YXZ, -yaw, -pitch, roll);
    let socket = game(0.3, 0.2, 0.0);
    let got = parts::added_angles(socket, [0.1, 0.0, 0.0]);
    assert!(got.angle_between(game(0.4, 0.2, 0.0)) < 1e-5, "first person: angles added");
    let off = PartOffset { pos: [0.0; 3], rot: [0.1, 0.0, 0.0] };
    let third = Mat4::from_quat(socket) * parts::offset_matrix(&off);
    let want = socket * game(0.1, 0.0, 0.0);
    assert!(Quat::from_mat4(&third).angle_between(want) < 1e-5, "third person: composed after");
    // The two differ when the socket is pitched.
    assert!(got.angle_between(want) > 1e-3);
}

/// 43. The lens (`RS\TRANSPARENT.LTB`): a texel of alpha 150/255 isn't
/// drawn, 170/255 is, opaque, from both sides.
#[test]
fn t43_lens_cut_out() {
    use chimera_world::map::{MapAlpha, MapMesh};
    use combat_arms::game::render_style::{self, Style};
    let lens = render_style::style("RS\\TRANSPARENT.LTB");
    assert!(matches!(lens, Style::Cutout { .. }), "{lens:?}");
    assert!(!lens.keeps(150) && lens.keeps(170) && !lens.keeps(160) && lens.keeps(161));
    let mut m = MapMesh::default();
    render_style::apply(&mut m, lens);
    let MapAlpha::Mask(cut) = m.alpha else { panic!("{:?}", m.alpha) };
    assert!(150.0 / 255.0 < cut && cut < 170.0 / 255.0);
    assert!(m.double_sided && !m.unlit, "two-sided, lit");
}
