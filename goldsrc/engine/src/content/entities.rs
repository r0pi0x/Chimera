//! A map's entities (specs/goldsrc/bsp.md, "Entities", "Brush entities
//! and render modes"): the list handed to Chimera as `MapEntity`s (keys
//! as the map has them, in entity space: GoldSrc's units and axes, as
//! `map::entities` expects), the spawn points, and what each brush entity
//! (`model` `*n`) is:
//!
//! - movers (`func_door`, `func_door_rotating`, `func_rotating`,
//!   `func_train`, `func_tracktrain`, `func_plat`, `func_button`,
//!   `func_rot_button`, `func_pendulum`, `func_breakable`,
//!   `func_pushable`): drawn and collided with through their own node
//!   (`MapEntity::mover`, their hull 0 regions as its volumes), where they
//!   rest at spawn until a logic layer moves them. GoldSrc's `angles` on
//!   them is the move direction, so it is handed over as `movedir`
//!   (Chimera turns a mover's node by `angles`);
//! - static solids (`func_wall`, `func_wall_toggle`, `func_conveyor`,
//!   `func_guntarget`): drawn and baked into the world's solids;
//! - drawn, not solid: `func_illusionary`, `func_water`;
//! - ladders (`func_ladder`): climbable volumes, not drawn;
//! - everything else with a model (`trigger_*`, `func_buyzone`,
//!   `func_bomb_target`, ...): neither drawn nor solid; its volumes kept
//!   on its entity for logic.
//!
//! A brush entity draws by its render keys: mode 1 flat colour, 2 blended
//! and 5 added at `renderamt`/255, 4 cut out; 0 opaque.
//!
//! Spawns: the classes the game lists (`GoldSrcGame::spawns`), each put
//! on the floor under it: a spawn's origin is the standing box's centre
//! (bsp.md, "Hull sizes"), so hull 1 is traced down from it 64 units and
//! the feet are the resting centre less 36 (`STAND_HALF_HEIGHT`). The
//! yaw comes from `angles` (or `angle`) as Source's maps': engine yaw =
//! GoldSrc yaw − 90°.
//!
//! The first entity (`worldspawn`) carries `map::entities::
//! LOGIC_FAMILY_KEY` = `goldsrc`: the map's logic is `crate::logic`'s,
//! never Source's. A named `func_wall` or `func_water` and every
//! `func_wall_toggle` get a node too (`brush_kind_of`: their logic flips
//! their frame, moves them or turns them off).
//!
//! Known shortcuts:
//! - Water volumes (`func_water`, `!` leaves) are drawn but not handed
//!   over as water.

use bevy::math::Vec3;
use goldsrc_formats::bsp::{Bsp, Entity, solids::convex_regions};

use super::{METERS_PER_UNIT, brushes::Drawn, collision, parse_vector, to_engine};
use crate::{
    core::Team,
    map::{
        MapAlpha, MapData, MapEntity,
        entities::{LOGIC_FAMILY_KEY, MapHull},
    },
};

/// Half the standing box's height: a spawn's origin above its feet
/// (bsp.md, hull 1 ±36).
pub const STAND_HALF_HEIGHT: f32 = 36.0;

/// What a brush entity is (module doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brush {
    Mover,
    StaticSolid,
    DrawnOnly,
    Ladder,
    Hidden,
}

/// The brush entity class `classname` is.
pub fn brush_kind(classname: &str) -> Brush {
    match classname.to_ascii_lowercase().as_str() {
        "func_door" | "func_door_rotating" | "func_rotating" | "func_train" | "func_tracktrain" | "func_plat"
        | "func_button" | "func_rot_button" | "func_pendulum" | "func_breakable" | "func_pushable" => Brush::Mover,
        "func_wall" | "func_wall_toggle" | "func_conveyor" | "func_guntarget" => Brush::StaticSolid,
        "func_illusionary" | "func_water" => Brush::DrawnOnly,
        "func_ladder" => Brush::Ladder,
        _ => Brush::Hidden,
    }
}

/// What the brush entity `e` is: as its class (`brush_kind`), except that
/// a named `func_wall` (its frame flips when fired: the `+a` set) and a
/// named `func_water` (it moves like a door when fired) get a node of
/// their own, and so does every `func_wall_toggle` (it turns off and on).
pub fn brush_kind_of(e: &Entity) -> Brush {
    let class = e.classname().to_ascii_lowercase();
    let named = e.get("targetname").is_some_and(|n| !n.is_empty());
    match class.as_str() {
        "func_wall" | "func_water" if named => Brush::Mover,
        "func_wall_toggle" => Brush::Mover,
        _ => brush_kind(&class),
    }
}

/// A brush entity's model index (`*n`), if it has one.
pub fn model_of(e: &Entity) -> Option<usize> {
    e.get("model")?.strip_prefix('*')?.parse().ok().filter(|&m| m > 0)
}

fn int(e: &Entity, key: &str) -> i32 {
    e.get(key).and_then(|v| v.trim().parse::<f32>().ok()).map_or(0, |v| v as i32)
}

