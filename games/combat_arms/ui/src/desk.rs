//! Combat Arms' screens and popups as the pointer and keys see them
//! (specs/combat_arms/ui_system.md I1, I3, M2), as plain data: `Desk`
//! holds the top-level layouts in draw order (Chimera's `canvas::Layers`)
//! with the modal capture stack, and turns pointer moves and presses into
//! hover states, interface sounds and fired commands.
//!
//! - A press goes to the top capture alone while a modal popup shows,
//!   else to each tree from the top down until one takes it; in a tree,
//!   to the deepest widget under the pointer, then its ancestors, the
//!   first enabled button taking it (`Layout::click_target`). It fires on
//!   the left button going *down*, with the button's click sound.
//! - Moving: every visible widget under the pointer hovers, the rest
//!   don't; a button plays its hover sound when the pointer enters it, not
//!   again while it stays.
//! - A tree whose root takes no mouse (HUD panels) is click-through.

use bevy::math::IVec2;

use super::layout::{Ctx, Kind, Layout, Sound, Textures};
use crate::client::canvas::{Layers, Tree, WidgetId};

/// What happened on the desk, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// An interface sound to play.
    Sound(Sound),
    /// A button fired: its tree, its name and command id.
    Fired { tree: usize, name: String, cmd: i32 },
}

/// A top-level tree.
#[derive(Clone, Debug)]
pub struct Sheet {
    pub name: String,
    pub layout: Layout,
    pub shown: bool,
    /// Takes the pointer (screens and popups; not HUD panels).
    pub mouse: bool,
}

/// The top-level trees, their order and capture, and what happened.
#[derive(Clone, Debug, Default)]
pub struct Desk {
    pub sheets: Vec<Sheet>,
    pub layers: Layers<usize>,
    pub events: Vec<Event>,
    /// The tree and button the left button went down on.
    pub pressed: Option<(usize, WidgetId)>,
}

impl Desk {
    /// Add a tree on top (hidden until shown); its index.
    pub fn add(&mut self, name: &str, layout: Layout, mouse: bool) -> usize {
        self.sheets.push(Sheet {
            name: name.to_string(),
            layout,
            shown: false,
            mouse,
        });
        let i = self.sheets.len() - 1;
        self.layers.raise(i);
        i
    }

    pub fn by_name(&self, name: &str) -> Option<usize> {
        self.sheets.iter().position(|s| s.name == name)
    }

    /// Show a tree: on top, captured if modal.
    pub fn show(&mut self, i: usize, modal: bool) {
        self.sheets[i].shown = true;
        self.layers.show(i, modal);
    }

    pub fn hide(&mut self, i: usize) {
        self.sheets[i].shown = false;
        self.layers.hide(i);
        let l = &mut self.sheets[i].layout;
        for id in l.tree.order() {
            let w = &mut l.tree.get_mut(id).data;
            w.hover = false;
            w.held = false;
        }
        if self.pressed.is_some_and(|(t, _)| t == i) {
            self.pressed = None;
        }
    }

    fn takes(&self, i: usize) -> bool {
        let s = &self.sheets[i];
        s.shown && s.mouse
    }

    /// The pointer moved to `p` (screen px).
    pub fn pointer_moved(&mut self, p: IVec2, ctx: &Ctx<impl Textures>) {
        let order = self.layers.pointer_order();
        for i in 0..self.sheets.len() {
            let under: Vec<WidgetId> = if order.contains(&i) && self.takes(i) {
                let l = &self.sheets[i].layout;
                l.tree
                    .order()
                    .into_iter()
                    .filter(|&id| {
                        id != Tree::<()>::ROOT
                            && l.tree.visible(id)
                            && l.tree.get(id).mouse
                            && l.hit_rect(id, ctx).contains(p)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let l = &mut self.sheets[i].layout;
            for id in l.tree.order() {
                let now = under.contains(&id);
                let w = &mut l.tree.get_mut(id).data;
                if now && !w.hover && w.enabled && let Kind::Button(b) = &w.kind {
                    self.events.push(Event::Sound(b.over));
                }
                w.hover = now;
            }
        }
    }

    /// The left button went down at `p`: the button it lands on fires.
    pub fn left_down(&mut self, p: IVec2, ctx: &Ctx<impl Textures>) {
        for i in self.layers.pointer_order() {
            if !self.takes(i) {
                continue;
            }
            let l = &self.sheets[i].layout;
            if let Some(id) = l.click_target(p, ctx) {
                let n = l.tree.get(id);
                let (name, cmd) = (n.name.clone(), n.data.cmd);
                if let Kind::Button(b) = &n.data.kind {
                    self.events.push(Event::Sound(b.click));
                }
                self.events.push(Event::Fired { tree: i, name, cmd });
                self.sheets[i].layout.widget_mut(id).held = true;
                self.pressed = Some((i, id));
                return;
            }
            if self.layers.captured() == Some(i) {
                return;
            }
        }
    }

    /// The left button came up.
    pub fn left_up(&mut self) {
        if let Some((t, id)) = self.pressed.take() {
            self.sheets[t].layout.widget_mut(id).held = false;
        }
    }

    /// This frame's events, taken.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

/// `Sound`s that played (`Event::Sound`, IS_NONE plays nothing).
pub fn sounds(events: &[Event]) -> Vec<Sound> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Sound(s) if *s != Sound::None => Some(*s),
            _ => None,
        })
        .collect()
}
