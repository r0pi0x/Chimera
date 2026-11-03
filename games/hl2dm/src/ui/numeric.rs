//! The four numeric panels (specs/hl2dm/hud.md 4): health, suit, ammo and
//! secondary ammo. Each remembers what it showed, starts the named HUD
//! events when the shown number changes (`scripts/hudanimations.txt` does
//! the flashing and the glow), and is drawn from the variables those
//! events move: the box (`BgColor`), the label and digits (`FgColor`), the
//! digits' glow (`Blur`), `Alpha`, and the ammo panel's `Position` and
//! `Size`.

use bevy::prelude::*;

use crate::game::hud::icon_key;
use super::{
    frame::{Align, Draw, Frame, fade},
    hud::{Hl2Hud, Inputs, WeaponNow},
    look::Look,
};

/// Below this a health or armour change runs the "below 20" events.
pub const LOW: i32 = 20;

/// What the panels remember between frames.
#[derive(Clone, Debug)]
pub struct Numeric {
    /// The health shown last (None after a reset).
    pub health: Option<i32>,
    /// The armour shown last (None until the first value; not reset at
    /// spawn).
    pub suit: Option<i32>,
    /// The ammo panel's weapon (its class) and numbers, remembered
    /// through frames where it is hidden.
    pub ammo_weapon: Option<String>,
    pub ammo_value: i32,
    pub ammo_reserve: i32,
    /// The secondary panel's weapon and the count it last showed.
    pub sec_weapon: Option<String>,
    pub sec_uses: bool,
    pub sec_value: i32,
}

impl Default for Numeric {
    fn default() -> Self {
        Numeric { health: None, suit: None, ammo_weapon: None, ammo_value: -1, ammo_reserve: -1, sec_weapon: None, sec_uses: false, sec_value: -1 }
    }
}

impl Hl2Hud {
    /// Health's, the suit's and the ammo panels' changes this frame.
    pub(super) fn think_numeric(&mut self, inp: &Inputs) {
        // Health: max(health, 0), whole points.
        let shown = inp.health.max(0.0).ceil() as i32;
        if self.numeric.health != Some(shown) {
            self.numeric.health = Some(shown);
            if shown >= LOW {
                self.fire("HealthIncreasedAbove20");
            } else if shown > 0 {
                self.fire("HealthIncreasedBelow20");
                self.fire("HealthLow");
            }
        }
        // The suit: the armour value, a whole number.
        let armour = inp.armour.max(0.0).ceil() as i32;
        let old = self.numeric.suit;
        if old != Some(armour) {
            self.numeric.suit = Some(armour);
            let was = old.unwrap_or(-1);
            if armour == 0 {
                self.fire("SuitPowerZero");
            } else if old.is_some() && armour < was {
                self.fire("SuitDamageTaken");
                if armour < LOW {
                    self.fire("SuitArmorLow");
                }
            } else if armour > was {
                if was <= 0 || armour >= LOW {
                    self.fire("SuitPowerIncreasedAbove20");
                } else {
                    self.fire("SuitPowerIncreasedBelow20");
                }
            }
        }
        self.think_ammo(inp.weapon.as_ref());
        self.think_secondary(inp.weapon.as_ref());
    }

    fn think_ammo(&mut self, weapon: Option<&WeaponNow>) {
        // No weapon, or one without a primary ammo type: hidden, and the
        // remembered weapon stays.
        let Some(w) = weapon.filter(|w| w.uses_primary) else { return };
        let value = w.ammo_value();
        let reserve = w.clip.map_or(-1, |_| w.reserve);
        let n = &self.numeric;
        let (same, old_value, old_reserve) = (n.ammo_weapon.as_deref() == Some(w.class.as_str()), n.ammo_value, n.ammo_reserve);
        // A change of the value runs its event (against the previous
        // weapon's numbers after a switch: a quirk kept).
        if value != old_value {
            if value == 0 {
                self.fire("AmmoEmpty");
            } else if value < old_value {
                self.fire("AmmoDecreased");
            } else {
                self.fire("AmmoIncreased");
            }
        }
        if same && reserve != old_reserve {
            self.fire(if reserve == 0 {
                "Ammo2Empty"
            } else if reserve < old_reserve {
                "Ammo2Decreased"
            } else {
                "Ammo2Increased"
            });
        }
        if !same {
            self.fire(if w.clip.is_some() { "WeaponUsesClips" } else { "WeaponDoesNotUseClips" });
            self.fire("WeaponChanged");
            self.numeric.ammo_weapon = Some(w.class.clone());
        }
        self.numeric.ammo_value = value;
        self.numeric.ammo_reserve = reserve;
    }

    fn think_secondary(&mut self, weapon: Option<&WeaponNow>) {
        let Some(w) = weapon else { return };
        let count = w.secondary.as_ref().map(|(_, n)| *n as i32);
        let same = self.numeric.sec_weapon.as_deref() == Some(w.class.as_str());
        if let Some(c) = count {
            let old = self.numeric.sec_value;
            if same && c != old {
                self.fire(if c == 0 {
                    "AmmoSecondaryEmpty"
                } else if c < old {
                    "AmmoSecondaryDecreased"
                } else {
                    "AmmoSecondaryIncreased"
                });
            }
            self.numeric.sec_value = c;
        }
        if !same {
            self.numeric.sec_weapon = Some(w.class.clone());
            self.numeric.sec_uses = count.is_some();
            self.fire(if count.is_some() { "WeaponUsesSecondaryAmmo" } else { "WeaponDoesNotUseSecondaryAmmo" });
        }
    }

