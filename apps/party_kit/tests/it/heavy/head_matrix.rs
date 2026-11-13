//! Head shots across games on the real bodies (the user's installs;
//! docs/DESIGN.md "Hits across games"; Ultimate Gun Game playtest, "Combat
//! Arms guns on HL2:DM heads"): every shooter game (CS:S's USP, Combat
//! Arms' M4A1, HL2:DM's .357) at every target game's body (CS:S's model
//! and its hitbox set, Combat Arms' soldier and its hit spheres, HL2:DM's
//! player models and their hitbox sets) on de_dust2, standing, crouched,
//! walking across, looking up and down, near and far, turned four ways:
//!
//! - a round at the centre of the target's posed head shape is a head hit,
//!   with more damage than one at the chest, whenever the head is the
//!   first shape on the aim line (another shape in front of it, an arm or
//!   the Combine's generic upper chest seen from behind, is hit instead,
//!   for every shooter alike; a walking body's rows are printed only, its
//!   pose moving within the tick the round leaves);
//! - a round just beside the head (6 cm past its shape, the line meeting
//!   no shape) hits nothing: Combat Arms' rules took a box by its
//!   bounding sphere, so its server walk turned such a near miss into a
//!   hit on the chest's box (priority over the head) and its shooter's
//!   test could favour a big box over the head the segment met first
//!   (`core::Hitbox::power_along`, `line_passes`).
//!
//! Over the network (a server and two clients), the shooter's client aims
//! at the head it draws and the server (its own posing, lag compensation)
//! decides; HL2:DM's models in each hold get the same head test. Prints
//! the matrix (`--nocapture`). Passes with a note without the three
//! installs.

use bevy::prelude::*;
use chimera_core::{
    core::{BodyRegion, Health, Hitbox, Hitgroup, Intent},
    mount::config::LocalConfig,
};
use chimera_sim::{
    minigame::{self, MinigameAppExt},
    testkit,
    weapon::{WeaponEvent, WeaponEventKind},
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::harness::CaSim;
use party_kit::PartyPartsPlugin;

use crate::gun_game::{held, no_finale, place, plain_rungs, soldier};

/// One gun of each game: (the game, its gun).
const GUNS: [(&str, &str); 3] = [
    ("cs_source", "cs_source:weapon_usp"),
    ("combat_arms", "combat_arms:m4a1"),
    ("hl2dm", "hl2dm:weapon_357"),
];

/// How far beside the head the near miss passes (m).
const BESIDE: f32 = 0.06;

/// The margin a cell's line keeps from other shapes to be asked anything
/// of (m): a gun's own spread and the pose's motion in the tick the round
/// leaves move it by less.
const MARGIN: f32 = 0.04;

fn installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    let hl2 = config.game_path("hl2dm").filter(|p| chimera_core::mount::served::usable("hl2dm", p, "hl2mp"));
    let ok = css.is_some() && ca.is_some() && hl2.is_some();
    if !ok {
        eprintln!("skipping: needs the CS:S, Combat Arms and HL2:DM installs");
    }
    ok
}

/// The gun game with `ladder`, its match long enough for every matrix.
fn heads_text(ladder: &[&str]) -> String {
    let text = crate::hl2dm_parts::ladder_text(ladder, false);
    assert!(text.contains("time_limit = \"10m\""));
    text.replace("time_limit = \"10m\"", "time_limit = \"600m\"")
}

/// Every hit the rounds dealt (target, hitgroup) and the shots that left
/// (owner).
#[derive(Resource, Default)]
struct Seen {
    hits: Vec<(Entity, Hitgroup)>,
    shots: Vec<Entity>,
}

fn collect(mut events: MessageReader<WeaponEvent>, mut out: ResMut<Seen>) {
    for e in events.read() {
        if e.replay {
            continue;
        }
        match e.kind {
            WeaponEventKind::Hit { target, hitgroup, .. } => out.hits.push((target, hitgroup)),
            WeaponEventKind::Shot { .. } => out.shots.push(e.owner),
            _ => {}
        }
    }
}

