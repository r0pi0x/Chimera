//! The sandbox on the range (chimera_sim's `minigame::range_place` with
//! `targets`, `props` and `layout`; the range is `chimera_world::range`):
//! a target asked for at 5, 10, 20, 40, 100 or 200 m stands on that
//! distance line at the player's side whatever the view, is hit there by
//! real bullets (TF2's sniper rifle, hitscan; TF2's rocket launcher, a
//! projectile) and measured (`HitMeasured::distance_m` the true distance,
//! `line_m` the line); a second target on a taken spot is nudged aside; a
//! new distance keeps its side; a strafer stays in its lane, a walker near
//! home and off the firing line; one rocket among three targets is one
//! blast with three victims; a shot from the platform is measured; bottles
//! and cans go on the range's shelf and a board on its board (then their
//! own when those are taken) and real bullets knock and mark them; a
//! layout made on the range comes back exactly there, one made elsewhere
//! on its distance lines. On the party's simulation, headless.

use bevy::prelude::*;
use chimera_core::{console::Console, core::LocalPlayer};
use chimera_sim::{
    minigame::{
        self,
        layout::{LayoutTarget, RepeatLayout, SandboxLayout},
        props,
        targets::{
            self, BlastMeasured, DISTANCES, HitMeasured, Prop, PropSpec, RangeLine, STRAFE_HALF, SetTarget, ShotGroup,
            SpawnProp, SpawnTarget, Target, TargetMotion, WALK_RADIUS,
        },
    },
    testkit,
    weapon::{
        WeaponEvent, WeaponEventKind,
        projectile::{Flyer, FlyerGone},
    },
};
use chimera_world::{greybox, harness::Sim, range::RangeLayout};
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

const GUN: &str = "combat_arms:m4a1";
const SNIPER: &str = "tf2:tf_weapon_sniperrifle";
const ROCKETS: &str = "tf2:tf_weapon_rocketlauncher";

#[derive(Resource, Default)]
struct Seen {
    hits: Vec<HitMeasured>,
    blasts: Vec<BlastMeasured>,
    /// What the guns did (for a failure's message).
    events: Vec<String>,
}

fn keep(
    mut seen: ResMut<Seen>,
    mut h: MessageReader<HitMeasured>,
    mut b: MessageReader<BlastMeasured>,
    mut w: MessageReader<WeaponEvent>,
    mut gone: MessageReader<FlyerGone>,
    flyers: Query<(&Flyer, &Transform)>,
    names: Query<&Name>,
) {
    for g in gone.read() {
        seen.events.push(format!("gone at {} expired {}", g.at, g.expired));
    }
    for (f, t) in &flyers {
        if f.ticks % 30 == 1 {
            seen.events.push(format!("flying at {} v {}", t.translation, f.velocity));
        }
    }
    seen.hits.extend(h.read().cloned());
    seen.blasts.extend(b.read().cloned());
    for e in w.read() {
        let line = match &e.kind {
            WeaponEventKind::Shot { to, hit, .. } => format!("shot to {to} hit {:?}", hit.map(|h| names.get(h).map(|n| n.to_string()).unwrap_or(format!("{h:?}")))),
            WeaponEventKind::ShotMade { .. } | WeaponEventKind::Fired { .. } => format!("{:?}", e.kind),
            _ => continue,
        };
        seen.events.push(line);
    }
}

