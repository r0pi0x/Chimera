//! What the settings mean for the game, with no drawing: the mouse look's
//! numbers (`CaMouseLook`), the sound levels (`CaSoundLevels`), the
//! brightness (`CaGamma`), the options with no page (`CaGameOptions`),
//! and the keys that act in a match: the sensitivity keys with their chat
//! line (M3), the HUD on/off key and the help key (H). `ui::options`
//! hands the first three to the shell.

use bevy::prelude::*;

use super::{
    CaOptions, mouse,
    settings::{self, CaSettings},
    sound::{self, CaSoundLevels, Multipliers},
    video::{self, CaMachine},
};
use crate::console::ConsoleAppExt;

/// The mouse look (M2): sensitivity S, its axis scale and the invert
/// switch.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CaMouseLook {
    pub sensitivity: i32,
    pub invert: bool,
}

impl Default for CaMouseLook {
    fn default() -> Self {
        Self {
            sensitivity: mouse::SENS_DEFAULT,
            invert: false,
        }
    }
}

impl CaMouseLook {
    /// `k(S)`, rad per count.
    pub fn scale(&self) -> f64 {
        mouse::axis_scale(self.sensitivity as f64)
    }

    /// The turn one count gives unzoomed, rad (`3 k` times the final
    /// half).
    pub fn rad_per_count(&self) -> f64 {
        mouse::LOOK_HALF * mouse::NO_SMOOTHING_GAIN * self.scale()
    }

    /// One frame's turn (M2): `(yaw, pitch)`.
    pub fn delta(&self, counts: (f64, f64), zoom_level: u8, blocked: bool) -> (f64, f64) {
        mouse::view_delta(counts, self.sensitivity as f64, zoom_level, self.invert, blocked)
    }
}

/// The brightness (G3): gamma, with 1.0 unchanged and above 1 brighter.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CaGamma(pub f64);

impl Default for CaGamma {
    fn default() -> Self {
        Self(video::GAMMA_DEFAULT)
    }
}

impl CaGamma {
    /// The display ramp for one channel (G3).
    pub fn ramp(&self) -> [u16; 256] {
        video::ramp(self.0)
    }
}

/// The options with no page (H).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CaGameOptions {
    pub always_run: bool,
    pub auto_weapon_switch: bool,
    pub weapon_sway: f32,
    pub message_duration: f32,
    pub hud_layout: i32,
    pub light_map: bool,
    pub disable_hardware_sound: bool,
}

impl Default for CaGameOptions {
    fn default() -> Self {
        Self {
            always_run: true,
            auto_weapon_switch: true,
            weapon_sway: 0.0,
            message_duration: 0.0,
            hud_layout: 0,
            light_map: true,
            disable_hardware_sound: true,
        }
    }
}

/// The HUD on/off key (H): a flag that starts on at game start and is
/// kept until the game exits (not saved, not reset between matches). Off
/// hides every layout-built HUD panel; the crosshair, damage overlays and
/// scope (code-drawn), the scoreboard and popups keep drawing. A server
/// message can hide the whole HUD on its own.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct CaHudSwitch {
    pub on: bool,
    pub server_hidden: bool,
}

impl Default for CaHudSwitch {
    fn default() -> Self {
        Self {
            on: true,
            server_hidden: false,
        }
    }
}

impl CaHudSwitch {
    /// Whether the layout-built panels are drawn.
    pub fn panels(&self) -> bool {
        self.on && !self.server_hidden
    }
}

/// The help popup (`HelpScreen`), toggled except for game masters.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HelpPopup {
    pub open: bool,
}

/// A line for the chat panel the options ask for: the sensitivity change
/// (M3).
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub id: &'static str,
    pub text: String,
    /// The HUD layout attributes' colour it uses (`mouse_sensitivity`).
    pub colour: &'static str,
    /// Real time it appeared and how long it shows, ms.
    pub born_ms: f64,
    pub life_ms: f64,
}

/// The lines the options put in the chat.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaNotices {
    pub lines: Vec<Notice>,
}

impl CaNotices {
    /// Lines still showing at `now_ms`.
    pub fn showing(&self, now_ms: f64) -> Vec<&Notice> {
        self.lines.iter().filter(|n| now_ms - n.born_ms <= n.life_ms).collect()
    }
}

/// Key picks of the radio and emote menus (`choose_1`..`choose_6`),
/// recorded for those menus when they exist.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct CaChoose(pub Vec<u8>);

