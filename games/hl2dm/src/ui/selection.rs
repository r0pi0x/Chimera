//! The weapon selection (specs/hl2dm/hud.md 5): Half-Life 2's horizontal
//! buckets. Six small numbered boxes in a row; the chosen bucket opens
//! into a column of large icon boxes, the highlighted weapon's name under
//! it. `slotN` opens a bucket or steps through it (`select_slot`),
//! `invnext` and `invprev` step through every weapon (`step`), attack
//! confirms and cancels the shot (`confirm`), `cancelselect` closes
//! (`cancel`); with `hud_fastswitch` 1 a slot key switches at once and
//! nothing is drawn. The selection fades after 0.5 s of no command, in
//! `FadeOutWeaponSelectionMenu`, and closes at 1.25 s, a quarter of the
//! way down its fade (a quirk kept).
//!
//! The functions here return what the game should do (`Action`); the Bevy
//! side switches the weapon and plays the sounds.

use bevy::prelude::*;

use super::{
    frame::{Align, Draw, Frame, fade},
    hud::{Hl2Hud, Inputs, Owned},
    look::Look,
    numeric::static_font,
};
use crate::game::hud::icon_key;

/// Buckets 0-5 exist.
pub const BUCKETS: u8 = 6;
/// Idle seconds before the fade, and before the selection closes.
pub const FADE_AFTER: f64 = 0.5;
pub const CLOSE_AFTER: f64 = 1.25;

/// The sounds of the selection.
pub const SOUND_MOVE: &str = "Player.WeaponSelectionMoveSlot";
pub const SOUND_DENY: &str = "Player.DenyWeaponSelection";
pub const SOUND_SELECTED: &str = "Player.WeaponSelected";
pub const SOUND_CLOSE: &str = "Player.WeaponSelectionClose";

/// What a selection command asks of the game.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Sound(&'static str),
    Switch(Entity),
}

#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub open: bool,
    pub highlighted: Option<Entity>,
    last_command: f64,
    faded: bool,
}

/// Weapons in (bucket, position) order.
fn ordered(owned: &[Owned]) -> Vec<&Owned> {
    let mut v: Vec<&Owned> = owned.iter().collect();
    v.sort_by_key(|o| (o.bucket, o.position));
    v
}

impl Selection {
    pub fn close_now(&mut self) {
        self.open = false;
        self.highlighted = None;
    }
}

impl Hl2Hud {
    fn open_selection(&mut self, weapon: Entity) {
        self.selection.highlighted = Some(weapon);
        self.selection.last_command = self.now;
        self.selection.faded = false;
        // Opening, or a command during the fade, runs the open event.
        self.selection.open = true;
        self.fire("OpenWeaponSelectionMenu");
    }

    /// `slotN` (`bucket` 0-5) with `hud_fastswitch` `fast`.
    pub fn select_slot(&mut self, bucket: u8, owned: &[Owned], active: Option<Entity>, fast: i32) -> Vec<Action> {
        if bucket >= BUCKETS {
            return Vec::new();
        }
        let all = ordered(owned);
        let in_bucket: Vec<&&Owned> = all.iter().filter(|o| o.bucket == bucket).collect();
        if fast >= 1 {
            // Start after the weapon in hand if it is in this bucket, else
            // at the bucket's start; the next selectable in order.
            let start = active.and_then(|a| in_bucket.iter().position(|o| o.entity == a).map(|i| i + 1)).unwrap_or(0);
            let next = in_bucket.iter().skip(start).find(|o| o.selectable).or_else(|| in_bucket.iter().find(|o| o.selectable));
            return match next {
                None => vec![Action::Sound(SOUND_DENY)],
                Some(o) if Some(o.entity) == active => Vec::new(),
                Some(o) => vec![Action::Switch(o.entity)],
            };
        }
        let current = self.selection.highlighted.filter(|_| self.selection.open);
        let start = current.and_then(|c| in_bucket.iter().position(|o| o.entity == c).map(|i| i + 1)).unwrap_or(0);
        let found = in_bucket.iter().skip(start).find(|o| o.selectable).or_else(|| in_bucket.iter().find(|o| o.selectable)).map(|o| o.entity);
        if let Some(w) = found {
            self.open_selection(w);
        }
        vec![Action::Sound(SOUND_MOVE)]
    }

