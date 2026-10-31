//! Objectives on stock maps (specs/cs_source/objectives.md): planting at
//! de_dust2's A site and the site's BombExplode outputs, cs_office's
//! hostages and rescue, shots at a hostage (Q8). Skipped without a CS:S
//! install.

use avian3d::prelude::Position;
use bevy::prelude::*;
use crate::stack::{
    console::Console,
    core::{SimTick, Team},
    games::{
        self, cs_source,
        cs_source::{
            TICK_INTERVAL,
            movement::{self, SourceMovementPlugin, to_engine},
            objectives::C4,
            weapons::CsWeaponsPlugin,
        },
    },
    harness::{CsSim, Sim},
    logic::Logic,
    map::{MapData, MapPlugin},
    mount::config::LocalConfig,
    objectives::{
        MapKind, MapObjectives,
        bomb::{BombOutcome, BombState, PlantedBomb},
        hostages::{Hostage, HostageTally},
    },
    weapon::{Inventory, give},
};

const UNIT: f32 = 0.0254;
/// The standing hull's centre above the feet, m (36 units).
const LIFT: f32 = 36.0 * UNIT;

fn load(name: &str) -> Option<MapData> {
    let installed = LocalConfig::load()
        .ok()?
        .game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())
        .is_some_and(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    if !installed {
        eprintln!("skipping: no CS:S install configured");
        return None;
    }
    Some(games::load_map_lean(&format!("cs_source:{name}")).expect("load map"))
}

