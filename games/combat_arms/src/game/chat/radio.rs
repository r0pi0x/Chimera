//! The radio's data and rules (specs/combat_arms/radio_chat.md R1, R3,
//! R5): the three menus read from one attribute document (`RadioMsgSound`
//! in the attributes archive), what picking an entry sends, and what a
//! receiver makes of a call (the line, the voice file, how it plays).
//!
//! Without the install the menus are ours (`data/radio.txt`, in the
//! attribute document's own syntax: texts of ours, no voice files, so the
//! built-in radio is silent).

use lithtech_formats::attributes::Document;

/// The report, answer and request menus' class numbers (fixed by the
/// client).
pub const REPORT: u8 = 0;
pub const ANSWER: u8 = 1;
pub const REQUEST: u8 = 2;

/// Entries shown (keys 1 to 9) per menu.
pub const ENTRIES_SHOWN: usize = 9;
/// No radio menu this long after a radio send, ms.
pub const COOLDOWN_MS: f64 = 2000.0;
/// The default outer radius of a positional voice line, u, and the inner
/// radius's share of it.
pub const DEFAULT_RADIUS: f32 = 1000.0;
pub const INNER_RATIO: f32 = 0.07;
/// The attribute file in the attributes archive.
pub const FILE: &str = "attributes/radiomsgsound.txt";

/// One menu entry.
#[derive(Clone, Debug, PartialEq)]
pub struct RadioEntry {
    /// 1 (the default): everyone hears it; 0: the sender's team only.
    pub send_to_enemy: bool,
    pub list_text: String,
    /// Voice files per template, by template index.
    pub sounds: Vec<Vec<String>>,
    pub chat: Vec<String>,
    /// 1 (the default): positional; 0: a plain interface sound.
    pub sound_3d: bool,
    pub radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RadioClass {
    pub name: String,
    pub entries: Vec<RadioEntry>,
}

/// The radio file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RadioData {
    /// Voice sets; an index here is what travels.
    pub templates: Vec<String>,
    pub classes: Vec<RadioClass>,
}

impl RadioData {
    /// Read the document (R1): templates until one is missing, classes
    /// until one is missing, entries `N0`, `N1`, ... for class `N`.
    pub fn read(doc: &Document) -> Self {
        let lossy = |b: Vec<u8>| b.iter().map(|&c| c as char).collect::<String>();
        let templates: Vec<String> = doc
            .section("SoundTemplate")
            .map(|s| s.string_list("Template").into_iter().map(&lossy).collect())
            .unwrap_or_default();
        let class_names: Vec<String> = doc
            .section("RadioClass")
            .map(|s| s.string_list("RadioClass").into_iter().map(|b| lossy(b).trim().to_string()).collect())
            .unwrap_or_default();
        let classes = class_names
            .into_iter()
            .map(|name| {
                let entries = doc
                    .records(&name)
                    .into_iter()
                    .map(|s| RadioEntry {
                        send_to_enemy: s.int("SendToEnemy", 1) != 0,
                        list_text: s.string("ListText").map(|b| lossy(b.to_vec())).unwrap_or_default(),
                        sounds: templates
                            .iter()
                            .map(|t| s.string_list(&format!("{t}_Snd")).into_iter().map(&lossy).collect())
                            .collect(),
                        chat: s.string_list("Chat").into_iter().map(&lossy).collect(),
                        sound_3d: s.int("3Dsound", 1) != 0,
                        radius: s.number("3DsoundRadius", DEFAULT_RADIUS as f64) as f32,
                    })
                    .collect();
                RadioClass { name, entries }
            })
            .collect();
        Self { templates, classes }
    }

    /// Parse a document's text.
    pub fn parse(text: &str) -> Result<Self, String> {
        Document::parse(text.as_bytes()).map(|d| Self::read(&d)).map_err(|e| e.to_string())
    }

    /// Ours (`data/radio.txt`).
    pub fn builtin() -> Self {
        Self::parse(include_str!("../../../data/radio.txt")).unwrap_or_else(|e| panic!("Combat Arms radio data: {e}"))
    }

