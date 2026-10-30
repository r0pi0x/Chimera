//! Minigames' contract (`chimera_sim::minigame`) on the test kit: a
//! definition as data, checked against what the app registered; started,
//! it takes the gameplay's rules, makes free-for-all teams, waits for its
//! minimum of players, spawns them protected with the rules' weapons; the
//! `chimera:ladder` rules move a killer up a rung (its weapon swapped),
//! a melee death down, and a kill on the last rung wins the match, which
//! then ends; at the time limit the highest rung wins.

use bevy::prelude::*;
use chimera_core::{
    core::{Damage, DamageKind, Dead, FreezeTime, God, Hitgroup, Intent, Score, SimClock, Team},
    gameplay::Gameplay,
};
use chimera_sim::{
    minigame::{self, InMinigame, MatchResults, MINIGAME, MinigameAppExt, MinigamePlugin, Protected, ladder::Rung, wins::{MatchWins, MatchesWon}},
    mode::{FFA_FIRST, Match, Phase},
    movement::placeholder,
    testkit::{self, PISTOL, RIFLE, SHOTGUN},
};
use chimera_world::{greybox, harness::Sim};

const DEF: &str = r#"
id = "kit:gun_game"
name = "Kit gun game"
players = { min = 2, max = 4 }
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "60s"

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
demote_on = "melee_death"
"#;

fn sim_with(def: &str) -> Sim {
    Sim::with(chimera_sim::SimBasePlugins, |app| {
        app.add_plugins((greybox::GreyboxMapPlugin, MinigamePlugin));
        testkit::add_weapons(app);
        app.add_minigames(&[("kit_gun_game.toml", def)]).expect("the definition reads");
    })
}

fn started(def: &str) -> Sim {
    let mut sim = sim_with(def);
    minigame::start(sim.app.world_mut(), "kit:gun_game").expect("it starts");
    sim
}

fn player(sim: &mut Sim) -> Entity {
    sim.spawn_character(greybox::SPAWNS[0], placeholder::ID)
}

fn phase(sim: &Sim) -> Phase {
    sim.app.world().resource::<Match>().phase
}

fn rung(sim: &Sim, p: Entity) -> usize {
    sim.app.world().get::<Rung>(p).map_or(0, |r| r.0)
}

/// `by` kills `p` with a hit of `kind`.
fn kill(sim: &mut Sim, p: Entity, by: Entity, kind: DamageKind) {
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: Some(by),
        amount: 5.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
}

/// Wait out a respawn's delay and protection (and the drawing).
fn settle(sim: &mut Sim) {
    sim.seconds(1.7);
}

#[test]
fn a_definition_is_checked_against_the_app() {
    let mut sim = sim_with(DEF);
    // A weapon the catalogue lacks, a set the definition doesn't allow, a
    // module nobody registered, teams the module doesn't play.
    for (from, to, says) in [
        ("\"kit:shotgun\"]", "\"kit:nothing\"]", "not in the weapon catalogue"),
        ("time_limit = \"60s\"", "time_limit = \"60s\"\n[slots]\nweapons = [\"other\"]", "not from the sets"),
        ("module = \"chimera:ladder\"", "module = \"kit:unknown\"", "no rules module"),
        ("teams = \"free_for_all\"", "teams = \"two\"", "teams"),
        ("demote_on = \"melee_death\"", "demote_on = \"sometimes\"", "unknown variant"),
    ] {
        let text = DEF.replace(from, to).replace("kit:gun_game", "kit:bad");
        sim.app.add_minigames(&[("bad.toml", &text)]).unwrap();
        let err = minigame::start(sim.app.world_mut(), "kit:bad").map(|_| ()).unwrap_err();
        assert!(err.contains(says), "{from} -> {to}: {err}");
    }
    assert!(minigame::start(sim.app.world_mut(), "kit:nope").is_err());
    let def = minigame::start(sim.app.world_mut(), "kit:gun_game").expect("the good one starts");
    assert_eq!(def.name, "Kit gun game");
    assert_eq!(sim.app.world().resource::<Gameplay>().rules, MINIGAME, "the minigame is the gameplay's rules");
}

#[test]
fn players_join_on_their_own_teams_and_the_match_waits_for_the_minimum() {
    let mut sim = started(DEF);
    let a = player(&mut sim);
    sim.seconds(0.5);
    assert!(sim.app.world().get::<InMinigame>(a).is_some(), "taken in");
    assert_eq!(phase(&sim), Phase::Waiting, "one player of two");
    assert_eq!(testkit::active_id(sim.app.world(), a), Some(PISTOL), "the first rung's weapon while waiting");
    let b = player(&mut sim);
    sim.ticks(3);
    assert_eq!(phase(&sim), Phase::Live, "two in: live at once");
    let teams: Vec<u8> = [a, b].iter().map(|p| sim.app.world().get::<Team>(*p).unwrap().0).collect();
    assert_eq!(teams, [FFA_FIRST, FFA_FIRST + 1], "a team each");
    let m = sim.app.world().resource::<Match>().clone();
    let now = sim.app.world().resource::<SimClock>().now;
    assert!((m.round_ends.unwrap() - now - 60.0).abs() < 0.1, "the time limit's timer");
    // Spawned at different points, protected for a while.
    assert!(sim.position(a).distance(sim.position(b)) > 1.0);
    assert!(sim.app.world().get::<Protected>(b).is_some() && sim.app.world().get::<God>(b).is_some());
    sim.seconds(0.6);
    assert!(sim.app.world().get::<God>(b).is_none(), "protection over");
}

