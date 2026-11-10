//! What any game's gun does that every game shares (playtest 2026-10-24;
//! `chimera_sim::weapon::impact`), on the party's greybox: both games'
//! bullets push a physics object at the hit (one `BulletImpulse` each,
//! CS:S's from its measured damage force, Combat Arms' from its damage at
//! 2 N·s a hit point), and a swap to the other game's gun ends the old
//! game's recoil: the view punch is zeroed at the switch and the old
//! game's return doesn't drift the new game's view (both ways).

use avian3d::prelude::{Collider, LinearVelocity, Mass, RigidBody};
use bevy::prelude::*;
use chimera_sim::{
    minigame::ladder::{self, Ladder},
    testkit,
    weapon::{ViewPunch, impact::RecoilGame},
};
use chimera_world::{harness::Sim, map::PropSurface};
use combat_arms::game::weapons::CaKick;

use crate::gun_game::{held, party, place, soldier};

pub(crate) fn set_rung(sim: &mut Sim, e: Entity, r: usize) {
    let w = sim.app.world_mut();
    let l = w.resource::<Ladder>().clone();
    ladder::set_rung(w, e, r, &l);
}

fn punch(sim: &Sim, e: Entity) -> Vec3 {
    sim.app.world().get::<ViewPunch>(e).map_or(Vec3::ZERO, |p| p.0)
}

/// A soldier at the greybox's middle holding the ladder's rung `r`.
pub(crate) fn holding(sim: &mut Sim, r: usize, id: &str) -> Entity {
    let a = soldier(sim);
    let _b = soldier(sim);
    sim.seconds(1.5);
    place(sim, a, Vec3::new(0.0, 0.1, 0.0));
    set_rung(sim, a, r);
    sim.seconds(1.5);
    assert_eq!(held(sim, a), Some(id));
    a
}

/// Hold the trigger for `secs` (pressed every other tick: semi-automatics
/// fire too), aiming level ahead.
fn spray(sim: &mut Sim, a: Entity, secs: f64) {
    let at = sim.position(a) + Vec3::new(0.0, 1.5, -20.0);
    for t in 0..(secs * sim.tick_hz()) as u64 {
        testkit::aim_at(sim, a, at);
        sim.intent(a).fire = t % 2 == 0;
        sim.ticks(1);
    }
    sim.intent(a).fire = false;
}

#[test]
fn a_swap_to_cs_sources_gun_ends_combat_arms_recoil() {
    let mut sim = party();
    // Rung 7 (0-based): Combat Arms' M4A1; rung 6: CS:S's AK-47.
    let a = holding(&mut sim, 7, "combat_arms:m4a1");
    spray(&mut sim, a, 0.6);
    assert!(punch(&sim, a).x > 0.01, "Combat Arms' kick pulled the view up: {:?}", punch(&sim, a));
    assert_eq!(sim.app.world().get::<RecoilGame>(a).map(|g| g.0.as_str()), Some("combat_arms"));
    set_rung(&mut sim, a, 6);
    sim.ticks(1);
    assert_eq!(held(&sim, a), Some("cs_source:weapon_ak47"));
    assert_eq!(sim.app.world().get::<RecoilGame>(a).map(|g| g.0.as_str()), Some("cs_source"));
    assert_eq!(punch(&sim, a), Vec3::ZERO, "the switch ends Combat Arms' punch");
    // Combat Arms' return doesn't go on into CS:S's view (it drifted
    // down below level before).
    for _ in 0..(1.0 * sim.tick_hz()) as u32 {
        sim.ticks(1);
        assert!(punch(&sim, a).length() < 1e-6, "no drift: {:?}", punch(&sim, a));
    }
    let k = sim.app.world().get::<CaKick>(a).cloned().unwrap_or_default();
    assert_eq!((k.pitch, k.yaw), (0.0, 0.0), "Combat Arms' kick ended");
}

