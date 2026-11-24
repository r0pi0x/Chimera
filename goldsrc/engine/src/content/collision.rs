//! A GoldSrc map's collision as Chimera's convex shapes, as the M0 spike
//! recommends (docs/plans/active/goldsrc.md §3; specs/goldsrc/bsp.md,
//! "Hulls and collision", "Reconstructing shapes"): the map holds no
//! brushes, so
//!
//! - hull 0's solid (and sky) regions of the world are the solids that
//!   stop everything: shots, bodies and every game's characters
//!   (`MapData::collision_hulls`, and as planes `collision_brushes`);
//! - hull 1's solid regions, shrunk back by the standing box where they
//!   border hull 1's open space (`solids::shrink` with
//!   `Shrink::Measured`, `solids::exposed_in`), are the player clip:
//!   brushes in `player_clips` and `trace_skip`, which characters meet
//!   and shots pass (`core::MapPlayerClips`). A shrunk piece whose every
//!   probe point (its corners pulled a tenth toward its centre, and the
//!   centre) lies in hull 0's solid only repeats hull 0 and is dropped
//!   (`repeats_hull0`).
//!
//! Static solid brush entities (`func_wall`...) add their own hull 0
//! regions, placed by their origin, to the solids; movers carry theirs as
//! their entity's volumes (`entities`). CS 1.6's own movement will trace
//! hulls 1 and 3 directly (M2); these shapes are what other games'
//! characters and every shot meet.
//!
//! Known shortcuts:
//! - Brush entities add no clip pieces of their own (their hull 1 is not
//!   read).
//! - Regions are cut from the model's bounds grown by 64 units
//!   (`solids::convex_regions`): the void around the world is boxed there.

use bevy::math::Vec3;
use goldsrc_formats::bsp::{
    Bsp,
    hull::{HULL_BOXES, blocks},
    solids::{Convex, Shrink, convex_regions, exposed_in, shrink},
};

use super::{METERS_PER_UNIT, to_engine, to_engine_dir};
use crate::{core::MapBrush, map::MapData};

/// A region (units, offset by `offset`) as an engine brush.
pub fn brush(c: &Convex, offset: [f64; 3]) -> MapBrush {
    let planes = c
        .planes
        .iter()
        .map(|h| {
            let n = [h.normal[0] as f32, h.normal[1] as f32, h.normal[2] as f32];
            let d = h.dist + h.normal[0] * offset[0] + h.normal[1] * offset[1] + h.normal[2] * offset[2];
            (to_engine_dir(n), d as f32 * METERS_PER_UNIT)
        })
        .collect();
    let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
    for p in corners(c, offset) {
        min = min.min(Vec3::from(p));
        max = max.max(Vec3::from(p));
    }
    MapBrush {
        planes,
        min,
        max,
        ladder: false,
        surface: None,
    }
}

/// A region's corners in engine space.
pub fn corners(c: &Convex, offset: [f64; 3]) -> Vec<[f32; 3]> {
    c.vertices
        .iter()
        .map(|v| to_engine([(v[0] + offset[0]) as f32, (v[1] + offset[1]) as f32, (v[2] + offset[2]) as f32]).to_array())
        .collect()
}

/// Whether shrunk clip piece `c` only repeats hull 0's solid (module doc).
pub fn repeats_hull0(bsp: &Bsp, c: &Convex) -> bool {
    let n = c.vertices.len().max(1) as f64;
    let centre = [0, 1, 2].map(|a| c.vertices.iter().map(|v| v[a]).sum::<f64>() / n);
    let solid = |p: [f64; 3]| blocks(bsp.contents(0, 0, p));
    solid(centre)
        && c.vertices
            .iter()
            .all(|v| solid([0, 1, 2].map(|a| v[a] * 0.9 + centre[a] * 0.1)))
}

/// What `add_world` added: its counts, for the module's tests and logs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub solids: usize,
    pub clips: usize,
    pub clips_dropped: usize,
}

/// The world's solids and player clip into `data` (module doc).
pub fn add_world(bsp: &Bsp, data: &mut MapData) -> Counts {
    let mut counts = Counts::default();
    for (_, c) in convex_regions(bsp, 0, 0) {
        data.collision_hulls.push(corners(&c, [0.0; 3]));
        data.collision_brushes.push(brush(&c, [0.0; 3]));
        counts.solids += 1;
    }
    let half = HULL_BOXES[1].1.map(|v| v as f64);
    let exposed = exposed_in(bsp, 0, 1);
    for (_, c) in convex_regions(bsp, 0, 1) {
        let Some(s) = shrink(&c, half, Shrink::Measured, &exposed) else { continue };
        if repeats_hull0(bsp, &s) {
            counts.clips_dropped += 1;
            continue;
        }
        let i = data.collision_brushes.len();
        data.collision_brushes.push(brush(&s, [0.0; 3]));
        data.player_clips.push(i);
        data.trace_skip.push(i);
        counts.clips += 1;
    }
    counts
}

/// A static solid brush entity's hull 0 regions (model `model`, placed at
/// `origin` units) into `data`'s solids.
pub fn add_static(bsp: &Bsp, model: usize, origin: [f32; 3], data: &mut MapData) {
    let o = origin.map(f64::from);
    for (_, c) in convex_regions(bsp, model, 0) {
        data.collision_hulls.push(corners(&c, o));
        data.collision_brushes.push(brush(&c, o));
    }
}

/// A ladder volume's regions (model `model` at `origin`) as climbable
/// brushes that traces pass.
pub fn add_ladder(bsp: &Bsp, model: usize, origin: [f32; 3], data: &mut MapData) {
    let o = origin.map(f64::from);
    for (_, c) in convex_regions(bsp, model, 0) {
        let mut b = brush(&c, o);
        b.ladder = true;
        data.trace_skip.push(data.collision_brushes.len());
        data.collision_brushes.push(b);
    }
}
