//! The Graphic tab's rules (specs/combat_arms/options.md G): the
//! brightness slider's gamma and the display ramp (G3), the resolution
//! list and the load-time fallback (G2), the quality presets and what
//! they derive (G4, `presets.toml`), the rules between the Custom
//! switches, and the blood switch's age rule (G5). Pure data and maths;
//! the settings table (`settings`) holds the values and `ui::options`
//! hands them to the renderer.
//!
//! Known shortcuts: the renderer's own settings (shadows, effect levels,
//! texture offsets, snow density, filtering) are computed (`Derived`) and
//! kept for a renderer to read, not applied: Chimera's renderer has no
//! such options; the display modes the list is built from are the
//! caller's (`Mode`), the app gives the monitor's.

use serde::Deserialize;

// ---------------------------------------------------------------------------
// G3: brightness

/// Gamma at slider 0 and 50.
pub const GAMMA_MIN: f64 = 0.5;
pub const GAMMA_MAX: f64 = 6.0;
/// The slider value that gives gamma 1.0.
pub const GAMMA_BREAK: f64 = 16.0;
/// When the profile has no `ScreenGamma`.
pub const GAMMA_DEFAULT: f64 = 1.22;

/// The slider value `s` (0 to 100) as gamma.
pub fn gamma_of_slider(s: f64) -> f64 {
    if s < GAMMA_BREAK {
        GAMMA_MIN + 0.5 * (s / GAMMA_BREAK).sqrt()
    } else {
        1.0 + 5.0 * ((s - GAMMA_BREAK) / 34.0).powi(2)
    }
}

/// A stored gamma as the slider value it shows.
pub fn slider_of_gamma(g: f64) -> i32 {
    if g < 1.0 {
        (0.5 + GAMMA_BREAK * ((g - GAMMA_MIN) / 0.5).powi(2)).floor() as i32
    } else {
        (0.5 + GAMMA_BREAK + 34.0 * ((g - 1.0) / 5.0).sqrt()).floor() as i32
    }
}

/// One channel's display ramp: input level `i` (0 to 255) as
/// `65535 (i/255)^(1/gamma)`, truncated.
pub fn ramp(gamma: f64) -> [u16; 256] {
    let mut out = [0u16; 256];
    for (i, o) in out.iter_mut().enumerate() {
        *o = (65535.0 * (i as f64 / 255.0).powf(1.0 / gamma)) as u16;
    }
    out
}

// ---------------------------------------------------------------------------
// G2: the resolution list

/// A display mode the renderer reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    /// Bits per pixel (16 or 32 are listed).
    pub depth: u32,
    /// The pixel format (the first reported mode's is the one listed).
    pub format: u32,
    /// Supports hardware transform and lighting.
    pub hardware_tnl: bool,
}

/// Smaller modes aren't listed.
pub const MIN_MODE: (u32, u32) = (800, 600);
/// The menus' fixed mode and the fallback.
pub const MENU_MODE: (u32, u32, u32) = (1024, 768, 32);

/// G2's list from the modes the renderer reports: at least 800 x 600, the
/// first reported mode's pixel format, 16 or 32 bits (and hardware
/// transform and lighting when `needs_tnl`), no duplicates, sorted by
/// width then height descending, then format, then depth descending.
pub fn mode_list(reported: &[Mode], needs_tnl: bool) -> Vec<Mode> {
    let Some(first) = reported.first() else { return Vec::new() };
    let mut out: Vec<Mode> = Vec::new();
    for m in reported {
        let ok = m.width >= MIN_MODE.0
            && m.height >= MIN_MODE.1
            && m.format == first.format
            && (m.depth == 16 || m.depth == 32)
            && (!needs_tnl || m.hardware_tnl);
        if ok && !out.iter().any(|o| (o.width, o.height, o.depth, o.format) == (m.width, m.height, m.depth, m.format)) {
            out.push(*m);
        }
    }
    out.sort_by(|a, b| {
        b.width
            .cmp(&a.width)
            .then(b.height.cmp(&a.height))
            .then(a.format.cmp(&b.format))
            .then(b.depth.cmp(&a.depth))
    });
    out
}