#[test]
fn a_swap_to_combat_arms_gun_ends_cs_sources_recoil() {
    let mut sim = party();
    let a = holding(&mut sim, 6, "cs_source:weapon_ak47");
    spray(&mut sim, a, 0.3);
    let kicked = punch(&sim, a);
    assert!(kicked.x > 0.01, "CS:S's kick: {kicked:?}");
    set_rung(&mut sim, a, 7);
    sim.ticks(1);
    assert_eq!(held(&sim, a), Some("combat_arms:m4a1"));
    assert_eq!(punch(&sim, a), Vec3::ZERO, "the switch ends CS:S's punch");
    // Combat Arms' own kick starts from level and returns to it (once
    // the M4A1 is drawn).
    sim.seconds(1.5);
    assert!(punch(&sim, a).length() < 1e-6, "no CS:S decay into it: {:?}", punch(&sim, a));
    spray(&mut sim, a, 0.3);
    assert!(punch(&sim, a).x > 0.0, "Combat Arms' kick: {:?}", punch(&sim, a));
    sim.seconds(2.0);
    assert!(punch(&sim, a).x.abs() < 1e-3, "returned to level: {:?}", punch(&sim, a));
}

/// A 20 kg physics box 4 m ahead at eye height, floating (no gravity
/// pull matters within the tick measured).
fn crate_ahead(sim: &mut Sim, a: Entity) -> Entity {
    let at = sim.position(a) + sim.state(a).eye_offset + Vec3::new(0.0, 0.0, -4.0);
    sim.app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::cuboid(0.6, 0.6, 0.6),
            Mass(20.0),
            PropSurface("metal".into()),
            Transform::from_translation(at),
        ))
        .id()
}

/// The box's speed away from the shooter after one round.
fn pushed_by_one_round(rung: usize, id: &str) -> f32 {
    let mut sim = party();
    let a = holding(&mut sim, rung, id);
    let b = crate_ahead(&mut sim, a);
    sim.ticks(2);
    let at = sim.app.world().get::<Transform>(b).unwrap().translation;
    testkit::aim_at(&mut sim, a, at);
    sim.ticks(1);
    let before = sim.app.world().get::<LinearVelocity>(b).unwrap().0;
    sim.intent(a).fire = true;
    sim.ticks(1);
    sim.intent(a).fire = false;
    let mut v = sim.app.world().get::<LinearVelocity>(b).unwrap().0;
    for _ in 0..4 {
        sim.ticks(1);
        let now = sim.app.world().get::<LinearVelocity>(b).unwrap().0;
        if -now.z > -v.z {
            v = now;
        }
    }
    -(v - before).z
}

#[test]
fn both_games_bullets_push_a_physics_object() {
    // CS:S's AK-47: 2400 kg·in/s (60.96 N·s) on 20 kg, 3.05 m/s.
    let cs = pushed_by_one_round(6, "cs_source:weapon_ak47");
    assert!((cs - 60.96 / 20.0).abs() < 0.3, "CS:S's push unchanged: {cs} m/s");
    // Combat Arms' M4A1: its damage at 4 m (about 30 HP, the roll's
    // +-20 %) at 2 N·s a hit point: about 3 m/s.
    let ca = pushed_by_one_round(7, "combat_arms:m4a1");
    assert!(ca > 1.5 && ca < 5.0, "Combat Arms' round pushes it too: {ca} m/s");
}

// ---------------------------------------------------------------------------
// Hits across games (docs/DESIGN.md "Hits across games")

/// Every hit a weapon dealt: (target, hitgroup).
#[derive(Resource, Default)]
pub(crate) struct Hits(Vec<(Entity, chimera_core::core::Hitgroup)>);

pub(crate) fn collect_hits(mut events: MessageReader<chimera_sim::weapon::WeaponEvent>, mut out: ResMut<Hits>) {
    for e in events.read() {
        if let chimera_sim::weapon::WeaponEventKind::Hit { target, hitgroup, .. } = e.kind
            && !e.replay
        {
            out.0.push((target, hitgroup));
        }
    }
}

