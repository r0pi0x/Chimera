//! The transparent generic shader's stage combiner (specs/halo_ce/
//! shaders.md, "Transparent generic"), on the CPU: `combine` runs the
//! stages over one texel's register values exactly as the spec writes
//! them (inputs and their mappings, products or dot products, the mux,
//! the output mappings and clamps, reads before writes); `tint` uses it to
//! give a shader drawn by its map 0 (`materials::look`) the colour the
//! whole combiner gives on average.
//!
//! **The tint.** The engine draws a generic shader as map 0 times a tint
//! (unlit, blended or added). For a shader whose first stage doesn't simply
//! multiply map 0 by constant colour 0, `tint` samples every map on a grid
//! (each at its own scale and offset, nearest, at time 0), runs the
//! combiner at each point, and divides the mean output colour (weighted by
//! the output alpha unless the shader adds) by map 0's mean brightness
//! weighted by its alpha (what the engine draws of it): map 0's pattern
//! keeps its scroll, and the surface has the combiner's mean colour (the
//! teleporter shield's green, its map 3's colour, which was drawn as map
//! 0's plain white).
//!
//! Known shortcuts: constant colour 0's animation is its function's mean
//! (`materials::periodic_mean`) and "A-out controls constant colour 0"
//! reads it the same way (no object's function A); the vertex colours are
//! white (the spec's inference for unlit shaders); the fade stage and the
//! framebuffer fade are not applied; the mean of the combined colour is
//! not the combined colour at each texel (only the mean hue and
//! brightness are kept).

use blam_formats::{
    cache::MapFile,
    shader::{
        GenericInput, GenericShader, GenericStage, InputMapping, OutputFunction, OutputMapping, PeriodicTables,
        StageOutput,
    },
};

/// The combiner's registers (shaders.md, "Registers start as"): four
/// colours each (rgb, alpha) for the maps, the vertex colours, the
/// scratch registers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Registers {
    pub maps: [[f32; 4]; 4],
    pub vertex: [[f32; 4]; 2],
    pub scratch: [[f32; 4]; 2],
}

/// One stage's constant colours: constant 0 (argb lower .. upper at `f`,
/// clamped) and constant 1, as [r, g, b, a].
fn constants(st: &GenericStage, f: f32) -> [[f32; 4]; 2] {
    let (lo, hi) = (st.constant_color0_lower, st.constant_color0_upper);
    let lerp = |a: f32, b: f32| (a + (b - a) * f).clamp(0.0, 1.0);
    let c1 = st.constant_color1;
    [
        [lerp(lo.r, hi.r), lerp(lo.g, hi.g), lerp(lo.b, hi.b), lerp(lo.a, hi.a)],
        [c1.r, c1.g, c1.b, c1.a],
    ]
}

fn map_value(m: InputMapping, x: f32) -> f32 {
    let c = x.clamp(0.0, 1.0);
    match m {
        InputMapping::Clamp => c,
        InputMapping::OneMinusClamp => 1.0 - c,
        InputMapping::ExpandSigned => 2.0 * c - 1.0,
        InputMapping::OneMinusExpandSigned => 1.0 - 2.0 * c,
        InputMapping::ClampMinusHalf => c - 0.5,
        InputMapping::HalfMinusClamp => 0.5 - c,
        InputMapping::Identity => x,
        InputMapping::Negate => -x,
    }
}

/// A register by its input code's number within its group (0..4: map 0
/// to 3... in the order map 0-3, vertex 0-1, scratch 0-1, constant 0-1).
fn register(r: &Registers, k: &[[f32; 4]; 2], i: u8) -> [f32; 4] {
    match i {
        0..=3 => r.maps[i as usize],
        4 | 5 => r.vertex[(i - 4) as usize],
        6 | 7 => r.scratch[(i - 6) as usize],
        _ => k[(i - 8) as usize],
    }
}