/// The sandbox playing on the range with the local player `me` just
/// behind the firing line.
fn play() -> (Sim, Entity, RangeLayout) {
    let mut sim = ugg();
    let w = sim.app.world_mut();
    greybox::unload(w);
    greybox::select(w, "range");
    greybox::respawn(w);
    sim.ticks(2);
    let l = sim.app.world().get_resource::<RangeLayout>().cloned().expect("the range is in");
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(l.point(-1.5, 0.0) + Vec3::Y, movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.app.init_resource::<Seen>().add_systems(FixedPostUpdate, keep);
    sim.seconds(1.5);
    assert!(sim.app.world().get_resource::<RangeLayout>().is_some(), "still on the range");
    (sim, me, l)
}

fn run(sim: &mut Sim, line: &str) -> String {
    let before = sim.app.world().resource::<Console>().output.len();
    sim.app.world_mut().resource_mut::<Console>().submit(line.to_string());
    sim.ticks(3);
    sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

/// `me` just behind the firing line at `side`, looking along `yaw`.
fn stand(sim: &mut Sim, me: Entity, l: &RangeLayout, side: f32, yaw: f32) {
    // Half a metre behind the line: clear of the platform beside the spawns.
    let at = l.point(-0.5, side) + Vec3::Y;
    chimera_sim::spawning::revive(sim.app.world_mut(), me, Some(Transform::from_translation(at)));
    chimera_core::core::set_view(sim.app.world_mut(), me, yaw, 0.0);
    sim.ticks(3);
}

fn add(sim: &mut Sim, req: SpawnTarget) -> Entity {
    targets::spawn(sim.app.world_mut(), &req).unwrap_or_else(|e| panic!("the target: {e}"))
}

fn target(sim: &Sim, e: Entity) -> Target {
    sim.app.world().get::<Target>(e).cloned().expect("a target")
}

fn frame(l: &RangeLayout, p: Vec3) -> Vec2 {
    minigame::range_place::along_side(l, p)
}

/// Fire `me`'s gun at `at` until a hit on `victim` is measured (or
/// `secs` pass); the hit.
fn shoot(sim: &mut Sim, me: Entity, victim: Entity, at: Vec3, secs: f64) -> Option<HitMeasured> {
    sim.app.world_mut().resource_mut::<Seen>().hits.clear();
    sim.app.world_mut().resource_mut::<Seen>().events.clear();
    testkit::aim_at(sim, me, at);
    let ticks = (secs * 66.0) as u32;
    for i in 0..ticks {
        sim.intent(me).fire = i % 20 < 2;
        sim.ticks(1);
        if let Some(h) = sim.app.world().resource::<Seen>().hits.iter().find(|h| h.victim == victim && h.attacker == Some(me)) {
            let h = h.clone();
            sim.intent(me).fire = false;
            return Some(h);
        }
    }
    sim.intent(me).fire = false;
    None
}

/// Where `me` aims TF2's rocket launcher for its rocket to land at target
/// `e`'s feet. The rocket flies from its start (23.5 forward, 12 right, 3
/// down from the eye, inches here) toward what a 2000-unit trace from the
/// eye meets, else that trace's end (specs/tf2/weapons.md 5.1). Within
/// 2000 units the trace meets the ground at the feet: aim there. Farther
/// (the 100 and 200 m lines) the trace meets nothing, the rocket heads for
/// its end and crosses the crosshair there, so aiming at the feet sends it
/// over them to land beyond: aim instead where the start-to-feet line is
/// 2000 units from the eye (the start moves with the view: a few rounds).
fn rocket_aim(sim: &Sim, me: Entity, e: Entity) -> Vec3 {
    const U: f32 = 0.0254;
    let feet = target(sim, e).home;
    let eye = sim.position(me) + sim.state(me).eye_offset;
    let reach = 2000.0 * U;
    if eye.distance(feet) <= reach {
        return feet;
    }
    let mut at = feet;
    for _ in 0..8 {
        let v = (at - eye).normalize();
        let right = v.cross(Vec3::Y).normalize();
        let up = right.cross(v);
        let start = eye + (v * 23.5 + right * 12.0 - up * 3.0) * U;
        // `start + t (feet - start)` at `reach` from the eye: the far root.
        let (d, o) = (feet - start, start - eye);
        let (a, b, c) = (d.dot(d), 2.0 * o.dot(d), o.dot(o) - reach * reach);
        let t = (-b + (b * b - 4.0 * a * c).sqrt()) / (2.0 * a);
        at = start + d * t;
    }
    at
}

fn on_rung(sim: &mut Sim, me: Entity, gun: &str) {
    run(sim, &format!("rung {gun}"));
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(gun), "{gun} in hand");
}

#[test]
fn every_distance_stands_on_its_line_and_is_hit_and_measured_by_a_hitscan_and_a_projectile_gun() {
    let (mut sim, me, l) = play();
    let sides = [-10.0, -6.0, -2.0, 2.0, 6.0, 10.0];
    let mut placed = Vec::new();
    for (d, side) in DISTANCES.iter().zip(sides) {
        // Looking well off the axis: off the range 200 m this way is
        // inside a side wall; here it is the 200 m line.
        stand(&mut sim, me, &l, side, 0.3);
        let e = add(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: *d, protected: true, ..default() });
        placed.push((e, *d, side));
    }
    sim.seconds(2.0);
    for (e, d, side) in &placed {
        let t = target(&sim, *e);
        let at = frame(&l, t.home);
        assert!((at.x - d).abs() < 0.05 && (at.y - side).abs() < 0.05, "{d} m at side {side}: {at:?}");
        assert!((t.origin - (l.point(0.0, *side) + Vec3::Y)).length() < 0.05, "origin on the firing line: {:?}", t.origin);
        assert_eq!((t.dir, t.distance_m), (l.dir, *d));
        assert_eq!(sim.app.world().get::<RangeLine>(*e).copied(), Some(RangeLine { distance_m: *d, side_m: *side }));
        let body = frame(&l, sim.position(*e));
        assert!((body - at).length() < 0.5, "the body at home: {body:?} vs {at:?}");
    }
    for gun in [SNIPER, ROCKETS] {
        on_rung(&mut sim, me, gun);
        for (e, d, side) in &placed {
            stand(&mut sim, me, &l, *side, 0.0);
            // A rocket at its feet (headless, a body has no hitboxes for a
            // flyer to meet: its splash measures it); a bullet at its centre.
            let centre = if gun == ROCKETS { rocket_aim(&sim, me, *e) } else { sim.position(*e) };
            let flight = if gun == ROCKETS { d / 20.0 + 3.0 } else { 3.0 };
            let hit = shoot(&mut sim, me, *e, centre, flight as f64)
                .unwrap_or_else(|| panic!("{gun} at {d} m: no hit measured; {:?}", sim.app.world().resource::<Seen>().events));
            let true_distance = sim.position(me).distance(sim.position(*e));
            assert!((hit.distance_m - true_distance).abs() < 0.5, "{gun} at {d} m: measured {} vs {true_distance}", hit.distance_m);
            assert!(hit.distance_m > *d, "{gun} at {d} m: the true distance is from behind the line ({})", hit.distance_m);
            assert_eq!(hit.line_m, Some(*d), "{gun} at {d} m");
            assert!(hit.damage > 0.0, "{gun} at {d} m: {hit:?}");
        }
    }
}

