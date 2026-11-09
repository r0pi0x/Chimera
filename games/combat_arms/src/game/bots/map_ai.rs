//! The maps' own AI data for the bots (specs/combat_arms/bots.md 3, 4):
//! the hand-placed AI volumes and patrol nodes, read from the map's
//! objects (`game::world` passes the AI classes through as map entities,
//! `MapEntities`), in place of the generated navigation areas where a map
//! has them (costarecon_cp: 86 `AIVolume`, 8 `AINodePatrol`;
//! infectedship_cp, noxiousdamnation, snowstorm and bossbattle_cp too).
//!
//! - Volumes (3): every volume class but ambient-life (`VOLUME_CLASSES`),
//!   each an axis-aligned box `Pos` ± `Dims`; a navigation area of its
//!   rectangle, its floor the first surface a ray down from the box's top
//!   meets at its centre, inside the box or a little under it (the maps
//!   place the boxes 0.2-0.8 m above their floors: `brain::FLOOR_BELOW`),
//!   else the box's bottom. Two
//!   volumes are linked when their boxes overlap or touch on all three
//!   axes (inclusive); the link is the shared rectangle of their top
//!   faces, crossed along the axis it is thinner in, and dropped when
//!   shorter than `nav::MIN_LINK` (48 u). More than `MAX_VOLUMES` (1024)
//!   disables them (the spec: no path finding at all; ours: the generated
//!   areas instead).
//! - Patrol nodes (4): `AINodePatrol` objects but those `StartDisabled`,
//!   linked by their `Next` and `Next1`..`Next4` names both ways (a bot
//!   turns back at a chain's end); each at its position dropped onto the
//!   area under it.
//!
//! - Jump-up volumes (`AIVolumeJumpUp`, Open question 16) link one way:
//!   left only for a higher box, or with `OnlyJumpDown` only for one not
//!   higher (`AiVolume::may_leave_to`; Death Room's four drop); a volume
//!   keeps at most 32 neighbours (`MAX_NEIGHBOURS`).
//!
//! Known shortcuts: the other volume types (ladder, stairs, ledge,
//! jump-over, junction, teleport) are plain volumes: no ladder climbs or
//! jumps along them (a jump-up is walked), no doors; path weights are 1
//! (every bot map's volumes have the default weight and `PreferredPath`
//! off, Open question 16); nothing keeps a bot
//! inside the volumes (it moves as a player, bots.md Open question 6);
//! the bot game-mode file isn't read: the patrol start nodes are the maps'
//! `AI_Node_A_01..08`/`AI_Node_B_01..08` (bots.md 4, `brain::start_node`);
//! the look-arounds while running and guard nodes aren't played; `AIRegion`,
//! `AINodeGoto` and `AINodeUseObject` are passed through but unused.

use bevy::prelude::*;

use super::nav::{MIN_LINK, PatrolGraph};
use crate::map::{
    MapEntities, MapEntity,
    entities::entity_to_engine,
    nav::{NavArea, NavMesh, Side, Via},
};

/// The AI volume classes bots walk (all but ambient-life ones, which are
/// for ambient characters).
pub const VOLUME_CLASSES: &[&str] = &[
    "AIVolume",
    "AIVolumeLadder",
    "AIVolumeStairs",
    "AIVolumeLedge",
    "AIVolumeJumpUp",
    "AIVolumeJumpOver",
    "AIVolumeJunction",
    "AIVolumeTeleport",
];

/// The patrol node class.
pub const PATROL_CLASS: &str = "AINodePatrol";

/// The most volumes path finding takes.
pub const MAX_VOLUMES: usize = 1024;

/// Whether `game::world` passes objects of `class` through (the AI
/// objects: volumes, nodes, regions).
pub fn is_ai_object(class: &str) -> bool {
    class.starts_with("AIVolume") || class.starts_with("AINode") || class == "AIRegion"
}

