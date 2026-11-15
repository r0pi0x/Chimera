//! Object functions (specs/halo_ce/objects.md, "Functions" and "Object
//! functions"): the definition in an object tag (`ObjectFunction`, element
//! 0x168) and the pure evaluation of one at a time from its definition, the
//! four incoming inputs and the outgoing values (`ObjectFunction::evaluate`,
//! `evaluate_all`), with the periodic tables the shaders share.
//!
//! Units: periods in seconds, times in seconds (`object_time`), every value
//! a plain number, 0 to 1 unless the bounds say otherwise.

use std::io;

use super::bit_flags;
use crate::cache::{Field, TagStruct, View};
use crate::scenario::coded_enum;
use crate::shader::{PeriodicFunction, PeriodicTables};

/// Ticks of phase between two object table slots in the function clock.
pub const FUNCTION_PHASE_STRIDE: u32 = 57;
/// "At the lower bound" tolerance.
pub const FUNCTION_EPSILON: f32 = 0.0001;
/// Seconds per tick, as stored.
pub const TICK_SECONDS: f32 = 1.0 / 30.0;

bit_flags! {
    /// The function's flags (offset 0).
    pub struct FunctionFlags(u32) {
        INVERT = 0,
        /// The output accumulates: (new + previous) modulo 1.
        ADDITIVE = 1,
        /// Does not deactivate below the lower bound.
        ALWAYS_ACTIVE = 2,
    }
}

coded_enum! {
    /// The transition function (`map to`) applied to the result.
    pub enum TransitionFunction {
        Linear = 0,
        Early = 1,
        VeryEarly = 2,
        Late = 3,
        VeryLate = 4,
        Cosine = 5,
    }
}

coded_enum! {
    /// How the function's result meets its bounds.
    pub enum BoundsMode {
        /// Clip to the bounds.
        Clip = 0,
        /// Clip to the bounds, then normalise to 0..1.
        ClipAndNormalize = 1,
        /// Scale 0..1 to fit the bounds.
        ScaleToFit = 2,
    }
}

impl TransitionFunction {
    /// The transition of `v` (clamped to 0..1 by the caller), computed
    /// directly rather than through the reference's byte table.
    pub fn apply(self, v: f32) -> f32 {
        use std::f32::consts::PI;
        match self {
            TransitionFunction::Linear => v,
            TransitionFunction::Early => v.sqrt(),
            TransitionFunction::VeryEarly => v.sqrt().sqrt(),
            TransitionFunction::Late => v * v,
            TransitionFunction::VeryLate => {
                let s = v * v;
                s * s
            }
            TransitionFunction::Cosine => 0.5 * ((PI * v - PI / 2.0).sin() + 1.0),
        }
    }
}

/// One function's result: the value and whether the function is active
/// (an inactive function is off for its consumers, a light for example).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FunctionOutput {
    pub value: f32,
    pub active: bool,
}

/// What an attachment's function reference (0 none, 1 to 4 outgoing A to
/// D) gives: none is value 1.0, active; a function that does not exist
/// (`outputs` always holds four) reads whatever `outputs` holds there.
pub fn attachment_function(outputs: &[FunctionOutput; 4], reference: i16) -> FunctionOutput {
    match reference {
        1..=4 => outputs[(reference - 1) as usize],
        _ => FunctionOutput {
            value: 1.0,
            active: true,
        },
    }
}

/// The function clock of an object: `(57 s + k) / 30` seconds, `s` the
/// object's slot in the object table (any stable integer will do) and `k`
/// the game tick.
pub fn object_time(slot: u32, tick: u32) -> f32 {
    (FUNCTION_PHASE_STRIDE.wrapping_mul(slot).wrapping_add(tick)) as f32 / 30.0
}

/// An object function definition (element 0x168 of an object's functions
/// block).
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectFunction {
    pub flags: FunctionFlags,
    /// Seconds.
    pub period: f32,
    /// Function input index (0 none, 1 to 4 incoming A to D, 5 to 8
    /// outgoing A to D, anything else reads 0).
    pub scale_period_by: i16,
    /// Out-of-table types (12 and up) read as `One`.
    pub function_type: PeriodicFunction,
    /// Function input index.
    pub scale_function_by: i16,
    pub wobble_function_type: PeriodicFunction,
    /// Seconds; the engine multiplies the time by it (not by its
    /// reciprocal as for the period).
    pub wobble_period: f32,
    /// 0 disables the wobble.
    pub wobble_magnitude: f32,
    /// 0 disables.
    pub square_wave_threshold: f32,
    /// At most 1 disables.
    pub step_count: i16,
    pub map_to: TransitionFunction,
    /// At most 1 disables.
    pub sawtooth_count: i16,
    /// Function input index.
    pub add: i16,
    /// Function input index.
    pub scale_result_by: i16,
    pub bounds_mode: BoundsMode,
    pub lower_bound: f32,
    pub upper_bound: f32,
    /// -1 none, else the outgoing function index 0 to 3 (another value is
    /// treated as none).
    pub turn_off_with: i16,
    /// Multiplies the result when greater than 0.
    pub scale_by: f32,
    /// Filled by the build: 1 / (upper - lower), 0 when the bounds are
    /// equal.
    pub runtime_inverse_range: f32,
    /// 1 / (sawtooth count - 1) when the count is above 1, else 0.
    pub runtime_sawtooth_modulus: f32,
    /// 1 / (step count - 1) when the count is above 1, else 0.
    pub runtime_step_factor: f32,
    /// 1 / period, 0 when the period is 0.
    pub runtime_inverse_period: f32,
    /// The editor's label ("shield glow source").
    pub usage: String,
}

