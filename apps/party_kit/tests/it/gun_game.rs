//! The original gun game (`originals:gun_game_dust2`, minigames/
//! gun_game_dust2.toml) on the party's simulation (`PartyPartsPlugin` on
//! `combat_arms::harness::CaSim`), headless on the greybox with scripted
//! Combat Arms soldiers: the definition checks against both games' parts
//! and takes the gameplay's rules; a scripted player shooting another
//! with each rung's weapon (its own aim and trigger, the weapon frame's
//! hits) climbs the ladder of CS:S's and Combat Arms' guns to the win; a
//! knife death costs a rung (the knife is the gun's game's: switches.rs).

use avian3d::prelude::Position;
use bevy::prelude::*;
use chimera_core::{
    core::{Dead, God, Score, Team},
    gameplay::Gameplay,
};
use chimera_sim::{
    minigame::{self, MINIGAME, MinigameEvent, ladder::Rung},
    mode::{Match, Phase},
    testkit,
    weapon::{Inventory, Weapon},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::movement, harness::CaSim};
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin};

/// The party's simulation on the greybox, the gun game started (its
/// rung extras off: `plain_rungs`).
pub(crate) fn party() -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    plain_rungs(sim.app.world_mut());
    no_finale(sim.app.world_mut());
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim
}

/// The host's setting "Knife finale: off" for this world: the tests of
/// the guns' ladder (ten rungs) predate the melee finale, which is on by
/// default (`the_gun_game_ends_on_a_melee_rung_by_default` and the full
/// game test play it).
pub(crate) fn no_finale(world: &mut World) {
    world
        .get_resource_or_init::<chimera_sim::minigame::overrides::MinigameOverrides>()
        .keys
        .insert("rules.knife_finale".into(), "false".into());
}

/// The ladders this world makes from now on draw no rung extras (no
/// game's hook: `ladder::RungExtrasHooks` gone), as before them: for the
/// tests of everything else (the kit's knife, one's own look and the
/// guns' own parts on every rung); rung_extras.rs tests them.
pub(crate) fn plain_rungs(world: &mut World) {
    world.remove_resource::<chimera_sim::minigame::ladder::RungExtrasHooks>();
}

/// A Combat Arms soldier, not yet in the game (the minigame takes it in).
/// The greybox's open middle, where `line_up` stands players (feet just
/// above the floor).
pub(crate) const OPEN: Vec3 = Vec3::new(0.0, 1.0, 0.0);

pub(crate) fn soldier(sim: &mut Sim) -> Entity {
    sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID)
}

pub(crate) fn place(sim: &mut Sim, e: Entity, at: Vec3) {
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(e).unwrap().translation = at;
    w.get_mut::<Position>(e).unwrap().0 = at;
}

pub(crate) fn held(sim: &Sim, e: Entity) -> Option<&'static str> {
    testkit::active_id(sim.app.world(), e)
}

pub(crate) fn rung(sim: &Sim, e: Entity) -> usize {
    sim.app.world().get::<Rung>(e).map_or(0, |r| r.0)
}

pub(crate) fn phase(sim: &Sim) -> Phase {
    sim.app.world().resource::<Match>().phase
}

pub(crate) fn alive(sim: &Sim, e: Entity) -> bool {
    sim.app.world().get::<Dead>(e).is_none()
}

/// `shooter` aims at `target`'s centre and works the trigger (pressed
/// every other tick, for the semi-automatics) until `target` dies or
/// `secs` pass; whether it died.
pub(crate) fn shoot_until_dead(sim: &mut Sim, shooter: Entity, target: Entity, secs: f64) -> bool {
    let ticks = (secs * sim.tick_hz()) as u64;
    for t in 0..ticks {
        let at = sim.position(target);
        testkit::aim_at(sim, shooter, at);
        // Held against the recoil (the view punch the shots climb by), as
        // a player pulls down: a long burst otherwise climbs over the head.
        let punch = sim.app.world().get::<chimera_sim::weapon::ViewPunch>(shooter).map_or(Vec3::ZERO, |p| p.0);
        {
            let mut i = sim.intent(shooter);
            i.pitch -= punch.x;
            i.yaw -= punch.y;
            i.fire = t % 2 == 0;
        }
        sim.ticks(1);
        if !alive(sim, target) {
            sim.intent(shooter).fire = false;
            return true;
        }
    }
    sim.intent(shooter).fire = false;
    false
}

