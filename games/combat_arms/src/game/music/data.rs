//! The music's data (data/music.toml): the files the player knows and
//! their lengths, the menu playlist's entries, the cues by their
//! attribute key and each mode's start track. The built-in table is the
//! spec's fallback for what the install keeps in keyed attribute files
//! (music_ambience.md Open question 1); `normalize` is how every path
//! is compared.

use std::{collections::HashMap, sync::OnceLock};

use serde::Deserialize;

/// A music path as compared: lower case, forward slashes, no leading
/// `Game/`.
pub fn normalize(path: &str) -> String {
    let p = path.trim().replace('\\', "/").to_ascii_lowercase();
    p.strip_prefix("game/").map(str::to_string).unwrap_or(p)
}

/// A menu playlist entry.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlaylistEntry {
    pub name: String,
    pub file: String,
}

/// A mode's start track (the room data's `StartBGM` and `LoopBGM`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ModeTrack {
    #[serde(default)]
    pub start: String,
    #[serde(default, rename = "loop")]
    pub looping: bool,
    /// `StartBGM_<map index>` entries: the start track on that map.
    #[serde(default)]
    pub maps: HashMap<String, String>,
}

/// The cues by their attribute key.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Cues {
    pub loading: String,
    pub quarantine_mode: String,
    pub quarantine_end: String,
    pub capture_progress: String,
    pub last_stage: String,
    pub war_cry: String,
    pub eagle_eye: String,
}

/// data/music.toml.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MusicData {
    pub version: String,
    /// Seconds.
    pub menu_fade: f64,
    pub trigger_fade_default: f64,
    pub quarantine_mode_fade: f64,
    pub quarantine_end_window: [f64; 2],
    pub hq_countdown: f64,
    pub playlist_guard: usize,
    pub slider_step: u32,
    pub music_volume_default: u32,
    pub sound_volume_default: u32,
    /// Normalized path to length in seconds.
    #[serde(default)]
    pub files: HashMap<String, f64>,
    #[serde(default)]
    pub playlist: Vec<PlaylistEntry>,
    pub cues: Cues,
    /// By mode key.
    #[serde(default)]
    pub modes: HashMap<String, ModeTrack>,
    /// Interface sound attribute key to its file.
    #[serde(default)]
    pub interface: HashMap<String, String>,
    /// A party match's music (`director`'s party cues).
    #[serde(default)]
    pub party: PartyMusic,
}

/// What a party match (a minigame, Chimera's `minigame::moments`) takes
/// of Combat Arms' music (music_ambience.md Section 10).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct PartyMusic {
    /// The mode whose start track and last-stage music it plays (Arms
    /// Race's for a gun game).
    #[serde(default)]
    pub mode: String,
    /// Its interface sounds' files by attribute key (inferred, Section 7).
    #[serde(default)]
    pub interface: HashMap<String, String>,
    /// The tracks of the playlist played during a party match (files
    /// below `Game/`); empty: the menu playlist's entries.
    #[serde(default)]
    pub playlist: Vec<String>,
    /// The `party_music` cvar's default: 0 off, 1 the start track once
    /// then the playlist, 2 Arms Race's original (the start track
    /// looping).
    #[serde(default = "default_party_music")]
    pub default_music: u8,
}

fn default_party_music() -> u8 {
    1
}

impl MusicData {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut d: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        d.files = d.files.into_iter().map(|(k, v)| (normalize(&k), v)).collect();
        for e in &mut d.playlist {
            e.file = normalize(&e.file);
        }
        for m in d.modes.values_mut() {
            m.start = normalize(&m.start);
            for v in m.maps.values_mut() {
                *v = normalize(v);
            }
        }
        for f in &mut d.party.playlist {
            *f = normalize(f);
        }
        d.party.default_music = d.party.default_music.min(2);
        let c = &mut d.cues;
        for f in [
            &mut c.loading,
            &mut c.quarantine_mode,
            &mut c.quarantine_end,
            &mut c.capture_progress,
            &mut c.last_stage,
            &mut c.war_cry,
            &mut c.eagle_eye,
        ] {
            *f = normalize(f);
        }
        Ok(d)
    }

    /// The built-in table.
    pub fn built_in() -> &'static Self {
        static DATA: OnceLock<MusicData> = OnceLock::new();
        DATA.get_or_init(|| Self::parse(chimera_core::game_data!("data/music.toml").text()).expect("data/music.toml"))
    }

    /// A mode's start track on map `index`: the map's entry, else the
    /// mode's; empty: none. And whether it loops.
    pub fn mode_track(&self, mode: &str, map_index: Option<u32>) -> Option<(&str, bool)> {
        let m = self.modes.get(mode)?;
        let map = map_index
            .and_then(|i| m.maps.get(&i.to_string()))
            .filter(|s| !s.is_empty());
        let file = map.map_or(m.start.as_str(), String::as_str);
        (!file.is_empty()).then_some((file, m.looping))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_table_reads() {
        let d = MusicData::built_in();
        assert_eq!(d.menu_fade, 10.0);
        assert_eq!(d.cues.loading, "bgm/map_loading.mp3");
        assert_eq!(d.files["bgm/modebgm/search&destroy.mp3"], 18.969);
        assert_eq!(
            d.mode_track("TMM", None),
            Some(("bgm/modebgm/search&destroy.mp3", false))
        );
        assert_eq!(d.mode_track("TDM", None), None);
        assert_eq!(normalize("Game\\BGM\\Map_Loading.mp3"), "bgm/map_loading.mp3");
        assert!(d.playlist.len() >= 3);
        assert_eq!(d.party.mode, "VWRM");
        assert_eq!(d.party.default_music, 1, "the playlist by default");
        assert!(d.party.playlist.is_empty(), "the menu playlist's entries");
        assert_eq!(d.party.interface["RoundStartSound"], "UI_SND/ROUNDSTART.WAV");
        assert!(d.interface.is_empty(), "plain Combat Arms' interface sounds stay silent");
    }

    #[test]
    fn a_maps_entry_beats_the_modes() {
        let mut d = MusicData::built_in().clone();
        d.modes
            .get_mut("TMM")
            .unwrap()
            .maps
            .insert("3".into(), "bgm/x.mp3".into());
        assert_eq!(d.mode_track("TMM", Some(3)), Some(("bgm/x.mp3", false)));
        assert_eq!(
            d.mode_track("TMM", Some(4)).unwrap().0,
            "bgm/modebgm/search&destroy.mp3"
        );
    }
}
