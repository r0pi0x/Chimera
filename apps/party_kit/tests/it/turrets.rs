//! Combat Arms' auto turret as a gun game rung (docs/DESIGN.md "Gun game
//! rules", "A turret rung"): placed by the rung's weapon, its kill is its
//! owner's with the turret weapon (a rung up, the kill feed's icon), and
//! it goes with the rung (a rung swap, its owner's death, his leaving, a
//! map's unload); any game's weapon breaks it (a kill-feed line, no rung);
//! a bot on the rung places one. On the party's simulation (greybox,
//! `PartyPartsPlugin` on `CaSim`) with a hand-written ladder: the turret,
//! CS:S's Desert Eagle, Combat Arms' M4A1.

use bevy::prelude::*;
use chimera_client::client::kill_feed::{KillFeed, KillFeedStatePlugin};
use chimera_core::core::{Damage, DamageKind, Destroyed, Died, Health, Hitgroup};
use chimera_sim::{
    minigame::{self, MinigameAppExt, Minigames, ladder::{self, Ladder}},
    testkit,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::special::{CaObject, DeviceKind, Kind},
    harness::CaSim,
};
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin};

use crate::gun_game::{OPEN, alive, held, no_finale, place, plain_rungs, rung, soldier};

const TURRET: &str = "combat_arms:auto-turret";
const DEAGLE: &str = "cs_source:weapon_deagle";
const ID: &str = "test:turret_game";

/// Deaths and broken things seen: (victim, killer, weapon), (what,
/// owner, breaker, weapon).
#[derive(Resource, Default)]
struct Seen {
    deaths: Vec<(Entity, Option<Entity>, Option<&'static str>)>,
    broken: Vec<Destroyed>,
}

fn note(mut died: MessageReader<Died>, mut destroyed: MessageReader<Destroyed>, mut seen: ResMut<Seen>) {
    for d in died.read() {
        seen.deaths.push((d.entity, d.attacker, d.damage.weapon));
    }
    seen.broken.extend(destroyed.read().copied());
}

/// The gun game with the ladder turret, Desert Eagle, M4A1 (no finale,
/// no rung extras), started.
fn turret_game() -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, KillFeedStatePlugin))
            .init_resource::<Seen>()
            .add_systems(FixedLast, note);
    });
    plain_rungs(sim.app.world_mut());
    no_finale(sim.app.world_mut());
    let text = sim.app.world().resource::<Minigames>().text(DEFAULT_MINIGAME).unwrap().to_string();
    let start = text.find("ladder = [").unwrap();
    let end = start + text[start..].find(']').unwrap() + 1;
    let text = format!("{}ladder = [\"{TURRET}\", \"{DEAGLE}\", \"combat_arms:m4a1\"]{}", &text[..start], &text[end..])
        .replace(&format!("id = \"{DEFAULT_MINIGAME}\""), &format!("id = \"{ID}\""));
    sim.app.add_minigames(&[("turret.toml", &text)]).unwrap();
    minigame::start(sim.app.world_mut(), ID).expect("the turret game starts");
    sim
}

/// The live turrets: (entity, owner, landed).
fn turrets(sim: &mut Sim) -> Vec<(Entity, Option<Entity>, bool)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &CaObject)>()
        .iter(w)
        .filter(|(_, o)| matches!(&o.kind, Kind::Device(p) if p.kind == DeviceKind::Turret))
        .map(|(e, o)| (e, o.owner, o.s.landed))
        .collect()
}

fn turret_of(sim: &mut Sim, owner: Entity) -> Option<Entity> {
    turrets(sim).into_iter().find(|t| t.1 == Some(owner)).map(|t| t.0)
}

/// Two soldiers in, both past their spawn protection: `a` on the turret
/// rung at the open middle facing -Z, `b` on rung `b_rung` 7 m in front
/// of him.
fn two(sim: &mut Sim, b_rung: usize) -> (Entity, Entity) {
    let a = soldier(sim);
    let b = soldier(sim);
    sim.seconds(1.5);
    let l = sim.app.world().resource::<Ladder>().clone();
    ladder::set_rung(sim.app.world_mut(), b, b_rung, &l);
    sim.seconds(3.0);
    place(sim, a, OPEN);
    place(sim, b, OPEN + Vec3::new(0.0, 0.0, -7.0));
    sim.intent(a).yaw = 0.0;
    sim.ticks(2);
    (a, b)
}

/// `p` places a turret a little ahead (the rung's weapon drawn, a press);
/// it has landed.
fn deploy(sim: &mut Sim, p: Entity) -> Entity {
    for _ in 0..(3.0 * sim.tick_hz()) as u32 {
        if held(sim, p) == Some(TURRET) {
            break;
        }
        sim.ticks(1);
    }
    assert_eq!(held(sim, p), Some(TURRET), "the turret rung's weapon in hand");
    {
        let mut i = sim.intent(p);
        i.pitch = -0.5;
        i.fire = true;
    }
    sim.ticks(1);
    sim.intent(p).fire = false;
    for _ in 0..(8.0 * sim.tick_hz()) as u32 {
        sim.ticks(1);
        if let Some(t) = turrets(sim).into_iter().find(|t| t.1 == Some(p) && t.2) {
            return t.0;
        }
    }
    panic!("no turret landed: {:?}", turrets(sim));
}

