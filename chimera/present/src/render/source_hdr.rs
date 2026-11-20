//! Source's integer HDR as numbers (specs/tf2/hdr_bloom.md; the map's
//! `map::HdrModel::Integer`): the percentile auto exposure (the centre
//! region's 16-bin luminance histogram, one bin refreshed per frame, the
//! 2 % / 60 % target with its hold band, the 3 % median floor, the
//! 10-frame odd-weight goal smoothing and its resets), how the drawn
//! scale follows its goal, the bloom amount's per-frame smoothing, the
//! bloom's soft threshold and 13-tap blur, and a CPU reference of the
//! whole bloom on an 8-bit frame for tests. The client's cameras run it
//! (`client::hdr`); the GPU passes (`source_bloom.wesl`,
//! `source_meter.wesl`) use the same constants.
//!
//! Known shortcuts (docs/tech-debt.md): How the drawn scale follows its
//! goal (`follow_goal`) is provisional: the spec's section 4 leaves the
//! engine's law open (Open question 1); taken as an exponential approach
//! with a time constant of 1 s / rate, three times faster when darkening.
//! Captures settle it at once. Why it's OK for now: on TF2's maps the
//! controller's narrow range (2fort 0.8-1.1) bounds any error.

/// Histogram bins over luminance 0..1 (the 17th GPU count is every
/// metered pixel, unused by the algorithm).
pub const BINS: usize = 16;
/// The GPU's counts: the 16 bins and every metered pixel.
pub const COUNTS: usize = BINS + 1;
/// Where the bright pixels are put (`mat_tonemap_percent_target` 60 %).
pub const PERCENT_TARGET: f32 = 0.60;
/// The share of pixels counted as bright (`mat_tonemap_percent_bright_pixels` 2 %).
pub const BRIGHT_SHARE: f32 = 0.02;
/// The floor on the median's luminance (`mat_tonemap_min_avglum` 3 %).
pub const MIN_MEDIAN: f32 = 0.03;
/// The metered region, centred (`mat_exposure_center_region_x / _y`).
pub const CENTER_REGION: (f32, f32) = (0.9, 0.85);
/// Luminance weights for metering (linear values).
pub const METER_LUMA: [f32; 3] = [0.2125, 0.7154, 0.0721];
/// Goal history length, frames.
pub const HISTORY: usize = 10;
/// Frames before an issued bin's count may be read.
pub const READ_DELAY: u64 = 3;
/// The bloom amount's smoothing per frame (`mat_bloomamount_rate`).
pub const BLOOM_RATE: f32 = 0.05;
/// Bloom multiplier on a frame that isn't HDR (`mat_non_hdr_bloom_scalefactor`,
/// `mat_hdr_level` 1).
pub const NON_HDR_BLOOM: f32 = 0.3;
/// The bloom's luminance weights (`r_bloomtintr/g/b`), on gamma values.
pub const BLOOM_TINT: [f32; 3] = [0.3, 0.59, 0.11];
/// The bloom's per-channel exponent (`r_bloomtintexponent`).
pub const BLOOM_EXPONENT: f32 = 2.2;
/// The blur's taps (quarter texels from the centre, weight of each of the
/// pair), the centre first; sampled bilinearly.
pub const BLUR_TAPS: [(f32, f32); 7] = [
    (0.0, 0.2013),
    (1.3366, 0.2185),
    (3.4295, 0.0821),
    (5.4264, 0.0461),
    (7.4359, 0.0262),
    (9.4436, 0.0162),
    (11.4401, 0.0102),
];
/// How much faster the scale moves down than up
/// (`mat_accelerate_adjust_exposure_down`).
pub const DARKEN_FACTOR: f32 = 3.0;
/// The exposure range without a controller (`mat_autoexposure_min / _max`).
pub const DEFAULT_RANGE: (f32, f32) = (0.5, 2.0);

/// Bin `b`'s lower edge (`b` = 16: the top edge), (b / 16)^1.5.
pub fn bin_edge(b: usize) -> f32 {
    (b as f32 / BINS as f32).powf(1.5)
}

/// The bins a luminance counts in: both edges inclusive (a value on an
/// edge counts in both bins), the top bin taking anything above 1.
pub fn bins_of(y: f32) -> impl Iterator<Item = usize> {
    (0..BINS).filter(move |&b| y >= bin_edge(b) && (y <= bin_edge(b + 1) || b == BINS - 1))
}