/// An AI volume: its box (engine metres) and, for a jump-up volume,
/// whether it only drops (`OnlyJumpDown`).
#[derive(Clone, Debug, PartialEq)]
pub struct AiVolume {
    pub name: String,
    pub class: String,
    pub min: Vec3,
    pub max: Vec3,
    pub only_jump_down: bool,
}

/// A volume keeps at most this many neighbour candidates (3: the link
/// builder's table).
pub const MAX_NEIGHBOURS: usize = 32;

impl AiVolume {
    /// Whether a path may enter this volume from `from` (a jump-up volume
    /// is entered from below its top, a drop's from above: the way that
    /// leaves it as `may_leave_to` allows).
    pub fn jump_or_any(&self, from: &AiVolume) -> bool {
        if !self.jump_up() {
            return true;
        }
        if self.only_jump_down { from.min.y >= self.min.y - 1e-4 } else { from.max.y <= self.max.y + 1e-4 }
    }

    /// A jump-up volume (3): one-way.
    pub fn jump_up(&self) -> bool {
        self.class == "AIVolumeJumpUp"
    }

    /// Whether a path may leave this volume for `next` (3, Open question
    /// 16): out of a jump-up volume only to a higher box (its top above
    /// this one's), or with `OnlyJumpDown` only to one whose bottom isn't
    /// higher than this one's; out of any other, always.
    pub fn may_leave_to(&self, next: &AiVolume) -> bool {
        if !self.jump_up() {
            return true;
        }
        if self.only_jump_down { next.min.y <= self.min.y + 1e-4 } else { next.max.y > self.max.y + 1e-4 }
    }
}

/// A patrol node (engine metres) and the names it links to.
#[derive(Clone, Debug, PartialEq)]
pub struct PatrolNode {
    pub name: String,
    pub pos: Vec3,
    pub next: Vec<String>,
}

/// The map's AI data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapAi {
    pub volumes: Vec<AiVolume>,
    pub patrol: Vec<PatrolNode>,
}

fn origin(e: &MapEntity, scale: f32) -> Vec3 {
    entity_to_engine(e.vector("origin"), scale)
}

impl MapAi {
    /// The AI objects among the map's entities (`scale` metres per entity
    /// unit).
    pub fn read(entities: &[MapEntity], scale: f32) -> Self {
        let mut out = Self::default();
        for e in entities {
            let class = e.classname();
            if VOLUME_CLASSES.contains(&class) {
                let c = origin(e, scale);
                let half = entity_to_engine(e.vector("Dims"), scale).abs();
                let flag = |k: &str| e.get(k).is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
                out.volumes.push(AiVolume {
                    name: e.get("targetname").unwrap_or("").into(),
                    class: class.into(),
                    min: c - half,
                    max: c + half,
                    only_jump_down: flag("OnlyJumpDown"),
                });
            } else if class == PATROL_CLASS && e.get("StartDisabled") != Some("1") {
                let next = ["Next", "Next1", "Next2", "Next3", "Next4"]
                    .iter()
                    .filter_map(|k| e.get(k))
                    .filter(|n| !n.is_empty())
                    .map(str::to_string)
                    .collect();
                out.patrol.push(PatrolNode {
                    name: e.get("targetname").unwrap_or("").into(),
                    pos: origin(e, scale),
                    next,
                });
            }
        }
        out
    }

    /// The loaded map's (`MapEntities`), if it has any.
    pub fn of(world: &World) -> Option<Self> {
        let m = world.get_resource::<MapEntities>()?;
        let ai = Self::read(&m.entities, m.scale);
        (!ai.volumes.is_empty() || !ai.patrol.is_empty()).then_some(ai)
    }

