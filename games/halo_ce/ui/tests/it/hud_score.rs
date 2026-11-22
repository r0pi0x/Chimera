//! The match's text on Halo's HUD (`hud::score`): the score line of a
//! team game and of free for all (a time-scored game in minutes and
//! seconds, the clock counting down under a time limit), and the game-over
//! line by winner.

use halo_ce::game::rules::{BLUE, RED, view::HaloMatchView};
use halo_ce_ui::hud::score::{clock, game_over, line};

#[test]
fn the_score_and_game_over_lines() {
    assert_eq!(clock(30 * 75), "1:15");
    let mut v = HaloMatchView { name: "CTF".into(), base: 1, team_play: true, score_to_win: 3, scores: [2, 1], time: 30 * 61, ..Default::default() };
    assert_eq!(line(&v, None, None), "CTF   1:01   Red 2  Blue 1   (to 3)");
    v.time_limit = 600.0;
    assert!(line(&v, None, None).contains("   8:59   "));
    let king = HaloMatchView { name: "King".into(), base: 4, score_to_win: 3600, minutes: true, time: 300, ..Default::default() };
    assert_eq!(line(&king, Some(450), Some(900)), "King   0:10   You 0:15  Leader 0:30   (to 2:00)");
    assert_eq!(game_over(&v, Some(RED), None), "");
    v.over = true;
    v.winner = Some(BLUE);
    assert_eq!(game_over(&v, Some(RED), None), "Game over\nBlue team wins");
    v.winner = None;
    assert_eq!(game_over(&v, Some(RED), None), "Game over\nTie");
    let ffa = HaloMatchView { over: true, winner: Some(7), ..king };
    assert_eq!(game_over(&ffa, Some(7), None), "Game over\nYou win");
    assert_eq!(game_over(&ffa, Some(5), Some("Bot 2")), "Game over\nBot 2 wins");
}
