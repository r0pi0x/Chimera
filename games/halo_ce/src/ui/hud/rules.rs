//! Halo's HUD rules as pure functions and small state machines
//! (specs/halo_ce/hud.md, sections 3 to 8 and 11): the flash function,
//! the meters' fill alphas, the shield's hit bookkeeping and overshield
//! stages, the health colour, the weapon's flash states, values and
//! overlay masks, the crosshair states with their latches, and the motion
//! tracker's blip mapping, history and sweep. Times are Halo ticks (30 a
//! second).

use blam_formats::{
    cache::Argb8,
    hud::{ColorBlock, MeterElement, MeteredPanel, weapon::FlashCutoffs},
};

/// Ticks per second.
pub const TICKS: f32 = 30.0;
/// The constant in the flash pulse (not exactly 2 pi: the game's).
#[allow(clippy::approx_constant)]
pub const FLASH_TWO_PI: f32 = 6.283;
/// The shield bar's background flashes below this shield.
pub const SHIELD_FLASH_BELOW: f32 = 0.25;
/// How long the lost part of a hit shield stays marked (ticks).
pub const SHIELD_HIT_HOLD: i64 = 15;
/// The overshield stages' fixed colours (hud.md 6: stage 1 up): red,
/// green, yellow, violet.
pub const OVERSHIELD: [u32; 4] = [0xFF0000, 0x00FF00, 0xFFFF00, 0x7F00FF];
/// The motion sensor's blip history (entries) and scan period (ticks).
pub const RADAR_HISTORY: usize = 10;
pub const RADAR_SCAN_PERIOD: u64 = 15;
pub const RADAR_MAX_BLIPS: usize = 16;
pub const RADAR_DISTANCE_EXPONENT: f32 = 0.7;
/// The tracker's blip field's half size, pixels (one window).
pub const RADAR_HALF_FIELD: f32 = 42.0;

/// A colour as RGBA fractions (sRGB, as the tags store them).
pub type Rgba = [f32; 4];

/// `Argb8` as RGBA fractions.
pub fn rgba(c: Argb8) -> Rgba {
    [c.r, c.g, c.b, c.a].map(|v| f32::from(v) / 255.0)
}

/// A 0xRRGGBB value with alpha `a` as RGBA fractions.
pub fn rgb_hex(v: u32, a: f32) -> Rgba {
    [((v >> 16) & 0xFF) as f32 / 255.0, ((v >> 8) & 0xFF) as f32 / 255.0, (v & 0xFF) as f32 / 255.0, a]
}

fn mix(a: Rgba, b: Rgba, f: f32) -> Rgba {
    [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * f)
}

/// The flash function (section 3) of `block` for an element that started
/// flashing at tick `start`, at tick `now`.
pub fn flash(block: &ColorBlock, start: i64, now: i64) -> Rgba {
    let plain = rgba(block.color);
    let peak = rgba(block.flash_color);
    let reverse = block.flash_flags & ColorBlock::REVERSE != 0;
    let (from, to) = if reverse { (peak, plain) } else { (plain, peak) };
    let period = block.flash_period;
    if period <= 0.0 {
        return plain;
    }
    let t = (now - start) as f32 / TICKS;
    let phase = t.rem_euclid(period);
    let (d, l) = (block.flash_delay, block.flash_length);
    if phase >= f32::from(block.flash_count) * (d + l) || d + l <= 0.0 {
        return plain;
    }
    let phase = phase.rem_euclid(d + l);
    if start == 0 {
        to
    } else if phase < l {
        let f = (1.0 - ((FLASH_TWO_PI * phase / l).cos() + 1.0) / 2.0).clamp(0.0, 1.0).sqrt();
        mix(from, to, f)
    } else {
        from
    }
}

/// A static element's, number's or overlay's colour: disabled, flashing
/// (from `start`) or plain (section 3, "Colour choice").
pub fn color(block: &ColorBlock, flashing: Option<i64>, disabled: bool, now: i64) -> Rgba {
    if disabled {
        rgba(block.disabled_color)
    } else if let Some(start) = flashing {
        flash(block, start, now)
    } else {
        rgba(block.color)
    }
}

/// A meter's alpha byte for `value` (section 4): max(minimum, clamp(m v +
/// b, 0, 255)).
pub fn meter_alpha(m: &MeterElement, value: i32) -> u8 {
    let a = (i32::from(m.alpha_multiplier) * value + i32::from(m.alpha_bias)).clamp(0, 255);
    a.max(i32::from(m.minimum)) as u8
}