/// The 17 counts of a frame of linear pixels' luminances (the GPU meter's
/// work, for tests).
pub fn histogram(lumas: impl IntoIterator<Item = f32>) -> [u32; COUNTS] {
    let mut out = [0; COUNTS];
    for y in lumas {
        for b in bins_of(y) {
            out[b] += 1;
        }
        out[BINS] += 1;
    }
    out
}

/// The metered rectangle of a `w` x `h` frame: (x0, y0, x1, y1), ends
/// exclusive.
pub fn meter_region(w: u32, h: u32) -> (u32, u32, u32, u32) {
    let skip = |n: u32, f: f32| ((n as f32 * (1.0 - f) * 0.5).round() as u32).min(n / 2);
    let (sx, sy) = (skip(w, CENTER_REGION.0), skip(h, CENTER_REGION.1));
    (sx, sy, w - sx, h - sy)
}

/// The luminance above which a share `p` of the counted pixels lie (the
/// spec's P(p)), interpolated inside its bin from the bin's top edge
/// down. None: no data (a bin walked has no count yet, or nothing counted).
pub fn bright_point(counts: &[Option<u32>; BINS], p: f32) -> Option<(f32, usize)> {
    let total: u64 = counts.iter().map(|c| c.unwrap_or(0) as u64).sum();
    if total == 0 {
        return None;
    }
    let mut needed = p;
    for b in (0..BINS).rev() {
        let share = counts[b]? as f32 / total as f32;
        if share >= needed && share > 0.0 {
            let (lo, hi) = (bin_edge(b), bin_edge(b + 1));
            return Some((hi - (hi - lo) * (needed / share), b));
        }
        needed -= share;
    }
    // Rounding left a sliver: the bottom of the range.
    Some((0.0, 0))
}

/// The bin holding the target (0.60): its bright point holds the scale.
pub fn target_bin() -> usize {
    bins_of(PERCENT_TARGET).next().unwrap_or(0)
}

/// The exposure range: the controller's values where set and above 0,
/// else the defaults; uncapped 0..20; max never below min.
pub fn resolve_range(custom_min: Option<f32>, custom_max: Option<f32>, uncap: bool) -> (f32, f32) {
    if uncap {
        return (0.0, 20.0);
    }
    let min = custom_min.filter(|v| *v > 0.0).unwrap_or(DEFAULT_RANGE.0);
    let max = custom_max.filter(|v| *v > 0.0).unwrap_or(DEFAULT_RANGE.1);
    (min, max.max(min))
}

/// This frame's target scale from the counts (spec 3.3), `k_now` the
/// scale the metered frame was drawn with, clamped to `range`. None: no
/// data (the goal holds).
pub fn target_scale(counts: &[Option<u32>; BINS], k_now: f32, range: (f32, f32)) -> Option<f32> {
    let (bright, bin) = bright_point(counts, BRIGHT_SHARE)?;
    let bright = if bin == target_bin() { PERCENT_TARGET } else { bright };
    target_from_points(bright, bright_point(counts, 0.5).map(|(m, _)| m), k_now, range)
}

/// Steps 3-6 of spec 3.3 from the two points.
pub fn target_from_points(bright: f32, median: Option<f32>, k_now: f32, range: (f32, f32)) -> Option<f32> {
    let mut r = PERCENT_TARGET / bright.max(0.0001);
    if let Some(m) = median.filter(|m| *m > 0.0) {
        r = r.max(MIN_MEDIAN / m);
    }
    let target = (r * k_now).max(0.001);
    Some(target.clamp(range.0, range.1).max(0.001))
}

/// The goal's 10-frame history (spec 3.4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GoalHistory {
    /// Oldest first.
    pub entries: Vec<f32>,
}

impl GoalHistory {
    /// Push this frame's target; the goal: the target itself until the
    /// history is full, then the odd weighting (|i - 5| / 5, oldest i = 0:
    /// the 5-frame-old entry weighs nothing; Quirks 1), clamped to `range`.
    pub fn push(&mut self, t: f32, range: (f32, f32)) -> f32 {
        if self.entries.len() == HISTORY {
            self.entries.remove(0);
        }
        self.entries.push(t);
        if self.entries.len() < HISTORY {
            return t;
        }
        let (mut sum, mut weights) = (0.0, 0.0);
        for (i, h) in self.entries.iter().enumerate() {
            let w = (i as f32 - 5.0).abs() / 5.0;
            sum += w * h;
            weights += w;
        }
        (sum / weights).clamp(range.0, range.1)
    }
}

