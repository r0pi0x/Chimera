//! The kit's recycling list (docs/backlog.md, "A recycling `VirtualList`"):
//! a list or a grid of any length that scrolls by the pixel and costs only
//! the rows in view.
//!
//! - **Model** (`VListState`, by key in `VirtualLists`): sections of
//!   items (`VSection`: a key, a count, whether it has a header row), the
//!   folded sections (they drop out, leaving their header), the shape
//!   (`VShape`: `cols` items a row, the header's and the row's heights, the
//!   gap) and the cursor (`VCursor`: a section's header or an item). The
//!   layout (`VLayout`) is the flat run of lines (a header, or a row of up
//!   to `cols` items) with each line's top: pure data, so tests drive it
//!   without a window. `cols` 1 is a list; more is the tile grid,
//!   virtualised by row.
//! - **Build**: `Ui::virtual_list(c, parent, key, frame, view_h, shape)` is
//!   the scroll container (`Ui::scroll`: eased wheel by the pixel, the
//!   Signal bar with its draggable thumb) whose content is as tall as the
//!   whole layout, and a pool of line slots (`VSlot`) created once: as many
//!   lines as the view holds, plus two spare.
//! - **Recycling** (`place_slots`, after the scroll's step): line `L` in
//!   view sits in slot `L % pool`, absolutely placed at its top; a slot's
//!   `VSlot::line` changes only when another line scrolls into it, and
//!   `generation` (`VListState::bump`: the data or the selection changed)
//!   marks them all. The screen binds a slot's text, icon and state in
//!   place on `Changed<VSlot>`: nothing is spawned or despawned while
//!   scrolling. Slots with no line are hidden (`Display::None`).
//! - **Keys** (`VListState::nav`, called by the screen that has the keys):
//!   ↑↓ the line above or below (a header is a stop; in a row the same
//!   column, or its last item), ←→ the item before or after in reading
//!   order (wrapping row to row and section to section; a folded
//!   section's header is a stop in its place), Home/End, PageUp/PageDown
//!   by a view. On a header, ← folds, →
//!   opens, and `toggle_fold` (Enter, Space) flips it. A move asks the
//!   view to reveal the cursor's line (eased, `reveal_margin`).
//! - **Pointer, the kit-wide selector rule** (the design rules, section 21:
//!   double-click): a press on an item (`VHit` on the node the screen
//!   makes clickable) selects it (`VListEvent::Select`); a second press on
//!   the same item within `DOUBLE_CLICK_SECS` is its primary action
//!   (`VListEvent::Activate`: switch to the gun, pick the character, join
//!   the server). A press on a header folds or opens it (`VListEvent::Fold`).
//!
//! Known shortcuts: lines have two heights (header and row); a grid's
//! tiles are all one width; no horizontal scrolling; a drag out of the
//! list (onto a pinned slot) is the screen's own.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use bevy::{
    prelude::*,
    ui::{DeprecatedInteraction as Interaction, UiSystems},
};

use crate::{
    controls::{DOUBLE_CLICK_SECS, Ui},
    motion::UiClock,
    scroll::{ReducedMotion, ScrollParts, ScrollTuning, ScrollViews},
    style::UiStyle,
    theme::Theme,
    units::UiUnits,
};

/// A section of the list: its key (what folding remembers), how many
/// items, and whether it shows a header line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VSection {
    pub key: String,
    pub len: usize,
    pub header: bool,
}

impl VSection {
    pub fn new(key: impl Into<String>, len: usize) -> Self {
        VSection { key: key.into(), len, header: true }
    }

    /// A section with no header line (a plain list's one section).
    pub fn plain(key: impl Into<String>, len: usize) -> Self {
        VSection { key: key.into(), len, header: false }
    }
}

/// The list's shape, cq.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VShape {
    /// Items a row (1: a list).
    pub cols: usize,
    pub header_h: f32,
    pub row_h: f32,
    /// Space under each line.
    pub gap: f32,
}

impl VShape {
    pub fn list(row_h: f32) -> Self {
        VShape { cols: 1, header_h: row_h, row_h, gap: 0.0 }
    }

    pub fn grid(cols: usize, header_h: f32, row_h: f32, gap: f32) -> Self {
        VShape { cols: cols.max(1), header_h, row_h, gap }
    }

