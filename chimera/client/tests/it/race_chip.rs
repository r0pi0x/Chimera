//! The race chip and its Tab board (`client::race_chip`, the Signal look):
//! only under a theme that turns the Signal HUD on, the chip stands in
//! for the minigame panel while a ladder plays; Tab opens its plate into
//! the board fast (0.12 s) and folds it back (0.09 s) from wherever it is;
//! leading, the chip shows the gap to second place; the board lists every
//! player best first, with the gun's icon (its game's own when it has
//! one, else the kit's by class), the bot mark and the YOU row.

use bevy::prelude::*;
use chimera_client::client::{
    console::{HeldActions, poll_showscores},
    minigame_scoreboard::{GunNames, MinigameScoreboardPlugin, ScoreboardRoot},
    race_chip::{BoardContent, BoardRow, ChipContent, ChipLine, LeaderChip, PingMeter, RaceChipRoot, RacePlate, ping_level},
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
    sim_with(signal, |_| {})
}

fn sim_with(signal: bool, extra: impl FnOnce(&mut App) + Send + Sync + 'static) -> Sim {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        extra(app);
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

/// The board reads the session wins the server replicates on the
/// characters (`MatchesWon`): a new count shows on the open board, the
/// bot's too, and a player with none has no trophy.
#[test]
fn the_board_follows_the_replicated_wins() {
    use chimera_client::client::race_chip::SessionWins;
    use chimera_sim::minigame::wins::MatchesWon;
    let mut sim = sim(true);
    player(&mut sim, "Alpha", 1, 4, true, false);
    player(&mut sim, "Botty", 2, 1, false, true);
    let find = |sim: &mut Sim, name: &str| {
        let w = sim.app.world_mut();
        w.query::<(Entity, &Name)>().iter(w).find(|(_, n)| n.as_str() == name).unwrap().0
    };
    let (alpha, botty) = (find(&mut sim, "Alpha"), find(&mut sim, "Botty"));
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    let wins = |sim: &mut Sim| {
        for _ in 0..3 {
            frame(sim, 0.05);
        }
        let w = sim.app.world_mut();
        let mut v: Vec<u32> = w.query::<&SessionWins>().iter(w).map(|s| s.0).collect();
        v.sort();
        v
    };
    assert!(wins(&mut sim).is_empty(), "no wins, no trophy");
    sim.app.world_mut().entity_mut(alpha).insert(MatchesWon(1));
    assert_eq!(wins(&mut sim), [1]);
    sim.app.world_mut().entity_mut(alpha).insert(MatchesWon(2));
    sim.app.world_mut().entity_mut(botty).insert(MatchesWon(1));
    assert_eq!(wins(&mut sim), [1, 2], "the new counts, the bot's too");
    let rows: Vec<(bool, u32)> = {
        let w = sim.app.world_mut();
        let mut r: Vec<(bool, u32)> = w.query::<&BoardRow>().iter(w).map(|r| (r.0.local, r.0.wins)).collect();
        r.sort();
        r
    };
    assert_eq!(rows, [(false, 1), (true, 2)], "the bot's row and yours");
}

/// The open Tab board covers the games' HUDs (`hud::HudCover`, "board"):
/// a game's root and a part outside one hidden, the shared crosshair told
/// to go (the cover is up), our own Signal HUD kept; closed, all back.
#[test]
fn the_board_covers_the_games_huds() {
    use chimera_client::client::hud::{BOARD_COVER, HudCover, HudPart, HudRoot, SignalHud};
    let mut sim = sim(true);
    player(&mut sim, "Alpha", 1, 4, true, false);
    let w = sim.app.world_mut();
    let root = w.spawn((Node::default(), HudRoot("test"))).id();
    let part = w.spawn((Node::default(), HudPart, Visibility::Inherited)).id();
    let ours = w.spawn((Node::default(), SignalHud, Visibility::Inherited)).id();
    for _ in 0..3 {
        frame(&mut sim, 1.0 / 60.0);
    }
    let vis = |sim: &Sim, e: Entity| *sim.app.world().get::<Visibility>(e).unwrap();
    assert!(!sim.app.world().resource::<HudCover>().is(BOARD_COVER));
    assert_ne!(vis(&sim, root), Visibility::Hidden);
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    frame(&mut sim, 0.0);
    frame(&mut sim, 0.0);
    assert!(sim.app.world().resource::<HudCover>().is(BOARD_COVER), "the cover is up (the shared crosshair reads it)");
    assert_eq!(vis(&sim, root), Visibility::Hidden, "a game's HUD root");
    assert_eq!(vis(&sim, part), Visibility::Hidden, "a part");
    assert_ne!(vis(&sim, ours), Visibility::Hidden, "our Signal HUD stays");
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = false;
    frame(&mut sim, 0.0);
    frame(&mut sim, 0.0);
    assert!(!sim.app.world().resource::<HudCover>().is(BOARD_COVER));
    assert_eq!(vis(&sim, part), Visibility::Inherited, "the part back");
    assert_eq!(vis(&sim, root), Visibility::Inherited, "the root back");
}

/// On a listen host its own player is the host (it has no network
/// character to say so: the local one while the process serves), so its
/// row says LOCAL for its ping, not a meter and "0"; a client's own row,
/// or single player, keeps the meter.
#[test]
fn a_listen_hosts_own_row_says_local() {
    for (role, local_ping) in [(chimera_core::core::NetRole::Server, true), (chimera_core::core::NetRole::Client, false)] {
        let mut sim = sim(true);
        sim.app.insert_resource(role);
        player(&mut sim, "Alpha", 1, 4, true, false);
        player(&mut sim, "Cee", 2, 6, false, false);
        sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
        for _ in 0..4 {
            frame(&mut sim, 0.05);
        }
        let w = sim.app.world_mut();
        let hosts: Vec<(bool, bool)> = w.query::<&BoardRow>().iter(w).map(|r| (r.0.local, r.0.host)).collect();
        assert!(hosts.contains(&(true, local_ping)) && hosts.contains(&(false, false)), "{role:?}: {hosts:?}");
        let texts = texts_under::<BoardContent>(&mut sim);
        assert_eq!(texts.iter().any(|t| t == "LOCAL"), local_ping, "{role:?}: {texts:?}");
    }
}

/// The chip's plate fits its line at every window size and UI scale (a
/// long leader's name, the text floor in a small window): laid out by
/// Bevy's own UI, the plate is never narrower than the line it holds, so
/// nothing is cropped (playtest7: "…ALPHA_9 05 | YOU 0…" cut both sides).
#[test]
fn the_chip_plate_fits_its_line_at_every_size() {
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
    // (physical width, height, scale factor)
    let sizes = [
        (1280, 720, 1.0),
        (1920, 1080, 1.0),
        (2560, 1440, 1.0),
        (1624, 1558, 1.0),
        (3440, 1440, 1.0),
        (1920, 1080, 1.25),
        (1920, 1080, 1.5),
        (1024, 600, 1.0),
    ];
    for (pw, ph, sf) in sizes {
        for leading in [false, true] {
            let mut sim = sim_with(true, |app| {
                app.init_resource::<Assets<Image>>()
                    .init_resource::<Assets<bevy::image::TextureAtlasLayout>>()
                    .init_resource::<ButtonInput<MouseButton>>()
                    .init_resource::<Touches>()
                    .init_resource::<bevy::picking::hover::HoverMap>()
                    .init_resource::<bevy::picking::pointer::PointerMap>()
                    .init_resource::<bevy::picking::events::PointerState>()
                    .add_message::<bevy::picking::pointer::PointerInput>()
                    .add_message::<bevy::picking::backend::PointerHits>()
                    .init_asset::<Font>();
                app.add_plugins((bevy::text::TextPlugin, bevy::ui::UiPlugin));
            });
            sim.app.world_mut().spawn((
                Camera2d,
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo { physical_size: UVec2::new(pw, ph), scale_factor: sf }),
                        ..default()
                    },
                    viewport: Some(Viewport { physical_size: UVec2::new(pw, ph), ..default() }),
                    ..default()
                },
            ));
            sim.app.insert_resource(UiUnits { height: ph as f32 / sf, scale_factor: sf, ..default() });
            // A long leader's name: wider than the look book's 21 cq.
            let (me, them) = if leading { (3, 1) } else { (1, 3) };
            player(&mut sim, "You", me, 4, true, false);
            player(&mut sim, "ALPHA_9_THE_VERY_LONG_NAMED", them, 9, false, true);
            for _ in 0..12 {
                frame(&mut sim, 0.05);
            }
            let w = sim.app.world_mut();
            let line = w.query_filtered::<&ComputedNode, With<ChipLine>>().single(w).expect("the chip's line").size.x;
            let plate = w.query_filtered::<&ComputedNode, With<RacePlate>>().single(w).unwrap().size.x;
            assert!(line > 0.0, "laid out");
            assert!(plate >= line + 1.0, "{pw}x{ph} @{sf} leading {leading}: the plate ({plate}) fits its line ({line})");
        }
    }
}

