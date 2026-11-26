//! A GoldSrc map's liquids as Chimera's water volumes (`MapData::water`,
//! read by every game's movement: swimming, water jumps, the splash),
//! from specs/goldsrc/bsp.md ("Contents") and entities.md ("func_water",
//! "func_wall, func_illusionary, func_wall_toggle"):
//!
//! - the world's: every path of hull 0's tree to a water, slime or lava
//!   leaf (`liquid`), a convex region cut out of the world's bounds, as a
//!   volume (`add_world`);
//! - a brush entity's: a `func_water`, or a `func_illusionary`, whose
//!   `skin` is a liquid's contents value (the 50 stock `func_water`s and
//!   `cs_italy`'s `*13` carry −3) gives its model's filled hull 0 regions
//!   (any end but empty: the brush's volume, whatever its own leaves
//!   hold), placed at its origin (`add_entity`). One that moves (a named
//!   `func_water` rises and falls like a door when fired) is a
//!   `MapData::water_movers` entry: its volumes follow its node.
//!
//! Known shortcuts:
//! - Slime and lava are both `slime` volumes, lava also `lava` (the
//!   install has neither).
//! - Currents are not read (the compiler's current contents never reach a
//!   compiled tree: bsp.md; `MapWaterVolume::current` is for custom maps), nor the clip trees' rare water ends (none in
//!   cstrike's maps).
//! - What is drawn for an eye under water (fog, tint) is not specified;
//!   none is drawn.

use goldsrc_formats::bsp::{
    Bsp, Entity,
    hull::{CONTENTS_LAVA, CONTENTS_WATER, filled, liquid},
    solids::convex_regions_where,
};

use super::collision::brush;
use crate::{core::MapWaterVolume, map::MapData};

/// A contents value as a volume's kind: `Some(slime)` for a liquid
/// (water: false; slime and lava: true), `None` otherwise.
pub fn slime_of(contents: i32) -> Option<bool> {
    liquid(contents).then_some(contents != CONTENTS_WATER)
}

/// The world's liquid leaves (module doc) into `data.water`; how many.
pub fn add_world(bsp: &Bsp, data: &mut MapData) -> usize {
    let regions = convex_regions_where(bsp, 0, 0, liquid);
    let n = regions.len();
    for (contents, c) in regions {
        data.water.push(MapWaterVolume {
            brush: brush(&c, [0.0; 3]),
            slime: contents != CONTENTS_WATER,
            lava: contents == CONTENTS_LAVA,
            current: None,
        });
    }
    n
}

/// The liquid entity `e`'s contents (its `skin`), when it is one: a
/// `func_water` or `func_illusionary` with a liquid's `skin`.
pub fn entity_liquid(e: &Entity) -> Option<i32> {
    let class = e.classname().to_ascii_lowercase();
    if class != "func_water" && class != "func_illusionary" {
        return None;
    }
    let skin = e.get("skin")?.trim().parse::<f32>().ok()? as i32;
    liquid(skin).then_some(skin)
}

