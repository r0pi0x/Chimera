//! The aux power bar and the team line (specs/hl2dm/hud.md 7). The bar
//! shows only while aux power is below 100 (and while it fades out): ten
//! chunks, a line per active suit device (sprint, breather; no flashlight
//! device in HL2:DM); its size and place, colour and fade are
//! `SuitAuxPower*` events' variables. The team line (team play only) fades
//! away while the bar shows.

use bevy::prelude::*;

use super::{
    frame::{Align, Draw, Frame, fade},
    hud::{Hl2Hud, Inputs, devices},
    look::Look,
};
use crate::game::rules::{COMBINE, REBELS, SPECTATOR, team_name};

/// What the bar remembers.
#[derive(Clone, Debug, Default)]
pub struct Aux {
    /// The value seen last (None after a reset).
    pub last: Option<f32>,
    /// The device count seen last.
    pub items: Option<u8>,
    /// The bar was low (two chunks or fewer lit).
    pub low: bool,
}

impl Aux {
    pub fn reset(&mut self) {
        *self = Aux::default();
    }
}

/// Lit chunks for an aux value: ten chunks, rounded.
pub fn lit_chunks(aux: f32, chunks: u32) -> u32 {
    ((chunks as f32 * aux / 100.0 + 0.5).floor().max(0.0) as u32).min(chunks)
}

/// The team's colour (death notices, scoreboard, target ID, team line).
pub fn team_colour(team: u8) -> [u8; 4] {
    match team {
        SPECTATOR => [204, 204, 204, 255],
        COMBINE => [153, 204, 255, 255],
        REBELS => [255, 64, 64, 255],
        _ => [255, 178, 0, 255],
    }
}

impl Hl2Hud {
    pub(super) fn think_aux(&mut self, inp: &Inputs) {
        let value = inp.aux.clamp(0.0, 100.0);
        if self.aux.last != Some(value) {
            let prev = self.aux.last.unwrap_or(-1.0);
            if value >= 100.0 && prev < 100.0 {
                self.fire("SuitAuxPowerMax");
                self.fire("FadeInTeamLine");
            } else if value < 100.0 && (prev >= 100.0 || self.aux.last.is_none()) {
                self.fire("SuitAuxPowerNotMax");
                self.fire("FadeOutTeamLine");
            }
            self.aux.last = Some(value);
        }
        let n = devices(inp);
        if self.aux.items != Some(n) {
            self.aux.items = Some(n);
            self.fire(match n {
                0 => "SuitAuxPowerNoItemsActive",
                1 => "SuitAuxPowerOneItemActive",
                2 => "SuitAuxPowerTwoItemsActive",
                _ => "SuitAuxPowerThreeItemsActive",
            });
        }
        // Low is re-evaluated, acted on only while a device is on or the
        // value is below 100.
        let low = lit_chunks(value, 10) <= 2;
        if (n > 0 || value < 100.0) && low != self.aux.low {
            self.aux.low = low;
            self.fire(if low { "SuitAuxPowerDecreasedBelow25" } else { "SuitAuxPowerIncreasedAbove25" });
        }
    }

