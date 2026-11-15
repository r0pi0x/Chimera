//! The level's own sounds (specs/halo_ce/sounds.md, "Sound environment,
//! background sound, sound scenery"; structure_bsp.md's background sound
//! and sound environment palettes; scenario.md's sound scenery): the data
//! a Halo map carries (`LevelAudio`, made by `maps::audio` and carried in
//! the `library`'s map entity) and the systems that play it.
//!
//! - **Where the listener is** (`LevelAudio::zone`): the collision BSP's
//!   point test (collision.md) gives the leaf, the structure BSP's leaf its
//!   cluster, the cluster its background sound and sound environment
//!   palette entries. The map carries the test as a pruned copy of the
//!   collision BSP's 3D tree (`prune`): each cluster's pair of entries is a
//!   *zone*, a leaf is its cluster's zone, a node whose two sides are one
//!   zone becomes that zone, and a solid side takes its sibling's (so a
//!   level whose clusters all name the same sounds carries no tree at all,
//!   and the listener in solid space hears the nearest open side's). A
//!   leaf without a cluster keeps the listener's last zone.
//! - **Background sound** (`background`): the zone's background sound as
//!   a looping sound, unspatialized at scale 1, one at a time: a change of
//!   zone stops the old (its tracks' fade out) and starts the new (its
//!   fade in): the cross-fade.
//! - **Sound environment** (`environment`): the zone's environment as data
//!   (`ListenerEnvironment`), for the presentation's room (`ui`).
//! - **Sound scenery** (`scenery`): each placement's looping sound
//!   attachments at its position (positional, the loop's own distances).
//!
//! Known shortcuts: a sound scenery's loop sits at the object's origin
//! (its attachment marker's offset is not applied) at scale 1 (its
//! attachment function is not evaluated); only the first structure BSP's
//! clusters (every multiplayer map has one).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    library::ActiveAudio,
    looping::{Listener, LoopRequest, LoopSource, owner_key},
};

/// Owner kinds of the level's loops (`looping::owner_key`).
pub const BACKGROUND_KIND: u8 = 1;
pub const SCENERY_KIND: u8 = 2;

/// A sound environment's reverb values (sounds.md's table, its units).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Environment {
    /// The tag's path.
    pub name: String,
    pub room_intensity: f32,
    pub room_intensity_hf: f32,
    pub decay_time: f32,
    pub decay_hf_ratio: f32,
    pub reflections_intensity: f32,
    pub reflections_delay: f32,
    pub reverb_intensity: f32,
    pub reverb_delay: f32,
    pub diffusion: f32,
    pub density: f32,
    pub hf_reference: f32,
}

/// A node of the pruned point test: a plane `n . p - d` in world units
/// (the flip applied), and its two sides (`Side` encoded: `>= 0` a node,
/// `-1` keep the last zone, `-(z + 2)` zone `z`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ZoneNode {
    pub plane: [f32; 4],
    pub front: i64,
    pub back: i64,
}

/// A side's encoding (module doc of `ZoneNode`).
pub fn zone_side(z: usize) -> i64 {
    -(z as i64) - 2
}

/// A sound scenery placement's loop: where (engine space) and the looping
/// sound.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneryLoop {
    pub at: [f32; 3],
    pub sound: String,
}

/// The level's sound data (module doc).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LevelAudio {
    /// The background sound palette: looping sound paths ("" none).
    pub backgrounds: Vec<String>,
    /// The sound environment palette.
    pub environments: Vec<Environment>,
    /// The zones: (background, environment) palette indices, -1 none.
    pub zones: Vec<[i32; 2]>,
    /// The pruned point test; `root` encoded as a side.
    pub tree: Vec<ZoneNode>,
    pub root: i64,
    /// How many clusters the level has.
    pub clusters: usize,
    pub scenery: Vec<SceneryLoop>,
}

impl LevelAudio {
    /// The zone of a point (world units), None for "keep the last".
    pub fn zone(&self, p: [f32; 3]) -> Option<usize> {
        let mut side = self.root;
        for _ in 0..=self.tree.len() {
            if side >= 0 {
                let n = self.tree.get(side as usize)?;
                let d = n.plane[0] * p[0] + n.plane[1] * p[1] + n.plane[2] * p[2] - n.plane[3];
                side = if d >= 0.0 { n.front } else { n.back };
            } else if side == -1 {
                return None;
            } else {
                return usize::try_from(-side - 2).ok().filter(|z| *z < self.zones.len());
            }
        }
        None
    }

