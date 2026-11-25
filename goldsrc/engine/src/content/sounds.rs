//! A GoldSrc game's sounds as Chimera's sound entries (specs/goldsrc/
//! mount.md: loose `.wav` files under `sound/`): GoldSrc plays waves by
//! path, so an entry's name is its wave's path under `sound/`
//! (`weapons/ak47-1.wav`), or several joined by `;` (one picked at
//! random each time it plays: a gun's `-1` and `-2` shots). Each wave is
//! read once into the map's clips; an entry with no wave found is left
//! out (the audio then says what it lacks).
//!
//! Every entry plays at full volume and pitch, falling off as GoldSrc's
//! normal attenuation (0.8: silent past 1250 units).

use crate::{
    map::sound::{Interval, MapSoundEntry, MapSounds, SoundLevel, decode_clip},
    mount::Mount,
};

/// GoldSrc's normal attenuation (`ATTN_NORM`).
pub const NORMAL: f32 = 0.8;

/// The waves an entry name lists (module doc).
pub fn waves(entry: &str) -> impl Iterator<Item = &str> {
    entry.split(';').map(str::trim).filter(|w| !w.is_empty())
}

/// Entry `name` into `sounds` with its waves read from `mount`, unless it
/// is there already; false when none of its waves was found.
pub fn add_entry(mount: &Mount, name: &str, sounds: &mut MapSounds) -> bool {
    let key = name.trim().to_ascii_lowercase();
    if key.is_empty() {
        return false;
    }
    if sounds.entries.contains_key(&key) {
        return true;
    }
    let mut found = Vec::new();
    for w in waves(&key) {
        let path = format!("sound/{w}");
        let Ok(bytes) = mount.read(&path) else { continue };
        let Ok(clip) = decode_clip(&bytes) else { continue };
        sounds.clips.push(clip);
        found.push(sounds.clips.len() - 1);
    }
    if found.is_empty() {
        return false;
    }
    sounds.entries.insert(
        key,
        MapSoundEntry {
            waves: found,
            volume: Interval::fixed(1.0),
            pitch: Interval::fixed(100.0),
            level: SoundLevel::Attenuation(NORMAL),
            channel: 0,
            dry: false,
            hearing: None,
        },
    );
    true
}
