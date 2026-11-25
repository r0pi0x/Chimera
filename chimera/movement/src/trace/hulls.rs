//! Clip hulls: a map's solid space precompiled per box size, traced as a
//! point (the Quake and GoldSrc way; docs/plans/active/shared-movement.md,
//! section 3). `ClipHulls` is what such a map offers: each hull's box
//! around the traced point, a point's contents in a hull, and a point
//! moved through a hull to its first contact (`HullHit`), over the world
//! and the brush models the map placed. A map that has them inserts
//! `MapClipHulls`; a movement whose config sets `QuakeConfig::clip_hulls`
//! then traces the world through the hull whose box is its own (standing
//! or ducked), and every other box, every other movement and every map
//! without hulls through the box sweep (`boxes`). Other characters, moving
//! solids, ladder volumes and props are box sweeps either way.
//!
//! The seam (`HullTracer`): the mover works from the feet (the box's
//! bottom centre), a hull is traced at the point its box is placed around.
//! `HullTracer::offset` is the difference (a box's minimum less the hull's:
//! 36 units up for a 72-high box centred on its point, 18 for a 36-high
//! one), added to a trace's ends and taken off its result; nothing else in
//! the movement knows where a hull's point is.
//!
//! The trait is implemented by a loader (`goldsrc_engine::hulls`, over the
//! BSP's clip node trees), so this crate depends on no loader. An
//! implementation must be a pure function of the map's data: the server
//! and a predicting client trace the same.

use std::sync::Arc;

use bevy::prelude::*;

/// What a point is in, in one hull.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HullContents {
    Empty,
    /// Solid (the sky too): the hull's box may not be here.
    Solid,
    Water,
    Slime,
    Lava,
}

/// A point moved through a hull (units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HullHit {
    /// 0 to 1: how far along the move the point got (1: no contact).
    pub fraction: f32,
    /// Where it got to, kept off the plane hit as the map's engine keeps
    /// it (GoldSrc: 1/32 unit along the plane's normal).
    pub end: Vec3,
    /// The plane hit, facing the open side (zero: none).
    pub normal: Vec3,
    /// The move started in solid: the mover treats the fraction as 0.
    pub start_solid: bool,
    /// All of it was in solid.
    pub all_solid: bool,
}

/// A map's clip hulls (module doc). Units, Z up, as the mover's.
pub trait ClipHulls: Send + Sync {
    /// Each hull's box (minimum, maximum) around the traced point, by hull
    /// index.
    fn boxes(&self) -> &[(Vec3, Vec3)];

    /// `point`'s contents in `hull`: the world's, else the first placed
    /// brush model's that isn't empty.
    fn contents(&self, hull: usize, point: Vec3) -> HullContents;

    /// A point moved from `start` to `end` through `hull`: the nearest
    /// contact of the world's and the placed brush models'.
    fn trace(&self, hull: usize, start: Vec3, end: Vec3) -> HullHit;
}

/// The loaded map's clip hulls, when it has them (GoldSrc maps:
/// `goldsrc_engine::hulls`). Map-scoped.
#[derive(Resource, Clone)]
pub struct MapClipHulls(pub Arc<dyn ClipHulls>);

/// Sizes equal within this are the same box (units).
const SAME_BOX: f32 = 0.01;

/// One character's hull traces this tick: the map's hulls and which of
/// them its standing and ducked boxes are (None: that box is swept).
#[derive(Clone, Copy)]
pub(crate) struct HullTracer<'a> {
    pub(crate) hulls: &'a dyn ClipHulls,
    /// (hull index, feet to hull point) of the standing and the ducked box.
    pub(crate) ids: [Option<(usize, Vec3)>; 2],
}

impl<'a> HullTracer<'a> {
    /// The hull traces of a character whose box is `half_width` wide and
    /// `heights` (standing, ducked) high; None when no hull is its
    /// standing box (the whole movement then sweeps boxes: a body fitted
    /// to another size, a map of other hulls).
    pub(crate) fn new(hulls: &'a dyn ClipHulls, half_width: f32, heights: (f32, f32)) -> Option<Self> {
        let find = |height: f32| {
            let lo = Vec3::new(-half_width, -half_width, 0.0);
            hulls.boxes().iter().enumerate().find_map(|(i, (min, max))| {
                let size = *max - *min;
                let same = (size.x - 2.0 * half_width).abs() < SAME_BOX
                    && (size.y - 2.0 * half_width).abs() < SAME_BOX
                    && (size.z - height).abs() < SAME_BOX;
                same.then_some((i, lo - *min))
            })
        };
        let standing = find(heights.0)?;
        Some(Self {
            hulls,
            ids: [Some(standing), find(heights.1)],
        })
    }

    /// The hull of the box `lo`..`hi` (relative to the feet) when it is the
    /// standing (`heights.0`) or the ducked one, with its offset.
    #[inline]
    pub(crate) fn of(&self, lo: Vec3, hi: Vec3, half_width: f32, heights: (f32, f32)) -> Option<(usize, Vec3)> {
        if lo.z != 0.0 || lo.x != -half_width || hi.x != half_width || lo.y != -half_width || hi.y != half_width {
            return None;
        }
        if hi.z == heights.0 {
            self.ids[0]
        } else if hi.z == heights.1 {
            self.ids[1]
        } else {
            None
        }
    }
}
