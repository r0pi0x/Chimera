//! Spawn safety on a map or an arena slice (docs/plans/active/
//! arena-slices.md, section 5 and A3): the measures of the owner's worry
//! ("a boundary that is just a full flat area, and then people just spawn
//! in each other's view and die almost instantly"), and the spawn points
//! a slice makes for itself where the map gives too few. Plain functions
//! over positions and a line-of-sight test the caller gives (`Sees`: a
//! physics ray in a game and in the `map_fitness` tool, a plan of walls in
//! the unit tests), so the tool, the game and the tests measure alike.
//!
//! - The visibility table (`SeeTable`) between spawn spots (the 1.5 m grid
//!   a gun game spawns on, `spawns::SpawnSpots`; sampled evenly to at most
//!   a few thousand): a pair sees when a ray from one's eye (`EYE` over the
//!   feet) reaches the other's chest or head (`CHEST`, `EYE`), either way,
//!   within `RANGE`.
//! - From it (`Safety::of`): `see_pairs`, the share of pairs that see each
//!   other (a flat open field is near 1); `spot_seen_by`, the mean share of
//!   the other spots that see a spot, and its 90th percentile (the first
//!   of the two numbers the owner asked for: from each spawn spot, how
//!   many others have a clear line to it); `seen_at_spawn` at N players,
//!   the chance a fresh spawn is in sight of at least one of N - 1 enemies
//!   standing at random spots when it takes the least seen of `CHOICES`
//!   random spots (what `points = "scored"` does, in short; Monte Carlo,
//!   `TRIALS` draws, seeded); `long_pair_m`, the longest pair that sees.
//! - Spawn points (`point_safety`): for each of the map's spawn points
//!   inside, how many of the other points see it and the share of the
//!   walkable floor (the spots) that sees it; `floor_sees_point`, the share
//!   of the floor that sees at least one spawn point (the second number).
//! - Cover and long sight (`level_rays`): from each spot's eye 16 level
//!   rays; `cover`, the share of spots with something solid within
//!   `COVER_M` in at least 4 of them; `long_sight_p95_m`, the 95th
//!   percentile of the ray lengths (capped at `SIGHT_CAP`).
//! - Spawn points made (`choose_points`): where a slice has fewer than it
//!   wants, more chosen from the spots, at least a few metres from its
//!   wall, each one the least seen by the points already there (then the
//!   least seen by the floor, then the farthest from them), deterministic
//!   from the seed given (the slice's key's hash): the same inputs give
//!   the same points on every machine.
//!
//! Known shortcuts: sight is rays without a field of view, fog or
//! lighting (a player facing away still "sees"); the spots stand for the
//! floor (a share of spots is a share of floor only as far as the grid is
//! even); `seen_at_spawn` puts enemies at random spots, not where play
//! takes them.

use bevy::prelude::*;

use crate::{map::play_area::PlanPolygon, weapon::random::Rng};

/// A player's eye and chest over the feet (m).
pub const EYE: f32 = 1.6;
pub const CHEST: f32 = 1.2;
/// The farthest pair the table counts (m).
pub const RANGE: f32 = 120.0;
/// Random spots a spawn chooses among in `seen_at_spawn`.
pub const CHOICES: usize = 16;
/// Monte Carlo draws for `seen_at_spawn`.
pub const TRIALS: usize = 2000;
/// Solid within this (m) on a level ray counts as cover.
pub const COVER_M: f32 = 3.0;
/// The longest level ray (m).
pub const SIGHT_CAP: f32 = 120.0;
/// The player counts the measures are given at.
pub const COUNTS: [usize; 5] = [2, 4, 8, 12, 16];

/// Whether a straight line from the first point to the second is clear.
pub trait Sees: Fn(Vec3, Vec3) -> bool {}
impl<T: Fn(Vec3, Vec3) -> bool> Sees for T {}

/// Whether players standing at `a` and `b` (feet) see each other: one's
/// eye to the other's chest or head, either way, within `RANGE`.
pub fn pair_sees(sees: &impl Sees, a: Vec3, b: Vec3) -> bool {
    if a.distance(b) > RANGE {
        return false;
    }
    let (ea, eb) = (a + Vec3::Y * EYE, b + Vec3::Y * EYE);
    sees(ea, eb) || sees(ea, b + Vec3::Y * CHEST) || sees(eb, a + Vec3::Y * CHEST)
}