fn sim(map: MapData) -> Sim {
    let mut sim = Sim::new(CsSim, (MapPlugin::new(map), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    sim.ticks(2);
    sim
}

fn character(sim: &mut Sim, feet_units: Vec3, team: u8) -> Entity {
    let e = sim.spawn_character(to_engine(feet_units) + Vec3::Y * (LIFT + 0.05), movement::ID);
    sim.app.world_mut().entity_mut(e).insert(Team(team));
    e
}

fn teleport(sim: &mut Sim, e: Entity, feet_units: Vec3) {
    let at = to_engine(feet_units) + Vec3::Y * (LIFT + 0.05);
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(e).unwrap().translation = at;
    if let Some(mut p) = w.get_mut::<Position>(e) {
        p.0 = at;
    }
}

/// O4/O5 and a plant at A: the site's BombExplode fires when it blows.
#[test]
fn dust2_plants_at_a_and_the_site_explodes() {
    let Some(map) = load("de_dust2") else { return };
    let mut sim = sim(map);
    let objectives = sim.app.world().resource::<MapObjectives>().clone();
    assert_eq!(objectives.kind(), MapKind::Bomb);
    assert_eq!(objectives.bomb_targets.len(), 2);
    assert_eq!(objectives.bomb_radius, None, "dust2 has no bombradius: 500");
    // O4 / O5: the A box (1072, 2336, 96)-(1248, 2624, 192).
    let hull = |x: f32, y: f32| {
        let feet = to_engine(Vec3::new(x, y, 96.03));
        (
            feet + Vec3::new(-16.0, 0.0, -16.0) * UNIT,
            feet + Vec3::new(16.0, 72.0, 16.0) * UNIT,
        )
    };
    let (lo, hi) = hull(1160.0, 2480.0);
    let a = objectives.bomb_target_at(lo, hi).expect("in A");
    let (lo, hi) = hull(1000.0, 2480.0);
    assert_eq!(objectives.bomb_target_at(lo, hi), None);

    let t = character(&mut sim, Vec3::new(1160.0, 2480.0, 100.0), 1);
    sim.seconds(0.5);
    give(sim.app.world_mut(), t, C4).unwrap();
    sim.seconds(1.2);
    assert!(sim.state(t).on_ground);
    sim.app.world_mut().resource_mut::<Console>().submit("mp_c4timer 10");
    sim.ticks(1);
    sim.intent(t).fire = true;
    sim.seconds(3.1);
    sim.intent(t).fire = false;
    let (bomb, at) = {
        let w = sim.app.world_mut();
        let v: Vec<(PlantedBomb, Vec3)> = w
            .query::<(&PlantedBomb, &Transform)>()
            .iter(w)
            .map(|(b, t)| (b.clone(), t.translation))
            .collect();
        assert_eq!(v.len(), 1, "planted");
        v[0].clone()
    };
    assert_eq!(bomb.site, Some(a));
    assert!(
        (bomb.explode_at - bomb.planted_at - 10.0).abs() < 1e-6,
        "mp_c4timer read at the plant"
    );
    let feet = sim.position(t).y - LIFT;
    assert!((at.y - feet).abs() < 0.05, "on the floor: {} vs {feet}", at.y);
    sim.app.world_mut().resource_mut::<Logic>().world.record = true;
    let planted_tick = sim.app.world().resource::<SimTick>().0;
    sim.seconds(10.2);
    assert_eq!(
        sim.app.world().resource::<BombState>().outcome,
        Some(BombOutcome::Exploded)
    );
    let logic = sim.app.world().resource::<Logic>();
    let site_index = objectives.bomb_targets[a].map_index.unwrap();
    let fired = logic.world.fired.iter().any(|(_, id, out)| {
        out == "BombExplode" && logic.world.get(*id).is_some_and(|e| e.map_index == Some(site_index))
    });
    assert!(fired, "A's BombExplode fired");
    // The planter stood on it: dead.
    assert!(sim.app.world().get::<crate::stack::core::Health>(t).unwrap().current <= 0.0);
    let _ = planted_tick;
}

/// O2: cs_office's four hostages stand where the map puts them; one
/// follows a CT over the nav mesh and is rescued in a rescue zone.
#[test]
fn office_hostages_follow_and_are_rescued() {
    let Some(map) = load("cs_office") else { return };
    let mut sim = sim(map);
    let objectives = sim.app.world().resource::<MapObjectives>().clone();
    assert_eq!(objectives.kind(), MapKind::Hostage);
    let want = [
        Vec3::new(1784.0, 734.0, -124.0),
        Vec3::new(1744.0, 802.0, -124.0),
        Vec3::new(2048.0, -344.0, -156.0),
        Vec3::new(1984.0, -344.0, -156.0),
    ];
    assert_eq!(objectives.hostages.len(), 4);
    for (h, w) in objectives.hostages.iter().zip(want) {
        assert!((h.feet - to_engine(w)).length() < 0.05, "{} vs {w}", h.feet / UNIT);
    }
    assert!(!objectives.rescue_zones.is_empty());
    let t = character(&mut sim, Vec3::new(1500.0, 300.0, -150.0), 1);
    let ct = character(&mut sim, Vec3::new(1500.0, 200.0, -150.0), 2);
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 0; mp_roundtime 5; chimera_rounds 1");
    sim.seconds(1.5);
    let hostages: Vec<(Entity, Vec3)> = {
        let w = sim.app.world_mut();
        w.query_filtered::<(Entity, &Transform), With<Hostage>>()
            .iter(w)
            .map(|(e, t)| (e, t.translation))
            .collect()
    };
    assert_eq!(hostages.len(), 4);
    for (h, at) in &hostages {
        assert!(sim.state(*h).on_ground, "hostage at {} not standing", at / UNIT);
    }
    let _ = t;
    // The CT walks up to the first hostage and uses it.
    let (h, at) = hostages[0];
    let hs = sim.position(h);
    teleport(&mut sim, ct, Vec3::new(1784.0, 734.0 - 50.0, -124.0));
    sim.ticks(3);
    let eye = sim.position(ct) + sim.state(ct).eye_offset;
    let d = hs - eye;
    sim.intent(ct).yaw = (-d.x).atan2(-d.z);
    sim.intent(ct).pitch = d.y.atan2(d.xz().length());
    sim.intent(ct).use_key = true;
    sim.ticks(2);
    sim.intent(ct).use_key = false;
    sim.ticks(1);
    assert_eq!(sim.app.world().get::<Hostage>(h).unwrap().leader, Some(ct));
    // He walks off 400 units; it comes after him.
    teleport(&mut sim, ct, Vec3::new(1784.0 - 400.0, 734.0 - 50.0, -124.0));
    sim.seconds(4.0);
    let moved = (sim.position(h) - at).xz().length() / UNIT;
    assert!(moved > 100.0, "it moved only {moved} units");
    // Into a rescue zone, its leader there 300 units on: on its way to
    // him, rescued (only a hostage following and walking is, Q11).
    let zone = objectives.rescue_zones[0].floor_centre();
    {
        let w = sim.app.world_mut();
        let to = zone + Vec3::Y * (LIFT + 0.1);
        for (e, at) in [(h, to), (ct, to + Vec3::X * 300.0 * UNIT)] {
            w.get_mut::<Transform>(e).unwrap().translation = at;
            if let Some(mut p) = w.get_mut::<Position>(e) {
                p.0 = at;
            }
        }
    }
    sim.ticks(3);
    assert!(sim.app.world().get_entity(h).is_err(), "rescued");
    assert_eq!(sim.app.world().resource::<HostageTally>().rescued, 1);
    let _ = Inventory::default();
}

/// A following hostage animates: idle standing, then a walk or run
/// sequence (HL2 citizen animations from `.ani` blocks) while it comes
/// after its leader, its body turned toward where it goes, and a nod when
/// it starts following.
#[test]
fn office_hostage_walks_after_its_leader() {
    use crate::stack::{
        games::cs_source::{
            hostage_anim::{FOLLOW_GESTURE, HostageAnim},
            player_anim::PlayerAnimPlugin,
        },
        map::anim::Animator,
    };
    let Some(map) = load("cs_office") else { return };
    let mut sim = Sim::new(CsSim, (
        MapPlugin::new(map),
        SourceMovementPlugin,
        CsWeaponsPlugin,
        PlayerAnimPlugin,
    ));
    sim.set_tick_interval(TICK_INTERVAL);
    sim.ticks(2);
    let ct = character(&mut sim, Vec3::new(1500.0, 200.0, -150.0), 2);
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 0; mp_roundtime 5; chimera_rounds 1");
    sim.seconds(1.5);
    let h = {
        let w = sim.app.world_mut();
        w.query::<(Entity, &Hostage)>()
            .iter(w)
            .find(|(_, h)| h.index == 0)
            .map(|(e, _)| e)
            .expect("hostage 1")
    };
    let main = |sim: &Sim| {
        let a = sim.app.world().get::<Animator>(h).expect("hostages get an animator");
        a.main.map(|s| a.set.sequences[s].name.to_lowercase())
    };
    assert_eq!(main(&sim).as_deref(), Some("idle_subtle"), "standing idle");
    // The CT uses it and walks off; it follows.
    let hs = sim.position(h);
    teleport(&mut sim, ct, Vec3::new(1784.0, 734.0 - 50.0, -124.0));
    sim.ticks(3);
    let eye = sim.position(ct) + sim.state(ct).eye_offset;
    let d = hs - eye;
    sim.intent(ct).yaw = (-d.x).atan2(-d.z);
    sim.intent(ct).pitch = d.y.atan2(d.xz().length());
    sim.intent(ct).use_key = true;
    sim.ticks(2);
    sim.intent(ct).use_key = false;
    sim.ticks(2);
    assert_eq!(sim.app.world().get::<Hostage>(h).unwrap().leader, Some(ct));
    {
        let a = sim.app.world().get::<Animator>(h).unwrap();
        let gesture = a
            .layers
            .first()
            .copied()
            .flatten()
            .map(|l| a.set.sequences[l.sequence].name.clone());
        assert_eq!(gesture.as_deref(), Some(FOLLOW_GESTURE), "a nod as it starts following");
    }
    teleport(&mut sim, ct, Vec3::new(1784.0 - 400.0, 734.0 - 50.0, -124.0));
    let mut seen = Vec::new();
    for _ in 0..30 {
        sim.seconds(0.1);
        if let Some(m) = main(&sim) {
            seen.push(m);
        }
        let v = sim.velocity(h).xz();
        let state = sim.app.world().get::<HostageAnim>(h).unwrap().clone();
        if v.length() / UNIT > 60.0 && state.activity != "ACT_IDLE" {
            // Facing where it goes (within the turn it is still making).
            let heading = (-v.x).atan2(-v.y);
            let yaw = sim.app.world().get::<Animator>(h).unwrap().yaw.unwrap();
            let off = (heading - yaw).rem_euclid(std::f32::consts::TAU);
            let off = off.min(std::f32::consts::TAU - off).to_degrees();
            assert!(off < 60.0, "body {off}° off its heading");
        }
    }
    assert!(
        seen.iter().any(|s| s == "walk_all" || s == "run_all"),
        "it never walked or ran: {seen:?}"
    );
    eprintln!("hostage played {seen:?}");
}

/// Q9 and Q11, measured on the probe server 2026-10-17 (cs_office and
/// cs_compound, specs/cs_source/objectives.md): each hostage spawned takes
/// the next of the four models in turn, the turn going on over rounds (a
/// 3-hostage map's rounds start one model later each time) and keeps the
/// map's skin (cs_office's 1); a hostage is
/// rescued only while it follows and walks, when its 26-unit-wide hull
/// reaches into a rescue zone (cs_office's zone face at y -1719: rescued
/// at origin y -1710, 9 units outside); idle in a zone, or standing by its
/// leader there, it isn't; and a map without rescue zones rescues nobody,
/// not even across the defenders' spawns.
#[test]
fn office_hostage_models_turn_and_rescue_rules() {
    use crate::stack::{map::{BodyName, BodySkin}, objectives::hostages::{HostageRules, spawn_all}};
    let Some(map) = load("cs_office") else { return };
    let mut sim = sim(map);
    let ct = character(&mut sim, Vec3::new(-1112.0, -1552.0, -330.0), 2);
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 0; mp_roundtime 5; chimera_rounds 1");
    sim.seconds(1.0);
    let models = sim.app.world().resource::<HostageRules>().models.clone();
    assert_eq!(models.len(), 4, "the four hostage models");
    let round = |sim: &mut Sim| -> Vec<(Entity, usize, String)> {
        let w = sim.app.world_mut();
        let mut v: Vec<(Entity, usize, String)> = w
            .query::<(Entity, &Hostage, &BodyName)>()
            .iter(w)
            .map(|(e, h, b)| (e, h.index, b.0.clone()))
            .collect();
        v.sort_by_key(|x| x.1);
        v
    };
    let which = |name: &str| models.iter().position(|m| m == name).unwrap();
    // Q9: four in a row, then (three hostages, as cs_compound) one later
    // each round.
    let first = round(&mut sim);
    assert_eq!(first.len(), 4);
    // The map's skin stays whatever the model (cs_office's: 1, measured).
    for (e, i, _) in &first {
        assert_eq!(sim.app.world().get::<BodySkin>(*e), Some(&BodySkin(1)), "hostage {i}'s skin");
    }
    let k0 = which(&first[0].2);
    for (i, (_, _, m)) in first.iter().enumerate() {
        assert_eq!(which(m), (k0 + i) % 4, "round 1, hostage {i}");
    }
    sim.app.world_mut().resource_mut::<MapObjectives>().hostages.truncate(3);
    for r in 0..3 {
        spawn_all(sim.app.world_mut());
        let got: Vec<usize> = round(&mut sim).iter().map(|x| which(&x.2)).collect();
        let want: Vec<usize> = (0..3).map(|i| (k0 + 4 + 3 * r + i) % 4).collect();
        assert_eq!(got, want, "three hostages, round {}", r + 2);
    }
    // Q11: one hostage at a time (Source units, feet on the floor).
    let floor = -335.9;
    let place = |sim: &mut Sim, leader: Option<(f32, f32)>, at: (f32, f32)| -> Entity {
        spawn_all(sim.app.world_mut());
        let h = round(sim)[0].0;
        teleport(sim, h, Vec3::new(at.0, at.1, floor));
        if let Some(l) = leader {
            teleport(sim, ct, Vec3::new(l.0, l.1, floor));
        }
        sim.app.world_mut().get_mut::<Hostage>(h).unwrap().leader = leader.map(|_| ct);
        h
    };
    let gone = |sim: &Sim, h: Entity| sim.app.world().get_entity(h).is_err();
    // Idle in the middle of the zone: stays.
    let h = place(&mut sim, None, (-600.0, -1870.0));
    sim.seconds(0.5);
    assert!(!gone(&sim, h), "an idle hostage in a zone isn't rescued");
    // Following, but standing by its leader in the zone: stays.
    let h = place(&mut sim, Some((-560.0, -1870.0)), (-600.0, -1870.0));
    sim.seconds(0.5);
    assert!(!gone(&sim, h), "one standing by its leader isn't rescued");
    // Walking along the face (its leader 400 units east) with its origin
    // 9 units outside, its hull 4 in: rescued at once.
    let h = place(&mut sim, Some((-400.0, -1710.0)), (-800.0, -1710.0));
    sim.ticks(2);
    assert!(gone(&sim, h), "the hull reaching in rescues");
    // 19 units outside, its hull 6 short: stays.
    let h = place(&mut sim, Some((-400.0, -1700.0)), (-800.0, -1700.0));
    let start = sim.position(h);
    sim.seconds(0.3);
    assert!(!gone(&sim, h), "a hull short of the zone isn't rescued");
    assert!((sim.position(h) - start).length() / UNIT > 10.0, "it walked");
    // Walking into the zone: rescued.
    let h = place(&mut sim, Some((-752.0, -1950.0)), (-752.0, -1600.0));
    sim.seconds(3.0);
    assert!(gone(&sim, h), "walking in rescues");
    // No rescue zones: led straight over a CT spawn (-752, -1744), it
    // stays.
    sim.app.world_mut().resource_mut::<MapObjectives>().rescue_zones.clear();
    let h = place(&mut sim, Some((-752.0, -1950.0)), (-752.0, -1600.0));
    let mut nearest = f32::MAX;
    for _ in 0..30 {
        sim.seconds(0.1);
        assert!(!gone(&sim, h), "no rescue without zones");
        let at = sim.position(h) / UNIT;
        nearest = nearest.min(Vec2::new(at.x - (-752.0), -at.z - (-1744.0)).length());
    }
    assert!(nearest < 40.0, "it passed the CT spawn: nearest {nearest} units");
}

/// Q8, the CS:S probe's shots again: one AK-47 or Desert Eagle shot from
/// 150 units at hostage 1 (fresh each time), aimed `z` units above its
/// feet, on the hostage model's first hitbox set (the same for all four
/// models) posed by its idle animation. CS:S's rule (objectives.md
/// Constants, Hostages, "the rule"): each hit takes the shot's damage
/// after falloff x the hostage's own multiplier; a bullet through one of
/// its boxes may hit another at x 0.5 with falloff again; the hits add
/// up, truncated after. Every shot is checked against that rule on the
/// boxes ours crosses, and against CS:S's number where ours crosses the
/// boxes CS:S's damage says it hit. Where they differ the spread put the
/// line elsewhere: CS:S's seeds aren't ours, its spread at 150 units
/// (about 1.5 units for the AK-47, 2.5 for the Deagle) is enough to move
/// a line between boxes, and its own repeats of the same aim took other
/// boxes (the spec's repeat), so a different box isn't a different rule.
#[test]
fn office_hostage_shots_from_150_units() {
    use crate::stack::{
        core::{Health, Hitboxes, Hitgroup, Intent},
        games::cs_source::weapons::{AK47, DEAGLE},
        objectives::hostages::HOSTAGE_HITGROUPS,
        weapon::{DamageEffect, WeaponEvent, WeaponEventKind, falloff},
    };
    use avian3d::prelude::ColliderAabb;
    #[derive(Resource, Default)]
    struct Segments(Vec<(Vec3, Vec3, Option<Entity>)>);
    fn record(mut e: MessageReader<WeaponEvent>, mut segments: ResMut<Segments>) {
        for e in e.read() {
            if let WeaponEventKind::Shot { from, to, hit, .. } | WeaponEventKind::ShotContinued { from, to, hit, .. } =
                e.kind
            {
                segments.0.push((from, to, hit));
            }
        }
    }
    use Hitgroup::*;
    // (weapon, z, CS:S's damage (100: killed), the hits that damage fits
    // by the rule (AK 35.79 x 2.5 = 89, x 1.5 = 53, x 0.75 = 26, x 0.6 =
    // 21; Deagle 50.85 x 1.5 = 76, x 0.75 = 38, x 0.6 + 23.78 x 0.6 =
    // 44), the hits ours makes). Five of ours cross CS:S's boxes and take
    // its damage. The others: AK z 48 goes on from the chest into the
    // stomach (84.81; CS:S's own repeats at z 48 took 85 twice in four),
    // AK z 36 takes the stomach, not an arm (62.64; CS:S's repeats at z
    // 36: 62 in six of eight), AK z 18 both legs (32.13; CS:S's z 24
    // repeats: 32 once), the Deagle at z 48 chest and stomach (killed)
    // and at z 36 the stomach (88.93), where CS:S's took an arm.
    let shots: [(&str, f32, i32, &[Hitgroup], &[Hitgroup]); 10] = [
        (AK47, 66.0, 89, &[Head], &[Head]),
        (AK47, 60.0, 53, &[Chest], &[Chest]),
        (AK47, 48.0, 53, &[Chest], &[Chest, Stomach]),
        (AK47, 36.0, 26, &[LeftArm], &[Stomach]),
        (AK47, 18.0, 21, &[LeftLeg], &[RightLeg, LeftLeg]),
        (DEAGLE, 66.0, 100, &[Head], &[Head]),
        (DEAGLE, 60.0, 76, &[Chest], &[Chest]),
        (DEAGLE, 48.0, 38, &[LeftArm], &[Chest, Stomach]),
        (DEAGLE, 36.0, 38, &[LeftArm], &[Stomach]),
        (DEAGLE, 18.0, 44, &[LeftLeg, LeftLeg], &[RightLeg, LeftLeg]),
    ];
    // Left and right count alike (the same multiplier).
    let same = |a: &[Hitgroup], b: &[Hitgroup]| {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(x, y)| HOSTAGE_HITGROUPS.get(*x) == HOSTAGE_HITGROUPS.get(*y))
    };
    let Some(map) = load("cs_office") else { return };
    // Hitboxes posed by its idle animation, as CS:S's stood.
    let mut sim = Sim::new(CsSim, (
        MapPlugin::new(map),
        SourceMovementPlugin,
        CsWeaponsPlugin,
        crate::stack::games::cs_source::player_anim::PlayerAnimPlugin,
    ));
    sim.set_tick_interval(TICK_INTERVAL);
    sim.ticks(2);
    sim.app.init_resource::<Segments>().add_systems(Last, record);
    let shooter_feet = Vec3::new(1634.0, 734.0, -159.9);
    let p = character(&mut sim, shooter_feet, 2);
    sim.seconds(1.0);
    let (mut matched, mut moved) = (0, Vec::new());
    for (weapon, z, theirs, their_hits, our_hits) in shots {
        crate::stack::objectives::hostages::spawn_all(sim.app.world_mut());
        *sim.app.world_mut().get_mut::<Inventory>(p).unwrap() = Inventory::default();
        give(sim.app.world_mut(), p, weapon).unwrap();
        teleport(&mut sim, p, shooter_feet);
        sim.seconds(1.5);
        let h = {
            let w = sim.app.world_mut();
            w.query::<(Entity, &Hostage)>()
                .iter(w)
                .find(|(_, h)| h.index == 0)
                .map(|(e, _)| e)
                .expect("hostage 1")
        };
        let feet = sim.position(h) - Vec3::Y * LIFT;
        assert!(
            (feet / UNIT - to_engine(Vec3::new(1784.0, 734.0, -159.9)) / UNIT).length() < 1.0,
            "hostage 1 stands where CS:S's did: {}",
            feet / UNIT
        );
        let eye = sim.position(p) + sim.state(p).eye_offset;
        let shooter = sim.position(p) - Vec3::Y * LIFT;
        assert!(((eye - shooter).y / UNIT - 64.0).abs() < 0.5, "a standing eye");
        // Ours stands 2 units higher there than CS:S's -159.9.
        assert!(sim.state(p).on_ground);
        assert!(
            ((shooter - feet).y / UNIT).abs() < 2.5,
            "on the same floor: {} vs {}",
            shooter / UNIT,
            feet / UNIT
        );
        let d = feet + Vec3::Y * z * UNIT - eye;
        sim.intent(p).yaw = (-d.x).atan2(-d.z);
        sim.intent(p).pitch = d.y.atan2(d.xz().length());
        sim.ticks(2);
        sim.app.world_mut().resource_mut::<Segments>().0.clear();
        sim.intent(p).fire = true;
        sim.ticks(1);
        sim.intent(p).fire = false;
        // The hostage's boxes as the shot met them, in their frame.
        let (boxes, frame) = {
            let w = sim.app.world();
            let t = w.get::<Transform>(h).unwrap();
            let floor = w.get::<ColliderAabb>(h).unwrap().min.y;
            let turn = w.get::<Intent>(h).unwrap().yaw_rotation().inverse();
            (w.get::<Hitboxes>(h).unwrap().0.clone(), (t.translation.with_y(floor), turn))
        };
        let effect = {
            let gun = sim.app.world().get::<Inventory>(p).unwrap().active.unwrap();
            sim.app.world().get::<DamageEffect>(gun).unwrap().clone()
        };
        sim.ticks(3);
        let health = sim.app.world().get::<Health>(h).map_or(0.0, |x| x.current);
        let taken = (1.0 - health) * 100.0;
        // Each segment that ends on the hostage: the box entered there (the
        // nearest one along it, as the engine picks) and the rule's damage.
        let segments = std::mem::take(&mut sim.app.world_mut().resource_mut::<Segments>().0);
        let mut hits = Vec::new();
        let (mut carried, mut rule) = (effect.amount, 0.0);
        // The bullet's line (its later segments may start inside a box).
        let dir = segments.first().map(|(from, to, _)| (*to - *from).normalize()).expect("a shot");
        for (from, to, hit) in segments {
            if hit != Some(h) {
                continue;
            }
            let (o, dl) = (frame.1 * (from - frame.0), frame.1 * dir);
            let group = boxes
                .iter()
                .filter_map(|b| Some((b.ray_entry(o, dl)?, b.group())))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, g)| g)
                .expect("a box where the bullet stopped");
            if !hits.is_empty() {
                carried *= 0.5;
            }
            carried *= falloff(effect.falloff, effect.falloff_step, (to - eye).length());
            rule += carried * HOSTAGE_HITGROUPS.get(group) * 100.0;
            hits.push(group);
        }
        eprintln!(
            "{weapon} z {z}: ours {taken:.2} by {hits:?} (rule {rule:.2}), CS:S {theirs} by {their_hits:?}"
        );
        assert!(!hits.is_empty(), "{weapon} at {z}: hit the hostage");
        if health > 0.0 {
            assert!((taken - rule).abs() < 0.01, "{weapon} at {z}: {taken}, the rule says {rule}");
        } else {
            assert!(rule >= 100.0, "{weapon} at {z}: killed, the rule says {rule}");
        }
        if same(&hits, their_hits) {
            matched += 1;
            let ours = if health > 0.0 { (taken + 1e-3).floor() as i32 } else { 100 };
            assert!((ours - theirs).abs() <= 1, "{weapon} at {z}: {ours}, CS:S {theirs}");
        }
        if !same(&hits, our_hits) {
            moved.push(format!("{weapon} at {z}: ours hit {hits:?}, was {our_hits:?}"));
        }
    }
    eprintln!("{matched} of 10 shots crossed the boxes CS:S's did");
    assert!(moved.is_empty(), "our shots crossed other boxes: {moved:#?}");
}

