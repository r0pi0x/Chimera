//! specs/combat_arms/attachments.md's HUD cases (30, 31) on `CaSim`, the
//! greybox and the HUD's state (`CaHudStatePlugin`, no window): the local
//! player holds the game's M4A1 with the sight it chose (`CaLoadout`).
//!
//! Tolerances: exact (texts, image numbers).

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{Health, LocalPlayer, Team};
use chimera_sim::weapon::{Inventory, Magazine, StartingWeapons, Zoomed};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::{
        attachments::CaLoadout,
        movement,
        weapons::{CaZoom, RIFLE},
    }, harness::CaSim};
use combat_arms_ui::hud::{CaHud, CaHudStatePlugin, model::RANGE_IN};

const U: f32 = 0.0185;

/// The local player at x = 30 looking down -Z, holding the M4A1 with
/// `parts`, settled and drawn.
fn setup(parts: &[&str]) -> (Sim, Entity) {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaHudStatePlugin));
        app.insert_resource(StartingWeapons {
            team: Vec::new(),
            all: vec![RIFLE],
        });
    });
    let me = sim.spawn_character(Vec3::new(30.0, 1.0, 37.0), movement::ID);
    sim.app
        .world_mut()
        .entity_mut(me)
        .insert((LocalPlayer, Team(1), Name::new("Me"), CaLoadout::default().with(RIFLE, parts)));
    sim.seconds(1.5);
    (sim, me)
}

fn hud(sim: &Sim) -> CaHud {
    sim.app.world().resource::<CaHud>().clone()
}

fn active(sim: &Sim, p: Entity) -> Entity {
    sim.app.world().get::<Inventory>(p).unwrap().active.unwrap()
}

/// Press zoom and wait for the transition.
fn zoom_in(sim: &mut Sim, p: Entity) {
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    let w = active(sim, p);
    for _ in 0..30 {
        if sim.app.world().get::<CaZoom>(w).unwrap().transition.is_none() {
            break;
        }
        sim.ticks(1);
    }
}

/// 30. M4A1 + DotSight_A_AR zoomed: no crosshair lines (the overlay); the
/// range readout for a wall 2000 units away: "50.0", gold.
#[test]
fn t30_overlay_hides_the_crosshair() {
    let (mut sim, me) = setup(&["DotSight_A_AR"]);
    let eye = sim.position(me) + sim.state(me).eye_offset;
    // A wall across the lane whose face is 2000 units ahead of the eye.
    let half = 0.5;
    sim.app.world_mut().spawn((
        Transform::from_xyz(30.0, eye.y, eye.z - 2000.0 * U - half),
        RigidBody::Static,
        Collider::cuboid(6.0, 6.0, 2.0 * half),
    ));
    sim.intent(me).pitch = 0.0;
    sim.ticks(2);
    assert!(hud(&sim).crosshair.draws.lines, "lines before zooming");
    zoom_in(&mut sim, me);
    assert!(sim.app.world().get::<Zoomed>(me).is_some_and(|z| z.scope), "the overlay");
    sim.ticks(1);
    let c = hud(&sim).crosshair;
    assert!(!c.draws.lines && c.elements.is_empty(), "no crosshair lines");
    assert_eq!(c.range, Some(("50.0".to_string(), RANGE_IN)));
}

/// 31. M4A1 + DotSight_B_AR, HP 73 / 100, clip 24 / 30: health image 70,
/// clip 80; HP 95: 100; HP 4: 0; shown unzoomed.
#[test]
fn t31_gauge_sight() {
    let (mut sim, me) = setup(&["DotSight_B_AR"]);
    let w = active(&sim, me);
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 24;
    for (hp, want) in [(73.0, 70), (95.0, 100), (4.0, 0)] {
        sim.app.world_mut().get_mut::<Health>(me).unwrap().current = hp / 100.0;
        sim.ticks(1);
        assert!(sim.app.world().get::<Zoomed>(me).is_none(), "unzoomed");
        assert_eq!(hud(&sim).crosshair.gauge, Some((want, 80)), "HP {hp}");
    }
}

/// hud_crosshair.md 98: the gauge sight's images belong to the weapon
/// panel: shown with the crosshair disabled, not while dead.
#[test]
fn hud_t98_gauge_sight_with_the_weapon_panel() {
    let (mut sim, me) = setup(&["DotSight_B_AR"]);
    let w = active(&sim, me);
    sim.app.world_mut().entity_mut(w).insert(combat_arms::game::hud::CaHudData {
        disable_crosshair: true,
        ..default()
    });
    sim.ticks(1);
    let h = hud(&sim);
    assert!(h.crosshair.elements.is_empty() && h.weapon.is_some());
    assert!(h.crosshair.gauge.is_some(), "crosshair or not");
    sim.app.world_mut().get_mut::<Health>(me).unwrap().current = 0.0;
    sim.ticks(1);
    let h = hud(&sim);
    assert!(h.weapon.is_none() && h.crosshair.gauge.is_none(), "with the weapon panel");
}