/// At most `max` of `spots`, evenly (every k-th in a fixed order: by x,
/// then z, then y).
pub fn sample(spots: &[Vec3], max: usize) -> Vec<Vec3> {
    let mut s = spots.to_vec();
    s.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.z.total_cmp(&b.z)).then(a.y.total_cmp(&b.y)));
    if s.len() <= max || max == 0 {
        return s;
    }
    let step = s.len() as f64 / max as f64;
    (0..max).map(|i| s[(i as f64 * step) as usize]).collect()
}

/// Which pairs of spots see each other (symmetric).
#[derive(Clone, Debug)]
pub struct SeeTable {
    pub spots: Vec<Vec3>,
    bits: Vec<u64>,
}

impl SeeTable {
    /// The table of `spots` (`pair_sees` for each pair).
    pub fn build(spots: &[Vec3], sees: &impl Sees) -> Self {
        let n = spots.len();
        let mut t = Self {
            spots: spots.to_vec(),
            bits: vec![0; (n * n).div_ceil(64)],
        };
        for i in 0..n {
            for j in i + 1..n {
                if pair_sees(sees, spots[i], spots[j]) {
                    t.set(i, j);
                    t.set(j, i);
                }
            }
        }
        t
    }

    fn set(&mut self, i: usize, j: usize) {
        let k = i * self.spots.len() + j;
        self.bits[k / 64] |= 1 << (k % 64);
    }

    pub fn len(&self) -> usize {
        self.spots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spots.is_empty()
    }

    /// Whether spots `i` and `j` see each other.
    pub fn sees(&self, i: usize, j: usize) -> bool {
        let k = i * self.spots.len() + j;
        self.bits[k / 64] & (1 << (k % 64)) != 0
    }

    /// How many other spots see spot `i`.
    pub fn seen_by(&self, i: usize) -> usize {
        (0..self.len()).filter(|j| *j != i && self.sees(i, *j)).count()
    }
}

/// The table's measures (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Safety {
    pub spots: usize,
    pub see_pairs: f32,
    /// The share of the other spots that see a spot: mean, 90th percentile.
    pub spot_seen_by: f32,
    pub spot_seen_by_p90: f32,
    /// (N, chance) for each of `COUNTS`.
    pub seen_at_spawn: Vec<(usize, f32)>,
    pub long_pair_m: f32,
}

impl Safety {
    /// The measures of `t`, the Monte Carlo seeded by `seed`.
    pub fn of(t: &SeeTable, seed: u32) -> Self {
        let n = t.len();
        if n < 2 {
            return Self {
                spots: n,
                seen_at_spawn: COUNTS.iter().map(|c| (*c, 0.0)).collect(),
                ..default()
            };
        }
        let mut pairs = 0u64;
        let mut long: f32 = 0.0;
        let mut shares: Vec<f32> = Vec::with_capacity(n);
        for i in 0..n {
            let mut seen = 0usize;
            for j in 0..n {
                if j != i && t.sees(i, j) {
                    seen += 1;
                    if j > i {
                        pairs += 1;
                        long = long.max(t.spots[i].distance(t.spots[j]));
                    }
                }
            }
            shares.push(seen as f32 / (n - 1) as f32);
        }
        let mean = shares.iter().sum::<f32>() / n as f32;
        shares.sort_by(f32::total_cmp);
        let p90 = shares[((n - 1) as f32 * 0.9).round() as usize];
        let mut rng = Rng::new(seed ^ 0x5afe_5eed);
        Self {
            spots: n,
            see_pairs: pairs as f32 / (n * (n - 1) / 2) as f32,
            spot_seen_by: mean,
            spot_seen_by_p90: p90,
            seen_at_spawn: COUNTS.iter().map(|c| (*c, seen_at_spawn(t, *c, &mut rng))).collect(),
            long_pair_m: long,
        }
    }

    /// `seen_at_spawn` at `n` players (the nearest count measured at or
    /// above it; the last past the end).
    pub fn seen_at(&self, n: usize) -> f32 {
        self.seen_at_spawn.iter().find(|(c, _)| *c >= n).or(self.seen_at_spawn.last()).map_or(0.0, |(_, s)| *s)
    }
}

fn below(rng: &mut Rng, n: usize) -> usize {
    ((rng.next() * n as f32) as usize).min(n - 1)
}

