//! An app's own interface sounds (docs/plans/active/ugg-ui.md, "UI
//! sound"): the `ui` bus. The UI sends `chimera_ui::UiCue`s (an event's
//! name, e.g. `ui_hover`, `lobby_ready`, `rank_up`) and asks for a loop
//! with `chimera_ui::UiMusic`; this plays them from the app's table and
//! files (`UiSounds`, inserted by the app: Ultimate Gun Game's Signal
//! look), and only under a theme that turns UI sound on
//! (`flags.sound.ui`: `active`). Without both, every cue is silent and
//! nothing runs (Lucker Party, the plain apps).
//!
//! - **The table** (`SoundTable`, TOML, the app's data): per event its
//!   `file` (a graded event's has `{nn}` and `grid = N`: the render
//!   nearest the cue's value, `nn` = round(value x N), two digits), its
//!   `gain` (dB; the files are baked at their tier's loudness, so 0), the
//!   per-play spread (`cents`, `db`: uniform within plus or minus), its
//!   `voices` (at most that many at once; a new play cuts the oldest
//!   with a click-free fade, so hover is one voice), `on` (off: silent;
//!   Tab and the kill feed's own line are off by default), `rate =
//!   "value"` (a slider: the speed follows the cue's value, 0.5 to 2.0,
//!   no random pitch), `loop = true` (music: on the music bus,
//!   `snd_musicvolume`, looping from the file's loop point, else
//!   seamlessly from its start). `[bus] duck_db`: how far the `ui` bus
//!   ducks while an announcer voice plays (`announcer::AnnouncerVoice`).
//! - **The bus**: a cue plays unpositioned as the announcer's voice and
//!   the match-end sound do (a `LiveClip` with flat gains, stereo as left
//!   and right, `PlaybackSettings::DESPAWN`), never through the room's
//!   DSP, at its gain times `snd_uivolume` (`SoundMix::ui`, default 0.8),
//!   times the duck, under the master `GlobalVolume`; the gains follow
//!   the volume while it plays.
//! - **State**: `UiVoices` holds the voices playing and a log of what
//!   played (`played`), kept headless too, so tests read them; the audio
//!   entities are spawned only where there is audio (`Assets<LiveClip>`).
//!
//! Known shortcuts: the rank-up plays the nearest of the table's
//! pre-rendered grid (17 renders, p in steps of 1/16) instead of rendering
//! `rank_up(p, seed)` for the exact rung (a Rust port of the generator is
//! the follow-up, tools/sounds/); the files are WAV (no compressed format
//! decodes yet), embedded in the app's binary.

use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use bevy::prelude::*;
pub use chimera_ui::sound::{CueEntry, SoundTable, UiCue, UiMusic};
/// The theme whose `flags.sound.ui` turns UI sound on (re-exported for apps
/// without the kit as a dependency).
pub use chimera_ui::Theme;

use crate::{
    audio::live_sound::{Gains, LiveClip, SoundMix},
    map::MapSoundClip,
};

/// The app's sounds: its table and its files decoded (module doc).
#[derive(Resource, Clone, Debug)]
pub struct UiSounds {
    pub table: SoundTable,
    pub clips: BTreeMap<String, MapSoundClip>,
}

impl UiSounds {
    /// From the table's TOML and the files' bytes (`name` without
    /// `.wav`). A file that doesn't decode is left out and said; a file
    /// the table names but none given is an error.
    pub fn new(table: &str, files: &[(&str, &[u8])]) -> Result<Self, String> {
        let table = SoundTable::from_toml(table)?;
        let mut clips = BTreeMap::new();
        for (name, bytes) in files {
            match crate::map::sound::decode_clip(bytes) {
                Ok(c) => {
                    clips.insert(name.to_string(), c);
                }
                Err(e) => warn!("ui sound: {name}.wav doesn't decode: {e}"),
            }
        }
        let missing: Vec<String> = table.files().into_iter().filter(|f| !clips.contains_key(f)).collect();
        if !missing.is_empty() {
            return Err(format!("ui sound: no file for {}", missing.join(", ")));
        }
        Ok(Self { table, clips })
    }
}

impl UiSounds {
    /// The table with a tenth of a second of silence for each file it
    /// names (tests: what plays is read from `UiVoices`).
    pub fn silent(table: &str) -> Result<Self, String> {
        let table = SoundTable::from_toml(table)?;
        let clip = MapSoundClip { rate: 48_000, channels: 1, samples: vec![0i16; 4800].into(), loop_start: None };
        let clips = table.files().into_iter().map(|f| (f, clip.clone())).collect();
        Ok(Self { table, clips })
    }
}

