//! TF2's HUD as a frame of draws (specs/tf2/hud.md): what the local
//! player's state shows each frame, in window pixels, for `systems` to
//! turn into UI nodes and for the tests to read without a window.
//!
//! - **Units** (hud.md "Units and conventions"): one layout unit is
//!   H / 480 pixels on both axes, scaled values truncated; `rN` counts from
//!   the right or bottom edge.
//! - **Health** (3.2, 3.3): the panel at (0, r120), 250 x 120 (its own
//!   file places it, so not `hudlayout.res`'s); the dark cross outline
//!   (`health_bg`, 73, 33, 55 x 55), the off-white cross over it cropped
//!   from the bottom up to the health's share of max (`health_color`, 75,
//!   35, 51 x 51; `health_dead` when dead), the number centred in its box
//!   (76, 52, 50 x 18) in `HudClassHealth`, `TanDark`; behind them the
//!   bonus cross (`health_over_bg`): white and grown toward 55 + 2 x 35
//!   units at the overheal cap while overhealed, pulsing every 0.4 s; red
//!   (`HUDDeathWarning`) at its base size below 49 % of max, pulsing every
//!   0.25 s.
//! - **Ammo** (3.5): the panel at `HudWeaponAmmo` (r95, r55, 94 x 45) in
//!   the team's colour (a rounded box: the material's distance-field panel
//!   baked to a box), the clip big (`HudFontGiantBold`, its box's bottom
//!   right) and the reserve small (`HudFontMediumSmall`, its box's bottom
//!   left), each over its shadow one unit right and down; a gun without a
//!   reserve (the Minigun) shows its one number in the wider box.
//! - **Crosshair** (3.4): the sprite sheet's "( · )" cell (the public
//!   `default`), centred, 32 x 32 screen pixels at any resolution (before:
//!   scaled by the height, 21 pixels at 720 lines, too faint to see on
//!   de_dust2's sand), in 200 200 200 (the recalled `cl_crosshair_*` defaults);
//!   behind the Sniper's scope too, as real TF2 draws it (hud.md
//!   "Measured: the Sniper's scope", against 3.4's rule).
//! - **Class portrait** (3.10): the 2D image the spec allows (not the 3D
//!   model panel): the team's background (`character_red_bg` /
//!   `character_blue_bg`, 9, r60, 100 x 50) under the class's picture
//!   (`class_<class><red|blue>`, 25, r88, 75 x 75), after the health
//!   panel.
//!
//! - **Item meter** (batch2b_mechanics.md 14): an item's meter (hype,
//!   boost, crikey, heads, a consumable's recharge) in
//!   `HudItemEffectMeter`'s box: its label over a bar (`item_meter`).
//!
//! Known shortcuts: the item meter's inner layout is ours; the max-health label, the low-ammo pulse (no
//! threshold in the data), damage numbers and the health-gain number
//! and the hit sound aren't drawn (open questions 6, 9, 3, 12); the
//! portrait is the 2D picture, not the 3D model; each gun's own crosshair is unknown (open question 2: the
//! scripts name it) so every gun shows the default; the ammo panel's
//! gradient, soft edge and outline are a flat rounded box.

use bevy::prelude::*;
use chimera_world::map::hud::HudCoord;

/// RGBA 0..255.
pub type Rgba = [u8; 4];

/// The window and its unit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub w: f32,
    pub h: f32,
}

impl Screen {
    pub fn new(w: f32, h: f32) -> Screen {
        Screen { w, h }
    }

    /// Pixels a unit takes.
    pub fn u(&self) -> f32 {
        self.h / 480.0
    }

    /// A length in units as whole pixels (truncated).
    pub fn scaled(&self, v: f32) -> f32 {
        (v * self.u()).trunc()
    }

    pub fn x(&self, c: HudCoord) -> f32 {
        match c {
            HudCoord::Start(v) => self.scaled(v),
            HudCoord::Centre(v) => self.w / 2.0 + self.scaled(v),
            HudCoord::End(v) => self.w - self.scaled(v),
        }
    }

    pub fn y(&self, c: HudCoord) -> f32 {
        match c {
            HudCoord::Start(v) => self.scaled(v),
            HudCoord::Centre(v) => self.h / 2.0 + self.scaled(v),
            HudCoord::End(v) => self.h - self.scaled(v),
        }
    }