    /// The navigation areas of the volumes (3), `unit` metres per world
    /// unit; `floor(min, max)`: the floor inside a box (m), if found.
    /// None without volumes or with more than `MAX_VOLUMES`.
    pub fn nav_mesh(&self, unit: f32, floor: impl Fn(Vec3, Vec3) -> Option<f32>) -> Option<NavMesh> {
        if self.volumes.is_empty() || self.volumes.len() > MAX_VOLUMES {
            return None;
        }
        let mut areas: Vec<NavArea> = self
            .volumes
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let y = floor(v.min, v.max).unwrap_or(v.min.y);
                let (min, max) = (v.min.xz(), v.max.xz());
                NavArea {
                    id: i as u32 + 1,
                    flags: 0,
                    min,
                    max,
                    heights: [y; 4],
                    center: Vec3::new((min.x + max.x) * 0.5, y, (min.y + max.y) * 0.5),
                    links: Vec::new(),
                    place: None,
                    hiding: Vec::new(),
                    visible: Vec::new(),
                    encounters: Vec::new(),
                }
            })
            .collect();
        const EPS: f32 = 1e-3;
        for i in 0..self.volumes.len() {
            for j in i + 1..self.volumes.len() {
                let (a, b) = (&self.volumes[i], &self.volumes[j]);
                let lo = a.min.max(b.min);
                let hi = a.max.min(b.max);
                if (lo - hi).cmpgt(Vec3::splat(EPS)).any() {
                    continue;
                }
                // The shared top rectangle, crossed along its thinner axis.
                let (wx, wz) = (hi.x - lo.x, hi.z - lo.z);
                let (side, length) = if wx <= wz {
                    let side = if b.min.x + b.max.x > a.min.x + a.max.x { Side::MaxX } else { Side::MinX };
                    (side, wz)
                } else {
                    let side = if b.min.z + b.max.z > a.min.z + a.max.z { Side::MaxZ } else { Side::MinZ };
                    (side, wx)
                };
                if length / unit < MIN_LINK {
                    continue;
                }
                // Jump-up volumes link one way (3); a volume keeps at most
                // `MAX_NEIGHBOURS` candidates.
                if a.may_leave_to(b) && b.jump_or_any(a) && areas[i].links.len() < MAX_NEIGHBOURS {
                    areas[i].links.push((j, Via::Walk(side)));
                }
                if b.may_leave_to(a) && a.jump_or_any(b) && areas[j].links.len() < MAX_NEIGHBOURS {
                    areas[j].links.push((i, Via::Walk(side.opposite())));
                }
            }
        }
        Some(NavMesh {
            areas,
            ladders: Vec::new(),
            places: Vec::new(),
            // Made for AI walkers: its links pass doors that may never
            // open and jump-ups a player can't climb, so a player's spawn
            // spots come from a walk instead (`minigame::spawns`).
            generated: true,
        })
    }

    /// The patrol graph of the map's nodes (4) on `mesh`, None without
    /// nodes.
    pub fn patrol_graph(&self, mesh: &NavMesh) -> Option<PatrolGraph> {
        if self.patrol.is_empty() || mesh.areas.is_empty() {
            return None;
        }
        let index = |name: &str| self.patrol.iter().position(|n| n.name.eq_ignore_ascii_case(name));
        let mut links = vec![Vec::new(); self.patrol.len()];
        for (k, n) in self.patrol.iter().enumerate() {
            for next in n.next.iter().filter_map(|m| index(m)).filter(|&m| m != k) {
                for (from, to) in [(k, next), (next, k)] {
                    if !links[from].contains(&to) {
                        links[from].push(to);
                    }
                }
            }
        }
        let mut nodes = Vec::new();
        let mut points = Vec::new();
        for n in &self.patrol {
            let area = mesh.nearest_area(n.pos)?;
            let a = &mesh.areas[area];
            nodes.push(area);
            points.push(if a.overlaps(n.pos.x, n.pos.z) {
                Vec3::new(n.pos.x, a.height_at(n.pos.x, n.pos.z), n.pos.z)
            } else {
                a.closest_point(n.pos)
            });
        }
        let names = self.patrol.iter().map(|n| n.name.clone()).collect();
        Some(PatrolGraph { nodes, points, links, names })
    }
}