/// Whether UI sound plays: the app's sounds are in and its theme turns
/// them on (`flags.sound.ui`).
pub fn active(theme: Option<&Theme>, sounds: Option<&UiSounds>) -> bool {
    sounds.is_some() && theme.is_some_and(|t| t.flag("sound", "ui"))
}

/// A run condition: `active`.
pub fn ui_sound_on(theme: Option<Res<Theme>>, sounds: Option<Res<UiSounds>>) -> bool {
    active(theme.as_deref(), sounds.as_deref())
}

/// A voice playing.
#[derive(Clone, Debug)]
pub struct UiVoice {
    pub event: String,
    pub file: String,
    /// Playback speed (1: as recorded).
    pub speed: f32,
    /// Its own gain (the table's and the spread's), before the bus.
    pub gain: f32,
    /// `Time<Real>` seconds.
    pub started: f64,
    pub ends: f64,
    gains: Arc<Gains>,
    entity: Option<Entity>,
}

impl UiVoice {
    /// The gain it plays at now (left and right alike).
    pub fn heard(&self) -> f32 {
        self.gains.get().0
    }
    /// Its audio entity (none headless).
    pub fn entity(&self) -> Option<Entity> {
        self.entity
    }
}

/// One play, logged.
#[derive(Clone, Debug, PartialEq)]
pub struct Played {
    pub event: String,
    pub file: String,
    pub speed: f32,
    pub gain: f32,
}

/// The voices playing and what played (module doc).
#[derive(Resource, Default)]
pub struct UiVoices {
    pub voices: Vec<UiVoice>,
    /// The music loop playing.
    pub music: Option<UiVoice>,
    /// Every play, oldest first (the last `LOG` kept).
    pub played: Vec<Played>,
    /// The `ui` bus's duck now (1: none).
    pub duck: f32,
    rng: u64,
}