/// `target` back in play, unprotected, `d` m in front of `shooter` (both
/// standing still, on the greybox's open middle: a spawn spot may be near
/// a ledge), the shooter's weapon drawn.
pub(crate) fn line_up(sim: &mut Sim, shooter: Entity, target: Entity, d: f32) {
    for _ in 0..(6.0 * sim.tick_hz()) as u32 {
        if alive(sim, target) && sim.app.world().get::<God>(target).is_none() {
            break;
        }
        sim.ticks(1);
    }
    assert!(alive(sim, target), "respawned");
    let from = OPEN;
    place(sim, shooter, from);
    place(sim, target, from + Vec3::new(0.0, 0.0, -d));
    // Settle, and finish drawing what the last kill gave.
    sim.seconds(1.3);
}

#[test]
fn the_gun_game_checks_against_both_games_and_takes_the_rules() {
    let mut sim = party();
    let g = sim.app.world().resource::<Gameplay>().clone();
    assert_eq!((g.rules, g.hud, g.binds), (MINIGAME, "combat_arms", "combat_arms"));
    assert_eq!(g.tick, combat_arms::game::INFO.tick(), "Combat Arms' tick");
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    assert_eq!(phase(&sim), Phase::Live);
    for p in [a, b] {
        assert_eq!(held(&sim, p), Some(cs_source::game::weapons::GLOCK), "the first rung: CS:S's Glock");
        let ids: Vec<&str> = {
            let w = sim.app.world();
            w.get::<Inventory>(p).unwrap().weapons.iter().filter_map(|x| w.get::<Weapon>(*x)).map(|x| x.id).collect()
        };
        assert!(ids.contains(&cs_source::game::weapons::KNIFE), "the kit's knife, the Glock's game's: {ids:?}");
    }
}

/// A scripted player shoots the other with every rung's gun, CS:S's and
/// Combat Arms' in turn, then kills with the melee finale's weapon (the
/// definition ends on one melee rung for ten guns): the match's end.
#[test]
fn a_full_gun_game_runs_to_a_winner_with_both_games_guns() {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    plain_rungs(sim.app.world_mut());
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.0);
    assert_eq!(phase(&sim), Phase::Live);
    let mut fired_with = Vec::new();
    let mut events = Vec::new();
    for kill in 0..11 {
        // The melee rung: close up.
        line_up(&mut sim, a, b, if kill == 10 { 1.2 } else { 4.0 });
        let gun = held(&sim, a).expect("a weapon in hand");
        assert!(
            shoot_until_dead(&mut sim, a, b, 8.0),
            "kill {kill}: {gun} didn't kill in 8 s (health {:?})",
            sim.app.world().get::<chimera_core::core::Health>(b).map(|h| h.current)
        );
        fired_with.push(gun);
        events.extend(
            sim.app
                .world_mut()
                .resource_mut::<Messages<MinigameEvent>>()
                .drain()
                .map(|e| (e.name, e.value)),
        );
        if kill < 10 {
            assert_eq!(rung(&sim, a), kill + 1, "a rung a kill");
        }
    }
    assert!(fired_with.iter().any(|g| g.starts_with("cs_source:")), "{fired_with:?}");
    assert!(fired_with.iter().any(|g| g.starts_with("combat_arms:")), "{fired_with:?}");
    assert_eq!(fired_with[9], cs_source::game::weapons::AWP, "the last gun");
    let melee = fired_with[10];
    let catalog = sim.app.world().resource::<chimera_sim::weapon::data::WeaponCatalog>();
    let is_melee = catalog.get(melee).is_some_and(|w| w.melee.is_some() || w.has_hook(combat_arms::game::inventory::KNIFE_HOOK));
    assert!(is_melee, "the finale's rung is a melee weapon: {melee}");
    assert!(events.contains(&("rung_up", 10)), "{events:?}");
    assert!(
        events.iter().all(|(n, _)| minigame::EVENT_NAMES.contains(n)),
        "every event the match raises is one the network can send: {events:?}"
    );
    let team_a = sim.app.world().get::<Team>(a).unwrap().0;
    assert_eq!(sim.app.world().resource::<Match>().winner, Some(team_a));
    sim.seconds(9.0);
    assert_eq!(phase(&sim), Phase::MatchOver);
    assert_eq!(sim.app.world().resource::<Match>().result, Some(Some(team_a)));
    assert_eq!(sim.app.world().get::<Score>(a).unwrap().kills, 11);
    assert_eq!(sim.app.world().get::<Score>(b).unwrap().deaths, 11);
}

