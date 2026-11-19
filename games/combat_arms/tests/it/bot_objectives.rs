//! Bots and the mode objectives (specs/combat_arms/bots_objectives.md):
//! no AI plays an objective (its Quirk 1). Our bots, which play every
//! mode, fight but never take, carry or use an objective (a flag, a bomb,
//! a seize trigger, intel, bullion, the VIP's role), no objective event
//! reaches their senses, and no mode's rules create AI but Hired Guns'
//! mercenaries (whose cases 8-24 are in `hired_guns`; 14's re-plan rate
//! here). Cases 1-4 and 7 are about the mission files and the goal data
//! (not read yet: board item 21).

use bevy::prelude::*;
use chimera_world::harness::Sim;
use combat_arms::game::{
    bots::{
        self, BotSettings, BotState, CaBot,
        brain::{Kind, Stimuli},
    },
    rules::{
        escort::CaVip,
        mercs::{self, CaMerc, MercOrder},
        objectives::{BOMB, Bullion, CaObjectives, FlagAt},
    },
};

use crate::{hired_guns, mode_kit::*};

fn bot(sim: &mut Sim, team: u8) -> Entity {
    bots::add_bot(sim.app.world_mut(), Some(team)).unwrap()
}

/// 5 (and 4): an objective mode's rules create no AI; a room on any map
/// in an objective mode plays with the humans it has.
#[test]
fn case_05_no_ai_in_objective_modes() {
    for key in ["TMM", "CTF", "SBT", "HQM", "BNT", "ESC", "SQM", "CMM", "CCTF"] {
        let mut sim = game(key, 4);
        join(&mut sim, JSF);
        join(&mut sim, CREED);
        until_live(&mut sim);
        sim.ticks(1000);
        let w = sim.app.world_mut();
        assert_eq!(w.query::<&CaBot>().iter(w).count(), 0, "{key}: no bot");
        assert_eq!(w.query::<&CaMerc>().iter(w).count(), 0, "{key}: no AI");
    }
}

/// 6: a bot near a bomb site while a human plants there: no stimulus is
/// posted by the plant, the bot keeps patrolling.
#[test]
fn case_06_bot_ignores_a_plant() {
    let mut sim = game("TMM", 6);
    let site = Vec3::new(0.0, 1.0, 0.0);
    objects(&mut sim).bomb_zones = vec![bomb_zone("Doobic", 0, CREED, site)];
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 1;
    let planter = join(&mut sim, JSF);
    let b = bot(&mut sim, CREED);
    past_warmup(&mut sim);
    unprotect_all(&mut sim);
    sim.ticks(3);
    // The bot 300 u off, facing away (yaw 0 looks down −Z; the site is at
    // +Z from it).
    let at = site - Vec3::new(0.0, 0.0, u(300.0));
    put(&mut sim, b, at);
    sim.intent(b).yaw = 0.0;
    select_bomb(&mut sim, planter);
    assert!(wait_for(&mut sim, 1.0, |s| in_hand(s, planter) == Some(BOMB)));
    sim.intent(planter).fire = true;
    let mut gunfire = false;
    let end = now(&sim) + 4.6;
    while now(&sim) < end {
        put(&mut sim, planter, site);
        put(&mut sim, b, at);
        sim.ticks(1);
        let s = sim.app.world().resource::<Stimuli>();
        gunfire |= s.list.iter().any(|x| x.kind == Kind::Gunfire && x.source == planter);
    }
    assert!(objectives(&sim).planted.is_some(), "planted");
    assert!(!gunfire, "a plant makes no gunfire stimulus");
    let state = sim.app.world().get::<CaBot>(b).unwrap().state;
    assert_eq!(state, BotState::Patrol, "the bot keeps patrolling");
}

