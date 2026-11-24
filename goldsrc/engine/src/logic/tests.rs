//! The logic world driven directly (specs/goldsrc/entities.md, "Test
//! cases"), with hand-made entities in entity space.

use bevy::math::Vec3;

use super::world::{Effect, Kind, NoSolid, Player, World};
use crate::map::{MapEntity, MapHull};

const DT: f64 = 0.01;

fn ent(keys: &[(&str, &str)]) -> MapEntity {
    MapEntity {
        keyvalues: keys.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        ..Default::default()
    }
}

/// A brush entity whose one volume is the box lo..hi.
fn brush(keys: &[(&str, &str)], lo: Vec3, hi: Vec3) -> MapEntity {
    let mut e = ent(keys);
    let planes = vec![
        (Vec3::X, hi.x),
        (Vec3::NEG_X, -lo.x),
        (Vec3::Y, hi.y),
        (Vec3::NEG_Y, -lo.y),
        (Vec3::Z, hi.z),
        (Vec3::NEG_Z, -lo.z),
    ];
    let mut points = Vec::new();
    for x in [lo.x, hi.x] {
        for y in [lo.y, hi.y] {
            for z in [lo.z, hi.z] {
                points.push(Vec3::new(x, y, z));
            }
        }
    }
    e.hulls = vec![MapHull { planes, points }];
    e.mover = true;
    e
}

fn player(origin: Vec3) -> Player {
    Player {
        id: 7,
        origin,
        mins: Vec3::new(-16.0, -16.0, -36.0),
        maxs: Vec3::new(16.0, 16.0, 36.0),
        eye: Vec3::new(0.0, 0.0, 28.0),
        alive: true,
        on_ground: true,
        ..Default::default()
    }
}

fn run(w: &mut World, players: &mut Vec<Player>, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        w.tick(DT, players, &NoSolid);
    }
}

fn fired(w: &World, name: &str) -> Vec<f64> {
    w.log.iter().filter(|(_, n)| n == name).map(|(t, _)| (t * 100.0).round() / 100.0).collect()
}

#[test]
fn cs_office_door_rises_108_units_in_2_7_seconds_and_returns() {
    let lo = Vec3::new(576.0, -2024.0, -336.0);
    let hi = Vec3::new(592.0, -1720.0, -160.0);
    let mut w = World::new(
        &[
            ent(&[("classname", "worldspawn")]),
            brush(
                &[
                    ("classname", "func_door"),
                    ("targetname", "door1"),
                    ("movedir", "-90 0 0"),
                    ("lip", "66"),
                    ("speed", "40"),
                    ("wait", "20"),
                    ("movesnd", "6"),
                    ("stopsnd", "1"),
                ],
                lo,
                hi,
            ),
        ],
        1,
    );
    let mut players = Vec::new();
    w.fire("door1", None, None, super::UseType::Toggle);
    run(&mut w, &mut players, 1.35);
    assert!((w.ents[1].origin.z - 54.0).abs() < 0.5, "{}", w.ents[1].origin);
    run(&mut w, &mut players, 1.36);
    assert_eq!(w.ents[1].origin, Vec3::new(0.0, 0.0, 108.0));
    assert!(w.effects.iter().any(|e| matches!(e, Effect::Sound { path, .. } if path == "doors/doorstop1.wav")));
    // A second fire while open: ignored.
    w.fire("door1", None, None, super::UseType::Toggle);
    run(&mut w, &mut players, 19.9);
    assert_eq!(w.ents[1].origin.z, 108.0, "open for 20 s");
    run(&mut w, &mut players, 0.2);
    assert!(w.ents[1].origin.z < 108.0, "closing after 20 s");
    run(&mut w, &mut players, 2.7);
    assert_eq!(w.ents[1].origin, Vec3::ZERO);
}

