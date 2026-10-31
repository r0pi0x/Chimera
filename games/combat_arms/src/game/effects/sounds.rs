//! Playing Combat Arms' sounds (specs/combat_arms/effects_sounds.md 10):
//! every effect writes a `CaSound` (the archive file, local or 3D at a
//! point with inner and outer radius, volume 0..100); `play` turns it
//! into Chimera's `PlaySound` with a `SoundLevel::Radius` (the
//! squared-distance gain and the distance low-pass, `rules`), loading the
//! file from the install the first time (`library::sound_clip`) and
//! adding it to the map's `SoundBank`. Server sounds (hurt, grenade
//! bounces) go out as `GameSound`, which a network server also sends to
//! every client.
//!
//! The effects master (`music::mix::CaVolumes::sound`, music_ambience.md
//! 8) scales every sound here, a server sound by the machine that plays
//! it first (a client's own slider doesn't reach the sounds the server
//! sends: they arrive as Chimera's `GameSound`).
//!
//! Known shortcuts (docs/tech-debt.md): a 3D sound's gain and low-pass
//! are set when it starts (Chimera's one-shots don't follow a moving
//! listener); server sounds reach every client, not those within 2R
//! (`rules::server_sends`: beyond R they are silent anyway); priorities
//! aren't kept (none are used to pick) and there is no 255-voice cap
//! (Chimera's own per-source and per-entry voice limits instead); a
//! gun's round impacts and casing bounces each replace its previous
//! one (`CHANNEL_IMPACT`, `CHANNEL_SHELL`; the original lets them pile
//! up, which clipped our mix at full auto).

use std::collections::HashMap;

use bevy::prelude::*;

use super::{library, rules};
use crate::map::{
    GameSound, MapSoundEntry, PlaySound, SoundControl, SoundLevel, StartSound,
    sound::{Interval, SoundBank},
};

/// A sound to play.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaSound {
    /// The file inside the install's archives.
    pub file: String,
    /// Where (metres); None: local (2D, in your head).
    pub at: Option<Vec3>,
    /// Inner and outer radius (u) of a 3D sound.
    pub radii: (f32, f32),
    /// 0..100.
    pub volume: f32,
    /// Who makes it, and on which channel: a new sound replaces the one
    /// that source plays there.
    pub source: Option<Entity>,
    pub channel: Option<u8>,
    /// A server sound (sent to clients).
    pub server: bool,
    /// A falloff of Chimera's that replaces the radii's (the party's
    /// shared footstep profile, `map::sound::EvenFootsteps`).
    pub level: Option<SoundLevel>,
}

impl CaSound {
    /// A local sound (yours: fire, reload, footsteps, dings, interface).
    pub fn local(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            at: None,
            radii: (0.0, 0.0),
            volume: 100.0,
            source: None,
            channel: None,
            server: false,
            level: None,
        }
    }

    /// A 3D sound at `at` (metres) with outer radius R (u) and the inner
    /// radius 7 % of it.
    pub fn at(file: impl Into<String>, at: Vec3, big_r: f32) -> Self {
        Self {
            at: Some(at),
            radii: (rules::inner_radius(big_r), big_r),
            ..Self::local(file)
        }
    }

    pub fn volume(self, volume: f32) -> Self {
        Self { volume, ..self }
    }

    pub fn from_source(self, source: Entity, channel: u8) -> Self {
        Self {
            source: Some(source),
            channel: Some(channel),
            ..self
        }
    }

    pub fn server(self) -> Self {
        Self { server: true, ..self }
    }

    /// A 3D sound falling off by Chimera's `level` instead of its radii.
    pub fn with_level(self, level: SoundLevel) -> Self {
        Self {
            level: Some(level),
            ..self
        }
    }

    /// The bank entry name it plays as.
    pub fn entry(&self) -> String {
        match (self.at, self.level) {
            (None, _) => format!("ca:{}", self.file),
            (Some(_), Some(level)) => format!("ca:{}@{level:?}", self.file),
            (Some(_), None) => format!("ca:{}@{}:{}", self.file, self.radii.0, self.radii.1),
        }
    }

    /// How it falls off with distance as Chimera's mixer plays it: none
    /// for a local sound, else its `level` or its radii's
    /// (`rules::sound_level_radii`: the squared-distance gain and the
    /// distance low-pass).
    pub fn sound_level(&self) -> SoundLevel {
        match (self.at, self.level) {
            (None, _) => SoundLevel::Db(0.0),
            (Some(_), Some(level)) => level,
            (Some(_), None) => rules::sound_level_radii(self.radii.0, self.radii.1, super::super::inventory::UNIT),
        }
    }
}

