//! `map_fitness`'s spawn-safety measures (`--slice`, `--safety`;
//! docs/plans/active/arena-slices.md, section 5 and A3): on the map or
//! the arena slice the tool loaded, the visibility table between the spawn
//! spots and its measures (`chimera_sim::minigame::slice_safety`), the
//! spawn points' exposure, cover, long sight, the vertical range, choke
//! points and the spawn points' spacing, judged by map_fit.toml's
//! `[slice]` (`map_fit::SliceLimits`); on a slice again with the spawn
//! points it makes for itself (`arena::slice_points`), and a top-down
//! picture (written as an uncompressed PNG: no dependency).
//!
//! Known shortcuts: chokes are counted on 48 x 48 sampled walks between
//! the mesh's areas (a share of walks crossing one area, not a cut); the
//! picture's floor is the first surface a ray down meets (a roof hides the
//! floor under it).

use super::*;
use chimera_sim::minigame::{
    arena,
    map_fit::{FitSettings, SliceVerdict},
    slice_safety::{self, Report, SeeTable},
};
use chimera_world::map::play_area::{PlanPolygon, PlayArea};

/// What `safety` measures on.
pub(super) struct SafetyInput<'a> {
    pub id: &'a str,
    /// The spawn spots (the 1.5 m grid; the spawn points without them) and
    /// the spawn points' feet.
    pub spots: &'a [Vec3],
    pub feet: &'a [Vec3],
    /// The bots' mesh and its main region.
    pub nav: &'a NavMesh,
    pub ids: &'a [usize],
    pub area_m2: f32,
    pub sight_m: f32,
    /// Source's map (inches in the map's own units).
    pub game_units: bool,
}

