//! The options' settings handed to the shell (specs/combat_arms/
//! options.md M2, G3, S2): what `game::options` works out (`CaMouseLook`,
//! `CaSoundLevels`, `CaGamma`) becomes the shell's mouse look, mixer
//! volumes and brightness.
//!
//! - Mouse: one count turns the unzoomed view by `1.5 k(S)` rad, which is
//!   the shell's `sensitivity * m_yaw` degrees with `sensitivity` 1; the
//!   zoom divisor is the weapon's own (`Zoomed::sensitivity`,
//!   `game::weapons::zoom`); invert is a negative `m_pitch`.
//! - Brightness: gamma `g` is the shell's `mat_monitorgamma` 2.2 / `g`
//!   (it raises the frame to `mat_monitorgamma / 2.2`), which the shell
//!   clamps to 1.6 to 2.6.
//! - Sound: the effects master is the shell's master volume; the music
//!   stream's own level is divided by it so music keeps its own level
//!   (and is silent while the master is 0).
//!
//! An app whose shell keeps these settings in its own cvars (Lucker
//! Party: `sensitivity`, `volume`, `snd_musicvolume`, `mat_monitorgamma`
//! in config.cfg, shared by every game it plays) says so with
//! `ShellOptions`: then nothing here touches them. Combat Arms' profile
//! isn't loaded there, and its levels (`soundvolume` 0 among them) had
//! set the master volume and the brightness back at every start.
//!
//! Known shortcuts: see `game::options::sound` (the sounds' own levels
//! per class aren't applied: the sounds carry no class, so only the
//! master reaches the mixer, and a level above 1 is clamped); the shell
//! clamps the brightness range.

use bevy::prelude::*;
use chimera_present::audio::live_sound::{MAX_GAIN, SoundMix};

use crate::{
    client::{audio::SoundOptions, gamma::MonitorGammaSetting, input::MouseSettings},
    game::options::{CaGamma, CaMouseLook, CaSoundLevels},
};

/// The shell's `mat_monitorgamma` for a gamma.
pub fn monitor_gamma(gamma: f64) -> f32 {
    (crate::client::gamma::DEFAULT_GAMMA as f64 / gamma.max(0.01)) as f32
}

/// The mouse look's degrees per count for the shell (M2).
pub fn degrees_per_count(look: &CaMouseLook) -> f32 {
    look.rad_per_count().to_degrees() as f32
}

/// An app that keeps the mouse look (`sensitivity`, `m_yaw`,
/// `m_pitch`), the volumes (`volume`, `snd_musicvolume`) and the
/// brightness (`mat_monitorgamma`) in the shell's cvars (saved in its
/// config.cfg, shared by the games it plays: Lucker Party) takes them out
/// of Combat Arms' profile: `apply` leaves `MouseSettings`,
/// `SoundOptions`, `SoundMix` and `MonitorGammaSetting` to the shell.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct ShellOptions;

#[allow(clippy::too_many_arguments)]
fn apply(
    shell: Option<Res<ShellOptions>>,
    look: Res<CaMouseLook>,
    levels: Res<CaSoundLevels>,
    gamma: Res<CaGamma>,
    mouse: Option<ResMut<MouseSettings>>,
    sound: Option<ResMut<SoundOptions>>,
    mix: Option<ResMut<SoundMix>>,
    monitor: Option<ResMut<MonitorGammaSetting>>,
) {
    if shell.is_some() {
        return;
    }
    if let Some(mut m) = mouse
        && (look.is_changed() || m.is_added())
    {
        let d = degrees_per_count(&look);
        let next = MouseSettings {
            sensitivity: 1.0,
            m_yaw: d,
            m_pitch: if look.invert { -d } else { d },
            ..*m
        };
        m.set_if_neq(next);
    }
    if levels.is_changed() {
        let master = levels.master.clamp(0.0, 1.0);
        if let Some(mut s) = sound {
            s.volume = master;
        }
        if let Some(mut m) = mix {
            m.music = if master > 0.0 { (levels.music / master).clamp(0.0, MAX_GAIN) } else { 0.0 };
        }
    }
    if let Some(mut m) = monitor
        && gamma.is_changed()
    {
        m.0 = monitor_gamma(gamma.0);
    }
}

pub struct CaOptionsApplyPlugin;

impl Plugin for CaOptionsApplyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply.after(crate::game::options::derive::update));
    }
}