    /// A box: left, top, wide, tall in units.
    pub fn rect(&self, x: HudCoord, y: HudCoord, wide: f32, tall: f32) -> Rect {
        let min = Vec2::new(self.x(x), self.y(y));
        Rect::from_corners(min, min + Vec2::new(self.scaled(wide), self.scaled(tall)))
    }

    /// A child's box at (x, y) units inside `parent`.
    pub fn child(&self, parent: Rect, x: f32, y: f32, wide: f32, tall: f32) -> Rect {
        let min = parent.min + Vec2::new(self.scaled(x), self.scaled(y));
        Rect::from_corners(min, min + Vec2::new(self.scaled(wide), self.scaled(tall)))
    }
}

/// Where a text's anchor sits (`hud_text::Align`).
pub use chimera_client::client::hud_text::Align;

/// One thing to draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// A filled box with rounded corners of `radius` pixels.
    Box { rect: Rect, color: Rgba, radius: f32 },
    /// Text in a scheme font (`font`, `tall` pixels), `at` the anchor.
    Text { font: &'static str, tall: f32, text: String, color: Rgba, at: Vec2, align: Align },
    /// The part `uv` (0..1 of the picture's rectangle) of a picture of the
    /// look (`GameHud::sprites` key) drawn into `rect`.
    Sprite { sprite: &'static str, rect: Rect, uv: Rect, color: Rgba },
}

impl Draw {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Draw::Text { text, .. } => Some(text),
            _ => None,
        }
    }
}

/// The opacity left of a crosshair that is `dim` dimmed.
fn dim_alpha(dim: f32) -> f32 {
    1.0 - chimera_client::client::third_person::DIM_AMOUNT * dim.clamp(0.0, 1.0)
}

/// The weapon in hand as the HUD reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ammo {
    /// Rounds loaded; with no reserve (the Minigun) the one number.
    pub clip: i32,
    /// The reserve; None: a gun without one (one number).
    pub reserve: Option<i32>,
}

/// What the HUD reads from the game in one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inputs {
    pub alive: bool,
    /// Health and max health, points.
    pub health: f32,
    pub max: f32,
    /// RED (false) or BLU (true).
    pub blu: bool,
    /// The gun's ammo (None: no gun or one that uses none: no panel).
    pub ammo: Option<Ammo>,
    /// The class portrait's picture (`game::hud::portrait`; None: none).
    pub portrait: Option<&'static str>,
    /// Seconds, for the pulses.
    pub time: f64,
    /// Behind a scope (the shell's overlay up, `hud::ScopeUp`): the
    /// held gun's charge as a fraction 0-1 (`HeldGauge`); the charge
    /// meter then, the crosshair still drawn (measured).
    pub scope: Option<f32>,
    /// How much the crosshair dims (0 .. 1): the over-the-shoulder camera's
    /// blocked-shot marker shows (`client::third_person::Shoulder::dim`).
    pub crosshair_dim: f32,
    /// An item's meter (specs/tf2/batch2b_mechanics.md 14, 11-13: hype,
    /// boost, crikey, heads, a consumable's recharge): its label and fill
    /// 0-1 (`tf2::game::items::meters`, `consumables`).
    pub meter: Option<(String, f32)>,
}

/// The pictures of this HUD that are the crosshair: they dim with the
/// blocked-shot marker (data per HUD; `Inputs::crosshair_dim`).
pub const CROSSHAIR_SPRITES: &[&str] = &["crosshairs"];

/// The scheme's colours and fonts this HUD uses, by name.
pub trait Scheme {
    fn colour(&self, name: &str) -> Option<Rgba>;
    /// A font's cell height in pixels at this window's height (None: the
    /// install lacks it).
    fn tall(&self, font: &str) -> Option<f32>;
    /// Whether the look holds the picture.
    fn has(&self, sprite: &str) -> bool;
}

/// Low health below this share of max (hud.md `HealthDeathWarning`).
pub const DEATH_WARNING: f32 = 0.49;
/// The bonus cross's growth at the overheal cap, units each side.
pub const BONUS_GROWTH: f32 = 35.0;