#[test]
fn a_toggle_button_shows_its_alternate_set_and_fires_after_its_delay() {
    let mut w = World::new(
        &[
            ent(&[("classname", "worldspawn")]),
            brush(
                &[("classname", "func_button"), ("spawnflags", "33"), ("delay", "1"), ("speed", "5"), ("target", "door1")],
                Vec3::new(-4.0, -4.0, 40.0),
                Vec3::new(4.0, 4.0, 56.0),
            ),
            ent(&[("classname", "info_target"), ("targetname", "door1")]),
        ],
        1,
    );
    // A player 40 units away looking at it.
    let mut p = player(Vec3::new(-40.0, 0.0, 20.0));
    p.view = Vec3::new(0.0, 0.0, 0.0);
    p.use_key = true;
    let mut players = vec![p];
    run(&mut w, &mut players, 0.01);
    assert_eq!(w.ents[1].frame, 1, "pressed: the +a set");
    assert!(fired(&w, "door1").is_empty(), "not yet");
    players[0].use_key = false;
    run(&mut w, &mut players, 1.0);
    assert_eq!(fired(&w, "door1"), vec![1.01]);
    players[0].use_key = true;
    run(&mut w, &mut players, 0.01);
    assert_eq!(w.ents[1].frame, 0, "second press: back out");
    run(&mut w, &mut players, 1.0);
    assert_eq!(fired(&w, "door1").len(), 2);
}

#[test]
fn a_named_door_does_not_open_by_touch_but_an_unnamed_one_does() {
    let door = |name: &str| {
        let mut keys = vec![("classname", "func_door"), ("movedir", "0 90 0"), ("speed", "100"), ("wait", "3")];
        if !name.is_empty() {
            keys.push(("targetname", name));
        }
        brush(&keys, Vec3::new(-32.0, -4.0, 0.0), Vec3::new(32.0, 4.0, 96.0))
    };
    for (name, opens) in [("", true), ("door1", false)] {
        let mut w = World::new(&[ent(&[("classname", "worldspawn")]), door(name)], 1);
        let mut players = vec![player(Vec3::new(0.0, -20.5, 37.0))];
        run(&mut w, &mut players, 0.5);
        assert_eq!(w.ents[1].origin != Vec3::ZERO, opens, "{name:?}");
    }
}

#[test]
fn a_rotating_door_swings_away_from_the_player() {
    // cs_militia *1: hinge 175 162 −105, a 64-unit door along +X.
    let mut e = brush(
        &[
            ("classname", "func_door_rotating"),
            ("origin", "175 162 -105"),
            ("distance", "90"),
            ("speed", "100"),
            ("wait", "3"),
        ],
        Vec3::new(-3.0, -3.0, -50.0),
        Vec3::new(61.0, 3.0, 50.0),
    );
    e.mover = true;
    let mut w = World::new(&[ent(&[("classname", "worldspawn")]), e], 1);
    // From the +Y side heading −Y (yaw −90), touching the door.
    let mut p = player(Vec3::new(175.0 + 30.0, 162.0 + 19.5, -105.0));
    p.view = Vec3::new(0.0, -90.0, 0.0);
    let mut players = vec![p];
    run(&mut w, &mut players, 0.95);
    assert!((w.ents[1].angles.y + 90.0).abs() < 1e-3, "{}", w.ents[1].angles);
    // Back closed 3 s + 0.9 s later.
    players.clear();
    run(&mut w, &mut players, 4.0);
    assert_eq!(w.ents[1].angles, Vec3::ZERO);
}

#[test]
fn doors_of_one_name_open_together_from_a_trigger() {
    let pane = |lo: Vec3, hi: Vec3, yaw: &str| {
        brush(
            &[("classname", "func_door"), ("targetname", "GlassDoor2"), ("movedir", yaw), ("lip", "-12"), ("speed", "50"), ("wait", "10")],
            lo,
            hi,
        )
    };
    let mut w = World::new(
        &[
            ent(&[("classname", "worldspawn")]),
            pane(Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 52.0, 92.0), "0 90 0"),
            pane(Vec3::new(0.0, -52.0, 0.0), Vec3::new(4.0, 0.0, 92.0), "0 270 0"),
            {
                let mut t = brush(
                    &[("classname", "trigger_multiple"), ("target", "GlassDoor2"), ("wait", "2")],
                    Vec3::new(-100.0, -100.0, 0.0),
                    Vec3::new(-50.0, 100.0, 100.0),
                );
                t.mover = false;
                t
            },
        ],
        1,
    );
    let mut players = vec![player(Vec3::new(-75.0, 0.0, 40.0))];
    run(&mut w, &mut players, 1.3);
    assert!((w.ents[1].origin.y - 62.0).abs() < 1e-3, "{}", w.ents[1].origin);
    assert!((w.ents[2].origin.y + 62.0).abs() < 1e-3, "{}", w.ents[2].origin);
    // Staying inside fires again every 2 s; the open doors ignore it.
    run(&mut w, &mut players, 2.0);
    assert_eq!(fired(&w, "GlassDoor2").len(), 2);
}

