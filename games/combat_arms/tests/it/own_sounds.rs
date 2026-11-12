//! Every install weapon plays its own sounds and effects (docs/backlog.md,
//! "Combat Arms guns' own sounds, scopes and models: audit", fixes 1 and
//! 4), on an install written here: a copy of our M4A1 and our M4A1 itself
//! get the sounds, muzzle groups and ammo effects of their own records
//! (effects_sounds.md, "How effects are described as data" and 11), not
//! their template's; a missing field is the spec's default; the reload and
//! select radii reach the played sounds.

use combat_arms::game::{
    data::{merge_weapon_docs, read_attributes, attributes::WEAPONS_FILE},
    effects::data::CaWeaponFx,
    view_model::CaViewParams,
};
use toml::{Table, Value};

use super::attribute_data::meters_per_unit;

const INSTALL: &str = r#"
[Ammo0] Name="556" Type=1 MaxAmount=120 ImpactFXName="Melee" FireFXName="9mm" TracerFXName="SMG02"
[Ammo1] Name="Plain" Type=1 MaxAmount=60
[Ammo2] Name="EmptyNames" Type=1 MaxAmount=60 ImpactFXName="" TracerFXName=""
[Weapon0] Name="M4A1" Guntype=4 AmmoName0="556" RapidFireCount=1 FireSnd="TEST\m4\Fire.wav" FireSndRadius=2100
    SilencedFireSnd1="TEST\m4\sil1.wav" SilencedFireSnd3="TEST\m4\sil3.wav" DryFireSnd="TEST\m4\dry.wav"
    ReloadSnd1="TEST\m4\r1.wav" ReloadSnd3="TEST\m4\r3.wav" SelectSnd="TEST\m4\sel.wav" DeselectSnd="TEST\m4\desel.wav"
    ReloadSndRadius=450 SelectSndRadius=250 PVMuzzleFXName="Own_PV" HHMuzzleFXName="Own_HH"
    PVFlashHiderFXName2="PV_Hide2" HHFlashHiderFXName1="HH_Hide1"
[Weapon1] Name="Copy Gun" Guntype=4 AmmoName0="Plain" RapidFireCount=1 FireSnd="TEST\copy\fire.wav"
    ReloadSnd2="TEST\copy\r2.wav" SelectSnd="TEST\copy\sel.wav" PVMuzzleFXName="Copy_PV" DotSightName="TEST\dot\Reticle.dtx"
[Weapon2] Name="Bare Gun" Guntype=4 AmmoName0="EmptyNames"
[Weapon3] Name="M67" Guntype=7 AmmoName0="Plain" FireSnd="TEST\frag\throw.wav" MiscSound1="TEST\frag\pin.wav"
"#;

fn merged() -> Vec<(String, Table)> {
    let (attrs, problems) = read_attributes(|name| match name {
        WEAPONS_FILE => Ok(INSTALL.as_bytes().to_vec()),
        _ => Ok(Vec::new()),
    });
    let weapons = attrs.weapons.as_ref().unwrap_or_else(|| panic!("{problems:?}"));
    let ours = [
        ("m4a1.toml", include_str!("../../data/weapons/m4a1.toml")),
        ("m67.toml", include_str!("../../data/weapons/m67.toml")),
    ];
    merge_weapon_docs(&ours, weapons, meters_per_unit())
        .unwrap()
        .into_iter()
        .map(|(n, t)| (n, t.parse().unwrap()))
        .collect()
}

fn hook<'a>(docs: &'a [(String, Table)], id: &str, name: &str) -> &'a Value {
    let (_, t) = docs
        .iter()
        .find(|(_, t)| t.get("id").and_then(Value::as_str) == Some(id))
        .unwrap_or_else(|| panic!("{id}"));
    t["hooks"].as_table().unwrap().get(name).unwrap_or_else(|| panic!("{id} has no {name}"))
}

fn fx(docs: &[(String, Table)], id: &str) -> CaWeaponFx {
    hook(docs, id, "combat_arms:effects").clone().try_into().unwrap()
}

fn view(docs: &[(String, Table)], id: &str) -> CaViewParams {
    hook(docs, id, "combat_arms:view_model").clone().try_into().unwrap()
}

/// A weapon's `DotSightName` is its reticle image (hud_crosshair.md A8):
/// the copy's own, none for a record that names none.
#[test]
fn a_weapon_carries_its_records_reticle() {
    let docs = merged();
    let reticle = |id: &str| hook(&docs, id, "combat_arms:hud").get("dot_sight").and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_string);
    let copy = "combat_arms:copy_gun";
    assert_eq!(reticle(copy).as_deref(), Some("TEST\\dot\\Reticle.dtx"));
    assert_eq!(reticle("combat_arms:m4a1"), None);
}