/// What a meter is drawn with (section 4): the fill value x and flash
/// extension e (0 to 1), the gradient's two colours, the flash colour
/// (scaled by phi), the empty colour and how much of the frame an empty
/// texel keeps, and the share of the frame a filled texel keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeterDraw {
    pub value: f32,
    pub extension: f32,
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub flash: [f32; 3],
    pub empty: [f32; 3],
    pub empty_keep: f32,
    pub fill_keep: f32,
    /// Grey of (1 - opacity): the filled texels' tint.
    pub tint: f32,
}

/// A meter's draw from its element, its min and max values, the
/// flashing and disabled flags and the reference time (a fraction of the
/// flash; negative: none). `colour` replaces both gradient ends (the
/// health panel's colour by vitality, the overshield stages').
pub fn meter_draw(m: &MeterElement, min_value: i32, max_value: i32, flashing: bool, disabled: bool, reference: f32, colour: Option<[f32; 3]>) -> MeterDraw {
    let rgb = |c: Argb8| [c.r, c.g, c.b].map(|v| f32::from(v) / 255.0);
    let lo = meter_alpha(m, min_value);
    let hi = meter_alpha(m, max_value);
    let phi = if reference < 0.0 { 0.0 } else { (1.0 - reference).clamp(0.0, 1.0) };
    let (min, max, flash) = if m.flags & 1 == 0 {
        let (a, b) = colour.map_or((rgb(m.min_color), rgb(m.max_color)), |c| (c, c));
        (a, b, rgb(m.flash_color).map(|v| v * phi))
    } else {
        let c = colour.unwrap_or(if flashing { rgb(m.max_color) } else { rgb(m.min_color) });
        (c, c, [0.0; 3])
    };
    let (min, max, flash) = if disabled { ([0.0; 3], [0.0; 3], [0.0; 3]) } else { (min, max, flash) };
    let (value, extension) = if disabled { (0.0, 0.0) } else { (f32::from(lo) / 255.0, f32::from(hi) / 255.0) };
    MeterDraw {
        value,
        extension,
        min,
        max,
        flash,
        empty: rgb(m.empty_color),
        empty_keep: 1.0 - f32::from(m.empty_color.a) / 255.0,
        fill_keep: m.fade.clamp(0.0, 1.0),
        tint: (1.0 - m.opacity).clamp(0.0, 1.0),
    }
}

/// The shield's hit bookkeeping of one local player (section 5): L (the
/// shield last shown), f (a fade time or -1) and t (a tick).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShieldMark {
    pub last: f32,
    pub fade: f32,
    pub tick: i64,
}

impl Default for ShieldMark {
    fn default() -> Self {
        ShieldMark { last: -1.0, fade: -1.0, tick: 0 }
    }
}

impl ShieldMark {
    /// One tick with the shield at `s` on tick `now`.
    pub fn update(&mut self, s: f32, now: i64) {
        if self.last < 0.0 {
            *self = ShieldMark { last: s, fade: -1.0, tick: now };
            return;
        }
        if self.last > s {
            if self.fade < 0.0 || self.fade > 1.0 {
                self.tick = now;
            }
            if now - self.tick < SHIELD_HIT_HOLD {
                self.fade = 0.0;
            } else {
                self.last = s;
                self.fade += (now - self.tick) as f32 / TICKS;
                self.tick = now;
            }
        } else if self.last < s {
            *self = ShieldMark { last: s, fade: -1.0, tick: now };
        } else {
            if self.fade > 0.0 {
                self.fade += (now - self.tick) as f32 / TICKS;
            }
            self.tick = now;
        }
    }
}

/// One stage of the shield meter (section 6): its min and max values, the
/// reference time and its colour (None: the tag's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShieldStage {
    pub min_value: i32,
    pub max_value: i32,
    pub reference: f32,
    pub colour: Option<[f32; 3]>,
}

/// The shield meter's stages for shield `s`, its mark and the value scale
/// (the tag's, or 255 when 0).
pub fn shield_stages(s: f32, mark: &ShieldMark, value_scale: i16) -> Vec<ShieldStage> {
    let scale = if value_scale == 0 { 255.0 } else { f32::from(value_scale) };
    let last = if mark.last < 0.0 { s } else { mark.last };
    let mut out = Vec::new();
    for i in 0..5 {
        let v = (s - i as f32).clamp(0.0, 1.0);
        let w = (last - i as f32).clamp(0.0, 1.0);
        let fading = w > v;
        let top = if fading { w } else { v };
        if v == 0.0 && top == 0.0 && i > 0 {
            break;
        }
        out.push(ShieldStage {
            min_value: ((scale * v) as i32).min(255),
            max_value: ((scale * top) as i32).min(255),
            reference: if fading { mark.fade } else { -1.0 },
            colour: (i > 0).then(|| {
                let c = OVERSHIELD[(i - 1).min(3)];
                [((c >> 16) & 0xFF) as f32 / 255.0, ((c >> 8) & 0xFF) as f32 / 255.0, (c & 0xFF) as f32 / 255.0]
            }),
        });
        if v == 0.0 && top == 0.0 {
            break;
        }
    }
    out
}