    /// A zone's background sound path, if it has one.
    pub fn background(&self, zone: usize) -> Option<&str> {
        let b = usize::try_from(self.zones.get(zone)?[0]).ok()?;
        self.backgrounds.get(b).map(String::as_str).filter(|s| !s.is_empty())
    }

    /// A zone's environment, if it has one.
    pub fn environment(&self, zone: usize) -> Option<&Environment> {
        let e = usize::try_from(self.zones.get(zone)?[1]).ok()?;
        self.environments.get(e)
    }
}

/// A 3D BSP to prune: its nodes (plane, back raw child, front raw child),
/// and what a leaf is (encoded side).
pub struct Bsp<'a> {
    /// Node `n`: its plane (flip applied) and back and front children
    /// (raw: `>= 0` node, `-1` solid, else leaf `raw & 0x7FFFFFFF`).
    pub node: &'a dyn Fn(u32) -> Option<([f32; 4], i32, i32)>,
    pub leaf: &'a dyn Fn(u32) -> i64,
    pub nodes: usize,
}

/// The pruned tree (module doc) from the root of `bsp`: the nodes and the
/// root's side.
pub fn prune(bsp: &Bsp) -> (Vec<ZoneNode>, i64) {
    let mut out = Vec::new();
    if bsp.nodes == 0 {
        return (out, -1);
    }
    let root = walk(bsp, 0, &mut out, 0);
    (out, root)
}

fn walk(bsp: &Bsp, raw: i32, out: &mut Vec<ZoneNode>, depth: usize) -> i64 {
    if depth > 1024 {
        return -1;
    }
    if raw == -1 {
        return -1;
    }
    if raw < 0 {
        return (bsp.leaf)(raw as u32 & 0x7FFF_FFFF);
    }
    let Some((plane, back, front)) = (bsp.node)(raw as u32) else { return -1 };
    let f = walk(bsp, front, out, depth + 1);
    let b = walk(bsp, back, out, depth + 1);
    if f == b || b == -1 {
        return f;
    }
    if f == -1 {
        return b;
    }
    out.push(ZoneNode { plane, front: f, back: b });
    (out.len() - 1) as i64
}

/// The level's part of the library now (None: none).
fn level(active: &ActiveAudio) -> Option<&LevelAudio> {
    active.0.as_deref()?.level.as_ref()
}

/// The listener's zone now (kept while the point test says "keep").
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ListenerZone(pub Option<usize>);

/// The listener's sound environment now (for the presentation's room).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ListenerEnvironment(pub Option<Environment>);

/// The listener's zone and environment.
pub fn environment(active: Res<ActiveAudio>, listener: Res<Listener>, mut zone: ResMut<ListenerZone>, mut env: ResMut<ListenerEnvironment>) {
    let Some(level) = level(&active) else {
        if zone.0.is_some() {
            zone.0 = None;
        }
        if env.0.is_some() {
            env.0 = None;
        }
        return;
    };
    let Some(p) = listener.0 else { return };
    if let Some(z) = level.zone(crate::game::maps::space::to_game(p))
        && zone.0 != Some(z)
    {
        zone.0 = Some(z);
    }
    let want = zone.0.and_then(|z| level.environment(z)).cloned();
    if env.0 != want {
        env.0 = want;
    }
}

/// The zone's background sound (module doc).
pub fn background(active: Res<ActiveAudio>, zone: Res<ListenerZone>, mut requests: MessageWriter<LoopRequest>) {
    let Some(level) = level(&active) else { return };
    let Some(z) = zone.0 else { return };
    let Some(b) = level.zones.get(z).map(|z| z[0]) else { return };
    let Some(sound) = level.background(z) else { return };
    requests.write(LoopRequest {
        key: owner_key(BACKGROUND_KIND, b as u64),
        sound: sound.into(),
        source: LoopSource::Unspatialized,
        scale: 1.0,
        alternate: false,
    });
}

/// The sound scenery's loops (module doc).
pub fn scenery(active: Res<ActiveAudio>, mut requests: MessageWriter<LoopRequest>) {
    let Some(level) = level(&active) else { return };
    for (i, s) in level.scenery.iter().enumerate() {
        requests.write(LoopRequest {
            key: owner_key(SCENERY_KIND, i as u64),
            sound: s.sound.clone(),
            source: LoopSource::At(Vec3::from(s.at)),
            scale: 1.0,
            alternate: false,
        });
    }
}
