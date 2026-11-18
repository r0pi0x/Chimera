//! Your own voice sounds (effects_sounds.md section 11, Corrections
//! 2026-10-22, 8): the client's local auto-cast entries played in your
//! head, in your model's sound template. Beside the jump voice
//! (`footsteps`) these are:
//!
//! - the **kill voice**: when a kill names you as the killer and the
//!   killing hit was not a groin hit (no check for suicide or team kill),
//!   one file of the kill entry, unless the previous kill voice is still
//!   playing;
//! - the **sprint** sound: while you sprint, the dash entry starts again
//!   as soon as the previous one has ended;
//! - the **idle** sound: while your control flags are exactly "run" (no
//!   movement, jump, crouch or walk key) a 5 s timer runs; when it expires
//!   the idle entry plays and then repeats back to back; any other flag
//!   stops the timer and kills the playing idle sound;
//! - the **heartbeat** (`ui::hud`, the low-health beat of hud_crosshair.md
//!   D2): a file of the heartbeat entry.
//!
//! The entries' files are `EventSounds::kill`, `dash`, `idle` and
//! `heartbeat` (data/effects.toml); an empty entry is silent (and stops the
//! idle timer). The pure state is `Voices`, so tests can drive it.
//!
//! Known shortcuts: killing the idle sound replaces it with a silent copy
//! on its channel (Chimera's one-shots have no stop); a clip's length is
//! taken as `VOICE_DEFAULT_LENGTH` until it has been loaded once; whether
//! holding fire sets a movement flag (and so also stops the idle sound) is
//! open (Open question 29): it doesn't here; "movement is allowed" is not
//! checked.

use bevy::prelude::*;

use super::{
    CaSoundLengths, Rolls,
    data::CaFxTables,
    rules,
    sounds::{CHANNEL_DASH, CHANNEL_IDLE, CHANNEL_KILL, CaSound},
};
use crate::{
    core::{Died, Health, Hitgroup, Intent, LocalPlayer, MovementState},
    slots::GameIdentity,
};

/// The idle timer (s).
pub const IDLE_AFTER: f32 = 5.0;
/// The length (s) a voice clip is taken to play when it isn't loaded.
pub const VOICE_DEFAULT_LENGTH: f32 = 1.0;

/// What the idle sound should do this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleAction {
    Nothing,
    /// Start the idle entry (again).
    Play,
    /// Kill the playing idle sound.
    Stop,
}

/// The local voices' timers.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Voices {
    /// Seconds with the flags exactly "run".
    pub idle_for: f32,
    /// When the idle sound ends (None: not playing).
    pub idle_until: Option<f64>,
    /// When the last dash sound and kill voice end.
    pub dash_until: f64,
    pub kill_until: f64,
}

impl Voices {
    /// One frame of the idle sound: `idle` the flags are exactly "run",
    /// `has_entry` the entry has a file.
    pub fn idle(&mut self, idle: bool, has_entry: bool, dt: f32, now: f64, length: f32) -> IdleAction {
        if !idle || !has_entry {
            self.idle_for = 0.0;
            return if self.idle_until.take().is_some() { IdleAction::Stop } else { IdleAction::Nothing };
        }
        match self.idle_until {
            Some(until) if now < until => IdleAction::Nothing,
            Some(_) => {
                self.idle_until = Some(now + length as f64);
                IdleAction::Play
            }
            None => {
                self.idle_for += dt;
                if self.idle_for + 1e-4 >= IDLE_AFTER {
                    self.idle_until = Some(now + length as f64);
                    IdleAction::Play
                } else {
                    IdleAction::Nothing
                }
            }
        }
    }

    /// The sprint sound: starts again once the previous one ended.
    pub fn dash(&mut self, sprinting: bool, now: f64, length: f32) -> bool {
        if sprinting && now >= self.dash_until {
            self.dash_until = now + length as f64;
            return true;
        }
        false
    }

    /// A kill by you with `zone` the killing hit: the kill voice plays
    /// unless a groin hit or the previous one still plays.
    pub fn kill(&mut self, zone: Hitgroup, now: f64, length: f32) -> bool {
        // The groin's zone (`effects::hurt_kinds`).
        if zone == Hitgroup::Stomach || now < self.kill_until {
            return false;
        }
        self.kill_until = now + length as f64;
        true
    }
}