/// Holding Tab (the key bound to `+showscores`, a polled bind that never
/// runs through the console) opens the board, whatever rung (and so
/// whichever game's gun and HUD) the local player is on, and letting go
/// folds it; not while the chat line takes the keys (playtest7: the board
/// read only the console's `+showscores`, so Tab never opened it).
#[test]
fn holding_tab_opens_the_board_on_every_rung() {
    for rung in 0..4 {
        let mut sim = sim(true);
        sim.app.init_resource::<ButtonInput<KeyCode>>().init_resource::<ButtonInput<MouseButton>>();
        sim.app.add_systems(PreUpdate, poll_showscores);
        sim.app
            .world_mut()
            .resource_mut::<chimera_core::console::Console>()
            .binds
            .insert("tab".into(), "+showscores".into());
        player(&mut sim, "Alpha", rung, 4, true, false);
        player(&mut sim, "Botty", 2, 1, false, true);
        for _ in 0..3 {
            frame(&mut sim, 1.0 / 60.0);
        }
        assert!(visible::<ChipContent>(&mut sim) && !visible::<BoardContent>(&mut sim), "rung {rung}: the chip");
        sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Tab);
        for _ in 0..4 {
            frame(&mut sim, 0.05);
        }
        assert!(visible::<BoardContent>(&mut sim), "rung {rung}: Tab held opens the board");
        assert!(!sim.app.world().resource::<HeldActions>().showscores, "not through the console");
        sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::Tab);
        for _ in 0..4 {
            frame(&mut sim, 0.05);
        }
        assert!(visible::<ChipContent>(&mut sim) && !visible::<BoardContent>(&mut sim), "rung {rung}: let go, it folds");
        // Typing chat: Tab is the line's, not the board's.
        sim.app.insert_resource(chimera_client::client::chat::ChatInput { open: Some(false), ..default() });
        sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Tab);
        for _ in 0..4 {
            frame(&mut sim, 0.05);
        }
        assert!(!visible::<BoardContent>(&mut sim), "rung {rung}: not while typing");
    }
}

