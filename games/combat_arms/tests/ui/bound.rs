//! The HUD panels, scoreboard and loading screen bound by name to the
//! game's layouts (`ui::hud::bound`, `ui::scoreboard::BoundBoard`,
//! `ui::loading`; ui_system.md L1, H, D; hud_crosshair.md D5-D7), on
//! small layouts written here with the controls the bindings look up
//! (the install's own: `heavy::install_ui`).

use bevy::math::{IVec2, Vec2};
use chimera_client::client::canvas::{HAlign, Scale, VAlign, WidgetId};
use combat_arms::{
    game::rules::{ScoreOrder, scoreboard::ScoreRow},
    ui::{
        CaUi,
        hud::{
            FeedLineView,
            bound::{self, ActionPanel, FeedArt, FeedPanel, ScorePanel, child_for},
            model::{Badge, Kill, KillIcon, SCORE_ALPHA, SCORE_PLAIN, feed_line, score_panel},
            score::ScorePanelView,
        },
        layout::{Draw, GaugeLook, ImageLook, Kind, Layout, TextLook},
        loading::{self, CaLoading},
        scoreboard::{BoundBoard, CaScoreboard, RoundResult, ping_icon},
    },
};

/// Our UI data with stand-in textures of these names.
fn ui_with(names: &[&str]) -> CaUi {
    let mut ui = CaUi::default();
    for n in names {
        ui.stand_ins.0.insert(combat_arms::ui::install::texture_key(n), (IVec2::new(64, 32), [255; 4]));
    }
    ui
}

fn text(font: &str, element: &str, colour: [u8; 4]) -> Kind {
    Kind::Text(TextLook {
        font: font.into(),
        element: element.into(),
        h: HAlign::Default,
        v: VAlign::Default,
        colour,
        ..Default::default()
    })
}

fn image(texture: &str) -> Kind {
    Kind::Image(ImageLook {
        texture: texture.into(),
        ..Default::default()
    })
}

fn window() -> Kind {
    Kind::Window(ImageLook {
        texture: "UI_SCR_Common\\blank.dtx".into(),
        ..Default::default()
    })
}

fn gauge(range: i32) -> Kind {
    Kind::Gauge(GaugeLook {
        frame: "frame".into(),
        bar: "bar".into(),
        bar_at: IVec2::ONE,
        max_range: range,
        fill: 0.0,
        bar_height: 8,
            ..Default::default()
    })
}

fn texts(draws: &[Draw]) -> Vec<(String, IVec2, [u8; 4])> {
    draws
        .iter()
        .filter_map(|d| match d {
            Draw::Text(t) => Some((t.lines.iter().map(|l| l.1.clone()).collect(), t.reference_at, t.colour)),
            _ => None,
        })
        .collect()
}

fn quads(draws: &[Draw]) -> Vec<(WidgetId, String, [f64; 2], [f64; 2])> {
    draws
        .iter()
        .filter_map(|d| match d {
            Draw::Quad(id, q) => Some((*id, q.texture.clone(), q.at, q.size)),
            _ => None,
        })
        .collect()
}

/// The main HUD layout's kill feed panel: `KillDeath` at (0, 35) with its
/// line template at (0, 10).
fn kill_death() -> Layout {
    let mut l = Layout::new();
    let panel = l.add(0, "KillDeath", IVec2::new(0, 35), IVec2::new(280, 115), window());
    let line = l.add(panel, "KillLogLine1", IVec2::new(0, 10), IVec2::new(350, 20), window());
    l.add(line, "Name1", IVec2::new(5, 3), IVec2::ZERO, text("bold 12", "XXXXXXXXXXXX", [124, 190, 221, 150]));
    l.add(line, "weapon", IVec2::new(105, 0), IVec2::new(64, 20), image("sample_weapon"));
    l.add(line, "icon", IVec2::new(169, 0), IVec2::new(32, 20), image("sample_icon"));
    l.add(line, "Name2", IVec2::new(201, 3), IVec2::ZERO, text("bold 12", "XXXXXXXXXXXX", [218, 115, 115, 150]));
    l.add(line, "SubInfo", IVec2::new(201, 3), IVec2::ZERO, text("bold 12", "XXXXXXXXXXXX", [255, 255, 255, 150]));
    l
}

