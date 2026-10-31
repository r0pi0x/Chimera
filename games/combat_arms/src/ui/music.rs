//! Playing Combat Arms' music and ambience (specs/combat_arms/
//! music_ambience.md; the state is `game::music`): the front end reads
//! what the client decided and plays it through Chimera's long-lived
//! sounds (`map::SoundControl`).
//!
//! - **The install's music** (`scan`): the loose files under `Game/BGM/`
//!   (found by name without case) and the player's own MP3s in `Jukebox/`
//!   next to the game folder are what the player can open; a Jukebox
//!   file joins the playlist ticked. The music is read from loose files,
//!   not from `BGM.rez`; with no install folder (the game served by the
//!   asset store or service) the same files come through the game's mount
//!   (`mount::LOOSE_MUSIC`, fetched when first played).
//! - **Music** (`play_music`): each stream of the player (`CaMusic::
//!   mixed`: the music master times its slot's gain) is a live sound
//!   under a key of its own, decoded on a background task the first time
//!   it plays; each frame its volume follows the fade. A stream that
//!   leaves the player stops.
//! - **Ambience** (`play_ambient`): every looping, switched-on `SoundFX`
//!   of the map (`CaAmbient`) is a live sound from the map's load, muted
//!   by its distance gain (`ambient::gain`, times `Volume` and the effects
//!   master); a sound with `Ambient` off is placed at its object with
//!   Chimera's radius level, following it with `PlayAttached` (the
//!   object's anchor, `objects::sound`).
//! - **The window** (`minimized`): both masters go to 0 while the window
//!   has no size.
//!
//! Known shortcuts (docs/tech-debt.md): a stream starts from its
//! beginning when its decode finishes, not where the wall clock has it
//! (a long file's first seconds are late); decoded tracks stay in the
//! sound bank until the map changes (a few hundred MB for a whole
//! playlist); a stream that outlives its map's sound bank restarts; the
//! window counts as minimized when it has no pixels (what Windows
//! reports), not by the platform's iconic state; the listener is the
//! local player's body (not a spectated one); the 3D map sounds' filters
//! (`Filter`) don't apply.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
    window::PrimaryWindow,
};

use crate::{
    core::LocalPlayer,
    game::{
        INFO,
        effects::{library, rules},
        inventory::UNIT,
        music::{CaMusicSet, MusicLoading, ambient::CaAmbient, data::normalize, director::CaMusic, mix::CaVolumes},
        objects::{
            CaObjects,
            geometry::{from_engine, to_engine},
        },
    },
    map::{
        MapSoundClip, SoundControl, SoundKey, SoundLevel, StartSound,
        sound::{MapSounds, SoundBank},
    },
};

/// Music keys, ambient keys (below the logic's bit 63 and the
/// soundscape's bit 62).
const MUSIC_KEYS: u64 = 1 << 59;
const AMBIENT_KEYS: u64 = 1 << 60;

enum Clip {
    Loading(Task<Option<MapSoundClip>>),
    Ready(usize),
    Failed,
}

/// What the player has started.
#[derive(Resource, Default)]
pub struct Playback {
    /// Normalized music path to the file on disk.
    paths: HashMap<String, PathBuf>,
    /// Normalized music path to its path in the game's mount (served).
    mounted: HashMap<String, String>,
    scanned: bool,
    /// Decoded music by file.
    clips: HashMap<String, Clip>,
    /// A stream (file, start time bits) to its key and last volume.
    playing: HashMap<(String, u64), (SoundKey, f32)>,
    next_key: u64,
    /// Ambient sounds (object id) to their last volume.
    ambient: HashMap<u32, f32>,
    /// Decoded ambient files by name.
    sound_files: HashMap<String, Option<usize>>,
}

impl Playback {
    /// A music file the client can open: its normalized path (`bgm/...`)
    /// and where it is on disk.
    pub fn add_file(&mut self, key: &str, path: PathBuf) {
        self.paths.insert(normalize(key), path);
    }

    /// The streams started and the ambient sounds going, for the tests.
    pub fn counts(&self) -> (usize, usize) {
        (self.playing.len(), self.ambient.len())
    }
}

/// The volume an ambient sound is played at: its distance gain at the
/// listener times `Volume` and the effects master `s`; a placed sound
/// (`Ambient` off) is at `Volume` times `s` and Chimera's radius level
/// does the rest. No listener: silent.
pub fn ambient_volume(s: &crate::game::music::ambient::SoundFx, listener: Option<Vec3>, master: f32) -> f32 {
    if !s.ambient {
        return s.volume / 100.0 * master;
    }
    listener.map_or(0.0, |p| s.gain_at(s.at.distance(p))) * master
}

/// The playback.
pub struct CaMusicPlayPlugin;

impl Plugin for CaMusicPlayPlugin {
    fn build(&self, app: &mut App) {
        crate::game::data::CaData::of(app);
        app.init_resource::<Playback>()
            .add_systems(Update, loading_flag.before(CaMusicSet))
            .add_systems(
                Update,
                (scan, minimized, play_music, play_ambient).chain().after(CaMusicSet),
            );
    }
}

