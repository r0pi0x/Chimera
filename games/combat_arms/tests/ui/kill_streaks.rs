//! specs/combat_arms/kill_streaks.md's weapon-skin cases (A8, B7: 29,
//! 30, 31, 41, 54), on Combat Arms' pack with the skins read from an
//! install's attribute files (`data::read_attributes`: each weapon's
//! `HUDEffectOverride` names a section of the layout and client files),
//! through the shell's choice of callout (`client::announcer::requests`)
//! by the killing weapon's id. The files here are written in the test,
//! with the spec's key and file names.

use chimera_client::client::announcer::requests;
use chimera_core::announcer::{AnnouncerAppExt, AnnouncerPack, AnnouncerPacks, KillFacts, SpecialShot};
use combat_arms::game::data::{
    attributes::{CLIENT_FILE, LAYOUT_FILE, WEAPONS_FILE},
    read_attributes,
};

const WEAPONS: &str = r#"
[Ammo0] Name="a"
[Weapon0] Name="PSY Rifle" AmmoName0="a" HUDEffectOverride="PSYWeaponEffect"
[Weapon1] Name="Robo Rifle" AmmoName0="a" HUDEffectOverride="TMWeaponEffect"
[Weapon2] Name="Phantom Rifle" AmmoName0="a" HUDEffectOverride="PhantomWeaponEffect"
[Weapon3] Name="Graffiti Rifle" AmmoName0="a" HUDEffectOverride="GraffitiWeaponEffect"
[Weapon4] Name="PSY Pistol" AmmoName0="a" HUDEffectOverride="PSYWeaponEffect"
[Weapon5] Name="Plain Rifle" AmmoName0="a"
"#;

const LAYOUT: &str = r#"
[HUDLayout0]
MKComboDoubleKillImg="UI_HUD_Message\Effect\DoubleKill.dtx"
[PSYWeaponEffect]
MKComboDoubleKillImg="UI_HUD_Message\Effect\DoubleKill_PSY.dtx"
MKRevengeImg="UI_HUD_Message\Effect\Revenge_PSY.dtx"
MKComboOneKillImg="UI_HUD_Message\Effect\Kill_PSY.dtx"
[TMWeaponEffect]
MKComboHeadShotImg="UI_HUD_Message\Effect\HeadShot_RoboSkull.dtx"
MKComboSkullImg="UI_HUD_Message\Effect\Head_RoboSkull.dtx"
[PhantomWeaponEffect]
MKComboDoubleKillImg="UI_HUD_Message\Effect\DoubleKill_phantom.dtx"
MKComboHeadShotImg="UI_HUD_Message\Effect\HeadShot_phantom.dtx"
[GraffitiWeaponEffect]
MKComboOneKillImg="UI_HUD_Message\Effect\Kil_Graffiti.dtx"
MKComboOneKillsImg="UI_HUD_Message\Effect\Kills_Graffiti.dtx"
"#;

const CLIENT: &str = r#"
[Interface]
DoubleKillSound="UI_SND\doublekill.wav"
[PSYWeaponEffect]
RevengeSound="UI_SND\Revenge_PSY.wav"
HeatShotKillSound="UI_SND\HeadShot_PSY.wav"
[TMWeaponEffect]
HeatShotKillSound="UI_SND\TM_Headshot.wav"
"#;

/// Combat Arms' pack with the skins the install's files give.
fn pack() -> AnnouncerPack {
    let (attrs, problems) = read_attributes(|name| match name {
        WEAPONS_FILE => Ok(WEAPONS.as_bytes().to_vec()),
        LAYOUT_FILE => Ok(LAYOUT.as_bytes().to_vec()),
        CLIENT_FILE => Ok(CLIENT.as_bytes().to_vec()),
        _ => Err(std::io::Error::new(std::io::ErrorKind::NotFound, name.to_string())),
    });
    assert!(attrs.weapons.is_some(), "{problems:?}");
    // The skins before the pack registers (the plugins' order varies).
    let mut app = bevy::prelude::App::new();
    app.announcer_skins("combat_arms", attrs.announcer_skins.clone())
        .register_announcer(combat_arms::game::parts::ANNOUNCER);
    let packs = app.world().resource::<AnnouncerPacks>();
    (*packs.get("combat_arms").unwrap().clone()).clone()
}

fn kill(multi: u8, shot: SpecialShot) -> KillFacts {
    KillFacts {
        pack: "combat_arms".into(),
        multi,
        shot,
        total: 5,
        ..Default::default()
    }
}

const EFFECT: &str = "UI_HUD_Message/Effect/";

