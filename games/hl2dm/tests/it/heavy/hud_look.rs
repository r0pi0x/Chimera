//! HL2:DM's HUD look from the install (specs/hl2dm/hud.md, checks I1 to
//! I6, test cases 2 and 3): the layout's numbers, the scheme's first
//! colours, the fonts by height and their files, the death and weapon
//! icons, the localised strings and the animation script the look carries
//! (`game::hud::load`). Skips without an install.

use chimera_world::map::hud::{GameHud, font_for_height};
use hl2dm::game::hud;
use source_engine::content::material::MaterialLoader;

use super::map_stock::install_mount;

fn look() -> Option<GameHud> {
    let mount = install_mount()?;
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    Some(hud::load(&mut materials).expect("the look loads"))
}

/// I1 and test 2: the layout's numbers and the scheme's first colours.
#[test]
fn the_layout_and_scheme_are_the_specs() {
    let Some(h) = look() else { return };
    let num = |p: &str, k: &str| h.panels[p].num(k).unwrap_or_else(|| panic!("{p} {k}"));
    assert_eq!(num("HudSuitPower", "BarWidth"), 92.0);
    assert_eq!(num("HudWeaponSelection", "LargeBoxWide"), 112.0);
    assert_eq!(num("HudHistoryResource", "history_gap"), 56.0);
    assert_eq!(num("HudDamageIndicator", "dmg_tall2"), 200.0);
    let (hp, ammo) = (&h.panels["HudHealth"], &h.panels["HudAmmo"]);
    assert_eq!((hp.wide, hp.tall), (102.0, 36.0), "the PC panel, not the Deck's");
    assert_eq!((ammo.wide, ammo.tall), (136.0, 36.0));
    // The first of a name listed twice (Open questions 1).
    assert_eq!(h.colors["FgColor"], [255, 220, 0, 100]);
    assert_eq!(h.colors["BgColor"], [0, 0, 0, 76]);
    assert_eq!(h.colors["SelectionSelectedBoxBg"], [0, 0, 0, 80]);
    assert_eq!(h.colors["Normal"], [255, 208, 64, 255]);
    assert_eq!(h.colors["Caution"], [255, 48, 0, 255]);
}

/// Test 3 and 4: the font entries by height and their files (HL2:DM's
/// reduced HalfLife2, GorDIN by weight, the crosshairs from Half-Life 2).
#[test]
fn the_fonts_follow_the_screen_height() {
    let Some(h) = look() else { return };
    let at = |name: &str, height: f32| font_for_height(&h.text_fonts[name], height).unwrap().clone();
    let e = at("Default", 1080.0);
    assert_eq!((e.family.as_str(), e.tall, e.weight, e.additive), ("GorDIN", 21.0, 700, false));
    let e = at("Default", 1440.0);
    assert!(e.additive);
    assert_eq!(at("Default", 600.0).tall, 14.0);
    assert_eq!(at("Crosshairs", 1080.0).tall, 40.0);
    assert_eq!(at("Crosshairs", 2160.0).tall, 80.0);
    for key in ["file:gordin-bold", "file:gordin-light", "file:halflife2", "file:hl2mp", "file:hl2crosshairs", "family:halflife2", "family:hl2cross"] {
        assert!(h.fonts.contains_key(key), "{key} in {:?}", h.fonts.keys().collect::<Vec<_>>());
    }
}

/// I4, I5: the icons the HUD draws (the death notices', the weapons',
/// the battery and kit, the default crosshair) and the strings.
#[test]
fn the_icons_and_strings_are_there() {
    let Some(h) = look() else { return };
    for (weapon, glyph) in [("shotgun", '0'), ("smg1", '/'), ("crowbar", '6'), ("pistol", '-'), ("357", '.'), ("slam", '*')] {
        let icon = h.icons.get(&format!("death_{weapon}")).unwrap_or_else(|| panic!("death_{weapon}"));
        assert_eq!(*icon, ("HL2MPTypeDeath".to_string(), glyph), "{weapon}");
    }
    assert!(h.sprites.contains_key("d_skull"));
    assert_eq!(h.icons["item_battery"].1, '*');
    assert_eq!(h.icons["item_healthkit"].1, '+');
    assert!(!h.icons.contains_key("item_healthvial"), "no icon for the vial");
    let d = &h.sprites["crosshair_default"];
    assert_eq!(&d.rect, &[0.0, 48.0, 24.0, 24.0]);
    // A weapon's own icons.
    assert!(h.icons.contains_key(&hud::icon_key("weapon_smg1", "weapon")), "the SMG1's glyph");
    assert!(h.icons.contains_key(&hud::icon_key("weapon_pistol", "crosshair")) || h.sprites.contains_key(&hud::icon_key("weapon_pistol", "crosshair")));
    let strings = &h.menus.as_ref().unwrap().strings;
    assert_eq!(strings["valve_hud_health"], "HEALTH");
    assert_eq!(strings["hl2_smg1"], "SMG\n(SUBMACHINE GUN)");
    // The chat box (the basechat's).
    let c = &h.panels["HudChat"];
    assert_eq!((c.wide, c.tall), (280.0, 120.0));
}