/// Sound channels by source.
pub const CHANNEL_FIRE: u8 = 1;
pub const CHANNEL_GRENADE: u8 = 2;
/// Your local voices (`voice`): kill, sprint and idle.
pub const CHANNEL_KILL: u8 = 3;
pub const CHANNEL_DASH: u8 = 4;
pub const CHANNEL_IDLE: u8 = 5;
/// A gun's (the weapon entity's, not its holder's: those would be heard
/// as the listener's own) round impacts and casing bounces: each
/// round's replaces the last round's, so full-auto fire up close keeps
/// one of each sounding instead of a pile at almost full gain
/// (`rules::sound_level_radii`'s falloff is nearly flat near the
/// source). Not the original's (effects_sounds.md 10 sets no limit on
/// them); our mixer sums every voice and clips past full scale.
pub const CHANNEL_IMPACT: u8 = 6;
pub const CHANNEL_SHELL: u8 = 7;
/// A team's grenade call-out at its thrower (`grenades::callout`).
pub const CHANNEL_CALLOUT: u8 = 8;

/// The install's sound files loaded so far: clip index in the bank (None:
/// doesn't read).
#[derive(Resource, Default)]
pub struct CaSoundFiles(pub HashMap<String, Option<usize>>);

/// Play the frame's `CaSound`s.
pub fn play(
    mut sounds: MessageReader<CaSound>,
    bank: Option<ResMut<SoundBank>>,
    mut files: ResMut<CaSoundFiles>,
    data: Res<crate::game::data::CaData>,
    mut lengths: ResMut<super::CaSoundLengths>,
    volumes: Option<Res<crate::game::music::mix::CaVolumes>>,
    mut out: MessageWriter<PlaySound>,
    mut game: MessageWriter<GameSound>,
) {
    let Some(mut bank) = bank else {
        sounds.clear();
        return;
    };
    let mount = data.mount();
    if bank.is_added() {
        files.0.clear();
        // Server sounds arrive by entry name: every client has them ready
        // (their radii as the server sends them, `rules::server_radii`).
        if let Some(mount) = mount.as_deref() {
            for s in server_sounds(super::data::CaFxTables::built_in()) {
                ensure(&mut bank, &mut files, &mut lengths, mount, &s);
            }
        }
    }
    // The effects master (music_ambience.md 8): every effect, ambient
    // sound and interface sound shares this one slider.
    let master = volumes.map_or(1.0, |v| v.sound());
    for s in sounds.read() {
        let entry = s.entry();
        if bank.0.entry(&entry).is_none() {
            let Some(mount) = mount.as_deref() else { continue };
            if !ensure(&mut bank, &mut files, &mut lengths, mount, s) {
                continue;
            }
        }
        let play = PlaySound {
            entry,
            at: s.at,
            volume: Some((s.volume / 100.0 * master).clamp(0.0, 1.0)),
            pitch: None,
            source: s.source,
            channel: s.channel,
        };
        if s.server {
            game.write(GameSound(play));
        } else {
            out.write(play);
        }
    }
}

/// Make `s`'s bank entry, reading its file the first time (false: it
/// doesn't read).
fn ensure(
    bank: &mut SoundBank,
    files: &mut CaSoundFiles,
    lengths: &mut super::CaSoundLengths,
    mount: &crate::mount::Mount,
    s: &CaSound,
) -> bool {
    let entry = s.entry().to_lowercase();
    if bank.0.entry(&entry).is_some() {
        return true;
    }
    let clip = *files.0.entry(s.file.clone()).or_insert_with(|| {
        let clip = library::sound_clip(&s.file, mount)?;
        let frames = clip.samples.len() / clip.channels.max(1) as usize;
        lengths.0.insert(s.file.clone(), frames as f32 / clip.rate.max(1) as f32);
        let sounds = std::sync::Arc::make_mut(&mut bank.0);
        sounds.clips.push(clip);
        Some(sounds.clips.len() - 1)
    });
    let Some(clip) = clip else { return false };
    let level = s.sound_level();
    std::sync::Arc::make_mut(&mut bank.0).entries.insert(
        entry,
        MapSoundEntry {
            waves: vec![clip],
            volume: Interval::fixed(1.0),
            pitch: Interval::fixed(100.0),
            level,
            channel: 0,
            dry: false,
        },
    );
    true
}