/// The gun game on de_dust2 with `ladder`, a shooter and a target at the
/// CT spawns (4.9 m apart, in the open).
fn dust2(ladder: &[&str]) -> Option<(Sim, Entity, Entity, Vec3, Vec3)> {
    if !installed() {
        return None;
    }
    let map = party_kit::load_map("cs_source:de_dust2", 0).unwrap();
    let (pa, pb) = (map.spawns[2].0 + Vec3::Y, map.spawns[0].0 + Vec3::Y);
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    plain_rungs(sim.app.world_mut());
    no_finale(sim.app.world_mut());
    sim.app.add_minigames(&[("heads.toml", &heads_text(ladder))]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim.app.init_resource::<Seen>().add_systems(Update, collect);
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    Some((sim, a, b, pa, pb))
}

fn set_rung(sim: &mut Sim, e: Entity, r: usize) {
    let w = sim.app.world_mut();
    let l = w.resource::<minigame::ladder::Ladder>().clone();
    minigame::ladder::set_rung(w, e, r, &l);
}

/// `b`'s hit shapes in the world of `w` (as every game places them: feet
/// at its box's bottom, turned by its look's yaw).
fn shapes_in(w: &World, b: Entity) -> Option<Vec<Hitbox>> {
    let boxes = chimera_core::core::hitboxes_in(w, b)?;
    let t = w.get::<Transform>(b)?.translation;
    let feet = w.get::<avian3d::prelude::ColliderAabb>(b).map_or(t, |a| t.with_y(a.min.y));
    let turn = w.get::<Intent>(b)?.yaw_rotation();
    Some(
        boxes
            .0
            .iter()
            .map(|h| Hitbox {
                center: feet + turn * h.center,
                rotation: turn * h.rotation,
                ..*h
            })
            .collect(),
    )
}

/// Where a row of the matrix aims, on a target's shapes seen from an eye.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Aim {
    /// The centre of the highest head shape.
    Head,
    /// The centre of the highest chest shape (else the stomach's).
    Chest,
    /// `BESIDE` past the head shape's edge, to the side as seen from the
    /// eye.
    BesideHead,
}

impl Aim {
    fn at(self, shapes: &[Hitbox], eye: Vec3) -> Option<Vec3> {
        let top = |r: BodyRegion| {
            shapes
                .iter()
                .filter(|h| h.region == r)
                .max_by(|x, y| x.center.y.total_cmp(&y.center.y))
                .copied()
        };
        match self {
            Aim::Head => top(BodyRegion::Head).map(|h| h.center),
            Aim::Chest => top(BodyRegion::Chest).or_else(|| top(BodyRegion::Stomach)).map(|h| h.center),
            Aim::BesideHead => {
                let h = top(BodyRegion::Head)?;
                let side = (h.center - eye).cross(Vec3::Y).normalize();
                // The head's extent along `side`, then past it.
                let (lo, hi) = h.bounds();
                let reach = ((hi - lo) / 2.0).dot(side.abs());
                Some(h.center + side * (reach + BESIDE))
            }
        }
    }
}

/// The region of the first shape the line from `eye` through `at` enters,
/// every shape grown by `grow` (m; None: it meets none).
fn first_on_line(shapes: &[Hitbox], eye: Vec3, at: Vec3, grow: f32) -> Option<BodyRegion> {
    let dir = (at - eye).normalize();
    shapes
        .iter()
        .map(|h| Hitbox {
            half: h.half + Vec3::splat(grow),
            radius: h.radius.map(|r| r + grow),
            ..*h
        })
        .filter_map(|h| h.ray_span(eye, dir).map(|(enter, _)| (enter, h.region)))
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|(_, r)| r)
}

/// A full clip in `a`'s gun (every round of the matrix has one to fire).
fn refill(w: &mut World, a: Entity) {
    if let Some(gun) = testkit::active(w, a)
        && let Some(mut m) = w.get_mut::<chimera_sim::weapon::Magazine>(gun)
    {
        m.clip = m.size.max(1);
        m.reserve = m.reserve_max;
    }
}

