//! The race chip and its Tab board (`client::race_chip`, the Signal look):
//! only under a theme that turns the Signal HUD on, the chip stands in
//! for the minigame panel while a ladder plays; Tab opens its plate into
//! the board fast (0.12 s) and folds it back (0.09 s) from wherever it is;
//! leading, the chip shows the gap to second place; the board lists every
//! player best first, with the gun's icon (its game's own when it has
//! one, else the kit's by class), the bot mark and the YOU row.

use bevy::prelude::*;
use chimera_client::client::{
    console::HeldActions,
    minigame_scoreboard::{GunNames, MinigameScoreboardPlugin, ScoreboardRoot},
    race_chip::{BoardContent, BoardRow, ChipContent, LeaderChip, PingMeter, RaceChipRoot, RacePlate, ping_level},
    signal::icons::SignalIcon,
    weapon_icons::{WeaponIcon, WeaponIcons},
};
use chimera_core::core::{Bot, LocalPlayer, Score};
use chimera_sim::{
    SimBasePlugins,
    minigame::{
        hud::MinigameHud,
        ladder::{Ladder, Rung},
        runtime::InMinigame,
    },
    weapon::data::WeaponDataAppExt,
};
use chimera_ui::{Theme, UiClock, UiUnits};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

const SIGNAL: &str = "name = \"test\"\n[flags.hud]\nsignal = true\n[icons]\nprefer_game = true\n";

fn sim(signal: bool) -> Sim {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins((GreyboxMapPlugin, MinigameScoreboardPlugin));
        app.add_weapon_data(&[
            ("p.toml", "id = \"kit:weapon_pistol\"\nslot = 1\ndraw = 0.5\nclass = \"pistol\"\n"),
            ("r.toml", "id = \"kit:weapon_rifle\"\nslot = 0\ndraw = 0.5\nclass = \"carbine\"\n"),
            ("s.toml", "id = \"other:weapon_shotgun\"\nslot = 0\ndraw = 0.5\nclass = \"shotgun\"\n"),
        ])
        .expect("the data reads");
    });
    if signal {
        sim.app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
    }
    sim.app.insert_resource(UiClock::manual());
    sim.app.init_resource::<HeldActions>();
    sim.app.insert_resource(Ladder {
        rungs: vec!["kit:weapon_pistol", "kit:weapon_rifle", "other:weapon_shotgun", "kit:weapon_pistol"],
        ..default()
    });
    sim.app.world_mut().insert_resource(GunNames::default());
    sim.app.world_mut().insert_resource(MinigameHud { title: "Gun game".into(), ..default() });
    sim
}

fn player(sim: &mut Sim, name: &'static str, rung: usize, kills: i32, local: bool, bot: bool) {
    let mut e = sim.app.world_mut().spawn((InMinigame, Name::new(name), Rung(rung), Score { kills, deaths: 1 }));
    if local {
        e.insert(LocalPlayer);
    }
    if bot {
        e.insert(Bot);
    }
}

/// One frame of `dt` seconds on the UI clock.
fn frame(sim: &mut Sim, dt: f32) {
    sim.app.world_mut().resource_mut::<UiClock>().step(dt);
    sim.app.update();
}

fn plate_w(sim: &mut Sim) -> f32 {
    let w = sim.app.world_mut();
    match w.query_filtered::<&Node, With<RacePlate>>().single(w).unwrap().width {
        Val::Px(v) => v,
        other => panic!("plate width {other:?}"),
    }
}

fn visible<C: Component>(sim: &mut Sim) -> bool {
    let w = sim.app.world_mut();
    w.query_filtered::<&Visibility, With<C>>().iter(w).any(|v| *v != Visibility::Hidden)
}

fn count<C: Component>(sim: &mut Sim) -> usize {
    let w = sim.app.world_mut();
    w.query_filtered::<(), With<C>>().iter(w).count()
}

fn texts_under<C: Component>(sim: &mut Sim) -> Vec<String> {
    let w = sim.app.world_mut();
    let roots: Vec<Entity> = w.query_filtered::<Entity, With<C>>().iter(w).collect();
    let mut out = Vec::new();
    let mut stack = roots;
    while let Some(e) = stack.pop() {
        if let Some(t) = w.get::<Text>(e) {
            out.push(t.0.clone());
        }
        if let Some(c) = w.get::<Children>(e) {
            stack.extend(c.iter());
        }
    }
    out
}