/// The scene for the board's Mute and talk marks: you and Ann (user 3) in
/// the minigame, the chat's feed, a captured cursor kept by the app's own
/// rule (`keep_cursor_free`), Tab bound as in play.
fn mute_sim() -> (Sim, Entity, Entity, Entity) {
    use chimera_client::client::{input, signal_chat::SignalChatPlugin};
    let mut sim = sim_with(true, |app| {
        app.add_plugins(SignalChatPlugin);
        app.add_systems(PostUpdate, input::keep_cursor_free);
    });
    sim.app.init_resource::<ButtonInput<KeyCode>>().init_resource::<ButtonInput<MouseButton>>();
    sim.app.add_systems(PreUpdate, poll_showscores);
    sim.app.world_mut().resource_mut::<chimera_core::console::Console>().binds.insert("tab".into(), "+showscores".into());
    player(&mut sim, "Me", 1, 4, true, false);
    let ann = sim
        .app
        .world_mut()
        .spawn((InMinigame, Name::new("Ann"), Rung(2), Score { kills: 2, deaths: 1 }, chimera_net::net::NetCharacter { owner: Some(3), name: "Ann".into() }))
        .id();
    let mut cursor = bevy::window::CursorOptions::default();
    chimera_client::client::input::capture_cursor(&mut cursor);
    let window = sim.app.world_mut().spawn(cursor).id();
    for _ in 0..3 {
        frame(&mut sim, 1.0 / 60.0);
    }
    (sim, ann, window, Entity::PLACEHOLDER)
}