/// The overheal pulse's alpha at `t` seconds into it (3.1: up to 255 over
/// 0.2 s, down to 127 by 0.4 s, then again from 127).
pub fn bonus_alpha(t: f64) -> f32 {
    if t < 0.2 {
        return (255.0 * t / 0.2) as f32;
    }
    let p = ((t - 0.2) % 0.4) as f32;
    if p < 0.2 { 255.0 - 128.0 * p / 0.2 } else { 127.0 + 128.0 * (p - 0.2) / 0.2 }
}

/// The low-health pulse's alpha at `t` (3.1: up 0.075 s, held to 0.125,
/// down by 0.2, off to 0.25, again).
pub fn dying_alpha(t: f64) -> f32 {
    let p = (t % 0.25) as f32;
    if p < 0.075 {
        255.0 * p / 0.075
    } else if p < 0.125 {
        255.0
    } else if p < 0.2 {
        255.0 * (1.0 - (p - 0.125) / 0.075)
    } else {
        0.0
    }
}

/// The health panel's state (3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthState {
    Normal,
    Overhealed,
    Low,
}

pub fn health_state(health: f32, max: f32) -> HealthState {
    if health > max {
        HealthState::Overhealed
    } else if health < DEATH_WARNING * max {
        HealthState::Low
    } else {
        HealthState::Normal
    }
}

/// The HUD's state between frames: when each pulse started.
#[derive(Clone, Debug, Default)]
pub struct Tf2Hud {
    pulse: Option<(HealthState, f64)>,
}