/// The measures (module doc): printed, written to `<out>/<key>_safety.txt`
/// and a row per report of `<out>/safety.csv`; on a slice (or with
/// `--dump`) a picture `<out>/<key>.png`.
pub(super) fn safety(w: &mut World, o: &Opts, c: &SafetyInput) {
    let fit = FitSettings::shipped();
    let area = w.get_resource::<PlayArea>().cloned();
    let key = o.slice.clone().unwrap_or_else(|| c.id.to_string());
    let seed = chimera_sim::weapon::random::crc32(key.as_bytes());
    let mut state = SystemState::<SpatialQuery>::new(w);
    let q = state.get(w).expect("the spatial query");
    let filter = SpatialQueryFilter::from_mask(chimera_core::core::SOLID_LAYERS);
    let sees = arena::sees_in(&q, &filter);
    let ray = |from: Vec3, dir: Vec3, max: f32| Dir3::new(dir).ok().and_then(|d| q.cast_ray(from, d, max, true, &filter)).map(|h| h.distance);
    let t = Instant::now();
    let table = SeeTable::build(&slice_safety::sample(c.spots, o.table), &sees);
    let table_s = t.elapsed().as_secs_f32();
    let (cover, long_sight_p95_m) = slice_safety::level_rays(&table.spots, &ray);
    let base = Report {
        area_m2: c.area_m2,
        sight_m: c.sight_m,
        safety: slice_safety::Safety::of(&table, seed),
        cover,
        long_sight_p95_m,
        ..Default::default()
    };
    let with_points = |points: &[Vec3]| {
        let (p, f) = slice_safety::point_safety(points, &table.spots, &sees);
        Report {
            points: p,
            floor_sees_point: f,
            ..base.clone()
        }
    };
    let before = with_points(c.feet);
    // The floors' heights under the spots: the vertical range.
    let mut ys: Vec<f32> = c.spots.iter().map(|s| s.y).collect();
    ys.sort_by(f32::total_cmp);
    let vertical = (pct(&ys, 0.05), pct(&ys, 0.95));
    let (choke, choke_at) = chokes(c.nav, c.ids);
    let unit = |p: Vec3| {
        if c.game_units {
            format!("{:.0} {:.0} {:.0}", p.x / 0.0254, -p.z / 0.0254, p.y / 0.0254)
        } else {
            format!("{:.1} {:.1} {:.1} m", p.x, p.y, p.z)
        }
    };
    let mut out = vec![
        format!(
            "{key}: spawn safety ({} spots, {} in the visibility table: {:.1} s; spawn points below in engine x, y, z, m)",
            c.spots.len(),
            table.len(),
            table_s
        ),
        format!(
            "vertical range {:.1} m (floors {:.1} to {:.1}, 5th to 95th percentile); choke: {:.0}% of 48 x 48 sampled walks cross one area, at {}",
            vertical.1 - vertical.0,
            vertical.0,
            vertical.1,
            100.0 * choke,
            choke_at.map_or("-".into(), unit)
        ),
    ];
    let judge = |r: &Report, out: &mut Vec<String>, what: &str| -> SliceVerdict {
        let v = fit.slice.judge(r, &fit);
        out.push(format!("--- {what}"));
        out.push(format!(
            "spawn spacing: {} spawn points, nearest pair {:.1} m, median pair {:.1} m",
            r.points.len(),
            pair_dist(r, 0.0),
            pair_dist(r, 0.5)
        ));
        out.push(format!(
            "THE TWO NUMBERS: a spawn spot is seen by {:.1}% of the other spawn spots (p90 {:.1}%); {:.1}% of the walkable floor sees at least one spawn point",
            100.0 * r.safety.spot_seen_by,
            100.0 * r.safety.spot_seen_by_p90,
            100.0 * r.floor_sees_point
        ));
        out.extend(r.lines());
        out.push(match v.players {
            Some([lo, hi]) => format!("PASSES for {lo} to {hi} players"),
            None => format!("FAILS: {}", v.failed.join("; ")),
        });
        v
    };
    let v = judge(&before, &mut out, "the map's spawn points inside (and a slice's seed)");
    let mut rows = vec![("before", before, v)];
    let mut made: Vec<Vec3> = Vec::new();
    let mut alone: Vec<Vec3> = Vec::new();
    if let Some(a) = &area {
        let spots = &c.spots[..c.spots.len().min(chimera_sim::minigame::spawns::MAX_SPOTS)];
        made = arena::slice_points(a, spots, c.feet, &q, &[]);
        let all: Vec<Vec3> = c.feet.iter().chain(&made).copied().collect();
        let after = with_points(&all);
        let v = judge(&after, &mut out, &format!("with the {} spawn points the slice makes", made.len()));
        if o.remeasure {
            // The slice as played: the map's points inside and the ones it
            // makes.
            let (game, map) = a.slice.map.split_once(':').unwrap_or(("", &a.slice.map));
            let file = o.repo.join("games").join(game).join("data").join("arena_slices.toml");
            match super::candidates::remeasure(&file, game, map, &a.slice.id, &super::candidates::measure_lines(&after, v.players)) {
                Ok(()) => out.push(format!("measures written into {}", file.display())),
                Err(e) => out.push(format!("--remeasure: {e}")),
            }
        }
        rows.push(("after", after, v));
        // As on a map none of whose spawn points is inside: the seed's and
        // the ones the slice makes alone.
        let seed_feet = [a.slice.seed];
        let mine = arena::slice_points(a, spots, &seed_feet, &q, &[]);
        let all: Vec<Vec3> = seed_feet.iter().chain(&mine).copied().collect();
        let own = with_points(&all);
        let v = judge(&own, &mut out, &format!("the seed's and the {} spawn points the slice makes with none of the map's", mine.len()));
        rows.push(("own", own, v));
        alone = mine;
    }
    let text = out.join("\n");
    println!("{text}");
    let stem = key.replace(':', "_");
    let _ = fs::write(o.out.join(format!("{stem}_safety.txt")), &text);
    let csv = o.out.join("safety.csv");
    let fresh = !csv.exists();
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&csv) {
        if fresh {
            let _ = writeln!(
                f,
                "key,which,points,area_m2,sight_m,spots,see_pairs,spot_seen_by,spot_seen_by_p90,seen_2,seen_4,seen_8,seen_12,seen_16,cover,long_sight_p95_m,long_pair_m,floor_sees_point,point_floor_seen,point_seen_by,vertical_m,choke,players,failed"
            );
        }
        for (what, r, v) in &rows {
            let s = &r.safety;
            let _ = writeln!(
                f,
                "{key},{what},{},{:.0},{:.1},{},{:.3},{:.3},{:.3},{},{:.3},{:.1},{:.0},{:.3},{:.3},{:.2},{:.1},{:.2},{},{}",
                r.points.len(),
                r.area_m2,
                r.sight_m,
                s.spots,
                s.see_pairs,
                s.spot_seen_by,
                s.spot_seen_by_p90,
                s.seen_at_spawn.iter().map(|(_, v)| format!("{v:.3}")).collect::<Vec<_>>().join(","),
                r.cover,
                r.long_sight_p95_m,
                s.long_pair_m,
                r.floor_sees_point,
                r.point_floor_seeing(),
                r.point_seen_by(),
                vertical.1 - vertical.0,
                choke,
                v.players.map_or("-".into(), |[a, b]| format!("{a}-{b}")),
                v.failed.join("; ").replace(',', " ")
            );
        }
    }
    if area.is_some() || o.dump {
        let file = o.out.join(format!("{stem}.png"));
        let plan = area.as_ref().map(|a| a.polygon);
        let seed_at = area.as_ref().map(|a| a.slice.seed);
        top_down(&q, &file, c, &table, plan, &made, &alone, seed_at, &[], &[]);
        eprintln!("  picture: {}", file.display());
    }
}