#[test]
fn a_taken_spot_nudges_a_new_target_aside_and_a_new_distance_keeps_its_side() {
    let (mut sim, me, l) = play();
    stand(&mut sim, me, &l, 3.0, -0.4);
    let a = add(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 40.0, ..default() });
    let b = add(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 40.0, ..default() });
    let (ta, tb) = (frame(&l, target(&sim, a).home), frame(&l, target(&sim, b).home));
    assert!((ta.y - 3.0).abs() < 0.05, "{ta:?}");
    assert!((tb.x - 40.0).abs() < 0.05 && (tb.y - ta.y).abs() >= 1.2 - 1e-3 && (tb.y - ta.y).abs() <= 1.6, "nudged aside: {tb:?}");
    // From the wall: clamped inside the range.
    stand(&mut sim, me, &l, l.half_width + 5.0, 0.0);
    let c = add(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 20.0, ..default() });
    let tc = frame(&l, target(&sim, c).home);
    assert!(tc.y < l.half_width - 0.5, "inside the range: {tc:?}");
    // A new distance: the same side, the new line.
    sim.app.world_mut().write_message(SetTarget { distance_m: Some(100.0), ..SetTarget::of(b) });
    sim.seconds(1.5);
    let t = target(&sim, b);
    let at = frame(&l, t.home);
    assert!((at.x - 100.0).abs() < 0.05 && (at.y - tb.y).abs() < 0.05, "{at:?}");
    assert_eq!(t.distance_m, 100.0);
    assert!((frame(&l, sim.position(b)) - at).length() < 0.5, "moved there");
}

#[test]
fn a_strafer_keeps_its_lane_and_a_walker_stays_near_home_off_the_firing_line() {
    let (mut sim, me, l) = play();
    // Near a side wall: the strafe's reach is kept inside it.
    stand(&mut sim, me, &l, l.half_width - 1.0, 0.0);
    let s = add(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 40.0, motion: TargetMotion::Strafes, ..default() });
    stand(&mut sim, me, &l, -2.0, 0.0);
    let w = add(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 5.0, motion: TargetMotion::Walks, ..default() });
    sim.seconds(1.5);
    let (hs, hw) = (frame(&l, target(&sim, s).home), frame(&l, target(&sim, w).home));
    assert!(hs.y + STRAFE_HALF < l.half_width, "the strafe fits inside the wall: {hs:?}");
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    let mut track = Vec::new();
    for _ in 0..60 {
        sim.seconds(0.25);
        let ps = frame(&l, sim.position(s));
        track.push(ps);
        assert!(ps.y.abs() < l.half_width - 0.3, "inside the walls: {ps:?}");
        (lo, hi) = (lo.min(ps.y), hi.max(ps.y));
        let pw = frame(&l, sim.position(w));
        assert!((pw - hw).length() < WALK_RADIUS + 1.5 + 0.3, "the walker near home: {pw:?} vs {hw:?}");
        assert!(pw.x > 0.3, "the walker off the firing line: {pw:?}");
    }
    assert!(hi - lo > 2.0 * STRAFE_HALF - 1.5, "it strafed across the lane: {lo}..{hi}");
    for ps in &track {
        assert!((ps.x - 40.0).abs() < 1.0, "the strafer stays on its line: {ps:?} home {hs:?} track {track:?}");
    }
}