fn hurt(sim: &mut Sim, victim: Entity, by: Option<Entity>, amount: f32) {
    let point = sim.position(victim);
    sim.app.world_mut().write_message(Damage {
        target: victim,
        attacker: by,
        amount,
        point,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
}

#[test]
fn a_turrets_kill_climbs_its_owner_and_takes_the_turret_away() {
    let mut sim = turret_game();
    let (a, b) = two(&mut sim, 2);
    assert_eq!(rung(&sim, a), 0);
    deploy(&mut sim, a);
    // Set up (2.833 s), then 12 a ray every 0.15 s: dead in a few seconds.
    let mut killed = false;
    for _ in 0..(10.0 * sim.tick_hz()) as u32 {
        sim.ticks(1);
        if !alive(&sim, b) {
            killed = true;
            break;
        }
    }
    assert!(killed, "the turret killed b");
    let deaths = sim.app.world().resource::<Seen>().deaths.clone();
    assert!(
        deaths.iter().any(|d| *d == (b, Some(a), Some(TURRET))),
        "the owner's kill with the turret: {deaths:?}"
    );
    sim.ticks(2);
    assert_eq!(rung(&sim, a), 1, "a rung up");
    // The kill feed names the turret weapon (its icon).
    let feed = sim.app.world().resource::<KillFeed>();
    assert!(feed.entries.iter().any(|e| e.weapon == Some(TURRET)), "the feed's line: {:?}", feed.entries);
    // Off the rung: his turret goes at once (the next server frame).
    sim.seconds(0.15);
    assert_eq!(turret_of(&mut sim, a), None, "the turret went with the rung");
}

#[test]
fn a_turret_goes_with_its_owners_death_and_his_leaving() {
    // Death.
    let mut sim = turret_game();
    let (a, b) = two(&mut sim, 2);
    deploy(&mut sim, a);
    hurt(&mut sim, a, Some(b), 10.0);
    sim.seconds(0.2);
    assert!(!alive(&sim, a));
    assert_eq!(turret_of(&mut sim, a), None, "gone with its owner's death");

    // Leaving.
    let mut sim = turret_game();
    let (a, _b) = two(&mut sim, 2);
    let t = deploy(&mut sim, a);
    sim.app.world_mut().entity_mut(a).despawn();
    sim.seconds(0.2);
    assert!(sim.app.world().get_entity(t).is_err(), "gone with its owner");

    // A demotion (one's own death costs a rung: the death takes it too) and
    // a rung swap without a kill.
    let mut sim = turret_game();
    let (a, _b) = two(&mut sim, 2);
    deploy(&mut sim, a);
    let l = sim.app.world().resource::<Ladder>().clone();
    ladder::set_rung(sim.app.world_mut(), a, 1, &l);
    sim.seconds(0.2);
    assert_eq!(turret_of(&mut sim, a), None, "gone with the rung");
}

#[test]
fn a_maps_unload_takes_the_turrets() {
    let mut sim = turret_game();
    let (a, _) = two(&mut sim, 2);
    deploy(&mut sim, a);
    chimera_world::map::unload_map(sim.app.world_mut());
    assert!(turrets(&mut sim).is_empty(), "no turret outlives its map");
}

#[test]
fn another_games_gun_breaks_a_turret_for_a_feed_line_and_no_rung() {
    let mut sim = turret_game();
    let (a, b) = two(&mut sim, 1);
    let t = deploy(&mut sim, a);
    // b holds the Desert Eagle and lives long enough to empty it.
    sim.app.world_mut().entity_mut(b).insert(Health { current: 100.0, max: 100.0 });
    for _ in 0..(3.0 * sim.tick_hz()) as u32 {
        if held(&sim, b) == Some(DEAGLE) {
            break;
        }
        sim.ticks(1);
    }
    assert_eq!(held(&sim, b), Some(DEAGLE));
    let before = rung(&sim, b);
    let mut broke = false;
    for k in 0..(10.0 * sim.tick_hz()) as u32 {
        let Ok(at) = sim.app.world().get_entity(t).map(|e| e.get::<Transform>().unwrap().translation) else {
            broke = true;
            break;
        };
        testkit::aim_at(&mut sim, b, at);
        sim.intent(b).fire = k % 8 == 0;
        sim.ticks(1);
    }
    assert!(broke, "the Desert Eagle broke the turret");
    let seen = sim.app.world().resource::<Seen>();
    let line = seen.broken.iter().find(|d| d.what == TURRET).copied().expect("a broken line");
    assert_eq!((line.owner, line.attacker, line.weapon), (Some(a), Some(b), Some(DEAGLE)));
    assert_eq!(rung(&sim, b), before, "no rung for a turret");
    let feed = sim.app.world().resource::<KillFeed>();
    assert!(
        feed.entries.iter().any(|e| e.victim.name.contains("'s ") && e.weapon == Some(DEAGLE)),
        "the breaker's feed line: {:?}",
        feed.entries
    );
}

#[test]
fn a_bot_on_the_turret_rung_places_one() {
    let mut sim = turret_game();
    for x in [-8.0, 8.0] {
        sim.app.world_mut().spawn((
            chimera_core::core::SpawnPoint::default(),
            Transform::from_translation(Vec3::new(x, 1.0, -10.0)),
        ));
    }
    {
        let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 2;
        s.quota_set = true;
    }
    let mut placed = false;
    for _ in 0..30 {
        sim.seconds(1.0);
        let bots: Vec<Entity> = {
            let w = sim.app.world_mut();
            w.query_filtered::<Entity, With<combat_arms::game::bots::CaBot>>().iter(w).collect()
        };
        if turrets(&mut sim).iter().any(|t| t.1.is_some_and(|o| bots.contains(&o)) && t.2) {
            placed = true;
            break;
        }
    }
    assert!(placed, "a bot placed its turret: {:?}", turrets(&mut sim));
}