/// Who wears which skin: the weapons naming it, by their ids; a weapon
/// without one wears none.
#[test]
fn skins_by_hud_effect_override() {
    let p = pack();
    assert_eq!(p.skin("combat_arms:psy_rifle").unwrap().name, "PSYWeaponEffect");
    assert_eq!(p.skin("combat_arms:psy_pistol").unwrap().name, "PSYWeaponEffect", "one section, two weapons");
    assert_eq!(p.skin("combat_arms:robo_rifle").unwrap().name, "TMWeaponEffect");
    assert!(p.skin("combat_arms:plain_rifle").is_none());
    let names: Vec<&str> = p.skins.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["PSYWeaponEffect", "TMWeaponEffect", "PhantomWeaponEffect", "GraffitiWeaponEffect"]);
}

/// 29. A PSY-skin double kill: `DoubleKill_PSY.dtx`; the voice the
/// pack's `doublekill.wav` (the PSY skin has no double-kill voice).
#[test]
fn t29_psy_double_kill() {
    let r = requests(&pack(), &kill(2, SpecialShot::None), Some("combat_arms:psy_rifle")).unwrap();
    assert_eq!(r.image, format!("{EFFECT}DoubleKill_PSY.dtx"));
    assert_eq!(r.voice.as_deref(), Some("UI_SND/doublekill.wav"));
    // Without the skin: the pack's.
    let r = requests(&pack(), &kill(2, SpecialShot::None), Some("combat_arms:plain_rifle")).unwrap();
    assert_eq!(r.image, format!("{EFFECT}DoubleKill.dtx"));
}

/// 30. A TM-skin head kill at count 1: `HeadShot_RoboSkull.dtx`, no top
/// image, `TM_Headshot.wav`; at count 2: the pack's `DoubleKill.dtx` with
/// the skin's skull `Head_RoboSkull.dtx` over it, `doublekill.wav` only.
#[test]
fn t30_tm_headshot() {
    let w = Some("combat_arms:robo_rifle");
    let r = requests(&pack(), &kill(1, SpecialShot::Head), w).unwrap();
    assert_eq!(r.image, format!("{EFFECT}HeadShot_RoboSkull.dtx"));
    assert_eq!(r.top, None);
    assert_eq!(r.voice.as_deref(), Some("UI_SND/TM_Headshot.wav"));
    let r = requests(&pack(), &kill(2, SpecialShot::Head), w).unwrap();
    assert_eq!(r.image, format!("{EFFECT}DoubleKill.dtx"));
    assert_eq!(r.top, Some(format!("{EFFECT}Head_RoboSkull.dtx")));
    assert_eq!(r.voice.as_deref(), Some("UI_SND/doublekill.wav"));
}

/// 31. A Phantom-skin head kill at count 2: `DoubleKill_phantom.dtx`,
/// the pack's skull (the section has no skull key), `doublekill.wav`.
#[test]
fn t31_phantom_double_headshot() {
    let r = requests(&pack(), &kill(2, SpecialShot::Head), Some("combat_arms:phantom_rifle")).unwrap();
    assert_eq!(r.image, format!("{EFFECT}DoubleKill_phantom.dtx"));
    assert_eq!(r.top, Some(format!("{EFFECT}Head.dtx")));
    assert_eq!(r.voice.as_deref(), Some("UI_SND/doublekill.wav"));
}

/// 41. A PSY-skin single revenge kill, body hit: the skin's revenge image
/// and its revenge line.
#[test]
fn t41_psy_revenge() {
    let facts = KillFacts {
        revenge: true,
        ..kill(1, SpecialShot::None)
    };
    let r = requests(&pack(), &facts, Some("combat_arms:psy_pistol")).unwrap();
    assert_eq!(r.image, format!("{EFFECT}Revenge_PSY.dtx"));
    assert_eq!(r.voice.as_deref(), Some("UI_SND/Revenge_PSY.wav"));
}

/// 54. A Graffiti-skin plain kill at N = 1: the skin's "KILL" image (a
/// file the install lacks: drawn blank, Quirk 5) with the number 1.
#[test]
fn t54_graffiti_plain_kill() {
    let facts = KillFacts {
        total: 1,
        ..kill(1, SpecialShot::None)
    };
    let r = requests(&pack(), &facts, Some("combat_arms:graffiti_rifle")).unwrap();
    assert_eq!(r.image, format!("{EFFECT}Kil_Graffiti.dtx"));
    assert_eq!(r.number, Some(1));
    assert_eq!(r.voice, None);
}