/// Health and no armour (the raw damage of each round is compared).
fn full_health(w: &mut World, b: Entity) {
    let mut h = w.get_mut::<Health>(b).unwrap();
    h.current = 100.0;
    h.max = 100.0;
    if let Some(mut a) = w.get_mut::<chimera_sim::weapon::Armor>(b) {
        a.amount = 0.0;
        a.helmet = false;
    }
}

/// One round's outcome: what the aim line met first (the shapes as they
/// are, then grown by `MARGIN`), the hitgroups dealt and the health lost
/// (points).
#[derive(Clone, Debug)]
struct Round {
    first: Option<BodyRegion>,
    first_grown: Option<BodyRegion>,
    hits: Vec<Hitgroup>,
    damage: f32,
}

/// One round from `a` at `aim` on `b`, the aim kept on it every tick
/// until the round leaves (a moving target's shape moves).
fn round_at(sim: &mut Sim, a: Entity, b: Entity, aim: Aim) -> Round {
    full_health(sim.app.world_mut(), b);
    refill(sim.app.world_mut(), a);
    let h0 = sim.app.world().get::<Health>(b).unwrap().current;
    {
        let mut seen = sim.app.world_mut().resource_mut::<Seen>();
        seen.hits.clear();
        seen.shots.clear();
    }
    let (mut first, mut first_grown) = (None, None);
    for _ in 0..12 {
        let eye = sim.position(a) + sim.state(a).eye_offset;
        let Some(shapes) = shapes_in(sim.app.world(), b) else { break };
        let Some(at) = aim.at(&shapes, eye) else { break };
        first = first_on_line(&shapes, eye, at, 0.0);
        first_grown = first_on_line(&shapes, eye, at, MARGIN);
        testkit::aim_at(sim, a, at);
        sim.intent(a).fire = true;
        sim.ticks(1);
        if sim.app.world().resource::<Seen>().shots.contains(&a) {
            break;
        }
    }
    sim.intent(a).fire = false;
    sim.ticks(4);
    let hits = sim.app.world().resource::<Seen>().hits.iter().filter(|(t, _)| *t == b).map(|(_, g)| *g).collect();
    let damage = (h0 - sim.app.world().get::<Health>(b).unwrap().current) * 100.0;
    // Ready for the next round (its fire cycle).
    sim.seconds(0.8);
    full_health(sim.app.world_mut(), b);
    Round {
        first,
        first_grown,
        hits,
        damage,
    }
}

/// How the target stands for a round.
#[derive(Clone, Copy, Debug)]
enum Stance {
    Standing,
    Crouched,
    /// Walking sideways across the shooter's view.
    Walking,
    /// Standing, looking 40 degrees up.
    LookingUp,
    /// Standing, looking 40 degrees down.
    LookingDown,
}

impl Stance {
    const ALL: [Stance; 5] = [Stance::Standing, Stance::Crouched, Stance::Walking, Stance::LookingUp, Stance::LookingDown];

    /// The look's pitch (radians, up positive).
    fn pitch(self) -> f32 {
        match self {
            Stance::LookingUp => 40f32.to_radians(),
            Stance::LookingDown => -40f32.to_radians(),
            _ => 0.0,
        }
    }

    fn set(self, i: &mut Intent, yaw: f32) {
        i.yaw = yaw;
        i.pitch = self.pitch();
        i.crouch = matches!(self, Stance::Crouched);
        i.walk = matches!(self, Stance::Walking);
        i.move_axis = if matches!(self, Stance::Walking) { Vec2::X } else { Vec2::ZERO };
    }
}