#[test]
fn race_chip_opens_and_folds_on_showscores() {
    let mut sim = sim(true);
    // Chasing (the leader's chip is wider: `leader_chip_shows_the_gap`).
    player(&mut sim, "Alpha", 1, 4, true, false);
    player(&mut sim, "Botty", 2, 1, false, true);
    for _ in 0..3 {
        frame(&mut sim, 1.0 / 60.0);
    }
    let cq = UiUnits::default().cq(1.0);
    assert!(visible::<RaceChipRoot>(&mut sim), "the chip is up while the ladder plays");
    assert!(visible::<ChipContent>(&mut sim) && !visible::<BoardContent>(&mut sim));
    assert!((plate_w(&mut sim) - 21.0 * cq).abs() < 1.0, "the chip is 21 cq wide: {}", plate_w(&mut sim));

    // Tab: open in 0.12 s, the old scoreboard stays down.
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    frame(&mut sim, 0.0);
    frame(&mut sim, 0.06);
    let mid = plate_w(&mut sim);
    assert!(mid > 21.0 * cq + 1.0 && mid < 64.0 * cq - 1.0, "halfway open: {mid}");
    frame(&mut sim, 0.07);
    frame(&mut sim, 0.0);
    assert!((plate_w(&mut sim) - 64.0 * cq).abs() < 1.0, "open in 0.12 s: {}", plate_w(&mut sim));
    assert!(visible::<BoardContent>(&mut sim) && !visible::<ChipContent>(&mut sim));
    {
        let w = sim.app.world_mut();
        let v = w.query_filtered::<&Visibility, With<ScoreboardRoot>>().single(w).unwrap();
        assert_eq!(*v, Visibility::Hidden, "the old board stays down under the Signal look");
    }

    // Released: folds back within 0.09 s.
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = false;
    frame(&mut sim, 0.0);
    frame(&mut sim, 0.1);
    frame(&mut sim, 0.0);
    assert!((plate_w(&mut sim) - 21.0 * cq).abs() < 1.0, "folded in 0.09 s: {}", plate_w(&mut sim));
    assert!(visible::<ChipContent>(&mut sim) && !visible::<BoardContent>(&mut sim));

    // Spammed: released halfway, it folds from where it is (no jump).
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    frame(&mut sim, 0.0);
    frame(&mut sim, 0.05);
    let opened = plate_w(&mut sim);
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = false;
    frame(&mut sim, 0.0);
    frame(&mut sim, 0.01);
    let folding = plate_w(&mut sim);
    assert!(folding <= opened + 0.5 && folding > 21.0 * cq + 1.0, "folds from {opened}, not from the end: {folding}");
}

#[test]
fn race_chip_needs_the_signal_theme() {
    let mut sim = sim(false);
    player(&mut sim, "Alpha", 2, 4, true, false);
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    for _ in 0..3 {
        frame(&mut sim, 1.0 / 60.0);
    }
    assert!(!visible::<RaceChipRoot>(&mut sim), "the shell's default theme keeps today's panel and board");
    let w = sim.app.world_mut();
    let v = w.query_filtered::<&Visibility, With<ScoreboardRoot>>().single(w).unwrap();
    assert_ne!(*v, Visibility::Hidden);
}

#[test]
fn leader_chip_shows_the_gap() {
    let mut sim = sim(true);
    player(&mut sim, "Alpha", 3, 4, true, false);
    player(&mut sim, "Larry", 1, 9, false, false);
    for _ in 0..3 {
        frame(&mut sim, 1.0 / 60.0);
    }
    assert_eq!(count::<LeaderChip>(&mut sim), 1, "leading: the leader variant");
    let texts = texts_under::<ChipContent>(&mut sim);
    for want in ["+2", "OVER", "LARRY", "04", "YOU"] {
        assert!(texts.iter().any(|t| t == want), "{want} in {texts:?}");
    }
    let w = sim.app.world_mut();
    let crowns = w.query::<&SignalIcon>().iter(w).filter(|i| i.name == "crown").count();
    assert_eq!(crowns, 1, "the crown");
    let cq = UiUnits::default().cq(1.0);
    for _ in 0..5 {
        frame(&mut sim, 0.05);
    }
    assert!((plate_w(&mut sim) - 23.0 * cq).abs() < 1.0, "leading, the chip is 23 cq wide: {}", plate_w(&mut sim));

    // Overtaken: the chasing chip, LEAD and the leader's name.
    let larry = {
        let w = sim.app.world_mut();
        w.query::<(Entity, &Name)>().iter(w).find(|(_, n)| n.as_str() == "Larry").unwrap().0
    };
    sim.app.world_mut().entity_mut(larry).insert(Rung(3)).insert(Score { kills: 20, deaths: 0 });
    frame(&mut sim, 1.0 / 60.0);
    frame(&mut sim, 1.0 / 60.0);
    assert_eq!(count::<LeaderChip>(&mut sim), 0);
    let texts = texts_under::<ChipContent>(&mut sim);
    for want in ["LEAD", "LARRY", "YOU"] {
        assert!(texts.iter().any(|t| t == want), "{want} in {texts:?}");
    }
    for _ in 0..10 {
        frame(&mut sim, 0.02);
    }
    assert!((plate_w(&mut sim) - 21.0 * cq).abs() < 1.0, "chasing, back to 21 cq: {}", plate_w(&mut sim));
}