fn hold_tab(sim: &mut Sim, down: bool) {
    let mut keys = sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    if down {
        keys.press(KeyCode::Tab);
    } else {
        keys.release(KeyCode::Tab);
    }
    for _ in 0..4 {
        frame(sim, 0.05);
    }
}

fn grabbed(sim: &Sim, window: Entity) -> bool {
    chimera_client::client::input::cursor_grabbed(sim.app.world().get::<bevy::window::CursorOptions>(window).unwrap())
}

fn say(sim: &mut Sim, from: Entity, text: &str) {
    use chimera_client::client::chat::{ChatEntry, ChatLine, LineKind};
    sim.app.world_mut().write_message(ChatEntry { kind: LineKind::All, sender: Some(from), name: Some("Ann".into()), team: None, text: text.into(), classic: Some(ChatLine::plain(text)), sound: None });
    frame(sim, 0.1);
}

fn shown_lines(sim: &Sim) -> Vec<String> {
    use chimera_client::client::{chat::{ChatFilter, ChatMutes}, signal_chat::SignalChatLog};
    let w = sim.app.world();
    w.resource::<SignalChatLog>().shown(w.resource::<ChatMutes>(), *w.resource::<ChatFilter>()).map(|l| l.text.clone()).collect()
}

/// Backlog "Chat (UGG UI M10)": the Tab board is held open with the cursor
/// captured; a right click frees it (`BoardPick`) and each other player's
/// row has a Mute, the chat's `ignore`: their lines go (past ones too), the
/// row shows it, a second press shows them again, and letting go of Tab
/// grabs the cursor again.
#[test]
fn a_right_click_on_the_board_frees_the_cursor_for_mute() {
    use chimera_client::client::{
        chat::ChatMutes,
        race_chip::{BoardMute, BoardMuted, BoardPick},
        widgets::Interaction,
    };
    let (mut sim, ann, window, _) = mute_sim();
    say(&mut sim, ann, "gg ez");
    assert_eq!(shown_lines(&sim), ["gg ez"]);
    hold_tab(&mut sim, true);
    assert!(visible::<BoardContent>(&mut sim) && grabbed(&sim, window), "Tab: the board opens, the cursor stays captured");
    assert_eq!(count::<BoardMute>(&mut sim), 0, "no buttons while it is captured");
    // The right click.
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
    frame(&mut sim, 0.05);
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Right);
    for _ in 0..3 {
        frame(&mut sim, 0.05);
    }
    assert!(sim.app.world().resource::<BoardPick>().0 && !grabbed(&sim, window), "the right click frees the cursor");
    let buttons: Vec<(Entity, BoardMute)> = {
        let w = sim.app.world_mut();
        w.query::<(Entity, &BoardMute)>().iter(w).map(|(e, m)| (e, m.clone())).collect()
    };
    assert_eq!(buttons.iter().map(|(_, m)| (m.owner, m.name.as_str())).collect::<Vec<_>>(), [(Some(3), "Ann")], "Mute on Ann's row, none on yours");
    let press = |sim: &mut Sim, b: Entity| {
        *sim.app.world_mut().get_mut::<Interaction>(b).unwrap() = Interaction::Pressed;
        frame(sim, 0.05);
        for _ in 0..2 {
            frame(sim, 0.05);
        }
    };
    press(&mut sim, buttons[0].0);
    assert!(sim.app.world().resource::<ChatMutes>().muted(Some(3), "Ann"), "the same ignore the chat uses");
    assert_eq!(count::<BoardMuted>(&mut sim), 1, "the row shows it");
    assert!(shown_lines(&sim).is_empty(), "her earlier line is gone");
    say(&mut sim, ann, "still here?");
    assert!(shown_lines(&sim).is_empty(), "and her new ones don't show");
    // The button toggles.
    let again = {
        let w = sim.app.world_mut();
        w.query::<(Entity, &BoardMute)>().iter(w).next().unwrap().0
    };
    press(&mut sim, again);
    assert!(!sim.app.world().resource::<ChatMutes>().muted(Some(3), "Ann"));
    assert_eq!(count::<BoardMuted>(&mut sim), 0);
    assert_eq!(shown_lines(&sim), ["gg ez", "still here?"], "unmuted: her lines show again");
    // Tab let go: the board folds, the buttons go, the cursor is grabbed.
    hold_tab(&mut sim, false);
    assert!(!sim.app.world().resource::<BoardPick>().0 && grabbed(&sim, window), "the cursor is captured again");
    assert_eq!(count::<BoardMute>(&mut sim), 0);
}

