//! A GoldSrc map's clip hulls for the movement (`chimera_movement::
//! ClipHulls`, docs/plans/active/shared-movement.md section 3;
//! specs/goldsrc/bsp.md, "Hulls and collision"): hulls 1-3 of the world
//! and of the brush entities that stay put (`func_wall`...), carried in
//! the converted map as plain data (`map::clip_trees::MapClipTrees`:
//! `trees`, filled by `content::maps`) and traced by the formats' own
//! hull trace (`goldsrc_formats::bsp::Bsp::trace`, on a `Bsp` holding only
//! the planes, clip nodes and models: `GoldSrcHulls::new`). A movement
//! that asks for clip hulls (`QuakeConfig::clip_hulls`: CS 1.6's) then
//! walks a GoldSrc map exactly where the original's box may be: player
//! clip, the grown slanted walls and all.
//!
//! `GoldSrcHullsPlugin` (which a GoldSrc game adds) offers the loaded
//! map's trees to the movement: `MapClipHulls` when a map with
//! `MapClipTrees` is there, before the movement's step, on the server and
//! on a predicting client alike (both hold the same converted map).
//!
//! The trace of several models is the nearest contact; a point's contents
//! the world's, else the first placed model's that isn't empty. The hull
//! trace is f64 over the file's f32 planes: the same on every machine.
//!
//! Known shortcuts:
//! - Moving brush entities (doors, lifts, breakables: `logic`'s movers)
//!   are not hull traced: the movement sweeps its box against their
//!   reconstructed volumes, as against every game's movers.
//! - Hull 0 (the point hull, the render tree) is not carried: shots and
//!   small boxes meet the reconstructed solids (`content::collision`).
//! - Water in a clip tree (none in CS 1.6's maps) reads as water, but the
//!   movement's water level still comes from Chimera's water volumes,
//!   which the map conversion doesn't fill yet.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_movement::{ClipHulls, HullContents, HullHit, MapClipHulls, MovementStep};
use goldsrc_formats::bsp::{
    Bsp, ClipNode, Model, Plane,
    hull::{self, HULL_BOXES},
};

use crate::map::clip_trees::{MapClipModel, MapClipTrees};

/// A map's clip trees as the conversion carries them: the planes, the clip
/// nodes, the hull boxes and the world as the first placed model.
pub fn trees(bsp: &Bsp) -> MapClipTrees {
    let mut t = MapClipTrees {
        boxes: [1, 2, 3].map(|h| HULL_BOXES[h]),
        planes: bsp.planes.iter().map(|p| [p.normal[0], p.normal[1], p.normal[2], p.dist]).collect(),
        nodes: bsp.clip_nodes.iter().map(|n| [n.plane, n.children[0] as i32, n.children[1] as i32]).collect(),
        models: Vec::new(),
    };
    place(bsp, 0, [0.0; 3], &mut t);
    t
}

/// Brush model `model` of `bsp` placed at `origin` (units) in `trees`.
pub fn place(bsp: &Bsp, model: usize, origin: [f32; 3], trees: &mut MapClipTrees) {
    if let Some(m) = bsp.models.get(model) {
        trees.models.push(MapClipModel { heads: [m.heads[1], m.heads[2], m.heads[3]], origin });
    }
}

/// A map's clip hulls, traced (module doc).
pub struct GoldSrcHulls {
    /// The planes, clip nodes and placed models only (model i's hull 0
    /// head is unused).
    bsp: Bsp,
    /// Each placed model's origin.
    origins: Vec<[f64; 3]>,
    /// Each placed model's hull roots (a negative one is a contents value:
    /// the whole model is that in this hull, and there is no tree to walk).
    heads: Vec<[i32; 3]>,
    boxes: [(Vec3, Vec3); 3],
}

impl GoldSrcHulls {
    pub fn new(trees: &MapClipTrees) -> Self {
        let bsp = Bsp {
            planes: trees
                .planes
                .iter()
                .map(|p| Plane { normal: [p[0], p[1], p[2]], dist: p[3], kind: 3 })
                .collect(),
            clip_nodes: trees
                .nodes
                .iter()
                .map(|n| ClipNode { plane: n[0], children: [n[1] as i16, n[2] as i16] })
                .collect(),
            models: trees
                .models
                .iter()
                .map(|m| Model {
                    mins: [0.0; 3],
                    maxs: [0.0; 3],
                    origin: m.origin,
                    heads: [0, m.heads[0], m.heads[1], m.heads[2]],
                    vis_leaves: 0,
                    first_face: 0,
                    face_count: 0,
                })
                .collect(),
            ..Default::default()
        };
        Self {
            bsp,
            origins: trees.models.iter().map(|m| m.origin.map(f64::from)).collect(),
            heads: trees.models.iter().map(|m| m.heads).collect(),
            boxes: trees.boxes.map(|(a, b)| (Vec3::from(a), Vec3::from(b))),
        }
    }

    fn local(&self, model: usize, p: Vec3) -> [f64; 3] {
        let o = self.origins[model];
        [p.x as f64 - o[0], p.y as f64 - o[1], p.z as f64 - o[2]]
    }
}

impl ClipHulls for GoldSrcHulls {
    fn boxes(&self) -> &[(Vec3, Vec3)] {
        &self.boxes
    }

    fn contents(&self, hull: usize, point: Vec3) -> HullContents {
        for model in 0..self.origins.len() {
            let head = self.heads[model][hull];
            let c = if head < 0 { head } else { self.bsp.contents(model, hull + 1, self.local(model, point)) };
            let c = match c {
                hull::CONTENTS_EMPTY => continue,
                hull::CONTENTS_WATER => HullContents::Water,
                hull::CONTENTS_SLIME => HullContents::Slime,
                hull::CONTENTS_LAVA => HullContents::Lava,
                _ => HullContents::Solid,
            };
            return c;
        }
        HullContents::Empty
    }