/// G2, at load: a stored mode that isn't among the listed becomes
/// 1024 x 768 x 32. With no 16-bit mode a stored 16 becomes 32.
pub fn load_mode(list: &[Mode], stored: (u32, u32, u32)) -> (u32, u32, u32) {
    let mut depth = stored.2;
    if depth == 16 && !list.iter().any(|m| m.depth == 16) {
        depth = 32;
    }
    if list.iter().any(|m| (m.width, m.height, m.depth) == (stored.0, stored.1, depth)) {
        (stored.0, stored.1, depth)
    } else {
        MENU_MODE
    }
}

// ---------------------------------------------------------------------------
// G4: presets

/// The quality choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quality {
    Low,
    Mid,
    High,
    Custom,
}

impl Quality {
    pub fn of(n: i32) -> Self {
        match n {
            1 => Quality::Mid,
            2 => Quality::High,
            3 => Quality::Custom,
            _ => Quality::Low,
        }
    }

    pub fn number(self) -> i32 {
        match self {
            Quality::Low => 0,
            Quality::Mid => 1,
            Quality::High => 2,
            Quality::Custom => 3,
        }
    }
}

/// One row of the preset table.
#[derive(Clone, Debug, Deserialize)]
pub struct Row {
    /// The stored `[Settings]` key.
    pub name: String,
    pub cvar: String,
    pub low: i32,
    pub mid: i32,
    pub high: i32,
    /// On the custom block.
    #[serde(default)]
    pub custom: bool,
}

impl Row {
    /// The value a preset gives (None for Custom).
    pub fn value(&self, q: Quality) -> Option<i32> {
        match q {
            Quality::Low => Some(self.low),
            Quality::Mid => Some(self.mid),
            Quality::High => Some(self.high),
            Quality::Custom => None,
        }
    }
}

#[derive(Deserialize)]
struct Table {
    row: Vec<Row>,
}

/// G4's table (`data/options/presets.toml`).
pub fn presets() -> Vec<Row> {
    toml::from_str::<Table>(chimera_core::game_data!("data/options/presets.toml").text())
        .expect("data/options/presets.toml")
        .row
}

/// The machine's capabilities (G4's "machine limits", G1's shader and
/// bloom switches).
#[derive(bevy::prelude::Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaMachine {
    /// Shader support: the shader and bloom switches, effects level High.
    pub shader: bool,
    pub triple_buffer: bool,
    pub music: bool,
    /// The renderer uses hardware transform and lighting (the mode list's
    /// filter).
    pub hardware_tnl: bool,
}

impl Default for CaMachine {
    fn default() -> Self {
        Self {
            shader: true,
            triple_buffer: true,
            music: true,
            hardware_tnl: false,
        }
    }
}

/// The projected shadows' settings (G4, derived).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shadows {
    pub blur: i32,
    pub texture_size: i32,
    pub back_face_cull: i32,
    pub per_frame: i32,
}

/// What the renderer's settings are, derived from the values written with
/// every preset or custom apply (G4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Derived {
    /// None: model projected shadows off.
    pub shadows: Option<Shadows>,
    pub client_fx_level: i32,
    pub debris_fx_level: i32,
    pub snow_density: f32,
    /// The texture groups' offsets (regular, large, extra large).
    pub texture_offsets: [i32; 3],
}

impl Default for Derived {
    fn default() -> Self {
        Self {
            shadows: None,
            client_fx_level: 0,
            debris_fx_level: 0,
            snow_density: 0.75,
            texture_offsets: [1, 1, 2],
        }
    }
}

/// G4's derived values from the written ones: `shadow_detail`,
/// `fx_detail`, `environmental_detail`, `detail_level`; `preset` says the
/// quality was a preset (not Custom), which on a shader-capable machine
/// forces both effect levels to 1 (Quirk 4). `previous` supplies the
/// texture offsets for a detail level the spec doesn't give.
pub fn derive(shadow_detail: i32, fx_detail: i32, environmental_detail: i32, detail_level: i32, preset: bool, machine: &CaMachine, previous: &Derived) -> Derived {
    let shadows = match shadow_detail {
        1 => Some(Shadows {
            blur: 0,
            texture_size: 128,
            back_face_cull: 0,
            per_frame: 0,
        }),
        2 => Some(Shadows {
            blur: 1,
            texture_size: 256,
            back_face_cull: 1,
            per_frame: 2,
        }),
        _ => None,
    };
    let mut fx = fx_detail.clamp(0, 2);
    if preset && machine.shader {
        fx = 1;
    }
    Derived {
        shadows,
        client_fx_level: fx,
        debris_fx_level: fx,
        snow_density: if environmental_detail == 0 { 0.75 } else { 1.0 },
        texture_offsets: if detail_level == 2 { [1, 1, 2] } else { previous.texture_offsets },
    }
}