#[test]
fn kills_climb_the_ladder_melee_deaths_demote_and_the_last_rung_wins() {
    let mut sim = started(DEF);
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    assert_eq!(phase(&sim), Phase::Live);

    // A kill: a rung up, the rifle in hand at once; the victim back
    // after the delay, still on its rung.
    kill(&mut sim, b, a, DamageKind::Bullet);
    assert_eq!(rung(&sim, a), 1);
    assert!(sim.app.world().get::<Dead>(b).is_some());
    sim.seconds(0.5);
    assert_eq!(testkit::active_id(sim.app.world(), a), Some(RIFLE), "swapped");
    let carried = sim.app.world().get::<chimera_sim::weapon::Inventory>(a).unwrap().weapons.len();
    assert_eq!(carried, 1, "the old rung's weapon taken");
    sim.seconds(0.6);
    assert!(sim.app.world().get::<Dead>(b).is_none(), "respawned after 1 s");
    assert_eq!(sim.app.world().get::<Score>(a).unwrap().kills, 1);
    assert_eq!(sim.app.world().get::<Score>(b).unwrap().deaths, 1);

    // b climbs, then a knifes it: b down a rung (and a up).
    settle(&mut sim);
    kill(&mut sim, a, b, DamageKind::Bullet);
    assert_eq!(rung(&sim, b), 1);
    sim.seconds(1.2);
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Melee);
    assert_eq!((rung(&sim, a), rung(&sim, b)), (2, 0), "a melee death demotes, the killer still climbs");
    settle(&mut sim);
    assert_eq!(testkit::active_id(sim.app.world(), b), Some(PISTOL), "the lower rung's weapon at respawn");
    assert_eq!(testkit::active_id(sim.app.world(), a), Some(SHOTGUN));

    // A kill on the last rung wins the match.
    kill(&mut sim, b, a, DamageKind::Bullet);
    let team_a = sim.app.world().get::<Team>(a).unwrap().0;
    assert!(matches!(phase(&sim), Phase::RoundOver { .. }), "{:?}", phase(&sim));
    assert_eq!(sim.app.world().resource::<Match>().winner, Some(team_a));
    // The win pauses everyone through the match's end: a's forward, jump
    // and fire are ignored (its look kept), the freeze is on for bots and
    // a client's prediction.
    let at = sim.position(a);
    for _ in 0..60 {
        let mut i = sim.app.world_mut().get_mut::<Intent>(a).unwrap();
        i.move_axis = Vec2::Y;
        i.jump = true;
        i.fire = true;
        i.yaw = 1.0;
        sim.ticks(1);
    }
    assert!(sim.app.world().resource::<FreezeTime>().0, "frozen");
    let i = sim.app.world().get::<Intent>(a).unwrap().clone();
    assert!(i.move_axis == Vec2::ZERO && !i.jump && !i.fire && i.yaw == 1.0, "{i:?}");
    assert!(sim.position(a).distance(at) < 0.01, "a stands: {} -> {}", at, sim.position(a));
    sim.seconds(minigame::runtime::ROUND_OVER as f64 + minigame::runtime::INTERMISSION as f64 + 0.2);
    assert_eq!(phase(&sim), Phase::MatchOver);
    assert_eq!(sim.app.world().resource::<Match>().result, Some(Some(team_a)));
    let status = minigame::runtime::status(sim.app.world_mut()).unwrap();
    assert!(status.contains("MatchOver") && status.contains("rung 3/3"), "{status}");

    // The results: the winner first, each player's final rung, kills and
    // deaths, and the HUD's data carries them.
    let results = sim.app.world().resource::<MatchResults>().clone();
    assert_eq!(results.title, "Kit gun game");
    assert_eq!(results.rungs, 3);
    let rows: Vec<(Option<u32>, i32, u32, bool)> =
        results.rows.iter().map(|r| (r.rung, r.kills, r.deaths, r.winner)).collect();
    assert_eq!(rows, [(Some(3), 3, 1, true), (Some(1), 1, 3, false)], "{results:?}");
    assert_eq!(results.rows[0].player, Some(a));
    assert_eq!(results.winner().map(|w| w.player), Some(Some(a)));
    let lines = results.lines(|r| r.name.clone());
    assert!(lines[0].ends_with("wins the match"), "{lines:?}");
    assert!(lines[1].starts_with("1. ") && lines[1].contains("rung 3/3") && lines[1].contains("3 kills"), "{lines:?}");
    assert!(lines[2].starts_with("2. ") && lines[2].contains("rung 1/3"), "{lines:?}");
    assert_eq!(sim.app.world().resource::<chimera_sim::minigame::hud::MinigameHud>().results.as_ref(), Some(&results));
    // Each row's final gun and matches won (this one counted), and the
    // match-end hold from the match over: the next match's change
    // `RESULTS_HOLD` s on (no server here to pick a map).
    let guns: Vec<(Option<&str>, u32)> = results.rows.iter().map(|r| (r.gun.as_deref(), r.wins)).collect();
    assert_eq!(guns, [(Some(SHOTGUN), 1), (Some(PISTOL), 0)]);
    let next = sim.app.world().resource::<minigame::NextMatch>().clone();
    let now = sim.app.world().resource::<SimClock>().now;
    assert_eq!(next.map, None);
    assert!(next.at > now && next.at <= now + minigame::runtime::RESULTS_HOLD, "{next:?} at {now}");

    // `minigame_restart`: everyone again from the first rung, no results.
    minigame::restart(sim.app.world_mut());
    assert!(sim.app.world().get_resource::<MatchResults>().is_none());
    assert!(sim.app.world().get_resource::<minigame::NextMatch>().is_none(), "the hold over");
    assert!(!sim.app.world().resource::<FreezeTime>().0, "the freeze lifted");
    settle(&mut sim);
    assert_eq!(phase(&sim), Phase::Live);
    assert_eq!((rung(&sim, a), rung(&sim, b)), (0, 0));
}

#[test]
fn at_the_time_limit_the_highest_rung_wins() {
    let mut sim = started(&DEF.replace("\"60s\"", "\"3s\""));
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    kill(&mut sim, a, b, DamageKind::Bullet);
    sim.seconds(2.5);
    let team_b = sim.app.world().get::<Team>(b).unwrap().0;
    assert!(matches!(phase(&sim), Phase::RoundOver { .. } | Phase::Intermission { .. }), "{:?}", phase(&sim));
    assert_eq!(sim.app.world().resource::<Match>().winner, Some(team_b));
}