    fn trace(&self, hull: usize, start: Vec3, end: Vec3) -> HullHit {
        let mut best = HullHit { fraction: 1.0, end, normal: Vec3::ZERO, start_solid: false, all_solid: false };
        for model in 0..self.origins.len() {
            let o = self.origins[model];
            let head = self.heads[model][hull];
            let solid = HullHit { fraction: 0.0, end: start, normal: Vec3::ZERO, start_solid: true, all_solid: true };
            if head < 0 {
                // No tree: all one contents.
                if hull::blocks(head) {
                    return solid;
                }
                continue;
            }
            let t = self.bsp.trace(model, hull + 1, self.local(model, start), self.local(model, end));
            // A start in solid stops the move whatever else is nearer.
            if t.started_solid {
                return HullHit { all_solid: t.all_solid, ..solid };
            }
            if (t.fraction as f32) < best.fraction {
                best = HullHit {
                    fraction: t.fraction as f32,
                    end: Vec3::new((t.end[0] + o[0]) as f32, (t.end[1] + o[1]) as f32, (t.end[2] + o[2]) as f32),
                    normal: Vec3::new(t.plane_normal[0] as f32, t.plane_normal[1] as f32, t.plane_normal[2] as f32),
                    start_solid: false,
                    all_solid: false,
                };
            }
        }
        best
    }
}

/// The loaded map's clip trees offered to the movement (module doc).
pub struct GoldSrcHullsPlugin;

impl Plugin for GoldSrcHullsPlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<Self>() {
            return;
        }
        app.add_systems(
            crate::core::Predict::Movement,
            offer.before(MovementStep).run_if(resource_exists_and_changed::<MapClipTrees>),
        );
    }
}

fn offer(trees: Res<MapClipTrees>, mut commands: Commands) {
    commands.insert_resource(MapClipHulls(Arc::new(GoldSrcHulls::new(&trees))));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A floor at z = 0 (hull 1's plane at z = 36) and a wall at x = 100
    /// (hull 1's at x = 84), as one clip tree: node 0 the floor, node 1 the
    /// wall.
    fn trees() -> MapClipTrees {
        MapClipTrees {
            boxes: [1, 2, 3].map(|h| HULL_BOXES[h]),
            planes: vec![[0.0, 0.0, 1.0, 36.0], [1.0, 0.0, 0.0, 84.0], [0.0, 0.0, 1.0, 18.0]],
            // Hull 1: above the floor plane go on to the wall, else solid;
            // in front of the wall plane (x > 84) solid, else empty.
            // Hull 3: the same with the floor at 18 (nodes 2, 3).
            nodes: vec![[0, 1, -2], [1, -2, -1], [2, 3, -2], [1, -2, -1]],
            models: vec![MapClipModel { heads: [0, -2, 2], origin: [0.0; 3] }],
        }
    }

    #[test]
    fn the_specs_floor_traces() {
        let h = GoldSrcHulls::new(&trees());
        // specs/goldsrc/bsp.md, test cases: from z = 100 to z = 0.
        let t = h.trace(0, Vec3::new(0.0, 0.0, 100.0), Vec3::ZERO);
        assert!((t.fraction - 0.639_687_5).abs() < 1e-6, "{t:?}");
        assert!((t.end.z - 36.031_25).abs() < 1e-5 && t.normal == Vec3::Z && !t.start_solid, "{t:?}");
        // From z = 20: in solid from the start.
        let t = h.trace(0, Vec3::new(0.0, 0.0, 20.0), Vec3::ZERO);
        assert!(t.start_solid && t.all_solid && t.fraction == 0.0, "{t:?}");
        // The ducked hull's floor is 18 up, the wall the same.
        let t = h.trace(2, Vec3::new(0.0, 0.0, 30.0), Vec3::new(200.0, 0.0, 30.0));
        assert!((t.end.x - (84.0 - 1.0 / 32.0)).abs() < 1e-4 && t.normal == Vec3::NEG_X, "{t:?}");
        assert_eq!(h.contents(0, Vec3::new(0.0, 0.0, 35.0)), HullContents::Solid);
        assert_eq!(h.contents(0, Vec3::new(0.0, 0.0, 37.0)), HullContents::Empty);
        assert_eq!(h.contents(2, Vec3::new(0.0, 0.0, 19.0)), HullContents::Empty);
        // Hull 2 of this model is solid everywhere (its head is a contents value).
        assert_eq!(h.contents(1, Vec3::new(0.0, 0.0, 500.0)), HullContents::Solid);
    }

    #[test]
    fn a_placed_model_is_traced_at_its_origin() {
        let mut t = trees();
        // The same tree again as a second model, 1000 units up: its floor
        // plane is then at z = 1036.
        t.models.push(MapClipModel { heads: [0, -2, 2], origin: [0.0, 0.0, 1000.0] });
        let h = GoldSrcHulls::new(&t);
        let hit = h.trace(0, Vec3::new(0.0, 0.0, 1100.0), Vec3::new(0.0, 0.0, 1000.0));
        assert!((hit.end.z - 1036.031_25).abs() < 1e-3 && hit.normal == Vec3::Z, "{hit:?}");
        assert_eq!(h.contents(0, Vec3::new(0.0, 0.0, 1035.0)), HullContents::Solid);
    }
}