#[test]
fn board_rows_best_first_with_icons() {
    let mut sim = sim(true);
    // The kit's game has its own icons (glyphs); `other` has none.
    let mut icons = WeaponIcons::default();
    icons.register("kit", |_| Some(WeaponIcon::Glyph { font: Handle::default(), ch: 'a', em: 12.0 }));
    sim.app.insert_resource(icons);
    player(&mut sim, "Alpha", 1, 4, true, false);
    player(&mut sim, "Botty", 2, 1, false, true);
    player(&mut sim, "Cee", 1, 6, false, false);
    let cee = {
        let w = sim.app.world_mut();
        w.query::<(Entity, &Name)>().iter(w).find(|(_, n)| n.as_str() == "Cee").unwrap().0
    };
    sim.app.world_mut().entity_mut(cee).insert(chimera_net::net::NetScore { ping: 150, ..default() });
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    for _ in 0..4 {
        frame(&mut sim, 0.05);
    }
    let w = sim.app.world_mut();
    // In the board's order (its children's).
    let board = w.query_filtered::<&Children, With<BoardContent>>().single(w).unwrap().to_vec();
    let rows: Vec<(String, bool, bool, Option<&'static str>)> = board
        .iter()
        .filter_map(|e| w.get::<BoardRow>(*e))
        .map(|r| (r.0.name.clone(), r.0.bot, r.0.local, r.0.class))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("Botty".to_string(), true, false, Some("shotgun")),
            ("Cee".to_string(), false, false, Some("rifle")),
            ("Player".to_string(), false, true, Some("rifle")),
        ],
        "best first: rung, then kills; carbine draws the rifle"
    );
    // Your row is tinted and outlined; the bot's slot has the robot.
    let yours = w.query::<(&BoardRow, Option<&Outline>, Option<&BackgroundColor>)>().iter(w).find(|(r, ..)| r.0.local).map(|(_, o, b)| (o.is_some(), b.is_some()));
    assert_eq!(yours, Some((true, true)), "your row is tinted and outlined");
    let names: Vec<&str> = w.query::<&SignalIcon>().iter(w).map(|i| i.name.as_str()).collect();
    assert!(names.contains(&"bot"), "the bot mark: {names:?}");
    assert!(names.contains(&"kill") && names.contains(&"death"), "the K and D marks: {names:?}");
    // The shotgun's game has no icon: the kit's; the kit's guns: their own.
    assert!(names.contains(&"shotgun"), "the fallback icon: {names:?}");
    assert!(!names.contains(&"rifle"), "a gun with its game's glyph draws that: {names:?}");
}

/// No board row leaves its gun cell empty: a game icon whose art isn't
/// loaded, or a gun of no class, draws the kit's; session wins show as
/// the trophy and the count, only when any.
#[test]
fn every_board_row_has_a_gun_icon() {
    use chimera_client::client::race_chip::SessionWins;
    let mut sim = sim(true);
    // The kit's game hands over an image that never loads.
    let mut icons = WeaponIcons::default();
    icons.register("kit", |_| Some(WeaponIcon::Image { image: Handle::default(), size: Vec2::new(64.0, 32.0) }));
    sim.app.insert_resource(icons);
    player(&mut sim, "Alpha", 1, 4, true, false);
    player(&mut sim, "Botty", 2, 1, false, true);
    player(&mut sim, "Cee", 0, 6, false, false);
    let alpha = {
        let w = sim.app.world_mut();
        w.query::<(Entity, &Name)>().iter(w).find(|(_, n)| n.as_str() == "Alpha").unwrap().0
    };
    sim.app.world_mut().entity_mut(alpha).insert(chimera_sim::minigame::wins::MatchesWon(2));
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    for _ in 0..3 {
        frame(&mut sim, 0.05);
    }
    let w = sim.app.world_mut();
    let names: Vec<String> = w.query::<&SignalIcon>().iter(w).map(|i| i.name.clone()).collect();
    for want in ["rifle", "pistol", "shotgun"] {
        assert!(names.iter().any(|n| n == want), "{want} drawn from the kit: {names:?}");
    }
    let wins: Vec<u32> = w.query::<&SessionWins>().iter(w).map(|s| s.0).collect();
    assert_eq!(wins, vec![2], "only Alpha's wins, as a trophy and its count");
    assert!(names.iter().any(|n| n == "trophy"));
}

