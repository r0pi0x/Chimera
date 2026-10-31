//! The install's own kill-cam layout and guide data (specs/combat_arms/
//! death_killcam.md 8, Open question 2): `Hud_KillCam`'s controls by role
//! bind to the death screen's values, and the tip records read. V1-keyed
//! (XML.rez, ATTRIBUTES.rez): skipped without the key or an install.

use bevy::math::Vec2;
use chimera_client::client::canvas::Scale;
use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::{
    game::{
        GAME, INFO,
        death_cam::screen::{
            DamageView, DeathScreen, DeathFacts, FastView, KillerView, RespawnView, RoomFacts, TargetView, TipView, WeaponView,
            Zone, parse_guide, pick_tip,
        },
        mount,
    },
    ui::{
        CaUi,
        hud::death::KillCamPanel,
        install,
        layout::Draw,
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

/// Files of the mount whose path holds `needle` (any case).
fn find(mount: &Mount, needle: &str) -> Vec<String> {
    let mut files: Vec<String> = mount
        .layers()
        .flat_map(|l| l.entries().into_iter().map(|e| e.path))
        .filter(|p| p.to_lowercase().contains(needle))
        .collect();
    files.sort();
    files
}

/// Where the kill-cam layout and the guide data are (the names the code
/// reads: `install::KILLCAM_FILE`, `GUIDE_FILE`).
#[test]
fn the_layout_and_the_guide_are_where_the_code_reads_them() {
    let Some(mount) = install() else { return };
    let kill = find(&mount, "killcam");
    eprintln!("kill-cam files: {kill:?}");
    assert!(kill.iter().any(|p| p == install::KILLCAM_FILE), "no {} in {kill:?}", install::KILLCAM_FILE);
    let guide = find(&mount, "guide");
    eprintln!("guide files: {guide:?}");
    assert!(guide.iter().any(|p| p == combat_arms::game::death_cam::GUIDE_FILE), "no {} in {guide:?}", combat_arms::game::death_cam::GUIDE_FILE);
}

/// The install's `KillCam` panel: every control the binding names by role
/// is there, the values go into them, and the screen draws.
#[test]
fn the_install_layout_binds_by_role() {
    let Some(mount) = install() else { return };
    let inst = std::sync::Arc::new(install::load(&mount, &[]).expect("the install's UI"));
    let ui = CaUi::with_install(inst.clone());
    let layout = inst.kill_cam.as_ref().expect("the kill-cam layout");
    let panel = KillCamPanel::new(layout).expect("KillCam");
    let roles: &[(&str, &[&str])] = &[
        ("KillerInfo", &["CamOn", "CamOff"]),
        ("CamOn", &["name", "Rank", "hp", "ap"]),
        ("CamOff", &["name", "Rank"]),
        ("KillerWeapon", &["name", "weapon_img"]),
        ("DamageWnd", &["Damage", "Damage_img"]),
        ("Target", &["name", "KillCount", "FreeCam_info"]),
        ("Banner", &["Banner_icon", "banner_description"]),
        ("Respawn", &["respawn_gauge", "timer", "des_FastRespawn_2", "des_FastRespawn"]),
        ("Respawn_EXPass", &["respawn_gauge", "timer"]),
    ];
    let mut missing = Vec::new();
    for (parent, children) in roles {
        let Some(p) = layout.find(parent) else {
            missing.push(parent.to_string());
            continue;
        };
        for c in *children {
            if layout.tree.find_from(p, c).is_none() {
                missing.push(format!("{parent}/{c}"));
            }
        }
    }
    assert!(missing.is_empty(), "controls the layout lacks: {missing:?}");
    let textures: std::collections::HashMap<String, String> = install::DEATH_LAYOUT_ART
        .iter()
        .filter_map(|k| Some((k.to_string(), inst.hud_texture(k)?)))
        .collect();
    for zone in [Zone::Head, Zone::Body, Zone::Arms, Zone::Leg, Zone::Groin] {
        let t = textures.get(&zone.string().to_lowercase()).unwrap_or_else(|| panic!("no texture for {zone:?}"));
        assert!(inst.art.contains_key(t), "{t} isn't decoded");
    }
    for bar in ["respawngauge_normal", "respawngauge_perk"] {
        assert!(textures.contains_key(bar), "no {bar}");
    }
    let screen = DeathScreen {
        killer: Some(KillerView {
            name: "Bravo".into(),
            cam_on: true,
            hp: "042".into(),
            ap: "000".into(),
        }),
        weapon: Some(WeaponView {
            name: "M4A1".into(),
            id: "combat_arms:m4a1".into(),
        }),
        damage: Some(DamageView {
            text: "75".into(),
            zone: Zone::Body,
        }),
        target: Some(TargetView {
            name: "Bravo".into(),
            kills_you: Some(3),
            free_cam_info: false,
        }),
        banner: Some(TipView {
            icon: String::new(),
            text: "A tip".into(),
        }),
        respawn: Some(RespawnView {
            gauge: 6,
            timer: "06:53".into(),
            perk: false,
            fast: Some(FastView::Free { left: 2, max: 3 }),
        }),
    };
    let size = Vec2::new(1024.0, 768.0);
    let built = panel.build(&screen, &ui.strings, &textures, Scale::of(size, size));
    let draws = built.draw(&ui.ctx());
    let texts: Vec<String> = draws
        .iter()
        .filter_map(|d| match d {
            Draw::Text(t) => Some(t.lines.iter().map(|l| l.1.clone()).collect::<String>()),
            _ => None,
        })
        .collect();
    eprintln!("texts drawn: {texts:?}");
    for want in ["Bravo", "042", "000", "75", "06:53"] {
        assert!(texts.iter().any(|t| t == want), "no {want:?} in {texts:?}");
    }
}

/// The guide data: tip records read, and one is picked for a death.
#[test]
fn the_guide_data_reads() {
    let Some(mount) = install() else { return };
    let bytes = mount.read(combat_arms::game::death_cam::GUIDE_FILE).expect("the guide file");
    let doc = lithtech_formats::attributes::Document::parse(&bytes).expect("it parses");
    let tips = parse_guide(&doc);
    eprintln!("{} tips; first: {:?}", tips.len(), tips.first());
    assert!(!tips.is_empty(), "no InGame records");
    let room = RoomFacts::default();
    let picked = pick_tip(&tips, &room, &DeathFacts::default(), 1);
    eprintln!("a tip for a plain death: {picked:?}");
}

/// The layout's controls, for reading the install (run with `--ignored
/// --nocapture`).
#[test]
#[ignore]
fn dump_layout() {
    let Some(mount) = install() else { return };
    let inst = install::load(&mount, &[]).expect("the install's UI");
    let layout = inst.kill_cam.as_ref().unwrap();
    for id in layout.tree.order() {
        let n = layout.tree.get(id);
        let t = layout.widget(id).text().map(|t| format!("text {:?}", t.element));
        eprintln!("{id:>3} {} parent {:?} show {} {t:?}", n.name, n.parent, n.show);
    }
    for (k, v) in &inst.hud_layout {
        if k.contains("damagepoint") || k.contains("observe") || k.contains("respawn") {
            eprintln!("HUDLayout0 {k} = {v}");
        }
    }
    if let Some(s) = &inst.strings {
        for (k, v) in &s.values {
            if k.contains("damagepoint") || k.contains("respawngauge") || k.contains("deadcam") || k.contains("fastrespawn") {
                eprintln!("string {k} = {v}");
            }
        }
    }
    if let Ok(bytes) = mount.read(combat_arms::game::death_cam::GUIDE_FILE) {
        let doc = lithtech_formats::attributes::Document::parse(&bytes).unwrap();
        eprintln!("guide sections: {:?}", doc.sections().iter().map(|s| s.name().to_string()).take(12).collect::<Vec<_>>());
        for s in doc.sections().iter().filter(|s| s.name().starts_with("InGame")).take(6) {
            let keys: Vec<String> = s
                .iter()
                .map(|(k, v)| match v {
                    lithtech_formats::attributes::Value::Str(b) => format!("{k}={:?}", String::from_utf8_lossy(b)),
                    v => format!("{k}={v:?}"),
                })
                .collect();
            eprintln!("[{}] {}", s.name(), keys.join(" | "));
        }
    }
    if let Ok(bytes) = mount.read(install::LAYOUT_ATTRIBUTES) {
        let doc = lithtech_formats::attributes::Document::parse(&bytes).unwrap();
        for s in doc.sections() {
            for (k, v) in s.iter() {
                if k.to_lowercase().contains("damagepoint") {
                    eprintln!("[{}] {k} = {v:?}", s.name());
                }
            }
        }
    }
    // Where the zone icons' strings are.
    for path in find(&mount, ".xml") {
        let Ok(bytes) = mount.read(&path) else { continue };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if text.to_lowercase().contains("damagepointicon") || text.to_lowercase().contains("observe_mode") {
            eprintln!("{path}");
            for (i, line) in text.lines().enumerate() {
                let l = line.to_lowercase();
                if l.contains("damagepointicon") || l.contains("observe_mode") || l.contains("respawngauge") {
                    eprintln!("  {i}: {}", line.trim());
                }
            }
        }
    }
}