/// What is wrong with a cell, if anything: a head that is the first
/// shape on the line (also with the shapes grown by `MARGIN`) must be a
/// head hit dealing more than the chest's round (another first shape
/// takes the round, for any shooter alike, so nothing is asked of it); a
/// line beside the head that meets no shape (grown by `MARGIN` either)
/// must hit nothing.
fn verdict(head: &Round, chest: &Round, beside: &Round) -> Option<String> {
    let mut why = Vec::new();
    if head.first == Some(BodyRegion::Head) && head.first_grown == Some(BodyRegion::Head) {
        if head.hits != [Hitgroup::Head] {
            why.push(format!("the head first on the line, hit {:?}", head.hits));
        } else if head.damage <= chest.damage * 1.5 {
            why.push(format!("head {:.0} against chest {:.0}", head.damage, chest.damage));
        }
    }
    if beside.first_grown.is_none() && !beside.hits.is_empty() {
        why.push(format!("a line beside the head hit {:?}", beside.hits));
    }
    (!why.is_empty()).then(|| why.join("; "))
}

fn row(shooter: &str, target: &str, label: &str, head: &Round, chest: &Round, beside: &Round) -> String {
    format!(
        "{shooter:>11} -> {target:>11} {label}: head {:?} {:.0} (first {:?}), chest {:?} {:.0}, beside {:?} (first {:?})",
        head.hits, head.damage, head.first, chest.hits, chest.damage, beside.hits, beside.first
    )
}

/// The matrix: every shooter at every target, each stance, the target
/// turned four ways, near and far.
#[test]
fn every_games_gun_hits_every_games_head_as_a_head() {
    let ladder: Vec<&str> = GUNS.iter().map(|(_, g)| *g).collect();
    let Some((mut sim, a, b, pa, pb)) = dust2(&ladder) else { return };
    let toward = (pa - pb).with_y(0.0).normalize();
    let mut wrong = Vec::new();
    let mut cells = 0;
    for (ti, (target, _)) in GUNS.iter().enumerate() {
        set_rung(&mut sim, b, ti);
        for (si, (shooter, gun)) in GUNS.iter().enumerate() {
            set_rung(&mut sim, a, si);
            sim.seconds(1.2);
            assert_eq!(held(&sim, a), Some(*gun));
            for distance in [2.5f32, 4.9] {
                for turn in [0.0f32, 90.0, 180.0, 270.0] {
                    for stance in Stance::ALL {
                        place(&mut sim, a, pb + toward * distance);
                        place(&mut sim, b, pb);
                        let face = (-toward.x).atan2(-toward.z) + turn.to_radians();
                        stance.set(&mut sim.intent(b), face);
                        let settle = if matches!(stance, Stance::Walking) { 0.3 } else { 0.8 };
                        sim.seconds(settle);
                        let head = round_at(&mut sim, a, b, Aim::Head);
                        place(&mut sim, b, pb);
                        sim.seconds(settle);
                        let chest = round_at(&mut sim, a, b, Aim::Chest);
                        place(&mut sim, b, pb);
                        sim.seconds(settle);
                        let beside = round_at(&mut sim, a, b, Aim::BesideHead);
                        let label = format!("{distance:.1} m turned {turn:>3} {stance:?}");
                        let line = row(shooter, target, &label, &head, &chest, &beside);
                        eprintln!("{line}");
                        cells += 1;
                        // A walking body's pose moves within the tick the
                        // round leaves: printed, not asked.
                        if !matches!(stance, Stance::Walking)
                            && let Some(why) = verdict(&head, &chest, &beside)
                        {
                            wrong.push(format!("{line}: {why}"));
                        }
                    }
                }
            }
        }
        Stance::Standing.set(&mut sim.intent(b), 0.0);
    }
    eprintln!("{cells} cells, {} wrong", wrong.len());
    assert!(wrong.is_empty(), "wrong:\n{}", wrong.join("\n"));
}

/// HL2:DM's guns a target may hold (each its own hold pose).
const HL2_HOLDS: [&str; 6] = [
    "hl2dm:weapon_pistol",
    "hl2dm:weapon_smg1",
    "hl2dm:weapon_ar2",
    "hl2dm:weapon_shotgun",
    "hl2dm:weapon_crossbow",
    "hl2dm:weapon_rpg",
];