/// The talk indicator (backlog "Who is talking"): a meter by a player's
/// name on the Tab board while their voice comes in (`net::voice::
/// VoiceActivity`, the transport's own state), yours while you send; it
/// clears when they stop. A bot has none.
#[test]
fn the_board_marks_who_is_talking_and_clears_it() {
    use chimera_client::client::talk::{TalkMark, Talking};
    use chimera_net::net::voice::VoiceActivity;
    let (mut sim, ann, _, _) = mute_sim();
    player(&mut sim, "Rook", 0, 0, false, true);
    hold_tab(&mut sim, true);
    let lit = |sim: &mut Sim| -> Vec<Option<u64>> {
        let w = sim.app.world_mut();
        w.query::<(&TalkMark, &Visibility)>().iter(w).filter(|(_, v)| **v != Visibility::Hidden).map(|(m, _)| m.owner).collect()
    };
    assert_eq!(count::<TalkMark>(&mut sim), 2, "a mark for each player, none for the bot");
    assert!(lit(&mut sim).is_empty(), "nobody talks");
    // Ann's voice arrives.
    let now = sim.app.world().resource::<Time<Real>>().elapsed_secs_f64();
    let mut activity = VoiceActivity::default();
    activity.heard.insert(ann, now);
    sim.app.insert_resource(activity);
    frame(&mut sim, 0.0);
    assert_eq!(lit(&mut sim), [Some(3)], "Ann's meter shows");
    // Silence: her last frame is old.
    sim.app.world_mut().resource_mut::<VoiceActivity>().heard.insert(ann, now - 5.0);
    frame(&mut sim, 0.0);
    assert!(lit(&mut sim).is_empty(), "it clears");
    // You talk.
    sim.app.world_mut().resource_mut::<VoiceActivity>().local = true;
    frame(&mut sim, 0.0);
    assert_eq!(lit(&mut sim), [None], "yours (the local player has no user id here)");
    assert_eq!(*sim.app.world().resource::<Talking>(), Talking { users: Default::default(), local: true });
    // The hook alone: with no voice transport a system sets `Talking`.
    sim.app.world_mut().remove_resource::<VoiceActivity>();
    *sim.app.world_mut().resource_mut::<Talking>() = Talking { users: [3].into(), local: false };
    frame(&mut sim, 0.0);
    assert_eq!(lit(&mut sim), [Some(3)]);
}
