//! GoldSrc's content converted into Chimera's types (specs/goldsrc/), one
//! concern per submodule:
//!
//! - `maps`: a BSP 30 map as `MapData` (`GoldSrcGame`: what a game gives
//!   the conversion; `load_level_with`, through the converted-map cache);
//! - `textures`: the map's textures, embedded or from the WADs its
//!   `worldspawn` names, and what each name's prefix means;
//! - `lightmap`: the faces' lightmap blocks packed into one atlas;
//! - `brushes`: the world's and the brush entities' faces as meshes;
//! - `collision`: hull 0's solid regions for everything and hull 1's
//!   regions shrunk back as player clip (docs/plans/active/goldsrc.md §3);
//! - `entities`: the entity list, spawns, brush entities;
//! - `sky`: the six `gfx/env` faces;
//! - `studio`: a studio model (MDL v10) as a character body with its
//!   sequences.
//!
//! Space: GoldSrc's axes and unit are Source's (bsp.md, "Units and
//! conventions"), so a point (x, y, z) in units is the engine's
//! (x, z, −y) × `METERS_PER_UNIT` (`to_engine`), the same mapping as
//! Source's maps and `map::entities`' entity space.

pub mod brushes;
pub mod collision;
pub mod entities;
pub mod lightmap;
pub mod maps;
pub mod sky;
pub mod studio;
pub mod textures;

use bevy::math::Vec3;

/// The unit (the inch) in metres.
pub const METERS_PER_UNIT: f32 = 0.0254;

/// A point in GoldSrc units as the engine's, in metres.
pub fn to_engine(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[2], -v[1]) * METERS_PER_UNIT
}

/// A direction in GoldSrc's axes as the engine's (no scale).
pub fn to_engine_dir(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[2], -v[1])
}

/// An engine point back in GoldSrc units.
pub fn to_game(p: Vec3) -> [f32; 3] {
    let q = p / METERS_PER_UNIT;
    [q.x, -q.z, q.y]
}

/// Parse "x y z" (missing parts 0).
pub fn parse_vector(s: &str) -> [f32; 3] {
    let mut out = [0.0; 3];
    for (o, w) in out.iter_mut().zip(s.split_whitespace()) {
        *o = w.parse().unwrap_or(0.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_space_round_trips() {
        let p = [12.0, -340.5, 64.0];
        let back = to_game(to_engine(p));
        for k in 0..3 {
            assert!((back[k] - p[k]).abs() < 1e-3, "{back:?}");
        }
        // Up is +Y, GoldSrc's +X stays +X, its +Y is the engine's -Z.
        assert_eq!(to_engine_dir([0.0, 0.0, 1.0]), Vec3::Y);
        assert_eq!(to_engine_dir([0.0, 1.0, 0.0]), Vec3::NEG_Z);
        assert_eq!(parse_vector("1 -2.5"), [1.0, -2.5, 0.0]);
    }
}
