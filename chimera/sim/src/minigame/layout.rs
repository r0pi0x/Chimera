//! The sandbox's remembered set-up (Ultimate Gun Game's `sandbox`): the
//! last layout of practice targets and props the player made, to put back
//! with one request, here or on another spot or map.
//!
//! - **What is remembered** (`SandboxLayout`): every target and prop that
//!   existed when the player last changed the layout: a spawn, a set or a
//!   single removal, by any way in (`targets::spawn`, `set`, `remove`;
//!   `props::spawn`, `remove`: the console, the requests, the screens'
//!   bridge). Clearing (`ClearTargets`, `ClearProps`, `target remove all`,
//!   the bench's Clear), deaths, knocked pieces, a map change and the
//!   sandbox stopping change nothing: the layout is still there to repeat.
//!   Each item keeps its settings (a target's body, gun, rung, distance,
//!   motion, shoots back, protected, armoured, respawn, place; a prop's kind and
//!   distance) and its line relative to an anchor: the first target's
//!   placing (the first prop's when there is no target), its spawner's
//!   position and flat view. So a layout is relative to the player.
//! - **Repeating it** (`RepeatLayout`, `repeat`; console `sandbox repeat`,
//!   also `target repeat`): every target and prop goes first, then each
//!   remembered item is placed again from where the player stands and
//!   looks now: the anchor becomes the player's position and flat view,
//!   every item's spawner spot and line turned and moved with it, each at
//!   its own distance on the floor there (`targets::find_spot`). An item
//!   with no floor (or a wall) there is refused (`targets::TargetRefused`,
//!   one each) and the rest are placed. Repeating doesn't change what is
//!   remembered.
//! - **On the range** (`chimera_world::range`, `range_place`): an item
//!   made on the range also keeps its side (`range_side`: a target's, or a
//!   prop's on a distance line; a prop on the range's own shelf or board
//!   goes back on that furniture). Repeated on the range it comes back on
//!   its distance line at that side, wherever the player stands; an item
//!   made elsewhere and repeated on the range goes to its distance line at
//!   the side its spawner's spot (moved and turned with the player, as
//!   above) gives. A layout made on the range and repeated on another map
//!   comes back relative to the player as any other: the first item's
//!   firing-line point at the player, each line's distance along the
//!   player's view, the items side by side as their lanes were. A target
//!   on the range's platform (`LayoutTarget::place`) comes back on the
//!   platform; off the range it is refused.
//! - **Lifetime**: a resource of the app (never removed by a match, a map
//!   change or a sandbox restart in the same run of the game). Not on disk.
//!
//! What the screens read: `SandboxLayout` (`targets`, `props`, the counts
//! and `summary`: one short line, empty when there is nothing to repeat).
//!
//! Known shortcuts: saving it to the user's settings would take a serde
//! derive on these plain structs, a key in the player's settings file,
//! written at each record and read into the resource at start; not done.

use bevy::prelude::*;

use super::{
    props::{self, PropSpec},
    targets::{self, SpawnTarget, Target, TargetMotion, TargetPlace, TargetRefused},
};
use crate::{console::command_player, core::Intent};

/// One remembered target: its settings and its line relative to the anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTarget {
    pub character: String,
    pub character_name: String,
    pub gun: String,
    pub rung: usize,
    pub distance_m: f32,
    pub motion: TargetMotion,
    pub shoots_back: bool,
    pub protected: bool,
    pub armored: bool,
    pub respawn_secs: f32,
    /// Its spawner's spot in the anchor's frame (m: +x right, -z ahead).
    pub offset: Vec3,
    /// Its line's yaw from the anchor's view (radians).
    pub yaw: f32,
    /// Made on the range: its side (m right of the range's axis; its
    /// distance line is `distance_m`). None: made elsewhere.
    pub range_side: Option<f32>,
    /// On a line or on the range's platform (repeated there; refused off
    /// the range).
    pub place: TargetPlace,
}

/// One remembered prop.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutProp {
    pub kind: String,
    pub distance_m: f32,
    pub offset: Vec3,
    pub yaw: f32,
    /// Made on the range standing on a distance line (not on the range's
    /// own shelf or board): its side, as `LayoutTarget::range_side`.
    pub range_side: Option<f32>,
}