#[test]
fn the_hud_data_shows_the_rung_the_leader_and_the_events() {
    use chimera_core::core::LocalPlayer;
    use chimera_sim::minigame::hud::MinigameHud;
    let mut sim = started(DEF);
    let a = player(&mut sim);
    let b = player(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    settle(&mut sim);
    let text = |h: &MinigameHud| h.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>();
    let hud = sim.app.world().resource::<MinigameHud>().clone();
    assert_eq!(hud.title, "Kit gun game");
    assert_eq!(text(&hud)[0], "Rung 1/3: {weapon}");
    assert_eq!(hud.lines[0].weapon, Some(PISTOL));
    assert_eq!(text(&hud)[1], "Next: {weapon}");
    assert_eq!(hud.lines[1].weapon, Some(RIFLE));
    assert!(text(&hud).contains(&"Tied for the lead".to_string()), "{:?}", text(&hud));
    assert!(text(&hud).iter().any(|t| t.starts_with("Time ")), "{:?}", text(&hud));

    // A kill: a rung up and the lead, with a notice.
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(2);
    let hud = sim.app.world().resource::<MinigameHud>().clone();
    assert_eq!(text(&hud)[0], "Rung 2/3: {weapon}");
    assert!(text(&hud).contains(&"You lead".to_string()), "{:?}", text(&hud));
    assert_eq!(hud.notices.len(), 1, "{:?}", hud.notices);
    assert_eq!(hud.notices[0].0.text, "Rung up! 2/3: {weapon}");
    assert_eq!(hud.notices[0].0.weapon, Some(RIFLE));

    // The other player leads after two kills to a's one.
    settle(&mut sim);
    kill(&mut sim, a, b, DamageKind::Bullet);
    sim.seconds(1.2);
    settle(&mut sim);
    kill(&mut sim, a, b, DamageKind::Bullet);
    sim.ticks(2);
    let hud = sim.app.world().resource::<MinigameHud>().clone();
    let leader = hud.lines.iter().find(|l| l.text.starts_with("Leader:")).expect("b leads");
    assert_eq!(leader.player, Some(b));

    // Notices go after their seconds; none of it without a minigame.
    sim.seconds(chimera_sim::minigame::hud::NOTICE_SECONDS + 1.0);
    assert!(sim.app.world().resource::<MinigameHud>().notices.is_empty());
    sim.app.world_mut().remove_resource::<chimera_sim::minigame::ActiveMinigame>();
    sim.ticks(2);
    assert!(!sim.app.world().resource::<MinigameHud>().playing());
}

/// Each player's status lines are a component the ladder writes when they
/// change: the start (tied), a rung up (you lead, the other is told the
/// leader), the other's catching up (tied), the last rung, the minigame's end.
#[test]
fn each_players_status_lines_follow_the_ladder_in_a_component() {
    use chimera_sim::minigame::hud::MinigameStatus;
    let mut sim = started(DEF);
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    let lines = |sim: &Sim, p: Entity| -> Vec<String> {
        sim.app.world().get::<MinigameStatus>(p).map(|s| s.0.iter().map(|l| l.text.clone()).collect()).unwrap_or_default()
    };
    let tied = vec!["Rung 1/3: {weapon}", "Next: {weapon}", "Tied for the lead"];
    assert_eq!(lines(&sim, a), tied);
    assert_eq!(lines(&sim, b), tied);
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(2);
    assert_eq!(lines(&sim, a), vec!["Rung 2/3: {weapon}", "Next: {weapon}", "You lead"]);
    assert_eq!(lines(&sim, b), vec!["Rung 1/3: {weapon}", "Next: {weapon}", "Leader: {player}, rung 2/3"]);
    let leader = sim.app.world().get::<MinigameStatus>(b).unwrap().0[2].player;
    assert_eq!(leader, Some(a));
    settle(&mut sim);
    kill(&mut sim, a, b, DamageKind::Bullet);
    sim.ticks(2);
    assert_eq!(lines(&sim, a)[2], "Tied for the lead");
    assert_eq!(lines(&sim, b), vec!["Rung 2/3: {weapon}", "Next: {weapon}", "Tied for the lead"]);
    // The last rung says so; the status goes with the minigame.
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(2);
    assert_eq!(lines(&sim, a)[..2], ["Rung 3/3: {weapon}", "Last rung: a kill wins"]);
    sim.app.world_mut().remove_resource::<chimera_sim::minigame::ActiveMinigame>();
    sim.ticks(2);
    assert!(sim.app.world().get::<MinigameStatus>(a).is_none());
}

/// Loose weapons lying in the world.
fn loose(sim: &mut Sim) -> usize {
    let w = sim.app.world_mut();
    w.query::<&chimera_sim::weapon::drop::Loose>().iter(w).count()
}

#[test]
fn nobody_drops_a_gun_at_death_under_the_ladder() {
    let mut sim = started(DEF);
    sim.app.insert_resource(chimera_sim::weapon::drop::DeathDrops(true));
    let (a, b) = (player(&mut sim), player(&mut sim));
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    assert_eq!(loose(&mut sim), 0, "the ladder gives the guns; none lies around");
}

#[test]
fn the_same_death_outside_a_ladder_drops_the_gun() {
    let mut sim = testkit::kit_sim();
    sim.app.insert_resource(chimera_sim::weapon::drop::DeathDrops(true));
    let a = testkit::with_player(&mut sim, None, RIFLE);
    let b = testkit::with_player(&mut sim, Some(greybox::SPAWNS[1]), RIFLE);
    kill(&mut sim, b, a, DamageKind::Bullet);
    assert_eq!(loose(&mut sim), 1);
}

#[test]
fn a_won_match_counts_for_its_winner_and_the_count_survives_a_restart() {
    let mut sim = started(DEF);
    let a = player(&mut sim);
    let b = player(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(Name::new("Alpha"));
    sim.app.world_mut().entity_mut(b).insert(Name::new("Bravo"));
    let won = |sim: &Sim, p: Entity| sim.app.world().get::<MatchesWon>(p).map(|w| w.0);
    settle(&mut sim);
    assert_eq!((won(&sim, a), won(&sim, b)), (Some(0), Some(0)), "everyone starts at none");
    // `a` climbs to the last rung and wins; `b` is killed on the way.
    for _ in 0..3 {
        kill(&mut sim, b, a, DamageKind::Bullet);
        sim.seconds(1.7);
    }
    sim.seconds(minigame::runtime::ROUND_OVER as f64 + minigame::runtime::INTERMISSION as f64 + 0.2);
    assert!(sim.app.world().resource::<MatchResults>().winner().is_some_and(|w| w.name == "Alpha"));
    assert_eq!((won(&sim, a), won(&sim, b)), (Some(1), Some(0)), "the winner has one");
    assert_eq!(sim.app.world().resource::<MatchWins>().of("Alpha"), 1);
    // A new match (what a rotation, `change_match` and `ugg_new` all do):
    // the tally stays.
    minigame::restart(sim.app.world_mut());
    settle(&mut sim);
    assert_eq!((won(&sim, a), won(&sim, b)), (Some(1), Some(0)), "kept over the restart");
    // The second match is `b`'s: both have one, `a` is not counted twice.
    for _ in 0..3 {
        kill(&mut sim, a, b, DamageKind::Bullet);
        sim.seconds(1.7);
    }
    sim.seconds(minigame::runtime::ROUND_OVER as f64 + minigame::runtime::INTERMISSION as f64 + 0.2);
    assert_eq!((won(&sim, a), won(&sim, b)), (Some(1), Some(1)));
}

/// The shooter's own killing round sounds once, though the kill swaps the
/// gun for the next rung's in the same tick or the next: one fire sound
/// (the old gun's) per round fired, on the one that hosts the match.
#[test]
fn the_shooters_own_killing_shot_sounds_once() {
    use chimera_sim::weapon::{WeaponEvent, WeaponEventKind};
    use chimera_world::map::PlaySound;
    let def = DEF.replace(r#"["kit:pistol", "kit:rifle", "kit:shotgun"]"#, r#"["kit:rifle", "kit:pistol", "kit:shotgun"]"#);
    let mut sim = started(&def);
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    assert_eq!(testkit::active_id(sim.app.world(), a), Some(RIFLE));
    let at = sim.position(a) + Vec3::new(0.0, 0.0, -4.0);
    sim.app.world_mut().get_mut::<Transform>(b).unwrap().translation = at;
    sim.app.world_mut().get_mut::<avian3d::prelude::Position>(b).unwrap().0 = at;
    sim.seconds(0.5);
    let mut sounds = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let mut events = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let (mut rounds, mut heard, mut after) = (0, 0, 0);
    for _ in 0..600 {
        let target = sim.position(b);
        testkit::aim_at(&mut sim, a, target);
        sim.intent(a).fire = after == 0;
        sim.ticks(1);
        let w = sim.app.world();
        rounds += events
            .read(w.resource::<Messages<WeaponEvent>>())
            .filter(|e| e.owner == a && matches!(e.kind, WeaponEventKind::Fired { .. }))
            .count();
        heard += sounds.read(w.resource::<Messages<PlaySound>>()).filter(|s| s.entry == "kit.rifle_fire").count();
        if w.get::<Dead>(b).is_some() {
            after += 1;
            if after > 10 {
                break;
            }
        }
    }
    sim.intent(a).fire = false;
    assert_eq!(rung(&sim, a), 1, "the kill climbed a rung");
    assert_ne!(testkit::active_id(sim.app.world(), a), Some(RIFLE), "the gun was swapped");
    assert!(rounds >= 10, "the rifle fired its rounds: {rounds}");
    assert_eq!(heard, rounds, "one fire sound per round, the killing one too");
}

/// The dead local player is told when it comes back (`core::RespawnDue`):
/// its death plus the definition's delay, the respawn at that time, then
/// nothing; nothing while alive, and nothing once the match is over.
#[test]
fn the_dead_local_player_knows_when_it_respawns() {
    use chimera_core::core::{LocalPlayer, RespawnDue};
    let mut sim = started(DEF);
    let a = player(&mut sim);
    let b = player(&mut sim);
    sim.app.world_mut().entity_mut(b).insert(LocalPlayer);
    settle(&mut sim);
    let due = |sim: &Sim| sim.app.world().resource::<RespawnDue>().0;
    assert_eq!(due(&sim), None, "alive");
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(1);
    let since = sim.app.world().get::<Dead>(b).expect("dead").since;
    let d = due(&sim).expect("a timed respawn");
    assert!((d.at - (since + 1.0)).abs() < 1e-9 && d.delay == 1.0, "{d:?} {since}");
    // Back at that time, not before.
    while sim.app.world().resource::<SimClock>().now + 0.05 < d.at {
        sim.ticks(1);
        assert!(sim.app.world().get::<Dead>(b).is_some(), "not before the time it was told");
    }
    sim.seconds(0.1);
    assert!(sim.app.world().get::<Dead>(b).is_none(), "respawned");
    assert_eq!(due(&sim), None);
    // Match over: the dead stay dead and are told nothing.
    settle(&mut sim);
    sim.app.world_mut().resource_mut::<Match>().phase = Phase::MatchOver;
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(2);
    assert!(sim.app.world().get::<Dead>(b).is_some());
    assert_eq!(due(&sim), None, "no countdown after the match");
}

/// The callout the local player's HUD data holds: its lines' texts, the
/// first line's player and weapon, its cue.
fn callout(sim: &Sim) -> Option<(Vec<String>, Option<Entity>, Option<&'static str>, Option<&'static str>)> {
    let hud = sim.app.world().resource::<chimera_sim::minigame::hud::MinigameHud>();
    let c = &hud.callout.as_ref()?.callout;
    Some((c.lines.iter().map(|l| l.text.clone()).collect(), c.lines[0].player, c.lines[0].weapon, c.cue))
}

#[test]
fn the_viewer_gets_callouts_for_its_rungs_the_lead_others_last_rung_and_the_win() {
    use chimera_core::core::LocalPlayer;
    use chimera_sim::minigame::{hud::CALLOUT_SECONDS, ladder::Leaders};
    let mut sim = started(DEF);
    let a = player(&mut sim);
    let b = player(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    settle(&mut sim);
    assert_eq!(callout(&sim), None, "nothing yet");

    // a's kill: a level up (its new gun), and the lead.
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(2);
    let (lines, _, weapon, cue) = callout(&sim).expect("a callout");
    assert_eq!(lines[0], "Level up: {weapon}");
    assert_eq!(lines[1], "Level 2/3");
    assert_eq!((weapon, cue), (Some(RIFLE), Some("rung_up")));
    assert_eq!(sim.app.world().resource::<Leaders>().0, vec![a]);

    // b catches up: both tied.
    settle(&mut sim);
    kill(&mut sim, a, b, DamageKind::Bullet);
    sim.ticks(2);
    let (lines, _, _, cue) = callout(&sim).expect("a callout");
    assert_eq!((lines[0].as_str(), cue), ("Tied for the lead", Some("lead_tied")));
    let mut both = vec![a, b];
    both.sort();
    assert_eq!(sim.app.world().resource::<Leaders>().0, both);

    // Seen by b: a on the last rung, then a's win.
    sim.app.world_mut().entity_mut(a).remove::<LocalPlayer>();
    sim.app.world_mut().entity_mut(b).insert(LocalPlayer);
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(2);
    let (lines, who, _, cue) = callout(&sim).expect("a callout");
    assert_eq!(lines[0], "{player} is on the last level!");
    assert_eq!((who, cue), (Some(a), Some("last_rung_other")));
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(2);
    let (lines, who, _, cue) = callout(&sim).expect("a callout");
    assert_eq!((lines[0].as_str(), who, cue), ("{player} wins!", Some(a), Some("match_lost")));
    sim.seconds(CALLOUT_SECONDS + 0.5);
    assert_eq!(callout(&sim), None, "gone after its seconds");
}

#[test]
fn the_viewers_own_last_rung_says_so_and_a_frames_callouts_merge() {
    use chimera_core::core::LocalPlayer;
    use chimera_sim::minigame::hud::{Callout, HudLine, merge};
    let mut sim = started(DEF);
    let a = player(&mut sim);
    let b = player(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(2);
    let (lines, _, _, cue) = callout(&sim).expect("a callout");
    assert_eq!(lines[0], "Last level!", "{lines:?}");
    assert_eq!(lines[1], "{weapon}: a kill wins");
    assert_eq!(cue, Some("last_rung"));
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(2);
    let (lines, _, _, cue) = callout(&sim).expect("a callout");
    assert_eq!((lines[0].as_str(), cue), ("You win!", Some("match_won")));

    // The highest level first, the others' first lines after it.
    let up = Callout::new(2, "rung_up", vec![HudLine::new("Level up"), HudLine::new("Level 2/3")]);
    let lead = Callout::new(1, "lead_taken", vec![HudLine::new("You took the lead")]);
    let m = merge(vec![lead.clone(), up]).unwrap();
    assert_eq!(m.cue, Some("rung_up"));
    let texts: Vec<_> = m.lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["Level up", "Level 2/3", "You took the lead"]);
    assert_eq!(merge(vec![]), None);
}

#[test]
fn a_definitions_cue_pack_must_be_registered() {
    use chimera_core::cues::CueAppExt;
    let mut sim = sim_with(DEF);
    sim.app.register_cues("id = \"kit\"\nname = \"Kit\"\ngame = \"kit\"\n");
    let with = |cues: &str| DEF.replace("time_limit", &format!("cues = \"{cues}\"\ntime_limit"));
    for ok in ["kit", "none", "default"] {
        minigame::add(sim.app.world_mut(), "ok.toml", &with(ok)).unwrap_or_else(|e| panic!("{ok}: {e}"));
    }
    let err = minigame::add(sim.app.world_mut(), "bad.toml", &with("nobody")).unwrap_err();
    assert!(err.contains("no cue pack `nobody`"), "{err}");
}

/// `p`'s own death: a suicide (its own damage, `kill`'s), or a fall (no
/// killer).
fn own_death(sim: &mut Sim, p: Entity, fall: bool) {
    let at = sim.position(p);
    sim.app.world_mut().entity_mut(p).remove::<God>();
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: (!fall).then_some(p),
        amount: 1000.0,
        point: at,
        dir: Vec3::NEG_Y,
        hitgroup: Hitgroup::Generic,
        kind: if fall { DamageKind::Fall } else { DamageKind::Generic },
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(2);
}

#[test]
fn a_suicide_or_a_fall_costs_a_rung_but_never_below_the_first() {
    use chimera_core::core::LocalPlayer;
    let mut sim = started(DEF);
    let a = player(&mut sim);
    let b = player(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    assert_eq!(rung(&sim, a), 1);
    settle(&mut sim);

    // `kill` (its own damage): a rung down, said big and in the notices;
    // no kill for anyone, a kill taken from a (the runtime's count).
    own_death(&mut sim, a, false);
    assert_eq!(rung(&sim, a), 0, "the suicide cost a rung");
    let (lines, _, weapon, cue) = callout(&sim).expect("a callout");
    assert_eq!(lines, ["Suicide! Level down: {weapon}", "Level 1/3"]);
    assert_eq!((weapon, cue), (Some(PISTOL), Some("rung_down")));
    let hud = sim.app.world().resource::<chimera_sim::minigame::hud::MinigameHud>();
    assert!(hud.notices.iter().any(|(n, _)| n.text == "Suicide! Down to 1/3: {weapon}"), "{:?}", hud.notices);
    assert_eq!(sim.app.world().get::<Score>(a).unwrap().kills, 0);
    assert_eq!(sim.app.world().get::<Score>(b).unwrap().kills, 0, "nobody's kill");
    settle(&mut sim);
    assert_eq!(testkit::active_id(sim.app.world(), a), Some(PISTOL), "back with the lower rung's weapon");

    // On the first rung a fall costs nothing.
    own_death(&mut sim, a, true);
    assert_eq!(rung(&sim, a), 0);
    // A fall from the second rung: down.
    settle(&mut sim);
    kill(&mut sim, a, b, DamageKind::Bullet);
    assert_eq!(rung(&sim, b), 1);
    settle(&mut sim);
    own_death(&mut sim, b, true);
    assert_eq!(rung(&sim, b), 0, "a fall cost a rung");

    // Off: nothing.
    let mut sim = started(&DEF.replace("demote_on = \"melee_death\"", "demote_on = \"melee_death\"\ndemote_on_suicide = false"));
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    settle(&mut sim);
    own_death(&mut sim, a, false);
    assert_eq!(rung(&sim, a), 1, "demote_on_suicide = false");
}

#[test]
fn a_late_joiner_starts_on_the_lowest_rung_the_average_or_the_first() {
    let four = DEF.replace(
        "ladder = [\"kit:pistol\", \"kit:rifle\", \"kit:shotgun\"]",
        "ladder = [\"kit:pistol\", \"kit:rifle\", \"kit:shotgun\", \"kit:pistol\"]",
    );
    let joiner = |join: &str, rungs: (usize, usize)| {
        let def = four.replace("demote_on =", &format!("join_rung = \"{join}\"\ndemote_on ="));
        let mut sim = started(&def);
        let a = player(&mut sim);
        let b = player(&mut sim);
        settle(&mut sim);
        assert_eq!(phase(&sim), Phase::Live);
        sim.app.world_mut().entity_mut(a).insert(Rung(rungs.0));
        sim.app.world_mut().entity_mut(b).insert(Rung(rungs.1));
        let c = player(&mut sim);
        settle(&mut sim);
        (rung(&sim, c), testkit::active_id(sim.app.world(), c))
    };
    assert_eq!(joiner("lowest", (2, 1)), (1, Some(RIFLE)), "the others' lowest, its gun in hand");
    assert_eq!(joiner("lowest", (2, 0)), (0, Some(PISTOL)));
    assert_eq!(joiner("average", (2, 0)), (1, Some(RIFLE)), "the average, rounded down");
    assert_eq!(joiner("average", (3, 2)), (2, Some(SHOTGUN)));
    assert_eq!(joiner("first", (2, 2)), (0, Some(PISTOL)));
    assert_eq!(joiner("lowest", (3, 3)).0, 2, "never the last rung");

    // Joining while the match waits: the first rung (the start puts
    // everyone there).
    let mut sim = started(&four);
    let a = player(&mut sim);
    sim.ticks(3);
    assert_eq!(phase(&sim), Phase::Waiting);
    assert_eq!(rung(&sim, a), 0);
}

const KNIFE: &str = r#"
id = "kit:knife"
base = "kit:base"
slot = 2
draw = 0.5

[melee.primary]
range = 2.0
hull = 0.5
facing_cos = 0.7
damage = 50.0
hit_refire = 0.5
miss_refire = 0.4
miss_refire_other = 0.5
force = 0.0

[melee.secondary]
range = 1.5
hull = 0.5
facing_cos = 0.7
damage = 65.0
hit_refire = 1.1
miss_refire = 1.0
miss_refire_other = 1.0
force = 0.0
"#;

fn knife_sim(def: &str) -> Sim {
    use chimera_sim::weapon::data::WeaponDataAppExt;
    Sim::with(chimera_sim::SimBasePlugins, |app| {
        app.add_plugins((greybox::GreyboxMapPlugin, MinigamePlugin));
        testkit::add_weapons(app);
        app.add_weapon_data(&[("knife.toml", KNIFE)]).expect("the knife loads");
        app.add_minigames(&[("kit_gun_game.toml", def)]).expect("the definition reads");
    })
}

/// The weapons `p` carries.
fn carried(sim: &Sim, p: Entity) -> Vec<&'static str> {
    let w = sim.app.world();
    let inv = w.get::<chimera_sim::weapon::Inventory>(p).unwrap();
    inv.weapons.iter().filter_map(|e| w.get::<chimera_sim::weapon::Weapon>(*e).map(|x| x.id)).collect()
}

#[test]
fn a_knife_finale_is_a_knife_alone_and_its_kill_wins() {
    use chimera_core::core::LocalPlayer;
    let finale = DEF
        .replace("points = \"map\"", "points = \"map\"\nkit = [\"kit:knife\"]")
        .replace("demote_on =", "knife_finale = true\ndemote_on =");
    let mut sim = knife_sim(&finale);
    minigame::start(sim.app.world_mut(), "kit:gun_game").expect("it starts");
    let a = player(&mut sim);
    let b = player(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    settle(&mut sim);
    let ladder = sim.app.world().resource::<minigame::ladder::Ladder>().clone();
    assert_eq!(ladder.rungs, [PISTOL, RIFLE, SHOTGUN, "kit:knife"], "a rung after the guns");
    assert_eq!(carried(&sim, a), ["kit:knife", PISTOL], "the kit and the gun below it");

    for _ in 0..3 {
        kill(&mut sim, b, a, DamageKind::Bullet);
        settle(&mut sim);
    }
    assert_eq!(rung(&sim, a), 3);
    assert_eq!(carried(&sim, a), ["kit:knife"], "the knife alone");
    assert_eq!(testkit::active_id(sim.app.world(), a), Some("kit:knife"));
    // Back from a death on it: still the knife alone.
    kill(&mut sim, a, b, DamageKind::Bullet);
    settle(&mut sim);
    assert_eq!(carried(&sim, a), ["kit:knife"]);
    assert_eq!(phase(&sim), Phase::Live);
    kill(&mut sim, b, a, DamageKind::Melee);
    let team_a = sim.app.world().get::<Team>(a).unwrap().0;
    assert!(matches!(phase(&sim), Phase::RoundOver { .. }), "{:?}", phase(&sim));
    assert_eq!(sim.app.world().resource::<Match>().winner, Some(team_a));

    // Checked: a gun named as a knife; no melee weapon at all in the games: no finale.
    let mut sim = knife_sim(DEF);
    for (finale, says) in [("\"kit:rifle\"", "not a melee weapon")] {
        let text = DEF.replace("demote_on =", &format!("knife_finale = {finale}\ndemote_on =")).replace("kit:gun_game", "kit:bad");
        sim.app.add_minigames(&[("bad.toml", &text)]).unwrap();
        let err = minigame::start(sim.app.world_mut(), "kit:bad").map(|_| ()).unwrap_err();
        assert!(err.contains(says), "{finale}: {err}");
    }
    let bare = DEF.replace("demote_on =", "knife_finale = true\ndemote_on =").replace("kit:gun_game", "kit:bare");
    let mut none = sim_with(&bare);
    minigame::start(none.app.world_mut(), "kit:bare").expect("starts, without a finale");
    assert!(!none.app.world().resource::<minigame::ladder::Ladder>().knife_finale);
    let named = DEF.replace("demote_on =", "knife_finale = \"kit:knife\"\ndemote_on =").replace("kit:gun_game", "kit:named");
    sim.app.add_minigames(&[("named.toml", &named)]).unwrap();
    minigame::start(sim.app.world_mut(), "kit:named").expect("a named knife, no kit");
}

/// Spawn protection's two windows (`protection = { damage, bullets }`,
/// Combat Arms' S6 in shape): `core::SpawnProtected` with both, `God`
/// for the damage window; after the bullets window the damage is still
/// blocked; both over, nothing is left. The first shot ends both.
#[test]
fn spawn_protection_has_a_damage_and_a_bullets_window_and_the_first_shot_ends_it() {
    use chimera_core::core::SpawnProtected;
    let def = DEF.replace("protection = \"0.5s\"", "protection = { damage = \"1s\", bullets = \"0.5s\" }");
    let mut sim = started(&def);
    let a = player(&mut sim);
    let b = player(&mut sim);
    sim.ticks(3);
    let now = sim.app.world().resource::<SimClock>().now;
    let p = *sim.app.world().get::<SpawnProtected>(b).expect("protected");
    assert!((p.damage_until - now - 1.0).abs() < 0.1 && (p.bullets_until - now - 0.5).abs() < 0.1, "{p:?}");
    assert!(p.passes_attacks(now) && p.hides_hits(now));
    sim.seconds(0.6);
    let w = sim.app.world();
    assert!(w.get::<God>(b).is_some(), "damage still blocked after the bullets window");
    let now = w.resource::<SimClock>().now;
    assert!(!w.get::<SpawnProtected>(b).unwrap().passes_attacks(now) && w.get::<SpawnProtected>(b).unwrap().hides_hits(now));
    sim.seconds(0.5);
    let w = sim.app.world();
    assert!(w.get::<God>(b).is_none() && w.get::<SpawnProtected>(b).is_none() && w.get::<Protected>(b).is_none(), "over");
    // A's protection is cut short by its first shot.
    kill(&mut sim, a, b, DamageKind::Bullet);
    sim.seconds(1.2);
    assert!(sim.app.world().get::<SpawnProtected>(a).is_some(), "respawned protected");
    sim.intent(a).fire = true;
    sim.ticks(2);
    let w = sim.app.world();
    assert!(w.get::<SpawnProtected>(a).is_none() && w.get::<God>(a).is_none(), "the first shot ends it");
}

/// Any attack ends spawn protection (`fire_ends`), not only the fire key:
/// a knife's secondary stab (`end_protection` hears its swing).
#[test]
fn a_knife_stab_ends_spawn_protection_too() {
    use chimera_core::core::SpawnProtected;
    let def = DEF
        .replace("points = \"map\"", "points = \"map\"\nkit = [\"kit:knife\"]")
        .replace("protection = \"0.5s\"", "protection = \"6s\"");
    let mut sim = knife_sim(&def);
    minigame::start(sim.app.world_mut(), "kit:gun_game").expect("it starts");
    let a = player(&mut sim);
    let _b = player(&mut sim);
    sim.seconds(1.0);
    let knife = {
        let w = sim.app.world();
        let inv = w.get::<chimera_sim::weapon::Inventory>(a).unwrap();
        inv.weapons.iter().copied().find(|e| w.get::<chimera_sim::weapon::Weapon>(*e).unwrap().id == "kit:knife").unwrap()
    };
    sim.app.world_mut().get_mut::<chimera_sim::weapon::Inventory>(a).unwrap().wanted = Some(knife);
    sim.seconds(1.0);
    assert_eq!(testkit::active_id(sim.app.world(), a), Some("kit:knife"));
    assert!(sim.app.world().get::<SpawnProtected>(a).is_some(), "still protected");
    sim.intent(a).secondary = true;
    sim.ticks(3);
    sim.intent(a).secondary = false;
    let w = sim.app.world();
    assert!(w.get::<SpawnProtected>(a).is_none() && w.get::<God>(a).is_none(), "the stab ends it");
}

/// The match's moments for a music director (`moments::MinigameMoments`):
/// the phase, the local player alive, the last rung's players, the
/// respawn countdown, the result; the music by the definition (`none`).
#[test]
fn the_moments_follow_the_match_for_the_local_player() {
    use chimera_core::core::LocalPlayer;
    use chimera_sim::minigame::moments::{MinigameMoments, MomentPhase};
    let mut sim = started(DEF);
    let a = player(&mut sim);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    sim.ticks(2);
    let m = |sim: &Sim| sim.app.world().resource::<MinigameMoments>().clone();
    assert!(m(&sim).playing && m(&sim).phase == MomentPhase::Waiting && m(&sim).alive);
    assert_eq!(m(&sim).music, None, "no game here lends a music director");
    let b = player(&mut sim);
    settle(&mut sim);
    assert_eq!(m(&sim).phase, MomentPhase::Live);
    // a climbs to the last rung.
    kill(&mut sim, b, a, DamageKind::Bullet);
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    assert_eq!(m(&sim).last_rung, 1, "a on the last rung");
    // a dies: the countdown's whole seconds.
    kill(&mut sim, a, b, DamageKind::Bullet);
    sim.ticks(2);
    assert!(!m(&sim).alive);
    assert_eq!(m(&sim).respawn_in, Some(1), "1 s delay");
    settle(&mut sim);
    assert_eq!(m(&sim).respawn_in, None);
    // a wins on the last rung.
    kill(&mut sim, b, a, DamageKind::Bullet);
    sim.ticks(3);
    assert_eq!(m(&sim).phase, MomentPhase::Over);
    assert_eq!(m(&sim).result, None, "the win's own moment: not decided yet");
    // The round's end decides the match (the match-end screen's).
    sim.seconds(5.5);
    assert_eq!(m(&sim).result, Some(Some(true)), "the local player won");
}

/// The ladder's tick (`refill`, `follow_lead`) is two systems gated by
/// its mode (`mode::mode_is`), chained where the runtime ran the mode's
/// update: after `join`, before `step_phases`. With `refill_ammo` a rung
/// gun's spent reserve is back by the next tick; without, it stays.
#[test]
fn the_ladders_tick_refills_each_tick_after_the_join_and_before_the_step() {
    let mut sim = sim_with(DEF);
    let order = chimera_world::harness::schedule_systems(sim.app.world_mut(), FixedUpdate);
    let at = |name: &str| {
        order
            .iter()
            .position(|s| s.ends_with(name))
            .unwrap_or_else(|| panic!("{name} not in {order:?}"))
    };
    assert!(at("runtime::join") < at("ladder::refill"));
    assert!(at("ladder::refill") < at("ladder::follow_lead"));
    assert!(at("ladder::follow_lead") < at("runtime::step_phases"));

    let reserve = |sim: &Sim, p: Entity| {
        let w = sim.app.world().get::<chimera_sim::weapon::Inventory>(p)?.active?;
        Some(sim.app.world().get::<chimera_sim::weapon::Magazine>(w)?.reserve)
    };
    let spend = |sim: &mut Sim, p: Entity| {
        let w = sim.app.world().get::<chimera_sim::weapon::Inventory>(p).and_then(|i| i.active).expect("a gun");
        sim.app.world_mut().get_mut::<chimera_sim::weapon::Magazine>(w).expect("a magazine").reserve = 0;
    };
    for (refills, def) in [(false, DEF.to_string()), (true, DEF.replace("demote_on", "refill_ammo = true\ndemote_on"))] {
        let mut sim = started(&def);
        let a = player(&mut sim);
        let _b = player(&mut sim);
        settle(&mut sim);
        spend(&mut sim, a);
        sim.ticks(1);
        let left = reserve(&sim, a).expect("a reserve");
        assert_eq!(left > 0, refills, "refill_ammo {refills}: {left} left");
    }
}

/// `a` hit by `b` for `amount`, a bullet's.
fn hit(sim: &mut Sim, a: Entity, b: Entity, amount: f32) {
    let at = sim.position(a);
    sim.app.world_mut().write_message(Damage {
        target: a,
        attacker: Some(b),
        amount,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
}

fn health(sim: &Sim, e: Entity) -> f32 {
    sim.app.world().get::<chimera_core::core::Health>(e).unwrap().current
}

/// The runtime's per-tick order (ecs-cleanup step 6, order flag O1):
/// `end_protection` ends the protection on the protection's last tick
/// when the player fires, before that same tick's damage, which lands.
/// Without the shot the protection blocks it.
#[test]
fn a_shot_on_the_protections_last_tick_ends_it_before_that_ticks_damage() {
    use chimera_core::core::SpawnProtected;
    let mut sim = started(DEF);
    let a = player(&mut sim);
    let b = player(&mut sim);
    sim.ticks(3);
    let before = health(&sim, a);
    hit(&mut sim, a, b, 1.0);
    sim.ticks(1);
    assert_eq!(health(&sim, a), before, "protected: no damage");
    // On to the protection's last tick: the next one, after which its
    // windows are over.
    loop {
        let w = sim.app.world();
        let c = w.resource::<SimClock>();
        let dt = c.delta.as_secs_f64();
        let until = w.get::<SpawnProtected>(a).expect("still protected").damage_until;
        assert!(c.now + dt + 1e-9 < until, "the next tick still protected");
        if c.now + 2.0 * dt + 1e-9 >= until {
            break;
        }
        sim.ticks(1);
    }
    sim.intent(a).fire = true;
    hit(&mut sim, a, b, 1.0);
    sim.ticks(1);
    let w = sim.app.world();
    assert!(w.get::<SpawnProtected>(a).is_none() && w.get::<God>(a).is_none(), "the shot ended it");
    assert!(health(&sim, a) < before, "the same tick's hit landed");
}

/// A joiner is taken in, spawned and protected in the tick it appears
/// (`join`'s world part applies before the phases step).
#[test]
fn a_joiner_is_taken_in_and_spawned_in_its_first_tick() {
    let mut sim = started(DEF);
    let a = player(&mut sim);
    sim.ticks(1);
    let w = sim.app.world();
    assert!(w.get::<InMinigame>(a).is_some() && w.get::<Protected>(a).is_some(), "in and protected");
    assert!(!w.get::<chimera_sim::weapon::Inventory>(a).unwrap().weapons.is_empty(), "armed");
}

/// The dead come back on the first tick at or after their death plus the
/// delay (`respawn_dead`), not a tick later.
#[test]
fn the_dead_respawn_on_the_tick_they_are_due() {
    let mut sim = started(DEF);
    let a = player(&mut sim);
    let b = player(&mut sim);
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    let since = sim.app.world().get::<Dead>(b).expect("dead").since;
    while sim.app.world().get::<Dead>(b).is_some() {
        sim.ticks(1);
    }
    let c = sim.app.world().resource::<SimClock>();
    let (now, dt) = (c.now, c.delta.as_secs_f64());
    assert!(now + 1e-9 >= since + 1.0 && now - dt + 1e-9 < since + 1.0, "back at {now}, due {}", since + 1.0);
}

/// What the playing mode's `ModeDeath` observer saw: the victim, the
/// killer's kills and rung then, the event's weapon.
#[derive(Resource, Default)]
struct DeathsSeen(Vec<(Entity, i32, usize, Option<&'static str>)>);

/// The mode's reaction to a death (`mode::ModeDeath`, plan ecs-cleanup
/// step 5) runs where the mode's `died` hook ran: in the death step,
/// after that death was counted and before the next one is, death by
/// death; it carries the weapon of the kill though the counting's rung up
/// swapped the killer's weapon before it ran.
#[test]
fn a_modes_death_reaction_runs_after_each_deaths_counting_in_order_with_the_kills_weapon() {
    use chimera_sim::mode::ModeDeath;
    let mut sim = started(DEF);
    sim.app.init_resource::<DeathsSeen>().add_observer(
        |death: On<ModeDeath>, scores: Query<&Score>, rungs: Query<&Rung>, mut seen: ResMut<DeathsSeen>| {
            let k = death.died.attacker.unwrap();
            let kills = scores.get(k).map_or(0, |s| s.kills);
            let rung = rungs.get(k).map_or(0, |r| r.0);
            assert_eq!(death.mode, "chimera:ladder");
            seen.0.push((death.victim, kills, rung, death.died.damage.weapon));
        },
    );
    let a = player(&mut sim);
    let b = player(&mut sim);
    let c = player(&mut sim);
    settle(&mut sim);
    assert_eq!(testkit::active_id(sim.app.world(), a), Some(PISTOL));
    // Two kills in one tick, b's first.
    for victim in [b, c] {
        let at = sim.position(victim);
        sim.app.world_mut().write_message(Damage {
            target: victim,
            attacker: Some(a),
            amount: 5.0,
            point: at,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Chest,
            kind: DamageKind::Bullet,
            weapon: None,
            force: Vec3::ZERO,
        });
    }
    sim.ticks(2);
    let seen = std::mem::take(&mut sim.app.world_mut().resource_mut::<DeathsSeen>().0);
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert_eq!((seen[0].0, seen[0].1, seen[0].2, seen[0].3), (b, 1, 1, Some(PISTOL)), "b's death after its counting only");
    assert_eq!((seen[1].0, seen[1].1, seen[1].2), (c, 2, 2), "c's after its own");
    assert_eq!(rung(&sim, a), 2);
}

/// What happened to whom when: (player, tick, what).
#[derive(Resource, Default)]
struct JoinLog(Vec<(Entity, u64, String)>);

fn log_joins(app: &mut App) {
    use chimera_sim::{minigame::ladder::OnRung, weapon::Weapon};
    fn push(log: &mut JoinLog, clock: &SimClock, e: Entity, what: String) {
        log.0.push((e, clock.tick, what));
    }
    app.init_resource::<JoinLog>()
        .add_observer(|on: On<Insert<InMinigame>>, c: Res<SimClock>, mut log: ResMut<JoinLog>| {
            push(&mut log, &c, on.entity, "in".into())
        })
        .add_observer(|on: On<Insert<Team>>, q: Query<&Team, With<InMinigame>>, c: Res<SimClock>, mut log: ResMut<JoinLog>| {
            if let Ok(t) = q.get(on.entity) {
                push(&mut log, &c, on.entity, format!("team {}", t.0))
            }
        })
        .add_observer(|on: On<Insert<Rung>>, q: Query<&Rung>, c: Res<SimClock>, mut log: ResMut<JoinLog>| {
            push(&mut log, &c, on.entity, format!("rung {}", q.get(on.entity).unwrap().0))
        })
        .add_observer(|on: On<Insert<OnRung>>, q: Query<&OnRung>, c: Res<SimClock>, mut log: ResMut<JoinLog>| {
            push(&mut log, &c, on.entity, format!("on rung {}", q.get(on.entity).unwrap().index))
        })
        // A weapon has no owner yet as it is spawned: anyone's.
        .add_observer(|on: On<Add<Weapon>>, q: Query<&Weapon>, c: Res<SimClock>, mut log: ResMut<JoinLog>| {
            push(&mut log, &c, Entity::PLACEHOLDER, format!("weapon {}", q.get(on.entity).unwrap().id))
        })
        .add_observer(|on: On<Insert<Protected>>, c: Res<SimClock>, mut log: ResMut<JoinLog>| {
            push(&mut log, &c, on.entity, "protected".into())
        });
}

/// `p`'s log from tick `from` on, with the weapons spawned then: (tick,
/// what).
fn joins_of(sim: &Sim, p: Entity, from: u64) -> Vec<(u64, String)> {
    let log = sim.app.world().resource::<JoinLog>();
    log.0.iter().filter(|(e, t, _)| (*e == p || *e == Entity::PLACEHOLDER) && *t >= from).map(|(_, t, w)| (*t, w.clone())).collect()
}

/// A joiner is taken in in one tick, in the old order (plan ecs-cleanup
/// step 8, the rules' `Joined` and `Arming` as events): in the minigame,
/// its team, the ladder's join rung, then the life: armed for the rung
/// (`OnRung`, once), the rung's weapon, protected; a late joiner gets the
/// others' rung and its set.
#[test]
fn a_joiners_team_rung_and_arming_come_in_one_tick_in_order() {
    let mut sim = started(DEF);
    log_joins(&mut sim.app);
    let a = player(&mut sim);
    let joined = sim.app.world().resource::<SimClock>().tick + 1;
    sim.ticks(1);
    let t = |n: u64, w: &str| (n, w.to_string());
    let team = sim.app.world().get::<Team>(a).unwrap().0;
    assert_eq!(
        joins_of(&sim, a, 0),
        [
            t(joined, "in"),
            t(joined, &format!("team {team}")),
            t(joined, "rung 0"),
            t(joined, "on rung 0"),
            t(joined, "weapon kit:pistol"),
            t(joined, "protected"),
        ]
    );
    // Late: the others on the second rung, a joiner comes in on it.
    let b = player(&mut sim);
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    settle(&mut sim);
    kill(&mut sim, a, b, DamageKind::Bullet);
    settle(&mut sim);
    assert_eq!((rung(&sim, a), rung(&sim, b)), (1, 1));
    let c = player(&mut sim);
    let joined = sim.app.world().resource::<SimClock>().tick + 1;
    sim.ticks(1);
    let team = sim.app.world().get::<Team>(c).unwrap().0;
    assert_eq!(
        joins_of(&sim, c, joined),
        [
            t(joined, "in"),
            t(joined, &format!("team {team}")),
            t(joined, "rung 1"),
            t(joined, "on rung 1"),
            t(joined, "weapon kit:rifle"),
            t(joined, "protected"),
        ]
    );
    sim.ticks(5);
    assert_eq!(joins_of(&sim, c, joined + 1), [], "nothing again");
}

/// What a second observer of `minigame::Arming` saw: (player, the rules,
/// its rung then, weapons carried then, weapons carried when its queued
/// work ran).
#[derive(Resource, Default)]
struct ArmingsSeen(Vec<(Entity, &'static str, Option<usize>, usize, usize)>);

/// Anything may observe a life's arming (plan ecs-cleanup step 8): a
/// second observer of `minigame::Arming` runs after the old weapons were
/// taken and the ladder's join rung was set, and its queued world work
/// applies in the runtime's flush, before the life's weapons are given.
#[test]
fn a_second_observer_of_arming_runs_before_the_weapons_are_given() {
    use chimera_sim::{minigame::Arming, weapon::Inventory};
    let mut sim = started(DEF);
    sim.app.init_resource::<ArmingsSeen>().add_observer(
        |arming: On<Arming>, rungs: Query<&Rung>, inv: Query<&Inventory>, mut commands: Commands| {
            let (e, rules) = (arming.player, arming.rules);
            let rung = rungs.get(e).ok().map(|r| r.0);
            let now = inv.get(e).map_or(0, |i| i.weapons.len());
            commands.queue(move |w: &mut World| {
                let then = w.get::<Inventory>(e).map_or(0, |i| i.weapons.len());
                w.resource_mut::<ArmingsSeen>().0.push((e, rules, rung, now, then));
            });
        },
    );
    let a = player(&mut sim);
    sim.ticks(1);
    let b = player(&mut sim);
    settle(&mut sim);
    kill(&mut sim, b, a, DamageKind::Bullet);
    settle(&mut sim);
    let seen = &sim.app.world().resource::<ArmingsSeen>().0;
    let ladder = "chimera:ladder";
    // Each joiner's first life; both again at the match's start; b's
    // after the kill.
    assert_eq!(
        *seen,
        [
            (a, ladder, Some(0), 0, 0),
            (b, ladder, Some(0), 0, 0),
            (a, ladder, Some(0), 0, 0),
            (b, ladder, Some(0), 0, 0),
            (b, ladder, Some(0), 0, 0),
        ],
        "{seen:?}"
    );
    assert!(!sim.app.world().get::<Inventory>(b).unwrap().weapons.is_empty(), "then armed");
}
