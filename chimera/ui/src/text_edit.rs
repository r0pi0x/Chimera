//! A one-line text field's editing, for every screen with one (the
//! party's Signal fields, the kit's picker's search, the Sandbox's search):
//! `Caret` (the caret and the other end of the selection, char indices),
//! `Edit` (an edit), `TextKey` and `text_key` (what a key does: an edit, or
//! the clipboard's copy, cut or paste). Pure data; the screen owns the text
//! and calls `Caret::key`.

use bevy::prelude::*;

/// A text entry's caret and the other end of its selection (char
/// indices; equal: nothing selected).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Caret {
    pub at: usize,
    pub anchor: usize,
}

/// An edit to a text entry.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// Typed or pasted text (control characters dropped), over the
    /// selection.
    Insert(String),
    Backspace,
    Delete,
    /// The caret a char left or right (`true`: extend the selection).
    Left(bool),
    Right(bool),
    Home(bool),
    End(bool),
    SelectAll,
    /// The caret to a char index (a click; `true`: extend, a drag).
    To(usize, bool),
}

impl Caret {
    /// The caret at the end of `text` (a click into an entry with nothing
    /// to place it by).
    pub fn end_of(text: &str) -> Self {
        let n = text.chars().count();
        Self { at: n, anchor: n }
    }

    /// The selected chars' range.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.at.min(self.anchor)..self.at.max(self.anchor)
    }

    /// As the kit's text field takes it (byte offsets into `text`).
    pub fn cursor(&self, text: &str) -> crate::controls::TextCursor {
        crate::controls::TextCursor { at: byte(text, self.at), anchor: byte(text, self.anchor) }
    }

    pub fn selected<'a>(&self, text: &'a str) -> &'a str {
        let r = self.range();
        &text[byte(text, r.start)..byte(text, r.end)]
    }

    /// Apply an edit to `text`, at most `max` bytes long. Returns whether
    /// the text changed.
    pub fn edit(&mut self, text: &mut String, edit: Edit, max: usize) -> bool {
        let n = text.chars().count();
        self.at = self.at.min(n);
        self.anchor = self.anchor.min(n);
        let r = self.range();
        let moved = |c: &mut Self, to: usize, extend: bool| {
            c.at = to;
            if !extend {
                c.anchor = to;
            }
        };
        match edit {
            Edit::Insert(s) => {
                let s: String = s.chars().filter(|c| !c.is_control()).collect();
                text.replace_range(byte(text, r.start)..byte(text, r.end), "");
                let mut fitted = String::new();
                for c in s.chars() {
                    if text.len() + fitted.len() + c.len_utf8() > max {
                        break;
                    }
                    fitted.push(c);
                }
                let at = byte(text, r.start);
                text.insert_str(at, &fitted);
                let to = r.start + fitted.chars().count();
                moved(self, to, false);
                return !fitted.is_empty() || !r.is_empty();
            }
            Edit::Backspace | Edit::Delete => {
                let cut = if !r.is_empty() {
                    r
                } else if edit == Edit::Backspace && self.at > 0 {
                    self.at - 1..self.at
                } else if edit == Edit::Delete && self.at < n {
                    self.at..self.at + 1
                } else {
                    return false;
                };
                text.replace_range(byte(text, cut.start)..byte(text, cut.end), "");
                moved(self, cut.start, false);
                return true;
            }
            Edit::Left(extend) => {
                let to = if !extend && !r.is_empty() {
                    r.start
                } else {
                    self.at.saturating_sub(1)
                };
                moved(self, to, extend);
            }
            Edit::Right(extend) => {
                let to = if !extend && !r.is_empty() {
                    r.end
                } else {
                    (self.at + 1).min(n)
                };
                moved(self, to, extend);
            }
            Edit::Home(extend) => moved(self, 0, extend),
            Edit::End(extend) => moved(self, n, extend),
            Edit::SelectAll => {
                self.anchor = 0;
                self.at = n;
            }
            Edit::To(i, extend) => moved(self, i.min(n), extend),
        }
        false
    }
}

/// A key in a one-line text field (`text_key`): an edit, or the
/// clipboard's copy, cut or paste.
#[derive(Clone, Debug, PartialEq)]
pub enum TextKey {
    Edit(Edit),
    Copy,
    Cut,
    Paste,
}

/// What a key does in a one-line text field (the party's Signal fields,
/// the kit's `text_field`): Ctrl+V and Shift+Insert paste, Ctrl+C and
/// Ctrl+Insert copy, Ctrl+X and Shift+Delete cut, Ctrl+A selects all;
/// the arrows, Home and End move (Shift extends the selection); typed
/// characters insert. None: not the field's (Enter, Tab, Esc).
pub fn text_key(logical: &bevy::input::keyboard::Key, code: KeyCode, ctrl: bool, shift: bool) -> Option<TextKey> {
    use bevy::input::keyboard::Key;
    Some(match (logical, code) {
        (_, KeyCode::KeyV) if ctrl => TextKey::Paste,
        (_, KeyCode::Insert) if shift => TextKey::Paste,
        (_, KeyCode::KeyC) if ctrl => TextKey::Copy,
        (_, KeyCode::Insert) if ctrl => TextKey::Copy,
        (_, KeyCode::KeyX) if ctrl => TextKey::Cut,
        (_, KeyCode::Delete) if shift => TextKey::Cut,
        (_, KeyCode::KeyA) if ctrl => TextKey::Edit(Edit::SelectAll),
        (_, KeyCode::ArrowLeft) => TextKey::Edit(Edit::Left(shift)),
        (_, KeyCode::ArrowRight) => TextKey::Edit(Edit::Right(shift)),
        (_, KeyCode::Home) => TextKey::Edit(Edit::Home(shift)),
        (_, KeyCode::End) => TextKey::Edit(Edit::End(shift)),
        (_, KeyCode::Delete) => TextKey::Edit(Edit::Delete),
        (Key::Backspace, _) => TextKey::Edit(Edit::Backspace),
        (Key::Space, _) => TextKey::Edit(Edit::Insert(" ".into())),
        (Key::Character(s), _) if !ctrl => TextKey::Edit(Edit::Insert(s.to_string())),
        _ => return None,
    })
}

impl Caret {
    /// Applies `key` to `text` (at most `max` bytes) with `clipboard`: a
    /// paste is one line (`crate::clipboard::single_line`); a
    /// `masked` field (a password) takes a paste but never gives its text
    /// (copy and cut do nothing). Whether the text changed.
    pub fn key(&mut self, text: &mut String, key: TextKey, clipboard: &mut crate::clipboard::Clipboard, max: usize, masked: bool) -> bool {
        match key {
            TextKey::Edit(e) => self.edit(text, e, max),
            TextKey::Paste => match clipboard.get() {
                Some(t) => self.edit(text, Edit::Insert(crate::clipboard::single_line(&t)), max),
                None => false,
            },
            TextKey::Copy | TextKey::Cut if masked || self.range().is_empty() => false,
            TextKey::Copy => {
                clipboard.set(self.selected(text));
                false
            }
            TextKey::Cut => {
                clipboard.set(self.selected(text));
                self.edit(text, Edit::Delete, max)
            }
        }
    }
}

/// The byte offset of char `i` in `s` (its end past the last).
fn byte(s: &str, i: usize) -> usize {
    s.char_indices().nth(i).map_or(s.len(), |(b, _)| b)
}
