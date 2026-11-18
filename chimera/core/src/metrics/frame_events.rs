//! Engine events by frame, for the frame inspector
//! (`chimera_client::client::frame_inspector`): anything that may cost a
//! frame its time says so here (`note(kind, text)`: a download finished, a
//! model built again, a pipeline compiled), from any thread, and the
//! inspector takes them each frame (`take`): a slow frame's record lists
//! what happened in it. `LogEvents` (a tracing layer, the client's
//! `console::log_layer`) notes every log line at INFO and above too, so
//! whatever logs what it did (late-file rebuilds, kept dressings written,
//! map load steps) is named without a call of its own.
//!
//! Cheap enough for always: a mutex push per event (none most frames), at
//! most `MAX` kept between takes (the rest counted, `Taken::dropped`).

use std::sync::Mutex;

use bevy::log::tracing::{self, Subscriber};
use bevy::log::tracing_subscriber::{Layer, layer::Context};

/// Events kept between two takes; later ones are only counted.
pub const MAX: usize = 256;

/// One event: what kind (`log`, `assets`, `pipelines`, a subsystem's
/// name) and what happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameEvent {
    pub kind: &'static str,
    pub text: String,
}

#[derive(Default)]
struct Pending {
    events: Vec<FrameEvent>,
    dropped: u32,
}

static PENDING: Mutex<Pending> = Mutex::new(Pending { events: Vec::new(), dropped: 0 });

/// Note an event for the frame now running (any thread).
pub fn note(kind: &'static str, text: impl Into<String>) {
    let mut p = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if p.events.len() < MAX {
        p.events.push(FrameEvent { kind, text: text.into() });
    } else {
        p.dropped += 1;
    }
}

/// What `take` hands over.
#[derive(Clone, Debug, Default)]
pub struct Taken {
    pub events: Vec<FrameEvent>,
    /// Events past `MAX`, not kept.
    pub dropped: u32,
}

/// The events noted since the last take.
pub fn take() -> Taken {
    let mut p = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if p.events.is_empty() && p.dropped == 0 {
        return Taken::default();
    }
    Taken { events: std::mem::take(&mut p.events), dropped: std::mem::take(&mut p.dropped) }
}

/// A tracing layer noting every event at INFO and above as a `log` event
/// (but the hitch log's own lines).
pub struct LogEvents;

impl<S: Subscriber> Layer<S> for LogEvents {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        if *event.metadata().level() > tracing::Level::INFO {
            return;
        }
        struct Msg(String);
        impl tracing::field::Visit for Msg {
            fn record_debug(&mut self, f: &tracing::field::Field, v: &dyn std::fmt::Debug) {
                if f.name() == "message" {
                    self.0 = format!("{v:?}");
                }
            }
            fn record_str(&mut self, f: &tracing::field::Field, v: &str) {
                if f.name() == "message" {
                    self.0 = v.to_string();
                }
            }
        }
        let mut m = Msg(String::new());
        event.record(&mut m);
        if m.0.starts_with("hitch:") || m.0.is_empty() {
            return;
        }
        let mut text = m.0;
        if text.len() > 300 {
            let cut = (0..=300).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
            text.truncate(cut);
            text.push_str("...");
        }
        note("log", text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_taken_once_and_capped() {
        let _ = take();
        note("test", "a");
        note("test", String::from("b"));
        let t = take();
        assert_eq!(t.events.iter().map(|e| e.text.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert!(take().events.is_empty());
        for i in 0..MAX + 3 {
            note("test", format!("{i}"));
        }
        let t = take();
        assert_eq!((t.events.len(), t.dropped), (MAX, 3));
    }
}