    /// The shortest line with its gap (how many a view holds at most).
    pub fn min_line(&self) -> f32 {
        self.header_h.min(self.row_h) + self.gap
    }
}

/// A line of the layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VLine {
    Header(usize),
    /// Items `first..first + count` of `section`.
    Row { section: usize, first: usize, count: usize },
}

/// Where the cursor is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VCursor {
    Header(usize),
    Item(usize, usize),
}

/// The flat run of lines (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VLayout {
    pub lines: Vec<VLine>,
    /// Each line's top, cq from the content's top.
    pub tops: Vec<f32>,
    pub heights: Vec<f32>,
    pub total: f32,
}

/// Lays sections out (folded ones keep their header only).
pub fn layout(sections: &[VSection], folded: &BTreeSet<String>, shape: &VShape) -> VLayout {
    let mut out = VLayout::default();
    let mut y = 0.0;
    let mut push = |out: &mut VLayout, l: VLine, h: f32| {
        out.lines.push(l);
        out.tops.push(y);
        out.heights.push(h);
        y += h + shape.gap;
    };
    let cols = shape.cols.max(1);
    for (s, sec) in sections.iter().enumerate() {
        if sec.header {
            push(&mut out, VLine::Header(s), shape.header_h);
            if folded.contains(&sec.key) {
                continue;
            }
        }
        let mut first = 0;
        while first < sec.len {
            let count = cols.min(sec.len - first);
            push(&mut out, VLine::Row { section: s, first, count }, shape.row_h);
            first += count;
        }
    }
    out.total = (y - shape.gap).max(0.0);
    out
}

impl VLayout {
    /// The lines that touch `from..to` (cq).
    pub fn visible(&self, from: f32, to: f32) -> Range<usize> {
        let a = self.tops.partition_point(|&t| t <= from).saturating_sub(1);
        // The first line whose top is at or past `to`.
        let b = self.tops.partition_point(|&t| t < to);
        // `a` may be a line ending above `from` (in the gap): skip it.
        let a = if a < self.lines.len() && self.tops[a] + self.heights[a] <= from { a + 1 } else { a };
        a.min(b)..b
    }

    /// The line a cursor sits on.
    pub fn line_of(&self, cur: VCursor) -> Option<usize> {
        self.lines.iter().position(|l| match (*l, cur) {
            (VLine::Header(s), VCursor::Header(c)) => s == c,
            (VLine::Row { section, first, count }, VCursor::Item(s, i)) => section == s && i >= first && i < first + count,
            _ => false,
        })
    }

    /// Items in reading order (folded sections' items left out).
    pub fn items(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.lines.iter().flat_map(|l| match *l {
            VLine::Row { section, first, count } => (first..first + count).map(move |i| (section, i)).collect::<Vec<_>>(),
            VLine::Header(_) => Vec::new(),
        })
    }
}

/// A key move (`VListState::nav`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VNav {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

/// One list's state (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VListState {
    pub sections: Vec<VSection>,
    pub folded: BTreeSet<String>,
    pub shape: Option<VShape>,
    pub layout: VLayout,
    pub cursor: Option<VCursor>,
    /// Bumped when what the slots show changed (data, selection).
    pub generation: u64,
    /// The cursor's line wants revealing (a key moved it).
    pub reveal: bool,
    /// The view's height, cq, as last laid out (PageUp/PageDown).
    pub view_h: f32,
}

impl VListState {
    fn relayout(&mut self) {
        let shape = self.shape.unwrap_or(VShape::list(2.0));
        self.layout = layout(&self.sections, &self.folded, &shape);
        // A cursor whose item went away (filtered out, folded) goes to its
        // section's header, or the first line.
        if let Some(c) = self.cursor
            && self.layout.line_of(c).is_none()
        {
            self.cursor = match c {
                VCursor::Item(s, _) if self.layout.line_of(VCursor::Header(s)).is_some() => Some(VCursor::Header(s)),
                _ => self.first(),
            };
        }
        self.generation += 1;
    }

    /// New sections (the data or a filter changed): laid out again, the
    /// slots rebound.
    pub fn set_sections(&mut self, sections: Vec<VSection>) {
        if self.sections != sections {
            self.sections = sections;
            self.relayout();
        }
    }