/// A shooter holding rung `shooter`'s gun and a target 4 m ahead holding
/// rung `target`'s, facing each other, the target unkillable; with
/// `bodies`, the map's systems (which give characters their bodies) and
/// those bodies.
fn duel(shooter: usize, target: usize, bodies: Option<fn(&mut App)>) -> (Sim, Entity, Entity) {
    let mut sim = match bodies {
        None => party(),
        Some(setup) => {
            let mut sim = Sim::new(
                combat_arms::harness::CaSim,
                (
                    chimera_world::greybox::GreyboxMapPlugin,
                    chimera_world::map::MapSimPlugin::empty(),
                    party_kit::PartyPartsPlugin,
                ),
            );
            setup(&mut sim.app);
            crate::gun_game::plain_rungs(sim.app.world_mut());
            chimera_sim::minigame::start(sim.app.world_mut(), party_kit::DEFAULT_MINIGAME).expect("the gun game starts");
            sim
        }
    };
    sim.app.init_resource::<Hits>().add_systems(Update, collect_hits);
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    // Standing on the floor (the spawn's height, then settled).
    place(&mut sim, a, Vec3::new(0.0, 1.0, 0.0));
    place(&mut sim, b, Vec3::new(0.0, 1.0, -4.0));
    set_rung(&mut sim, a, shooter);
    set_rung(&mut sim, b, target);
    sim.intent(a).yaw = 0.0;
    sim.intent(b).yaw = std::f32::consts::PI;
    sim.seconds(1.5);
    let mut h = sim.app.world_mut().get_mut::<chimera_core::core::Health>(b).unwrap();
    h.current = 100.0;
    h.max = 100.0;
    (sim, a, b)
}

/// Where the centre of `b`'s first hitbox of `region` is in the world.
pub(crate) fn shape_at(sim: &Sim, b: Entity, region: chimera_core::core::BodyRegion) -> Vec3 {
    let w = sim.app.world();
    let boxes = chimera_core::core::hitboxes_in(w, b).expect("hitboxes");
    let shape = boxes.0.iter().find(|h| h.region == region).unwrap_or_else(|| panic!("no {region:?}"));
    let t = w.get::<Transform>(b).unwrap().translation;
    let feet = w.get::<avian3d::prelude::ColliderAabb>(b).map_or(t, |a| t.with_y(a.min.y));
    feet + w.get::<chimera_core::core::Intent>(b).unwrap().yaw_rotation() * shape.center
}

/// One round from `a` at `point`; the hitgroups `b` was hit in.
pub(crate) fn shot_at(sim: &mut Sim, a: Entity, b: Entity, point: Vec3) -> Vec<chimera_core::core::Hitgroup> {
    sim.app.world_mut().resource_mut::<Hits>().0.clear();
    testkit::aim_at(sim, a, point);
    sim.ticks(1);
    sim.intent(a).fire = true;
    sim.ticks(1);
    sim.intent(a).fire = false;
    sim.ticks(6);
    let hits = sim.app.world().resource::<Hits>().0.iter().filter(|(t, _)| *t == b).map(|(_, g)| *g).collect();
    sim.seconds(1.0);
    hits
}

#[test]
fn a_cs_source_gun_hits_a_combat_arms_body_where_it_is_drawn() {
    use chimera_core::core::{BodyRegion as R, Hitgroup as G};
    // Rung 6: CS:S's AK-47; rung 7: Combat Arms' M4A1 (its body, its
    // hit spheres as the target's hitboxes).
    let (mut sim, a, b) = duel(6, 7, None);
    let boxes = chimera_core::core::hitboxes_in(sim.app.world(), b).expect("Combat Arms' body has hitboxes");
    assert!(boxes.0.iter().all(|h| h.radius.is_some()), "spheres");
    // The head is the head; the right hand and shin are the right arm and
    // leg (height bands on the collider would say left for both).
    let head = shape_at(&sim, b, R::Head);
    assert_eq!(shot_at(&mut sim, a, b, head), vec![G::Head]);
    let hand = shape_at(&sim, b, R::RightHand);
    assert_eq!(shot_at(&mut sim, a, b, hand), vec![G::RightArm]);
    let shin = shape_at(&sim, b, R::RightShin);
    assert_eq!(shot_at(&mut sim, a, b, shin), vec![G::RightLeg]);
}

