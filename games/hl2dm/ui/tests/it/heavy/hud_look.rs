//! The install's HUD look as `hl2dm_ui` draws it (specs/hl2dm/hud.md 4.6,
//! 6.2): the secondary-ammo panel from the script, the crosshair brackets'
//! glyphs. The look's own numbers are `hl2dm`'s (tests/it/heavy/hud_look.rs).
//! Skips without an install.

use chimera_world::map::hud::{GameHud, font_for_height};
use hl2dm::game::{GAME, INFO, hud, mount};
use source_engine::content::material::MaterialLoader;

use chimera_core::mount::{Mount, served};

fn install_mount() -> Option<Mount> {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no HL2:DM install (Steam, [games.hl2dm] path, or $CHIMERA_HL2DM_ROOT)");
        return None;
    };
    if !served::usable(GAME, &path, "hl2mp") {
        eprintln!("skipping: no HL2:DM install at {}", path.display());
        return None;
    }
    Some(mount::open(&path).expect("the install mounts"))
}

fn look() -> Option<GameHud> {
    let mount = install_mount()?;
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    Some(hud::load(&mut materials).expect("the look loads"))
}

/// ammo panel to r222; back to one without hides it.
#[test]
fn the_install_shows_the_secondary_panel() {
    use hl2dm_ui::{frame::{Draw, Screen}, hud::{Hl2Hud, Inputs, WeaponNow}, look::Look};
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

/// is the cell's, centred on the crosshair when drawn at cy - h/2 (6.2).
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

