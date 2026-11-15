//! King of the hill (specs/halo_ce/multiplayer_rules.md 6.6; scenario.md
//! "Netgame flags": hills): a hill is the hill flags (type 8) sharing a
//! team index (the hill's number), the first 12 of them in block order;
//! their convex hull in the horizontal plane is the hill (one flag alone, a
//! 2 WU square around it), from the lowest flag - 0.1 WU to the highest
//! + 0.8 WU. A player is on the hill when its unit's centre is inside.
//!
//! Each Halo tick while the game can score (`tick`): every player on the
//! hill's time +1 (its score), its team +1 but at most once a tick however
//! many of its players stand there (free for all: the player's own);
//! contested hills score for every team on them. The game starts on hill
//! number 0 (the lowest number when there is none); a moving hill moves
//! every 1800 ticks to a random other hill number. The score to win is in
//! minutes (x 1800 ticks: `variant::winning_score`).
//!
//! Known shortcuts: the hill is not drawn (the hilltop shader) and has no
//! marker; "hill contested", "controlled" and the chimes are not played;
//! the 2 WU square of a lone flag is axis-aligned (6.6 does not say how it
//! turns); two flags make a hull of no area that nobody stands in.

use bevy::prelude::*;
use blam_formats::{game_type::GameTypeOptions, scenario::NetgameFlagType};
use chimera_sim::mode::{Match, side};

use super::{
    HaloGameType, HaloMatch, HaloPlayer,
    netgame::{Netgame, WU},
    scoring::{HaloAnnounce, Line},
};
use crate::core::{Dead, Score, Team};

/// Hill flags used per hill.
pub const MAX_FLAGS: usize = 12;
/// Ticks between a moving hill's moves.
pub const MOVE_TICKS: i64 = 1800;
/// The hill's volume against its lowest and highest flag (WU).
pub const BOTTOM: f32 = -0.1;
pub const TOP: f32 = 0.8;

/// A hill: its outline (horizontal, metres: engine x and z, counter-
/// clockwise) and its floor and ceiling (engine y, metres).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hill {
    pub outline: Vec<Vec2>,
    pub bottom: f32,
    pub top: f32,
}

fn cross(o: Vec2, a: Vec2, b: Vec2) -> f32 {
    (a - o).perp_dot(b - o)
}

/// The convex hull of `points` (counter-clockwise, no collinear points).
pub fn convex_hull(points: &[Vec2]) -> Vec<Vec2> {
    let mut p: Vec<Vec2> = points.to_vec();
    p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let mut lower: Vec<Vec2> = Vec::new();
    for q in &p {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], *q) <= 0.0 {
            lower.pop();
        }
        lower.push(*q);
    }
    let mut upper: Vec<Vec2> = Vec::new();
    for q in p.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], *q) <= 0.0 {
            upper.pop();
        }
        upper.push(*q);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// The hill of flags at `points` (metres; the first 12 are used).
pub fn hill_of(points: &[Vec3]) -> Hill {
    let points = &points[..points.len().min(MAX_FLAGS)];
    if points.is_empty() {
        return Hill::default();
    }
    let flat: Vec<Vec2> = points.iter().map(|p| Vec2::new(p.x, p.z)).collect();
    let outline = if flat.len() == 1 {
        let (c, h) = (flat[0], WU);
        vec![c + Vec2::new(-h, -h), c + Vec2::new(h, -h), c + Vec2::new(h, h), c + Vec2::new(-h, h)]
    } else {
        convex_hull(&flat)
    };
    let low = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let high = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    Hill { outline, bottom: low + BOTTOM * WU, top: high + TOP * WU }
}

impl Hill {
    /// Whether `p` (metres) is inside.
    pub fn contains(&self, p: Vec3) -> bool {
        if self.outline.len() < 3 || p.y < self.bottom || p.y > self.top {
            return false;
        }
        let q = Vec2::new(p.x, p.z);
        let n = self.outline.len();
        (0..n).all(|i| cross(self.outline[i], self.outline[(i + 1) % n], q) >= 0.0)
    }
}

/// The hill numbers of the map, ascending.
pub fn numbers(n: &Netgame) -> Vec<i16> {
    let mut v: Vec<i16> = n.flags_of(NetgameFlagType::Hill).map(|f| f.team).collect();
    v.sort();
    v.dedup();
    v
}

/// Hill `number` of the map.
pub fn hill(n: &Netgame, number: i16) -> Hill {
    let points: Vec<Vec3> = n.flags_of(NetgameFlagType::Hill).filter(|f| f.team == number).map(|f| f.at).collect();
    hill_of(&points)
}

/// The hill in play.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct KingState {
    pub number: i16,
    pub hill: Hill,
    /// A moving hill's next move (game time).
    pub move_at: Option<i64>,
}

fn moving(world: &World) -> bool {
    matches!(world.resource::<HaloGameType>().0.rules.options, GameTypeOptions::KingOfTheHill { moving_hill: true })
}

/// The hill at the game's start (the round's setup): number 0.
pub fn setup(world: &mut World) {
    let n = world.get_resource::<Netgame>().cloned().unwrap_or_default();
    let list = numbers(&n);
    let number = if list.contains(&0) { 0 } else { list.first().copied().unwrap_or(0) };
    let now = world.resource::<HaloMatch>().time;
    let move_at = moving(world).then_some(now + MOVE_TICKS);
    world.insert_resource(KingState { number, hill: hill(&n, number), move_at });
}

/// One Halo tick of king of the hill (module doc).
pub fn tick(world: &mut World) {
    let (now, over) = {
        let m = world.resource::<HaloMatch>();
        (m.time, m.over)
    };
    if over {
        return;
    }
    let Some(state) = world.get_resource::<KingState>().cloned() else { return };
    let on: Vec<(Entity, u8)> = world
        .query_filtered::<(Entity, &Transform, &Team), (With<HaloPlayer>, Without<Dead>)>()
        .iter(world)
        .filter(|(_, t, _)| state.hill.contains(t.translation))
        .map(|(e, _, t)| (e, t.0))
        .collect();
    let mut scored: Vec<u8> = Vec::new();
    for (e, team) in on {
        if let Some(mut p) = world.get_mut::<HaloPlayer>(e) {
            p.score += 1;
        }
        if let Some(mut s) = world.get_mut::<Score>(e) {
            s.kills += 1;
        }
        if !scored.contains(&team) {
            scored.push(team);
            if let Some(i) = side(team) {
                world.resource_mut::<Match>().score[i] += 1;
            }
        }
    }
    // The moving hill.
    if let Some(at) = state.move_at
        && now >= at
    {
        let n = world.get_resource::<Netgame>().cloned().unwrap_or_default();
        let others: Vec<i16> = numbers(&n).into_iter().filter(|x| *x != state.number).collect();
        let mut next = state.clone();
        next.move_at = Some(now + MOVE_TICKS);
        if !others.is_empty() {
            let i = world.resource_mut::<HaloMatch>().roll(others.len() as u32) as usize;
            next.number = others[i];
            next.hill = hill(&n, next.number);
            world.write_message(HaloAnnounce { line: Line::HillMove, player: None });
        }
        world.insert_resource(next);
    }
}
