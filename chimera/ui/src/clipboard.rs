//! The clipboard the kit's text fields copy to and paste from
//! (`Clipboard`, a resource): the system's, through `arboard` (Windows,
//! X11 and Wayland), or a string of its own (`Clipboard::memory`: tests,
//! headless runs). The system clipboard is opened on first use and kept
//! for the app's life: on X11 and Wayland the copied text lives in the
//! process that copied it, and would go with a short-lived handle.
//!
//! `single_line` is what a paste into a one-line field keeps: line breaks
//! gone (a copied address with its newline pastes as the address).
//!
//! Known shortcuts: on X11 and Wayland, text copied here is gone from the
//! clipboard when the app quits unless a clipboard manager took it;
//! the primary selection (middle click) isn't read.

use bevy::prelude::*;

/// The clipboard (module doc). Without the resource, a field uses the
/// system's.
#[derive(Resource, Default)]
pub struct Clipboard {
    inner: Inner,
}

#[derive(Default)]
enum Inner {
    /// The system's, opened at first use (None until then, or when it
    /// can't be).
    #[default]
    System,
    Open(arboard::Clipboard),
    Memory(Option<String>),
}

impl Clipboard {
    /// A clipboard of its own holding `text` (tests, headless runs).
    pub fn memory(text: Option<&str>) -> Self {
        Self { inner: Inner::Memory(text.map(str::to_string)) }
    }

    fn system(&mut self) -> Option<&mut arboard::Clipboard> {
        if matches!(self.inner, Inner::System) {
            match arboard::Clipboard::new() {
                Ok(c) => self.inner = Inner::Open(c),
                Err(e) => {
                    warn!("clipboard: {e}");
                    return None;
                }
            }
        }
        match &mut self.inner {
            Inner::Open(c) => Some(c),
            _ => None,
        }
    }

    /// The clipboard's text, if it holds any.
    pub fn get(&mut self) -> Option<String> {
        if let Inner::Memory(m) = &self.inner {
            return m.clone();
        }
        self.system()?.get_text().ok()
    }

    /// Puts `text` on the clipboard.
    pub fn set(&mut self, text: &str) {
        if let Inner::Memory(m) = &mut self.inner {
            *m = Some(text.to_string());
            return;
        }
        if let Some(c) = self.system()
            && let Err(e) = c.set_text(text.to_string())
        {
            warn!("clipboard: {e}");
        }
    }
}

/// `text` for a one-line field: line breaks dropped, and the spaces
/// around a pasted line (a copied address's trailing newline).
pub fn single_line(text: &str) -> String {
    let t: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    if text.contains(['\n', '\r']) { t.trim().to_string() } else { t }
}