/// The chance a fresh spawn is seen at once with `players` in the match
/// (module doc).
pub fn seen_at_spawn(t: &SeeTable, players: usize, rng: &mut Rng) -> f32 {
    let n = t.len();
    if n < 2 || players < 2 {
        return 0.0;
    }
    let mut seen = 0usize;
    let mut enemies = Vec::with_capacity(players);
    for _ in 0..TRIALS {
        enemies.clear();
        enemies.extend((0..players - 1).map(|_| below(rng, n)));
        let best = (0..CHOICES)
            .map(|_| {
                let s = below(rng, n);
                enemies.iter().filter(|e| **e == s || t.sees(s, **e)).count()
            })
            .min()
            .unwrap_or(0);
        if best > 0 {
            seen += 1;
        }
    }
    seen as f32 / TRIALS as f32
}

/// One spawn point's exposure.
#[derive(Clone, Debug, PartialEq)]
pub struct PointSafety {
    pub at: Vec3,
    /// Of the other spawn points, how many see it.
    pub seen_by_points: usize,
    /// The share of the floor (the spots) that sees it.
    pub floor_seeing: f32,
}

/// The spawn points' exposure (module doc), and the share of the floor
/// that sees at least one of them.
pub fn point_safety(points: &[Vec3], floor: &[Vec3], sees: &impl Sees) -> (Vec<PointSafety>, f32) {
    let seeing: Vec<Vec<bool>> = points.iter().map(|p| floor.iter().map(|f| pair_sees(sees, *p, *f)).collect()).collect();
    let out = points
        .iter()
        .enumerate()
        .map(|(i, p)| PointSafety {
            at: *p,
            seen_by_points: points.iter().enumerate().filter(|(j, q)| *j != i && pair_sees(sees, *p, **q)).count(),
            floor_seeing: seeing[i].iter().filter(|s| **s).count() as f32 / floor.len().max(1) as f32,
        })
        .collect();
    let any = (0..floor.len()).filter(|k| seeing.iter().any(|s| s[*k])).count();
    (out, any as f32 / floor.len().max(1) as f32)
}

/// Cover and long sight from the spots' eyes (module doc): `ray(from,
/// dir, max)` is the distance to the first solid thing, None past `max`.
pub fn level_rays(spots: &[Vec3], ray: &impl Fn(Vec3, Vec3, f32) -> Option<f32>) -> (f32, f32) {
    if spots.is_empty() {
        return (0.0, 0.0);
    }
    let mut covered = 0usize;
    let mut lengths = Vec::with_capacity(spots.len() * 16);
    for s in spots {
        let eye = *s + Vec3::Y * EYE;
        let mut near = 0;
        for k in 0..16 {
            let t = k as f32 * std::f32::consts::TAU / 16.0;
            let d = ray(eye, Vec3::new(t.cos(), 0.0, t.sin()), SIGHT_CAP).unwrap_or(SIGHT_CAP);
            if d <= COVER_M {
                near += 1;
            }
            lengths.push(d);
        }
        if near >= 4 {
            covered += 1;
        }
    }
    lengths.sort_by(f32::total_cmp);
    let p95 = lengths[((lengths.len() - 1) as f32 * 0.95).round() as usize];
    (covered as f32 / spots.len() as f32, p95)
}

/// Every measure of a map or a slice (`map_fit::SliceLimits::judge`
/// reads it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// The walkable area (m2) and mean sight (m) as the pool measures them.
    pub area_m2: f32,
    pub sight_m: f32,
    pub safety: Safety,
    pub cover: f32,
    pub long_sight_p95_m: f32,
    pub points: Vec<PointSafety>,
    pub floor_sees_point: f32,
}

impl Report {
    /// The measures: `spots` sampled to at most `max_spots` for the table,
    /// the spawn points `points`, `sees` and `ray` the world's (module doc).
    pub fn measure(
        spots: &[Vec3],
        points: &[Vec3],
        sees: &impl Sees,
        ray: &impl Fn(Vec3, Vec3, f32) -> Option<f32>,
        max_spots: usize,
        seed: u32,
        (area_m2, sight_m): (f32, f32),
    ) -> Self {
        let table = SeeTable::build(&sample(spots, max_spots), sees);
        let (cover, long_sight_p95_m) = level_rays(&table.spots, ray);
        let (points, floor_sees_point) = point_safety(points, &table.spots, sees);
        Self {
            area_m2,
            sight_m,
            safety: Safety::of(&table, seed),
            cover,
            long_sight_p95_m,
            points,
            floor_sees_point,
        }
    }

