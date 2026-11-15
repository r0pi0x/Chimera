//! The music's state and rules (music_ambience.md 2-4, 8; section
//! numbers below are the spec's): the player (Chimera's `MusicPlayer`),
//! the menu playlist and its per-frame update, the cues each moment of a
//! match starts, the interface sounds they name and the order of events
//! at phase changes. Pure: every method takes the wall clock in
//! milliseconds, so the tests step a day in a line; `observe` turns the
//! match's facts into the moments.
//!
//! A party match (a minigame: Chimera's `minigame::moments` in
//! `MusicFacts::party`) gets Section 10's cues instead, when Combat Arms
//! plays its music (`MinigameMoments::music`; else the director is
//! silent): the loading track going in, at the start of play (the match
//! live) `RoundStartSound` and the party mode's start track if the local
//! player is alive (data/music.toml `[party]`: Arms Race's; dead then, no
//! music that match), `last_stage_bgm` looping while anyone is on the
//! ladder's last rung (and the start track again from its start once
//! nobody is, as the mode loops), `RespawnCountSound` at each whole
//! second of the respawn countdown, and the music stopped and paused on
//! the match-end screen. The rung-up and last-rung sounds are the cue
//! pack's, the win and lose sounds the match-end screen's.
//!
//! The party's own music is ours, not the original's, and is set by
//! `CaMusic::party_music` (the `party_music` cvar, default from
//! data/music.toml `[party]`): 1 (default) plays the start track once,
//! then a shuffled playlist (`party_next`: each playable track once
//! before any repeats, cross-fading over the menu fade as the menu's
//! near-end rule), the last-stage music taking over while anyone is on
//! the last rung and the playlist (not the start track) resuming after
//! it; 2 is the original's looping start track; 0 no party music. Only a
//! party match is affected: Combat Arms' own modes keep the parity above.

use std::collections::HashMap;

use bevy::prelude::*;

use super::super::rules::{
    self, Teams,
    state::{ALPHA, BRAVO},
};
use super::{
    data::{MusicData, normalize},
    playlist::Playlist,
};
use crate::music::{MusicPlayer, Stream};
use chimera_sim::minigame::moments::{MinigameMoments, MomentPhase};

/// Which screen the client shows (the playlist runs on the menu and
/// result screens; a match has cues only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    /// Lobby screens: the room list, room, shop and every other menu (and
    /// the title, where there is no map).
    #[default]
    Menu,
    /// Joining a server: the loading screen.
    Loading,
    /// In a match.
    Match,
    /// The result screen after a match.
    Result,
}

/// The music files the client can open and their lengths (ms; None until
/// known). A file may be known but not here yet (`pending`: served by the
/// asset store, its track's set still to come, `ui::music`'s streaming):
/// the party playlist moves on only to tracks that are here.
#[derive(Resource, Clone, Debug, Default)]
pub struct MusicFiles {
    files: HashMap<String, Option<f64>>,
    pending: std::collections::BTreeSet<String>,
}

impl MusicFiles {
    /// Every file the built-in table lists, with its length: what the
    /// tests use, and an install that holds them all.
    pub fn from_data(d: &MusicData) -> Self {
        Self {
            files: d.files.iter().map(|(k, v)| (k.clone(), Some(v * 1000.0))).collect(),
            pending: Default::default(),
        }
    }

    pub fn insert(&mut self, file: &str, length_ms: Option<f64>) {
        self.files.insert(normalize(file), length_ms);
    }

    pub fn remove(&mut self, file: &str) {
        self.files.remove(&normalize(file));
    }

    pub fn exists(&self, file: &str) -> bool {
        self.files.contains_key(&normalize(file))
    }

    /// Mark `file` as not here yet (true) or here (false).
    pub fn set_pending(&mut self, file: &str, pending: bool) {
        if pending {
            self.pending.insert(normalize(file));
        } else {
            self.pending.remove(&normalize(file));
        }
    }

    /// Known and here (not pending).
    pub fn ready(&self, file: &str) -> bool {
        self.exists(file) && !self.pending.contains(&normalize(file))
    }

