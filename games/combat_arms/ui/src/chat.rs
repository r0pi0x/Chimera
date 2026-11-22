//! The match chat's queue (specs/combat_arms/ui_system.md H, "Chat
//! panel"), as plain data: messages are clones of the line template
//! stacked down from its position, each at the previous one's top + its
//! height + 4 ref px; after adding, the oldest go while the stack is
//! taller than the queue window (180 ref); every frame, for each message
//! older than its life, the *oldest* message goes (a quirk). Heights are
//! the template's wrapped text measured at the HUD's scale, truncated to
//! ref px, so the capacity depends on the resolution.
//!
//! The match's chat panel (`talk`) shows the lines `game::chat` keeps
//! through this queue (radio_chat.md).

use std::collections::VecDeque;

use crate::client::canvas::Scale;

use super::layout::text::{self, Font};

/// The queue window's top-left and size, ref px.
pub const QUEUE_AT: (i32, i32) = (0, 520);
pub const QUEUE_SIZE: (i32, i32) = (400, 180);
/// The line template's position and wrap width, ref px.
pub const TEMPLATE_AT: (i32, i32) = (6, 525);
pub const TEMPLATE_WRAP: i32 = 380;
/// Between queued messages, ref px.
pub const GAP: i32 = 4;
/// Message lives, ms: player, team, whisper and clan lines; system lines.
pub const PLAYER_LIFE: f64 = 10000.0;
pub const SYSTEM_LIVES: [f64; 2] = [5000.0, 15000.0];
/// The chat input's limit, characters.
pub const INPUT_MAX: usize = 119;

/// Message colours (the HUD layout attributes'), `game::chat`'s.
pub mod colours {
    pub use crate::game::chat::show::colours::*;
}

/// A queued message.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Message {
    pub text: String,
    pub born: f64,
    pub life: f64,
    /// Ref px.
    pub height: i32,
    /// The text's colour, and the sender's name (byte range of the text)
    /// with its own.
    pub colour: super::layout::Rgba,
    pub name: Option<(usize, usize, super::layout::Rgba)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatQueue {
    pub messages: VecDeque<Message>,
}

/// A message's height in ref px: the template's wrapped text measured at
/// scale `s`.
pub fn height(text_: &str, font: &Font, s: Scale) -> i32 {
    text::measure(text_, font, TEMPLATE_WRAP, s).reference.y
}

impl ChatQueue {
    /// The stack's height: heights and gaps, without the last gap.
    pub fn total(&self) -> i32 {
        let n = self.messages.len() as i32;
        self.messages.iter().map(|m| m.height).sum::<i32>() + GAP * (n - 1).max(0)
    }

    /// Add a message, then drop the oldest while the stack is too tall.
    pub fn push(&mut self, m: Message) {
        self.messages.push_back(m);
        while self.total() > QUEUE_SIZE.1 && !self.messages.is_empty() {
            self.messages.pop_front();
        }
    }

    /// One frame at `now`: as many of the oldest go as are past their
    /// life.
    pub fn frame(&mut self, now: f64) {
        let expired = self.messages.iter().filter(|m| now - m.born > m.life).count();
        for _ in 0..expired {
            self.messages.pop_front();
        }
    }

    /// Each message's top, ref px.
    pub fn tops(&self) -> Vec<i32> {
        let mut y = TEMPLATE_AT.1;
        self.messages
            .iter()
            .map(|m| {
                let top = y;
                y += m.height + GAP;
                top
            })
            .collect()
    }
}