/// Chimera's broad phase on Combat Arms' bodies (docs/performance.md, "A
/// broad phase for the hitscan"): a CS:S round at one Combat Arms body's
/// head makes that body's hitboxes and pose (and hits the head), while a
/// second Combat Arms body 3 m to the side, which the ray passes far from,
/// is left unposed.
#[test]
fn a_cs_source_round_poses_only_the_combat_arms_bodies_it_passes() {
    use chimera_core::core::{BodyRegion as R, Hitgroup as G, LazyHitboxes};
    use combat_arms::game::characters::CaPose;
    let (mut sim, a, b) = duel(6, 7, None);
    let c = soldier(&mut sim);
    sim.seconds(0.5);
    place(&mut sim, c, Vec3::new(3.0, 1.0, -4.0));
    set_rung(&mut sim, c, 7);
    sim.intent(c).yaw = std::f32::consts::PI;
    sim.seconds(1.5);
    let head = shape_at(&sim, b, R::Head);
    testkit::aim_at(&mut sim, a, head);
    sim.ticks(1);
    // The shot's tick reads the cells the tick before left.
    let cells = |sim: &Sim, e: Entity| {
        let w = sim.app.world();
        (w.get::<LazyHitboxes>(e).expect("made on demand").clone(), w.get::<CaPose>(e).unwrap().clone())
    };
    let (near, far) = (cells(&sim, b), cells(&sim, c));
    assert!(!near.0.0.is_made() && !far.0.0.is_made() && !far.1.is_posed_any(), "nothing asked yet");
    sim.app.world_mut().resource_mut::<Hits>().0.clear();
    sim.intent(a).fire = true;
    sim.ticks(1);
    sim.intent(a).fire = false;
    let hits: Vec<_> = sim.app.world().resource::<Hits>().0.clone();
    assert_eq!(hits, vec![(b, G::Head)], "the head, as before");
    assert!(near.0.0.is_made(), "the body the ray reaches is posed");
    assert!(far.0.1.as_ref().is_some_and(|b| b.is_made()), "the far body's bound was tested");
    assert!(!far.0.0.is_made() && !far.1.is_posed_any(), "the far body is left unposed");
}

/// The map's bodies: Combat Arms' (no hitboxes: it makes its own) and a
/// short CS:S stand-in whose head box is 1 m up, below where Combat Arms'
/// own skeleton has its chest, so only its own boxes give a head shot.
fn short_cs_body(app: &mut App) {
    use chimera_core::core::{BodyRegion as R, Hitbox};
    let cube = |y: f32, x: f32, region| Hitbox::cuboid(Vec3::new(x, y, 0.0), Vec3::splat(0.12), Quat::IDENTITY, region);
    cs_body(app, vec![cube(1.0, 0.0, R::Head), cube(0.7, 0.0, R::Chest), cube(0.25, 0.12, R::RightShin)]);
}

/// The map's bodies with a tall CS:S stand-in: its head box 1.95 m up,
/// mostly above the hull (as a real CS:S model's head pokes out of a
/// shorter hull), a neck box under it.
fn tall_cs_body(app: &mut App) {
    use chimera_core::core::{BodyRegion as R, Hitbox};
    let cube = |y: f32, half: f32, region| Hitbox::cuboid(Vec3::new(0.0, y, 0.0), Vec3::splat(half), Quat::IDENTITY, region);
    cs_body(app, vec![cube(1.95, 0.12, R::Head), cube(1.68, 0.06, R::Neck), cube(1.3, 0.2, R::Chest)]);
}