/// 2.1-2.7: bots take nothing. An Alpha bot in S&D gets no bomb; a bot
/// on the enemy flag leaves it home; a bot on a bag of bullion leaves it;
/// a bot is never the VIP.
#[test]
fn bots_take_no_objective() {
    // S&D: the bomb goes to the human attacker only.
    let mut sim = game("TMM", 6);
    let human = join(&mut sim, JSF);
    let jsf_bot = bot(&mut sim, JSF);
    bot(&mut sim, CREED);
    past_warmup(&mut sim);
    sim.ticks(3);
    assert!(weapons(&sim, human).contains(&BOMB));
    assert!(!weapons(&sim, jsf_bot).contains(&BOMB), "no bomb for a bot");

    // Capture the flag: a bot standing on the enemy flag.
    let mut sim = game("CTF", 6);
    let home = Vec3::new(0.0, 1.0, 25.0);
    let (flag, zone) = flag_and_zone(CREED, home);
    let (flag_a, zone_a) = flag_and_zone(JSF, Vec3::new(0.0, 1.0, 30.0));
    {
        let mut o = objects(&mut sim);
        o.flags = vec![flag_a, flag];
        o.flag_zones = vec![zone_a, zone];
    }
    sim.app.world_mut().resource_mut::<BotSettings>().stop = 1;
    join(&mut sim, CREED);
    let b = bot(&mut sim, JSF);
    until_live(&mut sim);
    hold_at(&mut sim, b, home, 0.5);
    let o = objectives(&sim);
    assert_eq!(o.flag(CREED).unwrap().at, FlagAt::Home, "the flag stays home");
    assert!(carry(&sim, b).flag.is_none());

    // Hired Guns: a bot on a bag.
    let mut sim = hired_guns::hg_game(3);
    join(&mut sim, JSF);
    let b = bot(&mut sim, CREED);
    past_warmup(&mut sim);
    let at = Vec3::new(10.0, 0.0, 28.0);
    sim.app.world_mut().resource_mut::<CaObjectives>().bullion = vec![Bullion {
        id: 1,
        at,
        bars: 4,
        money: 400,
        bag: true,
    }];
    hold_at(&mut sim, b, at + Vec3::Y * 0.9, 0.5);
    assert_eq!(objectives(&sim).bullion.len(), 1, "the bag stays");
    assert_eq!(carry(&sim, b).bars, 0);

    // VIP Escort: the VIP is the human escort player.
    let mut sim = game("ESC", 6);
    let human = join(&mut sim, JSF);
    bot(&mut sim, JSF);
    bot(&mut sim, CREED);
    past_warmup(&mut sim);
    let w = sim.app.world_mut();
    let vips: Vec<Entity> = w.query_filtered::<Entity, With<CaVip>>().iter(w).collect();
    assert_eq!(vips, vec![human]);
}

/// 14: a following unit's path to its running leader is planned again at
/// least once a second.
#[test]
fn case_14_replan_rate() {
    let (mut sim, a, _b) = hired_guns::hg(1, 1);
    let merc = mercs::hire(sim.app.world_mut(), a[0], 0).unwrap();
    mercs::set_order(sim.app.world_mut(), a[0], MercOrder::FollowMe);
    let centre = Vec3::new(20.0, 1.0, 22.0);
    let mut goals: Vec<(f64, Vec3)> = Vec::new();
    let start = now(&sim);
    while now(&sim) < start + 6.0 {
        // 6 m/s round a 10 m circle: faster than the unit runs.
        let t = (now(&sim) - start) as f32 * 0.6;
        let at = centre + Vec3::new(t.cos(), 0.0, t.sin()) * 10.0;
        put(&mut sim, a[0], at);
        sim.ticks(1);
        if let Some(g) = sim.app.world().get::<CaBot>(merc).unwrap().route_goal
            && goals.last().is_none_or(|(_, last)| *last != g)
        {
            goals.push((now(&sim), g));
        }
    }
    let gaps: Vec<f64> = goals.windows(2).map(|w| w[1].0 - w[0].0).collect();
    assert!(goals.len() >= 4, "planned again: {goals:?}");
    // While following (not holding at the leader), within a second and a
    // frame.
    assert!(gaps.iter().all(|g| *g <= 1.06), "gaps {gaps:?}");
}