    /// The install's radio file when it can be read, else ours.
    pub fn load(data: &crate::game::data::CaDataInner) -> Self {
        if !data.builtin_only()
            && let Some(mount) = data.mount()
            && let Ok(bytes) = mount.read(FILE)
        {
            match Document::parse(&bytes) {
                Ok(d) => {
                    let data = Self::read(&d);
                    if !data.classes.is_empty() {
                        return data;
                    }
                }
                Err(e) => bevy::log::warn!("combat_arms radio: {FILE}: {e}; using the built-in menus"),
            }
        }
        Self::builtin()
    }

    /// The position of a voice set's name (case-insensitive).
    pub fn template_index(&self, name: &str) -> Option<usize> {
        self.templates.iter().position(|t| t.eq_ignore_ascii_case(name))
    }

    pub fn entry(&self, class: u8, entry: u8) -> Option<&RadioEntry> {
        self.classes.get(class as usize)?.entries.get(entry as usize)
    }

    /// A class's menu: its title and the entries shown (at most nine).
    pub fn menu(&self, class: u8) -> Option<(&str, &[RadioEntry])> {
        let c = self.classes.get(class as usize)?;
        Some((&c.name, &c.entries[..c.entries.len().min(ENTRIES_SHOWN)]))
    }
}

/// The wire form of a radio message from a sender's client (R3 step 5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadioSend {
    /// The sender's position, engine space (m).
    pub at: [f32; 3],
    pub class: u8,
    pub entry: u8,
    /// The drawn line.
    pub r: u8,
    pub to_all: bool,
    /// The voice set's index; 255: none.
    pub t: u8,
}

/// What picking entry `index` (0-based) of `class` sends (R3), or None
/// when the menu has no such entry. `voice` is the sender's voice set
/// name, if it has one; `previous` is the index kept from before (0 at
/// the start), updated. `draw(n)` is the random line in `0..n`.
pub fn pick(
    data: &RadioData,
    class: u8,
    index: usize,
    voice: Option<&str>,
    previous: &mut i32,
    at: [f32; 3],
    draw: impl FnOnce(usize) -> usize,
) -> Option<RadioSend> {
    let entry = data.classes.get(class as usize)?.entries.get(index)?;
    if let Some(name) = voice {
        *previous = data.template_index(name).map_or(-1, |i| i as i32);
    }
    let t = *previous;
    let n = usize::try_from(t).ok().and_then(|t| entry.sounds.get(t)).map_or(0, Vec::len);
    let r = if n == 0 { 0 } else { draw(n) % n };
    Some(RadioSend {
        at,
        class,
        entry: index as u8,
        r: r as u8,
        to_all: entry.send_to_enemy,
        t: if t < 0 { 255 } else { t as u8 },
    })
}

/// How a receiver plays a call (R5 step 3).
#[derive(Clone, Debug, PartialEq)]
pub struct RadioSound {
    pub file: String,
    /// Some: a 3D sound there (engine space) with this outer radius; None:
    /// a plain interface sound at full volume.
    pub at: Option<([f32; 3], f32)>,
}

/// The line's text part (`Chat r`, empty past the list) and the sound a
/// received call plays, if any.
pub fn receive(data: &RadioData, class: u8, entry: u8, r: u8, t: u8, at: [f32; 3]) -> (String, Option<RadioSound>) {
    let Some(e) = data.entry(class, entry) else {
        return (String::new(), None);
    };
    let r = r as usize;
    let chat = e.chat.get(r).cloned().unwrap_or_default();
    // A sound only when the line index is also inside the chat lines
    // (Quirk 3).
    let sound = if r >= e.chat.len() {
        None
    } else {
        e.sounds.get(t as usize).filter(|files| !files.is_empty()).map(|files| RadioSound {
            file: files.get(r).unwrap_or(&files[0]).clone(),
            at: e.sound_3d.then_some((at, e.radius)),
        })
    };
    (chat, sound)
}