    /// One numeric panel: its box, label, digits with their glow, the
    /// reserve and an icon above the label.
    #[allow(clippy::too_many_arguments)]
    fn numeric_panel(&self, look: &Look, frame: &mut Frame, name: &'static str, token: &str, value: i32, reserve: Option<i32>, icon: Option<(String, char)>) {
        let alpha = self.anim.number(name, "Alpha").unwrap_or(255.0) / 255.0;
        if alpha <= 0.0 {
            return;
        }
        let position = self.anim.get(name, "Position").and_then(|v| v.pair());
        let size = self.anim.get(name, "Size").and_then(|v| v.pair());
        let Some(rect) = look.animated_rect(name, position, size) else { return };
        let s = look.screen;
        let fg = self.anim.rgba(name, "FgColor").unwrap_or_else(|| look.colour("FgColor"));
        let bg = self.anim.rgba(name, "BgColor").unwrap_or_else(|| look.colour("BgColor"));
        frame.push(Draw::Box { rect, color: fade(bg, alpha), round: true });
        let at = |x: &str, y: &str| rect.min + Vec2::new(s.scaled(look.key(name, x)), s.scaled(look.key(name, y)));
        let label = look.string(token, "");
        frame.push(Draw::text("Default", look.tall("Default"), label.clone(), fade(fg, alpha), at("text_xpos", "text_ypos"), Align::Left));
        let digits = at("digit_xpos", "digit_ypos");
        let text = value.to_string();
        frame.push(Draw::text("HudNumbers", look.tall("HudNumbers"), text.clone(), fade(fg, alpha), digits, Align::Left));
        // The glow: the digits again in the glow font, once a whole point
        // of `Blur`, then once more by its fraction.
        let blur = self.anim.number(name, "Blur").unwrap_or(0.0).max(0.0);
        let glow = |k: f32| Draw::text("HudNumbersGlow", look.tall("HudNumbersGlow"), text.clone(), fade(fg, alpha * k), digits, Align::Left);
        for _ in 0..blur.floor() as u32 {
            frame.push(glow(1.0));
        }
        if blur.fract() > 0.0 {
            frame.push(glow(blur.fract()));
        }
        if let Some(r) = reserve {
            frame.push(Draw::text("HudNumbersSmall", look.tall("HudNumbersSmall"), r.to_string(), fade(fg, alpha), at("digit2_xpos", "digit2_ypos"), Align::Left));
        }
        if let Some((font, ch)) = icon {
            // Over the label: centred on its width, the icon's middle
            // half above the label's top.
            let label_w = look.width("Default", &label);
            let icon_font: &'static str = font_name(&font);
            let (tall, w) = (look.tall(icon_font), look.width(icon_font, &ch.to_string()));
            let x = rect.min.x + s.scaled(look.key(name, "text_xpos")) + ((label_w - w) / 2.0).trunc();
            let y = rect.min.y + s.scaled(look.key(name, "text_ypos")) - (look.tall("Default") + (tall / 2.0).trunc());
            frame.push(Draw::text(icon_font, tall, ch.to_string(), fade(fg, alpha), Vec2::new(x, y), Align::Left));
        }
    }

    pub(super) fn draw_numeric(&self, look: &Look, inp: &Inputs, frame: &mut Frame) {
        if inp.hide_health {
            return;
        }
        if inp.alive {
            self.numeric_panel(look, frame, "HudHealth", "#Valve_Hud_HEALTH", self.numeric.health.unwrap_or(0), None, None);
        }
        // The suit's panel shows while the armour does, dead or not.
        if let Some(a) = self.numeric.suit {
            self.numeric_panel(look, frame, "HudSuit", "#Valve_Hud_SUIT", a, None, None);
        }
        if !inp.alive || inp.hide_selection {
            return;
        }
        let Some(w) = &inp.weapon else { return };
        if w.uses_primary {
            let icon = look.glyph(&icon_key(&w.class, "ammo"));
            self.numeric_panel(look, frame, "HudAmmo", "#Valve_Hud_AMMO", w.ammo_value(), w.clip.map(|_| w.reserve), icon);
        }
        // The secondary panel draws what its events left visible.
        let value = w.secondary.as_ref().map_or(self.numeric.sec_value, |(_, n)| *n as i32);
        let icon = look.glyph(&icon_key(&w.class, "ammo2")).filter(|_| w.secondary.is_some());
        self.numeric_panel(look, frame, "HudAmmoSecondary", "#Valve_Hud_AMMO_ALT", value, None, icon);
    }
}

/// The scheme font name of an icon's font as a static string (the few the
/// weapons name).
fn font_name(name: &str) -> &'static str {
    match name {
        "WeaponIconsSmall" => "WeaponIconsSmall",
        "WeaponIconsSmallSecondary" => "WeaponIconsSmallSecondary",
        "WeaponIconsSelected" => "WeaponIconsSelected",
        "Crosshairs" => "Crosshairs",
        "HL2MPTypeDeath" => "HL2MPTypeDeath",
        "QuickInfo" => "QuickInfo",
        _ => "WeaponIcons",
    }
}

/// A scheme font's name as the draw list holds it.
pub fn static_font(name: &str) -> &'static str {
    font_name(name)
}
