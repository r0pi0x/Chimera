//! How a listener hears a Source sound (specs/source/sound_hearing.md;
//! measured in TF2, the same engine rules in HL2:DM and CS:S), as pure
//! functions over Source units: the distance curve of a sound level
//! (`source_gain`: inverse distance from 36 u with 1 dB of air loss per
//! 300 u, capped, compressed toward 1 above 0.5, faded to silence near
//! −47 dB), compatibility attenuation's level encoding
//! (`compat_level`, `compat_attenuation`), the wall loss
//! (`obscured_gain`), the stereo-speaker pan law (`speaker_pan`), the
//! room's wet share (`dsp_wet`) and the distance-variant blend
//! (`far_weight`). What an entry carries of them is `EntryHearing`, set
//! by the game's loader (Source's: `source_engine::content::sound`, with
//! its mixer table); an entry without one (Combat Arms', Halo's) keeps
//! its game's own law.
//!
//! Known shortcuts (docs/tech-debt.md): the pan law's overall level
//! between the measured directions (straight ahead 0.64, to the side 1.0,
//! behind 0.48 per ear; above or below 0.90) is a straight line in the
//! angle, and elevation blends toward the overhead level only above 45°
//! (one measured point at 45°); headphone and surround modes are not
//! measured (spec Q5), so every listener hears the stereo-speaker law.
//! The fade's end point (raw gain 0.0045) is the spec's fit (Q1). Why it's
//! OK for now: the measured points are exact; the rest is smooth between
//! them.

/// Source's reference level (`snd_refdb`, dB) and distance (`snd_refdist`, u).
pub const REF_LEVEL: f32 = 60.0;
pub const REF_DIST: f32 = 36.0;
/// Air loss: 1 dB per this many units.
pub const AIR_LOSS_UNITS: f32 = 300.0;
/// The raw gain's cap (+20 dB) before the knee.
pub const RAW_GAIN_CAP: f32 = 10.0;
/// Raw gains from here up are compressed toward 1.
pub const KNEE_START: f32 = 0.5;
/// Below this raw gain the output fades linearly to 0 at `FADE_ZERO`.
pub const FADE_TOP: f32 = 0.008;
pub const FADE_ZERO: f32 = 0.0045;
/// `snd_obscured_gain_dB`: the wall loss is three times it (−8.1 dB).
pub const OBSCURED_DB: f32 = -2.7;
/// The room's wet-mix distance range (`dsp_dist_min`, `dsp_dist_max`, u).
pub const DSP_DIST_MIN: f32 = 0.0;
pub const DSP_DIST_MAX: f32 = 1440.0;
/// Sounds below this level send half as much to the room.
pub const DSP_DB_MIN: f32 = 80.0;
pub const DSP_DB_MIXDROP: f32 = 0.5;
/// The distance-variant ("^") blend: near until 240 u, far from 1320 u.
pub const DISTVAR_NEAR: f32 = 240.0;
pub const DISTVAR_FAR: f32 = 1320.0;
/// The room's wet share range of the automatic room (measured on TF2's
/// itemtest) and of most fixed presets.
pub const AUTO_ROOM_MIX: (f32, f32) = (0.32, 0.80);
pub const FIXED_ROOM_MIX: (f32, f32) = (0.2, 0.7);

/// What an entry carries of Source's hearing (`MapSoundEntry::hearing`):
/// its mixer value, the wall loss and whether its waves are
/// distance-variant. An entry with one is heard by Source's rules (the
/// stereo-speaker pan, the wall trace, the room's wet share); without one,
/// by its game's own.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EntryHearing {
    /// The mixer's value when a player makes it (its source is a
    /// character) and otherwise (Default_Mix's first matched group with a
    /// value, sound_hearing.md 6).
    pub mix_player: f32,
    pub mix_world: f32,
    /// Its gain when the line from the ear to it is blocked by the world
    /// (`obscured_gain`; 1: walls don't matter).
    pub obscured: f32,
    /// Its waves are distance-variant stereo ("^": left near, right far),
    /// blended by `far_weight`.
    pub near_far: bool,
}

impl EntryHearing {
    /// The mixer value for a sound made by a player (`player`) or not.
    pub fn mix(&self, player: bool) -> f32 {
        if player { self.mix_player } else { self.mix_world }
    }
}

/// The knee's exponent for level `l`: 2.5 up to 90 dB, falling 0.034 per
/// dB above (0.8 at 140).
pub fn knee_power(l: f32) -> f32 {
    if l <= 90.0 { 2.5 } else { 2.5 - 0.034 * (l - 90.0) }
}

