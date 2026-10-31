//! Effects that follow an entity, on the user's install
//! (specs/combat_arms/special_weapons.md Follow-up (2026-10-30), case
//! 54's data; game_modes.md Corrections (2026-10-30) 7, case 114): the
//! projectile records' looks in flight as the merge reads them, the
//! infected faces' eye groups and the effect file's eye group keys.
//! Needs the V1 key; passes with a note without.

use combat_arms::game::{
    GAME, INFO,
    data::CaData,
    effects::library::FX_FILE,
    special::{PROJECTILE, ProjectileParams},
};
use chimera_core::mount::config::LocalConfig;
use lithtech_formats::fxf::FxFile;

fn params(id: &str) -> Option<ProjectileParams> {
    let ca = CaData::install();
    ca.weapon_set().weapons.iter().find(|w| w.id == id).and_then(|w| w.hook::<ProjectileParams>(PROJECTILE))
}

/// Case 54's and Follow-up 1-3's data: the flame puff's group with smooth
/// shut-down and no fly sound; every impact projectile's look printed.
#[test]
fn projectile_looks_from_the_install() {
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.weapons.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data isn't readable (the V1 key)");
        return;
    }
    let mut seen = std::collections::BTreeSet::new();
    for w in &ca.weapon_set().weapons {
        let Some(p) = w.hook::<ProjectileParams>(PROJECTILE) else { continue };
        if seen.insert(p.look.key()) {
            eprintln!("{}: {:?}", w.id, p.look);
        }
    }
    let flame = params("combat_arms:flamethrower").expect("the flamethrower");
    assert_eq!(flame.look.fx, "Flame_Yellow");
    assert!(flame.look.fx_smooth, "FXSmoothShutdown 1");
    assert!(flame.look.fly_sound.is_empty(), "FlySound 0: none");
    assert!(flame.look.flare.is_none() && flame.look.smoke_trail.is_none());
    let law = params("combat_arms:law").expect("the LAW");
    assert!(!law.look.fx.is_empty(), "the rocket's trail group: {:?}", law.look);
    assert_eq!(ProjectileParams::is_modelless(&flame.shown_key()), true);
}

/// Case 114: override 0's face lists `Eye_Red_Tail_L`, faces 38 and 39
/// the assassin's and tanker's; the effect file's red eyes are six keys
/// on `EYE_L` and `EYE_R`; the infected body's sockets are printed.
#[test]
fn t114_eye_groups_by_face() {
    let ca = CaData::install();
    if ca.attributes().and_then(|a| a.models.as_ref()).is_none() {
        eprintln!("skipping: the install's attribute data isn't readable (the V1 key)");
        return;
    }
    let ch = ca.characters();
    let face = |id: i32| ch.face.iter().find(|f| f.id == id).map(|f| f.client_fx.clone()).unwrap_or_default();
    let o = ch.infection.iter().find(|r| r.id == 0).expect("override 0");
    assert_eq!(o.face, 20);
    assert_eq!(face(20), vec!["Eye_Red_Tail_L".to_string()]);
    assert_eq!(face(38), vec!["Eye_Assasin_Tail_SH".to_string()]);
    assert_eq!(face(39), vec!["Eye_Tanker_Tail_SH".to_string()]);
    assert_eq!(face(244), vec!["1".to_string()], "a group the effect file lacks");
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else { return };
    let mount = (INFO.open)(&install).unwrap();
    let Ok(bytes) = mount.read(FX_FILE) else {
        eprintln!("skipping the effect file: it doesn't read");
        return;
    };
    let fx = FxFile::parse(&String::from_utf8_lossy(&bytes)).unwrap();
    let g = fx.group("Eye_Red_Tail_L").expect("the red eyes");
    assert_eq!(g.keys.len(), 6);
    for k in &g.keys {
        eprintln!("{} {:.2}-{:.2} {:?}", k.kind, k.start, k.end, k.props);
    }
    let sockets: Vec<&str> = g.keys.iter().filter_map(|k| k.string("AttachName")).collect();
    assert!(sockets.iter().any(|s| s.eq_ignore_ascii_case("EYE_L")) && sockets.iter().any(|s| s.eq_ignore_ascii_case("EYE_R")), "{sockets:?}");
    assert!(fx.group("1").is_none());
    let file = |p: &str| mount.read(p).ok();
    for id in [0, 7] {
        let r = ch.model(id).unwrap();
        match combat_arms::game::model::load(&r.file, &file) {
            Ok(l) => eprintln!("body {id} sockets: {:?}", l.model.sockets.iter().map(|s| &s.name).collect::<Vec<_>>()),
            Err(e) => eprintln!("body {id}: {e}"),
        }
    }
    if let Some(f) = ch.face.iter().find(|f| f.id == 20).and_then(|f| f.file.clone()) {
        match combat_arms::game::model::load(&f, &file) {
            Ok(l) => eprintln!("face 20 sockets: {:?}", l.model.sockets.iter().map(|s| &s.name).collect::<Vec<_>>()),
            Err(e) => eprintln!("face 20: {e}"),
        }
    }
}