#[test]
fn the_clic_blinker_toggles_its_wall_every_half_second() {
    let mut w = World::new(
        &[
            ent(&[("classname", "worldspawn")]),
            {
                let mut t = brush(
                    &[("classname", "trigger_multiple"), ("target", "CLIC"), ("wait", "-1")],
                    Vec3::new(-50.0, -50.0, 0.0),
                    Vec3::new(50.0, 50.0, 100.0),
                );
                t.mover = false;
                t
            },
            brush(&[("classname", "func_wall"), ("targetname", "CLIC")], Vec3::new(200.0, 0.0, 0.0), Vec3::new(210.0, 10.0, 10.0)),
            ent(&[("classname", "trigger_relay"), ("targetname", "CLIC"), ("target", "CLIC"), ("triggerstate", "2"), ("delay", "0.5")]),
            ent(&[("classname", "light"), ("targetname", "CLIC"), ("style", "32")]),
            ent(&[("classname", "env_sprite"), ("targetname", "CLIC")]),
        ],
        1,
    );
    let mut players = vec![player(Vec3::new(0.0, 0.0, 40.0))];
    run(&mut w, &mut players, 0.01);
    assert_eq!(w.ents[2].frame, 1);
    assert_eq!(w.style(32), Some("a"));
    assert!(matches!(w.ents[5].kind, Kind::Sprite { on: true }));
    players.clear();
    let mut frames = Vec::new();
    for _ in 0..4 {
        run(&mut w, &mut players, 0.5);
        frames.push(w.ents[2].frame);
    }
    assert_eq!(frames, vec![0, 1, 0, 1]);
    assert!(w.ents[1].removed, "the once trigger is gone");
}

#[test]
fn a_multi_manager_fires_on_its_timetable_and_loops_through_a_relay() {
    // cs_747's pl_l_mm.
    let mut w = World::new(
        &[
            ent(&[("classname", "worldspawn")]),
            ent(&[("classname", "trigger_auto"), ("target", "pl_l_mm"), ("spawnflags", "1")]),
            ent(&[
                ("classname", "multi_manager"),
                ("targetname", "pl_l_mm"),
                ("l1", "0"),
                ("l1#1", "0.5"),
                ("l2", "1"),
                ("l2#1", "1.5"),
                ("pl_l_r", "3"),
            ]),
            ent(&[("classname", "trigger_relay"), ("targetname", "pl_l_r"), ("target", "pl_l_mm"), ("delay", "1")]),
        ],
        1,
    );
    let mut players = Vec::new();
    run(&mut w, &mut players, 4.2);
    assert_eq!(fired(&w, "l1"), vec![0.1, 0.6, 4.1]);
    assert_eq!(fired(&w, "pl_l_r"), vec![3.1]);
    assert_eq!(fired(&w, "pl_l_mm"), vec![0.1, 4.1]);
}

#[test]
fn a_multi_manager_keeps_sixteen_targets_sorted_by_delay() {
    let mut keys: Vec<(String, String)> = vec![("classname".into(), "multi_manager".into()), ("targetname".into(), "m".into())];
    for i in 0..17 {
        keys.push((format!("t{i}"), format!("{}", 20 - i)));
    }
    let e = MapEntity { keyvalues: keys, ..Default::default() };
    let w = World::new(&[e], 1);
    let Kind::MultiManager(m) = &w.ents[0].kind else { panic!() };
    assert_eq!(m.targets.len(), 16);
    assert_eq!(m.targets[0], ("t15".into(), 5.0));
}

