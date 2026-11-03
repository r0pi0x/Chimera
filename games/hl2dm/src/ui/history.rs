//! The pickup history (specs/hl2dm/hud.md 9): a column of icons down the
//! right, newest at the bottom, for weapons, ammo (with its count), items
//! (battery, health kit) and refused ammo ("FULL"). Five rows at most: a
//! counter picks the next slot and the sixth entry overwrites the first.
//! An entry lives five seconds (2.5 for "FULL") and fades over its last
//! 3.2: alpha is 80 per second left.

use bevy::prelude::*;

use super::{
    frame::{Align, Draw, Frame, fade},
    hud::Hl2Hud,
    look::Look,
};

/// `hud_drawhistory_time`.
pub const LIFE: f64 = 5.0;
/// An ammo-denied entry's life.
pub const DENIED_LIFE: f64 = 2.5;
/// A row's height and the column's (units).
pub const GAP: f32 = 56.0;
pub const COLUMN: f32 = 320.0;

/// An icon: its scheme font and glyph.
pub type Glyph = (&'static str, char);

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub slot: usize,
    pub icon: Option<Glyph>,
    /// The count beside the icon (ammo: what was added).
    pub count: Option<i32>,
    /// Text instead (`FULL`).
    pub text: Option<String>,
    /// `Normal`, or `Caution` (no ammo, refused).
    pub caution: bool,
    /// The weapon it announces (no second row for the same one).
    pub weapon: Option<Entity>,
    /// The ammo type of an ammo or refused entry.
    pub ammo: Option<String>,
    pub denied: bool,
    /// When it dies (client time).
    pub until: f64,
}

#[derive(Clone, Debug, Default)]
pub struct History {
    pub entries: Vec<Entry>,
    counter: usize,
}

impl History {
    pub fn clear(&mut self) {
        *self = History::default();
    }

    /// Entries gone by `now`; the counter goes back to 0 when none is left.
    pub fn expire(&mut self, now: f64) {
        self.entries.retain(|e| e.until > now);
        if self.entries.is_empty() {
            self.counter = 0;
        }
    }

    fn add(&mut self, mut e: Entry) {
        if GAP * (self.counter as f32 + 1.0) > COLUMN {
            self.counter = 0;
        }
        e.slot = self.counter;
        self.counter += 1;
        // A new entry in a slot takes it over.
        self.entries.retain(|o| o.slot != e.slot);
        self.entries.push(e);
    }

    fn entry(now: f64, life: f64) -> Entry {
        Entry { slot: 0, icon: None, count: None, text: None, caution: false, weapon: None, ammo: None, denied: false, until: now + life }
    }

    /// A weapon became the player's (not an exhaustible one, nor one
    /// already listed).
    pub fn weapon(&mut self, weapon: Entity, icon: Option<Glyph>, no_ammo: bool, exhaustible: bool, now: f64) {
        if exhaustible || self.entries.iter().any(|e| e.weapon == Some(weapon)) {
            return;
        }
        self.add(Entry { icon, caution: no_ammo, weapon: Some(weapon), ..Self::entry(now, LIFE) });
    }

    /// Ammo rose by `count`; `icon` is the first owned weapon's ammo icon
    /// (None: a blank row that still takes its slot). A refused-ammo entry
    /// of the type is killed.
    pub fn ammo(&mut self, ammo: &str, count: i32, icon: Option<Glyph>, now: f64) {
        if let Some(d) = self.entries.iter().position(|e| e.denied && e.ammo.as_deref() == Some(ammo)) {
            let slot = self.entries.remove(d).slot;
            // Its slot is reused.
            self.counter = slot;
        }
        self.add(Entry { icon, count: Some(count), ammo: Some(ammo.into()), ..Self::entry(now, LIFE) });
    }

    /// A battery or a health kit.
    pub fn item(&mut self, icon: Option<Glyph>, now: f64) {
        if icon.is_some() {
            self.add(Entry { icon, ..Self::entry(now, LIFE) });
        }
    }

    /// A pickup refused: the type is full.
    pub fn denied(&mut self, ammo: &str, now: f64) {
        if self.entries.iter().any(|e| !e.denied && e.ammo.as_deref() == Some(ammo)) {
            return;
        }
        if let Some(e) = self.entries.iter_mut().find(|e| e.denied && e.ammo.as_deref() == Some(ammo)) {
            e.until = now + DENIED_LIFE;
            return;
        }
        self.add(Entry { text: Some("FULL".into()), caution: true, ammo: Some(ammo.into()), denied: true, ..Self::entry(now, DENIED_LIFE) });
    }
}