// ---------------------------------------------------------------------------
// The Custom switches

/// The Custom block's switches and the rules between them (G4): clicked
/// one at a time, they keep each other consistent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Custom {
    pub anisotropic: bool,
    pub trilinear: bool,
    pub shader: bool,
    pub bloom: bool,
    /// 0 Low, 1 Mid, 2 High.
    pub effects: i32,
    pub machine: CaMachine,
}

impl Custom {
    pub fn new(machine: CaMachine) -> Self {
        Self {
            anisotropic: false,
            trilinear: false,
            shader: false,
            bloom: false,
            effects: 0,
            machine,
        }
        .enforced()
    }

    /// The rules applied to the current state (on load, and after every
    /// click).
    pub fn enforced(mut self) -> Self {
        if self.anisotropic && self.trilinear {
            self.trilinear = false;
        }
        if !self.machine.shader {
            self.shader = false;
            self.bloom = false;
            if self.effects == 2 {
                self.effects = 1;
            }
        } else if self.shader {
            if self.effects == 0 {
                self.effects = 1;
            }
        } else {
            self.bloom = false;
            if self.effects == 2 {
                self.effects = 1;
            }
        }
        self
    }

    /// Anisotropic on turns trilinear off.
    pub fn click_anisotropic(&mut self, on: bool) {
        self.anisotropic = on;
        if on {
            self.trilinear = false;
        }
        *self = self.enforced();
    }

    /// Trilinear on turns anisotropic off.
    pub fn click_trilinear(&mut self, on: bool) {
        self.trilinear = on;
        if on {
            self.anisotropic = false;
        }
        *self = self.enforced();
    }

    /// Shader on: bloom selectable, effects Low becomes Mid and is
    /// disabled, High selectable. Off: bloom off and disabled, High
    /// becomes Mid and is disabled, Low selectable.
    pub fn click_shader(&mut self, on: bool) {
        if self.machine.shader {
            self.shader = on;
        }
        *self = self.enforced();
    }

    pub fn click_bloom(&mut self, on: bool) {
        if self.machine.shader && self.shader {
            self.bloom = on;
        }
    }

    pub fn click_effects(&mut self, level: i32) {
        if self.effects_enabled(level) {
            self.effects = level;
        }
    }

    pub fn bloom_enabled(&self) -> bool {
        self.machine.shader && self.shader
    }

    pub fn shader_enabled(&self) -> bool {
        self.machine.shader
    }

    /// Whether effects level `level` (0 Low, 1 Mid, 2 High) can be picked.
    pub fn effects_enabled(&self, level: i32) -> bool {
        match level {
            0 => !(self.machine.shader && self.shader),
            2 => self.machine.shader && self.shader,
            _ => true,
        }
    }
}

// ---------------------------------------------------------------------------
// G5: blood

/// Builds with their own rule for the blood switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Region {
    #[default]
    Europe,
    Us,
    Korea,
    Brazil,
    Russia,
}

/// Whether the blood switch can be on: a low-violence build never; the US
/// and Korean builds not for an account under 18.
pub fn gore_allowed(low_violence: bool, region: Region, age: Option<u32>) -> bool {
    if low_violence {
        return false;
    }
    match region {
        Region::Us | Region::Korea => age.is_none_or(|a| a >= 18),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_round_trips() {
        assert!((gamma_of_slider(0.0) - 0.5).abs() < 1e-9);
        assert!((gamma_of_slider(4.0) - 0.75).abs() < 1e-9);
        assert!((gamma_of_slider(16.0) - 1.0).abs() < 1e-9);
        assert!((gamma_of_slider(33.0) - 2.25).abs() < 1e-9);
        assert!((gamma_of_slider(50.0) - 6.0).abs() < 1e-9);
        assert_eq!(slider_of_gamma(1.22), 23);
    }

    #[test]
    fn the_preset_table_reads() {
        let rows = presets();
        assert_eq!(rows.len(), 24);
        assert_eq!(rows.iter().filter(|r| r.custom).count(), 9);
    }
}