/// The loading screen is up (the client is joining a server).
fn loading_flag(loading: Option<Res<super::loading::CaLoading>>, mut flag: ResMut<MusicLoading>) {
    let up = loading.is_some_and(|l| l.label.is_some());
    if flag.0 != up {
        flag.0 = up;
    }
}

/// Find the install's music once.
fn scan(mut pb: ResMut<Playback>, mut music: ResMut<CaMusic>, data: Res<crate::game::data::CaData>) {
    if pb.scanned {
        return;
    }
    pb.scanned = true;
    if data.builtin_only() {
        return;
    }
    let Some(install) = INFO.install() else { return };
    let mut found = Vec::new();
    walk(&install.join("Game").join("BGM"), "bgm", &mut found);
    if !install.join("Game").is_dir()
        && let Some(mount) = crate::game::world::mount()
    {
        // Served: the loose music as the packer stored it.
        let prefix = format!("{}/", crate::game::mount::LOOSE_MUSIC);
        for (e, _) in mount.entries() {
            if let Some(rest) = e.path.strip_prefix(&prefix) {
                let key = format!("bgm/{rest}");
                let len = music.data.files.get(&key).map(|s| s * 1000.0);
                music.files.insert(&key, len);
                pb.mounted.insert(key, e.path);
            }
        }
    }
    let mut jukebox = Vec::new();
    if let Ok(dir) = std::fs::read_dir(install.join("Jukebox")) {
        for e in dir.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("mp3"))
                && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
            {
                jukebox.push((stem.to_string(), p));
            }
        }
    }
    jukebox.sort();
    for (key, path) in found {
        let len = music.data.files.get(&key).map(|s| s * 1000.0);
        music.files.insert(&key, len);
        pb.paths.insert(key, path);
    }
    music
        .playlist
        .set_jukebox(&jukebox.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>());
    for (name, path) in jukebox {
        let key = format!("../jukebox/{}.mp3", name.to_ascii_lowercase());
        music.files.insert(&key, None);
        pb.paths.insert(key, path);
    }
    info!("Combat Arms music: {} files in the install", music.files.len());
}

/// The files below `dir`, by their path from `prefix` (lower case).
fn walk(dir: &std::path::Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let key = normalize(&format!("{prefix}/{name}"));
        if p.is_dir() {
            walk(&p, &key, out);
        } else {
            out.push((key, p));
        }
    }
}

/// Both masters go to 0 while the window is minimized (it has no pixels).
fn minimized(window: Query<&Window, With<PrimaryWindow>>, mut volumes: ResMut<CaVolumes>) {
    let Ok(w) = window.single() else { return };
    let gone = w.physical_width() == 0 || w.physical_height() == 0;
    if volumes.minimized != gone {
        volumes.minimized = gone;
    }
}

fn bank_of<'a>(bank: &'a mut Option<ResMut<SoundBank>>) -> Option<&'a mut MapSounds> {
    bank.as_mut().map(|b| Arc::make_mut(&mut b.0))
}

/// The music's streams as live sounds.
#[allow(clippy::too_many_arguments)]
fn play_music(
    mut pb: ResMut<Playback>,
    mut music: ResMut<CaMusic>,
    volumes: Res<CaVolumes>,
    mut bank: Option<ResMut<SoundBank>>,
    mut control: MessageWriter<SoundControl>,
    mut commands: Commands,
) {
    if bank.is_none() {
        commands.insert_resource(SoundBank(Arc::new(MapSounds::default())));
        return;
    }
    if bank.as_ref().is_some_and(|b| b.is_added()) {
        // A new bank: our clips and sounds are gone with the old one.
        for (_, (key, _)) in pb.playing.drain() {
            control.write(SoundControl::Stop(key));
        }
        pb.clips.clear();
        pb.sound_files.clear();
        pb.ambient.clear();
    }
    let streams = music.mixed(volumes.music());
    let alive: Vec<(String, u64)> = streams.iter().map(|s| (s.file.clone(), s.started.to_bits())).collect();
    let gone: Vec<(String, u64)> = pb.playing.keys().filter(|k| !alive.contains(k)).cloned().collect();
    for k in gone {
        if let Some((key, _)) = pb.playing.remove(&k) {
            control.write(SoundControl::Stop(key));
        }
    }
    for s in streams {
        let id = (s.file.clone(), s.started.to_bits());
        if let Some((key, last)) = pb.playing.get_mut(&id) {
            if (*last - s.gain).abs() > 1e-3 {
                *last = s.gain;
                control.write(SoundControl::Change {
                    key: *key,
                    volume: Some(s.gain),
                    pitch: None,
                });
            }
            continue;
        }
        let file = normalize(&s.file);
        // Start decoding, or take the decoded clip.
        let state = pb.clips.remove(&file);
        let state = match state {
            None => match pb.paths.get(&file).cloned() {
                Some(path) => Clip::Loading(AsyncComputeTaskPool::get().spawn(async move {
                    let bytes = std::fs::read(&path).ok()?;
                    source_formats::wav::decode_any(&bytes).ok()
                })),
                None => match pb.mounted.get(&file).cloned() {
                    Some(path) => Clip::Loading(AsyncComputeTaskPool::get().spawn(async move {
                        let bytes = crate::game::world::mount()?.read(&path).ok()?;
                        source_formats::wav::decode_any(&bytes).ok()
                    })),
                    None => Clip::Failed,
                },
            },
            Some(Clip::Loading(mut task)) => match block_on(poll_once(&mut task)) {
                Some(Some(clip)) => {
                    let ms = clip.samples.len() as f64 / clip.channels.max(1) as f64 / clip.rate.max(1) as f64 * 1000.0;
                    if music.files.length_ms(&file).is_none() {
                        music.files.insert(&file, Some(ms));
                    }
                    let sounds = bank_of(&mut bank).expect("bank");
                    sounds.clips.push(clip);
                    Clip::Ready(sounds.clips.len() - 1)
                }
                Some(None) => {
                    warn!("Combat Arms music {file}: not readable");
                    Clip::Failed
                }
                None => Clip::Loading(task),
            },
            Some(other) => other,
        };
        if let Clip::Ready(idx) = &state {
            pb.next_key += 1;
            let key = SoundKey(MUSIC_KEYS | pb.next_key);
            control.write(SoundControl::Start(StartSound {
                key,
                entry: format!("ca:bgm:{file}|stream"),
                at: None,
                follow: None,
                volume: Some(s.gain),
                pitch: Some(100.0),
                level: Some(SoundLevel::Db(0.0)),
                clip: Some(*idx),
                looping: s.looping,
                dry: true,
            }));
            pb.playing.insert(id, (key, s.gain));
        }
        pb.clips.insert(file, state);
    }
}