/// D5 bound: each kill line is a clone of the template, 20 ref px apart
/// from y = 45; the names at the running x in the spec's colours, the
/// weapon icon and badges the given textures (the second and later
/// badges in a copy of `icon`), the sample texts gone.
#[test]
fn kill_feed_binds_by_name() {
    let ui = ui_with(&[
        "icon_combat_arms:m4a1",
        "badge_Headshot",
        "badge_Groin",
        "badge_Revenge",
        "suicide",
        "sample_weapon",
        "sample_icon",
    ]);
    let panel = FeedPanel::new(&kill_death()).expect("KillDeath and its line");
    let width = |s: &str| s.chars().count() as f32 * 7.0;
    let kills = [
        Kill {
            killer: Some(("Alpha One".into(), true)),
            victim: ("Bravo Two".into(), false),
            icon: KillIcon::Weapon(Some("combat_arms:m4a1".into())),
            badges: vec![Badge::Headshot, Badge::Revenge, Badge::Groin],
        },
        Kill {
            killer: None,
            victim: ("Bravo Two".into(), false),
            icon: KillIcon::Suicide,
            badges: vec![],
        },
    ];
    let lines: Vec<FeedLineView> = kills
        .iter()
        .enumerate()
        .map(|(i, k)| FeedLineView {
            y: 45.0 + 20.0 * i as f32,
            kill: k.clone(),
            items: feed_line(k, width),
        })
        .collect();
    let art = FeedArt {
        icon: &|i| match i {
            KillIcon::Weapon(Some(id)) => Some(format!("icon_{id}")),
            KillIcon::Suicide => Some("suicide".into()),
            _ => None,
        },
        badge: &|b| Some(format!("badge_{b:?}")),
    };
    let l = panel.build(&lines, &art, Scale::ONE);
    let draws = l.draw(&ui.ctx());
    let t = texts(&draws);
    eprintln!("{t:?}");
    let at = |s: &str, y: i32| t.iter().find(|(x, at, _)| x == s && at.y == y).cloned();
    let killer = at("Alpha One", 48).expect("the killer on line 0 (45 + 3)");
    assert_eq!(killer.1.x, 5);
    assert_eq!(killer.2, [124, 190, 221, 150]);
    let victim = at("Bravo Two", 48).expect("the victim on line 0");
    // killer 9 chars x 7 + icon 64 + three badges x 32.
    assert_eq!(victim.1.x, 5 + 63 + 64 + 96);
    assert_eq!(victim.2, [218, 115, 115, 150]);
    // No killer: the victim twice, in the victim's colour, on line 1.
    assert_eq!(t.iter().filter(|(s, at, _)| s == "Bravo Two" && at.y == 68).count(), 2);
    assert!(t.iter().all(|(s, ..)| !s.starts_with('X')), "samples hidden");
    let q = quads(&draws);
    eprintln!("{q:?}");
    let quad = |tex: &str| q.iter().find(|x| x.1 == tex).cloned();
    let weapon = quad("icon_combat_arms:m4a1").expect("the weapon icon");
    assert_eq!(weapon.2, [68.0, 45.0]);
    let first = quad("badge_Headshot").expect("the first badge in `icon`");
    assert_eq!(first.2, [132.0, 45.0]);
    // Later badges into the second slot: only the last one stays (quirk).
    let second = quad("badge_Groin").expect("the second slot");
    // At the last badge's x (Corrections 6): the first's + 64.
    assert_eq!(second.2, [196.0, 45.0]);
    assert!(quad("badge_Revenge").is_none());
    assert!(quad("suicide").is_some_and(|s| s.2[1] == 65.0));
    assert!(quad("sample_weapon").is_none() && quad("sample_icon").is_none());
}