/// The original gun game ends on melee by default: one melee rung for its
/// ten guns, a melee weapon of the installed games' (never the infected's
/// hands unless as the infected, once at most), on every seed; Host
/// settings off take the finale away.
#[test]
fn the_gun_game_ends_on_a_melee_rung_by_default() {
    use chimera_sim::minigame::ladder::{Ladder, RungSeed};
    for seed in 0..40 {
        let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
        sim.app.insert_resource(RungSeed(seed));
        minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
        let w = sim.app.world();
        let l = w.resource::<Ladder>();
        let catalog = w.resource::<chimera_sim::weapon::data::WeaponCatalog>();
        assert!(l.knife_finale && l.melee_rungs == 1 && l.rungs.len() == 11, "seed {seed}: {:?}", l.rungs);
        let last = *l.rungs.last().unwrap();
        let is_melee = |id: &str| catalog.get(id).is_some_and(|w| w.melee.is_some() || w.has_hook(combat_arms::game::inventory::KNIFE_HOOK));
        assert!(is_melee(last), "{last}");
        assert!(!catalog.get(last).unwrap().has_hook(combat_arms::game::rungs::INFECTED), "{last}: hands only as the character");
        let characters = (0..l.rungs.len()).filter(|i| l.character(*i).is_some()).count();
        assert!(characters <= 1 && (0..10).all(|i| l.character(i).is_none()), "seed {seed}: {:?}", l.extras);
    }
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    sim.app.world_mut().resource_mut::<chimera_sim::minigame::overrides::MinigameOverrides>().keys.insert("rules.knife_finale".into(), "false".into());
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let l = sim.app.world().resource::<chimera_sim::minigame::ladder::Ladder>();
    assert!(!l.knife_finale && l.rungs.len() == 10, "no finale with the host's setting off");
}

/// CS:S's knife (the kit's, with the Glock) kills a player on the fourth
/// rung: it goes back to the third, CS:S's Desert Eagle at its respawn.
#[test]
fn a_knife_death_costs_a_rung() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.0);
    sim.app.world_mut().entity_mut(b).insert(Rung(3));
    // a takes out its knife.
    let knife = {
        let w = sim.app.world();
        let inv = w.get::<Inventory>(a).unwrap();
        inv.weapons
            .iter()
            .copied()
            .find(|x| w.get::<Weapon>(*x).is_some_and(|x| x.id == cs_source::game::weapons::KNIFE))
            .unwrap()
    };
    sim.app.world_mut().get_mut::<Inventory>(a).unwrap().wanted = Some(knife);
    line_up(&mut sim, a, b, 1.2);
    assert_eq!(held(&sim, a), Some(cs_source::game::weapons::KNIFE));
    assert!(shoot_until_dead(&mut sim, a, b, 8.0), "knifed");
    assert_eq!(rung(&sim, b), 2, "a rung down");
    assert_eq!(rung(&sim, a), 1, "the killer still climbs");
    line_up(&mut sim, a, b, 4.0);
    assert_eq!(held(&sim, b), Some(cs_source::game::weapons::DEAGLE));
}

