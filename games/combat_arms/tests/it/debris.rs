//! Debris of destructibles: specs/combat_arms/effects_sounds.md Follow-up
//! (2026-10-31b) 1, cases 84-92 (`game::effects::debris`: the table, the
//! pieces' count, velocity, offsets, models and scale, fall, bounce, sky
//! and fade), and world_objects.md Follow-up (2026-10-31b) 3 to 5, cases
//! 48, 49, 51 and 52 (`WorldModelDebris` launched, its spin axis, `FIRE`,
//! `Spawn`); in play, a breakable shot to death sends its debris and every
//! client throws the pieces. The install's table (case 84) and the census
//! (case 50): `heavy::world_objects_2`.

use std::time::Duration;

use bevy::prelude::*;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        effects::debris::{
            CaDebrisPiece, CaDebrisTable, DebrisHit, DebrisLevel, Piece, PieceStep, axes, count, launch,
            velocity_bounds,
        },
        movement,
        objects::runtime::{Fx, Kind, spin_rate},
        world,
    },
    harness::CaSim,
};
use lithtech_formats::world::{Object, Value};

use crate::{
    world_objects::{T, close, wm},
    world_objects_play::{cuboid, prop, world_with},
};

fn table() -> CaDebrisTable {
    CaDebrisTable::built_in()
}

fn record(name: &str) -> combat_arms::game::effects::debris::DebrisRecord {
    let t = table();
    t.get(t.id(name).unwrap()).unwrap().clone()
}