impl UiVoices {
    /// The events played, in order.
    pub fn events(&self) -> Vec<&str> {
        self.played.iter().map(|p| p.event.as_str()).collect()
    }
    /// The files played, in order.
    pub fn files(&self) -> Vec<&str> {
        self.played.iter().map(|p| p.file.as_str()).collect()
    }
    fn uniform(&mut self) -> f32 {
        // xorshift64*
        let mut x = self.rng | 1;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        ((x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

/// How many plays `UiVoices::played` keeps.
pub const LOG: usize = 256;

/// The `ui` bus's systems (`PostUpdate`, after the screens sent their cues).
pub struct UiSoundPlugin;

impl Plugin for UiSoundPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<Messages<UiCue>>() {
            app.add_message::<UiCue>();
        }
        app.init_resource::<UiMusic>()
            .init_resource::<SoundMix>()
            .insert_resource(UiVoices { duck: 1.0, rng: 0x9E37_79B9_7F4A_7C15, ..default() })
            .add_systems(
                PostUpdate,
                (play_cues, play_music, mix).chain().run_if(resource_exists::<UiSounds>),
            );
    }
}

fn clip_secs(c: &MapSoundClip) -> f64 {
    let frames = c.samples.len() / (c.channels.max(1) as usize);
    frames as f64 / c.rate.max(1) as f64
}

fn spawn_voice(
    commands: &mut Commands,
    clips: &mut Option<ResMut<Assets<LiveClip>>>,
    clip: MapSoundClip,
    gains: &Arc<Gains>,
    speed: f32,
) -> Option<Entity> {
    let clips = clips.as_mut()?;
    let handle = clips.add(LiveClip::new(clip, gains.clone(), default()));
    let settings = PlaybackSettings { speed, ..PlaybackSettings::DESPAWN };
    Some(commands.spawn((AudioPlayer(handle), settings)).id())
}

#[allow(clippy::too_many_arguments)]
fn play_cues(
    mut cues: MessageReader<UiCue>,
    theme: Option<Res<Theme>>,
    sounds: Res<UiSounds>,
    mix: Res<SoundMix>,
    time: Res<Time<Real>>,
    mut voices: ResMut<UiVoices>,
    mut clips: Option<ResMut<Assets<LiveClip>>>,
    mut commands: Commands,
) {
    let on = active(theme.as_deref(), Some(&sounds));
    let now = time.elapsed_secs_f64();
    for cue in cues.read() {
        if !on {
            continue;
        }
        let Some(entry) = sounds.table.events.get(cue.event.as_ref()) else {
            debug!("ui sound: no event {}", cue.event);
            continue;
        };
        if !entry.on || entry.looping {
            continue;
        }
        let file = entry.file_for(cue.value);
        let Some(clip) = sounds.clips.get(&file).cloned() else { continue };
        let speed = if entry.rate_from_value {
            0.5 + 1.5 * cue.value.unwrap_or(0.5)
        } else {
            let u = voices.uniform();
            2f32.powf(entry.cents * u / 1200.0)
        };
        let u = voices.uniform();
        let gain = 10f32.powf((entry.gain + entry.db * u) / 20.0);
        // The voice limit: the oldest of this event's go first.
        let mut same: Vec<usize> =
            voices.voices.iter().enumerate().filter(|(_, v)| v.event == cue.event).map(|(i, _)| i).collect();
        while same.len() >= entry.voices {
            let i = same.remove(0);
            voices.voices[i].gains.fade_out();
            voices.voices[i].ends = now;
        }
        voices.voices.retain(|v| v.ends > now);
        let level = gain * mix.ui.clamp(0.0, 1.0) * voices.duck;
        let gains = Arc::new(Gains::new(level, level));
        let ends = now + clip_secs(&clip) / speed.max(0.01) as f64;
        let entity = spawn_voice(&mut commands, &mut clips, clip, &gains, speed);
        voices.voices.push(UiVoice {
            event: cue.event.to_string(),
            file: file.clone(),
            speed,
            gain,
            started: now,
            ends,
            gains,
            entity,
        });
        voices.played.push(Played { event: cue.event.to_string(), file, speed, gain });
        if voices.played.len() > LOG {
            let over = voices.played.len() - LOG;
            voices.played.drain(..over);
        }
    }
}

/// The loop `UiMusic` asks for (only while UI sound is on).
#[allow(clippy::too_many_arguments)]
fn play_music(
    want: Res<UiMusic>,
    theme: Option<Res<Theme>>,
    sounds: Res<UiSounds>,
    mix: Res<SoundMix>,
    time: Res<Time<Real>>,
    mut voices: ResMut<UiVoices>,
    mut clips: Option<ResMut<Assets<LiveClip>>>,
    mut commands: Commands,
) {
    let on = active(theme.as_deref(), Some(&sounds));
    let want: Option<Cow<'static, str>> = want.0.clone().filter(|_| on);
    let playing = voices.music.as_ref().map(|m| m.event.clone());
    if playing.as_deref() == want.as_deref() {
        return;
    }
    if let Some(old) = voices.music.take() {
        old.gains.fade_out();
    }
    let Some(event) = want else { return };
    let Some(entry) = sounds.table.events.get(event.as_ref()).filter(|e| e.on) else { return };
    let file = entry.file_for(None);
    let Some(mut clip) = sounds.clips.get(&file).cloned() else { return };
    if entry.looping && clip.loop_start.is_none() {
        // A seamless file loops from its first frame.
        clip.loop_start = Some(0);
    }
    let gain = 10f32.powf(entry.gain / 20.0);
    let level = gain * mix.music.clamp(0.0, 1.0);
    let gains = Arc::new(Gains::new(level, level));
    let now = time.elapsed_secs_f64();
    let entity = spawn_voice(&mut commands, &mut clips, clip, &gains, 1.0);
    info!("ui sound: music {file} at {level:.2}");
    voices.music = Some(UiVoice {
        event: event.to_string(),
        file: file.clone(),
        speed: 1.0,
        gain,
        started: now,
        ends: f64::INFINITY,
        gains,
        entity,
    });
    voices.played.push(Played { event: event.to_string(), file, speed: 1.0, gain });
}

/// The voices' gains follow the bus (the volume, the duck); ended ones go.
fn mix(
    sounds: Res<UiSounds>,
    mix: Res<SoundMix>,
    time: Res<Time<Real>>,
    announcer: Query<(), With<super::announcer::AnnouncerVoice>>,
    mut voices: ResMut<UiVoices>,
) {
    let now = time.elapsed_secs_f64();
    let duck = if announcer.is_empty() { 1.0 } else { 10f32.powf(-sounds.table.duck_db / 20.0) };
    voices.duck = duck;
    let ui = mix.ui.clamp(0.0, 1.0);
    voices.voices.retain(|v| v.ends > now);
    for v in &voices.voices {
        let level = v.gain * ui * duck;
        if (v.gains.get().0 - level).abs() > 1e-6 {
            v.gains.set(level, level);
        }
    }
    if let Some(m) = &voices.music {
        let level = m.gain * mix.music.clamp(0.0, 1.0);
        if (m.gains.get().0 - level).abs() > 1e-6 {
            m.gains.set(level, level);
        }
    }
}