/// The raw gain of level `l` at `units`: (L − 60) dB, inverse distance
/// from 36 u and 1 dB per 300 u, at most `RAW_GAIN_CAP`.
pub fn raw_gain(l: f32, units: f32) -> f32 {
    if units <= 0.0 {
        return RAW_GAIN_CAP;
    }
    let db = (l - REF_LEVEL) - 20.0 * (units / REF_DIST).log10() - units / AIR_LOSS_UNITS;
    10f32.powf(db / 20.0).min(RAW_GAIN_CAP)
}

/// The gain of Source sound level `l` at `units` (sound_hearing.md 2):
/// the raw gain, compressed toward 1 from `KNEE_START` (1 − 0.5 (0.5 /
/// raw)^p), as is between, faded linearly to 0 below `FADE_TOP`. Level 0
/// (or less) has no falloff.
pub fn source_gain(l: f32, units: f32) -> f32 {
    if l <= 0.0 {
        return 1.0;
    }
    let raw = raw_gain(l, units);
    if raw >= KNEE_START {
        1.0 - 0.5 * (KNEE_START / raw).powf(knee_power(l))
    } else if raw >= FADE_TOP {
        raw
    } else if raw > FADE_ZERO {
        FADE_TOP * (raw - FADE_ZERO) / (FADE_TOP - FADE_ZERO)
    } else {
        0.0
    }
}

/// The distance (u) from which level `l` is silent: where its raw gain
/// falls to `FADE_ZERO` (it only falls with distance).
pub fn source_silent_from(l: f32) -> f32 {
    let (mut lo, mut hi) = (0.0f32, 1.0e5f32);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if raw_gain(l, mid) > FADE_ZERO { lo = mid } else { hi = mid }
    }
    hi
}

/// The sound level a compatibility (or plain) attenuation `a` encodes:
/// trunc(50 + 20 / a); 0 for none.
pub fn compat_level(a: f32) -> f32 {
    if a <= 0.0 { 0.0 } else { (50.0 + 20.0 / a).trunc() }
}

/// The attenuation a compatibility entry falls off with: its script value
/// through the level encoding and back, 20 / (L − 50) (0.52 → level 88 →
/// 0.526, silent beyond 1900 u); 4 at or below level 50, 0 for none.
pub fn compat_attenuation(a: f32) -> f32 {
    if a <= 0.0 {
        return 0.0;
    }
    let l = compat_level(a);
    if l > 50.0 { 20.0 / (l - 50.0) } else { 4.0 }
}

/// The level a `SoundLevel` stands for in the mixer's and the room's rules
/// (a compatibility entry: its encoded level below 256, spec Q7).
pub fn level_of(level: super::SoundLevel) -> f32 {
    match level {
        super::SoundLevel::Db(l) | super::SoundLevel::Inverse(l) => l,
        super::SoundLevel::Attenuation(a) => compat_level(a),
        super::SoundLevel::Radius { .. } => 0.0,
    }
}

/// The wall loss's gain for `snd_obscured_gain_dB` = `db`: three times it
/// in dB (−2.7 → 0.393).
pub fn obscured_gain(db: f32) -> f32 {
    10f32.powf(3.0 * db / 20.0)
}

/// The share of a sound sent to the room at `units` for a room whose mix
/// range is `range` (sound_hearing.md 7): from its minimum at
/// `DSP_DIST_MIN` to its maximum at `DSP_DIST_MAX`, halved below
/// `DSP_DB_MIN`.
pub fn dsp_wet(range: (f32, f32), units: f32, level: f32) -> f32 {
    let t = ((units - DSP_DIST_MIN) / (DSP_DIST_MAX - DSP_DIST_MIN)).clamp(0.0, 1.0);
    let wet = range.0 + (range.1 - range.0) * t;
    if level < DSP_DB_MIN { wet * DSP_DB_MIXDROP } else { wet }
}

/// The far version's weight of a distance-variant wave at `units`
/// (0 within 240 u, 1 from 1320 u, linear between); the near one's is
/// 1 minus it.
pub fn far_weight(units: f32) -> f32 {
    ((units - DISTVAR_NEAR) / (DISTVAR_FAR - DISTVAR_NEAR)).clamp(0.0, 1.0)
}

/// Measured left/right ratios of the stereo-speaker law by azimuth (°),
/// sound_hearing.md 5.
const PAN_RATIOS: [(f32, f32); 13] = [
    (0.0, 1.0),
    (3.0, 1.05),
    (6.0, 1.17),
    (9.0, 1.31),
    (12.0, 1.50),
    (15.0, 1.69),
    (21.0, 2.26),
    (24.0, 2.74),
    (27.0, 3.32),
    (30.0, 3.85),
    (33.0, 5.0),
    (36.0, 7.7),
    (39.0, 10.4),
];
/// From this azimuth (°) the far ear gets nothing.
pub const PAN_HARD: f32 = 44.0;

