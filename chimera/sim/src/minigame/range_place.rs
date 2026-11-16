//! Where the sandbox's practice targets and props go on the range
//! (`chimera_world::range`, the sandbox's default map): while its
//! `RangeLayout` is in, a thing asked for at `d` m stands on the range's
//! `d` m line (`RangeLayout::point(d, side)`), not `d` m along the
//! player's view, so the painted distance lines mean what they say.
//!
//! - **The side**: the spawner's sideways offset from the range's axis
//!   (`RangeLayout::right`), clamped inside the range's width less the
//!   thing's own reach (a strafer's `targets::STRAFE_HALF`, a walker's
//!   `targets::WALK_RADIUS`), then nudged sideways in 0.5 m steps to the
//!   nearest spot that is free: no other target's or prop's footprint
//!   there (`Footprint`, its reach included), a floor at the range's floor
//!   height (not a cover crate's top), no wall or crate in a standing box
//!   (`targets::find_spot` from the firing line), and for a strafer both
//!   ends of its strafe, for a walker the four ends of its walk, standable
//!   too. Refused when no side of that line has room.
//! - **What it is placed from**: the firing line's point at that side, a
//!   standing player's centre high (1 m up; `RangeSpot::origin`), along
//!   the range's down-range direction: the thing faces straight back up
//!   range, a strafer strafes across the lane along its line.
//! - `RangeLine` (on a target placed so) keeps its line and side, for the
//!   measurements (`targets::HitMeasured::line_m`) and the layout's repeat
//!   (`layout`).
//!
//! Off the range (no `RangeLayout`) none of this applies: `targets` and
//! `props` place along the spawner's view as before.
//!
//! Known shortcuts: the range's cover crates aren't in `RangeLayout`, so
//! they are found by the floor and standing-box checks, not avoided by
//! plan; the side is clamped by the floor's half width alone.

use bevy::prelude::*;
use chimera_world::range::RangeLayout;

use super::{
    props::{OnRangeFurniture, Prop},
    targets::{STRAFE_HALF, Target, TargetMotion, WALK_RADIUS, find_spot},
};

/// How far a floor found may be from the range's floor height (m): more is
/// a crate's top, not the floor.
const FLOOR_TOLERANCE: f32 = 0.25;
/// The nudge's step sideways (m).
const STEP: f32 = 0.5;
/// Kept off a side wall past the thing's own reach (m).
const WALL_MARGIN: f32 = 0.6;
/// A standing body's half width and depth (m).
const BODY_HALF: f32 = 0.6;

/// A target's line on the range: its distance line (m down range from the
/// firing line) and its side (m right of the axis, as placed; a strafer
/// and a walker move about it).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct RangeLine {
    pub distance_m: f32,
    pub side_m: f32,
}

/// What a thing covers on the range's floor: half its reach across the
/// lane and along it (m), its motion's reach included.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    pub across: f32,
    pub along: f32,
}

impl Footprint {
    pub fn of_target(motion: TargetMotion) -> Self {
        match motion {
            TargetMotion::Still => Self { across: BODY_HALF, along: BODY_HALF },
            TargetMotion::Strafes => Self { across: STRAFE_HALF + BODY_HALF, along: BODY_HALF },
            TargetMotion::Walks => Self { across: WALK_RADIUS + BODY_HALF, along: WALK_RADIUS + BODY_HALF },
        }
    }

    /// A prop standing on the floor on its own (a shelf, a board).
    pub fn of_prop(kind: &str) -> Self {
        if kind == "target_board" { Self { across: 0.6, along: 0.3 } } else { Self { across: 0.9, along: 0.4 } }
    }
}

/// A spot on the range: its feet (or foot), the firing line's point it is
/// placed from, and its side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangeSpot {
    pub home: Vec3,
    pub origin: Vec3,
    pub side: f32,
}

/// The range's layout, while the range is the map in play.
pub fn layout(world: &World) -> Option<RangeLayout> {
    world.get_resource::<RangeLayout>().cloned()
}

/// A point in the range's frame: (m down range from the firing line, m
/// right of the axis).
pub fn along_side(l: &RangeLayout, p: Vec3) -> Vec2 {
    let d = p - l.firing_line;
    Vec2::new(d.dot(l.dir), d.dot(l.right))
}

