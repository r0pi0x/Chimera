//! The options as data (specs/combat_arms/options.md M1, G1, S1, H):
//! one `Setting` per console variable, naming the profile place it
//! lives in (section and key), its type, its code default (what a
//! profile without the key gives), the page that shows it, when a change
//! applies, and how its profile value converts to the variable's. The
//! quality presets' rows (`presets.toml`) are settings too, on the
//! Graphic page. Console variables are registered from the table
//! (`register`); `CaSettings` holds their values.
//!
//! `ext` settings are variables another subsystem already owns
//! (`radarscale`, `rotateradar`: `game::radar`); the options read and
//! write them through the console.

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::video;
use crate::{
    console::{Console, ConsoleAppExt},
    profile::{self, Profile},
};

/// A value's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Int,
    Float,
    /// Written `TRUE`/`FALSE`; the variable holds 0 or 1.
    Bool,
}

/// When a change applies (the spec's "Applies" columns).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applies {
    /// While the control moves (a preview; Cancel undoes it).
    Now,
    /// On OK.
    Ok,
    /// When the next match starts.
    NextMatch,
    /// At the next start of the game.
    Restart,
    /// On the next entry into a world.
    NextMap,
}

/// The options screen's tabs, and the settings no page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Graphic,
    Sound,
    Control,
    Other,
}

/// The tabs in the order OK writes them.
pub const WRITE_ORDER: [Tab; 4] = [Tab::Graphic, Tab::Sound, Tab::Control, Tab::Other];

/// The page a setting is shown on; None: no page (H).
pub type Page = Option<Tab>;

/// How the profile's value becomes the variable's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conv {
    Same,
    /// The variable is the stored value / 10.
    Tenths,
    /// The variable is 1 minus the stored value.
    OneMinus,
}

/// One option.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub cvar: String,
    pub section: String,
    pub key: String,
    pub ty: Ty,
    /// What a missing key gives, in the profile's form ("" none: keep the
    /// variable's current value).
    pub default: String,
    pub page: Page,
    pub applies: Applies,
    pub conv: Conv,
    /// Owned by another subsystem's console variable.
    pub ext: bool,
    /// The quality preset row this is (index into `presets()`).
    pub preset: Option<usize>,
    pub help: String,
}

#[allow(clippy::too_many_arguments)]
fn s(
    cvar: &str,
    section: &str,
    key: &str,
    ty: Ty,
    default: &str,
    page: Page,
    applies: Applies,
    help: &str,
) -> Setting {
    Setting {
        cvar: cvar.into(),
        section: section.into(),
        key: key.into(),
        ty,
        default: default.into(),
        page,
        applies,
        conv: Conv::Same,
        ext: false,
        preset: None,
        help: help.into(),
    }
}

fn conv(mut x: Setting, c: Conv) -> Setting {
    x.conv = c;
    x
}

fn ext(mut x: Setting) -> Setting {
    x.ext = true;
    x
}