    pub(super) fn draw_aux(&self, look: &Look, inp: &Inputs, frame: &mut Frame) {
        const NAME: &str = "HudSuitPower";
        if inp.hide_health || !inp.alive {
            return;
        }
        let Some(colour) = self.anim.rgba(NAME, "AuxPowerColor") else { return };
        if colour[3] == 0 {
            return;
        }
        let position = self.anim.get(NAME, "Position").and_then(|v| v.pair());
        let size = self.anim.get(NAME, "Size").and_then(|v| v.pair());
        let Some(rect) = look.animated_rect(NAME, position, size) else { return };
        let s = look.screen;
        let bg = self.anim.rgba(NAME, "BgColor").unwrap_or_else(|| look.colour("BgColor"));
        frame.push(Draw::Box { rect, color: bg, round: true });
        // The bar: chunks of the layout's width and gap, the lit ones in
        // the colour, the rest in its RGB at the disabled alpha.
        let (bar_w, chunk_w, gap) = (look.key(NAME, "BarWidth"), look.key(NAME, "BarChunkWidth"), look.key(NAME, "BarChunkGap"));
        let chunks = ((bar_w / (chunk_w + gap)).floor() as u32).max(1);
        let lit = lit_chunks(self.aux.last.unwrap_or(inp.aux), chunks);
        let disabled = look.key(NAME, "AuxPowerDisabledAlpha") as u8;
        let (inset_x, inset_y, bar_h) = (look.key(NAME, "BarInsetX"), look.key(NAME, "BarInsetY"), look.key(NAME, "BarHeight"));
        for i in 0..chunks {
            let x = rect.min.x + s.scaled(inset_x + i as f32 * (chunk_w + gap));
            let min = Vec2::new(x, rect.min.y + s.scaled(inset_y));
            let c = if i < lit { colour } else { [colour[0], colour[1], colour[2], disabled] };
            frame.push(Draw::Box { rect: Rect::from_corners(min, min + Vec2::new(s.scaled(chunk_w), s.scaled(bar_h))), color: c, round: false });
        }
        let tall = look.tall("Default");
        let at = |x: f32, y: f32| rect.min + Vec2::new(s.scaled(x), s.scaled(y));
        frame.push(Draw::text("Default", tall, look.string("#Valve_Hud_AUX_POWER", "AUX POWER"), colour, at(look.key(NAME, "text_xpos"), look.key(NAME, "text_ypos")), Align::Left));
        // A line per active device: oxygen, flashlight, sprint.
        let (x2, y2, gap2) = (look.key(NAME, "text2_xpos"), look.key(NAME, "text2_ypos"), look.key(NAME, "text2_gap"));
        let mut y = y2;
        for (on, token, fallback) in [(inp.breathing, "#Valve_Hud_OXYGEN", "OXYGEN"), (false, "#Valve_Hud_FLASHLIGHT", "FLASHLIGHT"), (inp.sprinting, "#Valve_Hud_SPRINT", "SPRINT")] {
            if on {
                frame.push(Draw::text("Default", tall, look.string(token, fallback), colour, at(x2, y), Align::Left));
                y += gap2;
            }
        }
    }

    /// The team line: "Team Combine" in the team's colour on a box fitted
    /// to the text, in team play while alive; its alpha is the
    /// `FadeOutTeamLine` and `FadeInTeamLine` events'.
    pub(super) fn draw_team_line(&self, look: &Look, inp: &Inputs, frame: &mut Frame) {
        if !inp.teamplay || !inp.alive || inp.hide_misc {
            return;
        }
        let alpha = self.anim.number("TeamDisplay", "Alpha").unwrap_or(255.0) / 255.0;
        if alpha <= 0.0 {
            return;
        }
        let s = look.screen;
        let text = look.string("#Team", "Team %s1").replace("%s1", team_name(inp.team));
        let (tall, w) = (look.tall("Default"), look.width("Default", &text));
        let Some(p) = look.panel("TeamDisplay") else { return };
        let pad = s.scaled(8.0);
        let min = Vec2::new(s.x(p.x), s.y(p.y));
        let rect = Rect::from_corners(min, min + Vec2::new(w + 2.0 * pad, tall + 2.0 * pad));
        frame.push(Draw::Box { rect, color: fade(look.colour("BgColor"), alpha), round: true });
        frame.push(Draw::text("Default", tall, text, fade(team_colour(inp.team), alpha), min + Vec2::splat(pad), Align::Left));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec test 22: aux 100, 55, 25, 24.99 and 0 light 10, 6, 3, 2 and 0
    /// chunks; low (two or fewer) for the last two.
    #[test]
    fn chunks_by_aux() {
        assert_eq!([100.0, 55.0, 25.0, 24.99, 0.0].map(|a| lit_chunks(a, 10)), [10, 6, 3, 2, 0]);
    }

    #[test]
    fn team_colours() {
        assert_eq!(team_colour(COMBINE), [153, 204, 255, 255]);
        assert_eq!(team_colour(REBELS), [255, 64, 64, 255]);
        assert_eq!(team_colour(0), [255, 178, 0, 255]);
    }
}
