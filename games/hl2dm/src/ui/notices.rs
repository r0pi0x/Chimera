//! Death notices and the target ID (specs/hl2dm/hud.md 10 and 11). A
//! notice is a line at the top right: killer, the weapon's icon, victim
//! (a suicide only the skull and the victim), names in their teams'
//! colours, the icon in `255 80 0`; at most four lines, six seconds each,
//! no fade. The target ID names the player under the crosshair: "Enemy:
//! name", or in team play for a teammate "Friend: name Health: n%".

use bevy::prelude::*;

use super::{
    aux_power::team_colour,
    frame::{Align, Draw, Frame, Rgba},
    hud::Hl2Hud,
    look::Look,
};

/// `hud_deathnotice_time`.
pub const LIFE: f64 = 6.0;
/// `MaxDeathNotices`.
pub const MAX: usize = 4;
/// Every icon's colour.
pub const ICON_COLOUR: Rgba = [255, 80, 0, 255];

#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    /// The killer's name and team (None: a suicide or the world).
    pub killer: Option<(String, u8)>,
    pub victim: (String, u8),
    /// The weapon's notice name (`smg1`, `physics`, `trigger_hurt`).
    pub weapon: String,
    pub until: f64,
}

impl Notice {
    /// A suicide: no killer (the caller leaves it out when the killer is
    /// the victim: players may share a name).
    pub fn suicide(&self) -> bool {
        self.killer.is_none()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Notices {
    pub list: Vec<Notice>,
    /// The player under the crosshair, set each frame.
    pub target: Option<Target>,
}

/// The target ID's subject.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub name: String,
    pub team: u8,
    /// A teammate in team play, with its health over its maximum.
    pub friend: Option<f32>,
}

impl Hl2Hud {
    /// A `player_death`: the oldest notice goes when four are up.
    pub fn death_notice(&mut self, killer: Option<(String, u8)>, victim: (String, u8), weapon: &str) {
        if self.notices.list.len() >= MAX {
            self.notices.list.remove(0);
        }
        self.notices.list.push(Notice { killer, victim, weapon: weapon.to_lowercase(), until: self.now + LIFE });
    }

    pub(super) fn draw_notices(&self, look: &Look, frame: &mut Frame) {
        let s = look.screen;
        let tall = look.tall("HudNumbersTimer");
        let line = s.scaled(15.0) + s.scaled(4.0);
        for (i, n) in self.notices.list.iter().enumerate() {
            let y = s.scaled(16.0) + i as f32 * line as f32;
            let mut right = s.w - s.scaled(16.0);
            let victim_w = look.width("HudNumbersTimer", &n.victim.0);
            frame.push(Draw::text("HudNumbersTimer", tall, n.victim.0.clone(), team_colour(n.victim.1), Vec2::new(right, y), Align::Right));
            right -= victim_w;
            // The icon: a glyph of the notice font, else the skull (every
            // suicide, and weapons without an icon).
            let glyph = if n.suicide() { None } else { look.glyph(&format!("death_{}", n.weapon)) };
            let icon_w = match glyph {
                Some((font, ch)) => {
                    let f = super::numeric::static_font(&font);
                    let w = look.width(f, &ch.to_string());
                    frame.push(Draw::text(f, look.tall(f), ch.to_string(), ICON_COLOUR, Vec2::new(right - w, y), Align::Left));
                    w
                }
                None => match look.hud.sprites.get("d_skull") {
                    Some(sp) => {
                        let (w, h) = (s.scaled(sp.rect[2]), s.scaled(sp.rect[3]));
                        let min = Vec2::new(right - w, y);
                        frame.push(Draw::Sprite { sprite: "d_skull".into(), rect: Rect::from_corners(min, min + Vec2::new(w, h)), color: ICON_COLOUR, additive: true, flip: [false; 2] });
                        w
                    }
                    None => 0.0,
                },
            };
            right -= icon_w;
            if !n.suicide()
                && let Some((name, team)) = &n.killer
            {
                frame.push(Draw::text("HudNumbersTimer", tall, name.clone(), team_colour(*team), Vec2::new(right, y), Align::Right));
            }
        }
    }

    /// The target ID line, centred at y 260 units.
    pub fn draw_target(&self, look: &Look, frame: &mut Frame) {
        let Some(t) = &self.notices.target else { return };
        let s = look.screen;
        let text = match t.friend {
            Some(h) => look
                .string("#Playerid_sameteam", "Friend: %s1 Health: %s2")
                .replace("%s1", &t.name)
                .replace("%s2", &format!("{}%", h as i32)),
            None => look.string("#Playerid_diffteam", "Enemy: %s1").replace("%s1", &t.name),
        };
        frame.push(Draw::text("TargetID", look.tall("TargetID"), text, team_colour(t.team), Vec2::new(s.w / 2.0, s.scaled(260.0)), Align::Centre));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suicides_are_the_victims_alone() {
        let v = ("B".to_string(), 0);
        let n = |k: Option<(String, u8)>| Notice { killer: k, victim: v.clone(), weapon: "smg1".into(), until: 1.0 };
        assert!(n(None).suicide());
        assert!(!n(Some(("A".into(), 0))).suicide());
    }
}