/// The health meter's value and colour for body vitality `b` (section 6):
/// value = min(trunc(scale b), 255) with the tag's scale or 8; the max
/// colour at or above the maximum cutoff, the min colour at or below the
/// minimum, the medium colour between.
pub fn health(panel: &MeteredPanel, b: f32) -> (i32, [f32; 3]) {
    let m = &panel.meter;
    let scale = if m.value_scale == 0 { 8.0 } else { f32::from(m.value_scale) };
    let value = ((scale * b.max(0.0)) as i32).min(255);
    let c = if b >= panel.max_cutoff {
        m.max_color
    } else if b <= panel.min_cutoff {
        m.min_color
    } else {
        panel.medium_color
    };
    (value, [c.r, c.g, c.b].map(|v| f32::from(v) / 255.0))
}

/// What the HUD reads of the weapon in hand (section 7, "Weapon interface
/// state").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeaponNow {
    pub loaded: i32,
    pub reserve: i32,
    pub loaded_max: i32,
    /// Magazine 0 reloading or chambering.
    pub reloading: bool,
    /// Magazine 0 idle.
    pub ready: bool,
    /// Has a magazine at all.
    pub magazine: bool,
    pub heat: f32,
    /// 0 full battery, 1 empty.
    pub age: f32,
    pub overheated: bool,
    /// The zoom level, -1 none.
    pub zoom: i16,
    pub firing: bool,
}

/// Overlay mask bits (section 7).
pub const MASK_FLASHING: i16 = 1;
pub const MASK_EMPTY: i16 = 2;
pub const MASK_RELOAD: i16 = 4;
pub const MASK_DEFAULT: i16 = 8;
pub const MASK_ALWAYS: i16 = 16;

/// One weapon state's flags and values (section 7's table).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeaponState {
    pub flashing: bool,
    pub disabled: bool,
    pub value: i64,
    pub real: Option<f32>,
    pub mask: i16,
}

fn mask(flashing: bool, empty: bool, reload: bool) -> i16 {
    let mut m = MASK_ALWAYS;
    if flashing {
        m |= MASK_FLASHING;
    }
    if empty {
        m |= MASK_EMPTY;
    }
    if reload {
        m |= MASK_RELOAD;
    }
    if !(flashing || empty || reload) {
        m |= MASK_DEFAULT;
    }
    m
}

/// The eight states of the weapon by `cutoffs` (states 4 and 5, the
/// second magazine, and 6 and 7, the aim assist's target, stay empty: no
/// Halo weapon here has two magazines or aim assist).
pub fn weapon_states(w: &WeaponNow, cutoffs: &FlashCutoffs) -> [WeaponState; 8] {
    let mut s = [WeaponState::default(); 8];
    let heat_pct = w.heat * 100.0;
    let battery_pct = (1.0 - w.age) * 100.0;
    let age_pct = (w.age * 100.0) as i32;
    let total_flash = w.reserve <= i32::from(cutoffs.total_ammo);
    s[0] = WeaponState {
        flashing: total_flash,
        disabled: w.reserve == 0,
        value: i64::from(w.reserve),
        real: None,
        mask: mask(total_flash && !w.reloading, w.reserve == 0, w.reloading),
    };
    let loaded_flash = w.loaded <= i32::from(cutoffs.loaded_ammo);
    s[1] = WeaponState {
        flashing: loaded_flash && !w.reloading,
        disabled: false,
        value: i64::from(w.loaded),
        real: None,
        mask: mask(loaded_flash, w.loaded == 0, w.reloading),
    };
    let heat_flash = heat_pct >= f32::from(cutoffs.heat);
    s[2] = WeaponState {
        flashing: heat_flash,
        disabled: false,
        value: i64::from((w.heat * 255.0) as i32),
        real: None,
        mask: mask(heat_flash, age_pct >= 100, w.overheated),
    };
    let battery_flash = battery_pct <= f32::from(cutoffs.age);
    s[3] = WeaponState {
        flashing: battery_flash,
        disabled: age_pct >= 100,
        value: i64::from(battery_pct as i32),
        real: None,
        mask: mask(battery_flash, age_pct >= 100, w.overheated),
    };
    s[6].real = Some(f32::NAN);
    s[7].real = Some(f32::NAN);
    s
}