impl Tf2Hud {
    /// This frame's draws for `i` on `screen` in `scheme`.
    pub fn frame(&mut self, i: &Inputs, screen: Screen, scheme: &dyn Scheme) -> Vec<Draw> {
        let mut out = Vec::new();
        let white = [255, 255, 255, 255];
        let tan_dark = scheme.colour("TanDark").unwrap_or([117, 107, 94, 255]);
        let tan_light = scheme.colour("TanLight").unwrap_or([235, 226, 202, 255]);
        let shadow = scheme.colour("TanDarker").unwrap_or([46, 43, 42, 255]);
        let faint = scheme.colour("TransparentBlack").unwrap_or([0, 0, 0, 196]);
        let full = Rect::new(0.0, 0.0, 1.0, 1.0);
        // The class portrait (3.10, the 2D image): the team's background
        // (9, r60, 100 x 50) under the class's picture (25, r88, 75 x 75),
        // drawn first: the health panel (zpos 2) overlaps its right edge
        // and its number is above it.
        if let (true, Some(p)) = (i.alive, i.portrait) {
            let bg = tf2::game::hud::portrait_bg(i.blu);
            if scheme.has(bg) {
                let rect = screen.rect(HudCoord::Start(9.0), HudCoord::End(60.0), 100.0, 50.0);
                out.push(Draw::Sprite { sprite: bg, rect, uv: full, color: white });
            }
            if scheme.has(p) {
                let rect = screen.rect(HudCoord::Start(25.0), HudCoord::End(88.0), 75.0, 75.0);
                out.push(Draw::Sprite { sprite: p, rect, uv: full, color: white });
            }
        }
        // Health.
        let panel = screen.rect(HudCoord::Start(0.0), HudCoord::End(120.0), 250.0, 120.0);
        let state = if i.alive { health_state(i.health, i.max) } else { HealthState::Normal };
        let since = match self.pulse {
            Some((s, t)) if s == state => t,
            _ => {
                self.pulse = Some((state, i.time));
                i.time
            }
        };
        let t = i.time - since;
        match state {
            HealthState::Overhealed => {
                let cap = (1.5 * i.max / 5.0).floor() * 5.0;
                let k = ((i.health - i.max) / (cap - i.max).max(1.0)).clamp(0.0, 1.0);
                let grow = BONUS_GROWTH * k;
                let r = screen.child(panel, 73.0 - grow, 33.0 - grow, 55.0 + 2.0 * grow, 55.0 + 2.0 * grow);
                out.push(Draw::Sprite { sprite: "health_over_bg", rect: r, uv: full, color: [255, 255, 255, bonus_alpha(t) as u8] });
            }
            HealthState::Low => {
                let red = scheme.colour("HUDDeathWarning").unwrap_or([255, 0, 0, 255]);
                let r = screen.child(panel, 73.0, 33.0, 55.0, 55.0);
                out.push(Draw::Sprite { sprite: "health_over_bg", rect: r, uv: full, color: [red[0], red[1], red[2], dying_alpha(t) as u8] });
            }
            HealthState::Normal => {}
        }
        out.push(Draw::Sprite { sprite: "health_bg", rect: screen.child(panel, 73.0, 33.0, 55.0, 55.0), uv: full, color: white });
        let fill = screen.child(panel, 75.0, 35.0, 51.0, 51.0);
        if !i.alive {
            if scheme.has("health_dead") {
                out.push(Draw::Sprite { sprite: "health_dead", rect: fill, uv: full, color: white });
            }
        } else {
            let f = (i.health / i.max.max(1.0)).clamp(0.0, 1.0);
            let top = fill.max.y - fill.height() * f;
            let rect = Rect::new(fill.min.x, top, fill.max.x, fill.max.y);
            out.push(Draw::Sprite { sprite: "health_color", rect, uv: Rect::new(0.0, 1.0 - f, 1.0, 1.0), color: white });
            if let Some(tall) = scheme.tall("HudClassHealth") {
                let b = screen.child(panel, 76.0, 52.0, 50.0, 18.0);
                let text = format!("{}", i.health.max(0.0).round() as i32);
                let at = Vec2::new(b.center().x, b.center().y - tall / 2.0);
                out.push(Draw::Text { font: "HudClassHealth", tall, text, color: tan_dark, at, align: Align::Centre });
            }
        }
        // Ammo.
        if let (true, Some(ammo)) = (i.alive, &i.ammo) {
            let p = screen.rect(HudCoord::End(95.0), HudCoord::End(55.0), 94.0, 45.0);
            let team = if i.blu { scheme.colour("HUDBlueTeamSolid").unwrap_or([104, 124, 155, 255]) } else { scheme.colour("HUDRedTeamSolid").unwrap_or([180, 92, 77, 255]) };
            let bg = screen.child(p, 4.0, 0.0, 90.0, 45.0);
            let radius = screen.scaled(6.0);
            out.push(Draw::Box { rect: bg.inflate(screen.scaled(1.0)), color: [0, 0, 0, 115], radius });
            out.push(Draw::Box { rect: bg, color: team, radius });
            let big = |out: &mut Vec<Draw>, b: Rect, text: String| {
                if let Some(tall) = scheme.tall("HudFontGiantBold") {
                    let one = screen.scaled(1.0);
                    let at = Vec2::new(b.max.x, b.max.y - tall);
                    out.push(Draw::Text { font: "HudFontGiantBold", tall, text: text.clone(), color: shadow, at: at + Vec2::splat(one), align: Align::Right });
                    out.push(Draw::Text { font: "HudFontGiantBold", tall, text, color: tan_light, at, align: Align::Right });
                }
            };
            match ammo.reserve {
                Some(reserve) => {
                    big(&mut out, screen.child(p, 4.0, 0.0, 55.0, 40.0), ammo.clip.to_string());
                    if let Some(tall) = scheme.tall("HudFontMediumSmall") {
                        let b = screen.child(p, 59.0, 8.0, 40.0, 27.0);
                        let one = screen.scaled(1.0);
                        let at = Vec2::new(b.min.x, b.max.y - tall);
                        out.push(Draw::Text { font: "HudFontMediumSmall", tall, text: reserve.to_string(), color: faint, at: at + Vec2::splat(one), align: Align::Left });
                        out.push(Draw::Text { font: "HudFontMediumSmall", tall, text: reserve.to_string(), color: tan_light, at, align: Align::Left });
                    }
                }
                None => big(&mut out, screen.child(p, 0.0, 2.0, 84.0, 40.0), ammo.clip.to_string()),
            }
        }
        // An item's meter.
        if let (true, Some((label, fill))) = (i.alive, &i.meter) {
            out.extend(item_meter(label, *fill, screen, scheme));
        }
        // The charge meter beside the scope's middle.
        if let (true, Some(charge)) = (i.alive, i.scope) {
            out.extend(charge_meter(charge, screen, scheme));
        }
        // Crosshair: the sheet's (0, 0, 32, 32) of 128 x 128; behind a
        // scope too (measured on real TF2, hud.md "Measured: the Sniper's
        // scope" and Open questions 13: 3.4 says a scope hides it, the
        // game draws it).
        if i.alive && scheme.has("crosshairs") {
            // Screen pixels, not layout units (3.4): 32 at any height.
            let size = 32.0;
            let c = Vec2::new(screen.w / 2.0, screen.h / 2.0);
            let rect = Rect::from_center_size(c, Vec2::splat(size));
            let alpha = (255.0 * dim_alpha(i.crosshair_dim)).round() as u8;
            out.push(Draw::Sprite { sprite: CROSSHAIR_SPRITES[0], rect, uv: Rect::new(0.0, 0.0, 0.25, 0.25), color: [200, 200, 200, alpha] });
        }
        out
    }
}

