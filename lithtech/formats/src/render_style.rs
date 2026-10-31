//! Render-style files (`RS\*.LTB`; specs/combat_arms/models_ltb.md
//! "Render-style file" and Follow-up (2026-10-31) 1): not models (byte 0
//! is 5, container version 3). A header of 20 bytes, the styles' total
//! size and count, then per style its size, a lighting material (68
//! bytes), the pass count and the passes, **each pass record followed by
//! its own "has shader options" byte** (and 10 bytes when it is
//! non-zero: the vertex- and pixel-shader flags and ids), then the style's
//! 8 effect-option bytes. The game registers no shaders, so the
//! fixed-function stages and states are what draws (`Pass`).
//!
//! All numbers are little-endian; an enum is a u32; a bool is one byte
//! and three bytes of padding.

/// How many styles a file has at most, and how big its style data is.
pub const MAX_STYLES: u32 = 32;
pub const MAX_SIZE: u32 = 1_000_000;

/// A style's lighting material (68 bytes): colours as r, g, b, a.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Material {
    pub ambient: [f32; 4],
    pub diffuse: [f32; 4],
    pub emissive: [f32; 4],
    pub specular: [f32; 4],
    pub power: f32,
}

/// One texture stage (120 bytes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stage {
    /// 0 none, 1-4 the piece's texture slot 0-3.
    pub texture: u32,
    pub color_op: u32,
    pub color_arg1: u32,
    pub color_arg2: u32,
    pub alpha_op: u32,
    pub alpha_arg1: u32,
    pub alpha_arg2: u32,
    /// 0-3 the mesh's UV sets, 4 camera-space normal, 5 camera-space
    /// position, 6 camera-space reflection, 7 world-space normal, 8
    /// world-space position, 9 world-space reflection.
    pub coord_source: u32,
    /// 0 wrap, 1 clamp, 2 mirror, 3 mirror once.
    pub u_address: u32,
    pub v_address: u32,
    pub filter: u32,
    pub use_transform: bool,
    /// Row-major 4 x 4, applied to the generated coordinates as a row
    /// vector.
    pub transform: [f32; 16],
    pub projected: bool,
    pub coord_count: u32,
}

/// One pass (532 bytes, then the shader-options byte).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pass {
    pub stages: [Stage; 4],
    /// 0 none, 1 add, 2 saturate, 3 alpha, 4 source colour, 5
    /// destination colour, 6 src.src + dst.dst, 7 src.src + dst, 8 src.a,
    /// 9 src.a + dst, 10 multiply.
    pub blend: u32,
    /// 0 test and write, 1 test only, 2 none.
    pub depth: u32,
    /// 0 none (two-sided), 1 and 2 one-sided.
    pub cull: u32,
    /// The texture factor, ARGB.
    pub factor: u32,
    pub alpha_ref: u32,
    pub dynamic_lighting: bool,
    pub depth_fn: u32,
    /// 0 off, 1 <, 2 <=, 3 >, 4 >=, 5 =, 6 !=.
    pub alpha_fn: u32,
    pub fill: u32,
    pub bump_env: bool,
    pub bump_stage: u32,
    pub bump_scale: f32,
    pub bump_offset: f32,
    /// (use vertex shader, id, use pixel shader, id) when the pass's
    /// shader-options byte was non-zero.
    pub shaders: Option<(bool, i32, bool, i32)>,
}

/// One style: its material, its passes and its effect options.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style {
    pub material: Material,
    pub passes: Vec<Pass>,
    /// (use effect shader, id); none in the 13 stock files that end
    /// after their passes.
    pub effect: Option<(bool, i32)>,
}

/// A render-style file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderStyleFile {
    pub styles: Vec<Style>,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len()).ok_or("render style cut short")?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }
    /// A bool: one byte, three padding.
    fn bool4(&mut self) -> Result<bool, String> {
        Ok(self.take(4)?[0] != 0)
    }
    fn floats<const N: usize>(&mut self) -> Result<[f32; N], String> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = self.f32()?;
        }
        Ok(out)
    }
}