    /// `invnext` (+1) or `invprev` (-1).
    pub fn step(&mut self, step: i32, owned: &[Owned], active: Option<Entity>, fast: i32) -> Vec<Action> {
        let order: Vec<&Owned> = ordered(owned).into_iter().filter(|o| o.selectable).collect();
        if order.is_empty() {
            return Vec::new();
        }
        let from = self.selection.highlighted.filter(|_| self.selection.open).or(active);
        let i = from.and_then(|f| order.iter().position(|o| o.entity == f));
        let n = order.len() as i32;
        let next = match i {
            Some(i) => (i as i32 + step).rem_euclid(n),
            None if step < 0 => n - 1,
            None => 0,
        } as usize;
        let w = order[next].entity;
        self.open_selection(w);
        let mut out = vec![Action::Sound(SOUND_MOVE)];
        if fast >= 1 {
            out.extend(self.confirm(owned));
        }
        out
    }

    /// Attack pressed while the selection is open.
    pub fn confirm(&mut self, owned: &[Owned]) -> Vec<Action> {
        let Some(w) = self.selection.highlighted else { return Vec::new() };
        if !owned.iter().find(|o| o.entity == w).is_some_and(|o| o.selectable) {
            return vec![Action::Sound(SOUND_DENY)];
        }
        self.close_selection();
        vec![Action::Switch(w), Action::Sound(SOUND_SELECTED), Action::Sound(SOUND_CLOSE)]
    }

    /// `cancelselect` or Escape.
    pub fn cancel(&mut self) -> Vec<Action> {
        if !self.selection.open {
            return Vec::new();
        }
        self.close_selection();
        vec![Action::Sound(SOUND_CLOSE)]
    }

    fn close_selection(&mut self) {
        self.selection.open = false;
        self.fire("CloseWeaponSelectionMenu");
    }

    pub(super) fn think_selection(&mut self, _inp: &Inputs) {
        if !self.selection.open {
            return;
        }
        // The timers start their events at their own moments, not at the
        // frame that noticed them.
        let last = self.selection.last_command;
        let idle = self.now - last;
        if idle > FADE_AFTER && !self.selection.faded {
            self.selection.faded = true;
            self.fire_at("FadeOutWeaponSelectionMenu", last + FADE_AFTER);
        }
        if idle > CLOSE_AFTER {
            self.selection.open = false;
            self.fire_at("CloseWeaponSelectionMenu", last + CLOSE_AFTER);
        }
    }