/// The staggered histogram (spec 3.2): one bin issued per frame, its count
/// usable `READ_DELAY` frames later, from the frame it was issued in. The
/// GPU counts every bin each frame (cheaper than one pass per bin); this
/// keeps only the issued bin's count from each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meter {
    /// Each count with the frame it was issued in.
    pub counts: [Option<(u32, u64)>; COUNTS],
    /// Issued bins: (bin, frame issued, the count once delivered).
    pub pending: Vec<(usize, u64, Option<u32>)>,
    /// Bins issued so far at the start (in order 0..16).
    started: usize,
}

impl Meter {
    /// Issue this frame's bin: at the start 0..16 in order, then the one
    /// whose last count is the oldest (not one already waiting).
    pub fn issue(&mut self, frame: u64) -> usize {
        let bin = if self.started < COUNTS {
            self.started += 1;
            self.started - 1
        } else {
            (0..COUNTS)
                .filter(|b| !self.pending.iter().any(|p| p.0 == *b))
                .min_by_key(|b| self.counts[*b].map_or(0, |c| c.1 + 1))
                .unwrap_or(0)
        };
        self.pending.push((bin, frame, None));
        bin
    }

    /// The GPU's counts of `frame` (every bin) arrived.
    pub fn deliver(&mut self, frame: u64, counts: &[u32; COUNTS]) {
        for p in self.pending.iter_mut().filter(|p| p.1 == frame) {
            p.2 = Some(counts[p.0]);
        }
    }

    /// Take the counts that may be read at `now`; drop issues whose frame's
    /// counts never came (re-issued later).
    pub fn collect(&mut self, now: u64) {
        let counts = &mut self.counts;
        self.pending.retain(|&(bin, frame, count)| {
            if now < frame + READ_DELAY {
                return true;
            }
            if let Some(c) = count {
                counts[bin] = Some((c, frame));
                return false;
            }
            now < frame + 16
        });
    }

    /// The 16 bins' counts (None where a bin has none yet).
    pub fn bins(&self) -> [Option<u32>; BINS] {
        std::array::from_fn(|b| self.counts[b].map(|c| c.0))
    }
}

/// The provisional catch-up (module doc, Known shortcuts): the drawn scale
/// `k` after `dt` seconds of following `goal` at tone-map rate `rate`.
pub fn follow_goal(k: f32, goal: f32, dt: f32, rate: f32) -> f32 {
    let speed = rate.max(0.0) * if goal < k { DARKEN_FACTOR } else { 1.0 };
    goal + (k - goal) * (-dt.max(0.0) * speed).exp()
}

/// The auto exposure's state: the meter, the goal and the scale drawn with.
#[derive(Clone, Debug, PartialEq)]
pub struct Exposure {
    pub meter: Meter,
    pub history: GoalHistory,
    /// The goal (held while there is no data).
    pub goal: f32,
    /// The scale the frame is drawn with.
    pub scale: f32,
}

impl Default for Exposure {
    fn default() -> Self {
        Self {
            meter: Meter::default(),
            history: GoalHistory::default(),
            goal: 1.0,
            scale: 1.0,
        }
    }
}

impl Exposure {
    /// Level start, the local player's respawn, a new spectated target:
    /// the history emptied, the scale exactly 1 (spec 3.4). The meter
    /// keeps its counts.
    pub fn reset(&mut self) {
        self.history = GoalHistory::default();
        self.goal = 1.0;
        self.scale = 1.0;
    }

    /// One frame: the goal from the counts read by `frame`, then the scale
    /// towards it (`settle`: at once, as captures do).
    pub fn step(&mut self, frame: u64, range: (f32, f32), dt: f32, rate: f32, settle: bool) {
        self.meter.collect(frame);
        if let Some(t) = target_scale(&self.meter.bins(), self.scale, range) {
            self.goal = self.history.push(t, range);
        }
        self.scale = if settle {
            self.goal
        } else {
            follow_goal(self.scale, self.goal, dt, rate)
        };
    }
}