/// The `q` quantile of the straight distances between `r`'s spawn
/// points' pairs.
fn pair_dist(r: &Report, q: f32) -> f32 {
    let mut d: Vec<f32> = Vec::new();
    for i in 0..r.points.len() {
        for j in i + 1..r.points.len() {
            d.push(r.points[i].at.distance(r.points[j].at));
        }
    }
    d.sort_by(f32::total_cmp);
    pct(&d, q)
}

/// Choke points: walks between 48 areas of the main region and 48 others
/// (evenly drawn), the share of them crossing the most crossed area, and
/// where it is.
fn chokes(nav: &NavMesh, ids: &[usize]) -> (f32, Option<Vec3>) {
    if ids.len() < 4 {
        return (0.0, None);
    }
    let pick = |k: usize, off: usize| ids[(k * ids.len() / 48 + off) % ids.len()];
    let mut count = vec![0u32; nav.areas.len()];
    let mut walks = 0u32;
    for k in 0..48 {
        let from = pick(k, 0);
        let (dist, prev) = dijkstra_prev(nav, from);
        for m in 0..48 {
            let mut at = pick(m, ids.len() / 96 + 1);
            if !dist[at].is_finite() || at == from {
                continue;
            }
            walks += 1;
            // The areas between (the ends left out).
            while let Some(p) = prev[at] {
                if p == from {
                    break;
                }
                count[p] += 1;
                at = p;
            }
        }
    }
    let best = (0..count.len()).max_by_key(|i| count[*i]).unwrap_or(0);
    (count[best] as f32 / walks.max(1) as f32, (count[best] > 0).then(|| nav.areas[best].center))
}

/// Walking distance (walk links only) with each area's previous one on its
/// path.
fn dijkstra_prev(nav: &NavMesh, from: usize) -> (Vec<f32>, Vec<Option<usize>>) {
    let mut d = vec![f32::INFINITY; nav.areas.len()];
    let mut prev = vec![None; nav.areas.len()];
    let mut heap = BinaryHeap::new();
    d[from] = 0.0;
    heap.push(Item(0.0, from));
    while let Some(Item(at, i)) = heap.pop() {
        if at > d[i] {
            continue;
        }
        let a = &nav.areas[i];
        for (j, via) in &a.links {
            if !matches!(via, Via::Walk(_)) {
                continue;
            }
            let next = at + a.center.distance(nav.areas[*j].center);
            if next < d[*j] {
                d[*j] = next;
                prev[*j] = Some(i);
                heap.push(Item(next, *j));
            }
        }
    }
    (d, prev)
}