/// A deterministic stream of draws.
fn draws(seed: u64) -> impl FnMut() -> f32 {
    let mut s = seed;
    move || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (s >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// 84. The table: six records, ids 0-5; `56x56WoodBox`'s `Rotate` on;
/// `Window` with three models and two bounce and two explode sounds;
/// `Stone`'s world velocity from (-50, 100, -50). Names case ignored.
#[test]
fn es84_debris_table() {
    let t = table();
    assert_eq!(t.0.len(), 6);
    let names: Vec<&str> = t.0.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["None", "Window", "Window_smoll", "Window_Large", "56x56WoodBox", "Stone"]);
    assert!(record("56x56woodbox").rotate != 0);
    let w = record("Window");
    assert_eq!((w.models.len(), w.bounce_sounds.len(), w.explode_sounds.len()), (3, 2, 2));
    assert_eq!(record("Stone").world_vel_min, [-50.0, 100.0, -50.0]);
    // The install's form, read as attribute records.
    let doc = lithtech_formats::attributes::Document::parse(
        br#"
[Debris0] Name="None"
[Debris1] Name="Window" SurfaceId=40 MinScale=0.5 MaxScale=1.2 MinVel=<-800.0,-500.0,-800.0> MaxVel=<800.0,800.0,800.0> Number=30 Rotate=1 MinBounce=10 MaxBounce=15 GravityScale=2.0 Alpha=0.7 Fadetime=3.0 MinLifetime=4.0 MaxLifetime=5.0 Model1="a.ltb" Model2="b.ltb" Skin0="s.dtx" BounceSnd1="g1.wav" BounceSnd2="g2.wav" ExplodeSnd1="e1.wav"
"#,
    )
    .unwrap();
    let parsed = CaDebrisTable::parse(&doc);
    assert_eq!(parsed.0.len(), 2);
    let r = &parsed.0[1];
    assert_eq!((r.name.as_str(), r.surface, r.number, r.bounce), ("Window", 40, 30, [10, 15]));
    assert_eq!(r.vel_min, [-800.0, -500.0, -800.0]);
    assert_eq!((r.models.len(), r.skins.len(), r.bounce_sounds.len(), r.explode_sounds.len()), (2, 1, 2, 1));
}

/// 85. Counts: `Window` (30) high 20, medium 13, low 6; `56x56WoodBox`
/// low 1; `None` one piece with nothing drawn or heard.
#[test]
fn es85_debris_count() {
    let n = record("Window").number;
    assert_eq!([count(n, DebrisLevel::High), count(n, DebrisLevel::Medium), count(n, DebrisLevel::Low)], [20, 13, 6]);
    assert_eq!(count(record("56x56WoodBox").number, DebrisLevel::Low), 1);
    let none = &record("None");
    let p = launch(none, DebrisLevel::High, Vec3::ZERO, Vec3::Z, &mut draws(1));
    assert_eq!(p.len(), 1);
    assert!(none.models.is_empty() && none.explode_sounds.is_empty() && none.bounce_sounds.is_empty());
}

/// 86. `56x56WoodBox` along (0, 0, 1): every velocity within (-150, 300,
/// -150)..(150, 500, 150); along (1, 0, 0) (right (0, 0, -1)) the bounds
/// are (-150, 300, 150) and (150, 500, -150), the same box.
#[test]
fn es86_debris_velocity_bounds() {
    let r = &record("56x56WoodBox");
    let (f, right, up) = axes(Vec3::Z);
    assert_eq!((f, right, up), (Vec3::Z, Vec3::X, Vec3::Y));
    assert_eq!(velocity_bounds(r, Vec3::Z), (Vec3::new(-150.0, 300.0, -150.0), Vec3::new(150.0, 500.0, 150.0)));
    let (lo, hi) = velocity_bounds(r, Vec3::X);
    assert!(lo.abs_diff_eq(Vec3::new(-150.0, 300.0, 150.0), 1e-4) && hi.abs_diff_eq(Vec3::new(150.0, 500.0, -150.0), 1e-4));
    for dir in [Vec3::Z, Vec3::X] {
        for p in launch(r, DebrisLevel::High, Vec3::ZERO, dir, &mut draws(7)) {
            assert!(p.v.x.abs() <= 150.0 + 1e-3 && p.v.z.abs() <= 150.0 + 1e-3, "{}", p.v);
            assert!((300.0 - 1e-3..=500.0 + 1e-3).contains(&p.v.y), "{}", p.v);
        }
    }
}

/// 87. `Window` pieces start at the death position + (10, 10, 10);
/// `Window_Large`'s within ±30 per axis.
#[test]
fn es87_debris_offsets() {
    let at = Vec3::new(5.0, 6.0, 7.0);
    for p in launch(&record("Window"), DebrisLevel::High, at, Vec3::Z, &mut draws(3)) {
        assert!(p.at.abs_diff_eq(at + Vec3::splat(10.0), 1e-4), "{}", p.at);
    }
    for p in launch(&record("Window_Large"), DebrisLevel::High, at, Vec3::Z, &mut draws(3)) {
        assert!((p.at - at).abs().max_element() <= 30.0 + 1e-4);
    }
}

/// 88. `Window`, 20 pieces: models 1, 2, 3 in turn; scales in [0.4,
/// 1.44]; alpha 0.7.
#[test]
fn es88_debris_models_and_scale() {
    let p = launch(&record("Window"), DebrisLevel::High, Vec3::ZERO, Vec3::Z, &mut draws(11));
    assert_eq!(p.len(), 20);
    for (i, x) in p.iter().enumerate() {
        assert_eq!(x.model, i % 3);
        assert!((0.4 - 1e-5..=1.44 + 1e-5).contains(&x.scale), "{}", x.scale);
        assert_eq!(x.alpha, 0.7);
    }
}

fn piece(v: Vec3) -> Piece {
    Piece {
        at: Vec3::ZERO,
        v,
        model: 0,
        scale: 1.0,
        alpha: 0.7,
        life: 4.5,
        age: 0.0,
        bounces: 10,
        rotate: true,
        angles: Vec3::new(0.3, 0.7, 0.2),
        spin: Vec3::ONE,
        resting: false,
        liquid: false,
    }
}

/// 89. A `56x56WoodBox` piece launched up at 400 u/s in open air, 60 fps:
/// after 12 frames it has risen 52.5 u and moves up at 100 u/s.
#[test]
fn es89_debris_fall() {
    let mut p = piece(Vec3::new(0.0, 400.0, 0.0));
    let g = record("56x56WoodBox").gravity;
    for _ in 0..12 {
        assert_eq!(p.step(g, 1.0 / 60.0, |_, _| None, &mut draws(1)), PieceStep::Moved);
    }
    close(p.at.y, 52.5, 1e-3, "height");
    close(p.v.y, 100.0, 1e-3, "speed");
}

/// A floor at y 0 (or a wall at x 0 with `wall`).
fn plane(wall: bool) -> impl Fn(Vec3, Vec3) -> Option<DebrisHit> {
    move |a: Vec3, b: Vec3| {
        let (pa, pb) = if wall { (a.x, b.x) } else { (a.y, b.y) };
        if (pa > 0.0) == (pb > 0.0) {
            return None;
        }
        let t = pa / (pa - pb);
        Some(DebrisHit {
            at: a + (b - a) * t,
            normal: if wall { Vec3::X } else { Vec3::Y },
            removes: false,
        })
    }
}

/// 90. A `Window` piece hitting a floor at (100, -300, 0) goes on at
/// (50, 150, 0), 0.3 u above the hit, one bounce less; at 60 fps a floor
/// hit at (0, -200, 0) leaves |v| 100 < 116: at rest, at its yaw only. A
/// wall hit doesn't cost a bounce.
#[test]
fn es90_debris_bounce() {
    let g = record("Window").gravity;
    // At 60 fps; the frame's gravity (½ x 2 x 1000 x dt) comes before the
    // bounce, so the piece hits the floor at (100, -300, 0).
    let dt = 1.0 / 60.0;
    let mut p = piece(Vec3::new(100.0, -300.0 + 1000.0 * dt, 0.0));
    p.at = Vec3::new(0.0, 4.0, 0.0);
    let out = p.step(g, dt, plane(false), &mut draws(2));
    assert!(matches!(out, PieceStep::Bounced { ground: true, .. }), "{out:?}");
    assert!(p.v.abs_diff_eq(Vec3::new(50.0, 150.0, 0.0), 1e-3), "{}", p.v);
    close(p.at.y, 0.3, 1e-3, "0.3 u off the floor");
    assert_eq!(p.bounces, 9);
    let mut p = piece(Vec3::new(0.0, -200.0, 0.0));
    p.at = Vec3::new(0.0, 1.0, 0.0);
    p.step(1e-6, 1.0 / 60.0, plane(false), &mut draws(2));
    assert!(p.resting, "at rest");
    assert_eq!((p.angles.x, p.angles.z), (0.0, 0.0), "lying at its yaw");
    let mut p = piece(Vec3::new(-300.0, 0.0, 0.0));
    p.at = Vec3::new(4.0, 50.0, 0.0);
    let out = p.step(g, dt, plane(true), &mut draws(2));
    assert!(matches!(out, PieceStep::Bounced { ground: false, .. }));
    assert_eq!(p.bounces, 10, "a wall costs nothing");
}

/// 91. A segment hitting the sky removes the piece.
#[test]
fn es91_debris_sky() {
    let mut p = piece(Vec3::new(0.0, -100.0, 0.0));
    p.at = Vec3::new(0.0, 1.0, 0.0);
    let sky = |a: Vec3, b: Vec3| plane(false)(a, b).map(|h| DebrisHit { removes: true, ..h });
    assert_eq!(p.step(1.0, 0.05, sky, &mut draws(2)), PieceStep::Removed);
}

/// 92. A `Window` piece with life 4.5 s and a 3 s fade: alpha 0.7 to 4.5
/// s, 0.5 at 6.0 s, 0.2 at 6.9 s, gone after 7.5 s.
#[test]
fn es92_debris_end() {
    let fade = record("Window").fade;
    let mut p = piece(Vec3::ZERO);
    for (t, a) in [(4.5, 0.7), (6.0, 0.5), (6.9, 0.2)] {
        p.age = t;
        close(p.alpha_now(fade), a, 1e-4, &format!("at {t} s"));
        assert!(!p.done(fade));
    }
    p.age = 7.5;
    assert!(p.done(fade), "gone at 7.5 s");
}

/// A breakable `B` with debris pieces `BDebris0` and `BDebris1`.
fn breakable_with_debris() -> T {
    T::new(vec![
        wm(
            "Breakable",
            "B",
            Vec3::new(0.0, 50.0, 0.0),
            Vec3::splat(10.0),
            &[("HitPoints", "10".into()), ("DebrisType", "Window".into())],
        ),
        wm("WorldModelDebris", "BDebris0", Vec3::new(0.0, 50.0, 0.0), Vec3::splat(5.0), &[("Rotation", "0 0.5 0.25".into())]),
        wm("WorldModelDebris", "BDebris1", Vec3::new(5.0, 50.0, 0.0), Vec3::splat(5.0), &[]),
        wm("WorldModelDebris", "Other", Vec3::new(5.0, 50.0, 0.0), Vec3::splat(5.0), &[]),
    ])
}

/// 48. `B` dies: both pieces launched (different draws), `BDebris2`
/// absent: done; the debris effect sent along the death direction. A
/// piece at (100, 200, -50) u/s is about (50, -100, -25) u away after 0.5
/// s, through the floor, its yaw and roll never changing, still there after
/// 60 s, non-solid and ray-hit.
#[test]
fn wo48_debris_launch() {
    let mut t = breakable_with_debris();
    t.frame();
    t.w.hit_dir = Vec3::X;
    let b = t.id("B");
    t.w.damage(b, 20.0, Default::default(), true);
    let flights: Vec<_> = t
        .w
        .objects
        .iter()
        .filter_map(|o| match &o.kind {
            Kind::Flight(f) => Some((o.name.clone(), f.v)),
            _ => None,
        })
        .collect();
    assert_eq!(flights.len(), 2, "{flights:?}");
    assert_ne!(flights[0].1, flights[1].1, "own draws");
    assert!(t.w.fx.contains(&Fx::Debris {
        at: Vec3::new(0.0, 50.0, 0.0),
        debris: "Window".into(),
        dir: Vec3::X
    }));
    let id = t.id("BDebris0");
    t.w.launch_debris_with(id, Vec3::new(100.0, 200.0, -50.0), Vec3::new(0.5, 0.0, 0.0));
    let start = t.pos("BDebris0");
    t.frames(10);
    let d = t.pos("BDebris0") - start;
    let want = Vec3::new(50.0, -100.0, -25.0);
    assert!((d - want).length() <= 0.1 * want.length(), "{d}");
    let o = &t.w.objects[id as usize];
    assert!(o.visible && !o.solid && o.rayhit);
    let Kind::Flight(f) = &o.kind else { panic!() };
    close(f.angles.y, 0.5, 1e-6, "yaw kept");
    close(f.angles.z, 0.25, 1e-6, "roll kept");
    t.frames(1200);
    assert!(!t.w.objects[id as usize].removed, "still there after 60 s");
}

/// 49. The spin axis: periods (0.5, 0, 0) → pitch 4π rad/s; (0.5, 0.25,
/// 0) → 8π; (0.5, 0.25, -1) → -2π; (0.0005, 0, 0) → none.
#[test]
fn wo49_debris_spin_axis() {
    use std::f32::consts::PI;
    close(spin_rate(Vec3::new(0.5, 0.0, 0.0)), 4.0 * PI, 1e-4, "x");
    close(spin_rate(Vec3::new(0.5, 0.25, 0.0)), 8.0 * PI, 1e-4, "y");
    close(spin_rate(Vec3::new(0.5, 0.25, -1.0)), -2.0 * PI, 1e-4, "z");
    assert_eq!(spin_rate(Vec3::new(0.0005, 0.0, 0.0)), 0.0);
}

/// 51. `FIRE` on a live destructible with `WeaponId` w: one shot of w,
/// straight down, going off in place; on a dead one: nothing.
#[test]
fn wo51_fire() {
    let mut t = T::new(vec![wm(
        "WorldModel",
        "Gun",
        Vec3::new(0.0, 100.0, 0.0),
        Vec3::splat(10.0),
        &[("WeaponId", "1225".into()), ("NeverDestroy", "0".into()), ("HitPoints", "10".into())],
    )]);
    t.send("Gun", "FIRE");
    let fires = |t: &T| t.w.fx.iter().filter(|f| matches!(f, Fx::Fire { .. })).cloned().collect::<Vec<_>>();
    assert_eq!(
        fires(&t),
        vec![Fx::Fire {
            weapon: 1225,
            at: Vec3::new(0.0, 100.0, 0.0),
            dir: Vec3::NEG_Y,
            in_place: true,
            factor: 1.0,
            shooter: 0
        }]
    );
    t.w.damage(0, 20.0, Default::default(), false);
    t.w.fx.clear();
    t.send("Gun", "FIRE");
    assert!(fires(&t).is_empty(), "a dead one doesn't fire");
}

/// 52. `Spawn` `Explosion (DamageRadius 300)` on a breakable: at its death
/// an `Explosion` exists at its place, doing nothing until `ON`.
#[test]
fn wo52_spawn() {
    let mut t = T::new(vec![wm(
        "Breakable",
        "B",
        Vec3::new(10.0, 20.0, 30.0),
        Vec3::splat(10.0),
        &[("HitPoints", "10".into()), ("Spawn", "Explosion (DamageRadius 300) (Name Boom)".into())],
    )]);
    t.w.damage(0, 20.0, Default::default(), false);
    let id = t.id("Boom");
    let o = &t.w.objects[id as usize];
    assert_eq!((o.class.as_str(), o.pose.pos), ("Explosion", Vec3::new(10.0, 20.0, 30.0)));
    assert!(matches!(o.kind, Kind::Explosion { radius: 300.0, .. }));
    assert!(!t.w.fx.iter().any(|f| matches!(f, Fx::Explode { .. })), "nothing yet");
    t.send("Boom", "ON");
    assert!(t.w.fx.iter().any(|f| matches!(f, Fx::Explode { radius: 300.0, .. })));
    // An unknown class makes nothing.
    let before = t.w.objects.len();
    t.w.spawn_object("NoSuchClass (A 1)", t.w.objects[0].pose);
    assert_eq!(t.w.objects.len(), before);
}

/// A crate (`Breakable`, `DebrisType` `56x56WoodBox`) in front of the
/// spawn.
fn crate_world() -> lithtech_formats::world::World {
    world_with(
        vec![cuboid("Crate", [-28.0, 0.0, 150.0], [28.0, 56.0, 206.0], false)],
        vec![Object {
            class: "Breakable".into(),
            properties: vec![
                prop("Name", Value::String("Crate".into())),
                prop("HitPoints", Value::Real(10.0)),
                prop("MaxHitPoints", Value::Real(10.0)),
                prop("RemoveOnDeath", Value::Bool(false)),
                prop("DebrisType", Value::String("56x56WoodBox".into())),
                prop("Solid", Value::Bool(true)),
                prop("Rayhit", Value::Bool(true)),
            ],
        }],
    )
}

fn shoot_the_crate(sim: &mut Sim, shooter: Entity) {
    let s = movement::movement_data().meters_per_unit;
    let at = Vec3::new(0.0, 28.0 * s, -178.0 * s);
    let d = at - (sim.position(shooter) + sim.state(shooter).eye_offset);
    sim.intent(shooter).pitch = d.y.atan2(d.xz().length());
    sim.intent(shooter).fire = true;
}

/// In play: the crate shot to death throws `56x56WoodBox` pieces (6 at
/// high detail) that fall onto the floor and are gone within life +
/// fade (2.2 s).
#[test]
fn a_broken_crate_throws_its_debris() {
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&crate_world(), "crate", s, &|_| None);
    let feet = map.spawns[0].0;
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.app.insert_resource(table());
    sim.ticks(3);
    let shooter = sim.spawn_character(feet + Vec3::Y, movement::ID);
    sim.seconds(1.5);
    shoot_the_crate(&mut sim, shooter);
    let mut seen = 0;
    for _ in 0..120 {
        sim.ticks(1);
        let w = sim.app.world_mut();
        seen = seen.max(w.query::<&CaDebrisPiece>().iter(w).count());
        if seen > 0 {
            break;
        }
    }
    sim.intent(shooter).fire = false;
    assert_eq!(seen, 6, "six pieces");
    sim.seconds(0.8);
    {
        let w = sim.app.world_mut();
        let ys: Vec<f32> = w.query::<&CaDebrisPiece>().iter(w).map(|p| p.piece.at.y).collect();
        assert!(ys.iter().all(|y| *y >= -0.01), "on or above the floor: {ys:?}");
    }
    sim.seconds(2.0);
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&CaDebrisPiece>().iter(w).count(), 0, "gone after life + fade");
}

/// Over the network: a client throws the pieces of a crate the server
/// breaks (the server sends the effect, each machine runs its pieces).
#[test]
fn clients_get_the_debris() {
    let s = movement::movement_data().meters_per_unit;
    let map = world::convert(&crate_world(), "crate", s, &|_| None);
    let link = LinkConditions {
        latency: Duration::from_millis(40),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 3, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(map.clone()));
        app.insert_resource(CaDebrisTable::built_in());
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(30);
    let crate_ = sim.server.app.world().resource::<combat_arms::game::objects::CaObjects>().named("Crate").unwrap();
    sim.server.app.world_mut().resource_mut::<combat_arms::game::objects::CaObjects>().world.hit_dir = Vec3::Z;
    sim.server.app.world_mut().resource_mut::<combat_arms::game::objects::CaObjects>().world.damage(crate_, 50.0, Default::default(), false);
    combat_arms::game::objects::apply_effects(sim.server.app.world_mut());
    let mut got = 0;
    for _ in 0..30 {
        sim.ticks(1);
        let w = sim.clients[0].app.world_mut();
        got = got.max(w.query::<&CaDebrisPiece>().iter(w).count());
    }
    assert_eq!(got, 6, "the client's pieces");
}
