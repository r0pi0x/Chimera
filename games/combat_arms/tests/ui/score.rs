//! The game mode on the HUD (hud_crosshair.md D6, D7) and the scoreboard
//! (ui_system.md, game_modes.md S13), headless: a TDM match on the
//! greybox shows its counters, the local team highlighted and no timer;
//! Elimination Pro shows its round timer; the scoreboard lists a kill.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Hitgroup, LocalPlayer, SimClock, Team};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        movement,
        rules::{CaMatch, CaMatchView, Phase},
        weapons::RIFLE,
    },
    harness::{CaSim, with_mode},
    ui::{
        hud::{
            CaHudStatePlugin,
            model::{SCORE_ALPHA, SCORE_PLAIN, goal_string, goal_text, round_dividers},
            score::CaScoreHud,
        },
        scoreboard::{CaScoreboard, CaScoreboardStatePlugin},
    },
};

fn sim(mode: &str) -> (Sim, Entity, Entity) {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaHudStatePlugin, CaScoreboardStatePlugin));
    });
    with_mode(sim.app.world_mut(), mode);
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    let them = sim.spawn_character(Vec3::new(2.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert((Team(1), LocalPlayer, Name::new("me")));
    sim.app.world_mut().entity_mut(them).insert((Team(2), Name::new("them")));
    for _ in 0..60 {
        if sim.app.world().resource::<CaMatch>().phase == Phase::Live {
            break;
        }
        sim.ticks(1);
    }
    (sim, me, them)
}

fn score_hud(sim: &Sim) -> CaScoreHud {
    sim.app.world().resource::<CaScoreHud>().clone()
}

#[test]
fn tdm_panel_counts_kills_without_a_timer() {
    let (mut sim, me, them) = sim("TDM");
    sim.seconds(4.1);
    sim.app.world_mut().write_message(Damage {
        target: them,
        attacker: Some(me),
        amount: 5.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: Some(RIFLE),
        force: Vec3::ZERO,
    });
    sim.ticks(2);
    let hud = score_hud(&sim);
    let panel = hud.panel.expect("a panel in TDM");
    assert_eq!(panel.mode_name, "Elimination");
    assert_eq!(panel.goal, "100 Kills", "the string table's (ours)");
    assert_eq!((panel.rows[0].score.as_str(), panel.rows[1].score.as_str()), ("1", "0"));
    assert_eq!(panel.rows[0].colour, SCORE_ALPHA, "our team highlighted");
    assert_eq!(panel.rows[1].colour, SCORE_PLAIN);
    assert_eq!(panel.rows[0].bar, 1.0, "floor(1 / 100 x 155)");
    assert_eq!(panel.highlight, Some(Vec2::new(344.0, 18.0)));
    assert_eq!(hud.timer, None, "TDM has no timer");
    // The scoreboard, while the score key is held.
    sim.app
        .world_mut()
        .resource_mut::<chimera_client::client::console::HeldActions>()
        .showscores = true;
    sim.ticks(1);
    let board = sim.app.world().resource::<CaScoreboard>().clone();
    assert!(board.shown);
    assert_eq!(board.lists[0][0].name, "me");
    assert_eq!((board.lists[0][0].kills, board.lists[1][0].deaths), (1, 1));
    assert_eq!(board.scores, [1, 0]);
}

#[test]
fn elimination_pro_shows_its_round_timer() {
    let (mut sim, ..) = sim("TSV");
    sim.seconds(1.0);
    let timer = score_hud(&sim).timer.expect("a timer in the warm-up");
    assert_eq!(timer, "00 : 08", "10 s warm-up, 1 s and a bit gone");
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-19): hud_crosshair.md 92-94, game_modes.md 87-91,
// ui_system.md 23, 24, 32

fn match_view(sim: &Sim) -> CaMatchView {
    sim.app.world().resource::<CaMatchView>().clone()
}

/// Run until `f` holds (at most `secs`).
fn until(sim: &mut Sim, secs: f32, f: impl Fn(&Sim) -> bool) {
    let mut left = (secs / 0.015) as u32 + 2;
    while !f(sim) {
        assert!(left > 0, "not within {secs} s");
        left -= 1;
        sim.ticks(1);
    }
}

fn live_round(sim: &mut Sim) {
    until(sim, 30.0, |s| {
        let v = match_view(s);
        v.phase == Phase::Live && !v.warmup
    });
}

fn board(sim: &Sim) -> CaScoreboard {
    sim.app.world().resource::<CaScoreboard>().clone()
}

fn kill(sim: &mut Sim, me: Entity, them: Entity) {
    sim.app.world_mut().write_message(Damage {
        target: them,
        attacker: Some(me),
        amount: 5.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: Some(RIFLE),
        force: Vec3::ZERO,
    });
    sim.ticks(2);
}

/// game_modes.md 87: the goal texts are the string table's.
#[test]
fn gm87_goal_text() {
    let ours = combat_arms::ui::CaUi::default().strings;
    let text = |key: &str, g: u32| goal_text(key, g, |id| ours.resolve(id));
    for key in ["TMM", "TSV", "SBT", "ESC"] {
        assert_eq!(text(key, 7), "7 Rounds", "{key}");
    }
    assert_eq!(text("CTF", 5), "5 Points");
    assert_eq!(text("SQM2", 300), "300 Points");
    assert_eq!(text("TDM", 100), "100 Kills");
    assert_eq!(text("HQM", 10), "10 Min.");
    assert_eq!(goal_string("TMM"), Some("IDS_GAMEROOM_ROUND"));
    assert_eq!(goal_string("CTF"), None, "the goal, a space and IDS_POINTS");
}

/// The install's table writes its inserts FormatMessage-style: the goal
/// header shows the number, not the raw `%1!d!`.
#[test]
fn goal_text_fills_the_installs_inserts() {
    let table = |id: &str| match id {
        "IDS_GAMEROOM_KILL" => "%1!d! Kills".to_string(),
        "IDS_GAMEROOM_ROUND" => "Goal: %1!d! Rounds".to_string(),
        other => other.to_string(),
    };
    assert_eq!(goal_text("TDM", 100, table), "100 Kills");
    assert_eq!(goal_text("TMM", 7, table), "Goal: 7 Rounds");
}

/// hud_crosshair.md 92, game_modes.md 88: the timer across a Search &
/// Destroy round: the setup's delay (none at 0), the round's time, the
/// bomb's fuse after a plant in its own timer, nothing at the result.
#[test]
fn hud_t92_gm88_timers_across_a_round() {
    use combat_arms::game::rules::{
        CaRuleSettings,
        objectives::{CaObjectives, Planted},
        objects::{BombZone, CaMapObjects, ObjBox},
    };
    let (mut sim, ..) = sim("TMM");
    // The warm-up's end; the next setup with the stock delay 0: no timer.
    until(&mut sim, 20.0, |s| matches!(match_view(s).phase, Phase::Intermission { .. }));
    assert_eq!(score_hud(&sim).timer, None, "the result: none");
    sim.app.world_mut().resource_mut::<CaRuleSettings>().object_start = 5.0;
    until(&mut sim, 10.0, |s| matches!(match_view(s).phase, Phase::Setup { .. }));
    assert_eq!(score_hud(&sim).timer.as_deref(), Some("00 : 05"), "a 5 s setup");
    live_round(&mut sim);
    assert_eq!(score_hud(&sim).timer.as_deref(), Some("03 : 00"));
    sim.seconds(1.5);
    assert_eq!(score_hud(&sim).timer.as_deref(), Some("02 : 58"));
    // A plant: the round timer hidden, the bomb timer the fuse.
    sim.app.world_mut().resource_mut::<CaMapObjects>().bomb_zones = vec![BombZone {
        family: "Doobic".into(),
        number: 0,
        defusers: 2,
        can_defuse: true,
        defuse_time: 7.0,
        detonate_time: 40.0,
        area: ObjBox {
            centre: Vec3::new(500.0, 0.0, 500.0),
            half: Vec3::ONE,
        },
    }];
    let (now, dt) = {
        let c = sim.app.world().resource::<SimClock>();
        (c.now, c.dt())
    };
    sim.app.world_mut().resource_mut::<CaObjectives>().planted = Some(Planted {
        zone: 0,
        number: 0,
        at: Vec3::new(500.0, 0.0, 500.0),
        team: 1,
        detonates: now + dt as f64 + 40.0 + 1e-3,
    });
    sim.ticks(1);
    let h = score_hud(&sim);
    assert_eq!((h.timer, h.bomb_timer.as_deref()), (None, Some("00 : 40")));
    sim.seconds(0.5);
    assert_eq!(score_hud(&sim).bomb_timer.as_deref(), Some("00 : 39"));
    // The round's result: both hidden.
    sim.app.world_mut().resource_mut::<combat_arms::game::rules::CaMatch>().win(2);
    sim.ticks(3);
    let h = score_hud(&sim);
    assert_eq!((h.timer, h.bomb_timer), (None, None));
}

/// hud_crosshair.md 93, game_modes.md 89: round modes' row titles count
/// each team's living players; the bars carry G - 1 round dividers.
#[test]
fn hud_t93_gm89_living_players_and_dividers() {
    let marks = round_dividers(Vec2::new(448.0, 32.0), Vec2::new(157.0, 16.0), 7);
    let xs: Vec<f32> = marks.iter().map(|(p, _)| p.x).collect();
    assert_eq!(xs, vec![471.0, 493.0, 515.0, 537.0, 559.0, 581.0]);
    assert!(marks.iter().all(|(p, h)| p.y == 34.0 && *h == 14.0));
    // Elimination Pro 2 v 2: one Bravo player dies, then the next round.
    let (mut sim, me, them) = sim("TSV");
    let mut extra = Vec::new();
    for (x, team) in [(4.0, 1), (6.0, 2)] {
        let e = sim.spawn_character(Vec3::new(x, 1.0, 0.0), movement::ID);
        sim.app.world_mut().entity_mut(e).insert(Team(team));
        extra.push(e);
    }
    live_round(&mut sim);
    // Past the round start's spawn protection.
    sim.seconds(4.5);
    let titles = |sim: &Sim| score_hud(sim).panel.map(|p| p.labels);
    assert_eq!(titles(&sim), Some(["2".to_string(), "2".to_string()]));
    let p = score_hud(&sim).panel.unwrap();
    assert!(p.titles_are_counts);
    assert_eq!(p.dividers, match_view(&sim).goal);
    kill(&mut sim, me, them);
    assert_eq!(titles(&sim), Some(["2".to_string(), "1".to_string()]));
    // Bravo's last one: the round ends; the next one starts with all.
    kill(&mut sim, me, extra[1]);
    until(&mut sim, 5.0, |s| score_hud(s).panel.is_none());
    until(&mut sim, 20.0, |s| matches!(match_view(s).phase, Phase::Live) && titles(s).is_some_and(|t| t[1] == "2"));
}

/// hud_crosshair.md 94, game_modes.md 91, ui_system.md 24: a TDM result
/// hides the mode panel and shows the banner alone (no lists), up to the
/// match's result.
#[test]
fn hud_t94_gm91_ui24_tdm_result() {
    use combat_arms::game::rules::CaRuleSettings;
    let (mut sim, me, them) = sim("TDM");
    sim.app.world_mut().resource_mut::<CaRuleSettings>().goal = 1;
    sim.seconds(4.1);
    assert!(score_hud(&sim).panel.is_some());
    kill(&mut sim, me, them);
    until(&mut sim, 1.0, |s| matches!(match_view(s).phase, Phase::RoundOver { .. }));
    sim.ticks(1);
    let b = board(&sim);
    assert!(b.shown && !b.lists_shown, "the banner alone");
    assert_eq!(b.result, Some(combat_arms::ui::scoreboard::RoundResult::Win));
    assert!(score_hud(&sim).panel.is_none(), "no mode panel");
    until(&mut sim, 10.0, |s| match_view(s).phase == Phase::MatchOver);
    sim.ticks(1);
    assert!(board(&sim).shown && score_hud(&sim).panel.is_none(), "up to the match's result");
}

/// game_modes.md 91, ui_system.md 23 and 24: a round mode's result (here
/// the warm-up's end) keeps the board up over the lists until the next
/// round's setup, whatever the score key does.
#[test]
fn gm91_ui23_result_until_the_next_setup() {
    let (mut sim, ..) = sim("TSV");
    until(&mut sim, 20.0, |s| matches!(match_view(s).phase, Phase::RoundOver { .. }));
    sim.ticks(1);
    let start = sim.app.world().resource::<SimClock>().now;
    let b = board(&sim);
    assert!(b.shown && b.lists_shown, "the banner over the lists");
    assert_eq!(b.result, None, "a warm-up without a winner: no banner");
    assert!(score_hud(&sim).panel.is_none());
    let held = |sim: &mut Sim, on: bool| {
        sim.app
            .world_mut()
            .resource_mut::<chimera_client::client::console::HeldActions>()
            .showscores = on
    };
    for (t, key) in [(3.0, true), (5.9, false)] {
        let now = sim.app.world().resource::<SimClock>().now;
        sim.seconds(t - (now - start));
        held(&mut sim, key);
        sim.ticks(1);
        assert!(board(&sim).shown, "shown at {t} s");
    }
    until(&mut sim, 1.0, |s| matches!(match_view(s).phase, Phase::Setup { .. } | Phase::Live));
    sim.ticks(1);
    assert!(!board(&sim).shown, "hidden at the next setup");
    assert!(score_hud(&sim).panel.is_some());
}

/// ui_system.md 25: the banner and the sound by mode and winner.
#[test]
fn ui25_banner_choice() {
    use combat_arms::ui::scoreboard::{RoundResult as R, round_result};
    assert_eq!(round_result("TMM", Some(1), Some(1), false).0, Some(R::Win));
    assert_eq!(round_result("TMM", Some(2), Some(1), false).0, Some(R::Lose));
    assert_eq!(round_result("FFA", Some(7), Some(3), false).0, Some(R::FfaLose));
    assert_eq!(round_result("QRT", Some(1), Some(1), false), (Some(R::HumanWin), Some("QRTHumanWinSound")));
    assert_eq!(round_result("TDM", Some(2), Some(1), false).1, Some("RoundBravoTeamWinSound"));
    assert_eq!(round_result("BNT", Some(2), Some(2), false).0, Some(R::FfaWin), "Spy Hunt off Alpha");
    assert_eq!(round_result("BNT", Some(1), Some(1), false).0, Some(R::Win));
    assert_eq!(round_result("TMM", None, Some(1), true), (None, None), "a training round");
    assert_eq!(round_result("TMM", None, Some(1), false).0, Some(R::Draw));
}

/// ui_system.md 32: the Esc menu hides the board the score key shows.
#[test]
fn ui32_popups_hide_the_scoreboard() {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((
            GreyboxMapPlugin,
            CaHudStatePlugin,
            CaScoreboardStatePlugin,
            combat_arms::ui::menu::CaMenuStatePlugin,
        ));
    });
    with_mode(sim.app.world_mut(), "TDM");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert((Team(1), LocalPlayer, Name::new("me")));
    sim.seconds(1.0);
    sim.app
        .world_mut()
        .resource_mut::<chimera_client::client::console::HeldActions>()
        .showscores = true;
    sim.ticks(1);
    assert!(board(&sim).shown);
    let open = |sim: &mut Sim, on: bool| {
        sim.app
            .world_mut()
            .resource_mut::<combat_arms::ui::menu::InGameMenu>()
            .set_open(on)
    };
    open(&mut sim, true);
    sim.ticks(1);
    assert!(!board(&sim).shown, "hidden under the menu");
    open(&mut sim, false);
    sim.ticks(1);
    assert!(board(&sim).shown, "back, the key still held");
}