/// The last layout the player made (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SandboxLayout {
    pub targets: Vec<LayoutTarget>,
    pub props: Vec<LayoutProp>,
    /// "2 targets, 1 prop: Spartan (sniper_rifle) 10 m, ...; target_board 12 m"
    /// (empty: nothing to repeat).
    pub summary: String,
}

impl SandboxLayout {
    /// Nothing to repeat.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty() && self.props.is_empty()
    }
}

/// Put the remembered layout back in front of the player (module doc).
#[derive(Message, Clone, Copy, Debug, Default, PartialEq)]
pub struct RepeatLayout {
    /// Whose position and view it is placed from (None: the local player).
    pub by: Option<Entity>,
}

/// The flat view's yaw of a flat direction (`Intent::yaw`'s convention:
/// yaw 0 looks along -z).
fn yaw_of(dir: Vec3) -> f32 {
    (-dir.x).atan2(-dir.z)
}

fn dir_of(yaw: f32) -> Vec3 {
    Vec3::new(-yaw.sin(), 0.0, -yaw.cos())
}

/// Remember what stands now as the layout (a player's change).
pub(super) fn record(world: &mut World) {
    let all_t = targets::all_targets(world);
    let all_p = props::all_props(world);
    let anchor = all_t.first().map(|(_, t)| (t.origin, t.dir)).or_else(|| all_p.first().map(|(_, p)| (p.origin, p.dir)));
    let mut layout = SandboxLayout::default();
    if let Some((o, d)) = anchor {
        let ay = yaw_of(d);
        let to_local = Quat::from_rotation_y(-ay);
        let rel = |origin: Vec3, dir: Vec3| (to_local * (origin - o), yaw_of(dir) - ay);
        let range = super::range_place::layout(world);
        for (e, t) in &all_t {
            let (offset, yaw) = rel(t.origin, t.dir);
            let side = world.get::<targets::RangeLine>(*e).map(|l| l.side_m);
            layout.targets.push(target_of(t, offset, yaw, side));
        }
        for (e, p) in &all_p {
            let (offset, yaw) = rel(p.origin, p.dir);
            let on_line = range.is_some() && world.get::<props::OnRangeFurniture>(*e).is_none();
            let range_side = range.as_ref().filter(|_| on_line).map(|l| super::range_place::along_side(l, p.home).y);
            layout.props.push(LayoutProp { kind: p.spec.kind.clone(), distance_m: p.spec.distance_m, offset, yaw, range_side });
        }
    }
    layout.summary = summary(&layout);
    if world.get_resource::<SandboxLayout>() != Some(&layout) {
        info!("SandboxLayout {}", if layout.summary.is_empty() { "empty" } else { &layout.summary });
        world.insert_resource(layout);
    }
}

fn target_of(t: &Target, offset: Vec3, yaw: f32, range_side: Option<f32>) -> LayoutTarget {
    LayoutTarget {
        character: t.character.clone(),
        character_name: t.character_name.clone(),
        gun: t.gun.clone(),
        rung: t.rung,
        distance_m: t.distance_m,
        motion: t.motion,
        shoots_back: t.shoots_back,
        protected: t.protected,
        armored: t.armored,
        respawn_secs: t.respawn_secs,
        offset,
        yaw,
        range_side,
        place: t.place,
    }
}

fn summary(l: &SandboxLayout) -> String {
    if l.is_empty() {
        return String::new();
    }
    let plural = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
    let items: Vec<String> = l
        .targets
        .iter()
        .map(|t| {
            let gun = t.gun.split_once(':').map_or(t.gun.as_str(), |(_, g)| g);
            match t.place {
                TargetPlace::Platform => format!("{} ({gun}) on the platform", t.character_name),
                TargetPlace::Line => format!("{} ({gun}) {:.0} m {}", t.character_name, t.distance_m, t.motion.word()),
            }
        })
        .chain(l.props.iter().map(|p| format!("{} {:.0} m", p.kind, p.distance_m)))
        .collect();
    format!("{}, {}: {}", plural(l.targets.len(), "target"), plural(l.props.len(), "prop"), items.join("; "))
}

