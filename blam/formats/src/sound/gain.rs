//! Sound distance attenuation and gain (specs/halo_ce/sounds.md,
//! "Gain" and "Distance"), as pure functions: the inverse-distance falloff
//! of positional mono sounds and its audibility cutoff, the linear fade
//! of stereo sounds, the product that is an impulse's gain and the
//! console's volume in hundredths of a decibel. Distances are world units.

/// `lerp(a, b, s) = (b - a) s + a`: how the scale modifiers apply.
pub fn lerp(a: f32, b: f32, s: f32) -> f32 {
    (b - a) * s + a
}

/// A mono (positional) sound's attenuation at distance `d`: 1 up to the
/// minimum distance `min`, then `min / d` (6 dB per doubling). There is
/// no floor and no cut-off here: see `mono_audible`.
pub fn mono_gain(min: f32, d: f32) -> f32 {
    if d <= min { 1.0 } else { min / d }
}

/// Whether a mono sound at distance `d` is audible: the maximum distance
/// is a cutoff, not an endpoint (beyond it the sound fades out over 2 s
/// and an impulse stops).
pub fn mono_audible(max: f32, d: f32) -> bool {
    d <= max
}

/// A stereo sound's linear fade: 1 up to `min`, 0 from `max`, linear in
/// between. When `max` is not above `min` the fade is 1 inside `min` and 0
/// outside.
pub fn stereo_gain(min: f32, max: f32, d: f32) -> f32 {
    if max <= min {
        return if d <= min { 1.0 } else { 0.0 };
    }
    (1.0 - (d - min) / (max - min)).clamp(0.0, 1.0)
}

/// The factors of an impulse's gain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GainFactors {
    /// The permutation's gain.
    pub permutation: f32,
    /// The tag's gain modifier.
    pub modifier: f32,
    /// The tag's gain modifier when the play scale is zero and one.
    pub scale_zero: f32,
    pub scale_one: f32,
    /// The play request's scale, 0..1.
    pub scale: f32,
    /// Fade factor (1 normally; the stereo distance fade is multiplied in).
    pub fade: f32,
    pub source: f32,
    /// The class's current gain (1 unless a script changed it).
    pub class: f32,
    /// The dialogue duck (1 except while scripted dialogue plays).
    pub duck: f32,
}

impl Default for GainFactors {
    fn default() -> Self {
        GainFactors {
            permutation: 1.0,
            modifier: 1.0,
            scale_zero: 1.0,
            scale_one: 1.0,
            scale: 1.0,
            fade: 1.0,
            source: 1.0,
            class: 1.0,
            duck: 1.0,
        }
    }
}

impl GainFactors {
    /// `clamp(pg x gm x lerp(zg, og, scale) x fade x sg x cg x duck, 0, 1)`.
    pub fn gain(&self) -> f32 {
        (self.permutation
            * self.modifier
            * lerp(self.scale_zero, self.scale_one, self.scale)
            * self.fade
            * self.source
            * self.class
            * self.duck)
            .clamp(0.0, 1.0)
    }
}

/// The console's volume for a gain: `trunc(2000 log10 g)` hundredths of a
/// decibel, -10000 (silence) for a gain of 0 or less.
pub fn hardware_volume(g: f32) -> i32 {
    if g <= 0.0 {
        return -10_000;
    }
    ((2000.0 * g.log10()).trunc() as i32).max(-10_000)
}
