//! What the HUD reads from the install's look (`map::hud::GameHud`, read
//! by `game::hud`): panel boxes in window pixels, scheme colours, fonts
//! by the rule of specs/hl2dm/hud.md 2 (the entry whose `yres` holds the
//! window's height, else the first without one scaled from 480 lines;
//! GorDIN's weight picks its file), glyph and sprite icons.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_client::client::hud_text::{GlyphFont, width};
use chimera_world::map::hud::{GameHud, HudCoord, HudPanel, UiFontSize, font_for_height};

use super::frame::{Rgba, Screen};

/// The look at a window size.
#[derive(Clone, Debug)]
pub struct Look {
    pub hud: Arc<GameHud>,
    pub screen: Screen,
}

/// A scheme font as drawn: its file, blend and cell height in pixels.
#[derive(Clone, Debug)]
pub struct LookFont {
    pub font: GlyphFont,
    pub tall: f32,
}

/// The suffix of GorDIN's file for a weight (300 Light, 400 Regular, 600
/// Semibold, 700 Bold, 900 Black; 0 is Regular, 1000 Black).
fn weight_suffix(weight: u32) -> &'static str {
    match weight {
        0..=350 if weight > 0 => "light",
        0..=499 => "regular",
        500..=649 => "semibold",
        650..=799 => "bold",
        _ => "black",
    }
}

impl Look {
    pub fn new(hud: Arc<GameHud>, screen: Screen) -> Look {
        Look { hud, screen }
    }

    /// A scheme colour (the first of a name listed twice), white when the
    /// scheme lacks it.
    pub fn colour(&self, name: &str) -> Rgba {
        self.hud.colors.get(name).copied().unwrap_or([255, 255, 255, 255])
    }

    pub fn panel(&self, name: &str) -> Option<&HudPanel> {
        self.hud.panels.get(name)
    }

    /// A panel's numeric key in units (`0` when missing).
    pub fn key(&self, panel: &str, key: &str) -> f32 {
        self.panel(panel).and_then(|p| p.num(key)).unwrap_or(0.0)
    }

    /// A panel's layout box.
    pub fn rect(&self, name: &str) -> Option<Rect> {
        let p = self.panel(name)?;
        Some(self.screen.rect(p.x, p.y, p.wide, p.tall))
    }

    /// The scheme font entry that applies at this window's height.
    pub fn entry(&self, name: &str) -> Option<&UiFontSize> {
        font_for_height(self.hud.text_fonts.get(name)?, self.screen.h)
    }

    /// A scheme font's cell height in pixels: an entry for this height is
    /// in pixels, one without `yres` scaled from 480 lines (truncated).
    pub fn tall(&self, name: &str) -> f32 {
        let Some(e) = self.entry(name) else { return 0.0 };
        let h = self.screen.h.round() as u32;
        if e.yres.is_some_and(|(lo, hi)| (lo..=hi).contains(&h)) { e.tall } else { (e.tall * self.screen.h / 480.0).trunc() }
    }

    /// A scheme font with its file, when the install had it.
    pub fn font(&self, name: &str) -> Option<LookFont> {
        let e = self.entry(name)?;
        let family = e.family.to_lowercase();
        let suffix = weight_suffix(e.weight);
        let file = self
            .hud
            .fonts
            .get(&format!("file:{family}-{suffix}"))
            .or_else(|| self.hud.fonts.get(&format!("file:{family}")))
            .or_else(|| self.hud.fonts.get(&format!("family:{family}")))?;
        Some(LookFont { font: GlyphFont { data: file.data.clone(), additive: e.additive }, tall: self.tall(name) })
    }

    /// The width of `text` in a scheme font (0 without its file).
    pub fn width(&self, name: &str, text: &str) -> f32 {
        self.font(name).map_or(0.0, |f| width(&f.font.data, f.tall, text))
    }

    /// A glyph icon (a font name and its character) by key.
    pub fn glyph(&self, key: &str) -> Option<(String, char)> {
        self.hud.icons.get(key).cloned()
    }

    /// A string by `#token` (`fallback` when the files lack it).
    pub fn string(&self, token: &str, fallback: &str) -> String {
        // Text that isn't a `#token` is its own.
        if !token.starts_with('#') {
            return token.to_string();
        }
        self.hud.menus.as_ref().map_or(fallback, |m| m.string(token, fallback)).to_string()
    }

    /// A panel's position and size with the animations' overrides applied:
    /// `position` and `size` (units, x from the left) when set.
    pub fn animated_rect(&self, name: &str, position: Option<[f32; 2]>, size: Option<[f32; 2]>) -> Option<Rect> {
        let p = self.panel(name)?;
        let (x, y) = match position {
            Some([x, y]) => (HudCoord::Start(x), HudCoord::Start(y)),
            None => (p.x, p.y),
        };
        let [w, h] = size.unwrap_or([p.wide, p.tall]);
        Some(self.screen.rect(x, y, w, h))
    }
}

