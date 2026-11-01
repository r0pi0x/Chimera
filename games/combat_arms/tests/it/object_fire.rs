//! specs/combat_arms/world_objects.md Follow-up (2026-10-31b) 5, case 51:
//! a destructible's weapon (`FIRE`, `CreateExplosion`) as a real shot
//! (`game::objects::fire`), on `CaSimWith` and an install written here (no
//! file of the game): the object's `WeaponId` is the install's weapon
//! record index; a grenade, a projectile and a gun each fire as they
//! define, with no character credited.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind};
use chimera_sim::weapon::{self, Hitscan, grenade::Detonated};
use chimera_world::harness::Sim;
use combat_arms::{
    game::{
        TICK_INTERVAL,
        data::{
            CaData,
            attributes::{FX_FILE, WEAPONS_FILE},
            read_attributes,
        },
        grenades::{CaGrenade, CaGrenadeData},
        movement,
        objects::{
            CaObjectFire, CaObjects, apply_effects,
            commands::Ctx,
            fire::{self, Shot},
            geometry::from_engine,
            runtime::ObjectWorld,
        },
        special::{CaObject, CaProjectileData},
        weapons::{CaDamage, base_damage},
    },
    harness::CaSimWith,
};

use super::{
    special_weapons::{HP, U, player, solid},
    world_objects::wm,
};

/// Weapon records 0-2: our LAW, M4A1 and M67 by name (merged with ours).
const WEAPONS_TXT: &str = r#"
[Ammo0] Name="LAW" Type=1 MaxAmount=3 AreaDamage=40 AreaDamageRadius=550 AreaDamageType=7 Health=50.0
    ProjectileFXName="Missile" ImpactFXName="Rocket"
[Ammo1] Name="556" InstDamage=30
[Ammo2] Name="Frag" Type=1 MaxAmount=1 AreaDamage=100 AreaDamageRadius=400 AreaDamageType=7
[Weapon0] Name="LAW" Guntype=9 AmmoName0="LAW" ShotsPerClip=1 RapidFireCount=1 Range=5000
[Weapon1] Name="M4A1" Guntype=4 AmmoName0="556" ShotsPerClip=30 Range=3500
[Weapon2] Name="M67" Guntype=7 AmmoName0="Frag"
"#;

const FX_TXT: &str = r#"
[ProjectileFX0] Name="Missile" Class="CProjectile" Velocity=1300 Lifetime=10.0 Gravity=0 CanHitSameKind=0
    Model="" ModelScale=<1.0, 1.0, 1.0> FlySound=0 Light=0 Flare=0 SmokeTrail=0
[ImpactFX0] Name="Rocket" Sound="" SoundRadius=0 CreateMark=0 Mark="" DoSurfaceFX=0
"#;

const LAW: i32 = 0;
const M4A1: i32 = 1;
const M67: i32 = 2;

fn data() -> CaData {
    let (attrs, problems) = read_attributes(|name| match name {
        WEAPONS_FILE => Ok(WEAPONS_TXT.as_bytes().to_vec()),
        FX_FILE => Ok(FX_TXT.as_bytes().to_vec()),
        _ => Err(std::io::Error::new(std::io::ErrorKind::NotFound, name.to_string())),
    });
    assert!(attrs.weapons.is_some(), "{problems:?}");
    CaData::with_attributes(attrs)
}

#[derive(Resource, Default)]
struct Hurt(Vec<Damage>);
#[derive(Resource, Default)]
struct Booms(Vec<Detonated>);

fn sim() -> Sim {
    let mut sim = Sim::with(CaSimWith(data()), |app| {
        app.init_resource::<Hurt>().init_resource::<Booms>().add_systems(
            FixedPostUpdate,
            (
                |mut r: MessageReader<Damage>, mut l: ResMut<Hurt>| l.0.extend(r.read().cloned()),
                |mut r: MessageReader<Detonated>, mut l: ResMut<Booms>| l.0.extend(r.read().cloned()),
            ),
        );
    });
    sim.set_tick_interval(TICK_INTERVAL);
    solid(
        &mut sim,
        Vec3::new(-200.0, -1.0, -200.0),
        Vec3::new(200.0, 0.0, 200.0),
        None,
    );
    sim.ticks(1);
    sim
}

