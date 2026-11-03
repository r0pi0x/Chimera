//! What the HUD tests share: a stand-in look (the spec's layout, colours
//! and font sizes), an animation script with the events the tests run (the
//! spec's numbers, section 3 to 9, in our own words: the install's file is
//! the install's), and builders for the HUD's inputs.

use bevy::prelude::*;
use hl2dm::ui::{
    frame::Screen,
    hud::{Hl2Hud, Inputs, Owned, WeaponNow},
    look::{Look, spec_look},
};
use source_engine::hud::anim::Script;

/// The events the tests run.
pub const SCRIPT: &str = r#"
event HealthIncreasedAbove20
{
	StopEvent HealthLoop 0
	StopEvent HealthPulse 0
	StopEvent HealthLow 0
	Animate HudHealth BgColor "BgColor" Linear 0 0
	Animate HudHealth FgColor "FgColor" Linear 0 0.03
	Animate HudHealth Blur "3" Linear 0 0.1
	Animate HudHealth Blur "0" Deaccel 0.1 2.0
}
event HealthIncreasedBelow20
{
	Animate HudHealth FgColor "BrightFg" Linear 0 0.25
	Animate HudHealth FgColor "FgColor" Linear 0.3 0.75
	Animate HudHealth Blur "3" Linear 0 0.1
	Animate HudHealth Blur "0" Deaccel 0.1 2.0
}
event HealthLow
{
	StopEvent HealthDamageTaken 0
	StopEvent HealthPulse 0
	StopEvent HealthLoop 0
	Animate HudHealth BgColor "DamagedBg" Linear 0 0.1
	Animate HudHealth BgColor "BgColor" Deaccel 0.1 1.75
	Animate HudHealth FgColor "BrightFg" Linear 0 0.2
	Animate HudHealth FgColor "DamagedFg" Linear 0.2 1.2
	Animate HudHealth Blur "5" Linear 0 0.1
	Animate HudHealth Blur "3" Deaccel 0.1 0.9
	RunEvent HealthPulse 1.0
}
event HealthPulse
{
	Animate HudHealth Blur "5" Linear 0 0.1
	Animate HudHealth Blur "2" Deaccel 0.1 0.8
	Animate HudHealth BgColor "100 0 0 80" Linear 0 0.1
	Animate HudHealth BgColor "BgColor" Deaccel 0.1 0.8
	RunEvent HealthLoop 0.8
}
event HealthLoop
{
	RunEvent HealthPulse 0
}
event HealthDamageTaken
{
	Animate HudHealth FgColor "BrightFg" Linear 0 0.25
	Animate HudHealth FgColor "FgColor" Linear 0.3 0.75
	Animate HudHealth Blur "3" Linear 0 0.1
	Animate HudHealth Blur "0" Deaccel 0.1 2.0
}
event SuitDamageTaken
{
	Animate HudSuit FgColor "BrightFg" Linear 0 0.25
	Animate HudSuit FgColor "FgColor" Linear 0.3 0.75
}
event SuitArmorLow
{
	StopEvent SuitDamageTaken 0
	Animate HudSuit FgColor "BrightFg" Linear 0 0.25
	Animate HudSuit FgColor "FgColor" Linear 0.3 0.75
}
event SuitPowerZero
{
	Animate HudSuit Alpha "0" Linear 0 0.4
}
event SuitPowerIncreasedAbove20
{
	StopEvent SuitPowerZero 0
	Animate HudSuit Alpha "255" Linear 0 0
	Animate HudSuit BgColor "BgColor" Linear 0 0
	Animate HudSuit FgColor "FgColor" Linear 0 0.05
}
event SuitPowerIncreasedBelow20
{
	StopEvent SuitPowerZero 0
	Animate HudSuit Alpha "255" Linear 0 0
	Animate HudSuit FgColor "BrightFg" Linear 0 0.1
	Animate HudSuit FgColor "FgColor" Linear 0.2 0.5
}
event AmmoEmpty
{
	Animate HudAmmo FgColor "BrightDamagedFg" Linear 0 0.2
	Animate HudAmmo FgColor "DamagedFg" Accel 0.2 1.2
}
event AmmoDecreased
{
	StopEvent AmmoIncreased 0
	Animate HudAmmo Blur "7" Linear 0 0
	Animate HudAmmo Blur "0" Deaccel 0.1 1.5
}
event AmmoIncreased
{
	Animate HudAmmo FgColor "BrightFg" Linear 0 0.15
	Animate HudAmmo FgColor "FgColor" Deaccel 0.15 1.5
	Animate HudAmmo Blur "5" Linear 0 0
	Animate HudAmmo Blur "0" Accel 0.01 1.5
}
event WeaponUsesClips
{
	Animate HudAmmo Position "r150 432" Deaccel 0 0.4
	Animate HudAmmo Size "132 36" Deaccel 0 0.4
}
event WeaponDoesNotUseClips
{
	Animate HudAmmo Position "r118 432" Deaccel 0 0.4
	Animate HudAmmo Size "100 36" Deaccel 0 0.4
}
event WeaponChanged
{
	Animate HudAmmo BgColor "250 220 0 80" Linear 0 0.1
	Animate HudAmmo BgColor "BgColor" Deaccel 0.1 1.0
	Animate HudAmmo FgColor "BrightFg" Linear 0 0.1
	Animate HudAmmo FgColor "FgColor" Linear 0.2 1.5
}
event WeaponUsesSecondaryAmmo
{
	StopAnimation HudAmmo Position 0
	StopAnimation HudAmmo Size 0
	StopPanelAnimations HudAmmoSecondary 0
	Animate HudAmmoSecondary Alpha "255" Linear 0 0.1
	Animate HudAmmo Position "r222 432" Deaccel 0 0.5
	Animate HudAmmo Size "132 36" Deaccel 0 0.4
}
event WeaponDoesNotUseSecondaryAmmo
{
	StopPanelAnimations HudAmmoSecondary 0
	Animate HudAmmoSecondary Alpha "0" Linear 0 0.1
}
event SuitAuxPowerMax
{
	Animate HudSuitPower BgColor "0 0 0 0" Linear 0 0.4
	Animate HudSuitPower AuxPowerColor "0 0 0 0" Linear 0 0.4
}
event SuitAuxPowerNotMax
{
	Animate HudSuitPower BgColor "BgColor" Linear 0 0.4
	Animate HudSuitPower AuxPowerColor "255 220 0 220" Linear 0 0.4
}
event SuitAuxPowerDecreasedBelow25
{
	Animate HudSuitPower AuxPowerColor "255 0 0 220" Linear 0 0.4
}
event SuitAuxPowerIncreasedAbove25
{
	Animate HudSuitPower AuxPowerColor "255 220 0 220" Linear 0 0.4
}
event SuitAuxPowerNoItemsActive
{
	Animate HudSuitPower Size "102 26" Linear 0 0.4
	Animate HudSuitPower Position "16 400" Linear 0 0.4
}
event SuitAuxPowerOneItemActive
{
	Animate HudSuitPower Size "102 36" Linear 0 0.4
	Animate HudSuitPower Position "16 390" Linear 0 0.4
}
event FadeOutTeamLine
{
	Animate TeamDisplay Alpha "0" Linear 0 0.25
}
event FadeInTeamLine
{
	Animate TeamDisplay Alpha "255" Linear 0 0.5
}
event OpenWeaponSelectionMenu
{
	StopEvent CloseWeaponSelectionMenu 0
	StopEvent FadeOutWeaponSelectionMenu 0
	Animate HudWeaponSelection Alpha "128" Linear 0 0.1
	Animate HudWeaponSelection SelectionAlpha "255" Linear 0 0.1
	Animate HudWeaponSelection FgColor "FgColor" Linear 0 0.1
	Animate HudWeaponSelection TextColor "BrightFg" Linear 0 0.1
	Animate HudWeaponSelection TextScan "1" Linear 0 0.1
}
event FadeOutWeaponSelectionMenu
{
	Animate HudWeaponSelection FgColor "0 0 0 0" Linear 0.5 1.0
	Animate HudWeaponSelection TextColor "0 0 0 0" Linear 0.5 1.0
	Animate HudWeaponSelection Alpha "0" Linear 0.5 1.0
	Animate HudWeaponSelection SelectionAlpha "0" Linear 0.5 1.0
}
event CloseWeaponSelectionMenu
{
	Animate HudWeaponSelection FgColor "0 0 0 0" Linear 0 0.1
	Animate HudWeaponSelection TextColor "0 0 0 0" Linear 0 0.1
	Animate HudWeaponSelection Alpha "0" Linear 0 0.1
	Animate HudWeaponSelection SelectionAlpha "0" Linear 0 0.1
}
event HudTakeDamageLeft
{
	Animate HudDamageIndicator DmgColorLeft "255 88 0 200" Linear 0 0
	Animate HudDamageIndicator DmgColorLeft "255 0 0 200" Linear 0 0.3
	Animate HudDamageIndicator DmgColorLeft "255 0 0 0" Deaccel 0.3 0.5
}
event HudTakeDamageRight
{
	Animate HudDamageIndicator DmgColorRight "255 88 0 200" Linear 0 0
	Animate HudDamageIndicator DmgColorRight "255 0 0 200" Linear 0 0.3
	Animate HudDamageIndicator DmgColorRight "255 0 0 0" Deaccel 0.3 0.5
}
event HudTakeDamageBehind
{
	RunEvent HudTakeDamageLeft 0
	RunEvent HudTakeDamageRight 0
}
event HudTakeDamageHighLeft
{
	Animate HudDamageIndicator DmgHighColorLeft "255 88 0 255" Linear 0 0
	Animate HudDamageIndicator DmgHighColorLeft "255 0 0 200" Linear 0 0.4
	Animate HudDamageIndicator DmgHighColorLeft "255 0 0 0" Deaccel 0.4 2.4
}
event HudTakeDamageHigh
{
	Animate HudDamageIndicator DmgFullscreenColor "255 88 0 200" Linear 0 0
	Animate HudDamageIndicator DmgFullscreenColor "255 0 0 200" Linear 0 0.4
	Animate HudDamageIndicator DmgFullscreenColor "255 0 0 0" Deaccel 0.4 2.4
}
event HudPlayerDeath
{
	StopEvent HealthLoop 0
	StopEvent HealthPulse 0
	Animate HudDamageIndicator DmgFullscreenColor "255 0 0 96" Deaccel 0.1 0.2
	Animate HudDamageIndicator DmgFullscreenColor "255 0 0 255" Deaccel 0.3 4.0
}
"#;