/// Recompute the derived values from the settings.
pub fn update(
    settings: Res<CaSettings>,
    machine: Res<CaMachine>,
    mut look: ResMut<CaMouseLook>,
    mut levels: ResMut<CaSoundLevels>,
    mut gamma: ResMut<CaGamma>,
    mut game: ResMut<CaGameOptions>,
) {
    if !settings.is_changed() && !machine.is_changed() {
        return;
    }
    look.set_if_neq(CaMouseLook {
        sensitivity: settings.int("MouseSensitivity"),
        invert: settings.int("MouseInvertY") != 0,
    });
    let m = Multipliers {
        weapons: settings.float("weaponssoundmultiplier") as f32,
        speech: settings.float("speechsoundmultiplier") as f32,
        default: settings.float("defaultsoundmultiplier") as f32,
    };
    let stored = settings.int("soundvolume");
    levels.set_if_neq(CaSoundLevels {
        master: sound::master(settings.bool("enablesound"), stored),
        music: sound::music_level(settings.bool("enablemusic") && machine.music, settings.int("musicvolume")),
        multipliers: m,
        stored,
        mic: settings.int("MicVolume"),
        speaker: settings.int("SpeakerVolume"),
    });
    gamma.set_if_neq(CaGamma(settings.float("GammaR") as f32 as f64));
    game.set_if_neq(CaGameOptions {
        always_run: settings.bool("AlwaysRun"),
        auto_weapon_switch: settings.bool("AutoWeaponSwitch"),
        weapon_sway: settings.float("WeaponSway") as f32,
        message_duration: settings.float("MessageDuration") as f32,
        hud_layout: settings.int("HUDLayout"),
        light_map: settings.bool("LightMap"),
        disable_hardware_sound: settings.bool("DisableHardwareSound"),
    });
}

/// M3: a sensitivity key was pressed (`inc`: `SensitivityInc`). S moves
/// within 1 to 99 and applies at once; the chat line shows even when S
/// didn't change. The value reaches the profile at its next save.
pub fn sensitivity_key(world: &mut World, inc: bool) {
    let table = world.resource::<CaOptions>().table.clone();
    let s = settings::get(world, &table, "MouseSensitivity").and_then(|v| v.parse::<i32>().ok()).unwrap_or(mouse::SENS_DEFAULT);
    let next = mouse::sensitivity_key(s, inc);
    settings::set(world, &table, "MouseSensitivity", &next.to_string());
    // The screen's pending value, if it's open, follows.
    world.resource_mut::<CaOptions>().pending.remove("mousesensitivity");
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0);
    world.resource_mut::<CaNotices>().lines.push(Notice {
        id: "sensitivity",
        text: format!("Sensitivity changed to {next}"),
        colour: "mouse_sensitivity",
        born_ms: now,
        life_ms: mouse::SENS_MESSAGE_TIME,
    });
}

pub fn register(app: &mut App) {
    app.init_resource::<CaChoose>();
    app.console_command("combat_arms:sensitivityinc", "Mouse sensitivity up one (1 to 99), with a chat line.", |w, _| {
        sensitivity_key(w, true);
        Ok(None)
    })
    .console_command("combat_arms:sensitivitydec", "Mouse sensitivity down one (1 to 99), with a chat line.", |w, _| {
        sensitivity_key(w, false);
        Ok(None)
    })
    .console_command("combat_arms:hudonoff", "Hide or show the HUD's panels (kept until the game exits).", |w, _| {
        let mut h = w.resource_mut::<CaHudSwitch>();
        h.on = !h.on;
        Ok(None)
    })
    .console_command("combat_arms:helpscreen", "Toggle the help popup (not for game masters).", |w, _| {
        if !w.resource::<super::CaAccount>().game_master {
            let mut p = w.resource_mut::<HelpPopup>();
            p.open = !p.open;
        }
        Ok(None)
    });
    for n in 1u8..=6 {
        // Its profile binds them again at every start; other apps sharing
        // config.cfg have no such command.
        app.world_mut().get_resource_or_init::<crate::console::Console>().bound_for_session(&format!("choose_{n}"));
        app.console_command(&format!("combat_arms:choose_{n}"), "A menu line pick (keys 1 to 6).", move |w, _| {
            w.resource_mut::<CaChoose>().0.push(n);
            Ok(None)
        });
    }
}
