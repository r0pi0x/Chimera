//! What the UI asks to hear (docs/plans/active/ugg-ui.md, "UI sound"):
//! `UiCue`, a message naming an event of the app's sound table
//! (`ui_hover`, `ui_select`, `lobby_ready`, `rank_up`, ...), and
//! `UiMusic`, the loop the screen wants under it (the main menu's). The
//! kit only names them; the client shell's `ui_sound` looks each event up
//! in the app's table and plays it on the `ui` bus, and only under a
//! theme that turns UI sound on (`flags.sound.ui`), so a cue sent under
//! any other theme is silent. `SoundTable` is the app's event table
//! (TOML: per event its file, gain, per-play spread, voice limit, on/off,
//! a slider's rate, a graded event's grid, music's loop), read here so
//! the shell needs no TOML reader of its own.

use std::{borrow::Cow, collections::BTreeMap};

use bevy::prelude::*;

/// One UI event to sound (module doc).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct UiCue {
    /// The event's name in the app's sound table.
    pub event: Cow<'static, str>,
    /// A value the event follows, 0 to 1: a slider's position (its rate),
    /// a ladder's progress (which render of a graded event plays).
    pub value: Option<f32>,
}

impl UiCue {
    pub fn new(event: &'static str) -> Self {
        Self { event: Cow::Borrowed(event), value: None }
    }

    /// An event that follows a value, 0 to 1.
    pub fn at(event: &'static str, value: f32) -> Self {
        Self { event: Cow::Borrowed(event), value: Some(value.clamp(0.0, 1.0)) }
    }
}

/// The music loop the screens want now (an event of the app's table
/// with `loop = true`), or none.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct UiMusic(pub Option<Cow<'static, str>>);

/// One event of the table (module doc).
#[derive(Clone, Debug, PartialEq)]
pub struct CueEntry {
    /// The file's name (without `.wav`); `{nn}` for a graded event.
    pub file: String,
    /// A graded event's steps (renders `0..=grid`).
    pub grid: Option<u32>,
    /// dB.
    pub gain: f32,
    /// Per-play spread, plus or minus: cents of pitch, dB of gain.
    pub cents: f32,
    pub db: f32,
    /// At most this many at once.
    pub voices: usize,
    pub on: bool,
    /// The speed follows the cue's value (0.5 to 2.0).
    pub rate_from_value: bool,
    /// Music: loops, on the music bus.
    pub looping: bool,
}

impl CueEntry {
    /// The file a cue of this event plays.
    pub fn file_for(&self, value: Option<f32>) -> String {
        match self.grid {
            Some(n) => {
                let nn = (value.unwrap_or(0.0).clamp(0.0, 1.0) * n as f32).round() as u32;
                self.file.replace("{nn}", &format!("{nn:02}"))
            }
            None => self.file.clone(),
        }
    }
}

/// The event table (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SoundTable {
    pub events: BTreeMap<String, CueEntry>,
    /// dB the `ui` bus ducks under an announcer voice.
    pub duck_db: f32,
}

impl SoundTable {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let doc: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
        let num = |v: Option<&toml::Value>, d: f32| -> f32 {
            v.and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))).map_or(d, |f| f as f32)
        };
        let duck_db = num(doc.get("bus").and_then(|b| b.get("duck_db")), 4.0);
        let mut events = BTreeMap::new();
        for (name, t) in doc.get("event").and_then(|e| e.as_table()).into_iter().flatten() {
            let t = t.as_table().ok_or_else(|| format!("event {name}: not a table"))?;
            let file = t.get("file").and_then(|f| f.as_str()).unwrap_or(name).to_string();
            let grid = t.get("grid").and_then(|g| g.as_integer()).map(|g| g.max(1) as u32);
            if grid.is_some() != file.contains("{nn}") {
                return Err(format!("event {name}: `grid` and a file with {{nn}} go together"));
            }
            let rate = t.get("rate").and_then(|r| r.as_str());
            if rate.is_some_and(|r| r != "value") {
                return Err(format!("event {name}: rate is \"value\" or absent"));
            }
            events.insert(
                name.clone(),
                CueEntry {
                    file,
                    grid,
                    gain: num(t.get("gain"), 0.0),
                    cents: num(t.get("cents"), 0.0),
                    db: num(t.get("db"), 0.0),
                    voices: t.get("voices").and_then(|v| v.as_integer()).map_or(2, |v| v.max(1) as usize),
                    on: t.get("on").and_then(|v| v.as_bool()).unwrap_or(true),
                    rate_from_value: rate.is_some(),
                    looping: t.get("loop").and_then(|v| v.as_bool()).unwrap_or(false),
                },
            );
        }
        Ok(Self { events, duck_db })
    }

    /// Every file the table names.
    pub fn files(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for e in self.events.values() {
            match e.grid {
                Some(n) => out.extend((0..=n).map(|i| e.file.replace("{nn}", &format!("{i:02}")))),
                None => out.push(e.file.clone()),
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = r#"
[bus]
duck_db = 4
[event.rank_up]
file = "rank_p{nn}"
grid = 16
[event.ui_slider]
rate = "value"
"#;

    #[test]
    fn graded_events_pick_the_nearest_render() {
        let t = SoundTable::from_toml(TABLE).unwrap();
        let e = &t.events["rank_up"];
        assert_eq!(e.file_for(Some(0.0)), "rank_p00");
        assert_eq!(e.file_for(Some(0.5)), "rank_p08");
        assert_eq!(e.file_for(Some(1.0)), "rank_p16");
        assert_eq!(t.files().len(), 18);
        assert!(t.events["ui_slider"].rate_from_value);
        assert_eq!(t.duck_db, 4.0);
        assert!(SoundTable::from_toml("[event.a]\ngrid = 3\n").is_err(), "a grid needs {{nn}}");
    }
}