/// An item's meter (batch2b_mechanics.md 14) in `HudItemEffectMeter`'s
/// box (hud.md 1: r162, r52, 100 x 50): a faint backing, its label
/// (`HudFontSmall`, `TanLight`) and a bar (10, 30, 80 x 8) filled by
/// `fill` in `HudProgressBarActive` over `HudProgressBarInActive`, the
/// fill `HudProgressBarActiveLow` below a quarter. Its inner layout is
/// ours (the meter's own `.res` isn't specified).
pub fn item_meter(label: &str, fill: f32, screen: Screen, scheme: &dyn Scheme) -> Vec<Draw> {
    let mut out = Vec::new();
    let p = screen.rect(HudCoord::End(162.0), HudCoord::End(52.0), 100.0, 50.0);
    let radius = screen.scaled(4.0);
    out.push(Draw::Box { rect: screen.child(p, 4.0, 4.0, 92.0, 42.0), color: [0, 0, 0, 115], radius });
    if let Some(tall) = scheme.tall("HudFontSmall") {
        let tan = scheme.colour("TanLight").unwrap_or([235, 226, 202, 255]);
        let b = screen.child(p, 10.0, 10.0, 80.0, 16.0);
        out.push(Draw::Text { font: "HudFontSmall", tall, text: label.to_string(), color: tan, at: Vec2::new(b.min.x, b.min.y), align: Align::Left });
    }
    let bar = screen.child(p, 10.0, 30.0, 80.0, 8.0);
    let back = scheme.colour("HudProgressBarInActive").unwrap_or([140, 120, 73, 255]);
    let f = fill.clamp(0.0, 1.0);
    let front = if f < 0.25 {
        scheme.colour("HudProgressBarActiveLow").unwrap_or([240, 30, 30, 255])
    } else {
        scheme.colour("HudProgressBarActive").unwrap_or([240, 207, 78, 255])
    };
    out.push(Draw::Box { rect: bar, color: back, radius: 0.0 });
    if f > 0.0 {
        out.push(Draw::Box { rect: Rect::new(bar.min.x, bar.min.y, bar.min.x + bar.width() * f, bar.max.y), color: front, radius: 0.0 });
    }
    out
}

