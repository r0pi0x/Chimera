//! Install launchers: impact projectile weapons the install's attribute
//! data adds or changes (special_weapons.md 1-4, "Where the numbers
//! live"; effects_sounds.md 6, 11), on `CaSimWith` and documents written
//! here (no file of the game). The cases the 2026-10-28 playtest found:
//! a `Guntype` 9 weapon whose ammo fires the flame puff (`Yellow_Flame`)
//! and has 999 rounds (the stock data's boss flame launcher) copied the
//! first rocket launcher, so its puffs flew as rockets with no blast and
//! no lasting sphere: they hurt nobody. And every launcher's round was
//! drawn as a placeholder ball (or its launcher's own model), without
//! its fire sound or explosion.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Health};
use chimera_sim::weapon::{
    self, Inventory, Magazine,
    data::WeaponSet,
    grenade::{Detonated, GrenadeKind},
};
use chimera_world::{harness::Sim, map::loose::ShownItem};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        data::{CaData, read_attributes, attributes::{FX_FILE, WEAPONS_FILE}},
        effects::{self, CaFxRequest, data::{CaWeaponFx, EFFECTS}, sounds::CaSound},
        grenades::OverTimeKind,
        effects::FxKind,
        special::{CaObject, FlareLook, PROJECTILE, ProjectileLook, ProjectileParams},
    },
    harness::CaSimWith,
};

use super::special_weapons::{HP, U, body, health, player, set_health, solid};

/// Our flamethrower, our LAW, a boss flame launcher (its ammo fires the
/// flame puff, 999 rounds, infinite) and an NPC copy of the LAW, all
/// `Guntype` 9; the names of the new two are ours.
const WEAPONS_TXT: &str = r#"
[Ammo0] Name="Flamethrower" Type=1 MaxAmount=300 InstDamageType=3 ProgDamage=12.0 ProgDamageDuration=1.0
    ProgDamageRadius=75.0 ProgDamageLifetime=2.0 ProgDamageType=3 ProjectileFXName="Yellow_Flame" ImpactFXName="Bolt"
[Ammo1] Name="LAW" Type=1 MaxAmount=3 AreaDamage=40 AreaDamageRadius=550 AreaDamageType=7 Health=50.0
    ProjectileFXName="Missile" ImpactFXName="Rocket"
[Ammo2] Name="Test_Boss_Flamer" Type=1 MaxAmount=999 SpawnedAmount=999 InstDamageType=3 ProgDamage=10.0
    ProgDamageDuration=1.0 ProgDamageRadius=75.0 ProgDamageLifetime=2.0 ProgDamageType=3
    ProjectileFXName="Yellow_Flame" ImpactFXName="Bolt"
[Ammo3] Name="Test_LAW_npc" Type=1 MaxAmount=3 AreaDamage=40 AreaDamageRadius=550 AreaDamageType=7 Health=50.0
    ProjectileFXName="Missile" ImpactFXName="Rocket" FireFXName="Test_M136"
[Weapon0] Name="LAW" Guntype=9 AmmoName0="LAW" ShotsPerClip=1 RapidFireCount=1 Range=5000
    FireSnd="TEST_SND\LAW\fire.wav" FireSndRadius=2000
[Weapon1] Name="Flamethrower" Guntype=9 AmmoName0="Flamethrower" ShotsPerClip=300 RapidFireCount=0 Range=1200
[Weapon2] Name="Test_Boss_Flamer" Guntype=9 AmmoName0="Test_Boss_Flamer" ShotsPerClip=999 InfiniteAmmo=1
    RapidFireCount=0 Range=2000
[Weapon3] Name="Test_LAW_npc" Guntype=9 AmmoName0="Test_LAW_npc" ShotsPerClip=1 RapidFireCount=0 Range=5000
    FireSnd="TEST_SND\LAW\fire.wav" FireSndRadius=2000
"#;

/// The records they fire and show (`FX.TXT`).
const FX_TXT: &str = r#"
[ProjectileFX0] Name="Yellow_Flame" Class="CProjectile" Velocity=500 Lifetime=1.0 Gravity=0 CanHitSameKind=0
    Model="" ModelScale=<0.0, 0.0, 0.0> FXName="Test_Flame" FXSmoothShutdown=1 FlySound=0 SoundRadius=900
    Light=0 Flare=0 SmokeTrail=0
