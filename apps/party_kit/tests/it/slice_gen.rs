//! The arena-slice candidate generator on toy maps drawn in code
//! (`map_fitness`'s `slice_gen`, included from the tool's source;
//! docs/plans/active/arena-slices.md, A5): two rooms and a corridor cut at
//! the corridor, a ring cut twice across, an open field yielding no
//! passing candidate while a walled maze yields one, and the outline a
//! simple polygon.

#[allow(dead_code)]
#[path = "../../src/bin/map_fitness/slice_gen.rs"]
mod slice_gen;

use bevy::prelude::*;
use chimera_sim::minigame::map_fit::FitSettings;
use slice_gen::{Band, CellIndex, Graph, Input, Settings, generate, grow, outline};

/// Text rows of `w` columns: `.` floor, `#` none.
fn rows(lines: &[String]) -> Vec<&str> {
    lines.iter().map(String::as_str).collect()
}

/// Two 20 x 20 m rooms joined by a corridor 2 m wide and 10 m long (1 m
/// cells).
fn two_rooms() -> Vec<String> {
    let mut v = Vec::new();
    for r in 0..20 {
        let corridor = r == 9 || r == 10;
        let mid = if corridor { ".".repeat(10) } else { "#".repeat(10) };
        v.push(format!("{}{mid}{}", ".".repeat(20), ".".repeat(20)));
    }
    v
}

#[test]
fn two_rooms_and_a_corridor_are_cut_at_the_corridor() {
    let g = Graph::from_text(&rows(&two_rooms()), 1.0);
    let seed = g.cells.iter().position(|c| c.min == Vec2::new(10.0, 10.0)).unwrap();
    // Aimed at one room: the core stays in it, the ring reaches the other.
    let grown = grow(&g, seed, 400.0).expect("grows");
    for (i, c) in g.cells.iter().enumerate() {
        let x = c.min.x;
        if x < 20.0 {
            assert!(grown.region[i], "room A's cell at {x} left out");
        } else if x >= 30.0 {
            assert!(!grown.region[i], "room B's cell at {x} taken");
        }
    }
    assert!((grown.cut_m - 2.0).abs() < 1e-3, "the cut is the corridor's width: {}", grown.cut_m);
    // Its outline is a simple polygon round room A (and a stub of corridor).
    let pts = outline(&g, &grown.region, 16).expect("an outline");
    let poly = chimera_world::map::play_area::PlanPolygon::new(&pts).unwrap();
    assert!(poly.contains(Vec2::new(10.0, 10.0)) && !poly.contains(Vec2::new(40.0, 10.0)));
    assert!((poly.area() - 400.0).abs() < 60.0, "{}", poly.area());
}

#[test]
fn a_ring_is_cut_twice_across() {
    // A square ring 40 m a side, 5 m wide (1 m cells).
    let lines: Vec<String> = (0..40)
        .map(|r| (0..40).map(|c| if (5..35).contains(&r) && (5..35).contains(&c) { '#' } else { '.' }).collect())
        .collect();
    let g = Graph::from_text(&rows(&lines), 1.0);
    let seed = g.cells.iter().position(|c| c.min == Vec2::new(20.0, 2.0)).unwrap();
    let grown = grow(&g, seed, 200.0).expect("grows");
    assert!((grown.cut_m - 10.0).abs() < 1e-3, "two cuts of 5 m: {}", grown.cut_m);
    assert!(grown.region[seed]);
    // The far side of the ring is not in it.
    let far = g.cells.iter().position(|c| c.min == Vec2::new(20.0, 37.0)).unwrap();
    assert!(!grown.region[far]);
}

/// Walls in plan view (segments) a line of sight can't cross.
fn sees_past(walls: &[(Vec2, Vec2)]) -> impl Fn(Vec3, Vec3) -> bool + '_ {
    move |a: Vec3, b: Vec3| {
        let (p, q) = (a.xz(), b.xz());
        let side = |o: Vec2, x: Vec2, y: Vec2| (x - o).perp_dot(y - o);
        !walls.iter().any(|(c, d)| (side(*c, *d, p) > 0.0) != (side(*c, *d, q) > 0.0) && (side(p, q, *c) > 0.0) != (side(p, q, *d) > 0.0))
    }
}

fn ray_past(walls: &[(Vec2, Vec2)]) -> impl Fn(Vec3, Vec3, f32) -> Option<f32> + '_ {
    move |from: Vec3, dir: Vec3, max: f32| {
        let (p, d) = (from.xz(), dir.xz());
        if d.length_squared() < 1e-6 {
            return None;
        }
        walls
            .iter()
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
            .reduce(f32::min)
            .filter(|t| *t <= max)
    }
}

/// Every 1.5 m over `size` x `size` m.
fn spots(size: f32) -> Vec<Vec3> {
    let mut v = Vec::new();
    let mut x = 0.75;
    while x < size {
        let mut z = 0.75;
        while z < size {
            v.push(Vec3::new(x, 0.0, z));
            z += 1.5;
        }
        x += 1.5;
    }
    v
}

/// The generator over a `size` m square of 2 m cells with `walls`.
fn run(size: f32, walls: &[(Vec2, Vec2)]) -> Vec<slice_gen::Candidate> {
    let n = (size / 2.0) as usize;
    let lines: Vec<String> = (0..n).map(|_| ".".repeat(n)).collect();
    let g = Graph::from_text(&rows(&lines), 2.0);
    let index = CellIndex::new(&g);
    let spots = spots(size);
    let score = vec![0.5; g.len()];
    let fit = FitSettings::shipped();
    let input = Input {
        graph: &g,
        index: &index,
        spots: &spots,
        points: &[],
        score: &score,
        must: &[],
        sight_m: 20.0,
        sees: sees_past(walls),
        ray: ray_past(walls),
    };
    let settings = Settings {
        seeds: 4,
        table: 400,
        ..Settings::default()
    };
    generate(&input, Band { lo: 2, hi: 4 }, &settings, &fit, "toy:field")
}

#[test]
fn an_open_field_yields_no_passing_candidate() {
    let all = run(100.0, &[]);
    assert!(!all.is_empty(), "candidates are made");
    for c in &all {
        assert!(!c.passes(), "an open field passed: {:?}", c.fails);
        assert!(c.fails.iter().any(|f| f.starts_with("cover") || f.starts_with("see_pairs")), "{:?}", c.fails);
        assert!(c.poly.points().len() <= 16);
    }
}

#[test]
fn a_walled_maze_yields_a_passing_candidate() {
    // 10 m rooms, each wall with a 2 m door, staggered.
    let mut walls = Vec::new();
    let size = 100.0;
    for k in 1..10 {
        let c = k as f32 * 10.0;
        for cell in 0..10 {
            let lo = cell as f32 * 10.0;
            let door = lo + if (k + cell) % 2 == 0 { 2.0 } else { 6.0 };
            walls.push((Vec2::new(c, lo), Vec2::new(c, door)));
            walls.push((Vec2::new(c, door + 2.0), Vec2::new(c, lo + 10.0)));
            walls.push((Vec2::new(lo, c), Vec2::new(door, c)));
            walls.push((Vec2::new(door + 2.0, c), Vec2::new(lo + 10.0, c)));
        }
    }
    let _ = size;
    let all = run(100.0, &walls);
    assert!(all.iter().any(|c| c.passes()), "{:?}", all.iter().map(|c| &c.fails).collect::<Vec<_>>());
}