/// The mode indicator layout with TDM's `Eli` child (D6's places) and a
/// round mode's (`s&d`, its timer strip).
fn indicator() -> Layout {
    let mut l = Layout::new();
    let ind = l.add(0, "indicator", IVec2::new(342, 0), IVec2::new(340, 115), window());
    for name in ["Eli", "s&d"] {
        let c = l.add(ind, name, IVec2::ZERO, IVec2::new(340, 115), window());
        l.tree.get_mut(c).show = false;
        let bg = l.add(c, "bg_mode", IVec2::new(5, 3), IVec2::new(220, 16), image("bg02"));
        l.add(bg, "mode_text", IVec2::new(100, 100), IVec2::ZERO, text("number 14", "Capture the Flag", [218; 4]));
        let bg = l.add(c, "bg_goal", IVec2::new(230, 3), IVec2::new(105, 16), image("bg02"));
        l.add(bg, "goal_text", IVec2::new(100, 100), IVec2::ZERO, text("number 14", "999 Rounds", [218; 4]));
        l.add(c, "bg_up", IVec2::new(2, 18), IVec2::new(336, 40), image("bg_up"));
        let down = l.add(c, "bg_down", IVec2::new(2, 26), IVec2::new(336, 40), image("bg_down"));
        l.tree.get_mut(down).show = false;
        let p = l.add(c, "toptitle_parent", IVec2::new(5, 23), IVec2::new(90, 18), window());
        l.add(p, "top_title", IVec2::new(10, 0), IVec2::ZERO, text("bold 18", "$hud_indicator_alpha", [170, 170, 170, 255]));
        let p = l.add(c, "bottomtitle_parent", IVec2::new(5, 43), IVec2::new(90, 18), window());
        l.add(p, "bottom_title", IVec2::new(10, 0), IVec2::ZERO, text("bold 18", "$hud_indicator_bravo", [170, 170, 170, 255]));
        let p = l.add(c, "topround_parent", IVec2::new(270, 23), IVec2::new(65, 18), window());
        l.add(p, "top_round", IVec2::new(10, 0), IVec2::ZERO, text("number 18", "999", [170, 170, 170, 255]));
        let p = l.add(c, "bottomround_parent", IVec2::new(270, 43), IVec2::new(65, 18), window());
        l.add(p, "bottom_round", IVec2::new(10, 0), IVec2::ZERO, text("number 18", "999", [170, 170, 170, 255]));
        l.add(c, "alpha_gauge", IVec2::new(106, 32), IVec2::new(157, 16), gauge(155));
        l.add(c, "bravo_gauge", IVec2::new(106, 43), IVec2::new(157, 16), gauge(155));
        if name == "s&d" {
            let r = l.add(c, "RoundTime", IVec2::new(2, 63), IVec2::new(336, 17), image("blank"));
            l.add(r, "round_timer", IVec2::new(100, -3), IVec2::ZERO, text("number 20", "99 : 99 : 99", [255; 4]));
            let b = l.add(c, "BombTime", IVec2::new(2, 63), IVec2::new(336, 17), image("blank"));
            l.add(b, "bomb_timer", IVec2::new(100, -3), IVec2::ZERO, text("number 20", "99 : 99", [255; 4]));
        }
    }
    l
}