/// HL2:DM's models a target may wear (the Combine, metro police, citizens).
const HL2_MODELS: [&str; 5] = [
    "models/combine_soldier.mdl",
    "models/combine_super_soldier.mdl",
    "models/police.mdl",
    "models/humans/group03/female_01.mdl",
    "models/humans/group03/male_07.mdl",
];

#[test]
fn every_games_gun_hits_each_hl2dm_models_head_in_each_hold() {
    let ladder: Vec<&str> = GUNS.iter().map(|(_, g)| *g).chain(HL2_HOLDS).collect();
    let Some((mut sim, a, b, pa, pb)) = dust2(&ladder) else { return };
    let toward = (pa - pb).with_y(0.0).normalize();
    let mut wrong = Vec::new();
    for model in HL2_MODELS {
        for (hi, hold) in HL2_HOLDS.iter().enumerate() {
            set_rung(&mut sim, b, GUNS.len() + hi);
            sim.app.world_mut().entity_mut(b).insert(hl2dm::game::weapons::inventory::Hl2Model(model.into()));
            for (si, (shooter, gun)) in GUNS.iter().enumerate() {
                set_rung(&mut sim, a, si);
                sim.seconds(1.2);
                assert_eq!(held(&sim, a), Some(*gun));
                assert_eq!(held(&sim, b), Some(*hold));
                place(&mut sim, a, pa);
                place(&mut sim, b, pb);
                Stance::Standing.set(&mut sim.intent(b), (-toward.x).atan2(-toward.z));
                sim.seconds(0.8);
                let head = round_at(&mut sim, a, b, Aim::Head);
                let chest = round_at(&mut sim, a, b, Aim::Chest);
                let beside = round_at(&mut sim, a, b, Aim::BesideHead);
                let line = row(shooter, &format!("{model} holding {hold}"), "", &head, &chest, &beside);
                eprintln!("{line}");
                if let Some(why) = verdict(&head, &chest, &beside) {
                    wrong.push(format!("{line}: {why}"));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "wrong:\n{}", wrong.join("\n"));
}

// ---------------------------------------------------------------------------
// Over the network: a server and two clients on de_dust2, the shooter's
// client aiming at the head it draws (its own hitboxes of the other
// player, posed from the drawn animation), the server's lag compensation
// and its own posing deciding.

fn net_dust2() -> Option<(chimera_net::harness::NetSim, Vec3, Vec3)> {
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    if !installed() {
        return None;
    }
    let map = party_kit::load_map("cs_source:de_dust2", 0).unwrap();
    let (pa, pb) = (map.spawns[2].0 + Vec3::Y, map.spawns[0].0 + Vec3::Y);
    let ladder: Vec<&str> = GUNS.iter().map(|(_, g)| *g).collect();
    let text = heads_text(&ladder);
    let setup = move |app: &mut App| {
        app.add_plugins((MapSimPlugin::new(map.clone()), PartyPartsPlugin));
        plain_rungs(app.world_mut());
        app.add_minigames(&[("heads.toml", &text)]).unwrap();
    };
    let link = LinkConditions {
        latency: std::time::Duration::from_millis(50),
        jitter: std::time::Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 11, 2, setup);
    sim.set_frame(combat_arms::game::TICK_INTERVAL);
    no_finale(sim.server.app.world_mut());
    minigame::start(sim.server.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim.server.app.init_resource::<Seen>().add_systems(Update, collect);
    sim.until_joined(900);
    sim.ticks(300);
    Some((sim, pa, pb))
}

fn net_place(sim: &mut chimera_net::harness::NetSim, e: Entity, at: Vec3) {
    let w = sim.server.app.world_mut();
    w.get_mut::<Transform>(e).unwrap().translation = at;
    w.get_mut::<avian3d::prelude::Position>(e).unwrap().0 = at;
}

/// One round from client 0 at the head of client 1's player as client 0
/// draws it: (hitgroups the server dealt, health lost, how far client 0's
/// aim point was from the server's present head, m).
fn net_round(sim: &mut chimera_net::harness::NetSim, b: Entity) -> (Vec<Hitgroup>, f32, f32) {
    let me = sim.local_player(0).unwrap();
    let shooter = sim.character_of(0).unwrap();
    refill(sim.server.app.world_mut(), shooter);
    refill(sim.clients[0].app.world_mut(), me);
    let id1 = sim.client_id(1);
    let b_seen = chimera_net::harness::NetSim::owned_by(&mut sim.clients[0], Some(id1)).expect("client 0 sees b");
    full_health(sim.server.app.world_mut(), b);
    let h0 = sim.server.app.world().get::<Health>(b).unwrap().current;
    {
        let mut seen = sim.server.app.world_mut().resource_mut::<Seen>();
        seen.hits.clear();
        seen.shots.clear();
    }
    let mut off = f32::NAN;
    for _ in 0..12 {
        let eye = {
            let w = sim.clients[0].app.world();
            w.get::<Transform>(me).unwrap().translation + w.get::<chimera_core::core::MovementState>(me).unwrap().eye_offset
        };
        let Some(at) = shapes_in(sim.clients[0].app.world(), b_seen).and_then(|s| Aim::Head.at(&s, eye)) else {
            break;
        };
        if let Some(server) = shapes_in(sim.server.app.world(), b).and_then(|s| Aim::Head.at(&s, eye)) {
            off = at.distance(server);
        }
        testkit::aim_at(&mut sim.clients[0], me, at);
        sim.clients[0].intent(me).fire = true;
        sim.step();
        if sim.server.app.world().resource::<Seen>().shots.contains(&shooter) {
            break;
        }
    }
    sim.clients[0].intent(me).fire = false;
    sim.ticks(6);
    let hits = sim.server.app.world().resource::<Seen>().hits.iter().filter(|(t, _)| *t == b).map(|(_, g)| *g).collect();
    let lost = (h0 - sim.server.app.world().get::<Health>(b).unwrap().current) * 100.0;
    sim.ticks(60);
    (hits, lost, off)
}

/// Every shooter at an HL2:DM player's head over the network, standing,
/// crouched, walking across and looking down (looking up, its arms are in
/// front of its head for every shooter); every shooter at a CS:S player's
/// head likewise. (Combat Arms' bodies over the network are another
/// matter: their client's spheres stand off the server's; printed here.)
#[test]
fn every_games_gun_hits_a_head_as_drawn_over_the_network() {
    let Some((mut sim, pa, pb)) = net_dust2() else { return };
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let mut wrong = Vec::new();
    for (ti, (target, _)) in GUNS.iter().enumerate() {
        for (si, (shooter, gun)) in GUNS.iter().enumerate() {
            {
                let w = sim.server.app.world_mut();
                let l = w.resource::<minigame::ladder::Ladder>().clone();
                minigame::ladder::set_rung(w, a, si, &l);
                minigame::ladder::set_rung(w, b, ti, &l);
            }
            sim.ticks(90);
            assert_eq!(testkit::active_id(sim.server.app.world(), a), Some(*gun));
            for stance in [Stance::Standing, Stance::Crouched, Stance::Walking, Stance::LookingDown] {
                net_place(&mut sim, a, pa);
                net_place(&mut sim, b, pb);
                let face = (pa - pb).with_y(0.0);
                let them = sim.local_player(1).unwrap();
                stance.set(&mut sim.clients[1].intent(them), (-face.x).atan2(-face.z));
                sim.ticks(if matches!(stance, Stance::Walking) { 20 } else { 60 });
                let (head, damage, off) = net_round(&mut sim, b);
                let line = format!(
                    "net {shooter:>11} -> {target:>11} {:>11}: head {head:?} {damage:.0} (drawn {off:.3} m from the server's)",
                    format!("{stance:?}"),
                );
                eprintln!("{line}");
                if *target != "combat_arms" && head != [Hitgroup::Head] {
                    wrong.push(line);
                }
                Stance::Standing.set(&mut sim.clients[1].intent(them), (-face.x).atan2(-face.z));
            }
        }
    }
    assert!(wrong.is_empty(), "not a head shot:\n{}", wrong.join("\n"));
}