/// The table: M1, G1 (but the preset rows), S1 and H.
pub fn base() -> Vec<Setting> {
    use Applies::{Now, Ok as AtOk, NextMap, NextMatch};
    use Ty::{Bool, Float, Int};
    let g = Some(Tab::Graphic);
    let snd = Some(Tab::Sound);
    let c = Some(Tab::Control);
    vec![
        // M1: the mouse.
        s("MouseSensitivity", "Controls", "Sensitivity", Int, "5", c, AtOk, "Mouse sensitivity, 0 to 100."),
        s("MouseInvertY", "Controls", "InvertMouse", Int, "0", c, AtOk, "1: looking up and down is inverted."),
        ext(s("rotateradar", "Controls", "RotateRadar", Int, "0", c, NextMap, "1: the radar turns with the view.")),
        // G1: display.
        s("windowed", "Display", "windowed", Int, "0", g, Now, "1: a window, not full screen (window-mode builds)."),
        s("ScreenWidthSetting", "Display", "ScreenWidth", Int, "1024", g, NextMatch, "The match's screen width."),
        s("ScreenHeightSetting", "Display", "ScreenHeight", Int, "768", g, NextMatch, "The match's screen height."),
        s("ScreenDepthSetting", "Display", "ScreenBitDepth", Int, "32", g, NextMatch, "The match's colour depth, 16 or 32."),
        s("GammaR", "Display", "ScreenGamma", Float, "1.22", g, Now, "Brightness (gamma): 0.5 to 6 on the slider."),
        s("AntiAliasFSOverSample", "Display", "AntiAlias", Int, "0", g, NextMatch, "Anti-aliasing samples, 0 off."),
        s("HardwareCursor", "Display", "HardwareCursor", Bool, "FALSE", None, AtOk, "The hardware cursor."),
        s("VSyncOnFlip", "Display", "VSync", Bool, "FALSE", None, AtOk, "Wait for the vertical sync."),
        ext(s("radarscale", "Display", "RadarScale", Float, "1.0", None, AtOk, "The radar's zoom.")),
        s("PerformanceSetting", "Settings", "PerformanceSetting", Int, "0", g, AtOk, "Quality: 0 Low, 1 Mid, 2 High, 3 Custom."),
        s("Gore", "Game", "Gore", Int, "0", g, AtOk, "1: blood."),
        // S1: sound and voice.
        s("enablemusic", "Sound", "musicenable", Bool, "TRUE", snd, Now, "Music on."),
        s("musicvolume", "Sound", "musicvolume", Int, "0", snd, Now, "Music volume, 0 to 100."),
        s("bgmrotation", "Sound", "bgmrotation", Bool, "TRUE", snd, Now, "Music plays in order, not random."),
        s("enablesound", "Sound", "soundenable", Bool, "TRUE", snd, Now, "Effects on."),
        s("soundvolume", "Sound", "soundvolume", Int, "0", snd, Now, "Effects volume, 0 to 100."),
        s("EnableVoiceChat", "Sound", "EnableVoiceChat", Bool, "FALSE", snd, Now, "Voice chat on."),
        s("SoundAdjustVolume", "Sound", "SoundAdjustVolume", Int, "0", snd, AtOk, "100: game sound is lowered during voice."),
        s("MicVolume", "Sound", "MicVolume", Int, "0", snd, Now, "Microphone volume, 0 to 100."),
        s("MicDeviceIndex", "Sound", "MicDeviceIndex", Int, "0", snd, Now, "Microphone device."),
        s("SpeakerVolume", "Sound", "SpeakerVolume", Int, "0", snd, Now, "Speaker volume, 0 to 100."),
        s("SpeakerDeviceIndex", "Sound", "SpeakerDeviceIndex", Int, "0", snd, Now, "Speaker device."),
        s("weaponssoundmultiplier", "Sound", "weaponssoundmultiplier", Float, "1.0", None, AtOk, "The weapons sound class's multiplier."),
        s("speechsoundmultiplier", "Sound", "speechsoundmultiplier", Float, "1.0", None, AtOk, "The speech sound class's multiplier."),
        s("defaultsoundmultiplier", "Sound", "defaultsoundmultiplier", Float, "0.75", None, AtOk, "The default sound class's multiplier."),
        // H: options with no page.
        s("AlwaysRun", "Game", "AlwaysRun", Int, "1", None, AtOk, "1: run is the default, walk slows."),
        s("AutoWeaponSwitch", "Game", "AutoWeaponSwitch", Int, "1", None, AtOk, "1: switch to a picked-up weapon."),
        conv(s("WeaponSway", "Game", "WeaponSway", Float, "0", None, AtOk, "Weapon sway multiplier (stored x10)."), Conv::Tenths),
        conv(s("MessageDuration", "Game", "MessageDur", Float, "0", None, AtOk, "HUD message duration multiplier (stored x10)."), Conv::Tenths),
        s("HUDLayout", "Game", "HUDLayout", Int, "0", None, AtOk, "The HUD layout's index."),
        s("LightMap", "Game", "LightMap", Int, "1", None, AtOk, "1: light maps."),
        conv(s("DisableHardwareSound", "Game", "DirectSound", Int, "0", None, AtOk, "1: hardware sound mixing off."), Conv::OneMinus),
    ]
}

/// The whole table: `base` and the presets' rows.
pub fn all() -> Vec<Setting> {
    let mut t = base();
    for (i, r) in video::presets().iter().enumerate() {
        // The custom block's rows are the Graphic page's own switches;
        // the others are the preset's alone.
        let mut x = s(&r.cvar, "Settings", &r.name, Ty::Int, "", Some(Tab::Graphic), Applies::Ok, "A quality setting.");
        x.preset = Some(i);
        t.push(x);
    }
    t
}

impl Setting {
    /// The variable's text for a profile value's text (the stored form).
    pub fn to_cvar(&self, stored: &str) -> String {
        let n = stored.trim().trim_end_matches(['f', 'F']);
        let float = || n.parse::<f64>().ok();
        match (self.ty, self.conv) {
            (Ty::Bool, _) => match stored.trim().to_ascii_uppercase().as_str() {
                "TRUE" => "1".into(),
                "FALSE" => "0".into(),
                o => if o.parse::<f64>().unwrap_or(0.0) != 0.0 { "1" } else { "0" }.into(),
            },
            (_, Conv::Tenths) => float().map_or_else(|| "0".into(), |v| format!("{}", (v / 10.0) as f32)),
            (_, Conv::OneMinus) => float().map_or_else(|| "0".into(), |v| format!("{}", 1 - v as i32)),
            (Ty::Int, _) => float().map_or_else(|| "0".into(), |v| format!("{}", v as i32)),
            (Ty::Float, _) => float().map_or_else(|| "0".into(), |v| format!("{}", v as f32)),
        }
    }