/// How a brush entity's render keys draw it (render, colour, flat).
fn render_look(e: &Entity) -> (Option<(MapAlpha, f32)>, Option<[u8; 3]>, bool) {
    let mode = int(e, "rendermode");
    let amount = e.get("renderamt").and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(255.0) / 255.0;
    let color = e.get("rendercolor").map(parse_vector).map(|c| c.map(|v| v.clamp(0.0, 255.0) as u8));
    let color = color.filter(|c| *c != [255; 3] && *c != [0; 3]);
    match mode {
        1 => (Some((MapAlpha::Blend, amount)), Some(color.unwrap_or([255; 3])), true),
        2 => (Some((MapAlpha::Blend, amount)), None, false),
        5 => (Some((MapAlpha::Add, amount)), None, false),
        _ => (None, None, false),
    }
}

/// The yaw of an entity's `angles` (or `angle`) as an engine `Intent` yaw.
pub fn yaw(e: &Entity) -> f32 {
    let deg = e
        .get("angles")
        .map(|a| parse_vector(a)[1])
        .or_else(|| e.get("angle").and_then(|a| a.trim().parse::<f32>().ok()))
        .unwrap_or(0.0);
    deg.to_radians() - std::f32::consts::FRAC_PI_2
}

/// The feet under a spawn's origin (units): hull 1 traced 64 units down,
/// less the box's half height; the origin itself less it when the trace
/// starts in solid.
pub fn feet(bsp: &Bsp, origin: [f32; 3]) -> [f32; 3] {
    let start = origin.map(f64::from);
    let end = [start[0], start[1], start[2] - 64.0];
    let t = bsp.trace(0, 1, start, end);
    let z = if t.started_solid { start[2] } else { t.end[2] };
    [origin[0], origin[1], z as f32 - STAND_HALF_HEIGHT]
}

/// What the entity pass gives the rest of the conversion.
#[derive(Default)]
pub struct Parts {
    /// The brush models to draw (the world first).
    pub drawn: Vec<Drawn>,
}

/// The entities of `bsp` into `data` (entities, spawns, solids of static
/// brush entities, ladders) and what to draw. `spawns`: spawn classes and
/// their teams.
pub fn add(bsp: &Bsp, spawns: &[(&str, Option<Team>)], data: &mut MapData) -> Parts {
    let mut parts = Parts {
        drawn: vec![Drawn::default()],
    };
    data.entity_scale = METERS_PER_UNIT;
    for e in bsp.entities() {
        let class = e.classname().to_string();
        let origin = e.get("origin").map(parse_vector).unwrap_or([0.0; 3]);
        if let Some(&(_, team)) = spawns.iter().find(|(c, _)| c.eq_ignore_ascii_case(&class)) {
            data.spawns.push((to_engine(feet(bsp, origin)), team));
            data.spawn_yaws.push(yaw(&e));
        }
        let mut keyvalues = e.pairs.clone();
        // The map's logic is GoldSrc's (`logic`), not Source's.
        if data.entities.is_empty() {
            keyvalues.push((LOGIC_FAMILY_KEY.into(), crate::logic::FAMILY.into()));
        }
        let mut entity = MapEntity::default();
        if let Some(model) = model_of(&e) {
            let kind = brush_kind_of(&e);
            let index = data.entities.len();
            let (render, color, flat) = render_look(&e);
            if kind == Brush::Mover {
                for (k, _) in keyvalues.iter_mut().filter(|(k, _)| k.eq_ignore_ascii_case("angles")) {
                    *k = "movedir".into();
                }
                entity.mover = true;
            }
            // Volumes in entity space, local to the origin.
            entity.hulls = convex_regions(bsp, model, 0)
                .into_iter()
                .map(|(_, c)| MapHull {
                    planes: c
                        .planes
                        .iter()
                        .map(|h| (Vec3::new(h.normal[0] as f32, h.normal[1] as f32, h.normal[2] as f32), h.dist as f32))
                        .collect(),
                    points: c.vertices.iter().map(|v| Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32)).collect(),
                })
                .collect();
            match kind {
                Brush::Mover => parts.drawn.push(Drawn { model, offset: [0.0; 3], mover: Some(index), render, color, flat }),
                Brush::StaticSolid | Brush::DrawnOnly => {
                    if kind == Brush::StaticSolid {
                        collision::add_static(bsp, model, origin, data);
                        // Its clip hulls beside the world's (`crate::hulls`).
                        crate::hulls::place(bsp, model, origin, data.extras.entry());
                    }
                    parts.drawn.push(Drawn { model, offset: origin, mover: None, render, color, flat });
                }
                Brush::Ladder => collision::add_ladder(bsp, model, origin, data),
                Brush::Hidden => {}
            }
        }
        entity.keyvalues = keyvalues;
        data.entities.push(entity);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brush_classes_and_models() {
        assert_eq!(brush_kind("func_door"), Brush::Mover);
        assert_eq!(brush_kind("FUNC_WALL"), Brush::StaticSolid);
        assert_eq!(brush_kind("func_illusionary"), Brush::DrawnOnly);
        assert_eq!(brush_kind("func_buyzone"), Brush::Hidden);
        let e = Entity { pairs: vec![("model".into(), "*12".into()), ("angles".into(), "0 90 0".into())] };
        assert_eq!(model_of(&e), Some(12));
        assert!((yaw(&e) - 0.0).abs() < 1e-6);
    }
}