[ProjectileFX1] Name="Missile" Class="CProjectile" Velocity=1300 Lifetime=10.0 Gravity=0 CanHitSameKind=0
    Model="TEST_PT\Models\Rocket.ltb" ModelScale=<0.35, 0.35, 0.35> Skin0="TEST_PT\Skins\Rocket.dtx"
    FXName="Test_Tail" FXLoop=0 FXSmoothShutdown=1 FlySound="TEST_SND\Rocket\fly.wav" SoundRadius=1200
    Light=1 LightRadius=200 Flare=1 FlareSprite="TEST_FX\Flare.spr" FlareScale=0.5 FlareYOffset=2.0
    SmokeTrail=1 SmokeTrailType=2
[FireFX0] Name="Test_M136" FXName="Test_Rocket_Fire" EjectShells=0
[ImpactFX0] Name="Rocket" Sound="TEST_SND\Explosions\boom.wav" SoundRadius=1500 CreateMark=0 Mark=""
    DoSurfaceFX=0 FXName="Test_Boom"
[ImpactFX1] Name="Bolt" Sound="Surface" SoundRadius=0 CreateMark=0 Mark="" DoSurfaceFX=0
"#;

const BOSS: &str = "combat_arms:test_boss_flamer";
const NPC_LAW: &str = "combat_arms:test_law_npc";
const LAW: &str = "combat_arms:law";

fn data() -> CaData {
    let (attrs, problems) = read_attributes(|name| match name {
        WEAPONS_FILE => Ok(WEAPONS_TXT.as_bytes().to_vec()),
        FX_FILE => Ok(FX_TXT.as_bytes().to_vec()),
        _ => Err(std::io::Error::new(std::io::ErrorKind::NotFound, name.to_string())),
    });
    assert!(attrs.weapons.is_some(), "{problems:?}");
    CaData::with_attributes(attrs)
}

fn set() -> &'static WeaponSet {
    data().weapon_set()
}

#[derive(Resource, Default)]
struct Heard(Vec<CaSound>);
#[derive(Resource, Default)]
struct Looks(Vec<CaFxRequest>);
#[derive(Resource, Default)]
struct Booms(Vec<Detonated>);
#[derive(Resource, Default)]
struct Hurt(Vec<Damage>);

fn sim() -> Sim {
    let mut sim = Sim::with(CaSimWith(data()), |app| {
        app.init_resource::<Heard>()
            .init_resource::<Looks>()
            .init_resource::<Booms>()
            .init_resource::<Hurt>()
            .add_systems(
                FixedPostUpdate,
                (
                    |mut r: MessageReader<Detonated>, mut l: ResMut<Booms>| l.0.extend(r.read().cloned()),
                    |mut r: MessageReader<Damage>, mut l: ResMut<Hurt>| l.0.extend(r.read().cloned()),
                ),
            )
            .add_systems(
                Update,
                (
                    |mut r: MessageReader<CaFxRequest>, mut l: ResMut<Looks>| l.0.extend(r.read().cloned()),
                    |mut r: MessageReader<CaSound>, mut h: ResMut<Heard>| h.0.extend(r.read().cloned()),
                )
                    .after(effects::CaEffectsSet),
            );
    });
    sim.set_tick_interval(TICK_INTERVAL);
    solid(&mut sim, Vec3::new(-200.0, -1.0, -200.0), Vec3::new(200.0, 0.0, 200.0), None);
    sim.ticks(1);
    sim
}

/// A player holding `id`, settled.
fn armed(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = weapon::give(sim.app.world_mut(), p, id).expect("unknown weapon");
    sim.seconds(3.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "{id} drawn");
    w
}

/// `shooter` looks at `victim`'s centre.
fn aim_at_chest(sim: &mut Sim, shooter: Entity, victim: Entity) {
    let (c, _) = body(sim, victim);
    let eye = sim.position(shooter) + sim.state(shooter).eye_offset;
    let d = (c - eye).normalize();
    sim.intent(shooter).pitch = d.y.asin();
}