/// The stand-in look the tests use: the layout, colours and fonts the spec
/// states (specs/hl2dm/hud.md 2, 4.2, 5.2, 7, 8, 9), without the install.
/// The font names carry the spec's sizes; no font files.
pub fn spec_look(screen: Screen) -> Look {
    let mut hud = GameHud::default();
    for (name, c) in [
        ("FgColor", [255, 220, 0, 100]),
        ("BgColor", [0, 0, 0, 76]),
        ("BrightFg", [255, 220, 0, 255]),
        ("DamagedBg", [180, 0, 0, 200]),
        ("DamagedFg", [180, 0, 0, 230]),
        ("BrightDamagedFg", [255, 0, 0, 255]),
        ("SelectionNumberFg", [255, 220, 0, 255]),
        ("SelectionTextFg", [255, 220, 0, 255]),
        ("SelectionEmptyBoxBg", [0, 0, 0, 80]),
        ("SelectionBoxBg", [0, 0, 0, 80]),
        ("SelectionSelectedBoxBg", [0, 0, 0, 80]),
        ("ZoomReticleColor", [255, 220, 0, 255]),
        ("Normal", [255, 208, 64, 255]),
        ("Caution", [255, 48, 0, 255]),
        ("SectionedListPanel.BgColor", [0, 0, 0, 90]),
    ] {
        hud.colors.insert(name.into(), c);
    }
    let sized = |family: &str, tall: f32, weight: u32, additive: bool| UiFontSize {
        family: family.into(),
        tall,
        weight,
        yres: None,
        antialias: true,
        additive,
    };
    for (name, family, tall, weight) in [
        ("Default", "GorDIN", 10.0, 700),
        ("HudNumbers", "HalfLife2", 32.0, 0),
        ("HudNumbersGlow", "HalfLife2", 32.0, 0),
        ("HudNumbersSmall", "HalfLife2", 16.0, 1000),
        ("HudNumbersTimer", "GorDIN", 15.0, 0),
        ("WeaponIcons", "HalfLife2", 64.0, 0),
        ("WeaponIconsSelected", "HalfLife2", 64.0, 0),
        ("WeaponIconsSmall", "HalfLife2", 32.0, 0),
        ("WeaponIconsSmallSecondary", "HalfLife2", 25.0, 0),
        ("Crosshairs", "HalfLife2", 17.0, 0),
        ("QuickInfo", "HL2cross", 28.0, 0),
        ("HudSelectionNumbers", "GorDIN", 11.0, 700),
        ("HudSelectionText", "GorDIN", 7.0, 600),
        ("HL2MPTypeDeath", "HL2MP", 32.0, 0),
        ("TargetID", "GorDIN", 24.0, 0),
    ] {
        hud.text_fonts.insert(name.into(), vec![sized(family, tall, weight, true)]);
    }
    // `Default`'s entries by height (the spec's table).
    hud.text_fonts.insert(
        "Default".into(),
        vec![
            UiFontSize { yres: Some((1, 599)), ..sized("GorDIN", 12.0, 600, false) },
            UiFontSize { yres: Some((600, 767)), ..sized("GorDIN", 14.0, 600, false) },
            UiFontSize { yres: Some((768, 1023)), ..sized("GorDIN", 19.0, 700, false) },
            UiFontSize { yres: Some((1024, 1199)), ..sized("GorDIN", 21.0, 700, false) },
            sized("GorDIN", 10.0, 700, true),
        ],
    );
    hud.text_fonts.insert(
        "Crosshairs".into(),
        vec![
            UiFontSize { yres: Some((1, 1599)), ..sized("HalfLife2", 40.0, 0, true) },
            UiFontSize { yres: Some((1600, 3199)), ..sized("HalfLife2", 80.0, 0, true) },
            UiFontSize { yres: Some((3200, 4799)), ..sized("HalfLife2", 120.0, 0, true) },
            sized("HalfLife2", 17.0, 0, true),
        ],
    );
    let mut panel = |name: &str, x: HudCoord, y: HudCoord, wide: f32, tall: f32, keys: &[(&str, &str)]| {
        hud.panels.insert(
            name.into(),
            HudPanel {
                x,
                y,
                wide,
                tall,
                background: None,
                icon: Vec2::ZERO,
                digit: Vec2::ZERO,
                digit2: Vec2::ZERO,
                keys: keys.iter().map(|(k, v)| (k.to_lowercase(), v.to_string())).collect(),
            },
        );
    };
    use HudCoord::{End, Start};
    let text = |tx: &'static str, ty: &'static str, dx: &'static str, dy: &'static str| [("text_xpos", tx), ("text_ypos", ty), ("digit_xpos", dx), ("digit_ypos", dy)];
    panel("HudHealth", Start(16.0), Start(432.0), 102.0, 36.0, &text("8", "20", "50", "2"));
    panel("HudSuit", Start(140.0), Start(432.0), 108.0, 36.0, &text("8", "20", "50", "2"));
    let mut ammo = text("8", "20", "44", "2").to_vec();
    ammo.extend([("digit2_xpos", "98"), ("digit2_ypos", "16")]);
    panel("HudAmmo", End(150.0), Start(432.0), 136.0, 36.0, &ammo);
    panel("HudAmmoSecondary", End(76.0), Start(432.0), 60.0, 36.0, &text("8", "22", "36", "2"));
    panel(
        "HudSuitPower",
        Start(16.0),
        Start(396.0),
        102.0,
        26.0,
        &[("AuxPowerDisabledAlpha", "70"), ("BarInsetX", "8"), ("BarInsetY", "15"), ("BarWidth", "92"), ("BarHeight", "4"), ("BarChunkWidth", "6"), ("BarChunkGap", "3"), ("text_xpos", "8"), ("text_ypos", "4"), ("text2_xpos", "8"), ("text2_ypos", "22"), ("text2_gap", "10")],
    );
    panel(
        "HudWeaponSelection",
        Start(0.0),
        Start(16.0),
        640.0,
        400.0,
        &[("SmallBoxSize", "32"), ("LargeBoxWide", "112"), ("LargeBoxTall", "84"), ("BoxGap", "8"), ("SelectionNumberXPos", "4"), ("SelectionNumberYPos", "4"), ("TextYPos", "64")],
    );
    panel(
        "HudHistoryResource",
        End(252.0),
        Start(40.0),
        248.0,
        320.0,
        &[("history_gap", "56"), ("icon_inset", "38"), ("text_inset", "36")],
    );
    panel("HudDeathNotice", Start(0.0), Start(0.0), 640.0, 480.0, &[]);
    panel("TeamDisplay", Start(16.0), Start(410.0), 0.0, 0.0, &[]);
    panel("TargetID", Start(0.0), Start(0.0), 640.0, 480.0, &[]);
    panel("HudZoom", Start(0.0), Start(0.0), 640.0, 480.0, &[("DashHeight", "4")]);
    let strings = [
        ("valve_hud_health", "HEALTH"),
        ("valve_hud_suit", "SUIT"),
        ("valve_hud_ammo", "AMMO"),
        ("valve_hud_ammo_alt", "ALT"),
        ("valve_hud_aux_power", "AUX POWER"),
        ("valve_hud_oxygen", "OXYGEN"),
        ("valve_hud_flashlight", "FLASHLIGHT"),
        ("valve_hud_sprint", "SPRINT"),
        ("team", "Team %s1"),
        ("playerid_diffteam", "Enemy: %s1"),
        ("playerid_sameteam", "Friend: %s1 Health: %s2"),
        ("scoreboard_player", "%s1    -   %s2 player"),
        ("scoreboard_players", "%s1    -   %s2 players"),
        ("playerscore", "Score"),
        ("playerdeath", "Deaths"),
        ("playerping", "Latency"),
    ];
    hud.menus = Some(chimera_world::map::hud::GameMenus {
        strings: strings.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        ..Default::default()
    });
    Look::new(Arc::new(hud), screen)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec test 1: at 1920 by 1080 a unit is 2.25 pixels; the health box
    /// is at (36, 972), 229 by 81; the ammo box 337 from the right, 306
    /// wide.
    #[test]
    fn boxes_scale_by_height_and_truncate() {
        let l = spec_look(Screen::new(1920.0, 1080.0));
        assert_eq!(l.screen.u(), 2.25);
        assert_eq!(l.rect("HudHealth"), Some(Rect::new(36.0, 972.0, 36.0 + 229.0, 972.0 + 81.0)));
        let a = l.rect("HudAmmo").unwrap();
        assert_eq!((a.min.x, a.width()), (1920.0 - 337.0, 306.0));
    }

    /// Spec tests 3 and 4: font entries by height, with the unscaled
    /// pixels of a `yres` entry.
    #[test]
    fn fonts_follow_the_height() {
        let at = |h: f32| spec_look(Screen::new(h * 16.0 / 9.0, h));
        let l = at(1080.0);
        let e = l.entry("Default").unwrap();
        assert_eq!((e.family.as_str(), e.tall, e.weight, e.additive), ("GorDIN", 21.0, 700, false));
        let e = at(1440.0).entry("Default").unwrap().clone();
        assert!(e.additive);
        assert_eq!(at(1440.0).tall("Default"), 30.0);
        let e = at(600.0).entry("Default").unwrap().clone();
        assert_eq!((e.tall, e.weight), (14.0, 600));
        assert_eq!(l.tall("HudNumbers"), 72.0);
        assert_eq!((l.tall("Crosshairs"), at(2160.0).tall("Crosshairs")), (40.0, 80.0));
    }

    #[test]
    fn weights_pick_gordins_files() {
        assert_eq!(
            [300, 400, 600, 700, 900, 0, 1000].map(weight_suffix),
            ["light", "regular", "semibold", "bold", "black", "regular", "black"]
        );
    }
}