/// The firing line's point at `side`, a standing player's centre high.
pub fn origin_at(l: &RangeLayout, side: f32) -> Vec3 {
    l.point(0.0, side) + Vec3::Y
}

/// What stands on the range's floor now: targets' and free-standing props'
/// footprints (their centre in the range's frame), but `except`'s. Props
/// on the range's own furniture stand off the lane and are left out.
fn taken(world: &mut World, l: &RangeLayout, except: Option<Entity>) -> Vec<(Vec2, Footprint)> {
    let mut out = Vec::new();
    for (e, t) in world.query::<(Entity, &Target)>().iter(world) {
        if Some(e) != except {
            out.push((along_side(l, t.home), Footprint::of_target(t.motion)));
        }
    }
    for (e, p, on) in world.query::<(Entity, &Prop, Option<&OnRangeFurniture>)>().iter(world) {
        if Some(e) != except && on.is_none() {
            out.push((along_side(l, p.home), Footprint::of_prop(&p.spec.kind)));
        }
    }
    out
}

/// Feet on the range's floor at (`along`, `side`), or why not.
fn floor_at(world: &mut World, l: &RangeLayout, along: f32, side: f32) -> Result<Vec3, String> {
    let feet = find_spot(world, origin_at(l, side), l.dir, along)?;
    if (feet.y - l.firing_line.y).abs() > FLOOR_TOLERANCE {
        return Err(format!("something stands on the {along} m line there"));
    }
    Ok(feet)
}

/// The spot on the `distance` m line nearest `side` with room for
/// `reach` (module doc), `except` left out of what is taken (a target
/// moved along its own line).
pub fn line_spot(
    world: &mut World,
    l: &RangeLayout,
    distance: f32,
    side: f32,
    reach: Footprint,
    except: Option<Entity>,
) -> Result<RangeSpot, String> {
    if !(distance.is_finite() && distance > 0.5) {
        return Err(format!("a distance of {distance} m is too short"));
    }
    let limit = (l.half_width - reach.across - WALL_MARGIN).max(0.0);
    let start = if side.is_finite() { side.clamp(-limit, limit) } else { 0.0 };
    let taken = taken(world, l, except);
    // The ends of its motion that must be standable too.
    let ends: Vec<Vec2> = if reach.along > BODY_HALF {
        let r = reach.along - BODY_HALF;
        vec![Vec2::new(r, 0.0), Vec2::new(-r, 0.0), Vec2::new(0.0, r), Vec2::new(0.0, -r)]
    } else if reach.across > BODY_HALF {
        let r = reach.across - BODY_HALF;
        vec![Vec2::new(0.0, r), Vec2::new(0.0, -r)]
    } else {
        Vec::new()
    };
    let mut why = None;
    let steps = (2.0 * limit / STEP).ceil() as i32 + 1;
    for k in 0..=2 * steps {
        // 0, +1, -1, +2, -2, ... steps from the side asked.
        let n: i32 = if k % 2 == 1 { (k + 1) / 2 } else { -(k / 2) };
        let off = n as f32 * STEP;
        let s = start + off;
        if s.abs() > limit + 1e-4 {
            continue;
        }
        let here = Vec2::new(distance, s);
        if taken
            .iter()
            .any(|(c, f)| (c.x - here.x).abs() < f.along + reach.along && (c.y - here.y).abs() < f.across + reach.across)
        {
            why.get_or_insert_with(|| format!("the {distance} m line is taken there"));
            continue;
        }
        let feet = match floor_at(world, l, distance, s) {
            Ok(f) => f,
            Err(e) => {
                why.get_or_insert(e);
                continue;
            }
        };
        let mut clear = true;
        for end in &ends {
            let (a, b) = (distance + end.x, s + end.y);
            // An end at or behind the firing line isn't checked (a walker
            // close to it).
            if a > 0.5 && floor_at(world, l, a, b).is_err() {
                clear = false;
                break;
            }
        }
        if !clear {
            why.get_or_insert_with(|| format!("no room to move on the {distance} m line there"));
            continue;
        }
        return Ok(RangeSpot { home: feet, origin: origin_at(l, s), side: s });
    }
    Err(format!("no free spot on the range's {distance} m line ({})", why.unwrap_or_else(|| "no room".into())))
}
