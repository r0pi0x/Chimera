//! Sampling one animation (specs/halo_ce/animations.md, "Uncompressed
//! frame data", "Compressed animation data", "Sampling a frame"): the
//! local rotation, translation and scale of every node at an integer frame
//! (`Animation::sample`) or at a fractional one for the continuous overlays
//! (`Animation::sample_continuous`), from the default data and the frame
//! data by the per-node flag bits, or from the Xbox's compressed key
//! stream. Also `decode_compressed_rotation`, the 12-bit-per-component
//! quaternion of the compressed form.
//!
//! An uncompressed rotation is returned as decoded (`c / 32767`, close to
//! unit length, **not** normalised, as the spec's expected samples are);
//! a compressed one is normalised, as the spec says every use of it is.

use std::io;

use super::pose::{NodePose, Pose, lerp3, quat_nlerp, quat_normalize};
use super::{Animation, QUAT16_DIVISOR};

/// Bytes of one uncompressed rotation, translation, scale.
const ROTATION_BYTES: usize = 8;
const TRANSLATION_BYTES: usize = 12;
const SCALE_BYTES: usize = 4;
/// Bytes of the compressed header before the rotation node headers.
const COMPRESSED_HEADER: usize = 0x2C;
/// Bytes of one compressed rotation key.
const COMPRESSED_ROTATION_BYTES: usize = 6;
/// Low bits of a compressed node header: the key count.
const KEY_COUNT_MASK: u32 = 0xFFF;
const KEY_COUNT_BITS: u32 = 12;

fn err(name: &str, msg: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("animation `{name}`: {msg}"))
}

