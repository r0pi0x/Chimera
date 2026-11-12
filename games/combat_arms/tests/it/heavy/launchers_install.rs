//! The launchers on the user's install (special_weapons.md 2, 3, 4;
//! weapon_firing.md Recoil): each one's projectile record as the loaders
//! read it, its view model's fire keys, and a level shot fired through the
//! real fire path: when the round leaves after the press, along which
//! direction, and what the flight does. The 2026-10-31 sighting (a LAW on
//! grandcanyon climbing steeply and appearing about 75 frames after the
//! press) is checked here against the install's numbers.
//! Needs the V1 key; skips with a note without. `--nocapture` for the table.

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_sim::weapon::{self, WeaponEvent, WeaponEventKind};
use chimera_world::harness::Sim;
use combat_arms::{
    game::{
        GAME, TICK_INTERVAL,
        data::CaData,
        movement,
        special::{CaObject, PROJECTILE, ProjectileParams},
        weapons::timing,
    },
    harness::CaSimWith,
};

#[derive(Resource, Default)]
struct Shots(Vec<(f64, f32, f32)>);

const LAUNCHERS: [&str; 8] = [
    "law", "rpg-7", "m136_at4", "m32", "m79", "mzp_1", "xm_25", "flamethrower",
];

#[test]
fn launchers_as_the_install_has_them() {
    if LocalConfig::load().ok().and_then(|c| c.game_path(GAME)).is_none() {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    }
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data isn't readable (the V1 key)");
        return;
    }
    for name in LAUNCHERS {
        let id = format!("combat_arms:{name}");
        let Some(d) = ca.weapon_set().weapons.iter().find(|w| w.id == id) else {
            eprintln!("{id}: not in the install's data");
            continue;
        };
        let Some(p) = d.hook::<ProjectileParams>(PROJECTILE) else {
            eprintln!("{id}: no projectile hook");
            continue;
        };
        let times = d.models.view.as_deref().and_then(timing::read);
        eprintln!(
            "{id}: speed {} gravity {} lifetime {} perturb {:?} dyn {} | view {:?} fire {:?} keys {:?} prefire {:?} (has {:?})",
            p.speed,
            p.gravity,
            p.lifetime,
            p.perturb,
            p.dynamic_perturb,
            d.models.view,
            times.as_ref().and_then(|t| t.fire),
            times.as_ref().map(|t| t.fire_keys.clone()),
            times.as_ref().and_then(|t| t.prefire),
            times.as_ref().map(|t| t.has_prefire),
        );
    }
    // The LAW's record, as the spec has it.
    let law = ca.weapon_set().weapons.iter().find(|w| w.id == "combat_arms:law").expect("the LAW");
    let p = law.hook::<ProjectileParams>(PROJECTILE).expect("projectile");
    assert_eq!(p.speed, 1300.0);
    assert!(!p.gravity, "the missile record has Gravity 0");
    assert_eq!(p.lifetime, 10.0);
    let t = law.models.view.as_deref().and_then(timing::read).expect("the LAW's view model reads");
    assert_eq!(t.fire_keys, vec![0.0], "the round leaves on the key at 0 ms");
    assert!(t.prefire.is_none_or(|p| p == 0.0), "no pre-fire delay: {:?}", t.prefire);
    assert!((t.fire.unwrap() - 1.0).abs() < 1e-3, "fire animation 1000 ms");

    // A level shot through the real fire path.
    let mut sim = Sim::with(CaSimWith(ca.clone()), |app| {
        app.init_resource::<Shots>().add_systems(
            FixedPostUpdate,
            |mut r: MessageReader<WeaponEvent>, c: Res<chimera_core::core::SimClock>, mut s: ResMut<Shots>| {
                for e in r.read() {
                    if let WeaponEventKind::Fired { yaw, pitch, .. } = e.kind
                        && !e.replay
                    {
                        s.0.push((c.now, yaw, pitch));
                    }
                }
            },
        );
    });
    sim.set_tick_interval(TICK_INTERVAL);
    let floor = sim.app.world_mut().spawn_empty().id();
    let _ = floor;
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    let _ = weapon::give(sim.app.world_mut(), p, "combat_arms:law").expect("law");
    sim.seconds(3.0);
    let t0 = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.intent(p).fire = true;
    let mut first = None;
    for _ in 0..300 {
        sim.ticks(1);
        let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
        let w = sim.app.world_mut();
        if first.is_none() && w.query::<&CaObject>().iter(w).next().is_some() {
            first = Some(now - t0);
            let o = w.query::<&CaObject>().iter(w).next().unwrap();
            assert!(o.velocity.y.abs() < 1.5 && o.velocity.x.abs() < 1.5, "level: {:?}", o.velocity);
            eprintln!("rocket exists {:.3} s after the press; velocity {:?}", now - t0, o.velocity);
        }
    }
    sim.intent(p).fire = false;
    eprintln!("shots (time, yaw, pitch): {:?}", sim.app.world().resource::<Shots>().0);
    let first = first.expect("a rocket left");
    assert!(first <= 0.05, "the rocket leaves within three updates of the press: {first}");
    let shots = &sim.app.world().resource::<Shots>().0;
    assert!(shots[0].2.abs() < 1e-3, "a level shot leaves level: {:?}", shots[0]);
}

