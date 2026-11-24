//! The M0 collision spike (docs/plans/active/goldsrc.md §3, "Collision"):
//! is hull 0's solid space as convex regions, plus hull 1's solid regions
//! shrunk back by the standing box, close enough to hull 1 for Chimera's
//! physics? Ground truth is hull 1 itself: a standing box centred at p is
//! blocked exactly when hull 1 calls p solid, and its first contact on a
//! move is hull 1's trace. Three reconstructions are compared:
//!
//! - `hull0`: hull 0's solid regions only (what bullets see; no player
//!   clip);
//! - `measured`: hull 0 plus hull 1's regions shrunk by the compiler's
//!   measured growth (`Shrink::Measured`);
//! - `support`: hull 0 plus hull 1's regions shrunk by the box's full
//!   support distance (`Shrink::Support`);
//! - `hull1only`: hull 1's regions shrunk as `measured`, without hull 0.
//!
//! A region's face is moved back only where it borders hull 1's open
//! space (`solids::exposed_in`): faces between two solid regions of the
//! tree stay, or a solid split by the tree's planes would lose a box's
//! width at every split.
//!
//! Points are drawn uniformly in the world's bounds and kept where hull 0
//! is open (the box's centre in the air); traces start at points where
//! hull 1 and the reconstruction agree the box is free and move 32-256
//! units in a random, mostly horizontal direction. A mismatch's error is
//! how far the box is from agreeing: its depth in the reconstruction
//! (blocked by it but free in hull 1), or how far its centre is from hull
//! 1's open space (free in it but solid in hull 1); a trace's error is the
//! difference of the two contact distances. Defaults keep the test quick
//! on de_dust2; `CHIMERA_GOLDSRC_SPIKE_SAMPLES=<points>` and
//! `CHIMERA_GOLDSRC_SPIKE_MAPS=de_dust2,cs_office,...` (best with
//! `--release`) give the plan's numbers. The test asserts only that the
//! reconstruction reads and prints the table.

use std::collections::HashMap;

use goldsrc_formats::bsp::Bsp;
use goldsrc_formats::bsp::hull::{HULL_BOXES, blocks};
use goldsrc_formats::bsp::solids::{Convex, Shrink, convex_regions, exposed_in, shrink};

use super::install::mount;

const CELL: f64 = 128.0;
/// Overlaps shallower than this count as touching, not blocking.
const TOUCH: f64 = 0.05;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Convex regions in a uniform grid of cells.
struct Grid {
    shapes: Vec<Convex>,
    cells: HashMap<(i32, i32, i32), Vec<u32>>,
}

fn cell(v: f64) -> i32 {
    (v / CELL).floor() as i32
}

impl Grid {
    fn new(shapes: Vec<Convex>) -> Grid {
        let mut cells: HashMap<_, Vec<u32>> = HashMap::new();
        for (i, s) in shapes.iter().enumerate() {
            for x in cell(s.mins[0])..=cell(s.maxs[0]) {
                for y in cell(s.mins[1])..=cell(s.maxs[1]) {
                    for z in cell(s.mins[2])..=cell(s.maxs[2]) {
                        cells.entry((x, y, z)).or_default().push(i as u32);
                    }
                }
            }
        }
        Grid { shapes, cells }
    }

    /// The shapes whose bounds meet the box `lo`..`hi`.
    fn near(&self, lo: [f64; 3], hi: [f64; 3]) -> Vec<&Convex> {
        let mut ids: Vec<u32> = Vec::new();
        for x in cell(lo[0])..=cell(hi[0]) {
            for y in cell(lo[1])..=cell(hi[1]) {
                for z in cell(lo[2])..=cell(hi[2]) {
                    if let Some(v) = self.cells.get(&(x, y, z)) {
                        ids.extend(v);
                    }
                }
            }
        }
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .map(|i| &self.shapes[i as usize])
            .filter(|s| (0..3).all(|a| s.mins[a] <= hi[a] && s.maxs[a] >= lo[a]))
            .collect()
    }