fn le_f32(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn le_i16(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

fn rotation16(b: &[u8], at: usize) -> [f32; 4] {
    let c = |k: usize| f32::from(le_i16(b, at + 2 * k)) / QUAT16_DIVISOR;
    [c(0), c(1), c(2), c(3)]
}

fn translation32(b: &[u8], at: usize) -> [f32; 3] {
    [le_f32(b, at), le_f32(b, at + 4), le_f32(b, at + 8)]
}

/// One compressed rotation (6 bytes, little-endian words w0, w1, w2 forming
/// the 48-bit number `w0 << 32 | w1 << 16 | w2`): four 12-bit fields i, j,
/// k, w from the top, each widened to 16 bits by repeating its top four
/// bits below it, read as signed and divided by 32767. **Not** normalised.
pub fn decode_compressed_rotation(b: [u8; 6]) -> [f32; 4] {
    let w0 = u64::from(u16::from_le_bytes([b[0], b[1]]));
    let w1 = u64::from(u16::from_le_bytes([b[2], b[3]]));
    let w2 = u64::from(u16::from_le_bytes([b[4], b[5]]));
    let x = (w0 << 32) | (w1 << 16) | w2;
    let field = |shift: u32| {
        let c = ((x >> shift) & 0xFFF) as u16;
        let wide = ((c << 4) | (c >> 8)) as i16;
        f32::from(wide) / QUAT16_DIVISOR
    };
    [field(36), field(24), field(12), field(0)]
}

/// A compressed node header: the index of its first key and its key count.
pub fn split_node_header(header: u32) -> (usize, usize) {
    ((header >> KEY_COUNT_BITS) as usize, (header & KEY_COUNT_MASK) as usize)
}

impl Animation {
    /// Sample at integer `frame` (`0..frame_count`): every node's
    /// rotation, translation and scale (the parts without a bit come from
    /// the default data). Errors name the animation.
    pub fn sample(&self, frame: usize) -> io::Result<Pose> {
        self.sample_continuous_inner(frame as f32, frame, false)
    }

    /// The continuous sample at fractional frame `x` (`0 <= x <= N`): frame
    /// `a = floor(x)` (`N - 1` with fraction 1 when `a >= N`), fraction
    /// `u`, the next frame `a + 1` (0 after the last, it wraps); an
    /// uncompressed animation blends rotations by nlerp and translations
    /// and scales linearly. A compressed one samples rotations at the
    /// integer frame `a` (the fraction is ignored: the spec's quirk) and
    /// translations and scales at `x`.
    pub fn sample_continuous(&self, x: f32) -> io::Result<Pose> {
        let n = self.frame_count.max(0) as usize;
        if n == 0 {
            return Err(err(&self.name, "has no frames"));
        }
        let x = if x.is_nan() { 0.0 } else { x.max(0.0) };
        let (a, _) = split_frame(x, n);
        self.sample_continuous_inner(x, a, true)
    }

    fn sample_continuous_inner(&self, x: f32, a: usize, continuous: bool) -> io::Result<Pose> {
        let n = self.frame_count.max(0) as usize;
        if a >= n {
            return Err(err(&self.name, format_args!("frame {a} but it has {n} frames")));
        }
        if self.is_compressed() {
            let s = CompressedStream::parse(self)?;
            // Rotation at the integer frame, the rest at x.
            let at = if continuous { x } else { a as f32 };
            return s.sample(self, a as f32, at);
        }
        if !continuous {
            return self.sample_uncompressed(a);
        }
        let (a, u) = split_frame(x, n);
        let next = if a + 1 >= n { 0 } else { a + 1 };
        let p = self.sample_uncompressed(a)?;
        let q = self.sample_uncompressed(next)?;
        Ok(Pose {
            nodes: p
                .nodes
                .iter()
                .zip(&q.nodes)
                .map(|(p, q)| NodePose {
                    rotation: quat_nlerp(p.rotation, q.rotation, u),
                    translation: lerp3(p.translation, q.translation, u),
                    scale: (1.0 - u) * p.scale + u * q.scale,
                })
                .collect(),
        })
    }

    fn sample_uncompressed(&self, frame: usize) -> io::Result<Pose> {
        let nodes = self.node_count();
        let fs = self.computed_frame_size();
        let ds = self.computed_default_size();
        let start = frame
            .checked_mul(fs)
            .ok_or_else(|| err(&self.name, "frame offset overflows"))?;
        let frame_bytes = self.frame_data.get(start..start + fs).ok_or_else(|| {
            err(
                &self.name,
                format_args!(
                    "frame {frame} ({fs} bytes) is outside its {} bytes of frame data",
                    self.frame_data.len()
                ),
            )
        })?;
        if self.default_data.len() < ds {
            return Err(err(
                &self.name,
                format_args!("{} bytes of default data, {ds} needed", self.default_data.len()),
            ));
        }
        let (mut fc, mut dc) = (0usize, 0usize);
        let mut out = Vec::with_capacity(nodes);
        for n in 0..nodes {
            let rotation = if self.has_rotation(n) {
                let r = rotation16(frame_bytes, fc);
                fc += ROTATION_BYTES;
                r
            } else {
                let r = rotation16(&self.default_data, dc);
                dc += ROTATION_BYTES;
                r
            };
            let translation = if self.has_translation(n) {
                let t = translation32(frame_bytes, fc);
                fc += TRANSLATION_BYTES;
                t
            } else {
                let t = translation32(&self.default_data, dc);
                dc += TRANSLATION_BYTES;
                t
            };
            let scale = if self.has_scale(n) {
                let s = le_f32(frame_bytes, fc);
                fc += SCALE_BYTES;
                s
            } else {
                let s = le_f32(&self.default_data, dc);
                dc += SCALE_BYTES;
                s
            };
            out.push(NodePose {
                rotation,
                translation,
                scale,
            });
        }
        Ok(Pose { nodes: out })
    }
}

/// `x` as a frame and a fraction (`a >= N` becomes `N - 1` and 1).
fn split_frame(x: f32, n: usize) -> (usize, f32) {
    let a = x.floor();
    if a >= n as f32 {
        (n - 1, 1.0)
    } else {
        (a as usize, x - a)
    }
}

/// The offsets of a compressed stream's sections (reads are bounds-checked).
struct CompressedStream<'a> {
    data: &'a [u8],
    /// Per kind (rotation, translation, scale): node headers, key times,
    /// defaults, keys offsets.
    sections: [[usize; 4]; 3],
}

impl<'a> CompressedStream<'a> {
    fn parse(a: &'a Animation) -> io::Result<Self> {
        let d = &a.frame_data[..];
        let off = a.compressed_data_offset;
        if off < 0 || off as usize > d.len() {
            return Err(err(
                &a.name,
                format_args!("compressed data offset {off} outside the stream"),
            ));
        }
        let data = &d[off as usize..];
        if data.len() < COMPRESSED_HEADER {
            return Err(err(&a.name, "compressed stream shorter than its header"));
        }
        // An offset outside the stream is not refused here: reads through
        // it fail and that node gets identity, zero, one (the spec's edge
        // case).
        let h = |i: usize| -> io::Result<usize> {
            let v = i32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
            Ok(usize::try_from(v).unwrap_or(usize::MAX / 2))
        };
        Ok(CompressedStream {
            data,
            sections: [
                // Rotation headers sit right after the fixed header.
                [COMPRESSED_HEADER, h(0x00)?, h(0x04)?, h(0x08)?],
                [h(0x0C)?, h(0x10)?, h(0x14)?, h(0x18)?],
                [h(0x1C)?, h(0x20)?, h(0x24)?, h(0x28)?],
            ],
        })
    }

    fn bytes(&self, at: usize, n: usize) -> Option<&'a [u8]> {
        self.data.get(at..at.checked_add(n)?)
    }

    fn header(&self, kind: usize, rank: usize) -> Option<(usize, usize)> {
        let b = self.bytes(self.sections[kind][0].saturating_add(4 * rank), 4)?;
        Some(split_node_header(u32::from_le_bytes([b[0], b[1], b[2], b[3]])))
    }

    fn time(&self, kind: usize, i: usize) -> Option<u16> {
        let b = self.bytes(self.sections[kind][1].saturating_add(2 * i), 2)?;
        Some(u16::from_le_bytes([b[0], b[1]]))
    }

    fn rotation_at(&self, section: usize, i: usize) -> Option<[f32; 4]> {
        let b = self.bytes(
            section.saturating_add(COMPRESSED_ROTATION_BYTES * i),
            COMPRESSED_ROTATION_BYTES,
        )?;
        Some(decode_compressed_rotation(b.try_into().ok()?))
    }

    fn translation_at(&self, section: usize, i: usize) -> Option<[f32; 3]> {
        let b = self.bytes(section.saturating_add(TRANSLATION_BYTES * i), TRANSLATION_BYTES)?;
        Some(translation32(b, 0))
    }

    fn scale_at(&self, section: usize, i: usize) -> Option<f32> {
        let b = self.bytes(section.saturating_add(SCALE_BYTES * i), SCALE_BYTES)?;
        Some(le_f32(b, 0))
    }

    /// `rot_frame` locates the rotation keys, `x` the translations and
    /// scales.
    fn sample(&self, a: &Animation, rot_frame: f32, x: f32) -> io::Result<Pose> {
        let nodes = a.node_count();
        let (mut rr, mut tr, mut sr) = (0usize, 0usize, 0usize);
        let mut out = Vec::with_capacity(nodes);
        for n in 0..nodes {
            // Kind 0 rotation, 1 translation, 2 scale; the defaults of
            // rotations and translations are indexed by node, a scale's by
            // the scale node's rank.
            let rotation = {
                let animated = a.has_rotation(n);
                let rank = rr;
                rr += usize::from(animated);
                self.node_rotation(animated.then_some(rank), n, rot_frame)
            };
            let translation = {
                let animated = a.has_translation(n);
                let rank = tr;
                tr += usize::from(animated);
                self.node_translation(animated.then_some(rank), n, x)
            };
            let scale = {
                let animated = a.has_scale(n);
                let rank = sr;
                sr += usize::from(animated);
                self.node_scale(animated.then_some(rank), rank, x)
            };
            out.push(NodePose {
                rotation: rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]),
                translation: translation.unwrap_or([0.0; 3]),
                scale: scale.unwrap_or(1.0),
            });
        }
        Ok(Pose { nodes: out })
    }

    /// `None`: a key range or default outside the stream (the spec's
    /// "identity, zero, one" for that node).
    fn node_rotation(&self, rank: Option<usize>, node: usize, x: f32) -> Option<[f32; 4]> {
        let defaults = self.sections[0][2];
        let keys = self.sections[0][3];
        let default = self.rotation_at(defaults, node)?;
        let Some(rank) = rank else {
            return Some(quat_normalize(default));
        };
        let (first, count) = self.header(0, rank)?;
        self.interpolate(
            count,
            x,
            |i| self.time(0, first + i),
            |i| self.rotation_at(keys, first + i),
            default,
            quat_nlerp,
            quat_normalize,
        )
    }

    fn node_translation(&self, rank: Option<usize>, node: usize, x: f32) -> Option<[f32; 3]> {
        let defaults = self.sections[1][2];
        let keys = self.sections[1][3];
        let default = self.translation_at(defaults, node)?;
        let Some(rank) = rank else {
            return Some(default);
        };
        let (first, count) = self.header(1, rank)?;
        self.interpolate(
            count,
            x,
            |i| self.time(1, first + i),
            |i| self.translation_at(keys, first + i),
            default,
            lerp3,
            |v| v,
        )
    }

    fn node_scale(&self, rank: Option<usize>, scale_rank: usize, x: f32) -> Option<f32> {
        let defaults = self.sections[2][2];
        let keys = self.sections[2][3];
        let Some(rank) = rank else {
            return Some(1.0);
        };
        let default = self.scale_at(defaults, scale_rank)?;
        let (first, count) = self.header(2, rank)?;
        self.interpolate(
            count,
            x,
            |i| self.time(2, first + i),
            |i| self.scale_at(keys, first + i),
            default,
            |a, b, u| (1.0 - u) * a + u * b,
            |v| v,
        )
    }

    /// The spec's key rule for one node: `count` keys with times `time(i)`
    /// and values `key(i)`, `default` the value at frame 0 (and after the
    /// last key).
    #[allow(clippy::too_many_arguments)]
    fn interpolate<T: Copy>(
        &self,
        count: usize,
        x: f32,
        time: impl Fn(usize) -> Option<u16>,
        key: impl Fn(usize) -> Option<T>,
        default: T,
        mix: impl Fn(T, T, f32) -> T,
        finish: impl Fn(T) -> T,
    ) -> Option<T> {
        if count == 0 {
            return Some(finish(default));
        }
        let fi = x.floor();
        let last = count - 1;
        let k_last = f32::from(time(last)?);
        // From (a, va) to (b, vb): the value at a when x is a.
        let between = |a: f32, va: T, b: f32, vb: T| -> T {
            if x == a {
                finish(va)
            } else {
                mix(va, vb, (x - a) / (b - a))
            }
        };
        if fi > k_last {
            return Some(finish(key(last)?));
        }
        let k0 = f32::from(time(0)?);
        if fi < k0 {
            return Some(between(0.0, default, k0, key(0)?));
        }
        if fi == k_last {
            return Some(between(k_last, key(last)?, k_last + 1.0, default));
        }
        // The key m with k_m <= fi < k_(m+1), by binary search.
        let (mut lo, mut hi) = (0usize, last);
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if f32::from(time(mid)?) <= fi {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(between(
            f32::from(time(lo)?),
            key(lo)?,
            f32::from(time(lo + 1)?),
            key(lo + 1)?,
        ))
    }
}