/// D6, D7 bound: TDM shows the indicator's `Eli` child only, its mode
/// and goal texts, the counters in their colours (your team's in its
/// highlight colour, its row's highlight shown), the bars as wide as D6
/// says (⌊gauge · 155 / 100⌋) and no timer; a round mode (`s&d`) its
/// timer in `RoundTime`.
#[test]
fn score_panel_binds_by_name() {
    let ui = ui_with(&["bg_up", "bg_down", "blank", "bg02"]);
    let panel = ScorePanel::new(&indicator()).unwrap();
    assert_eq!(child_for("TDM"), "Eli");
    let (rows, highlight) = score_panel([25, 10], 50, Some(0));
    let view = ScorePanelView {
        mode_key: "TDM".into(),
        mode_name: "Team Deathmatch".into(),
        goal: "50 Kills".into(),
        rows,
        labels: ["Alpha".into(), "Bravo".into()],
        highlight,
        titles_are_counts: false,
        dividers: 0,
    };
    let l = panel.build("TDM", &view, (None, None), Scale::ONE).unwrap();
    let draws = l.draw(&ui.ctx());
    let t = texts(&draws);
    eprintln!("{t:?}");
    let find = |s: &str| t.iter().find(|x| x.0 == s).cloned();
    assert!(find("Team Deathmatch").is_some() && find("50 Kills").is_some());
    assert_eq!(find("25").unwrap().2, SCORE_ALPHA);
    assert_eq!(find("10").unwrap().2, SCORE_PLAIN);
    // The labels are the layout's string ids (unresolved here: the raw
    // id, R2).
    assert!(find("$hud_indicator_alpha").is_some());
    assert!(find("999").is_none() && find("Capture the Flag").is_none());
    let q = quads(&draws);
    assert!(q.iter().any(|x| x.1 == "bg_up") && !q.iter().any(|x| x.1 == "bg_down"));
    // The bars: 25 / 50 -> 77 px; 10 / 50 -> 31 px.
    let bars: Vec<f64> = q.iter().filter(|x| x.1 == "bar").map(|x| x.3[0]).collect();
    assert_eq!(bars, [77.0, 31.0]);
    // Only one child drawn: the panel's quads all from it (no s&d timer).
    assert!(!q.iter().any(|x| x.1 == "blank"));
    let l = panel.build("TMM", &view, (Some("01 : 59"), None), Scale::ONE).unwrap();
    let t = texts(&l.draw(&ui.ctx()));
    assert!(t.iter().any(|x| x.0 == "01 : 59"), "{t:?}");
    // hud_crosshair.md 92, 93 bound: a plant shows the fuse in `BombTime`
    // (the round timer hidden); a round mode's rows carry the living
    // counts over the layout's labels, and its bars the dividers.
    let round = ScorePanelView {
        mode_key: "TMM".into(),
        labels: ["5".into(), "3".into()],
        titles_are_counts: true,
        dividers: 7,
        ..view.clone()
    };
    let l = panel.build("TMM", &round, (None, Some("00 : 40")), Scale::ONE).unwrap();
    let draws = l.draw(&ui.ctx());
    let t = texts(&draws);
    assert!(t.iter().any(|x| x.0 == "00 : 40") && !t.iter().any(|x| x.0 == "01 : 59"), "{t:?}");
    assert!(t.iter().any(|x| x.0 == "5") && t.iter().any(|x| x.0 == "3"), "{t:?}");
    assert!(!t.iter().any(|x| x.0.starts_with("$hud_indicator")), "counts, not labels");
    let marks: Vec<[f64; 2]> = quads(&draws).into_iter().filter(|x| x.1 == bound::DIVIDER).map(|x| x.2).collect();
    assert_eq!(marks.len(), 12, "G - 1 a bar");
    // The alpha bar's frame at (342 + 106, 32): the marks at 449 + 22 ...
    let alpha: Vec<f64> = marks.iter().filter(|m| m[1] == 34.0).map(|m| m[0]).collect();
    assert_eq!(alpha, [471.0, 493.0, 515.0, 537.0, 559.0, 581.0]);
}

/// ui_system.md 28, game_modes.md 90: the mode indicator's child per mode.
#[test]
fn ui28_gm90_indicator_children() {
    for (key, child) in [
        ("TDM", "Eli"),
        ("TSV", "Eli_pro"),
        ("TMM", "s&d"),
        ("SBT", "s&d"),
        ("CTF", "CTF"),
        ("FFA", "Indicator_OMA"),
        ("LMS", "LMS"),
        ("BNT", "Indicator_SPY"),
        ("QRT", "QRT"),
        ("HQM", "HQT"),
        ("SQM", "Squad"),
        ("SQM2", "HG2"),
        ("VTDM", "VIP_Eli"),
        ("ESC", "VIP_Esct"),
        ("VWRM", "WeaponRace"),
        ("ESL", "ESL"),
        ("HG", "HungerGame"),
        ("SFM", "Eli"),
        ("FRT", "FRT"),
        ("FRT_IShip", "FRT_IShip"),
    ] {
        assert_eq!(child_for(key), child, "{key}");
    }
}

