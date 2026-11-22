//! Reference-canvas UI for any game whose screens are trees of widgets laid
//! out in reference pixels on a fixed canvas and stretched to the screen
//! (Combat Arms' 1024x768 layouts first, `combat_arms_ui::layout`), as
//! plain data with no ECS or drawing, so a game's layout rules are unit
//! tested headless:
//!
//! - `Scale`: the screen's size over the reference canvas, (x_r, y_r).
//! - `Tree`: widgets in document order (draw order), each placed relative
//!   to its parent; absolute positions, lookup by name (case-insensitive,
//!   depth first, first match), copying a subtree into another tree (a
//!   panel cloned out of a shared file), visibility through ancestors, and
//!   the pointer's descent (`Tree::hit`: the last visible child that
//!   contains the pointer, level by level; a widget that takes no mouse
//!   makes its whole subtree click-through) with the game's own rects.
//! - `Layers`: top-level trees in draw order with a capture stack for
//!   modal ones (all pointer input to the top capture while there is one),
//!   and `centre` for a popup centred on the screen.
//! - Text: `HAlign`/`VAlign` inside a parent box (`align`), measuring and
//!   wrapping by a font's glyph advances (`Metrics`, `measure`, `wrap`).
//! - Timers: `Fade` (in and out over a time, alpha truncated), `Blink`
//!   (hidden for the end of each period, a finite count of times), and a
//!   gauge's bar width (`gauge_width`).
//!
//! What differs by game stays the game's: how sizes round on screen, its
//! hit rects, line spacing, text effects, its file formats and strings.
//! CS:S's VGUI screens don't use this (they are `client::vgui` and
//! `client::widgets`).

use bevy::math::{IVec2, Vec2};

// ---------------------------------------------------------------------------
// Scale

/// A tree's screen scale: screen px per reference px on each axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    pub x: f64,
    pub y: f64,
}

impl Default for Scale {
    fn default() -> Self {
        Self::ONE
    }
}

impl Scale {
    pub const ONE: Self = Self { x: 1.0, y: 1.0 };

    /// A `screen` stretched from a `reference` canvas, each axis on its
    /// own (no aspect correction).
    pub fn of(screen: Vec2, reference: Vec2) -> Self {
        Self {
            x: screen.x as f64 / reference.x as f64,
            y: screen.y as f64 / reference.y as f64,
        }
    }

    pub fn is_one(self) -> bool {
        self == Self::ONE
    }
}

// ---------------------------------------------------------------------------
// Trees

/// A widget's index in its tree.
pub type WidgetId = usize;

/// A widget: its place (relative to its parent, reference px), size,
/// flags and the game's own data.
#[derive(Clone, Debug, PartialEq)]
pub struct Node<T> {
    pub name: String,
    pub pos: IVec2,
    pub size: IVec2,
    /// Shown (else it and its subtree are neither drawn nor hit).
    pub show: bool,
    /// Takes part in hit tests (else its subtree is click-through).
    pub mouse: bool,
    pub parent: Option<WidgetId>,
    pub children: Vec<WidgetId>,
    pub data: T,
}

impl<T> Node<T> {
    pub fn new(name: impl Into<String>, pos: IVec2, size: IVec2, data: T) -> Self {
        Self {
            name: name.into(),
            pos,
            size,
            show: true,
            mouse: true,
            parent: None,
            children: Vec::new(),
            data,
        }
    }
}

/// Widgets under one root (index 0), children in document order: a later
/// sibling draws over an earlier one and wins hit tests.
#[derive(Clone, Debug, PartialEq)]
pub struct Tree<T> {
    nodes: Vec<Node<T>>,
}

impl<T> Tree<T> {
    /// A tree of only its root.
    pub fn new(root: Node<T>) -> Self {
        Self { nodes: vec![root] }
    }

    pub const ROOT: WidgetId = 0;

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Add `node` as `parent`'s last child.
    pub fn add(&mut self, parent: WidgetId, mut node: Node<T>) -> WidgetId {
        let id = self.nodes.len();
        node.parent = Some(parent);
        node.children.clear();
        self.nodes.push(node);
        self.nodes[parent].children.push(id);
        id
    }

    pub fn get(&self, id: WidgetId) -> &Node<T> {
        &self.nodes[id]
    }

    pub fn get_mut(&mut self, id: WidgetId) -> &mut Node<T> {
        &mut self.nodes[id]
    }

    /// The sum of its own position and all its ancestors'.
    pub fn abs_pos(&self, id: WidgetId) -> IVec2 {
        let mut at = IVec2::ZERO;
        let mut n = Some(id);
        while let Some(i) = n {
            at += self.nodes[i].pos;
            n = self.nodes[i].parent;
        }
        at
    }