/// The board's ping column: 4 bars under the theme's `net.good`, 3 under
/// `fair`, 2 (warn) under `bad`, else 1 (threat); a bot shows BOT.
#[test]
fn ping_colours_by_the_themes_thresholds() {
    use chimera_ui::Tone;
    let t = Theme::from_toml(SIGNAL).unwrap();
    let got: Vec<(u16, (u32, Tone))> = [0, 59, 60, 119, 120, 199, 200, 900].map(|p| (p, ping_level(&t, p, false))).map(|(p, l)| (p, (l.lit, l.tone))).into();
    assert_eq!(
        got,
        vec![
            (0, (4, Tone::Good)),
            (59, (4, Tone::Good)),
            (60, (3, Tone::Fg)),
            (119, (3, Tone::Fg)),
            (120, (2, Tone::Warn)),
            (199, (2, Tone::Warn)),
            (200, (1, Tone::Threat)),
            (900, (1, Tone::Threat)),
        ]
    );
    let strict = Theme::from_toml(&format!("{SIGNAL}[network]\nping_quiet_ms = 30\nping_fine_ms = 50\nping_warn_ms = 80\ngood_alpha = 0.7\n")).unwrap();
    assert_eq!(ping_level(&strict, 60, false).lit, 2, "the thresholds are the theme's");
    assert_eq!((ping_level(&t, 30, true).lit, ping_level(&t, 30, true).tone), (1, Tone::Threat), "a lossy line is the worst");
    assert_eq!(srgba(t.tone(Tone::Warn)), [255, 181, 71]);

    // On the board: Cee at 150 ms has two bars; the bot none.
    let mut sim = sim(true);
    player(&mut sim, "Alpha", 1, 4, true, false);
    player(&mut sim, "Botty", 2, 1, false, true);
    let alpha = {
        let w = sim.app.world_mut();
        w.query::<(Entity, &Name)>().iter(w).find(|(_, n)| n.as_str() == "Alpha").unwrap().0
    };
    sim.app.world_mut().entity_mut(alpha).insert(chimera_net::net::NetScore { ping: 150, ..default() });
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    for _ in 0..3 {
        frame(&mut sim, 0.05);
    }
    let w = sim.app.world_mut();
    let meters: Vec<u8> = w.query::<&PingMeter>().iter(w).map(|m| m.0).collect();
    assert_eq!(meters, vec![2], "one meter (the bot shows BOT): {meters:?}");
}

fn srgba(c: Color) -> [u8; 3] {
    let s = c.to_srgba();
    [(s.red * 255.0).round() as u8, (s.green * 255.0).round() as u8, (s.blue * 255.0).round() as u8]
}

/// The UI parity check's staging (`ui_parity`, docs/OBSERVABILITY.md "UI
/// parity against the look book"): the look book's fixture shows on the
/// board in its order, and every node the look book names carries its
/// name (`UiParityId`), rows counted from 1.
#[test]
fn a_fixture_stages_the_board_with_the_look_books_names() {
    use chimera_client::client::{race_chip::RaceStaged, ui_parity::Fixture};
    use chimera_ui::UiParityId;
    let mut sim = sim(true);
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/ui-parity/fixtures/hud.json");
    let fixture = Fixture::load(path).expect("the fixture reads");
    sim.app.insert_resource(RaceStaged { view: fixture.view(false), open: true });
    for _ in 0..4 {
        frame(&mut sim, 0.05);
    }
    let w = sim.app.world_mut();
    let ids: std::collections::HashMap<String, Entity> =
        w.query::<(Entity, &UiParityId)>().iter(w).map(|(e, id)| (id.0.clone(), e)).collect();
    let text = |w: &World, id: &str| ids.get(id).and_then(|e| w.get::<Text>(*e)).map(|t| t.0.clone());
    for (i, row) in fixture.rows.iter().enumerate() {
        let p = format!("board.row.{}", i + 1);
        assert_eq!(text(w, &format!("{p}.name")), Some(row.name.to_uppercase()), "{p}'s name");
        assert_eq!(text(w, &format!("{p}.rung")), Some(format!("{:02}", row.rung)), "{p}'s rung");
        for part in ["", ".pos", ".bot", ".icon", ".gun", ".game", ".k", ".d", ".ping", ".ladder", ".ticks"] {
            assert!(ids.contains_key(&format!("{p}{part}")), "{p}{part} is named");
        }
        assert_eq!(ids.contains_key(&format!("{p}.host")), row.host, "{p}.host only for the host");
        assert_eq!(ids.contains_key(&format!("{p}.wins")), row.wins > 0, "{p}.wins only with wins");
    }
    for id in ["race.plate", "board", "board.head.tag", "board.head.map", "board.head.game", "board.th.k", "board.th.ladder"] {
        assert!(ids.contains_key(id), "{id} is named");
    }
}