    pub(super) fn draw_selection(&self, look: &Look, inp: &Inputs, frame: &mut Frame) {
        const P: &str = "HudWeaponSelection";
        if !self.selection.open || inp.hide_selection || !inp.alive {
            return;
        }
        let Some(highlight) = self.selection.highlighted.and_then(|h| inp.owned.iter().find(|o| o.entity == h)) else { return };
        let s = look.screen;
        let (small, large_w, large_t, gap) = (
            s.scaled(look.key(P, "SmallBoxSize")),
            s.scaled(look.key(P, "LargeBoxWide")).trunc(),
            s.scaled(look.key(P, "LargeBoxTall")).trunc(),
            s.scaled(look.key(P, "BoxGap")),
        );
        let top = look.panel(P).map_or(0.0, |p| s.y(p.y));
        let x0 = ((s.w - (5.0 * (small + gap) + large_w)) / 2.0).floor();
        let (alpha, sel_alpha) = (self.anim.number(P, "Alpha").unwrap_or(0.0), self.anim.number(P, "SelectionAlpha").unwrap_or(0.0));
        let panel_fg = self.anim.rgba(P, "FgColor").unwrap_or([0; 4]);
        let text_colour = self.anim.rgba(P, "TextColor").unwrap_or([0; 4]);
        let (empty_bg, box_bg, selected_bg) = (look.colour("SelectionEmptyBoxBg"), look.colour("SelectionBoxBg"), look.colour("SelectionSelectedBoxBg"));
        let number_fg = look.colour("SelectionNumberFg");
        let selected_fg = look.colour("FgColor");
        let number = |frame: &mut Frame, at: Vec2, n: u8, k: f32| {
            let off = Vec2::new(s.scaled(look.key(P, "SelectionNumberXPos")), s.scaled(look.key(P, "SelectionNumberYPos")));
            frame.push(Draw::text("HudSelectionNumbers", look.tall("HudSelectionNumbers"), (n + 1).to_string(), fade(number_fg, k), at + off, Align::Left));
        };
        let mut x = x0;
        for bucket in 0..BUCKETS {
            let mine: Vec<&Owned> = inp.owned.iter().filter(|o| o.bucket == bucket).collect();
            if bucket == highlight.bucket {
                let last = mine.iter().map(|o| o.position).max().unwrap_or(0);
                let mut y = top;
                let mut first = true;
                for pos in 0..=last {
                    let w = mine.iter().find(|o| o.position == pos);
                    if w.is_none() && inp.hide_empty_slots {
                        continue;
                    }
                    let min = Vec2::new(x, y);
                    let rect = Rect::from_corners(min, min + Vec2::new(large_w, large_t));
                    let is_highlight = w.is_some_and(|w| w.entity == highlight.entity);
                    let a = if is_highlight { sel_alpha } else { sel_alpha * alpha / 255.0 } / 255.0;
                    let bg = match w {
                        None => empty_bg,
                        Some(_) if is_highlight => selected_bg,
                        Some(_) => box_bg,
                    };
                    frame.push(Draw::Box { rect, color: fade(bg, a), round: true });
                    if first {
                        number(frame, min, bucket, a);
                        first = false;
                    }
                    if let Some(w) = w {
                        draw_weapon(self, look, frame, w, rect, is_highlight, a, selected_fg, panel_fg, text_colour);
                    }
                    y += large_t + gap;
                }
                x += large_w + gap;
            } else {
                let min = Vec2::new(x, top);
                let rect = Rect::from_corners(min, min + Vec2::splat(small));
                let a = alpha / 255.0;
                if mine.is_empty() {
                    frame.push(Draw::Box { rect, color: fade(empty_bg, a), round: true });
                } else {
                    frame.push(Draw::Box { rect, color: fade(box_bg, a), round: true });
                    number(frame, min, bucket, a);
                }
                x += small + gap;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_weapon(hud: &Hl2Hud, look: &Look, frame: &mut Frame, w: &Owned, rect: Rect, highlighted: bool, a: f32, selected_fg: [u8; 4], panel_fg: [u8; 4], text_colour: [u8; 4]) {
    let (large_w, large_t) = (rect.width(), rect.height());
    let glyph = |frame: &mut Frame, icon: &str, colour: [u8; 4]| {
        let Some((font, ch)) = look.glyph(&icon_key(&w.class, icon)) else { return };
        let f = static_font(&font);
        let (gw, gh) = (look.width(f, &ch.to_string()), look.tall(f));
        let at = Vec2::new(rect.min.x + ((large_w - gw) / 2.0).trunc(), rect.min.y + ((large_t - gh) / 2.0).trunc());
        frame.push(Draw::text(f, gh, ch.to_string(), colour, at, Align::Left));
    };
    if highlighted {
        let colour = [selected_fg[0], selected_fg[1], selected_fg[2], (a * 255.0).round() as u8];
        let colour = if w.selectable { colour } else { [255, 0, 0, colour[3]] };
        glyph(frame, "weapon_s", colour);
        glyph(frame, "weapon", colour);
        // The name under it, centred line by line.
        let name = look.string(&w.print_name, &w.class);
        let s = look.screen;
        let tall = look.tall("HudSelectionText");
        let scan = hud.anim.number("HudWeaponSelection", "TextScan").unwrap_or(1.0).clamp(0.0, 1.0);
        let shown = (name.chars().count() as f32 * scan).round() as usize;
        let text: String = name.chars().take(shown).collect();
        let lines: Vec<&str> = text.split('\n').collect();
        let y0 = rect.min.y + s.scaled(look.key("HudWeaponSelection", "TextYPos"));
        let last_w = look.width("HudSelectionText", lines.last().copied().unwrap_or(""));
        for (i, line) in lines.iter().enumerate() {
            let line_w = if i == 0 { look.width("HudSelectionText", line) } else { last_w };
            let at = Vec2::new(rect.min.x + ((large_w - line_w) / 2.0).trunc(), y0 + (i as f32 * 1.1 * tall).trunc());
            frame.push(Draw::text("HudSelectionText", tall, line.to_string(), text_colour, at, Align::Left));
        }
    } else {
        let base = if w.selectable { panel_fg } else { [255, 0, 0, 255] };
        glyph(frame, "weapon", fade(base, a));
    }
}