/// The server sounds the tables name, as clients get them (radii
/// quantised): hurt sounds and grenade bounces.
pub fn server_sounds(t: &super::data::CaFxTables) -> Vec<CaSound> {
    let mut out = Vec::new();
    let s = &t.sounds;
    let hurt = rules::server_radii(rules::inner_radius(s.hurt_radius), s.hurt_radius);
    for f in [&s.guns, &s.explosions, &s.burning, &s.falling, &s.gas, &s.head, &s.groin]
        .into_iter()
        .flatten()
    {
        out.push(server_sound(f, Vec3::ZERO, hurt));
    }
    for surface in &t.surface {
        if !surface.grenade.is_empty() {
            let r = rules::server_radii(rules::inner_radius(surface.grenade_radius), surface.grenade_radius);
            out.push(server_sound(&surface.grenade, Vec3::ZERO, r));
        }
    }
    out
}

/// A server sound at `at` with radii `(r, R)` as clients get them.
pub fn server_sound(file: &str, at: Vec3, (r, big_r): (f32, f32)) -> CaSound {
    CaSound {
        radii: (r, big_r),
        ..CaSound::at(file, at, big_r).server()
    }
}

/// A projectile record's create sound (special_weapons.md Corrections
/// (2026-10-31) 3): a server sound at the shot's fire position `at`
/// (metres), radius `radius` (u), volume 100, once, when the record has
/// one and a radius, and the weapon's automatic throw (or mine) call-out
/// (`auto_throw`) doesn't stand in for it.
pub fn create_sound(file: &str, radius: f32, auto_throw: bool, at: Vec3) -> Option<CaSound> {
    (radius > 0.0 && !file.is_empty() && !auto_throw).then(|| server_sound(file, at, (rules::inner_radius(radius), radius)))
}

/// `create_sound` written to the world's sounds (the server's frame).
pub fn write_create_sound(world: &mut World, file: &str, radius: f32, auto_throw: bool, at: Vec3) {
    if let Some(s) = create_sound(file, radius, auto_throw, at)
        && let Some(mut m) = world.get_resource_mut::<Messages<CaSound>>()
    {
        m.write(s);
    }
}

/// Start and stop the long-lived looping sounds (`view_model::attached::
/// CaLoopSound`: the attached effects' sounds), each file read the first
/// time as `play` does, scaled by the effects master when started.
pub fn loops(
    mut loops: MessageReader<crate::game::view_model::attached::CaLoopSound>,
    bank: Option<ResMut<SoundBank>>,
    mut files: ResMut<CaSoundFiles>,
    data: Res<crate::game::data::CaData>,
    mut lengths: ResMut<super::CaSoundLengths>,
    volumes: Option<Res<crate::game::music::mix::CaVolumes>>,
    mut control: MessageWriter<SoundControl>,
) {
    let Some(mut bank) = bank else {
        loops.clear();
        return;
    };
    let mount = data.mount();
    let master = volumes.map_or(1.0, |v| v.sound());
    for l in loops.read() {
        let Some(s) = &l.sound else {
            control.write(SoundControl::Stop(l.key));
            continue;
        };
        let entry = s.entry();
        if bank.0.entry(&entry).is_none() {
            let Some(mount) = mount.as_deref() else { continue };
            if !ensure(&mut bank, &mut files, &mut lengths, mount, s) {
                continue;
            }
        }
        control.write(SoundControl::Start(StartSound {
            key: l.key,
            entry,
            at: s.at,
            follow: if s.at.is_some() { l.follow } else { None },
            volume: Some((s.volume / 100.0 * master).clamp(0.0, 1.0)),
            looping: !l.once,
            ..default()
        }));
    }
}