/// The bloom amount A (spec 5.1): from 1 once per launch, smoothed towards
/// the map's bloom scale every frame bloom is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BloomAmount(pub f32);

impl Default for BloomAmount {
    fn default() -> Self {
        Self(1.0)
    }
}

impl BloomAmount {
    pub fn step(&mut self, scale: f32) -> f32 {
        self.0 = BLOOM_RATE * scale + (1.0 - BLOOM_RATE) * self.0;
        self.0
    }
}

/// The frame's bloom B from A: x 0.3 when the frame isn't HDR.
pub fn bloom_amount(a: f32, hdr_frame: bool) -> f32 {
    a * if hdr_frame { 1.0 } else { NON_HDR_BLOOM }
}

/// The soft threshold (spec 5.2) of a gamma colour: c^2.2 x its luminance.
pub fn bloom_shape(c: [f32; 3]) -> [f32; 3] {
    let l = c[0] * BLOOM_TINT[0] + c[1] * BLOOM_TINT[1] + c[2] * BLOOM_TINT[2];
    c.map(|v| v.max(0.0).powf(BLOOM_EXPONENT) * l)
}

/// The blur as a discrete kernel on texels (bilinear sampling spreads each
/// tap over its two texels): weights at offsets 0..=12, symmetric.
pub fn discrete_kernel() -> [f32; 13] {
    let mut k = [0.0; 13];
    k[0] = BLUR_TAPS[0].1;
    for &(o, w) in &BLUR_TAPS[1..] {
        let n = o.floor() as usize;
        let f = o - o.floor();
        k[n] += w * (1.0 - f);
        if n + 1 < 13 {
            k[n + 1] += w * f;
        }
    }
    k
}

/// The vertical pass's offset scale (Quirks 2): its step comes from the
/// target's width, so H / W.
pub fn vertical_scale(w: u32, h: u32) -> f32 {
    h as f32 / w.max(1) as f32
}

/// sRGB decode and encode (the 8-bit frame's).
pub fn srgb_decode(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}
pub fn srgb_encode(l: f32) -> f32 {
    if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }
}

fn byte(v: f32) -> f32 {
    (v.clamp(0.0, 1.0) * 255.0).round() / 255.0
}

/// The bloom on an 8-bit frame (CPU reference of the GPU passes, for
/// tests): `frame` row-major gamma colours 0..1, `w` x `h` (multiples of
/// 4), bloom amount `b`. Returns the frame with the bloom added.
pub fn bloom_reference(frame: &[[f32; 3]], w: usize, h: usize, b: f32) -> Vec<[f32; 3]> {
    let (qw, qh) = (w / 4, h / 4);
    let px = |x: usize, y: usize| frame[y.min(h - 1) * w + x.min(w - 1)];
    // Downsample: four 2x2 averages per quarter texel, each shaped.
    let mut quarter = vec![[0.0f32; 3]; qw * qh];
    for qy in 0..qh {
        for qx in 0..qw {
            let mut sum = [0.0; 3];
            for (bx, by) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
                let mut avg = [0.0; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let c = px(qx * 4 + bx + dx, qy * 4 + by + dy);
                    for i in 0..3 {
                        avg[i] += c[i] * 0.25;
                    }
                }
                let s = bloom_shape(avg);
                for i in 0..3 {
                    sum[i] += s[i] * 0.25;
                }
            }
            quarter[qy * qw + qx] = sum.map(byte);
        }
    }
    // Bilinear sample along one axis at a texel position (centres at n).
    let sample = |img: &[[f32; 3]], x: f32, y: f32| {
        let (x, y) = (x.clamp(0.0, (qw - 1) as f32), y.clamp(0.0, (qh - 1) as f32));
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(qw - 1), (y0 + 1).min(qh - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let at = |x: usize, y: usize| img[y * qw + x];
        let mut out = [0.0; 3];
        for i in 0..3 {
            let top = at(x0, y0)[i] * (1.0 - fx) + at(x1, y0)[i] * fx;
            let bottom = at(x0, y1)[i] * (1.0 - fx) + at(x1, y1)[i] * fx;
            out[i] = top * (1.0 - fy) + bottom * fy;
        }
        out
    };
    let blur = |img: &[[f32; 3]], dir: (f32, f32), gain: f32| {
        let mut out = vec![[0.0f32; 3]; qw * qh];
        for qy in 0..qh {
            for qx in 0..qw {
                let mut s = [0.0; 3];
                for (n, &(o, wt)) in BLUR_TAPS.iter().enumerate() {
                    let sides: &[f32] = if n == 0 { &[0.0] } else { &[o, -o] };
                    for &d in sides {
                        let c = sample(img, qx as f32 + d * dir.0, qy as f32 + d * dir.1);
                        for i in 0..3 {
                            s[i] += c[i] * wt;
                        }
                    }
                }
                out[qy * qw + qx] = s.map(|v| byte(v * gain));
            }
        }
        out
    };
    let horizontal = blur(&quarter, (1.0, 0.0), 1.0);
    let bloom = blur(&horizontal, (0.0, vertical_scale(w as u32, h as u32)), b);
    // Stretched over the screen, added in gamma space, clamped.
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let q = sample(&bloom, (x as f32 + 0.5) / 4.0 - 0.5, (y as f32 + 0.5) / 4.0 - 0.5);
            let c = frame[y * w + x];
            out.push(std::array::from_fn(|i| byte(c[i] + q[i])));
        }
    }
    out
}