    /// Its ancestors, nearest first (the root last).
    pub fn ancestors(&self, id: WidgetId) -> impl Iterator<Item = WidgetId> + '_ {
        std::iter::successors(self.nodes[id].parent, |&p| self.nodes[p].parent)
    }

    /// Shown, with every ancestor shown.
    pub fn visible(&self, id: WidgetId) -> bool {
        self.nodes[id].show && self.ancestors(id).all(|a| self.nodes[a].show)
    }

    /// Every widget under `from` (itself first), depth first in document
    /// order: the draw order.
    pub fn order_from(&self, from: WidgetId) -> Vec<WidgetId> {
        let mut out = Vec::new();
        let mut stack = vec![from];
        while let Some(i) = stack.pop() {
            out.push(i);
            stack.extend(self.nodes[i].children.iter().rev());
        }
        out
    }

    /// The whole tree in draw order.
    pub fn order(&self) -> Vec<WidgetId> {
        self.order_from(Self::ROOT)
    }

    /// The first widget named `name` (any case), depth first from `from`.
    pub fn find_from(&self, from: WidgetId, name: &str) -> Option<WidgetId> {
        self.order_from(from)
            .into_iter()
            .find(|&i| self.nodes[i].name.eq_ignore_ascii_case(name))
    }

    pub fn find(&self, name: &str) -> Option<WidgetId> {
        self.find_from(Self::ROOT, name)
    }

    /// The pointer's widget: from the root down, at each level the last
    /// visible child that takes the mouse and `contains` the pointer,
    /// until none does. None when no child of the root does.
    pub fn hit(&self, contains: impl Fn(WidgetId) -> bool) -> Option<WidgetId> {
        let mut at = Self::ROOT;
        loop {
            let next = self.nodes[at]
                .children
                .iter()
                .rev()
                .copied()
                .find(|&c| self.nodes[c].show && self.nodes[c].mouse && contains(c));
            match next {
                Some(c) => at = c,
                None => return (at != Self::ROOT).then_some(at),
            }
        }
    }
}

impl<T: Clone> Tree<T> {
    /// Copy `from`'s subtree out of `source` under `parent`; the copy's
    /// top.
    pub fn graft(&mut self, parent: WidgetId, source: &Tree<T>, from: WidgetId) -> WidgetId {
        let top = self.add(parent, source.nodes[from].clone());
        for &c in &source.nodes[from].children {
            self.graft(top, source, c);
        }
        top
    }
}

// ---------------------------------------------------------------------------
// Layers

/// Top-level trees in draw order (the last is topmost) and the capture
/// stack of the modal ones shown.
#[derive(Clone, Debug, PartialEq)]
pub struct Layers<K> {
    order: Vec<K>,
    capture: Vec<K>,
}

impl<K> Default for Layers<K> {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            capture: Vec::new(),
        }
    }
}

impl<K: Copy + PartialEq> Layers<K> {
    /// Put `k` on top of the draw order (added if new).
    pub fn raise(&mut self, k: K) {
        self.order.retain(|o| *o != k);
        self.order.push(k);
    }

    /// Shown: on top, and on the capture stack if modal.
    pub fn show(&mut self, k: K, modal: bool) {
        self.raise(k);
        self.capture.retain(|o| *o != k);
        if modal {
            self.capture.push(k);
        }
    }

    /// Hidden: off the capture stack (it keeps its place in the order).
    pub fn hide(&mut self, k: K) {
        self.capture.retain(|o| *o != k);
    }

    /// Bottom to top.
    pub fn order(&self) -> &[K] {
        &self.order
    }

    /// The trees a pointer event is offered to, in turn: the top capture
    /// alone while there is one, else every tree from the top down.
    pub fn pointer_order(&self) -> Vec<K> {
        match self.capture.last() {
            Some(k) => vec![*k],
            None => self.order.iter().rev().copied().collect(),
        }
    }

    pub fn captured(&self) -> Option<K> {
        self.capture.last().copied()
    }
}

/// A `size` box centred on a `screen`: each half floored on its own.
pub fn centre(screen: IVec2, size: IVec2) -> IVec2 {
    IVec2::new(
        screen.x.div_euclid(2) - size.x.div_euclid(2),
        screen.y.div_euclid(2) - size.y.div_euclid(2),
    )
}