    pub fn set_shape(&mut self, shape: VShape) {
        if self.shape != Some(shape) {
            self.shape = Some(shape);
            self.relayout();
        }
    }

    /// Something the slots show changed: rebind them all.
    pub fn bump(&mut self) {
        self.generation += 1;
    }

    pub fn is_folded(&self, section: usize) -> bool {
        self.sections.get(section).is_some_and(|s| self.folded.contains(&s.key))
    }

    /// Folds (`fold`) or opens a section.
    pub fn fold(&mut self, section: usize, fold: bool) {
        let Some(key) = self.sections.get(section).map(|s| s.key.clone()) else { return };
        let changed = if fold { self.folded.insert(key) } else { self.folded.remove(&key) };
        if changed {
            if fold && matches!(self.cursor, Some(VCursor::Item(s, _)) if s == section) {
                self.cursor = Some(VCursor::Header(section));
            }
            self.relayout();
        }
    }

    pub fn toggle_fold(&mut self, section: usize) {
        let f = self.is_folded(section);
        self.fold(section, !f);
    }

    fn first(&self) -> Option<VCursor> {
        self.layout.lines.first().map(|l| Self::cursor_at(*l, 0))
    }

    fn cursor_at(l: VLine, col: usize) -> VCursor {
        match l {
            VLine::Header(s) => VCursor::Header(s),
            VLine::Row { section, first, count } => VCursor::Item(section, first + col.min(count - 1)),
        }
    }

    /// The cursor's column in its row (0 on a header).
    fn col(&self) -> usize {
        match self.cursor {
            Some(VCursor::Item(_, i)) => i % self.shape.map_or(1, |s| s.cols.max(1)),
            _ => 0,
        }
    }

    /// Puts the cursor on `c` (a press), revealed; false when it's not shown.
    pub fn select(&mut self, c: VCursor) -> bool {
        if self.layout.line_of(c).is_none() {
            return false;
        }
        if self.cursor != Some(c) {
            self.cursor = Some(c);
            self.bump();
        }
        self.reveal = true;
        true
    }

    /// ←→'s stops in reading order: the items shown, a folded section's
    /// header where its items would be.
    pub fn stops(&self) -> Vec<VCursor> {
        let mut out = Vec::new();
        for l in &self.layout.lines {
            match *l {
                VLine::Header(s) if self.is_folded(s) => out.push(VCursor::Header(s)),
                VLine::Header(_) => {}
                VLine::Row { section, first, count } => out.extend((first..first + count).map(|i| VCursor::Item(section, i))),
            }
        }
        out
    }

    /// A key (module doc). True when it did something.
    pub fn nav(&mut self, k: VNav) -> bool {
        let lines = &self.layout.lines;
        if lines.is_empty() {
            return false;
        }
        let Some(cur) = self.cursor else {
            self.cursor = self.first();
            self.reveal = true;
            self.bump();
            return true;
        };
        let line = self.layout.line_of(cur).unwrap_or(0);
        let col = self.col();
        let page = (self.view_h / self.shape.map_or(2.0, |s| s.row_h + s.gap)).floor().max(1.0) as usize;
        let to_line = |l: usize| Self::cursor_at(lines[l.min(lines.len() - 1)], col);
        let next = match k {
            VNav::Up => (line > 0).then(|| to_line(line - 1)),
            VNav::Down => (line + 1 < lines.len()).then(|| to_line(line + 1)),
            VNav::PageUp => Some(to_line(line.saturating_sub(page))),
            VNav::PageDown => Some(to_line(line + page)),
            VNav::Home => self.first(),
            VNav::End => Some(Self::cursor_at(*lines.last().unwrap(), usize::MAX)),
            VNav::Left | VNav::Right => match cur {
                VCursor::Header(s) => {
                    let fold = k == VNav::Left;
                    if self.is_folded(s) != fold {
                        self.fold(s, fold);
                        return true;
                    }
                    if fold {
                        return false;
                    }
                    // Open already: → steps to its first item.
                    self.layout.line_of(VCursor::Item(s, 0)).map(|_| VCursor::Item(s, 0))
                }
                VCursor::Item(..) => {
                    // Reading order: every tile, and a folded section's
                    // header in its place.
                    let stops = self.stops();
                    let at = stops.iter().position(|&c| c == cur).unwrap_or(0);
                    let to = if k == VNav::Left { at.checked_sub(1) } else { (at + 1 < stops.len()).then_some(at + 1) };
                    to.map(|t| stops[t])
                }
            },
        };
        match next {
            Some(n) if n != cur => {
                self.cursor = Some(n);
                self.reveal = true;
                self.bump();
                true
            }
            _ => false,
        }
    }
}

