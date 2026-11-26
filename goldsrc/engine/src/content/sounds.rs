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
//!
//! The step sets (`add_steps`, specs/cs16/footsteps.md): a surface per
//! kind of `brushes::surface_kind` and the movement's `ladder` and `wade`,
//! whose left and right feet are entries listing the files each may pick
//! (the right foot 1 or 3, the left 2 or 4; the tile set's `pl_tile5` one
//! step in five), and the water's own (`SPLASH`, `BUBBLE`, the gasps).
//!
//! A map's own sounds (`add_map`): the waves its entities may play
//! (`logic::world::World::sound_paths`: doors, buttons, trains, fans,
//! `ambient_generic`, breakables, `+use`), each an entry by its path, as
//! the logic plays them.
//!
//! Known shortcuts:
//! - A sentence (`!NAME`: `sound/sentences.txt`) is not read: no spec
//!   covers that file, and the stock maps name none.

use crate::{
    map::sound::{Interval, MapSoundEntry, MapSounds, MapSurface, SoundLevel, decode_clip},
    mount::Mount,
};

/// A step set: the surface kind (`brushes::surface_kind`, plus the
/// movement's `ladder` and `wade`), its `materials.txt` letter (the
/// movement's volume reads it), and its four files' stem in
/// `sound/player/` (`pl_step` -> `pl_step1.wav`...).
type StepSet = (&'static str, char, &'static str);

/// CS 1.6's step sets by surface kind (specs/cs16/footsteps.md "The
/// surface type", "Playing a step"): wood, computer, glass and grass step
/// as concrete, as does flesh; slosh is the `S` type's set and the
/// movement's feet-in-water one.
const STEPS: &[StepSet] = &[
    ("concrete", 'C', "pl_step"),
    ("wood", 'W', "pl_step"),
    ("computer", 'P', "pl_step"),
    ("glass", 'Y', "pl_step"),
    ("grass", 'X', "pl_step"),
    ("metal", 'M', "pl_metal"),
    ("dirt", 'D', "pl_dirt"),
    ("metalvent", 'V', "pl_duct"),
    ("metalgrate", 'G', "pl_grate"),
    ("tile", 'T', "pl_tile"),
    ("slipperyslime", 'S', "pl_slosh"),
    ("snow", 'N', "pl_snow"),
    ("ladder", 'C', "pl_ladder"),
    ("wade", 'C', "pl_wade"),
];

/// The entries of a step set's feet: the right foot picks file 1 or 3,
/// the left 2 or 4, at even odds; the tile set plays `pl_tile5` one step
/// in five (the file listed once among four listings of the others).
pub fn step_entries(kind: &str, stem: &str) -> (String, String) {
    let file = |n: u32| format!("player/{stem}{n}.wav");
    let foot = |a: u32, b: u32| {
        if kind == "tile" {
            let (a, b, five) = (file(a), file(b), "player/pl_tile5.wav".to_string());
            format!("{a};{a};{b};{b};{five}")
        } else {
            format!("{};{}", file(a), file(b))
        }
    };
    (foot(2, 4), foot(1, 3))
}

/// The water's sounds (specs/cs16/water.md): a splash or swim stroke (one
/// of `pl_wade1`-`4`), a bubble (one of `pl_swim1`-`4`) and the two gasps.
pub const SPLASH: &str = "player/pl_wade1.wav;player/pl_wade2.wav;player/pl_wade3.wav;player/pl_wade4.wav";
pub const BUBBLE: &str = "player/pl_swim1.wav;player/pl_swim2.wav;player/pl_swim3.wav;player/pl_swim4.wav";
pub const GASP_OUT: &str = "player/pl_wade1.wav";
pub const GASP_SHORT: &str = "player/pl_wade2.wav";

/// The step surfaces (`MapSounds::surfaces`) of the sets above, with the
/// entries each foot plays.
pub fn step_surfaces() -> Vec<(String, MapSurface)> {
    STEPS
        .iter()
        .map(|&(kind, letter, stem)| {
            let (left, right) = step_entries(kind, stem);
            (
                kind.to_string(),
                MapSurface {
                    step_left: Some(left),
                    step_right: Some(right),
                    game_material: letter,
                    ..MapSurface::default()
                },
            )
        })
        .collect()
}

/// The step surfaces and their waves into `sounds`; the surfaces whose
/// waves are not in the install are left out (a warning each).
pub fn add_steps(mount: &Mount, sounds: &mut MapSounds, warnings: &mut Vec<String>) {
    for (name, surface) in step_surfaces() {
        let both = [surface.step_left.clone(), surface.step_right.clone()];
        if both.iter().flatten().all(|e| add_entry(mount, e, sounds)) {
            sounds.surfaces.insert(name, surface);
        } else {
            warnings.push(format!("step sounds {name}: not in the install"));
        }
    }
    for entry in [SPLASH, BUBBLE, GASP_OUT, GASP_SHORT] {
        if !add_entry(mount, entry, sounds) {
            warnings.push(format!("water sound {entry}: not in the install"));
        }
    }
}

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

/// The sounds a map's entities (its `logic` at spawn) may play into `sounds` (module doc); a wave
/// not found is a warning.
pub fn add_map(mount: &Mount, logic: &crate::logic::world::World, sounds: &mut MapSounds, warnings: &mut Vec<String>) {
    for path in logic.sound_paths() {
        if path.starts_with('!') {
            continue;
        }
        if !add_entry(mount, &path, sounds) {
            warnings.push(format!("sound {path}: not in the install"));
        }
    }
}
