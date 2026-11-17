//! Guns' sound entries belong to their own game (the UGG playtest of
//! 2026-11-17: the TF2 Scout's pistol fired with HL2:DM's sound).
//!
//! Source games share one namespace of sound script names
//! (`Weapon_Pistol.Single` is HL2's and TF2's, `Weapon_Shotgun.Single`
//! too), and a map's sound table keeps the first entry of a name, so a
//! guest game's gun can play the host's or another guest's sound. A game
//! whose names collide with another game's keeps its guns' entries under
//! its own prefix (TF2: `tf2:`, `maps::own_sound`).
//!
//! - `no_gun_sound_name_is_shared_between_games`: across
//!   `games/*/data/weapons`, a dotted sound name (an entry's, `Weapon_X.Y`)
//!   used by guns of two games fails unless each game's use is prefixed
//!   with its own id.
//! - `tf2_guns_name_their_sounds_under_its_prefix`: every sound entry
//!   TF2's weapon data names carries `tf2:`.
//!
//! The install-side check (each entry resolves to TF2's own waves, in the
//! table a match uses) is `tf2` heavy `guns::every_guns_sound_is_tf2s_own`.

use std::{collections::BTreeMap, fs};

use super::architecture::root;

/// The quoted strings of `text` shaped like a sound script entry
/// (`Name.Part`, an optional `game:` before it), comments skipped.
fn sound_names(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        for (i, part) in line.split('"').enumerate() {
            if i % 2 == 0 {
                continue;
            }
            let bare = part.split_once(':').map_or(part, |(_, b)| b);
            let (head, tail) = match bare.split_once('.') {
                Some(x) => x,
                None => continue,
            };
            let word = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
            if word(head) && word(tail) && head.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                out.push(part.to_string());
            }
        }
    }
    out
}

/// Each game's weapon data: its id (the folder) and every sound-shaped
/// name its weapon files hold.
fn games_sound_names() -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for game in fs::read_dir(root().join("games")).unwrap().flatten() {
        let dir = game.path().join("data/weapons");
        let Ok(files) = fs::read_dir(&dir) else { continue };
        let mut names = Vec::new();
        for f in files.flatten() {
            if f.path().extension().is_some_and(|e| e == "toml") {
                names.extend(sound_names(&fs::read_to_string(f.path()).unwrap()));
            }
        }
        out.insert(game.file_name().to_string_lossy().into_owned(), names);
    }
    out
}

#[test]
fn no_gun_sound_name_is_shared_between_games() {
    let games = games_sound_names();
    let mut users: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for (game, names) in &games {
        for n in names {
            // Under the game's own prefix: nobody else's.
            if n.starts_with(&format!("{game}:")) {
                continue;
            }
            let v = users.entry(n.to_lowercase()).or_default();
            if !v.contains(&game.as_str()) {
                v.push(game);
            }
        }
    }
    let shared: Vec<String> = users.iter().filter(|(_, g)| g.len() > 1).map(|(n, g)| format!("{n}: {}", g.join(", "))).collect();
    assert!(
        shared.is_empty(),
        "sound entries named by guns of more than one game (a map's table keeps one: the other game's gun plays \
         the first one's sound); give the game's entries its own prefix, as TF2's `tf2:` (`maps::own_sound`):\n{}",
        shared.join("\n")
    );
}

#[test]
fn tf2_guns_name_their_sounds_under_its_prefix() {
    let games = games_sound_names();
    let tf2 = &games["tf2"];
    assert!(tf2.len() > 50, "TF2's weapon data holds its sound names");
    let bare: Vec<&String> = tf2.iter().filter(|n| !n.starts_with("tf2:")).collect();
    assert!(bare.is_empty(), "TF2's guns name sounds without `tf2:` (they would play another game's): {bare:?}");
}
