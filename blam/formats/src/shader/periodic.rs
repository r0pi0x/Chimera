//! The periodic animation functions of the shader tags (specs/halo_ce/
//! shaders.md, "Periodic (animation) functions"): the twelve 1024-entry
//! byte tables, built once from the spec's generator (`PeriodicTables::new`:
//! the caller holds the value, there is no process-wide state), and
//! `PeriodicTables::value`, the value of a function at an input measured in
//! periods. A tag "period"
//! covers 0.7 of a table cycle (a quirk the spec says to keep), so the
//! functions repeat every 1/0.7 periods.

use super::PeriodicFunction;
use std::f32::consts::PI;

/// Entries per table.
pub const TABLE_SIZE: usize = 1024;
/// Table entries one unit of input advances.
pub const INDEX_RATE: f32 = 25.6;
/// Cosine cycles stored in one table.
pub const TABLE_CYCLES: f32 = 28.0;
/// The generator's seed.
pub const SEED: u32 = 0x20F3_F660;

type Tables = [[u8; TABLE_SIZE]; 12];

fn frac(v: f32) -> f32 {
    v - v.floor()
}

fn triangle(x: f32) -> f32 {
    let f = frac(x);
    if f < 0.5 { 2.0 * f } else { 2.0 - 2.0 * f }
}

fn build() -> Tables {
    let mut seed = SEED;
    let mut random = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 16) as f32 / 65535.0
    };
    let mut tables = [[0u8; TABLE_SIZE]; 12];
    for (code, table) in tables.iter_mut().enumerate() {
        // The stretched x table: built for every function (it consumes
        // four random values per entry), used by the variable-period
        // functions and spark.
        let mut stretched = vec![0f32; TABLE_SIZE + 1];
        for i in 0..TABLE_SIZE {
            let fi = i as f32;
            let r1 = random();
            let r2 = random();
            let r3 = random();
            let r4 = random();
            stretched[i + 1] = stretched[i]
                + 0.25 * r1
                + 0.25
                + ((8.2 * PI * fi / 1024.0).cos() + 1.0) * r2
                + ((10.2 * PI * fi / 1024.0).cos() + 1.0) * r3
                + ((14.6 * PI * fi / 1024.0).cos() + 1.0) * r4;
        }
        let total = stretched[TABLE_SIZE];
        for v in stretched.iter_mut() {
            *v /= total;
        }
        let mut raw = [0f32; TABLE_SIZE];
        for (i, out) in raw.iter_mut().enumerate() {
            let x = i as f32 * 0.027_343_748;
            let xr = 28.0 * stretched[i];
            *out = match code {
                2 => (2.0 * PI * x).cos(),
                3 => (2.0 * PI * xr).cos(),
                4 => triangle(x),
                5 => triangle(xr),
                6 => frac(x),
                7 => frac(xr),
                8 => random(),
                9 | 10 => {
                    ((0.897_597_9 * x).cos() * (25.132_742 * x).cos() + (43.9823 * x).cos() * (1.570_796_4 * x).sin())
                        / 2.0
                        + (PI * x).sin() * (2.0 * PI * x).cos()
                }
                11 => frac(xr) * frac(xr),
                _ => 0.0,
            };
        }
        let normalise = !matches!(code, 6 | 7);
        let (min, max) = raw
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        for (i, &v) in raw.iter().enumerate() {
            let v = if normalise && max > min {
                (v - min) / (max - min)
            } else {
                v
            };
            table[i] = (255.0 * v).clamp(0.0, 255.0) as u8;
        }
    }
    tables
}

/// The twelve byte tables. Building them takes about 50 000 generator
/// steps: build once and keep the value (a renderer's resource).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeriodicTables(Box<Tables>);

impl Default for PeriodicTables {
    fn default() -> Self {
        Self::new()
    }
}

impl PeriodicTables {
    /// Build the tables from the spec's generator.
    pub fn new() -> Self {
        PeriodicTables(Box::new(build()))
    }

    /// The byte table of a function (all zero for `One` and `Zero`, which
    /// are evaluated without one).
    pub fn table(&self, function: PeriodicFunction) -> &[u8; TABLE_SIZE] {
        &self.0[function.code() as usize]
    }

    /// The value (0..1) of `function` at input `s`, in periods. `One` is 1
    /// and `Zero` 0 without a table; the others read the table at `25.6 s`
    /// and interpolate linearly between neighbours (wrapping at 1024).
    /// Slides wrap their discontinuity; the rest clamp.
    pub fn value(&self, function: PeriodicFunction, s: f32) -> f32 {
        match function {
            PeriodicFunction::One => return 1.0,
            PeriodicFunction::Zero => return 0.0,
            _ => {}
        }
        let t = self.table(function);
        let p = INDEX_RATE * s;
        let floor = p.floor();
        let f = p - floor;
        let i = (floor as i64).rem_euclid(TABLE_SIZE as i64) as usize;
        let a = t[i] as f32 / 255.0;
        let mut b = t[(i + 1) % TABLE_SIZE] as f32 / 255.0;
        if matches!(
            function,
            PeriodicFunction::Slide | PeriodicFunction::SlideVariablePeriod
        ) {
            if a > 0.75 && b < 0.25 {
                b += 1.0;
            }
            let v = (1.0 - f) * a + f * b;
            if v > 1.0 { v - 1.0 } else { v }
        } else {
            ((1.0 - f) * a + f * b).clamp(0.0, 1.0)
        }
    }

    /// `value` at `(time + phase) / period`, a period of 0 counting as 1
    /// (how the game guards the texture animation).
    pub fn value_at(&self, function: PeriodicFunction, time: f32, period: f32, phase: f32) -> f32 {
        let period = if period == 0.0 { 1.0 } else { period };
        self.value(function, (time + phase) / period)
    }
}
