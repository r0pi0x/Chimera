//! The install's `fist` (weapon 934, Hunted's start weapon: game_modes.md
//! M15, case 84), as feedback 045da13b found it in a knife finale ("too
//! little damage, doesn't look like you're holding anything"): its
//! damage is its ammo record's (`InstDamage` 2, `Alt_InstDamage` 6; the
//! spec's melee table, special_weapons.md 5, has no fist row, so the
//! record is all there is), which is why the draw excludes it
//! (data/ladder_exclusions.toml); and its first-person model has no
//! pieces of its own, so what it draws is the character's hand model,
//! bound to its arm skeleton (view_models.md 1). Passes with a note
//! without an install or without the V1 and V2 keys.

use chimera_core::mount::config::LocalConfig;
use chimera_world::map::MapData;
use combat_arms::game::{
    GAME, INFO,
    data::CaData,
    rules::hunted,
    view_model::{self, VIEW_SCALE, hands},
};
use bevy::prelude::Vec3;

const FIST: &str = "combat_arms:fist";

#[test]
fn the_fist_is_its_records_punches_and_bare_hands() {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install");
        return;
    }
    if !(combat_arms::game::mount::can_read("V1") && combat_arms::game::mount::can_read("V2")) {
        eprintln!("skipping: needs the V1 and V2 keys");
        return;
    }
    let data = CaData::install();
    let Some(file) = data.attributes().and_then(|a| a.weapons.as_ref()) else {
        eprintln!("skipping: the install's attribute data doesn't read");
        return;
    };
    // Record 934 is the fist, and the merged document carries its ammo's
    // two damages and `Range`.
    assert_eq!(data.weapon_for_index(hunted::FIST), Some(FIST));
    let record = file.weapons.iter().find(|w| w.name == "fist").expect("the fist's record");
    let ammo = file.default_ammo(record).expect("its ammo");
    assert_eq!((ammo.inst_damage, ammo.alt_inst_damage, record.range), (2, 6, 130));
    let doc = data
        .weapon_docs()
        .iter()
        .map(|(_, t)| t.parse::<toml::Table>().unwrap())
        .find(|t| t.get("id").and_then(|v| v.as_str()) == Some(FIST))
        .expect("the fist's document");
    let knife = &doc["hooks"]["combat_arms:knife"];
    assert_eq!(knife["primary"]["damage"].as_float(), Some(2.0));
    assert_eq!(knife["secondary"]["damage"].as_float(), Some(6.0));
    assert_eq!(knife["range"].as_float(), Some(130.0));
    // Damage type 49 (F2), the surface's fist impact sounds, no select
    // sound (F1), the third person's world model with its knuckle piece.
    assert_eq!(knife["damage_type"].as_integer(), Some(49));
    let fx = &doc["hooks"]["combat_arms:effects"];
    assert_eq!(fx["ammo_kind"].as_str(), Some("fist"));
    assert_eq!(fx["select_sound"].as_str(), Some(""), "the fist names no select sound");
    assert!(doc["models"]["world"].as_str().is_some_and(|w| w.to_ascii_lowercase().contains("hh_melee_fist_ke")));

    // The look: no pieces of its own, the swing animations, and the hand
    // model's 43 bones all found in its skeleton.
    let mount = (INFO.open)(&install).unwrap();
    let read = |p: &str| mount.read(p).ok();
    let mut map = MapData::default();
    let w = data.weapon_set().get(FIST).expect("the fist's weapon");
    let path = w.models.view.as_deref().expect("a first-person model");
    let offset = w.models.view_offset.unwrap_or_default();
    let mut v = view_model::load(path, FIST, &w.models.view_skins, offset, &mut map, &read).expect("the fist's view model");
    assert!(v.model.meshes.is_empty(), "the fist's model draws nothing of its own: {}", v.model.meshes.len());
    let set = v.animations.as_ref().expect("its animations");
    for name in ["select", "idle_0", "fire_0", "alt_fire_0"] {
        assert!(set.sequence(name).is_some(), "{name}");
    }
    // The grip (case 84): the knuckles' idle, the right hand's finger
    // joints closed (about 70 degrees, 60 at least), the left relaxed.
    let mean = |side: &str| {
        let s = v.animations.as_ref().unwrap();
        let seq = s.sequence("idle_0").unwrap();
        let mut pose = s.defaults.clone();
        s.sequence_pose(seq, 0.24, &[], &mut pose);
        let angles: Vec<f32> = v
            .bones
            .iter()
            .zip(&pose)
            .filter(|(b, _)| {
                let n = b.name.to_ascii_lowercase();
                n.starts_with(&format!("bip01 {side} finger")) && n.len() > "bip01 r finger0".len() - 1 && !n.ends_with("finger0") && !n.ends_with("finger01") && !n.ends_with("finger02")
            })
            .map(|(_, p)| p.0.to_axis_angle().1.to_degrees())
            .collect();
        angles.iter().sum::<f32>() / angles.len() as f32
    };
    let (right, left) = (mean("r"), mean("l"));
    assert!(right > 60.0 && right < 90.0, "right hand closed: {right}");
    assert!(left > 20.0 && left < 40.0, "left hand relaxed: {left}");
    // Held, the hands are what shows: the default character's, attached.
    let choice = hands::default_choice(data.characters()).expect("the default character's hands");
    let (meshes, bones) = view_model::load_hands(&choice, &mut map, &read).expect("the hand model");
    assert_eq!(meshes.len(), 2, "sleeves and hands");
    let bound = hands::match_bones(&bones, &v.bones).expect("every hand bone in the fist's skeleton");
    assert_eq!(bound.len(), 43);
    let at = Vec3::from(lithtech_formats::ltb::to_rh_vec(offset)) * VIEW_SCALE;
    hands::attach(&mut v, meshes, &bones, VIEW_SCALE, at).expect("the hands attach");
    assert_eq!(v.model.meshes.len(), 2, "the bare hands drawn");
}