/// A top-down picture (x right, z down): the floor's height in grey
/// (rays down; outside the polygon darker), a 10 m grid, the slice's
/// polygon in blue, each table spot a dot from green (seen by none of the
/// others) to red (by a third of them or more), the map's spawn points
/// white, the slice's made ones yellow, the ones it makes with none of the
/// map's orange, its seed magenta; `open` (the wall's samples over open
/// floor, `--candidates`) red, `roofed` (under a roof) violet.
#[allow(clippy::too_many_arguments)]
pub(super) fn top_down(
    q: &SpatialQuery,
    file: &std::path::Path,
    c: &SafetyInput,
    table: &SeeTable,
    plan: Option<PlanPolygon>,
    made: &[Vec3],
    alone: &[Vec3],
    seed: Option<Vec3>,
    open: &[Vec2],
    roofed: &[Vec2],
) {
    let (mut lo, mut hi) = match plan {
        Some(p) => p.bounds(),
        None => c.ids.iter().fold((Vec2::MAX, Vec2::MIN), |(l, h), i| (l.min(c.nav.areas[*i].min), h.max(c.nav.areas[*i].max))),
    };
    lo -= Vec2::splat(12.0);
    hi += Vec2::splat(12.0);
    let ext = hi - lo;
    let ppm = (1600.0 / ext.max_element()).min(6.0);
    let (wd, ht) = ((ext.x * ppm) as usize + 1, (ext.y * ppm) as usize + 1);
    let mut img = vec![[0u8; 3]; wd * ht];
    let floor_hi = c.spots.iter().map(|s| s.y).fold(f32::MIN, f32::max);
    let floor_lo = c.spots.iter().map(|s| s.y).fold(f32::MAX, f32::min);
    let top = floor_hi + 30.0;
    let span = (floor_hi - floor_lo + 10.0).max(1.0);
    let filter = SpatialQueryFilter::from_mask(chimera_core::core::SOLID_LAYERS);
    for py in 0..ht {
        for px in 0..wd {
            let x = lo.x + (px as f32 + 0.5) / ppm;
            let z = lo.y + (py as f32 + 0.5) / ppm;
            let g = q
                .cast_ray(Vec3::new(x, top, z), Dir3::NEG_Y, top - floor_lo + 40.0, true, &filter)
                .map_or(0.0, |h| ((top - h.distance - (floor_lo - 5.0)) / span).clamp(0.0, 1.0));
            let v = (40.0 + 170.0 * g) as u8;
            let inside = plan.is_none_or(|p| p.contains(Vec2::new(x, z)));
            img[py * wd + px] = if inside { [v, v, v] } else { [v / 2, v / 2, v / 2] };
        }
    }
    let to_px = |p: Vec2| (((p.x - lo.x) * ppm) as i64, ((p.y - lo.y) * ppm) as i64);
    let put = |img: &mut Vec<[u8; 3]>, x: i64, y: i64, col: [u8; 3]| {
        if x >= 0 && y >= 0 && (x as usize) < wd && (y as usize) < ht {
            img[y as usize * wd + x as usize] = col;
        }
    };
    // The 10 m grid, dotted.
    let mut gx = (lo.x / 10.0).ceil() * 10.0;
    while gx < hi.x {
        let (x, _) = to_px(Vec2::new(gx, lo.y));
        for y in (0..ht as i64).step_by(3) {
            put(&mut img, x, y, [110, 110, 80]);
        }
        gx += 10.0;
    }
    let mut gz = (lo.y / 10.0).ceil() * 10.0;
    while gz < hi.y {
        let (_, y) = to_px(Vec2::new(lo.x, gz));
        for x in (0..wd as i64).step_by(3) {
            put(&mut img, x, y, [110, 110, 80]);
        }
        gz += 10.0;
    }
    if let Some(p) = plan {
        for (a, b) in p.edges() {
            let n = (a.distance(b) * ppm * 2.0) as i64 + 1;
            for k in 0..=n {
                let (x, y) = to_px(a.lerp(b, k as f32 / n as f32));
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1), (-1, 0), (0, -1)] {
                    put(&mut img, x + dx, y + dy, [59, 123, 255]);
                }
            }
        }
    }
    for p in open {
        let (x, y) = to_px(*p);
        for dy in -2..=2 {
            for dx in -2..=2 {
                put(&mut img, x + dx, y + dy, [255, 40, 40]);
            }
        }
    }
    for p in roofed {
        let (x, y) = to_px(*p);
        for dy in -3..=3 {
            for dx in -3..=3 {
                put(&mut img, x + dx, y + dy, [200, 0, 255]);
            }
        }
    }
    let n = table.len().max(2);
    for (i, s) in table.spots.iter().enumerate() {
        let share = (table.seen_by(i) as f32 / (n - 1) as f32 * 3.0).min(1.0);
        let col = [(255.0 * share) as u8, (220.0 * (1.0 - share)) as u8, 40];
        let (x, y) = to_px(s.xz());
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            put(&mut img, x + dx, y + dy, col);
        }
    }
    let mark = |img: &mut Vec<[u8; 3]>, p: Vec3, col: [u8; 3], r: i64| {
        let (x, y) = to_px(p.xz());
        for dy in -r..=r {
            for dx in -r..=r {
                let edge = dx.abs() == r || dy.abs() == r;
                put(img, x + dx, y + dy, if edge { [0, 0, 0] } else { col });
            }
        }
    };
    for f in c.feet {
        mark(&mut img, *f, [255, 255, 255], 5);
    }
    for m in made {
        mark(&mut img, *m, [255, 230, 0], 5);
    }
    for m in alone {
        mark(&mut img, *m, [255, 140, 0], 4);
    }
    if let Some(s) = seed {
        mark(&mut img, s, [255, 0, 255], 3);
    }
    let bytes: Vec<u8> = img.iter().flatten().copied().collect();
    if let Err(e) = fs::write(file, png(wd as u32, ht as u32, &bytes)) {
        eprintln!("{}: {e}", file.display());
    }
}

/// An RGB picture as PNG bytes (stored deflate blocks, not compressed).
fn png(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
    use chimera_sim::weapon::random::crc32;
    let mut raw = Vec::with_capacity(rgb.len() + h as usize);
    for row in rgb.chunks(w as usize * 3) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, b) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        let len = b.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for x in &raw {
        a = (a + *x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}