fn shot(at: Vec3, dir: Vec3, weapon: i32, in_place: bool, factor: f32) -> CaObjectFire {
    CaObjectFire {
        weapon,
        at,
        dir,
        in_place,
        factor,
        shooter: 0,
    }
}

/// A weapon's part, read from a freshly built one.
fn part<C: Component + Clone>(sim: &mut Sim, id: &str) -> C {
    let w = sim.app.world_mut();
    let holder = w.spawn_empty().id();
    let e = weapon::spawn_weapon(w, id, holder).expect(id);
    let c = w.get::<C>(e).cloned().expect("the part");
    w.despawn(e);
    w.despawn(holder);
    c
}

/// The record index is the install's: 0 the LAW, 1 the M4A1, 2 the M67;
/// an index past the records, or without the install, fires nothing.
#[test]
fn wo51_weapon_ids_are_install_record_indices() {
    let d = data();
    assert_eq!(d.weapon_for_index(LAW), Some("combat_arms:law"));
    assert_eq!(d.weapon_for_index(M4A1), Some("combat_arms:m4a1"));
    assert_eq!(d.weapon_for_index(M67), Some("combat_arms:m67"));
    let mut sim = sim();
    let f = shot(Vec3::Y, Vec3::NEG_Y, 3, true, 1.0);
    assert_eq!(fire::fire(sim.app.world_mut(), &f), Shot::NoWeapon);
    assert_eq!(
        CaData::builtin().weapon_for_index(M4A1),
        None,
        "no install: no record index"
    );
}

/// 51. `FIRE` on a live object holding the M67, no target, not along its
/// forward: one grenade straight down that goes off in place (fuse 0) on
/// the next server frame, x `DamageFactor`, hurting the player beside it
/// with no attacker.
#[test]
fn wo51_object_grenade_goes_off_in_place() {
    let mut sim = sim();
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 1);
    let data: CaGrenadeData = part(&mut sim, "combat_arms:m67");
    let at = Vec3::new(60.0 * U, 1.0, 0.0);
    let Shot::Grenade(g) = fire::fire(sim.app.world_mut(), &shot(at, Vec3::NEG_Y, M67, true, 0.5)) else {
        panic!("a grenade")
    };
    let c = sim.app.world().get::<CaGrenade>(g).unwrap().clone();
    assert_eq!((c.params.fuse, c.thrower, c.velocity), (0.0, None, Vec3::ZERO));
    assert_eq!(c.params.area_damage, data.params.area_damage * 0.5, "x DamageFactor");
    sim.seconds(0.2);
    let booms = &sim.app.world().resource::<Booms>().0;
    assert_eq!(booms.len(), 1, "it went off");
    assert!(
        (booms[0].at - at).length() < 0.05,
        "where it was made: {:?}",
        booms[0].at
    );
    let hurt: Vec<Damage> = sim
        .app
        .world()
        .resource::<Hurt>()
        .0
        .iter()
        .filter(|d| d.target == victim)
        .cloned()
        .collect();
    assert!(!hurt.is_empty(), "the blast hurt the player beside it");
    assert!(hurt.iter().all(|d| d.attacker.is_none() && d.kind == DamageKind::Blast));
}

/// The LAW from an object: its projectile in place detonates on the next
/// frame where it was made; along a direction, it flies at the record's
/// speed with its own lifetime, blast x `DamageFactor`.
#[test]
fn wo51_object_projectile_in_place_and_along() {
    let mut sim = sim();
    let data: CaProjectileData = part(&mut sim, "combat_arms:law");
    let at = Vec3::new(0.0, 2.0, 100.0 * U);
    let Shot::Projectile(p) = fire::fire(sim.app.world_mut(), &shot(at, Vec3::NEG_Y, LAW, true, 2.0)) else {
        panic!("a projectile")
    };
    let o = sim.app.world().get::<CaObject>(p).unwrap();
    assert_eq!((o.weapon, o.owner), ("combat_arms:law", None));
    sim.seconds(0.2);
    let booms = std::mem::take(&mut sim.app.world_mut().resource_mut::<Booms>().0);
    assert_eq!(booms.len(), 1, "in place: it went off");
    assert!((booms[0].at - at).length() < 0.05, "{:?}", booms[0].at);
    assert_eq!((booms[0].weapon, booms[0].thrower), ("combat_arms:law", None));

    let Shot::Projectile(p) = fire::fire(sim.app.world_mut(), &shot(at, Vec3::X, LAW, false, 2.0)) else {
        panic!("a projectile")
    };
    let o = sim.app.world().get::<CaObject>(p).unwrap();
    assert!(
        (o.velocity - Vec3::X * data.params.speed).length() < 1e-3,
        "{:?}",
        o.velocity
    );
    sim.seconds(0.2);
    assert!(sim.app.world().resource::<Booms>().0.is_empty(), "along: still flying");
}