#[test]
fn a_rocket_among_three_targets_and_a_shot_from_the_platform_are_measured() {
    let (mut sim, me, l) = play();
    stand(&mut sim, me, &l, 0.0, 0.0);
    let three: Vec<Entity> =
        (0..3).map(|_| add(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 20.0, protected: true, ..default() })).collect();
    sim.seconds(2.0);
    let sides: Vec<f32> = three.iter().map(|e| frame(&l, target(&sim, *e).home).y).collect();
    assert!((sides[1] - sides[0]).abs() >= 1.2 - 1e-3 && (sides[2] - sides[0]).abs() >= 1.2 - 1e-3, "{sides:?}");
    on_rung(&mut sim, me, ROCKETS);
    sim.app.world_mut().resource_mut::<Seen>().blasts.clear();
    let middle = rocket_aim(&sim, me, three[0]);
    shoot(&mut sim, me, three[0], middle, 4.0).expect("the rocket hit");
    sim.ticks(2);
    let blasts = sim.app.world().resource::<Seen>().blasts.clone();
    let b = blasts.iter().find(|b| b.attacker == Some(me)).unwrap_or_else(|| panic!("a blast: {blasts:?}"));
    for e in &three {
        assert!(b.hits.iter().any(|h| h.victim == *e && h.damage > 0.0), "{e:?} in the blast: {b:?}");
    }
    // From the platform's top: a hit at 100 m measured from up there.
    let far = add(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 100.0, protected: true, ..default() });
    chimera_sim::spawning::revive(sim.app.world_mut(), me, Some(Transform::from_translation(l.platform_top + Vec3::Y)));
    sim.seconds(1.0);
    assert!((sim.position(me).y - l.platform_top.y) > 0.5, "on the platform: {:?}", sim.position(me));
    on_rung(&mut sim, me, SNIPER);
    let centre = sim.position(far);
    let hit = shoot(&mut sim, me, far, centre, 4.0).expect("a hit from the platform");
    let true_distance = sim.position(me).distance(sim.position(far));
    assert!((hit.distance_m - true_distance).abs() < 0.5 && hit.line_m == Some(100.0), "{hit:?}");
}

fn prop(sim: &mut Sim, kind: &str, distance_m: f32) -> Entity {
    let e = props::spawn(sim.app.world_mut(), &SpawnProp { spec: PropSpec { kind: kind.into(), distance_m }, by: None })
        .unwrap_or_else(|e| panic!("the prop: {e}"));
    sim.ticks(2);
    e
}

fn prop_of(sim: &Sim, e: Entity) -> Prop {
    sim.app.world().get::<Prop>(e).cloned().expect("a prop")
}