/// The far ear's share of the near one's level for a source `deg` degrees
/// (0-90, folded to the front) off the view axis: the measured ratios
/// interpolated, then down to 0 at `PAN_HARD`.
pub fn far_share(deg: f32) -> f32 {
    let deg = deg.abs();
    if deg >= PAN_HARD {
        return 0.0;
    }
    let last = PAN_RATIOS[PAN_RATIOS.len() - 1];
    if deg >= last.0 {
        return (1.0 / last.1) * (PAN_HARD - deg) / (PAN_HARD - last.0);
    }
    let i = PAN_RATIOS.iter().rposition(|(a, _)| *a <= deg).unwrap_or(0);
    let (a0, r0) = PAN_RATIOS[i];
    let (a1, r1) = PAN_RATIOS[i + 1];
    1.0 / (r0 + (r1 - r0) * (deg - a0) / (a1 - a0))
}

/// The near ear's level by unfolded azimuth (0 ahead, 90 to the side, 180
/// behind; °): 0.64, 1.0, 0.48 measured, straight between.
fn near_level(deg: f32) -> f32 {
    let deg = deg.abs().min(180.0);
    if deg <= 90.0 { 0.64 + 0.36 * deg / 90.0 } else { 1.0 - 0.52 * (deg - 90.0) / 90.0 }
}

/// Per-ear level overhead (or underfoot).
const OVERHEAD_LEVEL: f32 = 0.90;

/// Per-ear gains (left, right) of a source in direction `local` (unit,
/// listener's frame: x right, y up, −z forward) by the stereo-speaker law
/// (sound_hearing.md 5): the near ear's level by azimuth, the far one's by
/// the measured ratio (none from 44° off the axis to 136°), behind
/// mirrored onto the front; toward overhead both blend to 0.90.
pub fn speaker_pan(local: [f32; 3]) -> (f32, f32) {
    let [x, y, z] = local;
    let (left, forward) = (-x, -z);
    let horizontal = (left * left + forward * forward).sqrt();
    let azimuth = left.atan2(forward).to_degrees();
    let folded = if azimuth.abs() > 90.0 { 180.0 - azimuth.abs() } else { azimuth.abs() };
    let near = near_level(azimuth);
    let far = near * far_share(folded);
    let up = (y / (horizontal * horizontal + y * y).sqrt().max(1e-6)).abs();
    let w = if horizontal < 1e-6 { 1.0 } else { ((up - std::f32::consts::FRAC_1_SQRT_2) / (1.0 - std::f32::consts::FRAC_1_SQRT_2)).clamp(0.0, 1.0) };
    let (near, far) = (near + (OVERHEAD_LEVEL - near) * w, far + (OVERHEAD_LEVEL - far) * w);
    if azimuth >= 0.0 { (near, far) } else { (far, near) }
}

/// Per-ear level of a level-0 world sound (not spatialized, centred).
pub const UNSPATIAL_LEVEL: f32 = 0.89;

#[cfg(test)]
mod tests {
    use super::*;

    fn close(got: f32, want: f32, tol: f32) -> bool {
        (got - want).abs() <= tol
    }

    /// The spec's table and test cases, to its third decimal (the fade
    /// region ±0.001).
    #[test]
    fn the_distance_curve_matches_the_measured_points() {
        let cases = [
            (75.0, 1000.0, 0.138),
            (75.0, 4800.0, 0.005),
            (75.0, 5600.0, 0.0),
            (94.0, 1100.0, 0.919),
            (94.0, 1900.0, 0.458),
            (94.0, 2400.0, 0.299),
            (110.0, 3400.0, 0.831),
            (120.0, 6000.0, 0.618),
            (140.0, 100.0, 0.954),
            (140.0, 7000.0, 0.895),
            (150.0, 9000.0, 0.808),
            (64.0, 50.0, 0.934),
            (60.0, 36.0, 0.909),
            (60.0, 100.0, 0.346),
            (70.0, 250.0, 0.414),
            (75.0, 250.0, 0.810),
            (80.0, 500.0, 0.675),
            (85.0, 1000.0, 0.436),
            (90.0, 1000.0, 0.833),
            (94.0, 1000.0, 0.940),
            (94.0, 4000.0, 0.097),
            (95.0, 1500.0, 0.811),
            (100.0, 2000.0, 0.835),
            (105.0, 3000.0, 0.725),
            (120.0, 9000.0, 0.126),
            (140.0, 6000.0, 0.932),
            (140.0, 9000.0, 0.762),
        ];
        for (l, d, want) in cases {
            let got = source_gain(l, d);
            assert!(close(got, want, 0.0015), "L {l} at {d} u: {got:.4}, want {want}");
        }
        // Level 0: no falloff anywhere.
        assert_eq!(source_gain(0.0, 5000.0), 1.0);
        // Point-blank: the cap's knee (0.9996 at 94, 0.954 at 140).
        assert!(close(source_gain(94.0, 0.0), 0.9996, 1e-4));
        // Silent from where the fade reaches 0 (the table's last column).
        for (l, silent) in [(60.0, 2770.0), (75.0, 5480.0), (94.0, 9700.0), (140.0, 21430.0)] {
            assert!(close(source_silent_from(l), silent, 15.0), "L {l}: {}", source_silent_from(l));
        }
    }