/// The action panel: the pickup prompt in `ActionDesc`'s `Message`, the
/// action gauge's title and fullness in `ActionGauge`; neither: nothing.
#[test]
fn action_panel_binds_by_name() {
    let ui = ui_with(&["plant_icon", "sample_icon", "blank", "bg"]);
    let mut src = Layout::new();
    let action = src.add(0, "Action", IVec2::new(0, 356), IVec2::new(1024, 85), window());
    src.tree.get_mut(action).show = false;
    let desc = src.add(action, "ActionDesc", IVec2::new(-7, 50), IVec2::new(1044, 40), image("blank"));
    src.add(desc, "Message", IVec2::new(100, 175), IVec2::ZERO, text("bold 14", "Defuse the BOMB", [255, 255, 255, 220]));
    let g = src.add(action, "ActionGauge", IVec2::new(385, 3), IVec2::new(255, 57), image("bg"));
    src.add(g, "Gauge", IVec2::new(42, 44), IVec2::new(184, 16), gauge(182));
    src.add(g, "ActionTitle", IVec2::new(60, 26), IVec2::ZERO, text("bold 14", "Bomb Planting", [255; 4]));
    src.add(g, "ActionCount", IVec2::new(60, 14), IVec2::ZERO, text("number 18", "99 / 99", [255; 4]));
    src.add(g, "ActionIcon", IVec2::ZERO, IVec2::new(64, 64), image("sample_icon"));
    let panel = ActionPanel::new(&src).unwrap();
    assert!(panel.build(None, None, Scale::ONE, &ui.fonts).is_none());
    let prompt = ui.strings.resolve("IDS_HUD_ITEMGET_MSG");
    let l = panel.build(Some(&prompt), None, Scale::ONE, &ui.fonts).unwrap();
    let t = texts(&l.draw(&ui.ctx()));
    assert_eq!(t.len(), 1, "{t:?}");
    assert_eq!(t[0].0, prompt);
    // hud_crosshair.md 95: `Message` centred on `ActionDesc` by its
    // reference width: (x + ⌊w_desc / 2⌋) − ⌊w / 2⌋.
    let w = combat_arms::ui::layout::text::measure(&prompt, ui.fonts.get("bold 14").unwrap(), 0, Scale::ONE)
        .reference
        .x;
    let message = l.find("Message").unwrap();
    assert_eq!(bound::at(&l, message).x as i32, -7 + 1044 / 2 - w / 2);
    // hud_crosshair.md 97: the gauge in whole percent (W5's floor of
    // p · MaxRange / 100).
    let l = panel.build(None, Some(("Planting", 49, Some("plant_icon"))), Scale::ONE, &ui.fonts).unwrap();
    let draws = l.draw(&ui.ctx());
    let t = texts(&draws);
    assert_eq!(t.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["Planting"]);
    let q = quads(&draws);
    assert!(q.iter().any(|x| x.1 == "plant_icon"));
    assert_eq!(q.iter().find(|x| x.1 == "bar").unwrap().3[0], 89.0, "49 % of 182");
}

/// The team list layout (ui_system.md's places): `Team_UserList` at (93,
/// 161), the lists at (71, 31) and (71, 231) in it, a row template per
/// list.
fn team_list() -> Layout {
    let mut l = Layout::new();
    let p = l.add(0, "Team_UserList", IVec2::new(93, 161), IVec2::new(838, 432), image("bg_list"));
    for (list, row, line) in [("UserList_UP", "User_Column", "listAlpha_"), ("UserList_Down", "DUser_Column", "listBravo_")] {
        let y = if list == "UserList_UP" { 31 } else { 231 };
        let li = l.add(p, list, IVec2::new(71, y), IVec2::new(766, 200), window());
        let c = l.add(li, row, IVec2::ZERO, IVec2::new(766, 25), window());
        let ln = l.add(c, "Line", IVec2::ZERO, IVec2::new(766, 25), image(line));
        let r = l.add(ln, "Root", IVec2::new(43, 5), IVec2::new(142, 16), window());
        l.add(r, "User_Name", IVec2::new(25, 2), IVec2::ZERO, text("Light 13", "WWWWWWWWWWWW", [210, 210, 210, 200]));
        l.add(r, "User_LevelImg", IVec2::new(1, -4), IVec2::new(32, 22), image("rank"));
        for (root, name, x) in [("Root1", "User_Kill", 356), ("Root11", "User_Death", 423), ("Root111", "User_Assist", 567), ("Root11111", "User_Mission", 500)] {
            let r = l.add(ln, root, IVec2::new(x, 5), IVec2::new(80, 16), window());
            l.add(r, name, IVec2::new(30, 2), IVec2::ZERO, text("Light 13", "999", [210, 210, 210, 200]));
        }
        l.add(ln, "User_Ping", IVec2::new(724, 5), IVec2::new(32, 16), image("ping"));
    }
    l
}

fn row(name: &str, team: u8, kills: u32) -> ScoreRow {
    ScoreRow {
        player: bevy::prelude::Entity::PLACEHOLDER,
        name: name.into(),
        team,
        kills,
        deaths: 1,
        assists: 0,
        captures: 2,
        returns: 0,
        detonations: 0,
        defuses: 0,
        mission_points: 0,
        hunted_ms: 0,
        ping: Some(120),
    }
}