/// The merged data: the boss flame launcher is a copy of the
/// flamethrower (the same projectile record), not of the LAW: its puff's
/// record (500 u/s for 1 s, no model) and its ammo's lasting sphere (burn
/// 10 HP/s for 1 s, 75 u, 2 s), the shooter exempt (`Guntype` 9); 999 in
/// the clip, infinite; the flamethrower's fire keys. The NPC LAW copies
/// the LAW and takes the missile record's model, its blast, its fire
/// sound and its impact record.
#[test]
fn install_launchers_copy_the_weapon_of_their_projectile() {
    let set = set();
    let boss = set.get(BOSS).expect("the boss flamer is merged");
    let p: ProjectileParams = boss.hook(PROJECTILE).unwrap();
    assert_eq!((p.speed, p.lifetime, p.gravity), (500.0, 1.0, false), "{p:?}");
    let ot = p.over_time.expect("a lasting sphere");
    assert_eq!((ot.kind, ot.rate, ot.duration, ot.radius, ot.lifetime), (OverTimeKind::Burn, 10.0, 1.0, 75.0, 2.0));
    assert!(p.shooter_exempt && p.area_damage == 0.0 && p.model.is_none(), "{p:?}");
    assert!(boss.hooks.contains_key(combat_arms::game::special::FLAME), "the flamethrower's fire keys");
    assert_eq!(boss.magazine.as_ref().map(|m| m.clip), Some(999));
    let fx: CaWeaponFx = boss.hook(EFFECTS).unwrap();
    // `Surface` for burn ammo: no sound (effects_sounds.md 11).
    assert_eq!(fx.impact, "Bolt");
    assert_eq!(fx.impact_record.as_ref().map(|r| r.sound.as_str()), Some(""));

    let npc = set.get(NPC_LAW).expect("the NPC LAW is merged");
    let p: ProjectileParams = npc.hook(PROJECTILE).unwrap();
    assert_eq!((p.speed, p.lifetime, p.area_damage, p.area_radius, p.area_type), (1300.0, 10.0, 40.0, 550.0, 7));
    assert!(p.over_time.is_none() && !npc.hooks.contains_key(combat_arms::game::special::FLAME));
    assert_eq!(p.model.as_deref(), Some("test_pt/models/rocket.ltb"));
    assert_eq!((p.model_scale, p.skins.clone()), ([0.35; 3], vec!["test_pt/skins/rocket.dtx".to_string()]));
    let fx: CaWeaponFx = npc.hook(EFFECTS).unwrap();
    assert_eq!((fx.fire_sound.as_str(), fx.fire_radius), ("test_snd/law/fire.wav", 2000.0));
    let r = fx.impact_record.expect("the Rocket record");
    assert_eq!((r.sound.as_str(), r.sound_radius, r.fx.as_str(), r.area), ("test_snd/explosions/boom.wav", 1500.0, "Test_Boom", true));
    // Ours takes the same from the install; both are drawn as one model.
    let law: ProjectileParams = set.get(LAW).unwrap().hook(PROJECTILE).unwrap();
    assert_eq!(law.shown_key(), p.shown_key());
    // The frame's own trace stays a unit's: the round is the projectile.
    assert_eq!(npc.hitscan.as_ref().map(|h| h.range), set.get(LAW).unwrap().hitscan.as_ref().map(|h| h.range));
}

/// The boss flamer's stream burns an enemy in front to death: its puffs
/// touch him and leave burning spheres (burn damage, credited to the
/// shooter and the weapon); 999 in the clip at the start; the shooter
/// never burns.
#[test]
fn install_flame_launcher_burns_and_kills() {
    let mut sim = sim();
    let shooter = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 1);
    let victim = player(&mut sim, 0.0, 200.0, 0.0, 2);
    let w = armed(&mut sim, shooter, BOSS);
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 999, "999 rounds, as the data says");
    aim_at_chest(&mut sim, shooter, victim);
    sim.intent(shooter).fire = true;
    sim.seconds(1.5);
    let burnt = 100.0 - health(&sim, victim);
    assert!(burnt > 5.0, "the puffs burn him: {burnt} HP");
    sim.seconds(12.0);
    sim.intent(shooter).fire = false;
    assert_eq!(health(&sim, victim), 0.0, "burnt to death");
    let hurt = &sim.app.world().resource::<Hurt>().0;
    assert!(
        hurt.iter().any(|d| d.target == victim && d.kind == DamageKind::Burn && d.attacker == Some(shooter) && d.weapon == Some(BOSS)),
        "burn damage from the shooter's weapon"
    );
    assert_eq!(health(&sim, shooter), 100.0, "his own flames never burn him");
}