impl Default for ObjectFunction {
    /// A function that is always 1: type `One`, period 1, bounds 0 to 1,
    /// nothing else set, with the runtime fields filled.
    fn default() -> Self {
        ObjectFunction {
            flags: FunctionFlags::default(),
            period: 1.0,
            scale_period_by: 0,
            function_type: PeriodicFunction::One,
            scale_function_by: 0,
            wobble_function_type: PeriodicFunction::One,
            wobble_period: 0.0,
            wobble_magnitude: 0.0,
            square_wave_threshold: 0.0,
            step_count: 0,
            map_to: TransitionFunction::Linear,
            sawtooth_count: 0,
            add: 0,
            scale_result_by: 0,
            bounds_mode: BoundsMode::Clip,
            lower_bound: 0.0,
            upper_bound: 1.0,
            turn_off_with: -1,
            scale_by: 0.0,
            runtime_inverse_range: 1.0,
            runtime_sawtooth_modulus: 0.0,
            runtime_step_factor: 0.0,
            runtime_inverse_period: 1.0,
            usage: String::new(),
        }
    }
}

fn lenient_periodic(v: &View<'_>, offset: usize) -> io::Result<PeriodicFunction> {
    Ok(PeriodicFunction::from_code(v.i16(offset)?).unwrap_or(PeriodicFunction::One))
}

impl TagStruct for ObjectFunction {
    const SIZE: usize = 0x168;

    fn read(v: &View<'_>) -> io::Result<Self> {
        Ok(ObjectFunction {
            flags: Field::read(v, 0x00)?,
            period: v.f32(0x04)?,
            scale_period_by: v.i16(0x08)?,
            function_type: lenient_periodic(v, 0x0A)?,
            scale_function_by: v.i16(0x0C)?,
            wobble_function_type: lenient_periodic(v, 0x0E)?,
            wobble_period: v.f32(0x10)?,
            wobble_magnitude: v.f32(0x14)?,
            square_wave_threshold: v.f32(0x18)?,
            step_count: v.i16(0x1C)?,
            map_to: TransitionFunction::from_code(v.i16(0x1E)?).unwrap_or(TransitionFunction::Linear),
            sawtooth_count: v.i16(0x20)?,
            add: v.i16(0x22)?,
            scale_result_by: v.i16(0x24)?,
            bounds_mode: Field::read(v, 0x26)?,
            lower_bound: v.f32(0x28)?,
            upper_bound: v.f32(0x2C)?,
            turn_off_with: v.i16(0x36)?,
            scale_by: v.f32(0x38)?,
            runtime_inverse_range: v.f32(0x138)?,
            runtime_sawtooth_modulus: v.f32(0x13C)?,
            runtime_step_factor: v.f32(0x140)?,
            runtime_inverse_period: v.f32(0x144)?,
            usage: Field::read(v, 0x148)?,
        })
    }
}

impl ObjectFunction {
    /// Recompute the four runtime fields from the other fields by the
    /// spec's formulas (a synthetic definition must fill them).
    pub fn with_runtime(mut self) -> Self {
        let range = self.upper_bound - self.lower_bound;
        self.runtime_inverse_range = if range == 0.0 { 0.0 } else { 1.0 / range };
        self.runtime_sawtooth_modulus = if self.sawtooth_count > 1 {
            1.0 / (self.sawtooth_count - 1) as f32
        } else {
            0.0
        };
        self.runtime_step_factor = if self.step_count > 1 {
            1.0 / (self.step_count - 1) as f32
        } else {
            0.0
        };
        self.runtime_inverse_period = if self.period == 0.0 { 0.0 } else { 1.0 / self.period };
        self
    }

