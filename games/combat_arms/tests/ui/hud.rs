//! specs/combat_arms/hud_crosshair.md's test cases, one test each
//! (`t01_` is test case 1): the behaviour (`ui::hud::model`) for the
//! numbers, and scenario tests on `CaSim` and the greybox with the HUD's
//! state (`CaHudStatePlugin`, no window) for what the world drives: the
//! crosshair from the held weapon, the hit indicator, damage taken, the
//! health block, the weapon panel, the kill feed, name tags. The rifle is
//! a fixture (tests/data/test_hud_rifle.toml: the spec's M4A1 at rest);
//! the knife is the game's M9.
//!
//! Tolerances: positions to 0.01 px (0.5 px where the spec gives "about"),
//! times to one HUD frame.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Died, Health, Hitgroup, LocalPlayer, RoundRestarts, Team};
use chimera_sim::weapon::{
    Inventory, Magazine, StartingWeapons, Weapon, WeaponEvent, WeaponEventKind, data::WeaponDataAppExt,
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use combat_arms::{
    game::{
        hud::CaHudData,
        inventory, movement,
    },
    harness::CaSim,
    ui::hud::{CaHud, CaHudStatePlugin, HudOptions, HudScreen, model, model::*, rapid_fire_count, spread_of},
};

const RIFLE: &str = "combat_arms:test_hud_rifle";
const KNIFE: &str = inventory::KNIFE;
const EPS: f32 = 0.01;

fn screen() -> Screen {
    Screen::default()
}

/// The M4A1's perturb fields (section B's example).
fn m4() -> Perturb {
    Perturb {
        min: 3,
        max: 100,
        zoomed_min: 2,
        zoomed_max: 30,
        duck: 2,
        move_duck: 3,
        ..default()
    }
}

/// The crosshair of shape `shape` (the weapon's own `weapon_shape`) with
/// limits (min, max), r, the seconds since the shot, an offset, on
/// `screen`.
fn cross(shape: u8, weapon_shape: u8, (min, max): (i32, i32), r: f32, since: Option<f32>, offset: i32, s: Screen) -> Vec<Element> {
    crosshair_elements(
        &CrosshairInputs {
            weapon_shape,
            shape,
            min,
            max,
            r,
            since_fire: since,
            since_alt: None,
            offset,
            wide: 4,
        },
        s,
    )
    .1
}

fn plus(min: i32, max: i32, r: f32, since: Option<f32>, offset: i32, s: Screen) -> Vec<PxRect> {
    cross(0, 0, (min, max), r, since, offset, s)
        .into_iter()
        .map(|e| match e {
            Element::Rect(r) => r,
            e => panic!("not a rect: {e:?}"),
        })
        .collect()
}

fn near(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

/// A horizontal arm: x range on row `row`.
fn h(r: &PxRect, x0: f32, x1: f32, row: f32) {
    assert!(
        near(r.min.x, x0, EPS) && near(r.max.x, x1, EPS) && near(r.min.y, row, EPS) && near(r.max.y, row + 1.0, EPS),
        "{r:?} != x [{x0}, {x1}) row {row}"
    );
}

/// A vertical arm: y range on column `col`.
fn v(r: &PxRect, y0: f32, y1: f32, col: f32) {
    assert!(
        near(r.min.y, y0, EPS) && near(r.max.y, y1, EPS) && near(r.min.x, col, EPS) && near(r.max.x, col + 1.0, EPS),
        "{r:?} != y [{y0}, {y1}) column {col}"
    );
}

fn close(a: Vec2, b: Vec2, tol: f32) {
    assert!(a.distance(b) <= tol, "{a} != {b}");
}

fn dot_at(e: &[Element], x: f32, y: f32) -> bool {
    e.iter().any(|e| matches!(e, Element::Rect(r) if r.min == Vec2::new(x, y) && r.max == Vec2::new(x + 1.0, y + 1.0)))
}

// ---------------------------------------------------------------------------
// Scenario setup

fn sim_with(loadout: &[&'static str]) -> Sim {
    let loadout = loadout.to_vec();
    Sim::with(CaSim, move |app| {
        app.add_plugins((GreyboxMapPlugin, CaHudStatePlugin));
        app.add_weapon_data(&[("test_hud_rifle.toml", include_str!("../data/test_hud_rifle.toml"))])
            .unwrap();
        app.insert_resource(StartingWeapons {
            team: Vec::new(),
            all: loadout,
        });
    })
}

/// The local player (team 1) with `loadout`, settled with its draw done.
fn setup(loadout: &[&'static str]) -> (Sim, Entity) {
    let mut sim = sim_with(loadout);
    let me = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(me).insert((LocalPlayer, Team(1), Name::new("Me")));
    sim.seconds(1.0);
    (sim, me)
}

fn hud(sim: &Sim) -> CaHud {
    sim.app.world().resource::<CaHud>().clone()
}

fn active(sim: &Sim, p: Entity) -> Entity {
    sim.app.world().get::<Inventory>(p).unwrap().active.unwrap()
}

/// Another character `ahead` m in front of `me` (yaw 0 looks down -Z).
fn other(sim: &mut Sim, me: Entity, ahead: f32, team: u8, name: &str) -> Entity {
    let at = sim.position(me) + Vec3::new(0.0, 0.0, -ahead);
    let e = sim.spawn_character(at, movement::ID);
    sim.app.world_mut().entity_mut(e).insert((Team(team), Name::new(name.to_string())));
    sim.ticks(10);
    e
}

/// Turn `me` to look level at `target`'s middle.
fn look_at(sim: &mut Sim, me: Entity, target: Entity) {
    let d = sim.position(target) - sim.position(me);
    let mut i = sim.intent(me);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = -0.1;
    sim.ticks(2);
}

fn hurt(sim: &mut Sim, target: Entity, attacker: Option<Entity>, amount: f32, dir: Vec3) {
    sim.app.world_mut().write_message(Damage {
        target,
        attacker,
        amount,
        point: Vec3::ZERO,
        dir,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
}

/// The local player's own shot trace (a gun's first segment) hitting
/// `target`'s hit box, as the firing writes it (A10).
fn shot(sim: &mut Sim, me: Entity, target: Entity) {
    let weapon = active(sim, me);
    sim.app.world_mut().write_message(WeaponEvent {
        owner: me,
        weapon,
        kind: WeaponEventKind::Shot {
            from: Vec3::ZERO,
            to: Vec3::ZERO,
            hit: Some(target),
            normal: None,
        },
        replay: false,
    });
}

/// Another character past its spawn window (more than 3.5 s since it
/// appeared).
fn veteran(sim: &mut Sim, me: Entity, ahead: f32, team: u8, name: &str) -> Entity {
    let e = other(sim, me, ahead, team, name);
    sim.seconds(3.6);
    e
}

fn now_ms(sim: &Sim) -> f64 {
    sim.app.world().resource::<Time>().elapsed_secs_f64() * 1000.0
}

// ---------------------------------------------------------------------------
// Crosshair geometry (A1 to A5)

#[test]
fn t01_plus_at_rest() {
    let r = plus(3, 100, 0.0, None, 0, screen());
    assert_eq!(r.len(), 4, "four arms, no dot");
    h(&r[0], 500.0, 507.0, 383.0);
    h(&r[1], 516.0, 523.0, 383.0);
    v(&r[2], 372.0, 379.0, 511.0);
    v(&r[3], 388.0, 395.0, 511.0);
    assert_eq!(choose_colour(ColourInputs { weapon: Some(None), ..default() }, [0; 4]), [0, 255, 0, 255]);
    // In the game: the fixture's cone 3/1000 is the gap; green.
    let (sim, _) = setup(&[RIFLE]);
    let c = hud(&sim).crosshair;
    assert!(c.draws.lines);
    assert_eq!(c.colour, [0, 255, 0, 255]);
    assert_eq!(c.elements, cross(0, 0, (3, 3), 0.0, None, 0, screen()));
}

#[test]
fn t02_full_spread() {
    let r = plus(3, 100, 1.0, None, 0, screen());
    h(&r[0], 403.0, 410.0, 383.0);
    h(&r[1], 613.0, 620.0, 383.0);
    v(&r[2], 275.0, 282.0, 511.0);
    v(&r[3], 485.0, 492.0, 511.0);
}

#[test]
fn t03_half_spread() {
    let r = plus(3, 100, 0.5, None, 0, screen());
    h(&r[0], 451.5, 458.5, 383.0);
    h(&r[1], 564.5, 571.5, 383.0);
}

#[test]
fn t04_1920x1080() {
    let s = Screen::new(1920.0, 1080.0);
    let m = Measures::new(3, 100, 0.0, None, 0, s);
    assert!(near(m.g, 5.625, EPS) && near(m.l, 13.125, EPS));
    assert_eq!(m.centre, Vec2::new(960.0, 540.0));
    let r = plus(3, 100, 0.0, None, 0, s);
    h(&r[0], 939.25, 952.375, 539.0);
    h(&r[1], 966.625, 979.75, 539.0);
    v(&r[2], 519.25, 532.375, 959.0);
    v(&r[3], 546.625, 559.75, 959.0);
}

#[test]
fn t05_1024x600_not_squashed() {
    let r = plus(3, 100, 0.0, None, 0, Screen::new(1024.0, 600.0));
    h(&r[0], 500.0, 507.0, 299.0);
    v(&r[2], 288.0, 295.0, 511.0);
    v(&r[3], 304.0, 311.0, 511.0);
}

#[test]
fn t06_just_fired() {
    let m = Measures::new(3, 100, 0.0, Some(0.0), 0, screen());
    assert!(near(m.d, 7.0, EPS) && near(m.l, 13.0, EPS));
    let r = plus(3, 100, 0.0, Some(0.0), 0, screen());
    h(&r[0], 490.0, 503.0, 383.0);
    h(&r[1], 520.0, 533.0, 383.0);
    v(&r[2], 362.0, 375.0, 511.0);
    v(&r[3], 392.0, 405.0, 511.0);
    // In the game: a shot (Fired) kicks the crosshair at once.
    let (mut sim, me) = setup(&[RIFLE]);
    let w = active(&sim, me);
    sim.app.world_mut().write_message(WeaponEvent {
        owner: me,
        weapon: w,
        kind: WeaponEventKind::Fired {
            id: RIFLE,
            origin: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            seed: 0,
            spread: chimera_sim::weapon::SpreadShape::Disc {
                inaccuracy: 0.003,
                spread: 0.0,
            },
            mode: 0,
        },
        replay: false,
    });
    sim.ticks(1);
    let m = hud(&sim).crosshair.measures.unwrap();
    assert!(near(m.d, 7.0, EPS) && near(m.l, 13.0, EPS), "{m:?}");
    sim.seconds(0.2);
    let m = hud(&sim).crosshair.measures.unwrap();
    assert!(near(m.d, 3.0, EPS) && near(m.l, 7.0, EPS), "{m:?}");
}

#[test]
fn t07_kick_half_way() {
    let m = Measures::new(3, 100, 0.0, Some(0.05), 0, screen());
    assert!(near(m.d, 5.0, EPS) && near(m.l, 10.0, EPS));
    let r = plus(3, 100, 0.0, Some(0.05), 0, screen());
    h(&r[0], 495.0, 505.0, 383.0);
    h(&r[1], 518.0, 528.0, 383.0);
}

#[test]
fn t08_kick_over() {
    assert_eq!(plus(3, 100, 0.0, Some(0.1), 0, screen()), plus(3, 100, 0.0, None, 0, screen()));
}

fn zoomed() -> Stance {
    Stance {
        zoomed: true,
        ..default()
    }
}

#[test]
fn t09_zoomed() {
    assert_eq!(perturb_limits(&m4(), &PerturbMods::default(), zoomed()), (2, 30));
    let r = plus(2, 30, 0.0, None, 0, screen());
    h(&r[0], 501.0, 508.0, 383.0);
    h(&r[1], 515.0, 522.0, 383.0);
}

#[test]
fn t10_zoomed_half() {
    let (min, max) = perturb_limits(&m4(), &PerturbMods::default(), zoomed());
    assert!(near(gap(min, max, 0.5, 1.0), 16.0, EPS));
    let r = plus(min, max, 0.5, None, 0, screen());
    h(&r[0], 487.0, 494.0, 383.0);
    h(&r[1], 529.0, 536.0, 383.0);
}

#[test]
fn t11_crouched_still() {
    let s = Stance {
        crouch: true,
        ..default()
    };
    let (min, max) = perturb_limits(&m4(), &PerturbMods::default(), s);
    assert_eq!((min, max), (2, 100));
    assert_eq!(plus(min, max, 0.0, None, 0, screen()), plus(2, 30, 0.0, None, 0, screen()));
    // Only with both crouch fields (edge case).
    let one = Perturb { move_duck: 0, ..m4() };
    assert_eq!(perturb_limits(&one, &PerturbMods::default(), s), (3, 100));
}

#[test]
fn t12_crouched_moving() {
    let s = Stance {
        crouch: true,
        moving: true,
        ..default()
    };
    let (min, max) = perturb_limits(&m4(), &PerturbMods::default(), s);
    assert_eq!((min, max), (3, 100));
    assert_eq!(plus(min, max, 0.0, None, 0, screen()), plus(3, 100, 0.0, None, 0, screen()));
}

#[test]
fn t13_offset_and_colour() {
    let m = Measures::new(3, 100, 0.0, None, 45, screen());
    assert!(near(m.d, 48.0, EPS));
    let r = plus(3, 100, 0.0, None, 45, screen());
    h(&r[0], 455.0, 462.0, 383.0);
    h(&r[1], 561.0, 568.0, 383.0);
    let c = choose_colour(
        ColourInputs {
            weapon: Some(Some([110, 191, 228])),
            ..default()
        },
        DEFAULT_CROSSHAIR,
    );
    assert_eq!(c, [110, 191, 228, 255]);
}

#[test]
fn t14_offset_unscaled() {
    let s = Screen::new(2048.0, 1536.0);
    let m = Measures::new(3, 100, 0.0, None, 45, s);
    assert!(near(m.g, 6.0, EPS) && near(m.d, 51.0, EPS) && near(m.l, 14.0, EPS));
    let r = plus(3, 100, 0.0, None, 45, s);
    h(&r[0], 957.0, 971.0, 767.0);
}

#[test]
fn t15_plus_with_dot() {
    let e = cross(1, 1, (3, 100), 0.0, None, 0, screen());
    assert_eq!(e[..4], cross(0, 0, (3, 100), 0.0, None, 0, screen())[..]);
    assert_eq!(e.len(), 5);
    assert!(dot_at(&e, 511.0, 383.0));
}

#[test]
fn t16_no_top_arm() {
    let e = cross(2, 2, (3, 100), 0.0, None, 0, screen());
    let plain = cross(0, 0, (3, 100), 0.0, None, 0, screen());
    assert_eq!(e.len(), 4);
    assert!(e.contains(&plain[0]) && e.contains(&plain[1]) && e.contains(&plain[3]));
    assert!(!e.contains(&plain[2]), "no top arm");
    assert!(dot_at(&e, 511.0, 383.0));
}

#[test]
fn t17_x() {
    let e = cross(3, 3, (3, 100), 0.0, None, 0, screen());
    assert!(!e.iter().any(|e| matches!(e, Element::Rect(r) if r.size().x > 1.0 || r.size().y > 1.0)), "no bars");
    let lines: Vec<_> = e
        .iter()
        .filter_map(|e| match e {
            Element::Line(a, b) => Some((*a, *b)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 4);
    let c = Vec2::new(512.0, 384.0);
    for (a, b) in &lines {
        assert!(near(a.distance(c), 5.0, EPS) && near(b.distance(c), 12.0, EPS));
    }
    close(lines[0].0, Vec2::new(508.46, 380.46), 0.01);
    close(lines[0].1, Vec2::new(503.51, 375.51), 0.01);
    assert!(dot_at(&e, 511.0, 383.0));
}

fn circles(e: &[Element]) -> Vec<f32> {
    e.iter()
        .filter_map(|e| match e {
            Element::Circle { centre, radius } => {
                assert_eq!(*centre, Vec2::new(512.0, 384.0));
                Some(*radius)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn t18_circle() {
    let e = cross(4, 4, (3, 100), 0.0, None, 0, screen());
    assert_eq!(circles(&e), vec![4.0]);
    let v = circle_vertices(Vec2::new(512.0, 384.0), 4.0);
    assert_eq!(v.len(), 32);
    close(v[0], Vec2::new(512.0, 380.0), 1e-4);
    assert!(dot_at(&e, 511.0, 383.0));
}

#[test]
fn t19_circle_just_fired() {
    let e = cross(4, 4, (3, 100), 0.0, Some(0.0), 0, screen());
    assert_eq!(circles(&e), vec![14.0]);
}

#[test]
fn t20_brackets() {
    let e = cross(6, 6, (3, 100), 0.0, None, 0, screen());
    let lines: Vec<_> = e
        .iter()
        .filter_map(|e| match e {
            Element::Line(a, b) => Some((*a, *b)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 4);
    let ends = |apex: Vec2| -> Vec<Vec2> { lines.iter().filter(|(a, _)| *a == apex).map(|(_, b)| *b).collect() };
    let left = ends(Vec2::new(494.0, 384.0));
    let right = ends(Vec2::new(530.0, 384.0));
    assert_eq!((left.len(), right.len()), (2, 2));
    close(left[0], Vec2::new(502.49, 375.51), 0.01);
    close(left[1], Vec2::new(502.49, 392.49), 0.01);
    close(right[0], Vec2::new(521.51, 375.51), 0.01);
    close(right[1], Vec2::new(521.51, 392.49), 0.01);
    for (a, b) in &lines {
        assert!(near(a.distance(*b), 12.0, EPS));
    }
    assert!(dot_at(&e, 511.0, 383.0));
}

#[test]
fn t21_double_circle() {
    let e = cross(5, 5, (3, 100), 0.0, None, 0, screen());
    assert_eq!(circles(&e), vec![4.0, 16.0], "inner 4; outer m' 15, M' 150, D' 15");
    assert!(!e.iter().any(|e| matches!(e, Element::Rect(_))), "no dot");
}

#[test]
fn t22_item_circle() {
    let shape = choose_shape(0, Some(5), true);
    assert_eq!(shape, 5);
    let e = cross(shape, 0, (3, 100), 0.0, None, 0, screen());
    assert_eq!(circles(&e), vec![4.0], "one circle");
    assert_eq!(e.len(), 1, "no dot");
    // An item outside a tournament room's list is ignored.
    assert_eq!(choose_shape(0, Some(5), false), 0);
}

#[test]
fn t23_weapon_shape_wins() {
    assert_eq!(choose_shape(6, Some(3), true), 6);
    // In the game: the item set on the HUD.
    let (mut sim, _) = setup(&[RIFLE]);
    sim.app.world_mut().resource_mut::<HudOptions>().item_shape = Some(3);
    sim.ticks(1);
    assert_eq!(hud(&sim).crosshair.shape, 3, "the fixture has no shape of its own");
}

#[test]
fn t24_item_and_dot_sight_colours() {
    let yellow = [255, 255, 0, 255];
    let base = ColourInputs {
        item: Some(yellow),
        weapon: Some(None),
        ..default()
    };
    assert_eq!(choose_colour(base, DEFAULT_CROSSHAIR), yellow);
    let magenta = [255, 0, 255, 255];
    assert_eq!(
        choose_colour(
            ColourInputs {
                dot_sight: Some(magenta),
                ..base
            },
            DEFAULT_CROSSHAIR
        ),
        magenta
    );
    // A zero item colour counts as none; no weapon data keeps the last.
    assert_eq!(
        choose_colour(
            ColourInputs {
                item: Some([0, 0, 0, 255]),
                weapon: Some(None),
                ..default()
            },
            HIT_CROSSHAIR
        ),
        DEFAULT_CROSSHAIR
    );
    assert_eq!(choose_colour(ColourInputs::default(), HIT_CROSSHAIR), HIT_CROSSHAIR);
    // In the game.
    let (mut sim, _) = setup(&[RIFLE]);
    sim.app.world_mut().resource_mut::<HudOptions>().item_colour = Some(yellow);
    sim.ticks(1);
    assert_eq!(hud(&sim).crosshair.colour, yellow);
}

// ---------------------------------------------------------------------------
// Hit indicator (A10)

#[test]
fn t25_hit_indicator() {
    let mut h = HitIndicator::default();
    h.hit(0.0, true);
    for t in [0.0, 100.0, 499.0, 500.0] {
        assert!(h.frame(t), "{t}");
    }
    assert!(!h.frame(516.7));
    // In the game: the local player's shot trace on an enemy turns the
    // crosshair red for 500 ms.
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = veteran(&mut sim, me, 5.0, 2, "Foe");
    shot(&mut sim, me, foe);
    sim.ticks(1);
    let t_hit = now_ms(&sim);
    assert_eq!(hud(&sim).crosshair.colour, HIT_CROSSHAIR);
    while now_ms(&sim) - t_hit <= 500.0 {
        assert_eq!(hud(&sim).crosshair.colour, HIT_CROSSHAIR, "at {}", now_ms(&sim) - t_hit);
        sim.ticks(1);
    }
    assert_eq!(hud(&sim).crosshair.colour, DEFAULT_CROSSHAIR, "green on the first frame past 500 ms");
}

#[test]
fn t26_second_hit_extends() {
    let mut h = HitIndicator::default();
    h.hit(0.0, true);
    assert!(h.frame(400.0));
    h.hit(400.0, true);
    assert!(h.frame(600.0) && h.frame(900.0));
    assert!(!h.frame(916.7));
}

#[test]
fn t27_teammate_hit() {
    let mut h = HitIndicator::default();
    h.hit(0.0, false);
    assert!(!h.frame(0.0));
    let (mut sim, me) = setup(&[RIFLE]);
    let mate = veteran(&mut sim, me, 5.0, 1, "Mate");
    shot(&mut sim, me, mate);
    sim.ticks(1);
    assert_eq!(hud(&sim).crosshair.colour, DEFAULT_CROSSHAIR);
}

#[test]
fn t28_round_end_clears() {
    let mut h = HitIndicator::default();
    h.hit(0.0, true);
    h.reset();
    assert!(!h.frame(10.0));
    let (mut sim, me) = setup(&[RIFLE]);
    sim.app.world_mut().init_resource::<RoundRestarts>();
    sim.ticks(1);
    let foe = veteran(&mut sim, me, 5.0, 2, "Foe");
    shot(&mut sim, me, foe);
    sim.ticks(1);
    assert_eq!(hud(&sim).crosshair.colour, HIT_CROSSHAIR);
    chimera_core::core::restart_round(sim.app.world_mut());
    sim.ticks(1);
    assert_eq!(hud(&sim).crosshair.colour, DEFAULT_CROSSHAIR, "the next frame");
}

// ---------------------------------------------------------------------------
// What draws when (A7, A8, A9)

fn shown() -> model::Visibility {
    model::Visibility {
        ok: true,
        weapon: true,
        ..default()
    }
}

#[test]
fn t29_scope_overlay() {
    let d = draws(model::Visibility {
        scope_overlay: true,
        ..shown()
    });
    assert_eq!(
        d,
        Draws {
            damage: true,
            ..default()
        },
        "damage feedback only"
    );
    // In the game: a sniper scope's overlay hides the lines.
    let (mut sim, me) = setup(&[RIFLE]);
    let w = active(&sim, me);
    let mut modes = chimera_sim::weapon::AltModes::new(2, 0.3, false);
    modes.current = 1;
    sim.app.world_mut().entity_mut(w).insert((
        modes,
        chimera_sim::weapon::Zoom {
            fov: vec![20.0],
            max_speed: None,
            unzoom_after_shot: false,
            scope: true,
        },
    ));
    sim.ticks(2);
    assert!(sim.app.world().get::<chimera_sim::weapon::Zoomed>(me).is_some_and(|z| z.scope));
    let c = hud(&sim).crosshair;
    assert!(!c.draws.lines && c.elements.is_empty());
}

#[test]
fn t30_disabled_with_dot_sight() {
    let d = draws(model::Visibility {
        disabled: true,
        dot_sight: true,
        ..shown()
    });
    assert!(!d.lines);
    assert_eq!(d.dot_sight, 1);
    assert_eq!(dot_sight_rect(screen()), PxRect::new(448.0, 448.0 + 129.0, 320.0, 320.0 + 129.0));
    // In the game: `disable_crosshair` and `dot_sight` from the hook.
    let (mut sim, me) = setup(&[RIFLE]);
    let w = active(&sim, me);
    sim.app.world_mut().entity_mut(w).insert(CaHudData {
        disable_crosshair: true,
        dot_sight: Some("reticle".into()),
        ..default()
    });
    sim.ticks(1);
    let c = hud(&sim).crosshair;
    assert!(c.elements.is_empty());
    assert_eq!((c.draws.dot_sight, c.dot_sight), (1, Some(dot_sight_rect(screen()))));
    assert_eq!(c.dot_sight_image.as_deref(), Some("reticle"), "the record's image is the one drawn");
}

/// A weapon whose record names no reticle draws none; one that does
/// draws it (its texture from the record) unless a scope overlay is up.
#[test]
fn t30b_reticle_follows_the_record_and_the_overlay() {
    let (mut sim, me) = setup(&[RIFLE]);
    sim.ticks(1);
    let c = hud(&sim).crosshair;
    assert!(c.dot_sight.is_none() && c.dot_sight_image.is_none() && c.draws.dot_sight == 0, "no DotSightName: no reticle");
    let w = active(&sim, me);
    sim.app.world_mut().entity_mut(w).insert(CaHudData {
        dot_sight: Some("hud/red_dot.dtx".into()),
        ..default()
    });
    sim.ticks(1);
    let c = hud(&sim).crosshair;
    assert_eq!((c.draws.dot_sight, c.dot_sight_image.as_deref()), (2, Some("hud/red_dot.dtx")), "drawn with the crosshair, twice (quirk 10)");
    // A scope overlay up: nothing but the range readout (A7, step 4).
    let d = draws(model::Visibility {
        scope_overlay: true,
        dot_sight: true,
        ..shown()
    });
    assert_eq!(d.dot_sight, 0);
}

#[test]
fn t31_dot_sight_twice() {
    let d = draws(model::Visibility {
        dot_sight: true,
        ..shown()
    });
    assert!(d.lines);
    assert_eq!(d.dot_sight, 2);
}

#[test]
fn t32_range_in() {
    assert_eq!(range_readout(Some(2000.0), 3500.0, 0.0), ("50.0".into(), [255, 198, 0, 255]));
    assert_eq!(RANGE_AT, Vec2::new(490.0, 400.0));
    // With the overlay up it still draws (A9); after the lines otherwise.
    let d = draws(model::Visibility {
        scope_overlay: true,
        range_readout: true,
        ..shown()
    });
    assert!(d.range_readout && !d.lines);
}

#[test]
fn t33_range_out() {
    assert_eq!(range_readout(Some(4000.0), 3500.0, 0.0), ("100.0".into(), [255, 0, 0, 255]));
    // An attachment's range bonus widens it: 3500 + trunc(3500 x 0.2).
    assert_eq!(range_readout(Some(4000.0), 3500.0, 0.2).1, RANGE_IN);
}

#[test]
fn t34_range_nothing() {
    assert_eq!(range_readout(None, 3500.0, 0.0).0, "");
}

// ---------------------------------------------------------------------------
// Damage taken (C)

/// Facing yaw 0, level: forward -Z, right +X.
const F: Vec3 = Vec3::NEG_Z;
const RT: Vec3 = Vec3::X;

#[test]
fn t35_from_ahead() {
    let d = DamageFeedback::hit(0.0, 1.0, F, RT, -F, 0.0).unwrap();
    assert!(near(d.s, 3.0, 1e-4));
    let p = d.corners(0.0, screen());
    close(p[0], Vec2::new(681.0, 215.0), 0.5);
    close(p[1], Vec2::new(343.0, 215.0), 0.5);
    close(p[2], Vec2::new(343.0, 553.0), 0.5);
    close(p[3], Vec2::new(681.0, 553.0), 0.5);
    // In the game: damage travelling back along the view (a shooter
    // ahead), the same corners.
    let (mut sim, me) = setup(&[RIFLE]);
    hurt(&mut sim, me, None, 0.01, Vec3::Z);
    sim.ticks(1);
    let v = hud(&sim).damage.expect("damage shown");
    close(v.corners[0], Vec2::new(681.0, 215.0), 0.5);
    close(v.corners[1], Vec2::new(343.0, 215.0), 0.5);
}

#[test]
fn t36_from_behind() {
    let d = DamageFeedback::hit(0.0, 1.0, F, RT, F, 0.0).unwrap();
    assert!(near(d.s, 9.0, 1e-4));
    let p = d.corners(0.0, screen());
    close(p[0], Vec2::new(342.8, 552.8), 0.5);
    close(p[1], Vec2::new(680.8, 553.2), 0.5);
}

#[test]
fn t37_from_the_right() {
    let d = DamageFeedback::hit(0.0, 1.0, F, RT, -RT, 0.0).unwrap();
    assert!(near(d.s, 12.0, 1e-4));
    let p = d.corners(0.0, screen());
    close(p[0], Vec2::new(680.7, 553.3), 0.5);
    close(p[1], Vec2::new(681.3, 215.3), 0.5);
}

#[test]
fn t38_turning_keeps_pointing() {
    let d = DamageFeedback::hit(0.0, 1.0, F, RT, -F, 0.0).unwrap();
    assert!(near(d.phi(std::f32::consts::FRAC_PI_2), 0.75 * std::f32::consts::PI, 1e-4));
    let p = d.corners(std::f32::consts::FRAC_PI_2, screen());
    close(p[0], Vec2::new(343.0, 215.0), 0.5);
    close(p[1], Vec2::new(343.0, 553.0), 0.5);
    // In the game: turning right (our yaw falls) by pi/2 after the hit.
    let (mut sim, me) = setup(&[RIFLE]);
    hurt(&mut sim, me, None, 0.01, Vec3::Z);
    sim.ticks(1);
    sim.intent(me).yaw -= std::f32::consts::FRAC_PI_2;
    sim.ticks(1);
    let v = hud(&sim).damage.unwrap();
    close(v.corners[0], Vec2::new(343.0, 215.0), 0.5);
    close(v.corners[1], Vec2::new(343.0, 553.0), 0.5);
}

#[test]
fn t39_fade_per_frame() {
    // The n-th HUD frame counting the hit's own as 1 (Corrections 3).
    let mut d = DamageFeedback::hit(0.0, 1.0, F, RT, -F, 0.0).unwrap();
    let frame = |n: u32| (n - 1) as f64 * 1000.0 / 60.0;
    for n in 1..=30 {
        assert!(d.frame(frame(n)));
        assert_eq!(d.alpha, (255 - 2 * n) as u8);
    }
    assert_eq!(d.alpha, 195, "the 30th frame");
    for n in 31..61 {
        assert!(d.frame(frame(n)), "{n}");
    }
    assert!(!d.frame(frame(61)), "gone at t_dmg + 1000 ms");
    // In the game: 253 on the hit's frame; gone on the first frame 1000
    // ms after.
    let (mut sim, me) = setup(&[RIFLE]);
    hurt(&mut sim, me, None, 0.01, Vec3::Z);
    sim.ticks(1);
    let t = now_ms(&sim);
    assert_eq!(hud(&sim).damage.unwrap().alpha, 253);
    sim.ticks(1);
    assert_eq!(hud(&sim).damage.unwrap().alpha, 251);
    while now_ms(&sim) - t < 1000.0 {
        assert!(hud(&sim).damage.is_some());
        sim.ticks(1);
    }
    assert!(hud(&sim).damage.is_none());
}

#[test]
fn t40_absorbed_hit() {
    assert!(DamageFeedback::hit(0.0, 0.0, F, RT, -F, 0.0).is_none());
    let (mut sim, me) = setup(&[RIFLE]);
    hurt(&mut sim, me, None, 0.0, Vec3::Z);
    sim.ticks(1);
    assert!(hud(&sim).damage.is_none());
}

/// A hit that loses no health shows nothing (C's trigger: a health loss
/// > 0): under `god` the bots' shots once lit the overlay and the arc for
/// as long as they kept shooting, at full health.
#[test]
fn hits_without_health_loss_show_nothing() {
    let (mut sim, me) = setup(&[RIFLE]);
    sim.app.world_mut().entity_mut(me).insert(chimera_core::core::God);
    for _ in 0..5 {
        hurt(&mut sim, me, None, 0.2, Vec3::Z);
        sim.ticks(1);
        assert!(hud(&sim).damage.is_none(), "no overlay under god");
    }
    sim.app.world_mut().entity_mut(me).remove::<chimera_core::core::God>();
    hurt(&mut sim, me, None, 0.01, Vec3::Z);
    sim.ticks(1);
    assert!(hud(&sim).damage.is_some(), "a hit that lands shows");
}

#[test]
fn t41_overlay_rect() {
    assert_eq!(hit_overlay_rect(Screen::new(1920.0, 1080.0)), PxRect::new(0.0, 1920.0, 0.0, 1440.0));
}

// ---------------------------------------------------------------------------
// Health, armour, stamina (D2, D3)

fn set_health(sim: &mut Sim, p: Entity, hp: f32) {
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = hp / 100.0;
    sim.ticks(1);
}

#[test]
fn t42_low_health() {
    assert_eq!(hp_shown(30.0, 100.0, 100.0), 30);
    assert!(low_health(30.0, 100.0, false));
    let c = health_colours(true);
    assert_eq!((c.values, c.hp_ap_labels, c.silhouette), (LOW_HEALTH, LOW_HEALTH, Some(LOW_HEALTH)));
    assert_eq!(c.other_labels, [216, 216, 216, 255]);
    // On the OS ms clock: silent on the first low frame, then a beat on
    // each frame more than 1000 ms after the last (Corrections 4).
    let mut beat = Heartbeat::default();
    let beats: Vec<u32> = (0..=180).filter(|n| beat.frame((*n as f64 * 1000.0 / 60.0).floor(), true)).collect();
    assert_eq!(beats, vec![61, 122], "more than 1000 ms apart");
    // In the game.
    let (mut sim, me) = setup(&[RIFLE]);
    set_health(&mut sim, me, 30.0);
    let s = hud(&sim).status.unwrap();
    assert_eq!((s.hp.as_str(), s.low, s.colours.values), ("30", true, LOW_HEALTH));
    let before = hud(&sim).heartbeats;
    sim.seconds(2.05);
    assert_eq!(hud(&sim).heartbeats - before, 2);
}

#[test]
fn t43_not_low() {
    assert!(!low_health(31.0, 100.0, false));
    let c = health_colours(false);
    assert_eq!((c.values, c.hp_ap_labels, c.silhouette), (VALUE_WHITE, LABEL_GREY, None));
    let mut beat = Heartbeat::default();
    assert!(!beat.frame(0.0, false));
    let (mut sim, me) = setup(&[RIFLE]);
    set_health(&mut sim, me, 31.0);
    let s = hud(&sim).status.unwrap();
    assert_eq!((s.hp.as_str(), s.low), ("31", false));
    let before = hud(&sim).heartbeats;
    sim.seconds(1.5);
    assert_eq!(hud(&sim).heartbeats, before);
}

#[test]
fn t44_interface_max_health() {
    assert_eq!(hp_shown(1.0, 200.0, 100.0), 1);
    assert_eq!(hp_shown(199.0, 200.0, 100.0), 99);
}

#[test]
fn t45_zero_health() {
    assert_eq!(hp_shown(0.0, 100.0, 100.0), 0);
    assert!(low_health(0.0, 100.0, false));
    assert_eq!(hp_shown(-5.0, 100.0, 100.0), 0);
}

#[test]
fn t46_infected() {
    assert!(!low_health(30.0, 100.0, true));
}

#[test]
fn t47_armour() {
    assert_eq!(ap_shown(75.0), 75);
    assert_eq!(ap_shown(-3.0), 0);
    let (mut sim, me) = setup(&[RIFLE]);
    // Combat Arms' armour points (every character has some).
    sim.app.world_mut().get_mut::<chimera_sim::weapon::Armor>(me).unwrap().amount = 75.0 / 100.0;
    sim.ticks(1);
    assert_eq!(hud(&sim).status.unwrap().ap, "75");
}

#[test]
fn t48_stamina() {
    assert_eq!(stamina_bar(50.0, 100.0), 60.0);
    assert_eq!(stamina_bar(50.0, 100.0) * Screen::new(1920.0, 1080.0).xr(), 112.5);
    assert_eq!(stamina_bar(100.0, 100.0) * Screen::new(1920.0, 1080.0).xr(), 225.0, "the full bar at 1920");
    assert_eq!(stamina_bar(0.0, 100.0), 0.0);
    assert_eq!(stamina_bar(100.0, 100.0), 120.0);
    // In the game: a rested player's full gauge.
    let (sim, _) = setup(&[RIFLE]);
    assert_eq!(hud(&sim).status.unwrap().stamina_bar, 120.0);
}

// ---------------------------------------------------------------------------
// The weapon panel (D4)

fn set_ammo(sim: &mut Sim, w: Entity, clip: u32, reserve: u32) {
    let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
    m.clip = clip;
    m.reserve = reserve;
    sim.ticks(1);
}

#[test]
fn t49_ammo_text() {
    let (mut sim, me) = setup(&[RIFLE]);
    let w = active(&sim, me);
    set_ammo(&mut sim, w, 7, 90);
    let p = hud(&sim).weapon.unwrap();
    assert_eq!(p.name, "M4A1");
    assert_eq!(p.ammo, Some(("007".into(), "090".into())));
    assert_eq!((panel::CLIP_AT, panel::SLASH_AT, panel::RESERVE_AT), (
        Vec2::new(910.0, 742.0),
        Vec2::new(962.0, 742.0),
        Vec2::new(974.0, 741.0)
    ));
    assert_eq!((panel::CLIP, panel::RESERVE), ([240, 240, 240, 255], [180, 180, 180, 255]));
}

#[test]
fn t50_digits() {
    assert_eq!((three_digits(30), three_digits(1200), three_digits(7)), ("030".into(), "1200".into(), "007".into()));
}

#[test]
fn t51_empty_no_warning() {
    let (mut sim, me) = setup(&[RIFLE]);
    let w = active(&sim, me);
    set_ammo(&mut sim, w, 0, 0);
    assert_eq!(hud(&sim).weapon.unwrap().ammo, Some(("000".into(), "000".into())));
}

#[test]
fn t52_knife() {
    let (sim, me) = setup(&[KNIFE]);
    assert_eq!(sim.app.world().get::<Weapon>(active(&sim, me)).unwrap().id, KNIFE);
    let p = hud(&sim).weapon.expect("the name stays");
    assert!(!p.name.is_empty());
    assert_eq!((p.ammo, p.icon), (None, None));
}

/// As corrected (case 77): only the mission bomb and a weapon in no order
/// list hide the panel.
#[test]
fn t53_not_a_gun() {
    let p = PanelInputs {
        name: "C4".into(),
        class: 0,
        ..default()
    };
    assert_eq!(weapon_panel(&p, true), None);
    let bomb = PanelInputs {
        class: 6,
        bomb: true,
        ..p.clone()
    };
    assert_eq!(weapon_panel(&bomb, true), None);
    // The bomb flag hides a gun's panel too.
    let (mut sim, me) = setup(&[RIFLE]);
    let w = active(&sim, me);
    sim.app.world_mut().entity_mut(w).insert(CaHudData {
        bomb: true,
        ..default()
    });
    sim.ticks(1);
    assert_eq!(hud(&sim).weapon, None);
}

/// As corrected (case 76): a sniper rifle is a primary and gets an icon
/// (single shot).
#[test]
fn t54_fire_mode_icon() {
    assert_eq!(fire_icon(1, None, 0, false), Some(FireIcon::Auto));
    assert_eq!(fire_icon(1, None, 1, false), Some(FireIcon::Single));
    assert_eq!(fire_icon(1, Some(2), 0, false), Some(FireIcon::Burst2));
    assert_eq!(fire_icon(1, Some(3), 0, false), Some(FireIcon::Burst3));
    assert_eq!(fire_icon(1, None, 1, false), Some(FireIcon::Single), "sniper rifle");
    assert_eq!(rapid_fire_count(None), 1);
    assert_eq!(panel::ICON, (Vec2::new(926.0, 731.0), Vec2::new(128.0, 16.0)));
    let (sim, _) = setup(&[RIFLE]);
    assert_eq!(hud(&sim).weapon.unwrap().icon, Some(FireIcon::Auto));
}

#[test]
fn t55_name_box() {
    let (at, size) = panel::NAME_BOX;
    assert_eq!(at.x + size.x, 1019.0, "right edge");
    assert_eq!(at.y + size.y / 2.0, 721.0, "centred on y");
}

// ---------------------------------------------------------------------------
// Kill feed (D5)

fn kill(killer: &str, victim: &str) -> Kill {
    Kill {
        killer: Some((killer.into(), true)),
        victim: (victim.into(), false),
        icon: KillIcon::Weapon(Some("m4a1".into())),
        badges: Vec::new(),
    }
}

#[test]
fn t56_feed_expiry() {
    let mut f = KillFeed::default();
    for (t, v) in [(0.0, "A"), (1000.0, "B"), (2000.0, "C")] {
        f.push(t, kill("K", v));
    }
    let ys: Vec<f32> = (0..f.lines.len()).map(KillFeed::line_y).collect();
    assert_eq!(ys, vec![45.0, 65.0, 85.0]);
    f.frame(30000.0 + 1000.0 / 60.0);
    let left: Vec<&str> = f.lines.iter().map(|(_, k)| k.victim.0.as_str()).collect();
    assert_eq!(left, vec!["B", "C"], "A gone, B at 45, C at 65");
    // In the game: Died messages become lines.
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = other(&mut sim, me, 5.0, 2, "Foe");
    let damage = Damage {
        target: foe,
        attacker: Some(me),
        amount: 1.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Head,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    };
    sim.app.world_mut().write_message(Died {
        entity: foe,
        attacker: Some(me),
        damage,
    });
    sim.ticks(1);
    let feed = hud(&sim).feed;
    assert_eq!(feed.len(), 1);
    let k = &feed[0].kill;
    assert_eq!(feed[0].y, 45.0);
    assert_eq!(k.killer, Some(("Me".into(), true)));
    assert_eq!(k.victim, ("Foe".into(), false));
    assert_eq!(k.icon, KillIcon::Weapon(Some(RIFLE.into())));
    assert_eq!(k.badges, vec![Badge::Headshot]);
    sim.seconds(30.05);
    assert!(hud(&sim).feed.is_empty());
}

#[test]
fn t57_six_kills() {
    let mut f = KillFeed::default();
    for i in 0..6 {
        f.push(i as f64 * 100.0, kill("K", &format!("V{i}")));
    }
    assert_eq!(f.lines.len(), 5);
    assert_eq!(f.lines[0].1.victim.0, "V1", "the first kill is gone");
    assert_eq!(f.lines[4].1.victim.0, "V5");
    assert_eq!(KillFeed::line_y(4), 125.0);
}

fn width(s: &str) -> f32 {
    10.0 * s.len() as f32
}

#[test]
fn t58_teammate_headshot() {
    let k = Kill {
        badges: vec![Badge::Headshot],
        ..kill("Mate", "Foe")
    };
    let items = feed_line(&k, width);
    assert_eq!(
        items,
        vec![
            FeedItem::Text {
                x: 5.0,
                text: "Mate".into(),
                colour: [124, 190, 221, 150]
            },
            FeedItem::Icon {
                x: 45.0,
                icon: KillIcon::Weapon(Some("m4a1".into()))
            },
            FeedItem::Badge {
                x: 109.0,
                badge: Badge::Headshot
            },
            FeedItem::Text {
                x: 141.0,
                text: "Foe".into(),
                colour: [218, 115, 115, 150]
            },
        ]
    );
}

#[test]
fn t59_badges_overwrite() {
    let k = Kill {
        badges: vec![Badge::Revenge, Badge::Headshot, Badge::Groin],
        ..kill("Mate", "Foe")
    };
    let items = feed_line(&k, width);
    let badges: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            FeedItem::Badge { x, badge } => Some((*x, *badge)),
            _ => None,
        })
        .collect();
    // The second slot moves to the last badge's x (Corrections 6).
    assert_eq!(badges, vec![(109.0, Badge::Revenge), (173.0, Badge::Groin)]);
    let icon_end = 45.0 + FEED_ICON_WIDTH;
    let victim_x = items
        .iter()
        .find_map(|i| match i {
            FeedItem::Text { x, text, .. } if text == "Foe" => Some(*x),
            _ => None,
        })
        .unwrap();
    assert_eq!(victim_x - icon_end, 96.0);
    // No killer: the victim's name in both slots, in the victim's colour.
    let world = Kill {
        killer: None,
        icon: KillIcon::Suicide,
        ..kill("", "Foe")
    };
    let items = feed_line(&world, width);
    assert!(matches!(&items[0], FeedItem::Text { text, colour, .. } if text == "Foe" && *colour == feed_colour(false)));
}

// ---------------------------------------------------------------------------
// Score panel and timer (D6, D7)

#[test]
fn t60_tdm_panel() {
    let ([alpha, bravo], highlight) = score_panel([25, 0], 100, Some(0));
    assert_eq!((alpha.score.as_str(), alpha.colour, alpha.bar), ("25", [223, 31, 20, 255], 38.0));
    assert_eq!((bravo.score.as_str(), bravo.colour, bravo.bar), ("0", [170, 170, 170, 255], 0.0));
    assert_eq!(highlight, Some(Vec2::new(344.0, 18.0)));
}

#[test]
fn t61_full_bar() {
    assert_eq!(score_bar(120, 100), 155.0);
    assert_eq!(score_bar(5, 0), 0.0);
}

#[test]
fn t62_round_timer() {
    assert_eq!(round_timer(61900.0), "01 : 01");
    assert_eq!(round_timer(400.0), "00 : 00");
    assert_eq!(round_timer(600000.0), "10 : 00");
    assert_eq!(round_timer(-5.0), "00 : 00");
    assert_eq!(round_timer(6_000_000.0), "100 : 00");
}

#[test]
fn t63_tdm_no_timer() {
    assert_eq!(timer(false, 61900.0), None);
    assert_eq!(timer(true, 61900.0).as_deref(), Some("01 : 01"));
}

// ---------------------------------------------------------------------------
// Name tags (A11)

#[test]
fn t64_enemy_tag_while_aimed() {
    let t = name_tag(TagInputs {
        local_alive: true,
        alive: true,
        aimed: true,
        seen: true,
        ..default()
    })
    .unwrap();
    assert_eq!((t.name, t.hp_bar), ([215, 17, 18, 200], false));
    assert_eq!(name_tag(TagInputs { local_alive: true, alive: true, seen: true, ..default() }), None);
    // In the game.
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = other(&mut sim, me, 5.0, 2, "Foe");
    look_at(&mut sim, me, foe);
    let h = hud(&sim);
    assert_eq!(h.aim_target, Some(foe));
    let tag = h.tags.iter().find(|t| t.entity == foe).expect("tag shown");
    assert_eq!((tag.name.as_str(), tag.tag.name, tag.tag.hp_bar), ("Foe", TAG_ENEMY, false));
    sim.intent(me).yaw += 1.2;
    sim.ticks(1);
    assert!(hud(&sim).tags.iter().all(|t| t.entity != foe), "gone once the ray leaves");
}

#[test]
fn t65_hit_flash() {
    let base = TagInputs {
        local_alive: true,
        alive: true,
        aimed: true,
        seen: true,
        ..default()
    };
    assert_eq!(name_tag(TagInputs { flashing: true, ..base }).unwrap().name, [250, 80, 0, 200]);
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = veteran(&mut sim, me, 5.0, 2, "Foe");
    look_at(&mut sim, me, foe);
    shot(&mut sim, me, foe);
    sim.ticks(1);
    let colour = |sim: &Sim| hud(sim).tags.iter().find(|t| t.entity == foe).unwrap().tag.name;
    assert_eq!(colour(&sim), TAG_FLASH);
    sim.seconds(0.25);
    assert_eq!(colour(&sim), TAG_ENEMY);
}

#[test]
fn t66_teammate_behind_a_wall() {
    let base = TagInputs {
        local_alive: true,
        teammate: true,
        alive: true,
        ..default()
    };
    let t = name_tag(base).unwrap();
    assert_eq!((t.opacity, t.hp_bar, t.name), (125, false, TAG_MATE));
    let t = name_tag(TagInputs {
        aimed: true,
        seen: true,
        ..base
    })
    .unwrap();
    assert_eq!((t.opacity, t.hp_bar), (255, true));
    // In the game: a wall between.
    let (mut sim, me) = setup(&[RIFLE]);
    let mate = other(&mut sim, me, 6.0, 1, "Mate");
    look_at(&mut sim, me, mate);
    let tag = |sim: &Sim| hud(sim).tags.into_iter().find(|t| t.entity == mate).unwrap().tag;
    assert_eq!((tag(&sim).opacity, tag(&sim).hp_bar), (255, true), "aimed with clear sight");
    let wall_at = sim.position(me) + Vec3::new(0.0, 0.5, -3.0);
    sim.app.world_mut().spawn((
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(4.0, 4.0, 0.2),
        Transform::from_translation(wall_at),
    ));
    sim.ticks(3);
    assert_eq!((tag(&sim).opacity, tag(&sim).hp_bar), (125, false), "behind the wall, not aimed");
}

#[test]
fn t67_aiming_does_not_change_the_crosshair() {
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = other(&mut sim, me, 5.0, 2, "Foe");
    look_at(&mut sim, me, foe);
    let on = hud(&sim);
    assert_eq!(on.aim_target, Some(foe));
    sim.intent(me).yaw += 1.2;
    sim.ticks(1);
    let off = hud(&sim);
    assert_eq!(off.aim_target, None);
    assert_eq!((on.crosshair.elements, on.crosshair.colour), (off.crosshair.elements, off.crosshair.colour));
}

// ---------------------------------------------------------------------------
// Spread (B): the gap from r (r's stepping is the firing part's,
// weapon_firing.md, and its tests)

#[test]
fn t68_one_shot() {
    let r = 3.0 * (1.0 / 80.0) * 1.3;
    assert!(near(r, 0.04875, 1e-6));
    assert!(near(gap(3, 100, r, 1.0), 7.73, 0.005));
}

#[test]
fn t69_full_auto_cap() {
    assert!(near(gap(3, 100, 0.30, 1.0), 32.1, 1e-4));
}

#[test]
fn t70_turning_in_place() {
    let (mut sim, me) = setup(&[RIFLE]);
    let before = hud(&sim).crosshair;
    for _ in 0..20 {
        sim.intent(me).yaw += 0.3;
        sim.ticks(1);
        assert_eq!(hud(&sim).crosshair, before);
    }
}

// ---------------------------------------------------------------------------
// The HUD's reading of the firing part's data

#[test]
fn spread_read_back_from_the_cone() {
    let limits: toml::Value = toml::from_str(
        "cone = { min = 3, max = 100, inc = 1.3, dec = 0.3 }\nzoomed_cone = { min = 2, max = 30, inc = 1.3, dec = 0.3 }",
    )
    .unwrap();
    let hud = CaHudData {
        duck_perturb: 2,
        move_duck_perturb: 3,
        ..default()
    };
    // The firing part's cone n = 3 + floor(97 r): r back.
    let (p, r) = spread_of(Some(&limits), &hud, 51.0, Stance::default());
    assert_eq!(p, m4());
    assert!(near(r, 48.0 / 97.0, 1e-6));
    // Zoomed: its own limits.
    let (_, r) = spread_of(Some(&limits), &hud, 16.0, zoomed());
    assert!(near(r, 0.5, 1e-6));
    // Without the data: the cone is the gap.
    let (p, r) = spread_of(None, &hud, 7.0, Stance::default());
    assert_eq!((p.min, p.max, r), (7, 7, 0.0));
    assert_eq!(screen().centre(), Vec2::new(512.0, 384.0));
    let _ = HudScreen::default();
}

// ---------------------------------------------------------------------------
// The follow-up answers (71-77)

#[test]
fn t71_hit_indicator_and_feed_on_the_ms_clock() {
    let mut h = HitIndicator::default();
    h.hit(10000.0, true);
    assert!(h.frame(10500.0), "500 is not more than 500: still red");
    assert!(!h.frame(10516.0), "the next frame: green");
    let mut f = KillFeed::default();
    f.push(0.0, kill("K", "A"));
    f.frame(30000.0);
    assert_eq!(f.lines.len(), 1, "shown at 30000");
    f.frame(30016.0);
    assert!(f.lines.is_empty(), "gone on the first frame after");
    // In the game the indicator runs on real time in whole ms: still red
    // 0.49 s on, green after 0.52 s.
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = veteran(&mut sim, me, 5.0, 2, "Foe");
    shot(&mut sim, me, foe);
    sim.ticks(1);
    let start = sim.app.world().resource::<Time<Real>>().elapsed_secs_f64();
    while sim.app.world().resource::<Time<Real>>().elapsed_secs_f64() - start < 0.49 {
        assert_eq!(hud(&sim).crosshair.colour, HIT_CROSSHAIR);
        sim.ticks(1);
    }
    sim.seconds(0.03);
    assert_eq!(hud(&sim).crosshair.colour, DEFAULT_CROSSHAIR);
}

#[test]
fn t72_kick_on_the_game_clock() {
    // Fired at game time 12.000, a frame at 12.050: D 5, l 10 (case 7).
    let m = Measures::new(3, 100, 0.0, Some((12.050f64 - 12.000) as f32), 0, screen());
    assert!(near(m.d, 5.0, EPS) && near(m.l, 10.0, EPS), "{m:?}");
}

#[test]
fn t73_gap_unrounded() {
    assert!(near(gap(3, 100, 0.04875, 1.0), 7.72875, 1e-4));
    assert!(near(gap(3, 100, 0.30, 1.0), 32.1, 1e-4));
    // In the game: the M4A1's meter at one shot; the gap unrounded, the
    // next shot's cone rounded down.
    use combat_arms::game::weapons::{CaSpread, FireMode};
    let (mut sim, me) = setup(&["combat_arms:m4a1"]);
    let w = active(&sim, me);
    for (r, cone) in [(0.04875, 7.0), (0.30, 32.0)] {
        let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
        {
            let mut s = sim.app.world_mut().get_mut::<CaSpread>(w).unwrap();
            s.fire = r;
            s.movement = 0.0;
            s.last_shot = Some(now);
        }
        sim.ticks(1);
        let c = hud(&sim).crosshair;
        assert!(near(c.r, r, 1e-6), "r {}", c.r);
        assert!(near(c.measures.unwrap().g, 3.0 + r * 97.0, 1e-3), "gap {:?}", c.measures);
        assert_eq!(sim.app.world().get::<CaSpread>(w).unwrap().cone(FireMode::Normal), cone);
    }
}

#[test]
fn t74_name_tag_anchor() {
    // The skeleton's name socket (characters.md 3: the spec's stand pose
    // without a body model), turned with the character.
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = other(&mut sim, me, 5.0, 2, "Foe");
    look_at(&mut sim, me, foe);
    let tag = hud(&sim).tags.into_iter().find(|t| t.entity == foe).expect("tag shown");
    let world = sim.app.world();
    let pose = world.get::<combat_arms::game::characters::CaPose>(foe).unwrap();
    let state = world.get::<chimera_core::core::MovementState>(foe).unwrap();
    let want = pose.world(world.get::<Transform>(foe).unwrap(), state, inventory::UNIT, pose.name_tag.unwrap());
    assert!((tag.anchor - want).length() < 1e-5, "{} vs {want}", tag.anchor);
    let above = (want - sim.position(foe)).y / inventory::UNIT;
    assert!((above - 45.87).abs() < 1e-2, "45.87 u above the hull centre: {above}");
}

#[test]
fn t75_no_distance_limit() {
    assert!(tag_in_front(20000.0));
    assert!(!tag_in_front(-1.0) && !tag_in_front(0.0), "behind the camera: hidden");
    // A teammate far away still has its tag (opacity by visibility).
    let (mut sim, me) = setup(&[RIFLE]);
    let far = 20000.0 * inventory::UNIT;
    let mate = other(&mut sim, me, far, 1, "Mate");
    assert!(hud(&sim).tags.iter().any(|t| t.entity == mate), "shown far away");
}

#[test]
fn t76_fire_mode_icons_from_the_data() {
    // M16A3: primary, RapidFireCount 0, burst toggled (3 rounds).
    assert_eq!(fire_icon(1, None, 0, false), Some(FireIcon::Auto));
    assert_eq!(fire_icon(1, Some(3), 0, false), Some(FireIcon::Burst3));
    // M24 sniper (primary) and M92FS (secondary): RapidFireCount 1.
    assert_eq!(fire_icon(1, None, 1, false), Some(FireIcon::Single));
    assert_eq!(fire_icon(2, None, 1, false), Some(FireIcon::Single));
    assert_eq!(fire_icon(1, None, 1, true), Some(FireIcon::Auto), "continuous fire on");
    // RapidFireCount 3, no burst: none; the M67 (throw class): none.
    assert_eq!(fire_icon(1, None, 3, false), None);
    assert_eq!(fire_icon(4, None, 1, false), None);
}

#[test]
fn t77_panel_by_inventory_class() {
    let panel = |class, bomb, infinite, clip, total| {
        weapon_panel(
            &PanelInputs {
                name: "W".into(),
                class,
                bomb,
                infinite,
                clip,
                total,
                rapid_fire_count: 1,
                ..default()
            },
            true,
        )
    };
    let m67 = panel(4, false, false, 1, 1).expect("the M67's panel");
    assert_eq!(m67.ammo, Some(("001".into(), "000".into())));
    assert_eq!(m67.icon, None);
    let knife = panel(3, false, true, 0, 0).expect("the knife's panel");
    assert_eq!((knife.ammo, knife.icon), (None, None));
    assert_eq!(panel(1, true, false, 30, 120), None, "the mission bomb");
    assert_eq!(panel(0, false, false, 30, 120), None, "in no order list");
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-19): test cases 78-100 (92-94 in score.rs, 98's
// scenario in attachments.rs)

/// 78. The hit indicator from the shot test: the local player's own round
/// traced into an enemy's hit box turns the crosshair red on that frame,
/// with nothing from the server, and B's tag flashes.
#[test]
fn t78_hit_indicator_from_the_shot_test() {
    let mut h = HitIndicator::default();
    h.hit(1000.0, hit_lights(true, false));
    assert!(h.frame(1500.0) && !h.frame(1501.0));
    // In the game: a real M4A1 round at B, 5 m ahead.
    let (mut sim, me) = setup(&["combat_arms:m4a1"]);
    let foe = veteran(&mut sim, me, 5.0, 2, "B");
    look_at(&mut sim, me, foe);
    assert_eq!(hud(&sim).crosshair.colour, DEFAULT_CROSSHAIR);
    sim.intent(me).fire = true;
    let mut frames = 0;
    while hud(&sim).crosshair.colour != HIT_CROSSHAIR {
        sim.ticks(1);
        frames += 1;
        // The round on the 3rd update after the press (weapon_firing.md
        // Corrections 10), red on that one.
        assert!(frames < 5, "red on the frame the round is traced");
    }
    sim.intent(me).fire = false;
    let tag = hud(&sim).tags.into_iter().find(|t| t.entity == foe).expect("B's tag");
    assert_eq!(tag.tag.name, TAG_FLASH);
}

/// 79. The spawn window: a hit within 3500 OS ms of B's (re)spawn, before
/// B fires, leaves the crosshair green; past it, or once B has fired, red.
#[test]
fn t79_spawn_window() {
    let mut w = SpawnWindow::default();
    w.spawn(10000.0);
    assert!(!hit_lights(true, w.open(13400.0)), "13400: green");
    assert!(!hit_lights(true, w.open(13500.0)), "13500 is not more than 3500 later");
    assert!(hit_lights(true, w.open(13501.0)), "13501: red");
    w.fire();
    assert!(hit_lights(true, w.open(12100.0)), "fired at 12000: red at 12100");
    // In the game: B just appeared.
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = other(&mut sim, me, 5.0, 2, "B");
    shot(&mut sim, me, foe);
    sim.ticks(1);
    assert_eq!(hud(&sim).crosshair.colour, DEFAULT_CROSSHAIR, "inside B's window");
    // B fires: its window shuts.
    let weapon = active(&sim, foe);
    sim.app.world_mut().write_message(WeaponEvent {
        owner: foe,
        weapon,
        kind: WeaponEventKind::Thrown,
        replay: false,
    });
    sim.ticks(1);
    shot(&mut sim, me, foe);
    sim.ticks(1);
    assert_eq!(hud(&sim).crosshair.colour, HIT_CROSSHAIR, "B has fired");
    // A respawn opens it again, for 3.5 s.
    sim.app.world_mut().get_mut::<Health>(foe).unwrap().current = 0.0;
    sim.ticks(1);
    sim.app.world_mut().get_mut::<Health>(foe).unwrap().current = 1.0;
    sim.ticks(1);
    sim.seconds(0.6);
    shot(&mut sim, me, foe);
    sim.ticks(1);
    assert_eq!(hud(&sim).crosshair.colour, DEFAULT_CROSSHAIR, "respawned 0.6 s ago");
    sim.seconds(3.0);
    shot(&mut sim, me, foe);
    sim.ticks(1);
    assert_eq!(hud(&sim).crosshair.colour, HIT_CROSSHAIR, "3.6 s after");
}

/// 80. Not from damage: the local player's grenade and fire hurting B
/// light nothing.
#[test]
fn t80_not_from_damage() {
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = veteran(&mut sim, me, 5.0, 2, "B");
    look_at(&mut sim, me, foe);
    for kind in [DamageKind::Blast, DamageKind::Burn, DamageKind::Bullet] {
        sim.app.world_mut().write_message(Damage {
            target: foe,
            attacker: Some(me),
            amount: 0.1,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Chest,
            kind,
            weapon: Some("combat_arms:m67"),
            force: Vec3::ZERO,
        });
        sim.ticks(1);
        let h = hud(&sim);
        assert_eq!(h.crosshair.colour, DEFAULT_CROSSHAIR, "{kind:?}");
        let tag = h.tags.iter().find(|t| t.entity == foe).unwrap();
        assert_eq!(tag.tag.name, TAG_ENEMY, "no flash: {kind:?}");
    }
}

/// 81. The damage direction uses the view's pitch: looking down 60
/// degrees, shot from ahead and 45 degrees above -> behind (s 9); level
/// -> ahead (s 3).
#[test]
fn t81_damage_direction_with_pitch() {
    let down = chimera_core::core::Intent {
        pitch: -60f32.to_radians(),
        ..default()
    };
    let (f, rt) = combat_arms::ui::hud::view_axes(&down);
    assert!((f - Vec3::new(0.0, -0.8660, -0.5)).length() < 1e-3, "{f}");
    assert!((rt - Vec3::X).length() < 1e-6);
    let d = Vec3::new(0.0, -0.7071, 0.7071);
    assert!((f.dot(d) - 0.2588).abs() < 1e-3);
    let s = DamageFeedback::hit(0.0, 1.0, f, rt, d, 0.0).unwrap().s;
    assert!(near(s, 9.0, 1e-3), "{s}");
    let (f, rt) = combat_arms::ui::hud::view_axes(&default());
    assert!(near(DamageFeedback::hit(0.0, 1.0, f, rt, d, 0.0).unwrap().s, 3.0, 1e-3));
    // In the game: the same hit while looking down shows the arc behind
    // (t36's corners).
    let (mut sim, me) = setup(&[RIFLE]);
    sim.intent(me).pitch = -60f32.to_radians();
    sim.ticks(1);
    hurt(&mut sim, me, None, 0.01, d);
    sim.ticks(1);
    let v = hud(&sim).damage.expect("damage shown");
    close(v.corners[0], Vec2::new(342.8, 552.8), 0.5);
    close(v.corners[1], Vec2::new(680.8, 553.2), 0.5);
}

/// 82. Damage with no direction comes along the world's forward axis:
/// behind at yaw 0 (s 9), on the right turned right by pi/2 (s 12).
#[test]
fn t82_directionless_damage() {
    use combat_arms::ui::hud::{NO_DIRECTION, view_axes};
    let (f, rt) = view_axes(&default());
    assert!(near(DamageFeedback::hit(0.0, 1.0, f, rt, NO_DIRECTION, 0.0).unwrap().s, 9.0, 1e-3));
    let right = chimera_core::core::Intent {
        yaw: -std::f32::consts::FRAC_PI_2,
        ..default()
    };
    let (f, rt) = view_axes(&right);
    assert!(near(DamageFeedback::hit(0.0, 1.0, f, rt, NO_DIRECTION, 0.0).unwrap().s, 12.0, 1e-3));
    // In the game: a zero direction at yaw 0 -> behind, never left.
    let (mut sim, me) = setup(&[RIFLE]);
    sim.intent(me).pitch = 0.0;
    sim.ticks(1);
    hurt(&mut sim, me, None, 0.01, Vec3::ZERO);
    sim.ticks(1);
    let v = hud(&sim).damage.expect("damage shown");
    close(v.corners[0], Vec2::new(342.8, 552.8), 0.5);
}

/// 83. The overlay's first frame draws 253, the next 251; a round going
/// live clears it.
#[test]
fn t83_overlay_first_frame_and_a_new_round() {
    use combat_arms::game::rules::Phase;
    let (mut sim, me) = setup(&[RIFLE]);
    hurt(&mut sim, me, None, 0.01, Vec3::Z);
    sim.ticks(1);
    assert_eq!(hud(&sim).damage.unwrap().alpha, 253);
    sim.ticks(1);
    assert_eq!(hud(&sim).damage.unwrap().alpha, 251);
    sim.app.world_mut().resource_mut::<combat_arms::game::rules::CaMatch>().phase = Phase::Live;
    sim.ticks(1);
    assert!(hud(&sim).damage.is_none(), "gone when the round goes live");
}

/// 84. The heartbeat: silent when health first drops (OS t 10000), beats
/// at 11010 and 12020 with frames every 10 ms; low again at 20000 after a
/// break: a beat at once.
#[test]
fn t84_heartbeat() {
    let mut beat = Heartbeat::default();
    let mut beats = Vec::new();
    for t in (10000..=12490).step_by(10) {
        if beat.frame(t as f64, true) {
            beats.push(t);
        }
    }
    assert_eq!(beats, vec![11010, 12020]);
    for t in (12500..20000).step_by(10) {
        assert!(!beat.frame(t as f64, false));
    }
    assert!(beat.frame(20000.0, true), "low again: a beat on the first frame");
    // In the game: no beat on the first low frame.
    let (mut sim, me) = setup(&[RIFLE]);
    let before = hud(&sim).heartbeats;
    set_health(&mut sim, me, 30.0);
    assert_eq!(hud(&sim).heartbeats, before, "silent at first");
    sim.seconds(0.9);
    assert_eq!(hud(&sim).heartbeats, before);
    sim.seconds(0.2);
    assert_eq!(hud(&sim).heartbeats, before + 1);
}

/// effects_sounds.md Corrections 2026-10-22, 8: each low-health beat plays
/// a file of the heartbeat entry, in your head.
#[test]
fn heartbeat_plays_its_sound() {
    use combat_arms::game::effects::sounds::CaSound;
    #[derive(Resource, Default)]
    struct Sounds(Vec<CaSound>);
    let (mut sim, me) = setup(&[RIFLE]);
    sim.app
        .init_resource::<Sounds>()
        .add_systems(Update, |mut r: MessageReader<CaSound>, mut s: ResMut<Sounds>| s.0.extend(r.read().cloned()));
    set_health(&mut sim, me, 25.0);
    sim.seconds(2.05);
    let beats = sim
        .app
        .world()
        .resource::<Sounds>()
        .0
        .iter()
        .filter(|s| s.file == "ui_snd/heartbeat.wav" && s.at.is_none())
        .count();
    assert_eq!(beats, 2, "a beat about every second, local");
}

/// grenades.md Corrections 2026-10-22, 1: Combat Arms' screen draws the
/// flash's white on a cosine; the shell's own fade (CS:S's) stays linear.
#[test]
fn flash_overlay_fades_on_a_cosine() {
    use chimera_client::client::senses::{FlashFade, overlay_amounts, overlay_amounts_with};
    use chimera_core::core::Blinded;
    // T = 25 s: white until T/2 = 12.5 s.
    let b = Blinded {
        alpha: 1.0,
        fade_start: 10.0,
        end: 22.5,
    };
    let ca = FlashFade(combat_arms::game::grenades::blinded_alpha);
    for (t, white, linear) in [(0.0, 1.0, 1.0), (3.125, 0.924, 0.75), (6.25, 0.7071, 0.5), (8.333, 0.5, 0.333), (12.5, 0.0, 0.0)] {
        let now = 10.0 + t;
        let (w, after) = overlay_amounts_with(ca, Some(&b), now, true);
        assert!((w - white).abs() < 1e-3 && after == w, "Combat Arms at {t}: {w}");
        let (w, _) = overlay_amounts(Some(&b), now, true);
        assert!((w - linear).abs() < 1e-3, "the shell's default at {t}: {w}");
    }
    assert_eq!(overlay_amounts_with(ca, None, 0.0, true), (0.0, 0.0));
}

fn died(sim: &mut Sim, entity: Entity, attacker: Option<Entity>, kind: DamageKind, weapon: Option<&'static str>, hitgroup: Hitgroup) {
    sim.app.world_mut().write_message(Died {
        entity,
        attacker,
        damage: Damage {
            target: entity,
            attacker,
            amount: 1.0,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            hitgroup,
            kind,
            weapon,
            force: Vec3::ZERO,
        },
    });
    sim.ticks(1);
}

fn last_kill(sim: &Sim) -> Kill {
    hud(sim).feed.last().expect("a kill line").kill.clone()
}

/// 85. An own grenade: "Me [the grenade's icon] Me", the first name in the
/// killer's (own team's) colour.
#[test]
fn t85_own_grenade_kill() {
    let (mut sim, me) = setup(&[RIFLE]);
    died(&mut sim, me, Some(me), DamageKind::Blast, Some("combat_arms:m67"), Hitgroup::Chest);
    let k = last_kill(&sim);
    assert_eq!(k.killer, Some(("Me".into(), true)));
    assert_eq!(k.icon, KillIcon::Weapon(Some("combat_arms:m67".into())), "not the barrel");
    let items = feed_line(&k, width);
    assert!(matches!(&items[0], FeedItem::Text { x, text, colour } if *x == 5.0 && text == "Me" && *colour == [124, 190, 221, 150]));
    assert!(matches!(items.last(), Some(FeedItem::Text { text, colour, .. }) if text == "Me" && *colour == [124, 190, 221, 150]));
}

/// 86. Kill lines without a self-killer: a fall, an ownerless barrel, and
/// another kind keeping its weapon's icon.
#[test]
fn t86_icons_without_a_self_killer() {
    assert_eq!(kill_icon(false, false, DeathKind::Crush, None), KillIcon::Suicide);
    assert_eq!(kill_icon(true, false, DeathKind::Crush, None), KillIcon::Suicide, "a fall dealt by the victim");
    assert_eq!(kill_icon(false, false, DeathKind::Blast, None), KillIcon::Barrel);
    assert_eq!(kill_icon(false, false, DeathKind::Burn, None), KillIcon::Fire);
    assert_eq!(kill_icon(false, false, DeathKind::Poison, None), KillIcon::Poison);
    assert_eq!(kill_icon(false, false, DeathKind::Bleeding, None), KillIcon::Bleeding);
    assert_eq!(kill_icon(false, false, DeathKind::Other, Some("w".into())), KillIcon::Weapon(Some("w".into())));
    assert_eq!(kill_icon(true, true, DeathKind::Other, Some("w".into())), KillIcon::Npc);
    // In the game.
    let (mut sim, me) = setup(&[RIFLE]);
    died(&mut sim, me, None, DamageKind::Fall, None, Hitgroup::Generic);
    let k = last_kill(&sim);
    assert_eq!(k.icon, KillIcon::Suicide);
    let items = feed_line(&k, width);
    assert!(matches!(&items[0], FeedItem::Text { text, .. } if text == "Me"));
    let v = other(&mut sim, me, 5.0, 2, "V");
    died(&mut sim, v, None, DamageKind::Blast, None, Hitgroup::Generic);
    let k = last_kill(&sim);
    assert_eq!((k.killer.clone(), k.icon.clone()), (None, KillIcon::Barrel));
    let items = feed_line(&k, width);
    assert!(matches!(&items[0], FeedItem::Text { text, colour, .. } if text == "V" && *colour == feed_colour(false)));
    died(&mut sim, v, None, DamageKind::Generic, Some("combat_arms:m4a1"), Hitgroup::Generic);
    assert_eq!(last_kill(&sim).icon, KillIcon::Weapon(Some("combat_arms:m4a1".into())));
}

/// 87. The headshot badge without a named killer.
#[test]
fn t87_headshot_without_a_killer() {
    assert_eq!(kill_badges(false, true, false, false, false), vec![Badge::Headshot]);
    let (mut sim, me) = setup(&[RIFLE]);
    let v = other(&mut sim, me, 5.0, 2, "V");
    died(&mut sim, v, None, DamageKind::Bullet, None, Hitgroup::Head);
    let k = last_kill(&sim);
    assert_eq!((k.killer, k.badges), (None, vec![Badge::Headshot]));
}

/// 88. The badge slots: the second at the last badge's x.
#[test]
fn t88_badge_slots() {
    let badges = |b: Vec<Badge>| {
        let items = feed_line(&Kill { badges: b, ..kill("K", "V") }, width);
        let slots: Vec<(f32, Badge)> = items
            .iter()
            .filter_map(|i| match i {
                FeedItem::Badge { x, badge } => Some((*x, *badge)),
                _ => None,
            })
            .collect();
        let victim = items
            .iter()
            .find_map(|i| match i {
                FeedItem::Text { x, text, .. } if text == "V" => Some(*x),
                _ => None,
            })
            .unwrap();
        (slots, victim)
    };
    let x_b = 5.0 + 10.0 + FEED_ICON_WIDTH;
    let (slots, victim) = badges(vec![Badge::Revenge, Badge::Headshot, Badge::Groin]);
    assert_eq!(slots, vec![(x_b, Badge::Revenge), (x_b + 64.0, Badge::Groin)]);
    assert_eq!(victim, x_b + 96.0);
    let (slots, victim) = badges(vec![Badge::Revenge, Badge::Headshot, Badge::Groin, Badge::Flag]);
    assert_eq!(slots[1], (x_b + 96.0, Badge::Flag));
    assert_eq!(victim, x_b + 128.0);
}

/// 89. The flag badge's image: the enemy flag for a victim on the local
/// player's team, the team flag otherwise.
#[test]
fn t89_flag_badge_image() {
    assert_eq!(kill_badges(false, false, false, true, true), vec![Badge::Flag]);
    assert_eq!(kill_badges(false, false, false, true, false), vec![Badge::TeamFlag]);
    let mut art = combat_arms::ui::hud::BoundHud::default();
    art.textures.insert("killenemyflagimg".into(), "enemy_flag.dtx".into());
    art.textures.insert("killteamflagimg".into(), "team_flag.dtx".into());
    assert_eq!(art.badge(Badge::Flag).as_deref(), Some("enemy_flag.dtx"));
    assert_eq!(art.badge(Badge::TeamFlag).as_deref(), Some("team_flag.dtx"));
    // In the game: the victim carried a flag.
    use combat_arms::game::rules::objectives::CaCarry;
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = other(&mut sim, me, 5.0, 2, "Foe");
    sim.app.world_mut().entity_mut(foe).insert(CaCarry {
        flag: Some(1),
        ..default()
    });
    died(&mut sim, foe, Some(me), DamageKind::Bullet, None, Hitgroup::Chest);
    assert_eq!(last_kill(&sim).badges, vec![Badge::TeamFlag]);
}

/// 90. A dead player's tag shows to everyone, over its body, in the dead
/// colour, while the body is there.
#[test]
fn t90_dead_players_tags() {
    let dead = TagInputs {
        local_alive: true,
        alive: false,
        seen: true,
        body_visible: true,
        ..default()
    };
    let t = name_tag(dead).expect("shown without aiming");
    assert_eq!((t.name, t.opacity, t.rank), (TAG_DEAD, 255, false));
    assert_eq!(name_tag(TagInputs { body_visible: false, ..dead }), None, "no body: no tag");
    assert!(name_tag(TagInputs { local_alive: false, ..dead }).is_some(), "the local player dead too");
    // In the game: B killed 10 m ahead, not aimed at.
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = other(&mut sim, me, 10.0, 2, "B");
    sim.intent(me).yaw += 1.2;
    sim.ticks(1);
    sim.app.world_mut().write_message(Damage {
        target: foe,
        attacker: Some(me),
        amount: 5.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(3);
    let body = {
        let w = sim.app.world_mut();
        let mut q = w.query::<(Entity, &combat_arms::game::characters::CaBody)>();
        q.iter(w).find(|(_, b)| b.owner == foe).map(|(e, b)| (e, b.at)).expect("a body")
    };
    let tag = hud(&sim).tags.into_iter().find(|t| t.entity == foe).expect("B's tag over its body");
    assert_eq!(tag.tag.name, TAG_DEAD);
    assert!((tag.anchor - body.1).length() < 1e-5);
    sim.app.world_mut().despawn(body.0);
    sim.ticks(1);
    assert!(hud(&sim).tags.iter().all(|t| t.entity != foe), "the body gone: no tag");
}

/// 91. The rank icon: on the aimed living player only.
#[test]
fn t91_rank_icon() {
    let mate = TagInputs {
        local_alive: true,
        teammate: true,
        alive: true,
        seen: true,
        ..default()
    };
    assert!(name_tag(TagInputs { aimed: true, ..mate }).unwrap().rank);
    assert!(!name_tag(mate).unwrap().rank);
    let corpse = TagInputs {
        aimed: true,
        alive: false,
        body_visible: true,
        ..mate
    };
    assert!(!name_tag(corpse).unwrap().rank);
    let (mut sim, me) = setup(&[RIFLE]);
    let m = other(&mut sim, me, 5.0, 1, "Mate");
    look_at(&mut sim, me, m);
    assert!(hud(&sim).tags.iter().find(|t| t.entity == m).unwrap().tag.rank);
}

/// 95. The pickup prompt: the string table's `IDS_HUD_ITEMGET_MSG`
/// centred on `ActionDesc` (bound.rs), for 100 ms after the aim leaves;
/// none under the scope overlay.
#[test]
fn t95_pickup_prompt() {
    let mut m = ActionMessage::default();
    m.request("IDS_HUD_ITEMGET_MSG", 1000.0);
    assert_eq!(m.frame(1100.0), Some("IDS_HUD_ITEMGET_MSG"));
    assert_eq!(m.frame(1101.0), None);
    assert_eq!(message_x(312, 400, 150), 437);
    let on = PickupInputs {
        draw_step: true,
        weapon_data: true,
        aiming_at_pickup: true,
        ..default()
    };
    assert_eq!(pickup_prompt(on, false), Some("IDS_HUD_ITEMGET_MSG"));
    assert_eq!(pickup_prompt(PickupInputs { scope_overlay: true, ..on }, false), None);
    assert_eq!(pickup_prompt(on, true), Some("IDS_HUD_Search_mybag"));
    // In the game (the view on a rifle, as the server reports it).
    use combat_arms::game::net::CaPickup;
    let (mut sim, me) = setup(&[RIFLE]);
    sim.app.world_mut().entity_mut(me).insert(CaPickup(Some(RIFLE.into())));
    sim.ticks(1);
    let a = hud(&sim).action.expect("the prompt");
    let ours = combat_arms::ui::CaUi::default().strings.resolve("IDS_HUD_ITEMGET_MSG");
    assert_eq!((a.id.as_str(), a.text.as_str()), ("IDS_HUD_ITEMGET_MSG", ours.as_str()));
    assert!(!a.text.contains("M4A1"), "no item name");
    sim.app.world_mut().entity_mut(me).insert(CaPickup(None));
    let left = now_ms(&sim);
    sim.ticks(1);
    while now_ms(&sim) - left <= 100.0 {
        assert!(hud(&sim).action.is_some(), "still shown {}", now_ms(&sim) - left);
        sim.ticks(1);
    }
    sim.ticks(1);
    assert!(hud(&sim).action.is_none(), "gone after 100 ms");
    // The scope overlay up: none.
    let w = active(&sim, me);
    let mut modes = chimera_sim::weapon::AltModes::new(2, 0.3, false);
    modes.current = 1;
    sim.app.world_mut().entity_mut(w).insert((
        modes,
        chimera_sim::weapon::Zoom {
            fov: vec![20.0],
            max_speed: None,
            unzoom_after_shot: false,
            scope: true,
        },
    ));
    sim.app.world_mut().entity_mut(me).insert(CaPickup(Some(RIFLE.into())));
    sim.ticks(2);
    assert!(sim.app.world().get::<chimera_sim::weapon::Zoomed>(me).is_some_and(|z| z.scope));
    sim.ticks(10);
    assert!(hud(&sim).action.is_none(), "no prompt under the scope");
}

/// 96. The prompt's conditions: a mine in hand or infected, none; the
/// bomb prompts in Search & Destroy.
#[test]
fn t96_prompt_conditions() {
    let on = PickupInputs {
        draw_step: true,
        weapon_data: true,
        aiming_at_pickup: true,
        ..default()
    };
    assert_eq!(pickup_prompt(PickupInputs { mine: true, ..on }, false), None);
    assert_eq!(pickup_prompt(PickupInputs { infected: true, ..on }, false), None);
    assert_eq!(bomb_prompt(true, false, false, false, false), Some("IDS_HUD_BOMBDEFUSE_MSG"));
    assert_eq!(bomb_prompt(true, true, false, false, false), None, "a defuse under way");
    assert_eq!(bomb_prompt(false, false, true, false, true), Some("IDS_HUD_BOMBSET_MSG"));
    assert_eq!(bomb_prompt(false, false, true, false, false), Some("IDS_HUD_BOMBSELECT_MSG"));
    assert_eq!(bomb_prompt(false, false, true, true, true), None, "planting");
    // In the game: a mine in hand.
    use combat_arms::game::net::CaPickup;
    let (mut sim, me) = setup(&[RIFLE]);
    let w = active(&sim, me);
    sim.app.world_mut().entity_mut(w).insert(CaHudData { mine: true, ..default() });
    sim.app.world_mut().entity_mut(me).insert(CaPickup(Some(RIFLE.into())));
    sim.ticks(2);
    assert!(hud(&sim).action.is_none(), "a mine in hand");
    // Search & Destroy on a site: Bravo aiming at it -> defuse; Alpha with
    // the bomb in it, the round live -> plant (the bomb in hand or not).
    use combat_arms::game::rules::{
        CaMatch, Phase,
        objectives::BOMB,
        objects::{BombZone, CaMapObjects, ObjBox},
    };
    let (mut sim, me) = setup(&[RIFLE]);
    combat_arms::harness::with_mode(sim.app.world_mut(), "TMM");
    sim.ticks(2);
    let at = sim.position(me);
    sim.app.world_mut().resource_mut::<CaMapObjects>().bomb_zones = vec![BombZone {
        family: "Doobic".into(),
        number: 0,
        defusers: 2,
        can_defuse: true,
        defuse_time: 7.0,
        detonate_time: 40.0,
        area: ObjBox {
            centre: at + Vec3::new(0.0, 0.0, -3.0),
            half: Vec3::new(1.5, 1.5, 1.5),
        },
    }];
    sim.app.world_mut().entity_mut(me).insert(chimera_core::core::Team(2));
    sim.intent(me).pitch = -0.2;
    sim.ticks(2);
    let id = |sim: &Sim| hud(sim).action.map(|a| a.id);
    assert_eq!(id(&sim).as_deref(), Some("IDS_HUD_BOMBDEFUSE_MSG"), "Bravo aiming at a site");
    sim.app.world_mut().entity_mut(me).insert(chimera_core::core::Team(1));
    sim.app.world_mut().resource_mut::<CaMapObjects>().bomb_zones[0].area.centre = at;
    {
        let mut m = sim.app.world_mut().resource_mut::<CaMatch>();
        m.phase = Phase::Live;
        m.warmup = false;
        m.round_ends = None;
    }
    combat_arms::game::rules::objectives::give_weapon(sim.app.world_mut(), me, BOMB);
    sim.ticks(10);
    assert_eq!(id(&sim).as_deref(), Some("IDS_HUD_BOMBSELECT_MSG"), "the bomb not in hand");
}

/// 97. The action gauge in whole percent on its own timer; its titles the
/// string table's.
#[test]
fn t97_action_gauge() {
    let mut g = ActionGauge::default();
    let t0 = 100.0;
    assert_eq!(g.frame(Some(GaugeKind::Defuse), 7.0, t0), Some(0));
    assert_eq!(g.frame(Some(GaugeKind::Defuse), 7.0, t0 + 3.5), Some(50));
    assert_eq!(gauge_bar(50, 200), 100);
    assert_eq!(g.frame(Some(GaugeKind::Defuse), 7.0, t0 + 6.999), Some(99));
    assert_eq!(gauge_bar(99, 200), 198);
    assert_eq!(g.frame(Some(GaugeKind::Defuse), 7.0, t0 + 7.0), None, "hidden at the total");
    assert_eq!(gauge_percent(1.999, 4.0), Some(49), "not 49.975");
    assert_eq!(GaugeKind::Defuse.title(), "$GaugeBombDefuse");
    assert_eq!(GaugeKind::Plant.title(), "$GaugeBombPlant");
    // In the game: the local player's defuse.
    use combat_arms::{
        game::rules::objectives::{ActionKind, CaAction},
        ui::hud::score::CaScoreHud,
    };
    let (mut sim, me) = setup(&[RIFLE]);
    sim.app.world_mut().entity_mut(me).insert(CaAction {
        kind: ActionKind::Defuse,
        held: 0.0,
        length: 7.0,
    });
    sim.ticks(1);
    let start = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.seconds(3.5);
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    let g = sim.app.world().resource::<CaScoreHud>().gauge.clone().expect("the gauge");
    assert_eq!(g.percent, (100.0 * (now - start) / 7.0 + 1e-9) as u32);
    let ours = combat_arms::ui::CaUi::default().strings.resolve("$GaugeBombDefuse");
    assert_eq!(g.title, ours);
    sim.seconds(3.6);
    assert!(sim.app.world().resource::<CaScoreHud>().gauge.is_none(), "hidden after 7 s, still held");
}

/// 98. The gauge sight's images: prefix, step, ".dtx".
#[test]
fn t98_gauge_sight_images() {
    let l = "ui_hud/crosshair/left_";
    let r = "ui_hud/crosshair/right_";
    let clip = (100.0 * 14.0f32 / 30.0) as i32;
    assert_eq!(gauge_images(l, r, 67, clip), (format!("{l}70.dtx"), format!("{r}50.dtx")));
    assert_eq!(gauge_images(l, r, 4, 100).0, format!("{l}0.dtx"));
    assert_eq!(gauge_images(l, r, 95, 100), (format!("{l}100.dtx"), format!("{r}100.dtx")));
}

/// 99. The range readout's look: `SysFont13` at 20 px, its right end at x
/// 490 and its top at y 400 (scaled); a hit closer than 1 unit, empty.
#[test]
fn t99_range_readout_look() {
    let ui = combat_arms::ui::CaUi::default();
    let font = ui.fonts.get(RANGE_FONT).expect("the range font");
    assert_eq!(font.height, 20);
    let d = combat_arms::ui::hud::range_text(&ui.fonts, screen(), "50.0", RANGE_IN).unwrap();
    assert_eq!((d.rect.x1, d.rect.y0), (490, 400), "{:?}", d.rect);
    let big = Screen::new(1920.0, 1080.0);
    let d = combat_arms::ui::hud::range_text(&ui.fonts, big, "50.0", RANGE_IN).unwrap();
    // Within the layout's whole reference px (T4: the text's reference
    // width ⌊80 / 1.875⌋ = 42 aligned in the box, so 1.25 px over).
    assert!((d.rect.x1 as f32 - 918.75).abs() <= 1.5 && (d.rect.y0 as f32 - 562.5).abs() <= 1.0, "{:?}", d.rect);
    assert_eq!(d.font.height, 28, "trunc(20 x 1.40625)");
    assert_eq!(range_readout(Some(0.6), 3500.0, 0.0).0, "");
}

/// 100. The weapon panel's name is the item table's.
#[test]
fn t100_weapon_name_from_the_item_table() {
    let (mut sim, _) = setup(&[RIFLE]);
    assert_eq!(hud(&sim).weapon.unwrap().name, "M4A1", "the data's without an item name");
    sim.app
        .world_mut()
        .resource_mut::<combat_arms::ui::hud::ItemNames>()
        .0
        .insert(RIFLE.into(), "M4A1 Custom".into());
    sim.ticks(1);
    assert_eq!(hud(&sim).weapon.unwrap().name, "M4A1 Custom");
}

/// grenades.md case 55: the flash's white is drawn over the HUD, chat and
/// pop-ups over the white.
#[test]
fn grenades_t55_flash_over_the_hud() {
    use chimera_client::client::senses::FLASH_OVERLAY_Z;
    assert!(combat_arms::ui::hud::HUD_Z < FLASH_OVERLAY_Z);
    // The chat's lowest layer (41) and the in-game menu's (60).
    assert!(FLASH_OVERLAY_Z < 41);
}

/// 101. The kill feed names the local player by the `name` setting (the
/// profile's), never by the character's entity name, and a bot by its own.
#[test]
fn t101_feed_names_players_not_characters() {
    let (mut sim, me) = setup(&[RIFLE]);
    sim.app.world_mut().entity_mut(me).insert(Name::new("Character"));
    sim.app.world_mut().insert_resource(chimera_net::net::NetSettings {
        name: "Ace".into(),
        ..default()
    });
    let bot = other(&mut sim, me, 5.0, 2, "AI_Bravo_7");
    died(&mut sim, bot, Some(me), DamageKind::Bullet, None, Hitgroup::Chest);
    let k = last_kill(&sim);
    assert_eq!(k.killer, Some(("Ace".into(), true)));
    assert_eq!(k.victim, ("AI_Bravo_7".into(), false));
    died(&mut sim, me, Some(bot), DamageKind::Bullet, None, Hitgroup::Chest);
    let k = last_kill(&sim);
    assert_eq!(k.killer, Some(("AI_Bravo_7".into(), false)));
    assert_eq!(k.victim, ("Ace".into(), true));
}

/// 102. A playing minigame's state words its lines with the players' and
/// the weapons' names, and shows only while one plays.
#[test]
fn t102_minigame_state_on_the_hud() {
    use chimera_sim::minigame::hud::{Callout, HudLine, MinigameHud, ShownCallout};
    let (mut sim, me) = setup(&[RIFLE]);
    let foe = other(&mut sim, me, 5.0, 2, "AI_Bravo_7");
    assert_eq!(hud(&sim).minigame, None, "no minigame, nothing shown");
    sim.app
        .world_mut()
        .resource_mut::<combat_arms::ui::hud::ItemNames>()
        .0
        .insert(RIFLE.into(), "Rifle One".into());
    sim.app.world_mut().insert_resource(MinigameHud {
        title: "Gun Game".into(),
        lines: vec![
            HudLine::new("Rung 3/10: {weapon}").weapon(RIFLE),
            HudLine::new("Leader: {player}").player(foe),
        ],
        notices: vec![(HudLine::new("Rung up! {weapon}").weapon(RIFLE), 1e9)],
        callout: Some(ShownCallout {
            callout: Callout::new(2, "rung_up", vec![HudLine::new("Level up: {weapon}").weapon(RIFLE), HudLine::new("Level 3/10")]),
            since: 0.0,
            id: 1,
        }),
        ..default()
    });
    sim.ticks(1);
    let m = hud(&sim).minigame.expect("shown while one plays");
    assert_eq!(m.title, "Gun Game");
    assert_eq!(m.lines, ["Rung 3/10: Rifle One", "Leader: AI_Bravo_7"]);
    // The callout ours (the shell's hidden while this HUD is up), the
    // notices left to the panels that list them.
    assert_eq!(m.callout, ["Level up: Rifle One", "Level 3/10"]);
    assert!(m.results.is_empty(), "no results while the match plays");
    // A finished match's results: the winner (a character here, named as
    // the HUD names it; one from the network has its name in the row) and a
    // line a player.
    use chimera_sim::minigame::results::{MatchResults, ResultRow};
    let row = |name: &str, player, winner| ResultRow {
        name: name.into(),
        kills: 4,
        deaths: 1,
        rung: Some(10),
        winner,
        player,
        ..Default::default()
    };
    sim.app.world_mut().resource_mut::<MinigameHud>().results = Some(MatchResults {
        title: "Gun Game".into(),
        rungs: 10,
        rows: vec![row("Remote", None, true), row("ignored", Some(foe), false)],
    });
    sim.ticks(1);
    let m = hud(&sim).minigame.expect("still shown");
    assert_eq!(m.results[0], "Remote wins the match");
    assert_eq!(m.results[1], "1. Remote  rung 10/10  4 kills  1 deaths");
    assert_eq!(m.results[2], "2. AI_Bravo_7  rung 10/10  4 kills  1 deaths", "a local character by the HUD's name");
    // The shell's match-end screen up: the whole panel steps aside (its
    // title and status lines drew into the screen's winner).
    sim.app.world_mut().resource_mut::<MinigameHud>().results_shown = true;
    sim.ticks(1);
    assert_eq!(hud(&sim).minigame, None, "nothing over the match-end screen");
    sim.app.world_mut().resource_mut::<MinigameHud>().results_shown = false;
    sim.ticks(1);
    assert!(hud(&sim).minigame.is_some(), "back once the screen goes");
    sim.app.world_mut().insert_resource(MinigameHud::default());
    sim.ticks(1);
    assert_eq!(hud(&sim).minigame, None);
}

// ---------------------------------------------------------------------------
// game_modes.md Corrections (2026-10-30) 3: the infected's screen.

/// 111. The local player infected (a rung's here), even zoomed with a
/// scope overlay: the `InfectedTex` rect is drawn every frame, over the
/// whole 1024 x 768 layout at 1920 x 1080 without the install's numbers;
/// changed back: gone the next frame.
#[test]
fn t111_infected_overlay() {
    let (mut sim, me) = setup(&[RIFLE]);
    assert_eq!(hud(&sim).infected, None);
    sim.app.world_mut().entity_mut(me).insert(combat_arms::game::rungs::CaRungInfected);
    sim.ticks(1);
    let r = hud(&sim).infected.expect("drawn while infected");
    assert_eq!((r.min, r.max), (Vec2::ZERO, Vec2::new(1024.0, 768.0)), "1024 x 768 at the reference size");
    sim.app.world_mut().entity_mut(me).remove::<combat_arms::game::rungs::CaRungInfected>();
    sim.ticks(1);
    assert_eq!(hud(&sim).infected, None, "gone the next frame");
    let r = model::infected_rect(Screen::new(1920.0, 1080.0), [0, 0], [1024, 768]);
    assert_eq!((r.min, r.max), (Vec2::ZERO, Vec2::new(1920.0, 1080.0)), "x and y scaled on their own");
}

// ---------------------------------------------------------------------------
// hud_crosshair.md Corrections (2026-10-30): the dot-sight reticle.

/// 102. The reticle's gates: drawn during the draw (nothing tests the
/// weapon's state) and when spectating in first person (not dead in a
/// death camera); not dead in the chase camera; not under a scope overlay.
#[test]
fn t102_reticle_gates() {
    let v = model::Visibility {
        ok: true,
        weapon: true,
        dot_sight: true,
        ..Default::default()
    };
    assert_eq!(model::draws(v).dot_sight, 2, "drawn (before and after the lines)");
    assert_eq!(model::draws(model::Visibility { disabled: true, ..v }).dot_sight, 1, "before the DisableCrosshair test");
    assert_eq!(model::draws(model::Visibility { death_cam: true, ..v }).dot_sight, 0, "dead in the chase camera");
    assert_eq!(model::draws(model::Visibility { scope_overlay: true, ..v }).dot_sight, 0, "under a scope overlay");
}

/// 103. At 1920 x 1080 the 129 x 129 rect is stretched: x 840, y 450, w
/// 241, h 181.
#[test]
fn t103_reticle_stretch() {
    let r = model::dot_sight_rect(Screen::new(1920.0, 1080.0));
    assert_eq!((r.min.x, r.min.y, r.max.x - r.min.x, r.max.y - r.min.y), (840.0, 450.0, 241.0, 181.0));
}