/// game_modes_2.md M11.10, M11.4's warning and M13's lines: Hired Guns'
/// counters and the pocket, the squad's lines; VIP Escort's roles, the
/// exit and the swap.
#[test]
fn modes_part_two_lines() {
    use combat_arms::{
        game::rules::{
            mercs::MercOrder,
            objectives::{Bullion, CaCarry, CaObjectives, Exit},
        },
        ui::hud::score::{objective_lines, squad_lines},
    };
    let mut o = CaObjectives {
        placed: 80,
        banked: [7, 3],
        ..Default::default()
    };
    o.bullion = vec![
        Bullion {
            id: 1,
            at: Vec3::ZERO,
            bars: 1,
            money: 100,
            bag: false,
        },
        Bullion {
            id: 2,
            at: Vec3::ZERO,
            bars: 12,
            money: 1200,
            bag: true,
        },
    ];
    let pocket = |bars: u32| CaCarry {
        bars,
        money: bars as i32 * 100,
        ..Default::default()
    };
    let lines = objective_lines(&o, pocket(11), 0.0);
    assert!(lines.contains(&"Banked 7 - 3, in bags 12, unclaimed 1".to_string()), "{lines:?}");
    assert!(lines.contains(&"Pocket 11 bars".to_string()));
    assert!(lines.contains(&"Speed penalty 3 % with 11 bars".to_string()));
    let lines = objective_lines(&o, pocket(30), 0.0);
    assert!(lines.contains(&"Speed penalty 45 % with 30 bars".to_string()), "{lines:?}");
    let squad = squad_lines(Some(800), Some(MercOrder::FollowMe), &[("Squad_1".into(), 0.5)]);
    assert_eq!(squad, ["Bank 800", "Mercenaries (follow me):", "  Squad_1 50 %"]);
    let esc = CaObjectives {
        escort: 2,
        exit: Some(Exit { opens: 30.0, open: false, ..Default::default() }),
        rounds_to_swap: 2,
        ..Default::default()
    };
    let lines = objective_lines(&esc, CaCarry::default(), 10.0);
    assert_eq!(lines, ["Bravo escorts the VIP", "The exit opens in 20 s", "Rounds until the swap: 2"]);
}

/// The mode's name and goal on the panel for Hired Guns.
#[test]
fn hired_guns_panel() {
    let (sim, _me, _them) = sim("SQM");
    let hud = score_hud(&sim);
    let panel = hud.panel.expect("a panel");
    assert_eq!((panel.mode_name.as_str(), panel.goal.as_str()), ("Hired Guns", "7 Rounds"));
}