    fn depth(&self, p: [f64; 3], half: [f64; 3]) -> f64 {
        let lo = [p[0] - half[0], p[1] - half[1], p[2] - half[2]];
        let hi = [p[0] + half[0], p[1] + half[1], p[2] + half[2]];
        self.near(lo, hi)
            .iter()
            .map(|s| s.box_overlap(p, half))
            .fold(f64::MIN, f64::max)
    }

    fn sweep(&self, p: [f64; 3], d: [f64; 3], half: [f64; 3]) -> f64 {
        let e = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        let lo = [
            p[0].min(e[0]) - half[0],
            p[1].min(e[1]) - half[1],
            p[2].min(e[2]) - half[2],
        ];
        let hi = [
            p[0].max(e[0]) + half[0],
            p[1].max(e[1]) + half[1],
            p[2].max(e[2]) + half[2],
        ];
        self.near(lo, hi)
            .iter()
            .filter_map(|s| s.box_sweep(p, d, half))
            .fold(1.0, f64::min)
    }
}

#[derive(Default)]
struct Stats {
    points: usize,
    false_blocked: Vec<f64>,
    false_free: Vec<f64>,
    traces: usize,
    trace_err: Vec<f64>,
}

fn pct(n: usize, of: usize) -> f64 {
    100.0 * n as f64 / of.max(1) as f64
}

impl Stats {
    fn line(&self, name: &str) -> String {
        let mis = self.false_blocked.len() + self.false_free.len();
        let over = |v: &[f64], t: f64| v.iter().filter(|&&e| e > t).count();
        let all: Vec<f64> = self.false_blocked.iter().chain(&self.false_free).copied().collect();
        let worst = |v: &[f64]| v.iter().copied().fold(0.0, f64::max);
        let mut te = self.trace_err.clone();
        te.sort_by(f64::total_cmp);
        let p99 = te
            .get(te.len().saturating_sub(1).min(te.len() * 99 / 100))
            .copied()
            .unwrap_or(0.0);
        format!(
            "{name:9} points {:6}: mismatch {:5.2}% (>1u {:5.2}%, >4u {:5.2}%; blocked-not-solid {}, solid-not-blocked {}), worst {:6.2}u | traces {:5}: off >1u {:5.2}%, >4u {:5.2}%, p99 {:5.2}u, worst {:6.2}u",
            self.points,
            pct(mis, self.points),
            pct(over(&all, 1.0), self.points),
            pct(over(&all, 4.0), self.points),
            self.false_blocked.len(),
            self.false_free.len(),
            worst(&all),
            self.traces,
            pct(over(&self.trace_err, 1.0), self.traces),
            pct(over(&self.trace_err, 4.0), self.traces),
            p99,
            worst(&self.trace_err),
        )
    }
}

/// How far `p` (solid in hull 1) is from hull 1's open space: the
/// shortest of 26 directions, stepped by a quarter unit up to 64.
fn out_of_hull1(b: &Bsp, p: [f64; 3]) -> f64 {
    let mut best = 64.0f64;
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                if (dx, dy, dz) == (0, 0, 0) {
                    continue;
                }
                let l = ((dx * dx + dy * dy + dz * dz) as f64).sqrt();
                let d = [dx as f64 / l, dy as f64 / l, dz as f64 / l];
                let mut s = 0.25;
                while s < best {
                    if !blocks(b.contents(0, 1, [p[0] + d[0] * s, p[1] + d[1] * s, p[2] + d[2] * s])) {
                        best = s;
                        break;
                    }
                    s += 0.25;
                }
            }
        }
    }
    best
}

