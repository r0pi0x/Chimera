//! The sandbox's second round of mechanics (chimera_sim's
//! `minigame::layout`, `conditions`, `rung_command`'s `as <body>`,
//! `targets`' own second bars, `props`' projectile misses and surfaces):
//! a layout cleared and repeated from another spot (one item refused
//! there), kept across a restart; every rung of the sandbox reachable by
//! its gun id, by the console and the request; TF2's shotgun as the class
//! asked for; the conditions as requests with their state; a body that
//! reports its own second bar; a projectile at a board that never lands
//! is a miss; props name their surfaces. On the party's simulation,
//! headless.

use bevy::prelude::*;
use chimera_core::{
    console::{Cheats, Console},
    core::{Damage, DamageKind, God, Health, Hitgroup, Intent, LocalPlayer, PropSurface},
};
use chimera_sim::{
    minigame::{
        self,
        conditions::{Condition, SandboxConditions, SetCondition},
        ladder::{Ladder, Rung},
        layout::{RepeatLayout, SandboxLayout},
        props,
        rung_command::{ChosenBody, PutOnGun},
        targets::{self, PropSpec, ShotGroup, SpawnProp, SpawnTarget, Target, TargetCharacter, TargetCharacters, TargetMotion, TargetVitals},
    },
    testkit,
    weapon::{Magazine, WeaponEvent, WeaponEventKind},
};
use chimera_world::harness::Sim;
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

const GUN: &str = "combat_arms:m4a1";
const SHOTGUN: &str = "tf2:tf_weapon_shotgun_primary";

fn play() -> (Sim, Entity) {
    let mut sim = ugg();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    (sim, me)
}

