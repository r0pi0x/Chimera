//! The match-end screen (`client::match_end`): nothing without a minigame
//! nor before the match-end hold (`minigame::NextMatch`: the win's own
//! callout has the match's end to itself); then the winner big, the final
//! standings in the scoreboard's columns with the local player's row
//! marked, the next map and a countdown to the change, and the winner
//! sound asked for once (the HUD's game's file first), the games' panels
//! told to step aside; gone with the results. Sized by the window's
//! height.

use bevy::prelude::*;
use chimera_client::client::{
    match_end::{MatchEndPart, MatchEndPlugin, MatchEndRoot, MatchEndRow, MatchEndView, WIDTHS, layout_height, next_line, scale, sound_candidates},
    minigame_scoreboard::GunNames,
};
use chimera_core::{
    core::{LocalPlayer, SimClock},
    game_parts::{GameParts, PartsRegistry},
};
use chimera_net::net::NetSettings;
use chimera_sim::{
    SimBasePlugins,
    minigame::{
        NextMatch,
        hud::MinigameHud,
        results::{MatchResults, ResultRow},
    },
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

static LOUD: GameParts = GameParts {
    game: "loud",
    guns: &[],
    knife: None,
    movement: None,
    characters: None,
    hud: None,
    maps: &[],
    announcer: None,
    match_won_sound: Some("sound/loud_win.wav"),
    match_lost_sound: None,
    music: false,
    cues: None,
};
static QUIET: GameParts = GameParts {
    game: "quiet",
    match_won_sound: None,
    ..LOUD
};
static OTHER: GameParts = GameParts {
    game: "other",
    match_won_sound: Some("snd/other_win.wav"),
    ..LOUD
};

fn view(sim: &Sim) -> MatchEndView {
    sim.app.world().resource::<MatchEndView>().clone()
}

fn shown(sim: &mut Sim) -> bool {
    let w = sim.app.world_mut();
    *w.query_filtered::<&Visibility, With<MatchEndRoot>>().single(w).unwrap() != Visibility::Hidden
}

fn part(sim: &mut Sim, which: MatchEndPart) -> String {
    let w = sim.app.world_mut();
    w.query::<(&MatchEndPart, &Text)>().iter(w).find(|(p, _)| **p == which).map(|(_, t)| t.0.clone()).unwrap()
}

fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

#[test]
fn the_hold_shows_the_winner_the_standings_and_the_next_map() {
    let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, MatchEndPlugin));
    let mut parts = PartsRegistry::default();
    for p in [&OTHER, &QUIET, &LOUD] {
        parts.register(p);
    }
    sim.app.insert_resource(parts);
    // The local player is named as the scoreboard names it (its `name`).
    sim.app.insert_resource(NetSettings { name: "Me".into(), ..default() });
    sim.app.insert_resource(GunNames([("kit:rifle".to_string(), "K&M Rifle".to_string())].into()));
    let me = sim.app.world_mut().spawn((LocalPlayer, Name::new("Me"))).id();
    let ann = sim.app.world_mut().spawn(Name::new("Ann")).id();
    sim.ticks(3);
    assert!(!view(&sim).shown && !shown(&mut sim), "nothing without a minigame (plain games)");

    // The match is decided: the results are in, but the hold not yet.
    let row = |name: &str, player, kills, gun: &str, wins, winner| ResultRow {
        name: name.into(),
        kills,
        deaths: 2,
        rung: Some(1),
        winner,
        player: Some(player),
        gun: Some(gun.into()),
        wins,
    };
    let results = MatchResults {
        title: "Kit gun game".into(),
        rungs: 3,
        rows: vec![row("Ann", ann, 7, "kit:shotgun", 2, true), row("Server's name for me", me, 3, "kit:rifle", 0, false)],
    };
    sim.app.insert_resource(MinigameHud {
        title: "Kit gun game".into(),
        results: Some(results.clone()),
        ..default()
    });
    sim.ticks(3);
    assert!(!view(&sim).shown, "the match's end has the win callout to itself");

    // The hold: the screen with the next map, counting down.
    let at = now(&sim) + 5.0;
    sim.app.insert_resource(NextMatch {
        map: Some("cs_source:de_dust2".into()),
        at,
    });
    sim.ticks(2);
    let v = view(&sim);
    assert!(v.shown && shown(&mut sim));
    assert!(sim.app.world().resource::<MinigameHud>().results_shown, "the games' panels leave the results to it");
    assert_eq!(v.title, "Kit gun game");
    assert_eq!((v.winner.as_deref(), v.you_won), (Some("Ann"), false));
    let want = [
        MatchEndRow {
            place: 1,
            name: "Ann".into(),
            kills: 7,
            deaths: 2,
            gun: "SHOTGUN".into(),
            wins: 2,
            winner: true,
            local: false,
        },
        MatchEndRow {
            place: 2,
            name: "Me".into(),
            kills: 3,
            deaths: 2,
            gun: "K&M Rifle".into(),
            wins: 0,
            winner: false,
            local: true,
        },
    ];
    assert_eq!(v.rows, want, "the board's names and guns; my row marked");
    assert_eq!(v.next, "Next map: de_dust2 in 5");
    assert_eq!(part(&mut sim, MatchEndPart::Winner), "Ann");
    assert_eq!(part(&mut sim, MatchEndPart::Subtitle), "wins the match");
    assert_eq!(part(&mut sim, MatchEndPart::Next), "Next map: de_dust2 in 5");
    // The winner sound: asked once, every game's file in order, none for
    // a game without one (no HUD's game here: the order registered).
    assert_eq!(v.sounds_asked, 1);
    assert_eq!(
        v.sound,
        [("other".to_string(), "snd/other_win.wav".to_string()), ("loud".to_string(), "sound/loud_win.wav".to_string())]
    );

    sim.seconds(3.2);
    assert_eq!(part(&mut sim, MatchEndPart::Next), "Next map: de_dust2 in 2");
    sim.seconds(2.0);
    assert_eq!(part(&mut sim, MatchEndPart::Next), "Changing map to de_dust2", "the map change under way");
    assert_eq!(view(&sim).sounds_asked, 1, "once a match");

    // The next match: gone.
    sim.app.world_mut().resource_mut::<MinigameHud>().results = None;
    sim.app.world_mut().remove_resource::<NextMatch>();
    sim.ticks(2);
    assert!(!view(&sim).shown && !shown(&mut sim));
    assert!(!sim.app.world().resource::<MinigameHud>().results_shown);

    // My win, then a draw (no sound for a draw).
    let mut mine = results.clone();
    mine.rows[0].winner = false;
    mine.rows[1].winner = true;
    sim.app.world_mut().resource_mut::<MinigameHud>().results = Some(mine);
    sim.app.insert_resource(NextMatch { map: None, at: now(&sim) + 8.0 });
    sim.ticks(2);
    let v = view(&sim);
    assert_eq!((v.winner.as_deref(), v.you_won, v.sounds_asked), (Some("Me"), true, 2));
    assert_eq!(part(&mut sim, MatchEndPart::Subtitle), "You win the match!");
    assert_eq!(v.next, "Next match in 8");
    sim.app.world_mut().resource_mut::<MinigameHud>().results = None;
    sim.ticks(2);
    let mut draw = results;
    draw.rows[0].winner = false;
    sim.app.world_mut().resource_mut::<MinigameHud>().results = Some(draw);
    sim.ticks(2);
    let v = view(&sim);
    assert!(v.shown && v.winner.is_none());
    assert_eq!(part(&mut sim, MatchEndPart::Winner), "Draw");
    assert_eq!(v.sounds_asked, 2, "a draw plays none");
}