    /// CS:S's compatibility attenuations go through the level encoding:
    /// the AK-47 (0.52) is silent beyond 1900 u, the AWP (0.28) beyond
    /// 3550, footsteps (1.0) beyond 1000; a = 1.0 at 504 u gives 0.496.
    #[test]
    fn compatibility_attenuation_cuts_off_linearly() {
        use super::super::sound::{SoundLevel, distance_gain, SOURCE_UNIT};
        let gain = |a: f32, units: f32| distance_gain(SoundLevel::Attenuation(compat_attenuation(a)), units * SOURCE_UNIT);
        for (a, silent) in [(0.52, 1900.0), (0.28, 3550.0), (1.0, 1000.0), (0.6, 1650.0), (0.64, 1550.0), (1.6, 600.0), (0.8, 1250.0), (0.48, 2050.0), (0.40, 2500.0)] {
            assert!(gain(a, silent - 5.0) > 0.0, "a {a} just inside {silent}");
            assert_eq!(gain(a, silent + 1.0), 0.0, "a {a} beyond {silent}");
        }
        assert_eq!(compat_level(0.52), 88.0);
        assert_eq!(compat_level(0.28), 121.0);
        assert!(close(gain(1.0, 504.0), 0.496, 1e-3));
        assert_eq!(gain(0.52, 1950.0), 0.0);
        assert_eq!(level_of(SoundLevel::Attenuation(compat_attenuation(0.8))), 75.0);
    }

    /// The wall loss: −8.1 dB (× 0.393); the cvar at −1 gives 0.708.
    #[test]
    fn the_wall_loss_is_three_times_the_cvar() {
        assert!(close(obscured_gain(OBSCURED_DB), 0.393, 1e-3));
        assert!(close(obscured_gain(-1.0), 0.708, 1e-3));
        assert_eq!(obscured_gain(0.0), 1.0);
        // The 297 u floor test: 75 dB, × 0.393.
        assert!(close(source_gain(75.0, 297.0) * obscured_gain(OBSCURED_DB), source_gain(75.0, 297.0) * 0.393, 1e-3));
    }

    /// The stereo-speaker law's left/right ratios at the measured angles,
    /// mirrored for the right and behind; hard past 44°.
    #[test]
    fn panning_follows_the_measured_ratios() {
        let dir = |deg: f32| {
            let r = deg.to_radians();
            // Positive azimuth is to the left: x (right) negative.
            [-r.sin(), 0.0, -r.cos()]
        };
        for (deg, ratio) in [(15.0, 1.69), (30.0, 3.85), (39.0, 10.4), (0.0, 1.0)] {
            let (l, r) = speaker_pan(dir(deg));
            assert!(close(l / r, ratio, 0.01), "{deg}°: {}", l / r);
            let (l, r) = speaker_pan(dir(-deg));
            assert!(close(r / l, ratio, 0.01), "−{deg}°");
        }
        // Behind mirrors the front: 150° as 30°.
        let (l, r) = speaker_pan(dir(150.0));
        assert!(close(l / r, 3.85, 0.01));
        // 60° to the left: the right ear gets nothing.
        assert_eq!(speaker_pan(dir(60.0)).1, 0.0);
        // Levels: ahead 0.64 per ear, the side's near ear 1, behind 0.48,
        // overhead 0.90.
        assert!(close(speaker_pan(dir(0.0)).0, 0.64, 1e-3));
        assert!(close(speaker_pan(dir(90.0)).0, 1.0, 1e-3));
        assert!(close(speaker_pan(dir(180.0)).1, 0.48, 1e-3));
        let (l, r) = speaker_pan([0.0, 1.0, 0.0]);
        assert!(close(l, 0.90, 1e-3) && close(r, 0.90, 1e-3));
    }

    /// The room's wet share and the distance-variant blend (the spec's
    /// cases).
    #[test]
    fn wet_share_and_far_weight() {
        assert!(close(dsp_wet(AUTO_ROOM_MIX, 500.0, 94.0), 0.487, 1e-3));
        assert!(close(dsp_wet(AUTO_ROOM_MIX, 500.0, 75.0), 0.243, 1e-3));
        assert!(close(dsp_wet(AUTO_ROOM_MIX, 2000.0, 94.0), 0.80, 1e-6));
        assert!(close(far_weight(800.0), 0.519, 1e-3));
        assert_eq!(far_weight(100.0), 0.0);
        assert_eq!(far_weight(1400.0), 1.0);
    }
}