#[test]
fn props_go_on_the_ranges_shelf_and_board_then_their_own_and_real_bullets_knock_and_mark_them() {
    let (mut sim, me, l) = play();
    stand(&mut sim, me, &l, 0.0, 0.0);
    let spots = &l.prop_spots;
    let mean = |r: std::ops::Range<usize>| spots[r.clone()].iter().copied().sum::<Vec3>() / r.len() as f32;
    let bottles = prop(&mut sim, "bottles", 30.0);
    let cans = prop(&mut sim, "tin_cans", 30.0);
    let board = prop(&mut sim, "target_board", 30.0);
    let (pb, pc, pd) = (prop_of(&sim, bottles), prop_of(&sim, cans), prop_of(&sim, board));
    assert!((pb.home - (mean(0..3) - Vec3::Y * 1.0)).length() < 0.05, "bottles on spots 1-3: {:?}", pb.home);
    assert!((pc.home - (mean(3..5) - Vec3::Y * 1.0)).length() < 0.05, "cans on spots 4-5: {:?}", pc.home);
    assert!((pd.home - (l.board - Vec3::Y * props::BOARD_CENTRE_Y)).length() < 0.05, "the range's board: {:?}", pd.home);
    assert!((pb.spec.distance_m - frame(&l, spots[0]).x).abs() < 0.05, "the shelf's distance: {}", pb.spec.distance_m);
    // Taken: a third row and a second board get their own on the line asked.
    let more = prop(&mut sim, "bottles", 30.0);
    let board2 = prop(&mut sim, "target_board", 25.0);
    let (pm, pd2) = (frame(&l, prop_of(&sim, more).home), frame(&l, prop_of(&sim, board2).home));
    assert!((pm.x - 30.0).abs() < 0.05 && (pd2.x - 25.0).abs() < 0.05, "{pm:?} {pd2:?}");
    // Real bullets: one bottle off the range's shelf, one on the range's board.
    on_rung(&mut sim, me, SNIPER);
    let standing = |sim: &Sim, e: Entity| prop_of(sim, e).standing;
    stand(&mut sim, me, &l, frame(&l, pb.home).y, 0.0);
    let piece = pb.home + Vec3::new(-0.625, 1.12, 0.0);
    testkit::aim_at(&mut sim, me, piece);
    for i in 0..200 {
        sim.intent(me).fire = i % 100 < 2;
        sim.ticks(1);
        if standing(&sim, bottles) < 6 {
            break;
        }
    }
    sim.intent(me).fire = false;
    assert_eq!(standing(&sim, bottles), 5, "a bottle knocked off the range's shelf");
    stand(&mut sim, me, &l, frame(&l, l.board).y, 0.0);
    testkit::aim_at(&mut sim, me, l.board + Vec3::new(0.1, 0.1, 0.0));
    for i in 0..200 {
        sim.intent(me).fire = i % 100 < 2;
        sim.ticks(1);
        if sim.app.world().get::<ShotGroup>(board).is_some_and(|g| !g.hits.is_empty()) {
            break;
        }
    }
    sim.intent(me).fire = false;
    let g = sim.app.world().get::<ShotGroup>(board).cloned().unwrap();
    assert_eq!(g.hits.len(), 1, "{g:?} {:?}", sim.app.world().resource::<Seen>().events);
    assert!((g.hits[0] - Vec2::new(0.1, 0.1)).length() < 0.1, "where it landed: {g:?}");
}

#[test]
fn a_layout_made_on_the_range_comes_back_there_and_one_made_elsewhere_goes_on_its_lines() {
    let (mut sim, me, l) = play();
    stand(&mut sim, me, &l, 5.0, 0.2);
    add(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 40.0, ..default() });
    stand(&mut sim, me, &l, -4.0, -0.3);
    add(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 100.0, motion: TargetMotion::Strafes, ..default() });
    prop(&mut sim, "bottles", 10.0);
    let homes = |sim: &mut Sim| -> Vec<Vec3> {
        let mut v: Vec<Vec3> = targets::all_targets(sim.app.world_mut()).into_iter().map(|(_, t)| t.home).collect();
        v.extend(props::all_props(sim.app.world_mut()).into_iter().map(|(_, p)| p.home));
        v
    };
    let before = homes(&mut sim);
    let layout = sim.app.world().resource::<SandboxLayout>().clone();
    assert_eq!(layout.targets.iter().map(|t| t.range_side).collect::<Vec<_>>(), [Some(5.0), Some(-4.0)]);
    // Elsewhere on the range, turned: exactly where they were.
    stand(&mut sim, me, &l, -15.0, 0.9);
    sim.app.world_mut().write_message(RepeatLayout::default());
    sim.seconds(1.5);
    let after = homes(&mut sim);
    assert_eq!(after.len(), 3);
    for (a, b) in before.iter().zip(&after) {
        assert!((*a - *b).length() < 0.05, "{a:?} vs {b:?}");
    }
    // A layout made elsewhere (relative to the player): on its lines.
    let mut other = layout.clone();
    other.props.clear();
    other.targets = vec![LayoutTarget { offset: Vec3::new(3.0, 0.0, 0.0), yaw: 0.5, distance_m: 20.0, range_side: None, ..layout.targets[0].clone() }];
    sim.app.world_mut().insert_resource(other);
    stand(&mut sim, me, &l, 0.0, 0.0);
    let said = run(&mut sim, "sandbox repeat");
    assert!(said.contains("1 placed"), "{said}");
    let t = targets::all_targets(sim.app.world_mut()).remove(0).1;
    let at = frame(&l, t.home);
    assert!((at.x - 20.0).abs() < 0.05 && (at.y - 3.0).abs() < 0.3, "the 20 m line at the spot's side: {at:?}");
}