impl Hl2Hud {
    pub(super) fn draw_history(&self, look: &Look, frame: &mut Frame) {
        let Some(p) = look.panel("HudHistoryResource") else { return };
        let s = look.screen;
        let origin = Vec2::new(s.x(p.x), s.y(p.y));
        let (inset_icon, inset_text) = (look.key("HudHistoryResource", "icon_inset"), look.key("HudHistoryResource", "text_inset"));
        for e in &self.history.entries {
            let left = (e.until - self.now).min(LIFE);
            if left <= 0.0 {
                continue;
            }
            let alpha = (80.0 * left).min(255.0) as f32 / 255.0;
            let colour = fade(look.colour(if e.caution { "Caution" } else { "Normal" }), alpha);
            let y = origin.y + s.scaled(COLUMN - GAP * (e.slot as f32 + 1.0));
            let mut icon_h = 0.0;
            if let Some((font, ch)) = e.icon {
                let (w, h) = (look.width(font, &ch.to_string()), look.tall(font));
                icon_h = h;
                let x = origin.x + s.scaled(p.wide) - w - s.scaled(inset_icon);
                frame.push(Draw::text(font, h, ch.to_string(), colour, Vec2::new(x, y), Align::Left));
            }
            let x = origin.x + s.scaled(p.wide) - s.scaled(inset_text);
            if let Some(c) = e.count.filter(|c| *c > 0) {
                let h = look.tall("HudNumbersSmall");
                frame.push(Draw::text("HudNumbersSmall", h, c.to_string(), colour, Vec2::new(x, y - ((h - icon_h) / 2.0).trunc()), Align::Left));
            }
            if let Some(t) = &e.text {
                let h = look.tall("Default");
                frame.push(Draw::text("Default", h, t.clone(), colour, Vec2::new(x, y - ((h - icon_h) / 2.0).trunc()), Align::Left));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    /// Spec test 55: six pickups in two seconds fill rows 0 to 4 and the
    /// sixth replaces row 0.
    #[test]
    fn six_entries_wrap_onto_the_first_row() {
        let mut h = History::default();
        for i in 0..6 {
            h.weapon(w(i + 1), Some(("WeaponIcons", 'a')), false, false, i as f64 / 3.0);
        }
        assert_eq!(h.entries.len(), 5);
        let slots: Vec<_> = h.entries.iter().map(|e| (e.weapon, e.slot)).collect();
        assert!(slots.contains(&(Some(w(6)), 0)), "{slots:?}");
        assert!(!slots.iter().any(|(e, _)| *e == Some(w(1))), "the first row was replaced");
    }

    /// Spec test 54: a refused pickup twice, a second apart: one row.
    #[test]
    fn refused_ammo_is_one_row() {
        let mut h = History::default();
        h.denied("hl2dm:smg1", 0.0);
        h.denied("hl2dm:smg1", 1.0);
        assert_eq!(h.entries.len(), 1);
        assert_eq!(h.entries[0].until, 3.5);
        assert!(h.entries[0].caution && h.entries[0].text.as_deref() == Some("FULL"));
        // A listed ammo entry of the type ignores the refusal.
        h.ammo("hl2dm:pistol", 20, None, 1.0);
        h.denied("hl2dm:pistol", 1.1);
        assert_eq!(h.entries.iter().filter(|e| e.denied).count(), 1);
    }

    /// Spec test 57: a weapon that is exhaustible gets no row of its own;
    /// the same weapon listed twice shows once.
    #[test]
    fn exhaustible_weapons_and_repeats() {
        let mut h = History::default();
        h.weapon(w(1), None, false, true, 0.0);
        assert!(h.entries.is_empty());
        h.weapon(w(2), None, false, false, 0.0);
        h.weapon(w(2), None, false, false, 0.5);
        assert_eq!(h.entries.len(), 1);
        h.expire(10.0);
        assert!(h.entries.is_empty());
    }

    /// Spec test 56: a vial leaves nothing (no icon), a kit a row.
    #[test]
    fn items_without_icons_add_nothing() {
        let mut h = History::default();
        h.item(None, 0.0);
        assert!(h.entries.is_empty());
        h.item(Some(("WeaponIcons", '+')), 0.0);
        assert_eq!(h.entries.len(), 1);
    }
}