#[test]
fn the_huds_game_sound_comes_first_and_the_countdown_words() {
    let mut parts = PartsRegistry::default();
    for p in [&OTHER, &QUIET, &LOUD] {
        parts.register(p);
    }
    let got = sound_candidates(&parts, Some("loud"));
    assert_eq!(got[0], ("loud".to_string(), "sound/loud_win.wav".to_string()));
    assert_eq!(got.len(), 2);
    let next = NextMatch {
        map: Some("greybox".into()),
        at: 0.0,
    };
    assert_eq!(next_line(&next, 0.4), "Next map: greybox in 1");
    assert_eq!(next_line(&next, -0.1), "Changing map to greybox");
    assert_eq!(next_line(&NextMatch { map: None, at: 0.0 }, 0.0), "Next match starting");
}

/// The screen scales with the window's height as the kill feed does
/// (sized for 720 lines, smaller when the rows wouldn't fit): at 1080 the winner and the standings' cells are
/// half as big again, and again when the window changes.
#[test]
fn the_screen_scales_with_the_windows_height() {
    use bevy::window::{PrimaryWindow, WindowResolution};
    assert_eq!(scale(720.0, 2), 1.0);
    assert_eq!(scale(1080.0, 2), 1.5);
    assert_eq!(scale(100.0, 2), scale(240.0, 2), "never under a third");
    // A full server's standings: smaller, to fit the window.
    assert!(scale(1080.0, 16) < 1.5 && scale(1080.0, 16) * layout_height(16) <= 1080.0);
    assert!(layout_height(6) <= 720.0, "six rows fit at full size");
    assert!(scale(1080.0, 10) < 1.5, "ten at 1080 lines: a little smaller (the countdown was on the edge)");
    let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, MatchEndPlugin));
    let window = sim
        .app
        .world_mut()
        .spawn((Window { resolution: WindowResolution::from((1920u32, 1080u32)), ..default() }, PrimaryWindow))
        .id();
    sim.ticks(2);
    let row = ResultRow {
        name: "Ann".into(),
        kills: 3,
        winner: true,
        ..Default::default()
    };
    sim.app.insert_resource(MinigameHud {
        title: "Gun game".into(),
        results: Some(MatchResults { title: "Gun game".into(), rungs: 3, rows: vec![row] }),
        ..default()
    });
    sim.app.insert_resource(NextMatch { map: None, at: now(&sim) + 5.0 });
    sim.ticks(3);
    assert!(shown(&mut sim));
    let sizes = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let winner = w
            .query::<(&MatchEndPart, &TextFont)>()
            .iter(w)
            .find(|(p, _)| **p == MatchEndPart::Winner)
            .map(|(_, f)| f.font_size)
            .unwrap();
        // A standings cell: a text with a fixed width (the name column's).
        let cell = w
            .query::<(&Text, &TextFont, &Node)>()
            .iter(w)
            .find(|(t, _, n)| t.0 == "Ann" && n.width != Val::Auto)
            .map(|(_, f, n)| (f.font_size, n.width))
            .expect("Ann's row");
        (winner, cell)
    };
    let (winner, (cell, width)) = sizes(&mut sim);
    assert_eq!(winner, FontSize::Px(96.0), "64 at 720 lines");
    assert_eq!(cell, FontSize::Px(30.0), "20 at 720 lines");
    assert_eq!(width, Val::Px(WIDTHS[1] * 1.5));
    // A smaller window: smaller again.
    sim.app.world_mut().get_mut::<Window>(window).unwrap().resolution = WindowResolution::from((1280u32, 720u32));
    sim.ticks(2);
    let (winner, (cell, _)) = sizes(&mut sim);
    assert_eq!((winner, cell), (FontSize::Px(64.0), FontSize::Px(20.0)));
}
