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