/// The M4A1 from an object above a player, straight down: each ray hits
/// him for the base damage (the roll's middle, the band at that range) x
/// `DamageFactor`, a bullet credited to no one; a wall between: nothing.
#[test]
fn wo51_object_gun_shoots_down() {
    let mut sim = sim();
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 1);
    let d: CaDamage = part(&mut sim, "combat_arms:m4a1");
    let h: Hitscan = part(&mut sim, "combat_arms:m4a1");
    let feet = sim.position(victim);
    let at = feet + Vec3::Y * 4.0;
    let Shot::Rays(n) = fire::fire(sim.app.world_mut(), &shot(at, Vec3::NEG_Y, M4A1, true, 1.5)) else {
        panic!("rays")
    };
    assert_eq!(n, h.pellets.max(1));
    sim.ticks(2);
    let hurt = std::mem::take(&mut sim.app.world_mut().resource_mut::<Hurt>().0);
    assert_eq!(hurt.len(), n as usize, "{hurt:?}");
    let top = hurt[0].point;
    let want = base_damage(&d, 0.5, false, at.y - top.y) * 1.5;
    for x in &hurt {
        assert_eq!(
            (x.target, x.attacker, x.kind, x.weapon),
            (victim, None, DamageKind::Bullet, Some("combat_arms:m4a1"))
        );
        assert!((x.amount - want).abs() < 1e-5, "{} vs {want}", x.amount);
    }
    assert!(want > 0.0 && want < 1.0 / HP);
    // A slab between the object and the player.
    solid(
        &mut sim,
        feet + Vec3::new(-1.0, 2.5, -1.0),
        feet + Vec3::new(1.0, 2.7, 1.0),
        None,
    );
    assert_eq!(
        fire::fire(sim.app.world_mut(), &shot(at, Vec3::NEG_Y, M4A1, true, 1.0)),
        Shot::Rays(0)
    );
}

/// The whole path: `FIRE` sent to a live world model with `WeaponId` 2
/// (the object world's `Fx::Fire`, `apply_effects`) makes the grenade; a
/// dead one fires nothing.
#[test]
fn wo51_fire_message_makes_the_shot() {
    let mut sim = sim();
    let scale = movement::movement_data().meters_per_unit;
    let centre = from_engine(Vec3::new(0.0, 2.0, 0.0), scale);
    let objects = vec![wm(
        "WorldModel",
        "Gun",
        centre,
        Vec3::splat(10.0),
        &[
            ("WeaponId", "2".into()),
            ("NeverDestroy", "0".into()),
            ("HitPoints", "10".into()),
        ],
    )];
    {
        let mut o = sim.app.world_mut().resource_mut::<CaObjects>();
        o.world = ObjectWorld::build(&objects);
        o.scale = scale;
        o.world.deliver(0, "FIRE", Ctx::default());
    }
    apply_effects(sim.app.world_mut());
    let w = sim.app.world_mut();
    let made: Vec<CaGrenade> = w.query::<&CaGrenade>().iter(w).cloned().collect();
    assert_eq!(made.len(), 1, "one grenade");
    assert!(
        (made[0].pos * made[0].unit - Vec3::new(0.0, 2.0, 0.0)).length() < 0.01,
        "at the object"
    );
    assert_eq!(made[0].params.fuse, 0.0, "in place");
    sim.seconds(0.2);
    {
        let mut o = sim.app.world_mut().resource_mut::<CaObjects>();
        o.world.damage(0, 20.0, Default::default(), false);
        o.world.fx.clear();
        o.world.deliver(0, "FIRE", Ctx::default());
    }
    apply_effects(sim.app.world_mut());
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&CaGrenade>().iter(w).count(), 0, "a dead one fires nothing");
}