/// Every list's state by key.
#[derive(Resource, Clone, Debug, Default)]
pub struct VirtualLists(pub BTreeMap<String, VListState>);

impl VirtualLists {
    pub fn get(&self, key: &str) -> Option<&VListState> {
        self.0.get(key)
    }

    pub fn state(&mut self, key: &str) -> &mut VListState {
        self.0.entry(key.to_string()).or_default()
    }
}

/// A built list (on its scroll view): its key, content and slot pool.
#[derive(Component, Clone, Debug)]
pub struct VirtualList {
    pub key: String,
    pub content: Entity,
    pub pool: Vec<Entity>,
}

/// A pooled line slot (module doc): the line it shows (an index into its
/// list's `VLayout::lines`) and the generation it was bound at. The
/// screen binds on `Changed<VSlot>`.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct VSlot {
    pub list: Entity,
    pub line: Option<usize>,
    pub generation: u64,
}

/// A node under a slot that the pointer picks an item by: `col` in the
/// slot's row (`None`: the slot's header). Give it `controls::clickable`.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct VHit {
    pub slot: Entity,
    pub col: Option<usize>,
}

/// What the pointer did on a list (module doc).
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum VListEvent {
    Select { key: String, at: VCursor },
    Activate { key: String, at: VCursor },
    Fold { key: String, section: usize, folded: bool },
}

/// What `Ui::virtual_list` built.
#[derive(Clone, Debug)]
pub struct VListParts {
    pub scroll: ScrollParts,
    pub slots: Vec<Entity>,
}

/// How many slots a view `view_h` cq tall needs: the lines it can show at
/// once, plus two spare.
pub fn pool_size(view_h: f32, shape: &VShape) -> usize {
    (view_h / shape.min_line().max(0.1)).ceil() as usize + 2
}

impl<'a> Ui<'a> {
    /// A recycling list under `parent` (module doc), `view_h` cq tall at
    /// most (sizes the pool), laid out by `shape`. Its state is
    /// `VirtualLists[key]` (sections, cursor, folds); the slots come back
    /// empty for the screen to fill with its line template once.
    pub fn virtual_list(&self, c: &mut Commands, parent: Entity, key: &str, frame: UiStyle, view_h: f32, shape: VShape) -> VListParts {
        let scroll = self.scroll(c, parent, key, frame, UiStyle::column());
        let n = pool_size(view_h, &shape);
        let slots: Vec<Entity> = (0..n)
            .map(|_| {
                c.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, display: Display::None, ..default() },
                    VSlot { list: scroll.view, line: None, generation: 0 },
                    ChildOf(scroll.content),
                ))
                .id()
            })
            .collect();
        c.entity(scroll.view).insert(VirtualList { key: key.to_string(), content: scroll.content, pool: slots.clone() });
        c.entity(scroll.content).insert(Node { width: Val::Percent(100.0), flex_shrink: 0.0, ..default() });
        let _ = shape;
        VListParts { scroll, slots }
    }
}