/// Combat Arms' bots (the gun game's `bots` slot) come in, each on a team
/// of its own, and fight it out with the ladder's guns: kills move them up.
#[test]
fn combat_arms_bots_play_the_gun_game() {
    let mut sim = party();
    // Eight more start points about the greybox's floor (it has two).
    for x in [-8.0, 8.0] {
        for z in [-25.0, -15.0, -5.0, 5.0] {
            sim.app.world_mut().spawn((
                chimera_core::core::SpawnPoint::default(),
                Transform::from_translation(Vec3::new(x, 1.0, z)),
            ));
        }
    }
    {
        let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 4;
        s.quota_set = true;
    }
    sim.seconds(2.0);
    let bots: Vec<Entity> = {
        let w = sim.app.world_mut();
        w.query_filtered::<Entity, With<combat_arms::game::bots::CaBot>>().iter(w).collect()
    };
    assert_eq!(bots.len(), 4);
    assert_eq!(phase(&sim), Phase::Live);
    let mut teams: Vec<u8> = bots.iter().map(|b| sim.app.world().get::<Team>(*b).unwrap().0).collect();
    teams.sort();
    teams.dedup();
    assert_eq!(teams.len(), 4, "a team each: {teams:?}");
    let in_hand: Vec<Option<&str>> = bots.iter().map(|b| held(&sim, *b)).collect();
    assert!(
        in_hand.iter().all(|h| [Some(cs_source::game::weapons::GLOCK), Some(cs_source::game::weapons::KNIFE)].contains(h)),
        "the first rung's Glock or the kit's knife: {in_hand:?}"
    );
    let mut kills = 0;
    for _ in 0..90 {
        sim.seconds(1.0);
        kills = bots.iter().map(|b| sim.app.world().get::<Score>(*b).map_or(0, |s| s.kills)).sum::<i32>();
        if kills >= 2 {
            break;
        }
    }
    assert!(kills >= 2, "the bots kill each other ({kills} kills in 90 s)");
    assert!(bots.iter().any(|b| rung(&sim, *b) > 0), "and climb");
}

/// A ladder gun never runs dry: shooting a rung's gun (the M60's 200
/// rounds, misses included) empties neither its clip nor its reserve for
/// good; the reserve is topped up, so the last rungs stay playable.
#[test]
fn a_rungs_gun_keeps_its_reserve() {
    use chimera_sim::weapon::Magazine;
    let mut sim = party();
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    sim.app.world_mut().entity_mut(a).insert(Rung(8));
    chimera_sim::weapon::give(sim.app.world_mut(), a, "combat_arms:m60");
    sim.seconds(2.0);
    assert_eq!(held(&sim, a), Some("combat_arms:m60"));
    let gun = testkit::active(sim.app.world(), a).unwrap();
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(gun).unwrap();
        m.clip = 0;
        m.reserve = 0;
    }
    sim.seconds(0.5);
    let m = sim.app.world().get::<Magazine>(gun).unwrap();
    assert_eq!(m.reserve, m.reserve_max, "the reserve is full again: {m:?}");
    assert!(m.reserve > 0);
}

/// A soldier holding CS:S's gun has its view model driven by Chimera's
/// generic driver (CS:S's rules, its entry in the party's
/// `GameViewAnimRules`); Combat Arms' lets go of it, and takes it back
/// with a Combat Arms gun.
#[test]
fn a_cs_source_guns_view_model_is_driven_by_chimera_and_ours_by_combat_arms() {
    use chimera_sim::weapon::view_anim::{OwnViewAnim, ViewModelPlay};
    let mut sim = party();
    let rules = sim.app.world().resource::<chimera_sim::weapon::view_anim::GameViewAnimRules>();
    assert!(rules.for_weapon(cs_source::game::weapons::GLOCK).is_some());
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    assert_eq!(held(&sim, a), Some(cs_source::game::weapons::GLOCK));
    sim.seconds(0.5);
    let w = sim.app.world();
    assert!(w.get::<OwnViewAnim>(a).is_none(), "CS:S's gun: not Combat Arms' driver's");
    assert!(w.get::<ViewModelPlay>(a).is_some(), "Chimera's driver has it");
    // The ladder's second rung: Combat Arms' Beretta.
    sim.app.world_mut().entity_mut(a).insert(Rung(1));
    chimera_sim::weapon::give(sim.app.world_mut(), a, "combat_arms:beretta_m92fs");
    sim.seconds(1.0);
    assert_eq!(held(&sim, a), Some("combat_arms:beretta_m92fs"));
    assert!(sim.app.world().get::<OwnViewAnim>(a).is_some(), "ours again");
}
