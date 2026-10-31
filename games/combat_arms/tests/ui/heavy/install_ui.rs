//! The install's own UI data through the UI system's readers
//! (specs/combat_arms/ui_system.md L1 and R1): every layout in XML.rez
//! reads as a widget tree (`layout::xml::read`), the language pack's
//! string tables (Language_Pack.rez) as string tables
//! (`layout::Strings::parse`). Both archives are V1-keyed: skipped
//! without the key (or without an install in chimera.local.toml).

use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::{
    game::{GAME, INFO, mount},
    ui::{
        install,
        layout::{Strings, xml},
    },
};

fn install() -> Option<Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    if !mount::can_read("V1") {
        eprintln!("skipping: no V1 key in chimera.local.toml");
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

/// The files of the archive layer named `archive` (its file name, any
/// case); None, with a note, when the asset store stands in for the
/// install (its files aren't grouped by archive).
fn files_of(mount: &Mount, archive: &str) -> Option<Vec<String>> {
    if INFO.served_alone() {
        eprintln!("skipping: lists the install's {archive} (served from the asset store here)");
        return None;
    }
    let layer = mount
        .layers()
        .find(|l| l.name().to_lowercase().ends_with(&archive.to_lowercase()))
        .unwrap_or_else(|| panic!("no {archive}"));
    let mut files: Vec<String> = layer.entries().into_iter().map(|e| e.path).collect();
    files.sort();
    Some(files)
}

fn utf8(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// L1: every `.xml` file of XML.rez that is a layout (`CBALAYOUT` root)
/// reads into a tree with at least one widget; the other XML files are
/// counted by root.
#[test]
fn install_layouts_read() {
    let Some(mount) = install() else { return };
    let Some(files) = files_of(&mount, "XML.rez") else { return };
    let mut roots = std::collections::BTreeMap::<String, usize>::new();
    let (mut layouts, mut failed) = (0, Vec::new());
    for path in files.iter().filter(|p| p.ends_with(".xml")) {
        let text = utf8(&mount.read(path).unwrap());
        let root = match xml::parse(&text) {
            Ok(e) => e.name.clone(),
            Err(e) => {
                failed.push(format!("{path}: {e}"));
                continue;
            }
        };
        *roots.entry(root.clone()).or_default() += 1;
        if root != "CBALAYOUT" {
            continue;
        }
        match xml::read(&text) {
            Ok(l) => {
                assert!(l.tree.order().len() > 1, "{path}: no widgets");
                layouts += 1;
            }
            Err(e) => failed.push(format!("{path}: {e}")),
        }
    }
    eprintln!("{} files; XML roots {roots:?}; {layouts} layouts read", files.len());
    for f in &failed {
        eprintln!("failed: {f}");
    }
    assert!(failed.is_empty(), "{} of the install's XML files don't read", failed.len());
    assert!(layouts > 100, "{layouts} layouts");
}

fn installed_ui(mount: &Mount) -> (combat_arms::ui::CaUi, std::sync::Arc<install::InstallUi>) {
    let inst = std::sync::Arc::new(install::load(mount, &[install::kill_icon("combat_arms:m4a1")]).expect("the install's UI"));
    (combat_arms::ui::CaUi::with_install(inst.clone()), inst)
}

/// With the keys the HUD's health and weapon panels are the game's HUD
/// layout's (`hud_main.xml`'s `character` and `weaponRoot`), the HUD's
/// values bound to its named controls: the HP value is drawn in
/// `Hp_text`'s place (its window's box, centred), the weapon's name in
/// `name`'s, the clip in `Ammo`'s, the stamina gauge's bar at its full
/// range for full stamina, the stance silhouette is the install's
/// standing image, and the panels' textures (their nine-slice
/// backgrounds) are the install's art. At 1024 x 768 (scale 1).
#[test]
fn install_hud_panels_bind() {
    use combat_arms::ui::{
        hud::{
            StatusView,
            install::{HudPanels, STAND},
            model::{FireIcon, WeaponPanel, health_colours},
        },
        layout::{Draw, Kind},
    };
    let Some(mount) = install() else { return };
    let (ui, inst) = installed_ui(&mount);
    assert_eq!(inst.language, install::language());
    let hud = inst.hud.as_ref().expect("hud_main.xml");
    let mut panels = HudPanels::new(hud).expect("character and weaponRoot");
    let c = panels.controls.clone();
    for (name, id) in [("Hp_text", c.hp), ("name", c.name), ("Ammo", c.clip), ("char_sp_gauge", c.stamina), ("characterStatus", c.silhouette), ("fire_icon", c.fire_icon)] {
        assert!(id.is_some(), "no {name} in the game's HUD layout");
    }
    let status = StatusView {
        hp: "87".into(),
        ap: "45".into(),
        low: false,
        colours: health_colours(false),
        crouched: false,
        stamina_bar: 120.0,
    };
    let weapon = WeaponPanel {
        name: "M4A1".into(),
        ammo: Some(("030".into(), "090".into())),
        icon: Some(FireIcon::Auto),
    };
    panels.bind(Some(&status), Some(&weapon), chimera_client::client::canvas::Scale::ONE);
    let draws = panels.layout.draw(&ui.ctx());
    let text = |s: &str| {
        draws.iter().find_map(|d| match d {
            Draw::Text(t) if t.lines.iter().any(|(_, l)| l == s) => Some(t.clone()),
            _ => None,
        })
    };
    let hp = text("87").expect("the HP value drawn");
    let hp_box = panels.at(panels.layout.tree.get(c.hp.unwrap()).parent.unwrap());
    eprintln!("HP at {:?} (its box at {hp_box})", hp.reference_at);
    assert_eq!(hp.reference_at.y, hp_box.y, "HP at its box's top (DEFAULT vertical)");
    assert!(hp.reference_at.x >= hp_box.x && hp.reference_at.x < hp_box.x + 40, "centred in its 40-wide box");
    let name = text("M4A1").expect("the weapon's name drawn");
    let clip = text("030").expect("the clip drawn");
    eprintln!("name at {:?}, clip at {:?}", name.reference_at, clip.reference_at);
    assert!(name.reference_at.x > 700 && clip.reference_at.y > 680, "the weapon panel bottom right");
    assert!(text("100").is_none(), "the layout's sample values replaced");
    // The gauge's bar at its full range.
    let Kind::Gauge(g) = &panels.layout.widget(c.stamina.unwrap()).kind else { panic!("not a gauge") };
    let bar = draws
        .iter()
        .filter_map(|d| match d {
            Draw::Quad(id, q) if *id == c.stamina.unwrap() && install::texture_key(&q.texture) == install::texture_key(&g.bar) => Some(q.clone()),
            _ => None,
        })
        .next()
        .expect("the stamina bar drawn");
    assert_eq!(bar.size[0] as i32, g.max_range, "{bar:?}");
    // The silhouette and the backgrounds: the install's art.
    let Kind::Image(sil) = &panels.layout.widget(c.silhouette.unwrap()).kind else { panic!() };
    assert_eq!(sil.texture, STAND);
    let quads: Vec<_> = draws.iter().filter_map(|d| if let Draw::Quad(_, q) = d { Some(q) } else { None }).collect();
    assert!(quads.len() >= 8, "{} quads", quads.len());
    for q in &quads {
        assert!(inst.art.contains_key(&install::texture_key(&q.texture)), "{} not in the install's art", q.texture);
    }
    assert!(quads.iter().any(|q| q.slices.is_some()), "nine-slice backgrounds");
    // Hidden: no status, no panel.
    panels.bind(None, None, chimera_client::client::canvas::Scale::ONE);
    assert!(panels.layout.draw(&ui.ctx()).is_empty());
}

/// With the keys the in-game menu is the game's (`popup_exit_ingame.xml`),
/// centred, its buttons bound by name (return to lobby, cancel and the
/// title's close button work; report, record and options disabled) and
/// its labels from the player's string table.
#[test]
fn install_ingame_menu() {
    use combat_arms::ui::{
        layout::Draw,
        menu::{CMD_CANCEL, CMD_LOBBY, INSTALL_BOX, InGameMenu},
    };
    let Some(mount) = install() else { return };
    let (ui, _) = installed_ui(&mount);
    let mut menu = InGameMenu::new(&ui);
    assert_eq!(menu.box_name, INSTALL_BOX);
    let l = menu.layout();
    let lobby = l.find("button_lobby").unwrap();
    assert_eq!(l.widget(lobby).cmd, CMD_LOBBY);
    assert_eq!(l.widget(l.find("button_cancel").unwrap()).cmd, CMD_CANCEL);
    assert!(!l.widget(l.find("button_userreport").unwrap()).enabled);
    // ui_system.md 27: the options button unbound: its normal look, no
    // command; the record label names the record key.
    if let Some(option) = l.find("button_option") {
        assert!(l.widget(option).enabled);
        assert_eq!(l.widget(option).cmd, combat_arms::ui::menu::CMD_NONE);
    }
    if let Some(label) = l.find(combat_arms::ui::menu::RECORD_LABEL) {
        let set = l.widget(label).text().and_then(|t| t.set.clone()).unwrap_or_default();
        assert!(!set.starts_with('$') && !set.contains("%s"), "{set}");
    }
    menu.screen = bevy::math::IVec2::new(1280, 720);
    menu.place();
    let b = menu.layout().find(INSTALL_BOX).unwrap();
    let at = menu.layout().tree.abs_pos(b);
    assert_eq!(at, bevy::math::IVec2::new(640 - 110, 360 - 87));
    menu.set_open(true);
    let draws = menu.layout().draw(&ui.ctx());
    let texts: Vec<String> = draws
        .iter()
        .filter_map(|d| if let Draw::Text(t) = d { Some(t.lines.iter().map(|l| l.1.clone()).collect::<String>()) } else { None })
        .collect();
    eprintln!("menu texts: {texts:?}");
    assert!(texts.len() >= 5);
    assert!(texts.iter().all(|t| !t.starts_with('$') && !t.starts_with('#')), "{texts:?}");
}

/// R1: the language pack's string tables read, with no duplicate keys
/// that would hide a value, and ids resolve through them (R2).
#[test]
fn install_string_tables_read() {
    let Some(mount) = install() else { return };
    let Some(files) = files_of(&mount, "Language_Pack.rez") else { return };
    eprintln!("Language_Pack.rez: {} files: {:?}", files.len(), files.iter().take(40).collect::<Vec<_>>());
    let mut tables = 0;
    for path in &files {
        let bytes = mount.read(path).unwrap();
        let Ok(text) = std::str::from_utf8(&bytes) else {
            eprintln!("{path}: not UTF-8 ({} bytes)", bytes.len());
            continue;
        };
        let t = Strings::parse(text);
        eprintln!("{path}: {} strings, {} duplicate keys", t.values.len(), t.duplicates.len());
        if t.values.len() > 100 {
            tables += 1;
        }
    }
    assert!(tables > 0, "no string table in Language_Pack.rez");
}

/// With the keys the rest of the HUD is the game's own layouts, bound by
/// name: the kill feed's line (`KillDeath`) with the install's weapon
/// icon and headshot badge, the TDM score panel (`hud_indicator.xml`'s
/// `Eli`) at D6's places, the pickup prompt in `Action`, the scoreboard
/// (`hud_result_team.xml`) with rows at test case 18's places and the
/// result banner (the HUD layout attributes' `scoreboardwintex`), the
/// loading screen (`hud_maploading.xml`) with the map's picture; every
/// texture they draw is the install's art, every text resolved.
#[test]
fn install_hud_rest_binds() {
    use combat_arms::{
        game::rules::{ScoreOrder, scoreboard::ScoreRow},
        ui::{
            hud::{
                BoundHud, FeedLineView,
                bound::FeedArt,
                model::{Badge, Kill, KillIcon, feed_line, score_panel},
                score::ScorePanelView,
            },
            layout::Draw,
            loading::{self, CaLoading},
            scoreboard::{BoundBoard, CaScoreboard, RoundResult},
        },
    };
    use chimera_client::client::canvas::Scale;
    let Some(mount) = install() else { return };
    let (ui, inst) = installed_ui(&mount);
    eprintln!("{inst:?}");
    assert!(inst.fonts.len() > 10, "the install's font list");
    assert!(ui.fonts.get("bold 12").is_some());
    let hud = BoundHud::of(&ui);
    let (feed, score, action) = (hud.feed.as_ref().unwrap(), hud.score.as_ref().unwrap(), hud.action.as_ref().unwrap());
    let check = |what: &str, draws: &[Draw]| {
        for d in draws {
            match d {
                Draw::Quad(_, q) => assert!(
                    q.texture == combat_arms::ui::hud::bound::DIVIDER
                        || inst.art.contains_key(&install::texture_key(&q.texture)),
                    "{what}: {} not in the install's art",
                    q.texture
                ),
                Draw::Text(t) => {
                    let s: String = t.lines.iter().map(|l| l.1.clone()).collect();
                    assert!(!s.starts_with('$') && !s.starts_with("XXX") && !s.contains("999"), "{what}: {s:?}");
                }
            }
        }
    };
    let texts = |draws: &[Draw]| -> Vec<(String, bevy::math::IVec2)> {
        draws
            .iter()
            .filter_map(|d| match d {
                Draw::Text(t) => Some((t.lines.iter().map(|l| l.1.clone()).collect(), t.reference_at)),
                _ => None,
            })
            .collect()
    };
    // The kill feed.
    let kill = Kill {
        killer: Some(("Alpha One".into(), true)),
        victim: ("Bravo Two".into(), false),
        icon: KillIcon::Weapon(Some("combat_arms:m4a1".into())),
        badges: vec![Badge::Headshot],
    };
    let line = FeedLineView {
        y: 45.0,
        items: feed_line(&kill, |s| combat_arms::ui::hud::feed_width(&ui.fonts, Default::default(), s)),
        kill,
    };
    let art = FeedArt {
        icon: &|i| hud.icon(i),
        badge: &|b| hud.badge(b),
    };
    let draws = feed.build(&[line], &art, Scale::ONE).draw(&ui.ctx());
    eprintln!("feed: {:?}", texts(&draws));
    check("feed", &draws);
    assert!(texts(&draws).iter().any(|(s, at)| s == "Alpha One" && *at == bevy::math::IVec2::new(5, 48)));
    assert_eq!(draws.iter().filter(|d| matches!(d, Draw::Quad(..))).count(), 2, "weapon icon and badge");
    // The score panel.
    let (rows, highlight) = score_panel([12, 7], 50, Some(1));
    let view = ScorePanelView {
        mode_key: "TDM".into(),
        mode_name: "Team Deathmatch".into(),
        goal: "50 kills".into(),
        rows,
        labels: ["Alpha".into(), "Bravo".into()],
        highlight,
        titles_are_counts: false,
        dividers: 0,
    };
    let draws = score.build("TDM", &view, (None, None), Scale::ONE).expect("Eli").draw(&ui.ctx());
    eprintln!("score: {:?}", texts(&draws));
    check("score panel", &draws);
    let t = texts(&draws);
    let at = |s: &str| t.iter().find(|x| x.0 == s).map(|x| x.1).unwrap_or_else(|| panic!("{s} not drawn"));
    assert!(at("12").x >= 612 && at("12").x < 677, "Alpha's score in its box (612, 23) 65 wide");
    assert!(at("7").y >= 43 && at("7").y < 61);
    // Every mode's own child (ui_system.md 28, game_modes.md 90).
    for mode in combat_arms::game::rules::MODES {
        let l = score.build(mode.key, &view, (Some("02 : 00"), None), Scale::ONE);
        let l = l.unwrap_or_else(|| panic!("{}: no {}", mode.key, combat_arms::ui::hud::bound::child_for(mode.key)));
        check(mode.key, &l.draw(&ui.ctx()));
    }
    // A round mode: the living counts in the row titles, the bars'
    // dividers, a plant's fuse in `BombTime` (hud_crosshair.md 92, 93).
    let round = ScorePanelView {
        mode_key: "TMM".into(),
        labels: ["5".into(), "3".into()],
        titles_are_counts: true,
        dividers: 7,
        ..view.clone()
    };
    let l = score.build("TMM", &round, (None, Some("00 : 40")), Scale::ONE).unwrap();
    let draws = l.draw(&ui.ctx());
    eprintln!("s&d: {:?}", texts(&draws));
    check("s&d", &draws);
    let t = texts(&draws);
    assert!(t.iter().any(|x| x.0 == "5") && t.iter().any(|x| x.0 == "3"), "{t:?}");
    let marks = draws
        .iter()
        .filter(|d| matches!(d, Draw::Quad(_, q) if q.texture == combat_arms::ui::hud::bound::DIVIDER))
        .count();
    assert_eq!(marks, 12, "G - 1 dividers a bar");
    if l.find("BombTime").is_some() {
        assert!(t.iter().any(|x| x.0 == "00 : 40"), "the bomb timer: {t:?}");
    }
    // The pickup prompt: the string table's, centred on `ActionDesc`
    // (hud_crosshair.md 95).
    let prompt = ui.strings.resolve("IDS_HUD_ITEMGET_MSG");
    assert!(!prompt.starts_with('$') && !prompt.is_empty());
    let l = action.build(Some(&prompt), None, Scale::ONE, &ui.fonts).unwrap();
    let draws = l.draw(&ui.ctx());
    eprintln!("action: {:?}", texts(&draws));
    check("action", &draws);
    assert_eq!(texts(&draws).len(), 1);
    let (desc, message) = (l.find("ActionDesc").unwrap(), l.find("Message").unwrap());
    let desc_mid = l.tree.abs_pos(desc).x + l.tree.get(desc).size.x / 2;
    let font = ui.fonts.get(&l.widget(message).text().unwrap().font).unwrap();
    let w = combat_arms::ui::layout::text::measure(&prompt, font, 0, Scale::ONE).reference.x;
    assert!((l.tree.abs_pos(message).x + w / 2 - desc_mid).abs() <= 1, "centred on ActionDesc");
    // The action gauge's titles (hud_crosshair.md 97).
    use combat_arms::ui::hud::model::GaugeKind;
    for kind in [GaugeKind::Plant, GaugeKind::Defuse] {
        let title = ui.strings.resolve(kind.title());
        assert!(!title.starts_with('$'), "{title}");
        let icon = kind.icon_key().and_then(|k| hud.texture(k));
        let draws = action.build(None, Some((&title, 50, icon)), Scale::ONE, &ui.fonts).unwrap().draw(&ui.ctx());
        check("gauge", &draws);
    }
    // The gauge sight's images (hud_crosshair.md 98) and the range
    // readout's font (99).
    let (l, r) = (inst.hud_prefix("CrossHairLeftTex").unwrap(), inst.hud_prefix("CrossHairRightTex").unwrap());
    let (left, right) = combat_arms::ui::hud::model::gauge_images(&l, &r, 67, 46);
    eprintln!("gauge sight: {left} {right}");
    assert!(inst.art.contains_key(&left) && inst.art.contains_key(&right), "{left} {right}");
    let range = ui.fonts.get(combat_arms::ui::hud::model::RANGE_FONT).expect("SysFont13 at 20");
    let sys13 = inst.fonts.iter().find(|f| f.number == 13).expect("SysFont13");
    assert_eq!((range.height, &range.face), (20, &ui.fonts.get(&sys13.name).unwrap().face));
    // The scoreboard.
    let row = |name: &str, team: u8| ScoreRow {
        player: bevy::prelude::Entity::PLACEHOLDER,
        name: name.into(),
        team,
        kills: 3,
        deaths: 1,
        assists: 0,
        captures: 0,
        returns: 0,
        detonations: 0,
        defuses: 0,
        mission_points: 0,
        hunted_ms: 0,
        ping: None,
    };
    let board = CaScoreboard {
        shown: true,
        lists: [vec![row("A0", 1), row("A1", 1)], vec![row("B0", 2)]],
        scores: [6, 3],
        headers: ["Alpha".into(), "Bravo".into()],
        order: Some(ScoreOrder::Kills),
        ffa: false,
        mode: None,
        result: Some(RoundResult::Win),
        lists_shown: true,
        sound: None,
    };
    let bound = BoundBoard::of(&ui);
    let draws = bound.build(&board, Scale::ONE).expect("the team list").draw(&ui.ctx());
    eprintln!("scoreboard: {:?}", texts(&draws));
    check("scoreboard", &draws);
    let t = texts(&draws);
    let a1 = t.iter().find(|x| x.0 == "A1").unwrap().1;
    let b0 = t.iter().find(|x| x.0 == "B0").unwrap().1;
    assert!((217..242).contains(&a1.y) && (392..417).contains(&b0.y), "{a1} {b0}");
    let banner = bound.banners[0].clone().expect("the win banner");
    assert!(draws.iter().any(|d| matches!(d, Draw::Quad(_, q) if install::texture_key(&q.texture) == banner)));
    // A single-round mode's result: the banner alone (ui_system.md 24).
    let alone = CaScoreboard {
        lists_shown: false,
        ..board.clone()
    };
    let draws = bound.build(&alone, Scale::ONE).unwrap().draw(&ui.ctx());
    check("result alone", &draws);
    assert!(texts(&draws).iter().all(|x| x.0 != "A1"), "no lists");
    assert!(draws.iter().any(|d| matches!(d, Draw::Quad(_, q) if install::texture_key(&q.texture) == banner)));
    // The loading screen (the map picture read as the screen opens).
    let src = inst.loading.as_ref().expect("hud_maploading.xml");
    let state = CaLoading {
        label: Some("combat_arms:junkflea".into()),
        progress: 0.5,
        mode: Some("QRT".into()),
    };
    let image = loading::map_image("junkflea").unwrap();
    assert!(install::decode(&mount, &image).is_some(), "{image}");
    let draws = loading::build(src, &state, Some(&image), &ui.strings, Scale::ONE).unwrap().draw(&ui.ctx());
    eprintln!("loading: {:?}", texts(&draws));
    let t = texts(&draws);
    assert!(t.iter().any(|x| x.0 == "JUNKFLEA"));
    // The mode's line (ui_system.md 30) and Quarantine's backdrop.
    assert!(t.iter().any(|x| x.0 == "Quarantine Regen"), "{t:?}");
    assert!(t.iter().all(|x| !x.0.starts_with('$')), "{t:?}");
}

/// With the install's real table (its goal formats are FormatMessage
/// style, `%1!d!`) the goal header shows a number and a unit, no raw
/// insert.
#[test]
fn install_goal_text_has_no_raw_inserts() {
    let Some(mount) = install() else { return };
    let (ui, _) = installed_ui(&mount);
    for (key, goal) in [("TDM", 100), ("TMM", 7), ("HQM", 10), ("CTF", 5)] {
        let text = combat_arms::ui::hud::model::goal_text(key, goal, |id| ui.strings.resolve(id));
        eprintln!("{key}: {text:?}");
        assert!(text.contains(&goal.to_string()) && !text.contains('%'), "{key}: {text}");
    }
}