/// The map's looping `SoundFX` objects as live sounds.
#[allow(clippy::too_many_arguments)]
fn play_ambient(
    mut pb: ResMut<Playback>,
    ambient: Res<CaAmbient>,
    objects: Res<CaObjects>,
    object_sounds: Res<crate::game::objects::sound::CaObjectSounds>,
    volumes: Res<CaVolumes>,
    data: Res<crate::game::data::CaData>,
    me: Query<&Transform, With<LocalPlayer>>,
    mut bank: Option<ResMut<SoundBank>>,
    mut control: MessageWriter<SoundControl>,
) {
    if bank.is_none() {
        return;
    }
    let scale = if objects.scale > 0.0 { objects.scale } else { UNIT };
    let listener = me.iter().next().map(|t| from_engine(t.translation, scale));
    let s_master = volumes.sound();
    let live: Vec<u32> = ambient.live().map(|s| s.id).collect();
    let gone: Vec<u32> = pb.ambient.keys().filter(|id| !live.contains(id)).copied().collect();
    for id in gone {
        pb.ambient.remove(&id);
        control.write(SoundControl::Stop(SoundKey(AMBIENT_KEYS | id as u64)));
    }
    let mount = data.mount();
    for s in ambient.live() {
        // An ambient sound is centred and falls off by its gain; a placed
        // one has Chimera's radius level to do that.
        let volume = ambient_volume(s, listener, s_master);
        if let Some(last) = pb.ambient.get_mut(&s.id) {
            if (*last - volume).abs() > 1e-3 {
                *last = volume;
                control.write(SoundControl::Change {
                    key: SoundKey(AMBIENT_KEYS | s.id as u64),
                    volume: Some(volume),
                    pitch: None,
                });
            }
            continue;
        }
        // A placed sound that follows its object (`PlayAttached`) waits
        // for the object's anchor (`objects::sound`).
        let follow = if s.attached && !s.ambient {
            let Some(e) = object_sounds.follow(&objects, s.id) else { continue };
            Some(e)
        } else {
            None
        };
        let Some(mount) = mount.as_deref() else { continue };
        let file = s.file.to_lowercase().replace('\\', "/");
        let clip = *pb.sound_files.entry(file.clone()).or_insert_with(|| {
            let clip = library::sound_clip(&file, mount)?;
            let sounds = bank_of(&mut bank)?;
            sounds.clips.push(clip);
            Some(sounds.clips.len() - 1)
        });
        let Some(clip) = clip else { continue };
        let (at, level) = if s.ambient {
            (None, SoundLevel::Db(0.0))
        } else {
            let inner = if s.inner < s.outer { s.inner } else { s.outer * 0.9999 };
            (
                Some(to_engine(s.at, scale)),
                rules::sound_level_radii(inner, s.outer, UNIT),
            )
        };
        control.write(SoundControl::Start(StartSound {
            key: SoundKey(AMBIENT_KEYS | s.id as u64),
            entry: format!("ca:amb:{file}"),
            at,
            follow,
            volume: Some(volume),
            pitch: Some(100.0),
            level: Some(level),
            clip: Some(clip),
            looping: true,
            dry: false,
        }));
        pb.ambient.insert(s.id, volume);
    }
}