/// Where every hostage stands once spawned, against CS:S's origins on the
/// probe server (`mashup_hostages` after a map load, 2026-10-17; spec 9,
/// "Spawn"): on the floor within a unit, on six stock hostage maps. The
/// local player flies with noclip meanwhile: hostages keep the game's own
/// movement (they took the player's and hung at the map's origins,
/// cs_office's first two 36 units up).
#[test]
fn hostages_stand_where_css_puts_them() {
    use crate::stack::{core::LocalPlayer, movement::noclip, objectives::hostages::spawn_all};
    #[rustfmt::skip]
    let measured: &[(&str, &[[f32; 3]])] = &[
        ("cs_office", &[[1784.0, 734.0, -159.9], [1744.0, 802.0, -159.9], [2048.0, -344.0, -159.9], [1984.0, -344.0, -159.9]]),
        ("cs_italy", &[[976.0, 2392.0, 0.0], [833.0, 2339.0, 128.0], [944.0, 2320.0, 0.0], [917.0, 2336.0, 128.0]]),
        ("cs_assault", &[[6718.3, 3895.2, -607.9], [6639.3, 3889.9, -607.9], [6482.5, 3839.5, -607.9], [6537.5, 3836.8, -607.9]]),
        ("cs_compound", &[[1578.8, -1581.6, 16.0], [1768.0, -1465.0, 16.0], [1776.0, -1592.0, 16.0]]),
        ("cs_havana", &[[80.0, 814.0, 248.0], [348.0, 817.0, 248.0], [128.0, 1280.0, 248.0], [240.0, 1280.0, 248.0]]),
        ("cs_militia", &[[608.0, 896.0, -151.9], [552.0, 864.0, -151.9], [1256.0, 400.0, -151.9], [832.0, 600.0, -151.9]]),
    ];
    let (mut report, mut bad) = (Vec::new(), 0);
    for (name, theirs) in measured {
        let Some(map) = load(name) else { return };
        let mut sim = sim(map);
        let flyer = sim.spawn_character(Vec3::new(0.0, 50.0, 0.0), noclip::ID);
        sim.app.world_mut().entity_mut(flyer).insert(LocalPlayer);
        spawn_all(sim.app.world_mut());
        sim.seconds(2.0);
        let ours: Vec<Entity> = {
            let w = sim.app.world_mut();
            w.query_filtered::<Entity, With<Hostage>>().iter(w).collect()
        };
        assert_eq!(ours.len(), theirs.len(), "{name}: hostages");
        for e in ours {
            let p = sim.position(e) - Vec3::Y * LIFT;
            // Engine metres to Source units (x, -z, y).
            let feet = Vec3::new(p.x, -p.z, p.y) / UNIT;
            let want = theirs
                .iter()
                .map(|w| Vec3::from_array(*w))
                .min_by(|a, b| a.xy().distance(feet.xy()).total_cmp(&b.xy().distance(feet.xy())))
                .unwrap();
            let off = feet - want;
            let ok = off.length() < 1.0 && sim.state(e).on_ground;
            bad += usize::from(!ok);
            report.push(format!(
                "{name}: ours {feet:.2} theirs {want:.1} off {off:.2}{}",
                if ok { "" } else { "  <- wrong" }
            ));
        }
    }
    let report = report.join("\n");
    eprintln!("{report}");
    assert_eq!(bad, 0, "{report}");
}