/// ui_system.md test case 18 on the game's own team list: five Alpha
/// players' rows at y = 192, 217, 242, 267, 292 (ref), Bravo's from 392;
/// each row's name, kills, deaths, assists and ping icon bound, the
/// mode's column only where the mode has one; the round's result banner
/// in the common frame's `Game_Result` while it shows.
#[test]
fn scoreboard_binds_by_name() {
    let ui = ui_with(&["bg_list", "listAlpha_", "listBravo_", "rank", &ping_icon(Some(120)), "sample_result", "lose", "win", "blank"]);
    let mut frame = Layout::new();
    let f = frame.add(0, "Result_Normal", IVec2::ZERO, IVec2::new(1024, 768), window());
    let r1 = frame.add(f, "Root1", IVec2::new(0, 30), IVec2::new(1024, 64), image("blank"));
    frame.add(r1, "Game_Result", IVec2::ZERO, IVec2::new(1024, 64), image("sample_result"));
    let bound = BoundBoard {
        frame: Some(frame),
        team: Some(team_list()),
        no_team: None,
        banners: [Some("win".into()), Some("lose".into()), Some("draw".into()), None, None, None, None],
    };
    let board = CaScoreboard {
        shown: true,
        lists: [
            (0..5).map(|i| row(&format!("A{i}"), 1, 10 - i)).collect(),
            vec![row("B0", 2, 3)],
        ],
        scores: [40, 3],
        headers: ["Alpha".into(), "Bravo".into()],
        order: Some(ScoreOrder::Kills),
        ffa: false,
        mode: None,
        result: None,
        lists_shown: true,
        sound: None,
    };
    let l = bound.build(&board, Scale::ONE).unwrap();
    let draws = l.draw(&ui.ctx());
    let t = texts(&draws);
    let names: Vec<(String, i32)> = t.iter().filter(|x| x.0.starts_with('A') || x.0.starts_with('B')).map(|x| (x.0.clone(), x.1.y)).collect();
    // Rows at 192 + 25 k; the name 5 + 2 below the row's top.
    assert_eq!(
        names,
        [("A0".into(), 199), ("A1".into(), 224), ("A2".into(), 249), ("A3".into(), 274), ("A4".into(), 299), ("B0".into(), 399)]
    );
    assert!(t.iter().any(|x| x.0 == "10") && t.iter().all(|x| x.0 != "999" && !x.0.starts_with('W')), "{t:?}");
    let q = quads(&draws);
    assert_eq!(q.iter().filter(|x| x.1 == ping_icon(Some(120))).count(), 6);
    assert!(!q.iter().any(|x| x.1 == "sample_result" || x.1 == "rank"));
    // Captures: the mode's column shows.
    let ctf = CaScoreboard {
        order: Some(ScoreOrder::Captures),
        result: Some(RoundResult::Lose),
        ..board.clone()
    };
    let l = bound.build(&ctf, Scale::ONE).unwrap();
    let draws = l.draw(&ui.ctx());
    assert!(texts(&draws).iter().filter(|x| x.0 == "2").count() >= 6);
    assert!(quads(&draws).iter().any(|x| x.1 == "lose"));
    // ui_system.md 24: a single-round mode's result: the banner alone.
    let alone = CaScoreboard {
        lists_shown: false,
        result: Some(RoundResult::Win),
        ..board.clone()
    };
    let draws = bound.build(&alone, Scale::ONE).unwrap().draw(&ui.ctx());
    assert!(quads(&draws).iter().any(|x| x.1 == "win"));
    assert!(texts(&draws).iter().all(|x| !x.0.starts_with('A')), "no lists");
    let _ = Vec2::ZERO;
}