/// The crosshair types (section "Crosshair types").
pub mod crosshair {
    pub const AIM: usize = 0;
    pub const ZOOM: usize = 1;
    pub const SHOULD_RELOAD: usize = 3;
    pub const FLASH_HEAT: usize = 4;
    pub const FLASH_TOTAL: usize = 5;
    pub const FLASH_BATTERY: usize = 6;
    pub const RELOAD: usize = 7;
    pub const NO_AMMO: usize = 8;
    pub const NO_GRENADE: usize = 9;
    pub const LOW_AMMO: usize = 10;
    pub const READY: usize = 16;
    pub const BATTERY_DEPLETED: usize = 18;
    pub const COUNT: usize = 19;
}

/// The crosshair states of one local player (section 8): per type its
/// value (aim: 0 or 1; zoom: level + 1; others: the tick it became
/// active, None when inactive) and the render bits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CrosshairStates {
    pub aim: i64,
    pub zoom: i64,
    pub start: [Option<i64>; crosshair::COUNT],
}

impl CrosshairStates {
    /// One tick: the types in `valid` from the weapon `w` with the root's
    /// `cutoffs`; a latched warning (8, 9, 18) clears once `hold(type)`
    /// ticks have passed since it started and its condition is gone.
    /// `grenades`: the unit has a grenade; `throwing`: the throw control.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, valid: u32, w: &WeaponNow, cutoffs: &FlashCutoffs, grenades: bool, throwing: bool, now: i64, hold: impl Fn(usize) -> i64) {
        use crosshair::*;
        self.aim = 0;
        self.zoom = if w.zoom >= 0 { i64::from(w.zoom) + 1 } else { 0 };
        for n in 2..COUNT {
            if valid & (1 << n) == 0 {
                continue;
            }
            let active = match n {
                SHOULD_RELOAD => w.reserve > 0 && w.loaded <= i32::from(cutoffs.loaded_ammo),
                FLASH_HEAT => w.heat * 100.0 >= f32::from(cutoffs.heat),
                FLASH_TOTAL => w.reserve <= i32::from(cutoffs.total_ammo) && !w.reloading,
                FLASH_BATTERY => w.age < 1.0 && (1.0 - w.age) * 100.0 <= f32::from(cutoffs.age),
                RELOAD => w.reloading,
                NO_AMMO => w.loaded == 0 && w.reserve == 0 && w.firing,
                NO_GRENADE => !grenades && throwing,
                LOW_AMMO => (w.reserve > 0 || w.loaded > 0) && w.reserve == 0 && w.loaded <= i32::from(cutoffs.loaded_ammo),
                READY => w.ready,
                BATTERY_DEPLETED => w.age >= 1.0 && w.firing,
                _ => false,
            };
            let latched = matches!(n, NO_AMMO | NO_GRENADE | BATTERY_DEPLETED);
            self.start[n] = match (self.start[n], active) {
                (None, true) => Some(now),
                (Some(s), true) => Some(s),
                (Some(s), false) if latched && now - s < hold(n) => Some(s),
                _ => None,
            };
        }
    }

    /// Whether type `n` renders now.
    pub fn renders(&self, n: usize) -> bool {
        match n {
            crosshair::AIM => true,
            crosshair::ZOOM => self.zoom > 0,
            n => self.start.get(n).copied().flatten().is_some(),
        }
    }
}

/// A unit's blip kind on the tracker (section 11) and its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlipKind {
    Own,
    Friend,
    Enemy,
}

impl BlipKind {
    pub fn colour(self) -> [f32; 3] {
        match self {
            BlipKind::Own => [1.0, 0.5, 0.0],
            BlipKind::Friend => [1.0, 1.0, 0.0],
            BlipKind::Enemy => [1.0, 0.0, 0.0],
        }
    }
}

/// A displacement component quantised to a signed byte (trunc(c / range
/// x 127)).
pub fn quantise(c: f32, range: f32) -> i8 {
    (c / range * 127.0).trunc().clamp(-127.0, 127.0) as i8
}