    /// Whether the stored runtime fields equal the spec's formulas (to a
    /// relative 1e-6).
    pub fn runtime_is_consistent(&self) -> bool {
        let r = self.clone().with_runtime();
        let same = |a: f32, b: f32| (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0);
        same(self.runtime_inverse_range, r.runtime_inverse_range)
            && same(self.runtime_sawtooth_modulus, r.runtime_sawtooth_modulus)
            && same(self.runtime_step_factor, r.runtime_step_factor)
            && same(self.runtime_inverse_period, r.runtime_inverse_period)
    }

    /// Evaluate function number `index` (0 to 3, its position in the
    /// block) at time `time` (seconds, `object_time`).
    ///
    /// `incoming` are the object's inputs A to D this tick; `outgoing` the
    /// four outgoing outputs as they stand: this tick's for the functions
    /// of a lower index (already computed), last tick's for the others
    /// (and for this function itself, which the additive flag and a turn
    /// off with itself read).
    pub fn evaluate(
        &self,
        tables: &PeriodicTables,
        index: usize,
        time: f32,
        incoming: &[f32; 4],
        outgoing: &[FunctionOutput; 4],
    ) -> FunctionOutput {
        let input = |i: i16| -> f32 {
            match i {
                1..=4 => incoming[(i - 1) as usize],
                5..=8 => outgoing[(i - 5) as usize].value,
                _ => 0.0,
            }
        };
        // 1. Scale period by.
        let mut p0 = self.runtime_inverse_period;
        if self.scale_period_by != 0 {
            let v = input(self.scale_period_by);
            if v > 0.0 {
                p0 /= v;
            }
        }
        // 2 to 4. The periodic function, scaled, inverted.
        let mut x = tables.value(self.function_type, time * p0);
        if self.scale_function_by != 0 {
            x *= input(self.scale_function_by);
        }
        if self.flags.contains(FunctionFlags::INVERT) {
            x = 1.0 - x;
        }
        // 5. Wobble.
        if self.wobble_magnitude != 0.0 {
            let w = tables.value(self.wobble_function_type, time * self.wobble_period);
            x += 2.0 * self.wobble_magnitude * (w - 0.5);
        }
        // 6. Square wave.
        if self.square_wave_threshold != 0.0 {
            x = if x > self.square_wave_threshold { 1.0 } else { 0.0 };
        }
        // 7. Steps.
        if self.step_count > 1 {
            x = (self.step_count as f32 * x).floor() * self.runtime_step_factor;
        }
        // 8. Sawtooth: the remainder keeps the sign of x.
        if self.runtime_sawtooth_modulus > 0.0 {
            x %= self.runtime_sawtooth_modulus;
        }
        // 9. Add, 10. scale result.
        if self.add != 0 {
            x = (x + input(self.add)).min(1.0);
        }
        if self.scale_result_by != 0 {
            x *= input(self.scale_result_by);
        }
        // 11, 12. Transition, scale by.
        let mut y = self.map_to.apply(x.clamp(0.0, 1.0));
        if self.scale_by > 0.0 {
            y *= self.scale_by;
        }
        // 13. Bounds.
        let (lo, hi) = (self.lower_bound, self.upper_bound);
        let always = self.flags.contains(FunctionFlags::ALWAYS_ACTIVE);
        let mut active;
        match self.bounds_mode {
            BoundsMode::ScaleToFit => {
                y = y * (hi - lo) + lo;
                active = always || lo + FUNCTION_EPSILON < y;
            }
            BoundsMode::Clip | BoundsMode::ClipAndNormalize => {
                if lo + FUNCTION_EPSILON >= y {
                    y = lo;
                    active = always;
                } else {
                    active = true;
                }
                if y > hi {
                    y = hi;
                }
                if self.bounds_mode == BoundsMode::ClipAndNormalize {
                    y = (y - lo) * self.runtime_inverse_range;
                }
            }
        }
        // 14. Turn off with.
        if (0..4).contains(&self.turn_off_with) && !outgoing[self.turn_off_with as usize].active {
            active = false;
        }
        // 15. Additive.
        if self.flags.contains(FunctionFlags::ADDITIVE) {
            y = (y + outgoing[index & 3].value) % 1.0;
        }
        FunctionOutput { value: y, active }
    }
}

/// Evaluate up to the first four functions in order A to D, each seeing
/// this tick's outputs of the lower ones and `previous` for the rest.
/// Functions the block does not have keep their `previous` output.
pub fn evaluate_all(
    functions: &[ObjectFunction],
    tables: &PeriodicTables,
    time: f32,
    incoming: &[f32; 4],
    previous: [FunctionOutput; 4],
) -> [FunctionOutput; 4] {
    let mut out = previous;
    for (i, f) in functions.iter().take(4).enumerate() {
        out[i] = f.evaluate(tables, i, time, incoming, &out);
    }
    out
}