/// Our own M4A1 takes its record's sounds and groups over the built-in
/// ones, and so does a copy over its template's.
#[test]
fn a_weapon_and_its_copy_play_their_own_sounds() {
    let docs = merged();
    let m4 = fx(&docs, "combat_arms:m4a1");
    assert_eq!(m4.fire_sound, "test/m4/fire.wav");
    assert_eq!(m4.fire_radius, 2100.0);
    assert_eq!(m4.silenced_sounds, ["test/m4/sil1.wav", "", "test/m4/sil3.wav"]);
    assert_eq!(m4.dry_fire_sound, "test/m4/dry.wav");
    assert_eq!(m4.reload_sounds, ["test/m4/r1.wav", "", "test/m4/r3.wav"]);
    assert_eq!((m4.select_sound.as_str(), m4.deselect_sound.as_str()), ("test/m4/sel.wav", "test/m4/desel.wav"));
    assert_eq!(m4.hh_muzzle, "Own_HH");
    assert_eq!(m4.hh_flash_hider, ["HH_Hide1"]);
    let v = view(&docs, "combat_arms:m4a1");
    assert_eq!(v.muzzle_fx, "Own_PV");
    assert_eq!(v.flash_hider_fx, ["", "", "PV_Hide2"], "indexed by level");
    // The ammo's effects: names our tables have.
    assert_eq!((m4.impact.as_str(), m4.fire.as_str(), m4.tracer.as_str()), ("Melee", "9mm", "SMG02"));

    // The copy has the template's tables but none of its values.
    let copy = fx(&docs, "combat_arms:copy_gun");
    assert_eq!(copy.fire_sound, "test/copy/fire.wav");
    assert!(copy.silenced_sounds.is_empty() && copy.dry_fire_sound.is_empty());
    assert_eq!(copy.reload_sounds, ["", "test/copy/r2.wav"]);
    assert_eq!((copy.select_sound.as_str(), copy.deselect_sound.as_str()), ("test/copy/sel.wav", ""));
    assert!(copy.hh_muzzle.is_empty() && copy.hh_flash_hider.is_empty());
    assert_eq!(view(&docs, "combat_arms:copy_gun").muzzle_fx, "Copy_PV");
    assert!(view(&docs, "combat_arms:copy_gun").flash_hider_fx.is_empty());
    // Plain ammo names no effects at all (a missing key).
    assert_eq!((copy.impact.as_str(), copy.fire.as_str(), copy.tracer.as_str()), ("", "", ""));
}

/// A missing field is the spec's default (attribute_data.md): no sound,
/// no group, radii 1000 / 300 / 300; a present but empty impact name is
/// `BulletFmj`, an empty tracer name `Standard` (which our tables lack: the
/// template's tracer stays).
#[test]
fn missing_fields_fall_back_to_the_spec_default() {
    let docs = merged();
    let bare = fx(&docs, "combat_arms:bare_gun");
    assert!(bare.fire_sound.is_empty() && bare.select_sound.is_empty() && bare.reload_sounds.is_empty());
    assert_eq!((bare.fire_radius, bare.reload_radius, bare.select_radius), (1000.0, 300.0, 300.0));
    assert_eq!(bare.impact, "BulletFmj");
    assert_eq!(bare.tracer, "SMG02", "`Standard` isn't in our tables: the template's (the patched M4A1's)");
    assert!(view(&docs, "combat_arms:bare_gun").muzzle_fx.is_empty());
}

/// The reload and select radii of the record are the played sounds'.
#[test]
fn reload_and_select_radii_are_used() {
    let docs = merged();
    let m4 = fx(&docs, "combat_arms:m4a1");
    assert_eq!(m4.key_sound(1), Some(("test/m4/r1.wav", 450.0)));
    assert_eq!(m4.key_sound(4), Some(("test/m4/sel.wav", 250.0)));
    assert_eq!(m4.key_sound(5), Some(("test/m4/desel.wav", 250.0)));
    let bare = fx(&docs, "combat_arms:bare_gun");
    assert_eq!(bare.key_sound(1), None);
}

/// A document with no effects table gets one (the M67 has none built in),
/// its misc sounds with it; a thrown weapon's ammo effects are the
/// grenade hook's, not the ray impacts'.
#[test]
fn a_document_without_an_effects_table_gets_one() {
    let docs = merged();
    let frag = fx(&docs, "combat_arms:m67");
    assert_eq!(frag.fire_sound, "test/frag/throw.wav");
    assert_eq!(frag.misc_sounds, ["test/frag/pin.wav"]);
    assert_eq!(frag.impact, "");
}

/// What a weapon's set on the asset service holds of its sounds
/// (`chimera_pack sets combat_arms`, `weapon/<id>`): every file its
/// record names, once each, as the mount reads them; none for a weapon
/// that names none.
#[test]
fn a_weapon_set_lists_its_sounds() {
    let docs = merged();
    let m4 = fx(&docs, "combat_arms:m4a1");
    let mut want: Vec<&str> = vec![
        "test/m4/desel.wav",
        "test/m4/dry.wav",
        "test/m4/fire.wav",
        "test/m4/r1.wav",
        "test/m4/r3.wav",
        "test/m4/sel.wav",
        "test/m4/sil1.wav",
        "test/m4/sil3.wav",
    ];
    want.sort_unstable();
    assert_eq!(m4.sound_paths(), want);
    let copy = fx(&docs, "combat_arms:copy_gun");
    assert_eq!(copy.sound_paths(), ["test/copy/fire.wav", "test/copy/r2.wav", "test/copy/sel.wav"]);
    let frag = fx(&docs, "combat_arms:m67");
    assert!(frag.sound_paths().contains(&"test/frag/pin.wav".to_string()), "misc sounds too");
    assert!(fx(&docs, "combat_arms:bare_gun").sound_paths().is_empty());
}