/// A HUD at `screen` on the stand-in look.
pub fn setup(screen: Screen) -> (Hl2Hud, Look) {
    let look = spec_look(screen);
    let script = Script::parse(SCRIPT, &|n| look.hud.colors.get(n).copied());
    (Hl2Hud::new(script, &look), look)
}

/// A HUD at 640 by 480.
pub fn base() -> (Hl2Hud, Look) {
    setup(Screen::base())
}

pub fn entity(n: u32) -> Entity {
    Entity::from_raw_u32(n).unwrap()
}

/// A living player with `health` and `armour`, the pistol in hand.
pub fn inputs() -> Inputs {
    Inputs { alive: true, health: 100.0, armour: 0.0, aux: 100.0, fov: 90.0, crosshair: true, quickinfo: true, weapon: Some(pistol(18, 150)), ..Inputs::default() }
}

pub fn pistol(clip: i32, reserve: i32) -> WeaponNow {
    WeaponNow { class: "weapon_pistol".into(), uses_primary: true, clip: Some(clip), max_clip: 18, reserve, ..WeaponNow::default() }
}

pub fn smg1(clip: i32, reserve: i32) -> WeaponNow {
    WeaponNow { class: "weapon_smg1".into(), uses_primary: true, clip: Some(clip), max_clip: 45, reserve, ..WeaponNow::default() }
}

pub fn owned(e: u32, class: &str, bucket: u8, position: u8, selectable: bool, name: &str) -> Owned {
    Owned { entity: entity(e), class: class.into(), bucket, position, selectable, print_name: name.into() }
}

/// Run the HUD for `seconds` in `step`s with `inp`, from its clock.
pub fn run(hud: &mut Hl2Hud, inp: &Inputs, seconds: f64) {
    let start = hud.now;
    let mut t = start;
    while t < start + seconds - 1e-9 {
        t += 0.05;
        hud.think(inp, t);
    }
}
