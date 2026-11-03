//! M1's skeleton on the headless harness (`harness::Hl2Sim`, the
//! greybox): HL2:DM is the gameplay, a new player moves on
//! `hl2dm:movement` (Source's with H1's numbers: run 190, gravity 600,
//! jump 160 units/s; specs/hl2dm/movement.md's cases are movement.rs) and
//! starts with its loadout (the SMG1 in hand; the pistol, whose bullet
//! does 8, specs/hl2dm/weapons.md 6, 24 to the head).

use bevy::prelude::*;
use chimera_core::{
    core::Health,
    gameplay::Gameplay,
    slots::MovementSlot,
};
use chimera_sim::{
    testkit,
    weapon::{Magazine, WeaponEvent},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use hl2dm::{
    game::{GAME, movement, weapons},
    harness::Hl2Sim,
};

const UNIT: f32 = 0.0254;

fn sim() -> Sim {
    Sim::new(Hl2Sim, GreyboxMapPlugin)
}

/// A player at the greybox's first spawn, settled.
fn player(sim: &mut Sim) -> Entity {
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    p
}

#[test]
fn hl2dm_is_the_gameplay_at_its_tick() {
    let sim = sim();
    let g = sim.app.world().resource::<Gameplay>();
    assert_eq!((g.rules, g.hud, g.binds), (GAME, GAME, GAME));
    assert!((sim.tick_hz() - 1.0 / 0.015).abs() < 0.01, "{}", sim.tick_hz());
}

#[test]
fn a_new_player_runs_hl2dms_movement_with_its_loadout() {
    let mut sim = sim();
    let p = player(&mut sim);
    let w = sim.app.world();
    assert_eq!(w.get::<MovementSlot>(p).map(|m| m.0), Some(movement::ID));
    // The spawn's loadout (weapons.md 3): the gravity gun in hand, the
    // SMG1 the last weapon.
    assert_eq!(testkit::active_id(w, p), Some(weapons::PHYSCANNON));
    let smg = w.get::<chimera_sim::weapon::Inventory>(p).unwrap().last.unwrap();
    assert_eq!(w.get::<Magazine>(smg).unwrap().clip, 45);
}

#[test]
fn runs_at_190_and_jumps_about_21_units() {
    let mut sim = sim();
    let p = player(&mut sim);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let speed = sim.velocity(p).xz().length() / UNIT;
    assert!((speed - 190.0).abs() < 5.0, "run speed {speed} units/s");
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.seconds(1.0);
    // A standing jump: 160 units/s against 600 units/s² peaks at
    // about 20 units (20.145 at the 0.015 s tick: movement.rs).
    let start = sim.position(p).y;
    sim.intent(p).jump = true;
    let mut peak = start;
    for _ in 0..60 {
        sim.ticks(1);
        peak = peak.max(sim.position(p).y);
    }
    sim.intent(p).jump = false;
    let height = (peak - start) / UNIT;
    assert!((height - 20.1).abs() < 1.5, "jump height {height} units");
}

#[test]
fn a_pistol_bullet_does_8_and_24_to_the_head() {
    let mut sim = sim();
    let p = player(&mut sim);
    let target = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 6.0, movement::ID);
    let pistol = {
        let w = sim.app.world();
        let inv = w.get::<chimera_sim::weapon::Inventory>(p).unwrap();
        inv.weapons
            .iter()
            .copied()
            .find(|e| w.get::<chimera_sim::weapon::Weapon>(*e).is_some_and(|x| x.id == weapons::PISTOL))
            .unwrap()
    };
    sim.app.world_mut().get_mut::<chimera_sim::weapon::Inventory>(p).unwrap().wanted = Some(pistol);
    sim.seconds(1.0);
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    for (aim, expected) in [(0.2, 8.0), (0.62, 24.0)] {
        sim.app.world_mut().get_mut::<Health>(target).unwrap().current = 1.0;
        let at = sim.position(target) + Vec3::Y * aim;
        testkit::aim_at(&mut sim, p, at);
        sim.ticks(2);
        sim.intent(p).fire = true;
        sim.ticks(2);
        sim.intent(p).fire = false;
        sim.seconds(0.6);
        let lost = (1.0 - sim.app.world().get::<Health>(target).unwrap().current) * 100.0;
        let hit = cursor
            .read(sim.app.world().resource::<Messages<WeaponEvent>>())
            .filter(|e| e.owner == p)
            .map(|e| format!("{:?}", e.kind))
            .collect::<Vec<_>>();
        assert!((lost - expected).abs() < 0.01, "lost {lost}, expected {expected}: {hit:?}");
    }
}