fn run(sim: &mut Sim, line: &str) -> String {
    let before = sim.app.world().resource::<Console>().output.len();
    sim.app.world_mut().resource_mut::<Console>().submit(line.to_string());
    sim.ticks(3);
    sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

/// `me` at `at` (feet on the greybox floor), looking along `yaw`.
fn stand(sim: &mut Sim, me: Entity, at: Vec3, yaw: f32) {
    chimera_sim::spawning::revive(sim.app.world_mut(), me, Some(Transform::from_translation(at)));
    chimera_core::core::set_view(sim.app.world_mut(), me, yaw, 0.0);
    sim.ticks(3);
}

fn targets_now(sim: &mut Sim) -> Vec<Target> {
    targets::all_targets(sim.app.world_mut()).into_iter().map(|(_, t)| t).collect()
}

fn props_now(sim: &mut Sim) -> Vec<targets::Prop> {
    props::all_props(sim.app.world_mut()).into_iter().map(|(_, p)| p).collect()
}

#[test]
fn a_layout_is_cleared_and_repeated_from_another_spot() {
    let (mut sim, me) = play();
    stand(&mut sim, me, Vec3::new(0.0, 1.0, 0.0), 0.0);
    let spawn = |sim: &mut Sim, req: SpawnTarget| targets::spawn(sim.app.world_mut(), &req).unwrap_or_else(|e| panic!("{e}"));
    spawn(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 8.0, ..default() });
    sim.app.world_mut().get_mut::<Intent>(me).unwrap().yaw = 0.3;
    spawn(&mut sim, SpawnTarget { gun: Some(GUN.into()), distance_m: 30.0, motion: TargetMotion::Strafes, protected: true, ..default() });
    sim.app.world_mut().get_mut::<Intent>(me).unwrap().yaw = -0.3;
    props::spawn(sim.app.world_mut(), &SpawnProp { spec: PropSpec { kind: "target_board".into(), distance_m: 10.0 }, by: None }).unwrap();
    let layout = sim.app.world().resource::<SandboxLayout>().clone();
    assert_eq!((layout.targets.len(), layout.props.len()), (2, 1), "{layout:?}");
    assert!(layout.summary.starts_with("2 targets, 1 prop:"), "{}", layout.summary);
    assert!((layout.targets[1].yaw - 0.3).abs() < 1e-4 && (layout.props[0].yaw + 0.3).abs() < 1e-4, "{layout:?}");
    // Cleared (the bench's Clear): the layout stays.
    targets::clear(sim.app.world_mut());
    props::clear(sim.app.world_mut());
    sim.ticks(3);
    assert!(targets_now(&mut sim).is_empty() && props_now(&mut sim).is_empty());
    assert_eq!(sim.app.world().resource::<SandboxLayout>(), &layout, "a clear isn't a change of the layout");
    // A sandbox restart keeps it too.
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts again");
    sim.seconds(1.0);
    assert_eq!(sim.app.world().resource::<SandboxLayout>(), &layout);
    // From another spot, turned a quarter: the same arrangement about the player.
    let here = Vec3::new(-20.0, 1.0, 20.0);
    let yaw = -std::f32::consts::FRAC_PI_2;
    stand(&mut sim, me, here, yaw);
    sim.app.world_mut().write_message(RepeatLayout::default());
    sim.seconds(1.5);
    let ts = targets_now(&mut sim);
    let ps = props_now(&mut sim);
    assert_eq!((ts.len(), ps.len()), (2, 1), "{ts:?} {ps:?}");
    let at = sim.position(me);
    let along = |y: f32, d: f32| at + Vec3::new(-(yaw + y).sin(), 0.0, -(yaw + y).cos()) * d;
    assert!((ts[0].home.xz() - along(0.0, 8.0).xz()).length() < 0.3, "{:?} vs {:?}", ts[0].home, along(0.0, 8.0));
    assert!((ts[1].home.xz() - along(0.3, 30.0).xz()).length() < 0.3, "{:?}", ts[1].home);
    assert!((ps[0].home.xz() - along(-0.3, 10.0).xz()).length() < 0.3, "{:?}", ps[0].home);
    assert!(ts[1].protected && ts[1].motion == TargetMotion::Strafes && ts[1].gun == GUN, "its settings: {:?}", ts[1]);
    // Facing out near the edge: the far one has no floor, the rest are placed.
    stand(&mut sim, me, Vec3::new(0.0, 1.0, 25.0), std::f32::consts::PI);
    let said = run(&mut sim, "sandbox repeat");
    assert!(said.contains("2 placed") && said.contains("refused"), "{said}");
    assert_eq!(targets_now(&mut sim).len() + props_now(&mut sim).len(), 2);
    assert_eq!(sim.app.world().resource::<SandboxLayout>(), &layout, "a repeat leaves the layout as it was");
}

#[test]
fn every_rung_of_the_sandbox_takes_its_gun_id_by_the_console_and_the_request() {
    let (mut sim, me) = play();
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let usable = &ladder.rungs[..ladder.rungs.len() - 1];
    let mut failed = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for (i, id) in usable.iter().enumerate() {
        if seen.contains(id) {
            continue;
        }
        seen.push(id);
        run(&mut sim, &format!("rung {id}"));
        sim.ticks(2);
        let rung = sim.app.world().get::<Rung>(me).map(|r| r.0);
        if rung != Some(i) {
            failed.push(format!("console {id}: rung {rung:?}, not {i}"));
        }
        // Back to the first, then the request.
        run(&mut sim, "rung 1");
        sim.app.world_mut().write_message(PutOnGun { gun: id.to_string(), character: None, who: None });
        sim.ticks(3);
        let rung = sim.app.world().get::<Rung>(me).map(|r| r.0);
        if rung != Some(i) {
            failed.push(format!("request {id}: rung {rung:?}, not {i}"));
        }
        let held = testkit::active_id(sim.app.world(), me);
        if held != Some(*id) {
            failed.push(format!("{id}: holds {held:?}"));
        }
    }
    eprintln!("{} guns checked: {}", seen.len(), seen.join(" "));
    assert!(failed.is_empty(), "{failed:#?}");
    for game in ["cs_source:", "hl2dm:", "halo_ce:", "tf2:", "combat_arms:"] {
        assert!(seen.iter().any(|g| g.starts_with(game)), "{game} on the ladder: {seen:?}");
    }
}