/// The air strike designators on the install (special_weapons.md 6.10):
/// their shells drawn as their own record (`LGB_Shell`'s missile model,
/// its group in flight) and blowing up as the shell ammo's impact record
/// (`Dirt_Burster`, `Exp_Napalm_Shell`), and the files a party host's
/// air strike plays and draws beyond the map's load
/// (`special::devices::strike_files`, Combat Arms' `base` set), all read
/// from the install.
#[test]
fn the_air_strike_shells_are_their_own_records() {
    use combat_arms::game::{
        effects::data::{CaFxTables, CaWeaponFx, EFFECTS},
        special::{DEVICE, DeviceParams, devices::strike_files},
    };
    if LocalConfig::load().ok().and_then(|c| c.game_path(GAME)).is_none() {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    }
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data isn't readable (the V1 key)");
        return;
    }
    let tables = CaFxTables::built_in();
    for (id, impact, group) in [
        ("combat_arms:lgb", "Dirt_Burster", "MOAB_EXP"),
        ("combat_arms:airstrike_napalm", "Exp_Napalm_Shell", "exp_napalm_SH"),
    ] {
        let w = ca.weapon_set().weapons.iter().find(|w| w.id == id).expect(id);
        let shell = w.hook::<DeviceParams>(DEVICE).and_then(|d| d.strike).expect("a strike").shell;
        let model = shell.model.as_deref().unwrap_or_default();
        assert!(model.ends_with("as_missile_01.ltb"), "{id}'s shell model: {model:?}");
        assert!(!shell.skins.is_empty(), "{id}'s shell skins");
        assert_eq!(shell.look.fx, "NVL_Head_Tail", "{id}'s shell's group in flight");
        let fx = w.hook::<CaWeaponFx>(EFFECTS).expect("effects");
        assert_eq!(fx.impact, impact, "{id}'s explosion record");
        let record = fx.impact_of(tables).expect("its record");
        assert_eq!(record.fx, group, "{id}'s explosion group");
        assert!(!record.sound.is_empty(), "{id}'s explosion sound");
    }
    let mount = ca.mount().expect("the install's mount");
    let fx = mount
        .read(combat_arms::game::effects::library::FX_FILE)
        .ok()
        .and_then(|b| lithtech_formats::fxf::FxFile::parse(&String::from_utf8_lossy(&b)).ok())
        .expect("the effect file");
    // The napalm's fire is its explosion group's burning particle systems,
    // drawn for (about) its burning sphere's life (5 s).
    let napalm = ca.weapon_set().weapons.iter().find(|w| w.id == "combat_arms:airstrike_napalm").unwrap();
    let burn = napalm.hook::<DeviceParams>(DEVICE).and_then(|d| d.strike).unwrap().shell.over_time.expect("a burning sphere");
    let group = fx.group(&napalm.hook::<CaWeaponFx>(EFFECTS).unwrap().impact_of(tables).unwrap().fx).expect("its group");
    let fire_ends = group.keys.iter().filter(|k| k.kind == "ParticleSystem").map(|k| k.end).fold(0.0, f32::max);
    eprintln!("napalm: group {} of {} s, fire to {fire_ends} s, sphere {} s", group.name, group.length(), burn.lifetime);
    assert!(group.length() >= burn.lifetime - 0.01, "the group lasts the sphere's life");
    assert!(fire_ends >= burn.lifetime - 1.0, "the fire burns most of it: {fire_ends} s");
    let files = strike_files(ca.weapon_set(), tables, Some(&fx));
    eprintln!("the air strike's files: {files:?}");
    for want in ["as_missile_01.ltb", "nvl_explosion.wav", "moab_exp.wav", "incomming.wav", "siren01.wav"] {
        assert!(files.iter().any(|f| f.ends_with(want)), "{want} in {files:?}");
    }
    let missing: Vec<&String> = files.iter().filter(|f| mount.read(f).is_err()).collect();
    assert!(missing.is_empty(), "every file reads from the install: {missing:?}");
}