    /// The share of the other spawn points that see the most seen one.
    pub fn point_seen_by(&self) -> f32 {
        let n = self.points.len();
        if n < 2 {
            return 0.0;
        }
        self.points.iter().map(|p| p.seen_by_points).max().unwrap_or(0) as f32 / (n - 1) as f32
    }

    /// The mean share of the floor seeing a spawn point.
    pub fn point_floor_seeing(&self) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }
        self.points.iter().map(|p| p.floor_seeing).sum::<f32>() / self.points.len() as f32
    }

    /// The measures as lines of text (the tool's output).
    pub fn lines(&self) -> Vec<String> {
        let s = &self.safety;
        let mut v = vec![
            format!("area_m2 {:.0}, sight_m {:.1}", self.area_m2, self.sight_m),
            format!(
                "spots in the table {}: see_pairs {:.3}; a spot is seen by {:.1}% of the other spots (p90 {:.1}%); longest seeing pair {:.0} m",
                s.spots,
                s.see_pairs,
                100.0 * s.spot_seen_by,
                100.0 * s.spot_seen_by_p90,
                s.long_pair_m
            ),
            format!(
                "seen_at_spawn: {}",
                s.seen_at_spawn.iter().map(|(n, v)| format!("{n}={v:.3}")).collect::<Vec<_>>().join(" ")
            ),
            format!("cover {:.3}, long_sight_p95 {:.1} m", self.cover, self.long_sight_p95_m),
            format!(
                "spawn points {}: the floor seeing at least one {:.1}%, each seen by {:.1}% of the floor on average; the most seen point by {:.0}% of the other points",
                self.points.len(),
                100.0 * self.floor_sees_point,
                100.0 * self.point_floor_seeing(),
                100.0 * self.point_seen_by()
            ),
        ];
        for p in &self.points {
            v.push(format!(
                "  point {:.1} {:.1} {:.1}: seen by {} of the other {} points, by {:.1}% of the floor",
                p.at.x,
                p.at.y,
                p.at.z,
                p.seen_by_points,
                self.points.len().saturating_sub(1),
                100.0 * p.floor_seeing
            ));
        }
        v
    }
}

/// How a slice's spawn points are made (`choose_points`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointRules {
    /// The points a slice wants (the map's inside counted).
    pub want: usize,
    /// Never nearer its wall than this (m).
    pub wall_gap: f32,
    /// Apart from each other at least this, where the spots allow (m).
    pub apart: f32,
}

impl Default for PointRules {
    fn default() -> Self {
        Self {
            want: 8,
            wall_gap: 4.0,
            apart: 10.0,
        }
    }
}

/// Spots tried as new points at most, and floor spots each is checked
/// against for its exposure.
const POINT_CANDIDATES: usize = 400;
const EXPOSURE_FLOOR: usize = 96;