fn measure(map: &str, b: &Bsp, samples: usize) -> Vec<String> {
    let half = HULL_BOXES[1].1.map(|v| v as f64);
    let hull0: Vec<Convex> = convex_regions(b, 0, 0).into_iter().map(|(_, c)| c).collect();
    let hull1: Vec<Convex> = convex_regions(b, 0, 1).into_iter().map(|(_, c)| c).collect();
    let mut out = Vec::new();
    let mut variants: Vec<(&str, Grid)> = vec![("hull0", Grid::new(hull0.clone()))];
    for (name, rule) in [("measured", Shrink::Measured), ("support", Shrink::Support)] {
        let shrunk: Vec<Convex> = hull1
            .iter()
            .filter_map(|c| shrink(c, half, rule, exposed_in(b, 0, 1)))
            .collect();
        let clip_only = shrunk
            .iter()
            .filter(|c| {
                let centre = [0, 1, 2].map(|a| c.vertices.iter().map(|v| v[a]).sum::<f64>() / c.vertices.len() as f64);
                !blocks(b.contents(0, 0, centre))
            })
            .count();
        out.push(format!(
            "{map}: hull 0 {} regions, hull 1 {} regions, {} left after shrinking ({name}), {clip_only} of them outside hull 0's solid",
            hull0.len(),
            hull1.len(),
            shrunk.len()
        ));
        if rule == Shrink::Measured {
            variants.push(("hull1only", Grid::new(shrunk.clone())));
        }
        variants.push((name, Grid::new(hull0.iter().cloned().chain(shrunk).collect())));
    }
    let m = &b.models[0];
    let debug = std::env::var_os("CHIMERA_GOLDSRC_SPIKE_DEBUG").is_some();
    let variants_h0 = Grid::new(hull0.clone());
    for (name, grid) in &variants {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let mut st = Stats::default();
        let mut tries = 0;
        while st.points < samples && tries < samples * 200 {
            tries += 1;
            let p = [0, 1, 2].map(|a| m.mins[a] as f64 + rng.next() * (m.maxs[a] - m.mins[a]) as f64);
            if blocks(b.contents(0, 0, p)) {
                continue;
            }
            st.points += 1;
            let truth = blocks(b.contents(0, 1, p));
            let depth = grid.depth(p, half);
            let recon = depth > TOUCH;
            if recon && !truth {
                st.false_blocked.push(depth);
                if debug {
                    let h0 = variants_h0.depth(p, half);
                    eprintln!("{name} blocked-not-solid at {p:?}: depth {depth:.2}, hull 0's depth {h0:.2}");
                }
            } else if truth && !recon {
                st.false_free.push(out_of_hull1(b, p));
                if debug {
                    eprintln!(
                        "{name} solid-not-blocked at {p:?}: {:.2} from hull 1's open space, depth {depth:.2}",
                        st.false_free.last().unwrap()
                    );
                }
            }
            if truth || recon || st.traces >= samples / 3 {
                continue;
            }
            let yaw = rng.next() * std::f64::consts::TAU;
            let pitch = (rng.next() - 0.5) * 0.6;
            let len = 32.0 + rng.next() * 224.0;
            let d = [
                yaw.cos() * pitch.cos() * len,
                yaw.sin() * pitch.cos() * len,
                pitch.sin() * len,
            ];
            let e = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            let t = b.trace(0, 1, p, e);
            if t.started_solid {
                continue;
            }
            // Hull 1's end is kept 1/32 off the plane; compare contacts.
            let hit1 = if t.fraction < 1.0 {
                t.fraction * len + 1.0 / 32.0
            } else {
                len
            };
            let hit2 = grid.sweep(p, d, half) * len;
            st.traces += 1;
            st.trace_err.push((hit1.min(len) - hit2).abs());
        }
        out.push(st.line(name));
    }
    out
}

#[test]
fn hull1_against_the_reconstructed_shapes() {
    let Some(m) = mount() else { return };
    let samples: usize = std::env::var("CHIMERA_GOLDSRC_SPIKE_SAMPLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1500);
    let maps = std::env::var("CHIMERA_GOLDSRC_SPIKE_MAPS").unwrap_or_else(|_| "de_dust2".into());
    for map in maps.split(',').map(str::trim).filter(|m| !m.is_empty()) {
        let b = Bsp::parse(&m.read(&format!("maps/{map}.bsp")).unwrap()).unwrap();
        let lines = measure(map, &b, samples);
        for l in &lines {
            eprintln!("{l}");
        }
        assert!(lines.len() >= 5);
    }
}