#[test]
fn tf2s_shotgun_is_the_class_asked_for() {
    let (mut sim, me) = play();
    let max = |sim: &Sim| sim.app.world().get::<Health>(me).map(|h| h.max).unwrap_or(0.0);
    let said = run(&mut sim, &format!("rung {SHOTGUN} as tf2:soldier"));
    sim.seconds(0.5);
    assert!(said.contains("as tf2:soldier"), "{said}");
    assert!((max(&sim) - 2.0).abs() < 1e-3, "the Soldier's 200: {}", max(&sim));
    sim.app.world_mut().write_message(PutOnGun { gun: SHOTGUN.into(), character: Some("tf2:heavy".into()), who: None });
    sim.seconds(0.5);
    assert!((max(&sim) - 3.0).abs() < 1e-3, "the Heavy's 300: {}", max(&sim));
    assert_eq!(sim.app.world().get::<ChosenBody>(me).map(|b| b.0.as_str()), Some("tf2:heavy"));
    // A class that never holds it, or another game's body: refused, nothing changes.
    for bad in ["tf2:scout", "halo_ce:spartan"] {
        let said = run(&mut sim, &format!("rung {SHOTGUN} as {bad}"));
        assert!(!said.contains(" as "), "{bad}: {said}");
        assert!((max(&sim) - 3.0).abs() < 1e-3, "{bad} changed nothing");
    }
    // Without `as`: the body chosen goes, the rung's own draw again.
    run(&mut sim, &format!("rung {GUN}"));
    sim.seconds(0.5);
    assert!(sim.app.world().get::<ChosenBody>(me).is_none());
}

#[test]
fn the_conditions_are_requests_with_their_state() {
    let (mut sim, me) = play();
    // `god` is the client's console's (headless: added here).
    chimera_net::net::cheats::player_commands(&mut sim.app);
    let state = |sim: &Sim| *sim.app.world().resource::<SandboxConditions>();
    assert!(state(&sim).endless_ammo && !state(&sim).god && !state(&sim).bots_frozen, "{:?}", state(&sim));
    sim.app.world_mut().write_message(SetCondition(Condition::God(true)));
    sim.ticks(4);
    assert!(sim.app.world().get::<God>(me).is_some() && state(&sim).god, "{:?}", state(&sim));
    run(&mut sim, "sandbox god off");
    sim.ticks(2);
    assert!(sim.app.world().get::<God>(me).is_none() && !state(&sim).god);
    // Endless ammo off: the reserve runs down; on: it fills again.
    run(&mut sim, &format!("rung {GUN}"));
    sim.seconds(0.5);
    run(&mut sim, "sandbox ammo off");
    let gun = sim.app.world().get::<chimera_sim::weapon::Inventory>(me).and_then(|i| i.active).expect("a gun");
    sim.app.world_mut().get_mut::<Magazine>(gun).unwrap().reserve = 0;
    sim.seconds(0.5);
    assert_eq!(sim.app.world().get::<Magazine>(gun).unwrap().reserve, 0, "no refill");
    assert!(!state(&sim).endless_ammo);
    sim.app.world_mut().write_message(SetCondition(Condition::EndlessAmmo(true)));
    sim.seconds(0.5);
    assert!(sim.app.world().get::<Magazine>(gun).unwrap().reserve > 0, "refilled");
    // Bots frozen: a strafing target holds still.
    let t = targets::spawn(sim.app.world_mut(), &SpawnTarget { gun: Some(GUN.into()), distance_m: 8.0, motion: TargetMotion::Strafes, ..default() }).unwrap();
    sim.seconds(1.5);
    run(&mut sim, "sandbox freeze on");
    sim.seconds(1.0);
    let a = sim.position(t);
    sim.seconds(1.5);
    assert!((sim.position(t) - a).xz().length() < 0.2, "frozen: {a} -> {}", sim.position(t));
    run(&mut sim, "sandbox freeze off");
    sim.seconds(1.5);
    assert!((sim.position(t) - a).xz().length() > 0.5, "strafing again");
    assert!(run(&mut sim, "sandbox conditions").contains("SandboxConditions god=off ammo=on"));
    assert!(run(&mut sim, "condition spread off").contains("condition"), "no such switch: the usage");
}