/// The drawn scale applied to a linear colour on the 8-bit frame: times
/// `k`, clamped to 1, stored as a byte (gamma).
pub fn scale_to_frame(linear: [f32; 3], k: f32) -> [f32; 3] {
    linear.map(|v| byte(srgb_encode((v * k).clamp(0.0, 1.0))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    fn bytes(c: [f32; 3]) -> [u8; 3] {
        c.map(|v| (v * 255.0).round() as u8)
    }

    /// A uniform frame of `rgb` bytes through the bloom at `b`: the
    /// centre pixel (far from the edges).
    fn uniform(rgb: [u8; 3], b: f32) -> [u8; 3] {
        let (w, h) = (128, 96);
        let frame = vec![rgb.map(|v| v as f32 / 255.0); w * h];
        bytes(bloom_reference(&frame, w, h, b)[(h / 2) * w + w / 2])
    }

    #[test]
    fn bloom_shape_of_grey_and_the_wall() {
        let s = bloom_shape([0.8; 3]);
        assert!(close(s[0], 0.490, 0.001), "{s:?}");
        let c = [157.0 / 255.0, 145.0 / 255.0, 122.0 / 255.0];
        let l = c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11;
        assert!(close(l, 0.573, 0.001), "{l}");
        let s = bloom_shape(c);
        for (got, want) in s.iter().zip([0.197, 0.166, 0.113]) {
            assert!(close(*got, want, 0.001), "{s:?}");
        }
        // Grey g gives g^3.2.
        for (g, want) in [(1.0, 1.0), (0.9, 0.714), (0.6, 0.195), (0.5, 0.109), (0.25, 0.012)] {
            assert!(close(bloom_shape([g; 3])[1], want, 0.001), "{g}");
        }
    }

    #[test]
    fn uniform_frames_at_2forts_bloom() {
        assert_eq!(uniform([128; 3], 0.5), [142; 3]);
        assert_eq!(uniform([204; 3], 0.5), [255; 3], "clamped");
        let wall = uniform([157, 145, 122], 0.5);
        for (got, want) in wall.iter().zip([182u8, 166, 136]) {
            assert!(got.abs_diff(want) <= 1, "{wall:?}");
        }
        // The flame's core.
        let core = uniform([255, 230, 180], 0.5);
        for (got, want) in core.iter().zip([255u8, 255, 234]) {
            assert!(got.abs_diff(want) <= 1, "{core:?}");
        }
        // Dark stays (near) unchanged.
        assert!(uniform([64; 3], 0.5)[0].abs_diff(66) <= 1);
    }

    #[test]
    fn uniform_frames_at_other_amounts() {
        // mat_hdr_level 1 on 2fort: 0.5 x 0.3.
        let b = bloom_amount(0.5, false);
        assert!(close(b, 0.15, 1e-6));
        assert_eq!(uniform([128; 3], b), [132; 3]);
        assert_eq!(uniform([204; 3], b), [223; 3]);
        // No controller: S = 1.
        assert_eq!(uniform([128; 3], 1.0), [156; 3]);
        // Bloom off (mat_disable_bloom 1 skips the passes): unchanged.
        assert_eq!(uniform([128; 3], 0.0), [128; 3]);
    }

    #[test]
    fn horizontal_blur_of_one_texel_is_the_discrete_kernel() {
        let k = discrete_kernel();
        let want = [
            0.2013, 0.1450, 0.0735, 0.0468, 0.0353, 0.0264, 0.0197, 0.0148, 0.0114, 0.0090, 0.0072, 0.0057, 0.0045,
        ];
        for (i, (g, w)) in k.iter().zip(want).enumerate() {
            assert!(close(*g, w, 0.00015), "texel {i}: {g} vs {w}");
        }
        let sum: f32 = k[0] + 2.0 * k[1..].iter().sum::<f32>();
        assert!(close(sum, 0.9999, 0.0002), "{sum}");
    }

    #[test]
    fn vertical_blur_is_narrower_by_h_over_w() {
        assert!(close(vertical_scale(1024, 768), 0.75, 1e-6));
        assert!(close(vertical_scale(1920, 1080), 0.5625, 1e-6));
        let want = [1.0025, 2.5721, 4.0698, 5.5769, 7.0827, 8.5801];
        for (tap, w) in BLUR_TAPS[1..].iter().zip(want) {
            assert!(close(tap.0 * 0.75, w, 0.0001), "{} vs {w}", tap.0 * 0.75);
        }
    }

    #[test]
    fn bloom_amount_settles_per_frame() {
        let mut a = BloomAmount::default();
        let mut seen = vec![];
        for _ in 0..90 {
            seen.push(a.step(0.5));
        }
        assert!(close(seen[0], 0.975, 1e-4));
        assert!(close(seen[13], 0.744, 1e-3));
        assert!(close(seen[89], 0.505, 1e-3));
    }

    #[test]
    fn bin_edges() {
        let want = [
            0.0, 0.0156, 0.0442, 0.0812, 0.125, 0.1747, 0.2296, 0.2894, 0.3536, 0.4219, 0.4941, 0.5701, 0.6495, 0.7324,
            0.8185, 0.9077, 1.0,
        ];
        for (b, w) in want.iter().enumerate() {
            assert!(close(bin_edge(b), *w, 0.0001), "edge {b}");
        }
        assert_eq!(target_bin(), 11);
        // Both edges inclusive; the top bin takes anything above 1.
        assert_eq!(bins_of(bin_edge(4)).collect::<Vec<_>>(), vec![3, 4]);
        assert_eq!(bins_of(1.5).collect::<Vec<_>>(), vec![15]);
        assert_eq!(bins_of(0.0).collect::<Vec<_>>(), vec![0]);
        let h = histogram([0.0, 0.6, 0.61, 0.95]);
        assert_eq!((h[0], h[11], h[15], h[BINS]), (1, 2, 1, 4));
    }

    #[test]
    fn the_metered_region() {
        assert_eq!(meter_region(1000, 1000), (50, 75, 950, 925));
    }

    #[test]
    fn exposure_targets() {
        let range = (0.8, 1.1);
        // r1 0.75, r2 0.15: 0.75, clamped to 0.8.
        assert!(close(target_from_points(0.8, Some(0.2), 1.0, range).unwrap(), 0.8, 1e-6));
        // r1 1.2: clamped to 1.1.
        assert!(close(target_from_points(0.5, Some(0.15), 1.0, range).unwrap(), 1.1, 1e-6));
        // Very dark: r1 6, r2 6, clamped to 2.
        let t = target_from_points(0.1, Some(0.005), 1.0, (0.5, 2.0)).unwrap();
        assert!(close(t, 2.0, 1e-6));
        // Unclamped, the median floor wins.
        let t = target_from_points(0.1, Some(0.001), 1.0, (0.0, 100.0)).unwrap();
        assert!(close(t, 30.0, 1e-3), "{t}");
    }

    #[test]
    fn the_bright_point_in_the_target_bin_holds() {
        // Everything in bin 11: P(0.02) falls there, the scale holds.
        let mut counts = [Some(0); BINS];
        counts[11] = Some(1000);
        let t = target_scale(&counts, 0.9, (0.5, 2.0)).unwrap();
        assert!(close(t, 0.9, 1e-6), "{t}");
        // A bin with no count yet: no data.
        counts[15] = None;
        assert_eq!(target_scale(&counts, 0.9, (0.5, 2.0)), None);
    }

    #[test]
    fn bright_point_interpolates_from_the_top() {
        let mut counts = [Some(0); BINS];
        counts[15] = Some(500);
        let (p, b) = bright_point(&counts, 0.02).unwrap();
        assert_eq!(b, 15);
        assert!(close(p, 1.0 - 0.0923 * 0.02, 1e-4), "{p}");
        assert_eq!(bright_point(&[Some(0); BINS], 0.02), None);
    }

    #[test]
    fn goal_smoothing_weighs_the_ends() {
        let range = (0.0, 10.0);
        let mut h = GoalHistory::default();
        for _ in 0..9 {
            assert_eq!(h.push(1.0, range), 1.0, "not full: the target itself");
        }
        assert!(close(h.push(0.8, range), 0.968, 1e-6));
        let mut h = GoalHistory::default();
        h.push(0.8, range);
        let mut g = 0.0;
        for _ in 0..9 {
            g = h.push(1.0, range);
        }
        assert!(close(g, 0.96, 1e-6));
        // Clamped to the range.
        assert!(close(h.push(1.0, (0.5, 0.9)), 0.9, 1e-6));
    }

    #[test]
    fn ranges() {
        assert_eq!(resolve_range(Some(0.8), Some(1.1), false), (0.8, 1.1));
        assert_eq!(resolve_range(Some(0.0), None, false), DEFAULT_RANGE, "0 isn't a value");
        assert_eq!(resolve_range(Some(1.5), Some(1.0), false), (1.5, 1.5), "max never below min");
        assert_eq!(resolve_range(Some(0.8), Some(1.1), true), (0.0, 20.0));
    }

    #[test]
    fn the_meter_staggers_its_bins() {
        let mut m = Meter::default();
        let counts: [u32; COUNTS] = std::array::from_fn(|b| b as u32 * 10);
        // At the start, 0..16 in order; each readable 3 frames on.
        for f in 0..COUNTS as u64 {
            assert_eq!(m.issue(f), f as usize);
            m.deliver(f, &counts);
            m.collect(f);
        }
        assert!(m.counts[COUNTS - 1].is_none(), "the last one not yet readable");
        m.collect(COUNTS as u64 + 2);
        assert!(m.counts.iter().all(|c| c.is_some()));
        assert_eq!(m.bins()[3], Some(30));
        // Then the oldest count again, bin 0.
        assert_eq!(m.issue(30), 0);
        assert_eq!(m.issue(31), 1);
        // A frame whose counts never came: dropped after a while.
        m.collect(60);
        assert!(m.pending.is_empty());
    }

    #[test]
    fn exposure_converges_and_resets() {
        let mut e = Exposure::default();
        // A frame whose brightest 2 % sit at 0.5 (bin 10): brighten.
        let mut gpu = [0u32; COUNTS];
        gpu[10] = 100;
        gpu[3] = 900;
        gpu[BINS] = 1000;
        for f in 0..60u64 {
            e.meter.issue(f);
            e.meter.deliver(f, &gpu);
            e.step(f, (0.8, 1.1), 1.0 / 60.0, 1.0, true);
        }
        assert!(close(e.scale, 1.1, 1e-4), "{}", e.scale);
        e.reset();
        assert_eq!((e.scale, e.goal, e.history.entries.len()), (1.0, 1.0, 0));
    }

    #[test]
    fn the_provisional_catch_up() {
        // One time constant (1 s at rate 1) brightening: 63 % of the way.
        assert!(close(follow_goal(1.0, 2.0, 1.0, 1.0), 2.0 - (-1.0f32).exp(), 1e-5));
        // Darkening three times faster.
        assert!(close(follow_goal(2.0, 1.0, 1.0 / 3.0, 1.0), 1.0 + (-1.0f32).exp(), 1e-5));
        // Rate 0.1: ten times slower.
        assert!(close(follow_goal(1.0, 2.0, 10.0, 0.1), 2.0 - (-1.0f32).exp(), 1e-5));
    }

    #[test]
    fn overbright_is_scaled_then_clamped() {
        // Overbright 5 at k 0.94: white; a dim colour 6 % darker (linear).
        assert_eq!(bytes(scale_to_frame([5.0, 2.0, 1.2], 0.94)), [255; 3]);
        let c = scale_to_frame([0.5; 3], 0.94);
        assert!(close(srgb_decode(c[0]), 0.47, 0.003), "{c:?}");
    }
}