/// Brush entity `e` (model `model` at `origin` units, entity `index`)
/// into `data.water` when it is a liquid (`entity_liquid`); `moves`: its
/// volumes follow its node (`water_movers`). How many volumes it added.
pub fn add_entity(
    bsp: &Bsp,
    e: &Entity,
    model: usize,
    origin: [f32; 3],
    index: usize,
    moves: bool,
    data: &mut MapData,
) -> usize {
    let Some(skin) = entity_liquid(e) else { return 0 };
    let Some(slime) = slime_of(skin) else { return 0 };
    let o = origin.map(f64::from);
    let regions = convex_regions_where(bsp, model, 0, filled);
    let n = regions.len();
    for (_, c) in regions {
        if moves {
            data.water_movers.push((data.water.len(), index));
        }
        data.water.push(MapWaterVolume {
            brush: brush(&c, o),
            slime,
            lava: skin == CONTENTS_LAVA,
            current: None,
        });
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use goldsrc_formats::bsp::{Child, Leaf, Model, Node, Plane, hull::CONTENTS_SOLID};

    fn leaf(contents: i32) -> Leaf {
        Leaf {
            contents,
            vis_offset: -1,
            mins: [0; 3],
            maxs: [0; 3],
            first_mark_surface: 0,
            mark_surface_count: 0,
            ambient: [0; 4],
        }
    }

    fn model(head: i32, half: f32) -> Model {
        Model {
            mins: [-half; 3],
            maxs: [half; 3],
            origin: [0.0; 3],
            heads: [head, 0, 0, 0],
            vis_leaves: 0,
            first_face: 0,
            face_count: 0,
        }
    }

    /// The world (model 0): hull 0 split at z = 0, empty above, water
    /// below, in bounds of ±100 (cut at ±164); model 1: the same plane,
    /// solid below, in bounds of ±10 (cut at ±74).
    fn pool() -> Bsp {
        let node = |back: u32| Node {
            plane: 0,
            children: [Child::Leaf(1), Child::Leaf(back)],
            mins: [0; 3],
            maxs: [0; 3],
            first_face: 0,
            face_count: 0,
        };
        Bsp {
            planes: vec![Plane {
                normal: [0.0, 0.0, 1.0],
                dist: 0.0,
                kind: 2,
            }],
            nodes: vec![node(2), node(3)],
            leaves: vec![
                leaf(CONTENTS_SOLID),
                leaf(-1),
                leaf(CONTENTS_WATER),
                leaf(CONTENTS_SOLID),
            ],
            models: vec![model(0, 100.0), model(1, 10.0)],
            ..Bsp::default()
        }
    }

    fn contains_units(v: &MapWaterVolume, p: [f32; 3]) -> bool {
        v.brush.contains(super::super::to_engine(p))
    }

    #[test]
    fn the_worlds_water_leaf_is_a_volume() {
        let mut data = MapData::default();
        assert_eq!(add_world(&pool(), &mut data), 1);
        let v = &data.water[0];
        assert!(!v.slime);
        assert!(contains_units(v, [0.0, 0.0, -1.0]) && contains_units(v, [150.0, -150.0, -150.0]));
        assert!(!contains_units(v, [0.0, 0.0, 1.0]));
    }

    #[test]
    fn a_func_water_is_its_brush_at_its_origin_and_a_named_one_moves() {
        let b = pool();
        let e = Entity {
            pairs: vec![
                ("classname".into(), "func_water".into()),
                ("skin".into(), "-3".into()),
                ("model".into(), "*1".into()),
            ],
        };
        let mut data = MapData::default();
        assert_eq!(add_entity(&b, &e, 1, [0.0, 0.0, 50.0], 7, false, &mut data), 1);
        assert!(data.water_movers.is_empty());
        let v = &data.water[0];
        assert!(contains_units(v, [0.0, 0.0, 49.0]) && !contains_units(v, [0.0, 0.0, 51.0]));
        assert!(!contains_units(v, [100.0, 0.0, 49.0]), "only the model's own bounds");
        assert_eq!(add_entity(&b, &e, 1, [0.0; 3], 7, true, &mut data), 1);
        assert_eq!(data.water_movers, vec![(1, 7)]);
        // Not a liquid by its skin: nothing.
        let dry = Entity {
            pairs: vec![
                ("classname".into(), "func_illusionary".into()),
                ("skin".into(), "-1".into()),
            ],
        };
        assert_eq!(add_entity(&b, &dry, 1, [0.0; 3], 8, false, &mut data), 0);
    }

    #[test]
    fn liquid_skins_and_kinds() {
        let e = |class: &str, skin: &str| Entity {
            pairs: vec![("classname".into(), class.into()), ("skin".into(), skin.into())],
        };
        assert_eq!(entity_liquid(&e("func_water", "-3")), Some(-3));
        assert_eq!(entity_liquid(&e("func_illusionary", "-3")), Some(-3));
        assert_eq!(entity_liquid(&e("func_illusionary", "-1")), None);
        assert_eq!(entity_liquid(&e("func_wall", "-3")), None);
        assert_eq!(entity_liquid(&e("func_water", "0")), None);
        assert_eq!(slime_of(-3), Some(false));
        assert_eq!(slime_of(-4), Some(true));
        assert_eq!(slime_of(-5), Some(true));
        assert_eq!(slime_of(-2), None);
    }
}