/// A colour input (rgb) before its mapping.
fn colour_input(r: &Registers, k: &[[f32; 4]; 2], input: GenericInput) -> [f32; 3] {
    let code = input as u8;
    match code {
        0 => [0.0; 3],
        1 => [1.0; 3],
        2 => [0.5; 3],
        3 => [-1.0; 3],
        4 => [-0.5; 3],
        5..=14 => {
            let v = register(r, k, code - 5);
            [v[0], v[1], v[2]]
        }
        _ => [register(r, k, code - 15)[3]; 3],
    }
}

/// An alpha input before its mapping: codes 5..14 the alpha, 15..24 the
/// blue channel.
fn alpha_input(r: &Registers, k: &[[f32; 4]; 2], input: GenericInput) -> f32 {
    let code = input as u8;
    match code {
        0 => 0.0,
        1 => 1.0,
        2 => 0.5,
        3 => -1.0,
        4 => -0.5,
        5..=14 => register(r, k, code - 5)[3],
        _ => register(r, k, code - 15)[2],
    }
}

fn out_map(m: OutputMapping, x: f32) -> f32 {
    let y = match m {
        OutputMapping::Identity => x,
        OutputMapping::Half => x * 0.5,
        OutputMapping::Double => x * 2.0,
        OutputMapping::Quadruple => x * 4.0,
        OutputMapping::BiasMinusHalf => x - 0.5,
        OutputMapping::ExpandSigned => 2.0 * x - 1.0,
    };
    y.clamp(-1.0, 1.0)
}

/// The register an output writes (None: discard).
fn target(r: &mut Registers, o: StageOutput) -> Option<&mut [f32; 4]> {
    Some(match o {
        StageOutput::Discard => return None,
        StageOutput::Scratch0 => &mut r.scratch[0],
        StageOutput::Scratch1 => &mut r.scratch[1],
        StageOutput::Vertex0 => &mut r.vertex[0],
        StageOutput::Vertex1 => &mut r.vertex[1],
        StageOutput::Map0 => &mut r.maps[0],
        StageOutput::Map1 => &mut r.maps[1],
        StageOutput::Map2 => &mut r.maps[2],
        StageOutput::Map3 => &mut r.maps[3],
    })
}

/// The combiner's output (scratch colour 0 rgb and scratch alpha 0, as
/// [r, g, b, a]; map 0 with no stages) for `stages` from `regs`, each
/// stage's constant colour 0 at its animation value `f(stage)`.
pub fn combine(stages: &[GenericStage], mut regs: Registers, f: &dyn Fn(&GenericStage) -> f32) -> [f32; 4] {
    if stages.is_empty() {
        return regs.maps[0];
    }
    regs.scratch = [[0.0; 4]; 2];
    for st in stages {
        let k = constants(st, f(st));
        let (ci, ai) = (&st.color_inputs, &st.alpha_inputs);
        let col = |inp: &blam_formats::shader::StageInput| {
            colour_input(&regs, &k, inp.input).map(|x| map_value(inp.mapping, x))
        };
        let alp = |inp: &blam_formats::shader::StageInput| map_value(inp.mapping, alpha_input(&regs, &k, inp.input));
        let (a, b, c, d) = (col(&ci.a), col(&ci.b), col(&ci.c), col(&ci.d));
        let product = |x: [f32; 3], y: [f32; 3], func: OutputFunction| match func {
            OutputFunction::Multiply => [x[0] * y[0], x[1] * y[1], x[2] * y[2]],
            OutputFunction::DotProduct => [x[0] * y[0] + x[1] * y[1] + x[2] * y[2]; 3],
        };
        let co = &st.color_outputs;
        let ab = product(a, b, co.ab_function);
        let cd = product(c, d, co.cd_function);
        let mux = regs.scratch[0][3] >= 0.5;
        let s = if st.flags & GenericStage::COLOR_MUX != 0 {
            if mux { cd } else { ab }
        } else {
            [ab[0] + cd[0], ab[1] + cd[1], ab[2] + cd[2]]
        };
        let (aa, ab_a, ac, ad) = (alp(&ai.a), alp(&ai.b), alp(&ai.c), alp(&ai.d));
        let (pab, pcd) = (aa * ab_a, ac * ad);
        let sa = if st.flags & GenericStage::ALPHA_MUX != 0 {
            if mux { pcd } else { pab }
        } else {
            pab + pcd
        };
        let ao = &st.alpha_outputs;
        // All reads above; now the writes.
        let cm = co.mapping;
        for (o, v) in [(co.ab, ab), (co.cd, cd), (co.mux_sum, s)] {
            if let Some(t) = target(&mut regs, o) {
                for i in 0..3 {
                    t[i] = out_map(cm, v[i]);
                }
            }
        }
        for (o, v) in [(ao.ab, pab), (ao.cd, pcd), (ao.mux_sum, sa)] {
            if let Some(t) = target(&mut regs, o) {
                t[3] = out_map(ao.mapping, v);
            }
        }
    }
    regs.scratch[0]
}