#[test]
fn a_multisource_masters_a_trigger() {
    let mut w = World::new(
        &[
            ent(&[("classname", "worldspawn")]),
            ent(&[("classname", "multisource"), ("targetname", "ms")]),
            ent(&[("classname", "trigger_relay"), ("targetname", "in"), ("target", "ms")]),
        ],
        1,
    );
    let mut players = Vec::new();
    run(&mut w, &mut players, 0.2);
    assert!(!w.master_ok("ms"));
    w.fire("in", None, None, super::UseType::Toggle);
    assert!(w.master_ok("ms"));
    assert!(w.master_ok("nobody"));
}

#[test]
fn only_trigger_crates_break_when_fired_not_when_shot() {
    let crate_ = |name: &str| {
        brush(
            &[("classname", "func_breakable"), ("targetname", name), ("spawnflags", "1"), ("health", "1"), ("material", "2"), ("explodemagnitude", "100")],
            Vec3::ZERO,
            Vec3::splat(32.0),
        )
    };
    let mut w = World::new(
        &[
            ent(&[("classname", "worldspawn")]),
            crate_("tgt_a"),
            crate_("tgt_b"),
            brush(&[("classname", "func_bomb_target"), ("target", "tgt_a")], Vec3::splat(100.0), Vec3::splat(200.0)),
        ],
        1,
    );
    w.damage(1, 1000.0, crate::core::DamageKind::Bullet, Vec3::splat(-10.0));
    assert!(w.ents[1].solid && w.ents[1].remove_at.is_none(), "Only Trigger");
    w.fire_entity(3, None);
    assert!(!w.ents[1].solid && w.ents[2].solid);
    assert!(w.effects.iter().any(|e| matches!(e, Effect::Explosion { magnitude, .. } if *magnitude == 100.0)));
    let mut players = Vec::new();
    run(&mut w, &mut players, 0.11);
    assert!(w.ents[1].removed && !w.ents[2].removed);
}

#[test]
fn trigger_hurt_hurts_each_player_once_a_window() {
    let mut w = World::new(
        &[{
            let mut t = brush(
                &[("classname", "trigger_hurt"), ("dmg", "10")],
                Vec3::new(-100.0, -100.0, 0.0),
                Vec3::new(100.0, 100.0, 100.0),
            );
            t.mover = false;
            t
        }],
        1,
    );
    let mut a = player(Vec3::new(0.0, 0.0, 40.0));
    let mut b = a.clone();
    b.id = 8;
    a.id = 7;
    let mut players = vec![a];
    run(&mut w, &mut players, 0.1);
    players.push(b);
    run(&mut w, &mut players, 0.3);
    let hurts: Vec<(u64, f32)> = w
        .effects
        .iter()
        .filter_map(|e| match e {
            Effect::Damage { player, points, .. } => Some((*player, *points)),
            _ => None,
        })
        .collect();
    assert_eq!(hurts, vec![(7, 5.0), (8, 5.0)]);
}

#[test]
fn a_door_closing_on_a_player_reverses_and_hurts() {
    let mut w = World::new(
        &[brush(
            &[("classname", "func_door"), ("movedir", "0 90 0"), ("speed", "100"), ("wait", "1"), ("dmg", "10")],
            Vec3::new(0.0, -32.0, 0.0),
            Vec3::new(8.0, 32.0, 96.0),
        )],
        1,
    );
    // A wall behind the player (NoSolid would let it be shoved anywhere).
    struct Wall;
    impl super::world::Solid for Wall {
        fn blocked(&self, c: Vec3, h: Vec3) -> bool {
            c.y - h.y < -40.0
        }
    }
    w.use_entity(0, None, None, super::UseType::Toggle);
    let mut players = Vec::new();
    run(&mut w, &mut players, 0.7);
    assert!((w.ents[0].origin.y - 62.0).abs() < 1e-3);
    // Someone steps into the doorway; the door closes on him.
    players.push(player(Vec3::new(4.0, 0.0, 48.0)));
    for _ in 0..150 {
        w.tick(DT, &mut players, &Wall);
    }
    assert!(w.effects.iter().any(|e| matches!(e, Effect::Damage { points, kind: crate::core::DamageKind::Crush, .. } if *points == 10.0)));
    assert!(players[0].origin.y < 0.0, "shoved first: {}", players[0].origin);
    assert!(w.ents[0].origin.y > 30.0, "reversed: {}", w.ents[0].origin);
}