/// Places the pool over the lines in view (module doc), reveals a moved
/// cursor and keeps the content as tall as the layout.
#[allow(clippy::type_complexity)]
pub fn place_slots(
    units: Res<UiUnits>,
    theme: Res<Theme>,
    reduced: Option<Res<ReducedMotion>>,
    mut lists: ResMut<VirtualLists>,
    mut views: ResMut<ScrollViews>,
    list_q: Query<&VirtualList>,
    mut nodes: Query<&mut Node>,
    mut slots: Query<&mut VSlot>,
) {
    let px = units.cq(1.0);
    let reduced = reduced.is_some_and(|r| r.0);
    let margin = ScrollTuning::from_theme(&theme).reveal_margin;
    for list in &list_q {
        let Some(st) = lists.0.get_mut(&list.key) else { continue };
        let view = views.state(&list.key);
        st.view_h = view.view;
        if std::mem::take(&mut st.reveal)
            && let Some(l) = st.cursor.and_then(|c| st.layout.line_of(c))
        {
            view.reveal(st.layout.tops[l], st.layout.tops[l] + st.layout.heights[l], margin, reduced);
        }
        // The content as tall as the whole layout.
        let total = Val::Px(units.snap(st.layout.total * px));
        if let Ok(mut n) = nodes.get_mut(list.content)
            && n.height != total
        {
            n.height = total;
        }
        // A view not laid out yet shows a pool's worth from the top.
        let (from, h) = (view.inside(), if view.view > 0.0 { view.view } else { list.pool.len() as f32 * 2.0 });
        let range = st.layout.visible(from, from + h);
        let range = range.start..range.end.min(range.start + list.pool.len());
        let n = list.pool.len();
        for (k, &slot) in list.pool.iter().enumerate() {
            // The line this slot holds: the one in range congruent to it.
            let line = range.clone().find(|l| l % n == k);
            if let Ok(mut s) = slots.get_mut(slot)
                && (s.line != line || (line.is_some() && s.generation != st.generation))
            {
                s.line = line;
                s.generation = st.generation;
            }
            let Ok(mut node) = nodes.get_mut(slot) else { continue };
            match line {
                Some(l) => {
                    let top = Val::Px(units.snap(st.layout.tops[l] * px));
                    let height = Val::Px(units.snap(st.layout.heights[l] * px));
                    if node.display != Display::Flex || node.top != top || node.height != height {
                        node.display = Display::Flex;
                        node.top = top;
                        node.height = height;
                    }
                }
                None => {
                    if node.display != Display::None {
                        node.display = Display::None;
                    }
                }
            }
        }
    }
}

/// The last press, for the double-click rule.
#[derive(Default)]
struct LastPress {
    at: Option<(String, VCursor, f64)>,
}

/// Presses on `VHit` nodes: select, a second press activates, a header
/// folds (module doc).
fn hits(
    clock: Res<UiClock>,
    pressed: Query<(&Interaction, &VHit), Changed<Interaction>>,
    slots: Query<&VSlot>,
    list_q: Query<&VirtualList>,
    mut lists: ResMut<VirtualLists>,
    mut last: Local<LastPress>,
    mut out: MessageWriter<VListEvent>,
) {
    for (i, hit) in &pressed {
        if *i != Interaction::Pressed {
            continue;
        }
        let Ok(slot) = slots.get(hit.slot) else { continue };
        let (Some(line), Ok(list)) = (slot.line, list_q.get(slot.list)) else { continue };
        let Some(st) = lists.0.get_mut(&list.key) else { continue };
        let Some(&l) = st.layout.lines.get(line) else { continue };
        let at = match (l, hit.col) {
            (VLine::Header(s), _) => {
                st.toggle_fold(s);
                st.select(VCursor::Header(s));
                out.write(VListEvent::Fold { key: list.key.clone(), section: s, folded: st.is_folded(s) });
                continue;
            }
            (VLine::Row { section, first, count }, Some(c)) if c < count => VCursor::Item(section, first + c),
            _ => continue,
        };
        st.select(at);
        let now = clock.now;
        let double = last.at.as_ref().is_some_and(|(k, c, t)| *k == list.key && *c == at && now - t <= DOUBLE_CLICK_SECS);
        if double {
            last.at = None;
            out.write(VListEvent::Activate { key: list.key.clone(), at });
        } else {
            last.at = Some((list.key.clone(), at, now));
            out.write(VListEvent::Select { key: list.key.clone(), at });
        }
    }
}

/// The list's systems (in the kit's plugin).
pub struct VListPlugin;

impl Plugin for VListPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VirtualLists>()
            .add_message::<VListEvent>()
            .add_systems(PreUpdate, hits.after(UiSystems::Focus))
            .add_systems(PostUpdate, place_slots.after(crate::scroll::scroll_step).in_set(crate::UiKitSystems));
    }
}