/// A stored blip's pixel offset from the disc's centre (y down) for the
/// viewer's facing `yaw` (radians, Halo's world frame: x forward at yaw 0,
/// y left), or None past the range (section 11, "Drawing a blip").
pub fn blip_offset(dx: i8, dy: i8, yaw: f32, range: f32, scale: f32, half_field: f32) -> Option<[f32; 2]> {
    // The byte times the range times the single-precision 1/127: byte 127 is exactly the range.
    let (x, y) = (f32::from(dx) * range * (1.0 / 127.0), f32::from(dy) * range * (1.0 / 127.0));
    let a = -(yaw + std::f32::consts::FRAC_PI_2);
    let (s, c) = a.sin_cos();
    let (rx, ry) = (x * c - y * s, x * s + y * c);
    let d = (rx * rx + ry * ry).sqrt().max(0.015625);
    // Strict in the drawing (hud.md 11): a unit at exactly the range is
    // scanned (byte 127) but not drawn.
    if d >= range {
        return None;
    }
    let r = half_field * (scale / 32.0) * (d / range).powf(RADAR_DISTANCE_EXPONENT);
    Some([-rx / d * r, ry / d * r])
}

/// History entry `h`'s brightness (w x w) and size factor (section 11,
/// step 5).
pub fn blip_fade(h: usize) -> (f32, f32) {
    let w = (10 - h.min(10)) as f32 / 10.0;
    (w * w, (1.0 - w).powf(3.5) * 7.0 + 1.0)
}

/// Whether a unit shows: alive, firing, throwing or moving at a squared
/// speed of at least `sensitivity` (WU per tick, squared), within `range`
/// horizontally.
pub fn shows(speed_wu_per_tick: f32, firing: bool, distance: f32, sensitivity: f32, range: f32) -> bool {
    (firing || speed_wu_per_tick * speed_wu_per_tick >= sensitivity) && distance <= range
}

/// The sweep's window half size s / 2 at tick `now` (section 11): t =
/// time mod 2.1 s; s = 1 / ((t + 0.0625) x 1.1) below 2.0375 s, else 0.4.
pub fn sweep(now: i64) -> f32 {
    let t = (now as f32 / TICKS).rem_euclid(2.1);
    let s = if t < 2.0375 { 1.0 / ((t + 0.0625) * 1.1) } else { 0.4 };
    s / 2.0
}

/// One blip in the history: whose (an id), its kind, its quantised
/// displacement and the viewer's facing when stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blip {
    pub id: u64,
    pub kind: BlipKind,
    pub dx: i8,
    pub dy: i8,
    pub yaw: f32,
}

/// A candidate on a scan: its id, kind, horizontal displacement from the
/// viewer (WU, Halo's world frame) and whether it shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub id: u64,
    pub kind: BlipKind,
    pub at: [f32; 2],
    pub shows: bool,
}

/// The motion tracker's blip history (section 11): ten entries, newest
/// first, rebuilt every 15th tick, else a copy of the previous one; the
/// newest entry's blips refreshed each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracker {
    pub entries: std::collections::VecDeque<Vec<Blip>>,
    pub tick: Option<u64>,
}

impl Tracker {
    /// Advance to tick `now` (one entry per tick passed, at most the
    /// history), then refresh the newest entry from `seen` with the
    /// viewer's `yaw`.
    pub fn frame(&mut self, now: u64, seen: &[Seen], yaw: f32, range: f32) {
        let steps = match self.tick {
            None => 1,
            Some(t) if now > t => (now - t).min(RADAR_HISTORY as u64),
            _ => 0,
        };
        for k in 0..steps {
            let tick = now + 1 - steps + k;
            let entry = if tick.is_multiple_of(RADAR_SCAN_PERIOD) || self.entries.is_empty() {
                seen.iter()
                    .filter(|s| s.shows)
                    .take(RADAR_MAX_BLIPS)
                    .map(|s| Blip { id: s.id, kind: s.kind, dx: quantise(s.at[0], range), dy: quantise(s.at[1], range), yaw })
                    .collect()
            } else {
                self.entries.front().cloned().unwrap_or_default()
            };
            self.entries.push_front(entry);
            self.entries.truncate(RADAR_HISTORY);
        }
        self.tick = Some(now);
        if let Some(newest) = self.entries.front_mut() {
            newest.retain_mut(|b| match seen.iter().find(|s| s.id == b.id) {
                Some(s) if s.shows => {
                    b.dx = quantise(s.at[0], range);
                    b.dy = quantise(s.at[1], range);
                    b.yaw = yaw;
                    true
                }
                _ => false,
            });
        }
    }
}