/// Put the remembered layout back from `by`'s (None: the local player's)
/// position and view: what was placed, and the refusals (each also a
/// `TargetRefused`).
pub fn repeat(world: &mut World, by: Option<Entity>) -> Result<(usize, Vec<String>), String> {
    if !super::rung_command::holds(world) {
        return Err("the layout is the sandbox's: start it with `sandbox`".into());
    }
    let layout = world.get_resource::<SandboxLayout>().cloned().unwrap_or_default();
    if layout.is_empty() {
        return Err("no layout to repeat yet (place a target or a prop first)".into());
    }
    let by = match by {
        Some(e) => e,
        None => command_player(world).map_err(|_| "no player to place it in front of".to_string())?,
    };
    let at = world.get::<Transform>(by).ok_or("the player has no place")?.translation;
    let yaw = world.get::<Intent>(by).map_or(0.0, |i| i.yaw);
    let turn = Quat::from_rotation_y(yaw);
    targets::clear(world);
    props::clear(world);
    // On the range an item made there comes back on its line at its side;
    // any other goes to its distance line at the side its spot gives
    // (`targets::spawn_from` on the range).
    let range = super::range_place::layout(world);
    let from = |side: Option<f32>, offset: Vec3, y: f32| match (&range, side) {
        (Some(l), Some(s)) => (super::range_place::origin_at(l, s), l.dir),
        _ => (at + turn * offset, dir_of(yaw + y)),
    };
    let mut placed = 0;
    let mut refused = Vec::new();
    for t in &layout.targets {
        let req = SpawnTarget {
            character: Some(t.character.clone()),
            gun: Some(t.gun.clone()),
            rung: None,
            distance_m: t.distance_m,
            motion: t.motion,
            shoots_back: t.shoots_back,
            protected: t.protected,
            armored: t.armored,
            respawn_secs: t.respawn_secs,
            by: Some(by),
        };
        let (origin, dir) = from(t.range_side, t.offset, t.yaw);
        match targets::spawn_placed_from(world, &req, t.place, origin, dir) {
            Ok(_) => placed += 1,
            Err(e) => refused.push(format!("{}: {e}", t.character_name)),
        }
    }
    for p in &layout.props {
        let spec = PropSpec { kind: p.kind.clone(), distance_m: p.distance_m };
        let (origin, dir) = from(p.range_side, p.offset, p.yaw);
        match props::spawn_from(world, &spec, origin, dir) {
            Ok(_) => placed += 1,
            Err(e) => refused.push(format!("{}: {e}", p.kind)),
        }
    }
    for reason in &refused {
        warn!("layout: {reason}");
        world.write_message(TargetRefused { reason: reason.clone() });
    }
    info!("SandboxLayout repeated: {placed} placed, {} refused", refused.len());
    Ok((placed, refused))
}

/// The console's words for a repeat.
pub fn repeat_command(world: &mut World) -> Result<Option<String>, String> {
    let (placed, refused) = repeat(world, None)?;
    let mut said = format!("repeated the layout: {placed} placed");
    for r in refused {
        said.push_str(&format!("\nrefused {r}"));
    }
    Ok(Some(said))
}

/// The requests, applied as world work.
fn apply_requests(mut asks: MessageReader<RepeatLayout>, mut commands: Commands) {
    for r in asks.read().copied() {
        commands.queue(move |world: &mut World| {
            if let Err(reason) = repeat(world, r.by) {
                warn!("layout: {reason}");
                world.write_message(TargetRefused { reason });
            }
        });
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SandboxLayout>()
        .add_message::<RepeatLayout>()
        .add_message::<TargetRefused>()
        .add_systems(Update, apply_requests);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_and_direction_agree_with_the_view() {
        for yaw in [0.0f32, 0.7, -2.0, 3.0] {
            let d = dir_of(yaw);
            assert!((yaw_of(d) - yaw).abs() < 1e-5, "{yaw}");
        }
        assert!((dir_of(0.0) - Vec3::NEG_Z).length() < 1e-6, "yaw 0 looks along -z");
    }
}