/// The map's bodies: Combat Arms' and a CS:S stand-in with `boxes`.
fn cs_body(app: &mut App, boxes: Vec<chimera_core::core::Hitbox>) {
    use chimera_world::map::{CharacterModels, MapCharacterModel};
    let body = |game, hitboxes| MapCharacterModel {
        game: Some(game),
        team: None,
        name: None,
        class: None,
        model: default(),
        hitboxes,
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    app.insert_resource(CharacterModels(std::sync::Arc::new(vec![
        body("combat_arms", Vec::new()),
        body("cs_source", boxes),
    ])));
}

#[test]
fn a_combat_arms_gun_hits_a_cs_source_body_where_it_is_drawn() {
    use chimera_core::core::{BodyRegion as R, Hitgroup as G};
    // Rung 7: Combat Arms' M4A1 at rung 6's CS:S player, who wears the
    // short CS:S body: its boxes, not Combat Arms' skeleton standing.
    let (mut sim, a, b) = duel(7, 6, Some(short_cs_body));
    let boxes = sim.app.world().get::<chimera_core::core::Hitboxes>(b).expect("the CS:S body's hitboxes");
    assert!(boxes.0.iter().all(|h| h.radius.is_none()), "boxes");
    let head = shape_at(&sim, b, R::Head);
    assert_eq!(shot_at(&mut sim, a, b, head), vec![G::Head]);
    let chest = shape_at(&sim, b, R::Chest);
    assert_eq!(shot_at(&mut sim, a, b, chest), vec![G::Chest]);
    let shin = shape_at(&sim, b, R::RightShin);
    assert_eq!(shot_at(&mut sim, a, b, shin), vec![G::RightLeg]);
}

/// Playtest 2026-10-28: Combat Arms' rounds never reached a CS:S head
/// poking out of the hull (they tested the hull before the shapes), so a
/// shot at the drawn head did nothing and only the neck box counted.
#[test]
fn a_combat_arms_round_reaches_a_cs_source_head_above_the_hull() {
    use chimera_core::core::{BodyRegion as R, Hitgroup as G};
    let (mut sim, a, b) = duel(7, 6, Some(tall_cs_body));
    let w = sim.app.world();
    let top = w.get::<avian3d::prelude::ColliderAabb>(b).unwrap().max.y;
    let head = shape_at(&sim, b, R::Head);
    assert!(head.y > top, "the stand-in's head centre ({}) is above the hull's top ({top})", head.y);
    let (h0, _) = health_and_armour(&sim, b);
    assert_eq!(shot_at(&mut sim, a, b, head), vec![G::Head]);
    let (h1, _) = health_and_armour(&sim, b);
    assert!(h0 - h1 > 30.0, "head damage: {}", h0 - h1);
    let neck = shape_at(&sim, b, R::Neck);
    assert_eq!(shot_at(&mut sim, a, b, neck), vec![G::Chest], "the neck is the neck");
}

/// The same body under a CS:S round: Source's rule, boxes poking out
/// count (it never had the bug; the two games agree).
#[test]
fn a_cs_source_round_reaches_the_same_head() {
    use chimera_core::core::{BodyRegion as R, Hitgroup as G};
    let (mut sim, a, b) = duel(6, 6, Some(tall_cs_body));
    let head = shape_at(&sim, b, R::Head);
    assert_eq!(shot_at(&mut sim, a, b, head), vec![G::Head]);
}

/// Every region a knife hit reported: (target, region).
#[derive(Resource, Default)]
struct Regions(Vec<(Entity, chimera_core::core::BodyRegion)>);

fn collect_regions(mut hits: MessageReader<chimera_core::core::RegionHit>, mut out: ResMut<Regions>) {
    out.0.extend(hits.read().map(|h| (h.target, h.region)));
}

/// Combat Arms' knife on a CS:S body: that body's boxes, not Combat
/// Arms' skeleton spheres hidden inside it (the short stand-in's head is
/// at the height of that skeleton's chest).
#[test]
fn a_combat_arms_knife_hits_a_cs_source_body_by_its_boxes() {
    use chimera_core::core::BodyRegion as R;
    let (mut sim, a, b) = duel(7, 6, Some(short_cs_body));
    sim.app.init_resource::<Regions>().add_systems(PostUpdate, collect_regions);
    // Up close, the knife (class key 3).
    let (pa, pb) = (sim.position(a), sim.position(b));
    place(&mut sim, b, Vec3::new(pa.x, pb.y, pa.z - 0.9));
    sim.intent(a).select = Some(2);
    sim.ticks(1);
    sim.intent(a).select = None;
    sim.seconds(1.0);
    assert_eq!(held(&sim, a), Some(combat_arms::game::inventory::KNIFE));
    let head = shape_at(&sim, b, R::Head);
    testkit::aim_at(&mut sim, a, head);
    sim.ticks(1);
    let (h0, _) = health_and_armour(&sim, b);
    sim.intent(a).fire = true;
    sim.ticks(1);
    sim.intent(a).fire = false;
    sim.seconds(1.0);
    let (h1, _) = health_and_armour(&sim, b);
    assert!(h1 < h0, "the knife hit");
    let regions: Vec<R> = sim.app.world().resource::<Regions>().0.iter().filter(|(t, _)| *t == b).map(|(_, r)| *r).collect();
    assert_eq!(regions.first(), Some(&R::Head), "{regions:?}");
}

/// A swap back and forth leaves exactly the current identity's hitboxes:
/// CS:S's boxes, Combat Arms' spheres, CS:S's boxes again.
#[test]
fn a_swap_leaves_only_the_current_identitys_hitboxes() {
    let (mut sim, _a, b) = duel(7, 6, Some(tall_cs_body));
    let shapes = |sim: &Sim| {
        let h = chimera_core::core::hitboxes_in(sim.app.world(), b).expect("hitboxes");
        (h.0.iter().all(|s| s.radius.is_none()), h.0.iter().all(|s| s.radius.is_some()))
    };
    assert_eq!(shapes(&sim), (true, false), "CS:S's boxes");
    for _ in 0..2 {
        set_rung(&mut sim, b, 7);
        sim.ticks(3);
        assert_eq!(shapes(&sim), (false, true), "Combat Arms' spheres");
        set_rung(&mut sim, b, 6);
        sim.ticks(3);
        assert_eq!(shapes(&sim), (true, false), "CS:S's boxes again");
    }
    // A swap and back within one tick.
    set_rung(&mut sim, b, 7);
    set_rung(&mut sim, b, 6);
    sim.ticks(3);
    assert_eq!(shapes(&sim), (true, false), "CS:S's boxes after a quick swap");
    set_rung(&mut sim, b, 7);
    sim.ticks(1);
    set_rung(&mut sim, b, 6);
    sim.ticks(3);
    assert_eq!(shapes(&sim), (true, false), "CS:S's boxes after a one-tick swap");
}

// ---------------------------------------------------------------------------
// Armour across games: one `weapon::Armor` per character, covering by its
// identity's game (CS:S's kevlar; Combat Arms' gear as `ArmorPieces`),
// every gun through the shared step (`weapon::hits::absorb`).

/// Health (hit points) and armour (points) `b` has.
pub(crate) fn health_and_armour(sim: &Sim, b: Entity) -> (f32, f32) {
    let w = sim.app.world();
    let h = w.get::<chimera_core::core::Health>(b).unwrap().current * 100.0;
    (h, w.get::<chimera_sim::weapon::Armor>(b).map_or(0.0, |a| a.amount * 100.0))
}

/// What one round at `b`'s first shape of `region` took: (health lost,
/// armour lost), in points.
fn round_on(sim: &mut Sim, a: Entity, b: Entity, region: chimera_core::core::BodyRegion) -> (f32, f32) {
    let (h0, a0) = health_and_armour(sim, b);
    let at = shape_at(sim, b, region);
    assert!(!shot_at(sim, a, b, at).is_empty(), "the round hit");
    let (h1, a1) = health_and_armour(sim, b);
    (h0 - h1, a0 - a1)
}

#[test]
fn cs_source_kevlar_takes_a_combat_arms_round_by_the_shared_rule() {
    use chimera_core::core::BodyRegion as R;
    // Combat Arms' M4A1 at a CS:S player wearing kevlar (bought: CS:S's
    // armour, no Combat Arms gear on it).
    let (mut sim, a, b) = duel(7, 6, Some(short_cs_body));
    assert!(sim.app.world().get::<chimera_sim::weapon::ArmorPieces>(b).is_none(), "no Combat Arms gear on a CS:S player");
    sim.app.world_mut().entity_mut(b).insert(chimera_sim::weapon::Armor { amount: 1.0, helmet: false });
    // Kevlar covers half (ratio 1): health takes half the round, armour
    // half the rest.
    let (health, armour) = round_on(&mut sim, a, b, R::Chest);
    assert!(health > 5.0, "{health}");
    assert!((armour / health - 0.5).abs() < 0.01, "kevlar's split: health -{health}, armour -{armour}");
    // A leg: kevlar doesn't cover it.
    let (health, armour) = round_on(&mut sim, a, b, R::RightShin);
    assert!(health > 5.0 && armour == 0.0, "health -{health}, armour -{armour}");
}

#[test]
fn combat_arms_gear_takes_a_cs_source_round_by_the_shared_rule() {
    use chimera_core::core::BodyRegion as R;
    // CS:S's AK-47 (armour ratio 1.55) at a Combat Arms soldier: its
    // default body gear (torso 0.15) and 100 points.
    let (mut sim, a, b) = duel(6, 7, None);
    let pieces = sim.app.world().get::<chimera_sim::weapon::ArmorPieces>(b).cloned().expect("Combat Arms' gear");
    assert_eq!(pieces, combat_arms::game::weapons::pieces_of(&[combat_arms::game::weapons::DEFAULT_BODY_GEAR]));
    // It blocks 0.15 x (2 - 1.55) of the round, all of it from the
    // armour: health takes 0.9325, armour 0.0675 (health in whole points).
    let (health, armour) = round_on(&mut sim, a, b, R::Chest);
    assert!(health > 20.0, "{health}");
    // (Health in whole points, what armour has left truncated: CS:S's
    // quantum.)
    let raw = (health / 0.9325, (health + 1.0) / 0.9325);
    assert!(
        armour >= raw.0 * 0.0675 - 1e-3 && armour <= raw.1 * 0.0675 + 1.0,
        "the gear's split: health -{health}, armour -{armour}"
    );
    // The groin: not covered.
    let (health, armour) = round_on(&mut sim, a, b, R::Groin);
    assert!(health > 20.0 && armour == 0.0, "health -{health}, armour -{armour}");
}

#[test]
fn armour_follows_the_identity_and_each_hud_shows_it() {
    use combat_arms::ui::hud::CaHud;
    use cs_source::ui::game_hud::{Part, PanelKind};
    use crate::hud::{party_with_huds, swap_to};
    let (mut sim, me, other) = party_with_huds();
    // CS:S's HUD: its armour panel up, drawing the viewed player's `Armor`
    // in points (headless there are no glyphs to read its digits back).
    let cs_armour = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let up = w
            .query::<(&Part, &Visibility)>()
            .iter(w)
            .any(|(p, v)| *p == Part::Panel(PanelKind::Armor) && *v != Visibility::Hidden);
        let me = w.query_filtered::<Entity, With<chimera_core::core::LocalPlayer>>().single(w).unwrap();
        up.then(|| w.get::<chimera_sim::weapon::Armor>(me).map_or(0.0, |a| a.amount * 100.0).round().to_string())
    };
    let ca_armour = |sim: &Sim| sim.app.world().resource::<CaHud>().status.as_ref().map(|s| s.ap.clone());
    // CS:S players get no Combat Arms armour.
    for e in [me, other] {
        assert!(sim.app.world().get::<chimera_sim::weapon::Armor>(e).is_none(), "no armour given to a CS:S player");
    }
    // Kevlar bought: CS:S's HUD shows it.
    sim.app.world_mut().entity_mut(me).insert(chimera_sim::weapon::Armor { amount: 0.5, helmet: false });
    sim.ticks(2);
    assert_eq!(cs_armour(&mut sim).as_deref(), Some("50"));
    // Combat Arms' gun: the same armour, now covering as Combat Arms'
    // gear, on Combat Arms' HUD.
    swap_to(&mut sim, me, 1);
    let w = sim.app.world();
    assert_eq!(w.get::<chimera_sim::weapon::Armor>(me).map(|a| a.amount), Some(0.5), "the amount kept");
    assert!(w.get::<chimera_sim::weapon::ArmorPieces>(me).is_some(), "Combat Arms' gear on");
    assert_eq!(ca_armour(&sim).as_deref(), Some("50"));
    // Back to CS:S: the gear off, kevlar again, CS:S's HUD.
    swap_to(&mut sim, me, 2);
    assert!(sim.app.world().get::<chimera_sim::weapon::ArmorPieces>(me).is_none(), "the gear off");
    assert_eq!(cs_armour(&mut sim).as_deref(), Some("50"));
}