// ---------------------------------------------------------------------------
// Text

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HAlign {
    /// The text's own position.
    #[default]
    Default,
    Left,
    Right,
    Centre,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VAlign {
    #[default]
    Default,
    Top,
    Bottom,
    Centre,
}

/// One axis of a box of length `len` aligned in a parent at `parent` of
/// length `parent_len` (its own position `own` when `Default`): left or
/// top at the parent's, centred by floored halves, right or bottom at its
/// end.
pub fn align_axis(own: i32, parent: i32, parent_len: i32, len: i32, start: bool, centre: bool, end: bool) -> i32 {
    if start {
        parent
    } else if centre {
        parent + parent_len.div_euclid(2) - len.div_euclid(2)
    } else if end {
        parent + parent_len - len
    } else {
        own
    }
}

/// A box of `size` at `own` aligned inside a parent box (`parent_at`,
/// `parent_size`), each axis on its own.
pub fn align(own: IVec2, size: IVec2, parent_at: IVec2, parent_size: IVec2, h: HAlign, v: VAlign) -> IVec2 {
    IVec2::new(
        align_axis(own.x, parent_at.x, parent_size.x, size.x, h == HAlign::Left, h == HAlign::Centre, h == HAlign::Right),
        align_axis(own.y, parent_at.y, parent_size.y, size.y, v == VAlign::Top, v == VAlign::Centre, v == VAlign::Bottom),
    )
}

/// A font's sizes in screen px: each glyph's advance (no kerning) and the
/// cell height.
pub trait Metrics {
    fn advance(&self, c: char) -> i32;
    fn height(&self) -> i32;
}

/// A string's width: the sum of its glyphs' advances.
pub fn measure(s: &str, m: &impl Metrics) -> i32 {
    s.chars().map(|c| m.advance(c)).sum()
}

/// `s` broken into lines no wider than `width` px, a character at a time:
/// a line that grows past `width` is cut just after its last space or tab
/// (which stays at the end of the upper line), or, with none, before the
/// character that overflowed (dropped if it is a space, tab or newline); a
/// newline ends a line and is not kept.
pub fn wrap(s: &str, m: &impl Metrics, width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line: Vec<char> = Vec::new();
    // Just after the last space or tab on the line.
    let mut cut: Option<usize> = None;
    for c in s.chars() {
        if c == '\n' {
            lines.push(line.drain(..).collect());
            cut = None;
            continue;
        }
        line.push(c);
        if c == ' ' || c == '\t' {
            cut = Some(line.len());
        }
        let w: i32 = line.iter().map(|&c| m.advance(c)).sum();
        if w > width {
            match cut.take() {
                Some(at) => {
                    let rest = line.split_off(at);
                    lines.push(line.drain(..).collect());
                    line = rest;
                }
                None => {
                    let last = line.pop().unwrap();
                    if !line.is_empty() {
                        lines.push(line.drain(..).collect());
                    }
                    if !matches!(last, ' ' | '\t') {
                        line.push(last);
                    }
                }
            }
        }
    }
    lines.push(line.into_iter().collect());
    lines
}

// ---------------------------------------------------------------------------
// Timers

/// A fade over `length` ms from `start`: alpha truncated, ⌊255 t / T⌋ in
/// and 255 − that out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fade {
    pub start: f64,
    pub length: f64,
    pub fade_in: bool,
}

impl Fade {
    /// The alpha at `now`, None once a fade-out is over (hidden); a
    /// fade-in over is 255.
    pub fn alpha(&self, now: f64) -> Option<u8> {
        let t = now - self.start;
        let done = t >= self.length;
        let part = if done { 255 } else { ((255.0 * t.max(0.0)) / self.length).floor() as u8 };
        match (self.fade_in, done) {
            (true, _) => Some(part),
            (false, true) => None,
            (false, false) => Some(255 - part),
        }
    }

    pub fn done(&self, now: f64) -> bool {
        now - self.start >= self.length
    }
}

/// A blink: hidden while the phase in each `period` is past `period −
/// off` (ms). `count` hides to go (−1: forever); it drops each time the
/// widget hides, and the blink ends on the frame after it reaches 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blink {
    pub start: f64,
    pub period: f64,
    pub off: f64,
    pub count: i32,
    hidden: bool,
    over: bool,
}

impl Blink {
    pub fn new(start: f64, period: f64, off: f64, count: i32) -> Self {
        Self {
            start,
            period,
            off,
            count,
            hidden: false,
            over: false,
        }
    }

    pub fn over(&self) -> bool {
        self.over
    }

    /// One frame at `now`: shown?
    pub fn frame(&mut self, now: f64) -> bool {
        if self.over {
            return true;
        }
        if self.count == 0 {
            self.over = true;
            return true;
        }
        let phase = (now - self.start).rem_euclid(self.period);
        let hidden = phase > self.period - self.off;
        if hidden && !self.hidden && self.count > 0 {
            self.count -= 1;
        }
        self.hidden = hidden;
        !hidden
    }
}