/// The NPC LAW: its rocket is drawn as the missile record's model, flies
/// to the enemy and blows up at him: 80 (40 twice) kills one at 70; the
/// fire plays the weapon's `FireSnd`, the blast the Rocket record's
/// sound (radius 1500) and its group.
#[test]
fn install_rocket_launcher_shows_hurts_and_sounds() {
    let mut sim = sim();
    let shooter = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 1);
    let victim = player(&mut sim, 0.0, 400.0, 0.0, 2);
    set_health(&mut sim, victim, 70.0);
    let (c, _) = body(&mut sim, victim);
    let w = armed(&mut sim, shooter, NPC_LAW);
    aim_at_chest(&mut sim, shooter, victim);
    sim.app.world_mut().resource_mut::<Heard>().0.clear();
    sim.intent(shooter).fire = true;
    let mut key = None;
    for _ in 0..40 {
        sim.ticks(1);
        let world = sim.app.world_mut();
        if let Some(k) = world.query_filtered::<&ShownItem, With<CaObject>>().iter(world).next() {
            key = Some(k.0.clone());
            break;
        }
    }
    sim.intent(shooter).fire = false;
    let key = key.expect("a rocket in flight");
    let law: ProjectileParams = set().get(NPC_LAW).unwrap().hook(PROJECTILE).unwrap();
    assert_eq!(key, law.shown_key(), "drawn as the missile record's model");
    sim.seconds(1.0);
    assert!(
        sim.app.world().resource::<Booms>().0.iter().any(|b| b.weapon == NPC_LAW && b.kind == GrenadeKind::Blast),
        "the rocket went off"
    );
    assert_eq!(sim.app.world().get::<Health>(victim).unwrap().current, 0.0, "80 kills him at 70");
    let hurt = &sim.app.world().resource::<Hurt>().0;
    let total: f32 = hurt.iter().filter(|d| d.target == victim).map(|d| d.amount / HP).sum();
    assert!((total - 80.0).abs() < 0.5 || total >= 70.0, "the blast's 80: {total}");
    let heard = &sim.app.world().resource::<Heard>().0;
    assert!(heard.iter().any(|s| s.file == "test_snd/law/fire.wav"), "the fire sound: {heard:?}");
    let boom = heard.iter().find(|s| s.file == "test_snd/explosions/boom.wav").expect("the explosion sound");
    assert_eq!(boom.radii.1, 1500.0);
    assert!(boom.at.is_some_and(|at| at.distance(c) < 1.0), "at the blast");
    let looks = &sim.app.world().resource::<Looks>().0;
    assert!(looks.iter().any(|l| l.group == "Test_Boom"), "the explosion's group: {looks:?}");
    let _ = (w, U);
}

// ---------------------------------------------------------------------------
// special_weapons.md Follow-up (2026-10-30): projectile looks and sounds
// in flight, from the projectile records.