/// The largest tint (linear): where map 0 is dark under a bright output
/// (map 0 isn't what draws the shader), a larger one would blow the
/// surface out to white.
pub const MAX_TINT: f32 = 2.0;

/// Grid points per side `tint` samples.
const GRID: u32 = 48;

/// The tint (linear, per channel, at most `MAX_TINT`) that gives map 0 drawn alone
/// the combiner's mean colour (module doc); None when it has no stages,
/// a map doesn't decode, or map 0 is black.
pub fn tint(file: &MapFile, s: &GenericShader, tables: &PeriodicTables) -> Option<[f32; 3]> {
    if s.stages.is_empty() || s.maps.is_empty() {
        return None;
    }
    let mut levels = Vec::new();
    for m in s.maps.iter().take(4) {
        let tag = file.tag(m.map.id)?;
        let image = blam_formats::bitmap::Bitmap::read(file, tag)
            .ok()?
            .images
            .into_iter()
            .next()?;
        levels.push(image.decode_level(file, 0, 0).ok()?);
    }
    let f = |st: &GenericStage| super::materials::periodic_mean(tables, st.constant_color0_function);
    let added = s.common.framebuffer_blend_function == blam_formats::shader::BlendFunction::Add;
    let (mut sum, mut base) = ([0.0f64; 3], 0.0f64);
    for y in 0..GRID {
        for x in 0..GRID {
            let (u, v) = ((x as f32 + 0.5) / GRID as f32, (y as f32 + 0.5) / GRID as f32);
            let mut regs = Registers {
                vertex: [[1.0; 4]; 2],
                ..Default::default()
            };
            for (i, (l, m)) in levels.iter().zip(&s.maps).enumerate() {
                let su = (u * m.u_scale + m.u_offset).rem_euclid(1.0);
                let sv = (v * m.v_scale + m.v_offset).rem_euclid(1.0);
                let sx = ((su * l.width as f32) as u32).min(l.width.saturating_sub(1));
                let sy = ((sv * l.height as f32) as u32).min(l.height.saturating_sub(1));
                let o = ((sy * l.width + sx) * 4) as usize;
                regs.maps[i] = [0, 1, 2, 3].map(|k| f32::from(l.rgba[o + k]) / 255.0);
            }
            let m0 = regs.maps[0];
            // The engine weights map 0's colour by its alpha (an added
            // surface's colour is premultiplied by it, a blended one's
            // covers by it); the game adds the output colour as it is and
            // blends it by the output alpha.
            base += f64::from((m0[0] + m0[1] + m0[2]) / 3.0 * m0[3]);
            let out = combine(&s.stages, regs, &f);
            let weight = if added { 1.0 } else { out[3].clamp(0.0, 1.0) };
            for i in 0..3 {
                sum[i] += f64::from(out[i].max(0.0) * weight);
            }
        }
    }
    if base <= 1e-6 {
        return None;
    }
    // Ratios in the game's gamma space, then to linear (a ratio's power).
    Some(sum.map(|c| ((c / base) as f32).powf(2.2).min(MAX_TINT)))
}