/// The Sniper's charge meter (measured on real TF2, hud.md "Measured":
/// a 1024x768 frame scoped at 0.5, 1.8 and 4 s): `sniperscope_numbers2`
/// (128 x 256 texels) drawn additively at half a unit a texel, its top left
/// 64 units right of the screen's middle and 64 up, so its wedge of 13
/// stripes (texels 6-68 across, from row 128 down) starts at the middle's
/// height to its right. The wedge lights from its left by the charge
/// (`charge` 0-1; the rest of it faint, a fifth as bright); the bolt and
/// "100%" (texels 70-128) only at a full charge.
pub fn charge_meter(charge: f32, screen: Screen, scheme: &dyn Scheme) -> Vec<Draw> {
    const SPRITE: &str = "sniperscope_numbers2";
    const TEXELS: Vec2 = Vec2::new(128.0, 256.0);
    const WEDGE: (f32, f32) = (6.0, 69.0);
    const BOLT: f32 = 70.0;
    if !scheme.has(SPRITE) {
        return Vec::new();
    }
    let u = screen.u();
    let texel = 0.5 * u;
    let origin = Vec2::new(screen.w / 2.0 + 64.0 * u, screen.h / 2.0 - 64.0 * u);
    // Texel columns a..b, the full height, in `alpha` of the picture's
    // own colours.
    let part = |a: f32, b: f32, alpha: u8| Draw::Sprite {
        sprite: SPRITE,
        rect: Rect::new(origin.x + a * texel, origin.y, origin.x + b * texel, origin.y + TEXELS.y * texel),
        uv: Rect::new(a / TEXELS.x, 0.0, b / TEXELS.x, 1.0),
        color: [255, 255, 255, alpha],
    };
    let c = charge.clamp(0.0, 1.0);
    let lit = WEDGE.0 + (WEDGE.1 - WEDGE.0) * c;
    let mut out = Vec::new();
    if lit > WEDGE.0 {
        out.push(part(WEDGE.0, lit, 255));
    }
    if lit < WEDGE.1 {
        out.push(part(lit, WEDGE.1, 51));
    }
    if c >= 1.0 {
        out.push(part(BOLT, TEXELS.x, 255));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Plain;
    impl Scheme for Plain {
        fn colour(&self, _: &str) -> Option<Rgba> {
            None
        }
        fn tall(&self, font: &str) -> Option<f32> {
            Some(match font {
                "HudClassHealth" => 36.0,
                "HudFontGiantBold" => 99.0,
                _ => 40.0,
            })
        }
        fn has(&self, _: &str) -> bool {
            true
        }
    }

    fn sprite<'a>(d: &'a [Draw], name: &str) -> Option<(&'a Rect, &'a Rect, &'a Rgba)> {
        d.iter().find_map(|x| match x {
            Draw::Sprite { sprite, rect, uv, color } if *sprite == name => Some((rect, uv, color)),
            _ => None,
        })
    }

    /// Tests 1, 2: the panels' places at 1080 and 720 lines.
    #[test]
    fn panels_sit_where_the_layout_puts_them_1_2() {
        let s = Screen::new(1920.0, 1080.0);
        assert_eq!(s.u(), 2.25);
        assert_eq!(s.rect(HudCoord::End(95.0), HudCoord::End(55.0), 94.0, 45.0), Rect::new(1707.0, 957.0, 1918.0, 1058.0));
        assert_eq!(s.rect(HudCoord::Start(0.0), HudCoord::End(120.0), 250.0, 120.0), Rect::new(0.0, 810.0, 562.0, 1080.0));
        let p = s.rect(HudCoord::Start(0.0), HudCoord::End(120.0), 250.0, 120.0);
        assert_eq!(s.child(p, 73.0, 33.0, 55.0, 55.0), Rect::new(164.0, 884.0, 287.0, 1007.0));
        let s = Screen::new(1280.0, 720.0);
        let r = s.rect(HudCoord::End(95.0), HudCoord::End(55.0), 94.0, 45.0);
        assert_eq!((r.min, r.size()), (Vec2::new(1138.0, 638.0), Vec2::new(141.0, 67.0)));
    }

    /// Tests 3-5: a Heavy at 150 fills half, at 146 is low, at 147 not.
    #[test]
    fn health_fill_and_the_low_state_3_4_5() {
        let s = Screen::new(1920.0, 1080.0);
        let mut hud = Tf2Hud::default();
        let i = Inputs { alive: true, health: 150.0, max: 300.0, ..default() };
        let d = hud.frame(&i, s, &Plain);
        assert_eq!(sprite(&d, "health_color").unwrap().1.min.y, 0.5);
        assert!(d.iter().any(|x| x.as_text() == Some("150")));
        assert!(sprite(&d, "health_over_bg").is_none(), "no pulse");
        assert_eq!(health_state(146.0, 300.0), HealthState::Low);
        assert_eq!(health_state(147.0, 300.0), HealthState::Normal);
        let d = hud.frame(&Inputs { health: 146.0, ..i.clone() }, s, &Plain);
        let (_, _, c) = sprite(&d, "health_over_bg").unwrap();
        assert_eq!(&c[..3], &[255, 0, 0]);
    }

    /// Test 6: a Scout at 185 / 125: overhealed, the bonus cross at its
    /// largest (55 + 70 units), white.
    #[test]
    fn overheal_grows_the_white_cross_6() {
        let s = Screen::new(1920.0, 1080.0);
        let d = Tf2Hud::default().frame(&Inputs { alive: true, health: 185.0, max: 125.0, ..default() }, s, &Plain);
        let (r, _, c) = sprite(&d, "health_over_bg").unwrap();
        assert_eq!(r.width(), s.scaled(125.0));
        assert_eq!(&c[..3], &[255, 255, 255]);
    }

    /// Tests 8, 9: the pulses' alphas.
    #[test]
    fn pulses_8_9() {
        assert!((bonus_alpha(0.2) - 255.0).abs() < 1.0);
        assert!((bonus_alpha(0.4) - 127.0).abs() < 1.0);
        assert!((bonus_alpha(0.6) - 255.0).abs() < 1.0);
        assert!((dying_alpha(0.075) - 255.0).abs() < 1.0);
        assert!((dying_alpha(0.1) - 255.0).abs() < 1.0);
        assert!(dying_alpha(0.2) < 1.0);
        assert!(dying_alpha(0.25) < 1.0);
    }

    /// Tests 10, 11: the Scattergun's clip and reserve, the Minigun's one
    /// number.
    #[test]
    fn ammo_clip_and_reserve_or_one_number_10_11() {
        let s = Screen::new(1920.0, 1080.0);
        let d = Tf2Hud::default().frame(
            &Inputs { alive: true, health: 125.0, max: 125.0, ammo: Some(Ammo { clip: 6, reserve: Some(32) }), ..default() },
            s,
            &Plain,
        );
        assert!(d.iter().filter(|x| x.as_text() == Some("6")).count() == 2, "the clip and its shadow");
        assert!(d.iter().any(|x| x.as_text() == Some("32")));
        let d = Tf2Hud::default().frame(
            &Inputs { alive: true, health: 300.0, max: 300.0, blu: true, ammo: Some(Ammo { clip: 200, reserve: None }), ..default() },
            s,
            &Plain,
        );
        let texts: Vec<&str> = d.iter().filter_map(Draw::as_text).collect();
        assert_eq!(texts, ["300", "200", "200"]);
        let blue = d.iter().any(|x| matches!(x, Draw::Box { color, .. } if *color == [104, 124, 155, 255]));
        assert!(blue, "the blue panel");
    }

    /// 3.10: the class portrait, the team's background under the class's
    /// picture at (25, r88) 75 x 75; the Demoman's and Engineer's short
    /// names; none when dead.
    #[test]
    fn class_portrait_3_10() {
        use tf2::game::hud::portrait;
        assert_eq!(portrait("demoman", false), Some("class_demored"));
        assert_eq!(portrait("engineer", true), Some("class_engiblue"));
        assert_eq!(portrait("scout", true), Some("class_scoutblue"));
        assert_eq!(portrait("nobody", true), None);
        let s = Screen::new(1920.0, 1080.0);
        let i = Inputs { alive: true, health: 200.0, max: 200.0, portrait: portrait("soldier", false), ..default() };
        let d = Tf2Hud::default().frame(&i, s, &Plain);
        let (r, ..) = sprite(&d, "class_soldierred").expect("the portrait");
        assert_eq!(*r, Rect::new(56.0, 882.0, 224.0, 1050.0));
        assert!(sprite(&d, "character_red_bg").is_some());
        let order: Vec<usize> = ["character_red_bg", "class_soldierred", "health_bg"]
            .iter()
            .map(|n| d.iter().position(|x| matches!(x, Draw::Sprite { sprite, .. } if sprite == n)).unwrap())
            .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "the portrait over its background, the health panel over both");
        let d = Tf2Hud::default().frame(&Inputs { alive: false, ..i }, s, &Plain);
        assert!(sprite(&d, "class_soldierred").is_none());
    }

    /// The over-the-shoulder camera's blocked-shot marker dims the
    /// crosshair (`Inputs::crosshair_dim`), and nothing else.
    #[test]
    fn the_blocked_shot_marker_dims_the_crosshair() {
        struct Sheets;
        impl Scheme for Sheets {
            fn colour(&self, _: &str) -> Option<Rgba> {
                None
            }
            fn tall(&self, _: &str) -> Option<f32> {
                None
            }
            fn has(&self, _: &str) -> bool {
                true
            }
        }
        let s = Screen::new(1024.0, 768.0);
        let open = Inputs { alive: true, health: 125.0, max: 125.0, ..default() };
        let alpha = |dim: f32| {
            let d = Tf2Hud::default().frame(&Inputs { crosshair_dim: dim, ..open.clone() }, s, &Sheets);
            sprite(&d, CROSSHAIR_SPRITES[0]).map(|(.., c)| c[3]).unwrap()
        };
        assert_eq!(alpha(0.0), 255);
        let full = (255.0 * (1.0 - chimera_client::client::third_person::DIM_AMOUNT)).round() as u8;
        assert_eq!(alpha(1.0), full);
        assert!(alpha(0.5) < 255 && alpha(0.5) > full);
    }

    /// Behind a scope: the crosshair still (measured, hud.md Open
    /// questions 13), the charge meter's
    /// wedge lit by the charge, the bolt and "100%" only when full, 64
    /// units right of the middle and 64 up at half a unit a texel.
    #[test]
    fn a_scope_keeps_the_crosshair_and_shows_the_charge() {
        struct Sheets;
        impl Scheme for Sheets {
            fn colour(&self, _: &str) -> Option<Rgba> {
                None
            }
            fn tall(&self, _: &str) -> Option<f32> {
                None
            }
            fn has(&self, _: &str) -> bool {
                true
            }
        }
        let s = Screen::new(1024.0, 768.0);
        let mut hud = Tf2Hud::default();
        let open = Inputs { alive: true, health: 125.0, max: 125.0, ..default() };
        let d = hud.frame(&open, s, &Sheets);
        assert!(sprite(&d, "crosshairs").is_some());
        assert!(sprite(&d, "sniperscope_numbers2").is_none());
        let half = hud.frame(&Inputs { scope: Some(0.5), ..open.clone() }, s, &Sheets);
        assert!(sprite(&half, "crosshairs").is_some(), "the crosshair behind a scope too");
        let parts: Vec<_> = half.iter().filter_map(|d| match d {
            Draw::Sprite { sprite: "sniperscope_numbers2", rect, uv, color } => Some((*rect, *uv, *color)),
            _ => None,
        }).collect();
        assert_eq!(parts.len(), 2, "lit and faint wedge, no bolt: {parts:?}");
        let u = 768.0 / 480.0;
        assert!((parts[0].0.min.x - (512.0 + 64.0 * u + 6.0 * 0.5 * u)).abs() < 1e-3);
        assert!((parts[0].0.min.y - (384.0 - 64.0 * u)).abs() < 1e-3);
        assert!((parts[0].1.max.x * 128.0 - 37.5).abs() < 1e-3, "lit to half the wedge");
        assert_eq!(parts[1].2[3], 51);
        let full = hud.frame(&Inputs { scope: Some(1.0), ..open }, s, &Sheets);
        let n = full.iter().filter(|d| matches!(d, Draw::Sprite { sprite: "sniperscope_numbers2", .. })).count();
        assert_eq!(n, 2, "the whole wedge and the bolt");
    }

    /// An item meter: in `HudItemEffectMeter`'s box (r162, r52), its label
    /// and a bar filled by its share, red below a quarter.
    #[test]
    fn an_item_meter_shows_its_label_and_fill() {
        let s = Screen::new(1920.0, 1080.0);
        let i = Inputs { alive: true, health: 125.0, max: 125.0, meter: Some(("HYPE".into(), 0.5)), ..default() };
        let d = Tf2Hud::default().frame(&i, s, &Plain);
        assert!(d.iter().any(|x| x.as_text() == Some("HYPE")));
        let boxes: Vec<Rect> = d.iter().filter_map(|x| match x { Draw::Box { rect, color, .. } if *color == [240, 207, 78, 255] => Some(*rect), _ => None }).collect();
        assert_eq!(boxes.len(), 1, "the fill");
        let bar = s.child(s.rect(HudCoord::End(162.0), HudCoord::End(52.0), 100.0, 50.0), 10.0, 30.0, 80.0, 8.0);
        assert!((boxes[0].width() - bar.width() / 2.0).abs() < 0.01);
        let low = item_meter("BOOST", 0.1, s, &Plain);
        assert!(low.iter().any(|x| matches!(x, Draw::Box { color, .. } if *color == [240, 30, 30, 255])));
    }
}