    /// The files known but not here yet.
    pub fn pending(&self) -> impl Iterator<Item = &String> {
        self.pending.iter()
    }

    pub fn length_ms(&self, file: &str) -> Option<f64> {
        self.files.get(&normalize(file)).copied().flatten()
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// The music master (`m`, 8) applied: a stream as the mixer plays it.
#[derive(Clone, Debug, PartialEq)]
pub struct Mixed {
    pub file: String,
    pub looping: bool,
    pub started: f64,
    /// Music master times the slot's gain.
    pub gain: f32,
}

/// The client's music.
#[derive(Resource, Clone, Debug)]
pub struct CaMusic {
    pub data: &'static MusicData,
    pub player: MusicPlayer,
    pub playlist: Playlist,
    pub files: MusicFiles,
    /// The menu layer's "current music" name.
    pub current: String,
    /// The result screen silences the playlist.
    pub paused: bool,
    pub screen: Screen,
    /// Interface sounds started since the bridge last took them (attribute
    /// keys: `RoundStartSound`, ...).
    pub sounds: Vec<String>,
    /// Every interface sound ever started, for the console and tests.
    pub heard: Vec<String>,
    /// The first real round start of this map was seen (its mode music
    /// played or not: it does not start later).
    mode_music_done: bool,
    /// Quarantine's end cue played this round.
    qrt_end_played: bool,
    /// Seize and Secure's cue is on.
    capture_on: bool,
    /// Arms Race: a player is on the last stage.
    last_stage_on: bool,
    /// The mode under way, for the cues that restart its music.
    mode: String,
    rng: u64,
    /// A party match plays (`observe_party`), not a Combat Arms mode.
    party: bool,
    /// The `party_music` cvar: 0 off, 1 the start track once then the
    /// playlist, 2 the original's looping start track.
    pub party_music: u8,
    /// The party playlist runs (the start track played; not ended by the
    /// match or a last stage).
    playlist_on: bool,
    /// The party playlist's tracks not yet played this round of it.
    bag: Vec<String>,
    /// The party playlist's track last started.
    party_last: String,
}

impl Default for CaMusic {
    fn default() -> Self {
        Self::new(MusicData::built_in())
    }
}

impl CaMusic {
    pub fn new(data: &'static MusicData) -> Self {
        Self {
            data,
            player: MusicPlayer::default(),
            playlist: Playlist::new(
                &data.version,
                data.playlist.iter().map(|e| (e.name.clone(), e.file.clone())),
            ),
            files: MusicFiles::default(),
            current: String::new(),
            paused: false,
            screen: Screen::Menu,
            sounds: Vec::new(),
            heard: Vec::new(),
            mode_music_done: false,
            qrt_end_played: false,
            capture_on: false,
            last_stage_on: false,
            mode: String::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
            party: false,
            party_music: data.party.default_music,
            playlist_on: false,
            bag: Vec::new(),
            party_last: String::new(),
        }
    }

    /// With every file of the table present (tests, an install with the
    /// whole folder).
    pub fn with_table_files(mut self) -> Self {
        self.files = MusicFiles::from_data(self.data);
        self
    }

    fn opens(&self, file: &str) -> bool {
        self.files.exists(file)
    }

    // --- The player through the menu layer (3).

    /// A start through the menu layer (the playlist, loading, mode and cue
    /// starts): the name already current is ignored; fade in seconds.
    /// (The seasonal substitution of `clientevent.txt` isn't read.)
    pub fn start(&mut self, file: &str, looping: bool, fade: f64, now: f64) {
        if file.is_empty() || self.current.eq_ignore_ascii_case(file) {
            return;
        }
        self.current = file.to_string();
        let opens = self.opens(file);
        self.player.start(file, looping, fade * 1000.0, now, opens);
    }

    /// A start straight on the player (a map music trigger, 5): the menu
    /// layer's name is bypassed.
    pub fn start_direct(&mut self, file: &str, looping: bool, fade: f64, now: f64) {
        let opens = self.opens(file);
        self.player.start(file, looping, fade * 1000.0, now, opens);
    }

    /// Close both slots and forget the current name.
    pub fn stop(&mut self) {
        self.player.stop();
        self.current.clear();
    }

    /// An interface sound (2D, full gain, the effects channel): its
    /// attribute key.
    pub fn sound(&mut self, key: &str) {
        self.sounds.push(key.to_string());
        self.heard.push(key.to_string());
    }

    /// The next playlist track, fading in over the menu fade.
    fn start_next(&mut self, now: f64) {
        let files = self.files.clone();
        let mut playlist = std::mem::take(&mut self.playlist);
        let mut rng = self.rng;
        let next = playlist.next(&|f| files.exists(f), |n| {
            let mut x = rng;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            rng = x;
            (x % n.max(1) as u64) as usize
        });
        self.playlist = playlist;
        self.rng = rng;
        if let Some((file, looping)) = next {
            self.start(&file, looping, self.data.menu_fade, now);
        }
    }

    fn draw(&mut self, n: usize) -> usize {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        (x % n.max(1) as u64) as usize
    }

    /// The party playlist's next track, fading in over the menu fade: the
    /// next of a shuffled bag of the playlist's playable files, which
    /// refills (shuffled again, not starting with the track just played)
    /// when it is empty, so every track plays once before any repeats.
    fn party_next(&mut self, now: f64) {
        self.fill_bag();
        // The next track that is here (one still coming waits its turn);
        // none left in this pass: a new pass (those still coming are in it
        // again).
        if !self.bag.iter().any(|f| self.files.ready(f)) {
            self.bag.clear();
            self.fill_bag();
        }
        let Some(i) = self.bag.iter().rposition(|f| self.files.ready(f)) else { return };
        let file = self.bag.remove(i);
        // A playlist of one repeats its track.
        let looping = self.party_playlist_size() == 1;
        self.party_last = file.clone();
        self.start(&file, looping, self.data.menu_fade, now);
    }

    /// The party playlist's bag refilled (shuffled) when empty.
    fn fill_bag(&mut self) {
        if self.bag.is_empty() {
            let files = &self.files;
            let source: Vec<&String> = if self.data.party.playlist.is_empty() {
                self.data.playlist.iter().map(|e| &e.file).collect()
            } else {
                self.data.party.playlist.iter().collect()
            };
            let mut bag: Vec<String> = Vec::new();
            for f in source {
                if files.exists(f) && !bag.contains(f) {
                    bag.push(f.clone());
                }
            }
            for i in (1..bag.len()).rev() {
                let j = self.draw(i + 1);
                bag.swap(i, j);
            }
            // Popped from the end: the track just played is not next.
            if bag.len() > 1 && bag.last().is_some_and(|l| l.eq_ignore_ascii_case(&self.party_last)) {
                let n = bag.len();
                bag.swap(0, n - 1);
            }
            self.bag = bag;
        }
    }

    /// The party playlist's next track (the bag drawn when empty), what
    /// `party_next` plays when it is here: what to fetch ahead.
    pub fn party_upcoming(&mut self) -> Option<String> {
        if self.party_music != 1 {
            return None;
        }
        self.fill_bag();
        self.bag.last().cloned()
    }

    /// The party music's files a match plays from its start on (the mode's
    /// start track, the last-stage music, the loading track) and the
    /// playlist's first: what a client fetches with the match's files.
    pub fn party_first_files(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if self.party_music == 0 {
            return out;
        }
        let mode = self.data.party.mode.clone();
        if let Some((file, _)) = self.data.mode_track(&mode, None) {
            out.push(file.to_string());
        }
        out.push(self.data.cues.last_stage.clone());
        out.push(self.data.cues.loading.clone());
        out.extend(self.party_upcoming());
        out.retain(|f| !f.is_empty());
        out
    }

    /// The party track playing now and the ms left of it (None: none, or
    /// its length unknown).
    pub fn party_left(&self, now: f64) -> Option<(String, f64)> {
        if !self.party || !self.playlist_on {
            return None;
        }
        // The newest stream: the one fading in during a cross-fade.
        let p = self.player.secondary.as_ref().or(self.player.primary.as_ref())?;
        let len = self.files.length_ms(&p.file)?;
        Some((p.file.clone(), (len - p.position(now, Some(len))).max(0.0)))
    }

    /// The `party_music` cvar set: None (0) in a party match stops what
    /// plays at once (the original's loop too, which the playlist's frame
    /// never looks at).
    pub fn set_party_music(&mut self, n: u8) {
        self.party_music = n;
        if self.party && n == 0 {
            self.playlist_on = false;
            self.stop();
        }
    }

    fn party_playlist_size(&self) -> usize {
        let src: Vec<&String> = if self.data.party.playlist.is_empty() {
            self.data.playlist.iter().map(|e| &e.file).collect()
        } else {
            self.data.party.playlist.iter().collect()
        };
        let mut seen: Vec<&String> = Vec::new();
        for f in src {
            if self.files.exists(f) && !seen.contains(&f) {
                seen.push(f);
            }
        }
        seen.len()
    }

    /// The party playlist's update, once a frame: the track after the one
    /// that is near its end (the menu fade left), as the menu's.
    fn party_frame(&mut self, now: f64) {
        if !self.party || !self.playlist_on || self.screen != Screen::Match || self.last_stage_on {
            return;
        }
        match self.party_music {
            1 => {
                let window = self.data.menu_fade * 1000.0;
                let files = &self.files;
                if !self.player.playing() || self.player.near_end_within(now, window, |f| files.length_ms(f)) {
                    self.party_next(now);
                }
            }
            // Changed while it played: off is silent, the original's
            // start track loops.
            m => {
                self.playlist_on = false;
                self.stop();
                if m != 0 {
                    self.mode_music(now);
                }
            }
        }
    }

    /// The menu update (3), once a frame while a menu screen is shown.
    pub fn menu_frame(&mut self, now: f64) {
        if self.paused {
            self.stop();
            return;
        }
        if self.player.playing() {
            let (eligible, near) = {
                let files = &self.files;
                let playable = |f: &str| files.exists(f);
                (
                    self.playlist.cursor_eligible(&playable),
                    self.player.near_end(now, |f| files.length_ms(f)),
                )
            };
            if eligible {
                if near {
                    self.start_next(now);
                }
            } else {
                self.stop();
                self.start_next(now);
            }
        } else {
            self.start_next(now);
        }
    }

    /// One client frame: the menu update on a menu screen, then the fade.
    pub fn frame(&mut self, now: f64) {
        if matches!(self.screen, Screen::Menu | Screen::Result) {
            self.menu_frame(now);
        }
        self.party_frame(now);
        self.player.update(now);
    }

    /// The streams with the music master `m` (0..1) applied.
    pub fn mixed(&self, m: f32) -> Vec<Mixed> {
        self.player
            .gains()
            .map(|(s, g): (&Stream, f32)| Mixed {
                file: s.file.clone(),
                looping: s.looping,
                started: s.started,
                gain: g * m,
            })
            .collect()
    }

    // --- Moments (4).

    fn mode_music(&mut self, now: f64) {
        let Some((file, looping)) = self.data.mode_track(&self.mode, None).map(|(f, l)| (f.to_string(), l)) else {
            return;
        };
        if self.party {
            match self.party_music {
                0 => return,
                // The start track once; the playlist follows it.
                1 => {
                    self.stop();
                    self.start(&file, false, 0.0, now);
                    self.playlist_on = true;
                    return;
                }
                _ => {}
            }
        }
        self.stop();
        self.start(&file, looping, 0.0, now);
    }

    /// Room to match: stop, the loading track (full volume, no loop).
    pub fn room_start(&mut self, now: f64) {
        self.screen = Screen::Loading;
        self.reset_map();
        self.stop();
        let f = self.data.cues.loading.clone();
        self.start(&f, false, 0.0, now);
    }

    /// The loading screen closes into the world; Quarantine's mode music
    /// fades in over 10 s.
    pub fn loading_closed(&mut self, mode: &str, now: f64) {
        self.screen = Screen::Match;
        self.mode = mode.to_string();
        self.stop();
        if mode == "QRT" {
            let (f, fade) = (self.data.cues.quarantine_mode.clone(), self.data.quarantine_mode_fade);
            self.start(&f, false, fade, now);
        }
    }

    /// A map begins (or the world is entered without a loading screen):
    /// the per-map cue state is fresh.
    pub fn reset_map(&mut self) {
        self.playlist_on = false;
        self.mode_music_done = false;
        self.qrt_end_played = false;
        self.capture_on = false;
        self.last_stage_on = false;
    }

    /// A round starts. `alive`: the local player lives. A warm-up only
    /// plays `FirstSpawnSound`; the first real round start plays
    /// `RoundStartSound` and the mode's music (stop, then start); later
    /// ones the sound alone.
    pub fn round_start(&mut self, mode: &str, warmup: bool, alive: bool, now: f64) {
        self.mode = mode.to_string();
        self.qrt_end_played = false;
        self.capture_on = false;
        if warmup {
            if alive {
                self.sound("FirstSpawnSound");
            }
            return;
        }
        let first = !self.mode_music_done;
        self.mode_music_done = true;
        if !alive {
            return;
        }
        self.sound("RoundStartSound");
        if first {
            self.mode_music(now);
        }
    }

    /// Quarantine's real round has `left` seconds on the clock.
    pub fn quarantine_clock(&mut self, left: f64, now: f64) {
        let [lo, hi] = self.data.quarantine_end_window;
        if self.qrt_end_played || !(left > lo && left <= hi) {
            return;
        }
        self.qrt_end_played = true;
        self.stop();
        let f = self.data.cues.quarantine_end.clone();
        self.start(&f, false, 0.0, now);
    }

    /// Seize and Secure: the capture timer (seconds left; None: it is not
    /// running).
    pub fn capture_timer(&mut self, left: Option<f64>, now: f64) {
        match left {
            Some(t) if t <= self.data.hq_countdown => {
                if !self.capture_on {
                    self.capture_on = true;
                    self.stop();
                    let f = self.data.cues.capture_progress.clone();
                    self.start(&f, false, 0.0, now);
                }
            }
            _ => {
                if self.capture_on {
                    self.capture_on = false;
                    self.stop();
                }
            }
        }
    }

    /// Arms Race: `n` players are on the last stage.
    pub fn last_stage(&mut self, n: u32, now: f64) {
        self.last_stage_with(n, now, true);
    }

    /// `last_stage`, its `last_stage_sound` played or not (a party match:
    /// the cue pack plays it).
    pub fn last_stage_with(&mut self, n: u32, now: f64, sound: bool) {
        if n > 0 && !self.last_stage_on {
            self.last_stage_on = true;
            if sound {
                self.sound("last_stage_sound");
            }
            let f = self.data.cues.last_stage.clone();
            if self.party && self.party_music == 0 {
                return;
            }
            if !self.current.eq_ignore_ascii_case(&f) {
                self.stop();
                self.start(&f, true, 0.0, now);
            }
        } else if n == 0 && self.last_stage_on {
            self.last_stage_on = false;
            self.stop();
            if self.party && self.party_music != 2 {
                // Back to the playlist, not the start track.
                if self.party_music == 1 && self.playlist_on {
                    self.party_next(now);
                }
            } else if self.data.modes.get(&self.mode).is_some_and(|m| m.looping) {
                self.mode_music(now);
            }
        }
    }

    /// A perk with a sound file as music (war cry, eagle eye): replaces
    /// whatever plays, and the mode's music doesn't come back (Quirk 5).
    pub fn perk_cue(&mut self, war_cry: bool, now: f64) {
        let f = if war_cry {
            &self.data.cues.war_cry
        } else {
            &self.data.cues.eagle_eye
        }
        .clone();
        self.stop();
        self.start(&f, false, 0.0, now);
    }

    /// A round's result (also the warm-up's): the win sound by the
    /// winning team, the same for both teams (Quirk 6).
    pub fn round_result(&mut self, mode: &str, winner: Option<u8>, my_team: u8) {
        let Some(m) = rules::mode_by_key(mode) else { return };
        let key = if mode == "QRT" {
            match winner {
                Some(ALPHA) => "QRTHumanWinSound",
                Some(BRAVO) => "QRTInfectorWinSound",
                _ => return,
            }
        } else {
            match (m.teams, winner) {
                (Teams::FreeForAll, _) => return,
                (Teams::AllAlpha, _) if my_team != ALPHA => return,
                (_, Some(ALPHA)) => "RoundAlphaTeamWinSound",
                (_, Some(BRAVO)) => "RoundBravoTeamWinSound",
                _ => return,
            }
        };
        self.sound(key);
    }

    /// The match ends: the client leaves the world for the result screen,
    /// music stopped and paused; one win or lose sound (a draw: none).
    /// `first`: the local player is first on the scoreboard.
    pub fn match_end(&mut self, mode: &str, result: Option<Option<u8>>, my_team: u8, first: bool) {
        self.screen = Screen::Result;
        self.stop();
        self.paused = true;
        let Some(m) = rules::mode_by_key(mode) else { return };
        let win = if mode == "QRT" {
            Some(true)
        } else {
            match (m.teams, result) {
                (_, Some(None)) | (_, None) => None,
                (Teams::FreeForAll, Some(_)) => Some(first),
                (_, Some(Some(w))) => Some(w == my_team),
            }
        };
        match win {
            Some(true) => self.sound("ResultWinSound"),
            Some(false) => self.sound("ResultLoseSound"),
            None => {}
        }
    }

    /// A party match is decided (its match-end screen): music stopped and
    /// paused; the screen plays the win or lose sound.
    pub fn party_end(&mut self) {
        self.playlist_on = false;
        self.screen = Screen::Result;
        self.stop();
        self.paused = true;
    }

    /// Leaving the result screen: unpaused, the playlist starts its next
    /// track at the next menu frame.
    pub fn leave_result(&mut self) {
        self.paused = false;
        self.screen = Screen::Menu;
    }

    /// Leaving a match from the in-game menu: stop.
    pub fn leave_match(&mut self) {
        self.playlist_on = false;
        self.screen = Screen::Menu;
        self.reset_map();
        self.stop();
    }
}

/// What the client knows of the match each frame.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MusicFacts {
    /// The screen the client shows.
    pub screen: Screen,
    /// The room key; empty: no mode.
    pub mode: String,
    pub phase: rules::Phase,
    pub warmup: bool,
    pub winner: Option<u8>,
    pub result: Option<Option<u8>>,
    /// The round clock's seconds left while live.
    pub round_left: Option<f64>,
    pub my_team: u8,
    pub alive: bool,
    /// Seize and Secure: the capture timer's seconds left.
    pub capture_left: Option<f64>,
    /// Arms Race: players on the last stage.
    pub last_stage: u32,
    /// The local player is first on the scoreboard.
    pub first: bool,
    /// The local player has War Cry's or Eagle Eye's buff.
    pub war_cry: bool,
    pub eagle_eye: bool,
    /// A party match's moments (a minigame plays): Section 10's cues
    /// instead of the mode's.
    pub party: Option<MinigameMoments>,
}

fn in_round(p: &rules::Phase) -> bool {
    matches!(p, rules::Phase::Setup { .. } | rules::Phase::Live)
}

/// The moments between two frames' facts (order of events at phase
/// changes): the screen's changes first, then the round's start, clocks
/// and results.
pub fn observe(m: &mut CaMusic, prev: &MusicFacts, now: &MusicFacts, ms: f64) {
    use rules::Phase;
    m.party = now.party.is_some();
    if let Some(party) = &now.party {
        observe_party(m, prev, now, party, ms);
        return;
    }
    match (prev.screen, now.screen) {
        (a, Screen::Loading) if a != Screen::Loading => m.room_start(ms),
        (Screen::Loading, Screen::Match) => m.loading_closed(&now.mode, ms),
        (Screen::Menu, Screen::Match) => {
            // A map entered with no loading screen (a local `map`).
            m.reset_map();
            m.loading_closed(&now.mode, ms);
        }
        (Screen::Match, Screen::Menu) => m.leave_match(),
        (Screen::Match, Screen::Result) => m.match_end(&now.mode, now.result, now.my_team, now.first),
        (Screen::Result, Screen::Menu) => m.leave_result(),
        (Screen::Result, Screen::Match) => {
            // A new match without the lobby in between (a restart).
            m.leave_result();
            m.reset_map();
            m.loading_closed(&now.mode, ms);
        }
        _ => {}
    }
    m.screen = now.screen;
    if now.screen != Screen::Match {
        return;
    }
    let before = if prev.screen == Screen::Match {
        prev.phase
    } else {
        Phase::Waiting
    };
    // A round starts: the phase enters setup or live from outside them.
    if in_round(&now.phase) && !in_round(&before) {
        m.round_start(&now.mode, now.warmup, now.alive, ms);
    }
    if now.mode == "QRT"
        && now.phase == Phase::Live
        && !now.warmup
        && let Some(left) = now.round_left
    {
        m.quarantine_clock(left, ms);
    }
    if now.mode == "HQM" {
        let live = now.phase == Phase::Live && !now.warmup;
        m.capture_timer(now.capture_left.filter(|_| live), ms);
    }
    if now.mode == "VWRM" && now.phase == Phase::Live {
        m.last_stage(now.last_stage, ms);
    }
    // A perk with a sound file as music: when the buff begins.
    if prev.screen == Screen::Match {
        if now.war_cry && !prev.war_cry {
            m.perk_cue(true, ms);
        } else if now.eagle_eye && !prev.eagle_eye {
            m.perk_cue(false, ms);
        }
    }
    // A round's result (the warm-up's too).
    if matches!(now.phase, Phase::RoundOver { .. }) && !matches!(before, Phase::RoundOver { .. }) {
        m.round_result(&now.mode, now.winner, now.my_team);
    }
}

/// A party match's moments (Section 10; module doc). `now.screen` is the
/// loading screen, the match or its match-end screen (`Result`).
pub fn observe_party(m: &mut CaMusic, prev: &MusicFacts, now: &MusicFacts, party: &MinigameMoments, ms: f64) {
    if party.music != Some(crate::game::GAME) {
        // Another game's music, or none: Combat Arms' is silent (and its
        // playlist doesn't run).
        m.playlist_on = false;
        if m.player.playing() {
            m.stop();
        }
        m.paused = false;
        m.screen = Screen::Match;
        return;
    }
    let before = prev.party.as_ref().filter(|_| prev.screen == Screen::Match);
    match (prev.screen, now.screen) {
        (Screen::Result, s) if s != Screen::Result => {
            m.leave_result();
            m.reset_map();
            if s == Screen::Loading {
                m.room_start(ms);
            } else {
                m.loading_closed(&now.mode, ms);
            }
        }
        (a, Screen::Loading) if a != Screen::Loading => m.room_start(ms),
        (Screen::Loading, Screen::Match) => m.loading_closed(&now.mode, ms),
        (Screen::Menu, Screen::Match) => {
            m.reset_map();
            m.loading_closed(&now.mode, ms);
        }
        (Screen::Match, Screen::Result) => m.party_end(),
        _ => {}
    }
    m.screen = now.screen;
    if now.screen != Screen::Match {
        return;
    }
    let was = before.map_or(MomentPhase::Waiting, |p| p.phase);
    // A new match on the same map (no match-end screen between).
    if was == MomentPhase::Over && party.phase != MomentPhase::Over {
        m.reset_map();
    }
    // The start of play: the round start's sound and the start track, if
    // alive (dead then: no music this match).
    if party.phase == MomentPhase::Live && was != MomentPhase::Live {
        m.round_start(&now.mode, false, party.alive, ms);
    }
    if party.phase == MomentPhase::Live {
        m.last_stage_with(party.last_rung, ms, false);
    }
    if party.respawn_in.is_some() && party.respawn_in != before.and_then(|p| p.respawn_in) {
        m.sound("RespawnCountSound");
    }
}
