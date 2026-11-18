use std::collections::HashMap;

use bevy::{
    prelude::*,
    text::LineBreak,
    ui::{ComputedNode, UiGlobalTransform},
    window::PrimaryWindow,
};

use crate::client::fonts::UiFonts;
use super::*;
use crate::console::ConsoleAppExt;
use crate::map::hud::GameUi;

// ---------------------------------------------------------------------------
// ComboBox: the drop-down list.

/// A combo box's open list: the entry highlighted and the first shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComboList {
    /// Which combo box it belongs to (the owner's number for it).
    pub owner: usize,
    pub len: usize,
    pub highlight: usize,
    pub first: usize,
}

/// What a key or click does to an open list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComboEvent {
    /// Still open.
    Open,
    /// Picked an entry: closed with it.
    Pick(usize),
    /// Closed without a pick.
    Close,
}

/// Keys an open list takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComboKey {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Escape,
}

/// Entries an open list shows at once (VGUI's `ComboBox` default).
pub const COMBO_ROWS: usize = 10;

impl ComboList {
    /// Open on `selected` (scrolled to show it).
    pub fn open(owner: usize, len: usize, selected: usize) -> Self {
        let mut l = Self {
            owner,
            len,
            highlight: selected.min(len.saturating_sub(1)),
            first: 0,
        };
        l.show_highlight();
        l
    }

    pub fn rows(&self) -> usize {
        self.len.min(COMBO_ROWS)
    }

    fn show_highlight(&mut self) {
        let rows = self.rows().max(1);
        if self.highlight < self.first {
            self.first = self.highlight;
        } else if self.highlight >= self.first + rows {
            self.first = self.highlight + 1 - rows;
        }
        self.first = self.first.min(self.len.saturating_sub(rows));
    }

    /// The pointer over entry `i`: highlighted.
    pub fn hover(&mut self, i: usize) {
        if i < self.len {
            self.highlight = i;
        }
    }

    /// Entry `i` highlighted and scrolled to (a typed letter jumps so).
    pub fn select(&mut self, i: usize) {
        if i < self.len {
            self.highlight = i;
            self.show_highlight();
        }
    }

    /// The wheel over the list: scrolled `notches` entries.
    pub fn wheel(&mut self, notches: i32) {
        let max = self.len.saturating_sub(self.rows()) as i32;
        self.first = (self.first as i32 + notches).clamp(0, max) as usize;
    }

    pub fn key(&mut self, key: ComboKey) -> ComboEvent {
        if self.len == 0 {
            return ComboEvent::Close;
        }
        let last = self.len - 1;
        let page = self.rows().max(1);
        self.highlight = match key {
            ComboKey::Up => self.highlight.saturating_sub(1),
            ComboKey::Down => (self.highlight + 1).min(last),
            ComboKey::PageUp => self.highlight.saturating_sub(page),
            ComboKey::PageDown => (self.highlight + page).min(last),
            ComboKey::Home => 0,
            ComboKey::End => last,
            ComboKey::Enter => return ComboEvent::Pick(self.highlight),
            ComboKey::Escape => return ComboEvent::Close,
        };
        self.show_highlight();
        ComboEvent::Open
    }
}

/// A closed combo box stepped (focused: Up/Down, the wheel): the entry
/// `dir` away, clamped as VGUI's (no wrapping).
pub fn combo_step(selected: usize, len: usize, dir: i32) -> usize {
    if len == 0 {
        return 0;
    }
    (selected as i32 + dir).clamp(0, len as i32 - 1) as usize
}