/// Cases 48, 50 and 54's data: the missile's group (no loop, smooth
/// shut-down), its fly sound and radius, its flare and its type 2 smoke
/// trail (no light: `Light` is never made); the flame puff's group and no
/// fly sound (`FlySound` 0). Both ride on the drawn key, so whatever draws
/// the projectile (a network client too) reads them back.
#[test]
fn t48_t50_projectile_looks_from_the_records() {
    let set = set();
    let law: ProjectileParams = set.get(NPC_LAW).unwrap().hook(PROJECTILE).unwrap();
    let want = ProjectileLook {
        fx: "Test_Tail".into(),
        fx_loop: false,
        fx_smooth: true,
        fly_sound: "test_snd/rocket/fly.wav".into(),
        sound_radius: 1200.0,
        flare: Some(FlareLook {
            sprite: "test_fx/flare.spr".into(),
            scale: [0.5, 0.5],
            y_offset: 2.0,
        }),
        smoke_trail: Some(2),
    };
    assert_eq!(law.look, want);
    assert_eq!(ProjectileLook::parse(&law.shown_key()), want, "the drawn key carries it");
    let flame: ProjectileParams = set.get(BOSS).unwrap().hook(PROJECTILE).unwrap();
    assert_eq!((flame.look.fx.as_str(), flame.look.fx_smooth), ("Test_Flame", true));
    assert!(flame.look.fly_sound.is_empty(), "FlySound 0: no sound");
    assert!(ProjectileParams::is_modelless(&flame.shown_key()));
    // A look without a fly sound carries no radius.
    assert_eq!(ProjectileLook::parse(&flame.shown_key()), ProjectileLook { sound_radius: 0.0, ..flame.look.clone() });
    // Follow-up 7, 8: the launcher's FireFX record; its round is launched
    // ammo (Type 1), not a grenade's.
    let fx: CaWeaponFx = set.get(NPC_LAW).unwrap().hook(EFFECTS).unwrap();
    assert_eq!(fx.fire, "Test_M136");
    let f = fx.fire_record.expect("the install's FireFX record");
    assert_eq!((f.fx.as_str(), f.shells), ("Test_Rocket_Fire", false));
    assert!(fx.launched);
}

/// Case 52: the LAW fired shows the shooter its FireFX group once, at the
/// shot's (server's) fire position, and no casing; its fire sound once.
#[test]
fn t52_launcher_muzzle() {
    let mut sim = sim();
    let shooter = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 1);
    sim.app.world_mut().entity_mut(shooter).insert(chimera_core::core::LocalPlayer);
    armed(&mut sim, shooter, NPC_LAW);
    sim.app.world_mut().resource_mut::<Looks>().0.clear();
    sim.app.world_mut().resource_mut::<Heard>().0.clear();
    let eye = sim.position(shooter) + sim.state(shooter).eye_offset;
    sim.intent(shooter).fire = true;
    sim.seconds(0.3);
    sim.intent(shooter).fire = false;
    let looks = &sim.app.world().resource::<Looks>().0;
    let own: Vec<_> = looks.iter().filter(|l| l.group == "Test_Rocket_Fire").collect();
    assert_eq!(own.len(), 1, "{looks:?}");
    assert_eq!(own[0].kind, FxKind::OwnFire);
    assert!(own[0].at.distance(eye) < 0.05, "at the fire position {eye}: {}", own[0].at);
    let world = sim.app.world_mut();
    assert_eq!(world.query::<&combat_arms::game::effects::CaShell>().iter(world).count(), 0, "no casing");
    let heard = &sim.app.world().resource::<Heard>().0;
    assert_eq!(heard.iter().filter(|s| s.file == "test_snd/law/fire.wav").count(), 1, "one fire sound");
}

/// Case 49 in play: the missile's group (`Test_Tail`, a follow-mode
/// emitter) rides the rocket the LAW fires: its puffs lie along the
/// flight, not at the launcher.
#[test]
fn t49_the_rockets_group_rides_it() {
    use chimera_world::map::particles::{ParticleSet, Particles, step_particles};
    use super::attached_effects::{TAIL_TEX, material, materials, particles, test_library};
    combat_arms::game::effects::library::set_library(Some(test_library()));
    let mut sim = sim();
    sim.app.init_resource::<Particles>().add_systems(Update, step_particles.in_set(ParticleSet::Step));
    materials(&mut sim);
    let shooter = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 1);
    armed(&mut sim, shooter, NPC_LAW);
    let eye = sim.position(shooter) + sim.state(shooter).eye_offset;
    sim.intent(shooter).fire = true;
    sim.seconds(0.25);
    sim.intent(shooter).fire = false;
    let tex = material(&sim, TAIL_TEX, false);
    let d: Vec<f32> = particles(&sim).iter().filter(|(_, p)| p.material == tex).map(|(_, p)| p.position.distance(eye) / U).collect();
    assert!(!d.is_empty(), "the group emits");
    let far = d.iter().cloned().fold(0.0, f32::max);
    assert!(far > 100.0, "puffs along the flight, up to {far} u from the launcher");
}