/// A gauge's bar width for a fill of `percent`: None (hidden) at 0 or
/// less, the full `max_range` at 100 or more, else ⌊p · max / 100⌋.
pub fn gauge_width(percent: f64, max_range: i32) -> Option<i32> {
    if percent <= 0.0 {
        None
    } else if percent >= 100.0 {
        Some(max_range)
    } else {
        Some((percent * max_range as f64 / 100.0).floor() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mono(i32, i32);
    impl Metrics for Mono {
        fn advance(&self, _: char) -> i32 {
            self.0
        }
        fn height(&self) -> i32 {
            self.1
        }
    }

    fn tree() -> Tree<()> {
        let mut t = Tree::new(Node::new("root", IVec2::ZERO, IVec2::splat(1024), ()));
        let a = t.add(0, Node::new("Panel", IVec2::new(100, 50), IVec2::new(200, 100), ()));
        t.add(a, Node::new("ok", IVec2::new(140, 70), IVec2::new(50, 20), ()));
        t.add(a, Node::new("OK", IVec2::new(0, 0), IVec2::new(10, 10), ()));
        t
    }

    #[test]
    fn positions_add_up_and_names_match_in_any_case() {
        let t = tree();
        assert_eq!(t.abs_pos(2), IVec2::new(240, 120));
        assert_eq!(t.find("PANEL"), Some(1));
        // The first match, depth first.
        assert_eq!(t.find("ok"), Some(2));
        let mut u = Tree::new(Node::new("r", IVec2::ZERO, IVec2::ZERO, ()));
        let top = u.graft(0, &t, 1);
        assert_eq!(u.get(top).children.len(), 2);
        assert_eq!(u.abs_pos(u.find("ok").unwrap()), IVec2::new(240, 120));
    }

    #[test]
    fn the_pointer_takes_the_last_visible_child_level_by_level() {
        let mut t = tree();
        fn inside(t: &Tree<()>, p: IVec2) -> impl Fn(WidgetId) -> bool + '_ {
            move |i| {
                let (a, s) = (t.abs_pos(i), t.get(i).size);
                p.x >= a.x && p.y >= a.y && p.x < a.x + s.x && p.y < a.y + s.y
            }
        }
        assert_eq!(t.hit(inside(&t, IVec2::new(250, 125))), Some(2));
        assert_eq!(t.hit(inside(&t, IVec2::new(105, 55))), Some(3));
        assert_eq!(t.hit(inside(&t, IVec2::new(5, 5))), None);
        t.get_mut(1).mouse = false;
        assert_eq!(t.hit(inside(&t, IVec2::new(250, 125))), None);
        t.get_mut(1).mouse = true;
        t.get_mut(1).show = false;
        assert!(!t.visible(2));
        assert_eq!(t.hit(inside(&t, IVec2::new(250, 125))), None);
    }

    #[test]
    fn layers_capture_the_pointer_while_a_modal_one_shows() {
        let mut l = Layers::default();
        l.raise(1);
        l.raise(2);
        assert_eq!(l.pointer_order(), vec![2, 1]);
        l.show(3, true);
        assert_eq!(l.pointer_order(), vec![3]);
        l.hide(3);
        assert_eq!(l.pointer_order(), vec![3, 2, 1]);
        assert_eq!(centre(IVec2::new(1366, 768), IVec2::new(220, 145)), IVec2::new(573, 312));
    }

    #[test]
    fn alignment_wrapping_and_timers() {
        let at = align(
            IVec2::new(7, 9),
            IVec2::new(30, 10),
            IVec2::new(100, 50),
            IVec2::new(200, 100),
            HAlign::Centre,
            VAlign::Default,
        );
        assert_eq!(at, IVec2::new(185, 9));
        let m = Mono(6, 10);
        assert_eq!(wrap("AAA BBB CCC", &m, 30), ["AAA ", "BBB ", "CCC"]);
        assert_eq!(wrap("ABCDEFGH", &m, 30), ["ABCDE", "FGH"]);
        assert_eq!(wrap("A\nB", &m, 30), ["A", "B"]);
        assert_eq!(measure("HELLO", &Mono(8, 13)), 40);
        let f = Fade {
            start: 0.0,
            length: 2000.0,
            fade_in: true,
        };
        assert_eq!(f.alpha(500.0), Some(63));
        assert_eq!(gauge_width(99.9, 120), Some(119));
        assert_eq!(gauge_width(0.0, 120), None);
    }
}