/// The spawn points to add to `existing` (the map's inside, the seed's)
/// from `spots`, up to `rules.want` in all (module doc); deterministic
/// for the same inputs and `seed`.
pub fn choose_points(spots: &[Vec3], existing: &[Vec3], plan: &PlanPolygon, sees: &impl Sees, rules: PointRules, seed: u32) -> Vec<Vec3> {
    let need = rules.want.saturating_sub(existing.len());
    if need == 0 {
        return Vec::new();
    }
    let all = sample(spots, usize::MAX);
    let floor = sample(&all, EXPOSURE_FLOOR);
    let mut pool: Vec<Vec3> = all.iter().copied().filter(|s| plan.contains3(*s) && plan.edge_distance(s.xz()) >= rules.wall_gap).collect();
    // A seeded shuffle, then the first `POINT_CANDIDATES`.
    let mut rng = Rng::new(seed ^ 0x9017_5eed);
    for i in (1..pool.len()).rev() {
        pool.swap(i, below(&mut rng, i + 1));
    }
    pool.truncate(POINT_CANDIDATES);
    let exposure: Vec<f32> = pool
        .iter()
        .map(|p| floor.iter().filter(|f| pair_sees(sees, *p, **f)).count() as f32 / floor.len().max(1) as f32)
        .collect();
    let mut chosen: Vec<Vec3> = existing.to_vec();
    let mut seen_by: Vec<usize> = pool.iter().map(|p| chosen.iter().filter(|c| pair_sees(sees, *p, **c)).count()).collect();
    let mut out = Vec::new();
    let mut taken = vec![false; pool.len()];
    for _ in 0..need {
        let near = |p: Vec3, chosen: &[Vec3]| chosen.iter().map(|c| c.distance(p)).fold(f32::INFINITY, f32::min);
        let pick = |gap: f32| {
            (0..pool.len())
                .filter(|k| !taken[*k] && near(pool[*k], &chosen) >= gap)
                .min_by(|a, b| {
                    let key = |k: usize| (seen_by[k], exposure[k], -near(pool[k], &chosen).min(60.0));
                    let (x, y) = (key(*a), key(*b));
                    x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)).then(x.2.total_cmp(&y.2)).then(a.cmp(b))
                })
        };
        let Some(k) = pick(rules.apart).or_else(|| pick(rules.apart / 2.0)).or_else(|| pick(2.0)) else { break };
        taken[k] = true;
        let p = pool[k];
        chosen.push(p);
        out.push(p);
        for (j, q) in pool.iter().enumerate() {
            if !taken[j] && pair_sees(sees, *q, p) {
                seen_by[j] += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plan of walls (x, z segments) seen through by nothing.
    fn walls(segs: &[(Vec2, Vec2)]) -> impl Fn(Vec3, Vec3) -> bool + '_ {
        move |a: Vec3, b: Vec3| {
            let (p, q) = (a.xz(), b.xz());
            !segs.iter().any(|(c, d)| {
                let side = |o: Vec2, x: Vec2, y: Vec2| (x - o).perp_dot(y - o);
                (side(c.to_owned(), *d, p) > 0.0) != (side(*c, *d, q) > 0.0) && (side(p, q, *c) > 0.0) != (side(p, q, *d) > 0.0)
            })
        }
    }

    fn grid(lo: Vec2, hi: Vec2, step: f32) -> Vec<Vec3> {
        let mut v = Vec::new();
        let mut x = lo.x;
        while x <= hi.x {
            let mut z = lo.y;
            while z <= hi.y {
                v.push(Vec3::new(x, 0.0, z));
                z += step;
            }
            x += step;
        }
        v
    }

    /// The distance along `dir` from `from` to the first wall.
    fn ray_of<'a>(segs: &'a [(Vec2, Vec2)]) -> impl Fn(Vec3, Vec3, f32) -> Option<f32> + 'a {
        move |from: Vec3, dir: Vec3, max: f32| {
            let (p, d) = (from.xz(), dir.xz());
            segs.iter()
                .filter_map(|(a, b)| {
                    let e = *b - *a;
                    let den = d.perp_dot(e);
                    if den.abs() < 1e-9 {
                        return None;
                    }
                    let t = (*a - p).perp_dot(e) / den;
                    let u = (*a - p).perp_dot(d) / den;
                    (t >= 0.0 && (0.0..=1.0).contains(&u)).then_some(t)
                })
                .fold(None, |m: Option<f32>, t| Some(m.map_or(t, |m| m.min(t))))
                .filter(|t| *t <= max)
        }
    }

    fn seg(a: [f32; 2], b: [f32; 2]) -> (Vec2, Vec2) {
        (Vec2::from_array(a), Vec2::from_array(b))
    }

    /// A 60 m square room's outline.
    fn room() -> Vec<(Vec2, Vec2)> {
        vec![seg([0., 0.], [60., 0.]), seg([60., 0.], [60., 60.]), seg([60., 60.], [0., 60.]), seg([0., 60.], [0., 0.])]
    }

    /// The room cut into 10 m cells by walls with a 2 m door in each.
    fn maze() -> Vec<(Vec2, Vec2)> {
        let mut w = room();
        for k in 1..6 {
            let c = k as f32 * 10.0;
            for cell in 0..6 {
                let lo = cell as f32 * 10.0;
                // Doors staggered so no straight line runs far.
                let door = lo + if (k + cell) % 2 == 0 { 2.0 } else { 6.0 };
                w.push(seg([c, lo], [c, door]));
                w.push(seg([c, door + 2.0], [c, lo + 10.0]));
                w.push(seg([lo, c], [door, c]));
                w.push(seg([door + 2.0, c], [lo + 10.0, c]));
            }
        }
        w
    }

    fn report(walls_: &[(Vec2, Vec2)], points: &[Vec3]) -> Report {
        let spots = grid(Vec2::splat(0.75), Vec2::splat(59.25), 1.5);
        let sees = walls(walls_);
        let ray = ray_of(walls_);
        // The pool's sight: level rays capped at 50 m.
        let (_, _) = level_rays(&spots, &ray);
        let sight = spots
            .iter()
            .flat_map(|s| (0..16).map(move |k| (*s, k)))
            .map(|(s, k)| {
                let t = k as f32 * std::f32::consts::TAU / 16.0;
                ray(s, Vec3::new(t.cos(), 0.0, t.sin()), 50.0).unwrap_or(50.0)
            })
            .sum::<f32>()
            / (spots.len() * 16) as f32;
        Report::measure(&spots, points, &sees, &ray, 800, 3, (3600.0, sight))
    }

    fn corners() -> Vec<Vec3> {
        [[5.0, 5.0], [55.0, 5.0], [5.0, 55.0], [55.0, 55.0], [25.0, 35.0], [35.0, 25.0]]
            .iter()
            .map(|p| Vec3::new(p[0], 0.0, p[1]))
            .collect()
    }

    fn limits() -> (super::super::map_fit::SliceLimits, super::super::map_fit::FitSettings) {
        let fit = super::super::map_fit::FitSettings::shipped();
        (fit.slice.clone(), fit)
    }

    #[test]
    fn a_walled_maze_passes() {
        let (l, fit) = limits();
        let r = report(&maze(), &corners());
        let v = l.judge(&r, &fit);
        assert!(v.failed.is_empty(), "{:?}\n{}", v.failed, r.lines().join("\n"));
        assert!(v.players.is_some());
    }

    #[test]
    fn a_flat_open_field_fails_on_see_pairs_and_seen_at_spawn() {
        let (l, fit) = limits();
        let r = report(&room(), &corners());
        assert!(r.safety.see_pairs > 0.99, "{}", r.safety.see_pairs);
        let v = l.judge(&r, &fit);
        assert!(v.failed.iter().any(|f| f.starts_with("see_pairs")), "{:?}", v.failed);
        assert!(v.failed.iter().any(|f| f.starts_with("no player count")), "seen_at_spawn: {:?}", v.failed);
        assert_eq!(v.players, None);
    }

    #[test]
    fn a_slice_whose_spawn_points_all_see_each_other_fails() {
        let (l, fit) = limits();
        // The maze, its points all in one cell.
        let one_cell: Vec<Vec3> = [[2.0, 2.0], [8.0, 2.0], [2.0, 8.0], [8.0, 8.0]].iter().map(|p| Vec3::new(p[0], 0.0, p[1])).collect();
        let r = report(&maze(), &one_cell);
        let v = l.judge(&r, &fit);
        assert!(v.failed.iter().any(|f| f.starts_with("point_seen_by")), "{:?}", v.failed);
        assert_eq!(v.players, None);
    }

    #[test]
    fn a_table_is_symmetric_and_walls_block_it() {
        let w = [(Vec2::new(5.0, -10.0), Vec2::new(5.0, 10.0))];
        let sees = walls(&w);
        let spots = [Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0), Vec3::new(8.0, 0.0, 0.0)];
        let t = SeeTable::build(&spots, &sees);
        assert!(t.sees(0, 1) && t.sees(1, 0));
        assert!(!t.sees(0, 2) && !t.sees(2, 1));
        assert_eq!(t.seen_by(0), 1);
    }

    #[test]
    fn spawn_points_are_chosen_apart_off_the_wall_and_the_same_each_time() {
        let plan = PlanPolygon::new(&[Vec2::new(0.0, 0.0), Vec2::new(60.0, 0.0), Vec2::new(60.0, 60.0), Vec2::new(0.0, 60.0)]).unwrap();
        // A wall across the middle with a door: two rooms.
        let w = [(Vec2::new(30.0, 0.0), Vec2::new(30.0, 26.0)), (Vec2::new(30.0, 34.0), Vec2::new(30.0, 60.0))];
        let sees = walls(&w);
        let spots = grid(Vec2::splat(1.0), Vec2::splat(59.0), 1.5);
        let rules = PointRules { want: 4, ..default() };
        let a = choose_points(&spots, &[Vec3::new(10.0, 0.0, 10.0)], &plan, &sees, rules, 7);
        assert_eq!(a.len(), 3);
        assert_eq!(a, choose_points(&spots, &[Vec3::new(10.0, 0.0, 10.0)], &plan, &sees, rules, 7), "deterministic");
        for p in &a {
            assert!(plan.edge_distance(p.xz()) >= rules.wall_gap, "{p} near the wall");
        }
        // The first one made is in the other room, out of the seed's sight.
        assert!(a[0].x > 30.0, "{:?}", a);
    }
}