/// The control flags are exactly "run": no movement, jump, crouch, walk
/// or sprint key.
pub fn flags_idle(i: &Intent) -> bool {
    i.move_axis == Vec2::ZERO && !i.opposed.any() && !i.jump && !i.crouch && !i.walk && !i.sprint
}

fn length_of(lengths: &CaSoundLengths, file: &str) -> f32 {
    lengths.0.get(file).copied().unwrap_or(VOICE_DEFAULT_LENGTH)
}

/// Play your kill, sprint and idle voices.
#[allow(clippy::too_many_arguments)]
pub fn voices(
    time: Res<Time>,
    mut died: MessageReader<Died>,
    mut me: Query<
        (Entity, &Intent, Option<&MovementState>, Option<&Health>, Option<&mut Voices>, Option<&GameIdentity>),
        With<LocalPlayer>,
    >,
    tables: Res<CaFxTables>,
    lengths: Res<CaSoundLengths>,
    weapons: Option<Res<crate::weapon::DamageWeapons>>,
    mut sounds: MessageWriter<CaSound>,
    mut rolls: Local<Rolls>,
    mut commands: Commands,
) {
    let mine = me.single().ok().map(|m| m.0);
    let identity_now = me.single().ok().and_then(|m| m.5).map(|i| i.0.as_str());
    // Your kills, with the game each belongs to (the killing weapon's, not
    // what you hold by now: a rung's swap may have changed your identity).
    let kills: Vec<(Hitgroup, bool)> = died
        .read()
        .filter(|d| d.attacker.is_some() && d.attacker == mine)
        .map(|d| {
            let game = crate::weapon::killing_game(&d.damage, weapons.as_deref(), identity_now);
            (d.damage.hitgroup, game.is_none_or(|g| g.eq_ignore_ascii_case(crate::game::GAME)))
        })
        .collect();
    let (dt, now) = (time.delta_secs(), time.elapsed_secs_f64());
    let Ok((e, intent, state, health, v, identity)) = me.single_mut() else { return };
    let Some(mut v) = v else {
        commands.entity(e).insert(Voices::default());
        return;
    };
    // Another game's identity (the party apps' `identity = "weapon"`)
    // speaks as that game's: none of ours (a playing idle sound stops).
    let ours = GameIdentity::allows(identity, crate::game::GAME);
    let alive = ours && health.is_none_or(|h| h.current > 0.0);
    let s = &tables.sounds;
    // Only a kill of this game's own (a Combat Arms gun) has its voice: a
    // kill with another game's gun is that game's, and is silent here.
    for zone in kills.into_iter().filter(|(_, own)| *own).map(|(zone, _)| zone) {
        if let Some(f) = rules::draw_any(&s.kill, rolls.next())
            && v.kill(zone, now, length_of(&lengths, f))
        {
            sounds.write(CaSound::local(f).from_source(e, CHANNEL_KILL));
        }
    }
    let sprinting = alive && state.is_some_and(|m| m.sprinting);
    if let Some(f) = rules::draw_any(&s.dash, rolls.next())
        && v.dash(sprinting, now, length_of(&lengths, f))
    {
        sounds.write(CaSound::local(f).from_source(e, CHANNEL_DASH));
    }
    let file = rules::draw_any(&s.idle, rolls.next());
    let length = file.map_or(VOICE_DEFAULT_LENGTH, |f| length_of(&lengths, f));
    match v.idle(alive && flags_idle(intent), file.is_some(), dt, now, length) {
        IdleAction::Play => {
            if let Some(f) = file {
                sounds.write(CaSound::local(f).from_source(e, CHANNEL_IDLE));
            }
        }
        IdleAction::Stop => {
            // Kill the playing sound: a silent one replaces it.
            if let Some(f) = s.idle.first() {
                sounds.write(CaSound::local(f).volume(0.0).from_source(e, CHANNEL_IDLE));
            }
        }
        IdleAction::Nothing => {}
    }
}