/// The loading screen (H): while joining, the map's name in `mapName`,
/// the gauge by the join's stage; nothing once in the game.
#[test]
fn loading_screen_binds_by_name() {
    let ui = ui_with(&["map_pic", "sample_map", "gold"]);
    let mut src = Layout::new();
    let pic = src.add(0, "LevelPic", IVec2::ZERO, IVec2::new(1024, 768), window());
    src.add(pic, "MapImage", IVec2::ZERO, IVec2::new(1024, 768), image("sample_map"));
    let op = src.add(0, "Operation", IVec2::ZERO, IVec2::new(1024, 768), window());
    src.add(op, "mapName", IVec2::new(37, 47), IVec2::ZERO, text("bold 40", "ROADKILL", [255; 4]));
    src.add(op, "operation_description", IVec2::new(51, 95), IVec2::ZERO, text("Light 14", "The physiological", [255; 4]));
    let bottom = src.add(0, "Bottom_Info", IVec2::new(10, 570), IVec2::new(1005, 190), image("gold"));
    let p = src.add(bottom, "LoadingProgress", IVec2::new(0, 9), IVec2::new(1004, 23), window());
    src.add(p, "LoadingGauge", IVec2::new(10, 0), IVec2::new(985, 38), gauge(980));
    assert!(loading::build(&src, &CaLoading::default(), None, &ui.strings, Scale::ONE).is_none());
    let state = CaLoading {
        label: Some("combat_arms:warehouse".into()),
        progress: 0.8,
        mode: None,
    };
    assert_eq!(loading::map_image("combat_arms:warehouse").as_deref(), Some("ui_scr_loading/maploading/mapimage/warehouse.dtx"));
    assert_eq!(loading::map_image("127.0.0.1:27042"), None);
    let l = loading::build(&src, &state, Some("map_pic"), &ui.strings, Scale::ONE).unwrap();
    let draws = l.draw(&ui.ctx());
    let t = texts(&draws);
    assert_eq!(t.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["WAREHOUSE"]);
    let q = quads(&draws);
    assert!(q.iter().any(|x| x.1 == "map_pic"));
    assert_eq!(q.iter().find(|x| x.1 == "bar").unwrap().3[0], 784.0);
}

/// ui_system.md 29: the loading gauge on the real clock: 0.2 % a ms to
/// 15 % while the early states last (T = 300 ms), then 55 % and 15 / T %
/// a ms to 100 %.
#[test]
fn ui29_loading_gauge() {
    use loading::{LoadingGauge, LoadingPart};
    let mut g = LoadingGauge::default();
    g.advance(75.0, LoadingPart::Early);
    assert!((g.percent - 15.0).abs() < 1e-9, "15 % from 75 ms");
    g.advance(225.0, LoadingPart::Early);
    assert_eq!(g.percent, 15.0);
    g.advance(500.0, LoadingPart::Level);
    assert_eq!(g.percent, 15.0, "it holds while the level is set up");
    g.advance(16.0, LoadingPart::Entering);
    assert_eq!(g.percent, 55.0);
    g.advance(450.0, LoadingPart::Entering);
    assert!((g.percent - 77.5).abs() < 1e-9, "0.05 % a ms");
    g.advance(450.0, LoadingPart::Entering);
    assert!((g.percent - 100.0).abs() < 1e-9, "100 % 900 ms after");
    for (p, bar) in [(15u32, 90), (55, 330), (100, 600)] {
        assert_eq!(combat_arms::ui::hud::model::gauge_bar(p, 600), bar);
    }
}

/// ui_system.md 30: the loading picture falls back to the map's team
/// deathmatch one; an official room's mode line.
#[test]
fn ui30_loading_picture_and_mode_line() {
    let picture = |map: &str, mode: &str| Some(format!("{map}_{mode}.dtx"));
    let exists = |t: &str| t == "m_TDM.dtx";
    assert_eq!(loading::loading_picture("m", "CTF", picture, exists).as_deref(), Some("m_TDM.dtx"));
    assert_eq!(loading::loading_picture("m", "CTF", picture, |_| true).as_deref(), Some("m_CTF.dtx"));
    assert_eq!(loading::mode_line("Capture the Flag", Some("Official")), "Official Capture the Flag");
    assert_eq!(loading::mode_line("Capture the Flag", None), "Capture the Flag");
}

/// ui_system.md 31: textures are drawn by their own names in every
/// language (no per-language texture map applies).
#[test]
fn ui31_no_per_language_textures() {
    let mut ui = ui_with(&["x"]);
    ui.strings = combat_arms::ui::layout::Strings::parse("x = \"y\"\n");
    let mut l = Layout::new();
    l.add(0, "pic", IVec2::ZERO, IVec2::new(64, 32), image("x"));
    let q = quads(&l.draw(&ui.ctx()));
    assert_eq!(q.iter().map(|x| x.1.as_str()).collect::<Vec<_>>(), ["x"]);
}