    /// The profile's text for the variable's value.
    pub fn to_stored(&self, cvar: &str) -> String {
        let v = cvar.trim().parse::<f64>().unwrap_or(0.0);
        match (self.ty, self.conv) {
            (Ty::Bool, _) => profile::boolean(v != 0.0),
            (_, Conv::Tenths) => format!("{}", (v * 10.0).round() as i32),
            (_, Conv::OneMinus) => format!("{}", 1 - v as i32),
            (Ty::Int, _) => format!("{}", v as i32),
            (Ty::Float, _) => format!("{}", v as f32),
        }
    }

    /// The variable's value in a profile: its key's, else the code default
    /// (None: keep the current value).
    pub fn from_profile(&self, p: &Profile) -> Option<String> {
        match p.raw(&self.section, &self.key) {
            Some(raw) => Some(self.to_cvar(&raw)),
            None if self.default.is_empty() => None,
            None => Some(self.to_cvar(&self.default)),
        }
    }
}

/// The values of the options' own variables (the live, "console"
/// values), by variable name in lower case, in the variable's text.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct CaSettings {
    pub values: BTreeMap<String, String>,
}

impl CaSettings {
    /// A table's code defaults.
    pub fn defaults(table: &[Setting]) -> Self {
        let mut values = BTreeMap::new();
        for x in table.iter().filter(|x| !x.ext) {
            let v = if x.default.is_empty() { "0".to_string() } else { x.to_cvar(&x.default) };
            values.insert(x.cvar.to_lowercase(), v);
        }
        // The preset rows start at Low.
        for (x, r) in table.iter().filter_map(|x| Some((x, video::presets().get(x.preset?)?.clone()))) {
            values.insert(x.cvar.to_lowercase(), r.low.to_string());
        }
        values.insert("restoredefaults".to_string(), "0".to_string());
        Self { values }
    }

    pub fn get(&self, cvar: &str) -> Option<&str> {
        self.values.get(&cvar.to_lowercase()).map(String::as_str)
    }

    pub fn set(&mut self, cvar: &str, v: &str) {
        self.values.insert(cvar.to_lowercase(), v.trim().to_string());
    }

    pub fn float(&self, cvar: &str) -> f64 {
        self.get(cvar).and_then(|v| v.parse().ok()).unwrap_or(0.0)
    }

    pub fn int(&self, cvar: &str) -> i32 {
        self.float(cvar) as i32
    }

    pub fn bool(&self, cvar: &str) -> bool {
        self.float(cvar) != 0.0
    }
}

/// Register the table's console variables (the ones the options own).
pub fn register(app: &mut App, table: &[Setting]) {
    app.console_cvar(
        "combat_arms:RestoreDefaults",
        "1: the next profile load restores every section from the default profile (then 0).",
        "0",
        |w| w.get_resource::<CaSettings>().and_then(|s| s.get("restoredefaults").map(str::to_string)),
        |w, v| {
            w.get_resource_mut::<CaSettings>().ok_or("not available")?.set("restoredefaults", v);
            Ok(())
        },
    );
    for x in table.iter().filter(|x| !x.ext) {
        let (g, st) = (x.cvar.to_lowercase(), x.cvar.to_lowercase());
        let ty = x.ty;
        let default = if x.default.is_empty() { "0".to_string() } else { x.to_cvar(&x.default) };
        app.console_cvar(
            &format!("combat_arms:{}", x.cvar),
            &x.help,
            &default,
            move |w| w.get_resource::<CaSettings>().and_then(|s| s.get(&g).map(str::to_string)),
            move |w, v| {
                let t = v.trim();
                let ok = match ty {
                    Ty::Float => t.parse::<f32>().is_ok(),
                    _ => t.parse::<f32>().is_ok() || t.parse::<i64>().is_ok(),
                };
                if !ok {
                    return Err(format!("bad value \"{v}\""));
                }
                w.get_resource_mut::<CaSettings>().ok_or("not available")?.set(&st, t);
                Ok(())
            },
        );
    }
}

/// A variable's value, through its owner (the settings' own, or the
/// console variable of an `ext` one).
pub fn get(world: &mut World, table: &[Setting], cvar: &str) -> Option<String> {
    let ext = table.iter().find(|x| x.cvar.eq_ignore_ascii_case(cvar)).is_some_and(|x| x.ext);
    if ext {
        let f = world.resource::<Console>().cvar(cvar)?.get.clone();
        f(world)
    } else {
        world.resource::<CaSettings>().get(cvar).map(str::to_string)
    }
}

/// Set a variable through its owner.
pub fn set(world: &mut World, table: &[Setting], cvar: &str, value: &str) {
    let ext = table.iter().find(|x| x.cvar.eq_ignore_ascii_case(cvar)).is_some_and(|x| x.ext);
    if ext {
        if let Some(f) = world.resource::<Console>().cvar(cvar).map(|c| c.set.clone()) {
            let _ = f(world, value);
        }
    } else {
        world.resource_mut::<CaSettings>().set(cvar, value);
    }
}