#[test]
fn conditions_are_refused_outside_the_sandbox_without_cheats() {
    let mut sim = ugg();
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.app.insert_resource(Cheats(0));
    sim.seconds(0.5);
    assert!(minigame::conditions::set(sim.app.world_mut(), Condition::BotsFrozen(true)).is_err());
    sim.app.insert_resource(Cheats(1));
    assert!(minigame::conditions::set(sim.app.world_mut(), Condition::BotsFrozen(true)).is_ok(), "a local match with cheats");
}

#[test]
fn a_body_reports_its_own_second_bar() {
    let (mut sim, _) = play();
    sim.app.world_mut().resource_mut::<TargetCharacters>().0.insert(
        0,
        TargetCharacter {
            id: "combat_arms:disc",
            name: "Disc",
            game: "combat_arms",
            holds: |_, _, gun| gun.starts_with("combat_arms:"),
            wear: |_, _, _| {},
            vitals: |_, _| (1.0, 0.5),
            second_bar: Some(|_, _| Some(0.42)),
        },
    );
    let e = targets::spawn(sim.app.world_mut(), &SpawnTarget { character: Some("combat_arms:disc".into()), gun: Some(GUN.into()), distance_m: 8.0, ..default() })
        .unwrap_or_else(|e| panic!("{e}"));
    sim.seconds(1.0);
    let v = *sim.app.world().get::<TargetVitals>(e).unwrap();
    assert!((v.shield - 0.42).abs() < 1e-6, "its own bar, not its armour: {v:?}");
}

#[test]
fn a_projectile_at_the_board_that_never_lands_is_a_miss() {
    let (mut sim, me) = play();
    stand(&mut sim, me, Vec3::new(0.0, 1.0, 0.0), 0.0);
    let board = props::spawn(sim.app.world_mut(), &SpawnProp { spec: PropSpec { kind: "target_board".into(), distance_m: 10.0 }, by: None }).unwrap();
    sim.ticks(2);
    let centre = sim.app.world().get::<Transform>(board).unwrap().transform_point(Vec3::new(0.0, props::BOARD_CENTRE_Y, 0.0));
    let eye = sim.position(me) + Vec3::Y * 0.6;
    let shot = |sim: &mut Sim| {
        let kind = WeaponEventKind::ShotMade { id: "halo_ce:plasma_rifle", origin: eye, dir: (centre - eye).normalize() };
        sim.app.world_mut().write_message(WeaponEvent { owner: me, weapon: Entity::PLACEHOLDER, kind, replay: false });
        sim.ticks(1);
    };
    // One that lands (its direct hit a tick later), one that never does.
    shot(&mut sim);
    let face = sim
        .app
        .world_mut()
        .query::<(Entity, &ChildOf, &PropSurface, &Transform)>()
        .iter(sim.app.world())
        .find(|(_, c, s, t)| c.parent() == board && s.0 == "wood" && t.translation.y > 1.0)
        .map(|(e, ..)| e);
    let face = face.expect("the board's parts name their surface");
    sim.ticks(2);
    sim.app.world_mut().write_message(Damage {
        target: face,
        attacker: Some(me),
        amount: 0.2,
        point: centre + (eye - centre).normalize() * 0.03,
        dir: (centre - eye).normalize(),
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: Some("halo_ce:plasma_rifle"),
        force: Vec3::ZERO,
    });
    sim.ticks(1);
    shot(&mut sim);
    let g = |sim: &Sim| sim.app.world().get::<ShotGroup>(board).cloned().unwrap();
    assert_eq!((g(&sim).hits.len(), g(&sim).misses), (1, 0), "{:?}", g(&sim));
    sim.seconds(props::PROJECTILE_MISS_SECS + 0.5);
    assert_eq!((g(&sim).hits.len(), g(&sim).misses), (1, 1), "the one that never landed: {:?}", g(&sim));
}