/// The animation script the look carries parses into the events the HUD
/// starts (an event the file lacks does nothing, so all of these must be
/// there).
#[test]
fn the_animation_script_has_every_event_the_hud_starts() {
    let Some(h) = look() else { return };
    let script = hud::animations(&h);
    for event in [
        "HealthIncreasedAbove20", "HealthIncreasedBelow20", "HealthLow", "HealthPulse", "HealthLoop", "HealthDamageTaken",
        "SuitDamageTaken", "SuitArmorLow", "SuitPowerZero", "SuitPowerIncreasedAbove20", "SuitPowerIncreasedBelow20",
        "AmmoEmpty", "AmmoDecreased", "AmmoIncreased", "WeaponUsesClips", "WeaponDoesNotUseClips", "WeaponChanged",
        "WeaponUsesSecondaryAmmo", "WeaponDoesNotUseSecondaryAmmo", "AmmoSecondaryEmpty", "AmmoSecondaryDecreased",
        "AmmoSecondaryIncreased", "SuitAuxPowerMax", "SuitAuxPowerNotMax", "SuitAuxPowerDecreasedBelow25",
        "SuitAuxPowerIncreasedAbove25", "SuitAuxPowerNoItemsActive", "SuitAuxPowerOneItemActive",
        "SuitAuxPowerTwoItemsActive", "SuitAuxPowerThreeItemsActive", "FadeOutTeamLine", "FadeInTeamLine",
        "OpenWeaponSelectionMenu", "CloseWeaponSelectionMenu", "FadeOutWeaponSelectionMenu", "HudTakeDamageLeft",
        "HudTakeDamageRight", "HudTakeDamageBehind", "HudTakeDamageHighLeft", "HudTakeDamageHighRight",
        "HudTakeDamageHigh", "HudTakeDamageDrown", "HudTakeDamagePoison", "HudTakeDamageBurn", "HudTakeDamageRadiation",
        "HudPlayerDeath",
    ] {
        assert!(script.events.contains_key(&event.to_lowercase()), "{event}");
    }
    // The deck's variants are left out: one WeaponUsesClips, to r150.
    let clips = &script.events["weaponusesclips"];
    assert_eq!(clips.len(), 2);
}

/// The damage indicator's full-screen red (the dead view's) is added to the
/// scene through `vgui/white_additive` (hud.md 8); without the picture the
/// HUD falls back to a plain blend, which ends in a solid red (M9's note:
/// the material names no `$basetexture`, the look loads `vgui/white`).
#[test]
fn the_additive_picture_is_there() {
    let Some(hud) = look() else { return };
    assert!(hud.sprites.contains_key("white_additive"), "the dead view would be a solid red");
}

/// The install's script (hud.md 4.6's table): switching to a weapon with a
/// secondary ammo type shows the panel with its count at r76 and moves the
/// ammo panel to r222; back to one without hides it.
#[cfg(feature = "ui")]
#[test]
fn the_install_shows_the_secondary_panel() {
    use hl2dm::ui::{frame::{Draw, Screen}, hud::{Hl2Hud, Inputs, WeaponNow}, look::Look};
    let Some(h) = look() else { return };
    let script = hud::animations(&h);
    let look = Look::new(std::sync::Arc::new(h), Screen::new(640.0, 480.0));
    let mut hud = Hl2Hud::new(script, &look);
    hud.reset(&look);
    let held = |class: &str, secondary: Option<(String, u32)>| Inputs {
        alive: true,
        health: 100.0,
        aux: 100.0,
        fov: 90.0,
        weapon: Some(WeaponNow { class: class.into(), uses_primary: true, clip: Some(18), max_clip: 18, reserve: 150, secondary }),
        ..Inputs::default()
    };
    let (pistol, smg) = (held("weapon_pistol", None), held("weapon_smg1", Some(("hl2dm:smg1_grenade".into(), 3))));
    let mut t = 0.0;
    let mut run = |hud: &mut Hl2Hud, inp: &Inputs| {
        for _ in 0..60 {
            t += 0.05;
            hud.think(inp, t);
        }
    };
    let digits = |hud: &Hl2Hud, inp: &Inputs| match hud.frame(&look, inp).find_text("3") {
        Some(Draw::Text { at, .. }) => Some(*at),
        _ => None,
    };
    run(&mut hud, &pistol);
    assert_eq!(digits(&hud, &pistol), None);
    run(&mut hud, &smg);
    assert_eq!(hud.anim.number("HudAmmoSecondary", "Alpha"), Some(255.0));
    let at = digits(&hud, &smg).expect("the count drawn");
    assert!(at.x > 640.0 - 76.0, "in the panel at r76: {at}");
    assert_eq!(hud.anim.get("HudAmmo", "Position").and_then(|v| v.pair()), Some([418.0, 432.0]));
    run(&mut hud, &pistol);
    assert_eq!(hud.anim.number("HudAmmoSecondary", "Alpha"), Some(0.0));
}

/// The playtest's cut-off crosshair brackets: the crosshairs font fills its
/// cell (no Windows descent; its hhea says otherwise), so a bracket's ink
/// is the cell's, centred on the crosshair when drawn at cy - h/2 (6.2).
#[cfg(feature = "ui")]
#[test]
fn the_brackets_fill_their_cell() {
    use chimera_client::client::hud_text::{Align, GlyphFont, HudText, ink_rect};
    use bevy::prelude::*;
    let Some(h) = look() else { return };
    let e = font_for_height(&h.text_fonts["QuickInfo"], 1080.0).unwrap();
    let family = e.family.to_lowercase();
    let file = h.fonts.get(&format!("file:{family}")).or_else(|| h.fonts.get(&format!("family:{family}"))).expect("its file");
    let tall = 63.0;
    for glyph in ["[", "]", "{", "}", "(", ")"] {
        let text = HudText {
            font: GlyphFont { data: file.data.clone(), additive: true },
            tall,
            text: glyph.into(),
            color: Color::WHITE,
            at: Vec2::new(100.0, 100.0),
            align: Align::Left,
        };
        let ink = ink_rect(&text).expect("ink");
        assert!(ink.min.y >= 99.0 && ink.max.y <= 100.0 + tall + 1.0, "{glyph}: {ink:?} outside the cell");
        assert!((ink.center().y - (100.0 + tall / 2.0)).abs() <= 1.5, "{glyph}: {ink:?} not centred");
    }
}