impl RenderStyleFile {
    /// Parse `bytes`; anything that isn't a version 3 render-style file, or
    /// has bytes left over, is refused.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Reader { b: bytes, at: 0 };
        let head = r.take(20)?;
        if head[0] != 5 || u16::from_le_bytes([head[2], head[3]]) != 3 {
            return Err("not a render-style file".into());
        }
        let total = r.u32()?;
        let count = r.u32()?;
        if total > MAX_SIZE || count == 0 || count > MAX_STYLES {
            return Err(format!("render style sizes {total} / {count}"));
        }
        let mut styles = Vec::new();
        for _ in 0..count {
            let size = r.u32()? as usize;
            let start = r.at;
            let material = Material {
                ambient: r.floats()?,
                diffuse: r.floats()?,
                emissive: r.floats()?,
                specular: r.floats()?,
                power: r.f32()?,
            };
            let passes = r.u32()?;
            if !(1..=4).contains(&passes) {
                return Err(format!("render style with {passes} passes"));
            }
            let mut out = Vec::new();
            for _ in 0..passes {
                let mut pass = read_pass(&mut r)?;
                // The shader options follow each pass record.
                if r.take(1)?[0] != 0 {
                    let o = r.take(10)?;
                    pass.shaders = Some((o[0] != 0, i32::from_le_bytes(o[1..5].try_into().unwrap()), o[5] != 0, i32::from_le_bytes(o[6..10].try_into().unwrap())));
                }
                out.push(pass);
            }
            if r.at - start != size {
                return Err(format!("render style data is {} bytes, its size says {size}", r.at - start));
            }
            // The effect options follow when the file has them (every stock
            // file but 13 of the old ones; they hold one style each).
            let effect = if bytes.len() - r.at >= 8 { Some((r.bool4()?, r.i32()?)) } else { None };
            styles.push(Style {
                material,
                passes: out,
                effect,
            });
        }
        if r.at != bytes.len() {
            return Err(format!("{} bytes after the render styles", bytes.len() - r.at));
        }
        Ok(Self { styles })
    }
}

fn read_stage(r: &mut Reader) -> Result<Stage, String> {
    Ok(Stage {
        texture: r.u32()?,
        color_op: r.u32()?,
        color_arg1: r.u32()?,
        color_arg2: r.u32()?,
        alpha_op: r.u32()?,
        alpha_arg1: r.u32()?,
        alpha_arg2: r.u32()?,
        coord_source: r.u32()?,
        u_address: r.u32()?,
        v_address: r.u32()?,
        filter: r.u32()?,
        use_transform: r.bool4()?,
        transform: r.floats()?,
        projected: r.bool4()?,
        coord_count: r.u32()?,
    })
}

fn read_pass(r: &mut Reader) -> Result<Pass, String> {
    let stages = [read_stage(r)?, read_stage(r)?, read_stage(r)?, read_stage(r)?];
    Ok(Pass {
        stages,
        blend: r.u32()?,
        depth: r.u32()?,
        cull: r.u32()?,
        factor: r.u32()?,
        alpha_ref: r.u32()?,
        dynamic_lighting: r.bool4()?,
        depth_fn: r.u32()?,
        alpha_fn: r.u32()?,
        fill: r.u32()?,
        bump_env: r.bool4()?,
        bump_stage: r.u32()?,
        bump_scale: r.f32()?,
        bump_offset: r.f32()?,
        shaders: None,
    })
}

/// Serialise a file the way the game's tool does (tests build fixtures
/// with it): the inverse of `parse`.
pub fn write(file: &RenderStyleFile) -> Vec<u8> {
    fn b4(out: &mut Vec<u8>, v: bool) {
        out.extend([u8::from(v), 0, 0, 0]);
    }
    let mut body = Vec::new();
    for s in &file.styles {
        let mut st = Vec::new();
        for c in [s.material.ambient, s.material.diffuse, s.material.emissive, s.material.specular] {
            for v in c {
                st.extend(v.to_le_bytes());
            }
        }
        st.extend(s.material.power.to_le_bytes());
        st.extend((s.passes.len() as u32).to_le_bytes());
        for p in &s.passes {
            for g in &p.stages {
                for v in [g.texture, g.color_op, g.color_arg1, g.color_arg2, g.alpha_op, g.alpha_arg1, g.alpha_arg2, g.coord_source, g.u_address, g.v_address, g.filter] {
                    st.extend(v.to_le_bytes());
                }
                b4(&mut st, g.use_transform);
                for v in g.transform {
                    st.extend(v.to_le_bytes());
                }
                b4(&mut st, g.projected);
                st.extend(g.coord_count.to_le_bytes());
            }
            for v in [p.blend, p.depth, p.cull, p.factor, p.alpha_ref] {
                st.extend(v.to_le_bytes());
            }
            b4(&mut st, p.dynamic_lighting);
            for v in [p.depth_fn, p.alpha_fn, p.fill] {
                st.extend(v.to_le_bytes());
            }
            b4(&mut st, p.bump_env);
            st.extend(p.bump_stage.to_le_bytes());
            st.extend(p.bump_scale.to_le_bytes());
            st.extend(p.bump_offset.to_le_bytes());
            match p.shaders {
                None => st.push(0),
                Some((vs, vid, ps, pid)) => {
                    st.push(1);
                    st.push(u8::from(vs));
                    st.extend(vid.to_le_bytes());
                    st.push(u8::from(ps));
                    st.extend(pid.to_le_bytes());
                }
            }
        }
        body.extend((st.len() as u32).to_le_bytes());
        body.extend(st);
        // The effect options: 8 bytes, the bool and its id.
        if let Some((on, id)) = s.effect {
            b4(&mut body, on);
            body.extend(id.to_le_bytes());
        }
    }
    let mut out = vec![5, 0, 3, 0];
    out.extend([0; 16]);
    out.extend((body.len() as u32).to_le_bytes());
    out.extend((file.styles.len() as u32).to_le_bytes());
    out.extend(body);
    out
}
