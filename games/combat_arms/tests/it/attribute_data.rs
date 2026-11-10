//! Combat Arms' attribute records and our loader against
//! specs/combat_arms/attribute_data.md, "Test cases" 12 to 29 (1 to 11,
//! the format's, are lithtech_formats' tests/it/attributes.rs), on
//! documents written here.

use combat_arms::game::data::{
    InstallAttributes, attributes::*, merge_movement, merge_weapon_docs, read_attributes, slug,
};
use combat_arms::game::movement::{MOVEMENT_DATA, MovementData};
use chimera_sim::weapon::data::WeaponSet;
use lithtech_formats::attributes::Document;

fn doc(text: &str) -> Document {
    Document::parse(text.as_bytes()).unwrap_or_else(|e| panic!("{text:?}: {e}"))
}

fn weapons(text: &str) -> Result<WeaponsFile, String> {
    WeaponsFile::read(&doc(text))
}

/// The built-in movement data's metres per world unit.
pub fn meters_per_unit() -> f64 {
    toml::from_str::<MovementData>(MOVEMENT_DATA).unwrap().meters_per_unit as f64
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

/// 12. Record enumeration.
#[test]
fn case_12_record_enumeration() {
    let f = weapons(r#"[Ammo0] Name="a" [Ammo1] Name="b" [Ammo3] Name="d" [Weapon0] Name="W""#).unwrap();
    let names: Vec<_> = f.ammo.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
}

/// 13. Links.
#[test]
fn case_13_links() {
    let f = weapons(
        r#"[Ammo0] Name="9mm" [Ammo1] Name="9MM"
        [Weapon0] Name="W" AmmoName0="9mm" AmmoName1="none" AmmoName2="9Mm" AmmoName4="9mm""#,
    )
    .unwrap();
    let w = f.weapon("w").unwrap();
    assert_eq!(w.ammo, [0, 0]);
    assert_eq!(w.default_ammo(), Some(0));
}

/// 14. Weapon defaults.
#[test]
fn case_14_weapon_defaults() {
    let f = weapons(r#"[Ammo0] Name="a" [Weapon0] Name="W" AmmoName0="a""#).unwrap();
    let w = &f.weapons[0];
    assert_eq!(
        (w.shots_per_clip, w.perturb.min, w.perturb.max, w.duck_perturb),
        (100, 100, 100, 100)
    );
    assert_eq!((w.zoomed.min, w.zoomed.max, w.range, w.vectors_per_round), (1, 500, 3000, 1));
    assert_eq!((w.fire_delay, w.rapid_fire_count), (30, 1));
    assert_eq!((w.move_penalty, w.zoomed_move_penalty, w.zoom_time), (0.01, 0.7, 0.5));
    assert_eq!((w.damage_r_facter, w.max_cam_recoil_pitch, w.base_cam_recoil_pitch), (0.1, 10.0, 1.2));
    assert_eq!((w.ani_type, w.client_weapon_type, w.gun_type), (5, 1, 0));
    assert!(w.drop_by_player_dead);
    assert!(w.effect_range.is_empty() && w.alt_effect_range.is_empty());
}

/// 15. Ammo defaults.
#[test]
fn case_15_ammo_defaults() {
    let f = weapons(r#"[Ammo0] Name="a" [Weapon0] Name="W""#).unwrap();
    let a = &f.ammo[0];
    assert_eq!((a.inst_damage, a.alt_inst_damage, a.inst_damage_type), (0, 0, -1));
    assert_eq!(a.priority, -1.0);
    assert_eq!((a.dec_delay, a.jitter_dec_delay), (150, 600));
    assert_eq!((a.jitter_wait_recover, a.jitter_recover), (0.1, 0.3));
    assert_eq!(a.damage_move_penalty_duration, -1.0);
    assert_eq!(a.perturb, AmmoPerturb::default());
    assert_eq!(a.zoomed_perturb, AmmoPerturb::default());
}

/// 16. Alt damage fallback.
#[test]
fn case_16_alt_damage_fallback() {
    let f = weapons(r#"[Ammo0] Name="a" InstDamage=35 [Weapon0] Name="W""#).unwrap();
    assert_eq!(f.ammo[0].alt_inst_damage, 35);
    let f = weapons(r#"[Ammo0] Name="a" InstDamage=35 Alt_InstDamage=75 [Weapon0] Name="W""#).unwrap();
    assert_eq!(f.ammo[0].alt_inst_damage, 75);
}

/// 17. Decimals in integer fields.
#[test]
fn case_17_decimals_in_integer_fields() {
    let f = weapons(
        r#"[Ammo0] Name="a" [Weapon0] Name="W" MinPerturb=3 DuckPerturb=6.5 ZoomedMaxPerturb=19.5 FireDelay=0.9 ShotsPerClip=30.0"#,
    )
    .unwrap();
    let w = &f.weapons[0];
    assert_eq!(
        (w.perturb.min, w.duck_perturb, w.zoomed.max, w.fire_delay, w.shots_per_clip),
        (3, 100, 500, 30, 100)
    );
}

/// 18. Integers in number fields.
#[test]
fn case_18_integers_in_number_fields() {
    let f = weapons(r#"[Ammo0] Name="a" [Weapon0] Name="W" MovePenalty=0 MaxCamRecoilPitch=5"#).unwrap();
    assert_eq!((f.weapons[0].move_penalty, f.weapons[0].max_cam_recoil_pitch), (0.0, 5.0));
}

/// 19. Fire delay clamp.
#[test]
fn case_19_fire_delay_clamp() {
    let f = weapons(r#"[Ammo0] Name="a" [Weapon0] Name="W" FireDelay=-20"#).unwrap();
    assert_eq!(f.weapons[0].fire_delay, 0);
}

/// 20. Bool fields.
#[test]
fn case_20_bool_fields() {
    let f = weapons(
        r#"[Ammo0] Name="a" [Weapon0] Name="A" DropByPlayerDead=FALSE [Weapon1] Name="B" DropByPlayerDead=0 Hidden=true"#,
    )
    .unwrap();
    assert!(!f.weapons[0].drop_by_player_dead);
    assert!(f.weapons[1].drop_by_player_dead);
    assert!(f.weapons[1].hidden);
    assert!(!f.weapons[0].hidden);
}

const BANDS_AMMO: &str = r#"[Ammo0] Name="a" InstDamage=35 Rangedamage0="1.0" Rangedamage1="0.7" Rangedamage2="0.35""#;

fn damage(f: &WeaponsFile, d: f64) -> f64 {
    let w = &f.weapons[0];
    let a = f.default_ammo(w).unwrap();
    a.inst_damage as f64 * range_multiplier(&w.effect_range, &a.range_damage, d)
}

/// 21. Range bands.
#[test]
fn case_21_range_bands() {
    let f = weapons(&format!(
        r#"{BANDS_AMMO} [Weapon0] Name="W" AmmoName0="a" Effectrange0="2500" Effectrange1="3000" Effectrange2="3500.9""#
    ))
    .unwrap();
    assert_eq!(f.weapons[0].effect_range, [2500, 3000, 3500]);
    for (d, m, dmg) in [
        (0.0, 1.0, 35.0),
        (2500.0, 1.0, 35.0),
        (2500.5, 0.7, 24.5),
        (3000.0, 0.7, 24.5),
        (3200.0, 0.35, 12.25),
        (3500.0, 0.35, 12.25),
        (3500.5, 1.0, 35.0),
    ] {
        let w = &f.weapons[0];
        close(range_multiplier(&w.effect_range, &f.ammo[0].range_damage, d), m);
        close(damage(&f, d), dmg);
    }
}

/// 22. Unquoted band element.
#[test]
fn case_22_unquoted_band_element() {
    let f = weapons(&format!(
        r#"{BANDS_AMMO} [Weapon0] Name="W" AmmoName0="a" Effectrange0=2500 Effectrange1="3000" Effectrange2="3500.9""#
    ))
    .unwrap();
    assert_eq!(f.weapons[0].effect_range, [0, 3000, 3500]);
    close(damage(&f, 100.0), 24.5);
    close(damage(&f, 0.0), 35.0);
}

/// 23. Short band list.
#[test]
fn case_23_short_band_list() {
    let f = weapons(&format!(
        r#"{BANDS_AMMO} [Weapon0] Name="W" AmmoName0="a" Effectrange0="2500" Effectrange2="3500.9""#
    ))
    .unwrap();
    assert_eq!(f.weapons[0].effect_range, [2500]);
    close(damage(&f, 3200.0), 35.0);
}

/// 24. Weapon order.
#[test]
fn case_24_weapon_order() {
    let text = r#"[Ammo0] Name="a" [Weapon0] Name="W" [Weapon1] Name="K"
        [WeaponOrder] Class0="Primary" Class1="Melee" Primary0="W" Melee0="K""#;
    assert_eq!(weapons(text).unwrap().order_flat(), [0, 1]);
    assert!(weapons(&text.replace(r#"Melee0="K""#, r#"Melee0="Nope""#)).is_err());
    // Auto-switch: an unknown name is ignored.
    let f = weapons(&format!(r#"{text} AutoSwitchWeapon0="K" AutoSwitchWeapon1="Nope" AutoSwitchWeapon2="W""#)).unwrap();
    assert_eq!(f.auto_switch, [1, 0]);
}

/// 25. Long string.
#[test]
fn case_25_long_string() {
    let named = |n: usize| weapons(&format!(r#"[Ammo0] Name="a" [Weapon0] Name="{}""#, "N".repeat(n))).unwrap();
    assert_eq!(named(128).weapons[0].name, "");
    assert_eq!(named(127).weapons[0].name, "N".repeat(127));
}

/// 26. Missing record kinds.
#[test]
fn case_26_missing_record_kinds() {
    assert!(weapons(r#"[Weapon0] Name="W""#).is_err());
    assert!(weapons(r#"[Ammo0] Name="a""#).is_err());
}

/// 27. Body type.
#[test]
fn case_27_body_type() {
    let m = ModelsFile::read(&doc(
        r#"[FigureType0] Name="Male" DashSpeed=1.0 Maxstamina=100 Speed=90.7 [FigureType1] Name="Female""#,
    ));
    let (male, female) = (&m.figure_types[0], &m.figure_types[1]);
    assert_eq!((male.dash_speed, male.max_stamina, male.stats[1]), (1.0, 100, 90));
    assert_eq!(female.dash_speed, 1.4);
    assert!(female.female);
    assert_eq!(female.variable_jump, 0.0);
    // "Sex": anything but Female (any case) is male; missing is female.
    let m = ModelsFile::read(&doc(r#"[FigureType0] Sex="Male" [FigureType1] Sex="FEMALE""#));
    assert!(!m.figure_types[0].female && m.figure_types[1].female);
}

/// 28. Server player.
#[test]
fn case_28_server_player() {
    let p = ServerPlayer::read(&doc(r#"[Player] WalkSpeed=90 RunSpeed=240.5 JumpSpeed="485""#)).unwrap();
    assert_eq!((p.walk_speed, p.run_speed, p.jump_speed, p.ladder_speed), (90.0, 240.5, 0.0, 0.0));
}

/// Our document for case 29: weapon `w`, clip 25, range 4000 world units.
const OUR_W: &str = r#"
id = "combat_arms:w"
units = { length = 0.01 }
slot = 0
draw = 1.0

[ammo."combat_arms:a"]
max = 50

[trigger]
automatic = true
cycle = 0.1
timing = "set"

[magazine]
clip = 25
ammo = "combat_arms:a"
reload = 2.0

[hitscan]
range = 4000.0
pellets = 1
punch_scale = 0.0

[damage]
amount = 25.0
falloff = 1.0
falloff_step = 1.0
hitgroups = { head = 1.0, chest = 1.0, stomach = 1.0, arm = 1.0, leg = 1.0 }
impulse = 0.0
"#;

fn merged_w(install: &str) -> WeaponSet {
    let (attrs, problems) = read_attributes(|name| {
        Ok(if name == WEAPONS_FILE {
            install.as_bytes().to_vec()
        } else {
            Vec::new()
        })
    });
    let docs = match &attrs.weapons {
        Some(w) => merge_weapon_docs(&[("w.toml", OUR_W)], w, 0.01).unwrap(),
        None => {
            assert!(!problems.is_empty());
            vec![("w.toml".to_string(), OUR_W.to_string())]
        }
    };
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    WeaponSet::parse(&refs, &[]).unwrap()
}

/// 29. Override merge (our loader). The spec's install document has no
/// `Ammo0`, which case 26 says fails to load: one is added here.
#[test]
fn case_29_override_merge() {
    let set = merged_w(r#"[Ammo0] Name="other" [Weapon0] Name="W" ShotsPerClip=30"#);
    let w = &set.weapons[0];
    assert_eq!(w.id, "combat_arms:w");
    let m = w.magazine.as_ref().unwrap();
    assert_eq!(m.clip, 30);
    // 3000 world units, the built-in default (the document is in world units, 0.01 m).
    assert_eq!(w.hitscan.as_ref().unwrap().range, 3000.0);
    // RapidFireCount's default 1: semi-automatic.
    assert!(!w.trigger.as_ref().unwrap().automatic);
    // A rejected install document: ours unchanged.
    let set = merged_w(r#"[Weapon0] Name="W" ShotsPerClip=30 k=1e5"#);
    let w = &set.weapons[0];
    assert_eq!(w.magazine.as_ref().unwrap().clip, 25);
    assert_eq!(w.hitscan.as_ref().unwrap().range, 4000.0);
}

/// "Our loader": the default ammo's reserve, and install weapons with no
/// match become new documents copied from ours of the same class, after
/// ours.
#[test]
fn install_ammo_and_new_records() {
    let install = r#"
        [Ammo0] Name="A" MaxAmount=120
        [Ammo1] Name="B ammo" MaxAmount=40
        [Weapon0] Name="New Gun" Guntype=4 ShotsPerClip=10 AmmoName0="B ammo"
        [Weapon1] Name="W" Guntype=4 ShotsPerClip=30 RapidFireCount=0 Range=3500 AmmoName0="A"
        [Weapon2] Name="Pistol" Guntype=1 AmmoName0="A"
    "#;
    let set = merged_w(install);
    let ids: Vec<_> = set.weapons.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, ["combat_arms:w", "combat_arms:new_gun"], "the pistol has no class in our data");
    let w = &set.weapons[0];
    assert!(w.trigger.as_ref().unwrap().automatic);
    assert_eq!(w.hitscan.as_ref().unwrap().range, 3500.0);
    assert_eq!(set.ammo["combat_arms:a"].max, 90);
    let n = &set.weapons[1];
    assert_eq!(n.magazine.as_ref().unwrap().clip, 10);
    assert_eq!(n.magazine.as_ref().unwrap().ammo, "combat_arms:b_ammo");
    assert_eq!(set.ammo["combat_arms:b_ammo"].max, 30);
    assert_eq!(slug("B ammo"), "b_ammo");
}

/// "Our loader" for movement: the install's FigureType0 replaces our
/// figure.
#[test]
fn movement_takes_figure_type_0() {
    let mut data: MovementData = toml::from_str(MOVEMENT_DATA).unwrap();
    let models = ModelsFile::read(&doc(
        r#"[FigureType0] Name="Male" DashSpeed=1.2 Maxstamina=120 DashGauge=20 GaugeRec=15 VariableJump=0.1"#,
    ));
    merge_movement(&mut data, &models);
    let f = &data.character.figure;
    assert_eq!((f.dash_speed, f.max_stamina, f.dash_gauge, f.gauge_recovery, f.variable_jump), (1.2, 120.0, 20.0, 15.0, 0.1));
    // Gear isn't the models file's: unchanged.
    assert_eq!(data.character.gear.len(), 1);
}

/// A body record's `AdditionalSkins#` (texture slots 5 on, characters.md
/// 2: the ghillie suits' leaves) are read with its upper and leg skins.
#[test]
fn body_additional_skins() {
    let m = ModelsFile::read(&doc(
        r#"[Model0] ModelFile="CHARS_M_BODY\G.ltb" UpperSkin0="CHARS_T_BODY\G.dtx" LowerSkin0="CHARS_T_BODY\L.DTX"
          AdditionalSkins0="CHARS_T_BODY\Fur.DTX" AdditionalSkins1="textures\Spec.dtx" ColorSkin1_AdditionalSkins0="x.dtx"
        [Model1] ModelFile="b.ltb""#,
    ));
    assert_eq!(m.bodies[0].additional_skins, ["CHARS_T_BODY\\Fur.DTX", "textures\\Spec.dtx"]);
    assert!(m.bodies[1].additional_skins.is_empty());
}

/// A loader with no readable files gets nothing and says why per file.
#[test]
fn unreadable_files_are_reported() {
    let (attrs, problems) =
        read_attributes(|name| Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, name.to_string())));
    assert_eq!(attrs, InstallAttributes::default());
    // Weapons, server, models, modification, marks, attachments, perk, set
    // item and effect files.
    assert_eq!(problems.len(), 9);
}

/// The merge onto our built-in documents as they are: an install with the
/// spec's reference M4A1 ("Reference values (one rifle)") gives back our
/// M4A1's numbers, every hook still reads its parameters, and an
/// install pistol with no matched pistol of ours adds no weapon.
#[test]
fn reference_m4a1_onto_our_documents() {
    use combat_arms::game::{
        data::WEAPON_DATA,
        hud::CaHudData,
        inventory::CaItem,
        weapons::{CaHandling, DamageParams, RecoilParams, SpreadParams},
    };
    let install = r#"
        [Ammo0] Name="5.56mm M4A1" InstDamage=35 InstDamageType=2 MaxAmount=120 SpawnedAmount=120
          SelectionAmount=120 Rangedamage0="1.0" Rangedamage1="0.7" Rangedamage2="0.35"
          FirePerturb=0.30 FireMovePerturb=0.38 FireDuckPerturb=0.30 FireMoveDuckPerturb=0.38
          WalkPerturb=0.04 RunPerturb=0.25 JumpPerturb=0.70 PerturbRecoil=1.0
          DecDelay=95 JitterDecDelay=95 JitterWaitRecover=0.1 JitterRecover=0.2
          DamageMovePenalty=40 DamageMovePenaltyDuration=0.3
        [Ammo1] Name="9mm"
        [Weapon0] Name="M9_Pistol" Guntype=1 AmmoName0="9mm"
        [Weapon1] Name="M4A1" Guntype=4 ShotsPerClip=30 RapidFireCount=0 FireDelay=0 Range=3500
          VectorsPerRound=1 MinPerturb=3 MaxPerturb=100 DuckPerturb=2 MoveDuckPerturb=3
          PerturbIncSpeed=1.3 PerturbDecSpeed=0.3 ZoomedMinPerturb=2 ZoomedMaxPerturb=30
          ZoomedPerturbIncSpeed=1.3 ZoomedPerturbDecSpeed=0.3 MovePenalty=0.1 ZoomedMovePenalty=0.1
          ZoomTime=0.15 BaseCamRecoilPitch=1.3 MaxCamRecoilPitch=5 BaseCamRecoilAccelPitch=0.2
          MaxCamRecoilWag=15 BaseCamRecoilWagL=1.2 BaseCamRecoilWagR=-1.2 MaxRandomPitch=0.02
          MaxRandomWag=0.01 BaseCamBackJitter=0.0 BackJitterRecover=1.0 DamageRFacter=0.05
          Effectrange0="2500" Effectrange1="3000" Effectrange2="3500" AmmoName0="5.56mm M4A1"
          ReloadPerBullet=0
    "#;
    let f = weapons(install).unwrap();
    let docs = merge_weapon_docs(WEAPON_DATA, &f, meters_per_unit()).unwrap();
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let merged = WeaponSet::parse(&refs, &[]).unwrap();
    let builtin = WeaponSet::parse(WEAPON_DATA, &[]).unwrap();
    // The install's pistol has no class in our data: no new weapon.
    assert_eq!(merged.weapons.len(), builtin.weapons.len());
    for (m, b) in merged.weapons.iter().zip(&builtin.weapons) {
        assert_eq!(m.id, b.id);
        assert_eq!(m.magazine.as_ref().map(|x| x.clip), b.magazine.as_ref().map(|x| x.clip), "{}", m.id);
        assert_eq!(m.hitscan.as_ref().map(|x| x.range), b.hitscan.as_ref().map(|x| x.range), "{}", m.id);
        assert_eq!(m.trigger.as_ref().map(|x| x.automatic), b.trigger.as_ref().map(|x| x.automatic));
        for name in ["combat_arms:damage", "combat_arms:spread", "combat_arms:recoil", "combat_arms:handling"] {
            if let (Some(mv), Some(bv)) = (m.hooks.get(name), b.hooks.get(name)) {
                // Same numbers, up to the f32 the hooks read them as.
                assert!(floats_close(mv, bv), "{}: {name}\n{mv}\n{bv}", m.id);
            }
        }
    }
    let m4 = merged.weapons.iter().find(|w| w.id == "combat_arms:m4a1").unwrap();
    assert_eq!(merged.ammo["combat_arms:5.56mm_m4a1"].max, 90);
    let d: DamageParams = m4.hook("combat_arms:damage").unwrap();
    assert_eq!((d.damage, d.damage_type, d.roll, d.fire_delay), (35.0, 2, 0.05, 0.0));
    assert_eq!(d.effect_range, [2500.0, 3000.0, 3500.0]);
    let s: SpreadParams = m4.hook("combat_arms:spread").unwrap();
    assert_eq!((s.cone.min, s.cone.max, s.dec_delay), (3, 100, 95.0));
    assert!(m4.hook::<RecoilParams>("combat_arms:recoil").is_some());
    assert_eq!(m4.hook::<CaHandling>("combat_arms:handling").unwrap().move_penalty, 0.1);
    let hud: CaHudData = m4.hook("combat_arms:hud").unwrap();
    assert_eq!((hud.duck_perturb, hud.move_duck_perturb), (2, 3));
    let item: CaItem = m4.hook("combat_arms:inventory").unwrap();
    assert!(item.drop_on_death && !item.infinite);
}

/// Two TOML values equal, floats within f32 precision; keys only one
/// side has must be the merge's additions (the install's zoomed state).
fn floats_close(a: &toml::Value, b: &toml::Value) -> bool {
    use toml::Value::*;
    match (a, b) {
        (Float(x), Float(y)) => ((*x as f32) - (*y as f32)).abs() <= 1e-6,
        (Integer(x), Float(y)) | (Float(y), Integer(x)) => (*x as f64 - y).abs() <= 1e-6,
        (Array(x), Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(x, y)| floats_close(x, y)),
        (Table(x), Table(y)) => y.iter().all(|(k, w)| x.get(k).is_some_and(|v| floats_close(v, w))),
        _ => a == b,
    }
}

/// 30. The knock-back fields: K and its duration from the ammo record;
/// on an infected victim (movement.md case 55) 0.3 s from 40^2 = 1600
/// u/s^2; the M9's duration 0: none; `StiffDuration` 0.5: 0.45 s. Our
/// documents take them from the install's default ammo.
#[test]
fn case_30_knock_back_fields() {
    use bevy::math::Vec3;
    use combat_arms::game::movement::HitKnockback;
    let f = weapons(
        r#"[Ammo0] Name="a" DamageMovePenalty=40 DamageMovePenaltyDuration=0.3
        [Ammo1] Name="m9" DamageMovePenalty=20 DamageMovePenaltyDuration=0
        [Weapon0] Name="W" AmmoName0="a""#,
    )
    .unwrap();
    let (a, m9) = (&f.ammo[0], &f.ammo[1]);
    close(a.damage_move_penalty, 40.0);
    close(a.damage_move_penalty_duration, 0.3);
    let k = HitKnockback::hit(None, 40.0, 0.3, 0.0, Vec3::Z, 0.0).unwrap();
    assert_eq!((k.strength * k.strength, k.duration), (1600.0, 0.3));
    assert!(HitKnockback::hit(None, m9.damage_move_penalty as f32, m9.damage_move_penalty_duration as f32, 0.0, Vec3::Z, 0.0).is_none());
    let k = HitKnockback::hit(None, 40.0, 0.3, 0.5, Vec3::Z, 0.0).unwrap();
    assert!((k.duration - 0.45).abs() < 1e-6);
    // Through the merge onto our M4A1.
    use combat_arms::game::weapons::DamageParams;
    let install = weapons(
        r#"[Ammo0] Name="5.56mm" DamageMovePenalty=25 DamageMovePenaltyDuration=0.5
        [Weapon0] Name="M4A1" AmmoName0="5.56mm""#,
    )
    .unwrap();
    let ours = include_str!("../../data/weapons/m4a1.toml");
    let docs = merge_weapon_docs(&[("m4a1.toml", ours)], &install, meters_per_unit()).unwrap();
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let set = WeaponSet::parse(&refs, &[]).unwrap();
    let d: DamageParams = set.weapons[0].hook("combat_arms:damage").unwrap();
    assert_eq!((d.move_penalty, d.move_penalty_duration), (25.0, 0.5));
}

/// 31. The inventory class and the command id come from the weapon
/// order, not `Guntype`.
#[test]
fn case_31_inventory_class_from_the_weapon_order() {
    let f = weapons(
        r#"[Ammo0] Name="x"
        [Weapon0] Name="A" Guntype=6 [Weapon1] Name="B" [Weapon2] Name="P" Guntype=1 [Weapon3] Name="Q"
        [WeaponOrder] Class0="Primary" Class1="Secondary" Primary0="A" Primary1="B" Secondary0="P""#,
    )
    .unwrap();
    let class: Vec<u8> = (0..4).map(|i| f.inventory_class(i)).collect();
    assert_eq!(class, [1, 1, 2, 0]);
    let ids: Vec<Option<usize>> = (0..4).map(|i| f.command_id(i)).collect();
    assert_eq!(ids, [Some(0), Some(1), Some(2), None]);
}

/// 32. The HUD's fire-mode fields are read (the icon itself: the HUD's
/// tests, tests/ui/hud.rs case 76); `HudFireMode` changes nothing.
#[test]
fn case_32_fire_mode_fields() {
    let f = weapons(
        r#"[Ammo0] Name="x"
        [Weapon0] Name="Auto" RapidFireCount=0
        [Weapon1] Name="Semi" RapidFireCount=1 ContinuousFire=1
        [Weapon2] Name="Three" RapidFireCount=3
        [Weapon3] Name="Burst2" TripleShotMode=1 TripleShotRoundNum=2
        [Weapon4] Name="Burst" TripleShotMode=1 HudFireMode=5"#,
    )
    .unwrap();
    let w = &f.weapons;
    assert_eq!((w[0].rapid_fire_count, w[1].rapid_fire_count, w[2].rapid_fire_count), (0, 1, 3));
    assert_eq!(w[1].continuous_fire, 1);
    assert_eq!((w[3].triple_shot_mode, w[3].triple_shot_round_num), (1, 2));
    assert_eq!(w[4].triple_shot_round_num, 3, "missing: 3 rounds");
    assert_eq!(w[4].hud_fire_mode, 5);
    // Our documents: RapidFireCount 0 automatic, 1 not.
    let set = merged_w(r#"[Ammo0] Name="A" [Weapon0] Name="W" RapidFireCount=0 AmmoName0="A""#);
    assert!(set.weapons[0].trigger.as_ref().unwrap().automatic);
}

/// The models: an install weapon's `PVModelNormal`, `PVSkin#` (the
/// `Hands` placeholder and other non-texture entries empty) and `Pos`
/// become our document's view model, `HHModel` its held model,
/// `MuzzlePos` and `BreachOffset` the view-model table's; the knife is
/// the install's "M9" and the incendiary grenade its "M-23F" (names
/// that aren't our IDs' slugs).
#[test]
fn models_and_names_from_the_install() {
    use combat_arms::game::view_model::CaViewParams;
    let install = weapons(
        r#"[Ammo0] Name="k"
        [Weapon0] Name="M9" Guntype=0 AmmoName0="k" PVModelNormal="GUNS_M_PV_MELEE\PV_Melee_M9_SH.ltb"
          PVSkin0="Hands" PVSkin1="GUNS_T_PV_MELEE\PV_M9.dtx" PVSkin2="PV_L_ARM" Pos=<0.5, -0.9, 1.5>
          HHModel="GUNS_M_HH\HH_M9.ltb" MuzzlePos=<1, 2, 3> BreachOffset=<1, 1, 1>
        [Weapon1] Name="M-23F" Guntype=7 AmmoName0="k" PVModelNormal="GUNS_M_PV_Throwing\PV_Throwing_M18Smoke_SH.ltb""#,
    )
    .unwrap();
    let docs = merge_weapon_docs(
        &[
            ("m9.toml", include_str!("../../data/weapons/m9.toml")),
            ("m23f.toml", include_str!("../../data/weapons/m23f.toml")),
        ],
        &install,
        meters_per_unit(),
    )
    .unwrap();
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let set = WeaponSet::parse(&refs, &[]).unwrap();
    assert_eq!(set.weapons.len(), 2, "both matched, nothing new");
    let knife = set.get("combat_arms:m9_knife").unwrap();
    assert_eq!(knife.models.view.as_deref(), Some("guns_m_pv_melee/pv_melee_m9_sh.ltb"));
    assert_eq!(knife.models.view_skins, ["", "guns_t_pv_melee/pv_m9.dtx", ""]);
    assert_eq!(knife.models.view_offset, Some([0.5, -0.9, 1.5]));
    assert_eq!(knife.models.world.as_deref(), Some("guns_m_hh/hh_m9.ltb"));
    let p: CaViewParams = knife.hook("combat_arms:view_model").unwrap();
    assert_eq!((p.muzzle_pos, p.breach_offset), ([1.0, 2.0, 3.0], [1.0; 3]));
    let grenade = set.get("combat_arms:m23f").unwrap();
    assert_eq!(grenade.models.view.as_deref(), Some("guns_m_pv_throwing/pv_throwing_m18smoke_sh.ltb"));
}

/// An install weapon's sprite skins (`.spr`, an animated texture
/// sequence: world_dat.md; the rainbow, lava-crystal and light-up guns'
/// `PVSkin#` / `HHSkin#`) are texture files, not empty slots: the
/// playtest's grey "Prismatic" M416 had its skin dropped. A weapon the
/// install adds (not one of ours) gets its held look too: its `HHSkin#`,
/// `HHRenderStyle#` and `AniType` (characters.md, "Third-person weapon
/// model and animation class").
#[test]
fn sprite_skins_and_an_added_weapons_held_look() {
    use combat_arms::game::data::CaData;
    let text = r#"[Ammo0] Name="a"
        [Weapon0] Name="M4A1" Guntype=2 AmmoName0="a" AniType=3
        [Weapon1] Name="M416 Prismatic" Guntype=2 AmmoName0="a" AniType=4
          PVModelNormal="GUNS_M_PV_AR\PV_AR_M416_SH.ltb"
          PVSkin0="Hands" PVSkin1="GUNS_T_PV_AR\PV_AR_RINBOW_M416.SPR" PVSkin2="GUNS_T_PV_AR\PV_M416.dtx"
          HHModel="GUNS_M_HH\HH_AR_M416_SH.ltb" HHSkin0="GUNS_T_HH\HH_AR_RINBOW_M416.spr" HHRenderStyle0="RS\GLOW.LTB""#;
    let ca = CaData::with_attributes(InstallAttributes {
        weapons: Some(weapons(text).unwrap()),
        ..Default::default()
    });
    let id = format!("combat_arms:{}", slug("M416 Prismatic"));
    let w = ca.weapon_set().get(&id).unwrap_or_else(|| panic!("{id} merged"));
    assert_eq!(
        w.models.view_skins,
        ["", "guns_t_pv_ar/pv_ar_rinbow_m416.spr", "guns_t_pv_ar/pv_m416.dtx"]
    );
    assert_eq!(w.models.world.as_deref(), Some("guns_m_hh/hh_ar_m416_sh.ltb"));
    let look = ca.characters().weapon(Some(&id));
    assert_eq!(look.skins, ["guns_t_hh/hh_ar_rinbow_m416.spr"]);
    assert_eq!(look.render_styles, ["RS\\GLOW.LTB"]);
    assert_eq!(look.ani_type, 4);
    // Ours keeps the install's class as before.
    assert_eq!(ca.characters().weapon(Some("combat_arms:m4a1")).ani_type, 3);
}

/// A grenade's projectile record (`ProjectileFX#` in `FX.TXT`, linked by
/// its ammo's `ProjectileFXName`; grenades.md "Data"): its speed, fuse,
/// drop speed, `CanHitSameKind` and thrown model (`Model`, `ModelScale`,
/// `Skin0`) reach the grenade hook; a record without a field (the XM84's
/// `DropVelocity`) leaves ours. Read through `read_attributes` from both
/// files.
#[test]
fn grenade_projectile_records_from_the_install() {
    use combat_arms::game::grenades::{GrenadeParams, HOOK};
    let weapons_txt = r#"[Ammo0] Name="M67" ProjectileFXName="G62"
        [Ammo1] Name="XM-84" ProjectileFXName="FB6"
        [Weapon0] Name="M67" Guntype=7 AmmoName0="M67"
        [Weapon1] Name="XM84" Guntype=7 AmmoName0="XM-84""#;
    let fx_txt = r#"[ProjectileFX0] Name="G62" Class="CGrenade" Velocity=1500 Lifetime=2.25 DropVelocity=450
          CanHitSameKind=1 Model="GUNS_M_HH\HH_Throwing_Test_SH.ltb" ModelScale=<2.0, 2.0, 2.0>
          Skin0="GUNS_T_HH\HH_test.dtx" FireOffset=0.0
        [ProjectileFX1] Name="FB6" Class="CGrenade" Velocity=1550 Lifetime=1.5 CanHitSameKind=0"#;
    let (attrs, _) = read_attributes(|name| match name {
        WEAPONS_FILE => Ok(weapons_txt.as_bytes().to_vec()),
        FX_FILE => Ok(fx_txt.as_bytes().to_vec()),
        _ => Err(std::io::Error::new(std::io::ErrorKind::NotFound, name.to_string())),
    });
    let f = attrs.weapons.unwrap();
    assert_eq!(f.projectiles.len(), 2);
    let g62 = f.projectile(&f.ammo[0]).unwrap();
    assert_eq!((g62.velocity, g62.lifetime, g62.drop_velocity), (Some(1500.0), Some(2.25), Some(450.0)));
    assert_eq!(g62.model_scale, [2.0; 3]);
    let docs = merge_weapon_docs(
        &[
            ("m67.toml", include_str!("../../data/weapons/m67.toml")),
            ("xm84.toml", include_str!("../../data/weapons/xm84.toml")),
        ],
        &f,
        meters_per_unit(),
    )
    .unwrap();
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let set = WeaponSet::parse(&refs, &[]).unwrap();
    let m67: GrenadeParams = set.get("combat_arms:m67").unwrap().hook(HOOK).unwrap();
    assert!((m67.speed - 1500.0).abs() < 1e-3 && (m67.fuse - 2.25).abs() < 1e-6, "{m67:?}");
    assert!((m67.drop_speed - 450.0).abs() < 1e-3 && m67.hits_same_kind);
    assert_eq!(m67.thrown_model.as_deref(), Some("guns_m_hh/hh_throwing_test_sh.ltb"));
    assert_eq!(m67.thrown_skins, ["guns_t_hh/hh_test.dtx"]);
    assert_eq!(m67.thrown_scale, [2.0; 3]);
    let xm84: GrenadeParams = set.get("combat_arms:xm84").unwrap().hook(HOOK).unwrap();
    assert!((xm84.fuse - 1.5).abs() < 1e-6 && !xm84.hits_same_kind);
    assert_eq!(xm84.thrown_model, None, "no model in its record: the placeholder");
    // Without the effect file, our documents' numbers stand.
    let bare = weapons(weapons_txt).unwrap();
    let docs = merge_weapon_docs(&[("m67.toml", include_str!("../../data/weapons/m67.toml"))], &bare, meters_per_unit())
        .unwrap();
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let set = WeaponSet::parse(&refs, &[]).unwrap();
    let m67: GrenadeParams = set.get("combat_arms:m67").unwrap().hook(HOOK).unwrap();
    assert_eq!((m67.speed, m67.fuse, m67.thrown_model), (1550.0, 2.5, None));
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-19): 33-36

/// 33. The node priority read: `Node4Priority=2.5` → 2.5; a node without
/// the key → 0. And our data's priorities by location.
#[test]
fn case_33_node_priority_read() {
    let d = doc(
        r#"[Skeleton0] Node0Name="a" Node1Name="b" Node2Name="c" Node3Name="d" Node4Name="e"
        Node4Priority=2.5 Node4Radius=3.0 Node4Location=1 Node4DamageFactor=6.0"#,
    );
    let nodes = SkeletonNode::read_all(d.records("Skeleton")[0]);
    assert_eq!(nodes.len(), 5);
    assert_eq!(nodes[4].priority, 2.5);
    assert_eq!((nodes[4].radius, nodes[4].location, nodes[4].damage_factor), (3.0, 1, 6.0));
    assert_eq!(nodes[3].priority, 0.0);
    let male = *combat_arms::game::weapons::MALE;
    let head = male.iter().find(|s| s.name == "Head").unwrap();
    let chest = male.iter().find(|s| s.name == "Upper_torso").unwrap();
    assert!(chest.priority > head.priority, "ours: the torso first");
}

/// 34. Burst defaults: a record with `TripleShotMode=1` and no other
/// `TripleShot*` key → kick 1.0, cap 50, cone 7/100, speeds 1.0/1.0,
/// delays 300/400 ms, 3 rounds; a hand-written document that leaves them
/// out gets the same (Corrections 2).
#[test]
fn case_34_burst_defaults() {
    let f = weapons(r#"[Ammo0] Name="a" [Weapon0] Name="W" AmmoName0="a" TripleShotMode=1"#).unwrap();
    let w = &f.weapons[0];
    assert_eq!((w.triple_shot_base_cam_recoil_pitch, w.triple_shot_max_cam_recoil_pitch), (1.0, 50.0));
    assert_eq!((w.triple_shot.min, w.triple_shot.max, w.triple_shot.inc, w.triple_shot.dec), (7, 100, 1.0, 1.0));
    assert_eq!((w.triple_shot_delay, w.triple_shot_select_delay, w.triple_shot_round_num), (300, 400, 3));
    use combat_arms::game::weapons::{RecoilParams, SpreadParams};
    let r: RecoilParams = toml::from_str("").unwrap();
    assert_eq!((r.burst_pitch, r.burst_max_pitch), (1.0, 50.0));
    let s: SpreadParams = toml::from_str("cone = { min = 3, max = 100, inc = 1.3, dec = 0.3 }\nstate = {}").unwrap();
    let b = s.burst_cone.unwrap();
    assert_eq!((b.min, b.max, b.inc, b.dec), (7, 100, 1.0, 1.0));
}

/// 35. Zoomed defaults: a record with no `Zoomed*` key → 1/500, 3.0/3.0;
/// the same for a hand-written document.
#[test]
fn case_35_zoomed_defaults() {
    let f = weapons(r#"[Ammo0] Name="a" [Weapon0] Name="W" AmmoName0="a""#).unwrap();
    let w = &f.weapons[0];
    assert_eq!((w.zoomed.min, w.zoomed.max, w.zoomed.inc, w.zoomed.dec), (1, 500, 3.0, 3.0));
    let s: combat_arms::game::weapons::SpreadParams =
        toml::from_str("cone = { min = 3, max = 100, inc = 1.3, dec = 0.3 }\nstate = {}").unwrap();
    let z = s.zoomed_cone.unwrap();
    assert_eq!((z.min, z.max, z.inc, z.dec), (1, 500, 3.0, 3.0));
}

/// 36. Server settings: no `[TimeWeight]` → `ReloadCheckTime` 0.8;
/// `ReloadCheckTime=1.0` → the window is the third-person reload at
/// rate 1.
#[test]
fn case_36_server_settings() {
    assert_eq!(ServerTimeWeight::read(&doc(r#"[Player] WalkSpeed=70.0"#)).reload_check_time, 0.8);
    let t = ServerTimeWeight::read(&doc(r#"[TimeWeight] ReloadCheckTime=1.0"#));
    assert_eq!(t.reload_check_time, 1.0);
    close(combat_arms::game::weapons::server_reload_mark(2.4, 1.0, t.reload_check_time), 2.4);
    let (attrs, _) = read_attributes(|name| {
        if name == SERVER_FILE {
            Ok(b"[TimeWeight] ReloadCheckTime=0.5".to_vec())
        } else {
            Err(std::io::Error::other("absent"))
        }
    });
    assert_eq!(attrs.time_weight.map(|t| t.reload_check_time), Some(0.5));
}

/// An install weapon copied from ours keeps our document's gun class (so
/// an automatic gun game's ladder takes it: `class` in the data), but a
/// thrown one copied from our frag doesn't: an install smoke isn't a
/// grenade that kills.
#[test]
fn install_copies_keep_a_guns_class_not_a_grenades() {
    use chimera_core::game_parts::GunClass;
    let install = weapons(
        r#"[Ammo0] Name="556" [Ammo1] Name="frag" [Ammo2] Name="smoke"
        [Weapon0] Name="M4A1" Guntype=4 AmmoName0="556"
        [Weapon1] Name="AK-47" Guntype=4 AmmoName0="556"
        [Weapon2] Name="M67" Guntype=7 AmmoName0="frag"
        [Weapon3] Name="Smoke X" Guntype=7 AmmoName0="smoke""#,
    )
    .unwrap();
    let docs = merge_weapon_docs(
        &[
            ("m4a1.toml", include_str!("../../data/weapons/m4a1.toml")),
            ("m67.toml", include_str!("../../data/weapons/m67.toml")),
        ],
        &install,
        meters_per_unit(),
    )
    .unwrap();
    let refs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
    let set = WeaponSet::parse(&refs, &[]).unwrap();
    let class = |id: &str| set.get(id).unwrap_or_else(|| panic!("{id}")).class;
    assert_eq!(class("combat_arms:m4a1"), Some(GunClass::Rifle));
    assert_eq!(class("combat_arms:ak-47"), Some(GunClass::Rifle), "a copy keeps a gun's class");
    assert_eq!(class("combat_arms:m67"), Some(GunClass::Grenade));
    assert_eq!(class("combat_arms:smoke_x"), None, "a thrown copy isn't a lethal grenade");
}

/// `CaData::install_weapon` (an index by id) finds the record the search
/// finds (`install_weapon_in`): slugs, the two names that aren't their
/// slugs, a name twice (the first wins), an empty name, unknown ids and
/// ids without the namespace.
#[test]
fn the_install_weapon_index_finds_what_the_search_finds() {
    use combat_arms::game::data::{CaData, install_weapon_in};
    let text = r#"[Ammo0] Name="a"
        [Weapon0] Name="M60" AmmoName0="a" Guntype=5
        [Weapon1] Name="5.56mm M4A1" AmmoName0="a" Guntype=4
        [Weapon2] Name="M9" AmmoName0="a" Guntype=0
        [Weapon3] Name="M-23F" AmmoName0="a" Guntype=7
        [Weapon4] Name="m60" AmmoName0="a" Guntype=6
        [Weapon5] Name="" AmmoName0="a" Guntype=1
        [Weapon6] Name="L96A1 (Event)" AmmoName0="a" Guntype=6
        [Weapon7] Name="m9_knife" AmmoName0="a" Guntype=3
        [Weapon8] Name="M9" AmmoName0="a" Guntype=2"#;
    let file = weapons(text).unwrap();
    let ca = CaData::with_attributes(InstallAttributes {
        weapons: Some(file.clone()),
        ..Default::default()
    });
    let mut found = 0;
    for id in [
        "combat_arms:m60",
        "combat_arms:5.56mm_m4a1",
        "combat_arms:m9",
        "combat_arms:m9_knife",
        "combat_arms:m23f",
        "combat_arms:m-23f",
        "combat_arms:l96a1__event_",
        "combat_arms:",
        "combat_arms:nothing",
        "combat_arms:M60",
        "m60",
        "cs_source:m60",
        "",
    ] {
        let want = install_weapon_in(&file, id);
        let got = ca.install_weapon(id);
        assert_eq!(got.map(|r| (&r.name, r.gun_type)), want.map(|r| (&r.name, r.gun_type)), "{id}");
        found += got.is_some() as usize;
        // Twice: the index is made once.
        assert_eq!(ca.install_weapon(id).map(|r| r.gun_type), want.map(|r| r.gun_type), "{id}");
    }
    assert!(found >= 5, "{found} found");
    // The first record of a name.
    assert_eq!(ca.install_weapon("combat_arms:m60").map(|r| r.gun_type), Some(5));
}
